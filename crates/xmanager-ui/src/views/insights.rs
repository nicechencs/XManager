//! Insights page: KPI cards, view histogram, Top-N ranking, and rank presets.

use crate::app::{AppState, ExportFormat, Route};
use crate::theme;
use crate::widgets::{btn, kind_color, page_heading, section_label, truncate_text};
use gpui::{div, prelude::*, px, Context, Div, SharedString};
use xmanager_core::{SortField, SortOrder, TimeRange, Tweet};

use super::stats_panel;

const COL_RANK: f32 = 36.0;
const COL_KIND: f32 = 36.0;
const COL_VIEWS: f32 = 64.0;
const COL_METRIC: f32 = 80.0;

pub fn render_insights(state: &AppState, cx: &mut Context<AppState>) -> impl gpui::IntoElement {
    let can_export = !state.loading && !state.filtered.is_empty();
    div()
        .id("insights-page")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scroll()
        .p_4()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(page_heading(
                    "数据洞察",
                    "统计范围 = 内容库当前筛选。点击直方图、排名或预设会立刻改筛选并回到内容库。",
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
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
        .child(stats_panel::render_stats_panel(state, cx))
        .child(render_rank_list(state, cx))
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

fn render_rank_list(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let top_n = state.applied_filter.top_n.unwrap_or(20);
    let sort = state.applied_filter.sort;
    let metric_label = rank_metric_label(sort);

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section_label(format!("排名 · 当前筛选 · Top {top_n}")))
        .child(if state.filtered.is_empty() {
            div()
                .text_sm()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child("当前筛选没有可排名的推文")
        } else {
            div()
                .flex()
                .flex_col()
                .bg(theme::c(theme::BG_PANEL))
                .border_1()
                .border_color(theme::c(theme::BORDER))
                .rounded_md()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(px(32.))
                        .px_2()
                        .bg(theme::c(theme::BG_ELEVATED))
                        .border_b_1()
                        .border_color(theme::c(theme::BORDER))
                        .child(header_cell("#", COL_RANK))
                        .child(header_cell("类型", COL_KIND))
                        .child(header_cell("曝光", COL_VIEWS))
                        .child(header_cell(metric_label, COL_METRIC))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .px_1()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child("内容"),
                        ),
                )
                .children(
                    state
                        .filtered
                        .iter()
                        .take(top_n)
                        .enumerate()
                        .map(|(idx, tweet)| {
                            let id = tweet.id.clone();
                            let kind = tweet.kind();
                            let views = tweet.views();
                            let metric = rank_metric_value(tweet, sort);
                            let text_preview = truncate_text(&tweet.text.replace('\n', " "), 64);
                            let row_bg = if idx % 2 == 0 {
                                theme::c(theme::BG_ROW)
                            } else {
                                theme::c(theme::BG_ROW_ALT)
                            };
                            div()
                                .id(SharedString::from(format!("insight-rank-{id}")))
                                .flex()
                                .flex_row()
                                .items_center()
                                .h(px(36.))
                                .px_2()
                                .bg(row_bg)
                                .border_b_1()
                                .border_color(theme::c(theme::BORDER))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.focus_tweet(&id, cx);
                                    this.active_route = Route::Library;
                                    cx.notify();
                                }))
                                .child(rank_cell(format!("{}", idx + 1), COL_RANK, true))
                                .child(
                                    div()
                                        .w(px(COL_KIND))
                                        .flex_none()
                                        .px_1()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(kind_color(kind))
                                        .child(kind.short()),
                                )
                                .child(rank_cell(format!("{views}"), COL_VIEWS, false))
                                .child(rank_cell(metric, COL_METRIC, false))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .px_1()
                                        .text_xs()
                                        .text_color(theme::c(theme::TEXT))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(text_preview),
                                )
                        }),
                )
        })
}

fn rank_metric_label(sort: SortField) -> &'static str {
    match sort {
        SortField::Views => "点赞率",
        other => other.label_zh(),
    }
}

fn rank_metric_value(tweet: &Tweet, sort: SortField) -> String {
    match sort {
        SortField::Views | SortField::LikeRate => Tweet::format_rate(tweet.like_rate()),
        SortField::BookmarkRate => Tweet::format_rate(tweet.bookmark_rate()),
        SortField::EngagementRate => Tweet::format_rate(tweet.engagement_rate()),
        SortField::RetweetRate => Tweet::format_rate(tweet.retweet_rate()),
        SortField::ReplyRate => Tweet::format_rate(tweet.reply_rate()),
        SortField::Engagement => format!("{}", tweet.engagement()),
        SortField::Date => tweet.display_date(),
    }
}

fn header_cell(label: &str, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .px_1()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::c(theme::TEXT_MUTED))
        .child(label.to_string())
}

fn rank_cell(text: impl Into<SharedString>, width: f32, muted: bool) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .px_1()
        .text_xs()
        .text_color(if muted {
            theme::c(theme::TEXT_MUTED)
        } else {
            theme::c(theme::TEXT)
        })
        .overflow_hidden()
        .whitespace_nowrap()
        .child(text.into())
}
