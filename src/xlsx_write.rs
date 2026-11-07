//! Writes .xlsx workbooks.

use crate::eval::*;
use crate::formula::{self, Expr};
use crate::model::*;
use crate::xml::esc;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

const NEW_FUNCS: &[&str] = &[
    "ACOT", "ACOTH", "AGGREGATE", "ARABIC", "BASE", "BETA.DIST", "BETA.INV", "BINOM.DIST", "BINOM.INV", "BITAND",
    "BITLSHIFT", "BITOR", "BITRSHIFT", "BITXOR", "CEILING.MATH", "CEILING.PRECISE", "CHISQ.DIST", "CHISQ.DIST.RT",
    "CHISQ.INV", "CHISQ.INV.RT", "CHISQ.TEST", "COMBINA", "CONCAT", "CONFIDENCE.NORM", "CONFIDENCE.T", "COT", "COTH",
    "COVARIANCE.P", "COVARIANCE.S", "CSC", "CSCH", "DAYS", "DECIMAL", "ERF.PRECISE", "ERFC.PRECISE", "EXPON.DIST",
    "F.DIST", "F.DIST.RT", "F.INV", "F.INV.RT", "F.TEST", "FLOOR.MATH", "FLOOR.PRECISE", "FORECAST.LINEAR",
    "FORMULATEXT", "GAMMA", "GAMMA.DIST", "GAMMA.INV", "GAMMALN.PRECISE", "GAUSS", "HYPGEOM.DIST", "IFNA", "IFS",
    "ISFORMULA", "ISOWEEKNUM", "LET", "LOGNORM.DIST", "LOGNORM.INV", "MAXIFS", "MINIFS", "MODE.MULT", "MODE.SNGL",
    "MUNIT", "NEGBINOM.DIST", "NORM.DIST", "NORM.INV", "NORM.S.DIST", "NORM.S.INV", "NUMBERVALUE", "PDURATION",
    "PERCENTILE.EXC", "PERCENTILE.INC", "PERCENTRANK.EXC", "PERCENTRANK.INC", "PERMUTATIONA", "PHI", "POISSON.DIST",
    "QUARTILE.EXC", "QUARTILE.INC", "RANDARRAY", "RANK.AVG", "RANK.EQ", "RRI", "SEC", "SECH", "SEQUENCE", "SHEET",
    "SHEETS", "SKEW.P", "SORTBY", "STDEV.P", "STDEV.S", "SWITCH", "T.DIST", "T.DIST.2T", "T.DIST.RT", "T.INV",
    "T.INV.2T", "T.TEST", "TEXTJOIN", "TEXTAFTER", "TEXTBEFORE", "TEXTSPLIT", "TOCOL", "TOROW", "UNICHAR", "UNICODE",
    "UNIQUE", "VAR.P", "VAR.S", "VSTACK", "HSTACK", "WEIBULL.DIST", "XLOOKUP", "XMATCH", "XOR", "Z.TEST", "LAMBDA",
    "MAP", "REDUCE", "SCAN", "BYROW", "BYCOL", "MAKEARRAY", "TAKE", "DROP", "CHOOSEROWS", "CHOOSECOLS", "EXPAND",
    "WRAPROWS", "WRAPCOLS",
];

fn prefix_calls(e: &mut Expr) {
    match e {
        Expr::Call(n, args) => {
            let up: &str = n;
            let renamed = match up {
                "FILTER" | "SORT" => Some(format!("_xlfn._xlws.{up}")),
                x if NEW_FUNCS.contains(&x) => Some(format!("_xlfn.{up}")),
                _ => None,
            };
            if let Some(r) = renamed {
                *n = r.into();
            }
            for a in args {
                prefix_calls(a);
            }
        }
        Expr::Neg(x) | Expr::Plus(x) | Expr::Percent(x) | Expr::Paren(x) | Expr::Spill(x) | Expr::Implicit(x) => prefix_calls(x),
        Expr::Bin(_, a, b) | Expr::RangeOp(a, b) => {
            prefix_calls(a);
            prefix_calls(b);
        }
        Expr::Array(rows) => rows.iter_mut().flatten().for_each(prefix_calls),
        _ => {}
    }
}

pub fn formula_for_file(e: &Expr) -> String {
    let mut x = e.clone();
    prefix_calls(&mut x);
    formula::print(&x)
}

fn hex(c: u32) -> String {
    format!("FF{:06X}", c & 0xFFFFFF)
}

fn fmt_id(code: &str, custom: &mut Vec<String>) -> u32 {
    let b = match code {
        "General" => 0,
        "0" => 1,
        "0.00" => 2,
        "#,##0" => 3,
        "#,##0.00" => 4,
        "0%" => 9,
        "0.00%" => 10,
        "0.00E+00" => 11,
        "# ?/?" => 12,
        "# ??/??" => 13,
        "m/d/yyyy" => 14,
        "d-mmm-yy" => 15,
        "d-mmm" => 16,
        "mmm-yy" => 17,
        "h:mm AM/PM" => 18,
        "h:mm:ss AM/PM" => 19,
        "h:mm" => 20,
        "h:mm:ss" => 21,
        "m/d/yyyy h:mm" => 22,
        "@" => 49,
        _ => {
            let i = custom.iter().position(|x| x == code).unwrap_or_else(|| {
                custom.push(code.to_string());
                custom.len() - 1
            });
            return 164 + i as u32;
        }
    };
    b
}

