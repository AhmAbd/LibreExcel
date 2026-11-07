//! Title bar, formula bar, sheet tabs, status bar, popup menu frame and message boxes.

use crate::app::{App, EditMode, Menu, MenuKind, Page, ViewMode};
use crate::icons::{self, Icon};
use crate::model::*;
use crate::theme::{self, *};
use egui::{pos2, vec2, Align2, Color32, Rect as R, Sense, Stroke, StrokeKind};
use std::cell::RefCell;

thread_local! {
    static AUTOFIT: RefCell<Vec<u32>> = const { RefCell::new(vec![]) };
}

pub fn bar_id() -> egui::Id {
    egui::Id::new("formula-bar")
}

/// Column auto-fit needs font measurements, so the grid asks for it here and it runs next frame.
pub fn pending_autofit(cols: Vec<u32>) {
    AUTOFIT.with(|a| a.borrow_mut().extend(cols));
}

pub fn run_autofit(a: &mut App, ctx: &egui::Context) {
    let cols: Vec<u32> = AUTOFIT.with(|x| std::mem::take(&mut *x.borrow_mut()));
    if cols.is_empty() {
        return;
    }
    let widths: Vec<(u32, f32)> = cols.iter().map(|c| (*c, crate::grid_input::autofit_col(a, ctx, *c))).collect();
    a.act(|a| {
        let sh = a.wb.sheet_mut(a.cur);
        for (c, w) in widths {
            sh.hidden_cols.remove(&c);
            sh.col_w.insert(c, w);
        }
    });
}

