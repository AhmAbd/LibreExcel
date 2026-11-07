//! Number formats: renders values through format codes, and reads typed input back into values.

use crate::model::Content;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rendered {
    pub text: String,
    /// Text before a `*` fill, pinned to the left edge (accounting formats).
    pub left: String,
    pub color: Option<u32>,
}

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Serial date in the 1900 system, which keeps the historic fake 29 Feb 1900.
pub fn ymd_to_serial(y: i64, m: i64, d: i64) -> f64 {
    let m0 = m - 1;
    let y2 = y + m0.div_euclid(12);
    let m2 = m0.rem_euclid(12) + 1;
    let days = days_from_civil(y2, m2, 1) + d - 1;
    let base = days_from_civil(1899, 12, 31);
    let mut n = days - base;
    if n >= 60 {
        n += 1;
    }
    n as f64
}

pub fn serial_to_ymd(s: f64) -> (i64, i64, i64) {
    let n = s.floor() as i64;
    if n == 60 {
        return (1900, 2, 29);
    }
    if n == 0 {
        return (1900, 1, 0);
    }
    let base = days_from_civil(1899, 12, 31);
    civil_from_days(base + if n > 60 { n - 1 } else { n })
}

pub fn weekday_of(s: f64) -> i64 {
    // 0 = Sunday. Serial 1 (1 Jan 1900) reads as a Sunday in the 1900 system.
    (s.floor() as i64 - 1).rem_euclid(7)
}

pub const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
    "November", "December",
];
pub const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Lit(String),
    Digit(char),
    Point,
    Thousands,
    Percent,
    Exp(bool),
    Date(String),
    Elapsed(char, usize),
    AmPm(String),
    Text,
    Fill(char),
    Skip,
    Slash,
}

#[derive(Clone, Debug, Default)]
struct Section {
    toks: Vec<Tok>,
    color: Option<u32>,
    cond: Option<(String, f64)>,
    is_date: bool,
    is_num: bool,
    is_frac: bool,
}

fn color_name(n: &str) -> Option<u32> {
    Some(match n.to_ascii_lowercase().as_str() {
        "red" => 0xFF0000,
        "blue" => 0x0000FF,
        "green" => 0x00FF00,
        "yellow" => 0xFFFF00,
        "magenta" => 0xFF00FF,
        "cyan" => 0x00FFFF,
        "white" => 0xFFFFFF,
        "black" => 0x000000,
        _ => return None,
    })
}

