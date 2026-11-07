//! Mouse, scrolling and the in-cell editor for the grid.

use crate::app::{App, Drag, EditMode, Menu, MenuKind};
use crate::grid::{self, editor_id, Geo};
use crate::model::*;
use crate::ops;
use crate::theme::{self, *};
use egui::{pos2, vec2, Color32, CursorIcon, Rect as R, Sense, Stroke, StrokeKind};
use std::cell::Cell as StdCell;

thread_local! {
    static SCROLL: StdCell<(f32, f32)> = const { StdCell::new((0.0, 0.0)) };
    static LAST_GEO: StdCell<Option<R>> = const { StdCell::new(None) };
}

const SCROLLBAR: f32 = 16.0;

/// Largest row and column the scrollbars cover.
pub fn extent(a: &App) -> (u32, u32) {
    let u = a.sheet().used_rect().unwrap_or(Rect::new(0, 0, 0, 0));
    let (top, left) = a.view();
    let (vr, vc) = grid::visible_counts(a);
    ((u.r2 + vr + 1).max(top + vr + 1).max(a.sel().active.row + 1), (u.c2 + vc + 1).max(left + vc + 1).max(a.sel().active.col + 1))
}

pub fn show(a: &mut App, ui: &mut egui::Ui, area: R) {
    let grid_rect = R::from_min_max(area.min, pos2(area.right() - SCROLLBAR, area.bottom()));
    let g = Geo::new(a, grid_rect);
    LAST_GEO.with(|x| x.set(Some(grid_rect)));
    let resp = ui.interact(grid_rect, egui::Id::new("grid"), Sense::click_and_drag());
    grid::paint(a, ui, &g);
    vscroll(a, ui, R::from_min_max(pos2(grid_rect.right(), area.top()), area.max), &g);
    let blocked = a.dialog.as_ref().is_some_and(|d| d.modal()) || a.page.is_some();
    if !blocked {
        mouse(a, ui, &g, &resp);
        wheel(a, ui, &resp);
    }
    editor(a, ui, &g);
    validation_button(a, ui, &g);
    hover_note(a, ui, &g, &resp);
}

fn vscroll(a: &mut App, ui: &mut egui::Ui, r: R, g: &Geo) {
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::from_rgb(0xF0, 0xF0, 0xF0));
    p.vline(r.left() + 0.5, r.top()..=r.bottom(), Stroke::new(1.0, HEADER_LINE));
    let up = R::from_min_size(r.min, vec2(r.width(), SCROLLBAR));
    let down = R::from_min_size(pos2(r.left(), r.bottom() - SCROLLBAR), vec2(r.width(), SCROLLBAR));
    crate::icons::draw(p, up.shrink(3.0), crate::icons::Icon::Up, TEXT_DIM);
    crate::icons::draw(p, down.shrink(3.0), crate::icons::Icon::Down, TEXT_DIM);
    let track = R::from_min_max(pos2(r.left(), up.bottom()), pos2(r.right(), down.top()));
    let (max_r, _) = extent(a);
    let (top, left) = a.view();
    let (vr, _) = grid::visible_counts(a);
    let fr = g.fr;
    let span = (max_r - fr).max(1) as f32;
    let h = (track.height() * vr as f32 / span).clamp(20.0, track.height());
    let y = track.top() + (track.height() - h) * ((top - fr) as f32 / (span - vr as f32).max(1.0)).clamp(0.0, 1.0);
    let thumb = R::from_min_size(pos2(r.left() + 3.0, y), vec2(r.width() - 6.0, h));
    let id = egui::Id::new("vscroll");
    let resp = ui.interact(track, id, Sense::click_and_drag());
    let col = if resp.dragged() { Color32::from_rgb(0x80, 0x80, 0x80) } else if resp.hovered() { Color32::from_rgb(0xA0, 0xA0, 0xA0) } else { Color32::from_rgb(0xC2, 0xC2, 0xC2) };
    ui.painter().rect_filled(thumb, 4.0, col);
    if resp.drag_started() {
        if let Some(pp) = resp.interact_pointer_pos() {
            if thumb.contains(pp) {
                a.drag = Drag::VScroll(pp.y, top);
            }
        }
    }
    if let (Drag::VScroll(y0, t0), Some(pp)) = (a.drag, resp.interact_pointer_pos()) {
        let rows_per_px = (span - vr as f32).max(1.0) / (track.height() - h).max(1.0);
        let t = (t0 as f32 + (pp.y - y0) * rows_per_px).max(fr as f32) as u32;
        a.views[a.cur] = (t.min(MAX_ROWS - 1), left);
    }
    if resp.drag_stopped() {
        a.drag = Drag::None;
    }
    if resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            let page = vr.max(1);
            if pp.y < thumb.top() {
                a.views[a.cur] = (top.saturating_sub(page).max(fr), left);
            } else if pp.y > thumb.bottom() {
                a.views[a.cur] = ((top + page).min(MAX_ROWS - 1), left);
            }
        }
    }
    for (b, d) in [(up, -1i64), (down, 1)] {
        let br = ui.interact(b, id.with(d), Sense::click());
        if br.clicked() || br.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().is_some_and(|t| i.time - t > 0.4)) {
            a.views[a.cur] = ((top as i64 + d).clamp(fr as i64, MAX_ROWS as i64 - 1) as u32, left);
            ui.ctx().request_repaint();
        }
    }
}

