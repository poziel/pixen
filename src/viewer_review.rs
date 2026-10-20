//! Inspector presentation for read-only review reports. Artwork remains owned by Session.
use crate::review::{Representation, ReviewResult};
use anyhow::{Result, ensure};
use base64::Engine;
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireImage {
    role: String,
    png: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ViewerReview {
    pub report: Value,
    images: Vec<WireImage>,
}
impl ViewerReview {
    pub fn new(result: &ReviewResult) -> Result<Self> {
        ensure!(
            result.images.iter().map(|i| i.png.len()).sum::<usize>() <= 40 * 1024 * 1024,
            "inspector review payload too large; reduce scales/views/frames"
        );
        let r = Self {
            report: result.report.clone(),
            images: result
                .images
                .iter()
                .map(|i| WireImage {
                    role: i.role.clone(),
                    png: base64::engine::general_purpose::STANDARD.encode(&i.png),
                })
                .collect(),
        };
        ensure!(
            serde_json::to_vec(&r)?.len() <= 64 * 1024 * 1024,
            "inspector review exceeds 64 MiB"
        );
        Ok(r)
    }
    pub fn frame_id(&self) -> Option<&str> {
        self.report["frame_id"]
            .as_str()
            .or_else(|| self.report["frames"][0]["frame_id"].as_str())
    }
}
#[derive(Clone, Copy)]
pub(super) enum LocalReview {
    Analyze,
    Sheet,
    Animation,
}
#[derive(Default)]
pub(super) struct ReviewState {
    pub representation: Representation,
    pub changed: bool,
    pub annotations: bool,
    pub source_identity: Option<u64>,
    latest: Option<ViewerReview>,
    active: Option<ViewerReview>,
    pending: bool,
    pending_live: bool,
    identity_checked: Option<std::time::Instant>,
    show: bool,
    textures: Vec<(String, egui::TextureHandle)>,
    pub local: Option<LocalReview>,
    pub selected_pair: Option<(String, String)>,
    zoom: f32,
}
impl ReviewState {
    pub fn receive(&mut self, r: ViewerReview) {
        self.latest = Some(r);
        self.pending = true;
        self.pending_live = false;
    }
    pub fn normal(&mut self, following: bool) {
        self.latest = None;
        self.pending = false;
        if following {
            self.live();
        } else {
            self.pending_live = true;
        }
    }
    pub fn check_identity_due(&mut self) -> bool {
        if self.active.is_none() && self.latest.is_none() {
            return false;
        }
        if self
            .identity_checked
            .is_none_or(|t| t.elapsed() >= std::time::Duration::from_millis(500))
        {
            self.identity_checked = Some(std::time::Instant::now());
            true
        } else {
            false
        }
    }
    pub fn resume(&mut self) {
        self.pending = self.latest.is_some();
    }
    pub fn apply(&mut self, following: bool) -> Option<crate::inspection::View> {
        if following && self.pending_live {
            self.pending_live = false;
            self.live();
            return None;
        }
        if !following || !self.pending {
            return None;
        }
        let r = self.latest.clone()?;
        self.pending = false;
        self.changed = true;
        self.show = true;
        self.textures.clear();
        self.zoom = 1.0;
        if let Some(representation) = r.report.get("representation") {
            self.representation =
                serde_json::from_value(representation.clone()).unwrap_or_default();
        } else if r.frame_id().is_some() {
            self.representation = Representation::Color;
        }
        self.annotations = r.report["kind"] == "sprite_analysis";
        let target = r.frame_id().map(|f| crate::inspection::View {
            frame_id: f.into(),
            reference_frame_id: None,
            mode: crate::inspection::Mode::Composite,
            layer_id: r.report["layer_id"].as_str().map(str::to_owned),
        });
        self.active = Some(r);
        target
    }
    pub fn local_result(&mut self, r: ViewerReview) {
        self.receive(r);
        self.apply(true);
    }
    pub fn live(&mut self) {
        self.changed = true;
        self.active = None;
        self.show = false;
        self.textures.clear();
        self.representation = Representation::Color;
        self.annotations = false;
        self.pending = false;
        self.pending_live = false;
    }
    pub fn stale(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|r| r.report["source_identity"].as_u64() != self.source_identity)
    }
    pub fn color_menu(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.menu_button("Color mode", |ui| {
            for (mode, label) in [
                (Representation::Color, "Color"),
                (Representation::Silhouette, "Silhouette"),
                (Representation::Grayscale, "Grayscale"),
            ] {
                changed |= ui
                    .selectable_value(&mut self.representation, mode, label)
                    .changed();
            }
        });
        changed
    }
    pub fn menu(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.menu_button("Review", |ui| {
            changed |= ui
                .add_enabled(
                    self.active
                        .as_ref()
                        .is_some_and(|r| r.report["kind"] == "sprite_analysis")
                        && !self.stale(),
                    egui::Checkbox::new(&mut self.annotations, "Component bounds"),
                )
                .on_hover_text("Analyze the current frame to compute component bounds.")
                .changed();
            if ui.button("Analyze current frame").clicked() {
                self.local = Some(LocalReview::Analyze);
                ui.close();
            }
            if ui.button("Review sheet").clicked() {
                self.local = Some(LocalReview::Sheet);
                ui.close();
            }
            if ui.button("Audit animation").clicked() {
                self.local = Some(LocalReview::Animation);
                ui.close();
            }
            if self.active.is_some() {
                ui.checkbox(&mut self.show, "Show review report");
                if let Some(id) = self
                    .active
                    .as_ref()
                    .and_then(|r| r.report["checkpoint_id"].as_str())
                {
                    ui.label(format!("Checkpoint: {id}"));
                }
                if ui.button("Return to live artwork").clicked() {
                    self.live();
                    changed = true;
                    ui.close();
                }
            }
        });
        changed
    }
    pub fn annotate(
        &self,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        frame_id: Option<&str>,
        layer_id: Option<&str>,
    ) -> Vec<u8> {
        if !self.annotations || self.stale() {
            return rgba;
        }
        let Some(r) = &self.active else {
            return rgba;
        };
        if r.report["kind"] != "sprite_analysis"
            || r.frame_id() != frame_id
            || r.report["layer_id"].as_str() != layer_id
        {
            return rgba;
        }
        let Some(mut image) = image::RgbaImage::from_raw(width, height, rgba.clone()) else {
            return rgba;
        };
        if let Some(components) = r.report["components"].as_array() {
            for c in components {
                let b = &c["bounds"];
                let x = b["x"].as_u64().unwrap_or(u64::MAX);
                let y = b["y"].as_u64().unwrap_or(u64::MAX);
                let w = b["width"].as_u64().unwrap_or(0);
                let h = b["height"].as_u64().unwrap_or(0);
                if w == 0
                    || h == 0
                    || x >= u64::from(width)
                    || y >= u64::from(height)
                    || x + w > u64::from(width)
                    || y + h > u64::from(height)
                {
                    continue;
                }
                for dx in x..x + w {
                    image.put_pixel(dx as u32, y as u32, image::Rgba([255, 190, 60, 255]));
                    image.put_pixel(
                        dx as u32,
                        (y + h - 1) as u32,
                        image::Rgba([255, 190, 60, 255]),
                    );
                }
                for dy in y..y + h {
                    image.put_pixel(x as u32, dy as u32, image::Rgba([255, 190, 60, 255]));
                    image.put_pixel(
                        (x + w - 1) as u32,
                        dy as u32,
                        image::Rgba([255, 190, 60, 255]),
                    );
                }
            }
        }
        image.into_raw()
    }
    pub fn paint_labels(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        size: [u32; 2],
        frame_id: Option<&str>,
        layer_id: Option<&str>,
    ) {
        if !self.annotations || self.stale() {
            return;
        }
        let Some(r) = &self.active else {
            return;
        };
        if r.report["kind"] != "sprite_analysis"
            || r.frame_id() != frame_id
            || r.report["layer_id"].as_str() != layer_id
        {
            return;
        }
        if let Some(components) = r.report["components"].as_array() {
            for c in components {
                let b = &c["bounds"];
                let x = b["x"].as_f64().unwrap_or(0.0) as f32;
                let y = b["y"].as_f64().unwrap_or(0.0) as f32;
                let pos = egui::pos2(
                    rect.left() + x / size[0] as f32 * rect.width() + 2.0,
                    (rect.top() + y / size[1] as f32 * rect.height() - 14.0).max(rect.top()),
                );
                let galley = painter.layout_no_wrap(
                    c["component_id"].to_string(),
                    egui::FontId::monospace(12.0),
                    egui::Color32::WHITE,
                );
                painter.rect_filled(
                    egui::Rect::from_min_size(pos, galley.size()),
                    2.0,
                    egui::Color32::from_black_alpha(210),
                );
                painter.galley(pos, galley, egui::Color32::WHITE);
            }
        }
    }
    pub fn panel(&mut self, ctx: &egui::Context, following: bool) {
        if !self.show {
            return;
        }
        let Some(active) = self.active.clone() else {
            return;
        };
        let stale = self.stale();
        let r = &active.report;
        egui::Window::new("Art review")
            .collapsible(false)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(16))
            .id(egui::Id::new("art-review-report"))
            .default_width(540.0)
            .default_height(420.0)
            .resizable(true)
            .open(&mut self.show)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                ui.add_space(8.0);
                ui.label(format!(
                    "{} · revision {}",
                    r["kind"].as_str().unwrap_or("Review"),
                    r["inspected_revision"]
                ));
                if stale {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "Stale report — artwork has changed since this review.",
                    );
                }
                if let Some(passed) = r["passed"].as_bool() {
                    ui.label(if passed {
                        "All supplied rules passed"
                    } else {
                        "Some supplied rules failed"
                    });
                }
                if r["kind"] == "sprite_analysis" {
                    ui.label(format!(
                        "Occupied pixels: {} · components: {} · rendered colors: {}",
                        r["occupied_pixels"], r["component_count"], r["rendered_color_count"]
                    ));
                    ui.label(format!("Bounds: {}", r["bounds"]));
                }
                if r["kind"] == "animation_audit" {
                    ui.label(format!(
                        "Duration: {}ms · distinct poses: {}",
                        r["total_duration_ms"], r["distinct_poses"]
                    ));
                }
                if let Some(transitions) = r["transitions"].as_array() {
                    ui.collapsing("Transitions", |ui| {
                        for t in transitions {
                            let label = format!(
                                "{} → {} · {} changed pixels",
                                t["from_frame_id"].as_str().unwrap_or("?"),
                                t["to_frame_id"].as_str().unwrap_or("?"),
                                t["diff"]["changed_pixels"]
                            );
                            if ui
                                .add_enabled(!following && !stale, egui::Button::new(label))
                                .clicked()
                            {
                                self.selected_pair = Some((
                                    t["from_frame_id"].as_str().unwrap().into(),
                                    t["to_frame_id"].as_str().unwrap().into(),
                                ));
                            }
                        }
                    });
                }
                if let Some(findings) = r["findings"].as_array() {
                    for f in findings.iter().take(16) {
                        ui.label(format!(
                            "{}: {}",
                            f["code"].as_str().unwrap_or("finding"),
                            f["message"].as_str().unwrap_or("Inspect evidence")
                        ));
                    }
                }
                if self.textures.is_empty() {
                    for wire in &active.images {
                        let decoded = (|| -> Result<image::RgbaImage> {
                            let bytes =
                                base64::engine::general_purpose::STANDARD.decode(&wire.png)?;
                            let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
                                .with_guessed_format()?;
                            let mut limits = image::Limits::default();
                            limits.max_alloc = Some(128 * 1024 * 1024);
                            reader.limits(limits);
                            let img = reader.decode()?.to_rgba8();
                            ensure!(
                                img.width() as u64 * img.height() as u64
                                    <= crate::review::MAX_IMAGE_PIXELS,
                                "oversized review image"
                            );
                            Ok(img)
                        })();
                        if let Ok(img) = decoded {
                            let max_side = ctx.input(|i| i.max_texture_side).clamp(1, 4096) as u32;
                            let img = fit_texture(img, max_side);
                            let size = [img.width() as usize, img.height() as usize];
                            self.textures.push((
                                wire.role.clone(),
                                ctx.load_texture(
                                    format!("review-{}", self.textures.len()),
                                    egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
                                    egui::TextureOptions::NEAREST,
                                ),
                            ));
                        } else {
                            ui.label("Review image could not be decoded within inspector limits.");
                        }
                    }
                }
                ui.add(egui::Slider::new(&mut self.zoom, 0.25..=8.0).text("Image scale"));
                egui::ScrollArea::both().max_height(500.0).show(ui, |ui| {
                    for (role, texture) in &self.textures {
                        ui.label(role);
                        ui.add(
                            egui::Image::new(texture)
                                .fit_to_exact_size(texture.size_vec2() * self.zoom),
                        );
                    }
                });
                ui.collapsing("Evidence JSON", |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            ui.monospace(serde_json::to_string_pretty(r).unwrap_or_default());
                        });
                });
            });
    }
}

