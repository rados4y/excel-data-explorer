use crate::{
    config::{is_network, Config, Source},
    db,
    engine::Engine,
    excel::Diagnostic,
};
use anyhow::{ensure, Context, Result};
use notify::Watcher;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub size: i64,
    pub mtime: String,
}
type CachedFingerprint = (Option<i64>, Option<String>, Option<String>, String);
pub fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let m = fs::metadata(path).context("Source missing, locked or unavailable")?;
    ensure!(m.is_file(), "Source is not a regular file");
    let time = m
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .to_string();
    Ok(Fingerprint {
        size: m.len() as i64,
        mtime: time,
    })
}
fn cancellation_wait(cancel: &AtomicBool, duration: Duration) -> Result<()> {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        ensure!(!cancel.load(Ordering::Relaxed), "Import cancelled");
        thread::sleep(
            until
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        );
    }
    Ok(())
}
pub fn stable_snapshot(
    path: &Path,
    config: &Config,
    cancel: &AtomicBool,
) -> Result<(tempfile::NamedTempFile, Fingerprint)> {
    let mut last = None;
    for attempt in 0..config.settings.stable_file_attempts {
        let acquire = (|| -> Result<_> {
            ensure!(!cancel.load(Ordering::Relaxed), "Import cancelled");
            let before = fingerprint(path)?;
            cancellation_wait(
                cancel,
                Duration::from_millis(config.settings.stable_file_check_ms),
            )?;
            ensure!(
                before == fingerprint(path)?,
                "Workbook is still changing; retry scheduled"
            );
            let suffix = path
                .extension()
                .and_then(|s| s.to_str())
                .context("Workbook has no extension")?;
            let mut snapshot = tempfile::Builder::new()
                .prefix("ede-snapshot-")
                .suffix(&format!(".{suffix}"))
                .tempfile()?;
            let mut file = File::open(path).context("Source locked or unavailable")?;
            let mut buffer = [0u8; 65536];
            loop {
                ensure!(!cancel.load(Ordering::Relaxed), "Import cancelled");
                let n = file
                    .read(&mut buffer)
                    .context("Source interrupted during snapshot")?;
                if n == 0 {
                    break;
                }
                snapshot.write_all(&buffer[..n])?;
            }
            snapshot.flush()?;
            ensure!(
                before == fingerprint(path)?
                    && before.size == snapshot.as_file().metadata()?.len() as i64,
                "Workbook changed during snapshot"
            );
            Ok((snapshot, before))
        })();
        match acquire {
            Ok(value) => return Ok(value),
            Err(e) => last = Some(e),
        }
        if attempt + 1 < config.settings.stable_file_attempts {
            cancellation_wait(cancel, Duration::from_millis(250 * (1 << attempt)))?;
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("Snapshot unavailable")))
}
impl Engine {
    pub fn sync_workbook(
        &self,
        workspace: &str,
        path: &str,
        force: bool,
        trigger: &str,
    ) -> Result<bool> {
        let config = self.config(workspace)?;
        let sources: Vec<&Source> = config.sources.iter().filter(|s| s.path == path).collect();
        ensure!(!sources.is_empty(), "Workbook is not configured");
        let c = self.connection()?;
        for s in &sources {
            c.execute("UPDATE source_state SET last_checked_at=CURRENT_TIMESTAMP WHERE source_definition_id=?",[db::qualified(workspace,&s.id)])?;
        }
        let observed = fingerprint(Path::new(path));
        let hash = config.import_hash();
        if !force {
            if let Ok(ref observed) = observed {
                let mut unchanged = true;
                for s in &sources {
                    let row:Option<CachedFingerprint>=c.query_row("SELECT last_seen_size,last_seen_mtime,last_seen_hash,status FROM source_state WHERE source_definition_id=?",[db::qualified(workspace,&s.id)],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
                    unchanged &= row.is_some_and(|(size, mtime, config_hash, status)| {
                        size == Some(observed.size)
                            && mtime.as_ref() == Some(&observed.mtime)
                            && config_hash.as_ref() == Some(&hash)
                            && status == "ready"
                    });
                }
                if unchanged {
                    return Ok(false);
                }
            }
        }
        c.execute("INSERT INTO import_runs(workspace_id,workbook,status,trigger) VALUES(?,?,'checking',?)",params![workspace,path,trigger])?;
        let run = c.last_insert_rowid();
        let start = Instant::now();
        for s in &sources {
            c.execute("UPDATE source_state SET status='checking',last_attempt_at=CURRENT_TIMESTAMP WHERE source_definition_id=?",[db::qualified(workspace,&s.id)])?;
        }
        let result = (|| -> Result<_> {
            self.stage_status(&config, &sources, "snapshotting")?;
            let (snapshot, fp) = stable_snapshot(Path::new(path), &config, &self.cancelled)?;
            self.stage_status(&config, &sources, "parsing")?;
            self.import_snapshot(&config, &sources, snapshot.path(), run, &fp, start)
        })();
        match result {
            Ok(_counts) => {
                tracing::info!(
                    source_count = sources.len(),
                    import_id = run,
                    elapsed_ms = start.elapsed().as_millis() as i64,
                    "workbook activated"
                );
                Ok(true)
            }
            Err(err) => {
                let unavailable = observed.is_err()
                    || err.to_string().contains("locked")
                    || err.to_string().contains("interrupted during snapshot");
                let cancelled = self.cancelled.load(Ordering::Relaxed);
                let status = if cancelled {
                    "error"
                } else if unavailable {
                    "unavailable"
                } else {
                    "error"
                };
                for s in &sources {
                    c.execute("UPDATE source_state SET status=?,last_error=? WHERE source_definition_id=?",params![status,err.to_string(),db::qualified(workspace,&s.id)])?;
                }
                c.execute("UPDATE import_runs SET status=?,error=?,finished_at=CURRENT_TIMESTAMP,elapsed_ms=? WHERE id=?",params![if cancelled{"cancelled"}else{"failed"},err.to_string(),start.elapsed().as_millis() as i64,run])?;
                self.store_diagnostic(
                    run,
                    &Diagnostic {
                        severity: "error".into(),
                        source_id: sources
                            .iter()
                            .map(|s| s.id.as_str())
                            .collect::<Vec<_>>()
                            .join(","),
                        sheet: sources
                            .iter()
                            .map(|s| s.sheet.as_str())
                            .collect::<Vec<_>>()
                            .join(","),
                        row: 0,
                        field: None,
                        code: if unavailable {
                            "source_unavailable"
                        } else {
                            "import_failed"
                        }
                        .into(),
                        message: err.to_string(),
                    },
                )?;
                tracing::warn!(
                    import_id = run,
                    status,
                    "workbook import failed; retaining cached generation"
                );
                Err(err)
            }
        }
    }
}
#[derive(Default)]
struct QueueState {
    pending: BTreeMap<(String, String), (bool, String)>,
    rescan: bool,
}
struct Shared {
    queue: Mutex<QueueState>,
    wake: Condvar,
}
type Events = Arc<dyn Fn(&str, Value) + Send + Sync>;
pub struct Scheduler {
    pub engine: Engine,
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
    _watcher: Mutex<Option<notify::RecommendedWatcher>>,
}
impl Scheduler {
    pub fn start(engine: Engine, events: Events) -> Arc<Self> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(QueueState {
                rescan: true,
                ..Default::default()
            }),
            wake: Condvar::new(),
        });
        let scheduler = Arc::new(Self {
            engine: engine.clone(),
            shared: shared.clone(),
            worker: Mutex::new(None),
            _watcher: Mutex::new(None),
        });
        let watch_shared = shared.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                if let Ok(event) = result {
                    if !event.kind.is_access() {
                        let mut q = watch_shared.queue.lock().unwrap_or_else(|e| e.into_inner());
                        q.rescan = true;
                        watch_shared.wake.notify_one();
                    }
                }
            })
            .ok();
        // Watch containing directories to survive Excel's rename/replace save pattern. Periodic scans cover missed events.
        if let Some(ref mut watcher) = watcher {
            let mut dirs = std::collections::BTreeSet::new();
            if let Ok(workspaces) = engine.list_workspaces() {
                for w in workspaces {
                    if let Ok(c) = engine.config(&w.id) {
                        for p in c.workbook_groups().keys() {
                            if !is_network(p) {
                                if let Some(parent) = Path::new(p).parent() {
                                    dirs.insert(parent.to_path_buf());
                                }
                            }
                        }
                    }
                    if let Some(parent) = Path::new(&w.config_path).parent() {
                        dirs.insert(parent.to_path_buf());
                    }
                }
            }
            for d in dirs {
                let _ = watcher.watch(&d, notify::RecursiveMode::NonRecursive);
            }
        }
        *scheduler._watcher.lock().unwrap() = watcher;
        let worker_engine = engine.clone();
        let worker_shared = shared.clone();
        let worker = thread::spawn(move || run_worker(worker_engine, worker_shared, events));
        *scheduler.worker.lock().unwrap() = Some(worker);
        scheduler
    }
    pub fn enqueue(&self, workspace: Option<&str>, source: Option<&str>) -> Result<()> {
        let mut jobs = vec![];
        for w in self
            .engine
            .list_workspaces()?
            .iter()
            .filter(|w| workspace.is_none_or(|id| id == w.id))
        {
            let c = self.engine.config(&w.id)?;
            for (path, sources) in c.workbook_groups() {
                if source.is_none_or(|id| sources.iter().any(|s| s.id == id)) {
                    jobs.push((w.id.clone(), path));
                }
            }
        }
        ensure!(!jobs.is_empty(), "No matching configured sources");
        let mut q = self
            .shared
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("Queue unavailable"))?;
        ensure!(
            q.pending.len()
                + jobs
                    .iter()
                    .filter(|key| !q.pending.contains_key(*key))
                    .count()
                <= 256,
            "Import queue capacity reached"
        );
        for key in jobs {
            q.pending.insert(key, (true, "manual".into()));
        }
        self.shared.wake.notify_one();
        Ok(())
    }
    pub fn scan(&self) {
        let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.rescan = true;
        self.shared.wake.notify_one();
    }
    pub fn shutdown(&self) {
        self.engine.cancelled.store(true, Ordering::Relaxed);
        self.shared.wake.notify_all();
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}
fn run_worker(engine: Engine, shared: Arc<Shared>, events: Events) {
    let mut due: BTreeMap<(String, String), Instant> = BTreeMap::new();
    let mut document_due = Instant::now();
    loop {
        if engine.cancelled.load(Ordering::Relaxed) {
            break;
        }
        let (rescan, manual) = {
            let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            let r = q.rescan;
            q.rescan = false;
            let key = q.pending.keys().next().cloned();
            let job = key.and_then(|key| q.pending.remove(&key).map(|v| (key, v)));
            (r, job)
        };
        let paused = engine.paused.load(Ordering::Relaxed);
        if rescan || due.values().any(|d| *d <= Instant::now()) || due.is_empty() {
            if let Ok(workspaces) = engine.list_workspaces() {
                for w in workspaces {
                    // Invalid config files never replace persisted profiles. Report through the UI event and persisted setting.
                    if Path::new(&w.config_path).is_file() {
                        let previous = engine.config(&w.id).ok().map(|c| c.hash());
                        let reload=Config::load(Path::new(&w.config_path)).and_then(|c| {ensure!(c.workspace.id==w.id,"Workspace ID changed; add the edited file as a new workspace explicitly");engine.add_config(Path::new(&w.config_path))});
                        match reload {
                            Ok(_) => {
                                let _ = engine
                                    .save_ui_state(&format!("config-error:{}", w.id), &json!({}));
                                if engine.config(&w.id).ok().map(|c| c.hash()) != previous {
                                    events("workspace.configChanged", json!({"workspace_id":w.id}));
                                }
                            }
                            Err(e) => {
                                let text = e.to_string();
                                let _ = engine.save_ui_state(
                                    &format!("config-error:{}", w.id),
                                    &json!({"error":text}),
                                );
                                events(
                                    "workspace.configError",
                                    json!({"workspace_id":w.id,"error":text}),
                                );
                            }
                        }
                    }
                    if let Ok(config) = engine.config(&w.id) {
                        for (path, _) in config.workbook_groups() {
                            let key = (w.id.clone(), path.clone());
                            let next = due.entry(key.clone()).or_insert(Instant::now());
                            if !paused && (rescan || *next <= Instant::now()) {
                                let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
                                if q.pending.len() < 256 {
                                    q.pending.entry(key).or_insert((
                                        false,
                                        if rescan { "notification" } else { "periodic" }.into(),
                                    ));
                                }
                            }
                            let seconds = if is_network(&path) {
                                config.settings.network_scan_seconds
                            } else {
                                config.settings.local_scan_seconds
                            };
                            if rescan || *next <= Instant::now() {
                                *next = Instant::now() + Duration::from_secs(seconds);
                            }
                        }
                    }
                }
            }
        }
        let job = manual.or_else(|| {
            let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            let key = q.pending.keys().next().cloned()?;
            let value = q.pending.remove(&key)?;
            Some((key, value))
        });
        if let Some(((ws, path), (force, trigger))) = job {
            if !paused || force {
                events(
                    "sync.statusChanged",
                    json!({"workspace_id":ws,"stage":"checking"}),
                );
                let result = engine.sync_workbook(&ws, &path, force, &trigger);
                events("sync.statusChanged", json!({"workspace_id":ws}));
                if result.unwrap_or(false) {
                    events("sync.sourceUpdated", json!({"workspace_id":ws}));
                    events("recordCache.invalidated", json!({"workspace_id":ws}));
                }
            }
            continue;
        }
        if !paused && Instant::now() >= document_due {
            let _ = engine.refresh_documents(None);
            document_due = Instant::now() + Duration::from_secs(30);
            events("sync.statusChanged", json!({"documents_checked":true}));
        }
        let q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        if q.pending.is_empty() && !q.rescan && !engine.cancelled.load(Ordering::Relaxed) {
            let _ = shared.wake.wait_timeout(q, Duration::from_secs(30));
        }
    }
}
