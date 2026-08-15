//! Application settings loaded from environment / `.env`.

use crate::error::{Error, Result};
use std::env;
use std::path::{Path, PathBuf};

/// X API credentials (OAuth 1.0a user context).
///
/// `Debug` redacts secret material so accidental `{:?}` / log fields cannot leak tokens.
#[derive(Clone, Default)]
pub struct Settings {
    pub api_key: String,
    pub api_secret: String,
    pub access_token: String,
    pub access_token_secret: String,
    pub bearer_token: String,
}

impl Settings {
    /// Load from process env and optional `.env` file (searched from cwd upward).
    ///
    /// File values override already-set process env so「刷新状态」can pick up
    /// a `.env` that was filled in after launch.
    pub fn load() -> Result<Self> {
        for candidate in [".env", "../.env", "../../.env"] {
            if Path::new(candidate).exists() {
                let _ = dotenvy::from_filename_override(candidate);
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

    /// Load from a specific `.env` path. File values override process env.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::Config(format!(
                "env file not found: {}",
                path.display()
            )));
        }
        dotenvy::from_filename_override(path).map_err(|e| Error::Config(e.to_string()))?;
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

    /// Presence flags only — never the raw secret values.
    pub fn credential_presence(&self) -> CredentialPresence {
        CredentialPresence {
            api_key: !self.api_key.is_empty(),
            api_secret: !self.api_secret.is_empty(),
            access_token: !self.access_token.is_empty(),
            access_token_secret: !self.access_token_secret.is_empty(),
            bearer_token: !self.bearer_token.is_empty(),
        }
    }
}

/// Which credential slots are non-empty. Safe to persist in logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CredentialPresence {
    pub api_key: bool,
    pub api_secret: bool,
    pub access_token: bool,
    pub access_token_secret: bool,
    pub bearer_token: bool,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::logging::mask_secret;
        f.debug_struct("Settings")
            .field("api_key", &mask_secret(&self.api_key))
            .field("api_secret", &mask_secret(&self.api_secret))
            .field("access_token", &mask_secret(&self.access_token))
            .field("access_token_secret", &mask_secret(&self.access_token_secret))
            .field("bearer_token", &mask_secret(&self.bearer_token))
            .finish()
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

    #[test]
    fn load_from_missing_file_errors() {
        let err = Settings::load_from("definitely-missing-xmanager.env").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_from_overrides_process_env() {
        std::env::set_var("X_API_KEY", "stale");
        std::env::set_var("X_API_SECRET", "stale");
        std::env::set_var("X_ACCESS_TOKEN", "stale");
        std::env::set_var("X_ACCESS_TOKEN_SECRET", "stale");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("override.env");
        std::fs::write(
            &path,
            "X_API_KEY=k\nX_API_SECRET=s\nX_ACCESS_TOKEN=t\nX_ACCESS_TOKEN_SECRET=ts\n",
        )
        .unwrap();
        let s = Settings::load_from(&path).unwrap();
        assert_eq!(s.api_key, "k");
        assert_eq!(s.api_secret, "s");
        assert_eq!(s.access_token, "t");
        assert_eq!(s.access_token_secret, "ts");
        for key in [
            "X_API_KEY",
            "X_API_SECRET",
            "X_ACCESS_TOKEN",
            "X_ACCESS_TOKEN_SECRET",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn debug_redacts_secrets() {
        let s = Settings {
            api_key: "key-value".into(),
            api_secret: "secret-value".into(),
            access_token: "token-value".into(),
            access_token_secret: "token-secret".into(),
            bearer_token: String::new(),
        };
        let rendered = format!("{s:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("<empty>"));
        assert!(!rendered.contains("key-value"));
        assert!(!rendered.contains("secret-value"));
        assert!(!rendered.contains("token-value"));
        assert!(!rendered.contains("token-secret"));
        let presence = s.credential_presence();
        assert!(presence.api_key);
        assert!(!presence.bearer_token);
    }
}
