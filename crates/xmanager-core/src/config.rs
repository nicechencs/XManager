//! Application settings loaded from environment / `.env`.

use crate::error::{Error, Result};
use crate::paths::PathResolver;
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
    /// Load from process env and optional `.env` file.
    ///
    /// Search order is documented on [`PathResolver::env_file_candidates`].
    /// File values override already-set process env so「刷新状态」can pick up
    /// a `.env` that was filled in after launch.
    pub fn load() -> Result<Self> {
        Self::load_with(&PathResolver::from_process())
    }

    /// Like [`Self::load`], using an injected path snapshot (tests / packaging).
    pub fn load_with(paths: &PathResolver) -> Result<Self> {
        if let Some(path) = paths.find_env_file() {
            let _ = dotenvy::from_filename_override(&path);
        }
        Self::from_env()
    }

    /// Path of the `.env` that [`Self::load`] would pick, if any.
    pub fn discovered_env_file() -> Option<PathBuf> {
        PathResolver::from_process().find_env_file()
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

    /// Cheap local layout check. Does not call the network.
    ///
    /// User Access Tokens look like `{10-19 digit user id}-{secret}`. Putting
    /// that value in `X_API_KEY` (Consumer Key) is the mix-up that produces
    /// HTTP 401 while the sidebar still says the fields are filled.
    pub fn credential_layout(&self) -> CredentialLayout {
        if !self.missing_oauth1().is_empty() {
            return CredentialLayout::Missing;
        }
        let key_looks_like_token = looks_like_user_access_token(&self.api_key);
        let token_looks_like_token = looks_like_user_access_token(&self.access_token);
        if key_looks_like_token {
            return CredentialLayout::Swapped;
        }
        if !token_looks_like_token {
            return CredentialLayout::AccessTokenShapeMissing;
        }
        CredentialLayout::Ready
    }

    /// Ensure the four OAuth 1.0a fields are present and not obviously swapped.
    pub fn require_oauth1(&self) -> Result<()> {
        let missing = self.missing_oauth1();
        if !missing.is_empty() {
            return Err(Error::MissingCredentials(format!(
                "{} — copy .env.example to .env next to the app or in the user data directory, then fill values from console.x.com → Keys and tokens",
                missing.join(", ")
            )));
        }
        match self.credential_layout() {
            CredentialLayout::Swapped => Err(Error::CredentialMisplaced(
                CredentialLayout::Swapped.user_facing().to_string(),
            )),
            CredentialLayout::Missing
            | CredentialLayout::Ready
            | CredentialLayout::AccessTokenShapeMissing => Ok(()),
        }
    }

    /// True when all OAuth1 fields are non-empty and not obviously swapped.
    pub fn has_oauth1(&self) -> bool {
        self.require_oauth1().is_ok()
    }

    /// Resolve a path for export files (`<data-dir>/exports`).
    pub fn default_export_dir() -> PathBuf {
        PathResolver::from_process().export_dir()
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

/// Local (no-network) assessment of OAuth 1.0a field layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialLayout {
    /// At least one of the four OAuth1 fields is empty.
    Missing,
    /// All four filled; Access Token has the `{user_id}-…` shape.
    Ready,
    /// `X_API_KEY` looks like a User Access Token (`{10-19 digits}-…`).
    Swapped,
    /// All four filled, but `X_ACCESS_TOKEN` is missing the hyphenated user-id prefix.
    AccessTokenShapeMissing,
}

impl CredentialLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Ready => "ready",
            Self::Swapped => "swapped",
            Self::AccessTokenShapeMissing => "access_token_shape_missing",
        }
    }

    /// Sidebar / empty-state line. Never a generic「缺少凭证」for swapped fields.
    pub fn sidebar_zh(self) -> &'static str {
        match self {
            Self::Missing => "缺少 OAuth 凭证，请配置 .env（程序旁边或用户配置目录）",
            Self::Ready => "凭证已配置 ✓",
            Self::Swapped => {
                "字段已填但填反：X_API_KEY 里是 Access Token（形如 用户ID-…）。请到 console.x.com → Keys and tokens 把 Consumer Key 与 User Token 对调。"
            }
            Self::AccessTokenShapeMissing => {
                "凭证已填写，但 X_ACCESS_TOKEN 不像用户令牌（应为「用户ID-…」）。可点「刷新状态」验证。"
            }
        }
    }

    pub fn user_facing(self) -> &'static str {
        match self {
            Self::Missing => {
                "缺少 OAuth 凭证。请把 .env.example 复制为 .env，放到程序同一个文件夹或用户配置目录，从 console.x.com → Keys and tokens 填写四项。"
            }
            Self::Ready => "OAuth 凭证字段已按 Consumer Key / User Access Token 填好。",
            Self::Swapped => {
                "凭证字段填反：X_API_KEY（Consumer Key）里放了 User Access Token（形如 1234567890-…）。请到 console.x.com → Keys and tokens 核对：API Key 是 Consumer Key，Access Token 才是「用户ID-」开头的那一项。"
            }
            Self::AccessTokenShapeMissing => {
                "四项都已填写，但 X_ACCESS_TOKEN 缺少「用户ID-」前缀，可能把 Consumer Key 填进了 User Token。请到 console.x.com → Keys and tokens 重新复制。"
            }
        }
    }
}

