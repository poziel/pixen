use crate::session::{CreateProject, Operation, Session};
use base64::Engine;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult, ContentBlock as Content, Implementation, ServerCapabilities, ServerConfig,
    },
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApplyEdits {
    project_id: String,
    expected_revision: u64,
    operations: Vec<Operation>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectId {
    project_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Preview {
    project_id: String,
    #[serde(default = "default_scale")]
    scale: u32,
}
fn default_scale() -> u32 {
    8
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenPreview {
    /// Supply exactly one of project_id (active editing project) or path (existing supported file).
    project_id: Option<String>,
    path: Option<PathBuf>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InspectProject {
    project_id: String,
    #[serde(default = "first_frame")]
    frame_id: String,
    #[serde(default = "default_scale")]
    scale: u32,
    crop: Option<crate::preview::Crop>,
}
fn first_frame() -> String {
    "frame-1".into()
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Revision {
    project_id: String,
    expected_revision: u64,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PixelRegion {
    project_id: String,
    frame_id: String,
    layer_id: String,
    region: crate::preview::Crop,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Draw {
    project_id: String,
    expected_revision: u64,
    frame_id: String,
    layer_id: String,
    operation: crate::edit::Drawing,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditLayer {
    project_id: String,
    expected_revision: u64,
    operation: crate::edit::LayerOperation,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditFrame {
    project_id: String,
    expected_revision: u64,
    operation: crate::edit::FrameOperation,
}

struct State {
    session: Session,
    viewers: BTreeMap<PathBuf, crate::viewer::ViewerProcess>,
    show_viewer: bool,
}
impl State {
    fn open(&mut self, path: PathBuf) -> anyhow::Result<serde_json::Value> {
        let key = preview_key(&path);
        let reuse = self.viewers.get_mut(&key).is_some_and(|v| v.alive());
        if reuse {
            match self.viewers.get_mut(&key).unwrap().activate() {
                Ok(mut reply) => {
                    reply["reused"] = true.into();
                    return Ok(reply);
                }
                Err(e) => {
                    let disconnected = e
                        .downcast_ref::<std::sync::mpsc::RecvTimeoutError>()
                        .is_some_and(|e| {
                            matches!(e, std::sync::mpsc::RecvTimeoutError::Disconnected)
                        })
                        || e.downcast_ref::<std::io::Error>().is_some_and(|e| {
                            matches!(
                                e.kind(),
                                std::io::ErrorKind::BrokenPipe
                                    | std::io::ErrorKind::ConnectionReset
                            )
                        })
                        || !self.viewers.get_mut(&key).unwrap().alive();
                    self.viewers.remove(&key);
                    if !disconnected {
                        return Err(e);
                    }
                }
            }
        }
        self.viewers.remove(&key);
        let (viewer, mut reply) = crate::viewer::ViewerProcess::open(&path)?;
        self.viewers.insert(key, viewer);
        reply["reused"] = false.into();
        Ok(reply)
    }
}
fn preview_key(path: &std::path::Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.as_os_str().to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}

#[derive(Clone)]
struct Server {
    state: Arc<Mutex<State>>,
}
fn result(value: anyhow::Result<serde_json::Value>) -> CallToolResult {
    match value {
        Ok(v) => CallToolResult::success(vec![Content::text(v.to_string())]),
        Err(e) => CallToolResult::error(vec![Content::text(format!("{e:#}"))]),
    }
}

#[tool_router]
impl Server {
    #[tool(
        description = "Create one transparent pixel-art project with one frame and layer. Palette entries are #RRGGBBAA, indexed from zero. Finish it before creating another."
    )]
    fn create_project(&self, Parameters(args): Parameters<CreateProject>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let created = state.session.create(args);
        if created.is_ok() && state.show_viewer {
            let path = state.session.recovery_path().unwrap().to_owned();
            if let Err(e) = state.open(path) {
                eprintln!("pixloom: viewer unavailable: {e:#}");
            }
        }
        result(created)
    }

    #[tool(
        description = "Apply one atomic transaction at expected_revision containing typed drawing, layer and/or frame operations. Drawing operations explicitly target frame_id and layer_id; structural operations carry the relevant stable ID. Coordinates are zero-based and strictly bounded; shapes have positive dimensions; fill is four-connected equal-index replacement. Invalid or stale batches change nothing, including undo history. One successful batch is one undo step. Returns revision, drawing changed bounds/target changes or conservative full-canvas structural bounds. No PNG is generated."
    )]
    fn apply_edits(&self, Parameters(args): Parameters<ApplyEdits>) -> CallToolResult {
        result(self.state.lock().unwrap().session.apply(
            &args.project_id,
            args.expected_revision,
            args.operations,
        ))
    }

    #[tool(
        description = "Inspect an active editing project without rendering an image: canvas dimensions, zero-based #RRGGBBAA palette, ordered layers and frames with stable IDs and durations, cel/link references, revision and undo/redo availability. IDs survive reorder and undo; viewer selection never changes editing targets."
    )]
    fn get_project_info(&self, Parameters(args): Parameters<ProjectId>) -> CallToolResult {
        result(self.state.lock().unwrap().session.info(&args.project_id))
    }

    #[tool(
        description = "Inspect at most 4096 exact uncomposited pixels in a bounded region of an explicit project/layer/frame. Returns each source x/y, zero-based palette_index and exact #RRGGBBAA value. Layer visibility/opacity and cel opacity are not applied. Positive region dimensions, zero-based coordinates; out-of-bounds regions are rejected, never clipped. Read-only."
    )]
    fn get_pixels(&self, Parameters(args): Parameters<PixelRegion>) -> CallToolResult {
        result(self.state.lock().unwrap().session.pixels(
            &args.project_id,
            &args.frame_id,
            &args.layer_id,
            args.region,
        ))
    }

    #[tool(
        description = "Draw one typed operation on an explicit project/layer/frame at expected_revision. Supports set_pixels, inclusive-endpoint Bresenham line, positive origin/dimension rectangle or ellipse (filled/outlined), four-connected equal-index fill, erase_region (transparent), clear_cel (transparent), and legacy clear with palette_index. All bounds and indices are strict; no clipping or implicit palette creation except a transparent entry if required. Ellipses test pixel centres inside the bounding ellipse; outlines are four-neighbour boundaries. Uses the same atomic executor as apply_edits and one undo step. No image is generated. Explicit link groups share edits; unlinked reused cels detach."
    )]
    fn draw(&self, Parameters(args): Parameters<Draw>) -> CallToolResult {
        result(self.state.lock().unwrap().session.apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Draw(crate::edit::DrawOperation {
                frame_id: args.frame_id,
                layer_id: args.layer_id,
                drawing: args.operation,
            })],
        ))
    }

    #[tool(
        description = "Atomically add/remove/rename/reorder a layer or update visibility/opacity, at expected_revision. Use explicit stable layer IDs; add_layer requires a new never-used ID and a name. Indices are zero-based final positions, bottom-to-top compositing order. New layers are transparent on every frame. Opacity is 0..1. Removing a layer is destructive but undoable; the last layer cannot be removed. Uses apply_edits' executor; no image is generated."
    )]
    fn edit_layer(&self, Parameters(args): Parameters<EditLayer>) -> CallToolResult {
        result(self.state.lock().unwrap().session.apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Layer(args.operation)],
        ))
    }

    #[tool(
        description = "Atomically add/remove/duplicate/reorder a frame or set its positive finite duration_ms, at expected_revision. Explicit stable frame IDs survive reorder; additions require a never-used ID. Indices are zero-based final positions. New frames are transparent. duplicate_frame copies all layer cels and clears duplicate links: editing it cannot modify its source. Removing a frame is destructive but undoable; the last frame cannot be removed. Existing tag membership is retained; non-contiguous tag results are rejected. Uses apply_edits' executor; no image is generated."
    )]
    fn edit_frame(&self, Parameters(args): Parameters<EditFrame>) -> CallToolResult {
        result(self.state.lock().unwrap().session.apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Frame(args.operation)],
        ))
    }

    #[tool(
        description = "Undo one whole committed transaction at expected_revision; one apply_edits batch is one step. Revision increases monotonically, and the live viewer updates. History is bounded to 32 snapshots/64 MiB total and is not persisted. Empty history or stale revision changes nothing; no PNG is generated."
    )]
    fn undo(&self, Parameters(args): Parameters<Revision>) -> CallToolResult {
        result(
            self.state
                .lock()
                .unwrap()
                .session
                .undo(&args.project_id, args.expected_revision),
        )
    }

    #[tool(
        description = "Redo one previously undone transaction at expected_revision. Revision increases and live preview updates. A new mutation clears redo. Empty history or stale revision changes nothing; no PNG is generated."
    )]
    fn redo(&self, Parameters(args): Parameters<Revision>) -> CallToolResult {
        result(
            self.state
                .lock()
                .unwrap()
                .session
                .redo(&args.project_id, args.expected_revision),
        )
    }

    #[tool(
        description = "Return the current composite as actual PNG image content with nearest-neighbour integer scaling (default 8), revision and original canvas dimensions."
    )]
    fn get_preview(&self, Parameters(args): Parameters<Preview>) -> CallToolResult {
        match self
            .state
            .lock()
            .unwrap()
            .session
            .preview(&args.project_id, args.scale)
        {
            Ok((png, metadata)) => CallToolResult::success(vec![
                Content::image(
                    base64::engine::general_purpose::STANDARD.encode(png),
                    "image/png",
                ),
                Content::text(metadata.to_string()),
            ]),
            Err(e) => result(Err(e)),
        }
    }

    #[tool(
        description = "Render an active editing project's frame as actual PNG image content for AI inspection. Defaults to frame-1 and scale 8. Optional crop uses zero-based source x/y and positive width/height wholly inside the canvas. Returns revision, frame_id, original dimensions, scale and source rectangle. Read-only; does not change editing or viewer selection."
    )]
    fn inspect_project(&self, Parameters(args): Parameters<InspectProject>) -> CallToolResult {
        match self.state.lock().unwrap().session.inspect(
            &args.project_id,
            &args.frame_id,
            args.crop,
            args.scale,
        ) {
            Ok((png, metadata)) => CallToolResult::success(vec![
                Content::image(
                    base64::engine::general_purpose::STANDARD.encode(png),
                    "image/png",
                ),
                Content::text(metadata.to_string()),
            ]),
            Err(e) => result(Err(e)),
        }
    }

    #[tool(
        description = "Show/restore and request foreground activation of a read-only user preview. Supply exactly one of project_id (active editing project) or path (existing Pixloom JSON/.pixloom, Pixelorama .pxo, RGBA Aseprite). Reuses an existing window for the same source; returns promptly after a bounded opening acknowledgement, never waits for user approval or closure. Existing files are never written. opened reports window initialization, focused is observed focus and activation remains subject to OS policy. Showing a window provides NO visual feedback to the AI; use inspect_project for an actual PNG image."
    )]
    fn open_preview(&self, Parameters(args): Parameters<OpenPreview>) -> CallToolResult {
        let opened = (|| -> anyhow::Result<serde_json::Value> {
            let mut state = self.state.lock().unwrap();
            let (path, mut metadata) = match (args.project_id, args.path) {
                (Some(id), None) => state.session.preview_source(&id)?,
                (None, Some(path)) => {
                    let path = path.canonicalize().map_err(|e| {
                        anyhow::anyhow!(
                            "open project {}: {e}; provide an existing supported file",
                            path.display()
                        )
                    })?;
                    let sprite = crate::preview::load(&path)?;
                    let meta = serde_json::json!({"path":path,"revision":null,"width":sprite.width,"height":sprite.height,"frame_count":sprite.frames.len()});
                    (path, meta)
                }
                _ => anyhow::bail!("supply exactly one of project_id or path"),
            };
            let reply = state.open(path)?;
            metadata
                .as_object_mut()
                .unwrap()
                .extend(reply.as_object().unwrap().clone());
            Ok(metadata)
        })();
        match opened {
            Ok(value) => result(Ok(value)),
            Err(error) => CallToolResult::error(vec![Content::text(
                serde_json::json!({"opened":false,"error":format!("{error:#}")}).to_string(),
            )]),
        }
    }

    #[tool(
        description = "Validate and save to the project's .pixloom.json output_path without overwriting any existing file. Success closes its viewer and releases the project; failure preserves the active session and viewer."
    )]
    fn finish_project(&self, Parameters(args): Parameters<ProjectId>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let key = state
            .session
            .preview_source(&args.project_id)
            .ok()
            .map(|(path, _)| preview_key(&path));
        let saved = state.session.finish(&args.project_id);
        if saved.is_ok()
            && let Some(key) = key
        {
            state.viewers.remove(&key);
        }
        result(saved)
    }
}
#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("pixloom", env!("CARGO_PKG_VERSION")))
            .with_instructions("Pixloom pixel-art authoring. Use get_project_info/get_pixels for exact inspection; edit_layer/edit_frame/draw or mixed apply_edits batches for atomic revision-checked changes; undo/redo for transaction history. inspect_project returns actual PNG content; open_preview only shows the user a read-only window. Viewer selection never changes explicit editing targets. finish_project saves without overwriting existing files.")
    }
}

pub fn run(viewer: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let server = Server {
            state: Arc::new(Mutex::new(State {
                session: Session::default(),
                viewers: BTreeMap::new(),
                show_viewer: viewer,
            })),
        };
        let service = server.serve(rmcp::transport::stdio()).await?;
        tokio::select! {
            result = service.waiting() => { result?; }
            _ = tokio::signal::ctrl_c() => {}
        }
        Ok(())
    })
}
