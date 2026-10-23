use super::*;

impl App {
    pub(super) fn select_panel(&mut self, index: usize) {
        let was_open = match index {
            0 => self.show_layers,
            1 => self.show_animation,
            _ => self.conversation.visible,
        };
        self.show_layers = index == 0 && !was_open;
        self.show_animation = index == 1 && !was_open;
        self.conversation.visible = index == 2 && !was_open;
    }
    pub(super) fn activity_rail(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("activity-rail")
            .exact_width(theme::metric("rail_width"))
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 6,
                right: 6,
                top: theme::metric("gap") as i8 + 2,
                bottom: theme::metric("gap") as i8,
            }))
            .show(ctx, |ui| {
                for (index, name, label, active) in [
                    (0, "panel_layers", "Layers", self.show_layers),
                    (1, "panel_animation", "Animation", self.show_animation),
                    (2, "panel_ai", "AI conversation", self.conversation.visible),
                ] {
                    let side = (theme::metric("rail_width") - 2.0 * theme::metric("gap")).max(24.0);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(side, theme::metric("rail_item_height")),
                        egui::Sense::click(),
                    );
                    if active || response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            theme::radius("control_radius"),
                            theme::color(if active { "selected" } else { "hover" }),
                        );
                    }
                    icons::paint(
                        ui,
                        name,
                        egui::Rect::from_center_size(
                            rect.center(),
                            egui::Vec2::splat(theme::metric("icon_size")),
                        ),
                        theme::color(if active || response.hovered() {
                            "icon_hover"
                        } else {
                            "icon"
                        }),
                    );
                    if response.on_hover_text(label).clicked() {
                        self.select_panel(index);
                    }
                }
            });
    }
    fn window_title(&self) -> String {
        self.conversation
            .saved_path()
            .or(self.opened_source.as_deref())
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if self.sprite.is_some() {
                    "Untitled.pixloom".into()
                } else {
                    "Pixloom".into()
                }
            })
    }

    #[cfg(test)]
    fn toggle_conversation(&mut self, _ctx: &egui::Context) {
        self.select_panel(2);
    }

    pub(super) fn minimum_window_size(&self) -> egui::Vec2 {
        let layers = self.sprite.is_some() && self.show_layers;
        let remaining = 160.0 + if layers { 160.0 } else { 0.0 };
        let width = if self.conversation.visible {
            (260.0_f32 + remaining).max(520.0).max(remaining * 2.0)
        } else if layers {
            320.0
        } else {
            280.0
        };
        let height = if self.conversation.visible && self.conversation.has_pending_attachments() {
            520.0
        } else if self.conversation.visible {
            if self.activity.is_some() {
                420.0
            } else {
                380.0
            }
        } else if self.sprite.is_some() && self.show_animation {
            360.0
        } else {
            260.0
        };
        let panel_minimum = if self.show_layers || self.show_animation || self.conversation.visible
        {
            theme::metric("rail_width")
                + theme::metric("sidebar_min_width")
                + theme::metric("canvas_min_width")
                + 3.0 * theme::metric("gap")
        } else {
            0.0
        };
        egui::vec2(width.max(panel_minimum), height)
    }
    pub(super) fn enforce_window_minimum(&self, ctx: &egui::Context) {
        let minimum = self.minimum_window_size();
        let changed = ctx.data_mut(|d| {
            let id = egui::Id::new("window-minimum");
            let changed = d.get_temp::<egui::Vec2>(id) != Some(minimum);
            d.insert_temp(id, minimum);
            changed
        });
        if changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(minimum));
        }
        if let Some(rect) = ctx.input(|i| i.viewport().inner_rect)
            && (rect.width() < minimum.x || rect.height() < minimum.y)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(rect.size().max(minimum)));
        }
    }

    pub(super) fn canvas_dialog(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("canvas-dialog");
        let Some((create, mut width, mut height)) =
            ctx.data(|d| d.get_temp::<(bool, u32, u32)>(id))
        else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(if create { "New artwork" } else { "Canvas size" })
            .collapsible(false)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(16))
            .resizable(false)
            .default_width(340.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(16.0,16.0);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_sized([76.0,32.0],egui::Label::new("Width"));
                    ui.add_sized([132.0,32.0],
                        egui::DragValue::new(&mut width)
                            .range(1..=4096)
                            .suffix(" px"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_sized([76.0,32.0],egui::Label::new("Height"));
                    ui.add_sized([132.0,32.0],
                        egui::DragValue::new(&mut height)
                            .range(1..=4096)
                            .suffix(" px"),
                    );
                });
                if !create {
                    ui.add(egui::Label::new(egui::RichText::new("Preview updates as you edit. Pixels stay at the top left; smaller bounds crop the artwork when applied.").small().weak()).wrap());
                }
                if create && self.sprite.is_some() {
                    ui.label("Save your current artwork before creating another.");
                }
                let valid_size = crate::schema::pixel_bytes(width, height).is_ok();
                if !valid_size {
                    ui.label("Canvas must contain at most 4,194,304 pixels.");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    apply = ui.add_enabled(self.conversation.can_save() && valid_size,
                        egui::Button::new(if create { "Create artwork" } else { "Resize canvas" })).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
                ui.add_space(8.0);
            });
        if cancel {
            open = false;
        }
        let preview =
            if !create && open && !apply && crate::schema::pixel_bytes(width, height).is_ok() {
                Some((width, height))
            } else {
                None
            };
        if self.canvas_preview != preview {
            self.canvas_preview = preview;
            self.rendered = None;
        }
        if apply {
            self.conversation
                .edit_canvas(ctx, self.source.as_deref(), width, height, create);
            if create {
                self.activity = None;
                self.activity_from_embedded = false;
                self.opened_source = None;
            }
        }
        ctx.data_mut(|data| {
            if !open || apply {
                data.remove::<(bool, u32, u32)>(id);
            } else {
                data.insert_temp(id, (create, width, height));
            }
        });
    }

    pub(super) fn title_bar(&mut self, ctx: &egui::Context) {
        let title = self.window_title();
        let changed = ctx.data_mut(|data| {
            let id = egui::Id::new("window-title");
            if data.get_temp::<String>(id).as_deref() == Some(&title) {
                false
            } else {
                data.insert_temp(id, title.clone());
                true
            }
        });
        if changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        }
        egui::TopBottomPanel::top("window-title-bar")
            .exact_height(theme::metric("bar_height"))
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(theme::color("shell"))
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let side = 96.0_f32.min(rect.width() * 0.36);
                let center = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + side, rect.top()),
                    egui::pos2(rect.right() - side, rect.bottom()),
                );
                let drag = ui
                    .put(
                        center,
                        egui::Label::new(egui::RichText::new(&title).size(13.0))
                            .truncate()
                            .halign(egui::Align::Center)
                            .selectable(false)
                            .sense(egui::Sense::click_and_drag()),
                    )
                    .on_hover_cursor(egui::CursorIcon::Default);
                if let Some(path) = self
                    .conversation
                    .saved_path()
                    .or(self.opened_source.as_deref())
                {
                    let text_width = ui
                        .painter()
                        .layout_no_wrap(
                            title.clone(),
                            egui::FontId::proportional(13.0),
                            ui.visuals().text_color(),
                        )
                        .size()
                        .x
                        .min(center.width());
                    let text_rect = egui::Rect::from_center_size(
                        center.center(),
                        egui::vec2(text_width, center.height()),
                    );
                    if ctx
                        .pointer_hover_pos()
                        .is_some_and(|p| text_rect.contains(p))
                    {
                        let mut tooltip = egui::Tooltip::for_enabled(&drag);
                        tooltip.popup = tooltip
                            .popup
                            .anchor(text_rect)
                            .align(egui::RectAlign::BOTTOM);
                        tooltip.show(|ui| {
                            ui.label(path.display().to_string());
                        });
                    }
                }
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                        !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                    ));
                } else if drag.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_size(rect.min, egui::vec2(68.0, 28.0)))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                        ui.visuals_mut().widgets.hovered.bg_fill = theme::color("hover");
                        ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                        ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                        let (logo_rect, _) =
                            ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                        let logo_id = egui::Id::new("application-logo");
                        let texture = ctx
                            .data(|d| d.get_temp::<egui::TextureHandle>(logo_id))
                            .or_else(|| {
                                let image = image::load_from_memory(include_bytes!(
                                    "../assets/pixloom-20.png"
                                ))
                                .ok()?
                                .to_rgba8();
                                let texture = ctx.load_texture(
                                    "application-logo",
                                    egui::ColorImage::from_rgba_unmultiplied(
                                        [image.width() as usize, image.height() as usize],
                                        image.as_raw(),
                                    ),
                                    egui::TextureOptions::NEAREST,
                                );
                                ctx.data_mut(|d| d.insert_temp(logo_id, texture.clone()));
                                Some(texture)
                            });
                        if let Some(texture) = texture {
                            ui.painter().image(
                                texture.id(),
                                egui::Rect::from_center_size(
                                    logo_rect.center(),
                                    egui::Vec2::splat(theme::metric("app_icon_size")),
                                ),
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        }
                        self.main_menu(ui, ctx);
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_max(
                            egui::pos2(rect.right() - 90.0, rect.top()),
                            rect.max,
                        ))
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        if window_button(ui, WindowButton::Close).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        if window_button(
                            ui,
                            if maximized {
                                WindowButton::Restore
                            } else {
                                WindowButton::Maximize
                            },
                        )
                        .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if window_button(ui, WindowButton::Minimize).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    },
                );
            });
    }
}

