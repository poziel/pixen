//! Frontend-independent application service. No webview, window, or event-loop types.
use crate::{
    inspection,
    model::Sprite,
    review,
    viewer_protocol::{Activity, ViewerReview},
};
use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Serialize)]
pub struct FrameInfo {
    pub id: String,
    pub duration_ms: f64,
}
#[derive(Clone, Serialize)]
pub struct LayerInfo {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub opacity: f64,
}
#[derive(Clone, Serialize)]
pub struct DocumentInfo {
    pub generation: u64,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameInfo>,
    pub layers: Vec<LayerInfo>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub source_width: u32,
    pub source_height: u32,
    pub alternate_view: bool,
    pub can_toggle_view: bool,
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub document: Option<DocumentInfo>,
    pub managed: bool,
    pub activity: Option<Activity>,
    pub inspection: Option<inspection::View>,
    pub review: Option<ViewerReview>,
    pub event_serial: u64,
    pub review_stale: bool,
}
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RenderRequest {
    pub generation: u64,
    pub frame_id: String,
    pub hidden_layers: BTreeSet<String>,
    pub layer_id: Option<String>,
    pub reference_frame_id: Option<String>,
    pub mode: inspection::Mode,
    pub representation: review::Representation,
    pub sheet: bool,
    pub canvas_width: Option<u32>,
    pub canvas_height: Option<u32>,
    pub override_visibility: bool,
    pub annotations: bool,
    pub event_serial: Option<u64>,
}
#[derive(Serialize)]
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub png: String,
}

