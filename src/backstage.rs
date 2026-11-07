//! The File tab's full-window pages: Home, New, Open, Info, Save As, Print, Export and About.

use crate::app::{App, Page};
use crate::icons::{self, Icon};
use crate::print::{self, Op};
use crate::theme::{self, GREEN, TEXT, TEXT_DIM, WHITE};
use egui::{pos2, vec2, Align2, Color32, Rect as R, RichText, Sense, Stroke, StrokeKind};

const NAV_W: f32 = 200.0;

pub fn show(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let Some(page) = a.page else { return };
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, WHITE);
    let nav = R::from_min_size(rect.min, vec2(NAV_W, rect.height()));
    p.rect_filled(nav, 0.0, GREEN);
    // Back arrow.
    let back = R::from_min_size(nav.min + vec2(8.0, 10.0), vec2(36.0, 36.0));
    let resp = ui.interact(back, ui.id().with("bs-back"), Sense::click());
    if resp.hovered() {
        p.circle_filled(back.center(), 17.0, theme::GREEN_HOVER);
    }
    icons::draw(&p, R::from_center_size(back.center(), vec2(18.0, 18.0)), Icon::Left, WHITE);
    if resp.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        a.page = None;
        return;
    }
    let items: [(Option<Page>, Icon, &str); 11] = [
        (Some(Page::Home), Icon::Recent, "Home"),
        (Some(Page::New), Icon::New, "New"),
        (Some(Page::Open), Icon::Open, "Open"),
        (None, Icon::Save, "Save"),
        (Some(Page::SaveAs), Icon::Save, "Save As"),
        (Some(Page::Print), Icon::Print, "Print"),
        (Some(Page::Export), Icon::Export, "Export"),
        (None, Icon::Close, "Close"),
        (Some(Page::Info), Icon::Info, "Info"),
        (Some(Page::About), Icon::Help, "About"),
        (None, Icon::Close, ""),
    ];
    let mut y = nav.top() + 60.0;
    for (i, (pg, icon, label)) in items.iter().enumerate() {
        if label.is_empty() {
            continue;
        }
        if i == 8 {
            y = nav.bottom() - 2.0 * 40.0 - 10.0;
        }
        let r = R::from_min_size(pos2(nav.left(), y), vec2(NAV_W, 40.0));
        let resp = ui.interact(r, ui.id().with(("bs", i)), Sense::click());
        let on = *pg == Some(page);
        if on {
            p.rect_filled(r, 0.0, theme::GREEN_DARK);
        } else if resp.hovered() {
            p.rect_filled(r, 0.0, theme::GREEN_HOVER);
        }
        icons::draw(&p, R::from_center_size(pos2(r.left() + 28.0, r.center().y), vec2(16.0, 16.0)), *icon, WHITE);
        p.text(pos2(r.left() + 48.0, r.center().y), Align2::LEFT_CENTER, *label, theme::ui_font(13.5), WHITE);
        if resp.clicked() {
            match (pg, *label) {
                (Some(x), _) => {
                    a.page = Some(*x);
                    if *x == Page::Print {
                        a.print_sheet = a.cur;
                        a.print_page = 0;
                    }
                }
                (None, "Save") => {
                    if a.save() {
                        a.page = None;
                    }
                }
                _ => {
                    a.new_book();
                    a.page = None;
                }
            }
        }
        y += 40.0;
    }
    let body = R::from_min_max(pos2(nav.right() + 40.0, rect.top() + 24.0), rect.max - vec2(32.0, 16.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(body.expand(4.0));
    let ui = &mut child;
    ui.label(RichText::new(crate::chrome::page_title(page)).size(30.0).color(TEXT));
    ui.add_space(14.0);
    match page {
        Page::Home | Page::New => home(a, ui, page == Page::Home),
        Page::Open => open(a, ui),
        Page::Info => info(a, ui),
        Page::SaveAs => save_as(a, ui),
        Page::Print => print_page(a, ui),
        Page::Export => export(a, ui),
        Page::About => about(ui),
    }
}

fn tile(ui: &mut egui::Ui, label: &str, draw: impl FnOnce(&egui::Painter, R)) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(150.0, 140.0), Sense::click());
    let p = ui.painter();
    let thumb = R::from_min_size(r.min, vec2(150.0, 112.0));
    p.rect(thumb, 0.0, WHITE, Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { GREEN } else { theme::BORDER }), StrokeKind::Inside);
    draw(p, thumb.shrink(8.0));
    p.text(pos2(r.left(), thumb.bottom() + 8.0), Align2::LEFT_TOP, label, theme::ui_font(12.5), TEXT);
    resp.clicked()
}