/// Horizontal scrollbar, drawn by the sheet tab bar.
pub fn hscroll(a: &mut App, ui: &mut egui::Ui, r: R) {
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::from_rgb(0xF0, 0xF0, 0xF0));
    let lb = R::from_min_size(r.min, vec2(SCROLLBAR, r.height()));
    let rb = R::from_min_size(pos2(r.right() - SCROLLBAR, r.top()), vec2(SCROLLBAR, r.height()));
    crate::icons::draw(p, lb.shrink(3.0), crate::icons::Icon::Left, TEXT_DIM);
    crate::icons::draw(p, rb.shrink(3.0), crate::icons::Icon::Right, TEXT_DIM);
    let track = R::from_min_max(pos2(lb.right(), r.top()), pos2(rb.left(), r.bottom()));
    let (_, max_c) = extent(a);
    let (top, left) = a.view();
    let (_, vc) = grid::visible_counts(a);
    let fc = a.sheet().freeze.1;
    let span = (max_c - fc).max(1) as f32;
    let w = (track.width() * vc as f32 / span).clamp(20.0, track.width());
    let x = track.left() + (track.width() - w) * ((left - fc) as f32 / (span - vc as f32).max(1.0)).clamp(0.0, 1.0);
    let thumb = R::from_min_size(pos2(x, r.top() + 3.0), vec2(w, r.height() - 6.0));
    let id = egui::Id::new("hscroll");
    let resp = ui.interact(track, id, Sense::click_and_drag());
    let col = if resp.dragged() { Color32::from_rgb(0x80, 0x80, 0x80) } else if resp.hovered() { Color32::from_rgb(0xA0, 0xA0, 0xA0) } else { Color32::from_rgb(0xC2, 0xC2, 0xC2) };
    ui.painter().rect_filled(thumb, 4.0, col);
    if resp.drag_started() {
        if let Some(pp) = resp.interact_pointer_pos() {
            if thumb.contains(pp) {
                a.drag = Drag::HScroll(pp.x, left);
            }
        }
    }
    if let (Drag::HScroll(x0, l0), Some(pp)) = (a.drag, resp.interact_pointer_pos()) {
        let per = (span - vc as f32).max(1.0) / (track.width() - w).max(1.0);
        let l = (l0 as f32 + (pp.x - x0) * per).max(fc as f32) as u32;
        a.views[a.cur] = (top, l.min(MAX_COLS - 1));
    }
    if resp.drag_stopped() {
        a.drag = Drag::None;
    }
    if resp.clicked() {
        if let Some(pp) = resp.interact_pointer_pos() {
            if pp.x < thumb.left() {
                a.views[a.cur] = (top, left.saturating_sub(vc.max(1)).max(fc));
            } else if pp.x > thumb.right() {
                a.views[a.cur] = (top, (left + vc.max(1)).min(MAX_COLS - 1));
            }
        }
    }
    for (b, d) in [(lb, -1i64), (rb, 1)] {
        if ui.interact(b, id.with(d), Sense::click()).clicked() {
            a.views[a.cur] = (top, (left as i64 + d).clamp(fc as i64, MAX_COLS as i64 - 1) as u32);
        }
    }
}

fn wheel(a: &mut App, ui: &mut egui::Ui, resp: &egui::Response) {
    if !resp.hovered() && !resp.dragged() {
        return;
    }
    let (zoom, delta) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
    if zoom != 1.0 {
        let z = (a.sheet().zoom * zoom).clamp(0.1, 4.0);
        a.wb.sheet_mut(a.cur).zoom = (z * 100.0).round() / 100.0;
        return;
    }
    if delta == egui::Vec2::ZERO {
        return;
    }
    let step = DEFAULT_ROW_H * a.sheet().zoom;
    let (mut ay, mut ax) = SCROLL.with(|s| s.get());
    ay += delta.y;
    ax += delta.x;
    let rows = (ay / step).trunc() as i64;
    let cols = (ax / (DEFAULT_COL_W * a.sheet().zoom * 0.7)).trunc() as i64;
    ay -= rows as f32 * step;
    ax -= cols as f32 * DEFAULT_COL_W * a.sheet().zoom * 0.7;
    SCROLL.with(|s| s.set((ay, ax)));
    let (fr, fc) = a.sheet().freeze;
    let (top, left) = a.view();
    let nt = (top as i64 - rows).clamp(fr as i64, MAX_ROWS as i64 - 1) as u32;
    let nl = (left as i64 - cols).clamp(fc as i64, MAX_COLS as i64 - 1) as u32;
    a.views[a.cur] = (nt, nl);
}

fn chart_rect(a: &App, g: &Geo, i: usize) -> R {
    let ch = &a.sheet().charts[i];
    let (ox, oy) = (grid::sheet_offset_x(a, g), grid::sheet_offset_y(a, g));
    R::from_min_size(pos2(ox + ch.x * g.z, oy + ch.y * g.z), vec2(ch.w * g.z, ch.h * g.z))
}

#[derive(PartialEq)]
enum Zone {
    Corner,
    ColHead(u32, Option<u32>),
    RowHead(u32, Option<u32>),
    Cell(Pos),
}

fn zone(a: &App, g: &Geo, p: egui::Pos2) -> Zone {
    let in_ch = p.y < g.rect.top() + g.hh;
    let in_rh = p.x < g.rect.left() + g.hw;
    if in_ch && in_rh {
        return Zone::Corner;
    }
    if in_ch {
        let border = g.cols.iter().rev().find(|(_, x, w)| (x + w - p.x).abs() <= 4.0).map(|e| e.0);
        return Zone::ColHead(g.hit(a, p).col, border);
    }
    if in_rh {
        let border = g.rows.iter().rev().find(|(_, y, h)| (y + h - p.y).abs() <= 3.0).map(|e| e.0);
        return Zone::RowHead(g.hit(a, p).row, border);
    }
    Zone::Cell(g.hit(a, p))
}