/// User Access Token issued by X: `{10-19 digit user id}-{opaque}`.
pub fn looks_like_user_access_token(value: &str) -> bool {
    let value = value.trim();
    let Some((prefix, rest)) = value.split_once('-') else {
        return false;
    };
    let digits = prefix.len();
    (10..=19).contains(&digits) && prefix.bytes().all(|b| b.is_ascii_digit()) && !rest.is_empty()
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::logging::mask_secret;
        f.debug_struct("Settings")
            .field("api_key", &mask_secret(&self.api_key))
            .field("api_secret", &mask_secret(&self.api_secret))
            .field("access_token", &mask_secret(&self.access_token))
            .field(
                "access_token_secret",
                &mask_secret(&self.access_token_secret),
            )
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
    fn load_with_reads_env_next_to_exe() {
        let dir = tempfile::tempdir().unwrap();
        let exe_dir = dir.path().join("app");
        let cwd = dir.path().join("cwd");
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(
            exe_dir.join(".env"),
            "X_API_KEY=from-exe\nX_API_SECRET=s\nX_ACCESS_TOKEN=t\nX_ACCESS_TOKEN_SECRET=ts\n",
        )
        .unwrap();
        for key in [
            "X_API_KEY",
            "X_API_SECRET",
            "X_ACCESS_TOKEN",
            "X_ACCESS_TOKEN_SECRET",
        ] {
            std::env::remove_var(key);
        }
        let paths = PathResolver {
            cwd,
            exe: Some(exe_dir.join("xmanager.exe")),
            user_data: None,
            data_dir_override: None,
        };
        let s = Settings::load_with(&paths).unwrap();
        assert_eq!(s.api_key, "from-exe");
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

    #[test]
    fn detects_user_access_token_shape() {
        assert!(looks_like_user_access_token(
            "1234567890-abcdefghijklmnopqrstuvwxyz"
        ));
        assert!(looks_like_user_access_token("1234567890123456789-abc"));
        assert!(!looks_like_user_access_token("xvz1evFS4aoBKhx8g5"));
        assert!(!looks_like_user_access_token("123-shortprefix"));
        assert!(!looks_like_user_access_token("1234567890"));
        assert!(!looks_like_user_access_token(""));
        assert!(!looks_like_user_access_token("abcdefghij-klmnop"));
    }

    #[test]
    fn layout_detects_swapped_api_key() {
        let swapped = Settings {
            api_key: "1847200000000000000-ThisIsAnAccessToken".into(),
            api_secret: "consumer-secret".into(),
            access_token: "xvz1evFS4aoBKhx8g5".into(),
            access_token_secret: "token-secret".into(),
            bearer_token: String::new(),
        };
        assert_eq!(swapped.credential_layout(), CredentialLayout::Swapped);
        assert!(!swapped.has_oauth1());
        let err = swapped.require_oauth1().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("填反") || msg.contains("Access Token"),
            "{msg}"
        );
        assert!(!msg.contains("缺少凭证"), "{msg}");
    }

    #[test]
    fn layout_ready_when_token_has_user_id_prefix() {
        let ready = Settings {
            api_key: "xvz1evFS4aoBKhx8g5".into(),
            api_secret: "consumer-secret".into(),
            access_token: "1234567890-UserAccessTokenSecret".into(),
            access_token_secret: "token-secret".into(),
            bearer_token: String::new(),
        };
        assert_eq!(ready.credential_layout(), CredentialLayout::Ready);
        assert!(ready.has_oauth1());
    }

    #[test]
    fn layout_warns_when_access_token_lacks_hyphenated_user_id() {
        let odd = Settings {
            api_key: "xvz1evFS4aoBKhx8g5".into(),
            api_secret: "consumer-secret".into(),
            access_token: "not-a-user-id-token".into(),
            access_token_secret: "token-secret".into(),
            bearer_token: String::new(),
        };
        assert_eq!(
            odd.credential_layout(),
            CredentialLayout::AccessTokenShapeMissing
        );
        // Still allow a live whoami — format may change; do not block startup.
        assert!(odd.has_oauth1());
    }

    #[test]
    fn layout_missing_when_any_field_empty() {
        let s = Settings {
            api_key: "k".into(),
            api_secret: "s".into(),
            access_token: String::new(),
            access_token_secret: "ts".into(),
            bearer_token: String::new(),
        };
        assert_eq!(s.credential_layout(), CredentialLayout::Missing);
        assert!(!s.has_oauth1());
    }
}
