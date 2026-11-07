//! The larger dialogs: Format Cells, Find and Replace, rules, functions, pivots, page setup,
//! sort, validation and subtotals.

use crate::app::App;
use crate::dialog_ui::{field, ok_cancel, parse_ref, plain, primary, range_field, tabs, Out};
use crate::dialogs::{self, Dialog, FmtState};
use crate::eval::Value;
use crate::model::*;
use crate::ops;
use crate::theme;
use egui::{vec2, Align2, Color32, ComboBox, RichText, Sense, Stroke, StrokeKind, TextEdit};
use std::sync::Arc;

fn style_preview(ui: &mut egui::Ui, st: &Style, text: &str, size: egui::Vec2) {
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(r, 0.0, st.fill.map(theme::rgb).unwrap_or(Color32::WHITE), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    let font = egui::FontId::new(theme::cell_px(st.size10, 1.0).min(28.0), theme::cell_family(&st.font, st.bold, st.italic));
    let color = st.color.map(theme::rgb).unwrap_or(Color32::BLACK);
    let g = p.layout_no_wrap(text.to_string(), font, color);
    let at = r.center() - g.size() / 2.0;
    let w = g.size().x;
    let h = g.size().y;
    p.galley(at, g, color);
    if st.underline > 0 {
        p.hline(at.x..=at.x + w, at.y + h - 2.0, Stroke::new(1.0, color));
    }
    if st.strike {
        p.hline(at.x..=at.x + w, at.y + h / 2.0, Stroke::new(1.0, color));
    }
}

fn color_button(ui: &mut egui::Ui, id: &str, c: &mut Option<u32>, auto_label: &str) {
    let shown = c.map(theme::rgb).unwrap_or(Color32::BLACK);
    let resp = ui.add(egui::Button::new(RichText::new("■■■■■■").color(shown)).min_size(vec2(120.0, 22.0)));
    let popup_id = ui.make_persistent_id(id);
    egui::Popup::from_toggle_button_response(&resp).id(popup_id).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(240.0);
        if let Some(v) = crate::menus::palette(ui, auto_label) {
            *c = v;
            ui.close();
        }
    });
}

pub fn format_cells(a: &mut App, ui: &mut egui::Ui, st: &mut FmtState) -> Out {
    tabs(ui, &["Number", "Alignment", "Font", "Border", "Fill", "Protection"], &mut st.tab);
    ui.allocate_ui(vec2(560.0, 330.0), |ui| {
        ui.set_min_size(vec2(560.0, 330.0));
        match st.tab {
            0 => number_tab(ui, st),
            1 => {
                egui::Grid::new("align").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
                    ui.label(RichText::new("Text alignment").strong());
                    ui.end_row();
                    ui.label("Horizontal:");
                    let hs = [(HAlign::General, "General"), (HAlign::Left, "Left (Indent)"), (HAlign::Center, "Center"), (HAlign::Right, "Right (Indent)"), (HAlign::Fill, "Fill"), (HAlign::Justify, "Justify"), (HAlign::CenterAcross, "Center Across Selection")];
                    ComboBox::from_id_salt("h").width(200.0).selected_text(hs.iter().find(|x| x.0 == st.style.h).map_or("General", |x| x.1)).show_ui(ui, |ui| {
                        for (v, l) in hs {
                            ui.selectable_value(&mut st.style.h, v, l);
                        }
                    });
                    ui.end_row();
                    ui.label("Vertical:");
                    let vs = [(VAlign::Top, "Top"), (VAlign::Center, "Center"), (VAlign::Bottom, "Bottom")];
                    ComboBox::from_id_salt("v").width(200.0).selected_text(vs.iter().find(|x| x.0 == st.style.v).map_or("Bottom", |x| x.1)).show_ui(ui, |ui| {
                        for (v, l) in vs {
                            ui.selectable_value(&mut st.style.v, v, l);
                        }
                    });
                    ui.end_row();
                    ui.label("Indent:");
                    ui.add(egui::DragValue::new(&mut st.style.indent).range(0..=15));
                    ui.end_row();
                    ui.label(RichText::new("Text control").strong());
                    ui.end_row();
                    ui.checkbox(&mut st.style.wrap, "Wrap text");
                    ui.end_row();
                    ui.checkbox(&mut st.style.shrink, "Shrink to fit");
                    ui.end_row();
                    ui.label(RichText::new("Orientation").strong());
                    ui.end_row();
                    ui.label("Degrees:");
                    ui.add(egui::DragValue::new(&mut st.style.rotation).range(-90..=90));
                    ui.end_row();
                });
            }
            2 => {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Font:");
                        let mut f = st.style.font.to_string();
                        field(ui, &mut f, 200.0);
                        egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                            egui::ScrollArea::vertical().id_salt("fonts").max_height(110.0).show(ui, |ui| {
                                ui.set_width(196.0);
                                for n in theme::FONT_NAMES {
                                    if ui.selectable_label(f == n, n).clicked() {
                                        f = n.to_string();
                                    }
                                }
                            });
                        });
                        if f != *st.style.font {
                            st.style.font = f.into();
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("Font style:");
                        let styles = ["Regular", "Italic", "Bold", "Bold Italic"];
                        let cur = st.style.bold as usize * 2 + st.style.italic as usize;
                        for (i, s) in styles.iter().enumerate() {
                            if ui.selectable_label(cur == i, *s).clicked() {
                                st.style.bold = i >= 2;
                                st.style.italic = i % 2 == 1;
                            }
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("Size:");
                        let mut s = st.style.size10 as f32 / 10.0;
                        ui.add(egui::DragValue::new(&mut s).range(1.0..=409.0).speed(0.5));
                        egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                            egui::ScrollArea::vertical().id_salt("sizes").max_height(110.0).show(ui, |ui| {
                                ui.set_width(60.0);
                                for v in [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0] {
                                    if ui.selectable_label((s - v).abs() < 0.01, format!("{v}")).clicked() {
                                        s = v;
                                    }
                                }
                            });
                        });
                        st.style.size10 = (s * 10.0).round() as u16;
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Underline:");
                    let us = ["None", "Single", "Double"];
                    ComboBox::from_id_salt("ul").selected_text(us[st.style.underline.min(2) as usize]).show_ui(ui, |ui| {
                        for (i, u) in us.iter().enumerate() {
                            ui.selectable_value(&mut st.style.underline, i as u8, *u);
                        }
                    });
                    ui.label("Color:");
                    color_button(ui, "fc-color", &mut st.style.color, "Automatic");
                });
                ui.checkbox(&mut st.style.strike, "Strikethrough");
                ui.label("Preview");
                style_preview(ui, &st.style, "AaBbCcYyZz", vec2(300.0, 50.0));
            }
            3 => border_tab(ui, st),
            4 => {
                ui.label("Background Color:");
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(260.0);
                        if let Some(v) = crate::menus::palette(ui, "No Color") {
                            st.style.fill = v;
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("Sample");
                        let (r, _) = ui.allocate_exact_size(vec2(200.0, 60.0), Sense::hover());
                        ui.painter().rect(r, 0.0, st.style.fill.map(theme::rgb).unwrap_or(Color32::WHITE), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
                    });
                });
            }
            _ => {
                ui.checkbox(&mut st.style.locked, "Locked");
                ui.checkbox(&mut st.style.hide_formula, "Hidden");
                ui.add_space(8.0);
                ui.label(RichText::new("Locking cells or hiding formulas has no effect until you protect the worksheet (Review tab, Protect Sheet button).").color(theme::TEXT_DIM));
            }
        }
    });
    let (ok, cancel) = ok_cancel(ui);
    if ok {
        apply_format(a, st);
        return Out::Close;
    }
    if cancel { Out::Close } else { Out::Stay }
}

