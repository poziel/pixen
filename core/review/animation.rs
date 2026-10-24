use super::*;
use std::collections::BTreeSet;
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditAnimationRequest {
    pub frame_ids: Option<Vec<String>>,
    pub layer_id: Option<String>,
    pub crop: Option<Crop>,
    #[serde(default = "render::one")]
    pub alpha_threshold: u32,
    #[serde(default)]
    pub include_loop_transition: bool,
    pub max_centroid_displacement: Option<f64>,
    pub max_area_ratio: Option<f64>,
    #[serde(default = "render::one")]
    pub scale: u32,
}
fn transition(
    a: &render::ScopedRgba,
    b: &render::ScopedRgba,
    from: &str,
    to: &str,
    is_loop: bool,
    measurements: (&Value, &Value),
    req: &AuditAnimationRequest,
) -> Value {
    let (ma, mb) = measurements;
    let mut diff = compare::pixels(&a.pixels, &b.pixels, a.width);
    if let Some(bounds) = &mut diff.bounds {
        bounds.x += a.source.x;
        bounds.y += a.source.y;
    }
    for e in &mut diff.examples {
        e["x"] = json!(e["x"].as_i64().unwrap() + a.source.x);
        e["y"] = json!(e["y"].as_i64().unwrap() + a.source.y);
    }
    let ca = &ma["centroid"];
    let cb = &mb["centroid"];
    let displacement = if ca.is_null() || cb.is_null() {
        None
    } else {
        Some(
            (ca["x"].as_f64().unwrap() - cb["x"].as_f64().unwrap())
                .hypot(ca["y"].as_f64().unwrap() - cb["y"].as_f64().unwrap()),
        )
    };
    let aa = ma["occupied_pixels"].as_u64().unwrap();
    let ab = mb["occupied_pixels"].as_u64().unwrap();
    let ratio = if aa == 0 && ab == 0 {
        Some(1.0)
    } else if aa == 0 || ab == 0 {
        None
    } else {
        Some(aa.max(ab) as f64 / aa.min(ab) as f64)
    };
    let mut warnings = Vec::new();
    if let (Some(d), Some(limit)) = (displacement, req.max_centroid_displacement)
        && d > limit
    {
        warnings.push("centroid_displacement");
    }
    if let (Some(r), Some(limit)) = (ratio, req.max_area_ratio)
        && r > limit
    {
        warnings.push("occupied_area_ratio");
    }
    json!({"from_frame_id":from,"to_frame_id":to,"loop_transition":is_loop,"changed_ratio":diff.changed_pixels as f64/(a.width as f64*a.height as f64),"diff":diff,"centroid_displacement":displacement,"occupied_area_ratio":ratio,"empty_transition":(aa==0)!=(ab==0),"warnings":warnings})
}
pub fn audit(doc: &Document, req: &AuditAnimationRequest) -> Result<ReviewResult> {
    ensure!(
        (1..=255).contains(&req.alpha_threshold) && req.scale > 0,
        "alpha_threshold must be 1..255 and scale positive"
    );
    ensure!(
        req.max_centroid_displacement
            .is_none_or(|n| n.is_finite() && n >= 0.0)
            && req.max_area_ratio.is_none_or(|n| n.is_finite() && n >= 1.0),
        "invalid animation warning thresholds"
    );
    let resolved = validate_scope(
        doc,
        &Scope {
            frame_ids: req.frame_ids.clone(),
            layer_ids: req.layer_id.clone().map(|id| vec![id]),
            crop: req.crop,
        },
    )?;
    ensure!(
        doc.canvas.width as u64
            * doc.canvas.height as u64
            * resolved.frames.len() as u64
            * resolved.layers.len().max(1) as u64
            <= MAX_WORK,
        "animation exceeds 64 million source pixel visits; select fewer frames/layers"
    );
    let r = resolved.crop;
    let iw = u64::from(r.width) * u64::from(req.scale);
    let ih = u64::from(r.height) * u64::from(req.scale);
    let columns = 4.min(resolved.frames.len() as u64);
    let cw = iw.max(160) + 16;
    let ch = ih + 40;
    let width = cw * columns;
    let height = ch * (resolved.frames.len() as u64).div_ceil(columns);
    ensure!(
        width
            .checked_mul(height)
            .is_some_and(|n| n <= MAX_IMAGE_PIXELS),
        "audit contact sheet exceeds 16 million pixels; reduce scale or select fewer frames"
    );
    let mut canvas =
        image::RgbaImage::from_pixel(width as u32, height as u32, image::Rgba([32, 32, 32, 255]));
    let sprite = crate::schema::compile(doc.clone())?;
    let ids = frame_ids(doc);
    let mut frames = Vec::new();
    let mut transitions = Vec::new();
    let mut holds = Vec::new();
    let mut distinct = BTreeSet::new();
    let mut distinct_bytes = 0usize;
    let mut total = 0.0;
    let mut previous: Option<(render::ScopedRgba, Value, String)> = None;
    let mut first: Option<(render::ScopedRgba, Value, String)> = None;
    let mut run_start = 0usize;
    let mut run_duration = 0.0;
    for (slot, fi) in resolved.frames.iter().enumerate() {
        let rgba = render::from_sprite(&sprite, *fi, req.layer_id.as_deref(), Some(r))?;
        let stats = measure::pixels(&rgba, req.alpha_threshold, 8)?;
        let encoded = render::encode(&render::image(&rgba)?)?;
        if distinct.insert(encoded.clone()) {
            distinct_bytes += encoded.len();
            ensure!(
                distinct_bytes <= 64 * 1024 * 1024,
                "distinct-pose evidence exceeds 64 MiB; select fewer frames"
            );
        }
        let duration = doc.frames[*fi].duration_ms;
        total += duration;
        if let Some((prev, meta, id)) = &previous {
            let t = transition(prev, &rgba, id, &ids[*fi], false, (meta, &stats), req);
            if t["diff"]["changed_pixels"] != 0 {
                if slot - run_start > 1 {
                    holds.push(json!({"start":run_start,"end":slot-1,"duration_ms":run_duration}));
                }
                run_start = slot;
                run_duration = 0.0;
            }
            transitions.push(t);
        }
        run_duration += duration;
        let bounds = &stats["bounds"];
        let edge = if bounds.is_null() {
            false
        } else {
            bounds["x"].as_i64() == Some(0)
                || bounds["y"].as_i64() == Some(0)
                || bounds["x"].as_i64().unwrap() + bounds["width"].as_i64().unwrap()
                    == doc.canvas.width as i64
                || bounds["y"].as_i64().unwrap() + bounds["height"].as_i64().unwrap()
                    == doc.canvas.height as i64
        };
        frames.push(json!({"frame_id":ids[*fi],"source_frame_index":fi,"duration_ms":duration,"bounds":bounds,"occupied_pixels":stats["occupied_pixels"],"centroid":stats["centroid"],"canvas_edge_contact":edge}));
        let img = render::image(&rgba)?;
        let image = image::imageops::resize(
            &img,
            iw as u32,
            ih as u32,
            image::imageops::FilterType::Nearest,
        );
        let x = (slot as u64 % columns * cw + 8) as u32;
        let y = (slot as u64 / columns * ch + 32) as u32;
        image::imageops::replace(&mut canvas, &image, i64::from(x), i64::from(y));
        render::label(
            &mut canvas,
            x,
            y - 24,
            &format!("Frame {} {duration}ms", slot + 1),
        );
        frames.last_mut().unwrap()["image_bounds"] = json!({"x":x,"y":y,"width":iw,"height":ih});
        if first.is_none() {
            first = Some((rgba.clone(), stats.clone(), ids[*fi].clone()));
        }
        previous = Some((rgba, stats, ids[*fi].clone()));
    }
    if resolved.frames.len() - run_start > 1 {
        holds.push(
            json!({"start":run_start,"end":resolved.frames.len()-1,"duration_ms":run_duration}),
        );
    }
    if req.include_loop_transition {
        let (a, ma, aid) = previous.as_ref().unwrap();
        let (b, mb, bid) = first.as_ref().unwrap();
        transitions.push(transition(a, b, aid, bid, true, (ma, mb), req));
    }
    let findings=transitions.iter().flat_map(|t|t["warnings"].as_array().unwrap().iter().map(move|code|json!({"code":code,"severity":"warning","message":"Transition exceeds the caller-supplied threshold; inspect whether intentional.","from_frame_id":t["from_frame_id"],"to_frame_id":t["to_frame_id"]}))).collect::<Vec<_>>();
    Ok(ReviewResult {
        report: json!({"kind":"animation_audit","source":r,"layer_id":req.layer_id,"alpha_threshold":req.alpha_threshold,"include_loop_transition":req.include_loop_transition,"subset_order":req.frame_ids,"total_duration_ms":total,"distinct_poses":distinct.len(),"frames":frames,"transitions":transitions,"holds":holds,"findings":findings}),
        images: vec![ReviewImage {
            role: "animation_audit".into(),
            png: render::encode(&canvas)?,
        }],
    })
}
