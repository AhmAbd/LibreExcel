//! Statistical functions and the probability distributions behind them.

use crate::eval::*;
use crate::funcs::*;
use crate::funcs_math::{combin, fact, mat_val, minverse};
use crate::model::*;

pub fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}
pub fn sq_dev(v: &[f64]) -> f64 {
    let m = mean(v);
    v.iter().map(|x| (x - m).powi(2)).sum()
}
pub fn variance(v: &[f64], sample: bool) -> R {
    let d = if sample { v.len() as f64 - 1.0 } else { v.len() as f64 };
    if d <= 0.0 {
        return Err(XlErr::Div0);
    }
    ok(sq_dev(v) / d)
}
pub fn stdev(v: &[f64], sample: bool) -> R {
    match variance(v, sample)? {
        Value::Num(x) => ok(x.sqrt()),
        o => Ok(o),
    }
}
pub fn median(v: &[f64]) -> R {
    if v.is_empty() {
        return Err(XlErr::Num);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    ok(if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 })
}
pub fn mode(v: &[f64]) -> R {
    let list = modes(v);
    list.first().map(|x| Value::Num(*x)).ok_or(XlErr::NA)
}
fn modes(v: &[f64]) -> Vec<f64> {
    let mut seen: Vec<(f64, usize)> = vec![];
    for x in v {
        match seen.iter_mut().find(|s| s.0 == *x) {
            Some(s) => s.1 += 1,
            None => seen.push((*x, 1)),
        }
    }
    let best = seen.iter().map(|x| x.1).max().unwrap_or(0);
    if best < 2 {
        return vec![];
    }
    seen.into_iter().filter(|x| x.1 == best).map(|x| x.0).collect()
}
pub fn percentile(v: &[f64], k: f64, exc: bool) -> Result<f64, XlErr> {
    if v.is_empty() || !(0.0..=1.0).contains(&k) {
        return Err(XlErr::Num);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len() as f64;
    let p = if exc {
        let p = k * (n + 1.0) - 1.0;
        if p < 0.0 || p > n - 1.0 {
            return Err(XlErr::Num);
        }
        p
    } else {
        k * (n - 1.0)
    };
    let i = p.floor() as usize;
    let f = p - p.floor();
    Ok(if i + 1 < s.len() { s[i] + f * (s[i + 1] - s[i]) } else { s[i] })
}
fn percentrank(v: &[f64], x: f64, sig: i32, exc: bool) -> R {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    if s.is_empty() || x < s[0] || x > s[s.len() - 1] {
        return Err(XlErr::NA);
    }
    let n = s.len() as f64;
    let rank = |y: f64| s.iter().filter(|q| **q < y).count() as f64;
    let val = if let Some(_) = s.iter().find(|q| **q == x) {
        if exc {
            (rank(x) + 1.0) / (n + 1.0)
        } else {
            rank(x) / (n - 1.0)
        }
    } else {
        let lo = s.iter().filter(|q| **q < x).cloned().fold(f64::MIN, f64::max);
        let hi = s.iter().filter(|q| **q > x).cloned().fold(f64::MAX, f64::min);
        let (a, b) = if exc { ((rank(lo) + 1.0) / (n + 1.0), (rank(hi) + 1.0) / (n + 1.0)) } else { (rank(lo) / (n - 1.0), rank(hi) / (n - 1.0)) };
        a + (x - lo) / (hi - lo) * (b - a)
    };
    let m = 10f64.powi(sig);
    ok((val * m).floor() / m)
}

pub fn ln_gamma(x: f64) -> f64 {
    let c = [
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ];
    let mut y = x;
    let t = x + 5.5;
    let t = t - (x + 0.5) * t.ln();
    let mut ser = 1.000000000190015;
    for k in c {
        y += 1.0;
        ser += k / y;
    }
    -t + (2.5066282746310005 * ser / x).ln()
}

/// Regularized lower incomplete gamma P(a, x).
pub fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        let mut ap = a;
        let mut sum = 1.0 / a;
        let mut del = sum;
        for _ in 0..1000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        sum * (-x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        let mut b = x + 1.0 - a;
        let mut c = 1e300;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < 1e-300 {
                d = 1e-300;
            }
            c = b + an / c;
            if c.abs() < 1e-300 {
                c = 1e-300;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-15 {
                break;
            }
        }
        1.0 - (-x + a * x.ln() - ln_gamma(a)).exp() * h
    }
}

fn betacf(a: f64, b: f64, x: f64) -> f64 {
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < 1e-300 {
        d = 1e-300;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..1000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-300 {
            d = 1e-300;
        }
        c = 1.0 + aa / c;
        if c.abs() < 1e-300 {
            c = 1e-300;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-300 {
            d = 1e-300;
        }
        c = 1.0 + aa / c;
        if c.abs() < 1e-300 {
            c = 1e-300;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// Regularized incomplete beta I_x(a, b).
pub fn beta_i(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(a, b, x) / a
    } else {
        1.0 - bt * betacf(b, a, 1.0 - x) / b
    }
}

pub fn erf(x: f64) -> f64 {
    let p = gamma_p(0.5, x * x);
    if x >= 0.0 {
        p
    } else {
        -p
    }
}

pub fn norm_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}
fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}
pub fn norm_inv(p: f64) -> f64 {
    // Acklam's rational approximation, polished with one Newton step.
    let a = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
    let b = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
    let c = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    let d = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
    let pl = 0.02425;
    let x = if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    };
    let e = norm_cdf(x) - p;
    x - e / norm_pdf(x)
}

/// Finds x with cdf(x) = p by bisection on [lo, hi].
fn invert(p: f64, mut lo: f64, mut hi: f64, cdf: impl Fn(f64) -> f64) -> f64 {
    while cdf(hi) < p && hi < 1e300 {
        hi *= 2.0;
    }
    for _ in 0..300 {
        let m = 0.5 * (lo + hi);
        if cdf(m) < p {
            lo = m;
        } else {
            hi = m;
        }
        if (hi - lo).abs() < 1e-13 * m.abs().max(1e-13) {
            break;
        }
    }
    0.5 * (lo + hi)
}

