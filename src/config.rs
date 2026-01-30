use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub agent_prompt: Option<String>,
    #[serde(default)]
    pub agent_prompt_file: Option<String>,
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
    pub prompt: Option<String>,
    pub prompt_file: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub persona: Option<String>,
    pub yolo: Option<bool>,
    #[serde(default)]
    pub quiet: bool,
    /// If set, response is treated as error when it matches this regex. Process exits with error.
    #[serde(default)]
    pub error_if: Option<String>,
}

impl Recipe {
    /// Returns true if the response matches the recipe's error_if regex.
    pub fn is_error(&self, response: &str) -> crate::Result<bool> {
        let Some(pattern) = &self.error_if else {
            return Ok(false);
        };
        let re = Regex::new(pattern)?;
        Ok(re.is_match(response))
    }
}

impl Config {
    /// Load config from the given path, or from picocode.yaml/picocode.yml in the current directory if path is None.
    pub fn load(path: Option<&str>) -> crate::Result<Self> {
        if let Some(path) = path {
            let p = Path::new(path);
            let content = std::fs::read_to_string(p).map_err(crate::PicocodeError::Io)?;
            let config = serde_yaml::from_str::<Config>(&content)
                .map_err(crate::PicocodeError::Yaml)?;
            return Ok(config);
        }
        let paths = ["picocode.yaml", "picocode.yml"];
        for path in paths {
            let p = Path::new(path);
            if p.exists() {
                let content = std::fs::read_to_string(p).map_err(crate::PicocodeError::Io)?;
                let config = serde_yaml::from_str::<Config>(&content)
                    .map_err(crate::PicocodeError::Yaml)?;
                return Ok(config);
            }
        }
        Ok(Config::default())
    }

    pub fn get_bash_auto_allow(&self) -> Vec<String> {
        self.tool_config
            .get("bash")
            .map(|s| s.auto_allow.clone())
            .unwrap_or_default()
    }
}

pub fn read_prompt(prompt: Option<String>, prompt_file: Option<String>) -> crate::Result<Option<String>> {
    if let Some(file_path) = prompt_file {
        let path = Path::new(&file_path);
        let content = std::fs::read_to_string(path).map_err(crate::PicocodeError::Io)?;
        Ok(Some(content))
    } else {
        Ok(prompt)
    }
}