fn mouse(a: &mut App, ui: &mut egui::Ui, g: &Geo, resp: &egui::Response) {
    let ctx = ui.ctx().clone();
    let (pressed, down, released, ptr, mods) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.interact_pos(), i.modifiers));
    let hover = resp.hover_pos();
    // Cursor shapes.
    if let Some(h) = hover {
        let icon = match zone(a, g, h) {
            Zone::ColHead(_, Some(_)) => CursorIcon::ResizeColumn,
            Zone::RowHead(_, Some(_)) => CursorIcon::ResizeRow,
            Zone::ColHead(..) => CursorIcon::PointingHand,
            Zone::RowHead(..) => CursorIcon::PointingHand,
            Zone::Corner => CursorIcon::Default,
            Zone::Cell(_) => {
                let sr = g.rect_of(a, a.sel().rect());
                if a.edit.is_none() && R::from_center_size(sr.right_bottom(), vec2(10.0, 10.0)).contains(h) {
                    CursorIcon::Crosshair
                } else if a.edit.is_none() && on_edge(sr, h) {
                    CursorIcon::Move
                } else if (0..a.sheet().charts.len()).any(|i| chart_rect(a, g, i).contains(h)) {
                    CursorIcon::Move
                } else {
                    CursorIcon::Cell
                }
            }
        };
        ctx.set_cursor_icon(icon);
    }
    match a.drag {
        Drag::ColResize(..) => ctx.set_cursor_icon(CursorIcon::ResizeColumn),
        Drag::RowResize(..) => ctx.set_cursor_icon(CursorIcon::ResizeRow),
        _ => {}
    }
    if pressed && hover.is_some() && a.menu.is_none() {
        press(a, g, hover.unwrap(), mods, &ctx);
    }
    if down {
        if let Some(p) = ptr {
            drag(a, g, p, mods, &ctx);
        }
    }
    if released && a.drag != Drag::None {
        release(a, ptr.unwrap_or_default(), mods, g);
    }
    if resp.double_clicked() {
        if let Some(h) = hover {
            double(a, g, h);
        }
    }
    if resp.secondary_clicked() {
        if let Some(h) = hover {
            let z = zone(a, g, h);
            let kind = match z {
                Zone::ColHead(c, _) => {
                    if !a.sel().ranges.iter().any(|r| r.whole_cols() && r.c1 <= c && c <= r.c2) {
                        a.sel_mut().ranges = vec![Rect::new(0, c, MAX_ROWS - 1, c)];
                        a.sel_mut().active = Pos::new(a.view().0, c);
                    }
                    MenuKind::ColCtx
                }
                Zone::RowHead(r, _) => {
                    if !a.sel().ranges.iter().any(|x| x.whole_rows() && x.r1 <= r && r <= x.r2) {
                        a.sel_mut().ranges = vec![Rect::new(r, 0, r, MAX_COLS - 1)];
                        a.sel_mut().active = Pos::new(r, a.view().1);
                    }
                    MenuKind::RowCtx
                }
                Zone::Cell(p) => {
                    if let Some(i) = (0..a.sheet().charts.len()).rev().find(|i| chart_rect(a, g, *i).contains(h)) {
                        a.chart_sel = Some(i);
                        MenuKind::ChartCtx
                    } else {
                        if a.edit.is_some() {
                            a.commit_edit(false);
                        }
                        if !a.sel().ranges.iter().any(|r| r.contains(p)) {
                            a.select(p, false);
                        }
                        MenuKind::CellCtx
                    }
                }
                Zone::Corner => MenuKind::CellCtx,
            };
            a.menu = Some(Menu { kind, pos: h, born: a.frame });
        }
    }
}

fn on_edge(r: R, p: egui::Pos2) -> bool {
    let outer = r.expand(3.0);
    let inner = r.shrink(3.0);
    outer.contains(p) && !inner.contains(p)
}

