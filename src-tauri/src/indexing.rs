use crate::{
    config::{Config, FieldType},
    db,
    engine::value_text,
    excel::ParsedRecord,
};
use anyhow::Result;
use rusqlite::{params, Connection};

pub fn stage_record(c: &Connection, config: &Config, r: ParsedRecord) -> Result<()> {
    let ws = &config.workspace.id;
    let id = db::identity(ws, &r.entity, &r.namespace, &r.key);
    c.execute(
        "INSERT INTO records VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
        params![
            id,
            ws,
            r.entity,
            r.namespace,
            r.key,
            db::qualified(ws, &r.source_id),
            r.sheet,
            r.row,
            serde_json::to_string(&r.fields)?,
            serde_json::to_string(&r.raw)?,
            r.hash,
            r.source_id
        ],
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "Source {} sheet {} row {}: duplicate scoped business key or staging failure ({e})",
            r.source_id,
            r.sheet,
            r.row
        )
    })?;
    let source = config
        .sources
        .iter()
        .find(|s| s.id == r.source_id)
        .expect("configured source");
    for (name, value) in &r.fields {
        if value.is_null() {
            continue;
        }
        let field = &source.fields[name];
        let text = value_text(value);
        let numeric = if matches!(field.kind, FieldType::Decimal | FieldType::Integer) {
            text.parse::<f64>().ok()
        } else {
            None
        };
        let date = if matches!(field.kind, FieldType::Date | FieldType::Datetime) {
            Some(text.as_str())
        } else {
            None
        };
        let boolean = value.as_bool().map(i64::from);
        c.execute(
            "INSERT INTO fields VALUES(?,?,?,?,?,?)",
            params![id, name, text, numeric, date, boolean],
        )?;
        if field.searchable
            || name == &source.key
            || (r.entity == config.views.main.entity
                && config.views.main.search_fields.contains(name))
        {
            c.execute("INSERT INTO fts VALUES(?,?,?)", params![id, name, text])?;
        }
    }
    for d in config.documents.iter().filter(|d| d.entity == r.entity) {
        let reference = r
            .fields
            .get(&d.path_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if reference.is_empty() {
            continue;
        }
        let root = &config.roots[&d.root];
        let result = crate::documents::resolve(std::path::Path::new(root), reference, d, false);
        let (path, status, error) = match result {
            Ok(p) => (p.to_string_lossy().to_string(), "unchecked", None),
            Err(e) => (reference.to_string(), "blocked", Some(e.to_string())),
        };
        let docid = crate::hash(&serde_json::to_string(&(ws, &d.id, &path))?);
        let ext = std::path::Path::new(&path)
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        c.execute(
            "INSERT OR IGNORE INTO documents VALUES(?,?,?,?,?,?,?,?,?)",
            params![docid, ws, reference, path, ext, status, error, root, d.id],
        )?;
        c.execute(
            "INSERT OR IGNORE INTO links VALUES(?,?,?)",
            params![id, docid, d.id],
        )?;
    }
    Ok(())
}
pub fn rebuild_relationships(c: &Connection, config: &Config, run: Option<i64>) -> Result<()> {
    let ws = &config.workspace.id;
    c.execute(
        "DELETE FROM record_relationships WHERE workspace_id=?",
        [ws],
    )?;
    for rel in &config.relationships {
        let id = db::qualified(ws, &rel.id);
        let mut args = vec![
            id.clone(),
            ws.clone(),
            rel.parent_field.clone(),
            rel.child_field.clone(),
            ws.clone(),
            rel.parent_entity.clone(),
            rel.child_entity.clone(),
        ];
        let scope =
            if let (Some(parent), Some(child)) = (&rel.parent_source_id, &rel.child_source_id) {
                args.push(db::qualified(ws, parent));
                args.push(db::qualified(ws, child));
                "AND p.source_definition_id=? AND ch.source_definition_id=?"
            } else {
                "AND ch.namespace=p.namespace"
            };
        c.execute(&format!("INSERT OR IGNORE INTO record_relationships(relationship_id,workspace_id,parent_record_id,child_record_id) SELECT ?,?,p.id,ch.id FROM records p CROSS JOIN record_field_index pf ON pf.record_id=p.id AND pf.field_name=? CROSS JOIN record_field_index cf ON cf.field_name=? AND cf.text_value=pf.text_value CROSS JOIN records ch ON ch.id=cf.record_id AND ch.workspace_id=p.workspace_id WHERE p.workspace_id=? AND p.entity_type=? AND p.active=1 AND ch.entity_type=? AND ch.active=1 {scope}"),rusqlite::params_from_iter(args))?;
        if let Some(run) = run {
            let mut args: Vec<rusqlite::types::Value> = vec![
                run.into(),
                rel.child_field.clone().into(),
                ws.clone().into(),
                rel.child_entity.clone().into(),
                id.into(),
            ];
            let scope = if let Some(child) = &rel.child_source_id {
                args.push(db::qualified(ws, child).into());
                "AND ch.source_definition_id=?"
            } else {
                ""
            };
            c.execute(&format!("INSERT INTO import_errors(import_id,severity,source_id,sheet,row,field,code,message) SELECT ?,'warning',s.source_id,ch.source_sheet,ch.source_row,?,'unmatched_relation','No matching parent in the configured workspace and relationship scope' FROM records ch JOIN source_definitions s ON s.id=ch.source_definition_id WHERE ch.workspace_id=? AND ch.entity_type=? AND ch.active=1 AND NOT EXISTS(SELECT 1 FROM record_relationships rr INDEXED BY related_child WHERE rr.relationship_id=? AND rr.child_record_id=ch.id) {scope}"),rusqlite::params_from_iter(args))?;
        }
    }
    Ok(())
}
