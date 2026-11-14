use explorer_core::{
    config::DocumentMapping,
    documents::{normalize_windows, resolve},
};
fn mapping() -> DocumentMapping {
    DocumentMapping {
        id: "pdf".into(),
        entity: "invoice".into(),
        path_field: "pdf".into(),
        root: "documents".into(),
        allowed_extensions: vec!["pdf".into()],
        allow_absolute: false,
    }
}
#[test]
fn safe_document_paths_and_missing_files() {
    let d = tempfile::tempdir().unwrap();
    let m = mapping();
    std::fs::write(d.path().join("yes.pdf"), b"PDF").unwrap();
    assert!(resolve(d.path(), "yes.pdf", &m, true).is_ok());
    assert!(resolve(d.path(), "missing.pdf", &m, true).is_err());
    for path in [
        "../outside.pdf",
        "folder/../../out.pdf",
        "evil.exe",
        "https://x/file.pdf",
        "file.pdf:evil.exe",
        "\\\\host\\share\\file.pdf",
    ] {
        assert!(resolve(d.path(), path, &m, false).is_err(), "{path}");
    }
    assert_eq!(
        normalize_windows("\\\\server\\share\\file.pdf"),
        "//server/share/file.pdf"
    );
}
#[cfg(unix)]
#[test]
fn symlink_escape_is_blocked() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("private.pdf"), b"x").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("private.pdf"),
        root.path().join("link.pdf"),
    )
    .unwrap();
    assert!(resolve(root.path(), "link.pdf", &mapping(), true).is_err());
}
