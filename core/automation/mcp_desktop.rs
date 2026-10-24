//! The desktop owns this server; stdio clients attach through an authenticated bridge.
use super::*;
use anyhow::{Context, Result, ensure};
use rmcp::{ErrorData, RoleServer, model::*, service::RequestContext};
use serde_json::Value;
use std::path::Path;

pub enum DesktopEvent {
    Activity(crate::viewer::Activity),
    Inspection(crate::inspection::View),
    Review(crate::viewer::review_ui::ViewerReview),
}
struct TurnCheckpoint {
    token: u64,
    document: Option<Arc<tempfile::NamedTempFile>>,
}
fn store_turn_snapshot(root: &Path, bytes: &[u8]) -> Result<Arc<tempfile::NamedTempFile>> {
    use std::io::Write;
    let mut file = tempfile::Builder::new()
        .prefix(".pixloom-turn-")
        .tempfile_in(root)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(Arc::new(file))
}
#[derive(Clone)]
pub struct Desktop {
    server: Server,
    root: PathBuf,
    baseline: Arc<Mutex<Option<Vec<u8>>>>,
    events: Arc<Mutex<std::sync::mpsc::Receiver<DesktopEvent>>>,
    turns: Arc<Mutex<Vec<TurnCheckpoint>>>,
    next_turn: Arc<Mutex<u64>>,
}

impl Desktop {
    pub fn drain_events(&self) -> Vec<DesktopEvent> {
        self.events.lock().unwrap().try_iter().collect()
    }
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn can_revert(&self) -> bool {
        self.baseline.lock().unwrap().is_some()
    }

    fn save_temporary(&self, state: &mut State) -> Result<()> {
        let id = state
            .session
            .active_id()
            .context("no artwork yet")?
            .to_owned();
        let destination = crate::output::resolved(&self.root.join("artwork.pixloom"))?;
        if state.session.info(&id)?["output_path"] == serde_json::json!(destination) {
            state.session.save(&id)?;
        }
        Ok(())
    }

    pub fn save(&self) -> Result<Value> {
        let mut state = self.server.state.lock().unwrap();
        let id = state
            .session
            .active_id()
            .context("no artwork yet")?
            .to_owned();
        state.session.save(&id)
    }
    pub fn new(root: PathBuf, source: Option<&Path>) -> Result<Self> {
        Self::with_destination(root, source, false)
    }

    pub fn for_save(root: PathBuf, source: Option<&Path>) -> Result<Self> {
        Self::with_destination(root, source, true)
    }

