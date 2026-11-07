//! Reads .xlsx workbooks.

use crate::formula::{self, Expr};
use crate::model::*;
use crate::xml::{self, Node};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

type Zip = zip::ZipArchive<Cursor<Vec<u8>>>;

fn part(z: &mut Zip, name: &str) -> Option<String> {
    let mut f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s.trim_start_matches('\u{feff}').to_string())
}

fn tree(z: &mut Zip, name: &str) -> Option<Node> {
    part(z, name).and_then(|s| xml::parse(&s).ok())
}

const DEFAULT_THEME: [u32; 12] = [
    0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47, 0x0563C1,
    0x954F72,
];

const INDEXED: [u32; 64] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x000000, 0xFFFFFF, 0xFF0000,
    0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080,
    0xC0C0C0, 0x808080, 0x9999FF, 0x993366, 0xFFFFCC, 0xCCFFFF, 0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, 0x000080,
    0xFF00FF, 0xFFFF00, 0x00FFFF, 0x800080, 0x800000, 0x008080, 0x0000FF, 0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99,
    0x99CCFF, 0xFF99CC, 0xCC99FF, 0xFFCC99, 0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600, 0x666699,
    0x969696, 0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399, 0x333333,
];

fn tint(c: u32, t: f64) -> u32 {
    let f = |x: u32| -> u32 {
        let x = x as f64;
        let y = if t < 0.0 { x * (1.0 + t) } else { x + (255.0 - x) * t };
        y.round().clamp(0.0, 255.0) as u32
    };
    (f(c >> 16 & 255) << 16) | (f(c >> 8 & 255) << 8) | f(c & 255)
}

struct Styles {
    theme: Vec<u32>,
    xf: Vec<u32>,
    dxf: Vec<CondStyle>,
}

fn color(n: Option<&Node>, theme: &[u32]) -> Option<u32> {
    let n = n?;
    if let Some(rgb) = n.attr("rgb") {
        let h = if rgb.len() == 8 { &rgb[2..] } else { rgb };
        return u32::from_str_radix(h, 16).ok();
    }
    if let Some(t) = n.attr_u("theme") {
        let base = theme.get(t as usize).copied()?;
        return Some(match n.attr_f("tint") {
            Some(x) => tint(base, x),
            None => base,
        });
    }
    if let Some(i) = n.attr_u("indexed") {
        return INDEXED.get(i as usize).copied();
    }
    None
}

fn builtin_fmt(id: u32) -> &'static str {
    match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5 => "$#,##0_);($#,##0)",
        6 => "$#,##0_);[Red]($#,##0)",
        7 => "$#,##0.00_);($#,##0.00)",
        8 => "$#,##0.00_);[Red]($#,##0.00)",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "m/d/yyyy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "m/d/yyyy h:mm",
        37 => "#,##0_);(#,##0)",
        38 => "#,##0_);[Red](#,##0)",
        39 => "#,##0.00_);(#,##0.00)",
        40 => "#,##0.00_);[Red](#,##0.00)",
        41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
        42 => "_($* #,##0_);_($* (#,##0);_($* \"-\"_);_(@_)",
        43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
        44 => "_($* #,##0.00_);_($* (#,##0.00);_($* \"-\"??_);_(@_)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => "General",
    }
}

fn line_style(s: &str) -> Option<LineStyle> {
    Some(match s {
        "thin" => LineStyle::Thin,
        "medium" | "mediumDashed" | "mediumDashDot" => LineStyle::Medium,
        "thick" => LineStyle::Thick,
        "dashed" | "dashDot" | "dashDotDot" | "slantDashDot" => LineStyle::Dashed,
        "dotted" => LineStyle::Dotted,
        "double" => LineStyle::Double,
        "hair" => LineStyle::Hair,
        _ => return None,
    })
}

