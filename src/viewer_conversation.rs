use crate::agent::attachments::Attachment;
use crate::agent::{Action, Agent, Event, Workspace};
use eframe::egui;
use serde_json::Value;
#[path = "viewer_chat_ui.rs"]
mod chat_ui;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
};

struct Message {
    id: String,
    author: &'static str,
    sent_at: chrono::DateTime<chrono::Local>,
    attachments: Vec<Attachment>,
    text: String,
}

struct Thought {
    id: String,
    position: usize,
    text: String,
    started: std::time::Instant,
    elapsed: Option<std::time::Duration>,
}

#[derive(Default)]
pub(super) struct Conversation {
    pub visible: bool,
    selection: Option<crate::agent::models::Selection>,
    catalog: Option<Vec<crate::agent::models::Model>>,
    agent: Option<Agent>,
    workspace: Workspace,
    document_events: Option<Receiver<Event>>,
    ready: bool,
    pub busy: bool,
    failed: bool,
    status: String,
    document_error: Option<String>,
    account: String,
    messages: Vec<Message>,
    checkpoints: std::collections::BTreeMap<String, u64>,
    thoughts: Vec<Thought>,
    draft: String,
    attachments: Vec<Attachment>,
    pending_attachments: Vec<Attachment>,
    attachment_picker: Option<Receiver<anyhow::Result<Vec<Attachment>>>>,
    pending_prompt: Option<String>,
    approvals: Vec<(Value, String)>,
    recovery: Option<PathBuf>,
    saved: Option<PathBuf>,
    revision: Option<u64>,
    saved_revision: Option<u64>,
    save_picker: Option<Receiver<anyhow::Result<Option<PathBuf>>>>,
    can_revert: bool,
    scroll_to_top: bool,
    viewer_events: Vec<crate::mcp::desktop::DesktopEvent>,
    artist_activity: Option<super::Activity>,
}