fn line(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Thin => "thin",
        LineStyle::Medium => "medium",
        LineStyle::Thick => "thick",
        LineStyle::Dashed => "dashed",
        LineStyle::Dotted => "dotted",
        LineStyle::Double => "double",
        LineStyle::Hair => "hair",
    }
}

/// Builds styles.xml. Every internal style id maps to the same cellXfs index, and rule styles to dxfs.
fn styles_xml(wb: &Workbook, dxf: &[CondStyle]) -> String {
    let mut fonts: Vec<String> = vec![];
    let mut fills: Vec<String> = vec![
        "<fill><patternFill patternType=\"none\"/></fill>".into(),
        "<fill><patternFill patternType=\"gray125\"/></fill>".into(),
    ];
    let mut borders: Vec<String> = vec![];
    let mut custom: Vec<String> = vec![];
    let mut xfs = String::new();
    fn index(list: &mut Vec<String>, s: String) -> usize {
        match list.iter().position(|x| *x == s) {
            Some(i) => i,
            None => {
                list.push(s);
                list.len() - 1
            }
        }
    }
    for st in &wb.styles.list {
        let mut f = String::from("<font>");
        if st.bold {
            f.push_str("<b/>");
        }
        if st.italic {
            f.push_str("<i/>");
        }
        if st.strike {
            f.push_str("<strike/>");
        }
        if st.underline == 1 {
            f.push_str("<u/>");
        } else if st.underline == 2 {
            f.push_str("<u val=\"double\"/>");
        }
        let _ = write!(f, "<sz val=\"{}\"/>", st.size10 as f32 / 10.0);
        if let Some(c) = st.color {
            let _ = write!(f, "<color rgb=\"{}\"/>", hex(c));
        }
        let _ = write!(f, "<name val=\"{}\"/><family val=\"2\"/></font>", esc(&st.font));
        let font_id = index(&mut fonts, f);
        let fill_id = match st.fill {
            Some(c) => index(&mut fills, format!("<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{}\"/><bgColor indexed=\"64\"/></patternFill></fill>", hex(c))),
            None => 0,
        };
        let mut b = String::from("<border>");
        for (i, n) in ["left", "right", "top", "bottom"].iter().enumerate() {
            match &st.borders[i] {
                Some(x) => {
                    let _ = write!(b, "<{n} style=\"{}\"><color rgb=\"{}\"/></{n}>", line(x.style), hex(x.color));
                }
                None => {
                    let _ = write!(b, "<{n}/>");
                }
            }
        }
        b.push_str("<diagonal/></border>");
        let border_id = index(&mut borders, b);
        let nf = fmt_id(&st.num_fmt, &mut custom);
        let mut xf = format!(
            "<xf numFmtId=\"{nf}\" fontId=\"{font_id}\" fillId=\"{fill_id}\" borderId=\"{border_id}\" xfId=\"0\" applyNumberFormat=\"1\" applyFont=\"1\" applyFill=\"1\" applyBorder=\"1\" applyAlignment=\"1\" applyProtection=\"1\">"
        );
        let h = match st.h {
            HAlign::General => "",
            HAlign::Left => " horizontal=\"left\"",
            HAlign::Center => " horizontal=\"center\"",
            HAlign::Right => " horizontal=\"right\"",
            HAlign::Fill => " horizontal=\"fill\"",
            HAlign::Justify => " horizontal=\"justify\"",
            HAlign::CenterAcross => " horizontal=\"centerContinuous\"",
        };
        let v = match st.v {
            VAlign::Top => " vertical=\"top\"",
            VAlign::Center => " vertical=\"center\"",
            VAlign::Bottom => "",
        };
        let rot = if st.rotation < 0 { 90 - st.rotation as i32 } else { st.rotation as i32 };
        let _ = write!(
            xf,
            "<alignment{h}{v}{}{}{}{}/>",
            if st.wrap { " wrapText=\"1\"" } else { "" },
            if st.shrink { " shrinkToFit=\"1\"" } else { "" },
            if st.indent > 0 { format!(" indent=\"{}\"", st.indent) } else { String::new() },
            if rot != 0 { format!(" textRotation=\"{rot}\"") } else { String::new() }
        );
        let _ = write!(xf, "<protection locked=\"{}\" hidden=\"{}\"/></xf>", st.locked as u8, st.hide_formula as u8);
        xfs.push_str(&xf);
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">");
    if !custom.is_empty() {
        let _ = write!(out, "<numFmts count=\"{}\">", custom.len());
        for (i, c) in custom.iter().enumerate() {
            let _ = write!(out, "<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>", 164 + i, esc(c));
        }
        out.push_str("</numFmts>");
    }
    let _ = write!(out, "<fonts count=\"{}\">{}</fonts>", fonts.len(), fonts.concat());
    let _ = write!(out, "<fills count=\"{}\">{}</fills>", fills.len(), fills.concat());
    let _ = write!(out, "<borders count=\"{}\">{}</borders>", borders.len(), borders.concat());
    out.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>");
    let _ = write!(out, "<cellXfs count=\"{}\">{xfs}</cellXfs>", wb.styles.list.len());
    out.push_str("<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>");
    let _ = write!(out, "<dxfs count=\"{}\">", dxf.len());
    for d in dxf {
        out.push_str("<dxf>");
        if d.bold || d.italic || d.color.is_some() {
            out.push_str("<font>");
            if d.bold {
                out.push_str("<b/>");
            }
            if d.italic {
                out.push_str("<i/>");
            }
            if let Some(c) = d.color {
                let _ = write!(out, "<color rgb=\"{}\"/>", hex(c));
            }
            out.push_str("</font>");
        }
        if let Some(c) = d.fill {
            let _ = write!(out, "<fill><patternFill><bgColor rgb=\"{}\"/></patternFill></fill>", hex(c));
        }
        out.push_str("</dxf>");
    }
    out.push_str("</dxfs><tableStyles count=\"0\"/></styleSheet>");
    out
}