fn read_styles(root: Option<Node>, theme: Vec<u32>, wb: &mut Workbook) -> Styles {
    let mut out = Styles { theme, xf: vec![0], dxf: vec![] };
    let Some(root) = root else { return out };
    let mut fmts: HashMap<u32, String> = HashMap::new();
    if let Some(nf) = root.child("numFmts") {
        for f in nf.children("numFmt") {
            if let (Some(id), Some(code)) = (f.attr_u("numFmtId"), f.attr("formatCode")) {
                fmts.insert(id, code.to_string());
            }
        }
    }
    let fonts: Vec<&Node> = root.child("fonts").map(|x| x.children("font").collect()).unwrap_or_default();
    let fills: Vec<&Node> = root.child("fills").map(|x| x.children("fill").collect()).unwrap_or_default();
    let borders: Vec<&Node> = root.child("borders").map(|x| x.children("border").collect()).unwrap_or_default();
    if let Some(xfs) = root.child("cellXfs") {
        out.xf.clear();
        for x in xfs.children("xf") {
            let mut s = Style::default();
            if let Some(f) = x.attr_u("fontId").and_then(|i| fonts.get(i as usize)) {
                if let Some(n) = f.child("name").and_then(|n| n.attr("val")) {
                    s.font = n.into();
                }
                if let Some(z) = f.child("sz").and_then(|n| n.attr_f("val")) {
                    s.size10 = (z * 10.0).round() as u16;
                }
                let flag = |n: &str| f.child(n).is_some_and(|b| b.attr("val") != Some("0") && b.attr("val") != Some("false"));
                s.bold = flag("b");
                s.italic = flag("i");
                s.strike = flag("strike");
                if let Some(u) = f.child("u") {
                    s.underline = if u.attr("val") == Some("double") { 2 } else { 1 };
                }
                s.color = color(f.child("color"), &out.theme).filter(|c| *c != 0);
            }
            if let Some(f) = x.attr_u("fillId").and_then(|i| fills.get(i as usize)) {
                if let Some(p) = f.child("patternFill") {
                    let ty = p.attr("patternType").unwrap_or("none");
                    if ty != "none" {
                        s.fill = color(p.child("fgColor"), &out.theme);
                    }
                }
            }
            if let Some(b) = x.attr_u("borderId").and_then(|i| borders.get(i as usize)) {
                for (i, nm) in ["left", "right", "top", "bottom"].iter().enumerate() {
                    if let Some(side) = b.child(nm) {
                        if let Some(ls) = side.attr("style").and_then(line_style) {
                            s.borders[i] = Some(Border { style: ls, color: color(side.child("color"), &out.theme).unwrap_or(0) });
                        }
                    }
                }
            }
            if let Some(id) = x.attr_u("numFmtId") {
                let code = fmts.get(&id).map(|x| x.as_str()).unwrap_or(builtin_fmt(id));
                s.num_fmt = code.into();
            }
            if let Some(a) = x.child("alignment") {
                s.h = match a.attr("horizontal") {
                    Some("left") => HAlign::Left,
                    Some("center") => HAlign::Center,
                    Some("right") => HAlign::Right,
                    Some("fill") => HAlign::Fill,
                    Some("justify") => HAlign::Justify,
                    Some("centerContinuous") => HAlign::CenterAcross,
                    _ => HAlign::General,
                };
                s.v = match a.attr("vertical") {
                    Some("top") => VAlign::Top,
                    Some("center") => VAlign::Center,
                    _ => VAlign::Bottom,
                };
                s.wrap = a.attr_on("wrapText");
                s.shrink = a.attr_on("shrinkToFit");
                s.indent = a.attr_u("indent").unwrap_or(0) as u8;
                s.rotation = match a.attr_u("textRotation").unwrap_or(0) {
                    r if r <= 90 => r as i16,
                    r if r <= 180 => -((r - 90) as i16),
                    _ => 0,
                };
            }
            if let Some(p) = x.child("protection") {
                s.locked = p.attr("locked") != Some("0");
                s.hide_formula = p.attr_on("hidden");
            }
            out.xf.push(wb.styles.intern(s));
        }
    }
    if let Some(d) = root.child("dxfs") {
        for x in d.children("dxf") {
            let mut c = CondStyle::default();
            if let Some(f) = x.child("font") {
                c.color = color(f.child("color"), &out.theme);
                c.bold = f.child("b").is_some();
                c.italic = f.child("i").is_some();
            }
            if let Some(p) = x.child("fill").and_then(|f| f.child("patternFill")) {
                c.fill = color(p.child("bgColor"), &out.theme).or(color(p.child("fgColor"), &out.theme));
            }
            out.dxf.push(c);
        }
    }
    out
}

