# Excel Data Explorer — Codex Implementation Specification

> **Purpose:** This file is the primary product and engineering specification for an AI coding agent (Codex). Build the described **working Windows desktop MVP**, not just a prototype, design, or implementation plan.
>
> **Priority:** Correct data > non-blocking operation > responsiveness > polished UI > extra features.
>
> **Status:** Implementation brief, v1.0. The sample configurations and fixtures are illustrative and must be included in the repository so that the application can be demonstrated without private company files.

---

## 0. Instructions for Codex

You are the implementation agent. Create a maintainable, runnable application following this specification.

1. **Implement working code, not merely a proposal.** Work incrementally through the milestones in Section 18. Do not stop after scaffolding or generating mock screens.
2. **Preserve a compiling/runnable state** after each milestone. Write tests for parsing, indexing, relationships, synchronization, and lifecycle behavior.
3. **Use current, officially supported versions** of dependencies compatible with Tauri 2, Rust stable, Node LTS, React, and Windows 11. Verify actual library APIs while implementing; do not assume code snippets from this specification are exact framework APIs.
4. If execution occurs outside Windows, build/test all cross-platform portions and add **Windows-specific integration tests and instructions**. Clearly report any Windows functionality that cannot be verified in the current environment.
5. Use a coherent, straightforward design. Prefer a **single application process**, a background task queue, SQLite, and a configuration-defined data model. Do not introduce cloud services, microservices, HTTP APIs, or an external database.
6. **Never modify source Excel files** or execute macros. The application is read-only with respect to source data.
7. If requirements are underspecified, apply the defaults in this document, document the decision in `docs/DECISIONS.md`, and continue rather than repeatedly asking for clarification.
8. Build a self-contained **demo workspace** with generated Excel files and sample documents; real user workbooks are not available yet.
9. For each major milestone, update `README.md` with the running instructions and report what was implemented, tested, and what remains.
10. Do not claim unmeasured performance targets have been achieved. Build benchmark hooks and record measured results.

**Implementation completion means:** I can install and run the Windows application; it automatically starts hidden at Windows sign-in if the setting is enabled, remains running in the tray, shows a cached offer list instantly when opened, allows drilling into invoices/protocols and linked documents, and synchronizes changed Excel files in the background without blocking browsing or destroying the previous valid cache on failure.

---

## 1. Product definition

### 1.1 Problem

Business data currently lives in a mixture of:

- Several Excel workbooks.
- Different worksheets within those workbooks.
- Different workbook layouts across projects or data sets.
- External files, typically PDF invoices, handover/acceptance protocols, and other documents, linked by IDs or paths.

Users need to browse this information as if it were a well-indexed relational application. Example: find an **offer**, double-click it, inspect related **invoices**, **protocols**, and **documents**, and open the original document.

The application must have **per-workspace configuration** rather than a hard-coded assumption that every Excel workbook has the same sheet names, columns, or relationships.

### 1.2 Guiding principles

- **Excel is an ingestion source, not an interactive query engine.** Interactive screens never reread Excel.
- **Local-first:** The last successfully imported dataset remains readable without network access or access to the original workbooks.
- **Fast warm start:** Start hidden at Windows sign-in, initialize the cache, synchronize in background, and reuse the process when the user opens the app.
- **Safe synchronization:** A failed, locked, incomplete, or temporarily unavailable source must not corrupt or erase cached records.
- **Configurable business model:** Entities, keys, fields, relations, views, and document paths are described in configuration.
- **Read-only MVP:** No writeback to Excel, PDF editing, macros, or automatic document modification.
- **Honest freshness:** Show last successful import and errors; never call stale data “up to date.”

### 1.3 Target users/platform

- One Windows user per local application instance.
- Primary supported platform: **Windows 11, x64**. Windows 10 support is optional if dependencies permit it.
- Excel sources may reside on local drives, mapped drives, or UNC/SMB network shares.
- Documents may reside in configured local or network folders.
- Multiple independent workspaces may be configured.

---

## 2. Architecture decision

Implement the following stack unless a concrete compatibility blocker requires a documented change:

| Layer | Choice | Responsibility |
|---|---|---|
| Native desktop shell | **Tauri 2** | Window, tray, autostart, single-instance, native OS integration |
| Core engine | **Rust (stable)** | Workbook parsing, file discovery, normalization, background import, query API |
| UI | **React + TypeScript + Vite** | Browsing, filtering, detail panels, configuration/status views |
| Components | Lightweight component system / accessible UI library | Tables, navigation, dialogs, forms |
| Data-grid virtualization | **TanStack Table** plus **TanStack Virtual** or equivalent | Smooth tables with large logical row counts |
| Persistent database | **SQLite** (Rust `rusqlite` or equivalent) | Canonical cached records, indexes, relationships, import metadata |
| Excel reader | **Calamine** or another maintained Rust reader | Read values without Excel installation |
| Configuration | **YAML**, parsed into strongly typed structs | Per-workspace source/entity/view definitions |
| Search | SQLite indexes and **FTS5** | Indexed field filters and full-text text search |
| Logging | `tracing` or equivalent | Diagnostic logs without leaking source data |
| Packaging | Tauri Windows installer | Per-user distribution and launch at sign-in |

**Do not** use a separate Python backend, background HTTP server, external SQL server, or always-running Windows service for the MVP. The Tauri application process itself is the warm background process. A hidden WebView window may be created at startup if that improves first-open performance; benchmark its cost.

