//! Financial and engineering functions.

use crate::eval::*;
use crate::funcs::*;
use crate::funcs_stat::erf;
use crate::funcs_text::yearfrac;
use crate::model::*;
use crate::numfmt;

fn pmt(rate: f64, nper: f64, pv: f64, fv: f64, ty: f64) -> f64 {
    if rate == 0.0 {
        return -(pv + fv) / nper;
    }
    let f = (1.0 + rate).powf(nper);
    -(rate * (pv * f + fv)) / ((1.0 + rate * ty) * (f - 1.0))
}

fn fv(rate: f64, nper: f64, pm: f64, pv: f64, ty: f64) -> f64 {
    if rate == 0.0 {
        return -(pv + pm * nper);
    }
    let f = (1.0 + rate).powf(nper);
    -(pv * f + pm * (1.0 + rate * ty) * (f - 1.0) / rate)
}

fn ipmt(rate: f64, per: f64, nper: f64, pv: f64, fv0: f64, ty: f64) -> f64 {
    let p = pmt(rate, nper, pv, fv0, ty);
    let bal = fv(rate, per - 1.0, p, pv, ty);
    let i = bal * rate;
    if ty == 1.0 {
        if per == 1.0 {
            0.0
        } else {
            (bal - p) * rate / (1.0 + rate) + 0.0 * i
        }
    } else {
        i
    }
}

fn solve_rate(f: impl Fn(f64) -> f64, guess: f64) -> Option<f64> {
    // Secant iteration, with a scan fallback for awkward cash flows.
    let (mut x0, mut x1) = (guess, guess + 0.01);
    let (mut y0, mut y1) = (f(x0), f(x1));
    for _ in 0..200 {
        if y1.abs() < 1e-9 {
            return Some(x1);
        }
        let d = y1 - y0;
        if d == 0.0 || !d.is_finite() {
            break;
        }
        let x2 = x1 - y1 * (x1 - x0) / d;
        x0 = x1;
        y0 = y1;
        x1 = if x2 <= -1.0 { (x1 - 1.0) / 2.0 } else { x2 };
        y1 = f(x1);
    }
    let mut prev: Option<(f64, f64)> = None;
    let mut x = -0.99;
    while x < 10.0 {
        let y = f(x);
        if let Some((a, b)) = prev {
            if b.is_finite() && y.is_finite() && b * y < 0.0 {
                let (mut lo, mut hi, mut flo) = (a, x, b);
                for _ in 0..200 {
                    let m = (lo + hi) / 2.0;
                    let fm = f(m);
                    if flo * fm <= 0.0 {
                        hi = m;
                    } else {
                        lo = m;
                        flo = fm;
                    }
                }
                return Some((lo + hi) / 2.0);
            }
        }
        prev = Some((x, y));
        x += 0.01;
    }
    None
}

fn year_basis(b: i64) -> f64 {
    if b == 3 { 365.0 } else { 360.0 }
}

fn coupon_price(settle: f64, mat: f64, rate: f64, yld: f64, red: f64, freq: f64, basis: i64) -> f64 {
    // Whole coupon periods remaining, with the first period's accrual fraction.
    let months = 12.0 / freq;
    let yrs = yearfrac(settle, mat, basis);
    let n0 = (yrs * freq).ceil().max(1.0);
    let frac = n0 - yrs * freq;
    let c = 100.0 * rate / freq;
    let _ = months;
    let mut p = red / (1.0 + yld / freq).powf(n0 - 1.0 + (1.0 - frac));
    for k in 0..n0 as i64 {
        p += c / (1.0 + yld / freq).powf(k as f64 + 1.0 - frac);
    }
    p - c * frac
}