fn number_tab(ui: &mut egui::Ui, st: &mut FmtState) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label("Category:");
            egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                ui.set_width(130.0);
                for (i, c) in dialogs::CATEGORIES.iter().enumerate() {
                    if ui.selectable_label(st.cat == i, *c).clicked() && st.cat != i {
                        st.cat = i;
                        st.code = match i {
                            4 => "m/d/yyyy".into(),
                            5 => "h:mm:ss AM/PM".into(),
                            7 => "# ?/?".into(),
                            10 => st.code.clone(),
                            _ => dialogs::build_code(i, if i == 0 || i == 9 { 0 } else { st.decimals.max(if matches!(i, 1 | 2 | 3 | 6 | 8) { 2 } else { 0 }) }, st.thousands, st.neg),
                        };
                        let (_, d, t) = dialogs::classify(&st.code);
                        st.decimals = d;
                        st.thousands = t;
                    }
                }
            });
        });
        ui.vertical(|ui| {
            ui.set_width(400.0);
            let mut s2 = st.style.clone();
            s2.num_fmt = st.code.clone().into();
            let shown = ops::render_value(&st.sample, &s2, 20);
            ui.label("Sample");
            egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).inner_margin(4.0).show(ui, |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(format!("{}{}", shown.left, shown.text)).color(shown.color.map(theme::rgb).unwrap_or(theme::TEXT)));
            });
            ui.add_space(4.0);
            let rebuild = |st: &mut FmtState| st.code = dialogs::build_code(st.cat, st.decimals, st.thousands, st.neg);
            match st.cat {
                0 => {
                    ui.label("General format cells have no specific number format.");
                }
                1 | 2 | 3 | 6 | 8 => {
                    ui.horizontal(|ui| {
                        ui.label("Decimal places:");
                        if ui.add(egui::DragValue::new(&mut st.decimals).range(0..=30)).changed() {
                            rebuild(st);
                        }
                    });
                    if st.cat == 1 && ui.checkbox(&mut st.thousands, "Use 1000 Separator (,)").changed() {
                        rebuild(st);
                    }
                    if st.cat == 1 || st.cat == 2 {
                        ui.label("Negative numbers:");
                        let pre = if st.cat == 2 { "$" } else { "" };
                        let opts = [format!("-{pre}1,234.10"), format!("{pre}1,234.10"), format!("({pre}1,234.10)"), format!("({pre}1,234.10)")];
                        for (i, o) in opts.iter().enumerate() {
                            let col = if i % 2 == 1 { theme::RED } else { theme::TEXT };
                            if ui.selectable_label(st.neg == i, RichText::new(o).color(col)).clicked() {
                                st.neg = i;
                                rebuild(st);
                            }
                        }
                    }
                }
                4 | 5 | 7 => {
                    ui.label("Type:");
                    let list: &[&str] = match st.cat {
                        4 => &["m/d/yyyy", "dddd, mmmm d, yyyy", "m/d/yy", "mm/dd/yy", "d-mmm", "d-mmm-yy", "mmm-yy", "mmmm-yy", "mmmm d, yyyy", "m/d/yyyy h:mm", "yyyy-mm-dd"],
                        5 => &["h:mm:ss AM/PM", "h:mm AM/PM", "h:mm", "h:mm:ss", "mm:ss", "[h]:mm:ss", "m/d/yyyy h:mm"],
                        _ => &["# ?/?", "# ??/??", "# ???/???", "# ?/2", "# ?/4", "# ?/8", "# ?/16", "# ?/10", "# ??/100"],
                    };
                    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("dt").max_height(180.0).show(ui, |ui| {
                            ui.set_width(380.0);
                            for c in list {
                                let mut s3 = st.style.clone();
                                s3.num_fmt = (*c).into();
                                let sample = if let Value::Num(_) = st.sample { st.sample.clone() } else { Value::Num(45000.5625) };
                                let label = format!("{}    ({c})", ops::render_value(&sample, &s3, 30).text);
                                if ui.selectable_label(st.code == *c, label).clicked() {
                                    st.code = c.to_string();
                                }
                            }
                        });
                    });
                }
                9 => {
                    ui.label("Text format cells are treated as text even when a number is in the cell.");
                }
                _ => {
                    ui.label("Type:");
                    field(ui, &mut st.code, 380.0);
                    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("custom").max_height(170.0).show(ui, |ui| {
                            ui.set_width(380.0);
                            for c in ["General", "0", "0.00", "#,##0", "#,##0.00", "#,##0_);(#,##0)", "#,##0.00_);[Red](#,##0.00)", "\"$\"#,##0.00", "0%", "0.00%", "0.00E+00", "# ?/?", "m/d/yyyy", "d-mmm-yy", "h:mm AM/PM", "[h]:mm:ss", "@", "0.0,\"K\"", "0.0,,\"M\""] {
                                if ui.selectable_label(st.code == c, c).clicked() {
                                    st.code = c.into();
                                }
                            }
                        });
                    });
                }
            }
        });
    });
}

