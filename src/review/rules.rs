use super::checkpoints::CheckpointStore;
use super::*;
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtRule {
    CanvasSize {
        width: u32,
        height: u32,
    },
    FrameCount {
        min: usize,
        max: usize,
    },
    FrameDuration {
        min_ms: f64,
        max_ms: f64,
        frame_ids: Option<Vec<String>>,
    },
    AllowedPaletteIndices {
        indices: Vec<u32>,
        scope: Option<Scope>,
    },
    MaxRenderedColors {
        max_colors: usize,
        scope: Option<Scope>,
    },
    BinaryAlpha {
        scope: Option<Scope>,
    },
    ProtectedRegion {
        checkpoint_id: String,
        region: Crop,
        frame_ids: Vec<String>,
        layer_id: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateArtworkRequest {
    pub rules: Vec<ArtRule>,
}
fn scoped_sprite(
    doc: &Document,
    scope: &ResolvedScope,
    isolated: bool,
) -> Result<crate::model::Sprite> {
    let mut sprite = crate::schema::compile(doc.clone())?;
    let ids = scope
        .layers
        .iter()
        .map(|i| doc.layers[*i].id.as_str())
        .collect::<Vec<_>>();
    sprite.layers.retain(|l| ids.contains(&l.id.as_str()));
    if isolated && sprite.layers.len() == 1 {
        sprite.layers[0].visible = true;
    }
    Ok(sprite)
}
fn isolated(scope: &Option<Scope>) -> bool {
    scope
        .as_ref()
        .is_some_and(|s| s.layer_ids.as_ref().is_some_and(|ls| ls.len() == 1))
}
fn frames(doc: &Document, ids: Option<&[String]>) -> Result<Vec<usize>> {
    indices(ids, &frame_ids(doc), 64)
}
fn work(doc: &Document, n: usize, layers: usize) -> u64 {
    u64::from(doc.canvas.width) * u64::from(doc.canvas.height) * n as u64 * layers.max(1) as u64
}
fn scoped(doc: &Document, s: &Option<Scope>) -> Result<ResolvedScope> {
    validate_scope(doc, s.as_ref().unwrap_or(&Scope::default()))
}
pub fn validate(
    doc: &Document,
    checkpoints: &CheckpointStore,
    req: &ValidateArtworkRequest,
) -> Result<ReviewResult> {
    ensure!(
        !req.rules.is_empty() && req.rules.len() <= 64,
        "provide 1..64 rules"
    );
    let mut cost = 0u64;
    let mut baseline_cost = 0u64;
    // Reject malformed rules and budget overflow before scanning artwork.
    for rule in &req.rules {
        match rule {
            ArtRule::CanvasSize { width, height } => {
                crate::schema::pixel_bytes(*width, *height)?;
            }
            ArtRule::FrameCount { min, max } => ensure!(min <= max, "inverted frame-count range"),
            ArtRule::FrameDuration {
                min_ms,
                max_ms,
                frame_ids,
            } => {
                ensure!(
                    min_ms.is_finite() && max_ms.is_finite() && *min_ms > 0.0 && min_ms <= max_ms,
                    "invalid frame-duration range"
                );
                frames(doc, frame_ids.as_deref())?;
            }
            ArtRule::AllowedPaletteIndices { indices, scope } => {
                ensure!(
                    !indices.is_empty()
                        && indices.iter().all(|i| (*i as usize) < doc.palette.len()),
                    "unknown or empty palette indices"
                );
                let s = scoped(doc, scope)?;
                cost += work(doc, s.frames.len(), s.layers.len());
            }
            ArtRule::MaxRenderedColors { scope, .. } | ArtRule::BinaryAlpha { scope } => {
                let s = scoped(doc, scope)?;
                cost += work(doc, s.frames.len(), s.layers.len());
            }
            ArtRule::ProtectedRegion {
                checkpoint_id,
                region,
                frame_ids: ids,
                layer_id,
            } => {
                let (base, _) = checkpoints.document(checkpoint_id)?;
                crop(base.canvas.width, base.canvas.height, Some(*region))?;
                let af = frame_ids(&base);
                let bf = frame_ids(doc);
                let selected = compare::selected_union(Some(ids), af, bf, 64)?;
                if let Some(id) = layer_id {
                    ensure!(
                        base.layers.iter().any(|l| &l.id == id)
                            || doc.layers.iter().any(|l| &l.id == id),
                        "unknown protected layer_id"
                    );
                }
                cost += work(
                    doc,
                    selected.len(),
                    if layer_id.is_some() {
                        1
                    } else {
                        doc.layers.len()
                    },
                );
                baseline_cost += work(
                    &base,
                    selected.len(),
                    if layer_id.is_some() {
                        1
                    } else {
                        base.layers.len()
                    },
                );
            }
        }
        ensure!(
            cost <= MAX_WORK && baseline_cost <= MAX_WORK,
            "validation exceeds 64 million source pixels per side; split rules or scope"
        );
    }
    let mut reports = Vec::new();
    let mut all_passed = true;
    for rule in &req.rules {
        let expected = serde_json::to_value(rule).unwrap_or(Value::Null);
        let (mut violations, mut examples) = (0u64, Vec::new());
        let observed = match rule {
            ArtRule::CanvasSize { width, height } => {
                if doc.canvas.width != *width || doc.canvas.height != *height {
                    violations = 1;
                }
                json!({"width":doc.canvas.width,"height":doc.canvas.height})
            }
            ArtRule::FrameCount { min, max } => {
                if doc.frames.len() < *min || doc.frames.len() > *max {
                    violations = 1;
                }
                json!({"count":doc.frames.len()})
            }
            ArtRule::FrameDuration {
                min_ms,
                max_ms,
                frame_ids: ids,
            } => {
                let fs = frames(doc, ids.as_deref())?;
                for f in &fs {
                    let duration = doc.frames[*f].duration_ms;
                    if duration < *min_ms || duration > *max_ms {
                        violations += 1;
                        if examples.len() < MAX_RECORDS {
                            examples.push(
                                json!({"frame_id":frame_ids(doc)[*f],"duration_ms":duration}),
                            );
                        }
                    }
                }
                json!({"checked_frames":fs.len()})
            }
            ArtRule::AllowedPaletteIndices { indices, scope } => {
                let allowed = indices
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>();
                let s = scoped(doc, scope)?;
                for fi in &s.frames {
                    for li in &s.layers {
                        let g = crate::edit::grid(doc, *li, *fi)?;
                        for (y, row) in g
                            .iter()
                            .enumerate()
                            .skip(s.crop.y as usize)
                            .take(s.crop.height as usize)
                        {
                            for (x, &index) in row
                                .iter()
                                .enumerate()
                                .skip(s.crop.x as usize)
                                .take(s.crop.width as usize)
                            {
                                if !allowed.contains(&index) {
                                    violations += 1;
                                    if examples.len() < MAX_RECORDS {
                                        examples.push(json!({"frame_id":frame_ids(doc)[*fi],"layer_id":doc.layers[*li].id,"x":x,"y":y,"palette_index":index}));
                                    }
                                }
                            }
                        }
                    }
                }
                json!({"checked_frames":s.frames.len(),"checked_layers":s.layers.len()})
            }
            ArtRule::MaxRenderedColors { max_colors, scope } => {
                let s = scoped(doc, scope)?;
                let mut colors = std::collections::BTreeSet::<[u8; 4]>::new();
                let sprite = scoped_sprite(doc, &s, isolated(scope))?;
                for fi in &s.frames {
                    let rgba = render::from_sprite(&sprite, *fi, None, Some(s.crop))?;
                    for p in rgba.pixels.as_chunks::<4>().0 {
                        if p[3] > 0 {
                            colors.insert(*p);
                        }
                    }
                }
                if colors.len() > *max_colors {
                    violations = (colors.len() - max_colors) as u64;
                }
                json!({"rendered_colors":colors.len()})
            }
            ArtRule::BinaryAlpha { scope } => {
                let s = scoped(doc, scope)?;
                let sprite = scoped_sprite(doc, &s, isolated(scope))?;
                for fi in &s.frames {
                    let rgba = render::from_sprite(&sprite, *fi, None, Some(s.crop))?;
                    for (i, p) in rgba.pixels.as_chunks::<4>().0.iter().enumerate() {
                        if p[3] != 0 && p[3] != 255 {
                            violations += 1;
                            if examples.len() < MAX_RECORDS {
                                examples.push(json!({"frame_id":frame_ids(doc)[*fi],"x":s.crop.x+(i as u32%s.crop.width)as i64,"y":s.crop.y+(i as u32/s.crop.width)as i64,"alpha":p[3]}));
                            }
                        }
                    }
                }
                json!({"checked_frames":s.frames.len()})
            }
            ArtRule::ProtectedRegion {
                checkpoint_id,
                region,
                frame_ids: ids,
                layer_id,
            } => {
                let (base, rev) = checkpoints.document(checkpoint_id)?;
                let af = frame_ids(&base);
                let bf = frame_ids(doc);
                for id in ids {
                    let present = af.contains(id)
                        && bf.contains(id)
                        && layer_id.as_ref().is_none_or(|lid| {
                            base.layers.iter().any(|l| &l.id == lid)
                                && doc.layers.iter().any(|l| &l.id == lid)
                        });
                    if !present || crop(doc.canvas.width, doc.canvas.height, Some(*region)).is_err()
                    {
                        violations += 1;
                        if examples.len() < MAX_RECORDS {
                            examples.push(json!({"frame_id":id,"reason":"protected target or pixels missing"}));
                        }
                        continue;
                    }
                    let a = render::frame(&base, id, layer_id.as_deref(), Some(*region))?;
                    let b = render::frame(doc, id, layer_id.as_deref(), Some(*region))?;
                    let diff = compare::pixels(&a.pixels, &b.pixels, region.width);
                    violations += diff.changed_pixels;
                    for mut e in diff.examples {
                        if examples.len() >= MAX_RECORDS {
                            break;
                        }
                        e["x"] = json!(e["x"].as_i64().unwrap() + region.x);
                        e["y"] = json!(e["y"].as_i64().unwrap() + region.y);
                        e["frame_id"] = json!(id);
                        examples.push(e);
                    }
                }
                json!({"baseline_revision":rev,"region":region})
            }
        };
        let passed = violations == 0;
        all_passed &= passed;
        reports.push(json!({"code":expected["type"],"passed":passed,"expected":expected,"observed":observed,"violation_count":violations,"examples":examples,"examples_truncated":violations>examples.len()as u64}));
    }
    let findings=reports.iter().filter(|r|r["passed"]==false).map(|r|json!({"code":r["code"],"severity":"rule_failure","message":"Artwork violates the caller-supplied rule.","evidence":r["observed"]})).collect::<Vec<_>>();
    Ok(ReviewResult::report(
        json!({"kind":"artwork_validation","passed":all_passed,"rules":reports,"findings":findings}),
    ))
}