fn sqref(s: &str) -> Vec<Rect> {
    s.split_whitespace().filter_map(parse_rect).collect()
}

fn read_cond(n: &Node, st: &Styles, sh: &mut Sheet) {
    let ranges = sqref(n.attr("sqref").unwrap_or(""));
    for r in n.children("cfRule") {
        let f: Vec<String> = r.children("formula").map(|x| x.all_text()).collect();
        let fa = f.first().cloned().unwrap_or_default();
        let fb = f.get(1).cloned().unwrap_or_default();
        let op = match r.attr("operator").unwrap_or("") {
            "greaterThan" => ">",
            "lessThan" => "<",
            "greaterThanOrEqual" => ">=",
            "lessThanOrEqual" => "<=",
            "equal" => "=",
            "notEqual" => "<>",
            "between" => "between",
            "notBetween" => "notbetween",
            _ => "=",
        };
        let rule = match r.attr("type").unwrap_or("") {
            "cellIs" => CondRule::CellIs { op: op.into(), a: fa, b: fb },
            "expression" => CondRule::Formula { formula: fa },
            "duplicateValues" => CondRule::Duplicate { unique: false },
            "uniqueValues" => CondRule::Duplicate { unique: true },
            "top10" => CondRule::TopBottom {
                top: !r.attr_on("bottom"),
                rank: r.attr_u("rank").unwrap_or(10),
                percent: r.attr_on("percent"),
            },
            "aboveAverage" => CondRule::AboveAverage { above: r.attr("aboveAverage") != Some("0") },
            "containsBlanks" => CondRule::Blanks { blank: true },
            "notContainsBlanks" => CondRule::Blanks { blank: false },
            "containsText" => CondRule::TextContains { text: r.attr("text").unwrap_or("").into(), mode: TextMode::Contains },
            "notContainsText" => CondRule::TextContains { text: r.attr("text").unwrap_or("").into(), mode: TextMode::NotContains },
            "beginsWith" => CondRule::TextContains { text: r.attr("text").unwrap_or("").into(), mode: TextMode::Begins },
            "endsWith" => CondRule::TextContains { text: r.attr("text").unwrap_or("").into(), mode: TextMode::Ends },
            "colorScale" => {
                let colors = r.child("colorScale").map(|c| c.children("color").filter_map(|x| color(Some(x), &st.theme)).collect()).unwrap_or_default();
                CondRule::ColorScale { colors }
            }
            "dataBar" => CondRule::DataBar { color: r.child("dataBar").and_then(|c| color(c.child("color"), &st.theme)).unwrap_or(0x638EC6) },
            "iconSet" => {
                let name = r.child("iconSet").and_then(|c| c.attr("iconSet")).unwrap_or("3Arrows");
                CondRule::IconSet {
                    set: if name.contains("Flags") {
                        IconSet::Flags3
                    } else if name.contains("Traffic") {
                        IconSet::Lights3
                    } else if name.contains("Stars") {
                        IconSet::Stars3
                    } else {
                        IconSet::Arrows3
                    },
                }
            }
            _ => continue,
        };
        let style = r.attr_u("dxfId").and_then(|i| st.dxf.get(i as usize)).cloned().unwrap_or_default();
        sh.cond.push(CondFormat { ranges: ranges.clone(), rule, style, stop_if_true: r.attr_on("stopIfTrue") });
    }
}

