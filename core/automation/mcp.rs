use crate::session::{CreateProject, Operation, Session};
#[path = "mcp_desktop.rs"]
pub mod desktop;
use base64::Engine;
pub use desktop::proxy as desktop_proxy;
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

fn authoring_instructions(context: &str) -> String {
    format!(
        "{context} Payload budget: keep each complete JSON tool call below 200 KiB (UTF-8 bytes); some AI clients reject calls around 256 KiB before they reach Pixen. Prefer rectangle/line/ellipse/fill for geometric regions and set_pixel_rows (x, y, rows of palette indices) for dense artwork instead of thousands of set_pixels objects. Split larger artwork into bounded row strips or sparse batches, read each returned revision and use it as the next expected_revision. Each batch is atomic and a separate undo step; earlier batches remain if a later one fails. Do not retry an oversized call unchanged. Use cropped get_pixels/inspect_project and selected contact-sheet frames to keep inspection responses small. Tool schemas describe the currently supported layer, frame, palette, region and canvas operations."
    )
}

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
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ContactSheet {
    project_id: String,
    /// Ordered subset of stable IDs, or omit for the entire timeline (maximum 64 frames).
    frame_ids: Option<Vec<String>>,
    #[serde(default = "default_scale")]
    scale: u32,
    #[serde(default = "default_columns")]
    columns: u32,
}
fn default_columns() -> u32 {
    4
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
struct ResizeCanvas {
    project_id: String,
    expected_revision: u64,
    width: u32,
    height: u32,
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

#[derive(Deserialize, JsonSchema)]
struct ReportActivity {
    project_id: String,
    /// Short user-facing description, such as "Drawing the ranger's weapons".
    message: String,
    /// True while working; false when finished.
    busy: bool,
    frame_id: Option<String>,
    layer_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenProject {
    path: PathBuf,
    output_path: Option<PathBuf>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExportProject {
    project_id: String,
    output_path: PathBuf,
    #[serde(default)]
    overwrite: bool,
    frame_id: Option<String>,
    #[serde(default)]
    loop_animation: bool,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditPalette {
    project_id: String,
    expected_revision: u64,
    operation: crate::edit_extra::PaletteOperation,
}
#[derive(Deserialize, JsonSchema)]
struct CopyRegion {
    project_id: String,
    expected_revision: u64,
    #[serde(flatten)]
    operation: crate::edit_extra::CopyRegion,
}
#[derive(Deserialize, JsonSchema)]
struct TransformRegion {
    project_id: String,
    expected_revision: u64,
    #[serde(flatten)]
    operation: crate::edit_extra::TransformRegion,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Analysis {
    project_id: String,
    frame_id: String,
    reference_frame_id: Option<String>,
    layer_id: Option<String>,
    #[serde(default = "default_scale")]
    scale: u32,
}
#[derive(Deserialize, JsonSchema)]
struct ReviewSheetArgs {
    project_id: String,
    #[serde(flatten)]
    request: crate::review::render::ReviewSheetRequest,
}
#[derive(Deserialize, JsonSchema)]
struct CompareCheckpointArgs {
    project_id: String,
    #[serde(flatten)]
    request: crate::review::compare::CompareCheckpointRequest,
}
#[derive(Deserialize, JsonSchema)]
struct AnalyzeSpriteArgs {
    project_id: String,
    #[serde(flatten)]
    request: crate::review::measure::AnalyzeSpriteRequest,
}
#[derive(Deserialize, JsonSchema)]
struct ValidateArtworkArgs {
    project_id: String,
    #[serde(flatten)]
    request: crate::review::rules::ValidateArtworkRequest,
}
#[derive(Deserialize, JsonSchema)]
struct AuditAnimationArgs {
    project_id: String,
    #[serde(flatten)]
    request: crate::review::animation::AuditAnimationRequest,
}
#[derive(Deserialize, JsonSchema)]
struct ManageCheckpointArgs {
    project_id: String,
    #[serde(flatten)]
    request: crate::review::checkpoints::CheckpointAction,
}
struct State {
    session: Session,
    viewers: BTreeMap<PathBuf, crate::viewer::ViewerProcess>,
    show_viewer: bool,
    custom_activity: Option<(String, String)>,
    last_inspection: Option<crate::inspection::View>,
    last_review: Option<crate::viewer::review_ui::ViewerReview>,
    desktop_events: Option<std::sync::mpsc::Sender<desktop::DesktopEvent>>,
}
impl State {
    fn inspection(&mut self, view: &crate::inspection::View) {
        self.last_inspection = Some(view.clone());
        if let Some(events) = &self.desktop_events {
            let _ = events.send(desktop::DesktopEvent::Inspection(view.clone()));
        }
        self.last_review = None;
        let Some(path) = self.session.recovery_path() else {
            return;
        };
        if let Some(viewer) = self.viewers.get_mut(&preview_key(path))
            && let Err(e) = viewer.inspection(view)
        {
            eprintln!("pixen: inspection delivery failed: {e:#}");
        }
    }
    fn review_response(
        &mut self,
        review: anyhow::Result<crate::review::ReviewResult>,
    ) -> CallToolResult {
        let r = match review {
            Ok(r) => r,
            Err(e) => return result(Err(e)),
        };
        let wire = match crate::viewer::review_ui::ViewerReview::new(&r) {
            Ok(w) => w,
            Err(e) => return result(Err(e)),
        };
        self.last_inspection = None;
        self.last_review = Some(wire.clone());
        if let Some(events) = &self.desktop_events {
            let _ = events.send(desktop::DesktopEvent::Review(wire.clone()));
        }
        if let Some(path) = self.session.recovery_path()
            && let Some(viewer) = self.viewers.get_mut(&preview_key(path))
            && let Err(e) = viewer.review(&wire)
        {
            eprintln!("pixen: review delivery failed: {e:#}");
        }
        let mut content = r
            .images
            .into_iter()
            .map(|i| {
                Content::image(
                    base64::engine::general_purpose::STANDARD.encode(i.png),
                    "image/png",
                )
            })
            .collect::<Vec<_>>();
        content.push(Content::text(r.report.to_string()));
        CallToolResult::success(content)
    }
    fn report_activity(&mut self, args: ReportActivity) -> anyhow::Result<serde_json::Value> {
        let message = args.message.trim();
        anyhow::ensure!(
            !message.is_empty()
                && message.chars().count() <= 240
                && !message.chars().any(char::is_control),
            "Activity must be 1..240 characters on one line"
        );
        let info = self.session.info(&args.project_id)?;
        if let Some(id) = &args.frame_id {
            anyhow::ensure!(
                info["frames"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|frame| frame["id"] == *id),
                "Unknown frame_id"
            );
        }
        if let Some(id) = &args.layer_id {
            anyhow::ensure!(
                info["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|layer| layer["id"] == *id),
                "Unknown layer_id"
            );
        }
        self.custom_activity = args
            .busy
            .then(|| (args.project_id.clone(), message.to_owned()));
        self.publish_activity(&crate::viewer::Activity {
            label: message.to_owned(),
            frame_id: args.frame_id,
            layer_id: args.layer_id,
            busy: args.busy,
        });
        Ok(serde_json::json!({"reported":true,"revision":info["revision"]}))
    }
    fn publish_activity(&mut self, activity: &crate::viewer::Activity) {
        if let Some(events) = &self.desktop_events {
            let _ = events.send(desktop::DesktopEvent::Activity(activity.clone()));
        }
        let Some(path) = self.session.recovery_path() else {
            return;
        };
        if let Some(viewer) = self.viewers.get_mut(&preview_key(path))
            && let Err(error) = viewer.activity(activity)
        {
            eprintln!("pixen: activity delivery failed: {error:#}");
        }
    }
    fn apply(
        &mut self,
        id: &str,
        expected: u64,
        operations: Vec<Operation>,
    ) -> anyhow::Result<serde_json::Value> {
        let mut event = edit_activity(&operations);
        let custom = self
            .custom_activity
            .as_ref()
            .filter(|(project, _)| project == id)
            .map(|(_, label)| label.clone());
        if let Some(label) = &custom {
            event.label = label.clone();
        }
        self.publish_activity(&event);
        let result = self.session.apply(id, expected, operations);
        event.busy = custom.is_some();
        match &result {
            Ok(_) => {}
            Err(_) => {
                event.label = "Edit failed · project unchanged".into();
                event.busy = false;
                event.frame_id = None;
                event.layer_id = None;
            }
        }
        self.publish_activity(&event);
        result
    }
    fn history(
        &mut self,
        id: &str,
        expected: u64,
        redo: bool,
    ) -> anyhow::Result<serde_json::Value> {
        let mut event = crate::viewer::Activity {
            label: if redo { "Redo" } else { "Undo" }.into(),
            frame_id: None,
            layer_id: None,
            busy: true,
        };
        self.publish_activity(&event);
        let result = if redo {
            self.session.redo(id, expected)
        } else {
            self.session.undo(id, expected)
        };
        event.busy = false;
        event.label = match &result {
            Ok(_) => event.label.clone(),
            Err(_) => format!("{} failed · project unchanged", event.label),
        };
        self.publish_activity(&event);
        result
    }
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
        self.viewers.insert(key.clone(), viewer);
        if let Some(viewer) = self.viewers.get_mut(&key) {
            let _ = viewer.activity(&crate::viewer::Activity {
                label: "Preview ready".into(),
                frame_id: None,
                layer_id: None,
                busy: false,
            });
        }
        if self
            .session
            .recovery_path()
            .is_some_and(|source| preview_key(source) == key)
            && let Some(view) = self.last_inspection.clone()
        {
            self.inspection(&view);
        }
        if self
            .session
            .recovery_path()
            .is_some_and(|p| preview_key(p) == key)
            && let Some(r) = self.last_review.clone()
            && let Some(viewer) = self.viewers.get_mut(&key)
        {
            let _ = viewer.review(&r);
        }
        reply["reused"] = false.into();
        Ok(reply)
    }
}
fn edit_activity(operations: &[Operation]) -> crate::viewer::Activity {
    use crate::edit::{FrameOperation as F, LayerOperation as L};
    let mut event = crate::viewer::Activity {
        label: "Apply edits".into(),
        frame_id: None,
        layer_id: None,
        busy: true,
    };
    for operation in operations {
        match operation {
            Operation::Draw(draw) => {
                event.label = "Draw pixels".into();
                event.frame_id = Some(draw.frame_id.clone());
                event.layer_id = Some(draw.layer_id.clone());
            }
            Operation::Palette(_) => {
                event.label = "Edit palette".into();
            }
            Operation::Region(region) => {
                let (f, l, label) = match region {
                    crate::edit_extra::RegionOperation::CopyRegion(r) => {
                        (&r.frame_id, &r.layer_id, "Copy region")
                    }
                    crate::edit_extra::RegionOperation::TransformRegion(r) => {
                        (&r.frame_id, &r.layer_id, "Transform region")
                    }
                };
                event.frame_id = Some(f.clone());
                event.layer_id = Some(l.clone());
                event.label = label.into();
            }
            Operation::Layer(layer) => {
                let (label, id) = match layer {
                    L::AddLayer { layer_id, .. } => ("Add layer", Some(layer_id)),
                    L::DuplicateLayer { new_layer_id, .. } => {
                        ("Duplicate layer", Some(new_layer_id))
                    }
                    L::RemoveLayer { .. } => ("Remove layer", None),
                    L::RenameLayer { layer_id, .. } => ("Rename layer", Some(layer_id)),
                    L::ReorderLayer { layer_id, .. } => ("Reorder layer", Some(layer_id)),
                    L::UpdateLayer { layer_id, .. } => ("Update layer", Some(layer_id)),
                    L::LinkCells { layer_id, .. } => ("Link cells", Some(layer_id)),
                    L::UnlinkCells { layer_id, .. } => ("Make cells independent", Some(layer_id)),
                    L::CopyCells { layer_id, .. } => ("Copy cells", Some(layer_id)),
                    L::PasteCells { layer_id, .. } => ("Paste cels", Some(layer_id)),
                    L::ClearCells { layer_id, .. } => ("Clear cells", Some(layer_id)),
                };
                event.label = label.into();
                event.layer_id = id.cloned();
            }
            Operation::Frame(frame) => {
                let (label, id) = match frame {
                    F::AddFrame { frame_id, .. } => ("Add frame", Some(frame_id)),
                    F::RemoveFrame { .. } => ("Remove frame", None),
                    F::DuplicateFrame { new_frame_id, .. } => {
                        ("Duplicate frame", Some(new_frame_id))
                    }
                    F::ReorderFrame { frame_id, .. } => ("Reorder frame", Some(frame_id)),
                    F::RenameFrame { frame_id, .. } => ("Rename frame", Some(frame_id)),
                    F::SetFrameDuration { frame_id, .. } => ("Change frame timing", Some(frame_id)),
                };
                event.label = label.into();
                event.frame_id = id.cloned();
            }
        }
    }
    if operations.len() > 1 {
        event.label = format!("{} ({} edits)", event.label, operations.len());
    }
    event
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
impl Server {
    fn analysis_image(&self, args: Analysis, mode: crate::inspection::Mode) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let rendered = state.session.analysis(
            &args.project_id,
            &args.frame_id,
            args.reference_frame_id.as_deref(),
            mode,
            args.scale,
            args.layer_id.as_deref(),
        );
        match rendered {
            Ok((png, metadata)) => {
                let view = crate::inspection::View {
                    frame_id: args.frame_id,
                    reference_frame_id: args.reference_frame_id,
                    mode,
                    layer_id: args.layer_id,
                };
                state.inspection(&view);
                CallToolResult::success(vec![
                    Content::image(
                        base64::engine::general_purpose::STANDARD.encode(png),
                        "image/png",
                    ),
                    Content::text(metadata.to_string()),
                ])
            }
            Err(e) => result(Err(e)),
        }
    }
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
        description = "Read-only multi-scale color/silhouette/grayscale PNG review with neutral labels, opaque backgrounds and source-space panel metadata. Defaults display_scale=1, enlarged_scale=8. May synchronize the inspector while Follow AI is on."
    )]
    fn get_review_sheet(&self, Parameters(args): Parameters<ReviewSheetArgs>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let r = state.session.review_sheet(&args.project_id, &args.request);
        state.review_response(r)
    }
    #[tool(
        description = "Compare the current document with an immutable session checkpoint: structural changes, exact indices/resolved colors, disjoint added/removed/repainted/alpha-only pixels, and optional before/current/difference images. No aesthetic rating; default include_images=true and scale=8."
    )]
    fn compare_checkpoint(
        &self,
        Parameters(args): Parameters<CompareCheckpointArgs>,
    ) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let r = state
            .session
            .compare_checkpoint(&args.project_id, &args.request);
        state.review_response(r)
    }
    #[tool(
        description = "Measure bounds, padding, occupancy, centroid, exact palette-index use, rendered colors and 4/8-connected components. One-pixel components are observations, not confirmed defects. Returns bounded annotated PNG evidence by default; never edits artwork."
    )]
    fn analyze_sprite(&self, Parameters(args): Parameters<AnalyzeSpriteArgs>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let r = state
            .session
            .analyze_sprite(&args.project_id, &args.request);
        state.review_response(r)
    }
    #[tool(
        description = "Check 1..64 caller-supplied canvas/count/timing/palette/color/alpha/protected-region rules. Valid violations return passed:false as successful reports; invalid arguments are tool errors. Protected regions use immutable checkpoints. No implicit art style or automatic repair."
    )]
    fn validate_artwork(
        &self,
        Parameters(args): Parameters<ValidateArtworkArgs>,
    ) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let r = state
            .session
            .validate_artwork(&args.project_id, &args.request);
        state.review_response(r)
    }
    #[tool(
        description = "Read-only animation evidence: durations, holds, distinct rendered poses, bounds/centroids, consecutive frame changes, optional loop transition and caller-threshold warnings. PNG contact sheet. Does not judge artistic quality or require identical loop endpoints."
    )]
    fn audit_animation(&self, Parameters(args): Parameters<AuditAnimationArgs>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let r = state
            .session
            .audit_animation(&args.project_id, &args.request);
        state.review_response(r)
    }
    #[tool(
        description = "Create/list/delete immutable session-local checkpoints independently of undo/redo. create/delete require expected_revision; create label is 1..80 characters. At most 16 snapshots/64 MiB, no silent eviction. Captures survive undo and save, and end with the editing session. Does not change artwork revision/history."
    )]
    fn manage_checkpoint(
        &self,
        Parameters(args): Parameters<ManageCheckpointArgs>,
    ) -> CallToolResult {
        result(
            self.state
                .lock()
                .unwrap()
                .session
                .manage_checkpoint(&args.project_id, args.request),
        )
    }
    #[tool(
        description = "Open any supported inspector input as an editable project: .pixloom, JSON, .pxo, .ase/.aseprite and supported raster images/animations. Preserve authoring palette indices. Default save destination is .pixloom (same file for .pixloom input, sibling otherwise); optional output_path selects .pixloom or .pixen.json. One active project; existing unrelated destinations are rejected."
    )]
    fn open_project(&self, Parameters(args): Parameters<OpenProject>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let opened = state.session.open(&args.path, args.output_path.as_deref());
        if opened.is_ok() {
            state.last_inspection = None;
            state.last_review = None;
            state.custom_activity = None;
        }
        if opened.is_ok() && state.show_viewer {
            let path = state.session.recovery_path().unwrap().to_owned();
            if let Err(e) = state.open(path) {
                eprintln!("pixen: viewer unavailable: {e:#}");
            }
        }
        result(opened)
    }
    #[tool(
        description = "Save the active project's .pixloom (preferred) or explicitly chosen JSON destination without ending the session or changing revision/history. Atomic checkpoint; refuses unrelated existing files or external changes since open/save."
    )]
    fn save_project(&self, Parameters(args): Parameters<ProjectId>) -> CallToolResult {
        result(self.state.lock().unwrap().session.save(&args.project_id))
    }
    #[tool(
        description = "Explicitly export active artwork to .aseprite/.ase, .pxo, .pixloom, .pixen.json or a supported image format (PNG/GIF/APNG/WebP/JPEG/BMP/TIFF/SVG). Does not change project destination, revision or history. Optional stable frame_id selects one image frame; animated images otherwise use full timeline. overwrite defaults false; loop_animation defaults false. Video uses CLI."
    )]
    fn export_project(&self, Parameters(args): Parameters<ExportProject>) -> CallToolResult {
        result(self.state.lock().unwrap().session.export(
            &args.project_id,
            &args.output_path,
            args.overwrite,
            args.frame_id.as_deref(),
            args.loop_animation,
        ))
    }
    #[tool(
        description = "Atomically append palette colors, set one exact #RRGGBBAA entry globally, or remap an exact index globally or within explicit layer/frame scope. No renumbering or deduplication. Remaps respect locked layers and intentional cel links. Set color retains a transparent entry for empty cels. One undo step; expected_revision required."
    )]
    fn edit_palette(&self, Parameters(args): Parameters<EditPalette>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Palette(args.operation)],
        ))
    }
    #[tool(
        description = "Copy a source layer/frame rectangle to explicit destination layer/frame and x/y; includes transparent pixels and preserves exact palette indices. Optional flip/clockwise quarter-turn transform. Reads source before writing, so overlaps are safe. Strict bounds, locked-layer/link behavior and atomic undo match draw."
    )]
    fn copy_region(&self, Parameters(args): Parameters<CopyRegion>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Region(
                crate::edit_extra::RegionOperation::CopyRegion(args.operation),
            )],
        ))
    }
    #[tool(
        description = "Move/flip/rotate a bounded region within explicit layer/frame. Snapshot source, clear old rectangle, paste transformed pixels at x/y atomically. none means translation; rotate90/180/270 are clockwise. Quarter-turn output dimensions swap. Includes transparent pixels; strict bounds and linked-cel behavior; one undo step."
    )]
    fn transform_region(&self, Parameters(args): Parameters<TransformRegion>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Region(
                crate::edit_extra::RegionOperation::TransformRegion(args.operation),
            )],
        ))
    }
    #[tool(
        description = "Return actual PNG onion-skin image: current pose over faint red previous and cyan next poses, without timeline wrapping. Optional reference_frame_id replaces neighbors with that red reference; optional layer_id isolates a layer. Read-only; selects matching inspection mode in an open viewer while Follow AI is enabled."
    )]
    fn get_onion_skin(&self, Parameters(args): Parameters<Analysis>) -> CallToolResult {
        self.analysis_image(args, crate::inspection::Mode::OnionSkin)
    }
    #[tool(
        description = "Return actual PNG frame comparison and changed_pixels count using exact composited RGBA. Pink marks changed pixels (including disappearing pixels); unchanged pixels are faint. Optional reference_frame_id defaults to previous frame, or self for first frame. Optional layer_id isolates a layer. Read-only; syncs open Follow AI viewer."
    )]
    fn compare_frames(&self, Parameters(args): Parameters<Analysis>) -> CallToolResult {
        self.analysis_image(args, crate::inspection::Mode::FrameComparison)
    }
    #[tool(
        description = "Return an isolated layer PNG, even if hidden, without changing artwork visibility. Explicit layer_id required. Read-only; clears onion/difference mode in Follow AI viewer and shows only this layer."
    )]
    fn inspect_layer(&self, Parameters(args): Parameters<Analysis>) -> CallToolResult {
        if args.layer_id.is_none() {
            return result(Err(anyhow::anyhow!("layer_id is required")));
        }
        self.analysis_image(args, crate::inspection::Mode::Composite)
    }
    #[tool(
        description = "Report a short user-facing activity description for an active project, e.g. Drawing the ranger's weapons. Does not modify the project or revision. Use busy=true to keep this description and spinner through subsequent edits; busy=false completes it and restores automatic activity descriptions. Optional stable frame_id/layer_id lets Follow AI select the target. Messages must be a single line of 1..240 characters."
    )]
    fn report_activity(&self, Parameters(args): Parameters<ReportActivity>) -> CallToolResult {
        result(self.state.lock().unwrap().report_activity(args))
    }

    #[tool(
        description = "Create one transparent pixel-art project with one frame and layer. Palette entries are #RRGGBBAA, indexed from zero. Finish it before creating another."
    )]
    fn create_project(&self, Parameters(args): Parameters<CreateProject>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let created = state.session.create(args);
        if created.is_ok() {
            state.last_inspection = None;
            state.last_review = None;
            state.custom_activity = None;
        }
        if created.is_ok() && state.show_viewer {
            let path = state.session.recovery_path().unwrap().to_owned();
            if let Err(e) = state.open(path) {
                eprintln!("pixen: viewer unavailable: {e:#}");
            }
        }
        result(created)
    }

    #[tool(
        description = "Resize the active canvas as one revision-checked undo step. Preserve exact palette indices, layer/frame IDs and timing. Anchor pixels at the top left; smaller bounds crop all cels, larger bounds pad with transparency. Does not scale artwork. Invalid dimensions or stale revisions leave document and history unchanged."
    )]
    fn resize_canvas(&self, Parameters(args): Parameters<ResizeCanvas>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        let mut activity = crate::viewer::Activity {
            label: "Resizing canvas…".into(),
            busy: true,
            frame_id: None,
            layer_id: None,
        };
        state.publish_activity(&activity);
        let resized = state.session.resize_canvas(
            &args.project_id,
            args.expected_revision,
            args.width,
            args.height,
        );
        activity.busy = false;
        activity.label = if resized.is_ok() {
            "Canvas resized"
        } else {
            "Canvas resize failed · project unchanged"
        }
        .into();
        state.publish_activity(&activity);
        result(resized)
    }

    #[tool(
        description = "Apply one atomic transaction at expected_revision containing typed drawing, layer and/or frame operations. Drawing operations explicitly target frame_id and layer_id; structural operations carry the relevant stable ID. Coordinates are zero-based and strictly bounded; shapes have positive dimensions; fill is four-connected equal-index replacement. Invalid or stale batches change nothing, including undo history. One successful batch is one undo step. Returns revision, drawing changed bounds/target changes or conservative full-canvas structural bounds. No PNG is generated."
    )]
    fn apply_edits(&self, Parameters(args): Parameters<ApplyEdits>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
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
        description = "Draw one typed operation on an explicit project/layer/frame at expected_revision. Supports compact set_pixel_rows (x,y,rows: equal-width arrays of palette indices; non-empty, strict bounds, all entries including transparent indices overwrite pixels), sparse set_pixels, inclusive-endpoint Bresenham line, positive origin/dimension rectangle or ellipse (filled/outlined), four-connected equal-index fill, erase_region (transparent), clear_cel (transparent), and legacy clear with palette_index. All bounds and indices are strict; no clipping or implicit palette creation except a transparent entry if required. Ellipses test pixel centres inside the bounding ellipse; outlines are four-neighbour boundaries. Uses the same atomic executor as apply_edits and one undo step. No image is generated. Explicit link groups share edits; unlinked reused cels detach."
    )]
    fn draw(&self, Parameters(args): Parameters<Draw>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
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
        description = "Atomically add/remove/duplicate/rename/reorder a layer, update visibility/opacity, or paste/clear/link/unlink timeline cells, at expected_revision. See the operation schema for current variants. Use explicit stable layer IDs; add_layer requires a new never-used ID and a name. Indices are zero-based final positions, bottom-to-top compositing order. New layers are transparent on every frame. duplicate_layer copies all cels independently under a new never-used new_layer_id and name. Opacity is 0..1. Removing a layer is destructive but undoable; the last layer cannot be removed. Uses apply_edits' executor; no image is generated."
    )]
    fn edit_layer(&self, Parameters(args): Parameters<EditLayer>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
            &args.project_id,
            args.expected_revision,
            vec![Operation::Layer(args.operation)],
        ))
    }

    #[tool(
        description = "Atomically add/remove/duplicate/rename/reorder a frame or set its positive finite duration_ms, at expected_revision. Explicit stable frame IDs survive reorder; additions require a never-used ID. Indices are zero-based final positions. New frames are transparent. duplicate_frame copies all layer cels and clears duplicate links: editing it cannot modify its source. Removing a frame is destructive but undoable; the last frame cannot be removed. Existing tag membership is retained; non-contiguous tag results are rejected. Uses apply_edits' executor; no image is generated. Duplication alone creates an identical hold, not motion: draw visually distinct keyframes and useful intermediate poses, then review get_contact_sheet and playback."
    )]
    fn edit_frame(&self, Parameters(args): Parameters<EditFrame>) -> CallToolResult {
        result(self.state.lock().unwrap().apply(
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
                .history(&args.project_id, args.expected_revision, false),
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
                .history(&args.project_id, args.expected_revision, true),
        )
    }

    #[tool(
        description = "Return only the FIRST ORDERED FRAME composite (not animation or viewer selection) as actual PNG image content with nearest-neighbour integer scaling (default 8), revision and original canvas dimensions."
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
        description = "Render an active editing project's frame as actual PNG image content for AI inspection. Defaults to frame-1 and scale 8. Optional crop uses zero-based source x/y and positive width/height wholly inside the canvas. Returns revision, frame_id, original dimensions, scale and source rectangle. Read-only artwork inspection; selects composite view/frame in an open viewer while Follow AI is enabled."
    )]
    fn inspect_project(&self, Parameters(args): Parameters<InspectProject>) -> CallToolResult {
        let mut state = self.state.lock().unwrap();
        match state
            .session
            .inspect(&args.project_id, &args.frame_id, args.crop, args.scale)
        {
            Ok((png, metadata)) => {
                state.inspection(&crate::inspection::View {
                    frame_id: args.frame_id,
                    reference_frame_id: None,
                    mode: crate::inspection::Mode::Composite,
                    layer_id: None,
                });
                CallToolResult::success(vec![
                    Content::image(
                        base64::engine::general_purpose::STANDARD.encode(png),
                        "image/png",
                    ),
                    Content::text(metadata.to_string()),
                ])
            }
            Err(e) => result(Err(e)),
        }
    }

    #[tool(
        description = "Return an actual PNG contact sheet for AI animation review, labelled with stable frame IDs and durations. Defaults to the full ordered timeline, scale 8 and four columns; optional ordered unique frame_ids selects 1..64 frames. Maximum 16 million output pixels. Metadata includes revision, canvas dimensions, frame image bounds and distinct_composites (identical rendered holds count once). Read-only. A sheet verifies poses, not playback; also inspect playback in the user viewer. Animation requests require multiple visually distinct frames with intentional motion, not just duplicate frames."
    )]
    fn get_contact_sheet(&self, Parameters(args): Parameters<ContactSheet>) -> CallToolResult {
        match self.state.lock().unwrap().session.contact_sheet(
            &args.project_id,
            args.frame_ids,
            args.scale,
            args.columns,
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
        description = "Show/restore and request foreground activation of a read-only user preview. Supply exactly one of project_id (active editing project) or path (existing Pixen JSON/.pixloom, Pixelorama .pxo, RGBA Aseprite, or GIF). Reuses an existing window for the same source; returns promptly after a bounded opening acknowledgement, never waits for user approval or closure. Existing files are never written. opened reports window initialization, focused is observed focus and activation remains subject to OS policy. Showing a window provides NO visual feedback to the AI; use inspect_project for an actual PNG image."
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
        description = "Validate and save to the project's .pixloom (preferred) or explicit JSON destination, refusing unrelated existing files and external changes. Success closes its viewer and releases the project; failure preserves the active session and viewer."
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
            .with_server_info(Implementation::new("pixen", env!("CARGO_PKG_VERSION")))
            .with_instructions(authoring_instructions("Pixen pixel-art authoring. Use report_activity with a brief user-facing description when starting a meaningful task (busy=true) and when finishing it (busy=false). An animation request means multiple visually distinct frames with intentional motion, not a static sprite in a multi-frame document. Workflow: block out, review, create keyframes, add useful intermediates, inspect get_contact_sheet and playback, correct, finish/save, export the full timeline (CLI --loop for looping GIF/WebP/APNG), open user preview. get_preview shows only the first frame; inspect_project targets a specific stable frame ID. Use get_project_info/get_pixels for exact inspection; edit_layer/edit_frame/draw or mixed apply_edits batches for atomic revision-checked changes; undo/redo for transaction history. inspect_project returns actual PNG content; open_preview only shows the user a read-only window. Viewer selection never changes explicit editing targets. Prefer .pixloom project destinations. open_project accepts all supported inspector inputs, save_project checkpoints without ending a session, and finish_project saves/releases it. Both refuse external file changes. export_project explicitly writes native or image formats. edit_palette/copy_region/transform_region share apply_edits transactions. get_onion_skin/compare_frames/inspect_layer return actual PNG review and sync an open Follow AI viewer without editing artwork. Use get_review_sheet for multi-scale silhouette/grayscale evidence, manage_checkpoint/compare_checkpoint for independent baselines, analyze_sprite for measurements, validate_artwork for caller rules and audit_animation for motion evidence. Read-only reviews never change artwork/history and do not score artistic quality or run critic agents. Checkpoints are session-local; follow report revisions and truncation flags."))
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
                custom_activity: None,
                last_inspection: None,
                last_review: None,
                desktop_events: None,
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

#[cfg(test)]
mod activity_tests {
    #[test]
    fn batches_follow_the_last_explicit_frame_and_layer() {
        let operations: Vec<super::Operation> = serde_json::from_value(serde_json::json!([
            {"type":"add_layer", "layer_id":"ink", "name":"Ink"},
            {"type":"add_frame", "frame_id":"pose", "duration_ms":100},
            {"type":"clear_cel", "frame_id":"pose", "layer_id":"ink"}
        ]))
        .unwrap();
        let event = super::edit_activity(&operations);
        assert_eq!(event.frame_id.as_deref(), Some("pose"));
        assert_eq!(event.layer_id.as_deref(), Some("ink"));
        assert!(event.busy);
        assert_eq!(event.label, "Draw pixels (3 edits)");
        let wire = serde_json::to_string(&event).unwrap();
        let decoded: crate::viewer::Activity = serde_json::from_str(&wire).unwrap();
        assert_eq!(decoded.frame_id, event.frame_id);
    }
}
