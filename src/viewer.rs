#[path = "viewer_chrome.rs"]
mod chrome;
#[path = "viewer_conversation.rs"]
mod conversation;
#[path = "viewer_dialog.rs"]
mod dialog;
#[path = "viewer_icons.rs"]
mod icons;
#[path = "viewer_layers.rs"]
mod layers;
#[path = "viewer_review.rs"]
pub(crate) mod review_ui;
#[path = "viewer_settings.rs"]
mod settings;
#[path = "viewer_sheet.rs"]
mod sheet;
#[path = "viewer_theme.rs"]
mod theme;
use crate::{
    model::Sprite,
    preview::{self, Playback},
};
use anyhow::{Context, Result, ensure};
use eframe::egui;
use eframe::emath::GuiRounding;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant, SystemTime},
};

fn viewer_executable(current: &Path) -> Result<PathBuf> {
    let name = if cfg!(windows) {
        "pixloom.exe"
    } else {
        "pixloom"
    };
    let path = current
        .parent()
        .context("executable has no parent directory")?
        .join(name);
    ensure!(
        path.is_file(),
        "Pixloom app missing: {}; install pixloom and pixloom-cli together",
        path.display()
    );
    Ok(path)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Activity {
    pub label: String,
    pub frame_id: Option<String>,
    pub layer_id: Option<String>,
    pub busy: bool,
}
pub struct ViewerProcess {
    child: Child,
    replies: Receiver<Value>,
}
impl ViewerProcess {
    pub fn open(path: &Path) -> Result<(Self, Value)> {
        let mut command = Command::new(viewer_executable(&std::env::current_exe()?)?);
        command
            .arg("--managed")
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .context("start preview app; check executable availability")?;
        let stdout = child
            .stdout
            .take()
            .context("viewer response pipe missing")?;
        let (tx, replies) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                let Ok(reply) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(reply).is_err() {
                    break;
                }
            }
        });
        let mut process = Self { child, replies };
        let reply = process.reply()?;
        Ok((process, reply))
    }
    fn reply(&mut self) -> Result<Value> {
        let reply=self.replies.recv_timeout(Duration::from_secs(3)).map_err(|error| {
            let message=match error {
                mpsc::RecvTimeoutError::Timeout=>"preview app did not confirm opening within 3 seconds; ensure an interactive desktop and working OpenGL driver, then retry open_preview",
                mpsc::RecvTimeoutError::Disconnected=>"preview process disconnected; ensure an interactive desktop and working OpenGL driver, then retry open_preview (see stderr for backend details)",
            };
            anyhow::Error::new(error).context(message)
        })?;
        ensure!(
            reply["opened"] == true,
            "preview unavailable: {}",
            reply["error"]
                .as_str()
                .unwrap_or("viewer disconnected; retry open_preview from an interactive desktop")
        );
        Ok(reply)
    }
    pub fn alive(&mut self) -> bool {
        self.child
            .try_wait()
            .ok()
            .is_some_and(|status| status.is_none())
    }
    pub fn activate(&mut self) -> Result<Value> {
        ensure!(
            self.alive(),
            "preview window has closed; call open_preview again"
        );
        writeln!(
            self.child
                .stdin
                .as_mut()
                .context("preview command pipe closed")?,
            "activate"
        )
        .context("viewer disconnected; retry open_preview")?;
        self.reply()
    }
    pub(crate) fn activity(&mut self, activity: &Activity) -> Result<()> {
        let input = self.child.stdin.as_mut().context("viewer input closed")?;
        serde_json::to_writer(&mut *input, activity)?;
        writeln!(input)?;
        input.flush()?;
        Ok(())
    }
    pub(crate) fn inspection(&mut self, view: &crate::inspection::View) -> Result<()> {
        let input = self.child.stdin.as_mut().context("viewer input closed")?;
        serde_json::to_writer(&mut *input, &json!({"inspection":view}))?;
        writeln!(input)?;
        input.flush()?;
        Ok(())
    }
    pub(crate) fn review(&mut self, review: &review_ui::ViewerReview) -> Result<()> {
        let input = self.child.stdin.as_mut().context("viewer input closed")?;
        serde_json::to_writer(&mut *input, &json!({"review":review}))?;
        writeln!(input)?;
        input.flush()?;
        Ok(())
    }
    pub fn close(&mut self) {
        drop(self.child.stdin.take());
        for _ in 0..100 {
            if !self.alive() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for ViewerProcess {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Menu,
    Reset,
    Attach,
    NewChat,
    Solo,
    Previous,
    Next,
    Play,
    Pause,
    Stop,
    Slower,
    Faster,
}
fn panel_header(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(
            theme::metric("panel_padding") as i8,
            0,
        ))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), theme::metric("header_height")),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_height(theme::metric("header_height"));
                    contents(ui);
                },
            );
        });
}
fn configure_text(ctx: &egui::Context) {
    theme::apply(ctx);
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    icon_button_shape(ui, icon, label, theme::radius("control_radius"), false)
}
fn icon_button_shape(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    rounding: egui::CornerRadius,
    border: bool,
) -> egui::Response {
    let _ = border;
    let size = theme::metric("button_size");
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("icon-button-rect", label)), rect));
    if response.hovered() || response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, rounding, theme::color("hover"));
    }
    let name = match icon {
        Icon::Reset => "reset",
        Icon::Solo => "solo",
        Icon::Menu => "menu",
        Icon::Previous => "previous",
        Icon::Next => "next",
        Icon::Play => "play",
        Icon::Pause => "pause",
        Icon::Stop => "stop",
        Icon::Slower => "minus",
        Icon::Faster => "plus",
        Icon::Attach => "attach",
        Icon::NewChat => "new",
    };
    icons::paint(
        ui,
        name,
        egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(theme::metric("icon_size"))),
        theme::color(if !ui.is_enabled() {
            "disabled"
        } else if response.hovered() {
            "icon_hover"
        } else {
            "icon"
        }),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}

