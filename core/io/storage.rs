#[path = "storage_transfer.rs"]
pub mod transfer;
// Storage modes are selected once per process, before opening files or WebViews.
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Default,
    Portable,
    SemiPortable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout {
    pub mode: Mode,
    pub config: PathBuf,
    pub recovery: PathBuf,
    pub temp: PathBuf,
    pub webview: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub themes: PathBuf,
}
static MODE: OnceLock<Mode> = OnceLock::new();
static LEASE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

pub fn mode_for(executable: &Path) -> io::Result<Mode> {
    let folder = executable
        .parent()
        .ok_or_else(|| io::Error::other("Executable has no parent"))?;
    let marker = folder.join("portable");
    if marker.is_dir() {
        return Ok(Mode::Default);
    }
    match std::fs::read_to_string(marker) {
        Ok(text) => match text.trim() {
            "" | "portable" => Ok(Mode::Portable),
            "semi-portable" => Ok(Mode::SemiPortable),
            _ => Err(io::Error::other(
                "The portable marker must be empty, portable, or semi-portable",
            )),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Mode::Default),
        Err(error) => Err(error),
    }
}
pub fn mode() -> io::Result<Mode> {
    if let Some(mode) = MODE.get() {
        return Ok(*mode);
    }
    let detected = mode_for(&std::env::current_exe()?)?;
    Ok(*MODE.get_or_init(|| detected))
}
pub fn installation() -> io::Result<PathBuf> {
    std::env::current_exe()?
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| io::Error::other("Executable has no parent"))
}
pub fn portable_root() -> io::Result<Option<PathBuf>> {
    Ok((mode()? != Mode::Default)
        .then(|| installation().map(|root| root.join("data")))
        .transpose()?)
}
fn environment_path(name: &str) -> io::Result<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other(format!("Missing {name} directory")))
}
pub fn layout_for(executable: &Path, mode: Mode) -> io::Result<Layout> {
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("Executable has no parent"))?;
    #[cfg(windows)]
    let (normal_config, normal_local) = (
        environment_path("APPDATA")?.join("com.poziel.pixen"),
        environment_path("LOCALAPPDATA")?.join("com.poziel.pixen"),
    );
    #[cfg(not(windows))]
    let (normal_config, normal_local) = (
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or(environment_path("HOME")?.join(".config"))
            .join("com.poziel.pixen"),
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or(environment_path("HOME")?.join(".cache"))
            .join("com.poziel.pixen"),
    );
    Ok(resolve(directory, mode, &normal_config, &normal_local))
}
fn resolve(directory: &Path, mode: Mode, normal_config: &Path, normal_local: &Path) -> Layout {
    let portable = directory.join("data");
    let durable = if mode == Mode::Default {
        normal_local.join("documents")
    } else {
        portable.clone()
    };
    let transient = if mode == Mode::Portable {
        portable.clone()
    } else {
        normal_local.to_owned()
    };
    let config = if mode == Mode::Default {
        normal_config.to_owned()
    } else {
        portable.join("config")
    };
    Layout {
        mode,
        themes: if mode == Mode::Default {
            normal_local.join("documents/themes")
        } else {
            portable.join("themes")
        },
        config,
        recovery: durable.join("recovery"),
        temp: transient.join("temp"),
        webview: transient.join("webview"),
        cache: transient.join("cache"),
        logs: transient.join("logs"),
    }
}
pub fn layout() -> io::Result<Layout> {
    layout_for(&std::env::current_exe()?, mode()?)
}

