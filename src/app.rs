//! Window state, undo, files, clipboard and keyboard handling.

use crate::cond::CondCache;
use crate::dialogs::Dialog;
use crate::eval::{self, CalcResult, Value};
use crate::model::*;
use crate::ops::{self, Clip, PasteOpts};
use crate::{ods, xlsx_read, xlsx_write};
use egui::{Key, Modifiers, Pos2};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RTab {
    Home,
    Insert,
    PageLayout,
    Formulas,
    Data,
    Review,
    View,
    Help,
    ChartDesign,
    TableDesign,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    New,
    Open,
    Info,
    SaveAs,
    Print,
    Export,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Normal,
    Layout,
    Break,
}

#[derive(Clone, Debug)]
pub struct Sel {
    pub ranges: Vec<Rect>,
    pub active: Pos,
    pub anchor: Pos,
}

impl Sel {
    pub fn cell(p: Pos) -> Sel {
        Sel { ranges: vec![Rect::cell(p)], active: p, anchor: p }
    }
    pub fn rect(&self) -> Rect {
        *self.ranges.last().unwrap()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditMode {
    Enter,
    Edit,
    Point,
}

pub struct Edit {
    pub sheet: usize,
    pub pos: Pos,
    pub text: String,
    pub mode: EditMode,
    pub in_bar: bool,
    /// Caret as a char index, read back from the text box each frame.
    pub caret: usize,
    /// Char span of a reference that pointing is currently writing.
    pub point: Option<(usize, usize)>,
    pub point_anchor: Pos,
    pub point_cur: Pos,
    pub focus: bool,
    pub set_caret: Option<usize>,
    pub ac: usize,
    pub ac_hidden: bool,
}

#[derive(Clone)]
pub struct Snap {
    wb: Workbook,
    cur: usize,
    sel: Sel,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MenuKind {
    Paste, Borders, FillColor, FontColor, Merge, NumFmt, CondFormat, FormatTable, CellStyles, Insert, Delete, Format, AutoSum,
    FillDown, Clear, SortFilter, Find, CellCtx, ColCtx, RowCtx, TabCtx(usize), Chart(u8), Freeze, Orientation, Margins,
    PageOrient, PageSize, PrintArea, FilterCol(u32), ChartCtx, TabColor(usize), Unhide, Group, Validation, WhatIf, Names,
    Breaks, ChartLayout, ChartStyle, Spell,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Menu {
    pub kind: MenuKind,
    pub pos: Pos2,
    pub born: u64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Drag {
    None,
    Select,
    Cols(u32),
    Rows(u32),
    ColResize(u32, f32, f32),
    RowResize(u32, f32, f32),
    Fill(Rect),
    Move(Rect, Pos),
    Chart(usize, u8, Pos2, [f32; 4]),
    VScroll(f32, u32),
    HScroll(f32, u32),
    Point,
}

pub struct App {
    pub wb: Workbook,
    pub calc: CalcResult,
    pub need_calc: bool,
    pub gen: u64,
    pub cf: CondCache,
    pub cur: usize,
    pub sels: Vec<Sel>,
    pub views: Vec<(u32, u32)>,
    pub edit: Option<Edit>,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    pub clip: Option<Clip>,
    pub clip_text: String,
    pub marquee: bool,
    pub painter_fmt: Option<(Clip, bool)>,
    pub path: Option<PathBuf>,
    pub name: String,
    pub modified: bool,
    pub autosave: bool,
    pub last_change: f64,
    pub tab: RTab,
    pub ribbon_min: bool,
    pub page: Option<Page>,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub alert: Option<(String, String)>,
    pub drag: Drag,
    pub chart_sel: Option<usize>,
    pub show_bar: bool,
    pub show_formulas: bool,
    pub view_mode: ViewMode,
    pub status: String,
    pub frame: u64,
    pub recent: Vec<String>,
    pub shot: Option<(PathBuf, u32)>,
    pub name_box: Option<String>,
    pub bar_tall: bool,
    pub search: String,
    pub search_open: bool,
    pub new_count: u32,
    pub maximized: bool,
    pub pivot_pane: Option<usize>,
    pub ac_items: Vec<&'static str>,
    pub filter_state: Option<crate::dialogs::FilterPanel>,
    pub find: crate::dialogs::FindState,
    pub print_sheet: usize,
    pub print_page: usize,
    pub tab_scroll: usize,
    pub rename_tab: Option<(usize, String)>,
    pub font_box: String,
}

impl App {
    pub fn new(shot: Option<PathBuf>, open: Option<PathBuf>) -> App {
        let mut a = App {
            wb: Workbook::default(),
            calc: CalcResult::default(),
            need_calc: true,
            gen: 1,
            cf: CondCache::default(),
            cur: 0,
            sels: vec![Sel::cell(Pos::new(0, 0))],
            views: vec![(0, 0)],
            edit: None,
            undo: vec![],
            redo: vec![],
            clip: None,
            clip_text: String::new(),
            marquee: false,
            painter_fmt: None,
            path: None,
            name: "Book1".into(),
            modified: false,
            autosave: false,
            last_change: 0.0,
            tab: RTab::Home,
            ribbon_min: false,
            page: None,
            menu: None,
            dialog: None,
            alert: None,
            drag: Drag::None,
            chart_sel: None,
            show_bar: true,
            show_formulas: false,
            view_mode: ViewMode::Normal,
            status: String::new(),
            frame: 0,
            recent: vec![],
            shot: shot.map(|p| (p, 0)),
            name_box: None,
            bar_tall: false,
            search: String::new(),
            search_open: false,
            new_count: 1,
            maximized: false,
            pivot_pane: None,
            ac_items: vec![],
            filter_state: None,
            find: Default::default(),
            print_sheet: 0,
            print_page: 0,
            tab_scroll: 0,
            rename_tab: None,
            font_box: String::new(),
        };
        if a.shot.is_some() && open.is_none() {
            a.load_wb(crate::sample::sample_book(), None, "Book1".into());
        }
        if let Some(p) = open {
            a.open_path(p);
        }
        a
    }

    pub fn sheet(&self) -> &Sheet {
        &self.wb.sheets[self.cur]
    }
    pub fn sel(&self) -> &Sel {
        &self.sels[self.cur]
    }
    pub fn sel_mut(&mut self) -> &mut Sel {
        &mut self.sels[self.cur]
    }
    pub fn view(&self) -> (u32, u32) {
        self.views[self.cur]
    }
    pub fn style_at(&self, p: Pos) -> Arc<Style> {
        self.wb.styles.get(self.sheet().style_id(p.row, p.col)).clone()
    }

    pub fn load_wb(&mut self, wb: Workbook, path: Option<PathBuf>, name: String) {
        let n = wb.sheets.len();
        self.wb = wb;
        self.cur = self.wb.sheets.iter().position(|s| !s.hidden).unwrap_or(0);
        self.sels = vec![Sel::cell(Pos::new(0, 0)); n];
        self.views = vec![(0, 0); n];
        for (i, s) in self.wb.sheets.iter().enumerate() {
            self.views[i] = (s.freeze.0, s.freeze.1);
            let p = Pos::new(s.freeze.0, s.freeze.1);
            self.sels[i] = Sel::cell(p);
        }
        self.undo.clear();
        self.redo.clear();
        self.edit = None;
        self.path = path;
        self.name = name;
        self.modified = false;
        self.touch_calc();
        self.chart_sel = None;
        self.page = None;
    }

    pub fn touch_calc(&mut self) {
        self.need_calc = true;
        self.gen += 1;
    }

    /// Records the state for undo before a change.
    pub fn snapshot(&mut self) {
        self.undo.push(Snap { wb: self.wb.clone(), cur: self.cur, sel: self.sel().clone() });
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Runs a change with undo and recalculation.
    pub fn act(&mut self, f: impl FnOnce(&mut App)) {
        self.snapshot();
        f(self);
        self.changed();
    }

    pub fn changed(&mut self) {
        self.touch_calc();
        self.modified = true;
        self.marquee = self.marquee && self.clip.is_some();
        let n = self.wb.sheets.len();
        self.sels.resize(n, Sel::cell(Pos::new(0, 0)));
        self.views.resize(n, (0, 0));
        self.cur = self.cur.min(n - 1);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        self.edit = None;
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snap { wb: std::mem::replace(&mut self.wb, s.wb), cur: self.cur, sel: self.sel().clone() });
            self.cur = s.cur.min(self.wb.sheets.len() - 1);
            self.changed();
            *self.sel_mut() = s.sel;
        }
    }

    pub fn redo(&mut self) {
        self.edit = None;
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snap { wb: std::mem::replace(&mut self.wb, s.wb), cur: self.cur, sel: self.sel().clone() });
            self.cur = s.cur.min(self.wb.sheets.len() - 1);
            self.changed();
            *self.sel_mut() = s.sel;
        }
    }

    pub fn recalc_if_needed(&mut self) {
        if self.need_calc {
            self.calc = eval::recalc(&self.wb);
            self.need_calc = false;
            self.cf = CondCache::default();
        }
    }

    pub fn protected_cell(&self, p: Pos) -> bool {
        self.sheet().protection.is_some() && self.style_at(p).locked
    }

    pub fn deny_protected(&mut self, rc: Rect) -> bool {
        if self.sheet().protection.is_none() {
            return false;
        }
        let locked = (rc.r1..=rc.r2.min(rc.r1 + 200)).any(|r| (rc.c1..=rc.c2.min(rc.c1 + 50)).any(|c| self.style_at(Pos::new(r, c)).locked));
        if locked {
            self.alert = Some(("Protected sheet".into(), "The cell or chart you're trying to change is on a protected sheet. To make a change, unprotect the sheet (Review > Unprotect Sheet).".into()));
        }
        locked
    }

    // ----- selection -----

    pub fn select(&mut self, p: Pos, extend: bool) {
        let p = Pos::new(p.row.min(MAX_ROWS - 1), p.col.min(MAX_COLS - 1));
        let s = self.sel_mut();
        if extend {
            let r = Rect::from_pos(s.anchor, p);
            *s.ranges.last_mut().unwrap() = r;
            s.active = s.anchor;
        } else {
            *s = Sel::cell(p);
        }
        let rect = self.sel().rect();
        let grown = self.grow_merges(rect);
        *self.sel_mut().ranges.last_mut().unwrap() = grown;
        self.chart_sel = None;
    }

    /// Expands a rectangle until no merged cell sticks out of it.
    pub fn grow_merges(&self, mut r: Rect) -> Rect {
        loop {
            let mut changed = false;
            for m in &self.sheet().merges {
                if m.intersects(&r) && !r.contains_rect(m) {
                    r = r.union(m);
                    changed = true;
                }
            }
            if !changed {
                return r;
            }
        }
    }

    fn step(&self, p: Pos, dr: i32, dc: i32) -> Pos {
        let sh = self.sheet();
        let (mut r, mut c) = (p.row as i64, p.col as i64);
        if let Some(m) = sh.merge_at(p) {
            if dr > 0 {
                r = m.r2 as i64;
            }
            if dc > 0 {
                c = m.c2 as i64;
            }
            if dr < 0 {
                r = m.r1 as i64;
            }
            if dc < 0 {
                c = m.c1 as i64;
            }
        }
        loop {
            r += dr as i64;
            c += dc as i64;
            if r < 0 || c < 0 || r >= MAX_ROWS as i64 || c >= MAX_COLS as i64 {
                return p;
            }
            if (dr != 0 && sh.row_hidden(r as u32)) || (dc != 0 && sh.hidden_cols.contains(&(c as u32))) {
                continue;
            }
            let q = Pos::new(r as u32, c as u32);
            return sh.merge_at(q).map(|m| Pos::new(m.r1, m.c1)).unwrap_or(q);
        }
    }

    /// Ctrl+arrow: to the edge of the current block of data, or to the next block.
    fn jump(&self, p: Pos, dr: i32, dc: i32) -> Pos {
        let sh = self.sheet();
        let filled = |q: Pos| !sh.is_empty_at(q.row, q.col) || self.calc.spill.contains_key(&(self.cur as u32, q.row, q.col));
        let mut q = p;
        let next = self.step(q, dr, dc);
        if next == q {
            return q;
        }
        if filled(q) && filled(next) {
            loop {
                let n = self.step(q, dr, dc);
                if n == q || !filled(n) {
                    return q;
                }
                q = n;
            }
        }
        // Find the next filled cell along the line, or the sheet edge.
        let limit_r = sh.rows.keys().next_back().copied().unwrap_or(0);
        loop {
            let n = self.step(q, dr, dc);
            if n == q {
                return q;
            }
            q = n;
            if filled(q) {
                return q;
            }
            if dr > 0 && q.row > limit_r {
                return Pos::new(MAX_ROWS - 1, q.col);
            }
            if dc > 0 && q.col > 16_383.min(sh.rows.values().filter_map(|m| m.keys().next_back()).max().copied().unwrap_or(0) + 1) {
                return Pos::new(q.row, MAX_COLS - 1);
            }
        }
    }

    pub fn move_sel(&mut self, dr: i32, dc: i32, ctrl: bool, shift: bool) {
        let s = self.sel().clone();
        let from = if shift {
            // The moving corner is the one opposite the anchor.
            let r = s.rect();
            Pos::new(if r.r1 == s.anchor.row { r.r2 } else { r.r1 }, if r.c1 == s.anchor.col { r.c2 } else { r.c1 })
        } else {
            s.active
        };
        let to = if ctrl { self.jump(from, dr, dc) } else { self.step(from, dr, dc) };
        if shift {
            let rect = self.grow_merges(Rect::from_pos(s.anchor, to));
            let sm = self.sel_mut();
            *sm.ranges.last_mut().unwrap() = rect;
            self.reveal(to);
        } else {
            self.select(to, false);
            self.reveal(to);
        }
    }

    /// Enter and Tab inside a multi-cell selection walk through it.
    pub fn advance(&mut self, dr: i32, dc: i32) {
        let s = self.sel().clone();
        let r = s.rect();
        if r.is_single() || s.ranges.len() > 1 || self.sheet().merge_at(s.active) == Some(r) {
            self.move_sel(dr, dc, false, false);
            return;
        }
        let mut p = s.active;
        if dc != 0 {
            let c = p.col as i64 + dc as i64;
            if c > r.c2 as i64 {
                p = Pos::new(if p.row >= r.r2 { r.r1 } else { p.row + 1 }, r.c1);
            } else if c < r.c1 as i64 {
                p = Pos::new(if p.row <= r.r1 { r.r2 } else { p.row - 1 }, r.c2);
            } else {
                p.col = c as u32;
            }
        } else {
            let rr = p.row as i64 + dr as i64;
            if rr > r.r2 as i64 {
                p = Pos::new(r.r1, if p.col >= r.c2 { r.c1 } else { p.col + 1 });
            } else if rr < r.r1 as i64 {
                p = Pos::new(r.r2, if p.col <= r.c1 { r.c2 } else { p.col - 1 });
            } else {
                p.row = rr as u32;
            }
        }
        self.sel_mut().active = p;
        self.reveal(p);
    }

    /// Scrolls so that a cell is visible. The grid sets `visible` each frame.
    pub fn reveal(&mut self, p: Pos) {
        let (fr, fc) = self.sheet().freeze;
        let (rows, cols) = crate::grid::visible_counts(self);
        let (mut top, mut left) = self.view();
        if p.row >= fr {
            if p.row < top {
                top = p.row;
            } else if p.row >= top + rows.max(1) {
                top = p.row + 1 - rows.max(1);
            }
        }
        if p.col >= fc {
            if p.col < left {
                left = p.col;
            } else if p.col >= left + cols.max(1) {
                left = p.col + 1 - cols.max(1);
            }
        }
        self.views[self.cur] = (top.max(fr), left.max(fc));
    }

    // ----- editing -----

    pub fn start_edit(&mut self, initial: Option<String>, in_bar: bool) {
        let p = self.sel().active;
        if self.protected_cell(p) {
            self.deny_protected(Rect::cell(p));
            return;
        }
        let (text, mode) = match initial {
            Some(t) => (t, EditMode::Enter),
            None => (ops::edit_text(&self.wb, self.cur, p.row, p.col), EditMode::Edit),
        };
        let n = text.chars().count();
        self.edit = Some(Edit { sheet: self.cur, pos: p, text, mode, in_bar, caret: n, point: None, point_anchor: p, point_cur: p, focus: true, set_caret: Some(n), ac: 0, ac_hidden: false });
        self.reveal(p);
    }

    pub fn cancel_edit(&mut self) {
        if let Some(e) = self.edit.take() {
            self.cur = e.sheet;
        }
    }

    /// Writes the edit into the cell. `array` fills the whole selection (Ctrl+Enter).
    pub fn commit_edit(&mut self, fill_selection: bool) -> bool {
        let Some(e) = self.edit.take() else { return true };
        self.cur = e.sheet;
        let mut text = e.text.clone();
        if text.starts_with('=') {
            // Close brackets the person left open, as the original does.
            let mut depth = 0i32;
            let mut q = false;
            for ch in text.chars() {
                match ch {
                    '"' => q = !q,
                    '(' if !q => depth += 1,
                    ')' if !q => depth -= 1,
                    _ => {}
                }
            }
            if q {
                text.push('"');
            }
            for _ in 0..depth.max(0) {
                text.push(')');
            }
        }
        if let Some(msg) = crate::dialogs::validate_input(self, e.pos, &text) {
            self.alert = Some(("Data validation".into(), msg));
            self.edit = Some(e);
            return false;
        }
        let targets: Vec<Pos> = if fill_selection {
            let r = self.sel().rect();
            (r.r1..=r.r2).flat_map(|row| (r.c1..=r.c2).map(move |col| Pos::new(row, col))).collect()
        } else {
            vec![e.pos]
        };
        self.snapshot();
        let base = e.pos;
        for p in targets {
            let input = if p != base && text.starts_with('=') {
                match crate::formula::parse(&text) {
                    Ok(x) => format!("={}", crate::formula::print(&crate::formula::shifted(&x, p.row as i64 - base.row as i64, p.col as i64 - base.col as i64))),
                    Err(_) => text.clone(),
                }
            } else {
                text.clone()
            };
            if let Err(m) = ops::set_input(&mut self.wb, self.cur, p.row, p.col, &input) {
                self.undo.pop();
                self.alert = Some(("There's a problem with this formula".into(), format!("{m}\n\nNot trying to type a formula? When the first character is an equal (=) sign, the entry is read as a formula. To type text, start it with an apostrophe (').")));
                self.edit = Some(Edit { focus: true, ..e });
                return false;
            }
            // Wrapped text and new lines grow the row unless its height was set by hand.
            if text.contains('\n') {
                let style = self.sheet().style_id(p.row, p.col);
                if !self.wb.styles.get(style).wrap {
                    ops::style_ranges(&mut self.wb, self.cur, &[Rect::cell(p)], &|s| s.wrap = true);
                }
            }
        }
        if let Some(Content::Text(t)) = self.sheet().get(base.row, base.col).map(|c| c.v.clone()) {
            if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("www.") {
                let url = if t.starts_with("www.") { format!("https://{t}") } else { t.to_string() };
                self.wb.sheet_mut(self.cur).links.insert(base, url);
            }
        }
        self.changed();
        self.autofit_rows(e.pos.row, e.pos.row);
        true
    }

    /// Rows without a hand-set height grow to fit wrapped text and large fonts.
    pub fn autofit_rows(&mut self, r1: u32, r2: u32) {
        let mut changes = vec![];
        {
            let sh = self.sheet();
            for r in r1..=r2.min(r1 + 500) {
                if sh.custom_h.contains(&r) {
                    continue;
                }
                let mut h = DEFAULT_ROW_H;
                if let Some(row) = sh.rows.get(&r) {
                    for (&c, cell) in row.iter() {
                        let st = self.wb.styles.get(cell.style);
                        let px = crate::theme::cell_px(st.size10, 1.0);
                        let line = (px * 1.36).max(DEFAULT_ROW_H - 5.0);
                        let mut lines = 1.0;
                        if st.wrap && cell.v != Content::Empty && sh.merge_at(Pos::new(r, c)).is_none() {
                            let t = ops::text_of(&self.wb, &self.calc, self.cur, r, c);
                            let w = sh.col_px(c).max(8.0) - 6.0;
                            let mut n = 0.0;
                            for part in t.split('\n') {
                                n += ((part.chars().count() as f32 * px * 0.5) / w).ceil().max(1.0);
                            }
                            lines = n;
                        }
                        h = h.max(line * lines + 5.0);
                    }
                }
                let cur = sh.row_h.get(&r).copied().unwrap_or(DEFAULT_ROW_H);
                if (cur - h).abs() > 0.5 {
                    changes.push((r, h));
                }
            }
        }
        if !changes.is_empty() {
            let sh = self.wb.sheet_mut(self.cur);
            for (r, h) in changes {
                if (h - DEFAULT_ROW_H).abs() < 0.5 {
                    sh.row_h.remove(&r);
                } else {
                    sh.row_h.insert(r, h);
                }
            }
        }
    }

    /// Inserts or replaces the reference being pointed at in the formula being typed.
    pub fn point_at(&mut self, rect: Rect, sheet: usize) {
        let own = self.edit.as_ref().map(|e| e.sheet).unwrap_or(self.cur);
        let mut r = rect.a1();
        if sheet != own {
            r = format!("{}!{}", crate::formula::quote_sheet(&self.wb.sheets[sheet].name), r);
        }
        let Some(e) = self.edit.as_mut() else { return };
        let mut chars: Vec<char> = e.text.chars().collect();
        let (s, t) = e.point.unwrap_or((e.caret, e.caret));
        let s = s.min(chars.len());
        let t = t.min(chars.len()).max(s);
        chars.splice(s..t, r.chars());
        e.text = chars.into_iter().collect();
        let end = s + r.chars().count();
        e.point = Some((s, end));
        e.caret = end;
        e.set_caret = Some(end);
        e.focus = true;
        e.mode = EditMode::Point;
    }

    /// True when the caret sits where a reference may be typed: after an operator, comma or bracket.
    pub fn wants_ref(&self) -> bool {
        let Some(e) = &self.edit else { return false };
        if !e.text.starts_with('=') && !e.text.starts_with('+') {
            return false;
        }
        if e.point.is_some() && e.point.map(|p| p.1) == Some(e.caret) {
            return true;
        }
        let before: Vec<char> = e.text.chars().take(e.caret).collect();
        let quotes = before.iter().filter(|c| **c == '"').count();
        if quotes % 2 == 1 {
            return false;
        }
        matches!(before.iter().rev().find(|c| **c != ' '), Some('=' | '+' | '-' | '*' | '/' | '^' | '(' | ',' | ':' | '<' | '>' | '&' | ';'))
    }

    // ----- clipboard -----

    pub fn copy(&mut self, ctx: &egui::Context, cut: bool) {
        let s = self.sel().clone();
        if s.ranges.len() > 1 {
            self.alert = Some(("Copy".into(), "This action won't work on multiple selections.".into()));
            return;
        }
        let r = s.rect();
        let r = Rect::new(r.r1, r.c1, r.r2.min(self.sheet().used_rect().map_or(r.r1, |u| u.r2.max(r.r1))), r.c2.min(self.sheet().used_rect().map_or(r.c1, |u| u.c2.max(r.c1))));
        let mut clip = ops::copy_rect(&self.wb, self.cur, r);
        clip.cut = cut;
        let mut t = String::new();
        for row in r.r1..=r.r2 {
            if self.sheet().row_hidden(row) {
                continue;
            }
            let line: Vec<String> = (r.c1..=r.c2).map(|c| ops::text_of(&self.wb, &self.calc, self.cur, row, c)).collect();
            t.push_str(&line.join("\t"));
            t.push('\n');
        }
        ctx.copy_text(t.clone());
        self.clip_text = t;
        self.clip = Some(clip);
        self.marquee = true;
    }

    pub fn paste_clip(&mut self, opts: PasteOpts) {
        let Some(clip) = self.clip.clone() else { return };
        let dest = self.sel().rect();
        if self.deny_protected(dest) {
            return;
        }
        self.act(|a| {
            if clip.cut && clip.sheet == a.cur && opts.what == ops::PasteWhat::All {
                ops::move_block(&mut a.wb, a.cur, clip.src, Pos::new(dest.r1, dest.c1));
                let r = Rect::new(dest.r1, dest.c1, dest.r1 + clip.src.rows() - 1, dest.c1 + clip.src.cols() - 1);
                *a.sel_mut() = Sel { ranges: vec![r], active: Pos::new(r.r1, r.c1), anchor: Pos::new(r.r1, r.c1) };
                a.clip = None;
                a.marquee = false;
                return;
            }
            let out = ops::paste(&mut a.wb, &a.calc, a.cur, &clip, dest, opts);
            if clip.cut {
                let src_sheet = clip.sheet;
                ops::clear_rect(&mut a.wb, src_sheet, clip.src);
                a.clip = None;
                a.marquee = false;
            }
            a.sel_mut().ranges = vec![out];
        });
    }

    /// Pastes plain text from other programs, split into cells by tabs and lines.
    pub fn paste_text(&mut self, text: &str) {
        if self.clip.is_some() && text.replace("\r\n", "\n").trim_end_matches('\n') == self.clip_text.trim_end_matches('\n') {
            self.paste_clip(PasteOpts::default());
            return;
        }
        let at = self.sel().active;
        if self.deny_protected(Rect::cell(at)) {
            return;
        }
        let rows: Vec<&str> = text.trim_end_matches(['\n', '\r']).split('\n').collect();
        self.act(|a| {
            let mut w = 1;
            for (i, line) in rows.iter().enumerate() {
                let cells: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
                w = w.max(cells.len() as u32);
                for (j, v) in cells.iter().enumerate() {
                    let _ = ops::set_input(&mut a.wb, a.cur, at.row + i as u32, at.col + j as u32, v);
                }
            }
            a.sel_mut().ranges = vec![Rect::new(at.row, at.col, at.row + rows.len() as u32 - 1, at.col + w - 1)];
        });
    }

    pub fn delete_contents(&mut self) {
        let rects = self.sel().ranges.clone();
        if rects.iter().any(|r| self.deny_protected(*r)) {
            return;
        }
        self.act(|a| ops::clear_contents(&mut a.wb, a.cur, &rects));
    }

    /// Alt+= and the AutoSum button: a SUM (or other function) over the numbers above or to the left.
    pub fn autosum(&mut self, func: &str) {
        let s = self.sel().clone();
        let r = s.rect();
        let sh = self.sheet();
        let is_num = |row: u32, col: u32| matches!(self.calc.value(&self.wb, self.cur, row, col), Value::Num(_));
        if !r.is_single() {
            // Sums under each column of the selection.
            let target_row = if (r.c1..=r.c2).all(|c| sh.is_empty_at(r.r2, c)) { r.r2 } else { r.r2 + 1 };
            let src_end = if target_row == r.r2 { r.r2 - 1 } else { r.r2 };
            self.act(|a| {
                for c in r.c1..=r.c2 {
                    let rng = Rect::new(r.r1, c, src_end, c).a1();
                    let _ = ops::set_input(&mut a.wb, a.cur, target_row, c, &format!("={func}({rng})"));
                }
            });
            return;
        }
        let p = s.active;
        let mut top = p.row;
        while top > 0 && is_num(top - 1, p.col) {
            top -= 1;
        }
        let range = if top < p.row {
            Rect::new(top, p.col, p.row - 1, p.col)
        } else {
            let mut left = p.col;
            while left > 0 && is_num(p.row, left - 1) {
                left -= 1;
            }
            if left < p.col {
                Rect::new(p.row, left, p.row, p.col - 1)
            } else {
                Rect::cell(Pos::new(p.row.saturating_sub(1), p.col))
            }
        };
        let text = format!("={func}({})", range.a1());
        let start = func.len() + 2;
        let end = start + range.a1().len();
        self.start_edit(Some(text), false);
        if let Some(e) = self.edit.as_mut() {
            e.point = Some((start, end));
            e.caret = end;
            e.set_caret = Some(end);
            e.mode = EditMode::Point;
            e.point_anchor = Pos::new(range.r1, range.c1);
            e.point_cur = Pos::new(range.r2, range.c2);
        }
    }

    // ----- files -----

    pub fn new_book(&mut self) {
        self.new_count += 1;
        let n = format!("Book{}", self.new_count);
        self.load_wb(Workbook::default(), None, n);
    }

    pub fn open_dialog(&mut self) {
        let f = rfd::FileDialog::new()
            .add_filter("All spreadsheets", &["xlsx", "xlsm", "ods", "csv", "tsv", "txt"])
            .add_filter("Excel workbook", &["xlsx", "xlsm"])
            .add_filter("OpenDocument spreadsheet", &["ods"])
            .add_filter("Text (comma or tab separated)", &["csv", "tsv", "txt"])
            .pick_file();
        if let Some(p) = f {
            self.open_path(p);
        }
    }

    pub fn open_path(&mut self, p: PathBuf) {
        let name = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "Book".into());
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let data = match std::fs::read(&p) {
            Ok(d) => d,
            Err(e) => {
                self.alert = Some(("Can't open file".into(), format!("{}\n\n{e}", p.display())));
                return;
            }
        };
        let res = match ext.as_str() {
            "ods" => ods::load(data),
            "csv" | "tsv" | "txt" => Ok(ods::load_delimited(&String::from_utf8_lossy(&data), &name)),
            _ => xlsx_read::load(data),
        };
        match res {
            Ok(wb) => {
                let keep = matches!(ext.as_str(), "xlsx" | "xlsm" | "ods" | "csv" | "tsv" | "txt");
                self.load_wb(wb, if keep { Some(p.clone()) } else { None }, name);
                self.add_recent(&p);
            }
            Err(e) => self.alert = Some(("Can't open file".into(), format!("We found a problem with {}.\n\n{e}", p.display()))),
        }
    }

    pub fn add_recent(&mut self, p: &std::path::Path) {
        let s = p.display().to_string();
        self.recent.retain(|x| *x != s);
        self.recent.insert(0, s);
        self.recent.truncate(12);
    }

    pub fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) -> bool {
        let f = rfd::FileDialog::new()
            .set_file_name(format!("{}.xlsx", self.name))
            .add_filter("Excel workbook (*.xlsx)", &["xlsx"])
            .add_filter("OpenDocument spreadsheet (*.ods)", &["ods"])
            .add_filter("CSV (comma delimited) (*.csv)", &["csv"])
            .add_filter("Text (tab delimited) (*.tsv)", &["tsv", "txt"])
            .add_filter("PDF (*.pdf)", &["pdf"])
            .save_file();
        match f {
            Some(p) => self.save_to(p),
            None => false,
        }
    }

