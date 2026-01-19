use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub agent_prompt: Option<String>,
    #[serde(default)]
    pub tool_config: HashMap<String, ToolSettings>,
    #[serde(default)]
    pub recipes: HashMap<String, Recipe>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ToolSettings {
    #[serde(default)]
    pub auto_allow: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Recipe {
    pub prompt: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub persona: Option<String>,
    pub yolo: Option<bool>,
}

impl Config {
    pub fn load() -> Self {
        let paths = ["picocode.yaml", "picocode.yml"];
        for path in paths {
            if Path::new(path).exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(config) = serde_yaml::from_str::<Config>(&content) {
                        return config;
                    }
                }
            }
        }
        Config::default()
    }

    pub fn get_bash_auto_allow(&self) -> Vec<String> {
        self.tool_config
            .get("bash")
            .map(|s| s.auto_allow.clone())
            .unwrap_or_default()
    }
}