/// A flat button with an icon, used in the title bar and elsewhere. Returns the response.
pub fn icon_button(ui: &mut egui::Ui, r: R, icon: Icon, ink: Color32, hover: Color32, id: egui::Id, enabled: bool) -> egui::Response {
    let resp = ui.interact(r, id, if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.hovered() {
        ui.painter().rect_filled(r, 2.0, hover);
    }
    let side = 16.0f32.min(r.height() - 4.0);
    icons::draw(ui.painter(), R::from_center_size(r.center(), vec2(side, side)), icon, if enabled { ink } else { ink.gamma_multiply(0.45) });
    resp
}

pub fn title_bar(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let ctx = ui.ctx().clone();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, GREEN);
    // Dragging the empty bar moves the window; double-click maximizes.
    let drag = ui.interact(rect, egui::Id::new("titlebar"), Sense::click_and_drag());
    if drag.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if drag.double_clicked() {
        a.maximized = !a.maximized;
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(a.maximized));
    }
    let h = rect.height();
    let mut x = rect.left() + 8.0;
    // App mark.
    let mark = R::from_min_size(pos2(x, rect.top() + 7.0), vec2(18.0, 18.0));
    p.rect_filled(mark, 3.0, WHITE);
    p.rect_filled(mark.shrink(2.5), 1.0, GREEN);
    p.text(mark.center(), Align2::CENTER_CENTER, "T", theme::ui_bold(11.0), WHITE);
    x += 30.0;
    // AutoSave switch.
    p.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, "AutoSave", theme::ui_font(12.0), WHITE);
    x += 56.0;
    let sw = R::from_min_size(pos2(x, rect.center().y - 8.0), vec2(36.0, 16.0));
    let swr = ui.interact(sw, egui::Id::new("autosave"), Sense::click());
    if swr.clicked() {
        if a.path.is_none() && !a.autosave {
            a.autosave = a.save();
        } else {
            a.autosave = !a.autosave;
        }
    }
    let p = ui.painter();
    p.rect(sw, 8.0, if a.autosave { WHITE } else { GREEN }, Stroke::new(1.0, WHITE), StrokeKind::Inside);
    let knob = if a.autosave { sw.right() - 8.0 } else { sw.left() + 8.0 };
    p.circle_filled(pos2(knob, sw.center().y), 5.0, if a.autosave { GREEN } else { WHITE });
    p.text(pos2(sw.right() + 4.0, sw.center().y), Align2::LEFT_CENTER, if a.autosave { "On" } else { "Off" }, theme::ui_font(11.5), WHITE);
    swr.on_hover_text("AutoSave saves your workbook after every change.");
    x += 64.0;
    // Quick access buttons.
    let bw = 30.0;
    let items = [(Icon::Save, "Save (Ctrl+S)", true), (Icon::Undo, "Undo (Ctrl+Z)", a.can_undo()), (Icon::Redo, "Redo (Ctrl+Y)", a.can_redo())];
    for (i, (ic, tip, en)) in items.iter().enumerate() {
        let r = R::from_min_size(pos2(x, rect.top()), vec2(bw, h));
        let resp = icon_button(ui, r, *ic, WHITE, GREEN_HOVER, egui::Id::new(("qat", i)), *en).on_hover_text(*tip);
        if resp.clicked() {
            match i {
                0 => {
                    a.save();
                }
                1 => a.undo(),
                _ => a.redo(),
            }
        }
        x += bw;
    }
    let r = R::from_min_size(pos2(x, rect.top()), vec2(18.0, h));
    icon_button(ui, r, Icon::Down, WHITE, GREEN_HOVER, egui::Id::new("qat-more"), true).on_hover_text("Customize Quick Access Toolbar");
    // Title.
    let title = a.title();
    let tw = 220.0f32.max(title.len() as f32 * 7.0);
    let center = rect.center().x - 110.0;
    let p = ui.painter();
    p.text(pos2(center.max(x + 20.0 + tw / 2.0), rect.center().y), Align2::CENTER_CENTER, &title, theme::ui_font(12.5), WHITE);
    // Search box.
    let sx = (center + tw / 2.0 + 30.0).max(x + tw + 40.0);
    let sbw = (rect.right() - 200.0 - sx).min(360.0);
    if sbw > 120.0 {
        let sr = R::from_min_size(pos2(sx, rect.top() + 5.0), vec2(sbw, h - 10.0));
        let resp = ui.interact(sr, egui::Id::new("search-pill"), Sense::click());
        let p = ui.painter();
        p.rect_filled(sr, 4.0, if resp.hovered() || a.search_open { Color32::from_rgb(0x4D, 0x8F, 0x6B) } else { Color32::from_rgb(0x3A, 0x84, 0x5C) });
        icons::draw(p, R::from_min_size(sr.min + vec2(8.0, (sr.height() - 14.0) / 2.0), vec2(14.0, 14.0)), Icon::Search, WHITE);
        if a.search_open {
            let te = egui::TextEdit::singleline(&mut a.search).id(egui::Id::new("search-box")).frame(egui::Frame::NONE).text_color(WHITE).font(theme::ui_font(12.0)).hint_text(egui::RichText::new("Search for tools, help and more (Alt+Q)").color(Color32::from_rgb(0xD8, 0xEB, 0xE0)));
            let tr = ui.put(R::from_min_max(sr.min + vec2(28.0, 3.0), sr.max - vec2(6.0, 2.0)), te);
            if tr.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !a.search.is_empty() {
                    a.find.what = a.search.clone();
                    crate::dialogs::find_next(a, false);
                }
                a.search_open = false;
            }
            if !tr.has_focus() && a.search_open {
                tr.request_focus();
            }
        } else {
            p.text(sr.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, "Search (Alt+Q)", theme::ui_font(12.0), Color32::from_rgb(0xE0, 0xF0, 0xE6));
            if resp.clicked() {
                a.search_open = true;
            }
        }
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::Q)) {
        a.search_open = true;
    }
    // Account and window buttons.
    let mut rx = rect.right();
    let wbw = 46.0;
    let wins = [(Icon::Close, 0u8), (if a.maximized { Icon::Restore } else { Icon::Maximize }, 1), (Icon::Minimize, 2)];
    for (ic, k) in wins {
        rx -= wbw;
        let r = R::from_min_size(pos2(rx, rect.top()), vec2(wbw, h));
        let hover = if k == 0 { Color32::from_rgb(0xE8, 0x11, 0x23) } else { GREEN_HOVER };
        if icon_button(ui, r, ic, WHITE, hover, egui::Id::new(("win", k)), true).clicked() {
            match k {
                0 => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                1 => {
                    a.maximized = !a.maximized;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(a.maximized));
                }
                _ => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            }
        }
    }
    rx -= 12.0;
    let av = pos2(rx - 14.0, rect.center().y);
    let p = ui.painter();
    p.circle_filled(av, 13.0, Color32::from_rgb(0xC8, 0xE6, 0xD2));
    let initials: String = a.wb.author.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
    p.text(av, Align2::CENTER_CENTER, if initials.is_empty() { "U".to_string() } else { initials }, theme::ui_bold(11.0), GREEN_DARK);
    p.text(pos2(av.x - 22.0, rect.center().y), Align2::RIGHT_CENTER, &a.wb.author, theme::ui_font(12.0), WHITE);
}

