use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeSpriteRequest {
    pub frame_id: String,
    pub layer_id: Option<String>,
    pub crop: Option<Crop>,
    #[serde(default = "render::one")]
    pub alpha_threshold: u32,
    #[serde(default = "render::eight")]
    pub connectivity: u32,
    #[serde(default = "yes")]
    pub include_annotation: bool,
}
fn yes() -> bool {
    true
}
pub fn pixels(
    scope: &render::ScopedRgba,
    alpha_threshold: u32,
    connectivity: u32,
) -> Result<Value> {
    ensure!(
        (1..=255).contains(&alpha_threshold),
        "alpha_threshold must be 1..255"
    );
    ensure!(
        connectivity == 4 || connectivity == 8,
        "connectivity must be 4 or 8"
    );
    ensure!(
        scope.pixels.len() == scope.width as usize * scope.height as usize * 4 && scope.width > 0,
        "invalid measurement image"
    );
    let ps = scope.pixels.as_chunks::<4>().0;
    let mut visited = vec![false; ps.len()];
    let mut components = Vec::new();
    let mut count = 0usize;
    let (mut occupied, mut sx, mut sy) = (0u64, 0u64, 0u64);
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    let mut colors = BTreeMap::<[u8; 4], u64>::new();
    let mut transparent = 0u64;
    for (i, p) in ps.iter().enumerate() {
        if p[3] > 0 {
            *colors.entry(*p).or_default() += 1;
        } else {
            transparent += 1;
        }
        if u32::from(p[3]) < alpha_threshold {
            continue;
        }
        let x = i as u32 % scope.width;
        let y = i as u32 / scope.width;
        occupied += 1;
        sx += u64::from(x);
        sy += u64::from(y);
        bounds = Some(match bounds {
            None => (x, y, x, y),
            Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x), b.max(y)),
        });
    }
    for start in 0..ps.len() {
        if visited[start] || u32::from(ps[start][3]) < alpha_threshold {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![start];
        let mut area = 0u64;
        let x = start as u32 % scope.width;
        let y = start as u32 / scope.width;
        let (mut l, mut t, mut r, mut b) = (x, y, x, y);
        while let Some(i) = stack.pop() {
            area += 1;
            let x = i as u32 % scope.width;
            let y = i as u32 / scope.width;
            l = l.min(x);
            t = t.min(y);
            r = r.max(x);
            b = b.max(y);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    if dx == 0 && dy == 0 || connectivity == 4 && dx != 0 && dy != 0 {
                        continue;
                    }
                    let nx = i64::from(x) + dx;
                    let ny = i64::from(y) + dy;
                    if nx < 0
                        || ny < 0
                        || nx >= i64::from(scope.width)
                        || ny >= i64::from(scope.height)
                    {
                        continue;
                    }
                    let n = ny as usize * scope.width as usize + nx as usize;
                    if !visited[n] && u32::from(ps[n][3]) >= alpha_threshold {
                        visited[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        count += 1;
        if components.len() < MAX_RECORDS {
            components.push(json!({"component_id":count,"area":area,"bounds":{"x":scope.source.x+i64::from(l),"y":scope.source.y+i64::from(t),"width":r-l+1,"height":b-t+1},"inspection_candidate":area==1}));
        }
    }
    let bbox=bounds.map(|(x,y,r,b)|json!({"x":scope.source.x+i64::from(x),"y":scope.source.y+i64::from(y),"width":r-x+1,"height":b-y+1}));
    let padding = bounds.map(
        |(x, y, r, b)| json!({"left":x,"top":y,"right":scope.width-1-r,"bottom":scope.height-1-b}),
    );
    let centroid = if occupied == 0 {
        Value::Null
    } else {
        json!({"x":scope.source.x as f64+sx as f64/occupied as f64,"y":scope.source.y as f64+sy as f64/occupied as f64})
    };
    let findings=components.iter().filter(|c|c["area"]==1).map(|c|json!({"code":"isolated_component","severity":"observation","bounds":c["bounds"],"component_id":c["component_id"],"message":"One-pixel component; inspect whether intentional."})).collect::<Vec<_>>();
    Ok(
        json!({"source":scope.source,"alpha_threshold":alpha_threshold,"connectivity":connectivity,"bounds":bbox,"padding":padding,"occupied_pixels":occupied,"unoccupied_pixels":ps.len()as u64-occupied,"transparent_pixels":transparent,"fill_ratio":occupied as f64/ps.len()as f64,"centroid":centroid,"component_count":count,"components":components,"rendered_color_count":colors.len(),"rendered_colors":colors.iter().take(MAX_RECORDS).map(|(rgba,n)|json!({"rgba":format!("#{:02X}{:02X}{:02X}{:02X}",rgba[0],rgba[1],rgba[2],rgba[3]),"pixels":n})).collect::<Vec<_>>(),"findings":findings,"truncation":{"components":count>MAX_RECORDS,"rendered_colors":colors.len()>MAX_RECORDS}}),
    )
}
pub fn annotation_scale(scope: &render::ScopedRgba) -> u32 {
    let mut scale = 8u32;
    while u64::from(scope.width) * u64::from(scope.height) * u64::from(scale) * u64::from(scale)
        > MAX_IMAGE_PIXELS
    {
        scale -= 1;
    }
    scale
}
pub fn annotate(scope: &render::ScopedRgba, report: &Value) -> Result<ReviewImage> {
    let scale = annotation_scale(scope);
    let mut image = image::imageops::resize(
        &render::image(scope)?,
        scope.width * scale,
        scope.height * scale,
        image::imageops::FilterType::Nearest,
    );
    for c in report["components"]
        .as_array()
        .context("missing components")?
    {
        let b = &c["bounds"];
        let x = (b["x"].as_i64().unwrap() - scope.source.x) as u32 * scale;
        let y = (b["y"].as_i64().unwrap() - scope.source.y) as u32 * scale;
        let w = b["width"].as_u64().unwrap() as u32 * scale;
        let h = b["height"].as_u64().unwrap() as u32 * scale;
        for dx in x..x + w {
            image.put_pixel(dx, y, image::Rgba([255, 190, 60, 255]));
            image.put_pixel(dx, y + h - 1, image::Rgba([255, 190, 60, 255]));
        }
        for dy in y..y + h {
            image.put_pixel(x, dy, image::Rgba([255, 190, 60, 255]));
            image.put_pixel(x + w - 1, dy, image::Rgba([255, 190, 60, 255]));
        }
        render::label(&mut image, x, y, &c["component_id"].to_string());
    }
    Ok(ReviewImage {
        role: "components".into(),
        png: render::encode(&image)?,
    })
}
pub fn sprite(doc: &Document, req: &AnalyzeSpriteRequest) -> Result<ReviewResult> {
    ensure!(
        (1..=255).contains(&req.alpha_threshold)
            && (req.connectivity == 4 || req.connectivity == 8),
        "alpha_threshold must be 1..255; connectivity 4 or 8"
    );
    let layers = doc
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| req.layer_id.as_ref().is_none_or(|id| id == &l.id))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    ensure!(
        doc.canvas.width as u64 * doc.canvas.height as u64 * (layers.len() as u64 + 1) <= MAX_WORK,
        "analysis exceeds 64 million source pixel visits; isolate a layer"
    );
    let scope = render::frame(doc, &req.frame_id, req.layer_id.as_deref(), req.crop)?;
    let mut report = pixels(&scope, req.alpha_threshold, req.connectivity)?;
    let f = frame_ids(doc)
        .iter()
        .position(|s| s == &req.frame_id)
        .unwrap();
    let mut raw = BTreeMap::<u32, u64>::new();
    for l in layers {
        let grid = crate::edit::grid(doc, l, f)?;
        for row in grid
            .iter()
            .skip(scope.source.y as usize)
            .take(scope.height as usize)
        {
            for index in row
                .iter()
                .skip(scope.source.x as usize)
                .take(scope.width as usize)
            {
                *raw.entry(*index).or_default() += 1;
            }
        }
    }
    report["kind"] = json!("sprite_analysis");
    report["frame_id"] = json!(req.frame_id);
    report["layer_id"] = json!(req.layer_id);
    report["palette_index_count"] = json!(raw.len());
    report["raw_inventory_semantics"] =
        json!("exact cel indices; all layers including hidden when layer_id is omitted");
    report["palette_indices"] = json!(
        raw.iter()
            .take(MAX_RECORDS)
            .map(|(i, n)| json!({"palette_index":i,"rgba":doc.palette[*i as usize],"pixels":n}))
            .collect::<Vec<_>>()
    );
    report["truncation"]["palette_indices"] = json!(raw.len() > MAX_RECORDS);
    let mut result = ReviewResult::report(report);
    if req.include_annotation {
        result.report["annotation_scale"] = json!(annotation_scale(&scope));
        result.images.push(annotate(&scope, &result.report)?);
    }
    Ok(result)
}
