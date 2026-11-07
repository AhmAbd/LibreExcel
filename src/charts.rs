//! Drawing and creating charts and sparklines.

use crate::app::App;
use crate::eval::{self, CalcResult, Ctx, Value};
use crate::model::*;
use crate::ops;
use crate::theme;
use egui::{pos2, vec2, Align2, Color32, Painter, Rect as R, Stroke, StrokeKind};

struct Data {
    cats: Vec<String>,
    series: Vec<(String, Vec<Option<f64>>, Color32)>,
}

fn eval_ref(ctx: &Ctx, src: &str, si: usize) -> Vec<Value> {
    if src.trim().is_empty() {
        return vec![];
    }
    let f = if src.starts_with('=') { src.to_string() } else { format!("={src}") };
    match ctx.eval_range_text(&f, si) {
        Value::Array(a) => a.data.clone(),
        x => vec![x],
    }
}

fn read(ch: &Chart, wb: &Workbook, calc: &CalcResult, si: usize) -> Data {
    let ctx = Ctx::reader(wb, calc);
    let mut series = vec![];
    let mut cats: Vec<String> = vec![];
    for (i, s) in ch.series.iter().enumerate() {
        let vals: Vec<Option<f64>> = eval_ref(&ctx, &s.vals, si).iter().map(|v| if let Value::Num(n) = v { Some(*n) } else { None }).collect();
        let name = match &s.name_ref {
            Some(r) => eval_ref(&ctx, r, si).first().and_then(|v| eval::to_text(v).ok()).unwrap_or_default(),
            None => s.name.clone(),
        };
        let name = if name.is_empty() { format!("Series{}", i + 1) } else { name };
        if cats.is_empty() {
            if let Some(c) = &s.cats {
                cats = eval_ref(&ctx, c, si)
                    .iter()
                    .map(|v| match v {
                        Value::Num(n) => {
                            let r = crate::numfmt::format_num("General", *n, 11).text;
                            r
                        }
                        x => eval::to_text(x).unwrap_or_default(),
                    })
                    .collect();
            }
        }
        let color = theme::rgb(s.color.unwrap_or(theme::SERIES[i % theme::SERIES.len()]));
        series.push((name, vals, color));
    }
    let n = series.iter().map(|s| s.1.len()).max().unwrap_or(0);
    if cats.len() < n {
        for k in cats.len()..n {
            cats.push((k + 1).to_string());
        }
    }
    Data { cats, series }
}

/// Round numbers for an axis from lo to hi.
fn nice_axis(lo: f64, hi: f64) -> (f64, f64, f64) {
    let (mut lo, mut hi) = (lo.min(0.0), hi.max(0.0));
    if hi - lo < 1e-12 {
        hi = lo + 1.0;
    }
    let raw = (hi - lo) / 5.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(mag * 10.0);
    lo = (lo / step).floor() * step;
    hi = (hi / step).ceil() * step;
    (lo, hi, step)
}