/// Name box, function buttons and the formula text box.
pub fn formula_bar(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, WHITE);
    p.hline(rect.left()..=rect.right(), rect.bottom() - 0.5, Stroke::new(1.0, RIBBON_LINE));
    let line_h = 22.0;
    let top = rect.top() + 4.0;
    // Name box.
    let nb = R::from_min_size(pos2(rect.left() + 4.0, top), vec2(96.0, line_h));
    let label = {
        let s = a.sel();
        let r = s.rect();
        if let Some(e) = &a.edit {
            if e.text.starts_with('=') {
                crate::grid_input::call_name(&e.text, e.caret).unwrap_or_default()
            } else {
                cell_name(e.pos.row, e.pos.col)
            }
        } else if let Some(i) = a.chart_sel {
            format!("Chart {}", i + 1)
        } else if crate::app::Drag::Select == a.drag && !r.is_single() {
            format!("{}R x {}C", r.rows(), r.cols())
        } else if let Some(n) = a.wb.names.iter().find(|n| n.expr.trim_start_matches('=').replace('$', "").ends_with(&format!("!{}", r.a1())) && a.sheet().name.len() > 0) {
            n.name.clone()
        } else {
            cell_name(s.active.row, s.active.col)
        }
    };
    let mut text = a.name_box.clone().unwrap_or(label);
    ui.painter().rect_stroke(nb, 0.0, Stroke::new(1.0, Color32::from_rgb(0xC6, 0xC6, 0xC6)), StrokeKind::Inside);
    let te = egui::TextEdit::singleline(&mut text).id(egui::Id::new("name-box")).frame(egui::Frame::NONE).font(theme::ui_font(12.5)).margin(egui::Margin::symmetric(5, 3));
    let resp = ui.put(nb.shrink2(vec2(1.0, 1.0)), te);
    if resp.gained_focus() {
        a.name_box = Some(text.clone());
        if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), resp.id) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()))));
            st.store(ui.ctx(), resp.id);
        }
    } else if resp.has_focus() {
        a.name_box = Some(text.clone());
    }
    if resp.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            crate::dialogs::go_to(a, &text, true);
        }
        a.name_box = None;
    }
    let p = ui.painter();
    let dd = R::from_min_size(pos2(nb.right(), top), vec2(14.0, line_h));
    icons::draw(p, dd.shrink(1.0), Icon::Down, TEXT_DIM);
    // Separator dots and buttons.
    let mut x = dd.right() + 8.0;
    for k in 0..3 {
        p.circle_filled(pos2(x, top + 6.0 + k as f32 * 5.0), 1.0, TEXT_OFF);
    }
    x += 8.0;
    let editing = a.edit.is_some();
    let cancel = R::from_min_size(pos2(x, top), vec2(26.0, line_h));
    if icon_button(ui, cancel, Icon::Cross, if editing { Color32::from_rgb(0xC4, 0x2B, 0x1C) } else { TEXT_OFF }, HOVER, egui::Id::new("fb-cancel"), editing).on_hover_text("Cancel").clicked() {
        a.cancel_edit();
    }
    x += 26.0;
    let enter = R::from_min_size(pos2(x, top), vec2(26.0, line_h));
    if icon_button(ui, enter, Icon::Check, if editing { GREEN } else { TEXT_OFF }, HOVER, egui::Id::new("fb-enter"), editing).on_hover_text("Enter").clicked() {
        a.commit_edit(false);
    }
    x += 26.0;
    let fx = R::from_min_size(pos2(x, top), vec2(28.0, line_h));
    let fxr = ui.interact(fx, egui::Id::new("fb-fx"), Sense::click()).on_hover_text("Insert Function");
    if fxr.hovered() {
        ui.painter().rect_filled(fx, 2.0, HOVER);
    }
    ui.painter().text(fx.center(), Align2::CENTER_CENTER, "fx", egui::FontId::new(14.0, egui::FontFamily::Name("serif-i".into())), TEXT);
    if fxr.clicked() {
        a.dialog = Some(crate::dialogs::Dialog::insert_function(a));
    }
    x += 34.0;
    // Formula box shares its text with the in-cell editor.
    let lines = if a.bar_tall { 4.0 } else { 1.0 };
    let fb = R::from_min_max(pos2(x, top), pos2(rect.right() - 26.0, top + line_h * lines));
    ui.painter().rect_stroke(fb, 0.0, Stroke::new(1.0, Color32::from_rgb(0xC6, 0xC6, 0xC6)), StrokeKind::Inside);
    let expand = R::from_min_size(pos2(fb.right() + 2.0, top), vec2(20.0, line_h));
    if icon_button(ui, expand, if a.bar_tall { Icon::Up } else { Icon::Down }, TEXT_DIM, HOVER, egui::Id::new("fb-expand"), true).on_hover_text(if a.bar_tall { "Collapse Formula Bar" } else { "Expand Formula Bar" }).clicked() {
        a.bar_tall = !a.bar_tall;
    }
    let font = theme::ui_font(13.0);
    let id = bar_id();
    let inner = fb.shrink2(vec2(5.0, 3.0));
    if a.edit.is_some() {
        let (focus, caret) = {
            let e = a.edit.as_ref().unwrap();
            (e.focus && e.in_bar, if e.in_bar { e.set_caret } else { None })
        };
        if let Some(n) = caret {
            let mut st = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(n))));
            st.store(ui.ctx(), id);
        }
        let e = a.edit.as_mut().unwrap();
        let f2 = font.clone();
        let mut layouter = |ui: &egui::Ui, t: &dyn egui::TextBuffer, w: f32| ui.fonts_mut(|f| f.layout_job(crate::grid::formula_job(t.as_str(), f2.clone(), w)));
        let te = egui::TextEdit::multiline(&mut e.text).id(id).frame(egui::Frame::NONE).font(font.clone()).margin(egui::Margin::ZERO).desired_width(inner.width()).desired_rows(1).return_key(Some(egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::Enter))).layouter(&mut layouter);
        let out = ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            egui::ScrollArea::vertical().max_height(inner.height()).id_salt("fb-scroll").show(ui, |ui| te.show(ui)).inner
        }).inner;
        if focus {
            out.response.request_focus();
            e.focus = false;
        }
        if e.in_bar && caret.is_some() {
            e.set_caret = None;
        }
        if out.response.has_focus() {
            e.in_bar = true;
            if let Some(cr) = out.cursor_range {
                e.caret = cr.primary.index.into();
            }
        }
        if out.response.changed() {
            e.point = None;
            if e.mode == EditMode::Point {
                e.mode = EditMode::Enter;
            }
        }
    } else {
        let p = a.sel().active;
        let shown = if a.chart_sel.is_some() {
            String::new()
        } else if a.style_at(p).hide_formula && a.sheet().protection.is_some() {
            String::new()
        } else {
            crate::ops::edit_text(&a.wb, a.cur, p.row, p.col)
        };
        let job = crate::grid::formula_job(&shown, font.clone(), inner.width());
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().with_clip_rect(inner).galley(inner.min, galley, TEXT);
        let resp = ui.interact(fb, egui::Id::new("fb-click"), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if resp.clicked() {
            a.start_edit(None, true);
            if let Some(e) = a.edit.as_mut() {
                e.in_bar = true;
                e.mode = EditMode::Edit;
            }
        }
    }
}

