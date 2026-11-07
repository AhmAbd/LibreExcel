//! The ribbon: tab row and the command groups of each tab.

use crate::app::{App, Menu, MenuKind, RTab, ViewMode};
use crate::dialogs::{self, Dialog};
use crate::icons::{self, Icon};
use crate::model::*;
use crate::theme::{self, *};
use egui::{pos2, vec2, Align2, Color32, Rect as R, Sense, Stroke, StrokeKind};
use std::cell::Cell as StdCell;

pub const TAB_H: f32 = 30.0;
pub const BODY_H: f32 = 96.0;

thread_local! {
    pub static LAST_FILL: StdCell<u32> = const { StdCell::new(0xFFFF00) };
    pub static LAST_FONT: StdCell<u32> = const { StdCell::new(0xFF0000) };
}

struct Rib<'u> {
    ui: &'u mut egui::Ui,
    x: f32,
    top: f32,
    group_x: f32,
    n: usize,
}

impl Rib<'_> {
    fn id(&mut self) -> egui::Id {
        self.n += 1;
        egui::Id::new(("rib", self.n))
    }

    fn hover_fill(&self, r: R, resp: &egui::Response, checked: bool) {
        let p = self.ui.painter();
        if checked {
            p.rect(r, 3.0, CHECKED, Stroke::new(1.0, Color32::from_rgb(0xB0, 0xB0, 0xB0)), StrokeKind::Inside);
        }
        if resp.is_pointer_button_down_on() {
            p.rect_filled(r, 3.0, PRESSED);
        } else if resp.hovered() {
            p.rect_filled(r, 3.0, HOVER);
        }
    }

    /// A tall button with a 32 pixel icon and a label under it.
    fn big(&mut self, icon: Icon, label: &str, tip: &str, arrow: bool) -> bool {
        let lines: Vec<&str> = label.split('\n').collect();
        let tw = lines.iter().map(|l| self.ui.fonts_mut(|f| f.layout_no_wrap(l.to_string(), theme::ui_font(12.0), TEXT).size().x)).fold(0.0, f32::max);
        let w = (tw + 12.0).max(44.0);
        let r = R::from_min_size(pos2(self.x, self.top + 2.0), vec2(w, 76.0));
        let id = self.id();
        let resp = self.ui.interact(r, id, Sense::click());
        self.hover_fill(r, &resp, false);
        let p = self.ui.painter();
        icons::draw(p, R::from_min_size(pos2(r.center().x - 16.0, r.top() + 4.0), vec2(32.0, 32.0)), icon, TEXT);
        for (i, l) in lines.iter().enumerate() {
            p.text(pos2(r.center().x, r.top() + 46.0 + i as f32 * 15.0), Align2::CENTER_CENTER, *l, theme::ui_font(12.0), TEXT);
        }
        if arrow {
            let y = r.top() + 46.0 + lines.len() as f32 * 15.0 - 2.0;
            icons::draw(p, R::from_center_size(pos2(r.center().x, y), vec2(10.0, 10.0)), Icon::Down, TEXT);
        }
        self.x += w + 2.0;
        resp.on_hover_text(tip).clicked()
    }

    /// A small button with an icon and optional label, placed at (x, row).
    fn small_at(&mut self, x: f32, row: usize, icon: Option<Icon>, label: &str, tip: &str, arrow: bool, checked: bool) -> (bool, f32) {
        let tw = if label.is_empty() { 0.0 } else { self.ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), theme::ui_font(12.0), TEXT).size().x) + 6.0 };
        let w = 24.0 + tw + if arrow { 12.0 } else { 0.0 } - if icon.is_none() { 18.0 } else { 0.0 };
        let r = R::from_min_size(pos2(x, self.top + 3.0 + row as f32 * 25.0), vec2(w, 24.0));
        let id = self.id();
        let resp = self.ui.interact(r, id, Sense::click());
        self.hover_fill(r, &resp, checked);
        let p = self.ui.painter();
        if let Some(ic) = icon {
            icons::draw(p, R::from_min_size(r.min + vec2(4.0, 4.0), vec2(16.0, 16.0)), ic, TEXT);
        }
        if !label.is_empty() {
            p.text(pos2(r.left() + if icon.is_some() { 24.0 } else { 6.0 }, r.center().y), Align2::LEFT_CENTER, label, theme::ui_font(12.0), TEXT);
        }
        if arrow {
            icons::draw(p, R::from_center_size(pos2(r.right() - 7.0, r.center().y), vec2(10.0, 10.0)), Icon::Down, TEXT);
        }
        let clicked = resp.clone().on_hover_text(tip).clicked();
        (clicked, w)
    }

    /// A small button whose arrow part opens a menu: returns (main clicked, arrow clicked, width).
    fn split_at(&mut self, x: f32, row: usize, icon: Icon, color_bar: Option<Color32>, tip: &str) -> (bool, bool, f32) {
        let r = R::from_min_size(pos2(x, self.top + 3.0 + row as f32 * 25.0), vec2(24.0, 24.0));
        let ar = R::from_min_size(pos2(x + 24.0, r.top()), vec2(12.0, 24.0));
        let id = self.id();
        let resp = self.ui.interact(r, id, Sense::click());
        let id2 = self.id();
        let aresp = self.ui.interact(ar, id2, Sense::click());
        if resp.hovered() || aresp.hovered() {
            self.ui.painter().rect_stroke(r.union(ar), 3.0, Stroke::new(1.0, Color32::from_rgb(0xC0, 0xC0, 0xC0)), StrokeKind::Inside);
        }
        self.hover_fill(r, &resp, false);
        self.hover_fill(ar, &aresp, false);
        let p = self.ui.painter();
        let ir = R::from_min_size(r.min + vec2(4.0, 3.0), vec2(16.0, 16.0));
        icons::draw(p, ir, icon, TEXT);
        if let Some(c) = color_bar {
            p.rect_filled(R::from_min_size(pos2(r.left() + 4.0, r.bottom() - 6.0), vec2(16.0, 3.5)), 0.0, c);
        }
        icons::draw(p, R::from_center_size(ar.center(), vec2(10.0, 10.0)), Icon::Down, TEXT);
        let main = resp.on_hover_text(tip).clicked();
        (main, aresp.clicked(), 36.0)
    }

    fn combo_rect(&self, x: f32, row: usize, w: f32) -> R {
        R::from_min_size(pos2(x, self.top + 4.0 + row as f32 * 25.0), vec2(w, 22.0))
    }

    /// Closes a group: separator line and its label, with an optional dialog launcher.
    fn end_group(&mut self, label: &str, launcher: bool) -> bool {
        let p = self.ui.painter();
        let label_y = self.top + BODY_H - 14.0;
        let lw = self.ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), theme::ui_font(11.5), TEXT_DIM).size().x) + (if launcher { 28.0 } else { 12.0 });
        self.x = self.x.max(self.group_x + lw);
        let center = (self.group_x + self.x) / 2.0;
        p.text(pos2(center, label_y), Align2::CENTER_CENTER, label, theme::ui_font(11.5), TEXT_DIM);
        let mut clicked = false;
        if launcher {
            let r = R::from_center_size(pos2(self.x - 6.0, label_y), vec2(14.0, 14.0));
            let id = self.id();
            let resp = self.ui.interact(r, id, Sense::click()).on_hover_text("More options");
            if resp.hovered() {
                self.ui.painter().rect_filled(r, 2.0, HOVER);
            }
            icons::draw(self.ui.painter(), r.shrink(1.0), Icon::Launcher, TEXT_DIM);
            clicked = resp.clicked();
        }
        self.x += 6.0;
        self.ui.painter().vline(self.x - 0.5, self.top + 6.0..=self.top + BODY_H - 6.0, Stroke::new(1.0, RIBBON_LINE));
        self.x += 6.0;
        self.group_x = self.x;
        clicked
    }
}