pub struct Application {
    managed: bool,
    desktop: Option<crate::mcp::desktop::Desktop>,
    directory: Option<tempfile::TempDir>,
    sprite: Option<Sprite>,
    alternate_sprite: Option<Sprite>,
    alternate_view: bool,
    source: Option<PathBuf>,
    stamp: Option<(SystemTime, u64)>,
    destination: Option<PathBuf>,
    revision: u64,
    saved_revision: u64,
    generation: u64,
    activity: Option<Activity>,
    inspection: Option<inspection::View>,
    review: Option<ViewerReview>,
    event_serial: u64,
    review_generation: Option<u64>,
}
impl Application {
    pub fn new(managed: bool) -> Self {
        Self {
            managed,
            desktop: None,
            directory: None,
            sprite: None,
            alternate_sprite: None,
            alternate_view: false,
            source: None,
            stamp: None,
            destination: None,
            revision: 0,
            saved_revision: 0,
            generation: 0,
            activity: None,
            inspection: None,
            review: None,
            event_serial: 0,
            review_generation: None,
        }
    }
    fn file_stamp(path: &Path) -> Result<(SystemTime, u64)> {
        let info = std::fs::metadata(path)?;
        Ok((info.modified()?, info.len()))
    }
    pub fn open(&mut self, path: &Path) -> Result<()> {
        let path = crate::output::resolved(path)?;
        let sprite = crate::preview::load(&path)?;
        let directory = tempfile::Builder::new()
            .prefix("pixloom-desktop-")
            .tempdir()?;
        let desktop = if self.managed {
            None
        } else {
            Some(crate::mcp::desktop::Desktop::for_save(
                directory.path().to_owned(),
                Some(&path),
            )?)
        };
        let native = path.extension().is_some_and(|x| x == "pixloom")
            || path.to_string_lossy().ends_with(".pixloom.json");
        self.cleanup_saved_recovery();
        self.stamp = Some(Self::file_stamp(&path)?);
        self.source = Some(path.clone());
        self.destination = native.then_some(path);
        self.alternate_view = false;
        self.update_sprite(sprite);
        self.desktop = desktop;
        self.directory = Some(directory);
        self.revision = self
            .desktop
            .as_ref()
            .and_then(|d| d.snapshot(None))
            .map(|s| s.0)
            .unwrap_or(0);
        self.saved_revision = self.revision;
        self.generation += 1;
        self.activity = None;
        self.inspection = None;
        self.review = None;
        self.event_serial += 1;
        Ok(())
    }
    pub fn create(&mut self, width: u32, height: u32) -> Result<()> {
        ensure!(!self.managed, "managed preview is read-only");
        let directory = tempfile::Builder::new()
            .prefix("pixloom-desktop-")
            .tempdir()?;
        let desktop = crate::mcp::desktop::Desktop::new(directory.path().to_owned(), None)?;
        desktop.create_artwork(width, height)?;
        let (revision, sprite, _) = desktop
            .snapshot(None)
            .context("new document snapshot unavailable")?;
        self.cleanup_saved_recovery();
        self.desktop = Some(desktop);
        self.directory = Some(directory);
        self.alternate_view = false;
        self.update_sprite(sprite);
        self.source = None;
        self.destination = None;
        self.stamp = None;
        self.revision = revision;
        self.saved_revision = revision;
        self.generation += 1;
        self.activity = None;
        self.inspection = None;
        self.review = None;
        self.event_serial += 1;
        Ok(())
    }
    fn cleanup_saved_recovery(&self) {
        if self.saved_revision == self.revision
            && let Some((_, _, recovery)) = self.desktop.as_ref().and_then(|d| d.snapshot(None))
        {
            let _ = std::fs::remove_file(recovery);
        }
    }
    fn refresh_document(&mut self) {
        if let Some((revision, sprite, _)) = self
            .desktop
            .as_ref()
            .and_then(|d| d.snapshot(Some(self.revision)))
        {
            self.revision = revision;
            self.update_sprite(sprite);
            self.generation += 1;
        }
    }
    fn update_sprite(&mut self, sprite: Sprite) {
        self.alternate_sprite = if sprite.frames.len() == 1 {
            crate::sheet::animation(&sprite).ok().flatten()
        } else if self.alternate_view {
            crate::sheet::sheet(&sprite).ok()
        } else {
            None
        };
        if self.alternate_sprite.is_none() {
            self.alternate_view = false;
        }
        self.sprite = Some(sprite);
    }
    fn presented(&self) -> Option<&Sprite> {
        if self.alternate_view {
            self.alternate_sprite.as_ref()
        } else {
            self.sprite.as_ref()
        }
    }
    pub fn toggle_view(&mut self) -> Result<()> {
        let source = self.sprite.as_ref().context("no document")?;
        if self.alternate_sprite.is_none() {
            self.alternate_sprite = Some(if source.frames.len() > 1 {
                crate::sheet::sheet(source)?
            } else {
                crate::sheet::animation(source)?.context("no animation detected")?
            });
        }
        self.alternate_view = !self.alternate_view;
        self.generation += 1;
        Ok(())
    }
    /// Resolve an explicit MCP target without losing targets that precede disk refresh.
    pub fn follow_target(&mut self, frame_id: &str) -> Result<&'static str> {
        let previous_generation = self.generation;
        self.snapshot()?;
        let source = self.sprite.as_ref().context("no document")?;
        let desired = if review::frame_keys(&source.frames)
            .iter()
            .any(|id| id == frame_id)
        {
            Some(false)
        } else if self.alternate_sprite.as_ref().is_some_and(|s| {
            review::frame_keys(&s.frames)
                .iter()
                .any(|id| id == frame_id)
        }) {
            Some(true)
        } else {
            None
        };
        match desired {
            Some(alternate) if alternate != self.alternate_view => {
                self.alternate_view = alternate;
                self.generation += 1;
                Ok("changed")
            }
            Some(_) if previous_generation != self.generation => Ok("changed"),
            Some(_) => Ok("found"),
            None => Ok("missing"),
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        ensure!(!self.managed, "managed preview is read-only");
        self.desktop
            .as_ref()
            .context("no document")?
            .resize_canvas(width, height)?;
        self.refresh_document();
        Ok(())
    }
    pub fn save(&mut self, path: Option<&Path>) -> Result<()> {
        ensure!(!self.managed, "managed preview is read-only");
        let desktop = self.desktop.as_ref().context("no document")?;
        if let Some(path) = path {
            desktop.save_as(path)?;
            self.destination = Some(crate::output::resolved(path)?);
        } else {
            ensure!(self.destination.is_some(), "choose a destination first");
            desktop.save()?;
        }
        self.source = self.destination.clone();
        self.stamp = self
            .source
            .as_ref()
            .map(|p| Self::file_stamp(p))
            .transpose()?;
        self.saved_revision = self.revision;
        Ok(())
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        if let Some(path) = self.source.clone() {
            let stamp = Self::file_stamp(&path)?;
            if Some(stamp) != self.stamp {
                ensure!(
                    self.managed || self.saved_revision == self.revision,
                    "file changed on disk while there are unsaved edits; save a copy before reopening"
                );
                // Preserve managed activity/inspection across file refreshes.
                let events = (
                    self.activity.clone(),
                    self.inspection.clone(),
                    self.review.clone(),
                    self.event_serial,
                );
                let alternate = self.alternate_view;
                self.open(&path)?;
                if alternate {
                    let _ = self.toggle_view();
                }
                (
                    self.activity,
                    self.inspection,
                    self.review,
                    self.event_serial,
                ) = events;
            }
        }
        self.refresh_document();
        let document = self.presented().map(|s| DocumentInfo {
            generation: self.generation,
            revision: self.revision,
            width: s.width,
            height: s.height,
            frames: review::frame_keys(&s.frames)
                .into_iter()
                .zip(&s.frames)
                .map(|(id, f)| FrameInfo {
                    id,
                    duration_ms: f.duration_ms,
                })
                .collect(),
            layers: s
                .layers
                .iter()
                .map(|l| LayerInfo {
                    id: l.id.clone(),
                    name: l.name.clone(),
                    visible: l.visible,
                    opacity: l.opacity,
                })
                .collect(),
            path: self.source.clone(),
            dirty: self.saved_revision != self.revision,
            source_width: self.sprite.as_ref().unwrap().width,
            source_height: self.sprite.as_ref().unwrap().height,
            alternate_view: self.alternate_view,
            can_toggle_view: self.alternate_sprite.is_some()
                || self.sprite.as_ref().unwrap().frames.len() > 1,
        });
        Ok(Snapshot {
            document,
            managed: self.managed,
            activity: self.activity.clone(),
            inspection: self.inspection.clone(),
            review: self.review.clone(),
            event_serial: self.event_serial,
            review_stale: self.review_generation != Some(self.generation),
        })
    }
    /// Parse the existing managed stdio protocol. Activation and EOF belong to the host.
    pub fn message(&mut self, line: &str) -> Result<()> {
        self.snapshot()?;
        let value: serde_json::Value = serde_json::from_str(line)?;
        if let Some(review) = value.get("review") {
            self.review = Some(serde_json::from_value(review.clone())?);
            self.review_generation = Some(self.generation);
        } else if let Some(view) = value.get("inspection") {
            self.inspection = Some(serde_json::from_value(view.clone())?);
            self.review = None;
        } else {
            self.activity = Some(serde_json::from_value(value)?);
            self.inspection = None;
            self.review = None;
        }
        self.event_serial += 1;
        Ok(())
    }
    pub fn render(&self, request: RenderRequest) -> Result<Rendered> {
        ensure!(
            request.generation == self.generation,
            "document changed; request a fresh snapshot"
        );
        ensure!(
            !request.annotations
                || request
                    .event_serial
                    .is_none_or(|serial| serial == self.event_serial),
            "review changed; request a fresh snapshot"
        );
        let mut sprite = self.presented().cloned().context("no document")?;
        if let Some(id) = &request.layer_id {
            ensure!(
                sprite.layers.iter().any(|l| &l.id == id),
                "unknown layer_id"
            );
            sprite.layers.retain(|l| &l.id == id);
            sprite.layers[0].visible = true;
        } else {
            for layer in &mut sprite.layers {
                if request.override_visibility {
                    layer.visible = !request.hidden_layers.contains(&layer.id);
                } else if request.hidden_layers.contains(&layer.id) {
                    layer.visible = false;
                }
            }
        }
        if request.sheet {
            sprite = if sprite.frames.len() > 1 {
                crate::sheet::sheet(&sprite)?
            } else {
                crate::sheet::animation(&sprite)?.context("no animation detected in this image")?
            };
        }
        let keys = review::frame_keys(&sprite.frames);
        let frame = if request.sheet {
            0
        } else {
            keys.iter()
                .position(|id| id == &request.frame_id)
                .context("unknown frame_id")?
        };
        let reference = request
            .reference_frame_id
            .as_ref()
            .map(|id| {
                keys.iter()
                    .position(|f| f == id)
                    .context("unknown reference_frame_id")
            })
            .transpose()?;
        let (pixels, _) = inspection::render(&sprite, frame, reference, request.mode)?;
        let mut pixels = review::render::transform(&pixels, request.representation);
        if request.annotations
            && self.review_generation == Some(self.generation)
            && let Some(report) = self.review.as_ref().map(|r| &r.report)
            && report["kind"] == "sprite_analysis"
            && report["frame_id"].as_str() == Some(&request.frame_id)
            && report["layer_id"].as_str() == request.layer_id.as_deref()
            && let Some(components) = report["components"].as_array()
        {
            for component in components {
                let b = &component["bounds"];
                let (x, y, w, h) = (
                    b["x"].as_u64().unwrap_or(u64::MAX),
                    b["y"].as_u64().unwrap_or(u64::MAX),
                    b["width"].as_u64().unwrap_or(0),
                    b["height"].as_u64().unwrap_or(0),
                );
                if w == 0
                    || h == 0
                    || x >= u64::from(sprite.width)
                    || y >= u64::from(sprite.height)
                    || w > u64::from(sprite.width) - x
                    || h > u64::from(sprite.height) - y
                {
                    continue;
                }
                for yy in y..y + h {
                    for xx in x..x + w {
                        if yy == y || yy == y + h - 1 || xx == x || xx == x + w - 1 {
                            let i = (yy as usize * sprite.width as usize + xx as usize) * 4;
                            pixels[i..i + 4].copy_from_slice(&[255, 190, 60, 255]);
                        }
                    }
                }
            }
        }
        let (width, height) = (
            request.canvas_width.unwrap_or(sprite.width),
            request.canvas_height.unwrap_or(sprite.height),
        );
        if (width, height) != (sprite.width, sprite.height) {
            let mut resized = vec![0; crate::schema::pixel_bytes(width, height)?];
            let count = width.min(sprite.width) as usize * 4;
            for y in 0..height.min(sprite.height) as usize {
                resized[y * width as usize * 4..y * width as usize * 4 + count].copy_from_slice(
                    &pixels[y * sprite.width as usize * 4..y * sprite.width as usize * 4 + count],
                );
            }
            pixels = resized;
        }
        let mut png = Vec::new();
        crate::raster::png(&mut png, width, height, &pixels)?;
        Ok(Rendered {
            width,
            height,
            png: base64::engine::general_purpose::STANDARD.encode(png),
        })
    }
    pub fn local_review(
        &mut self,
        kind: &str,
        frame_id: &str,
        layer_id: Option<String>,
    ) -> Result<ViewerReview> {
        let sprite = self.presented().context("no document")?;
        let doc = review::document_for(sprite)?;
        let mut result = match kind {
            "analyze" => review::measure::sprite(
                &doc,
                &serde_json::from_value(
                    serde_json::json!({"frame_id":frame_id,"layer_id":layer_id}),
                )?,
            )?,
            "sheet" => review::render::sheet(
                &doc,
                &serde_json::from_value(
                    serde_json::json!({"frame_id":frame_id,"layer_id":layer_id}),
                )?,
            )?,
            "animation" => review::animation::audit(
                &doc,
                &serde_json::from_value(serde_json::json!({"layer_id":layer_id}))?,
            )?,
            _ => anyhow::bail!("unknown review kind"),
        };
        review::stamp(
            &mut result,
            "desktop",
            self.revision,
            review::identity(&doc)?,
        );
        let wire = ViewerReview::new(&result)?;
        self.review = Some(wire.clone());
        self.review_generation = Some(self.generation);
        self.event_serial += 1;
        Ok(wire)
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.cleanup_saved_recovery();
    }
}