pub fn sheet_tabs(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, RIBBON_BG);
    p.hline(rect.left()..=rect.right(), rect.top() + 0.5, Stroke::new(1.0, RIBBON_LINE));
    let h = rect.height();
    let mut x = rect.left() + 4.0;
    for (i, ic) in [Icon::Left, Icon::Right].iter().enumerate() {
        let r = R::from_min_size(pos2(x, rect.top() + 2.0), vec2(22.0, h - 4.0));
        if icon_button(ui, r, *ic, TEXT_DIM, HOVER, egui::Id::new(("tabnav", i)), true).clicked() {
            if i == 0 {
                a.tab_scroll = a.tab_scroll.saturating_sub(1);
            } else {
                a.tab_scroll = (a.tab_scroll + 1).min(a.wb.sheets.len().saturating_sub(1));
            }
        }
        x += 22.0;
    }
    x += 6.0;
    let scroll_w = (rect.width() * 0.38).clamp(160.0, 520.0);
    let tabs_right = rect.right() - scroll_w - 10.0;
    let font = theme::ui_font(12.5);
    let bold = theme::ui_bold(12.5);
    let mut drop_target = None;
    let n = a.wb.sheets.len();
    for i in a.tab_scroll.min(n.saturating_sub(1))..n {
        let sh = &a.wb.sheets[i];
        if sh.hidden {
            continue;
        }
        let active = i == a.cur;
        let name = sh.name.clone();
        let tw = ui.fonts_mut(|f| f.layout_no_wrap(name.clone(), if active { bold.clone() } else { font.clone() }, TEXT).size().x) + 24.0;
        if x + tw > tabs_right {
            break;
        }
        let r = R::from_min_size(pos2(x, rect.top()), vec2(tw, h - 2.0));
        let resp = ui.interact(r, egui::Id::new(("tab", i)), Sense::click_and_drag());
        let p = ui.painter();
        if active {
            p.rect_filled(r, 0.0, WHITE);
            p.hline(r.left() + 6.0..=r.right() - 6.0, r.bottom() - 2.0, Stroke::new(3.0, ACCENT));
        } else if resp.hovered() {
            p.rect_filled(r, 0.0, HOVER);
        }
        if let Some(c) = sh.tab_color {
            p.hline(r.left() + 2.0..=r.right() - 2.0, r.bottom() - if active { 5.0 } else { 2.0 }, Stroke::new(3.0, theme::rgb(c)));
        }
        p.vline(r.right() + 0.5, r.top() + 6.0..=r.bottom() - 6.0, Stroke::new(1.0, RIBBON_LINE));
        if a.rename_tab.as_ref().is_some_and(|t| t.0 == i) {
            let mut t = a.rename_tab.as_ref().unwrap().1.clone();
            let te = egui::TextEdit::singleline(&mut t).id(egui::Id::new("tab-rename")).font(font.clone()).margin(egui::Margin::symmetric(2, 1));
            let tr = ui.put(r.shrink2(vec2(4.0, 4.0)), te);
            tr.request_focus();
            a.rename_tab.as_mut().unwrap().1 = t.clone();
            if tr.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let new = t.trim().to_string();
                a.rename_tab = None;
                rename_sheet(a, i, &new);
            }
        } else {
            p.text(r.center(), Align2::CENTER_CENTER, &name, if active { bold.clone() } else { font.clone() }, if active { GREEN_DARK } else { TEXT });
        }
        if resp.clicked() {
            a.switch_sheet(i);
        }
        if resp.double_clicked() {
            a.rename_tab = Some((i, name.clone()));
        }
        if resp.secondary_clicked() {
            a.switch_sheet(i);
            a.menu = Some(Menu { kind: MenuKind::TabCtx(i), pos: ui.input(|i| i.pointer.interact_pos()).unwrap_or(r.left_top()), born: a.frame });
        }
        if resp.dragged() {
            if let Some(pp) = ui.input(|i| i.pointer.interact_pos()) {
                drop_target = Some((i, pp.x));
            }
        }
        if resp.drag_stopped() {
            if let Some(pp) = ui.input(|i| i.pointer.interact_pos()) {
                drop_target = Some((i, -pp.x - 1.0));
            }
        }
        x += tw + 1.0;
    }
    // Reordering by dragging a tab.
    if let Some((from, px)) = drop_target {
        let release = px < 0.0;
        let px = if release { -px - 1.0 } else { px };
        let mut tx = rect.left() + 54.0;
        let mut to = n - 1;
        for i in a.tab_scroll.min(n.saturating_sub(1))..n {
            let w = ui.fonts_mut(|f| f.layout_no_wrap(a.wb.sheets[i].name.clone(), font.clone(), TEXT).size().x) + 25.0;
            if px < tx + w / 2.0 {
                to = i;
                break;
            }
            tx += w;
        }
        if release && to != from {
            a.act(|a| {
                let s = a.wb.sheets.remove(from);
                a.wb.sheets.insert(to.min(a.wb.sheets.len()), s);
                let sel = a.sels.remove(from);
                a.sels.insert(to.min(a.sels.len()), sel);
                let v = a.views.remove(from);
                a.views.insert(to.min(a.views.len()), v);
                a.cur = to.min(a.wb.sheets.len() - 1);
            });
        } else if !release {
            ui.painter().vline(px, rect.top()..=rect.bottom(), Stroke::new(2.0, ACCENT));
        }
    }
    // New sheet button.
    let plus = R::from_center_size(pos2(x + 16.0, rect.center().y), vec2(22.0, 22.0));
    let pr = ui.interact(plus, egui::Id::new("tab-plus"), Sense::click()).on_hover_text("New sheet");
    ui.painter().circle(plus.center(), 9.5, if pr.hovered() { HOVER } else { RIBBON_BG }, Stroke::new(1.0, TEXT_DIM));
    icons::draw(ui.painter(), plus.shrink(6.0), Icon::Plus, TEXT_DIM);
    if pr.clicked() {
        a.insert_sheet(a.wb.sheets.len() - 1);
    }
    let sr = R::from_min_max(pos2(rect.right() - scroll_w, rect.top() + 3.0), pos2(rect.right(), rect.bottom() - 3.0));
    crate::grid_input::hscroll(a, ui, sr);
}

