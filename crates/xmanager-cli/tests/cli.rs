//! Offline CLI tests. Live API commands are `#[ignore]`.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> Command {
    Command::cargo_bin("xmanager-cli").expect("xmanager-cli binary")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn stdout_json(cmd: &mut Command) -> Value {
    let output = cmd.output().expect("run cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "expected JSON stdout: {e}; status={:?}; stdout={stdout}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn help_lists_commands() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("filter"))
        .stdout(predicate::str::contains("creds"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn creds_missing_with_empty_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let env_path = dir.path().join("empty.env");
    std::fs::write(&env_path, "").unwrap();

    let mut cmd = bin();
    cmd.env_clear().args(["creds", "--env"]).arg(&env_path);
    let json = stdout_json(&mut cmd);
    assert_eq!(json["ok"], false);
    assert_eq!(json["code"], "missing_credentials");
    assert_eq!(json["oauth1"], false);
    let missing = json["missing"].as_array().expect("missing array");
    assert!(missing.iter().any(|v| v == "X_API_KEY"));
}

#[test]
fn creds_ok_from_env_file() {
    let dir = tempfile::tempdir().unwrap();
    let env_path = dir.path().join("ok.env");
    std::fs::write(
        &env_path,
        "X_API_KEY=k\nX_API_SECRET=s\nX_ACCESS_TOKEN=t\nX_ACCESS_TOKEN_SECRET=ts\n",
    )
    .unwrap();

    let mut cmd = bin();
    cmd.env_clear().args(["creds", "--env"]).arg(&env_path);
    let output = cmd.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["oauth1"], true);
    assert_eq!(json["missing"], json!([]));
    assert_eq!(json["layout"], "access_token_shape_missing");
}

#[test]
fn creds_detects_swapped_access_token_in_api_key() {
    let dir = tempfile::tempdir().unwrap();
    let env_path = dir.path().join("swapped.env");
    std::fs::write(
        &env_path,
        "X_API_KEY=1234567890-ThisIsTheUserAccessToken\nX_API_SECRET=s\nX_ACCESS_TOKEN=consumerLookingKey\nX_ACCESS_TOKEN_SECRET=ts\n",
    )
    .unwrap();

    let mut cmd = bin();
    cmd.env_clear().args(["creds", "--env"]).arg(&env_path);
    let json = stdout_json(&mut cmd);
    assert_eq!(json["ok"], false);
    assert_eq!(json["code"], "swapped_credentials");
    assert_eq!(json["layout"], "swapped");
    let hint = json["hint"].as_str().unwrap_or("");
    assert!(hint.contains("console.x.com"), "{hint}");
    assert!(hint.contains("X_API_KEY"), "{hint}");
}

#[test]
fn filter_max_views_and_kinds() {
    let input = fixture("tweets.json");

    let mut all = bin();
    all.args([
        "filter",
        "--input",
        input.to_str().unwrap(),
        "--max-views",
        "20",
    ]);
    let json = stdout_json(&mut all);
    assert_eq!(json["ok"], true);
    assert_eq!(json["count"], 2);
    let ids: Vec<&str> = json["tweets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["low-reply", "low-original"]);

    let mut originals = bin();
    originals.args([
        "filter",
        "--input",
        input.to_str().unwrap(),
        "--max-views",
        "20",
        "--kinds",
        "original",
    ]);
    let json = stdout_json(&mut originals);
    assert_eq!(json["count"], 1);
    assert_eq!(json["tweets"][0]["id"], "low-original");
}

#[test]
fn filter_low_views_preset_drops_replies() {
    let input = fixture("tweets.json");
    let mut cmd = bin();
    cmd.args([
        "filter",
        "--input",
        input.to_str().unwrap(),
        "--low-views",
        "50",
    ]);
    let json = stdout_json(&mut cmd);
    assert_eq!(json["ok"], true);
    let ids: Vec<&str> = json["tweets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["low-original", "mid-quote"]);
}

#[test]
fn summarize_counts_kinds() {
    let input = fixture("tweets.json");
    let mut cmd = bin();
    cmd.args(["summarize", "--input", input.to_str().unwrap()]);
    let json = stdout_json(&mut cmd);
    assert_eq!(json["ok"], true);
    assert_eq!(json["summary"]["count"], 4);
    assert_eq!(json["summary"]["original_count"], 2);
    assert_eq!(json["summary"]["reply_count"], 1);
    assert_eq!(json["summary"]["quote_count"], 1);
}

#[test]
fn export_writes_csv() {
    let input = fixture("tweets.json");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.csv");
    bin()
        .args([
            "export",
            "--input",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\": 4"));
    let csv = std::fs::read_to_string(&out).unwrap();
    assert!(csv.contains("low-original"));
    assert!(csv.contains("high-original"));
}

#[test]
fn delete_is_dry_run_without_yes() {
    let mut cmd = bin();
    cmd.env_clear().args(["delete", "--ids", "111,222"]);
    let json = stdout_json(&mut cmd);
    assert_eq!(json["ok"], true);
    assert_eq!(json["dry_run"], true);
    assert_eq!(json["count"], 2);
}

#[test]
fn delete_without_ids_fails() {
    let mut cmd = bin();
    cmd.env_clear().arg("delete");
    let output = cmd.output().unwrap();
    assert!(!output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["code"], "empty_ids");
}

#[test]
fn offline_commands_write_structured_logs() {
    let dir = tempfile::tempdir().unwrap();
    let log_dir = dir.path().join("logs");
    let out = dir.path().join("out.json");
    let input = fixture("tweets.json");

    bin()
        .env("XMANAGER_LOG_DIR", &log_dir)
        .env("XMANAGER_LOG_LEVEL", "info")
        .args([
            "export",
            "--input",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    bin()
        .env("XMANAGER_LOG_DIR", &log_dir)
        .env_remove("X_API_KEY")
        .args(["delete", "--ids", "111,222"])
        .assert()
        .success();

    let today = chrono::Local::now().format("%Y-%m-%d");
    let app = std::fs::read_to_string(log_dir.join(format!("xmanager-app-{today}.log"))).unwrap();
    let audit =
        std::fs::read_to_string(log_dir.join(format!("xmanager-audit-{today}.log"))).unwrap();
    assert!(app.contains("\"event\":\"app.start\""));
    assert!(app.contains("\"binary\":\"xmanager-cli\""));
    assert!(app.contains("\"event\":\"export.write\""));
    assert!(!app.contains("hello"));
    assert!(audit.contains("\"event\":\"cleanup.preview\""));
    assert!(audit.contains("\"dry_run\":true"));
}

#[test]
#[ignore]
fn live_whoami() {
    bin().arg("whoami").assert().success().stdout(
        predicate::str::contains("\"username\"").and(predicate::str::contains("\"ok\": true")),
    );
}
