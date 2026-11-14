use crate::{
    config::{Config, Direction, FieldType},
    db,
    documents::DocumentSummary,
    lifecycle::Settings,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone)]
pub struct Engine {
    pub db_path: PathBuf,
    pub write_gate: Arc<Mutex<()>>,
    pub cancelled: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub record_count: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub workspace_id: String,
    pub entity_type: String,
    pub namespace: String,
    pub business_key: String,
    pub source_id: String,
    pub source_sheet: String,
    pub source_row: u32,
    pub fields: Value,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct QueryOptions {
    pub search: String,
    pub filters: BTreeMap<String, Value>,
    pub sort: Option<String>,
    pub direction: Option<Direction>,
    pub offset: u64,
    pub limit: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub records: Vec<Record>,
    pub total: i64,
    pub offset: u64,
    pub limit: u32,
}
const RECORD_COLUMNS:&str="r.id,r.workspace_id,r.entity_type,r.namespace,r.business_key,r.source_definition_id,r.source_sheet,r.source_row,r.fields_json,r.updated_at";
fn read_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<Record> {
    let s: String = r.get(8)?;
    Ok(Record {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        entity_type: r.get(2)?,
        namespace: r.get(3)?,
        business_key: r.get(4)?,
        source_id: r
            .get::<_, String>(5)?
            .split_once('/')
            .map(|(_, s)| s.to_string())
            .unwrap_or_default(),
        source_sheet: r.get(6)?,
        source_row: r.get(7)?,
        fields: serde_json::from_str(&s).unwrap_or(Value::Null),
        updated_at: r.get(9)?,
    })
}
impl Engine {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        db::initialize(path.as_ref())?;
        let e = Self {
            db_path: path.as_ref().to_path_buf(),
            write_gate: Arc::new(Mutex::new(())),
            cancelled: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        };
        e.paused.store(e.settings()?.sync_paused, Ordering::Relaxed);
        Ok(e)
    }
    pub fn connection(&self) -> Result<rusqlite::Connection> {
        db::connect(&self.db_path)
    }
    pub fn config(&self, workspace: &str) -> Result<Config> {
        let c = self.connection()?;
        let text:String=c.query_row("SELECT v.config_json FROM workspaces w JOIN config_versions v ON v.id=w.active_config_version WHERE w.id=?",[workspace],|r|r.get(0)).context("Workspace not found")?;
        Ok(serde_json::from_str(&text)?)
    }
    pub fn add_config(&self, path: &Path) -> Result<WorkspaceSummary> {
        let config = Config::load(path)?;
        let path = std::fs::canonicalize(path)?;
        let _guard = self
            .write_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer unavailable"))?;
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        let ws = &config.workspace.id;
        tx.execute("INSERT INTO workspaces(id,name,config_path) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,config_path=excluded.config_path,updated_at=CURRENT_TIMESTAMP",params![ws,config.workspace.name,path.to_string_lossy()])?;
        let previous:Option<String>=tx.query_row("SELECT v.config_hash FROM workspaces w JOIN config_versions v ON v.id=w.active_config_version WHERE w.id=?",[ws],|r|r.get(0)).optional()?;
        if previous.as_deref() != Some(&config.hash()) {
            // Removing mappings is an explicit valid config change; retire only their own cached rows.
            let old:Option<String>=tx.query_row("SELECT v.config_json FROM workspaces w JOIN config_versions v ON v.id=w.active_config_version WHERE w.id=?",[ws],|r|r.get(0)).optional()?;
            if let Some(old) = old {
                let old: Config = serde_json::from_str(&old)?;
                for s in &old.sources {
                    if !config.sources.iter().any(|n| n.id == s.id) {
                        tx.execute(
                            "UPDATE records SET active=0 WHERE source_definition_id=?",
                            [db::qualified(ws, &s.id)],
                        )?;
                    }
                }
            }
            tx.execute("INSERT INTO config_versions(workspace_id,config_hash,config_json,is_valid) VALUES(?,?,?,1)",params![ws,config.hash(),serde_json::to_string(&config)?])?;
            let version = tx.last_insert_rowid();
            tx.execute(
                "UPDATE workspaces SET active_config_version=? WHERE id=?",
                params![version, ws],
            )?;
            for s in &config.sources {
                let id = db::qualified(ws, &s.id);
                tx.execute("INSERT INTO source_definitions(id,workspace_id,source_id,physical_path,sheet_name,entity_type,namespace,mapping_json,config_version) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET physical_path=excluded.physical_path,sheet_name=excluded.sheet_name,entity_type=excluded.entity_type,namespace=excluded.namespace,mapping_json=excluded.mapping_json,config_version=excluded.config_version",params![id,ws,s.id,s.path,s.sheet,s.entity,s.namespace,serde_json::to_string(s)?,version])?;
                tx.execute("INSERT INTO source_state(source_definition_id) VALUES(?) ON CONFLICT DO NOTHING",[id])?;
            }
            tx.execute(
                "DELETE FROM record_relationships WHERE workspace_id=?",
                [ws],
            )?;
            tx.execute(
                "DELETE FROM relationship_definitions WHERE workspace_id=?",
                [ws],
            )?;
            for rel in &config.relationships {
                tx.execute("INSERT INTO relationship_definitions(id,workspace_id,definition_json) VALUES(?,?,?)",params![db::qualified(ws,&rel.id),ws,serde_json::to_string(rel)?])?;
            }
            crate::indexing::rebuild_relationships(&tx, &config, None)?;
        }
        tx.commit()?;
        drop(_guard);
        self.list_workspaces()?
            .into_iter()
            .find(|w| &w.id == ws)
            .context("Workspace activation failed")
    }
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT w.id,w.name,w.config_path,(SELECT count(*) FROM records r WHERE r.workspace_id=w.id AND r.active=1) FROM workspaces w ORDER BY w.name,w.id")?;
        let rows = q
            .query_map([], |r| {
                Ok(WorkspaceSummary {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    config_path: r.get(2)?,
                    record_count: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
    pub fn settings(&self) -> Result<Settings> {
        let c = self.connection()?;
        let v: Option<String> = c
            .query_row(
                "SELECT value FROM app_settings WHERE key='preferences'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v.map(|v| serde_json::from_str(&v))
            .transpose()?
            .unwrap_or_default())
    }
    pub fn update_settings(&self, settings: Settings) -> Result<Settings> {
        let c = self.connection()?;
        c.execute("INSERT INTO app_settings(key,value) VALUES('preferences',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[serde_json::to_string(&settings)?])?;
        self.paused.store(settings.sync_paused, Ordering::Relaxed);
        Ok(settings)
    }
    pub fn set_paused(&self, paused: bool) -> Result<Settings> {
        let mut s = self.settings()?;
        s.sync_paused = paused;
        self.update_settings(s)
    }
    pub fn ui_state(&self, key: &str) -> Result<Value> {
        let c = self.connection()?;
        let v: Option<String> = c
            .query_row("SELECT value FROM ui_state WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(v.map(|v| serde_json::from_str(&v))
            .transpose()?
            .unwrap_or(json!({})))
    }
    pub fn save_ui_state(&self, key: &str, value: &Value) -> Result<()> {
        ensure!(
            key.len() < 200 && value.to_string().len() < 65536,
            "UI state too large"
        );
        self.connection()?.execute("INSERT INTO ui_state(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value.to_string()])?;
        Ok(())
    }
    pub fn record(&self, id: &str) -> Result<Record> {
        let c = self.connection()?;
        c.query_row(
            &format!("SELECT {RECORD_COLUMNS} FROM records r WHERE r.id=? AND r.active=1"),
            [id],
            read_record,
        )
        .context("Record not found in active cache")
    }
    pub fn query(
        &self,
        workspace: &str,
        entity: &str,
        options: &QueryOptions,
        related: Option<(&str, &str)>,
    ) -> Result<Page> {
        let config = self.config(workspace)?;
        let fields = config.entity_fields(entity)?;
        let sort = options
            .sort
            .as_deref()
            .unwrap_or(if entity == config.views.main.entity {
                &config.views.main.default_sort.field
            } else {
                fields.keys().next().context("No fields")?
            });
        let kind = fields.get(sort).context("Unknown sort field")?.kind;
        let direction =
            options
                .direction
                .as_ref()
                .unwrap_or(if entity == config.views.main.entity {
                    &config.views.main.default_sort.direction
                } else {
                    &Direction::Asc
                });
        let limit = options.limit.unwrap_or(100);
        ensure!(
            (1..=200).contains(&limit) && options.offset <= 10_000_000,
            "Pagination out of bounds"
        );
        ensure!(options.search.len() <= 512, "Search is too long");
        let mut clauses = vec![
            "r.workspace_id=?".to_string(),
            "r.entity_type=?".into(),
            "r.active=1".into(),
        ];
        let mut args: Vec<rusqlite::types::Value> =
            vec![workspace.to_string().into(), entity.to_string().into()];
        if let Some((parent, relation)) = related {
            let rel = config
                .relationships
                .iter()
                .find(|r| r.id == relation)
                .context("Unknown relationship")?;
            let p = self.record(parent)?;
            ensure!(
                p.workspace_id == workspace
                    && p.entity_type == rel.parent_entity
                    && entity == rel.child_entity,
                "Relationship scope mismatch"
            );
            clauses.push("r.id IN(SELECT rr.child_record_id FROM record_relationships rr WHERE rr.parent_record_id=? AND rr.relationship_id=?)".into());
            args.push(parent.to_string().into());
            args.push(db::qualified(workspace, relation).into());
        }
        for (name, value) in &options.filters {
            let f = fields.get(name).context("Unknown filter field")?;
            ensure!(
                f.filterable
                    || (entity == config.views.main.entity
                        && config.views.main.filters.contains(name)),
                "Field is not configured for filtering"
            );
            clauses.push("EXISTS(SELECT 1 FROM record_field_index fi WHERE fi.record_id=r.id AND fi.field_name=? AND fi.text_value=?)".into());
            args.push(name.clone().into());
            args.push(value_text(value).into());
        }
        if !options.search.trim().is_empty() {
            let search_fields: Vec<String> = if entity == config.views.main.entity {
                config.views.main.search_fields.clone()
            } else {
                fields
                    .iter()
                    .filter(|(_, f)| f.searchable || f.required)
                    .map(|(k, _)| k.clone())
                    .collect()
            };
            ensure!(!search_fields.is_empty(), "Entity has no searchable fields");
            let terms = options
                .search
                .split_whitespace()
                .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" AND ");
            clauses.push(format!("r.id IN(SELECT record_id FROM record_fts WHERE record_fts MATCH ? AND field_name IN({}))",vec!["?";search_fields.len()].join(",")));
            args.push(terms.into());
            args.extend(search_fields.into_iter().map(Into::into));
        }
        let where_sql = clauses.join(" AND ");
        let c = self.connection()?;
        // A deferred read transaction makes count+page one consistent WAL snapshot.
        c.execute_batch("BEGIN DEFERRED")?;
        let total = c.query_row(
            &format!("SELECT count(*) FROM records r WHERE {where_sql}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        let value_column = match kind {
            FieldType::Integer | FieldType::Decimal => "numeric_value",
            FieldType::Boolean => "bool_value",
            FieldType::Date | FieldType::Datetime => "date_value",
            _ => "text_value",
        };
        let direction = if *direction == Direction::Asc {
            "ASC"
        } else {
            "DESC"
        };
        let key_sort = config
            .sources
            .iter()
            .filter(|s| s.entity == entity)
            .all(|s| s.key == sort);
        let sql = if key_sort {
            format!("SELECT {RECORD_COLUMNS} FROM records r WHERE {where_sql} ORDER BY r.business_key {direction},r.id ASC LIMIT ? OFFSET ?")
        } else {
            format!("SELECT {RECORD_COLUMNS} FROM records r LEFT JOIN record_field_index s ON s.record_id=r.id AND s.field_name=? WHERE {where_sql} ORDER BY s.{value_column} {direction},r.id ASC LIMIT ? OFFSET ?")
        };
        let mut page_args = Vec::new();
        if !key_sort {
            page_args.push(sort.to_string().into());
        }
        page_args.extend(args);
        page_args.push((limit as i64).into());
        page_args.push((options.offset as i64).into());
        let records = c
            .prepare(&sql)?
            .query_map(rusqlite::params_from_iter(page_args.iter()), read_record)?
            .collect::<rusqlite::Result<_>>()?;
        c.execute_batch("COMMIT")?;
        Ok(Page {
            records,
            total,
            offset: options.offset,
            limit,
        })
    }
    pub fn related(&self, id: &str, relation: &str, options: &QueryOptions) -> Result<Page> {
        let r = self.record(id)?;
        let config = self.config(&r.workspace_id)?;
        let rel = config
            .relationships
            .iter()
            .find(|r| r.id == relation)
            .context("Unknown relationship")?;
        self.query(
            &r.workspace_id,
            &rel.child_entity,
            options,
            Some((id, relation)),
        )
    }
    pub fn documents(&self, id: &str, include_related: bool) -> Result<Vec<DocumentSummary>> {
        let r = self.record(id)?;
        let c = self.connection()?;
        let sql="SELECT DISTINCT d.id,d.original_ref,d.resolved_path,d.document_type,d.availability_status,d.status_detail,d.size,d.mapping_id FROM documents d JOIN record_documents rd ON rd.document_id=d.id JOIN records r ON r.id=rd.record_id AND r.active=1 WHERE d.workspace_id=? AND (rd.record_id=? OR (? AND rd.record_id IN(SELECT child_record_id FROM record_relationships WHERE parent_record_id=?))) ORDER BY d.original_ref,d.id LIMIT 1000";
        let rows = c
            .prepare(sql)?
            .query_map(params![r.workspace_id, id, include_related, id], |r| {
                let path: String = r.get(2)?;
                Ok(DocumentSummary {
                    id: r.get(0)?,
                    original_ref: r.get(1)?,
                    name: Path::new(&path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    resolved_path: path,
                    document_type: r.get(3)?,
                    availability_status: r.get(4)?,
                    status_detail: r.get(5)?,
                    size: r.get(6)?,
                    mapping_id: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
    pub fn validated_document(&self, id: &str) -> Result<PathBuf> {
        let c = self.connection()?;
        let (ws,reference,mapping):(String,String,String)=c.query_row("SELECT d.workspace_id,d.original_ref,d.mapping_id FROM documents d WHERE d.id=? AND EXISTS(SELECT 1 FROM record_documents rd JOIN records r ON r.id=rd.record_id AND r.active=1 WHERE rd.document_id=d.id)",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).context("Document reference not found")?;
        let config = self.config(&ws)?;
        let mapping = config
            .documents
            .iter()
            .find(|d| d.id == mapping)
            .context("Document mapping no longer exists")?;
        crate::documents::resolve(
            Path::new(&config.roots[&mapping.root]),
            &reference,
            mapping,
            true,
        )
    }
    pub fn history(
        &self,
        workspace: Option<&str>,
        source: Option<&str>,
        offset: u64,
    ) -> Result<Vec<Value>> {
        let c = self.connection()?;
        let mut stmt=c.prepare("SELECT i.id,i.workspace_id,i.workbook,i.status,i.trigger,i.started_at,i.finished_at,i.elapsed_ms,i.rows_read,i.rows_created,i.rows_updated,i.rows_deleted,i.rows_unchanged,i.error,i.stages_json FROM import_runs i WHERE (? IS NULL OR i.workspace_id=?) AND (? IS NULL OR EXISTS(SELECT 1 FROM source_definitions s WHERE s.workspace_id=i.workspace_id AND s.source_id=? AND s.physical_path=i.workbook)) ORDER BY i.id DESC LIMIT 50 OFFSET ?")?;
        let rows=stmt.query_map(params![workspace,workspace,source,source,offset.min(1000000) as i64],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"workspace_id":r.get::<_,String>(1)?,"workbook":r.get::<_,String>(2)?,"status":r.get::<_,String>(3)?,"trigger":r.get::<_,String>(4)?,"started_at":r.get::<_,String>(5)?,"finished_at":r.get::<_,Option<String>>(6)?,"elapsed_ms":r.get::<_,Option<i64>>(7)?,"rows_read":r.get::<_,i64>(8)?,"rows_created":r.get::<_,i64>(9)?,"rows_updated":r.get::<_,i64>(10)?,"rows_deleted":r.get::<_,i64>(11)?,"rows_unchanged":r.get::<_,i64>(12)?,"error":r.get::<_,Option<String>>(13)?,"stages":serde_json::from_str::<Value>(&r.get::<_,String>(14)?).unwrap_or(json!({}))})))?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
    pub fn errors(&self, run: i64) -> Result<Vec<Value>> {
        let c = self.connection()?;
        let rows=c.prepare("SELECT severity,source_id,sheet,row,field,code,message FROM import_errors WHERE import_id=? ORDER BY id LIMIT 1000")?.query_map([run],|r|Ok(json!({"severity":r.get::<_,String>(0)?,"source_id":r.get::<_,String>(1)?,"sheet":r.get::<_,String>(2)?,"row":r.get::<_,u32>(3)?,"field":r.get::<_,Option<String>>(4)?,"code":r.get::<_,String>(5)?,"message":r.get::<_,String>(6)?})))?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}
pub fn value_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
impl Engine {
    pub fn sync_status(&self, workspace: Option<&str>) -> Result<Value> {
        let c = self.connection()?;
        let mut q=c.prepare("SELECT s.workspace_id,s.source_id,s.physical_path,s.sheet_name,ss.status,ss.last_checked_at,ss.last_success_at,ss.last_error,ss.active_generation,(SELECT count(*) FROM records r WHERE r.source_definition_id=s.id AND r.active=1),(SELECT count(*) FROM import_errors e WHERE e.import_id=(SELECT max(i.id) FROM import_runs i WHERE i.workspace_id=s.workspace_id AND i.status='succeeded') AND e.severity='warning' AND e.source_id=s.source_id) FROM source_definitions s JOIN workspaces w ON w.id=s.workspace_id AND s.config_version=w.active_config_version JOIN source_state ss ON ss.source_definition_id=s.id WHERE (? IS NULL OR s.workspace_id=?) ORDER BY s.workspace_id,s.source_id")?;
        let sources=q.query_map(params![workspace,workspace],|r|Ok(json!({"workspace_id":r.get::<_,String>(0)?,"source_id":r.get::<_,String>(1)?,"path":r.get::<_,String>(2)?,"sheet":r.get::<_,String>(3)?,"status":r.get::<_,String>(4)?,"last_checked_at":r.get::<_,Option<String>>(5)?,"last_success_at":r.get::<_,Option<String>>(6)?,"last_error":r.get::<_,Option<String>>(7)?,"generation":r.get::<_,Option<i64>>(8)?,"record_count":r.get::<_,i64>(9)?,"warnings":r.get::<_,i64>(10)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut config_errors = Vec::new();
        for w in self
            .list_workspaces()?
            .iter()
            .filter(|w| workspace.is_none_or(|id| id == w.id))
        {
            let error = self.ui_state(&format!("config-error:{}", w.id))?;
            if let Some(error) = error.get("error").and_then(Value::as_str) {
                config_errors.push(json!({"workspace_id":w.id,"error":error}));
            }
        }
        Ok(
            json!({"config_errors":config_errors,"lifecycle_error":self.ui_state("lifecycle-error")?,"app_state":if self.cancelled.load(Ordering::Relaxed){"Exiting"}else if self.paused.load(Ordering::Relaxed){"SyncPaused"}else if sources.iter().any(|s|["checking","snapshotting","parsing","staging","activating"].contains(&s["status"].as_str().unwrap_or(""))){"Synchronizing"}else{"ReadyFromCache"},"paused":self.paused.load(Ordering::Relaxed),"sources":sources}),
        )
    }
    pub fn refresh_documents(&self, workspace: Option<&str>) -> Result<u64> {
        let c = self.connection()?;
        let refs=c.prepare("SELECT d.id,d.workspace_id,d.original_ref,d.mapping_id FROM documents d WHERE (? IS NULL OR d.workspace_id=?) AND (d.last_verified_at IS NULL OR d.last_verified_at<datetime('now','-5 minutes')) AND EXISTS(SELECT 1 FROM record_documents rd JOIN records r ON r.id=rd.record_id AND r.active=1 WHERE rd.document_id=d.id) ORDER BY d.last_verified_at,d.id LIMIT 5000")?.query_map(params![workspace,workspace],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let configs = self
            .list_workspaces()?
            .into_iter()
            .map(|w| self.config(&w.id).map(|c| (w.id, c)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let mut outcomes = Vec::new();
        for (id, ws, reference, mapping_id) in &refs {
            if self.cancelled.load(Ordering::Relaxed) {
                break;
            }
            let result = (|| -> Result<std::fs::Metadata> {
                let config = configs.get(ws).context("Workspace no longer exists")?;
                let mapping = config
                    .documents
                    .iter()
                    .find(|d| &d.id == mapping_id)
                    .context("Document mapping no longer exists")?;
                let path = crate::documents::resolve(
                    Path::new(&config.roots[&mapping.root]),
                    reference,
                    mapping,
                    true,
                )?;
                std::fs::metadata(path).context("Document disappeared during verification")
            })();
            outcomes.push((id, result));
        }
        // Filesystem checks happen outside the write transaction; one bounded update batch.
        let tx = c.unchecked_transaction()?;
        for (id, result) in outcomes {
            match result {
                Ok(meta) => {
                    tx.execute("UPDATE documents SET availability_status='available',status_detail=NULL,size=?,last_modified=?,last_verified_at=CURRENT_TIMESTAMP WHERE id=?",params![meta.len() as i64,meta.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_millis().to_string()),id])?;
                }
                Err(e) => {
                    let text = e.to_string();
                    let status = if text.contains("missing")
                        || text.contains("unavailable")
                        || text.contains("disappeared")
                    {
                        "missing"
                    } else {
                        "blocked"
                    };
                    tx.execute("UPDATE documents SET availability_status=?,status_detail=?,last_verified_at=CURRENT_TIMESTAMP WHERE id=?",params![status,text,id])?;
                }
            }
        }
        tx.commit()?;
        Ok(refs.len() as u64)
    }
}
