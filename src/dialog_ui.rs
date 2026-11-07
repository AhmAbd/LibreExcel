//! Dialog windows: the shared frame, small helpers and the simpler dialogs.

use crate::app::App;
use crate::dialog_tabs as big;
use crate::dialogs::{self, Dialog};
use crate::eval::Value;
use crate::model::*;
use crate::ops::{self, PasteOp, PasteWhat};
use crate::theme;
use egui::{vec2, Align2, Color32, ComboBox, RichText, Sense, Stroke, StrokeKind, TextEdit};
use std::sync::Arc;

/// Whether a dialog stays open after this frame.
#[derive(PartialEq)]
pub enum Out {
    Stay,
    Close,
}

fn title(d: &Dialog) -> &'static str {
    match d {
        Dialog::FormatCells(_) => "Format Cells",
        Dialog::FindReplace(_) => "Find and Replace",
        Dialog::GoTo(_) => "Go To",
        Dialog::GoSpecial(_) => "Go To Special",
        Dialog::DeleteCells(_) => "Delete",
        Dialog::InsertCells(_) => "Insert",
        Dialog::Zoom(_) => "Zoom",
        Dialog::NotesList => "Notes",
        Dialog::Symbol => "Symbol",
        Dialog::NameManager(_) => "Name Manager",
        Dialog::DefineName { editing: Some(_), .. } => "Edit Name",
        Dialog::DefineName { .. } => "New Name",
        Dialog::TextToColumns { .. } => "Convert Text to Columns",
        Dialog::Protect { .. } => "Protect Sheet",
        Dialog::Unprotect(_) => "Unprotect Sheet",
        Dialog::Stats => "Workbook Statistics",
        Dialog::Unhide(_) => "Unhide",
        Dialog::Shortcuts => "Keyboard Shortcuts",
        Dialog::PasteSpecial(_) => "Paste Special",
        Dialog::MoreColors { .. } => "Colors",
        Dialog::CreateTable { .. } => "Create Table",
        Dialog::Size { rows: true, .. } => "Row Height",
        Dialog::Size { .. } => "Column Width",
        Dialog::MoveCopy { .. } => "Move or Copy",
        Dialog::Series { .. } => "Series",
        Dialog::CondQuick { op, .. } => match op.as_str() {
            ">" => "Greater Than",
            "<" => "Less Than",
            "between" => "Between",
            "=" => "Equal To",
            "text" => "Text That Contains",
            _ => "New Formatting Rule",
        },
        Dialog::CondManager(_) => "Conditional Formatting Rules Manager",
        Dialog::GoalSeek { .. } => "Goal Seek",
        Dialog::Note { .. } => "Note",
        Dialog::InsertFunction { .. } => "Insert Function",
        Dialog::Hyperlink { .. } => "Insert Hyperlink",
        Dialog::Pivot { .. } => "Create PivotTable",
        Dialog::Sparkline { .. } => "Create Sparklines",
        Dialog::PageSetup(..) => "Page Setup",
        Dialog::Sort { .. } => "Sort",
        Dialog::Dedupe { .. } => "Remove Duplicates",
        Dialog::Validation(..) => "Data Validation",
        Dialog::Subtotal { .. } => "Subtotal",
        Dialog::ChartData { .. } => "Select Data Source",
    }
}

pub fn primary(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(theme::GREEN).min_size(vec2(76.0, 24.0))).clicked()
}

pub fn plain(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Button::new(text).min_size(vec2(76.0, 24.0))).clicked()
}

/// OK and Cancel at the bottom right. Enter means OK.
pub fn ok_cancel(ui: &mut egui::Ui) -> (bool, bool) {
    ui.add_space(8.0);
    let mut ok = false;
    let mut cancel = false;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        cancel = plain(ui, "Cancel");
        ok = primary(ui, "OK");
    });
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
    (ok || enter, cancel)
}

/// A row of tabs like the ones at the top of Format Cells.
pub fn tabs(ui: &mut egui::Ui, names: &[&str], cur: &mut u8) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, n) in names.iter().enumerate() {
            let on = *cur == i as u8;
            let w = ui.fonts_mut(|f| f.layout_no_wrap(n.to_string(), theme::ui_font(12.5), Color32::BLACK).size().x) + 20.0;
            let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
            if on {
                ui.painter().rect(r, 0.0, Color32::WHITE, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
                ui.painter().hline(r.left() + 1.0..=r.right() - 1.0, r.bottom(), Stroke::new(2.0, Color32::WHITE));
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, theme::HOVER);
            }
            ui.painter().text(r.center(), Align2::CENTER_CENTER, *n, theme::ui_font(12.5), theme::TEXT);
            if resp.clicked() {
                *cur = i as u8;
            }
        }
    });
    ui.separator();
}

pub fn field(ui: &mut egui::Ui, s: &mut String, w: f32) -> egui::Response {
    ui.add(TextEdit::singleline(s).desired_width(w))
}

/// A range box with a button that fills it from the current selection.
pub fn range_field(ui: &mut egui::Ui, a: &App, s: &mut String, w: f32, with_sheet: bool) {
    ui.horizontal(|ui| {
        field(ui, s, (w - 30.0).max(40.0));
        if ui.add(egui::Button::new("⬚").min_size(vec2(24.0, 20.0))).on_hover_text("Use the selected cells").clicked() {
            let rc = a.sel().rect();
            *s = if with_sheet { format!("={}!{}", crate::formula::quote_sheet(&a.sheet().name), rc.a1_abs()) } else { rc.a1_abs() };
        }
    });
}

/// Reads a reference such as Sheet1!$A$1:$B$4, A1:B4 or a defined name.
pub fn parse_ref(a: &App, text: &str) -> Option<(usize, Rect)> {
    let t = text.trim().trim_start_matches('=');
    match t.rsplit_once('!') {
        Some((s, b)) => Some((a.wb.sheet_index(s.trim_matches('\''))?, parse_rect(b.trim())?)),
        None => {
            if let Some(n) = a.wb.names.iter().find(|n| n.name.eq_ignore_ascii_case(t)) {
                let e = n.expr.clone();
                if e.trim_start_matches('=') != t {
                    return parse_ref(a, &e);
                }
            }
            Some((a.cur, parse_rect(t)?))
        }
    }
}

