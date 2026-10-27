use crate::{output, schema};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const HISTORY_STEPS: usize = 32;
pub const HISTORY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProject {
    pub width: u32,
    pub height: u32,
    /// Colours in #RRGGBBAA order; indices are zero-based and stable.
    pub palette: Vec<String>,
    /// Prefer .pixloom; explicit .pixen.json is also supported. Parent must exist.
    pub output_path: PathBuf,
}

pub use crate::edit::{Drawing, Operation, Pixel};

struct Project {
    id: String,
    revision: u64,
    doc: Arc<schema::Document>,
    output: PathBuf,
    recovery: PathBuf,
    saved_bytes: Option<Arc<Vec<u8>>>,
    checkpoints: crate::review::checkpoints::CheckpointStore,
    used_ids: BTreeSet<String>,
    next_cel: u64,
    undo: VecDeque<Vec<u8>>,
    redo: VecDeque<Vec<u8>>,
}

#[derive(Default)]
pub struct Session {
    active: Option<Project>,
    next_id: u64,
    deferred_recovery: bool,
}

#[derive(Clone)]
pub struct DocumentRevision {
    pub project_id: String,
    pub revision: u64,
    pub document: Arc<schema::Document>,
    pub recovery: PathBuf,
}
pub struct SaveSnapshot {
    pub state: DocumentRevision,
    pub destination: PathBuf,
    expected: Option<Arc<Vec<u8>>>,
}
pub struct SavedSnapshot {
    pub project_id: String,
    pub revision: u64,
    pub destination: PathBuf,
    bytes: Arc<Vec<u8>>,
}
impl SaveSnapshot {
    /// Called without an editor/session lock. This writes only the captured revision.
    pub fn write(self) -> Result<SavedSnapshot> {
        schema::validate_document(&self.state.document)?;
        let check = || -> Result<()> {
            if let Some(bytes) = &self.expected {
                ensure!(
                    output::read_bounded(&self.destination, output::MAX_OUTPUT_BYTES)? == **bytes,
                    "project changed on disk; refusing to overwrite external edits"
                );
            } else {
                ensure!(
                    !self.destination.exists(),
                    "destination already exists; choose a new filename"
                );
            }
            Ok(())
        };
        check()?;
        let prepared = output::Prepared::new(&self.destination, |file| {
            if crate::input::is_container(&self.destination) {
                crate::container::write(
                    output::LimitedFile::new(file, output::MAX_OUTPUT_BYTES),
                    &self.state.document,
                )
            } else {
                output::write_document(file, &self.state.document)
            }
        })?;
        if crate::input::is_container(&self.destination) {
            crate::container::Reader::open(prepared.staged_path())?.load()?;
        }
        let bytes = Arc::new(output::read_bounded(
            prepared.staged_path(),
            output::MAX_OUTPUT_BYTES,
        )?);
        check()?;
        prepared.commit(self.expected.is_some())?;
        Ok(SavedSnapshot {
            project_id: self.state.project_id,
            revision: self.state.revision,
            destination: self.destination,
            bytes,
        })
    }
}

impl Session {
    /// Desktop editing owns persistence scheduling; headless sessions retain durable transactions.
    pub fn defer_recovery(&mut self) {
        self.deferred_recovery = true;
    }
    pub fn current_document(&self) -> Result<&schema::Document> {
        Ok(&self.active.as_ref().context("no active project")?.doc)
    }
    pub fn revision_snapshot(&self) -> Result<DocumentRevision> {
        let p = self.active.as_ref().context("no active project")?;
        Ok(DocumentRevision {
            project_id: p.id.clone(),
            revision: p.revision,
            document: p.doc.clone(),
            recovery: p.recovery.clone(),
        })
    }
    pub fn save_baseline(&self) -> Result<Option<Vec<u8>>> {
        Ok(self.active.as_ref().context("no active project")?.saved_bytes.as_ref().map(|bytes| (**bytes).clone()))
    }
    pub fn restore_save_baseline(&mut self, destination: Option<&Path>, expected: Option<Vec<u8>>) -> Result<()> {
        if let Some(destination) = destination {
            // These paths were resolved when originally opened/saved. A disconnected
            // drive must not prevent reopening the in-memory checkpoint.
            ensure!(destination.is_absolute(), "Checkpoint save destination must be absolute");
            let resolved = destination.to_owned();
            ensure!(project_destination(&resolved), "Invalid checkpoint save destination");
            let project = self.active.as_mut().context("no active project")?;
            project.output = resolved;
            project.saved_bytes = expected.map(Arc::new);
        }
        let project = self.active.as_mut().context("no active project")?;
        project.undo.clear();
        project.redo.clear();
        Ok(())
    }
    pub fn prepare_save(&self, destination: Option<&Path>) -> Result<SaveSnapshot> {
        let p = self.active.as_ref().context("no active project")?;
        let destination = destination
            .map(output::resolved)
            .transpose()?
            .unwrap_or_else(|| p.output.clone());
        ensure!(
            project_destination(&destination),
            "choose .pixloom or .pixen.json"
        );
        let same = destination == p.output;
        Ok(SaveSnapshot {
            state: self.revision_snapshot()?,
            destination,
            expected: if same { p.saved_bytes.clone() } else { None },
        })
    }
    pub fn finish_save(&mut self, saved: &SavedSnapshot) -> Result<()> {
        self.project(&saved.project_id)?;
        let p = self.active.as_mut().unwrap();
        p.output = saved.destination.clone();
        p.saved_bytes = Some(saved.bytes.clone());
        Ok(())
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active.as_ref().map(|p| p.id.as_str())
    }

