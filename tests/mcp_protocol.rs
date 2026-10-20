use base64::Engine;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct Client {
    child: Child,
    responses: mpsc::Receiver<Value>,
    id: u64,
    diagnostics: mpsc::Receiver<String>,
}
impl Client {
    fn new(viewer: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pixloom-mcp"));
        if viewer {
            command.arg("--viewer");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (diag_tx, diagnostics) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let line = line.unwrap();
                eprintln!("{line}");
                if diag_tx.send(line).is_err() {
                    break;
                }
            }
        });
        let (tx, responses) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.unwrap();
                if tx
                    .send(serde_json::from_str(&line).expect("stdout must contain only JSON-RPC"))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut c = Self {
            child,
            responses,
            id: 0,
            diagnostics,
        };
        let init=c.request("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"pixloom-test","version":"1"}}));
        assert!(init.get("error").is_none(), "{init}");
        c.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        c
    }
    fn send(&mut self, v: Value) {
        writeln!(self.child.stdin.as_mut().unwrap(), "{v}").unwrap();
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        self.send(json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}));
        loop {
            let v = self
                .responses
                .recv_timeout(Duration::from_secs(15))
                .expect("MCP response timed out");
            if v["id"] == self.id {
                return v;
            }
        }
    }
    fn call(&mut self, name: &str, args: Value) -> Value {
        let response = self.request("tools/call", json!({"name":name,"arguments":args}));
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        for _ in self.diagnostics.try_iter() {}
        drop(self.child.stdin.take());
        for _ in 0..100 {
            if self.child.try_wait().unwrap().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod graphical {
    use super::*;
    use std::ffi::c_void;
    type Hwnd = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(callback: unsafe extern "system" fn(Hwnd, isize) -> i32, data: isize)
        -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        fn IsWindow(hwnd: Hwnd) -> i32;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, count: i32) -> i32;
        fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> i32;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetForegroundWindow() -> Hwnd;
        fn ShowWindow(hwnd: Hwnd, command: i32) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn GetDC(hwnd: Hwnd) -> *mut c_void;
        fn ReleaseDC(hwnd: Hwnd, dc: *mut c_void) -> i32;
        fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, w: i32, h: i32, flags: u32)
        -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
    }
    struct Search {
        pid: u32,
        hwnd: Hwnd,
    }
    unsafe extern "system" fn visit(hwnd: Hwnd, data: isize) -> i32 {
        let search = unsafe { &mut *(data as *mut Search) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        if pid == search.pid {
            let mut title = [0u16; 256];
            let len = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), 256) };
            let title = String::from_utf16_lossy(&title[..len as usize]);
            if title != "Pixloom" {
                return 1;
            }
            search.hwnd = hwnd;
            return 0;
        }
        1
    }
    fn window(c: &Client) -> Hwnd {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let pid = loop {
            let line = c
                .diagnostics
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .expect("viewer failed to open");
            if let Some(pid) = line
                .strip_prefix("pixloom: viewer opened (pid ")
                .and_then(|s| s.strip_suffix(')'))
            {
                break pid.parse().unwrap();
            }
        };
        let mut search = Search {
            pid,
            hwnd: std::ptr::null_mut(),
        };
        unsafe {
            EnumWindows(visit, &mut search as *mut Search as isize);
        }
        assert!(!search.hwnd.is_null());
        search.hwnd
    }
    fn wait_for(mut predicate: impl FnMut() -> bool) {
        for _ in 0..200 {
            if predicate() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("graphical condition timed out");
    }
    fn center(hwnd: Hwnd) -> u32 {
        unsafe {
            let mut rect = Rect::default();
            assert_ne!(GetClientRect(hwnd, &mut rect), 0);
            let mut point = Point {
                x: rect.right / 2,
                y: rect.bottom / 2,
            };
            ClientToScreen(hwnd, &mut point);
            let dc = GetDC(std::ptr::null_mut());
            let color = GetPixel(dc, point.x, point.y);
            ReleaseDC(std::ptr::null_mut(), dc);
            color
        }
    }

    #[test]
    #[ignore = "requires an interactive Windows desktop; opens real canvas windows"]
    fn viewer_updates_resizes_closes_and_survives_save_failure() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Client::new(true);
        let path = dir.path().join("view.pixloom.json");
        let created = c.call(
            "create_project",
            json!({"width":8,"height":8,"palette":["#FF0000FF","#00FF00FF"],"output_path":path}),
        );
        let id = text(&created)["project_id"].clone();
        let hwnd = window(&c);
        assert_ne!(unsafe { IsWindowVisible(hwnd) }, 0);
        assert_eq!(
            unsafe { GetWindowLongPtrW(hwnd, -20) } & 8,
            0,
            "preview must not be topmost"
        );
        let reopened = c.call("open_preview", json!({"project_id":id}));
        assert_eq!(text(&reopened)["reused"], true);
        let original_pid = text(&reopened)["pid"].clone();
        unsafe {
            ShowWindow(hwnd, 6);
        }
        wait_for(|| unsafe { IsIconic(hwnd) != 0 });
        let restored = c.call("open_preview", json!({"project_id":id}));
        assert_eq!(text(&restored)["pid"], original_pid);
        wait_for(|| unsafe { IsIconic(hwnd) == 0 });
        wait_for(|| center(hwnd) == 0xc0c0c0);
        for (revision, index, expected) in [(0, 0, 0x0000ff), (1, 1, 0x00ff00)] {
            let drawn=c.call("apply_edits",json!({"project_id":id,"expected_revision":revision,"operations":[{"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":index}]}));
            assert_ne!(drawn["isError"], true, "{drawn}");
            wait_for(|| center(hwnd) == expected);
        }
        unsafe {
            SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 800, 600, 0x0002 | 0x0004);
        }
        wait_for(|| center(hwnd) == 0x00ff00);
        assert_eq!(
            text(&c.call("get_preview", json!({"project_id":id,"scale":1})))["width"],
            8
        );
        std::fs::write(&path, b"existing").unwrap();
        assert_eq!(
            c.call("finish_project", json!({"project_id":id}))["isError"],
            true
        );
        assert_ne!(unsafe { IsWindow(hwnd) }, 0);
        std::fs::remove_file(&path).unwrap();
        assert_ne!(
            c.call("finish_project", json!({"project_id":id}))["isError"],
            true
        );
        assert_eq!(unsafe { IsWindow(hwnd) }, 0);
        let second=c.call("create_project",json!({"width":8,"height":8,"palette":["#FFFFFFFF"],"output_path":dir.path().join("manual.pixloom.json")}));
        let hwnd = window(&c);
        unsafe {
            PostMessageW(hwnd, 0x0010, 0, 0);
        }
        wait_for(|| unsafe { IsWindow(hwnd) == 0 });
        let id = text(&second)["project_id"].clone();
        assert_ne!(
            c.call("get_preview", json!({"project_id":id}))["isError"],
            true
        );
        assert_ne!(
            c.call("finish_project", json!({"project_id":id}))["isError"],
            true
        );
        let third=c.call("create_project",json!({"width":8,"height":8,"palette":["#FFFFFFFF"],"output_path":dir.path().join("crash.pixloom.json")}));
        assert_ne!(third["isError"], true);
        let hwnd = window(&c);
        c.child.kill().unwrap();
        c.child.wait().unwrap();
        wait_for(|| unsafe { IsWindow(hwnd) == 0 });
        assert!(!dir.path().join("crash.pixloom.json").exists());
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .any(|p| p
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".pixloom-recovery-"))
        );
    }
    #[test]
    #[ignore = "requires interactive Windows desktop; opens existing-file and live previews"]
    fn file_preview_reuses_does_not_mutate_and_live_updates_do_not_activate() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("animated.pixloom.json");
        let original = include_bytes!("../skills/pixloom/examples/animated.pixloom.json");
        std::fs::write(&source, original).unwrap();
        let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
        let mut c = Client::new(false);
        let created=c.call("create_project",json!({"width":8,"height":8,"palette":["#FFFFFFFF"],"output_path":dir.path().join("live.pixloom.json")}));
        let id = text(&created)["project_id"].clone();
        assert_eq!(
            text(&c.call("open_preview", json!({"project_id":id})))["opened"],
            true
        );
        let live = window(&c);
        let start = std::time::Instant::now();
        let opened = c.call("open_preview", json!({"path":source}));
        assert!(start.elapsed() < Duration::from_secs(4));
        assert_eq!(text(&opened)["opened"], true);
        assert_eq!(text(&opened)["frame_count"], 4);
        assert!(text(&opened)["revision"].is_null());
        let existing = window(&c);
        let focused = unsafe { GetForegroundWindow() };
        let edit=c.call("apply_edits",json!({"project_id":id,"expected_revision":0,"operations":[{"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":0}]}));
        assert_eq!(text(&edit)["revision"], 1);
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            unsafe { GetForegroundWindow() },
            focused,
            "live update stole foreground focus"
        );
        let again = c.call(
            "open_preview",
            json!({"path":source.parent().unwrap().join(".").join("animated.pixloom.json")}),
        );
        assert_eq!(text(&again)["pid"], text(&opened)["pid"]);
        assert_eq!(text(&again)["reused"], true);
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert_eq!(
            std::fs::metadata(&source).unwrap().modified().unwrap(),
            modified
        );
        assert_ne!(unsafe { IsWindow(live) }, 0);
        assert_ne!(unsafe { IsWindow(existing) }, 0);
        std::fs::remove_file(&source).unwrap();
        assert_eq!(
            c.call("open_preview", json!({"path":source}))["isError"],
            true
        );
        assert_ne!(unsafe { IsWindow(existing) }, 0);
        unsafe {
            PostMessageW(existing, 0x0010, 0, 0);
        }
        wait_for(|| unsafe { IsWindow(existing) == 0 });
        std::fs::write(&source, original).unwrap();
        let reopened = c.call("open_preview", json!({"path":source}));
        assert_eq!(text(&reopened)["reused"], false);
        let reopened = window(&c);
        assert_ne!(
            c.call("finish_project", json!({"project_id":id}))["isError"],
            true
        );
        assert_eq!(unsafe { IsWindow(live) }, 0);
        assert_ne!(unsafe { IsWindow(reopened) }, 0);
        drop(c);
        wait_for(|| unsafe { IsWindow(reopened) == 0 });
    }
}
fn text(result: &Value) -> Value {
    serde_json::from_str(
        result["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["type"] == "text")
            .unwrap()["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn inspection_returns_bounded_png_and_preview_tools_validate_contracts() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(false);
    for args in [
        json!({}),
        json!({"project_id":"missing","path":"missing.pixloom.json"}),
        json!({"path":dir.path().join("missing.pixloom.json")}),
        json!({"project_id":"missing"}),
    ] {
        let failure = c.call("open_preview", args);
        assert_eq!(failure["isError"], true);
        assert_eq!(text(&failure)["opened"], false);
    }
    let unsupported = dir.path().join("bad.txt");
    std::fs::write(&unsupported, b"unchanged").unwrap();
    assert_eq!(
        c.call("open_preview", json!({"path":unsupported}))["isError"],
        true
    );
    assert_eq!(std::fs::read(&unsupported).unwrap(), b"unchanged");
    let created=c.call("create_project",json!({"width":4,"height":3,"palette":["#FF0000FF"],"output_path":dir.path().join("crop.pixloom.json")}));
    let id = text(&created)["project_id"].clone();
    let edit=c.call("apply_edits",json!({"project_id":id,"expected_revision":0,"operations":[{"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":1,"y":1,"palette_index":0}]}]}));
    assert_eq!(text(&edit)["revision"], 1);
    let crop = json!({"x":1,"y":1,"width":2,"height":1});
    let preview = c.call(
        "inspect_project",
        json!({"project_id":id,"crop":crop,"scale":3}),
    );
    assert_ne!(preview["isError"], true, "{preview}");
    let metadata = text(&preview);
    assert_eq!(metadata["source"], crop);
    assert_eq!(metadata["frame_id"], "frame-1");
    assert_eq!(metadata["revision"], 1);
    assert_eq!(metadata["width"], 4);
    assert_eq!(metadata["height"], 3);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(preview["content"][0]["data"].as_str().unwrap())
        .unwrap();
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (6, 3));
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(3, 0).0, [0, 0, 0, 0]);
    let full = c.call("inspect_project", json!({"project_id":id}));
    assert_eq!(
        text(&full)["source"],
        json!({"x":0,"y":0,"width":4,"height":3})
    );
    for extra in [
        json!({"frame_id":"frame-2"}),
        json!({"crop":{"x":-1,"y":0,"width":1,"height":1}}),
        json!({"crop":{"x":3,"y":0,"width":2,"height":1}}),
        json!({"scale":0}),
    ] {
        let mut args = extra;
        args["project_id"] = id.clone();
        assert_eq!(c.call("inspect_project", args)["isError"], true);
    }
    assert_eq!(
        text(&c.call("get_preview", json!({"project_id":id,"scale":1})))["revision"],
        1
    );
    assert_ne!(
        c.call("finish_project", json!({"project_id":id}))["isError"],
        true
    );
}

