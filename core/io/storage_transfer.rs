//! Copy-first directory transactions. Source data and conflicting destinations are retained.
use super::{Layout, Mode};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub source: PathBuf,
    pub destination: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub from: Mode,
    pub to: Mode,
    pub entries: Vec<Entry>,
    pub marker: Option<Vec<u8>>,
}
impl Plan {
    pub fn new(id: String, from: &Layout, to: &Layout, marker: Option<Vec<u8>>) -> Self {
        let pairs = [
            (&from.config, &to.config),
            (&from.recovery, &to.recovery),
            (&from.themes, &to.themes),
            (&from.webview, &to.webview),
            (&from.cache, &to.cache),
            (&from.logs, &to.logs),
            (&from.temp, &to.temp),
        ];
        let entries = pairs
            .into_iter()
            .filter(|(source, destination)| source != destination)
            .map(|(source, destination)| Entry {
                source: source.clone(),
                destination: destination.clone(),
            })
            .collect();
        Self {
            id,
            from: from.mode,
            to: to.mode,
            entries,
            marker,
        }
    }
    pub fn validate(&self, from: &Layout, to: &Layout) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() < 80
                && self
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "Invalid migration identifier"
        );
        let expected = Self::new(self.id.clone(), from, to, self.marker.clone());
        ensure!(
            self.entries.len() == expected.entries.len(),
            "Invalid migration paths"
        );
        for (entry, valid) in self.entries.iter().zip(expected.entries) {
            ensure!(
                entry.source == valid.source && entry.destination == valid.destination,
                "Migration paths do not match the storage modes"
            );
        }
        Ok(())
    }
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    crate::output::Prepared::new(path, |file| {
        serde_json::to_writer_pretty(file, value)?;
        Ok(())
    })?
    .commit(true)
}
pub fn write_marker(folder: &Path, mode: Mode) -> Result<()> {
    let marker = folder.join("portable");
    if mode == Mode::Default {
        if marker.exists() {
            fs::remove_file(marker)?;
        }
    } else {
        let text = if mode == Mode::Portable {
            ""
        } else {
            "semi-portable\n"
        };
        crate::output::Prepared::new(&marker, |file| {
            file.write_all(text.as_bytes())?;
            Ok(())
        })?
        .commit(true)?;
    }
    Ok(())
}
pub fn restore_marker(folder: &Path, content: Option<&[u8]>) -> Result<()> {
    let marker = folder.join("portable");
    if let Some(content) = content {
        crate::output::Prepared::new(&marker, |file| {
            file.write_all(content)?;
            Ok(())
        })?
        .commit(true)?;
    } else if marker.exists() {
        fs::remove_file(marker)?;
    }
    Ok(())
}
fn reject_link(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "Storage migration cannot follow symbolic links: {}",
        path.display()
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Storage migration cannot follow junctions: {}",
            path.display()
        );
    }
    Ok(())
}
fn equal(a: &Path, b: &Path) -> Result<bool> {
    let mut left = fs::File::open(a)?;
    let mut right = fs::File::open(b)?;
    if left.metadata()?.len() != right.metadata()?.len() {
        return Ok(false);
    }
    let (mut x, mut y) = ([0; 65536], [0; 65536]);
    loop {
        let a = left.read(&mut x)?;
        let b = right.read(&mut y)?;
        if a != b || x[..a] != y[..b] {
            return Ok(false);
        }
        if a == 0 {
            return Ok(true);
        }
    }
}
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    reject_link(source)?;
    if source.is_file() {
        fs::create_dir_all(target.parent().context("No destination parent")?)?;
        let mut last = None;
        for _ in 0..30 {
            match fs::copy(source, target)
                .map_err(anyhow::Error::from)
                .and_then(|_| {
                    ensure!(
                        equal(source, target)?,
                        "Storage file changed during copying"
                    );
                    Ok(())
                }) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last = Some(error);
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
        return Err(last.unwrap()).with_context(|| format!("Could not copy {}", source.display()));
    }
    ensure!(
        source.is_dir(),
        "Unsupported storage file: {}",
        source.display()
    );
    fs::create_dir_all(target)?;
    for item in fs::read_dir(source)? {
        let item = item?;
        let path = item.path();
        if let Err(reason) = copy_tree(&path, &target.join(item.file_name())) {
            // WebView removes temporary files as its final subprocesses exit.
            if reason
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                && !path.exists()
            {
                continue;
            }
            return Err(reason).with_context(|| format!("Could not copy {}", path.display()));
        }
    }
    Ok(())
}
fn rename_after_shutdown(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut error = None;
    for _ in 0..30 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(reason) => {
                error = Some(reason);
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
    Err(error.unwrap())
}
fn sibling(path: &Path, id: &str, index: usize, kind: &str) -> Result<PathBuf> {
    Ok(path
        .parent()
        .context("Storage directory has no parent")?
        .join(format!(".pixen-{kind}-{id}-{index}")))
}
struct Step {
    destination: PathBuf,
    stage: PathBuf,
    backup: PathBuf,
    committed: bool,
}
pub struct Transaction {
    steps: Vec<Step>,
}
impl Transaction {
    pub fn stage(plan: &Plan) -> Result<Self> {
        let mut transaction = Self { steps: Vec::new() };
        for (index, entry) in plan.entries.iter().enumerate() {
            if !entry.source.exists() {
                continue;
            }
            ensure!(
                !entry.destination.starts_with(&entry.source)
                    && !entry.source.starts_with(&entry.destination),
                "Overlapping storage directories"
            );
            let stage = sibling(&entry.destination, &plan.id, index, "staging")?;
            let backup = sibling(&entry.destination, &plan.id, index, "backup")?;
            ensure!(
                !stage.exists() && !backup.exists(),
                "Migration staging directory already exists"
            );
            if entry.destination.exists() {
                reject_link(&entry.destination)?;
            }
            copy_tree(&entry.source, &stage)
                .with_context(|| format!("Could not stage {}", entry.source.display()))?;
            transaction.steps.push(Step {
                destination: entry.destination.clone(),
                stage,
                backup,
                committed: false,
            });
        }
        Ok(transaction)
    }
    pub fn commit(&mut self) -> Result<()> {
        for step in &mut self.steps {
            if step.destination.exists() {
                rename_after_shutdown(&step.destination, &step.backup)
                    .with_context(|| format!("Cannot back up {}", step.destination.display()))?;
            }
            if let Err(error) = rename_after_shutdown(&step.stage, &step.destination) {
                if step.backup.exists() {
                    let _ = rename_after_shutdown(&step.backup, &step.destination);
                }
                return Err(error).with_context(|| {
                    format!(
                        "Cannot activate staged storage {} -> {}",
                        step.stage.display(),
                        step.destination.display()
                    )
                });
            }
            step.committed = true;
        }
        Ok(())
    }
    pub fn rollback(&mut self) -> Result<()> {
        for step in self.steps.iter_mut().rev().filter(|step| step.committed) {
            // Keep the failed new copy too; never recursively delete user's directories.
            rename_after_shutdown(&step.destination, &step.stage)?;
            if step.backup.exists() {
                rename_after_shutdown(&step.backup, &step.destination)?;
            }
            step.committed = false;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn plan(root: &Path) -> Plan {
        Plan {
            id: "test".into(),
            from: Mode::Default,
            to: Mode::Portable,
            marker: None,
            entries: vec![Entry {
                source: root.join("old"),
                destination: root.join("new"),
            }],
        }
    }
    #[test]
    fn conflicting_destinations_are_backed_up_and_source_is_never_modified() {
        let root = tempfile::tempdir().unwrap();
        let plan = plan(root.path());
        fs::create_dir(&plan.entries[0].source).unwrap();
        fs::create_dir(&plan.entries[0].destination).unwrap();
        fs::write(plan.entries[0].source.join("settings.json"), "current").unwrap();
        fs::write(plan.entries[0].destination.join("settings.json"), "other").unwrap();
        let mut transaction = Transaction::stage(&plan).unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("new/settings.json")).unwrap(),
            "current"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("old/settings.json")).unwrap(),
            "current"
        );
        assert_eq!(
            fs::read_to_string(root.path().join(".pixen-backup-test-0/settings.json")).unwrap(),
            "other"
        );
        transaction.rollback().unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("new/settings.json")).unwrap(),
            "other"
        );
    }
    #[test]
    fn interrupted_commit_rolls_back_earlier_directories() {
        let root = tempfile::tempdir().unwrap();
        let mut plan = plan(root.path());
        fs::create_dir(root.path().join("old")).unwrap();
        fs::write(root.path().join("old/file"), "safe").unwrap();
        plan.entries.push(Entry {
            source: root.path().join("old"),
            destination: root.path().join("blocked/new"),
        });
        let mut transaction = Transaction::stage(&plan).unwrap();
        fs::create_dir(root.path().join("blocked/new")).unwrap();
        fs::write(root.path().join("blocked/new/lock"), "existing").unwrap();
        // Occupy the planned backup path so the second rename cannot commit.
        fs::create_dir(root.path().join("blocked/.pixen-backup-test-1")).unwrap();
        fs::write(
            root.path().join("blocked/.pixen-backup-test-1/file"),
            "preserve",
        )
        .unwrap();
        assert!(transaction.commit().is_err());
        transaction.rollback().unwrap();
        assert!(!root.path().join("new").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("old/file")).unwrap(),
            "safe"
        );
    }
    #[test]
    fn marker_commit_and_rollback_preserve_empty_marker_compatibility() {
        let root = tempfile::tempdir().unwrap();
        write_marker(root.path(), Mode::Portable).unwrap();
        assert_eq!(fs::read(root.path().join("portable")).unwrap(), b"");
        write_marker(root.path(), Mode::SemiPortable).unwrap();
        assert_eq!(
            super::super::mode_for(&root.path().join("pixen.exe")).unwrap(),
            Mode::SemiPortable
        );
        restore_marker(root.path(), Some(b"")).unwrap();
        assert_eq!(
            super::super::mode_for(&root.path().join("pixen.exe")).unwrap(),
            Mode::Portable
        );
        write_marker(root.path(), Mode::Default).unwrap();
        assert!(!root.path().join("portable").exists());
    }
}