fn read_valid(n: &Node, sh: &mut Sheet) {
    let f1 = n.child("formula1").map(|x| x.all_text()).unwrap_or_default();
    let f2 = n.child("formula2").map(|x| x.all_text()).unwrap_or_default();
    let mut v = Validation {
        ranges: sqref(n.attr("sqref").unwrap_or("")),
        kind: match n.attr("type").unwrap_or("") {
            "whole" => ValidKind::Whole,
            "decimal" => ValidKind::Decimal,
            "list" => ValidKind::List,
            "date" => ValidKind::Date,
            "time" => ValidKind::Time,
            "textLength" => ValidKind::TextLength,
            "custom" => ValidKind::Custom,
            _ => ValidKind::Any,
        },
        a: f1,
        b: f2,
        ignore_blank: n.attr_on("allowBlank"),
        in_cell_list: n.attr("showDropDown") != Some("1"),
        prompt_title: n.attr("promptTitle").unwrap_or("").into(),
        prompt: n.attr("prompt").unwrap_or("").into(),
        error_title: n.attr("errorTitle").unwrap_or("").into(),
        error: n.attr("error").unwrap_or("").into(),
        error_style: match n.attr("errorStyle") {
            Some("warning") => 1,
            Some("information") => 2,
            _ => 0,
        },
        ..Default::default()
    };
    if v.kind == ValidKind::List {
        if let Some(t) = v.a.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
            v.a = t.to_string();
        }
    }
    v.op = n.attr("operator").unwrap_or("between").to_string();
    sh.valid.push(v);
}

fn rels(z: &mut Zip, path: &str) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
    if let Some(n) = tree(z, &format!("{dir}/_rels/{file}.rels")) {
        for r in n.children("Relationship") {
            if let (Some(id), Some(t)) = (r.attr("Id"), r.attr("Target")) {
                let ty = r.attr("Type").unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
                let target = if r.attr("TargetMode") == Some("External") { t.to_string() } else { resolve(dir, t) };
                out.insert(id.to_string(), (target, ty));
            }
        }
    }
    out
}

fn resolve(dir: &str, t: &str) -> String {
    if let Some(a) = t.strip_prefix('/') {
        return a.to_string();
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|x| !x.is_empty()).collect();
    for p in t.split('/') {
        match p {
            ".." => {
                parts.pop();
            }
            "." => {}
            x => parts.push(x),
        }
    }
    parts.join("/")
}

fn chart_kind(root: &Node) -> Option<(ChartKind, &Node)> {
    let area = root.find("plotArea")?;
    for k in &area.kids {
        let stacked = k.child("grouping").and_then(|g| g.attr("val")).is_some_and(|v| v.contains("tacked"));
        let kind = match k.local() {
            "barChart" | "bar3DChart" => {
                let bar = k.child("barDir").and_then(|d| d.attr("val")) == Some("bar");
                match (bar, stacked) {
                    (true, true) => ChartKind::BarStacked,
                    (true, false) => ChartKind::Bar,
                    (false, true) => ChartKind::ColumnStacked,
                    (false, false) => ChartKind::Column,
                }
            }
            "lineChart" | "line3DChart" => {
                if k.child("marker").is_some_and(|m| m.attr("val") == Some("1")) {
                    ChartKind::LineMarkers
                } else {
                    ChartKind::Line
                }
            }
            "areaChart" | "area3DChart" => ChartKind::Area,
            "pieChart" | "pie3DChart" => ChartKind::Pie,
            "doughnutChart" => ChartKind::Doughnut,
            "scatterChart" => ChartKind::Scatter,
            _ => continue,
        };
        return Some((kind, k));
    }
    None
}

