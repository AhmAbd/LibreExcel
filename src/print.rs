//! Page layout for printing, the print preview and PDF export.
//!
//! A page is described as a list of simple drawing operations in points from the
//! page's top-left corner. The preview paints them on screen and the PDF writer
//! turns them into a content stream, so both always agree.

use crate::eval::{CalcResult, Value};
use crate::model::*;
use crate::ops;

#[derive(Clone, Debug)]
pub enum Op {
    Fill([f32; 4], u32),
    Line([f32; 4], f32, u32),
    Text { x: f32, y: f32, size: f32, face: u8, bold: bool, italic: bool, color: u32, text: String, clip: Option<[f32; 4]> },
    /// A chart drawn into this box (points); the preview draws it live.
    Chart { index: usize, rect: [f32; 4], scale: f32 },
}

pub struct Layout {
    pub page_w: f32,
    pub page_h: f32,
    /// Points per sheet pixel.
    pub scale: f32,
    pub area: Rect,
    /// Each page's block of cells.
    pub pages: Vec<Rect>,
    pub row_starts: Vec<u32>,
    pub col_starts: Vec<u32>,
}

pub fn paper_size(p: &PageSetup) -> (f32, f32) {
    let (w, h) = match p.paper {
        1 => (595.0, 842.0),
        2 => (612.0, 1008.0),
        3 => (842.0, 1191.0),
        _ => (612.0, 792.0),
    };
    if p.landscape { (h, w) } else { (w, h) }
}

pub const PAPER_NAMES: [&str; 4] = ["Letter (8.5\" x 11\")", "A4 (8.27\" x 11.69\")", "Legal (8.5\" x 14\")", "A3 (11.69\" x 16.54\")"];

const HEAD_W: f32 = 30.0;
const HEAD_H: f32 = 20.0;

/// The block of cells that prints: the print area, or everything used including charts.
pub fn print_area(sh: &Sheet) -> Option<Rect> {
    if let Some(a) = sh.page.print_area {
        return Some(a);
    }
    let mut r = sh.used_rect();
    for m in &sh.merges {
        r = Some(r.map_or(*m, |x| x.union(m)));
    }
    for ch in &sh.charts {
        // The cell under the chart's bottom-right corner.
        let (mut x, mut c) = (0.0, 0u32);
        while x < ch.x + ch.w && c < MAX_COLS - 1 {
            x += sh.col_px(c);
            c += 1;
        }
        let (mut y, mut row) = (0.0, 0u32);
        while y < ch.y + ch.h && row < MAX_ROWS - 1 {
            y += sh.row_px(row);
            row += 1;
        }
        let b = Rect::new(0, 0, row.saturating_sub(1), c.saturating_sub(1));
        r = Some(r.map_or(b, |x| x.union(&b)));
    }
    r
}

pub fn layout(wb: &Workbook, si: usize) -> Option<Layout> {
    let sh = &wb.sheets[si];
    let p = &sh.page;
    let area = print_area(sh)?;
    let (pw, ph) = paper_size(p);
    let avail_w = pw - (p.margins[0] + p.margins[1]) * 72.0;
    let avail_h = ph - (p.margins[2] + p.margins[3]) * 72.0;
    let head_w = if p.headings { HEAD_W } else { 0.0 };
    let head_h = if p.headings { HEAD_H } else { 0.0 };
    let mut scale = 0.75 * p.scale.clamp(10, 400) as f32 / 100.0;
    if p.fit_width {
        let w: f32 = (area.c1..=area.c2).map(|c| sh.col_px(c)).sum::<f32>() + head_w;
        if w > 0.0 {
            scale = (avail_w / w).min(0.75);
        }
    }
    let split = |lo: u32, hi: u32, size: &dyn Fn(u32) -> f32, room: f32| -> Vec<u32> {
        let mut starts = vec![lo];
        let mut used = 0.0;
        for k in lo..=hi {
            let s = size(k) * scale;
            if used + s > room && used > 0.0 {
                starts.push(k);
                used = 0.0;
            }
            used += s;
        }
        starts
    };
    let row_starts = split(area.r1, area.r2, &|r| sh.row_px(r), avail_h - head_h * scale);
    let col_starts = split(area.c1, area.c2, &|c| sh.col_px(c), avail_w - head_w * scale);
    let mut pages = vec![];
    // Down, then over, as Excel does by default.
    for (ci, &c0) in col_starts.iter().enumerate() {
        let c1 = col_starts.get(ci + 1).map(|x| x - 1).unwrap_or(area.c2);
        for (ri, &r0) in row_starts.iter().enumerate() {
            let r1 = row_starts.get(ri + 1).map(|x| x - 1).unwrap_or(area.r2);
            pages.push(Rect::new(r0, c0, r1, c1));
        }
    }
    Some(Layout { page_w: pw, page_h: ph, scale, area, pages, row_starts, col_starts })
}