    /// Independent baseline for reverting a whole conversational turn.
    pub fn document_snapshot(&self) -> Result<Vec<u8>> {
        snapshot(&self.active.as_ref().context("no active project")?.doc)
    }

    pub fn restore_document(&mut self, bytes: &[u8]) -> Result<()> {
        let next = schema::read_document(bytes)?;
        schema::validate_document(&next)?;
        let p = self.active.as_ref().context("no active project")?;
        let before = snapshot(&p.doc)?;
        if !self.deferred_recovery {
            output::Prepared::new(&p.recovery, |f| output::write_document(f, &next))?
                .commit(true)?;
        }
        let p = self.active.as_mut().unwrap();
        p.doc = Arc::new(next);
        p.revision += 1;
        p.undo.push_back(before);
        p.redo.clear();
        // used_ids remains a superset, preventing reuse after a revert.
        trim_history(p, false);
        Ok(())
    }

    /// Crop/pad all cels as one revision-checked, recoverable undo step.
    pub fn resize_canvas(
        &mut self,
        id: &str,
        expected: u64,
        width: u32,
        height: u32,
    ) -> Result<Value> {
        ensure!(
            self.project(id)?.revision == expected,
            "stale revision: current revision is {}",
            self.project(id)?.revision
        );
        if (
            self.project(id)?.doc.canvas.width,
            self.project(id)?.doc.canvas.height,
        ) == (width, height)
        {
            return self.info(id);
        }
        let mut doc = schema::read_document(&self.document_snapshot()?)?;
        schema::pixel_bytes(width, height)?;
        // Validate the target size and total cel budget before allocating grids.
        let mut target = doc.clone();
        target.canvas.width = width;
        target.canvas.height = height;
        schema::validate_structure(&target)?;
        schema::pixel_bytes(width, height)?
            .checked_mul(target.cels.len())
            .filter(|bytes| *bytes <= 256 * 1024 * 1024)
            .context("canvas exceeds cel memory budget")?;
        let grids = schema::decode_indices(&doc)?;
        let transparent = match doc
            .palette
            .iter()
            .position(|c| schema::decode_color(c).is_ok_and(|rgba| rgba[3] == 0))
        {
            Some(index) => index as u32,
            None => {
                doc.palette.push("#00000000".into());
                (doc.palette.len() - 1) as u32
            }
        };
        doc.cels = grids
            .into_iter()
            .map(|(id, old)| {
                let mut pixels = vec![vec![transparent; width as usize]; height as usize];
                for (dst, src) in pixels.iter_mut().zip(old) {
                    let count = dst.len().min(src.len());
                    dst[..count].copy_from_slice(&src[..count]);
                }
                if doc.version == 1 {
                    schema::CelDefinition::Legacy(schema::LegacyCel { id, pixels })
                } else {
                    schema::CelDefinition::Encoded(crate::encoding::Encoding::Grid(
                        crate::encoding::grid::Grid {
                            id,
                            pixels,
                            offset: None,
                            size: None,
                            default_index: None,
                        },
                    ))
                }
            })
            .collect();
        doc.canvas.width = width;
        doc.canvas.height = height;
        self.restore_document(&serde_json::to_vec(&doc)?)?;
        self.info(id)
    }

