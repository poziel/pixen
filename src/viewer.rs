//! Public desktop launch helpers; no UI toolkit is linked into headless tools.
pub use crate::viewer_protocol::{Activity, ViewerProcess};
pub mod review_ui {
    pub use crate::viewer_protocol::ViewerReview;
}
use anyhow::{Context, Result, ensure};
use std::{path::Path, process::Command};
pub fn launch(path: Option<&Path>, managed: bool) -> Result<()> {
    let executable = std::env::current_exe()?.with_file_name(if cfg!(windows) {
        "pixloom.exe"
    } else {
        "pixloom"
    });
    ensure!(
        executable.is_file(),
        "Pixloom desktop missing; build it with bun run tauri build --no-bundle"
    );
    let mut command = Command::new(executable);
    if let Some(path) = path {
        command.arg(path);
    }
    if managed {
        command.arg("--managed");
    }
    let status = command.status().context("launch Pixloom desktop")?;
    ensure!(status.success(), "Pixloom desktop exited with {status}");
    Ok(())
}
pub fn run(path: &Path) -> Result<()> {
    launch(Some(path), true)
}
pub fn inspect(path: &Path) -> Result<()> {
    launch(Some(path), false)
}
