use crate::config::DocumentMapping;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub name: String,
    pub original_ref: String,
    pub resolved_path: String,
    pub document_type: String,
    pub availability_status: String,
    pub status_detail: Option<String>,
    pub size: Option<i64>,
    pub mapping_id: String,
}
/// Normalize both Windows separators, including UNC prefixes. Never interpret shell expressions.
pub fn normalize_windows(path: &str) -> String {
    path.replace('\\', "/")
}
pub fn resolve(
    root: &Path,
    reference: &str,
    mapping: &DocumentMapping,
    verify: bool,
) -> Result<PathBuf> {
    ensure!(!reference.contains('\0'), "Invalid document path");
    let normalized = normalize_windows(reference);
    ensure!(
        !normalized.contains(":")
            || (mapping.allow_absolute
                && normalized.as_bytes().get(1) == Some(&b':')
                && !normalized[2..].contains(':')),
        "Document URL/alternate stream is not permitted"
    );
    let p = Path::new(&normalized);
    let windows_absolute =
        normalized.starts_with("//") || normalized.as_bytes().get(1) == Some(&b':');
    ensure!(
        mapping.allow_absolute || (!p.is_absolute() && !windows_absolute),
        "Absolute document path is not enabled"
    );
    ensure!(
        !p.components().any(|c| matches!(c, Component::ParentDir)),
        "Document traversal is not permitted"
    );
    let extension = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    ensure!(
        mapping
            .allowed_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(&extension)),
        "Unsupported document extension"
    );
    ensure!(
        !["exe", "com", "bat", "cmd", "ps1", "js", "vbs", "lnk", "url", "msi", "scr"]
            .contains(&extension.as_str()),
        "Executable document type is forbidden"
    );
    let joined = if p.is_absolute() || windows_absolute {
        p.to_path_buf()
    } else {
        root.join(p)
    };
    let root = crate::config::lexical_path(root);
    let joined = crate::config::lexical_path(&joined);
    ensure!(
        joined.starts_with(&root),
        "Document is outside configured root"
    );
    if verify {
        let canonical_root =
            std::fs::canonicalize(&root).context("Document root is unavailable")?;
        let canonical =
            std::fs::canonicalize(&joined).context("Document file is missing or unavailable")?;
        ensure!(
            canonical.starts_with(&canonical_root),
            "Document symlink/junction escapes configured root"
        );
        ensure!(canonical.is_file(), "Document is not a regular file");
        // Also validate target extension: a benign-looking link cannot launch an executable target.
        let ext = canonical
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        ensure!(
            mapping
                .allowed_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&ext)),
            "Resolved document extension is not permitted"
        );
        return Ok(canonical);
    }
    Ok(joined)
}
