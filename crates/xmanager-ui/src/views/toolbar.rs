//! Library page toolbar and the transient selection bar.

use crate::app::{chip_label, AppState, AppliedFilterChip, ExportFormat};
use crate::theme;
use crate::widgets::{btn, removable_chip, toggle_chip};
use gpui::{div, prelude::*, px, Context, Div};

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
    let dirty = if state.filter_draft_is_dirty() {
        " · 有未应用修改"
    } else {
        ""
    };
    format!(
        "{} · {} · {} · {} 类 · {} 条结果{}",
        d.time_range.label_zh(),
        views,
        d.sort.label_zh(),
        kind_count,
        state.filtered.len(),
        dirty
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
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(90.))
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(theme::c(theme::BG_ELEVATED))
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::c(theme::TEXT))
                .child(value.into()),
        )
}

pub fn render_toolbar(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let can_fetch = !state.loading;
    let s = &state.summary;
    div()
        .flex()
        .flex_col()
        .gap_2()
        .px_4()
        .py_3()
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
        .child(div().flex().flex_row().flex_wrap().items_center().gap_2().children([
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
            toggle_chip(
                "draft-dirty",
                if state.filter_draft_is_dirty() {
                    "未应用修改"
                } else {
                    "已生效"
                },
                state.filter_draft_is_dirty(),
                cx.listener(|this, _, _window, cx| {
                    if this.filter_draft_is_dirty() {
                        this.filter_drawer_open = true;
                        cx.notify();
                    }
                }),
            ),
        ]))
        .child(div().flex().flex_row().flex_wrap().gap_1().children({
            let enabled = !state.loading;
            state
                .applied_chips()
                .into_iter()
                .enumerate()
                .map(move |(idx, chip)| {
                    let label = chip_label(&chip);
                    applied_chip(format!("applied-chip-{idx}"), label, chip, enabled, cx)
                        .into_any_element()
                })
        }))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::c(theme::TEXT_MUTED))
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
        .gap_2()
        .min_h(px(48.))
        .px_4()
        .py_2()
        .bg(theme::c(theme::BG_PANEL))
        .border_t_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(if can_stage {
                    theme::c(theme::TEXT)
                } else {
                    theme::c(theme::TEXT_MUTED)
                })
                .child(if has_selection {
                    format!("已选 {} 条", state.selected.len())
                } else if has_focus {
                    "已查看当前推文，可加入安全清理".into()
                } else {
                    "勾选左侧方框，或点开一条后加入安全清理".into()
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
        .child(btn(
            "add-cleanup-btn",
            "加入安全清理",
            true,
            !state.loading && can_stage,
            cx.listener(|this, _, _window, cx| this.add_selected_to_cleanup(cx)),
        ))
}