pub fn rename_sheet(a: &mut App, i: usize, new: &str) {
    if new.is_empty() || new == a.wb.sheets[i].name {
        return;
    }
    if new.len() > 31 || new.contains(['\\', '/', '?', '*', '[', ']', ':']) || new.starts_with('\'') {
        a.alert = Some(("Rename sheet".into(), "You typed an invalid name for a sheet. Make sure that:\n\n• The name is 31 characters or fewer.\n• It doesn't contain any of these: : \\ / ? * [ ]\n• It doesn't begin or end with an apostrophe.".into()));
        return;
    }
    if a.wb.sheet_index(new).is_some_and(|k| k != i) {
        a.alert = Some(("Rename sheet".into(), "That name is already taken. Try a different one.".into()));
        return;
    }
    let s = new.to_string();
    a.act(|a| crate::ops::rename_sheet(&mut a.wb, i, &s));
}

pub fn status_bar(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, RIBBON_BG);
    p.hline(rect.left()..=rect.right(), rect.top() + 0.5, Stroke::new(1.0, RIBBON_LINE));
    let font = theme::ui_font(12.0);
    let mode = crate::grid::edit_mode_label(a);
    let mut left = mode.to_string();
    if a.calc.circular.is_some() {
        let (si, pos) = a.calc.circular.unwrap();
        left = format!("{left}     Circular References: {}{}", if si != a.cur { format!("{}!", a.wb.sheets.get(si).map_or("", |s| s.name.as_str())) } else { String::new() }, cell_name(pos.row, pos.col));
    }
    if !a.status.is_empty() {
        left = format!("{left}     {}", a.status);
    }
    if a.marquee && a.edit.is_none() {
        left = format!("{left}     Select destination and press ENTER or choose Paste");
    }
    if a.painter_fmt.is_some() {
        left = format!("{left}     Click a cell to paste the format");
    }
    if a.sheet().filter.as_ref().is_some_and(|f| f.cols.values().any(|c| c.allowed.is_some() || c.custom.is_some())) {
        let shown = a.sheet().filter.as_ref().map(|f| (f.range.r1 + 1..=f.range.r2).filter(|r| !a.sheet().filtered_rows.contains(r)).count()).unwrap_or(0);
        let total = a.sheet().filter.as_ref().map(|f| f.range.rows() - 1).unwrap_or(0);
        left = format!("{left}     {shown} of {total} records found");
    }
    p.text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, left, font.clone(), TEXT);
    // Zoom controls on the right.
    let mut x = rect.right() - 10.0;
    let z = a.sheet().zoom;
    let label = format!("{}%", (z * 100.0).round());
    let lr = R::from_min_size(pos2(x - 42.0, rect.top()), vec2(42.0, rect.height()));
    let resp = ui.interact(lr, egui::Id::new("zoom-label"), Sense::click()).on_hover_text("Zoom level");
    if resp.hovered() {
        ui.painter().rect_filled(lr, 0.0, HOVER);
    }
    ui.painter().text(lr.center(), Align2::CENTER_CENTER, label, font.clone(), TEXT);
    if resp.clicked() {
        a.dialog = Some(crate::dialogs::Dialog::Zoom((z * 100.0).round() as u32));
    }
    x -= 48.0;
    let plus = R::from_min_size(pos2(x - 16.0, rect.top() + 3.0), vec2(16.0, rect.height() - 6.0));
    if icon_button(ui, plus, Icon::Plus, TEXT_DIM, HOVER, egui::Id::new("zoom+"), true).clicked() {
        a.wb.sheet_mut(a.cur).zoom = ((z * 10.0).round() / 10.0 + 0.1).min(4.0);
    }
    x -= 20.0;
    let sw = 100.0;
    let track = R::from_min_size(pos2(x - sw, rect.center().y - 6.0), vec2(sw, 12.0));
    let tr = ui.interact(track, egui::Id::new("zoom-track"), Sense::click_and_drag());
    let p = ui.painter();
    p.hline(track.left()..=track.right(), track.center().y, Stroke::new(1.0, TEXT_DIM));
    p.vline(track.center().x, track.top() + 2.0..=track.bottom() - 2.0, Stroke::new(1.0, TEXT_DIM));
    // 10% to 100% on the left half, 100% to 400% on the right half.
    let frac = if z <= 1.0 { (z - 0.1) / 0.9 * 0.5 } else { 0.5 + (z - 1.0) / 3.0 * 0.5 };
    let kx = track.left() + frac * sw;
    p.rect_filled(R::from_center_size(pos2(kx, track.center().y), vec2(4.0, 12.0)), 1.0, TEXT);
    if tr.dragged() || tr.clicked() {
        if let Some(pp) = tr.interact_pointer_pos() {
            let f = ((pp.x - track.left()) / sw).clamp(0.0, 1.0);
            let nz = if f <= 0.5 { 0.1 + f / 0.5 * 0.9 } else { 1.0 + (f - 0.5) / 0.5 * 3.0 };
            let nz = if (nz - 1.0).abs() < 0.04 { 1.0 } else { nz };
            a.wb.sheet_mut(a.cur).zoom = (nz * 100.0).round() / 100.0;
        }
    }
    x -= sw + 4.0;
    let minus = R::from_min_size(pos2(x - 16.0, rect.top() + 3.0), vec2(16.0, rect.height() - 6.0));
    if icon_button(ui, minus, Icon::Minimize, TEXT_DIM, HOVER, egui::Id::new("zoom-"), true).clicked() {
        a.wb.sheet_mut(a.cur).zoom = ((z * 10.0).round() / 10.0 - 0.1).max(0.1);
    }
    x -= 32.0;
    for (ic, m, tip) in [(Icon::ViewBreak, ViewMode::Break, "Page Break Preview"), (Icon::ViewLayout, ViewMode::Layout, "Page Layout"), (Icon::ViewNormal, ViewMode::Normal, "Normal")] {
        let r = R::from_min_size(pos2(x - 24.0, rect.top() + 2.0), vec2(24.0, rect.height() - 4.0));
        if a.view_mode == m {
            ui.painter().rect_filled(r, 2.0, CHECKED);
        }
        if icon_button(ui, r, ic, TEXT_DIM, HOVER, egui::Id::new(("view", tip)), true).on_hover_text(tip).clicked() {
            a.view_mode = m;
        }
        x -= 26.0;
    }
    x -= 16.0;
    // Selection statistics.
    if a.edit.is_none() {
        if let Some((avg, count, sum, nums, lo, hi)) = a.stats() {
            let fmt = |v: f64| {
                let st = a.style_at(a.sel().active);
                let r = crate::ops::render_value(&crate::eval::Value::Num(v), &st, 11);
                if r.text.contains('#') { crate::eval::num_text(v) } else { r.text }
            };
            let mut parts = vec![];
            if nums > 0 {
                parts.push(format!("Average: {}", fmt(avg)));
            }
            if count > 1 || nums == 0 && count > 0 {
                parts.push(format!("Count: {count}"));
            }
            if nums > 0 {
                parts.push(format!("Sum: {}", fmt(sum)));
                let _ = (lo, hi);
            }
            let t = parts.join("     ");
            ui.painter().text(pos2(x, rect.center().y), Align2::RIGHT_CENTER, t, font, TEXT);
        }
    }
}

