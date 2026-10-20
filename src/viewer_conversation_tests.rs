use super::*;

#[test]
fn composer_shows_reference_cards_and_removes_them_without_starting_ai() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "Use blue shading").unwrap();
    let mut conversation = Conversation {
        visible: true,
        attachments: vec![Attachment::load(&path).unwrap()],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render_size(&mut conversation, &ctx, 800.0, 600.0);
    let output = render_size(&mut conversation, &ctx, 800.0, 600.0);
    assert!(
        output
            .shapes
            .iter()
            .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="notes.txt"))
    );
    let attach = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("attach-button-test-rect")))
        .unwrap();
    let send = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("composer-action-rect")))
        .unwrap();
    assert!(attach.center().x < send.center().x);
    assert!((attach.center().y - send.center().y).abs() < 1.0);
    let close = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("remove-reference-test-rect", 0_usize))))
        .unwrap()
        .center();
    for pressed in [true, false] {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events: vec![
                    egui::Event::PointerMoved(close),
                    egui::Event::PointerButton {
                        pos: close,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                conversation.panel(ctx, None);
            },
        );
    }
    assert!(conversation.attachments.is_empty());
    assert!(!conversation.active());
}

#[test]
fn short_bubbles_fit_content_and_times_are_hidden_by_default() {
    let now = chrono::Local::now();
    let message = |id: &str, minutes, text: &str| Message {
        id: id.into(),
        author: "You",
        sent_at: now + chrono::Duration::minutes(minutes),
        attachments: Vec::new(),
        text: text.into(),
    };
    let mut conversation = Conversation {
        visible: true,
        messages: vec![
            message("1", 0, "Hi"),
            message("2", 2, "Another request"),
            message("3", 4, "A third request"),
            message("4", 8, "Later request"),
        ],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render(&mut conversation, &ctx);
    let output = render(&mut conversation, &ctx);
    let text = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Hi" => Some(t),
            _ => None,
        })
        .unwrap();
    let bubble = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Rect(r)
                if r.fill == egui::Color32::from_gray(43) && r.rect.contains(text.pos) =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .unwrap();
    assert!(
        bubble.width() < 80.0,
        "short message should hug content: {bubble:?}"
    );
    assert_eq!(output.shapes.iter().filter(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==now.format("%H:%M").to_string())).count(),0);
}

#[test]
fn reference_only_message_survives_connection_queue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("brief.txt");
    std::fs::write(&path, "Use blue shading").unwrap();
    let (actions, commands) = mpsc::channel();
    let (events, receiver) = mpsc::channel();
    let mut conversation = Conversation {
        attachments: vec![Attachment::load(&path).unwrap()],
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    conversation.submit_draft(&ctx, None);
    assert!(conversation.attachments.is_empty());
    assert_eq!(conversation.messages[0].attachments.len(), 1);
    assert!(commands.try_recv().is_err());
    events.send(Event::Ready).unwrap();
    conversation.poll_events(&ctx, None);
    let Action::SendReferences(_, files) = commands.try_recv().unwrap() else {
        panic!("references missing")
    };
    assert!(
        files[0].input()["text"]
            .as_str()
            .unwrap()
            .contains("Use blue shading")
    );
    assert!(conversation.pending_attachments.is_empty());
}

#[test]
fn embedded_activity_uses_follow_bar_and_leaves_footer_for_application_status() {
    let (_, messages) = mpsc::channel();
    let mut app = super::super::App::empty(messages);
    let (actions, _commands) = mpsc::channel();
    let (events, receiver) = mpsc::channel();
    app.conversation.agent = Some(Agent {
        actions,
        events: receiver,
        canvas: Default::default(),
    });
    app.conversation.ready = true;
    events.send(Event::Working).unwrap();
    events
        .send(Event::Viewer(crate::mcp::desktop::DesktopEvent::Activity(
            super::super::Activity {
                label: "Drawing the slime".into(),
                busy: true,
                frame_id: Some("frame-1".into()),
                layer_id: Some("layer-1".into()),
            },
        )))
        .unwrap();
    let ctx = egui::Context::default();
    app.update_conversation(&ctx, false);
    assert_eq!(app.activity.as_ref().unwrap().label, "Drawing the slime");
    assert!(app.following());
    assert_eq!(app.status_message(std::time::Instant::now()).0, "Ready");
    events.send(Event::Finished).unwrap();
    app.update_conversation(&ctx, false);
    assert!(!app.activity.as_ref().unwrap().busy);
    assert!(!app.following());
}

