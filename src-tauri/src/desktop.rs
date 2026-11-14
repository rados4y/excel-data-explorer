use crate::{
    commands::Request,
    engine::Engine,
    lifecycle::{self, CloseAction, LaunchAction},
    sync::Scheduler,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
struct DesktopState {
    engine: Engine,
    scheduler: Arc<Scheduler>,
    logs: PathBuf,
    exit_ready: Arc<AtomicBool>,
    _log_guard: tracing_appender::non_blocking::WorkerGuard,
}
#[tauri::command]
async fn dispatch(
    app: tauri::AppHandle,
    request: Request,
    state: tauri::State<'_, DesktopState>,
) -> Result<serde_json::Value, String> {
    let engine = state.engine.clone();
    let scheduler = state.scheduler.clone();
    let logs = state.logs.clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
        match request {
            Request::SyncNow {
                workspace_id,
                source_id,
            } => {
                scheduler.enqueue(workspace_id.as_deref(), source_id.as_deref())?;
                Ok(serde_json::json!({"status":"queued"}))
            }
            Request::OpenDocument { document_id } => {
                let path = engine.validated_document(&document_id)?;
                app.opener()
                    .open_path(path.to_string_lossy().to_string(), None::<&str>)?;
                Ok(serde_json::json!(null))
            }
            Request::OpenLogs => {
                app.opener()
                    .open_path(logs.to_string_lossy().to_string(), None::<&str>)?;
                Ok(serde_json::json!(null))
            }
            Request::UpdateSettings { settings } => {
                // Register OS preference before persisting; failure leaves previous preference intact.
                if settings.autostart {
                    app.autolaunch().enable()?;
                } else {
                    app.autolaunch().disable()?;
                }
                let result = engine.update_settings(settings)?;
                scheduler.scan();
                Ok(serde_json::to_value(result)?)
            }
            Request::AddWorkspaceConfig { config_file_path } => {
                let w = engine.add_config(Path::new(&config_file_path))?;
                let _ =
                    engine.save_ui_state(&format!("config-error:{}", w.id), &serde_json::json!({}));
                scheduler.scan();
                let _ = app.emit(
                    "workspace.configChanged",
                    serde_json::json!({"workspace_id":w.id}),
                );
                Ok(serde_json::to_value(w)?)
            }
            Request::SetSyncPaused { paused } => {
                let value = engine.dispatch(Request::SetSyncPaused { paused })?;
                scheduler.scan();
                Ok(value)
            }
            other => engine.dispatch(other),
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
#[tauri::command]
async fn pick_workspace_config(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Workspace YAML", &["yaml", "yml"])
            .blocking_pick_file()
            .map(|p| p.to_string())
    })
    .await
    .map_err(|e| e.to_string())
}
fn show(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
fn copy_tree(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for item in std::fs::read_dir(source)? {
        let item = item?;
        let dest = target.join(item.file_name());
        if item.file_type()?.is_dir() {
            copy_tree(&item.path(), &dest)?;
        } else {
            std::fs::copy(item.path(), dest)?;
        }
    }
    Ok(())
}
fn install_demo(app: &tauri::AppHandle, engine: &Engine, data: &Path) -> anyhow::Result<()> {
    if !engine.list_workspaces()?.is_empty() {
        return Ok(());
    }
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let resource = app.path().resource_dir()?;
    let source = if cfg!(debug_assertions) && project.join("fixtures/demo").exists() {
        project.to_path_buf()
    } else {
        resource
    };
    let demo = data.join("demo-workspace");
    // Per-user writable demo copy. Never overwrite an existing user/demo workspace.
    if !demo.join("workspaces/demo-workspace.yaml").exists() {
        copy_tree(&source.join("workspaces"), &demo.join("workspaces"))?;
        copy_tree(&source.join("fixtures/demo"), &demo.join("fixtures/demo"))?;
    }
    engine.add_config(&demo.join("workspaces/demo-workspace.yaml"))?;
    engine.add_config(&demo.join("workspaces/alternate-workspace.yaml"))?;
    let mut settings = engine.settings()?;
    settings.selected_workspace = Some("demo".into());
    engine.update_settings(settings)?;
    Ok(())
}
pub fn run() {
    let app=tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app,args,_cwd|{if lifecycle::launch_action(&args,true)==LaunchAction::Show{show(app);}}))
        .plugin(tauri_plugin_autostart::Builder::new().args(["--background"]).build())
        .plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().with_state_flags(tauri_plugin_window_state::StateFlags::all()-tauri_plugin_window_state::StateFlags::VISIBLE).build())
        .invoke_handler(tauri::generate_handler![dispatch,pick_workspace_config])
        .setup(|app|{
            // BaseDirs resolves LOCALAPPDATA on Windows. Cache is never placed in a configured share.
            let data=directories::BaseDirs::new().ok_or("Cannot resolve per-user local data directory")?.data_local_dir().join("ExcelDataExplorer");std::fs::create_dir_all(&data)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&data,std::fs::Permissions::from_mode(0o700))?;}
            let logs=data.join("logs");std::fs::create_dir_all(&logs)?;
            let log=tracing_appender::rolling::daily(&logs,"explorer.log");let (writer,guard)=tracing_appender::non_blocking(log);
            let _=tracing_subscriber::fmt().with_ansi(false).with_writer(writer).with_max_level(tracing::Level::INFO).try_init();
            let engine=Engine::open(data.join("cache.sqlite"))?;install_demo(app.handle(),&engine,&data)?;
            let settings=engine.settings()?;
            let registration=if settings.autostart{app.autolaunch().enable()}else{app.autolaunch().disable()};
            if registration.is_ok(){engine.save_ui_state("lifecycle-error",&serde_json::json!({}))?;}
            if let Err(e)=registration{tracing::warn!("Autostart registration failed: {e}");engine.save_ui_state("lifecycle-error",&serde_json::json!({"error":format!("Autostart registration failed: {e}")}))?;}
            let event_app=app.handle().clone();let event_engine=engine.clone();
            let scheduler=Scheduler::start(engine.clone(),Arc::new(move|name,payload|{
                let _=event_app.emit(name,payload);
                if let Ok(status)=event_engine.sync_status(None){let sources=status["sources"].as_array();let errors=sources.is_some_and(|s|s.iter().any(|s|s["last_error"].is_string()));
                    if let Some(tray)=event_app.tray_by_id("main-tray"){let _=tray.set_tooltip(Some(if errors{"Excel Data Explorer — error; using cached data"}else if status["paused"]==true{"Excel Data Explorer — sync paused"}else{"Excel Data Explorer — ready from cache"}));}
                }
            }));
            app.manage(DesktopState{engine,scheduler,logs,exit_ready:Arc::new(AtomicBool::new(false)),_log_guard:guard});
            let open=MenuItem::with_id(app,"open","Open",true,None::<&str>)?;let sync=MenuItem::with_id(app,"sync","Sync now",true,None::<&str>)?;let pause=MenuItem::with_id(app,"pause","Pause / Resume sync",true,None::<&str>)?;let settings=MenuItem::with_id(app,"settings","Settings",true,None::<&str>)?;let status=MenuItem::with_id(app,"status","Sync status",true,None::<&str>)?;let exit=MenuItem::with_id(app,"exit","Exit",true,None::<&str>)?;
            let menu=Menu::with_items(app,&[&open,&sync,&pause,&settings,&status,&exit])?;
            TrayIconBuilder::with_id("main-tray").icon(tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?).tooltip("Excel Data Explorer — ready from cache").menu(&menu).show_menu_on_left_click(false)
                .on_tray_icon_event(|tray,event|{if matches!(event,TrayIconEvent::Click{button:MouseButton::Left,button_state:MouseButtonState::Up,..}){show(tray.app_handle());}})
                .on_menu_event(|app,event|{let state=app.state::<DesktopState>();match event.id.as_ref(){
                    "exit"=>app.exit(0),"sync"=>{let _=state.scheduler.enqueue(None,None);},"pause"=>{let _=state.engine.set_paused(!state.engine.paused.load(Ordering::Relaxed));state.scheduler.scan();let _=app.emit("sync.statusChanged",serde_json::json!({}));},
                    "settings"|"status"=>{show(app);let _=app.emit("explorer.navigate",serde_json::json!({"page":if event.id.as_ref()=="settings"{"settings"}else{"sync"}}));},_=>show(app)
                }}).build(app)?;
            if lifecycle::launch_action(&std::env::args().collect::<Vec<_>>(),false)==LaunchAction::Show{show(app.handle());}Ok(())
        })
        .on_window_event(|window,event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{if let Some(state)=window.app_handle().try_state::<DesktopState>(){if lifecycle::close_action(&state.engine.settings().unwrap_or_default(),false)==CloseAction::Hide{api.prevent_close();let _=window.hide();}}}})
        .build(tauri::generate_context!()).expect("Desktop initialization failed; cache was preserved");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            let state = app.state::<DesktopState>();
            if !state.exit_ready.load(Ordering::Relaxed) {
                api.prevent_exit();
                let scheduler = state.scheduler.clone();
                let ready = state.exit_ready.clone();
                let app = app.clone();
                std::thread::spawn(move || {
                    scheduler.shutdown();
                    ready.store(true, Ordering::Relaxed);
                    app.exit(0);
                });
            }
        }
        tauri::RunEvent::Resumed => {
            if let Some(state) = app.try_state::<DesktopState>() {
                state.scheduler.scan();
            }
        }
        _ => {}
    });
}