    pub fn save_as(&mut self, id: &str, destination: &Path) -> Result<Value> {
        ensure!(
            project_destination(destination),
            "choose .pixloom or .pixen.json"
        );
        let destination = output::resolved(destination)?;
        let p = self.project(id)?;
        if destination == p.output {
            return self.save(id);
        }
        ensure!(
            !destination.exists(),
            "destination already exists; choose a new filename"
        );
        let previous_output = p.output.clone();
        let p = self.active.as_mut().unwrap();
        let previous_bytes = p.saved_bytes.take();
        p.output = destination;
        match self.save(id) {
            Ok(saved) => Ok(saved),
            Err(error) => {
                let p = self.active.as_mut().unwrap();
                p.output = previous_output;
                p.saved_bytes = previous_bytes;
                Err(error)
            }
        }
    }

    pub fn create(&mut self, args: CreateProject) -> Result<Value> {
        ensure!(
            self.active.is_none(),
            "finish the active project before creating another"
        );
        schema::pixel_bytes(args.width, args.height)?;
        ensure!(!args.palette.is_empty(), "palette must not be empty");
        ensure!(
            project_destination(&args.output_path),
            "output_path must end in .pixloom or .pixen.json"
        );
        let output = output::resolved(&args.output_path)?;
        ensure!(!output.exists(), "output already exists");
        let mut palette = args.palette;
        for color in &palette {
            schema::decode_color(color)?;
        }
        let transparent = match palette
            .iter()
            .position(|c| schema::decode_color(c).is_ok_and(|p| p[3] == 0))
        {
            Some(i) => i as u32,
            None => {
                palette.push("#00000000".into());
                (palette.len() - 1) as u32
            }
        };
        let mut doc: schema::Document = serde_json::from_value(json!({
            "version":1, "canvas":{"width":args.width,"height":args.height}, "palette":palette,
            "cels":[{"id":"cel-1","pixels":vec![vec![transparent;args.width as usize];args.height as usize]}],
            "frames":[{"duration_ms":100.0}],
            "layers":[{"id":"layer-1","name":"Layer 1","timeline":[{"cel":"cel-1"}]}]
        }))?;
        doc.frames[0]
            .annotations
            .metadata
            .insert(crate::edit::FRAME_ID_KEY.into(), json!("frame-1"));
        schema::validate_document(&doc)?;
        let mut recovery = tempfile::Builder::new()
            .prefix(".pixloom-recovery-")
            .suffix(".pixen.json")
            .tempfile_in(crate::storage::recovery_dir(
                output.parent().context("output needs parent")?,
            )?)?;
        output::write_document(recovery.as_file_mut(), &doc)?;
        recovery.as_file().sync_all()?;
        let (_, recovery) = recovery.keep()?;
        self.next_id += 1;
        let id = format!("project-{}", self.next_id);
        eprintln!("pixen: recovery snapshot {}", recovery.display());
        self.active = Some(Project {
            id: id.clone(),
            revision: 0,
            doc: Arc::new(doc),
            output,
            recovery,
            saved_bytes: None,
            checkpoints: Default::default(),
            used_ids: BTreeSet::from(["frame:frame-1".into(), "layer:layer-1".into()]),
            next_cel: 1,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        });
        Ok(json!({"project_id":id,"revision":0,"frame_id":"frame-1","layer_id":"layer-1"}))
    }

    fn project(&self, id: &str) -> Result<&Project> {
        let p = self.active.as_ref().context("no active project")?;
        ensure!(p.id == id, "unknown project_id");
        Ok(p)
    }

    pub fn recovery_path(&self) -> Option<&Path> {
        self.active.as_ref().map(|p| p.recovery.as_path())
    }

    pub fn preview_source(&self, id: &str) -> Result<(PathBuf, Value)> {
        let p = self.project(id)?;
        Ok((
            p.recovery.clone(),
            json!({"project_id":p.id,"revision":p.revision,"width":p.doc.canvas.width,"height":p.doc.canvas.height,"frame_count":p.doc.frames.len()}),
        ))
    }

