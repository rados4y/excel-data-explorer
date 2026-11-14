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
  Source,
  PageDefinition,
  WorkbookPreview,
} from "./types";
export interface Commands {
  save_nested_views: [
    {
      workspace_id: string;
      page_id: string;
      views: import("./types").NestedViewInput[];
    },
    null,
  ];
  get_nested_records: [
    {
      workspace_id: string;
      parent_page_id: string;
      view_id: string;
      record_id: string;
      options: Query;
    },
    Page,
  ];
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
  inspect_workbook: [
    { file_path: string; sheet?: string | null; header_row?: number | null },
    WorkbookPreview,
  ];
  create_space: [{ name: string }, Workspace];
  rename_space: [{ workspace_id: string; name: string }, Workspace];
  add_excel_workbook: [
    {
      file_path: string;
      space_id: string | null;
      name: string | null;
      hidden_sheets: string[];
    },
    Workspace,
  ];
  list_pages: [{ workspace_id: string }, PageDefinition[]];
  get_page_schema: [{ workspace_id: string; page_id: string }, Config];
  save_page: [{ workspace_id: string; page: PageDefinition }, PageDefinition];
  create_view: [
    {
      workspace_id: string;
      source_id: string;
      title: string;
      columns: string[];
      filters: Record<string, import("./types").Scalar>;
      sort: { field: string; direction: "asc" | "desc" } | null;
    },
    PageDefinition,
  ];
  update_worksheet: [{ workspace_id: string; source: Source }, Config];
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
  if (status.sources.length === 0) return "No sources yet";
  if (status.app_state === "Synchronizing") return "Synchronizing";
  if (status.sources.some((s) => s.status === "unavailable"))
    return "Cached — source unavailable";
  if (status.sources.some((s) => s.status === "error"))
    return "Error — using cached data";
  if (status.sources.some((s) => !s.last_success_at)) return "Never imported";
  return "Up to date";
}