/// Rows and columns where a new printed page starts (not counting the first).
pub fn breaks(wb: &Workbook, _calc: &CalcResult, si: usize) -> (Vec<u32>, Vec<u32>) {
    match layout(wb, si) {
        Some(l) => (l.row_starts[1..].to_vec(), l.col_starts[1..].to_vec()),
        None => (vec![], vec![]),
    }
}

fn face_of(font: &str) -> u8 {
    let f = font.to_ascii_lowercase();
    if f.contains("courier") || f.contains("mono") || f.contains("consolas") {
        2
    } else if f.contains("times") || f.contains("serif") && !f.contains("sans") || f.contains("cambria") || f.contains("georgia") || f.contains("garamond") {
        1
    } else {
        0
    }
}

const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Width of text in points, from the standard PDF font metrics.
pub fn text_width(s: &str, size: f32, face: u8, bold: bool) -> f32 {
    if face == 2 {
        return s.chars().count() as f32 * 0.6 * size;
    }
    let mut w = 0.0;
    for ch in s.chars() {
        let k = ch as u32;
        w += if (32..127).contains(&k) { HELV[(k - 32) as usize] as f32 } else { 556.0 };
    }
    let f = match (face, bold) {
        (1, false) => 0.9,
        (1, true) => 0.95,
        (_, true) => 1.06,
        _ => 1.0,
    };
    w * size / 1000.0 * f
}

fn wrap_lines(s: &str, width: f32, size: f32, face: u8, bold: bool) -> Vec<String> {
    let mut out = vec![];
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let trial = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width(&trial, size, face, bold) > width && !line.is_empty() {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = trial;
            }
        }
        out.push(line);
    }
    out
}

fn line_width(b: &Border) -> f32 {
    match b.style {
        LineStyle::Medium => 1.5,
        LineStyle::Thick => 2.25,
        LineStyle::Double => 2.0,
        LineStyle::Hair => 0.4,
        _ => 0.75,
    }
}

fn expand_codes(s: &str, sheet: &str, page: usize, total: usize) -> String {
    let (y, m, d) = crate::eval::today_ymd();
    s.replace("&P", &page.to_string()).replace("&N", &total.to_string()).replace("&A", sheet).replace("&D", &format!("{m}/{d}/{y}")).replace("&F", "Book")
}

