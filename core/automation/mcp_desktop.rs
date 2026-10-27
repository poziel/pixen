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
    recovery: Arc<Mutex<Option<crate::recovery::RecoveryWorker>>>,
}

impl Desktop {
    pub fn restore_checkpoint(&self, bytes: &[u8]) -> Result<()> {
        self.server.state.lock().unwrap().session.restore_document(bytes)
    }
    pub fn save_baseline(&self) -> Result<Option<Vec<u8>>> {
        self.server.state.lock().unwrap().session.save_baseline()
    }
    pub fn restore_save_baseline(&self, destination: Option<&Path>, expected: Option<Vec<u8>>) -> Result<()> {
        self.server.state.lock().unwrap().session.restore_save_baseline(destination, expected)
    }
    pub fn export_document(&self) -> Result<crate::schema::Document> {
        Ok(self
            .server
            .state
            .lock()
            .unwrap()
            .session
            .current_document()?
            .clone())
    }
    pub fn enable_background_recovery(&self) {
        let mut session_state = self.server.state.lock().unwrap();
        let mut recovery = self.recovery.lock().unwrap();
        if recovery.is_some() {
            return;
        }
        session_state.session.defer_recovery();
        let worker = crate::recovery::RecoveryWorker::default();
        *recovery = Some(worker.clone());
        drop(recovery);
        drop(session_state);
        let session = Arc::downgrade(&self.server.state);
        // Recovery remains active even when the frontend is minimized or its timers are throttled.
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if worker.cancelled() {
                    break;
                }
                let Some(session) = session.upgrade() else {
                    break;
                };
                let snapshot = session
                    .try_lock()
                    .ok()
                    .and_then(|state| state.session.revision_snapshot().ok());
                if let Some(snapshot) = snapshot {
                    worker.schedule(snapshot, false);
                }
            }
        });
    }
    pub fn configure_recovery(&self, seconds: u64) -> Result<()> {
        if let Some(worker) = self.recovery.lock().unwrap().as_ref() {
            worker.configure(seconds)?;
        }
        Ok(())
    }
    pub fn recovery_status(&self) -> Option<crate::recovery::RecoveryStatus> {
        self.recovery.lock().unwrap().as_ref().map(|w| w.status())
    }
    pub fn cancel_recovery(&self) {
        if let Some(worker) = self.recovery.lock().unwrap().as_ref() {
            worker.cancel();
        }
    }
    pub fn recover_now(&self) -> Result<bool> {
        let snapshot = self
            .server
            .state
            .lock()
            .unwrap()
            .session
            .revision_snapshot()?;
        Ok(self
            .recovery
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|worker| worker.schedule(snapshot, true)))
    }
    pub fn prepare_save(&self, path: Option<&Path>) -> Result<crate::session::SaveSnapshot> {
        self.server.state.lock().unwrap().session.prepare_save(path)
    }
    pub fn finish_save(&self, saved: &crate::session::SavedSnapshot) -> Result<()> {
        self.server.state.lock().unwrap().session.finish_save(saved)
    }
    pub fn read_cel(&self, frame: &str, layer: &str) -> Result<crate::desktop::CelPixels> {
        let state = self.server.state.lock().unwrap();
        let doc = state.session.current_document()?;
        let f = crate::edit::frame_index(&doc, frame)?;
        let l = doc
            .layers
            .iter()
            .position(|l| l.id == layer)
            .context("unknown layer")?;
        let grid = crate::edit::grid(&doc, l, f)?;
        let palette = doc
            .palette
            .iter()
            .map(|c| crate::schema::decode_color(c))
            .collect::<Result<Vec<_>>>()?;
        Ok(crate::desktop::CelPixels {
            cel_opacity: doc.layers[l].timeline[f].opacity,
            pixels: grid
                .into_iter()
                .flatten()
                .flat_map(|i| palette[i as usize])
                .collect(),
            opacity: doc.layers[l].opacity * doc.layers[l].timeline[f].opacity,
        })
    }
    pub fn paint(
        &self,
        revision: u64,
        frame: &str,
        layer: &str,
        pixels: Vec<crate::desktop::PaintPixel>,
    ) -> Result<()> {
        use crate::edit::{DrawOperation, Drawing, Operation, Pixel};
        let mut state = self.server.state.lock().unwrap();
        let id = state.session.active_id().context("no artwork")?.to_owned();
        let doc = state.session.current_document()?;
        anyhow::ensure!(
            !pixels.is_empty()
                && pixels.len() <= (doc.canvas.width as usize * doc.canvas.height as usize),
            "invalid stroke size"
        );
        let mut palette = doc
            .palette
            .iter()
            .map(|c| crate::schema::decode_color(c))
            .collect::<Result<Vec<_>>>()?;
        let mut indices = palette
            .iter()
            .enumerate()
            .map(|(i, c)| (*c, i as u32))
            .collect::<std::collections::HashMap<_, _>>();
        let mut colors = Vec::new();
        let mut drawing = Vec::with_capacity(pixels.len());
        for p in pixels {
            anyhow::ensure!(
                p.x < doc.canvas.width && p.y < doc.canvas.height,
                "pixel outside canvas"
            );
            let color = crate::schema::decode_color(&p.color)?;
            let index = *indices.entry(color).or_insert_with(|| {
                let i = palette.len() as u32;
                palette.push(color);
                colors.push(p.color);
                i
            });
            drawing.push(Pixel {
                x: p.x.into(),
                y: p.y.into(),
                palette_index: index,
            });
        }
        let mut ops = Vec::new();
        if !colors.is_empty() {
            ops.push(Operation::Palette(
                crate::edit_extra::PaletteOperation::AppendColors { colors },
            ));
        }
        ops.push(Operation::Draw(DrawOperation {
            frame_id: frame.into(),
            layer_id: layer.into(),
            drawing: Drawing::SetPixels { pixels: drawing },
        }));
        state.session.apply(&id, revision, ops)?;
        Ok(())
    }
    pub fn edit_structure(
        &self,
        revision: u64,
        operation: crate::desktop::StructureOperation,
    ) -> Result<()> {
        let mut state = self.server.state.lock().unwrap();
        let id = state.session.active_id().context("no artwork")?.to_owned();
        use crate::desktop::StructureOperation as S;
        let operations = match operation {
            S::Batch(ops) => ops,
            op => vec![op],
        };
        anyhow::ensure!(
            !operations.is_empty() && operations.len() <= 4096,
            "invalid structure batch size"
        );
        let operations = operations
            .into_iter()
            .map(|op| match op {
                S::Layer(op) => Ok(crate::edit::Operation::Layer(op)),
                S::Frame(op) => Ok(crate::edit::Operation::Frame(op)),
                S::Batch(_) => anyhow::bail!("nested structure batches are not supported"),
            })
            .collect::<Result<Vec<_>>>()?;
        state.session.apply(&id, revision, operations)?;
        Ok(())
    }
    pub fn history(&self, revision: u64, redo: bool) -> Result<()> {
        let mut state = self.server.state.lock().unwrap();
        let id = state.session.active_id().context("no artwork")?.to_owned();
        if redo {
            state.session.redo(&id, revision)?;
        } else {
            state.session.undo(&id, revision)?;
        }
        Ok(())
    }
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
        if self.recovery.lock().unwrap().is_some() {
            return Ok(());
        }
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
                    name.ends_with(".pixloom") || name.ends_with(".pixen.json")
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
            recovery: Default::default(),
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
                serde_json::json!({"project_id":null,"message":"No artwork yet. Use create_project; its destination is managed by Pixen."}),
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

    /// Render from immutable in-memory state; recovery I/O is independent.
    pub fn snapshot(&self, previous: Option<u64>) -> Option<(u64, crate::model::Sprite, PathBuf)> {
        let snapshot = self
            .server
            .state
            .try_lock()
            .ok()?
            .session
            .revision_snapshot()
            .ok()?;
        if let Some(worker) = self.recovery.lock().unwrap().as_ref() {
            worker.schedule(snapshot.clone(), false);
        }
        if previous == Some(snapshot.revision) {
            return None;
        }
        let sprite = crate::schema::compile((*snapshot.document).clone()).ok()?;
        Some((snapshot.revision, sprite, snapshot.recovery))
    }
}