fn close_button(ui: &mut egui::Ui) -> Out {
    ui.add_space(6.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| if plain(ui, "Close") { Out::Close } else { Out::Stay }).inner
}

fn done(ok: bool, cancel: bool) -> Out {
    if ok || cancel { Out::Close } else { Out::Stay }
}

pub fn show(a: &mut App, ctx: &egui::Context) {
    let Some(mut d) = a.dialog.take() else { return };
    let screen = ctx.content_rect();
    if d.modal() {
        egui::Area::new(egui::Id::new("dlg-shade")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(20));
        });
    }
    let mut open = true;
    let mut out = Out::Stay;
    let t = title(&d);
    egui::Window::new(RichText::new(t).size(13.0))
        .id(egui::Id::new(("dlg", t)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(screen.center() - vec2(0.0, 60.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            out = body(a, ui, &mut d);
        });
    if a.alert.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        out = Out::Close;
    }
    if !open {
        out = Out::Close;
    }
    // A dialog can open another one in its place (Go To Special from Go To).
    if out == Out::Stay && a.dialog.is_none() {
        a.dialog = Some(d);
    }
}

fn body(a: &mut App, ui: &mut egui::Ui, d: &mut Dialog) -> Out {
    match d {
        Dialog::FormatCells(st) => big::format_cells(a, ui, st),
        Dialog::FindReplace(tab) => big::find_replace(a, ui, tab),
        Dialog::CondManager(sel) => big::cond_manager(a, ui, sel),
        Dialog::InsertFunction { search, cat, sel } => big::insert_function(a, ui, search, cat, sel),
        Dialog::Pivot { source, headers, rows, cols, values, new_sheet, at } => big::pivot(a, ui, source, headers, rows, cols, values, new_sheet, at),
        Dialog::PageSetup(ps, tab) => big::page_setup(a, ui, ps, tab),
        Dialog::Sort { range, header, keys, left_right } => big::sort(a, ui, range, header, keys, left_right),
        Dialog::Validation(v, tab) => big::validation(a, ui, v, tab),
        Dialog::Subtotal { range, at_change, func, cols } => big::subtotal(a, ui, range, at_change, func, cols),
        Dialog::GoTo(text) => go_to(a, ui, text),
        Dialog::GoSpecial(k) => {
            ui.label("Select");
            if *k == 0 {
                *k = 1;
            }
            for (v, label) in [(1u8, "Formulas"), (4, "Constants"), (6, "Blanks"), (2, "Notes"), (3, "Conditional formats"), (5, "Data validation")] {
                ui.radio_value(k, v, label);
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let k = *k;
                dialogs::select_special(a, k);
            }
            done(ok, cancel)
        }
        Dialog::DeleteCells(k) => {
            ui.label("Delete");
            for (v, label) in [(0u8, "Shift cells left"), (1, "Shift cells up"), (2, "Entire row"), (3, "Entire column")] {
                ui.radio_value(k, v, label);
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let k = *k;
                let r = a.sel().rect();
                match k {
                    2 => dialogs::delete_rows_cols(a, true),
                    3 => dialogs::delete_rows_cols(a, false),
                    _ => {
                        if !a.deny_protected(r) {
                            a.act(|a| ops::shift_cells(&mut a.wb, a.cur, r, k == 1, false));
                        }
                    }
                }
            }
            done(ok, cancel)
        }
        Dialog::InsertCells(k) => {
            ui.label("Insert");
            for (v, label) in [(0u8, "Shift cells right"), (1, "Shift cells down"), (2, "Entire row"), (3, "Entire column")] {
                ui.radio_value(k, v, label);
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let k = *k;
                let r = a.sel().rect();
                match k {
                    2 => dialogs::insert_rows_cols(a, true),
                    3 => dialogs::insert_rows_cols(a, false),
                    _ => a.act(|a| ops::shift_cells(&mut a.wb, a.cur, r, k == 1, true)),
                }
            }
            done(ok, cancel)
        }
        Dialog::Zoom(z) => {
            ui.label("Magnification");
            for v in [200u32, 100, 75, 50, 25] {
                ui.radio_value(z, v, format!("{v}%"));
            }
            let fit = ui.radio(false, "Fit selection").clicked();
            ui.horizontal(|ui| {
                ui.label("Custom:");
                ui.add(egui::DragValue::new(z).range(10..=400).suffix("%"));
            });
            if fit {
                dialogs::zoom_to_selection(a);
                return Out::Close;
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let v = *z as f32 / 100.0;
                a.wb.sheet_mut(a.cur).zoom = v.clamp(0.1, 4.0);
            }
            done(ok, cancel)
        }
        Dialog::NotesList => notes_list(a, ui),
        Dialog::Symbol => symbol(a, ui),
        Dialog::NameManager(sel) => name_manager(a, ui, sel),
        Dialog::DefineName { name, refers, comment, editing } => {
            egui::Grid::new("defname").num_columns(2).show(ui, |ui| {
                ui.label("Name:");
                field(ui, name, 260.0);
                ui.end_row();
                ui.label("Scope:");
                ui.label("Workbook");
                ui.end_row();
                ui.label("Comment:");
                ui.add(TextEdit::multiline(comment).desired_width(260.0).desired_rows(3));
                ui.end_row();
                ui.label("Refers to:");
                range_field(ui, a, refers, 260.0, true);
                ui.end_row();
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let n = name.trim().to_string();
                let valid = n.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && n.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') && parse_cell(&n).is_none();
                if !valid {
                    a.alert = Some(("Name".into(), "The name that you entered is not valid. Make sure it starts with a letter or underscore and has no spaces or cell references.".into()));
                    return Out::Stay;
                }
                if a.wb.names.iter().enumerate().any(|(i, x)| x.name.eq_ignore_ascii_case(&n) && Some(i) != *editing) {
                    a.alert = Some(("Name".into(), "The name already exists. Enter a unique name.".into()));
                    return Out::Stay;
                }
                let refers = if refers.starts_with('=') { refers.clone() } else { format!("={refers}") };
                let def = DefinedName { name: n, expr: refers, comment: comment.clone() };
                let ed = *editing;
                a.act(|a| match ed {
                    Some(i) if i < a.wb.names.len() => a.wb.names[i] = def,
                    _ => a.wb.names.push(def),
                });
            }
            done(ok, cancel)
        }
        Dialog::TextToColumns { comma, tab, semicolon, space, other } => {
            ui.label("Choose the characters that separate your data into columns.");
            ui.checkbox(tab, "Tab");
            ui.checkbox(semicolon, "Semicolon");
            ui.checkbox(comma, "Comma");
            ui.checkbox(space, "Space");
            ui.horizontal(|ui| {
                ui.label("Other:");
                field(ui, other, 30.0);
            });
            let r = a.sel().rect();
            if r.cols() > 1 {
                ui.label(RichText::new("Only one column can be converted at a time.").color(theme::RED));
            }
            let (ok, cancel) = ok_cancel(ui);
            if ok && r.cols() == 1 {
                let mut delims = vec![];
                for (on, ch) in [(*tab, '\t'), (*semicolon, ';'), (*comma, ','), (*space, ' ')] {
                    if on {
                        delims.push(ch);
                    }
                }
                delims.extend(other.chars().take(1));
                let r = a.sheet().used_rect().and_then(|u| r.intersection(&u)).unwrap_or(r);
                a.act(|a| ops::text_to_columns(&mut a.wb, a.cur, r, &delims, false));
                return Out::Close;
            }
            if cancel { Out::Close } else { Out::Stay }
        }
        Dialog::Protect { password, select_locked } => {
            ui.label("Password to unprotect sheet (optional):");
            ui.add(TextEdit::singleline(password).password(true).desired_width(260.0));
            ui.checkbox(select_locked, "Allow selecting locked cells");
            ui.label(RichText::new("Locked cells can't be changed while the sheet is protected.").color(theme::TEXT_DIM));
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let pr = Protection { password: password.clone(), select_locked: *select_locked };
                a.act(|a| a.wb.sheet_mut(a.cur).protection = Some(pr));
            }
            done(ok, cancel)
        }
        Dialog::Unprotect(pw) => {
            ui.label("Password:");
            ui.add(TextEdit::singleline(pw).password(true).desired_width(240.0)).request_focus();
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let want = a.sheet().protection.as_ref().map(|p| p.password.clone()).unwrap_or_default();
                if *pw == want {
                    a.act(|a| a.wb.sheet_mut(a.cur).protection = None);
                    return Out::Close;
                }
                a.alert = Some(("Unprotect Sheet".into(), "The password you supplied is not correct. Verify that the CAPS LOCK key is off and be sure to use the correct capitalization.".into()));
                pw.clear();
            }
            if cancel { Out::Close } else { Out::Stay }
        }
        Dialog::Stats => stats(a, ui),
        Dialog::Unhide(sel) => {
            let hidden: Vec<(usize, String)> = a.wb.sheets.iter().enumerate().filter(|(_, s)| s.hidden).map(|(i, s)| (i, s.name.clone())).collect();
            ui.label("Unhide one or more sheets:");
            egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                ui.set_min_size(vec2(260.0, 140.0));
                for (k, (_, n)) in hidden.iter().enumerate() {
                    if ui.selectable_label(*sel == k, n).clicked() {
                        *sel = k;
                    }
                }
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                if let Some(&(i, _)) = hidden.get(*sel) {
                    a.act(|a| a.wb.sheet_mut(i).hidden = false);
                    a.switch_sheet(i);
                }
            }
            done(ok, cancel)
        }
        Dialog::Shortcuts => shortcuts(ui),
        Dialog::PasteSpecial(o) => {
            ui.columns(2, |cols| {
                cols[0].label(RichText::new("Paste").strong());
                for (v, l) in [(PasteWhat::All, "All"), (PasteWhat::Formulas, "Formulas"), (PasteWhat::Values, "Values"), (PasteWhat::Formats, "Formats"), (PasteWhat::ValuesFormats, "Values and number formats"), (PasteWhat::NoBorders, "All except borders"), (PasteWhat::Widths, "Column widths")] {
                    cols[0].radio_value(&mut o.what, v, l);
                }
                cols[1].label(RichText::new("Operation").strong());
                for (v, l) in [(PasteOp::None, "None"), (PasteOp::Add, "Add"), (PasteOp::Sub, "Subtract"), (PasteOp::Mul, "Multiply"), (PasteOp::Div, "Divide")] {
                    cols[1].radio_value(&mut o.op, v, l);
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut o.skip_blanks, "Skip blanks");
                ui.checkbox(&mut o.transpose, "Transpose");
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let o = *o;
                if a.clip.is_none() {
                    a.alert = Some(("Paste Special".into(), "Copy some cells first.".into()));
                } else {
                    a.paste_clip(o);
                }
            }
            done(ok, cancel)
        }
        Dialog::MoreColors { fill, color } => {
            ui.label(if *fill { "Choose a fill color:" } else { "Choose a font color:" });
            let mut c = Color32::from_rgb(color[0], color[1], color[2]);
            egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque);
            *color = [c.r(), c.g(), c.b()];
            ui.horizontal(|ui| {
                ui.label("Hex:");
                let mut hex = format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                if field(ui, &mut hex, 80.0).changed() {
                    if let Ok(v) = u32::from_str_radix(hex.trim_start_matches('#'), 16) {
                        *color = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
                    }
                }
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let v = (color[0] as u32) << 16 | (color[1] as u32) << 8 | color[2] as u32;
                if *fill {
                    a.format(move |s| s.fill = Some(v));
                } else {
                    a.format(move |s| s.color = Some(v));
                }
            }
            done(ok, cancel)
        }
        Dialog::CreateTable { range, header, style } => create_table(a, ui, range, header, style),
        Dialog::Size { rows, value } => {
            ui.horizontal(|ui| {
                ui.label(if *rows { "Row height:" } else { "Column width:" });
                field(ui, value, 80.0).request_focus();
            });
            let (ok, cancel) = ok_cancel(ui);
            if ok {
                let Ok(v) = value.trim().parse::<f32>() else {
                    a.alert = Some(("Size".into(), "Enter a number.".into()));
                    return Out::Stay;
                };
                let r = a.sel().rect();
                let rows = *rows;
                a.act(|a| {
                    let sh = a.wb.sheet_mut(a.cur);
                    if rows {
                        let px = (v / 0.75).clamp(0.0, 546.0);
                        for k in r.r1..=r.r2.min(r.r1 + 100_000) {
                            if px == 0.0 {
                                sh.hidden_rows.insert(k);
                            } else {
                                sh.row_h.insert(k, px);
                                sh.custom_h.insert(k);
                            }
                        }
                    } else {
                        let px = if v <= 0.0 { 0.0 } else { (v * 7.0 + 5.0).round().min(1790.0) };
                        for k in r.c1..=r.c2 {
                            if px == 0.0 {
                                sh.hidden_cols.insert(k);
                            } else {
                                sh.col_w.insert(k, px);
                            }
                        }
                    }
                });
            }
            done(ok, cancel)
        }
        Dialog::MoveCopy { to, copy } => move_copy(a, ui, to, copy),
        Dialog::Series { rows, step, stop, kind } => series(a, ui, rows, step, stop, kind),
        Dialog::CondQuick { op, a: va, b: vb, style } => cond_quick(a, ui, op, va, vb, style),
        Dialog::GoalSeek { set, to, change } => goal_seek(a, ui, set, to, change),
        Dialog::Note { pos, text } => {
            ui.label(RichText::new(format!("{}:", a.wb.author)).strong());
            ui.add(TextEdit::multiline(text).desired_width(260.0).desired_rows(5)).request_focus();
            ui.add_space(6.0);
            let mut r = (false, false);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                r.1 = plain(ui, "Cancel");
                r.0 = primary(ui, "OK");
            });
            if r.0 {
                let (p, t, author) = (*pos, text.clone(), a.wb.author.clone());
                a.act(|a| {
                    let sh = a.wb.sheet_mut(a.cur);
                    if t.trim().is_empty() {
                        sh.notes.remove(&p);
                    } else {
                        sh.notes.insert(p, Note { text: t, author });
                    }
                });
            }
            done(r.0, r.1)
        }
        Dialog::Hyperlink { pos, text, addr } => hyperlink(a, ui, pos, text, addr),
        Dialog::Sparkline { kind, data, loc } => sparkline(a, ui, kind, data, loc),
        Dialog::Dedupe { range, header, cols } => dedupe(a, ui, range, header, cols),
        Dialog::ChartData { i, title, range, x_title, y_title } => chart_data(a, ui, *i, title, range, x_title, y_title),
    }
}