fn parse_sections(code: &str) -> Vec<Section> {
    let ch: Vec<char> = code.chars().collect();
    let mut secs = vec![];
    let mut s = Section::default();
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        match c {
            ';' => {
                secs.push(std::mem::take(&mut s));
                i += 1;
            }
            '"' => {
                i += 1;
                let mut t = String::new();
                while i < ch.len() && ch[i] != '"' {
                    t.push(ch[i]);
                    i += 1;
                }
                i += 1;
                s.toks.push(Tok::Lit(t));
            }
            '\\' => {
                if i + 1 < ch.len() {
                    s.toks.push(Tok::Lit(ch[i + 1].to_string()));
                }
                i += 2;
            }
            '_' => {
                s.toks.push(Tok::Skip);
                i += 2;
            }
            '*' => {
                if i + 1 < ch.len() {
                    s.toks.push(Tok::Fill(ch[i + 1]));
                }
                i += 2;
            }
            '[' => {
                let mut t = String::new();
                i += 1;
                while i < ch.len() && ch[i] != ']' {
                    t.push(ch[i]);
                    i += 1;
                }
                i += 1;
                let low = t.to_ascii_lowercase();
                if let Some(col) = color_name(&t) {
                    s.color = Some(col);
                } else if !t.is_empty() && low.chars().all(|x| x == 'h') {
                    s.toks.push(Tok::Elapsed('h', t.len()));
                    s.is_date = true;
                } else if !t.is_empty() && low.chars().all(|x| x == 'm') {
                    s.toks.push(Tok::Elapsed('m', t.len()));
                    s.is_date = true;
                } else if !t.is_empty() && low.chars().all(|x| x == 's') {
                    s.toks.push(Tok::Elapsed('s', t.len()));
                    s.is_date = true;
                } else if t.starts_with('$') {
                    let sym = t[1..].split('-').next().unwrap_or("");
                    s.toks.push(Tok::Lit(sym.to_string()));
                } else if let Some(c0) = t.chars().next() {
                    if "<>=".contains(c0) {
                        let op_end = t.find(|x: char| !"<>=".contains(x)).unwrap_or(t.len());
                        if let Ok(v) = t[op_end..].trim().parse::<f64>() {
                            s.cond = Some((t[..op_end].to_string(), v));
                        }
                    }
                }
            }
            '0' | '#' | '?' => {
                s.toks.push(Tok::Digit(c));
                s.is_num = true;
                i += 1;
            }
            '.' => {
                s.toks.push(Tok::Point);
                i += 1;
            }
            ',' => {
                s.toks.push(Tok::Thousands);
                i += 1;
            }
            '%' => {
                s.toks.push(Tok::Percent);
                i += 1;
            }
            '@' => {
                s.toks.push(Tok::Text);
                i += 1;
            }
            '/' => {
                s.toks.push(Tok::Slash);
                i += 1;
            }
            'E' | 'e' if i + 1 < ch.len() && (ch[i + 1] == '+' || ch[i + 1] == '-') => {
                s.toks.push(Tok::Exp(ch[i + 1] == '+'));
                i += 2;
            }
            'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
                let l = c.to_ascii_lowercase();
                let st = i;
                while i < ch.len() && ch[i].to_ascii_lowercase() == l {
                    i += 1;
                }
                s.toks.push(Tok::Date(std::iter::repeat(l).take(i - st).collect()));
                s.is_date = true;
            }
            'A' | 'a' => {
                let rest: String = ch[i..].iter().take(5).collect::<String>();
                if rest.to_ascii_uppercase().starts_with("AM/PM") {
                    s.toks.push(Tok::AmPm(rest));
                    s.is_date = true;
                    i += 5;
                } else if rest.to_ascii_uppercase().starts_with("A/P") {
                    s.toks.push(Tok::AmPm(rest[..3].to_string()));
                    s.is_date = true;
                    i += 3;
                } else {
                    s.toks.push(Tok::Lit(c.to_string()));
                    i += 1;
                }
            }
            _ => {
                s.toks.push(Tok::Lit(c.to_string()));
                i += 1;
            }
        }
    }
    secs.push(s);
    for sec in &mut secs {
        sec.is_frac = !sec.is_date && sec.is_num && sec.toks.contains(&Tok::Slash);
        if sec.is_date {
            // A slash in a date is a plain separator.
            for t in &mut sec.toks {
                if *t == Tok::Slash {
                    *t = Tok::Lit("/".into());
                }
                if *t == Tok::Point {
                    *t = Tok::Lit(".".into());
                }
                if *t == Tok::Thousands {
                    *t = Tok::Lit(",".into());
                }
            }
        } else if !sec.is_num {
            for t in &mut sec.toks {
                if *t == Tok::Slash {
                    *t = Tok::Lit("/".into());
                }
                if *t == Tok::Thousands {
                    *t = Tok::Lit(",".into());
                }
                if *t == Tok::Point {
                    *t = Tok::Lit(".".into());
                }
            }
        }
    }
    secs
}

pub fn general(x: f64, width_hint: usize) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return "#NUM!".into();
    }
    let a = x.abs();
    if a >= 1e11 || a < 1e-9 {
        // Scientific with up to six significant decimals.
        let s = format!("{:.5E}", x);
        let (m, e) = s.split_once('E').unwrap();
        let m = m.trim_end_matches('0').trim_end_matches('.');
        let ev: i32 = e.parse().unwrap_or(0);
        return format!("{}E{}{:02}", m, if ev < 0 { "-" } else { "+" }, ev.abs());
    }
    let int_digits = if a >= 1.0 { a.log10().floor() as usize + 1 } else { 1 };
    let max_dec = width_hint.saturating_sub(int_digits + 1 + (x < 0.0) as usize).min(15usize.saturating_sub(int_digits));
    let mut s = format!("{:.*}", max_dec, x);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

fn group_int(s: &str) -> String {
    let n = s.len();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        out.push(c);
        let left = n - i - 1;
        if left > 0 && left % 3 == 0 {
            out.push(',');
        }
    }
    out
}

