//! Codex adapter. Constructed only by explicit AI activation, never application startup.
use crate::mcp::desktop::{Bridge, Desktop};
#[path = "agent_process.rs"]
mod process;
use anyhow::{Context, Result, ensure};
use process::Process;
#[path = "agent_attachments.rs"]
pub mod attachments;
#[path = "agent_models.rs"]
pub mod models;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
pub struct Wake(pub Option<Arc<dyn Fn() + Send + Sync>>);
impl Wake {
    fn request_repaint(&self) {
        if let Some(wake) = &self.0 {
            wake();
        }
    }
}

pub enum Action {
    Send(String),
    SendReferences(String, Vec<attachments::Attachment>),
    Configure(models::Selection),
    Stop,
    NewConversation,
    Rewind(u64),
    SaveAs(PathBuf),
    Save,
    CreateArtwork(u32, u32),
    ResizeCanvas(u32, u32),
    Approval(Value, bool),
    Shutdown,
}
pub enum Event {
    Status(String),
    ApplicationStatus(String),
    Viewer(crate::mcp::desktop::DesktopEvent),
    Models(Vec<models::Model>),
    Account(String),
    Ready,
    Working,
    TurnCheckpoint(u64),
    Rewound(u64),
    Thought {
        id: String,
        text: String,
        done: bool,
        delta: bool,
    },
    Finished,
    BaselineAvailable(bool),
    DocumentDone,
    Message {
        id: String,
        text: String,
        delta: bool,
    },
    Approval {
        id: Value,
        description: String,
    },
    Resolved(Value),
    Error(String),
    Saved {
        path: PathBuf,
        revision: u64,
    },
    Canvas {
        revision: u64,
        sprite: Box<crate::model::Sprite>,
        recovery: PathBuf,
    },
}
pub struct Agent {
    pub actions: Sender<Action>,
    pub events: Receiver<Event>,
    pub canvas: Arc<Mutex<Option<Event>>>,
}

