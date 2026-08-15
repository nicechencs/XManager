//! Application state, async work helpers, and action handlers.

use std::collections::{HashMap, HashSet};

use chrono::Local;
use gpui::{AsyncApp, Context, PromptButton, PromptLevel, SharedString, WeakEntity, Window};
use xmanager_core::{
    export_csv, export_json, filter_tweets, summarize, view_bucket_bounds, view_histogram,
    FilterOptions, KindFilter, Settings, SortField, SortOrder, Summary, TimeRange, Tweet, User,
    XClient,
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
            Self::Medium => 88.0,
            Self::Narrow => 72.0,
        }
    }

    pub fn show_nav_labels(self) -> bool {
        matches!(self, Self::Wide)
    }

    /// Permanent inspector column (hidden on narrow until a tweet is focused).
    pub fn show_inspector(self) -> bool {
        !matches!(self, Self::Narrow)
    }

    /// Width used when the inspector column or focus overlay is visible.
    pub fn inspector_width(self) -> f32 {
        match self {
            Self::Wide => 380.0,
            Self::Medium => 300.0,
            // Narrow uses a compact overlay when a tweet is focused.
            Self::Narrow => 280.0,
        }
    }

    /// Whether the library should render the inspector right now.
    pub fn library_shows_inspector(self, has_focus: bool) -> bool {
        self.show_inspector() || has_focus
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Wide => "宽屏",
            Self::Medium => "中屏",
            Self::Narrow => "窄屏",
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Library,
    Insights,
    Cleanup,
}

/// Second-step confirmation tokens for irreversible deletion.
/// Spec requires either the exact candidate count or the word `DELETE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteConfirmToken {
    Count,
    DeleteWord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteConfirm {
    pub expected_count: usize,
    pub selected: Option<DeleteConfirmToken>,
}

/// Whether the chosen token unlocks real deletion for `expected_count` candidates.
pub fn delete_confirm_ready(confirm: &DeleteConfirm) -> bool {
    confirm.expected_count > 0
        && matches!(
            confirm.selected,
            Some(DeleteConfirmToken::Count | DeleteConfirmToken::DeleteWord)
        )
}

/// Removable applied-filter chips shown in the Library toolbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppliedFilterChip {
    TimeRange(String),
    MaxViews(u64),
    MinViews(u64),
    MaxEngagement(u64),
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
    if let Some(n) = filter.top_n {
        chips.push(AppliedFilterChip::TopN(n));
    }
    // Always surface the active sort metric so rank presets remain inspectable.
    chips.push(AppliedFilterChip::Sort(filter.sort.label_zh().to_string()));
    chips.push(AppliedFilterChip::Order(match filter.order {
        SortOrder::Asc => "最低优先".into(),
        SortOrder::Desc => "最高优先".into(),
    }));

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
    let kind_chip = if kind_labels.is_empty() {
        "类型：无".into()
    } else if kind_labels.len() == 4 {
        "类型：全部".into()
    } else {
        format!("类型：{}", kind_labels.join("/"))
    };
    chips.push(AppliedFilterChip::Kinds(kind_chip));
    chips
}