fn fmt_date(sec: &Section, v: f64, negative: bool) -> Rendered {
    let mut r = Rendered { color: sec.color, ..Default::default() };
    let v = v.abs();
    let mut day = v.floor();
    let mut secs = ((v - day) * 86400.0 * 1000.0).round() / 1000.0;
    let has_frac_sec = sec.toks.iter().enumerate().any(|(i, t)| {
        matches!(t, Tok::Lit(l) if l == ".") && matches!(sec.toks.get(i + 1), Some(Tok::Digit('0')))
    });
    if !has_frac_sec {
        secs = secs.round();
    }
    if secs >= 86400.0 {
        secs -= 86400.0;
        day += 1.0;
    }
    let (y, m, d) = serial_to_ymd(day);
    let total = secs;
    let hh = (total / 3600.0).floor() as i64;
    let mi = ((total - hh as f64 * 3600.0) / 60.0).floor() as i64;
    let ss_f = total - hh as f64 * 3600.0 - mi as f64 * 60.0;
    let ss = ss_f.floor() as i64;
    let ampm = sec.toks.iter().any(|t| matches!(t, Tok::AmPm(_)));
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    let toks = &sec.toks;
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Lit(t) => {
                if t == "." && matches!(toks.get(i + 1), Some(Tok::Digit('0'))) {
                    let mut n = 0;
                    while matches!(toks.get(i + 1 + n), Some(Tok::Digit('0'))) {
                        n += 1;
                    }
                    let frac = ss_f - ss as f64;
                    let t = format!("{:.*}", n, frac);
                    out.push_str(&t[1..]);
                    i += n;
                } else {
                    out.push_str(t);
                }
            }
            Tok::Date(t) => {
                let k = t.as_bytes()[0];
                let n = t.len();
                match k {
                    b'y' => {
                        if n <= 2 {
                            out.push_str(&format!("{:02}", y % 100));
                        } else {
                            out.push_str(&format!("{:04}", y));
                        }
                    }
                    b'm' => {
                        // m right after h, or right before s, means minutes.
                        let prev = toks[..i].iter().rev().find(|x| matches!(x, Tok::Date(_) | Tok::Elapsed(..)));
                        let next = toks[i + 1..].iter().find(|x| matches!(x, Tok::Date(_)));
                        let after_h = matches!(prev, Some(Tok::Date(p)) if p.starts_with('h'))
                            || matches!(prev, Some(Tok::Elapsed('h', _)));
                        let before_s = matches!(next, Some(Tok::Date(p)) if p.starts_with('s'));
                        if (after_h || before_s) && n <= 2 {
                            if n == 2 {
                                out.push_str(&format!("{:02}", mi));
                            } else {
                                out.push_str(&mi.to_string());
                            }
                        } else {
                            let name = MONTHS[(m - 1).clamp(0, 11) as usize];
                            match n {
                                1 => out.push_str(&m.to_string()),
                                2 => out.push_str(&format!("{:02}", m)),
                                3 => out.push_str(&name[..3]),
                                5 => out.push_str(&name[..1]),
                                _ => out.push_str(name),
                            }
                        }
                    }
                    b'd' => {
                        let wd = weekday_of(day) as usize;
                        match n {
                            1 => out.push_str(&d.to_string()),
                            2 => out.push_str(&format!("{:02}", d)),
                            3 => out.push_str(&DAYS[wd][..3]),
                            _ => out.push_str(DAYS[wd]),
                        }
                    }
                    b'h' => {
                        let h = if ampm {
                            let x = hh % 12;
                            if x == 0 {
                                12
                            } else {
                                x
                            }
                        } else {
                            hh
                        };
                        if n >= 2 {
                            out.push_str(&format!("{:02}", h));
                        } else {
                            out.push_str(&h.to_string());
                        }
                    }
                    b's' => {
                        if n >= 2 {
                            out.push_str(&format!("{:02}", ss));
                        } else {
                            out.push_str(&ss.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Tok::Elapsed(k, n) => {
                let tot = v * 86400.0;
                let val = match k {
                    'h' => (tot / 3600.0).floor() as i64,
                    'm' => (tot / 60.0).floor() as i64,
                    _ => tot.round() as i64,
                };
                out.push_str(&format!("{:0w$}", val, w = *n));
            }
            Tok::AmPm(t) => {
                let pm = hh >= 12;
                if t.len() >= 5 {
                    out.push_str(if pm { "PM" } else { "AM" });
                } else {
                    out.push_str(if pm { "P" } else { "A" });
                }
            }
            Tok::Skip => out.push(' '),
            Tok::Digit(c) => out.push(*c),
            _ => {}
        }
        i += 1;
    }
    r.text = out;
    r
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn fmt_frac(sec: &Section, v: f64) -> Rendered {
    // "# ?/?" or "# ??/??" or "# ?/8"
    let slash = sec.toks.iter().position(|t| *t == Tok::Slash).unwrap_or(0);
    let den_digits = sec.toks[slash + 1..].iter().filter(|t| matches!(t, Tok::Digit(_))).count();
    let fixed: String = sec.toks[slash + 1..]
        .iter()
        .filter_map(|t| match t {
            Tok::Lit(l) if l.chars().all(|c| c.is_ascii_digit()) => Some(l.clone()),
            _ => None,
        })
        .collect();
    let has_int = sec.toks[..slash].iter().filter(|t| matches!(t, Tok::Digit(_))).count() > 1
        || sec.toks[..slash].iter().any(|t| matches!(t, Tok::Lit(l) if l == " "));
    let a = v.abs();
    let int = if has_int { a.floor() } else { 0.0 };
    let frac = a - int;
    let (mut n, mut d);
    if let Ok(fd) = fixed.parse::<i64>() {
        d = fd;
        n = (frac * d as f64).round() as i64;
    } else {
        let max_d = 10i64.pow(den_digits.max(1) as u32) - 1;
        let (mut best_n, mut best_d, mut best_err) = (0, 1, f64::MAX);
        for dd in 1..=max_d {
            let nn = (frac * dd as f64).round() as i64;
            let err = (frac - nn as f64 / dd as f64).abs();
            if err + 1e-12 < best_err {
                best_err = err;
                best_n = nn;
                best_d = dd;
                if err < 1e-10 {
                    break;
                }
            }
        }
        n = best_n;
        d = best_d;
    }
    let g = gcd(n.abs(), d).max(1);
    if fixed.is_empty() {
        n /= g;
        d /= g;
    }
    let mut out = String::new();
    if v < 0.0 {
        out.push('-');
    }
    let int = if n == d && has_int {
        n = 0;
        int + 1.0
    } else {
        int
    };
    if has_int {
        if int != 0.0 || n == 0 {
            out.push_str(&format!("{}", int as i64));
        }
        if n != 0 {
            if int != 0.0 {
                out.push(' ');
            }
            out.push_str(&format!("{n}/{d}"));
        }
    } else {
        out.push_str(&format!("{n}/{d}"));
    }
    Rendered { text: out, left: String::new(), color: sec.color }
}

fn fmt_number(sec: &Section, v: f64, neg_prefix: bool) -> Rendered {
    let toks = &sec.toks;
    let mut scale = 1.0;
    // Digits before and after the decimal point, and the exponent part.
    let point = toks.iter().position(|t| *t == Tok::Point);
    let exp = toks.iter().position(|t| matches!(t, Tok::Exp(_)));
    let end_int = point.or(exp).unwrap_or(toks.len());
    let last_digit = toks.iter().rposition(|t| matches!(t, Tok::Digit(_)));
    let mut min_int = 0;
    let mut grouped = false;
    let mut int_slots = 0;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Digit(c) if i < end_int => {
                int_slots += 1;
                if *c == '0' || *c == '?' {
                    min_int += 1;
                }
            }
            Tok::Thousands => {
                // A comma after the last digit scales by 1000; between digits it groups.
                if Some(i) > last_digit && point.map_or(true, |p| i < p || i > p) && last_digit.map_or(false, |l| i > l) {
                    scale /= 1000.0;
                } else if i < end_int {
                    if matches!(toks.get(i + 1), Some(Tok::Point)) || (point.is_none() && i + 1 >= toks.len()) {
                        scale /= 1000.0;
                    } else {
                        grouped = true;
                    }
                }
            }
            Tok::Percent => scale *= 100.0,
            _ => {}
        }
    }
    let frac_end = exp.unwrap_or(toks.len());
    let frac_slots: Vec<char> = match point {
        Some(p) => toks[p + 1..frac_end]
            .iter()
            .filter_map(|t| if let Tok::Digit(c) = t { Some(*c) } else { None })
            .collect(),
        None => vec![],
    };
    let dec = frac_slots.len();
    let a = v.abs() * scale;
    let mut exp_val = 0i32;
    let mut mant = a;
    if exp.is_some() && a != 0.0 {
        let step = int_slots.max(1) as i32;
        exp_val = a.log10().floor() as i32;
        exp_val = exp_val.div_euclid(step) * step;
        mant = a / 10f64.powi(exp_val);
    }
    let s = format!("{:.*}", dec, round_half_away(mant, dec as i32));
    let (int_s, frac_s) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), f.to_string()),
        None => (s.clone(), String::new()),
    };
    let int_s = if int_s == "0" && min_int == 0 { String::new() } else { int_s };
    let mut int_out: String = int_s.clone();
    while int_out.len() < min_int {
        int_out.insert(0, '0');
    }
    if grouped {
        int_out = group_int(&int_out);
    }
    // Trim optional trailing fraction digits.
    let mut frac_out: Vec<char> = frac_s.chars().collect();
    for (i, c) in frac_slots.iter().enumerate().rev() {
        if *c == '#' && frac_out.get(i) == Some(&'0') && i + 1 == frac_out.len() {
            frac_out.pop();
        } else if *c == '?' && frac_out.get(i) == Some(&'0') {
            frac_out[i] = ' ';
        } else {
            break;
        }
    }
    let mut out = String::new();
    let mut left = String::new();
    if neg_prefix {
        out.push('-');
    }
    let mut int_written = false;
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Lit(t) => out.push_str(t),
            Tok::Digit(_) if i < end_int => {
                if !int_written {
                    out.push_str(&int_out);
                    int_written = true;
                }
            }
            Tok::Point => {
                if !frac_out.is_empty() || frac_slots.is_empty() || frac_slots.iter().any(|c| *c == '0') {
                    out.push('.');
                }
                out.extend(frac_out.iter());
            }
            Tok::Percent => out.push('%'),
            Tok::Exp(plus) => {
                out.push('E');
                if exp_val < 0 {
                    out.push('-');
                } else if *plus {
                    out.push('+');
                }
                let mut n = 0;
                for t in &toks[i + 1..] {
                    if matches!(t, Tok::Digit(_)) {
                        n += 1;
                    }
                }
                out.push_str(&format!("{:0w$}", exp_val.abs(), w = n));
                i = toks.len();
            }
            Tok::Skip => out.push(' '),
            Tok::Fill(_) => {
                left = std::mem::take(&mut out);
            }
            _ => {}
        }
        i += 1;
    }
    Rendered { text: out, left, color: sec.color }
}

