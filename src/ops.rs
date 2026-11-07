//! Editing operations on the workbook. Nothing here knows about the screen.

use crate::eval::*;
use crate::formula::{self, Expr};
use crate::model::*;
use crate::numfmt;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

/// The text a cell shows, with its number format applied.
pub fn text_of(wb: &Workbook, calc: &CalcResult, si: usize, r: u32, c: u32) -> String {
    let v = calc.value(wb, si, r, c);
    let st = wb.styles.get(wb.sheets[si].style_id(r, c));
    render_value(&v, st, 11).text
}

pub fn render_value(v: &Value, st: &Style, width: usize) -> numfmt::Rendered {
    match v {
        Value::Empty => numfmt::Rendered::default(),
        Value::Num(n) => numfmt::format_num(&st.num_fmt, *n, width),
        Value::Text(t) => numfmt::format_text(&st.num_fmt, t),
        Value::Bool(b) => numfmt::Rendered { text: if *b { "TRUE" } else { "FALSE" }.into(), ..Default::default() },
        Value::Err(e) => numfmt::Rendered { text: e.text().into(), ..Default::default() },
        Value::Array(a) => match a.data.first() {
            Some(x) => render_value(x, st, width),
            None => numfmt::Rendered::default(),
        },
        _ => numfmt::Rendered::default(),
    }
}

/// What the cell holds, as shown in the formula bar.
pub fn edit_text(wb: &Workbook, si: usize, r: u32, c: u32) -> String {
    let Some(cell) = wb.sheets[si].get(r, c) else { return String::new() };
    let st = wb.styles.get(cell.style);
    match &cell.v {
        Content::Empty => String::new(),
        Content::Num(n) => {
            if numfmt::is_date_format(&st.num_fmt) {
                let has_time = n.fract() != 0.0;
                let code = if *n < 1.0 && has_time {
                    "h:mm:ss AM/PM"
                } else if has_time {
                    "m/d/yyyy h:mm:ss AM/PM"
                } else {
                    "m/d/yyyy"
                };
                numfmt::format_num(code, *n, 20).text
            } else if st.num_fmt.ends_with('%') {
                format!("{}%", num_text(n * 100.0))
            } else {
                num_text(*n)
            }
        }
        Content::Text(t) => {
            // A leading apostrophe keeps text that would read as something else.
            let t2: &str = t;
            if t2.starts_with('=') || !matches!(numfmt::parse_input(t2).content, Content::Text(_)) {
                format!("'{t2}")
            } else {
                t2.to_string()
            }
        }
        Content::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        Content::Err(e) => e.text().into(),
        Content::Formula(e) => format!("={}", formula::print(e)),
    }
}

/// Applies typed text to a cell. Returns an error message for a bad formula.
pub fn set_input(wb: &mut Workbook, si: usize, r: u32, c: u32, input: &str) -> Result<(), String> {
    let cur_style = wb.sheets[si].style_id(r, c);
    let fmt = wb.styles.get(cur_style).num_fmt.clone();
    if &*fmt == "@" && !input.starts_with('=') {
        let content = if input.is_empty() { Content::Empty } else { Content::Text(input.into()) };
        wb.sheet_mut(si).set_content(r, c, content);
        return Ok(());
    }
    if let Some(rest) = input.strip_prefix('\'') {
        wb.sheet_mut(si).set_content(r, c, Content::Text(rest.into()));
        return Ok(());
    }
    if input.len() > 1 && (input.starts_with('=') || (input.starts_with('+') && input[1..].chars().any(|c| c.is_alphabetic()))) {
        let e = formula::parse(input)?;
        check_functions(wb, &e)?;
        wb.sheet_mut(si).set_content(r, c, Content::Formula(Arc::new(e)));
        return Ok(());
    }
    let t = numfmt::parse_input(input);
    wb.sheet_mut(si).set_content(r, c, t.content);
    if let Some(f) = t.fmt {
        if fmt.eq_ignore_ascii_case("general") || numfmt::is_date_format(&fmt) && !numfmt::is_date_format(&f) {
            let mut st = (**wb.styles.get(cur_style)).clone();
            st.num_fmt = f.into();
            let id = wb.styles.intern(st);
            wb.sheet_mut(si).cell_mut(r, c).style = id;
        }
    }
    Ok(())
}

fn check_functions(_wb: &Workbook, _e: &Expr) -> Result<(), String> {
    Ok(())
}

/// Changes styles across rectangles. Whole rows and columns also set the default style.
pub fn style_ranges(wb: &mut Workbook, si: usize, rects: &[Rect], f: &dyn Fn(&mut Style)) {
    let mut memo: HashMap<u32, u32> = HashMap::new();
    let mut map = |wb: &mut Workbook, old: u32| -> u32 {
        *memo.entry(old).or_insert_with(|| {
            let mut s = (**wb.styles.get(old)).clone();
            f(&mut s);
            wb.styles.intern(s)
        })
    };
    for rc in rects {
        if rc.whole_cols() {
            for c in rc.c1..=rc.c2 {
                let old = wb.sheets[si].col_style.get(&c).copied().unwrap_or(0);
                let n = map(wb, old);
                wb.sheet_mut(si).col_style.insert(c, n);
            }
        }
        if rc.whole_rows() {
            for r in rc.r1..=rc.r2 {
                let old = wb.sheets[si].row_style.get(&r).copied().unwrap_or(0);
                let n = map(wb, old);
                wb.sheet_mut(si).row_style.insert(r, n);
            }
        }
        let cells: Vec<(u32, u32, u32)> = if rc.whole_cols() || rc.whole_rows() {
            let mut v = vec![];
            for (&r, m) in wb.sheets[si].rows.range(rc.r1..=rc.r2) {
                for (&c, x) in m.range(rc.c1..=rc.c2) {
                    v.push((r, c, x.style));
                }
            }
            v
        } else {
            let sh = &wb.sheets[si];
            let mut v = Vec::with_capacity((rc.rows() * rc.cols()) as usize);
            for r in rc.r1..=rc.r2 {
                for c in rc.c1..=rc.c2 {
                    v.push((r, c, sh.style_id(r, c)));
                }
            }
            v
        };
        for (r, c, old) in cells {
            let n = map(wb, old);
            if n != old {
                wb.sheet_mut(si).cell_mut(r, c).style = n;
            }
        }
    }
}