/// Document lifetime is independent of the Codex process and its connection.
#[derive(Clone, Default)]
pub struct Workspace(Arc<Mutex<Option<Desktop>>>);
impl Workspace {
    #[cfg(test)]
    pub fn is_prepared(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
    fn prepare(&self, source: Option<&std::path::Path>) -> Result<Desktop> {
        self.prepare_document(source, false)
    }
    fn prepare_document(&self, source: Option<&std::path::Path>, saving: bool) -> Result<Desktop> {
        let mut active = self.0.lock().unwrap();
        if let Some(desktop) = &*active {
            return Ok(desktop.clone());
        }
        let root = tempfile::Builder::new()
            .prefix("pixloom-art-")
            .tempdir()?
            .keep();
        let desktop = if saving {
            Desktop::for_save(root, source)?
        } else {
            Desktop::new(root, source)?
        };
        *active = Some(desktop.clone());
        Ok(desktop)
    }

    pub fn document_action_from(
        &self,
        action: Action,
        source: Option<PathBuf>,
        wake: Wake,
    ) -> Receiver<Event> {
        let (events, receiver) = mpsc::channel();
        let workspace = self.clone();
        std::thread::spawn(move || {
            let output = Output {
                events,
                wake,
                canvas: None,
            };
            let result = (|| -> Result<()> {
                let desktop = workspace
                    .prepare_document(source.as_deref(), matches!(action, Action::Save))?;
                match action {
                    Action::SaveAs(path) => {
                        let saved = desktop.save_as(&path)?;
                        output.send(Event::Saved {
                            path,
                            revision: saved["revision"].as_u64().unwrap_or(0),
                        });
                    }
                    Action::Save => {
                        let saved = desktop.save()?;
                        output.send(Event::Saved {
                            path: PathBuf::from(
                                saved["saved_path"].as_str().context("missing saved path")?,
                            ),
                            revision: saved["revision"].as_u64().unwrap_or(0),
                        });
                    }
                    Action::CreateArtwork(width, height) => {
                        desktop.create_artwork(width, height)?;
                    }
                    Action::ResizeCanvas(width, height) => {
                        desktop.resize_canvas(width, height)?;
                    }
                    _ => anyhow::bail!("Unsupported document action"),
                }
                if let Some((revision, sprite, recovery)) = desktop.snapshot(None) {
                    output.send(Event::Canvas {
                        revision,
                        sprite: Box::new(sprite),
                        recovery,
                    });
                }
                output.send(Event::BaselineAvailable(desktop.can_revert()));
                Ok(())
            })();
            if let Err(error) = result {
                output.send(Event::ApplicationStatus(format!(
                    "Document action failed: {error:#}"
                )));
            }
            output.send(Event::DocumentDone);
        });
        receiver
    }
}

impl Agent {
    #[cfg(test)]
    pub fn start(source: Option<PathBuf>, wake: Wake) -> Self {
        Self::start_in(
            source,
            wake,
            Workspace::default(),
            models::Selection::default(),
        )
    }
    pub fn start_in(
        source: Option<PathBuf>,
        wake: Wake,
        workspace: Workspace,
        selection: models::Selection,
    ) -> Self {
        let (actions, commands) = mpsc::channel();
        let (events, receiver) = mpsc::channel();
        let canvas = Arc::new(Mutex::new(None));
        let latest_canvas = canvas.clone();
        std::thread::spawn(move || {
            let output = Output {
                events,
                wake,
                canvas: Some(latest_canvas),
            };
            if let Err(error) = run(commands, &output, source, workspace, selection) {
                output.send(Event::Error(format!("{error:#}")));
            }
        });
        Self {
            actions,
            events: receiver,
            canvas,
        }
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.actions.send(Action::Shutdown);
    }
}
struct Output {
    events: Sender<Event>,
    wake: Wake,
    canvas: Option<Arc<Mutex<Option<Event>>>>,
}
impl Output {
    fn send(&self, event: Event) {
        if matches!(&event, Event::Canvas { .. })
            && let Some(canvas) = &self.canvas
        {
            // A minimized UI needs only the newest artwork, not a queue of full sprites.
            *canvas.lock().unwrap() = Some(event);
        } else {
            let _ = self.events.send(event);
        }
        self.wake.request_repaint();
    }
}

#[derive(Default)]
struct Protocol {
    thread: Option<String>,
    turn: Option<String>,
    busy: bool,
    stop_requested: bool,
}
impl Protocol {
    fn interrupt(&mut self) -> Option<Value> {
        self.stop_requested = true;
        Some(
            json!({"method":"turn/interrupt","params":{"threadId":self.thread.as_ref()?,"turnId":self.turn.as_ref()?}}),
        )
    }
    fn notification(&mut self, message: &Value) -> Option<Value> {
        let p = &message["params"];
        if p["threadId"].as_str() != self.thread.as_deref() {
            return None;
        }
        match message["method"].as_str()? {
            "turn/started" => {
                self.turn = p["turn"]["id"].as_str().map(str::to_owned);
                if self.stop_requested {
                    return self.interrupt();
                }
            }
            "turn/completed" => {
                self.busy = false;
                self.turn = None;
                self.stop_requested = false;
            }
            _ => {}
        }
        None
    }
}

fn send(child: &mut Child, value: Value) -> Result<()> {
    let input = child.stdin.as_mut().context("Codex input closed")?;
    writeln!(input, "{value}")?;
    input.flush()?;
    Ok(())
}
fn request(
    child: &mut Child,
    pending: &mut BTreeMap<u64, (String, Instant)>,
    id: &mut u64,
    method: &str,
    params: Value,
) -> Result<()> {
    *id += 1;
    send(child, json!({"id":*id,"method":method,"params":params}))?;
    pending.insert(*id, (method.into(), Instant::now()));
    Ok(())
}

fn thread_params(
    config: &Value,
    root: &std::path::Path,
    bridge: &Bridge,
    executable: &std::path::Path,
) -> Value {
    let mut overrides = serde_json::Map::new();
    // Disable inherited MCP servers for this thread without touching user settings.
    // Replace normalized config tables with minimal inert entries. This avoids
    // round-tripping nullable TOML fields or copying inherited credentials.
    let mut servers: serde_json::Map<String, Value> = config["mcp_servers"]
        .as_object()
        .map(|servers| {
            servers
                .keys()
                .map(|name| (name.clone(), json!({"enabled":false})))
                .collect()
        })
        .unwrap_or_default();
    let server_name = format!("pixloom_desktop_{}", &bridge.token[..12]);
    servers.insert(server_name, json!({
        "command":executable,"args":["--bridge",bridge.address.to_string(),"--bridge-token",bridge.token],
        "enabled":true,"required":true,"startup_timeout_sec":15,"tool_timeout_sec":120,
        "default_tools_approval_mode":"approve",
    }));
    overrides.insert("mcp_servers".into(), Value::Object(servers));
    overrides.insert("features.shell_tool".into(), json!(false));
    overrides.insert("features.apps".into(), json!(false));
    overrides.insert("features.plugins".into(), json!(false));
    overrides.insert("web_search".into(), json!("disabled"));
    let plugins: serde_json::Map<String, Value> = config["plugins"]
        .as_object()
        .map(|plugins| {
            plugins
                .keys()
                .map(|name| (name.clone(), json!({"enabled":false})))
                .collect()
        })
        .unwrap_or_default();
    overrides.insert("plugins".into(), Value::Object(plugins));
    let mut apps: serde_json::Map<String, Value> = config["apps"]
        .as_object()
        .map(|apps| {
            apps.keys()
                .map(|name| (name.clone(), json!({"enabled":false})))
                .collect()
        })
        .unwrap_or_default();
    apps.insert("_default".into(), json!({"enabled":false}));
    overrides.insert("apps".into(), Value::Object(apps));
    json!({"cwd":root,"sandbox":"read-only","approvalPolicy":"on-request","config":overrides,
    "developerInstructions": concat!(
        "You are the pixel artist inside Pixloom. Respond conversationally and refine the current artwork through successive messages. ",
        "Use only the connected pixloom_desktop_* MCP server's tools for artwork. Do not write files, run commands, or use other integrations. ",
        "The app owns project saving and the live canvas. Never finish, replace, export, or open another viewer. ",
        "Use get_project_info and explicit revision checks. New artwork uses create_project with an output_path of artwork.pixloom (the app manages its actual location). ",
        "Use transparent #RRGGBBAA palette entries, intentional pixel clusters, clear silhouettes, controlled shading, and editable layers. ",
        "Block out, inspect_project to see actual pixels, refine, and inspect again. Animation needs distinct poses; inspect get_contact_sheet. ",
        "Use report_activity for concise progress. Images are inspection evidence, not aesthetic scores. Ask questions in ordinary chat when needed."
    )})
}

fn run(
    commands: Receiver<Action>,
    output: &Output,
    source: Option<PathBuf>,
    workspace: Workspace,
    mut selection: models::Selection,
) -> Result<()> {
    let mut model_catalog = Vec::new();
    let mut active_references = Vec::new();
    output.send(Event::Status("Preparing AI workspace…".into()));
    let desktop = workspace.prepare(source.as_deref())?;
    let root = desktop.root().to_owned();
    struct FinalSnapshot<'a>(&'a Desktop, &'a Output);
    impl Drop for FinalSnapshot<'_> {
        fn drop(&mut self) {
            if let Some((revision, sprite, recovery)) = self.0.snapshot(None) {
                self.1.send(Event::Canvas {
                    revision,
                    sprite: Box::new(sprite),
                    recovery,
                });
            }
            self.1.send(Event::BaselineAvailable(self.0.can_revert()));
        }
    }
    let _final_snapshot = FinalSnapshot(&desktop, output);
    if commands
        .try_iter()
        .any(|action| matches!(action, Action::Shutdown))
    {
        return Ok(());
    }
    let bridge = Bridge::start(desktop.clone())?;
    let mut executable = std::env::current_exe()?.with_file_name(if cfg!(windows) {
        "pixloom-mcp.exe"
    } else {
        "pixloom-mcp"
    });
    if cfg!(test) && !executable.is_file() {
        executable = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(if cfg!(windows) {
                "pixloom-mcp.exe"
            } else {
                "pixloom-mcp"
            });
    }
    ensure!(
        executable.is_file(),
        "Install pixloom-mcp beside Pixloom, then retry AI assistance."
    );
    let mut command = process::app_server_command();
    command
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut process = Process::spawn(&mut command)?;
    let (wire_tx, wire) = mpsc::channel::<Result<Value, String>>();
    let stdout = process
        .0
        .stdout
        .take()
        .context("Codex stdout unavailable")?;
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let parsed = line
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()));
            if wire_tx.send(parsed).is_err() {
                return;
            }
        }
        let _ = wire_tx.send(Err(
            "Codex disconnected. Reconnect to continue working on the recovered artwork.".into(),
        ));
    });
    // Bound diagnostics and drain stderr so a noisy subprocess cannot deadlock.
    let stderr = process.0.stderr.take().unwrap();
    let diagnostics = Arc::new(Mutex::new(VecDeque::new()));
    let diagnostic_writer = diagnostics.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut tail = diagnostic_writer.lock().unwrap();
            if tail.len() == 8 {
                tail.pop_front();
            }
            tail.push_back(line.chars().take(1500).collect::<String>());
        }
    });
    let mut protocol = Protocol::default();
    let mut pending = BTreeMap::new();
    let mut id = 0;
    let mut config = Value::Null;
    let mut revision = None;
    let mut next_snapshot = Instant::now();
    let mut ready = false;
    let mut stopping_since = None;
    let mut rewinding = None;
    let mut current_checkpoint = None;
    let mut turn_ids = BTreeMap::new();
    request(
        &mut process.0,
        &mut pending,
        &mut id,
        "initialize",
        json!({"clientInfo":{"name":"pixloom","title":"Pixloom","version":env!("CARGO_PKG_VERSION")}}),
    )?;
    loop {
        while let Ok(action) = commands.try_recv() {
            let action = match action {
                Action::Send(text) => Action::SendReferences(text, Vec::new()),
                other => other,
            };
            match action {
                Action::Shutdown => return Ok(()),
                Action::Configure(next) => selection = next,
                Action::Stop if protocol.busy => {
                    stopping_since.get_or_insert(Instant::now());
                    if let Some(msg) = protocol.interrupt() {
                        request(
                            &mut process.0,
                            &mut pending,
                            &mut id,
                            "turn/interrupt",
                            msg["params"].clone(),
                        )?;
                    }
                    output.send(Event::Status("Stopping…".into()));
                }
                Action::SendReferences(text, references) if ready && !protocol.busy => {
                    match desktop.begin_turn() {
                        Ok(info) => {
                            active_references.clear();
                            active_references.extend(references);
                            protocol.busy = true;
                            protocol.turn = None;
                            stopping_since = None;
                            protocol.stop_requested = false;
                            let token = desktop.checkpoint_turn()?;
                            current_checkpoint = Some(token);
                            output.send(Event::TurnCheckpoint(token));
                            output.send(Event::Working);
                            output.send(Event::BaselineAvailable(desktop.can_revert()));
                            request(&mut process.0, &mut pending, &mut id, "turn/start", {
                                let mut params = selection.turn(
                                    &protocol.thread,
                                    format!("{text}\n\n[Pixloom active project context: {info}]"),
                                );
                                params["input"].as_array_mut().unwrap().extend(
                                    active_references.iter().map(attachments::Attachment::input),
                                );
                                params
                            })?;
                        }
                        Err(e) => output.send(Event::Error(format!(
                            "Cannot prepare recoverable turn: {e:#}"
                        ))),
                    }
                }
                Action::Rewind(token) if !protocol.busy && ready => {
                    match desktop.rewind_count(token).and_then(|_| {
                        turn_ids
                            .get(&token)
                            .cloned()
                            .context("No completed Codex turn for this message")
                    }) {
                        Ok(turn_id) => {
                            rewinding = Some(token);
                            ready = false;
                            request(
                                &mut process.0,
                                &mut pending,
                                &mut id,
                                "thread/revert",
                                json!({"threadId":protocol.thread,"beforeTurnId":turn_id}),
                            )?;
                        }
                        Err(error) => {
                            output.send(Event::ApplicationStatus(format!(
                                "Cannot revert: {error:#}"
                            )));
                            output.send(Event::Finished);
                        }
                    }
                }
                Action::NewConversation if !protocol.busy && ready => {
                    desktop.clear_turns();
                    turn_ids.clear();
                    ready = false;
                    request(
                        &mut process.0,
                        &mut pending,
                        &mut id,
                        "thread/start",
                        selection.apply_thread(thread_params(&config, &root, &bridge, &executable)),
                    )?;
                }
                Action::Approval(request_id, accept) => {
                    send(
                        &mut process.0,
                        json!({"id":request_id,"result":{"decision":if accept {"accept"} else {"decline"}}}),
                    )?;
                    output.send(Event::Resolved(request_id));
                }
                _ => {}
            }
        }
        if next_snapshot.elapsed() >= Duration::from_millis(100) {
            for mut event in desktop.drain_events() {
                if !protocol.busy
                    && let crate::mcp::desktop::DesktopEvent::Activity(activity) = &mut event
                {
                    activity.busy = false;
                    activity.label = "Artist finished".into();
                }
                output.send(Event::Viewer(event));
            }
            if let Some((current, sprite, recovery)) = desktop.snapshot(revision) {
                revision = Some(current);
                output.send(Event::Canvas {
                    revision: current,
                    sprite: Box::new(sprite),
                    recovery,
                });
                output.send(Event::BaselineAvailable(desktop.can_revert()));
            }
            next_snapshot = Instant::now();
        }
        if pending
            .values()
            .any(|(_, since)| since.elapsed() > Duration::from_secs(60))
        {
            anyhow::bail!("Codex did not respond within 60 seconds. Reconnect to retry.");
        }
        if protocol.busy
            && stopping_since
                .is_some_and(|since: Instant| since.elapsed() > Duration::from_secs(10))
        {
            anyhow::bail!(
                "Codex did not stop within 10 seconds. Its connection was closed; completed edits are recovered."
            );
        }
        match wire.recv_timeout(Duration::from_millis(15)) {
            Ok(Ok(message)) => {
                if let Some(method) = message["method"].as_str() {
                    if message.get("id").is_some() {
                        if matches!(
                            method,
                            "item/commandExecution/requestApproval"
                                | "item/fileChange/requestApproval"
                        ) {
                            output.send(Event::Approval {
                                id: message["id"].clone(),
                                description: format!(
                                    "{}\n{}",
                                    message["params"]["reason"].as_str().unwrap_or(
                                        "Codex requests permission for an additional action."
                                    ),
                                    message["params"]["command"]
                                        .as_str()
                                        .or(message["params"]["grantRoot"].as_str())
                                        .unwrap_or("File changes requested by the agent")
                                ),
                            });
                        } else {
                            send(
                                &mut process.0,
                                json!({"id":message["id"],"error":{"code":-32601,"message":"This request is not supported by Pixloom. Ask the user in the conversation instead."}}),
                            )?;
                        }
                        continue;
                    }
                    if let Some(thread) = message["params"]["threadId"].as_str()
                        && Some(thread) != protocol.thread.as_deref()
                    {
                        continue;
                    }
                    if let Some(msg) = protocol.notification(&message) {
                        request(
                            &mut process.0,
                            &mut pending,
                            &mut id,
                            "turn/interrupt",
                            msg["params"].clone(),
                        )?;
                    }
                    let p = &message["params"];
                    match method {
                        "item/reasoning/summaryTextDelta" => output.send(Event::Thought {
                            id: p["itemId"].as_str().unwrap_or("thinking").into(),
                            text: p["delta"].as_str().unwrap_or_default().into(),
                            done: false,
                            delta: true,
                        }),
                        "item/started" if p["item"]["type"] == "reasoning" => {
                            output.send(Event::Thought {
                                id: p["item"]["id"].as_str().unwrap_or("thinking").into(),
                                text: String::new(),
                                done: false,
                                delta: true,
                            })
                        }
                        "item/completed" if p["item"]["type"] == "reasoning" => {
                            output.send(Event::Thought {
                                id: p["item"]["id"].as_str().unwrap_or("thinking").into(),
                                text: p["item"]["summary"]
                                    .as_array()
                                    .map(|s| {
                                        s.iter()
                                            .filter_map(Value::as_str)
                                            .collect::<Vec<_>>()
                                            .join("\n\n")
                                    })
                                    .unwrap_or_default(),
                                done: true,
                                delta: false,
                            })
                        }
                        "item/agentMessage/delta" => output.send(Event::Message {
                            id: p["itemId"].as_str().unwrap_or("message").into(),
                            text: p["delta"].as_str().unwrap_or_default().into(),
                            delta: true,
                        }),
                        "item/completed" if p["item"]["type"] == "agentMessage" => {
                            output.send(Event::Message {
                                id: p["item"]["id"].as_str().unwrap_or("message").into(),
                                text: p["item"]["text"].as_str().unwrap_or_default().into(),
                                delta: false,
                            })
                        }
                        "item/started" if p["item"]["type"] == "mcpToolCall" => {
                            output.send(Event::Status(format!(
                                "Working · {}",
                                p["item"]["tool"].as_str().unwrap_or("drawing")
                            )))
                        }
                        "turn/completed" => {
                            if let Some(error) = p["turn"].get("error").filter(|v| !v.is_null()) {
                                output.send(Event::Status(format!("Turn failed: {error}")));
                            } else {
                                output.send(Event::Status(
                                    if p["turn"]["status"] == "interrupted" {
                                        "Stopped. Completed edits are recovered."
                                    } else {
                                        "Ready · recovery saved"
                                    }
                                    .into(),
                                ));
                            }
                            output.send(Event::Finished);
                        }
                        "serverRequest/resolved" => {
                            output.send(Event::Resolved(p["requestId"].clone()))
                        }
                        "error" => output.send(Event::Status(format!(
                            "Codex: {}",
                            p["error"]["message"].as_str().unwrap_or("request failed")
                        ))),
                        _ => {}
                    }
                } else if let Some(response_id) = message["id"].as_u64() {
                    let Some((method, _)) = pending.remove(&response_id) else {
                        continue;
                    };
                    if let Some(error) = message.get("error") {
                        if method == "turn/start" || method == "thread/revert" {
                            ready = true;
                            rewinding = None;
                            protocol.busy = false;
                            output.send(Event::Finished);
                            output.send(Event::Status(format!("Codex request failed: {error}")));
                            continue;
                        }
                        anyhow::bail!("Codex {method}: {error}");
                    }
                    let result = &message["result"];
                    match method.as_str() {
                        "thread/revert" => {
                            if let Some(token) = rewinding.take() {
                                match desktop.rewind(token) {
                                    Ok(()) => {
                                        revision = None;
                                        if let Some((current, sprite, recovery)) =
                                            desktop.snapshot(None)
                                        {
                                            revision = Some(current);
                                            output.send(Event::Canvas {
                                                revision: current,
                                                sprite: Box::new(sprite),
                                                recovery,
                                            });
                                        }
                                        turn_ids.retain(|id, _| *id < token);
                                        output.send(Event::Rewound(token));
                                    }
                                    Err(error) => {
                                        anyhow::bail!(
                                            "Conversation history reverted, but artwork recovery failed: {error:#}. Reconnect before continuing."
                                        );
                                    }
                                }
                            }
                            ready = true;
                            output.send(Event::Finished);
                        }
                        "initialize" => {
                            send(&mut process.0, json!({"method":"initialized","params":{}}))?;
                            output.send(Event::Status("Checking Codex login…".into()));
                            request(
                                &mut process.0,
                                &mut pending,
                                &mut id,
                                "account/read",
                                json!({"refreshToken":false}),
                            )?;
                        }
                        "account/read" => {
                            ensure!(
                                !result["account"].is_null()
                                    || result["requiresOpenaiAuth"] == false,
                                "Codex is not signed in. Run codex login in a terminal, then reconnect."
                            );
                            output.send(Event::Account(
                                match result["account"]["type"].as_str() {
                                    Some("chatgpt") => "ChatGPT subscription",
                                    Some("apiKey") => "API key · usage-based billing",
                                    _ => "Configured Codex provider",
                                }
                                .into(),
                            ));
                            request(
                                &mut process.0,
                                &mut pending,
                                &mut id,
                                "config/read",
                                json!({"includeLayers":false}),
                            )?;
                        }
                        "config/read" => {
                            config = result["config"].clone();
                            request(
                                &mut process.0,
                                &mut pending,
                                &mut id,
                                "model/list",
                                json!({"limit":100,"includeHidden":false}),
                            )?;
                        }
                        "model/list" => {
                            model_catalog.extend(models::parse_catalog(result, true));
                            if let Some(cursor) = result["nextCursor"].as_str() {
                                request(
                                    &mut process.0,
                                    &mut pending,
                                    &mut id,
                                    "model/list",
                                    json!({"limit":100,"includeHidden":false,"cursor":cursor}),
                                )?;
                            } else {
                                output.send(Event::Models(std::mem::take(&mut model_catalog)));
                                request(
                                    &mut process.0,
                                    &mut pending,
                                    &mut id,
                                    "thread/start",
                                    selection.apply_thread(thread_params(
                                        &config,
                                        &root,
                                        &bridge,
                                        &executable,
                                    )),
                                )?;
                            }
                        }
                        "thread/start" => {
                            desktop.clear_turns();
                            turn_ids.clear();
                            protocol.thread = Some(
                                result["thread"]["id"]
                                    .as_str()
                                    .context("Codex returned no conversation ID")?
                                    .into(),
                            );
                            output.send(Event::Status("Connecting art tools…".into()));
                            request(
                                &mut process.0,
                                &mut pending,
                                &mut id,
                                "mcpServerStatus/list",
                                json!({"threadId":protocol.thread,"limit":100}),
                            )?;
                        }
                        "mcpServerStatus/list" => {
                            let servers = result["data"]
                                .as_array()
                                .context("Codex returned no MCP inventory")?;
                            let name = format!("pixloom_desktop_{}", &bridge.token[..12]);
                            let art = servers
                                .iter()
                                .find(|server| server["name"] == name)
                                .context("Codex did not connect to the active Pixloom canvas")?;
                            ensure!(
                                art["tools"].get("create_project").is_some()
                                    && art["tools"].get("apply_edits").is_some()
                                    && art["tools"].get("inspect_project").is_some(),
                                "Pixloom art tools did not load. Install matching pixloom and pixloom-mcp binaries, then reconnect."
                            );
                            ensure!(
                                servers.iter().all(|server| server["name"] == name
                                    || server["tools"]
                                        .as_object()
                                        .is_none_or(|tools| tools.is_empty())),
                                "Unexpected inherited MCP tools remain enabled: {}",
                                servers
                                    .iter()
                                    .filter(|server| server["name"] != name
                                        && server["tools"]
                                            .as_object()
                                            .is_some_and(|tools| !tools.is_empty()))
                                    .map(|server| format!(
                                        "{} ({})",
                                        server["name"], server["runtimeStatus"]
                                    ))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                            ready = true;
                            output.send(Event::Ready);
                        }
                        "turn/start" => {
                            protocol.turn = result["turn"]["id"].as_str().map(str::to_owned);
                            if let (Some(token), Some(turn)) =
                                (current_checkpoint, protocol.turn.clone())
                            {
                                turn_ids.insert(token, turn);
                            }
                            if protocol.stop_requested
                                && let Some(msg) = protocol.interrupt()
                            {
                                request(
                                    &mut process.0,
                                    &mut pending,
                                    &mut id,
                                    "turn/interrupt",
                                    msg["params"].clone(),
                                )?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Err(error)) => {
                let tail = diagnostics
                    .lock()
                    .unwrap()
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::bail!("{error}\n{tail}");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("Codex disconnected. Reconnect to retry.")
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    #[ignore = "uses existing ChatGPT subscription for one tiny animation turn and conversation revert"]
    fn subscription_artist_animates_and_rewinds_message() {
        let workspace = Workspace::default();
        let desktop = workspace.prepare(None).unwrap();
        desktop.create_artwork(4, 4).unwrap();
        let agent = Agent::start_in(
            None,
            Wake::default(),
            workspace.clone(),
            models::Selection::default(),
        );
        let deadline = Instant::now() + Duration::from_secs(150);
        let mut subscription = false;
        let mut token = None;
        let mut rewinding = false;
        let mut saw_rewound = false;
        loop {
            assert!(
                Instant::now() < deadline,
                "animation/revert smoke timed out"
            );
            match agent.events.recv_timeout(Duration::from_secs(3)) {
                Ok(Event::Account(account)) => subscription = account == "ChatGPT subscription",
                Ok(Event::Ready) => {
                    assert!(subscription, "This test requires subscription auth");
                    agent.actions.send(Action::Send("Animate the existing 4x4 artwork with exactly two frames, each 100ms. Draw one white pixel at (0,0) on frame 1 and one at (1,0) on frame 2, transparent elsewhere. Use edit_frame and drawing tools, and inspect the contact sheet. Reply briefly.".into())).unwrap();
                }
                Ok(Event::TurnCheckpoint(id)) => token = Some(id),
                Ok(Event::Rewound(_)) => saw_rewound = true,
                Ok(Event::Finished) => {
                    let (_, sprite, _) = desktop.snapshot(None).unwrap();
                    if !rewinding {
                        assert_eq!(sprite.frames.len(), 2);
                        assert_ne!(
                            crate::raster::render(&sprite, 0).unwrap(),
                            crate::raster::render(&sprite, 1).unwrap()
                        );
                        rewinding = true;
                        agent.actions.send(Action::Rewind(token.unwrap())).unwrap();
                    } else {
                        assert!(saw_rewound, "conversation revert must succeed");
                        assert_eq!(sprite.frames.len(), 1);
                        assert!(
                            crate::raster::render(&sprite, 0)
                                .unwrap()
                                .iter()
                                .all(|v| *v == 0)
                        );
                        break;
                    }
                }
                Ok(Event::Approval { .. }) => {
                    panic!("Unexpected approval for isolated artwork server")
                }
                Ok(Event::Error(error)) => panic!("{error}"),
                Ok(Event::Status(status)) if status.contains("request failed") => {
                    panic!("{status}")
                }
                Ok(Event::ApplicationStatus(status)) if status.contains("Cannot") => {
                    panic!("{status}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn thread_config_preserves_literal_names_while_disabling_inherited_integrations() {
        let root = tempfile::tempdir().unwrap();
        let desktop = Desktop::new(root.path().to_owned(), None).unwrap();
        let bridge = Bridge::start(desktop).unwrap();
        let params = thread_params(
            &json!({
                "mcp_servers":{"external.server":{"command":"external","args":[]}},
                "plugins":{"plugin.with.dots":{"enabled":true}},
                "apps":{"connector.with.dots":{"enabled":true}}
            }),
            root.path(),
            &bridge,
            std::path::Path::new("pixloom-mcp"),
        );
        assert_eq!(
            params["config"]["mcp_servers"]["external.server"]["enabled"],
            false
        );
        assert!(
            params["config"]["mcp_servers"]["external.server"]
                .get("command")
                .is_none()
        );
        assert!(
            params["config"]["mcp_servers"]
                .as_object()
                .unwrap()
                .iter()
                .any(|(name, value)| name.starts_with("pixloom_desktop_")
                    && value["enabled"] == true)
        );
        assert_eq!(
            params["config"]["plugins"]["plugin.with.dots"]["enabled"],
            false
        );
        assert_eq!(
            params["config"]["apps"]["connector.with.dots"]["enabled"],
            false
        );
        assert_eq!(params["config"]["features.apps"], false);
        assert_eq!(params["config"]["features.plugins"], false);
    }

    #[test]
    #[ignore = "requires locally installed and authenticated Codex; no inference requested"]
    fn installed_codex_initializes_with_embedded_mcp() {
        let agent = Agent::start(None, Wake::default());
        let until = Instant::now() + Duration::from_secs(60);
        let mut saw_catalog = false;
        loop {
            assert!(Instant::now() < until, "Codex initialization timed out");
            match agent.events.recv_timeout(Duration::from_secs(5)) {
                Ok(Event::Models(catalog)) => {
                    assert!(!catalog.is_empty());
                    assert!(
                        catalog
                            .iter()
                            .any(|m| m.id == "gpt-6.1-sol" && m.efforts.iter().any(|e| e == "low"))
                    );
                    saw_catalog = true;
                }
                Ok(Event::Ready) => {
                    assert!(saw_catalog);
                    break;
                }
                Ok(Event::Error(error)) => panic!("{error}"),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => panic!("{error}"),
            }
        }
    }

    #[test]
    #[ignore = "uses two tiny model turns; requires an existing ChatGPT subscription login"]
    fn subscription_artist_creates_then_refines_artwork() {
        let workspace = Workspace::default();
        let agent = Agent::start_in(
            None,
            Wake::default(),
            workspace.clone(),
            models::Selection::default(),
        );
        let until = Instant::now() + Duration::from_secs(150);
        let mut subscription = false;
        let mut turns = 0;
        loop {
            assert!(Instant::now() < until, "artist smoke test timed out");
            match agent.events.recv_timeout(Duration::from_secs(5)) {
                Ok(Event::Account(account)) => subscription = account == "ChatGPT subscription",
                Ok(Event::Ready) => {
                    assert!(
                        subscription,
                        "This smoke test only uses subscription authentication"
                    );
                    agent.actions.send(Action::Send("Create exactly a 4x4 sprite, palette [#00000000,#FFFFFFFF], with a single white pixel at (0,0) on the first layer/frame and everything else transparent. Use the Pixloom tools and inspect the result. Reply briefly.".into())).unwrap();
                }
                Ok(Event::Finished) => {
                    let desktop = workspace.0.lock().unwrap().clone().unwrap();
                    let (_, sprite, _) = desktop.snapshot(None).unwrap();
                    let rgba = crate::raster::render(&sprite, 0).unwrap();
                    assert_eq!((sprite.width, sprite.height), (4, 4));
                    assert_eq!(&rgba[..4], &[255, 255, 255, 255]);
                    if turns == 0 {
                        turns += 1;
                        agent.actions.send(Action::Send("Keep the existing white pixel. Add one white pixel at (3,3) on the same layer/frame, inspect the result, and reply briefly.".into())).unwrap();
                    } else {
                        assert_eq!(&rgba[60..64], &[255, 255, 255, 255]);
                        desktop.revert().unwrap();
                        let (_, sprite, _) = desktop.snapshot(None).unwrap();
                        assert_eq!(
                            &crate::raster::render(&sprite, 0).unwrap()[60..64],
                            &[0, 0, 0, 0]
                        );
                        break;
                    }
                }
                Ok(Event::Approval { .. }) => {
                    panic!("unexpected tool approval for scoped art tools")
                }
                Ok(Event::Error(error)) => panic!("{error}"),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => panic!("{error}"),
            }
        }
    }

    #[test]
    #[ignore = "one small reference-reading turn; requires ChatGPT subscription and refuses API-key inference"]
    fn subscription_artist_reads_image_and_text_references() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("red.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]))
            .save(&image)
            .unwrap();
        let text = dir.path().join("brief.txt");
        std::fs::write(&text, "Reference key: SLIME42").unwrap();
        let references = vec![
            attachments::Attachment::load(&image).unwrap(),
            attachments::Attachment::load(&text).unwrap(),
        ];
        let agent = Agent::start(None, Wake::default());
        let until = Instant::now() + Duration::from_secs(120);
        let mut subscription = false;
        let mut replies = BTreeMap::<String, String>::new();
        loop {
            assert!(Instant::now() < until, "Reference reading timed out");
            match agent.events.recv_timeout(Duration::from_secs(5)) {
                Ok(Event::Account(account)) => subscription = account == "ChatGPT subscription",
                Ok(Event::Ready) => {
                    assert!(subscription, "Refusing API-key inference");
                    agent.actions.send(Action::SendReferences("Read these two reference attachments. Reply with the dominant image color and the exact reference key from the text document. One line, no tools or artwork edits needed.".into(),references.clone())).unwrap();
                }
                Ok(Event::Message { id, text, delta }) => {
                    let reply = replies.entry(id).or_default();
                    if delta {
                        reply.push_str(&text);
                    } else {
                        *reply = text;
                    }
                }
                Ok(Event::Finished) => {
                    let reply = replies.values().cloned().collect::<Vec<_>>().join(" ");
                    assert!(
                        reply.to_lowercase().contains("red") && reply.contains("SLIME42"),
                        "{reply}"
                    );
                    break;
                }
                Ok(Event::Error(error)) => panic!("{error}"),
                Ok(Event::Approval { .. }) => {
                    panic!("Reference reading requested unexpected approval")
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => panic!("{error}"),
            }
        }
    }

    #[test]
    fn conversation_reuses_thread_and_interrupts_after_delayed_turn_ack() {
        let mut protocol = Protocol {
            thread: Some("thread-1".into()),
            busy: true,
            ..Default::default()
        };
        assert!(protocol.interrupt().is_none());
        let turn = json!({"method":"turn/started","params":{"threadId":"thread-1","turn":{"id":"turn-1"}}});
        let outgoing = protocol.notification(&turn);
        assert_eq!(outgoing.unwrap()["method"], "turn/interrupt");
        protocol.notification(&json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"interrupted"}}}));
        assert!(!protocol.busy);
        assert_eq!(protocol.thread.as_deref(), Some("thread-1"));
    }

    #[test]
    fn unrelated_thread_does_not_complete_current_turn() {
        let mut protocol = Protocol {
            thread: Some("ours".into()),
            busy: true,
            ..Default::default()
        };
        protocol.notification(&json!({"method":"turn/completed","params":{"threadId":"other","turn":{"status":"completed"}}}));
        assert!(protocol.busy);
    }
}