### 2.1 Logical data flow

```text
Configured Excel workbooks and external document paths
       |                  |
       +--------+---------+
                v
    Change detection + periodic scan
                |
                v
      Stable-file snapshot acquisition
                |
                v
     Parse / normalize / validate records
                |
                v
        SQLite import staging
                |
                v
       Atomic activation / reconciliation
                |
                v
      SQLite persistent cache + indexes
                |
                v
        Rust typed query commands
                |
                v
         React virtualized UI
```

Browsing is always serviced by SQLite. Source files may be completely unavailable while the UI is used.

---

## 3. Windows application lifecycle

### 3.1 Startup at Windows sign-in

Default preference: **autostart enabled; start hidden**.

- Register per-user autostart using the supported Tauri plugin or official Windows-compatible mechanism.
- On normal login launch, do not present a visible main window or steal keyboard focus.
- Initialize the application process, logging, SQLite, configuration, scheduler, and tray icon.
- Read cached workspace metadata immediately; run source checks after initialization, without blocking UI availability.
- If the user manually launches the app from the Start menu/desktop, show its window.
- If the app is already running, **reuse the existing process** and focus/restore its window; prevent a second working instance.
- Distinguish **autostart background invocation** from **user-open invocation**. Do not cause a duplicate or briefly flash a window on login.
- The app must tolerate Windows sleep/resume, shutdown, network reconnection, and session logoff.

### 3.2 Closing and reopening

- Normal window Close (`X`) means **hide to system tray**; the process and synchronization scheduler continue.
- The tray provides **Open**, **Sync now**, **Pause/Resume sync**, **Settings**, **Sync status**, and **Exit**.
- **Exit** is the one explicit action that terminates the process, following graceful cancellation and cleanup.
- A second executable launch while the process exists acts as **Open**.
- Preserve selected workspace, selected record when valid, table filters, column widths, window location/size, and navigation state.
- Tray menu and application window must expose a clear indicator of import errors.

### 3.3 App states

- `Starting`
- `ReadyFromCache`
- `Synchronizing`
- `SyncPaused`
- `SourceUnavailable`
- `SyncFailed` (workspace/source-specific)
- `Exiting`

A source may be unavailable while the application as a whole remains ready from cache. Keep **app state** separate from **individual source freshness**.

### 3.4 Resource use

- Idle CPU target under 1% on representative Windows hardware (measured over a reasonable interval).
- No busy loops or 1-second polling.
- Background tasks use bounded concurrency and cancellation.
- Avoid keeping entire large workbooks or all database rows resident in RAM.

---

## 4. Functional MVP scope

### Required in v1

1. Add and select a workspace from a YAML configuration file.
2. Import multiple workbooks and worksheets for one workspace.
3. Normalize configurable fields and business keys.
4. Cache records, searchable fields, relationships, document links, and provenance in SQLite.
5. Show an **Offers** table (generic configured main entity) with search, sorting, filtering, configurable columns, and virtualization/pagination.
6. Double-click an offer to view details and related **Invoices**, **Protocols**, and **Documents**.
7. Double-click a related record to view its fields and associated documents.
8. Open a linked document in the default Windows application, after secure path validation.
9. Start automatically hidden, single-instance, tray-based persistent runtime.
10. Synchronize changed sources in the background, preserving the last valid cache on any failed import.
11. Show sync state, last successful sync, row counts, import errors, source availability, and manual sync control.
12. Restore basic UI state and settings between launches.
13. Include a sample dataset, automated tests, installer instructions, and performance diagnostics.

### Explicitly out of scope

- Excel editing/writeback, formula evaluation, macro execution.
- OCR, PDF content extraction, document text search.
- Complex business dashboards and charts.
- Graphical mapping/ETL editor; MVP uses YAML configuration.
- Cloud or multi-user replication.
- Generic arbitrary scripting in configuration.
- Arbitrary SQL queries exposed to untrusted configuration/UI.
- Unsandboxed arbitrary file launching.

---

## 5. Configuration model

### 5.1 Concepts

- **Workspace:** A named isolated set of source definitions and views.
- **Source:** A workbook worksheet mapping or similar importable table. Multiple definitions can refer to different sheets of the same physical workbook.
- **Entity type:** Business object such as `offer`, `invoice`, `protocol`.
- **Record identity:** `(workspace, entity_type, namespace, normalized_business_key)`.
- **Relation:** A configurable parent/child join between two entity types.
- **Document reference:** A path-valued field on an entity; documents remain outside SQLite.
- **View:** Which entity to show, which fields appear, how to search/filter, and which relation tabs are available.

### 5.2 Complete representative configuration

Implement a parser and schema validator for a configuration substantially like this example. It is a **functional contract**, not necessarily the only acceptable internal YAML shape; provide the actual accepted schema in the repository.

