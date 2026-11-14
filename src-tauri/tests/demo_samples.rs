use explorer_core::{
    config::Config,
    db,
    engine::{Engine, QueryOptions},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const SAMPLES: [&str; 5] = [
    "01-offers-and-finance",
    "02-alternate-layout",
    "03-namespaces",
    "04-documents-and-orphans",
    "05-sync-recovery",
];
fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let dest = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
}
struct Sample {
    _directory: tempfile::TempDir,
    folder: PathBuf,
    engine: Engine,
    config: Config,
    expected: Value,
}
impl Sample {
    fn new(name: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("moved-sample");
        copy_tree(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("demo")
                .join(name),
            &folder,
        );
        let yaml = folder.join("workspace.yaml");
        let mut config = Config::load(&yaml).unwrap();
        config.settings.stable_file_check_ms = 1;
        std::fs::write(&yaml, serde_yaml::to_string(&config).unwrap()).unwrap();
        let expected =
            serde_json::from_slice(&std::fs::read(folder.join("expected-results.json")).unwrap())
                .unwrap();
        let engine = Engine::open(directory.path().join("cache.sqlite")).unwrap();
        engine.add_config(&yaml).unwrap();
        Self {
            _directory: directory,
            folder,
            engine,
            config,
            expected,
        }
    }
    fn import(&self) {
        for path in self.config.workbook_groups().keys() {
            self.engine
                .sync_workbook(&self.config.workspace.id, path, false, "sample-test")
                .unwrap();
        }
        self.engine
            .refresh_documents(Some(&self.config.workspace.id))
            .unwrap();
    }
    fn count(&self, entity: &str) -> i64 {
        self.engine
            .query(&self.config.workspace.id, entity, &Default::default(), None)
            .unwrap()
            .total
    }
}
#[test]
fn portable_samples_match_their_manifests_and_never_modify_inputs() {
    for name in SAMPLES {
        let sample = Sample::new(name);
        let original: Vec<_> = sample
            .config
            .workbook_groups()
            .keys()
            .map(|p| (p.clone(), std::fs::read(p).unwrap()))
            .collect();
        sample.import();
        assert_eq!(sample.expected["workspace_id"], sample.config.workspace.id);
        let mut documents = BTreeMap::new();
        for (entity, expected) in sample.expected["record_counts"].as_object().unwrap() {
            assert_eq!(
                sample.count(entity),
                expected.as_i64().unwrap(),
                "{name}: {entity}"
            );
            for r in sample
                .engine
                .query(
                    &sample.config.workspace.id,
                    entity,
                    &QueryOptions {
                        limit: Some(200),
                        ..Default::default()
                    },
                    None,
                )
                .unwrap()
                .records
            {
                for document in sample.engine.documents(&r.id, false).unwrap() {
                    documents.insert(document.id, document.availability_status);
                }
            }
        }
        let mut statuses: BTreeMap<String, u64> = BTreeMap::new();
        for status in documents.values() {
            *statuses.entry(status.clone()).or_default() += 1;
        }
        assert_eq!(
            serde_json::to_value(statuses).unwrap(),
            sample.expected["document_status_counts"],
            "{name}: document status"
        );
        if let Some(relations) = sample.expected["relationships"].as_array() {
            for rel in relations {
                let parent = db::identity(
                    &sample.config.workspace.id,
                    "offer",
                    rel["namespace"].as_str().unwrap(),
                    rel["parent"].as_str().unwrap(),
                );
                let mut children: Vec<_> = sample
                    .engine
                    .related(
                        &parent,
                        rel["relation"].as_str().unwrap(),
                        &Default::default(),
                    )
                    .unwrap()
                    .records
                    .into_iter()
                    .map(|r| r.business_key)
                    .collect();
                children.sort();
                let mut expected: Vec<_> = rel["children"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                expected.sort();
                assert_eq!(children, expected, "{name}: relation");
            }
        }
        let status = sample
            .engine
            .sync_status(Some(&sample.config.workspace.id))
            .unwrap();
        let warnings: i64 = status["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["warnings"].as_i64().unwrap())
            .sum();
        assert_eq!(
            warnings,
            sample.expected["orphan_count"].as_i64().unwrap(),
            "{name}: orphan warnings"
        );
        for (path, bytes) in original {
            assert_eq!(
                std::fs::read(path).unwrap(),
                bytes,
                "{name}: source workbook changed"
            );
        }
    }
}
#[test]
fn alternate_sample_preserves_business_keys_and_exact_values() {
    let s = Sample::new("02-alternate-layout");
    s.import();
    let page = s
        .engine
        .query(&s.config.workspace.id, "order", &Default::default(), None)
        .unwrap();
    let keys: Vec<_> = page
        .records
        .iter()
        .map(|r| r.business_key.as_str())
        .collect();
    assert_eq!(serde_json::to_value(keys).unwrap(), s.expected["keys"]);
    for (key, value) in s.expected["first_record"].as_object().unwrap() {
        assert_eq!(&page.records[0].fields[key], value);
    }
}
#[test]
fn repeated_ids_in_the_namespace_sample_have_distinct_internal_identities() {
    let s = Sample::new("03-namespaces");
    s.import();
    let rows = s
        .engine
        .query(&s.config.workspace.id, "offer", &Default::default(), None)
        .unwrap()
        .records;
    let ids: BTreeSet<_> = rows.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids.len(), 4);
    assert_eq!(
        rows.iter().filter(|r| r.business_key == "OFF-001").count(),
        2
    );
}
#[test]
fn document_sample_rejects_unavailable_or_unsafe_open_actions() {
    let s = Sample::new("04-documents-and-orphans");
    s.import();
    let parent = db::identity(&s.config.workspace.id, "offer", "primary", "OFF-001");
    for document in s.engine.documents(&parent, true).unwrap() {
        assert_eq!(
            s.engine.validated_document(&document.id).is_ok(),
            document.availability_status == "available"
        );
    }
    let orphan = db::identity(&s.config.workspace.id, "invoice", "primary", "INV-ORPHAN");
    assert_eq!(
        s.engine.record(&orphan).unwrap().fields["offer_id"],
        "OFF-404"
    );
}
#[test]
fn sample_recovery_sequence_keeps_the_last_successful_workbook_generation() {
    let s = Sample::new("05-sync-recovery");
    s.import();
    let source = &s.config.sources[0].path;
    assert_eq!(s.count("offer"), 2);
    std::fs::copy(s.folder.join("scenarios/updated.xlsx"), source).unwrap();
    s.engine
        .sync_workbook(&s.config.workspace.id, source, true, "sample-updated")
        .unwrap();
    assert_eq!(s.count("offer"), 3);
    let before = s.engine.sync_status(Some(&s.config.workspace.id)).unwrap()["sources"][0].clone();
    std::fs::copy(s.folder.join("scenarios/invalid.xlsx"), source).unwrap();
    assert!(s
        .engine
        .sync_workbook(&s.config.workspace.id, source, true, "sample-invalid")
        .is_err());
    assert_eq!(s.count("offer"), 3);
    let failed = s.engine.sync_status(Some(&s.config.workspace.id)).unwrap()["sources"][0].clone();
    assert_eq!(failed["generation"], before["generation"]);
    assert_eq!(failed["last_success_at"], before["last_success_at"]);
    assert_eq!(failed["status"], "error");
    let offline = s.folder.join("data/offers.xlsx.offline");
    std::fs::rename(source, &offline).unwrap();
    assert!(s
        .engine
        .sync_workbook(&s.config.workspace.id, source, true, "sample-offline")
        .is_err());
    assert_eq!(s.count("offer"), 3);
    std::fs::rename(offline, source).unwrap();
    std::fs::copy(s.folder.join("scenarios/baseline.xlsx"), source).unwrap();
    s.engine
        .sync_workbook(&s.config.workspace.id, source, true, "sample-recovery")
        .unwrap();
    assert_eq!(s.count("offer"), 2);
}
