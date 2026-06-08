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
    pub luarocks: Option<bool>,
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

#[derive(Debug, Deserialize, Default)]
pub struct ResolveConfig {
    pub overrides: Option<Vec<ModuleOverride>>,
    pub externals: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
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

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let config = match ext {
            "json" | "jsonc" => {
                let stripped = strip_jsonc_comments(&contents);
                serde_json::from_str(&stripped)
                    .map_err(|e| anyhow::anyhow!("failed to parse config: {}", e))?
            }
            _ => {
                // default to toml
                toml::from_str(&contents)
                    .map_err(|e| anyhow::anyhow!("failed to parse config: {}", e))?
            }
        };

        Ok(config)
    }
}

fn strip_jsonc_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            // string literal - pass through without stripping
            '"' => {
                out.push(c);
                while let Some(s) = chars.next() {
                    out.push(s);
                    if s == '\\' {
                        // escaped char - push next unconditionally
                        if let Some(escaped) = chars.next() {
                            out.push(escaped);
                        }
                    } else if s == '"' {
                        break;
                    }
                }
            }
            '/' => match chars.peek() {
                Some('/') => {
                    // line comment - skip to end of line
                    chars.next();
                    for c in chars.by_ref() {
                        if c == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    // block comment - skip to */
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '*'
                            && chars.peek() == Some(&'/') {
                                chars.next();
                                break;
                            }
                        // preserve newlines so line numbers stay accurate
                        if c == '\n' {
                            out.push('\n');
                        }
                    }
                }
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }

    out
}