```yaml
version: 1
workspace:
  id: project-offers-2026
  name: Project Offers 2026

roots:
  data: 'D:/Company/Data'
  documents: '//fileserver/Shared/Documents'

settings:
  timezone: Europe/Warsaw
  local_scan_seconds: 60
  network_scan_seconds: 300
  stable_file_check_ms: 1500
  stable_file_attempts: 3
  read_only: true

sources:
  - id: offers_2026
    path: '${data}/offers.xlsx'
    sheet: Offers
    entity: offer
    namespace: primary
    header_row: 1
    key: id
    fields:
      id: { column: OfferID, type: string, required: true, trim: true }
      customer: { column: Customer, type: string, searchable: true }
      description: { column: Description, type: string, searchable: true }
      amount: { column: Amount, type: decimal }
      status: { column: Status, type: string, filterable: true }
      created_at: { column: CreatedAt, type: date }

  - id: invoices_2026
    path: '${data}/finance.xlsx'
    sheet: Invoices
    entity: invoice
    namespace: primary
    header_row: 1
    key: id
    fields:
      id: { column: InvoiceID, type: string, required: true, trim: true }
      offer_id: { column: OfferID, type: string, required: true, trim: true }
      issued_at: { column: InvoiceDate, type: date }
      amount: { column: Amount, type: decimal }
      document_path: { column: PDF, type: string }

  - id: protocols_2026
    path: '${data}/finance.xlsx'
    sheet: Protocols
    entity: protocol
    namespace: primary
    header_row: 1
    key: id
    fields:
      id: { column: ProtocolID, type: string, required: true, trim: true }
      offer_id: { column: OfferID, type: string, required: true, trim: true }
      type: { column: Type, type: string }
      document_path: { column: PDF, type: string }

relationships:
  - id: offer_invoices
    parent_entity: offer
    child_entity: invoice
    parent_field: id
    child_field: offer_id
  - id: offer_protocols
    parent_entity: offer
    child_entity: protocol
    parent_field: id
    child_field: offer_id

documents:
  - id: invoice_pdf
    entity: invoice
    path_field: document_path
    root: documents
    allowed_extensions: [pdf]
  - id: protocol_pdf
    entity: protocol
    path_field: document_path
    root: documents
    allowed_extensions: [pdf, docx]

views:
  main:
    entity: offer
    title: Offers
    columns: [id, customer, description, amount, status, created_at]
    default_sort: { field: id, direction: asc }
    search_fields: [id, customer, description]
    filters: [status]
  detail:
    entity: offer
    title_field: id
    tabs:
      - { title: Overview, type: fields }
      - { title: Invoices, type: relationship, relationship: offer_invoices }
      - { title: Protocols, type: relationship, relationship: offer_protocols }
      - { title: Documents, type: related_documents }
```

### 5.3 Validation and transformation rules

- Reject configurations with duplicate IDs, missing source references, invalid relation field references, unknown view fields, or unsupported types.
- Check that required source header names exist; report source, sheet, and missing column.
- Keys are strings (never floating-point approximations). Normalize only as configured; avoid automatic destructive case folding or removal of leading zeros.
- Required key fields cannot be null or empty after normalization.
- Support at least `string`, `integer`, `decimal`, `boolean`, `date`, and `datetime` types.
- Preserve original source cell values as feasible for diagnostics.
- Represent decimals with a strategy avoiding accidental binary-floating-point identity or currency errors (e.g., canonical decimal strings, plus an indexed numeric representation only where suitable).
- Dates and timestamps must have an explicit, documented policy. Spreadsheet dates may be date-only; do not silently assign an arbitrary UTC offset.
- Formula cells: use available stored/cached values; never execute or recalculate formulas. Report when a formula has no usable cached value.
- Empty optional fields become null; invalid required values reject the **affected source import**.
- Duplicate business keys reject the affected source import by default, with row-level error details.
- Config updates are validated before activation; keep the last known-good configuration and cache if validation fails.
- Read root directories from configuration; expand only the documented placeholders, not arbitrary environment or shell expressions.

### 5.4 ID conflict policy

Different workbooks can contain the same offer IDs. Therefore identity must include a **namespace**. Default `namespace` is per entity/source if not configured; sources intentionally contributing to one logical entity namespace must opt into a common namespace. If two sources in a shared namespace produce the same business key, fail validation unless a future explicit merge strategy exists. **Never silently overwrite a record belonging to another source.**

---

## 6. Persistent data model

### 6.1 Database characteristics

- Store database under `%LOCALAPPDATA%/ExcelDataExplorer/` (resolve with proper per-user OS APIs).
- The DB must be on a **local disk**, not an SMB/NFS share.
- Enable `PRAGMA foreign_keys = ON`, use SQLite **WAL** mode where supported.
- Use migrations with schema versioning.
- Separate read operations from serialized/controlled write operations. Keep import activation transactions short when practical.
- Configure appropriate busy timeout and WAL checkpoint policy.
- Use parameterized queries and strict allowlists for dynamic column/sort identifiers.
- SQLite is authoritative for interactive reads; it is a **cache** relative to source files.

### 6.2 Required tables

A flexible relational model is required; choose actual SQL definitions and indexes appropriate for Rust/SQLite. The conceptual schema is:

#### `workspaces`

`id`, `name`, `config_path`, `active_config_version`, `created_at`, `updated_at`.

#### `config_versions`

`id`, `workspace_id`, `config_hash`, `config_json`, `is_valid`, `activated_at`, `validation_report`.

#### `source_definitions`

`id`, `workspace_id`, `physical_path`, `sheet_name`, `entity_type`, `namespace`, `mapping_json`, `config_version`.

A physical workbook can contain several source definitions; group them by normalized workbook path for snapshotting/import.

#### `source_state`

`source_definition_id`, `last_seen_size`, `last_seen_mtime`, `last_seen_hash` (optional), `last_attempt_at`, `last_success_at`, `active_generation`, `status`, `last_error`.

Do not use only the last-attempt time as a freshness indicator.

