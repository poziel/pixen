use super::*;
use image::{Rgba, RgbaImage};
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewSheetRequest {
    pub frame_id: String,
    pub layer_id: Option<String>,
    pub crop: Option<Crop>,
    #[serde(default = "one")]
    pub display_scale: u32,
    #[serde(default = "eight")]
    pub enlarged_scale: u32,
    #[serde(default = "default_views")]
    pub views: Vec<Representation>,
    #[serde(default = "default_backgrounds")]
    pub backgrounds: Vec<String>,
}
pub fn one() -> u32 {
    1
}
pub fn eight() -> u32 {
    8
}
fn default_views() -> Vec<Representation> {
    vec![
        Representation::Color,
        Representation::Silhouette,
        Representation::Grayscale,
    ]
}
fn default_backgrounds() -> Vec<String> {
    vec!["#FFFFFFFF".into(), "#202020FF".into()]
}
#[derive(Clone, Debug)]
pub struct ScopedRgba {
    pub source: Crop,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
pub fn transform(rgba: &[u8], mode: Representation) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for p in out.as_chunks_mut::<4>().0 {
        match mode {
            Representation::Color => {}
            Representation::Silhouette => p[..3].fill(0),
            Representation::Grayscale => {
                let g = ((u32::from(p[0]) * 2126
                    + u32::from(p[1]) * 7152
                    + u32::from(p[2]) * 722
                    + 5000)
                    / 10000) as u8;
                p[..3].fill(g);
            }
        }
    }
    out
}
pub fn from_sprite(
    sprite: &crate::model::Sprite,
    index: usize,
    layer_id: Option<&str>,
    region: Option<Crop>,
) -> Result<ScopedRgba> {
    let source = crop(sprite.width, sprite.height, region)?;
    let mut isolated;
    let sprite = if let Some(id) = layer_id {
        isolated = sprite.clone();
        ensure!(
            isolated.layers.iter().any(|l| l.id == id),
            "unknown layer_id"
        );
        isolated.layers.retain(|l| l.id == id);
        isolated.layers[0].visible = true;
        &isolated
    } else {
        sprite
    };
    let pixels = crate::raster::render(sprite, index)?;
    let mut out = Vec::with_capacity(source.width as usize * source.height as usize * 4);
    for y in source.y as u32..source.y as u32 + source.height {
        let start = (y as usize * sprite.width as usize + source.x as usize) * 4;
        out.extend_from_slice(&pixels[start..start + source.width as usize * 4]);
    }
    Ok(ScopedRgba {
        source,
        width: source.width,
        height: source.height,
        pixels: out,
    })
}
pub fn frame(
    doc: &Document,
    frame_id: &str,
    layer_id: Option<&str>,
    region: Option<Crop>,
) -> Result<ScopedRgba> {
    let i = frame_ids(doc)
        .iter()
        .position(|s| s == frame_id)
        .context("unknown frame_id")?;
    from_sprite(&crate::schema::compile(doc.clone())?, i, layer_id, region)
}
pub fn image(scope: &ScopedRgba) -> Result<RgbaImage> {
    RgbaImage::from_raw(scope.width, scope.height, scope.pixels.clone())
        .context("invalid review RGBA")
}
pub fn label(image: &mut RgbaImage, x: u32, y: u32, text: &str) {
    use ab_glyph::{Font, FontArc, ScaleFont};

    let Ok(font) = FontArc::try_from_slice(include_bytes!("../../assets/fonts/Hack-Regular.ttf"))
    else {
        return;
    };
    let scaled = font.as_scaled(12.0);
    let mut pen = x as f32;
    for c in text.chars() {
        let id = font.glyph_id(c);
        let glyph =
            id.with_scale_and_position(12.0, ab_glyph::point(pen, y as f32 + scaled.ascent()));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i64 + i64::from(gx);
                let py = bounds.min.y as i64 + i64::from(gy);
                if px >= 0 && py >= 0 && (px as u32) < image.width() && (py as u32) < image.height()
                {
                    let p = image.get_pixel_mut(px as u32, py as u32);
                    for v in &mut p.0[..3] {
                        *v = (*v as f32 * (1.0 - coverage) + 235.0 * coverage).round() as u8;
                    }
                    p.0[3] = 255;
                }
            });
        }
        pen += scaled.h_advance(id);
    }
}
pub fn encode(image: &RgbaImage) -> Result<Vec<u8>> {
    ensure!(
        u64::from(image.width()) * u64::from(image.height()) <= MAX_IMAGE_PIXELS,
        "review image exceeds 16 million pixels; reduce scale/views/frames"
    );
    let mut out = Vec::new();
    crate::raster::png(&mut out, image.width(), image.height(), image.as_raw())?;
    Ok(out)
}
/// Checked, neutral-label panels; every pixel including padding counts toward the image budget.
pub fn panel_sheet(panels: &[(String, RgbaImage)], columns: u32) -> Result<(Vec<u8>, Value)> {
    ensure!(!panels.is_empty() && columns > 0, "empty review sheet");
    let columns = columns.min(panels.len() as u32);
    let cw = panels
        .iter()
        .map(|(label, img)| u64::from(img.width()).max(label.chars().count() as u64 * 8) + 16)
        .max()
        .unwrap();
    let ch = panels
        .iter()
        .map(|(_, img)| u64::from(img.height()) + 32)
        .max()
        .unwrap();
    let w = cw * columns as u64;
    let h = ch * (panels.len() as u64).div_ceil(columns as u64);
    ensure!(
        w.checked_mul(h).is_some_and(|n| n <= MAX_IMAGE_PIXELS),
        "review sheet exceeds 16 million pixels; reduce scales/views/frames"
    );
    let mut canvas = RgbaImage::from_pixel(w as u32, h as u32, Rgba([32, 32, 32, 255]));
    let mut bounds = Vec::new();
    for (i, (text, img)) in panels.iter().enumerate() {
        let x = (i as u64 % columns as u64 * cw + 8) as u32;
        let y = (i as u64 / columns as u64 * ch + 24) as u32;
        image::imageops::replace(&mut canvas, img, i64::from(x), i64::from(y));
        label(&mut canvas, x, y - 18, text);
        bounds.push(json!({"label":text,"image_bounds":{"x":x,"y":y,"width":img.width(),"height":img.height()}}));
    }
    Ok((encode(&canvas)?, json!(bounds)))
}
pub fn sheet(doc: &Document, req: &ReviewSheetRequest) -> Result<ReviewResult> {
    ensure!(
        req.display_scale > 0 && req.enlarged_scale > 0,
        "scales must be positive"
    );
    ensure!(
        !req.views.is_empty() && req.views.len() <= 3,
        "select 1..3 representations"
    );
    for (i, v) in req.views.iter().enumerate() {
        ensure!(!req.views[..i].contains(v), "duplicate representation");
    }
    ensure!(
        !req.backgrounds.is_empty() && req.backgrounds.len() <= 4,
        "select 1..4 opaque backgrounds"
    );
    let backgrounds = req
        .backgrounds
        .iter()
        .map(|c| {
            let c = crate::schema::decode_color(c)?;
            ensure!(c[3] == 255, "review backgrounds must be opaque");
            Ok(c)
        })
        .collect::<Result<Vec<_>>>()?;
    let scope = frame(doc, &req.frame_id, req.layer_id.as_deref(), req.crop)?;
    let mut scales = vec![1, req.display_scale, req.enlarged_scale];
    scales.sort_unstable();
    scales.dedup();
    let mut total = 0u64;
    for scale in &scales {
        let w = u64::from(scope.width) * u64::from(*scale);
        let h = u64::from(scope.height) * u64::from(*scale);
        ensure!(
            w <= u32::MAX as u64 && h <= u32::MAX as u64,
            "review dimensions overflow"
        );
        total = total
            .checked_add(
                (w + 160)
                    .checked_mul(h + 32)
                    .context("review dimensions overflow")?
                    .checked_mul(req.views.len() as u64 * backgrounds.len() as u64)
                    .context("review dimensions overflow")?,
            )
            .context("review dimensions overflow")?;
    }
    ensure!(
        total <= MAX_IMAGE_PIXELS,
        "review sheet exceeds 16 million pixels; reduce scales/views"
    );
    let mut panels = Vec::new();
    let mut meta = Vec::new();
    for view in &req.views {
        let transformed = transform(&scope.pixels, *view);
        for (bi, bg) in backgrounds.iter().enumerate() {
            let mut pixels = transformed.clone();
            for p in pixels.as_chunks_mut::<4>().0 {
                let a = u32::from(p[3]);
                for i in 0..3 {
                    p[i] = ((u32::from(p[i]) * a + u32::from(bg[i]) * (255 - a) + 127) / 255) as u8;
                }
                p[3] = 255;
            }
            let img = RgbaImage::from_raw(scope.width, scope.height, pixels).unwrap();
            for scale in &scales {
                let scaled = image::imageops::resize(
                    &img,
                    scope.width * scale,
                    scope.height * scale,
                    image::imageops::FilterType::Nearest,
                );
                let mut roles = Vec::new();
                if *scale == 1 {
                    roles.push("native");
                }
                if *scale == req.display_scale {
                    roles.push("display");
                }
                if *scale == req.enlarged_scale {
                    roles.push("enlarged");
                }
                panels.push((format!("{view:?} {scale}x B{}", bi + 1), scaled));
                meta.push(json!({"representation":view,"scale":scale,"scale_roles":roles,"background":req.backgrounds[bi]}));
            }
        }
    }
    let (png, bounds) = panel_sheet(&panels, scales.len() as u32)?;
    for (m, b) in meta.iter_mut().zip(bounds.as_array().unwrap()) {
        m["image_bounds"] = b["image_bounds"].clone();
    }
    Ok(ReviewResult {
        report: json!({"kind":"review_sheet","frame_id":req.frame_id,"layer_id":req.layer_id,"source":scope.source,"representation":req.views[0],"panels":meta}),
        images: vec![ReviewImage {
            role: "review_sheet".into(),
            png,
        }],
    })
}