fn border_tab(ui: &mut egui::Ui, st: &mut FmtState) {
    let line = Some(Border { style: st.border_style, color: st.border_color });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(150.0);
            ui.label(RichText::new("Line").strong());
            ui.label("Style:");
            for (s, l) in [(LineStyle::Hair, "Hair"), (LineStyle::Dotted, "Dotted"), (LineStyle::Dashed, "Dashed"), (LineStyle::Thin, "Thin"), (LineStyle::Medium, "Medium"), (LineStyle::Thick, "Thick"), (LineStyle::Double, "Double")] {
                ui.radio_value(&mut st.border_style, s, l);
            }
            ui.label("Color:");
            let mut c = Some(st.border_color);
            color_button(ui, "bd-color", &mut c, "Automatic");
            st.border_color = c.unwrap_or(0);
        });
        ui.vertical(|ui| {
            ui.label(RichText::new("Presets").strong());
            ui.horizontal(|ui| {
                if ui.button("None").clicked() {
                    st.style.borders = [None; 4];
                    st.changed_border = [true; 4];
                    st.inner = [Some(None), Some(None)];
                }
                if ui.button("Outline").clicked() {
                    st.style.borders = [line; 4];
                    st.changed_border = [true; 4];
                }
                if ui.button("Inside").clicked() {
                    st.inner = [Some(line), Some(line)];
                }
            });
            ui.label(RichText::new("Border").strong());
            ui.label(RichText::new("Click the edges of the preview to add or remove lines.").color(theme::TEXT_DIM));
            let (r, resp) = ui.allocate_exact_size(vec2(220.0, 140.0), Sense::click());
            let p = ui.painter();
            p.rect_filled(r, 0.0, Color32::WHITE);
            let bx = r.shrink(18.0);
            p.text(bx.center(), Align2::CENTER_CENTER, "Text", theme::ui_font(13.0), theme::TEXT_DIM);
            let edges = [
                ([bx.left_top(), bx.left_bottom()], B_LEFT),
                ([bx.right_top(), bx.right_bottom()], B_RIGHT),
                ([bx.left_top(), bx.right_top()], B_TOP),
                ([bx.left_bottom(), bx.right_bottom()], B_BOTTOM),
            ];
            for (pts, k) in edges {
                if let Some(b) = st.style.borders[k] {
                    let w = match b.style {
                        LineStyle::Medium => 2.0,
                        LineStyle::Thick | LineStyle::Double => 3.0,
                        _ => 1.0,
                    };
                    p.line_segment(pts, Stroke::new(w, theme::rgb(b.color)));
                } else {
                    for q in pts {
                        p.circle_filled(q, 1.5, theme::TEXT_OFF);
                    }
                }
            }
            if let Some(Some(b)) = st.inner[0] {
                p.hline(bx.left()..=bx.right(), bx.center().y, Stroke::new(1.0, theme::rgb(b.color)));
            }
            if let Some(Some(b)) = st.inner[1] {
                p.vline(bx.center().x, bx.top()..=bx.bottom(), Stroke::new(1.0, theme::rgb(b.color)));
            }
            if let Some(m) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
                let d = [(m.x - bx.left()).abs(), (m.x - bx.right()).abs(), (m.y - bx.top()).abs(), (m.y - bx.bottom()).abs(), (m.y - bx.center().y).abs(), (m.x - bx.center().x).abs()];
                let (k, _) = d.iter().enumerate().fold((0, f32::MAX), |(bk, bv), (i, v)| if *v < bv { (i, *v) } else { (bk, bv) });
                if k < 4 {
                    st.style.borders[k] = if st.style.borders[k].is_some() { None } else { line };
                    st.changed_border[k] = true;
                } else {
                    let i = k - 4;
                    st.inner[i] = Some(if matches!(st.inner[i], Some(Some(_))) { None } else { line });
                }
            }
        });
    });
}

fn apply_format(a: &mut App, st: &FmtState) {
    let orig = (*a.style_at(a.sel().active)).clone();
    let new = st.style.clone();
    let code = st.code.clone();
    let changed_border = st.changed_border;
    let inner = st.inner;
    let rects = a.sel().ranges.clone();
    if a.sheet().protection.is_some() && rects.iter().any(|r| a.deny_protected(*r)) {
        return;
    }
    a.act(move |a| {
        let si = a.cur;
        // Copy only what the person changed, so mixed selections keep their other differences.
        ops::style_ranges(&mut a.wb, si, &rects, &|s| {
            if *code != *orig.num_fmt {
                s.num_fmt = code.clone().into();
            }
            macro_rules! take {
                ($($f:ident),*) => { $( if new.$f != orig.$f { s.$f = new.$f.clone(); } )* };
            }
            take!(font, size10, bold, italic, underline, strike, color, fill, h, v, wrap, shrink, indent, rotation, locked, hide_formula);
        });
        for r in &rects {
            let r = *r;
            for k in 0..4 {
                if !changed_border[k] {
                    continue;
                }
                let edge = match k {
                    B_LEFT => Rect::new(r.r1, r.c1, r.r2, r.c1),
                    B_RIGHT => Rect::new(r.r1, r.c2, r.r2, r.c2),
                    B_TOP => Rect::new(r.r1, r.c1, r.r1, r.c2),
                    _ => Rect::new(r.r2, r.c1, r.r2, r.c2),
                };
                let b = new.borders[k];
                ops::style_ranges(&mut a.wb, si, &[edge], &move |s| s.borders[k] = b);
            }
            if let Some(b) = inner[0] {
                if r.rows() > 1 {
                    ops::style_ranges(&mut a.wb, si, &[Rect::new(r.r1, r.c1, r.r2 - 1, r.c2)], &move |s| s.borders[B_BOTTOM] = b);
                    ops::style_ranges(&mut a.wb, si, &[Rect::new(r.r1 + 1, r.c1, r.r2, r.c2)], &move |s| s.borders[B_TOP] = b);
                }
            }
            if let Some(b) = inner[1] {
                if r.cols() > 1 {
                    ops::style_ranges(&mut a.wb, si, &[Rect::new(r.r1, r.c1, r.r2, r.c2 - 1)], &move |s| s.borders[B_RIGHT] = b);
                    ops::style_ranges(&mut a.wb, si, &[Rect::new(r.r1, r.c1 + 1, r.r2, r.c2)], &move |s| s.borders[B_LEFT] = b);
                }
            }
        }
    });
    let r = a.sel().rect();
    if r.rows() < 2000 {
        let last = a.sheet().used_rect().map_or(r.r1, |u| u.r2).min(r.r2);
        a.autofit_rows(r.r1, last.max(r.r1));
    }
}

