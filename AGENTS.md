# Excel Data Explorer
- The attached specification is the product contract. Record ambiguous choices in docs/DECISIONS.md.
- Excel is read-only input. Never execute macros, formulas, scripts or arbitrary paths.
- All interactive reads use SQLite; background imports activate whole physical workbooks atomically.
- Keep last valid cache and config on failure. Namespace identities and workspace boundaries are mandatory.
- Core compiles without the desktop feature; Tauri native integration uses `--features desktop`.
- Run Rust tests, frontend checks and appropriate end-to-end tests after functional changes.