#### `records`

`id`, `workspace_id`, `entity_type`, `namespace`, `business_key`, `source_definition_id`, `source_sheet`, `source_row`, `fields_json`, `record_hash`, `last_import_generation`, `active`, `updated_at`.

Required unique identity: `(workspace_id, entity_type, namespace, business_key)` among active records. Preserve or consistently regenerate stable internal identities so UI relations do not silently point to wrong records after imports. Index `workspace_id/entity_type/active` and source provenance.

#### `record_field_index`

Typed projections for configured sortable/filterable/searchable fields. E.g. `record_id`, `field_name`, `text_value`, `numeric_value`, `date_value`, `bool_value` (nullable). Add effective indexes according to query patterns. The JSON record provides full detail; the projections support efficient queries.

#### `relationship_definitions` and `record_relationships`

`relationship_id`, `workspace_id`, `parent_record_id`, `child_record_id`. Enforce uniqueness and index parent and child lookups. It must be possible to query all child records for one parent without scanning whole workbooks.

#### `documents`

`id`, `workspace_id`, `original_ref`, `resolved_path`, `document_type`, `size`, `last_modified`, `last_verified_at`, `availability_status`, `status_detail`.

Do not store document bytes in SQLite by default.

#### `record_documents`

`record_id`, `document_id`, `document_mapping_id`, with appropriate indexes/uniqueness.

#### `import_runs` / `import_errors`

Import ID, source/workbook reference, status, trigger, timings, row counts (read/created/updated/deleted/unchanged), generation, success/failure, severity, worksheet, row, field, error code and human-readable error.

#### `ui_state` / `app_settings`

Per-user window and view state, autostart enabled, close-to-tray behavior, sync paused state, last selected workspace and filters.

#### `record_fts`

FTS5 virtual table holding searchable string content and record references. Refresh transactionally during import activation; FTS search is over **configured record fields only**, not PDF contents.

### 6.3 Relationship and orphan semantics

- Configure one-to-many relationships as `parent_field = child_field` using normalized keys.
- Relation joins are scoped to the current workspace and matching namespace policies.
- If a child references an absent parent, retain the child and log an **unmatched relation warning**. Do not invent a missing parent.
- If a parent is removed on a successful import, update/remove affected edges; retain source-of-truth child records.
- The MVP may rebuild all relationship edges for a **workspace** in staging after source activation when incremental graph maintenance is complex; correctness is more important than premature optimization.
- Add relational tests for additions, deletions, key changes, and mismatched keys.

### 6.4 Field indexing and search

- Search primarily by indexed ID/customer/description fields defined in YAML.
- Use FTS5 for general text queries and indexed typed projections for filters and sorting.
- Use a parameterized, whitelisted query builder; UI clients cannot submit arbitrary SQL.
- Return **bounded pages**, not all matching records. Prefer stable cursor/keyset pagination when feasible; offset pagination is acceptable for MVP if measured adequately at the target scale.
- Ensure deterministic ordering with a unique tie-breaker such as internal record ID.

---

## 7. Background synchronization (critical)

### 7.1 Synchronization contract

1. Normal browsing **never waits** for Excel reading/parsing.
2. Sources are checked at startup, on file notifications, after resume/reconnect, through periodic scans, and on manual `Sync now`.
3. Imports are **source/workbook scoped**; changing `finance.xlsx` must not force reimport of unrelated `offers.xlsx`.
4. A workbook shared by multiple sheet mappings must be treated as one **physical snapshot** so its sheets are read from a consistent version.
5. The prior successful data for a source remains visible until a complete valid import activates.
6. A failed import does **not** clear the old cache.
7. Display per-source freshness, progress, and errors.
8. No two import jobs for the same workbook run concurrently.
9. Multiple rapid source changes coalesce into the latest necessary import.
10. On exit, tasks stop/cancel cleanly; recover interrupted staging on next startup.

### 7.2 Detection mechanisms

- OS filesystem watch notifications for supported local sources; debounce event bursts.
- Periodic verification for **all** sources, especially UNC/network paths where notifications are unreliable.
- Default **local scan every 60 seconds**; **network scan every 300 seconds**; configurable.
- App resume/network reconnection triggers a scan with backoff.
- File metadata fingerprint: path, length, last-modified time, configuration hash.
- Use content hashing only when metadata is inconclusive or configured. Hashing massive network files every minute is undesirable.
- A successful unchanged check updates `last_checked_at` without falsely advancing `last_successful_import_at`.

### 7.3 Stable snapshot acquisition

Excel may save using temporary files, replace/rename patterns, and transient locks. Never directly parse a workbook that is still changing without a stability check.

1. Observe the candidate file and debounce its change notifications.
2. Compare size and modification time over at least the configured stable interval.
3. Open it in a mode compatible with the source environment; distinguish locked/unavailable/missing.
4. When needed, copy its bytes into a local temporary snapshot file.
5. Recheck metadata after copy. If it changed during acquisition, discard snapshot and retry.
6. Parse the immutable snapshot instead of an actively changing network file.
7. Bound retries with exponential backoff and record the final status.

A stable metadata check is a practical heuristic, not a proof that a remote file is never being written; error handling and atomic activation remain mandatory.

### 7.4 Import state machine

```text
Queued
  -> Checking
  -> Snapshotting
  -> Parsing
  -> Normalizing
  -> Validating
  -> Staging
  -> Activating
  -> Succeeded

Any stage -> Failed / Cancelled / RetryScheduled
```

