//! Workflow-facing project/structure API. All edits commit through Session once.
use super::*;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const CONTRACT: &str = "pixen-mcp/v1";

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Discovery {
    /// live for desktop, headless for standalone. Omit to discover the connection.
    mode: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Info {
    project_id: Option<String>,
}

/// Common scope is separate from commands in edit_batch; nested calls cannot retarget.
#[derive(Deserialize, JsonSchema)]
pub(super) struct Structure {
    project_id: Option<String>,
    expected_revision: Option<u64>,
    #[serde(flatten)]
    command: Value,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Batch {
    project_id: Option<String>,
    expected_revision: Option<u64>,
    operations: Vec<Entry>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Entry {
    tool: String,
    arguments: Value,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Manage {
    op: String,
    project_id: Option<String>,
    expected_revision: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    palette: Option<Vec<String>>,
    output_path: Option<PathBuf>,
    mode: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Resize {
    project_id: Option<String>,
    expected_revision: Option<u64>,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct Failure {
    code: &'static str,
    message: String,
    details: Value,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for Failure {}
fn fail<T>(code: &'static str, message: impl Into<String>, details: Value) -> Result<T> {
    Err(Failure {
        code,
        message: message.into(),
        details,
    }
    .into())
}
fn required<T>(v: Option<T>, field: &str) -> Result<T> {
    v.ok_or_else(|| {
        Failure {
            code: "invalid_arguments",
            message: format!("Missing {field}"),
            details: json!({"field":field}),
        }
        .into()
    })
}
fn mode(state: &State) -> &'static str {
    if state.desktop_events.is_some() {
        "desktop"
    } else {
        "standalone"
    }
}
fn check_mode(state: &State, requested: Option<&str>) -> Result<()> {
    if let Some(requested) = requested {
        let expected = if mode(state) == "desktop" {
            "live"
        } else {
            "headless"
        };
        if requested != expected {
            return fail(
                "mode_mismatch",
                format!("This connection requires mode {expected}"),
                json!({}),
            );
        }
    }
    Ok(())
}
fn project(state: &State, requested: Option<&str>) -> Result<String> {
    let Some(active) = state.session.active_id() else {
        return fail("no_project", "Create or attach an artwork first", json!({}));
    };
    if requested.is_some_and(|id| id != active) {
        return fail(
            "project_mismatch",
            "Project does not match this connection's active artwork",
            json!({}),
        );
    }
    Ok(active.to_owned())
}
fn guard(state: &State, id: &str, expected: Option<u64>) -> Result<u64> {
    let Some(expected) = expected else {
        return fail(
            "revision_required",
            "Mutations require expected_revision",
            json!({}),
        );
    };
    if state.session.info(id)?["revision"] != expected {
        return fail(
            "stale_revision",
            "Refresh sprite_info and reconsider this edit",
            json!({}),
        );
    }
    Ok(expected)
}
fn envelope(
    state: &State,
    tool: &str,
    op: &str,
    targets: Value,
    value: Result<Value>,
) -> CallToolResult {
    let active = state
        .session
        .active_id()
        .and_then(|id| state.session.info(id).ok());
    let mut metadata = json!({"contract":CONTRACT,"tool":tool,"op":op});
    let error = value.is_err();
    match value {
        Ok(value) => {
            metadata["result"] = value;
            metadata["resolved_targets"] = targets;
        }
        Err(error) => {
            let (code, details) = if let Some(f) = error.downcast_ref::<Failure>() {
                (f.code, f.details.clone())
            } else {
                let message = error.to_string();
                let code = if message.contains("locked") {
                    "locked_layer"
                } else if message.contains("outside") || message.contains("bounds") {
                    "out_of_bounds"
                } else if message.contains("10000") || message.contains("limit") {
                    "resource_limit"
                } else if message.contains("disk") || message.contains("already exists") {
                    "external_change"
                } else if error.chain().any(|e| e.is::<std::io::Error>()) {
                    "io_error"
                } else {
                    "invalid_arguments"
                };
                (code, json!({}))
            };
            metadata["error"] = json!({"code":code,"message":error.to_string(),"tool":tool,"op":op,
                "capability":format!("{tool}.{op}"),"mode":mode(state),"retryable":false,
                "alternatives":if code == "application_owned" { vec!["Pixen UI Save / Save As / Open / Export"] } else { vec!["preflight", "sprite_info"] },"details":details});
        }
    }
    if let Some(info) = active {
        metadata["project_id"] = info["project_id"].clone();
        metadata["revision"] = info["revision"].clone();
    }
    let mut result = if error {
        CallToolResult::error(vec![Content::text(metadata.to_string())])
    } else {
        CallToolResult::success(vec![Content::text(metadata.to_string())])
    };
    result.structured_content = Some(metadata);
    result
}

pub(super) fn discovery(state: &State, args: Discovery) -> CallToolResult {
    let value = (|| {
        check_mode(state, args.mode.as_deref())?;
        let mut operations = serde_json::from_str::<Value>(include_str!(
            "../../docs/mcp-compatibility/operations.json"
        ))?["operations"]
            .as_array()
            .unwrap()
            .clone();
        for operation in &mut operations {
            let tool = operation["target_tool"].as_str().unwrap().to_owned();
            let op = operation["target_op"].as_str().unwrap().to_owned();
            let available = match tool.as_str() {
                "preflight" | "sprite_info" | "resize_canvas" => true,
                "sprite_manage" => op == "new",
                "layer" => [
                    "create",
                    "duplicate",
                    "delete",
                    "rename",
                    "reorder",
                    "set",
                    "batch",
                ]
                .contains(&op.as_str()),
                "frame" => ["add", "duplicate", "set_duration"].contains(&op.as_str()),
                _ => false,
            };
            operation["tool"] = operation["target_tool"].clone();
            operation["op"] = operation["target_op"].clone();
            operation["available"] = json!(available);
            if available {
                operation["reason"] = json!("Implemented by the attached Pixen session executor");
            }
            operation["owner"] = json!(if mode(state) == "desktop"
                && ["export", "sprite_manage"].contains(&tool.as_str())
                && op != "new"
            {
                "application"
            } else {
                "session"
            });
            operation["constraints"] = json!([
                "Exact schemas in tools/list; mandatory revision on edits",
                "One attached project; flat layers; one-based frame positions"
            ]);
            operation
                .as_object_mut()
                .unwrap()
                .remove("implementation_tools");
        }
        for (tool, ops) in [
            (
                "layer",
                vec![
                    "list",
                    "paste_cells",
                    "clear_cells",
                    "copy_cells",
                    "link_cells",
                    "unlink_cells",
                ],
            ),
            (
                "frame",
                vec!["list", "delete", "rename", "reorder", "batch"],
            ),
            ("edit_batch", vec!["default"]),
        ] {
            for op in ops {
                operations.push(json!({"tool":tool,"op":op,"available":true,"classification":"adapter","constraints":["Revision-checked atomic edits; list is read-only"],"reason":"Implemented using the session executor","alternatives":[],"owner":"session"}));
            }
        }
        Ok(
            json!({"ready":true,"has_project":state.session.active_id().is_some(),"mode":mode(state),
            "versions":{"pixen":env!("CARGO_PKG_VERSION"),"contract":CONTRACT},"frame_base":1,"coordinate_base":0,
            "palette_semantics":"exact_index","revision_required":true,"max_operations":10000,"recommended_request_bytes":200*1024,
            "viewer_enabled":state.show_viewer,"operations":operations}),
        )
    })();
    envelope(state, "preflight", "default", json!([]), value)
}
pub(super) fn info(state: &State, args: Info) -> CallToolResult {
    let value = (|| {
        let id = project(state, args.project_id.as_deref())?;
        let mut info = state.session.info(&id)?;
        info["color_model"] = json!("palette_index_with_rgba_entries");
        info["palette_semantics"] = json!("exact_index");
        info["upstream_color_modes"] = json!(false);
        info["index_zero_is_transparent"] = json!(
            crate::schema::decode_color(&state.session.current_document()?.palette[0])?[3] == 0
        );
        for (index, frame) in info["frames"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            frame["frame"] = json!(index + 1);
        }
        Ok(info)
    })();
    envelope(state, "sprite_info", "default", json!([]), value)
}
pub(super) fn manage(state: &mut State, args: Manage) -> CallToolResult {
    let op = args.op.clone();
    let value = (|| {
        check_mode(state, args.mode.as_deref())?;
        if op != "new" {
            if mode(state) == "desktop"
                && ["save", "save_as", "open", "finish", "export"].contains(&op.as_str())
            {
                return fail(
                    "application_owned",
                    "Use the Pixen application to manage files and artwork lifetime",
                    json!({}),
                );
            }
            return fail(
                "capability_unavailable",
                "This sprite_manage operation is not implemented; consult preflight",
                json!({}),
            );
        }
        ensure!(
            args.project_id.is_none() && args.expected_revision.is_none(),
            "new does not accept project_id or expected_revision"
        );
        let created = state.session.create(CreateProject {
            width: required(args.width, "width")?,
            height: required(args.height, "height")?,
            palette: required(args.palette, "palette")?,
            output_path: required(args.output_path, "output_path")?,
        })?;
        state.last_inspection = None;
        state.last_review = None;
        state.custom_activity = None;
        if state.show_viewer {
            let path = state.session.recovery_path().unwrap().to_owned();
            if let Err(error) = state.open(path) {
                eprintln!("pixen: viewer unavailable: {error:#}");
            }
        }
        Ok(created)
    })();
    envelope(state, "sprite_manage", &op, json!([]), value)
}
pub(super) fn resize(state: &mut State, args: Resize) -> CallToolResult {
    let value = (|| {
        let id = project(state, args.project_id.as_deref())?;
        let revision = guard(state, &id, args.expected_revision)?;
        state
            .session
            .resize_canvas(&id, revision, args.width, args.height)
    })();
    envelope(state, "resize_canvas", "default", json!([]), value)
}

fn fields(args: &Value, allowed: &[&str]) -> Result<()> {
    let object = args
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("arguments must be an object"))?;
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return fail(
                "unsupported_option",
                format!("Unsupported option {key}"),
                json!({"field":key}),
            );
        }
    }
    Ok(())
}
fn string(args: &Value, key: &str) -> Result<String> {
    required(args[key].as_str().map(str::to_owned), key)
}
fn number(args: &Value, key: &str) -> Result<f64> {
    required(args[key].as_f64(), key)
}
fn index(args: &Value) -> Result<Option<usize>> {
    if args.get("index").is_none() {
        return Ok(None);
    }
    Ok(Some(usize::try_from(required(
        args["index"].as_u64(),
        "index (nonnegative integer)",
    )?)?))
}

struct Planner {
    original: crate::schema::Document,
    candidate: crate::schema::Document,
    used: BTreeSet<String>,
    next_cel: u64,
    operations: Vec<Operation>,
    targets: Vec<Value>,
}
impl Planner {
    fn new(state: &State, id: &str) -> Result<Self> {
        let (doc, used, next_cel) = state.session.planning_state(id)?;
        Ok(Self {
            original: doc.clone(),
            candidate: doc,
            used,
            next_cel,
            operations: vec![],
            targets: vec![],
        })
    }
    fn fresh(&self, kind: &str) -> String {
        (1u64..)
            .map(|n| format!("{kind}-{n}"))
            .find(|id| !self.used.contains(&format!("{kind}:{id}")))
            .unwrap()
    }
    fn layer(&self, args: &Value) -> Result<String> {
        ensure!(
            !(args.get("layer").is_some() && args.get("layer_id").is_some()),
            "layer and layer_id are mutually exclusive"
        );
        if let Some(id) = args.get("layer_id") {
            let id = required(id.as_str(), "layer_id")?;
            if self.candidate.layers.iter().any(|l| l.id == id) {
                return Ok(id.into());
            }
            return fail(
                "target_not_found",
                "Unknown layer_id",
                json!({"layer_id":id}),
            );
        }
        let name = args
            .get("layer")
            .map(|n| required(n.as_str(), "layer"))
            .transpose()?;
        let matches = |doc: &crate::schema::Document| {
            doc.layers
                .iter()
                .filter(|l| name.is_none_or(|name| l.name == name))
                .map(|l| json!({"id":l.id,"name":l.name}))
                .collect::<Vec<_>>()
        };
        let mut found = matches(&self.original);
        let original_ids = self
            .original
            .layers
            .iter()
            .map(|l| l.id.as_str())
            .collect::<BTreeSet<_>>();
        found.extend(
            matches(&self.candidate)
                .into_iter()
                .filter(|l| name.is_some() && !original_ids.contains(l["id"].as_str().unwrap())),
        );
        match found.as_slice() {
            [one] => {
                let id = one["id"].as_str().unwrap();
                if !self.candidate.layers.iter().any(|l| l.id == id) {
                    return fail(
                        "target_not_found",
                        "Layer was removed earlier in the batch",
                        json!({"layer_id":id}),
                    );
                }
                Ok(id.into())
            }
            [] => fail(
                "target_not_found",
                "No matching layer",
                json!({"layer":name}),
            ),
            _ => fail(
                "ambiguous_target",
                "Supply an explicit layer_id",
                json!({"candidates":found}),
            ),
        }
    }
    fn frame(&self, args: &Value) -> Result<String> {
        ensure!(
            !(args.get("frame").is_some() && args.get("frame_id").is_some()),
            "frame and frame_id are mutually exclusive"
        );
        if let Some(id) = args.get("frame_id") {
            let id = required(id.as_str(), "frame_id")?;
            if self
                .candidate
                .frames
                .iter()
                .any(|f| crate::edit::frame_id(f) == Some(id))
            {
                return Ok(id.into());
            }
            return fail(
                "target_not_found",
                "Unknown frame_id",
                json!({"frame_id":id}),
            );
        }
        let frames = &self.original.frames;
        let position = match args.get("frame") {
            Some(value) => required(value.as_u64(), "frame (one-based integer)")?,
            None if frames.len() == 1 => 1,
            None => {
                return fail(
                    "ambiguous_target",
                    "Specify frame or frame_id on an animation",
                    json!({}),
                );
            }
        };
        let Some(frame) = position.checked_sub(1).and_then(|i| frames.get(i as usize)) else {
            return fail(
                "target_not_found",
                "Frame position is outside the timeline",
                json!({"frame":position}),
            );
        };
        Ok(crate::edit::frame_id(frame).unwrap().into())
    }
    fn push(&mut self, op: Operation) -> Result<()> {
        if self.operations.len() == 10000 {
            return fail(
                "resource_limit",
                "Expanded batch exceeds 10000 operations",
                json!({}),
            );
        }
        crate::edit::execute(
            &mut self.candidate,
            op.clone(),
            &mut self.used,
            &mut self.next_cel,
        )?;
        self.operations.push(op);
        Ok(())
    }
    fn command(&mut self, tool: &str, args: &Value) -> Result<()> {
        let op = args["op"].as_str().unwrap_or("default");
        match tool {
            "layer" => self.layer_command(op, args),
            "frame" => self.frame_command(op, args),
            // These document tools retain their current schemas until their roadmap issue.
            "draw" => {
                fields(args, &["frame_id", "layer_id", "operation"])?;
                let frame_id = self.frame(args)?;
                let layer_id = self.layer(args)?;
                let drawing =
                    serde_json::from_value(required(args.get("operation").cloned(), "operation")?)?;
                self.targets
                    .push(json!({"frame_id":frame_id,"layer_id":layer_id}));
                self.push(Operation::Draw(crate::edit::DrawOperation {
                    frame_id,
                    layer_id,
                    drawing,
                }))
            }
            "edit_palette" => {
                fields(args, &["operation"])?;
                self.push(Operation::Palette(serde_json::from_value(required(
                    args.get("operation").cloned(),
                    "operation",
                )?)?))
            }
            "copy_region" | "transform_region" => {
                let mut args = args.clone();
                fields(
                    &args,
                    &[
                        "frame_id",
                        "layer_id",
                        "source_frame_id",
                        "source_layer_id",
                        "source",
                        "region",
                        "x",
                        "y",
                        "transform",
                    ],
                )?;
                args["type"] = json!(tool);
                self.push(Operation::Region(serde_json::from_value(args)?))
            }
            "sprite_manage" | "export" | "open_project" | "save_project" | "finish_project"
            | "resize_canvas" | "edit_batch" => fail(
                "unsupported_operation",
                "Lifecycle, resize and nested batches cannot be mixed into edit_batch",
                json!({"tool":tool}),
            ),
            _ => fail(
                "unsupported_tool",
                format!("Unsupported batch tool {tool}"),
                json!({}),
            ),
        }
    }
    fn layer_command(&mut self, op: &str, args: &Value) -> Result<()> {
        use crate::edit::LayerOperation as L;
        let allowed: &[&str] = match op {
            "create" => &["op", "layer_id", "name", "index"],
            "duplicate" => &["op", "layer", "layer_id", "new_layer_id", "name", "index"],
            "delete" => &["op", "layer", "layer_id"],
            "rename" => &["op", "layer", "layer_id", "name"],
            "reorder" => &["op", "layer", "layer_id", "index"],
            "set" => &["op", "layer", "layer_id", "visible", "opacity"],
            // Existing independent cel/link operations remain explicit native capabilities.
            "paste_cells" | "clear_cells" | "link_cells" | "unlink_cells" | "copy_cells" => {
                let mut native = args.clone();
                native.as_object_mut().unwrap().remove("op");
                native["type"] = json!(op);
                return self.push(Operation::Layer(serde_json::from_value(native)?));
            }
            _ => {
                return fail(
                    "capability_unavailable",
                    format!(
                        "layer.{op} is unavailable; grouped layers, locks and cross-project copies are not implemented"
                    ),
                    json!({}),
                );
            }
        };
        fields(args, allowed)?;
        if op == "create" {
            let id = args
                .get("layer_id")
                .map(|v| required(v.as_str(), "layer_id").map(str::to_owned))
                .transpose()?
                .unwrap_or_else(|| self.fresh("layer"));
            let name = string(args, "name")?;
            self.targets
                .push(json!({"layer_id":id,"name":name,"created":true}));
            return self.push(Operation::Layer(L::AddLayer {
                layer_id: id,
                name,
                index: index(args)?,
            }));
        }
        let layer_id = self.layer(args)?;
        self.targets.push(json!({"layer_id":layer_id}));
        let operation = match op {
            "duplicate" => L::DuplicateLayer {
                layer_id,
                new_layer_id: args
                    .get("new_layer_id")
                    .map(|v| required(v.as_str(), "new_layer_id").map(str::to_owned))
                    .transpose()?
                    .unwrap_or_else(|| self.fresh("layer")),
                name: string(args, "name")?,
                index: index(args)?,
            },
            "delete" => L::RemoveLayer { layer_id },
            "rename" => L::RenameLayer {
                layer_id,
                name: string(args, "name")?,
            },
            "reorder" => L::ReorderLayer {
                layer_id,
                index: required(index(args)?, "index")?,
            },
            "set" => {
                ensure!(
                    args.get("visible").is_some() || args.get("opacity").is_some(),
                    "set requires visible or opacity"
                );
                L::UpdateLayer {
                    layer_id,
                    visible: args
                        .get("visible")
                        .map(|v| required(v.as_bool(), "visible"))
                        .transpose()?,
                    opacity: args
                        .get("opacity")
                        .map(|v| required(v.as_f64(), "opacity"))
                        .transpose()?,
                }
            }
            _ => unreachable!(),
        };
        if let L::DuplicateLayer { new_layer_id, .. } = &operation {
            self.targets
                .push(json!({"layer_id":new_layer_id,"created":true}));
        }
        self.push(Operation::Layer(operation))
    }
    fn frame_command(&mut self, op: &str, args: &Value) -> Result<()> {
        use crate::edit::FrameOperation as F;
        let allowed: &[&str] = match op {
            "add" => &["op", "frame_id", "count", "duration_ms", "index"],
            "duplicate" => &["op", "frame", "frame_id", "new_frame_id", "count", "index"],
            "set_duration" => &[
                "op",
                "frame",
                "frame_id",
                "frames",
                "duration_ms",
                "durations",
            ],
            "delete" => &["op", "frame", "frame_id"],
            "rename" => &["op", "frame", "frame_id", "name"],
            "reorder" => &["op", "frame", "frame_id", "index"],
            _ => {
                return fail(
                    "capability_unavailable",
                    format!("frame.{op} is unavailable"),
                    json!({}),
                );
            }
        };
        fields(args, allowed)?;
        if op == "add" || op == "duplicate" {
            let count = args
                .get("count")
                .map(|v| required(v.as_u64(), "count"))
                .transpose()?
                .unwrap_or(1);
            if count == 0 || count > 10000 || self.operations.len() as u64 + count > 10000 {
                return fail(
                    "resource_limit",
                    "count must fit a 1..10000 operation transaction",
                    json!({}),
                );
            }
            ensure!(
                count == 1
                    || args
                        .get(if op == "add" {
                            "frame_id"
                        } else {
                            "new_frame_id"
                        })
                        .is_none(),
                "Explicit new ID requires count 1"
            );
            let source = if op == "duplicate" {
                Some(self.frame(args)?)
            } else {
                None
            };
            let insert = match index(args)? {
                Some(index) => Some(index),
                None => source
                    .as_ref()
                    .map(|source| {
                        crate::edit::frame_index(&self.candidate, source).map(|index| index + 1)
                    })
                    .transpose()?,
            };
            for offset in 0..count as usize {
                let key = if op == "add" {
                    "frame_id"
                } else {
                    "new_frame_id"
                };
                let id = args
                    .get(key)
                    .map(|v| required(v.as_str(), key).map(str::to_owned))
                    .transpose()?
                    .unwrap_or_else(|| self.fresh("frame"));
                self.targets
                    .push(json!({"frame_id":id,"source_frame_id":source,"created":true}));
                let index = insert
                    .map(|i| {
                        i.checked_add(offset)
                            .ok_or_else(|| anyhow::anyhow!("index overflow"))
                    })
                    .transpose()?;
                let operation = if let Some(source) = &source {
                    F::DuplicateFrame {
                        frame_id: source.clone(),
                        new_frame_id: id,
                        index,
                    }
                } else {
                    F::AddFrame {
                        frame_id: id,
                        duration_ms: args
                            .get("duration_ms")
                            .map(|_| number(args, "duration_ms"))
                            .transpose()?
                            .unwrap_or(100.0),
                        index,
                    }
                };
                self.push(Operation::Frame(operation))?;
            }
            return Ok(());
        }
        if op == "set_duration" && (args.get("frames").is_some() || args.get("durations").is_some())
        {
            ensure!(
                args.get("frame").is_none() && args.get("frame_id").is_none(),
                "Bulk timing cannot be combined with frame/frame_id"
            );
            let ids = if let Some(frames) = args.get("frames") {
                let frames = required(frames.as_array(), "frames")?;
                ensure!(
                    !frames.is_empty() && frames.len() <= 10000,
                    "frames must contain 1..10000 positions"
                );
                frames
                    .iter()
                    .map(|f| self.frame(&json!({"frame":f})))
                    .collect::<Result<Vec<_>>>()?
            } else {
                self.original
                    .frames
                    .iter()
                    .map(|f| crate::edit::frame_id(f).unwrap().to_owned())
                    .collect()
            };
            let durations = if let Some(durations) = args.get("durations") {
                ensure!(
                    args.get("duration_ms").is_none(),
                    "durations and duration_ms are mutually exclusive"
                );
                let durations = required(durations.as_array(), "durations")?;
                ensure!(
                    durations.len() == ids.len(),
                    "durations must match the targeted frame count"
                );
                durations
                    .iter()
                    .map(|v| required(v.as_f64(), "durations[]"))
                    .collect::<Result<Vec<_>>>()?
            } else {
                vec![number(args, "duration_ms")?; ids.len()]
            };
            for (frame_id, duration_ms) in ids.into_iter().zip(durations) {
                self.targets.push(json!({"frame_id":frame_id}));
                self.push(Operation::Frame(F::SetFrameDuration {
                    frame_id,
                    duration_ms,
                }))?;
            }
            return Ok(());
        }
        let frame_id = self.frame(args)?;
        self.targets.push(json!({"frame_id":frame_id}));
        self.push(Operation::Frame(match op {
            "set_duration" => F::SetFrameDuration {
                frame_id,
                duration_ms: number(args, "duration_ms")?,
            },
            "delete" => F::RemoveFrame { frame_id },
            "rename" => F::RenameFrame {
                frame_id,
                name: string(args, "name")?,
            },
            "reorder" => F::ReorderFrame {
                frame_id,
                index: required(index(args)?, "index")?,
            },
            _ => unreachable!(),
        }))
    }
}

pub(super) fn structure(state: &mut State, tool: &str, args: Structure) -> CallToolResult {
    let op = args.command["op"].as_str().unwrap_or("default").to_owned();
    let mut targets = json!([]);
    let value = (|| {
        let id = project(state, args.project_id.as_deref())?;
        if op == "list" {
            fields(&args.command, &["op"])?;
            let mut info = state.session.info(&id)?;
            for (index, frame) in info["frames"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                frame["frame"] = json!(index + 1);
            }
            return Ok(
                json!({if tool=="layer" {"layers"} else {"frames"}:info[if tool=="layer" {"layers"} else {"frames"}]}),
            );
        }
        let revision = guard(state, &id, args.expected_revision)?;
        let mut planner = Planner::new(state, &id)?;
        if op == "batch" {
            fields(&args.command, &["op", "ops"])?;
            let ops = required(args.command["ops"].as_array(), "ops")?;
            ensure!(
                !ops.is_empty() && ops.len() <= 10000,
                "batch must contain 1..10000 operations"
            );
            for entry in ops {
                planner.command(tool, entry)?;
            }
        } else {
            planner.command(tool, &args.command)?;
        }
        targets = json!(planner.targets);
        state.apply(&id, revision, planner.operations)
    })();
    envelope(state, tool, &op, targets, value)
}
pub(super) fn batch(state: &mut State, args: Batch) -> CallToolResult {
    let mut targets = json!([]);
    let value = (|| {
        let id = project(state, args.project_id.as_deref())?;
        let revision = guard(state, &id, args.expected_revision)?;
        if args.operations.is_empty() || args.operations.len() > 10000 {
            return fail(
                "resource_limit",
                "batch must contain 1..10000 operations",
                json!({}),
            );
        }
        let mut planner = Planner::new(state, &id)?;
        for entry in args.operations {
            planner.command(&entry.tool, &entry.arguments)?;
        }
        targets = json!(planner.targets);
        state.apply(&id, revision, planner.operations)
    })();
    envelope(state, "edit_batch", "default", targets, value)
}

/// Generate workflow schemas from executor schemas, replacing native operation
/// discriminators and adding workflow selectors/counts. No legacy wire schemas.
pub(super) fn structure_schema(tool: &str) -> Arc<serde_json::Map<String, Value>> {
    let native = if tool == "layer" {
        serde_json::to_value(schemars::schema_for!(crate::edit::LayerOperation)).unwrap()
    } else {
        serde_json::to_value(schemars::schema_for!(crate::edit::FrameOperation)).unwrap()
    };
    let mut commands = Vec::new();
    for variant in native["oneOf"]
        .as_array()
        .or_else(|| native["anyOf"].as_array())
        .unwrap()
    {
        let mut variant = variant.clone();
        let kind = variant["properties"]["type"]["const"]
            .as_str()
            .or_else(|| variant["properties"]["type"]["enum"][0].as_str())
            .unwrap();
        let op = match kind {
            "add_layer" => "create",
            "duplicate_layer" => "duplicate",
            "remove_layer" => "delete",
            "rename_layer" => "rename",
            "reorder_layer" => "reorder",
            "update_layer" => "set",
            "add_frame" => "add",
            "duplicate_frame" => "duplicate",
            "remove_frame" => "delete",
            "rename_frame" => "rename",
            "reorder_frame" => "reorder",
            "set_frame_duration" => "set_duration",
            other => other,
        }
        .to_owned();
        let mut required = variant["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|k| k.as_str() != Some("type"))
            .cloned()
            .collect::<Vec<_>>();
        let props = variant["properties"].as_object_mut().unwrap();
        props.remove("type");
        props.insert("op".into(), json!({"type":"string","const":op}));
        required.push(json!("op"));
        if tool == "layer"
            && ["create", "duplicate", "delete", "rename", "reorder", "set"].contains(&op.as_str())
        {
            required.retain(|k| k != "layer_id" && k != "new_layer_id");
            if op != "create" {
                props.insert("layer".into(),json!({"type":"string","description":"Exact case-sensitive layer name; mutually exclusive with layer_id"}));
            }
        }
        if tool == "frame" {
            required.retain(|k| k != "frame_id" && k != "new_frame_id");
            if op != "add" {
                props.insert("frame".into(),json!({"type":"integer","minimum":1,"description":"One-based timeline position; mutually exclusive with frame_id"}));
            }
            if op == "add" || op == "duplicate" {
                props.insert(
                    "count".into(),
                    json!({"type":"integer","minimum":1,"maximum":10000,"default":1}),
                );
            }
            if op == "add" {
                required.retain(|k| k != "duration_ms");
            }
            if op == "set_duration" {
                required.retain(|k| k != "duration_ms");
                props.insert("durations".into(),json!({"type":"array","minItems":1,"maxItems":10000,"items":{"type":"number","exclusiveMinimum":0},"description":"One duration per targeted frame; all frames when frames is omitted"}));
                props.insert("frames".into(),json!({"type":"array","minItems":1,"maxItems":10000,"items":{"type":"integer","minimum":1},"description":"Explicit positions; mutually exclusive with frame/frame_id"}));
            }
        }
        variant["required"] = json!(required);
        commands.push(variant);
    }
    let mut top = commands.clone();
    top.push(json!({"type":"object","properties":{"op":{"const":"batch","type":"string"},"ops":{"type":"array","minItems":1,"maxItems":10000,"items":{"oneOf":commands}}},"required":["op","ops"],"additionalProperties":false}));
    for command in &mut top {
        command["properties"]["project_id"] = json!({"type":"string"});
        command["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
        command["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("expected_revision"));
    }
    top.push(json!({"type":"object","properties":{"op":{"const":"list","type":"string"},"project_id":{"type":"string"}},"required":["op"],"additionalProperties":false}));
    let mut schema = json!({"type":"object","oneOf":top});
    if let Some(defs) = native.get("$defs") {
        schema["$defs"] = defs.clone();
    }
    Arc::new(schema.as_object().unwrap().clone())
}
pub(super) fn output_schema() -> Arc<serde_json::Map<String, Value>> {
    Arc::new(json!({"type":"object","properties":{
        "contract":{"const":CONTRACT,"type":"string"},"tool":{"type":"string"},"op":{"type":"string"},
        "project_id":{"type":"string"},"revision":{"type":"integer","minimum":0},
        "resolved_targets":{"type":"array","items":{"type":"object"}},"result":{"type":"object"},"error":{"type":"object"}
    },"required":["contract","tool","op"],"oneOf":[{"required":["result","resolved_targets"]},{"required":["error"]}]}).as_object().unwrap().clone())
}
pub(super) fn application_owned(state: &State, tool: &str, op: &str) -> CallToolResult {
    envelope(
        state,
        tool,
        op,
        json!([]),
        fail(
            "application_owned",
            "Use the Pixen application for this file or lifecycle action",
            json!({}),
        ),
    )
}
pub(super) fn history(state: &mut State, args: InfoRevision, redo: bool) -> CallToolResult {
    let value = (|| {
        let id = project(state, args.project_id.as_deref())?;
        let revision = guard(state, &id, args.expected_revision)?;
        state.history(&id, revision, redo)
    })();
    envelope(
        state,
        if redo { "redo" } else { "undo" },
        "default",
        json!([]),
        value,
    )
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct InfoRevision {
    project_id: Option<String>,
    expected_revision: Option<u64>,
}

pub(super) fn batch_schema() -> Arc<serde_json::Map<String, Value>> {
    let mut definitions = serde_json::Map::new();
    let mut variants = Vec::new();
    for tool in [
        "layer",
        "frame",
        "draw",
        "edit_palette",
        "copy_region",
        "transform_region",
    ] {
        let mut arguments = match tool {
            "layer" | "frame" => Value::Object((*structure_schema(tool)).clone()),
            "draw" => serde_json::to_value(schemars::schema_for!(Draw)).unwrap(),
            "edit_palette" => serde_json::to_value(schemars::schema_for!(EditPalette)).unwrap(),
            "copy_region" => serde_json::to_value(schemars::schema_for!(CopyRegion)).unwrap(),
            _ => serde_json::to_value(schemars::schema_for!(TransformRegion)).unwrap(),
        };
        if let Some(defs) = arguments.as_object_mut().unwrap().remove("$defs") {
            definitions.extend(defs.as_object().unwrap().clone());
        }
        let strip = |schema: &mut Value| {
            if let Some(props) = schema["properties"].as_object_mut() {
                props.remove("project_id");
                props.remove("expected_revision");
            }
            if let Some(required) = schema["required"].as_array_mut() {
                required.retain(|v| v != "project_id" && v != "expected_revision");
            }
        };
        if tool == "layer" || tool == "frame" {
            let variants = arguments["oneOf"].as_array_mut().unwrap();
            variants.retain(|s| {
                !["list", "batch"]
                    .iter()
                    .any(|op| s["properties"]["op"]["const"] == *op)
            });
            for variant in variants {
                strip(variant);
            }
        } else {
            strip(&mut arguments);
        }
        variants.push(json!({"type":"object","properties":{"tool":{"type":"string","const":tool},"arguments":arguments},"required":["tool","arguments"],"additionalProperties":false}));
    }
    Arc::new(json!({"type":"object","properties":{"project_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":0},"operations":{"type":"array","minItems":1,"maxItems":10000,"items":{"oneOf":variants}}},"required":["expected_revision","operations"],"additionalProperties":false,"$defs":definitions}).as_object().unwrap().clone())
}
pub(super) fn revision_schema<T: JsonSchema>() -> Arc<serde_json::Map<String, Value>> {
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    if schema.get("required").is_none() {
        schema["required"] = json!([]);
    }
    schema["required"]
        .as_array_mut()
        .unwrap()
        .push(json!("expected_revision"));
    schema["properties"]["expected_revision"] = json!({"type":"integer","minimum":0});
    Arc::new(schema.as_object().unwrap().clone())
}
