//! Local inspection state; never writes back to the editing project.
use crate::model::Sprite;
use eframe::egui;
use std::collections::BTreeMap;

fn eye_button(ui: &mut egui::Ui, visible: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 28.0), egui::Sense::click());
    let color = ui.style().interact(&response).fg_stroke.color;
    super::icons::paint(
        ui,
        if visible { "eye" } else { "eye-off" },
        egui::Rect::from_center_size(rect.center(), egui::vec2(22.0, 22.0)),
        color,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(if visible {
            "Hide layer in preview"
        } else {
            "Show layer in preview"
        })
}

#[derive(Default)]
pub(super) struct LayerView {
    selected: Option<String>,
    visibility: BTreeMap<String, bool>,
    thumbnails: BTreeMap<String, egui::TextureHandle>,
    frame: Option<usize>,
}
impl LayerView {
    pub(super) fn select(&mut self, sprite: &Sprite, id: &str) {
        if sprite.layers.iter().any(|layer| layer.id == id) {
            self.selected = Some(id.to_owned());
        }
    }
    fn show_only(&mut self, sprite: &Sprite, id: &str) {
        self.selected = Some(id.to_owned());
        for layer in &sprite.layers {
            self.visibility.insert(layer.id.clone(), layer.id == id);
        }
    }
    pub(super) fn invalidate(&mut self) {
        self.thumbnails.clear();
        self.frame = None;
    }
    pub(super) fn render(&self, sprite: &Sprite, frame: usize) -> anyhow::Result<Vec<u8>> {
        let mut preview = sprite.clone();
        for layer in &mut preview.layers {
            layer.visible = self
                .visibility
                .get(&layer.id)
                .copied()
                .unwrap_or(layer.visible);
        }
        crate::raster::render(&preview, frame)
    }
    pub(super) fn panel(
        &mut self,
        ctx: &egui::Context,
        sprite: &Sprite,
        frame: usize,
        background: super::Background,
        enabled: bool,
    ) -> bool {
        self.visibility
            .retain(|id, _| sprite.layers.iter().any(|layer| &layer.id == id));
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !sprite.layers.iter().any(|layer| &layer.id == id))
        {
            self.selected = None;
        }
        if self.frame != Some(frame) {
            self.thumbnails.clear();
            self.frame = Some(frame);
        }
        let mut changed = false;
        super::theme::sidebar("layers", ctx).show(ctx, |ui| {
            super::theme::fill_sidebar(ui);
            if !enabled {
                ui.disable();
            }
            super::panel_header(ui, |ui| {
                ui.heading("LAYERS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if super::icon_button(ui, super::Icon::Reset, "Reset layer view").clicked() {
                        self.visibility.clear();
                        changed = true;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for layer in sprite.layers.iter().rev() {
                    if !self.thumbnails.contains_key(&layer.id) {
                        let mut isolated = sprite.clone();
                        isolated.layers.retain(|item| item.id == layer.id);
                        isolated.layers[0].visible = true;
                        if let Ok(rgba) = crate::raster::render(&isolated, frame)
                            && let Some(image) = super::square_thumbnail(
                                background,
                                sprite.width,
                                sprite.height,
                                &rgba,
                                40,
                            )
                        {
                            self.thumbnails.insert(
                                layer.id.clone(),
                                ctx.load_texture(
                                    format!("layer-{}", layer.id),
                                    image,
                                    egui::TextureOptions::NEAREST,
                                ),
                            );
                        }
                    }
                    ui.push_id(&layer.id, |ui| {
                        let selected = self.selected.as_ref() == Some(&layer.id);
                        let (rect, row) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 68.0),
                            egui::Sense::click(),
                        );
                        if row.clicked() {
                            self.selected = Some(layer.id.clone());
                        }
                        let fill = if selected {
                            ui.visuals().selection.bg_fill
                        } else if ui.rect_contains_pointer(rect) {
                            ui.visuals().widgets.hovered.bg_fill
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, 4.0, fill);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    let visible = self
                                        .visibility
                                        .get(&layer.id)
                                        .copied()
                                        .unwrap_or(layer.visible);
                                    if eye_button(ui, visible).clicked() {
                                        self.visibility.insert(layer.id.clone(), !visible);
                                        changed = true;
                                    }
                                    if super::icon_button(
                                        ui,
                                        super::Icon::Solo,
                                        "Show only this layer",
                                    )
                                    .clicked()
                                    {
                                        self.show_only(sprite, &layer.id);
                                        changed = true;
                                    }
                                });
                                if let Some(texture) = self.thumbnails.get(&layer.id) {
                                    ui.vertical(|ui| {
                                        ui.add_space(8.0);
                                        ui.add(
                                            egui::Image::new(texture)
                                                .fit_to_exact_size(egui::vec2(40.0, 40.0))
                                                .maintain_aspect_ratio(true)
                                                .sense(egui::Sense::empty()),
                                        );
                                    });
                                }
                                ui.vertical(|ui| {
                                    ui.add_space(8.0);
                                    let name = ui.add(
                                        egui::Label::new(&layer.name)
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    );
                                    let opacity = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!(
                                                "{:.0}% opacity",
                                                layer.opacity * 100.0
                                            ))
                                            .small(),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if name.clicked() || opacity.clicked() {
                                        self.selected = Some(layer.id.clone());
                                    }
                                });
                            });
                        });
                    });
                }
            });
        });
        changed
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn clicking_name_and_opacity_selects_the_layer() {
        use eframe::egui;
        let sprite =
            crate::schema::parse(include_bytes!("../skills/pixloom/examples/animated.json"))
                .unwrap();
        for (target, enabled) in [sprite.layers[0].name.clone(), "100% opacity".into()]
            .into_iter()
            .flat_map(|target| [(target.clone(), false), (target, true)])
        {
            let ctx = egui::Context::default();
            let mut view = super::LayerView::default();
            let input = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..Default::default()
            };
            for _ in 0..2 {
                let _ = ctx.run(input(), |ctx| {
                    view.panel(
                        ctx,
                        &sprite,
                        0,
                        super::super::Background::Checkerboard,
                        enabled,
                    );
                });
            }
            let output = ctx.run(input(), |ctx| {
                view.panel(
                    ctx,
                    &sprite,
                    0,
                    super::super::Background::Checkerboard,
                    enabled,
                );
            });
            let pos = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == target => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("layer label rendered");
            let mut click = input();
            click.events = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ];
            let _ = ctx.run(click, |ctx| {
                view.panel(
                    ctx,
                    &sprite,
                    0,
                    super::super::Background::Checkerboard,
                    enabled,
                );
            });
            assert_eq!(
                view.selected.as_deref(),
                if enabled {
                    Some(sprite.layers[0].id.as_str())
                } else {
                    None
                },
                "{target}"
            );
        }
    }
    #[test]
    fn hiding_and_solo_are_preview_only_and_follow_stable_ids() {
        let mut sprite =
            crate::schema::parse(include_bytes!("../skills/pixloom/examples/animated.json"))
                .unwrap();
        let mut second = sprite.layers[0].clone();
        second.id = "second".into();
        sprite.layers.push(second);
        let original = crate::raster::render(&sprite, 0).unwrap();
        let mut view = super::LayerView::default();
        for layer in &sprite.layers {
            view.visibility.insert(layer.id.clone(), false);
        }
        assert!(view.render(&sprite, 0).unwrap().iter().all(|v| *v == 0));
        view.visibility.clear();
        view.show_only(&sprite, "second");
        assert!(!view.visibility[&sprite.layers[0].id]);
        assert!(view.visibility["second"]);
        let solo = view.render(&sprite, 0).unwrap();
        sprite.layers.swap(0, 1);
        assert_eq!(view.render(&sprite, 0).unwrap(), solo);
        assert_eq!(crate::raster::render(&sprite, 0).unwrap(), original);
        view.visibility.insert(sprite.layers[1].id.clone(), true);
        assert_eq!(view.render(&sprite, 0).unwrap(), original);
        assert!(sprite.layers.iter().all(|layer| layer.visible));
    }
}
