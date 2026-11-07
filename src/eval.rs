//! The calculation engine: evaluates expression trees against a workbook.

use crate::formula::{self, BinOp, Expr, RefExpr};
use crate::funcs::{registry, FnDef};
use crate::model::*;
use crate::numfmt;
use std::cell::{Cell as StdCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Value>,
}

impl Array {
    pub fn get(&self, r: usize, c: usize) -> &Value {
        &self.data[r * self.cols + c]
    }
    pub fn new(rows: usize, cols: usize, data: Vec<Value>) -> Value {
        if rows == 0 || cols == 0 {
            return Value::Err(XlErr::Calc);
        }
        Value::Array(Arc::new(Array { rows, cols, data }))
    }
    pub fn row_of(&self, r: usize) -> Vec<Value> {
        (0..self.cols).map(|c| self.get(r, c).clone()).collect()
    }
    pub fn col_of(&self, c: usize) -> Vec<Value> {
        (0..self.rows).map(|r| self.get(r, c).clone()).collect()
    }
}

#[derive(Debug, PartialEq)]
pub struct LambdaDef {
    pub params: Vec<Arc<str>>,
    pub body: Expr,
    pub scope: Vec<(Arc<str>, Value)>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Empty,
    Num(f64),
    Text(Arc<str>),
    Bool(bool),
    Err(XlErr),
    Array(Arc<Array>),
    Ref(usize, Rect),
    Lambda(Arc<LambdaDef>),
}

pub type R = Result<Value, XlErr>;

impl Value {
    pub fn text(s: impl AsRef<str>) -> Value {
        Value::Text(s.as_ref().into())
    }
    pub fn is_err(&self) -> bool {
        matches!(self, Value::Err(_))
    }
}

/// Text form of a number as used by concatenation: up to 15 significant digits.
pub fn num_text(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    let a = x.abs();
    if !(1e-9..1e21).contains(&a) {
        return numfmt::general(x, 11);
    }
    let digits = a.log10().floor() as i32 + 1;
    let dec = (15 - digits).clamp(0, 20) as usize;
    let mut s = format!("{:.*}", dec, x);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

pub fn today_ymd() -> (i64, i64, i64) {
    use chrono::Datelike;
    let n = chrono::Local::now();
    (n.year() as i64, n.month() as i64, n.day() as i64)
}

pub fn now_serial() -> f64 {
    use chrono::{Datelike, Timelike};
    let n = chrono::Local::now();
    numfmt::ymd_to_serial(n.year() as i64, n.month() as i64, n.day() as i64)
        + (n.hour() as f64 * 3600.0 + n.minute() as f64 * 60.0 + n.second() as f64) / 86400.0
}

pub type Key = (u32, u32, u32);

#[derive(Clone, Debug, Default)]
pub struct CalcResult {
    pub vals: HashMap<Key, Value>,
    pub spill: HashMap<Key, Value>,
    pub spill_rects: HashMap<Key, Rect>,
    pub circular: Option<(usize, Pos)>,
}

impl CalcResult {
    /// The value shown for a cell.
    pub fn value(&self, wb: &Workbook, si: usize, r: u32, c: u32) -> Value {
        let key = (si as u32, r, c);
        if let Some(v) = self.vals.get(&key) {
            return v.clone();
        }
        if let Some(v) = self.spill.get(&key) {
            return v.clone();
        }
        match wb.sheets[si].get(r, c) {
            Some(x) => content_value(&x.v),
            None => Value::Empty,
        }
    }
}

pub fn content_value(c: &Content) -> Value {
    match c {
        Content::Empty | Content::Formula(_) => Value::Empty,
        Content::Num(n) => Value::Num(*n),
        Content::Text(t) => Value::Text(t.clone()),
        Content::Bool(b) => Value::Bool(*b),
        Content::Err(e) => Value::Err(*e),
    }
}

pub struct Ctx<'a> {
    pub wb: &'a Workbook,
    cache: RefCell<HashMap<Key, Value>>,
    visiting: RefCell<HashSet<Key>>,
    arrays: RefCell<HashMap<(usize, Rect), Arc<Array>>>,
    pub spill: HashMap<Key, Value>,
    pub spill_rects: HashMap<Key, Rect>,
    circular: StdCell<Option<(usize, Pos)>>,
    names: HashMap<String, Result<Expr, ()>>,
    bounds: Vec<(u32, u32)>,
    depth: StdCell<u32>,
    /// Finished results to read from instead of recalculating.
    pre: Option<&'a CalcResult>,
}

#[derive(Clone)]
pub struct Env {
    pub sheet: usize,
    pub pos: Pos,
    pub scope: Vec<(Arc<str>, Value)>,
}

/// What a function sees: the engine plus the cell it is being evaluated in.
pub struct Cx<'c, 'a> {
    pub ctx: &'c Ctx<'a>,
    pub sheet: usize,
    pub pos: Pos,
    pub scope: &'c [(Arc<str>, Value)],
}

impl<'c, 'a> Deref for Cx<'c, 'a> {
    type Target = Ctx<'a>;
    fn deref(&self) -> &Ctx<'a> {
        self.ctx
    }
}

