use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BundlerError {
    #[error("entry file does not exist: {0}")]
    EntryNotFound(PathBuf),

    #[error("cannot resolve module '{module}' required by '{requirer}'")]
    UnresolvedModule { module: String, requirer: PathBuf },

    #[error("circular dependency detected: {cycle}")]
    CircularDependency { cycle: String },

    #[error("failed to parse '{path}': {reason}")]
    ParseError { path: PathBuf, reason: String },

    #[error("failed to read '{path}': {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read injection file '{path}': {source}")]
    InjectionReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