    pub fn inspect(
        &self,
        id: &str,
        frame_id: &str,
        crop: Option<crate::preview::Crop>,
        scale: u32,
    ) -> Result<(Vec<u8>, Value)> {
        let p = self.project(id)?;
        let frame = crate::edit::frame_index(&p.doc, frame_id)?;
        let sprite = schema::compile((*p.doc).clone())?;
        let (png, source) = crate::preview::png(&sprite, frame, crop, scale)?;
        Ok((
            png,
            json!({"project_id":p.id,"revision":p.revision,"frame_id":frame_id,"width":sprite.width,"height":sprite.height,"source":source,"scale":scale}),
        ))
    }

    pub fn contact_sheet(
        &self,
        id: &str,
        frame_ids: Option<Vec<String>>,
        scale: u32,
        columns: u32,
    ) -> Result<(Vec<u8>, Value)> {
        let p = self.project(id)?;
        let indices = match frame_ids {
            None => (0..p.doc.frames.len()).collect(),
            Some(ids) => {
                ensure!(
                    ids.iter().collect::<std::collections::HashSet<_>>().len() == ids.len(),
                    "frame_ids must be unique"
                );
                ids.iter()
                    .map(|id| crate::edit::frame_index(&p.doc, id))
                    .collect::<Result<Vec<_>>>()?
            }
        };
        let sprite = schema::compile((*p.doc).clone())?;
        let (png, mut metadata) = crate::preview::contact_sheet(&sprite, &indices, scale, columns)?;
        metadata["project_id"] = p.id.clone().into();
        metadata["revision"] = p.revision.into();
        Ok((png, metadata))
    }

    pub fn apply(&mut self, id: &str, expected: u64, operations: Vec<Operation>) -> Result<Value> {
        let p = self.project(id)?;
        ensure!(
            p.revision == expected,
            "stale revision: current revision is {}",
            p.revision
        );
        ensure!(
            !operations.is_empty() && operations.len() <= 10000,
            "batch must contain 1..10000 operations"
        );
        let mut next = (*p.doc).clone();
        let before = snapshot(&p.doc)?;
        let mut used = p.used_ids.clone();
        let mut next_cel = p.next_cel;
        let structural = operations
            .iter()
            .any(|op| !matches!(op, Operation::Draw(_)));
        for op in operations {
            crate::edit::execute(&mut next, op, &mut used, &mut next_cel)?;
        }
        schema::validate_document(&next)?;
        let _ = snapshot(&next)?;
        let (bounds, changes) = changed(&p.doc, &next, structural)?;
        if !self.deferred_recovery {
            output::Prepared::new(&p.recovery, |f| output::write_document(f, &next))?
                .commit(true)?;
        }
        let p = self.active.as_mut().unwrap();
        p.doc = Arc::new(next);
        p.used_ids = used;
        p.next_cel = next_cel;
        p.redo.clear();
        p.undo.push_back(before);
        trim_history(p, false);
        p.revision += 1;
        Ok(
            json!({"revision":p.revision,"changed_bounds":bounds,"changes":changes,"undo_steps":p.undo.len(),"redo_steps":p.redo.len()}),
        )
    }