Track timings and counters for each stage.

### 7.5 Import pipeline

**Discover → Check → Snapshot → Parse → Normalize → Validate → Stage → Activate → Notify.**

- **Discover:** Find source definitions whose workbook path or configuration changed.
- **Check:** Verify path and stable file state.
- **Snapshot:** Copy changed workbook locally if needed and parse one stable version.
- **Parse:** Read only configured worksheets and required column values.
- **Normalize:** Map headers to field names; convert types, clean keys, derive document references.
- **Validate:** Check required headers/keys/types; reject duplicate identity collisions; warn for missing optional data.
- **Stage:** Write imported rows to staging tables or a dedicated staging database. Large files must not require accumulating all rows in RAM.
- **Activate:** In one atomic SQLite transaction, reconcile active record versions, projections, FTS rows, documents and relation edges, then advance successful source generation.
- **Notify:** Invalidate affected query caches and push a sync-completion event to the UI.

**Atomicity boundary:** A physical workbook containing multiple configured worksheets is activated as one logical update when they are imported together. Sources from different physical files can have different successful generations; the UI must disclose their individual freshness and must not imply a cross-file global transaction.

### 7.6 Change reconciliation

- Match by stable scoped business key, **not by Excel row number**.
- Compute normalized record hashes to detect unchanged values.
- Add new records; update changed records; mark removed records inactive/remove via explicit source reconciliation; retain unchanged records.
- Never delete records belonging to a different source definition based on a missing row in the current import.
- Update derived `record_field_index`, `record_fts`, documents, and relationships consistently.
- When only document availability changes, update document status without reimporting its entire Excel source.
- Replace application in-memory caches for changed records/views after successful commit.
- Use SQLite transactions and stage cleanup to recover from crash/power loss.

### 7.7 Concurrency and priorities

- Start with **one active workbook import** at a time; bounded job queue.
- Reading cached data takes priority over import work.
- I/O and CPU-heavy import work must not execute on the UI thread.
- Respect `Pause sync`, but allow manual `Sync now` to resume/override with an explicit UI behavior.
- Prevent uncontrolled memory growth (stream/batch stage rows).
- Avoid needless full recursive scans of directories containing many documents.

### 7.8 Failure matrix

| Event | Required behavior |
|---|---|
| Workbook unavailable/offline | Keep cached records, mark source unavailable, retry later |
| Workbook locked or changing | Backoff/retry; never publish partial data |
| Invalid sheet or missing required column | Fail affected import; retain previous generation |
| Duplicate business key | Fail affected import with worksheet/row diagnostics |
| Invalid optional value | Set null with warning if policy permits |
| Invalid required key/type | Fail affected import |
| Missing related parent | Keep child; show warning about unresolved edge |
| Missing external PDF | Keep link and show unavailable status |
| Import cancelled/crashes | Roll back activation; clear incomplete staging on recovery |
| Invalid YAML update | Keep last known-good profile active and report error |
| Network interruption during a read | Fail snapshot, retain cache, retry |
| DB migration failure | Fail safely; never silently recreate/delete user database |

### 7.9 Sync status UI

For each source show:

- Configured workbook path and sheet(s).
- Last checked time.
- Last **successful import** time.
- Current sync stage/progress, if available.
- File changed/unchanged/unavailable/error state.
- Cached record count.
- Error/warning counts and a diagnostic detail view.

State labels include: `Up to date`, `Checking`, `Synchronizing`, `Cached — source unavailable`, `Error — using cached data`, `Never imported`, and `Paused`.

---

## 8. Document handling

### 8.1 Resolution

- A document is defined by a configured record field and an allowed root directory.
- Normalize Windows paths carefully; support UNC `\\server\share\...` and slash variants from YAML.
- Support both relative paths (joined to a configured root) and explicitly permitted absolute paths.
- When a document path is empty, show no document rather than a broken action.
- Documents are **references**, not copied into the cache.
- Verify availability asynchronously; opening a document must give a useful error if it disappeared.

### 8.2 Safe opening

- Open files using the OS default application through a native API compatible with Tauri.
- Require a document reference originating from a configured mapping or trusted UI selection; **do not accept arbitrary untrusted paths directly from the frontend**.
- Validate allowed extensions and root containment before opening.
- Resolve canonical paths when possible to prevent `..` traversal and symlink/junction escapes; handle nonexistent files separately.
- Do not execute documents as scripts, shell commands, or PowerShell.
- Show filename, type, associated record, and availability in the UI.
- Avoid opening untrusted executable formats.

### 8.3 MVP document UI

- List related documents with name, type, link source, and availability.
- Double-click or click **Open document**.
- A missing/unavailable document remains visible with a clear warning.
- Native file preview is **optional**; opening in the registered program is sufficient for v1.

---

## 9. UI / UX specification

### 9.1 Overall layout

A practical, clean desktop application, optimized for working with business data:

