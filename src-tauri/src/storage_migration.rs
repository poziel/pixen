//! Storage migration is prepared while the app is running and copied only after it exits.
use crate::{Backend, Reply, error};
use anyhow::{Context, Result, ensure};
use pixen::{
    desktop::{DocumentWorkspace, WorkspaceCheckpoint},
    storage::{
        self, Layout, Mode,
        transfer::{self, Plan, Transaction},
    },
};
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Default)]
pub struct MigrationState(pub Mutex<Option<PathBuf>>);
#[derive(Default)]
pub struct ResumeState(pub Option<PathBuf>);
#[derive(Serialize)]
pub struct Status {
    pub mode: Mode,
    pub paths: Layout,
    pub managed: bool,
}

#[tauri::command]
pub fn storage_status(state: tauri::State<'_, Backend>) -> Reply<Status> {
    Ok(Status {
        mode: storage::mode().map_err(error)?,
        paths: storage::layout().map_err(error)?,
        managed: state.managed,
    })
}
fn migration_directory(path: &Path) -> Result<PathBuf> {
    let root = storage::installation()?.join(".pixen-migrations");
    let resolved = fs::canonicalize(path)?;
    ensure!(
        resolved.parent() == Some(fs::canonicalize(root)?.as_path()),
        "Invalid storage migration directory"
    );
    Ok(resolved)
}
fn load_plan(directory: &Path) -> Result<Plan> {
    let bytes = pixen::output::read_bounded(&directory.join("plan.json"), 1024 * 1024)?;
    let plan: Plan = serde_json::from_slice(&bytes)?;
    let exe = std::env::current_exe()?;
    plan.validate(
        &storage::layout_for(&exe, plan.from)?,
        &storage::layout_for(&exe, plan.to)?,
    )?;
    Ok(plan)
}
fn probe(destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("Storage destination has no parent")?;
    fs::create_dir_all(parent)?;
    let _probe = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Storage destination is not writable: {}", parent.display()))?;
    Ok(())
}
#[tauri::command]
pub async fn prepare_storage_migration(
    app: tauri::AppHandle,
    state: tauri::State<'_, Backend>,
    migration: tauri::State<'_, MigrationState>,
    target: Mode,
) -> Reply<String> {
    if state.managed {
        return Err("Change storage from the standalone editor, after closing MCP previews".into());
    }
    if target == storage::mode().map_err(error)? {
        return Err("This storage mode is already active".into());
    }
    if migration.0.lock().map_err(error)?.is_some() {
        return Err("A storage migration is already pending".into());
    }
    if state
        .migrating
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A storage migration is already being prepared".into());
    }
    // Detach any app-owned provider/MCP child before reserving the installation.
    app.state::<crate::ai::AiBackend>().shutdown().await;
    if let Err(reason) = storage::reserve_migration() {
        state.migrating.store(false, Ordering::SeqCst);
        return Err(error(reason));
    }
    let result = (|| -> Result<PathBuf> {
        let exe = std::env::current_exe()?;
        let from = storage::layout()?;
        let to = storage::layout_for(&exe, target)?;
        ensure!(
            storage::mode_for(&exe)? == from.mode,
            "The marker changed while Pixen was running. Restart before changing storage."
        );
        let folder = storage::installation()?;
        let _write_probe = tempfile::NamedTempFile::new_in(&folder)
            .context("The application folder must be writable to change its portable marker")?;
        let id = uuid::Uuid::new_v4().to_string();
        let directory = folder.join(".pixen-migrations").join(&id);
        fs::create_dir_all(&directory)?;
        let marker = match fs::read(folder.join("portable")) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let plan = Plan::new(id, &from, &to, marker);
        for entry in &plan.entries {
            probe(&entry.destination)?;
        }
        let checkpoint = state
            .app
            .lock()
            .map_err(|_| anyhow::anyhow!("Document workspace unavailable"))?
            .checkpoint()?;
        let bytes = serde_json::to_vec(&checkpoint)?;
        ensure!(
            bytes.len() as u64 <= pixen::output::MAX_OUTPUT_BYTES,
            "Open artwork is too large to checkpoint. Save and close some documents before changing storage."
        );
        pixen::output::Prepared::new(&directory.join("workspace.json"), |file| {
            use std::io::Write;
            file.write_all(&bytes)?;
            Ok(())
        })?
        .commit(true)?;
        transfer::write_json(&directory.join("plan.json"), &plan)?;
        Ok(directory)
    })();
    match result {
        Ok(directory) => {
            *migration.0.lock().map_err(error)? = Some(directory.clone());
            Ok(directory.to_string_lossy().into_owned())
        }
        Err(reason) => {
            state.migrating.store(false, Ordering::SeqCst);
            storage::cancel_migration();
            Err(error(reason))
        }
    }
}
#[tauri::command]
pub fn cancel_storage_migration(
    state: tauri::State<'_, Backend>,
    migration: tauri::State<'_, MigrationState>,
) {
    if let Ok(mut pending) = migration.0.lock() {
        pending.take();
    }
    state.migrating.store(false, Ordering::SeqCst);
    storage::cancel_migration();
}
fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}
#[tauri::command]
pub fn begin_storage_migration(
    app: tauri::AppHandle,
    state: tauri::State<'_, MigrationState>,
    directory: String,
) -> Reply<()> {
    let expected = state
        .0
        .lock()
        .map_err(error)?
        .clone()
        .ok_or("No prepared storage migration")?;
    if expected != PathBuf::from(&directory) {
        return Err("Migration ticket does not match".into());
    }
    let mut command = Command::new(std::env::current_exe().map_err(error)?);
    command.arg("--storage-migrate").arg(directory);
    hidden(&mut command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(error)?;
    app.exit(0);
    Ok(())
}
pub fn restore(directory: &Path) -> Result<DocumentWorkspace> {
    let directory = migration_directory(directory)?;
    let _plan = load_plan(&directory)?;
    let bytes = pixen::output::read_bounded(
        &directory.join("workspace.json"),
        pixen::output::MAX_OUTPUT_BYTES,
    )?;
    DocumentWorkspace::restore_checkpoint(serde_json::from_slice::<WorkspaceCheckpoint>(&bytes)?)
}
pub fn acknowledge(app: &tauri::AppHandle, ready: bool) {
    if ready && let Some(directory) = &app.state::<ResumeState>().0 {
        let _ = fs::write(directory.join("ready"), b"ready");
    }
}
fn start(directory: &Path) -> Result<std::process::Child> {
    let ready = directory.join("ready");
    if ready.exists() {
        fs::remove_file(ready)?;
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("restart.log"))?;
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("--storage-resume").arg(directory);
    hidden(&mut command);
    Ok(command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()?)
}
fn await_ready(child: &mut std::process::Child, directory: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        if directory.join("ready").is_file() {
            return Ok(());
        }
        ensure!(
            child.try_wait()?.is_none(),
            "The restarted application exited before becoming ready"
        );
        thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("The restarted application did not become ready in time")
}
pub fn run_helper(directory: &Path) -> Result<()> {
    let directory = migration_directory(directory)?;
    let plan = load_plan(&directory)?;
    let folder = storage::installation()?;
    let gate = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(folder.join(".pixen-migration-gate"))?;
    gate.try_lock()
        .context("Another storage migration is already running")?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .open(folder.join(".pixen-storage.lock"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while lease.try_lock().is_err() {
        ensure!(
            Instant::now() < deadline,
            "Pixen did not shut down; storage was not changed"
        );
        thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        storage::mode_for(&std::env::current_exe()?)? == plan.from,
        "Storage marker changed during migration"
    );
    // WebView children can outlive the parent window for a short shutdown period.
    thread::sleep(Duration::from_secs(2));
    let mut transaction = match Transaction::stage(&plan) {
        Ok(transaction) => transaction,
        Err(reason) => {
            lease.unlock()?;
            let _ = start(&directory);
            return Err(reason);
        }
    };
    let commit = transaction.commit().and_then(|_| {
        transfer::write_marker(&folder, plan.to).context("Cannot update portable marker")
    });
    if let Err(reason) = commit {
        let rollback = transaction.rollback();
        let marker = transfer::restore_marker(&folder, plan.marker.as_deref());
        lease.unlock()?;
        let _ = start(&directory);
        rollback?;
        marker?;
        return Err(reason);
    }
    lease.unlock()?;
    let mut child = match start(&directory) {
        Ok(child) => child,
        Err(reason) => {
            let _ = lease.lock();
            let rollback = transaction.rollback();
            let marker = transfer::restore_marker(&folder, plan.marker.as_deref());
            let _ = lease.unlock();
            let _ = start(&directory);
            rollback?;
            marker?;
            return Err(reason);
        }
    };
    if let Err(reason) = await_ready(&mut child, &directory) {
        let _ = child.kill();
        let _ = child.wait();
        lease.lock()?;
        let rollback = transaction.rollback();
        let marker = transfer::restore_marker(&folder, plan.marker.as_deref());
        lease.unlock()?;
        let _ = start(&directory);
        rollback?;
        marker?;
        return Err(reason);
    }
    fs::write(
        directory.join("complete"),
        b"Migration complete. Original sources and destination backups were retained.",
    )?;
    Ok(())
}
