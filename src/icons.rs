//! Hand-drawn vector icons for the ribbon, menus and title bar. Each icon is designed on a 16 by 16 grid.

use egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Paste, Cut, Copy, Painter, Borders, Fill, FontColor, AlignTop, AlignMiddle, AlignBottom, AlignLeft, AlignCenter, AlignRight,
    Orientation, Wrap, Merge, IndentDec, IndentInc, Currency, Percent, Comma, DecInc, DecDec, CondFormat, FormatTable,
    CellStyles, Insert, Delete, Format, AutoSum, FillDown, Clear, SortFilter, Find, Pivot, Table, ChartColumn, ChartLine,
    ChartPie, ChartBar, ChartArea, ChartScatter, Charts, Sparkline, Link, Note, Symbol, Function, Names, ShowFormulas,
    Calc, Freeze, Zoom, Zoom100, SortAZ, SortZA, Filter, FilterClear, TextCols, Dedupe, Validate, WhatIf, Group, Ungroup,
    Protect, Margins, PageOrient, PageSize, PrintArea, Undo, Redo, Save, New, Open, Print, Export, Search, Close, Minimize,
    Maximize, Restore, Check, Cross, Plus, Down, Up, Left, Right, Launcher, ViewNormal, ViewLayout, ViewBreak, Help,
    Refresh, Recent, Info, Share, Comment, Spell, Text, Bold, Italic, Underline, Grow, Shrink, Trace, Lock, Sheet, Hide,
    Window, Gridlines, Headings, Background,
}

struct P<'a> {
    p: &'a Painter,
    o: Pos2,
    s: f32,
    c: Color32,
}

impl P<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.o.x + x * self.s, self.o.y + y * self.s)
    }
    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.at(x, y), egui::vec2(w * self.s, h * self.s))
    }
    fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32, c: Color32) {
        self.p.line_segment([self.at(x1, y1), self.at(x2, y2)], Stroke::new((self.s).max(1.0), c));
    }
    fn thin(&self, x1: f32, y1: f32, x2: f32, y2: f32, c: Color32) {
        self.p.line_segment([self.at(x1, y1), self.at(x2, y2)], Stroke::new((self.s * 0.75).max(1.0), c));
    }
    fn fill(&self, x: f32, y: f32, w: f32, h: f32, c: Color32) {
        self.p.rect_filled(self.r(x, y, w, h), 0.0, c);
    }
    fn frame(&self, x: f32, y: f32, w: f32, h: f32, c: Color32) {
        self.p.rect_stroke(self.r(x, y, w, h), 0.0, Stroke::new((self.s * 0.9).max(1.0), c), egui::StrokeKind::Inside);
    }
    fn boxed(&self, x: f32, y: f32, w: f32, h: f32, fill: Color32, c: Color32) {
        self.fill(x, y, w, h, fill);
        self.frame(x, y, w, h, c);
    }
    fn poly(&self, pts: &[(f32, f32)], fill: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.p.add(Shape::convex_polygon(v, fill, Stroke::NONE));
    }
    fn path(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.p.add(Shape::line(v, Stroke::new((self.s).max(1.0), c)));
    }
    fn circle(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_filled(self.at(x, y), r * self.s, c);
    }
    fn ring(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_stroke(self.at(x, y), r * self.s, Stroke::new((self.s).max(1.0), c));
    }
    fn text(&self, x: f32, y: f32, t: &str, size: f32, c: Color32, bold: bool) {
        let f = if bold { crate::theme::ui_bold(size * self.s) } else { crate::theme::ui_font(size * self.s) };
        self.p.text(self.at(x, y), egui::Align2::CENTER_CENTER, t, f, c);
    }
    fn sheet(&self, x: f32, y: f32, w: f32, h: f32) {
        self.boxed(x, y, w, h, WHITE, GRAYD);
        let n = (h / 3.0).floor() as i32;
        for i in 1..n {
            self.thin(x, y + i as f32 * 3.0, x + w, y + i as f32 * 3.0, GRAYL);
        }
        self.thin(x + w / 2.0, y, x + w / 2.0, y + h, GRAYL);
    }
    fn arrow_down(&self, x: f32, y: f32, c: Color32) {
        self.poly(&[(x - 2.0, y), (x + 2.0, y), (x, y + 2.0)], c);
    }
}

