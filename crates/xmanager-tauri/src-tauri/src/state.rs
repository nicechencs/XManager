//! Application state and action handlers, ported 1:1 from the former GPUI
//! `xmanager-ui` app state. All GPUI plumbing (entity notify, scroll handles,
//! layout metrics) moved to the web frontend; safety-critical cleanup logic
//! (receipts, revisions, snapshots) is unchanged.

use std::collections::{HashMap, HashSet};

use chrono::Local;
use xmanager_core::logging::{self, artifact_file_stem, events, Outcome, Stream};
use xmanager_core::{
    export_csv, filter_tweets, summarize, view_bucket_bounds, view_histogram, CredentialLayout,
    CredentialPresence, FilterOptions, KindFilter, Settings, SortField, SortOrder, Summary,
    TimeRange, Tweet, TweetLookup, User,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Library,
    Insights,
    Cleanup,
}

/// Where the in-app delete confirm was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteSource {
    /// Content library: checkbox / focused row, no cleanup visit required.
    Library,
    /// Optional safe-cleanup review tray.
    Cleanup,
}

/// One-click confirm panel shown before irreversible delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteConfirm {
    pub expected_count: usize,
    pub source: DeleteSource,
    pub ids: HashSet<String>,
}

impl DeleteConfirm {
    pub fn library(ids: HashSet<String>) -> Self {
        let expected_count = ids.len();
        Self {
            expected_count,
            source: DeleteSource::Library,
            ids,
        }
    }

    pub fn cleanup(ids: HashSet<String>) -> Self {
        let expected_count = ids.len();
        Self {
            expected_count,
            source: DeleteSource::Cleanup,
            ids,
        }
    }
}

pub fn delete_confirm_ready(confirm: &DeleteConfirm) -> bool {
    confirm.expected_count > 0 && confirm.ids.len() == confirm.expected_count
}

/// Library confirm copy: count + irreversible, no extra “continue” wording.
pub fn library_delete_prompt(count: usize) -> String {
    format!("将永久删除 {count} 条，不可恢复")
}

/// Resolve tweet bodies for a delete/backup set. Library cache wins, then cleanup snapshot.
pub fn tweets_for_ids(
    ids: &HashSet<String>,
    all_tweets: &[Tweet],
    snapshot: &HashMap<String, Tweet>,
) -> Vec<Tweet> {
    let mut tweets: Vec<Tweet> = ids
        .iter()
        .filter_map(|id| {
            all_tweets
                .iter()
                .find(|tweet| tweet.id == *id)
                .cloned()
                .or_else(|| snapshot.get(id).cloned())
        })
        .collect();
    tweets.sort_by(|a, b| a.id.cmp(&b.id));
    tweets
}

/// Removable applied-filter chips shown in the Library toolbar.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "chip", rename_all = "snake_case")]
pub enum AppliedFilterChip {
    TimeRange { label: String },
    MaxViews { value: u64 },
    MinViews { value: u64 },
    MaxEngagement { value: u64 },
    OlderThanDays { days: u64 },
    TopN { n: usize },
    Sort { label: String },
    Order { label: String },
    Kinds { label: String },
}

/// Build chips for non-default applied filters so users can clear them one by one.
pub fn applied_filter_chips(filter: &FilterOptions) -> Vec<AppliedFilterChip> {
    let mut chips = Vec::new();
    if filter.time_range != TimeRange::All {
        chips.push(AppliedFilterChip::TimeRange {
            label: filter.time_range.label_zh(),
        });
    }
    if let Some(v) = filter.max_views {
        chips.push(AppliedFilterChip::MaxViews { value: v });
    }
    if let Some(v) = filter.min_views {
        chips.push(AppliedFilterChip::MinViews { value: v });
    }
    if let Some(v) = filter.max_engagement {
        chips.push(AppliedFilterChip::MaxEngagement { value: v });
    }
    if let Some(d) = filter.older_than_days {
        chips.push(AppliedFilterChip::OlderThanDays { days: d });
    }
    if let Some(n) = filter.top_n {
        chips.push(AppliedFilterChip::TopN { n });
    }
    // Default scope is 曝光 + 最低优先 + 全部类型; only show chips that change results.
    if filter.sort != SortField::Views {
        chips.push(AppliedFilterChip::Sort {
            label: filter.sort.label_zh().to_string(),
        });
    }
    if filter.order != SortOrder::Asc || filter.sort != SortField::Views {
        chips.push(AppliedFilterChip::Order {
            label: match filter.order {
                SortOrder::Asc => "最低优先".into(),
                SortOrder::Desc => "最高优先".into(),
            },
        });
    }

    let kinds = filter.kinds;
    let kind_labels: Vec<&str> = [
        (kinds.original, "原创"),
        (kinds.reply, "回帖"),
        (kinds.retweet, "转发"),
        (kinds.quote, "引用"),
    ]
    .into_iter()
    .filter_map(|(on, label)| on.then_some(label))
    .collect();
    if kind_labels.len() != 4 {
        let kind_label = if kind_labels.is_empty() {
            "类型：无".into()
        } else {
            format!("类型：{}", kind_labels.join("/"))
        };
        chips.push(AppliedFilterChip::Kinds { label: kind_label });
    }
    chips
}

/// Human-readable candidate line: tweet excerpt first, id second.
pub fn tweet_preview_label(
    id: &str,
    snapshot: &HashMap<String, Tweet>,
    all_tweets: &[Tweet],
) -> String {
    let text = snapshot.get(id).map(|t| t.text.as_str()).or_else(|| {
        all_tweets
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.text.as_str())
    });
    match text {
        Some(raw) => {
            let excerpt = raw.replace('\n', " ");
            let excerpt: String = excerpt.chars().take(36).collect();
            let ellipsis = if raw.chars().count() > 36 { "…" } else { "" };
            format!("{excerpt}{ellipsis}")
        }
        None => id.to_string(),
    }
}

pub fn cleanup_staging_notice(count: usize) -> String {
    format!("已加入安全清理（{count} 条）。可继续勾选，或前往安全清理。")
}

