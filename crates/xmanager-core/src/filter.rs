//! Tweet filtering and summary statistics.

use crate::models::{PostKind, Tweet};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Sort field for filtered results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortField {
    /// 曝光 / 浏览量
    #[default]
    Views,
    /// 总互动次数
    Engagement,
    /// 发布时间
    Date,
    /// 点赞率 likes/views
    LikeRate,
    /// 收藏率 bookmarks/views
    BookmarkRate,
    /// 互动率 engagement/views
    EngagementRate,
    /// 转发率
    RetweetRate,
    /// 回复率
    ReplyRate,
}

impl SortField {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Views => "曝光",
            Self::Engagement => "互动",
            Self::Date => "日期",
            Self::LikeRate => "点赞率",
            Self::BookmarkRate => "收藏率",
            Self::EngagementRate => "互动率",
            Self::RetweetRate => "转发率",
            Self::ReplyRate => "回复率",
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortOrder {
    /// 最低优先（升序）
    #[default]
    Asc,
    /// 最高优先（降序）
    Desc,
}

/// Quick time-window presets (relative to now).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeRange {
    #[default]
    All,
    Hours24,
    Days7,
    Days30,
    Days90,
    Days180,
    Days360,
    /// Custom hours (used when UI stepper sets arbitrary window).
    CustomHours(u64),
}

impl TimeRange {
    pub fn label_zh(self) -> String {
        match self {
            Self::All => "全部时间".into(),
            Self::Hours24 => "24 小时".into(),
            Self::Days7 => "7 天".into(),
            Self::Days30 => "30 天".into(),
            Self::Days90 => "90 天".into(),
            Self::Days180 => "180 天".into(),
            Self::Days360 => "360 天".into(),
            Self::CustomHours(h) => {
                if h % 24 == 0 {
                    format!("{} 天", h / 24)
                } else {
                    format!("{h} 小时")
                }
            }
        }
    }

    pub fn within_hours(self) -> Option<u64> {
        match self {
            Self::All => None,
            Self::Hours24 => Some(24),
            Self::Days7 => Some(24 * 7),
            Self::Days30 => Some(24 * 30),
            Self::Days90 => Some(24 * 90),
            Self::Days180 => Some(24 * 180),
            Self::Days360 => Some(24 * 360),
            Self::CustomHours(h) => {
                if h == 0 {
                    None
                } else {
                    Some(h)
                }
            }
        }
    }

    pub const PRESETS: &[TimeRange] = &[
        TimeRange::All,
        TimeRange::Hours24,
        TimeRange::Days7,
        TimeRange::Days30,
        TimeRange::Days90,
        TimeRange::Days180,
        TimeRange::Days360,
    ];
}

/// Which post kinds to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindFilter {
    pub original: bool,
    pub reply: bool,
    pub retweet: bool,
    pub quote: bool,
}

impl Default for KindFilter {
    fn default() -> Self {
        Self {
            original: true,
            reply: true,
            retweet: false,
            quote: true,
        }
    }
}

impl KindFilter {
    pub fn allows(self, kind: PostKind) -> bool {
        match kind {
            PostKind::Original => self.original,
            PostKind::Reply => self.reply,
            PostKind::Retweet => self.retweet,
            PostKind::Quote => self.quote,
        }
    }

    pub fn all() -> Self {
        Self {
            original: true,
            reply: true,
            retweet: true,
            quote: true,
        }
    }
}

/// Options for filtering tweets.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FilterOptions {
    pub max_views: Option<u64>,
    pub min_views: Option<u64>,
    pub max_engagement: Option<u64>,
    /// Keep posts older than N days.
    pub older_than_days: Option<u64>,
    /// Keep posts newer than N days (legacy; prefer `time_range`).
    pub newer_than_days: Option<u64>,
    /// Preferred time window preset.
    pub time_range: TimeRange,
    /// Post-kind inclusion.
    pub kinds: KindFilter,
    /// Deprecated alias path: if false, replies excluded (overridden by kinds when set).
    pub include_replies: bool,
    pub sort: SortField,
    pub order: SortOrder,
    /// After sorting, keep only first N (最高/最低 Top-N). `None` = all.
    pub top_n: Option<usize>,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            max_views: None,
            min_views: None,
            max_engagement: None,
            older_than_days: None,
            newer_than_days: None,
            time_range: TimeRange::All,
            kinds: KindFilter::default(),
            include_replies: true,
            sort: SortField::Views,
            order: SortOrder::Asc,
            top_n: None,
        }
    }
}

