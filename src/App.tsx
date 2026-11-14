import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  ArrowDownToLine,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Database,
  FileSpreadsheet,
  FolderOpen,
  LayoutList,
  Plus,
  RefreshCw,
  Search,
  Settings as SettingsIcon,
  ShieldCheck,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { api, formatTime, freshness, isTauri, label } from "./api";
import { Grid } from "./Grid";
import { Details } from "./Details";
import { SyncCenter } from "./SyncCenter";
import type {
  Config,
  Page,
  Scalar,
  Settings,
  SyncStatus,
  ViewState,
  Workspace,
} from "./types";
const blank: Page = { records: [], total: 0, offset: 0, limit: 100 };
export default function App() {
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]),
    [workspaceId, setWorkspaceId] = useState(""),
    [config, setConfig] = useState<Config | null>(null),
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
    [error, setError] = useState(""),
    [loading, setLoading] = useState(false),
    [search, setSearch] = useState(""),
    [searching, setSearching] = useState(false);
  const errorHandler = useCallback((e: string) => setError(e), []);
  const generation = useRef("");
  const workspaceRef = useRef(workspaceId);
  workspaceRef.current = workspaceId;
  const refresh = useCallback(async () => {
    if (!workspaceRef.current) return;
    try {
      const s = await api("get_sync_status", {
        workspace_id: workspaceRef.current,
      });
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
  useEffect(() => {
    if (!isTauri()) {
      setError(
        "Open this application in its desktop window to connect to the local cache. Run npm run desktop.",
      );
      return;
    }
    Promise.all([api("list_workspaces", {}), api("get_settings", {})])
      .then(([ws, s]) => {
        setWorkspaces(ws);
        setSettings(s);
        setWorkspaceId(
          s.selected_workspace && ws.some((w) => w.id === s.selected_workspace)
            ? s.selected_workspace
            : ws[0]?.id || "",
        );
      })
      .catch((e) => setError(String(e)));
  }, []);
  useEffect(() => {
    if (!workspaceId) return;
    let active = true;
    setLoaded(false);
    setConfig(null);
    setPage(blank);
    setDetail(null);
    generation.current = "";
    Promise.all([
      api("get_workspace_schema", { workspace_id: workspaceId }),
      api("get_ui_state", { key: `view:${workspaceId}` }),
      api("select_workspace", { workspace_id: workspaceId }),
    ])
      .then(([c, v]) => {
        if (active) {
          setConfig(c);
          const next = {
            sort: c.views.main.default_sort.field,
            direction: c.views.main.default_sort.direction,
            offset: 0,
            ...v,
          };
          setView(next);
          setSearch(next.search || "");
          setDetail(next.detailId || null);
          setLoaded(true);
        }
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    refresh();
    return () => {
      active = false;
    };
  }, [workspaceId]);
  useEffect(() => {
    if (!loaded || search === (view.search || "")) return;
    setSearching(true);
    const timer = setTimeout(() => {
      setView((v) => ({ ...v, search, offset: 0 }));
      setSearching(false);
    }, 220);
    return () => clearTimeout(timer);
  }, [search, loaded, view.search]);
  useEffect(() => {
    if (!loaded || !config) return;
    let active = true;
    setLoading(true);
    api("query_records", {
      workspace_id: workspaceId,
      entity_type: config.views.main.entity,
      options: {
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
    workspaceId,
    view.search,
    JSON.stringify(view.filters),
    view.sort,
    view.direction,
    view.offset,
    revision,
  ]);
  useEffect(() => {
    if (!loaded) return;
    const timer = setTimeout(() => {
      api("save_ui_state", { key: `view:${workspaceId}`, value: view }).catch(
        (e) => setError(String(e)),
      );
    }, 300);
    return () => clearTimeout(timer);
  }, [view, loaded, workspaceId]);
  useEffect(() => {
    const timer = setInterval(refresh, 10000);
    const focus = () => {
      refresh();
    };
    window.addEventListener("focus", focus);
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (modal) setModal(null);
        else {
          setDetail(null);
          setView((v) => ({ ...v, detailId: null }));
        }
      }
    };
    window.addEventListener("keydown", key);
    const unlisten = isTauri()
      ? listen<{ page?: "settings" | "sync" }>("explorer.navigate", (e) => {
          if (e.payload.page) setModal(e.payload.page);
        })
      : Promise.resolve(() => {});
    const unstatus = isTauri()
      ? listen("sync.statusChanged", refresh)
      : Promise.resolve(() => {});
    const unconfig = isTauri()
      ? listen("workspace.configChanged", () => {
          api("get_workspace_schema", { workspace_id: workspaceRef.current })
            .then(setConfig)
            .catch((e) => setError(String(e)));
          setRevision((r) => r + 1);
        })
      : Promise.resolve(() => {});
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", focus);
      window.removeEventListener("keydown", key);
      unlisten.then((f) => f());
      unstatus.then((f) => f());
      unconfig.then((f) => f());
    };
  }, [refresh, modal]);
  async function sync() {
    try {
      await api("sync_now", {
        workspace_id: workspaceId || null,
        source_id: null,
      });
      refresh();
      setRevision((r) => r + 1);
    } catch (e) {
      setError(String(e));
    }
  }
  async function add() {
    try {
      const result = await invoke<string | null>("pick_workspace_config");
      if (result) {
        const w = await api("add_workspace_config", {
          config_file_path: result,
        });
        setWorkspaces(await api("list_workspaces", {}));
        setWorkspaceId(w.id);
        setModal(null);
      }
    } catch (e) {
      setError(String(e));
    }
  }
  function updateSettings(s: Settings) {
    setSettings(s);
    refresh();
  }
  const fields =
    config?.sources.find((s) => s.entity === config.views.main.entity)
      ?.fields || {};
  const sourceCount = status?.sources.length || 0;
  const state = freshness(status);
  const success =
    status?.sources
      .map((s) => s.last_success_at)
      .filter((s): s is string => !!s)
      .sort()[0] || null;
  const hasErrors = status?.sources.some((s) =>
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
        <div className="nav-caption">WORKSPACE</div>
        <button className="nav-item current">
          <LayoutList size={18} />
          {config?.views.main.title || "Records"}
          <span className="nav-count">{page.total.toLocaleString()}</span>
        </button>
        <button className="nav-item" onClick={() => setModal("sync")}>
          <Activity size={18} />
          Sync center{hasErrors && <span className="error-dot" />}
        </button>
        <div className="nav-caption nav-space">MANAGE</div>
        <button className="nav-item" onClick={() => setModal("settings")}>
          <SettingsIcon size={18} />
          Settings
        </button>
        <button className="nav-item" onClick={add}>
          <FolderOpen size={18} />
          Add workspace
        </button>
        <div className="sidebar-bottom">
          <div className="local-icon">
            <Database size={17} />
          </div>
          <div>
            <strong>Local workspace</strong>
            <small>Your data stays on this device</small>
          </div>
          <ShieldCheck size={16} />
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            Workspaces <ChevronRight size={14} />
            <strong>
              {workspaces.find((w) => w.id === workspaceId)?.name ||
                "Select a workspace"}
            </strong>
          </div>
          <div className="top-actions">
            <span className="workspace-select">
              <FileSpreadsheet size={15} />
              <select
                aria-label="Workspace"
                value={workspaceId}
                onChange={(e) => setWorkspaceId(e.target.value)}
              >
                {workspaces.map((w) => (
                  <option key={w.id} value={w.id}>
                    {w.name}
                  </option>
                ))}
              </select>
              <ChevronDown size={14} />
            </span>
            <button
              className="icon-button"
              title="Settings"
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
              {config?.views.main.title || "Excel Data Explorer"}
              <span className="heading-count">
                {page.total.toLocaleString()}
              </span>
            </h1>
            <p>
              Explore your records. Follow the connections. Find what matters.
            </p>
          </div>
          <button className="button" onClick={() => setModal("sync")}>
            <span className={`status-dot ${hasErrors ? "amber" : ""}`} />
            {state}
            <ChevronDown size={14} />
          </button>
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
          <section className="records-panel">
            <div className="table-toolbar">
              <div className="search-box">
                <Search size={17} />
                <input
                  aria-label="Search records"
                  placeholder={`Search ${config?.views.main.title.toLowerCase() || "records"}…`}
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
                <SlidersHorizontal size={15} />
                {config?.views.main.filters.map((f) => (
                  <input
                    key={f}
                    aria-label={`Filter ${label(f)}`}
                    className="filter-input"
                    placeholder={`${fields[f]?.label || label(f)}: all`}
                    value={String(view.filters?.[f] ?? "")}
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
                  className="button sync-button"
                  onClick={sync}
                  title="Synchronize in background"
                >
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
                {loading || searching ? "Updating view…" : "All records"}{" "}
                <span className="muted">
                  / {page.total.toLocaleString()} results
                </span>
              </span>
              <span className="muted">Double-click a record to explore</span>
            </div>
            {config ? (
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
                      v.sort === sort && v.direction === "asc" ? "desc" : "asc",
                    offset: 0,
                  }))
                }
                selectedId={view.selectedId}
                onSelect={(r) => setView((v) => ({ ...v, selectedId: r.id }))}
                onOpen={(r) => {
                  setDetail(r.id);
                  setView((v) => ({ ...v, selectedId: r.id, detailId: r.id }));
                }}
                widths={view.widths}
                onWidths={(widths) => setView((v) => ({ ...v, widths }))}
                loading={loading}
              />
            ) : (
              <div className="empty">
                <FileSpreadsheet size={32} />
                <h3>
                  {workspaces.length
                    ? "Loading workspace…"
                    : "Your data, in one place"}
                </h3>
                <p>Add a workspace configuration to get started.</p>
                <button className="button primary" onClick={add}>
                  <Plus size={16} />
                  Add workspace
                </button>
              </div>
            )}
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
                    setView((v) => ({ ...v, offset: (v.offset || 0) + 100 }))
                  }
                >
                  <ChevronRight size={17} />
                </button>
              </div>
            </div>
          </section>
          {detail && config && (
            <Details
              recordId={detail}
              config={config}
              revision={revision}
              onClose={() => {
                setDetail(null);
                setView((v) => ({ ...v, detailId: null }));
              }}
              onError={errorHandler}
            />
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
          workspaceId={workspaceId}
          status={status}
          settings={settings}
          onSettings={updateSettings}
          onSync={sync}
          onClose={() => setModal(null)}
          onError={errorHandler}
          onAdd={add}
        />
      )}
    </div>
  );
}
