#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backstage;
mod charts;
mod chrome;
mod cond;
mod dialog_tabs;
mod dialog_ui;
mod dialogs;
mod eval;
mod formula;
mod funcs;
mod funcs_fin;
mod funcs_look;
mod funcs_math;
mod funcs_stat;
mod funcs_text;
mod grid;
mod grid_input;
mod icons;
mod menus;
mod model;
mod numfmt;
mod ods;
mod ops;
mod print;
mod ribbon;
mod sample;
mod theme;
mod xlsx_read;
mod xlsx_write;
mod xml;

#[cfg(test)]
mod engine_tests;

use app::App;
use egui::{pos2, vec2, Rect as R};
use std::path::PathBuf;

const TITLE_H: f32 = 32.0;
const SHEET_TABS_H: f32 = 28.0;
const STATUS_H: f32 = 24.0;

static SHOT: std::sync::LazyLock<std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<egui::ColorImage>>>>> = std::sync::LazyLock::new(Default::default);

struct Tabula {
    a: App,
    started: bool,
}

impl Tabula {
    fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx);
        let mut a = App::new(shot.clone(), open);
        if shot.is_none() {
            if let Some(s) = cc.storage {
                if let Some(r) = s.get_string("recent") {
                    a.recent = r.lines().filter(|l| !l.is_empty()).map(String::from).collect();
                }
                a.autosave = s.get_string("autosave").as_deref() == Some("1");
            }
        }
        // Screenshot runs can open a page or dialog first, to check how it looks.
        if a.shot.is_some() {
            match std::env::var("TABULA_SHOT_VIEW").as_deref() {
                Ok("print") => a.page = Some(app::Page::Print),
                Ok("home") => a.page = Some(app::Page::Home),
                Ok("format") => a.dialog = Some(dialogs::Dialog::format_cells(&a, 0)),
                Ok("sort") => a.dialog = Some(dialogs::Dialog::sort(&a)),
                _ => {}
            }
        }
        Tabula { a, started: false }
    }

    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, n)) = self.a.shot.clone() else { return };
        // Let fonts, layout and the first recalculation settle before taking the picture.
        if n == 8 {
            let slot = SHOT.clone();
            let c2 = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                *slot.lock().unwrap() = Some(img);
                c2.request_repaint();
            })));
        }
        let shot = SHOT.lock().unwrap().take();
        if let Some(img) = shot {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&[p.r(), p.g(), p.b(), p.a()]);
            }
            if let Some(im) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
                let _ = im.save(&path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            self.a.shot = None;
            return;
        }
        if n > 200 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.a.shot = Some((path, n + 1));
        ctx.request_repaint();
    }
}

impl eframe::App for Tabula {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let a = &mut self.a;
        a.frame += 1;
        if !self.started {
            self.started = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(a.title()));
        }
        a.recalc_if_needed();
        chrome::run_autofit(a, &ctx);
        a.keys(&ctx);
        a.recalc_if_needed();

        let full = ctx.content_rect();
        ui.painter().rect_filled(full, 0.0, theme::RIBBON_BG);
        let mut y = full.top();
        let title = R::from_min_size(pos2(full.left(), y), vec2(full.width(), TITLE_H));
        chrome::title_bar(a, ui, title);
        y += TITLE_H;
        if a.page.is_some() {
            let rest = R::from_min_max(pos2(full.left(), y), full.max);
            backstage::show(a, ui, rest);
        } else {
            let tabs = R::from_min_size(pos2(full.left(), y), vec2(full.width(), ribbon::TAB_H));
            ribbon::tabs(a, ui, tabs);
            y += ribbon::TAB_H;
            if !a.ribbon_min {
                let body = R::from_min_size(pos2(full.left(), y), vec2(full.width(), ribbon::BODY_H));
                ribbon::body(a, ui, body);
                y += ribbon::BODY_H;
            }
            if a.show_bar {
                let h = if a.bar_tall { 96.0 } else { 30.0 };
                let bar = R::from_min_size(pos2(full.left(), y + 2.0), vec2(full.width(), h));
                chrome::formula_bar(a, ui, bar);
                y += h + 2.0;
            }
            let status = R::from_min_max(pos2(full.left(), full.bottom() - STATUS_H), full.max);
            let tabs_r = R::from_min_max(pos2(full.left(), status.top() - SHEET_TABS_H), pos2(full.right(), status.top()));
            let grid = R::from_min_max(pos2(full.left(), y), pos2(full.right(), tabs_r.top()));
            grid_input::show(a, ui, grid);
            chrome::sheet_tabs(a, ui, tabs_r);
            chrome::status_bar(a, ui, status);
        }
        chrome::menus(a, &ctx);
        dialog_ui::show(a, &ctx);
        chrome::alert(a, &ctx);

        // Autosave a few seconds after the last change, when the book has a file.
        let now = ctx.input(|i| i.time);
        if a.modified && a.last_change == 0.0 {
            a.last_change = now;
        }
        if a.autosave && a.modified && a.path.is_some() && a.edit.is_none() && now - a.last_change > 3.0 {
            a.save();
            a.last_change = 0.0;
        }
        if !a.modified {
            a.last_change = 0.0;
        }
        let t = a.title();
        if ctx.cumulative_frame_nr() % 30 == 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t));
        }
        self.screenshot(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.a.shot.is_none() {
            storage.set_string("recent", self.a.recent.join("\n"));
            storage.set_string("autosave", if self.a.autosave { "1".into() } else { "0".into() });
        }
    }
}

fn main() -> eframe::Result {
    let mut shot = None;
    let mut open = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            shot = args.next().map(PathBuf::from);
        } else if !arg.starts_with("--") {
            open = Some(PathBuf::from(arg));
        }
    }
    let icon = egui::IconData { rgba: icons::app_icon_rgba(64), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Book1 - Tabula")
            .with_app_id("tabula")
            .with_decorations(false)
            .with_inner_size([1366.0, 800.0])
            .with_min_inner_size([720.0, 480.0])
            .with_icon(icon),
        persist_window: shot.is_none(),
        ..Default::default()
    };
    eframe::run_native("Tabula", options, Box::new(move |cc| Ok(Box::new(Tabula::new(cc, shot, open)))))
}
