//! Application shell and the three product routes.

use crate::app::{
    resolve_focused_tweet, AppState, DeleteConfirmToken, ExportFormat, Route,
};
use crate::theme;
use crate::views::{stats_panel, status_bar, toolbar, tweet_list};
use crate::widgets::{btn, danger_btn, section_label, stat_card, stepper, toggle_chip};
use gpui::{div, prelude::*, px, Context, Div, Stateful};
use xmanager_core::{KindFilter, SortField, SortOrder, TimeRange, Tweet};

fn clamp_opt(value: Option<u64>, delta: i64, min: u64, max: u64) -> Option<u64> {
    let next = (value.unwrap_or(0) as i64 + delta).clamp(min as i64, max as i64) as u64;
    (next > 0).then_some(next)
}

fn kind_chip(
    id: &'static str,
    label: &'static str,
    active: bool,
    field: fn(&mut KindFilter) -> &mut bool,
    cx: &mut Context<AppState>,
) -> impl gpui::IntoElement {
    toggle_chip(
        id,
        label,
        active,
        cx.listener(move |this, _, _window, cx| {
            let value = field(&mut this.filter_draft.kinds);
            *value = !*value;
            cx.notify();
        }),
    )
}

fn filter_drawer(state: &AppState, cx: &mut Context<AppState>) -> Stateful<Div> {
    let d = &state.filter_draft;
    div()
        .id("filter-drawer")
        .flex()
        .flex_col()
        .w(px(300.))
        .flex_none()
        .h_full()
        .gap_2()
        .p_3()
        .overflow_y_scroll()
        .bg(theme::c(theme::BG_PANEL))
        .border_r_1()
        .border_color(theme::c(theme::BORDER))
        .child(section_label("筛选与排序"))
        .child(section_label("拉取数量"))
        .child(div().flex().flex_row().flex_wrap().gap_1().children(
            [50usize, 100, 200, 500, 1000].into_iter().map(|n| {
                toggle_chip(
                    format!("fetch-limit-{n}"),
                    n.to_string(),
                    state.fetch_limit == n,
                    cx.listener(move |this, _, _window, cx| {
                        this.fetch_limit = n;
                        cx.notify();
                    }),
                )
            }),
        ))
        .child(section_label("时间范围"))
        .child(div().flex().flex_row().flex_wrap().gap_1().children(
            TimeRange::PRESETS.iter().copied().map(|range| {
                toggle_chip(
                    format!("time-{}", range.label_zh()),
                    range.label_zh(),
                    d.time_range == range,
                    cx.listener(move |this, _, _window, cx| {
                        this.filter_draft.time_range = range;
                        this.filter_draft.newer_than_days = None;
                        cx.notify();
                    }),
                )
            }),
        ))
        .child(section_label("帖子类型"))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .child(kind_chip(
                    "kind-original",
                    "原创",
                    d.kinds.original,
                    |k| &mut k.original,
                    cx,
                ))
                .child(kind_chip(
                    "kind-reply",
                    "回帖",
                    d.kinds.reply,
                    |k| &mut k.reply,
                    cx,
                ))
                .child(kind_chip(
                    "kind-retweet",
                    "转发",
                    d.kinds.retweet,
                    |k| &mut k.retweet,
                    cx,
                ))
                .child(kind_chip(
                    "kind-quote",
                    "引用",
                    d.kinds.quote,
                    |k| &mut k.quote,
                    cx,
                )),
        )
        .child(toggle_chip(
            "include-retweets",
            "拉取含转发",
            d.include_retweets,
            cx.listener(|this, _, _window, cx| {
                this.filter_draft.include_retweets = !this.filter_draft.include_retweets;
                cx.notify();
            }),
        ))
        .child(section_label("曝光上限"))
        .child({
            let entity = cx.weak_entity();
            stepper(
                "filter-max-views",
                d.max_views.unwrap_or(0),
                5,
                0,
                100_000,
                move |delta, _, app| {
                    entity
                        .update(app, |this, cx| {
                            this.filter_draft.max_views =
                                clamp_opt(this.filter_draft.max_views, delta, 0, 100_000);
                            cx.notify();
                        })
                        .ok();
                },
            )
        })
        .child(section_label("曝光下限"))
        .child({
            let entity = cx.weak_entity();
            stepper(
                "filter-min-views",
                d.min_views.unwrap_or(0),
                5,
                0,
                100_000,
                move |delta, _, app| {
                    entity
                        .update(app, |this, cx| {
                            this.filter_draft.min_views =
                                clamp_opt(this.filter_draft.min_views, delta, 0, 100_000);
                            cx.notify();
                        })
                        .ok();
                },
            )
        })
        .child(section_label("排序指标"))
        .child(
            div().flex().flex_row().flex_wrap().gap_1().children(
                [
                    (SortField::Views, "曝光"),
                    (SortField::LikeRate, "点赞率"),
                    (SortField::BookmarkRate, "收藏率"),
                    (SortField::EngagementRate, "互动率"),
                    (SortField::RetweetRate, "转发率"),
                    (SortField::ReplyRate, "回复率"),
                    (SortField::Engagement, "互动量"),
                    (SortField::Date, "日期"),
                ]
                .into_iter()
                .map(|(field, label)| {
                    toggle_chip(
                        format!("sort-{label}"),
                        label,
                        d.sort == field,
                        cx.listener(move |this, _, _window, cx| {
                            this.filter_draft.sort = field;
                            cx.notify();
                        }),
                    )
                }),
            ),
        )
        .child(section_label("低曝光快捷"))
        .child(
            div().flex().flex_row().flex_wrap().gap_1().children(
                [10u64, 20, 50, 100].into_iter().map(|threshold| {
                    toggle_chip(
                        format!("low-views-{threshold}"),
                        format!("≤{threshold}"),
                        d.max_views == Some(threshold)
                            && d.min_views.is_none()
                            && d.sort == SortField::Views
                            && d.order == SortOrder::Asc
                            && d.top_n.is_none(),
                        cx.listener(move |this, _, _window, cx| {
                            this.apply_low_exposure_preset(threshold, cx);
                            this.filter_drawer_open = false;
                        }),
                    )
                }),
            ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .child(toggle_chip(
                    "order-asc",
                    "最低",
                    d.order == SortOrder::Asc,
                    cx.listener(|this, _, _window, cx| {
                        this.filter_draft.order = SortOrder::Asc;
                        cx.notify();
                    }),
                ))
                .child(toggle_chip(
                    "order-desc",
                    "最高",
                    d.order == SortOrder::Desc,
                    cx.listener(|this, _, _window, cx| {
                        this.filter_draft.order = SortOrder::Desc;
                        cx.notify();
                    }),
                )),
        )
        .child(section_label("结果数量"))
        .child(
            div().flex().flex_row().flex_wrap().gap_1().children(
                [None, Some(10), Some(20), Some(50), Some(100)]
                    .into_iter()
                    .map(|n: Option<usize>| {
                        let label = n
                            .map(|v| format!("Top {v}"))
                            .unwrap_or_else(|| "全部".into());
                        toggle_chip(
                            format!("top-{}", label),
                            label,
                            d.top_n == n,
                            cx.listener(move |this, _, _window, cx| {
                                this.filter_draft.top_n = n;
                                cx.notify();
                            }),
                        )
                    }),
            ),
        )
        .child(btn(
            "apply-filters",
            "应用筛选",
            true,
            !state.loading,
            cx.listener(|this, _, _window, cx| {
                this.apply_filters(cx);
                this.filter_drawer_open = false;
                cx.notify();
            }),
        ))
        .child(btn(
            "clear-filters",
            "清除筛选",
            false,
            !state.loading,
            cx.listener(|this, _, _window, cx| {
                this.filter_draft = crate::app::FilterDraft::default();
                this.apply_filters(cx);
                this.filter_drawer_open = false;
                cx.notify();
            }),
        ))
}