fn short_num(x: f64) -> String {
    let a = x.abs();
    if a >= 1e9 {
        format!("{}B", eval::num_text(x / 1e9))
    } else if a >= 1e6 {
        format!("{}M", eval::num_text(x / 1e6))
    } else if x.fract() == 0.0 {
        crate::numfmt::format_num("#,##0", x, 12).text
    } else {
        let s = format!("{:.2}", x);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn draw(p: &Painter, r: R, ch: &Chart, wb: &Workbook, calc: &CalcResult, si: usize, z: f32) {
    p.rect(r, 0.0, Color32::WHITE, Stroke::new(1.0, Color32::from_rgb(0xD9, 0xD9, 0xD9)), StrokeKind::Inside);
    let cp = p.with_clip_rect(r.intersect(p.clip_rect()));
    let d = read(ch, wb, calc, si);
    let fs = |x: f32| (x * z).max(6.0);
    let text = Color32::from_rgb(0x59, 0x59, 0x59);
    let mut inner = r.shrink(10.0 * z);
    if !ch.title.is_empty() {
        cp.text(pos2(r.center().x, inner.top() + 8.0 * z), Align2::CENTER_CENTER, &ch.title, theme::ui_font(fs(14.0)), text);
        inner.min.y += 24.0 * z;
    }
    let pie = matches!(ch.kind, ChartKind::Pie | ChartKind::Doughnut);
    if ch.legend {
        let items: Vec<(String, Color32)> = if pie {
            d.cats.iter().enumerate().map(|(i, c)| (c.clone(), theme::rgb(theme::SERIES[i % 10]))).collect()
        } else {
            d.series.iter().map(|s| (s.0.clone(), s.2)).collect()
        };
        let font = theme::ui_font(fs(10.5));
        let widths: Vec<f32> = items.iter().map(|(n, _)| p.layout_no_wrap(n.clone(), font.clone(), text).size().x + 22.0 * z).collect();
        let total: f32 = widths.iter().sum();
        let mut x = r.center().x - total.min(inner.width()) / 2.0;
        let y = inner.bottom() - 7.0 * z;
        for ((n, c), w) in items.iter().zip(widths) {
            if x + w > r.right() {
                break;
            }
            cp.rect_filled(R::from_center_size(pos2(x + 5.0 * z, y), vec2(8.0 * z, 8.0 * z)), 0.0, *c);
            cp.text(pos2(x + 13.0 * z, y), Align2::LEFT_CENTER, n, font.clone(), text);
            x += w;
        }
        inner.max.y -= 20.0 * z;
    }
    if d.series.is_empty() {
        return;
    }
    if pie {
        let vals: Vec<f64> = d.series[0].1.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
        let total: f64 = vals.iter().sum();
        if total <= 0.0 {
            return;
        }
        let rad = inner.width().min(inner.height()) / 2.0 - 4.0;
        let c = inner.center();
        let mut a0 = -std::f32::consts::FRAC_PI_2;
        for (i, v) in vals.iter().enumerate() {
            let sweep = (*v / total) as f32 * std::f32::consts::TAU;
            let col = ch.series[0].color.filter(|_| vals.len() == 1).map(theme::rgb).unwrap_or(theme::rgb(theme::SERIES[i % 10]));
            let steps = ((sweep / 0.05).ceil() as usize).max(1);
            for k in 0..steps {
                let t0 = a0 + sweep * k as f32 / steps as f32;
                let t1 = a0 + sweep * (k + 1) as f32 / steps as f32;
                cp.add(egui::Shape::convex_polygon(vec![c, c + vec2(t0.cos(), t0.sin()) * rad, c + vec2(t1.cos(), t1.sin()) * rad], col, Stroke::NONE));
            }
            cp.line_segment([c, c + vec2(a0.cos(), a0.sin()) * rad], Stroke::new(1.5, Color32::WHITE));
            if ch.data_labels && sweep > 0.2 {
                let mid = a0 + sweep / 2.0;
                let at = c + vec2(mid.cos(), mid.sin()) * rad * 0.7;
                cp.text(at, Align2::CENTER_CENTER, format!("{:.0}%", v / total * 100.0), theme::ui_font(fs(10.0)), Color32::WHITE);
            }
            a0 += sweep;
        }
        if ch.kind == ChartKind::Doughnut {
            cp.circle_filled(c, rad * 0.5, Color32::WHITE);
        }
        return;
    }
    let scatter = ch.kind == ChartKind::Scatter;
    let horiz = matches!(ch.kind, ChartKind::Bar | ChartKind::BarStacked);
    let stacked = matches!(ch.kind, ChartKind::ColumnStacked | ChartKind::BarStacked);
    let n = d.cats.len().max(1);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    if stacked {
        for k in 0..n {
            let (mut pos, mut neg) = (0.0, 0.0);
            for s in &d.series {
                let v = s.1.get(k).copied().flatten().unwrap_or(0.0);
                if v >= 0.0 { pos += v } else { neg += v }
            }
            lo = lo.min(neg);
            hi = hi.max(pos);
        }
    } else {
        for s in &d.series {
            for v in s.1.iter().flatten() {
                lo = lo.min(*v);
                hi = hi.max(*v);
            }
        }
    }
    if lo > hi {
        lo = 0.0;
        hi = 1.0;
    }
    let (lo, hi, step) = nice_axis(lo, hi);
    let font = theme::ui_font(fs(10.0));
    let label_w = (lo..=hi).len_hint(step).iter().map(|v| p.layout_no_wrap(short_num(*v), font.clone(), text).size().x).fold(0.0, f32::max) + 6.0 * z;
    if !ch.y_title.is_empty() {
        cp.add(egui::epaint::TextShape::new(pos2(inner.left(), inner.center().y + 30.0 * z), p.layout_no_wrap(ch.y_title.clone(), theme::ui_font(fs(10.5)), text), text).with_angle(-std::f32::consts::FRAC_PI_2));
        inner.min.x += 16.0 * z;
    }
    if !ch.x_title.is_empty() {
        cp.text(pos2(inner.center().x, inner.bottom() - 6.0 * z), Align2::CENTER_CENTER, &ch.x_title, theme::ui_font(fs(10.5)), text);
        inner.max.y -= 16.0 * z;
    }
    let plot = if horiz {
        let cat_w = d.cats.iter().map(|c| p.layout_no_wrap(c.clone(), font.clone(), text).size().x).fold(0.0, f32::max).min(inner.width() * 0.3) + 6.0 * z;
        R::from_min_max(pos2(inner.left() + cat_w, inner.top() + 4.0), pos2(inner.right() - 6.0, inner.bottom() - 16.0 * z))
    } else {
        R::from_min_max(pos2(inner.left() + label_w, inner.top() + 4.0), pos2(inner.right() - 6.0, inner.bottom() - 16.0 * z))
    };
    let grid = Color32::from_rgb(0xD9, 0xD9, 0xD9);
    let to_v = |v: f64| -> f32 { ((v - lo) / (hi - lo)) as f32 };
    // Value axis and gridlines.
    let mut v = lo;
    while v <= hi + step * 0.001 {
        let t = to_v(v);
        if horiz {
            let x = plot.left() + t * plot.width();
            if ch.gridlines {
                cp.vline(x, plot.top()..=plot.bottom(), Stroke::new(1.0, grid));
            }
            cp.text(pos2(x, plot.bottom() + 3.0), Align2::CENTER_TOP, short_num(v), font.clone(), text);
        } else {
            let y = plot.bottom() - t * plot.height();
            if ch.gridlines {
                cp.hline(plot.left()..=plot.right(), y, Stroke::new(1.0, grid));
            }
            cp.text(pos2(plot.left() - 4.0, y), Align2::RIGHT_CENTER, short_num(v), font.clone(), text);
        }
        v += step;
    }
    let zero = to_v(0.0);
    if horiz {
        cp.vline(plot.left() + zero * plot.width(), plot.top()..=plot.bottom(), Stroke::new(1.0, Color32::from_rgb(0xBF, 0xBF, 0xBF)));
    } else {
        cp.hline(plot.left()..=plot.right(), plot.bottom() - zero * plot.height(), Stroke::new(1.0, Color32::from_rgb(0xBF, 0xBF, 0xBF)));
    }
    if scatter {
        // X values are the categories when numeric.
        let xs: Vec<f64> = d.cats.iter().enumerate().map(|(i, c)| c.parse::<f64>().unwrap_or(i as f64 + 1.0)).collect();
        let (xl, xh) = xs.iter().fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
        let (xl, xh, xs_step) = nice_axis(xl, xh);
        let mut xv = xl;
        while xv <= xh + xs_step * 0.001 {
            let x = plot.left() + ((xv - xl) / (xh - xl)) as f32 * plot.width();
            cp.text(pos2(x, plot.bottom() + 3.0), Align2::CENTER_TOP, short_num(xv), font.clone(), text);
            xv += xs_step;
        }
        for s in &d.series {
            for (k, v) in s.1.iter().enumerate() {
                if let (Some(v), Some(x)) = (v, xs.get(k)) {
                    let px = plot.left() + ((x - xl) / (xh - xl)) as f32 * plot.width();
                    let py = plot.bottom() - to_v(*v) * plot.height();
                    cp.circle_filled(pos2(px, py), 3.5 * z, s.2);
                }
            }
        }
        return;
    }
    let band = if horiz { plot.height() } else { plot.width() } / n as f32;
    // Category labels.
    let every = if horiz { 1 } else { ((n as f32 * 40.0 * z / plot.width()).ceil() as usize).max(1) };
    for (k, c) in d.cats.iter().enumerate() {
        if k % every != 0 {
            continue;
        }
        if horiz {
            let y = plot.top() + band * (k as f32 + 0.5);
            cp.text(pos2(plot.left() - 4.0, y), Align2::RIGHT_CENTER, c, font.clone(), text);
        } else {
            let x = plot.left() + band * (k as f32 + 0.5);
            cp.text(pos2(x, plot.bottom() + 3.0), Align2::CENTER_TOP, c, font.clone(), text);
        }
    }
    match ch.kind {
        ChartKind::Line | ChartKind::LineMarkers | ChartKind::Area => {
            for s in d.series.iter().rev() {
                let pts: Vec<Option<egui::Pos2>> = s.1.iter().enumerate().map(|(k, v)| v.map(|v| pos2(plot.left() + band * (k as f32 + 0.5), plot.bottom() - to_v(v) * plot.height()))).collect();
                if ch.kind == ChartKind::Area {
                    let base = plot.bottom() - zero * plot.height();
                    for w in pts.windows(2) {
                        if let (Some(a), Some(b)) = (w[0], w[1]) {
                            let col = s.2.gamma_multiply(0.85);
                            // Split into two triangles to stay convex either side of the base.
                            cp.add(egui::Shape::convex_polygon(vec![pos2(a.x, base), a, b, pos2(b.x, base)], col, Stroke::NONE));
                        }
                    }
                } else {
                    let line: Vec<egui::Pos2> = pts.iter().flatten().copied().collect();
                    cp.add(egui::Shape::line(line, Stroke::new(2.25 * z, s.2)));
                    if ch.kind == ChartKind::LineMarkers {
                        for q in pts.iter().flatten() {
                            cp.circle(*q, 3.5 * z, s.2, Stroke::new(1.0, Color32::WHITE));
                        }
                    }
                }
                if ch.data_labels {
                    for (k, q) in pts.iter().enumerate() {
                        if let (Some(q), Some(Some(v))) = (q, s.1.get(k)) {
                            cp.text(*q - vec2(0.0, 6.0 * z), Align2::CENTER_BOTTOM, short_num(*v), font.clone(), text);
                        }
                    }
                }
            }
        }
        _ => {
            let ns = d.series.len();
            let gap = band * 0.18;
            let bw = if stacked { band - 2.0 * gap } else { (band - 2.0 * gap) / ns as f32 };
            let mut pos_acc = vec![0.0f64; n];
            let mut neg_acc = vec![0.0f64; n];
            for (si2, s) in d.series.iter().enumerate() {
                for (k, v) in s.1.iter().enumerate() {
                    let Some(v) = v else { continue };
                    let (a0, a1) = if stacked {
                        let acc = if *v >= 0.0 { &mut pos_acc[k] } else { &mut neg_acc[k] };
                        let s0 = *acc;
                        *acc += v;
                        (s0, *acc)
                    } else {
                        (0.0, *v)
                    };
                    let (t0, t1) = (to_v(a0), to_v(a1));
                    let off = gap + if stacked { 0.0 } else { si2 as f32 * bw };
                    let rr = if horiz {
                        let y = plot.top() + band * k as f32 + off;
                        R::from_min_max(pos2(plot.left() + t0.min(t1) * plot.width(), y), pos2(plot.left() + t0.max(t1) * plot.width(), y + bw))
                    } else {
                        let x = plot.left() + band * k as f32 + off;
                        R::from_min_max(pos2(x, plot.bottom() - t0.max(t1) * plot.height()), pos2(x + bw, plot.bottom() - t0.min(t1) * plot.height()))
                    };
                    cp.rect_filled(rr, 0.0, s.2);
                    if ch.data_labels {
                        if horiz {
                            cp.text(rr.right_center() + vec2(3.0, 0.0), Align2::LEFT_CENTER, short_num(*v), font.clone(), text);
                        } else {
                            cp.text(rr.center_top() - vec2(0.0, 2.0), Align2::CENTER_BOTTOM, short_num(*v), font.clone(), text);
                        }
                    }
                }
            }
        }
    }
}

trait Ticks {
    fn len_hint(&self, step: f64) -> Vec<f64>;
}
impl Ticks for std::ops::RangeInclusive<f64> {
    fn len_hint(&self, step: f64) -> Vec<f64> {
        let mut v = vec![];
        let mut x = *self.start();
        while x <= *self.end() + step * 0.001 && v.len() < 50 {
            v.push(x);
            x += step;
        }
        v
    }
}

pub fn sparkline(p: &Painter, r: R, sp: &Sparkline, wb: &Workbook, calc: &CalcResult, si: usize) {
    let ctx = Ctx::reader(wb, calc);
    let vals: Vec<f64> = eval_ref(&ctx, &sp.source, si).iter().filter_map(|v| if let Value::Num(n) = v { Some(*n) } else { None }).collect();
    if vals.is_empty() {
        return;
    }
    let (lo, hi) = vals.iter().fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(*x), h.max(*x)));
    let span = if hi > lo { hi - lo } else { 1.0 };
    let col = theme::rgb(sp.color);
    let n = vals.len();
    if sp.kind == 0 {
        let pts: Vec<egui::Pos2> = vals.iter().enumerate().map(|(i, v)| pos2(r.left() + r.width() * i as f32 / (n - 1).max(1) as f32, r.bottom() - ((v - lo) / span) as f32 * r.height())).collect();
        p.add(egui::Shape::line(pts, Stroke::new(1.5, col)));
    } else {
        let bw = r.width() / n as f32;
        let base = if lo < 0.0 { lo.min(0.0) } else { 0.0 };
        let span = (hi.max(0.0) - base).max(1e-9);
        for (i, v) in vals.iter().enumerate() {
            let h = ((v - base) / span) as f32 * r.height();
            let rr = R::from_min_max(pos2(r.left() + i as f32 * bw + bw * 0.1, r.bottom() - h.max(1.0)), pos2(r.left() + (i + 1) as f32 * bw - bw * 0.1, r.bottom()));
            p.rect_filled(rr, 0.0, if *v < 0.0 { Color32::from_rgb(0xD0, 0x00, 0x00) } else { col });
        }
    }
}

