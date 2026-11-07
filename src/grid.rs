//! Paints the cell grid: headers, cells, borders, selection, charts and overlays.

use crate::app::{App, Drag, EditMode, ViewMode};
use crate::eval::Value;
use crate::model::*;
use crate::numfmt;
use crate::ops;
use crate::theme::{self, *};
use egui::{pos2, vec2, Align2, Color32, Painter, Rect as R, Stroke, StrokeKind};
use std::cell::Cell as StdCell;
use std::sync::Arc;

thread_local! {
    static COUNTS: StdCell<(u32, u32)> = const { StdCell::new((30, 15)) };
}

pub fn editor_id() -> egui::Id {
    egui::Id::new("cell-editor")
}

/// Rows and columns of the scrolling part that fit on screen, from the last frame.
pub fn visible_counts(_a: &App) -> (u32, u32) {
    COUNTS.with(|c| c.get())
}

pub struct Geo {
    pub rect: R,
    pub z: f32,
    pub hw: f32,
    pub hh: f32,
    pub cols: Vec<(u32, f32, f32)>,
    pub rows: Vec<(u32, f32, f32)>,
    pub fr: u32,
    pub fc: u32,
    pub left: u32,
    pub top: u32,
    pub sx0: f32,
    pub sy0: f32,
    pub cells: R,
}

impl Geo {
    pub fn new(a: &App, rect: R) -> Geo {
        let sh = a.sheet();
        let z = sh.zoom;
        let hh = if sh.headings { (DEFAULT_ROW_H * z).round() } else { 0.0 };
        let (top, left) = a.view();
        let last_row_guess = top + (rect.height() / (DEFAULT_ROW_H * z * 0.5)) as u32 + 2;
        let digits = (last_row_guess.min(MAX_ROWS) as f32).log10().floor() as i32 + 1;
        let hw = if sh.headings { ((digits.max(3) as f32 * 7.0 + 10.0) * z.max(0.6)).round() } else { 0.0 };
        let (fr, fc) = sh.freeze;
        let cells = R::from_min_max(rect.min + vec2(hw, hh), rect.max);
        let mut cols = vec![];
        let mut x = cells.left();
        for c in 0..fc {
            let w = (sh.col_px(c) * z).round();
            cols.push((c, x, w));
            x += w;
        }
        let sx0 = x;
        let mut c = left.max(fc);
        while x < cells.right() && c < MAX_COLS {
            let w = (sh.col_px(c) * z).round();
            cols.push((c, x, w));
            x += w;
            c += 1;
        }
        let mut rows = vec![];
        let mut y = cells.top();
        for r in 0..fr {
            let h = (sh.row_px(r) * z).round();
            rows.push((r, y, h));
            y += h;
        }
        let sy0 = y;
        let mut r = top.max(fr);
        while y < cells.bottom() && r < MAX_ROWS {
            let h = (sh.row_px(r) * z).round();
            rows.push((r, y, h));
            y += h;
            r += 1;
        }
        let full_rows = rows.iter().filter(|(r, y, h)| *r >= fr && y + h <= cells.bottom()).count() as u32;
        let full_cols = cols.iter().filter(|(c, x, w)| *c >= fc && x + w <= cells.right()).count() as u32;
        COUNTS.with(|k| k.set((full_rows.max(1), full_cols.max(1))));
        Geo { rect, z, hw, hh, cols, rows, fr, fc, left: left.max(fc), top: top.max(fr), sx0, sy0, cells }
    }

    fn pos_in(list: &[(u32, f32, f32)], frozen: u32, first: u32, start: f32, i: u32, size: &dyn Fn(u32) -> f32) -> f32 {
        if let Ok(k) = list.binary_search_by(|e| e.0.cmp(&i)) {
            return list[k].1;
        }
        if i < frozen {
            return list.iter().find(|e| e.0 == i).map_or(start, |e| e.1);
        }
        if i < first {
            let n = first - i;
            if n > 300 {
                return start - 100_000.0;
            }
            return start - (i..first).map(size).sum::<f32>();
        }
        let (lc, lx, lw) = list.last().copied().unwrap_or((first, start, 0.0));
        let n = i.saturating_sub(lc + 1);
        if n > 300 {
            return lx + lw + 100_000.0;
        }
        lx + lw + (lc + 1..i).map(size).sum::<f32>()
    }

    pub fn x(&self, a: &App, c: u32) -> f32 {
        let sh = a.sheet();
        Self::pos_in(&self.cols, self.fc, self.left, self.sx0, c, &|k| (sh.col_px(k) * self.z).round())
    }
    pub fn y(&self, a: &App, r: u32) -> f32 {
        let sh = a.sheet();
        Self::pos_in(&self.rows, self.fr, self.top, self.sy0, r, &|k| (sh.row_px(k) * self.z).round())
    }
    pub fn rect_of(&self, a: &App, rc: Rect) -> R {
        let x1 = self.x(a, rc.c1);
        let y1 = self.y(a, rc.r1);
        let x2 = if rc.c2 >= MAX_COLS - 1 { self.cells.right() + 10.0 } else { self.x(a, rc.c2 + 1) };
        let y2 = if rc.r2 >= MAX_ROWS - 1 { self.cells.bottom() + 10.0 } else { self.y(a, rc.r2 + 1) };
        R::from_min_max(pos2(x1, y1), pos2(x2, y2))
    }

