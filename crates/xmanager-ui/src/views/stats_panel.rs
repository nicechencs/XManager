//! Summary stats cards and view histogram bars.

use crate::app::AppState;
use crate::theme::{self, space};
use crate::widgets::{section_label, stat_card};
use gpui::{div, prelude::*, px, Context, Div, SharedString};
use xmanager_core::Tweet;

pub fn render_stats_panel(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let s = &state.summary;
    let max_bucket = state
        .histogram
        .iter()
        .map(|(_, c)| *c)
        .max()
        .unwrap_or(0)
        .max(1);

    let range_label = state.applied_filter.time_range.label_zh();
    let sort_label = state.applied_filter.sort.label_zh();
    let order_label = match state.applied_filter.order {
        xmanager_core::SortOrder::Asc => "最低",
        xmanager_core::SortOrder::Desc => "最高",
    };

    div()
        .flex()
        .flex_col()
        .gap(px(space::MD))
        .p(px(space::MD))
        .bg(theme::c(theme::BG_PANEL))
        .border_1()
        .border_color(theme::c(theme::BORDER))
        .rounded(px(theme::radius::LG))
        .shadow_sm()
        .child(section_label(format!(
            "统计 · {range_label} · 按{sort_label}{order_label}"
        )))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(stat_card("条数", format!("{}", s.count)))
                .child(stat_card("平均曝光", format!("{:.1}", s.avg_views)))
                .child(stat_card("中位曝光", format!("{:.1}", s.median_views)))
                .child(stat_card("最低曝光", format!("{}", s.min_views)))
                .child(stat_card("最高曝光", format!("{}", s.max_views)))
                .child(stat_card("均赞率", Tweet::format_rate(s.avg_like_rate)))
                .child(stat_card("均藏率", Tweet::format_rate(s.avg_bookmark_rate)))
                .child(stat_card(
                    "均互率",
                    Tweet::format_rate(s.avg_engagement_rate),
                ))
                .child(stat_card(
                    "类型",
                    format!(
                        "原{} 回{} 转{} 引{}",
                        s.original_count, s.reply_count, s.retweet_count, s.quote_count
                    ),
                )),
        )
        .child(section_label("曝光分布（点击区间筛选）"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(
                    state
                        .histogram
                        .iter()
                        .enumerate()
                        .map(|(idx, (label, count))| {
                            let ratio = *count as f32 / max_bucket as f32;
                            let bar_w = (ratio * 220.0).max(if *count > 0 { 4.0 } else { 0.0 });
                            let bucket_label = label.clone();
                            let click_label = label.clone();
                            let enabled = !state.loading;
                            div()
                                .id(SharedString::from(format!("hist-bucket-{idx}")))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .px_1()
                                .py_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    if enabled {
                                        this.apply_histogram_bucket(&click_label, cx);
                                    }
                                }))
                                .child(
                                    div()
                                        .w(px(72.))
                                        .text_xs()
                                        .text_color(theme::c(theme::TEXT_MUTED))
                                        .child(bucket_label),
                                )
                                .child(
                                    div()
                                        .h(px(12.))
                                        .w(px(bar_w))
                                        .rounded_sm()
                                        .bg(theme::c(theme::HIST_BAR)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::c(theme::TEXT))
                                        .child(format!("{count}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme::c(theme::ACCENT))
                                        .child("筛选"),
                                )
                        }),
                ),
        )
}
