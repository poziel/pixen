//! Model preferences and the installed CLI's cached picker catalog. No process or network work.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Selection {
    pub model: String,
    pub effort: String,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            model: "gpt-6.1-sol".into(),
            effort: "low".into(),
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Model {
    pub id: String,
    pub label: String,
    pub efforts: Vec<String>,
    pub default_effort: String,
}
fn preferences_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .map(|p| p.join("Pixloom").join("artist.json"))
}
impl Selection {
    pub fn load() -> Self {
        preferences_path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|s| !s.model.is_empty() && !s.effort.is_empty())
            .unwrap_or_default()
    }
    pub fn save(&self) -> Result<()> {
        self.save_to(&preferences_path().context("No application settings directory")?)
    }
    fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let parent = path.parent().context("No settings parent")?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(file.as_file_mut(), self)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
    pub fn apply_thread(&self, mut params: Value) -> Value {
        params["model"] = json!(self.model);
        params["config"]["model_reasoning_effort"] = json!(self.effort);
        params
    }
    pub fn turn(&self, thread: &Option<String>, text: String) -> Value {
        json!({"threadId":thread,"model":self.model,"effort":self.effort,
            "summary":"auto","input":[{"type":"text","text":text}]})
    }
}
pub(crate) fn parse_catalog(value: &Value, live: bool) -> Vec<Model> {
    value[if live { "data" } else { "models" }]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            if live {
                m["hidden"] != true
            } else {
                m["visibility"] == "list"
            }
        })
        .filter_map(|m| {
            let id = m[if live { "model" } else { "slug" }].as_str()?.to_owned();
            let efforts = m[if live {
                "supportedReasoningEfforts"
            } else {
                "supported_reasoning_levels"
            }]
            .as_array()?
            .iter()
            .filter_map(|e| {
                e[if live { "reasoningEffort" } else { "effort" }]
                    .as_str()
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
            if efforts.is_empty() {
                return None;
            }
            Some(Model {
                label: m[if live { "displayName" } else { "display_name" }]
                    .as_str()
                    .unwrap_or(&id)
                    .to_owned(),
                default_effort: m[if live {
                    "defaultReasoningEffort"
                } else {
                    "default_reasoning_level"
                }]
                .as_str()
                .unwrap_or(&efforts[0])
                .to_owned(),
                id,
                efforts,
            })
        })
        .collect()
}
pub(crate) fn cached_catalog() -> Vec<Model> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|p| PathBuf::from(p).join(".codex"))
        });
    let mut models = home
        .and_then(|p| {
            let path = p.join("models_cache.json");
            if std::fs::metadata(&path).ok()?.len() > 16 * 1024 * 1024 {
                return None;
            }
            serde_json::from_slice::<Value>(&std::fs::read(path).ok()?).ok()
        })
        .map(|v| parse_catalog(&v, false))
        .unwrap_or_default();
    if models.is_empty() {
        models.push(Model {
            id: "gpt-6.1-sol".into(),
            label: "GPT 6.1 Sol".into(),
            efforts: vec!["low".into()],
            default_effort: "low".into(),
        });
    }
    models
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_filters_hidden_models_and_preserves_advertised_effort_order() {
        let catalog = parse_catalog(
            &json!({"data":[
                {"model":"visible","displayName":"Visible", "defaultReasoningEffort":"high",
                    "supportedReasoningEfforts":[{"reasoningEffort":"high"},{"reasoningEffort":"low"}]},
                {"model":"hidden","hidden":true,"supportedReasoningEfforts":[{"reasoningEffort":"low"}]}
            ]}),
            true,
        );
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].id, "visible");
        assert_eq!(catalog[0].efforts, vec!["high", "low"]);
        assert_eq!(catalog[0].default_effort, "high");
        let cached = parse_catalog(
            &json!({"models":[{"slug":"cached","visibility":"list",
            "supported_reasoning_levels":[{"effort":"medium"}],"default_reasoning_level":"medium"}]}),
            false,
        );
        assert_eq!(cached[0].efforts, vec!["medium"]);
    }
    #[test]
    fn preferences_and_requests_use_exact_model_and_effort() {
        let selection = Selection::default();
        assert_eq!(selection.model, "gpt-6.1-sol");
        assert_eq!(selection.effort, "low");
        let mut params = selection.apply_thread(json!({"config":{"shell_tool":"disabled"}}));
        assert_eq!(params["model"], "gpt-6.1-sol");
        assert_eq!(params["config"]["model_reasoning_effort"], "low");
        assert_eq!(params["config"]["shell_tool"], "disabled");
        params = selection.turn(&Some("thread".into()), "hello".into());
        assert_eq!(params["effort"], "low");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("artist.json");
        selection.save_to(&path).unwrap();
        let changed = Selection {
            model: "another-model".into(),
            effort: "high".into(),
        };
        changed.save_to(&path).unwrap();
        assert_eq!(
            serde_json::from_slice::<Selection>(&std::fs::read(path).unwrap()).unwrap(),
            changed
        );
    }
}
