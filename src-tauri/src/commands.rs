use crate::{
    engine::{Engine, QueryOptions},
    lifecycle::Settings,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    ListWorkspaces,
    SelectWorkspace {
        workspace_id: String,
    },
    AddWorkspaceConfig {
        config_file_path: String,
    },
    GetWorkspaceSchema {
        workspace_id: String,
    },
    QueryRecords {
        workspace_id: String,
        entity_type: String,
        options: QueryOptions,
    },
    GetRecord {
        record_id: String,
    },
    GetRelatedRecords {
        record_id: String,
        relationship_id: String,
        options: QueryOptions,
    },
    GetRecordDocuments {
        record_id: String,
        include_related: bool,
    },
    OpenDocument {
        document_id: String,
    },
    GetSyncStatus {
        workspace_id: Option<String>,
    },
    SyncNow {
        workspace_id: Option<String>,
        source_id: Option<String>,
    },
    SetSyncPaused {
        paused: bool,
    },
    GetImportHistory {
        workspace_id: Option<String>,
        source_id: Option<String>,
        offset: u64,
    },
    GetImportErrors {
        import_id: i64,
    },
    GetSettings,
    UpdateSettings {
        settings: Settings,
    },
    GetUiState {
        key: String,
    },
    SaveUiState {
        key: String,
        value: Value,
    },
    OpenLogs,
}
impl Engine {
    pub fn dispatch(&self, request: Request) -> Result<Value> {
        Ok(match request {
            Request::ListWorkspaces => serde_json::to_value(self.list_workspaces()?)?,
            Request::SelectWorkspace { workspace_id } => {
                let w = self
                    .list_workspaces()?
                    .into_iter()
                    .find(|w| w.id == workspace_id)
                    .context("Workspace not found")?;
                let mut s = self.settings()?;
                s.selected_workspace = Some(workspace_id);
                self.update_settings(s)?;
                serde_json::to_value(w)?
            }
            Request::AddWorkspaceConfig { config_file_path } => {
                serde_json::to_value(self.add_config(std::path::Path::new(&config_file_path))?)?
            }
            Request::GetWorkspaceSchema { workspace_id } => {
                serde_json::to_value(self.config(&workspace_id)?)?
            }
            Request::QueryRecords {
                workspace_id,
                entity_type,
                options,
            } => serde_json::to_value(self.query(&workspace_id, &entity_type, &options, None)?)?,
            Request::GetRecord { record_id } => serde_json::to_value(self.record(&record_id)?)?,
            Request::GetRelatedRecords {
                record_id,
                relationship_id,
                options,
            } => serde_json::to_value(self.related(&record_id, &relationship_id, &options)?)?,
            Request::GetRecordDocuments {
                record_id,
                include_related,
            } => serde_json::to_value(self.documents(&record_id, include_related)?)?,
            Request::OpenDocument { document_id } => {
                self.validated_document(&document_id)?;
                json!({"validated":true})
            }
            Request::GetSyncStatus { workspace_id } => self.sync_status(workspace_id.as_deref())?,
            Request::SyncNow {
                workspace_id,
                source_id,
            } => {
                for w in self
                    .list_workspaces()?
                    .iter()
                    .filter(|w| workspace_id.as_ref().is_none_or(|id| &w.id == id))
                {
                    let c = self.config(&w.id)?;
                    for (path, sources) in c.workbook_groups() {
                        if source_id
                            .as_ref()
                            .is_none_or(|id| sources.iter().any(|s| &s.id == id))
                        {
                            let _ = self.sync_workbook(&w.id, &path, true, "manual");
                        }
                    }
                    self.refresh_documents(Some(&w.id))?;
                }
                json!({"status":"completed"})
            }
            Request::SetSyncPaused { paused } => serde_json::to_value(self.set_paused(paused)?)?,
            Request::GetImportHistory {
                workspace_id,
                source_id,
                offset,
            } => serde_json::to_value(self.history(
                workspace_id.as_deref(),
                source_id.as_deref(),
                offset,
            )?)?,
            Request::GetImportErrors { import_id } => {
                serde_json::to_value(self.errors(import_id)?)?
            }
            Request::GetSettings => serde_json::to_value(self.settings()?)?,
            Request::UpdateSettings { settings } => {
                serde_json::to_value(self.update_settings(settings)?)?
            }
            Request::GetUiState { key } => self.ui_state(&key)?,
            Request::SaveUiState { key, value } => {
                self.save_ui_state(&key, &value)?;
                json!(null)
            }
            Request::OpenLogs => json!(null),
        })
    }
}
