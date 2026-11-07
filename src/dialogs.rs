//! Dialog state and the commands behind ribbon buttons and menus.

use crate::app::App;
use crate::eval::{self, Value};
use crate::model::*;
use crate::ops::{self, PasteOpts};
use crate::theme;
use egui::{vec2, Color32, Rect as R};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct FmtState {
    pub tab: u8,
    pub style: Style,
    pub cat: usize,
    pub decimals: u32,
    pub thousands: bool,
    pub neg: usize,
    pub code: String,
    pub sample: Value,
    pub border_color: u32,
    pub border_style: LineStyle,
    pub changed_border: [bool; 4],
    pub inner: [Option<Option<Border>>; 2],
}

#[derive(Clone, Debug)]
pub enum Dialog {
    FormatCells(Box<FmtState>),
    FindReplace(u8),
    GoTo(String),
    GoSpecial(u8),
    DeleteCells(u8),
    InsertCells(u8),
    Zoom(u32),
    NotesList,
    Symbol,
    NameManager(Option<usize>),
    DefineName { name: String, refers: String, comment: String, editing: Option<usize> },
    TextToColumns { comma: bool, tab: bool, semicolon: bool, space: bool, other: String },
    Protect { password: String, select_locked: bool },
    Unprotect(String),
    Stats,
    Unhide(usize),
    Shortcuts,
    PasteSpecial(PasteOpts),
    MoreColors { fill: bool, color: [u8; 3] },
    CreateTable { range: String, header: bool, style: u8 },
    Size { rows: bool, value: String },
    MoveCopy { to: usize, copy: bool },
    Series { rows: bool, step: String, stop: String, kind: u8 },
    CondQuick { op: String, a: String, b: String, style: u8 },
    CondManager(usize),
    GoalSeek { set: String, to: String, change: String },
    Note { pos: Pos, text: String },
    InsertFunction { search: String, cat: String, sel: String },
    Hyperlink { pos: Pos, text: String, addr: String },
    Pivot { source: String, headers: Vec<String>, rows: Vec<u32>, cols: Vec<u32>, values: Vec<(u32, String)>, new_sheet: bool, at: String },
    Sparkline { kind: u8, data: String, loc: String },
    PageSetup(Box<PageSetup>, u8),
    Sort { range: Rect, header: bool, keys: Vec<(u32, bool)>, left_right: bool },
    Dedupe { range: Rect, header: bool, cols: Vec<bool> },
    Validation(Box<Validation>, u8),
    Subtotal { range: Rect, at_change: u32, func: usize, cols: Vec<bool> },
    ChartData { i: usize, title: String, range: String, x_title: String, y_title: String },
}

impl Dialog {
    /// Modal dialogs block the grid. The others let the person select cells while open.
    pub fn modal(&self) -> bool {
        !matches!(
            self,
            Dialog::FindReplace(_) | Dialog::NotesList | Dialog::CreateTable { .. } | Dialog::DefineName { .. } | Dialog::Validation(..) | Dialog::ChartData { .. } | Dialog::Pivot { .. } | Dialog::Sparkline { .. } | Dialog::GoalSeek { .. } | Dialog::CondQuick { .. } | Dialog::Sort { .. } | Dialog::Hyperlink { .. }
        )
    }

