//! Checks of the calculation engine and file formats on whole workbooks.

use crate::eval::*;
use crate::model::*;
use crate::{ods, ops, xlsx_read, xlsx_write};
use std::sync::Arc;

fn book(cells: &[(&str, &str)]) -> Workbook {
    let mut wb = Workbook::default();
    for (a, v) in cells {
        let p = parse_cell(a).unwrap();
        ops::set_input(&mut wb, 0, p.row, p.col, v).unwrap();
    }
    wb
}

fn shown(wb: &Workbook, a: &str) -> String {
    let calc = recalc(wb);
    let p = parse_cell(a).unwrap();
    ops::text_of(wb, &calc, 0, p.row, p.col)
}

#[test]
fn formulas_give_expected_values() {
    let wb = book(&[
        ("A1", "1"), ("A2", "2"), ("A3", "3"), ("A4", "4"),
        ("B1", "apple"), ("B2", "banana"), ("B3", "cherry"), ("B4", "apple"),
        ("C1", "=SUM(A1:A4)"), ("C2", "=AVERAGE(A1:A4)"), ("C3", "=VLOOKUP(\"banana\",CHOOSE({1,2},B1:B4,A1:A4),2,FALSE)"),
        ("C4", "=COUNTIF(B1:B4,\"apple\")"), ("C5", "=SUMIF(B1:B4,\"apple\",A1:A4)"), ("C6", "=IF(A1>0,\"yes\",\"no\")"),
        ("C7", "=INDEX(B1:B4,MATCH(3,A1:A4,0))"), ("C8", "=TEXT(1234.5,\"#,##0.00\")"), ("C9", "=CONCAT(B1,\"-\",B2)"),
        ("C10", "=ROUND(PI(),3)"), ("C11", "=DATE(2024,3,1)-DATE(2024,2,1)"), ("C12", "=PMT(0.05/12,60,10000)"),
        ("C13", "=1/0"), ("C14", "=XLOOKUP(\"cherry\",B1:B4,A1:A4)"), ("C15", "=SUMPRODUCT(A1:A4,A1:A4)"),
        ("C16", "=LEFT(\"spreadsheet\",6)&UPPER(\"x\")"), ("C17", "=STDEV.S(A1:A4)"), ("C18", "=LET(x,5,x*x)"),
        ("C19", "=NOSUCH(1)"), ("C20", "=TEXTJOIN(\", \",TRUE,B1:B3)"),
    ]);
    let want = [
        ("C1", "10"), ("C2", "2.5"), ("C3", "2"), ("C4", "2"), ("C5", "5"), ("C6", "yes"), ("C7", "cherry"),
        ("C8", "1,234.50"), ("C9", "apple-banana"), ("C10", "3.142"), ("C11", "29"), ("C13", "#DIV/0!"), ("C14", "3"),
        ("C15", "30"), ("C16", "spreadX"), ("C18", "25"), ("C19", "#NAME?"), ("C20", "apple, banana, cherry"),
    ];
    for (a, v) in want {
        assert_eq!(shown(&wb, a), v, "cell {a}");
    }
    assert!(shown(&wb, "C12").contains("188.71"), "{}", shown(&wb, "C12"));
    assert!(shown(&wb, "C17").starts_with("1.29099"), "{}", shown(&wb, "C17"));
}

#[test]
fn spills_and_cycles() {
    let wb = book(&[("A1", "3"), ("A2", "1"), ("A3", "2"), ("C1", "=SORT(A1:A3)"), ("D1", "=SEQUENCE(2,2)"), ("F1", "=F1+1"), ("G1", "=SUM(C1#)")]);
    assert_eq!(shown(&wb, "C3"), "3");
    assert_eq!(shown(&wb, "E2"), "4");
    assert_eq!(shown(&wb, "F1"), "0");
    assert_eq!(shown(&wb, "G1"), "6");
    let blocked = book(&[("A1", "=SEQUENCE(3)"), ("A2", "x")]);
    assert_eq!(shown(&blocked, "A1"), "#SPILL!");
}

#[test]
fn row_insert_moves_references() {
    let mut wb = book(&[("A1", "1"), ("A2", "2"), ("A3", "=SUM(A1:A2)")]);
    ops::insert_rows(&mut wb, 0, 1, 2);
    assert_eq!(ops::edit_text(&wb, 0, 4, 0), "=SUM(A1:A4)");
    ops::delete_rows(&mut wb, 0, 0, 1);
    assert_eq!(ops::edit_text(&wb, 0, 3, 0), "=SUM(A1:A3)");
}

