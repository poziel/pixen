use anyhow::Result;
use std::path::PathBuf;

#[cfg(windows)]
pub fn save_file() -> Result<Option<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetSaveFileNameW, OFN_EXPLORER, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST,
        OPENFILENAMEW,
    };
    let filter: Vec<u16> = "Pixloom project\0*.pixloom\0\0".encode_utf16().collect();
    let title: Vec<u16> = "Save artwork as\0".encode_utf16().collect();
    let extension: Vec<u16> = "pixloom\0".encode_utf16().collect();
    let mut path = vec![0u16; 32768];
    for (target, value) in path.iter_mut().zip("artwork.pixloom".encode_utf16()) {
        *target = value;
    }
    // Native synchronous dialog; all referenced buffers live through the call.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    dialog.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if unsafe { GetSaveFileNameW(&mut dialog) } != 0 {
        let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        return Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &path[..end],
        ))));
    }
    let error = unsafe { CommDlgExtendedError() };
    anyhow::ensure!(error == 0, "Windows save picker failed (code {error})");
    Ok(None)
}

#[cfg(not(windows))]
pub fn save_file() -> Result<Option<PathBuf>> {
    anyhow::bail!("Native Save As currently supports Windows")
}

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

#[cfg(windows)]
pub fn pick_references() -> Result<Vec<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = "Images and text documents\0*.png;*.jpg;*.jpeg;*.gif;*.webp;*.bmp;*.tif;*.tiff;*.txt;*.md;*.json;*.csv;*.svg;*.rs;*.js;*.ts;*.py;*.toml;*.yaml\0All files\0*.*\0\0".encode_utf16().collect();
    let title: Vec<u16> = "Attach reference files\0".encode_utf16().collect();
    let mut buffer = vec![0_u16; 65536];
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrFile = buffer.as_mut_ptr();
    dialog.nMaxFile = buffer.len() as u32;
    dialog.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_ALLOWMULTISELECT;
    // Buffers remain valid for the synchronous native dialog and its bounded writes.
    if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
        let error = unsafe { CommDlgExtendedError() };
        anyhow::ensure!(error == 0, "Reference picker failed (code {error})");
        return Ok(Vec::new());
    }
    let parts = buffer
        .split(|c| *c == 0)
        .take_while(|p| !p.is_empty())
        .map(|p| PathBuf::from(std::ffi::OsString::from_wide(p)))
        .collect::<Vec<_>>();
    Ok(if parts.len() > 1 {
        parts[1..].iter().map(|name| parts[0].join(name)).collect()
    } else {
        parts
    })
}
#[cfg(not(windows))]
pub fn pick_references() -> Result<Vec<PathBuf>> {
    anyhow::bail!("Native reference picker currently supports Windows")
}
