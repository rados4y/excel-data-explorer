# Verification report
Environment: Linux x86_64, 2026-11-14, Node 24.22.1, Rust stable 1.100.0, Chromium, current locked dependencies.

- Rust: **43 tests passed** (5 version/lifecycle, 10 cache/relationships, 5 curated sample/portability tests, 2 documents, 6 parsing, 9 spaces/pages, 6 synchronization). A Windows-only exclusive-file-lock test is supplied and is not counted as executed on Linux.
- Demo tooling: **2 Python standard-library tests passed**, covering deterministic XLSX/PDF generation and safe offline/online/scenario restoration.
- Frontend: **6 Vitest tests passed**; TypeScript and Vite production build passed.
- End-to-end: **13 Playwright tests passed** against the real Rust diagnostic process and SQLite. Covers virtualization, search, invoice drill-down, PDF path validation, filters, pagination, alternate workspace mappings, restored view state, source status, pause and cached browsing while a workbook is missing. Five gallery cases additionally validate every curated sample and capture its preview image on explicit request. The test-only event shell is stubbed; it does not verify native document launching.
- Rust formatting and Clippy (all core targets, warnings denied) plus Prettier check are available as verification commands.
- Native Windows-target type/API check (`x86_64-pc-windows-gnu`, desktop feature) passed using a user-local Zig C/resource compiler adapter on Linux. This includes the multi-workbook native file picker and dispatch commands for spaces/pages. It is a compile/type check, not a linked/installed Windows executable test.
- Release benchmark: 70,001 real imported records, measured timings/RSS in `PERFORMANCE.md` and raw JSON. Full 700,001-record inputs generated; full-scale import not measured here.

Required Windows follow-up: build/link/install the MSVC NSIS release, run `tests/windows/lifecycle.ps1`, the exclusive-lock test and the interactive checklist in `WINDOWS_RELEASE.md`; verify hidden sign-in, close-to-tray, existing-instance reuse, default PDF handler, sleep/reconnect and a real UNC share. CI workflow is supplied but was not executed remotely.

No Windows GUI, installer execution, code signing, cold/warm WebView launch, FPS or Windows idle-CPU result is claimed as verified. Source fixtures remain read-only during application imports. Tests mutate only generated/copied fixture workbooks, independently of application read-only ingestion.

## Curated sample gallery verification
All five `demo/` configurations import after their folders are copied to arbitrary temporary paths. Each `expected-results.json` is reconciled against real entity counts, relationship child IDs, orphan warning counts, and document statuses. Source workbook bytes remain unchanged during import. Tests verify decimal/boolean/date normalization, leading-zero keys, namespace identity isolation, rejected document openings, failed generation retention and offline recovery.

The five checked-in `preview.png` files were generated with `scripts/capture-demo-images.mjs` using Chromium and the actual Rust/SQLite backend. The full thirteen-test browser suite passes without rewriting screenshots; `npm run demo:images` performs explicit recapture. Input generation, relative README/config/workbook/image links, production TypeScript/Vite build, Rust formatting/Clippy and Prettier checks were also verified. The gallery remains compatible with optional YAML import; arbitrary workbooks can also use the configuration-free Add Excel workflow.

## Configuration-free spaces and pages
Nine Rust integration tests cover startup without profiles, persisted empty spaces, separate/grouped workbooks, every worksheet (including empty sheets and blank/duplicate headers), source-scoped pages, name-based column reorder stability, typed mapping/unique-key failures, fixed-filter saved views, offline restart, presentation edits without fingerprint invalidation, and legacy YAML compatibility. Three frontend model tests verify mapping refresh preserves logical identities and avoids overwriting duplicated headers.

Four new browser cases exercise empty startup, named empty spaces, importing multiple workbooks, default file-to-space and worksheet-to-page organization, grouping, page visibility across restart, saved filtered views, table/detail column visibility, and in-app key/type/label/header configuration. Four onboarding/editor screenshots accompany the [spaces and pages guide](SPACES_AND_PAGES.md), generated from the same real-core bridge. Native file dialogs and asynchronous desktop sync events require Windows acceptance in addition to these browser tests.