fn read_chart(root: &Node, x: f32, y: f32, w: f32, h: f32) -> Option<Chart> {
    let (kind, plot) = chart_kind(root)?;
    let mut series = vec![];
    let refs = |n: Option<&Node>| n.and_then(|v| v.find("f")).map(|f| f.all_text());
    for s in plot.children("ser") {
        let tx = s.child("tx");
        let mut item = Series {
            name_ref: refs(tx),
            name: tx.and_then(|t| t.find("v")).map(|v| v.all_text()).unwrap_or_default(),
            cats: refs(s.child("cat").or(s.child("xVal"))),
            vals: refs(s.child("val").or(s.child("yVal"))).unwrap_or_default(),
            color: None,
        };
        if let Some(c) = s.child("spPr").and_then(|p| p.find("srgbClr")).and_then(|c| c.attr("val")) {
            item.color = u32::from_str_radix(c, 16).ok();
        }
        if item.name.is_empty() {
            item.name = format!("Series{}", series.len() + 1);
        }
        series.push(item);
    }
    let chart = root.child("chart")?;
    let text_of = |n: Option<&Node>| -> String {
        let Some(n) = n else { return String::new() };
        let mut v = vec![];
        n.find_all("t", &mut v);
        v.iter().map(|t| t.all_text()).collect::<Vec<_>>().join("")
    };
    let area = root.find("plotArea")?;
    let axes: Vec<&Node> = area.kids.iter().filter(|k| k.local().ends_with("Ax")).collect();
    let ax_t = |i: usize| text_of(axes.get(i).and_then(|a| a.child("title")));
    Some(Chart {
        kind,
        title: text_of(chart.child("title")),
        series,
        x,
        y,
        w,
        h,
        legend: chart.child("legend").is_some(),
        x_title: ax_t(0),
        y_title: ax_t(1),
        data_labels: plot.find("dLbls").and_then(|d| d.child("showVal")).is_some_and(|v| v.attr("val") == Some("1")),
        gridlines: area.find("majorGridlines").is_some(),
    })
}

fn sheet_px(sh: &Sheet, col: u32, row: u32) -> (f32, f32) {
    let x: f32 = (0..col.min(2000)).map(|c| sh.col_px(c)).sum();
    let y: f32 = (0..row.min(5000)).map(|r| sh.row_px(r)).sum();
    (x, y)
}

fn read_drawing(z: &mut Zip, path: &str, sh: &mut Sheet) {
    let Some(root) = tree(z, path) else { return };
    let dr = rels(z, path);
    const EMU: f32 = 9525.0;
    for a in &root.kids {
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 480.0, 288.0);
        match a.local() {
            "twoCellAnchor" | "oneCellAnchor" => {
                let pt = |n: &Node, sh: &Sheet| -> Option<(f32, f32)> {
                    let c = n.child("col")?.all_text().parse().ok()?;
                    let r = n.child("row")?.all_text().parse().ok()?;
                    let co: f32 = n.child("colOff")?.all_text().parse().ok()?;
                    let ro: f32 = n.child("rowOff")?.all_text().parse().ok()?;
                    let (px, py) = sheet_px(sh, c, r);
                    Some((px + co / EMU, py + ro / EMU))
                };
                if let Some((px, py)) = a.child("from").and_then(|f| pt(f, sh)) {
                    x = px;
                    y = py;
                }
                if let Some((px, py)) = a.child("to").and_then(|f| pt(f, sh)) {
                    w = px - x;
                    h = py - y;
                }
                if let Some(e) = a.child("ext") {
                    w = e.attr_f("cx").unwrap_or(0.0) as f32 / EMU;
                    h = e.attr_f("cy").unwrap_or(0.0) as f32 / EMU;
                }
            }
            "absoluteAnchor" => {
                if let Some(p) = a.child("pos") {
                    x = p.attr_f("x").unwrap_or(0.0) as f32 / EMU;
                    y = p.attr_f("y").unwrap_or(0.0) as f32 / EMU;
                }
                if let Some(e) = a.child("ext") {
                    w = e.attr_f("cx").unwrap_or(0.0) as f32 / EMU;
                    h = e.attr_f("cy").unwrap_or(0.0) as f32 / EMU;
                }
            }
            _ => continue,
        }
        let Some(gf) = a.find("chart") else { continue };
        let Some((target, _)) = gf.attr("id").and_then(|id| dr.get(id)) else { continue };
        if let Some(ch) = tree(z, target) {
            if let Some(c) = read_chart(&ch, x, y, w, h) {
                sh.charts.push(c);
            }
        }
    }
}