enum Message {
    Activity(Activity),
    Inspection(crate::inspection::View),
    Review(review_ui::ViewerReview),
    Activate,
    Close,
}
#[derive(Clone, Copy, Default, PartialEq)]
enum Background {
    #[default]
    Checkerboard,
    Transparent,
    Black,
    White,
}
struct App {
    conversation: conversation::Conversation,
    managed: bool,
    source: Option<PathBuf>,
    opened_source: Option<PathBuf>,
    source_canvas: Option<(u32, u32)>,
    canvas_preview: Option<(u32, u32)>,
    speed: f64,
    zoom: f32,
    canvas_scale: Option<f32>,
    background: Background,
    layer_view: layers::LayerView,
    show_layers: bool,
    show_animation: bool,
    follow_ai: bool,
    review: review_ui::ReviewState,
    activity: Option<Activity>,
    activity_from_embedded: bool,
    follow_pending: bool,
    inspection: Option<crate::inspection::View>,
    inspection_pending: bool,
    inspection_mode: crate::inspection::Mode,
    inspection_reference: Option<String>,
    inspection_layer: Option<String>,
    pan: egui::Vec2,
    file_picker: Option<Receiver<Result<Option<PathBuf>>>>,
    alternate: Option<Sprite>,
    sheet_view: bool,
    sprite: Option<Sprite>,
    playback: Playback,
    texture: Option<egui::TextureHandle>,
    thumbnails: Vec<Option<egui::TextureHandle>>,
    thumbnail_selection: Option<usize>,
    rendered: Option<(usize, usize)>,
    messages: Receiver<Message>,
    checked: Instant,
    modified: Option<SystemTime>,
    clock: Instant,
    pending_ack: Option<u8>,
    error: Option<String>,
    last_error: Option<String>,
    error_since: Option<Instant>,
}
fn reply(value: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

impl App {
    fn conversation_panel(&mut self, ctx: &egui::Context) {
        self.update_conversation(ctx, true);
    }
    fn update_conversation(&mut self, ctx: &egui::Context, show: bool) {
        let canvas = if show {
            self.conversation.panel(ctx, self.source.as_deref())
        } else {
            self.conversation.poll_events(ctx, self.source.as_deref())
        };
        if let Some((sprite, recovery)) = canvas {
            self.source_canvas = Some((sprite.width, sprite.height));
            let before = self
                .sprite
                .as_ref()
                .map(|sprite| sprite.frames.clone())
                .unwrap_or_default();
            let mut playback = std::mem::replace(&mut self.playback, Playback::new(0));
            self.set_views(sprite, None);
            playback.reconcile_frames(&before, &self.project().frames);
            self.playback = playback;
            let source = self
                .conversation
                .watched_path()
                .unwrap_or(&recovery)
                .to_owned();
            self.modified = std::fs::metadata(&source).and_then(|m| m.modified()).ok();
            self.source = Some(source);
            self.thumbnails.clear();
            self.thumbnail_selection = None;
            self.rendered = None;
            self.layer_view.invalidate();
        }
        for event in self.conversation.take_viewer_events() {
            match event {
                crate::mcp::desktop::DesktopEvent::Activity(activity) => {
                    self.receive_activity(activity);
                    self.activity_from_embedded = true;
                }
                crate::mcp::desktop::DesktopEvent::Inspection(view) => {
                    self.review.normal(self.follow_ai);
                    self.inspection = Some(view);
                    self.inspection_pending = true;
                }
                crate::mcp::desktop::DesktopEvent::Review(review) => self.review.receive(review),
            }
        }
        if let Some(error) = self.conversation.take_document_error() {
            self.error = Some(error);
        }
    }
    fn following(&self) -> bool {
        self.follow_ai
            && self
                .activity
                .as_ref()
                .is_some_and(|a| !self.activity_from_embedded || a.busy)
    }
    fn receive_activity(&mut self, activity: Activity) {
        self.activity_from_embedded = false;
        self.follow_pending = activity.frame_id.is_some() || activity.layer_id.is_some();
        self.activity = Some(activity);
    }
    fn follow_activity(&mut self) {
        if !self.follow_ai || !self.follow_pending {
            return;
        }
        let Some(activity) = self.activity.clone() else {
            return;
        };
        if let Some(id) = &activity.frame_id
            && self.sprite.as_ref().is_some_and(|sprite| {
                !sprite
                    .frames
                    .iter()
                    .any(|frame| crate::edit::frame_id(frame) == Some(id.as_str()))
            })
            && self.alternate.as_ref().is_some_and(|sprite| {
                sprite
                    .frames
                    .iter()
                    .any(|frame| crate::edit::frame_id(frame) == Some(id.as_str()))
            })
        {
            self.toggle_view();
        }
        let Some(sprite) = self.sprite.as_ref() else {
            return;
        };
        // A source reload can lag its event; keep the request pending until the target exists.
        let frame = activity.frame_id.as_ref().and_then(|id| {
            sprite
                .frames
                .iter()
                .position(|frame| crate::edit::frame_id(frame) == Some(id.as_str()))
        });
        if activity.frame_id.is_some() && frame.is_none() {
            return;
        }
        if let Some(frame) = frame {
            self.playback.select(frame, sprite.frames.len());
        }
        if let Some(id) = &activity.layer_id {
            self.layer_view.select(sprite, id);
        }
        self.follow_pending = false;
    }
    fn follow_inspection(&mut self) {
        if !self.follow_ai || !self.inspection_pending {
            return;
        }
        let Some(view) = self.inspection.clone() else {
            return;
        };
        if self.sprite.as_ref().is_some_and(|s| {
            !s.frames
                .iter()
                .any(|f| crate::edit::frame_id(f) == Some(view.frame_id.as_str()))
        }) && self.alternate.as_ref().is_some_and(|s| {
            s.frames
                .iter()
                .any(|f| crate::edit::frame_id(f) == Some(view.frame_id.as_str()))
        }) {
            self.toggle_view();
        }
        let Some(sprite) = &self.sprite else {
            return;
        };
        let Some(frame) = sprite
            .frames
            .iter()
            .position(|f| crate::edit::frame_id(f) == Some(view.frame_id.as_str()))
        else {
            return;
        };
        if view.reference_frame_id.as_ref().is_some_and(|id| {
            !sprite
                .frames
                .iter()
                .any(|f| crate::edit::frame_id(f) == Some(id.as_str()))
        }) {
            return;
        }
        if view
            .layer_id
            .as_ref()
            .is_some_and(|id| !sprite.layers.iter().any(|l| &l.id == id))
        {
            return;
        }
        self.playback.select(frame, sprite.frames.len());
        self.inspection_mode = view.mode;
        self.inspection_reference = view.reference_frame_id;
        self.layer_view = Default::default();
        if let Some(id) = &view.layer_id {
            self.layer_view.select(sprite, id);
        }
        self.inspection_layer = view.layer_id;
        self.inspection_pending = false;
        self.rendered = None;
    }
    fn review_source_identity(&self) -> Option<u64> {
        let path = self.source.as_ref()?;
        let doc = crate::input::read(path)
            .or_else(|_| crate::review::document_for(self.project()))
            .ok()?;
        crate::review::identity(&doc).ok()
    }
    fn follow_review(&mut self) {
        if self.review.check_identity_due()
            && self.review_source_identity() != self.review.source_identity
        {
            self.modified = None;
        }
        if let Some(view) = self.review.apply(self.follow_ai) {
            self.inspection = Some(view);
            self.inspection_pending = true;
            self.follow_inspection();
            self.rendered = None;
        }
        if std::mem::take(&mut self.review.changed) {
            self.rendered = None;
        }
        if let Some(action) = self.review.local.take() {
            let result = (|| -> Result<crate::review::ReviewResult> {
                let doc = self
                    .source
                    .as_ref()
                    .and_then(|p| crate::input::read(p).ok())
                    .map(Ok)
                    .unwrap_or_else(|| crate::review::document_for(self.project()))?;
                let ids = crate::review::frame_ids(&doc);
                let id = ids
                    .get(self.playback.frame)
                    .context("current frame unavailable")?;
                let mut result = match action {
                    review_ui::LocalReview::Analyze => crate::review::measure::sprite(
                        &doc,
                        &serde_json::from_value(
                            json!({"frame_id":id,"layer_id":self.inspection_layer}),
                        )?,
                    )?,
                    review_ui::LocalReview::Sheet => crate::review::render::sheet(
                        &doc,
                        &serde_json::from_value(
                            json!({"frame_id":id,"layer_id":self.inspection_layer}),
                        )?,
                    )?,
                    review_ui::LocalReview::Animation => crate::review::animation::audit(
                        &doc,
                        &serde_json::from_value(json!({"layer_id":self.inspection_layer}))?,
                    )?,
                };
                crate::review::stamp(&mut result, "local", 0, crate::review::identity(&doc)?);
                result.report["inspected_revision"] = Value::Null;
                Ok(result)
            })();
            match result.and_then(|r| review_ui::ViewerReview::new(&r)) {
                Ok(r) => {
                    self.review.local_result(r);
                    self.inspection_mode = crate::inspection::Mode::Composite;
                    self.inspection_reference = None;
                    self.rendered = None;
                }
                Err(e) => self.report_error(format!("Review failed: {e:#}")),
            }
        }
    }
    fn canvas_dimensions(&self) -> (u32, u32) {
        self.canvas_preview
            .unwrap_or_else(|| (self.project().width, self.project().height))
    }
    fn canvas_pixels(&self) -> Result<Vec<u8>> {
        let mut source = self.project();
        if self.canvas_preview.is_some()
            && self.source_canvas != Some((source.width, source.height))
            && let Some(original) = self
                .alternate
                .as_ref()
                .filter(|s| self.source_canvas == Some((s.width, s.height)))
        {
            source = original;
        }
        let rgba = if std::ptr::eq(source, self.project()) {
            self.inspection_render()?
        } else {
            crate::raster::render(source, self.playback.frame.min(source.frames.len() - 1))?
        };
        let (width, height) = self.canvas_dimensions();
        if (width, height) == (source.width, source.height) {
            return Ok(rgba);
        }
        let mut resized = vec![0; crate::schema::pixel_bytes(width, height)?];
        let count = width.min(source.width) as usize * 4;
        for y in 0..height.min(source.height) as usize {
            resized[y * width as usize * 4..y * width as usize * 4 + count].copy_from_slice(
                &rgba[y * source.width as usize * 4..y * source.width as usize * 4 + count],
            );
        }
        Ok(resized)
    }

    fn inspection_render(&self) -> Result<Vec<u8>> {
        let sprite = self.project();
        let reference = self.inspection_reference.as_ref().and_then(|id| {
            sprite.frames.iter().enumerate().position(|(i, f)| {
                crate::edit::frame_id(f)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("viewer-frame-{i}"))
                    == *id
            })
        });
        crate::inspection::render_with(
            sprite.frames.len(),
            self.playback.frame,
            reference,
            self.inspection_mode,
            |frame| {
                if let Some(id) = &self.inspection_layer {
                    let mut isolated = sprite.clone();
                    isolated.layers.retain(|l| &l.id == id);
                    if let Some(layer) = isolated.layers.first_mut() {
                        layer.visible = true;
                    }
                    crate::raster::render(&isolated, frame)
                } else {
                    self.layer_view.render(sprite, frame)
                }
            },
        )
        .map(|(rgba, _)| {
            let rgba = crate::review::render::transform(&rgba, self.review.representation);
            self.review.annotate(
                rgba,
                sprite.width,
                sprite.height,
                Some(crate::review::frame_keys(&sprite.frames)[self.playback.frame].as_str()),
                self.inspection_layer.as_deref(),
            )
        })
    }
    fn activity_bar(&mut self, ctx: &egui::Context) {
        if self.activity.is_none() {
            return;
        }
        egui::TopBottomPanel::top("ai-activity")
            .frame(
                egui::Frame::NONE
                    .fill(theme::color("panel"))
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 8,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| {
                let (row, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::Sense::hover(),
                );
                let center = egui::Rect::from_min_max(
                    row.min + egui::vec2(48.0, 0.0),
                    row.max - egui::vec2(48.0, 0.0),
                );
                if self.activity.as_ref().is_some_and(|event| event.busy) {
                    let spinner = egui::Rect::from_min_size(row.min, egui::vec2(22.0, 22.0));
                    ui.put(spinner, egui::Spinner::new());
                }
                ui.put(
                    center,
                    egui::Label::new(&self.activity.as_ref().unwrap().label)
                        .halign(egui::Align::Center)
                        .truncate(),
                );
                let rect = egui::Rect::from_min_size(
                    egui::pos2(row.right() - 40.0, row.top()),
                    egui::vec2(40.0, 22.0),
                );
                let response = ui.interact(
                    rect,
                    egui::Id::new("follow-ai-switch"),
                    egui::Sense::click(),
                );
                let response = response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(if self.follow_ai {
                        "Following AI · click to inspect manually"
                    } else {
                        "Not following AI · click to follow"
                    });
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Checkbox,
                        true,
                        self.follow_ai,
                        "Follow AI",
                    )
                });
                if response.clicked() {
                    self.follow_ai = !self.follow_ai;
                    self.follow_pending = self.follow_ai;
                    self.inspection_pending = self.follow_ai && self.inspection.is_some();
                    if self.follow_ai {
                        self.review.resume();
                    }
                }
                let fill = if self.follow_ai {
                    theme::color("success")
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };
                ui.painter().rect_filled(rect, 11.0, fill);
                let x = if self.follow_ai {
                    rect.right() - 11.0
                } else {
                    rect.left() + 11.0
                };
                ui.painter().circle_filled(
                    egui::pos2(x, rect.center().y),
                    8.0,
                    egui::Color32::WHITE,
                );
            });
    }
    fn project(&self) -> &Sprite {
        self.sprite
            .as_ref()
            .expect("project controls require a loaded sprite")
    }
    fn empty(messages: Receiver<Message>) -> Self {
        let now = Instant::now();
        Self {
            conversation: Default::default(),
            managed: false,
            source: None,
            opened_source: None,
            source_canvas: None,
            canvas_preview: None,
            sprite: None,
            speed: 1.0,
            zoom: 1.0,
            canvas_scale: None,
            background: Default::default(),
            layer_view: Default::default(),
            show_layers: true,
            show_animation: false,
            follow_ai: true,
            review: Default::default(),
            activity: None,
            activity_from_embedded: false,
            follow_pending: false,
            inspection: None,
            inspection_pending: false,
            inspection_mode: crate::inspection::Mode::Composite,
            inspection_reference: None,
            inspection_layer: None,
            pan: eframe::egui::Vec2::ZERO,
            file_picker: None,
            alternate: None,
            sheet_view: false,
            playback: Playback::new(0),
            texture: None,
            thumbnails: Vec::new(),
            thumbnail_selection: None,
            rendered: None,
            messages,
            checked: now,
            modified: None,
            clock: now,
            pending_ack: None,
            error: None,
            last_error: None,
            error_since: None,
        }
    }

    fn open(&mut self, path: PathBuf) {
        if self.conversation.busy {
            self.report_error("Stop the AI turn before opening another document.".into());
            return;
        }
        match preview::load(&path) {
            Ok(sprite) => {
                self.conversation.reset();
                self.source_canvas = Some((sprite.width, sprite.height));
                self.playback = Playback::new(sprite.frames.len());
                self.set_views(sprite, None);
                self.modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                self.opened_source = Some(path.clone());
                self.source = Some(path);
                self.activity = None;
                self.follow_pending = false;
                self.review = Default::default();
                self.review.source_identity = self.review_source_identity();
                self.inspection = None;
                self.inspection_pending = false;
                self.inspection_mode = crate::inspection::Mode::Composite;
                self.inspection_reference = None;
                self.inspection_layer = None;
                self.layer_view = Default::default();
                self.zoom = 1.0;
                self.canvas_scale = None;
                self.pan = egui::Vec2::ZERO;
                self.thumbnails.clear();
                self.thumbnail_selection = None;
                self.rendered = None;
                self.clock = Instant::now();
                self.error = None;
            }
            Err(error) => self.report_error(format!("Open failed: {error:#}")),
        }
    }
    fn report_error(&mut self, message: String) {
        self.error_since = Some(Instant::now());
        self.last_error = Some(message.clone());
        self.error = Some(message);
    }
    fn can_toggle_view(&self) -> bool {
        self.alternate.is_some()
            || self
                .sprite
                .as_ref()
                .is_some_and(|sprite| sprite.frames.len() > 1)
    }
    fn set_views(&mut self, sprite: Sprite, preferred_sheet: Option<bool>) {
        self.canvas_preview = None;
        self.sheet_view = sprite.frames.len() == 1;
        self.alternate = if self.sheet_view {
            sheet::animation(&sprite).ok().flatten()
        } else {
            None
        };
        self.sprite = Some(sprite);
        self.layer_view.invalidate();
        if self.can_toggle_view() && preferred_sheet.is_some_and(|view| view != self.sheet_view) {
            self.toggle_view();
        }
    }
    fn toggle_view(&mut self) {
        if self.alternate.is_none()
            && self
                .sprite
                .as_ref()
                .is_some_and(|sprite| sprite.frames.len() > 1)
        {
            match sheet::sheet(self.project()) {
                Ok(sheet) => self.alternate = Some(sheet),
                Err(error) => {
                    self.report_error(format!("Sprite view unavailable: {error:#}"));
                    return;
                }
            }
        }
        if let Some(alternate) = self.alternate.take() {
            self.alternate = self.sprite.replace(alternate);
            self.sheet_view = !self.sheet_view;
            self.layer_view = Default::default();
            self.playback = Playback::new(self.project().frames.len());
            self.thumbnails.clear();
            self.thumbnail_selection = None;
            self.rendered = None;
            self.clock = Instant::now();
        }
    }
    fn start_file_picker(&mut self, ctx: &egui::Context) {
        if self.file_picker.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.file_picker = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(dialog::pick_file());
            ctx.request_repaint();
        });
    }
    fn activate(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.pending_ack = Some(2);
    }
    fn poll(&mut self) {
        let Some(source) = self.source.as_ref() else {
            return;
        };
        if self.checked.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.checked = Instant::now();
        match std::fs::metadata(source).and_then(|m| m.modified()) {
            Ok(stamp) if Some(stamp) != self.modified => match preview::load(source) {
                Ok(sprite) => {
                    self.source_canvas = Some((sprite.width, sprite.height));
                    self.opened_source = Some(source.clone());
                    self.conversation.source_reloaded();
                    let before = self.project().frames.clone();
                    let mut playback = std::mem::replace(&mut self.playback, Playback::new(0));
                    self.set_views(sprite, Some(self.sheet_view));
                    playback.reconcile_frames(&before, &self.project().frames);
                    self.playback = playback;
                    self.thumbnails.clear();
                    self.thumbnail_selection = None;
                    self.modified = Some(stamp);
                    self.review.source_identity = self.review_source_identity();
                    self.rendered = None;
                    self.error = None;
                }
                Err(e) => self.report_error(format!(
                    "Reload failed: {e:#}. Restore a supported project at this path."
                )),
            },
            Err(e) => {
                self.modified = None;
                self.report_error(format!(
                    "Source unavailable: {e}. Restore {} to resume live updates.",
                    source.display()
                ))
            }
            _ => {}
        }
    }
    fn main_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let response = icon_button(ui, Icon::Menu, "Menu");
        egui::Popup::menu(&response)
            .frame(
                egui::Frame::popup(ui.style())
                    .fill(theme::color("raised"))
                    .stroke(theme::stroke())
                    .corner_radius(theme::radius("corner_radius"))
                    .inner_margin(egui::Margin::symmetric(0, 4)),
            )
            .show(|ui| {
                ui.set_min_width(theme::metric("menu_width"));
                ui.spacing_mut().button_padding = egui::vec2(theme::metric("menu_padding"), 0.0);
                ui.spacing_mut().interact_size.y = theme::metric("menu_row_height");
                ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                ui.visuals_mut().widgets.hovered.bg_fill = theme::color("hover");
                ui.visuals_mut().widgets.hovered.weak_bg_fill = theme::color("hover");
                ui.visuals_mut().widgets.noninteractive.bg_stroke =
                    egui::Stroke::new(theme::metric("separator_width"), theme::color("border"));
                ui.add_enabled_ui(!self.following(), |ui| {
                    if ui
                        .add_enabled(
                            !self.managed && self.conversation.can_save(),
                            egui::Button::new("New artwork…"),
                        )
                        .clicked()
                    {
                        ctx.data_mut(|data| {
                            data.insert_temp(egui::Id::new("canvas-dialog"), (true, 32_u32, 32_u32))
                        });
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.file_picker.is_none(), egui::Button::new("Open file…"))
                        .clicked()
                    {
                        self.start_file_picker(ctx);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.file_picker.is_none(),
                            egui::Button::new("Open Project…"),
                        )
                        .clicked()
                    {
                        self.start_file_picker(ctx);
                        ui.close();
                    }
                    let can_save =
                        self.sprite.is_some() && !self.managed && self.conversation.can_save();
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save Project"))
                        .clicked()
                    {
                        self.conversation
                            .save_project(ctx, self.source.as_deref(), false);
                        ui.close();
                    }
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save As…"))
                        .clicked()
                    {
                        self.conversation
                            .save_project(ctx, self.source.as_deref(), true);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.managed && self.sprite.is_some() && self.conversation.can_save(),
                            egui::Button::new("Canvas size…"),
                        )
                        .clicked()
                    {
                        let (w, h) = self.source_canvas.unwrap_or((32, 32));
                        ctx.data_mut(|data| {
                            data.insert_temp(egui::Id::new("canvas-dialog"), (false, w, h))
                        });
                        ui.close();
                    }
                    let label = if self.sheet_view {
                        "Switch to animation view"
                    } else {
                        "Switch to sprite view"
                    };
                    if ui
                        .add_enabled(self.can_toggle_view(), egui::Button::new(label))
                        .clicked()
                    {
                        self.toggle_view();
                        ui.close();
                    }
                    if ui.selectable_label(self.show_layers, "Layers").clicked() {
                        self.select_panel(0);
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.show_animation, "Animation")
                        .clicked()
                    {
                        self.select_panel(1);
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.conversation.visible, "AI conversation")
                        .clicked()
                    {
                        self.select_panel(2);
                        ui.close();
                    }
                    ui.separator();
                    ui.menu_button("Background", |ui| {
                        for (background, label) in [
                            (Background::Checkerboard, "Pixel checkerboard"),
                            (Background::Transparent, "Transparent"),
                            (Background::Black, "Black"),
                            (Background::White, "White"),
                        ] {
                            if ui
                                .selectable_value(&mut self.background, background, label)
                                .changed()
                            {
                                self.rendered = None;
                                self.thumbnails.clear();
                                self.layer_view.invalidate();
                                ui.close();
                            }
                        }
                    });
                    if self.review.color_menu(ui) {
                        self.rendered = None;
                    }
                    ui.separator();
                    let mut onion = self.inspection_mode == crate::inspection::Mode::OnionSkin;
                    if ui
                        .checkbox(&mut onion, "Onion skin")
                        .on_hover_text(
                            "Red: previous frame. Cyan: next frame. Artwork stays unchanged.",
                        )
                        .changed()
                    {
                        self.inspection_mode = if onion {
                            crate::inspection::Mode::OnionSkin
                        } else {
                            crate::inspection::Mode::Composite
                        };
                        self.playback.playing = false;
                        self.rendered = None;
                    }
                    let mut comparison =
                        self.inspection_mode == crate::inspection::Mode::FrameComparison;
                    if ui
                .checkbox(&mut comparison, "Frame comparison")
                .on_hover_text(
                    "Pink: changed pixels. Compare to the previous frame or selected reference.",
                )
                .changed()
            {
                self.inspection_mode = if comparison {
                    crate::inspection::Mode::FrameComparison
                } else {
                    crate::inspection::Mode::Composite
                };
                self.playback.playing = false;
                self.rendered = None;
            }
                    if self.inspection_mode != crate::inspection::Mode::Composite {
                        let frames = self
                            .sprite
                            .as_ref()
                            .map(|s| {
                                s.frames
                                    .iter()
                                    .enumerate()
                                    .map(|(i, f)| {
                                        (
                                            crate::edit::frame_id(f)
                                                .map(str::to_owned)
                                                .unwrap_or_else(|| format!("viewer-frame-{i}")),
                                            i,
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        let previous = self.inspection_reference.clone();
                        egui::ComboBox::from_label("Reference")
                            .selected_text(
                                self.inspection_reference
                                    .as_deref()
                                    .unwrap_or("Neighboring frames"),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.inspection_reference,
                                    None,
                                    "Neighboring frames",
                                );
                                for (id, i) in frames {
                                    ui.selectable_value(
                                        &mut self.inspection_reference,
                                        Some(id.clone()),
                                        format!("{} · {}", i + 1, id),
                                    );
                                }
                            });
                        if previous != self.inspection_reference {
                            self.rendered = None;
                        }
                    }
                    if let Some(id) = self.inspection_layer.clone() {
                        ui.label(format!("Isolated layer: {id}"));
                        if ui.button("Show all layers").clicked() {
                            self.inspection_layer = None;
                            self.rendered = None;
                        }
                    }
                    if self.review.menu(ui) {
                        self.rendered = None;
                    }
                });
                ui.separator();
                if ui.button("Settings…").clicked() {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("app-settings"), true));
                    ui.close();
                }
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }
    fn controls(&mut self, ctx: &egui::Context) {
        if self.sprite.is_none() {
            return;
        }
        let count = self.project().frames.len();
        if !self.following()
            && count > 1
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Space))
        {
            self.playback.toggle(count);
        }
        if !self.following()
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Home))
        {
            self.playback.stop();
        }
        if !self.following()
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::ArrowRight))
        {
            self.playback
                .select((self.playback.frame + 1) % count, count);
        }
        if !self.following()
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft))
        {
            self.playback
                .select((self.playback.frame + count - 1) % count, count);
        }
        theme::sidebar("animation", ctx).show(ctx, |ui| {
            theme::fill_sidebar(ui);
            if self.following() {
                ui.disable();
            }
            panel_header(ui, |ui| {
                ui.heading("ANIMATION");
            });
            ui.separator();
            ui.spacing_mut().item_spacing.y = theme::metric("gap");
            let layout_selection = self.playback.frame;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 68.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let strip_width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        egui::vec2(strip_width, 68.0),
                        egui::Layout::left_to_right(egui::Align::Min),
                        |ui| {
                            egui::ScrollArea::horizontal()
                                .id_salt("frame-thumbnails")
                                .animated(false)
                                .max_width(strip_width)
                                .min_scrolled_width(0.0)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        for frame in 0..count {
                                            let selected = self.playback.frame == frame;
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(64.0, 68.0),
                                                egui::Sense::click(),
                                            );
                                            if ui.is_rect_visible(rect) {
                                                let border = if selected {
                                                    theme::color("accent")
                                                } else {
                                                    ui.visuals()
                                                        .widgets
                                                        .noninteractive
                                                        .bg_stroke
                                                        .color
                                                };
                                                ui.painter().rect_filled(
                                                    rect,
                                                    2.0,
                                                    if selected {
                                                        theme::color("selected")
                                                    } else {
                                                        theme::color("panel")
                                                    },
                                                );
                                                ui.painter().rect_stroke(
                                                    rect,
                                                    2.0,
                                                    egui::Stroke::new(
                                                        if selected { 2.0_f32 } else { 1.0_f32 },
                                                        border,
                                                    ),
                                                    egui::StrokeKind::Inside,
                                                );
                                                if let Some(texture) = self.thumbnail(ctx, frame) {
                                                    let size = texture.size_vec2();
                                                    let max = egui::vec2(52.0, 48.0);
                                                    let factor = (max.x / size.x)
                                                        .min(max.y / size.y)
                                                        .floor()
                                                        .max(1.0);
                                                    let image_rect = egui::Rect::from_center_size(
                                                        egui::pos2(
                                                            rect.center().x,
                                                            rect.top() + 28.0,
                                                        ),
                                                        size * factor,
                                                    );
                                                    ui.painter().image(
                                                        texture.id(),
                                                        image_rect,
                                                        egui::Rect::from_min_max(
                                                            egui::Pos2::ZERO,
                                                            egui::pos2(1.0, 1.0),
                                                        ),
                                                        egui::Color32::WHITE,
                                                    );
                                                }
                                                ui.painter().text(
                                                    egui::pos2(
                                                        rect.center().x,
                                                        rect.bottom() - 9.0,
                                                    ),
                                                    egui::Align2::CENTER_CENTER,
                                                    format!("{}", frame + 1),
                                                    egui::FontId::monospace(11.0),
                                                    border,
                                                );
                                            }
                                            if selected && self.thumbnail_selection != Some(frame) {
                                                response.scroll_to_me(Some(egui::Align::Center));
                                            }
                                            if response
                                                .on_hover_text(format!(
                                                    "Frame {} · {} ms",
                                                    frame + 1,
                                                    self.project().frames[frame].duration_ms
                                                ))
                                                .clicked()
                                            {
                                                self.playback.select(frame, count);
                                            }
                                        }
                                    });
                                });
                        },
                    );
                },
            );
            self.thumbnail_selection = Some(layout_selection);
            let width = 4.0 * 28.0 + 3.0 * ui.spacing().item_spacing.x;
            let (row, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::hover());
            let center = egui::Rect::from_center_size(row.center(), egui::vec2(width, 28.0));
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(center)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.add_enabled_ui(count > 1, |ui| {
                        if icon_button(ui, Icon::Previous, "Previous frame").clicked() {
                            self.playback
                                .select((self.playback.frame + count - 1) % count, count);
                        }
                        let (icon, label) = if self.playback.playing {
                            (Icon::Pause, "Pause")
                        } else {
                            (Icon::Play, "Play")
                        };
                        if icon_button(ui, icon, label).clicked() {
                            self.playback.toggle(count);
                        }
                        if icon_button(ui, Icon::Stop, "Stop").clicked() {
                            self.playback.stop();
                        }
                        if icon_button(ui, Icon::Next, "Next frame").clicked() {
                            self.playback
                                .select((self.playback.frame + 1) % count, count);
                        }
                    });
                },
            );
        });
    }
    fn status_message(&mut self, now: Instant) -> (String, bool) {
        if self.error != self.last_error {
            self.last_error = self.error.clone();
            self.error_since = self.error.as_ref().map(|_| now);
        }
        if self
            .error_since
            .is_some_and(|since| now.saturating_duration_since(since) < Duration::from_secs(8))
            && let Some(error) = &self.error
        {
            return (error.clone(), true);
        }
        if self.file_picker.is_some() {
            return ("Opening file…".into(), false);
        }
        let status = self.conversation.status();
        (
            if status.is_empty() {
                "Ready".into()
            } else {
                status.to_owned()
            },
            false,
        )
    }
    fn footer(&mut self, ctx: &egui::Context) {
        let (message, is_error) = self.status_message(Instant::now());
        egui::TopBottomPanel::bottom("file-status")
            .exact_height(theme::metric("bar_height"))
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(theme::color("shell"))
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 20.0),
                    egui::Sense::hover(),
                );
                let side = (rect.width() * 0.25).min(110.0);
                let gap = 6.0;
                let left = egui::Rect::from_min_max(
                    rect.min,
                    egui::pos2(rect.left() + side, rect.bottom()),
                );
                let right =
                    egui::Rect::from_min_max(egui::pos2(rect.right() - side, rect.top()), rect.max);
                let center = egui::Rect::from_min_max(
                    egui::pos2(left.right() + gap, rect.top()),
                    egui::pos2(right.left() - gap, rect.bottom()),
                );
                let timing = self
                    .sprite
                    .as_ref()
                    .map(|sprite| {
                        format!(
                            "{:.0} ms/frame",
                            sprite.frames[self.playback.frame].duration_ms / self.speed
                        )
                    })
                    .unwrap_or_default();
                let dimensions = self
                    .sprite
                    .as_ref()
                    .map(|sprite| {
                        let (width, height) = self
                            .canvas_preview
                            .or(self.source_canvas)
                            .unwrap_or((sprite.width, sprite.height));
                        format!("{width} × {height}")
                    })
                    .unwrap_or_default();
                for (bounds, text, align) in [
                    (left, dimensions, egui::Align::Min),
                    (center, message.clone(), egui::Align::Center),
                    (right, timing, egui::Align::Max),
                ] {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(bounds).layout(match align {
                            egui::Align::Min => egui::Layout::left_to_right(egui::Align::Center),
                            egui::Align::Max => egui::Layout::right_to_left(egui::Align::Center),
                            egui::Align::Center => egui::Layout::top_down(align),
                        }),
                        |ui| {
                            let color = if is_error && align == egui::Align::Center {
                                theme::color("error")
                            } else {
                                ui.visuals().text_color()
                            };
                            let font = egui::TextStyle::Body.resolve(ui.style());
                            let measure = |value: &str| {
                                ui.painter()
                                    .layout_no_wrap(value.to_owned(), font.clone(), color)
                                    .size()
                                    .x
                            };
                            let truncated = measure(&text) > bounds.width();
                            let displayed = text.clone();
                            let label =
                                egui::Label::new(egui::RichText::new(displayed).color(color))
                                    .halign(align)
                                    .truncate();
                            let response = if align == egui::Align::Center {
                                ui.add_sized(bounds.size(), label)
                            } else {
                                ui.add(label)
                            };
                            if align == egui::Align::Center && truncated {
                                response.on_hover_ui(|ui| {
                                    if is_error {
                                        ui.visuals_mut().override_text_color =
                                            Some(theme::color("error"));
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            4.0,
                                            theme::color("error_background"),
                                        );
                                    }
                                    ui.label(&text);
                                });
                            }
                        },
                    );
                }
            });
    }
    fn thumbnail(&mut self, ctx: &egui::Context, frame: usize) -> Option<egui::TextureHandle> {
        self.thumbnails
            .resize_with(self.project().frames.len(), || None);
        if self.thumbnails[frame].is_none() {
            match crate::raster::render(self.project(), frame) {
                Ok(rgba) => {
                    let image = square_thumbnail(
                        self.background,
                        self.project().width,
                        self.project().height,
                        &rgba,
                        48,
                    )?;
                    self.thumbnails[frame] = Some(ctx.load_texture(
                        format!("frame-thumbnail-{frame}"),
                        image,
                        egui::TextureOptions::NEAREST,
                    ));
                }
                Err(error) => self.report_error(format!("Thumbnail render failed: {error:#}")),
            }
        }
        self.thumbnails[frame].clone()
    }
}
impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::color("shell").to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        configure_text(ctx);
        chrome::resize_edges(ctx);
        self.title_bar(ctx);
        self.settings_window(ctx);
        self.canvas_dialog(ctx);
        while let Ok(message) = self.messages.try_recv() {
            match message {
                Message::Activate => self.activate(ctx),
                Message::Activity(event) => self.receive_activity(event),
                Message::Inspection(view) => {
                    self.review.normal(self.follow_ai);
                    self.inspection = Some(view);
                    self.inspection_pending = true;
                }
                Message::Review(review) => self.review.receive(review),
                Message::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
        if !self.following()
            && let Some(path) =
                ctx.input(|i| i.raw.dropped_files.iter().find_map(|f| f.path.clone()))
        {
            self.open(path);
        }
        let picked = self.file_picker.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(result) = picked {
            self.file_picker = None;
            match result {
                Ok(Some(path)) => self.open(path),
                Ok(None) => {}
                Err(error) => self.report_error(format!("Open failed: {error:#}")),
            }
        }
        if !self.conversation.active() {
            self.poll();
        }
        self.update_conversation(ctx, false);
        self.follow_activity();
        self.follow_inspection();
        self.follow_review();
        self.activity_bar(ctx);
        let following = self.following();
        self.review.panel(ctx, following);
        if let Some((reference, frame)) = self.review.selected_pair.take() {
            self.inspection = Some(crate::inspection::View {
                frame_id: frame,
                reference_frame_id: Some(reference),
                mode: crate::inspection::Mode::FrameComparison,
                layer_id: self.inspection_layer.clone(),
            });
            self.inspection_pending = true;
            let was_follow = self.follow_ai;
            self.follow_ai = true;
            self.follow_inspection();
            self.follow_ai = was_follow;
        }
        self.footer(ctx);
        let sidebar_area = ctx.available_rect();
        let sidebar_painter = ctx.layer_painter(egui::LayerId::background());
        let sidebar_shape = sidebar_painter.add(egui::Shape::Noop);
        self.activity_rail(ctx);
        self.conversation_panel(ctx);
        if self.sprite.is_none() && (self.show_layers || self.show_animation) {
            theme::sidebar("empty-tool-panel", ctx).show(ctx, |ui| {
                theme::fill_sidebar(ui);
                panel_header(ui, |ui| {
                    ui.heading(if self.show_layers {
                        "LAYERS"
                    } else {
                        "ANIMATION"
                    });
                });
                ui.separator();
                ui.weak("Open artwork to inspect its contents.");
            });
        }
        let now = Instant::now();
        if self.sprite.is_some() {
            self.playback.advance(
                now.duration_since(self.clock).mul_f64(self.speed),
                &self
                    .sprite
                    .as_ref()
                    .expect("loaded project")
                    .frames
                    .iter()
                    .map(|f| f.duration_ms)
                    .collect::<Vec<_>>(),
            );
            self.clock = now;
            if self.show_animation {
                self.controls(ctx);
            }
            let cell = checker_cell(self.canvas_scale.unwrap_or(1.0) * self.zoom);
            let manual = !self.following();
            if self.show_layers
                && self.layer_view.panel(
                    ctx,
                    self.sprite.as_ref().unwrap(),
                    self.playback.frame,
                    self.background,
                    manual,
                )
            {
                self.rendered = None;
            }
            if self.rendered != Some((self.playback.frame, cell)) {
                match self.canvas_pixels() {
                    Ok(rgba) => {
                        let pixels = preview_background_cells(
                            self.background,
                            self.canvas_dimensions().0 as usize,
                            self.canvas_dimensions().1 as usize,
                            &rgba,
                            cell,
                        );
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [
                                self.canvas_dimensions().0 as usize,
                                self.canvas_dimensions().1 as usize,
                            ],
                            &pixels,
                        );
                        match &mut self.texture {
                            Some(texture) => texture.set(image, egui::TextureOptions::NEAREST),
                            None => {
                                self.texture = Some(ctx.load_texture(
                                    "composite",
                                    image,
                                    egui::TextureOptions::NEAREST,
                                ))
                            }
                        }
                        self.rendered = Some((self.playback.frame, cell));
                    }
                    Err(e) => self.report_error(format!("Render failed: {e:#}")),
                }
            }
        }
        let container = egui::Rect::from_min_max(
            sidebar_area.min + egui::Vec2::splat(theme::metric("gap")),
            egui::pos2(
                ctx.available_rect().left(),
                sidebar_area.bottom() - theme::metric("gap"),
            ),
        );
        sidebar_painter.set(
            sidebar_shape,
            egui::epaint::RectShape::new(
                container,
                theme::radius("corner_radius"),
                theme::color("panel"),
                theme::stroke(),
                egui::StrokeKind::Inside,
            ),
        );
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("sidebar-container"), container));
        let panel_open = self.show_layers || self.show_animation || self.conversation.visible;
        let resize_active = [
            "layers",
            "animation",
            "art-conversation",
            "empty-tool-panel",
        ]
        .iter()
        .any(|id| {
            ctx.read_response(egui::Id::new(id).with("__resize"))
                .is_some_and(|r| r.dragged())
        });
        if panel_open
            && (resize_active
                || ctx.pointer_hover_pos().is_some_and(|p| {
                    (p.x - container.right()).abs() <= theme::metric("resize_hit_width")
                        && container.y_range().contains(p.y)
                }))
        {
            sidebar_painter.line_segment(
                [
                    egui::pos2(container.right(), container.top()),
                    egui::pos2(container.right(), container.bottom()),
                ],
                egui::Stroke::new(theme::metric("resize_handle_width"), theme::color("accent")),
            );
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme::color("canvas"))
                    .stroke(theme::stroke())
                    .corner_radius(theme::radius("corner_radius"))
                    .outer_margin(theme::margin("gap"))
                    .inner_margin(theme::margin("panel_padding")),
            )
            .show(ctx, |ui| {
                if self.following() {
                    ui.disable();
                }
                if self.sprite.is_none() {
                    let area = ui.available_rect_before_wrap();
                    let icon_size = theme::metric("empty_icon_size");
                    let icon_rect = egui::Rect::from_center_size(
                        area.center() - egui::vec2(0.0, 24.0),
                        egui::Vec2::splat(icon_size),
                    );
                    icons::paint(ui, "empty_workspace", icon_rect, theme::color("empty_icon"));
                    ui.painter().text(
                        icon_rect.center_bottom() + egui::vec2(0.0, 24.0),
                        egui::Align2::CENTER_TOP,
                        "Open a project or drag artwork here to begin",
                        egui::FontId::proportional(theme::metric("font_size")),
                        theme::color("muted"),
                    );
                }
                if let Some(texture_id) = self.texture.as_ref().map(|texture| texture.id()) {
                    let available = ui.available_rect_before_wrap();
                    let controls = egui::Rect::from_min_size(
                        available.right_bottom() - egui::vec2(36.0, 64.0),
                        egui::vec2(28.0, 56.0),
                    );
                    #[cfg(test)]
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("zoom-controls-test-rect"), controls)
                    });
                    let over_controls = ctx.input(|i| {
                        i.pointer
                            .hover_pos()
                            .is_some_and(|pos| controls.contains(pos))
                            || i.pointer
                                .press_origin()
                                .is_some_and(|pos| controls.contains(pos))
                    });
                    let response =
                        ui.interact(available, egui::Id::new("canvas-pan"), egui::Sense::drag());
                    if !over_controls && !self.following() {
                        response.clone().on_hover_cursor(egui::CursorIcon::Grab);
                    }
                    if !self.following() && response.dragged() && !over_controls {
                        self.pan += response.drag_delta();
                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    let old_zoom = self.zoom;
                    if !self.following() && response.hovered() && !over_controls {
                        let factor = ctx.input(|i| (i.raw_scroll_delta.y * 0.002).exp());
                        self.zoom = (self.zoom * factor).clamp(0.05, 64.0);
                    }
                    if old_zoom != self.zoom {
                        let anchor = ctx
                            .input(|i| i.pointer.hover_pos())
                            .unwrap_or(available.center());
                        self.pan = anchor
                            - available.center()
                            - (anchor - available.center() - self.pan) * (self.zoom / old_zoom);
                    }
                    let ppp = ctx.pixels_per_point();
                    // Fit once on opening; resizing leaves the artwork's pixel scale alone.
                    let initial_scale = ((available.width() * ppp
                        / self.canvas_dimensions().0 as f32)
                        .min(available.height() * ppp / self.canvas_dimensions().1 as f32))
                    .floor()
                    .max(1.0);
                    let scale = *self.canvas_scale.get_or_insert(initial_scale);
                    let size = egui::vec2(
                        self.canvas_dimensions().0 as f32,
                        self.canvas_dimensions().1 as f32,
                    ) * scale
                        * self.zoom
                        / ppp;
                    let rect = egui::Rect::from_min_size(
                        (available.center() + self.pan - size * 0.5).round_to_pixels(ppp),
                        size,
                    );
                    ui.painter().image(
                        texture_id,
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                    self.review.paint_labels(
                        ui.painter(),
                        rect,
                        [self.canvas_dimensions().0, self.canvas_dimensions().1],
                        Some(
                            crate::review::frame_keys(&self.project().frames)[self.playback.frame]
                                .as_str(),
                        ),
                        self.inspection_layer.as_deref(),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(controls), |ui| {
                        if self.following() {
                            ui.disable();
                        }
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.vertical(|ui| {
                            for (icon, label, factor, rounding) in [
                                (
                                    Icon::Faster,
                                    "Zoom in",
                                    1.25,
                                    egui::CornerRadius {
                                        nw: 4,
                                        ne: 4,
                                        sw: 0,
                                        se: 0,
                                    },
                                ),
                                (
                                    Icon::Slower,
                                    "Zoom out",
                                    0.8,
                                    egui::CornerRadius {
                                        nw: 0,
                                        ne: 0,
                                        sw: 4,
                                        se: 4,
                                    },
                                ),
                            ] {
                                if icon_button_shape(ui, icon, label, rounding, false).clicked() {
                                    let previous = self.zoom;
                                    self.zoom = (self.zoom * factor).clamp(0.05, 64.0);
                                    self.pan *= self.zoom / previous;
                                }
                            }
                        });
                        let stroke = ui.visuals().widgets.inactive.bg_stroke;
                        ui.painter()
                            .rect_stroke(controls, 4.0, stroke, egui::StrokeKind::Inside);
                        ui.painter().line_segment(
                            [
                                egui::pos2(controls.left(), controls.center().y),
                                egui::pos2(controls.right(), controls.center().y),
                            ],
                            stroke,
                        );
                    });
                }
            });
        if let Some(remaining) = self.pending_ack {
            if remaining == 0 {
                let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
                if !focused {
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Informational,
                    ));
                }
                reply(
                    json!({"opened":true,"pid":std::process::id(),"activation_requested":true,"focused":focused,"attention_requested":!focused,"activation_note":"Foreground activation is subject to operating system/window-manager policy; no always-on-top or focus-stealing workaround is used."}),
                );
                self.pending_ack = None;
            } else {
                self.pending_ack = Some(remaining - 1);
            }
        }
        self.enforce_window_minimum(ctx);
        ctx.request_repaint_after(Duration::from_millis(
            if self.playback.playing || self.pending_ack.is_some() {
                16
            } else {
                100
            },
        ));
    }
}