fn press(a: &mut App, g: &Geo, h: egui::Pos2, mods: egui::Modifiers, ctx: &egui::Context) {
    let z = zone(a, g, h);
    // Pointing at cells while typing a formula.
    if let (Some(_), Zone::Cell(p)) = (&a.edit, &z) {
        let pointing = a.wants_ref() || a.edit.as_ref().is_some_and(|e| e.mode == EditMode::Point && e.point.is_some_and(|x| x.1 == e.caret));
        if pointing {
            let fresh = a.edit.as_ref().is_some_and(|e| e.point.is_none_or(|x| x.1 != e.caret));
            if fresh || !mods.shift {
                if fresh {
                    a.edit.as_mut().unwrap().point = None;
                }
                let e = a.edit.as_mut().unwrap();
                e.point_anchor = *p;
            }
            let anchor = a.edit.as_ref().unwrap().point_anchor;
            let rect = a.grow_merges(Rect::from_pos(anchor, *p));
            let s = a.cur;
            a.point_at(rect, s);
            a.drag = Drag::Point;
            return;
        }
        let editing_here = a.edit.as_ref().is_some_and(|e| e.pos == *p && e.sheet == a.cur);
        if editing_here {
            return;
        }
        if !a.commit_edit(false) {
            return;
        }
    } else if a.edit.is_some() {
        if !a.commit_edit(false) {
            return;
        }
    }
    a.menu = None;
    // Charts sit on top of the cells.
    if let Zone::Cell(_) = z {
        if let Some(i) = a.chart_sel {
            if i < a.sheet().charts.len() {
                let r = chart_rect(a, g, i);
                for (k, hp) in grid::chart_handles(r).iter().enumerate() {
                    if hp.distance(h) <= 6.0 {
                        let c = &a.sheet().charts[i];
                        let orig = [c.x, c.y, c.w, c.h];
                        a.snapshot();
                        a.drag = Drag::Chart(i, k as u8 + 1, h, orig);
                        return;
                    }
                }
            }
        }
        if let Some(i) = (0..a.sheet().charts.len()).rev().find(|i| chart_rect(a, g, *i).contains(h)) {
            a.chart_sel = Some(i);
            a.tab = crate::app::RTab::ChartDesign;
            let c = &a.sheet().charts[i];
            let orig = [c.x, c.y, c.w, c.h];
            a.snapshot();
            a.drag = Drag::Chart(i, 0, h, orig);
            return;
        }
    }
    if a.chart_sel.take().is_some() && a.tab == crate::app::RTab::ChartDesign {
        a.tab = crate::app::RTab::Home;
    }
    match z {
        Zone::Corner => {
            a.sel_mut().ranges = vec![Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1)];
        }
        Zone::ColHead(c, border) => {
            if let Some(b) = border {
                a.snapshot();
                a.drag = Drag::ColResize(b, h.x, a.sheet().col_px(b));
                return;
            }
            let anchor = if mods.shift { a.sel().anchor.col } else { c };
            let r = Rect::new(0, anchor.min(c), MAX_ROWS - 1, anchor.max(c));
            if mods.command {
                a.sel_mut().ranges.push(r);
            } else {
                let top = a.view().0;
                *a.sel_mut() = crate::app::Sel { ranges: vec![r], active: Pos::new(top, c), anchor: Pos::new(top, anchor) };
            }
            a.drag = Drag::Cols(anchor);
        }
        Zone::RowHead(r, border) => {
            if let Some(b) = border {
                a.snapshot();
                a.drag = Drag::RowResize(b, h.y, a.sheet().row_px(b));
                return;
            }
            let anchor = if mods.shift { a.sel().anchor.row } else { r };
            let x = Rect::new(anchor.min(r), 0, anchor.max(r), MAX_COLS - 1);
            if mods.command {
                a.sel_mut().ranges.push(x);
            } else {
                let left = a.view().1;
                *a.sel_mut() = crate::app::Sel { ranges: vec![x], active: Pos::new(r, left), anchor: Pos::new(anchor, left) };
            }
            a.drag = Drag::Rows(anchor);
        }
        Zone::Cell(p) => {
            let sr = a.sel().rect();
            let srr = g.rect_of(a, sr);
            if R::from_center_size(srr.right_bottom(), vec2(10.0, 10.0)).contains(h) && a.sel().ranges.len() == 1 {
                a.drag = Drag::Fill(sr);
                return;
            }
            if on_edge(srr, h) && a.sel().ranges.len() == 1 {
                let grab = Pos::new(p.row.saturating_sub(sr.r1).min(sr.rows() - 1), p.col.saturating_sub(sr.c1).min(sr.cols() - 1));
                a.drag = Drag::Move(sr, grab);
                return;
            }
            for (r, c) in grid::filter_buttons(a) {
                if grid::filter_button_rect(g, a, r, c).contains(h) {
                    crate::dialogs::open_filter(a, r, c);
                    a.menu = Some(Menu { kind: MenuKind::FilterCol(c), pos: grid::filter_button_rect(g, a, r, c).left_bottom(), born: a.frame });
                    return;
                }
            }
            if let Some((fmt, sticky)) = a.painter_fmt.clone() {
                let _ = sticky;
                a.select(p, false);
                a.drag = Drag::Select;
                let _ = fmt;
                return;
            }
            if mods.command {
                let s = a.sel_mut();
                s.ranges.push(Rect::cell(p));
                s.active = p;
                s.anchor = p;
            } else {
                a.select(p, mods.shift);
            }
            a.drag = Drag::Select;
            // Links open on a plain click.
            if !mods.shift && !mods.command {
                if let Some(url) = a.sheet().links.get(&p).cloned() {
                    follow_link(a, ctx, &url);
                }
            }
        }
    }
}