    pub fn format_cells(a: &App, tab: u8) -> Dialog {
        let p = a.sel().active;
        let style = (*a.style_at(p)).clone();
        let sample = a.calc.value(&a.wb, a.cur, p.row, p.col);
        let (cat, decimals, thousands) = classify(&style.num_fmt);
        Dialog::FormatCells(Box::new(FmtState { tab, code: style.num_fmt.to_string(), style, cat, decimals, thousands, neg: 0, sample, border_color: 0, border_style: LineStyle::Thin, changed_border: [false; 4], inner: [None, None] }))
    }
    pub fn note_for(a: &App) -> Dialog {
        let p = a.sel().active;
        let text = a.sheet().notes.get(&p).map(|n| n.text.clone()).unwrap_or_default();
        Dialog::Note { pos: p, text }
    }
    pub fn insert_function(_a: &App) -> Dialog {
        Dialog::InsertFunction { search: String::new(), cat: "Most Recently Used".into(), sel: "SUM".into() }
    }
    pub fn hyperlink(a: &App) -> Dialog {
        let p = a.sel().active;
        let text = ops::text_of(&a.wb, &a.calc, a.cur, p.row, p.col);
        let addr = a.sheet().links.get(&p).cloned().unwrap_or_default();
        Dialog::Hyperlink { pos: p, text, addr }
    }
    pub fn create_table(a: &App) -> Dialog {
        let p = a.sel().active;
        let existing = a.sheet().tables.iter().find(|t| t.range.contains(p));
        let r = existing.map(|t| t.range).unwrap_or_else(|| if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() });
        Dialog::CreateTable { range: r.a1_abs(), header: true, style: existing.map_or(2, |t| t.style) }
    }
    pub fn create_pivot(a: &App) -> Dialog {
        let p = a.sel().active;
        let r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
        let headers = (r.c1..=r.c2).map(|c| ops::text_of(&a.wb, &a.calc, a.cur, r.r1, c)).collect();
        Dialog::Pivot { source: format!("{}!{}", crate::formula::quote_sheet(&a.sheet().name), r.a1_abs()), headers, rows: vec![], cols: vec![], values: vec![], new_sheet: true, at: String::new() }
    }
    pub fn sparkline(a: &App, kind: u8) -> Dialog {
        let r = a.sel().rect();
        Dialog::Sparkline { kind, data: if r.is_single() { String::new() } else { r.a1() }, loc: String::new() }
    }
    pub fn page_setup(a: &App) -> Dialog {
        Dialog::PageSetup(Box::new(a.sheet().page.clone()), 0)
    }
    pub fn define_name(a: &App) -> Dialog {
        let r = a.sel().rect();
        let guess = if r.r1 > 0 { ops::text_of(&a.wb, &a.calc, a.cur, r.r1.saturating_sub(1), r.c1) } else { String::new() };
        let name: String = guess.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
        Dialog::DefineName { name, refers: format!("={}!{}", crate::formula::quote_sheet(&a.sheet().name), r.a1_abs()), comment: String::new(), editing: None }
    }
    pub fn sort(a: &App) -> Dialog {
        let p = a.sel().active;
        let r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
        let header = guess_header(a, r);
        Dialog::Sort { range: r, header, keys: vec![(p.col.clamp(r.c1, r.c2), true)], left_right: false }
    }
    pub fn remove_duplicates(a: &App) -> Dialog {
        let p = a.sel().active;
        let r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
        Dialog::Dedupe { range: r, header: guess_header(a, r), cols: vec![true; r.cols() as usize] }
    }
    pub fn validation(a: &App) -> Dialog {
        let p = a.sel().active;
        let v = a.sheet().valid.iter().find(|v| v.ranges.iter().any(|r| r.contains(p))).cloned().unwrap_or_default();
        Dialog::Validation(Box::new(v), 0)
    }
    pub fn subtotal(a: &App) -> Dialog {
        let p = a.sel().active;
        let r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
        let mut cols = vec![false; r.cols() as usize];
        if let Some(l) = cols.last_mut() {
            *l = true;
        }
        Dialog::Subtotal { range: r, at_change: 0, func: 0, cols }
    }
    pub fn chart_data(a: &App, i: usize) -> Dialog {
        let c = &a.sheet().charts[i];
        let range = crate::charts::data_range_text(c);
        Dialog::ChartData { i, title: c.title.clone(), range, x_title: c.x_title.clone(), y_title: c.y_title.clone() }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FindState {
    pub what: String,
    pub with: String,
    pub replace: bool,
    pub match_case: bool,
    pub whole: bool,
    pub workbook: bool,
    pub in_values: bool,
    pub results: Vec<(usize, Pos, String)>,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct FilterPanel {
    pub col: u32,
    pub search: String,
    pub items: Vec<(String, bool)>,
    pub custom_op: String,
    pub custom_val: String,
}

pub fn guess_header(a: &App, r: Rect) -> bool {
    if r.rows() < 2 {
        return false;
    }
    let first_text = (r.c1..=r.c2).all(|c| matches!(a.calc.value(&a.wb, a.cur, r.r1, c), Value::Text(_)));
    let second_num = (r.c1..=r.c2).any(|c| !matches!(a.calc.value(&a.wb, a.cur, r.r1 + 1, c), Value::Text(_)));
    first_text && (second_num || a.style_at(Pos::new(r.r1, r.c1)).bold)
}

pub const QUICK_FORMATS: [(&str, &str); 11] = [
    ("General", "General"),
    ("Number", "0.00"),
    ("Currency", "\"$\"#,##0.00"),
    ("Accounting", "_(\"$\"* #,##0.00_);_(\"$\"* \\(#,##0.00\\);_(\"$\"* \"-\"??_);_(@_)"),
    ("Short Date", "m/d/yyyy"),
    ("Long Date", "[$-x-sysdate]dddd, mmmm d, yyyy"),
    ("Time", "[$-x-systime]h:mm:ss AM/PM"),
    ("Percentage", "0.00%"),
    ("Fraction", "# ?/?"),
    ("Scientific", "0.00E+00"),
    ("Text", "@"),
];

pub const CATEGORIES: [&str; 11] = ["General", "Number", "Currency", "Accounting", "Date", "Time", "Percentage", "Fraction", "Scientific", "Text", "Custom"];

/// (category index, decimals, thousands separator) read back from a format code.
pub fn classify(code: &str) -> (usize, u32, bool) {
    let decimals = code.split(';').next().unwrap_or("").split('.').nth(1).map_or(0, |d| d.chars().take_while(|c| *c == '0').count() as u32);
    let thousands = code.contains("#,##");
    let c = if code.eq_ignore_ascii_case("general") {
        0
    } else if code == "@" {
        9
    } else if code.contains("_(") && code.contains('*') {
        3
    } else if code.contains('$') && !code.contains("[$-") {
        2
    } else if code.contains('%') {
        6
    } else if code.contains("E+") {
        8
    } else if code.contains('?') && code.contains('/') {
        7
    } else if crate::numfmt::is_date_format(code) {
        if code.contains('h') && !code.contains('d') && !code.contains('y') { 5 } else { 4 }
    } else if code.chars().all(|c| "0#,.-()".contains(c)) || code.starts_with("0.0") || code == "0" {
        1
    } else {
        10
    };
    (c, decimals, thousands)
}

pub fn format_label(code: &str) -> String {
    let (c, _, _) = classify(code);
    if c == 4 && (code.contains("dddd") || code.contains("mmmm")) {
        return "Long Date".into();
    }
    if c == 4 {
        return "Short Date".into();
    }
    CATEGORIES[c].to_string()
}

/// Builds a code from the Number tab's choices.
pub fn build_code(cat: usize, decimals: u32, thousands: bool, neg: usize) -> String {
    let dec = if decimals > 0 { format!(".{}", "0".repeat(decimals as usize)) } else { String::new() };
    let base = if thousands { format!("#,##0{dec}") } else { format!("0{dec}") };
    match cat {
        0 => "General".into(),
        1 => match neg {
            1 => format!("{base};[Red]{base}"),
            2 => format!("{base}_);\\({base}\\)"),
            3 => format!("{base}_);[Red]\\({base}\\)"),
            _ => base,
        },
        2 => {
            let b = format!("\"$\"#,##0{dec}");
            match neg {
                1 => format!("{b};[Red]{b}"),
                2 => format!("{b}_);\\({b}\\)"),
                3 => format!("{b}_);[Red]\\({b}\\)"),
                _ => b,
            }
        }
        3 => format!("_(\"$\"* #,##0{dec}_);_(\"$\"* \\(#,##0{dec}\\);_(\"$\"* \"-\"??_);_(@_)"),
        6 => format!("0{dec}%"),
        8 => format!("0{dec}E+00"),
        9 => "@".into(),
        _ => base,
    }
}

pub fn change_decimals(code: &str, shown: &str, inc: bool) -> String {
    let code = if code.eq_ignore_ascii_case("general") {
        let d = shown.split('.').nth(1).map_or(0, |x| x.chars().filter(|c| c.is_ascii_digit()).count());
        if d == 0 { "0".to_string() } else { format!("0.{}", "0".repeat(d)) }
    } else {
        code.to_string()
    };
    // Change the decimals in every section of the code.
    code.split(';')
        .map(|sec| {
            let chars: Vec<char> = sec.chars().collect();
            let last_zero = chars.iter().rposition(|c| *c == '0');
            let Some(lz) = last_zero else { return sec.to_string() };
            let dot = chars[..=lz].iter().rposition(|c| *c == '.');
            let mut s: String;
            if inc {
                s = chars[..=lz].iter().collect();
                if dot.is_none() {
                    s.push('.');
                }
                s.push('0');
            } else {
                match dot {
                    Some(d) if lz == d + 1 => s = chars[..d].iter().collect(),
                    Some(_) => s = chars[..lz].iter().collect(),
                    None => return sec.to_string(),
                }
            }
            s.extend(chars[lz + 1..].iter());
            s
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub fn validate_input(a: &App, p: Pos, text: &str) -> Option<String> {
    let v = a.sheet().valid.iter().find(|v| v.ranges.iter().any(|r| r.contains(p)))?;
    if v.kind == ValidKind::Any || v.error_style != 0 {
        return None;
    }
    if text.is_empty() && v.ignore_blank {
        return None;
    }
    let typed = crate::numfmt::parse_input(text).content;
    let ctx = eval::Ctx::reader(&a.wb, &a.calc);
    let num = |s: &str| -> Option<f64> {
        if s.is_empty() {
            return None;
        }
        let src = if s.starts_with('=') { s.to_string() } else { format!("={s}") };
        eval::to_num(&ctx.eval_text(&src, a.cur, p)).ok().or_else(|| crate::numfmt::parse_input(s).content_num())
    };
    let ok = match v.kind {
        ValidKind::List => list_items(a, p).iter().any(|x| x.eq_ignore_ascii_case(text)),
        ValidKind::Custom => {
            let src = if v.a.starts_with('=') { v.a.clone() } else { format!("={}", v.a) };
            // Evaluate as if the cell already held the typed value.
            let mut wb = a.wb.clone();
            let _ = ops::set_input(&mut wb, a.cur, p.row, p.col, text);
            let calc = eval::recalc(&wb);
            let c2 = eval::Ctx::reader(&wb, &calc);
            eval::to_bool(&c2.eval_text(&src, a.cur, p)).unwrap_or(false)
        }
        _ => {
            let x = match (&v.kind, &typed) {
                (ValidKind::TextLength, _) => Some(text.chars().count() as f64),
                (_, Content::Num(n)) => Some(*n),
                _ => None,
            };
            match x {
                None => false,
                Some(x) => {
                    if v.kind == ValidKind::Whole && x.fract() != 0.0 {
                        false
                    } else {
                        let (lo, hi) = (num(&v.a), num(&v.b));
                        match v.op.as_str() {
                            "between" => lo.is_some_and(|l| x >= l) && hi.is_some_and(|h| x <= h),
                            "notBetween" => !(lo.is_some_and(|l| x >= l) && hi.is_some_and(|h| x <= h)),
                            "equal" => lo == Some(x),
                            "notEqual" => lo != Some(x),
                            "greaterThan" => lo.is_some_and(|l| x > l),
                            "lessThan" => lo.is_some_and(|l| x < l),
                            "greaterThanOrEqual" => lo.is_some_and(|l| x >= l),
                            "lessThanOrEqual" => lo.is_some_and(|l| x <= l),
                            _ => true,
                        }
                    }
                }
            }
        }
    };
    if ok {
        None
    } else if v.error.is_empty() {
        Some("This value doesn't match the data validation restrictions defined for this cell.".into())
    } else {
        Some(v.error.clone())
    }
}

trait ContentNum {
    fn content_num(&self) -> Option<f64>;
}
impl ContentNum for crate::numfmt::Typed {
    fn content_num(&self) -> Option<f64> {
        match self.content {
            Content::Num(n) => Some(n),
            _ => None,
        }
    }
}

/// Items of a list validation for a cell: typed items or the values of a range.
pub fn list_items(a: &App, p: Pos) -> Vec<String> {
    let Some(v) = a.sheet().valid.iter().find(|v| v.kind == ValidKind::List && v.ranges.iter().any(|r| r.contains(p))) else { return vec![] };
    let src = v.a.trim();
    if src.starts_with('=') || parse_rect(src).is_some() || src.contains('!') {
        let ctx = eval::Ctx::reader(&a.wb, &a.calc);
        let f = if src.starts_with('=') { src.to_string() } else { format!("={src}") };
        let val = ctx.eval_text(&f, a.cur, p);
        let val = ctx.arrayish(val);
        let mut out = vec![];
        if let Value::Array(arr) = val {
            for x in &arr.data {
                let t = eval::to_text(x).unwrap_or_default();
                if !t.is_empty() {
                    out.push(t);
                }
            }
        } else if let Ok(t) = eval::to_text(&val) {
            out.push(t);
        }
        out
    } else {
        src.trim_matches('"').split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    }
}

pub fn toggle_filter(a: &mut App) {
    if a.sheet().filter.is_some() {
        a.act(|a| {
            let sh = a.wb.sheet_mut(a.cur);
            sh.filter = None;
            sh.filtered_rows.clear();
        });
        return;
    }
    let p = a.sel().active;
    let r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
    if a.sheet().is_empty_at(r.r1, r.c1) && r.is_single() {
        a.alert = Some(("Filter".into(), "Select a cell in a range of data first, then try again.".into()));
        return;
    }
    a.act(|a| a.wb.sheet_mut(a.cur).filter = Some(AutoFilter { range: r, cols: Default::default() }));
}

pub fn clear_filter(a: &mut App) {
    if a.sheet().filter.is_none() {
        return;
    }
    a.act(|a| {
        let sh = a.wb.sheet_mut(a.cur);
        if let Some(f) = sh.filter.as_mut() {
            f.cols.clear();
        }
        sh.filtered_rows.clear();
    });
}

/// Prepares the filter dropdown for a column, making a filter on tables when needed.
pub fn open_filter(a: &mut App, r: u32, c: u32) {
    if a.sheet().filter.as_ref().is_none_or(|f| !f.range.contains(Pos::new(r, c))) {
        if let Some(t) = a.sheet().tables.iter().find(|t| t.range.contains(Pos::new(r, c))).cloned() {
            let rng = if t.totals { Rect::new(t.range.r1, t.range.c1, t.range.r2 - 1, t.range.c2) } else { t.range };
            a.wb.sheet_mut(a.cur).filter = Some(AutoFilter { range: rng, cols: Default::default() });
        }
    }
    let Some(f) = a.sheet().filter.clone() else { return };
    let k = c - f.range.c1;
    let allowed = f.cols.get(&k).and_then(|x| x.allowed.clone());
    let mut vals = BTreeSet::new();
    for row in f.range.r1 + 1..=f.range.r2 {
        vals.insert(ops::text_of(&a.wb, &a.calc, a.cur, row, c));
    }
    let mut items: Vec<(String, bool)> = vals.into_iter().map(|v| {
        let on = allowed.as_ref().is_none_or(|s| s.contains(&v));
        (v, on)
    }).collect();
    items.sort_by(|x, y| match (x.0.parse::<f64>(), y.0.parse::<f64>()) {
        (Ok(p), Ok(q)) => p.partial_cmp(&q).unwrap_or(std::cmp::Ordering::Equal),
        _ => x.0.to_lowercase().cmp(&y.0.to_lowercase()),
    });
    if let Some(i) = items.iter().position(|x| x.0.is_empty()) {
        let b = items.remove(i);
        items.push(b);
    }
    let custom = f.cols.get(&k).and_then(|x| x.custom.clone()).unwrap_or_default();
    a.filter_state = Some(FilterPanel { col: c, search: String::new(), items, custom_op: custom.0, custom_val: custom.1 });
}

pub fn filter_menu(a: &mut App, ui: &mut egui::Ui, c: u32) -> bool {
    use crate::chrome::{item, separator};
    use crate::icons::Icon;
    if a.filter_state.as_ref().is_none_or(|f| f.col != c) {
        return true;
    }
    let f = a.sheet().filter.clone();
    let Some(f) = f else { return true };
    if item(ui, Some(Icon::SortAZ), "Sort A to Z", "") {
        let keys = [(c, true)];
        let r = Rect::new(f.range.r1 + 1, f.range.c1, f.range.r2, f.range.c2);
        a.act(|a| ops::sort_range(&mut a.wb, &a.calc, a.cur, r, &keys, false));
        return true;
    }
    if item(ui, Some(Icon::SortZA), "Sort Z to A", "") {
        let keys = [(c, false)];
        let r = Rect::new(f.range.r1 + 1, f.range.c1, f.range.r2, f.range.c2);
        a.act(|a| ops::sort_range(&mut a.wb, &a.calc, a.cur, r, &keys, false));
        return true;
    }
    separator(ui);
    let k = c - f.range.c1;
    let header = ops::text_of(&a.wb, &a.calc, a.cur, f.range.r1, c);
    let active = f.cols.get(&k).is_some_and(|x| x.allowed.is_some() || x.custom.is_some());
    if crate::chrome::item_on(ui, Some(Icon::FilterClear), &format!("Clear Filter From \"{header}\""), "", active, false) {
        a.act(|a| {
            if let Some(ff) = a.wb.sheet_mut(a.cur).filter.as_mut() {
                ff.cols.remove(&k);
            }
            ops::apply_filter(&mut a.wb, &a.calc, a.cur);
        });
        return true;
    }
    let st = a.filter_state.as_mut().unwrap();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Filter:").size(12.0));
        egui::ComboBox::from_id_salt("custom-op").width(110.0).selected_text(if st.custom_op.is_empty() { "(none)".to_string() } else { st.custom_op.clone() }).show_ui(ui, |ui| {
            for op in ["", "equals", "does not equal", "contains", "does not contain", "begins with", "ends with", "greater than", "less than", "top 10", "above average", "below average"] {
                ui.selectable_value(&mut st.custom_op, op.to_string(), if op.is_empty() { "(none)" } else { op });
            }
        });
        ui.add(egui::TextEdit::singleline(&mut st.custom_val).desired_width(70.0));
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Search").desired_width(220.0));
    });
    let q = st.search.to_lowercase();
    egui::Frame::new().stroke(egui::Stroke::new(1.0, Color32::from_rgb(0xC8, 0xC8, 0xC8))).inner_margin(4.0).outer_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(200.0).min_scrolled_height(160.0).show(ui, |ui| {
            ui.set_min_width(210.0);
            let all = st.items.iter().filter(|x| x.0.to_lowercase().contains(&q)).all(|x| x.1);
            let mut all2 = all;
            if ui.checkbox(&mut all2, "(Select All)").changed() {
                for it in st.items.iter_mut().filter(|x| x.0.to_lowercase().contains(&q)) {
                    it.1 = all2;
                }
            }
            for it in st.items.iter_mut().filter(|x| x.0.to_lowercase().contains(&q)) {
                let label = if it.0.is_empty() { "(Blanks)".to_string() } else { it.0.clone() };
                ui.checkbox(&mut it.1, label);
            }
        });
    });
    let mut done = false;
    ui.horizontal(|ui| {
        ui.add_space(110.0);
        if ui.add(egui::Button::new("   OK   ").fill(Color32::from_rgb(0xE5, 0xF1, 0xEA))).clicked() {
            let st = a.filter_state.clone().unwrap();
            let all_on = st.items.iter().all(|x| x.1);
            let allowed: Option<BTreeSet<String>> = if all_on && st.search.is_empty() { None } else { Some(st.items.iter().filter(|x| x.1 && x.0.to_lowercase().contains(&q)).map(|x| x.0.clone()).collect()) };
            let custom = if st.custom_op.is_empty() { None } else { Some((st.custom_op.clone(), st.custom_val.clone())) };
            a.act(|a| {
                if let Some(ff) = a.wb.sheet_mut(a.cur).filter.as_mut() {
                    if allowed.is_none() && custom.is_none() {
                        ff.cols.remove(&k);
                    } else {
                        ff.cols.insert(k, ColFilter { allowed, custom });
                    }
                }
                ops::apply_filter(&mut a.wb, &a.calc, a.cur);
            });
            done = true;
        }
        if ui.button("Cancel").clicked() {
            done = true;
        }
    });
    done
}

pub fn filter_by_value(a: &mut App) {
    let p = a.sel().active;
    if a.sheet().filter.is_none() {
        toggle_filter(a);
    }
    let Some(f) = a.sheet().filter.clone() else { return };
    if !f.range.contains(p) {
        return;
    }
    let v = ops::text_of(&a.wb, &a.calc, a.cur, p.row, p.col);
    let k = p.col - f.range.c1;
    a.act(|a| {
        if let Some(ff) = a.wb.sheet_mut(a.cur).filter.as_mut() {
            ff.cols.insert(k, ColFilter { allowed: Some([v].into_iter().collect()), custom: None });
        }
        ops::apply_filter(&mut a.wb, &a.calc, a.cur);
    });
}

pub fn quick_sort(a: &mut App, asc: bool) {
    let p = a.sel().active;
    let mut r = if a.sel().rect().is_single() { ops::current_region(a.sheet(), p) } else { a.sel().rect() };
    if let Some(f) = &a.sheet().filter {
        if f.range.contains(p) {
            r = f.range;
        }
    }
    if guess_header(a, r) {
        r.r1 += 1;
    }
    if r.r1 > r.r2 {
        return;
    }
    let key = p.col.clamp(r.c1, r.c2);
    a.act(|a| ops::sort_range(&mut a.wb, &a.calc, a.cur, r, &[(key, asc)], false));
}

pub fn merge_center(a: &mut App) {
    let rects = a.sel().ranges.clone();
    let merged = a.sheet().merge_at(a.sel().active).is_some();
    if !merged {
        let lost = rects.iter().any(|r| (r.r1..=r.r2).flat_map(|row| (r.c1..=r.c2).map(move |c| (row, c))).filter(|(row, c)| !a.sheet().is_empty_at(*row, *c)).count() > 1);
        if lost {
            a.status = "Merging cells only keeps the upper-left value and discards other values.".into();
        }
    }
    a.act(|a| {
        ops::set_merge(&mut a.wb, a.cur, &rects, if merged { 2 } else { 0 });
        if !merged {
            ops::style_ranges(&mut a.wb, a.cur, &rects, &|s| s.h = HAlign::Center);
        }
    });
}

/// Border presets from the Borders menu.
pub fn apply_border_preset(a: &mut App, k: u8) {
    let rects = a.sel().ranges.clone();
    let thin = Some(Border { style: LineStyle::Thin, color: 0 });
    let thick = Some(Border { style: LineStyle::Medium, color: 0 });
    let double = Some(Border { style: LineStyle::Double, color: 0 });
    a.act(|a| {
        for rc in &rects {
            let set = |a: &mut App, part: Rect, f: &dyn Fn(&mut Style)| ops::style_ranges(&mut a.wb, a.cur, &[part], f);
            let top = Rect::new(rc.r1, rc.c1, rc.r1, rc.c2);
            let bottom = Rect::new(rc.r2, rc.c1, rc.r2, rc.c2);
            let left = Rect::new(rc.r1, rc.c1, rc.r2, rc.c1);
            let right = Rect::new(rc.r1, rc.c2, rc.r2, rc.c2);
            match k {
                0 => set(a, bottom, &|s| s.borders[B_BOTTOM] = thin),
                1 => set(a, top, &|s| s.borders[B_TOP] = thin),
                2 => set(a, left, &|s| s.borders[B_LEFT] = thin),
                3 => set(a, right, &|s| s.borders[B_RIGHT] = thin),
                4 => {
                    set(a, *rc, &|s| s.borders = [None; 4]);
                    // Also clear the touching edges of neighbours.
                    if rc.r1 > 0 {
                        set(a, Rect::new(rc.r1 - 1, rc.c1, rc.r1 - 1, rc.c2), &|s| s.borders[B_BOTTOM] = None);
                    }
                    if rc.c1 > 0 {
                        set(a, Rect::new(rc.r1, rc.c1 - 1, rc.r2, rc.c1 - 1), &|s| s.borders[B_RIGHT] = None);
                    }
                }
                5 => set(a, *rc, &|s| s.borders = [thin; 4]),
                6 | 7 => {
                    let b = if k == 7 { thick } else { thin };
                    set(a, top, &|s| s.borders[B_TOP] = b);
                    set(a, bottom, &|s| s.borders[B_BOTTOM] = b);
                    set(a, left, &|s| s.borders[B_LEFT] = b);
                    set(a, right, &|s| s.borders[B_RIGHT] = b);
                }
                8 => set(a, bottom, &|s| s.borders[B_BOTTOM] = double),
                9 => set(a, bottom, &|s| s.borders[B_BOTTOM] = thick),
                10 => {
                    set(a, top, &|s| s.borders[B_TOP] = thin);
                    set(a, bottom, &|s| s.borders[B_BOTTOM] = thin);
                }
                _ => {
                    set(a, top, &|s| s.borders[B_TOP] = thin);
                    set(a, bottom, &|s| s.borders[B_BOTTOM] = thick);
                }
            }
        }
    });
}

pub fn insert_rows_cols(a: &mut App, rows: bool) {
    if a.sheet().protection.is_some() {
        a.deny_protected(Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1));
        return;
    }
    let r = a.sel().rect();
    a.act(|a| {
        if rows {
            ops::insert_rows(&mut a.wb, a.cur, r.r1, r.rows().min(10_000));
        } else {
            ops::insert_cols(&mut a.wb, a.cur, r.c1, r.cols().min(1000));
        }
    });
}