    /// The cell under a screen point. Points beyond the edges give the nearest cell.
    pub fn hit(&self, a: &App, p: egui::Pos2) -> Pos {
        let col = if p.x < self.cells.left() {
            if self.fc > 0 { 0 } else { self.left.saturating_sub(1) }
        } else {
            self.cols.iter().find(|(_, x, w)| p.x >= *x && p.x < x + w).map(|e| e.0).unwrap_or_else(|| self.cols.last().map_or(0, |e| (e.0 + 1).min(MAX_COLS - 1)))
        };
        let row = if p.y < self.cells.top() {
            if self.fr > 0 { 0 } else { self.top.saturating_sub(1) }
        } else {
            self.rows.iter().find(|(_, y, h)| p.y >= *y && p.y < y + h).map(|e| e.0).unwrap_or_else(|| self.rows.last().map_or(0, |e| (e.0 + 1).min(MAX_ROWS - 1)))
        };
        let _ = a;
        Pos::new(row, col)
    }

    /// The four panes as (clip rect, row range, col range) in the visible lists.
    pub fn panes(&self) -> Vec<(R, std::ops::Range<usize>, std::ops::Range<usize>)> {
        let nfr = self.rows.iter().take_while(|e| e.0 < self.fr).count();
        let nfc = self.cols.iter().take_while(|e| e.0 < self.fc).count();
        let (l, t, rr, b) = (self.cells.left(), self.cells.top(), self.cells.right(), self.cells.bottom());
        let mut out = vec![];
        let ys = [(t, self.sy0, 0..nfr), (self.sy0, b, nfr..self.rows.len())];
        let xs = [(l, self.sx0, 0..nfc), (self.sx0, rr, nfc..self.cols.len())];
        for (y1, y2, rws) in ys.iter() {
            for (x1, x2, cls) in xs.iter() {
                if y2 > y1 && x2 > x1 {
                    out.push((R::from_min_max(pos2(*x1, *y1), pos2(*x2, *y2)), rws.clone(), cls.clone()));
                }
            }
        }
        out
    }
}

fn dash_rect(p: &Painter, r: R, color: Color32, width: f32, phase: f32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        let shapes = egui::Shape::dashed_line_with_offset(&[w[0], w[1]], Stroke::new(width, color), &[4.0], &[3.0], phase);
        p.extend(shapes);
    }
}

fn border_stroke(b: &Border, z: f32) -> (f32, Color32, u8) {
    let w = match b.style {
        LineStyle::Medium => 2.0,
        LineStyle::Thick => 3.0,
        LineStyle::Double => 3.0,
        _ => 1.0,
    };
    let kind = match b.style {
        LineStyle::Dashed => 1,
        LineStyle::Dotted | LineStyle::Hair => 2,
        LineStyle::Double => 3,
        _ => 0,
    };
    ((w * z.max(1.0)).round().max(1.0), theme::rgb(b.color), kind)
}

fn paint_border_line(p: &Painter, a: egui::Pos2, b: egui::Pos2, br: &Border, z: f32) {
    let (w, c, kind) = border_stroke(br, z);
    match kind {
        1 => p.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(w, c), 3.0, 2.0)),
        2 => p.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(w, c), 1.0, 1.0)),
        3 => {
            let horiz = (a.y - b.y).abs() < 0.1;
            let off = if horiz { vec2(0.0, 1.0) } else { vec2(1.0, 0.0) };
            p.line_segment([a - off, b - off], Stroke::new(1.0, c));
            p.line_segment([a + off, b + off], Stroke::new(1.0, c));
        }
        _ => {
            p.line_segment([a, b], Stroke::new(w, c));
        }
    }
}

/// Table style colours: (header, band, accent) for a style number.
pub fn table_colors(style: u8) -> (u32, u32) {
    // Matches Excel's TableStyleMedium1..7: black, then the six accent colours.
    let k = (style as usize + 6) % 7;
    let base = if k == 0 { 0x404040 } else { theme::THEME_TOP[3 + k] };
    (base, theme::tint(base, 0.8))
}

struct CellLook {
    fill: Option<u32>,
    color: Option<u32>,
    bold: bool,
    italic: bool,
}