pub fn follow_link(a: &mut App, ctx: &egui::Context, url: &str) {
    if let Some(inner) = url.strip_prefix('#') {
        let (sheet, cell) = match inner.rsplit_once('!') {
            Some((s, c)) => (a.wb.sheet_index(s.trim_matches('\'')), c),
            None => (Some(a.cur), inner),
        };
        if let (Some(si), Some(rc)) = (sheet, parse_rect(cell)) {
            a.switch_sheet(si);
            a.sel_mut().ranges = vec![rc];
            a.sel_mut().active = Pos::new(rc.r1, rc.c1);
            a.reveal(Pos::new(rc.r1, rc.c1));
            a.drag = Drag::None;
        }
    } else {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
}

fn drag(a: &mut App, g: &Geo, p: egui::Pos2, _mods: egui::Modifiers, ctx: &egui::Context) {
    // Keep scrolling while dragging past the edge.
    let auto = matches!(a.drag, Drag::Select | Drag::Fill(_) | Drag::Move(..) | Drag::Point | Drag::Cols(_) | Drag::Rows(_));
    if auto {
        let (top, left) = a.view();
        let (fr, fc) = a.sheet().freeze;
        let (mut nt, mut nl) = (top, left);
        if p.y > g.cells.bottom() {
            nt = (top + 1).min(MAX_ROWS - 1);
        } else if p.y < g.sy0 && p.y < g.cells.top() + 2.0 && top > fr {
            nt = top - 1;
        }
        if p.x > g.cells.right() {
            nl = (left + 1).min(MAX_COLS - 1);
        } else if p.x < g.cells.left() + 2.0 && left > fc {
            nl = left - 1;
        }
        if (nt, nl) != (top, left) && ctx.input(|i| i.time * 20.0).fract() < 0.5 {
            a.views[a.cur] = (nt, nl);
        }
        ctx.request_repaint();
    }
    let hit = g.hit(a, p);
    match a.drag {
        Drag::Select => {
            let anchor = a.sel().anchor;
            let rect = a.grow_merges(Rect::from_pos(anchor, hit));
            *a.sel_mut().ranges.last_mut().unwrap() = rect;
        }
        Drag::Cols(anchor) => {
            *a.sel_mut().ranges.last_mut().unwrap() = Rect::new(0, anchor.min(hit.col), MAX_ROWS - 1, anchor.max(hit.col));
        }
        Drag::Rows(anchor) => {
            *a.sel_mut().ranges.last_mut().unwrap() = Rect::new(anchor.min(hit.row), 0, anchor.max(hit.row), MAX_COLS - 1);
        }
        Drag::ColResize(c, x0, w0) => {
            let w = (w0 + (p.x - x0) / g.z).max(0.0).round();
            let cols: Vec<u32> = if a.sel().ranges.iter().any(|r| r.whole_cols() && r.c1 <= c && c <= r.c2) { a.sel().ranges.iter().filter(|r| r.whole_cols()).flat_map(|r| r.c1..=r.c2.min(r.c1 + 500)).collect() } else { vec![c] };
            let sh = a.wb.sheet_mut(a.cur);
            for k in cols {
                if w <= 0.0 {
                    sh.hidden_cols.insert(k);
                } else {
                    sh.hidden_cols.remove(&k);
                    sh.col_w.insert(k, w);
                }
            }
            a.status = format!("Width: {:.2} ({} pixels)", ((w - 5.0).max(0.0) / 7.0), w);
        }
        Drag::RowResize(r, y0, h0) => {
            let h = (h0 + (p.y - y0) / g.z).max(0.0).round();
            let rows: Vec<u32> = if a.sel().ranges.iter().any(|x| x.whole_rows() && x.r1 <= r && r <= x.r2) { a.sel().ranges.iter().filter(|x| x.whole_rows()).flat_map(|x| x.r1..=x.r2.min(x.r1 + 2000)).collect() } else { vec![r] };
            let sh = a.wb.sheet_mut(a.cur);
            for k in rows {
                if h <= 0.0 {
                    sh.hidden_rows.insert(k);
                } else {
                    sh.hidden_rows.remove(&k);
                    sh.row_h.insert(k, h);
                    sh.custom_h.insert(k);
                }
            }
            a.status = format!("Height: {:.2} ({} pixels)", h * 0.75, h);
        }
        Drag::Fill(_) => {
            let s = a.sel().rect();
            let dr = if hit.row > s.r2 { hit.row - s.r2 } else if hit.row < s.r1 { s.r1 - hit.row } else { 0 };
            let dc = if hit.col > s.c2 { hit.col - s.c2 } else if hit.col < s.c1 { s.c1 - hit.col } else { 0 };
            let t = if dr == 0 && dc == 0 {
                s
            } else if dr >= dc {
                if hit.row > s.r2 { Rect::new(s.r1, s.c1, hit.row, s.c2) } else { Rect::new(hit.row, s.c1, s.r2, s.c2) }
            } else if hit.col > s.c2 {
                Rect::new(s.r1, s.c1, s.r2, hit.col)
            } else {
                Rect::new(s.r1, hit.col, s.r2, s.c2)
            };
            a.drag = Drag::Fill(t);
        }
        Drag::Move(_, grab) => {
            let s = a.sel().rect();
            let r1 = hit.row.saturating_sub(grab.row);
            let c1 = hit.col.saturating_sub(grab.col);
            a.drag = Drag::Move(Rect::new(r1, c1, r1 + s.rows() - 1, c1 + s.cols() - 1), grab);
        }
        Drag::Chart(i, mode, start, o) => {
            let d = (p - start) / g.z;
            let [mut x, mut y, mut w, mut h] = o;
            match mode {
                0 => {
                    x = (o[0] + d.x).max(0.0);
                    y = (o[1] + d.y).max(0.0);
                }
                _ => {
                    let k = mode - 1;
                    if matches!(k, 0 | 6 | 7) {
                        x = o[0] + d.x.min(o[2] - 40.0);
                        w = o[2] - d.x.min(o[2] - 40.0);
                    }
                    if matches!(k, 2..=4) {
                        w = (o[2] + d.x).max(40.0);
                    }
                    if matches!(k, 0..=2) {
                        y = o[1] + d.y.min(o[3] - 30.0);
                        h = o[3] - d.y.min(o[3] - 30.0);
                    }
                    if matches!(k, 4..=6) {
                        h = (o[3] + d.y).max(30.0);
                    }
                }
            }
            if let Some(c) = a.wb.sheet_mut(a.cur).charts.get_mut(i) {
                c.x = x;
                c.y = y;
                c.w = w;
                c.h = h;
            }
        }
        Drag::Point => {
            let anchor = a.edit.as_ref().map(|e| e.point_anchor).unwrap_or(hit);
            if a.edit.as_ref().is_some_and(|e| e.point_cur != hit) {
                let rect = a.grow_merges(Rect::from_pos(anchor, hit));
                if let Some(e) = a.edit.as_mut() {
                    e.point_cur = hit;
                }
                let s = a.cur;
                a.point_at(rect, s);
            }
        }
        _ => {}
    }
}

fn release(a: &mut App, _p: egui::Pos2, mods: egui::Modifiers, _g: &Geo) {
    let d = a.drag;
    a.drag = Drag::None;
    match d {
        Drag::ColResize(..) | Drag::RowResize(..) | Drag::Chart(..) => {
            a.changed();
            a.status.clear();
        }
        Drag::Fill(t) => {
            let s = a.sel().rect();
            if t != s {
                if a.deny_protected(t) {
                    return;
                }
                a.act(|a| {
                    if t.contains_rect(&s) {
                        ops::autofill(&mut a.wb, a.cur, s, t, mods.command);
                    } else {
                        ops::clear_contents(&mut a.wb, a.cur, &[t]);
                    }
                });
                if t.contains_rect(&s) {
                    a.sel_mut().ranges = vec![t];
                }
            }
        }
        Drag::Move(t, _) => {
            let s = a.sel().rect();
            if t != s {
                if a.deny_protected(t) || a.deny_protected(s) {
                    return;
                }
                a.act(|a| {
                    if mods.command {
                        let clip = ops::copy_rect(&a.wb, a.cur, s);
                        ops::paste(&mut a.wb, &a.calc, a.cur, &clip, t, Default::default());
                    } else {
                        ops::move_block(&mut a.wb, a.cur, s, Pos::new(t.r1, t.c1));
                    }
                });
                *a.sel_mut() = crate::app::Sel { ranges: vec![t], active: Pos::new(t.r1, t.c1), anchor: Pos::new(t.r1, t.c1) };
            }
        }
        Drag::Select => {
            if let Some((fmt, sticky)) = a.painter_fmt.clone() {
                let dest = a.sel().rect();
                a.act(|a| {
                    ops::paste(&mut a.wb, &a.calc, a.cur, &fmt, dest, ops::PasteOpts { what: ops::PasteWhat::Formats, ..Default::default() });
                });
                if !sticky {
                    a.painter_fmt = None;
                }
            }
        }
        _ => {}
    }
}

/// Width that fits the longest text in a column.
pub fn autofit_col(a: &App, ctx: &egui::Context, c: u32) -> f32 {
    let sh = a.sheet();
    let mut w: f32 = 0.0;
    ctx.fonts_mut(|f| {
        for (r, row) in sh.rows.iter() {
            if row.get(&c).is_none() && !a.calc.spill.contains_key(&(a.cur as u32, *r, c)) {
                continue;
            }
            if sh.merge_at(Pos::new(*r, c)).is_some_and(|m| !m.is_single()) {
                continue;
            }
            let t = ops::text_of(&a.wb, &a.calc, a.cur, *r, c);
            if t.is_empty() {
                continue;
            }
            let st = a.wb.styles.get(sh.style_id(*r, c));
            let font = grid::style_font(st, 1.0);
            let longest = t.split('\n').map(|l| f.layout_no_wrap(l.to_string(), font.clone(), Color32::BLACK).size().x).fold(0.0, f32::max);
            w = w.max(longest + 7.0 + st.indent as f32 * 9.0);
        }
    });
    if w == 0.0 { DEFAULT_COL_W } else { w.ceil() }
}

fn double(a: &mut App, g: &Geo, h: egui::Pos2) {
    match zone(a, g, h) {
        Zone::ColHead(_, Some(c)) => {
            let cols: Vec<u32> = if a.sel().ranges.iter().any(|r| r.whole_cols() && r.c1 <= c && c <= r.c2) { a.sel().ranges.iter().flat_map(|r| r.c1..=r.c2.min(r.c1 + 300)).collect() } else { vec![c] };
            let ctx = egui::Context::default();
            let _ = ctx;
            a.drag = Drag::None;
            a.dialog = None;
            crate::chrome::pending_autofit(cols);
        }
        Zone::RowHead(_, Some(r)) => {
            a.act(|a| {
                let sh = a.wb.sheet_mut(a.cur);
                sh.custom_h.remove(&r);
                sh.row_h.remove(&r);
            });
            a.autofit_rows(r, r);
        }
        Zone::Cell(p) => {
            if a.chart_sel.is_some() {
                return;
            }
            let sr = g.rect_of(a, a.sel().rect());
            if R::from_center_size(sr.right_bottom(), vec2(10.0, 10.0)).contains(h) {
                // Fill down as far as the column beside has data.
                let s = a.sel().rect();
                let sh = a.sheet();
                let side = if s.c1 > 0 && !sh.is_empty_at(s.r2 + 1, s.c1 - 1) { Some(s.c1 - 1) } else if !sh.is_empty_at(s.r2 + 1, s.c2 + 1) { Some(s.c2 + 1) } else { None };
                if let Some(k) = side {
                    let mut end = s.r2 + 1;
                    while !sh.is_empty_at(end + 1, k) {
                        end += 1;
                    }
                    let t = Rect::new(s.r1, s.c1, end, s.c2);
                    a.act(|a| ops::autofill(&mut a.wb, a.cur, s, t, false));
                    a.sel_mut().ranges = vec![t];
                }
                return;
            }
            if a.edit.is_none() && a.sel().active == a.sheet().merge_at(p).map_or(p, |m| Pos::new(m.r1, m.c1)) {
                a.start_edit(None, false);
            }
        }
        _ => {}
    }
}

fn hover_note(a: &App, ui: &mut egui::Ui, g: &Geo, resp: &egui::Response) {
    let Some(h) = resp.hover_pos() else { return };
    if a.drag != Drag::None || a.menu.is_some() {
        return;
    }
    let Zone::Cell(p) = zone(a, g, h) else { return };
    let sh = a.sheet();
    let p = sh.merge_at(p).map_or(p, |m| Pos::new(m.r1, m.c1));
    if let Some(n) = sh.notes.get(&p) {
        let cr = g.rect_of(a, sh.merge_at(p).unwrap_or(Rect::cell(p)));
        egui::Area::new(egui::Id::new("note-hover")).order(egui::Order::Tooltip).fixed_pos(cr.right_top() + vec2(8.0, 0.0)).interactable(false).show(ui.ctx(), |ui| {
            egui::Frame::new().fill(Color32::from_rgb(0xFF, 0xFF, 0xE1)).stroke(Stroke::new(1.0, Color32::from_rgb(0x76, 0x76, 0x76))).inner_margin(6.0).show(ui, |ui| {
                ui.set_max_width(200.0);
                if !n.author.is_empty() {
                    ui.label(egui::RichText::new(format!("{}:", n.author)).font(theme::ui_bold(11.5)));
                }
                ui.label(egui::RichText::new(&n.text).size(11.5));
            });
        });
    } else if let Some(url) = sh.links.get(&p) {
        if ui.input(|i| i.pointer.time_since_last_movement() > 0.6) {
            resp.clone().on_hover_text(format!("{url} - Click once to follow."));
        }
    }
}

fn validation_button(a: &mut App, ui: &mut egui::Ui, g: &Geo) {
    if a.edit.is_some() {
        return;
    }
    let p = a.sel().active;
    let Some(v) = a.sheet().valid.iter().find(|v| v.kind == ValidKind::List && v.in_cell_list && v.ranges.iter().any(|r| r.contains(p))).cloned() else { return };
    let cr = g.rect_of(a, a.sheet().merge_at(p).unwrap_or(Rect::cell(p)));
    let b = R::from_min_size(pos2(cr.right() + 1.0, cr.top()), vec2(cr.height().max(16.0), cr.height()));
    if !g.cells.contains(b.center()) {
        return;
    }
    let resp = ui.interact(b, egui::Id::new("valid-btn"), Sense::click());
    let p2 = ui.painter();
    p2.rect(b, 0.0, if resp.hovered() { HOVER } else { Color32::from_rgb(0xF4, 0xF4, 0xF4) }, Stroke::new(1.0, Color32::from_rgb(0xA6, 0xA6, 0xA6)), StrokeKind::Inside);
    crate::icons::draw(p2, b.shrink(2.0), crate::icons::Icon::Down, TEXT);
    if resp.clicked() {
        let _ = v;
        a.menu = Some(Menu { kind: MenuKind::Validation, pos: cr.left_bottom(), born: a.frame });
    }
}

// ----- the in-cell editor -----

fn editor(a: &mut App, ui: &mut egui::Ui, g: &Geo) {
    let Some(e) = a.edit.as_ref() else {
        a.ac_items.clear();
        return;
    };
    if e.sheet != a.cur {
        return;
    }
    let pos = e.pos;
    let st = a.style_at(pos);
    let z = g.z;
    let font = grid::style_font(&st, z);
    let base = g.rect_of(a, a.sheet().merge_at(pos).unwrap_or(Rect::cell(pos)));
    if !g.cells.intersects(base) {
        return;
    }
    // The box grows to the right with the text, then wraps at the window edge.
    let text_w = ui.fonts_mut(|f| e.text.split('\n').map(|l| f.layout_no_wrap(l.to_string(), font.clone(), Color32::BLACK).size().x).fold(0.0, f32::max));
    let max_w = (g.cells.right() - base.left() - 4.0).max(base.width());
    let w = if st.wrap { base.width() } else { (text_w + 12.0).max(base.width()).min(max_w) };
    let lines = e.text.split('\n').count().max(1) as f32;
    let line_h = font.size * 1.25;
    let h = (lines * line_h + 4.0).max(base.height());
    let r = R::from_min_size(base.min, vec2(w, h));
    let fill = st.fill.map(theme::rgb).unwrap_or(WHITE);
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("editor-layer"));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(r.expand(1.0), 0.0, fill);
    painter.rect_stroke(r.expand(1.0), 0.0, Stroke::new(2.0, ACCENT), StrokeKind::Middle);
    let focus = e.focus && !e.in_bar;
    let set_caret = if !e.in_bar { e.set_caret } else { None };
    let id = editor_id();
    if let Some(n) = set_caret {
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(n))));
        state.store(ui.ctx(), id);
    }
    let color = st.color.map(theme::rgb).unwrap_or(Color32::BLACK);
    let mut out = None;
    let e = a.edit.as_mut().unwrap();
    let wrap_w = if st.wrap { w - 6.0 } else { f32::INFINITY };
    egui::Area::new(egui::Id::new("editor-area")).order(egui::Order::Middle).fixed_pos(r.min + vec2(2.0, 1.0)).constrain(false).show(ui.ctx(), |ui| {
        let f2 = font.clone();
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
            let mut job = grid::formula_job(text.as_str(), f2.clone(), wrap_w);
            if !text.as_str().starts_with('=') {
                for s in job.sections.iter_mut() {
                    s.format.color = color;
                }
            }
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let te = egui::TextEdit::multiline(&mut e.text)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(font.clone())
            .desired_width(w - 4.0)
            .desired_rows(1)
            .margin(egui::Margin::ZERO)
            .return_key(Some(egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::Enter)))
            .layouter(&mut layouter)
            .background_color(fill);
        out = Some(te.show(ui));
    });
    let out = out.unwrap();
    if focus {
        out.response.request_focus();
        e.focus = false;
    }
    if set_caret.is_some() {
        e.set_caret = None;
    }
    if out.response.has_focus() {
        if let Some(cr) = out.cursor_range {
            e.caret = cr.primary.index.into();
        }
        e.in_bar = false;
    }
    if out.response.changed() {
        e.point = None;
        e.ac_hidden = false;
        if e.mode == EditMode::Point {
            e.mode = EditMode::Enter;
        }
    }
    completion(a, ui, r);
}

