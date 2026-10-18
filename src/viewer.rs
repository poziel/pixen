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

pub struct ViewerProcess {
    child: Child,
    replies: Receiver<Value>,
}
impl ViewerProcess {
    pub fn open(path: &Path) -> Result<(Self, Value)> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("canvas-viewer")
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

enum Message {
    Activate,
    Close,
}
struct App {
    source: PathBuf,
    sprite: Sprite,
    playback: Playback,
    texture: Option<egui::TextureHandle>,
    thumbnails: Vec<Option<egui::TextureHandle>>,
    thumbnail_selection: Option<usize>,
    rendered: Option<usize>,
    messages: Receiver<Message>,
    checked: Instant,
    modified: Option<SystemTime>,
    clock: Instant,
    pending_ack: Option<u8>,
    error: Option<String>,
}
fn reply(value: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

impl App {
    fn activate(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.pending_ack = Some(2);
    }
    fn poll(&mut self) {
        if self.checked.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.checked = Instant::now();
        match std::fs::metadata(&self.source).and_then(|m| m.modified()) {
            Ok(stamp) if Some(stamp) != self.modified || self.error.is_some() => {
                match preview::load(&self.source) {
                    Ok(sprite) => {
                        self.playback
                            .reconcile_frames(&self.sprite.frames, &sprite.frames);
                        self.sprite = sprite;
                        self.thumbnails.clear();
                        self.thumbnail_selection = None;
                        self.modified = Some(stamp);
                        self.rendered = None;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(format!(
                            "Reload failed: {e:#}. Restore a supported project at this path."
                        ))
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!(
                    "Source unavailable: {e}. Restore {} to resume live updates.",
                    self.source.display()
                ))
            }
            _ => {}
        }
    }
    fn controls(&mut self, ctx: &egui::Context) {
        let count = self.sprite.frames.len();
        if count > 1 && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.playback.toggle(count);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
            self.playback.stop();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            self.playback
                .select((self.playback.frame + 1) % count, count);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            self.playback
                .select((self.playback.frame + count - 1) % count, count);
        }
        egui::TopBottomPanel::bottom("playback")
            .exact_height(if count > 1 { 120.0 } else { 80.0 })
            .show(ctx, |ui| {
                let layout_selection = self.playback.frame;
                ui.horizontal(|ui| {
                    let spacing = ui.spacing().item_spacing.x;
                    let overflow = count as f32 * (64.0 + spacing) - spacing > ui.available_width();
                    let strip_width = (ui.available_width()
                        - if overflow {
                            2.0 * (24.0 + spacing)
                        } else {
                            0.0
                        })
                    .max(1.0);
                    if overflow
                        && ui
                            .add_enabled(
                                self.playback.frame > 0,
                                egui::Button::new("<").min_size(egui::vec2(24.0, 24.0)),
                            )
                            .on_hover_text("Previous frame")
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
                                                    self.sprite.frames[frame].duration_ms
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
                            .add_enabled(
                                self.playback.frame + 1 < count,
                                egui::Button::new(">").min_size(egui::vec2(24.0, 24.0)),
                            )
                            .on_hover_text("Next frame")
                            .clicked()
                    {
                        self.playback.select(self.playback.frame + 1, count);
                    }
                });
                self.thumbnail_selection = Some(layout_selection);
                if count > 1 {
                    ui.horizontal(|ui| {
                        if ui
                            .button(if self.playback.playing {
                                "Pause"
                            } else {
                                "Play"
                            })
                            .clicked()
                        {
                            self.playback.toggle(count);
                        }
                        if ui.button("Stop").clicked() {
                            self.playback.stop();
                        }
                        ui.label(format!("Frame {} / {}", self.playback.frame + 1, count));
                    });
                }
            });
    }
    fn thumbnail(&mut self, ctx: &egui::Context, frame: usize) -> Option<egui::TextureHandle> {
        self.thumbnails
            .resize_with(self.sprite.frames.len(), || None);
        if self.thumbnails[frame].is_none() {
            match crate::raster::render(&self.sprite, frame) {
                Ok(rgba) => {
                    let rgba = checkerboard(
                        self.sprite.width as usize,
                        self.sprite.height as usize,
                        &rgba,
                    );
                    let source =
                        image::RgbaImage::from_raw(self.sprite.width, self.sprite.height, rgba)?;
                    let factor = (48.0 / self.sprite.width.max(self.sprite.height) as f64).min(1.0);
                    let width = (self.sprite.width as f64 * factor).floor().max(1.0) as u32;
                    let height = (self.sprite.height as f64 * factor).floor().max(1.0) as u32;
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
                Err(error) => self.error = Some(format!("Thumbnail render failed: {error:#}")),
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
                Message::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
        self.poll();
        let now = Instant::now();
        self.playback.advance(
            now.duration_since(self.clock),
            &self
                .sprite
                .frames
                .iter()
                .map(|f| f.duration_ms)
                .collect::<Vec<_>>(),
        );
        self.clock = now;
        self.controls(ctx);
        if self.rendered != Some(self.playback.frame) {
            match crate::raster::render(&self.sprite, self.playback.frame) {
                Ok(rgba) => {
                    let pixels = checkerboard(
                        self.sprite.width as usize,
                        self.sprite.height as usize,
                        &rgba,
                    );
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [self.sprite.width as usize, self.sprite.height as usize],
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
                    self.rendered = Some(self.playback.frame);
                }
                Err(e) => self.error = Some(format!("Render failed: {e:#}")),
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(32)))
            .show(ctx, |ui| {
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                if let Some(texture) = &self.texture {
                    let available = ui.available_rect_before_wrap();
                    let ppp = ctx.pixels_per_point();
                    let scale = ((available.width() * ppp / self.sprite.width as f32)
                        .min(available.height() * ppp / self.sprite.height as f32))
                    .floor()
                    .max(1.0);
                    let size = egui::vec2(self.sprite.width as f32, self.sprite.height as f32)
                        * scale
                        / ppp;
                    let rect = egui::Rect::from_min_size(
                        (available.center() - size * 0.5).round_to_pixels(ppp),
                        size,
                    );
                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
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

fn checkerboard(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for y in 0..height {
        for x in 0..width {
            let p = &rgba[(y * width + x) * 4..][..4];
            let checker = if (x / 8 + y / 8).is_multiple_of(2) {
                192
            } else {
                144
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

pub fn run(path: &Path) -> Result<()> {
    let outcome = (|| -> Result<()> {
        let modified = std::fs::metadata(path)?.modified().ok();
        let sprite = preview::load(path)?;
        let count = sprite.frames.len();
        let (tx, messages) = mpsc::channel();
        tx.send(Message::Activate)?;
        std::thread::spawn(move || {
            for line in BufReader::new(std::io::stdin()).lines() {
                match line.as_deref() {
                    Ok("activate") => {
                        if tx.send(Message::Activate).is_err() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            let _ = tx.send(Message::Close);
        });
        let now = Instant::now();
        let source = path.to_owned();
        let scale = (384 / sprite.width.max(sprite.height)).max(1);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([
                    (sprite.width * scale).min(1024) as f32,
                    (sprite.height * scale).min(768) as f32 + if count > 1 { 120.0 } else { 80.0 },
                ])
                .with_title("Pixloom")
                .with_active(true),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        };
        eframe::run_native(
            "Pixloom",
            options,
            Box::new(move |_cc| {
                eprintln!("pixloom: viewer opened (pid {})", std::process::id());
                Ok(Box::new(App {
                    source,
                    sprite,
                    playback: Playback::new(count),
                    texture: None,
                    thumbnails:Vec::new(),thumbnail_selection:None,
                    rendered: None,
                    messages,
                    checked: now,
                    modified,
                    clock: now,
                    pending_ack: None,
                    error: None,
                }))
            }),
        )
        .map_err(|e|anyhow::anyhow!("preview desktop initialization failed: {e}; use an interactive desktop with OpenGL support, or continue headlessly"))?;
        eprintln!("pixloom: viewer closed (pid {})", std::process::id());
        Ok(())
    })();
    if let Err(e) = &outcome {
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
            source: std::path::PathBuf::new(),
            sprite,
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
            app.controls(ctx);
            let bottom = ctx.available_rect().bottom();
            ctx.data_mut(|data| data.insert_temp(eframe::egui::Id::new("canvas-bottom"), bottom));
        })
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
                    (text.galley.text() == "Play").then_some(text.pos.y)
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
        let original = crate::schema::to_document(&app.sprite).unwrap();
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
            serde_json::to_value(crate::schema::to_document(&app.sprite).unwrap()).unwrap(),
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
        assert!(labels(&narrow).contains(&"<".into()));
        assert!(labels(&narrow).contains(&">".into()));
        app.playback.select(3, 4);
        let _ = layout(&mut app, &ctx, 160.0, vec![]);
        let _ = layout(&mut app, &ctx, 160.0, vec![]);
        assert!(
            app.thumbnails[3].is_some(),
            "selected offscreen frame must scroll into view"
        );
        app.sprite.frames.truncate(1);
        for l in &mut app.sprite.layers {
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
    fn checkerboard_composites_alpha_exactly() {
        assert_eq!(
            super::checkerboard(2, 1, &[255, 0, 0, 255, 0, 0, 0, 0]),
            [255, 0, 0, 255, 192, 192, 192, 255]
        );
    }
}