#[test]
fn standalone_resize_canvas_uses_revision_checks_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut client = Client::new(false);
    let created = client.call("create_project",json!({"width":2,"height":3,"palette":["#00000000"],"output_path":dir.path().join("sized.pixloom")}));
    let id = text(&created)["project_id"].clone();
    let resized = client.call(
        "resize_canvas",
        json!({"project_id":id,"expected_revision":0,"width":5,"height":4}),
    );
    assert_ne!(resized["isError"], true, "{resized}");
    assert_eq!(
        (
            text(&resized)["width"].as_u64(),
            text(&resized)["height"].as_u64()
        ),
        (Some(5), Some(4))
    );
    assert_eq!(text(&resized)["revision"], 1);
    let stale = client.call(
        "resize_canvas",
        json!({"project_id":id,"expected_revision":0,"width":1,"height":1}),
    );
    assert_eq!(stale["isError"], true);
    let undone = client.call("undo", json!({"project_id":id,"expected_revision":1}));
    assert_ne!(undone["isError"], true);
    let info = text(&client.call("get_project_info", json!({"project_id":id})));
    assert_eq!(
        (info["width"].as_u64(), info["height"].as_u64()),
        (Some(2), Some(3))
    );
}

#[test]
fn stdio_create_draw_preview_edit_finish_and_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(false);
    let listed = c.request("tools/list", json!({}));
    let mut names: Vec<_> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "analyze_sprite",
            "apply_edits",
            "audit_animation",
            "compare_checkpoint",
            "compare_frames",
            "copy_region",
            "create_project",
            "draw",
            "edit_frame",
            "edit_layer",
            "edit_palette",
            "export_project",
            "finish_project",
            "get_contact_sheet",
            "get_onion_skin",
            "get_pixels",
            "get_preview",
            "get_project_info",
            "get_review_sheet",
            "inspect_layer",
            "inspect_project",
            "manage_checkpoint",
            "open_preview",
            "open_project",
            "redo",
            "report_activity",
            "resize_canvas",
            "save_project",
            "transform_region",
            "undo",
            "validate_artwork"
        ]
    );
    let path = dir.path().join("sprite.pixloom.json");
    let created=c.call("create_project",json!({"width":8,"height":8,"palette":["#FF0000FF","#00FF00FF","#0000FFFF"],"output_path":path}));
    assert_ne!(created["isError"], true, "{created}");
    let project = text(&created);
    let id = &project["project_id"];
    let draw=c.call("apply_edits",json!({"project_id":id,"expected_revision":0,"operations":[
        {"type":"rectangle","frame_id":"frame-1","layer_id":"layer-1","x":1,"y":1,"width":6,"height":6,"filled":true,"palette_index":0},
        {"type":"line","frame_id":"frame-1","layer_id":"layer-1","x1":1,"y1":1,"x2":6,"y2":6,"palette_index":1},
        {"type":"set_pixels","frame_id":"frame-1","layer_id":"layer-1","pixels":[{"x":3,"y":2,"palette_index":2}]}
    ]}));
    assert_ne!(draw["isError"], true, "{draw}");
    assert_eq!(text(&draw)["revision"], 1);
    let preview = c.call("get_preview", json!({"project_id":id}));
    let content = &preview["content"][0];
    assert_eq!(content["type"], "image");
    assert_eq!(content["mimeType"], "image/png");
    let png = base64::engine::general_purpose::STANDARD
        .decode(content["data"].as_str().unwrap())
        .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (64, 64));
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(image.get_pixel(8, 8).0, [0, 255, 0, 255]);
    assert_eq!(image.get_pixel(24, 16).0, [0, 0, 255, 255]);
    assert_eq!(image.get_pixel(16, 8).0, [255, 0, 0, 255]);
    assert_eq!(text(&preview), json!({"revision":1,"width":8,"height":8}));
    let stale=c.call("apply_edits",json!({"project_id":id,"expected_revision":0,"operations":[{"type":"clear","frame_id":"frame-1","layer_id":"layer-1","palette_index":0}]}));
    assert_eq!(stale["isError"], true);
    let edit=c.call("apply_edits",json!({"project_id":id,"expected_revision":1,"operations":[{"type":"fill","frame_id":"frame-1","layer_id":"layer-1","x":0,"y":0,"palette_index":2}]}));
    assert_eq!(text(&edit)["revision"], 2);
    let finished = c.call("finish_project", json!({"project_id":id}));
    assert_eq!(text(&finished)["revision"], 2);
    pixloom::schema::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let second=c.call("create_project",json!({"width":1,"height":1,"palette":["#FFFFFFFF"],"output_path":dir.path().join("second.pixloom.json")}));
    assert_ne!(second["isError"], true);
    c.child.kill().unwrap();
    c.child.wait().unwrap();
    let recovery = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|p| p.ok())
        .find(|p| {
            p.file_name()
                .to_string_lossy()
                .starts_with(".pixloom-recovery-")
        })
        .unwrap();
    pixloom::schema::read_document(&std::fs::read(recovery.path()).unwrap()).unwrap();
    assert!(!dir.path().join("second.pixloom.json").exists());
}