fn context_metric(label: impl Into<String>, value: impl Into<String>) -> Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .py_1()
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT))
                .child(value.into()),
        )
}

fn inspector(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let width = state.layout_mode.inspector_width();
    let focused = state
        .focused_tweet_id
        .as_ref()
        .and_then(|id| resolve_focused_tweet(id, &state.filtered, &state.cleanup_snapshot));
    let body = focused.map(|(tweet, snapshot_only)| {
        let metrics = &tweet.public_metrics;
        let id = tweet.id.clone();
        let candidate = state.cleanup_candidates.contains(&id);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(theme::c(theme::TEXT))
                    .child(tweet.text.clone()),
            )
            .when(snapshot_only, |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(theme::c(theme::WARNING))
                        .child("不在当前筛选结果中，显示候选快照"),
                )
            })
            .child(context_metric("类型", tweet.kind().label_zh().to_string()))
            .child(context_metric("发布时间", tweet.display_date()))
            .child(context_metric("曝光", tweet.views().to_string()))
            .child(context_metric(
                "点赞 / 收藏",
                format!("{} / {}", metrics.like_count, metrics.bookmark_count),
            ))
            .child(context_metric(
                "互动率",
                Tweet::format_rate(tweet.engagement_rate()),
            ))
            .child(context_metric("推文 ID", id.clone()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(btn(
                        "focus-prev",
                        "上一条",
                        false,
                        !state.loading,
                        cx.listener(|this, _, _window, cx| this.focus_previous(cx)),
                    ))
                    .child(btn(
                        "focus-next",
                        "下一条",
                        false,
                        !state.loading,
                        cx.listener(|this, _, _window, cx| this.focus_next(cx)),
                    )),
            )
            .child(btn(
                "candidate-toggle",
                if candidate {
                    "移出安全清理"
                } else {
                    "加入安全清理"
                },
                !candidate,
                !state.loading,
                cx.listener(move |this, _, _window, cx| {
                    if candidate {
                        this.remove_cleanup_candidate(&id, cx)
                    } else {
                        this.add_cleanup_candidate(&id, cx)
                    }
                }),
            ))
    });
    let narrow_overlay = !state.layout_mode.show_inspector();
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .flex_none()
        .h_full()
        .gap_3()
        .p_4()
        .bg(theme::c(theme::BG_PANEL))
        .border_l_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(if focused.is_some() {
                            if narrow_overlay {
                                "推文检查（窄屏）"
                            } else {
                                "推文检查"
                            }
                        } else {
                            "检查器"
                        }),
                )
                .when(narrow_overlay && focused.is_some(), |el| {
                    el.child(btn(
                        "inspector-close",
                        "关闭",
                        false,
                        true,
                        cx.listener(|this, _, _window, cx| {
                            this.focused_tweet_id = None;
                            cx.notify();
                        }),
                    ))
                }),
        )
        .child(body.unwrap_or_else(|| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(section_label("选择一条推文查看完整内容"))
                .child(stat_card("当前结果", state.filtered.len().to_string()))
                .child(stat_card(
                    "安全清理候选",
                    state.cleanup_candidates.len().to_string(),
                ))
        }))
}

