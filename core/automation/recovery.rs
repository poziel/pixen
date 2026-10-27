//! A desktop recovery checkpoint never holds the document/session lock while writing.
use crate::session::DocumentRevision;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Default, Serialize)]
pub struct RecoveryStatus {
    pub writing: bool,
    pub revision: Option<u64>,
    pub saved_at: Option<u64>,
    pub error: Option<String>,
}
struct State {
    status: RecoveryStatus,
    interval: Duration,
    attempted: Instant,
    cancelled: bool,
}
#[derive(Clone)]
pub struct RecoveryWorker(Arc<Mutex<State>>);
impl Default for RecoveryWorker {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(State {
            status: RecoveryStatus {
                revision: Some(0),
                ..Default::default()
            },
            interval: Duration::from_secs(30),
            attempted: Instant::now(),
            cancelled: false,
        })))
    }
}
impl RecoveryWorker {
    pub fn configure(&self, seconds: u64) -> Result<()> {
        ensure!(
            (1..=3600).contains(&seconds),
            "recovery interval must be 1–3600 seconds"
        );
        self.0.lock().unwrap().interval = Duration::from_secs(seconds);
        Ok(())
    }
    pub fn status(&self) -> RecoveryStatus {
        self.0.lock().unwrap().status.clone()
    }
    pub fn cancelled(&self) -> bool {
        self.0.lock().unwrap().cancelled
    }
    pub fn cancel(&self) {
        self.0.lock().unwrap().cancelled = true;
    }
    pub fn schedule(&self, snapshot: DocumentRevision, force: bool) -> bool {
        self.schedule_with(snapshot, force, |snapshot| {
            crate::output::Prepared::new(&snapshot.recovery, |file| {
                crate::output::write_document(file, &snapshot.document)
            })
        })
    }
    fn schedule_with(
        &self,
        snapshot: DocumentRevision,
        force: bool,
        write: impl FnOnce(&DocumentRevision) -> Result<crate::output::Prepared> + Send + 'static,
    ) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.cancelled
            || state.status.writing
            || state.status.revision == Some(snapshot.revision)
            || (!force && state.attempted.elapsed() < state.interval)
        {
            return false;
        }
        state.status.writing = true;
        state.status.error = None;
        state.attempted = Instant::now();
        let shared = self.0.clone();
        std::thread::spawn(move || {
            let prepared = write(&snapshot);
            // Never hold even the status mutex during filesystem operations.
            // Cancellation observed before replacement discards the staged file.
            let cancelled = shared.lock().unwrap().cancelled;
            let result = prepared.and_then(|prepared| {
                if cancelled {
                    return Ok(());
                }
                prepared.commit(true)
            });
            let mut state = shared.lock().unwrap();
            state.status.writing = false;
            if state.cancelled {
                return;
            }
            match result {
                Ok(()) => {
                    state.status.revision = Some(snapshot.revision);
                    state.status.saved_at = Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    );
                }
                Err(error) => {
                    state.status.error =
                        Some(format!("Recovery file could not be updated: {error}"))
                }
            }
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{CreateProject, Session};
    use serde_json::json;

    #[test]
    fn worker_writes_an_immutable_revision_without_holding_the_editor() {
        let root = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        session
            .create(CreateProject {
                width: 2,
                height: 2,
                palette: vec!["#00000000".into()],
                output_path: root.path().join("work.pixloom"),
            })
            .unwrap();
        session.defer_recovery();
        session
            .apply(
                "project-1",
                0,
                serde_json::from_value(
                    json!([{"type":"rename_layer","layer_id":"layer-1","name":"First"}]),
                )
                .unwrap(),
            )
            .unwrap();
        let snapshot = session.revision_snapshot().unwrap();
        let path = snapshot.recovery.clone();
        let worker = RecoveryWorker::default();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        assert!(
            worker.schedule_with(snapshot.clone(), true, move |snapshot| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                crate::output::Prepared::new(&snapshot.recovery, |file| {
                    crate::output::write_document(file, &snapshot.document)
                })
            })
        );
        entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        session
            .apply(
                "project-1",
                1,
                serde_json::from_value(
                    json!([{"type":"rename_layer","layer_id":"layer-1","name":"Second"}]),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(session.current_document().unwrap().layers[0].name, "Second");
        assert_eq!(snapshot.document.layers[0].name, "First");
        assert!(
            !worker.schedule(session.revision_snapshot().unwrap(), true),
            "only one writer at a time"
        );
        release_tx.send(()).unwrap();
        wait(&worker);
        assert_eq!(worker.status().revision, Some(1));
        assert_eq!(
            crate::schema::read_document(&std::fs::read(&path).unwrap())
                .unwrap()
                .layers[0]
                .name,
            "First"
        );
        assert!(worker.schedule(session.revision_snapshot().unwrap(), true));
        wait(&worker);
        assert_eq!(worker.status().revision, Some(2));
        assert!(
            !worker.schedule(session.revision_snapshot().unwrap(), true),
            "unchanged state needs no recovery write"
        );
        assert!(worker.configure(0).is_err());
        assert!(worker.configure(3601).is_err());
    }
    #[test]
    fn failed_or_cancelled_writes_preserve_previous_recovery() {
        let root = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        session
            .create(CreateProject {
                width: 1,
                height: 1,
                palette: vec!["#00000000".into()],
                output_path: root.path().join("work.pixloom"),
            })
            .unwrap();
        session.defer_recovery();
        session
            .apply(
                "project-1",
                0,
                serde_json::from_value(
                    json!([{"type":"rename_layer","layer_id":"layer-1","name":"Changed"}]),
                )
                .unwrap(),
            )
            .unwrap();
        let snapshot = session.revision_snapshot().unwrap();
        let old = std::fs::read(&snapshot.recovery).unwrap();
        let worker = RecoveryWorker::default();
        assert!(
            worker.schedule_with(snapshot.clone(), true, |_| anyhow::bail!(
                "simulated disk failure"
            ))
        );
        wait(&worker);
        assert!(
            worker
                .status()
                .error
                .unwrap()
                .contains("simulated disk failure")
        );
        assert_eq!(std::fs::read(&snapshot.recovery).unwrap(), old);
        let (tx, rx) = std::sync::mpsc::channel();
        let (go, ready) = std::sync::mpsc::channel();
        assert!(
            worker.schedule_with(snapshot.clone(), true, move |snapshot| {
                tx.send(()).unwrap();
                ready.recv().unwrap();
                crate::output::Prepared::new(&snapshot.recovery, |file| {
                    crate::output::write_document(file, &snapshot.document)
                })
            })
        );
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        worker.cancel();
        go.send(()).unwrap();
        wait(&worker);
        assert_eq!(std::fs::read(&snapshot.recovery).unwrap(), old);
    }
    fn wait(worker: &RecoveryWorker) {
        let until = Instant::now() + Duration::from_secs(5);
        while worker.status().writing {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