fn cond_xml(c: &CondFormat, prio: &mut u32, dxf: &mut Vec<CondStyle>) -> String {
    let sq: Vec<String> = c.ranges.iter().map(|r| r.a1()).collect();
    let id = match dxf.iter().position(|d| *d == c.style) {
        Some(i) => i,
        None => {
            dxf.push(c.style.clone());
            dxf.len() - 1
        }
    };
    *prio += 1;
    let p = *prio;
    let first = c.ranges.first().map(|r| cell_name(r.r1, r.c1)).unwrap_or_default();
    let stop = if c.stop_if_true { " stopIfTrue=\"1\"" } else { "" };
    let body = match &c.rule {
        CondRule::CellIs { op, a, b } => {
            let o = match op.as_str() {
                ">" => "greaterThan",
                "<" => "lessThan",
                ">=" => "greaterThanOrEqual",
                "<=" => "lessThanOrEqual",
                "<>" => "notEqual",
                "between" => "between",
                "notbetween" => "notBetween",
                _ => "equal",
            };
            let f2 = if op.contains("between") { format!("<formula>{}</formula>", esc(b.trim_start_matches('='))) } else { String::new() };
            format!("<cfRule type=\"cellIs\" dxfId=\"{id}\" priority=\"{p}\"{stop} operator=\"{o}\"><formula>{}</formula>{f2}</cfRule>", esc(a.trim_start_matches('=')))
        }
        CondRule::Formula { formula } => format!("<cfRule type=\"expression\" dxfId=\"{id}\" priority=\"{p}\"{stop}><formula>{}</formula></cfRule>", esc(formula.trim_start_matches('='))),
        CondRule::TextContains { text, mode } => {
            let t = esc(text);
            let (ty, op, f) = match mode {
                TextMode::Contains => ("containsText", "containsText", format!("NOT(ISERROR(SEARCH(\"{t}\",{first})))")),
                TextMode::NotContains => ("notContainsText", "notContains", format!("ISERROR(SEARCH(\"{t}\",{first}))")),
                TextMode::Begins => ("beginsWith", "beginsWith", format!("LEFT({first},{})=\"{t}\"", text.chars().count())),
                TextMode::Ends => ("endsWith", "endsWith", format!("RIGHT({first},{})=\"{t}\"", text.chars().count())),
            };
            format!("<cfRule type=\"{ty}\" dxfId=\"{id}\" priority=\"{p}\" operator=\"{op}\" text=\"{t}\"><formula>{f}</formula></cfRule>")
        }
        CondRule::Duplicate { unique } => format!("<cfRule type=\"{}\" dxfId=\"{id}\" priority=\"{p}\"/>", if *unique { "uniqueValues" } else { "duplicateValues" }),
        CondRule::TopBottom { top, rank, percent } => format!(
            "<cfRule type=\"top10\" dxfId=\"{id}\" priority=\"{p}\" rank=\"{rank}\"{}{}/>",
            if *top { "" } else { " bottom=\"1\"" },
            if *percent { " percent=\"1\"" } else { "" }
        ),
        CondRule::AboveAverage { above } => format!("<cfRule type=\"aboveAverage\" dxfId=\"{id}\" priority=\"{p}\"{}/>", if *above { "" } else { " aboveAverage=\"0\"" }),
        CondRule::Blanks { blank } => {
            let (t, f) = if *blank { ("containsBlanks", format!("LEN(TRIM({first}))=0")) } else { ("notContainsBlanks", format!("LEN(TRIM({first}))>0")) };
            format!("<cfRule type=\"{t}\" dxfId=\"{id}\" priority=\"{p}\"><formula>{f}</formula></cfRule>")
        }
        CondRule::ColorScale { colors } => {
            let mut s = format!("<cfRule type=\"colorScale\" priority=\"{p}\"><colorScale><cfvo type=\"min\"/>");
            if colors.len() > 2 {
                s.push_str("<cfvo type=\"percentile\" val=\"50\"/>");
            }
            s.push_str("<cfvo type=\"max\"/>");
            for k in colors {
                let _ = write!(s, "<color rgb=\"{}\"/>", hex(*k));
            }
            s.push_str("</colorScale></cfRule>");
            s
        }
        CondRule::DataBar { color } => format!("<cfRule type=\"dataBar\" priority=\"{p}\"><dataBar><cfvo type=\"min\"/><cfvo type=\"max\"/><color rgb=\"{}\"/></dataBar></cfRule>", hex(*color)),
        CondRule::IconSet { set } => {
            let n = match set {
                IconSet::Arrows3 => "3Arrows",
                IconSet::Lights3 => "3TrafficLights1",
                IconSet::Flags3 => "3Flags",
                IconSet::Stars3 => "3Stars",
            };
            format!("<cfRule type=\"iconSet\" priority=\"{p}\"><iconSet iconSet=\"{n}\"><cfvo type=\"percent\" val=\"0\"/><cfvo type=\"percent\" val=\"33\"/><cfvo type=\"percent\" val=\"67\"/></iconSet></cfRule>")
        }
    };
    format!("<conditionalFormatting sqref=\"{}\">{body}</conditionalFormatting>", sq.join(" "))
}