/// Default first-run scope: 曝光≤50、原创+引用. Documented so an empty table is not a surprise.
#[allow(dead_code)]
pub fn is_default_cleanup_preset(filter: &FilterOptions) -> bool {
    filter.max_views == Some(50)
        && filter.min_views.is_none()
        && filter.max_engagement.is_none()
        && filter.older_than_days.is_none()
        && filter.time_range == TimeRange::All
        && filter.kinds.original
        && !filter.kinds.reply
        && !filter.kinds.retweet
        && filter.kinds.quote
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryEmptyKind {
    Loading,
    CredentialsMissing,
    CredentialsSwapped,
    FetchFailed,
    NeverSynced,
    AccountEmpty,
    FilteredEmpty,
}

impl LibraryEmptyKind {
    /// Single-glyph mark for the illustration-free empty panel.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Loading => "…",
            Self::CredentialsMissing => "钥",
            Self::CredentialsSwapped => "换",
            Self::FetchFailed => "!",
            Self::NeverSynced => "↓",
            Self::AccountEmpty => "空",
            Self::FilteredEmpty => "筛",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEmptyCopy {
    pub kind: LibraryEmptyKind,
    pub title: String,
    pub detail: String,
}

/// Distinguish 无凭证 / 填反 / 拉取失败 / 真空 / 筛选后 0 条.
pub fn library_empty_copy(
    loading: bool,
    tweet_count: usize,
    filtered_count: usize,
    last_synced: bool,
    last_fetch_failed: bool,
    layout: CredentialLayout,
) -> LibraryEmptyCopy {
    if loading && tweet_count == 0 {
        return LibraryEmptyCopy {
            kind: LibraryEmptyKind::Loading,
            title: "正在拉取…".into(),
            detail: "首次同步完成后会显示结果。".into(),
        };
    }
    if tweet_count == 0 {
        if layout == CredentialLayout::Swapped {
            return LibraryEmptyCopy {
                kind: LibraryEmptyKind::CredentialsSwapped,
                title: "凭证字段填反".into(),
                detail: CredentialLayout::Swapped.user_facing().to_string(),
            };
        }
        if layout == CredentialLayout::Missing {
            return LibraryEmptyCopy {
                kind: LibraryEmptyKind::CredentialsMissing,
                title: "尚未配置凭证".into(),
                detail: xmanager_core::env_placement_hint_zh(),
            };
        }
        if last_fetch_failed {
            return LibraryEmptyCopy {
                kind: LibraryEmptyKind::FetchFailed,
                title: "拉取失败".into(),
                detail: "上次没有拉到数据。请看上方错误（常见：字段填反、未开通按量付费、只读 Token），改完后再点「拉取并分析」。".into(),
            };
        }
        if !last_synced {
            return LibraryEmptyCopy {
                kind: LibraryEmptyKind::NeverSynced,
                title: "还没有拉取".into(),
                detail: "点「拉取并分析」同步你的推文。默认会先筛曝光≤50 的原创/引用，便于找低曝光内容。".into(),
            };
        }
        return LibraryEmptyCopy {
            kind: LibraryEmptyKind::AccountEmpty,
            title: "这次没有拉到推文".into(),
            detail: "账号可能没有近期帖子，或下次拉取时打开「拉取含转发」。".into(),
        };
    }
    if filtered_count == 0 {
        return LibraryEmptyCopy {
            kind: LibraryEmptyKind::FilteredEmpty,
            title: format!("共 {tweet_count} 条，筛选后 0 条"),
            detail:
                "默认范围是曝光≤50、不含回帖/转发。点下方芯片去掉条件，或「清除筛选」查看全部。"
                    .into(),
        };
    }
    LibraryEmptyCopy {
        kind: LibraryEmptyKind::AccountEmpty,
        title: "暂无数据".into(),
        detail: "先拉取你的推文，再按曝光、类型或时间筛选。".into(),
    }
}

pub fn credentials_line_healthy(layout: CredentialLayout, verified_user: bool) -> bool {
    verified_user || layout == CredentialLayout::Ready
}

/// Library slice stats line shown at the right of the filter card.
pub fn slice_stats_line_for(summary: &Summary) -> String {
    format!(
        "{} 条 · 均曝光 {:.0} · 均互率 {}",
        summary.count,
        summary.avg_views,
        Tweet::format_rate(summary.avg_engagement_rate)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub revision: u64,
    pub candidate_ids: HashSet<String>,
}

pub fn receipt_matches(
    receipt: Option<&Receipt>,
    revision: u64,
    candidates: &HashSet<String>,
) -> bool {
    receipt.is_some_and(|value| value.revision == revision && value.candidate_ids == *candidates)
}

/// Checkbox selection wins; if nothing is checked, the focused tweet still counts.
pub fn ids_for_cleanup_staging(
    selected: &HashSet<String>,
    focused_tweet_id: Option<&str>,
) -> Vec<String> {
    if !selected.is_empty() {
        return selected.iter().cloned().collect();
    }
    focused_tweet_id
        .filter(|id| !id.is_empty())
        .map(|id| vec![id.to_owned()])
        .unwrap_or_default()
}

/// Clicking an already-active sort header flips direction; a new field
/// defaults to lowest-first for views and highest-first otherwise.
pub fn next_sort_from_header(
    current_sort: SortField,
    current_order: SortOrder,
    clicked: SortField,
) -> (SortField, SortOrder) {
    if current_sort == clicked {
        let order = match current_order {
            SortOrder::Asc => SortOrder::Desc,
            SortOrder::Desc => SortOrder::Asc,
        };
        (clicked, order)
    } else {
        let order = match clicked {
            SortField::Views => SortOrder::Asc,
            _ => SortOrder::Desc,
        };
        (clicked, order)
    }
}

/// Next action the operator should take on the cleanup route.
pub fn cleanup_next_hint(
    candidate_count: usize,
    credentials_ok: bool,
    confirm: Option<&DeleteConfirm>,
) -> String {
    if candidate_count == 0 {
        return "内容库可直接「删除选中」。需要复核时再加入安全清理。".into();
    }
    if !credentials_ok {
        return "已有候选，但缺少有效 OAuth 凭证，无法删除。".into();
    }
    if let Some(confirm) = confirm {
        return format!(
            "将删除 {} 条。点「确认删除」后不可恢复；取消可继续复核。",
            confirm.expected_count
        );
    }
    format!("复核列表后点「删除 {candidate_count} 条」。会先自动备份，再确认一次即可。")
}

/// Refresh only the cached content that the bounded API response actually includes.
/// Candidates omitted by the response intentionally remain in `snapshot`.
pub fn refresh_candidate_snapshot(
    candidates: &HashSet<String>,
    snapshot: &mut HashMap<String, Tweet>,
    fetched: &[Tweet],
) -> bool {
    for tweet in fetched {
        if candidates.contains(&tweet.id) {
            snapshot.insert(tweet.id.clone(), tweet.clone());
        }
    }
    !candidates.is_empty()
}

/// Resolve focused content from the current filtered results first, then from
/// the immutable cleanup snapshot when a bounded refresh omitted the tweet.
/// The boolean indicates that the snapshot fallback was used.
pub fn resolve_focused_tweet<'a>(
    id: &str,
    filtered: &'a [Tweet],
    snapshot: &'a HashMap<String, Tweet>,
) -> Option<(&'a Tweet, bool)> {
    filtered
        .iter()
        .find(|tweet| tweet.id == id)
        .map(|tweet| (tweet, false))
        .or_else(|| snapshot.get(id).map(|tweet| (tweet, true)))
}

/// Draft filter values edited in the sidebar before applying.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FilterDraft {
    pub max_views: Option<u64>,
    pub min_views: Option<u64>,
    pub max_engagement: Option<u64>,
    pub older_than_days: Option<u64>,
    pub newer_than_days: Option<u64>,
    pub time_range: TimeRange,
    pub kinds: KindFilter,
    /// When true, retweets are included at API fetch time.
    pub include_retweets: bool,
    pub sort: SortField,
    pub order: SortOrder,
    /// Top-N after sort (None = all).
    pub top_n: Option<usize>,
}

impl Default for FilterDraft {
    fn default() -> Self {
        Self {
            max_views: Some(50),
            min_views: None,
            max_engagement: None,
            older_than_days: None,
            newer_than_days: None,
            time_range: TimeRange::All,
            kinds: KindFilter {
                original: true,
                reply: false,
                retweet: false,
                quote: true,
            },
            include_retweets: false,
            sort: SortField::Views,
            order: SortOrder::Asc,
            top_n: None,
        }
    }
}

impl FilterDraft {
    /// No view/kind/time restrictions. Used by「清除筛选」.
    pub fn unrestricted() -> Self {
        Self {
            max_views: None,
            min_views: None,
            max_engagement: None,
            older_than_days: None,
            newer_than_days: None,
            time_range: TimeRange::All,
            kinds: KindFilter::all(),
            include_retweets: true,
            sort: SortField::Views,
            order: SortOrder::Asc,
            top_n: None,
        }
    }

    pub fn to_filter_options(&self) -> FilterOptions {
        FilterOptions {
            max_views: self.max_views,
            min_views: self.min_views,
            max_engagement: self.max_engagement,
            older_than_days: self.older_than_days,
            newer_than_days: self.newer_than_days,
            time_range: self.time_range,
            kinds: self.kinds,
            include_replies: self.kinds.reply,
            sort: self.sort,
            order: self.order,
            top_n: self.top_n,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

/// Structured dry-run outcome for the cleanup workflow.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PreviewOutcome {
    pub revision: u64,
    pub total: usize,
    pub deletable: Vec<String>,
    pub missing: Vec<String>,
    pub failed: Vec<(String, String)>,
}

impl PreviewOutcome {
    pub fn summary_line(&self) -> String {
        format!(
            "预演完成：可删 {} · 不在库 {} · 失败 {}（共 {}）",
            self.deletable.len(),
            self.missing.len(),
            self.failed.len(),
            self.total
        )
    }
}

/// Structured real-delete outcome shown after an irreversible attempt.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeleteOutcome {
    pub succeeded: Vec<String>,
    pub failed: Vec<(String, String)>,
}

impl DeleteOutcome {
    pub fn summary_line(&self) -> String {
        format!(
            "删除结果：成功 {} · 失败 {}（共 {}）",
            self.succeeded.len(),
            self.failed.len(),
            self.succeeded.len() + self.failed.len()
        )
    }

    pub fn from_request(requested_ids: &[String], fail: &[(String, String)]) -> Self {
        let fail_ids: HashSet<String> = fail.iter().map(|(id, _)| id.clone()).collect();
        let mut succeeded: Vec<String> = requested_ids
            .iter()
            .filter(|id| !fail_ids.contains(*id))
            .cloned()
            .collect();
        succeeded.sort();
        let mut failed = fail.to_vec();
        failed.sort_by(|a, b| a.0.cmp(&b.0));
        Self { succeeded, failed }
    }
}

/// Classify dry-run candidates against the local cache without calling the API.
/// IDs present in `known_ids` are deletable; others are reported as missing.
pub fn classify_preview(
    candidate_ids: &HashSet<String>,
    known_ids: &HashSet<String>,
    revision: u64,
) -> PreviewOutcome {
    let mut deletable = Vec::new();
    let mut missing = Vec::new();
    for id in candidate_ids {
        if known_ids.contains(id) {
            deletable.push(id.clone());
        } else {
            missing.push(id.clone());
        }
    }
    deletable.sort();
    missing.sort();
    let total = candidate_ids.len();
    PreviewOutcome {
        revision,
        total,
        deletable,
        missing,
        failed: Vec::new(),
    }
}

/// Map an X API tweet lookup onto the current cleanup candidate set.
pub fn preview_from_lookup(
    candidate_ids: &HashSet<String>,
    lookup: &TweetLookup,
    revision: u64,
) -> PreviewOutcome {
    let found: HashSet<String> = lookup.found.iter().map(|t| t.id.clone()).collect();
    let missing_set: HashSet<String> = lookup.missing.iter().cloned().collect();
    let mut failed = lookup.failed.clone();
    let mut deletable = Vec::new();
    let mut missing = lookup.missing.clone();
    for id in candidate_ids {
        if found.contains(id) {
            deletable.push(id.clone());
        } else if missing_set.contains(id) {
            continue;
        } else if !failed.iter().any(|(fid, _)| fid == id) {
            failed.push((id.clone(), "lookup did not return this id".into()));
        }
    }
    deletable.sort();
    missing.sort();
    failed.sort_by(|a, b| a.0.cmp(&b.0));
    PreviewOutcome {
        revision,
        total: candidate_ids.len(),
        deletable,
        missing,
        failed,
    }
}

/// Background job payloads produced by `begin_*` methods and consumed by the
/// matching `finish_*` on the command side (around `spawn_blocking`).
pub mod job {
    use super::*;

    pub struct FetchJob {
        pub limit: usize,
        pub exclude_retweets: bool,
    }

    pub struct DeleteJob {
        pub ids: Vec<String>,
        pub requested: usize,
    }

    pub struct LivePreviewJob {
        pub ids: Vec<String>,
        pub revision: u64,
        pub candidates: HashSet<String>,
        pub then_confirm: bool,
    }

    pub struct ExportJob {
        pub tweets: Vec<Tweet>,
        pub path: std::path::PathBuf,
        pub format: ExportFormat,
        pub count: usize,
        pub revision: u64,
        pub candidate_ids: HashSet<String>,
        pub kind: &'static str,
    }
}

/// Root application state shared behind a mutex and rendered from snapshots.
pub struct Workspace {
    pub active_route: Route,
    pub credentials_ok: bool,
    pub credentials_msg: String,
    pub credential_layout: CredentialLayout,
    pub credential_presence: CredentialPresence,
    pub env_file: Option<String>,
    pub current_user: Option<User>,
    pub all_tweets: Vec<Tweet>,
    pub filtered: Vec<Tweet>,
    pub selected: HashSet<String>,
    /// Tweet currently shown in the contextual detail panel.
    pub focused_tweet_id: Option<String>,
    /// Compatibility mirror of the candidate IDs included in the last preview.
    /// The authoritative guard is `preview_receipt` + `cleanup_revision`.
    pub previewed_delete_ids: HashSet<String>,
    /// Transient candidate set used by the safe cleanup workflow.
    pub cleanup_candidates: HashSet<String>,
    /// Immutable in-memory content snapshots for candidates omitted by bounded refreshes.
    pub cleanup_snapshot: HashMap<String, Tweet>,
    /// Monotonic revision for candidate snapshots. Any mutation invalidates receipts.
    pub cleanup_revision: u64,
    pub backup_receipt: Option<Receipt>,
    pub preview_receipt: Option<Receipt>,
    /// Structured dry-run result bound to the current candidate revision.
    pub preview_outcome: Option<PreviewOutcome>,
    /// Last real-delete attempt summary (kept until next delete or clear).
    pub last_delete_outcome: Option<DeleteOutcome>,
    /// In-app second confirmation before irreversible deletion.
    pub delete_confirm: Option<DeleteConfirm>,
    pub filter_draft: FilterDraft,
    pub applied_filter: FilterOptions,
    pub fetch_limit: usize,
    pub loading: bool,
    pub status_msg: String,
    pub error_msg: Option<String>,
    /// Stats for the current library slice (`filtered`).
    pub summary: Summary,
    /// Stats for the full synced set (`all_tweets`). Insights uses this.
    pub all_summary: Summary,
    /// View histogram over `all_tweets` (not the current slice).
    pub histogram: Vec<(String, usize)>,
    /// Last successful tweet sync timestamp (local time display string).
    pub last_synced_at: Option<String>,
    /// Library banner after staging candidates without leaving the page.
    pub cleanup_notice: Option<String>,
    /// First click arms「清空」; second click actually clears.
    pub clear_cleanup_armed: bool,
    /// Last「拉取并分析」failed (distinct from export / delete errors).
    pub last_fetch_failed: bool,
}

impl Workspace {
    pub fn new() -> Self {
        let env_file = Settings::discovered_env_file().map(|p| p.display().to_string());
        let (credentials_ok, credential_layout, credential_presence, credentials_msg) =
            match Settings::load() {
                Ok(s) => {
                    let layout = s.credential_layout();
                    let presence = s.credential_presence();
                    let ok = s.has_oauth1();
                    if ok {
                        logging::info(Stream::App, events::APP_CONFIG)
                            .outcome(Outcome::Ok)
                            .field("oauth1", true)
                            .field("layout", layout.as_str())
                            .field("has_api_key", presence.api_key)
                            .field("has_api_secret", presence.api_secret)
                            .field("has_access_token", presence.access_token)
                            .field("has_access_token_secret", presence.access_token_secret)
                            .field("has_bearer", presence.bearer_token)
                            .field_opt("env_file", env_file.as_deref())
                            .emit();
                    } else {
                        logging::warn(Stream::App, events::APP_CONFIG)
                            .outcome(Outcome::Error)
                            .field("oauth1", false)
                            .field("layout", layout.as_str())
                            .field("has_api_key", presence.api_key)
                            .field("has_api_secret", presence.api_secret)
                            .field("has_access_token", presence.access_token)
                            .field("has_access_token_secret", presence.access_token_secret)
                            .field("has_bearer", presence.bearer_token)
                            .field_opt("env_file", env_file.as_deref())
                            .emit();
                    }
                    (ok, layout, presence, layout.sidebar_zh().to_string())
                }
                Err(e) => {
                    logging::error(Stream::App, events::APP_CONFIG)
                        .outcome(Outcome::Error)
                        .field("error", e.to_string())
                        .emit();
                    (
                        false,
                        CredentialLayout::Missing,
                        CredentialPresence {
                            api_key: false,
                            api_secret: false,
                            access_token: false,
                            access_token_secret: false,
                            bearer_token: false,
                        },
                        format!("读取配置失败: {e}"),
                    )
                }
            };

        let filter_draft = FilterDraft::default();
        let applied_filter = filter_draft.to_filter_options();

        Self {
            active_route: Route::Library,
            credentials_ok,
            credentials_msg,
            credential_layout,
            credential_presence,
            env_file,
            current_user: None,
            all_tweets: Vec::new(),
            filtered: Vec::new(),
            selected: HashSet::new(),
            focused_tweet_id: None,
            previewed_delete_ids: HashSet::new(),
            cleanup_candidates: HashSet::new(),
            cleanup_snapshot: HashMap::new(),
            cleanup_revision: 0,
            backup_receipt: None,
            preview_receipt: None,
            preview_outcome: None,
            last_delete_outcome: None,
            delete_confirm: None,
            filter_draft,
            applied_filter,
            fetch_limit: 100,
            loading: false,
            status_msg: "就绪".into(),
            error_msg: None,
            summary: Summary::default(),
            all_summary: Summary::default(),
            histogram: Vec::new(),
            last_synced_at: None,
            cleanup_notice: None,
            clear_cleanup_armed: false,
            last_fetch_failed: false,
        }
    }

    pub fn applied_chips(&self) -> Vec<AppliedFilterChip> {
        applied_filter_chips(&self.applied_filter)
    }

    /// Clear one applied-filter chip and recompute results immediately.
    pub fn remove_applied_chip(&mut self, chip: &AppliedFilterChip) {
        if self.loading {
            return;
        }
        match chip {
            AppliedFilterChip::TimeRange { .. } => {
                self.filter_draft.time_range = TimeRange::All;
                self.filter_draft.newer_than_days = None;
            }
            AppliedFilterChip::MaxViews { .. } => self.filter_draft.max_views = None,
            AppliedFilterChip::MinViews { .. } => self.filter_draft.min_views = None,
            AppliedFilterChip::MaxEngagement { .. } => self.filter_draft.max_engagement = None,
            AppliedFilterChip::OlderThanDays { .. } => self.filter_draft.older_than_days = None,
            AppliedFilterChip::TopN { .. } => self.filter_draft.top_n = None,
            AppliedFilterChip::Sort { .. } => {
                self.filter_draft.sort = SortField::Views;
            }
            AppliedFilterChip::Order { .. } => {
                self.filter_draft.order = SortOrder::Asc;
            }
            AppliedFilterChip::Kinds { .. } => {
                self.filter_draft.kinds = KindFilter::all();
            }
        }
        self.apply_filters();
    }

    pub fn recompute_filtered(&mut self) {
        self.filtered = filter_tweets(&self.all_tweets, &self.applied_filter);
        let previous_selection = self.selected.clone();
        let valid: HashSet<String> = self.filtered.iter().map(|t| t.id.clone()).collect();
        self.selected.retain(|id| valid.contains(id));
        if previous_selection != self.selected {
            self.previewed_delete_ids.clear();
        }
        let focused_is_visible = self
            .focused_tweet_id
            .as_ref()
            .is_some_and(|id| valid.contains(id));
        if !focused_is_visible {
            self.focused_tweet_id = None;
        }
        self.summary = summarize(&self.filtered);
        self.all_summary = summarize(&self.all_tweets);
        self.histogram = view_histogram(&self.all_tweets);
    }

    pub fn set_route(&mut self, route: Route) {
        self.active_route = route;
        if route == Route::Cleanup {
            self.cleanup_notice = None;
        }
    }

    pub fn dismiss_cleanup_notice(&mut self) {
        self.cleanup_notice = None;
    }

    pub fn focus_previous(&mut self) {
        self.focus_relative(-1);
    }

    pub fn focus_next(&mut self) {
        self.focus_relative(1);
    }

    fn focus_relative(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let current = self
            .focused_tweet_id
            .as_ref()
            .and_then(|id| self.filtered.iter().position(|t| &t.id == id))
            .unwrap_or(if delta < 0 { self.filtered.len() } else { 0 });
        let next = if self
            .focused_tweet_id
            .as_ref()
            .and_then(|id| self.filtered.iter().position(|tweet| &tweet.id == id))
            .is_none()
        {
            if delta < 0 {
                self.filtered.len().saturating_sub(1)
            } else {
                0
            }
        } else if delta < 0 {
            current.saturating_sub(1)
        } else {
            (current + 1).min(self.filtered.len().saturating_sub(1))
        };
        self.focused_tweet_id = self.filtered.get(next).map(|t| t.id.clone());
    }

    fn invalidate_cleanup_receipts(&mut self) {
        self.cleanup_revision = self.cleanup_revision.wrapping_add(1);
        self.backup_receipt = None;
        self.preview_receipt = None;
        self.preview_outcome = None;
        self.previewed_delete_ids.clear();
        self.delete_confirm = None;
    }

    /// Apply a histogram bucket as min/max views and return to Library.
    pub fn apply_histogram_bucket(&mut self, label: &str) {
        if self.loading {
            return;
        }
        let Some((lo, hi)) = view_bucket_bounds(label) else {
            self.set_error(format!("未知曝光区间: {label}"));
            return;
        };
        // Open lower bound (0) means "no min"; otherwise use inclusive min.
        self.filter_draft.min_views = if lo == 0 { None } else { Some(lo) };
        self.filter_draft.max_views = hi;
        self.filter_draft.sort = SortField::Views;
        self.filter_draft.order = SortOrder::Asc;
        self.filter_draft.top_n = None;
        self.apply_filters();
        self.active_route = Route::Library;
        self.status_msg = format!("已应用曝光区间「{label}」：{} 条结果", self.filtered.len());
    }

    /// Open the in-app second confirmation panel before irreversible deletion.
    pub fn begin_delete_confirm(&mut self) {
        if self.loading {
            return;
        }
        let n = self.cleanup_candidates.len();
        if n == 0 {
            self.set_error("请先加入安全清理候选");
            return;
        }
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            return;
        }
        if !self.has_valid_backup() || !self.has_valid_delete_preview() {
            self.set_error("真实删除前必须为当前候选完成备份并预演");
            return;
        }
        let ids = self.cleanup_candidates.clone();
        self.delete_confirm = Some(DeleteConfirm::cleanup(ids));
        self.status_msg = format!("将删除 {n} 条。点「确认删除」继续，或取消。");
        self.error_msg = None;
    }

    pub fn cancel_delete_confirm(&mut self) {
        if self.delete_confirm.take().is_some() {
            self.status_msg = "已取消删除".into();
            logging::info(Stream::Audit, events::CLEANUP_CANCEL)
                .outcome(Outcome::Cancel)
                .field("stage", "confirm")
                .field("revision", self.cleanup_revision)
                .field("count", self.cleanup_candidates.len() as u64)
                .emit();
        }
    }

    pub fn delete_confirm_is_ready(&self) -> bool {
        self.delete_confirm
            .as_ref()
            .is_some_and(delete_confirm_ready)
    }

    pub fn add_cleanup_candidate(&mut self, id: &str) {
        if self.loading || !self.all_tweets.iter().any(|t| t.id == id) {
            return;
        }
        if self.cleanup_candidates.insert(id.to_owned()) {
            if let Some(tweet) = self.all_tweets.iter().find(|tweet| tweet.id == id) {
                self.cleanup_snapshot.insert(id.to_owned(), tweet.clone());
            }
            self.invalidate_cleanup_receipts();
            self.status_msg = cleanup_staging_notice(self.cleanup_candidates.len());
            self.cleanup_notice = Some(cleanup_staging_notice(self.cleanup_candidates.len()));
            self.clear_cleanup_armed = false;
        }
    }

    pub fn remove_cleanup_candidate(&mut self, id: &str) {
        if self.loading {
            return;
        }
        if self.cleanup_candidates.remove(id) {
            self.cleanup_snapshot.remove(id);
            self.invalidate_cleanup_receipts();
            if self.focused_tweet_id.as_deref() == Some(id)
                && resolve_focused_tweet(id, &self.filtered, &self.cleanup_snapshot).is_none()
            {
                self.focused_tweet_id = None;
            }
            self.clear_cleanup_armed = false;
            self.status_msg = format!("已移除候选（{} 条）", self.cleanup_candidates.len());
        }
    }

    pub fn request_clear_cleanup(&mut self) {
        if self.loading || self.cleanup_candidates.is_empty() {
            return;
        }
        if self.clear_cleanup_armed {
            self.clear_cleanup_candidates();
            return;
        }
        self.clear_cleanup_armed = true;
        self.status_msg = "再点一次「确认清空」才会移除全部候选".into();
    }

    pub fn clear_cleanup_candidates(&mut self) {
        if self.loading || self.cleanup_candidates.is_empty() {
            return;
        }
        self.cleanup_candidates.clear();
        self.cleanup_snapshot.clear();
        self.invalidate_cleanup_receipts();
        self.clear_cleanup_armed = false;
        self.cleanup_notice = None;
        self.status_msg = "已清空安全清理候选".into();
    }

    /// Open the library-direct delete confirm. Checkbox wins; focused row counts if none checked.
    pub fn request_library_delete(&mut self) {
        if self.loading {
            return;
        }
        let ids = ids_for_cleanup_staging(&self.selected, self.focused_tweet_id.as_deref());
        if ids.is_empty() {
            self.set_error("请先勾选推文，或点开一条再删除");
            return;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            return;
        }
        let n = ids.len();
        self.delete_confirm = Some(DeleteConfirm::library(ids.into_iter().collect()));
        self.status_msg = library_delete_prompt(n);
        self.error_msg = None;
    }

    pub fn add_selected_to_cleanup(&mut self) {
        if self.loading {
            return;
        }
        let selected_ids =
            ids_for_cleanup_staging(&self.selected, self.focused_tweet_id.as_deref());
        if selected_ids.is_empty() {
            self.set_error("请先勾选推文，或点开一条再加入安全清理");
            return;
        }
        let before = self.cleanup_candidates.len();
        self.cleanup_candidates.extend(selected_ids.iter().cloned());
        for id in selected_ids {
            if let Some(tweet) = self.all_tweets.iter().find(|tweet| tweet.id == id) {
                self.cleanup_snapshot.insert(id, tweet.clone());
            }
        }
        if self.cleanup_candidates.len() != before {
            self.invalidate_cleanup_receipts();
        }
        self.clear_cleanup_armed = false;
        self.cleanup_notice = Some(cleanup_staging_notice(self.cleanup_candidates.len()));
        self.status_msg = cleanup_staging_notice(self.cleanup_candidates.len());
    }

    /// Write a CSV backup under the export directory and bind a receipt. Idempotent when
    /// a matching backup already exists.
    pub fn ensure_backup_for_current_candidates(&mut self) -> Result<String, String> {
        if self.has_valid_backup() {
            return Ok("already-backed-up".into());
        }
        let tweets = self.cleanup_tweets();
        if tweets.is_empty() {
            return Err("暂无安全清理候选".into());
        }
        let written = self.write_cleanup_csv_backup(&tweets, "auto")?;
        self.backup_receipt = Some(Receipt {
            revision: self.cleanup_revision,
            candidate_ids: self.cleanup_candidates.clone(),
        });
        Ok(written)
    }

    fn write_cleanup_csv_backup(&self, tweets: &[Tweet], source: &str) -> Result<String, String> {
        if tweets.is_empty() {
            return Err("没有可备份的推文".into());
        }
        let dir = Settings::default_export_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建导出目录: {e}"))?;
        let path = dir.join(format!(
            "{}.csv",
            artifact_file_stem("cleanup", Local::now())
        ));
        let written = export_csv(tweets, &path).map_err(|e| e.to_string())?;
        logging::info(Stream::Audit, events::CLEANUP_BACKUP)
            .outcome(Outcome::Ok)
            .field("revision", self.cleanup_revision)
            .field("count", tweets.len() as u64)
            .field("path", written.display().to_string())
            .field("receipt_bound", source == "auto")
            .field("source", source)
            .emit();
        Ok(written.display().to_string())
    }

    fn backup_ids_to_exports(&self, ids: &HashSet<String>) -> Result<String, String> {
        let tweets = tweets_for_ids(ids, &self.all_tweets, &self.cleanup_snapshot);
        if tweets.is_empty() {
            return Err("没有可备份的推文，已取消删除".into());
        }
        self.write_cleanup_csv_backup(&tweets, "library")
    }

    /// Local dry-run against the current cache/snapshot. Does not call the API.
    pub fn run_local_preview(&mut self) -> Result<PreviewOutcome, String> {
        if self.cleanup_candidates.is_empty() {
            return Err("请先加入安全清理候选".into());
        }
        if !self.has_valid_backup() {
            return Err("请先为当前候选创建备份".into());
        }
        let snapshot = self.cleanup_candidates.clone();
        let revision = self.cleanup_revision;
        let known: HashSet<String> = self
            .all_tweets
            .iter()
            .map(|t| t.id.clone())
            .chain(self.cleanup_snapshot.keys().cloned())
            .collect();
        let outcome = classify_preview(&snapshot, &known, revision);
        self.preview_receipt = Some(Receipt {
            revision,
            candidate_ids: snapshot.clone(),
        });
        self.previewed_delete_ids = snapshot;
        self.status_msg = outcome.summary_line();
        Ok(outcome)
    }

    fn apply_preview_outcome(&mut self, snapshot: HashSet<String>, outcome: PreviewOutcome) {
        let revision = outcome.revision;
        self.preview_receipt = Some(Receipt {
            revision,
            candidate_ids: snapshot.clone(),
        });
        self.previewed_delete_ids = snapshot;
        self.status_msg = outcome.summary_line();
        logging::info(Stream::Audit, events::CLEANUP_PREVIEW)
            .outcome(Outcome::Ok)
            .field("revision", revision)
            .field("total", outcome.total as u64)
            .field("deletable", outcome.deletable.len() as u64)
            .field("missing", outcome.missing.len() as u64)
            .field("failed", outcome.failed.len() as u64)
            .field("source", "lookup_or_local")
            .emit();
        self.preview_outcome = Some(outcome);
    }

    /// Capture a live X API lookup job. `then_confirm` opens the delete panel on success.
    pub fn begin_live_preview(&mut self, then_confirm: bool) -> Option<job::LivePreviewJob> {
        let ids: Vec<String> = self.cleanup_candidates.iter().cloned().collect();
        if ids.is_empty() {
            self.set_error("请先加入安全清理候选");
            return None;
        }
        let revision = self.cleanup_revision;
        let candidates = self.cleanup_candidates.clone();
        self.set_busy(format!("正在向 X 预演 {} 条…", ids.len()));
        Some(job::LivePreviewJob {
            ids,
            revision,
            candidates,
            then_confirm,
        })
    }

    pub fn finish_live_preview(
        &mut self,
        job: job::LivePreviewJob,
        result: Result<TweetLookup, String>,
    ) {
        self.loading = false;
        match result {
            Ok(lookup) => {
                if self.cleanup_revision != job.revision
                    || self.cleanup_candidates != job.candidates
                {
                    self.set_error("候选已变化，请重新预演");
                } else {
                    let outcome = preview_from_lookup(&job.candidates, &lookup, job.revision);
                    self.error_msg = None;
                    self.apply_preview_outcome(job.candidates, outcome);
                    if job.then_confirm {
                        self.begin_delete_confirm();
                    }
                }
            }
            Err(err) => {
                // Lookup is a separate X product; network/tier failures
                // must not block backup+confirm+delete.
                match self.run_local_preview() {
                    Ok(_) => {
                        self.error_msg = None;
                        self.status_msg =
                            format!("X API 预演不可用，已改用本地缓存。原因: {err}（详见 logs/）");
                        if job.then_confirm {
                            self.begin_delete_confirm();
                        }
                    }
                    Err(_) => {
                        self.set_error(format!("预演失败: {err}（详见 logs/）"));
                    }
                }
            }
        }
    }

    pub fn cleanup_tweets(&self) -> Vec<Tweet> {
        self.cleanup_candidates
            .iter()
            .filter_map(|id| self.cleanup_snapshot.get(id).cloned())
            .collect()
    }

    pub fn has_valid_backup(&self) -> bool {
        receipt_matches(
            self.backup_receipt.as_ref(),
            self.cleanup_revision,
            &self.cleanup_candidates,
        )
    }

    pub fn has_valid_preview(&self) -> bool {
        receipt_matches(
            self.preview_receipt.as_ref(),
            self.cleanup_revision,
            &self.cleanup_candidates,
        )
    }

    pub fn apply_sort_header(&mut self, field: SortField) {
        if self.loading {
            return;
        }
        let (sort, order) =
            next_sort_from_header(self.filter_draft.sort, self.filter_draft.order, field);
        self.filter_draft.sort = sort;
        self.filter_draft.order = order;
        self.apply_filters();
    }

    pub fn apply_filters(&mut self) {
        if self.loading {
            return;
        }
        self.applied_filter = self.filter_draft.to_filter_options();
        self.previewed_delete_ids.clear();
        self.recompute_filtered();
        self.status_msg = format!(
            "已应用筛选：{} / {} 条",
            self.filtered.len(),
            self.all_tweets.len()
        );
        self.error_msg = None;
    }

    pub fn select_all_filtered(&mut self) {
        if self.loading {
            return;
        }
        self.selected = self.filtered.iter().map(|t| t.id.clone()).collect();
        self.previewed_delete_ids.clear();
        self.status_msg = format!("已全选 {} 条", self.selected.len());
    }

    pub fn select_none(&mut self) {
        if self.loading {
            return;
        }
        self.selected.clear();
        self.previewed_delete_ids.clear();
        self.status_msg = "已取消全部选择".into();
    }

    pub fn invert_selection(&mut self) {
        if self.loading {
            return;
        }
        let mut next = HashSet::new();
        for t in &self.filtered {
            if !self.selected.contains(&t.id) {
                next.insert(t.id.clone());
            }
        }
        self.selected = next;
        self.previewed_delete_ids.clear();
        self.status_msg = format!("已反选，当前 {} 条", self.selected.len());
    }

    pub fn toggle_selected(&mut self, id: &str) {
        if self.loading {
            return;
        }
        if self.selected.contains(id) {
            self.selected.remove(id);
        } else {
            self.selected.insert(id.to_string());
        }
        self.previewed_delete_ids.clear();
    }

    pub fn focus_tweet(&mut self, id: &str) {
        self.focused_tweet_id = Some(id.to_string());
    }

    pub fn toggle_tweet_focus(&mut self, id: &str) {
        if self.focused_tweet_id.as_deref() == Some(id) {
            self.focused_tweet_id = None;
        } else {
            self.focus_tweet(id);
        }
    }

    /// Open a tweet in the library, loosening view/kind bounds if it is filtered out.
    pub fn open_tweet_in_library(&mut self, id: &str) {
        if self.loading {
            return;
        }
        let tweet = self.all_tweets.iter().find(|t| t.id == id).cloned();
        if let Some(tweet) = tweet {
            let views = tweet.views();
            let mut loosened = false;
            if self.filter_draft.max_views.is_some_and(|max| views > max) {
                self.filter_draft.max_views = None;
                loosened = true;
            }
            if self.filter_draft.min_views.is_some_and(|min| views < min) {
                self.filter_draft.min_views = None;
                loosened = true;
            }
            if !self.filter_draft.kinds.allows(tweet.kind()) {
                self.filter_draft.kinds = KindFilter::all();
                loosened = true;
            }
            if self.filter_draft.top_n.is_some() && !self.filtered.iter().any(|t| t.id == id) {
                self.filter_draft.top_n = None;
                loosened = true;
            }
            if loosened {
                self.apply_filters();
            }
        }
        self.focus_tweet(id);
        self.active_route = Route::Library;
    }

    pub fn has_valid_delete_preview(&self) -> bool {
        self.has_valid_preview()
    }

    pub fn set_busy(&mut self, msg: impl Into<String>) {
        self.loading = true;
        self.status_msg = msg.into();
        self.error_msg = None;
    }

    pub fn set_error(&mut self, err: impl Into<String>) {
        self.loading = false;
        let msg = err.into();
        logging::error(Stream::App, events::UI_ERROR)
            .outcome(Outcome::Error)
            .field("message", msg.clone())
            .emit();
        self.error_msg = Some(msg.clone());
        self.status_msg = "出错".into();
    }

    pub fn clear_error(&mut self) {
        self.error_msg = None;
    }

    fn reload_credentials(&mut self) {
        match Settings::load() {
            Ok(s) => {
                self.credential_layout = s.credential_layout();
                self.credential_presence = s.credential_presence();
                self.credentials_ok = s.has_oauth1();
                self.credentials_msg = self.credential_layout.sidebar_zh().to_string();
            }
            Err(e) => {
                self.credentials_ok = false;
                self.credential_layout = CredentialLayout::Missing;
                self.credentials_msg = format!("读取配置失败: {e}");
            }
        }
    }

    fn credentials_block_reason(&self) -> String {
        match self.credential_layout {
            CredentialLayout::Swapped => CredentialLayout::Swapped.user_facing().to_string(),
            CredentialLayout::AccessTokenShapeMissing => {
                format!(
                    "{} 仍可点「刷新状态」做一次 whoami。",
                    CredentialLayout::AccessTokenShapeMissing.user_facing()
                )
            }
            CredentialLayout::Missing | CredentialLayout::Ready => {
                "未配置有效凭证，无法拉取。把 .env 放到程序旁边或用户配置目录后，再点「拉取并分析」或「刷新状态」。".into()
            }
        }
    }

    /// Validate credentials and capture the fetch job; sets busy on success.
    pub fn begin_fetch(&mut self) -> Option<job::FetchJob> {
        if self.loading {
            return None;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.last_fetch_failed = true;
            self.set_error(self.credentials_block_reason());
            return None;
        }

        let limit = self.fetch_limit;
        // Fetch broadly (include replies); kind filter applied client-side.
        // Retweets still controlled at API for volume.
        let exclude_retweets = !self.filter_draft.include_retweets;

        self.set_busy(format!("正在拉取推文（最多 {limit} 条）…"));
        Some(job::FetchJob {
            limit,
            exclude_retweets,
        })
    }

    pub fn finish_fetch(&mut self, result: Result<(User, Vec<Tweet>), String>) {
        self.loading = false;
        match result {
            Ok((user, tweets)) => {
                let n = tweets.len();
                self.last_fetch_failed = false;
                self.credentials_ok = true;
                self.credentials_msg = format!("凭证有效 · @{}", user.username);
                self.current_user = Some(user);
                self.all_tweets = tweets;
                self.selected.clear();
                self.focused_tweet_id = None;
                let reset_safety_steps = refresh_candidate_snapshot(
                    &self.cleanup_candidates,
                    &mut self.cleanup_snapshot,
                    &self.all_tweets,
                );
                if reset_safety_steps {
                    self.invalidate_cleanup_receipts();
                }
                self.applied_filter = self.filter_draft.to_filter_options();
                self.recompute_filtered();
                self.last_synced_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
                self.error_msg = None;
                self.status_msg = if self.cleanup_candidates.is_empty() {
                    format!("拉取完成：共 {n} 条，筛选后 {} 条", self.filtered.len())
                } else {
                    format!(
                        "拉取完成：共 {n} 条，筛选后 {} 条；已保留 {} 条候选，备份与预演已重置",
                        self.filtered.len(),
                        self.cleanup_candidates.len()
                    )
                };
            }
            Err(e) => {
                self.last_fetch_failed = true;
                self.set_error(format!("拉取失败: {e}"));
            }
        }
    }

    pub fn begin_whoami(&mut self) -> bool {
        if self.loading {
            return false;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error(self.credentials_block_reason());
            return false;
        }
        self.set_busy("正在获取当前用户…");
        true
    }

    pub fn finish_whoami(&mut self, result: Result<User, String>) {
        self.loading = false;
        match result {
            Ok(user) => {
                self.credentials_ok = true;
                self.credentials_msg = format!("凭证有效 · @{}", user.username);
                self.status_msg = format!("当前用户: @{}", user.username);
                self.current_user = Some(user);
                self.error_msg = None;
            }
            Err(e) => self.set_error(format!("获取用户失败: {e}")),
        }
    }

    /// Begin a library-scope export of the current filtered slice. `path` is
    /// None when the save dialog could not open; the write then falls back to
    /// the default export directory with a status note.
    pub fn begin_export_tweets(
        &mut self,
        format: ExportFormat,
        path: Option<std::path::PathBuf>,
    ) -> Option<job::ExportJob> {
        if self.loading {
            return None;
        }
        let tweets = self.filtered.clone();
        if tweets.is_empty() {
            self.set_error("没有可导出的推文（请先拉取并筛选）");
            return None;
        }
        let ext = match format {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        };
        let suggested = format!("{}.{ext}", artifact_file_stem("library", Local::now()));
        self.prepare_export(format, tweets, path, suggested, "library", None)
    }

    /// Export the immutable candidate snapshot and bind a backup receipt only on success.
    pub fn begin_export_candidates(
        &mut self,
        format: ExportFormat,
        path: Option<std::path::PathBuf>,
    ) -> Option<job::ExportJob> {
        if self.loading {
            return None;
        }
        let tweets = self.cleanup_tweets();
        if tweets.is_empty() {
            self.set_error("暂无安全清理候选");
            return None;
        }
        let candidate_ids = self.cleanup_candidates.clone();
        let revision = self.cleanup_revision;
        let ext = match format {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        };
        let suggested = format!("{}.{ext}", artifact_file_stem("cleanup", Local::now()));
        self.prepare_export(
            format,
            tweets,
            path,
            suggested,
            "cleanup",
            Some((revision, candidate_ids)),
        )
    }

    fn prepare_export(
        &mut self,
        format: ExportFormat,
        tweets: Vec<Tweet>,
        path: Option<std::path::PathBuf>,
        suggested: String,
        kind: &'static str,
        receipt: Option<(u64, HashSet<String>)>,
    ) -> Option<job::ExportJob> {
        let count = tweets.len();
        let (path, note) = match path {
            Some(p) => (p, None),
            None => {
                let dir = Settings::default_export_dir();
                let _ = std::fs::create_dir_all(&dir);
                let note = if kind == "library" {
                    Some("未打开存盘框，改写到默认导出目录".to_string())
                } else {
                    None
                };
                (dir.join(&suggested), note)
            }
        };
        if let Some(note) = note {
            self.status_msg = note;
        }
        let (revision, candidate_ids) = receipt
            .map(|(r, c)| (r, c))
            .unwrap_or((self.cleanup_revision, self.cleanup_candidates.clone()));
        Some(job::ExportJob {
            tweets,
            path,
            format,
            count,
            revision,
            candidate_ids,
            kind,
        })
    }

    pub fn finish_export(
        &mut self,
        kind: &'static str,
        count: usize,
        revision: u64,
        candidate_ids: HashSet<String>,
        result: Result<String, String>,
    ) {
        self.loading = false;
        match result {
            Ok(path) => {
                if kind == "cleanup" {
                    let bound = self.cleanup_revision == revision
                        && self.cleanup_candidates == candidate_ids;
                    if bound {
                        self.backup_receipt = Some(Receipt {
                            revision,
                            candidate_ids,
                        });
                    }
                    logging::info(Stream::Audit, events::CLEANUP_BACKUP)
                        .outcome(Outcome::Ok)
                        .field("revision", revision)
                        .field("count", count as u64)
                        .field("path", path.as_str())
                        .field("receipt_bound", bound)
                        .emit();
                    self.error_msg = None;
                    self.status_msg = format!("已备份 {count} 条候选 → {path}");
                } else {
                    self.error_msg = None;
                    self.status_msg = format!("已导出 {count} 条 → {path}");
                }
            }
            Err(err) => {
                if kind == "cleanup" {
                    logging::error(Stream::Audit, events::CLEANUP_BACKUP)
                        .outcome(Outcome::Error)
                        .field("revision", revision)
                        .field("count", count as u64)
                        .field("error", err.as_str())
                        .emit();
                    self.set_error(format!("备份失败: {err}"));
                } else {
                    self.set_error(format!("导出失败: {err}"));
                }
            }
        }
    }

    /// Apply a ranking preset: time window + sort metric + high/low + optional top-N.
    pub fn apply_rank_preset(
        &mut self,
        range: TimeRange,
        sort: SortField,
        order: SortOrder,
        top_n: Option<usize>,
    ) {
        self.filter_draft.time_range = range;
        self.filter_draft.sort = sort;
        self.filter_draft.order = order;
        self.filter_draft.top_n = top_n;
        // When ranking rates/high performers, drop leftover histogram bounds
        // so the preset is not accidentally empty.
        if !matches!(sort, SortField::Views) || matches!(order, SortOrder::Desc) {
            self.filter_draft.max_views = None;
            self.filter_draft.min_views = None;
        }
        self.apply_filters();
    }

    /// Apply low-exposure quick filter within current time range.
    pub fn apply_low_exposure_preset(&mut self, threshold: u64) {
        self.filter_draft.max_views = Some(threshold);
        self.filter_draft.min_views = None;
        self.filter_draft.sort = SortField::Views;
        self.filter_draft.order = SortOrder::Asc;
        self.filter_draft.top_n = None;
        self.apply_filters();
    }

    /// Backup + live preview, then open the in-app confirm panel.
    pub fn delete_previewed(&mut self) -> Option<job::LivePreviewJob> {
        if self.loading {
            return None;
        }
        if self.cleanup_candidates.is_empty() {
            self.set_error("请先加入安全清理候选");
            return None;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            return None;
        }
        if let Err(err) = self.ensure_backup_for_current_candidates() {
            self.set_error(err);
            return None;
        }
        self.begin_live_preview(true)
    }

    /// Commit real deletion after the in-app confirmation panel is accepted.
    pub fn confirm_and_delete(&mut self) -> Option<job::DeleteJob> {
        if !self.delete_confirm_is_ready() {
            self.set_error("请先确认删除");
            return None;
        }
        let Some(confirm) = self.delete_confirm.take() else {
            self.set_error("请先确认删除");
            return None;
        };
        match confirm.source {
            DeleteSource::Library => {
                self.active_route = Route::Library;
                if let Err(err) = self.backup_ids_to_exports(&confirm.ids) {
                    self.set_error(err);
                    return None;
                }
                let ids: Vec<String> = confirm.ids.into_iter().collect();
                self.begin_delete_ids(ids, false)
            }
            DeleteSource::Cleanup => {
                if confirm.expected_count != self.cleanup_candidates.len()
                    || confirm.ids != self.cleanup_candidates
                {
                    self.set_error("候选数量已变化，请重新点删除");
                    return None;
                }
                let ids: Vec<String> = confirm.ids.into_iter().collect();
                self.begin_delete_ids(ids, true)
            }
        }
    }

    fn apply_deleted_ids(&mut self, deleted: &HashSet<String>) {
        if deleted.is_empty() {
            return;
        }
        for id in deleted {
            self.cleanup_snapshot.remove(id);
        }
        self.all_tweets.retain(|t| !deleted.contains(&t.id));
        self.selected.retain(|id| !deleted.contains(id));
        let cleanup_changed = self
            .cleanup_candidates
            .iter()
            .any(|id| deleted.contains(id));
        self.cleanup_candidates.retain(|id| !deleted.contains(id));
        if cleanup_changed {
            self.invalidate_cleanup_receipts();
        }
        if self
            .focused_tweet_id
            .as_ref()
            .is_some_and(|id| deleted.contains(id))
        {
            self.focused_tweet_id = None;
        }
        self.recompute_filtered();
    }

    pub fn begin_delete_ids(
        &mut self,
        ids: Vec<String>,
        require_cleanup_receipts: bool,
    ) -> Option<job::DeleteJob> {
        if self.loading {
            return None;
        }
        if ids.is_empty() {
            self.set_error(if require_cleanup_receipts {
                "请先加入安全清理候选"
            } else {
                "请先勾选推文，或点开一条再删除"
            });
            return None;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            return None;
        }
        if require_cleanup_receipts
            && (!self.has_valid_backup() || !self.has_valid_delete_preview())
        {
            self.set_error("真实删除前必须为当前候选完成备份并预演");
            return None;
        }

        let n = ids.len();
        self.set_busy(format!("正在删除 {n} 条…"));
        Some(job::DeleteJob { ids, requested: n })
    }

    pub fn finish_delete(
        &mut self,
        job: job::DeleteJob,
        result: Result<(usize, Vec<(String, String)>), String>,
    ) {
        self.loading = false;
        match result {
            Ok((ok, fail)) => {
                let outcome = DeleteOutcome::from_request(&job.ids, &fail);
                if ok > 0 {
                    let deleted: HashSet<String> = outcome.succeeded.iter().cloned().collect();
                    self.apply_deleted_ids(&deleted);
                }
                self.last_delete_outcome = Some(outcome.clone());
                self.status_msg = outcome.summary_line();
                let batch_outcome = if fail.is_empty() {
                    Outcome::Ok
                } else if outcome.succeeded.is_empty() {
                    Outcome::Error
                } else {
                    Outcome::Partial
                };
                logging::info(Stream::Audit, events::CLEANUP_DELETE)
                    .outcome(batch_outcome)
                    .field("requested", job.requested as u64)
                    .field("succeeded", outcome.succeeded.len() as u64)
                    .field("failed", fail.len() as u64)
                    .field("succeeded_ids", outcome.succeeded.clone())
                    .field(
                        "failed_ids",
                        fail.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                    )
                    .emit();
                if fail.is_empty() {
                    self.error_msg = None;
                } else {
                    let sample = fail
                        .iter()
                        .take(3)
                        .map(|(id, e)| format!("{id}: {e}"))
                        .collect::<Vec<_>>()
                        .join("; ");
                    self.error_msg = Some(format!("部分失败: {sample}"));
                }
            }
            Err(e) => {
                logging::error(Stream::Audit, events::CLEANUP_DELETE)
                    .outcome(Outcome::Error)
                    .field("requested", job.requested as u64)
                    .field("error", e.as_str())
                    .emit();
                self.set_error(format!("删除失败: {e}（详见 logs/）"));
            }
        }
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