#[test]
fn message_times_are_below_messages_and_align_to_the_correct_side() {
    use chrono::TimeZone;
    let mut conversation = Conversation {
        visible: true,
        messages: vec![
            Message {
                id: "user".into(),
                author: "You",
                attachments: Vec::new(),
                text: "User artwork request".into(),
                sent_at: chrono::Local
                    .with_ymd_and_hms(2026, 1, 1, 12, 34, 0)
                    .unwrap(),
            },
            Message {
                id: "artist".into(),
                author: "Artist",
                attachments: Vec::new(),
                text: "Artist response".into(),
                sent_at: chrono::Local
                    .with_ymd_and_hms(2026, 1, 1, 12, 35, 0)
                    .unwrap(),
            },
        ],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render(&mut conversation, &ctx);
    let hidden = render(&mut conversation, &ctx);
    let has_time = |output: &egui::FullOutput, time: &str| {
        output
            .shapes
            .iter()
            .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==time))
    };
    assert!(!has_time(&hidden, "12:34") && !has_time(&hidden, "12:35"));
    let rect = ctx
        .data(|d| {
            d.get_temp::<egui::Rect>(egui::Id::new(("message-test-rect", &String::from("user"))))
        })
        .unwrap();
    let hovered = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events: vec![egui::Event::PointerMoved(rect.center())],
            ..Default::default()
        },
        |ctx| {
            conversation.panel(ctx, None);
        },
    );
    assert!(has_time(&hovered, "12:34"));
    assert!(!has_time(&hovered, "12:35"));
    let after = ctx
        .data(|d| {
            d.get_temp::<egui::Rect>(egui::Id::new(("message-test-rect", &String::from("user"))))
        })
        .unwrap();
    assert_eq!(
        rect, after,
        "Hover toolbar must reserve its height while hidden"
    );
}

#[test]
fn chat_panel_clamps_to_half_window_width_and_send_is_compact() {
    let mut conversation = Conversation {
        visible: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    for width in [800.0, 600.0, 520.0] {
        render_size(&mut conversation, &ctx, width, 600.0);
        render_size(&mut conversation, &ctx, width, 600.0);
        let panel = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("conversation-panel-test-rect")))
            .unwrap();
        assert!(
            panel.width() >= 260.0 && panel.width() <= width * 0.5 + 1.0,
            "{panel:?} for {width}"
        );
        let send = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("composer-action-rect")))
            .unwrap();
        assert_eq!(send.size(), egui::vec2(22.0, 22.0));
    }
}

#[test]
fn model_and_thinking_settings_stack_below_the_section_heading() {
    let (_, messages) = mpsc::channel();
    let mut app = super::super::App::empty(messages);
    app.conversation.selection = Some(crate::agent::models::Selection::default());
    app.conversation.catalog = Some(vec![crate::agent::models::Model {
        id: "gpt-6.1-sol".into(),
        label: "GPT 6.1 Sol".into(),
        efforts: vec!["low".into()],
        default_effort: "low".into(),
    }]);
    let ctx = egui::Context::default();
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("app-settings"), true);
    });
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new("settings-navigation"),
            (String::new(), 2_usize),
        )
    });
    let mut output = None;
    for _ in 0..3 {
        output = Some(ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| app.settings_window(ctx),
        ));
    }
    let output = output.unwrap();
    let rect = |label| {
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(t.galley.rect.translate(t.pos.to_vec2()))
                }
                _ => None,
            })
            .unwrap()
    };
    assert!(rect("GPT 6.1 Sol").top() > rect("ChatGPT model").bottom());
    assert!(rect("Thinking level").top() > rect("GPT 6.1 Sol").bottom());
    assert!(rect("Low").top() > rect("Thinking level").bottom());
    assert!((rect("ChatGPT model").left() - rect("Thinking level").left()).abs() < 1.0);
    assert!(!app.conversation.active());
}

fn render(conversation: &mut Conversation, ctx: &egui::Context) -> egui::FullOutput {
    render_size(conversation, ctx, 800.0, 480.0)
}
fn render_size(
    conversation: &mut Conversation,
    ctx: &egui::Context,
    width: f32,
    height: f32,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        },
        |ctx| {
            conversation.panel(ctx, None);
        },
    )
}

#[test]
fn minimum_window_keeps_composer_and_approval_reachable() {
    let (actions, _commands) = mpsc::channel();
    let (_events, receiver) = mpsc::channel();
    let mut conversation = Conversation {
        visible: true,
        busy: true,
        ready: true,
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        recovery: Some("temporary.pixloom.json".into()),
        approvals: vec![(serde_json::json!(1), "An approval description\n".repeat(30))],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render_size(&mut conversation, &ctx, 280.0, 260.0);
    let output = render_size(&mut conversation, &ctx, 280.0, 260.0);
    let action = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("composer-action-rect")))
        .unwrap();
    assert!(action.bottom() <= 260.0);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Decline" && shape.clip_rect.contains(text.pos) && text.pos.y+text.galley.rect.bottom() <= shape.clip_rect.bottom())));
}