fn t_cdf(t0: f64, df: f64) -> f64 {
    let x = df / (df + t0 * t0);
    let p = 0.5 * beta_i(df / 2.0, 0.5, x);
    if t0 > 0.0 {
        1.0 - p
    } else {
        p
    }
}
fn t_pdf(x: f64, df: f64) -> f64 {
    (ln_gamma((df + 1.0) / 2.0) - ln_gamma(df / 2.0)).exp() / (df * std::f64::consts::PI).sqrt() * (1.0 + x * x / df).powf(-(df + 1.0) / 2.0)
}
fn t_inv(p: f64, df: f64) -> f64 {
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -t_inv(1.0 - p, df);
    }
    invert(p, 0.0, 16.0, |x| t_cdf(x, df))
}
fn chi_cdf(x: f64, k: f64) -> f64 {
    gamma_p(k / 2.0, x / 2.0)
}
fn f_cdf(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        beta_i(a / 2.0, b / 2.0, a * x / (a * x + b))
    }
}

fn binom_pmf(k: f64, n0: f64, p: f64) -> f64 {
    if p == 0.0 {
        return if k == 0.0 { 1.0 } else { 0.0 };
    }
    if p == 1.0 {
        return if k == n0 { 1.0 } else { 0.0 };
    }
    (ln_gamma(n0 + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n0 - k + 1.0) + k * p.ln() + (n0 - k) * (1.0 - p).ln()).exp()
}

/// Least-squares straight line through the pairs: (slope, intercept).
pub fn linfit(x: &[f64], y: &[f64]) -> Result<(f64, f64), XlErr> {
    if x.len() < 2 {
        return Err(XlErr::Div0);
    }
    let (mx, my) = (mean(x), mean(y));
    let sxx: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    if sxx == 0.0 {
        return Err(XlErr::Div0);
    }
    let b = sxy / sxx;
    Ok((b, my - b * mx))
}

fn correl(x: &[f64], y: &[f64]) -> Result<f64, XlErr> {
    if x.is_empty() {
        return Err(XlErr::Div0);
    }
    let (mx, my) = (mean(x), mean(y));
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let d = (sq_dev(x) * sq_dev(y)).sqrt();
    if d == 0.0 {
        return Err(XlErr::Div0);
    }
    Ok(sxy / d)
}

fn nth(c: &Cx, a: &[Value], largest: bool) -> R {
    let mut v = c.nums(&a[..1])?;
    let k = n(a, 1)?.ceil() as usize;
    if k == 0 || k > v.len() {
        return Err(XlErr::Num);
    }
    v.sort_by(|p, q| p.total_cmp(q));
    Ok(Value::Num(if largest { v[v.len() - k] } else { v[k - 1] }))
}

fn rank(a: &[Value], c: &Cx, avg: bool) -> R {
    let x = n(a, 0)?;
    let v = c.nums(&a[1..2])?;
    let asc = nd(a, 2, 0.0)? != 0.0;
    if !v.contains(&x) {
        return Err(XlErr::NA);
    }
    let better = v.iter().filter(|q| if asc { **q < x } else { **q > x }).count() as f64;
    let same = v.iter().filter(|q| **q == x).count() as f64;
    Ok(Value::Num(better + 1.0 + if avg { (same - 1.0) / 2.0 } else { 0.0 }))
}

fn ttest(a: &[Value], c: &Cx) -> R {
    let tails = n(a, 2)?;
    let ty = n(a, 3)?;
    let (t0, df) = if ty == 1.0 {
        let (x, y) = pairs(c, &a[0], &a[1])?;
        let d: Vec<f64> = x.iter().zip(&y).map(|(p, q)| p - q).collect();
        let s = match stdev(&d, true)? { Value::Num(s) => s, _ => 0.0 };
        (mean(&d) / (s / (d.len() as f64).sqrt()), d.len() as f64 - 1.0)
    } else {
        let x = c.nums(&a[..1])?;
        let y = c.nums(&a[1..2])?;
        let (nx, ny) = (x.len() as f64, y.len() as f64);
        let (vx, vy) = (sq_dev(&x) / (nx - 1.0), sq_dev(&y) / (ny - 1.0));
        if ty == 2.0 {
            let sp = (((nx - 1.0) * vx + (ny - 1.0) * vy) / (nx + ny - 2.0)).sqrt();
            ((mean(&x) - mean(&y)) / (sp * (1.0 / nx + 1.0 / ny).sqrt()), nx + ny - 2.0)
        } else {
            let (p, q) = (vx / nx, vy / ny);
            ((mean(&x) - mean(&y)) / (p + q).sqrt(), (p + q).powi(2) / (p * p / (nx - 1.0) + q * q / (ny - 1.0)))
        }
    };
    let tail = 1.0 - t_cdf(t0.abs(), df);
    ok(if tails == 1.0 { tail } else { 2.0 * tail })
}

/// Ordinary least squares with an optional intercept. Returns coefficients with the intercept last.
fn ols(ys: &[f64], xs: &[Vec<f64>], constant: bool) -> Option<Vec<f64>> {
    let rows = ys.len();
    let k = xs.first()?.len() + constant as usize;
    let design: Vec<Vec<f64>> = xs
        .iter()
        .map(|r| {
            let mut x = r.clone();
            if constant {
                x.push(1.0);
            }
            x
        })
        .collect();
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for i in 0..rows {
        for p in 0..k {
            xty[p] += design[i][p] * ys[i];
            for q in 0..k {
                xtx[p][q] += design[i][p] * design[i][q];
            }
        }
    }
    let inv = minverse(&xtx)?;
    Some((0..k).map(|p| (0..k).map(|q| inv[p][q] * xty[q]).sum()).collect())
}