fn round_half_away(x: f64, d: i32) -> f64 {
    let m = 10f64.powi(d);
    let y = x * m;
    if !y.is_finite() {
        return x;
    }
    // Nudges binary noise so 1.005 style values round the way people expect.
    let r = (y + y.abs().max(1.0) * 1e-14).round();
    r / m
}

fn cond_ok(c: &(String, f64), v: f64) -> bool {
    match c.0.as_str() {
        "<" => v < c.1,
        "<=" => v <= c.1,
        ">" => v > c.1,
        ">=" => v >= c.1,
        "=" => v == c.1,
        "<>" => v != c.1,
        _ => false,
    }
}

pub fn format_num(code: &str, v: f64, width_hint: usize) -> Rendered {
    if code.eq_ignore_ascii_case("general") || code.is_empty() {
        return Rendered { text: general(v, width_hint.max(3)), ..Default::default() };
    }
    let secs = parse_sections(code);
    let n = secs.len();
    // Choose the section.
    let (sec, abs_sign): (&Section, bool);
    if secs.iter().any(|s| s.cond.is_some()) {
        let mut chosen = None;
        for s in &secs {
            match &s.cond {
                Some(c) => {
                    if cond_ok(c, v) {
                        chosen = Some(s);
                        break;
                    }
                }
                None => {
                    chosen = Some(s);
                    break;
                }
            }
        }
        sec = chosen.unwrap_or(&secs[0]);
        abs_sign = v < 0.0 && sec.cond.is_none();
    } else if n == 1 {
        sec = &secs[0];
        abs_sign = v < 0.0;
    } else if v > 0.0 || (v == 0.0 && n < 3) {
        sec = &secs[0];
        abs_sign = false;
    } else if v < 0.0 {
        sec = &secs[1];
        abs_sign = false;
    } else {
        sec = &secs[2];
        abs_sign = false;
    }
    if sec.is_date {
        return fmt_date(sec, v, false);
    }
    if sec.is_frac {
        return fmt_frac(sec, if abs_sign { v } else { v.abs() });
    }
    if sec.is_num {
        return fmt_number(sec, v, abs_sign);
    }
    // Literal-only section.
    if sec.toks.iter().any(|t| *t == Tok::Text) || sec.toks.is_empty() {
        if sec.toks.is_empty() {
            return Rendered { color: sec.color, ..Default::default() };
        }
        let txt = general(v, width_hint.max(3));
        return format_text_section(sec, &txt);
    }
    let mut r = format_text_section(sec, "");
    if abs_sign {
        r.text.insert(0, '-');
    }
    r
}

