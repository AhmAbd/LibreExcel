//! Colours, fonts and small drawing helpers shared by the whole window.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

pub const GREEN: Color32 = Color32::from_rgb(0x21, 0x73, 0x46);
pub const GREEN_DARK: Color32 = Color32::from_rgb(0x18, 0x5C, 0x37);
pub const GREEN_HOVER: Color32 = Color32::from_rgb(0x2E, 0x83, 0x55);
pub const ACCENT: Color32 = Color32::from_rgb(0x10, 0x7C, 0x41);
pub const RIBBON_BG: Color32 = Color32::from_rgb(0xF3, 0xF3, 0xF3);
pub const RIBBON_LINE: Color32 = Color32::from_rgb(0xD6, 0xD6, 0xD6);
pub const HOVER: Color32 = Color32::from_rgb(0xE1, 0xE1, 0xE1);
pub const PRESSED: Color32 = Color32::from_rgb(0xC8, 0xC8, 0xC8);
pub const CHECKED: Color32 = Color32::from_rgb(0xD2, 0xD2, 0xD2);
pub const TEXT: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x60, 0x60, 0x60);
pub const TEXT_OFF: Color32 = Color32::from_rgb(0xA6, 0xA6, 0xA6);
pub const HEADER_BG: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
pub const HEADER_SEL: Color32 = Color32::from_rgb(0xD3, 0xD3, 0xD3);
pub const HEADER_ALL: Color32 = Color32::from_rgb(0xBF, 0xBF, 0xBF);
pub const HEADER_LINE: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);
pub const HEADER_TEXT: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
pub const GRID: Color32 = Color32::from_rgb(0xDA, 0xDA, 0xDA);
pub const SEL_FILL: Color32 = Color32::from_rgba_premultiplied(18, 18, 18, 22);
pub const WHITE: Color32 = Color32::WHITE;
pub const BORDER: Color32 = Color32::from_rgb(0xAB, 0xAB, 0xAB);
pub const BLUE: Color32 = Color32::from_rgb(0x2B, 0x57, 0x9A);
pub const LINK: Color32 = Color32::from_rgb(0x05, 0x63, 0xC1);
pub const RED: Color32 = Color32::from_rgb(0xC4, 0x2B, 0x1C);

/// Colours used for references while a formula is being typed.
pub const REF_COLORS: [Color32; 8] = [
    Color32::from_rgb(0x41, 0x72, 0xC4),
    Color32::from_rgb(0xC0, 0x50, 0x4D),
    Color32::from_rgb(0x8C, 0x5F, 0xB8),
    Color32::from_rgb(0x2E, 0x8B, 0x57),
    Color32::from_rgb(0x9C, 0x65, 0x00),
    Color32::from_rgb(0x00, 0x8B, 0x8B),
    Color32::from_rgb(0xC7, 0x15, 0x85),
    Color32::from_rgb(0x55, 0x6B, 0x2F),
];

/// The theme palette shown in colour pickers: ten columns, a top row and five shades each.
pub const THEME_TOP: [u32; 10] = [0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47];
pub const STANDARD: [u32; 10] = [0xC00000, 0xFF0000, 0xFFC000, 0xFFFF00, 0x92D050, 0x00B050, 0x00B0F0, 0x0070C0, 0x002060, 0x7030A0];
/// Series colours for charts.
pub const SERIES: [u32; 10] = [0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47, 0x264478, 0x9E480E, 0x636363, 0x997300];

pub fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn to_u32(c: Color32) -> u32 {
    ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32
}

/// Lighter (t > 0) or darker (t < 0) version of a colour, as the palette shades are made.
pub fn tint(c: u32, t: f32) -> u32 {
    let f = |x: u32| -> u32 {
        let x = x as f32;
        let y = if t >= 0.0 { x + (255.0 - x) * t } else { x * (1.0 + t) };
        y.round().clamp(0.0, 255.0) as u32
    };
    (f(c >> 16 & 255) << 16) | (f(c >> 8 & 255) << 8) | f(c & 255)
}

pub fn shades(c: u32) -> [u32; 5] {
    let lum = ((c >> 16 & 255) * 3 + (c >> 8 & 255) * 6 + (c & 255)) / 10;
    if lum > 230 {
        [tint(c, -0.05), tint(c, -0.15), tint(c, -0.25), tint(c, -0.35), tint(c, -0.5)]
    } else if lum < 20 {
        [tint(c, 0.5), tint(c, 0.35), tint(c, 0.25), tint(c, 0.15), tint(c, 0.05)]
    } else {
        [tint(c, 0.8), tint(c, 0.6), tint(c, 0.4), tint(c, -0.25), tint(c, -0.5)]
    }
}

pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn ui_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("ui-bold".into()))
}

/// Fonts the cell font box offers. Each maps to a bundled face with the same metrics or look.
pub const FONT_NAMES: [&str; 14] = [
    "Calibri", "Calibri Light", "Arial", "Arial Narrow", "Cambria", "Candara", "Consolas", "Courier New", "Georgia", "Helvetica", "Segoe UI", "Tahoma", "Times New Roman", "Verdana",
];