    pub fn info(&self, id: &str) -> Result<Value> {
        let p = self.project(id)?;
        Ok(
            json!({"project_id":p.id,"revision":p.revision,"width":p.doc.canvas.width,"height":p.doc.canvas.height,
            "output_path":p.output,"palette":p.doc.palette,"palette_semantics":"zero-based indices; exact #RRGGBBAA values, including duplicate colours; uncomposited cel pixels",
            "layers":p.doc.layers.iter().enumerate().map(|(index,l)|json!({"id":l.id,"index":index,"name":l.name,"visible":l.visible,"opacity":l.opacity,"locked":l.locked,"timeline":l.timeline})).collect::<Vec<_>>(),
            "frames":p.doc.frames.iter().enumerate().map(|(index,f)|json!({"id":crate::edit::frame_id(f),"index":index,"duration_ms":f.duration_ms})).collect::<Vec<_>>(),
            "undo_steps":p.undo.len(),"redo_steps":p.redo.len(),"history_max_steps":HISTORY_STEPS,"history_max_bytes":HISTORY_BYTES}),
        )
    }
    pub fn pixels(
        &self,
        id: &str,
        frame_id: &str,
        layer_id: &str,
        region: crate::preview::Crop,
    ) -> Result<Value> {
        let p = self.project(id)?;
        let frame = crate::edit::frame_index(&p.doc, frame_id)?;
        let layer = p
            .doc
            .layers
            .iter()
            .position(|l| l.id == layer_id)
            .context("unknown layer_id")?;
        ensure!(
            region.x >= 0
                && region.y >= 0
                && region.width > 0
                && region.height > 0
                && region.x < i64::from(p.doc.canvas.width)
                && region.y < i64::from(p.doc.canvas.height)
                && region.x as u64 + u64::from(region.width) <= u64::from(p.doc.canvas.width)
                && region.y as u64 + u64::from(region.height) <= u64::from(p.doc.canvas.height),
            "region must lie wholly within the canvas with positive dimensions"
        );
        ensure!(
            u64::from(region.width) * u64::from(region.height) <= 4096,
            "pixel inspection is limited to 4096 pixels per call"
        );
        let grid = crate::edit::grid(&p.doc, layer, frame)?;
        let mut pixels = Vec::new();
        for y in region.y..region.y + i64::from(region.height) {
            for x in region.x..region.x + i64::from(region.width) {
                let index = grid[y as usize][x as usize];
                pixels.push(
                    json!({"x":x,"y":y,"palette_index":index,"rgba":p.doc.palette[index as usize]}),
                );
            }
        }
        Ok(
            json!({"project_id":p.id,"revision":p.revision,"frame_id":frame_id,"layer_id":layer_id,"source":region,"semantics":"exact uncomposited cel palette indices and #RRGGBBAA; layer visibility/opacity and cel opacity are not applied","pixels":pixels}),
        )
    }
    pub fn undo(&mut self, id: &str, expected: u64) -> Result<Value> {
        self.restore(id, expected, false)
    }
    pub fn redo(&mut self, id: &str, expected: u64) -> Result<Value> {
        self.restore(id, expected, true)
    }
    fn restore(&mut self, id: &str, expected: u64, redo: bool) -> Result<Value> {
        let p = self.project(id)?;
        ensure!(
            p.revision == expected,
            "stale revision: current revision is {}",
            p.revision
        );
        let stack = if redo { &p.redo } else { &p.undo };
        let bytes = stack.back().context(if redo {
            "nothing to redo"
        } else {
            "nothing to undo"
        })?;
        let next = schema::read_document(bytes)?;
        let current = snapshot(&p.doc)?;
        if !self.deferred_recovery {
            output::Prepared::new(&p.recovery, |f| output::write_document(f, &next))?
                .commit(true)?;
        }
        let p = self.active.as_mut().unwrap();
        if redo {
            p.redo.pop_back();
            p.undo.push_back(current);
        } else {
            p.undo.pop_back();
            p.redo.push_back(current);
        }
        p.doc = Arc::new(next);
        p.revision += 1;
        trim_history(p, !redo);
        Ok(
            json!({"revision":p.revision,"changed_bounds":{"x":0,"y":0,"width":p.doc.canvas.width,"height":p.doc.canvas.height},"undo_steps":p.undo.len(),"redo_steps":p.redo.len()}),
        )
    }

    pub fn preview(&self, id: &str, scale: u32) -> Result<(Vec<u8>, Value)> {
        let p = self.project(id)?;
        let first = crate::edit::frame_id(&p.doc.frames[0])
            .context("missing frame ID")?
            .to_owned();
        let (png, info) = self.inspect(id, &first, None, scale)?;
        Ok((
            png,
            json!({"revision":info["revision"],"width":info["width"],"height":info["height"]}),
        ))
    }