/// Rewrites every structural thing on a sheet after a row or column insert or delete.
fn restructure(wb: &mut Workbook, si: usize, rows: bool, at: u32, n: u32, insert: bool) {
    let name = wb.sheets[si].name.clone();
    let own: Vec<String> = wb.sheets.iter().map(|s| s.name.clone()).collect();
    let fix_expr = |e: &mut Expr, fsi: usize| {
        formula::map_refs(e, &mut |r| {
            if !ref_targets(r, &name, &own[fsi]) {
                return true;
            }
            match (rows, insert) {
                (true, true) => {
                    formula::ins_rows(r, at, n);
                    true
                }
                (false, true) => {
                    formula::ins_cols(r, at, n);
                    true
                }
                (true, false) => formula::del_rows(r, at, n),
                (false, false) => formula::del_cols(r, at, n),
            }
        })
    };
    wb.for_each_formula(|fsi, _, e| fix_expr(e, fsi));
    let names_fix = |s: &str, fsi: usize| -> String {
        match formula::parse(s) {
            Ok(mut e) => {
                fix_expr(&mut e, fsi);
                formula::print(&e)
            }
            Err(_) => s.to_string(),
        }
    };
    for i in 0..wb.names.len() {
        // Defined names always carry their sheet; the owner index is irrelevant.
        wb.names[i].expr = names_fix(&wb.names[i].expr, si);
    }
    for i in 0..wb.sheets.len() {
        let has = !wb.sheets[i].charts.is_empty();
        if has {
            let sh = wb.sheet_mut(i);
            for ch in &mut sh.charts {
                for s in &mut ch.series {
                    s.vals = names_fix(&s.vals, i);
                    if let Some(c) = &mut s.cats {
                        *c = names_fix(c, i);
                    }
                    if let Some(c) = &mut s.name_ref {
                        *c = names_fix(c, i);
                    }
                }
            }
        }
    }
    let map_rect = |x: Rect| -> Option<Rect> {
        if insert {
            Some(formula::rect_ins(x, rows, at, n))
        } else {
            formula::rect_del(x, rows, at, n)
        }
    };
    let map_idx = |x: u32| -> Option<u32> {
        if x < at {
            Some(x)
        } else if insert {
            Some(x + n)
        } else if x >= at + n {
            Some(x - n)
        } else {
            None
        }
    };
    let map_pos = |p: Pos| -> Option<Pos> {
        if rows {
            map_idx(p.row).map(|r| Pos::new(r, p.col))
        } else {
            map_idx(p.col).map(|c| Pos::new(p.row, c))
        }
    };
    let sh = wb.sheet_mut(si);
    if rows {
        let old = std::mem::take(&mut sh.rows);
        for (r, m) in old {
            if let Some(nr) = map_idx(r) {
                sh.rows.insert(nr, m);
            }
        }
    } else {
        let old = std::mem::take(&mut sh.rows);
        for (r, m) in old {
            let mut nm = BTreeMap::new();
            for (c, cell) in m.iter() {
                if let Some(nc) = map_idx(*c) {
                    nm.insert(nc, cell.clone());
                }
            }
            if !nm.is_empty() {
                sh.rows.insert(r, Arc::new(nm));
            }
        }
    }
    fn remap_hash<V>(h: &mut HashMap<u32, V>, f: &dyn Fn(u32) -> Option<u32>) {
        let old = std::mem::take(h);
        for (k, v) in old {
            if let Some(k2) = f(k) {
                h.insert(k2, v);
            }
        }
    }
    fn remap_set(h: &mut std::collections::BTreeSet<u32>, f: &dyn Fn(u32) -> Option<u32>) {
        let old = std::mem::take(h);
        for k in old {
            if let Some(k2) = f(k) {
                h.insert(k2);
            }
        }
    }
    if rows {
        remap_hash(&mut sh.row_h, &map_idx);
        remap_hash(&mut sh.row_style, &map_idx);
        remap_set(&mut sh.custom_h, &map_idx);
        remap_set(&mut sh.hidden_rows, &map_idx);
        remap_set(&mut sh.filtered_rows, &map_idx);
        let old = std::mem::take(&mut sh.outline_rows);
        for (k, v) in old {
            if let Some(k2) = map_idx(k) {
                sh.outline_rows.insert(k2, v);
            }
        }
    } else {
        remap_hash(&mut sh.col_w, &map_idx);
        remap_hash(&mut sh.col_style, &map_idx);
        remap_set(&mut sh.hidden_cols, &map_idx);
        let old = std::mem::take(&mut sh.outline_cols);
        for (k, v) in old {
            if let Some(k2) = map_idx(k) {
                sh.outline_cols.insert(k2, v);
            }
        }
    }
    sh.merges = sh.merges.iter().filter_map(|x| map_rect(*x)).filter(|x| !x.is_single()).collect();
    for c in &mut sh.cond {
        c.ranges = c.ranges.iter().filter_map(|x| map_rect(*x)).collect();
    }
    sh.cond.retain(|c| !c.ranges.is_empty());
    for c in &mut sh.valid {
        c.ranges = c.ranges.iter().filter_map(|x| map_rect(*x)).collect();
    }
    sh.valid.retain(|c| !c.ranges.is_empty());
    let notes = std::mem::take(&mut sh.notes);
    for (p, v) in notes {
        if let Some(p2) = map_pos(p) {
            sh.notes.insert(p2, v);
        }
    }
    let links = std::mem::take(&mut sh.links);
    for (p, v) in links {
        if let Some(p2) = map_pos(p) {
            sh.links.insert(p2, v);
        }
    }
    let sparks = std::mem::take(&mut sh.sparks);
    for (p, v) in sparks {
        if let Some(p2) = map_pos(p) {
            sh.sparks.insert(p2, v);
        }
    }
    for t in &mut sh.tables {
        if let Some(x) = map_rect(t.range) {
            t.range = x;
        }
    }
    if let Some(f) = &mut sh.filter {
        match map_rect(f.range) {
            Some(x) => f.range = x,
            None => sh.filter = None,
        }
    }
    if let Some(p) = sh.page.print_area {
        sh.page.print_area = map_rect(p);
    }
}

pub fn insert_rows(wb: &mut Workbook, si: usize, at: u32, n: u32) {
    restructure(wb, si, true, at, n, true);
}
pub fn insert_cols(wb: &mut Workbook, si: usize, at: u32, n: u32) {
    restructure(wb, si, false, at, n, true);
}
pub fn delete_rows(wb: &mut Workbook, si: usize, at: u32, n: u32) {
    restructure(wb, si, true, at, n, false);
}
pub fn delete_cols(wb: &mut Workbook, si: usize, at: u32, n: u32) {
    restructure(wb, si, false, at, n, false);
}