/// Drawing operations for one printed page.
pub fn page_ops(wb: &Workbook, calc: &CalcResult, si: usize, l: &Layout, page: usize, cond: &crate::cond::CondCache) -> Vec<Op> {
    let sh = &wb.sheets[si];
    let p = &sh.page;
    let s = l.scale;
    let pr = l.pages[page];
    let mut ops = vec![];
    let left = p.margins[0] * 72.0;
    let top = p.margins[2] * 72.0;
    let head_w = if p.headings { HEAD_W * s } else { 0.0 };
    let head_h = if p.headings { HEAD_H * s } else { 0.0 };
    let ox = left + head_w;
    let oy = top + head_h;
    // Column and row edges within the page.
    let mut xs = vec![ox];
    for c in pr.c1..=pr.c2 {
        let last = *xs.last().unwrap();
        xs.push(last + sh.col_px(c) * s);
    }
    let mut ys = vec![oy];
    for r in pr.r1..=pr.r2 {
        let last = *ys.last().unwrap();
        ys.push(last + sh.row_px(r) * s);
    }
    let right = *xs.last().unwrap();
    let bottom = *ys.last().unwrap();
    let cx = |c: u32| xs[(c.clamp(pr.c1, pr.c2 + 1) - pr.c1) as usize];
    let cy = |r: u32| ys[(r.clamp(pr.r1, pr.r2 + 1) - pr.r1) as usize];
    let font_scale = s / 0.75;
    if p.headings {
        let hs = 8.0 * font_scale;
        ops.push(Op::Fill([left, top, right, oy], 0xF2F2F2));
        ops.push(Op::Fill([left, oy, ox, bottom], 0xF2F2F2));
        for c in pr.c1..=pr.c2 {
            if sh.col_px(c) == 0.0 {
                continue;
            }
            let n = col_name(c);
            let w = text_width(&n, hs, 0, false);
            ops.push(Op::Text { x: (cx(c) + cx(c + 1) - w) / 2.0, y: oy - head_h * 0.3, size: hs, face: 0, bold: false, italic: false, color: 0x404040, text: n, clip: None });
            ops.push(Op::Line([cx(c + 1), top, cx(c + 1), oy], 0.5, 0xA0A0A0));
        }
        for r in pr.r1..=pr.r2 {
            if sh.row_px(r) == 0.0 {
                continue;
            }
            let n = (r + 1).to_string();
            let w = text_width(&n, hs, 0, false);
            ops.push(Op::Text { x: left + (head_w - w) / 2.0, y: cy(r + 1) - (cy(r + 1) - cy(r)) * 0.3, size: hs, face: 0, bold: false, italic: false, color: 0x404040, text: n, clip: None });
            ops.push(Op::Line([left, cy(r + 1), ox, cy(r + 1)], 0.5, 0xA0A0A0));
        }
        ops.push(Op::Line([left, oy, right, oy], 0.5, 0xA0A0A0));
        ops.push(Op::Line([ox, top, ox, bottom], 0.5, 0xA0A0A0));
    }
    // Table header and banded rows: (fill, text colour, bold).
    let table_look = |r: u32, c: u32| -> Option<(Option<u32>, Option<u32>, bool)> {
        let t = sh.tables.iter().find(|t| t.range.contains(Pos::new(r, c)))?;
        let (head, band) = crate::grid::table_colors(t.style);
        if t.header && r == t.range.r1 {
            return Some((Some(head), Some(0xFFFFFF), true));
        }
        if t.totals && r == t.range.r2 {
            return Some((None, None, true));
        }
        let k = r - t.range.r1 - t.header as u32;
        (t.banded && k % 2 == 0).then_some((Some(band), None, false))
    };
    let merged_away = |r: u32, c: u32| sh.merges.iter().any(|m| m.contains(Pos::new(r, c)) && !(m.r1.max(pr.r1) == r && m.c1.max(pr.c1) == c));
    // Fills.
    for r in pr.r1..=pr.r2 {
        if sh.row_px(r) == 0.0 {
            continue;
        }
        for c in pr.c1..=pr.c2 {
            if sh.col_px(c) == 0.0 || merged_away(r, c) {
                continue;
            }
            let st = wb.styles.get(sh.style_id(r, c));
            let hit = cond.map.get(&(r, c));
            let fill = hit.and_then(|h| h.style.as_ref().and_then(|x| x.fill)).or(hit.and_then(|h| h.scale)).or(st.fill).or(table_look(r, c).and_then(|t| t.0));
            let m = sh.merge_at(Pos::new(r, c)).unwrap_or(Rect::cell(Pos::new(r, c)));
            let rect = [cx(m.c1.max(pr.c1)), cy(m.r1.max(pr.r1)), cx(m.c2.min(pr.c2) + 1), cy(m.r2.min(pr.r2) + 1)];
            if let Some(f) = fill {
                ops.push(Op::Fill(rect, f));
            }
            if let Some((t, col)) = hit.and_then(|h| h.bar) {
                let pad = 1.5 * s;
                ops.push(Op::Fill([rect[0] + pad, rect[1] + pad, rect[0] + pad + (rect[2] - rect[0] - 2.0 * pad) * t, rect[3] - pad], crate::theme::tint(col, 0.35)));
            }
        }
    }
    if p.gridlines {
        for c in pr.c1..=pr.c2 + 1 {
            ops.push(Op::Line([cx(c), oy, cx(c), bottom], 0.25, 0xBFBFBF));
        }
        for r in pr.r1..=pr.r2 + 1 {
            ops.push(Op::Line([ox, cy(r), right, cy(r)], 0.25, 0xBFBFBF));
        }
    }
    // Text.
    for r in pr.r1..=pr.r2 {
        if sh.row_px(r) == 0.0 {
            continue;
        }
        for c in pr.c1..=pr.c2 {
            if sh.col_px(c) == 0.0 || merged_away(r, c) {
                continue;
            }
            let v = calc.value(wb, si, r, c);
            if matches!(v, Value::Empty) {
                continue;
            }
            let st = wb.styles.get(sh.style_id(r, c));
            let hit = cond.map.get(&(r, c));
            let m = sh.merge_at(Pos::new(r, c)).unwrap_or(Rect::cell(Pos::new(r, c)));
            let (x0, y0, x1, y1) = (cx(m.c1.max(pr.c1)), cy(m.r1.max(pr.r1)), cx(m.c2.min(pr.c2) + 1), cy(m.r2.min(pr.r2) + 1));
            let size = st.size10 as f32 / 10.0 * font_scale;
            let face = face_of(&st.font);
            let look = table_look(r, c);
            let bold = st.bold || look.is_some_and(|t| t.2) || hit.and_then(|h| h.style.as_ref()).is_some_and(|x| x.bold);
            let italic = st.italic || hit.and_then(|h| h.style.as_ref()).is_some_and(|x| x.italic);
            let chars = ((x1 - x0) / (size * 0.55)).max(1.0) as usize;
            let rend = ops::render_value(&v, st, chars);
            let mut text = format!("{}{}", rend.left, rend.text);
            let color = hit.and_then(|h| h.style.as_ref().and_then(|x| x.color)).or(rend.color).or(st.color).or(look.and_then(|t| t.1)).unwrap_or(0);
            let is_num = matches!(v, Value::Num(_));
            let align = match st.h {
                HAlign::Left | HAlign::Fill | HAlign::Justify => 0,
                HAlign::Center | HAlign::CenterAcross => 1,
                HAlign::Right => 2,
                HAlign::General => match v {
                    Value::Num(_) => 2,
                    Value::Bool(_) | Value::Err(_) => 1,
                    _ => 0,
                },
            };
            let pad = 2.0 * s + st.indent as f32 * 9.0 * s;
            let mut tw = text_width(&text, size, face, bold);
            let mut clip_r = x1;
            let mut clip_l = x0;
            if is_num && tw > x1 - x0 - 2.0 * s {
                text = "#".repeat(((x1 - x0) / text_width("#", size, face, bold)).max(1.0) as usize);
                tw = text_width(&text, size, face, bold);
            } else if !st.wrap && tw > x1 - x0 - pad {
                // Text runs on over empty neighbours.
                if align == 0 {
                    let mut k = m.c2 + 1;
                    while k <= pr.c2 && clip_r - x0 < tw + pad && sh.is_empty_at(r, k) && matches!(calc.value(wb, si, r, k), Value::Empty) {
                        clip_r = cx(k + 1);
                        k += 1;
                    }
                } else if align == 2 {
                    let mut k = m.c1;
                    while k > pr.c1 && x1 - clip_l < tw + pad && sh.is_empty_at(r, k - 1) && matches!(calc.value(wb, si, r, k - 1), Value::Empty) {
                        k -= 1;
                        clip_l = cx(k);
                    }
                }
            }
            let lines = if st.wrap { wrap_lines(&text, x1 - x0 - 2.0 * pad.min(4.0 * s), size, face, bold) } else { vec![text] };
            let lh = size * 1.2;
            let block = lh * lines.len() as f32;
            let first_base = match st.v {
                VAlign::Top => y0 + size + 1.0 * s,
                VAlign::Center => (y0 + y1) / 2.0 - block / 2.0 + size,
                VAlign::Bottom => y1 - block + size - 0.25 * size - 1.0 * s,
            };
            for (i, line) in lines.into_iter().enumerate() {
                let w = if i == 0 && !st.wrap { tw } else { text_width(&line, size, face, bold) };
                let x = match align {
                    0 => x0 + pad,
                    1 => (x0 + x1 - w) / 2.0,
                    _ => x1 - pad - w,
                };
                ops.push(Op::Text { x, y: first_base + lh * i as f32, size, face, bold, italic, color, text: line, clip: Some([clip_l, y0, clip_r, y1]) });
            }
            if st.underline > 0 {
                let x = match align {
                    0 => x0 + pad,
                    1 => (x0 + x1 - tw) / 2.0,
                    _ => x1 - pad - tw,
                };
                let y = first_base + size * 0.12;
                ops.push(Op::Line([x, y, x + tw.min(clip_r - x), y], size * 0.06, color));
            }
        }
    }
    // Borders.
    for r in pr.r1..=pr.r2 {
        for c in pr.c1..=pr.c2 {
            let st = wb.styles.get(sh.style_id(r, c));
            if st.borders.iter().all(|b| b.is_none()) {
                continue;
            }
            let (x0, y0, x1, y1) = (cx(c), cy(r), cx(c + 1), cy(r + 1));
            if let Some(b) = st.borders[B_LEFT] {
                ops.push(Op::Line([x0, y0, x0, y1], line_width(&b), b.color));
            }
            if let Some(b) = st.borders[B_RIGHT] {
                ops.push(Op::Line([x1, y0, x1, y1], line_width(&b), b.color));
            }
            if let Some(b) = st.borders[B_TOP] {
                ops.push(Op::Line([x0, y0, x1, y0], line_width(&b), b.color));
            }
            if let Some(b) = st.borders[B_BOTTOM] {
                ops.push(Op::Line([x0, y1, x1, y1], line_width(&b), b.color));
            }
        }
    }
    // Charts that fall on this page.
    let px0: f32 = (0..pr.c1).map(|c| sh.col_px(c)).sum();
    let py0: f32 = (0..pr.r1).map(|r| sh.row_px(r)).sum();
    let pw_px = (right - ox) / s;
    let ph_px = (bottom - oy) / s;
    for (i, ch) in sh.charts.iter().enumerate() {
        let (x, y) = (ch.x - px0, ch.y - py0);
        if x + ch.w <= 0.0 || y + ch.h <= 0.0 || x >= pw_px + 1.0 || y >= ph_px + 1.0 {
            continue;
        }
        ops.push(Op::Chart { index: i, rect: [ox + x * s, oy + y * s, ox + (x + ch.w) * s, oy + (y + ch.h) * s], scale: s / 0.75 });
    }
    // Header and footer.
    let total = l.pages.len();
    for (txt, y) in [(&p.header, 0.4 * 72.0), (&p.footer, l.page_h - 0.35 * 72.0)] {
        if txt.is_empty() {
            continue;
        }
        let t = expand_codes(txt, &sh.name, page + 1, total);
        let w = text_width(&t, 9.0, 0, false);
        ops.push(Op::Text { x: (l.page_w - w) / 2.0, y, size: 9.0, face: 0, bold: false, italic: false, color: 0x000000, text: t, clip: None });
    }
    ops
}

