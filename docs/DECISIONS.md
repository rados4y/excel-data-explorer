# Decisions
- Follow formal milestone order M0–M6, keeping CLI and desktop entry points runnable.
- Core CLI is a diagnostic/test entry point to the same Rust engine, not a server or second production backend.
- Relative roots resolve against the YAML file's directory. Only `${rootName}` placeholders expand.
- Namespaces must match for relationship joins. Unspecified namespace defaults to the source ID.
- Date-only values remain YYYY-MM-DD. Naive datetimes use configured IANA timezone, reject ambiguous/nonexistent local times, and become RFC3339. Explicit offsets are preserved as an instant, normalized to fixed-precision UTC RFC3339 for consistent sorting.
- Decimal fields are canonical decimal strings. Numeric projections are for approximate sorting only; keys stay strings.
- Optional invalid cells become null plus diagnostics. Required invalid cells reject the physical workbook import.
- Current dependency verification (2026-11-14): Tauri 2.12.5 / official v2 plugins, React 19, Vite 7, Calamine 0.38, rusqlite 0.42, SHA-2 0.11, Rust Decimal 1.43, and serde-saphyr 2.1 for maintained YAML parsing. Exact resolved versions are locked in npm/Cargo lockfiles. Rust stable 1.100 and Node 24 were used here.
- Atomic activation also includes the source fingerprint and successful import status, avoiding crash windows between published records and freshness metadata.
- Cached formula-only rows can extend past Calamine's trimmed value range; validation considers both the value and formula ranges.
- Relationship joins deliberately order indexed parent→field→child-field→record lookups, and orphan probes use the reverse child index. Benchmarks caught pathological planner choices without these constraints.
- Document availability checks use bounded batches of up to 5,000 references and reuse parsed profiles; filesystem checks occur outside the write transaction. Verification is scheduled every 30 seconds for references unchecked or older than five minutes. Opening always revalidates immediately.
- The built-in demo is copied to the user data directory so source examples can be edited in Excel without writing under Program Files. Existing demo files are never overwritten by application startup.

- The curated `demo/` gallery supplements the original first-run fixture. Each example owns a unique workspace ID, relative data/document roots, checked-in XLSX/PDF inputs, a static YAML profile and expected-result manifest. Generator commands rewrite only explicitly synthetic data and manifests; authored configuration is left intact. Scenario helpers intentionally modify only the sync sample's input, outside the application's read-only ingest path.
- Gallery illustrations are screenshots of the actual React UI against a Rust/SQLite CLI IPC test bridge. Capture tests copy samples to temporary directories and never replace repository workbooks. Screenshot output is explicit (`demo:images`), rather than a side effect of routine browser tests.