/// Moves a block of cells, and points every reference to them at the new place.
pub fn move_block(wb: &mut Workbook, si: usize, src: Rect, to: Pos) {
    let dr = to.row as i64 - src.r1 as i64;
    let dc = to.col as i64 - src.c1 as i64;
    if dr == 0 && dc == 0 {
        return;
    }
    let name = wb.sheets[si].name.clone();
    let own: Vec<String> = wb.sheets.iter().map(|s| s.name.clone()).collect();
    wb.for_each_formula(|fsi, _, e| {
        formula::map_refs(e, &mut |r| {
            if ref_targets(r, &name, &own[fsi]) && src.contains_rect(&r.rect()) {
                let mut mv = |x: &mut formula::Ref1| {
                    x.row = (x.row as i64 + dr).clamp(0, MAX_ROWS as i64 - 1) as u32;
                    x.col = (x.col as i64 + dc).clamp(0, MAX_COLS as i64 - 1) as u32;
                };
                mv(&mut r.a);
                if let Some(b) = &mut r.b {
                    mv(b);
                }
            }
            true
        })
    });
    let sh = wb.sheet_mut(si);
    let mut cells = vec![];
    let keys: Vec<(u32, u32)> = sh
        .rows
        .range(src.r1..=src.r2)
        .flat_map(|(r, m)| m.range(src.c1..=src.c2).map(move |(c, _)| (*r, *c)))
        .collect();
    for (r, c) in keys {
        if let Some(x) = sh.get(r, c) {
            cells.push((r, c, x.clone()));
        }
        sh.remove(r, c);
    }
    let dst = Rect::new(to.row, to.col, to.row + src.rows() - 1, to.col + src.cols() - 1);
    // Whatever was at the destination is replaced.
    let keys: Vec<(u32, u32)> = sh
        .rows
        .range(dst.r1..=dst.r2)
        .flat_map(|(r, m)| m.range(dst.c1..=dst.c2).map(move |(c, _)| (*r, *c)))
        .collect();
    for (r, c) in keys {
        sh.remove(r, c);
    }
    for (r, c, x) in cells {
        let (nr, nc) = ((r as i64 + dr) as u32, (c as i64 + dc) as u32);
        *sh.cell_mut(nr, nc) = x;
    }
    let moved: Vec<Rect> = sh.merges.iter().copied().filter(|m| src.contains_rect(m)).collect();
    sh.merges.retain(|m| !src.contains_rect(m));
    for m in moved {
        sh.merges.push(Rect::new((m.r1 as i64 + dr) as u32, (m.c1 as i64 + dc) as u32, (m.r2 as i64 + dr) as u32, (m.c2 as i64 + dc) as u32));
    }
}

/// Inserts or deletes cells inside a rectangle, shifting neighbours down/right or up/left.
pub fn shift_cells(wb: &mut Workbook, si: usize, rc: Rect, down: bool, insert: bool) {
    let Some(used) = wb.sheets[si].used_rect() else { return };
    if down {
        let n = rc.rows();
        let tail = Rect::new(if insert { rc.r1 } else { rc.r2 + 1 }, rc.c1, used.r2.max(rc.r2) + 1, rc.c2);
        if insert {
            move_block(wb, si, tail, Pos::new(rc.r1 + n, rc.c1));
        } else {
            clear_rect(wb, si, rc);
            move_block(wb, si, tail, Pos::new(rc.r1, rc.c1));
        }
    } else {
        let n = rc.cols();
        let tail = Rect::new(rc.r1, if insert { rc.c1 } else { rc.c2 + 1 }, rc.r2, used.c2.max(rc.c2) + 1);
        if insert {
            move_block(wb, si, tail, Pos::new(rc.r1, rc.c1 + n));
        } else {
            clear_rect(wb, si, rc);
            move_block(wb, si, tail, Pos::new(rc.r1, rc.c1));
        }
    }
}

pub fn clear_rect(wb: &mut Workbook, si: usize, rc: Rect) {
    let sh = wb.sheet_mut(si);
    let keys: Vec<(u32, u32)> = sh
        .rows
        .range(rc.r1..=rc.r2)
        .flat_map(|(r, m)| m.range(rc.c1..=rc.c2).map(move |(c, _)| (*r, *c)))
        .collect();
    for (r, c) in keys {
        sh.remove(r, c);
    }
}

/// Clears contents but keeps formatting.
pub fn clear_contents(wb: &mut Workbook, si: usize, rects: &[Rect]) {
    let sh = wb.sheet_mut(si);
    for rc in rects {
        let keys: Vec<(u32, u32)> = sh
            .rows
            .range(rc.r1..=rc.r2)
            .flat_map(|(r, m)| m.range(rc.c1..=rc.c2).map(move |(c, _)| (*r, *c)))
            .collect();
        for (r, c) in keys {
            sh.cell_mut(r, c).v = Content::Empty;
            if sh.get(r, c).map(|x| x.style) == Some(0) {
                sh.remove(r, c);
            }
        }
    }
}

pub fn rename_sheet(wb: &mut Workbook, si: usize, new: &str) {
    let old = wb.sheets[si].name.clone();
    let new_arc: Arc<str> = new.into();
    wb.for_each_formula(|_, _, e| {
        formula::map_refs(e, &mut |r| {
            if matches!(&r.sheet, Some(s) if s.eq_ignore_ascii_case(&old)) {
                r.sheet = Some(new_arc.clone());
            }
            true
        })
    });
    let rewrite = |s: &str| -> String {
        match formula::parse(s) {
            Ok(mut e) => {
                formula::map_refs(&mut e, &mut |r| {
                    if matches!(&r.sheet, Some(x) if x.eq_ignore_ascii_case(&old)) {
                        r.sheet = Some(new_arc.clone());
                    }
                    true
                });
                formula::print(&e)
            }
            Err(_) => s.to_string(),
        }
    };
    for n in &mut wb.names {
        n.expr = rewrite(&n.expr);
    }
    for i in 0..wb.sheets.len() {
        if wb.sheets[i].charts.is_empty() {
            continue;
        }
        let sh = wb.sheet_mut(i);
        for ch in &mut sh.charts {
            for s in &mut ch.series {
                s.vals = rewrite(&s.vals);
                s.cats = s.cats.as_deref().map(&rewrite);
                s.name_ref = s.name_ref.as_deref().map(&rewrite);
            }
        }
    }
    wb.sheet_mut(si).name = new.to_string();
}

/// Removes a sheet. Formulas that pointed at it turn into #REF!.
pub fn delete_sheet(wb: &mut Workbook, si: usize) {
    let name = wb.sheets[si].name.clone();
    wb.sheets.remove(si);
    wb.for_each_formula(|_, _, e| {
        formula::map_refs(e, &mut |r| !matches!(&r.sheet, Some(s) if s.eq_ignore_ascii_case(&name)))
    });
}

