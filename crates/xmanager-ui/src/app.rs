//! Application state, async work helpers, and action handlers.

use std::collections::{HashMap, HashSet};

use chrono::Local;
use gpui::{AsyncApp, Context, SharedString, UniformListScrollHandle, WeakEntity, Window};
use xmanager_core::logging::{self, artifact_file_stem, events, Outcome, Stream};
use xmanager_core::{
    export_csv, export_json, filter_tweets, summarize, view_bucket_bounds, view_histogram,
    CredentialLayout, FilterOptions, KindFilter, Settings, SortField, SortOrder, Summary,
    TimeRange, Tweet, TweetLookup, User, XClient,
};

/// Responsive shell breakpoints matching the guided-cleanup workspace spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// ≥1200px: full navigation + inspector.
    Wide,
    /// 800–1199px: compact navigation, keep inspector.
    Medium,
    /// <800px: icon nav, hide inspector by default.
    Narrow,
}

impl LayoutMode {
    pub fn from_width(width_px: f32) -> Self {
        if width_px >= 1200.0 {
            Self::Wide
        } else if width_px >= 800.0 {
            Self::Medium
        } else {
            Self::Narrow
        }
    }

    pub fn nav_width(self) -> f32 {
        match self {
            Self::Wide => 220.0,
            Self::Medium => 96.0,
            Self::Narrow => 80.0,
        }
    }

    pub fn show_nav_labels(self) -> bool {
        matches!(self, Self::Wide)
    }

    /// Permanent inspector column (Wide/Medium only).
    pub fn show_inspector(self) -> bool {
        !matches!(self, Self::Narrow)
    }

    /// Width used by the Wide/Medium inspector column.
    /// Narrow overlay ignores this and uses `flex_1` / 100% width.
    pub fn inspector_width(self) -> f32 {
        match self {
            Self::Wide => 380.0,
            Self::Medium => 300.0,
            Self::Narrow => 280.0,
        }
    }

    /// Side-column inspector: Wide only. Medium/Narrow use an overlay drawer.
    pub fn library_shows_inspector(self, _has_focus: bool) -> bool {
        matches!(self, Self::Wide)
    }

    /// Narrow replaces the list with a full-height filter overlay.
    pub fn filter_as_overlay(self) -> bool {
        matches!(self, Self::Narrow)
    }

    /// Medium/Narrow show the inspector as a full-height overlay when focused.
    pub fn inspector_as_overlay(self) -> bool {
        matches!(self, Self::Medium | Self::Narrow)
    }

    /// Narrow tweet rows are labeled cards instead of a compact table.
    pub fn tweet_list_as_cards(self) -> bool {
        matches!(self, Self::Narrow)
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Wide => "宽屏",
            Self::Medium => "中屏",
            Self::Narrow => "窄屏",
        }
    }
}

/// Drag state for the library list / inspector splitter.
#[derive(Debug, Clone, Copy)]
pub struct SplitDrag {
    pub start_x: f32,
    pub start_inspector_width: f32,
}

pub const INSPECTOR_WIDTH_MIN: f32 = 220.0;
pub const INSPECTOR_WIDTH_MAX: f32 = 640.0;

pub fn clamp_inspector_width(width: f32) -> f32 {
    width.clamp(INSPECTOR_WIDTH_MIN, INSPECTOR_WIDTH_MAX)
}

/// Structured dry-run outcome for the cleanup workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Library,
    Insights,
    Cleanup,
}

/// Where the in-app delete confirm was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppliedFilterChip {
    TimeRange(String),
    MaxViews(u64),
    MinViews(u64),
    MaxEngagement(u64),
    OlderThanDays(u64),
    TopN(usize),
    Sort(String),
    Order(String),
    Kinds(String),
}

