use explorer_core::{
    config::{Direction, FieldType, PageDefinition, Sort},
    engine::{Engine, Page, QueryOptions},
    nested::NestedViewInput,
};
use rust_xlsxwriter::Workbook;
use serde_json::json;
use std::collections::BTreeMap;

struct Fixture {
    dir: tempfile::TempDir,
    engine: Engine,
    workspace: String,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.xlsx");
        let mut book = Workbook::new();
        let offers = book.add_worksheet();
        offers.set_name("Offers").unwrap();
        for (row, cells) in [
            ["OfferID", "Customer"],
            ["OFF-A", "Harbor"],
            ["OFF-B", "Pine"],
            ["OFF-C", "Cedar"],
        ]
        .iter()
        .enumerate()
        {
            for (col, cell) in cells.iter().enumerate() {
                offers.write_string(row as u32, col as u16, *cell).unwrap();
            }
        }
        let invoices = book.add_worksheet();
        invoices.set_name("Invoices").unwrap();
        for (col, cell) in ["InvoiceID", "OfferID", "Status"].iter().enumerate() {
            invoices.write_string(0, col as u16, *cell).unwrap();
        }
        for row in 1..=30 {
            invoices
                .write_string(row, 0, format!("INV-{row:03}"))
                .unwrap();
            invoices.write_string(row, 1, "OFF-A").unwrap();
            invoices
                .write_string(row, 2, if row == 1 { "Paid" } else { "Open" })
                .unwrap();
        }
        for (i, key) in ["OFF-B", "off-a", "", "UNKNOWN"].iter().enumerate() {
            invoices
                .write_string(i as u32 + 31, 0, format!("OTHER-{i}"))
                .unwrap();
            invoices.write_string(i as u32 + 31, 1, *key).unwrap();
        }
        let protocols = book.add_worksheet();
        protocols.set_name("Protocols").unwrap();
        for (row, cells) in [
            ["ProtocolID", "OfferID", "InvoiceID"],
            ["PR-1", "OFF-A", "INV-001"],
            ["PR-2", "OFF-B", "OTHER-0"],
        ]
        .iter()
        .enumerate()
        {
            for (col, cell) in cells.iter().enumerate() {
                protocols
                    .write_string(row as u32, col as u16, *cell)
                    .unwrap();
            }
        }
        book.save(&path).unwrap();
        let path = std::fs::canonicalize(path).unwrap();
        let engine = Engine::open(dir.path().join("cache.sqlite")).unwrap();
        let workspace = engine
            .add_excel_workbook(&path, None, None, &[])
            .unwrap()
            .id;
        let mut config = engine.config(&workspace).unwrap();
        config.settings.stable_file_check_ms = 1;
        engine.persist_space(&mut config).unwrap();
        engine
            .sync_workbook(&workspace, path.to_str().unwrap(), true, "test")
            .unwrap();
        Self {
            dir,
            engine,
            workspace,
        }
    }
    fn page(&self, title: &str) -> PageDefinition {
        self.engine
            .config(&self.workspace)
            .unwrap()
            .pages
            .into_iter()
            .find(|p| p.title == title)
            .unwrap()
    }
    fn link(
        &self,
        title: &str,
        child: &PageDefinition,
        parent_field: &str,
        child_field: &str,
    ) -> NestedViewInput {
        NestedViewInput {
            id: None,
            title: title.into(),
            page_id: child.id.clone(),
            parent_field: parent_field.into(),
            child_field: child_field.into(),
        }
    }
    fn offers(&self) -> Page {
        let p = self.page("Offers");
        self.engine
            .query(
                &self.workspace,
                &p.entity,
                &QueryOptions {
                    page_id: Some(p.id),
                    ..Default::default()
                },
                None,
            )
            .unwrap()
    }
    fn save(&self) {
        let p = self.page("Offers");
        self.engine
            .save_nested_views(
                &self.workspace,
                &p.id,
                vec![
                    self.link("Invoices", &self.page("Invoices"), "column_1", "column_2"),
                    self.link("Protocols", &self.page("Protocols"), "column_1", "column_2"),
                ],
            )
            .unwrap();
    }
    fn nested(&self, record: &str, index: usize, options: QueryOptions) -> Page {
        let p = self.page("Offers");
        self.engine
            .nested_records(
                &self.workspace,
                &p.id,
                &p.related_views[index].id,
                record,
                options,
            )
            .unwrap()
    }
}

