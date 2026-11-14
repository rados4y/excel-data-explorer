import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  ArrowDownToLine,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Columns3,
  Database,
  FilePlus2,
  FileSpreadsheet,
  FolderOpen,
  LayoutList,
  Plus,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
  Settings2,
  ShieldCheck,
  X,
} from "lucide-react";
import { api, formatTime, freshness, isTauri, label } from "./api";
import { Grid } from "./Grid";
import { Details } from "./Details";
import { SyncCenter } from "./SyncCenter";
import {
  AddExcelDialog,
  NewSpaceDialog,
  PagesDialog,
  ViewDialog,
  WorksheetDialog,
} from "./SpaceDialogs";
import type {
  Config,
  Page,
  PageDefinition,
  Scalar,
  Settings,
  Source,
  SyncStatus,
  ViewState,
  Workspace,
} from "./types";
const blank: Page = { records: [], total: 0, offset: 0, limit: 100 };
type Editor =
  | { kind: "space" }
  | { kind: "excel"; files: string[] }
  | { kind: "pages" }
  | { kind: "view"; page?: PageDefinition }
  | { kind: "worksheet"; source: Source }
  | null;
function viewKey(space: string, page: string) {
  return page === "main" ? `view:${space}` : `view:${space}:${page}`;
}
export default function App() {
  const [spaces, setSpaces] = useState<Workspace[]>([]),
    [spaceId, setSpaceId] = useState(""),
    [root, setRoot] = useState<Config | null>(null),
    [pages, setPages] = useState<PageDefinition[]>([]),
    [pageId, setPageId] = useState(""),
    [config, setConfig] = useState<Config | null>(null),
    [structureVersion, setStructureVersion] = useState(0),
    [settings, setSettings] = useState<Settings>({
      autostart: true,
      close_to_tray: true,
      sync_paused: false,
      selected_workspace: null,
    });
  const [status, setStatus] = useState<SyncStatus | null>(null),
    [view, setView] = useState<ViewState>({}),
    [loaded, setLoaded] = useState(false),
    [page, setPage] = useState<Page>(blank),
    [detail, setDetail] = useState<string | null>(null),
    [revision, setRevision] = useState(0),
    [modal, setModal] = useState<"sync" | "settings" | null>(null),
    [editor, setEditor] = useState<Editor>(null),
    [error, setError] = useState(""),
    [loading, setLoading] = useState(false),
    [search, setSearch] = useState("");
  const generation = useRef(""),
    spaceRef = useRef(spaceId),
    pageRef = useRef(pageId),
    structureTicket = useRef(0);
  spaceRef.current = spaceId;
  pageRef.current = pageId;
  const fail = useCallback((e: string) => setError(e), []);
  const activePage = pages.find((p) => p.id === pageId && !p.hidden);
  const refresh = useCallback(async () => {
    const id = spaceRef.current;
    if (!id) return;
    try {
      const s = await api("get_sync_status", { workspace_id: id });
      if (id !== spaceRef.current) return;
      setStatus(s);
      setSettings((v) => ({ ...v, sync_paused: s.paused }));
      if (s.lifecycle_error?.error) setError(s.lifecycle_error.error);
      if (s.config_errors?.length) setError(s.config_errors[0].error);
      const key = s.sources
        .map((s) => `${s.source_id}:${s.generation}`)
        .join(",");
      if (key !== generation.current) {
        generation.current = key;
        setRevision((r) => r + 1);
      }
    } catch (e) {
      setError(String(e));
    }
  }, []);
  const reloadStructure = useCallback(
    async (preferredPage?: string) => {
      const id = spaceRef.current;
      if (!id) return;
      const ticket = ++structureTicket.current;
      const [c, ps] = await Promise.all([
        api("get_workspace_schema", { workspace_id: id }),
        api("list_pages", { workspace_id: id }),
      ]);
      if (ticket !== structureTicket.current || id !== spaceRef.current) return;
      setRoot(c);
      setPages(ps);
      const preferred = preferredPage || pageRef.current;
      setPageId(
        ps.find((p) => p.id === preferred && !p.hidden)?.id ||
          ps.find((p) => !p.hidden)?.id ||
          "",
      );
      setStructureVersion((n) => n + 1);
      setSpaces(await api("list_workspaces", {}));
      await refresh();
    },
    [refresh],
  );
  useEffect(() => {
    if (!isTauri()) {
      setError(
        "Open this application in its desktop window to connect to the local cache. Run npm run desktop.",
      );
      return;
    }
    let active = true;
    Promise.all([api("list_workspaces", {}), api("get_settings", {})])
      .then(([ws, s]) => {
        if (active) {
          setSpaces(ws);
          setSettings(s);
          setSpaceId(
            s.selected_workspace &&
              ws.some((w) => w.id === s.selected_workspace)
              ? s.selected_workspace
              : ws[0]?.id || "",
          );
        }
      })
      .catch((e) => setError(String(e)));
    return () => {
      active = false;
    };
  }, []);
  useEffect(() => {
    if (!spaceId) {
      setRoot(null);
      setPages([]);
      setPageId("");
      setConfig(null);
      setLoaded(false);
      return;
    }
    let active = true;
    ++structureTicket.current;
    setLoaded(false);
    setRoot(null);
    setConfig(null);
    setPages([]);
    setPageId("");
    setPage(blank);
    setDetail(null);
    setStatus(null);
    generation.current = "";
    Promise.all([
      api("get_workspace_schema", { workspace_id: spaceId }),
      api("list_pages", { workspace_id: spaceId }),
      api("get_ui_state", { key: `navigation:${spaceId}` }),
      api("select_workspace", { workspace_id: spaceId }),
    ])
      .then(([c, ps, nav]) => {
        if (active) {
          setRoot(c);
          setPages(ps);
          setPageId(
            ps.find((p) => p.id === nav.pageId && !p.hidden)?.id ||
              ps.find((p) => !p.hidden)?.id ||
              "",
          );
          setStructureVersion((n) => n + 1);
        }
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    refresh();
    return () => {
      active = false;
    };
  }, [spaceId, refresh]);
  useEffect(() => {
    if (!spaceId || !pageId) {
      setConfig(null);
      setLoaded(false);
      return;
    }
    let active = true;
    setLoaded(false);
    setDetail(null);
    setConfig(null);
    setPage(blank);
    Promise.all([
      api("get_page_schema", { workspace_id: spaceId, page_id: pageId }),
      api("get_ui_state", { key: viewKey(spaceId, pageId) }),
    ])
      .then(([c, v]) => {
        if (!active) return;
        const columns = c.views.main.columns;
        const filters = Object.fromEntries(
          Object.entries(v.filters || {}).filter(([f]) => columns.includes(f)),
        );
        const next = {
          ...v,
          filters,
          sort: columns.includes(v.sort || "")
            ? v.sort
            : c.views.main.default_sort.field,
          direction: v.direction || c.views.main.default_sort.direction,
          offset: v.offset || 0,
        };
        setConfig(c);
        setView(next);
        setSearch(next.search || "");
        setDetail(next.detailId || null);
        setLoaded(true);
        api("save_ui_state", {
          key: `navigation:${spaceId}`,
          value: { pageId },
        }).catch((e) => setError(String(e)));
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, [spaceId, pageId, structureVersion]);
  useEffect(() => {
    if (!loaded || search === (view.search || "")) return;
    const timer = setTimeout(
      () => setView((v) => ({ ...v, search, offset: 0 })),
      220,
    );
    return () => clearTimeout(timer);
  }, [search, loaded, view.search]);
  useEffect(() => {
    if (!loaded || !config || !pageId) return;
    let active = true;
    setLoading(true);
    api("query_records", {
      workspace_id: spaceId,
      entity_type: config.views.main.entity,
      options: {
        page_id: pageId,
        search: view.search || "",
        filters: view.filters || {},
        sort: view.sort,
        direction: view.direction,
        offset: view.offset || 0,
        limit: 100,
      },
    })
      .then((r) => {
        if (active) {
          setPage(r);
          setLoading(false);
          performance.mark("cached-ui-ready");
        }
      })
      .catch((e) => {
        if (active) {
          setError(String(e));
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [
    loaded,
    config,
    spaceId,
    pageId,
    view.search,
    JSON.stringify(view.filters),
    view.sort,
    view.direction,
    view.offset,
    revision,
  ]);
  useEffect(() => {
    if (!loaded || !pageId) return;
    const timer = setTimeout(
      () =>
        api("save_ui_state", {
          key: viewKey(spaceId, pageId),
          value: view,
        }).catch((e) => setError(String(e))),
      300,
    );
    return () => clearTimeout(timer);
  }, [view, loaded, spaceId, pageId]);
  useEffect(() => {
    const timer = setInterval(refresh, 10000);
    const focus = () => {
      refresh();
    };
    window.addEventListener("focus", focus);
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !editor) {
        if (modal) setModal(null);
        else {
          setDetail(null);
          setView((v) => ({ ...v, detailId: null }));
        }
      }
    };
    window.addEventListener("keydown", key);
    const subscriptions = isTauri()
      ? [
          listen<{ page?: "settings" | "sync" }>("explorer.navigate", (e) => {
            if (e.payload.page) setModal(e.payload.page);
          }),
          listen("sync.statusChanged", refresh),
          listen<{ workspace_id?: string }>("workspace.configChanged", (e) => {
            if (
              !e.payload.workspace_id ||
              e.payload.workspace_id === spaceRef.current
            )
              reloadStructure().catch((e) => setError(String(e)));
          }),
        ]
      : [];
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", focus);
      window.removeEventListener("keydown", key);
      subscriptions.forEach((p) => p.then((f) => f()));
    };
  }, [refresh, reloadStructure, editor, modal]);
  async function sync() {
    try {
      await api("sync_now", { workspace_id: spaceId || null, source_id: null });
      await refresh();
      setRevision((r) => r + 1);
    } catch (e) {
      setError(String(e));
    }
  }
  async function addExcel() {
    try {
      const files = await invoke<string[]>("pick_excel_files");
      if (files.length) setEditor({ kind: "excel", files });
    } catch (e) {
      setError(String(e));
    }
  }
  async function addYaml() {
    try {
      const path = await invoke<string | null>("pick_workspace_config");
      if (path) {
        const w = await api("add_workspace_config", { config_file_path: path });
        await selectCreated(w);
        setModal(null);
      }
    } catch (e) {
      setError(String(e));
    }
  }
  async function selectCreated(w: Workspace) {
    setSpaces(await api("list_workspaces", {}));
    if (w.id === spaceRef.current) await reloadStructure();
    else setSpaceId(w.id);
  }
  async function changed(preferred?: string) {
    await reloadStructure(preferred);
    setRevision((n) => n + 1);
  }
  function mapping() {
    const source = root?.sources.find((s) => s.id === activePage?.source_id);
    if (source) setEditor({ kind: "worksheet", source });
    else setEditor({ kind: "pages" });
  }
  function closeDetail() {
    setDetail(null);
    setView((v) => ({ ...v, detailId: null }));
  }
  const fields =
    config?.sources.find((s) => s.entity === config.views.main.entity)
      ?.fields || {};
  const state = freshness(status),
    sourceCount = status?.sources.length || 0,
    success =
      status?.sources
        .map((s) => s.last_success_at)
        .filter((s): s is string => !!s)
        .sort()[0] || null,
    hasErrors = status?.sources.some((s) =>
      ["error", "unavailable"].includes(s.status),
    );
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-symbol">
            <FileSpreadsheet size={24} />
          </div>
          <span>
            EXCEL
            <br />
            <strong>EXPLORER</strong>
          </span>
        </div>
        <div className="nav-caption">
          PAGES & VIEWS
          {root && (
            <button
              className="icon-button"
              aria-label="Manage pages and views"
              onClick={() => setEditor({ kind: "pages" })}
            >
              <Settings2 size={13} />
            </button>
          )}
        </div>
        <nav className="page-navigation" aria-label="Pages">
          {pages
            .filter((p) => !p.hidden)
            .map((p) => (
              <button
                className={`nav-item ${p.id === pageId ? "current" : ""}`}
                key={p.id}
                aria-label={`Page ${p.title}`}
                onClick={() => setPageId(p.id)}
              >
                {p.kind === "view" ? (
                  <LayoutList size={17} />
                ) : (
                  <FileSpreadsheet size={17} />
                )}
                <span>{p.title}</span>
                {p.id === pageId && (
                  <span className="nav-count">
                    {page.total.toLocaleString()}
                  </span>
                )}
              </button>
            ))}
          {root?.sources.length === 0 && (
            <p className="nav-empty">Add Excel to create worksheet pages.</p>
          )}
          {!!root?.sources.length && (
            <button
              className="nav-item add-view-link"
              onClick={() => setEditor({ kind: "view" })}
            >
              <Plus size={16} />
              New view
            </button>
          )}
        </nav>
        <div className="nav-caption nav-space">MANAGE</div>
        <button className="nav-item" onClick={addExcel}>
          <FilePlus2 size={18} />
          Add Excel
        </button>
        <button
          className="nav-item"
          onClick={() => setEditor({ kind: "space" })}
        >
          <Plus size={18} />
          New space
        </button>
        <button
          className="nav-item"
          onClick={() => setModal("sync")}
          disabled={!spaceId}
        >
          <Activity size={18} />
          Sync center{hasErrors && <span className="error-dot" />}
        </button>
        <button className="nav-item" onClick={() => setModal("settings")}>
          <SettingsIcon size={18} />
          Settings
        </button>
        <button
          className="nav-item import-yaml-link"
          aria-label="Add workspace"
          onClick={addYaml}
        >
          <FolderOpen size={16} />
          Import YAML
        </button>
        <div className="sidebar-bottom">
          <div className="local-icon">
            <Database size={17} />
          </div>
          <div>
            <strong>Local spaces</strong>
            <small>Your data stays on this device</small>
          </div>
          <ShieldCheck size={16} />
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            Spaces <ChevronRight size={14} />
            <strong>{root?.workspace.name || "Your Excel workspace"}</strong>
            {activePage && (
              <>
                <ChevronRight size={14} />
                <span>{activePage.title}</span>
              </>
            )}
          </div>
          <div className="top-actions">
            <span className="workspace-select">
              <FileSpreadsheet size={15} />
              <select
                aria-label="Space"
                value={spaceId}
                onChange={(e) => setSpaceId(e.target.value)}
              >
                {!spaces.length && <option value="">No spaces yet</option>}
                {spaces.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
              <ChevronDown size={14} />
            </span>
            <button
              className="icon-button"
              aria-label="Settings"
              onClick={() => setModal("settings")}
            >
              <SettingsIcon size={19} />
            </button>
          </div>
        </header>
        <div className="workspace-heading">
          <div>
            <div className="eyebrow">YOUR WORKSPACE, CONNECTED</div>
            <h1>
              {config?.views.main.title ||
                root?.workspace.name ||
                "Your Excel, organized"}
              {config && (
                <span className="heading-count">
                  {page.total.toLocaleString()}
                </span>
              )}
            </h1>
            <p>
              {config
                ? `${activePage?.kind === "view" ? "Saved view" : "Worksheet page"} · Browse cached data and follow its connections.`
                : "Start with a workbook. Organize its worksheets into pages and views."}
            </p>
          </div>
          {root && (
            <button className="button" onClick={() => setModal("sync")}>
              <span className={`status-dot ${hasErrors ? "amber" : ""}`} />
              {state}
              <ChevronDown size={14} />
            </button>
          )}
        </div>
        {error && (
          <div role="alert" className="error-banner">
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="Dismiss error"
              onClick={() => setError("")}
            >
              <X size={16} />
            </button>
          </div>
        )}
        <main className="work-area">
          {!config ? (
            <section className="welcome-panel">
              <div className="welcome-symbol">
                <FileSpreadsheet size={38} />
              </div>
              <span className="eyebrow">NO CONFIGURATION FILE REQUIRED</span>
              <h2>
                {root?.sources.length
                  ? "Your pages are hidden"
                  : root
                    ? "This space is ready for Excel"
                    : "Start with your first Excel workbook"}
              </h2>
              <p>
                {root?.sources.length
                  ? "Show a worksheet page or create a view to start browsing."
                  : "Add a workbook to create a space automatically. Each worksheet becomes its own page, ready for you to configure."}
              </p>
              <div className="welcome-actions">
                <button className="button primary" onClick={addExcel}>
                  <FilePlus2 size={17} />
                  Add Excel workbook
                </button>
                {root?.sources.length ? (
                  <button
                    className="button"
                    onClick={() => setEditor({ kind: "pages" })}
                  >
                    Manage pages and views
                  </button>
                ) : (
                  <button
                    className="button"
                    onClick={() => setEditor({ kind: "space" })}
                  >
                    <Plus size={16} />
                    Create an empty space
                  </button>
                )}
              </div>
              <div className="welcome-steps">
                <span>
                  <strong>1</strong>Add Excel
                </span>
                <ChevronRight size={13} />
                <span>
                  <strong>2</strong>Configure worksheets
                </span>
                <ChevronRight size={13} />
                <span>
                  <strong>3</strong>Create your views
                </span>
              </div>
            </section>
          ) : (
            <>
              <section className="records-panel">
                <div className="table-toolbar">
                  <div className="search-box">
                    <Search size={17} />
                    <input
                      aria-label="Search records"
                      placeholder={`Search ${config.views.main.title.toLowerCase()}…`}
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                    {search && (
                      <button
                        className="icon-button"
                        aria-label="Clear search"
                        onClick={() => setSearch("")}
                      >
                        <X size={14} />
                      </button>
                    )}
                  </div>
                  <div className="toolbar-right">
                    {config.views.main.filters.map((f) => (
                      <input
                        key={f}
                        aria-label={`Filter ${fields[f]?.label || label(f)}`}
                        className="filter-input"
                        placeholder={`${fields[f]?.label || label(f)}: all`}
                        disabled={Object.hasOwn(
                          activePage?.fixed_filters || {},
                          f,
                        )}
                        value={String(
                          activePage?.fixed_filters[f] ??
                            view.filters?.[f] ??
                            "",
                        )}
                        onChange={(e) => {
                          const filters: Record<string, Scalar> = {
                            ...view.filters,
                          };
                          if (e.target.value) filters[f] = e.target.value;
                          else delete filters[f];
                          setView((v) => ({ ...v, filters, offset: 0 }));
                        }}
                      />
                    ))}
                    <button
                      className="button"
                      aria-label="Configure visible columns"
                      onClick={() =>
                        activePage &&
                        setEditor({ kind: "view", page: activePage })
                      }
                    >
                      <Columns3 size={15} />
                      Columns
                    </button>
                    <button className="button sync-button" onClick={sync}>
                      <RefreshCw
                        size={15}
                        className={
                          status?.app_state === "Synchronizing" ? "spin" : ""
                        }
                      />
                      Sync now
                    </button>
                  </div>
                </div>
                <div className="table-caption">
                  <span>
                    <span className="live-dot" />
                    {loading ? "Updating view…" : "All records"}{" "}
                    <span className="muted">
                      / {page.total.toLocaleString()} results
                    </span>
                  </span>
                  <button className="back-link" onClick={mapping}>
                    <Settings2 size={13} />
                    Worksheet settings
                  </button>
                </div>
                {activePage &&
                  Object.keys(activePage.fixed_filters).length > 0 && (
                    <div className="view-filter-summary">
                      Saved view filters:{" "}
                      {Object.entries(activePage.fixed_filters).map(
                        ([f, v]) => (
                          <span key={f}>
                            {fields[f]?.label || label(f)} = {String(v)}
                          </span>
                        ),
                      )}
                    </div>
                  )}
                <Grid
                  rows={page.records}
                  fields={fields}
                  columns={config.views.main.columns}
                  sort={view.sort}
                  direction={view.direction}
                  onSort={(sort) =>
                    setView((v) => ({
                      ...v,
                      sort,
                      direction:
                        v.sort === sort && v.direction === "asc"
                          ? "desc"
                          : "asc",
                      offset: 0,
                    }))
                  }
                  selectedId={view.selectedId}
                  onSelect={(r) => setView((v) => ({ ...v, selectedId: r.id }))}
                  onOpen={(r) => {
                    setDetail(r.id);
                    setView((v) => ({
                      ...v,
                      selectedId: r.id,
                      detailId: r.id,
                    }));
                  }}
                  widths={view.widths}
                  onWidths={(widths) => setView((v) => ({ ...v, widths }))}
                  loading={loading}
                />
                <div className="table-footer">
                  <span>
                    {page.total
                      ? `${(view.offset || 0) + 1}–${Math.min((view.offset || 0) + 100, page.total)} of ${page.total.toLocaleString()}`
                      : "0 records"}
                  </span>
                  <div className="pager">
                    <span>100 per page</span>
                    <button
                      className="icon-button"
                      aria-label="Previous page"
                      disabled={!view.offset}
                      onClick={() =>
                        setView((v) => ({
                          ...v,
                          offset: Math.max(0, (v.offset || 0) - 100),
                        }))
                      }
                    >
                      <ChevronLeft size={17} />
                    </button>
                    <button
                      className="icon-button"
                      aria-label="Next page"
                      disabled={(view.offset || 0) + 100 >= page.total}
                      onClick={() =>
                        setView((v) => ({
                          ...v,
                          offset: (v.offset || 0) + 100,
                        }))
                      }
                    >
                      <ChevronRight size={17} />
                    </button>
                  </div>
                </div>
              </section>
              {detail && (
                <Details
                  recordId={detail}
                  config={config}
                  revision={revision}
                  visibleColumns={config.views.main.columns}
                  onClose={closeDetail}
                  onError={fail}
                />
              )}
            </>
          )}
        </main>
        <footer className="statusbar">
          <span>
            <Database size={13} />
            Cached data: {formatTime(success)}
            <span className="divider">|</span>
            {sourceCount} sources<span className="divider">|</span>
            {status?.sources.filter((s) => s.last_error).length || 0} errors
          </span>
          <span>
            <ArrowDownToLine size={13} />
            Read-only source files<span className="divider">|</span>
            <span className="status-dot" />
            Local cache
          </span>
        </footer>
      </div>
      {modal && (
        <SyncCenter
          mode={modal}
          managed={root?.managed}
          workspaceId={spaceId}
          status={status}
          settings={settings}
          onSettings={(s) => {
            setSettings(s);
            refresh();
          }}
          onSync={sync}
          onClose={() => setModal(null)}
          onError={fail}
          onAdd={addYaml}
        />
      )}
      {editor?.kind === "space" && (
        <NewSpaceDialog
          onClose={() => setEditor(null)}
          onCreated={selectCreated}
        />
      )}
      {editor?.kind === "excel" && (
        <AddExcelDialog
          files={editor.files}
          spaces={spaces}
          onClose={() => setEditor(null)}
          onAdded={selectCreated}
        />
      )}
      {editor?.kind === "pages" && root && (
        <PagesDialog
          config={root}
          pages={pages}
          onClose={() => setEditor(null)}
          onChanged={() => changed()}
          onEdit={(page) => setEditor({ kind: "view", page })}
          onWorksheet={(source) => setEditor({ kind: "worksheet", source })}
          onNewView={() => setEditor({ kind: "view" })}
        />
      )}
      {editor?.kind === "view" && root && (
        <ViewDialog
          config={root}
          page={editor.page}
          initialSourceId={activePage?.source_id || undefined}
          initialFilters={view.filters}
          onClose={() => setEditor(null)}
          onSaved={changed}
        />
      )}
      {editor?.kind === "worksheet" && root && (
        <WorksheetDialog
          workspaceId={spaceId}
          source={editor.source}
          onClose={() => setEditor(null)}
          onSaved={() => changed()}
        />
      )}
    </div>
  );
}
