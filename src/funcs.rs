//! The function library: registry, helpers, and the logical group.

use crate::eval::*;
use crate::model::*;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub type F = for<'c, 'a, 'v> fn(&Cx<'c, 'a>, &'v [Value]) -> R;

pub struct FnDef {
    pub name: &'static str,
    pub cat: &'static str,
    pub lift: bool,
    pub min: u16,
    pub max: u16,
    pub sig: &'static str,
    pub desc: &'static str,
    pub f: F,
}

pub struct Reg {
    pub map: HashMap<&'static str, FnDef>,
    pub cat: &'static str,
}

pub const MANY: u16 = 255;

impl Reg {
    /// Registers a scalar function, which maps over arrays automatically.
    pub fn s(&mut self, name: &'static str, min: u16, max: u16, sig: &'static str, desc: &'static str, f: F) {
        self.map.insert(name, FnDef { name, cat: self.cat, lift: true, min, max, sig, desc, f });
    }
    /// Registers a function that receives its ranges and arrays whole.
    pub fn a(&mut self, name: &'static str, min: u16, max: u16, sig: &'static str, desc: &'static str, f: F) {
        self.map.insert(name, FnDef { name, cat: self.cat, lift: false, min, max, sig, desc, f });
    }
    pub fn alias(&mut self, name: &'static str, of: &'static str) {
        if let Some(d) = self.map.get(of) {
            let n = FnDef { name, cat: d.cat, lift: d.lift, min: d.min, max: d.max, sig: d.sig, desc: d.desc, f: d.f };
            self.map.insert(name, n);
        }
    }
}

pub fn registry() -> &'static Reg {
    static R: OnceLock<Reg> = OnceLock::new();
    R.get_or_init(|| {
        let mut r = Reg { map: HashMap::new(), cat: "" };
        logical(&mut r);
        crate::funcs_math::math(&mut r);
        crate::funcs_text::text(&mut r);
        crate::funcs_text::date(&mut r);
        crate::funcs_text::info(&mut r);
        crate::funcs_stat::stats(&mut r);
        crate::funcs_look::lookup(&mut r);
        crate::funcs_fin::financial(&mut r);
        crate::funcs_fin::engineering(&mut r);
        crate::funcs_look::database(&mut r);
        // Functions the evaluator handles itself still appear in the catalogue.
        let stub: F = |_, _| Err(XlErr::Value);
        r.cat = "Logical";
        r.a("IF", 1, 3, "logical_test, [value_if_true], [value_if_false]", "Checks a condition and returns one value if TRUE, another if FALSE", stub);
        r.a("IFERROR", 2, 2, "value, value_if_error", "Returns value_if_error if the expression is an error", stub);
        r.a("IFNA", 2, 2, "value, value_if_na", "Returns value_if_na if the expression gives #N/A", stub);
        r.a("IFS", 2, MANY, "logical_test1, value_if_true1, ...", "Returns the value for the first TRUE condition", stub);
        r.a("SWITCH", 3, MANY, "expression, value1, result1, ..., [default]", "Compares an expression to a list of values", stub);
        r.a("LET", 3, MANY, "name1, value1, ..., calculation", "Assigns names to values inside a formula", stub);
        r.a("LAMBDA", 1, MANY, "[parameter1, ...], calculation", "Creates a custom reusable function", stub);
        r.cat = "Lookup & Reference";
        r.a("CHOOSE", 2, MANY, "index_num, value1, [value2], ...", "Chooses a value from a list by index number", stub);
        r
    })
}

pub fn n(a: &[Value], i: usize) -> Result<f64, XlErr> {
    a.get(i).map_or(Err(XlErr::Value), to_num)
}
/// Optional number: absent or blank gives the default.
pub fn nd(a: &[Value], i: usize, d: f64) -> Result<f64, XlErr> {
    match a.get(i) {
        None | Some(Value::Empty) => Ok(d),
        Some(v) => to_num(v),
    }
}
pub fn t(a: &[Value], i: usize) -> Result<String, XlErr> {
    a.get(i).map_or(Err(XlErr::Value), to_text)
}
pub fn td(a: &[Value], i: usize, d: &str) -> Result<String, XlErr> {
    match a.get(i) {
        None | Some(Value::Empty) => Ok(d.to_string()),
        Some(v) => to_text(v),
    }
}
pub fn bd(a: &[Value], i: usize, d: bool) -> Result<bool, XlErr> {
    match a.get(i) {
        None | Some(Value::Empty) => Ok(d),
        Some(v) => to_bool(v),
    }
}
pub fn ok(x: f64) -> R {
    if x.is_finite() {
        Ok(Value::Num(x))
    } else {
        Err(XlErr::Num)
    }
}
pub fn txt(s: impl AsRef<str>) -> R {
    Ok(Value::Text(s.as_ref().into()))
}