pub fn delete_rows_cols(a: &mut App, rows: bool) {
    if a.sheet().protection.is_some() {
        a.deny_protected(Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1));
        return;
    }
    let r = a.sel().rect();
    a.act(|a| {
        if rows {
            ops::delete_rows(&mut a.wb, a.cur, r.r1, r.rows());
        } else {
            ops::delete_cols(&mut a.wb, a.cur, r.c1, r.cols());
        }
    });
    let p = Pos::new(r.r1.min(MAX_ROWS - 1), r.c1.min(MAX_COLS - 1));
    a.select(p, false);
}

pub fn delete_sheet(a: &mut App, i: usize) {
    if a.wb.protected {
        a.alert = Some(("Protected workbook".into(), "The workbook is protected, so sheets can't be added, deleted or renamed.".into()));
        return;
    }
    if a.wb.sheets.iter().filter(|s| !s.hidden).count() <= 1 {
        a.alert = Some(("Delete sheet".into(), "A workbook must contain at least one visible worksheet.".into()));
        return;
    }
    a.act(|a| {
        ops::delete_sheet(&mut a.wb, i);
        a.sels.remove(i);
        a.views.remove(i);
        a.cur = a.cur.min(a.wb.sheets.len() - 1);
        if a.wb.sheets[a.cur].hidden {
            a.cur = a.wb.sheets.iter().position(|s| !s.hidden).unwrap_or(0);
        }
    });
}

