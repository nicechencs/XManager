//! Serializable view state returned by every command. The frontend is a pure
//! renderer over this snapshot; it never owns application state.

use serde::Serialize;
use xmanager_core::{CredentialLayout, CredentialPresence, FilterOptions, Summary, Tweet, User};

use crate::state::{
    cleanup_next_hint, library_empty_copy, slice_stats_line_for, tweet_preview_label, Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteView {
    Library,
    Insights,
    Cleanup,
}

#[derive(Debug, Clone, Serialize)]
pub struct CredentialsView {
    pub ok: bool,
    pub layout: CredentialLayout,
    pub msg: String,
    pub presence: CredentialPresence,
    /// Green (true) / amber (false) status pill in the sidebar and status bar.
    pub healthy: bool,
    pub env_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistogramBucketView {
    pub label: String,
    pub count: usize,
    /// Whether this bucket overlaps the applied min/max views filter; the
    /// insights histogram highlights the current slice this way.
    pub overlaps: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmptyStateView {
    pub kind: crate::state::LibraryEmptyKind,
    pub mark: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupRowView {
    pub id: String,
    pub tweet: Option<Tweet>,
    /// Human label: excerpt first, raw id when the snapshot lacks the tweet.
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteConfirmView {
    pub expected_count: usize,
    pub source: crate::state::DeleteSource,
    /// Library: 「将永久删除 N 条，不可恢复」. Cleanup: the long irreversible copy.
    pub prompt: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupView {
    pub count: usize,
    pub rows: Vec<CleanupRowView>,
    pub next_hint: String,
    pub has_valid_backup: bool,
    pub has_valid_preview: bool,
    pub preview_outcome: Option<crate::state::PreviewOutcome>,
    pub last_delete_outcome: Option<crate::state::DeleteOutcome>,
    pub delete_confirm: Option<DeleteConfirmView>,
    pub notice: Option<String>,
    pub clear_armed: bool,
}

/// Full view state handed to the frontend after every command.
#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshot {
    pub route: RouteView,
    pub loading: bool,
    pub status_msg: String,
    pub error_msg: Option<String>,
    pub credentials: CredentialsView,
    pub current_user: Option<User>,
    pub all_tweets: Vec<Tweet>,
    pub filtered: Vec<Tweet>,
    pub selected: Vec<String>,
    pub focused_tweet_id: Option<String>,
    pub filter_draft: crate::state::FilterDraft,
    pub applied_filter: FilterOptions,
    pub applied_chips: Vec<crate::state::AppliedFilterChip>,
    pub summary: Summary,
    pub all_summary: Summary,
    pub histogram: Vec<HistogramBucketView>,
    pub last_synced_at: Option<String>,
    pub fetch_limit: usize,
    pub empty_state: EmptyStateView,
    pub slice_stats_line: String,
    pub cleanup: CleanupView,
    pub default_export_dir: String,
}

fn empty_state_for(ws: &Workspace) -> EmptyStateView {
    let copy = library_empty_copy(
        ws.loading,
        ws.all_tweets.len(),
        ws.filtered.len(),
        ws.last_synced_at.is_some(),
        ws.last_fetch_failed,
        ws.credential_layout,
    );
    EmptyStateView {
        kind: copy.kind,
        mark: copy.kind.mark().to_string(),
        title: copy.title,
        detail: copy.detail,
    }
}

impl Workspace {
    /// Build the serializable view state. Histogram buckets are annotated with
    /// current-slice overlap here so the frontend stays free of filter math.
    pub fn snapshot(&self) -> UiSnapshot {
        let rows = {
            let mut ids: Vec<&String> = self.cleanup_candidates.iter().collect();
            ids.sort();
            ids.iter()
                .map(|id| CleanupRowView {
                    id: (*id).clone(),
                    tweet: self.cleanup_snapshot.get(*id).cloned(),
                    preview: tweet_preview_label(id, &self.cleanup_snapshot, &self.all_tweets),
                })
                .collect()
        };
        UiSnapshot {
            route: match self.active_route {
                crate::state::Route::Library => RouteView::Library,
                crate::state::Route::Insights => RouteView::Insights,
                crate::state::Route::Cleanup => RouteView::Cleanup,
            },
            loading: self.loading,
            status_msg: self.status_msg.clone(),
            error_msg: self.error_msg.clone(),
            credentials: CredentialsView {
                ok: self.credentials_ok,
                layout: self.credential_layout,
                msg: self.credentials_msg.clone(),
                presence: self.credential_presence,
                healthy: crate::state::credentials_line_healthy(
                    self.credential_layout,
                    self.current_user.is_some(),
                ),
                env_file: self.env_file.clone(),
            },
            current_user: self.current_user.clone(),
            all_tweets: self.all_tweets.clone(),
            filtered: self.filtered.clone(),
            selected: {
                let mut ids: Vec<String> = self.selected.iter().cloned().collect();
                ids.sort();
                ids
            },
            focused_tweet_id: self.focused_tweet_id.clone(),
            filter_draft: self.filter_draft.clone(),
            applied_filter: self.applied_filter.clone(),
            applied_chips: self.applied_chips(),
            summary: self.summary.clone(),
            all_summary: self.all_summary.clone(),
            histogram: self
                .histogram
                .iter()
                .map(|(label, count)| HistogramBucketView {
                    label: label.clone(),
                    count: *count,
                    overlaps: xmanager_core::bucket_overlaps_view_filter(
                        label,
                        self.applied_filter.min_views,
                        self.applied_filter.max_views,
                    ),
                })
                .collect(),
            last_synced_at: self.last_synced_at.clone(),
            fetch_limit: self.fetch_limit,
            empty_state: empty_state_for(self),
            slice_stats_line: slice_stats_line_for(&self.summary),
            cleanup: CleanupView {
                count: self.cleanup_candidates.len(),
                rows,
                next_hint: cleanup_next_hint(
                    self.cleanup_candidates.len(),
                    self.credentials_ok,
                    self.delete_confirm.as_ref(),
                ),
                has_valid_backup: self.has_valid_backup(),
                has_valid_preview: self.has_valid_preview(),
                preview_outcome: self.preview_outcome.clone(),
                last_delete_outcome: self.last_delete_outcome.clone(),
                delete_confirm: self.delete_confirm.as_ref().map(|confirm| {
                    let (prompt, caption) = match confirm.source {
                        crate::state::DeleteSource::Library => (
                            crate::state::library_delete_prompt(confirm.expected_count),
                            "确认后会自动备份到导出目录，再向 X 提交删除。仍留在内容库。"
                                .to_string(),
                        ),
                        crate::state::DeleteSource::Cleanup => (
                            format!(
                                "将永久删除 {} 条候选，此操作不可恢复",
                                confirm.expected_count
                            ),
                            "备份已写入导出目录。点确认后才会向 X 提交删除。".to_string(),
                        ),
                    };
                    DeleteConfirmView {
                        expected_count: confirm.expected_count,
                        source: confirm.source,
                        prompt,
                        caption,
                    }
                }),
                notice: self.cleanup_notice.clone(),
                clear_armed: self.clear_cleanup_armed,
            },
            default_export_dir: xmanager_core::Settings::default_export_dir()
                .display()
                .to_string(),
        }
    }
}