impl<'c, 'a> Cx<'c, 'a> {
    /// Numbers as SUM reads them: inside ranges only real numbers count, typed arguments are converted.
    pub fn nums(&self, args: &[Value]) -> Result<Vec<f64>, XlErr> {
        let mut out = vec![];
        for v in args {
            self.push_nums(v, &mut out)?;
        }
        Ok(out)
    }
    fn push_nums(&self, v: &Value, out: &mut Vec<f64>) -> Result<(), XlErr> {
        match v {
            Value::Ref(si, rc) => {
                let mut err = None;
                self.each_in(*si, *rc, &mut |_, _, x| match x {
                    Value::Num(n) => out.push(n),
                    Value::Err(e) if err.is_none() => err = Some(e),
                    _ => {}
                });
                if let Some(e) = err {
                    return Err(e);
                }
            }
            Value::Array(a) => {
                for x in &a.data {
                    match x {
                        Value::Num(n) => out.push(*n),
                        Value::Err(e) => return Err(*e),
                        _ => {}
                    }
                }
            }
            Value::Empty => {}
            x => out.push(to_num(x)?),
        }
        Ok(())
    }
    /// Numbers with the "A" rules: text in ranges is 0 and TRUE is 1.
    pub fn nums_a(&self, args: &[Value]) -> Result<Vec<f64>, XlErr> {
        let mut out = vec![];
        for v in args {
            match v {
                Value::Ref(..) | Value::Array(_) => {
                    for x in self.flat_nonblank(v) {
                        match x {
                            Value::Num(n) => out.push(n),
                            Value::Bool(b) => out.push(b as i32 as f64),
                            Value::Text(_) => out.push(0.0),
                            Value::Err(e) => return Err(e),
                            _ => {}
                        }
                    }
                }
                Value::Empty => {}
                x => out.push(to_num(x)?),
            }
        }
        Ok(out)
    }
    /// Every non-empty value in a range or array, in no particular order for references.
    pub fn flat_nonblank(&self, v: &Value) -> Vec<Value> {
        let mut out = vec![];
        match v {
            Value::Ref(si, rc) => self.each_in(*si, *rc, &mut |_, _, x| {
                if x != Value::Empty {
                    out.push(x)
                }
            }),
            Value::Array(a) => out.extend(a.data.iter().filter(|x| **x != Value::Empty).cloned()),
            Value::Empty => {}
            x => out.push(x.clone()),
        }
        out
    }
    pub fn arr(&self, v: &Value) -> Arc<Array> {
        match v {
            Value::Ref(si, rc) => self.array_of(*si, *rc),
            Value::Array(a) => a.clone(),
            x => Arc::new(Array { rows: 1, cols: 1, data: vec![x.clone()] }),
        }
    }
    /// A range as a flat list, row by row, with blanks kept.
    pub fn flat(&self, v: &Value) -> Vec<Value> {
        self.arr(v).data.clone()
    }
    pub fn nums_dense(&self, v: &Value) -> Result<Vec<Option<f64>>, XlErr> {
        self.flat(v)
            .iter()
            .map(|x| match x {
                Value::Num(n) => Ok(Some(*n)),
                Value::Err(e) => Err(*e),
                _ => Ok(None),
            })
            .collect()
    }
    pub fn env(&self) -> Env {
        Env { sheet: self.sheet, pos: self.pos, scope: self.scope.to_vec() }
    }
}

/// Pairs of numbers from two equally-sized ranges, skipping pairs with a non-number.
pub fn pairs(c: &Cx, x: &Value, y: &Value) -> Result<(Vec<f64>, Vec<f64>), XlErr> {
    let a = c.nums_dense(x)?;
    let b = c.nums_dense(y)?;
    if a.len() != b.len() {
        return Err(XlErr::NA);
    }
    let mut xs = vec![];
    let mut ys = vec![];
    for (p, q) in a.iter().zip(b.iter()) {
        if let (Some(p), Some(q)) = (p, q) {
            xs.push(*p);
            ys.push(*q);
        }
    }
    Ok((xs, ys))
}

fn bools(c: &Cx, a: &[Value]) -> Result<Vec<bool>, XlErr> {
    let mut out = vec![];
    for v in a {
        match v {
            Value::Ref(..) | Value::Array(_) => {
                for x in c.flat_nonblank(v) {
                    match x {
                        Value::Bool(b) => out.push(b),
                        Value::Num(n) => out.push(n != 0.0),
                        Value::Err(e) => return Err(e),
                        _ => {}
                    }
                }
            }
            Value::Empty => {}
            x => out.push(to_bool(x)?),
        }
    }
    if out.is_empty() {
        return Err(XlErr::Value);
    }
    Ok(out)
}

