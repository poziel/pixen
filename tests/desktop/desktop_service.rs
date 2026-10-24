use pixloom::desktop::{Application, RenderRequest};

#[test]
fn document_lifetime_save_and_preview_do_not_require_a_frontend() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("saved.pixloom");
    let mut app = Application::new(false);
    app.create(4, 3).unwrap();
    let state = app.snapshot().unwrap();
    let document = state.document.unwrap();
    assert_eq!((document.width, document.height), (4, 3));
    let revision = document.revision;
    let request = RenderRequest {
        generation: document.generation,
        frame_id: document.frames[0].id.clone(),
        hidden_layers: document.layers.iter().map(|l| l.id.clone()).collect(),
        ..Default::default()
    };
    let image = app.render(request).unwrap();
    assert_eq!((image.width, image.height), (4, 3));
    assert_eq!(app.snapshot().unwrap().document.unwrap().revision, revision);
    app.save(Some(&path)).unwrap();
    app.resize(6, 5).unwrap();
    app.save(None).unwrap();
    let mut reopened = Application::new(false);
    reopened.open(&path).unwrap();
    let doc = reopened.snapshot().unwrap().document.unwrap();
    assert_eq!((doc.width, doc.height), (6, 5));
}

#[test]
fn managed_view_is_read_only_and_rejects_stale_render_requests() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.pixloom");
    let mut writer = Application::new(false);
    writer.create(2, 2).unwrap();
    writer.save(Some(&path)).unwrap();
    let mut app = Application::new(true);
    app.open(&path).unwrap();
    assert!(app.create(4, 4).is_err());
    assert!(app.resize(4, 4).is_err());
    assert!(app.save(None).is_err());
    let doc = app.snapshot().unwrap().document.unwrap();
    assert!(
        app.render(RenderRequest {
            generation: doc.generation + 1,
            frame_id: doc.frames[0].id.clone(),
            ..Default::default()
        })
        .is_err()
    );
    app.message(r#"{"label":"Drawing","frame_id":null,"layer_id":null,"busy":true}"#)
        .unwrap();
    assert!(app.snapshot().unwrap().activity.unwrap().busy);
    writer.resize(3, 4).unwrap();
    writer.save(None).unwrap();
    let updated = app.snapshot().unwrap().document.unwrap();
    assert_eq!((updated.width, updated.height), (3, 4));
    assert!(updated.generation > doc.generation);
}

#[test]
fn view_conversion_never_changes_the_saved_document() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("strip.png");
    let mut image = image::RgbaImage::new(32, 16);
    for y in 3..13 {
        for x in 3..13 {
            image.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            image.put_pixel(x + 16, y, image::Rgba([0, 255, 0, 255]));
        }
    }
    image.save(&path).unwrap();
    let mut app = Application::new(false);
    app.open(&path).unwrap();
    let before = app.snapshot().unwrap().document.unwrap();
    assert_eq!(before.frames.len(), 1);
    app.toggle_view().unwrap();
    let after = app.snapshot().unwrap().document.unwrap();
    assert_eq!((after.width, after.height, after.frames.len()), (16, 16, 2));
    assert_eq!(before.revision, after.revision);
    assert_eq!(app.follow_target("frame-2").unwrap(), "found");
    assert_eq!(app.follow_target(&before.frames[0].id).unwrap(), "changed");
    assert_eq!(app.snapshot().unwrap().document.unwrap().width, 32);
}

#[test]
fn inspection_review_and_resize_preview_preserve_source_pixels() {
    use base64::Engine;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("art.png");
    image::RgbaImage::from_pixel(4, 3, image::Rgba([255, 0, 0, 255]))
        .save(&path)
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut app = Application::new(false);
    app.open(&path).unwrap();
    let doc = app.snapshot().unwrap().document.unwrap();
    let frame = doc.frames[0].id.clone();
    let review = app.local_review("analyze", &frame, None).unwrap();
    assert_eq!(review.report["kind"], "sprite_analysis");
    assert!(!app.snapshot().unwrap().review_stale);
    let rendered = app
        .render(RenderRequest {
            generation: doc.generation,
            frame_id: frame.clone(),
            canvas_width: Some(6),
            canvas_height: Some(5),
            ..Default::default()
        })
        .unwrap();
    let image = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(rendered.png)
            .unwrap(),
    )
    .unwrap()
    .into_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(5, 4).0, [0, 0, 0, 0]);
    let hidden = app
        .render(RenderRequest {
            generation: doc.generation,
            frame_id: frame,
            hidden_layers: doc.layers.iter().map(|l| l.id.clone()).collect(),
            ..Default::default()
        })
        .unwrap();
    let image = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(hidden.png)
            .unwrap(),
    )
    .unwrap()
    .into_rgba8();
    assert!(image.pixels().all(|p| p.0[3] == 0));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    app.resize(5, 4).unwrap();
    assert!(app.snapshot().unwrap().review_stale);
}

#[test]
fn external_edits_do_not_overwrite_unsaved_document_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("art.pixloom");
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    app.save(Some(&path)).unwrap();
    app.resize(3, 3).unwrap();
    let mut other = Application::new(false);
    other.open(&path).unwrap();
    other.resize(4, 4).unwrap();
    other.save(None).unwrap();
    let external = std::fs::read(&path).unwrap();
    assert!(app.snapshot().is_err());
    assert!(app.save(None).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), external);
    let copy = root.path().join("copy.pixloom");
    app.save(Some(&copy)).unwrap();
    let sprite = pixloom::preview::load(&copy).unwrap();
    assert_eq!((sprite.width, sprite.height), (3, 3));
}