fn open(a: &mut App, kind: MenuKind, at: egui::Pos2) {
    a.menu = if a.menu.is_some_and(|m| m.kind == kind) { None } else { Some(Menu { kind, pos: at, born: a.frame }) };
}

fn pointer_below(ui: &egui::Ui) -> egui::Pos2 {
    let p = ui.input(|i| i.pointer.interact_pos()).unwrap_or_default();
    pos2(p.x - 12.0, (p.y + 14.0).max(TAB_H * 2.0 + BODY_H - 4.0))
}

pub fn tabs(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, RIBBON_BG);
    let mut x = rect.left() + 6.0;
    let in_table = a.sheet().tables.iter().any(|t| t.range.contains(a.sel().active));
    let mut list: Vec<(&str, Option<RTab>)> = vec![("File", None), ("Home", Some(RTab::Home)), ("Insert", Some(RTab::Insert)), ("Page Layout", Some(RTab::PageLayout)), ("Formulas", Some(RTab::Formulas)), ("Data", Some(RTab::Data)), ("Review", Some(RTab::Review)), ("View", Some(RTab::View)), ("Help", Some(RTab::Help))];
    if a.chart_sel.is_some() {
        list.push(("Chart Design", Some(RTab::ChartDesign)));
    }
    if in_table {
        list.push(("Table Design", Some(RTab::TableDesign)));
    }
    if a.tab == RTab::ChartDesign && a.chart_sel.is_none() || a.tab == RTab::TableDesign && !in_table {
        a.tab = RTab::Home;
    }
    for (label, t) in list {
        let active = t == Some(a.tab) && !a.ribbon_min;
        let contextual = matches!(t, Some(RTab::ChartDesign | RTab::TableDesign));
        let font = if active { theme::ui_bold(12.5) } else { theme::ui_font(12.5) };
        let tw = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font.clone(), TEXT).size().x) + 20.0;
        let r = R::from_min_size(pos2(x, rect.top() + 2.0), vec2(tw, rect.height() - 2.0));
        let resp = ui.interact(r, egui::Id::new(("tab", label)), Sense::click());
        let p = ui.painter();
        if resp.hovered() && !active {
            p.rect_filled(r.shrink2(vec2(0.0, 3.0)), 3.0, HOVER);
        }
        p.text(r.center(), Align2::CENTER_CENTER, label, font, if active || contextual { GREEN_DARK } else { TEXT });
        if active {
            p.hline(r.left() + 8.0..=r.right() - 8.0, r.bottom() - 2.0, Stroke::new(3.0, ACCENT));
        }
        if resp.clicked() {
            match t {
                None => {
                    if a.edit.is_some() {
                        a.commit_edit(false);
                    }
                    a.page = Some(crate::app::Page::Home);
                    a.menu = None;
                }
                Some(t) => {
                    if a.ribbon_min {
                        a.ribbon_min = false;
                    }
                    a.tab = t;
                }
            }
        }
        if resp.double_clicked() && t.is_some() {
            a.ribbon_min = !a.ribbon_min;
        }
        x += tw;
    }
    // Comments and Share on the right.
    let sr = R::from_min_size(pos2(rect.right() - 86.0, rect.top() + 3.0), vec2(78.0, rect.height() - 6.0));
    let resp = ui.interact(sr, egui::Id::new("share"), Sense::click()).on_hover_text("Share this workbook (opens Save As)");
    ui.painter().rect_filled(sr, 4.0, if resp.hovered() { GREEN_HOVER } else { GREEN });
    icons::draw(ui.painter(), R::from_min_size(sr.min + vec2(8.0, 4.0), vec2(16.0, 16.0)), Icon::Share, WHITE);
    ui.painter().text(sr.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, "Share", theme::ui_font(12.5), WHITE);
    if resp.clicked() {
        a.page = Some(crate::app::Page::SaveAs);
    }
    let cr = R::from_min_size(pos2(sr.left() - 108.0, sr.top()), vec2(100.0, sr.height()));
    let resp = ui.interact(cr, egui::Id::new("comments-btn"), Sense::click()).on_hover_text("Notes on this sheet");
    ui.painter().rect(cr, 4.0, if resp.hovered() { HOVER } else { WHITE }, Stroke::new(1.0, Color32::from_rgb(0xC8, 0xC8, 0xC8)), StrokeKind::Inside);
    icons::draw(ui.painter(), R::from_min_size(cr.min + vec2(8.0, 4.0), vec2(16.0, 16.0)), Icon::Comment, TEXT);
    ui.painter().text(cr.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, "Comments", theme::ui_font(12.5), TEXT);
    if resp.clicked() {
        a.dialog = Some(Dialog::NotesList);
    }
}

