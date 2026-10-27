//! Atomic settings JSON storage. Frame owns the document schema and write scheduling.
use crate::{Reply, error};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tauri::Manager;

const MAX_BYTES: u64 = 1024 * 1024;

fn file_path(portable: Option<&Path>, normal: &Path) -> PathBuf {
    portable
        .map(|root| root.join("config"))
        .unwrap_or_else(|| normal.to_owned())
        .join("settings.json")
}
fn settings_path(app: &tauri::AppHandle) -> Reply<PathBuf> {
    let portable = pixen::storage::portable_root().map_err(error)?;
    let normal = app.path().app_config_dir().map_err(error)?;
    Ok(file_path(portable.as_deref(), &normal))
}
fn read(path: &Path) -> std::io::Result<Option<String>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut text = String::new();
    file.take(MAX_BYTES + 1).read_to_string(&mut text)?;
    if text.len() as u64 > MAX_BYTES {
        return Err(std::io::Error::other("Settings file exceeds 1 MiB"));
    }
    Ok(Some(text))
}
fn write(path: &Path, json: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        json.len() as u64 <= MAX_BYTES,
        "Settings file exceeds 1 MiB"
    );
    let value: serde_json::Value = serde_json::from_str(json)?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Settings must be a JSON object"))?;
    anyhow::ensure!(
        object
            .values()
            .all(|value| value.is_string() || value.is_boolean() || value.is_number()),
        "Settings values must be strings, numbers, or booleans"
    );
    write_atomic(path, json)
}
fn write_keybindings(path: &Path, json: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        json.len() as u64 <= MAX_BYTES,
        "Keybindings file exceeds 1 MiB"
    );
    let value: serde_json::Value = serde_json::from_str(json)?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Keybindings must be a JSON object"))?;
    anyhow::ensure!(
        object.iter().all(|(id, value)| !id.trim().is_empty()
            && (value.is_null() || value.as_str().is_some_and(|text| !text.trim().is_empty()))),
        "Keybindings must map command IDs to shortcuts or null"
    );
    write_atomic(path, json)
}
fn write_atomic(path: &Path, json: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| anyhow::anyhow!("Settings have no parent directory"))?,
    )?;
    pixen::output::Prepared::new(path, |file| {
        file.write_all(json.as_bytes())?;
        Ok(())
    })?
    .commit(true)
}
#[tauri::command]
pub fn read_settings_file(app: tauri::AppHandle) -> Reply<Option<String>> {
    read(&settings_path(&app)?).map_err(error)
}
#[tauri::command]
pub fn write_settings_file(app: tauri::AppHandle, json: String) -> Reply<()> {
    write(&settings_path(&app)?, &json).map_err(error)
}

#[tauri::command]
pub fn read_keybindings_file(app: tauri::AppHandle) -> Reply<Option<String>> {
    read(&settings_path(&app)?.with_file_name("keybindings.json")).map_err(error)
}
#[tauri::command]
pub fn write_keybindings_file(app: tauri::AppHandle, json: String) -> Reply<()> {
    write_keybindings(
        &settings_path(&app)?.with_file_name("keybindings.json"),
        &json,
    )
    .map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keybindings_round_trip_in_both_modes_without_changing_settings() {
        let root = tempfile::tempdir().unwrap();
        for portable in [None, Some(root.path())] {
            let settings = file_path(portable, &root.path().join("user"));
            let path = settings.with_file_name("keybindings.json");
            write(&settings, "{}").unwrap();
            assert_eq!(read(&path).unwrap(), None);
            let json = r#"{"pixen.open":"Ctrl+L","pixen.new":null}"#;
            write_keybindings(&path, json).unwrap();
            assert_eq!(read(&path).unwrap().as_deref(), Some(json));
            for invalid in ["{bad", r#"{"pixen.open":false}"#, r#"{"pixen.open":" "}"#] {
                assert!(write_keybindings(&path, invalid).is_err());
                assert_eq!(read(&path).unwrap().as_deref(), Some(json));
            }
            assert_eq!(read(&settings).unwrap().as_deref(), Some("{}"));
        }
    }
    #[test]
    fn normal_and_portable_paths_are_separate() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user-appdata");
        let portable = root.path().join("application/data");
        assert_eq!(file_path(None, &user), user.join("settings.json"));
        assert_eq!(
            file_path(Some(&portable), &user),
            portable.join("config/settings.json")
        );
    }
    #[test]
    fn both_modes_create_load_and_atomically_replace_configuration() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user-appdata");
        let portable = root.path().join("application/data");
        for path in [file_path(None, &user), file_path(Some(&portable), &user)] {
            assert_eq!(read(&path).unwrap(), None);
            write(&path, "{\"pixen.recoveryInterval\":45}").unwrap();
            assert_eq!(
                read(&path).unwrap().unwrap(),
                "{\"pixen.recoveryInterval\":45}"
            );
            write(&path, "{\"pixen.background\":\"black\"}").unwrap();
            assert_eq!(
                read(&path).unwrap().unwrap(),
                "{\"pixen.background\":\"black\"}"
            );
            assert!(write(&path, "{bad json").is_err());
            assert!(write(&path, "{\"invalid\":null}").is_err());
            assert_eq!(
                read(&path).unwrap().unwrap(),
                "{\"pixen.background\":\"black\"}"
            );
        }
    }
}
