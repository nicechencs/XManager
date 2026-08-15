//! Domain models for X API tweets and users.

use serde::{Deserialize, Serialize};

/// Public engagement metrics for a tweet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicMetrics {
    #[serde(default)]
    pub retweet_count: u64,
    #[serde(default)]
    pub reply_count: u64,
    #[serde(default)]
    pub like_count: u64,
    #[serde(default)]
    pub quote_count: u64,
    #[serde(default)]
    pub bookmark_count: u64,
    #[serde(default)]
    pub impression_count: u64,
}

/// Classification of a post for filtering / display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostKind {
    /// Original tweet (not a reply / retweet / quote).
    Original,
    /// Reply to another user.
    Reply,
    /// Pure retweet (RT).
    Retweet,
    /// Quote tweet.
    Quote,
}

impl PostKind {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Original => "原创",
            Self::Reply => "回帖",
            Self::Retweet => "转发",
            Self::Quote => "引用",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Original => "原",
            Self::Reply => "回",
            Self::Retweet => "转",
            Self::Quote => "引",
        }
    }
}

/// A single tweet / post from X API v2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tweet {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub public_metrics: PublicMetrics,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub in_reply_to_user_id: Option<String>,
    /// True when API marks this as a retweet via `referenced_tweets`.
    #[serde(default)]
    pub is_retweet: bool,
    /// True when API marks this as a quote via `referenced_tweets`.
    #[serde(default)]
    pub is_quote: bool,
}

impl Tweet {
    /// Impression / view count (曝光).
    pub fn views(&self) -> u64 {
        self.public_metrics.impression_count
    }

    /// Sum of likes, retweets, replies, quotes, and bookmarks.
    pub fn engagement(&self) -> u64 {
        let m = &self.public_metrics;
        m.like_count
            + m.retweet_count
            + m.reply_count
            + m.quote_count
            + m.bookmark_count
    }

    /// True if this tweet is a reply to another user.
    pub fn is_reply(&self) -> bool {
        self.in_reply_to_user_id.is_some()
    }

    /// Derived post kind (转发 > 回帖 > 引用 > 原创).
    pub fn kind(&self) -> PostKind {
        if self.is_retweet {
            return PostKind::Retweet;
        }
        if self.is_reply() {
            return PostKind::Reply;
        }
        if self.is_quote {
            return PostKind::Quote;
        }
        // Fallback: classic "RT @" prefix
        let t = self.text.trim_start();
        if t.starts_with("RT @") {
            return PostKind::Retweet;
        }
        PostKind::Original
    }

    /// 点赞率 = likes / max(views, 1)
    pub fn like_rate(&self) -> f64 {
        rate(self.public_metrics.like_count, self.views())
    }

    /// 收藏率 = bookmarks / max(views, 1)
    pub fn bookmark_rate(&self) -> f64 {
        rate(self.public_metrics.bookmark_count, self.views())
    }

    /// 互动率 = engagement / max(views, 1)
    pub fn engagement_rate(&self) -> f64 {
        rate(self.engagement(), self.views())
    }

    /// 转发率 = retweets / max(views, 1)
    pub fn retweet_rate(&self) -> f64 {
        rate(self.public_metrics.retweet_count, self.views())
    }

    /// 回复率 = replies / max(views, 1)
    pub fn reply_rate(&self) -> f64 {
        rate(self.public_metrics.reply_count, self.views())
    }

    /// Format a rate as percentage string, e.g. `1.23%`.
    pub fn format_rate(rate: f64) -> String {
        format!("{:.2}%", rate * 100.0)
    }

    /// Short date for display (`YYYY-MM-DD HH:MM`) when parseable.
    pub fn display_date(&self) -> String {
        let Some(raw) = self.created_at.as_deref() else {
            return "-".into();
        };
        if raw.len() >= 16 {
            let date = &raw[0..10];
            let time = &raw[11..16];
            format!("{date} {time}")
        } else {
            raw.to_string()
        }
    }
}

fn rate(num: u64, views: u64) -> f64 {
    let denom = views.max(1) as f64;
    num as f64 / denom
}

/// Authenticated user profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub username: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engagement_rates_and_kind() {
        let t = Tweet {
            id: "1".into(),
            text: "hi".into(),
            created_at: Some("2024-01-15T12:34:56.000Z".into()),
            public_metrics: PublicMetrics {
                like_count: 2,
                retweet_count: 1,
                reply_count: 0,
                quote_count: 0,
                bookmark_count: 1,
                impression_count: 100,
            },
            conversation_id: None,
            in_reply_to_user_id: None,
            is_retweet: false,
            is_quote: false,
        };
        assert_eq!(t.views(), 100);
        assert_eq!(t.engagement(), 4);
        assert!((t.like_rate() - 0.02).abs() < 1e-9);
        assert!((t.bookmark_rate() - 0.01).abs() < 1e-9);
        assert_eq!(t.kind(), PostKind::Original);
        assert_eq!(t.display_date(), "2024-01-15 12:34");
    }

    #[test]
    fn kind_priority() {
        let mut t = Tweet {
            id: "1".into(),
            text: "RT @x: hi".into(),
            created_at: None,
            public_metrics: PublicMetrics::default(),
            conversation_id: None,
            in_reply_to_user_id: Some("9".into()),
            is_retweet: true,
            is_quote: true,
        };
        assert_eq!(t.kind(), PostKind::Retweet);
        t.is_retweet = false;
        assert_eq!(t.kind(), PostKind::Reply);
        t.in_reply_to_user_id = None;
        assert_eq!(t.kind(), PostKind::Quote);
    }
}