impl FilterOptions {
    /// Convenience: low-view filter with default threshold 50.
    pub fn low_views(threshold: u64) -> Self {
        Self {
            max_views: Some(threshold),
            kinds: KindFilter {
                original: true,
                reply: false,
                retweet: false,
                quote: true,
            },
            include_replies: false,
            sort: SortField::Views,
            order: SortOrder::Asc,
            ..Default::default()
        }
    }

    /// Effective kind filter: merge legacy include_replies.
    fn effective_kinds(&self) -> KindFilter {
        let mut k = self.kinds;
        if !self.include_replies {
            k.reply = false;
        }
        k
    }
}

/// Aggregate stats for a tweet list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub avg_views: f64,
    pub median_views: f64,
    pub min_views: u64,
    pub max_views: u64,
    pub total_engagement: u64,
    pub total_views: u64,
    pub avg_like_rate: f64,
    pub avg_bookmark_rate: f64,
    pub avg_engagement_rate: f64,
    pub original_count: usize,
    pub reply_count: usize,
    pub retweet_count: usize,
    pub quote_count: usize,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            count: 0,
            avg_views: 0.0,
            median_views: 0.0,
            min_views: 0,
            max_views: 0,
            total_engagement: 0,
            total_views: 0,
            avg_like_rate: 0.0,
            avg_bookmark_rate: 0.0,
            avg_engagement_rate: 0.0,
            original_count: 0,
            reply_count: 0,
            retweet_count: 0,
            quote_count: 0,
        }
    }
}

/// View histogram buckets used by the UI/stats.
/// Each entry is `(label, min_inclusive, max_inclusive)`.
pub const VIEW_BUCKETS: &[(&str, u64, Option<u64>)] = &[
    ("0–10", 0, Some(10)),
    ("11–50", 11, Some(50)),
    ("51–100", 51, Some(100)),
    ("101–500", 101, Some(500)),
    ("501–1000", 501, Some(1000)),
    ("1000+", 1001, None),
];

/// Resolve histogram bucket bounds by label. Returns `(min_views, max_views)`.
/// `min_views = 0` is treated as "no lower bound" by callers that prefer open ranges.
pub fn view_bucket_bounds(label: &str) -> Option<(u64, Option<u64>)> {
    VIEW_BUCKETS
        .iter()
        .find(|(bucket_label, _, _)| *bucket_label == label)
        .map(|(_, lo, hi)| (*lo, *hi))
}

/// True when a histogram bucket intersects the current view filter.
/// No highlight when the library is not bounding views.
pub fn bucket_overlaps_view_filter(
    label: &str,
    min_views: Option<u64>,
    max_views: Option<u64>,
) -> bool {
    if min_views.is_none() && max_views.is_none() {
        return false;
    }
    let Some((lo, hi)) = view_bucket_bounds(label) else {
        return false;
    };
    let filter_lo = min_views.unwrap_or(0);
    let filter_hi = max_views.unwrap_or(u64::MAX);
    let bucket_hi = hi.unwrap_or(u64::MAX);
    lo <= filter_hi && bucket_hi >= filter_lo
}

