//! Reopen artwork after a controlled storage restart without saving over project files.
use super::*;

#[derive(Serialize, Deserialize)]
pub struct ArtworkCheckpoint {
    document: crate::schema::Document,
    source: Option<PathBuf>,
    destination: Option<PathBuf>,
    dirty: bool,
    saved_bytes: Option<Vec<u8>>,
}
#[derive(Serialize, Deserialize)]
pub struct WorkspaceCheckpoint {
    documents: Vec<ArtworkCheckpoint>,
    active: Option<usize>,
}
impl DocumentWorkspace {
    pub fn checkpoint(&mut self) -> Result<WorkspaceCheckpoint> {
        ensure!(
            !self.current.managed,
            "Storage cannot change from an MCP-managed preview"
        );
        let snapshot = self.snapshot()?;
        let active = snapshot
            .documents
            .iter()
            .position(|doc| Some(&doc.id) == snapshot.active_document.as_ref());
        let mut documents = Vec::new();
        for entry in snapshot.documents {
            let app = self.document_mut(&entry.id)?;
            ensure!(!app.saving, "Wait for the current artwork save to finish");
            let desktop = app.ai_desktop()?;
            documents.push(ArtworkCheckpoint {
                document: desktop.export_document()?,
                source: app.source.clone(),
                destination: app.destination.clone(),
                dirty: entry.dirty,
                saved_bytes: desktop.save_baseline()?,
            });
        }
        Ok(WorkspaceCheckpoint { documents, active })
    }
    pub fn restore_checkpoint(checkpoint: WorkspaceCheckpoint) -> Result<Self> {
        let mut workspace = Self::new(Application::new(false));
        let mut active_id = None;
        for (index, entry) in checkpoint.documents.into_iter().enumerate() {
            let mut app = Application::new(false);
            // The checkpoint is authoritative, even if an external file changed or vanished.
            app.create(entry.document.canvas.width, entry.document.canvas.height)?;
            let desktop = app.ai_desktop()?;
            desktop.restore_checkpoint(&serde_json::to_vec(&entry.document)?)?;
            desktop.restore_save_baseline(entry.destination.as_deref(), entry.saved_bytes)?;
            app.refresh_document();
            app.source = entry
                .source
                .filter(|path| Application::file_stamp(path).is_ok());
            app.stamp = app
                .source
                .as_deref()
                .and_then(|path| Application::file_stamp(path).ok());
            app.destination = entry.destination;
            app.saved_revision = if entry.dirty { u64::MAX } else { app.revision };
            workspace.install(app);
            if checkpoint.active == Some(index) {
                active_id = workspace.current_id.clone();
            }
        }
        if let Some(id) = active_id {
            workspace.activate(&id)?;
        }
        Ok(workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_reopens_even_when_the_original_parent_directory_is_gone() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("disconnected");
        std::fs::create_dir(&folder).unwrap();
        let project = folder.join("original.pixloom");
        let mut app = Application::new(false);
        app.create(4, 4).unwrap();
        app.save(Some(&project)).unwrap();
        let mut workspace = DocumentWorkspace::new(app);
        workspace.active().resize(7, 7).unwrap();
        let checkpoint = workspace.checkpoint().unwrap();
        std::fs::remove_file(&project).unwrap();
        std::fs::remove_dir(&folder).unwrap();
        let mut restored = DocumentWorkspace::restore_checkpoint(checkpoint).unwrap();
        assert_eq!(
            restored.snapshot().unwrap().active.document.unwrap().width,
            7
        );
        assert!(
            restored
                .active()
                .destination
                .as_ref()
                .unwrap()
                .ends_with("original.pixloom")
        );
    }
    #[test]
    fn checkpoint_remains_restorable_when_the_saved_file_is_corrupted_externally() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("original.pixloom");
        let mut app = Application::new(false);
        app.create(4, 4).unwrap();
        app.save(Some(&project)).unwrap();
        let mut workspace = DocumentWorkspace::new(app);
        workspace.active().resize(7, 7).unwrap();
        let checkpoint = workspace.checkpoint().unwrap();
        std::fs::write(&project, b"externally corrupted").unwrap();
        let mut restored = DocumentWorkspace::restore_checkpoint(checkpoint).unwrap();
        assert_eq!(
            restored.snapshot().unwrap().active.document.unwrap().width,
            7
        );
        assert_eq!(std::fs::read(&project).unwrap(), b"externally corrupted");
        assert!(restored.active().save(None).is_err());
        assert_eq!(std::fs::read(&project).unwrap(), b"externally corrupted");
    }
    #[test]
    fn restores_untitled_and_changed_artwork_without_modifying_saved_projects() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("original.pixloom");
        let mut app = Application::new(false);
        app.create(4, 4).unwrap();
        app.save(Some(&project)).unwrap();
        let original = std::fs::read(&project).unwrap();
        let mut workspace = DocumentWorkspace::new(app);
        workspace.active().resize(8, 8).unwrap();
        workspace.create(6, 6).unwrap();
        let checkpoint = workspace.checkpoint().unwrap();
        let mut restored = DocumentWorkspace::restore_checkpoint(checkpoint).unwrap();
        let snapshot = restored.snapshot().unwrap();
        assert_eq!(snapshot.documents.len(), 2);
        assert!(snapshot.documents[0].dirty);
        assert_eq!(
            snapshot.documents[0].path.as_deref(),
            Some(std::fs::canonicalize(&project).unwrap().as_path())
        );
        assert_eq!(snapshot.documents[1].path, None);
        assert_eq!(snapshot.active.document.unwrap().source_width, 6);
        assert_eq!(std::fs::read(&project).unwrap(), original);
        let first = snapshot.documents[0].id.clone();
        restored.activate(&first).unwrap();
        assert_eq!(
            restored
                .snapshot()
                .unwrap()
                .active
                .document
                .unwrap()
                .source_width,
            8
        );
        assert!(restored.active().history(false).is_err());
        restored.active().save(None).unwrap();
        assert_ne!(std::fs::read(project).unwrap(), original);
    }
}