/// The identifier being typed before the caret, as (start char index, text).
fn ident_before(text: &str, caret: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let caret = caret.min(chars.len());
    let mut s = caret;
    while s > 0 && (chars[s - 1].is_alphanumeric() || chars[s - 1] == '.' || chars[s - 1] == '_') {
        s -= 1;
    }
    if s == caret || !chars[s].is_alphabetic() {
        return None;
    }
    if s > 0 && matches!(chars[s - 1], '!' | ':' | '$' | '\'') {
        return None;
    }
    if chars[..s].iter().filter(|c| **c == '"').count() % 2 == 1 {
        return None;
    }
    Some((s, chars[s..caret].iter().collect()))
}

/// The innermost function call around the caret.
fn call_at(text: &str, caret: usize) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().take(caret).collect();
    let mut depth = 0i32;
    let mut args = 0usize;
    let mut q = false;
    let mut i = chars.len();
    while i > 0 {
        i -= 1;
        match chars[i] {
            '"' => q = !q,
            ')' if !q => depth += 1,
            ',' if !q && depth == 0 => args += 1,
            '(' if !q => {
                if depth == 0 {
                    let mut s = i;
                    while s > 0 && (chars[s - 1].is_alphanumeric() || chars[s - 1] == '.' || chars[s - 1] == '_') {
                        s -= 1;
                    }
                    if s < i {
                        return Some((chars[s..i].iter().collect::<String>().to_ascii_uppercase(), args));
                    }
                    return None;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn completion(a: &mut App, ui: &mut egui::Ui, r: R) {
    let e = a.edit.as_ref().unwrap();
    a.ac_items.clear();
    if !e.text.starts_with('=') {
        return;
    }
    let reg = crate::funcs::registry();
    if let Some((_, pre)) = ident_before(&e.text, e.caret) {
        let up = pre.to_ascii_uppercase();
        let mut names: Vec<&'static str> = reg.map.keys().copied().filter(|n| n.starts_with(&up)).collect();
        for d in &a.wb.names {
            if d.name.to_ascii_uppercase().starts_with(&up) {
                names.push(Box::leak(d.name.clone().into_boxed_str()));
            }
        }
        names.sort();
        names.dedup();
        names.truncate(14);
        a.ac_items = names;
    }
    let e = a.edit.as_ref().unwrap();
    let below = r.left_bottom() + vec2(0.0, 3.0);
    if !a.ac_items.is_empty() && !e.ac_hidden {
        let sel = e.ac.min(a.ac_items.len() - 1);
        let mut chosen = None;
        egui::Area::new(egui::Id::new("ac")).order(egui::Order::Foreground).fixed_pos(below).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(2.0).corner_radius(2.0).show(ui, |ui| {
                ui.set_width(200.0);
                for (i, n) in a.ac_items.iter().enumerate() {
                    let (rr, resp) = ui.allocate_exact_size(vec2(196.0, 20.0), Sense::click());
                    if i == sel {
                        ui.painter().rect_filled(rr, 0.0, Color32::from_rgb(0xC6, 0xE0, 0xCF));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rr, 0.0, HOVER);
                    }
                    let is_fn = reg.map.contains_key(n);
                    crate::icons::draw(ui.painter(), R::from_min_size(rr.min + vec2(3.0, 3.0), vec2(14.0, 14.0)), if is_fn { crate::icons::Icon::Function } else { crate::icons::Icon::Names }, TEXT_DIM);
                    ui.painter().text(rr.left_center() + vec2(22.0, 0.0), egui::Align2::LEFT_CENTER, *n, theme::ui_font(12.0), TEXT);
                    if resp.clicked() {
                        chosen = Some(*n);
                    }
                }
            });
            if let Some(d) = reg.map.get(a.ac_items[sel]) {
                let rr = ui.min_rect();
                egui::Area::new(egui::Id::new("ac-tip")).order(egui::Order::Tooltip).fixed_pos(pos2(rr.right() + 6.0, rr.top() + sel as f32 * 20.0)).show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).inner_margin(6.0).show(ui, |ui| {
                        ui.set_max_width(260.0);
                        ui.label(egui::RichText::new(d.desc).size(11.5));
                    });
                });
            }
        });
        if let Some(n) = chosen {
            accept_completion(a, n);
        }
        return;
    }
    if let Some((name, arg)) = call_at(&e.text, e.caret) {
        if let Some(d) = reg.map.get(name.as_str()) {
            egui::Area::new(egui::Id::new("sig")).order(egui::Order::Foreground).fixed_pos(below).interactable(false).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).inner_margin(egui::Margin::symmetric(6, 3)).show(ui, |ui| {
                    let mut job = egui::text::LayoutJob::default();
                    let f = theme::ui_font(11.5);
                    let sig = d.sig;
                    let (head, rest) = sig.split_once('(').unwrap_or((sig, ""));
                    job.append(head, 0.0, egui::TextFormat { font_id: f.clone(), color: TEXT, ..Default::default() });
                    job.append("(", 0.0, egui::TextFormat { font_id: f.clone(), color: TEXT, ..Default::default() });
                    let parts: Vec<&str> = rest.trim_end_matches(')').split(", ").collect();
                    for (i, p) in parts.iter().enumerate() {
                        if i > 0 {
                            job.append(", ", 0.0, egui::TextFormat { font_id: f.clone(), color: TEXT, ..Default::default() });
                        }
                        let cur = i == arg.min(parts.len() - 1) || (i == parts.len() - 1 && arg >= parts.len());
                        job.append(p, 0.0, egui::TextFormat { font_id: if cur { theme::ui_bold(11.5) } else { f.clone() }, color: TEXT, ..Default::default() });
                    }
                    job.append(")", 0.0, egui::TextFormat { font_id: f, color: TEXT, ..Default::default() });
                    ui.label(job);
                });
            });
        }
    }
}