impl<'a> Ctx<'a> {
    pub fn new(wb: &'a Workbook) -> Self {
        let mut names = HashMap::new();
        for n in &wb.names {
            names.insert(n.name.to_ascii_uppercase(), formula::parse(&n.expr).map_err(|_| ()));
        }
        let bounds = wb
            .sheets
            .iter()
            .map(|s| {
                let mut mr = 0;
                let mut mc = 0;
                for (&r, m) in &s.rows {
                    mr = mr.max(r);
                    if let Some((&c, _)) = m.iter().next_back() {
                        mc = mc.max(c);
                    }
                }
                (mr, mc)
            })
            .collect();
        Ctx {
            wb,
            cache: RefCell::new(HashMap::new()),
            visiting: RefCell::new(HashSet::new()),
            arrays: RefCell::new(HashMap::new()),
            spill: HashMap::new(),
            spill_rects: HashMap::new(),
            circular: StdCell::new(None),
            names,
            bounds,
            depth: StdCell::new(0),
            pre: None,
        }
    }

    /// An evaluator that reads finished results, for rules drawn on screen.
    pub fn reader(wb: &'a Workbook, res: &'a CalcResult) -> Self {
        let mut c = Ctx::new(wb);
        c.pre = Some(res);
        c.spill_rects = res.spill_rects.clone();
        c
    }

    fn with_spills(wb: &'a Workbook, spill: HashMap<Key, Value>, rects: HashMap<Key, Rect>) -> Self {
        let mut c = Ctx::new(wb);
        for k in spill.keys() {
            let b = &mut c.bounds[k.0 as usize];
            b.0 = b.0.max(k.1);
            b.1 = b.1.max(k.2);
        }
        c.spill = spill;
        c.spill_rects = rects;
        c
    }

    /// Value of a cell, computing and remembering formulas. Formula blanks stay Empty.
    pub fn cell(&self, si: usize, r: u32, c: u32) -> Value {
        if let Some(p) = self.pre {
            if si >= self.wb.sheets.len() {
                return Value::Err(XlErr::Ref);
            }
            return top_scalar(&p.value(self.wb, si, r, c));
        }
        let Some(sh) = self.wb.sheets.get(si) else {
            return Value::Err(XlErr::Ref);
        };
        let Some(cell) = sh.get(r, c) else {
            if !self.spill.is_empty() {
                if let Some(v) = self.spill.get(&(si as u32, r, c)) {
                    return v.clone();
                }
            }
            return Value::Empty;
        };
        match &cell.v {
            Content::Formula(e) => {
                let key = (si as u32, r, c);
                if let Some(v) = self.cache.borrow().get(&key) {
                    return top_scalar(v);
                }
                top_scalar(&self.compute(key, e))
            }
            Content::Empty => {
                if !self.spill.is_empty() {
                    if let Some(v) = self.spill.get(&(si as u32, r, c)) {
                        return v.clone();
                    }
                }
                Value::Empty
            }
            x => content_value(x),
        }
    }

    fn compute(&self, key: Key, e: &Expr) -> Value {
        if !self.visiting.borrow_mut().insert(key) {
            if self.circular.get().is_none() {
                self.circular.set(Some((key.0 as usize, Pos::new(key.1, key.2))));
            }
            return Value::Num(0.0);
        }
        let mut env = Env { sheet: key.0 as usize, pos: Pos::new(key.1, key.2), scope: vec![] };
        let v = stacker::maybe_grow(256 * 1024, 8 * 1024 * 1024, || self.eval(e, &mut env));
        let mut v = self.settle(v);
        if self.circular.get() == Some((key.0 as usize, Pos::new(key.1, key.2))) {
            v = Value::Num(0.0);
        }
        self.visiting.borrow_mut().remove(&key);
        self.cache.borrow_mut().insert(key, v.clone());
        v
    }

    /// Makes a formula's final value: single references read through, multi-cell ones become arrays.
    fn settle(&self, v: Value) -> Value {
        match v {
            Value::Ref(si, rc) => {
                if rc.is_single() {
                    match self.cell(si, rc.r1, rc.c1) {
                        Value::Empty => Value::Num(0.0),
                        x => x,
                    }
                } else {
                    Value::Array(self.array_of(si, rc))
                }
            }
            Value::Empty => Value::Num(0.0),
            Value::Array(a) if a.rows == 1 && a.cols == 1 => match a.data[0].clone() {
                Value::Empty => Value::Num(0.0),
                x => x,
            },
            Value::Lambda(_) => Value::Err(XlErr::Calc),
            x => x,
        }
    }

    pub fn array_of(&self, si: usize, rc: Rect) -> Arc<Array> {
        if let Some(a) = self.arrays.borrow().get(&(si, rc)) {
            return a.clone();
        }
        let (mr, mc) = self.bounds.get(si).copied().unwrap_or((0, 0));
        // Whole rows and columns are clipped to what the sheet uses.
        let r2 = if rc.whole_cols() || rc.rows() > 100_000 { rc.r2.min(mr.max(rc.r1)) } else { rc.r2 };
        let c2 = if rc.whole_rows() { rc.c2.min(mc.max(rc.c1)) } else { rc.c2 };
        let rows = (r2 - rc.r1 + 1) as usize;
        let cols = (c2 - rc.c1 + 1) as usize;
        let mut data = Vec::with_capacity(rows * cols);
        for r in rc.r1..=r2 {
            for c in rc.c1..=c2 {
                data.push(self.cell(si, r, c));
            }
        }
        let a = Arc::new(Array { rows, cols, data });
        self.arrays.borrow_mut().insert((si, rc), a.clone());
        a
    }