#[test]
fn sort_filter_fill() {
    let mut wb = book(&[("A1", "3"), ("A2", "1"), ("A3", "2"), ("B1", "Mon"), ("C1", "1"), ("C2", "3")]);
    let calc = recalc(&wb);
    ops::sort_range(&mut wb, &calc, 0, Rect::new(0, 0, 2, 0), &[(0, true)], false);
    assert_eq!(shown(&wb, "A1"), "1");
    ops::autofill(&mut wb, 0, Rect::new(0, 1, 0, 1), Rect::new(0, 1, 2, 1), false);
    assert_eq!(shown(&wb, "B3"), "Wed");
    ops::autofill(&mut wb, 0, Rect::new(0, 2, 1, 2), Rect::new(0, 2, 3, 2), false);
    assert_eq!(shown(&wb, "C4"), "7");
}

#[test]
fn xlsx_and_ods_round_trip() {
    let mut wb = book(&[("A1", "12.5"), ("A2", "hello & <you>"), ("A3", "=A1*2"), ("B1", "2024-01-15"), ("B2", "TRUE"), ("B3", "=UNIQUE(A1:A2)")]);
    ops::style_ranges(&mut wb, 0, &[Rect::cell(Pos::new(0, 0))], &|s| {
        s.bold = true;
        s.fill = Some(0xFFEE00);
        s.num_fmt = "0.00".into();
    });
    ops::set_merge(&mut wb, 0, &[Rect::new(5, 0, 5, 3)], 0);
    let sh = Arc::make_mut(&mut wb.sheets[0]);
    sh.freeze = (1, 0);
    sh.charts.push(Chart { kind: ChartKind::Column, title: "T".into(), series: vec![Series { name: "S".into(), vals: "Sheet1!$A$1:$A$3".into(), ..Default::default() }], x: 10.0, y: 10.0, w: 300.0, h: 200.0, legend: true, x_title: String::new(), y_title: String::new(), data_labels: false, gridlines: true });
    sh.notes.insert(Pos::new(0, 0), Note { text: "hi".into(), author: "me".into() });
    let calc = recalc(&wb);
    let bytes = xlsx_write::save(&wb, &calc).unwrap();
    let back = xlsx_read::load(bytes).unwrap();
    assert_eq!(shown(&back, "A3"), "25");
    assert_eq!(shown(&back, "A1"), "12.50");
    assert_eq!(shown(&back, "A2"), "hello & <you>");
    assert_eq!(ops::edit_text(&back, 0, 2, 1), "=UNIQUE(A1:A2)");
    let st = back.styles.get(back.sheets[0].style_id(0, 0));
    assert!(st.bold);
    assert_eq!(st.fill, Some(0xFFEE00));
    assert_eq!(back.sheets[0].freeze, (1, 0));
    assert_eq!(back.sheets[0].merges.len(), 1);
    assert_eq!(back.sheets[0].charts.len(), 1);
    assert_eq!(back.sheets[0].charts[0].kind, ChartKind::Column);
    assert_eq!(back.sheets[0].notes.len(), 1);

    let o = ods::save(&wb, &calc).unwrap();
    let back = ods::load(o).unwrap();
    assert_eq!(shown(&back, "A3"), "25");
    assert_eq!(back.sheets[0].get(0, 0).map(|c| c.v.clone()), Some(Content::Num(12.5)));
    assert_eq!(ops::edit_text(&back, 0, 1, 0), "hello & <you>");
    assert_eq!(back.sheets[0].merges.len(), 1);
}

#[test]
fn delimited_text() {
    let wb = ods::load_delimited("a,b\n1,\"x,y\"\n", "data");
    assert_eq!(shown(&wb, "B2"), "x,y");
    assert_eq!(shown(&wb, "A2"), "1");
    let calc = recalc(&wb);
    assert_eq!(ods::save_delimited(&wb, &calc, 0, ','), "a,b\n1,\"x,y\"\n");
}

#[test]
fn debug_merge_xml() {
    let mut wb = book(&[("A1", "1")]);
    ops::set_merge(&mut wb, 0, &[Rect::new(5, 0, 5, 3)], 0);
    eprintln!("merges before {:?}", wb.sheets[0].merges);
    let calc = recalc(&wb);
    let bytes = xlsx_write::save(&wb, &calc).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut z.by_name("xl/worksheets/sheet1.xml").unwrap(), &mut s).unwrap();
    let back = xlsx_read::load(xlsx_write::save(&wb, &calc).unwrap()).unwrap();
    assert_eq!(back.sheets[0].merges.len(), 1);
}

#[test]
fn pdf_export_has_pages_and_chart() {
    let wb = crate::sample::sample_book();
    let calc = crate::eval::recalc(&wb);
    let bytes = crate::print::pdf(&wb, &calc, &[0, 1]);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("%PDF-1.4"));
    assert!(text.contains("/Count "));
    assert!(text.contains("(Quarterly Sales Report) Tj"));
    assert!(text.contains("(Sales by Quarter) Tj"), "chart title should be drawn into the PDF");
    assert!(text.trim_end().ends_with("%%EOF"));
    let (rows, cols) = crate::print::breaks(&wb, &calc, 0);
    assert!(rows.is_empty());
    assert!(!cols.is_empty(), "the chart makes the sheet wider than one portrait page");
}