pub fn find_replace(a: &mut App, ui: &mut egui::Ui, tab: &mut u8) -> Out {
    tabs(ui, &["Find", "Replace"], tab);
    a.find.replace = *tab == 1;
    egui::Grid::new("fr").num_columns(2).show(ui, |ui| {
        ui.label("Find what:");
        let r = field(ui, &mut a.find.what, 300.0);
        if ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        ui.end_row();
        if *tab == 1 {
            ui.label("Replace with:");
            field(ui, &mut a.find.with, 300.0);
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut a.find.match_case, "Match case");
        ui.checkbox(&mut a.find.whole, "Match entire cell contents");
    });
    ui.horizontal(|ui| {
        ui.label("Within:");
        ComboBox::from_id_salt("fr-within").selected_text(if a.find.workbook { "Workbook" } else { "Sheet" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut a.find.workbook, false, "Sheet");
            ui.selectable_value(&mut a.find.workbook, true, "Workbook");
        });
        ui.label("Look in:");
        ComboBox::from_id_salt("fr-look").selected_text(if a.find.in_values { "Values" } else { "Formulas" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut a.find.in_values, false, "Formulas");
            ui.selectable_value(&mut a.find.in_values, true, "Values");
        });
    });
    ui.add_space(6.0);
    let mut close = false;
    ui.horizontal(|ui| {
        if *tab == 1 {
            if plain(ui, "Replace All") {
                let n = dialogs::replace(a, true);
                a.alert = Some(("Find and Replace".into(), if n == 0 { "We couldn't find anything to replace.".into() } else { format!("All done. We made {n} replacements.") }));
            }
            if plain(ui, "Replace") {
                dialogs::replace(a, false);
            }
        }
        if plain(ui, "Find All") {
            a.find.results = dialogs::find_matches(a);
            a.find.message = format!("{} cell(s) found", a.find.results.len());
        }
        if primary(ui, "Find Next") || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            dialogs::find_next(a, ui.input(|i| i.modifiers.shift));
        }
        close = plain(ui, "Close");
    });
    if !a.find.results.is_empty() {
        let results = a.find.results.clone();
        egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                ui.set_width(460.0);
                egui::Grid::new("fr-res").striped(true).num_columns(3).show(ui, |ui| {
                    ui.label(RichText::new("Sheet").strong());
                    ui.label(RichText::new("Cell").strong());
                    ui.label(RichText::new("Value").strong());
                    ui.end_row();
                    for (si, p, t) in results {
                        let name = a.wb.sheets.get(si).map(|s| s.name.clone()).unwrap_or_default();
                        if ui.selectable_label(si == a.cur && a.sel().active == p, name).clicked() {
                            a.switch_sheet(si);
                            a.select(p, false);
                            a.reveal(p);
                        }
                        ui.label(cell_name(p.row, p.col));
                        ui.label(t);
                        ui.end_row();
                    }
                });
            });
        });
    }
    if !a.find.message.is_empty() {
        ui.label(RichText::new(&a.find.message).color(theme::TEXT_DIM));
    }
    if close { Out::Close } else { Out::Stay }
}

pub fn cond_manager(a: &mut App, ui: &mut egui::Ui, sel: &mut usize) -> Out {
    let rules = a.sheet().cond.clone();
    ui.horizontal(|ui| {
        if plain(ui, "New Rule...") {
            a.dialog = Some(Dialog::CondQuick { op: "formula".into(), a: "=".into(), b: String::new(), style: 0 });
        }
        if ui.add_enabled(*sel < rules.len(), egui::Button::new("Delete Rule").min_size(vec2(76.0, 24.0))).clicked() {
            let i = *sel;
            a.act(|a| {
                a.wb.sheet_mut(a.cur).cond.remove(i);
            });
        }
        if ui.add_enabled(*sel > 0 && *sel < rules.len(), egui::Button::new("▲")).on_hover_text("Move Up").clicked() {
            let i = *sel;
            a.act(|a| a.wb.sheet_mut(a.cur).cond.swap(i, i - 1));
            *sel -= 1;
        }
        if ui.add_enabled(*sel + 1 < rules.len(), egui::Button::new("▼")).on_hover_text("Move Down").clicked() {
            let i = *sel;
            a.act(|a| a.wb.sheet_mut(a.cur).cond.swap(i, i + 1));
            *sel += 1;
        }
    });
    if a.dialog.is_some() {
        return Out::Close;
    }
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        ui.set_min_size(vec2(600.0, 200.0));
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            egui::Grid::new("cm").striped(true).num_columns(4).min_col_width(80.0).show(ui, |ui| {
                ui.label(RichText::new("Rule (applied in order shown)").strong());
                ui.label(RichText::new("Format").strong());
                ui.label(RichText::new("Applies to").strong());
                ui.label(RichText::new("Stop If True").strong());
                ui.end_row();
                for (i, cf) in rules.iter().enumerate() {
                    let desc = match &cf.rule {
                        CondRule::CellIs { op, a: x, b } => {
                            if op.to_lowercase().contains("between") { format!("Cell Value {op} {x} and {b}") } else { format!("Cell Value {op} {x}") }
                        }
                        CondRule::TextContains { text, .. } => format!("Cell Value contains '{text}'"),
                        CondRule::Duplicate { unique } => if *unique { "Unique Values".into() } else { "Duplicate Values".into() },
                        CondRule::TopBottom { top, rank, percent } => format!("{} {rank}{}", if *top { "Top" } else { "Bottom" }, if *percent { "%" } else { "" }),
                        CondRule::AboveAverage { above } => if *above { "Above Average".into() } else { "Below Average".into() },
                        CondRule::Blanks { blank } => if *blank { "Blank cells".into() } else { "No blanks".into() },
                        CondRule::Formula { formula } => format!("Formula: ={formula}"),
                        CondRule::ColorScale { .. } => "Graded Color Scale".into(),
                        CondRule::DataBar { .. } => "Data Bar".into(),
                        CondRule::IconSet { .. } => "Icon Set".into(),
                    };
                    if ui.selectable_label(*sel == i, desc).clicked() {
                        *sel = i;
                    }
                    let (r, _) = ui.allocate_exact_size(vec2(90.0, 18.0), Sense::hover());
                    let fill = cf.style.fill.map(theme::rgb).unwrap_or(Color32::WHITE);
                    ui.painter().rect(r, 0.0, fill, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "AaBbCcYyZz", theme::ui_font(11.0), cf.style.color.map(theme::rgb).unwrap_or(Color32::BLACK));
                    ui.label(cf.ranges.iter().map(|r| format!("={}", r.a1_abs())).collect::<Vec<_>>().join(","));
                    let mut stop = cf.stop_if_true;
                    if ui.checkbox(&mut stop, "").changed() {
                        a.act(|a| a.wb.sheet_mut(a.cur).cond[i].stop_if_true = stop);
                    }
                    ui.end_row();
                }
            });
        });
    });
    let (ok, cancel) = ok_cancel(ui);
    if ok || cancel { Out::Close } else { Out::Stay }
}

