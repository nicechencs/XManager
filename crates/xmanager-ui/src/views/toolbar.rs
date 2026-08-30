//! Library page toolbar and the transient selection bar.

use crate::app::{
    chip_label, is_default_cleanup_preset, AppState, AppliedFilterChip, ExportFormat, Route,
};
use crate::theme::{self, space};
use crate::widgets::{btn, danger_btn, removable_chip, surface_card, toggle_chip};
use gpui::{div, prelude::*, px, Context, Div};

pub fn render_cleanup_notice(state: &AppState, cx: &mut Context<AppState>) -> Option<Div> {
    state.cleanup_notice.as_ref().map(|msg| {
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(space::SM))
            .px(px(space::LG))
            .py(px(space::SM))
            .bg(theme::c(theme::CHIP_ACTIVE))
            .border_b_1()
            .border_color(theme::c(theme::ACCENT))
            .child(
                theme::type_body(div().flex_1().text_color(theme::c(theme::TEXT)))
                    .child(msg.clone()),
            )
            .child(btn(
                "notice-go-cleanup",
                "去安全清理",
                true,
                true,
                cx.listener(|this, _, _window, cx| this.set_route(Route::Cleanup, cx)),
            ))
            .child(btn(
                "notice-dismiss",
                "继续挑选",
                false,
                true,
                cx.listener(|this, _, _window, cx| this.dismiss_cleanup_notice(cx)),
            ))
    })
}

pub fn active_filter_summary(state: &AppState) -> String {
    let d = &state.applied_filter;
    let views = d
        .max_views
        .map(|v| format!("曝光 ≤ {v}"))
        .unwrap_or_else(|| "曝光不限".into());
    let kind_count = [
        d.kinds.original,
        d.kinds.reply,
        d.kinds.retweet,
        d.kinds.quote,
    ]
    .into_iter()
    .filter(|active| *active)
    .count();
    format!(
        "{} · {} · {} · {} 类 · {} 条结果",
        d.time_range.label_zh(),
        views,
        d.sort.label_zh(),
        kind_count,
        state.filtered.len()
    )
}

fn applied_chip(
    id: impl Into<gpui::SharedString>,
    label: impl Into<String>,
    chip: AppliedFilterChip,
    enabled: bool,
    cx: &mut Context<AppState>,
) -> impl gpui::IntoElement {
    removable_chip(
        id,
        label.into(),
        enabled,
        cx.listener(move |this, _, _window, cx| this.remove_applied_chip(chip.clone(), cx)),
    )
}

fn kpi(label: &str, value: impl Into<String>) -> Div {
    surface_card(
        div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .min_w(px(96.))
            .px(px(space::MD))
            .py(px(space::SM)),
    )
    .child(theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED))).child(label.to_string()))
    .child(theme::type_title(div().text_color(theme::c(theme::TEXT))).child(value.into()))
}