/// Filter tweets by views / rates / age / kind, then sort (and optional top-N).
pub fn filter_tweets(tweets: &[Tweet], opts: &FilterOptions) -> Vec<Tweet> {
    let now = Utc::now();
    let older_cutoff = opts.older_than_days.map(|d| now - Duration::days(d as i64));

    // Time window: time_range takes precedence; fall back to newer_than_days.
    let newer_cutoff = if let Some(hours) = opts.time_range.within_hours() {
        Some(now - Duration::hours(hours as i64))
    } else {
        opts.newer_than_days.map(|d| now - Duration::days(d as i64))
    };

    let kinds = opts.effective_kinds();

    let mut result: Vec<Tweet> = tweets
        .iter()
        .filter(|t| {
            let views = t.views();
            if let Some(max) = opts.max_views {
                if views > max {
                    return false;
                }
            }
            if let Some(min) = opts.min_views {
                if views < min {
                    return false;
                }
            }
            if let Some(max_e) = opts.max_engagement {
                if t.engagement() > max_e {
                    return false;
                }
            }
            if !kinds.allows(t.kind()) {
                return false;
            }

            let created = parse_created_at(t.created_at.as_deref());
            if let Some(cutoff) = older_cutoff {
                match created {
                    Some(c) if c <= cutoff => {}
                    _ => return false,
                }
            }
            if let Some(cutoff) = newer_cutoff {
                match created {
                    Some(c) if c >= cutoff => {}
                    _ => return false,
                }
            }
            true
        })
        .cloned()
        .collect();

    let reverse = matches!(opts.order, SortOrder::Desc);
    result.sort_by(|a, b| {
        let ord = match opts.sort {
            SortField::Views => a.views().cmp(&b.views()),
            SortField::Engagement => a.engagement().cmp(&b.engagement()),
            SortField::Date => a
                .created_at
                .as_deref()
                .unwrap_or("")
                .cmp(b.created_at.as_deref().unwrap_or("")),
            SortField::LikeRate => a
                .like_rate()
                .partial_cmp(&b.like_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortField::BookmarkRate => a
                .bookmark_rate()
                .partial_cmp(&b.bookmark_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortField::EngagementRate => a
                .engagement_rate()
                .partial_cmp(&b.engagement_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortField::RetweetRate => a
                .retweet_rate()
                .partial_cmp(&b.retweet_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
            SortField::ReplyRate => a
                .reply_rate()
                .partial_cmp(&b.reply_rate())
                .unwrap_or(std::cmp::Ordering::Equal),
        };
        if reverse {
            ord.reverse()
        } else {
            ord
        }
    });

    if let Some(n) = opts.top_n {
        if result.len() > n {
            result.truncate(n);
        }
    }
    result
}

/// Compute summary statistics.
pub fn summarize(tweets: &[Tweet]) -> Summary {
    if tweets.is_empty() {
        return Summary::default();
    }
    let mut views: Vec<u64> = tweets.iter().map(|t| t.views()).collect();
    let total_views: u64 = views.iter().sum();
    let total_engagement: u64 = tweets.iter().map(|t| t.engagement()).sum();
    let min_views = *views.iter().min().unwrap_or(&0);
    let max_views = *views.iter().max().unwrap_or(&0);
    let avg_views = total_views as f64 / tweets.len() as f64;
    views.sort_unstable();
    let median_views = if views.len() % 2 == 1 {
        views[views.len() / 2] as f64
    } else {
        let mid = views.len() / 2;
        (views[mid - 1] as f64 + views[mid] as f64) / 2.0
    };

    let n = tweets.len() as f64;
    let avg_like_rate = tweets.iter().map(|t| t.like_rate()).sum::<f64>() / n;
    let avg_bookmark_rate = tweets.iter().map(|t| t.bookmark_rate()).sum::<f64>() / n;
    let avg_engagement_rate = tweets.iter().map(|t| t.engagement_rate()).sum::<f64>() / n;

    let mut original_count = 0usize;
    let mut reply_count = 0usize;
    let mut retweet_count = 0usize;
    let mut quote_count = 0usize;
    for t in tweets {
        match t.kind() {
            PostKind::Original => original_count += 1,
            PostKind::Reply => reply_count += 1,
            PostKind::Retweet => retweet_count += 1,
            PostKind::Quote => quote_count += 1,
        }
    }

    Summary {
        count: tweets.len(),
        avg_views,
        median_views,
        min_views,
        max_views,
        total_engagement,
        total_views,
        avg_like_rate,
        avg_bookmark_rate,
        avg_engagement_rate,
        original_count,
        reply_count,
        retweet_count,
        quote_count,
    }
}

/// Count tweets into view buckets. Returns (label, count) pairs.
pub fn view_histogram(tweets: &[Tweet]) -> Vec<(String, usize)> {
    let mut counts = vec![0usize; VIEW_BUCKETS.len()];
    for t in tweets {
        let v = t.views();
        for (i, &(_, lo, hi)) in VIEW_BUCKETS.iter().enumerate() {
            let in_bucket = match hi {
                Some(h) => v >= lo && v <= h,
                None => v >= lo,
            };
            if in_bucket {
                counts[i] += 1;
                break;
            }
        }
    }
    VIEW_BUCKETS
        .iter()
        .zip(counts)
        .map(|(&(label, _, _), c)| (label.to_string(), c))
        .collect()
}

/// Parse X API `created_at` into UTC.
pub fn parse_created_at(value: Option<&str>) -> Option<DateTime<Utc>> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%SZ")
                .ok()
                .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PublicMetrics;

    fn tweet(id: &str, views: u64, likes: u64, created: &str, kind: PostKind) -> Tweet {
        Tweet {
            id: id.into(),
            text: match kind {
                PostKind::Retweet => format!("RT @x: t{id}"),
                _ => format!("t{id}"),
            },
            created_at: Some(created.into()),
            public_metrics: PublicMetrics {
                impression_count: views,
                like_count: likes,
                bookmark_count: likes / 2,
                ..Default::default()
            },
            non_public_metrics: crate::models::NonPublicMetrics::default(),
            conversation_id: None,
            in_reply_to_user_id: if kind == PostKind::Reply {
                Some("99".into())
            } else {
                None
            },
            is_retweet: kind == PostKind::Retweet,
            is_quote: kind == PostKind::Quote,
        }
    }

    #[test]
    fn filter_max_views_and_kinds() {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let tweets = vec![
            tweet("1", 5, 1, &now, PostKind::Original),
            tweet("2", 80, 8, &now, PostKind::Original),
            tweet("3", 10, 1, &now, PostKind::Reply),
            tweet("4", 3, 0, &now, PostKind::Retweet),
        ];
        let opts = FilterOptions {
            max_views: Some(50),
            kinds: KindFilter {
                original: true,
                reply: false,
                retweet: false,
                quote: true,
            },
            include_replies: true,
            ..Default::default()
        };
        let out = filter_tweets(&tweets, &opts);
        assert_eq!(
            out.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["1"]
        );
    }

    #[test]
    fn sort_like_rate_desc() {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let tweets = vec![
            tweet("a", 100, 1, &now, PostKind::Original),  // 1%
            tweet("b", 100, 10, &now, PostKind::Original), // 10%
            tweet("c", 50, 5, &now, PostKind::Original),   // 10%
        ];
        let opts = FilterOptions {
            sort: SortField::LikeRate,
            order: SortOrder::Desc,
            kinds: KindFilter::all(),
            include_replies: true,
            ..Default::default()
        };
        let out = filter_tweets(&tweets, &opts);
        assert_eq!(out[0].id, "b");
        assert!(out[0].like_rate() >= out[1].like_rate());
    }

    #[test]
    fn top_n_lowest_views() {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let tweets = vec![
            tweet("1", 5, 0, &now, PostKind::Original),
            tweet("2", 50, 0, &now, PostKind::Original),
            tweet("3", 1, 0, &now, PostKind::Original),
        ];
        let opts = FilterOptions {
            sort: SortField::Views,
            order: SortOrder::Asc,
            top_n: Some(2),
            kinds: KindFilter::all(),
            include_replies: true,
            ..Default::default()
        };
        let out = filter_tweets(&tweets, &opts);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "3");
        assert_eq!(out[1].id, "1");
    }

    #[test]
    fn view_bucket_bounds_match_labels() {
        assert_eq!(view_bucket_bounds("0–10"), Some((0, Some(10))));
        assert_eq!(view_bucket_bounds("11–50"), Some((11, Some(50))));
        assert_eq!(view_bucket_bounds("1000+"), Some((1001, None)));
        assert_eq!(view_bucket_bounds("missing"), None);
    }

    #[test]
    fn bucket_highlight_follows_view_slice() {
        assert!(bucket_overlaps_view_filter("0–10", None, Some(50)));
        assert!(bucket_overlaps_view_filter("11–50", None, Some(50)));
        assert!(!bucket_overlaps_view_filter("51–100", None, Some(50)));
        assert!(!bucket_overlaps_view_filter("1000+", None, Some(50)));
        assert!(bucket_overlaps_view_filter("1000+", Some(1001), None));
        assert!(!bucket_overlaps_view_filter("0–10", None, None));
        let mut t = tweet("1", 1, 0, "2026-08-30T11:11:00Z", PostKind::Original);
        t.created_at = Some("2026-08-30T11:11:00Z".into());
        assert_eq!(t.display_date_short(), "08-30");
        assert_eq!(t.display_date(), "2026-08-30 11:11");
    }

    #[test]
    fn summarize_median_and_kinds() {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let tweets = vec![
            tweet("1", 1, 0, &now, PostKind::Original),
            tweet("2", 2, 0, &now, PostKind::Reply),
            tweet("3", 3, 0, &now, PostKind::Original),
        ];
        let s = summarize(&tweets);
        assert_eq!(s.count, 3);
        assert_eq!(s.median_views, 2.0);
        assert_eq!(s.original_count, 2);
        assert_eq!(s.reply_count, 1);
    }
}
