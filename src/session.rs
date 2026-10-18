use crate::{output, schema};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

pub const HISTORY_STEPS: usize = 32;
pub const HISTORY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProject {
    pub width: u32,
    pub height: u32,
    /// Colours in #RRGGBBAA order; indices are zero-based and stable.
    pub palette: Vec<String>,
    /// Destination must end in .pixloom.json and its parent must exist.
    pub output_path: PathBuf,
}

pub use crate::edit::{Drawing, Operation, Pixel};

struct Project {
    id: String,
    revision: u64,
    doc: schema::Document,
    output: PathBuf,
    recovery: PathBuf,
    used_ids: BTreeSet<String>,
    next_cel: u64,
    undo: VecDeque<Vec<u8>>,
    redo: VecDeque<Vec<u8>>,
}

#[derive(Default)]
pub struct Session {
    active: Option<Project>,
    next_id: u64,
}

impl Session {
    pub fn create(&mut self, args: CreateProject) -> Result<Value> {
        ensure!(
            self.active.is_none(),
            "finish the active project before creating another"
        );
        schema::pixel_bytes(args.width, args.height)?;
        ensure!(!args.palette.is_empty(), "palette must not be empty");
        ensure!(
            args.output_path
                .to_string_lossy()
                .ends_with(".pixloom.json"),
            "output_path must end in .pixloom.json"
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
            .suffix(".pixloom.json")
            .tempfile_in(output.parent().context("output needs parent")?)?;
        output::write_document(recovery.as_file_mut(), &doc)?;
        recovery.as_file().sync_all()?;
        let (_, recovery) = recovery.keep()?;
        self.next_id += 1;
        let id = format!("project-{}", self.next_id);
        eprintln!("pixloom: recovery snapshot {}", recovery.display());
        self.active = Some(Project {
            id: id.clone(),
            revision: 0,
            doc,
            output,
            recovery,
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
        let sprite = schema::compile(p.doc.clone())?;
        let (png, source) = crate::preview::png(&sprite, frame, crop, scale)?;
        Ok((
            png,
            json!({"project_id":p.id,"revision":p.revision,"frame_id":frame_id,"width":sprite.width,"height":sprite.height,"source":source,"scale":scale}),
        ))
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
        let mut next = p.doc.clone();
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
        output::Prepared::new(&p.recovery, |f| output::write_document(f, &next))?.commit(true)?;
        let p = self.active.as_mut().unwrap();
        p.doc = next;
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
            "palette":p.doc.palette,"palette_semantics":"zero-based indices; exact #RRGGBBAA values, including duplicate colours; uncomposited cel pixels",
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
        output::Prepared::new(&p.recovery, |f| output::write_document(f, &next))?.commit(true)?;
        let p = self.active.as_mut().unwrap();
        if redo {
            p.redo.pop_back();
            p.undo.push_back(current);
        } else {
            p.undo.pop_back();
            p.redo.push_back(current);
        }
        p.doc = next;
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

    pub fn finish(&mut self, id: &str) -> Result<Value> {
        let p = self.project(id)?;
        schema::validate_document(&p.doc)?;
        output::Prepared::new(&p.output, |f| output::write_document(f, &p.doc))?.commit(false)?;
        let result = json!({"saved_path":p.output,"revision":p.revision});
        if let Err(e) = std::fs::remove_file(&p.recovery) {
            eprintln!("pixloom: could not remove recovery snapshot: {e}");
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
