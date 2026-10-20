use super::*;

impl Conversation {
    pub(super) fn show_panel(&mut self, ctx: &egui::Context, source: Option<&Path>) {
        let _panel = egui::SidePanel::left("art-conversation")
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(32)).inner_margin(8.0))
            .default_width(340.0).min_width(260.0)
            .max_width((ctx.content_rect().width()*0.5).max(260.0)).resizable(true)
            .show(ctx, |ui| {
                super::super::panel_header(ui, |ui| {
                    ui.heading("Art conversation").on_hover_text(&self.account);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(!self.busy, |ui| {
                            if super::super::icon_button(ui,super::super::Icon::NewChat,"New conversation").clicked() {
                                self.messages.clear(); self.checkpoints.clear(); self.thoughts.clear();
                                self.pending_prompt=None;
                                if self.ready {self.ready=false;self.action(Action::NewConversation);}
                            }
                        });
                    });
                });
                ui.separator();
                egui::TopBottomPanel::bottom("art-composer")
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4,8)))
                    .show_inside(ui, |ui| self.composer(ui,source));
                let approval_height = (ui.available_height() - 78.0).clamp(8.0, 140.0);
                let mut body=egui::ScrollArea::vertical().id_salt("art-body")
                    .auto_shrink([false,false]).stick_to_bottom(self.approvals.is_empty());
                if self.scroll_to_top { body=body.vertical_scroll_offset(0.0); self.scroll_to_top=false; }
                body.show(ui, |ui| {
                    if let Some((id,description))=self.approvals.first().cloned() {
                        ui.group(|ui| {
                            ui.label(egui::RichText::new("Codex requests approval").color(super::super::SYSTEM_TEXT));
                            egui::ScrollArea::vertical().id_salt("approval")
                                .min_scrolled_height(8.0).max_height(approval_height)
                                .show(ui,|ui| { ui.label(egui::RichText::new(description).monospace().color(super::super::SYSTEM_TEXT)); });
                            ui.horizontal(|ui| {
                                if ui.button("Allow once").clicked() { self.action(Action::Approval(id.clone(),true)); }
                                if ui.button("Decline").clicked() { self.action(Action::Approval(id.clone(),false)); }
                            });
                        });
                    }
                    if self.messages.is_empty() && !self.busy {
                        let available = ui.available_rect_before_wrap();
                        ui.scope_builder(egui::UiBuilder::new().max_rect(available), |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.add_space((available.height() * 0.5 - 34.0).max(0.0));
                                ui.label("What would you like to create?");
                                ui.add(egui::Label::new(egui::RichText::new("Describe your artwork below. Codex starts with your first message.").small().weak()).wrap());
                            });
                        });
                    }
                    let mut rewind=None;
                    for (index,message) in self.messages.iter().enumerate() {
                        thoughts_at(ui,&self.thoughts,index);
                        let artist=message.author=="Artist";
                        let full_width=ui.available_width();
                        ui.add_space(10.0);
                        ui.push_id(&message.id,|ui| {
                            let row=ui.vertical(|ui| {
                                ui.set_min_width(full_width);
                                if artist {
                                    ui.scope(|ui| {
                                        ui.set_max_width((full_width-12.0).max(1.0));
                                        ui.add(egui::Label::new(&message.text).wrap().selectable(true));
                                    });
                                } else {
                                    ui.with_layout(egui::Layout::top_down(egui::Align::Max),|ui| {
                                        egui::Frame::NONE.fill(egui::Color32::from_gray(43)).corner_radius(6).inner_margin(10).show(ui,|ui| {
                                            ui.set_max_width((full_width-44.0).max(1.0));
                                            if !message.attachments.is_empty() {attachment_cards(ui,&message.attachments,false);ui.add_space(6.0);}
                                            if !message.text.is_empty() {ui.add(egui::Label::new(&message.text).wrap().selectable(true));}
                                        });
                                    });
                                }
                                let content=ui.min_rect();
                                let hovered=ui.ctx().pointer_hover_pos().is_some_and(|p|egui::Rect::from_min_max(content.min,content.max+egui::vec2(0.0,25.0)).contains(p));
                                ui.add_space(3.0);
                                ui.allocate_ui_with_layout(egui::vec2(full_width,22.0),if artist {egui::Layout::left_to_right(egui::Align::Center)} else {egui::Layout::right_to_left(egui::Align::Center)},|ui| {
                                    if !hovered {ui.set_opacity(0.0);ui.disable();}
                                    ui.label(egui::RichText::new(message.sent_at.format("%H:%M").to_string()).small().color(super::super::SYSTEM_TEXT));
                                    if artist {
                                        if message_tool(ui,"copy","Copy message").clicked() {ui.ctx().copy_text(message.text.clone());}
                                    } else if let Some(token)=self.checkpoints.get(&message.id) {
                                        ui.add_enabled_ui(!self.busy && self.ready && !self.failed,|ui| {
                                            if message_tool(ui,"revert","Revert to this message and edit it").clicked() {rewind=Some(*token);}
                                        });
                                    }
                                });
                            });
                            #[cfg(test)] ui.ctx().data_mut(|d|d.insert_temp(egui::Id::new(("message-test-rect",&message.id)),row.response.rect));
                            let _=row;
                        });
                    }
                    thoughts_at(ui,&self.thoughts,self.messages.len());
                    if let Some(token)=rewind {self.rewind_message(token);}


                });
            });
        #[cfg(test)]
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("conversation-panel-test-rect"),
                _panel.response.rect,
            )
        });
    }

    fn composer(&mut self, ui: &mut egui::Ui, source: Option<&Path>) {
        let can_send = !self.busy
            && self.save_picker.is_none()
            && self.document_events.is_none()
            && self.attachment_picker.is_none();
        let has_content = !self.draft.trim().is_empty() || !self.attachments.is_empty();
        let mut submit = false;
        let focused = ui
            .ctx()
            .memory(|m| m.has_focus(egui::Id::new("art-prompt")));
        egui::Frame::NONE.fill(egui::Color32::from_gray(24))
            .stroke(egui::Stroke::new(1.0_f32,egui::Color32::from_gray(if focused {110} else {65})))
            .corner_radius(6).inner_margin(12).show(ui, |ui| {
                if !self.attachments.is_empty() {
                    if let Some(index)=attachment_cards(ui,&self.attachments,!self.busy) { self.attachments.remove(index); }
                    ui.add_space(8.0);
                }
                let response=egui::ScrollArea::vertical().id_salt("prompt-scroll").max_height((ui.ctx().content_rect().height()*0.22).clamp(24.0,140.0)).stick_to_bottom(true).show(ui, |ui| {
                    ui.add_enabled(can_send,egui::TextEdit::multiline(&mut self.draft)
                        .id(egui::Id::new("art-prompt")).frame(false).desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .return_key(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT,egui::Key::Enter))
                        .hint_text("Describe artwork or ask for a change…"))
                }).inner;
                if response.has_focus() && can_send {
                    submit=ui.input(|i| i.events.iter().any(|event| matches!(event,egui::Event::Key { key:egui::Key::Enter,pressed:true,modifiers,.. } if !modifiers.shift && !modifiers.ctrl && !modifiers.alt && !modifiers.command)));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(can_send,|ui| {
                        ui.visuals_mut().widgets.inactive.bg_fill=egui::Color32::TRANSPARENT;
                        ui.visuals_mut().widgets.inactive.bg_stroke=egui::Stroke::NONE;
                        ui.visuals_mut().widgets.inactive.bg_fill=egui::Color32::TRANSPARENT;
                        ui.visuals_mut().widgets.inactive.bg_stroke=egui::Stroke::NONE;
                        ui.visuals_mut().widgets.hovered.bg_fill=egui::Color32::from_gray(55);
                        ui.visuals_mut().widgets.hovered.bg_stroke=egui::Stroke::NONE;
                        ui.visuals_mut().widgets.active.bg_stroke=egui::Stroke::NONE;
                        let attach=super::super::icon_button(ui,super::super::Icon::Attach,"Attach reference files");
                        #[cfg(test)] ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("attach-button-test-rect"),attach.rect));
                        if attach.clicked() { self.pick_attachments(ui.ctx()); }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let stopping=self.busy && self.agent.is_some();
                        let response=send_button(ui,stopping,stopping || (can_send && has_content));
                        if response.clicked() {
                            if stopping {
                                if self.pending_prompt.take().is_some() { self.pending_attachments.clear(); self.busy=false; self.status="Message cancelled".into(); }
                                else { self.action(Action::Stop); }
                            } else { submit=true; }
                        }
                    });
                });
            });
        if submit && can_send && (!self.draft.trim().is_empty() || !self.attachments.is_empty()) {
            self.submit_draft(ui.ctx(), source);
        }
    }
}