fn mini_grid(p: &egui::Painter, r: R, header: Option<Color32>) {
    for k in 0..=6 {
        let y = r.top() + r.height() * k as f32 / 6.0;
        p.hline(r.left()..=r.right(), y, Stroke::new(1.0, theme::GRID));
    }
    for k in 0..=4 {
        let x = r.left() + r.width() * k as f32 / 4.0;
        p.vline(x, r.top()..=r.bottom(), Stroke::new(1.0, theme::GRID));
    }
    if let Some(c) = header {
        p.rect_filled(R::from_min_size(r.min, vec2(r.width(), r.height() / 6.0)), 0.0, c);
    }
}

fn home(a: &mut App, ui: &mut egui::Ui, with_recent: bool) {
    ui.horizontal(|ui| {
        if tile(ui, "Blank workbook", |p, r| mini_grid(p, r, None)) {
            a.new_book();
            a.page = None;
        }
        ui.add_space(12.0);
        if tile(ui, "Sales report", |p, r| {
            mini_grid(p, r, Some(theme::rgb(0x4472C4)));
            for (i, h) in [0.5f32, 0.8, 0.35, 0.65].iter().enumerate() {
                let x = r.left() + 14.0 + i as f32 * 26.0;
                p.rect_filled(R::from_min_max(pos2(x, r.bottom() - r.height() * 0.6 * h), pos2(x + 16.0, r.bottom() - 4.0)), 0.0, theme::rgb(0xED7D31));
            }
        }) {
            let wb = crate::sample::sample_book();
            a.new_count += 1;
            let n = format!("Book{}", a.new_count);
            a.load_wb(wb, None, n);
            a.page = None;
        }
        ui.add_space(12.0);
        if tile(ui, "Monthly budget", |p, r| mini_grid(p, r, Some(theme::rgb(0x70AD47)))) {
            a.new_count += 1;
            let n = format!("Book{}", a.new_count);
            a.load_wb(budget_book(), None, n);
            a.page = None;
        }
    });
    if with_recent {
        ui.add_space(24.0);
        recent_list(a, ui);
    }
}

