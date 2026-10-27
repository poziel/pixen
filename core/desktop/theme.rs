//! Discover custom theme files; Frame owns parsing, validation, and application.
use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

const DEFAULT_THEME: &str = include_str!("../../assets/themes/pixen.yaml");
const MAX_THEME_BYTES: usize = 128 * 1024;
const MAX_THEME_FILES: usize = 128;

#[derive(Clone, Debug, Serialize)]
pub struct ThemeSource {
    pub text: String,
    pub format: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThemeFile {
    pub file_name: String,
    pub source: ThemeSource,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThemeConfig {
    pub builtin: ThemeSource,
    pub files: Vec<ThemeFile>,
    pub selected: Option<String>,
}

fn discover(root: &Path) -> Vec<ThemeFile> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_file())
                .map(|_| entry.path())
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let extension = path.extension()?.to_str()?.to_ascii_lowercase();
            let format = if extension == "yml" {
                "yaml"
            } else {
                extension.as_str()
            };
            if !["json", "jsonc", "json5", "yaml", "toml", "ini"].contains(&format) {
                return None;
            }
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .ok()?
                .take((MAX_THEME_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .ok()?;
            if bytes.len() > MAX_THEME_BYTES {
                return None;
            }
            Some(ThemeFile {
                file_name: path.file_name()?.to_str()?.into(),
                source: ThemeSource {
                    text: String::from_utf8(bytes).ok()?,
                    format: format.into(),
                },
            })
        })
        .take(MAX_THEME_FILES)
        .collect()
}

pub fn config() -> &'static ThemeConfig {
    static THEME: OnceLock<ThemeConfig> = OnceLock::new();
    THEME.get_or_init(|| {
        let mut files = crate::storage::layout().map(|layout| discover(&layout.themes)).unwrap_or_default();
        // Keep older extracted releases compatible; new customizations use the active profile.
        if let Ok(exe) = std::env::current_exe() {
            for legacy in discover(&exe.with_file_name("themes")) {
                if !files.iter().any(|file| file.file_name == legacy.file_name) { files.push(legacy); }
            }
        }
        ThemeConfig {
            builtin: ThemeSource {
                text: DEFAULT_THEME.into(),
                format: "yaml".into(),
            },
            files,
            selected: std::env::var("PIXEN_THEME")
                .or_else(|_| std::env::var("PIXLOOM_THEME"))
                .ok(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_supported_files_without_a_manifest_in_stable_order() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("z.YML"), "tokens: {}").unwrap();
        std::fs::write(root.path().join("a.toml"), "[tokens]").unwrap();
        std::fs::write(root.path().join("README.txt"), "Not a theme").unwrap();
        std::fs::create_dir(root.path().join("directory.yaml")).unwrap();
        let files = discover(root.path());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].file_name, "a.toml");
        assert_eq!(files[1].source.format, "yaml");
    }

    #[test]
    fn ignores_unreadable_formats_oversized_and_non_utf8_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("large.yaml"),
            vec![b' '; MAX_THEME_BYTES + 1],
        )
        .unwrap();
        std::fs::write(root.path().join("invalid.yaml"), [255]).unwrap();
        std::fs::write(root.path().join("script.js"), "{}").unwrap();
        assert!(discover(root.path()).is_empty());
        assert!(discover(&root.path().join("missing")).is_empty());
    }

    #[test]
    fn builtin_is_embedded_regardless_of_external_pixen_files() {
        assert!(DEFAULT_THEME.contains("id: pixen"));
    }
}
