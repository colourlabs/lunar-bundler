use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub bundle: Option<BundleConfig>,
    pub paths: Option<PathsConfig>,
    pub inject: Option<InjectConfig>,
    pub resolve: Option<ResolveConfig>,
}

#[derive(Debug, Deserialize)]
pub struct BundleConfig {
    pub entry: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub lua_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PathsConfig {
    pub search: Option<Vec<PathBuf>>,
}

#[derive(Debug, Deserialize)]
pub struct InjectConfig {
    pub top: Option<PathBuf>,
    pub bottom: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveConfig {
    pub overrides: Option<Vec<ModuleOverride>>,
}

#[derive(Debug, Deserialize)]
pub struct ModuleOverride {
    pub module: String,
    pub path: PathBuf,
}

impl Config {
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}