```text
+----------------------------------------------------------------------+
| Excel Data Explorer       [Workspace v]     Sync: Up to date    [⚙]  |
+----------------------------------------------------------------------+
| Offers                                   [Search offers ...]  [Sync] |
|----------------------------------------------------------------------|
| Offer ID | Customer | Description | Amount | Status | Created       |
| OFF-001  | ABC Corp | Project A   | 12,500 | Active | 2026-03-01    |
| OFF-002  | XYZ Ltd  | Project B   |  8,200 | Draft  | 2026-03-02    |
| ...                                                                  |
+----------------------------------------------------------------------+
| Detail: OFF-001                                                       |
| [Overview] [Invoices] [Protocols] [Documents]                          |
|----------------------------------------------------------------------|
| Invoice INV-1001 | 2026-03-10 | 5,000 | [Open PDF]                    |
| Invoice INV-1045 | 2026-03-15 | 7,500 | [Open PDF]                    |
+----------------------------------------------------------------------+
| Cached data: 18 Nov, 09:30 | 3 sources | 0 errors                     |
+----------------------------------------------------------------------+
```

The detail area may be a drawer, split pane, or routed page. **Double-click behavior is mandatory**: double-clicking a table row opens its detail. Single-click may select the row.

### 9.2 Main table

- Search box, multi-column sorting where reasonable, configuration-defined filters.
- Columns, display labels and default sorting from YAML.
- Virtualized rows; never render thousands of DOM rows at once.
- Fixed-height or responsive layout and clear loading/empty/error states.
- Loading should not flicker when returning to a recently opened record.
- Export/copy row ID is optional; changing Excel is not allowed.
- Support keyboard navigation, Enter to open selected record, Escape/back navigation.

### 9.3 Record details

- **Overview:** configured fields in clear label/value layout.
- **Invoices:** paged list of related invoice records; double-click to drill down.
- **Protocols:** equivalent related protocol view.
- **Documents:** unified list reachable from the record and its configured related records, deduplicated by document identity when appropriate.
- Display original workbook/sheet/row provenance where useful (e.g., an expandable “Source details” section).

### 9.4 Settings & sync center

- Workspace selector/add configuration file.
- Autostart toggle (**enabled by default**).
- Close to tray (**enabled by default**; explicit Exit remains available).
- Pause/resume sync and manual sync.
- Per-source status table and errors.
- Open logs folder, view config validation errors.
- Option to reopen main window on manual launch.

### 9.5 Accessibility

- Distinguishable focus states, keyboard navigation, accessible controls.
- Clear text indicators, not color alone, for stale/error sync state.
- Reasonable Windows scaling and DPI support.

---

## 10. Typed backend/UI command contract

Define strongly typed Rust command arguments/results mirrored in TypeScript. Suggested commands:

```text
list_workspaces() -> WorkspaceSummary[]
select_workspace(workspaceId) -> WorkspaceSummary
add_workspace_config(configFilePath) -> WorkspaceSummary | ValidationError
get_workspace_schema(workspaceId) -> EntitySchema + ViewSchema

query_records(workspaceId, entityType, queryOptions) -> PagedRecords
get_record(recordId) -> RecordDetail
get_related_records(recordId, relationshipId, queryOptions) -> PagedRecords
get_record_documents(recordId, includeRelated) -> DocumentSummary[]
open_document(documentId) -> Result

get_sync_status(workspaceId?) -> SyncStatus
sync_now(workspaceId?, sourceId?) -> JobStatus
set_sync_paused(paused) -> SyncState
get_import_history(workspaceId?, sourceId?, page) -> ImportRun[]
get_import_errors(importId) -> ImportError[]

get_settings() -> AppSettings
update_settings(changes) -> AppSettings
```

`queryOptions` contains whitelisted filters, sort column and order, search query, and bounded pagination parameters. UI text can never interpolate into SQL syntax.

Push events for `sync.statusChanged`, `sync.sourceUpdated`, `workspace.configChanged`, and `recordCache.invalidated` (exact names can follow project conventions).

**Important:** event streams are notifications. All authoritative state can be queried from the backend after window restoration or missed events.

---

## 11. Performance requirements and measurement

Performance figures below are **targets to benchmark**, not proven guarantees. Use a modern Windows 11 laptop/desktop with an SSD and representative datasets.

| Operation | Initial target |
|---|---|
| Restore already-running hidden window | p95 < 250 ms |
| Open cached record details | p95 < 120 ms |
| Typical indexed search/filter response | p95 < 200 ms |
| Switch cached detail tabs | p95 < 120 ms |
| Scroll large tables | ~60 FPS during normal interaction |
| Cold application launch to cached UI | target < 2 s |
| Idle CPU (no active sync) | average < 1% |

Benchmark fixture scale:

- **100,000 offers**.
- **500,000 invoices**.
- **100,000 protocols**.
- **50,000 external document references**.
- Multiple workbooks with a mixture of local files and simulated unavailable network paths.

Build a repeatable benchmark executable or script, plus a written report format for p50/p95/p99 timings, import throughput, peak memory, cold vs warm launch, and foreground UI response while an import runs. Synthetic fixtures are fine. Tests and success reports must distinguish measured results from targets.

No artificial synchronous delays in navigation. Bounded UI caches and optional prefetch of commonly opened relation tabs are appropriate.

---

## 12. Reliability and security

- Source Excel access is strictly read-only.
- Workbook formulas/macros are never executed or recalculated.
- Do not trust workbook text, YAML strings, or document paths as executable content.
- Preserve last committed DB state through failed imports and crashes.
- DB migrations must be recoverable; back up user database before destructive migrations when required.
- Protect local database/log directory with per-user filesystem permissions.
- SQLite at-rest encryption is **not automatic**. Document that Windows device encryption/BitLocker may be necessary for confidential business data; optional app-layer encryption is future work.
- Avoid logging customer information, entire workbook rows, or document contents by default. Logs should contain source ID, paths only when needed, stage, row number, field and sanitized diagnostic details.
- File operations and external document opening must honor Windows user permissions.
- Installer/signing: release documentation must explain code signing (production recommendation), even if local development uses unsigned binaries.
- Use configuration schema validation and unit tests to prevent malformed user config from causing panics.

