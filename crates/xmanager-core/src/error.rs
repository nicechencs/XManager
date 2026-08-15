//! Error types for xmanager-core.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing credentials: {0} (缺少凭证，请检查 .env)")]
    MissingCredentials(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("network error: {0} (网络错误)")]
    Network(String),

    #[error("X API HTTP {status}: {message} (X API 请求失败)")]
    Api { status: u16, message: String },

    #[error("rate limited; retry after {retry_after_secs}s (触发限流)")]
    RateLimited { retry_after_secs: u64 },

    #[error("parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn api(status: u16, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            message: message.into(),
        }
    }
}