    pub fn save_to(&mut self, p: PathBuf) -> bool {
        self.recalc_if_needed();
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let bytes: Result<Vec<u8>, String> = match ext.as_str() {
            "ods" => ods::save(&self.wb, &self.calc),
            "csv" => Ok(ods::save_delimited(&self.wb, &self.calc, self.cur, ',').into_bytes()),
            "tsv" | "txt" => Ok(ods::save_delimited(&self.wb, &self.calc, self.cur, '\t').into_bytes()),
            "pdf" => Ok(crate::print::pdf(&self.wb, &self.calc, &[self.cur])),
            _ => xlsx_write::save(&self.wb, &self.calc),
        };
        match bytes.and_then(|b| std::fs::write(&p, b).map_err(|e| e.to_string())) {
            Ok(()) => {
                if ext != "pdf" {
                    self.name = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                    self.path = Some(p.clone());
                    self.modified = false;
                }
                self.add_recent(&p);
                self.status = format!("Saved to {}", p.display());
                true
            }
            Err(e) => {
                self.alert = Some(("Can't save".into(), format!("{}\n\n{e}", p.display())));
                false
            }
        }
    }

    pub fn export_pdf(&mut self, all: bool) {
        let f = rfd::FileDialog::new().set_file_name(format!("{}.pdf", self.name)).add_filter("PDF", &["pdf"]).save_file();
        if let Some(p) = f {
            self.recalc_if_needed();
            let sheets: Vec<usize> = if all { (0..self.wb.sheets.len()).filter(|i| !self.wb.sheets[*i].hidden).collect() } else { vec![self.cur] };
            match std::fs::write(&p, crate::print::pdf(&self.wb, &self.calc, &sheets)) {
                Ok(()) => self.status = format!("Exported {}", p.display()),
                Err(e) => self.alert = Some(("Can't export".into(), e.to_string())),
            }
        }
    }