/// Makes series from a block of data: the first column (or row) gives categories, the first row (or column) names.
pub fn series_from(a: &App, si: usize, r: Rect, by_cols: Option<bool>) -> Vec<Series> {
    let sname = crate::formula::quote_sheet(&a.wb.sheets[si].name);
    let is_text = |row: u32, c: u32| matches!(a.calc.value(&a.wb, si, row, c), Value::Text(_) | Value::Empty);
    let header = r.rows() > 1 && (r.c1..=r.c2).any(|c| matches!(a.calc.value(&a.wb, si, r.r1, c), Value::Text(_))) && (r.c1..=r.c2).skip(1).all(|c| is_text(r.r1, c));
    let label_col = r.cols() > 1 && (r.r1 + header as u32..=r.r2).all(|row| is_text(row, r.c1));
    let d0r = r.r1 + header as u32;
    let d0c = r.c1 + label_col as u32;
    let by_cols = by_cols.unwrap_or((r.r2 - d0r + 1) >= (r.c2 - d0c + 1));
    let abs = |x: Rect| format!("{sname}!{}", x.a1_abs());
    let mut out = vec![];
    if by_cols {
        let cats = if label_col { Some(abs(Rect::new(d0r, r.c1, r.r2, r.c1))) } else { None };
        for c in d0c..=r.c2 {
            out.push(Series { name: String::new(), name_ref: if header { Some(abs(Rect::cell(Pos::new(r.r1, c)))) } else { None }, cats: cats.clone(), vals: abs(Rect::new(d0r, c, r.r2, c)), color: None });
        }
    } else {
        let cats = if header { Some(abs(Rect::new(r.r1, d0c, r.r1, r.c2))) } else { None };
        for row in d0r..=r.r2 {
            out.push(Series { name: String::new(), name_ref: if label_col { Some(abs(Rect::cell(Pos::new(row, r.c1)))) } else { None }, cats: cats.clone(), vals: abs(Rect::new(row, d0c, row, r.c2)), color: None });
        }
    }
    out
}

