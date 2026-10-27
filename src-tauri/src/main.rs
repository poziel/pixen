#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod settings_file;
mod storage_migration;
mod export;
mod ai;
mod ai_context;
mod color_sampler;
use clap::Parser;
use color_sampler::sample_color;
use pixen::desktop::{Application, DocumentWorkspace, RenderRequest, Rendered, WorkspaceSnapshot};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};

#[derive(Parser)]
#[command(version, about = "Pixen pixel art workbench")]
struct Args {
    #[arg(long, hide = true)]
    ai_mcp_proxy: bool,
    input: Option<PathBuf>,
    #[arg(long, hide = true, conflicts_with_all = ["input", "managed", "ai_mcp_proxy", "storage_resume"])]
    storage_migrate: Option<PathBuf>,
    #[arg(long, hide = true, conflicts_with_all = ["input", "managed", "ai_mcp_proxy"])]
    storage_resume: Option<PathBuf>,
    #[arg(long, hide = true, requires = "input")]
    managed: bool,
}
#[derive(Clone)]
struct Backend {
    app: Arc<Mutex<DocumentWorkspace>>,
    managed: bool,
    migrating: Arc<std::sync::atomic::AtomicBool>,
}
type Reply<T> = Result<T, String>;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
async fn work<T: Send + 'static>(
    state: &Backend,
    run: impl FnOnce(&mut Application) -> Reply<T> + Send + 'static,
) -> Reply<T> {
    workspace_work(state, move |workspace| run(workspace.active())).await
}
async fn workspace_work<T: Send + 'static>(
    state: &Backend,
    run: impl FnOnce(&mut DocumentWorkspace) -> Reply<T> + Send + 'static,
) -> Reply<T> {
    if state.migrating.load(std::sync::atomic::Ordering::SeqCst) { return Err("Storage migration is pending".into()); }
    let app = state.app.clone();
    let migrating = state.migrating.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut app = app.lock().map_err(error)?;
        if migrating.load(std::sync::atomic::Ordering::SeqCst) { return Err("Storage migration is pending".into()); }
        run(&mut app)
    })
    .await
    .map_err(error)?
}
fn acknowledge(value: serde_json::Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}
#[tauri::command]
async fn snapshot(state: tauri::State<'_, Backend>) -> Reply<WorkspaceSnapshot> {
    workspace_work(&state, |app| app.snapshot().map_err(error)).await
}
#[tauri::command]
async fn read_cel(
    state: tauri::State<'_, Backend>,
    document_id: String,
    frame_id: String,
    layer_id: String,
) -> Reply<pixen::desktop::CelPixels> {
    workspace_work(&state, move |workspace| {
        if workspace
            .snapshot()
            .map_err(error)?
            .active_document
            .as_deref()
            != Some(&document_id)
        {
            return Err("active document changed".into());
        }
        workspace
            .active()
            .cel_data(&frame_id, &layer_id)
            .map_err(error)
    })
    .await
}
#[tauri::command]
async fn paint_pixels(
    state: tauri::State<'_, Backend>,
    document_id: String,
    generation: u64,
    frame_id: String,
    layer_id: String,
    pixels: Vec<pixen::desktop::PaintPixel>,
) -> Reply<()> {
    workspace_work(&state, move |workspace| {
        if workspace
            .snapshot()
            .map_err(error)?
            .active_document
            .as_deref()
            != Some(&document_id)
        {
            return Err("active document changed".into());
        }
        workspace
            .active()
            .paint(generation, &frame_id, &layer_id, pixels)
            .map_err(error)
    })
    .await
}
#[tauri::command]
async fn edit_structure(
    state: tauri::State<'_, Backend>,
    document_id: String,
    generation: u64,
    operation: pixen::desktop::StructureOperation,
) -> Reply<()> {
    workspace_work(&state, move |workspace| {
        if workspace
            .snapshot()
            .map_err(error)?
            .active_document
            .as_deref()
            != Some(&document_id)
        {
            return Err("active document changed".into());
        }
        workspace
            .active()
            .edit_structure(generation, operation)
            .map_err(error)
    })
    .await
}