pub fn hide_sheet(a: &mut App) {
    if a.wb.sheets.iter().filter(|s| !s.hidden).count() <= 1 {
        a.alert = Some(("Hide sheet".into(), "A workbook must contain at least one visible worksheet. To hide this sheet, first insert a new sheet or unhide one.".into()));
        return;
    }
    a.act(|a| {
        a.wb.sheet_mut(a.cur).hidden = true;
    });
    a.next_sheet(1);
}

/// 0 hide rows, 1 hide columns, 2 hide sheet, 3 unhide rows, 4 unhide columns, 5 unhide sheet.
pub fn hide_unhide(a: &mut App, k: u8) {
    let rects = a.sel().ranges.clone();
    match k {
        2 => hide_sheet(a),
        5 => a.dialog = Some(Dialog::Unhide(0)),
        _ => a.act(|a| {
            let sh = a.wb.sheet_mut(a.cur);
            for r in &rects {
                match k {
                    0 => sh.hidden_rows.extend(r.r1..=r.r2.min(r.r1 + 100_000)),
                    1 => sh.hidden_cols.extend(r.c1..=r.c2),
                    3 => {
                        let (lo, hi) = (r.r1.saturating_sub(if r.is_single() { 1 } else { 0 }), r.r2 + 1);
                        sh.hidden_rows.retain(|x| *x < lo || *x > hi);
                    }
                    _ => {
                        let (lo, hi) = (r.c1.saturating_sub(if r.is_single() { 1 } else { 0 }), r.c2 + 1);
                        sh.hidden_cols.retain(|x| *x < lo || *x > hi);
                    }
                }
            }
        }),
    }
}