pub fn body(a: &mut App, ui: &mut egui::Ui, rect: R) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, RIBBON_BG);
    let card = rect.shrink2(vec2(6.0, 2.0));
    p.rect(card, 6.0, WHITE, Stroke::new(1.0, Color32::from_rgb(0xE1, 0xE1, 0xE1)), StrokeKind::Inside);
    let mut r = Rib { ui, x: card.left() + 6.0, top: card.top(), group_x: card.left() + 6.0, n: 0 };
    let clip = card.shrink(1.0);
    let saved_clip = r.ui.clip_rect();
    r.ui.set_clip_rect(clip);
    match a.tab {
        RTab::Home => home(a, &mut r),
        RTab::Insert => insert(a, &mut r),
        RTab::PageLayout => page_layout(a, &mut r),
        RTab::Formulas => formulas(a, &mut r),
        RTab::Data => data(a, &mut r),
        RTab::Review => review(a, &mut r),
        RTab::View => view(a, &mut r),
        RTab::Help => help(a, &mut r),
        RTab::ChartDesign => chart_design(a, &mut r),
        RTab::TableDesign => table_design(a, &mut r),
    }
    // Collapse arrow.
    let cr = R::from_min_size(pos2(card.right() - 22.0, card.bottom() - 20.0), vec2(18.0, 18.0));
    if crate::chrome::icon_button(r.ui, cr, Icon::Up, TEXT_DIM, HOVER, egui::Id::new("rib-collapse"), true).on_hover_text("Collapse the Ribbon (Ctrl+F1)").clicked() {
        a.ribbon_min = true;
    }
    r.ui.set_clip_rect(saved_clip);
}

fn cur_style(a: &App) -> std::sync::Arc<Style> {
    a.style_at(a.sel().active)
}

fn combo(r: &mut Rib, rect: R, id: &str, text: &str, items: &[String]) -> Option<String> {
    let mut pick = None;
    r.ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        egui::ComboBox::from_id_salt(id).width(rect.width() - 8.0).height(320.0).selected_text(egui::RichText::new(text).size(12.0)).show_ui(ui, |ui| {
            for it in items {
                if ui.selectable_label(it == text, it.as_str()).clicked() {
                    pick = Some(it.clone());
                }
            }
        });
    });
    pick
}

