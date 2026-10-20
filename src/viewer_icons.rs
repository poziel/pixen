//! Editable runtime SVGs; texture cache refreshes edited files without recompiling.
use super::*;

#[derive(Clone)]
struct Cached {
    checked: Instant,
    modified: Option<SystemTime>,
    texture: egui::TextureHandle,
}

fn directory() -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    if source.is_dir() {
        source
    } else {
        std::env::current_exe()
            .unwrap_or_default()
            .with_file_name("icons")
    }
}

fn raster(bytes: &[u8]) -> Result<egui::ColorImage> {
    ensure!(bytes.len() <= 128 * 1024, "SVG icon exceeds 128 KiB");
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())?;
    let mut pixels = resvg::tiny_skia::Pixmap::new(72, 72).context("icon buffer")?;
    let scale = (72.0 / tree.size().width()).min(72.0 / tree.size().height());
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(
        (72.0 - tree.size().width() * scale) * 0.5,
        (72.0 - tree.size().height() * scale) * 0.5,
    );
    resvg::render(&tree, transform, &mut pixels.as_mut());
    Ok(egui::ColorImage::from_rgba_premultiplied(
        [72, 72],
        pixels.data(),
    ))
}

pub(super) fn paint(ui: &egui::Ui, name: &str, rect: egui::Rect, color: egui::Color32) {
    let id = egui::Id::new(("svg-icon", name));
    let cached = ui.ctx().data(|d| d.get_temp::<Cached>(id));
    let texture = if let Some(cached) = &cached
        && cached.checked.elapsed() < Duration::from_millis(500)
    {
        Some(cached.texture.clone())
    } else {
        let path = directory().join(format!("{name}.svg"));
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if let Some(mut cached) = cached.clone()
            && cached.modified == modified
        {
            cached.checked = Instant::now();
            ui.ctx().data_mut(|d| d.insert_temp(id, cached.clone()));
            Some(cached.texture)
        } else {
            let image = std::fs::read(&path)
                .ok()
                .and_then(|data| raster(&data).ok());
            image
                .map(|image| {
                    let texture = ui.ctx().load_texture(
                        format!("icon-{name}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(
                            id,
                            Cached {
                                checked: Instant::now(),
                                modified,
                                texture: texture.clone(),
                            },
                        )
                    });
                    texture
                })
                .or_else(|| cached.map(|c| c.texture))
        }
    };
    if let Some(texture) = texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_runtime_svgs_render_visible_pixels() {
        for entry in std::fs::read_dir(directory()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "svg") {
                continue;
            }
            let image = raster(&std::fs::read(&path).unwrap()).unwrap();
            assert!(image.pixels.iter().any(|p| p.a() > 0), "{}", path.display());
        }
    }
}
