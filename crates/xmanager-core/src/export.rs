//! Export tweets to CSV / JSON.

use crate::error::Result;
use crate::models::Tweet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Write tweets to a CSV file. Returns the resolved path.
pub fn export_csv(tweets: &[Tweet], path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record([
        "id",
        "created_at",
        "kind",
        "views",
        "likes",
        "retweets",
        "replies",
        "quotes",
        "bookmarks",
        "engagement",
        "like_rate",
        "bookmark_rate",
        "engagement_rate",
        "text",
    ])?;
    for t in tweets {
        let m = &t.public_metrics;
        wtr.write_record([
            t.id.as_str(),
            t.created_at.as_deref().unwrap_or(""),
            t.kind().label_zh(),
            &t.views().to_string(),
            &m.like_count.to_string(),
            &m.retweet_count.to_string(),
            &m.reply_count.to_string(),
            &m.quote_count.to_string(),
            &m.bookmark_count.to_string(),
            &t.engagement().to_string(),
            &format!("{:.6}", t.like_rate()),
            &format!("{:.6}", t.bookmark_rate()),
            &format!("{:.6}", t.engagement_rate()),
            t.text.as_str(),
        ])?;
    }
    wtr.flush()?;
    Ok(path.to_path_buf())
}

/// Write tweets to a JSON file (array of objects). Returns the resolved path.
pub fn export_json(tweets: &[Tweet], path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let payload: Vec<serde_json::Value> = tweets
        .iter()
        .map(|t| {
            let m = &t.public_metrics;
            serde_json::json!({
                "id": t.id,
                "created_at": t.created_at,
                "kind": t.kind().label_zh(),
                "views": t.views(),
                "likes": m.like_count,
                "retweets": m.retweet_count,
                "replies": m.reply_count,
                "quotes": m.quote_count,
                "bookmarks": m.bookmark_count,
                "engagement": t.engagement(),
                "like_rate": t.like_rate(),
                "bookmark_rate": t.bookmark_rate(),
                "engagement_rate": t.engagement_rate(),
                "text": t.text,
                "conversation_id": t.conversation_id,
                "in_reply_to_user_id": t.in_reply_to_user_id,
                "is_retweet": t.is_retweet,
                "is_quote": t.is_quote,
                "public_metrics": m,
            })
        })
        .collect();
    let mut file = File::create(path)?;
    let body = serde_json::to_string_pretty(&payload)?;
    file.write_all(body.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(path.to_path_buf())
}

/// Export based on file extension (`.csv` / `.json`). Defaults to CSV.
pub fn export_auto(tweets: &[Tweet], path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => export_json(tweets, path),
        _ => export_csv(tweets, path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PublicMetrics, Tweet};
    use tempfile::tempdir;

    #[test]
    fn csv_and_json_roundtrip_shape() {
        let dir = tempdir().unwrap();
        let tweets = vec![Tweet {
            id: "1".into(),
            text: "hello, world".into(),
            created_at: Some("2024-01-01T00:00:00Z".into()),
            public_metrics: PublicMetrics {
                impression_count: 3,
                like_count: 1,
                ..Default::default()
            },
            conversation_id: None,
            in_reply_to_user_id: None,
            is_retweet: false,
            is_quote: false,
        }];
        let csv_path = export_csv(&tweets, dir.path().join("t.csv")).unwrap();
        let json_path = export_json(&tweets, dir.path().join("t.json")).unwrap();
        assert!(csv_path.exists());
        assert!(json_path.exists());
        let csv = std::fs::read_to_string(csv_path).unwrap();
        assert!(csv.contains("hello, world"));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(json_path).unwrap()).unwrap();
        assert_eq!(v[0]["views"], 3);
    }
}
