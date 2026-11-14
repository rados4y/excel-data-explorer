//! Explicit page-to-page joins. Source identities/namespaces are never rewritten.
use crate::{
    config::{RelatedView, Relationship},
    engine::{Engine, Page, QueryOptions},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestedViewInput {
    pub id: Option<String>,
    pub title: String,
    pub page_id: String,
    pub parent_field: String,
    pub child_field: String,
}
impl Engine {
    pub fn save_nested_views(
        &self,
        workspace: &str,
        page_id: &str,
        inputs: Vec<NestedViewInput>,
    ) -> Result<()> {
        let mut c = self.config(workspace)?;
        if c.pages.is_empty() {
            c.pages = c.effective_pages();
        }
        let parent = c
            .pages
            .iter()
            .find(|p| p.id == page_id)
            .context("Parent page not found")?
            .clone();
        let parent_source = parent
            .source_id
            .as_ref()
            .context("Create a worksheet-specific view before configuring related views")?;
        ensure!(inputs.len() <= 20, "Keep at most 20 related views per page");
        let mut nested = Vec::new();
        for input in inputs {
            let child = c
                .pages
                .iter()
                .find(|p| p.id == input.page_id)
                .context("Child page not found")?;
            let child_source = child
                .source_id
                .as_ref()
                .context("Choose a worksheet-specific child view")?;
            ensure!(
                parent_source != child_source,
                "Choose a different worksheet for the child view"
            );
            let id = if let Some(id) = input.id {
                ensure!(
                    parent.related_views.iter().any(|v| v.id == id),
                    "Unknown related view ID"
                );
                id
            } else {
                format!(
                    "nested_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos()
                )
            };
            ensure!(
                !nested.iter().any(|v: &RelatedView| v.id == id),
                "Duplicate related view ID"
            );
            let relationship = format!("join_{}", id);
            ensure!(relationship.len() <= 100, "Invalid related view ID");
            let relation = Relationship {
                id: relationship.clone(),
                parent_entity: parent.entity.clone(),
                child_entity: child.entity.clone(),
                parent_field: input.parent_field,
                child_field: input.child_field,
                parent_source_id: Some(parent_source.clone()),
                child_source_id: Some(child_source.clone()),
            };
            // Replace only definitions owned by this page's prior links.
            if let Some(existing) = c.relationships.iter().position(|r| r.id == relationship) {
                ensure!(
                    parent
                        .related_views
                        .iter()
                        .any(|v| v.relationship == relationship),
                    "Relationship ID already exists"
                );
                c.relationships[existing] = relation;
            } else {
                c.relationships.push(relation);
            }
            nested.push(RelatedView {
                id,
                title: input.title.trim().to_string(),
                relationship,
                page_id: child.id.clone(),
            });
        }
        c.pages
            .iter_mut()
            .find(|p| p.id == page_id)
            .expect("existing page")
            .related_views = nested;
        for old in &parent.related_views {
            if !c.pages.iter().any(|p| {
                p.related_views
                    .iter()
                    .any(|v| v.relationship == old.relationship)
            }) && !c
                .views
                .detail
                .tabs
                .iter()
                .any(|t| t.relationship.as_ref() == Some(&old.relationship))
            {
                c.relationships.retain(|r| r.id != old.relationship);
            }
        }
        // Validation, definitions and cached relationship edges commit together. No Excel reads.
        self.persist_space(&mut c)?;
        Ok(())
    }
    pub fn nested_records(
        &self,
        workspace: &str,
        parent_page_id: &str,
        view_id: &str,
        record_id: &str,
        mut options: QueryOptions,
    ) -> Result<Page> {
        let c = self.config(workspace)?;
        let parent = c
            .pages
            .iter()
            .find(|p| p.id == parent_page_id)
            .context("Parent page not found")?;
        let link = parent
            .related_views
            .iter()
            .find(|v| v.id == view_id)
            .context("Related view not found")?;
        let record = self.record(record_id)?;
        ensure!(
            record.workspace_id == workspace
                && record.entity_type == parent.entity
                && Some(&record.source_id) == parent.source_id.as_ref(),
            "Related view parent scope mismatch"
        );
        options.page_id = Some(link.page_id.clone());
        self.related(record_id, &link.relationship, &options)
    }
}