---

## 13. Reference repository layout

Use a clear structure like:

```text
excel-data-explorer/
├── README.md
├── AGENTS.md                           # concise implementation rules for coding agents
├── package.json
├── pnpm-lock.yaml                      # or npm lockfile
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/
│   ├── app/
│   ├── components/
│   │   ├── DataGrid/
│   │   ├── RecordDetails/
│   │   ├── SyncStatus/
│   │   └── WorkspacePicker/
│   ├── pages/
│   ├── hooks/
│   ├── api/                             # typed Tauri command wrappers
│   └── types/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── app_lifecycle.rs
│       ├── commands/
│       ├── config/
│       ├── db/
│       │   └── migrations/
│       ├── excel/
│       ├── indexing/
│       ├── relations/
│       ├── documents/
│       ├── sync/
│       ├── jobs/
│       └── settings/
├── workspaces/
│   └── demo-workspace.yaml
├── fixtures/
│   ├── generate_demo_data.*             # Rust, Python or Node script
│   └── expected-results.json
├── tests/
│   ├── integration/
│   └── e2e/
├── benchmarks/
└── docs/
    ├── ARCHITECTURE.md
    ├── CONFIGURATION.md
    ├── DATA_MODEL.md
    ├── SYNCHRONIZATION.md
    ├── PERFORMANCE.md
    ├── WINDOWS_RELEASE.md
    └── DECISIONS.md
```

The exact structure can vary slightly as long as boundaries remain clear.

---

## 14. Demo data and deterministic fixtures

The MVP must run without access to any private workbook. Include a generator producing:

1. `offers.xlsx` with columns `OfferID`, `Customer`, `Description`, `Amount`, `Status`, `CreatedAt`.
2. `finance.xlsx`, with `Invoices` and `Protocols` sheets linked to `offers.xlsx` by `OfferID`.
3. Sample PDF documents stored in a local `documents/` directory, referenced by paths in worksheet cells.
4. A demo YAML configuration pointing to **portable relative project paths** (do not require `D:/Company/Data` or a real network share for the demo).
5. A manifest of expected records, relation counts and document statuses.
6. Optional fixture variants with missing files, duplicate IDs, invalid types, broken links, renamed/reordered rows, and source updates.

Provide a simple command documented in README that regenerates sample inputs deterministically. The demo should be displayed on the first run or be easily selectable through the workspace picker.

---

## 15. Automated tests (minimum)

### Parsing & normalization

- Multi-worksheet workbook parsing.
- Header resolution independent of physical column order.
- Required/optional fields and type conversions.
- Stable string keys with leading zeros.
- Date and decimal semantics.
- Duplicate keys cause clear validation failures.
- Invalid workbook cannot replace successful cached data.

### Data model & relationships

- Offer → multiple invoices.
- Offer → multiple protocols.
- Unknown offer ID on invoice produces an orphan warning.
- Removing/rekeying record updates relationships correctly.
- Same key in different configured namespaces remains distinct.
- Pagination, filtering and sorting are deterministic and safe.

### Synchronization

- First import populates DB.
- Unchanged source does not trigger a reimport.
- Changed workbook updates only affected source mappings.
- Multiple sheets of one workbook activate consistently.
- Rapid changes are coalesced.
- Locked/missing/network-offline source retains cache.
- Validation failure retains previous generation.
- Interrupted staging and failed transaction recover cleanly.
- Stale source status remains visible until successful import.
- UI query remains possible while importing.

### Documents

- Relative path under configured root resolves.
- UNC path parsing is correct.
- Missing document is marked missing without crashing.
- Traversal outside root is rejected.
- Unsupported/executable extension is rejected.

### Lifecycle (Windows)

- Autostart launches hidden.
- Normal launch shows window.
- Closing hides to tray without exiting.
- Tray Open restores the existing window.
- Running executable a second time reuses first process.
- Explicit Exit terminates.
- Sync resumes after sleep/reconnect as feasible to test.

If a CI environment cannot run Windows GUI tests, document manual/automated Windows-specific verification steps and make the tests runnable on a Windows machine.

---

## 16. Acceptance tests / user stories

**A. Hidden autostart**  
Given autostart is enabled, after Windows sign-in the process is running with a tray icon but no foreground window.

**B. Reuse existing instance**  
Given it is running in the tray, launching the app from Start opens the current window without creating another background worker.

**C. Instant cached browsing**  
Given at least one successful import, the app can show offers and open details with all source Excel files temporarily disconnected.

**D. Drill-down**  
Given offer `OFF-001` has invoices `INV-1001`, `INV-1045` and protocol `PR-022`, double-clicking the offer shows the expected related items and their document links.

**E. Open PDF**  
Given a linked PDF exists within an allowed document root, selecting Open launches the Windows default PDF handler. A missing file produces a nonfatal clear error.

**F. Background update**  
Given an invoice is added to a source workbook, the next successful sync makes it appear under the correct offer without reopening the app.

**G. Failure isolation**  
Given an updated workbook has malformed required keys, sync fails with actionable errors and the previous valid cached records remain visible.