const WHITE: Color32 = Color32::WHITE;
const GRAYD: Color32 = Color32::from_rgb(0x60, 0x60, 0x60);
const GRAY: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
const GRAYL: Color32 = Color32::from_rgb(0xBF, 0xBF, 0xBF);
const BLUE: Color32 = Color32::from_rgb(0x2B, 0x7C, 0xD3);
const GREEN: Color32 = Color32::from_rgb(0x21, 0x73, 0x46);
const LGREEN: Color32 = Color32::from_rgb(0x70, 0xAD, 0x47);
const RED: Color32 = Color32::from_rgb(0xD1, 0x34, 0x38);
const ORANGE: Color32 = Color32::from_rgb(0xED, 0x7D, 0x31);
const YELLOW: Color32 = Color32::from_rgb(0xFF, 0xC0, 0x00);
const PURPLE: Color32 = Color32::from_rgb(0x88, 0x5E, 0xC6);
const PAPER: Color32 = Color32::from_rgb(0xF6, 0xF6, 0xF6);
const BROWN: Color32 = Color32::from_rgb(0xA8, 0x77, 0x3F);

/// Draws `icon` filling the square `rect`. `ink` is the colour for plain line work.
pub fn draw(p: &Painter, rect: Rect, icon: Icon, ink: Color32) {
    let side = rect.width().min(rect.height());
    let o = rect.center() - egui::vec2(side / 2.0, side / 2.0);
    let d = P { p, o, s: side / 16.0, c: ink };
    let c = d.c;
    use Icon::*;
    match icon {
        Paste => {
            d.boxed(2.0, 2.5, 10.0, 12.5, BROWN, BROWN);
            d.boxed(4.5, 1.0, 5.0, 3.0, GRAYL, GRAYD);
            d.boxed(6.5, 6.0, 8.0, 9.0, WHITE, GRAYD);
            for y in [8.5, 10.5, 12.5] {
                d.thin(8.0, y, 13.0, y, GRAY);
            }
        }
        Cut => {
            d.ring(4.5, 12.0, 2.2, c);
            d.ring(11.5, 12.0, 2.2, c);
            d.line(5.8, 10.3, 11.0, 2.0, c);
            d.line(10.2, 10.3, 5.0, 2.0, c);
        }
        Copy => {
            d.boxed(2.0, 1.5, 8.0, 10.0, WHITE, GRAYD);
            d.boxed(6.0, 4.5, 8.0, 10.0, WHITE, GRAYD);
            for y in [7.5, 9.5, 11.5] {
                d.thin(7.5, y, 12.5, y, GRAY);
            }
        }
        Painter => {
            d.boxed(2.0, 2.0, 11.0, 4.0, YELLOW, GRAYD);
            d.line(13.0, 4.0, 14.5, 4.0, c);
            d.line(14.5, 4.0, 14.5, 8.0, c);
            d.line(14.5, 8.0, 8.0, 8.0, c);
            d.fill(7.0, 8.0, 2.0, 6.5, GRAYD);
        }
        Borders => {
            d.frame(2.0, 2.0, 12.0, 12.0, GRAYL);
            d.thin(8.0, 2.0, 8.0, 14.0, GRAYL);
            d.thin(2.0, 8.0, 14.0, 8.0, GRAYL);
            d.line(2.0, 13.5, 14.0, 13.5, c);
        }
        Fill => {
            d.poly(&[(3.0, 7.0), (8.0, 2.0), (13.0, 7.0), (8.0, 12.0)], WHITE);
            d.path(&[(3.0, 7.0), (8.0, 2.0), (13.0, 7.0), (8.0, 12.0), (3.0, 7.0)], c);
            d.circle(13.5, 10.5, 1.3, c);
        }
        FontColor => {
            d.text(8.0, 6.5, "A", 12.0, c, false);
        }
        AlignTop | AlignMiddle | AlignBottom => {
            let y0 = match icon { AlignTop => 2.0, AlignMiddle => 5.5, _ => 9.0 };
            d.line(2.0, y0, 14.0, y0, c);
            d.line(4.0, y0 + 2.5, 12.0, y0 + 2.5, c);
            d.line(2.0, y0 + 5.0, 14.0, y0 + 5.0, c);
        }
        AlignLeft | AlignCenter | AlignRight => {
            for (i, y) in [3.0, 6.0, 9.0, 12.0].iter().enumerate() {
                let w = if i % 2 == 0 { 12.0 } else { 8.0 };
                let x = match icon { AlignLeft => 2.0, AlignCenter => 8.0 - w / 2.0, _ => 14.0 - w };
                d.line(x, *y, x + w, *y, c);
            }
        }
        Orientation => {
            d.text(6.0, 9.0, "ab", 7.5, c, false);
            d.line(4.0, 14.0, 14.0, 4.0, c);
            d.poly(&[(14.5, 3.5), (11.0, 4.0), (14.0, 7.0)], c);
        }
        Wrap => {
            d.line(2.0, 3.0, 14.0, 3.0, c);
            d.path(&[(2.0, 7.0), (12.0, 7.0), (14.0, 9.0), (12.0, 11.0), (8.0, 11.0)], c);
            d.poly(&[(6.0, 11.0), (8.5, 9.0), (8.5, 13.0)], c);
            d.line(2.0, 11.0, 5.0, 11.0, c);
            d.line(2.0, 14.5, 9.0, 14.5, c);
        }
        Merge => {
            d.frame(1.5, 3.5, 13.0, 9.0, GRAYD);
            d.line(3.5, 8.0, 12.5, 8.0, BLUE);
            d.poly(&[(3.0, 8.0), (5.5, 6.0), (5.5, 10.0)], BLUE);
            d.poly(&[(13.0, 8.0), (10.5, 6.0), (10.5, 10.0)], BLUE);
        }
        IndentDec | IndentInc => {
            for y in [3.0, 13.0] {
                d.line(2.0, y, 14.0, y, c);
            }
            for y in [6.3, 9.6] {
                d.line(7.0, y, 14.0, y, c);
            }
            if icon == IndentInc {
                d.poly(&[(5.0, 8.0), (2.0, 5.5), (2.0, 10.5)], BLUE);
            } else {
                d.poly(&[(2.0, 8.0), (5.0, 5.5), (5.0, 10.5)], BLUE);
            }
        }
        Currency => {
            d.boxed(1.0, 4.0, 14.0, 8.0, Color32::from_rgb(0xD8, 0xEB, 0xD0), GREEN);
            d.circle(8.0, 8.0, 2.5, GREEN);
            d.circle(8.0, 8.0, 1.3, Color32::from_rgb(0xD8, 0xEB, 0xD0));
        }
        Percent => d.text(8.0, 8.0, "%", 12.0, c, false),
        Comma => d.text(8.0, 6.0, ",", 16.0, c, true),
        DecInc | DecDec => {
            let (a, b) = if icon == DecInc { (".0", ".00") } else { (".00", ".0") };
            d.text(8.0, 4.5, a, 7.0, c, false);
            d.text(8.0, 12.0, b, 7.0, c, false);
            d.line(1.0, 8.5, 4.0, 8.5, BLUE);
            d.poly(&[(1.0, 8.5), (3.0, 6.8), (3.0, 10.2)], BLUE);
        }
        CondFormat => {
            d.sheet(1.0, 1.0, 12.0, 12.0);
            d.fill(1.5, 4.0, 5.5, 2.5, RED);
            d.fill(7.0, 7.0, 5.5, 2.5, BLUE);
            d.fill(1.5, 10.0, 5.5, 2.5, YELLOW);
        }
        FormatTable => {
            d.boxed(1.0, 2.0, 14.0, 12.0, WHITE, GRAYD);
            d.fill(1.0, 2.0, 14.0, 3.0, BLUE);
            d.fill(1.5, 8.0, 13.0, 2.5, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.thin(8.0, 2.0, 8.0, 14.0, GRAYL);
        }
        CellStyles => {
            d.boxed(1.0, 2.0, 6.5, 5.0, Color32::from_rgb(0xFF, 0xC7, 0xCE), GRAYD);
            d.boxed(8.5, 2.0, 6.5, 5.0, Color32::from_rgb(0xC6, 0xEF, 0xCE), GRAYD);
            d.boxed(1.0, 9.0, 6.5, 5.0, Color32::from_rgb(0xFF, 0xEB, 0x9C), GRAYD);
            d.boxed(8.5, 9.0, 6.5, 5.0, WHITE, GRAYD);
        }
        Insert => {
            d.sheet(1.0, 4.0, 12.0, 10.0);
            d.fill(1.5, 7.0, 11.0, 3.0, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.line(12.0, 2.5, 15.5, 2.5, GREEN);
            d.line(13.75, 0.75, 13.75, 4.25, GREEN);
        }
        Delete => {
            d.sheet(1.0, 4.0, 12.0, 10.0);
            d.fill(1.5, 7.0, 11.0, 3.0, Color32::from_rgb(0xF8, 0xCB, 0xAD));
            d.line(12.0, 1.0, 15.0, 4.0, RED);
            d.line(15.0, 1.0, 12.0, 4.0, RED);
        }
        Format => {
            d.sheet(1.0, 4.0, 12.0, 10.0);
            d.boxed(10.0, 1.0, 5.0, 5.0, Color32::from_rgb(0xBD, 0xD7, 0xEE), BLUE);
        }
        AutoSum => d.text(8.0, 8.0, "Σ", 14.0, c, false),
        FillDown => {
            d.frame(3.0, 1.5, 10.0, 13.0, GRAYD);
            d.line(8.0, 4.0, 8.0, 11.0, BLUE);
            d.poly(&[(8.0, 13.0), (5.0, 9.5), (11.0, 9.5)], BLUE);
        }
        Clear => {
            d.poly(&[(2.0, 10.0), (8.0, 4.0), (12.0, 8.0), (6.0, 14.0)], Color32::from_rgb(0xF1, 0x9C, 0xBB));
            d.poly(&[(8.0, 4.0), (10.0, 2.0), (14.0, 6.0), (12.0, 8.0)], GRAYD);
            d.line(6.0, 14.5, 14.0, 14.5, c);
        }
        SortFilter => {
            d.text(5.0, 4.5, "A", 7.0, BLUE, true);
            d.text(5.0, 11.5, "Z", 7.0, c, true);
            d.poly(&[(9.0, 3.0), (15.0, 3.0), (12.8, 7.0), (12.8, 11.0), (11.2, 12.5), (11.2, 7.0)], GRAY);
        }
        Find => {
            d.ring(6.5, 6.5, 4.0, c);
            d.line(9.5, 9.5, 14.0, 14.0, c);
        }
        Pivot => {
            d.boxed(1.0, 1.0, 14.0, 14.0, WHITE, GRAYD);
            d.fill(1.0, 1.0, 14.0, 3.5, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.fill(1.0, 1.0, 4.0, 14.0, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.path(&[(8.0, 12.0), (8.0, 8.0), (12.0, 8.0)], GREEN);
            d.poly(&[(12.5, 8.0), (10.5, 6.5), (10.5, 9.5)], GREEN);
        }
        Table => {
            d.boxed(1.0, 2.0, 14.0, 12.0, WHITE, GRAYD);
            d.fill(1.0, 2.0, 14.0, 3.0, BLUE);
            for y in [8.0, 11.0] {
                d.thin(1.0, y, 15.0, y, GRAYL);
            }
            d.thin(6.0, 5.0, 6.0, 14.0, GRAYL);
            d.thin(10.5, 5.0, 10.5, 14.0, GRAYL);
        }
        ChartColumn | Charts => {
            d.line(1.5, 14.5, 14.5, 14.5, GRAYD);
            d.fill(2.5, 7.0, 3.0, 7.0, BLUE);
            d.fill(6.5, 3.0, 3.0, 11.0, ORANGE);
            d.fill(10.5, 9.0, 3.0, 5.0, GRAY);
        }
        ChartBar => {
            d.line(1.5, 1.5, 1.5, 14.5, GRAYD);
            d.fill(2.0, 2.5, 8.0, 3.0, BLUE);
            d.fill(2.0, 6.5, 12.0, 3.0, ORANGE);
            d.fill(2.0, 10.5, 5.0, 3.0, GRAY);
        }
        ChartLine => {
            d.line(1.5, 14.5, 14.5, 14.5, GRAYD);
            d.path(&[(2.0, 11.0), (5.5, 6.0), (9.0, 9.0), (14.0, 3.0)], BLUE);
            d.path(&[(2.0, 13.0), (5.5, 11.0), (9.0, 12.0), (14.0, 8.0)], ORANGE);
        }
        ChartArea => {
            d.poly(&[(1.5, 14.5), (1.5, 9.0), (6.0, 4.0), (10.0, 8.0), (14.5, 3.0), (14.5, 14.5)], BLUE);
            d.poly(&[(1.5, 14.5), (1.5, 12.0), (6.0, 9.5), (10.0, 11.5), (14.5, 9.0), (14.5, 14.5)], ORANGE);
        }
        ChartPie => {
            d.circle(8.0, 8.0, 6.5, BLUE);
            d.poly(&[(8.0, 8.0), (8.0, 1.5), (11.5, 2.5), (13.7, 5.0), (14.5, 8.0)], ORANGE);
            d.poly(&[(8.0, 8.0), (14.5, 8.0), (13.7, 11.0), (12.0, 13.0)], GRAY);
        }
        ChartScatter => {
            d.line(1.5, 14.5, 14.5, 14.5, GRAYD);
            d.line(1.5, 1.5, 1.5, 14.5, GRAYD);
            for (x, y) in [(4.0, 11.0), (6.0, 8.0), (8.0, 9.5), (10.0, 5.0), (12.5, 3.5), (12.0, 7.0)] {
                d.circle(x, y, 1.2, BLUE);
            }
        }
        Sparkline => {
            d.frame(1.0, 3.0, 14.0, 10.0, GRAYD);
            d.path(&[(2.5, 10.0), (5.0, 6.0), (8.0, 9.0), (10.5, 5.0), (13.5, 7.0)], BLUE);
        }
        Link => {
            d.p.rect_stroke(d.r(1.0, 6.0, 8.0, 4.5), 2.2 * d.s, Stroke::new(d.s * 1.3, c), egui::StrokeKind::Middle);
            d.p.rect_stroke(d.r(7.0, 5.5, 8.0, 4.5), 2.2 * d.s, Stroke::new(d.s * 1.3, BLUE), egui::StrokeKind::Middle);
        }
        Note | Comment => {
            d.poly(&[(1.5, 2.0), (14.5, 2.0), (14.5, 11.0), (1.5, 11.0)], if icon == Note { Color32::from_rgb(0xFF, 0xF2, 0xCC) } else { WHITE });
            d.path(&[(1.5, 2.0), (14.5, 2.0), (14.5, 11.0), (7.0, 11.0), (4.0, 14.0), (4.0, 11.0), (1.5, 11.0), (1.5, 2.0)], c);
            d.thin(4.0, 5.0, 12.0, 5.0, GRAY);
            d.thin(4.0, 8.0, 10.0, 8.0, GRAY);
        }
        Symbol => d.text(8.0, 8.0, "Ω", 13.0, c, false),
        Function => d.text(8.0, 7.5, "fx", 11.0, c, false),
        Names => {
            d.boxed(1.0, 3.0, 14.0, 10.0, WHITE, GRAYD);
            d.text(8.0, 8.0, "ab", 7.0, BLUE, true);
        }
        ShowFormulas => {
            d.sheet(1.0, 2.0, 14.0, 12.0);
            d.text(8.0, 8.0, "fx", 8.0, BLUE, true);
        }
        Calc => {
            d.boxed(3.0, 1.0, 10.0, 14.0, PAPER, GRAYD);
            d.fill(4.5, 2.5, 7.0, 3.0, GREEN);
            for y in [7.5, 10.0, 12.5] {
                for x in [5.0, 8.0, 11.0] {
                    d.circle(x, y, 0.8, c);
                }
            }
        }
        Freeze => {
            d.sheet(1.0, 1.0, 14.0, 14.0);
            d.fill(1.0, 1.0, 14.0, 4.0, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.fill(1.0, 1.0, 4.5, 14.0, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.line(1.0, 5.0, 15.0, 5.0, BLUE);
            d.line(5.5, 1.0, 5.5, 15.0, BLUE);
        }
        Zoom => {
            d.ring(6.5, 6.5, 4.5, c);
            d.line(10.0, 10.0, 14.5, 14.5, c);
            d.line(4.5, 6.5, 8.5, 6.5, c);
            d.line(6.5, 4.5, 6.5, 8.5, c);
        }
        Zoom100 => {
            d.boxed(1.0, 3.0, 14.0, 10.0, WHITE, GRAYD);
            d.text(8.0, 8.0, "100", 6.0, BLUE, true);
        }
        SortAZ | SortZA => {
            let (a, b) = if icon == SortAZ { ("A", "Z") } else { ("Z", "A") };
            d.text(5.0, 4.5, a, 7.0, BLUE, true);
            d.text(5.0, 11.5, b, 7.0, c, true);
            d.line(12.0, 2.0, 12.0, 12.0, c);
            d.poly(&[(12.0, 14.5), (9.5, 11.0), (14.5, 11.0)], c);
        }
        Filter | FilterClear => {
            d.poly(&[(1.5, 2.0), (14.5, 2.0), (9.5, 8.0), (6.5, 8.0)], GRAY);
            d.poly(&[(6.5, 8.0), (9.5, 8.0), (9.5, 12.0), (6.5, 14.5)], GRAY);
            if icon == FilterClear {
                d.line(10.5, 10.0, 15.0, 14.5, RED);
                d.line(15.0, 10.0, 10.5, 14.5, RED);
            }
        }
        TextCols => {
            d.frame(1.0, 3.0, 6.0, 10.0, GRAYD);
            d.frame(9.0, 3.0, 6.0, 10.0, GRAYD);
            d.line(4.0, 8.0, 12.0, 8.0, BLUE);
            d.poly(&[(13.0, 8.0), (10.5, 6.0), (10.5, 10.0)], BLUE);
        }
        Dedupe => {
            d.sheet(1.0, 1.0, 10.0, 12.0);
            d.fill(1.5, 4.0, 9.0, 2.5, Color32::from_rgb(0xBD, 0xD7, 0xEE));
            d.line(10.0, 9.0, 15.0, 14.0, RED);
            d.line(15.0, 9.0, 10.0, 14.0, RED);
        }
        Validate => {
            d.sheet(1.0, 1.0, 11.0, 12.0);
            d.path(&[(8.5, 11.5), (10.5, 14.0), (15.0, 7.5)], GREEN);
        }
        WhatIf => {
            d.sheet(1.0, 1.0, 11.0, 11.0);
            d.text(12.0, 11.0, "?", 10.0, BLUE, true);
        }
        Group | Ungroup => {
            d.line(3.0, 3.0, 3.0, 13.0, c);
            d.line(3.0, 13.0, 5.0, 13.0, c);
            d.boxed(1.0, 1.0, 4.0, 4.0, WHITE, c);
            d.line(2.0, 3.0, 4.0, 3.0, c);
            if icon == Ungroup {
                d.line(3.0, 2.0, 3.0, 4.0, c);
            }
            for y in [6.0, 9.0, 12.0] {
                d.fill(7.0, y, 8.0, 2.0, if icon == Group { BLUE } else { GRAY });
            }
        }
        Protect | Lock => {
            d.p.rect_stroke(d.r(5.0, 1.5, 6.0, 8.0), 3.0 * d.s, Stroke::new(d.s * 1.4, c), egui::StrokeKind::Middle);
            d.boxed(3.0, 7.0, 10.0, 8.0, YELLOW, GRAYD);
            d.circle(8.0, 11.0, 1.2, GRAYD);
        }
        Margins => {
            d.boxed(2.0, 1.0, 12.0, 14.0, WHITE, GRAYD);
            d.frame(4.0, 3.0, 8.0, 10.0, BLUE);
        }
        PageOrient => {
            d.boxed(1.0, 4.0, 10.0, 11.0, WHITE, GRAYD);
            d.boxed(5.0, 1.0, 10.0, 7.0, WHITE, BLUE);
        }
        PageSize => {
            d.boxed(1.0, 1.0, 9.0, 12.0, WHITE, GRAYD);
            d.boxed(6.0, 5.0, 9.0, 10.0, WHITE, BLUE);
        }
        PrintArea => {
            d.sheet(1.0, 1.0, 14.0, 14.0);
            d.p.rect_stroke(d.r(3.0, 3.0, 8.0, 8.0), 0.0, Stroke::new(d.s * 1.3, BLUE), egui::StrokeKind::Middle);
        }
        Undo | Redo => {
            let flip = |x: f32| if icon == Redo { 16.0 - x } else { x };
            d.path(&[(flip(3.0), 6.0), (flip(10.0), 6.0), (flip(13.0), 9.0), (flip(10.0), 12.5), (flip(6.0), 12.5)], c);
            d.poly(&[(flip(1.0), 6.0), (flip(4.5), 2.5), (flip(4.5), 9.5)], c);
        }
        Save => {
            d.p.rect_stroke(d.r(2.0, 2.0, 12.0, 12.0), 1.5 * d.s, Stroke::new(d.s.max(1.0), c), egui::StrokeKind::Middle);
            d.frame(5.0, 2.0, 6.0, 4.0, c);
            d.frame(4.5, 9.0, 7.0, 5.0, c);
        }
        New => {
            d.poly(&[(3.0, 1.0), (10.0, 1.0), (13.0, 4.0), (13.0, 15.0), (3.0, 15.0)], WHITE);
            d.path(&[(10.0, 1.0), (3.0, 1.0), (3.0, 15.0), (13.0, 15.0), (13.0, 4.0), (10.0, 1.0), (10.0, 4.0), (13.0, 4.0)], c);
        }
        Open => {
            d.poly(&[(1.0, 3.0), (6.0, 3.0), (7.5, 4.5), (14.0, 4.5), (14.0, 13.5), (1.0, 13.5)], YELLOW);
            d.poly(&[(1.0, 13.5), (3.5, 7.0), (16.0, 7.0), (14.0, 13.5)], Color32::from_rgb(0xFF, 0xD9, 0x66));
        }
        Print => {
            d.boxed(4.0, 1.0, 8.0, 5.0, WHITE, c);
            d.boxed(1.0, 5.5, 14.0, 6.0, GRAYL, c);
            d.boxed(4.0, 9.5, 8.0, 5.5, WHITE, c);
        }
        Export | Share => {
            d.frame(2.0, 6.0, 12.0, 9.0, c);
            d.line(8.0, 1.5, 8.0, 10.0, c);
            d.path(&[(5.0, 4.5), (8.0, 1.5), (11.0, 4.5)], c);
        }
        Search => {
            d.ring(6.5, 6.5, 4.0, c);
            d.line(9.5, 9.5, 13.5, 13.5, c);
        }
        Close | Cross => {
            d.line(3.5, 3.5, 12.5, 12.5, c);
            d.line(12.5, 3.5, 3.5, 12.5, c);
        }
        Minimize => d.line(3.0, 8.0, 13.0, 8.0, c),
        Maximize => d.frame(3.0, 3.0, 10.0, 10.0, c),
        Restore => {
            d.frame(3.0, 5.5, 7.5, 7.5, c);
            d.path(&[(5.5, 5.5), (5.5, 3.0), (13.0, 3.0), (13.0, 10.5), (10.5, 10.5)], c);
        }
        Check => d.path(&[(2.5, 8.5), (6.0, 12.0), (13.5, 4.0)], c),
        Plus => {
            d.line(8.0, 2.5, 8.0, 13.5, c);
            d.line(2.5, 8.0, 13.5, 8.0, c);
        }
        Down => d.path(&[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)], c),
        Up => d.path(&[(4.0, 10.0), (8.0, 6.0), (12.0, 10.0)], c),
        Left => d.path(&[(10.0, 4.0), (6.0, 8.0), (10.0, 12.0)], c),
        Right => d.path(&[(6.0, 4.0), (10.0, 8.0), (6.0, 12.0)], c),
        Launcher => {
            d.path(&[(4.0, 12.0), (4.0, 4.0), (12.0, 4.0)], c);
            d.line(7.0, 7.0, 12.0, 12.0, c);
            d.path(&[(8.5, 12.0), (12.0, 12.0), (12.0, 8.5)], c);
        }
        ViewNormal => {
            d.frame(2.0, 3.0, 12.0, 10.0, c);
            d.thin(2.0, 6.5, 14.0, 6.5, c);
            d.thin(2.0, 9.8, 14.0, 9.8, c);
            d.thin(6.0, 3.0, 6.0, 13.0, c);
            d.thin(10.0, 3.0, 10.0, 13.0, c);
        }
        ViewLayout => {
            d.frame(2.0, 2.0, 12.0, 12.0, c);
            d.frame(4.0, 4.0, 8.0, 8.0, c);
        }
        ViewBreak => {
            d.frame(2.0, 2.0, 12.0, 12.0, c);
            d.line(8.0, 2.0, 8.0, 14.0, c);
            d.line(2.0, 8.0, 14.0, 8.0, c);
        }
        Help | Info => {
            d.ring(8.0, 8.0, 6.5, c);
            d.text(8.0, 8.0, if icon == Help { "?" } else { "i" }, 9.0, c, true);
        }
        Refresh => {
            d.p.add(Shape::circle_stroke(d.at(8.0, 8.0), 5.0 * d.s, Stroke::new(d.s * 1.2, GREEN)));
            d.poly(&[(13.0, 3.0), (13.5, 8.0), (9.5, 5.5)], GREEN);
        }
        Recent => {
            d.ring(8.0, 8.0, 6.0, c);
            d.path(&[(8.0, 4.0), (8.0, 8.0), (11.0, 10.0)], c);
        }
        Spell => {
            d.text(6.0, 5.5, "abc", 6.5, c, true);
            d.path(&[(5.0, 11.0), (8.0, 14.0), (14.0, 8.0)], GREEN);
        }
        Text => {
            d.frame(1.5, 3.0, 13.0, 10.0, c);
            d.text(8.0, 8.0, "A", 9.0, c, false);
        }
        Bold => d.text(8.0, 8.0, "B", 12.0, c, true),
        Italic => d.text(8.0, 8.0, "I", 12.0, c, false),
        Underline => {
            d.text(8.0, 7.0, "U", 11.0, c, false);
            d.line(4.0, 14.0, 12.0, 14.0, c);
        }
        Grow => {
            d.text(6.0, 9.0, "A", 12.0, c, false);
            d.poly(&[(13.0, 2.5), (10.5, 6.0), (15.5, 6.0)], c);
        }
        Shrink => {
            d.text(6.0, 9.5, "A", 9.0, c, false);
            d.poly(&[(13.0, 6.0), (10.5, 2.5), (15.5, 2.5)], c);
        }
        Trace => {
            d.circle(3.0, 12.0, 1.6, BLUE);
            d.line(3.0, 12.0, 12.0, 4.0, BLUE);
            d.poly(&[(14.0, 2.5), (10.0, 3.5), (12.5, 6.5)], BLUE);
        }
        Sheet => d.sheet(1.0, 2.0, 14.0, 12.0),
        Hide => {
            d.path(&[(1.5, 8.0), (4.5, 4.5), (8.0, 3.5), (11.5, 4.5), (14.5, 8.0), (11.5, 11.5), (8.0, 12.5), (4.5, 11.5), (1.5, 8.0)], c);
            d.circle(8.0, 8.0, 2.0, c);
            d.line(2.0, 14.0, 14.0, 2.0, c);
        }
        Window => {
            d.frame(1.5, 2.0, 13.0, 12.0, c);
            d.fill(1.5, 2.0, 13.0, 2.5, c);
        }
        Gridlines => {
            d.frame(2.0, 2.0, 12.0, 12.0, c);
            for k in [6.0, 10.0] {
                d.thin(2.0, k, 14.0, k, c);
                d.thin(k, 2.0, k, 14.0, c);
            }
        }
        Headings => {
            d.frame(2.0, 2.0, 12.0, 12.0, c);
            d.fill(2.0, 2.0, 12.0, 3.0, GRAYL);
            d.fill(2.0, 2.0, 3.0, 12.0, GRAYL);
        }
        Background => {
            d.boxed(1.0, 2.0, 14.0, 12.0, Color32::from_rgb(0xDE, 0xEB, 0xF7), GRAYD);
            d.poly(&[(2.0, 13.0), (6.0, 7.0), (9.0, 11.0), (11.0, 9.0), (14.0, 13.0)], LGREEN);
            d.circle(11.5, 5.0, 1.5, YELLOW);
        }
    }
}

/// The application icon: a green tile with a white grid, drawn in pixels for the window list.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let mut v = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 / s, y as f32 / s);
            let i = ((y * size + x) * 4) as usize;
            let inside = fx > 0.06 && fx < 0.94 && fy > 0.06 && fy < 0.94;
            if !inside {
                continue;
            }
            let mut c = [0x21u8, 0x73, 0x46, 255];
            let grid = fx > 0.42 && fx < 0.86 && fy > 0.18 && fy < 0.82;
            if grid {
                let gx = ((fx - 0.42) / 0.11).fract();
                let gy = ((fy - 0.18) / 0.16).fract();
                c = if gx < 0.12 || gy < 0.12 { [0x21, 0x73, 0x46, 255] } else { [0xE8, 0xF5, 0xEC, 255] };
            }
            let tile = fx > 0.12 && fx < 0.52 && fy > 0.28 && fy < 0.72;
            if tile {
                c = [0x18, 0x5C, 0x37, 255];
                // A white "T" mark.
                let bar = fy > 0.36 && fy < 0.43 && fx > 0.19 && fx < 0.45;
                let stem = fx > 0.29 && fx < 0.35 && fy > 0.36 && fy < 0.65;
                if bar || stem {
                    c = [255, 255, 255, 255];
                }
            }
            v[i..i + 4].copy_from_slice(&c);
        }
    }
    v
}