pub fn financial(r: &mut Reg) {
    r.cat = "Financial";
    r.s("PMT", 3, 5, "rate, nper, pv, [fv], [type]", "Payment for a loan", |_, a| {
        let k = n(a, 1)?;
        if k == 0.0 { return Err(XlErr::Num); }
        ok(pmt(n(a, 0)?, k, n(a, 2)?, nd(a, 3, 0.0)?, nd(a, 4, 0.0)?))
    });
    r.s("FV", 3, 5, "rate, nper, pmt, [pv], [type]", "Future value of an investment", |_, a| ok(fv(n(a, 0)?, n(a, 1)?, n(a, 2)?, nd(a, 3, 0.0)?, nd(a, 4, 0.0)?)));
    r.s("PV", 3, 5, "rate, nper, pmt, [fv], [type]", "Present value of an investment", |_, a| {
        let (rate, k, p, f, ty) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, nd(a, 3, 0.0)?, nd(a, 4, 0.0)?);
        if rate == 0.0 { return ok(-(f + p * k)); }
        let q = (1.0 + rate).powf(k);
        ok(-(f + p * (1.0 + rate * ty) * (q - 1.0) / rate) / q)
    });
    r.s("NPER", 3, 5, "rate, pmt, pv, [fv], [type]", "Number of payment periods", |_, a| {
        let (rate, p, pv0, f, ty) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, nd(a, 3, 0.0)?, nd(a, 4, 0.0)?);
        if rate == 0.0 {
            if p == 0.0 { return Err(XlErr::Num); }
            return ok(-(pv0 + f) / p);
        }
        let z = p * (1.0 + rate * ty) / rate;
        ok(((z - f) / (z + pv0)).ln() / (1.0 + rate).ln())
    });
    r.s("RATE", 3, 6, "nper, pmt, pv, [fv], [type], [guess]", "Interest rate per period", |_, a| {
        let (k, p, pv0, f, ty, g) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, nd(a, 3, 0.0)?, nd(a, 4, 0.0)?, nd(a, 5, 0.1)?);
        solve_rate(|x| {
            if x.abs() < 1e-12 { return pv0 + p * k + f; }
            let q = (1.0 + x).powf(k);
            pv0 * q + p * (1.0 + x * ty) * (q - 1.0) / x + f
        }, g).map(Value::Num).ok_or(XlErr::Num)
    });
    r.s("IPMT", 4, 6, "rate, per, nper, pv, [fv], [type]", "Interest part of a payment", |_, a| {
        let (rate, per, k) = (n(a, 0)?, n(a, 1)?, n(a, 2)?);
        if per < 1.0 || per > k { return Err(XlErr::Num); }
        ok(ipmt(rate, per, k, n(a, 3)?, nd(a, 4, 0.0)?, nd(a, 5, 0.0)?))
    });
    r.s("PPMT", 4, 6, "rate, per, nper, pv, [fv], [type]", "Principal part of a payment", |_, a| {
        let (rate, per, k, pv0, f, ty) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, nd(a, 4, 0.0)?, nd(a, 5, 0.0)?);
        if per < 1.0 || per > k { return Err(XlErr::Num); }
        ok(pmt(rate, k, pv0, f, ty) - ipmt(rate, per, k, pv0, f, ty))
    });
    r.s("ISPMT", 4, 4, "rate, per, nper, pv", "Interest paid on a straight-line loan", |_, a| {
        let (rate, per, k, pv0) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?);
        ok(pv0 * rate * (per / k - 1.0))
    });
    r.a("CUMIPMT", 6, 6, "rate, nper, pv, start_period, end_period, type", "Cumulative interest paid", |_, a| {
        let (rate, k, pv0, s, e, ty) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, n(a, 4)?, n(a, 5)?);
        ok((s as i64..=e as i64).map(|p| ipmt(rate, p as f64, k, pv0, 0.0, ty)).sum())
    });
    r.a("CUMPRINC", 6, 6, "rate, nper, pv, start_period, end_period, type", "Cumulative principal paid", |_, a| {
        let (rate, k, pv0, s, e, ty) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, n(a, 4)?, n(a, 5)?);
        let p0 = pmt(rate, k, pv0, 0.0, ty);
        ok((s as i64..=e as i64).map(|p| p0 - ipmt(rate, p as f64, k, pv0, 0.0, ty)).sum())
    });
    r.a("NPV", 2, MANY, "rate, value1, [value2], ...", "Net present value of cash flows", |c, a| {
        let rate = n(a, 0)?;
        let v = c.nums(&a[1..])?;
        ok(v.iter().enumerate().map(|(i, x)| x / (1.0 + rate).powi(i as i32 + 1)).sum())
    });
    r.a("IRR", 1, 2, "values, [guess]", "Internal rate of return", |c, a| {
        let v = c.nums(&a[..1])?;
        if !v.iter().any(|x| *x > 0.0) || !v.iter().any(|x| *x < 0.0) { return Err(XlErr::Num); }
        solve_rate(|r0| v.iter().enumerate().map(|(i, x)| x / (1.0 + r0).powi(i as i32)).sum(), nd(a, 1, 0.1)?).map(Value::Num).ok_or(XlErr::Num)
    });
    r.a("MIRR", 3, 3, "values, finance_rate, reinvest_rate", "Modified internal rate of return", |c, a| {
        let v = c.nums(&a[..1])?;
        let (fr, rr) = (n(a, 1)?, n(a, 2)?);
        let k = v.len() as f64;
        let pos: f64 = v.iter().enumerate().filter(|x| *x.1 > 0.0).map(|(i, x)| x * (1.0 + rr).powf(k - 1.0 - i as f64)).sum();
        let neg: f64 = v.iter().enumerate().filter(|x| *x.1 < 0.0).map(|(i, x)| x / (1.0 + fr).powi(i as i32)).sum();
        if neg == 0.0 { return Err(XlErr::Div0); }
        ok((pos / -neg).powf(1.0 / (k - 1.0)) - 1.0)
    });
    r.a("XNPV", 3, 3, "rate, values, dates", "Net present value of dated cash flows", |c, a| {
        let rate = n(a, 0)?;
        let (v, d) = pairs(c, &a[1], &a[2])?;
        if d.is_empty() { return Err(XlErr::Num); }
        let d0 = d[0];
        ok(v.iter().zip(&d).map(|(x, t0)| x / (1.0 + rate).powf((t0 - d0) / 365.0)).sum())
    });
    r.a("XIRR", 2, 3, "values, dates, [guess]", "Internal rate of return of dated cash flows", |c, a| {
        let (v, d) = pairs(c, &a[0], &a[1])?;
        if !v.iter().any(|x| *x > 0.0) || !v.iter().any(|x| *x < 0.0) { return Err(XlErr::Num); }
        let d0 = d[0];
        solve_rate(|r0| v.iter().zip(&d).map(|(x, t0)| x / (1.0 + r0).powf((t0 - d0) / 365.0)).sum(), nd(a, 2, 0.1)?).map(Value::Num).ok_or(XlErr::Num)
    });
    r.a("FVSCHEDULE", 2, 2, "principal, schedule", "Future value after a schedule of rates", |c, a| ok(c.nums(&a[1..2])?.iter().fold(n(a, 0)?, |p, q| p * (1.0 + q))));
    r.s("SLN", 3, 3, "cost, salvage, life", "Straight-line depreciation", |_, a| { let l = n(a, 2)?; if l == 0.0 { Err(XlErr::Div0) } else { ok((n(a, 0)? - n(a, 1)?) / l) } });
    r.s("SYD", 4, 4, "cost, salvage, life, per", "Sum-of-years' digits depreciation", |_, a| {
        let (c0, s, l, p) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?);
        ok((c0 - s) * (l - p + 1.0) * 2.0 / (l * (l + 1.0)))
    });
    r.s("DB", 4, 5, "cost, salvage, life, period, [month]", "Fixed-declining balance depreciation", |_, a| {
        let (c0, s, l, p, m) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, nd(a, 4, 12.0)?);
        if c0 <= 0.0 { return Ok(Value::Num(0.0)); }
        let rate = ((1.0 - (s / c0).powf(1.0 / l)) * 1000.0).round() / 1000.0;
        let mut total = 0.0;
        let mut dep = c0 * rate * m / 12.0;
        if p == 1.0 { return ok(dep); }
        let mut i = 2.0;
        while i <= p {
            total += dep;
            dep = if i == l + 1.0 { (c0 - total) * rate * (12.0 - m) / 12.0 } else { (c0 - total) * rate };
            i += 1.0;
        }
        ok(dep)
    });
    r.s("DDB", 4, 5, "cost, salvage, life, period, [factor]", "Double-declining balance depreciation", |_, a| {
        let (c0, s, l, p, f) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, nd(a, 4, 2.0)?);
        let mut book = c0;
        let mut dep = 0.0;
        let mut i = 1.0;
        while i <= p {
            dep = (book * f / l).min((book - s).max(0.0));
            book -= dep;
            i += 1.0;
        }
        ok(dep)
    });
    r.s("VDB", 5, 7, "cost, salvage, life, start_period, end_period, [factor], [no_switch]", "Variable-declining balance depreciation", |_, a| {
        let (c0, s, l, st, en, f) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, n(a, 4)?, nd(a, 5, 2.0)?);
        let mut book = c0;
        let mut total = 0.0;
        let mut p = 0.0;
        while p < en {
            let ddb = book * f / l;
            let sl = (book - s) / (l - p).max(1.0);
            let dep = ddb.max(sl).min((book - s).max(0.0));
            let lo = st.max(p);
            let hi = en.min(p + 1.0);
            if hi > lo { total += dep * (hi - lo); }
            book -= dep;
            p += 1.0;
        }
        ok(total)
    });
    r.s("EFFECT", 2, 2, "nominal_rate, npery", "Effective annual rate", |_, a| { let (x, k) = (n(a, 0)?, n(a, 1)?.trunc()); if x <= 0.0 || k < 1.0 { Err(XlErr::Num) } else { ok((1.0 + x / k).powf(k) - 1.0) } });
    r.s("NOMINAL", 2, 2, "effect_rate, npery", "Nominal annual rate", |_, a| { let (x, k) = (n(a, 0)?, n(a, 1)?.trunc()); if x <= 0.0 || k < 1.0 { Err(XlErr::Num) } else { ok(k * ((1.0 + x).powf(1.0 / k) - 1.0)) } });
    r.s("PDURATION", 3, 3, "rate, pv, fv", "Periods to reach a value", |_, a| ok((n(a, 2)? / n(a, 1)?).ln() / (1.0 + n(a, 0)?).ln()));
    r.s("RRI", 3, 3, "nper, pv, fv", "Equivalent growth rate", |_, a| ok((n(a, 2)? / n(a, 1)?).powf(1.0 / n(a, 0)?) - 1.0));
    r.s("DOLLARDE", 2, 2, "fractional_dollar, fraction", "Converts a fractional price to decimal", |_, a| {
        let (x, f) = (n(a, 0)?, n(a, 1)?.trunc());
        if f < 1.0 { return Err(XlErr::Num); }
        let i = x.trunc();
        ok(i + (x - i) * 10f64.powf(f.log10().ceil()) / f)
    });
    r.s("DOLLARFR", 2, 2, "decimal_dollar, fraction", "Converts a decimal price to fractional", |_, a| {
        let (x, f) = (n(a, 0)?, n(a, 1)?.trunc());
        if f < 1.0 { return Err(XlErr::Num); }
        let i = x.trunc();
        ok(i + (x - i) * f / 10f64.powf(f.log10().ceil()))
    });
    r.s("DISC", 4, 5, "settlement, maturity, pr, redemption, [basis]", "Discount rate for a security", |_, a| {
        let (s, m, p, red, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, nd(a, 4, 0.0)? as i64);
        ok((red - p) / red / yearfrac(s, m, b))
    });
    r.s("INTRATE", 4, 5, "settlement, maturity, investment, redemption, [basis]", "Interest rate for a security", |_, a| {
        let (s, m, i, red, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, nd(a, 4, 0.0)? as i64);
        ok((red - i) / i / yearfrac(s, m, b))
    });
    r.s("RECEIVED", 4, 5, "settlement, maturity, investment, discount, [basis]", "Amount received at maturity", |_, a| {
        let (s, m, i, d, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, nd(a, 4, 0.0)? as i64);
        ok(i / (1.0 - d * yearfrac(s, m, b)))
    });
    r.s("TBILLPRICE", 3, 3, "settlement, maturity, discount", "Price of a Treasury bill", |_, a| ok(100.0 * (1.0 - n(a, 2)? * (n(a, 1)? - n(a, 0)?) / 360.0)));
    r.s("TBILLYIELD", 3, 3, "settlement, maturity, pr", "Yield of a Treasury bill", |_, a| { let p = n(a, 2)?; ok((100.0 - p) / p * 360.0 / (n(a, 1)? - n(a, 0)?)) });
    r.s("TBILLEQ", 3, 3, "settlement, maturity, discount", "Bond-equivalent yield of a Treasury bill", |_, a| { let (d, q) = (n(a, 2)?, n(a, 1)? - n(a, 0)?); ok(365.0 * d / (360.0 - d * q)) });
    r.s("PRICEDISC", 4, 5, "settlement, maturity, discount, redemption, [basis]", "Price of a discounted security", |_, a| ok(n(a, 3)? * (1.0 - n(a, 2)? * yearfrac(n(a, 0)?, n(a, 1)?, nd(a, 4, 0.0)? as i64))));
    r.s("PRICE", 6, 7, "settlement, maturity, rate, yld, redemption, frequency, [basis]", "Price of a coupon bond per 100", |_, a| ok(coupon_price(n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, n(a, 4)?, n(a, 5)?, nd(a, 6, 0.0)? as i64)));
    r.s("YIELD", 6, 7, "settlement, maturity, rate, pr, redemption, frequency, [basis]", "Yield of a coupon bond", |_, a| {
        let (s, m, rate, p, red, f, b) = (n(a, 0)?, n(a, 1)?, n(a, 2)?, n(a, 3)?, n(a, 4)?, n(a, 5)?, nd(a, 6, 0.0)? as i64);
        solve_rate(|y| coupon_price(s, m, rate, y, red, f, b) - p, rate.max(0.01)).map(Value::Num).ok_or(XlErr::Num)
    });
    r.s("ACCRINTM", 4, 5, "issue, settlement, rate, par, [basis]", "Accrued interest paid at maturity", |_, a| ok(n(a, 3)? * n(a, 2)? * yearfrac(n(a, 0)?, n(a, 1)?, nd(a, 4, 0.0)? as i64)));
    let _ = (year_basis, numfmt::general);
}