#[derive(Clone, Copy)]
enum WindowButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}
fn window_button(ui: &mut egui::Ui, button: WindowButton) -> egui::Response {
    let label = match button {
        WindowButton::Minimize => "Minimize",
        WindowButton::Maximize => "Maximize",
        WindowButton::Restore => "Restore",
        WindowButton::Close => "Close",
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, 28.0), egui::Sense::click());
    #[cfg(test)]
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new(label), rect));
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            4,
            if matches!(button, WindowButton::Close) {
                theme::color("close_hover")
            } else {
                theme::color("hover")
            },
        );
    }
    let color = if response.hovered() && matches!(button, WindowButton::Close) {
        egui::Color32::WHITE
    } else {
        ui.visuals().text_color()
    };
    let name = match button {
        WindowButton::Minimize => "minimize",
        WindowButton::Maximize => "maximize",
        WindowButton::Restore => "restore",
        WindowButton::Close => "close",
    };
    icons::paint(
        ui,
        name,
        egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
        color,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.on_hover_text(label)
}

pub(super) fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let hover = ctx.pointer_hover_pos();
    let rect = ctx.content_rect();
    let edge = 7.0;
    let grip = egui::Rect::from_min_max(rect.max - egui::vec2(20.0, 20.0), rect.max);
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("native-resize-grip"),
    ));
    for offset in [5.0, 9.0, 13.0] {
        painter.line_segment(
            [
                rect.max - egui::vec2(offset, 3.0),
                rect.max - egui::vec2(3.0, offset),
            ],
            egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_gray(if hover.is_some_and(|pos| grip.contains(pos)) {
                    180
                } else {
                    85
                }),
            ),
        );
    }
    let Some(pos) = hover else {
        return;
    };
    let left = pos.x < rect.left() + edge;
    let right = pos.x > rect.right() - edge || grip.contains(pos);
    let top = pos.y < rect.top() + 4.0;
    let bottom = pos.y > rect.bottom() - edge || grip.contains(pos);
    use egui::ResizeDirection::*;
    let direction = match (left, right, top, bottom) {
        (true, _, true, _) => Some((NorthWest, egui::CursorIcon::ResizeNwSe)),
        (_, true, true, _) => Some((NorthEast, egui::CursorIcon::ResizeNeSw)),
        (true, _, _, true) => Some((SouthWest, egui::CursorIcon::ResizeNeSw)),
        (_, true, _, true) => Some((SouthEast, egui::CursorIcon::ResizeNwSe)),
        (true, _, _, _) => Some((West, egui::CursorIcon::ResizeHorizontal)),
        (_, true, _, _) => Some((East, egui::CursorIcon::ResizeHorizontal)),
        (_, _, true, _) => Some((North, egui::CursorIcon::ResizeVertical)),
        (_, _, _, true) => Some((South, egui::CursorIcon::ResizeVertical)),
        _ => None,
    };
    if let Some((direction, cursor)) = direction {
        ctx.set_cursor_icon(cursor);
        let stroke = egui::Stroke::new(2.0_f32, theme::color("resize_grip"));
        if left {
            painter.line_segment([rect.left_top(), rect.left_bottom()], stroke);
        }
        if right {
            painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
        }
        if top {
            painter.line_segment([rect.left_top(), rect.right_top()], stroke);
        }
        if bottom {
            painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
        }
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let (_, rx) = mpsc::channel();
        App::empty(rx)
    }
    #[test]
    fn renderer_clear_color_matches_shell_without_linear_conversion() {
        use eframe::App as _;
        let app = app();
        assert_eq!(
            app.clear_color(&egui::Visuals::dark()),
            theme::color("shell").to_normalized_gamma_f32()
        );
    }
    #[test]
    fn panel_switching_is_exclusive_and_toggling_preserves_playback() {
        let mut app = app();
        app.playback.frame = 3;
        app.speed = 2.0;
        for index in [1, 2, 0, 2, 1] {
            app.select_panel(index);
            assert_eq!(
                [
                    app.show_layers,
                    app.show_animation,
                    app.conversation.visible
                ]
                .into_iter()
                .filter(|v| *v)
                .count(),
                1
            );
            assert_eq!(app.playback.frame, 3);
            assert_eq!(app.speed, 2.0);
            assert!(!app.conversation.active());
        }
        app.select_panel(1);
        assert!(!app.show_layers && !app.show_animation && !app.conversation.visible);
    }
    #[test]
    fn title_and_status_bars_share_theme_height() {
        let mut app = app();
        let ctx = egui::Context::default();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| {
                app.title_bar(ctx);
                assert_eq!(ctx.available_rect().top(), theme::metric("bar_height"));
                app.footer(ctx);
                assert_eq!(
                    600.0 - ctx.available_rect().bottom(),
                    theme::metric("bar_height")
                );
            },
        );
    }
    #[test]
    fn chat_toggles_inward_without_native_window_changes_or_starting_ai() {
        let mut app = app();
        let ctx = egui::Context::default();
        for visible in [true, false] {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                app.toggle_conversation(ctx)
            });
            assert_eq!(app.conversation.visible, visible);
            assert!(!app.conversation.active());
            assert!(
                !output
                    .viewport_output
                    .values()
                    .flat_map(|v| &v.commands)
                    .any(|c| matches!(
                        c,
                        egui::ViewportCommand::OuterPosition(_)
                            | egui::ViewportCommand::InnerSize(_)
                    ))
            );
        }
    }

    #[test]
    fn hovering_application_title_keeps_normal_cursor_and_has_no_tooltip() {
        let mut app = app();
        let ctx = egui::Context::default();
        render(&mut app, &ctx, vec![]);
        for time in [1.0, 4.0] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 500.0),
                    )),
                    time: Some(time),
                    events: vec![egui::Event::PointerMoved(egui::pos2(400.0, 18.0))],
                    ..Default::default()
                },
                |ctx| app.title_bar(ctx),
            );
            assert_eq!(
                output.platform_output.cursor_icon,
                egui::CursorIcon::Default
            );
            assert_eq!(output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == "Pixloom")).count(), 1);
        }
    }

    #[test]
    fn file_tooltip_contains_full_path_below_centered_title() {
        let mut app = app();
        app.opened_source = Some(PathBuf::from("C:/art/sword.pixloom"));
        let ctx = egui::Context::default();
        render(&mut app, &ctx, vec![]);
        let mut found = false;
        for time in [1.0, 2.0, 3.0] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 500.0),
                    )),
                    time: Some(time),
                    events: if time == 1.0 {
                        vec![egui::Event::PointerMoved(egui::pos2(400.0, 18.0))]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ctx| app.title_bar(ctx),
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == "C:/art/sword.pixloom"
                {
                    assert!(text.pos.y > 32.0);
                    assert!((text.pos.x + text.galley.rect.center().x - 400.0).abs() < 8.0);
                    found = true;
                }
            }
        }
        assert!(found, "full file path should appear on title hover");
    }
    fn render(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 500.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.title_bar(ctx),
        )
    }
    #[test]
    fn title_is_centered_and_tracks_loaded_filename() {
        let mut app = app();
        let ctx = egui::Context::default();
        for (source, expected) in [
            (None, "Pixloom"),
            (Some(PathBuf::from("C:/art/sword.pixloom")), "sword.pixloom"),
        ] {
            app.opened_source = source;
            let output = render(&mut app, &ctx, vec![]);
            let title = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == expected => Some(t),
                    _ => None,
                })
                .unwrap();
            assert!((title.pos.x + title.galley.rect.center().x - 400.0).abs() < 1.0);
        }
        assert!(!app.conversation.active());
    }
    #[test]
    fn caption_buttons_send_native_window_commands() {
        let mut app = app();
        let ctx = egui::Context::default();
        render(&mut app, &ctx, vec![]);
        for label in ["Minimize", "Maximize", "Close"] {
            let point = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(label)))
                .unwrap()
                .center();
            let mut all = Vec::new();
            for pressed in [true, false] {
                let output = render(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
                all.extend(
                    output
                        .viewport_output
                        .values()
                        .flat_map(|out| out.commands.clone()),
                );
            }
            assert!(all.iter().any(|command| matches!(
                (label, command),
                ("Minimize", egui::ViewportCommand::Minimized(true))
                    | ("Maximize", egui::ViewportCommand::Maximized(true))
                    | ("Close", egui::ViewportCommand::Close)
            )));
        }
    }

    #[test]
    fn title_drag_emits_a_native_drag_command() {
        let mut app = app();
        let ctx = egui::Context::default();
        render(&mut app, &ctx, vec![]);
        let point = egui::pos2(400.0, 18.0);
        render(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let output = render(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point + egui::vec2(40.0, 0.0))],
        );
        assert!(output.viewport_output.values().any(|out| {
            out.commands
                .iter()
                .any(|cmd| matches!(cmd, egui::ViewportCommand::StartDrag))
        }));
    }
}

#[cfg(test)]
mod resize_tests {
    use super::*;
    #[test]
    fn corner_grip_starts_native_resize_from_twenty_pixel_target() {
        let ctx = egui::Context::default();
        let pos = egui::pos2(785.0, 485.0);
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 500.0),
                )),
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            resize_edges,
        );
        assert_eq!(
            out.platform_output.cursor_icon,
            egui::CursorIcon::ResizeNwSe
        );
        assert!(
            out.viewport_output
                .values()
                .any(|v| v.commands.iter().any(|c| matches!(
                    c,
                    egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast)
                )))
        );
        assert!(
            out.shapes.len() >= 5,
            "Grip and hovered edges should be painted"
        );
    }
}
