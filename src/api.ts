import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  Config,
  DataRecord,
  Document,
  ImportError,
  ImportRun,
  Page,
  Query,
  Settings,
  SyncStatus,
  ViewState,
  Workspace,
} from "./types";
export interface Commands {
  list_workspaces: [Record<string, never>, Workspace[]];
  select_workspace: [{ workspace_id: string }, Workspace];
  add_workspace_config: [{ config_file_path: string }, Workspace];
  get_workspace_schema: [{ workspace_id: string }, Config];
  query_records: [
    { workspace_id: string; entity_type: string; options: Query },
    Page,
  ];
  get_record: [{ record_id: string }, DataRecord];
  get_related_records: [
    { record_id: string; relationship_id: string; options: Query },
    Page,
  ];
  get_record_documents: [
    { record_id: string; include_related: boolean },
    Document[],
  ];
  open_document: [{ document_id: string }, unknown];
  get_sync_status: [{ workspace_id: string | null }, SyncStatus];
  sync_now: [
    { workspace_id: string | null; source_id: string | null },
    { status: string },
  ];
  set_sync_paused: [{ paused: boolean }, Settings];
  get_import_history: [
    { workspace_id: string | null; source_id: string | null; offset: number },
    ImportRun[],
  ];
  get_import_errors: [{ import_id: number }, ImportError[]];
  get_settings: [Record<string, never>, Settings];
  update_settings: [{ settings: Settings }, Settings];
  get_ui_state: [{ key: string }, ViewState];
  save_ui_state: [{ key: string; value: ViewState }, null];
  open_logs: [Record<string, never>, null];
}
export async function api<K extends keyof Commands>(
  command: K,
  args: Commands[K][0],
): Promise<Commands[K][1]> {
  return invoke("dispatch", { request: { command, ...args } });
}
export { isTauri };
export function label(field: string) {
  return field === "id"
    ? "ID"
    : field.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
export function formatTime(time: string | null) {
  if (!time) return "Never imported";
  const date = new Date(
    time.includes("T") ? time : time.replace(" ", "T") + "Z",
  );
  return Number.isNaN(date.getTime())
    ? time
    : date.toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
}
export function freshness(status: SyncStatus | null) {
  if (!status) return "Starting";
  if (status.config_errors?.length)
    return "Configuration error — using cached data";
  if (status.paused) return "Paused";
  if (status.app_state === "Synchronizing") return "Synchronizing";
  if (status.sources.some((s) => s.status === "unavailable"))
    return "Cached — source unavailable";
  if (status.sources.some((s) => s.status === "error"))
    return "Error — using cached data";
  if (status.sources.some((s) => !s.last_success_at)) return "Never imported";
  return "Up to date";
}
