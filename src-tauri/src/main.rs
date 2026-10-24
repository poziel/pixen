#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use clap::Parser;
use pixloom::desktop::{
    Application, DocumentWorkspace, RenderRequest, Rendered, WorkspaceSnapshot,
};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};

#[derive(Parser)]
#[command(version, about = "Pixloom pixel art workbench")]
struct Args {
    input: Option<PathBuf>,
    #[arg(long, hide = true, requires = "input")]
    managed: bool,
}
#[derive(Clone)]
struct Backend {
    app: Arc<Mutex<DocumentWorkspace>>,
    managed: bool,
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
    let app = state.app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut app = app.lock().map_err(error)?;
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
async fn save_document(state: tauri::State<'_, Backend>, save_as: bool) -> Reply<bool> {
    if state.managed {
        return Err("managed preview is read-only".into());
    }
    work(&state, move |app| {
        let needs_path = save_as
            || app
                .snapshot()
                .map_err(error)?
                .document
                .as_ref()
                .is_none_or(|d| {
                    d.path.as_ref().is_none_or(|p| {
                        !(p.to_string_lossy().ends_with(".pixloom")
                            || p.to_string_lossy().ends_with(".pixloom.json"))
                    })
                });
        let path = if needs_path {
            rfd::FileDialog::new()
                .add_filter("Pixloom", &["pixloom"])
                .set_file_name("artwork.pixloom")
                .save_file()
        } else {
            None
        };
        if needs_path && path.is_none() {
            return Ok(false);
        }
        app.save(path.as_deref()).map_err(error)?;
        Ok(true)
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
    document_id: String,
    discard: bool,
) -> Reply<()> {
    workspace_work(&state, move |app| {
        app.close(&document_id, discard).map_err(error)
    })
    .await
}
#[tauri::command]
async fn review_document(
    state: tauri::State<'_, Backend>,
    kind: String,
    frame_id: String,
    layer_id: Option<String>,
) -> Reply<pixloom::viewer_protocol::ViewerReview> {
    work(&state, move |app| {
        app.local_review(&kind, &frame_id, layer_id).map_err(error)
    })
    .await
}
#[tauri::command]
fn desktop_ready(state: tauri::State<'_, Backend>) {
    eprintln!("pixloom: viewer opened (pid {})", std::process::id());
    if state.managed {
        acknowledge(
            serde_json::json!({"opened":true,"pid":std::process::id(),"activation_requested":true}),
        );
    }
}
#[tauri::command]
fn theme_config() -> pixloom::theme::ThemeSource {
    pixloom::theme::config().clone()
}
fn main() {
    let args = Args::parse();
    let mut application = Application::new(args.managed);
    if let Some(path) = &args.input {
        if let Err(e) = application.open(path) {
            if args.managed {
                acknowledge(serde_json::json!({"opened":false,"error":format!("{e:#}")}));
            }
            eprintln!("pixloom: {e:#}");
            std::process::exit(1);
        }
    }
    let backend = Backend {
        app: Arc::new(Mutex::new(DocumentWorkspace::new(application))),
        managed: args.managed,
    };
    let protocol = backend.clone();
    let result = tauri::Builder::default().plugin(tauri_plugin_opener::init()).manage(backend)
        .invoke_handler(tauri::generate_handler![snapshot, open_document, create_document, activate_document, close_document, resize_document, toggle_view, follow_target, save_document, render, review_document, desktop_ready, theme_config])
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
                            if let Err(e) = app.active().message(&line) { eprintln!("pixloom: ignored invalid preview message: {e}"); }
                            let _ = handle.emit("pixloom:update", ());
                        }
                    }
                    handle.exit(0);
                });
            }
            Ok(())
        }).run(tauri::generate_context!());
    if let Err(e) = result {
        if args.managed {
            acknowledge(serde_json::json!({"opened":false,"error":e.to_string()}));
        }
        eprintln!("pixloom: {e}");
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
        assert!(Args::try_parse_from(["pixloom", "--managed"]).is_err());
        assert!(
            Args::try_parse_from(["pixloom", "art.pixloom", "--managed"])
                .unwrap()
                .managed
        );
    }
}
