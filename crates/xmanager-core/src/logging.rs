//! Structured local logging for XManager.
//!
//! Two streams, two retention windows:
//! - [`Stream::App`]: diagnostics (API, export, config, UI). Default keep **14** days.
//! - [`Stream::Audit`]: irreversible / safety workflow (backup, preview, delete).
//!   Default keep **90** days. Info+ audit records are never dropped by `min_level`.
//!
//! Files: `{dir}/xmanager-{stream}-{YYYY-MM-DD}.log` (JSON Lines).
//! Event names: `{area}.{action}` lowercase dotted identifiers (see [`events`]).
//!
//! Secrets, OAuth tokens, and tweet text must never appear in fields or messages.

use crate::error::{Error, Result};
use chrono::{Local, NaiveDate};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// Relative log folder name under the data directory (`logs/`).
pub const DEFAULT_LOG_DIR: &str = "logs";
/// Operational / diagnostic retention.
pub const DEFAULT_APP_RETENTION_DAYS: u32 = 14;
/// Safety-audit retention (backup / preview / delete).
pub const DEFAULT_AUDIT_RETENTION_DAYS: u32 = 90;

const FILE_PREFIX: &str = "xmanager";
const FILE_EXT: &str = "log";

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Stable event identifiers. Pattern: `{area}.{action}` (lowercase, `[a-z0-9_]`).
pub mod events {
    pub const APP_START: &str = "app.start";
    pub const APP_CONFIG: &str = "app.config";
    pub const API_REQUEST: &str = "api.request";
    pub const API_RATE_LIMITED: &str = "api.rate_limited";
    pub const ACCOUNT_WHOAMI: &str = "account.whoami";
    pub const TIMELINE_FETCH: &str = "timeline.fetch";
    pub const TWEET_LOOKUP: &str = "tweet.lookup";
    pub const EXPORT_WRITE: &str = "export.write";
    pub const TWEET_DELETE: &str = "tweet.delete";
    pub const CLEANUP_BACKUP: &str = "cleanup.backup";
    pub const CLEANUP_PREVIEW: &str = "cleanup.preview";
    pub const CLEANUP_DELETE: &str = "cleanup.delete";
    pub const CLEANUP_CANCEL: &str = "cleanup.cancel";
    pub const UI_ERROR: &str = "ui.error";

    /// Every catalogued event name (used by tests and docs generation).
    pub const ALL: &[&str] = &[
        APP_START,
        APP_CONFIG,
        API_REQUEST,
        API_RATE_LIMITED,
        ACCOUNT_WHOAMI,
        TIMELINE_FETCH,
        TWEET_LOOKUP,
        EXPORT_WRITE,
        TWEET_DELETE,
        CLEANUP_BACKUP,
        CLEANUP_PREVIEW,
        CLEANUP_DELETE,
        CLEANUP_CANCEL,
        UI_ERROR,
    ];
}

/// Keys that must never be persisted. Matched case-insensitively as a substring.
const SENSITIVE_KEY_MARKERS: &[&str] = &[
    "api_key",
    "api_secret",
    "access_token",
    "access_token_secret",
    "bearer_token",
    "authorization",
    "password",
    "secret",
    "token",
];

/// Severity. `min_level` keeps records whose rank is >= the configured rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Destination file family. Also encoded in the filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    App,
    Audit,
}

impl Stream {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Audit => "audit",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "app" => Some(Self::App),
            "audit" => Some(Self::Audit),
            _ => None,
        }
    }
}

/// Machine-readable result of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Error,
    Cancel,
    Partial,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Cancel => "cancel",
            Self::Partial => "partial",
        }
    }
}

/// File-name and retention policy. Values come from env or defaults.
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub dir: PathBuf,
    pub min_level: Level,
    pub app_retention_days: u32,
    pub audit_retention_days: u32,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from(DEFAULT_LOG_DIR),
            min_level: Level::Info,
            app_retention_days: DEFAULT_APP_RETENTION_DAYS,
            audit_retention_days: DEFAULT_AUDIT_RETENTION_DAYS,
        }
    }
}