pub fn insert_chart(a: &mut App, kind: ChartKind) {
    let p = a.sel().active;
    let r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
    if a.sheet().is_empty_at(r.r1, r.c1) && r.is_single() {
        a.alert = Some(("Insert Chart".into(), "Select the data you want to chart first: a block of numbers with labels in the first row and column.".into()));
        return;
    }
    let series = series_from(a, a.cur, r, None);
    let sh = a.sheet();
    let x: f32 = (0..=r.c2).map(|c| sh.col_px(c)).sum::<f32>() + 24.0;
    let y: f32 = (0..r.r1).map(|k| sh.row_px(k)).sum::<f32>();
    let title = if series.len() == 1 {
        series[0].name_ref.as_ref().map(|n| {
            let ctx = Ctx::reader(&a.wb, &a.calc);
            eval_ref(&ctx, n, a.cur).first().and_then(|v| eval::to_text(v).ok()).unwrap_or_default()
        }).unwrap_or_else(|| "Chart Title".into())
    } else {
        "Chart Title".into()
    };
    let chart = Chart { kind, title, series, x, y, w: 480.0, h: 288.0, legend: true, x_title: String::new(), y_title: String::new(), data_labels: false, gridlines: !matches!(kind, ChartKind::Pie | ChartKind::Doughnut) };
    a.act(|a| a.wb.sheet_mut(a.cur).charts.push(chart));
    a.chart_sel = Some(a.sheet().charts.len() - 1);
    a.tab = crate::app::RTab::ChartDesign;
}