/// Shows the open popup menu; clicking outside closes it.
pub fn menus(a: &mut App, ctx: &egui::Context) {
    let Some(m) = a.menu else { return };
    let screen = ctx.content_rect();
    let resp = egui::Area::new(egui::Id::new(("menu", format!("{:?}", m.kind)))).order(egui::Order::Foreground).fixed_pos(m.pos).constrain_to(screen).show(ctx, |ui| {
        egui::Frame::new().fill(WHITE).stroke(Stroke::new(1.0, Color32::from_rgb(0xC8, 0xC8, 0xC8))).corner_radius(4.0).inner_margin(4.0).shadow(egui::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(40) }).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            crate::menus::content(a, ui, m.kind)
        })
    });
    let close = resp.inner.inner;
    let clicked_out = ctx.input(|i| i.pointer.any_pressed()) && !resp.response.rect.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default()) && a.frame > m.born + 1;
    let sub_open = ctx.any_popup_open();
    if close || clicked_out && !sub_open {
        if a.menu == Some(m) {
            a.menu = None;
        }
    }
}

/// One row in a popup menu.
pub fn item(ui: &mut egui::Ui, icon: Option<Icon>, label: &str, key: &str) -> bool {
    item_on(ui, icon, label, key, true, false)
}

pub fn item_on(ui: &mut egui::Ui, icon: Option<Icon>, label: &str, key: &str, enabled: bool, checked: bool) -> bool {
    let w = ui.available_width().max(230.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if resp.hovered() && enabled {
        p.rect_filled(r, 3.0, HOVER);
    }
    if checked {
        p.rect_filled(R::from_min_size(r.min + vec2(4.0, 3.0), vec2(20.0, 20.0)), 2.0, CHECKED);
    }
    let ink = if enabled { TEXT } else { TEXT_OFF };
    if let Some(ic) = icon {
        icons::draw(p, R::from_min_size(r.min + vec2(6.0, 5.0), vec2(16.0, 16.0)), ic, ink);
    } else if checked {
        icons::draw(p, R::from_min_size(r.min + vec2(6.0, 5.0), vec2(16.0, 16.0)), Icon::Check, ink);
    }
    p.text(r.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, label, theme::ui_font(12.5), ink);
    if !key.is_empty() {
        p.text(r.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, key, theme::ui_font(11.5), TEXT_DIM);
    }
    resp.clicked() && enabled
}

pub fn separator(ui: &mut egui::Ui) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 7.0), Sense::hover());
    ui.painter().hline(r.left() + 4.0..=r.right() - 4.0, r.center().y, Stroke::new(1.0, RIBBON_LINE));
}

