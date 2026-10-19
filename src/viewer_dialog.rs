use anyhow::Result;
use std::path::PathBuf;

#[cfg(windows)]
pub fn pick_file() -> Result<Option<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let filter: Vec<u16> = "Pixel art and images\0*.pixloom;*.json;*.spritex;*.pxo;*.ase;*.aseprite;*.png;*.jpg;*.jpeg;*.gif;*.apng;*.webp;*.bmp;*.tif;*.tiff\0All files\0*.*\0\0".encode_utf16().collect();
    let title: Vec<u16> = "Open file\0".encode_utf16().collect();
    let mut path = vec![0u16; 32768];
    // All pointers refer to live, null-terminated buffers for this synchronous call.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    dialog.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    // The API only writes within lpstrFile/nMaxFile; buffers outlive the call.
    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        return Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &path[..end],
        ))));
    }
    let error = unsafe { CommDlgExtendedError() };
    anyhow::ensure!(error == 0, "Windows file picker failed (code {error})");
    Ok(None)
}

#[cfg(not(windows))]
pub fn pick_file() -> Result<Option<PathBuf>> {
    anyhow::bail!("Native file picker currently supports Windows; drag a file into Pixloom")
}