pub fn paint(a: &mut App, ui: &mut egui::Ui, g: &Geo) {
    if a.cf.sheet != a.cur || a.cf.gen != a.gen {
        a.cf = crate::cond::compute(&a.wb, &a.calc, a.cur, a.gen);
    }
    let p = ui.painter().with_clip_rect(g.rect);
    let sh = a.sheet().clone();
    let si = a.cur;
    let z = g.z;
    p.rect_filled(g.cells, 0.0, WHITE);
    let edit_pos = a.edit.as_ref().filter(|e| e.sheet == si).map(|e| e.pos);
    let tables: Vec<&TableDef> = sh.tables.iter().collect();
    let table_look = |r: u32, c: u32| -> Option<CellLook> {
        let t = tables.iter().find(|t| t.range.contains(Pos::new(r, c)))?;
        let (head, band) = table_colors(t.style);
        if t.header && r == t.range.r1 {
            return Some(CellLook { fill: Some(head), color: Some(0xFFFFFF), bold: true, italic: false });
        }
        if t.totals && r == t.range.r2 {
            return Some(CellLook { fill: None, color: None, bold: true, italic: false });
        }
        let k = r - t.range.r1 - t.header as u32;
        if t.banded && k % 2 == 0 {
            return Some(CellLook { fill: Some(band), color: None, bold: false, italic: false });
        }
        None
    };
    for (clip, rws, cls) in g.panes() {
        let pp = p.with_clip_rect(clip);
        let rows = &g.rows[rws.clone()];
        let cols = &g.cols[cls.clone()];
        if rows.is_empty() || cols.is_empty() {
            continue;
        }
        // Gridlines.
        if sh.gridlines {
            let (x0, x1) = (cols[0].1, cols.last().map(|c| c.1 + c.2).unwrap());
            let (y0, y1) = (rows[0].1, rows.last().map(|r| r.1 + r.2).unwrap());
            for (_, x, w) in cols {
                if *w > 0.0 {
                    pp.vline(x + w - 0.5, y0..=y1, Stroke::new(1.0, GRID));
                }
            }
            for (_, y, h) in rows {
                if *h > 0.0 {
                    pp.hline(x0..=x1, y + h - 0.5, Stroke::new(1.0, GRID));
                }
            }
        }
        // Fills, merges and conditional colours.
        let mut merged_done: Vec<Rect> = vec![];
        for (r, y, h) in rows {
            if *h <= 0.0 {
                continue;
            }
            for (c, x, w) in cols {
                if *w <= 0.0 {
                    continue;
                }
                let pos = Pos::new(*r, *c);
                let (cell_rect, rc) = match sh.merge_at(pos) {
                    Some(m) => {
                        if merged_done.contains(&m) {
                            continue;
                        }
                        merged_done.push(m);
                        (g.rect_of(a, m), m)
                    }
                    None => (R::from_min_size(pos2(*x, *y), vec2(*w, *h)), Rect::cell(pos)),
                };
                let st = a.wb.styles.get(sh.style_id(rc.r1, rc.c1));
                let hit = a.cf.map.get(&(rc.r1, rc.c1));
                let tl = table_look(rc.r1, rc.c1);
                let fill = hit.and_then(|h| h.style.as_ref().and_then(|s| s.fill)).or(hit.and_then(|h| h.scale)).or(st.fill).or(tl.as_ref().and_then(|t| t.fill));
                if let Some(f) = fill {
                    pp.rect_filled(cell_rect, 0.0, theme::rgb(f));
                } else if !rc.is_single() {
                    pp.rect_filled(cell_rect, 0.0, WHITE);
                    if sh.gridlines {
                        pp.rect_stroke(cell_rect.translate(vec2(-0.5, -0.5)), 0.0, Stroke::new(1.0, GRID), StrokeKind::Outside);
                    }
                }
                if let Some((frac, col)) = hit.and_then(|h| h.bar) {
                    let bw = (cell_rect.width() - 4.0) * frac.clamp(0.0, 1.0);
                    let br = R::from_min_size(cell_rect.min + vec2(2.0, 2.0), vec2(bw, cell_rect.height() - 5.0));
                    let c0 = theme::rgb(col);
                    pp.rect_filled(br, 0.0, c0.gamma_multiply(0.55));
                    pp.rect_stroke(br, 0.0, Stroke::new(1.0, c0), StrokeKind::Inside);
                }
            }
        }
        // Borders. Each cell draws its own right and bottom borders, plus left and top at the pane edges.
        for (r, y, h) in rows {
            for (c, x, w) in cols {
                let Some(cell_style) = sh.get(*r, *c).map(|x| x.style).or_else(|| Some(sh.base_style(*r, *c))) else { continue };
                if cell_style == 0 {
                    continue;
                }
                let st = a.wb.styles.get(cell_style);
                if st.borders.iter().all(|b| b.is_none()) {
                    continue;
                }
                let cr = R::from_min_size(pos2(*x, *y), vec2(*w, *h));
                let (l, t, rr, b) = (cr.left() - 0.5, cr.top() - 0.5, cr.right() - 0.5, cr.bottom() - 0.5);
                if let Some(bd) = &st.borders[B_LEFT] {
                    paint_border_line(&pp, pos2(l, t), pos2(l, b), bd, z);
                }
                if let Some(bd) = &st.borders[B_TOP] {
                    paint_border_line(&pp, pos2(l, t), pos2(rr, t), bd, z);
                }
                if let Some(bd) = &st.borders[B_RIGHT] {
                    paint_border_line(&pp, pos2(rr, t), pos2(rr, b), bd, z);
                }
                if let Some(bd) = &st.borders[B_BOTTOM] {
                    paint_border_line(&pp, pos2(l, b), pos2(rr, b), bd, z);
                }
            }
        }
        // Text. Cells left of the pane may overflow into it, so look a little to the left.
        let first_col = cols[0].0;
        let last_col = cols.last().unwrap().0;
        let right_edge = cols.last().map(|c| c.1 + c.2).unwrap();
        let mut merged_text: Vec<Rect> = vec![];
        for (r, y, h) in rows {
            if *h <= 0.0 {
                continue;
            }
            let row_map = sh.rows.get(r);
            let spill_cells: Vec<u32> = if a.calc.spill.is_empty() { vec![] } else { (first_col.saturating_sub(10)..=last_col).filter(|c| a.calc.spill.contains_key(&(si as u32, *r, *c))).collect() };
            let mut cs: Vec<u32> = row_map.map(|m| m.range(first_col.saturating_sub(30).max(if first_col >= g.fc { g.fc.min(first_col) } else { 0 })..=last_col).map(|(c, _)| *c).collect()).unwrap_or_default();
            cs.extend(spill_cells);
            for m in &sh.merges {
                if m.r1 <= *r && *r <= m.r2 && m.c2 >= first_col && m.c1 <= last_col && !cs.contains(&m.c1) {
                    cs.push(m.c1);
                }
            }
            cs.sort();
            cs.dedup();
            for c in cs {
                if Some(Pos::new(*r, c)) == edit_pos {
                    continue;
                }
                let pos = Pos::new(*r, c);
                let merge = sh.merge_at(pos);
                if let Some(m) = merge {
                    let first_row = m.r1.max(rows[0].0);
                    if *r != first_row || c != m.c1 || merged_text.contains(&m) {
                        continue;
                    }
                    merged_text.push(m);
                }
                let (src_r, src_c) = merge.map_or((*r, c), |m| (m.r1, m.c1));
                let v = a.calc.value(&a.wb, si, src_r, src_c);
                let has_formula = matches!(sh.get(src_r, src_c).map(|x| &x.v), Some(Content::Formula(_)));
                if matches!(v, Value::Empty) && !(a.show_formulas && has_formula) {
                    continue;
                }
                let cell_rect = match merge {
                    Some(m) => g.rect_of(a, m),
                    None => R::from_min_size(pos2(g.x(a, c), *y), vec2((sh.col_px(c) * z).round(), *h)),
                };
                if cell_rect.width() <= 0.0 {
                    continue;
                }
                let st = a.wb.styles.get(sh.style_id(src_r, src_c)).clone();
                let hit = a.cf.map.get(&(src_r, src_c));
                let tl = table_look(src_r, src_c);
                let mut rendered = if a.show_formulas && has_formula {
                    numfmt::Rendered { text: ops::edit_text(&a.wb, si, src_r, src_c), ..Default::default() }
                } else {
                    ops::render_value(&v, &st, ((cell_rect.width() / (7.0 * z)).floor() as usize).clamp(1, 11))
                };
                if hit.is_some_and(|h| h.bar_only) {
                    rendered.text.clear();
                }
                let link = sh.links.contains_key(&Pos::new(src_r, src_c));
                let color = rendered
                    .color
                    .or(hit.and_then(|h| h.style.as_ref().and_then(|s| s.color)))
                    .or(st.color)
                    .or(tl.as_ref().and_then(|t| t.color))
                    .map(theme::rgb)
                    .unwrap_or(if link { LINK } else { Color32::BLACK });
                let bold = st.bold || hit.is_some_and(|h| h.style.as_ref().is_some_and(|s| s.bold)) || tl.as_ref().is_some_and(|t| t.bold);
                let italic = st.italic || hit.is_some_and(|h| h.style.as_ref().is_some_and(|s| s.italic));
                let fam = theme::cell_family(&st.font, bold, italic);
                let mut px = theme::cell_px(st.size10, z);
                let font = egui::FontId::new(px, fam.clone());
                let is_num = matches!(v, Value::Num(_)) && !(a.show_formulas && has_formula);
                let align = match st.h {
                    HAlign::General => {
                        if is_num || matches!(v, Value::Err(_)) {
                            HAlign::Right
                        } else if matches!(v, Value::Bool(_)) {
                            HAlign::Center
                        } else {
                            HAlign::Left
                        }
                    }
                    HAlign::Justify => HAlign::Left,
                    x => x,
                };
                let pad = (2.0 * z).max(1.0);
                let indent = st.indent as f32 * 9.0 * z;
                let avail = cell_rect.width() - 2.0 * pad - indent;
                let mut galley;
                if st.wrap && !is_num {
                    galley = p.layout(rendered.text.clone(), font.clone(), color, avail.max(4.0));
                } else {
                    galley = p.layout_no_wrap(rendered.text.clone(), font.clone(), color);
                    if st.shrink && galley.size().x > avail && galley.size().x > 0.0 {
                        px *= (avail / galley.size().x).max(0.3);
                        galley = p.layout_no_wrap(rendered.text.clone(), egui::FontId::new(px, fam.clone()), color);
                    }
                    if is_num && galley.size().x > avail + 1.0 {
                        let mut fixed = None;
                        if let Value::Num(n) = v {
                            if st.num_fmt.eq_ignore_ascii_case("general") {
                                for chars in (1..11).rev() {
                                    let t = numfmt::general(n, chars);
                                    let gl = p.layout_no_wrap(t.clone(), font.clone(), color);
                                    if gl.size().x <= avail + 1.0 && !t.contains('#') {
                                        fixed = Some(gl);
                                        break;
                                    }
                                }
                            }
                        }
                        galley = fixed.unwrap_or_else(|| {
                            let hw = p.layout_no_wrap("#".into(), font.clone(), color).size().x.max(1.0);
                            let n = ((avail / hw).floor() as usize).max(1);
                            p.layout_no_wrap("#".repeat(n), font.clone(), color)
                        });
                        rendered.left.clear();
                    }
                }
                if st.h == HAlign::Fill && !rendered.text.is_empty() && galley.size().x > 0.0 {
                    let n = (avail / galley.size().x).floor().max(1.0) as usize;
                    galley = p.layout_no_wrap(rendered.text.repeat(n), font.clone(), color);
                }
                let gw = galley.size().x;
                let gh = galley.size().y;
                // Overflow into empty neighbours for unwrapped text.
                let mut text_clip = cell_rect;
                if !st.wrap && !is_num && merge.is_none() && gw > avail && st.rotation == 0 {
                    let mut right = cell_rect.right();
                    let mut left = cell_rect.left();
                    let empty = |k: u32| sh.is_empty_at(*r, k) && !a.calc.spill.contains_key(&(si as u32, *r, k)) && sh.merge_at(Pos::new(*r, k)).is_none() && Some(Pos::new(*r, k)) != edit_pos;
                    if matches!(align, HAlign::Left | HAlign::Center) {
                        let mut k = c + 1;
                        let need = if align == HAlign::Center { (gw - avail) / 2.0 } else { gw - avail };
                        while right - cell_rect.right() < need && k < MAX_COLS && empty(k) && right < right_edge + 2000.0 {
                            right += (sh.col_px(k) * z).round();
                            k += 1;
                        }
                    }
                    if matches!(align, HAlign::Right | HAlign::Center) {
                        let mut k = c as i64 - 1;
                        let need = if align == HAlign::Center { (gw - avail) / 2.0 } else { gw - avail };
                        while cell_rect.left() - left < need && k >= 0 && empty(k as u32) {
                            left -= (sh.col_px(k as u32) * z).round();
                            k -= 1;
                        }
                    }
                    text_clip = R::from_min_max(pos2(left, cell_rect.top()), pos2(right, cell_rect.bottom()));
                    if text_clip.width() > cell_rect.width() {
                        // Hide gridlines under the overflowing text.
                        let fill = st.fill.map(theme::rgb).unwrap_or(WHITE);
                        let under = R::from_min_max(pos2(text_clip.left(), text_clip.top()), pos2(text_clip.right() - 1.0, text_clip.bottom() - 1.0));
                        let tp = pp.with_clip_rect(clip.intersect(under));
                        if st.fill.is_none() {
                            tp.rect_filled(under, 0.0, fill);
                        }
                    }
                }
                let tx = match align {
                    HAlign::Right => cell_rect.right() - pad - gw - indent,
                    HAlign::Center | HAlign::CenterAcross => cell_rect.center().x - gw / 2.0,
                    _ => cell_rect.left() + pad + indent,
                };
                let ty = match st.v {
                    VAlign::Top => cell_rect.top() + 1.0,
                    VAlign::Center => cell_rect.center().y - gh / 2.0,
                    VAlign::Bottom => cell_rect.bottom() - gh - (1.5 * z).max(1.0),
                };
                let tp = pp.with_clip_rect(clip.intersect(text_clip.shrink2(vec2(0.0, 0.0))));
                if st.rotation != 0 {
                    let ang = if st.rotation == 255 { 90.0 } else { -(st.rotation as f32) };
                    let rad = ang.to_radians();
                    let c0 = cell_rect.center();
                    let off = vec2(-gw / 2.0, -gh / 2.0);
                    let rot = egui::emath::Rot2::from_angle(rad);
                    let start = c0 + rot * off;
                    tp.add(egui::epaint::TextShape::new(start, galley.clone(), color).with_angle(rad));
                } else {
                    if !rendered.left.is_empty() {
                        tp.text(pos2(cell_rect.left() + pad, ty), Align2::LEFT_TOP, &rendered.left, font.clone(), color);
                    }
                    tp.galley(pos2(tx, ty), galley.clone(), color);
                    if st.underline > 0 || link && st.color.is_none() {
                        let uy = ty + gh - 2.0 * z.max(0.8);
                        tp.hline(tx..=tx + gw, uy, Stroke::new(1.0, color));
                        if st.underline == 2 {
                            tp.hline(tx..=tx + gw, uy + 2.0, Stroke::new(1.0, color));
                        }
                    }
                    if st.strike {
                        tp.hline(tx..=tx + gw, ty + gh * 0.55, Stroke::new(1.0, color));
                    }
                }
                if let Some((set, k)) = hit.and_then(|h| h.icon) {
                    crate::cond::paint_icon(&tp, pos2(cell_rect.left() + 3.0 + 6.0 * z, cell_rect.center().y), set, k, z);
                }
            }
        }
        // Sparklines.
        for (pos, sp) in &sh.sparks {
            if rows.iter().any(|e| e.0 == pos.row) && cols.iter().any(|e| e.0 == pos.col) {
                let cr = g.rect_of(a, Rect::cell(*pos)).shrink(3.0);
                crate::charts::sparkline(&pp, cr, sp, &a.wb, &a.calc, si);
            }
        }
        // Notes markers and filter buttons.
        for (pos, _) in &sh.notes {
            if rows.iter().any(|e| e.0 == pos.row) && cols.iter().any(|e| e.0 == pos.col) {
                let cr = g.rect_of(a, sh.merge_at(*pos).unwrap_or(Rect::cell(*pos)));
                let s = 6.0 * z.max(0.7);
                pp.add(egui::Shape::convex_polygon(vec![cr.right_top() + vec2(-s - 1.0, 0.0), cr.right_top() + vec2(-1.0, 0.0), cr.right_top() + vec2(-1.0, s)], Color32::from_rgb(0xC0, 0x00, 0x00), Stroke::NONE));
            }
        }
        for (r, c) in filter_buttons(a) {
            if rows.iter().any(|e| e.0 == r) && cols.iter().any(|e| e.0 == c) {
                let b = filter_button_rect(g, a, r, c);
                let active = a.sheet().filter.as_ref().is_some_and(|f| f.cols.get(&(c - f.range.c1)).is_some_and(|x| x.allowed.is_some() || x.custom.is_some()));
                pp.rect(b, 2.0, Color32::from_rgb(0xF4, 0xF4, 0xF4), Stroke::new(1.0, Color32::from_rgb(0xA6, 0xA6, 0xA6)), StrokeKind::Inside);
                if active {
                    crate::icons::draw(&pp, b.shrink(2.0), crate::icons::Icon::Filter, TEXT);
                } else {
                    crate::icons::draw(&pp, b.shrink(1.0), crate::icons::Icon::Down, TEXT);
                }
            }
        }
        // Page breaks.
        if a.view_mode != ViewMode::Normal {
            let (rb, cb) = crate::print::breaks(&a.wb, &a.calc, si);
            for r in rb {
                let y = g.y(a, r) - 0.5;
                pp.extend(egui::Shape::dashed_line(&[pos2(clip.left(), y), pos2(clip.right(), y)], Stroke::new(1.5, BLUE), 6.0, 4.0));
            }
            for c in cb {
                let x = g.x(a, c) - 0.5;
                pp.extend(egui::Shape::dashed_line(&[pos2(x, clip.top()), pos2(x, clip.bottom())], Stroke::new(1.5, BLUE), 6.0, 4.0));
            }
        }
        paint_selection(a, &pp, g);
    }
    paint_headers(a, &p, g);
    // Freeze lines.
    if g.fr > 0 {
        p.hline(g.cells.left()..=g.cells.right(), g.sy0 - 0.5, Stroke::new(1.0, Color32::from_rgb(0x9B, 0x9B, 0x9B)));
    }
    if g.fc > 0 {
        p.vline(g.sx0 - 0.5, g.cells.top()..=g.cells.bottom(), Stroke::new(1.0, Color32::from_rgb(0x9B, 0x9B, 0x9B)));
    }
    // Charts float over the scrolling pane.
    let ox = sheet_offset_x(a, g);
    let oy = sheet_offset_y(a, g);
    let cp = p.with_clip_rect(g.cells);
    for (i, ch) in sh.charts.iter().enumerate() {
        let r = R::from_min_size(pos2(ox + ch.x * z, oy + ch.y * z), vec2(ch.w * z, ch.h * z));
        if !r.intersects(g.cells) {
            continue;
        }
        crate::charts::draw(&cp, r, ch, &a.wb, &a.calc, si, z);
        if a.chart_sel == Some(i) {
            cp.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(0x8A, 0x8A, 0x8A)), StrokeKind::Outside);
            for h in chart_handles(r) {
                cp.circle(h, 4.0, WHITE, Stroke::new(1.0, Color32::from_rgb(0x70, 0x70, 0x70)));
            }
        }
    }
}