/// Selects cells of one kind: 1 formulas, 2 notes, 3 conditional formats, 4 constants, 5 validation, 6 blanks.
pub fn select_special(a: &mut App, k: u8) {
    let sh = a.sheet();
    let mut cells: Vec<Rect> = vec![];
    match k {
        2 => cells.extend(sh.notes.keys().map(|p| Rect::cell(*p))),
        3 => cells.extend(sh.cond.iter().flat_map(|c| c.ranges.iter().copied())),
        5 => cells.extend(sh.valid.iter().flat_map(|c| c.ranges.iter().copied())),
        6 => {
            let u = sh.used_rect().unwrap_or(Rect::new(0, 0, 0, 0));
            let scope = if a.sel().rect().is_single() { u } else { a.sel().rect().intersection(&u).unwrap_or(u) };
            for r in scope.r1..=scope.r2 {
                for c in scope.c1..=scope.c2 {
                    if sh.is_empty_at(r, c) {
                        cells.push(Rect::cell(Pos::new(r, c)));
                    }
                }
            }
        }
        _ => {
            for (r, row) in &sh.rows {
                for (c, cell) in row.iter() {
                    let f = matches!(cell.v, Content::Formula(_));
                    if (k == 1 && f) || (k == 4 && !f && cell.v != Content::Empty) {
                        cells.push(Rect::cell(Pos::new(*r, *c)));
                    }
                }
            }
        }
    }
    if cells.is_empty() {
        a.alert = Some(("Go To Special".into(), "No cells were found.".into()));
        return;
    }
    cells.truncate(5000);
    let first = Pos::new(cells[0].r1, cells[0].c1);
    *a.sel_mut() = crate::app::Sel { ranges: cells, active: first, anchor: first };
    a.reveal(first);
}