pub fn insert_function(a: &mut App, ui: &mut egui::Ui, search: &mut String, cat: &mut String, sel: &mut String) -> Out {
    let reg = &crate::funcs::registry().map;
    let mut cats: Vec<&str> = reg.values().map(|f| f.cat).filter(|c| !c.is_empty()).collect();
    cats.sort();
    cats.dedup();
    ui.label("Search for a function:");
    ui.horizontal(|ui| {
        let before = search.clone();
        field(ui, search, 330.0);
        if plain(ui, "Go") || (*search != before && !search.is_empty()) {
            *cat = "All".into();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Or select a category:");
        ComboBox::from_id_salt("if-cat").width(200.0).selected_text(cat.as_str()).show_ui(ui, |ui| {
            for c in ["Most Recently Used", "All"].iter().chain(cats.iter()) {
                ui.selectable_value(cat, c.to_string(), *c);
            }
        });
    });
    const RECENT: [&str; 10] = ["SUM", "AVERAGE", "IF", "HYPERLINK", "COUNT", "MAX", "SIN", "SUMIF", "PMT", "STDEV"];
    let q = search.to_uppercase();
    let list: Vec<&str> = if cat == "Most Recently Used" {
        RECENT.to_vec()
    } else {
        let mut v: Vec<&str> = reg.values().filter(|f| (cat == "All" || f.cat == cat.as_str()) && (q.is_empty() || f.name.contains(&q) || f.desc.to_uppercase().contains(&q))).map(|f| f.name).collect();
        v.sort();
        v.dedup();
        v
    };
    ui.label("Select a function:");
    let mut dbl = false;
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
            ui.set_width(420.0);
            for n in &list {
                let r = ui.selectable_label(sel == n, *n);
                if r.clicked() {
                    *sel = n.to_string();
                }
                if r.double_clicked() {
                    dbl = true;
                }
            }
        });
    });
    if let Some(f) = reg.get(sel.as_str()) {
        ui.label(RichText::new(f.sig).strong());
        ui.label(f.desc);
    }
    let (ok, cancel) = ok_cancel(ui);
    if (ok || dbl) && reg.contains_key(sel.as_str()) {
        let s = sel.clone();
        dialogs::insert_function_text(a, &s);
        return Out::Close;
    }
    if cancel { Out::Close } else { Out::Stay }
}

const AGGS: [&str; 6] = ["Sum", "Count", "Average", "Max", "Min", "Product"];

