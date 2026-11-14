#[cfg(not(feature = "desktop"))]
fn main() -> anyhow::Result<()> {
    use explorer_core::{commands::Request, engine::Engine};
    use std::io::{self, BufRead};
    let args: Vec<String> = std::env::args().collect();
    let db = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("./explorer.sqlite");
    if args.get(1).map(String::as_str) == Some("rpc") {
        let e = Engine::open(db)?;
        for line in io::stdin().lock().lines() {
            let line = line?;
            let result = serde_json::from_str::<Request>(&line)
                .map_err(anyhow::Error::from)
                .and_then(|r| e.dispatch(r));
            let response = match result {
                Ok(value) => serde_json::json!({"ok":value}),
                Err(err) => serde_json::json!({"error":err.to_string()}),
            };
            println!("{response}");
        }
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("import") {
        let e = Engine::open(db)?;
        let config = args
            .get(3)
            .ok_or_else(|| anyhow::anyhow!("Usage: import <local-db> <workspace.yaml>"))?;
        let w = e.add_config(std::path::Path::new(config))?;
        println!(
            "{}",
            e.dispatch(Request::SyncNow {
                workspace_id: Some(w.id),
                source_id: None
            })?
        );
        return Ok(());
    }
    println!("Excel Data Explorer {}\nCore CLI: import <local-db> <workspace.yaml> | rpc <local-db> (JSON lines on stdin)\nDesktop: npm run desktop",explorer_core::version());
    Ok(())
}
#[cfg(feature = "desktop")]
fn main() {
    explorer_core::desktop::run();
}