    fn with_destination(root: PathBuf, source: Option<&Path>, keep_original: bool) -> Result<Self> {
        let mut session = Session::default();
        if let Some(source) = source {
            let original = keep_original
                && source.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy().to_ascii_lowercase();
                    name.ends_with(".pixloom") || name.ends_with(".pixloom.json")
                });
            let temporary = root.join("artwork.pixloom");
            session.open(source, Some(if original { source } else { &temporary }))?;
            if !original {
                let id = session.active_id().unwrap().to_owned();
                session.save(&id)?;
            }
        }
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        Ok(Self {
            events: Arc::new(Mutex::new(event_rx)),
            server: Server {
                state: Arc::new(Mutex::new(State {
                    session,
                    viewers: BTreeMap::new(),
                    show_viewer: false,
                    custom_activity: None,
                    last_inspection: None,
                    last_review: None,
                    desktop_events: Some(event_tx),
                })),
            },
            root,
            baseline: Default::default(),
            turns: Default::default(),
            next_turn: Default::default(),
        })
    }

    pub fn create_artwork(&self, width: u32, height: u32) -> Result<()> {
        let mut state = self.server.state.lock().unwrap();
        state.session.create(crate::session::CreateProject {
            width,
            height,
            palette: vec!["#00000000".into(), "#FFFFFFFF".into()],
            output_path: self.root.join("artwork.pixloom"),
        })?;
        self.save_temporary(&mut state)
    }

    pub fn resize_canvas(&self, width: u32, height: u32) -> Result<()> {
        let mut state = self.server.state.lock().unwrap();
        let id = state
            .session
            .active_id()
            .context("no artwork yet")?
            .to_owned();
        let revision = state.session.info(&id)?["revision"]
            .as_u64()
            .context("missing revision")?;
        state.session.resize_canvas(&id, revision, width, height)?;
        self.save_temporary(&mut state)?;
        *self.baseline.lock().unwrap() = None;
        Ok(())
    }

    pub fn begin_turn(&self) -> Result<Value> {
        let state = self.server.state.lock().unwrap();
        *self.baseline.lock().unwrap() = if state.session.active_id().is_some() {
            Some(state.session.document_snapshot()?)
        } else {
            None
        };
        match state.session.active_id() {
            Some(id) => state.session.info(id),
            None => Ok(
                serde_json::json!({"project_id":null,"message":"No artwork yet. Use create_project; its destination is managed by Pixloom."}),
            ),
        }
    }

    pub fn checkpoint_turn(&self) -> Result<u64> {
        let mut next = self.next_turn.lock().unwrap();
        let document = self
            .baseline
            .lock()
            .unwrap()
            .as_ref()
            .map(|bytes| store_turn_snapshot(&self.root, bytes))
            .transpose()?;
        *next += 1;
        self.turns.lock().unwrap().push(TurnCheckpoint {
            token: *next,
            document,
        });
        Ok(*next)
    }
    pub fn rewind_count(&self, token: u64) -> Result<usize> {
        let turns = self.turns.lock().unwrap();
        let index = turns
            .iter()
            .position(|turn| turn.token == token)
            .context("No checkpoint for this message")?;
        Ok(turns.len() - index)
    }
    pub fn rewind(&self, token: u64) -> Result<()> {
        let snapshot = {
            let turns = self.turns.lock().unwrap();
            turns
                .iter()
                .find(|turn| turn.token == token)
                .context("No checkpoint for this message")?
                .document
                .clone()
        };
        let mut state = self.server.state.lock().unwrap();
        if let Some(snapshot) = snapshot {
            state
                .session
                .restore_document(&std::fs::read(snapshot.path())?)?;
            self.save_temporary(&mut state)?;
        }
        self.turns.lock().unwrap().retain(|turn| turn.token < token);
        *self.baseline.lock().unwrap() = None;
        Ok(())
    }
    pub fn clear_turns(&self) {
        self.turns.lock().unwrap().clear();
    }

    #[cfg(test)]
    pub fn revert(&self) -> Result<()> {
        let baseline = self
            .baseline
            .lock()
            .unwrap()
            .clone()
            .context("no artwork baseline for this turn")?;
        let mut state = self.server.state.lock().unwrap();
        state.session.restore_document(&baseline)?;
        self.save_temporary(&mut state)?;
        *self.baseline.lock().unwrap() = None;
        Ok(())
    }

    pub fn save_as(&self, path: &Path) -> Result<Value> {
        let mut state = self.server.state.lock().unwrap();
        let id = state
            .session
            .active_id()
            .context("no artwork yet")?
            .to_owned();
        state.session.save_as(&id, path)
    }

    /// The GUI never waits for a tool call that is writing recovery or rendering.
    pub fn snapshot(&self, previous: Option<u64>) -> Option<(u64, crate::model::Sprite, PathBuf)> {
        let state = self.server.state.try_lock().ok()?;
        let id = state.session.active_id()?;
        let revision = state.session.info(id).ok()?["revision"].as_u64()?;
        if previous == Some(revision) {
            return None;
        }
        let path = state.session.recovery_path()?.to_owned();
        let sprite = crate::preview::load(&path).ok()?;
        Some((revision, sprite, path))
    }
}

fn allowed(name: &str) -> bool {
    matches!(
        name,
        "create_project"
            | "get_project_info"
            | "get_pixels"
            | "get_preview"
            | "apply_edits"
            | "resize_canvas"
            | "draw"
            | "edit_layer"
            | "edit_frame"
            | "edit_palette"
            | "copy_region"
            | "transform_region"
            | "undo"
            | "redo"
            | "inspect_project"
            | "get_contact_sheet"
            | "get_onion_skin"
            | "compare_frames"
            | "inspect_layer"
            | "report_activity"
            | "get_review_sheet"
            | "manage_checkpoint"
            | "compare_checkpoint"
            | "analyze_sprite"
            | "validate_artwork"
            | "audit_animation"
    )
}