#[allow(clippy::too_many_arguments)]
pub fn pivot(a: &mut App, ui: &mut egui::Ui, source: &mut String, headers: &mut Vec<String>, rows: &mut Vec<u32>, cols: &mut Vec<u32>, values: &mut Vec<(u32, String)>, new_sheet: &mut bool, at: &mut String) -> Out {
    ui.label("Choose the data that you want to analyze:");
    ui.horizontal(|ui| {
        ui.label("Table/Range:");
        let before = source.clone();
        range_field(ui, a, source, 280.0, true);
        if *source != before {
            if let Some((si, r)) = parse_ref(a, source) {
                *headers = (r.c1..=r.c2).map(|c| ops::text_of(&a.wb, &a.calc, si, r.r1, c)).collect();
                rows.clear();
                cols.clear();
                values.clear();
            }
        }
    });
    ui.label("Choose where you want the PivotTable report to be placed:");
    ui.radio_value(new_sheet, true, "New Worksheet");
    ui.horizontal(|ui| {
        ui.radio_value(new_sheet, false, "Existing Worksheet");
        if !*new_sheet {
            range_field(ui, a, at, 180.0, true);
        }
    });
    ui.separator();
    ui.label(RichText::new("PivotTable Fields").strong());
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        egui::Grid::new("pv").striped(true).num_columns(2).show(ui, |ui| {
            for (k, h) in headers.iter().enumerate() {
                let k = k as u32;
                let mut area = if rows.contains(&k) {
                    1
                } else if cols.contains(&k) {
                    2
                } else if values.iter().any(|v| v.0 == k) {
                    3
                } else {
                    0
                };
                let before = area;
                ui.label(if h.is_empty() { format!("Column {}", k + 1) } else { h.clone() });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut area, 0, "—");
                    ui.radio_value(&mut area, 1, "Rows");
                    ui.radio_value(&mut area, 2, "Columns");
                    ui.radio_value(&mut area, 3, "Values");
                    if let Some(v) = values.iter_mut().find(|v| v.0 == k) {
                        ComboBox::from_id_salt(("agg", k)).width(80.0).selected_text(v.1.as_str()).show_ui(ui, |ui| {
                            for g in AGGS {
                                ui.selectable_value(&mut v.1, g.to_string(), g);
                            }
                        });
                    }
                });
                if area != before {
                    rows.retain(|x| *x != k);
                    cols.retain(|x| *x != k);
                    values.retain(|x| x.0 != k);
                    match area {
                        1 => rows.push(k),
                        2 => cols.push(k),
                        3 => {
                            let numeric = parse_ref(a, source).is_some_and(|(si, r)| matches!(a.calc.value(&a.wb, si, r.r1 + 1, r.c1 + k), Value::Num(_)));
                            values.push((k, if numeric { "Sum" } else { "Count" }.into()));
                        }
                        _ => {}
                    }
                }
                ui.end_row();
            }
        });
    });
    let (ok, cancel) = ok_cancel(ui);
    if ok {
        let Some((src_si, src)) = parse_ref(a, source) else {
            a.alert = Some(("PivotTable".into(), "The PivotTable field name is not valid. Select a range with a header row.".into()));
            return Out::Stay;
        };
        if src.rows() < 2 {
            a.alert = Some(("PivotTable".into(), "Select a range with a header row and at least one row of data.".into()));
            return Out::Stay;
        }
        if values.is_empty() && rows.is_empty() {
            a.alert = Some(("PivotTable".into(), "Choose at least one field for Rows or Values.".into()));
            return Out::Stay;
        }
        let target = if *new_sheet { None } else { parse_ref(a, at) };
        if !*new_sheet && target.is_none() {
            a.alert = Some(("PivotTable".into(), "The destination reference is not valid.".into()));
            return Out::Stay;
        }
        let def0 = PivotDef { source_sheet: a.wb.sheets[src_si].name.clone(), source: src, target: Pos::new(2, 0), rows: rows.clone(), cols: cols.clone(), values: values.clone(), filters: vec![] };
        let mut made = None;
        a.act(|a| {
            let (dst, pos) = match target {
                Some((si, r)) => (si, Pos::new(r.r1, r.c1)),
                None => {
                    let name = a.wb.unique_sheet_name("Sheet");
                    a.wb.sheets.insert(a.cur, Arc::new(Sheet::new(&name)));
                    (a.cur, Pos::new(2, 0))
                }
            };
            let mut def = def0;
            def.target = pos;
            a.wb.sheet_mut(dst).pivots.push(def.clone());
            let calc = a.calc.clone();
            made = ops::build_pivot(&mut a.wb, &calc, &def).map(|r| (dst, r));
        });
        let n = a.wb.sheets.len();
        a.sels.resize(n, crate::app::Sel::cell(Pos::new(0, 0)));
        a.views.resize(n, (0, 0));
        if let Some((dst, r)) = made {
            a.switch_sheet(dst);
            a.select(Pos::new(r.r1, r.c1), false);
            a.pivot_pane = Some(a.sheet().pivots.len().saturating_sub(1));
        }
        return Out::Close;
    }
    if cancel { Out::Close } else { Out::Stay }
}

pub fn page_setup(a: &mut App, ui: &mut egui::Ui, ps: &mut PageSetup, tab: &mut u8) -> Out {
    tabs(ui, &["Page", "Margins", "Header/Footer", "Sheet"], tab);
    ui.allocate_ui(vec2(440.0, 250.0), |ui| {
        ui.set_min_size(vec2(440.0, 250.0));
        match *tab {
            0 => {
                ui.label(RichText::new("Orientation").strong());
                ui.horizontal(|ui| {
                    ui.radio_value(&mut ps.landscape, false, "Portrait");
                    ui.radio_value(&mut ps.landscape, true, "Landscape");
                });
                ui.label(RichText::new("Scaling").strong());
                ui.horizontal(|ui| {
                    ui.radio_value(&mut ps.fit_width, false, "Adjust to:");
                    ui.add_enabled(!ps.fit_width, egui::DragValue::new(&mut ps.scale).range(10..=400).suffix("% normal size"));
                });
                ui.radio_value(&mut ps.fit_width, true, "Fit to 1 page wide");
                ui.horizontal(|ui| {
                    ui.label("Paper size:");
                    ComboBox::from_id_salt("paper").width(200.0).selected_text(crate::print::PAPER_NAMES[ps.paper.min(3) as usize]).show_ui(ui, |ui| {
                        for (i, n) in crate::print::PAPER_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut ps.paper, i as u8, *n);
                        }
                    });
                });
            }
            1 => {
                egui::Grid::new("margins").num_columns(4).show(ui, |ui| {
                    let names = ["Left:", "Right:", "Top:", "Bottom:"];
                    for (i, n) in names.iter().enumerate() {
                        ui.label(*n);
                        ui.add(egui::DragValue::new(&mut ps.margins[i]).range(0.0..=5.0).speed(0.05).max_decimals(2).suffix("\""));
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Normal").clicked() {
                        ps.margins = [0.7, 0.7, 0.75, 0.75];
                    }
                    if ui.button("Wide").clicked() {
                        ps.margins = [1.0, 1.0, 1.0, 1.0];
                    }
                    if ui.button("Narrow").clicked() {
                        ps.margins = [0.25, 0.25, 0.75, 0.75];
                    }
                });
            }
            2 => {
                ui.label("Header:");
                field(ui, &mut ps.header, 400.0);
                ui.label("Footer:");
                field(ui, &mut ps.footer, 400.0);
                ui.label(RichText::new("Codes: &P page number, &N number of pages, &A sheet name, &D date.").color(theme::TEXT_DIM));
                ui.horizontal(|ui| {
                    if ui.button("Page 1 of ?").clicked() {
                        ps.footer = "Page &P of &N".into();
                    }
                    if ui.button("Sheet name").clicked() {
                        ps.header = "&A".into();
                    }
                });
            }
            _ => {
                let mut area = ps.print_area.map(|r| r.a1_abs()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Print area:");
                    range_field(ui, a, &mut area, 220.0, false);
                });
                ps.print_area = if area.trim().is_empty() { None } else { parse_rect(area.trim().trim_start_matches('=')).or(ps.print_area) };
                ui.label(RichText::new("Print").strong());
                ui.checkbox(&mut ps.gridlines, "Gridlines");
                ui.checkbox(&mut ps.headings, "Row and column headings");
            }
        }
    });
    let mut r = (false, false, false);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        r.2 = plain(ui, "Print Preview");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r.1 = plain(ui, "Cancel");
            r.0 = primary(ui, "OK");
        });
    });
    if r.0 || r.2 {
        let p = ps.clone();
        a.act(|a| a.wb.sheet_mut(a.cur).page = p);
        if r.2 {
            a.print_sheet = a.cur;
            a.print_page = 0;
            a.page = Some(crate::app::Page::Print);
        }
        return Out::Close;
    }
    if r.1 { Out::Close } else { Out::Stay }
}