fn checker_cell(pixel_scale: f32) -> usize {
    if pixel_scale < 1.0 {
        16
    } else if pixel_scale < 2.0 {
        8
    } else if pixel_scale < 4.0 {
        4
    } else if pixel_scale < 8.0 {
        2
    } else {
        1
    }
}
fn square_thumbnail(
    background: Background,
    width: u32,
    height: u32,
    rgba: &[u8],
    edge: u32,
) -> Option<egui::ColorImage> {
    let source = image::RgbaImage::from_raw(width, height, rgba.to_vec())?;
    let scale = edge as f64 / width.max(height) as f64;
    let w = (width as f64 * scale).round().clamp(1.0, edge as f64) as u32;
    let h = (height as f64 * scale).round().clamp(1.0, edge as f64) as u32;
    let fitted = image::imageops::resize(&source, w, h, image::imageops::FilterType::Nearest);
    let mut square = vec![0; (edge * edge * 4) as usize];
    let (x, y) = ((edge - w) / 2, (edge - h) / 2);
    for row in 0..h as usize {
        let start = (((row + y as usize) * edge as usize) + x as usize) * 4;
        square[start..start + w as usize * 4]
            .copy_from_slice(&fitted.as_raw()[row * w as usize * 4..(row + 1) * w as usize * 4]);
    }
    let pixels = preview_background(background, edge as usize, edge as usize, &square);
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [edge as usize, edge as usize],
        &pixels,
    ))
}

