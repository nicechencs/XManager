//! Insights page: all-tweets distribution, current-slice contrast, and presets.

use super::stats_panel;
use super::toolbar::active_filter_summary;
use crate::app::{AppState, ExportFormat, Route};
use crate::theme::{self, space};
use crate::widgets::{
    btn, empty_mark, kind_color, page_heading, section_label, surface_card, toggle_chip,
    truncate_text,
};
use gpui::{div, prelude::*, px, Context, SharedString};
use xmanager_core::{SortField, SortOrder, TimeRange};

pub fn render_insights(state: &AppState, cx: &mut Context<AppState>) -> impl gpui::IntoElement {
    let can_export = !state.loading && !state.filtered.is_empty();
    div()
        .id("insights-page")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scroll()
        .p(px(space::LG))
        .gap(px(space::MD))
        .bg(theme::c(theme::BG))
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
                    "看全部已同步内容的形状，当前内容库筛选是高亮切片。",
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
        .child(render_scope_bar(state, cx))
        .child(if state.all_tweets.is_empty() {
            empty_insights().into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(space::MD))
                .child(stats_panel::render_scope_cards(state))
                .child(stats_panel::render_histogram(state, cx))
                .child(render_presets(state, cx))
                .child(render_samples(state, cx))
                .into_any_element()
        })
}

fn empty_insights() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(space::MD))
        .py(px(space::XL))
        .child(empty_mark("▦"))
        .child(
            theme::type_title(div().text_color(theme::c(theme::TEXT))).child("还没有可分析的数据"),
        )
        .child(
            theme::type_body(div().text_color(theme::c(theme::TEXT_MUTED)))
                .child("先到内容库拉取推文，这里会显示全部数据的曝光分布。"),
        )
}

fn render_scope_bar(state: &AppState, cx: &mut Context<AppState>) -> gpui::Div {
    surface_card(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(space::SM))
            .px(px(space::MD))
            .py(px(space::SM)),
    )
    .child(
        theme::type_body(div().text_color(theme::c(theme::TEXT))).child(format!(
            "当前切片 {} / {} 条 · {}",
            state.summary.count,
            state.all_summary.count,
            active_filter_summary(state)
        )),
    )
    .child(btn(
        "insights-open-library",
        "在内容库查看",
        true,
        true,
        cx.listener(|this, _, _window, cx| this.set_route(Route::Library, cx)),
    ))
}

fn render_presets(state: &AppState, cx: &mut Context<AppState>) -> gpui::Div {
    let enabled = !state.loading;
    div()
        .flex()
        .flex_col()
        .gap(px(space::SM))
        .child(section_label("常用切片（应用到内容库）"))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(space::SM))
                .child(preset_chip(
                    "insight-low-10",
                    "≤10 曝光",
                    enabled,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(10, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(preset_chip(
                    "insight-low-20",
                    "≤20 曝光",
                    enabled,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(20, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(preset_chip(
                    "insight-low-50",
                    "≤50 曝光待清理",
                    enabled,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(50, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(preset_chip(
                    "insight-low-100",
                    "≤100 曝光",
                    enabled,
                    cx.listener(|this, _, _window, cx| {
                        this.apply_low_exposure_preset(100, cx);
                        this.active_route = Route::Library;
                        cx.notify();
                    }),
                ))
                .child(preset_chip(
                    "insight-low-views",
                    "30天 · 曝光最低",
                    enabled,
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
                .child(preset_chip(
                    "insight-like-high",
                    "7天 · 点赞率最高",
                    enabled,
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
                .child(preset_chip(
                    "insight-bookmark",
                    "30天 · 收藏率最高",
                    enabled,
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
                )),
        )
}

fn preset_chip(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl gpui::IntoElement {
    toggle_chip(id, label, false, move |ev, window, cx| {
        if enabled {
            on_click(ev, window, cx);
        }
    })
}

fn render_samples(state: &AppState, cx: &mut Context<AppState>) -> gpui::Div {
    let samples = state.lowest_view_samples(5);
    surface_card(div().flex().flex_col())
        .child(
            div()
                .px(px(space::MD))
                .py(px(space::SM))
                .border_b_1()
                .border_color(theme::c(theme::BORDER))
                .child(section_label("低曝光样本 · 点一行去内容库")),
        )
        .children(samples.into_iter().enumerate().map(|(idx, tweet)| {
            let id = tweet.id.clone();
            let kind = tweet.kind();
            let preview = truncate_text(&tweet.text.replace('\n', " "), 72);
            let views = tweet.views();
            let row_bg = if idx % 2 == 0 {
                theme::c(theme::BG_ROW)
            } else {
                theme::c(theme::BG_ROW_ALT)
            };
            div()
                .id(SharedString::from(format!("insight-sample-{id}")))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::SM))
                .h(px(40.))
                .px(px(space::MD))
                .bg(row_bg)
                .cursor_pointer()
                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.open_tweet_in_library(&id, cx);
                }))
                .child(
                    theme::type_meta(
                        div()
                            .w(px(24.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(kind_color(kind)),
                    )
                    .child(kind.short()),
                )
                .child(
                    theme::type_body(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_color(theme::c(theme::TEXT))
                            .overflow_hidden()
                            .whitespace_nowrap(),
                    )
                    .child(preview),
                )
                .child(
                    theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED)))
                        .child(format!("曝光 {views}")),
                )
        }))
}
