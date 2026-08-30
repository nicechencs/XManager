//! UI intent actions sent from the web frontend. Every action is applied to
//! the [`Workspace`](crate::state::Workspace) synchronously and answered with
//! a fresh [`UiSnapshot`](crate::dto::UiSnapshot).

use serde::{Deserialize, Serialize};
use xmanager_core::{PostKind, SortField, SortOrder, TimeRange};

use crate::state::{AppliedFilterChip, Route};

/// Numeric stepper inputs in the filter drawer. Bounds mirror the GPUI stepper:
/// view/engagement steps allow 0–100000 (exposure) / 0–1000000 (engagement),
/// age allows 0–3600 days; 0 renders as 不限 in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepperField {
    MaxViews,
    MinViews,
    MaxEngagement,
    OlderThanDays,
}

impl StepperField {
    pub fn clamp(self, value: u64) -> u64 {
        let max = match self {
            StepperField::MaxViews | StepperField::MinViews => 100_000,
            StepperField::MaxEngagement => 1_000_000,
            StepperField::OlderThanDays => 3_600,
        };
        value.min(max)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    SetRoute {
        route: Route,
    },
    FocusNext,
    FocusPrev,
    FocusTweet {
        id: String,
    },
    /// Clicking the focused row again unfocuses it.
    ToggleTweetFocus {
        id: String,
    },
    UnfocusTweet,
    ToggleSelected {
        id: String,
    },
    ToggleFocusedSelection,
    SelectAll,
    SelectNone,
    InvertSelection,
    /// Click a sortable list header; same field flips, new field uses defaults.
    SortHeader {
        field: SortField,
    },
    SetTimeRange {
        range: TimeRange,
    },
    ToggleKind {
        kind: PostKind,
    },
    SetStepper {
        field: StepperField,
        value: u64,
    },
    SetSort {
        field: SortField,
    },
    SetOrder {
        order: SortOrder,
    },
    SetTopN {
        n: Option<usize>,
    },
    ApplyLowExposure {
        threshold: u64,
    },
    ApplyHistogramBucket {
        label: String,
    },
    ApplyRankPreset {
        range: TimeRange,
        sort: SortField,
        order: SortOrder,
        top_n: Option<usize>,
    },
    /// Open a tweet in the library, loosening filters if it is filtered out.
    OpenTweetInLibrary {
        id: String,
    },
    RemoveChip {
        chip: AppliedFilterChip,
    },
    ClearFilters,
    SetFetchLimit {
        limit: usize,
    },
    /// Toggles next-fetch retweet inclusion in the draft without refiltering.
    ToggleIncludeRetweets,
    /// Library delete: checkbox selection wins, else the focused tweet.
    RequestLibraryDelete,
    AddSelectedToCleanup,
    AddCleanupCandidate {
        id: String,
    },
    RemoveCleanupCandidate {
        id: String,
    },
    /// First click arms 清空, second click actually clears.
    RequestClearCleanup,
    CancelDeleteConfirm,
    DismissCleanupNotice,
    ClearError,
    /// Status-line only updates (e.g. 已取消导出 after a dismissed dialog).
    SetStatus {
        message: String,
    },
}
