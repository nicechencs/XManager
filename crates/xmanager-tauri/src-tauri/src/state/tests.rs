//! Unit tests ported from the former GPUI app state (crates/xmanager-ui).

use super::{
    applied_filter_chips, classify_preview, cleanup_next_hint, cleanup_staging_notice,
    credentials_line_healthy, delete_confirm_ready, ids_for_cleanup_staging,
    is_default_cleanup_preset, library_delete_prompt, library_empty_copy, next_sort_from_header,
    preview_from_lookup, receipt_matches, refresh_candidate_snapshot, resolve_focused_tweet,
    tweet_preview_label, tweets_for_ids, AppliedFilterChip, DeleteConfirm, DeleteOutcome,
    DeleteSource, FilterDraft, LibraryEmptyKind, Receipt,
};
use std::collections::{HashMap, HashSet};
use xmanager_core::{
    CredentialLayout, FilterOptions, KindFilter, PublicMetrics, SortField, SortOrder, TimeRange,
    Tweet, TweetLookup,
};

fn tweet(id: &str, text: &str) -> Tweet {
    Tweet {
        id: id.into(),
        text: text.into(),
        created_at: None,
        public_metrics: PublicMetrics::default(),
        non_public_metrics: xmanager_core::NonPublicMetrics::default(),
        conversation_id: None,
        in_reply_to_user_id: None,
        is_retweet: false,
        is_quote: false,
    }
}

fn chip_label(chip: &AppliedFilterChip) -> String {
    match chip {
        AppliedFilterChip::TimeRange { label } => label.clone(),
        AppliedFilterChip::MaxViews { value } => format!("曝光 ≤ {value}"),
        AppliedFilterChip::MinViews { value } => format!("曝光 ≥ {value}"),
        AppliedFilterChip::MaxEngagement { value } => format!("互动 ≤ {value}"),
        AppliedFilterChip::OlderThanDays { days } => format!("早于 {days} 天"),
        AppliedFilterChip::TopN { n } => format!("Top {n}"),
        AppliedFilterChip::Sort { label } => format!("排序：{label}"),
        AppliedFilterChip::Order { label } => label.clone(),
        AppliedFilterChip::Kinds { label } => label.clone(),
    }
}

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
    let mut snapshot = HashMap::from([("old".to_owned(), tweet("old", "cached old"))]);
    let fetched = vec![tweet("new", "fresh new")];
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
    assert!(cleanup_next_hint(0, true, None).contains("删除选中"));
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
    let all = vec![tweet("a", "library")];
    let snapshot = HashMap::from([("b".to_owned(), tweet("b", "snapshot"))]);
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
        tweet("abc", "一条很长的推文正文用来确认预演结果不再只显示 ID"),
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
            .find(|c| matches!(c, AppliedFilterChip::MaxViews { value: 50 })),
        Some(AppliedFilterChip::MaxViews { value: 50 })
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
        found: vec![tweet("a", "keep")],
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
