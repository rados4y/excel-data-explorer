mod common;
use common::Fixture;
use explorer_core::{db, engine::QueryOptions};
use serde_json::json;
#[test]
fn offline_queries_relations_and_documents() {
    let f = Fixture::new();
    f.import();
    for path in f.config.workbook_groups().keys() {
        std::fs::remove_file(path).unwrap();
    }
    let options = QueryOptions {
        search: "OFF-001".into(),
        ..Default::default()
    };
    let page = f.engine.query("demo", "offer", &options, None).unwrap();
    assert_eq!(page.total, 1);
    let id = &page.records[0].id;
    assert_eq!(
        f.engine
            .related(id, "offer_invoices", &Default::default())
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        f.engine
            .related(id, "offer_protocols", &Default::default())
            .unwrap()
            .total,
        1
    );
    assert_eq!(f.engine.documents(id, true).unwrap().len(), 3);
    let runs = f.engine.history(Some("demo"), None, 0).unwrap();
    assert!(runs.iter().any(|r| !f
        .engine
        .errors(r["id"].as_i64().unwrap())
        .unwrap()
        .is_empty()));
}
#[test]
fn atomic_workbook_failure_and_duplicates_preserve_cache() {
    let f = Fixture::new();
    f.import();
    let count = f
        .engine
        .query("demo", "offer", &Default::default(), None)
        .unwrap()
        .total;
    f.offers(&[("OFF-001", "A"), ("OFF-001", "B")]);
    assert!(f
        .engine
        .import_file("demo", &f.config.sources[0].path)
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        count
    );
    let mut book = rust_xlsxwriter::Workbook::new();
    book.add_worksheet()
        .set_name("Invoices")
        .unwrap()
        .write_string(0, 0, "wrong")
        .unwrap();
    book.save(&f.config.sources[1].path).unwrap();
    assert!(f
        .engine
        .import_file("demo", &f.config.sources[1].path)
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "invoice", &Default::default(), None)
            .unwrap()
            .total,
        241
    );
    assert_eq!(
        f.engine
            .query("demo", "protocol", &Default::default(), None)
            .unwrap()
            .total,
        60
    );
}
#[test]
fn removals_rekeys_and_provenance() {
    let f = Fixture::new();
    f.import();
    let id = db::identity("demo", "offer", "primary", "OFF-001");
    assert_eq!(
        f.engine
            .related(&id, "offer_invoices", &Default::default())
            .unwrap()
            .total,
        2
    );
    f.offers(&[("NEW-001", "New"), ("OFF-002", "Other")]);
    f.engine
        .import_file("demo", &f.config.sources[0].path)
        .unwrap();
    assert!(f.engine.record(&id).is_err());
    assert_eq!(
        f.engine
            .query("demo", "invoice", &Default::default(), None)
            .unwrap()
            .total,
        241
    );
    let id = db::identity("demo", "offer", "primary", "NEW-001");
    assert_eq!(
        f.engine
            .related(&id, "offer_invoices", &Default::default())
            .unwrap()
            .total,
        0
    );
    assert_eq!(f.engine.record(&id).unwrap().source_row, 2);
}
#[test]
fn deterministic_safe_pages_filters_and_search() {
    let f = Fixture::new();
    f.import();
    let mut q = QueryOptions {
        limit: Some(7),
        sort: Some("status".into()),
        ..Default::default()
    };
    let a = f.engine.query("demo", "offer", &q, None).unwrap();
    q.offset = 7;
    let b = f.engine.query("demo", "offer", &q, None).unwrap();
    assert!(a
        .records
        .iter()
        .all(|r| b.records.iter().all(|x| x.id != r.id)));
    q.offset = 0;
    q.filters.insert("status".into(), json!("Active"));
    assert_eq!(f.engine.query("demo", "offer", &q, None).unwrap().total, 40);
    q.sort = Some("id); DROP TABLE records;--".into());
    assert!(f.engine.query("demo", "offer", &q, None).is_err());
    q.sort = None;
    q.filters.clear();
    q.search = "\" OR * --".into();
    assert!(f.engine.query("demo", "offer", &q, None).is_ok());
    q.limit = Some(201);
    assert!(f.engine.query("demo", "offer", &q, None).is_err());
}
#[test]
fn namespaces_isolate_same_key_and_invalid_config_keeps_active() {
    let mut f = Fixture::new();
    f.import();
    let mut source = f.config.sources[0].clone();
    source.id = "secondary".into();
    source.namespace = "secondary".into();
    f.config.sources.push(source);
    std::fs::write(&f.yaml, serde_yaml::to_string(&f.config).unwrap()).unwrap();
    f.engine.add_config(&f.yaml).unwrap();
    f.engine
        .import_file("demo", &f.config.sources[0].path)
        .unwrap();
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        240
    );
    let id = db::identity("demo", "offer", "secondary", "OFF-001");
    assert_eq!(
        f.engine
            .related(&id, "offer_invoices", &Default::default())
            .unwrap()
            .total,
        0
    );
    std::fs::write(&f.yaml, "invalid: [").unwrap();
    assert!(f.engine.add_config(&f.yaml).is_err());
    assert_eq!(f.engine.config("demo").unwrap().sources.len(), 4);
}
#[test]
fn shared_namespace_collision_is_rejected() {
    let mut f = Fixture::new();
    f.import();
    let mut source = f.config.sources[0].clone();
    source.id = "collider".into();
    f.config.sources.push(source);
    std::fs::write(&f.yaml, serde_yaml::to_string(&f.config).unwrap()).unwrap();
    f.engine.add_config(&f.yaml).unwrap();
    assert!(f
        .engine
        .import_file("demo", &f.config.sources[0].path)
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
}
#[test]
fn preferences_and_ui_state_survive_restart() {
    let f = Fixture::new();
    let mut s = f.engine.settings().unwrap();
    s.autostart = false;
    s.selected_workspace = Some("demo".into());
    f.engine.update_settings(s).unwrap();
    f.engine
        .save_ui_state("demo", &json!({"search":"ABC"}))
        .unwrap();
    let e = explorer_core::engine::Engine::open(&f.engine.db_path).unwrap();
    assert!(!e.settings().unwrap().autostart);
    assert_eq!(e.ui_state("demo").unwrap()["search"], "ABC");
}
#[test]
fn relationships_follow_indexed_join_order() {
    let f = Fixture::new();
    f.import();
    let c = f.engine.connection().unwrap();
    let mut stmt=c.prepare("EXPLAIN QUERY PLAN SELECT p.id,ch.id FROM records p CROSS JOIN record_field_index pf ON pf.record_id=p.id AND pf.field_name='id' CROSS JOIN record_field_index cf ON cf.field_name='offer_id' AND cf.text_value=pf.text_value CROSS JOIN records ch ON ch.id=cf.record_id AND ch.workspace_id=p.workspace_id AND ch.namespace=p.namespace WHERE p.workspace_id='demo' AND p.entity_type='offer' AND p.active=1 AND ch.entity_type='invoice' AND ch.active=1").unwrap();
    let plan = stmt
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(plan.iter().any(|s| s.contains("field_text")));
    let child = plan.iter().find(|s| s.contains("SEARCH ch")).unwrap();
    assert!(
        child.contains("id=?"),
        "Child must use identity lookup: {plan:?}"
    );
}
#[test]
fn document_availability_and_stable_identity_after_reorder() {
    let f = Fixture::new();
    f.import();
    assert_eq!(f.engine.refresh_documents(Some("demo")).unwrap(), 4);
    let id = db::identity("demo", "offer", "primary", "OFF-001");
    assert!(f
        .engine
        .documents(&id, true)
        .unwrap()
        .iter()
        .all(|d| d.availability_status == "available"));
    let missing = db::identity("demo", "invoice", "primary", "INV-000003");
    assert_eq!(
        f.engine.documents(&missing, false).unwrap()[0].availability_status,
        "missing"
    );
    f.offers(&[("OFF-002", "Other"), ("OFF-001", "Moved")]);
    f.engine
        .import_file("demo", &f.config.sources[0].path)
        .unwrap();
    let record = f.engine.record(&id).unwrap();
    assert_eq!(record.source_row, 3);
    assert_eq!(record.business_key, "OFF-001");
    assert_eq!(
        f.engine
            .related(&id, "offer_invoices", &Default::default())
            .unwrap()
            .total,
        2
    );
}
#[test]
fn second_sheet_validation_rolls_back_first_sheet() {
    let f = Fixture::new();
    f.import();
    let mut book = rust_xlsxwriter::Workbook::new();
    let s = book.add_worksheet();
    s.set_name("Invoices").unwrap();
    for (i, h) in ["InvoiceID", "OfferID", "InvoiceDate", "Amount", "PDF"]
        .iter()
        .enumerate()
    {
        s.write_string(0, i as u16, *h).unwrap();
    }
    s.write_string(1, 0, "INV-NEW").unwrap();
    s.write_string(1, 1, "OFF-001").unwrap();
    book.add_worksheet()
        .set_name("Protocols")
        .unwrap()
        .write_string(0, 0, "invalid header")
        .unwrap();
    book.save(&f.config.sources[1].path).unwrap();
    assert!(f
        .engine
        .import_file("demo", &f.config.sources[1].path)
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "invoice", &Default::default(), None)
            .unwrap()
            .total,
        241
    );
    assert_eq!(
        f.engine
            .query("demo", "protocol", &Default::default(), None)
            .unwrap()
            .total,
        60
    );
}