#[tauri::command]
async fn edit_history(
    state: tauri::State<'_, Backend>,
    document_id: String,
    redo: bool,
) -> Reply<()> {
    workspace_work(&state, move |workspace| {
        if workspace
            .snapshot()
            .map_err(error)?
            .active_document
            .as_deref()
            != Some(&document_id)
        {
            return Err("active document changed".into());
        }
        workspace.active().history(redo).map_err(error)
    })
    .await
}
#[tauri::command]
async fn open_document(state: tauri::State<'_, Backend>, path: Option<PathBuf>) -> Reply<bool> {
    if state.managed {
        return Err("managed preview cannot open another file".into());
    }
    workspace_work(&state, move |app| {
        let path = path.or_else(|| {
            rfd::FileDialog::new()
                .add_filter(
                    "Pixel art",
                    &[
                        "pixloom", "json", "pxo", "ase", "aseprite", "png", "jpg", "jpeg", "gif",
                        "apng", "webp", "bmp", "tif", "tiff",
                    ],
                )
                .pick_file()
        });
        if let Some(path) = path {
            app.open(&path).map_err(error)?;
            Ok(true)
        } else {
            Ok(false)
        }
    })
    .await
}
#[tauri::command]
async fn create_document(state: tauri::State<'_, Backend>, width: u32, height: u32) -> Reply<()> {
    workspace_work(&state, move |app| app.create(width, height).map_err(error)).await
}
#[tauri::command]
async fn resize_document(state: tauri::State<'_, Backend>, width: u32, height: u32) -> Reply<()> {
    work(&state, move |app| app.resize(width, height).map_err(error)).await
}
#[tauri::command]
async fn toggle_view(state: tauri::State<'_, Backend>) -> Reply<()> {
    work(&state, |app| app.toggle_view().map_err(error)).await
}
#[tauri::command]
async fn follow_target(state: tauri::State<'_, Backend>, frame_id: String) -> Reply<String> {
    work(&state, move |app| {
        app.follow_target(&frame_id)
            .map(str::to_owned)
            .map_err(error)
    })
    .await
}
#[tauri::command]
async fn configure_recovery(state: tauri::State<'_, Backend>, seconds: u64) -> Reply<()> {
    workspace_work(&state, move |workspace| {
        workspace.configure_recovery(seconds).map_err(error)
    })
    .await
}
#[tauri::command]
async fn save_document(
    state: tauri::State<'_, Backend>,
    save_as: bool,
    document_id: Option<String>,
) -> Reply<bool> {
    if state.managed {
        return Err("managed preview is read-only".into());
    }
    let (id, needs_path) = workspace_work(&state, move |workspace| {
        let id = match document_id {
            Some(id) => id,
            None => workspace
                .snapshot()
                .map_err(error)?
                .active_document
                .ok_or("no document")?,
        };
        let doc = workspace
            .document_mut(&id)
            .map_err(error)?
            .snapshot()
            .map_err(error)?
            .document
            .ok_or("no document")?;
        let needs = save_as
            || doc.path.as_ref().is_none_or(|p| {
                !(p.to_string_lossy().ends_with(".pixloom")
                    || p.to_string_lossy().ends_with(".pixen.json"))
            });
        Ok((id, needs))
    })
    .await?;
    let path = if needs_path {
        tauri::async_runtime::spawn_blocking(|| {
            rfd::FileDialog::new()
                .add_filter("Pixen projects", &["pixloom"])
                .set_file_name("artwork.pixloom")
                .save_file()
        })
        .await
        .map_err(error)?
    } else {
        None
    };
    if needs_path && path.is_none() {
        return Ok(false);
    }
    let target = id.clone();
    let snapshot = workspace_work(&state, move |workspace| {
        workspace
            .document_mut(&target)
            .map_err(error)?
            .prepare_save(path.as_deref())
            .map_err(error)
    })
    .await?;
    // Crucially, no workspace/session mutex is held during encoding, validation, or disk I/O.
    let result = tauri::async_runtime::spawn_blocking(move || snapshot.write())
        .await
        .map_err(error)
        .and_then(|result| result.map_err(error));
    workspace_work(&state, move |workspace| {
        let app = workspace.document_mut(&id).map_err(error)?;
        match result {
            Ok(saved) => {
                app.finish_save(&saved).map_err(error)?;
                Ok(true)
            }
            Err(e) => {
                app.abort_save();
                Err(e)
            }
        }
    })
    .await
}
#[tauri::command]
async fn render(
    state: tauri::State<'_, Backend>,
    request: RenderRequest,
    document_id: Option<String>,
) -> Reply<Rendered> {
    workspace_work(&state, move |app| {
        app.render(document_id.as_deref(), request).map_err(error)
    })
    .await
}
#[tauri::command]
async fn activate_document(state: tauri::State<'_, Backend>, document_id: String) -> Reply<()> {
    workspace_work(&state, move |app| app.activate(&document_id).map_err(error)).await
}
#[tauri::command]
async fn close_document(
    state: tauri::State<'_, Backend>,
    ai: tauri::State<'_, ai::AiBackend>,
    document_id: String,
    discard: bool,
) -> Reply<()> {
    let closed_id = document_id.clone();
    workspace_work(&state, move |app| {
        app.close(&document_id, discard).map_err(error)
    })
    .await?;
    ai.close_document(&closed_id).await;
    Ok(())
}
#[tauri::command]
async fn review_document(
    state: tauri::State<'_, Backend>,
    kind: String,
    frame_id: String,
    layer_id: Option<String>,
) -> Reply<pixen::viewer_protocol::ViewerReview> {
    work(&state, move |app| {
        app.local_review(&kind, &frame_id, layer_id).map_err(error)
    })
    .await
}
#[tauri::command]
fn desktop_ready(app: tauri::AppHandle, state: tauri::State<'_, Backend>, preferences_ready: Option<bool>) {
    storage_migration::acknowledge(&app, preferences_ready.unwrap_or(true));
    eprintln!("pixen: viewer opened (pid {})", std::process::id());
    if state.managed {
        acknowledge(
            serde_json::json!({"opened":true,"pid":std::process::id(),"activation_requested":true}),
        );
    }
}
#[tauri::command]
fn theme_config() -> pixen::theme::ThemeConfig {
    pixen::theme::config().clone()
}
fn main() {
    let args = Args::parse();
    if let Some(directory) = &args.storage_migrate {
        if let Err(reason) = storage_migration::run_helper(directory) {
            let _ = std::fs::write(directory.join("error.txt"),format!("{reason:#}"));
            rfd::MessageDialog::new().set_title("Pixen storage migration").set_description(format!("Storage migration could not finish. Original data was preserved.\n\n{reason:#}\n\nDetails: {}",directory.display())).set_level(rfd::MessageLevel::Error).show();
        }
        return;
    }
    if let Err(error) = pixen::storage::initialize_resuming(args.storage_resume.is_some()) {
        eprintln!("Pixen storage: {error}");
        std::process::exit(1);
    }
    if args.ai_mcp_proxy {
        if let Err(e) = ai::proxy() {
            eprintln!("pixen MCP: {e}");
            std::process::exit(1);
        }
        return;
    }
    let mut application = Application::new(args.managed);
    if let Some(path) = &args.input {
        if let Err(e) = application.open(path) {
            if args.managed {
                acknowledge(serde_json::json!({"opened":false,"error":format!("{e:#}")}));
            }
            eprintln!("pixen: {e:#}");
            std::process::exit(1);
        }
    }
    let workspace = match &args.storage_resume {
        Some(directory) => match storage_migration::restore(directory) { Ok(workspace)=>workspace, Err(reason)=>{eprintln!("Cannot restore migration checkpoint: {reason:#}");std::process::exit(1);} },
        None => DocumentWorkspace::new(application),
    };
    let backend = Backend {
        app: Arc::new(Mutex::new(workspace)),
        managed: args.managed,
        migrating: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let protocol = backend.clone();
    let mut context = tauri::generate_context!();
    let paths = pixen::storage::layout().expect("Storage paths were validated at startup");
    context.config_mut().app.app_directories_override = Some(
        tauri::utils::config::AppDirectoriesOverride::Directories(tauri::utils::config::AppDirectoryOverrides {
            config: Some(paths.config.clone()), data: Some(paths.config.clone()),
            local_data: Some(paths.webview.clone()), cache: Some(paths.cache.clone()), log: Some(paths.logs.clone()),
        }),
    );
    for window in &mut context.config_mut().app.windows { window.data_directory = None; }
    let result = tauri::Builder::default().plugin(tauri_plugin_opener::init()).manage(backend).manage(ai::AiBackend::default()).manage(storage_migration::MigrationState::default()).manage(storage_migration::ResumeState(args.storage_resume.clone()))
        .invoke_handler(tauri::generate_handler![storage_migration::storage_status, storage_migration::prepare_storage_migration, storage_migration::begin_storage_migration, storage_migration::cancel_storage_migration,settings_file::read_settings_file, settings_file::write_settings_file, settings_file::read_keybindings_file, settings_file::write_keybindings_file,ai::ai_connect, ai::ai_request, ai::ai_disconnect, ai::ai_models, ai::ai_test_provider, ai::ai_drop_files, export::choose_export_folder, export::export_document, sample_color, snapshot, read_cel, paint_pixels, edit_structure, edit_history, open_document, create_document, activate_document, close_document, resize_document, toggle_view, follow_target, save_document, configure_recovery, render, review_document, desktop_ready, theme_config])
        .setup(move |app| {
            if protocol.managed {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    for line in std::io::stdin().lock().lines() {
                        let Ok(line) = line else { break; };
                        if line == "activate" {
                            if let Some(window) = handle.get_webview_window("main") {
                                let _ = window.show(); let _ = window.unminimize(); let _ = window.set_focus();
                                acknowledge(serde_json::json!({"opened":true,"pid":std::process::id(),"activation_requested":true,"focused":window.is_focused().unwrap_or(false)}));
                            }
                        } else if let Ok(mut app) = protocol.app.lock() {
                            if let Err(e) = app.active().message(&line) { eprintln!("pixen: ignored invalid preview message: {e}"); }
                            let _ = handle.emit("pixen:update", ());
                        }
                    }
                    if let Some(window) = handle.get_webview_window("main") { let _ = window.close(); } else { handle.exit(0); }
                });
            }
            Ok(())
        }).build(context);
    let result = result.map(|app| {
        app.run(|handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                tauri::async_runtime::block_on(handle.state::<ai::AiBackend>().shutdown());
            }
        })
    });
    if let Err(e) = result {
        if args.managed {
            acknowledge(serde_json::json!({"opened":false,"error":e.to_string()}));
        }
        eprintln!("pixen: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    #[test]
    fn desktop_arguments_are_separate_from_headless_commands() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("[INPUT]"));
        assert!(!help.contains("validate"));
        assert!(Args::try_parse_from(["pixen", "--managed"]).is_err());
        assert!(
            Args::try_parse_from(["pixen", "art.pixloom", "--managed"])
                .unwrap()
                .managed
        );
    }
}
