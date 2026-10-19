use super::*;
use image::RgbaImage;
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareCheckpointRequest {
    pub checkpoint_id: String,
    pub frame_ids: Option<Vec<String>>,
    pub layer_ids: Option<Vec<String>>,
    pub crop: Option<Crop>,
    #[serde(default = "yes")]
    pub include_images: bool,
    #[serde(default = "render::eight")]
    pub scale: u32,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct PixelDiff {
    pub changed_pixels: u64,
    pub added: u64,
    pub removed: u64,
    pub repainted: u64,
    pub alpha_only: u64,
    pub bounds: Option<Crop>,
    pub examples: Vec<Value>,
    pub examples_truncated: bool,
}
pub fn pixels(a: &[u8], b: &[u8], width: u32) -> PixelDiff {
    let mut diff = PixelDiff::default();
    let mut bound: Option<(u32, u32, u32, u32)> = None;
    for (i, (a, b)) in a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .enumerate()
    {
        if a == b || a[3] == 0 && b[3] == 0 {
            continue;
        }
        diff.changed_pixels += 1;
        if a[3] == 0 {
            diff.added += 1;
        } else if b[3] == 0 {
            diff.removed += 1;
        } else if a[..3] == b[..3] {
            diff.alpha_only += 1;
        } else {
            diff.repainted += 1;
        }
        let x = i as u32 % width;
        let y = i as u32 / width;
        if diff.examples.len() < MAX_RECORDS {
            diff.examples.push(json!({"x":x,"y":y}));
        }
        bound = Some(match bound {
            None => (x, y, x, y),
            Some((l, t, r, bot)) => (l.min(x), t.min(y), r.max(x), bot.max(y)),
        });
    }
    diff.bounds = bound.map(|(x, y, r, b)| Crop {
        x: i64::from(x),
        y: i64::from(y),
        width: r - x + 1,
        height: b - y + 1,
    });
    diff.examples_truncated = diff.changed_pixels > MAX_RECORDS as u64;
    diff
}
pub fn difference_image(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = b.to_vec();
    for ((a, b), o) in a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .zip(out.as_chunks_mut::<4>().0)
    {
        if a != b && (a[3] != 0 || b[3] != 0) {
            *o = [255, 64, 180, 255];
        } else {
            o[3] = ((u16::from(o[3]) * 60) / 255) as u8;
        }
    }
    out
}
fn offset(diff: &mut PixelDiff, r: Crop) {
    if let Some(b) = &mut diff.bounds {
        b.x += r.x;
        b.y += r.y;
    }
    for e in &mut diff.examples {
        e["x"] = json!(e["x"].as_i64().unwrap() + r.x);
        e["y"] = json!(e["y"].as_i64().unwrap() + r.y);
    }
}
pub fn selected_union(
    requested: Option<&[String]>,
    a: Vec<String>,
    b: Vec<String>,
    limit: usize,
) -> Result<Vec<String>> {
    let mut all = a;
    for id in b {
        if !all.contains(&id) {
            all.push(id);
        }
    }
    let selected = indices(requested, &all, limit)?;
    Ok(selected.into_iter().map(|i| all[i].clone()).collect())
}
fn union_render(
    sprite: &crate::model::Sprite,
    fi: Option<usize>,
    layers: &[String],
    r: Crop,
) -> Result<Vec<u8>> {
    let mut out = vec![0; r.width as usize * r.height as usize * 4];
    let Some(fi) = fi else {
        return Ok(out);
    };
    let mut s = sprite.clone();
    s.layers.retain(|l| layers.contains(&l.id));
    let rgba = crate::raster::render(&s, fi)?;
    for y in 0..r.height {
        for x in 0..r.width {
            let sx = r.x as u32 + x;
            let sy = r.y as u32 + y;
            if sx < s.width && sy < s.height {
                let src = (sy as usize * s.width as usize + sx as usize) * 4;
                let dst = (y as usize * r.width as usize + x as usize) * 4;
                out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
            }
        }
    }
    Ok(out)
}
pub fn documents(
    a: &Document,
    b: &Document,
    req: &CompareCheckpointRequest,
) -> Result<ReviewResult> {
    ensure!(req.scale > 0, "scale must be positive");
    let af = frame_ids(a);
    let bf = frame_ids(b);
    let fs = selected_union(req.frame_ids.as_deref(), af.clone(), bf.clone(), 64)?;
    let al = a.layers.iter().map(|l| l.id.clone()).collect::<Vec<_>>();
    let bl = b.layers.iter().map(|l| l.id.clone()).collect::<Vec<_>>();
    let ls = selected_union(
        req.layer_ids.as_deref(),
        al.clone(),
        bl.clone(),
        crate::schema::MAX_LAYERS,
    )?;
    let r = crop(
        a.canvas.width.max(b.canvas.width),
        a.canvas.height.max(b.canvas.height),
        req.crop,
    )?;
    ensure!(
        u64::from(r.width) * u64::from(r.height) * fs.len() as u64 * (ls.len() as u64 + 1)
            <= MAX_WORK,
        "checkpoint comparison exceeds 64 million union pixel visits; select fewer frames/layers"
    );
    for d in [a, b] {
        ensure!(
            d.canvas.width as u64
                * d.canvas.height as u64
                * fs.len() as u64
                * (ls.len() as u64 + 1)
                <= MAX_WORK,
            "checkpoint comparison exceeds 64 million source pixels; select fewer frames/layers"
        );
    }
    if req.include_images {
        let w = u64::from(r.width) * req.scale as u64;
        let h = u64::from(r.height) * req.scale as u64;
        ensure!(
            (w + 160)
                .checked_mul(h + 32)
                .and_then(|n| n.checked_mul(fs.len() as u64 * 3))
                .is_some_and(|n| n <= MAX_IMAGE_PIXELS),
            "comparison images exceed 16 million pixels; reduce scale/select frames or set include_images:false"
        );
    }
    let colors_a = a
        .palette
        .iter()
        .map(|c| crate::schema::decode_color(c))
        .collect::<Result<Vec<_>>>()?;
    let colors_b = b
        .palette
        .iter()
        .map(|c| crate::schema::decode_color(c))
        .collect::<Result<Vec<_>>>()?;
    let sa = crate::schema::compile(a.clone())?;
    let sb = crate::schema::compile(b.clone())?;
    let mut frames = Vec::new();
    let mut cells = Vec::new();
    let mut panels = Vec::new();
    for (ordinal, fid) in fs.iter().enumerate() {
        let ai = af.iter().position(|id| id == fid);
        let bi = bf.iter().position(|id| id == fid);
        let ra = union_render(&sa, ai, &ls, r)?;
        let rb = union_render(&sb, bi, &ls, r)?;
        let mut diff = pixels(&ra, &rb, r.width);
        offset(&mut diff, r);
        frames.push(json!({"frame_id":fid,"before_present":ai.is_some(),"current_present":bi.is_some(),"diff":diff}));
        for lid in &ls {
            let li_a = al.iter().position(|id| id == lid);
            let li_b = bl.iter().position(|id| id == lid);
            let ga = if let (Some(l), Some(f)) = (li_a, ai) {
                Some(crate::edit::grid(a, l, f)?)
            } else {
                None
            };
            let gb = if let (Some(l), Some(f)) = (li_b, bi) {
                Some(crate::edit::grid(b, l, f)?)
            } else {
                None
            };
            let (mut index_changes, mut resolved_changes) = (0u64, 0u64);
            let mut cell_bounds: Option<(usize, usize, usize, usize)> = None;
            for y in r.y as usize..r.y as usize + r.height as usize {
                for x in r.x as usize..r.x as usize + r.width as usize {
                    let pa = ga
                        .as_ref()
                        .and_then(|g| g.get(y))
                        .and_then(|row| row.get(x))
                        .copied();
                    let pb = gb
                        .as_ref()
                        .and_then(|g| g.get(y))
                        .and_then(|row| row.get(x))
                        .copied();
                    if pa != pb {
                        index_changes += 1;
                    }
                    let ca = pa.map(|i| colors_a[i as usize]);
                    let cb = pb.map(|i| colors_b[i as usize]);
                    if ca != cb {
                        resolved_changes += 1;
                    }
                    if pa != pb || ca != cb {
                        cell_bounds = Some(match cell_bounds {
                            None => (x, y, x, y),
                            Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x), b.max(y)),
                        });
                    }
                }
            }
            let bounds =
                cell_bounds.map(|(x, y, r, b)| json!({"x":x,"y":y,"width":r-x+1,"height":b-y+1}));
            cells.push(json!({"frame_id":fid,"layer_id":lid,"index_changes":index_changes,"resolved_changes":resolved_changes,"changed_bounds":bounds}));
        }
        if req.include_images {
            let changes = difference_image(&ra, &rb);
            for (role, pixels) in [("before", ra), ("current", rb), ("difference", changes)] {
                let img = RgbaImage::from_raw(r.width, r.height, pixels).unwrap();
                let img = image::imageops::resize(
                    &img,
                    r.width * req.scale,
                    r.height * req.scale,
                    image::imageops::FilterType::Nearest,
                );
                panels.push((format!("Frame {} {role}", ordinal + 1), img));
            }
        }
    }
    let mut structure = Vec::new();
    let av = serde_json::to_value(a)?;
    let bv = serde_json::to_value(b)?;
    for field in ["canvas", "palette", "layers", "frames", "tags"] {
        if av[field] != bv[field] {
            if let (Some(before), Some(after)) = (av[field].as_array(), bv[field].as_array()) {
                let mut entries = Vec::new();
                let mut changed = 0;
                for i in 0..before.len().max(after.len()) {
                    if before.get(i) != after.get(i) {
                        changed += 1;
                        if entries.len() < MAX_RECORDS {
                            entries.push(
                                json!({"index":i,"before":before.get(i),"after":after.get(i)}),
                            );
                        }
                    }
                }
                structure.push(json!({"field":field,"before_count":before.len(),"after_count":after.len(),"changed_entries":changed,"entries":entries,"truncated":changed>MAX_RECORDS}));
            } else {
                structure.push(json!({"field":field,"before":av[field],"after":bv[field]}));
            }
        }
    }
    let added_frames = bf.iter().filter(|id| !af.contains(id)).collect::<Vec<_>>();
    let removed_frames = af.iter().filter(|id| !bf.contains(id)).collect::<Vec<_>>();
    let added_layers = bl.iter().filter(|id| !al.contains(id)).collect::<Vec<_>>();
    let removed_layers = al.iter().filter(|id| !bl.contains(id)).collect::<Vec<_>>();
    let mut result = ReviewResult::report(
        json!({"kind":"checkpoint_comparison","checkpoint_id":req.checkpoint_id,"source":r,"frames":frames,"cells":cells,"structure":structure,"added_frames":added_frames,"removed_frames":removed_frames,"added_layers":added_layers,"removed_layers":removed_layers}),
    );
    if !panels.is_empty() {
        let (png, meta) = render::panel_sheet(&panels, 3)?;
        result.report["panels"] = meta;
        result.images.push(ReviewImage {
            role: "checkpoint_comparison".into(),
            png,
        });
    }
    Ok(result)
}
