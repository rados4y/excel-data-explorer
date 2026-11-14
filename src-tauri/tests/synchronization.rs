mod common;
use common::Fixture;
use explorer_core::{
    engine::Engine,
    sync::{stable_snapshot, Scheduler},
};
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
#[test]
fn first_unchanged_changed_and_source_isolation() {
    let f = Fixture::new();
    for path in f.config.workbook_groups().keys() {
        assert!(f.engine.sync_workbook("demo", path, false, "test").unwrap());
    }
    let before = f.engine.sync_status(Some("demo")).unwrap();
    let n = f.engine.history(Some("demo"), None, 0).unwrap().len();
    for path in f.config.workbook_groups().keys() {
        assert!(!f.engine.sync_workbook("demo", path, false, "test").unwrap());
    }
    assert_eq!(f.engine.history(Some("demo"), None, 0).unwrap().len(), n);
    f.offers(&[("OFF-001", "Updated"), ("0007", "Leading zero")]);
    assert!(f
        .engine
        .sync_workbook("demo", &f.config.sources[0].path, false, "test")
        .unwrap());
    let after = f.engine.sync_status(Some("demo")).unwrap();
    for old in before["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["source_id"] != "offers")
    {
        let new = after["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["source_id"] == old["source_id"])
            .unwrap();
        assert_eq!(old["generation"], new["generation"]);
    }
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        2
    );
}
#[test]
fn missing_and_validation_failure_keep_stale_generation() {
    let f = Fixture::new();
    let path = &f.config.sources[0].path;
    f.engine.sync_workbook("demo", path, false, "test").unwrap();
    let before = f.engine.sync_status(Some("demo")).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(f.engine.sync_workbook("demo", path, false, "test").is_err());
    let stale = f.engine.sync_status(Some("demo")).unwrap();
    let source = |s: serde_json::Value| {
        s["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["source_id"] == "offers")
            .unwrap()
            .clone()
    };
    assert_eq!(
        source(before.clone())["last_success_at"],
        source(stale.clone())["last_success_at"]
    );
    assert_eq!(source(stale)["status"], "unavailable");
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
    f.offers(&[("", "Bad required key")]);
    assert!(f.engine.sync_workbook("demo", path, false, "test").is_err());
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
}
#[test]
fn snapshot_rejects_changing_source_and_respects_cancellation() {
    let f = Fixture::new();
    let mut c = f.config.clone();
    c.settings.stable_file_check_ms = 100;
    c.settings.stable_file_attempts = 1;
    let path = c.sources[0].path.clone();
    let path2 = path.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(path2, b"changing file").unwrap();
    });
    assert!(stable_snapshot(std::path::Path::new(&path), &c, &f.engine.cancelled).is_err());
    writer.join().unwrap();
    f.engine.cancelled.store(true, Ordering::Relaxed);
    assert!(stable_snapshot(std::path::Path::new(&path), &c, &f.engine.cancelled).is_err());
}
#[test]
fn cancelled_activation_and_crash_recovery_preserve_cache() {
    let f = Fixture::new();
    f.import();
    f.offers(&[("NEW", "Changed")]);
    f.engine.cancelled.store(true, Ordering::Relaxed);
    assert!(f
        .engine
        .sync_workbook("demo", &f.config.sources[0].path, true, "test")
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
    f.engine.connection().unwrap().execute("INSERT INTO import_runs(workspace_id,workbook,status,trigger) VALUES('demo','x','staging','test')",[]).unwrap();
    let e = Engine::open(&f.engine.db_path).unwrap();
    assert!(e
        .history(Some("demo"), None, 0)
        .unwrap()
        .iter()
        .any(|r| r["status"] == "cancelled"));
    assert_eq!(
        e.query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
}
#[test]
fn queries_remain_available_during_snapshot_and_pause_allows_manual() {
    let mut f = Fixture::new();
    f.import();
    f.config.settings.stable_file_check_ms = 250;
    std::fs::write(&f.yaml, serde_yaml::to_string(&f.config).unwrap()).unwrap();
    f.engine.add_config(&f.yaml).unwrap();
    f.engine.set_paused(true).unwrap();
    let scheduler = Scheduler::start(f.engine.clone(), Arc::new(|_, _| {}));
    let start = Instant::now();
    for _ in 0..20 {
        scheduler.enqueue(Some("demo"), Some("offers")).unwrap();
    }
    // Reader must work while the worker is waiting for stable source metadata.
    for _ in 0..10 {
        assert_eq!(
            f.engine
                .query("demo", "offer", &Default::default(), None)
                .unwrap()
                .total,
            120
        );
    }
    assert!(start.elapsed() < Duration::from_secs(2));
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if f.engine
            .history(Some("demo"), None, 0)
            .unwrap()
            .iter()
            .any(|r| r["trigger"] == "manual" && r["status"] == "succeeded")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    scheduler.shutdown();
    let manual = f
        .engine
        .history(Some("demo"), None, 0)
        .unwrap()
        .into_iter()
        .filter(|r| r["trigger"] == "manual")
        .collect::<Vec<_>>();
    assert!(!manual.is_empty());
    assert!(
        manual.len() <= 2,
        "Rapid requests should coalesce: {}",
        manual.len()
    );
}
#[test]
fn database_failure_rolls_back_all_activated_tables() {
    let f = Fixture::new();
    f.import();
    let c = f.engine.connection().unwrap();
    c.execute_batch("CREATE TRIGGER fail_activation BEFORE INSERT ON record_field_index BEGIN SELECT RAISE(ABORT,'injected activation failure'); END;").unwrap();
    f.offers(&[("NEW", "Changed")]);
    assert!(f
        .engine
        .sync_workbook("demo", &f.config.sources[0].path, true, "test")
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
    let id = explorer_core::db::identity("demo", "offer", "primary", "OFF-001");
    assert_eq!(
        f.engine
            .related(&id, "offer_invoices", &Default::default())
            .unwrap()
            .total,
        2
    );
}
#[cfg(windows)]
#[test]
fn windows_exclusive_lock_preserves_cache() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    f.import();
    let path = &f.config.sources[0].path;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .unwrap();
    assert!(f
        .engine
        .sync_workbook("demo", path, true, "lock-test")
        .is_err());
    assert_eq!(
        f.engine
            .query("demo", "offer", &Default::default(), None)
            .unwrap()
            .total,
        120
    );
    drop(lock);
    assert!(f
        .engine
        .sync_workbook("demo", path, true, "lock-recovery")
        .unwrap());
}
