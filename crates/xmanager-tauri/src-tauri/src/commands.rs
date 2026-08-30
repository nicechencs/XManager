//! Tauri command surface. Every command answers with the fresh [`UiSnapshot`]
//! so the frontend can blindly re-render. Blocking work (HTTP, file IO) runs
//! on the async runtime's blocking pool.

use std::sync::{Arc, Mutex, MutexGuard};

use tauri::State;
use xmanager_core::{Settings, Tweet, TweetLookup, User, XClient};

use crate::actions::Action;
use crate::dto::UiSnapshot;
use crate::state::{job, ExportFormat, Workspace};

pub type SharedWorkspace = Arc<Mutex<Workspace>>;

fn lock(state: &SharedWorkspace) -> MutexGuard<'_, Workspace> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Short CPU-only critical sections use a plain task; the mutex is never held
/// across an await.
#[tauri::command]
pub async fn initialize(state: State<'_, SharedWorkspace>) -> Result<UiSnapshot, String> {
    Ok(lock(state.inner()).snapshot())
}

#[tauri::command]
pub async fn dispatch(
    state: State<'_, SharedWorkspace>,
    action: Action,
) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    let mut ws = lock(&ws);
    apply_action(&mut ws, action);
    Ok(ws.snapshot())
}

fn apply_action(ws: &mut Workspace, action: Action) {
    use Action::*;
    match action {
        SetRoute { route } => ws.set_route(route),
        FocusNext => ws.focus_next(),
        FocusPrev => ws.focus_previous(),
        FocusTweet { id } => ws.focus_tweet(&id),
        ToggleTweetFocus { id } => ws.toggle_tweet_focus(&id),
        UnfocusTweet => ws.focused_tweet_id = None,
        ToggleSelected { id } => ws.toggle_selected(&id),
        ToggleFocusedSelection => {
            if let Some(id) = ws.focused_tweet_id.clone() {
                ws.toggle_selected(&id);
            }
        }
        SelectAll => ws.select_all_filtered(),
        SelectNone => ws.select_none(),
        InvertSelection => ws.invert_selection(),
        SortHeader { field } => ws.apply_sort_header(field),
        SetTimeRange { range } => {
            if !ws.loading {
                ws.filter_draft.time_range = range;
                ws.filter_draft.newer_than_days = None;
                ws.apply_filters();
            }
        }
        ToggleKind { kind } => {
            if !ws.loading {
                match kind {
                    xmanager_core::PostKind::Original => {
                        ws.filter_draft.kinds.original = !ws.filter_draft.kinds.original
                    }
                    xmanager_core::PostKind::Reply => {
                        ws.filter_draft.kinds.reply = !ws.filter_draft.kinds.reply
                    }
                    xmanager_core::PostKind::Retweet => {
                        ws.filter_draft.kinds.retweet = !ws.filter_draft.kinds.retweet
                    }
                    xmanager_core::PostKind::Quote => {
                        ws.filter_draft.kinds.quote = !ws.filter_draft.kinds.quote
                    }
                }
                ws.apply_filters();
            }
        }
        SetStepper { field, value } => {
            if !ws.loading {
                let value = field.clamp(value);
                let value = (value > 0).then_some(value);
                match field {
                    crate::actions::StepperField::MaxViews => ws.filter_draft.max_views = value,
                    crate::actions::StepperField::MinViews => ws.filter_draft.min_views = value,
                    crate::actions::StepperField::MaxEngagement => {
                        ws.filter_draft.max_engagement = value
                    }
                    crate::actions::StepperField::OlderThanDays => {
                        ws.filter_draft.older_than_days = value
                    }
                }
                ws.apply_filters();
            }
        }
        SetSort { field } => {
            if !ws.loading {
                ws.filter_draft.sort = field;
                ws.apply_filters();
            }
        }
        SetOrder { order } => {
            if !ws.loading {
                ws.filter_draft.order = order;
                ws.apply_filters();
            }
        }
        SetTopN { n } => {
            if !ws.loading {
                ws.filter_draft.top_n = n;
                ws.apply_filters();
            }
        }
        ApplyLowExposure { threshold } => ws.apply_low_exposure_preset(threshold),
        ApplyHistogramBucket { label } => ws.apply_histogram_bucket(&label),
        ApplyRankPreset {
            range,
            sort,
            order,
            top_n,
        } => ws.apply_rank_preset(range, sort, order, top_n),
        OpenTweetInLibrary { id } => ws.open_tweet_in_library(&id),
        RemoveChip { chip } => ws.remove_applied_chip(&chip),
        ClearFilters => {
            if !ws.loading {
                ws.filter_draft = crate::state::FilterDraft::unrestricted();
                ws.apply_filters();
            }
        }
        SetFetchLimit { limit } => {
            ws.fetch_limit = limit;
        }
        ToggleIncludeRetweets => {
            ws.filter_draft.include_retweets = !ws.filter_draft.include_retweets;
        }
        RequestLibraryDelete => ws.request_library_delete(),
        AddSelectedToCleanup => ws.add_selected_to_cleanup(),
        AddCleanupCandidate { id } => ws.add_cleanup_candidate(&id),
        RemoveCleanupCandidate { id } => ws.remove_cleanup_candidate(&id),
        RequestClearCleanup => ws.request_clear_cleanup(),
        CancelDeleteConfirm => ws.cancel_delete_confirm(),
        DismissCleanupNotice => ws.dismiss_cleanup_notice(),
        ClearError => ws.clear_error(),
        SetStatus { message } => ws.status_msg = message,
    }
}