#[test]
fn selecting_offer_scopes_both_lists_with_exact_keys_pagination_and_unchanged_identities() {
    let f = Fixture::new();
    let before = f.offers();
    let sources = f.engine.config(&f.workspace).unwrap().sources;
    assert_ne!(sources[0].namespace, sources[1].namespace);
    let bytes = std::fs::read(f.dir.path().join("project.xlsx")).unwrap();
    f.save();
    let a = &before.records[0].id;
    let b = &before.records[1].id;
    let c = &before.records[2].id;
    let rows = f.nested(
        a,
        0,
        QueryOptions {
            limit: Some(25),
            ..Default::default()
        },
    );
    assert_eq!((rows.total, rows.records.len()), (30, 25));
    assert_eq!(rows.records[0].fields["column_1"], "INV-001");
    assert_eq!(
        f.nested(
            a,
            0,
            QueryOptions {
                offset: 25,
                limit: Some(25),
                ..Default::default()
            }
        )
        .records
        .len(),
        5
    );
    assert_eq!(
        f.nested(a, 1, Default::default()).records[0].fields["column_1"],
        "PR-1"
    );
    assert_eq!(f.nested(b, 0, Default::default()).total, 1);
    assert_eq!(
        f.nested(b, 1, Default::default()).records[0].fields["column_1"],
        "PR-2"
    );
    assert_eq!(f.nested(c, 0, Default::default()).total, 0);
    assert_eq!(f.offers().records[0].id, *a);
    assert_eq!(
        std::fs::read(f.dir.path().join("project.xlsx")).unwrap(),
        bytes
    );
}
#[test]
fn child_view_columns_filters_sort_and_hidden_navigation_work_offline_after_restart() {
    let f = Fixture::new();
    let child = f.page("Invoices");
    let view = f
        .engine
        .create_view(
            &f.workspace,
            child.source_id.as_ref().unwrap(),
            "Open invoices".into(),
            vec!["column_1".into(), "column_3".into()],
            BTreeMap::from([("column_3".into(), json!("Open"))]),
            Some(Sort {
                field: "column_1".into(),
                direction: Direction::Desc,
            }),
        )
        .unwrap();
    let mut hidden = view.clone();
    hidden.hidden = true;
    f.engine.save_page(&f.workspace, hidden).unwrap();
    let parent = f.page("Offers");
    let a = f.offers().records[0].id.clone();
    std::fs::remove_file(f.dir.path().join("project.xlsx")).unwrap();
    f.engine
        .save_nested_views(
            &f.workspace,
            &parent.id,
            vec![f.link("Unpaid", &view, "column_1", "column_2")],
        )
        .unwrap();
    let p = f.page("Offers");
    let e = Engine::open(f.dir.path().join("cache.sqlite")).unwrap();
    let rows = e
        .nested_records(
            &f.workspace,
            &p.id,
            &p.related_views[0].id,
            &a,
            QueryOptions {
                filters: BTreeMap::from([("column_3".into(), json!("Paid"))]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(rows.total, 29);
    assert_eq!(rows.records[0].fields["column_1"], "INV-030");
    assert!(e
        .nested_records(
            &f.workspace,
            &p.id,
            &p.related_views[0].id,
            &a,
            QueryOptions {
                sort: Some("column_2".into()),
                ..Default::default()
            }
        )
        .is_err());
    assert!(e.config(&f.workspace).unwrap().for_page(&view.id).is_err());
}
#[test]
fn deeper_nested_views_and_removing_links_rebuild_cached_edges_without_excel() {
    let f = Fixture::new();
    f.save();
    let parent = f.page("Invoices");
    f.engine
        .save_nested_views(
            &f.workspace,
            &parent.id,
            vec![f.link(
                "Invoice protocols",
                &f.page("Protocols"),
                "column_1",
                "column_3",
            )],
        )
        .unwrap();
    let a = f.offers().records[0].id.clone();
    let invoice = f.nested(&a, 0, Default::default()).records[0].clone();
    let current = f.page("Invoices");
    assert_eq!(
        f.engine
            .nested_records(
                &f.workspace,
                &current.id,
                &current.related_views[0].id,
                &invoice.id,
                Default::default()
            )
            .unwrap()
            .records[0]
            .fields["column_1"],
        "PR-1"
    );
    std::fs::remove_file(f.dir.path().join("project.xlsx")).unwrap();
    let offers = f.page("Offers");
    let old = offers.related_views[0].clone();
    f.engine
        .save_nested_views(&f.workspace, &offers.id, Vec::new())
        .unwrap();
    assert!(f
        .engine
        .nested_records(&f.workspace, &offers.id, &old.id, &a, Default::default())
        .is_err());
    assert!(!f
        .engine
        .config(&f.workspace)
        .unwrap()
        .relationships
        .iter()
        .any(|r| r.id == old.relationship));
    assert_eq!(f.page("Invoices").related_views.len(), 1);
    assert_eq!(f.offers().total, 3);
}
#[test]
fn invalid_nested_updates_preserve_config_and_relationships_and_reject_wrong_parent_scope() {
    let f = Fixture::new();
    f.save();
    let p = f.page("Offers");
    let a = f.offers().records[0].id.clone();
    let hash = f.engine.config(&f.workspace).unwrap().hash();
    let mut bad = f.link("Bad", &f.page("Invoices"), "missing", "column_2");
    assert!(f
        .engine
        .save_nested_views(&f.workspace, &p.id, vec![bad.clone()])
        .is_err());
    bad.parent_field = "column_1".into();
    bad.page_id = "unknown".into();
    assert!(f
        .engine
        .save_nested_views(&f.workspace, &p.id, vec![bad.clone()])
        .is_err());
    bad.page_id = f.page("Invoices").id;
    bad.id = Some("unknown".into());
    assert!(f
        .engine
        .save_nested_views(&f.workspace, &p.id, vec![bad])
        .is_err());
    let mut source = f.engine.config(&f.workspace).unwrap().sources[1].clone();
    source.fields.get_mut("column_2").unwrap().kind = FieldType::Integer;
    assert!(f.engine.update_worksheet(&f.workspace, source).is_err());
    assert_eq!(f.engine.config(&f.workspace).unwrap().hash(), hash);
    let wrong = f.nested(&a, 0, Default::default()).records[0].id.clone();
    assert!(f
        .engine
        .nested_records(
            &f.workspace,
            &p.id,
            &p.related_views[0].id,
            &wrong,
            Default::default()
        )
        .is_err());
    let other = f
        .engine
        .add_excel_workbook(&f.dir.path().join("project.xlsx"), None, None, &[])
        .unwrap()
        .id;
    f.engine
        .sync_workbook(
            &other,
            &f.engine.config(&other).unwrap().sources[0].path,
            true,
            "test",
        )
        .unwrap();
    let op = f.engine.config(&other).unwrap().pages[0].clone();
    let other_record = f
        .engine
        .query(&other, &op.entity, &Default::default(), None)
        .unwrap()
        .records[0]
        .id
        .clone();
    assert!(f
        .engine
        .nested_records(
            &f.workspace,
            &p.id,
            &p.related_views[0].id,
            &other_record,
            Default::default()
        )
        .is_err());
    assert_eq!(f.nested(&a, 0, Default::default()).total, 30);
}

#[test]
fn explicit_source_pairs_exclude_other_namespaces_with_identical_entities_and_keys() {
    let f = Fixture::new();
    f.save();
    let mut config = f.engine.config(&f.workspace).unwrap();
    for i in 0..2 {
        let mut extra = config.sources[i].clone();
        extra.id = format!("shadow_{i}");
        extra.namespace = format!("shadow_{i}");
        config.sources.push(extra);
    }
    f.engine.persist_space(&mut config).unwrap();
    f.engine
        .sync_workbook(&f.workspace, &config.sources[0].path, true, "test")
        .unwrap();
    let a = f.offers().records[0].id.clone();
    let rows = f.nested(&a, 0, Default::default());
    assert_eq!(rows.total, 30);
    assert!(rows
        .records
        .iter()
        .all(|r| r.source_id == config.sources[1].id));
    let shadow = f
        .engine
        .query(
            &f.workspace,
            &config.sources[0].entity,
            &Default::default(),
            None,
        )
        .unwrap()
        .records
        .into_iter()
        .find(|r| r.source_id == "shadow_0")
        .unwrap();
    let parent = f.page("Offers");
    assert!(f
        .engine
        .nested_records(
            &f.workspace,
            &parent.id,
            &parent.related_views[0].id,
            &shadow.id,
            Default::default()
        )
        .is_err());
    let sibling = f
        .engine
        .create_view(
            &f.workspace,
            &config.sources[0].id,
            "Other offer view".into(),
            vec!["column_1".into()],
            Default::default(),
            None,
        )
        .unwrap();
    assert!(sibling.related_views.is_empty());
}