    /// Visits every non-empty value in a rectangle.
    pub fn each_in(&self, si: usize, rc: Rect, f: &mut dyn FnMut(u32, u32, Value)) {
        let Some(sh) = self.wb.sheets.get(si) else { return };
        for (&r, m) in sh.rows.range(rc.r1..=rc.r2) {
            for (&c, cell) in m.range(rc.c1..=rc.c2) {
                if cell.v != Content::Empty {
                    f(r, c, self.cell(si, r, c));
                }
            }
        }
        let spill = self.pre.map_or(&self.spill, |p| &p.spill);
        if !spill.is_empty() {
            for (k, v) in spill {
                if k.0 as usize == si && rc.contains(Pos::new(k.1, k.2)) {
                    f(k.1, k.2, v.clone());
                }
            }
        }
    }

    /// Reduces a value to a single scalar: reads references through and picks array corners.
    pub fn scalar(&self, v: Value, env_pos: Option<(usize, Pos)>) -> Value {
        match v {
            Value::Ref(si, rc) => {
                if rc.is_single() {
                    self.cell(si, rc.r1, rc.c1)
                } else if let Some((_, p)) = env_pos {
                    // Implicit intersection with the formula's own row or column.
                    if rc.cols() == 1 && p.row >= rc.r1 && p.row <= rc.r2 {
                        self.cell(si, p.row, rc.c1)
                    } else if rc.rows() == 1 && p.col >= rc.c1 && p.col <= rc.c2 {
                        self.cell(si, rc.r1, p.col)
                    } else {
                        Value::Err(XlErr::Value)
                    }
                } else {
                    self.cell(si, rc.r1, rc.c1)
                }
            }
            Value::Array(a) => a.data.first().cloned().unwrap_or(Value::Empty),
            x => x,
        }
    }

    fn sheet_of(&self, r: &RefExpr, env: &Env) -> Option<usize> {
        match &r.sheet {
            Some(s) => self.wb.sheet_index(s),
            None => Some(env.sheet),
        }
    }

