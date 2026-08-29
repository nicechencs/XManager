//! Error types for xmanager-core.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing credentials: {0} (缺少凭证，请检查 .env)")]
    MissingCredentials(String),

    #[error("{0}")]
    CredentialMisplaced(String),

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

    /// Operator-facing Chinese hint appended to fetch / whoami / delete errors.
    pub fn user_facing(&self) -> String {
        match self {
            Self::MissingCredentials(_) => {
                format!("{self}。请把 .env.example 复制为 .env，从 console.x.com → Keys and tokens 填写四项。")
            }
            Self::CredentialMisplaced(msg) => msg.clone(),
            Self::Api { status: 401, .. } => {
                format!(
                    "{self}。凭证被拒绝：常见原因是 X_API_KEY 与 X_ACCESS_TOKEN 填反（Access Token 形如 用户ID-…），或 Token 已失效。请到 console.x.com → Keys and tokens 重新复制，不要把 Access Token 填进 X_API_KEY。"
                )
            }
            Self::Api { status: 403, .. } => {
                format!(
                    "{self}。权限不足：确认 App 已开通按量付费（pay-per-use）访问；删除需要 Read and write，改权限后必须重新生成 User Access Token。"
                )
            }
            Self::RateLimited { retry_after_secs } => {
                format!(
                    "触发限流，建议 {retry_after_secs} 秒后再试。界面不会自动长时间等待，请稍后再点拉取。"
                )
            }
            other => other.to_string(),
        }
    }
}