fn go_to(a: &mut App, ui: &mut egui::Ui, text: &mut String) -> Out {
    ui.label("Go to:");
    let names: Vec<(String, String)> = a.wb.names.iter().map(|n| (n.name.clone(), n.expr.clone())).collect();
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        ui.set_min_size(vec2(280.0, 140.0));
        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
            for (n, e) in &names {
                if ui.selectable_label(text == n, n).on_hover_text(e).clicked() {
                    *text = n.clone();
                }
            }
        });
    });
    ui.label("Reference:");
    field(ui, text, 280.0).request_focus();
    ui.add_space(6.0);
    let mut r = (false, false, false);
    ui.horizontal(|ui| {
        r.2 = plain(ui, "Special...");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r.1 = plain(ui, "Cancel");
            r.0 = primary(ui, "OK");
        });
    });
    if r.2 {
        a.dialog = Some(Dialog::GoSpecial(0));
        return Out::Close;
    }
    if r.0 || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        let t = text.clone();
        dialogs::go_to(a, &t, false);
        return Out::Close;
    }
    if r.1 { Out::Close } else { Out::Stay }
}

fn notes_list(a: &mut App, ui: &mut egui::Ui) -> Out {
    let notes: Vec<(Pos, String, String)> = a.sheet().notes.iter().map(|(p, n)| (*p, n.author.clone(), n.text.clone())).collect();
    if notes.is_empty() {
        ui.label("There are no notes on this sheet.");
    }
    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
        ui.set_min_width(340.0);
        for (p, author, text) in notes {
            egui::Frame::new().fill(Color32::from_rgb(0xFF, 0xFF, 0xE1)).stroke(Stroke::new(1.0, theme::BORDER)).inner_margin(6.0).show(ui, |ui| {
                ui.set_width(320.0);
                ui.horizontal(|ui| {
                    if ui.link(RichText::new(cell_name(p.row, p.col)).strong()).clicked() {
                        a.select(p, false);
                        a.reveal(p);
                    }
                    ui.label(RichText::new(author).color(theme::TEXT_DIM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Delete").clicked() {
                            a.act(|a| {
                                a.wb.sheet_mut(a.cur).notes.remove(&p);
                            });
                        }
                        if ui.small_button("Edit").clicked() {
                            a.dialog = Some(Dialog::Note { pos: p, text: text.clone() });
                        }
                    });
                });
                ui.label(&text);
            });
        }
    });
    if a.dialog.is_some() {
        return Out::Close;
    }
    close_button(ui)
}

