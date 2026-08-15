//! Application settings loaded from environment / `.env`.

use crate::error::{Error, Result};
use std::env;
use std::path::{Path, PathBuf};

/// X API credentials (OAuth 1.0a user context).
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub api_key: String,
    pub api_secret: String,
    pub access_token: String,
    pub access_token_secret: String,
    pub bearer_token: String,
}

impl Settings {
    /// Load from process env and optional `.env` file (searched from cwd upward).
    pub fn load() -> Result<Self> {
        // Best-effort load; ignore missing file.
        let _ = dotenvy::dotenv();
        // Also try project-relative paths common in this monorepo.
        for candidate in [".env", "../.env", "../../.env"] {
            if Path::new(candidate).exists() {
                let _ = dotenvy::from_filename(candidate);
                break;
            }
        }
        Self::from_env()
    }

    /// Load only from current process environment (no file IO).
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            api_key: env_var("X_API_KEY"),
            api_secret: env_var("X_API_SECRET"),
            access_token: env_var("X_ACCESS_TOKEN"),
            access_token_secret: env_var("X_ACCESS_TOKEN_SECRET"),
            bearer_token: env_var("X_BEARER_TOKEN"),
        })
    }

    /// Load from a specific `.env` path then process env.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            dotenvy::from_filename(path).map_err(|e| Error::Config(e.to_string()))?;
        }
        Self::from_env()
    }

    /// OAuth 1.0a keys that are empty (does not touch the network).
    pub fn missing_oauth1(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.api_key.is_empty() {
            missing.push("X_API_KEY");
        }
        if self.api_secret.is_empty() {
            missing.push("X_API_SECRET");
        }
        if self.access_token.is_empty() {
            missing.push("X_ACCESS_TOKEN");
        }
        if self.access_token_secret.is_empty() {
            missing.push("X_ACCESS_TOKEN_SECRET");
        }
        missing
    }

    /// Ensure the four OAuth 1.0a fields are present.
    pub fn require_oauth1(&self) -> Result<()> {
        let missing = self.missing_oauth1();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(Error::MissingCredentials(format!(
                "{} — copy .env.example to .env and fill values from developer.x.com",
                missing.join(", ")
            )))
        }
    }

    /// True when all OAuth1 fields are non-empty.
    pub fn has_oauth1(&self) -> bool {
        self.require_oauth1().is_ok()
    }

    /// Resolve a path for export files (default `./exports`).
    pub fn default_export_dir() -> PathBuf {
        PathBuf::from("exports")
    }
}

fn env_var(key: &str) -> String {
    env::var(key).unwrap_or_default().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credentials_error() {
        let s = Settings::default();
        assert!(s.require_oauth1().is_err());
    }
}