pub fn sort(a: &mut App, ui: &mut egui::Ui, range: &mut Rect, header: &mut bool, keys: &mut Vec<(u32, bool)>, left_right: &mut bool) -> Out {
    ui.horizontal(|ui| {
        if plain(ui, "+ Add Level") {
            let next = (range.c1..=range.c2).find(|c| !keys.iter().any(|k| k.0 == *c)).unwrap_or(range.c1);
            keys.push((next, true));
        }
        if plain(ui, "Delete Level") && keys.len() > 1 {
            keys.pop();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(header, "My data has headers");
        });
    });
    let si = a.cur;
    let hdr = *header;
    let rg = *range;
    let name_of = |c: u32| -> String {
        if hdr {
            let t = ops::text_of(&a.wb, &a.calc, si, rg.r1, c);
            if !t.is_empty() {
                return t;
            }
        }
        format!("Column {}", col_name(c))
    };
    egui::Grid::new("sortkeys").num_columns(3).striped(true).show(ui, |ui| {
        ui.label("");
        ui.label(RichText::new("Column").strong());
        ui.label(RichText::new("Order").strong());
        ui.end_row();
        for (i, (c, asc)) in keys.iter_mut().enumerate() {
            ui.label(if i == 0 { "Sort by" } else { "Then by" });
            ComboBox::from_id_salt(("sk", i)).width(180.0).selected_text(name_of(*c)).show_ui(ui, |ui| {
                for k in rg.c1..=rg.c2 {
                    ui.selectable_value(c, k, name_of(k));
                }
            });
            let num = matches!(a.calc.value(&a.wb, si, rg.r1 + hdr as u32, *c), Value::Num(_));
            let (up, down) = if num { ("Smallest to Largest", "Largest to Smallest") } else { ("A to Z", "Z to A") };
            ComboBox::from_id_salt(("so", i)).width(160.0).selected_text(if *asc { up } else { down }).show_ui(ui, |ui| {
                ui.selectable_value(asc, true, up);
                ui.selectable_value(asc, false, down);
            });
            ui.end_row();
        }
    });
    ui.checkbox(left_right, "Sort left to right");
    let (ok, cancel) = ok_cancel(ui);
    if ok {
        let mut r = *range;
        if *header && !*left_right {
            r.r1 += 1;
        }
        let ks = if *left_right { keys.iter().map(|(c, asc)| (range.r1 + (c - range.c1), *asc)).collect::<Vec<_>>() } else { keys.clone() };
        let lr = *left_right;
        if a.deny_protected(r) {
            return Out::Close;
        }
        a.act(|a| {
            let calc = a.calc.clone();
            ops::sort_range(&mut a.wb, &calc, a.cur, r, &ks, lr);
        });
        return Out::Close;
    }
    if cancel { Out::Close } else { Out::Stay }
}

pub fn validation(a: &mut App, ui: &mut egui::Ui, v: &mut Validation, tab: &mut u8) -> Out {
    tabs(ui, &["Settings", "Input Message", "Error Alert"], tab);
    let kinds = [(ValidKind::Any, "Any value"), (ValidKind::Whole, "Whole number"), (ValidKind::Decimal, "Decimal"), (ValidKind::List, "List"), (ValidKind::Date, "Date"), (ValidKind::Time, "Time"), (ValidKind::TextLength, "Text length"), (ValidKind::Custom, "Custom")];
    let ops_list = [("between", "between"), ("notBetween", "not between"), ("equal", "equal to"), ("notEqual", "not equal to"), ("greaterThan", "greater than"), ("lessThan", "less than"), ("greaterThanOrEqual", "greater than or equal to"), ("lessThanOrEqual", "less than or equal to")];
    ui.allocate_ui(vec2(420.0, 230.0), |ui| {
        ui.set_min_size(vec2(420.0, 230.0));
        match *tab {
            0 => {
                ui.label(RichText::new("Validation criteria").strong());
                egui::Grid::new("vd").num_columns(2).show(ui, |ui| {
                    ui.label("Allow:");
                    ComboBox::from_id_salt("vk").width(200.0).selected_text(kinds.iter().find(|k| k.0 == v.kind).map_or("Any value", |k| k.1)).show_ui(ui, |ui| {
                        for (k, l) in kinds {
                            ui.selectable_value(&mut v.kind, k, l);
                        }
                    });
                    ui.end_row();
                    match v.kind {
                        ValidKind::Any => {}
                        ValidKind::List => {
                            ui.label("Source:");
                            range_field(ui, a, &mut v.a, 220.0, false);
                            ui.end_row();
                            ui.label("");
                            ui.checkbox(&mut v.in_cell_list, "In-cell dropdown");
                            ui.end_row();
                        }
                        ValidKind::Custom => {
                            ui.label("Formula:");
                            field(ui, &mut v.a, 220.0);
                            ui.end_row();
                        }
                        _ => {
                            ui.label("Data:");
                            ComboBox::from_id_salt("vo").width(200.0).selected_text(ops_list.iter().find(|o| o.0.eq_ignore_ascii_case(&v.op)).map_or("between", |o| o.1)).show_ui(ui, |ui| {
                                for (o, l) in ops_list {
                                    ui.selectable_value(&mut v.op, o.to_string(), l);
                                }
                            });
                            ui.end_row();
                            let two = v.op.eq_ignore_ascii_case("between") || v.op.eq_ignore_ascii_case("notBetween");
                            ui.label(if two { "Minimum:" } else { "Value:" });
                            field(ui, &mut v.a, 220.0);
                            ui.end_row();
                            if two {
                                ui.label("Maximum:");
                                field(ui, &mut v.b, 220.0);
                                ui.end_row();
                            }
                        }
                    }
                });
                ui.checkbox(&mut v.ignore_blank, "Ignore blank");
                if v.kind == ValidKind::List {
                    ui.label(RichText::new("Type items separated by commas (Yes,No,Maybe) or select a range.").color(theme::TEXT_DIM));
                }
            }
            1 => {
                ui.label("When cell is selected, show this input message:");
                ui.label("Title:");
                field(ui, &mut v.prompt_title, 400.0);
                ui.label("Input message:");
                ui.add(TextEdit::multiline(&mut v.prompt).desired_width(400.0).desired_rows(5));
            }
            _ => {
                ui.label("When invalid data is entered, show this error alert:");
                ui.horizontal(|ui| {
                    ui.label("Style:");
                    let ss = ["Stop", "Warning", "Information"];
                    ComboBox::from_id_salt("ves").selected_text(ss[v.error_style.min(2) as usize]).show_ui(ui, |ui| {
                        for (i, s) in ss.iter().enumerate() {
                            ui.selectable_value(&mut v.error_style, i as u8, *s);
                        }
                    });
                });
                ui.label("Title:");
                field(ui, &mut v.error_title, 400.0);
                ui.label("Error message:");
                ui.add(TextEdit::multiline(&mut v.error).desired_width(400.0).desired_rows(4));
            }
        }
    });
    let mut r = (false, false, false);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        r.2 = plain(ui, "Clear All");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            r.1 = plain(ui, "Cancel");
            r.0 = primary(ui, "OK");
        });
    });
    if r.0 || r.2 {
        let mut v2 = (*v).clone();
        let sel = a.sel().ranges.clone();
        v2.ranges = sel.clone();
        let clear = r.2 || (v2.kind == ValidKind::Any && v2.prompt.is_empty());
        a.act(|a| {
            let sh = a.wb.sheet_mut(a.cur);
            // Cells in the selection lose any rule they had.
            sh.valid.retain_mut(|x| {
                x.ranges.retain(|rr| !sel.iter().any(|s| s.contains_rect(rr)));
                !x.ranges.is_empty()
            });
            if !clear {
                sh.valid.push(v2);
            }
        });
        return Out::Close;
    }
    if r.1 { Out::Close } else { Out::Stay }
}