fn symbol(a: &mut App, ui: &mut egui::Ui) -> Out {
    const SYMS: &str = "©®™°±×÷µ¶§•…‰€£¥¢¤←↑→↓↔↕⇐⇒⇔√∞≈≠≤≥∑∏∫∂∆∇∈∉∩∪⊂⊃αβγδεζηθλμπσφψωΩΣΠΔ½⅓¼¾⅔⅛¹²³★☆♠♣♥♦✓✔✗✘☐☑☒●○■□▲△▼▽◆◇";
    ui.label("Click a symbol to insert it into the active cell.");
    let mut pick = None;
    egui::Grid::new("syms").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for (i, ch) in SYMS.chars().enumerate() {
            if ui.add(egui::Button::new(RichText::new(ch.to_string()).size(16.0)).min_size(vec2(28.0, 28.0))).clicked() {
                pick = Some(ch);
            }
            if i % 16 == 15 {
                ui.end_row();
            }
        }
    });
    if let Some(ch) = pick {
        if a.edit.is_none() {
            a.start_edit(None, false);
        }
        if let Some(e) = a.edit.as_mut() {
            let mut cs: Vec<char> = e.text.chars().collect();
            let at = e.caret.min(cs.len());
            cs.insert(at, ch);
            e.text = cs.into_iter().collect();
            e.caret = at + 1;
            e.set_caret = Some(at + 1);
            e.focus = true;
        }
    }
    close_button(ui)
}