fn fit_texture(img: image::RgbaImage, max_side: u32) -> image::RgbaImage {
    let max_side = max_side.max(1);
    if img.width() <= max_side && img.height() <= max_side {
        return img;
    }
    let ratio = (max_side as f64 / img.width() as f64).min(max_side as f64 / img.height() as f64);
    image::imageops::resize(
        &img,
        ((img.width() as f64 * ratio).floor() as u32).max(1),
        ((img.height() as f64 * ratio).floor() as u32).max(1),
        image::imageops::FilterType::Nearest,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn gpu_preview_fitting_keeps_aspect_ratio() {
        let img = image::RgbaImage::new(6000, 1000);
        let img = fit_texture(img, 2048);
        assert_eq!(img.width(), 2048);
        assert_eq!(img.height(), 341);
    }
    fn report(identity: u64) -> ViewerReview {
        ViewerReview {
            report: json!({"kind":"sprite_analysis","frame_id":"f","representation":"silhouette","source_identity":identity,"inspected_revision":3}),
            images: Vec::new(),
        }
    }
    #[test]
    fn manual_review_stays_fixed_until_following_and_normal_view_replaces_pending_report() {
        let mut s = ReviewState::default();
        s.receive(report(1));
        assert!(s.apply(false).is_none());
        assert_eq!(s.representation, Representation::Color);
        assert!(s.apply(true).is_some());
        assert_eq!(s.representation, Representation::Silhouette);
        s.source_identity = Some(1);
        assert!(!s.stale());
        s.source_identity = Some(2);
        assert!(s.stale());
        s.receive(report(2));
        s.normal(false);
        s.resume();
        assert!(s.apply(true).is_none());
        assert_eq!(s.representation, Representation::Color);
    }
}