#[test]
fn focused_tools_and_mixed_apply_share_one_revision_and_history_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(false);
    let created=c.call("create_project",json!({"width":5,"height":5,"palette":["#00000000","#FF0000FF"],"output_path":dir.path().join("tools.pixloom.json")}));
    let id = text(&created)["project_id"].clone();
    let info = c.call("get_project_info", json!({"project_id":id}));
    assert_eq!(text(&info)["frames"][0]["id"], "frame-1");
    let layer=c.call("edit_layer",json!({"project_id":id,"expected_revision":0,"operation":{"type":"add_layer","layer_id":"ink","name":"Ink"}}));
    assert_ne!(layer["isError"], true, "{layer}");
    assert_eq!(text(&layer)["revision"], 1);
    let frame=c.call("edit_frame",json!({"project_id":id,"expected_revision":1,"operation":{"type":"add_frame","frame_id":"next","duration_ms":250}}));
    assert_ne!(frame["isError"], true, "{frame}");
    assert_eq!(text(&frame)["revision"], 2);
    let draw=c.call("draw",json!({"project_id":id,"expected_revision":2,"frame_id":"next","layer_id":"ink","operation":{"type":"ellipse","x":0,"y":0,"width":5,"height":5,"palette_index":1,"filled":true}}));
    assert_ne!(draw["isError"], true, "{draw}");
    assert_eq!(text(&draw)["revision"], 3);
    assert!(
        draw["content"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["type"] != "image")
    );
    let region=c.call("get_pixels",json!({"project_id":id,"frame_id":"next","layer_id":"ink","region":{"x":2,"y":2,"width":1,"height":1}}));
    assert_eq!(
        text(&region)["pixels"][0],
        json!({"x":2,"y":2,"palette_index":1,"rgba":"#FF0000FF"})
    );
    let review = c.call(
        "inspect_project",
        json!({"project_id":id,"frame_id":"next","scale":1}),
    );
    assert_eq!(review["content"][0]["type"], "image");
    assert_eq!(text(&review)["frame_id"], "next");
    let stale=c.call("draw",json!({"project_id":id,"expected_revision":2,"frame_id":"next","layer_id":"ink","operation":{"type":"clear_cel"}}));
    assert_eq!(stale["isError"], true);
    let batch=c.call("apply_edits",json!({"project_id":id,"expected_revision":3,"operations":[{"type":"duplicate_frame","frame_id":"next","new_frame_id":"copy"},{"type":"erase_region","frame_id":"copy","layer_id":"ink","x":2,"y":2,"width":1,"height":1}]}));
    assert_ne!(batch["isError"], true, "{batch}");
    assert_eq!(text(&batch)["revision"], 4);
    let undone = c.call("undo", json!({"project_id":id,"expected_revision":4}));
    assert_eq!(text(&undone)["revision"], 5);
    assert_eq!(
        text(&c.call("get_project_info", json!({"project_id":id})))["frames"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let redone = c.call("redo", json!({"project_id":id,"expected_revision":5}));
    assert_eq!(text(&redone)["revision"], 6);
    assert_eq!(
        text(&c.call("get_project_info", json!({"project_id":id})))["frames"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_ne!(
        c.call("finish_project", json!({"project_id":id}))["isError"],
        true
    );
}

#[test]
fn animation_review_save_reopen_and_looping_export_preserve_the_timeline() {
    use pixloom::{
        export::{self, ExportOptions, ImageFormat},
        preview, raster,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("motion.pixloom.json");
    let mut c = Client::new(false);
    let created = text(&c.call("create_project", json!({"width":4,"height":3,"palette":["#00000000","#FF0000FF","#00FF00FF"],"output_path":path})));
    let id = created["project_id"].clone();
    let mut ops = vec![json!({"type":"set_frame_duration","frame_id":"frame-1","duration_ms":180})];
    for i in 2..=4 {
        ops.push(json!({"type":"add_frame","frame_id":format!("pose-{i}"),"duration_ms":80*i}));
    }
    for (i, frame) in ["frame-1", "pose-2", "pose-3", "pose-4"].iter().enumerate() {
        ops.push(json!({"type":"set_pixels","frame_id":frame,"layer_id":"layer-1","pixels":[{"x":i,"y":1,"palette_index":1+i%2}]}));
    }
    assert_eq!(
        text(&c.call(
            "apply_edits",
            json!({"project_id":id,"expected_revision":0,"operations":ops})
        ))["revision"],
        1
    );
    let before = text(&c.call("get_project_info", json!({"project_id":id})));
    let sheet = c.call(
        "get_contact_sheet",
        json!({"project_id":id,"scale":4,"columns":2}),
    );
    assert_ne!(sheet["isError"], true, "{sheet}");
    let metadata = text(&sheet);
    assert_eq!(metadata["distinct_composites"], 4);
    assert_eq!(metadata["revision"], 1);
    assert_eq!(metadata["frames"][1]["frame_id"], "pose-2");
    assert_eq!(metadata["frames"][1]["duration_ms"], 160.0);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(sheet["content"][0]["data"].as_str().unwrap())
        .unwrap();
    let image = image::load_from_memory(&bytes).unwrap().into_rgba8();
    let bounds = &metadata["frames"][1]["image_bounds"];
    let x = bounds["x"].as_u64().unwrap() as u32;
    let y = bounds["y"].as_u64().unwrap() as u32;
    assert_eq!(image.get_pixel(x + 4, y + 4).0, [0, 255, 0, 255]);
    // Labels must be rasterized below every composite, not just returned as metadata.
    for entry in metadata["frames"].as_array().unwrap() {
        let b = &entry["image_bounds"];
        let x = b["x"].as_u64().unwrap() as u32;
        let y = b["y"].as_u64().unwrap() as u32 + b["height"].as_u64().unwrap() as u32;
        assert!(
            (y..y + 38)
                .any(|py| (x..(x + 70).min(image.width()))
                    .any(|px| image.get_pixel(px, py)[0] > 150))
        );
    }
    for args in [
        json!({"project_id":id,"columns":0}),
        json!({"project_id":id,"scale":0}),
        json!({"project_id":id,"scale":u32::MAX}),
        json!({"project_id":id,"frame_ids":[]}),
        json!({"project_id":id,"frame_ids":["pose-2","pose-2"]}),
        json!({"project_id":id,"frame_ids":["missing"]}),
    ] {
        assert_eq!(c.call("get_contact_sheet", args)["isError"], true);
    }
    let subset = text(&c.call(
        "get_contact_sheet",
        json!({"project_id":id,"frame_ids":["pose-4","frame-1"],"scale":1}),
    ));
    assert_eq!(subset["frames"][0]["frame_id"], "pose-4");
    assert_eq!(subset["frames"].as_array().unwrap().len(), 2);
    let mut renders = Vec::new();
    for frame in ["frame-1", "pose-2", "pose-3", "pose-4"] {
        let png = c.call(
            "inspect_project",
            json!({"project_id":id,"frame_id":frame,"scale":1}),
        );
        assert_eq!(text(&png)["frame_id"], frame);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(png["content"][0]["data"].as_str().unwrap())
            .unwrap();
        renders.push(
            image::load_from_memory(&bytes)
                .unwrap()
                .into_rgba8()
                .into_raw(),
        );
    }
    assert_eq!(
        text(&c.call("get_project_info", json!({"project_id":id}))),
        before
    );
    assert_ne!(
        c.call("finish_project", json!({"project_id":id}))["isError"],
        true
    );
    let reopened = preview::load(&path).unwrap();
    assert_eq!(
        reopened
            .frames
            .iter()
            .map(|f| f.duration_ms)
            .collect::<Vec<_>>(),
        [180.0, 160.0, 240.0, 320.0]
    );
    for (i, pixels) in renders.iter().enumerate() {
        assert_eq!(&raster::render(&reopened, i).unwrap(), pixels);
    }
    let mut encoded = std::io::Cursor::new(Vec::new());
    export::write(
        &mut encoded,
        &reopened,
        ImageFormat::Gif,
        &ExportOptions {
            loop_animation: true,
            ..Default::default()
        },
    )
    .unwrap();
    let gif = dir.path().join("motion.gif");
    std::fs::write(&gif, encoded.into_inner()).unwrap();
    let before_bytes = std::fs::read(&gif).unwrap();
    let imported = preview::load(&gif).unwrap();
    assert_eq!(imported.frames.len(), 4);
    for (i, pixels) in renders.iter().enumerate() {
        assert_eq!(&raster::render(&imported, i).unwrap(), pixels);
        assert_eq!(
            imported.frames[i].duration_ms,
            reopened.frames[i].duration_ms
        );
    }
    assert_eq!(std::fs::read(&gif).unwrap(), before_bytes);
}

#[test]
fn activity_reporting_is_validated_and_does_not_edit_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(false);
    let created = text(&c.call(
        "create_project",
        json!({
            "width":4,"height":4,"palette":["#00000000"],
            "output_path":dir.path().join("activity.pixloom.json")
        }),
    ));
    let id = &created["project_id"];
    let before = text(&c.call("get_project_info", json!({"project_id":id})));
    let report = c.call(
        "report_activity",
        json!({
            "project_id":id,"message":"Drawing the ranger's weapons","busy":true,
            "frame_id":"frame-1","layer_id":"layer-1"
        }),
    );
    assert_ne!(report["isError"], true, "{report}");
    assert_eq!(text(&report)["revision"], 0);
    for extra in [json!({"message":""}), json!({"frame_id":"missing"})] {
        let mut args = json!({"project_id":id,"message":"Drawing","busy":true});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(c.call("report_activity", args)["isError"], true);
    }
    assert_ne!(
        c.call(
            "report_activity",
            json!({"project_id":id,"message":"Weapons ready","busy":false})
        )["isError"],
        true
    );
    assert_eq!(
        text(&c.call("get_project_info", json!({"project_id":id}))),
        before
    );
}

#[test]
fn project_container_palette_regions_and_animation_analysis_work_through_mcp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("art.pixloom");
    let mut c = Client::new(false);
    let created = c.call(
        "create_project",
        json!({"width":4,"height":4,"palette":["#00000000","#FF0000FF"],"output_path":path}),
    );
    assert_ne!(created["isError"], true, "{created}");
    let id = text(&created)["project_id"].as_str().unwrap().to_owned();
    let r = c.call("edit_palette", json!({"project_id":id,"expected_revision":0,"operation":{"type":"append_colors","colors":["#00FF00FF"]}}));
    assert_ne!(r["isError"], true, "{r}");
    let r = c.call("draw", json!({"project_id":id,"expected_revision":1,"frame_id":"frame-1","layer_id":"layer-1","operation":{"type":"set_pixels","pixels":[{"x":0,"y":0,"palette_index":1},{"x":1,"y":0,"palette_index":2}]}}));
    assert_ne!(r["isError"], true, "{r}");
    let r = c.call("copy_region", json!({"project_id":id,"expected_revision":2,"source_frame_id":"frame-1","source_layer_id":"layer-1","region":{"x":0,"y":0,"width":2,"height":1},"frame_id":"frame-1","layer_id":"layer-1","x":1,"y":1}));
    assert_ne!(r["isError"], true, "{r}");
    let r = c.call("transform_region", json!({"project_id":id,"expected_revision":3,"frame_id":"frame-1","layer_id":"layer-1","region":{"x":1,"y":1,"width":2,"height":1},"x":1,"y":1,"transform":"flip_horizontal"}));
    assert_ne!(r["isError"], true, "{r}");
    let pixels = text(&c.call("get_pixels", json!({"project_id":id,"frame_id":"frame-1","layer_id":"layer-1","region":{"x":1,"y":1,"width":2,"height":1}})));
    assert_eq!(pixels["pixels"][0]["palette_index"], 2);
    assert_eq!(pixels["pixels"][1]["palette_index"], 1);
    let r = c.call("edit_frame", json!({"project_id":id,"expected_revision":4,"operation":{"type":"add_frame","frame_id":"next","duration_ms":100}}));
    assert_ne!(r["isError"], true, "{r}");
    let compared = c.call(
        "compare_frames",
        json!({"project_id":id,"frame_id":"next","reference_frame_id":"frame-1","scale":1}),
    );
    assert_ne!(compared["isError"], true, "{compared}");
    assert!(
        compared["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["type"] == "image")
    );
    assert_eq!(text(&compared)["changed_pixels"], 4);
    let onion = c.call(
        "get_onion_skin",
        json!({"project_id":id,"frame_id":"next","scale":1}),
    );
    assert_ne!(onion["isError"], true, "{onion}");
    assert_eq!(
        text(&c.call("get_project_info", json!({"project_id":id})))["revision"],
        5
    );
    let saved = c.call("save_project", json!({"project_id":id}));
    assert_ne!(saved["isError"], true, "{saved}");
    assert!(pixloom::container::Reader::open(&path).is_ok());
    let r = c.call("finish_project", json!({"project_id":id}));
    assert_ne!(r["isError"], true, "{r}");
    let reopened = c.call("open_project", json!({"path":path}));
    assert_ne!(reopened["isError"], true, "{reopened}");
    let reopened_id = text(&reopened)["project_id"].as_str().unwrap().to_owned();
    let info = text(&c.call("get_project_info", json!({"project_id":reopened_id})));
    assert_eq!(info["palette"][2], "#00FF00FF");
    assert_eq!(info["frames"][1]["id"], "next");
    let export = c.call(
        "export_project",
        json!({"project_id":reopened_id,"output_path":dir.path().join("art.aseprite")}),
    );
    assert_ne!(export["isError"], true, "{export}");
    assert!(pixloom::native::read(&dir.path().join("art.aseprite")).is_ok());
}

#[test]
fn review_apis_return_images_reports_and_never_edit_artwork() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(false);
    let created=c.call("create_project",json!({"width":4,"height":4,"palette":["#00000000","#FF0000FF"],"output_path":dir.path().join("review.pixloom")}));
    let id = text(&created)["project_id"].clone();
    let cp = c.call(
        "manage_checkpoint",
        json!({"project_id":id,"action":"create","expected_revision":0,"label":"original"}),
    );
    assert_ne!(cp["isError"], true, "{cp}");
    let checkpoint = text(&cp)["checkpoint_id"].clone();
    for (name, args) in [
        (
            "get_review_sheet",
            json!({"project_id":id,"frame_id":"frame-1"}),
        ),
        (
            "analyze_sprite",
            json!({"project_id":id,"frame_id":"frame-1"}),
        ),
        (
            "compare_checkpoint",
            json!({"project_id":id,"checkpoint_id":checkpoint}),
        ),
        ("audit_animation", json!({"project_id":id})),
    ] {
        let r = c.call(name, args);
        assert_ne!(r["isError"], true, "{name}: {r}");
        assert!(
            r["content"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["type"] == "image")
        );
        assert_eq!(text(&r)["inspected_revision"], 0);
    }
    let r = c.call(
        "validate_artwork",
        json!({"project_id":id,"rules":[{"type":"canvas_size","width":8,"height":8}]}),
    );
    assert_ne!(r["isError"], true);
    assert_eq!(text(&r)["passed"], false);
    assert_eq!(
        text(&c.call("get_project_info", json!({"project_id":id})))["revision"],
        0
    );
}
