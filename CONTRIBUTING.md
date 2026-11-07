# Contributing to LibreExcel

Thank you for your interest in LibreExcel (Tabula). Every contribution helps:
bug reports, Excel compatibility gaps, new functions, UI polish, tests and docs.

By taking part, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to help

- **Report a bug.** Open an issue with the bug report template. Include steps to reproduce, what you expected and what happened. Attach a small `.xlsx` file if the bug depends on a workbook.
- **Report an Excel difference.** If a formula, format or behavior gives a different result than Excel, tell us the input, the Excel result and the Tabula result.
- **Suggest a feature.** Open an issue with the feature request template. Explain the use case first.
- **Send a pull request.** Fixes and features are welcome. For large changes, open an issue first so we can agree on the approach.

## Development setup

1. Install the stable Rust toolchain from <https://rustup.rs>.
2. On Debian or Ubuntu, install the system libraries:

   ```bash
   sudo apt install libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev
   ```

3. Clone and run:

   ```bash
   git clone https://github.com/AhmAbd/LibreExcel.git
   cd LibreExcel
   cargo run
   ```

4. Run the tests:

   ```bash
   cargo test
   ```

## Pull request checklist

- The project builds with `cargo build` and the tests pass with `cargo test`.
- New formula functions or engine changes come with a test in `src/engine_tests.rs`.
- UI changes include a before and after screenshot in the pull request.
- Keep each pull request focused on one change.
- Write clear commit messages in the imperative mood, for example `Add XMATCH function`.

## Code guidelines

- Match Excel behavior where possible. When Excel and Tabula differ, Excel is the reference.
- Keep the code dependency-light. Discuss new crates in an issue first.
- Follow the style of the surrounding code.
- Prefer small, readable functions and short comments that explain why, not what.

## Adding a formula function

1. Find the right module: `funcs_math.rs`, `funcs_stat.rs`, `funcs_text.rs`, `funcs_look.rs` or `funcs_fin.rs`.
2. Add the function next to related ones, following the existing pattern.
3. Return the same error values as Excel (`#VALUE!`, `#N/A`, `#DIV/0!` and so on).
4. Add test cases to `src/engine_tests.rs`, including edge cases and errors.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