#[test]
fn opening_conversation_panel_does_not_start_ai() {
    let mut conversation = Conversation {
        visible: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let output = render(&mut conversation, &ctx);
    assert!(conversation.agent.is_none());
    assert!(!conversation.workspace.is_prepared());
    assert!(!output.shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text() == "Enable AI assistance")
    ));
}

#[test]
fn first_message_waits_for_ready_then_is_sent_once() {
    let (actions, commands) = mpsc::channel();
    let (events, receiver) = mpsc::channel();
    let mut conversation = Conversation {
        pending_prompt: Some("Draw a tiny sword".into()),
        busy: true,
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        ..Default::default()
    };
    assert!(commands.try_recv().is_err());
    events.send(Event::Ready).unwrap();
    render(&mut conversation, &egui::Context::default());
    assert!(matches!(commands.try_recv(),Ok(Action::Send(text)) if text == "Draw a tiny sword"));
    assert!(conversation.busy);
    assert!(conversation.pending_prompt.is_none());
    render(&mut conversation, &egui::Context::default());
    assert!(commands.try_recv().is_err());
}

#[test]
fn conversation_is_left_and_consecutive_messages_stack_without_author_tags() {
    let mut conversation = Conversation {
        visible: true,
        messages: vec![
            Message {
                id: "1".into(),
                author: "You",
                attachments: Vec::new(),
                sent_at: chrono::Local::now(),
                text: "user first".into(),
            },
            Message {
                id: "2".into(),
                author: "You",
                attachments: Vec::new(),
                sent_at: chrono::Local::now(),
                text: "user second".into(),
            },
            Message {
                id: "3".into(),
                author: "Artist",
                attachments: Vec::new(),
                sent_at: chrono::Local::now(),
                text: "artist first".into(),
            },
            Message {
                id: "4".into(),
                author: "Artist",
                attachments: Vec::new(),
                sent_at: chrono::Local::now(),
                text: "artist second".into(),
            },
        ],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render(&mut conversation, &ctx);
    let mut remaining = egui::Rect::NOTHING;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        },
        |ctx| {
            conversation.panel(ctx, None);
            remaining = ctx.available_rect();
        },
    );
    assert!(remaining.left() > 200.0 && remaining.right() == 800.0);
    let text = |name: &str| {
        output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == name => Some(t),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(text("You").is_empty());
    assert!(text("Artist").is_empty());
    assert!(text("artist second")[0].pos.y > text("artist first")[0].pos.y);
    assert!((text("artist second")[0].pos.x - text("artist first")[0].pos.x).abs() < 1.0);
    assert!(text("user second")[0].pos.y > text("user first")[0].pos.y);
    let user_box = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.fill == egui::Color32::from_gray(43)
                    && rect.rect.contains(text("user first")[0].pos) =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .unwrap();
    let right_padding = remaining.left() - 8.0 - user_box.right();
    let left_padding = text("artist first")[0].pos.x - 8.0;
    assert!(left_padding.abs() < 1.0);
    assert!(right_padding < 2.0);
}

#[test]
fn main_menu_save_works_for_a_project_without_starting_ai() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("manual.pixloom.json");
    let mut session = crate::session::Session::default();
    session
        .create(crate::session::CreateProject {
            width: 2,
            height: 2,
            palette: vec!["#00000000".into()],
            output_path: source.clone(),
        })
        .unwrap();
    session.save("project-1").unwrap();
    let mut conversation = Conversation::default();
    let ctx = egui::Context::default();
    conversation.save_project(&ctx, Some(&source), false);
    assert!(conversation.busy);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while conversation.busy {
        render(&mut conversation, &ctx);
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(conversation.agent.is_none());
    assert_eq!(
        conversation.saved_path().unwrap().canonicalize().unwrap(),
        source.canonicalize().unwrap()
    );
}

#[test]
fn failed_revert_and_agent_disconnect_preserve_baseline_capability() {
    let (actions, _commands) = mpsc::channel();
    let (events, receiver) = mpsc::channel();
    let mut conversation = Conversation {
        can_revert: true,
        busy: true,
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        ..Default::default()
    };
    events
        .send(Event::Status(
            "Document action failed: recovery unavailable".into(),
        ))
        .unwrap();
    events.send(Event::DocumentDone).unwrap();
    events
        .send(Event::Error("Codex disconnected".into()))
        .unwrap();
    render(&mut conversation, &egui::Context::default());
    assert!(conversation.can_revert);
    assert!(!conversation.busy);
}