fn name_manager(a: &mut App, ui: &mut egui::Ui, sel: &mut Option<usize>) -> Out {
    let names = a.wb.names.clone();
    ui.horizontal(|ui| {
        if plain(ui, "New...") {
            a.dialog = Some(Dialog::define_name(a));
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Edit...").min_size(vec2(76.0, 24.0))).clicked() {
            if let Some(n) = sel.and_then(|i| names.get(i)) {
                a.dialog = Some(Dialog::DefineName { name: n.name.clone(), refers: n.expr.clone(), comment: n.comment.clone(), editing: *sel });
            }
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Delete").min_size(vec2(76.0, 24.0))).clicked() {
            if let Some(i) = *sel {
                a.act(|a| {
                    if i < a.wb.names.len() {
                        a.wb.names.remove(i);
                    }
                });
                *sel = None;
            }
        }
    });
    if a.dialog.is_some() {
        return Out::Close;
    }
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        ui.set_min_size(vec2(520.0, 200.0));
        egui::Grid::new("names").striped(true).num_columns(3).min_col_width(120.0).show(ui, |ui| {
            ui.label(RichText::new("Name").strong());
            ui.label(RichText::new("Refers To").strong());
            ui.label(RichText::new("Comment").strong());
            ui.end_row();
            for (i, n) in names.iter().enumerate() {
                if ui.selectable_label(*sel == Some(i), &n.name).clicked() {
                    *sel = Some(i);
                }
                ui.label(&n.expr);
                ui.label(&n.comment);
                ui.end_row();
            }
        });
    });
    close_button(ui)
}

fn stats(a: &mut App, ui: &mut egui::Ui) -> Out {
    fn count(sh: &Sheet, f: &dyn Fn(&Cell) -> bool) -> usize {
        sh.rows.values().flat_map(|r| r.values()).filter(|c| f(c)).count()
    }
    let sh = a.sheet();
    let all = |f: &dyn Fn(&Sheet) -> usize| a.wb.sheets.iter().map(|s| f(s)).sum::<usize>().to_string();
    let data = |c: &Cell| c.v != Content::Empty;
    let formula = |c: &Cell| matches!(c.v, Content::Formula(_));
    let rows: Vec<(&str, String, String)> = vec![
        ("End of sheet", sh.used_rect().map(|u| cell_name(u.r2, u.c2)).unwrap_or("A1".into()), String::new()),
        ("Cells with data", count(sh, &data).to_string(), all(&|s| count(s, &data))),
        ("Formulas", count(sh, &formula).to_string(), all(&|s| count(s, &formula))),
        ("Tables", sh.tables.len().to_string(), all(&|s| s.tables.len())),
        ("PivotTables", sh.pivots.len().to_string(), all(&|s| s.pivots.len())),
        ("Charts", sh.charts.len().to_string(), all(&|s| s.charts.len())),
        ("Notes", sh.notes.len().to_string(), all(&|s| s.notes.len())),
        ("Sheets", String::new(), a.wb.sheets.len().to_string()),
        ("Defined names", String::new(), a.wb.names.len().to_string()),
    ];
    egui::Grid::new("stats").striped(true).num_columns(3).min_col_width(90.0).show(ui, |ui| {
        ui.label("");
        ui.label(RichText::new("Current sheet").strong());
        ui.label(RichText::new("Workbook").strong());
        ui.end_row();
        for (k, v1, v2) in rows {
            ui.label(k);
            ui.label(v1);
            ui.label(v2);
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| if primary(ui, "OK") { Out::Close } else { Out::Stay }).inner
}

fn shortcuts(ui: &mut egui::Ui) -> Out {
    const KEYS: [(&str, &str); 30] = [
        ("Ctrl+N", "New workbook"),
        ("Ctrl+O", "Open"),
        ("Ctrl+S", "Save"),
        ("F12", "Save As"),
        ("Ctrl+P", "Print"),
        ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
        ("Ctrl+C / X / V", "Copy / Cut / Paste"),
        ("Ctrl+Alt+V", "Paste Special"),
        ("Ctrl+F / Ctrl+H", "Find / Replace"),
        ("Ctrl+G or F5", "Go To"),
        ("F2", "Edit the active cell"),
        ("F4", "Toggle absolute reference"),
        ("Ctrl+1", "Format Cells"),
        ("Ctrl+B / I / U", "Bold / Italic / Underline"),
        ("Ctrl+5", "Strikethrough"),
        ("Ctrl+Shift+$", "Currency format"),
        ("Ctrl+Shift+%", "Percentage format"),
        ("Alt+=", "AutoSum"),
        ("Ctrl+;", "Insert today's date"),
        ("Ctrl+Shift+:", "Insert the current time"),
        ("Ctrl+D / Ctrl+R", "Fill down / right"),
        ("Ctrl+Enter", "Fill the selection with the entry"),
        ("Alt+Enter", "New line in a cell"),
        ("Ctrl+Arrow", "Move to the edge of data"),
        ("Ctrl+Space / Shift+Space", "Select column / row"),
        ("Ctrl+A", "Select all"),
        ("Ctrl+PgUp / PgDn", "Previous / next sheet"),
        ("Ctrl+Shift+L", "Turn filters on or off"),
        ("Ctrl+T", "Create table"),
        ("Shift+F11", "Insert a new sheet"),
    ];
    egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
        egui::Grid::new("keys").striped(true).num_columns(2).min_col_width(160.0).show(ui, |ui| {
            for (k, v) in KEYS {
                ui.label(RichText::new(k).strong());
                ui.label(v);
                ui.end_row();
            }
        });
    });
    close_button(ui)
}

