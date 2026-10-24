//! Select theme file text; Frame owns parsing, token validation and application.
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

const DEFAULT_THEME: &str = include_str!("../../assets/themes/pixloom.yaml");
const MAX_THEME_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct ThemeSource {
    pub text: String,
    pub format: String,
}

fn fallback() -> ThemeSource {
    ThemeSource {
        text: DEFAULT_THEME.into(),
        format: "yaml".into(),
    }
}

fn load(root: &Path, selected: Option<&str>) -> ThemeSource {
    let registry: Value = std::fs::read(root.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let id = selected.unwrap_or_else(|| registry["default"].as_str().unwrap_or("pixloom"));
    let path = registry["themes"][id]
        .as_str()
        .map(|name| root.join(name))
        .unwrap_or_else(|| {
            if id == "pixloom" {
                root.join("pixloom.yaml")
            } else {
                id.into()
            }
        });
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = if extension == "yml" {
        "yaml"
    } else {
        extension.as_str()
    };
    if !["json", "jsonc", "json5", "yaml", "toml", "ini"].contains(&format) {
        return fallback();
    }
    use std::io::Read;
    let Some(bytes) = std::fs::File::open(path).ok().and_then(|file| {
        let mut bytes = Vec::new();
        file.take((MAX_THEME_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        (bytes.len() <= MAX_THEME_BYTES).then_some(bytes)
    }) else {
        return fallback();
    };
    match String::from_utf8(bytes) {
        Ok(text) => ThemeSource {
            text,
            format: format.into(),
        },
        Err(_) => fallback(),
    }
}

pub fn config() -> &'static ThemeSource {
    static THEME: OnceLock<ThemeSource> = OnceLock::new();
    THEME.get_or_init(|| {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/themes");
        let root = if source.is_dir() {
            source
        } else {
            std::env::current_exe()
                .unwrap_or_default()
                .with_file_name("themes")
        };
        load(&root, std::env::var("PIXLOOM_THEME").ok().as_deref())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_selects_raw_theme_without_color_translation() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("manifest.json"),
            r#"{"default":"custom","themes":{"custom":"custom.toml"}}"#,
        )
        .unwrap();
        let text = "[frame.text.input]\npadding = \"24px\"\n";
        std::fs::write(root.path().join("custom.toml"), text).unwrap();
        let source = load(root.path(), None);
        assert_eq!(source.text, text);
        assert_eq!(source.format, "toml");
    }

    #[test]
    fn explicit_file_paths_support_yaml_alias_and_bounded_fallbacks() {
        let root = tempfile::tempdir().unwrap();
        let custom = root.path().join("custom.YML");
        std::fs::write(&custom, "frame.accent: '#FE725E'").unwrap();
        assert_eq!(load(root.path(), custom.to_str()).format, "yaml");
        assert_eq!(load(root.path(), Some("missing.yaml")).text, DEFAULT_THEME);
        assert_eq!(load(root.path(), Some("theme.js")).text, DEFAULT_THEME);
        std::fs::write(&custom, vec![b' '; MAX_THEME_BYTES + 1]).unwrap();
        assert_eq!(load(root.path(), custom.to_str()).text, DEFAULT_THEME);
    }
}