#[test]
fn long_status_and_transcript_leave_composer_visible() {
    let (actions, _commands) = mpsc::channel();
    let (_events, receiver) = mpsc::channel();
    let mut conversation = Conversation {
        visible: true,
        ready: true,
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        recovery: Some(PathBuf::from("temporary.pixloom.json")),
        status: "An unusually long diagnostic message ".repeat(100),
        messages: vec![Message {
            id: "artist".into(),
            author: "Artist",
            attachments: Vec::new(),
            sent_at: chrono::Local::now(),
            text: "Many lines of conversation.\n".repeat(100),
        }],
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render(&mut conversation, &ctx);
    render(&mut conversation, &ctx);
    let button = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("composer-action-rect")))
        .unwrap();
    assert!(
        button.bottom() <= 480.0,
        "composer is outside window: {button:?}"
    );
}

#[test]
fn enter_sends_and_shift_enter_inserts_a_newline() {
    let (actions, commands) = mpsc::channel();
    let (_events, receiver) = mpsc::channel();
    let mut conversation = Conversation {
        visible: true,
        ready: true,
        draft: "draw a sword".into(),
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    render(&mut conversation, &ctx);
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("art-prompt")));
    let key = |modifiers| egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    let input = |events, modifiers| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 480.0),
        )),
        events,
        modifiers,
        ..Default::default()
    };
    let _ = ctx.run(
        input(vec![key(egui::Modifiers::SHIFT)], egui::Modifiers::SHIFT),
        |ctx| {
            conversation.panel(ctx, None);
        },
    );
    assert!(conversation.draft.contains('\n'));
    assert!(commands.try_recv().is_err());
    let _ = ctx.run(
        input(vec![key(egui::Modifiers::NONE)], egui::Modifiers::NONE),
        |ctx| {
            conversation.panel(ctx, None);
        },
    );
    assert!(matches!(commands.try_recv(),Ok(Action::Send(text)) if text.contains("draw a sword")));
    assert!(conversation.draft.is_empty());
}

#[test]
fn rewind_restores_prompt_references_and_truncates_later_messages_only_on_success() {
    let (events, receiver) = mpsc::channel();
    let (actions, commands) = mpsc::channel();
    let mut conversation = Conversation {
        ready: true,
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        ..Default::default()
    };
    for (id, author, text) in [
        ("u1", "You", "first"),
        ("a1", "Artist", "done"),
        ("u2", "You", "change"),
        ("a2", "Artist", "changed"),
    ] {
        conversation.messages.push(Message {
            id: id.into(),
            author,
            text: text.into(),
            attachments: vec![],
            sent_at: chrono::Local::now(),
        });
    }
    conversation.checkpoints.insert("u1".into(), 1);
    conversation.checkpoints.insert("u2".into(), 2);
    conversation.rewind_message(2);
    assert!(matches!(commands.try_recv().unwrap(), Action::Rewind(2)));
    assert_eq!(conversation.messages.len(), 4);
    events.send(Event::Rewound(2)).unwrap();
    events.send(Event::Finished).unwrap();
    conversation.poll_events(&egui::Context::default(), None);
    assert_eq!(conversation.messages.len(), 2);
    assert_eq!(conversation.draft, "change");
    assert!(!conversation.busy);
    assert_eq!(conversation.checkpoints.len(), 1);
}

#[test]
fn reasoning_summaries_stay_collapsed_and_duration_stops_when_done() {
    let (events, receiver) = mpsc::channel();
    let (actions, _) = mpsc::channel();
    let mut conversation = Conversation {
        visible: true,
        ready: true,
        agent: Some(Agent {
            actions,
            events: receiver,
            canvas: Default::default(),
        }),
        ..Default::default()
    };
    events
        .send(Event::Thought {
            id: "thought".into(),
            text: "Checking the pose spacing.".into(),
            done: false,
            delta: true,
        })
        .unwrap();
    let ctx = egui::Context::default();
    render(&mut conversation, &ctx);
    assert!(conversation.thoughts[0].elapsed.is_none());
    events
        .send(Event::Thought {
            id: "thought".into(),
            text: "Checking the pose spacing.".into(),
            done: true,
            delta: false,
        })
        .unwrap();
    events
        .send(Event::Message {
            id: "reply".into(),
            text: "Animation complete.".into(),
            delta: false,
        })
        .unwrap();
    let output = render(&mut conversation, &ctx);
    assert_eq!(conversation.thoughts[0].position, 0);
    assert!(conversation.thoughts[0].elapsed.is_some());
    assert!(!output.shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Checking the pose spacing.")
    ));
    assert!(output.shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Animation complete.")
    ));
}
