use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub workspace: Workspace,
    pub roots: BTreeMap<String, String>,
    #[serde(default)]
    pub settings: ImportSettings,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub documents: Vec<DocumentMapping>,
    pub views: Views,
    #[serde(default)]
    pub managed: bool,
    #[serde(default)]
    pub pages: Vec<PageDefinition>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportSettings {
    pub timezone: String,
    pub local_scan_seconds: u64,
    pub network_scan_seconds: u64,
    pub stable_file_check_ms: u64,
    pub stable_file_attempts: u32,
    pub read_only: bool,
}
impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            timezone: "UTC".into(),
            local_scan_seconds: 60,
            network_scan_seconds: 300,
            stable_file_check_ms: 1500,
            stable_file_attempts: 3,
            read_only: true,
        }
    }
}
fn one() -> u32 {
    1
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub path: String,
    pub sheet: String,
    pub entity: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default = "one")]
    pub header_row: u32,
    pub key: String,
    #[serde(default)]
    pub key_mode: KeyMode,
    pub fields: BTreeMap<String, Field>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Integer,
    Decimal,
    Boolean,
    Date,
    Datetime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub column: String,
    #[serde(default)]
    pub column_index: Option<u32>,
    #[serde(rename = "type")]
    pub kind: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub trim: bool,
    #[serde(default)]
    pub searchable: bool,
    #[serde(default)]
    pub filterable: bool,
    #[serde(default)]
    pub label: Option<String>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KeyMode {
    #[default]
    Field,
    RowNumber,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    #[default]
    Worksheet,
    View,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageDefinition {
    pub id: String,
    pub title: String,
    pub entity: String,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub kind: PageKind,
    pub columns: Vec<String>,
    pub default_sort: Sort,
    #[serde(default)]
    pub search_fields: Vec<String>,
    #[serde(default)]
    pub filters: Vec<String>,
    #[serde(default)]
    pub fixed_filters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub related_views: Vec<RelatedView>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelatedView {
    pub id: String,
    pub title: String,
    pub relationship: String,
    pub page_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub id: String,
    pub parent_entity: String,
    pub child_entity: String,
    pub parent_field: String,
    pub child_field: String,
    #[serde(default)]
    pub parent_source_id: Option<String>,
    #[serde(default)]
    pub child_source_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentMapping {
    pub id: String,
    pub entity: String,
    pub path_field: String,
    pub root: String,
    pub allowed_extensions: Vec<String>,
    #[serde(default)]
    pub allow_absolute: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Views {
    pub main: MainView,
    pub detail: DetailView,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MainView {
    pub entity: String,
    pub title: String,
    pub columns: Vec<String>,
    pub default_sort: Sort,
    #[serde(default)]
    pub search_fields: Vec<String>,
    #[serde(default)]
    pub filters: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub field: String,
    pub direction: Direction,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Asc,
    Desc,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailView {
    pub entity: String,
    pub title_field: String,
    pub tabs: Vec<Tab>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    pub title: String,
    #[serde(rename = "type")]
    pub kind: TabType,
    #[serde(default)]
    pub relationship: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TabType {
    Fields,
    Relationship,
    RelatedDocuments,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
fn unique<'a>(ids: impl Iterator<Item = &'a str>, what: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        ensure!(valid_id(id), "Invalid {what} ID: {id}");
        ensure!(seen.insert(id), "Duplicate {what} ID: {id}");
    }
    Ok(())
}
pub fn lexical_path(path: &Path) -> PathBuf {
    // Canonical Windows paths use the verbatim prefix, where '/' is NOT a
    // separator. YAML commonly appends /file.xlsx to a canonical root.
    #[cfg(windows)]
    let normalized = {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        PathBuf::from(std::ffi::OsString::from_wide(
            &path
                .as_os_str()
                .encode_wide()
                .map(|c| if c == b'/' as u16 { b'\\' as u16 } else { c })
                .collect::<Vec<_>>(),
        ))
    };
    #[cfg(windows)]
    let path = normalized.as_path();
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            _ => out.push(c.as_os_str()),
        }
    }
    out
}
pub fn is_network(path: &str) -> bool {
    if let Some(rest) = path
        .strip_prefix("\\\\?\\")
        .or_else(|| path.strip_prefix("//?/"))
    {
        return rest
            .get(..4)
            .is_some_and(|p| p.eq_ignore_ascii_case("UNC\\") || p.eq_ignore_ascii_case("UNC/"));
    }
    path.starts_with("//") || path.starts_with("\\\\")
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let path = std::fs::canonicalize(path).context("Cannot read workspace configuration")?;
        let text = std::fs::read_to_string(&path)?;
        Self::parse(&text, path.parent().unwrap_or(Path::new(".")))
    }
    pub fn parse(text: &str, base: &Path) -> Result<Self> {
        let mut c: Self = serde_yaml::from_str(text).context("Invalid workspace YAML")?;
        for value in c.roots.values_mut() {
            ensure!(!value.contains("${"), "Roots cannot contain placeholders");
            let p = Path::new(value);
            if !p.is_absolute() && !is_network(value) {
                *value = lexical_path(&base.join(p)).to_string_lossy().into_owned();
            } else {
                *value = lexical_path(p).to_string_lossy().into_owned();
            }
        }
        for s in &mut c.sources {
            for (k, v) in &c.roots {
                s.path = s.path.replace(&format!("${{{k}}}"), v);
            }
            ensure!(
                !s.path.contains("${"),
                "Source {} has unknown placeholder",
                s.id
            );
            let p = Path::new(&s.path);
            if !p.is_absolute() && !is_network(&s.path) {
                s.path = lexical_path(&base.join(p)).to_string_lossy().into_owned();
            } else {
                s.path = lexical_path(p).to_string_lossy().into_owned();
            }
            if s.namespace.is_empty() {
                s.namespace = s.id.clone();
            }
        }
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1,
            "Unsupported configuration version {}",
            self.version
        );
        ensure!(valid_id(&self.workspace.id), "Invalid workspace ID");

        ensure!(self.settings.read_only, "Only read_only: true is supported");
        ensure!(
            self.settings.local_scan_seconds >= 5 && self.settings.network_scan_seconds >= 5,
            "Scan intervals must be at least 5 seconds"
        );
        ensure!(
            (1..=10).contains(&self.settings.stable_file_attempts),
            "stable_file_attempts must be 1..10"
        );
        ensure!(
            (1..=60000).contains(&self.settings.stable_file_check_ms),
            "stable_file_check_ms must be 1..60000"
        );
        self.settings
            .timezone
            .parse::<chrono_tz::Tz>()
            .context("Unknown IANA timezone")?;
        unique(self.sources.iter().map(|s| s.id.as_str()), "source")?;
        unique(
            self.relationships.iter().map(|s| s.id.as_str()),
            "relationship",
        )?;
        unique(
            self.documents.iter().map(|s| s.id.as_str()),
            "document mapping",
        )?;
        ensure!(
            !self.workspace.name.trim().is_empty() && self.workspace.name.len() <= 200,
            "Space name must be 1–200 characters"
        );
        if self.sources.is_empty() {
            ensure!(
                self.managed && self.pages.is_empty(),
                "Workspace needs sources"
            );
            return Ok(());
        }
        for s in &self.sources {
            ensure!(
                valid_id(&s.entity) && valid_id(&s.namespace),
                "Invalid entity or namespace in source {}",
                s.id
            );
            ensure!(
                s.header_row > 0,
                "Source {} header_row must be 1-based",
                s.id
            );
            ensure!(!s.sheet.is_empty(), "Source {} needs a sheet", s.id);
            ensure!(
                s.fields.contains_key(&s.key),
                "Source {} key field is missing",
                s.id
            );
            let key = &s.fields[&s.key];
            ensure!(
                key.kind == FieldType::String && key.required,
                "Source {} key must be a required string",
                s.id
            );
            if s.key_mode == KeyMode::RowNumber {
                ensure!(
                    s.key == "__row_id",
                    "Generated row identity must use __row_id"
                );
            }
            unique(s.fields.keys().map(String::as_str), "field")?;
            for f in s.fields.values() {
                ensure!(
                    !f.column.is_empty(),
                    "Source {} has empty column mapping",
                    s.id
                );
                if let Some(index) = f.column_index {
                    ensure!(
                        (1..=16384).contains(&index),
                        "Column index must be 1..16384"
                    );
                }
            }
        }
        unique(self.pages.iter().map(|p| p.id.as_str()), "page")?;
        for page in &self.pages {
            self.validate_page(page)?;
        }
        for r in &self.relationships {
            ensure!(
                r.parent_source_id.is_some() == r.child_source_id.is_some(),
                "Relationship must scope both worksheets or neither"
            );
            for (source_id, entity, field) in [
                (&r.parent_source_id, &r.parent_entity, &r.parent_field),
                (&r.child_source_id, &r.child_entity, &r.child_field),
            ] {
                if let Some(id) = source_id {
                    ensure!(
                        self.sources.iter().any(|s| &s.id == id
                            && &s.entity == entity
                            && s.fields.contains_key(field)),
                        "Relationship worksheet/field scope mismatch"
                    );
                }
            }
            self.field(&r.parent_entity, &r.parent_field)?;
            self.field(&r.child_entity, &r.child_field)?;
            ensure!(
                self.field(&r.parent_entity, &r.parent_field)?.kind
                    == self.field(&r.child_entity, &r.child_field)?.kind,
                "Relation {} has incompatible field types",
                r.id
            );
        }
        for d in &self.documents {
            ensure!(
                self.field(&d.entity, &d.path_field)?.kind == FieldType::String,
                "Document path field must be string"
            );
            ensure!(
                self.roots.contains_key(&d.root),
                "Document {} references unknown root {}",
                d.id,
                d.root
            );
            ensure!(
                !d.allowed_extensions.is_empty(),
                "Document {} needs allowed extensions",
                d.id
            );
            for e in &d.allowed_extensions {
                ensure!(
                    ["pdf", "docx", "xlsx", "txt", "png", "jpg", "jpeg"]
                        .contains(&e.to_lowercase().as_str()),
                    "Unsafe/unsupported extension: {e}"
                );
            }
        }
        let v = &self.views.main;
        ensure!(!v.columns.is_empty(), "Main view needs columns");
        for f in v
            .columns
            .iter()
            .chain(&v.search_fields)
            .chain(&v.filters)
            .chain(std::iter::once(&v.default_sort.field))
        {
            self.field(&v.entity, f)?;
        }
        ensure!(
            self.views.detail.entity == v.entity,
            "Detail entity must match main entity"
        );
        self.field(&v.entity, &self.views.detail.title_field)?;
        for t in &self.views.detail.tabs {
            if t.kind == TabType::Relationship {
                let r = self
                    .relationships
                    .iter()
                    .find(|r| Some(&r.id) == t.relationship.as_ref())
                    .context("Detail tab references missing relationship")?;
                ensure!(
                    r.parent_entity == v.entity,
                    "Detail relationship parent must match entity"
                );
            }
        }
        Ok(())
    }
    pub fn effective_pages(&self) -> Vec<PageDefinition> {
        if !self.pages.is_empty() {
            return self.pages.clone();
        }
        if self.sources.is_empty() {
            return Vec::new();
        }
        let v = &self.views.main;
        vec![PageDefinition {
            id: "main".into(),
            title: v.title.clone(),
            entity: v.entity.clone(),
            source_id: None,
            kind: PageKind::Worksheet,
            columns: v.columns.clone(),
            default_sort: v.default_sort.clone(),
            search_fields: v.search_fields.clone(),
            filters: v.filters.clone(),
            fixed_filters: BTreeMap::new(),
            hidden: false,
            related_views: Vec::new(),
        }]
    }
    pub fn validate_page(&self, page: &PageDefinition) -> Result<()> {
        ensure!(
            valid_id(&page.id) && !page.title.trim().is_empty() && page.title.len() <= 200,
            "Invalid page name or ID"
        );
        ensure!(
            !page.columns.is_empty(),
            "Keep at least one visible column on a page"
        );
        unique(page.columns.iter().map(String::as_str), "page column")?;
        unique(
            page.related_views.iter().map(|v| v.id.as_str()),
            "related view",
        )?;
        for nested in &page.related_views {
            ensure!(
                valid_id(&nested.id)
                    && !nested.title.trim().is_empty()
                    && nested.title.len() <= 200,
                "Invalid related view title or ID"
            );
            let relation = self
                .relationships
                .iter()
                .find(|r| r.id == nested.relationship)
                .context("Related view references an unknown relationship")?;
            let child = self
                .pages
                .iter()
                .find(|p| p.id == nested.page_id)
                .context("Related view references an unknown child page")?;
            ensure!(
                relation.parent_entity == page.entity && relation.child_entity == child.entity,
                "Related view entity scope mismatch"
            );
            ensure!(
                relation.parent_source_id.is_some()
                    && relation.parent_source_id == page.source_id
                    && relation.child_source_id == child.source_id,
                "Related view worksheet scope mismatch"
            );
        }
        if let Some(source) = &page.source_id {
            let s = self
                .sources
                .iter()
                .find(|s| &s.id == source)
                .context("Page references an unknown worksheet")?;
            ensure!(s.entity == page.entity, "Page worksheet/entity mismatch");
        }
        for field in page
            .columns
            .iter()
            .chain(&page.search_fields)
            .chain(&page.filters)
            .chain(page.fixed_filters.keys())
            .chain(std::iter::once(&page.default_sort.field))
        {
            self.field(&page.entity, field)?;
        }
        ensure!(
            page.columns.contains(&page.default_sort.field),
            "Sort column must be visible"
        );
        ensure!(
            page.search_fields.iter().all(|f| page.columns.contains(f)),
            "Search fields must be visible"
        );
        ensure!(
            page.filters.iter().all(|f| page.columns.contains(f)),
            "Filter controls must use visible columns"
        );
        for (field, value) in &page.fixed_filters {
            ensure!(
                !value.is_null() && !value.is_array() && !value.is_object(),
                "View filters need a text, number or boolean value"
            );
            ensure!(
                page.columns.contains(field),
                "Saved filter column must be visible"
            );
        }
        Ok(())
    }
    pub fn for_page(&self, id: &str) -> Result<Self> {
        let page = self
            .effective_pages()
            .into_iter()
            .find(|p| p.id == id)
            .context("Page not found")?;
        ensure!(
            !page.hidden,
            "This page is hidden; show it in Pages and views first"
        );
        let mut config = self.clone();
        config.views.main = MainView {
            entity: page.entity.clone(),
            title: page.title,
            columns: page.columns.clone(),
            default_sort: page.default_sort,
            search_fields: page.search_fields,
            filters: page.filters,
        };
        if self.views.detail.entity != page.entity || id != "main" {
            let mut tabs = vec![Tab {
                title: "Overview".into(),
                kind: TabType::Fields,
                relationship: None,
            }];
            for r in self.relationships.iter().filter(|r| {
                r.parent_entity == page.entity
                    && !self
                        .pages
                        .iter()
                        .any(|p| p.related_views.iter().any(|v| v.relationship == r.id))
            }) {
                tabs.push(Tab {
                    title: r.child_entity.clone(),
                    kind: TabType::Relationship,
                    relationship: Some(r.id.clone()),
                });
            }
            tabs.push(Tab {
                title: "Documents".into(),
                kind: TabType::RelatedDocuments,
                relationship: None,
            });
            let source = self
                .sources
                .iter()
                .find(|s| s.entity == page.entity)
                .context("Page entity not found")?;
            config.views.detail = DetailView {
                entity: page.entity,
                title_field: if page.columns.contains(&source.key) {
                    source.key.clone()
                } else {
                    page.columns[0].clone()
                },
                tabs,
            };
        }
        if !page.columns.contains(&config.views.detail.title_field) {
            config.views.detail.title_field = page.columns[0].clone();
        }
        Ok(config)
    }
    /// UI-only page/column changes do not invalidate successfully imported source data.
    pub fn import_hash(&self) -> String {
        let mut sources = serde_json::to_value(&self.sources).expect("serializable sources");
        for source in sources.as_array_mut().into_iter().flatten() {
            if let Some(fields) = source
                .get_mut("fields")
                .and_then(serde_json::Value::as_object_mut)
            {
                for field in fields.values_mut() {
                    if let Some(f) = field.as_object_mut() {
                        f.remove("label");
                        f.remove("filterable");
                    }
                }
            }
        }
        crate::hash(&serde_json::json!({"sources":sources,"relationships":self.relationships,"documents":self.documents,"roots":self.roots,"timezone":self.settings.timezone,"legacy_search":self.views.main.search_fields}).to_string())
    }
    pub fn field(&self, entity: &str, field: &str) -> Result<&Field> {
        let mut found = None;
        for s in self.sources.iter().filter(|s| s.entity == entity) {
            let f = s
                .fields
                .get(field)
                .with_context(|| format!("Unknown field {entity}.{field} in source {}", s.id))?;
            if let Some(prev) = found {
                let prev: &Field = prev;
                ensure!(
                    prev.kind == f.kind,
                    "Inconsistent types for {entity}.{field}"
                );
            }
            found = Some(f);
        }
        found.with_context(|| format!("Unknown entity {entity}"))
    }
    pub fn entity_fields(&self, entity: &str) -> Result<BTreeMap<String, Field>> {
        let mut fields = BTreeMap::new();
        for s in self.sources.iter().filter(|s| s.entity == entity) {
            fields.extend(s.fields.clone());
        }
        if fields.is_empty() {
            bail!("Unknown entity {entity}");
        }
        Ok(fields)
    }
    pub fn hash(&self) -> String {
        crate::hash(&serde_json::to_string(self).expect("serializable config"))
    }
    pub fn workbook_groups(&self) -> BTreeMap<String, Vec<&Source>> {
        let mut groups = BTreeMap::new();
        for s in &self.sources {
            groups
                .entry(s.path.clone())
                .or_insert_with(Vec::new)
                .push(s);
        }
        groups
    }
}
