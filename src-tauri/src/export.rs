use super::{Backend, Reply, error, workspace_work};
use anyhow::{Context, ensure};
use pixen::desktop_export::{ExportRequest, output_names, stage_batch};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRequest {
    directory: PathBuf,
    name: String,
    exports: Vec<ExportRequest>,
}
#[tauri::command]
pub async fn choose_export_folder(state: tauri::State<'_, Backend>) -> Reply<Option<PathBuf>> {
    if state.managed {
        return Err("managed preview is read-only".into());
    }
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Export folder")
            .pick_folder()
    })
    .await
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub async fn export_document(
    state: tauri::State<'_, Backend>,
    document_id: String,
    request: BatchRequest,
) -> Reply<bool> {
    if state.managed {
        return Err("managed preview is read-only".into());
    }
    let (document, protected) = workspace_work(&state, move |workspace| {
        let snapshot = workspace.snapshot().map_err(error)?;
        if snapshot.active_document.as_deref() != Some(&document_id) {
            return Err("active document changed".into());
        }
        let protected = snapshot
            .documents
            .into_iter()
            .filter_map(|d| d.path)
            .collect::<Vec<_>>();
        Ok((
            workspace.active().export_document().map_err(error)?,
            protected,
        ))
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<bool> {
        let names = output_names(&request.name, &request.exports)?;
        let directory = request
            .directory
            .canonicalize()
            .context("export folder must exist")?;
        ensure!(directory.is_dir(), "choose an export folder");
        let destinations = names
            .iter()
            .map(|name| pixen::output::resolved(&directory.join(name)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let protected = protected
            .iter()
            .map(|p| pixen::output::resolved(p))
            .collect::<anyhow::Result<Vec<_>>>()?;
        for destination in &destinations {
            ensure!(
                destination.parent() == Some(directory.as_path()),
                "export destination must remain in the selected folder"
            );
            ensure!(
                !protected.contains(destination),
                "export must not replace an open project: {}",
                destination.display()
            );
        }
        let existing = destinations
            .iter()
            .filter(|p| p.exists())
            .collect::<Vec<_>>();
        if !existing.is_empty()
            && rfd::MessageDialog::new()
                .set_title("Replace exported files?")
                .set_description(format!(
                    "Replace {} existing {} in {}?\n\n{}",
                    existing.len(),
                    if existing.len() == 1 { "file" } else { "files" },
                    directory.display(),
                    existing
                        .iter()
                        .map(|p| p.file_name().unwrap().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("\n")
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                != rfd::MessageDialogResult::Yes
        {
            return Ok(false);
        }
        let prepared = stage_batch(&document, &request.exports, &destinations)?;
        for (written, file) in prepared.into_iter().enumerate() {
            file.commit(true).with_context(|| {
                format!(
                    "Export stopped after writing {written} of {} files",
                    destinations.len()
                )
            })?;
        }
        Ok(true)
    })
    .await
    .map_err(error)?
    .map_err(|error| format!("{error:#}"))
}
