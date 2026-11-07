//! Conditional formatting: works out which visible cells each rule changes.

use crate::eval::{self, CalcResult, Ctx, Value};
use crate::model::*;
use crate::ops;
use egui::{pos2, vec2, Color32, Painter, Pos2, Stroke};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Hit {
    pub style: Option<CondStyle>,
    pub scale: Option<u32>,
    pub bar: Option<(f32, u32)>,
    pub bar_only: bool,
    pub icon: Option<(IconSet, u8)>,
}

#[derive(Default)]
pub struct CondCache {
    pub sheet: usize,
    pub gen: u64,
    pub map: HashMap<(u32, u32), Hit>,
}

/// Cells of a range that hold something, limited to the used area.
fn cells(sh: &Sheet, r: &Rect) -> Vec<Pos> {
    let Some(u) = sh.used_rect() else { return vec![] };
    let Some(x) = r.intersection(&u) else { return vec![] };
    let mut out = vec![];
    for (&row, m) in sh.rows.range(x.r1..=x.r2) {
        for (&c, _) in m.range(x.c1..=x.c2) {
            out.push(Pos::new(row, c));
        }
    }
    out
}

fn all_cells(r: &Rect, sh: &Sheet) -> Vec<Pos> {
    let u = sh.used_rect().unwrap_or(Rect::new(0, 0, 0, 0));
    let Some(x) = r.intersection(&Rect::new(0, 0, u.r2.max(r.r1), u.c2.max(r.c1))) else { return vec![] };
    if x.rows() as u64 * x.cols() as u64 > 400_000 {
        return cells(sh, r);
    }
    (x.r1..=x.r2).flat_map(|row| (x.c1..=x.c2).map(move |c| Pos::new(row, c))).collect()
}

fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| -> u32 {
        let x = (a >> s & 255) as f32;
        let y = (b >> s & 255) as f32;
        (x + (y - x) * t).round().clamp(0.0, 255.0) as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

pub fn compute(wb: &Workbook, calc: &CalcResult, si: usize, gen: u64) -> CondCache {
    let mut map: HashMap<(u32, u32), Hit> = HashMap::new();
    let sh = &wb.sheets[si];
    if sh.cond.is_empty() {
        return CondCache { sheet: si, gen, map };
    }
    let ctx = Ctx::reader(wb, calc);
    let mut stopped: std::collections::HashSet<(u32, u32)> = Default::default();
    for cf in &sh.cond {
        let mut pts: Vec<Pos> = vec![];
        for r in &cf.ranges {
            pts.extend(if matches!(cf.rule, CondRule::Blanks { blank: true } | CondRule::Formula { .. }) { all_cells(r, sh) } else { cells(sh, r) });
        }
        let num = |p: &Pos| match calc.value(wb, si, p.row, p.col) {
            Value::Num(n) => Some(n),
            _ => None,
        };
        let nums: Vec<f64> = pts.iter().filter_map(num).collect();
        let first = cf.ranges.first().map(|r| Pos::new(r.r1, r.c1)).unwrap_or(Pos::new(0, 0));
        let eval_at = |src: &str, p: Pos| -> Value {
            if src.is_empty() {
                return Value::Empty;
            }
            let s = if src.starts_with('=') { src.to_string() } else { format!("={src}") };
            // Relative references are written for the first cell and move with each cell.
            match crate::formula::parse(&s) {
                Ok(e) => {
                    let e2 = crate::formula::shifted(&e, p.row as i64 - first.row as i64, p.col as i64 - first.col as i64);
                    ctx.eval_text(&format!("={}", crate::formula::print(&e2)), si, p)
                }
                Err(_) => Value::Err(XlErr::Name),
            }
        };
        let mut counts: HashMap<String, usize> = HashMap::new();
        if let CondRule::Duplicate { .. } = cf.rule {
            for p in &pts {
                let t = ops::text_of(wb, calc, si, p.row, p.col).to_lowercase();
                if !t.is_empty() {
                    *counts.entry(t).or_default() += 1;
                }
            }
        }
        let (lo, hi) = nums.iter().fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        let avg = if nums.is_empty() { 0.0 } else { nums.iter().sum::<f64>() / nums.len() as f64 };
        let mut sorted = nums.clone();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        for p in pts {
            let key = (p.row, p.col);
            if stopped.contains(&key) {
                continue;
            }
            let v = calc.value(wb, si, p.row, p.col);
            let n = num(&p);
            let hit_style = match &cf.rule {
                CondRule::CellIs { op, a, b } => {
                    let x = &v;
                    let av = eval_at(a, p);
                    let bv = eval_at(b, p);
                    let c1 = eval::compare(x, &av);
                    let c2 = eval::compare(x, &bv);
                    use std::cmp::Ordering::*;
                    if matches!(x, Value::Empty) {
                        false
                    } else {
                        match op.as_str() {
                            ">" | "greaterThan" => c1 == Some(Greater),
                            "<" | "lessThan" => c1 == Some(Less),
                            ">=" | "greaterThanOrEqual" => matches!(c1, Some(Greater | Equal)),
                            "<=" | "lessThanOrEqual" => matches!(c1, Some(Less | Equal)),
                            "=" | "equal" => c1 == Some(Equal),
                            "<>" | "notEqual" => c1 != Some(Equal),
                            "between" => matches!(c1, Some(Greater | Equal)) && matches!(c2, Some(Less | Equal)),
                            "notbetween" | "notBetween" => !(matches!(c1, Some(Greater | Equal)) && matches!(c2, Some(Less | Equal))),
                            _ => false,
                        }
                    }
                }
                CondRule::TextContains { text, mode } => {
                    let t = ops::text_of(wb, calc, si, p.row, p.col).to_lowercase();
                    let q = text.to_lowercase();
                    match mode {
                        TextMode::Contains => t.contains(&q),
                        TextMode::NotContains => !t.contains(&q),
                        TextMode::Begins => t.starts_with(&q),
                        TextMode::Ends => t.ends_with(&q),
                    }
                }
                CondRule::Duplicate { unique } => {
                    let t = ops::text_of(wb, calc, si, p.row, p.col).to_lowercase();
                    let c = counts.get(&t).copied().unwrap_or(0);
                    !t.is_empty() && if *unique { c == 1 } else { c > 1 }
                }
                CondRule::TopBottom { top, rank, percent } => match n {
                    None => false,
                    Some(x) => {
                        let k = if *percent { ((sorted.len() as f64 * *rank as f64 / 100.0).floor() as usize).max(1) } else { *rank as usize };
                        let k = k.min(sorted.len()).max(1);
                        if *top { x >= sorted[k - 1] } else { x <= sorted[sorted.len() - k] }
                    }
                },
                CondRule::AboveAverage { above } => n.is_some_and(|x| if *above { x > avg } else { x < avg }),
                CondRule::Blanks { blank } => {
                    let empty = matches!(v, Value::Empty) || matches!(&v, Value::Text(t) if t.trim().is_empty());
                    empty == *blank
                }
                CondRule::Formula { formula } => eval::to_bool(&eval_at(formula, p)).unwrap_or(false),
                CondRule::ColorScale { colors } => {
                    if let Some(x) = n {
                        let t = if hi > lo { ((x - lo) / (hi - lo)) as f32 } else { 0.5 };
                        let c = if colors.len() >= 3 {
                            if t < 0.5 { lerp(colors[0], colors[1], t * 2.0) } else { lerp(colors[1], colors[2], (t - 0.5) * 2.0) }
                        } else if colors.len() == 2 {
                            lerp(colors[0], colors[1], t)
                        } else {
                            0xFFFFFF
                        };
                        map.entry(key).or_default().scale.get_or_insert(c);
                    }
                    false
                }
                CondRule::DataBar { color } => {
                    if let Some(x) = n {
                        let base = lo.min(0.0);
                        let t = if hi > base { ((x - base) / (hi - base)) as f32 } else { 1.0 };
                        map.entry(key).or_default().bar.get_or_insert((t.clamp(0.05, 1.0), *color));
                    }
                    false
                }
                CondRule::IconSet { set } => {
                    if let Some(x) = n {
                        let t = if hi > lo { (x - lo) / (hi - lo) } else { 1.0 };
                        let k = if t >= 0.67 { 2 } else if t >= 0.33 { 1 } else { 0 };
                        map.entry(key).or_default().icon.get_or_insert((*set, k));
                    }
                    false
                }
            };
            if hit_style {
                let h = map.entry(key).or_default();
                if h.style.is_none() {
                    h.style = Some(cf.style.clone());
                }
                if cf.stop_if_true {
                    stopped.insert(key);
                }
            }
        }
    }
    CondCache { sheet: si, gen, map }
}

/// Draws one icon of an icon set at a point.
pub fn paint_icon(p: &Painter, at: Pos2, set: IconSet, k: u8, z: f32) {
    let s = 6.0 * z.max(0.7);
    let colors = [Color32::from_rgb(0xF8, 0x69, 0x6B), Color32::from_rgb(0xFF, 0xC0, 0x00), Color32::from_rgb(0x63, 0xBE, 0x7B)];
    let c = colors[k as usize % 3];
    match set {
        IconSet::Arrows3 => {
            let pts = match k {
                2 => vec![pos2(at.x, at.y - s), pos2(at.x + s, at.y), pos2(at.x - s, at.y)],
                0 => vec![pos2(at.x - s, at.y), pos2(at.x + s, at.y), pos2(at.x, at.y + s)],
                _ => vec![pos2(at.x, at.y - s), pos2(at.x + s, at.y), pos2(at.x, at.y + s)],
            };
            p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
            let tail = match k {
                2 => [pos2(at.x, at.y), pos2(at.x, at.y + s)],
                0 => [pos2(at.x, at.y - s), pos2(at.x, at.y)],
                _ => [pos2(at.x - s, at.y), pos2(at.x, at.y)],
            };
            p.line_segment(tail, Stroke::new(s * 0.6, c));
        }
        IconSet::Lights3 => {
            p.circle_filled(at, s, c);
            p.circle_stroke(at, s, Stroke::new(1.0, Color32::from_rgb(0x40, 0x40, 0x40)));
        }
        IconSet::Flags3 => {
            p.line_segment([at + vec2(-s * 0.6, -s), at + vec2(-s * 0.6, s)], Stroke::new(1.2, Color32::from_rgb(0x40, 0x40, 0x40)));
            p.add(egui::Shape::convex_polygon(vec![at + vec2(-s * 0.6, -s), at + vec2(s, -s * 0.5), at + vec2(-s * 0.6, 0.0)], c, Stroke::NONE));
        }
        IconSet::Stars3 => {
            let mut pts = vec![];
            for i in 0..10 {
                let ang = std::f32::consts::PI * (i as f32 / 5.0) - std::f32::consts::FRAC_PI_2;
                let r = if i % 2 == 0 { s } else { s * 0.45 };
                pts.push(at + vec2(ang.cos() * r, ang.sin() * r));
            }
            let fill = if k == 2 { Color32::from_rgb(0xFF, 0xC0, 0x00) } else { Color32::from_rgb(0xD9, 0xD9, 0xD9) };
            // A star isn't convex, so draw it as triangles from the centre.
            for i in 0..10 {
                p.add(egui::Shape::convex_polygon(vec![at, pts[i], pts[(i + 1) % 10]], fill, Stroke::NONE));
            }
            if k == 1 {
                for i in 0..5 {
                    p.add(egui::Shape::convex_polygon(vec![at, pts[i], pts[(i + 1) % 10]], Color32::from_rgb(0xFF, 0xC0, 0x00), Stroke::NONE));
                }
            }
        }
    }
}
