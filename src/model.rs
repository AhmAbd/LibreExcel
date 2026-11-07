//! The workbook data model: cells, styles, sheets and everything attached to them.

use crate::formula::{Expr, RefExpr};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;
pub const DEFAULT_COL_W: f32 = 64.0;
pub const DEFAULT_ROW_H: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default)]
pub struct Pos {
    pub row: u32,
    pub col: u32,
}

impl Pos {
    pub fn new(row: u32, col: u32) -> Self {
        Pos { row, col }
    }
}

/// An inclusive rectangle of cells.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Rect {
    pub r1: u32,
    pub c1: u32,
    pub r2: u32,
    pub c2: u32,
}

impl Rect {
    pub fn new(r1: u32, c1: u32, r2: u32, c2: u32) -> Self {
        Rect { r1: r1.min(r2), c1: c1.min(c2), r2: r1.max(r2), c2: c1.max(c2) }
    }
    pub fn cell(p: Pos) -> Self {
        Rect { r1: p.row, c1: p.col, r2: p.row, c2: p.col }
    }
    pub fn from_pos(a: Pos, b: Pos) -> Self {
        Rect::new(a.row, a.col, b.row, b.col)
    }
    pub fn contains(&self, p: Pos) -> bool {
        p.row >= self.r1 && p.row <= self.r2 && p.col >= self.c1 && p.col <= self.c2
    }
    pub fn contains_rect(&self, o: &Rect) -> bool {
        o.r1 >= self.r1 && o.r2 <= self.r2 && o.c1 >= self.c1 && o.c2 <= self.c2
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.r1 <= o.r2 && o.r1 <= self.r2 && self.c1 <= o.c2 && o.c1 <= self.c2
    }
    pub fn intersection(&self, o: &Rect) -> Option<Rect> {
        if self.intersects(o) {
            Some(Rect {
                r1: self.r1.max(o.r1),
                c1: self.c1.max(o.c1),
                r2: self.r2.min(o.r2),
                c2: self.c2.min(o.c2),
            })
        } else {
            None
        }
    }
    pub fn union(&self, o: &Rect) -> Rect {
        Rect { r1: self.r1.min(o.r1), c1: self.c1.min(o.c1), r2: self.r2.max(o.r2), c2: self.c2.max(o.c2) }
    }
    pub fn rows(&self) -> u32 {
        self.r2 - self.r1 + 1
    }
    pub fn cols(&self) -> u32 {
        self.c2 - self.c1 + 1
    }
    pub fn is_single(&self) -> bool {
        self.r1 == self.r2 && self.c1 == self.c2
    }
    pub fn whole_cols(&self) -> bool {
        self.r1 == 0 && self.r2 == MAX_ROWS - 1
    }
    pub fn whole_rows(&self) -> bool {
        self.c1 == 0 && self.c2 == MAX_COLS - 1
    }
    pub fn a1(&self) -> String {
        if self.whole_cols() {
            format!("{}:{}", col_name(self.c1), col_name(self.c2))
        } else if self.whole_rows() {
            format!("{}:{}", self.r1 + 1, self.r2 + 1)
        } else if self.is_single() {
            cell_name(self.r1, self.c1)
        } else {
            format!("{}:{}", cell_name(self.r1, self.c1), cell_name(self.r2, self.c2))
        }
    }
    pub fn a1_abs(&self) -> String {
        if self.is_single() {
            format!("${}${}", col_name(self.c1), self.r1 + 1)
        } else {
            format!("${}${}:${}${}", col_name(self.c1), self.r1 + 1, col_name(self.c2), self.r2 + 1)
        }
    }
}

