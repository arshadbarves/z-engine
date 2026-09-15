use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("invalid discovery limits: {0}")]
    InvalidOptions(String),
    #[error("cannot open workspace {path}: {source}")]
    Workspace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("workspace must be a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("project discovery cancelled")]
    Cancelled,
}
