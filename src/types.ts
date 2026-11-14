export type Scalar = string | number | boolean | null;
export interface Workspace {
  id: string;
  name: string;
  config_path: string;
  record_count: number;
}
export interface Field {
  column: string;
  type: "string" | "integer" | "decimal" | "boolean" | "date" | "datetime";
  label?: string;
  searchable: boolean;
  filterable: boolean;
  required: boolean;
  trim: boolean;
}
export interface Source {
  id: string;
  path: string;
  sheet: string;
  entity: string;
  namespace: string;
  key: string;
  fields: Record<string, Field>;
}
export interface Relationship {
  id: string;
  parent_entity: string;
  child_entity: string;
  parent_field: string;
  child_field: string;
}
export interface Config {
  workspace: { id: string; name: string };
  sources: Source[];
  relationships: Relationship[];
  views: {
    main: {
      entity: string;
      title: string;
      columns: string[];
      default_sort: { field: string; direction: "asc" | "desc" };
      search_fields: string[];
      filters: string[];
    };
    detail: {
      title_field: string;
      tabs: {
        title: string;
        type: "fields" | "relationship" | "related_documents";
        relationship?: string;
      }[];
    };
  };
}
export interface DataRecord {
  id: string;
  workspace_id: string;
  entity_type: string;
  namespace: string;
  business_key: string;
  fields: Record<string, Scalar>;
  source_id: string;
  source_sheet: string;
  source_row: number;
  updated_at: string;
}
export interface Page {
  records: DataRecord[];
  total: number;
  offset: number;
  limit: number;
}
export interface Query {
  search?: string;
  filters?: Record<string, Scalar>;
  sort?: string;
  direction?: "asc" | "desc";
  offset?: number;
  limit?: number;
}
export interface Document {
  id: string;
  name: string;
  original_ref: string;
  resolved_path: string;
  document_type: string;
  availability_status: string;
  status_detail: string | null;
  size: number | null;
  mapping_id: string;
}
export interface Settings {
  autostart: boolean;
  close_to_tray: boolean;
  sync_paused: boolean;
  selected_workspace: string | null;
}
export interface SourceStatus {
  workspace_id: string;
  source_id: string;
  path: string;
  sheet: string;
  status: string;
  last_checked_at: string | null;
  last_success_at: string | null;
  last_error: string | null;
  generation: number | null;
  record_count: number;
  warnings: number;
}
export interface SyncStatus {
  config_errors?: { workspace_id: string; error: string }[];
  lifecycle_error?: { error?: string };
  app_state: string;
  paused: boolean;
  sources: SourceStatus[];
}
export interface ImportRun {
  id: number;
  workbook: string;
  status: string;
  started_at: string;
  finished_at: string | null;
  elapsed_ms: number | null;
  rows_read: number;
  rows_created: number;
  rows_updated: number;
  rows_deleted: number;
  rows_unchanged: number;
  error: string | null;
}
export interface ImportError {
  severity: string;
  source_id: string;
  sheet: string;
  row: number;
  field: string | null;
  code: string;
  message: string;
}
export interface ViewState {
  search?: string;
  filters?: Record<string, Scalar>;
  sort?: string;
  direction?: "asc" | "desc";
  offset?: number;
  selectedId?: string | null;
  detailId?: string | null;
  widths?: Record<string, number>;
}