fn format_text_section(sec: &Section, s: &str) -> Rendered {
    let mut out = String::new();
    let mut left = String::new();
    for t in &sec.toks {
        match t {
            Tok::Lit(l) => out.push_str(l),
            Tok::Text => out.push_str(s),
            Tok::Skip => out.push(' '),
            Tok::Fill(_) => left = std::mem::take(&mut out),
            _ => {}
        }
    }
    Rendered { text: out, left, color: sec.color }
}

pub fn format_text(code: &str, s: &str) -> Rendered {
    if code.eq_ignore_ascii_case("general") || !code.contains('@') {
        let secs = if code.contains(';') { parse_sections(code) } else { vec![] };
        if secs.len() < 4 {
            return Rendered { text: s.to_string(), ..Default::default() };
        }
        return format_text_section(&secs[3], s);
    }
    let secs = parse_sections(code);
    let sec = secs.iter().rev().find(|x| x.toks.contains(&Tok::Text)).unwrap_or(&secs[0]);
    format_text_section(sec, s)
}

pub fn is_date_format(code: &str) -> bool {
    if code.eq_ignore_ascii_case("general") {
        return false;
    }
    parse_sections(code).first().map(|s| s.is_date).unwrap_or(false)
}

pub fn is_text_format(code: &str) -> bool {
    code == "@"
}

