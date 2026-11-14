# Configuration contract (version 1)
The accepted YAML is deserialized into the structs in `src-tauri/src/config.rs`. Unknown keys and unknown enum values are rejected. `workspaces/demo-workspace.yaml` is the complete reference profile; `alternate-workspace.yaml` demonstrates different headers/entity/key names. There is no scripting or arbitrary SQL.

## Root keys
- `version`: required integer, exactly 1.
- `workspace`: required `{id, name}` strings. IDs are 1–100 ASCII letters/digits/underscore/hyphen.
- `roots`: required map from root name to directory path. Relative directories resolve against the YAML file, not the working directory. Root paths cannot contain placeholders. Local paths, mapped drives and UNC roots work on Windows.
- `settings`: optional object. Defaults: `timezone: UTC`, `local_scan_seconds: 60`, `network_scan_seconds: 300`, `stable_file_check_ms: 1500`, `stable_file_attempts: 3`, `read_only: true`. Scan intervals must be ≥5 seconds; stable interval is 1–60,000 ms, attempts 1–10; read_only must be true. Timezone is a valid IANA name (e.g. Europe/Warsaw).
- `sources`: required nonempty list, described below.
- `relationships`: optional list, default empty.
- `documents`: optional list, default empty.
- `views`: required `{main, detail}`.

## Sources and fields
Each source requires `id`, `path`, `sheet`, `entity`, `key`, `fields`; optional `namespace` defaults to the source ID; `header_row` defaults to 1. `path` expands only `${rootName}` using `roots`; unknown/unexpanded placeholders are errors. Paths are compared after portable relative resolution. Prefer a single consistent path spelling for one physical workbook; Windows drive letters and UNC shares are interpreted by Windows.

`fields` maps internal field names to `{column, type, required?, trim?, searchable?, filterable?, label?}`. `column` is the exact Excel header name, independent of its physical position. The supported types are `string`, `integer`, `decimal`, `boolean`, `date`, `datetime`. Boolean options default false; label defaults to a humanized field name. The key field must be a required string. All mapped headers must exist; blank/duplicate headers cannot hide missing columns. All sources for one entity must contain compatible types for referenced view/relation fields.

Normalization never case-folds or strips leading zeros. `trim: true` trims surrounding whitespace only. Excel text keys preserve exact strings; numeric keys are converted to plain strings, with numbers beyond IEEE-754's exact integer range rejected. Store identifiers with leading zeros or >15 significant digits as Excel **Text**, not a custom numeric display format.

Empty optional cells become null. Invalid optional values become null with a warning. Invalid required cells reject the complete physical workbook import. Diagnostics name source, worksheet, one-based Excel row, field and error code. Original cell text is retained privately in raw_json for local diagnostics; customer values are not logged. Duplicate keys in the same entity/namespace fail with a source/row message. A shared namespace must be explicitly configured for cross-source relations; distinct namespaces keep duplicate keys distinct.

Integers must be exact whole signed 64-bit values. Decimals are canonical decimal strings using Rust Decimal (up to its supported precision); source floating-point cells necessarily inherit Excel's numeric precision. Exact strings remain authoritative; the numeric projection is only for sorting. Boolean values accept true/false/1/0, case insensitive. Date values accept ISO YYYY-MM-DD or Excel date cells and remain date-only. Datetimes normalize to fixed-precision UTC RFC3339 instants so indexed chronological sorting is consistent across offsets; inputs accept RFC3339 offsets; naive ISO timestamps are interpreted in the configured IANA timezone and ambiguous/nonexistent DST times fail normalization. No arbitrary UTC offset is assigned to date-only cells. Formulas use stored results; missing cached results produce diagnostics. Macros and formulas are never executed/recalculated.

## Relationships
`{id, parent_entity, child_entity, parent_field, child_field}`. All values are required. Joins require equal normalized field text, current workspace and matching namespace. Field types must match. One-to-many edges are indexed. Missing parents produce warnings and do not remove children. A non-key parent field can yield multiple parent matches when its value is not unique; configure the parent business key for ordinary one-to-many relations.

## Documents
`{id, entity, path_field, root, allowed_extensions, allow_absolute?}`. `root` names an existing configured directory. `path_field` must be a string field on that entity. `allow_absolute` defaults false; even enabled absolute references must remain inside the mapping's allowed root. Only safe extension allowlists are accepted: pdf, docx, xlsx, txt, png, jpg, jpeg. Executables, scripts, URLs, alternate streams, traversal and canonical symlink/junction escapes are rejected. Missing files remain visible. Empty references create no link. Only cached document IDs can be opened by the frontend.

## Views
`main` requires `entity`, `title`, `columns` (nonempty field-name list), `default_sort: {field, direction: asc|desc}`; optional `search_fields` and `filters` default empty. All referenced fields must exist. The UI shows configured labels from field definitions. Search is literal token-prefix FTS over the configured fields; filters are exact values (case sensitive). Columns can be resized. MVP sorting uses one column at a time.

`detail` requires `entity` (same as main), `title_field`, `tabs`. A tab is `{title, type, relationship?}`; type is `fields`, `relationship`, or `related_documents`. Relationship tabs require a known relation whose parent is the main entity. Child record drill-down automatically exposes its fields and documents. The document tab includes directly related children and deduplicates cached document identities. It returns up to 1,000 links with an explicit UI cap label.

## Errors and activation
Examples: `Source invoices sheet Invoices missing column InvoiceID`; `Unknown field offer.missing in source offers`; `Duplicate source ID: offers`; `Source offers sheet Offers row 4 field id: Required value is empty`; `Document traversal is not permitted`.

Add/reload validates before activating; invalid YAML keeps the persisted last-known-good profile and cache. The worker also checks valid config changes at scan time. Config errors are persisted and visible in sync status. A changed mapping forces a new source fingerprint. Source cache generations remain visible while mappings are being reimported; the command schema may already reflect the valid new profile. Removed source mappings retire only their owned records. For disruptive entity/key changes, prefer a new workspace ID until its first successful import.