    /// Sends the active sheet to the system's default printer.
    pub fn print_now(&mut self, all: bool) {
        self.recalc_if_needed();
        let sheets: Vec<usize> = if all { (0..self.wb.sheets.len()).filter(|i| !self.wb.sheets[*i].hidden).collect() } else { vec![self.cur] };
        let tmp = std::env::temp_dir().join(format!("tabula-print-{}.pdf", std::process::id()));
        if std::fs::write(&tmp, crate::print::pdf(&self.wb, &self.calc, &sheets)).is_err() {
            return;
        }
        match std::process::Command::new("lp").arg(&tmp).output() {
            Ok(o) if o.status.success() => self.status = "Sent to the printer".into(),
            Ok(o) => self.alert = Some(("Print".into(), format!("The printer didn't accept the job.\n\n{}", String::from_utf8_lossy(&o.stderr)))),
            Err(_) => self.alert = Some(("Print".into(), "No printing service was found on this computer. Use Export to make a PDF instead.".into())),
        }
    }

    // ----- keyboard -----

    pub fn keys(&mut self, ctx: &egui::Context) {
        let typing_elsewhere = ctx.memory(|m| m.focused()).is_some_and(|id| id != crate::grid::editor_id() && id != crate::chrome::bar_id());
        if self.dialog.is_some() && !matches!(self.dialog, Some(Dialog::FindReplace(_))) || self.page.is_some() || self.alert.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                if self.alert.is_some() {
                    self.alert = None;
                } else if self.page.is_some() {
                    self.page = None;
                } else {
                    self.dialog = None;
                }
            }
            return;
        }
        let k = |ctx: &egui::Context, m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        let cmd = Modifiers::COMMAND;
        let cs = Modifiers::COMMAND | Modifiers::SHIFT;
        let sh = Modifiers::SHIFT;
        let none = Modifiers::NONE;
        // Shortcuts that work in any state.
        if k(ctx, cmd, Key::S) {
            if self.edit.is_some() && !self.commit_edit(false) {
                return;
            }
            self.save();
        }
        if k(ctx, Modifiers::NONE, Key::F12) {
            self.save_as();
        }
        if k(ctx, cmd, Key::O) {
            self.open_dialog();
        }
        if k(ctx, cmd, Key::N) {
            self.new_book();
        }
        if k(ctx, cmd, Key::P) {
            self.page = Some(Page::Print);
        }
        if k(ctx, cmd, Key::F1) {
            self.ribbon_min = !self.ribbon_min;
        }
        if k(ctx, cmd, Key::PageDown) {
            self.next_sheet(1);
        }
        if k(ctx, cmd, Key::PageUp) {
            self.next_sheet(-1);
        }
        if self.edit.is_some() {
            self.edit_keys(ctx);
            return;
        }
        if typing_elsewhere {
            return;
        }
        if self.menu.is_some() && k(ctx, none, Key::Escape) {
            self.menu = None;
            return;
        }
        if k(ctx, none, Key::Escape) {
            self.marquee = false;
            self.painter_fmt = None;
            self.chart_sel = None;
        }
        if let Some(i) = self.chart_sel {
            if k(ctx, none, Key::Delete) || k(ctx, none, Key::Backspace) {
                self.act(|a| {
                    a.wb.sheet_mut(a.cur).charts.remove(i);
                });
                self.chart_sel = None;
            }
            return;
        }
        let arrows = [(Key::ArrowUp, -1, 0), (Key::ArrowDown, 1, 0), (Key::ArrowLeft, 0, -1), (Key::ArrowRight, 0, 1)];
        for (key, dr, dc) in arrows {
            for (m, ctrl, shift) in [(none, false, false), (sh, false, true), (cmd, true, false), (cs, true, true)] {
                if k(ctx, m, key) {
                    self.move_sel(dr, dc, ctrl, shift);
                    self.marquee = self.marquee && self.clip.is_some();
                }
            }
        }
        if k(ctx, none, Key::Enter) {
            self.advance(1, 0);
        }
        if k(ctx, sh, Key::Enter) {
            self.advance(-1, 0);
        }
        if k(ctx, none, Key::Tab) {
            self.advance(0, 1);
        }
        if k(ctx, sh, Key::Tab) {
            self.advance(0, -1);
        }
        let page = crate::grid::visible_counts(self).0.max(1) as i32;
        for (m, shift) in [(none, false), (sh, true)] {
            if k(ctx, m, Key::PageDown) {
                let (t, l) = self.view();
                self.views[self.cur] = ((t + page as u32).min(MAX_ROWS - 1), l);
                let a = self.sel().active;
                self.move_to(Pos::new((a.row as i32 + page).min(MAX_ROWS as i32 - 1) as u32, a.col), shift);
            }
            if k(ctx, m, Key::PageUp) {
                let (t, l) = self.view();
                self.views[self.cur] = (t.saturating_sub(page as u32).max(self.sheet().freeze.0), l);
                let a = self.sel().active;
                self.move_to(Pos::new((a.row as i32 - page).max(0) as u32, a.col), shift);
            }
        }
        if k(ctx, none, Key::Home) {
            let a = self.sel().active;
            self.move_to(Pos::new(a.row, self.sheet().freeze.1), false);
        }
        if k(ctx, cmd, Key::Home) {
            let f = self.sheet().freeze;
            self.move_to(Pos::new(f.0, f.1), false);
        }
        if k(ctx, cmd, Key::End) {
            let u = self.sheet().used_rect().unwrap_or(Rect::new(0, 0, 0, 0));
            self.move_to(Pos::new(u.r2, u.c2), false);
        }
        if k(ctx, cs, Key::End) {
            let u = self.sheet().used_rect().unwrap_or(Rect::new(0, 0, 0, 0));
            self.move_to(Pos::new(u.r2, u.c2), true);
        }
        if k(ctx, none, Key::Delete) {
            self.delete_contents();
        }
        if k(ctx, none, Key::Backspace) {
            self.delete_contents();
            self.start_edit(Some(String::new()), false);
        }
        if k(ctx, none, Key::F2) {
            self.start_edit(None, false);
        }
        if k(ctx, none, Key::F9) {
            self.touch_calc();
        }
        if k(ctx, sh, Key::F2) {
            self.dialog = Some(Dialog::note_for(self));
        }
        if k(ctx, sh, Key::F3) {
            self.dialog = Some(Dialog::insert_function(self));
        }
        if k(ctx, sh, Key::F11) {
            self.insert_sheet(self.cur);
        }
        if k(ctx, none, Key::F5) || k(ctx, cmd, Key::G) {
            self.dialog = Some(Dialog::GoTo(String::new()));
        }
        if k(ctx, cmd, Key::Z) {
            self.undo();
        }
        if k(ctx, cmd, Key::Y) || k(ctx, cs, Key::Z) {
            self.redo();
        }
        if k(ctx, cmd, Key::A) {
            let p = self.sel().active;
            let region = ops::current_region(self.sheet(), p);
            let r = if region.is_single() || self.sel().rect() == region { Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1) } else { region };
            self.sel_mut().ranges = vec![r];
        }
        if k(ctx, Modifiers::CTRL, Key::Space) {
            let r = self.sel().rect();
            self.sel_mut().ranges = vec![Rect::new(0, r.c1, MAX_ROWS - 1, r.c2)];
        }
        if k(ctx, sh, Key::Space) {
            let r = self.sel().rect();
            self.sel_mut().ranges = vec![Rect::new(r.r1, 0, r.r2, MAX_COLS - 1)];
        }
        let toggles: [(Key, fn(&mut Style) -> &mut bool); 3] = [(Key::B, |s| &mut s.bold), (Key::I, |s| &mut s.italic), (Key::Num5, |s| &mut s.strike)];
        for (key, f) in toggles {
            if k(ctx, cmd, key) {
                let on = !*f(&mut (*self.style_at(self.sel().active)).clone());
                self.format(move |s| *f(s) = on);
            }
        }
        if k(ctx, cmd, Key::U) {
            let on = self.style_at(self.sel().active).underline == 0;
            self.format(move |s| s.underline = on as u8);
        }
        if k(ctx, cmd, Key::Num1) {
            self.dialog = Some(Dialog::format_cells(self, 0));
        }
        if k(ctx, cs, Key::F) {
            self.dialog = Some(Dialog::format_cells(self, 2));
        }
        if k(ctx, cmd, Key::F) {
            self.find.replace = false;
            self.dialog = Some(Dialog::FindReplace(0));
        }
        if k(ctx, cmd, Key::H) {
            self.find.replace = true;
            self.dialog = Some(Dialog::FindReplace(1));
        }
        if k(ctx, cmd, Key::K) {
            self.dialog = Some(Dialog::hyperlink(self));
        }
        if k(ctx, cmd, Key::T) || k(ctx, cmd, Key::L) {
            self.dialog = Some(Dialog::create_table(self));
        }
        if k(ctx, cs, Key::L) {
            crate::dialogs::toggle_filter(self);
        }
        if k(ctx, cmd, Key::D) {
            self.fill_dir(0);
        }
        if k(ctx, cmd, Key::R) {
            self.fill_dir(1);
        }
        if k(ctx, Modifiers::ALT, Key::Equals) {
            self.autosum("SUM");
        }
        if k(ctx, cmd, Key::Semicolon) {
            let (y, m, d) = eval::today_ymd();
            self.start_edit(Some(format!("{m}/{d}/{y}")), false);
        }
        if k(ctx, cs, Key::Semicolon) || k(ctx, cs, Key::Colon) {
            let t = eval::now_serial().fract();
            let mins = (t * 1440.0).round() as i64;
            self.start_edit(Some(format!("{}:{:02} {}", (mins / 60 + 11) % 12 + 1, mins % 60, if mins >= 720 { "PM" } else { "AM" })), false);
        }
        if ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "`")) && i.modifiers.command) {
            self.show_formulas = !self.show_formulas;
        }
        if k(ctx, cs, Key::Num4) {
            self.format(|s| s.num_fmt = "\"$\"#,##0.00".into());
        }
        if k(ctx, cs, Key::Num5) {
            self.format(|s| s.num_fmt = "0%".into());
        }
        if k(ctx, cs, Key::Num1) {
            self.format(|s| s.num_fmt = "#,##0.00".into());
        }
        if k(ctx, cs, Key::Num3) {
            self.format(|s| s.num_fmt = "d-mmm-yy".into());
        }
        if k(ctx, cmd, Key::Minus) {
            self.dialog = Some(Dialog::DeleteCells(0));
        }
        if k(ctx, cs, Key::Plus) || k(ctx, cs, Key::Equals) {
            self.dialog = Some(Dialog::InsertCells(0));
        }
        // Clipboard events and typed text.
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy => self.copy(ctx, false),
                egui::Event::Cut => self.copy(ctx, true),
                egui::Event::Paste(t) => self.paste_text(&t),
                egui::Event::Text(t) if !ctx.input(|i| i.modifiers.command) => {
                    if self.menu.is_none() && t != "`" || !ctx.input(|i| i.modifiers.command) {
                        self.start_edit(Some(t), false);
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    fn move_to(&mut self, p: Pos, extend: bool) {
        self.select(p, extend);
        self.reveal(p);
    }

    fn edit_keys(&mut self, ctx: &egui::Context) {
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        let none = Modifiers::NONE;
        let ac_open = !self.ac_items.is_empty() && !self.edit.as_ref().unwrap().ac_hidden;
        if ac_open {
            if k(none, Key::ArrowDown) {
                let e = self.edit.as_mut().unwrap();
                e.ac = (e.ac + 1).min(self.ac_items.len() - 1);
            }
            if k(none, Key::ArrowUp) {
                let e = self.edit.as_mut().unwrap();
                e.ac = e.ac.saturating_sub(1);
            }
            if k(none, Key::Tab) {
                let name = self.ac_items[self.edit.as_ref().unwrap().ac.min(self.ac_items.len() - 1)];
                crate::grid::accept_completion(self, name);
                return;
            }
            if k(none, Key::Escape) {
                self.edit.as_mut().unwrap().ac_hidden = true;
                return;
            }
        }
        if k(none, Key::Escape) {
            self.cancel_edit();
            return;
        }
        if k(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter) || k(Modifiers::COMMAND, Key::Enter) {
            self.commit_edit(true);
            return;
        }
        for (key, m, dr, dc) in [(Key::Enter, none, 1, 0), (Key::Enter, Modifiers::SHIFT, -1, 0), (Key::Tab, none, 0, 1), (Key::Tab, Modifiers::SHIFT, 0, -1)] {
            if k(m, key) {
                if self.commit_edit(false) {
                    self.advance(dr, dc);
                }
                return;
            }
        }
        if k(none, Key::F4) {
            crate::grid::toggle_abs_at_caret(self);
        }
        if k(none, Key::F2) {
            let e = self.edit.as_mut().unwrap();
            e.mode = if e.mode == EditMode::Edit { EditMode::Enter } else { EditMode::Edit };
            e.point = None;
        }
        let mode = self.edit.as_ref().unwrap().mode;
        let in_bar = self.edit.as_ref().unwrap().in_bar;
        if in_bar {
            return;
        }
        let pointing = self.wants_ref() || mode == EditMode::Point;
        for (key, dr, dc) in [(Key::ArrowUp, -1i32, 0i32), (Key::ArrowDown, 1, 0), (Key::ArrowLeft, 0, -1), (Key::ArrowRight, 0, 1)] {
            for (m, shift) in [(none, false), (Modifiers::SHIFT, true)] {
                if mode == EditMode::Edit && !pointing {
                    continue;
                }
                if !(pointing || mode == EditMode::Enter) {
                    continue;
                }
                if ctx.input(|i| i.key_pressed(key) && i.modifiers.matches_exact(m)) {
                    k(m, key);
                    if pointing {
                        let (anchor, cur, fresh) = {
                            let e = self.edit.as_ref().unwrap();
                            (e.point_anchor, e.point_cur, e.point.is_none_or(|p| p.1 != e.caret))
                        };
                        let base = if fresh { self.edit.as_ref().unwrap().pos } else { cur };
                        let n = Pos::new((base.row as i64 + dr as i64).clamp(0, MAX_ROWS as i64 - 1) as u32, (base.col as i64 + dc as i64).clamp(0, MAX_COLS as i64 - 1) as u32);
                        let a = if shift && !fresh { anchor } else { n };
                        if fresh {
                            self.edit.as_mut().unwrap().point = None;
                        }
                        {
                            let e = self.edit.as_mut().unwrap();
                            e.point_anchor = a;
                            e.point_cur = n;
                        }
                        let s = self.cur;
                        self.point_at(Rect::from_pos(a, n), s);
                        self.reveal(n);
                    } else if self.commit_edit(false) {
                        self.move_sel(dr, dc, false, false);
                    }
                    return;
                }
            }
        }
    }

    pub fn next_sheet(&mut self, d: i32) {
        let n = self.wb.sheets.len() as i32;
        let mut i = self.cur as i32;
        for _ in 0..n {
            i = (i + d).rem_euclid(n);
            if !self.wb.sheets[i as usize].hidden {
                break;
            }
        }
        self.switch_sheet(i as usize);
    }

    pub fn switch_sheet(&mut self, i: usize) {
        if let Some(e) = &self.edit {
            if !(e.text.starts_with('=') && self.wants_ref()) && e.mode != EditMode::Point {
                if !self.commit_edit(false) {
                    return;
                }
            }
        }
        self.cur = i.min(self.wb.sheets.len() - 1);
        self.chart_sel = None;
        if let Some(e) = &mut self.edit {
            e.point = if e.mode == EditMode::Point { e.point } else { None };
        }
    }

    pub fn insert_sheet(&mut self, at: usize) {
        self.act(|a| {
            let n = a.wb.unique_sheet_name("Sheet");
            let i = (at + 1).min(a.wb.sheets.len());
            a.wb.sheets.insert(i, Arc::new(Sheet::new(&n)));
            a.sels.insert(i, Sel::cell(Pos::new(0, 0)));
            a.views.insert(i, (0, 0));
            a.cur = i;
        });
    }

    /// Applies a style change to the selection (with undo).
    pub fn format(&mut self, f: impl Fn(&mut Style) + 'static) {
        let rects = self.sel().ranges.clone();
        if self.sheet().protection.is_some() && rects.iter().any(|r| self.deny_protected(*r)) {
            return;
        }
        self.act(|a| {
            ops::style_ranges(&mut a.wb, a.cur, &rects, &f);
        });
        let r = self.sel().rect();
        if r.rows() < 2000 {
            let last = self.sheet().used_rect().map_or(r.r1, |u| u.r2).min(r.r2);
            self.autofit_rows(r.r1, last.max(r.r1));
        }
    }

    pub fn fill_dir(&mut self, dir: u8) {
        let r = self.sel().rect();
        self.act(|a| match dir {
            0 => {
                let src = if r.rows() == 1 && r.r1 > 0 { Rect::new(r.r1 - 1, r.c1, r.r1 - 1, r.c2) } else { Rect::new(r.r1, r.c1, r.r1, r.c2) };
                ops::autofill(&mut a.wb, a.cur, src, src.union(&r), true);
            }
            1 => {
                let src = if r.cols() == 1 && r.c1 > 0 { Rect::new(r.r1, r.c1 - 1, r.r2, r.c1 - 1) } else { Rect::new(r.r1, r.c1, r.r2, r.c1) };
                ops::autofill(&mut a.wb, a.cur, src, src.union(&r), true);
            }
            2 => {
                let src = Rect::new(r.r2, r.c1, r.r2, r.c2);
                if r.r1 < r.r2 {
                    ops::autofill(&mut a.wb, a.cur, src, r, true);
                }
            }
            _ => {
                let src = Rect::new(r.r1, r.c2, r.r2, r.c2);
                if r.c1 < r.c2 {
                    ops::autofill(&mut a.wb, a.cur, src, r, true);
                }
            }
        });
    }

    /// Statistics shown in the status bar for the selection.
    pub fn stats(&self) -> Option<(f64, usize, f64, usize, f64, f64)> {
        let s = self.sel();
        let (mut sum, mut count, mut nums) = (0.0, 0usize, 0usize);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        let mut cells = 0;
        for r in &s.ranges {
            if r.is_single() && s.ranges.len() == 1 {
                return None;
            }
            let used = self.sheet().used_rect().map(|u| u.union(&Rect::new(0, 0, 0, 0)));
            let Some(u) = used else { continue };
            let Some(x) = r.intersection(&Rect::new(0, 0, u.r2 + 1, u.c2 + 1)) else { continue };
            for row in x.r1..=x.r2 {
                if self.sheet().row_hidden(row) {
                    continue;
                }
                for c in x.c1..=x.c2 {
                    cells += 1;
                    if cells > 2_000_000 {
                        break;
                    }
                    match self.calc.value(&self.wb, self.cur, row, c) {
                        Value::Num(n) => {
                            sum += n;
                            nums += 1;
                            count += 1;
                            lo = lo.min(n);
                            hi = hi.max(n);
                        }
                        Value::Empty => {}
                        _ => count += 1,
                    }
                }
            }
        }
        if count == 0 {
            return Some((0.0, 0, 0.0, 0, 0.0, 0.0));
        }
        Some((if nums > 0 { sum / nums as f64 } else { 0.0 }, count, sum, nums, lo, hi))
    }

    pub fn title(&self) -> String {
        format!("{}{} - Tabula", self.name, if self.modified && self.path.is_some() { " •" } else { "" })
    }
}