fn create_table(a: &mut App, ui: &mut egui::Ui, range: &mut String, header: &mut bool, style: &mut u8) -> Out {
    ui.label("Where is the data for your table?");
    range_field(ui, a, range, 260.0, false);
    ui.checkbox(header, "My table has headers");
    let (ok, cancel) = ok_cancel(ui);
    if !ok {
        return done(false, cancel);
    }
    let Some((_, mut r)) = parse_ref(a, range) else {
        a.alert = Some(("Create Table".into(), "The reference isn't valid.".into()));
        return Out::Stay;
    };
    let st = *style;
    if let Some(i) = a.sheet().tables.iter().position(|t| t.range == r) {
        a.act(|a| a.wb.sheet_mut(a.cur).tables[i].style = st);
        return Out::Close;
    }
    if a.sheet().tables.iter().any(|t| t.range.intersects(&r)) {
        a.alert = Some(("Create Table".into(), "A table can't overlap another table.".into()));
        return Out::Stay;
    }
    let hdr = *header;
    a.act(|a| {
        let si = a.cur;
        if !hdr {
            ops::shift_cells(&mut a.wb, si, Rect::new(r.r1, r.c1, r.r1, r.c2), true, true);
            for c in r.c1..=r.c2 {
                let _ = ops::set_input(&mut a.wb, si, r.r1, c, &format!("Column{}", c - r.c1 + 1));
            }
            r.r2 += 1;
        }
        let columns: Vec<String> = (r.c1..=r.c2)
            .map(|c| a.wb.sheets[si].get(r.r1, c).map(|x| match &x.v {
                Content::Text(t) => t.to_string(),
                Content::Num(n) => crate::eval::num_text(*n),
                _ => String::new(),
            }).unwrap_or_default())
            .enumerate()
            .map(|(i, t)| if t.is_empty() { format!("Column{}", i + 1) } else { t })
            .collect();
        let name = (1..).map(|k| format!("Table{k}")).find(|n| !a.wb.sheets.iter().any(|s| s.tables.iter().any(|t| &t.name == n))).unwrap_or_default();
        let sh = a.wb.sheet_mut(si);
        sh.tables.push(TableDef { name, range: r, style: st, header: true, totals: false, banded: true, columns });
        if sh.filter.is_none() {
            sh.filter = Some(AutoFilter { range: r, cols: Default::default() });
        }
    });
    a.tab = crate::app::RTab::TableDesign;
    Out::Close
}

fn move_copy(a: &mut App, ui: &mut egui::Ui, to: &mut usize, copy: &mut bool) -> Out {
    ui.label(format!("Move selected sheet \"{}\"", a.sheet().name));
    ui.label("Before sheet:");
    let names: Vec<String> = a.wb.sheets.iter().map(|s| s.name.clone()).collect();
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        ui.set_min_size(vec2(260.0, 150.0));
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            for (i, n) in names.iter().enumerate() {
                if ui.selectable_label(*to == i, n).clicked() {
                    *to = i;
                }
            }
            if ui.selectable_label(*to >= names.len(), "(move to end)").clicked() {
                *to = names.len();
            }
        });
    });
    ui.checkbox(copy, "Create a copy");
    let (ok, cancel) = ok_cancel(ui);
    if ok {
        let (to, copy) = (*to, *copy);
        let from = a.cur;
        let sels = a.sels.clone();
        a.act(|a| {
            let to = to.min(a.wb.sheets.len());
            if copy {
                let mut s = (*a.wb.sheets[from]).clone();
                let stem = s.name.clone();
                let mut k = 2;
                while a.wb.sheet_index(&format!("{stem} ({k})")).is_some() {
                    k += 1;
                }
                s.name = format!("{stem} ({k})");
                // Table names must stay unique across the workbook.
                s.tables.clear();
                a.wb.sheets.insert(to, Arc::new(s));
                a.cur = to;
            } else {
                let s = a.wb.sheets.remove(from);
                let at = if to > from { to - 1 } else { to };
                a.wb.sheets.insert(at, s);
                a.cur = at;
            }
        });
        let n = a.wb.sheets.len();
        a.sels = vec![crate::app::Sel::cell(Pos::new(0, 0)); n];
        a.sels[a.cur] = sels[from].clone();
        a.views = vec![(0, 0); n];
    }
    done(ok, cancel)
}

fn series(a: &mut App, ui: &mut egui::Ui, rows: &mut bool, step: &mut String, stop: &mut String, kind: &mut u8) -> Out {
    ui.columns(2, |cols| {
        cols[0].label(RichText::new("Series in").strong());
        cols[0].radio_value(rows, true, "Rows");
        cols[0].radio_value(rows, false, "Columns");
        cols[1].label(RichText::new("Type").strong());
        cols[1].radio_value(kind, 0, "Linear");
        cols[1].radio_value(kind, 1, "Growth");
    });
    egui::Grid::new("series").num_columns(2).show(ui, |ui| {
        ui.label("Step value:");
        field(ui, step, 100.0);
        ui.end_row();
        ui.label("Stop value:");
        field(ui, stop, 100.0);
        ui.end_row();
    });
    let (ok, cancel) = ok_cancel(ui);
    if !ok {
        return done(false, cancel);
    }
    let st: f64 = step.trim().parse().unwrap_or(1.0);
    let stop_v: Option<f64> = stop.trim().parse().ok();
    let (by_rows, growth) = (*rows, *kind == 1);
    let mut r = a.sel().rect();
    let Value::Num(start) = a.calc.value(&a.wb, a.cur, r.r1, r.c1) else {
        a.alert = Some(("Series".into(), "Type a starting number in the first cell.".into()));
        return Out::Stay;
    };
    if r.is_single() {
        if let Some(sv) = stop_v {
            let n = if growth {
                if st > 1.0 && start > 0.0 { ((sv / start).ln() / st.ln()).floor().max(0.0) as u32 } else { 0 }
            } else if st != 0.0 {
                ((sv - start) / st).floor().max(0.0) as u32
            } else {
                0
            };
            let n = n.min(10_000);
            if by_rows {
                r.c2 = r.c1 + n;
            } else {
                r.r2 = r.r1 + n;
            }
        }
    }
    a.act(|a| {
        let lines: Vec<(u32, u32)> = if by_rows { (r.r1..=r.r2).map(|x| (x, r.c1)).collect() } else { (r.c1..=r.c2).map(|x| (r.r1, x)).collect() };
        let len = if by_rows { r.cols() } else { r.rows() };
        for (lr, lc) in lines {
            let Value::Num(mut v) = a.calc.value(&a.wb, a.cur, lr, lc) else { continue };
            for k in 1..len {
                v = if growth { v * st } else { v + st };
                if stop_v.is_some_and(|s| if st >= 0.0 { v > s + 1e-9 } else { v < s - 1e-9 }) {
                    break;
                }
                let (rr, cc) = if by_rows { (lr, lc + k) } else { (lr + k, lc) };
                a.wb.sheet_mut(a.cur).set_content(rr, cc, Content::Num(v));
            }
        }
    });
    Out::Close
}

