use explorer_core::{
    config::{Config, Direction, FieldType, KeyMode, PageKind, Sort},
    engine::{Engine, QueryOptions},
};
use rust_xlsxwriter::Workbook;
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
struct Fixture {
    dir: tempfile::TempDir,
    engine: Engine,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.xlsx");
        let mut book = Workbook::new();
        {
            let s = book.add_worksheet();
            s.set_name("Offers").unwrap();
            for (i, h) in ["OfferID", "Customer", "Amount", "Status"]
                .iter()
                .enumerate()
            {
                s.write_string(0, i as u16, *h).unwrap();
            }
            for (i, row) in [
                ["001", "Harbor", "12.5000", "Active"],
                ["001", "Pine", "4.25", "Draft"],
                ["003", "Cedar", "7", "Active"],
            ]
            .iter()
            .enumerate()
            {
                for (j, v) in row.iter().enumerate() {
                    s.write_string(i as u32 + 1, j as u16, *v).unwrap();
                }
            }
        }
        {
            let s = book.add_worksheet();
            s.set_name("Invoices").unwrap();
            s.write_string(0, 0, "Number").unwrap();
            s.write_string(0, 1, "Number").unwrap();
            s.write_string(1, 0, "INV-1").unwrap();
            s.write_string(1, 1, "001").unwrap();
            s.write_string(1, 2, "Blank header still maps by column position")
                .unwrap();
        }
        book.add_worksheet().set_name("Empty").unwrap();
        book.save(&path).unwrap();
        let path = std::fs::canonicalize(path).unwrap();
        let engine = Engine::open(dir.path().join("cache.sqlite")).unwrap();
        Self { dir, engine, path }
    }
    fn add(&self) -> String {
        let id = self
            .engine
            .add_excel_workbook(&self.path, None, None, &[])
            .unwrap()
            .id;
        let mut config = self.engine.config(&id).unwrap();
        config.settings.stable_file_check_ms = 1;
        self.engine.persist_space(&mut config).unwrap();
        id
    }
    fn import(&self, id: &str) {
        for path in self.engine.config(id).unwrap().workbook_groups().keys() {
            self.engine.sync_workbook(id, path, false, "test").unwrap();
        }
    }
    fn page(&self, id: &str, sheet: &str) -> explorer_core::config::PageDefinition {
        self.engine
            .config(id)
            .unwrap()
            .effective_pages()
            .into_iter()
            .find(|p| p.title == sheet)
            .unwrap()
    }
}
#[test]
fn starts_without_any_configuration_and_persists_an_empty_space() {
    let f = Fixture::new();
    assert!(f.engine.list_workspaces().unwrap().is_empty());
    let s = f.engine.create_space("Finance area".into()).unwrap();
    assert!(s.config_path.starts_with("app://"));
    let c = f.engine.config(&s.id).unwrap();
    assert!(c.sources.is_empty() && c.pages.is_empty());
    let e = Engine::open(f.dir.path().join("cache.sqlite")).unwrap();
    assert_eq!(e.list_workspaces().unwrap()[0].name, "Finance area");
    assert!(!f.dir.path().join("workspace.yaml").exists());
}
#[test]
fn adds_one_workbook_as_one_space_with_each_worksheet_as_a_page() {
    let f = Fixture::new();
    let bytes = std::fs::read(&f.path).unwrap();
    let preview = f.engine.inspect_workbook(&f.path, None, None).unwrap();
    assert_eq!(preview.worksheets.len(), 3);
    assert_eq!(preview.worksheets[0].columns[0].label, "OfferID");
    let id = f.add();
    f.import(&id);
    let c = f.engine.config(&id).unwrap();
    assert_eq!(c.pages.len(), 3);
    assert!(c.pages.iter().all(|p| p.kind == PageKind::Worksheet));
    let offers = f.page(&id, "Offers");
    let rows = f
        .engine
        .query(
            &id,
            &offers.entity,
            &QueryOptions {
                page_id: Some(offers.id),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert_eq!(rows.total, 3);
    assert_eq!(rows.records[0].fields["column_1"], "001");
    let invoices = f.page(&id, "Invoices");
    assert_eq!(
        f.engine
            .query(
                &id,
                &invoices.entity,
                &QueryOptions {
                    page_id: Some(invoices.id),
                    ..Default::default()
                },
                None
            )
            .unwrap()
            .total,
        1
    );
    let empty = f.page(&id, "Empty");
    assert_eq!(
        f.engine
            .query(&id, &empty.entity, &Default::default(), None)
            .unwrap()
            .total,
        0
    );
    assert_eq!(std::fs::read(&f.path).unwrap(), bytes);
}
#[test]
fn default_workbooks_use_separate_spaces_and_can_be_grouped_explicitly() {
    let f = Fixture::new();
    let first = f.add();
    let second = f.add();
    assert_ne!(first, second);
    let group = f.engine.create_space("Combined".into()).unwrap();
    f.engine
        .add_excel_workbook(&f.path, Some(&group.id), None, &[])
        .unwrap();
    f.engine
        .add_excel_workbook(&f.path, Some(&group.id), None, &[])
        .unwrap();
    assert_eq!(f.engine.config(&group.id).unwrap().pages.len(), 6);
    assert_eq!(f.engine.list_workspaces().unwrap().len(), 3);
}
#[test]
fn saved_views_use_authoritative_filters_and_survive_offline_restart() {
    let f = Fixture::new();
    let id = f.add();
    f.import(&id);
    let worksheet = f.page(&id, "Offers");
    let mut filters = BTreeMap::new();
    filters.insert("column_4".into(), json!("Active"));
    let view = f
        .engine
        .create_view(
            &id,
            worksheet.source_id.as_ref().unwrap(),
            "Active offers".into(),
            vec!["column_1".into(), "column_2".into(), "column_4".into()],
            filters,
            Some(Sort {
                field: "column_2".into(),
                direction: Direction::Desc,
            }),
        )
        .unwrap();
    std::fs::remove_file(&f.path).unwrap();
    let e = Engine::open(f.dir.path().join("cache.sqlite")).unwrap();
    let result = e
        .query(
            &id,
            &view.entity,
            &QueryOptions {
                page_id: Some(view.id.clone()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.records[0].fields["column_2"], "Harbor");
    let mut override_filter = BTreeMap::new();
    override_filter.insert("column_4".into(), json!("Draft"));
    assert_eq!(
        e.query(
            &id,
            &view.entity,
            &QueryOptions {
                page_id: Some(view.id),
                filters: override_filter,
                ..Default::default()
            },
            None
        )
        .unwrap()
        .total,
        2
    );
}
#[test]
fn column_visibility_and_page_visibility_do_not_require_excel_or_invalidate_fingerprints() {
    let f = Fixture::new();
    let id = f.add();
    f.import(&id);
    let hash = f.engine.config(&id).unwrap().import_hash();
    let mut page = f.page(&id, "Offers");
    page.columns = vec!["column_1".into(), "column_2".into()];
    page.search_fields = page.columns.clone();
    f.engine.save_page(&id, page.clone()).unwrap();
    assert_eq!(f.engine.config(&id).unwrap().import_hash(), hash);
    assert!(!f
        .engine
        .sync_workbook(&id, f.path.to_str().unwrap(), false, "unchanged")
        .unwrap());
    std::fs::remove_file(&f.path).unwrap();
    page.hidden = true;
    f.engine.save_page(&id, page.clone()).unwrap();
    assert!(f.engine.config(&id).unwrap().for_page(&page.id).is_err());
    page.hidden = false;
    f.engine.save_page(&id, page.clone()).unwrap();
    assert_eq!(
        f.engine
            .config(&id)
            .unwrap()
            .for_page(&page.id)
            .unwrap()
            .views
            .main
            .columns
            .len(),
        2
    );
    assert_eq!(
        f.engine
            .query(
                &id,
                &page.entity,
                &QueryOptions {
                    page_id: Some(page.id),
                    ..Default::default()
                },
                None
            )
            .unwrap()
            .total,
        3
    );
}
#[test]
fn worksheet_types_and_unique_keys_are_validated_without_destroying_cache() {
    let f = Fixture::new();
    let id = f.add();
    f.import(&id);
    let mut source = f.engine.config(&id).unwrap().sources[0].clone();
    source.fields.get_mut("column_3").unwrap().kind = FieldType::Decimal;
    f.engine.update_worksheet(&id, source.clone()).unwrap();
    f.import(&id);
    let page = f.page(&id, "Offers");
    let records = f
        .engine
        .query(&id, &page.entity, &Default::default(), None)
        .unwrap();
    assert_eq!(
        records
            .records
            .iter()
            .find(|r| r.fields["column_2"] == "Harbor")
            .unwrap()
            .fields["column_3"],
        "12.5"
    );
    source.key = "column_1".into();
    source.key_mode = KeyMode::Field;
    source.fields.remove("__row_id");
    source.fields.get_mut("column_1").unwrap().required = true;
    f.engine.update_worksheet(&id, source).unwrap();
    assert!(f
        .engine
        .sync_workbook(&id, f.path.to_str().unwrap(), true, "duplicate-key")
        .is_err());
    assert_eq!(
        f.engine
            .query(&id, &page.entity, &Default::default(), None)
            .unwrap()
            .total,
        3
    );
}
#[test]
fn invalid_page_or_mapping_updates_leave_the_active_config_unchanged() {
    let f = Fixture::new();
    let id = f.add();
    let before = f.engine.config(&id).unwrap().hash();
    let mut page = f.page(&id, "Offers");
    page.columns = Vec::new();
    assert!(f.engine.save_page(&id, page).is_err());
    let mut source = f.engine.config(&id).unwrap().sources[0].clone();
    source.header_row = 0;
    assert!(f.engine.update_worksheet(&id, source).is_err());
    assert_eq!(f.engine.config(&id).unwrap().hash(), before);
    assert!(f
        .engine
        .inspect_workbook(Path::new("/not-a-file.exe"), None, None)
        .is_err());
}
#[test]
fn legacy_yaml_pages_remain_compatible_when_views_are_added() {
    let f = Fixture::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let w = f
        .engine
        .add_config(&root.join("workspaces/alternate-workspace.yaml"))
        .unwrap();
    let config = f.engine.config(&w.id).unwrap();
    assert_eq!(config.effective_pages()[0].id, "main");
    let v = f
        .engine
        .create_view(
            &w.id,
            "orders",
            "Customer lookup".into(),
            vec!["code".into(), "customer".into()],
            BTreeMap::new(),
            None,
        )
        .unwrap();
    let c = f.engine.config(&w.id).unwrap();
    assert_eq!(c.effective_pages().len(), 2);
    assert_eq!(
        c.for_page(&v.id).unwrap().views.main.title,
        "Customer lookup"
    );
    assert!(c.managed);
    Config::parse(
        &std::fs::read_to_string(root.join("workspaces/demo-workspace.yaml")).unwrap(),
        root,
    )
    .unwrap();
}
#[test]
fn inferred_unique_headers_keep_their_meaning_when_excel_columns_are_reordered() {
    let f = Fixture::new();
    let id = f.add();
    f.import(&id);
    let c = f.engine.config(&id).unwrap();
    assert!(c.sources[0].fields["column_1"].column_index.is_none());
    assert_eq!(c.sources[1].fields["column_1"].column_index, Some(1));
    assert_eq!(c.sources[1].fields["column_2"].column_index, Some(2));
    assert_eq!(c.sources[1].fields["column_3"].column_index, Some(3));
    let mut book = Workbook::new();
    {
        let s = book.add_worksheet();
        s.set_name("Offers").unwrap();
        for (i, h) in ["Customer", "Status", "Amount", "OfferID"]
            .iter()
            .enumerate()
        {
            s.write_string(0, i as u16, *h).unwrap();
        }
        for (i, row) in [
            ["Harbor", "Active", "12.5000", "001"],
            ["Pine", "Draft", "4.25", "001"],
            ["Cedar", "Active", "7", "003"],
        ]
        .iter()
        .enumerate()
        {
            for (j, v) in row.iter().enumerate() {
                s.write_string(i as u32 + 1, j as u16, *v).unwrap();
            }
        }
    }
    {
        let s = book.add_worksheet();
        s.set_name("Invoices").unwrap();
        s.write_string(0, 0, "Number").unwrap();
        s.write_string(0, 1, "Number").unwrap();
        s.write_string(1, 0, "INV-1").unwrap();
        s.write_string(1, 1, "001").unwrap();
        s.write_string(1, 2, "Blank header still maps by column position")
            .unwrap();
    }
    book.add_worksheet().set_name("Empty").unwrap();
    book.save(&f.path).unwrap();
    f.import(&id);
    let page = f.page(&id, "Offers");
    let records = f
        .engine
        .query(&id, &page.entity, &Default::default(), None)
        .unwrap()
        .records;
    let harbor = records
        .iter()
        .find(|r| r.fields["column_2"] == "Harbor")
        .unwrap();
    assert_eq!(harbor.fields["column_1"], "001");
    assert_eq!(harbor.fields["column_4"], "Active");
}