pub fn chart_handles(r: R) -> [egui::Pos2; 8] {
    let c = r.center();
    [r.left_top(), pos2(c.x, r.top()), r.right_top(), pos2(r.right(), c.y), r.right_bottom(), pos2(c.x, r.bottom()), r.left_bottom(), pos2(r.left(), c.y)]
}

/// Screen x of the sheet's left edge (column A), as charts are anchored to it.
pub fn sheet_offset_x(a: &App, g: &Geo) -> f32 {
    let sh = a.sheet();
    let hidden: f32 = (g.fc..g.left).map(|c| sh.col_px(c)).sum::<f32>() * g.z;
    g.cells.left() - hidden
}

pub fn sheet_offset_y(a: &App, g: &Geo) -> f32 {
    let sh = a.sheet();
    let n = g.top - g.fr;
    let hidden: f32 = if n > 20_000 {
        (g.fr..g.top).filter(|r| sh.row_h.contains_key(r) || sh.row_hidden(*r)).map(|r| sh.row_px(r) - DEFAULT_ROW_H).sum::<f32>() + n as f32 * DEFAULT_ROW_H
    } else {
        (g.fr..g.top).map(|r| sh.row_px(r)).sum::<f32>()
    } * g.z;
    g.cells.top() - hidden
}

/// Header cells that show a filter dropdown button.
pub fn filter_buttons(a: &App) -> Vec<(u32, u32)> {
    let sh = a.sheet();
    let mut v = vec![];
    if let Some(f) = &sh.filter {
        for c in f.range.c1..=f.range.c2 {
            v.push((f.range.r1, c));
        }
    }
    for t in &sh.tables {
        if t.header && sh.filter.as_ref().is_none_or(|f| f.range != t.range) {
            for c in t.range.c1..=t.range.c2 {
                v.push((t.range.r1, c));
            }
        }
    }
    v
}

