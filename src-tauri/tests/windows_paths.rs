use explorer_core::config::is_network;

#[test]
fn network_detection_distinguishes_verbatim_local_drives_from_unc_shares() {
    for local in [
        r"C:\data\book.xlsx",
        r"\\?\C:\data\book.xlsx",
        "//?/C:/data/book.xlsx",
        "/tmp/book.xlsx",
    ] {
        assert!(!is_network(local), "{local}");
    }
    for network in [
        r"\\server\share\book.xlsx",
        "//server/share/book.xlsx",
        r"\\?\UNC\server\share\book.xlsx",
        "//?/UNC/server/share/book.xlsx",
    ] {
        assert!(is_network(network), "{network}");
    }
}

#[cfg(windows)]
#[test]
fn yaml_root_expansion_produces_readable_canonical_windows_workbook_paths() {
    use explorer_core::{config::Config, engine::Engine};
    use std::path::Path;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for name in ["offers.xlsx", "finance.xlsx"] {
        std::fs::copy(root.join("fixtures/demo/data").join(name), data.join(name)).unwrap();
    }
    let text = include_str!("../../workspaces/demo-workspace.yaml")
        .replace("../fixtures/demo/data", "./data")
        .replace("../fixtures/demo/documents", "./documents")
        .replace("stable_file_check_ms: 1500", "stable_file_check_ms: 1");
    let yaml = dir.path().join("workspace.yaml");
    std::fs::write(&yaml, text).unwrap();
    let config = Config::load(&yaml).unwrap();
    for source in &config.sources {
        assert!(Path::new(&source.path).is_file(), "{}", source.path);
        assert!(!is_network(&source.path));
    }
    let engine = Engine::open(dir.path().join("cache.sqlite")).unwrap();
    engine.add_config(&yaml).unwrap();
    for path in config.workbook_groups().keys() {
        engine.import_file("demo", path).unwrap();
    }
}
