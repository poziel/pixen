use super::*;

const PAGES: [(&str, &str); 3] = [
    ("Playback", "speed animation preview timing"),
    ("Workspace", "panels layers animation visibility interface"),
    (
        "AI conversation",
        "chatgpt codex models thinking reasoning low medium high xhigh max ultra gpt 6.1 6 5.6 sol astra luna terra",
    ),
];

fn matches(page: usize, query: &str) -> bool {
    let haystack = format!("{} {}", PAGES[page].0, PAGES[page].1).to_lowercase();
    query.split_whitespace().all(|word| haystack.contains(word))
}
fn effort_label(effort: &str) -> &str {
    match effort {
        "low" => "Low",
        "medium" => "Medium",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Maximum",
        "ultra" => "Ultra",
        other => other,
    }
}

impl App {
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("app-settings");
        let mut open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        if !open {
            return;
        }
        let state_id = egui::Id::new("settings-navigation");
        let (mut search, mut page) = ctx
            .data(|d| d.get_temp::<(String, usize)>(state_id))
            .unwrap_or_default();
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(16))
            .default_size(egui::vec2(640.0, 440.0))
            .min_size(egui::vec2(260.0, 300.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut search)
                        .hint_text("Search settings…")
                        .desired_width(f32::INFINITY),
                );
                ui.separator();
                let query = search.trim().to_lowercase();
                let height = ui.available_height().max(280.0);
                let sidebar_width = (ui.available_width() * 0.25).clamp(88.0, 150.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(sidebar_width, height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            for (index, (label, _)) in PAGES.iter().enumerate() {
                                if matches(index, &query)
                                    && navigation_item(ui, sidebar_width, page == index, label)
                                        .clicked()
                                {
                                    page = index;
                                    search.clear();
                                }
                            }
                        },
                    );
                    ui.separator();
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings-content")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width().max(100.0));
                                let mut any = false;
                                for (index, (heading, _)) in PAGES.iter().enumerate() {
                                    if (query.is_empty() && index != page)
                                        || !matches(index, &query)
                                    {
                                        continue;
                                    }
                                    any = true;
                                    ui.heading(*heading);
                                    ui.add_space(10.0);
                                    match index {
                                        0 => self.playback_settings(ui),
                                        1 => {
                                            ui.checkbox(&mut self.show_layers, "Show layers");
                                            ui.checkbox(&mut self.show_animation, "Show animation");
                                        }
                                        _ => self.artist_settings(ui),
                                    }
                                    ui.add_space(24.0);
                                }
                                if !any {
                                    ui.label("No settings match your search.");
                                }
                            });
                    });
                });
            });
        ctx.data_mut(|d| {
            d.insert_temp(id, open);
            d.insert_temp(state_id, (search, page));
        });
    }

    fn playback_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Preview speed");
        ui.add_enabled_ui(!self.following(), |ui| {
            ui.add(
                egui::Slider::new(&mut self.speed, 0.125..=8.0)
                    .logarithmic(true)
                    .suffix("×"),
            );
            ui.horizontal_wrapped(|ui| {
                for speed in [0.25, 0.5, 1.0, 2.0, 4.0] {
                    if ui
                        .selectable_label(self.speed == speed, format!("{speed}×"))
                        .clicked()
                    {
                        self.speed = speed;
                    }
                }
            });
        });
        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new("Saved frame timing stays unchanged.")
                    .small()
                    .weak(),
            )
            .wrap(),
        );
    }

    fn artist_settings(&mut self, ui: &mut egui::Ui) {
        let (mut selection, catalog) = self.conversation.model_settings();
        let previous = selection.clone();
        ui.label("ChatGPT model");
        ui.add_enabled_ui(!self.conversation.busy, |ui| {
            egui::ComboBox::from_id_salt("artist-model")
                .width(ui.available_width().min(300.0))
                .selected_text(
                    catalog
                        .iter()
                        .find(|m| m.id == selection.model)
                        .map(|m| m.label.as_str())
                        .unwrap_or(&selection.model),
                )
                .show_ui(ui, |ui| {
                    for model in &catalog {
                        ui.selectable_value(&mut selection.model, model.id.clone(), &model.label);
                    }
                });
            if let Some(model) = catalog.iter().find(|m| m.id == selection.model) {
                if !model.efforts.contains(&selection.effort) {
                    selection.effort = model.default_effort.clone();
                }
                ui.add_space(12.0);
                ui.label("Thinking level");
                egui::ComboBox::from_id_salt("artist-effort")
                    .width(ui.available_width().min(300.0))
                    .selected_text(effort_label(&selection.effort))
                    .show_ui(ui, |ui| {
                        for effort in &model.efforts {
                            ui.selectable_value(
                                &mut selection.effort,
                                effort.clone(),
                                effort_label(effort),
                            );
                        }
                    });
            }
        });
        ui.add_space(12.0);
        ui.add(egui::Label::new(egui::RichText::new("Uses your Codex sign-in. Changes apply to the next message and are remembered.").small().weak()).wrap());
        if self.conversation.busy {
            ui.label(
                egui::RichText::new(
                    "Wait for the current operation to finish before changing models.",
                )
                .small()
                .weak(),
            );
        }
        if previous != selection {
            self.conversation.set_model_settings(selection);
        }
    }
}

fn navigation_item(ui: &mut egui::Ui, width: f32, selected: bool, label: &str) -> egui::Response {
    let response = ui.add_sized([width, 30.0], egui::Button::selectable(selected, ""));
    ui.painter().text(
        response.rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        ui.style()
            .interact_selectable(&response, selected)
            .text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    response
}

#[cfg(test)]
mod navigation_tests {
    use super::*;
    #[test]
    fn tab_labels_share_left_inset() {
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for label in ["Playback", "Workspace", "AI conversation"] {
                    navigation_item(ui, 150.0, false, label);
                }
            });
        });
        let positions: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if ["Playback", "Workspace", "AI conversation"].contains(&t.galley.text()) =>
                {
                    Some(t.pos.x)
                }
                _ => None,
            })
            .collect();
        assert_eq!(positions.len(), 3);
        assert!(positions.iter().all(|x| (*x - positions[0]).abs() < 0.1));
    }
}
