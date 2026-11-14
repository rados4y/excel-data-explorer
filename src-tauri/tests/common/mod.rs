#![allow(dead_code)]
use explorer_core::{config::Config, engine::Engine};
use std::path::{Path, PathBuf};
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub engine: Engine,
    pub config: Config,
    pub yaml: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        for file in ["offers.xlsx", "finance.xlsx", "alternate.xlsx"] {
            std::fs::copy(
                root().join("fixtures/demo/data").join(file),
                data.join(file),
            )
            .unwrap();
        }
        let mut config = Config::load(&root().join("workspaces/demo-workspace.yaml")).unwrap();
        config
            .roots
            .insert("data".into(), data.to_string_lossy().to_string());
        config.settings.stable_file_check_ms = 10;
        for s in &mut config.sources {
            s.path = data
                .join(Path::new(&s.path).file_name().unwrap())
                .to_string_lossy()
                .into_owned();
        }
        let yaml = dir.path().join("workspace.yaml");
        std::fs::write(&yaml, serde_yaml::to_string(&config).unwrap()).unwrap();
        let engine = Engine::open(dir.path().join("cache.sqlite")).unwrap();
        engine.add_config(&yaml).unwrap();
        Self {
            dir,
            engine,
            config,
            yaml,
        }
    }
    pub fn import(&self) {
        for path in self.config.workbook_groups().keys() {
            self.engine.import_file("demo", path).unwrap();
        }
    }
    pub fn offers(&self, rows: &[(&str, &str)]) {
        let mut w = rust_xlsxwriter::Workbook::new();
        let s = w.add_worksheet();
        s.set_name("Offers").unwrap();
        for (col, v) in [
            "OfferID",
            "Customer",
            "Description",
            "Amount",
            "Status",
            "CreatedAt",
        ]
        .iter()
        .enumerate()
        {
            s.write_string(0, col as u16, *v).unwrap();
        }
        for (i, (id, customer)) in rows.iter().enumerate() {
            let r = i as u32 + 1;
            s.write_string(r, 0, *id).unwrap();
            s.write_string(r, 1, *customer).unwrap();
            s.write_string(r, 2, "Fixture").unwrap();
            s.write_string(r, 3, "10.25").unwrap();
            s.write_string(r, 4, "Active").unwrap();
            s.write_string(r, 5, "2026-03-01").unwrap();
        }
        w.save(self.config.sources[0].path.clone()).unwrap();
    }
}
