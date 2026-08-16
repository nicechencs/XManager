//! Application shell and the three product routes.

use crate::app::{
    cleanup_next_hint, resolve_focused_tweet, tweet_preview_label, AppState, DeleteConfirmToken,
    ExportFormat, Route,
};
use crate::theme;
use crate::views::{status_bar, toolbar, tweet_list};
use crate::widgets::{
    btn, count_badge, danger_btn, kind_badge, metric_tile, nav_destination, page_heading,
    section_label, stepper, toggle_chip,
};
use gpui::{
    div, prelude::*, px, Context, CursorStyle, Div, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Stateful,
};
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
            this.apply_filters(cx);
        }),
    )
}

fn filter_drawer(state: &AppState, cx: &mut Context<AppState>) -> Stateful<Div> {
    let d = &state.filter_draft;
    let overlay = state.layout_mode.filter_as_overlay();
    div()
        .id("filter-drawer")
        .flex()
        .flex_col()
        .when(overlay, |el| {
            el.flex_1().w_full().min_w(px(0.)).min_h(px(0.))
        })
        .when(!overlay, |el| el.w(px(300.)).flex_none())
        .h_full()
        .gap_2()
        .p_3()
        .overflow_y_scroll()
        .bg(theme::c(theme::BG_PANEL))
        .when(!overlay, |el| {
            el.border_r_1().border_color(theme::c(theme::BORDER))
        })
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .child(section_label("筛选与排序"))
                .when(overlay, |el| {
                    el.child(btn(
                        "filter-drawer-close",
                        "关闭",
                        false,
                        true,
                        cx.listener(|this, _, _window, cx| {
                            this.filter_drawer_open = false;
                            cx.notify();
                        }),
                    ))
                }),
        )
        .child(
            theme::type_caption(div().text_color(theme::c(theme::TEXT_MUTED)))
                .child("改条件后列表立即更新。拉取数量只影响下次「拉取并分析」。"),
        )
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
                        this.apply_filters(cx);
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
                            this.apply_filters(cx);
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
                            this.apply_filters(cx);
                        })
                        .ok();
                },
            )
        })
        .child(section_label("互动上限"))
        .child({
            let entity = cx.weak_entity();
            stepper(
                "filter-max-engagement",
                d.max_engagement.unwrap_or(0),
                5,
                0,
                1_000_000,
                move |delta, _, app| {
                    entity
                        .update(app, |this, cx| {
                            this.filter_draft.max_engagement =
                                clamp_opt(this.filter_draft.max_engagement, delta, 0, 1_000_000);
                            this.apply_filters(cx);
                        })
                        .ok();
                },
            )
        })
        .child(section_label("早于天数"))
        .child({
            let entity = cx.weak_entity();
            stepper(
                "filter-older-than-days",
                d.older_than_days.unwrap_or(0),
                1,
                0,
                3600,
                move |delta, _, app| {
                    entity
                        .update(app, |this, cx| {
                            this.filter_draft.older_than_days =
                                clamp_opt(this.filter_draft.older_than_days, delta, 0, 3600);
                            this.apply_filters(cx);
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
                            this.apply_filters(cx);
                        }),
                    )
                }),
            ),
        )
        .child(section_label("升降序"))
        .child(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .child(toggle_chip(
                    "order-asc",
                    "最低优先",
                    d.order == SortOrder::Asc,
                    cx.listener(|this, _, _window, cx| {
                        this.filter_draft.order = SortOrder::Asc;
                        this.apply_filters(cx);
                    }),
                ))
                .child(toggle_chip(
                    "order-desc",
                    "最高优先",
                    d.order == SortOrder::Desc,
                    cx.listener(|this, _, _window, cx| {
                        this.filter_draft.order = SortOrder::Desc;
                        this.apply_filters(cx);
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
                                this.apply_filters(cx);
                            }),
                        )
                    }),
            ),
        )
        .child(section_label("低曝光快捷"))
        .child(div().flex().flex_row().flex_wrap().gap_1().children(
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
        ))
        .child(btn(
            "apply-filters",
            "完成",
            true,
            true,
            cx.listener(|this, _, _window, cx| {
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
                this.filter_draft = crate::app::FilterDraft::unrestricted();
                this.apply_filters(cx);
                this.filter_drawer_open = false;
                cx.notify();
            }),
        ))
        .child(section_label("下次拉取（不改当前列表）"))
        .child(
            theme::type_caption(div().text_color(theme::c(theme::TEXT_DIM)))
                .child("改完后请到内容库点「拉取并分析」。"),
        )
        .child(div().flex().flex_row().flex_wrap().gap_1().children(
            [50usize, 100, 200, 500, 1000].into_iter().map(|n| {
                toggle_chip(
                    format!("fetch-limit-{n}"),
                    format!("{n} 条"),
                    state.fetch_limit == n,
                    cx.listener(move |this, _, _window, cx| {
                        this.fetch_limit = n;
                        cx.notify();
                    }),
                )
            }),
        ))
        .child(toggle_chip(
            "include-retweets",
            "拉取含转发",
            d.include_retweets,
            cx.listener(|this, _, _window, cx| {
                this.filter_draft.include_retweets = !this.filter_draft.include_retweets;
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

fn inspector(state: &AppState, cx: &mut Context<AppState>) -> Stateful<Div> {
    let overlay = state.layout_mode.inspector_as_overlay();
    let focused = state
        .focused_tweet_id
        .as_ref()
        .and_then(|id| resolve_focused_tweet(id, &state.filtered, &state.cleanup_snapshot));
    let body = focused.map(|(tweet, snapshot_only)| {
        let metrics = &tweet.public_metrics;
        let id = tweet.id.clone();
        let candidate = state.cleanup_candidates.contains(&id);
        let kind = tweet.kind();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(kind_badge(kind))
                    .when(candidate, |el| {
                        el.child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(theme::c(theme::CHIP_ACTIVE))
                                .text_xs()
                                .text_color(theme::c(theme::TEXT))
                                .child("已在安全清理"),
                        )
                    }),
            )
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
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(metric_tile("曝光", tweet.views().to_string()))
                    .child(metric_tile("点赞", metrics.like_count.to_string()))
                    .child(metric_tile("收藏", metrics.bookmark_count.to_string()))
                    .child(metric_tile("转发", metrics.retweet_count.to_string()))
                    .child(metric_tile("回复", metrics.reply_count.to_string()))
                    .child(metric_tile("互动量", tweet.engagement().to_string()))
                    .child(metric_tile(
                        "点赞率",
                        Tweet::format_rate(tweet.like_rate()),
                    ))
                    .child(metric_tile(
                        "互动率",
                        Tweet::format_rate(tweet.engagement_rate()),
                    )),
            )
            .child(context_metric("发布时间", tweet.display_date()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .py_1()
                    .border_b_1()
                    .border_color(theme::c(theme::BORDER))
                    .child(
                        theme::type_caption(div().text_color(theme::c(theme::TEXT_MUTED)))
                            .child("推文 ID"),
                    )
                    .child(
                        theme::type_mono(div().text_color(theme::c(theme::TEXT))).child(id.clone()),
                    ),
            )
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
    div()
        .id("tweet-inspector")
        .flex()
        .flex_col()
        .when(overlay, |el| {
            el.flex_1().w_full().min_w(px(0.)).min_h(px(0.))
        })
        .when(!overlay, |el| {
            el.w(px(state.effective_inspector_width())).flex_none()
        })
        .h_full()
        .gap_3()
        .p_4()
        .overflow_y_scroll()
        .bg(theme::c(theme::BG_PANEL))
        .when(!overlay, |el| {
            el.border_l_1().border_color(theme::c(theme::BORDER))
        })
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
                            "推文检查"
                        } else {
                            "检查器"
                        }),
                )
                .when(overlay, |el| {
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
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(metric_tile("当前结果", state.filtered.len().to_string()))
                        .child(metric_tile(
                            "安全清理候选",
                            state.cleanup_candidates.len().to_string(),
                        )),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::c(theme::TEXT_DIM))
                        .child("J / K 上下条 · 空格勾选 · / 打开筛选 · 1 内容库 · 2 洞察 · 3 清理 · Esc 关闭"),
                )
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
    let compact = !show_labels;
    let nav_w = state.layout_mode.nav_width();
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
            theme::type_display(div().text_color(theme::c(theme::TEXT)))
                .child(if show_labels { "XManager" } else { "XM" }),
        )
        .when(show_labels, |el| el.child(section_label("工作台")))
        .child(nav_destination(
            "nav-library",
            if compact { "内容" } else { "内容库" },
            compact,
            state.active_route == Route::Library,
            None,
            cx.listener(|this, _, _window, cx| this.set_route(Route::Library, cx)),
        ))
        .child(nav_destination(
            "nav-insights",
            if compact { "洞察" } else { "数据洞察" },
            compact,
            state.active_route == Route::Insights,
            None,
            cx.listener(|this, _, _window, cx| this.set_route(Route::Insights, cx)),
        ))
        .child(nav_destination(
            "nav-cleanup",
            if compact { "清理" } else { "安全清理" },
            compact,
            state.active_route == Route::Cleanup,
            Some(state.cleanup_candidates.len()),
            cx.listener(|this, _, _window, cx| this.set_route(Route::Cleanup, cx)),
        ))
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(if show_labels {
                    format!("{} 条数据", state.all_tweets.len())
                } else {
                    format!("{} 条", state.all_tweets.len())
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
                    None => "未同步".into(),
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
            if show_labels { "刷新状态" } else { "刷新" },
            false,
            !state.loading,
            cx.listener(|this, _, _window, cx| this.refresh_whoami(cx)),
        ))
        .when(show_labels, |el| {
            el.child(
                theme::type_caption(div().text_color(theme::c(theme::TEXT_DIM)))
                    .child("J/K 上下 · 空格勾选 · / 筛选 · 1–3 切页 · Esc 关闭"),
            )
        })
        .child(btn(
            "sidebar-theme",
            if show_labels {
                format!(
                    "{} → {}",
                    state.theme_mode.label_zh(),
                    state.theme_mode.opposite_label_zh()
                )
            } else {
                state.theme_mode.opposite_label_zh().to_string()
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

fn render_library(state: &AppState, cx: &mut Context<AppState>) -> Stateful<Div> {
    let layout = state.layout_mode;
    let focused = state.focused_tweet_id.is_some();
    let inspector_overlay = layout.inspector_as_overlay() && focused;
    // Narrow: replace the list with a full-height overlay instead of sibling columns.
    if layout.filter_as_overlay() && state.filter_drawer_open {
        return div()
            .id("library-filter-overlay")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .min_w(px(0.))
            .child(filter_drawer(state, cx));
    }
    // Medium/Narrow: focused inspector replaces the list (never also a side column).
    if inspector_overlay {
        return div()
            .id("library-inspector-overlay")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .min_w(px(0.))
            .child(inspector(state, cx));
    }

    let show_split = !inspector_overlay && layout.library_shows_inspector(focused);
    let mut content = div()
        .id("library-split-row")
        .flex()
        .flex_row()
        .flex_1()
        .min_h(px(0.))
        .min_w(px(0.))
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
            if this.split_drag.is_some() {
                this.update_split_drag(f32::from(event.position.x), cx);
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _window, cx| this.end_split_drag(cx)),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _: &MouseUpEvent, _window, cx| this.end_split_drag(cx)),
        );
    if state.filter_drawer_open {
        content = content.child(filter_drawer(state, cx));
    }
    content = content.child(
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(280.))
            .min_h(px(0.))
            .child(toolbar::render_toolbar(state, cx))
            .children(toolbar::render_cleanup_notice(state, cx))
            .children(loading_banner(state))
            .child(tweet_list::render_tweet_list(state, cx))
            .child(toolbar::render_bulk_bar(state, cx)),
    );
    if show_split {
        content = content
            .child(list_resize_handle(state, cx))
            .child(inspector(state, cx));
    }
    content
}

fn list_resize_handle(state: &AppState, cx: &mut Context<AppState>) -> impl gpui::IntoElement {
    let dragging = state.split_drag.is_some();
    div()
        .id("library-list-resize")
        .flex_none()
        .w(px(6.))
        .h_full()
        .cursor(CursorStyle::ResizeLeftRight)
        .bg(theme::c(if dragging {
            theme::ACCENT
        } else {
            theme::BORDER
        }))
        .hover(|s| s.bg(theme::c(theme::ACCENT)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                if event.click_count >= 2 {
                    this.reset_split_width(cx);
                } else {
                    this.begin_split_drag(f32::from(event.position.x), cx);
                }
            }),
        )
}