pub fn filter_button_rect(g: &Geo, a: &App, r: u32, c: u32) -> R {
    let cr = g.rect_of(a, Rect::cell(Pos::new(r, c)));
    let s = (16.0 * g.z).min(cr.height() - 2.0).max(10.0);
    R::from_min_size(pos2(cr.right() - s - 2.0, cr.bottom() - s - 2.0), vec2(s, s))
}

fn paint_selection(a: &App, p: &Painter, g: &Geo) {
    let s = a.sel();
    // Reference colours while typing a formula.
    if let Some(e) = a.edit.as_ref().filter(|e| e.text.starts_with('=')) {
        for (i, (_, _, t)) in crate::formula::find_refs(&e.text).iter().enumerate() {
            let (sheet, rest) = match t.rsplit_once('!') {
                Some((sn, r)) => (a.wb.sheet_index(sn.trim_matches('\'')), r),
                None => (Some(e.sheet), t.as_str()),
            };
            if sheet != Some(a.cur) {
                continue;
            }
            if let Some(rc) = parse_rect(rest) {
                let col = REF_COLORS[i % REF_COLORS.len()];
                let r = g.rect_of(a, rc);
                p.rect_filled(r, 0.0, col.gamma_multiply(0.12));
                p.rect_stroke(r, 0.0, Stroke::new(1.5, col), StrokeKind::Inside);
                let corner = r.right_bottom();
                p.rect_filled(R::from_center_size(corner - vec2(2.0, 2.0), vec2(5.0, 5.0)), 0.0, col);
            }
        }
    }
    let editing_formula = a.edit.as_ref().is_some_and(|e| e.text.starts_with('=') && e.sheet != a.cur);
    for (i, rc) in s.ranges.iter().enumerate() {
        let r = g.rect_of(a, *rc);
        let single = rc.is_single() || a.sheet().merge_at(Pos::new(rc.r1, rc.c1)) == Some(*rc);
        if !single {
            p.rect_filled(r, 0.0, SEL_FILL);
            if i == s.ranges.len() - 1 {
                // The active cell stays clear.
                let ar = g.rect_of(a, a.sheet().merge_at(s.active).unwrap_or(Rect::cell(s.active)));
                p.rect_filled(ar.shrink(1.0), 0.0, a.sheet().get(s.active.row, s.active.col).and_then(|c| a.wb.styles.get(c.style).fill).map(theme::rgb).unwrap_or(WHITE));
            }
        }
        if i == s.ranges.len() - 1 && !editing_formula {
            p.rect_stroke(r.expand(0.5), 0.0, Stroke::new(2.0, ACCENT), StrokeKind::Middle);
            if s.ranges.len() == 1 && a.edit.is_none() {
                let h = R::from_center_size(r.right_bottom(), vec2(6.0, 6.0));
                p.rect_filled(h.expand(1.0), 0.0, WHITE);
                p.rect_filled(h, 0.0, ACCENT);
            }
        }
    }
    if let Drag::Fill(src) = a.drag {
        let _ = src;
    }
    if let Some(c) = a.clip.as_ref().filter(|c| a.marquee && c.sheet == a.cur) {
        let r = g.rect_of(a, c.src);
        let phase = (p.ctx().input(|i| i.time) * 12.0) as f32 % 7.0;
        dash_rect(p, r.shrink(0.5), ACCENT, 1.5, phase);
        p.ctx().request_repaint_after(std::time::Duration::from_millis(80));
    }
    // Spill outline for the active cell.
    let key = (a.cur as u32, s.active.row, s.active.col);
    let spill = a.calc.spill_rects.get(&key).copied().or_else(|| a.calc.spill_rects.iter().find(|(k, r)| k.0 == a.cur as u32 && r.contains(s.active)).map(|(_, r)| *r));
    if let Some(sr) = spill.filter(|r| !r.is_single()) {
        let r = g.rect_of(a, sr);
        p.extend(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(1.0, Color32::from_rgb(0x41, 0x72, 0xC4)), 3.0, 2.0));
    }
    // Fill handle drag preview and move preview.
    match a.drag {
        Drag::Fill(t) | Drag::Move(t, _) if t != s.rect() => {
            let r = g.rect_of(a, t);
            p.extend(egui::Shape::dashed_line(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], Stroke::new(2.0, Color32::from_rgb(0x70, 0x70, 0x70)), 3.0, 3.0));
        }
        _ => {}
    }
}

