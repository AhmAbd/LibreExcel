//! Contents of every popup menu. Each returns true when the menu should close.

use crate::app::{App, MenuKind};
use crate::chrome::{header, item, item_on, separator};
use crate::dialogs::{self, Dialog};
use crate::icons::Icon;
use crate::model::*;
use crate::ops::{self, PasteOpts, PasteWhat};
use crate::ribbon::{LAST_FILL, LAST_FONT};
use crate::theme::{self, *};
use egui::{vec2, Color32, Rect as R, Sense, Stroke, StrokeKind};

pub const CHART_KINDS: [(ChartKind, &str, Icon); 10] = [
    (ChartKind::Column, "Clustered Column", Icon::ChartColumn),
    (ChartKind::ColumnStacked, "Stacked Column", Icon::ChartColumn),
    (ChartKind::Bar, "Clustered Bar", Icon::ChartBar),
    (ChartKind::BarStacked, "Stacked Bar", Icon::ChartBar),
    (ChartKind::Line, "Line", Icon::ChartLine),
    (ChartKind::LineMarkers, "Line with Markers", Icon::ChartLine),
    (ChartKind::Area, "Area", Icon::ChartArea),
    (ChartKind::Pie, "Pie", Icon::ChartPie),
    (ChartKind::Doughnut, "Doughnut", Icon::ChartPie),
    (ChartKind::Scatter, "Scatter", Icon::ChartScatter),
];

/// Theme and standard colour grid. Returns Some(None) for "automatic / no colour".
pub fn palette(ui: &mut egui::Ui, none_label: &str) -> Option<Option<u32>> {
    let mut out = None;
    if item(ui, None, none_label, "") {
        out = Some(None);
    }
    header(ui, "Theme Colors");
    let sw = 18.0;
    let swatch = |ui: &mut egui::Ui, c: u32, out: &mut Option<Option<u32>>| {
        let (r, resp) = ui.allocate_exact_size(vec2(sw + 4.0, sw + 2.0), Sense::click());
        let rr = R::from_center_size(r.center(), vec2(sw - 2.0, sw - 4.0));
        ui.painter().rect_filled(rr, 0.0, theme::rgb(c));
        ui.painter().rect_stroke(rr, 0.0, Stroke::new(1.0, if resp.hovered() { Color32::from_rgb(0xF2, 0x94, 0x36) } else { Color32::from_rgb(0xD0, 0xD0, 0xD0) }), StrokeKind::Outside);
        if resp.clicked() {
            *out = Some(Some(c));
        }
        resp.on_hover_text(format!("#{c:06X}"));
    };
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        for c in THEME_TOP {
            swatch(ui, c, &mut out);
        }
    });
    for k in 0..5 {
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            for c in THEME_TOP {
                swatch(ui, theme::shades(c)[k], &mut out);
            }
        });
    }
    header(ui, "Standard Colors");
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        for c in STANDARD {
            swatch(ui, c, &mut out);
        }
    });
    out
}

