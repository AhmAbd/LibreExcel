//! Math and trigonometry functions.

use crate::eval::*;
use crate::funcs::*;
use crate::model::*;

pub fn gcd(a: f64, b: f64) -> f64 {
    let (mut a, mut b) = (a.abs().trunc(), b.abs().trunc());
    while b > 0.0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

pub fn fact(x: f64) -> f64 {
    let n = x.trunc() as i64;
    let mut v = 1.0;
    for i in 2..=n {
        v *= i as f64;
    }
    v
}

pub fn combin(n: f64, k: f64) -> f64 {
    let (n, k) = (n.trunc(), k.trunc());
    let k = k.min(n - k);
    let mut v = 1.0;
    let mut i = 0.0;
    while i < k {
        v = v * (n - i) / (i + 1.0);
        i += 1.0;
    }
    v.round()
}

pub fn round_to(x: f64, d: i32, mode: u8) -> f64 {
    // mode: 0 nearest, 1 up (away from zero), 2 down (toward zero).
    let m = 10f64.powi(d.abs());
    let y = if d >= 0 { x * m } else { x / m };
    let cleaned = {
        let r = y.round();
        if (y - r).abs() < 1e-12 * y.abs().max(1.0) {
            r
        } else {
            y
        }
    };
    let z = match mode {
        0 => {
            let nudged = y.abs() + y.abs().max(1.0) * 1e-14;
            (nudged + 0.5).floor().copysign(y)
        }
        1 => cleaned.abs().ceil().copysign(y),
        _ => cleaned.trunc(),
    };
    if d >= 0 {
        z / m
    } else {
        z * m
    }
}

fn ceil_sig(x: f64, s: f64) -> R {
    if s == 0.0 {
        return Ok(Value::Num(0.0));
    }
    if x > 0.0 && s < 0.0 {
        return Err(XlErr::Num);
    }
    ok((x / s - 1e-12).ceil() * s)
}

fn floor_sig(x: f64, s: f64) -> R {
    if s == 0.0 {
        return if x == 0.0 { Ok(Value::Num(0.0)) } else { Err(XlErr::Div0) };
    }
    if x > 0.0 && s < 0.0 {
        return Err(XlErr::Num);
    }
    ok((x / s + 1e-12).floor() * s)
}

fn roman(mut n: i64) -> String {
    let t = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut s = String::new();
    for (v, r) in t {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn from_roman(s: &str) -> Option<f64> {
    let val = |c: char| {
        Some(match c.to_ascii_uppercase() {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        })
    };
    let ch: Vec<char> = s.chars().collect();
    let mut tot = 0i64;
    for i in 0..ch.len() {
        let v = val(ch[i])?;
        let nx = if i + 1 < ch.len() { val(ch[i + 1])? } else { 0 };
        if v < nx {
            tot -= v
        } else {
            tot += v
        }
    }
    Some(tot as f64)
}

fn mdet(m: &[Vec<f64>]) -> f64 {
    let n = m.len();
    let mut a = m.to_vec();
    let mut det = 1.0;
    for i in 0..n {
        let mut p = i;
        for j in i + 1..n {
            if a[j][i].abs() > a[p][i].abs() {
                p = j;
            }
        }
        if a[p][i] == 0.0 {
            return 0.0;
        }
        if p != i {
            a.swap(p, i);
            det = -det;
        }
        det *= a[i][i];
        for j in i + 1..n {
            let f = a[j][i] / a[i][i];
            for k in i..n {
                a[j][k] -= f * a[i][k];
            }
        }
    }
    det
}

pub fn mat(c: &Cx, v: &Value) -> Result<Vec<Vec<f64>>, XlErr> {
    let a = c.arr(v);
    let mut out = vec![];
    for r in 0..a.rows {
        let mut row = vec![];
        for x in a.row_of(r) {
            match x {
                Value::Num(n) => row.push(n),
                Value::Bool(b) => row.push(b as i32 as f64),
                Value::Err(e) => return Err(e),
                _ => return Err(XlErr::Value),
            }
        }
        out.push(row);
    }
    Ok(out)
}

pub fn mat_val(m: &[Vec<f64>]) -> Value {
    let rows = m.len();
    let cols = m.first().map_or(0, |x| x.len());
    Array::new(rows, cols, m.iter().flatten().map(|x| Value::Num(*x)).collect())
}

pub fn minverse(m: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut x = r.clone();
            for j in 0..n {
                x.push(if i == j { 1.0 } else { 0.0 });
            }
            x
        })
        .collect();
    for i in 0..n {
        let mut p = i;
        for j in i + 1..n {
            if a[j][i].abs() > a[p][i].abs() {
                p = j;
            }
        }
        if a[p][i].abs() < 1e-14 {
            return None;
        }
        a.swap(p, i);
        let d = a[i][i];
        for k in 0..2 * n {
            a[i][k] /= d;
        }
        for j in 0..n {
            if j != i {
                let f = a[j][i];
                for k in 0..2 * n {
                    a[j][k] -= f * a[i][k];
                }
            }
        }
    }
    Some(a.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// SUMIFS-style condition pairs turned into a keep-list over a range shape.
fn crit_mask(c: &Cx, pairs: &[Value], rows: usize, cols: usize) -> Result<Vec<bool>, XlErr> {
    let mut mask = vec![true; rows * cols];
    for p in pairs.chunks(2) {
        let [rng, cr] = p else { return Err(XlErr::Value) };
        let arr = c.arr(rng);
        if arr.rows * arr.cols != rows * cols {
            return Err(XlErr::Value);
        }
        let cr0 = c.ctx.scalar(cr.clone(), Some((c.sheet, c.pos)));
        if let Value::Err(e) = cr0 {
            return Err(e);
        }
        let crit = Crit::new(&cr0);
        for (i, m) in mask.iter_mut().enumerate() {
            if *m {
                *m = crit.matches(&arr.data[i]);
            }
        }
    }
    Ok(mask)
}

fn ifs_values(c: &Cx, target: &Value, conds: &[Value]) -> Result<Vec<f64>, XlErr> {
    let t = c.arr(target);
    let mask = crit_mask(c, conds, t.rows, t.cols)?;
    let mut out = vec![];
    for (i, m) in mask.iter().enumerate() {
        if *m {
            match t.data[i] {
                Value::Num(x) => out.push(x),
                Value::Err(e) => return Err(e),
                _ => {}
            }
        }
    }
    Ok(out)
}

/// A SUMIF range shaped like the criteria range, starting at its own top-left cell.
fn resized(sum: &Value, crit: &Value) -> Value {
    let (Value::Ref(si, s), Value::Ref(_, k)) = (sum, crit) else { return sum.clone() };
    Value::Ref(*si, Rect::new(s.r1, s.c1, s.r1 + k.r2 - k.r1, s.c1 + k.c2 - k.c1))
}

fn is_subtotal_cell(c: &Cx, si: usize, r: u32, co: u32) -> bool {
    if let Some(x) = c.wb.sheets[si].get(r, co) {
        if let Content::Formula(e) = &x.v {
            return matches!(&**e, crate::formula::Expr::Call(n, _) if &**n == "SUBTOTAL" || &**n == "AGGREGATE");
        }
    }
    false
}

/// Applies an aggregate by SUBTOTAL number.
pub fn aggregate_by(code: i64, v: &[f64], counta: usize) -> R {
    use crate::funcs_stat as s;
    match code {
        1 => {
            if v.is_empty() {
                Err(XlErr::Div0)
            } else {
                ok(v.iter().sum::<f64>() / v.len() as f64)
            }
        }
        2 => Ok(Value::Num(v.len() as f64)),
        3 => Ok(Value::Num(counta as f64)),
        4 => ok(v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)).or(Ok(Value::Num(0.0))),
        5 => ok(v.iter().cloned().fold(f64::INFINITY, f64::min)).or(Ok(Value::Num(0.0))),
        6 => ok(v.iter().product()),
        7 => s::stdev(v, true),
        8 => s::stdev(v, false),
        9 => ok(v.iter().sum()),
        10 => s::variance(v, true),
        11 => s::variance(v, false),
        12 => s::median(v),
        _ => Err(XlErr::Value),
    }
}

fn subtotal(c: &Cx, code: i64, ranges: &[Value], skip_hidden: bool) -> R {
    let mut v = vec![];
    let mut counta = 0;
    for rng in ranges {
        match rng {
            Value::Ref(si, rc) => {
                let mut err = None;
                let sh = &c.wb.sheets[*si];
                c.each_in(*si, *rc, &mut |r, co, x| {
                    if sh.filtered_rows.contains(&r) || (skip_hidden && sh.hidden_rows.contains(&r)) {
                        return;
                    }
                    if is_subtotal_cell(c, *si, r, co) {
                        return;
                    }
                    counta += 1;
                    match x {
                        Value::Num(n) => v.push(n),
                        Value::Err(e) if err.is_none() => err = Some(e),
                        _ => {}
                    }
                });
                if let Some(e) = err {
                    return Err(e);
                }
            }
            x => {
                let m = c.nums(std::slice::from_ref(x))?;
                counta += m.len();
                v.extend(m);
            }
        }
    }
    aggregate_by(code, &v, counta)
}

pub fn rand01() -> f64 {
    use std::cell::Cell;
    thread_local! { static S: Cell<u64> = const { Cell::new(0) }; }
    S.with(|s| {
        let mut x = s.get();
        if x == 0 {
            x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(88172645463325252) | 1;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        (x >> 11) as f64 / (1u64 << 53) as f64
    })
}

pub fn math(r: &mut Reg) {
    r.cat = "Math & Trig";
    macro_rules! m1 {
        ($name:literal, $desc:literal, $f:expr) => {
            r.s($name, 1, 1, "number", $desc, |_, a| {
                let f: fn(f64) -> f64 = $f;
                ok(f(n(a, 0)?))
            });
        };
    }
    m1!("ABS", "Absolute value of a number", f64::abs);
    m1!("SIN", "Sine of an angle in radians", f64::sin);
    m1!("COS", "Cosine of an angle in radians", f64::cos);
    m1!("TAN", "Tangent of an angle in radians", f64::tan);
    m1!("SINH", "Hyperbolic sine", f64::sinh);
    m1!("COSH", "Hyperbolic cosine", f64::cosh);
    m1!("TANH", "Hyperbolic tangent", f64::tanh);
    m1!("ATAN", "Arctangent, in radians", f64::atan);
    m1!("ASINH", "Inverse hyperbolic sine", f64::asinh);
    m1!("EXP", "e raised to a power", f64::exp);
    m1!("RADIANS", "Converts degrees to radians", f64::to_radians);
    m1!("DEGREES", "Converts radians to degrees", f64::to_degrees);
    m1!("SIGN", "Sign of a number: 1, 0 or -1", |x| if x == 0.0 { 0.0 } else { x.signum() });
    m1!("INT", "Rounds down to the nearest integer", f64::floor);
    m1!("SEC", "Secant of an angle", |x| 1.0 / x.cos());
    m1!("SECH", "Hyperbolic secant", |x| 1.0 / x.cosh());
    m1!("ACOT", "Arccotangent of a number", |x| std::f64::consts::FRAC_PI_2 - x.atan());
    r.s("ACOS", 1, 1, "number", "Arccosine, in radians", |_, a| { let x = n(a, 0)?; if x.abs() > 1.0 { Err(XlErr::Num) } else { ok(x.acos()) } });
    r.s("ASIN", 1, 1, "number", "Arcsine, in radians", |_, a| { let x = n(a, 0)?; if x.abs() > 1.0 { Err(XlErr::Num) } else { ok(x.asin()) } });
    r.s("ACOSH", 1, 1, "number", "Inverse hyperbolic cosine", |_, a| { let x = n(a, 0)?; if x < 1.0 { Err(XlErr::Num) } else { ok(x.acosh()) } });
    r.s("ATANH", 1, 1, "number", "Inverse hyperbolic tangent", |_, a| { let x = n(a, 0)?; if x.abs() >= 1.0 { Err(XlErr::Num) } else { ok(x.atanh()) } });
    r.s("ACOTH", 1, 1, "number", "Inverse hyperbolic cotangent", |_, a| { let x = n(a, 0)?; if x.abs() <= 1.0 { Err(XlErr::Num) } else { ok(0.5 * ((x + 1.0) / (x - 1.0)).ln()) } });
    r.s("COT", 1, 1, "number", "Cotangent of an angle", |_, a| { let x = n(a, 0)?; if x == 0.0 { Err(XlErr::Div0) } else { ok(1.0 / x.tan()) } });
    r.s("COTH", 1, 1, "number", "Hyperbolic cotangent", |_, a| { let x = n(a, 0)?; if x == 0.0 { Err(XlErr::Div0) } else { ok(1.0 / x.tanh()) } });
    r.s("CSC", 1, 1, "number", "Cosecant of an angle", |_, a| { let x = n(a, 0)?; if x == 0.0 { Err(XlErr::Div0) } else { ok(1.0 / x.sin()) } });
    r.s("CSCH", 1, 1, "number", "Hyperbolic cosecant", |_, a| { let x = n(a, 0)?; if x == 0.0 { Err(XlErr::Div0) } else { ok(1.0 / x.sinh()) } });
    r.s("ATAN2", 2, 2, "x_num, y_num", "Arctangent from x and y coordinates", |_, a| {
        let (x, y) = (n(a, 0)?, n(a, 1)?);
        if x == 0.0 && y == 0.0 { Err(XlErr::Div0) } else { ok(y.atan2(x)) }
    });
    r.a("PI", 0, 0, "", "The value of pi", |_, _| Ok(Value::Num(std::f64::consts::PI)));
    r.s("LN", 1, 1, "number", "Natural logarithm", |_, a| { let x = n(a, 0)?; if x <= 0.0 { Err(XlErr::Num) } else { ok(x.ln()) } });
    r.s("LOG10", 1, 1, "number", "Base-10 logarithm", |_, a| { let x = n(a, 0)?; if x <= 0.0 { Err(XlErr::Num) } else { ok(x.log10()) } });
    r.s("LOG", 1, 2, "number, [base]", "Logarithm to the given base", |_, a| {
        let (x, b) = (n(a, 0)?, nd(a, 1, 10.0)?);
        if x <= 0.0 || b <= 0.0 { return Err(XlErr::Num); }
        if b == 1.0 { return Err(XlErr::Div0); }
        ok(x.ln() / b.ln())
    });
    r.s("SQRT", 1, 1, "number", "Positive square root", |_, a| { let x = n(a, 0)?; if x < 0.0 { Err(XlErr::Num) } else { ok(x.sqrt()) } });
    r.s("SQRTPI", 1, 1, "number", "Square root of (number * pi)", |_, a| { let x = n(a, 0)?; if x < 0.0 { Err(XlErr::Num) } else { ok((x * std::f64::consts::PI).sqrt()) } });
    r.s("POWER", 2, 2, "number, power", "Raises a number to a power", |_, a| {
        let (x, y) = (n(a, 0)?, n(a, 1)?);
        if x == 0.0 && y == 0.0 { return Err(XlErr::Num); }
        if x == 0.0 && y < 0.0 { return Err(XlErr::Div0); }
        ok(x.powf(y))
    });
    r.s("MOD", 2, 2, "number, divisor", "Remainder after division", |_, a| {
        let (x, d) = (n(a, 0)?, n(a, 1)?);
        if d == 0.0 { return Err(XlErr::Div0); }
        ok(x - d * (x / d).floor())
    });
    r.s("QUOTIENT", 2, 2, "numerator, denominator", "Integer part of a division", |_, a| {
        let d = n(a, 1)?;
        if d == 0.0 { return Err(XlErr::Div0); }
        ok((n(a, 0)? / d).trunc())
    });
    r.s("ROUND", 2, 2, "number, num_digits", "Rounds to a number of digits", |_, a| ok(round_to(n(a, 0)?, n(a, 1)? as i32, 0)));
    r.s("ROUNDUP", 2, 2, "number, num_digits", "Rounds away from zero", |_, a| ok(round_to(n(a, 0)?, n(a, 1)? as i32, 1)));
    r.s("ROUNDDOWN", 2, 2, "number, num_digits", "Rounds toward zero", |_, a| ok(round_to(n(a, 0)?, n(a, 1)? as i32, 2)));
    r.s("TRUNC", 1, 2, "number, [num_digits]", "Truncates a number", |_, a| ok(round_to(n(a, 0)?, nd(a, 1, 0.0)? as i32, 2)));
    r.s("MROUND", 2, 2, "number, multiple", "Rounds to the nearest multiple", |_, a| {
        let (x, m) = (n(a, 0)?, n(a, 1)?);
        if m == 0.0 { return Ok(Value::Num(0.0)); }
        if x * m < 0.0 { return Err(XlErr::Num); }
        ok((x / m + 1e-12f64.copysign(x / m)).round() * m)
    });
    r.s("CEILING", 2, 2, "number, significance", "Rounds up to a multiple", |_, a| ceil_sig(n(a, 0)?, n(a, 1)?));
    r.s("CEILING.PRECISE", 1, 2, "number, [significance]", "Rounds up to a multiple, whatever the sign", |_, a| ceil_sig(n(a, 0)?, nd(a, 1, 1.0)?.abs()));
    r.alias("ISO.CEILING", "CEILING.PRECISE");
    r.s("CEILING.MATH", 1, 3, "number, [significance], [mode]", "Rounds up to a multiple", |_, a| {
        let (x, s, mode) = (n(a, 0)?, nd(a, 1, 1.0)?.abs(), nd(a, 2, 0.0)?);
        if s == 0.0 { return Ok(Value::Num(0.0)); }
        if x < 0.0 && mode != 0.0 { ok(-((-x / s).ceil() * s)) } else { ok((x / s).ceil() * s) }
    });
    r.s("FLOOR", 2, 2, "number, significance", "Rounds down to a multiple", |_, a| floor_sig(n(a, 0)?, n(a, 1)?));
    r.s("FLOOR.PRECISE", 1, 2, "number, [significance]", "Rounds down to a multiple, whatever the sign", |_, a| floor_sig(n(a, 0)?, nd(a, 1, 1.0)?.abs()));
    r.s("FLOOR.MATH", 1, 3, "number, [significance], [mode]", "Rounds down to a multiple", |_, a| {
        let (x, s, mode) = (n(a, 0)?, nd(a, 1, 1.0)?.abs(), nd(a, 2, 0.0)?);
        if s == 0.0 { return Ok(Value::Num(0.0)); }
        if x < 0.0 && mode != 0.0 { ok(-((-x / s).floor() * s)) } else { ok((x / s).floor() * s) }
    });
    r.s("EVEN", 1, 1, "number", "Rounds up to the nearest even integer", |_, a| {
        let x = n(a, 0)?;
        ok(((x.abs() / 2.0).ceil() * 2.0).copysign(x))
    });
    r.s("ODD", 1, 1, "number", "Rounds up to the nearest odd integer", |_, a| {
        let x = n(a, 0)?;
        let y = x.abs().ceil();
        let y = if y % 2.0 == 0.0 { y + 1.0 } else { y };
        ok(if x == 0.0 { 1.0 } else { y.copysign(x) })
    });
    r.s("FACT", 1, 1, "number", "Factorial", |_, a| { let x = n(a, 0)?; if x < 0.0 { Err(XlErr::Num) } else { ok(fact(x)) } });
    r.s("FACTDOUBLE", 1, 1, "number", "Double factorial", |_, a| {
        let x = n(a, 0)?.trunc();
        if x < -1.0 { return Err(XlErr::Num); }
        let mut v = 1.0; let mut i = x;
        while i > 1.0 { v *= i; i -= 2.0; }
        ok(v)
    });
    r.s("COMBIN", 2, 2, "number, number_chosen", "Number of combinations", |_, a| {
        let (x, k) = (n(a, 0)?, n(a, 1)?);
        if x < 0.0 || k < 0.0 || k > x { Err(XlErr::Num) } else { ok(combin(x, k)) }
    });
    r.s("COMBINA", 2, 2, "number, number_chosen", "Combinations with repetitions", |_, a| {
        let (x, k) = (n(a, 0)?, n(a, 1)?);
        if x < 0.0 || k < 0.0 { Err(XlErr::Num) } else { ok(combin(x + k - 1.0, k)) }
    });
    r.a("GCD", 1, MANY, "number1, [number2], ...", "Greatest common divisor", |c, a| ok(c.nums(a)?.iter().fold(0.0, |x, y| gcd(x, *y))));
    r.a("LCM", 1, MANY, "number1, [number2], ...", "Least common multiple", |c, a| {
        let v = c.nums(a)?;
        let mut l: f64 = 1.0;
        for x in v {
            let x = x.trunc().abs();
            if x == 0.0 { return Ok(Value::Num(0.0)); }
            l = l / gcd(l, x) * x;
        }
        ok(l)
    });
    aggregates(r);
}

fn aggregates(r: &mut Reg) {
    r.a("SUM", 1, MANY, "number1, [number2], ...", "Adds all the numbers in a range of cells", |c, a| ok(c.nums(a)?.iter().sum()));
    r.a("PRODUCT", 1, MANY, "number1, [number2], ...", "Multiplies all its arguments", |c, a| {
        let v = c.nums(a)?;
        if v.is_empty() { Ok(Value::Num(0.0)) } else { ok(v.iter().product()) }
    });
    r.a("SUMSQ", 1, MANY, "number1, [number2], ...", "Sum of the squares", |c, a| ok(c.nums(a)?.iter().map(|x| x * x).sum()));
    r.a("SUMIF", 2, 3, "range, criteria, [sum_range]", "Adds the cells that meet a condition", |c, a| {
        let target = match a.get(2) { Some(s) if *s != Value::Empty => resized(s, &a[0]), _ => a[0].clone() };
        ok(ifs_values(c, &target, &[a[0].clone(), a[1].clone()])?.iter().sum())
    });
    r.a("SUMIFS", 3, MANY, "sum_range, criteria_range1, criteria1, ...", "Adds the cells that meet all conditions", |c, a| ok(ifs_values(c, &a[0], &a[1..])?.iter().sum()));
    r.a("AVERAGEIF", 2, 3, "range, criteria, [average_range]", "Average of the cells that meet a condition", |c, a| {
        let target = match a.get(2) { Some(s) if *s != Value::Empty => resized(s, &a[0]), _ => a[0].clone() };
        let v = ifs_values(c, &target, &[a[0].clone(), a[1].clone()])?;
        if v.is_empty() { Err(XlErr::Div0) } else { ok(v.iter().sum::<f64>() / v.len() as f64) }
    });
    r.a("AVERAGEIFS", 3, MANY, "average_range, criteria_range1, criteria1, ...", "Average of the cells that meet all conditions", |c, a| {
        let v = ifs_values(c, &a[0], &a[1..])?;
        if v.is_empty() { Err(XlErr::Div0) } else { ok(v.iter().sum::<f64>() / v.len() as f64) }
    });
    r.a("MAXIFS", 3, MANY, "max_range, criteria_range1, criteria1, ...", "Largest value among cells meeting conditions", |c, a| {
        let v = ifs_values(c, &a[0], &a[1..])?;
        Ok(Value::Num(if v.is_empty() { 0.0 } else { v.iter().cloned().fold(f64::MIN, f64::max) }))
    });
    r.a("MINIFS", 3, MANY, "min_range, criteria_range1, criteria1, ...", "Smallest value among cells meeting conditions", |c, a| {
        let v = ifs_values(c, &a[0], &a[1..])?;
        Ok(Value::Num(if v.is_empty() { 0.0 } else { v.iter().cloned().fold(f64::MAX, f64::min) }))
    });
    r.a("COUNTIF", 2, 2, "range, criteria", "Counts cells that meet a condition", |c, a| {
        let arr = c.arr(&a[0]);
        Ok(Value::Num(crit_mask(c, a, arr.rows, arr.cols)?.iter().filter(|x| **x).count() as f64))
    });
    r.a("COUNTIFS", 2, MANY, "criteria_range1, criteria1, ...", "Counts cells that meet all conditions", |c, a| {
        let arr = c.arr(&a[0]);
        Ok(Value::Num(crit_mask(c, a, arr.rows, arr.cols)?.iter().filter(|x| **x).count() as f64))
    });
    r.a("SUMPRODUCT", 1, MANY, "array1, [array2], ...", "Sum of the products of matching items", |c, a| {
        let arrs: Vec<_> = a.iter().map(|x| c.arr(x)).collect();
        let (rows, cols) = (arrs[0].rows, arrs[0].cols);
        if arrs.iter().any(|x| x.rows != rows || x.cols != cols) { return Err(XlErr::Value); }
        let mut sum = 0.0;
        for i in 0..rows * cols {
            let mut p = 1.0;
            for x in &arrs {
                match &x.data[i] {
                    Value::Num(n) => p *= n,
                    Value::Bool(_) | Value::Text(_) | Value::Empty => p = 0.0,
                    Value::Err(e) => return Err(*e),
                    _ => {}
                }
            }
            sum += p;
        }
        ok(sum)
    });
    r.a("SUMX2MY2", 2, 2, "array_x, array_y", "Sum of the differences of squares", |c, a| { let (x, y) = pairs(c, &a[0], &a[1])?; ok(x.iter().zip(&y).map(|(p, q)| p * p - q * q).sum()) });
    r.a("SUMX2PY2", 2, 2, "array_x, array_y", "Sum of the sums of squares", |c, a| { let (x, y) = pairs(c, &a[0], &a[1])?; ok(x.iter().zip(&y).map(|(p, q)| p * p + q * q).sum()) });
    r.a("SUMXMY2", 2, 2, "array_x, array_y", "Sum of squares of differences", |c, a| { let (x, y) = pairs(c, &a[0], &a[1])?; ok(x.iter().zip(&y).map(|(p, q)| (p - q).powi(2)).sum()) });
    r.a("SERIESSUM", 4, 4, "x, n, m, coefficients", "Sum of a power series", |c, a| {
        let (x, n0, m) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        let co = c.nums(&a[3..4])?;
        ok(co.iter().enumerate().map(|(i, k)| k * x.powf(n0 + i as f64 * m)).sum())
    });
    r.a("MULTINOMIAL", 1, MANY, "number1, [number2], ...", "Multinomial of a set of numbers", |c, a| {
        let v = c.nums(a)?;
        let s: f64 = v.iter().sum();
        ok(fact(s) / v.iter().map(|x| fact(*x)).product::<f64>())
    });
    r.a("MDETERM", 1, 1, "array", "Determinant of a matrix", |c, a| {
        let m = mat(c, &a[0])?;
        if m.iter().any(|r| r.len() != m.len()) { return Err(XlErr::Value); }
        ok(mdet(&m))
    });
    r.a("MINVERSE", 1, 1, "array", "Inverse of a matrix", |c, a| {
        let m = mat(c, &a[0])?;
        if m.iter().any(|r| r.len() != m.len()) { return Err(XlErr::Value); }
        minverse(&m).map(|x| mat_val(&x)).ok_or(XlErr::Num)
    });
    r.a("MMULT", 2, 2, "array1, array2", "Matrix product of two arrays", |c, a| {
        let (x, y) = (mat(c, &a[0])?, mat(c, &a[1])?);
        if x[0].len() != y.len() { return Err(XlErr::Value); }
        let m: Vec<Vec<f64>> = x.iter().map(|row| (0..y[0].len()).map(|j| row.iter().enumerate().map(|(k, v)| v * y[k][j]).sum()).collect()).collect();
        Ok(mat_val(&m))
    });
    r.a("MUNIT", 1, 1, "dimension", "Identity matrix", |_, a| {
        let k = n(a, 0)? as usize;
        if k == 0 { return Err(XlErr::Value); }
        Ok(mat_val(&(0..k).map(|i| (0..k).map(|j| (i == j) as i32 as f64).collect()).collect::<Vec<_>>()))
    });
    r.a("RAND", 0, 0, "", "Random number between 0 and 1", |_, _| Ok(Value::Num(rand01())));
    r.s("RANDBETWEEN", 2, 2, "bottom, top", "Random integer between two numbers", |_, a| {
        let (lo, hi) = (n(a, 0)?.ceil(), n(a, 1)?.floor());
        if lo > hi { return Err(XlErr::Num); }
        ok(lo + (rand01() * (hi - lo + 1.0)).floor())
    });
    r.a("RANDARRAY", 0, 5, "[rows], [columns], [min], [max], [whole_number]", "Array of random numbers", |_, a| {
        let (rows, cols) = (nd(a, 0, 1.0)? as usize, nd(a, 1, 1.0)? as usize);
        let (lo, hi, whole) = (nd(a, 2, 0.0)?, nd(a, 3, 1.0)?, bd(a, 4, false)?);
        let data = (0..rows * cols).map(|_| Value::Num(if whole { lo + (rand01() * (hi - lo + 1.0)).floor() } else { lo + rand01() * (hi - lo) })).collect();
        Ok(Array::new(rows, cols, data))
    });
    r.a("SEQUENCE", 1, 4, "rows, [columns], [start], [step]", "Array of sequential numbers", |_, a| {
        let (rows, cols) = (n(a, 0)? as usize, nd(a, 1, 1.0)? as usize);
        let (st, step) = (nd(a, 2, 1.0)?, nd(a, 3, 1.0)?);
        if rows * cols > 5_000_000 { return Err(XlErr::Num); }
        Ok(Array::new(rows, cols, (0..rows * cols).map(|i| Value::Num(st + i as f64 * step)).collect()))
    });
    r.s("ROMAN", 1, 2, "number, [form]", "Converts to a Roman numeral", |_, a| {
        let x = n(a, 0)?.trunc();
        if !(0.0..=3999.0).contains(&x) { return Err(XlErr::Value); }
        txt(roman(x as i64))
    });
    r.s("ARABIC", 1, 1, "text", "Converts a Roman numeral to a number", |_, a| from_roman(&t(a, 0)?).map(Value::Num).ok_or(XlErr::Value));
    r.s("BASE", 2, 3, "number, radix, [min_length]", "Converts a number to text in a base", |_, a| {
        let (mut x, b, w) = (n(a, 0)? as u64, n(a, 1)? as u64, nd(a, 2, 0.0)? as usize);
        if !(2..=36).contains(&b) { return Err(XlErr::Num); }
        let mut s = vec![];
        while x > 0 { s.push(std::char::from_digit((x % b) as u32, b as u32).unwrap().to_ascii_uppercase()); x /= b; }
        if s.is_empty() { s.push('0'); }
        s.reverse();
        let s: String = s.into_iter().collect();
        txt(format!("{:0>w$}", s, w = w))
    });
    r.s("DECIMAL", 2, 2, "text, radix", "Converts text in a base to a number", |_, a| {
        let b = n(a, 1)? as u32;
        if !(2..=36).contains(&b) { return Err(XlErr::Num); }
        u64::from_str_radix(t(a, 0)?.trim(), b).map(|x| Value::Num(x as f64)).map_err(|_| XlErr::Num)
    });
    r.a("SUBTOTAL", 2, MANY, "function_num, ref1, [ref2], ...", "Subtotal that respects filtered rows", |c, a| {
        let code = n(a, 0)? as i64;
        let (code, skip) = if code > 100 { (code - 100, true) } else { (code, false) };
        subtotal(c, code, &a[1..], skip)
    });
    r.a("AGGREGATE", 3, MANY, "function_num, options, ref1, ...", "Aggregate that can ignore errors and hidden rows", |c, a| {
        let code = n(a, 0)? as i64;
        let opt = n(a, 1)? as i64;
        let ignore_err = matches!(opt, 2 | 3 | 6 | 7);
        let flat: Vec<Value> = c.flat_nonblank(&a[2]);
        let mut v = vec![];
        let mut count = 0;
        for x in flat {
            match x {
                Value::Num(n) => { v.push(n); count += 1; }
                Value::Err(e) => { if !ignore_err { return Err(e); } }
                _ => count += 1,
            }
        }
        match code {
            1..=12 => aggregate_by(code, &v, count),
            13 => crate::funcs_stat::mode(&v),
            14 | 15 => {
                let k = n(a, 3)? as usize;
                let mut s = v.clone();
                s.sort_by(|p, q| p.partial_cmp(q).unwrap());
                if code == 14 { s.reverse(); }
                s.get(k.wrapping_sub(1)).map(|x| Value::Num(*x)).ok_or(XlErr::Num)
            }
            16 | 18 => crate::funcs_stat::percentile(&v, n(a, 3)?, code == 18).map(Value::Num),
            17 | 19 => crate::funcs_stat::percentile(&v, n(a, 3)? / 4.0, code == 19).map(Value::Num),
            _ => Err(XlErr::Value),
        }
    });
}