fn read_sheet(z: &mut Zip, path: &str, st: &Styles, strings: &[Arc<str>], sh: &mut Sheet) {
    let Some(root) = tree(z, path) else { return };
    let sr = rels(z, path);
    if let Some(v) = root.child("sheetViews").and_then(|x| x.child("sheetView")) {
        if v.attr("showGridLines") == Some("0") {
            sh.gridlines = false;
        }
        if v.attr("showRowColHeaders") == Some("0") {
            sh.headings = false;
        }
        if let Some(z) = v.attr_f("zoomScale") {
            sh.zoom = (z as f32 / 100.0).clamp(0.1, 4.0);
        }
        if let Some(p) = v.child("pane") {
            if p.attr("state").is_some_and(|s| s.contains("rozen")) {
                sh.freeze = (p.attr_u("ySplit").unwrap_or(0), p.attr_u("xSplit").unwrap_or(0));
            }
        }
    }
    let default_h = root.child("sheetFormatPr").and_then(|f| f.attr_f("defaultRowHeight")).unwrap_or(15.0);
    if let Some(cols) = root.child("cols") {
        for c in cols.children("col") {
            let (a, b) = (c.attr_u("min").unwrap_or(1), c.attr_u("max").unwrap_or(1));
            if b.saturating_sub(a) > 2000 && !c.attr_on("hidden") {
                continue;
            }
            for i in a.saturating_sub(1)..b.min(MAX_COLS) {
                if let Some(w) = c.attr_f("width") {
                    sh.col_w.insert(i, (w * 7.0 + 5.0).round() as f32);
                }
                if c.attr_on("hidden") {
                    sh.hidden_cols.insert(i);
                }
                if let Some(s) = c.attr_u("style") {
                    if let Some(id) = st.xf.get(s as usize) {
                        if *id != 0 {
                            sh.col_style.insert(i, *id);
                        }
                    }
                }
            }
        }
    }
    let mut shared: HashMap<u32, (Expr, u32, u32)> = HashMap::new();
    if let Some(data) = root.child("sheetData") {
        for row in data.children("row") {
            let r = row.attr_u("r").unwrap_or(1).saturating_sub(1);
            if let Some(h) = row.attr_f("ht") {
                if row.attr_on("customHeight") || (h - default_h).abs() > 0.01 {
                    sh.row_h.insert(r, (h as f32 * 4.0 / 3.0).round());
                    if row.attr_on("customHeight") {
                        sh.custom_h.insert(r);
                    }
                }
            }
            if row.attr_on("hidden") {
                sh.hidden_rows.insert(r);
            }
            if row.attr_on("customFormat") {
                if let Some(id) = row.attr_u("s").and_then(|s| st.xf.get(s as usize)) {
                    sh.row_style.insert(r, *id);
                }
            }
            if let Some(l) = row.attr_u("outlineLevel") {
                sh.outline_rows.insert(r, l as u8);
            }
            for c in row.children("c") {
                let Some(p) = c.attr("r").and_then(parse_cell) else { continue };
                let style = c.attr_u("s").and_then(|s| st.xf.get(s as usize)).copied().unwrap_or(0);
                let t = c.attr("t").unwrap_or("n");
                let v = c.child("v").map(|x| x.all_text());
                let mut content = match t {
                    "s" => v.and_then(|i| i.trim().parse::<usize>().ok()).and_then(|i| strings.get(i).cloned()).map(Content::Text).unwrap_or_default(),
                    "str" => Content::Text(v.unwrap_or_default().into()),
                    "inlineStr" => Content::Text(c.child("is").map(|x| x.all_text()).unwrap_or_default().into()),
                    "b" => Content::Bool(v.as_deref() == Some("1")),
                    "e" => Content::Err(v.as_deref().and_then(XlErr::parse).unwrap_or(XlErr::Value)),
                    _ => v.and_then(|x| x.trim().parse::<f64>().ok()).map(Content::Num).unwrap_or_default(),
                };
                if let Some(f) = c.child("f") {
                    let txt = f.all_text();
                    let si = f.attr_u("si");
                    let parsed = if f.attr("t") == Some("shared") && txt.is_empty() {
                        si.and_then(|s| shared.get(&s)).map(|(e, r0, c0)| formula::shifted(e, p.row as i64 - *r0 as i64, p.col as i64 - *c0 as i64))
                    } else {
                        let e = formula::parse(&txt).ok();
                        if let (Some(s), Some(x)) = (si, &e) {
                            if f.attr("t") == Some("shared") {
                                shared.insert(s, (x.clone(), p.row, p.col));
                            }
                        }
                        e
                    };
                    if let Some(e) = parsed {
                        content = Content::Formula(Arc::new(e));
                    }
                }
                if content != Content::Empty || style != 0 {
                    let cell = sh.cell_mut(p.row, p.col);
                    cell.v = content;
                    cell.style = style;
                }
            }
        }
    }
    if let Some(m) = root.child("mergeCells") {
        for c in m.children("mergeCell") {
            if let Some(x) = c.attr("ref").and_then(parse_rect) {
                sh.merges.push(x);
            }
        }
    }
    for c in root.children("conditionalFormatting") {
        read_cond(c, st, sh);
    }
    if let Some(d) = root.child("dataValidations") {
        for v in d.children("dataValidation") {
            read_valid(v, sh);
        }
    }
    if let Some(h) = root.child("hyperlinks") {
        for l in h.children("hyperlink") {
            let Some(rc) = l.attr("ref").and_then(parse_rect) else { continue };
            let target = l.attr("id").and_then(|i| sr.get(i)).map(|x| x.0.clone()).or(l.attr("location").map(|x| format!("#{x}")));
            if let Some(t) = target {
                sh.links.insert(Pos::new(rc.r1, rc.c1), t);
            }
        }
    }
    if let Some(a) = root.child("autoFilter") {
        if let Some(range) = a.attr("ref").and_then(parse_rect) {
            let mut f = AutoFilter { range, cols: Default::default() };
            for fc in a.children("filterColumn") {
                let Some(id) = fc.attr_u("colId") else { continue };
                let mut cf = ColFilter::default();
                if let Some(fl) = fc.child("filters") {
                    cf.allowed = Some(fl.children("filter").filter_map(|x| x.attr("val").map(|v| v.to_string())).collect());
                }
                f.cols.insert(id, cf);
            }
            sh.filter = Some(f);
        }
    }
    if let Some(ps) = root.child("pageSetup") {
        sh.page.landscape = ps.attr("orientation") == Some("landscape");
        sh.page.paper = match ps.attr_u("paperSize") {
            Some(9) => 1,
            Some(5) => 2,
            Some(8) => 3,
            _ => 0,
        };
        sh.page.scale = ps.attr_u("scale").unwrap_or(100);
    }
    if let Some(m) = root.child("pageMargins") {
        sh.page.margins = [
            m.attr_f("left").unwrap_or(0.7) as f32,
            m.attr_f("right").unwrap_or(0.7) as f32,
            m.attr_f("top").unwrap_or(0.75) as f32,
            m.attr_f("bottom").unwrap_or(0.75) as f32,
        ];
    }
    if let Some(tc) = root.child("sheetPr").and_then(|p| p.child("tabColor")) {
        sh.tab_color = color(Some(tc), &st.theme);
    }
    if root.child("sheetProtection").is_some_and(|p| p.attr_on("sheet")) {
        sh.protection = Some(Protection::default());
    }
    for (target, ty) in sr.values() {
        match ty.as_str() {
            "drawing" => read_drawing(z, target, sh),
            "comments" => {
                if let Some(c) = tree(z, target) {
                    let authors: Vec<String> = c.child("authors").map(|a| a.children("author").map(|x| x.all_text()).collect()).unwrap_or_default();
                    if let Some(list) = c.child("commentList") {
                        for k in list.children("comment") {
                            let Some(p) = k.attr("ref").and_then(parse_cell) else { continue };
                            let text = k.child("text").map(|t| t.all_text()).unwrap_or_default();
                            let author = k.attr_u("authorId").and_then(|i| authors.get(i as usize)).cloned().unwrap_or_default();
                            sh.notes.insert(p, Note { text, author });
                        }
                    }
                }
            }
            "table" => {
                if let Some(t) = tree(z, target) {
                    if let Some(range) = t.attr("ref").and_then(parse_rect) {
                        let columns = t.child("tableColumns").map(|c| c.children("tableColumn").filter_map(|x| x.attr("name").map(|s| s.to_string())).collect()).unwrap_or_default();
                        let info = t.child("tableStyleInfo");
                        let sty = info.and_then(|i| i.attr("name")).and_then(|n| n.trim_start_matches(|c: char| !c.is_ascii_digit()).parse::<u8>().ok()).unwrap_or(2);
                        sh.tables.push(TableDef {
                            name: t.attr("displayName").or(t.attr("name")).unwrap_or("Table1").to_string(),
                            range,
                            style: sty,
                            header: t.attr("headerRowCount") != Some("0"),
                            totals: t.attr_u("totalsRowCount").unwrap_or(0) > 0,
                            banded: info.is_none_or(|i| i.attr("showRowStripes") != Some("0")),
                            columns,
                        });
                        if t.child("autoFilter").is_some() && sh.filter.is_none() {
                            sh.filter = Some(AutoFilter { range, cols: Default::default() });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn load(data: Vec<u8>) -> Result<Workbook, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| format!("Not a spreadsheet file: {e}"))?;
    let root = tree(&mut z, "xl/workbook.xml").ok_or("This file has no workbook part")?;
    let wr = rels(&mut z, "xl/workbook.xml");
    let mut wb = Workbook { sheets: vec![], ..Default::default() };
    let mut theme = DEFAULT_THEME.to_vec();
    if let Some(t) = tree(&mut z, "xl/theme/theme1.xml") {
        if let Some(cs) = t.find("clrScheme") {
            let order = ["lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"];
            for (i, n) in order.iter().enumerate() {
                if let Some(c) = cs.child(n).and_then(|k| k.kids.first()) {
                    let v = c.attr("lastClr").or(c.attr("val")).unwrap_or("");
                    if let Ok(x) = u32::from_str_radix(v, 16) {
                        theme[i] = x;
                    }
                }
            }
        }
    }
    let styles = read_styles(tree(&mut z, "xl/styles.xml"), theme, &mut wb);
    let mut strings: Vec<Arc<str>> = vec![];
    if let Some(s) = tree(&mut z, "xl/sharedStrings.xml") {
        for si in s.children("si") {
            // Phonetic runs are not part of the shown text.
            let t: String = si.kids.iter().filter(|k| k.local() == "t" || k.local() == "r").map(|k| k.all_text()).collect();
            strings.push(t.into());
        }
    }
    let list: Vec<(String, String, bool)> = root
        .child("sheets")
        .map(|s| {
            s.children("sheet")
                .filter_map(|x| {
                    let id = x.attr("r:id").or(x.attr("id"))?;
                    let target = wr.get(id)?.0.clone();
                    Some((x.attr("name")?.to_string(), target, x.attr("state").is_some_and(|v| v != "visible")))
                })
                .collect()
        })
        .unwrap_or_default();
    for (name, target, hidden) in list {
        let mut sh = Sheet::new(&name);
        sh.hidden = hidden;
        read_sheet(&mut z, &target, &styles, &strings, &mut sh);
        wb.sheets.push(Arc::new(sh));
    }
    if wb.sheets.is_empty() {
        return Err("The workbook has no sheets".into());
    }
    if let Some(d) = root.child("definedNames") {
        for n in d.children("definedName") {
            let name = n.attr("name").unwrap_or("").to_string();
            if name.starts_with("_xlnm") {
                if name == "_xlnm.Print_Area" {
                    if let Some(i) = n.attr_u("localSheetId") {
                        let t = n.all_text();
                        let rc = t.rsplit('!').next().and_then(parse_rect);
                        if (i as usize) < wb.sheets.len() {
                            wb.sheet_mut(i as usize).page.print_area = rc;
                        }
                    }
                }
                continue;
            }
            wb.names.push(DefinedName { name, expr: n.all_text(), comment: n.attr("comment").unwrap_or("").into() });
        }
    }
    Ok(wb)
}
