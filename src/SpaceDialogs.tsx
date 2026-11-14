import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  FileSpreadsheet,
  Eye,
  EyeOff,
  Plus,
  Settings2,
  X,
  ArrowUp,
  ArrowDown,
  RefreshCw,
} from "lucide-react";
import { api, label } from "./api";
import { refreshWorksheet, sourceColumns } from "./spaceModel";
import type {
  Config,
  Field,
  PageDefinition,
  Scalar,
  Source,
  WorkbookPreview,
  Workspace,
} from "./types";

export function Dialog(p: {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const timer = setTimeout(
      () =>
        ref.current?.querySelector<HTMLElement>("input,button,select")?.focus(),
      0,
    );
    return () => {
      clearTimeout(timer);
      previous?.focus();
    };
  }, []);
  return (
    <div
      className="modal-shade"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) p.onClose();
      }}
    >
      <section
        ref={ref}
        className={`modal ${p.wide ? "wide-modal" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={p.title}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            p.onClose();
          }
          if (e.key === "Tab") {
            const controls = ref.current?.querySelectorAll<HTMLElement>(
              "input:not(:disabled),button:not(:disabled),select:not(:disabled),textarea:not(:disabled)",
            );
            if (controls?.length) {
              const first = controls[0],
                last = controls[controls.length - 1];
              if (e.shiftKey && document.activeElement === first) {
                e.preventDefault();
                last.focus();
              } else if (!e.shiftKey && document.activeElement === last) {
                e.preventDefault();
                first.focus();
              }
            }
          }
        }}
      >
        <div className="modal-heading">
          <div>
            <span className="eyebrow">SPACES & PAGES</span>
            <h2>{p.title}</h2>
            {p.description && (
              <p className="dialog-description">{p.description}</p>
            )}
          </div>
          <button
            className="icon-button"
            aria-label="Close dialog"
            onClick={p.onClose}
          >
            <X size={20} />
          </button>
        </div>
        <div className="modal-body space-form">{p.children}</div>
      </section>
    </div>
  );
}
function Failure({ message }: { message: string }) {
  return message ? (
    <p role="alert" className="inline-error">
      {message}
    </p>
  ) : null;
}

export function NewSpaceDialog(p: {
  onClose: () => void;
  onCreated: (workspace: Workspace) => Promise<void>;
}) {
  const [name, setName] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  return (
    <Dialog
      title="Create a space"
      description="A space contains workbook pages and your saved views. You can add Excel files later."
      onClose={p.onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          try {
            const w = await api("create_space", { name });
            await p.onCreated(w);
            p.onClose();
          } catch (e) {
            setError(String(e));
            setBusy(false);
          }
        }}
      >
        <label className="form-field">
          Space name
          <input
            required
            maxLength={200}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Finance, Sales, Project North"
          />
        </label>
        <Failure message={error} />
        <div className="form-actions">
          <button type="button" className="button" onClick={p.onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={busy || !name.trim()}>
            {busy ? "Creating…" : "Create space"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}

export function AddExcelDialog(p: {
  files: string[];
  spaces: Workspace[];
  onClose: () => void;
  onAdded: (workspace: Workspace) => Promise<void>;
}) {
  const [previews, setPreviews] = useState<WorkbookPreview[]>([]),
    [loading, setLoading] = useState(true),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [target, setTarget] = useState(""),
    [names, setNames] = useState<Record<string, string>>({}),
    [hidden, setHidden] = useState<Record<string, string[]>>({}),
    [selected, setSelected] = useState<Record<string, string>>({});
  const inspections = useRef(new Map<string, Promise<WorkbookPreview>>());
  const completed = useRef(new Map<string, Workspace>());
  const cancelled = useRef(false);
  useEffect(() => {
    cancelled.current = false;
    return () => {
      cancelled.current = true;
    };
  }, []);
  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const results = [];
        for (const path of p.files) {
          if (!active) return;
          let inspection = inspections.current.get(path);
          if (!inspection) {
            inspection = api("inspect_workbook", { file_path: path });
            inspections.current.set(path, inspection);
          }
          const preview = await inspection;
          if (!active) return;
          results.push(preview);
        }
        if (active) {
          setPreviews(results);
          setLoading(false);
        }
      } catch (e) {
        if (active) {
          setError(String(e));
          setLoading(false);
        }
      }
    })();
    return () => {
      active = false;
    };
  }, [p.files]);
  async function add() {
    setBusy(true);
    setError("");
    try {
      for (const file of previews) {
        if (cancelled.current) return;
        let w = completed.current.get(file.path);
        if (!w) {
          w = await api("add_excel_workbook", {
            file_path: file.path,
            space_id: target || null,
            name: names[file.path] || file.name,
            hidden_sheets: hidden[file.path] || [],
          });
          completed.current.set(file.path, w);
        }
        await p.onAdded(w);
        if (cancelled.current) return;
        await api("sync_now", { workspace_id: w.id, source_id: null });
        await p.onAdded(w);
      }
      if (!cancelled.current) p.onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }
  return (
    <Dialog
      title="Add Excel workbooks"
      description="Each workbook creates a separate space by default. Every worksheet becomes a page; no YAML file is needed."
      onClose={p.onClose}
      wide
    >
      {loading ? (
        <div className="import-loading">
          <RefreshCw className="spin" size={22} />
          <strong>Inspecting workbook sheets and headers…</strong>
          <span>Reading a stable snapshot. Source files stay read-only.</span>
        </div>
      ) : (
        <>
          {previews.length > 0 && (
            <label className="form-field">
              Where to add the workbook
              <select
                value={target}
                onChange={(e) => setTarget(e.target.value)}
              >
                <option value="">New space for each workbook (default)</option>
                {p.spaces.map((s) => (
                  <option key={s.id} value={s.id}>
                    Existing space: {s.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          {previews.map((file) => {
            const current =
              file.worksheets.find((s) => s.name === selected[file.path]) ||
              file.worksheets[0];
            return (
              <article className="workbook-preview" key={file.path}>
                <div className="preview-heading">
                  <FileSpreadsheet size={24} />
                  <div>
                    <strong>{file.name}</strong>
                    <small>
                      {file.worksheets.length} worksheet
                      {file.worksheets.length === 1 ? "" : "s"} ·{" "}
                      {file.worksheets
                        .reduce((n, s) => n + s.row_count, 0)
                        .toLocaleString()}{" "}
                      populated data rows
                    </small>
                  </div>
                </div>
                {!target && (
                  <label className="form-field">
                    Space name for {file.name}
                    <input
                      aria-label={`Space name for ${file.name}`}
                      maxLength={200}
                      value={names[file.path] ?? file.name}
                      onChange={(e) =>
                        setNames((n) => ({ ...n, [file.path]: e.target.value }))
                      }
                    />
                  </label>
                )}
                <div className="worksheet-choices">
                  {file.worksheets.map((sheet) => (
                    <label key={sheet.name}>
                      <input
                        type="checkbox"
                        aria-label={`Show ${sheet.name} worksheet`}
                        checked={
                          !(hidden[file.path] || []).includes(sheet.name)
                        }
                        onChange={(e) =>
                          setHidden((h) => ({
                            ...h,
                            [file.path]: e.target.checked
                              ? (h[file.path] || []).filter(
                                  (n) => n !== sheet.name,
                                )
                              : [...(h[file.path] || []), sheet.name],
                          }))
                        }
                      />
                      <span>
                        {sheet.name}
                        <small>
                          {sheet.columns.length} columns · {sheet.row_count}{" "}
                          rows{sheet.empty ? " · empty worksheet" : ""}
                        </small>
                      </span>
                    </label>
                  ))}
                </div>
                <p className="small muted">
                  Unchecked worksheets are hidden from navigation. Their data
                  remains in the cache and can be shown in Pages and views.
                  Initial fields keep their source values as text; configure
                  types and a unique ID after adding.
                </p>
                {current && (
                  <>
                    <label className="form-field">
                      Preview worksheet
                      <select
                        value={current.name}
                        onChange={(e) =>
                          setSelected((s) => ({
                            ...s,
                            [file.path]: e.target.value,
                          }))
                        }
                      >
                        {file.worksheets.map((s) => (
                          <option key={s.name}>{s.name}</option>
                        ))}
                      </select>
                    </label>
                    <div className="sheet-preview-table">
                      <table>
                        <thead>
                          <tr>
                            {current.columns.slice(0, 12).map((c) => (
                              <th key={c.id}>{c.label}</th>
                            ))}
                          </tr>
                        </thead>
                        <tbody>
                          {current.rows.slice(0, 5).map((row, i) => (
                            <tr key={i}>
                              {row.map((v, j) => (
                                <td key={j}>{String(v ?? "")}</td>
                              ))}
                            </tr>
                          ))}
                        </tbody>
                      </table>
                      {current.rows.length === 0 && (
                        <p className="muted">
                          No data rows yet. This worksheet still gets its own
                          page.
                        </p>
                      )}
                    </div>
                  </>
                )}
              </article>
            );
          })}
        </>
      )}
      <Failure message={error} />
      <div className="form-actions">
        <button className="button" onClick={p.onClose}>
          Cancel
        </button>
        <button
          className="button primary"
          disabled={loading || busy || previews.length === 0}
          onClick={add}
        >
          {busy ? "Adding and starting sync…" : "Add workbooks"}
        </button>
      </div>
    </Dialog>
  );
}

export function PagesDialog(p: {
  config: Config;
  pages: PageDefinition[];
  onClose: () => void;
  onChanged: () => Promise<void>;
  onEdit: (page: PageDefinition) => void;
  onWorksheet: (source: Source) => void;
  onNewView: () => void;
}) {
  const [name, setName] = useState(p.config.workspace.name),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  async function toggle(page: PageDefinition) {
    setBusy(true);
    try {
      await api("save_page", {
        workspace_id: p.config.workspace.id,
        page: { ...page, hidden: !page.hidden },
      });
      await p.onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog
      title="Pages and views"
      description="Organize this space. Hiding pages or columns changes this user's display; it is not an access-permission system."
      onClose={p.onClose}
    >
      <form
        className="rename-space"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          try {
            await api("rename_space", {
              workspace_id: p.config.workspace.id,
              name,
            });
            await p.onChanged();
          } catch (e) {
            setError(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <label className="form-field">
          Space name
          <input
            value={name}
            maxLength={200}
            required
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <button className="button" disabled={busy}>
          Rename space
        </button>
      </form>
      <div className="dialog-section-heading">
        <h3>Pages in this space</h3>
        <button
          className="button"
          onClick={p.onNewView}
          disabled={!p.config.sources.length}
        >
          <Plus size={15} />
          New view
        </button>
      </div>
      {p.pages.length === 0 && (
        <p className="muted small">
          No pages yet. Add an Excel workbook to this space.
        </p>
      )}
      {p.pages.map((page) => (
        <div className="page-settings-row" key={page.id}>
          <span>
            <strong>{page.title}</strong>
            <small>
              {page.kind === "view" ? "Saved view" : "Worksheet page"} ·{" "}
              {page.columns.length} visible columns
              {page.hidden ? " · hidden" : ""}
            </small>
          </span>
          <button
            className="button"
            disabled={busy}
            onClick={() => toggle(page)}
            aria-label={`${page.hidden ? "Show" : "Hide"} page ${page.title}`}
          >
            {page.hidden ? <Eye size={15} /> : <EyeOff size={15} />}{" "}
            {page.hidden ? "Show" : "Hide"}
          </button>
          <button
            className="icon-button"
            aria-label={`Edit page ${page.title}`}
            onClick={() => p.onEdit(page)}
          >
            <Settings2 size={17} />
          </button>
        </div>
      ))}
      <h3>Worksheet mappings</h3>
      {p.config.sources.map((source) => (
        <button
          className="worksheet-mapping-link"
          key={source.id}
          onClick={() => p.onWorksheet(source)}
        >
          <FileSpreadsheet size={17} />
          <span>
            <strong>{source.sheet}</strong>
            <small>
              {source.path.split(/[\\/]/).at(-1)} · {source.namespace}
            </small>
          </span>
          <Settings2 size={16} />
        </button>
      ))}
      <Failure message={error} />
    </Dialog>
  );
}

export function ViewDialog(p: {
  config: Config;
  page?: PageDefinition;
  initialSourceId?: string;
  initialFilters?: Record<string, Scalar>;
  onClose: () => void;
  onSaved: (id: string) => Promise<void>;
}) {
  const start =
    p.config.sources.find(
      (s) => s.id === (p.page?.source_id || p.initialSourceId),
    ) ||
    p.config.sources.find((s) => s.entity === p.page?.entity) ||
    p.config.sources[0];
  const [sourceId, setSourceId] = useState(start?.id || ""),
    [title, setTitle] = useState(p.page?.title || "New view"),
    [columns, setColumns] = useState<string[]>(
      p.page?.columns || sourceColumns(start),
    ),
    [sort, setSort] = useState(
      p.page?.default_sort || {
        field:
          p.page?.columns[0] ||
          Object.keys(start?.fields || {}).find((f) => f !== "__row_id") ||
          "__row_id",
        direction: "asc" as "asc" | "desc",
      },
    ),
    [filterRows, setFilterRows] = useState<[string, string][]>(
      Object.entries(p.page?.fixed_filters || p.initialFilters || {}).map(
        ([k, v]) => [k, String(v)],
      ),
    ),
    [hidden, setHidden] = useState(p.page?.hidden || false),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const source = p.config.sources.find((s) => s.id === sourceId) || start;
  const fields = source?.fields || {};
  const choices = [
    ...columns,
    ...Object.keys(fields).filter((f) => !columns.includes(f)),
  ];
  function toggle(field: string, checked: boolean) {
    const next = checked
      ? [...columns, field]
      : columns.filter((f) => f !== field);
    setColumns(next);
    if (!next.includes(sort.field))
      setSort((s) => ({ ...s, field: next[0] || "" }));
    setFilterRows((rows) => rows.filter(([f]) => next.includes(f)));
  }
  async function save(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      const filters = Object.fromEntries(filterRows.filter(([k]) => k));
      let saved: PageDefinition;
      if (p.page) {
        saved = await api("save_page", {
          workspace_id: p.config.workspace.id,
          page: {
            ...p.page,
            title: title.trim(),
            columns,
            default_sort: sort,
            search_fields: columns.filter(
              (f) =>
                fields[f]?.searchable ||
                f === source?.key ||
                p.config.views.main.search_fields.includes(f),
            ),
            filters: [
              ...new Set([
                ...p.page.filters.filter((f) => columns.includes(f)),
                ...Object.keys(filters),
              ]),
            ],
            fixed_filters: filters,
            hidden,
          },
        });
      } else {
        saved = await api("create_view", {
          workspace_id: p.config.workspace.id,
          source_id: sourceId,
          title,
          columns,
          filters,
          sort,
        });
      }
      await p.onSaved(saved.id);
      p.onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }
  return (
    <Dialog
      title={p.page ? "Edit page" : "Create a view"}
      description="Choose visible columns and a default sort. Saved exact-match filters create a focused view without duplicating Excel data."
      onClose={p.onClose}
    >
      <form onSubmit={save}>
        <div className="form-two-columns">
          <label className="form-field">
            {p.page ? "Page title" : "View name"}
            <input
              required
              maxLength={200}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </label>
          {!p.page && (
            <label className="form-field">
              Source worksheet
              <select
                value={sourceId}
                onChange={(e) => {
                  const s = p.config.sources.find(
                    (s) => s.id === e.target.value,
                  )!;
                  const fields = sourceColumns(s);
                  const next = fields.length ? fields : ["__row_id"];
                  setSourceId(s.id);
                  setColumns(next);
                  setSort({ field: next[0], direction: "asc" });
                  setFilterRows([]);
                }}
              >
                {p.config.sources.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.sheet} · {s.path.split(/[\\/]/).at(-1)}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
        <h3>Visible columns</h3>
        <p className="small muted">
          These columns appear in this page's table and record overview. Hidden
          columns stay in the source/cache. Keep at least one visible.
        </p>
        <div className="column-choice-list">
          {choices.map((field) => (
            <div key={field} className="column-choice">
              <label>
                <input
                  type="checkbox"
                  aria-label={`Show column ${fields[field]?.label || label(field)}`}
                  checked={columns.includes(field)}
                  onChange={(e) => toggle(field, e.target.checked)}
                />
                {fields[field]?.label || label(field)}
              </label>
              {columns.includes(field) && (
                <span>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label={`Move ${field} up`}
                    disabled={columns.indexOf(field) === 0}
                    onClick={() =>
                      setColumns((c) => {
                        const n = [...c],
                          i = n.indexOf(field);
                        [n[i - 1], n[i]] = [n[i], n[i - 1]];
                        return n;
                      })
                    }
                  >
                    <ArrowUp size={13} />
                  </button>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label={`Move ${field} down`}
                    disabled={columns.indexOf(field) === columns.length - 1}
                    onClick={() =>
                      setColumns((c) => {
                        const n = [...c],
                          i = n.indexOf(field);
                        [n[i + 1], n[i]] = [n[i], n[i + 1]];
                        return n;
                      })
                    }
                  >
                    <ArrowDown size={13} />
                  </button>
                </span>
              )}
            </div>
          ))}
        </div>
        <div className="form-two-columns">
          <label className="form-field">
            Sort by
            <select
              value={sort.field}
              onChange={(e) =>
                setSort((s) => ({ ...s, field: e.target.value }))
              }
            >
              {columns.map((f) => (
                <option key={f} value={f}>
                  {fields[f]?.label || label(f)}
                </option>
              ))}
            </select>
          </label>
          <label className="form-field">
            Sort direction
            <select
              value={sort.direction}
              onChange={(e) =>
                setSort((s) => ({
                  ...s,
                  direction: e.target.value as "asc" | "desc",
                }))
              }
            >
              <option value="asc">Ascending</option>
              <option value="desc">Descending</option>
            </select>
          </label>
        </div>
        <div className="dialog-section-heading">
          <h3>Saved filters</h3>
          <button
            type="button"
            className="button"
            disabled={!columns.length}
            onClick={() => setFilterRows((r) => [...r, [columns[0], ""]])}
          >
            <Plus size={14} />
            Add filter
          </button>
        </div>
        {filterRows.map(([field, value], i) => (
          <div className="saved-filter" key={i}>
            <select
              aria-label={`Filter field ${i + 1}`}
              value={field}
              onChange={(e) =>
                setFilterRows((rows) =>
                  rows.map((r, j) => (j === i ? [e.target.value, r[1]] : r)),
                )
              }
            >
              {columns.map((f) => (
                <option key={f} value={f}>
                  {fields[f]?.label || label(f)}
                </option>
              ))}
            </select>
            <span>equals</span>
            <input
              aria-label={`Filter value ${i + 1}`}
              value={value}
              onChange={(e) =>
                setFilterRows((rows) =>
                  rows.map((r, j) => (j === i ? [r[0], e.target.value] : r)),
                )
              }
              placeholder="Exact value, e.g. Active"
            />
            <button
              type="button"
              className="icon-button"
              aria-label={`Remove filter ${i + 1}`}
              onClick={() =>
                setFilterRows((rows) => rows.filter((_, j) => j !== i))
              }
            >
              <X size={15} />
            </button>
          </div>
        ))}
        {p.page && (
          <label className="check-field">
            <input
              type="checkbox"
              checked={!hidden}
              onChange={(e) => setHidden(!e.target.checked)}
            />
            Show this page in the sidebar
          </label>
        )}
        <Failure message={error} />
        <div className="form-actions">
          <button type="button" className="button" onClick={p.onClose}>
            Cancel
          </button>
          <button
            className="button primary"
            disabled={busy || !columns.length || !title.trim()}
          >
            {busy ? "Saving…" : p.page ? "Save page" : "Create view"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}

export function WorksheetDialog(p: {
  workspaceId: string;
  source: Source;
  onClose: () => void;
  onSaved: () => Promise<void>;
}) {
  const [draft, setDraft] = useState<Source>(structuredClone(p.source)),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const fields = Object.entries(draft.fields)
    .filter(([id]) => id !== "__row_id")
    .sort(
      (a, b) =>
        (a[1].column_index ?? (Number(a[0].replace("column_", "")) || 99999)) -
        (b[1].column_index ?? (Number(b[0].replace("column_", "")) || 99999)),
    );
  function field(id: string, changes: Partial<Field>) {
    setDraft((d) => ({
      ...d,
      fields: { ...d.fields, [id]: { ...d.fields[id], ...changes } },
    }));
  }
  async function refreshHeaders() {
    setBusy(true);
    try {
      const preview = await api("inspect_workbook", {
        file_path: draft.path,
        sheet: draft.sheet,
        header_row: draft.header_row,
      });
      const columns = preview.worksheets[0].columns;
      setDraft(refreshWorksheet(draft, columns));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog
      title={`Worksheet settings: ${draft.sheet}`}
      description="Map source columns and choose field types. Changes are validated before saving; failed reimports keep the last valid cached records."
      onClose={p.onClose}
      wide
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          try {
            await api("update_worksheet", {
              workspace_id: p.workspaceId,
              source: draft,
            });
            await api("sync_now", {
              workspace_id: p.workspaceId,
              source_id: draft.id,
            });
            await p.onSaved();
            p.onClose();
          } catch (e) {
            setError(String(e));
            setBusy(false);
          }
        }}
      >
        <div className="form-two-columns">
          <label className="form-field">
            Header row
            <input
              aria-label="Header row"
              type="number"
              min={1}
              max={1048576}
              value={draft.header_row}
              onChange={(e) =>
                setDraft((d) => ({ ...d, header_row: Number(e.target.value) }))
              }
            />
          </label>
          <label className="form-field">
            Unique record ID
            <select
              aria-label="Unique record ID"
              value={draft.key_mode === "row_number" ? "__row_id" : draft.key}
              onChange={(e) => {
                const key = e.target.value;
                setDraft((d) => {
                  const fields = { ...d.fields };
                  if (key !== "__row_id") delete fields.__row_id;
                  fields[key] = {
                    ...(fields[key] || {
                      column: "__row_id",
                      column_index: null,
                      label: "Row",
                      trim: false,
                      searchable: false,
                      filterable: false,
                    }),
                    type: "string",
                    required: true,
                  };
                  return {
                    ...d,
                    key,
                    key_mode: key === "__row_id" ? "row_number" : "field",
                    fields,
                  };
                });
              }}
            >
              <option value="__row_id">Automatic row number (fallback)</option>
              {fields.map(([id, f]) => (
                <option key={id} value={id}>
                  {f.label || f.column}
                </option>
              ))}
            </select>
          </label>
        </div>
        {draft.key_mode === "row_number" && (
          <p className="mapping-note">
            Row numbers allow duplicate data and sheets without IDs. They follow
            worksheet positions when rows move. Choose a unique ID column for
            stable identities across reordering.
          </p>
        )}
        <button
          type="button"
          className="button"
          onClick={refreshHeaders}
          disabled={busy}
        >
          <RefreshCw size={14} />
          Read headers from workbook
        </button>
        <p className="small muted">
          Use Read headers after choosing another header row or after
          adding/removing Excel columns. Indexed columns use their physical
          position; a blank position uses the exact header name.
        </p>
        <div className="mapping-table">
          <table>
            <thead>
              <tr>
                <th>Excel header</th>
                <th>Column #</th>
                <th>Display label</th>
                <th>Type</th>
                <th>Required</th>
                <th>Trim</th>
                <th>Search</th>
                <th>Filter</th>
              </tr>
            </thead>
            <tbody>
              {fields.map(([id, f]) => (
                <tr key={id}>
                  <td>
                    <input
                      aria-label={`Header for ${f.column}`}
                      value={f.column}
                      onChange={(e) => field(id, { column: e.target.value })}
                    />
                  </td>
                  <td>
                    <input
                      aria-label={`Column position for ${f.column}`}
                      type="number"
                      min={1}
                      max={16384}
                      value={f.column_index ?? ""}
                      onChange={(e) =>
                        field(id, {
                          column_index: e.target.value
                            ? Number(e.target.value)
                            : null,
                        })
                      }
                    />
                  </td>
                  <td>
                    <input
                      aria-label={`Label for ${f.column}`}
                      value={f.label || ""}
                      onChange={(e) => field(id, { label: e.target.value })}
                    />
                  </td>
                  <td>
                    <select
                      aria-label={`Type for ${f.column}`}
                      disabled={draft.key === id}
                      value={f.type}
                      onChange={(e) =>
                        field(id, { type: e.target.value as Field["type"] })
                      }
                    >
                      {[
                        "string",
                        "integer",
                        "decimal",
                        "boolean",
                        "date",
                        "datetime",
                      ].map((type) => (
                        <option key={type}>{type}</option>
                      ))}
                    </select>
                  </td>
                  {(
                    ["required", "trim", "searchable", "filterable"] as const
                  ).map((option) => (
                    <td key={option}>
                      <input
                        type="checkbox"
                        aria-label={`${option} for ${f.column}`}
                        disabled={option === "required" && draft.key === id}
                        checked={f[option]}
                        onChange={(e) =>
                          field(id, { [option]: e.target.checked })
                        }
                      />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
          {fields.length === 0 && (
            <p className="muted small">
              No mapped columns. Read headers when this worksheet contains data.
            </p>
          )}
        </div>
        <Failure message={error} />
        <div className="form-actions">
          <button type="button" className="button" onClick={p.onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={busy}>
            {busy ? "Saving and syncing…" : "Save mapping and sync"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