impl Conversation {
    pub fn model_settings(
        &mut self,
    ) -> (
        crate::agent::models::Selection,
        Vec<crate::agent::models::Model>,
    ) {
        let selection = self
            .selection
            .get_or_insert_with(crate::agent::models::Selection::load)
            .clone();
        let catalog = self
            .catalog
            .get_or_insert_with(crate::agent::models::cached_catalog)
            .clone();
        (selection, catalog)
    }
    pub fn set_model_settings(&mut self, selection: crate::agent::models::Selection) {
        if self.busy {
            return;
        }
        if let Err(error) = selection.save() {
            self.document_error = Some(format!("Cannot save model preference: {error:#}"));
        }
        if self.agent.is_some() {
            self.action(Action::Configure(selection.clone()));
        }
        self.selection = Some(selection);
    }
    pub fn has_pending_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }
    pub fn take_viewer_events(&mut self) -> Vec<crate::mcp::desktop::DesktopEvent> {
        std::mem::take(&mut self.viewer_events)
    }
    fn artist_activity(&mut self, activity: super::Activity) {
        self.artist_activity = Some(activity.clone());
        self.viewer_events
            .push(crate::mcp::desktop::DesktopEvent::Activity(activity));
    }
    pub fn take_document_error(&mut self) -> Option<String> {
        self.document_error.take()
    }
    pub fn saved_path(&self) -> Option<&Path> {
        self.saved.as_deref()
    }
    pub fn watched_path(&self) -> Option<&Path> {
        if self.active() {
            None
        } else {
            self.saved_path()
        }
    }
    pub fn source_reloaded(&mut self) {
        // A normal, non-AI file watcher has loaded newer on-disk contents. Do not
        // let a cached save session later replace them with its old snapshot.
        if !self.active() {
            self.workspace = Workspace::default();
            self.saved = None;
            self.recovery = None;
            self.revision = None;
            self.saved_revision = None;
            self.can_revert = false;
        }
    }
    pub fn can_save(&self) -> bool {
        !self.busy && self.document_events.is_none() && self.save_picker.is_none()
    }

    pub fn save_project(&mut self, ctx: &egui::Context, source: Option<&Path>, save_as: bool) {
        if !self.can_save() {
            return;
        }
        if save_as
            || (self.saved.is_none()
                && (self.recovery.is_some()
                    || source.is_none_or(|p| {
                        !p.file_name().is_some_and(|name| {
                            let name = name.to_string_lossy().to_ascii_lowercase();
                            name.ends_with(".pixloom") || name.ends_with(".pixloom.json")
                        })
                    })))
        {
            let (tx, rx) = mpsc::channel();
            self.status = "Choosing save destination…".into();
            self.save_picker = Some(rx);
            let wake = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(super::dialog::save_file());
                wake.request_repaint();
            });
        } else {
            self.status = "Saving project…".into();
            self.busy = true;
            self.document_events = Some(self.workspace.document_action_from(
                Action::Save,
                source.map(Path::to_owned),
                ctx.clone(),
            ));
        }
    }

    pub fn edit_canvas(
        &mut self,
        ctx: &egui::Context,
        source: Option<&Path>,
        width: u32,
        height: u32,
        create: bool,
    ) {
        if !self.can_save() {
            return;
        }
        if create {
            self.reset();
        }
        self.status = if create {
            "Creating artwork…"
        } else {
            "Resizing canvas…"
        }
        .into();
        self.busy = true;
        self.document_events = Some(self.workspace.document_action_from(
            if create {
                Action::CreateArtwork(width, height)
            } else {
                Action::ResizeCanvas(width, height)
            },
            if create {
                None
            } else {
                source.map(Path::to_owned)
            },
            ctx.clone(),
        ));
    }

    fn pick_attachments(&mut self, ctx: &egui::Context) {
        if self.attachment_picker.is_some() {
            return;
        }
        let available = 8_usize.saturating_sub(self.attachments.len());
        let (tx, rx) = mpsc::channel();
        self.attachment_picker = Some(rx);
        let wake = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let paths = super::dialog::pick_references()?;
                anyhow::ensure!(
                    paths.len() <= available,
                    "Attach at most 8 reference files per message"
                );
                paths
                    .iter()
                    .map(|p| Attachment::load(p))
                    .collect::<anyhow::Result<Vec<_>>>()
            })();
            let _ = tx.send(result);
            wake.request_repaint();
        });
    }
    fn send_action(text: String, references: Vec<Attachment>) -> Action {
        if references.is_empty() {
            Action::Send(text)
        } else {
            Action::SendReferences(text, references)
        }
    }
    fn rewind_message(&mut self, token: u64) {
        if self.busy || !self.ready {
            return;
        }
        self.busy = true;
        self.status = "Reverting conversation…".into();
        self.action(Action::Rewind(token));
    }
    fn submit_draft(&mut self, ctx: &egui::Context, source: Option<&Path>) {
        let text = std::mem::take(&mut self.draft);
        if text.trim().is_empty() && self.attachments.is_empty() {
            return;
        }
        let attachments = std::mem::take(&mut self.attachments);
        self.messages.push(Message {
            id: format!("user-{}", self.messages.len()),
            author: "You",
            attachments: attachments.clone(),
            sent_at: chrono::Local::now(),
            text: text.clone(),
        });
        self.busy = true;
        if self.ready && !self.failed {
            self.action(Self::send_action(text, attachments));
        } else {
            self.pending_prompt = Some(text);
            self.pending_attachments = attachments;
            if self.agent.is_none() || self.failed {
                self.agent = Some(Agent::start_in(
                    source.map(Path::to_owned),
                    ctx.clone(),
                    self.workspace.clone(),
                    self.selection
                        .get_or_insert_with(crate::agent::models::Selection::load)
                        .clone(),
                ));
            }
            self.failed = false;
            self.ready = false;
            self.status = "Connecting to Codex…".into();
        }
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn active(&self) -> bool {
        self.agent.is_some() || self.document_events.is_some()
    }

    pub fn reset(&mut self) {
        *self = Self {
            visible: self.visible,
            selection: self.selection.take(),
            catalog: self.catalog.take(),
            ..Self::default()
        };
    }

    fn action(&mut self, action: Action) {
        if self
            .agent
            .as_ref()
            .is_none_or(|a| a.actions.send(action).is_err())
        {
            self.failed = true;
            self.busy = false;
            self.status = "AI disconnected. Reconnect to continue.".into();
        }
    }

    pub fn poll_events(
        &mut self,
        ctx: &egui::Context,
        source: Option<&Path>,
    ) -> Option<(crate::model::Sprite, PathBuf)> {
        if let Some(result) = self
            .attachment_picker
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            self.attachment_picker = None;
            match result {
                Ok(files) => self.attachments.extend(files),
                Err(error) => {
                    self.document_error = Some(format!("Cannot attach reference: {error:#}"))
                }
            }
        }
        let mut canvas = None;
        // Always drain events, including while the panel is collapsed.
        let mut events: Vec<_> = self
            .agent
            .as_ref()
            .map(|a| a.events.try_iter().collect())
            .unwrap_or_default();
        if let Some(agent) = &self.agent
            && let Some(canvas) = agent.canvas.lock().unwrap().take()
        {
            events.push(canvas);
        }
        if let Some(receiver) = &self.document_events {
            events.extend(receiver.try_iter());
        }
        for event in events {
            match event {
                Event::Viewer(event) => match event {
                    crate::mcp::desktop::DesktopEvent::Activity(activity) => {
                        self.artist_activity(activity)
                    }
                    other => self.viewer_events.push(other),
                },
                Event::ApplicationStatus(status) => {
                    if status.starts_with("Document action failed:") {
                        self.document_error = Some(status.clone());
                    }
                    self.status = status;
                }
                Event::Status(status) => {
                    if status.starts_with("Document action failed:") {
                        self.document_error = Some(status.clone());
                        self.status = status;
                    } else if self.ready {
                        let mut activity =
                            self.artist_activity.clone().unwrap_or(super::Activity {
                                label: String::new(),
                                busy: self.busy,
                                frame_id: None,
                                layer_id: None,
                            });
                        activity.label = status;
                        activity.busy = self.busy;
                        self.artist_activity(activity);
                    } else {
                        self.status = status;
                    }
                }
                Event::Models(catalog) => {
                    if !catalog.is_empty() {
                        self.catalog = Some(catalog);
                    }
                }
                Event::Account(account) => self.account = account,
                Event::Ready => {
                    self.ready = true;
                    self.busy = false;
                    self.status = "Ready".into();
                    if let Some(text) = self.pending_prompt.take() {
                        self.busy = true;
                        let references = std::mem::take(&mut self.pending_attachments);
                        self.action(Self::send_action(text, references));
                    }
                }
                Event::TurnCheckpoint(token) => {
                    if let Some(message) = self.messages.iter().rev().find(|m| m.author == "You") {
                        self.checkpoints.insert(message.id.clone(), token);
                    }
                }
                Event::Rewound(token) => {
                    if let Some(index) = self
                        .messages
                        .iter()
                        .position(|m| self.checkpoints.get(&m.id) == Some(&token))
                    {
                        self.draft = self.messages[index].text.clone();
                        self.attachments = self.messages[index].attachments.clone();
                        self.messages.truncate(index);
                        self.thoughts.retain(|t| t.position < index);
                        self.checkpoints.retain(|_, value| *value < token);
                        self.status = "Conversation reverted".into();
                        ctx.memory_mut(|m| m.request_focus(egui::Id::new("art-prompt")));
                    }
                }
                Event::Thought {
                    id,
                    text,
                    done,
                    delta,
                } => {
                    let index = self
                        .thoughts
                        .iter()
                        .position(|t| t.id == id)
                        .unwrap_or_else(|| {
                            self.thoughts.push(Thought {
                                id: id.clone(),
                                position: self.messages.len(),
                                text: String::new(),
                                started: std::time::Instant::now(),
                                elapsed: None,
                            });
                            self.thoughts.len() - 1
                        });
                    let thought = &mut self.thoughts[index];
                    if delta {
                        thought.text.push_str(&text);
                    } else if !text.is_empty() {
                        thought.text = text;
                    }
                    if done {
                        thought.elapsed = Some(thought.started.elapsed());
                    }
                }
                Event::Working => {
                    self.busy = true;
                    self.status = "Ready".into();
                    self.artist_activity(super::Activity {
                        label: "Artist is working…".into(),
                        busy: true,
                        frame_id: None,
                        layer_id: None,
                    });
                }
                Event::Finished => {
                    for thought in &mut self.thoughts {
                        thought
                            .elapsed
                            .get_or_insert_with(|| thought.started.elapsed());
                    }
                    self.busy = false;
                    let mut activity = self.artist_activity.clone().unwrap_or(super::Activity {
                        label: "Artist finished".into(),
                        busy: false,
                        frame_id: None,
                        layer_id: None,
                    });
                    activity.busy = false;
                    self.artist_activity(activity);
                    self.approvals.clear();
                }
                Event::BaselineAvailable(available) => self.can_revert = available,
                Event::DocumentDone => {
                    if self.status == "Creating artwork…" {
                        self.status = "Artwork created".into();
                    }
                    if self.status == "Resizing canvas…" {
                        self.status = "Canvas resized".into();
                    }
                    self.busy = false;
                    self.document_events = None;
                }
                Event::Error(error) => {
                    self.checkpoints.clear();
                    for thought in &mut self.thoughts {
                        thought
                            .elapsed
                            .get_or_insert_with(|| thought.started.elapsed());
                    }
                    if let Some(mut activity) = self.artist_activity.clone() {
                        activity.busy = false;
                        activity.label = "Artist stopped".into();
                        self.artist_activity(activity);
                    }
                    self.status = error;
                    self.failed = true;
                    self.busy = false;
                    self.ready = false;
                    self.approvals.clear();
                }
                Event::Message { id, text, delta } => {
                    if let Some(message) = self.messages.iter_mut().find(|m| m.id == id) {
                        if delta {
                            message.text.push_str(&text);
                        } else {
                            message.text = text;
                        }
                    } else {
                        self.messages.push(Message {
                            id,
                            author: "Artist",
                            attachments: Vec::new(),
                            sent_at: chrono::Local::now(),
                            text,
                        });
                    }
                }
                Event::Approval { id, description } => {
                    self.approvals.push((id, description));
                    self.artist_activity(super::Activity {
                        label: "Awaiting approval".into(),
                        busy: true,
                        frame_id: None,
                        layer_id: None,
                    });
                    self.scroll_to_top = true;
                }
                Event::Resolved(id) => self.approvals.retain(|(other, _)| *other != id),
                Event::Saved { path, revision } => {
                    self.saved = Some(path);
                    self.saved_revision = Some(revision);
                    self.status = "Project saved".into();
                }
                Event::Canvas {
                    revision,
                    sprite,
                    recovery,
                } => {
                    self.revision = Some(revision);
                    self.recovery = Some(recovery.clone());
                    if !self.failed && self.approvals.is_empty() && !self.busy {
                        self.status = format!("Recovery saved · revision {revision}");
                    }
                    canvas = Some((*sprite, recovery));
                }
            }
        }
        if let Some(result) = self.save_picker.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.save_picker = None;
            match result {
                Ok(Some(path)) => {
                    self.status = "Saving project…".into();
                    self.busy = true;
                    self.document_events = Some(self.workspace.document_action_from(
                        Action::SaveAs(path),
                        source.map(Path::to_owned),
                        ctx.clone(),
                    ));
                }
                Ok(None) => self.status = "Save cancelled".into(),
                Err(error) => self.status = format!("Save failed: {error:#}"),
            }
        }
        canvas
    }
    pub fn panel(
        &mut self,
        ctx: &egui::Context,
        source: Option<&Path>,
    ) -> Option<(crate::model::Sprite, PathBuf)> {
        let canvas = self.poll_events(ctx, source);
        if self.visible {
            self.show_panel(ctx, source);
        }
        canvas
    }
}

#[cfg(test)]
#[path = "viewer_conversation_tests.rs"]
mod tests;
