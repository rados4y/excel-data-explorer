use crate::{
    config::{Config, Source},
    db,
    engine::Engine,
    excel::Diagnostic,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::atomic::Ordering, time::Instant};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportCounts {
    pub read: i64,
    pub created: i64,
    pub updated: i64,
    pub deleted: i64,
    pub unchanged: i64,
}
const STAGE_SQL:&str="
CREATE TABLE records(id TEXT PRIMARY KEY,workspace_id TEXT,entity_type TEXT,namespace TEXT,business_key TEXT,source_definition_id TEXT,source_sheet TEXT,source_row INTEGER,fields_json TEXT,raw_json TEXT,record_hash TEXT,source_id TEXT,UNIQUE(entity_type,namespace,business_key));
CREATE TABLE fields(record_id TEXT,field_name TEXT,text_value TEXT,numeric_value REAL,date_value TEXT,bool_value INTEGER);
CREATE TABLE fts(record_id TEXT,field_name TEXT,content TEXT);
CREATE TABLE documents(id TEXT PRIMARY KEY,workspace_id TEXT,original_ref TEXT,resolved_path TEXT,document_type TEXT,availability_status TEXT,status_detail TEXT,root_path TEXT,mapping_id TEXT);
CREATE TABLE links(record_id TEXT,document_id TEXT,document_mapping_id TEXT,PRIMARY KEY(record_id,document_id,document_mapping_id));
CREATE INDEX staged_source ON records(source_definition_id);
";
impl Engine {
    pub fn import_snapshot(
        &self,
        config: &Config,
        sources: &[&Source],
        snapshot: &Path,
        run: i64,
        fingerprint: &crate::sync::Fingerprint,
        started: Instant,
    ) -> Result<ImportCounts> {
        let stage_file = tempfile::Builder::new()
            .prefix(&db::stage_prefix(&self.db_path))
            .suffix(".sqlite")
            .tempfile_in(
                self.db_path
                    .parent()
                    .context("Database needs parent directory")?,
            )?;
        let mut stage = Connection::open(stage_file.path())?;
        stage.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;")?;
        stage.execute_batch(STAGE_SQL)?;
        let tx = stage.transaction()?;
        let parse_time = Instant::now();
        let parsed = crate::excel::parse_workbook(
            snapshot,
            sources,
            config,
            |r| {
                ensure!(!self.cancelled.load(Ordering::Relaxed), "Import cancelled");
                crate::indexing::stage_record(&tx, config, r)
            },
            |d| self.store_diagnostic(run, &d),
        );
        let read = parsed? as i64;
        tx.commit()?;
        self.stage_status(config, sources, "staging")?;
        let parse_ms = parse_time.elapsed().as_millis() as i64;
        ensure!(!self.cancelled.load(Ordering::Relaxed), "Import cancelled");
        let _gate = self
            .write_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer unavailable"))?;
        // A config edited while parsing cannot publish an obsolete generation.
        ensure!(
            self.config(&config.workspace.id)?.import_hash() == config.import_hash(),
            "Configuration changed during import; retry required"
        );
        self.stage_status(config, sources, "activating")?;
        let activate_time = Instant::now();
        let mut c = self.connection()?;
        c.execute(
            "ATTACH DATABASE ? AS stage",
            [stage_file.path().to_string_lossy()],
        )?;
        let tx = c.transaction()?;
        let conflict:Option<(String,u32)>=tx.query_row("SELECT s.source_id,s.source_row FROM stage.records s JOIN main.records r ON r.id=s.id WHERE r.source_definition_id<>s.source_definition_id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        ensure!(conflict.is_none(),"Shared namespace business-key collision in source/row {:?}; previous source ownership preserved",conflict);
        let created:i64=tx.query_row("SELECT count(*) FROM stage.records s LEFT JOIN main.records r ON r.id=s.id WHERE r.id IS NULL OR r.active=0",[],|r|r.get(0))?;
        let updated:i64=tx.query_row("SELECT count(*) FROM stage.records s JOIN main.records r ON r.id=s.id WHERE r.active=1 AND s.record_hash<>r.record_hash",[],|r|r.get(0))?;
        let unchanged:i64=tx.query_row("SELECT count(*) FROM stage.records s JOIN main.records r ON r.id=s.id WHERE r.active=1 AND s.record_hash=r.record_hash",[],|r|r.get(0))?;
        let mut deleted = 0;
        for s in sources {
            let source = db::qualified(&config.workspace.id, &s.id);
            deleted+=tx.execute("UPDATE records SET active=0,updated_at=CURRENT_TIMESTAMP WHERE source_definition_id=? AND active=1 AND id NOT IN(SELECT id FROM stage.records)",[&source])? as i64;
            tx.execute("DELETE FROM record_field_index WHERE record_id IN(SELECT id FROM records WHERE source_definition_id=?)",[&source])?;
            tx.execute("DELETE FROM record_fts WHERE record_id IN(SELECT id FROM records WHERE source_definition_id=?)",[&source])?;
            tx.execute("DELETE FROM record_documents WHERE record_id IN(SELECT id FROM records WHERE source_definition_id=?)",[&source])?;
        }
        tx.execute("INSERT INTO records(id,workspace_id,entity_type,namespace,business_key,source_definition_id,source_sheet,source_row,fields_json,raw_json,record_hash,last_import_generation,active) SELECT id,workspace_id,entity_type,namespace,business_key,source_definition_id,source_sheet,source_row,fields_json,raw_json,record_hash,?,1 FROM stage.records WHERE 1 ON CONFLICT(id) DO UPDATE SET source_sheet=excluded.source_sheet,source_row=excluded.source_row,fields_json=excluded.fields_json,raw_json=excluded.raw_json,last_import_generation=excluded.last_import_generation,active=1,updated_at=CASE WHEN records.record_hash<>excluded.record_hash OR records.active=0 THEN CURRENT_TIMESTAMP ELSE records.updated_at END,record_hash=excluded.record_hash",[run])?;
        tx.execute(
            "INSERT INTO record_field_index SELECT * FROM stage.fields",
            [],
        )?;
        tx.execute(
            "INSERT INTO record_fts(record_id,field_name,content) SELECT * FROM stage.fts",
            [],
        )?;
        tx.execute("INSERT INTO documents(id,workspace_id,original_ref,resolved_path,document_type,availability_status,status_detail,root_path,mapping_id) SELECT * FROM stage.documents WHERE 1 ON CONFLICT(id) DO UPDATE SET original_ref=excluded.original_ref",[])?;
        tx.execute("INSERT INTO record_documents SELECT * FROM stage.links", [])?;
        crate::indexing::rebuild_relationships(&tx, config, Some(run))?;
        for s in sources {
            tx.execute("UPDATE source_state SET status='ready',last_error=NULL,last_success_at=CURRENT_TIMESTAMP,active_generation=?,last_seen_size=?,last_seen_mtime=?,last_seen_hash=? WHERE source_definition_id=?",params![run,fingerprint.size,fingerprint.mtime,config.import_hash(),db::qualified(&config.workspace.id,&s.id)])?;
        }
        ensure!(
            !self.cancelled.load(Ordering::Relaxed),
            "Import cancelled before commit"
        );
        let activate_ms = activate_time.elapsed().as_millis() as i64;
        tx.execute("UPDATE import_runs SET status='succeeded',finished_at=CURRENT_TIMESTAMP,elapsed_ms=?,rows_read=?,rows_created=?,rows_updated=?,rows_deleted=?,rows_unchanged=?,generation=?,stages_json=? WHERE id=?",params![started.elapsed().as_millis() as i64,read,created,updated,deleted,unchanged,run,serde_json::json!({"parse_normalize_validate_stage_ms":parse_ms,"activate_ms":activate_ms}).to_string(),run])?;
        tx.commit()?;
        Ok(ImportCounts {
            read,
            created,
            updated,
            deleted,
            unchanged,
        })
    }
    pub fn store_diagnostic(&self, run: i64, d: &Diagnostic) -> Result<()> {
        self.connection()?.execute("INSERT INTO import_errors(import_id,severity,source_id,sheet,row,field,code,message) VALUES(?,?,?,?,?,?,?,?)",params![run,d.severity,d.source_id,d.sheet,d.row,d.field,d.code,d.message])?;
        Ok(())
    }
    pub fn stage_status(&self, config: &Config, sources: &[&Source], status: &str) -> Result<()> {
        let c = self.connection()?;
        for s in sources {
            c.execute(
                "UPDATE source_state SET status=? WHERE source_definition_id=?",
                params![status, db::qualified(&config.workspace.id, &s.id)],
            )?;
        }
        Ok(())
    }
    /// Synchronous diagnostic import. The desktop always calls this from its background queue.
    pub fn import_file(&self, workspace: &str, workbook: &str) -> Result<ImportCounts> {
        self.sync_workbook(workspace, workbook, true, "diagnostic")?;
        let c = self.connection()?;
        Ok(c.query_row("SELECT rows_read,rows_created,rows_updated,rows_deleted,rows_unchanged FROM import_runs WHERE workspace_id=? AND workbook=? AND status='succeeded' ORDER BY id DESC LIMIT 1",params![workspace,workbook],|r|Ok(ImportCounts{read:r.get(0)?,created:r.get(1)?,updated:r.get(2)?,deleted:r.get(3)?,unchanged:r.get(4)?}))?)
    }
}