impl LogConfig {
    /// Read `XMANAGER_LOG_*` from the process environment (missing keys → defaults).
    ///
    /// When `XMANAGER_LOG_DIR` is unset, the directory is
    /// [`crate::paths::PathResolver::log_dir`] (repo `./logs` while developing,
    /// user data dir for a packaged app).
    pub fn from_env() -> Self {
        let mut cfg = Self::from_vars(std::env::vars());
        match std::env::var("XMANAGER_LOG_DIR") {
            Ok(ref dir) if !dir.trim().is_empty() => {}
            _ => {
                cfg.dir = crate::paths::PathResolver::from_process().log_dir();
            }
        }
        cfg
    }

    /// Apply `XMANAGER_LOG_*` overrides from an arbitrary key/value iterator.
    pub fn from_vars(vars: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut cfg = Self::default();
        for (key, value) in vars {
            match key.as_str() {
                "XMANAGER_LOG_DIR" => {
                    let dir = value.trim();
                    if !dir.is_empty() {
                        cfg.dir = PathBuf::from(dir);
                    }
                }
                "XMANAGER_LOG_LEVEL" => {
                    if let Some(parsed) = Level::parse(&value) {
                        cfg.min_level = parsed;
                    }
                }
                "XMANAGER_LOG_APP_RETENTION_DAYS" => {
                    if let Ok(n) = value.trim().parse::<u32>() {
                        cfg.app_retention_days = n;
                    }
                }
                "XMANAGER_LOG_AUDIT_RETENTION_DAYS" => {
                    if let Ok(n) = value.trim().parse::<u32>() {
                        cfg.audit_retention_days = n;
                    }
                }
                _ => {}
            }
        }
        cfg
    }

    pub fn retention_days(&self, stream: Stream) -> u32 {
        match stream {
            Stream::App => self.app_retention_days,
            Stream::Audit => self.audit_retention_days,
        }
    }
}

/// Daily log file name: `xmanager-{stream}-{YYYY-MM-DD}.log`.
pub fn log_file_name(stream: Stream, date: NaiveDate) -> String {
    format!(
        "{FILE_PREFIX}-{}-{}.{}",
        stream.as_str(),
        date.format("%Y-%m-%d"),
        FILE_EXT
    )
}

/// Parse `xmanager-{stream}-{YYYY-MM-DD}.log`. Rejects anything else.
pub fn parse_log_file_name(name: &str) -> Option<(Stream, NaiveDate)> {
    let stem = name.strip_suffix(&format!(".{FILE_EXT}"))?;
    let rest = stem.strip_prefix(&format!("{FILE_PREFIX}-"))?;
    let (stream_raw, date_raw) = rest.split_once('-')?;
    let stream = Stream::parse(stream_raw)?;
    // date is YYYY-MM-DD — the first split consumed only "app" / "audit".
    let date = NaiveDate::parse_from_str(date_raw, "%Y-%m-%d").ok()?;
    Some((stream, date))
}

/// `true` when `file_date` is strictly older than `today - retention_days`.
/// A file dated exactly `today - retention_days` is kept.
pub fn is_expired(file_date: NaiveDate, today: NaiveDate, retention_days: u32) -> bool {
    let cutoff = today - chrono::Duration::days(i64::from(retention_days));
    file_date < cutoff
}

/// `{area}.{action}` — at least two lowercase dotted segments.
pub fn is_valid_event_name(name: &str) -> bool {
    let mut parts = name.split('.');
    let first = parts.next();
    let second = parts.next();
    if first.is_none() || second.is_none() {
        return false;
    }
    std::iter::once(first.unwrap())
        .chain(std::iter::once(second.unwrap()))
        .chain(parts)
        .all(is_event_segment)
}

fn is_event_segment(part: &str) -> bool {
    let mut chars = part.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// True when a field key looks like a secret (substring match, case-insensitive).
pub fn is_sensitive_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    SENSITIVE_KEY_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
}

