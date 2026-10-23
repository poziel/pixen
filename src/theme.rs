//! Runtime theme registry. Set PIXLOOM_THEME to a registered name or JSON path.
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;
pub fn config() -> &'static Value {
    static THEME: OnceLock<Value> = OnceLock::new();
    THEME.get_or_init(|| {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/themes");
        let root = if source.is_dir() {
            source
        } else {
            std::env::current_exe()
                .unwrap_or_default()
                .with_file_name("themes")
        };
        let registry: Value = std::fs::read(root.join("manifest.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let selected = std::env::var("PIXLOOM_THEME")
            .ok()
            .unwrap_or_else(|| registry["default"].as_str().unwrap_or("2026-dark").into());
        let path = registry["themes"][&selected]
            .as_str()
            .map(|p| root.join(p))
            .unwrap_or_else(|| PathBuf::from(selected));
        let fallback: Value =
            serde_json::from_str(include_str!("../assets/themes/2026-dark.json")).unwrap();
        let mut value = fallback;
        if let Some(custom) = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        {
            if let Some(font) = custom["font_file"].as_str() {
                value["font_file"] = font.into();
            }
            for section in ["colors", "metrics"] {
                if let Some(entries) = custom[section].as_object() {
                    for (key, entry) in entries {
                        let valid = if section == "metrics" {
                            entry
                                .as_f64()
                                .is_some_and(|n| n.is_finite() && (0.0..=4096.0).contains(&n))
                        } else {
                            entry.as_str().is_some_and(|s| {
                                s.len() == 7
                                    && s.starts_with('#')
                                    && u32::from_str_radix(&s[1..], 16).is_ok()
                            })
                        };
                        if valid && !value[section][key].is_null() {
                            value[section][key] = entry.clone();
                        }
                    }
                }
            }
        }
        value
    })
}