fn valid_xml(v: &Validation) -> String {
    let ty = match v.kind {
        ValidKind::Any => "none",
        ValidKind::Whole => "whole",
        ValidKind::Decimal => "decimal",
        ValidKind::List => "list",
        ValidKind::Date => "date",
        ValidKind::Time => "time",
        ValidKind::TextLength => "textLength",
        ValidKind::Custom => "custom",
    };
    let sq: Vec<String> = v.ranges.iter().map(|r| r.a1()).collect();
    let es = match v.error_style {
        1 => " errorStyle=\"warning\"",
        2 => " errorStyle=\"information\"",
        _ => "",
    };
    let mut f1 = v.a.trim_start_matches('=').to_string();
    if v.kind == ValidKind::List && !v.a.starts_with('=') && !v.a.contains('!') && parse_rect(&v.a).is_none() {
        f1 = format!("\"{}\"", v.a);
    }
    let mut s = format!(
        "<dataValidation type=\"{ty}\"{es} operator=\"{}\" allowBlank=\"{}\" showDropDown=\"{}\" showInputMessage=\"1\" showErrorMessage=\"1\" errorTitle=\"{}\" error=\"{}\" promptTitle=\"{}\" prompt=\"{}\" sqref=\"{}\">",
        esc(&v.op),
        v.ignore_blank as u8,
        !v.in_cell_list as u8,
        esc(&v.error_title),
        esc(&v.error),
        esc(&v.prompt_title),
        esc(&v.prompt),
        sq.join(" ")
    );
    let _ = write!(s, "<formula1>{}</formula1>", esc(&f1));
    if !v.b.is_empty() {
        let _ = write!(s, "<formula2>{}</formula2>", esc(v.b.trim_start_matches('=')));
    }
    s.push_str("</dataValidation>");
    s
}

fn series_xml(i: usize, s: &Series, kind: ChartKind) -> String {
    let mut x = format!("<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>");
    match &s.name_ref {
        Some(r) => {
            let _ = write!(x, "<c:tx><c:strRef><c:f>{}</c:f></c:strRef></c:tx>", esc(r));
        }
        None => {
            let _ = write!(x, "<c:tx><c:v>{}</c:v></c:tx>", esc(&s.name));
        }
    }
    if let Some(c) = s.color {
        let _ = write!(x, "<c:spPr><a:solidFill><a:srgbClr val=\"{:06X}\"/></a:solidFill></c:spPr>", c);
    }
    if kind == ChartKind::Scatter {
        if let Some(c) = &s.cats {
            let _ = write!(x, "<c:xVal><c:numRef><c:f>{}</c:f></c:numRef></c:xVal>", esc(c));
        }
        let _ = write!(x, "<c:yVal><c:numRef><c:f>{}</c:f></c:numRef></c:yVal><c:smooth val=\"0\"/>", esc(&s.vals));
    } else {
        if let Some(c) = &s.cats {
            let _ = write!(x, "<c:cat><c:strRef><c:f>{}</c:f></c:strRef></c:cat>", esc(c));
        }
        let _ = write!(x, "<c:val><c:numRef><c:f>{}</c:f></c:numRef></c:val>", esc(&s.vals));
    }
    x.push_str("</c:ser>");
    x
}

fn text_xml(t: &str) -> String {
    format!("<c:title><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>", esc(t))
}

