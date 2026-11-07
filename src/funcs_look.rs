//! Lookup, reference, dynamic-array and database functions.

use crate::eval::*;
use crate::formula::{self, Expr};
use crate::funcs::*;
use crate::model::*;
use std::cmp::Ordering;
use std::sync::Arc;

fn scalar_of(c: &Cx, v: &Value) -> Value {
    c.ctx.scalar(v.clone(), Some((c.sheet, c.pos)))
}

/// Binary-style "last value not above the key" scan, tolerant of blanks.
fn approx_pos(list: &[Value], key: &Value) -> Option<usize> {
    let (mut lo, mut hi) = (0usize, list.len());
    while lo < hi {
        let m = (lo + hi) / 2;
        match compare(&list[m], key) {
            Some(Ordering::Greater) => hi = m,
            _ => lo = m + 1,
        }
    }
    let mut i = lo;
    while i > 0 {
        i -= 1;
        if std::mem::discriminant(&list[i]) == std::mem::discriminant(key) {
            return Some(i);
        }
    }
    None
}

fn eq_match(x: &Value, key: &Value, wildcard: Option<&Vec<char>>) -> bool {
    match (x, key) {
        (Value::Text(a), Value::Text(_)) if wildcard.is_some() => {
            wild(wildcard.unwrap(), &a.to_lowercase().chars().collect::<Vec<_>>())
        }
        (Value::Text(a), Value::Text(b)) => a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase(),
        (Value::Num(a), Value::Num(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Err(a), Value::Err(b)) => a == b,
        _ => false,
    }
}

fn wild_pat(key: &Value) -> Option<Vec<char>> {
    match key {
        Value::Text(t) if t.contains(['*', '?', '~']) => Some(t.to_lowercase().chars().collect()),
        _ => None,
    }
}

fn exact_pos(list: &[Value], key: &Value) -> Option<usize> {
    let p = wild_pat(key);
    list.iter().position(|x| eq_match(x, key, p.as_ref()))
}

/// MATCH modes 1, 0, -1.
fn match_pos(list: &[Value], key: &Value, mode: i32) -> Option<usize> {
    match mode {
        0 => exact_pos(list, key),
        m if m > 0 => approx_pos(list, key),
        _ => {
            // Descending list: last value not below the key.
            let mut best = None;
            for (i, x) in list.iter().enumerate() {
                match compare(x, key) {
                    Some(Ordering::Less) => break,
                    Some(_) => best = Some(i),
                    None => {}
                }
            }
            best
        }
    }
}

/// XMATCH / XLOOKUP search.
fn x_pos(list: &[Value], key: &Value, mode: i32, search: i32) -> Option<usize> {
    let idx: Vec<usize> = if search < 0 { (0..list.len()).rev().collect() } else { (0..list.len()).collect() };
    let p = if mode == 2 { wild_pat(key) } else { None };
    for &i in &idx {
        if eq_match(&list[i], key, p.as_ref()) {
            return Some(i);
        }
    }
    if mode == -1 || mode == 1 {
        let mut best: Option<usize> = None;
        for &i in &idx {
            let v = &list[i];
            let side = compare(v, key);
            let good = if mode == -1 { side == Some(Ordering::Less) } else { side == Some(Ordering::Greater) };
            if !good || std::mem::discriminant(v) != std::mem::discriminant(key) {
                continue;
            }
            best = match best {
                None => Some(i),
                Some(b) => {
                    let o = compare(v, &list[b]);
                    let better = if mode == -1 { o == Some(Ordering::Greater) } else { o == Some(Ordering::Less) };
                    Some(if better { i } else { b })
                }
            };
        }
        return best;
    }
    None
}

fn sub_ref(v: &Value, r: usize, c: usize, h: usize, w: usize) -> Option<Value> {
    if let Value::Ref(si, rc) = v {
        let (r, c) = (rc.r1 + r as u32, rc.c1 + c as u32);
        return Some(Value::Ref(*si, Rect::new(r, c, r + h as u32 - 1, c + w as u32 - 1)));
    }
    None
}

fn index_val(c: &Cx, v: &Value, row: i64, col: i64) -> R {
    let (rows, cols) = match v {
        Value::Ref(_, rc) => (rc.rows() as i64, rc.cols() as i64),
        x => { let a = c.arr(x); (a.rows as i64, a.cols as i64) }
    };
    if row < 0 || col < 0 || row > rows || col > cols {
        return Err(XlErr::Ref);
    }
    let (r0, h) = if row == 0 { (0, rows) } else { (row - 1, 1) };
    let (c0, w) = if col == 0 { (0, cols) } else { (col - 1, 1) };
    if let Some(x) = sub_ref(v, r0 as usize, c0 as usize, h as usize, w as usize) {
        return Ok(x);
    }
    let a = c.arr(v);
    let mut data = vec![];
    for i in r0..r0 + h {
        for j in c0..c0 + w {
            data.push(a.get(i as usize, j as usize).clone());
        }
    }
    Ok(Array::new(h as usize, w as usize, data))
}

fn sort_rows(a: &Array, keys: &[(usize, bool)], by_col: bool) -> Value {
    let (n0, m) = if by_col { (a.cols, a.rows) } else { (a.rows, a.cols) };
    let get = |i: usize, k: usize| if by_col { a.get(k, i) } else { a.get(i, k) };
    let mut idx: Vec<usize> = (0..n0).collect();
    idx.sort_by(|x, y| {
        for (k, asc) in keys {
            let (p, q) = (get(*x, *k), get(*y, *k));
            let (be_p, be_q) = (*p == Value::Empty, *q == Value::Empty);
            if be_p != be_q {
                return if be_p { Ordering::Greater } else { Ordering::Less };
            }
            if let Some(o) = compare(p, q) {
                if o != Ordering::Equal {
                    return if *asc { o } else { o.reverse() };
                }
            }
        }
        Ordering::Equal
    });
    let mut data = vec![];
    if by_col {
        for r in 0..m {
            for i in &idx {
                data.push(a.get(r, *i).clone());
            }
        }
        Array::new(m, n0, data)
    } else {
        for i in &idx {
            data.extend(a.row_of(*i));
        }
        Array::new(n0, m, data)
    }
}

fn stack(c: &Cx, a: &[Value], horizontal: bool) -> R {
    let arrs: Vec<Arc<Array>> = a.iter().map(|x| c.arr(x)).collect();
    if horizontal {
        let rows = arrs.iter().map(|x| x.rows).max().unwrap_or(0);
        let mut data = vec![];
        for r in 0..rows {
            for x in &arrs {
                for k in 0..x.cols {
                    data.push(if r < x.rows { x.get(r, k).clone() } else { Value::Err(XlErr::NA) });
                }
            }
        }
        Ok(Array::new(rows, arrs.iter().map(|x| x.cols).sum(), data))
    } else {
        let cols = arrs.iter().map(|x| x.cols).max().unwrap_or(0);
        let mut data = vec![];
        for x in &arrs {
            for r in 0..x.rows {
                for k in 0..cols {
                    data.push(if k < x.cols { x.get(r, k).clone() } else { Value::Err(XlErr::NA) });
                }
            }
        }
        Ok(Array::new(arrs.iter().map(|x| x.rows).sum(), cols, data))
    }
}

fn pick_lines(c: &Cx, a: &[Value], rows: bool) -> R {
    let x = c.arr(&a[0]);
    let lim = if rows { x.rows } else { x.cols } as i64;
    let mut idx = vec![];
    for v in &a[1..] {
        for q in c.nums(std::slice::from_ref(v))? {
            let k = q as i64;
            let i = if k < 0 { lim + k } else { k - 1 };
            if i < 0 || i >= lim {
                return Err(XlErr::Value);
            }
            idx.push(i as usize);
        }
    }
    let mut data = vec![];
    if rows {
        for i in &idx {
            data.extend(x.row_of(*i));
        }
        Ok(Array::new(idx.len(), x.cols, data))
    } else {
        for r in 0..x.rows {
            for i in &idx {
                data.push(x.get(r, *i).clone());
            }
        }
        Ok(Array::new(x.rows, idx.len(), data))
    }
}

fn take_drop(c: &Cx, a: &[Value], take: bool) -> R {
    let x = c.arr(&a[0]);
    let span = |len: usize, v: Option<f64>| -> (usize, usize) {
        let l = len as i64;
        match v {
            None => (0, len),
            Some(q) => {
                let k = q as i64;
                if take {
                    if k >= 0 { (0, k.min(l) as usize) } else { ((l + k).max(0) as usize, len) }
                } else if k >= 0 {
                    (k.min(l) as usize, len)
                } else {
                    (0, (l + k).max(0) as usize)
                }
            }
        }
    };
    let opt = |i: usize| -> Result<Option<f64>, XlErr> {
        match a.get(i) {
            None | Some(Value::Empty) => Ok(None),
            Some(v) => to_num(&scalar_of(c, v)).map(Some),
        }
    };
    let (r0, r1) = span(x.rows, opt(1)?);
    let (c0, c1) = span(x.cols, opt(2)?);
    if r1 <= r0 || c1 <= c0 {
        return Err(XlErr::Calc);
    }
    let mut data = vec![];
    for i in r0..r1 {
        for j in c0..c1 {
            data.push(x.get(i, j).clone());
        }
    }
    Ok(Array::new(r1 - r0, c1 - c0, data))
}

pub fn lookup(r: &mut Reg) {
    r.cat = "Lookup & Reference";
    r.a("VLOOKUP", 3, 4, "lookup_value, table_array, col_index_num, [range_lookup]", "Looks up a value in the first column of a table", |c, a| {
        let key = scalar_of(c, &a[0]);
        if let Value::Err(e) = key { return Err(e); }
        let tab = c.arr(&a[1]);
        let col = to_num(&scalar_of(c, &a[2]))? as usize;
        if col < 1 { return Err(XlErr::Value); }
        if col > tab.cols { return Err(XlErr::Ref); }
        let first = tab.col_of(0);
        let i = match bd(a, 3, true)? {
            true => approx_pos(&first, &key),
            false => exact_pos(&first, &key),
        };
        i.map(|i| tab.get(i, col - 1).clone()).ok_or(XlErr::NA)
    });
    r.a("HLOOKUP", 3, 4, "lookup_value, table_array, row_index_num, [range_lookup]", "Looks up a value in the top row of a table", |c, a| {
        let key = scalar_of(c, &a[0]);
        if let Value::Err(e) = key { return Err(e); }
        let tab = c.arr(&a[1]);
        let row = to_num(&scalar_of(c, &a[2]))? as usize;
        if row < 1 { return Err(XlErr::Value); }
        if row > tab.rows { return Err(XlErr::Ref); }
        let first = tab.row_of(0);
        let i = match bd(a, 3, true)? {
            true => approx_pos(&first, &key),
            false => exact_pos(&first, &key),
        };
        i.map(|i| tab.get(row - 1, i).clone()).ok_or(XlErr::NA)
    });
    r.a("LOOKUP", 2, 3, "lookup_value, lookup_vector, [result_vector]", "Looks up a value in a sorted list", |c, a| {
        let key = scalar_of(c, &a[0]);
        let v = c.arr(&a[1]);
        let list = if v.rows == 1 { v.row_of(0) } else if a.len() == 2 && v.cols > 1 { v.col_of(0) } else { v.data.clone() };
        let i = approx_pos(&list, &key).ok_or(XlErr::NA)?;
        match a.get(2) {
            Some(res) => c.flat(res).get(i).cloned().ok_or(XlErr::NA),
            None => {
                if v.cols > 1 && v.rows > 1 { Ok(v.get(i, v.cols - 1).clone()) } else { Ok(list[i].clone()) }
            }
        }
    });
    r.a("MATCH", 2, 3, "lookup_value, lookup_array, [match_type]", "Position of a value in a list", |c, a| {
        let key = scalar_of(c, &a[0]);
        if let Value::Err(e) = key { return Err(e); }
        let list = c.flat(&a[1]);
        match_pos(&list, &key, nd(a, 2, 1.0)? as i32).map(|i| Value::Num(i as f64 + 1.0)).ok_or(XlErr::NA)
    });
    r.a("XMATCH", 2, 4, "lookup_value, lookup_array, [match_mode], [search_mode]", "Position of a value, modern form", |c, a| {
        let key = scalar_of(c, &a[0]);
        x_pos(&c.flat(&a[1]), &key, nd(a, 2, 0.0)? as i32, nd(a, 3, 1.0)? as i32).map(|i| Value::Num(i as f64 + 1.0)).ok_or(XlErr::NA)
    });
    r.a("XLOOKUP", 3, 6, "lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode]", "Finds a value and returns the matching item", |c, a| {
        let key = scalar_of(c, &a[0]);
        if let Value::Err(e) = key { return Err(e); }
        let la = c.arr(&a[1]);
        let list = c.flat(&a[1]);
        let i = x_pos(&list, &key, nd(a, 4, 0.0)? as i32, nd(a, 5, 1.0)? as i32);
        let ra = c.arr(&a[2]);
        match i {
            Some(i) => {
                if la.rows == 1 {
                    if ra.rows == 1 { return Ok(ra.get(0, i).clone()); }
                    return Ok(Array::new(ra.rows, 1, ra.col_of(i)));
                }
                if ra.cols == 1 { return Ok(ra.get(i, 0).clone()); }
                Ok(Array::new(1, ra.cols, ra.row_of(i)))
            }
            None => match a.get(3) {
                Some(d) if *d != Value::Empty => Ok(d.clone()),
                _ => Err(XlErr::NA),
            },
        }
    });
    r.a("INDEX", 2, 4, "array, row_num, [column_num], [area_num]", "Value at a row and column of a range", |c, a| {
        let row = to_num(&scalar_of(c, &a[1]))? as i64;
        match a.get(2) {
            Some(v) => index_val(c, &a[0], row, to_num(&scalar_of(c, v))? as i64),
            None => {
                let (rows, cols) = match &a[0] {
                    Value::Ref(_, rc) => (rc.rows() as i64, rc.cols() as i64),
                    x => { let q = c.arr(x); (q.rows as i64, q.cols as i64) }
                };
                if rows == 1 && cols > 1 { index_val(c, &a[0], 1, row) } else { index_val(c, &a[0], row, if cols == 1 { 1 } else { 0 }) }
            }
        }
    });
    r.a("OFFSET", 3, 5, "reference, rows, cols, [height], [width]", "A reference offset from a starting point", |c, a| {
        let Value::Ref(si, rc) = a[0] else { return Err(XlErr::Value) };
        let (dr, dc) = (n(a, 1)? as i64, n(a, 2)? as i64);
        let (h, w) = (nd(a, 3, rc.rows() as f64)? as i64, nd(a, 4, rc.cols() as f64)? as i64);
        let (r1, c1) = (rc.r1 as i64 + dr, rc.c1 as i64 + dc);
        if r1 < 0 || c1 < 0 || h == 0 || w == 0 || r1 + h > MAX_ROWS as i64 || c1 + w > MAX_COLS as i64 { return Err(XlErr::Ref); }
        Ok(Value::Ref(si, Rect::new(r1 as u32, c1 as u32, (r1 + h - 1) as u32, (c1 + w - 1) as u32)))
    });
    r.a("INDIRECT", 1, 2, "ref_text, [a1]", "Reference named by text", |c, a| {
        let s = to_text(&scalar_of(c, &a[0]))?;
        let e = formula::parse(&s).map_err(|_| XlErr::Ref)?;
        let mut env = c.env();
        match &e {
            Expr::Ref(_) | Expr::Name(_) => match c.eval(&e, &mut env) {
                v @ Value::Ref(..) => Ok(v),
                _ => Err(XlErr::Ref),
            },
            _ => Err(XlErr::Ref),
        }
    });
    r.a("ROW", 0, 1, "[reference]", "Row number of a reference", |c, a| match a.first() {
        Some(Value::Ref(_, rc)) => {
            if rc.is_single() || rc.rows() == 1 { Ok(Value::Num(rc.r1 as f64 + 1.0)) } else {
                Ok(Array::new(rc.rows() as usize, 1, (rc.r1..=rc.r2).map(|x| Value::Num(x as f64 + 1.0)).collect()))
            }
        }
        Some(_) => Err(XlErr::Value),
        None => Ok(Value::Num(c.pos.row as f64 + 1.0)),
    });
    r.a("COLUMN", 0, 1, "[reference]", "Column number of a reference", |c, a| match a.first() {
        Some(Value::Ref(_, rc)) => {
            if rc.cols() == 1 { Ok(Value::Num(rc.c1 as f64 + 1.0)) } else {
                Ok(Array::new(1, rc.cols() as usize, (rc.c1..=rc.c2).map(|x| Value::Num(x as f64 + 1.0)).collect()))
            }
        }
        Some(_) => Err(XlErr::Value),
        None => Ok(Value::Num(c.pos.col as f64 + 1.0)),
    });
    r.a("ROWS", 1, 1, "array", "Number of rows", |c, a| Ok(Value::Num(match &a[0] { Value::Ref(_, rc) => rc.rows() as f64, x => c.arr(x).rows as f64 })));
    r.a("COLUMNS", 1, 1, "array", "Number of columns", |c, a| Ok(Value::Num(match &a[0] { Value::Ref(_, rc) => rc.cols() as f64, x => c.arr(x).cols as f64 })));
    r.a("AREAS", 1, 1, "reference", "Number of areas in a reference", |_, _| Ok(Value::Num(1.0)));
    r.s("ADDRESS", 2, 5, "row_num, column_num, [abs_num], [a1], [sheet_text]", "Cell address as text", |_, a| {
        let (row, col, ab) = (n(a, 0)? as u32, n(a, 1)? as u32, nd(a, 2, 1.0)? as i32);
        if row < 1 || col < 1 || col > MAX_COLS || row > MAX_ROWS { return Err(XlErr::Value); }
        let s = format!("{}{}{}{}", if ab == 1 || ab == 3 { "$" } else { "" }, col_name(col - 1), if ab == 1 || ab == 2 { "$" } else { "" }, row);
        match a.get(4) {
            Some(v) if *v != Value::Empty => txt(format!("{}!{s}", formula::quote_sheet(&to_text(v)?))),
            _ => txt(s),
        }
    });
    r.a("HYPERLINK", 1, 2, "link_location, [friendly_name]", "Creates a clickable link", |c, a| match a.get(1) {
        Some(v) => Ok(scalar_of(c, v)),
        None => Ok(scalar_of(c, &a[0])),
    });
    r.a("TRANSPOSE", 1, 1, "array", "Flips rows and columns", |c, a| {
        let x = c.arr(&a[0]);
        let mut data = vec![];
        for k in 0..x.cols { data.extend(x.col_of(k)); }
        Ok(Array::new(x.cols, x.rows, data))
    });
    r.a("SORT", 1, 4, "array, [sort_index], [sort_order], [by_col]", "Sorts a range or array", |c, a| {
        let x = c.arr(&a[0]);
        let idx = nd(a, 1, 1.0)? as usize;
        let asc = nd(a, 2, 1.0)? >= 0.0;
        let by_col = bd(a, 3, false)?;
        if idx < 1 || idx > if by_col { x.rows } else { x.cols } { return Err(XlErr::Value); }
        Ok(sort_rows(&x, &[(idx - 1, asc)], by_col))
    });
    r.a("SORTBY", 2, MANY, "array, by_array1, [sort_order1], ...", "Sorts by the values of other ranges", |c, a| {
        let x = c.arr(&a[0]);
        let mut extra = (*x).clone();
        let mut keys = vec![];
        let mut i = 1;
        while i < a.len() {
            let by = c.flat(&a[i]);
            if by.len() != x.rows { return Err(XlErr::Value); }
            let asc = nd(a, i + 1, 1.0)? >= 0.0;
            let k = extra.cols;
            let mut d = Vec::with_capacity(extra.rows * (k + 1));
            for r in 0..extra.rows {
                d.extend(extra.row_of(r));
                d.push(by[r].clone());
            }
            extra = Array { rows: x.rows, cols: k + 1, data: d };
            keys.push((k, asc));
            i += 2;
        }
        let Value::Array(s) = sort_rows(&extra, &keys, false) else { return Err(XlErr::Value) };
        let mut data = vec![];
        for r in 0..s.rows { data.extend(s.row_of(r)[..x.cols].iter().cloned()); }
        Ok(Array::new(x.rows, x.cols, data))
    });
    r.a("FILTER", 2, 3, "array, include, [if_empty]", "Filters a range by a condition", |c, a| {
        let x = c.arr(&a[0]);
        let inc = c.arr(&c.arrayish(a[1].clone()));
        let keep: Vec<bool> = inc.data.iter().map(to_bool).collect::<Result<_, _>>()?;
        let mut data = vec![];
        let (rows, cols);
        if inc.cols == 1 && keep.len() == x.rows {
            for (i, k) in keep.iter().enumerate() { if *k { data.extend(x.row_of(i)); } }
            cols = x.cols;
            rows = data.len() / cols.max(1);
        } else if inc.rows == 1 && keep.len() == x.cols {
            let idx: Vec<usize> = (0..x.cols).filter(|k| keep[*k]).collect();
            for r in 0..x.rows { for k in &idx { data.push(x.get(r, *k).clone()); } }
            cols = idx.len();
            rows = if cols == 0 { 0 } else { x.rows };
        } else {
            return Err(XlErr::Value);
        }
        if data.is_empty() {
            return match a.get(2) { Some(v) => Ok(v.clone()), None => Err(XlErr::Calc) };
        }
        Ok(Array::new(rows, cols, data))
    });
    r.a("UNIQUE", 1, 3, "array, [by_col], [exactly_once]", "Distinct rows of a range", |c, a| {
        let x = c.arr(&a[0]);
        let by_col = bd(a, 1, false)?;
        let once = bd(a, 2, false)?;
        let lines: Vec<Vec<Value>> = if by_col { (0..x.cols).map(|k| x.col_of(k)).collect() } else { (0..x.rows).map(|r| x.row_of(r)).collect() };
        let same = |p: &Vec<Value>, q: &Vec<Value>| p.iter().zip(q).all(|(u, v)| compare(u, v) == Some(Ordering::Equal) && (*u == Value::Empty) == (*v == Value::Empty));
        let mut out: Vec<(Vec<Value>, usize)> = vec![];
        for l in lines {
            match out.iter_mut().find(|o| same(&o.0, &l)) {
                Some(o) => o.1 += 1,
                None => out.push((l, 1)),
            }
        }
        if once { out.retain(|o| o.1 == 1); }
        if out.is_empty() { return Err(XlErr::Calc); }
        let n0 = out.len();
        if by_col {
            let h = x.rows;
            let mut data = vec![];
            for r in 0..h { for o in &out { data.push(o.0[r].clone()); } }
            Ok(Array::new(h, n0, data))
        } else {
            Ok(Array::new(n0, x.cols, out.into_iter().flat_map(|o| o.0).collect()))
        }
    });
    r.a("VSTACK", 1, MANY, "array1, [array2], ...", "Stacks arrays vertically", |c, a| stack(c, a, false));
    r.a("HSTACK", 1, MANY, "array1, [array2], ...", "Stacks arrays horizontally", |c, a| stack(c, a, true));
    r.a("CHOOSEROWS", 2, MANY, "array, row_num1, ...", "Chosen rows of an array", |c, a| pick_lines(c, a, true));
    r.a("CHOOSECOLS", 2, MANY, "array, col_num1, ...", "Chosen columns of an array", |c, a| pick_lines(c, a, false));
    r.a("TAKE", 2, 3, "array, rows, [columns]", "Rows or columns from the start or end", |c, a| take_drop(c, a, true));
    r.a("DROP", 2, 3, "array, rows, [columns]", "Removes rows or columns from the start or end", |c, a| take_drop(c, a, false));
    r.a("TOCOL", 1, 3, "array, [ignore], [scan_by_column]", "Array as one column", |c, a| {
        let x = c.arr(&a[0]);
        let ig = nd(a, 1, 0.0)? as i32;
        let mut d: Vec<Value> = if bd(a, 2, false)? { (0..x.cols).flat_map(|k| x.col_of(k)).collect() } else { x.data.clone() };
        d.retain(|v| !((ig == 1 || ig == 3) && *v == Value::Empty) && !((ig == 2 || ig == 3) && v.is_err()));
        let l = d.len();
        Ok(Array::new(l, 1, d))
    });
    r.a("TOROW", 1, 3, "array, [ignore], [scan_by_column]", "Array as one row", |c, a| {
        let x = c.arr(&a[0]);
        let ig = nd(a, 1, 0.0)? as i32;
        let mut d: Vec<Value> = if bd(a, 2, false)? { (0..x.cols).flat_map(|k| x.col_of(k)).collect() } else { x.data.clone() };
        d.retain(|v| !((ig == 1 || ig == 3) && *v == Value::Empty) && !((ig == 2 || ig == 3) && v.is_err()));
        let l = d.len();
        Ok(Array::new(1, l, d))
    });
    r.a("WRAPROWS", 2, 3, "vector, wrap_count, [pad_with]", "Wraps a vector into rows", |c, a| {
        let d = c.flat(&a[0]);
        let w = n(a, 1)? as usize;
        if w == 0 { return Err(XlErr::Num); }
        let pad = a.get(2).cloned().unwrap_or(Value::Err(XlErr::NA));
        let rows = d.len().div_ceil(w);
        Ok(Array::new(rows, w, (0..rows * w).map(|i| d.get(i).cloned().unwrap_or(pad.clone())).collect()))
    });
    r.a("WRAPCOLS", 2, 3, "vector, wrap_count, [pad_with]", "Wraps a vector into columns", |c, a| {
        let d = c.flat(&a[0]);
        let h = n(a, 1)? as usize;
        if h == 0 { return Err(XlErr::Num); }
        let pad = a.get(2).cloned().unwrap_or(Value::Err(XlErr::NA));
        let cols = d.len().div_ceil(h);
        let mut data = vec![];
        for r in 0..h { for k in 0..cols { data.push(d.get(k * h + r).cloned().unwrap_or(pad.clone())); } }
        Ok(Array::new(h, cols, data))
    });
    r.a("EXPAND", 2, 4, "array, rows, [columns], [pad_with]", "Expands an array to given dimensions", |c, a| {
        let x = c.arr(&a[0]);
        let rows = nd(a, 1, x.rows as f64)? as usize;
        let cols = nd(a, 2, x.cols as f64)? as usize;
        if rows < x.rows || cols < x.cols { return Err(XlErr::Value); }
        let pad = a.get(3).cloned().unwrap_or(Value::Err(XlErr::NA));
        let mut data = vec![];
        for i in 0..rows { for j in 0..cols { data.push(if i < x.rows && j < x.cols { x.get(i, j).clone() } else { pad.clone() }); } }
        Ok(Array::new(rows, cols, data))
    });
}

/// Rows of a database that satisfy a criteria block, and the column to read.
fn db_rows(c: &Cx, a: &[Value]) -> Result<Vec<Value>, XlErr> {
    let db = c.arr(&a[0]);
    let crit = c.arr(&a[2]);
    let head = db.row_of(0);
    let col = match scalar_of(c, &a[1]) {
        Value::Num(k) => k as usize - 1,
        v => {
            let name = to_text(&v)?;
            head.iter().position(|h| matches!(h, Value::Text(t) if t.eq_ignore_ascii_case(&name))).ok_or(XlErr::Value)?
        }
    };
    if col >= db.cols {
        return Err(XlErr::Value);
    }
    // Each criteria row is an OR alternative; its cells are ANDed.
    let mut tests: Vec<Vec<(usize, Crit)>> = vec![];
    let names = crit.row_of(0);
    for r in 1..crit.rows {
        let mut row = vec![];
        for (k, nm) in names.iter().enumerate() {
            let v = crit.get(r, k);
            if *v == Value::Empty {
                continue;
            }
            let Value::Text(t) = nm else { continue };
            let i = head.iter().position(|h| matches!(h, Value::Text(x) if x.eq_ignore_ascii_case(t))).ok_or(XlErr::Value)?;
            row.push((i, Crit::new(v)));
        }
        tests.push(row);
    }
    let mut out = vec![];
    for r in 1..db.rows {
        if tests.is_empty() || tests.iter().any(|t| t.iter().all(|(i, cr)| cr.matches(db.get(r, *i)))) {
            out.push(db.get(r, col).clone());
        }
    }
    Ok(out)
}

fn db_nums(c: &Cx, a: &[Value]) -> Result<Vec<f64>, XlErr> {
    Ok(db_rows(c, a)?.iter().filter_map(|x| if let Value::Num(n) = x { Some(*n) } else { None }).collect())
}

pub fn database(r: &mut Reg) {
    use crate::funcs_stat as s;
    r.cat = "Database";
    const SIG: &str = "database, field, criteria";
    r.a("DSUM", 3, 3, SIG, "Adds matching database entries", |c, a| ok(db_nums(c, a)?.iter().sum()));
    r.a("DAVERAGE", 3, 3, SIG, "Average of matching database entries", |c, a| { let v = db_nums(c, a)?; if v.is_empty() { Err(XlErr::Div0) } else { ok(s::mean(&v)) } });
    r.a("DCOUNT", 3, 3, SIG, "Counts matching numeric entries", |c, a| Ok(Value::Num(db_nums(c, a)?.len() as f64)));
    r.a("DCOUNTA", 3, 3, SIG, "Counts matching non-empty entries", |c, a| Ok(Value::Num(db_rows(c, a)?.iter().filter(|x| **x != Value::Empty).count() as f64)));
    r.a("DMAX", 3, 3, SIG, "Largest matching entry", |c, a| Ok(Value::Num(db_nums(c, a)?.iter().cloned().fold(f64::MIN, f64::max))));
    r.a("DMIN", 3, 3, SIG, "Smallest matching entry", |c, a| Ok(Value::Num(db_nums(c, a)?.iter().cloned().fold(f64::MAX, f64::min))));
    r.a("DPRODUCT", 3, 3, SIG, "Multiplies matching entries", |c, a| ok(db_nums(c, a)?.iter().product()));
    r.a("DSTDEV", 3, 3, SIG, "Sample standard deviation of matches", |c, a| s::stdev(&db_nums(c, a)?, true));
    r.a("DSTDEVP", 3, 3, SIG, "Population standard deviation of matches", |c, a| s::stdev(&db_nums(c, a)?, false));
    r.a("DVAR", 3, 3, SIG, "Sample variance of matches", |c, a| s::variance(&db_nums(c, a)?, true));
    r.a("DVARP", 3, 3, SIG, "Population variance of matches", |c, a| s::variance(&db_nums(c, a)?, false));
    r.a("DGET", 3, 3, SIG, "The single entry that matches", |c, a| {
        let v = db_rows(c, a)?;
        match v.len() { 0 => Err(XlErr::Value), 1 => Ok(v[0].clone()), _ => Err(XlErr::Num) }
    });
}