fn home(a: &mut App, r: &mut Rib) {
    let st = cur_style(a);
    // Clipboard.
    let px = r.x;
    let paste_r = R::from_min_size(pos2(px, r.top + 2.0), vec2(48.0, 76.0));
    if r.big(Icon::Paste, "Paste", "Paste (Ctrl+V)", true) {
        let pos = r.ui.input(|i| i.pointer.interact_pos()).unwrap_or_default();
        if pos.y > paste_r.top() + 40.0 {
            open(a, MenuKind::Paste, paste_r.left_bottom());
        } else if a.clip.is_some() {
            a.paste_clip(Default::default());
        } else {
            r.ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
    }
    let x = r.x;
    let (c1, w1) = r.small_at(x, 0, Some(Icon::Cut), "", "Cut (Ctrl+X)", false, false);
    let (c2, w2) = r.small_at(x, 1, Some(Icon::Copy), "", "Copy (Ctrl+C)", false, false);
    let (c3, w3) = r.small_at(x, 2, Some(Icon::Painter), "", "Format Painter: click to copy formatting, double-click to keep it on", false, a.painter_fmt.is_some());
    let ctx = r.ui.ctx().clone();
    if c1 {
        a.copy(&ctx, true);
    }
    if c2 {
        a.copy(&ctx, false);
    }
    if c3 {
        if a.painter_fmt.is_some() {
            a.painter_fmt = None;
        } else {
            let rc = a.sel().rect();
            let sticky = r.ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
            a.painter_fmt = Some((crate::ops::copy_rect(&a.wb, a.cur, Rect::new(rc.r1, rc.c1, rc.r2.min(rc.r1 + 200), rc.c2.min(rc.c1 + 50))), sticky));
        }
    }
    r.x += w1.max(w2).max(w3) + 4.0;
    r.end_group("Clipboard", false);
    // Font.
    let x = r.x;
    let names: Vec<String> = theme::FONT_NAMES.iter().map(|s| s.to_string()).collect();
    if let Some(f) = combo(r, r.combo_rect(x, 0, 140.0), "font-name", &st.font, &names) {
        a.format(move |s| s.font = f.as_str().into());
    }
    let sizes: Vec<String> = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72].iter().map(|s| s.to_string()).collect();
    let cur_size = format!("{}", st.size10 as f32 / 10.0);
    if let Some(s) = combo(r, r.combo_rect(x + 144.0, 0, 56.0), "font-size", &cur_size, &sizes) {
        let v: u16 = s.parse::<u16>().unwrap_or(11) * 10;
        a.format(move |st| st.size10 = v);
    }
    let (g1, _) = r.small_at(x + 202.0, 0, Some(Icon::Grow), "", "Increase Font Size (Ctrl+Shift+>)", false, false);
    let (g2, _) = r.small_at(x + 226.0, 0, Some(Icon::Shrink), "", "Decrease Font Size (Ctrl+Shift+<)", false, false);
    if g1 || g2 {
        let steps = [80u16, 90, 100, 110, 120, 140, 160, 180, 200, 220, 240, 260, 280, 360, 480, 720];
        let cur = st.size10;
        let next = if g1 { steps.iter().copied().find(|s| *s > cur).unwrap_or(cur + 100) } else { steps.iter().rev().copied().find(|s| *s < cur).unwrap_or(10.max(cur.saturating_sub(10))) };
        a.format(move |s| s.size10 = next);
    }
    let (b, _) = r.small_at(x, 1, Some(Icon::Bold), "", "Bold (Ctrl+B)", false, st.bold);
    let (i, _) = r.small_at(x + 25.0, 1, Some(Icon::Italic), "", "Italic (Ctrl+I)", false, st.italic);
    let (u, ua, _) = r.split_at(x + 50.0, 1, Icon::Underline, None, "Underline (Ctrl+U)");
    if b {
        let on = !st.bold;
        a.format(move |s| s.bold = on);
    }
    if i {
        let on = !st.italic;
        a.format(move |s| s.italic = on);
    }
    if u {
        let on = if st.underline > 0 { 0 } else { 1 };
        a.format(move |s| s.underline = on);
    }
    if ua {
        let on = if st.underline == 2 { 0 } else { 2 };
        a.format(move |s| s.underline = on);
    }
    let (bm, ba, _) = r.split_at(x + 92.0, 1, Icon::Borders, None, "Bottom Border");
    if bm {
        dialogs::apply_border_preset(a, 0);
    }
    if ba {
        open(a, MenuKind::Borders, pos2(x + 92.0, r.top + 52.0));
    }
    let fill = LAST_FILL.with(|c| c.get());
    let (fm, fa, _) = r.split_at(x + 132.0, 1, Icon::Fill, Some(theme::rgb(fill)), "Fill Color");
    if fm {
        a.format(move |s| s.fill = Some(fill));
    }
    if fa {
        open(a, MenuKind::FillColor, pos2(x + 132.0, r.top + 52.0));
    }
    let fc = LAST_FONT.with(|c| c.get());
    let (cm, ca, _) = r.split_at(x + 172.0, 1, Icon::FontColor, Some(theme::rgb(fc)), "Font Color");
    if cm {
        a.format(move |s| s.color = Some(fc));
    }
    if ca {
        open(a, MenuKind::FontColor, pos2(x + 172.0, r.top + 52.0));
    }
    r.x = x + 252.0;
    if r.end_group("Font", true) {
        a.dialog = Some(Dialog::format_cells(a, 2));
    }
    // Alignment.
    let x = r.x;
    let vals = [(Icon::AlignTop, VAlign::Top, "Top Align"), (Icon::AlignMiddle, VAlign::Center, "Middle Align"), (Icon::AlignBottom, VAlign::Bottom, "Bottom Align")];
    for (k, (ic, v, tip)) in vals.iter().enumerate() {
        let (c, _) = r.small_at(x + k as f32 * 25.0, 0, Some(*ic), "", tip, false, st.v == *v);
        if c {
            let v = *v;
            a.format(move |s| s.v = v);
        }
    }
    let hals = [(Icon::AlignLeft, HAlign::Left, "Align Left"), (Icon::AlignCenter, HAlign::Center, "Center"), (Icon::AlignRight, HAlign::Right, "Align Right")];
    for (k, (ic, h, tip)) in hals.iter().enumerate() {
        let (c, _) = r.small_at(x + k as f32 * 25.0, 1, Some(*ic), "", tip, false, st.h == *h);
        if c {
            let h = if st.h == *h { HAlign::General } else { *h };
            a.format(move |s| s.h = h);
        }
    }
    let (o1, o2, _) = r.split_at(x + 80.0, 0, Icon::Orientation, None, "Orientation");
    if o1 || o2 {
        open(a, MenuKind::Orientation, pos2(x + 80.0, r.top + 28.0));
    }
    let (d1, _) = r.small_at(x + 80.0, 1, Some(Icon::IndentDec), "", "Decrease Indent", false, false);
    let (d2, _) = r.small_at(x + 105.0, 1, Some(Icon::IndentInc), "", "Increase Indent", false, false);
    if d1 {
        a.format(|s| s.indent = s.indent.saturating_sub(1));
    }
    if d2 {
        a.format(|s| {
            s.indent = (s.indent + 1).min(15);
            if !matches!(s.h, HAlign::Left | HAlign::Right) {
                s.h = HAlign::Left;
            }
        });
    }
    let (wr, _) = r.small_at(x + 132.0, 0, Some(Icon::Wrap), "Wrap Text", "Wrap Text", false, st.wrap);
    if wr {
        let on = !st.wrap;
        a.format(move |s| s.wrap = on);
    }
    let (mm, mw) = r.small_at(x + 132.0, 1, Some(Icon::Merge), "Merge & Center", "Merge & Center", false, a.sheet().merge_at(a.sel().active).is_some());
    let ar = R::from_min_size(pos2(x + 132.0 + mw, r.top + 28.0), vec2(14.0, 24.0));
    let resp = r.ui.interact(ar, egui::Id::new("merge-arrow"), Sense::click());
    if resp.hovered() {
        r.ui.painter().rect_filled(ar, 2.0, HOVER);
    }
    icons::draw(r.ui.painter(), R::from_center_size(ar.center(), vec2(10.0, 10.0)), Icon::Down, TEXT);
    if mm {
        dialogs::merge_center(a);
    }
    if resp.clicked() {
        open(a, MenuKind::Merge, pos2(x + 132.0, r.top + 52.0));
    }
    r.x = x + 132.0 + mw + 18.0;
    if r.end_group("Alignment", true) {
        a.dialog = Some(Dialog::format_cells(a, 1));
    }
    // Number.
    let x = r.x;
    let label = dialogs::format_label(&st.num_fmt);
    let lr = r.combo_rect(x, 0, 130.0);
    let lresp = r.ui.interact(lr, egui::Id::new("numfmt-combo"), Sense::click());
    r.ui.painter().rect(lr, 2.0, if lresp.hovered() { Color32::from_rgb(0xF6, 0xF6, 0xF6) } else { WHITE }, Stroke::new(1.0, Color32::from_rgb(0xC0, 0xC0, 0xC0)), StrokeKind::Inside);
    r.ui.painter().text(lr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, label, theme::ui_font(12.0), TEXT);
    icons::draw(r.ui.painter(), R::from_center_size(pos2(lr.right() - 9.0, lr.center().y), vec2(10.0, 10.0)), Icon::Down, TEXT);
    if lresp.clicked() {
        open(a, MenuKind::NumFmt, lr.left_bottom());
    }
    let (cur1, _, _) = r.split_at(x, 1, Icon::Currency, None, "Accounting Number Format");
    if cur1 {
        a.format(|s| s.num_fmt = "_(\"$\"* #,##0.00_);_(\"$\"* \\(#,##0.00\\);_(\"$\"* \"-\"??_);_(@_)".into());
    }
    let (pc, _) = r.small_at(x + 38.0, 1, Some(Icon::Percent), "", "Percent Style (Ctrl+Shift+%)", false, false);
    let (cm2, _) = r.small_at(x + 62.0, 1, Some(Icon::Comma), "", "Comma Style", false, false);
    let (di, _) = r.small_at(x + 86.0, 1, Some(Icon::DecInc), "", "Increase Decimal", false, false);
    let (dd, _) = r.small_at(x + 110.0, 1, Some(Icon::DecDec), "", "Decrease Decimal", false, false);
    if pc {
        a.format(|s| s.num_fmt = "0%".into());
    }
    if cm2 {
        a.format(|s| s.num_fmt = "_(* #,##0.00_);_(* \\(#,##0.00\\);_(* \"-\"??_);_(@_)".into());
    }
    if di || dd {
        let shown = crate::ops::text_of(&a.wb, &a.calc, a.cur, a.sel().active.row, a.sel().active.col);
        let code = dialogs::change_decimals(&st.num_fmt, &shown, di);
        a.format(move |s| s.num_fmt = code.as_str().into());
    }
    r.x = x + 136.0;
    if r.end_group("Number", true) {
        a.dialog = Some(Dialog::format_cells(a, 0));
    }
    // Styles.
    let sx = r.x;
    if r.big(Icon::CondFormat, "Conditional\nFormatting", "Conditional Formatting", true) {
        open(a, MenuKind::CondFormat, pos2(sx, r.top + 80.0));
    }
    let sx = r.x;
    if r.big(Icon::FormatTable, "Format as\nTable", "Format as Table", true) {
        open(a, MenuKind::FormatTable, pos2(sx, r.top + 80.0));
    }
    let sx = r.x;
    if r.big(Icon::CellStyles, "Cell\nStyles", "Cell Styles", true) {
        open(a, MenuKind::CellStyles, pos2(sx, r.top + 80.0));
    }
    r.end_group("Styles", false);
    // Cells.
    for (ic, label, kind) in [(Icon::Insert, "Insert", MenuKind::Insert), (Icon::Delete, "Delete", MenuKind::Delete), (Icon::Format, "Format", MenuKind::Format)] {
        let sx = r.x;
        if r.big(ic, label, label, true) {
            open(a, kind, pos2(sx, r.top + 80.0));
        }
    }
    r.end_group("Cells", false);
    // Editing.
    let x = r.x;
    let (s1, s2, _) = r.split_at(x, 0, Icon::AutoSum, None, "AutoSum (Alt+=)");
    if s1 {
        a.autosum("SUM");
    }
    if s2 {
        open(a, MenuKind::AutoSum, pos2(x, r.top + 28.0));
    }
    let (f1, _) = r.small_at(x, 1, Some(Icon::FillDown), "Fill", "Fill", true, false);
    if f1 {
        open(a, MenuKind::FillDown, pos2(x, r.top + 52.0));
    }
    let (cl, _) = r.small_at(x, 2, Some(Icon::Clear), "Clear", "Clear", true, false);
    if cl {
        open(a, MenuKind::Clear, pos2(x, r.top + 77.0));
    }
    r.x = x + 74.0;
    let sx = r.x;
    if r.big(Icon::SortFilter, "Sort &\nFilter", "Sort & Filter", true) {
        open(a, MenuKind::SortFilter, pos2(sx, r.top + 80.0));
    }
    let sx = r.x;
    if r.big(Icon::Find, "Find &\nSelect", "Find & Select", true) {
        open(a, MenuKind::Find, pos2(sx, r.top + 80.0));
    }
    r.end_group("Editing", false);
}