#[derive(Clone, Debug)]
pub struct ClipCell {
    pub content: Content,
    pub style: Arc<Style>,
}

/// Copied cells, independent of the sheet they came from.
#[derive(Clone, Debug)]
pub struct Clip {
    pub sheet: usize,
    pub src: Rect,
    pub cells: Vec<Vec<Option<ClipCell>>>,
    pub merges: Vec<Rect>,
    pub widths: Vec<f32>,
    pub cut: bool,
}

pub fn copy_rect(wb: &Workbook, si: usize, rc: Rect) -> Clip {
    let sh = &wb.sheets[si];
    let mut cells = Vec::with_capacity(rc.rows() as usize);
    for r in rc.r1..=rc.r2 {
        let mut row = Vec::with_capacity(rc.cols() as usize);
        for c in rc.c1..=rc.c2 {
            row.push(sh.get(r, c).map(|x| ClipCell { content: x.v.clone(), style: wb.styles.get(x.style).clone() }));
        }
        cells.push(row);
    }
    let merges = sh.merges.iter().filter(|m| rc.contains_rect(m)).copied().collect();
    let widths = (rc.c1..=rc.c2).map(|c| sh.col_w.get(&c).copied().unwrap_or(DEFAULT_COL_W)).collect();
    Clip { sheet: si, src: rc, cells, merges, widths, cut: false }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PasteWhat {
    All,
    Values,
    Formulas,
    Formats,
    ValuesFormats,
    NoBorders,
    Widths,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PasteOp {
    None,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug)]
pub struct PasteOpts {
    pub what: PasteWhat,
    pub op: PasteOp,
    pub skip_blanks: bool,
    pub transpose: bool,
}

impl Default for PasteOpts {
    fn default() -> Self {
        PasteOpts { what: PasteWhat::All, op: PasteOp::None, skip_blanks: false, transpose: false }
    }
}

/// Pastes a clip at `dest`. If `dest` is a multiple of the clip's size the clip repeats.
pub fn paste(wb: &mut Workbook, calc: &CalcResult, si: usize, clip: &Clip, dest: Rect, o: PasteOpts) -> Rect {
    let (mut h, mut w) = (clip.src.rows(), clip.src.cols());
    if o.transpose {
        std::mem::swap(&mut h, &mut w);
    }
    let rep_r = if dest.rows() > h && dest.rows() % h == 0 { dest.rows() / h } else { 1 };
    let rep_c = if dest.cols() > w && dest.cols() % w == 0 { dest.cols() / w } else { 1 };
    let out = Rect::new(
        dest.r1,
        dest.c1,
        (dest.r1 + h * rep_r - 1).min(MAX_ROWS - 1),
        (dest.c1 + w * rep_c - 1).min(MAX_COLS - 1),
    );
    if o.what == PasteWhat::Widths {
        for (i, x) in clip.widths.iter().enumerate() {
            wb.sheet_mut(si).col_w.insert(dest.c1 + i as u32, *x);
        }
        return out;
    }
    let num_op = |a: f64, b: f64, op: PasteOp| -> Option<f64> {
        Some(match op {
            PasteOp::Add => a + b,
            PasteOp::Sub => a - b,
            PasteOp::Mul => a * b,
            PasteOp::Div => {
                if b == 0.0 {
                    return None;
                }
                a / b
            }
            PasteOp::None => b,
        })
    };
    for rr in 0..rep_r {
        for cc in 0..rep_c {
            for i in 0..clip.cells.len() as u32 {
                for j in 0..clip.cells[i as usize].len() as u32 {
                    let (di, dj) = if o.transpose { (j, i) } else { (i, j) };
                    let (r, c) = (dest.r1 + rr * h + di, dest.c1 + cc * w + dj);
                    if r >= MAX_ROWS || c >= MAX_COLS {
                        continue;
                    }
                    let src = clip.cells[i as usize][j as usize].as_ref();
                    if o.skip_blanks && src.map_or(true, |x| x.content == Content::Empty) {
                        continue;
                    }
                    let (sr, sc) = (clip.src.r1 + i, clip.src.c1 + j);
                    let content = match src {
                        None => Content::Empty,
                        Some(x) => x.content.clone(),
                    };
                    let want_value = matches!(o.what, PasteWhat::All | PasteWhat::Values | PasteWhat::Formulas | PasteWhat::ValuesFormats | PasteWhat::NoBorders);
                    let want_fmt = matches!(o.what, PasteWhat::All | PasteWhat::Formats | PasteWhat::ValuesFormats | PasteWhat::NoBorders);
                    if want_value {
                        let as_value = matches!(o.what, PasteWhat::Values | PasteWhat::ValuesFormats);
                        let mut new = match &content {
                            Content::Formula(e) if !as_value && !clip.cut => {
                                Content::Formula(Arc::new(formula::shifted(e, r as i64 - sr as i64, c as i64 - sc as i64)))
                            }
                            Content::Formula(e) if !as_value => Content::Formula(e.clone()),
                            Content::Formula(_) => match calc.value(wb, clip.sheet, sr, sc) {
                                Value::Num(n) => Content::Num(n),
                                Value::Text(t) => Content::Text(t),
                                Value::Bool(b) => Content::Bool(b),
                                Value::Err(e) => Content::Err(e),
                                _ => Content::Empty,
                            },
                            x => x.clone(),
                        };
                        if o.op != PasteOp::None {
                            let old = match calc.value(wb, si, r, c) {
                                Value::Num(n) => n,
                                _ => 0.0,
                            };
                            if let Content::Num(b) = new {
                                new = match num_op(old, b, o.op) {
                                    Some(x) => Content::Num(x),
                                    None => Content::Err(XlErr::Div0),
                                };
                            }
                        }
                        wb.sheet_mut(si).set_content(r, c, new);
                    }
                    if want_fmt {
                        let mut st = match src {
                            Some(x) => (*x.style).clone(),
                            None => Style::default(),
                        };
                        if o.what == PasteWhat::NoBorders {
                            st.borders = [None; 4];
                        }
                        let id = wb.styles.intern(st);
                        if id != 0 || wb.sheets[si].get(r, c).is_some() {
                            wb.sheet_mut(si).cell_mut(r, c).style = id;
                        }
                    }
                }
            }
            if o.what == PasteWhat::All && !o.transpose {
                for m in &clip.merges {
                    let sh = wb.sheet_mut(si);
                    let dr = (dest.r1 + rr * h) as i64 - clip.src.r1 as i64;
                    let dc = (dest.c1 + cc * w) as i64 - clip.src.c1 as i64;
                    let x = Rect::new((m.r1 as i64 + dr) as u32, (m.c1 as i64 + dc) as u32, (m.r2 as i64 + dr) as u32, (m.c2 as i64 + dc) as u32);
                    sh.merges.retain(|y| !y.intersects(&x));
                    sh.merges.push(x);
                }
            }
        }
    }
    out
}

pub fn set_merge(wb: &mut Workbook, si: usize, rects: &[Rect], mode: u8) {
    // mode: 0 merge, 1 merge across, 2 unmerge.
    let sh = wb.sheet_mut(si);
    for rc in rects {
        sh.merges.retain(|m| !m.intersects(rc));
        if mode == 2 || rc.is_single() {
            continue;
        }
        let parts: Vec<Rect> = if mode == 1 {
            (rc.r1..=rc.r2).map(|r| Rect::new(r, rc.c1, r, rc.c2)).filter(|x| !x.is_single()).collect()
        } else {
            vec![*rc]
        };
        for p in parts {
            // The merged cell keeps only the upper-left value.
            let first = (p.r1..=p.r2)
                .flat_map(|r| (p.c1..=p.c2).map(move |c| (r, c)))
                .find(|(r, c)| !sh.is_empty_at(*r, *c));
            if let Some((r, c)) = first {
                if (r, c) != (p.r1, p.c1) {
                    let v = sh.get(r, c).unwrap().v.clone();
                    sh.set_content(p.r1, p.c1, v);
                }
            }
            for r in p.r1..=p.r2 {
                for c in p.c1..=p.c2 {
                    if (r, c) != (p.r1, p.c1) && !sh.is_empty_at(r, c) {
                        sh.cell_mut(r, c).v = Content::Empty;
                    }
                }
            }
            sh.merges.push(p);
        }
    }
}

/// Sort keys: (column, ascending). Rows move as units; formulas adjust as if copied.
pub fn sort_range(wb: &mut Workbook, calc: &CalcResult, si: usize, rc: Rect, keys: &[(u32, bool)], by_rows_left_right: bool) {
    use std::cmp::Ordering;
    if by_rows_left_right {
        // Sorting columns by the values in a row.
        let mut idx: Vec<u32> = (rc.c1..=rc.c2).collect();
        idx.sort_by(|a, b| {
            for (k, asc) in keys {
                let (x, y) = (calc.value(wb, si, *k, *a), calc.value(wb, si, *k, *b));
                let o = sort_cmp(&x, &y, *asc);
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        let sh = &wb.sheets[si];
        let snapshot: Vec<Vec<Option<Cell>>> = idx.iter().map(|c| (rc.r1..=rc.r2).map(|r| sh.get(r, *c).cloned()).collect()).collect();
        clear_rect(wb, si, rc);
        for (j, col) in snapshot.into_iter().enumerate() {
            let nc = rc.c1 + j as u32;
            let dc = nc as i64 - idx[j] as i64;
            for (i, cell) in col.into_iter().enumerate() {
                if let Some(mut x) = cell {
                    if let Content::Formula(e) = &x.v {
                        x.v = Content::Formula(Arc::new(formula::shifted(e, 0, dc)));
                    }
                    *wb.sheet_mut(si).cell_mut(rc.r1 + i as u32, nc) = x;
                }
            }
        }
        return;
    }
    let mut idx: Vec<u32> = (rc.r1..=rc.r2).collect();
    let vals: HashMap<(u32, u32), Value> = idx
        .iter()
        .flat_map(|r| keys.iter().map(move |(k, _)| (*r, *k)))
        .map(|(r, k)| ((r, k), calc.value(wb, si, r, k)))
        .collect();
    idx.sort_by(|a, b| {
        for (k, asc) in keys {
            let o = sort_cmp(&vals[&(*a, *k)], &vals[&(*b, *k)], *asc);
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    });
    let sh = &wb.sheets[si];
    let snapshot: Vec<Vec<Option<Cell>>> = idx.iter().map(|r| (rc.c1..=rc.c2).map(|c| sh.get(*r, c).cloned()).collect()).collect();
    clear_rect(wb, si, rc);
    let sh = wb.sheet_mut(si);
    for (i, row) in snapshot.into_iter().enumerate() {
        let nr = rc.r1 + i as u32;
        let dr = nr as i64 - idx[i] as i64;
        for (j, cell) in row.into_iter().enumerate() {
            if let Some(mut x) = cell {
                if let Content::Formula(e) = &x.v {
                    x.v = Content::Formula(Arc::new(formula::shifted(e, dr, 0)));
                }
                *sh.cell_mut(nr, rc.c1 + j as u32) = x;
            }
        }
    }
}

fn sort_cmp(a: &Value, b: &Value, asc: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    // Blanks always sort last, whatever the direction.
    match (a, b) {
        (Value::Empty, Value::Empty) => Equal,
        (Value::Empty, _) => Greater,
        (_, Value::Empty) => Less,
        (Value::Err(_), Value::Err(_)) => Equal,
        (Value::Err(_), _) => Greater,
        (_, Value::Err(_)) => Less,
        _ => {
            let o = compare(a, b).unwrap_or(Equal);
            if asc {
                o
            } else {
                o.reverse()
            }
        }
    }
}

fn custom_ok(v: &Value, shown: &str, op: &str, arg: &str) -> bool {
    let num = numfmt::parse_plain_number(arg);
    let val = match v {
        Value::Num(n) => Some(*n),
        _ => None,
    };
    let (ls, la) = (shown.to_lowercase(), arg.to_lowercase());
    match op {
        "equals" => ls == la || (num.is_some() && num == val),
        "notequals" => !(ls == la || (num.is_some() && num == val)),
        "begins" => ls.starts_with(&la),
        "ends" => ls.ends_with(&la),
        "contains" => ls.contains(&la),
        "notcontains" => !ls.contains(&la),
        "greater" => matches!((val, num), (Some(a), Some(b)) if a > b),
        "greatereq" => matches!((val, num), (Some(a), Some(b)) if a >= b),
        "less" => matches!((val, num), (Some(a), Some(b)) if a < b),
        "lesseq" => matches!((val, num), (Some(a), Some(b)) if a <= b),
        "between" => match (val, arg.split_once(';')) {
            (Some(a), Some((lo, hi))) => match (lo.trim().parse::<f64>(), hi.trim().parse::<f64>()) {
                (Ok(x), Ok(y)) => a >= x && a <= y,
                _ => false,
            },
            _ => false,
        },
        _ => true,
    }
}

/// Recomputes which rows the AutoFilter hides.
pub fn apply_filter(wb: &mut Workbook, calc: &CalcResult, si: usize) {
    let Some(f) = wb.sheets[si].filter.clone() else {
        wb.sheet_mut(si).filtered_rows.clear();
        return;
    };
    let mut hidden = std::collections::BTreeSet::new();
    let last = f.range.r2;
    for r in f.range.r1 + 1..=last {
        for (&off, cf) in &f.cols {
            let c = f.range.c1 + off;
            let shown = text_of(wb, calc, si, r, c);
            let mut keep = true;
            if let Some(a) = &cf.allowed {
                keep = a.contains(&shown);
            }
            if let Some((op, arg)) = &cf.custom {
                keep = keep && custom_ok(&calc.value(wb, si, r, c), &shown, op, arg);
            }
            if !keep {
                hidden.insert(r);
                break;
            }
        }
    }
    wb.sheet_mut(si).filtered_rows = hidden;
}

/// Grows a single cell to the contiguous data block around it.
pub fn current_region(sh: &Sheet, p: Pos) -> Rect {
    let mut rc = Rect::cell(p);
    loop {
        let mut grown = rc;
        let filled_row = |r: u32, rc: &Rect| (rc.c1.saturating_sub(1)..=rc.c2 + 1).any(|c| !sh.is_empty_at(r, c));
        let filled_col = |c: u32, rc: &Rect| (rc.r1.saturating_sub(1)..=rc.r2 + 1).any(|r| !sh.is_empty_at(r, c));
        if rc.r1 > 0 && filled_row(rc.r1 - 1, &rc) {
            grown.r1 -= 1;
        }
        if rc.r2 + 1 < MAX_ROWS && filled_row(rc.r2 + 1, &rc) {
            grown.r2 += 1;
        }
        if rc.c1 > 0 && filled_col(rc.c1 - 1, &rc) {
            grown.c1 -= 1;
        }
        if rc.c2 + 1 < MAX_COLS && filled_col(rc.c2 + 1, &rc) {
            grown.c2 += 1;
        }
        if grown == rc {
            return rc;
        }
        rc = grown;
    }
}

/// Deletes rows that repeat earlier rows in the chosen columns. Returns the count removed.
pub fn remove_duplicates(wb: &mut Workbook, calc: &CalcResult, si: usize, rc: Rect, cols: &[u32]) -> usize {
    let mut seen: HashSet<Vec<String>> = HashSet::new();
    let mut keep = vec![];
    for r in rc.r1..=rc.r2 {
        let key: Vec<String> = cols.iter().map(|c| text_of(wb, calc, si, r, *c).to_lowercase()).collect();
        if seen.insert(key) {
            keep.push(r);
        }
    }
    let removed = rc.rows() as usize - keep.len();
    if removed == 0 {
        return 0;
    }
    let sh = &wb.sheets[si];
    let rows: Vec<Vec<Option<Cell>>> = keep.iter().map(|r| (rc.c1..=rc.c2).map(|c| sh.get(*r, c).cloned()).collect()).collect();
    clear_rect(wb, si, rc);
    let sh = wb.sheet_mut(si);
    for (i, row) in rows.into_iter().enumerate() {
        for (j, cell) in row.into_iter().enumerate() {
            if let Some(x) = cell {
                *sh.cell_mut(rc.r1 + i as u32, rc.c1 + j as u32) = x;
            }
        }
    }
    removed
}

pub fn text_to_columns(wb: &mut Workbook, si: usize, rc: Rect, delims: &[char], merge_runs: bool) {
    for r in rc.r1..=rc.r2 {
        let txt = match wb.sheets[si].get(r, rc.c1).map(|x| x.v.clone()) {
            Some(Content::Text(t)) => t.to_string(),
            _ => continue,
        };
        let mut parts: Vec<&str> = txt.split(|c| delims.contains(&c)).collect();
        if merge_runs {
            parts.retain(|p| !p.is_empty());
        }
        for (j, p) in parts.iter().enumerate() {
            let t = numfmt::parse_input(p);
            wb.sheet_mut(si).set_content(r, rc.c1 + j as u32, t.content);
        }
    }
}

const LISTS: [&[&str]; 4] = [
    &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    &["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    &["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
];

fn list_next(t: &str, step: i64) -> Option<String> {
    for l in LISTS {
        if let Some(i) = l.iter().position(|x| x.eq_ignore_ascii_case(t)) {
            let n = (i as i64 + step).rem_euclid(l.len() as i64) as usize;
            let x = l[n];
            return Some(if t.chars().all(|c| c.is_uppercase()) {
                x.to_uppercase()
            } else if t.chars().all(|c| c.is_lowercase()) {
                x.to_lowercase()
            } else {
                x.to_string()
            });
        }
    }
    None
}

fn split_trailing_num(t: &str) -> Option<(&str, i64, usize)> {
    let i = t.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |x| x + t[x..].chars().next().unwrap().len_utf8());
    if i >= t.len() {
        return None;
    }
    t[i..].parse::<i64>().ok().map(|n| (&t[..i], n, t.len() - i))
}

/// Produces the k-th value (k = 1 after the end of the seed) for one line of seed cells.
fn series_value(seed: &[Content], is_date: bool, k: i64, plain_copy: bool) -> Content {
    let n = seed.len() as i64;
    let nums: Option<Vec<f64>> = seed.iter().map(|c| if let Content::Num(x) = c { Some(*x) } else { None }).collect();
    if !plain_copy {
        if let Some(v) = &nums {
            if v.len() >= 2 {
                // Least-squares line through the seed, as drag-fill does.
                let xs: Vec<f64> = (0..v.len()).map(|i| i as f64).collect();
                if let Ok((b, a)) = crate::funcs_stat::linfit(&xs, v) {
                    return Content::Num(a + b * (n - 1 + k) as f64);
                }
            } else if is_date {
                return Content::Num(v[0] + k as f64);
            }
        }
        if n == 1 || seed.windows(2).all(|w| matches!(w, [Content::Text(_), Content::Text(_)])) {
            if let Content::Text(t) = &seed[seed.len() - 1] {
                if let Some(x) = list_next(t, k) {
                    return Content::Text(x.into());
                }
                let first = if let Content::Text(a) = &seed[0] { split_trailing_num(a) } else { None };
                if let (Some((p, last, w)), Some((p0, f, _))) = (split_trailing_num(t), first) {
                    if p == p0 {
                        let step = if n > 1 { (last - f) / (n - 1) } else { 1 };
                        return Content::Text(format!("{p}{:0w$}", last + step * k, w = w).into());
                    }
                }
            }
        }
    }
    seed[((n - 1 + k) % n) as usize].clone()
}

/// Fills `target` from the `src` block, in whichever direction target extends src.
pub fn autofill(wb: &mut Workbook, si: usize, src: Rect, target: Rect, copy_only: bool) {
    let down = target.r2 > src.r2 || target.r1 < src.r1;
    let forward = target.r2 > src.r2 || target.c2 > src.c2;
    let (lines, len): (u32, u32) = if down { (src.cols(), src.rows()) } else { (src.rows(), src.cols()) };
    let span = if down { target.rows() - src.rows() } else { target.cols() - src.cols() };
    for l in 0..lines {
        let at = |i: u32| -> (u32, u32) {
            if down {
                (src.r1 + i, src.c1 + l)
            } else {
                (src.r1 + l, src.c1 + i)
            }
        };
        let cells: Vec<Cell> = (0..len)
            .map(|i| {
                let (r, c) = at(i);
                wb.sheets[si].get(r, c).cloned().unwrap_or(Cell { v: Content::Empty, style: wb.sheets[si].base_style(r, c) })
            })
            .collect();
        let seed: Vec<Content> = cells.iter().map(|c| c.v.clone()).collect();
        let is_date = numfmt::is_date_format(&wb.styles.get(cells[0].style).num_fmt);
        let any_formula = seed.iter().any(|c| matches!(c, Content::Formula(_)));
        for k in 1..=span {
            let (r, c) = if forward {
                if down {
                    (src.r2 + k, src.c1 + l)
                } else {
                    (src.r1 + l, src.c2 + k)
                }
            } else if down {
                (src.r1 - k, src.c1 + l)
            } else {
                (src.r1 + l, src.c1 - k)
            };
            // The seed cell whose pattern this target takes.
            let si_idx = if forward { ((k - 1) % len) as usize } else { (len - 1 - (k - 1) % len) as usize };
            let origin = &cells[si_idx];
            let v = if any_formula || copy_only {
                match &origin.v {
                    Content::Formula(e) => {
                        let (or, oc) = at(si_idx as u32);
                        Content::Formula(Arc::new(formula::shifted(e, r as i64 - or as i64, c as i64 - oc as i64)))
                    }
                    x => x.clone(),
                }
            } else if forward {
                series_value(&seed, is_date, k as i64, false)
            } else {
                let rev: Vec<Content> = seed.iter().rev().cloned().collect();
                series_value(&rev, is_date, k as i64, false)
            };
            let sh = wb.sheet_mut(si);
            let cell = sh.cell_mut(r, c);
            cell.v = v;
            cell.style = origin.style;
        }
    }
}

/// One pivot layout written to a sheet. Returns the area used.
pub fn build_pivot(wb: &mut Workbook, calc: &CalcResult, def: &PivotDef) -> Option<Rect> {
    let src_si = wb.sheet_index(&def.source_sheet)?;
    let dst_si = wb.sheets.iter().position(|s| s.pivots.iter().any(|p| p.target == def.target && p.source == def.source))?;
    let src = def.source;
    let head: Vec<String> = (src.c1..=src.c2).map(|c| text_of(wb, calc, src_si, src.r1, c)).collect();
    let mut groups: BTreeMap<(Vec<Key2>, Vec<Key2>), Vec<Vec<f64>>> = BTreeMap::new();
    let mut counts: BTreeMap<(Vec<Key2>, Vec<Key2>), Vec<usize>> = BTreeMap::new();
    let (mut row_keys, mut col_keys): (BTreeMap<Vec<Key2>, ()>, BTreeMap<Vec<Key2>, ()>) = Default::default();
    for r in src.r1 + 1..=src.r2 {
        if def.filters.iter().any(|(c, want)| match want {
            Some(w) => text_of(wb, calc, src_si, r, src.c1 + c) != *w,
            None => false,
        }) {
            continue;
        }
        let key = |cols: &[u32]| -> Vec<Key2> {
            cols.iter().map(|c| Key2::of(&calc.value(wb, src_si, r, src.c1 + c), text_of(wb, calc, src_si, r, src.c1 + c))).collect()
        };
        let (rk, ck) = (key(&def.rows), key(&def.cols));
        row_keys.insert(rk.clone(), ());
        col_keys.insert(ck.clone(), ());
        let e = groups.entry((rk.clone(), ck.clone())).or_insert_with(|| vec![vec![]; def.values.len()]);
        let ce = counts.entry((rk, ck)).or_insert_with(|| vec![0; def.values.len()]);
        for (i, (c, _)) in def.values.iter().enumerate() {
            let v = calc.value(wb, src_si, r, src.c1 + c);
            if v != Value::Empty {
                ce[i] += 1;
            }
            if let Value::Num(n) = v {
                e[i].push(n);
            }
        }
    }
    let aggregate = |vals: &[f64], cnt: usize, how: &str| -> Value {
        match how {
            "Count" => Value::Num(cnt as f64),
            "Average" => {
                if vals.is_empty() {
                    Value::Err(XlErr::Div0)
                } else {
                    Value::Num(vals.iter().sum::<f64>() / vals.len() as f64)
                }
            }
            "Max" => Value::Num(vals.iter().cloned().fold(f64::MIN, f64::max)),
            "Min" => Value::Num(vals.iter().cloned().fold(f64::MAX, f64::min)),
            "Product" => Value::Num(vals.iter().product()),
            _ => Value::Num(vals.iter().sum()),
        }
    };
    let rows: Vec<Vec<Key2>> = row_keys.keys().cloned().collect();
    let cols: Vec<Vec<Key2>> = col_keys.keys().cloned().collect();
    let (t0, c0) = (def.target.row, def.target.col);
    let nr = def.rows.len().max(1) as u32;
    let nv = def.values.len().max(1) as u32;
    let body_cols = cols.len() as u32 * nv + if cols.len() > 1 { nv } else { 0 };
    let head_rows: u32 = if !def.cols.is_empty() { 2 } else { 1 };
    let area = Rect::new(t0, c0, t0 + head_rows + rows.len() as u32, c0 + nr + body_cols - 1);
    clear_rect(wb, dst_si, area.union(&Rect::new(t0, c0, t0 + 2000, c0 + 60)));
    let bold = wb.styles.intern(Style { bold: true, fill: Some(0xDDEBF7), ..Style::default() });
    let total = wb.styles.intern(Style {
        bold: true,
        fill: Some(0xDDEBF7),
        borders: [None, None, Some(Border { style: LineStyle::Thin, color: 0x5B9BD5 }), None],
        ..Style::default()
    });
    let plain_num = wb.styles.intern(Style { num_fmt: "#,##0.00".into(), ..Style::default() });
    let put = |wb: &mut Workbook, r: u32, c: u32, v: Value, st: u32| {
        let sh = wb.sheet_mut(dst_si);
        let content = match v {
            Value::Num(n) => Content::Num(n),
            Value::Text(t) => Content::Text(t),
            Value::Err(e) => Content::Err(e),
            _ => Content::Empty,
        };
        let cell = sh.cell_mut(r, c);
        cell.v = content;
        cell.style = st;
    };
    // Header rows.
    let hr = t0 + head_rows - 1;
    for (i, c) in def.rows.iter().enumerate() {
        put(wb, hr, c0 + i as u32, Value::text(head.get(*c as usize).cloned().unwrap_or_default()), bold);
    }
    if def.rows.is_empty() {
        put(wb, hr, c0, Value::text("Total"), bold);
    }
    let vname = |i: usize| -> String {
        let (c, how) = &def.values[i];
        format!("{how} of {}", head.get(*c as usize).cloned().unwrap_or_default())
    };
    if head_rows == 2 {
        for c in c0..c0 + nr + body_cols {
            put(wb, t0, c, Value::Empty, bold);
        }
        put(wb, t0, c0 + nr, Value::text("Column Labels"), bold);
    }
    let mut col = c0 + nr;
    for k in &cols {
        for i in 0..nv as usize {
            let label = if def.cols.is_empty() {
                vname(i)
            } else if nv > 1 {
                format!("{} - {}", k.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join(" "), vname(i))
            } else {
                k.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join(" ")
            };
            put(wb, hr, col, Value::text(label), bold);
            col += 1;
        }
    }
    if cols.len() > 1 {
        for i in 0..nv as usize {
            put(wb, hr, col, Value::text(if nv > 1 { format!("Total {}", vname(i)) } else { "Grand Total".into() }), bold);
            col += 1;
        }
    }
    let mut r = hr + 1;
    for rk in &rows {
        for (i, k) in rk.iter().enumerate() {
            put(wb, r, c0 + i as u32, Value::text(&k.text), 0);
        }
        let mut col = c0 + nr;
        let mut across: Vec<(Vec<f64>, usize)> = vec![(vec![], 0); nv as usize];
        for ck in &cols {
            let key = (rk.clone(), ck.clone());
            for i in 0..nv as usize {
                if let (Some(g), Some(cn)) = (groups.get(&key), counts.get(&key)) {
                    put(wb, r, col, aggregate(&g[i], cn[i], &def.values[i].1), plain_num);
                    across[i].0.extend(&g[i]);
                    across[i].1 += cn[i];
                }
                col += 1;
            }
        }
        if cols.len() > 1 {
            for i in 0..nv as usize {
                put(wb, r, col, aggregate(&across[i].0, across[i].1, &def.values[i].1), plain_num);
                col += 1;
            }
        }
        r += 1;
    }
    // Grand total row.
    if !def.rows.is_empty() {
        for c in c0..c0 + nr {
            put(wb, r, c, Value::Empty, total);
        }
        put(wb, r, c0, Value::text("Grand Total"), total);
        let mut col = c0 + nr;
        let mut all: Vec<(Vec<f64>, usize)> = vec![(vec![], 0); nv as usize];
        for ck in &cols {
            for i in 0..nv as usize {
                let (mut v, mut n) = (vec![], 0);
                for rk in &rows {
                    let key = (rk.clone(), ck.clone());
                    if let (Some(g), Some(cn)) = (groups.get(&key), counts.get(&key)) {
                        v.extend(&g[i]);
                        n += cn[i];
                    }
                }
                all[i].0.extend(&v);
                all[i].1 += n;
                put(wb, r, col, aggregate(&v, n, &def.values[i].1), total);
                col += 1;
            }
        }
        if cols.len() > 1 {
            for i in 0..nv as usize {
                put(wb, r, col, aggregate(&all[i].0, all[i].1, &def.values[i].1), total);
                col += 1;
            }
        }
    }
    let sh = wb.sheet_mut(dst_si);
    for c in c0..c0 + nr + body_cols {
        sh.col_w.entry(c).or_insert(96.0);
    }
    Some(Rect::new(t0, c0, r, c0 + nr + body_cols - 1))
}

/// A pivot grouping key that sorts numbers numerically and text alphabetically.
#[derive(Clone, Debug, PartialEq)]
pub struct Key2 {
    num: Option<f64>,
    text: String,
}

impl Key2 {
    fn of(v: &Value, shown: String) -> Key2 {
        let text = if shown.is_empty() { "(blank)".to_string() } else { shown };
        Key2 { num: if let Value::Num(n) = v { Some(*n) } else { None }, text }
    }
}
impl Eq for Key2 {}
impl PartialOrd for Key2 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key2 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self.num, o.num) {
            (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Equal),
            (Some(_), None) => Less,
            (None, Some(_)) => Greater,
            _ => self.text.to_lowercase().cmp(&o.text.to_lowercase()),
        }
    }
}

/// Goal Seek: changes one cell until another reaches a target. Returns the found value.
pub fn goal_seek(wb: &mut Workbook, si: usize, set: Pos, target: f64, change: Pos) -> Option<f64> {
    let base = match wb.sheets[si].get(change.row, change.col).map(|x| x.v.clone()) {
        Some(Content::Num(n)) => n,
        _ => 0.0,
    };
    let f = |wb: &mut Workbook, x: f64| -> Option<f64> {
        wb.sheet_mut(si).set_content(change.row, change.col, Content::Num(x));
        match recalc(wb).value(wb, si, set.row, set.col) {
            Value::Num(n) => Some(n - target),
            _ => None,
        }
    };
    let (mut x0, mut x1) = (base, if base == 0.0 { 1.0 } else { base * 1.1 });
    let (mut y0, mut y1) = (f(wb, x0)?, f(wb, x1)?);
    for _ in 0..200 {
        if y1.abs() < 1e-7 {
            return Some(x1);
        }
        let d = y1 - y0;
        if d == 0.0 {
            x1 += 0.5 + x1.abs() * 0.1;
            y1 = f(wb, x1)?;
            continue;
        }
        let x2 = x1 - y1 * (x1 - x0) / d;
        x0 = x1;
        y0 = y1;
        x1 = x2;
        y1 = f(wb, x1)?;
    }
    None
}