fn attachment_cards(
    ui: &mut egui::Ui,
    attachments: &[Attachment],
    removable: bool,
) -> Option<usize> {
    let mut remove = None;
    egui::ScrollArea::horizontal()
        .id_salt(("attachment-cards", ui.id()))
        .max_height(96.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, file) in attachments.iter().enumerate() {
                    ui.push_id(&file.path, |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(96.0, 88.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 4, egui::Color32::from_gray(38));
                        let preview = egui::Rect::from_min_max(
                            rect.min + egui::vec2(6.0, 6.0),
                            rect.max - egui::vec2(6.0, 22.0),
                        );
                        if let Some(texture) = file.texture(ui.ctx()) {
                            let size = texture.size_vec2();
                            let factor = (preview.width() / size.x).min(preview.height() / size.y);
                            ui.painter().image(
                                texture.id(),
                                egui::Rect::from_center_size(preview.center(), size * factor),
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        } else {
                            super::super::icons::paint(
                                ui,
                                "file",
                                egui::Rect::from_center_size(
                                    preview.center(),
                                    egui::vec2(28.0, 28.0),
                                ),
                                ui.visuals().text_color(),
                            );
                        }
                        let label = egui::Rect::from_min_max(
                            egui::pos2(rect.left() + 6.0, rect.bottom() - 19.0),
                            rect.max - egui::vec2(6.0, 2.0),
                        );
                        ui.put(
                            label,
                            egui::Label::new(egui::RichText::new(&file.name).small()).truncate(),
                        );
                        if removable {
                            let close = egui::Rect::from_min_size(
                                rect.right_top() - egui::vec2(20.0, -2.0),
                                egui::vec2(18.0, 18.0),
                            );
                            #[cfg(test)]
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(
                                    egui::Id::new(("remove-reference-test-rect", index)),
                                    close,
                                )
                            });
                            let response = ui
                                .interact(close, ui.id().with("remove"), egui::Sense::click())
                                .on_hover_text("Remove attachment");
                            super::super::icons::paint(
                                ui,
                                "close",
                                close,
                                ui.style().interact(&response).fg_stroke.color,
                            );
                            if response.clicked() {
                                remove = Some(index);
                            }
                        }
                        response.on_hover_text(&file.name);
                    });
                }
            });
        });
    remove
}