/// The sheet and block a chart's series cover.
pub fn data_block(ch: &Chart) -> Option<(String, Rect)> {
    let mut out: Option<(String, Rect)> = None;
    for s in &ch.series {
        for t in [Some(&s.vals), s.cats.as_ref(), s.name_ref.as_ref()].into_iter().flatten() {
            let t = t.trim_start_matches('=');
            let (sh, body) = match t.rsplit_once('!') {
                Some((a, b)) => (a.trim_matches('\'').to_string(), b),
                None => (String::new(), t),
            };
            if let Some(r) = parse_rect(body) {
                out = Some(match out {
                    Some((n, x)) => (n, x.union(&r)),
                    None => (sh, r),
                });
            }
        }
    }
    out
}

pub fn data_range_text(ch: &Chart) -> String {
    match data_block(ch) {
        Some((s, r)) if !s.is_empty() => format!("={}!{}", crate::formula::quote_sheet(&s), r.a1_abs()),
        Some((_, r)) => format!("={}", r.a1_abs()),
        None => String::new(),
    }
}

/// Rebuilds a chart's series from a range text such as =Sheet1!$A$1:$C$5.
pub fn set_range(a: &mut App, i: usize, text: &str, by_cols: Option<bool>) -> bool {
    let t = text.trim().trim_start_matches('=');
    let (si, body) = match t.rsplit_once('!') {
        Some((s, b)) => (a.wb.sheet_index(s.trim_matches('\'')), b),
        None => (Some(a.cur), t),
    };
    let (Some(si), Some(r)) = (si, parse_rect(body)) else { return false };
    let series = series_from(a, si, r, by_cols);
    a.act(|a| a.wb.sheet_mut(a.cur).charts[i].series = series);
    true
}

pub fn switch_rows_cols(a: &mut App, i: usize) {
    let ch = a.sheet().charts[i].clone();
    let by_cols_now = ch.series.first().and_then(|s| parse_rect(s.vals.rsplit('!').next().unwrap_or(""))).is_none_or(|r| r.cols() == 1 && r.rows() > 1);
    let text = data_range_text(&ch);
    set_range(a, i, &text, Some(!by_cols_now));
}
