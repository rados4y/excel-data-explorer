use anyhow::Result;
use explorer_core::{
    db,
    engine::{Engine, QueryOptions},
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, time::Instant};
fn percentiles(mut times: Vec<f64>) -> Value {
    times.sort_by(|a, b| a.total_cmp(b));
    let n = times.len();
    json!({"samples":n,"p50_ms":times[n/2],"p95_ms":times[((n as f64*0.95).ceil() as usize).saturating_sub(1)],"p99_ms":times[((n as f64*0.99).ceil() as usize).saturating_sub(1)]})
}
fn measure<F: FnMut() -> Result<()>>(n: usize, mut f: F) -> Result<Value> {
    let mut times = Vec::new();
    for _ in 0..n {
        let now = Instant::now();
        f()?;
        times.push(now.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(percentiles(times))
}
fn peak_rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|s| s.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let config = args
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("benchmark <config.yaml> <local.sqlite> [output.json]"))?;
    let path = args.get(2).context("database path required")?;
    let start = Instant::now();
    let engine = Engine::open(path)?;
    let w = engine.add_config(Path::new(config))?;
    let config = engine.config(&w.id)?;
    let setup_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut imports = Vec::new();
    for (path, _) in config.workbook_groups() {
        let now = Instant::now();
        let changed = engine.sync_workbook(&w.id, &path, false, "benchmark")?;
        imports.push(json!({"workbook":Path::new(&path).file_name().unwrap().to_string_lossy(),"elapsed_ms":now.elapsed().as_secs_f64()*1000.0,"imported":changed}));
    }
    let query = QueryOptions {
        limit: Some(100),
        ..Default::default()
    };
    let count = engine
        .query(&w.id, &config.views.main.entity, &query, None)?
        .total;
    let default_page = measure(50, || {
        engine.query(&w.id, &config.views.main.entity, &query, None)?;
        Ok(())
    })?;
    let search = QueryOptions {
        search: "Project 025".into(),
        ..query.clone()
    };
    let search_timing = measure(50, || {
        engine.query(&w.id, &config.views.main.entity, &search, None)?;
        Ok(())
    })?;
    let mut filters = BTreeMap::new();
    filters.insert("status".into(), json!("Active"));
    let filter = QueryOptions {
        filters,
        sort: Some("amount".into()),
        ..query.clone()
    };
    let filter_timing = measure(50, || {
        engine.query(&w.id, &config.views.main.entity, &filter, None)?;
        Ok(())
    })?;
    let id = db::identity(&w.id, &config.views.main.entity, "primary", "OFF-001");
    let details = measure(100, || {
        engine.record(&id)?;
        Ok(())
    })?;
    let relations = measure(100, || {
        engine.related(&id, "offer_invoices", &query)?;
        Ok(())
    })?;
    let warm_open = measure(30, || {
        let e = Engine::open(path)?;
        e.query(&w.id, &config.views.main.entity, &query, None)?;
        Ok(())
    })?;
    // Force one workbook import while measuring actual concurrent foreground reads.
    let e = engine.clone();
    let workspace = w.id.clone();
    let workbook = config.sources[0].path.clone();
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let active = running.clone();
    let worker = std::thread::spawn(move || {
        let result = e.sync_workbook(&workspace, &workbook, true, "benchmark-concurrent");
        active.store(false, std::sync::atomic::Ordering::SeqCst);
        result
    });
    let mut concurrent = Vec::new();
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        let now = Instant::now();
        engine.query(&w.id, &config.views.main.entity, &query, None)?;
        concurrent.push(now.elapsed().as_secs_f64() * 1000.0);
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    worker.join().unwrap()?;
    let report = json!({"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"setup_ms":setup_ms,"main_records":count,"imports":imports,"default_page":default_page,"search":search_timing,"filter_sort":filter_timing,"record_detail":details,"related_tab":relations,"process_warm_cache_open":warm_open,"foreground_query_during_import":percentiles(concurrent),"peak_rss_kib":peak_rss_kib(),"database_bytes":std::fs::metadata(path)?.len(),"windows_launch_restore_idle_cpu_fps":"not measured"});
    if let Some(output) = args.get(3) {
        std::fs::write(output, serde_json::to_string_pretty(&report)?)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
use anyhow::Context;