fn paint_headers(a: &App, p: &Painter, g: &Geo) {
    if g.hw <= 0.0 {
        return;
    }
    let s = a.sel();
    let z = g.z;
    let font = theme::ui_font((11.0 * z).clamp(7.0, 22.0));
    let ch = R::from_min_max(pos2(g.rect.left() + g.hw, g.rect.top()), pos2(g.rect.right(), g.rect.top() + g.hh));
    let rh = R::from_min_max(pos2(g.rect.left(), g.rect.top() + g.hh), pos2(g.rect.left() + g.hw, g.rect.bottom()));
    p.rect_filled(ch, 0.0, HEADER_BG);
    p.rect_filled(rh, 0.0, HEADER_BG);
    let col_sel = |c: u32| s.ranges.iter().any(|r| r.c1 <= c && c <= r.c2);
    let col_full = |c: u32| s.ranges.iter().any(|r| r.c1 <= c && c <= r.c2 && r.whole_cols());
    let row_sel = |r: u32| s.ranges.iter().any(|x| x.r1 <= r && r <= x.r2);
    let row_full = |r: u32| s.ranges.iter().any(|x| x.r1 <= r && r <= x.r2 && x.whole_rows());
    let cp = p.with_clip_rect(ch);
    for (clip_x, list) in [(g.cells.left()..g.sx0, true), (g.sx0..g.cells.right(), false)] {
        let pp = cp.with_clip_rect(R::from_min_max(pos2(clip_x.start, ch.top()), pos2(clip_x.end, ch.bottom())));
        for (c, x, w) in g.cols.iter().filter(|e| (e.0 < g.fc) == list) {
            if *w <= 0.0 {
                continue;
            }
            let r = R::from_min_size(pos2(*x, ch.top()), vec2(*w, g.hh));
            let (sel, full) = (col_sel(*c), col_full(*c));
            if sel {
                pp.rect_filled(r, 0.0, if full { Color32::from_rgb(0xC6, 0xE0, 0xCF) } else { HEADER_SEL });
                pp.hline(r.left()..=r.right(), r.bottom() - 1.0, Stroke::new(2.0, ACCENT));
            }
            pp.vline(r.right() - 0.5, r.top()..=r.bottom(), Stroke::new(1.0, HEADER_LINE));
            pp.text(r.center(), Align2::CENTER_CENTER, col_name(*c), font.clone(), if sel { GREEN_DARK } else { HEADER_TEXT });
        }
    }
    let rp = p.with_clip_rect(rh);
    for (clip_y, list) in [(g.cells.top()..g.sy0, true), (g.sy0..g.cells.bottom(), false)] {
        let pp = rp.with_clip_rect(R::from_min_max(pos2(rh.left(), clip_y.start), pos2(rh.right(), clip_y.end)));
        for (r, y, h) in g.rows.iter().filter(|e| (e.0 < g.fr) == list) {
            if *h <= 0.0 {
                continue;
            }
            let b = R::from_min_size(pos2(rh.left(), *y), vec2(g.hw, *h));
            let (sel, full) = (row_sel(*r), row_full(*r));
            if sel {
                pp.rect_filled(b, 0.0, if full { Color32::from_rgb(0xC6, 0xE0, 0xCF) } else { HEADER_SEL });
                pp.vline(b.right() - 1.0, b.top()..=b.bottom(), Stroke::new(2.0, ACCENT));
            }
            pp.hline(b.left()..=b.right(), b.bottom() - 0.5, Stroke::new(1.0, HEADER_LINE));
            pp.text(pos2(b.right() - 5.0, b.center().y), Align2::RIGHT_CENTER, (r + 1).to_string(), font.clone(), if sel { GREEN_DARK } else { HEADER_TEXT });
        }
    }
    // Hidden rows and columns show a double line in the headers.
    let sh = a.sheet();
    for (c, x, _) in &g.cols {
        if *c > 0 && sh.hidden_cols.contains(&(c - 1)) {
            cp.vline(x - 2.0, ch.top()..=ch.bottom(), Stroke::new(1.0, HEADER_TEXT));
        }
    }
    for (r, y, _) in &g.rows {
        if *r > 0 && sh.row_hidden(r - 1) {
            rp.hline(rh.left()..=rh.right(), y - 2.0, Stroke::new(1.0, HEADER_TEXT));
        }
    }
    let corner = R::from_min_size(g.rect.min, vec2(g.hw, g.hh));
    p.rect_filled(corner, 0.0, HEADER_BG);
    let all = s.ranges.iter().any(|r| r.whole_cols() && r.whole_rows());
    p.add(egui::Shape::convex_polygon(
        vec![corner.right_bottom() + vec2(-4.0, -4.0), corner.right_bottom() + vec2(-4.0, -4.0 - 10.0 * z.min(1.2)), corner.right_bottom() + vec2(-4.0 - 10.0 * z.min(1.2), -4.0)],
        if all { ACCENT } else { Color32::from_rgb(0xB4, 0xB4, 0xB4) },
        Stroke::NONE,
    ));
    p.hline(g.rect.left()..=g.rect.right(), g.rect.top() + g.hh - 0.5, Stroke::new(1.0, HEADER_LINE));
    p.vline(g.rect.left() + g.hw - 0.5, g.rect.top()..=g.rect.bottom(), Stroke::new(1.0, HEADER_LINE));
}

