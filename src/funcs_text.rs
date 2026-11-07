//! Text, date and time, and information functions.

use crate::eval::*;
use crate::formula;
use crate::funcs::*;
use crate::funcs_math::round_to;
use crate::model::*;
use crate::numfmt;

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn find_in(hay: &[char], needle: &[char], start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start);
    }
    if hay.len() < needle.len() {
        return None;
    }
    (start..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

fn fixed_text(x: f64, d: i32, nocomma: bool) -> String {
    let code = if nocomma {
        if d > 0 { format!("0.{}", "0".repeat(d as usize)) } else { "0".to_string() }
    } else if d > 0 {
        format!("#,##0.{}", "0".repeat(d as usize))
    } else {
        "#,##0".to_string()
    };
    numfmt::format_num(&code, round_to(x, d, 0), 20).text
}

fn textsplit_pieces(s: &str, delims: &[String], ignore_empty: bool) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut rest = s;
    'o: while !rest.is_empty() {
        for d in delims {
            if !d.is_empty() && rest.starts_with(d.as_str()) {
                out.push(std::mem::take(&mut cur));
                rest = &rest[d.len()..];
                continue 'o;
            }
        }
        let ch = rest.chars().next().unwrap();
        cur.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out.push(cur);
    if ignore_empty {
        out.retain(|x| !x.is_empty());
    }
    out
}