fn render_cleanup(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let candidate_ids: Vec<String> = state.cleanup_candidates.iter().cloned().collect();
    let rows = candidate_ids.iter().map(|id| {
        let tweet = state.cleanup_snapshot.get(id);
        let id_for_focus = id.clone();
        let id_for_remove = id.clone();
        let preview = tweet
            .map(|t| crate::widgets::truncate_text(&t.text.replace('\n', " "), 90))
            .unwrap_or_else(|| id.clone());
        let meta = tweet
            .map(|t| {
                format!(
                    "{} · 曝光 {} · {}",
                    t.kind().label_zh(),
                    t.views(),
                    t.display_date()
                )
            })
            .unwrap_or_else(|| format!("ID {id}"));
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(theme::c(theme::BG_ROW))
            .border_b_1()
            .border_color(theme::c(theme::BORDER))
            .child(tweet.map(|t| kind_badge(t.kind())).unwrap_or_else(|| {
                div()
                    .text_xs()
                    .text_color(theme::c(theme::TEXT_MUTED))
                    .child("快照")
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(160.))
                    .gap_1()
                    .child(div().text_sm().child(preview))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::c(theme::TEXT_MUTED))
                            .child(meta),
                    ),
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
    let can_preview_delete = has_candidates;
    let can_open_delete_confirm = has_candidates;
    let confirm = state.delete_confirm.as_ref();
    let confirm_ready = state.delete_confirm_is_ready();
    let expected_count = confirm
        .map(|c| c.expected_count)
        .unwrap_or(candidate_ids.len());
    let selected_token = confirm.and_then(|c| c.selected);
    let typed_confirm = confirm.map(|c| c.typed.as_str()).unwrap_or("").to_string();
    let typed_empty = typed_confirm.is_empty();
    let next_hint = cleanup_next_hint(
        candidate_ids.len(),
        state.credentials_ok,
        state.has_valid_backup(),
        state.has_valid_preview(),
        confirm,
    );
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .p_4()
        .gap_3()
        .child(page_heading("安全清理", next_hint))
        .child({
            let stacked = state.layout_mode.stack_cleanup_steps();
            let steps = if stacked {
                div().flex().flex_col().gap_2()
            } else {
                div().flex().flex_row().flex_wrap().gap_2()
            };
            steps
                .child(step_card(
                    "1",
                    "加入候选",
                    !candidate_ids.is_empty(),
                    stacked,
                ))
                .child(step_card(
                    "2",
                    "自动备份与预演",
                    state.has_valid_backup() && state.has_valid_preview(),
                    stacked,
                ))
                .child(step_card(
                    "3",
                    "二次确认",
                    confirm.is_some(),
                    stacked,
                ))
                .child(step_card(
                    "4",
                    "确认并删除",
                    can_open_delete_confirm && confirm_ready,
                    stacked,
                ))
        })
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(section_label(format!("{} 条候选", candidate_ids.len())))
                .child(count_badge(candidate_ids.len()))
                .child(div().flex_1())
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
                    if state.clear_cleanup_armed {
                        "确认清空"
                    } else {
                        "清空候选"
                    },
                    state.clear_cleanup_armed,
                    has_candidates,
                    cx.listener(|this, _, _window, cx| this.request_clear_cleanup(cx)),
                )),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(section_label("可选"))
                .child(
                    theme::type_caption(div().text_color(theme::c(theme::TEXT_DIM)))
                        .child("主按钮已包含备份和预演，以下仅在需要单独导出或复查时使用。"),
                )
                .child(btn(
                    "backup-csv",
                    "另存 CSV",
                    false,
                    has_candidates,
                    cx.listener(|this, _, window, cx| {
                        this.export_candidates(ExportFormat::Csv, window, cx)
                    }),
                ))
                .child(btn(
                    "backup-json",
                    "另存 JSON",
                    false,
                    has_candidates,
                    cx.listener(|this, _, window, cx| {
                        this.export_candidates(ExportFormat::Json, window, cx)
                    }),
                ))
                .child(btn(
                    "preview-cleanup",
                    "只预演不删除",
                    false,
                    can_preview_delete,
                    cx.listener(|this, _, window, cx| this.preview_selected(window, cx)),
                )),
        )
        .when_some(state.preview_outcome.as_ref(), |el, outcome| {
            let sample_deletable = outcome
                .deletable
                .iter()
                .take(6)
                .map(|id| tweet_preview_label(id, &state.cleanup_snapshot, &state.all_tweets))
                .collect::<Vec<_>>()
                .join("；");
            let sample_missing = outcome
                .missing
                .iter()
                .take(6)
                .map(|id| tweet_preview_label(id, &state.cleanup_snapshot, &state.all_tweets))
                .collect::<Vec<_>>()
                .join("；");
            let sample_failed = outcome
                .failed
                .iter()
                .take(3)
                .map(|(id, err)| {
                    format!(
                        "{}：{err}",
                        tweet_preview_label(id, &state.cleanup_snapshot, &state.all_tweets)
                    )
                })
                .collect::<Vec<_>>()
                .join("；");
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
                .take(6)
                .map(|id| tweet_preview_label(id, &state.cleanup_snapshot, &state.all_tweets))
                .collect::<Vec<_>>()
                .join("；");
            let sample_fail = outcome
                .failed
                .iter()
                .take(5)
                .map(|(id, err)| {
                    format!(
                        "{}：{err}",
                        tweet_preview_label(id, &state.cleanup_snapshot, &state.all_tweets)
                    )
                })
                .collect::<Vec<_>>()
                .join("；");
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
                        box_el.child(div().text_xs().text_color(theme::c(theme::DANGER)).child(
                            format!(
                                "仍留在候选中的失败项 {} 条：{sample_fail}",
                                outcome.failed.len()
                            ),
                        ))
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
                            .child("可点 chip，或输入确切数量 / DELETE"),
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
                                    this.select_delete_confirm_token(DeleteConfirmToken::Count, cx)
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
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("delete-confirm-typed")
                                    .flex()
                                    .flex_1()
                                    .items_center()
                                    .min_h(px(30.))
                                    .px_2()
                                    .rounded_md()
                                    .bg(theme::c(theme::BG))
                                    .border_1()
                                    .border_color(theme::c(theme::BORDER_STRONG))
                                    .tab_index(0)
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, _window, cx| {
                                            let key = event.keystroke.key.as_str();
                                            if key == "backspace" || key == "delete" {
                                                this.backspace_delete_confirm(cx);
                                                cx.stop_propagation();
                                            } else if let Some(ch) =
                                                event.keystroke.key_char.as_deref()
                                            {
                                                if ch.chars().any(|c| !c.is_control()) {
                                                    this.push_delete_confirm_text(ch, cx);
                                                    cx.stop_propagation();
                                                }
                                            }
                                        },
                                    ))
                                    .child(
                                        theme::type_mono(
                                            div().text_color(theme::c(if typed_empty {
                                                theme::TEXT_MUTED
                                            } else {
                                                theme::TEXT
                                            })),
                                        )
                                        .child(if typed_empty {
                                            "输入数量或 DELETE".to_string()
                                        } else {
                                            typed_confirm.clone()
                                        }),
                                    ),
                            )
                            .child(btn(
                                "delete-confirm-backspace",
                                "退格",
                                false,
                                !state.loading && !typed_empty,
                                cx.listener(|this, _, _window, cx| {
                                    this.backspace_delete_confirm(cx)
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
                                !state.loading,
                                cx.listener(|this, _, window, cx| {
                                    this.confirm_and_delete(window, cx)
                                }),
                            ))
                            .child(btn(
                                "delete-confirm-cancel",
                                "取消",
                                false,
                                !state.loading,
                                cx.listener(|this, _, _window, cx| this.cancel_delete_confirm(cx)),
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
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::c(theme::TEXT_MUTED))
                        .child("在内容库勾选或打开一条，再加入安全清理。"),
                )
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
                .id("cleanup-candidate-list")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .gap_1()
                .children(rows)
                .into_any_element()
        })
}

fn step_card(number: &str, label: &str, done: bool, stacked: bool) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .when(stacked, |el| el.w_full())
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

pub fn render_root(state: &AppState, cx: &mut Context<AppState>) -> impl gpui::IntoElement {
    let page = match state.active_route {
        Route::Library => render_library(state, cx).into_any_element(),
        Route::Insights => crate::views::render_insights(state, cx).into_any_element(),
        Route::Cleanup => render_cleanup(state, cx).into_any_element(),
    };
    theme::apply_root_type(
        div()
            .id("xmanager-root")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::c(theme::BG))
            .text_color(theme::c(theme::TEXT))
            .tab_index(0),
    )
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
            if this.handle_workspace_key(event.keystroke.key.as_str(), cx) {
                cx.stop_propagation();
            }
        }))
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