/// A parsed entry from the keyboard: its content, and the number format it suggests.
pub struct Typed {
    pub content: Content,
    pub fmt: Option<String>,
}

fn parse_clock(s: &str) -> Option<(f64, bool)> {
    // "12:30", "12:30:15", "1:30 PM". Returns day fraction and whether seconds were given.
    let low = s.trim().to_ascii_lowercase();
    let (body, ap) = if let Some(x) = low.strip_suffix("pm") {
        (x.trim(), Some(true))
    } else if let Some(x) = low.strip_suffix("am") {
        (x.trim(), Some(false))
    } else if let Some(x) = low.strip_suffix('p') {
        (x.trim(), Some(true))
    } else if let Some(x) = low.strip_suffix('a') {
        (x.trim(), Some(false))
    } else {
        (low.as_str(), None)
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() < 2 && ap.is_none() || parts.len() > 3 {
        return None;
    }
    if !parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit() || c == '.')) {
        return None;
    }
    let mut h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts.get(1).map_or(Some(0.0), |x| x.parse().ok())?;
    let sec: f64 = parts.get(2).map_or(Some(0.0), |x| x.parse().ok())?;
    if m >= 60.0 || sec >= 60.0 {
        return None;
    }
    if let Some(pm) = ap {
        if h > 12.0 {
            return None;
        }
        if h == 12.0 {
            h = 0.0;
        }
        if pm {
            h += 12.0;
        }
    }
    Some(((h * 3600.0 + m * 60.0 + sec) / 86400.0, parts.len() == 3))
}

fn month_from(s: &str) -> Option<i64> {
    let l = s.to_ascii_lowercase();
    if l.len() < 3 {
        return None;
    }
    MONTHS.iter().position(|m| m.to_ascii_lowercase().starts_with(&l)).map(|i| i as i64 + 1)
}

fn valid_ymd(y: i64, m: i64, d: i64) -> bool {
    if !(1..=12).contains(&m) || d < 1 {
        return false;
    }
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    d <= days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)
}

/// Parses a date in month/day/year order, ISO order, or with a month name.
pub fn parse_date(s: &str) -> Option<(f64, &'static str)> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let now_year = crate::eval::today_ymd().0;
    let sep = if t.contains('/') {
        '/'
    } else if t.contains('-') {
        '-'
    } else if t.contains(' ') {
        ' '
    } else {
        return None;
    };
    let t2 = t.replace(',', " ");
    let parts: Vec<&str> = t2.split(|c| c == sep || c == ' ').filter(|x| !x.is_empty()).collect();
    let digits = |x: &&str| x.chars().all(|c| c.is_ascii_digit());
    if parts.len() == 3 {
        if parts.iter().all(digits) {
            let (a, b, c): (i64, i64, i64) = (parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?);
            if parts[0].len() == 4 {
                if valid_ymd(a, b, c) {
                    return Some((ymd_to_serial(a, b, c), "m/d/yyyy"));
                }
                return None;
            }
            let y = if parts[2].len() <= 2 {
                if c < 30 {
                    2000 + c
                } else {
                    1900 + c
                }
            } else {
                c
            };
            if !(1900..=9999).contains(&y) {
                return None;
            }
            if valid_ymd(y, a, b) {
                return Some((ymd_to_serial(y, a, b), "m/d/yyyy"));
            }
            return None;
        }
        // 12-Mar-2024 or March 12 2024
        if let (Some(m), true) = (month_from(parts[1]), digits(&parts[0]) && digits(&parts[2])) {
            let d: i64 = parts[0].parse().ok()?;
            let y: i64 = parts[2].parse().ok()?;
            let y = if y < 100 { 2000 + y } else { y };
            if valid_ymd(y, m, d) {
                return Some((ymd_to_serial(y, m, d), "d-mmm-yy"));
            }
        }
        if let (Some(m), true) = (month_from(parts[0]), digits(&parts[1]) && digits(&parts[2])) {
            let d: i64 = parts[1].parse().ok()?;
            let y: i64 = parts[2].parse().ok()?;
            let y = if y < 100 { 2000 + y } else { y };
            if valid_ymd(y, m, d) {
                return Some((ymd_to_serial(y, m, d), "d-mmm-yy"));
            }
        }
        return None;
    }
    if parts.len() == 2 {
        if parts.iter().all(digits) && sep != ' ' {
            let (a, b): (i64, i64) = (parts[0].parse().ok()?, parts[1].parse().ok()?);
            if parts[1].len() == 4 && (1..=12).contains(&a) {
                return Some((ymd_to_serial(b, a, 1), "mmm-yy"));
            }
            if valid_ymd(now_year, a, b) {
                return Some((ymd_to_serial(now_year, a, b), "d-mmm"));
            }
            return None;
        }
        if let (Some(m), true) = (month_from(parts[1]), digits(&parts[0])) {
            let d: i64 = parts[0].parse().ok()?;
            if valid_ymd(now_year, m, d) {
                return Some((ymd_to_serial(now_year, m, d), "d-mmm"));
            }
        }
        if let (Some(m), true) = (month_from(parts[0]), digits(&parts[1])) {
            let d: i64 = parts[1].parse().ok()?;
            if parts[1].len() == 4 {
                return Some((ymd_to_serial(d, m, 1), "mmm-yy"));
            }
            if valid_ymd(now_year, m, d) {
                return Some((ymd_to_serial(now_year, m, d), "d-mmm"));
            }
        }
    }
    None
}