fn send_button(ui: &mut egui::Ui, stopping: bool, enabled: bool) -> egui::Response {
    let label = if stopping { "Stop" } else { "Send" };
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
        #[cfg(test)]
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new("composer-action-rect"), rect));
        let visuals = ui.style().interact(&response);
        let fill = if enabled {
            egui::Color32::from_gray(if response.hovered() { 240 } else { 218 })
        } else {
            visuals.weak_bg_fill
        };
        ui.painter().rect_filled(rect, 4, fill);
        let color = if enabled {
            egui::Color32::from_gray(28)
        } else {
            visuals.fg_stroke.color
        };
        super::super::icons::paint(
            ui,
            if stopping { "stop" } else { "send" },
            rect.shrink(2.0),
            color,
        );
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        response.on_hover_text(if stopping {
            "Stop AI turn"
        } else {
            "Send message (Enter)"
        })
    })
    .inner
}

fn message_tool(ui: &mut egui::Ui, name: &str, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4, egui::Color32::from_gray(55));
    }
    super::super::icons::paint(ui, name, rect.shrink(3.0), super::super::SYSTEM_TEXT);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("message-tool-test", ui.id(), name)), rect));
    response.on_hover_text(label)
}
fn thoughts_at(ui: &mut egui::Ui, thoughts: &[Thought], position: usize) {
    for thought in thoughts.iter().filter(|t| t.position == position) {
        let elapsed = thought
            .elapsed
            .unwrap_or_else(|| thought.started.elapsed())
            .as_secs();
        if thought.elapsed.is_none() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
        egui::CollapsingHeader::new(format!(
            "{} · {}s",
            if thought.elapsed.is_some() {
                "Thought"
            } else {
                "Thinking"
            },
            elapsed
        ))
        .id_salt(("reasoning-summary", &thought.id))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(if thought.text.is_empty() {
                    "No reasoning summary provided yet."
                } else {
                    &thought.text
                })
                .small()
                .color(super::super::SYSTEM_TEXT),
            );
        });
    }
}