/// Reads known_y and known_x into rows of predictors.
fn regression_input(c: &Cx, a: &[Value]) -> Result<(Vec<f64>, Vec<Vec<f64>>), XlErr> {
    let y = c.arr(&a[0]);
    let ys: Vec<f64> = y.data.iter().map(to_num).collect::<Result<_, _>>()?;
    let xs: Vec<Vec<f64>> = match a.get(1) {
        None | Some(Value::Empty) => (1..=ys.len()).map(|i| vec![i as f64]).collect(),
        Some(v) => {
            let x = c.arr(v);
            if x.rows == ys.len() {
                (0..x.rows).map(|r| x.row_of(r).iter().map(to_num).collect::<Result<Vec<_>, _>>()).collect::<Result<_, _>>()?
            } else if x.cols == ys.len() {
                (0..x.cols).map(|k| x.col_of(k).iter().map(to_num).collect::<Result<Vec<_>, _>>()).collect::<Result<_, _>>()?
            } else {
                return Err(XlErr::Ref);
            }
        }
    };
    Ok((ys, xs))
}

pub fn stats(r: &mut Reg) {
    r.cat = "Statistical";
    r.a("AVERAGE", 1, MANY, "number1, [number2], ...", "Average of the arguments", |c, a| { let v = c.nums(a)?; if v.is_empty() { Err(XlErr::Div0) } else { ok(mean(&v)) } });
    r.a("AVERAGEA", 1, MANY, "value1, [value2], ...", "Average, counting text as 0", |c, a| { let v = c.nums_a(a)?; if v.is_empty() { Err(XlErr::Div0) } else { ok(mean(&v)) } });
    r.a("COUNT", 1, MANY, "value1, [value2], ...", "Counts cells that contain numbers", |c, a| {
        let mut k = 0;
        for v in a {
            match v {
                Value::Ref(..) | Value::Array(_) => k += c.flat_nonblank(v).iter().filter(|x| matches!(x, Value::Num(_))).count(),
                Value::Empty => {}
                x => if to_num(x).is_ok() { k += 1 },
            }
        }
        Ok(Value::Num(k as f64))
    });
    r.a("COUNTA", 1, MANY, "value1, [value2], ...", "Counts cells that are not empty", |c, a| Ok(Value::Num(a.iter().map(|v| c.flat_nonblank(v).len()).sum::<usize>() as f64)));
    r.a("COUNTBLANK", 1, 1, "range", "Counts empty cells", |c, a| {
        Ok(Value::Num(match &a[0] {
            Value::Ref(_, rc) => {
                let tot = rc.rows() as f64 * rc.cols() as f64;
                let filled = c.flat_nonblank(&a[0]).iter().filter(|x| !matches!(x, Value::Text(t) if t.is_empty())).count();
                tot - filled as f64
            }
            v => c.flat(v).iter().filter(|x| matches!(x, Value::Empty)).count() as f64,
        }))
    });
    r.a("MAX", 1, MANY, "number1, [number2], ...", "Largest value", |c, a| { let v = c.nums(a)?; Ok(Value::Num(if v.is_empty() { 0.0 } else { v.iter().cloned().fold(f64::MIN, f64::max) })) });
    r.a("MIN", 1, MANY, "number1, [number2], ...", "Smallest value", |c, a| { let v = c.nums(a)?; Ok(Value::Num(if v.is_empty() { 0.0 } else { v.iter().cloned().fold(f64::MAX, f64::min) })) });
    r.a("MAXA", 1, MANY, "value1, [value2], ...", "Largest value, counting text as 0", |c, a| { let v = c.nums_a(a)?; Ok(Value::Num(if v.is_empty() { 0.0 } else { v.iter().cloned().fold(f64::MIN, f64::max) })) });
    r.a("MINA", 1, MANY, "value1, [value2], ...", "Smallest value, counting text as 0", |c, a| { let v = c.nums_a(a)?; Ok(Value::Num(if v.is_empty() { 0.0 } else { v.iter().cloned().fold(f64::MAX, f64::min) })) });
    r.a("MEDIAN", 1, MANY, "number1, [number2], ...", "Middle value", |c, a| median(&c.nums(a)?));
    r.a("MODE", 1, MANY, "number1, [number2], ...", "Most frequent value", |c, a| mode(&c.nums(a)?));
    r.alias("MODE.SNGL", "MODE");
    r.a("MODE.MULT", 1, MANY, "number1, [number2], ...", "All the most frequent values", |c, a| {
        let m = modes(&c.nums(a)?);
        if m.is_empty() { return Err(XlErr::NA); }
        let l = m.len();
        Ok(Array::new(l, 1, m.into_iter().map(Value::Num).collect()))
    });
    r.a("STDEV", 1, MANY, "number1, [number2], ...", "Standard deviation of a sample", |c, a| stdev(&c.nums(a)?, true));
    r.alias("STDEV.S", "STDEV");
    r.a("STDEVP", 1, MANY, "number1, [number2], ...", "Standard deviation of a population", |c, a| stdev(&c.nums(a)?, false));
    r.alias("STDEV.P", "STDEVP");
    r.a("STDEVA", 1, MANY, "value1, [value2], ...", "Sample standard deviation, counting text as 0", |c, a| stdev(&c.nums_a(a)?, true));
    r.a("STDEVPA", 1, MANY, "value1, [value2], ...", "Population standard deviation, counting text as 0", |c, a| stdev(&c.nums_a(a)?, false));
    r.a("VAR", 1, MANY, "number1, [number2], ...", "Variance of a sample", |c, a| variance(&c.nums(a)?, true));
    r.alias("VAR.S", "VAR");
    r.a("VARP", 1, MANY, "number1, [number2], ...", "Variance of a population", |c, a| variance(&c.nums(a)?, false));
    r.alias("VAR.P", "VARP");
    r.a("VARA", 1, MANY, "value1, [value2], ...", "Sample variance, counting text as 0", |c, a| variance(&c.nums_a(a)?, true));
    r.a("VARPA", 1, MANY, "value1, [value2], ...", "Population variance, counting text as 0", |c, a| variance(&c.nums_a(a)?, false));
    r.a("DEVSQ", 1, MANY, "number1, [number2], ...", "Sum of squared deviations", |c, a| ok(sq_dev(&c.nums(a)?)));
    r.a("AVEDEV", 1, MANY, "number1, [number2], ...", "Average absolute deviation", |c, a| { let v = c.nums(a)?; let m = mean(&v); ok(v.iter().map(|x| (x - m).abs()).sum::<f64>() / v.len() as f64) });
    r.a("GEOMEAN", 1, MANY, "number1, [number2], ...", "Geometric mean", |c, a| {
        let v = c.nums(a)?;
        if v.is_empty() || v.iter().any(|x| *x <= 0.0) { return Err(XlErr::Num); }
        ok((v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp())
    });
    r.a("HARMEAN", 1, MANY, "number1, [number2], ...", "Harmonic mean", |c, a| {
        let v = c.nums(a)?;
        if v.is_empty() || v.iter().any(|x| *x <= 0.0) { return Err(XlErr::Num); }
        ok(v.len() as f64 / v.iter().map(|x| 1.0 / x).sum::<f64>())
    });
    r.a("TRIMMEAN", 2, 2, "array, percent", "Mean of the interior of a data set", |c, a| {
        let mut v = c.nums(&a[..1])?;
        let p = n(a, 1)?;
        if !(0.0..1.0).contains(&p) { return Err(XlErr::Num); }
        v.sort_by(|x, y| x.total_cmp(y));
        let k = ((v.len() as f64 * p) / 2.0).floor() as usize;
        ok(mean(&v[k..v.len() - k]))
    });
    r.a("KURT", 1, MANY, "number1, [number2], ...", "Kurtosis of a data set", |c, a| {
        let v = c.nums(a)?;
        let k = v.len() as f64;
        if k < 4.0 { return Err(XlErr::Div0); }
        let (m, s) = (mean(&v), (sq_dev(&v) / (k - 1.0)).sqrt());
        if s == 0.0 { return Err(XlErr::Div0); }
        let q: f64 = v.iter().map(|x| ((x - m) / s).powi(4)).sum();
        ok(k * (k + 1.0) / ((k - 1.0) * (k - 2.0) * (k - 3.0)) * q - 3.0 * (k - 1.0).powi(2) / ((k - 2.0) * (k - 3.0)))
    });
    r.a("SKEW", 1, MANY, "number1, [number2], ...", "Skewness of a sample", |c, a| {
        let v = c.nums(a)?;
        let k = v.len() as f64;
        if k < 3.0 { return Err(XlErr::Div0); }
        let (m, s) = (mean(&v), (sq_dev(&v) / (k - 1.0)).sqrt());
        if s == 0.0 { return Err(XlErr::Div0); }
        ok(k / ((k - 1.0) * (k - 2.0)) * v.iter().map(|x| ((x - m) / s).powi(3)).sum::<f64>())
    });
    r.a("SKEW.P", 1, MANY, "number1, [number2], ...", "Skewness of a population", |c, a| {
        let v = c.nums(a)?;
        let k = v.len() as f64;
        let (m, s) = (mean(&v), (sq_dev(&v) / k).sqrt());
        if s == 0.0 { return Err(XlErr::Div0); }
        ok(v.iter().map(|x| ((x - m) / s).powi(3)).sum::<f64>() / k)
    });
    r.a("LARGE", 2, 2, "array, k", "The k-th largest value", |c, a| nth(c, a, true));
    r.a("SMALL", 2, 2, "array, k", "The k-th smallest value", |c, a| nth(c, a, false));
    r.a("PERCENTILE", 2, 2, "array, k", "The k-th percentile", |c, a| percentile(&c.nums(&a[..1])?, n(a, 1)?, false).map(Value::Num));
    r.alias("PERCENTILE.INC", "PERCENTILE");
    r.a("PERCENTILE.EXC", 2, 2, "array, k", "The k-th percentile, exclusive", |c, a| percentile(&c.nums(&a[..1])?, n(a, 1)?, true).map(Value::Num));
    r.a("QUARTILE", 2, 2, "array, quart", "A quartile of a data set", |c, a| percentile(&c.nums(&a[..1])?, n(a, 1)?.trunc() / 4.0, false).map(Value::Num));
    r.alias("QUARTILE.INC", "QUARTILE");
    r.a("QUARTILE.EXC", 2, 2, "array, quart", "A quartile, exclusive", |c, a| percentile(&c.nums(&a[..1])?, n(a, 1)?.trunc() / 4.0, true).map(Value::Num));
    r.a("PERCENTRANK", 2, 3, "array, x, [significance]", "Rank of a value as a percentage", |c, a| percentrank(&c.nums(&a[..1])?, n(a, 1)?, nd(a, 2, 3.0)? as i32, false));
    r.alias("PERCENTRANK.INC", "PERCENTRANK");
    r.a("PERCENTRANK.EXC", 2, 3, "array, x, [significance]", "Rank of a value as a percentage, exclusive", |c, a| percentrank(&c.nums(&a[..1])?, n(a, 1)?, nd(a, 2, 3.0)? as i32, true));
    r.a("RANK", 2, 3, "number, ref, [order]", "Rank of a number in a list", |c, a| rank(a, c, false));
    r.alias("RANK.EQ", "RANK");
    r.a("RANK.AVG", 2, 3, "number, ref, [order]", "Rank, with ties averaged", |c, a| rank(a, c, true));
    r.a("CORREL", 2, 2, "array1, array2", "Correlation coefficient", |c, a| { let (x, y) = pairs(c, &a[0], &a[1])?; correl(&x, &y).map(Value::Num) });
    r.alias("PEARSON", "CORREL");
    r.a("RSQ", 2, 2, "known_y's, known_x's", "Square of the correlation", |c, a| { let (y, x) = pairs(c, &a[0], &a[1])?; ok(correl(&x, &y)?.powi(2)) });
    r.a("COVAR", 2, 2, "array1, array2", "Population covariance", |c, a| {
        let (x, y) = pairs(c, &a[0], &a[1])?;
        if x.is_empty() { return Err(XlErr::Div0); }
        let (p, q) = (mean(&x), mean(&y));
        ok(x.iter().zip(&y).map(|(u, v)| (u - p) * (v - q)).sum::<f64>() / x.len() as f64)
    });
    r.alias("COVARIANCE.P", "COVAR");
    r.a("COVARIANCE.S", 2, 2, "array1, array2", "Sample covariance", |c, a| {
        let (x, y) = pairs(c, &a[0], &a[1])?;
        if x.len() < 2 { return Err(XlErr::Div0); }
        let (p, q) = (mean(&x), mean(&y));
        ok(x.iter().zip(&y).map(|(u, v)| (u - p) * (v - q)).sum::<f64>() / (x.len() as f64 - 1.0))
    });
    r.a("SLOPE", 2, 2, "known_y's, known_x's", "Slope of the regression line", |c, a| { let (y, x) = pairs(c, &a[0], &a[1])?; ok(linfit(&x, &y)?.0) });
    r.a("INTERCEPT", 2, 2, "known_y's, known_x's", "Intercept of the regression line", |c, a| { let (y, x) = pairs(c, &a[0], &a[1])?; ok(linfit(&x, &y)?.1) });
    r.a("STEYX", 2, 2, "known_y's, known_x's", "Standard error of the regression", |c, a| {
        let (y, x) = pairs(c, &a[0], &a[1])?;
        let (b, i) = linfit(&x, &y)?;
        if x.len() < 3 { return Err(XlErr::Div0); }
        ok((x.iter().zip(&y).map(|(p, q)| (q - b * p - i).powi(2)).sum::<f64>() / (x.len() as f64 - 2.0)).sqrt())
    });
    r.a("FORECAST", 3, 3, "x, known_y's, known_x's", "Value along a linear trend", |c, a| { let (y, x) = pairs(c, &a[1], &a[2])?; let (b, i) = linfit(&x, &y)?; ok(i + b * n(a, 0)?) });
    r.alias("FORECAST.LINEAR", "FORECAST");
    r.a("TREND", 1, 4, "known_y's, [known_x's], [new_x's], [const]", "Values along a linear trend", |c, a| {
        let (ys, xs) = regression_input(c, a)?;
        let con = bd(a, 3, true)?;
        let co = ols(&ys, &xs, con).ok_or(XlErr::Num)?;
        let new: Vec<Vec<f64>> = match a.get(2) {
            None | Some(Value::Empty) => xs,
            Some(v) => { let x = c.arr(v); (0..x.rows).map(|r| x.row_of(r).iter().map(to_num).collect::<Result<Vec<_>, _>>()).collect::<Result<_, _>>()? }
        };
        let k = xs_len(&new);
        let out: Vec<Value> = new.iter().map(|r| Value::Num(r.iter().zip(&co).map(|(p, q)| p * q).sum::<f64>() + if con { co[k] } else { 0.0 })).collect();
        Ok(Array::new(out.len(), 1, out))
    });
    r.a("GROWTH", 1, 4, "known_y's, [known_x's], [new_x's], [const]", "Values along an exponential trend", |c, a| {
        let (ys, xs) = regression_input(c, a)?;
        if ys.iter().any(|y| *y <= 0.0) { return Err(XlErr::Num); }
        let logs: Vec<f64> = ys.iter().map(|y| y.ln()).collect();
        let con = bd(a, 3, true)?;
        let co = ols(&logs, &xs, con).ok_or(XlErr::Num)?;
        let new: Vec<Vec<f64>> = match a.get(2) {
            None | Some(Value::Empty) => xs,
            Some(v) => { let x = c.arr(v); (0..x.rows).map(|r| x.row_of(r).iter().map(to_num).collect::<Result<Vec<_>, _>>()).collect::<Result<_, _>>()? }
        };
        let k = xs_len(&new);
        let out: Vec<Value> = new.iter().map(|r| Value::Num((r.iter().zip(&co).map(|(p, q)| p * q).sum::<f64>() + if con { co[k] } else { 0.0 }).exp())).collect();
        Ok(Array::new(out.len(), 1, out))
    });
    r.a("LINEST", 1, 4, "known_y's, [known_x's], [const], [stats]", "Line of best fit coefficients", |c, a| {
        let (ys, xs) = regression_input(c, a)?;
        let con = bd(a, 2, true)?;
        let mut co = ols(&ys, &xs, con).ok_or(XlErr::Num)?;
        if !con { co.push(0.0); }
        let k = co.len();
        let (mut p, ic) = (co[..k - 1].to_vec(), co[k - 1]);
        p.reverse();
        p.push(ic);
        Ok(Array::new(1, k, p.into_iter().map(Value::Num).collect()))
    });
    r.a("LOGEST", 1, 4, "known_y's, [known_x's], [const], [stats]", "Exponential curve coefficients", |c, a| {
        let (ys, xs) = regression_input(c, a)?;
        if ys.iter().any(|y| *y <= 0.0) { return Err(XlErr::Num); }
        let logs: Vec<f64> = ys.iter().map(|y| y.ln()).collect();
        let con = bd(a, 2, true)?;
        let mut co = ols(&logs, &xs, con).ok_or(XlErr::Num)?;
        if !con { co.push(0.0); }
        let k = co.len();
        let (mut p, ic) = (co[..k - 1].to_vec(), co[k - 1]);
        p.reverse();
        p.push(ic);
        Ok(Array::new(1, k, p.into_iter().map(|x| Value::Num(x.exp())).collect()))
    });
    r.a("FREQUENCY", 2, 2, "data_array, bins_array", "Frequency distribution", |c, a| {
        let d = c.nums(&a[..1])?;
        let b = c.nums(&a[1..2])?;
        let mut idx: Vec<usize> = (0..b.len()).collect();
        idx.sort_by(|p, q| b[*p].total_cmp(&b[*q]));
        let mut out = vec![0.0; b.len() + 1];
        for x in d {
            let mut placed = false;
            for i in &idx {
                if x <= b[*i] { out[*i] += 1.0; placed = true; break; }
            }
            if !placed { *out.last_mut().unwrap() += 1.0; }
        }
        let l = out.len();
        Ok(Array::new(l, 1, out.into_iter().map(Value::Num).collect()))
    });
    r.a("PROB", 3, 4, "x_range, prob_range, lower_limit, [upper_limit]", "Probability that values fall in a range", |c, a| {
        let (x, p) = pairs(c, &a[0], &a[1])?;
        let lo = n(a, 2)?;
        let hi = nd(a, 3, lo)?;
        ok(x.iter().zip(&p).filter(|(u, _)| **u >= lo && **u <= hi).map(|(_, v)| *v).sum())
    });
    dist(r);
}

fn xs_len(x: &[Vec<f64>]) -> usize {
    x.first().map_or(0, |r| r.len())
}

fn dist(r: &mut Reg) {
    r.s("PERMUT", 2, 2, "number, number_chosen", "Number of permutations", |_, a| {
        let (x, k) = (n(a, 0)?.trunc(), n(a, 1)?.trunc());
        if x < k || k < 0.0 { return Err(XlErr::Num); }
        ok(((x - k + 1.0) as i64..=x as i64).map(|i| i as f64).product())
    });
    r.s("PERMUTATIONA", 2, 2, "number, number_chosen", "Permutations with repetitions", |_, a| ok(n(a, 0)?.trunc().powf(n(a, 1)?.trunc())));
    r.s("FISHER", 1, 1, "x", "Fisher transformation", |_, a| { let x = n(a, 0)?; if x.abs() >= 1.0 { Err(XlErr::Num) } else { ok(x.atanh()) } });
    r.s("FISHERINV", 1, 1, "y", "Inverse Fisher transformation", |_, a| ok(n(a, 0)?.tanh()));
    r.s("GAMMA", 1, 1, "number", "Gamma function", |_, a| {
        let x = n(a, 0)?;
        if x <= 0.0 && x == x.trunc() { return Err(XlErr::Num); }
        if x > 0.0 && x == x.trunc() && x < 171.0 { return ok(fact(x - 1.0)); }
        if x < 0.5 {
            return ok(std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * ln_gamma(1.0 - x).exp()));
        }
        ok(ln_gamma(x).exp())
    });
    r.s("GAMMALN", 1, 1, "x", "Natural log of the gamma function", |_, a| { let x = n(a, 0)?; if x <= 0.0 { Err(XlErr::Num) } else { ok(ln_gamma(x)) } });
    r.alias("GAMMALN.PRECISE", "GAMMALN");
    r.s("PHI", 1, 1, "x", "Standard normal density", |_, a| ok(norm_pdf(n(a, 0)?)));
    r.s("GAUSS", 1, 1, "z", "Probability between the mean and z", |_, a| ok(norm_cdf(n(a, 0)?) - 0.5));
    r.s("STANDARDIZE", 3, 3, "x, mean, standard_dev", "Normalized value", |_, a| {
        let s = n(a, 2)?;
        if s <= 0.0 { return Err(XlErr::Num); }
        ok((n(a, 0)? - n(a, 1)?) / s)
    });
    r.s("NORM.DIST", 4, 4, "x, mean, standard_dev, cumulative", "Normal distribution", |_, a| {
        let (x, m, s) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if s <= 0.0 { return Err(XlErr::Num); }
        let z = (x - m) / s;
        ok(if bd(a, 3, false)? { norm_cdf(z) } else { norm_pdf(z) / s })
    });
    r.alias("NORMDIST", "NORM.DIST");
    r.s("NORM.INV", 3, 3, "probability, mean, standard_dev", "Inverse normal distribution", |_, a| {
        let (p, m, s) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if p <= 0.0 || p >= 1.0 || s <= 0.0 { return Err(XlErr::Num); }
        ok(m + s * norm_inv(p))
    });
    r.alias("NORMINV", "NORM.INV");
    r.s("NORM.S.DIST", 1, 2, "z, [cumulative]", "Standard normal distribution", |_, a| { let z = n(a, 0)?; ok(if bd(a, 1, true)? { norm_cdf(z) } else { norm_pdf(z) }) });
    r.alias("NORMSDIST", "NORM.S.DIST");
    r.s("NORM.S.INV", 1, 1, "probability", "Inverse standard normal distribution", |_, a| { let p = n(a, 0)?; if p <= 0.0 || p >= 1.0 { Err(XlErr::Num) } else { ok(norm_inv(p)) } });
    r.alias("NORMSINV", "NORM.S.INV");
    r.s("LOGNORM.DIST", 4, 4, "x, mean, standard_dev, cumulative", "Lognormal distribution", |_, a| {
        let (x, m, s) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if x <= 0.0 || s <= 0.0 { return Err(XlErr::Num); }
        let z = (x.ln() - m) / s;
        ok(if bd(a, 3, false)? { norm_cdf(z) } else { norm_pdf(z) / (x * s) })
    });
    r.s("LOGNORM.INV", 3, 3, "probability, mean, standard_dev", "Inverse lognormal distribution", |_, a| {
        let (p, m, s) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if p <= 0.0 || p >= 1.0 || s <= 0.0 { return Err(XlErr::Num); }
        ok((m + s * norm_inv(p)).exp())
    });
    r.s("T.DIST", 3, 3, "x, deg_freedom, cumulative", "Student's t distribution", |_, a| {
        let (x, df) = (n(a, 0)?, n(a, 1)?.trunc());
        if df < 1.0 { return Err(XlErr::Num); }
        ok(if bd(a, 2, false)? { t_cdf(x, df) } else { t_pdf(x, df) })
    });
    r.s("T.DIST.RT", 2, 2, "x, deg_freedom", "Right-tailed t distribution", |_, a| ok(1.0 - t_cdf(n(a, 0)?, n(a, 1)?.trunc())));
    r.s("T.DIST.2T", 2, 2, "x, deg_freedom", "Two-tailed t distribution", |_, a| { let x = n(a, 0)?; if x < 0.0 { Err(XlErr::Num) } else { ok(2.0 * (1.0 - t_cdf(x, n(a, 1)?.trunc()))) } });
    r.s("T.INV", 2, 2, "probability, deg_freedom", "Inverse t distribution", |_, a| { let p = n(a, 0)?; if p <= 0.0 || p >= 1.0 { Err(XlErr::Num) } else { ok(t_inv(p, n(a, 1)?.trunc())) } });
    r.s("T.INV.2T", 2, 2, "probability, deg_freedom", "Two-tailed inverse t distribution", |_, a| { let p = n(a, 0)?; if p <= 0.0 || p > 1.0 { Err(XlErr::Num) } else { ok(t_inv(1.0 - p / 2.0, n(a, 1)?.trunc())) } });
    r.alias("TINV", "T.INV.2T");
    r.s("CHISQ.DIST", 3, 3, "x, deg_freedom, cumulative", "Chi-squared distribution", |_, a| {
        let (x, k) = (n(a, 0)?, n(a, 1)?.trunc());
        if x < 0.0 || k < 1.0 { return Err(XlErr::Num); }
        ok(if bd(a, 2, false)? { chi_cdf(x, k) } else { ((k / 2.0 - 1.0) * x.ln() - x / 2.0 - k / 2.0 * 2f64.ln() - ln_gamma(k / 2.0)).exp() })
    });
    r.s("CHISQ.DIST.RT", 2, 2, "x, deg_freedom", "Right-tailed chi-squared distribution", |_, a| ok(1.0 - chi_cdf(n(a, 0)?, n(a, 1)?.trunc())));
    r.alias("CHIDIST", "CHISQ.DIST.RT");
    r.s("CHISQ.INV", 2, 2, "probability, deg_freedom", "Inverse chi-squared distribution", |_, a| { let (p, k) = (n(a, 0)?, n(a, 1)?.trunc()); if !(0.0..1.0).contains(&p) { Err(XlErr::Num) } else { ok(invert(p, 0.0, k.max(1.0) * 4.0, |x| chi_cdf(x, k))) } });
    r.s("CHISQ.INV.RT", 2, 2, "probability, deg_freedom", "Right-tailed inverse chi-squared", |_, a| { let (p, k) = (n(a, 0)?, n(a, 1)?.trunc()); if p <= 0.0 || p > 1.0 { Err(XlErr::Num) } else { ok(invert(1.0 - p, 0.0, k.max(1.0) * 4.0, |x| chi_cdf(x, k))) } });
    r.alias("CHIINV", "CHISQ.INV.RT");
    r.a("CHISQ.TEST", 2, 2, "actual_range, expected_range", "Chi-squared test", |c, a| {
        let (o, e) = (c.arr(&a[0]), c.arr(&a[1]));
        let (x, y) = pairs(c, &a[0], &a[1])?;
        let df = if o.rows > 1 && o.cols > 1 { (o.rows - 1) * (o.cols - 1) } else { (o.rows * o.cols).saturating_sub(1) } as f64;
        let _ = e;
        let s: f64 = x.iter().zip(&y).map(|(p, q)| (p - q).powi(2) / q).sum();
        ok(1.0 - chi_cdf(s, df))
    });
    r.alias("CHITEST", "CHISQ.TEST");
    r.s("F.DIST", 4, 4, "x, deg_freedom1, deg_freedom2, cumulative", "F distribution", |_, a| {
        let (x, p, q) = (n(a, 0)?, n(a, 1)?.trunc(), n(a, 2)?.trunc());
        if x < 0.0 || p < 1.0 || q < 1.0 { return Err(XlErr::Num); }
        if bd(a, 3, false)? { return ok(f_cdf(x, p, q)); }
        let lb = ln_gamma(p / 2.0) + ln_gamma(q / 2.0) - ln_gamma((p + q) / 2.0);
        ok((0.5 * (p * (p * x).ln() + q * q.ln() - (p + q) * (p * x + q).ln()) - x.ln() - lb).exp())
    });
    r.s("F.DIST.RT", 3, 3, "x, deg_freedom1, deg_freedom2", "Right-tailed F distribution", |_, a| ok(1.0 - f_cdf(n(a, 0)?, n(a, 1)?.trunc(), n(a, 2)?.trunc())));
    r.alias("FDIST", "F.DIST.RT");
    r.s("F.INV", 3, 3, "probability, deg_freedom1, deg_freedom2", "Inverse F distribution", |_, a| { let (p, d1, d2) = (n(a, 0)?, n(a, 1)?, n(a, 2)?); if !(0.0..1.0).contains(&p) { Err(XlErr::Num) } else { ok(invert(p, 0.0, 16.0, |x| f_cdf(x, d1, d2))) } });
    r.s("F.INV.RT", 3, 3, "probability, deg_freedom1, deg_freedom2", "Right-tailed inverse F", |_, a| { let (p, d1, d2) = (n(a, 0)?, n(a, 1)?, n(a, 2)?); if p <= 0.0 || p > 1.0 { Err(XlErr::Num) } else { ok(invert(1.0 - p, 0.0, 16.0, |x| f_cdf(x, d1, d2))) } });
    r.alias("FINV", "F.INV.RT");
    r.a("F.TEST", 2, 2, "array1, array2", "F-test of two data sets", |c, a| {
        let (x, y) = (c.nums(&a[..1])?, c.nums(&a[1..2])?);
        if x.len() < 2 || y.len() < 2 { return Err(XlErr::Div0); }
        let (vx, vy) = (sq_dev(&x) / (x.len() as f64 - 1.0), sq_dev(&y) / (y.len() as f64 - 1.0));
        if vy == 0.0 { return Err(XlErr::Div0); }
        let p = f_cdf(vx / vy, x.len() as f64 - 1.0, y.len() as f64 - 1.0);
        ok(2.0 * p.min(1.0 - p))
    });
    r.alias("FTEST", "F.TEST");
    r.a("T.TEST", 4, 4, "array1, array2, tails, type", "Student's t-test", |c, a| ttest(a, c));
    r.alias("TTEST", "T.TEST");
    r.a("Z.TEST", 2, 3, "array, x, [sigma]", "One-tailed z-test", |c, a| {
        let v = c.nums(&a[..1])?;
        let x = n(a, 1)?;
        let s = match a.get(2) { Some(q) if *q != Value::Empty => to_num(q)?, _ => (sq_dev(&v) / (v.len() as f64 - 1.0)).sqrt() };
        if s == 0.0 { return Err(XlErr::Div0); }
        ok(1.0 - norm_cdf((mean(&v) - x) / (s / (v.len() as f64).sqrt())))
    });
    r.alias("ZTEST", "Z.TEST");
    r.s("CONFIDENCE.NORM", 3, 3, "alpha, standard_dev, size", "Confidence interval, normal", |_, a| ok(norm_inv(1.0 - n(a, 0)? / 2.0) * n(a, 1)? / n(a, 2)?.sqrt()));
    r.alias("CONFIDENCE", "CONFIDENCE.NORM");
    r.s("CONFIDENCE.T", 3, 3, "alpha, standard_dev, size", "Confidence interval, Student's t", |_, a| { let k = n(a, 2)?; ok(t_inv(1.0 - n(a, 0)? / 2.0, k - 1.0) * n(a, 1)? / k.sqrt()) });
    r.s("BINOM.DIST", 4, 4, "number_s, trials, probability_s, cumulative", "Binomial distribution", |_, a| {
        let (k, t0, p) = (n(a, 0)?.trunc(), n(a, 1)?.trunc(), n(a, 2)?);
        if k < 0.0 || k > t0 || !(0.0..=1.0).contains(&p) { return Err(XlErr::Num); }
        if bd(a, 3, false)? { ok((0..=k as i64).map(|i| binom_pmf(i as f64, t0, p)).sum::<f64>().min(1.0)) } else { ok(binom_pmf(k, t0, p)) }
    });
    r.alias("BINOMDIST", "BINOM.DIST");
    r.s("BINOM.INV", 3, 3, "trials, probability_s, alpha", "Smallest value with cumulative binomial at least alpha", |_, a| {
        let (t0, p, al) = (n(a, 0)?.trunc(), n(a, 1)?, n(a, 2)?);
        let mut c = 0.0; let mut k = 0.0;
        while k <= t0 { c += binom_pmf(k, t0, p); if c >= al { return ok(k); } k += 1.0; }
        ok(t0)
    });
    r.alias("CRITBINOM", "BINOM.INV");
    r.s("NEGBINOM.DIST", 4, 4, "number_f, number_s, probability_s, cumulative", "Negative binomial distribution", |_, a| {
        let (f, s, p) = (n(a, 0)?.trunc(), n(a, 1)?.trunc(), n(a, 2)?);
        let pmf = |x: f64| combin(x + s - 1.0, s - 1.0) * p.powf(s) * (1.0 - p).powf(x);
        if bd(a, 3, false)? { ok((0..=f as i64).map(|i| pmf(i as f64)).sum()) } else { ok(pmf(f)) }
    });
    r.s("POISSON.DIST", 3, 3, "x, mean, cumulative", "Poisson distribution", |_, a| {
        let (x, m) = (n(a, 0)?.trunc(), n(a, 1)?);
        if x < 0.0 || m < 0.0 { return Err(XlErr::Num); }
        let pmf = |k: f64| (k * m.ln() - m - ln_gamma(k + 1.0)).exp();
        if bd(a, 2, false)? { ok((0..=x as i64).map(|i| pmf(i as f64)).sum()) } else { ok(pmf(x)) }
    });
    r.alias("POISSON", "POISSON.DIST");
    r.s("EXPON.DIST", 3, 3, "x, lambda, cumulative", "Exponential distribution", |_, a| {
        let (x, l) = (n(a, 0)?, n(a, 1)?);
        if x < 0.0 || l <= 0.0 { return Err(XlErr::Num); }
        ok(if bd(a, 2, false)? { 1.0 - (-l * x).exp() } else { l * (-l * x).exp() })
    });
    r.alias("EXPONDIST", "EXPON.DIST");
    r.s("GAMMA.DIST", 4, 4, "x, alpha, beta, cumulative", "Gamma distribution", |_, a| {
        let (x, p, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if x < 0.0 || p <= 0.0 || b <= 0.0 { return Err(XlErr::Num); }
        ok(if bd(a, 3, false)? { gamma_p(p, x / b) } else { ((p - 1.0) * x.ln() - x / b - p * b.ln() - ln_gamma(p)).exp() })
    });
    r.alias("GAMMADIST", "GAMMA.DIST");
    r.s("GAMMA.INV", 3, 3, "probability, alpha, beta", "Inverse gamma distribution", |_, a| { let (p, q, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?); if !(0.0..1.0).contains(&p) { Err(XlErr::Num) } else { ok(invert(p, 0.0, q * b * 4.0, |x| gamma_p(q, x / b))) } });
    r.alias("GAMMAINV", "GAMMA.INV");
    r.s("BETA.DIST", 4, 6, "x, alpha, beta, cumulative, [A], [B]", "Beta distribution", |_, a| {
        let (x, p, q, lo, hi) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, nd(a, 4, 0.0)?, nd(a, 5, 1.0)?);
        let z = (x - lo) / (hi - lo);
        if !(0.0..=1.0).contains(&z) || p <= 0.0 || q <= 0.0 { return Err(XlErr::Num); }
        if bd(a, 3, false)? { return ok(beta_i(p, q, z)); }
        ok(((p - 1.0) * z.ln() + (q - 1.0) * (1.0 - z).ln() + ln_gamma(p + q) - ln_gamma(p) - ln_gamma(q)).exp() / (hi - lo))
    });
    r.s("BETA.INV", 3, 5, "probability, alpha, beta, [A], [B]", "Inverse beta distribution", |_, a| {
        let (p, q, s, lo, hi) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, nd(a, 3, 0.0)?, nd(a, 4, 1.0)?);
        ok(lo + (hi - lo) * invert(p, 0.0, 1.0, |x| beta_i(q, s, x)))
    });
    r.s("WEIBULL.DIST", 4, 4, "x, alpha, beta, cumulative", "Weibull distribution", |_, a| {
        let (x, p, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if x < 0.0 || p <= 0.0 || b <= 0.0 { return Err(XlErr::Num); }
        ok(if bd(a, 3, false)? { 1.0 - (-(x / b).powf(p)).exp() } else { p / b.powf(p) * x.powf(p - 1.0) * (-(x / b).powf(p)).exp() })
    });
    r.alias("WEIBULL", "WEIBULL.DIST");
    r.s("HYPGEOM.DIST", 5, 5, "sample_s, number_sample, population_s, number_pop, cumulative", "Hypergeometric distribution", |_, a| {
        let (k, s, ps, pop) = (n(a, 0)?.trunc(), n(a, 1)?.trunc(), n(a, 2)?.trunc(), n(a, 3)?.trunc());
        let pmf = |x: f64| combin(ps, x) * combin(pop - ps, s - x) / combin(pop, s);
        if bd(a, 4, false)? { ok((0..=k as i64).map(|i| pmf(i as f64)).sum()) } else { ok(pmf(k)) }
    });
    let _ = mat_val;
}