    pub fn eval(&self, e: &Expr, env: &mut Env) -> Value {
        match e {
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Text(s.clone()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Err(x) => Value::Err(*x),
            Expr::Missing => Value::Empty,
            Expr::Ref(r) => match self.sheet_of(r, env) {
                Some(si) => Value::Ref(si, r.rect()),
                None => Value::Err(XlErr::Ref),
            },
            Expr::Name(n) => self.eval_name(n, env),
            Expr::Paren(x) => self.eval(x, env),
            Expr::Neg(x) => {
                let v = self.eval(x, env);
                self.unary(v, |a| -a)
            }
            Expr::Plus(x) => self.eval(x, env),
            Expr::Percent(x) => {
                let v = self.eval(x, env);
                self.unary(v, |a| a / 100.0)
            }
            Expr::Bin(op, a, b) => {
                let l = self.eval(a, env);
                let r = self.eval(b, env);
                self.binary(*op, l, r)
            }
            Expr::Array(rows) => {
                let nr = rows.len();
                let nc = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                let mut data = Vec::with_capacity(nr * nc);
                for r in rows {
                    for i in 0..nc {
                        data.push(match r.get(i) {
                            Some(x) => {
                                let v = self.eval(x, env);
                                self.scalar(v, None)
                            }
                            None => Value::Err(XlErr::NA),
                        });
                    }
                }
                Array::new(nr, nc, data)
            }
            Expr::Spill(x) => {
                let v = self.eval(x, env);
                if let Value::Ref(si, rc) = v {
                    let key = (si as u32, rc.r1, rc.c1);
                    if let Some(s) = self.spill_rects.get(&key) {
                        return Value::Ref(si, *s);
                    }
                    if let Some(c) = self.wb.sheets[si].get(rc.r1, rc.c1) {
                        if let Content::Formula(f) = &c.v {
                            if let Some(c0) = self.cache.borrow().get(&key).cloned() {
                                return c0;
                            }
                            return self.compute(key, f);
                        }
                    }
                    return Value::Err(XlErr::Ref);
                }
                v
            }
            Expr::Implicit(x) => {
                let v = self.eval(x, env);
                self.scalar(v, Some((env.sheet, env.pos)))
            }
            Expr::Ref3D(a, b, r) => {
                let (Some(i), Some(j)) = (self.wb.sheet_index(a), self.wb.sheet_index(b)) else {
                    return Value::Err(XlErr::Ref);
                };
                let rc = r.rect();
                let mut data = vec![];
                let mut rows = 0;
                let mut cols = 0;
                for si in i.min(j)..=i.max(j) {
                    let arr = self.array_of(si, rc);
                    rows += arr.rows;
                    cols = arr.cols;
                    data.extend(arr.data.iter().cloned());
                }
                Array::new(rows, cols, data)
            }
            Expr::Struct(t, spec) => self.eval_struct(t, spec, env),
            Expr::RangeOp(a, b) => {
                let l = self.eval(a, env);
                let r = self.eval(b, env);
                match (l, r) {
                    (Value::Ref(s1, x), Value::Ref(s2, y)) if s1 == s2 => Value::Ref(s1, x.union(&y)),
                    (Value::Err(e), _) | (_, Value::Err(e)) => Value::Err(e),
                    _ => Value::Err(XlErr::Value),
                }
            }
            Expr::Call(name, args) => self.call(name, args, env),
        }
    }

    fn eval_name(&self, n: &Arc<str>, env: &mut Env) -> Value {
        for (k, v) in env.scope.iter().rev() {
            if k.eq_ignore_ascii_case(n) {
                return v.clone();
            }
        }
        match self.names.get(n.as_ref()) {
            Some(Ok(e)) => {
                let mut e2 = Env { sheet: env.sheet, pos: env.pos, scope: vec![] };
                self.eval(e, &mut e2)
            }
            Some(Err(_)) => Value::Err(XlErr::Name),
            None => Value::Err(XlErr::Name),
        }
    }

    fn eval_struct(&self, table: &str, spec: &str, env: &Env) -> Value {
        // Find the table: by name, or the table holding the formula for `[@Col]`.
        let mut found: Option<(usize, &TableDef)> = None;
        for (si, sh) in self.wb.sheets.iter().enumerate() {
            for t in &sh.tables {
                let hit = if table.is_empty() {
                    si == env.sheet && t.range.contains(env.pos)
                } else {
                    t.name.eq_ignore_ascii_case(table)
                };
                if hit {
                    found = Some((si, t));
                }
            }
        }
        let Some((si, t)) = found else {
            return Value::Err(XlErr::Name);
        };
        let hdr = t.header as u32;
        let data = Rect::new(t.range.r1 + hdr, t.range.c1, t.range.r2 - t.totals as u32, t.range.c2);
        let spec = spec.trim();
        let parts: Vec<String> = spec
            .split("],")
            .map(|p| p.trim().trim_start_matches('[').trim_end_matches(']').trim().to_string())
            .collect();
        let mut rc = data;
        let mut col: Option<u32> = None;
        let mut col2: Option<u32> = None;
        let mut this_row = false;
        for p in &parts {
            let low = p.to_ascii_lowercase();
            match low.as_str() {
                "#all" => rc = t.range,
                "#data" => rc = data,
                "#headers" => rc = Rect::new(t.range.r1, t.range.c1, t.range.r1, t.range.c2),
                "#totals" => rc = Rect::new(t.range.r2, t.range.c1, t.range.r2, t.range.c2),
                "#this row" => this_row = true,
                "" => {}
                _ => {
                    let (name, tr) = match p.strip_prefix('@') {
                        Some(x) => (x.trim_start_matches('[').trim_end_matches(']'), true),
                        None => (p.as_str(), false),
                    };
                    this_row |= tr;
                    if let Some((a, b)) = name.split_once("]:[") {
                        col = t.columns.iter().position(|c| c.eq_ignore_ascii_case(a)).map(|x| x as u32);
                        col2 = t.columns.iter().position(|c| c.eq_ignore_ascii_case(b)).map(|x| x as u32);
                    } else {
                        match t.columns.iter().position(|c| c.eq_ignore_ascii_case(name)) {
                            Some(i) => col = Some(i as u32),
                            None => return Value::Err(XlErr::Ref),
                        }
                    }
                }
            }
        }
        if let Some(c) = col {
            let c2 = col2.unwrap_or(c);
            rc.c1 = t.range.c1 + c;
            rc.c2 = t.range.c1 + c2;
        }
        if this_row {
            rc.r1 = env.pos.row;
            rc.r2 = env.pos.row;
        }
        Value::Ref(si, rc)
    }

    pub fn arrayish(&self, v: Value) -> Value {
        match v {
            Value::Ref(si, rc) => {
                if rc.is_single() {
                    self.cell(si, rc.r1, rc.c1)
                } else {
                    Value::Array(self.array_of(si, rc))
                }
            }
            x => x,
        }
    }

    fn unary(&self, v: Value, f: impl Fn(f64) -> f64 + Copy) -> Value {
        match self.arrayish(v) {
            Value::Array(a) => {
                let data = a
                    .data
                    .iter()
                    .map(|x| match to_num(x) {
                        Ok(n) => Value::Num(f(n)),
                        Err(e) => Value::Err(e),
                    })
                    .collect();
                Array::new(a.rows, a.cols, data)
            }
            x => match to_num(&x) {
                Ok(n) => Value::Num(f(n)),
                Err(e) => Value::Err(e),
            },
        }
    }

    pub fn binary(&self, op: BinOp, l: Value, r: Value) -> Value {
        let l = self.arrayish(l);
        let r = self.arrayish(r);
        match (&l, &r) {
            (Value::Array(_), _) | (_, Value::Array(_)) => broadcast2(&l, &r, |a, b| scalar_bin(op, a, b)),
            _ => scalar_bin(op, &l, &r),
        }
    }

    fn call(&self, name: &Arc<str>, args: &[Expr], env: &mut Env) -> Value {
        match name.as_ref() {
            "IF" => return self.f_if(args, env),
            "IFERROR" | "IFNA" => {
                if args.len() != 2 {
                    return Value::Err(XlErr::Value);
                }
                let v = self.eval(&args[0], env);
                let v = self.arrayish(v);
                let na = name.as_ref() == "IFNA";
                let hit = |x: &Value| if na { *x == Value::Err(XlErr::NA) } else { x.is_err() };
                return match v {
                    Value::Array(a) => {
                        let alt = self.eval(&args[1], env);
                        let alt = self.arrayish(alt);
                        let data = a
                            .data
                            .iter()
                            .enumerate()
                            .map(|(i, x)| {
                                if hit(x) {
                                    match &alt {
                                        Value::Array(b) if b.data.len() == a.data.len() => b.data[i].clone(),
                                        Value::Array(b) => b.data[0].clone(),
                                        o => o.clone(),
                                    }
                                } else {
                                    x.clone()
                                }
                            })
                            .collect();
                        Array::new(a.rows, a.cols, data)
                    }
                    x if hit(&x) => self.eval(&args[1], env),
                    x => x,
                };
            }
            "IFS" => {
                let mut i = 0;
                while i + 1 < args.len() {
                    let c = self.eval(&args[i], env);
                    let c = self.scalar(c, Some((env.sheet, env.pos)));
                    match to_bool(&c) {
                        Ok(true) => return self.eval(&args[i + 1], env),
                        Ok(false) => {}
                        Err(e) => return Value::Err(e),
                    }
                    i += 2;
                }
                return Value::Err(XlErr::NA);
            }
            "SWITCH" => {
                if args.len() < 3 {
                    return Value::Err(XlErr::Value);
                }
                let v = self.eval(&args[0], env);
                let v = self.scalar(v, Some((env.sheet, env.pos)));
                if v.is_err() {
                    return v;
                }
                let mut i = 1;
                while i + 1 < args.len() {
                    let c = self.eval(&args[i], env);
                    let c = self.scalar(c, None);
                    if compare(&v, &c) == Some(std::cmp::Ordering::Equal) {
                        return self.eval(&args[i + 1], env);
                    }
                    i += 2;
                }
                if i < args.len() {
                    return self.eval(&args[i], env);
                }
                return Value::Err(XlErr::NA);
            }
            "CHOOSE" => {
                if args.len() < 2 {
                    return Value::Err(XlErr::Value);
                }
                let v = self.eval(&args[0], env);
                if let Value::Array(ix) = self.arrayish(v.clone()) {
                    if ix.data.len() > 1 {
                        let opts: Vec<Value> = args[1..].iter().map(|a| { let x = self.eval(a, env); self.arrayish(x) }).collect();
                        let dims = |x: &Value| match x { Value::Array(a) => (a.rows, a.cols), _ => (1, 1) };
                        let (mut rows, mut cols) = (ix.rows, ix.cols);
                        for o in &opts {
                            let (r, c) = dims(o);
                            rows = rows.max(r);
                            cols = cols.max(c);
                        }
                        let pick = |x: &Value, r: usize, c: usize| match x {
                            Value::Array(a) => {
                                let rr = if a.rows == 1 { 0 } else { r };
                                let cc = if a.cols == 1 { 0 } else { c };
                                if rr < a.rows && cc < a.cols { a.get(rr, cc).clone() } else { Value::Err(XlErr::NA) }
                            }
                            other => other.clone(),
                        };
                        let mut data = Vec::with_capacity(rows * cols);
                        for r in 0..rows {
                            for c in 0..cols {
                                let k = pick(&Value::Array(ix.clone()), r, c);
                                data.push(match to_num(&k) {
                                    Ok(n) if n >= 1.0 && (n as usize) <= opts.len() => pick(&opts[n as usize - 1], r, c),
                                    Ok(_) => Value::Err(XlErr::Value),
                                    Err(e) => Value::Err(e),
                                });
                            }
                        }
                        return Array::new(rows, cols, data);
                    }
                }
                let v = self.scalar(v, Some((env.sheet, env.pos)));
                return match to_num(&v) {
                    Ok(n) => {
                        let i = n.trunc() as usize;
                        if i < 1 || i >= args.len() {
                            Value::Err(XlErr::Value)
                        } else {
                            self.eval(&args[i], env)
                        }
                    }
                    Err(e) => Value::Err(e),
                };
            }
            "LET" => {
                if args.len() < 3 || args.len() % 2 == 0 {
                    return Value::Err(XlErr::Value);
                }
                let n0 = env.scope.len();
                let mut i = 0;
                while i + 1 < args.len() {
                    let Expr::Name(n) = &args[i] else {
                        env.scope.truncate(n0);
                        return Value::Err(XlErr::Value);
                    };
                    let v = self.eval(&args[i + 1], env);
                    env.scope.push((n.clone(), v));
                    i += 2;
                }
                let v = self.eval(&args[args.len() - 1], env);
                env.scope.truncate(n0);
                return v;
            }
            "LAMBDA" => {
                let Some((body, ps)) = args.split_last() else {
                    return Value::Err(XlErr::Value);
                };
                let mut params = vec![];
                for p in ps {
                    if let Expr::Name(n) = p {
                        params.push(n.clone());
                    } else {
                        return Value::Err(XlErr::Value);
                    }
                }
                return Value::Lambda(Arc::new(LambdaDef { params, body: body.clone(), scope: env.scope.clone() }));
            }
            _ => {}
        }
        // A LET variable or a defined name holding a LAMBDA.
        let is_local = env.scope.iter().any(|(k, _)| k.eq_ignore_ascii_case(name));
        if is_local || self.names.contains_key(name.as_ref()) {
            let f = self.eval_name(name, env);
            if let Value::Lambda(l) = f {
                let vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();
                return self.apply(&l, vals, env);
            }
        }
        let Some(def) = registry().map.get(name.as_ref()) else {
            return Value::Err(XlErr::Name);
        };
        if args.len() < def.min as usize || args.len() > def.max as usize {
            return Value::Err(XlErr::Value);
        }
        let vals: Vec<Value> = args.iter().map(|a| self.eval(a, env)).collect();
        self.invoke(def, vals, env)
    }

    pub fn apply(&self, l: &LambdaDef, vals: Vec<Value>, env: &Env) -> Value {
        if vals.len() != l.params.len() {
            return Value::Err(XlErr::Value);
        }
        let mut scope = l.scope.clone();
        for (p, v) in l.params.iter().zip(vals) {
            scope.push((p.clone(), v));
        }
        let d = self.depth.get();
        if d > 2000 {
            return Value::Err(XlErr::Num);
        }
        self.depth.set(d + 1);
        let mut e = Env { sheet: env.sheet, pos: env.pos, scope };
        let v = stacker::maybe_grow(256 * 1024, 8 * 1024 * 1024, || self.eval(&l.body, &mut e));
        self.depth.set(d);
        v
    }

    fn invoke(&self, def: &FnDef, vals: Vec<Value>, env: &Env) -> Value {
        let cx = Cx { ctx: self, sheet: env.sheet, pos: env.pos, scope: &env.scope };
        if def.lift {
            let mut vals = vals;
            let mut any = false;
            let mut rows = 1;
            let mut cols = 1;
            for v in vals.iter_mut() {
                if let Value::Ref(..) = v {
                    *v = self.arrayish(std::mem::take(v));
                }
                if let Value::Array(a) = v {
                    if a.rows == 1 && a.cols == 1 {
                        let x = a.data[0].clone();
                        *v = x;
                    } else {
                        any = true;
                        rows = rows.max(a.rows);
                        cols = cols.max(a.cols);
                    }
                }
            }
            if any {
                let mut data = Vec::with_capacity(rows * cols);
                for r in 0..rows {
                    for c in 0..cols {
                        let one: Vec<Value> = vals.iter().map(|v| pick(v, r, c)).collect();
                        data.push(match (def.f)(&cx, &one) {
                            Ok(x) => x,
                            Err(e) => Value::Err(e),
                        });
                    }
                }
                return Array::new(rows, cols, data);
            }
            return (def.f)(&cx, &vals).unwrap_or_else(Value::Err);
        }
        (def.f)(&cx, &vals).unwrap_or_else(Value::Err)
    }

    fn f_if(&self, args: &[Expr], env: &mut Env) -> Value {
        if args.is_empty() || args.len() > 3 {
            return Value::Err(XlErr::Value);
        }
        let c = self.eval(&args[0], env);
        let c = self.arrayish(c);
        if let Value::Array(a) = &c {
            let t = args.get(1).map(|x| self.eval(x, env)).map(|x| self.arrayish(x)).unwrap_or(Value::Bool(true));
            let f = args.get(2).map(|x| self.eval(x, env)).map(|x| self.arrayish(x)).unwrap_or(Value::Bool(false));
            let mut data = Vec::with_capacity(a.data.len());
            for r in 0..a.rows {
                for col in 0..a.cols {
                    let x = a.get(r, col);
                    data.push(match to_bool(x) {
                        Ok(true) => pick(&t, r, col),
                        Ok(false) => pick(&f, r, col),
                        Err(e) => Value::Err(e),
                    });
                }
            }
            return Array::new(a.rows, a.cols, data);
        }
        match to_bool(&c) {
            Ok(true) => match args.get(1) {
                Some(Expr::Missing) => Value::Num(0.0),
                Some(x) => self.eval(x, env),
                None => Value::Bool(true),
            },
            Ok(false) => match args.get(2) {
                Some(Expr::Missing) => Value::Num(0.0),
                Some(x) => self.eval(x, env),
                None => Value::Bool(false),
            },
            Err(e) => Value::Err(e),
        }
    }

    pub fn into_result(self) -> CalcResult {
        CalcResult {
            vals: self.cache.into_inner(),
            spill: self.spill,
            spill_rects: self.spill_rects,
            circular: self.circular.get(),
        }
    }

    /// Evaluates formula text and keeps ranges whole, for chart series and sparklines.
    pub fn eval_range_text(&self, src: &str, si: usize) -> Value {
        match formula::parse(src) {
            Ok(e) => {
                let mut env = Env { sheet: si, pos: Pos::new(0, 0), scope: vec![] };
                let v = self.eval(&e, &mut env);
                self.arrayish(v)
            }
            Err(_) => Value::Err(XlErr::Name),
        }
    }

    /// Evaluates formula text at a position, for conditional formats, validation and Goal Seek.
    pub fn eval_text(&self, src: &str, si: usize, pos: Pos) -> Value {
        match formula::parse(src) {
            Ok(e) => {
                let mut env = Env { sheet: si, pos, scope: vec![] };
                let v = self.eval(&e, &mut env);
                self.scalar(v, Some((si, pos)))
            }
            Err(_) => Value::Err(XlErr::Name),
        }
    }
}

fn pick(v: &Value, r: usize, c: usize) -> Value {
    match v {
        Value::Array(a) => {
            let rr = if a.rows == 1 { 0 } else { r };
            let cc = if a.cols == 1 { 0 } else { c };
            if rr < a.rows && cc < a.cols {
                a.get(rr, cc).clone()
            } else {
                Value::Err(XlErr::NA)
            }
        }
        x => x.clone(),
    }
}

pub fn broadcast2(l: &Value, r: &Value, f: impl Fn(&Value, &Value) -> Value) -> Value {
    let dims = |v: &Value| match v {
        Value::Array(a) => (a.rows, a.cols),
        _ => (1, 1),
    };
    let (ar, ac) = dims(l);
    let (br, bc) = dims(r);
    let (rows, cols) = (ar.max(br), ac.max(bc));
    let mut data = Vec::with_capacity(rows * cols);
    for i in 0..rows {
        for j in 0..cols {
            data.push(f(&pick(l, i, j), &pick(r, i, j)));
        }
    }
    Array::new(rows, cols, data)
}

fn top_scalar(v: &Value) -> Value {
    match v {
        Value::Array(a) => a.data.first().cloned().unwrap_or(Value::Empty),
        x => x.clone(),
    }
}

pub fn to_num(v: &Value) -> Result<f64, XlErr> {
    match v {
        Value::Num(n) => Ok(*n),
        Value::Empty => Ok(0.0),
        Value::Bool(b) => Ok(*b as i32 as f64),
        Value::Err(e) => Err(*e),
        Value::Text(t) => numfmt::text_to_number(t).ok_or(XlErr::Value),
        _ => Err(XlErr::Value),
    }
}

pub fn to_text(v: &Value) -> Result<String, XlErr> {
    match v {
        Value::Num(n) => Ok(num_text(*n)),
        Value::Empty => Ok(String::new()),
        Value::Bool(b) => Ok(if *b { "TRUE" } else { "FALSE" }.into()),
        Value::Err(e) => Err(*e),
        Value::Text(t) => Ok(t.to_string()),
        _ => Err(XlErr::Value),
    }
}

pub fn to_bool(v: &Value) -> Result<bool, XlErr> {
    match v {
        Value::Num(n) => Ok(*n != 0.0),
        Value::Empty => Ok(false),
        Value::Bool(b) => Ok(*b),
        Value::Err(e) => Err(*e),
        Value::Text(t) => {
            if t.eq_ignore_ascii_case("true") {
                Ok(true)
            } else if t.eq_ignore_ascii_case("false") {
                Ok(false)
            } else {
                Err(XlErr::Value)
            }
        }
        _ => Err(XlErr::Value),
    }
}

/// Spreadsheet ordering: numbers before text before booleans. None if an error is involved.
pub fn compare(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    let rank = |v: &Value| match v {
        Value::Num(_) | Value::Empty => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        _ => 3,
    };
    match (a, b) {
        (Value::Err(_), _) | (_, Value::Err(_)) => None,
        (Value::Empty, Value::Empty) => Some(Equal),
        (Value::Empty, Value::Text(t)) => Some(if t.is_empty() { Equal } else { Less }),
        (Value::Text(t), Value::Empty) => Some(if t.is_empty() { Equal } else { Greater }),
        (Value::Empty, Value::Bool(x)) => Some(false.cmp(x)),
        (Value::Bool(x), Value::Empty) => Some(x.cmp(&false)),
        (Value::Empty, Value::Num(y)) => 0f64.partial_cmp(y),
        (Value::Num(x), Value::Empty) => x.partial_cmp(&0.0),
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        (Value::Text(x), Value::Text(y)) => {
            if x.is_ascii() && y.is_ascii() {
                Some(cmp_ascii_ci(x, y))
            } else {
                Some(x.to_lowercase().cmp(&y.to_lowercase()))
            }
        }
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        _ => Some(rank(a).cmp(&rank(b))),
    }
}

fn cmp_ascii_ci(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes().map(|c| c.to_ascii_lowercase()).cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

fn scalar_bin(op: BinOp, l: &Value, r: &Value) -> Value {
    use std::cmp::Ordering::*;
    if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
        if let Value::Err(e) = l {
            return Value::Err(*e);
        }
        if let Value::Err(e) = r {
            return Value::Err(*e);
        }
        let Some(o) = compare(l, r) else {
            return Value::Err(XlErr::Value);
        };
        return Value::Bool(match op {
            BinOp::Eq => o == Equal,
            BinOp::Ne => o != Equal,
            BinOp::Lt => o == Less,
            BinOp::Le => o != Greater,
            BinOp::Gt => o == Greater,
            _ => o != Less,
        });
    }
    if op == BinOp::Concat {
        return match (to_text(l), to_text(r)) {
            (Ok(a), Ok(b)) => Value::Text((a + &b).into()),
            (Err(e), _) | (_, Err(e)) => Value::Err(e),
        };
    }
    let a = match to_num(l) {
        Ok(x) => x,
        Err(e) => return Value::Err(e),
    };
    let b = match to_num(r) {
        Ok(x) => x,
        Err(e) => return Value::Err(e),
    };
    let v = match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => {
            if b == 0.0 {
                return Value::Err(XlErr::Div0);
            }
            a / b
        }
        BinOp::Pow => {
            if a == 0.0 && b == 0.0 {
                return Value::Err(XlErr::Num);
            }
            if a == 0.0 && b < 0.0 {
                return Value::Err(XlErr::Div0);
            }
            a.powf(b)
        }
        _ => unreachable!(),
    };
    if v.is_finite() {
        // Values that differ from zero only by binary noise read as zero after + and -.
        if matches!(op, BinOp::Add | BinOp::Sub) && v != 0.0 && v.abs() < a.abs().max(b.abs()) * 1e-15 {
            return Value::Num(0.0);
        }
        Value::Num(v)
    } else {
        Value::Err(XlErr::Num)
    }
}