fn chart_xml(ch: &Chart) -> String {
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><c:chart>");
    if !ch.title.is_empty() {
        x.push_str(&text_xml(&ch.title));
    }
    x.push_str("<c:autoTitleDeleted val=\"0\"/><c:plotArea><c:layout/>");
    let ser: String = ch.series.iter().enumerate().map(|(i, s)| series_xml(i, s, ch.kind)).collect();
    let dl = if ch.data_labels { "<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls>" } else { "" };
    let axes = "<c:axId val=\"111111\"/><c:axId val=\"222222\"/>";
    let (plain_axes, scatter_axes) = (true, ch.kind == ChartKind::Scatter);
    match ch.kind {
        ChartKind::Column | ChartKind::ColumnStacked | ChartKind::Bar | ChartKind::BarStacked => {
            let stacked = matches!(ch.kind, ChartKind::ColumnStacked | ChartKind::BarStacked);
            let dir = if matches!(ch.kind, ChartKind::Bar | ChartKind::BarStacked) { "bar" } else { "col" };
            let _ = write!(x, "<c:barChart><c:barDir val=\"{dir}\"/><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>{ser}{dl}<c:gapWidth val=\"150\"/>{}{axes}</c:barChart>", if stacked { "stacked" } else { "clustered" }, if stacked { "<c:overlap val=\"100\"/>" } else { "" });
        }
        ChartKind::Line | ChartKind::LineMarkers => {
            let _ = write!(x, "<c:lineChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/>{ser}{dl}<c:marker val=\"{}\"/>{axes}</c:lineChart>", (ch.kind == ChartKind::LineMarkers) as u8);
        }
        ChartKind::Area => {
            let _ = write!(x, "<c:areaChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/>{ser}{dl}{axes}</c:areaChart>");
        }
        ChartKind::Pie => {
            let _ = write!(x, "<c:pieChart><c:varyColors val=\"1\"/>{ser}{dl}</c:pieChart>");
        }
        ChartKind::Doughnut => {
            let _ = write!(x, "<c:doughnutChart><c:varyColors val=\"1\"/>{ser}{dl}<c:holeSize val=\"60\"/></c:doughnutChart>");
        }
        ChartKind::Scatter => {
            let _ = write!(x, "<c:scatterChart><c:scatterStyle val=\"lineMarker\"/><c:varyColors val=\"0\"/>{ser}{dl}{axes}</c:scatterChart>");
        }
    }
    let round = matches!(ch.kind, ChartKind::Pie | ChartKind::Doughnut);
    if plain_axes && !round {
        let horizontal = matches!(ch.kind, ChartKind::Bar | ChartKind::BarStacked);
        let (cat_pos, val_pos) = if horizontal { ("l", "b") } else { ("b", "l") };
        let grid = if ch.gridlines { "<c:majorGridlines/>" } else { "" };
        let t1 = if ch.x_title.is_empty() { String::new() } else { text_xml(&ch.x_title) };
        let t2 = if ch.y_title.is_empty() { String::new() } else { text_xml(&ch.y_title) };
        let first = if scatter_axes { "valAx" } else { "catAx" };
        let _ = write!(x, "<c:{first}><c:axId val=\"111111\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"{cat_pos}\"/>{t1}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"222222\"/><c:crosses val=\"autoZero\"/></c:{first}>");
        let _ = write!(x, "<c:valAx><c:axId val=\"222222\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"{val_pos}\"/>{grid}{t2}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"111111\"/><c:crosses val=\"autoZero\"/></c:valAx>");
    }
    x.push_str("</c:plotArea>");
    if ch.legend {
        x.push_str("<c:legend><c:legendPos val=\"b\"/><c:overlay val=\"0\"/></c:legend>");
    }
    x.push_str("<c:plotVisOnly val=\"1\"/></c:chart></c:chartSpace>");
    x
}

