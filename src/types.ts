export type Scalar = string | number | boolean | null;
export interface Workspace {
  id: string;
  name: string;
  config_path: string;
  record_count: number;
}
export interface Field {
  column: string;
  column_index?: number | null;
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
  key_mode?: "field" | "row_number";
  header_row: number;
  fields: Record<string, Field>;
}
export interface Relationship {
  parent_source_id?: string | null;
  child_source_id?: string | null;
  id: string;
  parent_entity: string;
  child_entity: string;
  parent_field: string;
  child_field: string;
}
export interface Config {
  workspace: { id: string; name: string };
  sources: Source[];
  managed?: boolean;
  pages?: PageDefinition[];
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
  page_id?: string;
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
  pageId?: string;
  search?: string;
  filters?: Record<string, Scalar>;
  sort?: string;
  direction?: "asc" | "desc";
  offset?: number;
  selectedId?: string | null;
  detailId?: string | null;
  widths?: Record<string, number>;
}

export interface RelatedView {
  id: string;
  title: string;
  relationship: string;
  page_id: string;
}
export interface NestedViewInput {
  id?: string | null;
  title: string;
  page_id: string;
  parent_field: string;
  child_field: string;
}
export interface PageDefinition {
  related_views?: RelatedView[];
  id: string;
  title: string;
  entity: string;
  source_id: string | null;
  kind: "worksheet" | "view";
  columns: string[];
  default_sort: { field: string; direction: "asc" | "desc" };
  search_fields: string[];
  filters: string[];
  fixed_filters: Record<string, Scalar>;
  hidden: boolean;
}
export interface ColumnPreview {
  id: string;
  column: string;
  column_index: number;
  label: string;
}
export interface WorksheetPreview {
  name: string;
  header_row: number;
  columns: ColumnPreview[];
  rows: Scalar[][];
  row_count: number;
  empty: boolean;
}
export interface WorkbookPreview {
  path: string;
  name: string;
  worksheets: WorksheetPreview[];
}
