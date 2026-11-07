//! OpenDocument Spreadsheet (.ods) reading and writing, plus delimited text.

use crate::eval::*;
use crate::formula::{self, Expr};
use crate::model::*;
use crate::numfmt;
use crate::ops;
use crate::xml::{self, esc, Node};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;
use zip::write::SimpleFileOptions;

/// Converts ODF formula text ("of:=SUM([.A1:.B2];[Sheet2.C3])") to A1 form.
fn from_odf(s: &str) -> String {
    let body = s.strip_prefix("of:").or(s.strip_prefix("oooc:")).unwrap_or(s);
    let body = body.strip_prefix('=').unwrap_or(body);
    let mut out = String::new();
    let ch: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < ch.len() {
        match ch[i] {
            '"' => {
                out.push('"');
                i += 1;
                while i < ch.len() {
                    out.push(ch[i]);
                    if ch[i] == '"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            '[' => {
                let st = i + 1;
                while i < ch.len() && ch[i] != ']' {
                    i += 1;
                }
                let inner: String = ch[st..i.min(ch.len())].iter().collect();
                i += 1;
                let mut parts = inner.split(':');
                let a = parts.next().unwrap_or("");
                let b = parts.next();
                let conv = |p: &str, first: bool, o: &mut String| {
                    let p = p.trim_start_matches('$');
                    if let Some((sheet, cell)) = p.rsplit_once('.') {
                        if !sheet.is_empty() && first {
                            o.push_str(&formula::quote_sheet(sheet.trim_matches('\'')));
                            o.push('!');
                        }
                        o.push_str(cell);
                    } else {
                        o.push_str(p);
                    }
                };
                conv(a, true, &mut out);
                if let Some(b) = b {
                    out.push(':');
                    conv(b, false, &mut out);
                }
            }
            ';' => {
                out.push(',');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Prints an expression as ODF formula text.
fn to_odf(e: &Expr) -> String {
    let text = formula::print(e);
    let spans = formula::find_refs(&text);
    let ch: Vec<char> = text.chars().collect();
    let mut out = String::from("of:=");
    let mut i = 0;
    let piece = |s: &str| -> String {
        // "Sheet!A1:B2" into "[$Sheet.A1:.B2]".
        let (sheet, r) = match s.rsplit_once('!') {
            Some((a, b)) => (format!("${}", a.trim_matches('\'')), b),
            None => (String::new(), s),
        };
        let mut p = r.split(':');
        let a = p.next().unwrap_or("");
        match p.next() {
            Some(b) => format!("[{sheet}.{a}:.{b}]"),
            None => format!("[{sheet}.{a}]"),
        }
    };
    let mut in_str = false;
    while i < ch.len() {
        if !in_str {
            if let Some(sp) = spans.iter().find(|s| s.0 == i) {
                out.push_str(&piece(&sp.2));
                i = sp.1;
                continue;
            }
        }
        let c = ch[i];
        if c == '"' {
            in_str = !in_str;
        }
        out.push(if c == ',' && !in_str { ';' } else { c });
        i += 1;
    }
    out
}

fn parse_color(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches('#'), 16).ok()
}

fn cell_style_from(n: &Node) -> Style {
    let mut s = Style::default();
    if let Some(t) = n.child("text-properties") {
        if t.attr("font-weight") == Some("bold") {
            s.bold = true;
        }
        if t.attr("font-style") == Some("italic") {
            s.italic = true;
        }
        if let Some(c) = t.attr("color").and_then(parse_color) {
            if c != 0 {
                s.color = Some(c);
            }
        }
        if let Some(z) = t.attr("font-size").and_then(|x| x.trim_end_matches("pt").parse::<f32>().ok()) {
            s.size10 = (z * 10.0).round() as u16;
        }
        if let Some(f) = t.attr("font-name") {
            s.font = f.into();
        }
        if t.attr("text-underline-style").is_some_and(|x| x != "none") {
            s.underline = 1;
        }
        if t.attr("text-line-through-style").is_some_and(|x| x != "none") {
            s.strike = true;
        }
    }
    if let Some(p) = n.child("paragraph-properties") {
        s.h = match p.attr("text-align") {
            Some("center") => HAlign::Center,
            Some("end") | Some("right") => HAlign::Right,
            Some("start") | Some("left") => HAlign::Left,
            Some("justify") => HAlign::Justify,
            _ => HAlign::General,
        };
    }
    if let Some(c) = n.child("table-cell-properties") {
        if let Some(col) = c.attr("background-color").filter(|x| *x != "transparent").and_then(parse_color) {
            s.fill = Some(col);
        }
        s.wrap = c.attr("wrap-option") == Some("wrap");
        s.v = match c.attr("vertical-align") {
            Some("top") => VAlign::Top,
            Some("middle") => VAlign::Center,
            _ => VAlign::Bottom,
        };
        let border = |v: Option<&str>| -> Option<Border> {
            let v = v?;
            if v == "none" {
                return None;
            }
            let color = v.split_whitespace().find_map(|p| p.strip_prefix('#')).and_then(parse_color).unwrap_or(0);
            Some(Border { style: if v.contains("double") { LineStyle::Double } else { LineStyle::Thin }, color })
        };
        let all = border(c.attr("border"));
        s.borders[B_LEFT] = border(c.attr("border-left")).or(all);
        s.borders[B_RIGHT] = border(c.attr("border-right")).or(all);
        s.borders[B_TOP] = border(c.attr("border-top")).or(all);
        s.borders[B_BOTTOM] = border(c.attr("border-bottom")).or(all);
    }
    s
}

pub fn load(data: Vec<u8>) -> Result<Workbook, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| format!("Not a spreadsheet file: {e}"))?;
    let mut s = String::new();
    z.by_name("content.xml").map_err(|e| e.to_string())?.read_to_string(&mut s).map_err(|e| e.to_string())?;
    let root = xml::parse(&s)?;
    let mut wb = Workbook { sheets: vec![], ..Default::default() };
    let mut styles: HashMap<String, u32> = HashMap::new();
    let mut col_px: HashMap<String, f32> = HashMap::new();
    let mut row_px: HashMap<String, f32> = HashMap::new();
    let length = |v: &str| -> Option<f32> {
        if let Some(x) = v.strip_suffix("in") {
            return x.parse::<f32>().ok().map(|i| i * 96.0);
        }
        if let Some(x) = v.strip_suffix("cm") {
            return x.parse::<f32>().ok().map(|i| i * 96.0 / 2.54);
        }
        if let Some(x) = v.strip_suffix("mm") {
            return x.parse::<f32>().ok().map(|i| i * 96.0 / 25.4);
        }
        v.strip_suffix("pt").and_then(|x| x.parse::<f32>().ok()).map(|i| i * 96.0 / 72.0)
    };
    if let Some(a) = root.child("automatic-styles") {
        for st in a.children("style") {
            let Some(name) = st.attr("name") else { continue };
            match st.attr("family") {
                Some("table-cell") => {
                    let id = wb.styles.intern(cell_style_from(st));
                    styles.insert(name.to_string(), id);
                }
                Some("table-column") => {
                    if let Some(w) = st.child("table-column-properties").and_then(|p| p.attr("column-width")).and_then(length) {
                        col_px.insert(name.to_string(), w);
                    }
                }
                Some("table-row") => {
                    if let Some(w) = st.child("table-row-properties").and_then(|p| p.attr("row-height")).and_then(length) {
                        row_px.insert(name.to_string(), w);
                    }
                }
                _ => {}
            }
        }
    }
    let body = root.find("spreadsheet").ok_or("This file has no spreadsheet body")?;
    for t in body.children("table") {
        let mut sh = Sheet::new(t.attr("name").unwrap_or("Sheet"));
        let mut c = 0u32;
        for col in t.children("table-column") {
            let k = col.attr_u("number-columns-repeated").unwrap_or(1);
            if k < 1000 {
                if let Some(w) = col.attr("style-name").and_then(|x| col_px.get(x)) {
                    for i in c..c + k {
                        sh.col_w.insert(i, *w);
                    }
                }
            }
            c += k;
        }
        let mut r = 0u32;
        let mut rows: Vec<&Node> = vec![];
        for k in &t.kids {
            match k.local() {
                "table-row" => rows.push(k),
                "table-header-rows" | "table-row-group" | "table-rows" => rows.extend(k.children("table-row")),
                _ => {}
            }
        }
        for row in rows {
            let rep = row.attr_u("number-rows-repeated").unwrap_or(1);
            let has = row.kids.iter().any(|x| x.attr("value-type").is_some() || x.attr("formula").is_some() || x.attr("number-columns-spanned").is_some() || x.attr("style-name").is_some_and(|s| styles.get(s).is_some_and(|i| *i != 0)));
            if has && rep < 2000 {
                for rr in 0..rep {
                    let mut c = 0u32;
                    for cell in row.kids.iter().filter(|x| x.local().ends_with("table-cell")) {
                        let k = cell.attr_u("number-columns-repeated").unwrap_or(1);
                        let style = cell.attr("style-name").and_then(|x| styles.get(x)).copied().unwrap_or(0);
                        let ty = cell.attr("value-type").unwrap_or("");
                        let text: String = cell.children("p").map(|p| p.all_text()).collect::<Vec<_>>().join("\n");
                        let mut content = match ty {
                            "float" | "percentage" | "currency" => cell.attr_f("value").map(Content::Num).unwrap_or_default(),
                            "date" => cell.attr("date-value").and_then(parse_iso).map(Content::Num).unwrap_or_default(),
                            "time" => cell.attr("time-value").and_then(parse_iso_time).map(Content::Num).unwrap_or_default(),
                            "boolean" => Content::Bool(cell.attr("boolean-value") == Some("true")),
                            "string" => Content::Text(text.into()),
                            _ => Content::Empty,
                        };
                        if let Some(f) = cell.attr("formula") {
                            if let Ok(e) = formula::parse(&from_odf(f)) {
                                content = Content::Formula(Arc::new(e));
                            }
                        }
                        let code = match ty {
                            "percentage" => Some("0.00%"),
                            "date" => Some("m/d/yyyy"),
                            "time" => Some("h:mm:ss"),
                            "currency" => Some("$#,##0.00"),
                            _ => None,
                        };
                        let style = match code {
                            Some(cd) => {
                                let mut st = (**wb.styles.get(style)).clone();
                                st.num_fmt = cd.into();
                                wb.styles.intern(st)
                            }
                            None => style,
                        };
                        if k < 2000 && (content != Content::Empty || style != 0) {
                            for i in 0..k {
                                let x = sh.cell_mut(r + rr, c + i);
                                x.v = content.clone();
                                x.style = style;
                            }
                        }
                        let (cs, rs) = (cell.attr_u("number-columns-spanned").unwrap_or(1), cell.attr_u("number-rows-spanned").unwrap_or(1));
                        if cs > 1 || rs > 1 {
                            sh.merges.push(Rect::new(r + rr, c, r + rr + rs - 1, c + cs - 1));
                        }
                        c += k;
                    }
                    if let Some(h) = row.attr("style-name").and_then(|x| row_px.get(x)) {
                        sh.row_h.insert(r + rr, *h);
                    }
                }
            }
            r += rep;
        }
        wb.sheets.push(Arc::new(sh));
    }
    if wb.sheets.is_empty() {
        return Err("The file has no sheets".into());
    }
    Ok(wb)
}

fn parse_iso(s: &str) -> Option<f64> {
    let (d, t) = s.split_once('T').unwrap_or((s, ""));
    let p: Vec<i64> = d.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 3 {
        return None;
    }
    let mut v = numfmt::ymd_to_serial(p[0], p[1], p[2]);
    if !t.is_empty() {
        let q: Vec<f64> = t.split(':').filter_map(|x| x.parse().ok()).collect();
        if q.len() >= 2 {
            v += (q[0] * 3600.0 + q[1] * 60.0 + q.get(2).copied().unwrap_or(0.0)) / 86400.0;
        }
    }
    Some(v)
}

fn parse_iso_time(s: &str) -> Option<f64> {
    // PT12H30M15S
    let b = s.strip_prefix("PT")?;
    let (h, b) = b.split_once('H')?;
    let (m, b) = b.split_once('M')?;
    let sec = b.trim_end_matches('S').parse::<f64>().unwrap_or(0.0);
    Some((h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + sec) / 86400.0)
}

fn cell_attrs(st: &Style) -> String {
    let code: &str = &st.num_fmt;
    if code.contains('%') {
        return "ce-pct".into();
    }
    String::new()
}

fn style_xml(i: usize, st: &Style) -> String {
    let mut x = format!("<style:style style:name=\"ce{i}\" style:family=\"table-cell\" style:parent-style-name=\"Default\"");
    if numfmt::is_date_format(&st.num_fmt) {
        x.push_str(" style:data-style-name=\"Ndate\"");
    } else if st.num_fmt.contains('%') {
        x.push_str(" style:data-style-name=\"Npct\"");
    } else if st.num_fmt.contains('$') {
        x.push_str(" style:data-style-name=\"Ncur\"");
    } else if st.num_fmt.contains("0.00") {
        x.push_str(" style:data-style-name=\"Ndec\"");
    }
    x.push('>');
    let _ = write!(x, "<style:table-cell-properties style:vertical-align=\"{}\"", match st.v { VAlign::Top => "top", VAlign::Center => "middle", VAlign::Bottom => "automatic" });
    if let Some(c) = st.fill {
        let _ = write!(x, " fo:background-color=\"#{:06x}\"", c);
    }
    if st.wrap {
        x.push_str(" fo:wrap-option=\"wrap\"");
    }
    for (k, n) in ["left", "right", "top", "bottom"].iter().enumerate() {
        if let Some(b) = &st.borders[k] {
            let w = match b.style { LineStyle::Medium => "0.74pt", LineStyle::Thick => "1.5pt", _ => "0.35pt" };
            let _ = write!(x, " fo:border-{n}=\"{w} {} #{:06x}\"", if b.style == LineStyle::Double { "double" } else { "solid" }, b.color);
        }
    }
    x.push_str("/>");
    if st.h != HAlign::General {
        let a = match st.h { HAlign::Center | HAlign::CenterAcross => "center", HAlign::Right => "end", HAlign::Justify => "justify", _ => "start" };
        let _ = write!(x, "<style:paragraph-properties fo:text-align=\"{a}\"/>");
    }
    let _ = write!(x, "<style:text-properties style:font-name=\"{0}\" fo:font-family=\"{0}\" fo:font-size=\"{1}pt\"", esc(&st.font), st.size10 as f32 / 10.0);
    if st.bold {
        x.push_str(" fo:font-weight=\"bold\"");
    }
    if st.italic {
        x.push_str(" fo:font-style=\"italic\"");
    }
    if let Some(c) = st.color {
        let _ = write!(x, " fo:color=\"#{:06x}\"", c);
    }
    if st.underline > 0 {
        x.push_str(" style:text-underline-style=\"solid\"");
    }
    if st.strike {
        x.push_str(" style:text-line-through-style=\"solid\"");
    }
    x.push_str("/></style:style>");
    x
}

fn iso_date(n: f64) -> String {
    let (y, m, d) = numfmt::serial_to_ymd(n);
    let secs = (n.fract() * 86400.0).round() as i64;
    if secs == 0 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
    }
}

pub fn save(wb: &Workbook, calc: &CalcResult) -> Result<Vec<u8>, String> {
    let mut body = String::new();
    let mut autos = String::new();
    for (i, st) in wb.styles.list.iter().enumerate() {
        autos.push_str(&style_xml(i, st));
    }
    let mut widths: Vec<u32> = vec![];
    let mut heights: Vec<u32> = vec![];
    let wid = |list: &mut Vec<u32>, px: f32| -> usize {
        let k = px.round() as u32;
        list.iter().position(|x| *x == k).unwrap_or_else(|| {
            list.push(k);
            list.len() - 1
        })
    };
    let mut sheets = String::new();
    for (si, sh) in wb.sheets.iter().enumerate() {
        let _ = write!(sheets, "<table:table table:name=\"{}\">", esc(&sh.name));
        let used = sh.merges.iter().fold(sh.used_rect().unwrap_or(Rect::new(0, 0, 0, 0)), |u, m| u.union(m));
        for c in 0..=used.c2 {
            let w = wid(&mut widths, sh.col_w.get(&c).copied().unwrap_or(DEFAULT_COL_W));
            let _ = write!(sheets, "<table:table-column table:style-name=\"co{w}\"/>");
        }
        let mut r = 0;
        while r <= used.r2 {
            let h = wid(&mut heights, sh.row_h.get(&r).copied().unwrap_or(DEFAULT_ROW_H));
            let _ = write!(sheets, "<table:table-row table:style-name=\"ro{h}\"{}>", if sh.row_hidden(r) { " table:visibility=\"collapse\"" } else { "" });
            let mut c = 0;
            while c <= used.c2 {
                if sh.merges.iter().any(|m| m.contains(Pos::new(r, c)) && (m.r1, m.c1) != (r, c)) {
                    sheets.push_str("<table:covered-table-cell/>");
                    c += 1;
                    continue;
                }
                let span = sh.merges.iter().find(|m| (m.r1, m.c1) == (r, c)).map_or(String::new(), |m| format!(" table:number-columns-spanned=\"{}\" table:number-rows-spanned=\"{}\"", m.cols(), m.rows()));
                let style = sh.style_id(r, c);
                let st = wb.styles.get(style);
                let sty = format!(" table:style-name=\"ce{style}\"");
                let cell = sh.get(r, c);
                let value = calc.value(wb, si, r, c);
                let form = match cell.map(|x| &x.v) {
                    Some(Content::Formula(e)) => format!(" table:formula=\"{}\"", esc(&to_odf(e))),
                    _ => String::new(),
                };
                let shown = esc(&ops::text_of(wb, calc, si, r, c));
                match value {
                    Value::Num(n) => {
                        if numfmt::is_date_format(&st.num_fmt) {
                            let _ = write!(sheets, "<table:table-cell{sty}{span}{form} office:value-type=\"date\" office:date-value=\"{}\"><text:p>{shown}</text:p></table:table-cell>", iso_date(n));
                        } else if !cell_attrs(st).is_empty() {
                            let _ = write!(sheets, "<table:table-cell{sty}{span}{form} office:value-type=\"percentage\" office:value=\"{n}\"><text:p>{shown}</text:p></table:table-cell>");
                        } else {
                            let _ = write!(sheets, "<table:table-cell{sty}{span}{form} office:value-type=\"float\" office:value=\"{n}\"><text:p>{shown}</text:p></table:table-cell>");
                        }
                    }
                    Value::Text(_) | Value::Err(_) => {
                        let _ = write!(sheets, "<table:table-cell{sty}{span}{form} office:value-type=\"string\"><text:p>{shown}</text:p></table:table-cell>");
                    }
                    Value::Bool(b) => {
                        let _ = write!(sheets, "<table:table-cell{sty}{span}{form} office:value-type=\"boolean\" office:boolean-value=\"{b}\"><text:p>{shown}</text:p></table:table-cell>");
                    }
                    _ => {
                        let _ = write!(sheets, "<table:table-cell{sty}{span}{form}/>");
                    }
                }
                c += 1;
            }
            sheets.push_str("</table:table-row>");
            r += 1;
        }
        sheets.push_str("</table:table>");
    }
    for (i, w) in widths.iter().enumerate() {
        let _ = write!(autos, "<style:style style:name=\"co{i}\" style:family=\"table-column\"><style:table-column-properties style:column-width=\"{:.3}in\"/></style:style>", *w as f32 / 96.0);
    }
    for (i, w) in heights.iter().enumerate() {
        let _ = write!(autos, "<style:style style:name=\"ro{i}\" style:family=\"table-row\"><style:table-row-properties style:row-height=\"{:.3}in\"/></style:style>", *w as f32 / 96.0);
    }
    let data_styles = "<number:date-style style:name=\"Ndate\"><number:month/><number:text>/</number:text><number:day/><number:text>/</number:text><number:year number:style=\"long\"/></number:date-style><number:percentage-style style:name=\"Npct\"><number:number number:decimal-places=\"2\" number:min-integer-digits=\"1\"/><number:text>%</number:text></number:percentage-style><number:number-style style:name=\"Ndec\"><number:number number:decimal-places=\"2\" number:min-integer-digits=\"1\" number:grouping=\"true\"/></number:number-style><number:currency-style style:name=\"Ncur\"><number:text>$</number:text><number:number number:decimal-places=\"2\" number:min-integer-digits=\"1\" number:grouping=\"true\"/></number:currency-style>";
    let _ = write!(body, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" xmlns:number=\"urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0\" xmlns:of=\"urn:oasis:names:tc:opendocument:xmlns:of:1.2\" office:version=\"1.2\"><office:automatic-styles>{data_styles}{autos}</office:automatic-styles><office:body><office:spreadsheet>{sheets}</office:spreadsheet></office:body></office:document-content>");
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let opt = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let e = |e: zip::result::ZipError| e.to_string();
    z.start_file("mimetype", stored).map_err(e)?;
    z.write_all(b"application/vnd.oasis.opendocument.spreadsheet").map_err(|e| e.to_string())?;
    z.start_file("content.xml", opt).map_err(e)?;
    z.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    z.start_file("META-INF/manifest.xml", opt).map_err(e)?;
    z.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.2\"><manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.2\" manifest:media-type=\"application/vnd.oasis.opendocument.spreadsheet\"/><manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/></manifest:manifest>").map_err(|e| e.to_string())?;
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}

/// Reads delimited text into a sheet.
pub fn load_delimited(text: &str, name: &str) -> Workbook {
    let t = text.trim_start_matches('\u{feff}');
    let head = t.lines().next().unwrap_or("");
    let delim = [',', '\t', ';', '|'].into_iter().max_by_key(|d| head.matches(*d).count()).unwrap_or(',');
    let mut wb = Workbook::default();
    let mut sh = Sheet::new(name);
    let (mut r, mut c) = (0u32, 0u32);
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = t.chars().peekable();
    let mut was_quoted = false;
    let flush = |sh: &mut Sheet, wb: &mut Workbook, r: u32, c: u32, s: &mut String, q: bool| {
        if !s.is_empty() {
            let typed = if q { numfmt::Typed { content: Content::Text(s.as_str().into()), fmt: None } } else { numfmt::parse_input(s) };
            let cell = sh.cell_mut(r, c);
            cell.v = typed.content;
            if let Some(f) = typed.fmt {
                cell.style = wb.styles.intern(Style { num_fmt: f.into(), ..Style::default() });
            }
        }
        s.clear();
    };
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' && cur.is_empty() {
            quoted = true;
            was_quoted = true;
        } else if ch == delim {
            flush(&mut sh, &mut wb, r, c, &mut cur, was_quoted);
            was_quoted = false;
            c += 1;
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            flush(&mut sh, &mut wb, r, c, &mut cur, was_quoted);
            was_quoted = false;
            r += 1;
            c = 0;
        } else {
            cur.push(ch);
        }
    }
    flush(&mut sh, &mut wb, r, c, &mut cur, was_quoted);
    wb.sheets = vec![Arc::new(sh)];
    wb
}

pub fn save_delimited(wb: &Workbook, calc: &CalcResult, si: usize, delim: char) -> String {
    let sh = &wb.sheets[si];
    let Some(u) = sh.used_content_rect() else { return String::new() };
    let mut out = String::new();
    for r in 0..=u.r2 {
        for c in 0..=u.c2 {
            if c > 0 {
                out.push(delim);
            }
            let t = ops::text_of(wb, calc, si, r, c);
            if t.contains(delim) || t.contains('"') || t.contains('\n') {
                out.push('"');
                out.push_str(&t.replace('"', "\"\""));
                out.push('"');
            } else {
                out.push_str(&t);
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odf_formula() {
        assert_eq!(from_odf("of:=SUM([.A1:.B2];[Sheet2.C3])"), "SUM(A1:B2,Sheet2!C3)");
        let e = formula::parse("SUM(A1:B2,Sheet2!C3)").unwrap();
        assert_eq!(to_odf(&e), "of:=SUM([.A1:.B2];[$Sheet2.C3])");
    }
}
