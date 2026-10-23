//! Runtime theme registry. Set PIXLOOM_THEME to a registered name or JSON path.
use super::*;
use std::sync::OnceLock;
fn config() -> &'static Value {
    static THEME: OnceLock<Value> = OnceLock::new();
    THEME.get_or_init(|| {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/themes");
        let root = if source.is_dir() {
            source
        } else {
            std::env::current_exe()
                .unwrap_or_default()
                .with_file_name("themes")
        };
        let registry: Value = std::fs::read(root.join("manifest.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let selected = std::env::var("PIXLOOM_THEME")
            .ok()
            .unwrap_or_else(|| registry["default"].as_str().unwrap_or("2026-dark").into());
        let path = registry["themes"][&selected]
            .as_str()
            .map(|p| root.join(p))
            .unwrap_or_else(|| PathBuf::from(selected));
        let fallback: Value =
            serde_json::from_str(include_str!("../assets/themes/2026-dark.json")).unwrap();
        let mut value = fallback;
        if let Some(custom) = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        {
            if let Some(font) = custom["font_file"].as_str() {
                value["font_file"] = font.into();
            }
            for section in ["colors", "metrics"] {
                if let Some(entries) = custom[section].as_object() {
                    for (key, entry) in entries {
                        let valid = if section == "metrics" {
                            entry
                                .as_f64()
                                .is_some_and(|n| n.is_finite() && (0.0..=4096.0).contains(&n))
                        } else {
                            entry.as_str().is_some_and(|s| {
                                s.len() == 7
                                    && s.starts_with('#')
                                    && u32::from_str_radix(&s[1..], 16).is_ok()
                            })
                        };
                        if valid && !value[section][key].is_null() {
                            value[section][key] = entry.clone();
                        }
                    }
                }
            }
        }
        value
    })
}
pub(super) fn metric(name: &str) -> f32 {
    config()["metrics"][name]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0)
        .min(4096.0) as f32
}
pub(super) fn color(name: &str) -> egui::Color32 {
    let s = config()["colors"][name].as_str().unwrap_or("#BFBFBF");
    u32::from_str_radix(s.trim_start_matches('#'), 16)
        .ok()
        .map(|v| egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
        .unwrap_or(egui::Color32::GRAY)
}
pub(super) fn margin(name: &str) -> egui::Margin {
    egui::Margin::same(metric(name).min(127.0) as i8)
}
pub(super) fn radius(name: &str) -> egui::CornerRadius {
    egui::CornerRadius::same(metric(name).min(255.0) as u8)
}
pub(super) fn stroke() -> egui::Stroke {
    egui::Stroke::new(metric("border_width"), color("border"))
}
pub(super) fn apply(ctx: &egui::Context) {
    let font_id = egui::Id::new("theme-font-loaded");
    if !ctx.data(|d| d.get_temp::<bool>(font_id)).unwrap_or(false) {
        if let Ok(bytes) = std::fs::read(
            config()["font_file"]
                .as_str()
                .unwrap_or("C:/Windows/Fonts/segoeui.ttf"),
        ) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Segoe UI".into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Segoe UI".into());
            ctx.set_fonts(fonts);
        }
        ctx.data_mut(|d| d.insert_temp(font_id, true));
    }
    ctx.style_mut(|s| {
        s.visuals.panel_fill = color("shell");
        s.visuals.window_fill = color("raised");
        s.visuals.window_stroke = stroke();
        s.visuals.window_corner_radius = radius("corner_radius");
        s.visuals.menu_corner_radius = radius("corner_radius");
        s.visuals.override_text_color = Some(color("text"));
        s.visuals.weak_text_color = Some(color("muted"));
        s.visuals.extreme_bg_color = color("canvas");
        s.visuals.faint_bg_color = color("raised");
        s.visuals.selection.bg_fill = color("selected");
        s.visuals.selection.stroke = egui::Stroke::new(1.0_f32, color("text"));
        for (state, fill) in [
            (&mut s.visuals.widgets.inactive, "button"),
            (&mut s.visuals.widgets.hovered, "button_hover"),
            (&mut s.visuals.widgets.active, "selected"),
            (&mut s.visuals.widgets.noninteractive, "panel"),
        ] {
            state.bg_fill = color(fill);
            state.weak_bg_fill = color(fill);
            state.bg_stroke = stroke();
            state.fg_stroke.color = color("text");
            state.corner_radius = radius("control_radius");
        }
        s.spacing.item_spacing = egui::vec2(metric("gap"), metric("gap"));
        s.spacing.button_padding = egui::vec2(metric("panel_padding"), 4.0);
        s.spacing.menu_margin = egui::Margin::symmetric(0, 4);
        s.spacing.interact_size.y = metric("button_size");
        s.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(metric("font_size")),
        );
        s.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(metric("font_size")),
        );
        s.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional(metric("small_font_size")),
        );
        s.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(metric("small_font_size")),
        );
    });
}

/// Sidebar interiors are transparent: rail and content share one enclosing frame.
pub(super) fn sidebar_frame() -> egui::Frame {
    egui::Frame::NONE.inner_margin(egui::Margin {
        left: metric("panel_padding") as i8,
        right: metric("panel_padding") as i8,
        top: (metric("panel_padding") + metric("gap")) as i8,
        bottom: (metric("panel_padding") + metric("gap")) as i8,
    })
}
pub(super) fn sidebar(id: &'static str, ctx: &egui::Context) -> egui::SidePanel {
    egui::SidePanel::left(id)
        .resizable(true)
        .show_separator_line(false)
        .default_width(metric("sidebar_width"))
        .width_range(
            metric("sidebar_min_width")
                ..=(ctx.available_rect().width() - metric("canvas_min_width"))
                    .max(metric("sidebar_min_width")),
        )
        .frame(sidebar_frame())
}

pub(super) fn fill_sidebar(ui: &mut egui::Ui) {
    ui.set_min_width(ui.available_width());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_edge_drags_in_both_directions_and_reserves_canvas_space() {
        let ctx = egui::Context::default();
        let frame = |events| {
            let mut rect = egui::Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    rect = sidebar("resize-test", ctx)
                        .show(ctx, |ui| {
                            fill_sidebar(ui);
                            ui.label("Layers");
                        })
                        .response
                        .rect;
                },
            );
            rect
        };
        frame(vec![]);
        let initial = frame(vec![]);
        let edge = egui::pos2(initial.right(), 200.0);
        frame(vec![egui::Event::PointerMoved(edge)]);
        frame(vec![egui::Event::PointerButton {
            pos: edge,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        let wide_point = edge + egui::vec2(100.0, 0.0);
        frame(vec![egui::Event::PointerMoved(wide_point)]);
        let wider = frame(vec![]);
        assert!(wider.width() > initial.width() + 50.0);
        let narrow_point = edge - egui::vec2(80.0, 0.0);
        frame(vec![egui::Event::PointerMoved(narrow_point)]);
        let narrower = frame(vec![]);
        assert!(narrower.width() < initial.width() - 40.0);
        assert!(narrower.width() >= metric("sidebar_min_width"));
        assert!(900.0 - wider.width() >= metric("canvas_min_width"));
        frame(vec![egui::Event::PointerButton {
            pos: narrow_point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
    }
}
