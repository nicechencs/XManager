//! Where XManager looks for `.env` and writes logs / exports.
//!
//! Packaged desktop builds often start with a cwd that is not the binary's
//! folder (`/` on macOS, `System32` on some Windows launches). Developers
//! still `cargo run` from the repo, so checkout-relative `logs/` and `exports/`
//! stay the default when the cwd looks like a project.

use std::env;
use std::path::{Path, PathBuf};

/// Folder name under the platform user-data directory.
pub const APP_DIR_NAME: &str = "XManager";
pub const ENV_FILE_NAME: &str = ".env";
pub const EXPORTS_DIR_NAME: &str = "exports";
pub const LOGS_DIR_NAME: &str = "logs";
/// Overrides the data root (`logs/` and `exports/` live underneath).
pub const DATA_DIR_ENV: &str = "XMANAGER_DATA_DIR";

/// Snapshot of locations used to resolve config and data directories.
///
/// Production code uses [`PathResolver::from_process`]; tests build one with
/// fake cwd / exe / user-data paths.
#[derive(Debug, Clone)]
pub struct PathResolver {
    pub cwd: PathBuf,
    /// Running binary, if known.
    pub exe: Option<PathBuf>,
    /// Platform user data dir (`%APPDATA%\XManager` etc.), if known.
    pub user_data: Option<PathBuf>,
    /// `XMANAGER_DATA_DIR` override.
    pub data_dir_override: Option<PathBuf>,
}

impl PathResolver {
    pub fn from_process() -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let data_dir_override = env::var_os(DATA_DIR_ENV)
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        Self {
            cwd,
            exe: env::current_exe().ok(),
            user_data: platform_user_data_dir(),
            data_dir_override,
        }
    }

    /// `.env` candidates; [`Self::find_env_file`] picks the first existing file.
    ///
    /// Order:
    /// 1. Next to the executable (skipped when the binary lives under Cargo
    ///    `target/`, so `cargo run` does not look in `target/debug`)
    /// 2. macOS: folder that contains `XManager.app`, plus `Contents/Resources`
    /// 3. `XMANAGER_DATA_DIR/.env` when that override is set
    /// 4. `{cwd}/.env`, `{cwd}/../.env`, `{cwd}/../../.env`
    /// 5. User data dir
    pub fn env_file_candidates(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut push = |p: PathBuf| {
            if !out.iter().any(|existing| existing == &p) {
                out.push(p);
            }
        };

        for dir in self.executable_config_dirs() {
            push(dir.join(ENV_FILE_NAME));
        }
        if let Some(dir) = &self.data_dir_override {
            push(dir.join(ENV_FILE_NAME));
        }
        push(self.cwd.join(ENV_FILE_NAME));
        if let Some(parent) = self.cwd.parent() {
            push(parent.join(ENV_FILE_NAME));
            if let Some(grand) = parent.parent() {
                push(grand.join(ENV_FILE_NAME));
            }
        }
        if let Some(dir) = &self.user_data {
            push(dir.join(ENV_FILE_NAME));
        }
        out
    }

    pub fn find_env_file(&self) -> Option<PathBuf> {
        self.env_file_candidates()
            .into_iter()
            .find(|path| path.is_file())
    }

    /// Root for `logs/` and `exports/`.
    ///
    /// 1. `XMANAGER_DATA_DIR` when set
    /// 2. cwd if it looks like a checkout (`Cargo.toml`, `.env`, or `.env.example`)
    /// 3. directory of the executable when a `.env` sits next to it (unpacked zip)
    /// 4. macOS: folder containing `XManager.app` when `.env` is there
    /// 5. user data dir
    /// 6. cwd
    pub fn data_dir(&self) -> PathBuf {
        if let Some(dir) = &self.data_dir_override {
            return dir.clone();
        }
        if looks_like_project_dir(&self.cwd) {
            return self.cwd.clone();
        }
        for dir in self.executable_config_dirs() {
            if dir.join(ENV_FILE_NAME).is_file() {
                return dir;
            }
        }
        if let Some(dir) = &self.user_data {
            return dir.clone();
        }
        self.cwd.clone()
    }

    pub fn export_dir(&self) -> PathBuf {
        self.data_dir().join(EXPORTS_DIR_NAME)
    }

    pub fn log_dir(&self) -> PathBuf {
        self.data_dir().join(LOGS_DIR_NAME)
    }

    /// Short Chinese hint for empty states / errors. Lists real folders when known.
    pub fn env_placement_hint_zh(&self) -> String {
        let mut places: Vec<String> = Vec::new();
        for dir in self.executable_config_dirs() {
            push_unique(&mut places, dir.display().to_string());
        }
        if looks_like_project_dir(&self.cwd) {
            push_unique(&mut places, self.cwd.display().to_string());
        }
        if let Some(dir) = &self.data_dir_override {
            push_unique(&mut places, dir.display().to_string());
        }
        if let Some(dir) = &self.user_data {
            push_unique(&mut places, dir.display().to_string());
        }
        if places.is_empty() {
            places.push("程序同一个文件夹或当前工作目录".into());
        }
        format!(
            "请把 .env 放到 {}，然后点「刷新状态」。Consumer Key 不要填 Access Token。",
            places.join(" 或 ")
        )
    }

    fn executable_config_dirs(&self) -> Vec<PathBuf> {
        let Some(exe) = &self.exe else {
            return Vec::new();
        };
        let Some(exe_dir) = exe.parent() else {
            return Vec::new();
        };
        if is_cargo_target_dir(exe_dir) {
            return Vec::new();
        }
        let mut dirs = vec![exe_dir.to_path_buf()];
        if let Some(bundle_parent) = macos_app_bundle_parent(exe_dir) {
            dirs.push(bundle_parent);
            if let Some(contents) = exe_dir.parent() {
                dirs.push(contents.join("Resources"));
            }
        }
        dirs
    }
}