/// Recalculates the entire workbook. Spilled arrays need a second pass so readers see them.
pub fn recalc(wb: &Workbook) -> CalcResult {
    let ctx = Ctx::new(wb);
    run_all(&ctx);
    let (spill, rects) = collect_spills(wb, &ctx);
    if spill.is_empty() && rects.is_empty() {
        return ctx.into_result();
    }
    let circular = ctx.circular.get();
    drop(ctx);
    let ctx = Ctx::with_spills(wb, spill, rects);
    ctx.circular.set(circular);
    run_all(&ctx);
    // A second reading settles spill sizes that depend on other spills.
    let (spill2, rects2) = collect_spills(wb, &ctx);
    if spill2 != ctx.spill {
        let c2 = Ctx::with_spills(wb, spill2, rects2);
        run_all(&c2);
        let (s3, r3) = collect_spills(wb, &c2);
        let c3 = Ctx::with_spills(wb, s3, r3);
        run_all(&c3);
        return mark_blocked(c3.into_result());
    }
    mark_blocked(ctx.into_result())
}

fn mark_blocked(mut res: CalcResult) -> CalcResult {
    let rects: Vec<Key> = res.spill_rects.keys().copied().collect();
    let mut blocked = vec![];
    for k in rects {
        if res.spill_rects[&k].is_single() {
            blocked.push(k);
        }
    }
    for k in blocked {
        res.spill_rects.remove(&k);
        res.vals.insert(k, Value::Err(XlErr::Spill));
    }
    res
}

