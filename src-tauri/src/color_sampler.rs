use serde::Serialize;

#[derive(Serialize)]
pub struct Sample {
    color: String,
    pixels: Vec<u8>,
    width: usize,
}

#[tauri::command]
pub async fn sample_color(window: tauri::WebviewWindow, x: f64, y: f64) -> Result<Sample, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    if !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || y < 0.0
        || x * scale >= size.width as f64
        || y * scale >= size.height as f64
    {
        return Err("Pointer is outside the application".into());
    }
    let px = origin.x + (x * scale).round() as i32;
    let py = origin.y + (y * scale).round() as i32;
    tauri::async_runtime::spawn_blocking(move || {
        capture(
            px,
            py,
            origin.x,
            origin.y,
            size.width as i32,
            size.height as i32,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(windows)]
fn capture(x: i32, y: i32, left: i32, top: i32, width: i32, height: i32) -> Result<Sample, String> {
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(window: *mut c_void) -> *mut c_void;
        fn ReleaseDC(window: *mut c_void, dc: *mut c_void) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn GetPixel(dc: *mut c_void, x: i32, y: i32) -> u32;
        fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
        fn CreateCompatibleBitmap(dc: *mut c_void, width: i32, height: i32) -> *mut c_void;
        fn SelectObject(dc: *mut c_void, object: *mut c_void) -> *mut c_void;
        fn DeleteObject(object: *mut c_void) -> i32;
        fn DeleteDC(dc: *mut c_void) -> i32;
        fn BitBlt(
            target: *mut c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            source: *mut c_void,
            sx: i32,
            sy: i32,
            operation: u32,
        ) -> i32;
    }
    // Capture only a small, app-bounded region. The DC is released before returning.
    unsafe {
        let dc = GetDC(std::ptr::null_mut());
        if dc.is_null() {
            return Err("Cannot capture application colors".into());
        }
        let memory = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, 11, 11);
        if memory.is_null() || bitmap.is_null() {
            if !memory.is_null() {
                DeleteDC(memory);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            ReleaseDC(std::ptr::null_mut(), dc);
            return Err("Cannot allocate color preview".into());
        }
        let previous = SelectObject(memory, bitmap);
        let source_x = (x - 5).max(left);
        let source_y = (y - 5).max(top);
        let copy_width = (left + width - source_x).min(11);
        let copy_height = (top + height - source_y).min(11);
        let copied = BitBlt(
            memory,
            0,
            0,
            copy_width,
            copy_height,
            dc,
            source_x,
            source_y,
            0x00CC0020,
        );
        let mut pixels = Vec::with_capacity(11 * 11 * 4);
        let mut failed = copied == 0;
        for dy in -5..=5 {
            for dx in -5..=5 {
                let value = GetPixel(
                    memory,
                    (x + dx - source_x).clamp(0, copy_width - 1),
                    (y + dy - source_y).clamp(0, copy_height - 1),
                );
                if value == u32::MAX {
                    failed = true;
                }
                pixels.extend_from_slice(&[
                    (value & 255) as u8,
                    ((value >> 8) & 255) as u8,
                    ((value >> 16) & 255) as u8,
                    255,
                ]);
            }
        }
        SelectObject(memory, previous);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), dc);
        if failed {
            return Err("Cannot read application colors".into());
        }
        let center = &pixels[(5 * 11 + 5) * 4..][..3];
        let color = format!("#{:02x}{:02x}{:02x}", center[0], center[1], center[2]);
        Ok(Sample {
            color,
            pixels,
            width: 11,
        })
    }
}

#[cfg(not(windows))]
fn capture(_: i32, _: i32, _: i32, _: i32, _: i32, _: i32) -> Result<Sample, String> {
    Err("Application color capture is not available on this platform".into())
}