fn radix(s: &str, base: u32, bits: u32) -> Result<f64, XlErr> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(0.0);
    }
    let v = i64::from_str_radix(s, base).map_err(|_| XlErr::Num)?;
    // Ten-digit values use the top bit as a sign.
    let width = match base {
        2 => 10,
        8 => 10,
        _ => 10,
    };
    if s.len() >= width {
        let lim = 1i64 << bits;
        if v >= lim / 2 {
            return Ok((v - lim) as f64);
        }
    }
    Ok(v as f64)
}

fn to_radix(v: f64, base: u32, bits: u32, places: Option<usize>) -> R {
    let lim = 1i64 << (bits - 1);
    let x = v.trunc() as i64;
    if x < -lim || x >= lim {
        return Err(XlErr::Num);
    }
    let u = if x < 0 { (x + (lim << 1)) as u64 } else { x as u64 };
    let s = match base {
        2 => format!("{:b}", u),
        8 => format!("{:o}", u),
        _ => format!("{:X}", u),
    };
    match places {
        Some(p) if x >= 0 => {
            if s.len() > p {
                return Err(XlErr::Num);
            }
            txt(format!("{:0>w$}", s, w = p))
        }
        _ => txt(s),
    }
}

fn conv_factor(u: &str) -> Option<(&'static str, f64)> {
    Some(match u {
        "m" => ("len", 1.0),
        "km" => ("len", 1000.0),
        "cm" => ("len", 0.01),
        "mm" => ("len", 0.001),
        "in" => ("len", 0.0254),
        "ft" => ("len", 0.3048),
        "yd" => ("len", 0.9144),
        "mi" => ("len", 1609.344),
        "Nmi" => ("len", 1852.0),
        "g" => ("mass", 1.0),
        "kg" => ("mass", 1000.0),
        "mg" => ("mass", 0.001),
        "lbm" => ("mass", 453.59237),
        "ozm" => ("mass", 28.349523125),
        "ton" => ("mass", 907184.74),
        "sec" | "s" => ("time", 1.0),
        "min" | "mn" => ("time", 60.0),
        "hr" => ("time", 3600.0),
        "day" | "d" => ("time", 86400.0),
        "yr" => ("time", 31557600.0),
        "l" | "L" | "lt" => ("vol", 1.0),
        "ml" => ("vol", 0.001),
        "gal" => ("vol", 3.785411784),
        "qt" => ("vol", 0.946352946),
        "pt" => ("vol", 0.473176473),
        "cup" => ("vol", 0.2365882365),
        "oz" => ("vol", 0.0295735295625),
        "tbs" => ("vol", 0.01478676478125),
        "tsp" => ("vol", 0.00492892159375),
        "J" => ("energy", 1.0),
        "kJ" => ("energy", 1000.0),
        "cal" => ("energy", 4.1868),
        "kcal" => ("energy", 4186.8),
        "Wh" => ("energy", 3600.0),
        "kWh" => ("energy", 3600000.0),
        "W" => ("power", 1.0),
        "kW" => ("power", 1000.0),
        "HP" | "h" => ("power", 745.69987158227),
        "Pa" => ("press", 1.0),
        "kPa" => ("press", 1000.0),
        "atm" => ("press", 101325.0),
        "mmHg" => ("press", 133.322),
        "psi" => ("press", 6894.757293),
        "bit" => ("info", 1.0),
        "byte" => ("info", 8.0),
        _ => return None,
    })
}