/// Text box contents of the editor with references coloured, for both editors.
pub fn formula_job(text: &str, font: egui::FontId, wrap: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    let base = egui::TextFormat { font_id: font.clone(), color: Color32::BLACK, ..Default::default() };
    if !text.starts_with('=') {
        job.append(text, 0.0, base);
        return job;
    }
    let refs = crate::formula::find_refs(text);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let byte = |ci: usize| chars.get(ci).map_or(text.len(), |x| x.0);
    let mut at = 0usize;
    for (i, (s, e, _)) in refs.iter().enumerate() {
        if *s > at {
            job.append(&text[byte(at)..byte(*s)], 0.0, base.clone());
        }
        let col = REF_COLORS[i % REF_COLORS.len()];
        job.append(&text[byte(*s)..byte(*e)], 0.0, egui::TextFormat { color: col, ..base.clone() });
        at = *e;
    }
    if at < chars.len() {
        job.append(&text[byte(at)..], 0.0, base);
    }
    job
}

pub fn style_font(st: &Arc<Style>, z: f32) -> egui::FontId {
    egui::FontId::new(theme::cell_px(st.size10, z), theme::cell_family(&st.font, st.bold, st.italic))
}

pub fn edit_mode_label(a: &App) -> &'static str {
    match &a.edit {
        None => "Ready",
        Some(e) => match e.mode {
            EditMode::Enter => "Enter",
            EditMode::Edit => "Edit",
            EditMode::Point => "Point",
        },
    }
}

pub use crate::grid_input::{accept_completion, toggle_abs_at_caret};