/// Owns independent document sessions for one desktop window.
pub struct DocumentWorkspace {
    current: Application,
    current_id: Option<String>,
    parked: Vec<(String, Application)>,
    next_id: u64,
}
#[derive(Serialize)]
pub struct OpenDocument {
    pub id: String,
    pub path: Option<PathBuf>,
    pub dirty: bool,
}
#[derive(Serialize)]
pub struct WorkspaceSnapshot {
    #[serde(flatten)]
    pub active: Snapshot,
    pub active_document: Option<String>,
    pub documents: Vec<OpenDocument>,
}
impl DocumentWorkspace {
    pub fn new(application: Application) -> Self {
        let current_id = application
            .sprite
            .is_some()
            .then(|| "document-1".to_owned());
        let next_id = if current_id.is_some() { 2 } else { 1 };
        Self {
            current: application,
            current_id,
            parked: Vec::new(),
            next_id,
        }
    }
    pub fn active(&mut self) -> &mut Application {
        &mut self.current
    }
    fn install(&mut self, app: Application) {
        let previous = std::mem::replace(&mut self.current, app);
        if let Some(id) = self.current_id.take() {
            self.parked.push((id, previous));
        }
        self.current_id = Some(format!("document-{}", self.next_id));
        self.next_id += 1;
    }
    pub fn open(&mut self, path: &Path) -> Result<()> {
        ensure!(
            !self.current.managed,
            "managed preview cannot open another file"
        );
        let path = crate::output::resolved(path)?;
        if self.current.source.as_ref() == Some(&path) {
            return Ok(());
        }
        if let Some((id, _)) = self
            .parked
            .iter()
            .find(|(_, app)| app.source.as_ref() == Some(&path))
        {
            let id = id.clone();
            return self.activate(&id);
        }
        let mut app = Application::new(false);
        app.open(&path)?;
        self.install(app);
        Ok(())
    }
    pub fn create(&mut self, width: u32, height: u32) -> Result<()> {
        ensure!(!self.current.managed, "managed preview is read-only");
        let mut app = Application::new(false);
        app.create(width, height)?;
        self.install(app);
        Ok(())
    }
    pub fn activate(&mut self, id: &str) -> Result<()> {
        if self.current_id.as_deref() == Some(id) {
            return Ok(());
        }
        ensure!(
            !self.current.managed,
            "managed preview cannot switch documents"
        );
        let index = self
            .parked
            .iter()
            .position(|(key, _)| key == id)
            .context("document not found")?;
        let (next_id, next) = self.parked.remove(index);
        let previous = std::mem::replace(&mut self.current, next);
        if let Some(previous_id) = self.current_id.replace(next_id) {
            self.parked.insert(index, (previous_id, previous));
        }
        Ok(())
    }
    pub fn close(&mut self, id: &str, discard: bool) -> Result<()> {
        ensure!(
            !self.current.managed,
            "managed preview cannot close its document"
        );
        let app = if self.current_id.as_deref() == Some(id) {
            &mut self.current
        } else {
            &mut self
                .parked
                .iter_mut()
                .find(|(key, _)| key == id)
                .context("document not found")?
                .1
        };
        let dirty = app.snapshot()?.document.is_some_and(|doc| doc.dirty);
        ensure!(!dirty || discard, "document has unsaved changes");
        if self.current_id.as_deref() == Some(id) {
            let (next_id, next) = self
                .parked
                .pop()
                .map(|(id, app)| (Some(id), app))
                .unwrap_or_else(|| (None, Application::new(false)));
            self.current = next;
            self.current_id = next_id;
        } else {
            self.parked.retain(|(key, _)| key != id);
        }
        Ok(())
    }
    pub fn render(&mut self, id: Option<&str>, request: RenderRequest) -> Result<Rendered> {
        if let Some(id) = id {
            ensure!(
                self.current_id.as_deref() == Some(id),
                "stale document render request"
            );
        }
        self.current.render(request)
    }
    pub fn snapshot(&mut self) -> Result<WorkspaceSnapshot> {
        let active = self.current.snapshot()?;
        let mut documents = Vec::new();
        for (id, app) in &mut self.parked {
            // Inactive sessions retain their unsaved artwork and recovery storage.
            app.refresh_document();
            documents.push(OpenDocument {
                id: id.clone(),
                path: app.destination.clone().or_else(|| app.source.clone()),
                dirty: app.revision != app.saved_revision,
            });
        }
        if let (Some(id), Some(doc)) = (&self.current_id, &active.document) {
            documents.push(OpenDocument {
                id: id.clone(),
                path: doc.path.clone(),
                dirty: doc.dirty,
            });
        }
        documents.sort_by_key(|doc| {
            doc.id
                .strip_prefix("document-")
                .and_then(|id| id.parse::<u64>().ok())
                .unwrap_or(0)
        });
        Ok(WorkspaceSnapshot {
            active,
            active_document: self.current_id.clone(),
            documents,
        })
    }
}