pub fn find_matches(a: &App) -> Vec<(usize, Pos, String)> {
    let f = &a.find;
    if f.what.is_empty() {
        return vec![];
    }
    let sheets: Vec<usize> = if f.workbook { (0..a.wb.sheets.len()).collect() } else { vec![a.cur] };
    let pat: Vec<char> = if f.match_case { f.what.chars().collect() } else { f.what.to_lowercase().chars().collect() };
    let has_wild = pat.contains(&'*') || pat.contains(&'?');
    let mut out = vec![];
    for si in sheets {
        for (r, row) in &a.wb.sheets[si].rows {
            for (c, cell) in row.iter() {
                if cell.v == Content::Empty {
                    continue;
                }
                let text = if f.in_values { ops::text_of(&a.wb, &a.calc, si, *r, *c) } else { ops::edit_text(&a.wb, si, *r, *c) };
                let t: Vec<char> = if f.match_case { text.chars().collect() } else { text.to_lowercase().chars().collect() };
                let hit = if f.whole {
                    if has_wild { eval::wild(&pat, &t) } else { t == pat }
                } else if has_wild {
                    let mut p2 = vec!['*'];
                    p2.extend(pat.iter());
                    p2.push('*');
                    eval::wild(&p2, &t)
                } else {
                    t.windows(pat.len().max(1)).any(|w| w == pat.as_slice())
                };
                if hit {
                    out.push((si, Pos::new(*r, *c), text));
                }
            }
        }
    }
    out
}

pub fn find_next(a: &mut App, back: bool) {
    let found = find_matches(a);
    if found.is_empty() {
        a.find.message = "We couldn't find what you were looking for.".into();
        a.alert = Some(("Find".into(), format!("We couldn't find what you were looking for. Select Options for more ways to search.\n\n\"{}\"", a.find.what)));
        return;
    }
    let cur = (a.cur, a.sel().active);
    let key = |x: &(usize, Pos, String)| (x.0, x.1.row, x.1.col);
    let ck = (cur.0, cur.1.row, cur.1.col);
    let next = if back { found.iter().rev().find(|x| key(x) < ck).or(found.last()) } else { found.iter().find(|x| key(x) > ck).or(found.first()) };
    if let Some((si, p, _)) = next.cloned() {
        a.switch_sheet(si);
        a.select(p, false);
        a.reveal(p);
    }
    a.find.message.clear();
}

pub fn replace(a: &mut App, all: bool) -> usize {
    let found = find_matches(a);
    let targets: Vec<(usize, Pos)> = if all {
        found.iter().map(|x| (x.0, x.1)).collect()
    } else {
        let cur = (a.cur, a.sel().active);
        if found.iter().any(|x| (x.0, x.1) == cur) { vec![cur] } else { vec![] }
    };
    if targets.is_empty() {
        if !all {
            find_next(a, false);
        }
        return 0;
    }
    let what = a.find.what.clone();
    let with = a.find.with.clone();
    let case = a.find.match_case;
    let n = targets.len();
    a.act(|a| {
        for (si, p) in &targets {
            let old = ops::edit_text(&a.wb, *si, p.row, p.col);
            let new = if case {
                old.replace(&what, &with)
            } else {
                let lower = old.to_lowercase();
                let wl = what.to_lowercase();
                let mut out = String::new();
                let mut i = 0;
                while let Some(k) = lower[i..].find(&wl) {
                    out.push_str(&old[i..i + k]);
                    out.push_str(&with);
                    i += k + wl.len();
                }
                out.push_str(&old[i..]);
                out
            };
            let _ = ops::set_input(&mut a.wb, *si, p.row, p.col, &new);
        }
    });
    if !all {
        find_next(a, false);
    }
    n
}

/// Name box and Go To: jumps to a reference or a defined name. Unknown names get defined for the selection.
pub fn go_to(a: &mut App, text: &str, define: bool) {
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    let (sheet, body) = match t.rsplit_once('!') {
        Some((s, b)) => (a.wb.sheet_index(s.trim_matches('\'')), b),
        None => (Some(a.cur), t),
    };
    if let (Some(si), Some(rc)) = (sheet, parse_rect(body)) {
        a.switch_sheet(si);
        let p = Pos::new(rc.r1, rc.c1);
        *a.sel_mut() = crate::app::Sel { ranges: vec![rc], active: p, anchor: p };
        a.reveal(p);
        return;
    }
    if let Some(n) = a.wb.names.iter().find(|n| n.name.eq_ignore_ascii_case(t)).cloned() {
        let e = n.expr.trim_start_matches('=').to_string();
        go_to(a, &e, false);
        return;
    }
    if let Some(tb) = a.wb.sheets.iter().enumerate().find_map(|(i, s)| s.tables.iter().find(|x| x.name.eq_ignore_ascii_case(t)).map(|x| (i, x.range))) {
        a.switch_sheet(tb.0);
        a.sel_mut().ranges = vec![tb.1];
        a.sel_mut().active = Pos::new(tb.1.r1, tb.1.c1);
        return;
    }
    let valid = t.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    if define && valid {
        let r = a.sel().rect();
        let expr = format!("={}!{}", crate::formula::quote_sheet(&a.sheet().name), r.a1_abs());
        let name = t.to_string();
        a.act(|a| a.wb.names.push(DefinedName { name, expr, comment: String::new() }));
    } else {
        a.alert = Some(("Go To".into(), "Reference isn't valid.".into()));
    }
}

pub fn insert_function_text(a: &mut App, name: &str) {
    let is_fn = crate::funcs::registry().map.contains_key(name);
    if let Some(e) = a.edit.as_mut() {
        let mut chars: Vec<char> = e.text.chars().collect();
        let ins = if is_fn { format!("{name}(") } else { name.to_string() };
        let at = e.caret.min(chars.len());
        chars.splice(at..at, ins.chars());
        e.text = chars.into_iter().collect();
        e.caret = at + ins.chars().count();
        e.set_caret = Some(e.caret);
        e.focus = true;
        return;
    }
    a.start_edit(Some(if is_fn { format!("={name}(") } else { format!("={name}") }), false);
}