fn logical(r: &mut Reg) {
    r.cat = "Logical";
    r.a("AND", 1, MANY, "logical1, [logical2], ...", "TRUE if all arguments are TRUE", |c, a| Ok(Value::Bool(bools(c, a)?.iter().all(|x| *x))));
    r.a("OR", 1, MANY, "logical1, [logical2], ...", "TRUE if any argument is TRUE", |c, a| Ok(Value::Bool(bools(c, a)?.iter().any(|x| *x))));
    r.a("XOR", 1, MANY, "logical1, [logical2], ...", "TRUE if an odd number of arguments are TRUE", |c, a| Ok(Value::Bool(bools(c, a)?.iter().filter(|x| **x).count() % 2 == 1)));
    r.s("NOT", 1, 1, "logical", "Reverses a logical value", |_, a| Ok(Value::Bool(!to_bool(&a[0])?)));
    r.a("TRUE", 0, 0, "", "Returns the logical value TRUE", |_, _| Ok(Value::Bool(true)));
    r.a("FALSE", 0, 0, "", "Returns the logical value FALSE", |_, _| Ok(Value::Bool(false)));
    // Functions that take LAMBDAs.
    r.a("MAP", 2, MANY, "array1, [array2, ...], lambda", "Applies a LAMBDA to each value of the arrays", |c, a| {
        let (f, arrs) = a.split_last().unwrap();
        let Value::Lambda(l) = f else { return Err(XlErr::Value) };
        let arrs: Vec<_> = arrs.iter().map(|x| c.arr(x)).collect();
        let (rows, cols) = (arrs[0].rows, arrs[0].cols);
        let env = c.env();
        let mut data = vec![];
        for i in 0..rows * cols {
            let vals = arrs.iter().map(|x| x.data.get(i).cloned().unwrap_or(Value::Empty)).collect();
            data.push(c.ctx.scalar(c.apply(l, vals, &env), None));
        }
        Ok(Array::new(rows, cols, data))
    });
    r.a("REDUCE", 3, 3, "initial_value, array, lambda", "Reduces an array to one value with a LAMBDA", |c, a| {
        let Value::Lambda(l) = &a[2] else { return Err(XlErr::Value) };
        let env = c.env();
        let mut acc = c.ctx.scalar(a[0].clone(), None);
        for v in c.flat(&a[1]) {
            acc = c.apply(l, vec![acc, v], &env);
        }
        Ok(acc)
    });
    r.a("SCAN", 3, 3, "initial_value, array, lambda", "Running application of a LAMBDA over an array", |c, a| {
        let Value::Lambda(l) = &a[2] else { return Err(XlErr::Value) };
        let env = c.env();
        let arr = c.arr(&a[1]);
        let mut acc = c.ctx.scalar(a[0].clone(), None);
        let mut data = vec![];
        for v in arr.data.iter() {
            acc = c.apply(l, vec![acc, v.clone()], &env);
            data.push(acc.clone());
        }
        Ok(Array::new(arr.rows, arr.cols, data))
    });
    r.a("BYROW", 2, 2, "array, lambda", "Applies a LAMBDA to each row", |c, a| {
        let Value::Lambda(l) = &a[1] else { return Err(XlErr::Value) };
        let env = c.env();
        let arr = c.arr(&a[0]);
        let data = (0..arr.rows)
            .map(|i| c.ctx.scalar(c.apply(l, vec![Array::new(1, arr.cols, arr.row_of(i))], &env), None))
            .collect();
        Ok(Array::new(arr.rows, 1, data))
    });
    r.a("BYCOL", 2, 2, "array, lambda", "Applies a LAMBDA to each column", |c, a| {
        let Value::Lambda(l) = &a[1] else { return Err(XlErr::Value) };
        let env = c.env();
        let arr = c.arr(&a[0]);
        let data = (0..arr.cols)
            .map(|i| c.ctx.scalar(c.apply(l, vec![Array::new(arr.rows, 1, arr.col_of(i))], &env), None))
            .collect();
        Ok(Array::new(1, arr.cols, data))
    });
    r.a("MAKEARRAY", 3, 3, "rows, cols, lambda", "Builds an array by calling a LAMBDA", |c, a| {
        let Value::Lambda(l) = &a[2] else { return Err(XlErr::Value) };
        let env = c.env();
        let (rows, cols) = (n(a, 0)? as usize, n(a, 1)? as usize);
        let mut data = vec![];
        for i in 0..rows {
            for j in 0..cols {
                let v = vec![Value::Num(i as f64 + 1.0), Value::Num(j as f64 + 1.0)];
                data.push(c.ctx.scalar(c.apply(l, v, &env), None));
            }
        }
        Ok(Array::new(rows, cols, data))
    });
}
