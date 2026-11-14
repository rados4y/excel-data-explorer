use calamine::Data;
use explorer_core::{
    config::{Config, Field, FieldType},
    excel::{normalize, parse_workbook},
};
use std::path::Path;
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
#[test]
fn multi_sheet_and_alternate_layout() {
    let c = Config::load(&root().join("workspaces/demo-workspace.yaml")).unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for (path, sources) in c.workbook_groups() {
        parse_workbook(
            Path::new(&path),
            &sources,
            &c,
            |r| {
                *counts.entry(r.entity).or_insert(0) += 1;
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();
    }
    assert_eq!(counts["offer"], 120);
    assert_eq!(counts["invoice"], 241);
    assert_eq!(counts["protocol"], 60);
    let c = Config::load(&root().join("workspaces/alternate-workspace.yaml")).unwrap();
    let mut rows = vec![];
    parse_workbook(
        Path::new(&c.sources[0].path),
        &[&c.sources[0]],
        &c,
        |r| {
            rows.push(r);
            Ok(())
        },
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(rows[0].key, "0007");
    assert_eq!(rows[0].fields["amount"], "123.45");
    assert_eq!(rows[0].fields["customer"], "Acme");
}
fn field(kind: FieldType) -> Field {
    Field {
        column: "X".into(),
        kind,
        required: true,
        trim: true,
        searchable: false,
        filterable: false,
        label: None,
    }
}
#[test]
fn types_and_date_policy() {
    assert_eq!(
        normalize(
            &Data::String(" 0007 ".into()),
            &field(FieldType::String),
            "UTC"
        )
        .unwrap(),
        "0007"
    );
    assert_eq!(
        normalize(
            &Data::String("0.1000".into()),
            &field(FieldType::Decimal),
            "UTC"
        )
        .unwrap(),
        "0.1"
    );
    assert_eq!(
        normalize(
            &Data::String("2026-03-01".into()),
            &field(FieldType::Date),
            "UTC"
        )
        .unwrap(),
        "2026-03-01"
    );
    assert!(normalize(
        &Data::String("1.2".into()),
        &field(FieldType::Integer),
        "UTC"
    )
    .is_err());
    assert_eq!(
        normalize(&Data::Bool(true), &field(FieldType::Boolean), "UTC").unwrap(),
        true
    );
    assert!(normalize(&Data::Empty, &field(FieldType::String), "UTC").is_err());
    assert!(normalize(
        &Data::String("2026-10-25T02:30:00".into()),
        &field(FieldType::Datetime),
        "Europe/Warsaw"
    )
    .is_err());
}
#[test]
fn config_rejects_bad_references_duplicates_and_types() {
    let text = std::fs::read_to_string(root().join("workspaces/demo-workspace.yaml")).unwrap();
    for broken in [
        text.replace("type: decimal", "type: money"),
        text.replace("id: invoices", "id: offers"),
        text.replace("child_field: offer_id", "child_field: missing"),
        text.replace("filters: [status]", "filters: [missing]"),
        text.replace("${data}", "${env:HOME}"),
        text.replace("key: id", "key: missing"),
    ] {
        assert!(Config::parse(&broken, &root()).is_err());
    }
}
#[test]
fn reordered_headers_excel_dates_and_optional_diagnostics() {
    use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("layout.xlsx");
    let mut w = Workbook::new();
    let s = w.add_worksheet();
    s.set_name("Offers").unwrap();
    for (i, h) in [
        "Amount",
        "Customer",
        "CreatedAt",
        "Status",
        "Description",
        "OfferID",
    ]
    .iter()
    .enumerate()
    {
        s.write_string(0, i as u16, *h).unwrap();
    }
    s.write_string(1, 0, "not decimal").unwrap();
    s.write_string(1, 1, "Customer").unwrap();
    s.write_datetime_with_format(
        1,
        2,
        ExcelDateTime::parse_from_str("2026-03-01").unwrap(),
        &Format::new().set_num_format("yyyy-mm-dd"),
    )
    .unwrap();
    s.write_string(1, 3, "Active").unwrap();
    s.write_string(1, 4, "Fixture").unwrap();
    s.write_string(1, 5, "0007").unwrap();
    w.save(&path).unwrap();
    let mut c = Config::load(&root().join("workspaces/demo-workspace.yaml")).unwrap();
    c.sources.truncate(1);
    c.sources[0].path = path.to_string_lossy().to_string();
    let mut rows = vec![];
    let mut warnings = vec![];
    parse_workbook(
        &path,
        &[&c.sources[0]],
        &c,
        |r| {
            rows.push(r);
            Ok(())
        },
        |d| {
            warnings.push(d);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(rows[0].key, "0007");
    assert_eq!(rows[0].fields["created_at"], "2026-03-01");
    assert!(rows[0].fields["amount"].is_null());
    assert_eq!(warnings[0].field.as_deref(), Some("amount"));
    assert_eq!(warnings[0].row, 2);
}
#[test]
fn formula_without_cached_key_rejects_source() {
    use rust_xlsxwriter::{Formula, Workbook};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("formula.xlsx");
    let mut w = Workbook::new();
    let s = w.add_worksheet();
    s.set_name("Orders").unwrap();
    s.write_string(0, 0, "Client").unwrap();
    s.write_string(0, 1, "Code").unwrap();
    s.write_string(0, 2, "Total").unwrap();
    s.set_formula_result_default("");
    s.write_formula(1, 1, Formula::new("=A99").set_result(""))
        .unwrap();
    w.save(&path).unwrap();
    let c = Config::load(&root().join("workspaces/alternate-workspace.yaml")).unwrap();
    let mut diagnostics = vec![];
    let result = parse_workbook(
        &path,
        &[&c.sources[0]],
        &c,
        |_| Ok(()),
        |d| {
            diagnostics.push(d);
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(diagnostics[0].code, "formula_without_cache");
}
#[test]
fn datetimes_sort_as_utc_instants() {
    assert_eq!(
        normalize(
            &Data::String("2026-03-01T03:00:00+02:00".into()),
            &field(FieldType::Datetime),
            "Europe/Warsaw"
        )
        .unwrap(),
        "2026-03-01T01:00:00.000000000Z"
    );
}