fn insert(a: &mut App, r: &mut Rib) {
    if r.big(Icon::Pivot, "PivotTable", "Summarize data with a PivotTable", false) {
        a.dialog = Some(Dialog::create_pivot(a));
    }
    if r.big(Icon::Table, "Table", "Create a table (Ctrl+T)", false) {
        a.dialog = Some(Dialog::create_table(a));
    }
    r.end_group("Tables", false);
    let kinds = [
        (Icon::ChartColumn, "Column", ChartKind::Column),
        (Icon::ChartBar, "Bar", ChartKind::Bar),
        (Icon::ChartLine, "Line", ChartKind::Line),
        (Icon::ChartArea, "Area", ChartKind::Area),
        (Icon::ChartPie, "Pie", ChartKind::Pie),
        (Icon::ChartScatter, "Scatter", ChartKind::Scatter),
    ];
    for (ic, label, k) in kinds {
        let sx = r.x;
        if r.big(ic, label, &format!("Insert {label} Chart"), true) {
            let pos = r.ui.input(|i| i.pointer.interact_pos()).unwrap_or_default();
            if pos.y > r.top + 50.0 {
                open(a, MenuKind::Chart(k as u8), pos2(sx, r.top + 80.0));
            } else {
                crate::charts::insert_chart(a, k);
            }
        }
    }
    r.end_group("Charts", false);
    let x = r.x;
    let (l1, w1) = r.small_at(x, 0, Some(Icon::Sparkline), "Line", "Insert a line sparkline", false, false);
    let (l2, w2) = r.small_at(x, 1, Some(Icon::ChartColumn), "Column", "Insert a column sparkline", false, false);
    if l1 || l2 {
        a.dialog = Some(Dialog::sparkline(a, if l1 { 0 } else { 1 }));
    }
    r.x = x + w1.max(w2) + 4.0;
    r.end_group("Sparklines", false);
    if r.big(Icon::Link, "Link", "Insert Link (Ctrl+K)", false) {
        a.dialog = Some(Dialog::hyperlink(a));
    }
    r.end_group("Links", false);
    if r.big(Icon::Note, "Note", "Insert Note (Shift+F2)", false) {
        a.dialog = Some(Dialog::note_for(a));
    }
    r.end_group("Notes", false);
    if r.big(Icon::Symbol, "Symbol", "Insert Symbol", false) {
        a.dialog = Some(Dialog::Symbol);
    }
    r.end_group("Symbols", false);
}

fn page_layout(a: &mut App, r: &mut Rib) {
    for (ic, label, kind) in [(Icon::Margins, "Margins", MenuKind::Margins), (Icon::PageOrient, "Orientation", MenuKind::PageOrient), (Icon::PageSize, "Size", MenuKind::PageSize), (Icon::PrintArea, "Print\nArea", MenuKind::PrintArea)] {
        let sx = r.x;
        if r.big(ic, label, label, true) {
            open(a, kind, pos2(sx, r.top + 80.0));
        }
    }
    if r.big(Icon::Background, "Page\nSetup", "Page Setup", false) {
        a.dialog = Some(Dialog::page_setup(a));
    }
    if r.end_group("Page Setup", true) {
        a.dialog = Some(Dialog::page_setup(a));
    }
    let x = r.x;
    let scale = a.sheet().page.scale;
    r.ui.painter().text(pos2(x, r.top + 15.0), Align2::LEFT_CENTER, "Scale:", theme::ui_font(12.0), TEXT);
    let mut v = scale as i32;
    let resp = r.ui.put(R::from_min_size(pos2(x + 44.0, r.top + 4.0), vec2(70.0, 22.0)), egui::DragValue::new(&mut v).range(10..=400).suffix("%"));
    if resp.changed() {
        let v = v as u32;
        a.act(|a| a.wb.sheet_mut(a.cur).page.scale = v);
    }
    let mut fit = a.sheet().page.fit_width;
    if r.ui.put(R::from_min_size(pos2(x, r.top + 30.0), vec2(140.0, 22.0)), egui::Checkbox::new(&mut fit, "Fit to 1 page wide")).changed() {
        a.act(|a| a.wb.sheet_mut(a.cur).page.fit_width = fit);
    }
    r.x = x + 140.0;
    r.end_group("Scale to Fit", false);
    let x = r.x;
    let sh = a.sheet();
    let (mut gv, mut gp, mut hv, mut hp) = (sh.gridlines, sh.page.gridlines, sh.headings, sh.page.headings);
    let p = r.ui.painter();
    p.text(pos2(x + 30.0, r.top + 12.0), Align2::CENTER_CENTER, "Gridlines", theme::ui_font(12.0), TEXT);
    p.text(pos2(x + 110.0, r.top + 12.0), Align2::CENTER_CENTER, "Headings", theme::ui_font(12.0), TEXT);
    let c1 = r.ui.put(R::from_min_size(pos2(x, r.top + 26.0), vec2(70.0, 20.0)), egui::Checkbox::new(&mut gv, "View")).changed();
    let c2 = r.ui.put(R::from_min_size(pos2(x, r.top + 48.0), vec2(70.0, 20.0)), egui::Checkbox::new(&mut gp, "Print")).changed();
    let c3 = r.ui.put(R::from_min_size(pos2(x + 80.0, r.top + 26.0), vec2(70.0, 20.0)), egui::Checkbox::new(&mut hv, "View")).changed();
    let c4 = r.ui.put(R::from_min_size(pos2(x + 80.0, r.top + 48.0), vec2(70.0, 20.0)), egui::Checkbox::new(&mut hp, "Print")).changed();
    if c1 || c2 || c3 || c4 {
        a.act(|a| {
            let s = a.wb.sheet_mut(a.cur);
            s.gridlines = gv;
            s.page.gridlines = gp;
            s.headings = hv;
            s.page.headings = hp;
        });
    }
    r.x = x + 160.0;
    r.end_group("Sheet Options", false);
}

