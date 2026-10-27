#[path = "../../src-tauri/src/ai_context.rs"]
mod ai_context;
use base64::Engine;
use pixen::desktop::{Application, PaintPixel};

#[test]
fn animation_context_includes_distinct_frames_in_order() {
    let root=tempfile::tempdir().unwrap(); let mut app=Application::new(false); app.create(2,1).unwrap();
    let first=app.snapshot().unwrap().document.unwrap();
    app.edit_structure(first.generation,serde_json::from_value(serde_json::json!({"type":"duplicate_frame","frame_id":first.frames[0].id,"new_frame_id":"second","index":1})).unwrap()).unwrap();
    let current=app.snapshot().unwrap().document.unwrap();
    app.paint(current.generation,"second",&current.layers[0].id,vec![PaintPixel{x:0,y:0,color:"#00ff00ff".into()}]).unwrap();
    let context=ai_context::prepare(&app.ai_desktop().unwrap(),root.path(),&[]).unwrap();
    assert!(context.notes.contains("2 frame(s) total"));
    let bytes=base64::engine::general_purpose::STANDARD.decode(&context.images[0].data).unwrap();
    let png=image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(png.get_pixel(0,0).0[3],0);
    assert_eq!(png.get_pixel(png.width()/2,0).0,[0,255,0,255]);
}

#[test]
fn each_message_gets_fresh_pixels_and_user_references_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let desktop = app.ai_desktop().unwrap();
    let first = ai_context::prepare(&desktop, root.path(), &[]).unwrap();
    let info = app.snapshot().unwrap().document.unwrap();
    app.paint(
        info.generation,
        &info.frames[0].id,
        &info.layers[0].id,
        vec![PaintPixel {
            x: 0,
            y: 0,
            color: "#ff0000ff".into(),
        }],
    )
    .unwrap();
    let reference = ai_context::Reference {
        name: "palette.txt".into(),
        data: base64::engine::general_purpose::STANDARD.encode("Use warm colors"),
    };
    let second = ai_context::prepare(&desktop, root.path(), &[reference]).unwrap();
    assert_ne!(first.images[0].data, second.images[0].data);
    assert!(std::path::Path::new(&first.images[0].path).is_file());
    assert!(second.notes.contains("Use warm colors"));
    assert!(second.notes.contains("palette.txt"));
    let decoded = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(&second.images[0].data)
            .unwrap(),
    )
    .unwrap()
    .to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
}

#[test]
fn image_references_are_normalized_and_bad_files_do_not_send() {
    let root = tempfile::tempdir().unwrap();
    let mut app = Application::new(false);
    app.create(2, 2).unwrap();
    let desktop = app.ai_desktop().unwrap();
    let mut png = Vec::new();
    pixen::raster::png(&mut png, 1, 1, &[0, 255, 0, 255]).unwrap();
    let reference = ai_context::Reference {
        name: "../reference.png".into(),
        data: base64::engine::general_purpose::STANDARD.encode(&png),
    };
    let context = ai_context::prepare(&desktop, root.path(), &[reference]).unwrap();
    assert_eq!(context.images.len(), 2);
    assert!(std::path::Path::new(&context.images[1].path).starts_with(root.path()));
    let invalid = ai_context::Reference {
        name: "binary.bin".into(),
        data: base64::engine::general_purpose::STANDARD.encode([0, 255, 12]),
    };
    assert!(ai_context::prepare(&desktop, root.path(), &[invalid]).is_err());
}