fn error_banner(state: &AppState, cx: &mut Context<AppState>) -> Option<Div> {
    state.error_msg.as_ref().map(|msg| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_2()
            .bg(theme::c(theme::ERROR_BG))
            .child(
                div()
                    .text_sm()
                    .text_color(theme::c(theme::TEXT))
                    .child(msg.clone()),
            )
            .child(btn(
                "dismiss-error",
                "关闭",
                false,
                true,
                cx.listener(|this, _, _window, cx| {
                    this.clear_error();
                    cx.notify();
                }),
            ))
    })
}

fn navigation_sidebar(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let show_labels = state.layout_mode.show_nav_labels();
    let nav_w = state.layout_mode.nav_width();
    let route_button = |id: &'static str,
                        full: &'static str,
                        short: &'static str,
                        route: Route,
                        active: bool,
                        badge: Option<usize>| {
        let base = if show_labels { full } else { short };
        let label = badge
            .map(|count| {
                if show_labels {
                    format!("{base}  {count}")
                } else {
                    format!("{base}:{count}")
                }
            })
            .unwrap_or_else(|| base.into());
        toggle_chip(
            id,
            label,
            active,
            cx.listener(move |this, _, _window, cx| this.set_route(route, cx)),
        )
    };
    div()
        .flex()
        .flex_col()
        .w(px(nav_w))
        .flex_none()
        .h_full()
        .p_3()
        .gap_2()
        .bg(theme::c(theme::BG_PANEL))
        .border_r_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .child(if show_labels { "XManager" } else { "XM" }),
        )
        .when(show_labels, |el| el.child(section_label("工作台")))
        .child(route_button(
            "nav-library",
            "内容库",
            "库",
            Route::Library,
            state.active_route == Route::Library,
            None,
        ))
        .child(route_button(
            "nav-insights",
            "数据洞察",
            "析",
            Route::Insights,
            state.active_route == Route::Insights,
            None,
        ))
        .child(route_button(
            "nav-cleanup",
            "安全清理",
            "清",
            Route::Cleanup,
            state.active_route == Route::Cleanup,
            Some(state.cleanup_candidates.len()),
        ))
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(if show_labels {
                    format!("{} 条数据", state.all_tweets.len())
                } else {
                    format!("{}", state.all_tweets.len())
                }),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(match &state.last_synced_at {
                    Some(ts) if show_labels => format!("同步 {ts}"),
                    Some(ts) => ts.to_string(),
                    None if show_labels => "尚未同步".into(),
                    None => "—".into(),
                }),
        )
        .when(show_labels, |el| {
            el.child(
                div()
                    .text_xs()
                    .text_color(if state.credentials_ok {
                        theme::c(theme::SUCCESS)
                    } else {
                        theme::c(theme::WARNING)
                    })
                    .child(state.credentials_msg.clone()),
            )
        })
        .child(btn(
            "sidebar-refresh",
            if show_labels { "刷新状态" } else { "刷" },
            false,
            !state.loading,
            cx.listener(|this, _, _window, cx| this.refresh_whoami(cx)),
        ))
        .child(btn(
            "sidebar-theme",
            if show_labels {
                format!("外观：{}", state.theme_mode.label_zh())
            } else {
                state.theme_mode.label_zh().to_string()
            },
            false,
            true,
            cx.listener(|this, _, _window, cx| this.toggle_theme(cx)),
        ))
}