/// Build chips for non-default applied filters so users can clear them one by one.
pub fn applied_filter_chips(filter: &FilterOptions) -> Vec<AppliedFilterChip> {
    let mut chips = Vec::new();
    if filter.time_range != TimeRange::All {
        chips.push(AppliedFilterChip::TimeRange(filter.time_range.label_zh()));
    }
    if let Some(v) = filter.max_views {
        chips.push(AppliedFilterChip::MaxViews(v));
    }
    if let Some(v) = filter.min_views {
        chips.push(AppliedFilterChip::MinViews(v));
    }
    if let Some(v) = filter.max_engagement {
        chips.push(AppliedFilterChip::MaxEngagement(v));
    }
    if let Some(d) = filter.older_than_days {
        chips.push(AppliedFilterChip::OlderThanDays(d));
    }
    if let Some(n) = filter.top_n {
        chips.push(AppliedFilterChip::TopN(n));
    }
    // Default scope is 曝光 + 最低优先 + 全部类型; only show chips that change results.
    if filter.sort != SortField::Views {
        chips.push(AppliedFilterChip::Sort(filter.sort.label_zh().to_string()));
    }
    if filter.order != SortOrder::Asc || filter.sort != SortField::Views {
        chips.push(AppliedFilterChip::Order(match filter.order {
            SortOrder::Asc => "最低优先".into(),
            SortOrder::Desc => "最高优先".into(),
        }));
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
        let kind_chip = if kind_labels.is_empty() {
            "类型：无".into()
        } else {
            format!("类型：{}", kind_labels.join("/"))
        };
        chips.push(AppliedFilterChip::Kinds(kind_chip));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn chip_label(chip: &AppliedFilterChip) -> String {
    match chip {
        AppliedFilterChip::TimeRange(label) => label.clone(),
        AppliedFilterChip::MaxViews(v) => format!("曝光 ≤ {v}"),
        AppliedFilterChip::MinViews(v) => format!("曝光 ≥ {v}"),
        AppliedFilterChip::MaxEngagement(v) => format!("互动 ≤ {v}"),
        AppliedFilterChip::OlderThanDays(d) => format!("早于 {d} 天"),
        AppliedFilterChip::TopN(n) => format!("Top {n}"),
        AppliedFilterChip::Sort(label) => format!("排序：{label}"),
        AppliedFilterChip::Order(label) => label.clone(),
        AppliedFilterChip::Kinds(label) => label.clone(),
    }
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

/// Keyboard destinations and list actions that should work from any route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceShortcut {
    FocusNext,
    FocusPrev,
    ToggleFocusedSelection,
    OpenFilter,
    CloseOverlay,
    GoLibrary,
    GoInsights,
    GoCleanup,
}

/// Map a GPUI key name onto a workspace shortcut.
/// While the delete-confirm field is open, only Escape is claimed.
pub fn workspace_shortcut(key: &str, confirm_open: bool) -> Option<WorkspaceShortcut> {
    let key = key.to_ascii_lowercase();
    if confirm_open {
        return (key == "escape").then_some(WorkspaceShortcut::CloseOverlay);
    }
    match key.as_str() {
        "escape" => Some(WorkspaceShortcut::CloseOverlay),
        "j" | "down" | "arrowdown" => Some(WorkspaceShortcut::FocusNext),
        "k" | "up" | "arrowup" => Some(WorkspaceShortcut::FocusPrev),
        "space" => Some(WorkspaceShortcut::ToggleFocusedSelection),
        "/" | "slash" => Some(WorkspaceShortcut::OpenFilter),
        "1" => Some(WorkspaceShortcut::GoLibrary),
        "2" => Some(WorkspaceShortcut::GoInsights),
        "3" => Some(WorkspaceShortcut::GoCleanup),
        _ => None,
    }
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
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

/// Root application state rendered by the main window.
pub struct AppState {
    pub active_route: Route,
    /// Current UI appearance. Palette conversion is process-wide so GPUI
    /// elements created during render all observe the same mode.
    pub theme_mode: crate::theme::ThemeMode,
    pub credentials_ok: bool,
    pub credentials_msg: SharedString,
    pub credential_layout: CredentialLayout,
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
    pub filter_drawer_open: bool,
    pub filter_draft: FilterDraft,
    pub applied_filter: FilterOptions,
    pub fetch_limit: usize,
    pub loading: bool,
    pub status_msg: SharedString,
    pub error_msg: Option<SharedString>,
    pub summary: Summary,
    pub histogram: Vec<(String, usize)>,
    /// Last successful tweet sync timestamp (local time display string).
    pub last_synced_at: Option<SharedString>,
    /// Current responsive shell mode (updated from window viewport).
    pub layout_mode: LayoutMode,
    /// Survives route remounts so the library list keeps its scroll offset.
    pub library_scroll: UniformListScrollHandle,
    /// User-resized inspector column; leftover width is the tweet list.
    pub inspector_width_px: f32,
    pub split_drag: Option<SplitDrag>,
    /// Library banner after staging candidates without leaving the page.
    pub cleanup_notice: Option<SharedString>,
    /// First click arms「清空」; second click actually clears.
    pub clear_cleanup_armed: bool,
    /// Last「拉取并分析」failed (distinct from export / delete errors).
    pub last_fetch_failed: bool,
}

impl AppState {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        // Start every window in the documented light default. The palette
        // helper is global because render functions intentionally call
        // `theme::c(token)` without threading mode through every widget.
        crate::theme::set_mode(crate::theme::ThemeMode::Light);
        let env_file = Settings::discovered_env_file().map(|p| p.display().to_string());
        let (credentials_ok, credential_layout, credentials_msg) = match Settings::load() {
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
                (ok, layout, SharedString::from(layout.sidebar_zh()))
            }
            Err(e) => {
                logging::error(Stream::App, events::APP_CONFIG)
                    .outcome(Outcome::Error)
                    .field("error", e.to_string())
                    .emit();
                (
                    false,
                    CredentialLayout::Missing,
                    SharedString::from(format!("读取配置失败: {e}")),
                )
            }
        };

        let filter_draft = FilterDraft::default();
        let applied_filter = filter_draft.to_filter_options();

        Self {
            active_route: Route::Library,
            theme_mode: crate::theme::ThemeMode::Light,
            credentials_ok,
            credentials_msg,
            credential_layout,
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
            filter_drawer_open: false,
            filter_draft,
            applied_filter,
            fetch_limit: 100,
            loading: false,
            status_msg: SharedString::from("就绪"),
            error_msg: None,
            summary: Summary::default(),
            histogram: Vec::new(),
            last_synced_at: None,
            layout_mode: LayoutMode::Wide,
            library_scroll: UniformListScrollHandle::default(),
            inspector_width_px: LayoutMode::Wide.inspector_width(),
            split_drag: None,
            cleanup_notice: None,
            clear_cleanup_armed: false,
            last_fetch_failed: false,
        }
    }

    pub fn effective_inspector_width(&self) -> f32 {
        clamp_inspector_width(self.inspector_width_px)
    }

    pub fn begin_split_drag(&mut self, x: f32, cx: &mut Context<Self>) {
        self.split_drag = Some(SplitDrag {
            start_x: x,
            start_inspector_width: self.effective_inspector_width(),
        });
        cx.notify();
    }

    pub fn update_split_drag(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.split_drag else {
            return;
        };
        // Handle sits on the list's right edge: drag right → list grows → inspector shrinks.
        let next = drag.start_inspector_width - (x - drag.start_x);
        let clamped = clamp_inspector_width(next);
        if (clamped - self.inspector_width_px).abs() >= 0.5 {
            self.inspector_width_px = clamped;
            cx.notify();
        }
    }

    pub fn end_split_drag(&mut self, cx: &mut Context<Self>) {
        if self.split_drag.take().is_some() {
            cx.notify();
        }
    }

    pub fn reset_split_width(&mut self, cx: &mut Context<Self>) {
        self.inspector_width_px = self.layout_mode.inspector_width();
        self.split_drag = None;
        cx.notify();
    }

    /// True when the drawer draft differs from the currently applied filter.
    #[allow(dead_code)]
    pub fn filter_draft_is_dirty(&self) -> bool {
        self.filter_draft.to_filter_options() != self.applied_filter
    }

    pub fn applied_chips(&self) -> Vec<AppliedFilterChip> {
        applied_filter_chips(&self.applied_filter)
    }

    /// Clear one applied-filter chip and recompute results immediately.
    pub fn remove_applied_chip(&mut self, chip: AppliedFilterChip, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        match chip {
            AppliedFilterChip::TimeRange(_) => {
                self.filter_draft.time_range = TimeRange::All;
                self.filter_draft.newer_than_days = None;
            }
            AppliedFilterChip::MaxViews(_) => self.filter_draft.max_views = None,
            AppliedFilterChip::MinViews(_) => self.filter_draft.min_views = None,
            AppliedFilterChip::MaxEngagement(_) => self.filter_draft.max_engagement = None,
            AppliedFilterChip::OlderThanDays(_) => self.filter_draft.older_than_days = None,
            AppliedFilterChip::TopN(_) => self.filter_draft.top_n = None,
            AppliedFilterChip::Sort(_) => {
                self.filter_draft.sort = SortField::Views;
            }
            AppliedFilterChip::Order(_) => {
                self.filter_draft.order = SortOrder::Asc;
            }
            AppliedFilterChip::Kinds(_) => {
                self.filter_draft.kinds = KindFilter::all();
            }
        }
        self.apply_filters(cx);
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
        self.histogram = view_histogram(&self.filtered);
    }

    pub fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.active_route = route;
        if route == Route::Cleanup {
            self.cleanup_notice = None;
        }
        cx.notify();
    }

    pub fn dismiss_cleanup_notice(&mut self, cx: &mut Context<Self>) {
        self.cleanup_notice = None;
        cx.notify();
    }

    /// Toggle the appearance without changing route, filters, selections, or
    /// cleanup state. GPUI is notified so the full tree is rendered instantly.
    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_mode = crate::theme::toggle_mode();
        cx.notify();
    }

    pub fn toggle_filter_drawer(&mut self, cx: &mut Context<Self>) {
        self.filter_drawer_open = !self.filter_drawer_open;
        cx.notify();
    }

    pub fn focus_previous(&mut self, cx: &mut Context<Self>) {
        self.focus_relative(-1, cx);
    }

    pub fn focus_next(&mut self, cx: &mut Context<Self>) {
        self.focus_relative(1, cx);
    }

    fn focus_relative(&mut self, delta: isize, cx: &mut Context<Self>) {
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
        cx.notify();
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
    pub fn apply_histogram_bucket(&mut self, label: &str, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let Some((lo, hi)) = view_bucket_bounds(label) else {
            self.set_error(format!("未知曝光区间: {label}"));
            cx.notify();
            return;
        };
        // Open lower bound (0) means "no min"; otherwise use inclusive min.
        self.filter_draft.min_views = if lo == 0 { None } else { Some(lo) };
        self.filter_draft.max_views = hi;
        self.filter_draft.sort = SortField::Views;
        self.filter_draft.order = SortOrder::Asc;
        self.filter_draft.top_n = None;
        self.apply_filters(cx);
        self.active_route = Route::Library;
        self.status_msg = SharedString::from(format!(
            "已应用曝光区间「{label}」：{} 条结果",
            self.filtered.len()
        ));
        cx.notify();
    }

    /// Open the in-app second confirmation panel before irreversible deletion.
    pub fn begin_delete_confirm(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let n = self.cleanup_candidates.len();
        if n == 0 {
            self.set_error("请先加入安全清理候选");
            cx.notify();
            return;
        }
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            cx.notify();
            return;
        }
        if !self.has_valid_backup() || !self.has_valid_delete_preview() {
            self.set_error("真实删除前必须为当前候选完成备份并预演");
            cx.notify();
            return;
        }
        self.delete_confirm = Some(DeleteConfirm::cleanup(self.cleanup_candidates.clone()));
        self.status_msg =
            SharedString::from(format!("将删除 {n} 条。点「确认删除」继续，或取消。"));
        self.error_msg = None;
        cx.notify();
    }

    pub fn cancel_delete_confirm(&mut self, cx: &mut Context<Self>) {
        if self.delete_confirm.take().is_some() {
            self.status_msg = SharedString::from("已取消删除");
            logging::info(Stream::Audit, events::CLEANUP_CANCEL)
                .outcome(Outcome::Cancel)
                .field("stage", "confirm")
                .field("revision", self.cleanup_revision)
                .field("count", self.cleanup_candidates.len() as u64)
                .emit();
        }
        cx.notify();
    }

    pub fn delete_confirm_is_ready(&self) -> bool {
        self.delete_confirm
            .as_ref()
            .is_some_and(delete_confirm_ready)
    }

    pub fn add_cleanup_candidate(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.loading || !self.all_tweets.iter().any(|t| t.id == id) {
            return;
        }
        if self.cleanup_candidates.insert(id.to_owned()) {
            if let Some(tweet) = self.all_tweets.iter().find(|tweet| tweet.id == id) {
                self.cleanup_snapshot.insert(id.to_owned(), tweet.clone());
            }
            self.invalidate_cleanup_receipts();
            self.status_msg =
                SharedString::from(cleanup_staging_notice(self.cleanup_candidates.len()));
            self.cleanup_notice = Some(SharedString::from(cleanup_staging_notice(
                self.cleanup_candidates.len(),
            )));
            self.clear_cleanup_armed = false;
        }
        cx.notify();
    }

    pub fn remove_cleanup_candidate(&mut self, id: &str, cx: &mut Context<Self>) {
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
            self.status_msg = SharedString::from(format!(
                "已移除候选（{} 条）",
                self.cleanup_candidates.len()
            ));
        }
        cx.notify();
    }

    pub fn request_clear_cleanup(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.cleanup_candidates.is_empty() {
            return;
        }
        if self.clear_cleanup_armed {
            self.clear_cleanup_candidates(cx);
            return;
        }
        self.clear_cleanup_armed = true;
        self.status_msg = SharedString::from("再点一次「确认清空」才会移除全部候选");
        cx.notify();
    }

    pub fn clear_cleanup_candidates(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.cleanup_candidates.is_empty() {
            return;
        }
        self.cleanup_candidates.clear();
        self.cleanup_snapshot.clear();
        self.invalidate_cleanup_receipts();
        self.clear_cleanup_armed = false;
        self.cleanup_notice = None;
        self.status_msg = SharedString::from("已清空安全清理候选");
        cx.notify();
    }

    /// Open the library-direct delete confirm. Checkbox wins; focused row counts if none checked.
    pub fn request_library_delete(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let ids = ids_for_cleanup_staging(&self.selected, self.focused_tweet_id.as_deref());
        if ids.is_empty() {
            self.set_error("请先勾选推文，或点开一条再删除");
            cx.notify();
            return;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            cx.notify();
            return;
        }
        let n = ids.len();
        self.delete_confirm = Some(DeleteConfirm::library(ids.into_iter().collect()));
        self.status_msg = SharedString::from(library_delete_prompt(n));
        self.error_msg = None;
        cx.notify();
    }

    pub fn add_selected_to_cleanup(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let selected_ids =
            ids_for_cleanup_staging(&self.selected, self.focused_tweet_id.as_deref());
        if selected_ids.is_empty() {
            self.set_error("请先勾选推文，或点开一条再加入安全清理");
            cx.notify();
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
        self.cleanup_notice = Some(SharedString::from(cleanup_staging_notice(
            self.cleanup_candidates.len(),
        )));
        self.status_msg = SharedString::from(cleanup_staging_notice(self.cleanup_candidates.len()));
        cx.notify();
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
        self.preview_outcome = Some(outcome.clone());
        self.status_msg = SharedString::from(outcome.summary_line());
        Ok(outcome)
    }

    fn prepare_safe_delete(&mut self) -> Result<String, String> {
        self.ensure_backup_for_current_candidates()
    }

    fn apply_preview_outcome(&mut self, snapshot: HashSet<String>, outcome: PreviewOutcome) {
        let revision = outcome.revision;
        self.preview_receipt = Some(Receipt {
            revision,
            candidate_ids: snapshot.clone(),
        });
        self.previewed_delete_ids = snapshot;
        self.status_msg = SharedString::from(outcome.summary_line());
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

    /// Live X API lookup. `then_confirm` opens the in-app delete panel on success.
    fn start_live_preview(&mut self, then_confirm: bool, cx: &mut Context<Self>) {
        let ids: Vec<String> = self.cleanup_candidates.iter().cloned().collect();
        if ids.is_empty() {
            self.set_error("请先加入安全清理候选");
            cx.notify();
            return;
        }
        let revision = self.cleanup_revision;
        let snapshot = self.cleanup_candidates.clone();
        self.set_busy(format!("正在向 X 预演 {} 条…", ids.len()));
        cx.notify();
        let entity = cx.weak_entity();
        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let settings = Settings::load().map_err(|e| e.user_facing())?;
                    let client = XClient::new(settings).map_err(|e| e.user_facing())?;
                    client.lookup_tweets(&ids).map_err(|e| e.user_facing())
                })
                .await;
            entity
                .update(cx, |state, cx| {
                    state.loading = false;
                    match result {
                        Ok(lookup) => {
                            if state.cleanup_revision != revision
                                || state.cleanup_candidates != snapshot
                            {
                                state.set_error("候选已变化，请重新预演");
                            } else {
                                let outcome = preview_from_lookup(&snapshot, &lookup, revision);
                                state.error_msg = None;
                                state.apply_preview_outcome(snapshot, outcome);
                                if then_confirm {
                                    state.begin_delete_confirm(cx);
                                    return;
                                }
                            }
                        }
                        Err(err) => {
                            // Lookup is a separate X product; network/tier failures
                            // must not block backup+confirm+delete.
                            match state.run_local_preview() {
                                Ok(_) => {
                                    state.error_msg = None;
                                    state.status_msg = SharedString::from(format!(
                                        "X API 预演不可用，已改用本地缓存。原因: {err}（详见 logs/）"
                                    ));
                                    if then_confirm {
                                        state.begin_delete_confirm(cx);
                                        return;
                                    }
                                }
                                Err(_) => {
                                    state.set_error(format!(
                                        "预演失败: {err}（详见 logs/）"
                                    ));
                                }
                            }
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
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

    pub fn apply_sort_header(&mut self, field: SortField, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let (sort, order) =
            next_sort_from_header(self.filter_draft.sort, self.filter_draft.order, field);
        self.filter_draft.sort = sort;
        self.filter_draft.order = order;
        self.apply_filters(cx);
    }

    pub fn handle_workspace_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(shortcut) = workspace_shortcut(key, self.delete_confirm.is_some()) else {
            return false;
        };
        match shortcut {
            WorkspaceShortcut::FocusNext => {
                if self.active_route != Route::Library {
                    self.active_route = Route::Library;
                }
                self.focus_next(cx);
            }
            WorkspaceShortcut::FocusPrev => {
                if self.active_route != Route::Library {
                    self.active_route = Route::Library;
                }
                self.focus_previous(cx);
            }
            WorkspaceShortcut::ToggleFocusedSelection => {
                if let Some(id) = self.focused_tweet_id.clone() {
                    self.toggle_selected(&id, cx);
                } else {
                    return false;
                }
            }
            WorkspaceShortcut::OpenFilter => {
                self.active_route = Route::Library;
                self.filter_drawer_open = true;
                cx.notify();
            }
            WorkspaceShortcut::CloseOverlay => {
                if self.delete_confirm.is_some() {
                    self.cancel_delete_confirm(cx);
                } else if self.error_msg.is_some() {
                    self.clear_error();
                    cx.notify();
                } else if self.filter_drawer_open {
                    self.filter_drawer_open = false;
                    cx.notify();
                } else if self.focused_tweet_id.is_some() && self.layout_mode.inspector_as_overlay()
                {
                    self.focused_tweet_id = None;
                    cx.notify();
                } else {
                    return false;
                }
            }
            WorkspaceShortcut::GoLibrary => self.set_route(Route::Library, cx),
            WorkspaceShortcut::GoInsights => self.set_route(Route::Insights, cx),
            WorkspaceShortcut::GoCleanup => self.set_route(Route::Cleanup, cx),
        }
        true
    }

    pub fn apply_filters(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.applied_filter = self.filter_draft.to_filter_options();
        self.previewed_delete_ids.clear();
        self.recompute_filtered();
        self.status_msg = SharedString::from(format!(
            "已应用筛选：{} / {} 条",
            self.filtered.len(),
            self.all_tweets.len()
        ));
        self.error_msg = None;
        cx.notify();
    }

    pub fn select_all_filtered(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.selected = self.filtered.iter().map(|t| t.id.clone()).collect();
        self.previewed_delete_ids.clear();
        self.status_msg = SharedString::from(format!("已全选 {} 条", self.selected.len()));
        cx.notify();
    }

    pub fn select_none(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.selected.clear();
        self.previewed_delete_ids.clear();
        self.status_msg = SharedString::from("已取消全部选择");
        cx.notify();
    }

    pub fn invert_selection(&mut self, cx: &mut Context<Self>) {
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
        self.status_msg = SharedString::from(format!("已反选，当前 {} 条", self.selected.len()));
        cx.notify();
    }

    pub fn toggle_selected(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if self.selected.contains(id) {
            self.selected.remove(id);
        } else {
            self.selected.insert(id.to_string());
        }
        self.previewed_delete_ids.clear();
        cx.notify();
    }

    pub fn focus_tweet(&mut self, id: &str, cx: &mut Context<Self>) {
        self.focused_tweet_id = Some(id.to_string());
        cx.notify();
    }

    pub fn has_valid_delete_preview(&self) -> bool {
        self.has_valid_preview()
    }

    #[allow(dead_code)]
    pub fn selected_tweets(&self) -> Vec<Tweet> {
        if self.selected.is_empty() {
            return Vec::new();
        }
        self.filtered
            .iter()
            .filter(|t| self.selected.contains(&t.id))
            .cloned()
            .collect()
    }

    #[allow(dead_code)]
    pub fn export_target_tweets(&self) -> Vec<Tweet> {
        let selected = self.selected_tweets();
        if selected.is_empty() {
            self.filtered.clone()
        } else {
            selected
        }
    }

    pub fn set_busy(&mut self, msg: impl Into<SharedString>) {
        self.loading = true;
        self.status_msg = msg.into();
        self.error_msg = None;
    }

    pub fn set_error(&mut self, err: impl Into<SharedString>) {
        self.loading = false;
        let msg = err.into();
        logging::error(Stream::App, events::UI_ERROR)
            .outcome(Outcome::Error)
            .field("message", msg.to_string())
            .emit();
        self.error_msg = Some(msg.clone());
        self.status_msg = SharedString::from("出错");
    }

    pub fn clear_error(&mut self) {
        self.error_msg = None;
    }

    /// Run blocking work on the background executor, then update UI state on the main thread.
    pub fn run_blocking<T, W, D>(cx: &mut Context<Self>, work: W, on_done: D)
    where
        T: Send + 'static,
        W: FnOnce() -> T + Send + 'static,
        D: FnOnce(&mut AppState, T, &mut Context<Self>) + 'static,
    {
        let entity = cx.weak_entity();
        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let result = cx.background_executor().spawn(async move { work() }).await;
            entity
                .update(cx, |state, cx| {
                    on_done(state, result, cx);
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn reload_credentials(&mut self) {
        match Settings::load() {
            Ok(s) => {
                self.credential_layout = s.credential_layout();
                self.credentials_ok = s.has_oauth1();
                self.credentials_msg = SharedString::from(self.credential_layout.sidebar_zh());
            }
            Err(e) => {
                self.credentials_ok = false;
                self.credential_layout = CredentialLayout::Missing;
                self.credentials_msg = SharedString::from(format!("读取配置失败: {e}"));
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

    pub fn fetch_tweets(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.last_fetch_failed = true;
            self.set_error(self.credentials_block_reason());
            cx.notify();
            return;
        }

        let limit = self.fetch_limit;
        // Fetch broadly (include replies); kind filter applied client-side.
        // Retweets still controlled at API for volume.
        let exclude_retweets = !self.filter_draft.include_retweets;
        let exclude_replies = false;

        self.set_busy(format!("正在拉取推文（最多 {limit} 条）…"));
        cx.notify();

        Self::run_blocking(
            cx,
            move || -> Result<(User, Vec<Tweet>), String> {
                let settings = Settings::load().map_err(|e| e.user_facing())?;
                let client = XClient::new(settings).map_err(|e| e.user_facing())?;
                client
                    .fetch_own_tweets(limit, exclude_retweets, exclude_replies)
                    .map_err(|e| e.user_facing())
            },
            |state, result, _cx| {
                state.loading = false;
                match result {
                    Ok((user, tweets)) => {
                        let n = tweets.len();
                        state.last_fetch_failed = false;
                        state.credentials_ok = true;
                        state.credentials_msg =
                            SharedString::from(format!("凭证有效 · @{}", user.username));
                        state.current_user = Some(user);
                        state.all_tweets = tweets;
                        state.selected.clear();
                        state.focused_tweet_id = None;
                        let reset_safety_steps = refresh_candidate_snapshot(
                            &state.cleanup_candidates,
                            &mut state.cleanup_snapshot,
                            &state.all_tweets,
                        );
                        if reset_safety_steps {
                            state.invalidate_cleanup_receipts();
                        }
                        state.applied_filter = state.filter_draft.to_filter_options();
                        state.recompute_filtered();
                        state.last_synced_at = Some(SharedString::from(
                            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        ));
                        state.error_msg = None;
                        state.status_msg = if state.cleanup_candidates.is_empty() {
                            SharedString::from(format!(
                                "拉取完成：共 {n} 条，筛选后 {} 条",
                                state.filtered.len()
                            ))
                        } else {
                            SharedString::from(format!(
                                "拉取完成：共 {n} 条，筛选后 {} 条；已保留 {} 条候选，备份与预演已重置",
                                state.filtered.len(),
                                state.cleanup_candidates.len()
                            ))
                        };
                    }
                    Err(e) => {
                        state.last_fetch_failed = true;
                        state.set_error(format!("拉取失败: {e}"));
                    }
                }
            },
        );
    }

    pub fn refresh_whoami(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error(self.credentials_block_reason());
            cx.notify();
            return;
        }
        self.set_busy("正在获取当前用户…");
        cx.notify();

        Self::run_blocking(
            cx,
            || -> Result<User, String> {
                let settings = Settings::load().map_err(|e| e.user_facing())?;
                let client = XClient::new(settings).map_err(|e| e.user_facing())?;
                client.get_me().map_err(|e| e.user_facing())
            },
            |state, result, _cx| {
                state.loading = false;
                match result {
                    Ok(user) => {
                        state.credentials_ok = true;
                        state.credentials_msg =
                            SharedString::from(format!("凭证有效 · @{}", user.username));
                        state.status_msg =
                            SharedString::from(format!("当前用户: @{}", user.username));
                        state.current_user = Some(user);
                        state.error_msg = None;
                    }
                    Err(e) => state.set_error(format!("获取用户失败: {e}")),
                }
            },
        );
    }

    pub fn export_tweets(
        &mut self,
        format: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Library exports always represent the current filtered scope. Cleanup
        // backups use `export_candidates` and are intentionally separate.
        let tweets = self.filtered.clone();
        if tweets.is_empty() {
            self.set_error("没有可导出的推文（请先拉取并筛选）");
            cx.notify();
            return;
        }

        let ext = match format {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        };
        let suggested = format!("{}.{ext}", artifact_file_stem("library", Local::now()));
        let n = tweets.len();

        let dir = Settings::default_export_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path_rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        let entity = cx.weak_entity();

        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let path = match path_rx.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => {
                    entity
                        .update(cx, |state, cx| {
                            state.loading = false;
                            state.status_msg = SharedString::from("已取消导出");
                            cx.notify();
                        })
                        .ok();
                    return;
                }
                Ok(Err(_)) | Err(_) => {
                    let fallback = dir.join(&suggested);
                    entity
                        .update(cx, |state, cx| {
                            state.status_msg =
                                SharedString::from("未打开存盘框，改写到默认导出目录");
                            cx.notify();
                        })
                        .ok();
                    fallback
                }
            };

            let result = cx
                .background_executor()
                .spawn({
                    let tweets = tweets.clone();
                    let path = path.clone();
                    async move {
                        match format {
                            ExportFormat::Csv => export_csv(&tweets, &path)
                                .map(|p| p.display().to_string())
                                .map_err(|e| e.to_string()),
                            ExportFormat::Json => export_json(&tweets, &path)
                                .map(|p| p.display().to_string())
                                .map_err(|e| e.to_string()),
                        }
                    }
                })
                .await;

            entity
                .update(cx, |state, cx| {
                    state.loading = false;
                    match result {
                        Ok(p) => {
                            state.error_msg = None;
                            state.status_msg = SharedString::from(format!("已导出 {n} 条 → {p}"));
                        }
                        Err(e) => state.set_error(format!("导出失败: {e}")),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();

        let _ = window;
    }

    /// Export the immutable candidate snapshot and bind a backup receipt only on success.
    pub fn export_candidates(
        &mut self,
        format: ExportFormat,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tweets = self.cleanup_tweets();
        if tweets.is_empty() {
            self.set_error("暂无安全清理候选");
            cx.notify();
            return;
        }
        let candidate_ids = self.cleanup_candidates.clone();
        let revision = self.cleanup_revision;
        let ext = match format {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        };
        let suggested = format!("{}.{ext}", artifact_file_stem("cleanup", Local::now()));
        let n = tweets.len();
        let dir = Settings::default_export_dir();
        let _ = std::fs::create_dir_all(&dir);
        // Do not set loading before the file dialog: a stuck picker used to
        // freeze every cleanup action, including 真实删除.
        let path_rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        let entity = cx.weak_entity();
        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let path = match path_rx.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => {
                    entity
                        .update(cx, |state, cx| {
                            state.status_msg = SharedString::from("已取消备份");
                            cx.notify();
                        })
                        .ok();
                    return;
                }
                Ok(Err(_)) | Err(_) => dir.join(&suggested),
            };
            let result = cx
                .background_executor()
                .spawn({
                    let tweets = tweets.clone();
                    let path = path.clone();
                    async move {
                        match format {
                            ExportFormat::Csv => export_csv(&tweets, &path),
                            ExportFormat::Json => export_json(&tweets, &path),
                        }
                        .map(|p| p.display().to_string())
                        .map_err(|e| e.to_string())
                    }
                })
                .await;
            entity
                .update(cx, |state, cx| {
                    state.loading = false;
                    match result {
                        Ok(path) => {
                            let bound = state.cleanup_revision == revision
                                && state.cleanup_candidates == candidate_ids;
                            if bound {
                                state.backup_receipt = Some(Receipt {
                                    revision,
                                    candidate_ids: candidate_ids.clone(),
                                });
                            }
                            logging::info(Stream::Audit, events::CLEANUP_BACKUP)
                                .outcome(Outcome::Ok)
                                .field("revision", revision)
                                .field("count", n as u64)
                                .field("path", path.as_str())
                                .field("receipt_bound", bound)
                                .emit();
                            state.error_msg = None;
                            state.status_msg =
                                SharedString::from(format!("已备份 {n} 条候选 → {path}"));
                        }
                        Err(err) => {
                            logging::error(Stream::Audit, events::CLEANUP_BACKUP)
                                .outcome(Outcome::Error)
                                .field("revision", revision)
                                .field("count", n as u64)
                                .field("error", err.as_str())
                                .emit();
                            state.set_error(format!("备份失败: {err}"));
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    /// Apply a ranking preset: time window + sort metric + high/low + optional top-N.
    pub fn apply_rank_preset(
        &mut self,
        range: TimeRange,
        sort: SortField,
        order: SortOrder,
        top_n: Option<usize>,
        cx: &mut Context<Self>,
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
        self.apply_filters(cx);
    }

    /// Apply low-exposure quick filter within current time range.
    pub fn apply_low_exposure_preset(&mut self, threshold: u64, cx: &mut Context<Self>) {
        self.filter_draft.max_views = Some(threshold);
        self.filter_draft.min_views = None;
        self.filter_draft.sort = SortField::Views;
        self.filter_draft.order = SortOrder::Asc;
        self.filter_draft.top_n = None;
        self.apply_filters(cx);
    }

    /// Backup if needed, then dry-run against the X API (or local cache if no creds).
    #[allow(dead_code)]
    pub fn preview_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if self.loading {
            return;
        }
        match self.prepare_safe_delete() {
            Ok(_) => {
                self.reload_credentials();
                if self.credentials_ok {
                    self.start_live_preview(false, cx);
                } else if let Err(err) = self.run_local_preview() {
                    self.set_error(err);
                    cx.notify();
                } else {
                    cx.notify();
                }
            }
            Err(err) => {
                self.set_error(err);
                cx.notify();
            }
        }
    }

    /// Backup + live preview, then open the in-app confirm panel.
    pub fn delete_previewed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if self.loading {
            return;
        }
        if self.cleanup_candidates.is_empty() {
            self.set_error("请先加入安全清理候选");
            cx.notify();
            return;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            cx.notify();
            return;
        }
        if let Err(err) = self.prepare_safe_delete() {
            self.set_error(err);
            cx.notify();
            return;
        }
        self.start_live_preview(true, cx);
    }

    /// Commit real deletion after the in-app confirmation panel is accepted.
    pub fn confirm_and_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if !self.delete_confirm_is_ready() {
            self.set_error("请先确认删除");
            cx.notify();
            return;
        }
        let Some(confirm) = self.delete_confirm.take() else {
            self.set_error("请先确认删除");
            cx.notify();
            return;
        };
        match confirm.source {
            DeleteSource::Library => {
                self.active_route = Route::Library;
                if let Err(err) = self.backup_ids_to_exports(&confirm.ids) {
                    self.set_error(err);
                    cx.notify();
                    return;
                }
                let ids: Vec<String> = confirm.ids.into_iter().collect();
                self.execute_delete_ids(ids, false, cx);
            }
            DeleteSource::Cleanup => {
                if confirm.expected_count != self.cleanup_candidates.len()
                    || confirm.ids != self.cleanup_candidates
                {
                    self.set_error("候选数量已变化，请重新点删除");
                    cx.notify();
                    return;
                }
                self.execute_delete_ids(confirm.ids.into_iter().collect(), true, cx);
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

    fn execute_delete_ids(
        &mut self,
        ids: Vec<String>,
        require_cleanup_receipts: bool,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        if ids.is_empty() {
            self.set_error(if require_cleanup_receipts {
                "请先加入安全清理候选"
            } else {
                "请先勾选推文，或点开一条再删除"
            });
            cx.notify();
            return;
        }
        self.reload_credentials();
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            cx.notify();
            return;
        }
        if require_cleanup_receipts
            && (!self.has_valid_backup() || !self.has_valid_delete_preview())
        {
            self.set_error("真实删除前必须为当前候选完成备份并预演");
            cx.notify();
            return;
        }

        let n = ids.len();
        let revision = self.cleanup_revision;
        let snapshot: HashSet<String> = ids.iter().cloned().collect();
        self.set_busy(format!("正在删除 {n} 条…"));
        cx.notify();

        let entity = cx.weak_entity();
        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let valid = entity
                .update(cx, |state, cx| {
                    let valid = if require_cleanup_receipts {
                        state.has_valid_backup()
                            && state.has_valid_preview()
                            && state.cleanup_revision == revision
                            && state.cleanup_candidates == snapshot
                    } else {
                        state.credentials_ok
                    };
                    if !valid {
                        state.loading = false;
                        state.set_error(if require_cleanup_receipts {
                            "候选版本、备份或预演已变化，请重新执行安全清理步骤"
                        } else {
                            "未配置有效凭证，无法删除"
                        });
                        cx.notify();
                    }
                    valid
                })
                .unwrap_or(false);
            if !valid {
                return;
            }

            let result = cx
                .background_executor()
                .spawn(async move {
                    let settings = Settings::load().map_err(|e| e.user_facing())?;
                    let client = XClient::new(settings).map_err(|e| e.user_facing())?;
                    let outcome = client.delete_tweets(&ids);
                    Ok::<_, String>((ids, outcome))
                })
                .await;

            entity
                .update(cx, |state, cx| {
                    state.loading = false;
                    match result {
                        Ok((requested_ids, (ok, fail))) => {
                            let outcome = DeleteOutcome::from_request(&requested_ids, &fail);
                            if ok > 0 {
                                let deleted: HashSet<String> =
                                    outcome.succeeded.iter().cloned().collect();
                                state.apply_deleted_ids(&deleted);
                            }
                            state.last_delete_outcome = Some(outcome.clone());
                            state.status_msg = SharedString::from(outcome.summary_line());
                            let batch_outcome = if fail.is_empty() {
                                Outcome::Ok
                            } else if outcome.succeeded.is_empty() {
                                Outcome::Error
                            } else {
                                Outcome::Partial
                            };
                            logging::info(Stream::Audit, events::CLEANUP_DELETE)
                                .outcome(batch_outcome)
                                .field("revision", revision)
                                .field("requested", requested_ids.len() as u64)
                                .field("succeeded", outcome.succeeded.len() as u64)
                                .field("failed", fail.len() as u64)
                                .field("succeeded_ids", outcome.succeeded.clone())
                                .field(
                                    "failed_ids",
                                    fail.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                                )
                                .emit();
                            if fail.is_empty() {
                                state.error_msg = None;
                            } else {
                                let sample = fail
                                    .iter()
                                    .take(3)
                                    .map(|(id, e)| format!("{id}: {e}"))
                                    .collect::<Vec<_>>()
                                    .join("; ");
                                state.error_msg =
                                    Some(SharedString::from(format!("部分失败: {sample}")));
                            }
                        }
                        Err(e) => {
                            logging::error(Stream::Audit, events::CLEANUP_DELETE)
                                .outcome(Outcome::Error)
                                .field("revision", revision)
                                .field("requested", n as u64)
                                .field("error", e.as_str())
                                .emit();
                            state.set_error(format!("删除失败: {e}（详见 logs/）"));
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        applied_filter_chips, chip_label, clamp_inspector_width, classify_preview,
        cleanup_next_hint, cleanup_staging_notice, credentials_line_healthy, delete_confirm_ready,
        ids_for_cleanup_staging, is_default_cleanup_preset, library_delete_prompt,
        library_empty_copy, next_sort_from_header, preview_from_lookup, receipt_matches,
        refresh_candidate_snapshot, resolve_focused_tweet, tweet_preview_label, tweets_for_ids,
        workspace_shortcut, AppliedFilterChip, DeleteConfirm, DeleteOutcome, DeleteSource,
        FilterDraft, LayoutMode, LibraryEmptyKind, Receipt, WorkspaceShortcut, INSPECTOR_WIDTH_MAX,
        INSPECTOR_WIDTH_MIN,
    };
    use std::collections::{HashMap, HashSet};
    use xmanager_core::{
        CredentialLayout, FilterOptions, KindFilter, PublicMetrics, SortField, SortOrder,
        TimeRange, Tweet, TweetLookup,
    };

    #[test]
    fn receipt_requires_same_revision_and_candidate_snapshot() {
        let candidates = HashSet::from(["a".to_owned(), "b".to_owned()]);
        let receipt = Receipt {
            revision: 4,
            candidate_ids: candidates.clone(),
        };
        assert!(receipt_matches(Some(&receipt), 4, &candidates));
        assert!(!receipt_matches(Some(&receipt), 5, &candidates));
        let changed = HashSet::from(["a".to_owned()]);
        assert!(!receipt_matches(Some(&receipt), 4, &changed));
        assert!(!receipt_matches(None, 4, &candidates));
    }

    #[test]
    fn bounded_refresh_preserves_omitted_candidate_snapshot() {
        let candidates = HashSet::from(["old".to_owned(), "new".to_owned()]);
        let mut snapshot = HashMap::from([(
            "old".to_owned(),
            Tweet {
                id: "old".into(),
                text: "cached old".into(),
                created_at: None,
                public_metrics: PublicMetrics::default(),
                non_public_metrics: xmanager_core::NonPublicMetrics::default(),
                conversation_id: None,
                in_reply_to_user_id: None,
                is_retweet: false,
                is_quote: false,
            },
        )]);
        let fetched = vec![Tweet {
            id: "new".into(),
            text: "fresh new".into(),
            created_at: None,
            public_metrics: PublicMetrics::default(),
            non_public_metrics: xmanager_core::NonPublicMetrics::default(),
            conversation_id: None,
            in_reply_to_user_id: None,
            is_retweet: false,
            is_quote: false,
        }];
        assert!(refresh_candidate_snapshot(
            &candidates,
            &mut snapshot,
            &fetched
        ));
        assert_eq!(snapshot["old"].text, "cached old");
        assert_eq!(snapshot["new"].text, "fresh new");
        let (resolved, snapshot_only) =
            resolve_focused_tweet("old", &[], &snapshot).expect("cached candidate resolves");
        assert!(snapshot_only);
        assert_eq!(resolved.text, "cached old");
    }

    #[test]
    fn staging_ids_fall_back_to_focused_tweet() {
        let empty = HashSet::new();
        assert!(ids_for_cleanup_staging(&empty, None).is_empty());
        assert_eq!(
            ids_for_cleanup_staging(&empty, Some("42")),
            vec!["42".to_string()]
        );
        let selected = HashSet::from(["a".to_owned(), "b".to_owned()]);
        let mut ids = ids_for_cleanup_staging(&selected, Some("42"));
        ids.sort();
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn cleanup_hint_unlocks_delete_after_staging() {
        let empty = cleanup_next_hint(0, true, None);
        assert!(empty.contains("内容库"));
        let staged = cleanup_next_hint(2, true, None);
        assert!(staged.contains("删除 2 条"));
        let confirm = DeleteConfirm::cleanup(HashSet::from(["a".to_owned(), "b".to_owned()]));
        let waiting = cleanup_next_hint(2, true, Some(&confirm));
        assert!(waiting.contains("确认删除"));
        assert!(!waiting.contains("DELETE"));
        let empty = cleanup_next_hint(0, true, None);
        assert!(empty.contains("删除选中"));
        assert!(empty.contains("安全清理"));
    }

    #[test]
    fn delete_confirm_is_ready_when_count_is_positive() {
        assert!(!delete_confirm_ready(&DeleteConfirm::cleanup(
            HashSet::new()
        )));
        assert!(delete_confirm_ready(&DeleteConfirm::library(
            HashSet::from(["a".to_owned(), "b".to_owned(), "c".to_owned()])
        )));
    }

    #[test]
    fn library_delete_prompt_is_one_irreversible_confirm() {
        assert_eq!(library_delete_prompt(4), "将永久删除 4 条，不可恢复");
        let confirm = DeleteConfirm::library(HashSet::from(["1".to_owned(), "2".to_owned()]));
        assert_eq!(confirm.source, DeleteSource::Library);
        assert_eq!(confirm.expected_count, 2);
        assert!(delete_confirm_ready(&confirm));
    }

    #[test]
    fn tweets_for_ids_prefers_library_then_snapshot() {
        let all = vec![Tweet {
            id: "a".into(),
            text: "library".into(),
            created_at: None,
            public_metrics: PublicMetrics::default(),
            non_public_metrics: xmanager_core::NonPublicMetrics::default(),
            conversation_id: None,
            in_reply_to_user_id: None,
            is_retweet: false,
            is_quote: false,
        }];
        let snapshot = HashMap::from([(
            "b".to_owned(),
            Tweet {
                id: "b".into(),
                text: "snapshot".into(),
                created_at: None,
                public_metrics: PublicMetrics::default(),
                non_public_metrics: xmanager_core::NonPublicMetrics::default(),
                conversation_id: None,
                in_reply_to_user_id: None,
                is_retweet: false,
                is_quote: false,
            },
        )]);
        let ids = HashSet::from(["a".to_owned(), "b".to_owned(), "missing".to_owned()]);
        let tweets = tweets_for_ids(&ids, &all, &snapshot);
        assert_eq!(
            tweets.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(tweets[0].text, "library");
        assert_eq!(tweets[1].text, "snapshot");
    }

    #[test]
    fn unrestricted_draft_drops_the_low_view_preset() {
        let preset = FilterDraft::default();
        assert_eq!(preset.max_views, Some(50));
        assert!(is_default_cleanup_preset(&preset.to_filter_options()));
        let open = FilterDraft::unrestricted();
        assert_eq!(open.max_views, None);
        assert!(open.kinds.reply);
        assert!(open.kinds.retweet);
        assert!(open.time_range == TimeRange::All);
        assert!(open.top_n.is_none());
        assert!(!is_default_cleanup_preset(&open.to_filter_options()));
    }

    #[test]
    fn library_empty_copy_distinguishes_vacancy_filter_and_creds() {
        let never = library_empty_copy(false, 0, 0, false, false, CredentialLayout::Ready);
        assert_eq!(never.kind, LibraryEmptyKind::NeverSynced);
        assert!(never.detail.contains("曝光≤50"));

        let swapped = library_empty_copy(false, 0, 0, false, false, CredentialLayout::Swapped);
        assert_eq!(swapped.kind, LibraryEmptyKind::CredentialsSwapped);
        assert!(swapped.detail.contains("console.x.com"));
        assert!(!swapped.title.contains("缺少"));

        let missing = library_empty_copy(false, 0, 0, false, false, CredentialLayout::Missing);
        assert_eq!(missing.kind, LibraryEmptyKind::CredentialsMissing);
        assert!(missing.detail.contains(".env"), "{}", missing.detail);
        assert!(missing.detail.contains("刷新状态"), "{}", missing.detail);
        assert!(!missing.detail.contains("项目根目录"), "{}", missing.detail);

        let failed = library_empty_copy(false, 0, 0, false, true, CredentialLayout::Ready);
        assert_eq!(failed.kind, LibraryEmptyKind::FetchFailed);
        assert!(failed.title.contains("失败"));

        let filtered = library_empty_copy(false, 87, 0, true, false, CredentialLayout::Ready);
        assert_eq!(filtered.kind, LibraryEmptyKind::FilteredEmpty);
        assert!(filtered.title.contains("87"));
        assert!(filtered.detail.contains("清除筛选"));
        assert_eq!(LibraryEmptyKind::CredentialsSwapped.mark(), "换");
        assert_eq!(LibraryEmptyKind::FilteredEmpty.mark(), "筛");

        let account = library_empty_copy(false, 0, 0, true, false, CredentialLayout::Ready);
        assert_eq!(account.kind, LibraryEmptyKind::AccountEmpty);

        assert!(credentials_line_healthy(CredentialLayout::Ready, false));
        assert!(!credentials_line_healthy(CredentialLayout::Swapped, false));
        assert!(credentials_line_healthy(
            CredentialLayout::AccessTokenShapeMissing,
            true
        ));
    }

    #[test]
    fn default_scope_omits_sort_order_and_all_kinds_chips() {
        let filter = FilterOptions {
            max_views: None,
            min_views: None,
            max_engagement: None,
            older_than_days: None,
            newer_than_days: None,
            time_range: TimeRange::All,
            kinds: KindFilter::all(),
            include_replies: true,
            sort: SortField::Views,
            order: SortOrder::Asc,
            top_n: None,
        };
        let chips = applied_filter_chips(&filter);
        assert!(chips.is_empty(), "{chips:?}");
    }

    #[test]
    fn cleanup_staging_notice_stays_on_library() {
        let notice = cleanup_staging_notice(3);
        assert!(notice.contains("3 条"));
        assert!(notice.contains("继续勾选"));
        assert!(!notice.contains("已跳转"));
    }

    #[test]
    fn tweet_preview_label_prefers_excerpt_over_raw_id() {
        let snapshot = HashMap::from([(
            "abc".to_owned(),
            Tweet {
                id: "abc".into(),
                text: "一条很长的推文正文用来确认预演结果不再只显示 ID".into(),
                created_at: None,
                public_metrics: PublicMetrics::default(),
                non_public_metrics: xmanager_core::NonPublicMetrics::default(),
                conversation_id: None,
                in_reply_to_user_id: None,
                is_retweet: false,
                is_quote: false,
            },
        )]);
        let label = tweet_preview_label("abc", &snapshot, &[]);
        assert!(label.contains("一条很长"));
        assert!(!label.starts_with("abc"));
        assert_eq!(tweet_preview_label("missing", &snapshot, &[]), "missing");
    }

    #[test]
    fn applied_filter_chips_include_non_default_scope() {
        let filter = FilterOptions {
            max_views: Some(50),
            min_views: Some(1),
            max_engagement: Some(10),
            older_than_days: Some(90),
            newer_than_days: None,
            time_range: TimeRange::Days7,
            kinds: KindFilter {
                original: true,
                reply: false,
                retweet: false,
                quote: true,
            },
            include_replies: false,
            sort: SortField::LikeRate,
            order: SortOrder::Desc,
            top_n: Some(20),
        };
        let chips = applied_filter_chips(&filter);
        let labels: Vec<String> = chips.iter().map(chip_label).collect();
        assert!(labels.iter().any(|l| l.contains("7 天")));
        assert!(labels.iter().any(|l| l == "曝光 ≤ 50"));
        assert!(labels.iter().any(|l| l == "曝光 ≥ 1"));
        assert!(labels.iter().any(|l| l == "互动 ≤ 10"));
        assert!(labels.iter().any(|l| l == "早于 90 天"));
        assert!(labels.iter().any(|l| l == "Top 20"));
        assert!(labels.iter().any(|l| l.contains("点赞率")));
        assert!(labels.iter().any(|l| l == "最高优先"));
        assert!(labels.iter().any(|l| l.contains("原创")));
        assert!(matches!(
            chips
                .iter()
                .find(|c| matches!(c, AppliedFilterChip::MaxViews(_))),
            Some(AppliedFilterChip::MaxViews(50))
        ));
    }

    #[test]
    fn delete_outcome_splits_success_and_failure() {
        let requested = vec!["a".into(), "b".into(), "c".into()];
        let fail = vec![("b".into(), "gone".into())];
        let outcome = DeleteOutcome::from_request(&requested, &fail);
        assert_eq!(outcome.succeeded, vec!["a".to_owned(), "c".to_owned()]);
        assert_eq!(outcome.failed.len(), 1);
        assert_eq!(outcome.failed[0].0, "b");
        assert!(outcome.summary_line().contains("成功 2"));
        assert!(outcome.summary_line().contains("失败 1"));
    }

    #[test]
    fn preview_from_lookup_splits_found_missing_and_failed() {
        let candidates = HashSet::from(["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let lookup = TweetLookup {
            found: vec![Tweet {
                id: "a".into(),
                text: "keep".into(),
                created_at: None,
                public_metrics: PublicMetrics::default(),
                non_public_metrics: xmanager_core::NonPublicMetrics::default(),
                conversation_id: None,
                in_reply_to_user_id: None,
                is_retweet: false,
                is_quote: false,
            }],
            missing: vec!["b".into()],
            failed: vec![("c".into(), "forbidden".into())],
        };
        let outcome = preview_from_lookup(&candidates, &lookup, 3);
        assert_eq!(outcome.deletable, vec!["a".to_string()]);
        assert_eq!(outcome.missing, vec!["b".to_string()]);
        assert_eq!(outcome.failed, vec![("c".to_string(), "forbidden".into())]);
        assert_eq!(outcome.total, 3);
    }

    #[test]
    fn classify_preview_splits_known_and_missing() {
        let candidates = HashSet::from(["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let known = HashSet::from(["a".to_owned(), "c".to_owned()]);
        let outcome = classify_preview(&candidates, &known, 7);
        assert_eq!(outcome.revision, 7);
        assert_eq!(outcome.total, 3);
        assert_eq!(outcome.deletable, vec!["a".to_owned(), "c".to_owned()]);
        assert_eq!(outcome.missing, vec!["b".to_owned()]);
        assert!(outcome.failed.is_empty());
        assert!(outcome.summary_line().contains("可删 2"));
    }

    #[test]
    fn sort_header_toggles_same_field_and_defaults_new_field() {
        assert_eq!(
            next_sort_from_header(SortField::Views, SortOrder::Asc, SortField::Views),
            (SortField::Views, SortOrder::Desc)
        );
        assert_eq!(
            next_sort_from_header(SortField::Views, SortOrder::Asc, SortField::LikeRate),
            (SortField::LikeRate, SortOrder::Desc)
        );
        assert_eq!(
            next_sort_from_header(SortField::LikeRate, SortOrder::Desc, SortField::Views),
            (SortField::Views, SortOrder::Asc)
        );
    }

    #[test]
    fn workspace_shortcuts_yield_to_delete_confirm_except_escape() {
        assert_eq!(
            workspace_shortcut("j", false),
            Some(WorkspaceShortcut::FocusNext)
        );
        assert_eq!(
            workspace_shortcut("slash", false),
            Some(WorkspaceShortcut::OpenFilter)
        );
        assert_eq!(workspace_shortcut("j", true), None);
        assert_eq!(
            workspace_shortcut("Escape", true),
            Some(WorkspaceShortcut::CloseOverlay)
        );
        assert_eq!(
            workspace_shortcut("2", false),
            Some(WorkspaceShortcut::GoInsights)
        );
    }

    #[test]
    fn inspector_width_clamps_to_usable_range() {
        assert_eq!(clamp_inspector_width(100.0), INSPECTOR_WIDTH_MIN);
        assert_eq!(clamp_inspector_width(900.0), INSPECTOR_WIDTH_MAX);
        assert_eq!(clamp_inspector_width(380.0), 380.0);
    }

    #[test]
    fn layout_mode_breakpoints() {
        assert_eq!(LayoutMode::from_width(1440.0), LayoutMode::Wide);
        assert_eq!(LayoutMode::from_width(1200.0), LayoutMode::Wide);
        assert_eq!(LayoutMode::from_width(1000.0), LayoutMode::Medium);
        assert_eq!(LayoutMode::from_width(800.0), LayoutMode::Medium);
        assert_eq!(LayoutMode::from_width(799.0), LayoutMode::Narrow);
        assert!(LayoutMode::Wide.show_inspector());
        assert!(LayoutMode::Medium.show_inspector());
        assert!(!LayoutMode::Narrow.show_inspector());
        assert!(LayoutMode::Wide.show_nav_labels());
        assert!(!LayoutMode::Medium.show_nav_labels());
        assert!(LayoutMode::Narrow.inspector_width() > 0.0);
        assert!(LayoutMode::Wide.library_shows_inspector(false));
        assert!(!LayoutMode::Medium.library_shows_inspector(false));
        assert!(!LayoutMode::Medium.library_shows_inspector(true));
        assert!(!LayoutMode::Narrow.library_shows_inspector(false));
        assert!(!LayoutMode::Narrow.library_shows_inspector(true));
        assert!(!LayoutMode::Wide.filter_as_overlay());
        assert!(!LayoutMode::Wide.inspector_as_overlay());
        assert!(!LayoutMode::Medium.filter_as_overlay());
        assert!(LayoutMode::Medium.inspector_as_overlay());
        assert!(!LayoutMode::Medium.tweet_list_as_cards());
        assert!(LayoutMode::Narrow.filter_as_overlay());
        assert!(LayoutMode::Narrow.inspector_as_overlay());
        assert!(LayoutMode::Narrow.tweet_list_as_cards());
    }
}