pub fn names_from_selection(a: &mut App) {
    let r = a.sel().rect();
    if r.rows() < 2 {
        a.alert = Some(("Create Names".into(), "Select a range with labels in its top row first.".into()));
        return;
    }
    let sheet = crate::formula::quote_sheet(&a.sheet().name);
    let mut new = vec![];
    for c in r.c1..=r.c2 {
        let label = ops::text_of(&a.wb, &a.calc, a.cur, r.r1, c);
        let name: String = label.trim().chars().map(|ch| if ch.is_alphanumeric() { ch } else { '_' }).collect();
        if name.is_empty() || name.chars().next().is_some_and(|x| x.is_ascii_digit()) {
            continue;
        }
        let rng = Rect::new(r.r1 + 1, c, r.r2, c);
        new.push(DefinedName { name, expr: format!("={sheet}!{}", rng.a1_abs()), comment: String::new() });
    }
    a.act(|a| {
        for n in new {
            a.wb.names.retain(|x| !x.name.eq_ignore_ascii_case(&n.name));
            a.wb.names.push(n);
        }
    });
}

pub fn trace_precedents(a: &mut App) {
    let p = a.sel().active;
    let text = ops::edit_text(&a.wb, a.cur, p.row, p.col);
    if !text.starts_with('=') {
        a.status = "This cell has no formula.".into();
        return;
    }
    let mut ranges = vec![];
    for (_, _, t) in crate::formula::find_refs(&text) {
        if !t.contains('!') {
            if let Some(r) = parse_rect(&t) {
                ranges.push(r);
            }
        }
    }
    if ranges.is_empty() {
        a.status = "The formula doesn't refer to cells on this sheet.".into();
        return;
    }
    ranges.push(Rect::cell(p));
    *a.sel_mut() = crate::app::Sel { ranges, active: p, anchor: p };
}

pub fn next_error(a: &mut App) {
    let cur = a.sel().active;
    let mut errs: Vec<Pos> = a.calc.vals.iter().filter(|(k, v)| k.0 == a.cur as u32 && v.is_err()).map(|(k, _)| Pos::new(k.1, k.2)).collect();
    errs.sort();
    match errs.iter().find(|p| **p > cur).or(errs.first()) {
        Some(p) => {
            let p = *p;
            a.select(p, false);
            a.reveal(p);
        }
        None => a.alert = Some(("Error Checking".into(), "The error check for the entire sheet is complete.".into())),
    }
}

pub fn import_text(a: &mut App) {
    let Some(p) = rfd::FileDialog::new().add_filter("Text files", &["csv", "tsv", "txt"]).pick_file() else { return };
    let Ok(data) = std::fs::read(&p) else { return };
    let name = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "Data".into());
    let wb2 = crate::ods::load_delimited(&String::from_utf8_lossy(&data), &name);
    a.act(|a| {
        let mut sh = (*wb2.sheets[0]).clone();
        let mut nm: String = name.chars().filter(|c| !"\\/?*[]:".contains(*c)).take(28).collect();
        if a.wb.sheet_index(&nm).is_some() {
            nm = a.wb.unique_sheet_name(&format!("{nm} "));
        }
        sh.name = nm;
        // Styles in the imported sheet refer to the other workbook's table.
        for row in sh.rows.values_mut() {
            for cell in Arc::make_mut(row).values_mut() {
                if cell.style != 0 {
                    let st = (**wb2.styles.get(cell.style)).clone();
                    cell.style = a.wb.styles.intern(st);
                }
            }
        }
        a.wb.sheets.push(Arc::new(sh));
        a.cur = a.wb.sheets.len() - 1;
    });
}

pub fn refresh_all(a: &mut App) {
    a.act(|a| {
        a.calc = eval::recalc(&a.wb);
        for si in 0..a.wb.sheets.len() {
            let defs = a.wb.sheets[si].pivots.clone();
            for d in defs {
                let calc = eval::recalc(&a.wb);
                ops::build_pivot(&mut a.wb, &calc, &d);
            }
        }
    });
}

pub fn group_rows(a: &mut App, group: bool) {
    let r = a.sel().rect();
    let rows = !r.whole_cols();
    a.act(|a| {
        let sh = a.wb.sheet_mut(a.cur);
        if rows {
            for k in r.r1..=r.r2.min(r.r1 + 10_000) {
                let e = sh.outline_rows.entry(k).or_insert(0);
                *e = if group { (*e + 1).min(7) } else { e.saturating_sub(1) };
            }
            sh.outline_rows.retain(|_, v| *v > 0);
        } else {
            for k in r.c1..=r.c2 {
                let e = sh.outline_cols.entry(k).or_insert(0);
                *e = if group { (*e + 1).min(7) } else { e.saturating_sub(1) };
            }
            sh.outline_cols.retain(|_, v| *v > 0);
        }
    });
    a.status = if group { "Grouped. Use Hide/Unhide to collapse the group." } else { "Ungrouped." }.into();
}

pub fn unprotect(a: &mut App) {
    let pw = a.sheet().protection.as_ref().map(|p| p.password.clone()).unwrap_or_default();
    if pw.is_empty() {
        a.act(|a| a.wb.sheet_mut(a.cur).protection = None);
    } else {
        a.dialog = Some(Dialog::Unprotect(String::new()));
    }
}

pub fn zoom_to_selection(a: &mut App) {
    let r = a.sel().rect();
    let Some(gr) = crate::grid_input::last_grid_rect() else { return };
    let sh = a.sheet();
    let w: f32 = (r.c1..=r.c2.min(r.c1 + 200)).map(|c| sh.col_px(c)).sum::<f32>() + 40.0;
    let h: f32 = (r.r1..=r.r2.min(r.r1 + 500)).map(|k| sh.row_px(k)).sum::<f32>() + 24.0;
    let z = (gr.width() / w).min(gr.height() / h).clamp(0.1, 4.0);
    a.wb.sheet_mut(a.cur).zoom = (z * 100.0).floor() / 100.0;
    a.views[a.cur] = (r.r1, r.c1);
}

pub fn table_totals(a: &mut App, ti: usize, on: bool) {
    a.act(|a| {
        let t = a.sheet().tables[ti].clone();
        let sh = a.wb.sheet_mut(a.cur);
        let tt = &mut sh.tables[ti];
        tt.totals = on;
        if on {
            tt.range.r2 += 1;
            let row = tt.range.r2;
            let data = Rect::new(t.range.r1 + t.header as u32, t.range.c1, t.range.r2, t.range.c2);
            let _ = ops::set_input(&mut a.wb, a.cur, row, t.range.c1, "Total");
            let last = t.range.c2;
            let _ = ops::set_input(&mut a.wb, a.cur, row, last, &format!("=SUBTOTAL(109,{})", Rect::new(data.r1, last, data.r2, last).a1()));
        } else {
            let row = tt.range.r2;
            tt.range.r2 -= 1;
            ops::clear_rect(&mut a.wb, a.cur, Rect::new(row, t.range.c1, row, t.range.c2));
        }
    });
}

