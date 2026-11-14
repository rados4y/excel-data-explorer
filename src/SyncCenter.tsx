import { useEffect, useRef, useState } from "react";
import {
  Activity,
  AlertCircle,
  Check,
  FileSpreadsheet,
  FolderOpen,
  Pause,
  Play,
  RefreshCw,
  X,
} from "lucide-react";
import { api, formatTime } from "./api";
import type { ImportError, ImportRun, Settings, SyncStatus } from "./types";
export function SyncCenter(p: {
  mode: "sync" | "settings";
  managed?: boolean;
  status: SyncStatus | null;
  workspaceId: string;
  settings: Settings;
  onSettings: (s: Settings) => void;
  onSync: () => void;
  onClose: () => void;
  onError: (s: string) => void;
  onAdd: () => void;
}) {
  const dialog = useRef<HTMLElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    return () => previous?.focus();
  }, []);
  const [history, setHistory] = useState<ImportRun[]>([]),
    [errors, setErrors] = useState<ImportError[]>([]);
  useEffect(() => {
    api("get_import_history", {
      workspace_id: p.workspaceId,
      source_id: null,
      offset: 0,
    })
      .then(setHistory)
      .catch((e) => p.onError(String(e)));
  }, [p.workspaceId, p.status]);
  function setting(key: keyof Settings, value: boolean) {
    api("update_settings", { settings: { ...p.settings, [key]: value } })
      .then(p.onSettings)
      .catch((e) => p.onError(String(e)));
  }
  return (
    <div
      className="modal-shade"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) p.onClose();
      }}
    >
      <section
        className="modal"
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-label={p.mode === "settings" ? "Settings" : "Sync center"}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            p.onClose();
          }
          if (e.key === "Tab") {
            const controls = dialog.current?.querySelectorAll<HTMLElement>(
              'button:not(:disabled),input:not(:disabled),select:not(:disabled),[tabindex="0"]',
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
            <span className="eyebrow">WORKSPACE</span>
            <h2>{p.mode === "settings" ? "Settings" : "Sync center"}</h2>
          </div>
          <button
            autoFocus
            className="icon-button"
            aria-label="Close dialog"
            onClick={p.onClose}
          >
            <X size={20} />
          </button>
        </div>
        <div className="modal-body">
          {p.mode === "settings" ? (
            <>
              <h3>Desktop preferences</h3>
              <label className="setting">
                <span>
                  <strong>Start at Windows sign-in</strong>
                  <small>Start hidden and synchronize in the background.</small>
                </span>
                <input
                  type="checkbox"
                  checked={p.settings.autostart}
                  onChange={(e) => setting("autostart", e.target.checked)}
                />
              </label>
              <label className="setting">
                <span>
                  <strong>Close to system tray</strong>
                  <small>
                    Keep the cache ready when you close the window. Use tray
                    Exit to quit.
                  </small>
                </span>
                <input
                  type="checkbox"
                  checked={p.settings.close_to_tray}
                  onChange={(e) => setting("close_to_tray", e.target.checked)}
                />
              </label>
              <h3>Advanced configuration</h3>
              <p className="muted">
                Excel spaces are configured through Pages and views. You can
                also import an existing YAML workspace for advanced
                relationships and document mappings.
              </p>
              <button className="button" onClick={p.onAdd}>
                <FolderOpen size={16} /> Add workspace configuration
              </button>
              {!p.managed && p.workspaceId && (
                <button
                  className="button"
                  onClick={async () => {
                    try {
                      const ws = await api("list_workspaces", {});
                      const w = ws.find((w) => w.id === p.workspaceId);
                      if (w && !w.config_path.startsWith("app://")) {
                        await api("add_workspace_config", {
                          config_file_path: w.config_path,
                        });
                        p.onSync();
                      }
                    } catch (e) {
                      p.onError(String(e));
                    }
                  }}
                >
                  <RefreshCw size={16} /> Reload imported YAML
                </button>
              )}
              <h3>Diagnostics</h3>
              <button
                className="button"
                onClick={() =>
                  api("open_logs", {}).catch((e) => p.onError(String(e)))
                }
              >
                <FolderOpen size={16} /> Open logs folder
              </button>
              <p className="small muted">
                Source files are read-only. Cached business data is stored
                locally without application-level encryption.
              </p>
            </>
          ) : (
            <>
              <div className="sync-actions">
                <div>
                  <Activity size={18} />
                  <strong>
                    {p.status?.app_state === "Synchronizing"
                      ? "Synchronizing in background"
                      : "Cached data is available"}
                  </strong>
                </div>
                <button
                  className="button"
                  onClick={() =>
                    api("set_sync_paused", { paused: !p.settings.sync_paused })
                      .then(p.onSettings)
                      .catch((e) => p.onError(String(e)))
                  }
                >
                  {p.settings.sync_paused ? (
                    <Play size={15} />
                  ) : (
                    <Pause size={15} />
                  )}{" "}
                  {p.settings.sync_paused ? "Resume" : "Pause"}
                </button>
                <button className="button primary" onClick={p.onSync}>
                  <RefreshCw size={15} /> Sync now
                </button>
              </div>
              <p className="muted small">
                Each workbook has its own successful import time. Sync now also
                runs while periodic sync is paused.
              </p>
              <div className="source-list">
                {p.status?.sources.map((s) => (
                  <article className="source-card" key={s.source_id}>
                    <div className="source-heading">
                      <FileSpreadsheet size={20} />
                      <strong>{s.source_id}</strong>
                      <span
                        className={`badge ${s.status === "ready" ? "active" : "draft"}`}
                      >
                        {s.status === "ready"
                          ? "Up to date"
                          : s.status === "unavailable"
                            ? "Cached — source unavailable"
                            : s.status === "error"
                              ? "Error — using cached data"
                              : s.status}
                      </span>
                    </div>
                    <p className="source-path">
                      {s.path} · {s.sheet}
                    </p>
                    <div className="source-stats">
                      <span>
                        {s.record_count.toLocaleString()} cached records
                      </span>
                      <span>Imported {formatTime(s.last_success_at)}</span>
                      <span>Checked {formatTime(s.last_checked_at)}</span>
                      {s.warnings > 0 && (
                        <span className="warning">{s.warnings} warnings</span>
                      )}
                    </div>
                    {s.last_error && (
                      <p className="inline-error">
                        <AlertCircle size={15} />
                        {s.last_error}
                      </p>
                    )}
                  </article>
                ))}
              </div>
              <h3>Recent imports</h3>
              {history.length === 0 ? (
                <p className="muted">No import attempts yet.</p>
              ) : (
                history.map((h) => (
                  <button
                    key={h.id}
                    className="history-row"
                    onClick={() =>
                      api("get_import_errors", { import_id: h.id })
                        .then(setErrors)
                        .catch((e) => p.onError(String(e)))
                    }
                  >
                    {h.status === "succeeded" ? (
                      <Check size={16} />
                    ) : (
                      <Activity size={16} />
                    )}
                    <span>
                      <strong>{h.workbook.split(/[\\/]/).at(-1)}</strong>
                      <small>
                        {formatTime(h.started_at)} · {h.rows_read} rows · +
                        {h.rows_created} / ~{h.rows_updated} / −{h.rows_deleted}
                        {h.elapsed_ms != null ? ` · ${h.elapsed_ms} ms` : ""}
                      </small>
                      {h.error && <small className="warning">{h.error}</small>}
                    </span>
                    <span className="badge">{h.status}</span>
                  </button>
                ))
              )}
              {errors.length > 0 && (
                <div className="diagnostics">
                  <h3>Import diagnostics</h3>
                  {errors.map((e, i) => (
                    <p key={i}>
                      <strong>
                        {e.severity} · {e.source_id} · {e.sheet}:{e.row}
                        {e.field ? ` · ${e.field}` : ""}
                      </strong>
                      <br />
                      {e.message}
                    </p>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      </section>
    </div>
  );
}