/// Replace a credential with a presence marker. Never returns the raw value.
pub fn mask_secret(value: &str) -> &'static str {
    if value.trim().is_empty() {
        "<empty>"
    } else {
        "<redacted>"
    }
}

/// Strip query string so pagination tokens do not land in Info/Debug URL fields.
pub fn sanitize_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

fn redact_fields(fields: &mut Map<String, Value>) {
    let keys: Vec<String> = fields.keys().cloned().collect();
    for key in keys {
        if !is_sensitive_key(&key) {
            continue;
        }
        // Presence flags (bool/number) are safe; only string-like secret material is stripped.
        match fields.get(&key) {
            Some(Value::Bool(_) | Value::Number(_) | Value::Null) => {}
            _ => {
                fields.insert(key, Value::String("[redacted]".into()));
            }
        }
    }
}

/// Suggested export / backup file stem (no extension).
/// Library: `xmanager-library-YYYYMMDD-HHMMSS`
/// Cleanup: `xmanager-cleanup-YYYYMMDD-HHMMSS`
pub fn artifact_file_stem(kind: &str, now: chrono::DateTime<Local>) -> String {
    format!("xmanager-{kind}-{}", now.format("%Y%m%d-%H%M%S"))
}

struct StreamWriter {
    date: NaiveDate,
    file: BufWriter<File>,
}

/// Process-wide file logger. Prefer [`init`] / [`event`]; tests may construct one.
pub struct Logger {
    config: LogConfig,
    inner: Mutex<LoggerInner>,
}

struct LoggerInner {
    app: Option<StreamWriter>,
    audit: Option<StreamWriter>,
}

impl Logger {
    pub fn open(config: LogConfig) -> Result<Self> {
        fs::create_dir_all(&config.dir)?;
        Ok(Self {
            config,
            inner: Mutex::new(LoggerInner {
                app: None,
                audit: None,
            }),
        })
    }

    pub fn config(&self) -> &LogConfig {
        &self.config
    }

    /// Delete files whose name-date is older than the stream retention.
    /// Returns the number of files removed.
    pub fn prune(&self, today: NaiveDate) -> Result<usize> {
        prune_dir(&self.config, today)
    }

    pub fn emit(&self, record: &LogRecord) {
        if !self.should_write(record.level, record.stream) {
            return;
        }
        let line = record.to_jsonl();
        if let Ok(mut guard) = self.inner.lock() {
            let _ = guard.write_line(&self.config, record.stream, record.level, &line);
        }
    }

    fn should_write(&self, level: Level, stream: Stream) -> bool {
        if stream == Stream::Audit && level >= Level::Info {
            return true;
        }
        level >= self.config.min_level
    }
}

impl LoggerInner {
    fn write_line(
        &mut self,
        config: &LogConfig,
        stream: Stream,
        level: Level,
        line: &str,
    ) -> Result<()> {
        let today = Local::now().date_naive();
        let slot = match stream {
            Stream::App => &mut self.app,
            Stream::Audit => &mut self.audit,
        };
        let needs_open = match slot {
            Some(w) => w.date != today,
            None => true,
        };
        if needs_open {
            *slot = Some(open_stream_file(&config.dir, stream, today)?);
        }
        let writer = slot.as_mut().expect("stream file just opened");
        writeln!(writer.file, "{line}")?;
        writer.file.flush()?;
        if stream == Stream::Audit && level >= Level::Info {
            writer.file.get_ref().sync_data()?;
        }
        Ok(())
    }
}

fn open_stream_file(dir: &Path, stream: Stream, date: NaiveDate) -> Result<StreamWriter> {
    fs::create_dir_all(dir)?;
    let path = dir.join(log_file_name(stream, date));
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    Ok(StreamWriter {
        date,
        file: BufWriter::new(file),
    })
}