pub fn chip_label(chip: &AppliedFilterChip) -> String {
    match chip {
        AppliedFilterChip::TimeRange(label) => label.clone(),
        AppliedFilterChip::MaxViews(v) => format!("曝光 ≤ {v}"),
        AppliedFilterChip::MinViews(v) => format!("曝光 ≥ {v}"),
        AppliedFilterChip::MaxEngagement(v) => format!("互动 ≤ {v}"),
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
}

impl AppState {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        // Start every window in the documented light default. The palette
        // helper is global because render functions intentionally call
        // `theme::c(token)` without threading mode through every widget.
        crate::theme::set_mode(crate::theme::ThemeMode::Light);
        let (credentials_ok, credentials_msg) = match Settings::load() {
            Ok(s) if s.has_oauth1() => (true, SharedString::from("凭证已配置 ✓")),
            Ok(_) => (
                false,
                SharedString::from("缺少 OAuth 凭证，请配置 .env（X_API_KEY 等）"),
            ),
            Err(e) => (false, SharedString::from(format!("读取配置失败: {e}"))),
        };

        let filter_draft = FilterDraft::default();
        let applied_filter = filter_draft.to_filter_options();

        Self {
            active_route: Route::Library,
            theme_mode: crate::theme::ThemeMode::Light,
            credentials_ok,
            credentials_msg,
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
        }
    }

    /// True when the drawer draft differs from the currently applied filter.
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
        self.delete_confirm = Some(DeleteConfirm {
            expected_count: n,
            selected: None,
        });
        self.status_msg = SharedString::from(format!(
            "请确认删除 {n} 条：选择「确认数量 {n}」或「DELETE」"
        ));
        self.error_msg = None;
        cx.notify();
    }

    pub fn cancel_delete_confirm(&mut self, cx: &mut Context<Self>) {
        if self.delete_confirm.take().is_some() {
            self.status_msg = SharedString::from("已取消真实删除确认");
        }
        cx.notify();
    }

    pub fn select_delete_confirm_token(
        &mut self,
        token: DeleteConfirmToken,
        cx: &mut Context<Self>,
    ) {
        if let Some(confirm) = self.delete_confirm.as_mut() {
            confirm.selected = Some(token);
            cx.notify();
        }
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
            self.status_msg = SharedString::from(format!(
                "已加入安全清理候选（{} 条）",
                self.cleanup_candidates.len()
            ));
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
            self.status_msg = SharedString::from(format!(
                "已移除候选（{} 条）",
                self.cleanup_candidates.len()
            ));
        }
        cx.notify();
    }

    pub fn clear_cleanup_candidates(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.cleanup_candidates.is_empty() {
            return;
        }
        self.cleanup_candidates.clear();
        self.cleanup_snapshot.clear();
        self.invalidate_cleanup_receipts();
        self.status_msg = SharedString::from("已清空安全清理候选");
        cx.notify();
    }

    pub fn add_selected_to_cleanup(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.selected.is_empty() {
            return;
        }
        let before = self.cleanup_candidates.len();
        let selected_ids: Vec<String> = self.selected.iter().cloned().collect();
        self.cleanup_candidates.extend(selected_ids.iter().cloned());
        for id in selected_ids {
            if let Some(tweet) = self.all_tweets.iter().find(|tweet| tweet.id == id) {
                self.cleanup_snapshot.insert(id, tweet.clone());
            }
        }
        if self.cleanup_candidates.len() != before {
            self.invalidate_cleanup_receipts();
        }
        self.status_msg = SharedString::from(format!(
            "安全清理候选：{} 条",
            self.cleanup_candidates.len()
        ));
        self.active_route = Route::Cleanup;
        cx.notify();
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

    pub fn fetch_tweets(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法拉取推文");
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
                let settings = Settings::load().map_err(|e| e.to_string())?;
                let client = XClient::new(settings).map_err(|e| e.to_string())?;
                client
                    .fetch_own_tweets(limit, exclude_retweets, exclude_replies)
                    .map_err(|e| e.to_string())
            },
            |state, result, _cx| {
                state.loading = false;
                match result {
                    Ok((user, tweets)) => {
                        let n = tweets.len();
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
                    Err(e) => state.set_error(format!("拉取失败: {e}")),
                }
            },
        );
    }

    pub fn refresh_whoami(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if !self.credentials_ok {
            self.set_error("未配置有效凭证");
            cx.notify();
            return;
        }
        self.set_busy("正在获取当前用户…");
        cx.notify();

        Self::run_blocking(
            cx,
            || -> Result<User, String> {
                let settings = Settings::load().map_err(|e| e.to_string())?;
                let client = XClient::new(settings).map_err(|e| e.to_string())?;
                client.get_me().map_err(|e| e.to_string())
            },
            |state, result, _cx| {
                state.loading = false;
                match result {
                    Ok(user) => {
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
        let stamp = Local::now().format("%Y%m%d_%H%M%S");
        let suggested = format!("low_{stamp}.{ext}");
        let n = tweets.len();
        let source = "筛选结果";

        self.set_busy(format!("准备导出 {n} 条（{source}）…"));
        cx.notify();

        let dir = Settings::default_export_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path_rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        let entity = cx.weak_entity();

        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let chosen = match path_rx.await {
                Ok(Ok(Some(path))) => Some(path),
                Ok(Ok(None)) => None,
                Ok(Err(_)) | Err(_) => None,
            };

            let path = match chosen {
                Some(p) => p,
                None => {
                    // Fallback: write under exports/
                    let fallback = dir.join(&suggested);
                    entity
                        .update(cx, |state, cx| {
                            state.status_msg =
                                SharedString::from("未选择路径，使用默认 exports/ 目录");
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
        let suggested = format!(
            "xmanager_cleanup_{}.{}",
            Local::now().format("%Y%m%d_%H%M%S"),
            ext
        );
        let n = tweets.len();
        self.set_busy(format!("准备备份 {n} 条候选…"));
        cx.notify();
        let dir = Settings::default_export_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path_rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        let entity = cx.weak_entity();
        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let chosen = match path_rx.await {
                Ok(Ok(Some(path))) => Some(path),
                _ => None,
            };
            let path = chosen.unwrap_or_else(|| dir.join(&suggested));
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
                            if state.cleanup_revision == revision
                                && state.cleanup_candidates == candidate_ids
                            {
                                state.backup_receipt = Some(Receipt {
                                    revision,
                                    candidate_ids,
                                });
                            }
                            state.error_msg = None;
                            state.status_msg =
                                SharedString::from(format!("已备份 {n} 条候选 → {path}"));
                        }
                        Err(err) => state.set_error(format!("备份失败: {err}")),
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
        // When ranking rates/high performers, clear max_views so results aren't empty.
        if !matches!(sort, SortField::Views) || matches!(order, SortOrder::Desc) {
            self.filter_draft.max_views = None;
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

    /// Open the safe dry-run confirmation flow without changing the user's mode toggle.
    pub fn preview_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_valid_backup() {
            self.set_error("请先为当前候选创建备份");
            cx.notify();
            return;
        }
        self.delete_selected_mode(true, window, cx);
    }

    /// Real deletion is guarded by an unchanged, successful dry-run preview and
    /// an in-app second confirmation (count or DELETE).
    pub fn delete_previewed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Opening the panel does not start deletion; user must pick a token first.
        let _ = window;
        self.begin_delete_confirm(cx);
    }

    /// Commit real deletion after the in-app confirmation token is selected.
    pub fn confirm_and_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.delete_confirm_is_ready() {
            self.set_error("请先选择「确认数量」或「DELETE」以解锁真实删除");
            cx.notify();
            return;
        }
        let expected = self
            .delete_confirm
            .as_ref()
            .map(|c| c.expected_count)
            .unwrap_or(0);
        if expected != self.cleanup_candidates.len() {
            self.delete_confirm = None;
            self.set_error("候选数量已变化，请重新执行安全清理步骤");
            cx.notify();
            return;
        }
        // Consume the confirmation so a second click cannot re-enter without re-confirming.
        self.delete_confirm = None;
        self.delete_selected_mode(false, window, cx);
    }

    fn delete_selected_mode(&mut self, dry_run: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let ids: Vec<String> = self.cleanup_candidates.iter().cloned().collect();
        if ids.is_empty() {
            self.set_error("请先加入安全清理候选");
            cx.notify();
            return;
        }
        if !self.credentials_ok {
            self.set_error("未配置有效凭证，无法删除");
            cx.notify();
            return;
        }

        if !dry_run && (!self.has_valid_backup() || !self.has_valid_delete_preview()) {
            self.set_error("真实删除前必须为当前候选完成备份并预演");
            cx.notify();
            return;
        }

        let n = ids.len();
        let revision = self.cleanup_revision;
        let snapshot: HashSet<String> = ids.iter().cloned().collect();
        let title = if dry_run {
            format!("预演删除 {} 条？", n)
        } else {
            format!("最终确认：删除 {} 条？", n)
        };
        let detail = if dry_run {
            Some("当前为预演模式：不会真正删除，仅列出将删除的 ID。")
        } else {
            Some("删除后无法撤销。你已完成数量/DELETE 二次确认。")
        };

        // Mark the operation busy before opening the prompt so a second click
        // cannot enqueue another prompt/delete request.
        self.set_busy(if dry_run {
            "等待确认预演…"
        } else {
            "等待最终确认真实删除…"
        });
        cx.notify();

        let answer = window.prompt(
            PromptLevel::Warning,
            &title,
            detail,
            &[
                PromptButton::ok(if dry_run {
                    "开始预演"
                } else {
                    "立即删除"
                }),
                PromptButton::cancel("取消"),
            ],
            cx,
        );

        let entity = cx.weak_entity();
        cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let choice = answer.await.ok();
            if choice != Some(0) {
                entity
                    .update(cx, |state, cx| {
                        state.loading = false;
                        state.status_msg = SharedString::from("已取消删除");
                        cx.notify();
                    })
                    .ok();
                return;
            }

            let valid = entity
                .update(cx, |state, cx| {
                    let receipts_valid =
                        state.has_valid_backup() && (dry_run || state.has_valid_preview());
                    let snapshot_valid =
                        state.cleanup_revision == revision && state.cleanup_candidates == snapshot;
                    let valid = receipts_valid && snapshot_valid;
                    if !valid {
                        state.loading = false;
                        state.set_error(if dry_run {
                            "候选版本或备份已变化，请重新创建备份后再预演"
                        } else {
                            "候选版本、备份或预演已变化，请重新执行安全清理步骤"
                        });
                        cx.notify();
                    }
                    valid
                })
                .unwrap_or(false);
            if !valid {
                return;
            }

            entity
                .update(cx, |state, cx| {
                    state.status_msg = SharedString::from(if dry_run {
                        format!("预演删除 {n} 条…")
                    } else {
                        format!("正在删除 {n} 条…")
                    });
                    cx.notify();
                })
                .ok();

            if dry_run {
                entity
                    .update(cx, |state, cx| {
                        state.loading = false;
                        state.error_msg = None;
                        if state.cleanup_revision == revision
                            && state.cleanup_candidates == snapshot
                        {
                            let known: HashSet<String> = state
                                .all_tweets
                                .iter()
                                .map(|t| t.id.clone())
                                .chain(state.cleanup_snapshot.keys().cloned())
                                .collect();
                            let outcome = classify_preview(&snapshot, &known, revision);
                            state.preview_receipt = Some(Receipt {
                                revision,
                                candidate_ids: snapshot.clone(),
                            });
                            state.previewed_delete_ids = snapshot.clone();
                            state.status_msg = SharedString::from(outcome.summary_line());
                            state.preview_outcome = Some(outcome);
                        } else {
                            state.status_msg = SharedString::from(
                                "预演完成，但候选版本已变化，结果未写入",
                            );
                        }
                        cx.notify();
                    })
                    .ok();
                return;
            }

            let result = cx
                .background_executor()
                .spawn(async move {
                    let settings = Settings::load().map_err(|e| e.to_string())?;
                    let client = XClient::new(settings).map_err(|e| e.to_string())?;
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
                                // Reconcile against the immutable request, never a selection
                                // that may have changed while the API call was in flight.
                                let deleted: HashSet<String> =
                                    outcome.succeeded.iter().cloned().collect();
                                for id in &deleted {
                                    state.cleanup_snapshot.remove(id);
                                }
                                state.all_tweets.retain(|t| !deleted.contains(&t.id));
                                state.selected.retain(|id| !deleted.contains(id));
                                state.cleanup_candidates.retain(|id| !deleted.contains(id));
                                state.invalidate_cleanup_receipts();
                                state.recompute_filtered();
                            }
                            state.last_delete_outcome = Some(outcome.clone());
                            state.status_msg = SharedString::from(outcome.summary_line());
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
                        Err(e) => state.set_error(format!("删除失败: {e}")),
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
        applied_filter_chips, chip_label, classify_preview, delete_confirm_ready, receipt_matches,
        refresh_candidate_snapshot, resolve_focused_tweet, AppliedFilterChip, DeleteConfirm,
        DeleteConfirmToken, DeleteOutcome, LayoutMode, Receipt,
    };
    use std::collections::{HashMap, HashSet};
    use xmanager_core::{
        FilterOptions, KindFilter, PublicMetrics, SortField, SortOrder, TimeRange, Tweet,
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
    fn delete_confirm_requires_count_or_delete_word() {
        let empty = DeleteConfirm {
            expected_count: 0,
            selected: Some(DeleteConfirmToken::DeleteWord),
        };
        assert!(!delete_confirm_ready(&empty));

        let unselected = DeleteConfirm {
            expected_count: 3,
            selected: None,
        };
        assert!(!delete_confirm_ready(&unselected));

        let by_count = DeleteConfirm {
            expected_count: 3,
            selected: Some(DeleteConfirmToken::Count),
        };
        assert!(delete_confirm_ready(&by_count));

        let by_word = DeleteConfirm {
            expected_count: 3,
            selected: Some(DeleteConfirmToken::DeleteWord),
        };
        assert!(delete_confirm_ready(&by_word));
    }

    #[test]
    fn applied_filter_chips_include_non_default_scope() {
        let filter = FilterOptions {
            max_views: Some(50),
            min_views: Some(1),
            max_engagement: None,
            older_than_days: None,
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
        assert!(labels.iter().any(|l| l == "Top 20"));
        assert!(labels.iter().any(|l| l.contains("点赞率")));
        assert!(labels.iter().any(|l| l == "最高优先"));
        assert!(labels.iter().any(|l| l.contains("原创")));
        assert!(matches!(
            chips.iter().find(|c| matches!(c, AppliedFilterChip::MaxViews(_))),
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
        // Narrow keeps a usable overlay width and only opens when focused.
        assert!(LayoutMode::Narrow.inspector_width() > 0.0);
        assert!(!LayoutMode::Narrow.library_shows_inspector(false));
        assert!(LayoutMode::Narrow.library_shows_inspector(true));
        assert!(LayoutMode::Wide.library_shows_inspector(false));
    }
}