const NS_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[allow(clippy::too_many_arguments)]
fn sheet_xml(wb: &Workbook, calc: &CalcResult, si: usize, strings: &mut Vec<String>, map: &mut HashMap<String, usize>, prio: &mut u32, dxf: &mut Vec<CondStyle>, rels: &mut Vec<String>) -> String {
    let sh = &wb.sheets[si];
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">");
    if let Some(c) = sh.tab_color {
        let _ = write!(x, "<sheetPr><tabColor rgb=\"{}\"/></sheetPr>", hex(c));
    }
    if let Some(u) = sh.used_rect() {
        let _ = write!(x, "<dimension ref=\"{}\"/>", u.a1());
    }
    let _ = write!(x, "<sheetViews><sheetView workbookViewId=\"0\"{}{}{}{}>", if si == 0 { " tabSelected=\"1\"" } else { "" }, if sh.gridlines { "" } else { " showGridLines=\"0\"" }, if sh.headings { "" } else { " showRowColHeaders=\"0\"" }, if sh.zoom != 1.0 { format!(" zoomScale=\"{}\"", (sh.zoom * 100.0).round()) } else { String::new() });
    if sh.freeze != (0, 0) {
        let (r, c) = sh.freeze;
        let pane = match (r > 0, c > 0) {
            (true, true) => "bottomRight",
            (true, false) => "bottomLeft",
            _ => "topRight",
        };
        let _ = write!(x, "<pane{}{} topLeftCell=\"{}\" activePane=\"{pane}\" state=\"frozen\"/>", if c > 0 { format!(" xSplit=\"{c}\"") } else { String::new() }, if r > 0 { format!(" ySplit=\"{r}\"") } else { String::new() }, cell_name(r, c));
    }
    x.push_str("</sheetView></sheetViews><sheetFormatPr defaultRowHeight=\"15\"/>");
    let mut cols: Vec<u32> = sh.col_w.keys().chain(sh.hidden_cols.iter()).chain(sh.col_style.keys()).copied().collect();
    cols.sort();
    cols.dedup();
    if !cols.is_empty() {
        x.push_str("<cols>");
        for c in cols {
            let w = sh.col_w.get(&c).copied().unwrap_or(DEFAULT_COL_W);
            let _ = write!(x, "<col min=\"{0}\" max=\"{0}\" width=\"{1:.2}\" customWidth=\"1\"{2}{3}/>", c + 1, (w - 5.0).max(0.0) / 7.0, if sh.hidden_cols.contains(&c) { " hidden=\"1\"" } else { "" }, sh.col_style.get(&c).map_or(String::new(), |s| format!(" style=\"{s}\"")));
        }
        x.push_str("</cols>");
    }
    x.push_str("<sheetData>");
    let mut rows: Vec<u32> = sh.rows.keys().chain(sh.row_h.keys()).chain(sh.hidden_rows.iter()).chain(sh.filtered_rows.iter()).chain(sh.row_style.keys()).copied().collect();
    rows.sort();
    rows.dedup();
    for r in rows {
        let _ = write!(x, "<row r=\"{}\"", r + 1);
        if let Some(h) = sh.row_h.get(&r) {
            let _ = write!(x, " ht=\"{}\" customHeight=\"{}\"", h * 0.75, sh.custom_h.contains(&r) as u8);
        }
        if sh.row_hidden(r) {
            x.push_str(" hidden=\"1\"");
        }
        if let Some(s) = sh.row_style.get(&r) {
            let _ = write!(x, " s=\"{s}\" customFormat=\"1\"");
        }
        x.push('>');
        if let Some(m) = sh.rows.get(&r) {
            for (&c, cell) in m.iter() {
                let a = cell_name(r, c);
                let _ = write!(x, "<c r=\"{a}\" s=\"{}\"", cell.style);
                match &cell.v {
                    Content::Empty => x.push_str("/>"),
                    Content::Num(n) => {
                        let _ = write!(x, "><v>{n}</v></c>");
                    }
                    Content::Bool(b) => {
                        let _ = write!(x, " t=\"b\"><v>{}</v></c>", *b as u8);
                    }
                    Content::Err(e) => {
                        let _ = write!(x, " t=\"e\"><v>{}</v></c>", esc(e.text()));
                    }
                    Content::Text(t) => {
                        let i = *map.entry(t.to_string()).or_insert_with(|| {
                            strings.push(t.to_string());
                            strings.len() - 1
                        });
                        let _ = write!(x, " t=\"s\"><v>{i}</v></c>");
                    }
                    Content::Formula(e) => {
                        let f = esc(&formula_for_file(e));
                        let spilled = calc.spill_rects.get(&(si as u32, r, c)).filter(|x| !x.is_single());
                        let open = match spilled {
                            Some(rc) => format!("<f t=\"array\" ref=\"{}\">", rc.a1()),
                            None => "<f>".to_string(),
                        };
                        match calc.value(wb, si, r, c) {
                            Value::Num(n) => {
                                let _ = write!(x, ">{open}{f}</f><v>{n}</v></c>");
                            }
                            Value::Text(t) => {
                                let _ = write!(x, " t=\"str\">{open}{f}</f><v>{}</v></c>", esc(&t));
                            }
                            Value::Bool(b) => {
                                let _ = write!(x, " t=\"b\">{open}{f}</f><v>{}</v></c>", b as u8);
                            }
                            Value::Err(e) => {
                                let _ = write!(x, " t=\"e\">{open}{f}</f><v>{}</v></c>", esc(e.text()));
                            }
                            _ => {
                                let _ = write!(x, ">{open}{f}</f></c>");
                            }
                        }
                    }
                }
            }
        }
        x.push_str("</row>");
    }
    x.push_str("</sheetData>");
    if let Some(p) = &sh.protection {
        let _ = write!(x, "<sheetProtection sheet=\"1\" objects=\"1\" scenarios=\"1\"{}/>", if p.select_locked { "" } else { " selectLockedCells=\"1\"" });
    }
    let table_ranges: Vec<Rect> = sh.tables.iter().map(|t| t.range).collect();
    if let Some(f) = &sh.filter {
        if !table_ranges.contains(&f.range) {
            let _ = write!(x, "<autoFilter ref=\"{}\">", f.range.a1());
            for (k, cf) in &f.cols {
                if let Some(a) = &cf.allowed {
                    let _ = write!(x, "<filterColumn colId=\"{k}\"><filters>");
                    for v in a {
                        let _ = write!(x, "<filter val=\"{}\"/>", esc(v));
                    }
                    x.push_str("</filters></filterColumn>");
                }
            }
            x.push_str("</autoFilter>");
        }
    }
    if !sh.merges.is_empty() {
        let _ = write!(x, "<mergeCells count=\"{}\">", sh.merges.len());
        for m in &sh.merges {
            let _ = write!(x, "<mergeCell ref=\"{}\"/>", m.a1());
        }
        x.push_str("</mergeCells>");
    }
    for c in &sh.cond {
        x.push_str(&cond_xml(c, prio, dxf));
    }
    if !sh.valid.is_empty() {
        let _ = write!(x, "<dataValidations count=\"{}\">", sh.valid.len());
        for v in &sh.valid {
            x.push_str(&valid_xml(v));
        }
        x.push_str("</dataValidations>");
    }
    if !sh.links.is_empty() {
        x.push_str("<hyperlinks>");
        for (p, t) in &sh.links {
            let a = cell_name(p.row, p.col);
            if let Some(l) = t.strip_prefix('#') {
                let _ = write!(x, "<hyperlink ref=\"{a}\" location=\"{}\"/>", esc(l));
            } else {
                rels.push(format!("<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>", rels.len() + 1, esc(t)));
                let _ = write!(x, "<hyperlink ref=\"{a}\" r:id=\"rId{}\"/>", rels.len());
            }
        }
        x.push_str("</hyperlinks>");
    }
    let ps = &sh.page;
    let _ = write!(x, "<pageMargins left=\"{}\" right=\"{}\" top=\"{}\" bottom=\"{}\" header=\"0.3\" footer=\"0.3\"/>", ps.margins[0], ps.margins[1], ps.margins[2], ps.margins[3]);
    let paper = [1, 9, 5, 8][ps.paper.min(3) as usize];
    let _ = write!(x, "<pageSetup paperSize=\"{paper}\" scale=\"{}\" orientation=\"{}\"/>", ps.scale, if ps.landscape { "landscape" } else { "portrait" });
    x
}