fn budget_book() -> crate::model::Workbook {
    use crate::model::*;
    let mut wb = Workbook::default();
    let set = |wb: &mut Workbook, r: u32, c: u32, s: &str| {
        let _ = crate::ops::set_input(wb, 0, r, c, s);
    };
    set(&mut wb, 0, 0, "Monthly Budget");
    for (c, h) in ["Category", "Planned", "Actual", "Difference"].iter().enumerate() {
        set(&mut wb, 2, c as u32, h);
    }
    let rows = [("Rent", 1200.0, 1200.0), ("Groceries", 450.0, 512.4), ("Utilities", 180.0, 164.9), ("Transport", 120.0, 98.0), ("Fun", 150.0, 210.5), ("Savings", 400.0, 400.0)];
    for (i, (n, p, act)) in rows.iter().enumerate() {
        let r = 3 + i as u32;
        set(&mut wb, r, 0, n);
        set(&mut wb, r, 1, &p.to_string());
        set(&mut wb, r, 2, &act.to_string());
        set(&mut wb, r, 3, &format!("=B{0}-C{0}", r + 1));
    }
    set(&mut wb, 9, 0, "Total");
    for c in 1..4u32 {
        let n = col_name(c);
        set(&mut wb, 9, c, &format!("=SUM({n}4:{n}9)"));
    }
    crate::ops::style_ranges(&mut wb, 0, &[Rect::cell(Pos::new(0, 0))], &|s| {
        s.bold = true;
        s.size10 = 160;
    });
    crate::ops::style_ranges(&mut wb, 0, &[Rect::new(2, 0, 2, 3)], &|s| {
        s.bold = true;
        s.fill = Some(0x70AD47);
        s.color = Some(0xFFFFFF);
    });
    crate::ops::style_ranges(&mut wb, 0, &[Rect::new(3, 1, 9, 3)], &|s| s.num_fmt = "\"$\"#,##0.00".into());
    crate::ops::style_ranges(&mut wb, 0, &[Rect::new(9, 0, 9, 3)], &|s| {
        s.bold = true;
        s.borders[B_TOP] = Some(Border { style: LineStyle::Thin, color: 0 });
    });
    wb.sheet_mut(0).col_w.insert(0, 100.0);
    wb.sheet_mut(0).cond.push(CondFormat {
        ranges: vec![Rect::new(3, 3, 8, 3)],
        rule: CondRule::CellIs { op: "<".into(), a: "0".into(), b: String::new() },
        style: CondStyle { fill: Some(0xFFC7CE), color: Some(0x9C0006), bold: false, italic: false },
        stop_if_true: false,
    });
    wb
}

fn recent_list(a: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new("Recent").size(16.0).strong().color(TEXT));
    ui.add_space(6.0);
    if a.recent.is_empty() {
        ui.label(RichText::new("You haven't opened any files recently. Click Open to browse for a workbook.").color(TEXT_DIM));
        return;
    }
    let list = a.recent.clone();
    for path in list {
        let pb = std::path::PathBuf::from(&path);
        let name = pb.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or(path.clone());
        let dir = pb.parent().map(|x| x.display().to_string()).unwrap_or_default();
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 44.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, theme::HOVER);
        }
        icons::draw(ui.painter(), R::from_center_size(pos2(r.left() + 20.0, r.center().y), vec2(20.0, 20.0)), Icon::Sheet, GREEN);
        ui.painter().text(pos2(r.left() + 44.0, r.top() + 8.0), Align2::LEFT_TOP, &name, theme::ui_font(13.0), TEXT);
        ui.painter().text(pos2(r.left() + 44.0, r.bottom() - 8.0), Align2::LEFT_BOTTOM, &dir, theme::ui_font(11.0), TEXT_DIM);
        if resp.clicked() {
            a.open_path(pb);
        }
    }
}

fn big_button(ui: &mut egui::Ui, icon: Icon, label: &str, sub: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(360.0, 52.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, 0.0, theme::HOVER);
    }
    icons::draw(p, R::from_center_size(pos2(r.left() + 26.0, r.center().y), vec2(24.0, 24.0)), icon, TEXT);
    p.text(pos2(r.left() + 56.0, r.top() + 9.0), Align2::LEFT_TOP, label, theme::ui_font(14.0), TEXT);
    p.text(pos2(r.left() + 56.0, r.bottom() - 9.0), Align2::LEFT_BOTTOM, sub, theme::ui_font(11.5), TEXT_DIM);
    resp.clicked()
}

fn open(a: &mut App, ui: &mut egui::Ui) {
    if big_button(ui, Icon::Open, "Browse", "Open an Excel, OpenDocument or CSV file") {
        a.open_dialog();
    }
    ui.add_space(16.0);
    recent_list(a, ui);
}

