// Mirror of the serde DTOs produced by crates/xmanager-tauri (dto.rs / state.rs).
// Field names and enum spellings must match the Rust definitions exactly.

export type RouteView = "library" | "insights" | "cleanup";

export type PostKind = "original" | "reply" | "retweet" | "quote";

/** Core enums serialize with PascalCase variant names (no rename_all in core). */
export type TimeRange =
  | "All"
  | "Hours24"
  | "Days7"
  | "Days30"
  | "Days90"
  | "Days180"
  | "Days360";

export type SortField =
  | "Views"
  | "Engagement"
  | "Date"
  | "LikeRate"
  | "BookmarkRate"
  | "EngagementRate"
  | "RetweetRate"
  | "ReplyRate";

export type SortOrder = "Asc" | "Desc";

export interface KindFilter {
  original: boolean;
  reply: boolean;
  retweet: boolean;
  quote: boolean;
}

export interface PublicMetrics {
  retweet_count: number;
  reply_count: number;
  like_count: number;
  quote_count: number;
  bookmark_count: number;
  impression_count: number;
}

export interface NonPublicMetrics {
  impression_count: number;
}

export interface Tweet {
  id: string;
  text: string;
  created_at: string | null;
  public_metrics: PublicMetrics;
  non_public_metrics: NonPublicMetrics;
  conversation_id: string | null;
  in_reply_to_user_id: string | null;
  is_retweet: boolean;
  is_quote: boolean;
}

export interface User {
  id: string;
  username: string;
  name: string;
}

export interface FilterOptions {
  max_views: number | null;
  min_views: number | null;
  max_engagement: number | null;
  older_than_days: number | null;
  newer_than_days: number | null;
  time_range: TimeRange;
  kinds: KindFilter;
  include_replies: boolean;
  sort: SortField;
  order: SortOrder;
  top_n: number | null;
}

export interface FilterDraft {
  max_views: number | null;
  min_views: number | null;
  max_engagement: number | null;
  older_than_days: number | null;
  newer_than_days: number | null;
  time_range: TimeRange;
  kinds: KindFilter;
  include_retweets: boolean;
  sort: SortField;
  order: SortOrder;
  top_n: number | null;
}

export type AppliedFilterChip =
  | { chip: "time_range"; label: string }
  | { chip: "max_views"; value: number }
  | { chip: "min_views"; value: number }
  | { chip: "max_engagement"; value: number }
  | { chip: "older_than_days"; days: number }
  | { chip: "top_n"; n: number }
  | { chip: "sort"; label: string }
  | { chip: "order"; label: string }
  | { chip: "kinds"; label: string };

export interface Summary {
  count: number;
  avg_views: number;
  median_views: number;
  min_views: number;
  max_views: number;
  total_engagement: number;
  total_views: number;
  avg_like_rate: number;
  avg_bookmark_rate: number;
  avg_engagement_rate: number;
  original_count: number;
  reply_count: number;
  retweet_count: number;
  quote_count: number;
}

export type CredentialLayout =
  | "missing"
  | "ready"
  | "swapped"
  | "access_token_shape_missing";

export interface CredentialPresence {
  api_key: boolean;
  api_secret: boolean;
  access_token: boolean;
  access_token_secret: boolean;
  bearer_token: boolean;
}

export interface CredentialsView {
  ok: boolean;
  layout: CredentialLayout;
  msg: string;
  presence: CredentialPresence;
  healthy: boolean;
  env_file: string | null;
}

export interface HistogramBucketView {
  label: string;
  count: number;
  overlaps: boolean;
}

export type LibraryEmptyKind =
  | "loading"
  | "credentials_missing"
  | "credentials_swapped"
  | "fetch_failed"
  | "never_synced"
  | "account_empty"
  | "filtered_empty";

export interface EmptyStateView {
  kind: LibraryEmptyKind;
  mark: string;
  title: string;
  detail: string;
}

export interface CleanupRowView {
  id: string;
  tweet: Tweet | null;
  preview: string;
}

export type DeleteSource = "library" | "cleanup";

export interface DeleteConfirmView {
  expected_count: number;
  source: DeleteSource;
  prompt: string;
  caption: string;
}

export interface PreviewOutcome {
  revision: number;
  total: number;
  deletable: string[];
  missing: string[];
  failed: [string, string][];
}

export interface DeleteOutcome {
  succeeded: string[];
  failed: [string, string][];
}

export interface CleanupView {
  count: number;
  rows: CleanupRowView[];
  next_hint: string;
  has_valid_backup: boolean;
  has_valid_preview: boolean;
  preview_outcome: PreviewOutcome | null;
  last_delete_outcome: DeleteOutcome | null;
  delete_confirm: DeleteConfirmView | null;
  notice: string | null;
  clear_armed: boolean;
}

export interface UiSnapshot {
  route: RouteView;
  loading: boolean;
  status_msg: string;
  error_msg: string | null;
  credentials: CredentialsView;
  current_user: User | null;
  all_tweets: Tweet[];
  filtered: Tweet[];
  selected: string[];
  focused_tweet_id: string | null;
  filter_draft: FilterDraft;
  applied_filter: FilterOptions;
  applied_chips: AppliedFilterChip[];
  summary: Summary;
  all_summary: Summary;
  histogram: HistogramBucketView[];
  last_synced_at: string | null;
  fetch_limit: number;
  empty_state: EmptyStateView;
  slice_stats_line: string;
  cleanup: CleanupView;
  default_export_dir: string;
}

export type StepperField =
  | "max_views"
  | "min_views"
  | "max_engagement"
  | "older_than_days";

export type Action =
  | { type: "set_route"; route: RouteView }
  | { type: "focus_next" }
  | { type: "focus_prev" }
  | { type: "focus_tweet"; id: string }
  | { type: "toggle_tweet_focus"; id: string }
  | { type: "unfocus_tweet" }
  | { type: "toggle_selected"; id: string }
  | { type: "toggle_focused_selection" }
  | { type: "select_all" }
  | { type: "select_none" }
  | { type: "invert_selection" }
  | { type: "sort_header"; field: SortField }
  | { type: "set_time_range"; range: TimeRange }
  | { type: "toggle_kind"; kind: PostKind }
  | { type: "set_stepper"; field: StepperField; value: number }
  | { type: "set_sort"; field: SortField }
  | { type: "set_order"; order: SortOrder }
  | { type: "set_top_n"; n: number | null }
  | { type: "apply_low_exposure"; threshold: number }
  | { type: "apply_histogram_bucket"; label: string }
  | {
      type: "apply_rank_preset";
      range: TimeRange;
      sort: SortField;
      order: SortOrder;
      top_n: number | null;
    }
  | { type: "open_tweet_in_library"; id: string }
  | { type: "remove_chip"; chip: AppliedFilterChip }
  | { type: "clear_filters" }
  | { type: "set_fetch_limit"; limit: number }
  | { type: "toggle_include_retweets" }
  | { type: "request_library_delete" }
  | { type: "add_selected_to_cleanup" }
  | { type: "add_cleanup_candidate"; id: string }
  | { type: "remove_cleanup_candidate"; id: string }
  | { type: "request_clear_cleanup" }
  | { type: "cancel_delete_confirm" }
  | { type: "dismiss_cleanup_notice" }
  | { type: "clear_error" }
  | { type: "set_status"; message: string };

export type ExportFormat = "csv" | "json";
