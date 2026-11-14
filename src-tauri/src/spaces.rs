//! Configuration-free workbook onboarding and persistent space/page editing.
//! Workbook inspection is explicit, snapshot based and kept out of interactive queries.
use crate::{
    config::*,
    engine::{Engine, WorkspaceSummary},
};
use anyhow::{ensure, Context, Result};
use calamine::{open_workbook_auto, Data, DataType, Reader};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnPreview {
    pub id: String,
    pub column: String,
    pub column_index: u32,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorksheetPreview {
    pub name: String,
    pub header_row: u32,
    pub columns: Vec<ColumnPreview>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: u32,
    pub empty: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkbookPreview {
    pub path: String,
    pub name: String,
    pub worksheets: Vec<WorksheetPreview>,
}
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::DateTime(d) => d.as_datetime().map(|d| d.to_string()).unwrap_or_default(),
        _ => cell.to_string(),
    }
}
fn fresh_id(prefix: &str) -> String {
    format!(
        "{prefix}_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
impl Config {
    pub fn empty_space(id: String, name: String) -> Self {
        Self {
            version: 1,
            workspace: Workspace { id, name },
            roots: BTreeMap::new(),
            settings: ImportSettings::default(),
            sources: Vec::new(),
            relationships: Vec::new(),
            documents: Vec::new(),
            views: Views {
                main: MainView {
                    entity: "empty".into(),
                    title: "Pages".into(),
                    columns: Vec::new(),
                    default_sort: Sort {
                        field: String::new(),
                        direction: Direction::Asc,
                    },
                    search_fields: Vec::new(),
                    filters: Vec::new(),
                },
                detail: DetailView {
                    entity: "empty".into(),
                    title_field: String::new(),
                    tabs: Vec::new(),
                },
            },
            managed: true,
            pages: Vec::new(),
        }
    }
    fn reset_root_view(&mut self) {
        if let Some(page) = self.pages.first() {
            let page = page.clone();
            // Root view is only a compatibility fallback. Actual page requests use their own schema.
            self.views.main = MainView {
                entity: page.entity.clone(),
                title: page.title,
                columns: page.columns.clone(),
                default_sort: page.default_sort,
                search_fields: page.search_fields,
                filters: page.filters,
            };
            self.views.detail = DetailView {
                entity: page.entity,
                title_field: page.columns[0].clone(),
                tabs: vec![
                    Tab {
                        title: "Overview".into(),
                        kind: TabType::Fields,
                        relationship: None,
                    },
                    Tab {
                        title: "Documents".into(),
                        kind: TabType::RelatedDocuments,
                        relationship: None,
                    },
                ],
            };
        }
    }
}
impl Engine {
    pub fn inspect_workbook(
        &self,
        path: &Path,
        sheet: Option<&str>,
        header_row: Option<u32>,
    ) -> Result<WorkbookPreview> {
        let path = std::fs::canonicalize(path).context("Cannot access selected workbook")?;
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        ensure!(
            ["xlsx", "xls", "xlsm", "xlsb", "ods"].contains(&extension.as_str()),
            "Select an Excel or OpenDocument workbook"
        );
        if let Some(row) = header_row {
            ensure!(
                (1..=1_048_576).contains(&row),
                "Header row must be 1..1048576"
            );
        }
        let config = Config::empty_space("inspection".into(), "Workbook inspection".into());
        let (snapshot, _) = crate::sync::stable_snapshot(&path, &config, &self.cancelled)?;
        let mut reader =
            open_workbook_auto(snapshot.path()).context("Unable to inspect this workbook")?;
        let names = reader.sheet_names().to_vec();
        ensure!(!names.is_empty(), "Workbook has no worksheets");
        if let Some(sheet) = sheet {
            ensure!(names.iter().any(|n| n == sheet), "Worksheet not found");
        }
        let mut worksheets = Vec::new();
        for name in names.into_iter().filter(|n| sheet.is_none_or(|s| s == n)) {
            ensure!(
                !self.cancelled.load(Ordering::Relaxed),
                "Inspection cancelled"
            );
            let range = reader
                .worksheet_range(&name)
                .with_context(|| format!("Cannot read worksheet {name}"))?;
            let first = range.start().unwrap_or((0, 0));
            let header = header_row.unwrap_or(first.0 + 1);
            let last = range.end().unwrap_or((0, 0));
            let mut columns = Vec::new();
            if !range.is_empty() {
                for index in first.1..=last.1 {
                    let value = range
                        .get_value((header - 1, index))
                        .map(cell_text)
                        .unwrap_or_default();
                    let label = if value.trim().is_empty() {
                        format!("Column {}", index + 1)
                    } else {
                        value.clone()
                    };
                    columns.push(ColumnPreview {
                        id: format!("column_{}", index + 1),
                        column: value,
                        column_index: index + 1,
                        label,
                    });
                }
            }
            let mut rows = Vec::new();
            let mut row_count = 0;
            for row in header..=last.0 {
                let cells: Vec<_> = columns
                    .iter()
                    .map(|c| {
                        range
                            .get_value((row, c.column_index - 1))
                            .unwrap_or(&Data::Empty)
                    })
                    .collect();
                if cells.iter().all(|c| c.is_empty()) {
                    continue;
                }
                row_count += 1;
                if rows.len() < 8 {
                    rows.push(
                        cells
                            .into_iter()
                            .take(12)
                            .map(|c| {
                                let text = cell_text(c);
                                Value::String(text.chars().take(240).collect())
                            })
                            .collect(),
                    );
                }
            }
            worksheets.push(WorksheetPreview {
                name,
                header_row: header,
                columns,
                rows,
                row_count,
                empty: range.is_empty(),
            });
        }
        Ok(WorkbookPreview {
            path: path.to_string_lossy().to_string(),
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            worksheets,
        })
    }
    pub fn persist_space(&self, config: &mut Config) -> Result<WorkspaceSummary> {
        config.managed = true;
        self.activate_config(config, &format!("app://spaces/{}", config.workspace.id))
    }
    pub fn create_space(&self, name: String) -> Result<WorkspaceSummary> {
        let mut c = Config::empty_space(fresh_id("space"), name.trim().to_string());
        self.persist_space(&mut c)
    }
    pub fn rename_space(&self, id: &str, name: String) -> Result<WorkspaceSummary> {
        let mut c = self.config(id)?;
        c.workspace.name = name.trim().to_string();
        self.persist_space(&mut c)
    }
    pub fn add_excel_workbook(
        &self,
        path: &Path,
        space_id: Option<&str>,
        name: Option<String>,
        hidden_sheets: &[String],
    ) -> Result<WorkspaceSummary> {
        let preview = self.inspect_workbook(path, None, None)?;
        let mut c = if let Some(id) = space_id {
            self.config(id)?
        } else {
            Config::empty_space(
                fresh_id("space"),
                name.clone().unwrap_or_else(|| preview.name.clone()),
            )
        };
        if space_id.is_none() {
            c.workspace.name = name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(preview.name);
        }
        if c.pages.is_empty() && !c.sources.is_empty() {
            c.pages = c.effective_pages();
        }
        for (position, sheet) in preview.worksheets.iter().enumerate() {
            let id = format!("sheet_{}_{}", c.sources.len() + 1, position + 1);
            let mut fields: BTreeMap<String, Field> = sheet
                .columns
                .iter()
                .map(|column| {
                    (
                        column.id.clone(),
                        Field {
                            column: if column.column.trim().is_empty() {
                                column.label.clone()
                            } else {
                                column.column.clone()
                            },
                            column_index: if column.column.trim().is_empty()
                                || sheet
                                    .columns
                                    .iter()
                                    .filter(|c| c.column == column.column)
                                    .count()
                                    > 1
                            {
                                Some(column.column_index)
                            } else {
                                None
                            },
                            kind: FieldType::String,
                            required: false,
                            trim: false,
                            searchable: true,
                            filterable: true,
                            label: Some(column.label.clone()),
                        },
                    )
                })
                .collect();
            fields.insert(
                "__row_id".into(),
                Field {
                    column: "__row_id".into(),
                    column_index: None,
                    kind: FieldType::String,
                    required: true,
                    trim: false,
                    searchable: false,
                    filterable: false,
                    label: Some("Row".into()),
                },
            );
            let source = Source {
                id: id.clone(),
                path: preview.path.clone(),
                sheet: sheet.name.clone(),
                entity: id.clone(),
                namespace: id.clone(),
                header_row: sheet.header_row,
                key: "__row_id".into(),
                key_mode: KeyMode::RowNumber,
                fields,
            };
            let columns: Vec<_> = if sheet.columns.is_empty() {
                vec!["__row_id".into()]
            } else {
                sheet.columns.iter().map(|c| c.id.clone()).collect()
            };
            let mut title = sheet.name.clone();
            if c.pages.iter().any(|p| p.title == title) {
                title = format!(
                    "{} · {}",
                    Path::new(&preview.path)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    sheet.name
                );
                let base = title.clone();
                let mut suffix = 2;
                while c.pages.iter().any(|p| p.title == title) {
                    title = format!("{base} ({suffix})");
                    suffix += 1;
                }
            }
            let page = PageDefinition {
                id: id.clone(),
                title,
                entity: id.clone(),
                source_id: Some(id),
                kind: PageKind::Worksheet,
                columns: columns.clone(),
                default_sort: Sort {
                    field: columns[0].clone(),
                    direction: Direction::Asc,
                },
                search_fields: columns
                    .iter()
                    .filter(|s| s.as_str() != "__row_id")
                    .cloned()
                    .collect(),
                filters: Vec::new(),
                fixed_filters: BTreeMap::new(),
                hidden: hidden_sheets.contains(&sheet.name),
            };
            c.sources.push(source);
            c.pages.push(page);
        }
        if c.views.main.entity == "empty" {
            c.reset_root_view();
        }
        self.persist_space(&mut c)
    }
    pub fn save_page(&self, workspace: &str, page: PageDefinition) -> Result<PageDefinition> {
        let mut c = self.config(workspace)?;
        if c.pages.is_empty() {
            c.pages = c.effective_pages();
        }
        let old = c
            .pages
            .iter()
            .find(|p| p.id == page.id)
            .context("Page not found")?;
        ensure!(
            old.source_id == page.source_id && old.entity == page.entity && old.kind == page.kind,
            "A page cannot change its source worksheet"
        );
        c.validate_page(&page)?;
        *c.pages
            .iter_mut()
            .find(|p| p.id == page.id)
            .expect("existing page") = page.clone();
        self.persist_space(&mut c)?;
        Ok(page)
    }
    pub fn create_view(
        &self,
        workspace: &str,
        source_id: &str,
        title: String,
        columns: Vec<String>,
        filters: BTreeMap<String, Value>,
        sort: Option<Sort>,
    ) -> Result<PageDefinition> {
        let mut c = self.config(workspace)?;
        if c.pages.is_empty() {
            c.pages = c.effective_pages();
        }
        let source = c
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .context("Worksheet not found")?;
        ensure!(!columns.is_empty(), "Choose at least one column");
        let page = PageDefinition {
            id: fresh_id("view"),
            title: title.trim().to_string(),
            entity: source.entity.clone(),
            source_id: Some(source.id.clone()),
            kind: PageKind::View,
            default_sort: sort.unwrap_or(Sort {
                field: columns[0].clone(),
                direction: Direction::Asc,
            }),
            search_fields: columns
                .iter()
                .filter(|f| {
                    source.fields.get(*f).is_some_and(|v| v.searchable)
                        || *f == &source.key
                        || c.views.main.search_fields.contains(*f)
                })
                .cloned()
                .collect(),
            columns,
            filters: filters.keys().cloned().collect(),
            fixed_filters: filters,
            hidden: false,
        };
        c.validate_page(&page)?;
        c.pages.push(page.clone());
        self.persist_space(&mut c)?;
        Ok(page)
    }
    pub fn update_worksheet(&self, workspace: &str, source: Source) -> Result<Config> {
        let mut c = self.config(workspace)?;
        let old = c
            .sources
            .iter()
            .find(|s| s.id == source.id)
            .context("Worksheet not found")?;
        ensure!(
            old.path == source.path
                && old.sheet == source.sheet
                && old.entity == source.entity
                && old.namespace == source.namespace,
            "Worksheet identity/path cannot change in the mapping editor"
        );
        ensure!(
            (1..=1_048_576).contains(&source.header_row),
            "Invalid header row"
        );
        *c.sources
            .iter_mut()
            .find(|s| s.id == source.id)
            .expect("existing source") = source.clone();
        for page in &mut c.pages {
            if page.entity == source.entity {
                page.columns.retain(|f| source.fields.contains_key(f));
                if page.columns.is_empty() {
                    page.columns.push(
                        source
                            .fields
                            .keys()
                            .find(|f| f.as_str() != "__row_id")
                            .or_else(|| source.fields.keys().next())
                            .context("Worksheet needs fields")?
                            .clone(),
                    );
                }
                page.search_fields.retain(|f| {
                    page.columns.contains(f)
                        && (source.fields.get(f).is_some_and(|v| v.searchable) || f == &source.key)
                });
                page.filters.retain(|f| page.columns.contains(f));
                page.fixed_filters.retain(|f, _| page.columns.contains(f));
                if !page.columns.contains(&page.default_sort.field) {
                    page.default_sort.field = page.columns[0].clone();
                }
            }
        }
        if !c.pages.is_empty() {
            c.reset_root_view();
        }
        self.persist_space(&mut c)?;
        Ok(c)
    }
}