fn info(a: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(&a.name).size(18.0).color(TEXT));
    ui.label(RichText::new(a.path.as_ref().map(|p| p.display().to_string()).unwrap_or("Not saved yet".into())).color(TEXT_DIM));
    ui.add_space(16.0);
    let cells: usize = a.wb.sheets.iter().map(|s| s.rows.values().map(|r| r.len()).sum::<usize>()).sum();
    egui::Grid::new("info").num_columns(2).spacing(vec2(30.0, 8.0)).show(ui, |ui| {
        let row = |ui: &mut egui::Ui, k: &str, v: String| {
            ui.label(RichText::new(k).color(TEXT_DIM));
            ui.label(v);
            ui.end_row();
        };
        row(ui, "Sheets", a.wb.sheets.len().to_string());
        row(ui, "Cells used", cells.to_string());
        row(ui, "Defined names", a.wb.names.len().to_string());
        row(ui, "Author", a.wb.author.clone());
        row(ui, "Changes", if a.modified { "Not saved".into() } else { "All changes saved".into() });
    });
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.label("Author:");
        ui.text_edit_singleline(&mut a.wb.author);
    });
    ui.add_space(16.0);
    let prot = a.sheet().protection.is_some();
    if big_button(ui, Icon::Lock, if prot { "Unprotect Sheet" } else { "Protect Workbook" }, if prot { "This sheet is protected. Click to unprotect it." } else { "Control what changes people can make to this sheet" }) {
        a.page = None;
        if prot {
            crate::dialogs::unprotect(a);
        } else {
            a.dialog = Some(crate::dialogs::Dialog::Protect { password: String::new(), select_locked: true });
        }
    }
    if big_button(ui, Icon::Find, "Check Workbook", "Look at formulas, notes and hidden sheets") {
        a.page = None;
        a.dialog = Some(crate::dialogs::Dialog::Stats);
    }
}

fn save_as(a: &mut App, ui: &mut egui::Ui) {
    if big_button(ui, Icon::Open, "Browse", "Choose where to save, and the file type") && a.save_as() {
        a.page = None;
    }
    if let Some(p) = a.path.clone() {
        ui.add_space(8.0);
        if big_button(ui, Icon::Save, "Save", &format!("Save to {}", p.display())) && a.save() {
            a.page = None;
        }
    }
    ui.add_space(16.0);
    ui.label(RichText::new("Tabula saves Excel workbooks (.xlsx), OpenDocument spreadsheets (.ods), CSV and tab-separated text, and PDF.").color(TEXT_DIM));
}

fn export(a: &mut App, ui: &mut egui::Ui) {
    if big_button(ui, Icon::Export, "Create PDF", "Save the active sheet as a PDF document") {
        a.export_pdf(false);
    }
    if big_button(ui, Icon::Export, "Create PDF of whole workbook", "Every visible sheet, one after another") {
        a.export_pdf(true);
    }
    ui.add_space(8.0);
    for (label, ext, desc) in [("OpenDocument Spreadsheet", "ods", "For LibreOffice and other apps"), ("CSV (Comma delimited)", "csv", "Saves only the active sheet"), ("Text (Tab delimited)", "tsv", "Saves only the active sheet")] {
        if big_button(ui, Icon::Save, label, desc) {
            let f = rfd::FileDialog::new().set_file_name(format!("{}.{ext}", a.name)).add_filter(label, &[ext]).save_file();
            if let Some(p) = f {
                if a.save_to(p) {
                    a.page = None;
                }
            }
        }
    }
}

fn about(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
    ui.painter().rect_filled(r, 8.0, GREEN);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, "T", theme::ui_bold(40.0), WHITE);
    ui.add_space(8.0);
    ui.label(RichText::new(format!("Tabula {}", env!("CARGO_PKG_VERSION"))).size(18.0).color(TEXT));
    ui.label(RichText::new("A spreadsheet for your desktop: formulas, charts, tables, PivotTables and more. Opens and saves Excel and OpenDocument files.").color(TEXT_DIM));
    ui.add_space(10.0);
    ui.label(RichText::new("Fonts: Carlito, Liberation and Noto, under the SIL Open Font License.").color(TEXT_DIM).size(11.5));
}