pub fn initialize() -> io::Result<()> {
    initialize_resuming(false)
}
/// Only the migration helper's resumed desktop may start while restart is being verified.
pub fn initialize_resuming(resuming: bool) -> io::Result<()> {
    let selected = mode()?;
    let layout = layout()?;
    let lock = installation()?.join(".pixen-storage.lock");
    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock)
    {
        Ok(file) => {
            file.try_lock_shared().map_err(|_| {
                io::Error::other(
                    "Pixen storage migration is in progress; try again after it finishes",
                )
            })?;
            Some(file)
        }
        Err(_) if selected == Mode::Default => None,
        Err(error) => return Err(error),
    };
    if !resuming {
        let gate = installation()?.join(".pixen-migration-gate");
        if gate.exists() {
            let probe = File::open(gate)?;
            probe.try_lock_shared().map_err(|_| {
                io::Error::other("Pixen is finishing a storage migration; try again shortly")
            })?;
        }
    }
    if selected != Mode::Default {
        for path in [
            &layout.config,
            &layout.recovery,
            &layout.temp,
            &layout.webview,
            &layout.cache,
            &layout.logs,
            &layout.themes,
        ] {
            std::fs::create_dir_all(path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("Cannot initialize storage at {}: {error}", path.display()),
                )
            })?;
        }
        let _probe = tempfile::NamedTempFile::new_in(&layout.config)?;
    }
    // Preserve the browser profile from versions that used the app-data root.
    // This relocates only its legacy directory before any WebView process starts.
    if selected != Mode::SemiPortable {
        let old = layout.webview.parent().unwrap().join("EBWebView");
        let new = layout.webview.join("EBWebView");
        if old.is_dir() && !new.exists() {
            std::fs::create_dir_all(&layout.webview)?;
            std::fs::rename(old, new)?;
        }
    }
    let _ = LEASE.set(Mutex::new(file));
    Ok(())
}
/// Upgrade this process's lease before accepting migration. Other windows/MCPs must close.
pub fn reserve_migration() -> io::Result<()> {
    let guard = LEASE
        .get()
        .ok_or_else(|| io::Error::other("Storage has not been initialized"))?
        .lock()
        .map_err(|_| io::Error::other("Storage lease is unavailable"))?;
    let file = guard.as_ref().ok_or_else(|| {
        io::Error::other(
            "The application folder is not writable. Copy Pixen to a writable folder first.",
        )
    })?;
    file.unlock()?;
    if file.try_lock().is_err() {
        file.lock_shared()?;
        return Err(io::Error::other(
            "Close other Pixen windows and MCP connections before changing storage mode",
        ));
    }
    Ok(())
}
pub fn cancel_migration() {
    if let Some(lock) = LEASE.get()
        && let Ok(guard) = lock.lock()
        && let Some(file) = guard.as_ref()
    {
        let _ = file.unlock();
        let _ = file.lock_shared();
    }
}
pub fn temporary_dir(prefix: &str) -> io::Result<tempfile::TempDir> {
    let parent = layout()?.temp;
    std::fs::create_dir_all(&parent)?;
    tempfile::Builder::new().prefix(prefix).tempdir_in(parent)
}
pub fn recovery_dir(normal_parent: &Path) -> io::Result<PathBuf> {
    if mode()? == Mode::Default {
        return Ok(normal_parent.to_owned());
    }
    let directory = layout()?.recovery;
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_modes_are_explicit_and_an_empty_marker_remains_portable() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("pixen.exe");
        std::fs::write(root.path().join("portable.txt"), "").unwrap();
        assert_eq!(mode_for(&exe).unwrap(), Mode::Default);
        std::fs::write(root.path().join("portable"), "").unwrap();
        assert_eq!(mode_for(&exe).unwrap(), Mode::Portable);
        std::fs::write(root.path().join("portable"), "semi-portable\n").unwrap();
        assert_eq!(mode_for(&exe).unwrap(), Mode::SemiPortable);
        std::fs::write(root.path().join("portable"), "typo").unwrap();
        assert!(mode_for(&exe).is_err());
    }
    #[test]
    fn semi_portable_keeps_durable_data_local_and_transient_data_on_the_computer() {
        let root = Path::new("installation");
        let config = Path::new("user-config");
        let local = Path::new("user-local");
        let semi = resolve(root, Mode::SemiPortable, config, local);
        let portable = resolve(root, Mode::Portable, config, local);
        let normal = resolve(root, Mode::Default, config, local);
        assert_eq!(semi.config, portable.config);
        assert_eq!(semi.recovery, portable.recovery);
        assert_eq!(semi.webview, normal.webview);
        assert_ne!(semi.webview, portable.webview);
        assert_eq!(normal.config, config);
        assert_ne!(semi.themes, normal.themes);
    }
    #[test]
    fn shared_installation_leases_prevent_exclusive_migration() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("lease");
        let first = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        first.lock_shared().unwrap();
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        assert!(second.try_lock().is_err());
        first.unlock().unwrap();
        assert!(second.try_lock().is_ok());
    }
}
