use anyhow::{ensure, Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::{path::Path, time::Duration};
pub fn connect(path: &Path) -> Result<Connection> {
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    c.busy_timeout(Duration::from_secs(5))?;
    c.pragma_update(None, "foreign_keys", "ON")?;
    Ok(c)
}
pub fn initialize(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut c = connect(path)?;
    c.pragma_update(None, "journal_mode", "WAL")?;
    c.pragma_update(None, "wal_autocheckpoint", 1000)?;
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    ensure!(
        version <= 1,
        "Database schema is newer than this application; database left intact"
    );
    if version == 0 {
        let tx = c.transaction()?;
        tx.execute_batch(include_str!("schema.sql"))
            .context("Database migration failed; existing database preserved")?;
        tx.commit()?;
    }
    // Crash recovery never touches previously activated rows.
    c.execute("UPDATE import_runs SET status='cancelled',error='Interrupted before completion',finished_at=CURRENT_TIMESTAMP WHERE status NOT IN ('succeeded','failed','cancelled','unchanged')",[])?;
    c.execute("UPDATE source_state SET status=CASE WHEN last_success_at IS NULL THEN 'never' ELSE 'error' END,last_error='Previous synchronization interrupted; using cached data' WHERE status IN ('checking','snapshotting','parsing','staging','activating')",[])?;
    // Single-instance startup recovery: remove only this database's abandoned local stages.
    if let Some(parent) = path.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&stage_prefix(path))
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
    Ok(())
}
pub fn qualified(workspace: &str, id: &str) -> String {
    format!("{workspace}/{id}")
}
pub fn identity(workspace: &str, entity: &str, namespace: &str, key: &str) -> String {
    crate::hash(
        &serde_json::to_string(&(workspace, entity, namespace, key))
            .expect("serializable identity"),
    )
}

pub fn stage_prefix(path: &Path) -> String {
    format!("ede-stage-{}-", &crate::hash(&path.to_string_lossy())[..16])
}