/// Reads a plain number with optional thousands commas. Used for typed input and VALUE().
pub fn parse_plain_number(t: &str) -> Option<f64> {
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    let first = t.chars().next()?;
    if !(first.is_ascii_digit() || first == '.' || first == '-' || first == '+') {
        return None;
    }
    if t.contains(',') {
        // Every comma group must be followed by digits; "1,2" is not accepted as a number.
        let (int, frac) = t.split_once('.').map_or((t, None), |(a, b)| (a, Some(b)));
        let groups: Vec<&str> = int.trim_start_matches(['-', '+']).split(',').collect();
        if groups.len() < 2 || groups[1..].iter().any(|g| g.len() != 3) || groups[0].is_empty() {
            return None;
        }
        let mut s: String = int.chars().filter(|c| *c != ',').collect();
        if let Some(f) = frac {
            s.push('.');
            s.push_str(f);
        }
        return s.parse().ok().filter(|x: &f64| x.is_finite());
    }
    if t.chars().any(|c| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))) {
        return None;
    }
    t.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Turns what someone typed into cell content, the way a spreadsheet interprets entries.
pub fn parse_input(input: &str) -> Typed {
    let t = input.trim();
    if t.is_empty() {
        return Typed { content: Content::Empty, fmt: None };
    }
    if let Some(n) = parse_plain_number(t) {
        return Typed { content: Content::Num(n), fmt: None };
    }
    let up = t.to_ascii_uppercase();
    if up == "TRUE" || up == "FALSE" {
        return Typed { content: Content::Bool(up == "TRUE"), fmt: None };
    }
    if let Some(e) = crate::model::XlErr::parse(t) {
        return Typed { content: Content::Err(e), fmt: None };
    }
    // Percent
    if let Some(b) = t.strip_suffix('%') {
        if let Some(n) = parse_plain_number(b) {
            let f = if b.contains('.') { "0.00%" } else { "0%" };
            return Typed { content: Content::Num(n / 100.0), fmt: Some(f.into()) };
        }
    }
    // Currency, with the sign on either side of the symbol, and accounting parentheses.
    let (neg_paren, t2) = match t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(x) => (true, x.trim()),
        None => (false, t),
    };
    for sym in ['$', '€', '£', '¥'] {
        let body = t2.strip_prefix(sym).map(|b| (false, b)).or(t2.strip_prefix('-').and_then(|x| x.strip_prefix(sym)).map(|b| (true, b)));
        if let Some((neg, b)) = body {
            if let Some(n) = parse_plain_number(b.trim()) {
                let f = if b.contains('.') {
                    format!("{sym}#,##0.00")
                } else {
                    format!("{sym}#,##0")
                };
                let v = if neg || neg_paren { -n } else { n };
                return Typed { content: Content::Num(v), fmt: Some(f) };
            }
        }
    }
    if neg_paren {
        if let Some(n) = parse_plain_number(t2) {
            return Typed { content: Content::Num(-n), fmt: None };
        }
    }
    // "1 1/2" mixed fractions and "0 1/4".
    if let Some((i, f)) = t.split_once(' ') {
        if let (Ok(iv), Some((a, b))) = (i.parse::<i64>(), f.trim().split_once('/')) {
            if let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>()) {
                if b != 0.0 {
                    let v = iv.abs() as f64 + a / b;
                    return Typed {
                        content: Content::Num(if iv < 0 { -v } else { v }),
                        fmt: Some("# ?/?".into()),
                    };
                }
            }
        }
    }
    if t.contains(':') || t.to_ascii_lowercase().ends_with(" am") || t.to_ascii_lowercase().ends_with(" pm") {
        // Date and time together.
        if let Some(i) = t.rfind(' ') {
            let head = &t[..i];
            let idx = if head.to_ascii_lowercase().ends_with(|c: char| c.is_ascii_digit()) && head.contains(' ') && !head.contains(':') {
                None
            } else {
                head.find(' ')
            };
            let (dp, tp) = match idx {
                Some(j) if head.contains(':') => (&t[..j], &t[j + 1..]),
                _ => (head, &t[i + 1..]),
            };
            if let (Some((d, _)), Some((c, _))) = (parse_date(dp), parse_clock(tp)) {
                return Typed { content: Content::Num(d + c), fmt: Some("m/d/yyyy h:mm".into()) };
            }
        }
        if let Some((c, sec)) = parse_clock(t) {
            let ampm = t.to_ascii_lowercase().contains('m') || t.to_ascii_lowercase().ends_with('p');
            let f = match (ampm, sec) {
                (true, true) => "h:mm:ss AM/PM",
                (true, false) => "h:mm AM/PM",
                (false, true) => "h:mm:ss",
                (false, false) => "h:mm",
            };
            return Typed { content: Content::Num(c), fmt: Some(f.into()) };
        }
    }
    if let Some((d, f)) = parse_date(t) {
        return Typed { content: Content::Num(d), fmt: Some(f.into()) };
    }
    Typed { content: Content::Text(input.into()), fmt: None }
}