/// The bundled family used for a font name and style.
pub fn cell_family(name: &str, bold: bool, italic: bool) -> FontFamily {
    let n = name.to_ascii_lowercase();
    let base = if n.contains("courier") || n.contains("consolas") || n.contains("mono") {
        "mono"
    } else if n.contains("times") || n.contains("cambria") || n.contains("georgia") || n.contains("serif") && !n.contains("sans") || n.contains("garamond") {
        "serif"
    } else if n.contains("arial") || n.contains("helvetica") || n.contains("verdana") || n.contains("tahoma") || n.contains("segoe") || n.contains("liberation sans") {
        "sans"
    } else {
        "carlito"
    };
    let suffix = match (bold, italic) {
        (false, false) => "",
        (true, false) => "-b",
        (false, true) => "-i",
        (true, true) => "-bi",
    };
    FontFamily::Name(format!("{base}{suffix}").into())
}

/// Cell font size in screen points: Excel sizes are typographic points at 96 dots per inch.
pub fn cell_px(size10: u16, zoom: f32) -> f32 {
    size10 as f32 / 10.0 * 96.0 / 72.0 * zoom
}

macro_rules! font {
    ($f:literal) => {
        include_bytes!(concat!("../assets/fonts/", $f))
    };
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let mut add = |key: &str, data: &'static [u8]| {
        defs.font_data.insert(key.to_string(), Arc::new(FontData::from_static(data)));
    };
    add("noto", font!("NotoSans-Regular.ttf"));
    add("noto-bold", font!("NotoSans-Bold.ttf"));
    add("symbols", font!("NotoSansSymbols2-Regular.ttf"));
    add("carlito", font!("Carlito-Regular.ttf"));
    add("carlito-b", font!("Carlito-Bold.ttf"));
    add("carlito-i", font!("Carlito-Italic.ttf"));
    add("carlito-bi", font!("Carlito-BoldItalic.ttf"));
    add("sans", font!("LiberationSans-Regular.ttf"));
    add("sans-b", font!("LiberationSans-Bold.ttf"));
    add("sans-i", font!("LiberationSans-Italic.ttf"));
    add("sans-bi", font!("LiberationSans-BoldItalic.ttf"));
    add("serif", font!("LiberationSerif-Regular.ttf"));
    add("serif-b", font!("LiberationSerif-Bold.ttf"));
    add("serif-i", font!("LiberationSerif-Italic.ttf"));
    add("serif-bi", font!("LiberationSerif-BoldItalic.ttf"));
    add("mono", font!("LiberationMono-Regular.ttf"));
    add("mono-b", font!("LiberationMono-Bold.ttf"));
    add("mono-i", font!("LiberationMono-Italic.ttf"));
    add("mono-bi", font!("LiberationMono-BoldItalic.ttf"));
    let fallback: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut chain = |first: &str| {
        let mut v = vec![first.to_string(), "noto".to_string(), "symbols".to_string()];
        v.extend(fallback.iter().cloned());
        v
    };
    let prop = chain("noto");
    defs.families.insert(FontFamily::Proportional, prop);
    defs.families.insert(FontFamily::Name("ui-bold".into()), chain("noto-bold"));
    for base in ["carlito", "sans", "serif", "mono"] {
        for s in ["", "-b", "-i", "-bi"] {
            let k = format!("{base}{s}");
            defs.families.insert(FontFamily::Name(k.clone().into()), chain(&k));
        }
    }
    let mono = chain("mono");
    defs.families.insert(FontFamily::Monospace, mono);
    ctx.set_fonts(defs);
}

pub fn apply_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        s.visuals = egui::Visuals::light();
        s.visuals.panel_fill = RIBBON_BG;
        s.visuals.window_fill = WHITE;
        s.visuals.extreme_bg_color = WHITE;
        s.visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
        s.visuals.window_corner_radius = 4.into();
        s.visuals.menu_corner_radius = 4.into();
        s.visuals.selection.bg_fill = Color32::from_rgb(0xC5, 0xE0, 0xCF);
        s.visuals.selection.stroke = egui::Stroke::new(1.0, GREEN);
        s.visuals.hyperlink_color = LINK;
        for w in [&mut s.visuals.widgets.inactive, &mut s.visuals.widgets.noninteractive, &mut s.visuals.widgets.hovered, &mut s.visuals.widgets.active, &mut s.visuals.widgets.open] {
            w.corner_radius = 3.into();
            w.fg_stroke.color = TEXT;
        }
        s.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xFB, 0xFB, 0xFB);
        s.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0xFB, 0xFB, 0xFB);
        s.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0xC8, 0xC8, 0xC8));
        s.visuals.widgets.hovered.weak_bg_fill = HOVER;
        s.visuals.widgets.hovered.bg_fill = HOVER;
        s.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(0xA0, 0xA0, 0xA0));
        s.visuals.widgets.active.weak_bg_fill = PRESSED;
        s.visuals.widgets.active.bg_fill = PRESSED;
        s.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, RIBBON_LINE);
        s.visuals.widgets.noninteractive.fg_stroke.color = TEXT;
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size.y = 22.0;
        s.interaction.tooltip_delay = 0.45;
        use egui::TextStyle::*;
        s.text_styles = [
            (Small, ui_font(10.5)),
            (Body, ui_font(12.5)),
            (Button, ui_font(12.5)),
            (Heading, ui_font(17.0)),
            (Monospace, FontId::monospace(12.5)),
        ]
        .into();
    });
}