pub fn engineering(r: &mut Reg) {
    r.cat = "Engineering";
    r.s("BIN2DEC", 1, 1, "number", "Binary to decimal", |_, a| radix(&t(a, 0)?, 2, 10).map(Value::Num));
    r.s("OCT2DEC", 1, 1, "number", "Octal to decimal", |_, a| radix(&t(a, 0)?, 8, 30).map(Value::Num));
    r.s("HEX2DEC", 1, 1, "number", "Hexadecimal to decimal", |_, a| radix(&t(a, 0)?, 16, 40).map(Value::Num));
    r.s("DEC2BIN", 1, 2, "number, [places]", "Decimal to binary", |_, a| to_radix(n(a, 0)?, 2, 10, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("DEC2OCT", 1, 2, "number, [places]", "Decimal to octal", |_, a| to_radix(n(a, 0)?, 8, 30, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("DEC2HEX", 1, 2, "number, [places]", "Decimal to hexadecimal", |_, a| to_radix(n(a, 0)?, 16, 40, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("BIN2HEX", 1, 2, "number, [places]", "Binary to hexadecimal", |_, a| to_radix(radix(&t(a, 0)?, 2, 10)?, 16, 40, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("BIN2OCT", 1, 2, "number, [places]", "Binary to octal", |_, a| to_radix(radix(&t(a, 0)?, 2, 10)?, 8, 30, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("HEX2BIN", 1, 2, "number, [places]", "Hexadecimal to binary", |_, a| to_radix(radix(&t(a, 0)?, 16, 40)?, 2, 10, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("HEX2OCT", 1, 2, "number, [places]", "Hexadecimal to octal", |_, a| to_radix(radix(&t(a, 0)?, 16, 40)?, 8, 30, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("OCT2BIN", 1, 2, "number, [places]", "Octal to binary", |_, a| to_radix(radix(&t(a, 0)?, 8, 30)?, 2, 10, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("OCT2HEX", 1, 2, "number, [places]", "Octal to hexadecimal", |_, a| to_radix(radix(&t(a, 0)?, 8, 30)?, 16, 40, a.get(1).and_then(|x| to_num(x).ok()).map(|x| x as usize)));
    r.s("BITAND", 2, 2, "number1, number2", "Bitwise AND", |_, a| Ok(Value::Num(((n(a, 0)? as u64) & (n(a, 1)? as u64)) as f64)));
    r.s("BITOR", 2, 2, "number1, number2", "Bitwise OR", |_, a| Ok(Value::Num(((n(a, 0)? as u64) | (n(a, 1)? as u64)) as f64)));
    r.s("BITXOR", 2, 2, "number1, number2", "Bitwise XOR", |_, a| Ok(Value::Num(((n(a, 0)? as u64) ^ (n(a, 1)? as u64)) as f64)));
    r.s("BITLSHIFT", 2, 2, "number, shift_amount", "Bitwise left shift", |_, a| ok(n(a, 0)?.trunc() * 2f64.powf(n(a, 1)?.trunc())));
    r.s("BITRSHIFT", 2, 2, "number, shift_amount", "Bitwise right shift", |_, a| ok((n(a, 0)?.trunc() / 2f64.powf(n(a, 1)?.trunc())).floor()));
    r.s("DELTA", 1, 2, "number1, [number2]", "Tests whether two numbers are equal", |_, a| Ok(Value::Num((n(a, 0)? == nd(a, 1, 0.0)?) as i32 as f64)));
    r.s("GESTEP", 1, 2, "number, [step]", "Tests whether a number is at least the step", |_, a| Ok(Value::Num((n(a, 0)? >= nd(a, 1, 0.0)?) as i32 as f64)));
    r.s("ERF", 1, 2, "lower_limit, [upper_limit]", "Error function", |_, a| {
        let x = n(a, 0)?;
        match a.get(1) { Some(v) if *v != Value::Empty => ok(erf(to_num(v)?) - erf(x)), _ => ok(erf(x)) }
    });
    r.alias("ERF.PRECISE", "ERF");
    r.s("ERFC", 1, 1, "x", "Complementary error function", |_, a| ok(1.0 - erf(n(a, 0)?)));
    r.alias("ERFC.PRECISE", "ERFC");
    r.s("CONVERT", 3, 3, "number, from_unit, to_unit", "Converts between measurement units", |_, a| {
        let (x, f, to) = (n(a, 0)?, t(a, 1)?, t(a, 2)?);
        let temp = |u: &str| matches!(u, "C" | "cel" | "F" | "fah" | "K" | "kel");
        if temp(&f) && temp(&to) {
            let c = match f.as_str() { "C" | "cel" => x, "F" | "fah" => (x - 32.0) * 5.0 / 9.0, _ => x - 273.15 };
            return ok(match to.as_str() { "C" | "cel" => c, "F" | "fah" => c * 9.0 / 5.0 + 32.0, _ => c + 273.15 });
        }
        let (Some((k1, a1)), Some((k2, a2))) = (conv_factor(&f), conv_factor(&to)) else { return Err(XlErr::NA) };
        if k1 != k2 { return Err(XlErr::NA); }
        ok(x * a1 / a2)
    });
    r.s("COMPLEX", 2, 3, "real_num, i_num, [suffix]", "Builds a complex number as text", |_, a| {
        let (x, y) = (n(a, 0)?, n(a, 1)?);
        let u = td(a, 2, "i")?;
        txt(if y == 0.0 { num_text(x) } else if x == 0.0 { format!("{}{u}", num_text(y)) } else { format!("{}{}{}{u}", num_text(x), if y > 0.0 { "+" } else { "" }, num_text(y)) })
    });
}