impl ServerHandler for Desktop {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("pixloom-desktop", env!("CARGO_PKG_VERSION")))
            .with_instructions("Edit the active Pixloom canvas using explicit stable frame/layer IDs and revision-checked transactions. The desktop owns saving, project lifetime and the visible canvas. Do not finish, export, open another project or viewer. create_project creates a recoverable temporary project at the app-managed destination. Use get_project_info for current revision, inspect_project for actual PNG evidence, get_contact_sheet to inspect distinct animation poses, and report_activity for concise progress. Compose deliberate pixel clusters, editable layers and a controlled #RRGGBBAA palette; inspect and refine the result. Undo/redo and checkpoints are session-local. Review tools measure evidence, not artistic quality.")
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut result = self.server.list_tools(request, context).await?;
        result.tools.retain(|tool| allowed(&tool.name));
        Ok(result)
    }

    async fn call_tool(
        &self,
        mut request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if !allowed(&request.name) {
            return Ok(CallToolResult::error(vec![Content::text(
                "This action is managed by the Pixloom application.",
            )])
            .into());
        }
        if request.name == "create_project" {
            request.arguments.get_or_insert_default().insert(
                "output_path".into(),
                serde_json::json!(self.root.join("artwork.pixloom")),
            );
        }
        let creating = request.name == "create_project";
        let mutating = matches!(
            request.name.as_ref(),
            "create_project"
                | "apply_edits"
                | "resize_canvas"
                | "draw"
                | "edit_layer"
                | "edit_frame"
                | "edit_palette"
                | "copy_region"
                | "transform_region"
                | "undo"
                | "redo"
        );
        let response = self.server.call_tool(request, context).await?;
        if matches!(&response, CallToolResponse::Complete(result) if result.is_error != Some(true))
        {
            let mut state = self.server.state.lock().unwrap();
            if state.session.active_id().is_some() {
                if creating {
                    let blank = state.session.document_snapshot().ok();
                    *self.baseline.lock().unwrap() = blank.clone();
                    if let Some(turn) = self.turns.lock().unwrap().last_mut()
                        && turn.document.is_none()
                    {
                        turn.document = blank
                            .map(|bytes| store_turn_snapshot(&self.root, &bytes))
                            .transpose()
                            .map_err(|e| {
                                ErrorData::internal_error(
                                    format!("Cannot checkpoint the new artwork: {e:#}"),
                                    None,
                                )
                            })?;
                    }
                }
                if mutating && let Err(error) = self.save_temporary(&mut state) {
                    return Ok(CallToolResult::error(vec![Content::text(format!("Artwork is recovered, but the temporary container could not be updated: {error:#}"))]).into());
                }
            }
        }
        Ok(response)
    }
}

pub struct Bridge {
    pub address: std::net::SocketAddr,
    pub token: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Bridge {
    pub fn start(desktop: Desktop) -> Result<Self> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|e| anyhow::anyhow!("bridge entropy: {e}"))?;
        let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let secret = token.clone();
        let (shutdown, mut stop) = tokio::sync::oneshot::channel();
        let runtime = tokio::runtime::Runtime::new()?;
        let worker = std::thread::spawn(move || {
            runtime.block_on(async move {
            let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return; };
            loop {
                tokio::select! {
                    _ = &mut stop => break,
                    connection = listener.accept() => {
                        let Ok((socket, _)) = connection else { break; };
                        let secret = secret.clone();
                        let server = desktop.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncReadExt;
                            let mut socket = socket;
                            let mut line = [0u8; 65];
                            let auth = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read_exact(&mut line)).await;
                            if !matches!(auth, Ok(Ok(_))) || &line[..64] != secret.as_bytes() || line[64] != b'\n' { return; }
                            if let Ok(service) = server.serve(socket).await { let _ = service.waiting().await; }
                        });
                    }
                }
            }
        })
        });
        Ok(Self {
            address,
            token,
            shutdown: Some(shutdown),
            worker: Some(worker),
        })
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(stop) = self.shutdown.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn proxy(address: &str, token: &str) -> Result<()> {
    use std::io::Write;
    let address: std::net::SocketAddr = address.parse()?;
    ensure!(address.ip().is_loopback(), "desktop bridge must be local");
    let mut socket =
        std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(3))?;
    writeln!(socket, "{token}")?;
    let mut input_socket = socket.try_clone()?;
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut input_socket);
        let _ = input_socket.shutdown(std::net::Shutdown::Write);
    });
    std::io::copy(&mut socket, &mut std::io::stdout().lock())?;
    Ok(())
}

#[cfg(test)]
#[path = "mcp_desktop_tests.rs"]
mod tests;
