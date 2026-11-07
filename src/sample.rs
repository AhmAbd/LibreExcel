//! A small example workbook, used for screenshots.

use crate::model::*;
use crate::ops;

pub fn sample_book() -> Workbook {
    let mut wb = Workbook::default();
    let rows: [(&str, &str, [f64; 4]); 8] = [
        ("North", "Laptops", [12500.0, 14200.0, 13900.0, 16100.0]),
        ("North", "Monitors", [4300.0, 3900.0, 4700.0, 5200.0]),
        ("South", "Laptops", [9800.0, 10400.0, 11800.0, 12300.0]),
        ("South", "Monitors", [3100.0, 3600.0, 3300.0, 4100.0]),
        ("East", "Laptops", [15200.0, 14800.0, 16500.0, 17900.0]),
        ("East", "Monitors", [5100.0, 5600.0, 5900.0, 6400.0]),
        ("West", "Laptops", [11000.0, 11900.0, 12600.0, 13400.0]),
        ("West", "Monitors", [3800.0, 4200.0, 4000.0, 4600.0]),
    ];
    let head = ["Region", "Product", "Q1", "Q2", "Q3", "Q4", "Total", "Growth"];
    let set = |wb: &mut Workbook, r: u32, c: u32, s: &str| {
        let _ = ops::set_input(wb, 0, r, c, s);
    };
    set(&mut wb, 0, 0, "Quarterly Sales Report");
    for (c, h) in head.iter().enumerate() {
        set(&mut wb, 2, c as u32, h);
    }
    for (i, (reg, prod, q)) in rows.iter().enumerate() {
        let r = 3 + i as u32;
        set(&mut wb, r, 0, reg);
        set(&mut wb, r, 1, prod);
        for (k, v) in q.iter().enumerate() {
            set(&mut wb, r, 2 + k as u32, &v.to_string());
        }
        set(&mut wb, r, 6, &format!("=SUM(C{0}:F{0})", r + 1));
        set(&mut wb, r, 7, &format!("=F{0}/C{0}-1", r + 1));
    }
    set(&mut wb, 11, 0, "Total");
    for c in 2..7u32 {
        let n = col_name(c);
        set(&mut wb, 11, c, &format!("=SUBTOTAL(109,{n}4:{n}11)"));
    }
    set(&mut wb, 11, 7, "=F12/C12-1");
    set(&mut wb, 13, 0, "Best region");
    set(&mut wb, 13, 2, "=INDEX(A4:A11,MATCH(MAX(G4:G11),G4:G11,0))");
    set(&mut wb, 14, 0, "Average total");
    set(&mut wb, 14, 2, "=AVERAGE(G4:G11)");

    ops::style_ranges(&mut wb, 0, &[Rect::cell(Pos::new(0, 0))], &|s| {
        s.bold = true;
        s.size10 = 180;
        s.color = Some(0x1F4E78);
    });
    ops::style_ranges(&mut wb, 0, &[Rect::new(3, 2, 11, 6), Rect::new(14, 2, 14, 2)], &|s| s.num_fmt = "#,##0".into());
    ops::style_ranges(&mut wb, 0, &[Rect::new(3, 7, 11, 7)], &|s| s.num_fmt = "0.0%".into());
    ops::style_ranges(&mut wb, 0, &[Rect::new(11, 0, 11, 7)], &|s| {
        s.bold = true;
        s.borders[B_TOP] = Some(Border { style: LineStyle::Thin, color: 0x4472C4 });
        s.borders[B_BOTTOM] = Some(Border { style: LineStyle::Double, color: 0x4472C4 });
    });
    ops::style_ranges(&mut wb, 0, &[Rect::new(13, 0, 14, 0)], &|s| s.italic = true);
    {
        let sh = wb.sheet_mut(0);
        sh.col_w.insert(0, 90.0);
        sh.col_w.insert(1, 80.0);
        sh.row_h.insert(0, 30.0);
        sh.custom_h.insert(0);
        sh.tables.push(TableDef {
            name: "Sales".into(),
            range: Rect::new(2, 0, 10, 7),
            style: 2,
            header: true,
            totals: false,
            banded: true,
            columns: head.iter().map(|s| s.to_string()).collect(),
        });
        sh.filter = Some(AutoFilter { range: Rect::new(2, 0, 10, 7), cols: Default::default() });
        sh.cond.push(CondFormat { ranges: vec![Rect::new(3, 6, 10, 6)], rule: CondRule::DataBar { color: 0x5B9BD5 }, style: CondStyle::default(), stop_if_true: false });
        sh.cond.push(CondFormat {
            ranges: vec![Rect::new(3, 7, 10, 7)],
            rule: CondRule::CellIs { op: ">".into(), a: "0.2".into(), b: String::new() },
            style: CondStyle { fill: Some(0xC6EFCE), color: Some(0x006100), bold: false, italic: false },
            stop_if_true: false,
        });
        sh.charts.push(Chart {
            kind: ChartKind::Column,
            title: "Sales by Quarter".into(),
            series: (0..4u32)
                .map(|i| Series { name: String::new(), name_ref: Some(format!("Sheet1!${}$3", col_name(2 + i))), cats: Some("Sheet1!$A$4:$A$11".into()), vals: format!("Sheet1!${0}$4:${0}$11", col_name(2 + i)), color: None })
                .collect(),
            x: 690.0,
            y: 40.0,
            w: 460.0,
            h: 280.0,
            legend: true,
            x_title: String::new(),
            y_title: String::new(),
            data_labels: false,
            gridlines: true,
        });
    }
    let mut s2 = Sheet::new("Notes");
    s2.set_content(0, 0, Content::Text("Use this sheet for notes.".into()));
    wb.sheets.push(std::sync::Arc::new(s2));
    wb
}