/// Platform user data directory for this app, if the home/config env is set.
pub fn platform_user_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("APPDATA").map(|p| PathBuf::from(p).join(APP_DIR_NAME))
    }
    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME").map(|p| {
            PathBuf::from(p)
                .join("Library")
                .join("Application Support")
                .join(APP_DIR_NAME)
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join(APP_DIR_NAME))
    }
}

/// Process-wide `.env` placement hint (empty-state copy).
pub fn env_placement_hint_zh() -> String {
    PathResolver::from_process().env_placement_hint_zh()
}

pub fn looks_like_project_dir(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file()
        || dir.join(ENV_FILE_NAME).is_file()
        || dir.join(".env.example").is_file()
}

/// `true` when `dir` is a Cargo build output folder (`target/debug`,
/// `target/<triple>/release`, `.../deps`, …).
fn is_cargo_target_dir(dir: &Path) -> bool {
    let names: Vec<String> = dir
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_ascii_lowercase()))
        .collect();
    let Some(i) = names.iter().position(|n| n == "target") else {
        return false;
    };
    let rest = &names[i + 1..];
    match rest {
        [profile, ..] if is_cargo_profile(profile) => true,
        [_, profile, ..] if is_cargo_profile(profile) => true,
        _ => false,
    }
}

fn is_cargo_profile(name: &str) -> bool {
    name == "debug" || name == "release"
}

/// If `exe_dir` is `Foo.app/Contents/MacOS`, return the directory that contains `Foo.app`.
fn macos_app_bundle_parent(exe_dir: &Path) -> Option<PathBuf> {
    let macos = exe_dir.file_name()?.to_str()?;
    if !macos.eq_ignore_ascii_case("macos") {
        return None;
    }
    let contents = exe_dir.parent()?;
    if !contents
        .file_name()?
        .to_str()?
        .eq_ignore_ascii_case("contents")
    {
        return None;
    }
    let app = contents.parent()?;
    let is_app = app
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("app"));
    if !is_app {
        return None;
    }
    app.parent().map(Path::to_path_buf)
}