    /// Open any input supported by the inspector, preserving authoring indices where available.
    pub fn open(&mut self, path: &Path, output_path: Option<&Path>) -> Result<Value> {
        ensure!(
            self.active.is_none(),
            "finish the active project before opening another"
        );
        let source = path.canonicalize().context("open project")?;
        let mut doc = match source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "json" | "pixloom" | "spritex" => crate::input::read(&source)?,
            _ => schema::to_document(&crate::preview::load(&source)?)?,
        };
        crate::edit::transparent(&mut doc);
        let mut ids = BTreeSet::new();
        for frame in &doc.frames {
            if let Some(id) = crate::edit::frame_id(frame) {
                schema::identifier(id)?;
                ensure!(ids.insert(id.to_owned()), "duplicate persisted frame ID");
            }
        }
        for i in 0..doc.frames.len() {
            if crate::edit::frame_id(&doc.frames[i]).is_none() {
                let mut n = i + 1;
                while ids.contains(&format!("frame-{n}")) {
                    n += 1;
                }
                let id = format!("frame-{n}");
                ids.insert(id.clone());
                doc.frames[i]
                    .annotations
                    .metadata
                    .insert(crate::edit::FRAME_ID_KEY.into(), json!(id));
            }
        }
        schema::validate_document(&doc)?;
        snapshot(&doc)?;
        let default_output = if source
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pixloom"))
        {
            source.clone()
        } else {
            let stem = source
                .file_stem()
                .context("missing filename")?
                .to_string_lossy();
            source.with_file_name(format!(
                "{}.pixloom",
                stem.strip_suffix(".pixloom")
                    .or_else(|| stem.strip_suffix(".spritex"))
                    .unwrap_or(&stem)
            ))
        };
        let output = output::resolved(output_path.unwrap_or(&default_output))?;
        ensure!(
            project_destination(&output),
            "project output must end in .pixloom or .pixen.json; use export_project for other formats"
        );
        let same_source = output == source;
        ensure!(
            same_source || !output.exists(),
            "output already exists; choose another output_path"
        );
        let saved_bytes = if same_source {
            Some(Arc::new(output::read_bounded(
                &output,
                output::MAX_OUTPUT_BYTES,
            )?))
        } else {
            None
        };
        let mut recovery = tempfile::Builder::new()
            .prefix(".pixloom-recovery-")
            .suffix(".pixen.json")
            .tempfile_in(crate::storage::recovery_dir(
                output.parent().context("output needs parent")?,
            )?)?;
        output::write_document(recovery.as_file_mut(), &doc)?;
        recovery.as_file().sync_all()?;
        let (_, recovery) = recovery.keep()?;
        self.next_id += 1;
        let id = format!("project-{}", self.next_id);
        let used_ids = doc
            .layers
            .iter()
            .map(|l| format!("layer:{}", l.id))
            .chain(ids.into_iter().map(|id| format!("frame:{id}")))
            .collect();
        self.active = Some(Project {
            id: id.clone(),
            revision: 0,
            doc: Arc::new(doc),
            output,
            recovery,
            saved_bytes,
            checkpoints: Default::default(),
            used_ids,
            next_cel: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
        });
        self.info(&id)
    }

    pub fn save(&mut self, id: &str) -> Result<Value> {
        let p = self.project(id)?;
        schema::validate_document(&p.doc)?;
        if let Some(previous) = &p.saved_bytes {
            ensure!(
                &output::read_bounded(&p.output, output::MAX_OUTPUT_BYTES)? == previous.as_ref(),
                "project changed on disk; refusing to overwrite external edits"
            );
        } else {
            ensure!(!p.output.exists(), "output already exists");
        }
        let prepared = output::Prepared::new(&p.output, |f| {
            if crate::input::is_container(&p.output) {
                crate::container::write(
                    output::LimitedFile::new(f, output::MAX_OUTPUT_BYTES),
                    &p.doc,
                )
            } else {
                output::write_document(f, &p.doc)
            }
        })?;
        if crate::input::is_container(&p.output) {
            crate::container::Reader::open(prepared.staged_path())?.load()?;
        }
        let bytes = output::read_bounded(prepared.staged_path(), output::MAX_OUTPUT_BYTES)?;
        prepared.commit(p.saved_bytes.is_some())?;
        let result = json!({"saved_path":p.output,"revision":p.revision});
        self.active.as_mut().unwrap().saved_bytes = Some(Arc::new(bytes));
        Ok(result)
    }

    pub fn export(
        &self,
        id: &str,
        path: &Path,
        overwrite: bool,
        frame_id: Option<&str>,
        loop_animation: bool,
    ) -> Result<Value> {
        let p = self.project(id)?;
        let destination = output::resolved(path)?;
        ensure!(
            destination != p.output && destination != p.recovery,
            "export must not replace the active project or recovery"
        );
        let sprite = schema::compile((*p.doc).clone())?;
        let extension = destination
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let prepared = output::Prepared::new(&destination, |f| {
            if project_destination(&destination) {
                ensure!(
                    frame_id.is_none() && !loop_animation,
                    "project export preserves the full timeline"
                );
                if crate::input::is_container(&destination) {
                    crate::container::write(
                        output::LimitedFile::new(f, output::MAX_OUTPUT_BYTES),
                        &p.doc,
                    )
                } else {
                    output::write_document(f, &p.doc)
                }
            } else if matches!(extension.as_str(), "ase" | "aseprite" | "pxo") {
                ensure!(
                    frame_id.is_none() && !loop_animation,
                    "native export preserves the full timeline"
                );
                let format = if extension == "pxo" {
                    crate::formats::ProjectFormat::Pxo
                } else {
                    crate::formats::ProjectFormat::Aseprite
                };
                crate::native::write(
                    output::LimitedFile::new(f, output::MAX_OUTPUT_BYTES),
                    &sprite,
                    format,
                )
            } else {
                let formats = crate::formats::parse_export(&extension)?;
                let crate::formats::OutputFormat::Image(format) = formats[0] else {
                    anyhow::bail!("use the CLI for video export");
                };
                let frame = frame_id
                    .map(|id| crate::edit::frame_index(&p.doc, id).map(|i| i + 1))
                    .transpose()?;
                let options = crate::export::ExportOptions {
                    frame,
                    loop_animation,
                    background: None,
                };
                crate::export::validate(&sprite, format, &options)?;
                crate::export::write(
                    output::LimitedFile::new(f, output::MAX_OUTPUT_BYTES),
                    &sprite,
                    format,
                    &options,
                )
            }
        })?;
        if crate::input::is_container(&destination) {
            crate::container::Reader::open(prepared.staged_path())?.load()?;
        }
        prepared.commit(overwrite)?;
        Ok(json!({"exported_path":destination,"revision":p.revision}))
    }

    pub fn analysis(
        &self,
        id: &str,
        frame_id: &str,
        reference_id: Option<&str>,
        mode: crate::inspection::Mode,
        scale: u32,
        layer_id: Option<&str>,
    ) -> Result<(Vec<u8>, Value)> {
        let p = self.project(id)?;
        let frame = crate::edit::frame_index(&p.doc, frame_id)?;
        let reference = reference_id
            .map(|id| crate::edit::frame_index(&p.doc, id))
            .transpose()?;
        let mut sprite = schema::compile((*p.doc).clone())?;
        if let Some(id) = layer_id {
            ensure!(sprite.layers.iter().any(|l| l.id == id), "unknown layer_id");
            sprite.layers.retain(|l| l.id == id);
            sprite.layers[0].visible = true;
        }
        let (rgba, changed) = crate::inspection::render(&sprite, frame, reference, mode)?;
        let png = crate::inspection::png(sprite.width, sprite.height, &rgba, scale)?;
        Ok((
            png,
            json!({"project_id":id,"revision":p.revision,"frame_id":frame_id,"reference_frame_id":reference_id,"mode":mode,"layer_id":layer_id,"width":sprite.width,"height":sprite.height,"scale":scale,"changed_pixels":changed}),
        ))
    }

    pub fn manage_checkpoint(
        &mut self,
        id: &str,
        action: crate::review::checkpoints::CheckpointAction,
    ) -> Result<Value> {
        use crate::review::checkpoints::CheckpointAction;
        let p = self.project(id)?;
        let expected = match &action {
            CheckpointAction::Create {
                expected_revision, ..
            }
            | CheckpointAction::Delete {
                expected_revision, ..
            } => Some(*expected_revision),
            CheckpointAction::List => None,
        };
        if let Some(expected) = expected {
            ensure!(
                p.revision == expected,
                "stale revision: current revision is {}",
                p.revision
            );
        }
        let p = self.active.as_mut().unwrap();
        let mut result = match action {
            CheckpointAction::Create { label, .. } => {
                p.checkpoints.create(&p.doc, p.revision, &label)?
            }
            CheckpointAction::Delete { checkpoint_id, .. } => {
                p.checkpoints.delete(&checkpoint_id)?
            }
            CheckpointAction::List => p.checkpoints.list(),
        };
        result["project_id"] = json!(id);
        result["inspected_revision"] = json!(p.revision);
        result["report_version"] = json!(1);
        Ok(result)
    }
    pub fn compare_checkpoint(
        &self,
        id: &str,
        request: &crate::review::compare::CompareCheckpointRequest,
    ) -> Result<crate::review::ReviewResult> {
        let p = self.project(id)?;
        let (before, revision) = p.checkpoints.document(&request.checkpoint_id)?;
        let mut result = crate::review::compare::documents(&before, &p.doc, request)?;
        result.report["baseline_revision"] = json!(revision);
        crate::review::stamp(
            &mut result,
            id,
            p.revision,
            crate::review::identity(&p.doc)?,
        );
        Ok(result)
    }
    pub fn validate_artwork(
        &self,
        id: &str,
        request: &crate::review::rules::ValidateArtworkRequest,
    ) -> Result<crate::review::ReviewResult> {
        let p = self.project(id)?;
        let mut result = crate::review::rules::validate(&p.doc, &p.checkpoints, request)?;
        crate::review::stamp(
            &mut result,
            id,
            p.revision,
            crate::review::identity(&p.doc)?,
        );
        Ok(result)
    }
    pub fn review_sheet(
        &self,
        id: &str,
        request: &crate::review::render::ReviewSheetRequest,
    ) -> Result<crate::review::ReviewResult> {
        let p = self.project(id)?;
        let mut result = crate::review::render::sheet(&p.doc, request)?;
        crate::review::stamp(
            &mut result,
            id,
            p.revision,
            crate::review::identity(&p.doc)?,
        );
        Ok(result)
    }
    pub fn analyze_sprite(
        &self,
        id: &str,
        request: &crate::review::measure::AnalyzeSpriteRequest,
    ) -> Result<crate::review::ReviewResult> {
        let p = self.project(id)?;
        let mut result = crate::review::measure::sprite(&p.doc, request)?;
        crate::review::stamp(
            &mut result,
            id,
            p.revision,
            crate::review::identity(&p.doc)?,
        );
        Ok(result)
    }
    pub fn audit_animation(
        &self,
        id: &str,
        request: &crate::review::animation::AuditAnimationRequest,
    ) -> Result<crate::review::ReviewResult> {
        let p = self.project(id)?;
        let mut result = crate::review::animation::audit(&p.doc, request)?;
        crate::review::stamp(
            &mut result,
            id,
            p.revision,
            crate::review::identity(&p.doc)?,
        );
        Ok(result)
    }
    pub fn finish(&mut self, id: &str) -> Result<Value> {
        let result = self.save(id)?;
        let p = self.project(id)?;
        if let Err(e) = std::fs::remove_file(&p.recovery) {
            eprintln!("pixen: could not remove recovery snapshot: {e}");
        }
        self.active = None;
        Ok(result)
    }
}

