# Windows build and verification
Use Windows 11 x64, current Rust stable with `x86_64-pc-windows-msvc`, Node 22/24 LTS, Python 3, Visual Studio 2022 Build Tools (Desktop development with C++ and Windows SDK), and the WebView2 Evergreen Runtime. Excel is not required.

```powershell
npm ci
python fixtures/generate_demo_data.py
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
npm test
cargo check --manifest-path src-tauri/Cargo.toml --features desktop
npm run desktop
npm run desktop:build
```

The final command builds an NSIS **per-user** installer under `src-tauri/target/release/bundle/nsis/`. It enables Tauri's custom protocol for the packaged frontend. Install as the normal user. The installer bundles generated demo workbooks, PDFs and YAML profiles; a fresh installation starts empty and offers Add Excel / New space. The bundled/reference demo assets are optional examples; no configuration is required at startup. No real company files are included.

The normal Windows defaults are implemented as: **sign-in invocation `--background` stays hidden; manual invocation shows the window; a second launch restores the existing process; X hides to tray; tray Exit cancels work and terminates; all browsing reads SQLite**. Autostart uses the official Tauri 2 autostart plugin with `--background`, per user. The window is configured invisible initially; persisted window visibility is deliberately excluded from restoration to avoid login flashes. Geometry and maximization are restored by the official window-state plugin.

Tray actions: Open, Sync now, Pause / Resume sync, Settings, Sync status, Exit. Left-click opens. Tray tooltip discloses source failures. Settings persist autostart and close-to-tray preferences. Turning close-to-tray off makes X exit gracefully.

## Runnable Windows lifecycle test
With no instance running:
```powershell
powershell -ExecutionPolicy Bypass -File tests/windows/lifecycle.ps1 -Exe 'C:\path\excel-data-explorer.exe'
```
This checks hidden background launch, process reuse, second-launch restoration and close-to-tray. It leaves the process running so you can test tray Open and Exit. Use the tray to Exit before continuing. The script does not force-kill user data work.

## Interactive acceptance checklist (requires Windows)
First verify the configuration-free workflow in the spaces/pages checks below. For the original relational checks in steps 3–9, explicitly Import YAML using `workspaces/demo-workspace.yaml` after fixture generation.

1. Enable autostart, Exit, sign out and back in. Verify tray/process exists with no visible window/focus flash. Check registry Startup entry contains `--background`. Disable autostart and verify the entry is removed.
2. Launch from Start twice. Verify one process and one worker; minimize then launch again to confirm restoration.
3. Filter offers, resize columns and move/resize the window. X, tray Open, then Exit/restart: verify workspace, filters, selection, details and geometry.
4. Double-click OFF-001. Invoices contains INV-1001 and INV-1045; Protocols contains PR-022. Drill into an invoice; open its PDF with the registered viewer. Missing PDF and traversal/executable references fail without closing Explorer.
5. Temporarily move both workbooks away. Browse cached offers and details; Sync now must show stale/unavailable labels and original successful import times. Restore files and sync to clear errors.
6. Add invoice, save Excel, browse while synchronization runs. Test lock with Excel/open file, rename/replace save, malformed key, missing sheet, interrupted network share and a valid recovery. Finance sheets must publish together.
7. Sleep/resume and disconnect/reconnect a real UNC share. Resume/periodic checks must recover. Check no source file bytes or timestamps changed due to Explorer.
8. Pause sync: automatic checks stop, explicit Sync now still works and pause preference remains enabled. Resume recovers changes.
9. Exit during snapshot/parsing; restart. Committed cache remains valid; interrupted imports are cancelled. A blocking SMB OS call can delay cleanup.
10. Run target-scale benchmarks and Windows idle/restore measurements; record p50/p95/p99, RAM, UI FPS and cold vs warm launch.

## CI and release
`.github/workflows/ci.yml` runs Linux core/UI tests and Windows native build/check, and uploads an unsigned NSIS artifact on manual workflow dispatch. It is supplied but has not been executed from this workspace.

Unsigned local builds may show SmartScreen warnings. For production, configure Tauri Windows signing (`certificateThumbprint`, `digestAlgorithm`, `timestampUrl` or a protected signing command) using the organization's certificate and secret store; sign the executable and installer, verify signatures, and test install/update/uninstall under a standard account. No certificate or credentials are committed. Keep `%LOCALAPPDATA%/ExcelDataExplorer/` for user cache/settings/logs; don't silently delete it during upgrades or reinstall.

The SQLite cache is not encrypted by the application. Use Windows user permissions and device encryption/BitLocker for confidential data. Logs include stages, source identifiers, row/field diagnostics and timings, not full workbook rows or document content. Logs rotate daily; retention cleanup is currently manual.

This delivery was implemented in Linux. Windows native code can be cross-checked for API/type correctness where tooling permits, but sign-in behavior, Windows file locks/junctions, WebView2, default PDF launching and installer execution require the tests above on Windows. Do not treat Linux IPC browser tests as Windows GUI acceptance.

## Spaces and pages integration checks
With a fresh local app database, verify the empty start screen appears without a YAML file. Use the native multi-file picker to add two workbooks and verify separate spaces, worksheet pages (including empty sheets), named-space grouping, worksheet header/type/key edits, saved views, column/page hiding, per-page state restoration and offline browsing. Hidden pages/columns are display settings, not access-control rules. Existing YAML-imported caches should retain their original main/detail behavior. A Windows-target compile check validates the added multi-file picker API; native dialog behavior must still be exercised on Windows.