fn run_all(ctx: &Ctx) {
    for (si, sh) in ctx.wb.sheets.iter().enumerate() {
        for (&r, m) in &sh.rows {
            for (&c, cell) in m.iter() {
                if let Content::Formula(_) = cell.v {
                    ctx.cell(si, r, c);
                }
            }
        }
    }
}

fn collect_spills(wb: &Workbook, ctx: &Ctx) -> (HashMap<Key, Value>, HashMap<Key, Rect>) {
    let mut spill = HashMap::new();
    let mut rects = HashMap::new();
    let cache = ctx.cache.borrow();
    for (k, v) in cache.iter() {
        let Value::Array(a) = v else { continue };
        let si = k.0 as usize;
        let rc = Rect::new(
            k.1,
            k.2,
            k.1.saturating_add(a.rows as u32 - 1),
            k.2.saturating_add(a.cols as u32 - 1),
        );
        let sh = &wb.sheets[si];
        let mut blocked = rc.r2 >= MAX_ROWS || rc.c2 >= MAX_COLS;
        if !blocked {
            'o: for (&r, m) in sh.rows.range(rc.r1..=rc.r2) {
                for (&c, cell) in m.range(rc.c1..=rc.c2) {
                    if (r, c) != (k.1, k.2) && cell.v != Content::Empty {
                        blocked = true;
                        break 'o;
                    }
                }
            }
        }
        if blocked {
            // A one-cell rectangle marks the anchor as blocked for mark_blocked.
            rects.insert(*k, Rect::new(k.1, k.2, k.1, k.2));
            continue;
        }
        rects.insert(*k, rc);
        for r in 0..a.rows {
            for c in 0..a.cols {
                if (r, c) != (0, 0) {
                    spill.insert((k.0, k.1 + r as u32, k.2 + c as u32), a.get(r, c).clone());
                }
            }
        }
    }
    // Anchors keep their first element as their own value in the cell cache.
    (spill, rects)
}