pub fn col_name(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

pub fn cell_name(r: u32, c: u32) -> String {
    format!("{}{}", col_name(c), r + 1)
}

pub fn parse_col(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    let mut v: u32 = 0;
    for ch in s.bytes() {
        let ch = ch.to_ascii_uppercase();
        if !ch.is_ascii_uppercase() {
            return None;
        }
        v = v * 26 + (ch - b'A') as u32 + 1;
    }
    if v == 0 || v > MAX_COLS {
        None
    } else {
        Some(v - 1)
    }
}

/// Parses "B7" or "$B$7" into a position.
pub fn parse_cell(s: &str) -> Option<Pos> {
    let s = s.trim().replace('$', "");
    let split = s.find(|c: char| c.is_ascii_digit())?;
    let (c, r) = s.split_at(split);
    let col = parse_col(c)?;
    let row: u32 = r.parse().ok()?;
    if row == 0 || row > MAX_ROWS {
        return None;
    }
    Some(Pos::new(row - 1, col))
}

/// Parses "A1", "A1:B5", "C:E" or "2:9".
pub fn parse_rect(s: &str) -> Option<Rect> {
    let s = s.trim().replace('$', "");
    if let Some((a, b)) = s.split_once(':') {
        if let (Some(x), Some(y)) = (parse_cell(a), parse_cell(b)) {
            return Some(Rect::from_pos(x, y));
        }
        if let (Some(x), Some(y)) = (parse_col(a), parse_col(b)) {
            return Some(Rect::new(0, x, MAX_ROWS - 1, y));
        }
        if let (Ok(x), Ok(y)) = (a.parse::<u32>(), b.parse::<u32>()) {
            if x >= 1 && y >= 1 && x <= MAX_ROWS && y <= MAX_ROWS {
                return Some(Rect::new(x - 1, 0, y - 1, MAX_COLS - 1));
            }
        }
        None
    } else {
        parse_cell(&s).map(Rect::cell)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum XlErr {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    NA,
    Spill,
    Calc,
}

impl XlErr {
    pub fn text(self) -> &'static str {
        match self {
            XlErr::Null => "#NULL!",
            XlErr::Div0 => "#DIV/0!",
            XlErr::Value => "#VALUE!",
            XlErr::Ref => "#REF!",
            XlErr::Name => "#NAME?",
            XlErr::Num => "#NUM!",
            XlErr::NA => "#N/A",
            XlErr::Spill => "#SPILL!",
            XlErr::Calc => "#CALC!",
        }
    }
    pub fn parse(s: &str) -> Option<XlErr> {
        Some(match s.to_ascii_uppercase().as_str() {
            "#NULL!" => XlErr::Null,
            "#DIV/0!" => XlErr::Div0,
            "#VALUE!" => XlErr::Value,
            "#REF!" => XlErr::Ref,
            "#NAME?" => XlErr::Name,
            "#NUM!" => XlErr::Num,
            "#N/A" => XlErr::NA,
            "#SPILL!" => XlErr::Spill,
            "#CALC!" => XlErr::Calc,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Content {
    #[default]
    Empty,
    Num(f64),
    Text(Arc<str>),
    Bool(bool),
    Err(XlErr),
    Formula(Arc<Expr>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub v: Content,
    pub style: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum HAlign {
    #[default]
    General,
    Left,
    Center,
    Right,
    Fill,
    Justify,
    CenterAcross,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum VAlign {
    Top,
    Center,
    #[default]
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LineStyle {
    Thin,
    Medium,
    Thick,
    Dashed,
    Dotted,
    Double,
    Hair,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Border {
    pub style: LineStyle,
    pub color: u32,
}

pub const B_LEFT: usize = 0;
pub const B_RIGHT: usize = 1;
pub const B_TOP: usize = 2;
pub const B_BOTTOM: usize = 3;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Style {
    pub font: Arc<str>,
    pub size10: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: u8,
    pub strike: bool,
    pub color: Option<u32>,
    pub fill: Option<u32>,
    pub h: HAlign,
    pub v: VAlign,
    pub wrap: bool,
    pub shrink: bool,
    pub indent: u8,
    pub rotation: i16,
    pub num_fmt: Arc<str>,
    pub borders: [Option<Border>; 4],
    pub locked: bool,
    pub hide_formula: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            font: "Calibri".into(),
            size10: 110,
            bold: false,
            italic: false,
            underline: 0,
            strike: false,
            color: None,
            fill: None,
            h: HAlign::General,
            v: VAlign::Bottom,
            wrap: false,
            shrink: false,
            indent: 0,
            rotation: 0,
            num_fmt: "General".into(),
            borders: [None; 4],
            locked: true,
            hide_formula: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StyleTable {
    pub list: Vec<Arc<Style>>,
    map: HashMap<Arc<Style>, u32>,
}

impl Default for StyleTable {
    fn default() -> Self {
        let d = Arc::new(Style::default());
        let mut map = HashMap::new();
        map.insert(d.clone(), 0);
        StyleTable { list: vec![d], map }
    }
}

impl StyleTable {
    pub fn get(&self, id: u32) -> &Arc<Style> {
        self.list.get(id as usize).unwrap_or(&self.list[0])
    }
    pub fn intern(&mut self, s: Style) -> u32 {
        if let Some(i) = self.map.get(&s) {
            return *i;
        }
        let a = Arc::new(s);
        let id = self.list.len() as u32;
        self.list.push(a.clone());
        self.map.insert(a, id);
        id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub text: String,
    pub author: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CondRule {
    /// op: ">", "<", ">=", "<=", "=", "<>", "between", "notbetween"; operands are formulas.
    CellIs { op: String, a: String, b: String },
    TextContains { text: String, mode: TextMode },
    Duplicate { unique: bool },
    TopBottom { top: bool, rank: u32, percent: bool },
    AboveAverage { above: bool },
    Blanks { blank: bool },
    Formula { formula: String },
    ColorScale { colors: Vec<u32> },
    DataBar { color: u32 },
    IconSet { set: IconSet },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextMode {
    Contains,
    NotContains,
    Begins,
    Ends,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconSet {
    Arrows3,
    Lights3,
    Flags3,
    Stars3,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CondStyle {
    pub fill: Option<u32>,
    pub color: Option<u32>,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CondFormat {
    pub ranges: Vec<Rect>,
    pub rule: CondRule,
    pub style: CondStyle,
    pub stop_if_true: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValidKind {
    Any,
    Whole,
    Decimal,
    List,
    Date,
    Time,
    TextLength,
    Custom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Validation {
    pub ranges: Vec<Rect>,
    pub kind: ValidKind,
    pub op: String,
    pub a: String,
    pub b: String,
    pub ignore_blank: bool,
    pub in_cell_list: bool,
    pub prompt_title: String,
    pub prompt: String,
    pub error_title: String,
    pub error: String,
    /// 0 stop, 1 warning, 2 information.
    pub error_style: u8,
}

impl Default for Validation {
    fn default() -> Self {
        Validation {
            ranges: vec![],
            kind: ValidKind::Any,
            op: "between".into(),
            a: String::new(),
            b: String::new(),
            ignore_blank: true,
            in_cell_list: true,
            prompt_title: String::new(),
            prompt: String::new(),
            error_title: String::new(),
            error: String::new(),
            error_style: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartKind {
    Column,
    ColumnStacked,
    Bar,
    BarStacked,
    Line,
    LineMarkers,
    Area,
    Pie,
    Doughnut,
    Scatter,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Series {
    pub name: String,
    pub name_ref: Option<String>,
    pub cats: Option<String>,
    pub vals: String,
    pub color: Option<u32>,
}

/// A floating chart. Its anchor is in unzoomed pixels from the sheet's top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub kind: ChartKind,
    pub title: String,
    pub series: Vec<Series>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub legend: bool,
    pub x_title: String,
    pub y_title: String,
    pub data_labels: bool,
    pub gridlines: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableDef {
    pub name: String,
    pub range: Rect,
    pub style: u8,
    pub header: bool,
    pub totals: bool,
    pub banded: bool,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ColFilter {
    /// Displayed values that stay visible. None means no value filter.
    pub allowed: Option<BTreeSet<String>>,
    /// Optional comparison ("greater", "less", "contains", ...) with its operand.
    pub custom: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoFilter {
    pub range: Rect,
    pub cols: BTreeMap<u32, ColFilter>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageSetup {
    pub landscape: bool,
    /// 0 Letter, 1 A4, 2 Legal, 3 A3.
    pub paper: u8,
    /// left, right, top, bottom in inches.
    pub margins: [f32; 4],
    pub print_area: Option<Rect>,
    pub scale: u32,
    pub fit_width: bool,
    pub gridlines: bool,
    pub headings: bool,
    pub header: String,
    pub footer: String,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            landscape: false,
            paper: 0,
            margins: [0.7, 0.7, 0.75, 0.75],
            print_area: None,
            scale: 100,
            fit_width: false,
            gridlines: false,
            headings: false,
            header: String::new(),
            footer: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Protection {
    pub password: String,
    pub select_locked: bool,
}

/// A saved pivot definition, so the output can be refreshed.
#[derive(Clone, Debug, PartialEq)]
pub struct PivotDef {
    pub source_sheet: String,
    pub source: Rect,
    pub target: Pos,
    pub rows: Vec<u32>,
    pub cols: Vec<u32>,
    /// (source column, aggregate name).
    pub values: Vec<(u32, String)>,
    pub filters: Vec<(u32, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sparkline {
    pub kind: u8,
    pub source: String,
    pub color: u32,
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub name: String,
    pub rows: BTreeMap<u32, Arc<BTreeMap<u32, Cell>>>,
    pub col_w: HashMap<u32, f32>,
    pub row_h: HashMap<u32, f32>,
    /// Rows whose height the person set by hand; those no longer auto-fit.
    pub custom_h: BTreeSet<u32>,
    pub hidden_cols: BTreeSet<u32>,
    pub hidden_rows: BTreeSet<u32>,
    pub filtered_rows: BTreeSet<u32>,
    pub col_style: HashMap<u32, u32>,
    pub row_style: HashMap<u32, u32>,
    pub merges: Vec<Rect>,
    pub freeze: (u32, u32),
    pub gridlines: bool,
    pub headings: bool,
    pub tab_color: Option<u32>,
    pub hidden: bool,
    pub cond: Vec<CondFormat>,
    pub valid: Vec<Validation>,
    pub notes: BTreeMap<Pos, Note>,
    pub links: BTreeMap<Pos, String>,
    pub charts: Vec<Chart>,
    pub tables: Vec<TableDef>,
    pub filter: Option<AutoFilter>,
    pub page: PageSetup,
    pub protection: Option<Protection>,
    pub pivots: Vec<PivotDef>,
    pub sparks: BTreeMap<Pos, Sparkline>,
    pub outline_rows: BTreeMap<u32, u8>,
    pub outline_cols: BTreeMap<u32, u8>,
    pub zoom: f32,
}

impl Sheet {
    pub fn new(name: &str) -> Self {
        Sheet {
            name: name.to_string(),
            rows: BTreeMap::new(),
            col_w: HashMap::new(),
            row_h: HashMap::new(),
            custom_h: BTreeSet::new(),
            hidden_cols: BTreeSet::new(),
            hidden_rows: BTreeSet::new(),
            filtered_rows: BTreeSet::new(),
            col_style: HashMap::new(),
            row_style: HashMap::new(),
            merges: vec![],
            freeze: (0, 0),
            gridlines: true,
            headings: true,
            tab_color: None,
            hidden: false,
            cond: vec![],
            valid: vec![],
            notes: BTreeMap::new(),
            links: BTreeMap::new(),
            charts: vec![],
            tables: vec![],
            filter: None,
            page: PageSetup::default(),
            protection: None,
            pivots: vec![],
            sparks: BTreeMap::new(),
            outline_rows: BTreeMap::new(),
            outline_cols: BTreeMap::new(),
            zoom: 1.0,
        }
    }

    pub fn get(&self, r: u32, c: u32) -> Option<&Cell> {
        self.rows.get(&r).and_then(|m| m.get(&c))
    }

    pub fn cell_mut(&mut self, r: u32, c: u32) -> &mut Cell {
        let row = self.rows.entry(r).or_default();
        Arc::make_mut(row).entry(c).or_default()
    }

    pub fn remove(&mut self, r: u32, c: u32) {
        if let Some(row) = self.rows.get_mut(&r) {
            if row.contains_key(&c) {
                Arc::make_mut(row).remove(&c);
            }
            if row.is_empty() {
                self.rows.remove(&r);
            }
        }
    }

    pub fn set_content(&mut self, r: u32, c: u32, v: Content) {
        let st = self.base_style(r, c);
        if let Some(cell) = self.get(r, c) {
            if cell.style == 0 && matches!(v, Content::Empty) {
                self.remove(r, c);
                return;
            }
        } else if matches!(v, Content::Empty) && st == 0 {
            return;
        }
        let exists = self.get(r, c).is_some();
        let cell = self.cell_mut(r, c);
        if !exists {
            cell.style = st;
        }
        cell.v = v;
    }

    /// Style of a cell ignoring cell-level formatting: row first, then column.
    pub fn base_style(&self, r: u32, c: u32) -> u32 {
        self.row_style.get(&r).or_else(|| self.col_style.get(&c)).copied().unwrap_or(0)
    }

    pub fn style_id(&self, r: u32, c: u32) -> u32 {
        self.get(r, c).map(|x| x.style).unwrap_or_else(|| self.base_style(r, c))
    }

    pub fn col_px(&self, c: u32) -> f32 {
        if self.hidden_cols.contains(&c) {
            0.0
        } else {
            self.col_w.get(&c).copied().unwrap_or(DEFAULT_COL_W)
        }
    }

    pub fn row_px(&self, r: u32) -> f32 {
        if self.hidden_rows.contains(&r) || self.filtered_rows.contains(&r) {
            0.0
        } else {
            self.row_h.get(&r).copied().unwrap_or(DEFAULT_ROW_H)
        }
    }

    pub fn row_hidden(&self, r: u32) -> bool {
        self.hidden_rows.contains(&r) || self.filtered_rows.contains(&r)
    }

    /// Bottom-right corner of everything with content. None if the sheet is empty.
    pub fn used_rect(&self) -> Option<Rect> {
        let mut r: Option<Rect> = None;
        for (&row, m) in &self.rows {
            for (&col, c) in m.iter() {
                if c.v != Content::Empty || c.style != 0 {
                    let x = Rect::new(row, col, row, col);
                    r = Some(r.map_or(x, |o| o.union(&x)));
                }
            }
        }
        r
    }

    pub fn used_content_rect(&self) -> Option<Rect> {
        let mut r: Option<Rect> = None;
        for (&row, m) in &self.rows {
            let mut any = false;
            let (mut lo, mut hi) = (u32::MAX, 0);
            for (&col, c) in m.iter() {
                if c.v != Content::Empty {
                    any = true;
                    lo = lo.min(col);
                    hi = hi.max(col);
                }
            }
            if any {
                let x = Rect::new(row, lo, row, hi);
                r = Some(r.map_or(x, |o| o.union(&x)));
            }
        }
        r
    }

    pub fn merge_at(&self, p: Pos) -> Option<Rect> {
        self.merges.iter().find(|m| m.contains(p)).copied()
    }

    pub fn is_empty_at(&self, r: u32, c: u32) -> bool {
        self.get(r, c).map(|x| x.v == Content::Empty).unwrap_or(true)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DefinedName {
    pub name: String,
    pub expr: String,
    pub comment: String,
}

#[derive(Clone, Debug)]
pub struct Workbook {
    pub sheets: Vec<Arc<Sheet>>,
    pub styles: StyleTable,
    pub names: Vec<DefinedName>,
    pub protected: bool,
    pub author: String,
}

impl Default for Workbook {
    fn default() -> Self {
        Workbook {
            sheets: vec![Arc::new(Sheet::new("Sheet1"))],
            styles: StyleTable::default(),
            names: vec![],
            protected: false,
            author: "User".into(),
        }
    }
}

impl Workbook {
    pub fn sheet_index(&self, name: &str) -> Option<usize> {
        self.sheets.iter().position(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn sheet_mut(&mut self, i: usize) -> &mut Sheet {
        Arc::make_mut(&mut self.sheets[i])
    }

    pub fn unique_sheet_name(&self, stem: &str) -> String {
        let mut i = 1;
        loop {
            let n = format!("{stem}{i}");
            if self.sheet_index(&n).is_none() {
                return n;
            }
            i += 1;
        }
    }

    /// Calls `f` for every formula in the workbook so it can rewrite the expression.
    pub fn for_each_formula(&mut self, mut f: impl FnMut(usize, Pos, &mut Expr)) {
        for si in 0..self.sheets.len() {
            let keys: Vec<(u32, Vec<u32>)> = self.sheets[si]
                .rows
                .iter()
                .filter_map(|(r, m)| {
                    let cols: Vec<u32> = m
                        .iter()
                        .filter(|(_, c)| matches!(c.v, Content::Formula(_)))
                        .map(|(c, _)| *c)
                        .collect();
                    if cols.is_empty() {
                        None
                    } else {
                        Some((*r, cols))
                    }
                })
                .collect();
            if keys.is_empty() {
                continue;
            }
            let sh = self.sheet_mut(si);
            for (r, cols) in keys {
                for c in cols {
                    let cell = sh.cell_mut(r, c);
                    if let Content::Formula(e) = &mut cell.v {
                        let mut ex = (**e).clone();
                        f(si, Pos::new(r, c), &mut ex);
                        *e = Arc::new(ex);
                    }
                }
            }
        }
    }
}

/// True if an unqualified or qualified reference points at `sheet`.
pub fn ref_targets(r: &RefExpr, sheet: &str, own_sheet: &str) -> bool {
    match &r.sheet {
        Some(s) => s.eq_ignore_ascii_case(sheet),
        None => own_sheet.eq_ignore_ascii_case(sheet),
    }
}