fn cond_quick(a: &mut App, ui: &mut egui::Ui, op: &mut String, va: &mut String, vb: &mut String, style: &mut u8) -> Out {
    const STYLES: [(&str, Option<u32>, Option<u32>); 6] = [
        ("Light Red Fill with Dark Red Text", Some(0xFFC7CE), Some(0x9C0006)),
        ("Yellow Fill with Dark Yellow Text", Some(0xFFEB9C), Some(0x9C5700)),
        ("Green Fill with Dark Green Text", Some(0xC6EFCE), Some(0x006100)),
        ("Light Red Fill", Some(0xFFC7CE), None),
        ("Red Text", None, Some(0x9C0006)),
        ("Bold Text", None, None),
    ];
    let label = match op.as_str() {
        ">" => "Format cells that are GREATER THAN:",
        "<" => "Format cells that are LESS THAN:",
        "between" => "Format cells that are BETWEEN:",
        "=" => "Format cells that are EQUAL TO:",
        "text" => "Format cells that contain the text:",
        _ => "Format values where this formula is true:",
    };
    ui.label(label);
    ui.horizontal(|ui| {
        range_field(ui, a, va, if op == "formula" { 300.0 } else { 150.0 }, false);
        if op == "between" {
            ui.label("and");
            range_field(ui, a, vb, 150.0, false);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Format with:");
        ComboBox::from_id_salt("cq-style").width(260.0).selected_text(STYLES[*style as usize % 6].0).show_ui(ui, |ui| {
            for (i, s) in STYLES.iter().enumerate() {
                ui.selectable_value(style, i as u8, s.0);
            }
        });
    });
    let (ok, cancel) = ok_cancel(ui);
    if ok {
        let s = STYLES[*style as usize % 6];
        let cs = CondStyle { fill: s.1, color: s.2, bold: *style == 5, italic: false };
        let rule = match op.as_str() {
            "text" => CondRule::TextContains { text: va.clone(), mode: TextMode::Contains },
            "formula" => CondRule::Formula { formula: va.trim_start_matches('=').to_string() },
            o => CondRule::CellIs { op: o.to_string(), a: va.trim_start_matches('=').to_string(), b: vb.trim_start_matches('=').to_string() },
        };
        let ranges = a.sel().ranges.clone();
        a.act(|a| a.wb.sheet_mut(a.cur).cond.push(CondFormat { ranges, rule, style: cs, stop_if_true: false }));
    }
    done(ok, cancel)
}

fn goal_seek(a: &mut App, ui: &mut egui::Ui, set: &mut String, to: &mut String, change: &mut String) -> Out {
    egui::Grid::new("gs").num_columns(2).show(ui, |ui| {
        ui.label("Set cell:");
        range_field(ui, a, set, 160.0, false);
        ui.end_row();
        ui.label("To value:");
        field(ui, to, 160.0);
        ui.end_row();
        ui.label("By changing cell:");
        range_field(ui, a, change, 160.0, false);
        ui.end_row();
    });
    let (ok, cancel) = ok_cancel(ui);
    if !ok {
        return done(false, cancel);
    }
    let cell = |s: &str| parse_cell(&s.trim().trim_start_matches('=').replace('$', ""));
    let (Some(sp), Some(cp), Ok(target)) = (cell(set), cell(change), to.trim().parse::<f64>()) else {
        a.alert = Some(("Goal Seek".into(), "Enter a cell for Set cell and By changing cell, and a number for To value.".into()));
        return Out::Stay;
    };
    if !matches!(a.sheet().get(sp.row, sp.col).map(|c| &c.v), Some(Content::Formula(_))) {
        a.alert = Some(("Goal Seek".into(), "Set cell must contain a formula.".into()));
        return Out::Stay;
    }
    let mut res = None;
    a.act(|a| res = ops::goal_seek(&mut a.wb, a.cur, sp, target, cp));
    let name = cell_name(sp.row, sp.col);
    a.alert = Some(match res {
        Some(v) => ("Goal Seek Status".into(), format!("Goal Seeking with Cell {name} found a solution.\n\nTarget value: {target}\nCurrent value: {}", crate::eval::num_text(v))),
        None => ("Goal Seek Status".into(), format!("Goal Seeking with Cell {name} may not have found a solution.")),
    });
    Out::Close
}

fn hyperlink(a: &mut App, ui: &mut egui::Ui, pos: &mut Pos, text: &mut String, addr: &mut String) -> Out {
    egui::Grid::new("hl").num_columns(2).show(ui, |ui| {
        ui.label("Text to display:");
        field(ui, text, 300.0);
        ui.end_row();
        ui.label("Address:");
        field(ui, addr, 300.0);
        ui.end_row();
    });
    ui.label(RichText::new("A web address (https://...), an email (mailto:...), or a place in this workbook such as Sheet2!A1.").color(theme::TEXT_DIM).size(11.5));
    let mut r = (false, false, false);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        r.2 = a.sheet().links.contains_key(pos) && plain(ui, "Remove Link");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r.1 = plain(ui, "Cancel");
            r.0 = primary(ui, "OK");
        });
    });
    let p = *pos;
    if r.2 {
        a.act(|a| {
            a.wb.sheet_mut(a.cur).links.remove(&p);
        });
        return Out::Close;
    }
    if (r.0 || ui.input(|i| i.key_pressed(egui::Key::Enter))) && !addr.trim().is_empty() {
        let t = text.clone();
        let mut u = addr.trim().to_string();
        if u.contains('@') && !u.contains(':') {
            u = format!("mailto:{u}");
        } else if u.starts_with("www.") {
            u = format!("https://{u}");
        }
        a.act(|a| {
            let si = a.cur;
            let shown = if t.is_empty() { u.clone() } else { t };
            if ops::text_of(&a.wb, &a.calc, si, p.row, p.col) != shown {
                let _ = ops::set_input(&mut a.wb, si, p.row, p.col, &shown);
            }
            ops::style_ranges(&mut a.wb, si, &[Rect::cell(p)], &|s| {
                s.underline = 1;
                s.color = Some(0x0563C1);
            });
            a.wb.sheet_mut(si).links.insert(p, u);
        });
        return Out::Close;
    }
    if r.1 { Out::Close } else { Out::Stay }
}