/// Paints one preview page's operations on screen.
pub fn paint_page(p: &egui::Painter, r: R, ops: &[Op], k: f32, a: &App, si: usize) {
    let tx = |x: f32, y: f32| pos2(r.left() + x * k, r.top() + y * k);
    for op in ops {
        match op {
            Op::Fill(b, c) => {
                p.rect_filled(R::from_min_max(tx(b[0], b[1]), tx(b[2], b[3])), 0.0, theme::rgb(*c));
            }
            Op::Line(l, w, c) => {
                p.line_segment([tx(l[0], l[1]), tx(l[2], l[3])], Stroke::new((w * k).max(0.5), theme::rgb(*c)));
            }
            Op::Text { x, y, size, face, bold, italic, color, text, clip } => {
                let fam = theme::cell_family(match face {
                    1 => "Liberation Serif",
                    2 => "Liberation Mono",
                    _ => "Liberation Sans",
                }, *bold, *italic);
                let font = egui::FontId::new((size * k).max(1.0), fam);
                let g = p.layout_no_wrap(text.clone(), font, theme::rgb(*color));
                let at = tx(*x, *y) - vec2(0.0, g.size().y * 0.8);
                let cp = match clip {
                    Some(c) => p.with_clip_rect(R::from_min_max(tx(c[0], c[1]), tx(c[2], c[3])).intersect(p.clip_rect())),
                    None => p.clone(),
                };
                cp.galley(at, g, theme::rgb(*color));
            }
            Op::Chart { index, rect, scale } => {
                if let Some(ch) = a.wb.sheets[si].charts.get(*index) {
                    let cr = R::from_min_max(tx(rect[0], rect[1]), tx(rect[2], rect[3]));
                    let z = cr.width() / ch.w;
                    let _ = scale;
                    crate::charts::draw(&p.with_clip_rect(r.intersect(p.clip_rect())), cr, ch, &a.wb, &a.calc, si, z);
                }
            }
        }
    }
}