fn push_unique(places: &mut Vec<String>, value: String) {
    if !places.iter().any(|existing| existing == &value) {
        places.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_env(dir: &Path, api_key: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(ENV_FILE_NAME);
        fs::write(
            &path,
            format!(
                "X_API_KEY={api_key}\nX_API_SECRET=s\nX_ACCESS_TOKEN=t\nX_ACCESS_TOKEN_SECRET=ts\n"
            ),
        )
        .unwrap();
        path
    }

    fn resolver(cwd: &Path, exe: Option<PathBuf>, user_data: Option<PathBuf>) -> PathResolver {
        PathResolver {
            cwd: cwd.to_path_buf(),
            exe,
            user_data,
            data_dir_override: None,
        }
    }

    #[test]
    fn env_next_to_exe_wins_over_cwd_and_user_data() {
        let root = tempfile::tempdir().unwrap();
        let exe_dir = root.path().join("app");
        let cwd = root.path().join("cwd");
        let user = root.path().join("appdata");
        let expected = write_env(&exe_dir, "from-exe");
        write_env(&cwd, "from-cwd");
        write_env(&user, "from-user");

        let found = resolver(&cwd, Some(exe_dir.join("xmanager.exe")), Some(user)).find_env_file();
        assert_eq!(found.as_deref(), Some(expected.as_path()));
    }

    #[test]
    fn cargo_target_exe_is_skipped_so_cwd_env_wins() {
        let root = tempfile::tempdir().unwrap();
        let target_dir = root.path().join("target").join("debug");
        let cwd = root.path().join("repo");
        let user = root.path().join("appdata");
        write_env(&target_dir, "from-target");
        let expected = write_env(&cwd, "from-cwd");
        write_env(&user, "from-user");

        let found =
            resolver(&cwd, Some(target_dir.join("xmanager.exe")), Some(user)).find_env_file();
        assert_eq!(found.as_deref(), Some(expected.as_path()));
    }

    #[test]
    fn user_data_env_used_when_cwd_has_none() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("empty-cwd");
        let user = root.path().join("appdata");
        fs::create_dir_all(&cwd).unwrap();
        let expected = write_env(&user, "from-user");

        let found = resolver(&cwd, None, Some(user)).find_env_file();
        assert_eq!(found.as_deref(), Some(expected.as_path()));
    }

    #[test]
    fn macos_app_bundle_parent_env_is_a_candidate() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Downloads");
        let macos = downloads
            .join("XManager.app")
            .join("Contents")
            .join("MacOS");
        fs::create_dir_all(&macos).unwrap();
        let expected = write_env(&downloads, "from-bundle-parent");
        let cwd = root.path().join("not-a-project");
        fs::create_dir_all(&cwd).unwrap();

        let found = resolver(&cwd, Some(macos.join("xmanager")), None).find_env_file();
        assert_eq!(found.as_deref(), Some(expected.as_path()));
    }

    #[test]
    fn data_dir_override_wins() {
        let root = tempfile::tempdir().unwrap();
        let override_dir = root.path().join("custom");
        fs::create_dir_all(&override_dir).unwrap();
        let paths = PathResolver {
            cwd: root.path().join("cwd"),
            exe: None,
            user_data: Some(root.path().join("appdata")),
            data_dir_override: Some(override_dir.clone()),
        };
        assert_eq!(paths.data_dir(), override_dir);
        assert_eq!(paths.export_dir(), override_dir.join("exports"));
        assert_eq!(paths.log_dir(), override_dir.join("logs"));
    }

    #[test]
    fn data_dir_uses_project_cwd() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("repo");
        fs::create_dir_all(&cwd).unwrap();
        fs::write(cwd.join("Cargo.toml"), "[package]\nname=\"demo\"\n").unwrap();
        let paths = resolver(
            &cwd,
            Some(cwd.join("target").join("debug").join("xmanager")),
            None,
        );
        assert_eq!(paths.data_dir(), cwd);
        assert_eq!(paths.export_dir(), cwd.join("exports"));
    }

    #[test]
    fn data_dir_uses_exe_folder_when_env_sits_beside_binary() {
        let root = tempfile::tempdir().unwrap();
        let exe_dir = root.path().join("portable");
        write_env(&exe_dir, "portable");
        let cwd = root.path().join("system32");
        fs::create_dir_all(&cwd).unwrap();
        let user = root.path().join("appdata");
        fs::create_dir_all(&user).unwrap();

        let paths = resolver(&cwd, Some(exe_dir.join("XManager.exe")), Some(user));
        assert_eq!(paths.data_dir(), exe_dir);
    }

    #[test]
    fn data_dir_falls_back_to_user_data() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("system32");
        fs::create_dir_all(&cwd).unwrap();
        let user = root.path().join("appdata");
        fs::create_dir_all(&user).unwrap();
        let exe_dir = root.path().join("Program Files").join("XManager");
        fs::create_dir_all(&exe_dir).unwrap();

        let paths = resolver(&cwd, Some(exe_dir.join("XManager.exe")), Some(user.clone()));
        assert_eq!(paths.data_dir(), user);
        assert_eq!(paths.log_dir(), user.join("logs"));
    }

    #[test]
    fn env_hint_mentions_env_file_and_refresh() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("XManager");
        fs::create_dir_all(&user).unwrap();
        let cwd = root.path().join("cwd");
        fs::create_dir_all(&cwd).unwrap();
        let hint = resolver(&cwd, None, Some(user.clone())).env_placement_hint_zh();
        assert!(hint.contains(".env"), "{hint}");
        assert!(hint.contains("刷新状态"), "{hint}");
        assert!(hint.contains(&user.display().to_string()), "{hint}");
        assert!(!hint.contains("项目根目录"), "{hint}");
    }

    #[test]
    fn cargo_target_triple_dir_is_skipped() {
        let root = tempfile::tempdir().unwrap();
        let target_dir = root
            .path()
            .join("target")
            .join("aarch64-apple-darwin")
            .join("release");
        write_env(&target_dir, "from-target");
        let cwd = root.path().join("repo");
        let expected = write_env(&cwd, "from-cwd");
        let found = resolver(&cwd, Some(target_dir.join("xmanager")), None).find_env_file();
        assert_eq!(found.as_deref(), Some(expected.as_path()));
    }
}