fn formulas(a: &mut App, r: &mut Rib) {
    if r.big(Icon::Function, "Insert\nFunction", "Insert Function (Shift+F3)", false) {
        a.dialog = Some(Dialog::insert_function(a));
    }
    let x = r.x;
    let cats = ["Financial", "Logical", "Text", "Date & Time", "Lookup & Reference", "Math & Trig", "Statistical", "Engineering", "Information", "Database"];
    let reg = crate::funcs::registry();
    for (k, cat) in cats.iter().enumerate() {
        let col = k / 3;
        let row = k % 3;
        let rect = R::from_min_size(pos2(x + col as f32 * 118.0, r.top + 3.0 + row as f32 * 25.0), vec2(114.0, 24.0));
        let mut names: Vec<&str> = reg.map.values().filter(|d| d.cat == *cat).map(|d| d.name).collect();
        names.sort();
        let label = match *cat {
            "Lookup & Reference" => "Lookup & Ref",
            x => x,
        };
        let mut chosen = None;
        r.ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.menu_button(egui::RichText::new(format!("{label} ▾")).size(12.0), |ui| {
                ui.set_min_width(170.0);
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for n in &names {
                        let resp = ui.button(*n);
                        let resp = if let Some(d) = reg.map.get(n) { resp.on_hover_text(format!("{}\n\n{}", d.sig, d.desc)) } else { resp };
                        if resp.clicked() {
                            chosen = Some(n.to_string());
                            ui.close();
                        }
                    }
                });
            });
        });
        if let Some(n) = chosen {
            dialogs::insert_function_text(a, &n);
        }
    }
    r.x = x + 4.0 * 118.0;
    r.end_group("Function Library", false);
    if r.big(Icon::Names, "Name\nManager", "Name Manager (Ctrl+F3)", false) {
        a.dialog = Some(Dialog::NameManager(None));
    }
    let x = r.x;
    let (d1, w1) = r.small_at(x, 0, Some(Icon::Names), "Define Name", "Define Name", false, false);
    let (d2, w2) = r.small_at(x, 1, Some(Icon::Function), "Use in Formula", "Use in Formula", true, false);
    let (d3, w3) = r.small_at(x, 2, Some(Icon::Table), "Create from Selection", "Create names from the top row of the selection", false, false);
    if d1 {
        a.dialog = Some(Dialog::define_name(a));
    }
    if d2 {
        open(a, MenuKind::Names, pos2(x, r.top + 52.0));
    }
    if d3 {
        dialogs::names_from_selection(a);
    }
    r.x = x + w1.max(w2).max(w3) + 4.0;
    r.end_group("Defined Names", false);
    let x = r.x;
    let (s1, w1) = r.small_at(x, 0, Some(Icon::ShowFormulas), "Show Formulas", "Show Formulas (Ctrl+`)", false, a.show_formulas);
    let (s2, w2) = r.small_at(x, 1, Some(Icon::Trace), "Trace Precedents", "Select the cells this formula uses", false, false);
    let (s3, w3) = r.small_at(x, 2, Some(Icon::Validate), "Error Checking", "Go to the next cell with an error", false, false);
    if s1 {
        a.show_formulas = !a.show_formulas;
    }
    if s2 {
        dialogs::trace_precedents(a);
    }
    if s3 {
        dialogs::next_error(a);
    }
    r.x = x + w1.max(w2).max(w3) + 4.0;
    r.end_group("Formula Auditing", false);
    if r.big(Icon::Calc, "Calculate\nNow", "Calculate Now (F9)", false) {
        a.touch_calc();
    }
    r.end_group("Calculation", false);
}