pub fn content(a: &mut App, ui: &mut egui::Ui, kind: MenuKind) -> bool {
    ui.set_min_width(230.0);
    let ctx = ui.ctx().clone();
    match kind {
        MenuKind::Paste => {
            let mut opt = None;
            if item(ui, Some(Icon::Paste), "Paste", "Ctrl+V") {
                opt = Some(PasteOpts::default());
            }
            if item(ui, Some(Icon::Text), "Values", "") {
                opt = Some(PasteOpts { what: PasteWhat::Values, ..Default::default() });
            }
            if item(ui, Some(Icon::Function), "Formulas", "") {
                opt = Some(PasteOpts { what: PasteWhat::Formulas, ..Default::default() });
            }
            if item(ui, Some(Icon::Painter), "Formatting", "") {
                opt = Some(PasteOpts { what: PasteWhat::Formats, ..Default::default() });
            }
            if item(ui, Some(Icon::Orientation), "Transpose", "") {
                opt = Some(PasteOpts { transpose: true, ..Default::default() });
            }
            if item(ui, Some(Icon::Borders), "No Borders", "") {
                opt = Some(PasteOpts { what: PasteWhat::NoBorders, ..Default::default() });
            }
            if item(ui, Some(Icon::Format), "Keep Source Column Widths", "") {
                opt = Some(PasteOpts { what: PasteWhat::Widths, ..Default::default() });
            }
            separator(ui);
            if item(ui, None, "Paste Special...", "Ctrl+Alt+V") {
                a.dialog = Some(Dialog::PasteSpecial(PasteOpts::default()));
                return true;
            }
            if let Some(o) = opt {
                if a.clip.is_some() {
                    a.paste_clip(o);
                    if o.what == PasteWhat::Widths {
                        a.paste_clip(PasteOpts::default());
                    }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
                return true;
            }
            false
        }
        MenuKind::Borders => {
            let presets = [
                (0, "Bottom Border"), (1, "Top Border"), (2, "Left Border"), (3, "Right Border"), (4, "No Border"), (5, "All Borders"),
                (6, "Outside Borders"), (7, "Thick Outside Borders"), (8, "Bottom Double Border"), (9, "Thick Bottom Border"), (10, "Top and Bottom Border"), (11, "Top and Thick Bottom Border"),
            ];
            header(ui, "Borders");
            for (k, label) in presets {
                if item(ui, Some(Icon::Borders), label, "") {
                    dialogs::apply_border_preset(a, k);
                    return true;
                }
            }
            separator(ui);
            if item(ui, None, "More Borders...", "") {
                a.dialog = Some(Dialog::format_cells(a, 3));
                return true;
            }
            false
        }
        MenuKind::FillColor => {
            if let Some(c) = palette(ui, "No Fill") {
                if let Some(c) = c {
                    LAST_FILL.with(|x| x.set(c));
                }
                a.format(move |s| s.fill = c);
                return true;
            }
            separator(ui);
            if item(ui, None, "More Colors...", "") {
                a.dialog = Some(Dialog::MoreColors { fill: true, color: [255, 255, 0] });
                return true;
            }
            false
        }
        MenuKind::FontColor => {
            if let Some(c) = palette(ui, "Automatic") {
                if let Some(c) = c {
                    LAST_FONT.with(|x| x.set(c));
                }
                a.format(move |s| s.color = c);
                return true;
            }
            separator(ui);
            if item(ui, None, "More Colors...", "") {
                a.dialog = Some(Dialog::MoreColors { fill: false, color: [255, 0, 0] });
                return true;
            }
            false
        }
        MenuKind::Merge => {
            let rects = a.sel().ranges.clone();
            if item(ui, Some(Icon::Merge), "Merge & Center", "") {
                dialogs::merge_center(a);
                return true;
            }
            if item(ui, Some(Icon::Merge), "Merge Across", "") {
                a.act(|a| ops::set_merge(&mut a.wb, a.cur, &rects, 1));
                return true;
            }
            if item(ui, Some(Icon::Merge), "Merge Cells", "") {
                a.act(|a| ops::set_merge(&mut a.wb, a.cur, &rects, 0));
                return true;
            }
            if item(ui, Some(Icon::Merge), "Unmerge Cells", "") {
                a.act(|a| ops::set_merge(&mut a.wb, a.cur, &rects, 2));
                return true;
            }
            false
        }
        MenuKind::NumFmt => {
            let p = a.sel().active;
            let v = a.calc.value(&a.wb, a.cur, p.row, p.col);
            let st = a.style_at(p);
            for (label, code) in dialogs::QUICK_FORMATS {
                let mut s = (*st).clone();
                s.num_fmt = code.into();
                let sample = match &v {
                    crate::eval::Value::Num(_) => ops::render_value(&v, &s, 11).text,
                    _ => String::new(),
                };
                if item(ui, None, label, &sample) {
                    let c = code.to_string();
                    a.format(move |s| s.num_fmt = c.as_str().into());
                    return true;
                }
            }
            separator(ui);
            if item(ui, None, "More Number Formats...", "") {
                a.dialog = Some(Dialog::format_cells(a, 0));
                return true;
            }
            false
        }
        MenuKind::CondFormat => cond_menu(a, ui),
        MenuKind::FormatTable => {
            header(ui, "Light, Medium and Dark");
            let mut pick = None;
            for row in 0..2u8 {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    for k in 0..6u8 {
                        let s = row * 6 + k + 1;
                        let (r, resp) = ui.allocate_exact_size(vec2(44.0, 36.0), Sense::click());
                        dialogs::table_swatch(ui.painter(), r.shrink(3.0), s, resp.hovered());
                        if resp.clicked() {
                            pick = Some(s);
                        }
                    }
                });
            }
            if let Some(s) = pick {
                let mut d = Dialog::create_table(a);
                if let Dialog::CreateTable { style, .. } = &mut d {
                    *style = s;
                }
                a.dialog = Some(d);
                return true;
            }
            false
        }
        MenuKind::CellStyles => {
            let mut chosen = None;
            for (group, list) in dialogs::cell_style_groups() {
                header(ui, group);
                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(460.0);
                    for (name, f) in list {
                        let mut st = Style::default();
                        f(&mut st);
                        let (r, resp) = ui.allocate_exact_size(vec2(88.0, 26.0), Sense::click());
                        let p = ui.painter();
                        p.rect_filled(r.shrink(2.0), 0.0, st.fill.map(theme::rgb).unwrap_or(WHITE));
                        p.rect_stroke(r.shrink(2.0), 0.0, Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { Color32::from_rgb(0xF2, 0x94, 0x36) } else { Color32::from_rgb(0xD0, 0xD0, 0xD0) }), StrokeKind::Inside);
                        let font = egui::FontId::new(if st.size10 > 120 { 14.0 } else { 12.0 }, theme::cell_family(&st.font, st.bold, st.italic));
                        p.with_clip_rect(r.shrink(3.0)).text(r.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, name, font, st.color.map(theme::rgb).unwrap_or(Color32::BLACK));
                        if resp.clicked() {
                            chosen = Some(f);
                        }
                    }
                });
            }
            if let Some(f) = chosen {
                a.format(move |s| {
                    let keep_fmt = s.num_fmt.clone();
                    let mut n = Style::default();
                    f(&mut n);
                    let numeric = n.num_fmt.as_ref() != "General";
                    *s = Style { borders: if n.borders.iter().any(|b| b.is_some()) { n.borders } else { s.borders }, num_fmt: if numeric { n.num_fmt.clone() } else { keep_fmt }, ..n };
                });
                return true;
            }
            false
        }
        MenuKind::Insert => {
            if item(ui, Some(Icon::Insert), "Insert Cells...", "Ctrl+Shift+=") {
                a.dialog = Some(Dialog::InsertCells(0));
                return true;
            }
            if item(ui, Some(Icon::Insert), "Insert Sheet Rows", "") {
                dialogs::insert_rows_cols(a, true);
                return true;
            }
            if item(ui, Some(Icon::Insert), "Insert Sheet Columns", "") {
                dialogs::insert_rows_cols(a, false);
                return true;
            }
            if item(ui, Some(Icon::Sheet), "Insert Sheet", "Shift+F11") {
                a.insert_sheet(a.cur);
                return true;
            }
            false
        }
        MenuKind::Delete => {
            if item(ui, Some(Icon::Delete), "Delete Cells...", "Ctrl+-") {
                a.dialog = Some(Dialog::DeleteCells(0));
                return true;
            }
            if item(ui, Some(Icon::Delete), "Delete Sheet Rows", "") {
                dialogs::delete_rows_cols(a, true);
                return true;
            }
            if item(ui, Some(Icon::Delete), "Delete Sheet Columns", "") {
                dialogs::delete_rows_cols(a, false);
                return true;
            }
            if item(ui, Some(Icon::Sheet), "Delete Sheet", "") {
                dialogs::delete_sheet(a, a.cur);
                return true;
            }
            false
        }
        MenuKind::Format => {
            header(ui, "Cell Size");
            if item(ui, None, "Row Height...", "") {
                a.dialog = Some(Dialog::Size { rows: true, value: format!("{}", a.sheet().row_px(a.sel().active.row) * 0.75) });
                return true;
            }
            if item(ui, None, "AutoFit Row Height", "") {
                let r = a.sel().rect();
                a.act(|a| {
                    let sh = a.wb.sheet_mut(a.cur);
                    for k in r.r1..=r.r2.min(r.r1 + 5000) {
                        sh.custom_h.remove(&k);
                        sh.row_h.remove(&k);
                    }
                });
                a.autofit_rows(r.r1, r.r2.min(r.r1 + 5000));
                return true;
            }
            if item(ui, None, "Column Width...", "") {
                a.dialog = Some(Dialog::Size { rows: false, value: format!("{:.2}", (a.sheet().col_px(a.sel().active.col) - 5.0).max(0.0) / 7.0) });
                return true;
            }
            if item(ui, None, "AutoFit Column Width", "") {
                let r = a.sel().rect();
                crate::chrome::pending_autofit((r.c1..=r.c2.min(r.c1 + 300)).collect());
                return true;
            }
            header(ui, "Visibility");
            ui.menu_button("Hide & Unhide", |ui| {
                for (label, k) in [("Hide Rows", 0), ("Hide Columns", 1), ("Hide Sheet", 2), ("Unhide Rows", 3), ("Unhide Columns", 4), ("Unhide Sheet...", 5)] {
                    if ui.button(label).clicked() {
                        dialogs::hide_unhide(a, k);
                        ui.close();
                    }
                }
            });
            header(ui, "Organize Sheets");
            if item(ui, None, "Rename Sheet", "") {
                a.rename_tab = Some((a.cur, a.sheet().name.clone()));
                return true;
            }
            if item(ui, None, "Move or Copy Sheet...", "") {
                a.dialog = Some(Dialog::MoveCopy { to: a.cur + 1, copy: false });
                return true;
            }
            ui.menu_button("Tab Color", |ui| {
                if let Some(c) = palette(ui, "No Color") {
                    a.act(|a| a.wb.sheet_mut(a.cur).tab_color = c);
                    ui.close();
                }
            });
            header(ui, "Protection");
            let protected = a.sheet().protection.is_some();
            if item(ui, Some(Icon::Protect), if protected { "Unprotect Sheet..." } else { "Protect Sheet..." }, "") {
                if protected {
                    dialogs::unprotect(a);
                } else {
                    a.dialog = Some(Dialog::Protect { password: String::new(), select_locked: true });
                }
                return true;
            }
            let locked = a.style_at(a.sel().active).locked;
            if item_on(ui, Some(Icon::Lock), "Lock Cell", "", true, locked) {
                a.format(move |s| s.locked = !locked);
                return true;
            }
            if item(ui, None, "Format Cells...", "Ctrl+1") {
                a.dialog = Some(Dialog::format_cells(a, 0));
                return true;
            }
            false
        }
        MenuKind::AutoSum => {
            for (label, f) in [("Sum", "SUM"), ("Average", "AVERAGE"), ("Count Numbers", "COUNT"), ("Max", "MAX"), ("Min", "MIN")] {
                if item(ui, None, label, "") {
                    a.autosum(f);
                    return true;
                }
            }
            separator(ui);
            if item(ui, None, "More Functions...", "") {
                a.dialog = Some(Dialog::insert_function(a));
                return true;
            }
            false
        }
        MenuKind::FillDown => {
            for (label, k, key) in [("Down", 0u8, "Ctrl+D"), ("Right", 1, "Ctrl+R"), ("Up", 2, ""), ("Left", 3, "")] {
                if item(ui, None, label, key) {
                    a.fill_dir(k);
                    return true;
                }
            }
            if item(ui, None, "Series...", "") {
                a.dialog = Some(Dialog::Series { rows: false, step: "1".into(), stop: String::new(), kind: 0 });
                return true;
            }
            if item(ui, None, "Flash Fill (copy pattern down)", "") {
                a.fill_dir(0);
                return true;
            }
            false
        }
        MenuKind::Clear => {
            let rects = a.sel().ranges.clone();
            if item(ui, Some(Icon::Clear), "Clear All", "") {
                a.act(|a| {
                    for r in &rects {
                        ops::clear_rect(&mut a.wb, a.cur, *r);
                    }
                });
                return true;
            }
            if item(ui, None, "Clear Formats", "") {
                a.format(|s| *s = Style::default());
                return true;
            }
            if item(ui, None, "Clear Contents", "Delete") {
                a.delete_contents();
                return true;
            }
            if item(ui, None, "Clear Notes", "") {
                a.act(|a| a.wb.sheet_mut(a.cur).notes.retain(|p, _| !rects.iter().any(|r| r.contains(*p))));
                return true;
            }
            if item(ui, None, "Clear Hyperlinks", "") {
                a.act(|a| a.wb.sheet_mut(a.cur).links.retain(|p, _| !rects.iter().any(|r| r.contains(*p))));
                return true;
            }
            false
        }
        MenuKind::SortFilter => {
            if item(ui, Some(Icon::SortAZ), "Sort A to Z", "") {
                dialogs::quick_sort(a, true);
                return true;
            }
            if item(ui, Some(Icon::SortZA), "Sort Z to A", "") {
                dialogs::quick_sort(a, false);
                return true;
            }
            if item(ui, Some(Icon::SortFilter), "Custom Sort...", "") {
                a.dialog = Some(Dialog::sort(a));
                return true;
            }
            separator(ui);
            if item_on(ui, Some(Icon::Filter), "Filter", "Ctrl+Shift+L", true, a.sheet().filter.is_some()) {
                dialogs::toggle_filter(a);
                return true;
            }
            if item_on(ui, Some(Icon::FilterClear), "Clear", "", a.sheet().filter.is_some(), false) {
                dialogs::clear_filter(a);
                return true;
            }
            if item_on(ui, Some(Icon::Refresh), "Reapply", "", a.sheet().filter.is_some(), false) {
                a.act(|a| ops::apply_filter(&mut a.wb, &a.calc, a.cur));
                return true;
            }
            false
        }
        MenuKind::Find => {
            if item(ui, Some(Icon::Find), "Find...", "Ctrl+F") {
                a.find.replace = false;
                a.dialog = Some(Dialog::FindReplace(0));
                return true;
            }
            if item(ui, None, "Replace...", "Ctrl+H") {
                a.find.replace = true;
                a.dialog = Some(Dialog::FindReplace(1));
                return true;
            }
            if item(ui, None, "Go To...", "Ctrl+G") {
                a.dialog = Some(Dialog::GoTo(String::new()));
                return true;
            }
            if item(ui, None, "Go To Special...", "") {
                a.dialog = Some(Dialog::GoSpecial(0));
                return true;
            }
            separator(ui);
            for (label, k) in [("Formulas", 1u8), ("Notes", 2), ("Conditional Formatting", 3), ("Constants", 4), ("Data Validation", 5)] {
                if item(ui, None, label, "") {
                    dialogs::select_special(a, k);
                    return true;
                }
            }
            false
        }
        MenuKind::CellCtx => cell_ctx(a, ui, &ctx, 0),
        MenuKind::ColCtx => cell_ctx(a, ui, &ctx, 1),
        MenuKind::RowCtx => cell_ctx(a, ui, &ctx, 2),
        MenuKind::TabCtx(i) => {
            if item(ui, Some(Icon::Sheet), "Insert...", "") {
                a.insert_sheet(i);
                return true;
            }
            if item(ui, Some(Icon::Delete), "Delete", "") {
                dialogs::delete_sheet(a, i);
                return true;
            }
            if item(ui, None, "Rename", "") {
                a.rename_tab = Some((i, a.wb.sheets[i].name.clone()));
                return true;
            }
            if item(ui, None, "Move or Copy...", "") {
                a.dialog = Some(Dialog::MoveCopy { to: i + 1, copy: false });
                return true;
            }
            if item(ui, Some(Icon::Protect), if a.sheet().protection.is_some() { "Unprotect Sheet..." } else { "Protect Sheet..." }, "") {
                if a.sheet().protection.is_some() {
                    dialogs::unprotect(a);
                } else {
                    a.dialog = Some(Dialog::Protect { password: String::new(), select_locked: true });
                }
                return true;
            }
            ui.menu_button("      Tab Color", |ui| {
                if let Some(c) = palette(ui, "No Color") {
                    a.act(|a| a.wb.sheet_mut(i).tab_color = c);
                    ui.close();
                }
            });
            separator(ui);
            if item(ui, Some(Icon::Hide), "Hide", "") {
                dialogs::hide_sheet(a);
                return true;
            }
            let any_hidden = a.wb.sheets.iter().any(|s| s.hidden);
            if item_on(ui, None, "Unhide...", "", any_hidden, false) {
                a.dialog = Some(Dialog::Unhide(0));
                return true;
            }
            false
        }
        MenuKind::Chart(k) => {
            if k == 100 {
                header(ui, "Change Chart Type");
            } else {
                header(ui, "Insert Chart");
            }
            for (kind, label, ic) in CHART_KINDS {
                let group = |x: ChartKind| match x {
                    ChartKind::Column | ChartKind::ColumnStacked => 0,
                    ChartKind::Bar | ChartKind::BarStacked => 1,
                    ChartKind::Line | ChartKind::LineMarkers => 2,
                    ChartKind::Area => 3,
                    ChartKind::Pie | ChartKind::Doughnut => 4,
                    ChartKind::Scatter => 5,
                };
                if k != 100 && group(kind) != group(CHART_KINDS.iter().find(|c| c.0 as u8 == k).map_or(ChartKind::Column, |c| c.0)) {
                    continue;
                }
                if item(ui, Some(ic), label, "") {
                    if k == 100 {
                        if let Some(i) = a.chart_sel {
                            a.act(|a| a.wb.sheet_mut(a.cur).charts[i].kind = kind);
                        }
                    } else {
                        crate::charts::insert_chart(a, kind);
                    }
                    return true;
                }
            }
            false
        }
        MenuKind::ChartCtx => {
            let Some(i) = a.chart_sel else { return true };
            if item(ui, Some(Icon::Delete), "Delete", "Delete") {
                a.act(|a| {
                    a.wb.sheet_mut(a.cur).charts.remove(i);
                });
                a.chart_sel = None;
                return true;
            }
            if item(ui, Some(Icon::Table), "Select Data...", "") {
                a.dialog = Some(Dialog::chart_data(a, i));
                return true;
            }
            if item(ui, Some(Icon::Text), "Edit Titles...", "") {
                a.dialog = Some(Dialog::chart_data(a, i));
                return true;
            }
            ui.menu_button("      Change Chart Type", |ui| {
                for (kind, label, _) in CHART_KINDS {
                    if ui.button(label).clicked() {
                        a.act(|a| a.wb.sheet_mut(a.cur).charts[i].kind = kind);
                        ui.close();
                    }
                }
            });
            if item(ui, Some(Icon::Copy), "Duplicate", "") {
                a.act(|a| {
                    let mut c = a.sheet().charts[i].clone();
                    c.x += 20.0;
                    c.y += 20.0;
                    a.wb.sheet_mut(a.cur).charts.push(c);
                });
                return true;
            }
            false
        }
        MenuKind::Freeze => {
            let s = a.sel().active;
            let frozen = a.sheet().freeze != (0, 0);
            if item(ui, Some(Icon::Freeze), if frozen { "Unfreeze Panes" } else { "Freeze Panes" }, "") {
                let (top, left) = a.view();
                let f = if frozen { (0, 0) } else { (s.row, s.col) };
                a.act(|a| a.wb.sheet_mut(a.cur).freeze = f);
                if !frozen {
                    let _ = (top, left);
                    a.views[a.cur] = (f.0.max(top), f.1.max(left));
                }
                return true;
            }
            if item(ui, Some(Icon::Freeze), "Freeze Top Row", "") {
                let top = a.view().0;
                a.act(|a| a.wb.sheet_mut(a.cur).freeze = (top + 1, 0));
                return true;
            }
            if item(ui, Some(Icon::Freeze), "Freeze First Column", "") {
                let left = a.view().1;
                a.act(|a| a.wb.sheet_mut(a.cur).freeze = (0, left + 1));
                return true;
            }
            false
        }
        MenuKind::Orientation => {
            for (label, rot) in [("Angle Counterclockwise", 45i16), ("Angle Clockwise", -45), ("Vertical Text", 255), ("Rotate Text Up", 90), ("Rotate Text Down", -90), ("Horizontal (no rotation)", 0)] {
                if item(ui, Some(Icon::Orientation), label, "") {
                    a.format(move |s| s.rotation = rot);
                    return true;
                }
            }
            separator(ui);
            if item(ui, None, "Format Cell Alignment", "") {
                a.dialog = Some(Dialog::format_cells(a, 1));
                return true;
            }
            false
        }
        MenuKind::Margins => {
            for (label, m) in [("Normal", [0.7, 0.7, 0.75, 0.75]), ("Wide", [1.0, 1.0, 1.0, 1.0]), ("Narrow", [0.25, 0.25, 0.75, 0.75])] {
                let cur = a.sheet().page.margins == m;
                if item_on(ui, Some(Icon::Margins), label, &format!("{}\" / {}\"", m[2], m[0]), true, cur) {
                    a.act(|a| a.wb.sheet_mut(a.cur).page.margins = m);
                    return true;
                }
            }
            if item(ui, None, "Custom Margins...", "") {
                a.dialog = Some(Dialog::page_setup(a));
                return true;
            }
            false
        }
        MenuKind::PageOrient => {
            for (label, l) in [("Portrait", false), ("Landscape", true)] {
                if item_on(ui, Some(Icon::PageOrient), label, "", true, a.sheet().page.landscape == l) {
                    a.act(|a| a.wb.sheet_mut(a.cur).page.landscape = l);
                    return true;
                }
            }
            false
        }
        MenuKind::PageSize => {
            for (label, k, dims) in [("Letter", 0u8, "8.5\" x 11\""), ("A4", 1, "8.27\" x 11.69\""), ("Legal", 2, "8.5\" x 14\""), ("A3", 3, "11.69\" x 16.54\"")] {
                if item_on(ui, Some(Icon::PageSize), label, dims, true, a.sheet().page.paper == k) {
                    a.act(|a| a.wb.sheet_mut(a.cur).page.paper = k);
                    return true;
                }
            }
            false
        }
        MenuKind::PrintArea => {
            if item(ui, Some(Icon::PrintArea), "Set Print Area", "") {
                let r = a.sel().rect();
                a.act(|a| a.wb.sheet_mut(a.cur).page.print_area = Some(r));
                return true;
            }
            if item(ui, None, "Clear Print Area", "") {
                a.act(|a| a.wb.sheet_mut(a.cur).page.print_area = None);
                return true;
            }
            false
        }
        MenuKind::FilterCol(c) => dialogs::filter_menu(a, ui, c),
        MenuKind::TabColor(i) => {
            if let Some(c) = palette(ui, "No Color") {
                a.act(|a| a.wb.sheet_mut(i).tab_color = c);
                return true;
            }
            false
        }
        MenuKind::Validation => {
            let p = a.sel().active;
            let items = dialogs::list_items(a, p);
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for it in items {
                    if item(ui, None, &it, "") {
                        let s = it.clone();
                        a.act(|a| {
                            let _ = ops::set_input(&mut a.wb, a.cur, p.row, p.col, &s);
                        });
                        a.menu = None;
                    }
                }
            });
            a.menu.is_none()
        }
        MenuKind::WhatIf => {
            if item(ui, Some(Icon::WhatIf), "Goal Seek...", "") {
                let p = a.sel().active;
                a.dialog = Some(Dialog::GoalSeek { set: cell_name(p.row, p.col), to: String::new(), change: String::new() });
                return true;
            }
            false
        }
        MenuKind::Names => {
            let names: Vec<String> = a.wb.names.iter().map(|n| n.name.clone()).collect();
            if names.is_empty() {
                item_on(ui, None, "(no names defined)", "", false, false);
            }
            for n in names {
                if item(ui, Some(Icon::Names), &n, "") {
                    dialogs::insert_function_text(a, &n);
                    return true;
                }
            }
            separator(ui);
            if item(ui, None, "Paste Names...", "") {
                a.dialog = Some(Dialog::NameManager(None));
                return true;
            }
            false
        }
        MenuKind::ChartLayout => {
            let Some(i) = a.chart_sel else { return true };
            let c = a.sheet().charts[i].clone();
            let toggles: [(&str, bool, fn(&mut Chart)); 5] = [
                ("Chart Title", !c.title.is_empty(), |c| c.title = if c.title.is_empty() { "Chart Title".into() } else { String::new() }),
                ("Legend", c.legend, |c| c.legend = !c.legend),
                ("Data Labels", c.data_labels, |c| c.data_labels = !c.data_labels),
                ("Gridlines", c.gridlines, |c| c.gridlines = !c.gridlines),
                ("Axis Titles", !c.x_title.is_empty() || !c.y_title.is_empty(), |c| {
                    if c.x_title.is_empty() && c.y_title.is_empty() {
                        c.x_title = "Axis Title".into();
                        c.y_title = "Axis Title".into();
                    } else {
                        c.x_title.clear();
                        c.y_title.clear();
                    }
                }),
            ];
            for (label, on, f) in toggles {
                if item_on(ui, None, label, "", true, on) {
                    a.act(|a| f(&mut a.wb.sheet_mut(a.cur).charts[i]));
                    return true;
                }
            }
            false
        }
        MenuKind::ChartStyle => {
            let Some(i) = a.chart_sel else { return true };
            let sets: [(&str, [u32; 4]); 5] = [
                ("Colorful", [0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000]),
                ("Blue", [0x2F5597, 0x4472C4, 0x8FAADC, 0xB4C7E7]),
                ("Green", [0x375623, 0x548235, 0x70AD47, 0xA9D18E]),
                ("Orange", [0x843C0C, 0xC55A11, 0xED7D31, 0xF4B183]),
                ("Gray", [0x262626, 0x595959, 0x7F7F7F, 0xBFBFBF]),
            ];
            for (label, cols) in sets {
                let (r, resp) = ui.allocate_exact_size(vec2(230.0, 26.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 3.0, HOVER);
                }
                for (k, c) in cols.iter().enumerate() {
                    ui.painter().rect_filled(R::from_min_size(r.min + vec2(8.0 + k as f32 * 18.0, 6.0), vec2(16.0, 14.0)), 0.0, theme::rgb(*c));
                }
                ui.painter().text(r.left_center() + vec2(86.0, 0.0), egui::Align2::LEFT_CENTER, label, theme::ui_font(12.5), TEXT);
                if resp.clicked() {
                    a.act(|a| {
                        for (k, s) in a.wb.sheet_mut(a.cur).charts[i].series.iter_mut().enumerate() {
                            s.color = if label == "Colorful" { None } else { Some(cols[k % 4]) };
                        }
                    });
                    return true;
                }
            }
            false
        }
        MenuKind::Unhide | MenuKind::Group | MenuKind::Breaks | MenuKind::Spell => true,
    }
}