/// Parsed criterion as used by COUNTIF and friends.
pub struct Crit {
    op: u8,
    val: Value,
    pat: Option<Vec<char>>,
}

impl Crit {
    pub fn new(v: &Value) -> Crit {
        let (op, val): (u8, Value) = match v {
            Value::Text(t) => {
                let s: &str = t;
                let (op, rest) = if let Some(x) = s.strip_prefix(">=") {
                    (b'g', x)
                } else if let Some(x) = s.strip_prefix("<=") {
                    (b'l', x)
                } else if let Some(x) = s.strip_prefix("<>") {
                    (b'n', x)
                } else if let Some(x) = s.strip_prefix('>') {
                    (b'>', x)
                } else if let Some(x) = s.strip_prefix('<') {
                    (b'<', x)
                } else if let Some(x) = s.strip_prefix('=') {
                    (b'=', x)
                } else {
                    (b'=', s)
                };
                let val = match numfmt::parse_input(rest).content {
                    Content::Num(n) => Value::Num(n),
                    Content::Bool(b) => Value::Bool(b),
                    Content::Err(e) => Value::Err(e),
                    Content::Empty => Value::Empty,
                    _ => Value::text(rest),
                };
                (op, val)
            }
            Value::Empty => (b'=', Value::Num(0.0)),
            x => (b'=', x.clone()),
        };
        let pat = match &val {
            Value::Text(t) if t.contains(['*', '?', '~']) => Some(t.to_lowercase().chars().collect()),
            _ => None,
        };
        Crit { op, val, pat }
    }