fn preview_background(background: Background, width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    preview_background_cells(background, width, height, rgba, 1)
}
fn preview_background_cells(
    background: Background,
    width: usize,
    height: usize,
    rgba: &[u8],
    cell: usize,
) -> Vec<u8> {
    if background == Background::Transparent {
        return rgba.to_vec();
    }
    let mut out = Vec::with_capacity(rgba.len());
    for y in 0..height {
        for x in 0..width {
            let p = &rgba[(y * width + x) * 4..][..4];
            let checker = match background {
                Background::Checkerboard if (x / cell + y / cell).is_multiple_of(2) => 255,
                Background::Checkerboard => 144,
                Background::Black => 0,
                Background::White => 255,
                Background::Transparent => unreachable!(),
            };
            for &c in &p[..3] {
                out.push(
                    ((u32::from(c) * u32::from(p[3]) + checker * (255 - u32::from(p[3])) + 127)
                        / 255) as u8,
                );
            }
            out.push(255);
        }
    }
    out
}

/// Compatibility entry point for the managed MCP viewer.
pub fn run(path: &Path) -> Result<()> {
    launch(Some(path), true)
}

pub fn inspect(path: &Path) -> Result<()> {
    launch(Some(path), false)
}

/// Both desktop inspection and AI live preview use the same application.
pub fn launch(path: Option<&Path>, managed: bool) -> Result<()> {
    let outcome = (|| -> Result<()> {
        let (tx, messages) = mpsc::channel();
        let mut app = App::empty(messages);
        app.managed = managed;
        if let Some(path) = path {
            app.open(path.to_owned());
        }
        if managed {
            ensure!(path.is_some(), "managed preview requires a source project");
            if let Some(error) = &app.error {
                anyhow::bail!("{error}");
            }
            tx.send(Message::Activate)?;
            std::thread::spawn(move || {
                for line in BufReader::new(std::io::stdin()).lines() {
                    match line.as_deref() {
                        Ok("activate") => {
                            if tx.send(Message::Activate).is_err() {
                                return;
                            }
                        }
                        Ok(line) => {
                            if let Ok(value) = serde_json::from_str::<Value>(line) {
                                let message = if let Some(review) = value.get("review") {
                                    serde_json::from_value(review.clone())
                                        .ok()
                                        .map(Message::Review)
                                } else if let Some(view) = value.get("inspection") {
                                    serde_json::from_value(view.clone())
                                        .ok()
                                        .map(Message::Inspection)
                                } else {
                                    serde_json::from_value(value).ok().map(Message::Activity)
                                };
                                if let Some(message) = message
                                    && tx.send(message).is_err()
                                {
                                    return;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = tx.send(Message::Close);
            });
        }
        let icon = image::load_from_memory_with_format(
            include_bytes!("../assets/pixloom-48.png"),
            image::ImageFormat::Png,
        )
        .context("decode embedded Pixloom window icon")?
        .into_rgba8();
        let icon = egui::IconData {
            width: icon.width(),
            height: icon.height(),
            rgba: icon.into_raw(),
        };
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([640.0, 560.0])
                .with_decorations(false)
                .with_title("Pixloom")
                .with_icon(icon)
                .with_min_inner_size([280.0, 260.0])
                .with_active(true),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        };
        eframe::run_native("Pixloom", options, Box::new(move |cc| {
            #[cfg(windows)]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(handle) = cc.window_handle()
                    && let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    let preference = windows_sys::Win32::Graphics::Dwm::DWMWCP_ROUNDSMALL;
                    // The native handle is valid for this creation callback; DWM copies the value.
                    unsafe { windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                        handle.hwnd.get() as _, windows_sys::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                        (&preference as *const i32).cast(), std::mem::size_of_val(&preference) as u32); }
                }
            }
            #[cfg(not(windows))]
            let _ = cc;
            eprintln!("pixloom: viewer opened (pid {})", std::process::id());
            Ok(Box::new(app))
        })).map_err(|e| anyhow::anyhow!("preview desktop initialization failed: {e}; use an interactive desktop with OpenGL support, or continue headlessly"))?;
        eprintln!("pixloom: viewer closed (pid {})", std::process::id());
        Ok(())
    })();
    if managed && let Err(e) = &outcome {
        reply(json!({"opened":false,"error":format!("{e:#}")}));
    }
    outcome
}

#[cfg(test)]
mod tests {
    use eframe::egui;
    #[test]
    fn rectangular_thumbnail_has_full_square_background_and_correct_artwork_ratio() {
        for (width, height) in [(2, 4), (4, 2)] {
            let rgba = [255, 0, 0, 255].repeat((width * height) as usize);
            let preview =
                super::square_thumbnail(super::Background::Checkerboard, width, height, &rgba, 40)
                    .unwrap();
            assert_eq!(preview.size, [40, 40]);
            assert!(preview.pixels.iter().all(|p| p.a() == 255));
            let red = egui::Color32::RED;
            assert_eq!(preview.pixels.iter().filter(|p| **p == red).count(), 800);
            for y in 0..40 {
                for x in 0..40 {
                    let art = if width < height {
                        (10..30).contains(&x)
                    } else {
                        (10..30).contains(&y)
                    };
                    assert_eq!(preview.pixels[y * 40 + x] == red, art);
                }
            }
        }
    }
    #[test]
    fn minimum_window_size_accounts_for_chat_layers_and_playback() {
        let mut app = app();
        app.conversation.visible = true;
        let full = app.minimum_window_size();
        assert!(full.x >= 640.0 && full.y >= 380.0);
        app.show_layers = false;
        let chat_only = app.minimum_window_size();
        assert!(chat_only.x >= 520.0 && chat_only.x < full.x);
        app.conversation.visible = false;
        assert!(app.minimum_window_size().x < chat_only.x);
    }
    #[test]
    fn canvas_zoom_stays_below_dialogs_and_cannot_be_clicked_through_them() {
        use eframe::App as _;
        let mut app = app();
        let ctx = egui::Context::default();
        let mut frame = eframe::Frame::_new_kittest();
        let mut dialog_layer = None;
        let mut run = |app: &mut super::App, covered, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.update(ctx, &mut frame);
                    if covered {
                        egui::Window::new("Covering dialog")
                            .fixed_pos(egui::Pos2::ZERO)
                            .fixed_size(egui::vec2(900.0, 700.0))
                            .show(ctx, |ui| {
                                ui.set_min_size(egui::vec2(900.0, 700.0));
                                dialog_layer = Some(ui.layer_id());
                            });
                    }
                },
            );
        };
        for _ in 0..3 {
            run(&mut app, true, vec![]);
        }
        let rect = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("zoom-controls-test-rect")))
            .unwrap();
        let point = rect.min + egui::vec2(14.0, 14.0);
        let original = app.zoom;
        for pressed in [true, false] {
            run(
                &mut app,
                true,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(
            app.zoom, original,
            "Covered zoom control must not receive clicks"
        );
        // Stop borrowing the output layer before checking the draw/hit-test order.
        drop(run);
        assert_eq!(ctx.layer_id_at(point), dialog_layer);
    }

    #[test]
    fn settings_search_filters_categories_without_starting_ai() {
        let mut app = app();
        let ctx = egui::Context::default();
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("app-settings"), true);
            d.insert_temp(
                egui::Id::new("settings-navigation"),
                ("visibility".to_owned(), 0_usize),
            );
        });
        let mut output = None;
        for _ in 0..2 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ctx| app.settings_window(ctx),
            ));
        }
        let output = output.unwrap();
        let has_text = |value| {
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==value))
        };
        assert!(has_text("Show layers"));
        assert!(has_text("Show animation"));
        assert!(!has_text("Preview speed"));
        let rect = |label| {
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == label => {
                        Some(t.galley.rect.translate(t.pos.to_vec2()))
                    }
                    _ => None,
                })
                .unwrap()
        };
        assert!(
            rect("Show animation").top() > rect("Show layers").bottom(),
            "Settings controls must stack vertically"
        );
        assert!(!app.conversation.active());
    }

    #[test]
    fn canvas_size_preview_is_reversible_and_cancel_preserves_pixels() {
        let mut app = app();
        let original = crate::raster::render(app.project(), 0).unwrap();
        let ctx = egui::Context::default();
        let id = egui::Id::new("canvas-dialog");
        ctx.data_mut(|d| d.insert_temp(id, (false, 1_u32, 1_u32)));
        let run = |app: &mut super::App, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.canvas_dialog(ctx),
            )
        };
        run(&mut app, vec![]);
        assert_eq!(app.canvas_dimensions(), (1, 1));
        assert_eq!(app.canvas_pixels().unwrap(), original[..4]);
        let (width, height) = (app.project().width + 2, app.project().height + 2);
        ctx.data_mut(|d| d.insert_temp(id, (false, width, height)));
        run(&mut app, vec![]);
        let output = run(&mut app, vec![]);
        let preview = app.canvas_pixels().unwrap();
        assert_eq!(preview.len(), (width * height * 4) as usize);
        assert_eq!(&preview[..4], &original[..4]);
        assert_eq!(&preview[preview.len() - 4..], &[0, 0, 0, 0]);
        let point = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Cancel" => {
                    Some(t.pos + t.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            run(
                &mut app,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert!(app.canvas_preview.is_none());
        assert!(ctx.data(|d| d.get_temp::<(bool, u32, u32)>(id)).is_none());
        assert_eq!(crate::raster::render(app.project(), 0).unwrap(), original);
        assert!(!app.conversation.active());
    }
    #[test]
    fn panel_headers_share_height_and_vertical_alignment() {
        let mut app = app();
        app.conversation.visible = true;
        let ctx = egui::Context::default();
        let mut output = None;
        for _ in 0..2 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    app.title_bar(ctx);
                    app.conversation_panel(ctx);
                    app.layer_view.panel(
                        ctx,
                        app.sprite.as_ref().unwrap(),
                        0,
                        app.background,
                        true,
                    );
                },
            ));
        }
        let output = output.unwrap();
        let center = |label| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap()
        };
        assert!((center("AI CONVERSATION").y - center("LAYERS").y).abs() < 1.0);
        assert!(center("LAYERS").y > 0.0);
    }

    #[test]
    fn menu_order_and_settings_speed_preserve_document_timing() {
        let mut app = app();
        let original = serde_json::to_value(&app.sprite.as_ref().unwrap().frames).unwrap();
        let ctx = egui::Context::default();
        let run = |app: &mut super::App, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.title_bar(ctx);
                    app.settings_window(ctx);
                },
            )
        };
        let click = |app: &mut super::App, point| {
            for pressed in [true, false] {
                run(
                    app,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        };
        run(&mut app, vec![]);
        click(&mut app, egui::pos2(56.0, 18.0));
        let output = run(&mut app, vec![]);
        let center = |output: &egui::FullOutput, label| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing menu item {label}"))
        };
        let mut previous = 0.0;
        for label in [
            "New artwork…",
            "Open file…",
            "Open Project…",
            "Save Project",
            "Save As…",
            "Canvas size…",
            "Switch to sprite view",
            "Layers",
            "Animation",
            "Background",
            "Color mode",
            "Onion skin",
            "Frame comparison",
            "Review",
            "Settings…",
            "Quit",
        ] {
            let y = center(&output, label).y;
            assert!(y > previous, "Menu item out of order: {label}");
            previous = y;
        }
        click(&mut app, center(&output, "Settings…"));
        let output = run(&mut app, vec![]);
        click(&mut app, center(&output, "2×"));
        assert_eq!(app.speed, 2.0);
        assert_eq!(
            serde_json::to_value(&app.sprite.as_ref().unwrap().frames).unwrap(),
            original
        );
    }

    #[test]
    fn manual_save_keeps_watching_the_saved_file_and_preserves_external_refreshes() {
        use crate::session::{CreateProject, Session};
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("manual.pixloom");
        let mut original = Session::default();
        original
            .create(CreateProject {
                width: 2,
                height: 2,
                palette: vec!["#00000000".into(), "#FFFFFFFF".into()],
                output_path: source.clone(),
            })
            .unwrap();
        original.save("project-1").unwrap();
        let mut app = app();
        app.open(source.clone());
        let ctx = eframe::egui::Context::default();
        let save = |app: &mut super::App| {
            app.conversation
                .save_project(&ctx, app.source.as_deref(), false);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while app.conversation.busy {
                let _ = ctx.run(Default::default(), |ctx| app.conversation_panel(ctx));
                assert!(std::time::Instant::now() < until);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        save(&mut app);
        assert_eq!(
            app.source.as_ref().unwrap().canonicalize().unwrap(),
            source.canonicalize().unwrap()
        );
        let mut external = Session::default();
        external.open(&source, None).unwrap();
        external.apply("project-1",0,serde_json::from_value(serde_json::json!([{"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":0,"y":0,"palette_index":1}]}])).unwrap()).unwrap();
        external.save("project-1").unwrap();
        app.modified = None;
        app.checked = std::time::Instant::now() - std::time::Duration::from_millis(200);
        app.poll();
        assert_eq!(
            &crate::raster::render(app.project(), 0).unwrap()[..4],
            &[255, 255, 255, 255]
        );
        save(&mut app);
        assert_eq!(
            &crate::raster::render(&crate::preview::load(&source).unwrap(), 0).unwrap()[..4],
            &[255, 255, 255, 255]
        );
    }

    fn app() -> super::App {
        let sprite = crate::schema::parse(include_bytes!(
            "../skills/pixloom/examples/animated.pixloom.json"
        ))
        .unwrap();
        let (_, messages) = std::sync::mpsc::channel();
        let now = std::time::Instant::now();
        let mut playback = crate::preview::Playback::new(sprite.frames.len());
        playback.stop();
        super::App {
            conversation: Default::default(),
            managed: false,
            source: None,
            opened_source: None,
            source_canvas: None,
            canvas_preview: None,
            speed: 1.0,
            zoom: 1.0,
            canvas_scale: None,
            background: Default::default(),
            layer_view: Default::default(),
            show_layers: true,
            show_animation: false,
            follow_ai: true,
            review: Default::default(),
            activity: None,
            activity_from_embedded: false,
            follow_pending: false,
            inspection: None,
            inspection_pending: false,
            inspection_mode: crate::inspection::Mode::Composite,
            inspection_reference: None,
            inspection_layer: None,
            pan: eframe::egui::Vec2::ZERO,
            file_picker: None,
            alternate: None,
            sheet_view: false,
            sprite: Some(sprite),
            playback,
            texture: None,
            thumbnails: Vec::new(),
            thumbnail_selection: None,
            rendered: None,
            messages,
            checked: now,
            modified: None,
            clock: now,
            pending_ack: None,
            error: None,
            last_error: None,
            error_since: None,
        }
    }
    fn layout(
        app: &mut super::App,
        ctx: &eframe::egui::Context,
        width: f32,
        events: Vec<eframe::egui::Event>,
    ) -> eframe::egui::FullOutput {
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                eframe::egui::Pos2::ZERO,
                eframe::egui::vec2(width, 500.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(
            eframe::egui::RawInput {
                screen_rect: input.screen_rect,
                ..Default::default()
            },
            |ctx| {
                app.footer(ctx);
                app.controls(ctx);
            },
        );
        ctx.run(input, |ctx| {
            app.footer(ctx);
            app.controls(ctx);
            let bottom = ctx.available_rect().bottom();
            ctx.data_mut(|data| data.insert_temp(eframe::egui::Id::new("canvas-bottom"), bottom));
        })
    }
    #[test]
    fn footer_labels_share_padding_and_vertical_alignment() {
        use eframe::egui;
        let mut app = app();
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..Default::default()
            },
            |ctx| app.footer(ctx),
        );
        let label_bottoms: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.pos.y + text.galley.rect.bottom()),
                _ => None,
            })
            .collect();
        assert_eq!(label_bottoms.len(), 3);
        assert!(
            label_bottoms
                .iter()
                .all(|bottom| (bottom - label_bottoms[0]).abs() < 1.0)
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                if text.galley.text().contains("ms/frame") {
                    assert!((text.pos.x + text.galley.rect.right() - 632.0).abs() < 1.0);
                }
                if text.galley.text().contains('×') {
                    assert!((text.pos.x + text.galley.rect.left() - 8.0).abs() < 1.0);
                }
            }
        }
    }
    #[test]
    fn activity_text_stays_centered_with_or_without_spinner() {
        use eframe::egui;
        let mut app = app();
        let ctx = egui::Context::default();
        for busy in [false, true] {
            app.receive_activity(super::Activity {
                label: "Drawing ranger weapons".into(),
                frame_id: None,
                layer_id: None,
                busy,
            });
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ctx| app.activity_bar(ctx),
            );
            let text = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Drawing ranger weapons" => {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("activity without added prefixes");
            assert!((text.pos.x + text.galley.rect.center().x - 320.0).abs() < 1.0);
        }
    }
    #[test]
    fn follow_locks_controls_only_for_an_ai_connected_viewer() {
        let mut app = app();
        assert!(!app.following());
        app.receive_activity(super::Activity {
            label: "Ready".into(),
            frame_id: None,
            layer_id: None,
            busy: false,
        });
        assert!(app.following());
        app.follow_ai = false;
        assert!(!app.following());
    }
    #[test]
    fn ai_follow_selects_explicit_targets_only_when_enabled() {
        let mut app = app();
        let frame_id = crate::edit::frame_id(&app.project().frames[1])
            .unwrap_or("target")
            .to_owned();
        app.sprite.as_mut().unwrap().frames[1]
            .annotations
            .metadata
            .insert(
                crate::edit::FRAME_ID_KEY.into(),
                serde_json::json!(frame_id),
            );
        let event = super::Activity {
            label: "Draw pixels".into(),
            frame_id: Some(frame_id),
            layer_id: Some(app.project().layers[0].id.clone()),
            busy: false,
        };
        app.follow_ai = false;
        app.receive_activity(event.clone());
        app.follow_activity();
        assert_eq!(app.playback.frame, 0);
        app.follow_ai = true;
        app.follow_activity();
        assert_eq!(app.playback.frame, 1);
        assert!(!app.playback.playing);
    }
    #[test]
    fn thumbnails_precede_playback_and_keyboard_selection_pauses() {
        use eframe::egui;
        let mut app = app();
        let ctx = egui::Context::default();
        let output = layout(&mut app, &ctx, 384.0, vec![]);
        let _canvas_bottom = ctx.data(|data| {
            data.get_temp::<f32>(egui::Id::new("canvas-bottom"))
                .unwrap()
        });
        let images: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Mesh(mesh) = &s.shape {
                    (mesh.calc_bounds().width() != super::theme::metric("icon_size"))
                        .then_some(mesh.calc_bounds())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            !images.is_empty(),
            "timeline must draw actual thumbnail images"
        );
        let play = output
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(text) = &s.shape {
                    (text.galley.text().ends_with("ms/frame")).then_some(text.pos.y)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(images.iter().all(|r| r.top() >= 0.0 && r.bottom() < play));
        let original = crate::schema::to_document(app.sprite.as_ref().unwrap()).unwrap();
        app.playback.playing = true;
        let point = images[1].center();
        let _ = layout(
            &mut app,
            &ctx,
            384.0,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let _ = layout(
            &mut app,
            &ctx,
            384.0,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(app.playback.frame, 1);
        assert!(!app.playback.playing);
        let _ = layout(
            &mut app,
            &ctx,
            384.0,
            vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(app.playback.frame, 2);
        assert!(!app.playback.playing);
        assert_eq!(
            serde_json::to_value(crate::schema::to_document(app.sprite.as_ref().unwrap()).unwrap())
                .unwrap(),
            serde_json::to_value(original).unwrap()
        );
    }
    #[test]
    fn overflow_navigation_and_static_controls_are_conditional() {
        use eframe::egui;
        let mut app = app();
        let ctx = egui::Context::default();
        let narrow = layout(&mut app, &ctx, 640.0, vec![]);
        let labels = |output: &egui::FullOutput| {
            output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        Some(t.galley.text().to_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        assert!(!labels(&narrow).contains(&"<".into()));
        assert!(!labels(&narrow).contains(&">".into()));
        app.playback.select(3, 4);
        let _ = layout(&mut app, &ctx, 640.0, vec![]);
        let _ = layout(&mut app, &ctx, 640.0, vec![]);
        assert!(
            app.thumbnails[3].is_some(),
            "selected offscreen frame must scroll into view"
        );
        app.sprite.as_mut().unwrap().frames.truncate(1);
        for l in &mut app.sprite.as_mut().unwrap().layers {
            l.timeline.truncate(1);
        }
        app.playback.reconcile(1);
        app.thumbnail_selection = None;
        let static_layout = layout(&mut app, &ctx, 384.0, vec![]);
        let labels = labels(&static_layout);
        assert!(
            !labels
                .iter()
                .any(|s| ["Play", "Pause", "Stop", "<", ">"].contains(&s.as_str()))
        );
    }
    #[test]
    fn next_frame_button_steps_without_changing_speed_or_saved_timing() {
        use eframe::egui;
        let mut app = app();
        let ctx = egui::Context::default();
        let output = layout(&mut app, &ctx, 384.0, vec![]);
        let _ = output;
        let point = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("icon-button-rect", "Next frame"))))
            .unwrap()
            .center();

        let durations: Vec<_> = app
            .sprite
            .as_ref()
            .unwrap()
            .frames
            .iter()
            .map(|f| f.duration_ms)
            .collect();
        for pressed in [true, false] {
            layout(
                &mut app,
                &ctx,
                384.0,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(app.speed, 1.0);
        assert_eq!(app.playback.frame, 1);
        assert_eq!(
            durations,
            app.sprite
                .as_ref()
                .expect("loaded project")
                .frames
                .iter()
                .map(|f| f.duration_ms)
                .collect::<Vec<_>>()
        );
        let output = layout(&mut app, &ctx, 384.0, vec![]);
        let expected = format!("{:.0} ms/frame", durations[1]);
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == expected))
        );
    }
    #[test]
    fn footer_status_and_corner_labels_are_below_playback_without_file_paths() {
        use eframe::egui;
        for width in [640.0, 280.0] {
            let mut app = app();
            app.source = Some("a-very-long-file-name-to-check-ellipsis.pixloom".into());
            let ctx = egui::Context::default();
            let output = layout(&mut app, &ctx, width, vec![]);
            let button = ctx
                .data(|d| {
                    d.get_temp::<egui::Rect>(egui::Id::new(("icon-button-rect", "Next frame")))
                })
                .unwrap();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t)
                        if t.galley.text().contains("ms/frame")
                            || t.galley.text().contains('×')
                            || t.galley.text() == "Ready" =>
                    {
                        Some(t)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(labels.len(), 3);
            for label in labels {
                assert!(label.pos.y >= button.bottom());
            }
            assert!(!output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().ends_with(".pixloom"))));
        }
    }
    #[test]
    fn status_displays_application_state_without_source_path() {
        let mut app = app();
        app.source = Some("C:/art/projects/flame.pixloom".into());
        assert_eq!(
            app.status_message(std::time::Instant::now()),
            ("Ready".into(), false)
        );
    }
    #[test]
    fn errors_temporarily_replace_filename_then_expire_without_clearing_source() {
        let mut app = app();
        app.source = Some("flame.pixloom".into());
        let now = std::time::Instant::now();
        app.error = Some("Unsupported file".into());
        assert_eq!(app.status_message(now), ("Unsupported file".into(), true));
        assert!(
            app.status_message(now + std::time::Duration::from_secs(7))
                .1
        );
        assert_eq!(
            app.status_message(now + std::time::Duration::from_secs(8)),
            ("Ready".into(), false)
        );
        assert_eq!(app.source.unwrap().file_name().unwrap(), "flame.pixloom");
    }
    #[test]
    fn switching_views_is_reversible_and_plain_images_have_no_switch() {
        let mut app = app();
        let sprite = app.project().clone();
        let pixels = crate::raster::render(&sprite, 0).unwrap();
        app.set_views(sprite, None);
        assert!(app.can_toggle_view());
        app.toggle_view();
        assert!(app.sheet_view);
        assert_eq!(app.project().frames.len(), 1);
        app.toggle_view();
        assert!(!app.sheet_view);
        assert_eq!(crate::raster::render(app.project(), 0).unwrap(), pixels);
    }
    #[test]
    fn empty_app_can_load_a_project_and_does_not_poll_a_missing_source() {
        let (_, messages) = std::sync::mpsc::channel();
        let mut app = super::App::empty(messages);
        app.checked = std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.poll();
        assert!(app.sprite.is_none());
        assert!(app.error.is_none());
        assert!(!app.playback.playing);
        layout(&mut app, &eframe::egui::Context::default(), 640.0, vec![]);
        app.open(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("skills/pixloom/examples/animated.pixloom.json"),
        );
        assert!(app.sprite.is_some());
        assert!(app.source.is_some());
        assert!(app.error.is_none());
    }
    #[test]
    fn opening_projects_resets_selection_and_failed_open_preserves_source() {
        let mut app = app();
        app.playback
            .select(2, app.sprite.as_ref().unwrap().frames.len());
        app.speed = 2.0;
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("skills/pixloom/examples/animated.pixloom.json");
        app.open(path.clone());
        assert_eq!(app.source.as_ref(), Some(&path));
        assert_eq!(app.playback.frame, 0);
        assert!(app.playback.playing);
        assert!(app.error.is_none());
        let count = app.sprite.as_ref().unwrap().frames.len();
        app.open(path.with_extension("missing"));
        assert_eq!(app.source.as_ref(), Some(&path));
        assert_eq!(app.sprite.as_ref().unwrap().frames.len(), count);
        assert!(app.error.is_some());
    }
    #[test]
    fn checker_cells_follow_zoom_and_never_exceed_sixteen_pixels() {
        assert_eq!(
            [0.05, 1.0, 2.0, 4.0, 8.0, 64.0].map(super::checker_cell),
            [16, 8, 4, 2, 1, 1]
        );
    }
    #[test]
    fn preview_backgrounds_preserve_transparency_and_composite_solids() {
        use super::{Background, preview_background};
        let rgba = [200, 100, 50, 0, 100, 50, 0, 255];
        assert_eq!(
            preview_background(Background::Transparent, 2, 1, &rgba),
            rgba
        );
        assert_eq!(
            preview_background(Background::Black, 2, 1, &rgba),
            [0, 0, 0, 255, 100, 50, 0, 255]
        );
        assert_eq!(
            preview_background(Background::White, 2, 1, &rgba),
            [255, 255, 255, 255, 100, 50, 0, 255]
        );
    }
    #[test]
    fn checkerboard_composites_alpha_exactly() {
        assert_eq!(
            super::preview_background(
                super::Background::Checkerboard,
                2,
                1,
                &[255, 0, 0, 255, 0, 0, 0, 0]
            ),
            [255, 0, 0, 255, 144, 144, 144, 255]
        );
    }
    #[test]
    fn inspection_follow_syncs_mode_frames_and_pixels_without_overriding_manual_view() {
        let mut app = app();
        for (i, frame) in app.sprite.as_mut().unwrap().frames.iter_mut().enumerate() {
            frame.annotations.metadata.insert(
                crate::edit::FRAME_ID_KEY.into(),
                serde_json::json!(format!("frame-{i}")),
            );
        }
        app.follow_ai = false;
        app.inspection_mode = crate::inspection::Mode::OnionSkin;
        app.inspection = Some(crate::inspection::View {
            frame_id: "frame-1".into(),
            reference_frame_id: Some("frame-0".into()),
            mode: crate::inspection::Mode::FrameComparison,
            layer_id: None,
        });
        app.inspection_pending = true;
        app.follow_inspection();
        assert_eq!(app.playback.frame, 0);
        assert_eq!(app.inspection_mode, crate::inspection::Mode::OnionSkin);
        assert!(app.inspection_pending);
        app.follow_ai = true;
        app.follow_inspection();
        assert_eq!(app.playback.frame, 1);
        assert_eq!(
            app.inspection_mode,
            crate::inspection::Mode::FrameComparison
        );
        assert!(!app.playback.playing);
        assert!(!app.inspection_pending);
        let (expected, _) = crate::inspection::render(
            app.project(),
            1,
            Some(0),
            crate::inspection::Mode::FrameComparison,
        )
        .unwrap();
        assert_eq!(app.inspection_render().unwrap(), expected);
        app.inspection.as_mut().unwrap().mode = crate::inspection::Mode::OnionSkin;
        app.inspection_pending = true;
        app.follow_inspection();
        let (expected, _) = crate::inspection::render(
            app.project(),
            1,
            Some(0),
            crate::inspection::Mode::OnionSkin,
        )
        .unwrap();
        assert_eq!(app.inspection_render().unwrap(), expected);
        app.inspection.as_mut().unwrap().frame_id = "not-yet-loaded".into();
        app.inspection_pending = true;
        app.follow_inspection();
        assert!(app.inspection_pending);
        assert_eq!(app.playback.frame, 1);
    }
    #[test]
    fn inspection_menu_toggles_manually_and_remains_openable_while_following() {
        use eframe::egui;
        fn frame(
            app: &mut super::App,
            ctx: &egui::Context,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| app.title_bar(ctx),
            )
        }
        fn click(app: &mut super::App, ctx: &egui::Context, point: egui::Pos2) {
            let _ = frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            let _ = frame(
                app,
                ctx,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
        }
        fn text_center(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
        }
        let mut app = app();
        let ctx = egui::Context::default();
        let _ = frame(&mut app, &ctx, vec![]);
        let _ = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, egui::pos2(56.0, 22.0));
        let out = frame(&mut app, &ctx, vec![]);
        let onion = text_center(&out, "Onion skin").expect("menu exposes onion skin toggle");
        assert!(text_center(&out, "Frame comparison").is_some());
        click(&mut app, &ctx, onion);
        assert_eq!(app.inspection_mode, crate::inspection::Mode::OnionSkin);
        assert!(!app.playback.playing);
        // Following must disable the settings, not the menu button itself.
        let _ = frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        app.receive_activity(super::Activity {
            label: "Review animation".into(),
            frame_id: None,
            layer_id: None,
            busy: false,
        });
        let _ = frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, egui::pos2(56.0, 22.0));
        let out = frame(&mut app, &ctx, vec![]);
        let comparison = text_center(&out, "Frame comparison").expect("following menu still opens");
        click(&mut app, &ctx, comparison);
        assert_eq!(
            app.inspection_mode,
            crate::inspection::Mode::OnionSkin,
            "following settings remain read-only"
        );
    }
    #[test]
    fn review_representations_match_shared_pixels_and_following_preserves_manual_state() {
        let mut app = app();
        for (i, f) in app.sprite.as_mut().unwrap().frames.iter_mut().enumerate() {
            f.annotations.metadata.insert(
                crate::edit::FRAME_ID_KEY.into(),
                serde_json::json!(format!("f{i}")),
            );
        }
        let result = crate::review::ReviewResult::report(
            serde_json::json!({"kind":"review_sheet","representation":"grayscale","frame_id":"f1","source_identity":1,"inspected_revision":0}),
        );
        app.follow_ai = false;
        app.review
            .receive(super::review_ui::ViewerReview::new(&result).unwrap());
        app.follow_review();
        assert_eq!(app.playback.frame, 0);
        assert_eq!(
            app.review.representation,
            crate::review::Representation::Color
        );
        app.follow_ai = true;
        app.follow_review();
        assert_eq!(app.playback.frame, 1);
        assert_eq!(
            app.review.representation,
            crate::review::Representation::Grayscale
        );
        let expected = crate::review::render::transform(
            &crate::raster::render(app.project(), 1).unwrap(),
            crate::review::Representation::Grayscale,
        );
        assert_eq!(app.inspection_render().unwrap(), expected);
        app.review.representation = crate::review::Representation::Silhouette;
        assert_eq!(
            app.inspection_render().unwrap(),
            crate::review::render::transform(
                &crate::raster::render(app.project(), 1).unwrap(),
                crate::review::Representation::Silhouette
            )
        );
    }
    #[test]
    fn source_reload_marks_review_stale_and_disables_component_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review.pixloom.json");
        let doc = crate::review::document_for(app().project()).unwrap();
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        let mut app = app();
        app.open(path.clone());
        let frame = crate::review::frame_ids(&doc)[0].clone();
        let mut result = crate::review::measure::sprite(
            &doc,
            &serde_json::from_value(serde_json::json!({"frame_id":frame})).unwrap(),
        )
        .unwrap();
        crate::review::stamp(
            &mut result,
            "test",
            0,
            crate::review::identity(&doc).unwrap(),
        );
        app.review
            .receive(super::review_ui::ViewerReview::new(&result).unwrap());
        app.follow_review();
        assert!(!app.review.stale());
        let mut edited = doc;
        edited.annotations.user_data = "new source".into();
        std::fs::write(&path, serde_json::to_vec(&edited).unwrap()).unwrap();
        app.checked = std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.modified = None;
        app.poll();
        assert!(app.review.stale());
        let raw = crate::raster::render(app.project(), app.playback.frame).unwrap();
        assert_eq!(app.inspection_render().unwrap(), raw);
    }
    #[test]
    fn a_report_without_frame_invalidates_previous_overlay_cache() {
        let mut app = app();
        app.review.representation = crate::review::Representation::Silhouette;
        app.rendered = Some((0, 1));
        let result = crate::review::ReviewResult::report(
            serde_json::json!({"kind":"artwork_validation","passed":true,"source_identity":1,"inspected_revision":0}),
        );
        app.review
            .receive(super::review_ui::ViewerReview::new(&result).unwrap());
        app.follow_review();
        assert!(
            app.rendered.is_none(),
            "report changes must invalidate previously cached inspection overlays"
        );
    }
}