fn data(a: &mut App, r: &mut Rib) {
    if r.big(Icon::Open, "From\nText/CSV", "Import data from a text or CSV file into a new sheet", false) {
        dialogs::import_text(a);
    }
    if r.big(Icon::Refresh, "Refresh\nAll", "Recalculate everything and refresh PivotTables", false) {
        dialogs::refresh_all(a);
    }
    r.end_group("Get & Transform Data", false);
    let x = r.x;
    let (az, _) = r.small_at(x, 0, Some(Icon::SortAZ), "", "Sort A to Z", false, false);
    let (za, _) = r.small_at(x, 1, Some(Icon::SortZA), "", "Sort Z to A", false, false);
    if az || za {
        dialogs::quick_sort(a, az);
    }
    r.x = x + 30.0;
    if r.big(Icon::SortFilter, "Sort", "Sort", false) {
        a.dialog = Some(Dialog::sort(a));
    }
    let on = a.sheet().filter.is_some();
    let fx = r.x;
    if r.big(Icon::Filter, "Filter", "Filter (Ctrl+Shift+L)", false) {
        dialogs::toggle_filter(a);
    }
    if on {
        r.ui.painter().rect_stroke(R::from_min_size(pos2(fx, r.top + 2.0), vec2(44.0, 76.0)), 3.0, Stroke::new(1.0, Color32::from_rgb(0xB0, 0xB0, 0xB0)), StrokeKind::Inside);
    }
    let x = r.x;
    let (c1, w1) = r.small_at(x, 0, Some(Icon::FilterClear), "Clear", "Clear the filter", false, false);
    let (c2, w2) = r.small_at(x, 1, Some(Icon::Refresh), "Reapply", "Reapply the filter", false, false);
    if c1 {
        dialogs::clear_filter(a);
    }
    if c2 {
        a.act(|a| crate::ops::apply_filter(&mut a.wb, &a.calc, a.cur));
    }
    r.x = x + w1.max(w2) + 4.0;
    r.end_group("Sort & Filter", false);
    if r.big(Icon::TextCols, "Text to\nColumns", "Split text into columns", false) {
        a.dialog = Some(Dialog::TextToColumns { comma: true, tab: true, semicolon: false, space: false, other: String::new() });
    }
    if r.big(Icon::Dedupe, "Remove\nDuplicates", "Remove Duplicates", false) {
        a.dialog = Some(Dialog::remove_duplicates(a));
    }
    if r.big(Icon::Validate, "Data\nValidation", "Data Validation", false) {
        a.dialog = Some(Dialog::validation(a));
    }
    r.end_group("Data Tools", false);
    let sx = r.x;
    if r.big(Icon::WhatIf, "What-If\nAnalysis", "What-If Analysis", true) {
        open(a, MenuKind::WhatIf, pos2(sx, r.top + 80.0));
    }
    r.end_group("Forecast", false);
    let x = r.x;
    let (g1, w1) = r.small_at(x, 0, Some(Icon::Group), "Group", "Group rows or columns (Shift+Alt+Right)", false, false);
    let (g2, w2) = r.small_at(x, 1, Some(Icon::Ungroup), "Ungroup", "Ungroup rows or columns", false, false);
    let (g3, w3) = r.small_at(x, 2, Some(Icon::AutoSum), "Subtotal", "Insert subtotals for groups of rows", false, false);
    if g1 || g2 {
        dialogs::group_rows(a, g1);
    }
    if g3 {
        a.dialog = Some(Dialog::subtotal(a));
    }
    r.x = x + w1.max(w2).max(w3) + 4.0;
    r.end_group("Outline", false);
}

fn review(a: &mut App, r: &mut Rib) {
    if r.big(Icon::Info, "Workbook\nStatistics", "Counts of sheets, cells, formulas and charts", false) {
        a.dialog = Some(Dialog::Stats);
    }
    r.end_group("Proofing", false);
    if r.big(Icon::Note, "New\nNote", "New Note (Shift+F2)", false) {
        a.dialog = Some(Dialog::note_for(a));
    }
    let x = r.x;
    let (n1, w1) = r.small_at(x, 0, Some(Icon::Cross), "Delete Note", "Delete the note in the selected cell", false, false);
    let (n2, w2) = r.small_at(x, 1, Some(Icon::Comment), "All Notes", "List all notes on this sheet", false, false);
    if n1 {
        let rects = a.sel().ranges.clone();
        a.act(|a| a.wb.sheet_mut(a.cur).notes.retain(|p, _| !rects.iter().any(|r| r.contains(*p))));
    }
    if n2 {
        a.dialog = Some(Dialog::NotesList);
    }
    r.x = x + w1.max(w2) + 4.0;
    r.end_group("Notes", false);
    let protected = a.sheet().protection.is_some();
    if r.big(Icon::Protect, if protected { "Unprotect\nSheet" } else { "Protect\nSheet" }, "Prevent changes to locked cells", false) {
        if protected {
            dialogs::unprotect(a);
        } else {
            a.dialog = Some(Dialog::Protect { password: String::new(), select_locked: true });
        }
    }
    let wbp = a.wb.protected;
    if r.big(Icon::Lock, "Protect\nWorkbook", "Prevent adding, removing or renaming sheets", false) {
        a.act(|a| a.wb.protected = !wbp);
    }
    if wbp {
        r.ui.painter().rect_stroke(R::from_min_size(pos2(r.x - 62.0, r.top + 2.0), vec2(60.0, 76.0)), 3.0, Stroke::new(1.0, Color32::from_rgb(0xB0, 0xB0, 0xB0)), StrokeKind::Inside);
    }
    r.end_group("Protect", false);
}

fn view(a: &mut App, r: &mut Rib) {
    for (ic, label, m) in [(Icon::ViewNormal, "Normal", ViewMode::Normal), (Icon::ViewBreak, "Page Break\nPreview", ViewMode::Break), (Icon::ViewLayout, "Page\nLayout", ViewMode::Layout)] {
        let x0 = r.x;
        if r.big(ic, label, label, false) {
            a.view_mode = m;
        }
        if a.view_mode == m {
            r.ui.painter().rect_stroke(R::from_min_size(pos2(x0, r.top + 2.0), vec2(r.x - x0 - 2.0, 76.0)), 3.0, Stroke::new(1.0, Color32::from_rgb(0xB0, 0xB0, 0xB0)), StrokeKind::Inside);
        }
    }
    r.end_group("Workbook Views", false);
    let x = r.x;
    let mut grid = a.sheet().gridlines;
    let mut bar = a.show_bar;
    let mut heads = a.sheet().headings;
    let c1 = r.ui.put(R::from_min_size(pos2(x, r.top + 4.0), vec2(110.0, 20.0)), egui::Checkbox::new(&mut grid, "Gridlines")).changed();
    let c2 = r.ui.put(R::from_min_size(pos2(x, r.top + 28.0), vec2(110.0, 20.0)), egui::Checkbox::new(&mut bar, "Formula Bar")).changed();
    let c3 = r.ui.put(R::from_min_size(pos2(x, r.top + 52.0), vec2(110.0, 20.0)), egui::Checkbox::new(&mut heads, "Headings")).changed();
    if c1 || c3 {
        let s = a.wb.sheet_mut(a.cur);
        s.gridlines = grid;
        s.headings = heads;
        a.modified = true;
    }
    if c2 {
        a.show_bar = bar;
    }
    r.x = x + 112.0;
    r.end_group("Show", false);
    if r.big(Icon::Zoom, "Zoom", "Zoom", false) {
        a.dialog = Some(Dialog::Zoom((a.sheet().zoom * 100.0).round() as u32));
    }
    if r.big(Icon::Zoom100, "100%", "Zoom to 100%", false) {
        a.wb.sheet_mut(a.cur).zoom = 1.0;
    }
    if r.big(Icon::Find, "Zoom to\nSelection", "Zoom so the selection fills the window", false) {
        dialogs::zoom_to_selection(a);
    }
    r.end_group("Zoom", false);
    let sx = r.x;
    if r.big(Icon::Freeze, "Freeze\nPanes", "Freeze Panes", true) {
        open(a, MenuKind::Freeze, pos2(sx, r.top + 80.0));
    }
    let x = r.x;
    let (h1, w1) = r.small_at(x, 0, Some(Icon::Hide), "Hide Sheet", "Hide the current sheet", false, false);
    let (h2, w2) = r.small_at(x, 1, Some(Icon::Sheet), "Unhide Sheet", "Unhide a sheet", false, false);
    if h1 {
        dialogs::hide_sheet(a);
    }
    if h2 {
        a.dialog = Some(Dialog::Unhide(0));
    }
    r.x = x + w1.max(w2) + 4.0;
    r.end_group("Window", false);
}