    pub fn matches(&self, v: &Value) -> bool {
        use std::cmp::Ordering::*;
        if self.op == b'=' || self.op == b'n' {
            let eq = match (&self.val, v) {
                (Value::Empty, x) => matches!(x, Value::Empty) || matches!(x, Value::Text(t) if t.is_empty()),
                (Value::Text(_), Value::Text(b)) if self.pat.is_some() => {
                    wild(self.pat.as_ref().unwrap(), &b.to_lowercase().chars().collect::<Vec<_>>())
                }
                (Value::Text(a), Value::Text(b)) => a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase(),
                (Value::Num(a), Value::Num(b)) => a == b,
                (Value::Num(a), Value::Text(b)) => numfmt::parse_plain_number(b) == Some(*a),
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (Value::Err(a), Value::Err(b)) => a == b,
                _ => false,
            };
            return if self.op == b'=' { eq } else { !eq };
        }
        // Ordered comparisons only look at the same type.
        let same = matches!(
            (&self.val, v),
            (Value::Num(_), Value::Num(_)) | (Value::Text(_), Value::Text(_)) | (Value::Bool(_), Value::Bool(_))
        );
        if !same {
            return false;
        }
        let Some(o) = compare(v, &self.val) else { return false };
        match self.op {
            b'>' => o == Greater,
            b'<' => o == Less,
            b'g' => o != Less,
            b'l' => o != Greater,
            _ => false,
        }
    }
}

/// Wildcard match: `*` any run, `?` one character, `~` escapes.
pub fn wild(p: &[char], s: &[char]) -> bool {
    let (mut i, mut j) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while j < s.len() {
        if i < p.len() && p[i] == '~' && i + 1 < p.len() && p[i + 1] == s[j] {
            i += 2;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if i < p.len() && p[i] != '~' && (p[i] == '?' || p[i] == s[j]) {
            i += 1;
            j += 1;
        } else if let Some(st) = star {
            i = st + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}