pub fn text(r: &mut Reg) {
    r.cat = "Text";
    r.s("LEN", 1, 1, "text", "Number of characters", |_, a| Ok(Value::Num(t(a, 0)?.chars().count() as f64)));
    r.s("LOWER", 1, 1, "text", "Converts to lowercase", |_, a| txt(t(a, 0)?.to_lowercase()));
    r.s("UPPER", 1, 1, "text", "Converts to uppercase", |_, a| txt(t(a, 0)?.to_uppercase()));
    r.s("PROPER", 1, 1, "text", "Capitalizes each word", |_, a| {
        let mut out = String::new();
        let mut up = true;
        for ch in t(a, 0)?.chars() {
            if up { out.extend(ch.to_uppercase()); } else { out.extend(ch.to_lowercase()); }
            up = !ch.is_alphabetic();
        }
        txt(out)
    });
    r.s("TRIM", 1, 1, "text", "Removes extra spaces", |_, a| txt(t(a, 0)?.split(' ').filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ")));
    r.s("CLEAN", 1, 1, "text", "Removes non-printable characters", |_, a| txt(t(a, 0)?.chars().filter(|c| (*c as u32) >= 32).collect::<String>()));
    r.s("LEFT", 1, 2, "text, [num_chars]", "Leftmost characters", |_, a| {
        let k = nd(a, 1, 1.0)?;
        if k < 0.0 { return Err(XlErr::Value); }
        txt(t(a, 0)?.chars().take(k as usize).collect::<String>())
    });
    r.s("RIGHT", 1, 2, "text, [num_chars]", "Rightmost characters", |_, a| {
        let k = nd(a, 1, 1.0)?;
        if k < 0.0 { return Err(XlErr::Value); }
        let c = chars(&t(a, 0)?);
        txt(c[c.len().saturating_sub(k as usize)..].iter().collect::<String>())
    });
    r.s("MID", 3, 3, "text, start_num, num_chars", "Characters from the middle of text", |_, a| {
        let (st, k) = (n(a, 1)?, n(a, 2)?);
        if st < 1.0 || k < 0.0 { return Err(XlErr::Value); }
        txt(t(a, 0)?.chars().skip(st as usize - 1).take(k as usize).collect::<String>())
    });
    r.s("REPT", 2, 2, "text, number_times", "Repeats text", |_, a| {
        let k = n(a, 1)?;
        if k < 0.0 { return Err(XlErr::Value); }
        let s = t(a, 0)?;
        if s.len() as f64 * k > 32767.0 { return Err(XlErr::Value); }
        txt(s.repeat(k as usize))
    });
    r.s("FIND", 2, 3, "find_text, within_text, [start_num]", "Finds text, case-sensitive", |_, a| {
        let st = nd(a, 2, 1.0)?;
        if st < 1.0 { return Err(XlErr::Value); }
        find_in(&chars(&t(a, 1)?), &chars(&t(a, 0)?), st as usize - 1).map(|i| Value::Num(i as f64 + 1.0)).ok_or(XlErr::Value)
    });
    r.s("SEARCH", 2, 3, "find_text, within_text, [start_num]", "Finds text with wildcards, ignoring case", |_, a| {
        let st = nd(a, 2, 1.0)?;
        if st < 1.0 { return Err(XlErr::Value); }
        let hay = chars(&t(a, 1)?.to_lowercase());
        let pat = chars(&t(a, 0)?.to_lowercase());
        if pat.iter().any(|c| matches!(c, '*' | '?')) {
            let mut p2 = pat.clone();
            p2.push('*');
            for i in (st as usize - 1)..=hay.len() {
                if wild(&p2, &hay[i..]) { return Ok(Value::Num(i as f64 + 1.0)); }
            }
            return Err(XlErr::Value);
        }
        find_in(&hay, &pat, st as usize - 1).map(|i| Value::Num(i as f64 + 1.0)).ok_or(XlErr::Value)
    });
    r.s("REPLACE", 4, 4, "old_text, start_num, num_chars, new_text", "Replaces characters at a position", |_, a| {
        let c = chars(&t(a, 0)?);
        let (st, k) = (n(a, 1)? as usize, n(a, 2)? as usize);
        if st < 1 { return Err(XlErr::Value); }
        let st = (st - 1).min(c.len());
        let end = (st + k).min(c.len());
        let mut out: String = c[..st].iter().collect();
        out.push_str(&t(a, 3)?);
        out.extend(c[end..].iter());
        txt(out)
    });
    r.s("SUBSTITUTE", 3, 4, "text, old_text, new_text, [instance_num]", "Substitutes new text for old text", |_, a| {
        let (s, old, new) = (t(a, 0)?, t(a, 1)?, t(a, 2)?);
        if old.is_empty() { return txt(s); }
        if let Some(Value::Num(k)) = a.get(3) {
            let mut out = String::new();
            let mut rest = s.as_str();
            let mut i = 0;
            while let Some(p) = rest.find(&old) {
                i += 1;
                if i == *k as i64 {
                    out.push_str(&rest[..p]);
                    out.push_str(&new);
                    out.push_str(&rest[p + old.len()..]);
                    return txt(out);
                }
                out.push_str(&rest[..p + old.len()]);
                rest = &rest[p + old.len()..];
            }
            out.push_str(rest);
            return txt(out);
        }
        txt(s.replace(&old, &new))
    });
    r.s("EXACT", 2, 2, "text1, text2", "Checks if two texts are identical", |_, a| Ok(Value::Bool(t(a, 0)? == t(a, 1)?)));
    r.s("CHAR", 1, 1, "number", "Character for a code number", |_, a| {
        let x = n(a, 0)? as u32;
        if !(1..=255).contains(&x) { return Err(XlErr::Value); }
        txt(char::from_u32(x).unwrap_or(' ').to_string())
    });
    r.s("UNICHAR", 1, 1, "number", "Unicode character for a number", |_, a| char::from_u32(n(a, 0)? as u32).map(|c| Value::text(c.to_string())).ok_or(XlErr::Value));
    r.s("CODE", 1, 1, "text", "Code of the first character", |_, a| t(a, 0)?.chars().next().map(|c| Value::Num(c as u32 as f64)).ok_or(XlErr::Value));
    r.alias("UNICODE", "CODE");
    r.s("VALUE", 1, 1, "text", "Converts text to a number", |_, a| {
        match &a[0] {
            Value::Num(x) => Ok(Value::Num(*x)),
            Value::Empty => Ok(Value::Num(0.0)),
            v => numfmt::text_to_number(&to_text(v)?).map(Value::Num).ok_or(XlErr::Value),
        }
    });
    r.s("NUMBERVALUE", 1, 3, "text, [decimal_separator], [group_separator]", "Converts text to a number with given separators", |_, a| {
        let s = t(a, 0)?;
        let (d, g) = (td(a, 1, ".")?, td(a, 2, ",")?);
        let s: String = s.replace(&g, "").replace(&d, ".").chars().filter(|c| !c.is_whitespace()).collect();
        if s.is_empty() { return Ok(Value::Num(0.0)); }
        numfmt::text_to_number(&s).map(Value::Num).ok_or(XlErr::Value)
    });
    r.s("TEXT", 2, 2, "value, format_text", "Formats a value as text", |_, a| {
        let code = t(a, 1)?;
        match &a[0] {
            Value::Text(s) => match numfmt::text_to_number(s) {
                Some(x) => txt(numfmt::format_num(&code, x, 15).text),
                None => txt(numfmt::format_text(&code, s).text),
            },
            v => { let r = numfmt::format_num(&code, to_num(v)?, 15); txt(format!("{}{}", r.left, r.text)) }
        }
    });
    r.s("FIXED", 1, 3, "number, [decimals], [no_commas]", "Formats a number as text with fixed decimals", |_, a| txt(fixed_text(n(a, 0)?, nd(a, 1, 2.0)? as i32, bd(a, 2, false)?)));
    r.s("DOLLAR", 1, 2, "number, [decimals]", "Formats a number as currency text", |_, a| {
        let (x, d) = (n(a, 0)?, nd(a, 1, 2.0)? as i32);
        let s = fixed_text(x.abs(), d, false);
        txt(if x < 0.0 { format!("(${s})") } else { format!("${s}") })
    });
    r.s("T", 1, 1, "value", "Returns text only, otherwise empty", |_, a| match &a[0] { Value::Text(x) => Ok(Value::Text(x.clone())), Value::Err(e) => Err(*e), _ => txt("") });
    r.a("CONCAT", 1, MANY, "text1, [text2], ...", "Joins text from ranges", |c, a| {
        let mut s = String::new();
        for v in a {
            for x in c.flat(v) { s.push_str(&to_text(&x)?); }
        }
        txt(s)
    });
    r.a("CONCATENATE", 1, MANY, "text1, [text2], ...", "Joins several text items", |c, a| {
        let vals: Vec<Value> = a.iter().map(|x| c.arrayish(x.clone())).collect();
        if vals.iter().any(|v| matches!(v, Value::Array(_))) {
            let mut acc = Value::text("");
            for v in vals { acc = c.binary(formula::BinOp::Concat, acc, v); }
            return Ok(acc);
        }
        let mut s = String::new();
        for v in &vals { s.push_str(&to_text(v)?); }
        txt(s)
    });
    r.a("TEXTJOIN", 3, MANY, "delimiter, ignore_empty, text1, [text2], ...", "Joins text with a delimiter", |c, a| {
        let d = c.flat(&a[0]).iter().map(to_text).collect::<Result<Vec<_>, _>>()?;
        let d0 = d.first().cloned().unwrap_or_default();
        let ig = to_bool(&c.arrayish(a[1].clone()))?;
        let mut parts = vec![];
        for v in &a[2..] {
            for x in c.flat(v) {
                let s = to_text(&x)?;
                if !(ig && s.is_empty()) { parts.push(s); }
            }
        }
        txt(parts.join(&d0))
    });
    r.a("TEXTSPLIT", 2, 6, "text, col_delimiter, [row_delimiter], [ignore_empty], [match_mode], [pad_with]", "Splits text into rows and columns", |c, a| {
        let s = to_text(&c.arrayish(a[0].clone()))?;
        let cold: Vec<String> = c.flat(&a[1]).iter().filter_map(|x| to_text(x).ok()).collect();
        let rowd: Vec<String> = match a.get(2) { Some(v) => c.flat(v).iter().filter_map(|x| to_text(x).ok()).collect(), None => vec![] };
        let ig = bd(a, 3, false)?;
        let rows = if rowd.iter().any(|x| !x.is_empty()) { textsplit_pieces(&s, &rowd, ig) } else { vec![s] };
        let grid: Vec<Vec<String>> = rows.iter().map(|x| textsplit_pieces(x, &cold, ig)).collect();
        let cols = grid.iter().map(|x| x.len()).max().unwrap_or(1).max(1);
        let pad = a.get(5).cloned().unwrap_or(Value::Err(XlErr::NA));
        let mut data = vec![];
        for row in &grid {
            for i in 0..cols {
                data.push(row.get(i).map_or(pad.clone(), Value::text));
            }
        }
        Ok(Array::new(grid.len(), cols, data))
    });
    r.s("TEXTBEFORE", 2, 3, "text, delimiter, [instance_num]", "Text before a delimiter", |_, a| {
        let (s, d, k) = (t(a, 0)?, t(a, 1)?, nd(a, 2, 1.0)? as i64);
        let idx: Vec<usize> = s.match_indices(&d).map(|x| x.0).collect();
        let i = if k > 0 { idx.get(k as usize - 1) } else { idx.len().checked_sub((-k) as usize).and_then(|j| idx.get(j)) };
        i.map(|i| Value::text(&s[..*i])).ok_or(XlErr::NA)
    });
    r.s("TEXTAFTER", 2, 3, "text, delimiter, [instance_num]", "Text after a delimiter", |_, a| {
        let (s, d, k) = (t(a, 0)?, t(a, 1)?, nd(a, 2, 1.0)? as i64);
        let idx: Vec<usize> = s.match_indices(&d).map(|x| x.0).collect();
        let i = if k > 0 { idx.get(k as usize - 1) } else { idx.len().checked_sub((-k) as usize).and_then(|j| idx.get(j)) };
        i.map(|i| Value::text(&s[*i + d.len()..])).ok_or(XlErr::NA)
    });
    r.s("VALUETOTEXT", 1, 2, "value, [format]", "Value as text", |_, a| txt(to_text(&a[0])?));
    r.a("ARRAYTOTEXT", 1, 2, "array, [format]", "Array as text", |c, a| {
        let parts = c.flat(&a[0]).iter().map(to_text).collect::<Result<Vec<_>, _>>()?;
        txt(parts.join(", "))
    });
}

fn month_add(s: f64, m: i64) -> (i64, i64, i64) {
    let (y, mo, d) = numfmt::serial_to_ymd(s);
    let tot = y * 12 + mo - 1 + m;
    let (ny, nm) = (tot.div_euclid(12), tot.rem_euclid(12) + 1);
    let last = numfmt::serial_to_ymd(numfmt::ymd_to_serial(ny, nm + 1, 0)).2;
    (ny, nm, d.min(last))
}

pub fn yearfrac(a: f64, b: f64, basis: i64) -> f64 {
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    match basis {
        1 => {
            let (y1, ..) = numfmt::serial_to_ymd(a);
            let (y2, ..) = numfmt::serial_to_ymd(b);
            let days = b - a;
            let year_len = if y1 == y2 {
                numfmt::ymd_to_serial(y1 + 1, 1, 1) - numfmt::ymd_to_serial(y1, 1, 1)
            } else {
                let mut tot = 0.0;
                for y in y1..=y2 {
                    tot += numfmt::ymd_to_serial(y + 1, 1, 1) - numfmt::ymd_to_serial(y, 1, 1);
                }
                tot / (y2 - y1 + 1) as f64
            };
            days / year_len
        }
        2 => (b - a) / 360.0,
        3 => (b - a) / 365.0,
        4 => days360(a, b, true) / 360.0,
        _ => days360(a, b, false) / 360.0,
    }
}

pub fn days360(a: f64, b: f64, euro: bool) -> f64 {
    let (y1, m1, mut d1) = numfmt::serial_to_ymd(a);
    let (y2, m2, mut d2) = numfmt::serial_to_ymd(b);
    if euro {
        d1 = d1.min(30);
        d2 = d2.min(30);
    } else {
        let last1 = numfmt::serial_to_ymd(numfmt::ymd_to_serial(y1, m1 + 1, 0)).2;
        if d1 == last1 {
            d1 = 30;
        }
        if d2 == 31 && d1 >= 30 {
            d2 = 30;
        }
    }
    ((y2 - y1) * 360 + (m2 - m1) * 30 + d2 - d1) as f64
}

fn weekend_mask(v: Option<&Value>) -> Result<[bool; 7], XlErr> {
    // Index 0 is Sunday.
    let mut m = [false; 7];
    match v {
        None | Some(Value::Empty) => {
            m[0] = true;
            m[6] = true;
        }
        Some(Value::Text(s)) => {
            if s.len() != 7 {
                return Err(XlErr::Value);
            }
            for (i, c) in s.chars().enumerate() {
                m[(i + 1) % 7] = c == '1';
            }
        }
        Some(x) => {
            let k = to_num(x)? as i64;
            match k {
                1..=7 => {
                    m[(k as usize + 5) % 7] = true;
                    m[(k as usize + 6) % 7] = true;
                }
                11..=17 => m[(k as usize - 10) % 7] = true,
                _ => return Err(XlErr::Num),
            }
        }
    }
    Ok(m)
}

fn networkdays(c: &Cx, a: f64, b: f64, wk: [bool; 7], hol: Option<&Value>) -> Result<f64, XlErr> {
    let holidays = match hol {
        Some(v) => c.nums(std::slice::from_ref(v))?,
        None => vec![],
    };
    let (lo, hi, sign) = if a <= b { (a, b, 1.0) } else { (b, a, -1.0) };
    let mut count = 0.0;
    let mut d = lo.floor();
    while d <= hi {
        if !wk[numfmt::weekday_of(d) as usize] && !holidays.contains(&d) {
            count += 1.0;
        }
        d += 1.0;
    }
    Ok(count * sign)
}

fn workday(c: &Cx, st: f64, days: f64, wk: [bool; 7], hol: Option<&Value>) -> R {
    if wk.iter().all(|x| *x) {
        return Err(XlErr::Value);
    }
    let holidays = match hol {
        Some(v) => c.nums(std::slice::from_ref(v))?,
        None => vec![],
    };
    let step = if days >= 0.0 { 1.0 } else { -1.0 };
    let mut left = days.abs().trunc();
    let mut d = st.floor();
    while left > 0.0 {
        d += step;
        if !wk[numfmt::weekday_of(d) as usize] && !holidays.contains(&d) {
            left -= 1.0;
        }
    }
    Ok(Value::Num(d))
}

fn iso_week(s: f64) -> f64 {
    let wd = (numfmt::weekday_of(s) + 6) % 7; // Monday = 0
    let thu = s.floor() - wd as f64 + 3.0;
    let (y, ..) = numfmt::serial_to_ymd(thu);
    let jan1 = numfmt::ymd_to_serial(y, 1, 1);
    ((thu - jan1) / 7.0).floor() + 1.0
}

pub fn date(r: &mut Reg) {
    r.cat = "Date & Time";
    r.a("TODAY", 0, 0, "", "Today's date", |_, _| Ok(Value::Num(now_serial().floor())));
    r.a("NOW", 0, 0, "", "Current date and time", |_, _| Ok(Value::Num(now_serial())));
    r.s("DATE", 3, 3, "year, month, day", "Serial number for a date", |_, a| {
        let mut y = n(a, 0)?.trunc() as i64;
        if (0..1900).contains(&y) { y += 1900; }
        if y < 0 { return Err(XlErr::Num); }
        let v = numfmt::ymd_to_serial(y, n(a, 1)?.trunc() as i64, n(a, 2)?.trunc() as i64);
        if v < 0.0 { Err(XlErr::Num) } else { Ok(Value::Num(v)) }
    });
    r.s("TIME", 3, 3, "hour, minute, second", "Serial fraction for a time", |_, a| {
        let v = (n(a, 0)?.trunc() * 3600.0 + n(a, 1)?.trunc() * 60.0 + n(a, 2)?.trunc()) / 86400.0;
        if v < 0.0 { Err(XlErr::Num) } else { ok(v.fract()) }
    });
    r.s("YEAR", 1, 1, "serial_number", "Year of a date", |_, a| Ok(Value::Num(numfmt::serial_to_ymd(n(a, 0)?).0 as f64)));
    r.s("MONTH", 1, 1, "serial_number", "Month of a date", |_, a| Ok(Value::Num(numfmt::serial_to_ymd(n(a, 0)?).1 as f64)));
    r.s("DAY", 1, 1, "serial_number", "Day of the month", |_, a| Ok(Value::Num(numfmt::serial_to_ymd(n(a, 0)?).2 as f64)));
    fn secs(a: &[Value]) -> Result<f64, XlErr> { Ok((n(a, 0)?.fract() * 86400.0).round()) }
    r.s("HOUR", 1, 1, "serial_number", "Hour of a time", |_, a| Ok(Value::Num((secs(a)? / 3600.0).floor() % 24.0)));
    r.s("MINUTE", 1, 1, "serial_number", "Minute of a time", |_, a| Ok(Value::Num(((secs(a)? % 3600.0) / 60.0).floor())));
    r.s("SECOND", 1, 1, "serial_number", "Second of a time", |_, a| Ok(Value::Num(secs(a)? % 60.0)));
    r.s("WEEKDAY", 1, 2, "serial_number, [return_type]", "Day of the week", |_, a| {
        let wd = numfmt::weekday_of(n(a, 0)?);
        let ty = nd(a, 1, 1.0)? as i64;
        let v = match ty {
            1 | 17 => wd + 1,
            2 | 11 => (wd + 6) % 7 + 1,
            3 => (wd + 6) % 7,
            12..=16 => (wd + 7 - (ty - 10)).rem_euclid(7) + 1,
            _ => return Err(XlErr::Num),
        };
        Ok(Value::Num(v as f64))
    });
    r.s("WEEKNUM", 1, 2, "serial_number, [return_type]", "Week number of the year", |_, a| {
        let s = n(a, 0)?.floor();
        let ty = nd(a, 1, 1.0)? as i64;
        if ty == 21 { return Ok(Value::Num(iso_week(s))); }
        let start = match ty { 1 | 17 => 0, 2 | 11 => 1, 12..=16 => ty - 10, _ => return Err(XlErr::Num) };
        let (y, ..) = numfmt::serial_to_ymd(s);
        let jan1 = numfmt::ymd_to_serial(y, 1, 1);
        let off = (numfmt::weekday_of(jan1) - start).rem_euclid(7) as f64;
        Ok(Value::Num(((s - jan1 + off) / 7.0).floor() + 1.0))
    });
    r.s("ISOWEEKNUM", 1, 1, "date", "ISO week number", |_, a| Ok(Value::Num(iso_week(n(a, 0)?))));
    r.s("DAYS", 2, 2, "end_date, start_date", "Days between two dates", |_, a| ok(n(a, 0)?.floor() - n(a, 1)?.floor()));
    r.s("DAYS360", 2, 3, "start_date, end_date, [method]", "Days between dates on a 360-day year", |_, a| ok(days360(n(a, 0)?, n(a, 1)?, bd(a, 2, false)?)));
    r.s("EDATE", 2, 2, "start_date, months", "Date a number of months away", |_, a| {
        let (y, m, d) = month_add(n(a, 0)?, n(a, 1)?.trunc() as i64);
        Ok(Value::Num(numfmt::ymd_to_serial(y, m, d)))
    });
    r.s("EOMONTH", 2, 2, "start_date, months", "Last day of a month", |_, a| {
        let (y, m, _) = month_add(n(a, 0)?, n(a, 1)?.trunc() as i64);
        Ok(Value::Num(numfmt::ymd_to_serial(y, m + 1, 0)))
    });
    r.s("DATEVALUE", 1, 1, "date_text", "Converts date text to a serial number", |_, a| {
        let s = t(a, 0)?;
        let p = numfmt::parse_input(&s);
        match p.content { Content::Num(x) if p.fmt.is_some() => Ok(Value::Num(x.floor())), _ => Err(XlErr::Value) }
    });
    r.s("TIMEVALUE", 1, 1, "time_text", "Converts time text to a serial fraction", |_, a| {
        let s = t(a, 0)?;
        let p = numfmt::parse_input(&s);
        match p.content { Content::Num(x) if p.fmt.is_some() => ok(x.fract()), _ => Err(XlErr::Value) }
    });
    r.s("DATEDIF", 3, 3, "start_date, end_date, unit", "Difference between two dates", |_, a| {
        let (s, e) = (n(a, 0)?.floor(), n(a, 1)?.floor());
        if s > e { return Err(XlErr::Num); }
        let (y1, m1, d1) = numfmt::serial_to_ymd(s);
        let (y2, m2, d2) = numfmt::serial_to_ymd(e);
        let months = (y2 - y1) * 12 + (m2 - m1) - (d2 < d1) as i64;
        let v = match t(a, 2)?.to_ascii_uppercase().as_str() {
            "D" => e - s,
            "M" => months as f64,
            "Y" => (months / 12) as f64,
            "YM" => (months % 12) as f64,
            "YD" => {
                let base = if (m2, d2) >= (m1, d1) { y2 } else { y2 - 1 };
                e - numfmt::ymd_to_serial(base, m1, d1)
            }
            "MD" => {
                if d2 >= d1 { (d2 - d1) as f64 } else {
                    let prev = numfmt::ymd_to_serial(y2, m2, 0);
                    let (_, _, ld) = numfmt::serial_to_ymd(prev);
                    (ld - d1 + d2) as f64
                }
            }
            _ => return Err(XlErr::Num),
        };
        Ok(Value::Num(v))
    });
    r.s("YEARFRAC", 2, 3, "start_date, end_date, [basis]", "Fraction of the year between two dates", |_, a| ok(yearfrac(n(a, 0)?, n(a, 1)?, nd(a, 2, 0.0)? as i64)));
    r.a("NETWORKDAYS", 2, 3, "start_date, end_date, [holidays]", "Working days between two dates", |c, a| {
        let (x, y) = (to_num(&c.arrayish(a[0].clone()))?, to_num(&c.arrayish(a[1].clone()))?);
        ok(networkdays(c, x, y, weekend_mask(None)?, a.get(2))?)
    });
    r.a("NETWORKDAYS.INTL", 2, 4, "start_date, end_date, [weekend], [holidays]", "Working days with custom weekends", |c, a| {
        let (x, y) = (to_num(&c.arrayish(a[0].clone()))?, to_num(&c.arrayish(a[1].clone()))?);
        ok(networkdays(c, x, y, weekend_mask(a.get(2))?, a.get(3))?)
    });
    r.a("WORKDAY", 2, 3, "start_date, days, [holidays]", "Date a number of working days away", |c, a| {
        let (x, y) = (to_num(&c.arrayish(a[0].clone()))?, to_num(&c.arrayish(a[1].clone()))?);
        workday(c, x, y, weekend_mask(None)?, a.get(2))
    });
    r.a("WORKDAY.INTL", 2, 4, "start_date, days, [weekend], [holidays]", "Working day with custom weekends", |c, a| {
        let (x, y) = (to_num(&c.arrayish(a[0].clone()))?, to_num(&c.arrayish(a[1].clone()))?);
        workday(c, x, y, weekend_mask(a.get(2))?, a.get(3))
    });
}

pub fn info(r: &mut Reg) {
    r.cat = "Information";
    r.s("ISBLANK", 1, 1, "value", "TRUE if the cell is empty", |_, a| Ok(Value::Bool(a[0] == Value::Empty)));
    r.s("ISERR", 1, 1, "value", "TRUE for any error except #N/A", |_, a| Ok(Value::Bool(matches!(a[0], Value::Err(e) if e != XlErr::NA))));
    r.s("ISERROR", 1, 1, "value", "TRUE for any error", |_, a| Ok(Value::Bool(a[0].is_err())));
    r.s("ISNA", 1, 1, "value", "TRUE for #N/A", |_, a| Ok(Value::Bool(a[0] == Value::Err(XlErr::NA))));
    r.s("ISLOGICAL", 1, 1, "value", "TRUE for a logical value", |_, a| Ok(Value::Bool(matches!(a[0], Value::Bool(_)))));
    r.s("ISNUMBER", 1, 1, "value", "TRUE for a number", |_, a| Ok(Value::Bool(matches!(a[0], Value::Num(_)))));
    r.s("ISTEXT", 1, 1, "value", "TRUE for text", |_, a| Ok(Value::Bool(matches!(a[0], Value::Text(_)))));
    r.s("ISNONTEXT", 1, 1, "value", "TRUE for anything except text", |_, a| Ok(Value::Bool(!matches!(a[0], Value::Text(_)))));
    r.s("ISEVEN", 1, 1, "number", "TRUE for an even number", |_, a| Ok(Value::Bool((n(a, 0)?.trunc() as i64) % 2 == 0)));
    r.s("ISODD", 1, 1, "number", "TRUE for an odd number", |_, a| Ok(Value::Bool((n(a, 0)?.trunc() as i64) % 2 != 0)));
    r.a("ISREF", 1, 1, "value", "TRUE for a reference", |_, a| Ok(Value::Bool(matches!(a[0], Value::Ref(..)))));
    r.a("ISFORMULA", 1, 1, "reference", "TRUE if the cell has a formula", |c, a| match &a[0] {
        Value::Ref(si, rc) => Ok(Value::Bool(matches!(c.wb.sheets[*si].get(rc.r1, rc.c1).map(|x| &x.v), Some(Content::Formula(_))))),
        _ => Err(XlErr::Value),
    });
    r.a("FORMULATEXT", 1, 1, "reference", "The formula in a cell, as text", |c, a| match &a[0] {
        Value::Ref(si, rc) => match c.wb.sheets[*si].get(rc.r1, rc.c1).map(|x| &x.v) {
            Some(Content::Formula(e)) => txt(format!("={}", formula::print(e))),
            _ => Err(XlErr::NA),
        },
        _ => Err(XlErr::Value),
    });
    r.s("N", 1, 1, "value", "Value converted to a number", |_, a| match &a[0] {
        Value::Num(x) => Ok(Value::Num(*x)),
        Value::Bool(b) => Ok(Value::Num(*b as i32 as f64)),
        Value::Err(e) => Err(*e),
        _ => Ok(Value::Num(0.0)),
    });
    r.a("NA", 0, 0, "", "The error value #N/A", |_, _| Err(XlErr::NA));
    r.s("TYPE", 1, 1, "value", "Number describing the value's type", |_, a| Ok(Value::Num(match a[0] {
        Value::Num(_) | Value::Empty => 1.0,
        Value::Text(_) => 2.0,
        Value::Bool(_) => 4.0,
        Value::Err(_) => 16.0,
        _ => 64.0,
    })));
    r.s("ERROR.TYPE", 1, 1, "error_val", "Number for an error type", |_, a| match a[0] {
        Value::Err(e) => Ok(Value::Num(match e {
            XlErr::Null => 1.0, XlErr::Div0 => 2.0, XlErr::Value => 3.0, XlErr::Ref => 4.0, XlErr::Name => 5.0,
            XlErr::Num => 6.0, XlErr::NA => 7.0, XlErr::Spill => 9.0, XlErr::Calc => 14.0,
        })),
        _ => Err(XlErr::NA),
    });
    r.a("SHEET", 0, 1, "[value]", "Sheet number", |c, a| match a.first() {
        Some(Value::Ref(si, _)) => Ok(Value::Num(*si as f64 + 1.0)),
        Some(Value::Text(s)) => c.wb.sheet_index(s).map(|i| Value::Num(i as f64 + 1.0)).ok_or(XlErr::NA),
        _ => Ok(Value::Num(c.sheet as f64 + 1.0)),
    });
    r.a("SHEETS", 0, 1, "[reference]", "Number of sheets", |c, _| Ok(Value::Num(c.wb.sheets.len() as f64)));
    r.a("INFO", 1, 1, "type_text", "Information about the working environment", |c, a| match t(a, 0)?.to_ascii_lowercase().as_str() {
        "numfile" => Ok(Value::Num(c.wb.sheets.len() as f64)),
        "recalc" => txt("Automatic"),
        "system" => txt("pcdos"),
        "release" => txt("16.0"),
        _ => Err(XlErr::NA),
    });
    r.a("CELL", 1, 2, "info_type, [reference]", "Information about a cell", |c, a| {
        let (si, p) = match a.get(1) {
            Some(Value::Ref(si, rc)) => (*si, Pos::new(rc.r1, rc.c1)),
            _ => (c.sheet, c.pos),
        };
        match t(a, 0)?.to_ascii_lowercase().as_str() {
            "address" => txt(format!("${}${}", col_name(p.col), p.row + 1)),
            "row" => Ok(Value::Num(p.row as f64 + 1.0)),
            "col" => Ok(Value::Num(p.col as f64 + 1.0)),
            "contents" => Ok(c.cell(si, p.row, p.col)),
            "type" => txt(match c.cell(si, p.row, p.col) { Value::Empty => "b", Value::Text(_) => "l", _ => "v" }),
            "width" => Ok(Value::Num((c.wb.sheets[si].col_px(p.col) as f64 / 7.0).round())),
            "filename" => txt(format!("[{}]", c.wb.sheets[si].name)),
            _ => Err(XlErr::Value),
        }
    });
}