fn cell_ctx(a: &mut App, ui: &mut egui::Ui, ctx: &egui::Context, which: u8) -> bool {
    if item(ui, Some(Icon::Cut), "Cut", "Ctrl+X") {
        a.copy(ctx, true);
        return true;
    }
    if item(ui, Some(Icon::Copy), "Copy", "Ctrl+C") {
        a.copy(ctx, false);
        return true;
    }
    header(ui, "Paste Options:");
    let mut po = None;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        for (ic, tip, what) in [(Icon::Paste, "Paste", PasteWhat::All), (Icon::Text, "Values", PasteWhat::Values), (Icon::Function, "Formulas", PasteWhat::Formulas), (Icon::Painter, "Formatting", PasteWhat::Formats)] {
            let (r, resp) = ui.allocate_exact_size(vec2(30.0, 28.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 3.0, HOVER);
            }
            crate::icons::draw(ui.painter(), r.shrink(5.0), ic, TEXT);
            if resp.on_hover_text(tip).clicked() {
                po = Some(what);
            }
        }
    });
    if let Some(w) = po {
        if a.clip.is_some() {
            a.paste_clip(PasteOpts { what: w, ..Default::default() });
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        return true;
    }
    if item_on(ui, None, "Paste Special...", "", a.clip.is_some(), false) {
        a.dialog = Some(Dialog::PasteSpecial(PasteOpts::default()));
        return true;
    }
    separator(ui);
    match which {
        1 => {
            if item(ui, Some(Icon::Insert), "Insert", "") {
                dialogs::insert_rows_cols(a, false);
                return true;
            }
            if item(ui, Some(Icon::Delete), "Delete", "") {
                dialogs::delete_rows_cols(a, false);
                return true;
            }
        }
        2 => {
            if item(ui, Some(Icon::Insert), "Insert", "") {
                dialogs::insert_rows_cols(a, true);
                return true;
            }
            if item(ui, Some(Icon::Delete), "Delete", "") {
                dialogs::delete_rows_cols(a, true);
                return true;
            }
        }
        _ => {
            if item(ui, Some(Icon::Insert), "Insert...", "") {
                a.dialog = Some(Dialog::InsertCells(0));
                return true;
            }
            if item(ui, Some(Icon::Delete), "Delete...", "") {
                a.dialog = Some(Dialog::DeleteCells(0));
                return true;
            }
        }
    }
    if item(ui, Some(Icon::Clear), "Clear Contents", "") {
        a.delete_contents();
        return true;
    }
    if which == 0 {
        separator(ui);
        ui.menu_button("      Filter", |ui| {
            if ui.button("Filter by Selected Cell's Value").clicked() {
                dialogs::filter_by_value(a);
                ui.close();
            }
            if ui.button("Clear Filter").clicked() {
                dialogs::clear_filter(a);
                ui.close();
            }
        });
        ui.menu_button("      Sort", |ui| {
            if ui.button("Sort A to Z").clicked() {
                dialogs::quick_sort(a, true);
                ui.close();
            }
            if ui.button("Sort Z to A").clicked() {
                dialogs::quick_sort(a, false);
                ui.close();
            }
            if ui.button("Custom Sort...").clicked() {
                a.dialog = Some(Dialog::sort(a));
                ui.close();
            }
        });
        separator(ui);
        let has_note = a.sheet().notes.contains_key(&a.sel().active);
        if item(ui, Some(Icon::Note), if has_note { "Edit Note" } else { "New Note" }, "Shift+F2") {
            a.dialog = Some(Dialog::note_for(a));
            return true;
        }
        if has_note && item(ui, Some(Icon::Cross), "Delete Note", "") {
            let p = a.sel().active;
            a.act(|a| {
                a.wb.sheet_mut(a.cur).notes.remove(&p);
            });
            return true;
        }
    }
    separator(ui);
    if item(ui, Some(Icon::Format), "Format Cells...", "Ctrl+1") {
        a.dialog = Some(Dialog::format_cells(a, 0));
        return true;
    }
    match which {
        1 => {
            if item(ui, None, "Column Width...", "") {
                a.dialog = Some(Dialog::Size { rows: false, value: format!("{:.2}", (a.sheet().col_px(a.sel().active.col) - 5.0).max(0.0) / 7.0) });
                return true;
            }
            if item(ui, Some(Icon::Hide), "Hide", "") {
                dialogs::hide_unhide(a, 1);
                return true;
            }
            if item(ui, None, "Unhide", "") {
                dialogs::hide_unhide(a, 4);
                return true;
            }
        }
        2 => {
            if item(ui, None, "Row Height...", "") {
                a.dialog = Some(Dialog::Size { rows: true, value: format!("{}", a.sheet().row_px(a.sel().active.row) * 0.75) });
                return true;
            }
            if item(ui, Some(Icon::Hide), "Hide", "") {
                dialogs::hide_unhide(a, 0);
                return true;
            }
            if item(ui, None, "Unhide", "") {
                dialogs::hide_unhide(a, 3);
                return true;
            }
        }
        _ => {
            if dialogs::list_items(a, a.sel().active).len() > 0 && item(ui, None, "Pick From Drop-down List...", "") {
                a.menu = Some(crate::app::Menu { kind: MenuKind::Validation, pos: ui.min_rect().left_top(), born: a.frame });
                return false;
            }
            if item(ui, Some(Icon::Names), "Define Name...", "") {
                a.dialog = Some(Dialog::define_name(a));
                return true;
            }
            if item(ui, Some(Icon::Link), "Link", "Ctrl+K") {
                a.dialog = Some(Dialog::hyperlink(a));
                return true;
            }
        }
    }
    false
}