**H. Offline browsing**  
Given a mapped or UNC source is offline, the user can continue working with cached records and sees when the source was last imported successfully.

**I. Configuration adaptability**  
Given a second workspace uses different sheet/column names but valid YAML mappings, it is imported and browsed without changing Rust or React source code.

**J. Restore UI state**  
Given the user selects a workspace and filters offers, hiding and reopening the window restores a sensible prior view.

---

## 17. Milestone definition of done

Do not mark a milestone complete just because UI screens exist. Each milestone requires implementation, tests, and instructions for verifying the behavior.

### M0 — Repository, tooling, demo fixtures

Deliver:

- Tauri/React/Rust project skeleton.
- Tooling, linting, formatting and test commands.
- Demo workbook/document generator and workspace YAML.
- Architecture and decision docs.

Done when project runs, demo files can be generated, and basic test suite passes.

### M1 — Lifecycle and Windows integration

Deliver:

- Single-instance behavior.
- Hidden launch on Windows login.
- Tray menu and close-to-tray.
- Manual launch/reopen existing instance.
- Persisted preferences and explicit Exit.

Done when lifecycle acceptance tests A/B/J pass on Windows.

### M2 — Config parser and Excel ingestion

Deliver:

- Typed YAML schema + validator.
- Multiple workbooks/worksheets and per-field mapping.
- Value normalization, row diagnostics, document reference extraction.
- Deterministic fixtures and parsing unit tests.

Done when two differently structured workspaces import into a normalized representation and invalid mappings produce useful errors.

### M3 — Database/index/query core

Deliver:

- SQLite schema/migrations, indexed records, FTS, relations and documents.
- Typed Rust query services and UI command boundary.
- Source provenance and data-integrity tests.

Done when record and relationship queries work **offline**, without opening Excel files.

### M4 — Usable interactive UI

Deliver:

- Workspace picker, virtualized offer table, search/filter/sort.
- Record details, invoices/protocols/document tabs.
- Document opening, navigation, and persisted UI state.

Done when acceptance tests C/D/E/I pass end to end.

### M5 — Reliable background synchronization

Deliver:

- File watches + periodic polling.
- Stable snapshots and import queue.
- Staging, transactional activation, row hash reconciliation.
- Error handling, retry, stale status, sync controls.

Done when acceptance tests F/G/H pass and UI remains responsive during large imports.

### M6 — Performance, hardening, release

Deliver:

- Benchmark generation and measured latency/memory results.
- Failure and recovery tests.
- Windows installer, release instructions, operational logging.
- Known-limitations list, polished user documentation.

Done when high-priority acceptance tests pass, launch/reopen/sync behavior is verified on Windows, and performance targets have actual measured results or documented gaps.

**Recommended implementation order:** M0 → M2 → M3 → M4 → M5 → M1/M6 integration work as needed. Windows lifecycle can be implemented early in parallel, but the earliest demonstrable value is the offline indexed data explorer. Preserve the formal milestone definitions when reporting.

---

## 18. Required repository documentation

Create and keep these files accurate:

- `README.md`: what the app does, screenshots if available, prerequisites, dev/build/test instructions, how to run the demo, how to open it from tray, troubleshooting.
- `AGENTS.md`: concise coding guardrails and architecture invariants for future Codex work.
- `docs/ARCHITECTURE.md`: subsystem boundaries and concurrency model.
- `docs/CONFIGURATION.md`: complete YAML schema with examples and failure messages.
- `docs/DATA_MODEL.md`: actual schema/migrations/indexing choices and record identity policy.
- `docs/SYNCHRONIZATION.md`: state machine, generation activation, rollback/retry behaviors, network source caveats.
- `docs/PERFORMANCE.md`: benchmark instructions, machine specs, measured results and gaps.
- `docs/WINDOWS_RELEASE.md`: installer/build, autostart, permissions, signing and Windows-specific testing.
- `docs/DECISIONS.md`: architecture decisions/assumptions that weren't explicitly settled.

---

## 19. Final delivery contract for Codex

When finished, provide:

1. A working application repository (not pseudocode).
2. Clear commands to **install dependencies**, **generate demo data**, **run development mode**, **run tests**, and **build a Windows installer**.
3. A concise summary of implemented modules and architectural choices.
4. A test report with passing/failing tests and environment limitations.
5. Real benchmark measurements, or an explicit statement if they could not be measured.
6. Known limitations and the smallest sensible next steps.
7. Explicit verification of the default behavior:
   - **Windows startup = start hidden and synchronize in background.**
   - **Close window = keep running in system tray.**
   - **Manual launch = show/reuse the same instance.**
   - **Interactive data = read from persistent SQLite cache, not directly from Excel.**

**Do not substitute a design-only answer for the implementation.** Start by building the repository and a functioning data/cache/query path with demo files; then add UI, synchronization, lifecycle integration, testing, and packaging.

---

## 20. Short command to use with this specification

After adding this file to a repository, send Codex:

> Implement the application described in `EXCEL_DATA_EXPLORER_CODEX_SPEC.md`. Treat it as the source of truth. Start with a working repository, deterministic demo workbooks, configuration parsing, SQLite caching and relational queries; then implement the desktop UI, background synchronization, Windows tray/autostart/single-instance lifecycle, tests, and packaging. Work in milestones, keep the app runnable, and report tested functionality and remaining gaps. Do not stop at a plan.