fn print_page(a: &mut App, ui: &mut egui::Ui) {
    let full = ui.available_rect_before_wrap();
    let left = R::from_min_size(full.min, vec2(300.0, full.height()));
    let right = R::from_min_max(pos2(left.right() + 24.0, full.top()), full.max);
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(left).layout(egui::Layout::top_down(egui::Align::Min)));
    let lui = &mut lui;
    lui.spacing_mut().item_spacing = vec2(8.0, 8.0);
    let si = a.print_sheet.min(a.wb.sheets.len() - 1);
    if lui.add(egui::Button::new(RichText::new("   Print   ").size(14.0).color(WHITE)).fill(GREEN).min_size(vec2(100.0, 56.0))).clicked() {
        let cur = a.cur;
        a.cur = si;
        a.print_now(false);
        a.cur = cur;
    }
    lui.label(RichText::new("Printer").strong());
    lui.label(RichText::new("System default printer").color(TEXT_DIM));
    lui.label(RichText::new("Settings").strong());
    egui::ComboBox::from_id_salt("pr-sheet").width(280.0).selected_text(format!("Sheet: {}", a.wb.sheets[si].name)).show_ui(lui, |ui| {
        for (i, s) in a.wb.sheets.iter().enumerate() {
            if !s.hidden && ui.selectable_label(i == si, &s.name).clicked() {
                a.print_sheet = i;
                a.print_page = 0;
            }
        }
    });
    let mut ps = a.wb.sheets[si].page.clone();
    egui::ComboBox::from_id_salt("pr-orient").width(280.0).selected_text(if ps.landscape { "Landscape Orientation" } else { "Portrait Orientation" }).show_ui(lui, |ui| {
        ui.selectable_value(&mut ps.landscape, false, "Portrait Orientation");
        ui.selectable_value(&mut ps.landscape, true, "Landscape Orientation");
    });
    egui::ComboBox::from_id_salt("pr-paper").width(280.0).selected_text(print::PAPER_NAMES[ps.paper.min(3) as usize]).show_ui(lui, |ui| {
        for (i, n) in print::PAPER_NAMES.iter().enumerate() {
            ui.selectable_value(&mut ps.paper, i as u8, *n);
        }
    });
    let margin_name = if ps.margins == [0.25, 0.25, 0.75, 0.75] { "Narrow Margins" } else if ps.margins == [1.0, 1.0, 1.0, 1.0] { "Wide Margins" } else { "Normal Margins" };
    egui::ComboBox::from_id_salt("pr-margins").width(280.0).selected_text(margin_name).show_ui(lui, |ui| {
        if ui.selectable_label(margin_name == "Normal Margins", "Normal Margins").clicked() {
            ps.margins = [0.7, 0.7, 0.75, 0.75];
        }
        if ui.selectable_label(margin_name == "Wide Margins", "Wide Margins").clicked() {
            ps.margins = [1.0, 1.0, 1.0, 1.0];
        }
        if ui.selectable_label(margin_name == "Narrow Margins", "Narrow Margins").clicked() {
            ps.margins = [0.25, 0.25, 0.75, 0.75];
        }
    });
    egui::ComboBox::from_id_salt("pr-scale").width(280.0).selected_text(if ps.fit_width { "Fit All Columns on One Page" } else { "No Scaling" }).show_ui(lui, |ui| {
        ui.selectable_value(&mut ps.fit_width, false, "No Scaling");
        ui.selectable_value(&mut ps.fit_width, true, "Fit All Columns on One Page");
    });
    lui.checkbox(&mut ps.gridlines, "Print gridlines");
    if ps != a.wb.sheets[si].page {
        a.act(|a| a.wb.sheet_mut(si).page = ps);
        a.print_page = 0;
    }
    if lui.link("Page Setup").clicked() {
        a.cur = si;
        a.dialog = Some(crate::dialogs::Dialog::page_setup(a));
        a.page = None;
        return;
    }
    if lui.button("Export as PDF...").clicked() {
        let cur = a.cur;
        a.cur = si;
        a.export_pdf(false);
        a.cur = cur;
    }
    // Preview.
    let p = ui.painter().clone();
    p.rect_filled(right, 0.0, Color32::from_rgb(0xE6, 0xE6, 0xE6));
    a.recalc_if_needed();
    let Some(l) = print::layout(&a.wb, si) else {
        p.text(right.center(), Align2::CENTER_CENTER, "We didn't find anything to print.", theme::ui_font(14.0), TEXT_DIM);
        return;
    };
    let n = l.pages.len();
    a.print_page = a.print_page.min(n - 1);
    let area = right.shrink2(vec2(20.0, 40.0));
    let k = (area.width() / l.page_w).min(area.height() / l.page_h);
    let pr = R::from_center_size(area.center() - vec2(0.0, 10.0), vec2(l.page_w * k, l.page_h * k));
    p.rect_filled(pr.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(40));
    p.rect_filled(pr, 0.0, WHITE);
    let cond = crate::cond::compute(&a.wb, &a.calc, si, 0);
    let ops = print::page_ops(&a.wb, &a.calc, si, &l, a.print_page, &cond);
    paint_page(&p.with_clip_rect(pr), pr, &ops, k, a, si);
    // Page arrows.
    let bar_y = right.bottom() - 18.0;
    let prev = R::from_center_size(pos2(right.center().x - 60.0, bar_y), vec2(22.0, 22.0));
    let next = R::from_center_size(pos2(right.center().x + 60.0, bar_y), vec2(22.0, 22.0));
    if ui.interact(prev, ui.id().with("pp-prev"), Sense::click()).clicked() && a.print_page > 0 {
        a.print_page -= 1;
    }
    if ui.interact(next, ui.id().with("pp-next"), Sense::click()).clicked() && a.print_page + 1 < n {
        a.print_page += 1;
    }
    icons::draw(&p, prev, Icon::Left, if a.print_page > 0 { TEXT } else { theme::TEXT_OFF });
    icons::draw(&p, next, Icon::Right, if a.print_page + 1 < n { TEXT } else { theme::TEXT_OFF });
    p.text(pos2(right.center().x, bar_y), Align2::CENTER_CENTER, format!("{} of {}", a.print_page + 1, n), theme::ui_font(12.5), TEXT);
}