fn cond_menu(a: &mut App, ui: &mut egui::Ui) -> bool {
    let mut close = false;
    let ranges = a.sel().ranges.clone();
    let add = |a: &mut App, rule: CondRule, style: CondStyle| {
        let rs = ranges.clone();
        a.act(|a| a.wb.sheet_mut(a.cur).cond.push(CondFormat { ranges: rs, rule, style, stop_if_true: false }));
    };
    let red = CondStyle { fill: Some(0xFFC7CE), color: Some(0x9C0006), bold: false, italic: false };
    ui.menu_button("      Highlight Cells Rules", |ui| {
        for (label, op) in [("Greater Than...", ">"), ("Less Than...", "<"), ("Between...", "between"), ("Equal To...", "=")] {
            if ui.button(label).clicked() {
                a.dialog = Some(Dialog::CondQuick { op: op.into(), a: String::new(), b: String::new(), style: 0 });
                ui.close();
                close = true;
            }
        }
        if ui.button("Text that Contains...").clicked() {
            a.dialog = Some(Dialog::CondQuick { op: "text".into(), a: String::new(), b: String::new(), style: 0 });
            ui.close();
            close = true;
        }
        if ui.button("Duplicate Values").clicked() {
            add(a, CondRule::Duplicate { unique: false }, red.clone());
            ui.close();
            close = true;
        }
        if ui.button("Blank Cells").clicked() {
            add(a, CondRule::Blanks { blank: true }, red.clone());
            ui.close();
            close = true;
        }
    });
    ui.menu_button("      Top/Bottom Rules", |ui| {
        let opts: [(&str, CondRule); 6] = [
            ("Top 10 Items", CondRule::TopBottom { top: true, rank: 10, percent: false }),
            ("Top 10%", CondRule::TopBottom { top: true, rank: 10, percent: true }),
            ("Bottom 10 Items", CondRule::TopBottom { top: false, rank: 10, percent: false }),
            ("Bottom 10%", CondRule::TopBottom { top: false, rank: 10, percent: true }),
            ("Above Average", CondRule::AboveAverage { above: true }),
            ("Below Average", CondRule::AboveAverage { above: false }),
        ];
        for (label, rule) in opts {
            if ui.button(label).clicked() {
                add(a, rule, red.clone());
                ui.close();
                close = true;
            }
        }
    });
    ui.menu_button("      Data Bars", |ui| {
        for (label, c) in [("Blue", 0x638EC6u32), ("Green", 0x63BE7B), ("Red", 0xF8696B), ("Orange", 0xFFB628), ("Light Blue", 0x008AEF), ("Purple", 0xD6007B)] {
            if ui.button(label).clicked() {
                add(a, CondRule::DataBar { color: c }, CondStyle::default());
                ui.close();
                close = true;
            }
        }
    });
    ui.menu_button("      Color Scales", |ui| {
        let sets: [(&str, Vec<u32>); 4] = [
            ("Green - Yellow - Red", vec![0x63BE7B, 0xFFEB84, 0xF8696B]),
            ("Red - Yellow - Green", vec![0xF8696B, 0xFFEB84, 0x63BE7B]),
            ("White - Red", vec![0xFFFFFF, 0xF8696B]),
            ("Green - White", vec![0x63BE7B, 0xFFFFFF]),
        ];
        for (label, cols) in sets {
            if ui.button(label).clicked() {
                add(a, CondRule::ColorScale { colors: cols }, CondStyle::default());
                ui.close();
                close = true;
            }
        }
    });
    ui.menu_button("      Icon Sets", |ui| {
        for (label, set) in [("3 Arrows", IconSet::Arrows3), ("3 Traffic Lights", IconSet::Lights3), ("3 Flags", IconSet::Flags3), ("3 Stars", IconSet::Stars3)] {
            if ui.button(label).clicked() {
                add(a, CondRule::IconSet { set }, CondStyle::default());
                ui.close();
                close = true;
            }
        }
    });
    separator(ui);
    if item(ui, None, "New Rule...", "") {
        a.dialog = Some(Dialog::CondQuick { op: "formula".into(), a: "=".into(), b: String::new(), style: 0 });
        return true;
    }
    if item(ui, None, "Clear Rules from Selected Cells", "") {
        let rs = ranges.clone();
        a.act(|a| {
            let sh = a.wb.sheet_mut(a.cur);
            sh.cond.retain(|c| !c.ranges.iter().any(|r| rs.iter().any(|s| s.intersects(r))));
        });
        return true;
    }
    if item(ui, None, "Clear Rules from Entire Sheet", "") {
        a.act(|a| a.wb.sheet_mut(a.cur).cond.clear());
        return true;
    }
    if item(ui, None, "Manage Rules...", "") {
        a.dialog = Some(Dialog::CondManager(0));
        return true;
    }
    close
}