#[tauri::command]
pub async fn fetch_tweets(state: State<'_, SharedWorkspace>) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    let Some(job) = lock(&ws).begin_fetch() else {
        return Ok(lock(&ws).snapshot());
    };
    let result =
        tauri::async_runtime::spawn_blocking(move || -> Result<(User, Vec<Tweet>), String> {
            let settings = Settings::load().map_err(|e| e.user_facing())?;
            let client = XClient::new(settings).map_err(|e| e.user_facing())?;
            client
                .fetch_own_tweets(job.limit, job.exclude_retweets, false)
                .map_err(|e| e.user_facing())
        })
        .await
        .map_err(|e| e.to_string())?;
    let mut ws = lock(&ws);
    ws.finish_fetch(result);
    Ok(ws.snapshot())
}

#[tauri::command]
pub async fn refresh_whoami(state: State<'_, SharedWorkspace>) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    if !lock(&ws).begin_whoami() {
        return Ok(lock(&ws).snapshot());
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<User, String> {
        let settings = Settings::load().map_err(|e| e.user_facing())?;
        let client = XClient::new(settings).map_err(|e| e.user_facing())?;
        client.get_me().map_err(|e| e.user_facing())
    })
    .await
    .map_err(|e| e.to_string())?;
    let mut ws = lock(&ws);
    ws.finish_whoami(result);
    Ok(ws.snapshot())
}

/// Cleanup-route delete: ensure backup, run the live X API preview (falling
/// back to the local cache), then open the in-app confirm panel.
#[tauri::command]
pub async fn delete_previewed(state: State<'_, SharedWorkspace>) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    let Some(job) = lock(&ws).delete_previewed() else {
        return Ok(lock(&ws).snapshot());
    };
    let ids = job.ids.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<TweetLookup, String> {
        let settings = Settings::load().map_err(|e| e.user_facing())?;
        let client = XClient::new(settings).map_err(|e| e.user_facing())?;
        client.lookup_tweets(&ids).map_err(|e| e.user_facing())
    })
    .await
    .map_err(|e| e.to_string())?;
    let mut ws = lock(&ws);
    ws.finish_live_preview(job, result);
    Ok(ws.snapshot())
}

#[tauri::command]
pub async fn confirm_and_delete(state: State<'_, SharedWorkspace>) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    let Some(job) = lock(&ws).confirm_and_delete() else {
        return Ok(lock(&ws).snapshot());
    };
    let ids = job.ids.clone();
    let result = tauri::async_runtime::spawn_blocking(
        move || -> Result<(usize, Vec<(String, String)>), String> {
            let settings = Settings::load().map_err(|e| e.user_facing())?;
            let client = XClient::new(settings).map_err(|e| e.user_facing())?;
            let (ok, fail) = client.delete_tweets(&ids);
            Ok((ok, fail))
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut ws = lock(&ws);
    ws.finish_delete(job, result);
    Ok(ws.snapshot())
}

#[tauri::command]
pub async fn export_tweets(
    state: State<'_, SharedWorkspace>,
    format: ExportFormat,
    path: Option<String>,
) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    let Some(job) = lock(&ws).begin_export_tweets(format, path.map(std::path::PathBuf::from))
    else {
        return Ok(lock(&ws).snapshot());
    };
    let kind = job.kind;
    let count = job.count;
    let revision = job.revision;
    let candidate_ids = job.candidate_ids.clone();
    let result = tauri::async_runtime::spawn_blocking(move || run_export(job))
        .await
        .map_err(|e| e.to_string())?;
    let mut ws = lock(&ws);
    ws.finish_export(kind, count, revision, candidate_ids, result);
    Ok(ws.snapshot())
}

#[tauri::command]
pub async fn export_candidates(
    state: State<'_, SharedWorkspace>,
    format: ExportFormat,
    path: Option<String>,
) -> Result<UiSnapshot, String> {
    let ws = state.inner().clone();
    let Some(job) = lock(&ws).begin_export_candidates(format, path.map(std::path::PathBuf::from))
    else {
        return Ok(lock(&ws).snapshot());
    };
    let kind = job.kind;
    let count = job.count;
    let revision = job.revision;
    let candidate_ids = job.candidate_ids.clone();
    let result = tauri::async_runtime::spawn_blocking(move || run_export(job))
        .await
        .map_err(|e| e.to_string())?;
    let mut ws = lock(&ws);
    ws.finish_export(kind, count, revision, candidate_ids, result);
    Ok(ws.snapshot())
}

fn run_export(job: job::ExportJob) -> Result<String, String> {
    match job.format {
        ExportFormat::Csv => xmanager_core::export_csv(&job.tweets, &job.path)
            .map(|p| p.display().to_string())
            .map_err(|e| e.to_string()),
        ExportFormat::Json => xmanager_core::export_json(&job.tweets, &job.path)
            .map(|p| p.display().to_string())
            .map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn default_export_dir() -> Result<String, String> {
    Ok(Settings::default_export_dir().display().to_string())
}
