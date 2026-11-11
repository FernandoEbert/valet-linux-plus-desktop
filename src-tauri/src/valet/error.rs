use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("could not run `valet state --json`: {0}")]
    Spawn(#[from] std::io::Error),

    #[error("`valet state --json` exited with code {code:?}: {message}")]
    CommandFailed { code: Option<i32>, message: String },

    #[error("`valet state --json` returned invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("unsupported state schema_version {found:?} (supported: {supported})")]
    UnsupportedSchema { found: Option<u64>, supported: u64 },

    #[error("Valet is not installed: {0} not found")]
    NotInstalled(String),

    #[error("could not read {path}: {source}")]
    Read { path: String, source: std::io::Error },

    #[error("invalid {path}: {source}")]
    InvalidConfig { path: String, source: serde_json::Error },
}