pub fn accept_completion(a: &mut App, name: &str) {
    let Some(e) = a.edit.as_mut() else { return };
    let Some((s, _)) = ident_before(&e.text, e.caret) else { return };
    let mut chars: Vec<char> = e.text.chars().collect();
    let is_fn = crate::funcs::registry().map.contains_key(name);
    let ins = if is_fn { format!("{name}(") } else { name.to_string() };
    let end = e.caret.min(chars.len());
    chars.splice(s..end, ins.chars());
    e.text = chars.into_iter().collect();
    e.caret = s + ins.chars().count();
    e.set_caret = Some(e.caret);
    e.focus = true;
    e.ac = 0;
}

/// F4 while editing: cycles $ signs on the reference at the caret.
pub fn toggle_abs_at_caret(a: &mut App) {
    let Some(e) = a.edit.as_mut() else { return };
    let refs = crate::formula::find_refs(&e.text);
    let Some((s, t, text)) = refs.into_iter().find(|(s, t, _)| *s <= e.caret && e.caret <= *t) else { return };
    let (sheet, body) = match text.rsplit_once('!') {
        Some((a, b)) => (format!("{a}!"), b.to_string()),
        None => (String::new(), text.clone()),
    };
    let new = format!("{sheet}{}", crate::formula::cycle_abs(&body));
    let mut chars: Vec<char> = e.text.chars().collect();
    chars.splice(s..t, new.chars());
    e.text = chars.into_iter().collect();
    let end = s + new.chars().count();
    e.caret = end;
    e.set_caret = Some(end);
    if e.point.is_some() {
        e.point = Some((s, end));
    }
    e.focus = true;
}

pub fn last_grid_rect() -> Option<R> {
    LAST_GEO.with(|x| x.get())
}

/// Name of the function being typed, shown in the name box while editing a formula.
pub fn call_name(text: &str, caret: usize) -> Option<String> {
    call_at(text, caret).map(|x| x.0).filter(|n| crate::funcs::registry().map.contains_key(n.as_str()))
}