pub fn subtotal(a: &mut App, ui: &mut egui::Ui, range: &mut Rect, at_change: &mut u32, func: &mut usize, cols: &mut [bool]) -> Out {
    const FUNCS: [(&str, u32); 6] = [("Sum", 9), ("Count", 3), ("Average", 1), ("Max", 4), ("Min", 5), ("Product", 6)];
    let si = a.cur;
    let rg = *range;
    let name_of = |k: u32| {
        let t = ops::text_of(&a.wb, &a.calc, si, rg.r1, rg.c1 + k);
        if t.is_empty() { format!("Column {}", col_name(rg.c1 + k)) } else { t }
    };
    egui::Grid::new("st").num_columns(2).show(ui, |ui| {
        ui.label("At each change in:");
        ComboBox::from_id_salt("st-at").width(180.0).selected_text(name_of(*at_change)).show_ui(ui, |ui| {
            for k in 0..rg.cols() {
                ui.selectable_value(at_change, k, name_of(k));
            }
        });
        ui.end_row();
        ui.label("Use function:");
        ComboBox::from_id_salt("st-fn").width(180.0).selected_text(FUNCS[*func].0).show_ui(ui, |ui| {
            for (i, f) in FUNCS.iter().enumerate() {
                ui.selectable_value(func, i, f.0);
            }
        });
        ui.end_row();
    });
    ui.label("Add subtotal to:");
    egui::Frame::new().stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
        ui.set_min_size(vec2(280.0, 100.0));
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            for (k, on) in cols.iter_mut().enumerate() {
                ui.checkbox(on, name_of(k as u32));
            }
        });
    });
    let (ok, cancel) = ok_cancel(ui);
    if ok {
        let r = rg;
        let key = r.c1 + *at_change;
        let (fname, fnum) = FUNCS[*func];
        let targets: Vec<u32> = cols.iter().enumerate().filter(|(_, on)| **on).map(|(k, _)| r.c1 + k as u32).collect();
        if r.rows() < 2 || targets.is_empty() {
            return Out::Close;
        }
        let mut groups: Vec<(u32, u32, String)> = vec![];
        let mut start = r.r1 + 1;
        for row in r.r1 + 1..=r.r2 {
            let cur = ops::text_of(&a.wb, &a.calc, si, row, key);
            let next = if row < r.r2 { Some(ops::text_of(&a.wb, &a.calc, si, row + 1, key)) } else { None };
            if next.as_deref() != Some(cur.as_str()) {
                groups.push((start, row, cur));
                start = row + 1;
            }
        }
        a.act(|a| {
            // Bottom up, so inserted rows don't move the groups still to do.
            let mut extra = 0;
            for (g0, g1, label) in groups.iter().rev() {
                let at = g1 + 1;
                ops::insert_rows(&mut a.wb, si, at, 1);
                let _ = ops::set_input(&mut a.wb, si, at, key, &format!("{label} {fname}"));
                for c in &targets {
                    let n = col_name(*c);
                    let _ = ops::set_input(&mut a.wb, si, at, *c, &format!("=SUBTOTAL({fnum},{n}{}:{n}{})", g0 + 1, g1 + 1));
                }
                ops::style_ranges(&mut a.wb, si, &[Rect::new(at, r.c1, at, r.c2)], &|s| s.bold = true);
                extra += 1;
            }
            let last = r.r2 + extra + 1;
            ops::insert_rows(&mut a.wb, si, last, 1);
            let _ = ops::set_input(&mut a.wb, si, last, key, &format!("Grand {fname}"));
            for c in &targets {
                let n = col_name(*c);
                let _ = ops::set_input(&mut a.wb, si, last, *c, &format!("=SUBTOTAL({fnum},{n}{}:{n}{})", r.r1 + 2, last));
            }
            ops::style_ranges(&mut a.wb, si, &[Rect::new(last, r.c1, last, r.c2)], &|s| s.bold = true);
        });
        return Out::Close;
    }
    if cancel { Out::Close } else { Out::Stay }
}