fn prune_dir(config: &LogConfig, today: NaiveDate) -> Result<usize> {
    let dir = &config.dir;
    if !dir.exists() {
        return Ok(0);
    }
    let mut removed = 0usize;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some((stream, date)) = parse_log_file_name(name) else {
            continue;
        };
        if is_expired(date, today, config.retention_days(stream)) {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// One JSONL record. Built via [`event`] / [`EventBuilder`].
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub ts: chrono::DateTime<Local>,
    pub level: Level,
    pub stream: Stream,
    pub event: String,
    pub outcome: Option<Outcome>,
    pub fields: Map<String, Value>,
}

impl LogRecord {
    pub fn to_jsonl(&self) -> String {
        let mut fields = self.fields.clone();
        redact_fields(&mut fields);
        let mut obj = Map::new();
        obj.insert(
            "ts".into(),
            Value::String(self.ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        );
        obj.insert("level".into(), Value::String(self.level.as_str().into()));
        obj.insert("stream".into(), Value::String(self.stream.as_str().into()));
        obj.insert("event".into(), Value::String(self.event.clone()));
        if let Some(outcome) = self.outcome {
            obj.insert("outcome".into(), Value::String(outcome.as_str().into()));
        }
        if !fields.is_empty() {
            obj.insert("fields".into(), Value::Object(fields));
        }
        Value::Object(obj).to_string()
    }
}

/// Fluent builder used by application code.
pub struct EventBuilder {
    level: Level,
    stream: Stream,
    event: String,
    outcome: Option<Outcome>,
    fields: Map<String, Value>,
}

impl EventBuilder {
    pub fn outcome(mut self, outcome: Outcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    pub fn field(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let key = key.into();
        let encoded = serde_json::to_value(value).unwrap_or(Value::Null);
        self.fields.insert(key, encoded);
        self
    }

    pub fn field_opt<T: Serialize>(self, key: impl Into<String>, value: Option<T>) -> Self {
        match value {
            Some(v) => self.field(key, v),
            None => self,
        }
    }

    pub fn emit(self) {
        let record = LogRecord {
            ts: Local::now(),
            level: self.level,
            stream: self.stream,
            event: self.event,
            outcome: self.outcome,
            fields: self.fields,
        };
        emit_record(&record);
    }
}

/// Start a record. No-op until [`init`] / [`init_best_effort`] succeeds.
pub fn event(level: Level, stream: Stream, name: &'static str) -> EventBuilder {
    debug_assert!(is_valid_event_name(name), "invalid log event name: {name}");
    EventBuilder {
        level,
        stream,
        event: name.to_string(),
        outcome: None,
        fields: Map::new(),
    }
}

pub fn debug(stream: Stream, name: &'static str) -> EventBuilder {
    event(Level::Debug, stream, name)
}

pub fn info(stream: Stream, name: &'static str) -> EventBuilder {
    event(Level::Info, stream, name)
}

pub fn warn(stream: Stream, name: &'static str) -> EventBuilder {
    event(Level::Warn, stream, name)
}

pub fn error(stream: Stream, name: &'static str) -> EventBuilder {
    event(Level::Error, stream, name)
}

/// Open the process logger, prune expired files, and install the global sink.
pub fn init(config: LogConfig) -> Result<()> {
    let logger = Logger::open(config)?;
    let today = Local::now().date_naive();
    let _ = logger.prune(today);
    LOGGER
        .set(logger)
        .map_err(|_| Error::Other("logging already initialized".into()))?;
    Ok(())
}

/// Initialize from env. Failures print to stderr and disable file logging.
pub fn init_best_effort() -> bool {
    match init(LogConfig::from_env()) {
        Ok(()) => true,
        Err(err) => {
            eprintln!("xmanager: file logging disabled: {err}");
            false
        }
    }
}

pub fn is_initialized() -> bool {
    LOGGER.get().is_some()
}

pub fn emit_record(record: &LogRecord) {
    if let Some(logger) = LOGGER.get() {
        logger.emit(record);
        return;
    }
    if record.level >= Level::Warn {
        eprintln!("{}", record.to_jsonl());
    }
}

/// Elapsed milliseconds from a `SystemTime` start (saturating, 0 if clock went backwards).
pub fn elapsed_ms(started: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(started)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Compact map helper for tests / callers that already have JSON values.
pub fn fields_from_pairs(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

/// Convenience JSON object (re-export site for call sites that already depend on core).
pub fn json_value(value: impl Serialize) -> Value {
    json!(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn event_catalog_matches_naming_rule() {
        let mut seen = std::collections::BTreeSet::new();
        for name in events::ALL {
            assert!(is_valid_event_name(name), "invalid event: {name}");
            assert!(seen.insert(*name), "duplicate event: {name}");
        }
        assert!(!is_valid_event_name("fetch"));
        assert!(!is_valid_event_name("API.request"));
        assert!(!is_valid_event_name("api."));
        assert!(!is_valid_event_name(".request"));
        assert!(is_valid_event_name("cleanup.delete"));
    }

    #[test]
    fn file_name_roundtrip() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        let app = log_file_name(Stream::App, date);
        let audit = log_file_name(Stream::Audit, date);
        assert_eq!(app, "xmanager-app-2026-08-15.log");
        assert_eq!(audit, "xmanager-audit-2026-08-15.log");
        assert_eq!(parse_log_file_name(&app), Some((Stream::App, date)));
        assert_eq!(parse_log_file_name(&audit), Some((Stream::Audit, date)));
        assert_eq!(parse_log_file_name("readme.md"), None);
        assert_eq!(parse_log_file_name("xmanager-app-2026-08-15.jsonl"), None);
        assert_eq!(parse_log_file_name("xmanager-other-2026-08-15.log"), None);
        assert_eq!(parse_log_file_name("xmanager-app-20260815.log"), None);
    }

    #[test]
    fn retention_keeps_boundary_day() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        let keep = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let drop = NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        assert!(!is_expired(keep, today, 14));
        assert!(is_expired(drop, today, 14));
        assert!(!is_expired(today, today, 14));
        let audit_keep = NaiveDate::from_ymd_opt(2026, 5, 17).unwrap();
        let audit_drop = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();
        assert!(!is_expired(audit_keep, today, 90));
        assert!(is_expired(audit_drop, today, 90));
    }

    #[test]
    fn prune_removes_only_expired_matching_files() {
        let dir = tempdir().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        let cfg = LogConfig {
            dir: dir.path().to_path_buf(),
            ..LogConfig::default()
        };
        let keep_app = dir.path().join(log_file_name(
            Stream::App,
            today - chrono::Duration::days(14),
        ));
        let drop_app = dir.path().join(log_file_name(
            Stream::App,
            today - chrono::Duration::days(15),
        ));
        let keep_audit = dir.path().join(log_file_name(
            Stream::Audit,
            today - chrono::Duration::days(90),
        ));
        let drop_audit = dir.path().join(log_file_name(
            Stream::Audit,
            today - chrono::Duration::days(91),
        ));
        let stray = dir.path().join("notes.txt");
        for path in [&keep_app, &drop_app, &keep_audit, &drop_audit, &stray] {
            fs::write(path, "x\n").unwrap();
        }
        let removed = prune_dir(&cfg, today).unwrap();
        assert_eq!(removed, 2);
        assert!(keep_app.exists());
        assert!(!drop_app.exists());
        assert!(keep_audit.exists());
        assert!(!drop_audit.exists());
        assert!(stray.exists());
    }

    #[test]
    fn jsonl_schema_and_secret_redaction() {
        let mut fields = Map::new();
        fields.insert("count".into(), json!(3));
        fields.insert("has_access_token".into(), json!(true));
        fields.insert("access_token".into(), json!("secret-value"));
        fields.insert("Authorization".into(), json!("Bearer abc"));
        let record = LogRecord {
            ts: chrono::DateTime::parse_from_rfc3339("2026-08-15T15:20:01.123+08:00")
                .unwrap()
                .with_timezone(&Local),
            level: Level::Info,
            stream: Stream::Audit,
            event: events::CLEANUP_DELETE.to_string(),
            outcome: Some(Outcome::Partial),
            fields,
        };
        let line = record.to_jsonl();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["level"], "info");
        assert_eq!(v["stream"], "audit");
        assert_eq!(v["event"], "cleanup.delete");
        assert_eq!(v["outcome"], "partial");
        assert_eq!(v["fields"]["count"], 3);
        assert_eq!(v["fields"]["has_access_token"], true);
        assert_eq!(v["fields"]["access_token"], "[redacted]");
        assert_eq!(v["fields"]["Authorization"], "[redacted]");
        assert!(!line.contains("secret-value"));
        assert!(!line.contains("Bearer abc"));
        assert!(v["ts"].as_str().unwrap().contains("2026-08-15"));
    }

    #[test]
    fn logger_writes_app_and_audit_files() {
        let dir = tempdir().unwrap();
        let logger = Logger::open(LogConfig {
            dir: dir.path().to_path_buf(),
            min_level: Level::Info,
            ..LogConfig::default()
        })
        .unwrap();
        let mut fields = Map::new();
        fields.insert("count".into(), json!(2));
        logger.emit(&LogRecord {
            ts: Local::now(),
            level: Level::Info,
            stream: Stream::App,
            event: events::TIMELINE_FETCH.to_string(),
            outcome: Some(Outcome::Ok),
            fields: fields.clone(),
        });
        logger.emit(&LogRecord {
            ts: Local::now(),
            level: Level::Info,
            stream: Stream::Audit,
            event: events::CLEANUP_BACKUP.to_string(),
            outcome: Some(Outcome::Ok),
            fields,
        });
        // Debug below min_level must not appear on the app stream.
        logger.emit(&LogRecord {
            ts: Local::now(),
            level: Level::Debug,
            stream: Stream::App,
            event: events::API_REQUEST.to_string(),
            outcome: None,
            fields: Map::new(),
        });
        let today = Local::now().date_naive();
        let app_path = dir.path().join(log_file_name(Stream::App, today));
        let audit_path = dir.path().join(log_file_name(Stream::Audit, today));
        let app = fs::read_to_string(app_path).unwrap();
        let audit = fs::read_to_string(audit_path).unwrap();
        assert!(app.contains("timeline.fetch"));
        assert!(!app.contains("api.request"));
        assert!(audit.contains("cleanup.backup"));
    }

    #[test]
    fn audit_info_ignores_min_level() {
        let dir = tempdir().unwrap();
        let logger = Logger::open(LogConfig {
            dir: dir.path().to_path_buf(),
            min_level: Level::Error,
            ..LogConfig::default()
        })
        .unwrap();
        logger.emit(&LogRecord {
            ts: Local::now(),
            level: Level::Info,
            stream: Stream::Audit,
            event: events::CLEANUP_DELETE.to_string(),
            outcome: Some(Outcome::Ok),
            fields: Map::new(),
        });
        logger.emit(&LogRecord {
            ts: Local::now(),
            level: Level::Info,
            stream: Stream::App,
            event: events::TIMELINE_FETCH.to_string(),
            outcome: Some(Outcome::Ok),
            fields: Map::new(),
        });
        let today = Local::now().date_naive();
        let audit =
            fs::read_to_string(dir.path().join(log_file_name(Stream::Audit, today))).unwrap();
        assert!(audit.contains("cleanup.delete"));
        let app_path = dir.path().join(log_file_name(Stream::App, today));
        assert!(!app_path.exists() || fs::read_to_string(app_path).unwrap().is_empty());
    }

    #[test]
    fn sanitize_url_strips_query() {
        assert_eq!(
            sanitize_url("https://api.x.com/2/users/1/tweets?pagination_token=abc"),
            "https://api.x.com/2/users/1/tweets"
        );
        assert_eq!(sanitize_url("/users/me"), "/users/me");
    }

    #[test]
    fn artifact_stem_uses_kebab_and_kind() {
        let ts = chrono::TimeZone::with_ymd_and_hms(&Local, 2026, 8, 15, 15, 20, 1).unwrap();
        assert_eq!(
            artifact_file_stem("library", ts),
            "xmanager-library-20260815-152001"
        );
        assert_eq!(
            artifact_file_stem("cleanup", ts),
            "xmanager-cleanup-20260815-152001"
        );
    }

    #[test]
    fn mask_secret_never_echoes_value() {
        assert_eq!(mask_secret(""), "<empty>");
        assert_eq!(mask_secret("   "), "<empty>");
        assert_eq!(mask_secret("abcd"), "<redacted>");
        assert_ne!(mask_secret("abcd"), "abcd");
    }

    #[test]
    fn log_config_from_vars_reads_overrides() {
        let cfg = LogConfig::from_vars([
            ("XMANAGER_LOG_DIR".into(), "/tmp/xmanager-logs".into()),
            ("XMANAGER_LOG_LEVEL".into(), "debug".into()),
            ("XMANAGER_LOG_APP_RETENTION_DAYS".into(), "7".into()),
            ("XMANAGER_LOG_AUDIT_RETENTION_DAYS".into(), "30".into()),
            ("UNRELATED".into(), "x".into()),
        ]);
        assert_eq!(cfg.dir, PathBuf::from("/tmp/xmanager-logs"));
        assert_eq!(cfg.min_level, Level::Debug);
        assert_eq!(cfg.app_retention_days, 7);
        assert_eq!(cfg.audit_retention_days, 30);
    }

    #[test]
    fn log_config_from_vars_ignores_invalid_overrides() {
        let cfg = LogConfig::from_vars([
            ("XMANAGER_LOG_DIR".into(), "   ".into()),
            ("XMANAGER_LOG_LEVEL".into(), "verbose".into()),
            ("XMANAGER_LOG_APP_RETENTION_DAYS".into(), "nope".into()),
            ("XMANAGER_LOG_AUDIT_RETENTION_DAYS".into(), "".into()),
        ]);
        assert_eq!(cfg.dir, PathBuf::from(DEFAULT_LOG_DIR));
        assert_eq!(cfg.min_level, Level::Info);
        assert_eq!(cfg.app_retention_days, DEFAULT_APP_RETENTION_DAYS);
        assert_eq!(cfg.audit_retention_days, DEFAULT_AUDIT_RETENTION_DAYS);
    }

    #[test]
    fn level_and_stream_parse() {
        assert_eq!(Level::parse("DEBUG"), Some(Level::Debug));
        assert_eq!(Level::parse("warning"), Some(Level::Warn));
        assert_eq!(Level::parse("nope"), None);
        assert_eq!(Stream::parse("app"), Some(Stream::App));
        assert_eq!(Stream::parse("audit"), Some(Stream::Audit));
        assert_eq!(Stream::parse("other"), None);
        assert_eq!(Outcome::Partial.as_str(), "partial");
    }

    #[test]
    fn concurrent_writes_keep_one_json_object_per_line() {
        use std::sync::Arc;
        use std::thread;
        let dir = tempdir().unwrap();
        let logger = Arc::new(
            Logger::open(LogConfig {
                dir: dir.path().to_path_buf(),
                min_level: Level::Debug,
                ..LogConfig::default()
            })
            .unwrap(),
        );
        let mut handles = Vec::new();
        for i in 0..8 {
            let logger = Arc::clone(&logger);
            handles.push(thread::spawn(move || {
                for n in 0..20 {
                    logger.emit(&LogRecord {
                        ts: Local::now(),
                        level: Level::Info,
                        stream: Stream::App,
                        event: events::API_REQUEST.to_string(),
                        outcome: Some(Outcome::Ok),
                        fields: {
                            let mut m = Map::new();
                            m.insert("worker".into(), json!(i));
                            m.insert("n".into(), json!(n));
                            m
                        },
                    });
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let today = Local::now().date_naive();
        let body = fs::read_to_string(dir.path().join(log_file_name(Stream::App, today))).unwrap();
        let lines: Vec<&str> = body.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 160);
        for line in lines {
            let v: Value = serde_json::from_str(line).expect("jsonl line");
            assert_eq!(v["event"], "api.request");
            assert!(v["ts"].as_str().is_some());
        }
    }
}