fn help(a: &mut App, r: &mut Rib) {
    if r.big(Icon::Help, "Help", "About this program", false) {
        a.page = Some(crate::app::Page::About);
    }
    if r.big(Icon::Text, "Keyboard\nShortcuts", "List of keyboard shortcuts", false) {
        a.dialog = Some(Dialog::Shortcuts);
    }
    if r.big(Icon::Function, "Function\nReference", "Browse all functions", false) {
        a.dialog = Some(Dialog::insert_function(a));
    }
    r.end_group("Help", false);
}

fn chart_design(a: &mut App, r: &mut Rib) {
    let Some(i) = a.chart_sel else { return };
    let sx = r.x;
    if r.big(Icon::Charts, "Add Chart\nElement", "Add Chart Element", true) {
        open(a, MenuKind::ChartLayout, pos2(sx, r.top + 80.0));
    }
    let sx = r.x;
    if r.big(Icon::Fill, "Change\nColors", "Change the colours of the series", true) {
        open(a, MenuKind::ChartStyle, pos2(sx, r.top + 80.0));
    }
    r.end_group("Chart Layouts", false);
    if r.big(Icon::TextCols, "Switch\nRow/Column", "Swap rows and columns in the chart", false) {
        crate::charts::switch_rows_cols(a, i);
    }
    if r.big(Icon::Table, "Select\nData", "Change the data range", false) {
        a.dialog = Some(Dialog::chart_data(a, i));
    }
    r.end_group("Data", false);
    let sx = r.x;
    if r.big(Icon::ChartColumn, "Change\nChart Type", "Change Chart Type", true) {
        open(a, MenuKind::Chart(100), pos2(sx, r.top + 80.0));
    }
    r.end_group("Type", false);
    if r.big(Icon::Delete, "Delete\nChart", "Delete the selected chart", false) {
        a.act(|a| {
            a.wb.sheet_mut(a.cur).charts.remove(i);
        });
        a.chart_sel = None;
    }
    r.end_group("Chart", false);
}

fn table_design(a: &mut App, r: &mut Rib) {
    let p = a.sel().active;
    let Some(ti) = a.sheet().tables.iter().position(|t| t.range.contains(p)) else { return };
    let t = a.sheet().tables[ti].clone();
    let x = r.x;
    r.ui.painter().text(pos2(x, r.top + 12.0), Align2::LEFT_CENTER, "Table Name:", theme::ui_font(12.0), TEXT);
    let mut name = t.name.clone();
    let resp = r.ui.put(R::from_min_size(pos2(x, r.top + 24.0), vec2(110.0, 22.0)), egui::TextEdit::singleline(&mut name).font(theme::ui_font(12.0)));
    if resp.lost_focus() && name != t.name && !name.trim().is_empty() {
        let n = name.trim().replace(' ', "_");
        a.act(|a| a.wb.sheet_mut(a.cur).tables[ti].name = n);
    }
    r.x = x + 116.0;
    r.end_group("Properties", false);
    let x = r.x;
    let (c1, w1) = r.small_at(x, 0, Some(Icon::Dedupe), "Remove Duplicates", "Remove duplicate rows in this table", false, false);
    let (c2, w2) = r.small_at(x, 1, Some(Icon::Table), "Convert to Range", "Turn the table back into normal cells", false, false);
    let (c3, w3) = r.small_at(x, 2, Some(Icon::Format), "Resize Table", "Change the table's range", false, false);
    if c1 {
        a.sel_mut().ranges = vec![t.range];
        a.dialog = Some(Dialog::remove_duplicates(a));
    }
    if c2 {
        a.act(|a| {
            let sh = a.wb.sheet_mut(a.cur);
            let tt = sh.tables.remove(ti);
            if sh.filter.as_ref().is_some_and(|f| f.range == tt.range) {
                sh.filter = None;
                sh.filtered_rows.clear();
            }
        });
    }
    if c3 {
        a.dialog = Some(Dialog::create_table(a));
    }
    r.x = x + w1.max(w2).max(w3) + 4.0;
    r.end_group("Tools", false);
    let x = r.x;
    let (mut h, mut tot, mut band) = (t.header, t.totals, t.banded);
    let c1 = r.ui.put(R::from_min_size(pos2(x, r.top + 4.0), vec2(110.0, 20.0)), egui::Checkbox::new(&mut h, "Header Row")).changed();
    let c2 = r.ui.put(R::from_min_size(pos2(x, r.top + 28.0), vec2(110.0, 20.0)), egui::Checkbox::new(&mut tot, "Total Row")).changed();
    let c3 = r.ui.put(R::from_min_size(pos2(x, r.top + 52.0), vec2(110.0, 20.0)), egui::Checkbox::new(&mut band, "Banded Rows")).changed();
    if c1 || c3 {
        a.act(|a| {
            let tt = &mut a.wb.sheet_mut(a.cur).tables[ti];
            tt.header = h;
            tt.banded = band;
        });
    }
    if c2 {
        dialogs::table_totals(a, ti, tot);
    }
    r.x = x + 112.0;
    r.end_group("Table Style Options", false);
    let x = r.x;
    for k in 0..12u8 {
        let rr = R::from_min_size(pos2(x + (k % 6) as f32 * 40.0, r.top + 6.0 + (k / 6) as f32 * 34.0), vec2(36.0, 30.0));
        let resp = r.ui.interact(rr, egui::Id::new(("tstyle", k)), Sense::click());
        let st = k + 1;
        dialogs::table_swatch(r.ui.painter(), rr, st, resp.hovered() || t.style == st);
        if resp.clicked() {
            a.act(|a| a.wb.sheet_mut(a.cur).tables[ti].style = st);
        }
    }
    r.x = x + 6.0 * 40.0;
    r.end_group("Table Styles", false);
}