fn loading_banner(state: &AppState) -> Option<Div> {
    if !state.loading {
        return None;
    }
    let keep_old = !state.all_tweets.is_empty();
    Some(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .bg(theme::c(theme::BG_ELEVATED))
            .border_b_1()
            .border_color(theme::c(theme::BORDER))
            .child(
                div()
                    .text_sm()
                    .text_color(theme::c(theme::ACCENT))
                    .child(if keep_old {
                        "正在更新…已保留当前列表，完成后自动刷新。"
                    } else {
                        "首次加载中，请稍候…"
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(theme::c(theme::TEXT_MUTED))
                    .child(state.status_msg.clone()),
            ),
    )
}

fn render_library(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let show_inspector = state
        .layout_mode
        .library_shows_inspector(state.focused_tweet_id.is_some());
    let mut content = div().flex().flex_row().flex_1().min_h(px(0.)).min_w(px(0.));
    if state.filter_drawer_open {
        content = content.child(filter_drawer(state, cx));
    }
    content = content.child(
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .child(toolbar::render_toolbar(state, cx))
            .children(loading_banner(state))
            .child(tweet_list::render_tweet_list(state, cx))
            .child(toolbar::render_bulk_bar(state, cx)),
    );
    if show_inspector {
        content = content.child(inspector(state, cx));
    }
    content
}

fn render_insights(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let can_export = !state.loading && !state.filtered.is_empty();
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .p_4()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("数据洞察"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(btn(
                            "insights-export-csv",
                            "导出 CSV",
                            false,
                            can_export,
                            cx.listener(|this, _, window, cx| {
                                this.export_tweets(ExportFormat::Csv, window, cx)
                            }),
                        ))
                        .child(btn(
                            "insights-export-json",
                            "导出 JSON",
                            false,
                            can_export,
                            cx.listener(|this, _, window, cx| {
                                this.export_tweets(ExportFormat::Json, window, cx)
                            }),
                        )),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child("统计范围 = 内容库当前已生效筛选；点击直方图或预设会回到内容库。"),
        )
        .child(stats_panel::render_stats_panel(state, cx))
        .child(section_label("排名预设"))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(btn(
                    "insight-like-high",
                    "7天 · 点赞率最高",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_rank_preset(
                            TimeRange::Days7,
                            SortField::LikeRate,
                            SortOrder::Desc,
                            Some(20),
                            cx,
                        );
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(btn(
                    "insight-low-views",
                    "30天 · 曝光最低",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_rank_preset(
                            TimeRange::Days30,
                            SortField::Views,
                            SortOrder::Asc,
                            Some(50),
                            cx,
                        );
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(btn(
                    "insight-bookmark",
                    "30天 · 收藏率最高",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_rank_preset(
                            TimeRange::Days30,
                            SortField::BookmarkRate,
                            SortOrder::Desc,
                            Some(20),
                            cx,
                        );
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(btn(
                    "insight-low-10",
                    "≤10 曝光",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(10, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(btn(
                    "insight-low-20",
                    "≤20 曝光",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(20, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(btn(
                    "insight-low-50",
                    "≤50 曝光待清理",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(50, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(btn(
                    "insight-low-100",
                    "≤100 曝光",
                    false,
                    !state.loading,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(100, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                )),
        )
}

fn render_cleanup(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let candidate_ids: Vec<String> = state.cleanup_candidates.iter().cloned().collect();
    let rows = candidate_ids.iter().map(|id| {
        let tweet = state.cleanup_snapshot.get(id);
        let id_for_focus = id.clone();
        let id_for_remove = id.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(theme::c(theme::BG_ROW))
            .border_b_1()
            .border_color(theme::c(theme::BORDER))
            .child(
                div().flex_1().text_sm().child(
                    tweet
                        .map(|t| crate::widgets::truncate_text(&t.text.replace('\n', " "), 90))
                        .unwrap_or_else(|| id.clone()),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::c(theme::TEXT_MUTED))
                    .child(id.clone()),
            )
            .child(btn(
                format!("cleanup-focus-{id}"),
                "查看",
                false,
                !state.loading,
                cx.listener(move |this, _, _window, cx| {
                    this.focus_tweet(&id_for_focus, cx);
                    this.active_route = Route::Library;
                    cx.notify();
                }),
            ))
            .child(btn(
                format!("cleanup-remove-{id}"),
                "移除",
                false,
                !state.loading,
                cx.listener(move |this, _, _window, cx| {
                    this.remove_cleanup_candidate(&id_for_remove, cx)
                }),
            ))
    });
    let has_candidates = !state.loading && !candidate_ids.is_empty();
    let can_preview_delete = has_candidates && state.credentials_ok;
    let can_open_delete_confirm =
        can_preview_delete && state.has_valid_backup() && state.has_valid_preview();
    let confirm = state.delete_confirm.as_ref();
    let confirm_ready = state.delete_confirm_is_ready();
    let expected_count = confirm.map(|c| c.expected_count).unwrap_or(candidate_ids.len());
    let selected_token = confirm.and_then(|c| c.selected);
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .p_4()
        .gap_3()
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("安全清理"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child("候选集合会随着每次修改生成新版本；备份与预演必须针对同一版本。"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(step_card("1", "复核候选", !candidate_ids.is_empty()))
                .child(step_card("2", "创建备份", state.has_valid_backup()))
                .child(step_card("3", "预演删除", state.has_valid_preview()))
                .child(step_card(
                    "4",
                    "真实删除",
                    can_open_delete_confirm && confirm_ready,
                )),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(section_label(format!("{} 条候选", candidate_ids.len())))
                .child(div().flex_1())
                .child(btn(
                    "backup-csv",
                    "备份 CSV",
                    false,
                    has_candidates,
                    cx.listener(|this, _, window, cx| {
                        this.export_candidates(ExportFormat::Csv, window, cx)
                    }),
                ))
                .child(btn(
                    "backup-json",
                    "备份 JSON",
                    false,
                    has_candidates,
                    cx.listener(|this, _, window, cx| {
                        this.export_candidates(ExportFormat::Json, window, cx)
                    }),
                ))
                .child(btn(
                    "preview-cleanup",
                    "预演删除",
                    true,
                    can_preview_delete && state.has_valid_backup(),
                    cx.listener(|this, _, window, cx| this.preview_selected(window, cx)),
                ))
                .child(danger_btn(
                    "delete-cleanup",
                    if confirm.is_some() {
                        "继续确认…"
                    } else {
                        "真实删除"
                    },
                    can_open_delete_confirm,
                    cx.listener(|this, _, window, cx| this.delete_previewed(window, cx)),
                ))
                .child(btn(
                    "clear-cleanup",
                    "清空",
                    false,
                    has_candidates,
                    cx.listener(|this, _, _window, cx| this.clear_cleanup_candidates(cx)),
                )),
        )
        .when_some(state.preview_outcome.as_ref(), |el, outcome| {
            let sample_deletable = outcome
                .deletable
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let sample_missing = outcome
                .missing
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let sample_failed = outcome
                .failed
                .iter()
                .take(3)
                .map(|(id, err)| format!("{id}: {err}"))
                .collect::<Vec<_>>()
                .join("; ");
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3()
                    .py_3()
                    .rounded_md()
                    .bg(theme::c(theme::BG_ELEVATED))
                    .border_1()
                    .border_color(theme::c(theme::BORDER))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(outcome.summary_line()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::c(theme::SUCCESS))
                            .child(format!(
                                "可删除 {} 条{}",
                                outcome.deletable.len(),
                                if sample_deletable.is_empty() {
                                    String::new()
                                } else {
                                    format!("：{sample_deletable}")
                                }
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::c(theme::WARNING))
                            .child(format!(
                                "不在当前库 {} 条{}",
                                outcome.missing.len(),
                                if sample_missing.is_empty() {
                                    String::new()
                                } else {
                                    format!("：{sample_missing}")
                                }
                            )),
                    )
                    .when(!outcome.failed.is_empty(), |box_el| {
                        box_el.child(
                            div()
                                .text_xs()
                                .text_color(theme::c(theme::DANGER))
                                .child(format!(
                                    "失败 {} 条：{sample_failed}",
                                    outcome.failed.len()
                                )),
                        )
                    }),
            )
        })
        .when_some(state.last_delete_outcome.as_ref(), |el, outcome| {
            let sample_ok = outcome
                .succeeded
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let sample_fail = outcome
                .failed
                .iter()
                .take(5)
                .map(|(id, err)| format!("{id}: {err}"))
                .collect::<Vec<_>>()
                .join("; ");
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3()
                    .py_3()
                    .rounded_md()
                    .bg(theme::c(theme::BG_ELEVATED))
                    .border_1()
                    .border_color(if outcome.failed.is_empty() {
                        theme::c(theme::SUCCESS)
                    } else {
                        theme::c(theme::DANGER)
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(outcome.summary_line()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::c(theme::SUCCESS))
                            .child(format!(
                                "成功 {} 条{}",
                                outcome.succeeded.len(),
                                if sample_ok.is_empty() {
                                    String::new()
                                } else {
                                    format!("：{sample_ok}")
                                }
                            )),
                    )
                    .when(!outcome.failed.is_empty(), |box_el| {
                        box_el.child(
                            div()
                                .text_xs()
                                .text_color(theme::c(theme::DANGER))
                                .child(format!(
                                    "仍留在候选中的失败项 {} 条：{sample_fail}",
                                    outcome.failed.len()
                                )),
                        )
                    }),
            )
        })
        .when(confirm.is_some(), |el| {
            el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .px_3()
                    .py_3()
                    .rounded_md()
                    .bg(theme::c(theme::BG_ELEVATED))
                    .border_1()
                    .border_color(theme::c(theme::DANGER))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme::c(theme::DANGER))
                            .child(format!(
                                "二次确认：将永久删除 {expected_count} 条候选，操作不可恢复"
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::c(theme::TEXT_MUTED))
                            .child("请选择确认方式之一（等同于输入数量或 DELETE）："),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(toggle_chip(
                                "delete-confirm-count",
                                format!("确认数量 {expected_count}"),
                                selected_token == Some(DeleteConfirmToken::Count),
                                cx.listener(|this, _, _window, cx| {
                                    this.select_delete_confirm_token(
                                        DeleteConfirmToken::Count,
                                        cx,
                                    )
                                }),
                            ))
                            .child(toggle_chip(
                                "delete-confirm-delete",
                                "DELETE",
                                selected_token == Some(DeleteConfirmToken::DeleteWord),
                                cx.listener(|this, _, _window, cx| {
                                    this.select_delete_confirm_token(
                                        DeleteConfirmToken::DeleteWord,
                                        cx,
                                    )
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(danger_btn(
                                "delete-confirm-execute",
                                "确认并删除",
                                confirm_ready && !state.loading,
                                cx.listener(|this, _, window, cx| {
                                    this.confirm_and_delete(window, cx)
                                }),
                            ))
                            .child(btn(
                                "delete-confirm-cancel",
                                "取消",
                                false,
                                !state.loading,
                                cx.listener(|this, _, _window, cx| {
                                    this.cancel_delete_confirm(cx)
                                }),
                            )),
                    ),
            )
        })
        .child(if candidate_ids.is_empty() {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .flex_1()
                .gap_2()
                .child(section_label("还没有候选推文"))
                .child(btn(
                    "cleanup-to-library",
                    "返回内容库选择",
                    true,
                    true,
                    cx.listener(|this, _, _window, cx| this.set_route(Route::Library, cx)),
                ))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(rows)
                .into_any_element()
        })
}

fn step_card(number: &str, label: &str, done: bool) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(if done {
            theme::c(theme::CHIP_ACTIVE)
        } else {
            theme::c(theme::BG_ELEVATED)
        })
        .border_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .child(number.to_string()),
        )
        .child(div().text_sm().child(label.to_string()))
        .child(
            div()
                .text_xs()
                .text_color(if done {
                    theme::c(theme::SUCCESS)
                } else {
                    theme::c(theme::TEXT_DIM)
                })
                .child(if done { "完成" } else { "待处理" }),
        )
}

pub fn render_root(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let page = match state.active_route {
        Route::Library => render_library(state, cx),
        Route::Insights => render_insights(state, cx),
        Route::Cleanup => render_cleanup(state, cx),
    };
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(theme::c(theme::BG))
        .text_color(theme::c(theme::TEXT))
        .children(error_banner(state, cx))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h(px(0.))
                .child(navigation_sidebar(state, cx))
                .child(page),
        )
        .child(status_bar::render_status_bar(state))
}