/// Saves the workbook as .xlsx bytes.
pub fn save(wb: &Workbook, calc: &CalcResult) -> Result<Vec<u8>, String> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opt = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut put = |z: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: &str, body: &str| -> Result<(), String> {
        z.start_file(name, opt).map_err(|e| e.to_string())?;
        z.write_all(body.as_bytes()).map_err(|e| e.to_string())
    };
    let mut strings = vec![];
    let mut map = HashMap::new();
    let mut prio = 0;
    let mut dxf = vec![];
    let mut types = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"vml\" ContentType=\"application/vnd.openxmlformats-officedocument.vmlDrawing\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>");
    let mut wb_rels = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_REL}\">");
    let mut sheets_xml = String::new();
    let (mut chart_no, mut drawing_no, mut table_no) = (0, 0, 0);
    for si in 0..wb.sheets.len() {
        let sh = &wb.sheets[si];
        let mut rels: Vec<String> = vec![];
        let mut x = sheet_xml(wb, calc, si, &mut strings, &mut map, &mut prio, &mut dxf, &mut rels);
        if !sh.charts.is_empty() {
            drawing_no += 1;
            let mut d = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">");
            let mut dr = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_REL}\">");
            for (i, ch) in sh.charts.iter().enumerate() {
                chart_no += 1;
                put(&mut z, &format!("xl/charts/chart{chart_no}.xml"), &chart_xml(ch))?;
                let _ = write!(types, "<Override PartName=\"/xl/charts/chart{chart_no}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawingml.chart+xml\"/>");
                let _ = write!(dr, "<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/chart\" Target=\"../charts/chart{chart_no}.xml\"/>", i + 1);
                let (ex, ey, ew, eh) = ((ch.x * 9525.0) as i64, (ch.y * 9525.0) as i64, (ch.w * 9525.0) as i64, (ch.h * 9525.0) as i64);
                let _ = write!(d, "<xdr:absoluteAnchor><xdr:pos x=\"{ex}\" y=\"{ey}\"/><xdr:ext cx=\"{ew}\" cy=\"{eh}\"/><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{}\" name=\"Chart {}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart r:id=\"rId{}\"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:absoluteAnchor>", i + 2, i + 1, i + 1);
            }
            d.push_str("</xdr:wsDr>");
            dr.push_str("</Relationships>");
            put(&mut z, &format!("xl/drawings/drawing{drawing_no}.xml"), &d)?;
            put(&mut z, &format!("xl/drawings/_rels/drawing{drawing_no}.xml.rels"), &dr)?;
            let _ = write!(types, "<Override PartName=\"/xl/drawings/drawing{drawing_no}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawing+xml\"/>");
            rels.push(format!("<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/drawing\" Target=\"../drawings/drawing{drawing_no}.xml\"/>", rels.len() + 1));
            let _ = write!(x, "<drawing r:id=\"rId{}\"/>", rels.len());
        }
        if !sh.notes.is_empty() {
            let n = si + 1;
            let mut c = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<comments xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><authors>");
            let mut authors: Vec<&str> = sh.notes.values().map(|x| x.author.as_str()).collect();
            authors.sort();
            authors.dedup();
            for a in &authors {
                let _ = write!(c, "<author>{}</author>", esc(a));
            }
            c.push_str("</authors><commentList>");
            let mut v = String::from("<xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\"><o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"1\"/></o:shapelayout><v:shapetype id=\"_x0000_t202\" coordsize=\"21600,21600\" o:spt=\"202\" path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/><v:path gradientshapeok=\"t\" o:connecttype=\"rect\"/></v:shapetype>");
            for (i, (p, note)) in sh.notes.iter().enumerate() {
                let aid = authors.iter().position(|a| *a == note.author).unwrap_or(0);
                let _ = write!(c, "<comment ref=\"{}\" authorId=\"{aid}\"><text><r><t xml:space=\"preserve\">{}</t></r></text></comment>", cell_name(p.row, p.col), esc(&note.text));
                let _ = write!(v, "<v:shape id=\"_x0000_s{}\" type=\"#_x0000_t202\" style=\"position:absolute;visibility:hidden\" fillcolor=\"#ffffe1\"><x:ClientData ObjectType=\"Note\"><x:Row>{}</x:Row><x:Column>{}</x:Column></x:ClientData></v:shape>", 1025 + i, p.row, p.col);
            }
            c.push_str("</commentList></comments>");
            v.push_str("</xml>");
            put(&mut z, &format!("xl/comments{n}.xml"), &c)?;
            put(&mut z, &format!("xl/drawings/vmlDrawing{n}.vml"), &v)?;
            let _ = write!(types, "<Override PartName=\"/xl/comments{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml\"/>");
            rels.push(format!("<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/comments\" Target=\"../comments{n}.xml\"/>", rels.len() + 1));
            rels.push(format!("<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/vmlDrawing\" Target=\"../drawings/vmlDrawing{n}.vml\"/>", rels.len() + 1));
            let _ = write!(x, "<legacyDrawing r:id=\"rId{}\"/>", rels.len());
        }
        if !sh.tables.is_empty() {
            let _ = write!(x, "<tableParts count=\"{}\">", sh.tables.len());
            for t in &sh.tables {
                table_no += 1;
                let mut tx = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<table xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" id=\"{table_no}\" name=\"{0}\" displayName=\"{0}\" ref=\"{1}\" totalsRowCount=\"{2}\"><autoFilter ref=\"{1}\"/><tableColumns count=\"{3}\">", esc(&t.name), t.range.a1(), t.totals as u8, t.columns.len());
                for (i, c) in t.columns.iter().enumerate() {
                    let _ = write!(tx, "<tableColumn id=\"{}\" name=\"{}\"/>", i + 1, esc(c));
                }
                let _ = write!(tx, "</tableColumns><tableStyleInfo name=\"TableStyleMedium{}\" showRowStripes=\"{}\"/></table>", t.style, t.banded as u8);
                put(&mut z, &format!("xl/tables/table{table_no}.xml"), &tx)?;
                let _ = write!(types, "<Override PartName=\"/xl/tables/table{table_no}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml\"/>");
                rels.push(format!("<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/table\" Target=\"../tables/table{table_no}.xml\"/>", rels.len() + 1));
                let _ = write!(x, "<tablePart r:id=\"rId{}\"/>", rels.len());
            }
            x.push_str("</tableParts>");
        }
        x.push_str("</worksheet>");
        let n = si + 1;
        put(&mut z, &format!("xl/worksheets/sheet{n}.xml"), &x)?;
        if !rels.is_empty() {
            put(&mut z, &format!("xl/worksheets/_rels/sheet{n}.xml.rels"), &format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_REL}\">{}</Relationships>", rels.concat()))?;
        }
        let _ = write!(types, "<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>");
        let _ = write!(wb_rels, "<Relationship Id=\"rId{n}\" Type=\"{OFFICE_REL}/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>");
        let _ = write!(sheets_xml, "<sheet name=\"{}\" sheetId=\"{n}\"{} r:id=\"rId{n}\"/>", esc(&sh.name), if sh.hidden { " state=\"hidden\"" } else { "" });
    }
    let k = wb.sheets.len();
    let _ = write!(wb_rels, "<Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId{}\" Type=\"{OFFICE_REL}/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>", k + 1, k + 2);
    types.push_str("</Types>");
    let mut names = String::new();
    let print_areas: Vec<(usize, Rect)> = wb.sheets.iter().enumerate().filter_map(|(i, s)| s.page.print_area.map(|r| (i, r))).collect();
    if !wb.names.is_empty() || !print_areas.is_empty() {
        names.push_str("<definedNames>");
        for n in &wb.names {
            let _ = write!(names, "<definedName name=\"{}\">{}</definedName>", esc(&n.name), esc(n.expr.trim_start_matches('=')));
        }
        for (i, r) in print_areas {
            let _ = write!(names, "<definedName name=\"_xlnm.Print_Area\" localSheetId=\"{i}\">{}!{}</definedName>", esc(&formula::quote_sheet(&wb.sheets[i].name)), r.a1_abs());
        }
        names.push_str("</definedNames>");
    }
    put(&mut z, "[Content_Types].xml", &types)?;
    put(&mut z, "_rels/.rels", &format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_REL}\"><Relationship Id=\"rId1\" Type=\"{OFFICE_REL}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"))?;
    put(&mut z, "xl/workbook.xml", &format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"{OFFICE_REL}\"><bookViews><workbookView/></bookViews><sheets>{sheets_xml}</sheets>{names}<calcPr fullCalcOnLoad=\"1\"/></workbook>"))?;
    put(&mut z, "xl/_rels/workbook.xml.rels", &wb_rels)?;
    put(&mut z, "xl/styles.xml", &styles_xml(wb, &dxf))?;
    let mut ss = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{0}\" uniqueCount=\"{0}\">", strings.len());
    for s in &strings {
        let _ = write!(ss, "<si><t xml:space=\"preserve\">{}</t></si>", esc(s));
    }
    ss.push_str("</sst>");
    put(&mut z, "xl/sharedStrings.xml", &ss)?;
    let cur = z.finish().map_err(|e| e.to_string())?;
    Ok(cur.into_inner())
}