/// Excel-compatible VALUE(): numbers, percents, currency, dates and times from text.
pub fn text_to_number(s: &str) -> Option<f64> {
    match parse_input(s).content {
        Content::Num(n) => Some(n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(code: &str, v: f64) -> String {
        format_num(code, v, 11).text
    }

    #[test]
    fn numbers() {
        assert_eq!(f("0.00", 3.14159), "3.14");
        assert_eq!(f("#,##0", 1234567.0), "1,234,567");
        assert_eq!(f("#,##0.00", -1234.5), "-1,234.50");
        assert_eq!(f("0%", 0.256), "26%");
        assert_eq!(f("0.00%", 0.25), "25.00%");
        assert_eq!(f("$#,##0.00", 1234.5), "$1,234.50");
        assert_eq!(f("0.00E+00", 12345.0), "1.23E+04");
        assert_eq!(f("#,##0.00;[Red](#,##0.00)", -5.0), "(5.00)");
        assert_eq!(f("000", 7.0), "007");
        assert_eq!(f("#,##0,", 12000.0), "12");
    }

    #[test]
    fn accounting() {
        let r = format_num("_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)", 1234.5, 11);
        assert_eq!(r.left.trim(), "$");
        assert_eq!(r.text.trim(), "1,234.50");
    }

    #[test]
    fn dates() {
        assert_eq!(ymd_to_serial(2024, 1, 1), 45292.0);
        assert_eq!(serial_to_ymd(45292.0), (2024, 1, 1));
        assert_eq!(ymd_to_serial(1900, 1, 1), 1.0);
        assert_eq!(ymd_to_serial(1900, 3, 1), 61.0);
        assert_eq!(f("m/d/yyyy", 45292.0), "1/1/2024");
        assert_eq!(f("yyyy-mm-dd", 45292.0), "2024-01-01");
        assert_eq!(f("mmm d, yyyy", 45292.0), "Jan 1, 2024");
        assert_eq!(f("dddd", 45292.0), "Monday");
        assert_eq!(f("h:mm AM/PM", 0.75), "6:00 PM");
        assert_eq!(f("[h]:mm", 1.5), "36:00");
    }

    #[test]
    fn fractions() {
        assert_eq!(f("# ?/?", 1.5), "1 1/2");
        assert_eq!(f("# ??/??", 0.25), "1/4");
    }

    #[test]
    fn general_fmt() {
        assert_eq!(general(1.0 / 3.0, 11), "0.333333333");
        assert_eq!(general(1234.5, 11), "1234.5");
        assert_eq!(general(1e12, 11), "1E+12");
    }

    #[test]
    fn input() {
        assert_eq!(parse_input("1,234.5").content, Content::Num(1234.5));
        assert_eq!(parse_input("50%").content, Content::Num(0.5));
        assert_eq!(parse_input("$1,000").content, Content::Num(1000.0));
        assert_eq!(parse_input("1/15/2024").content, Content::Num(45306.0));
        assert_eq!(parse_input("TRUE").content, Content::Bool(true));
        assert_eq!(parse_input("hello").content, Content::Text("hello".into()));
        assert_eq!(parse_input("12:30").content, Content::Num(0.5208333333333334));
    }
}