fn snapshot(doc: &schema::Document) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(doc)?;
    ensure!(
        bytes.len() <= HISTORY_BYTES,
        "document exceeds the 64 MiB transaction/history budget"
    );
    Ok(bytes)
}
fn trim_history(p: &mut Project, keep_redo: bool) {
    while p.undo.len() + p.redo.len() > HISTORY_STEPS
        || p.undo.iter().chain(&p.redo).map(Vec::len).sum::<usize>() > HISTORY_BYTES
    {
        let (first, second) = if keep_redo {
            (&mut p.undo, &mut p.redo)
        } else {
            (&mut p.redo, &mut p.undo)
        };
        if first.pop_front().is_none() {
            second.pop_front();
        }
    }
}
fn changed(
    before: &schema::Document,
    after: &schema::Document,
    structural: bool,
) -> Result<(Value, Value)> {
    if structural {
        return Ok((
            json!({"x":0,"y":0,"width":after.canvas.width,"height":after.canvas.height}),
            json!([]),
        ));
    }
    let mut all: Option<(usize, usize, usize, usize)> = None;
    let mut changes = Vec::new();
    for (l, layer) in after.layers.iter().enumerate() {
        for (f, frame) in after.frames.iter().enumerate() {
            let a = crate::edit::grid(before, l, f)?;
            let b = crate::edit::grid(after, l, f)?;
            let mut local: Option<(usize, usize, usize, usize)> = None;
            for y in 0..a.len() {
                for x in 0..a[y].len() {
                    if a[y][x] != b[y][x] {
                        local = Some(match local {
                            None => (x, y, x, y),
                            Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                        });
                    }
                }
            }
            if let Some((x, y, r, b)) = local {
                changes.push(json!({"frame_id":crate::edit::frame_id(frame),"layer_id":layer.id,"bounds":{"x":x,"y":y,"width":r-x+1,"height":b-y+1}}));
                all = Some(match all {
                    None => (x, y, r, b),
                    Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(r), e.max(b)),
                });
            }
        }
    }
    Ok((
        all.map(|(x, y, r, b)| json!({"x":x,"y":y,"width":r-x+1,"height":b-y+1}))
            .unwrap_or(Value::Null),
        json!(changes),
    ))
}

fn project_destination(path: &Path) -> bool {
    let name = path.to_string_lossy().to_ascii_lowercase();
    name.ends_with(".pixloom") || name.ends_with(".pixen.json")
}
