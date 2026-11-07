# LibreExcel (Tabula)

Tabula is a free and open-source (libre) spreadsheet application with the look,
feel and functionality of Microsoft Excel. It brings the familiar Excel user
interface (title bar, ribbon tabs, formula bar, grid, sheet tabs, status bar
and Backstage file view) to a native desktop app written in Rust.

The goal: people who know Excel can sit down and work right away, without
proprietary software.

## Features

- Excel-style UI: ribbon (Home, Insert, Formulas, Data and more), formula bar,
  Name Box, sheet tabs and status bar
- Formula engine with Excel-compatible syntax and functions: math, statistical,
  text, lookup (`VLOOKUP`, `XLOOKUP`, `INDEX`/`MATCH`), logical and financial
- Cell formatting and Excel number formats
- Conditional formatting
- Sorting and filtering
- Charts
- Print layout and page setup
- Open and save `.xlsx` and `.ods` files, plus CSV and other delimited text
- Undo and redo, recent files

## Build

You need a Rust toolchain (stable, edition 2021). Install it from
<https://rustup.rs>.

```bash
cargo build --release
```

The binary is at `target/release/tabula`.

Run it directly, optionally with a file to open:

```bash
cargo run --release -- path/to/workbook.xlsx
```

On Linux, the app supports both Wayland and X11.

Run the engine tests:

```bash
cargo test
```

## License

Tabula is released under the [MIT License](LICENSE).

The fonts in `assets/fonts/` (Liberation, Carlito and Noto) are third-party
fonts under the SIL Open Font License 1.1. They are not covered by the MIT
License.