fn sparkline(a: &mut App, ui: &mut egui::Ui, kind: &mut u8, data: &mut String, loc: &mut String) -> Out {
    ui.label("Choose the data that you want:");
    ui.horizontal(|ui| {
        ui.label("Data Range:");
        range_field(ui, a, data, 220.0, false);
    });
    ui.label("Choose where you want the sparklines to be placed:");
    ui.horizontal(|ui| {
        ui.label("Location Range:");
        range_field(ui, a, loc, 220.0, false);
    });
    ui.horizontal(|ui| {
        ui.radio_value(kind, 0, "Line");
        ui.radio_value(kind, 1, "Column");
    });
    let (ok, cancel) = ok_cancel(ui);
    if !ok {
        return done(false, cancel);
    }
    let (Some((_, dr)), Some((_, lr))) = (parse_ref(a, data), parse_ref(a, loc)) else {
        a.alert = Some(("Create Sparklines".into(), "The data or location reference isn't valid.".into()));
        return Out::Stay;
    };
    let mut out: Vec<(Pos, String)> = vec![];
    if lr.is_single() {
        out.push((Pos::new(lr.r1, lr.c1), dr.a1()));
    } else if lr.cols() == 1 && lr.rows() == dr.rows() {
        for (k, r) in (lr.r1..=lr.r2).enumerate() {
            out.push((Pos::new(r, lr.c1), Rect::new(dr.r1 + k as u32, dr.c1, dr.r1 + k as u32, dr.c2).a1()));
        }
    } else if lr.rows() == 1 && lr.cols() == dr.cols() {
        for (k, c) in (lr.c1..=lr.c2).enumerate() {
            out.push((Pos::new(lr.r1, c), Rect::new(dr.r1, dr.c1 + k as u32, dr.r2, dr.c1 + k as u32).a1()));
        }
    } else {
        a.alert = Some(("Create Sparklines".into(), "Location reference is not valid: the cells must be in one row or column and match the size of the data.".into()));
        return Out::Stay;
    }
    let k = *kind;
    a.act(|a| {
        let sh = a.wb.sheet_mut(a.cur);
        for (p, s) in out {
            sh.sparks.insert(p, Sparkline { kind: k, source: s, color: 0x4472C4 });
        }
    });
    Out::Close
}

fn dedupe(a: &mut App, ui: &mut egui::Ui, range: &mut Rect, header: &mut bool, cols: &mut [bool]) -> Out {
    ui.label("To delete duplicate values, select one or more columns that contain duplicates.");
    ui.horizontal(|ui| {
        if ui.button("Select All").clicked() {
            cols.iter_mut().for_each(|c| *c = true);
        }
        if ui.button("Unselect All").clicked() {
            cols.iter_mut().for_each(|c| *c = false);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(header, "My data has headers");
        });
    });
    let rg = *range;
    let hdr = *header;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        ui.set_min_size(vec2(320.0, 120.0));
        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
            for (k, on) in cols.iter_mut().enumerate() {
                let c = rg.c1 + k as u32;
                let name = if hdr { ops::text_of(&a.wb, &a.calc, a.cur, rg.r1, c) } else { format!("Column {}", col_name(c)) };
                ui.checkbox(on, name);
            }
        });
    });
    let (ok, cancel) = ok_cancel(ui);
    if !ok {
        return done(false, cancel);
    }
    let keys: Vec<u32> = cols.iter().enumerate().filter(|(_, on)| **on).map(|(k, _)| rg.c1 + k as u32).collect();
    if keys.is_empty() {
        a.alert = Some(("Remove Duplicates".into(), "Select at least one column.".into()));
        return Out::Stay;
    }
    let mut r = rg;
    if hdr {
        r.r1 += 1;
    }
    let total = r.rows() as usize;
    let mut n = 0;
    a.act(|a| {
        let calc = a.calc.clone();
        n = ops::remove_duplicates(&mut a.wb, &calc, a.cur, r, &keys);
    });
    a.alert = Some(("Remove Duplicates".into(), if n == 0 { "No duplicate values found.".into() } else { format!("{n} duplicate values found and removed; {} unique values remain.", total.saturating_sub(n)) }));
    Out::Close
}

fn chart_data(a: &mut App, ui: &mut egui::Ui, i: usize, title: &mut String, range: &mut String, x_title: &mut String, y_title: &mut String) -> Out {
    if i >= a.sheet().charts.len() {
        return Out::Close;
    }
    egui::Grid::new("cd").num_columns(2).show(ui, |ui| {
        ui.label("Chart data range:");
        range_field(ui, a, range, 300.0, true);
        ui.end_row();
        ui.label("Chart title:");
        field(ui, title, 300.0);
        ui.end_row();
        ui.label("Horizontal axis title:");
        field(ui, x_title, 300.0);
        ui.end_row();
        ui.label("Vertical axis title:");
        field(ui, y_title, 300.0);
        ui.end_row();
    });
    let mut r = (false, false, false);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        r.2 = plain(ui, "Switch Row/Column");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r.1 = plain(ui, "Cancel");
            r.0 = primary(ui, "OK");
        });
    });
    if r.2 {
        crate::charts::switch_rows_cols(a, i);
        *range = crate::charts::data_range_text(&a.sheet().charts[i]);
    }
    if r.0 {
        let old = crate::charts::data_range_text(&a.sheet().charts[i]);
        if range.trim() != old && !crate::charts::set_range(a, i, range, None) {
            a.alert = Some(("Select Data Source".into(), "The reference isn't valid.".into()));
            return Out::Stay;
        }
        let (t, x, y) = (title.clone(), x_title.clone(), y_title.clone());
        a.act(|a| {
            let ch = &mut a.wb.sheet_mut(a.cur).charts[i];
            ch.title = t;
            ch.x_title = x;
            ch.y_title = y;
        });
    }
    done(r.0, r.1)
}
