#[path = "viewer_dialog.rs"]
mod dialog;
#[path = "viewer_layers.rs"]
mod layers;
#[path = "viewer_review.rs"]
pub(crate) mod review_ui;
#[path = "viewer_sheet.rs"]
mod sheet;
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
    Solo,
    Previous,
    Next,
    Play,
    Pause,
    Stop,
    Slower,
    Faster,
}
fn icon_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    icon_button_shape(ui, icon, label, egui::CornerRadius::same(4), true)
}
fn icon_button_shape(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    rounding: egui::CornerRadius,
    border: bool,
) -> egui::Response {
    let compact = matches!(icon, Icon::Reset | Icon::Solo);
    let size = if compact {
        egui::vec2(22.0, 28.0)
    } else {
        egui::vec2(28.0, 28.0)
    };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let visuals = ui.style().interact(&response);
    if !compact {
        ui.painter().rect(
            rect,
            rounding,
            visuals.bg_fill,
            if border {
                visuals.bg_stroke
            } else {
                egui::Stroke::NONE
            },
            egui::StrokeKind::Inside,
        );
    }
    let c = rect.center();
    let color = visuals.fg_stroke.color;
    let stroke = egui::Stroke::new(2.0_f32, color);
    let point = |x, y| c + egui::vec2(x, y);
    match icon {
        Icon::Reset => {
            let points = (0..=16)
                .map(|i| {
                    let angle = -2.4 + i as f32 * 5.0 / 16.0;
                    point(angle.cos() * 6.0, angle.sin() * 6.0)
                })
                .collect();
            ui.painter().add(egui::Shape::line(points, stroke));
            ui.painter()
                .line_segment([point(-7.0, -7.0), point(-4.0, -4.0)], stroke);
            ui.painter()
                .line_segment([point(-4.0, -4.0), point(-1.0, -7.0)], stroke);
        }
        Icon::Solo => {
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(14.0, 14.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter().circle_filled(c, 3.0, color);
        }
        Icon::Menu => {
            for y in [-5.0, 0.0, 5.0] {
                ui.painter()
                    .line_segment([point(-6.0, y), point(6.0, y)], stroke);
            }
        }
        Icon::Previous | Icon::Next => {
            let direction = if matches!(icon, Icon::Previous) {
                -1.0
            } else {
                1.0
            };
            ui.painter().line_segment(
                [point(-3.0 * direction, -6.0), point(3.0 * direction, 0.0)],
                stroke,
            );
            ui.painter().line_segment(
                [point(3.0 * direction, 0.0), point(-3.0 * direction, 6.0)],
                stroke,
            );
        }
        Icon::Play => {
            ui.painter().add(egui::Shape::convex_polygon(
                vec![point(-4.0, -7.0), point(6.0, 0.0), point(-4.0, 7.0)],
                color,
                egui::Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for x in [-4.0, 3.0] {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(point(x, -6.0), egui::vec2(3.0, 12.0)),
                    0.0,
                    color,
                );
            }
        }
        Icon::Stop => {
            ui.painter().rect_filled(
                egui::Rect::from_center_size(c, egui::vec2(12.0, 12.0)),
                1.0,
                color,
            );
        }
        Icon::Slower | Icon::Faster => {
            ui.painter()
                .line_segment([point(-6.0, 0.0), point(6.0, 0.0)], stroke);
            if matches!(icon, Icon::Faster) {
                ui.painter()
                    .line_segment([point(0.0, -6.0), point(0.0, 6.0)], stroke);
            }
        }
    }
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
    source: Option<PathBuf>,
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
    fn following(&self) -> bool {
        self.follow_ai && self.activity.is_some()
    }
    fn receive_activity(&mut self, activity: Activity) {
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
                    .fill(egui::Color32::from_gray(32))
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
                    egui::Color32::from_rgb(50, 140, 105)
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
            source: None,
            sprite: None,
            speed: 1.0,
            zoom: 1.0,
            canvas_scale: None,
            background: Default::default(),
            layer_view: Default::default(),
            show_layers: true,
            show_animation: true,
            follow_ai: true,
            review: Default::default(),
            activity: None,
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
        match preview::load(&path) {
            Ok(sprite) => {
                self.playback = Playback::new(sprite.frames.len());
                self.set_views(sprite, None);
                self.modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
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
    fn canvas_menu(&mut self, ctx: &egui::Context, position: egui::Pos2) {
        egui::Area::new(egui::Id::new("app-menu"))
            .fixed_pos(position)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let response = icon_button(ui, Icon::Menu, "Menu");
                egui::Popup::menu(&response).show(|ui| {
                    if self.following() { ui.disable(); }
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
                    ui.separator();
                    ui.checkbox(&mut self.show_layers, "Show Layers");
                    ui.checkbox(&mut self.show_animation, "Show Animation");
                    if self.review.menu(ui) { self.rendered=None; }
                    ui.separator();
                    let mut onion=self.inspection_mode==crate::inspection::Mode::OnionSkin;
                    if ui.checkbox(&mut onion,"Onion skin").on_hover_text("Red: previous frame. Cyan: next frame. Artwork stays unchanged.").changed() {
                        self.inspection_mode=if onion { crate::inspection::Mode::OnionSkin } else { crate::inspection::Mode::Composite };
                        self.playback.playing=false;
                        self.rendered=None;
                    }
                    let mut comparison=self.inspection_mode==crate::inspection::Mode::FrameComparison;
                    if ui.checkbox(&mut comparison,"Frame comparison").on_hover_text("Pink: changed pixels. Compare to the previous frame or selected reference.").changed() {
                        self.inspection_mode=if comparison { crate::inspection::Mode::FrameComparison } else { crate::inspection::Mode::Composite };
                        self.playback.playing=false;
                        self.rendered=None;
                    }
                    if self.inspection_mode!=crate::inspection::Mode::Composite {
                        let frames=self.sprite.as_ref().map(|s| s.frames.iter().enumerate().map(|(i,f)| (crate::edit::frame_id(f).map(str::to_owned).unwrap_or_else(|| format!("viewer-frame-{i}")),i)).collect::<Vec<_>>()).unwrap_or_default();
                        let previous=self.inspection_reference.clone();
                        egui::ComboBox::from_label("Reference").selected_text(self.inspection_reference.as_deref().unwrap_or("Neighboring frames")).show_ui(ui,|ui| {
                            ui.selectable_value(&mut self.inspection_reference,None,"Neighboring frames");
                            for (id,i) in frames { ui.selectable_value(&mut self.inspection_reference,Some(id.clone()),format!("{} · {}",i+1,id)); }
                        });
                        if previous!=self.inspection_reference { self.rendered=None; }
                    }
                    if let Some(id)=self.inspection_layer.clone() {
                        ui.label(format!("Isolated layer: {id}"));
                        if ui.button("Show all layers").clicked() { self.inspection_layer=None; self.rendered=None; }
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
                });
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
        egui::TopBottomPanel::bottom("playback")
            .exact_height(116.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show(ctx, |ui| {
                if self.following() {
                    ui.disable();
                }
                ui.spacing_mut().item_spacing.y = 6.0;
                let layout_selection = self.playback.frame;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 68.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let spacing = ui.spacing().item_spacing.x;
                        let overflow =
                            count as f32 * (64.0 + spacing) - spacing > ui.available_width();
                        let strip_width = (ui.available_width()
                            - if overflow {
                                2.0 * (28.0 + spacing)
                            } else {
                                0.0
                            })
                        .max(1.0);
                        if overflow
                            && ui
                                .add_enabled_ui(self.playback.frame > 0, |ui| {
                                    icon_button(ui, Icon::Previous, "Previous frame")
                                })
                                .inner
                                .clicked()
                        {
                            self.playback.select(self.playback.frame - 1, count);
                        }
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
                                                        egui::Color32::from_rgb(112, 200, 255)
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
                                                            egui::Color32::from_rgb(32, 58, 82)
                                                        } else {
                                                            egui::Color32::from_gray(32)
                                                        },
                                                    );
                                                    ui.painter().rect_stroke(
                                                        rect,
                                                        2.0,
                                                        egui::Stroke::new(
                                                            if selected {
                                                                2.0_f32
                                                            } else {
                                                                1.0_f32
                                                            },
                                                            border,
                                                        ),
                                                        egui::StrokeKind::Inside,
                                                    );
                                                    if let Some(texture) =
                                                        self.thumbnail(ctx, frame)
                                                    {
                                                        let size = texture.size_vec2();
                                                        let max = egui::vec2(52.0, 48.0);
                                                        let factor = (max.x / size.x)
                                                            .min(max.y / size.y)
                                                            .floor()
                                                            .max(1.0);
                                                        let image_rect =
                                                            egui::Rect::from_center_size(
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
                                                if selected
                                                    && self.thumbnail_selection != Some(frame)
                                                {
                                                    response
                                                        .scroll_to_me(Some(egui::Align::Center));
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
                        if overflow
                            && ui
                                .add_enabled_ui(self.playback.frame + 1 < count, |ui| {
                                    icon_button(ui, Icon::Next, "Next frame")
                                })
                                .inner
                                .clicked()
                        {
                            self.playback.select(self.playback.frame + 1, count);
                        }
                    },
                );
                self.thumbnail_selection = Some(layout_selection);
                let width = 4.0 * 28.0 + 3.0 * ui.spacing().item_spacing.x;
                let (row, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 28.0),
                    egui::Sense::hover(),
                );
                let center = egui::Rect::from_center_size(row.center(), egui::vec2(width, 28.0));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(center)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.add_enabled_ui(count > 1, |ui| {
                            if icon_button(ui, Icon::Slower, "Slower").clicked() {
                                self.speed = (self.speed / 1.25).max(0.125);
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
                            if icon_button(ui, Icon::Faster, "Faster").clicked() {
                                self.speed = (self.speed * 1.25).min(8.0);
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
        (
            self.source
                .as_ref()
                .map(|path| {
                    let absolute = path.canonicalize().unwrap_or_else(|_| path.clone());
                    let text = absolute.to_string_lossy();
                    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
                })
                .unwrap_or_else(|| "No file open".into()),
            false,
        )
    }
    fn footer(&mut self, ctx: &egui::Context) {
        let (message, is_error) = self.status_message(Instant::now());
        egui::TopBottomPanel::bottom("file-status")
            .exact_height(22.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.panel_fill)
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
                let frame = self
                    .sprite
                    .as_ref()
                    .map(|sprite| {
                        format!(
                            "Frame {} / {}",
                            self.playback.frame + 1,
                            sprite.frames.len()
                        )
                    })
                    .unwrap_or_default();
                for (bounds, text, align) in [
                    (left, timing, egui::Align::Min),
                    (center, message.clone(), egui::Align::Center),
                    (right, frame, egui::Align::Max),
                ] {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(bounds).layout(match align {
                            egui::Align::Min => egui::Layout::left_to_right(egui::Align::Center),
                            egui::Align::Max => egui::Layout::right_to_left(egui::Align::Center),
                            egui::Align::Center => egui::Layout::top_down(align),
                        }),
                        |ui| {
                            let color = if is_error && align == egui::Align::Center {
                                egui::Color32::LIGHT_RED
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
                            let displayed =
                                if align == egui::Align::Center && !is_error && truncated {
                                    let chars: Vec<char> = text.chars().collect();
                                    let mut start = 0;
                                    while start < chars.len()
                                        && measure(&format!(
                                            "…{}",
                                            chars[start..].iter().collect::<String>()
                                        )) > bounds.width()
                                    {
                                        start += 1;
                                    }
                                    format!("…{}", chars[start..].iter().collect::<String>())
                                } else {
                                    text.clone()
                                };
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
                                            Some(egui::Color32::LIGHT_RED);
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            4.0,
                                            egui::Color32::from_rgb(65, 25, 25),
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
                    let rgba = preview_background(
                        self.background,
                        self.project().width as usize,
                        self.project().height as usize,
                        &rgba,
                    );
                    let source = image::RgbaImage::from_raw(
                        self.project().width,
                        self.project().height,
                        rgba,
                    )?;
                    let factor = (48.0
                        / self
                            .sprite
                            .as_ref()
                            .expect("loaded project")
                            .width
                            .max(self.project().height) as f64)
                        .min(1.0);
                    let width = (self.project().width as f64 * factor).floor().max(1.0) as u32;
                    let height = (self.project().height as f64 * factor).floor().max(1.0) as u32;
                    let small = image::imageops::resize(
                        &source,
                        width,
                        height,
                        image::imageops::FilterType::Nearest,
                    );
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        small.as_raw(),
                    );
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
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
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
        self.poll();
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
                match self.inspection_render() {
                    Ok(rgba) => {
                        let pixels = preview_background_cells(
                            self.background,
                            self.project().width as usize,
                            self.project().height as usize,
                            &rgba,
                            cell,
                        );
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [
                                self.project().width as usize,
                                self.project().height as usize,
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
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(32)))
            .show(ctx, |ui| {
                let canvas = ui.available_rect_before_wrap();
                self.canvas_menu(ctx, canvas.min + egui::vec2(8.0, 8.0));
                if self.following() {
                    ui.disable();
                }
                if self.sprite.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("Open a pixel art project");
                            ui.label("Drag a file here or choose Open…");
                            ui.label("Pixloom · Pixelorama · Aseprite · PNG · JPEG · GIF · WebP");
                        });
                    });
                }
                if let Some(texture_id) = self.texture.as_ref().map(|texture| texture.id()) {
                    let available = ui.available_rect_before_wrap();
                    let controls = egui::Rect::from_min_size(
                        available.right_bottom() - egui::vec2(36.0, 64.0),
                        egui::vec2(28.0, 56.0),
                    );
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
                    let initial_scale = ((available.width() * ppp / self.project().width as f32)
                        .min(available.height() * ppp / self.project().height as f32))
                    .floor()
                    .max(1.0);
                    let scale = *self.canvas_scale.get_or_insert(initial_scale);
                    let size =
                        egui::vec2(self.project().width as f32, self.project().height as f32)
                            * scale
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
                        [self.project().width, self.project().height],
                        Some(
                            crate::review::frame_keys(&self.project().frames)[self.playback.frame]
                                .as_str(),
                        ),
                        self.inspection_layer.as_deref(),
                    );
                    egui::Area::new(egui::Id::new("canvas-zoom-controls"))
                        .fixed_pos(controls.min)
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
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
                                    if icon_button_shape(ui, icon, label, rounding, false).clicked()
                                    {
                                        let previous = self.zoom;
                                        self.zoom = (self.zoom * factor).clamp(0.05, 64.0);
                                        self.pan *= self.zoom / previous;
                                    }
                                }
                            });
                            let stroke = ui.visuals().widgets.inactive.bg_stroke;
                            ui.painter().rect_stroke(
                                controls,
                                4.0,
                                stroke,
                                egui::StrokeKind::Inside,
                            );
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
                .with_title("Pixloom")
                .with_icon(icon)
                .with_min_inner_size([280.0, 260.0])
                .with_active(true),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        };
        eframe::run_native("Pixloom", options, Box::new(move |_cc| {
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
            source: None,
            speed: 1.0,
            zoom: 1.0,
            canvas_scale: None,
            background: Default::default(),
            layer_view: Default::default(),
            show_layers: true,
            show_animation: true,
            follow_ai: true,
            review: Default::default(),
            activity: None,
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
                    assert!((text.pos.x + text.galley.rect.left() - 8.0).abs() < 1.0);
                }
                if text.galley.text().starts_with("Frame ") {
                    assert!((text.pos.x + text.galley.rect.right() - 632.0).abs() < 1.0);
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
        let canvas_bottom = ctx.data(|data| {
            data.get_temp::<f32>(egui::Id::new("canvas-bottom"))
                .unwrap()
        });
        let images: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Mesh(mesh) = &s.shape {
                    Some(mesh.calc_bounds())
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
        assert!(
            images
                .iter()
                .all(|r| r.top() >= canvas_bottom && r.bottom() < play)
        );
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
        let narrow = layout(&mut app, &ctx, 160.0, vec![]);
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
        let _ = layout(&mut app, &ctx, 160.0, vec![]);
        let _ = layout(&mut app, &ctx, 160.0, vec![]);
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
    fn faster_icon_updates_effective_timing_without_editing_frames() {
        use eframe::egui;
        let mut app = app();
        let ctx = egui::Context::default();
        let output = layout(&mut app, &ctx, 384.0, vec![]);
        let buttons: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.rect.size() == egui::vec2(28.0, 28.0) => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(buttons.len(), 4);
        assert!((buttons[0].left() + buttons[3].right() - 384.0).abs() < 2.0);
        let point = buttons[3].center();
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
        assert_eq!(app.speed, 1.25);
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
        let expected = format!("{:.0} ms/frame", durations[0] / 1.25);
        assert!(
            output
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == expected))
        );
    }
    #[test]
    fn filename_and_corner_labels_are_below_playback_and_truncate_when_narrow() {
        use eframe::egui;
        for width in [640.0, 280.0] {
            let mut app = app();
            app.source = Some("a-very-long-file-name-to-check-ellipsis.pixloom".into());
            let ctx = egui::Context::default();
            let output = layout(&mut app, &ctx, width, vec![]);
            let buttons: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r)
                        if r.rect.size() == egui::vec2(28.0, 28.0) && r.rect.bottom() > 450.0 =>
                    {
                        Some(r.rect)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(buttons.len(), 4);
            assert!((buttons[0].left() + buttons[3].right() - width).abs() < 1.0);
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t)
                        if t.galley.text().contains("ms/frame")
                            || t.galley.text().starts_with("Frame ")
                            || t.galley.text().ends_with(".pixloom") =>
                    {
                        Some(t)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(labels.len(), 3);
            for label in labels {
                assert!(label.pos.y >= buttons[0].bottom());
            }
            let name = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text().ends_with(".pixloom") => Some(t),
                    _ => None,
                })
                .unwrap();
            assert_eq!(name.galley.text().starts_with("…"), width == 280.0);
        }
    }
    #[test]
    fn status_preserves_the_full_source_path() {
        let mut app = app();
        app.source = Some("C:/art/projects/flame.pixloom".into());
        assert_eq!(
            app.status_message(std::time::Instant::now()),
            ("C:/art/projects/flame.pixloom".into(), false)
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
            ("flame.pixloom".into(), false)
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
                |ctx| app.canvas_menu(ctx, egui::pos2(8.0, 8.0)),
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
        click(&mut app, &ctx, egui::pos2(22.0, 22.0));
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
        click(&mut app, &ctx, egui::pos2(22.0, 22.0));
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