// ---------- PDF writer ----------

fn pdf_str(s: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for ch in s.chars() {
        let b = match ch as u32 {
            0x20AC => 0x80,
            0x2018 => 0x91,
            0x2019 => 0x92,
            0x201C => 0x93,
            0x201D => 0x94,
            0x2022 => 0x95,
            0x2013 => 0x96,
            0x2014 => 0x97,
            k @ 0x20..=0x7E => k as u8,
            k @ 0xA0..=0xFF => k as u8,
            _ => b'?',
        };
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

fn rgb_op(c: u32, stroke: bool) -> String {
    let (r, g, b) = ((c >> 16 & 255) as f32 / 255.0, (c >> 8 & 255) as f32 / 255.0, (c & 255) as f32 / 255.0);
    format!("{r:.3} {g:.3} {b:.3} {}\n", if stroke { "RG" } else { "rg" })
}

const FONT_NAMES: [&str; 12] = [
    "Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique",
    "Times-Roman", "Times-Bold", "Times-Italic", "Times-BoldItalic",
    "Courier", "Courier-Bold", "Courier-Oblique", "Courier-BoldOblique",
];

fn font_index(face: u8, bold: bool, italic: bool) -> usize {
    face as usize * 4 + bold as usize + italic as usize * 2
}

/// Draws a chart with a hidden egui context and turns the shapes into page operations.
fn chart_ops(wb: &Workbook, calc: &CalcResult, si: usize, index: usize, rect: [f32; 4], scale: f32) -> Vec<Op> {
    thread_local! {
        static CTX: std::cell::RefCell<Option<egui::Context>> = const { std::cell::RefCell::new(None) };
    }
    let ch = &wb.sheets[si].charts[index];
    let size = egui::vec2(ch.w, ch.h);
    let ctx = CTX.with(|c| {
        c.borrow_mut()
            .get_or_insert_with(|| {
                let ctx = egui::Context::default();
                crate::theme::install_fonts(&ctx);
                let mut warm = ctx.run_ui(egui::RawInput::default(), |_| {});
                warm.textures_delta.clear();
                ctx
            })
            .clone()
    });
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size + egui::vec2(20.0, 20.0))), ..Default::default() };
    let mut out = ctx.run_ui(input, |ui| {
        let p = ui.ctx().layer_painter(egui::LayerId::background());
        crate::charts::draw(&p, egui::Rect::from_min_size(egui::Pos2::ZERO, size), ch, wb, calc, si, 1.0);
    });
    // Nothing uploads this context's font atlas, so its texture updates are not needed.
    out.textures_delta.clear();
    let k = (rect[2] - rect[0]) / ch.w;
    let tx = |p: egui::Pos2| (rect[0] + p.x * k, rect[1] + p.y * k);
    let col = |c: egui::Color32| ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32;
    let mut ops = vec![];
    let mut push_poly = |pts: &[egui::Pos2], fill: egui::Color32, ops: &mut Vec<Op>| {
        // Polygons become thin horizontal strips; good enough for chart shapes.
        if fill.a() == 0 || pts.len() < 3 {
            return;
        }
        let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p.y), h.max(p.y)));
        let mut y = lo;
        let step = 0.75 / k;
        while y < hi {
            let ym = y + step / 2.0;
            let mut xs = vec![];
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (a.y <= ym) != (b.y <= ym) {
                    xs.push(a.x + (ym - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for w in xs.chunks(2) {
                if let [x0, x1] = w {
                    let (ax, ay) = tx(egui::pos2(*x0, y));
                    let (bx, by) = tx(egui::pos2(*x1, (y + step).min(hi)));
                    ops.push(Op::Fill([ax, ay, bx, by], col(fill)));
                }
            }
            y += step;
        }
    };
    let _ = scale;
    for cs in out.shapes {
        walk(cs.shape, &mut |shape| match shape {
            egui::Shape::Rect(r) => {
                let (x0, y0) = tx(r.rect.min);
                let (x1, y1) = tx(r.rect.max);
                if r.fill.a() > 0 {
                    ops.push(Op::Fill([x0, y0, x1, y1], col(r.fill)));
                }
                if r.stroke.width > 0.0 && r.stroke.color.a() > 0 {
                    let w = r.stroke.width * k;
                    let c = col(r.stroke.color);
                    ops.push(Op::Line([x0, y0, x1, y0], w, c));
                    ops.push(Op::Line([x0, y1, x1, y1], w, c));
                    ops.push(Op::Line([x0, y0, x0, y1], w, c));
                    ops.push(Op::Line([x1, y0, x1, y1], w, c));
                }
            }
            egui::Shape::LineSegment { points, stroke } => {
                let (a, b) = (tx(points[0]), tx(points[1]));
                ops.push(Op::Line([a.0, a.1, b.0, b.1], stroke.width * k, col(stroke.color)));
            }
            egui::Shape::Path(ps) => {
                if ps.closed {
                    push_poly(&ps.points, ps.fill, &mut ops);
                }
                if ps.stroke.width > 0.0 {
                    let c = match ps.stroke.color {
                        egui::epaint::ColorMode::Solid(c) => c,
                        _ => egui::Color32::GRAY,
                    };
                    let n = ps.points.len();
                    let segs = if ps.closed { n } else { n.saturating_sub(1) };
                    for i in 0..segs {
                        let (a, b) = (tx(ps.points[i]), tx(ps.points[(i + 1) % n]));
                        ops.push(Op::Line([a.0, a.1, b.0, b.1], ps.stroke.width * k, col(c)));
                    }
                }
            }
            egui::Shape::Circle(c) => {
                let pts: Vec<egui::Pos2> = (0..16).map(|i| {
                    let t = i as f32 / 16.0 * std::f32::consts::TAU;
                    c.center + egui::vec2(t.cos(), t.sin()) * c.radius
                }).collect();
                push_poly(&pts, c.fill, &mut ops);
            }
            egui::Shape::Text(t) => {
                let color = if t.fallback_color.a() > 0 { col(t.fallback_color) } else { 0x595959 };
                let size = t.galley.job.sections.first().map(|s| s.format.font_id.size).unwrap_or(10.0) * k;
                let (x, y) = tx(t.pos);
                let text = t.galley.text().to_string();
                if t.angle.abs() > 0.1 {
                    return;
                }
                ops.push(Op::Text { x, y: y + size * 0.95, size, face: 0, bold: false, italic: false, color, text, clip: None });
            }
            _ => {}
        });
    }
    ops
}

fn walk(s: egui::Shape, f: &mut dyn FnMut(egui::Shape)) {
    if let egui::Shape::Vec(v) = s {
        for x in v {
            walk(x, f);
        }
    } else {
        f(s)
    }
}

/// Writes the given sheets as a PDF document.
pub fn pdf(wb: &Workbook, calc: &CalcResult, sheets: &[usize]) -> Vec<u8> {
    let mut pages: Vec<(f32, f32, Vec<u8>)> = vec![];
    for &si in sheets {
        let Some(l) = layout(wb, si) else { continue };
        let cond = crate::cond::compute(wb, calc, si, 0);
        for i in 0..l.pages.len() {
            let mut list = page_ops(wb, calc, si, &l, i, &cond);
            let mut expanded = vec![];
            for op in list.drain(..) {
                if let Op::Chart { index, rect, scale } = op {
                    expanded.extend(chart_ops(wb, calc, si, index, rect, scale));
                } else {
                    expanded.push(op);
                }
            }
            pages.push((l.page_w, l.page_h, content_stream(&expanded, l.page_h)));
        }
    }
    if pages.is_empty() {
        pages.push((612.0, 792.0, Vec::new()));
    }
    write_pdf(&pages)
}

fn content_stream(ops: &[Op], ph: f32) -> Vec<u8> {
    let mut s: Vec<u8> = vec![];
    for op in ops {
        match op {
            Op::Fill(r, c) => {
                s.extend(rgb_op(*c, false).bytes());
                s.extend(format!("{:.2} {:.2} {:.2} {:.2} re f\n", r[0], ph - r[3], r[2] - r[0], r[3] - r[1]).bytes());
            }
            Op::Line(l, w, c) => {
                s.extend(rgb_op(*c, true).bytes());
                s.extend(format!("{:.2} w {:.2} {:.2} m {:.2} {:.2} l S\n", w, l[0], ph - l[1], l[2], ph - l[3]).bytes());
            }
            Op::Text { x, y, size, face, bold, italic, color, text, clip } => {
                if text.is_empty() {
                    continue;
                }
                if let Some(c) = clip {
                    s.extend(format!("q {:.2} {:.2} {:.2} {:.2} re W n\n", c[0], ph - c[3], c[2] - c[0], c[3] - c[1]).bytes());
                }
                s.extend(rgb_op(*color, false).bytes());
                s.extend(format!("BT /F{} {:.2} Tf {:.2} {:.2} Td ", font_index(*face, *bold, *italic) + 1, size, x, ph - y).bytes());
                s.extend(pdf_str(text));
                s.extend(b" Tj ET\n");
                if clip.is_some() {
                    s.extend(b"Q\n");
                }
            }
            Op::Chart { .. } => {}
        }
    }
    s
}

fn write_pdf(pages: &[(f32, f32, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = vec![];
    let n_fonts = FONT_NAMES.len();
    // Object numbers: 1 catalog, 2 page tree, 3.. fonts, then per page: content, page.
    let first_page_obj = 3 + n_fonts;
    let mut obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let n = offsets.len();
        out.extend(format!("{n} 0 obj\n").bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    };
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", first_page_obj + i * 2 + 1)).collect();
    obj(&mut out, &mut offsets, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()).as_bytes());
    for f in FONT_NAMES {
        obj(&mut out, &mut offsets, format!("<< /Type /Font /Subtype /Type1 /BaseFont /{f} /Encoding /WinAnsiEncoding >>").as_bytes());
    }
    let fonts: Vec<String> = (0..n_fonts).map(|i| format!("/F{} {} 0 R", i + 1, i + 3)).collect();
    let font_dict = fonts.join(" ");
    for (i, (w, h, content)) in pages.iter().enumerate() {
        let mut body = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        body.extend(content);
        body.extend(b"\nendstream");
        obj(&mut out, &mut offsets, &body);
        let content_obj = first_page_obj + i * 2;
        obj(&mut out, &mut offsets, format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.0} {h:.0}] /Resources << /Font << {font_dict} >> >> /Contents {content_obj} 0 R >>").as_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).bytes());
    for o in &offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).bytes());
    out
}
