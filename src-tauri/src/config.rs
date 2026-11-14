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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    pub id: String,
    pub parent_entity: String,
    pub child_entity: String,
    pub parent_field: String,
    pub child_field: String,
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
        ensure!(!self.sources.is_empty(), "Workspace needs sources");
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
            unique(s.fields.keys().map(String::as_str), "field")?;
            for f in s.fields.values() {
                ensure!(
                    !f.column.is_empty(),
                    "Source {} has empty column mapping",
                    s.id
                );
            }
        }
        for r in &self.relationships {
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