pub fn header(ui: &mut egui::Ui, text: &str) {
    let w = ui.available_width().max(230.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0xF0, 0xF0, 0xF0));
    ui.painter().text(r.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, text, theme::ui_bold(12.0), TEXT);
}

/// Message boxes shown over everything.
pub fn alert(a: &mut App, ctx: &egui::Context) {
    let Some((title, msg)) = a.alert.clone() else { return };
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("alert-shade")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.allocate_rect(screen, Sense::click());
    });
    let mut close = false;
    egui::Window::new(title).id(egui::Id::new("alert")).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, -40.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 15.0, Color32::from_rgb(0x00, 0x67, 0xC0));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "i", theme::ui_bold(18.0), WHITE);
            ui.add_space(8.0);
            ui.label(egui::RichText::new(&msg).size(12.5));
        });
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("    OK    ").fill(Color32::from_rgb(0xE5, 0xF1, 0xEA))).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                close = true;
            }
        });
    });
    if close {
        a.alert = None;
    }
}

pub fn page_title(p: Page) -> &'static str {
    match p {
        Page::Home => "Home",
        Page::New => "New",
        Page::Open => "Open",
        Page::Info => "Info",
        Page::SaveAs => "Save As",
        Page::Print => "Print",
        Page::Export => "Export",
        Page::About => "About",
    }
}