pub fn table_swatch(p: &egui::Painter, r: R, style: u8, hover: bool) {
    let (head, band) = crate::grid::table_colors(style);
    let rows = 5;
    let h = r.height() / rows as f32;
    for k in 0..rows {
        let rr = R::from_min_size(r.min + vec2(0.0, k as f32 * h), vec2(r.width(), h));
        let c = if k == 0 { head } else if k % 2 == 1 { band } else { 0xFFFFFF };
        p.rect_filled(rr, 0.0, theme::rgb(c));
    }
    p.rect_stroke(r, 0.0, egui::Stroke::new(if hover { 2.0 } else { 1.0 }, if hover { Color32::from_rgb(0xF2, 0x94, 0x36) } else { theme::rgb(head) }), egui::StrokeKind::Outside);
}

type StyleFn = fn(&mut Style);

pub fn cell_style_groups() -> Vec<(&'static str, Vec<(&'static str, StyleFn)>)> {
    vec![
        (
            "Good, Bad and Neutral",
            vec![
                ("Normal", |_s: &mut Style| {}),
                ("Bad", |s: &mut Style| {
                    s.fill = Some(0xFFC7CE);
                    s.color = Some(0x9C0006);
                }),
                ("Good", |s: &mut Style| {
                    s.fill = Some(0xC6EFCE);
                    s.color = Some(0x006100);
                }),
                ("Neutral", |s: &mut Style| {
                    s.fill = Some(0xFFEB9C);
                    s.color = Some(0x9C5700);
                }),
            ],
        ),
        (
            "Data and Model",
            vec![
                ("Calculation", |s: &mut Style| {
                    s.fill = Some(0xF2F2F2);
                    s.color = Some(0xFA7D00);
                    s.bold = true;
                    s.borders = [Some(Border { style: LineStyle::Thin, color: 0x7F7F7F }); 4];
                }),
                ("Check Cell", |s: &mut Style| {
                    s.fill = Some(0xA5A5A5);
                    s.color = Some(0xFFFFFF);
                    s.bold = true;
                    s.borders = [Some(Border { style: LineStyle::Double, color: 0x3F3F3F }); 4];
                }),
                ("Explanatory", |s: &mut Style| {
                    s.italic = true;
                    s.color = Some(0x7F7F7F);
                }),
                ("Input", |s: &mut Style| {
                    s.fill = Some(0xFFCC99);
                    s.color = Some(0x3F3F76);
                    s.borders = [Some(Border { style: LineStyle::Thin, color: 0x7F7F7F }); 4];
                }),
                ("Linked Cell", |s: &mut Style| {
                    s.color = Some(0xFA7D00);
                    s.borders[B_BOTTOM] = Some(Border { style: LineStyle::Double, color: 0xFF8001 });
                }),
                ("Note", |s: &mut Style| {
                    s.fill = Some(0xFFFFCC);
                    s.borders = [Some(Border { style: LineStyle::Thin, color: 0xB2B2B2 }); 4];
                }),
                ("Output", |s: &mut Style| {
                    s.fill = Some(0xF2F2F2);
                    s.color = Some(0x3F3F3F);
                    s.bold = true;
                    s.borders = [Some(Border { style: LineStyle::Thin, color: 0x3F3F3F }); 4];
                }),
                ("Warning Text", |s: &mut Style| s.color = Some(0xFF0000)),
            ],
        ),
        (
            "Titles and Headings",
            vec![
                ("Heading 1", |s: &mut Style| {
                    s.bold = true;
                    s.size10 = 150;
                    s.color = Some(0x44546A);
                    s.borders[B_BOTTOM] = Some(Border { style: LineStyle::Thick, color: 0x4472C4 });
                }),
                ("Heading 2", |s: &mut Style| {
                    s.bold = true;
                    s.size10 = 130;
                    s.color = Some(0x44546A);
                    s.borders[B_BOTTOM] = Some(Border { style: LineStyle::Thick, color: 0xA2B8E1 });
                }),
                ("Heading 3", |s: &mut Style| {
                    s.bold = true;
                    s.color = Some(0x44546A);
                    s.borders[B_BOTTOM] = Some(Border { style: LineStyle::Medium, color: 0x8EA9DB });
                }),
                ("Title", |s: &mut Style| {
                    s.size10 = 180;
                    s.color = Some(0x44546A);
                    s.font = "Calibri Light".into();
                }),
                ("Total", |s: &mut Style| {
                    s.bold = true;
                    s.borders[B_TOP] = Some(Border { style: LineStyle::Thin, color: 0x4472C4 });
                    s.borders[B_BOTTOM] = Some(Border { style: LineStyle::Double, color: 0x4472C4 });
                }),
            ],
        ),
        (
            "Themed Cell Styles",
            vec![
                ("Accent1", |s: &mut Style| {
                    s.fill = Some(0x4472C4);
                    s.color = Some(0xFFFFFF);
                }),
                ("Accent2", |s: &mut Style| {
                    s.fill = Some(0xED7D31);
                    s.color = Some(0xFFFFFF);
                }),
                ("Accent3", |s: &mut Style| {
                    s.fill = Some(0xA5A5A5);
                    s.color = Some(0xFFFFFF);
                }),
                ("Accent4", |s: &mut Style| {
                    s.fill = Some(0xFFC000);
                    s.color = Some(0xFFFFFF);
                }),
                ("20% - Accent1", |s: &mut Style| s.fill = Some(0xD9E1F2)),
                ("20% - Accent2", |s: &mut Style| s.fill = Some(0xFCE4D6)),
                ("40% - Accent5", |s: &mut Style| s.fill = Some(0xBDD7EE)),
                ("40% - Accent6", |s: &mut Style| s.fill = Some(0xC6E0B4)),
            ],
        ),
        (
            "Number Format",
            vec![
                ("Comma", |s: &mut Style| s.num_fmt = "_(* #,##0.00_);_(* \\(#,##0.00\\);_(* \"-\"??_);_(@_)".into()),
                ("Comma [0]", |s: &mut Style| s.num_fmt = "_(* #,##0_);_(* \\(#,##0\\);_(* \"-\"_);_(@_)".into()),
                ("Currency", |s: &mut Style| s.num_fmt = "_(\"$\"* #,##0.00_);_(\"$\"* \\(#,##0.00\\);_(\"$\"* \"-\"??_);_(@_)".into()),
                ("Percent", |s: &mut Style| s.num_fmt = "0%".into()),
            ],
        ),
    ]
}
