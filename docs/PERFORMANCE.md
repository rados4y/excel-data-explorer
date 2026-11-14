# Performance measurements
Measured 2026-11-14 in the task's Linux x86_64 container, 3 virtual CPUs on AMD EPYC 9V74/KVM, Rust 1.100 release profile, SQLite local overlay storage. Shared machine activity introduces timing outliers. This is **not** a Windows 11 / SSD certification.

The measured fixture contains **10,000 offers, 50,001 invoices (including one orphan), 10,000 protocols and 50,000 document references**: 70,001 records. Only three sample PDFs exist. The benchmark imports references but does not launch viewers or benchmark all document-availability probes.

Raw measurements: `benchmarks/linux-70k.json`. Timings are milliseconds, from real Rust/SQLite calls, including connection/config lookup cost; no mocked records. Queries return at most 100 rows.

| Operation | Samples | p50 | p95 | p99 |
|---|---:|---:|---:|---:|
| Default indexed offer page | 50 | 1.83 | 16.38 | 40.90 |
| FTS prefix search | 50 | 2.34 | 7.10 | 10.68 |
| Status filter + decimal amount sort | 50 | 33.19 | 37.13 | 125.46 |
| Cached record detail | 100 | 0.28 | 0.46 | 0.55 |
| Related invoice page | 100 | 1.77 | 5.50 | 31.32 |
| Engine reopen + cached page, warm OS cache | 30 | 1.96 | 2.40 | 16.10 |
| Foreground page during forced offer import | 173 | 1.93 | 3.67 | 25.14 |

Initial finance workbook import: **11.30 s** for 60,001 rows. Offers: **3.48 s** for 10,000 rows, including workspace edge rebuilding. Combined ≈4,736 records/s. Peak core-process RSS: **33,500 KiB (32.7 MiB)**. Final main database: **330,772,480 bytes (315.5 MiB)**, excluding transient staging/WAL files and installer/WebView memory. Import timing includes the benchmark's 10 ms stability interval; production defaults to 1,500 ms. Stage/activation breakdowns are stored in `import_runs.stages_json`.

The benchmark caught two pathological SQLite plans: unconstrained relation joins could iterate a parent/child cross product; orphan probes could scan the relation prefix for each child. Explicit indexed join order and the reverse child index fix these. Related UI queries now begin with indexed parent edges; default business-key ordering uses a covering record index. Regression tests inspect the join plan.

## Reproduce
From the project root, with Rust stable, Node and Python 3:
```sh
# Independent directory/database for a clean first import; local disk only.
python benchmarks/run.py --offers 10000 --output /local/ssd/ede-benchmark-70k
# Specification-scale fixture (100k offers, 500k invoices + one orphan, 100k protocols).
python benchmarks/run.py --offers 100000 --output /local/ssd/ede-benchmark-700k
```
Use a fresh output directory for initial-import timings; rerunning reuses the cache and reports unchanged sources. The generator is deterministic and includes 50,000 references at both of these sizes. Allow at least 8 GB free space for the large cache, staging and WAL. Outputs include the configuration, generated OOXML, cache and `results.json`. `src-tauri/src/bin/benchmark.rs` runs 30–100 samples per scenario and foreground reads during a real forced import. `VmHWM` is read from `/proc` on Linux; peak RSS is null on other platforms, so use Windows Performance Monitor or Process Explorer there.

## Windows and remaining gaps
The full 700,001-record inputs were generated here, but **were not imported/benchmarked** because this workspace's limited local disk also holds build toolchains. The 70k results cannot establish the 700k target. Run the full-scale script on the release Windows machine and record p50/p95/p99 and peak RAM.

Warm *engine* reopening is not a process launch, window restore or completed WebView paint. Cold application launch, hidden WebView cost, warm tray restoration, Windows idle CPU, UI frame rate, large-file Calamine peak RAM, SMB latency/reconnect/locks and full Windows foreground performance remain **unmeasured**. The specification's <250/<120/<200 ms and <1% CPU values remain targets for those scenarios.

For window restoration and idle CPU, run `tests/windows/measure.ps1 -Exe <installed exe>` while no import is active. It reports visible-window-handle p50/p95/p99, a 60-second one-core CPU percentage, and working set. It explicitly does not measure completed WebView paint. The frontend emits `performance.mark('cached-ui-ready')` after a cached page arrives; use WebView developer tools plus ETW/Windows Performance Recorder to measure real launch-to-paint and scrolling frame rate. Include machine, build/version, dataset, local/network path, cold/warm cache conditions, active-import condition and sample count with each report.