pub fn render_toolbar(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let can_fetch = !state.loading;
    let s = &state.summary;
    div()
        .flex()
        .flex_col()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .bg(theme::c(theme::BG_PANEL))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_3()
                .child(crate::widgets::page_heading(
                    "内容库",
                    "筛选、检查并整理你的 X 内容",
                ))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(btn(
                            "library-export-csv",
                            "导出 CSV",
                            false,
                            !state.loading && !state.filtered.is_empty(),
                            cx.listener(|this, _, window, cx| {
                                this.export_tweets(ExportFormat::Csv, window, cx)
                            }),
                        ))
                        .child(btn(
                            "library-export-json",
                            "导出 JSON",
                            false,
                            !state.loading && !state.filtered.is_empty(),
                            cx.listener(|this, _, window, cx| {
                                this.export_tweets(ExportFormat::Json, window, cx)
                            }),
                        ))
                        .child(btn(
                            "library-fetch",
                            if state.loading {
                                "分析中…"
                            } else {
                                "拉取并分析"
                            },
                            true,
                            can_fetch,
                            cx.listener(|this, _, _window, cx| this.fetch_tweets(cx)),
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(space::SM))
                .children([
                    toggle_chip(
                        "filter-drawer-toggle",
                        {
                            let n = state.applied_chips().len();
                            if state.filter_drawer_open {
                                format!("收起筛选 · {n}")
                            } else {
                                format!("筛选 · {n}")
                            }
                        },
                        state.filter_drawer_open,
                        cx.listener(|this, _, _window, cx| this.toggle_filter_drawer(cx)),
                    ),
                    toggle_chip(
                        "sort-summary",
                        format!("排序：{}", state.applied_filter.sort.label_zh()),
                        false,
                        cx.listener(|this, _, _window, cx| {
                            this.filter_drawer_open = true;
                            cx.notify();
                        }),
                    ),
                ]),
        )
        .child({
            let enabled = !state.loading;
            let chips: Vec<_> = state
                .applied_chips()
                .into_iter()
                .enumerate()
                .map(|(idx, chip)| {
                    let label = chip_label(&chip);
                    applied_chip(format!("applied-chip-{idx}"), label, chip, enabled, cx)
                        .into_any_element()
                })
                .collect();
            let has_chips = !chips.is_empty();
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(space::SM))
                .children(chips)
                .when(has_chips, |el| {
                    el.child(btn(
                        "toolbar-clear-filters",
                        "清除全部",
                        false,
                        !state.loading,
                        cx.listener(|this, _, _window, cx| {
                            this.filter_draft = crate::app::FilterDraft::unrestricted();
                            this.apply_filters(cx);
                            cx.notify();
                        }),
                    ))
                })
        })
        .when(is_default_cleanup_preset(&state.applied_filter), |el| {
            el.child(
                theme::type_meta(div().text_color(theme::c(theme::TEXT_DIM)))
                    .child("默认范围：曝光≤50、不含回帖/转发。点芯片即可放宽。"),
            )
        })
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(space::SM))
                .child(
                    theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED)))
                        .child(active_filter_summary(state)),
                )
                .child(kpi("结果", s.count.to_string()))
                .child(kpi("平均曝光", format!("{:.0}", s.avg_views)))
                .child(kpi(
                    "均互率",
                    xmanager_core::Tweet::format_rate(s.avg_engagement_rate),
                )),
        )
}

pub fn render_bulk_bar(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let has_selection = !state.selected.is_empty();
    let has_focus = state.focused_tweet_id.is_some();
    let can_stage = has_selection || has_focus;
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(space::SM))
        .min_h(px(56.))
        .px(px(space::LG))
        .py(px(space::SM))
        .bg(theme::c(theme::BG_PANEL))
        .border_t_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            theme::type_label(div().text_color(if can_stage {
                theme::c(theme::TEXT)
            } else {
                theme::c(theme::TEXT_MUTED)
            }))
            .child(if has_selection {
                format!("已选 {} 条。可直接删除，或加入安全清理复核。", state.selected.len())
            } else if has_focus {
                "已查看当前推文，可删除或加入安全清理（不会离开本页）".into()
            } else {
                "勾选左侧方框，或点开一条后删除 / 加入安全清理".into()
            }),
        )
        .child(btn(
            "select-all-btn",
            "全选",
            false,
            !state.loading && !state.filtered.is_empty(),
            cx.listener(|this, _, _window, cx| this.select_all_filtered(cx)),
        ))
        .child(btn(
            "select-none-btn",
            "清除选择",
            false,
            !state.loading && has_selection,
            cx.listener(|this, _, _window, cx| this.select_none(cx)),
        ))
        .child(btn(
            "invert-selection-btn",
            "反选",
            false,
            !state.loading && !state.filtered.is_empty(),
            cx.listener(|this, _, _window, cx| this.invert_selection(cx)),
        ))
        .child(div().flex_1())
        .child(danger_btn(
            "library-delete-selected",
            "删除选中",
            !state.loading && can_stage,
            cx.listener(|this, _, _window, cx| this.request_library_delete(cx)),
        ))
        .child(btn(
            "add-cleanup-btn",
            "加入安全清理",
            false,
            !state.loading && can_stage,
            cx.listener(|this, _, _window, cx| this.add_selected_to_cleanup(cx)),
        ))
}