/// Tools exposed by the document-bound app bridge; also used for provider allowlists.
pub fn artwork_tool_names() -> &'static [&'static str] {
    &[
        "create_project",
        "get_project_info",
        "get_pixels",
        "get_preview",
        "apply_edits",
        "resize_canvas",
        "draw",
        "edit_layer",
        "edit_frame",
        "edit_palette",
        "copy_region",
        "transform_region",
        "undo",
        "redo",
        "inspect_project",
        "get_contact_sheet",
        "get_onion_skin",
        "compare_frames",
        "inspect_layer",
        "report_activity",
        "get_review_sheet",
        "manage_checkpoint",
        "compare_checkpoint",
        "analyze_sprite",
        "validate_artwork",
        "audit_animation",
    ]
}
fn allowed(name: &str) -> bool {
    artwork_tool_names().contains(&name)
}

impl ServerHandler for Desktop {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("pixen-desktop", env!("CARGO_PKG_VERSION")))
            .with_instructions(authoring_instructions("Edit the active Pixen canvas using explicit stable frame/layer IDs and revision-checked transactions. The desktop owns saving, project lifetime and the visible canvas. Do not finish, export, open another project or viewer. create_project creates a recoverable temporary project at the app-managed destination. Use get_project_info for current revision, inspect_project for actual PNG evidence, get_contact_sheet to inspect distinct animation poses, and report_activity for concise progress. Compose deliberate pixel clusters, editable layers and a controlled #RRGGBBAA palette; inspect and refine the result. Undo/redo and checkpoints are session-local. Review tools measure evidence, not artistic quality."))
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
                "This action is managed by the Pixen application.",
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
