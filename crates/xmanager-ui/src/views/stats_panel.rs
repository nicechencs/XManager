//! Insights comparison cards and the all-tweets view histogram.

use crate::app::AppState;
use crate::theme::{self, space};
use crate::widgets::{section_label, stat_card, surface_card};
use gpui::{div, prelude::*, px, Context, Div, SharedString};
use xmanager_core::{bucket_overlaps_view_filter, Tweet};

fn kind_mix(s: &xmanager_core::Summary) -> String {
    format!(
        "原{} · 引{} · 回{} · 转{}",
        s.original_count, s.quote_count, s.reply_count, s.retweet_count
    )
}

fn scope_card(title: &str, s: &xmanager_core::Summary, extra: Option<String>) -> Div {
    surface_card(
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(220.))
            .gap(px(space::SM))
            .p(px(space::MD)),
    )
    .child(section_label(title.to_string()))
    .child(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(space::LG))
            .child(stat_card("均曝光", format!("{:.0}", s.avg_views)))
            .child(stat_card("中位曝光", format!("{:.0}", s.median_views)))
            .child(stat_card(
                "均互率",
                Tweet::format_rate(s.avg_engagement_rate),
            )),
    )
    .child(theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED))).child(kind_mix(s)))
    .when_some(extra, |el, line| {
        el.child(theme::type_meta(div().text_color(theme::c(theme::ACCENT))).child(line))
    })
}

pub fn render_scope_cards(state: &AppState) -> Div {
    let all = &state.all_summary;
    let slice = &state.summary;
    let share = if all.count == 0 {
        None
    } else {
        Some(format!(
            "占全部 {:.0}%",
            (slice.count as f64 / all.count as f64) * 100.0
        ))
    };
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(space::MD))
        .child(scope_card(&format!("全部 {} 条", all.count), all, None))
        .child(scope_card(
            &format!("当前切片 {} 条", slice.count),
            slice,
            share,
        ))
}

pub fn render_histogram(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let max_bucket = state
        .histogram
        .iter()
        .map(|(_, c)| *c)
        .max()
        .unwrap_or(0)
        .max(1);
    let min_views = state.applied_filter.min_views;
    let max_views = state.applied_filter.max_views;

    surface_card(div().flex().flex_col().gap(px(space::SM)).p(px(space::MD)))
        .child(section_label("曝光分布（全部数据，点柱去内容库）"))
        .child(
            div().flex().flex_col().gap(px(space::XS)).children(
                state
                    .histogram
                    .iter()
                    .enumerate()
                    .map(|(idx, (label, count))| {
                        let ratio = *count as f32 / max_bucket as f32;
                        let bar_w = (ratio * 360.0).max(if *count > 0 { 6.0 } else { 0.0 });
                        let in_slice = bucket_overlaps_view_filter(label, min_views, max_views);
                        let clickable = *count > 0 && !state.loading;
                        let bucket_label = label.clone();
                        let click_label = label.clone();
                        div()
                            .id(SharedString::from(format!("hist-bucket-{idx}")))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(space::SM))
                            .px(px(space::XS))
                            .py(px(space::XS))
                            .rounded(px(theme::radius::SM))
                            .bg(if in_slice {
                                theme::c(theme::CHIP_ACTIVE)
                            } else {
                                theme::c(theme::BG_ELEVATED)
                            })
                            .when(clickable, |el| {
                                el.cursor_pointer()
                                    .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                            })
                            .when(clickable, |el| {
                                el.on_click(cx.listener(move |this, _, _window, cx| {
                                    this.apply_histogram_bucket(&click_label, cx);
                                }))
                            })
                            .child(
                                theme::type_meta(
                                    div().w(px(72.)).text_color(theme::c(theme::TEXT_MUTED)),
                                )
                                .child(bucket_label),
                            )
                            .child(
                                div()
                                    .h(px(14.))
                                    .w(px(bar_w))
                                    .rounded(px(theme::radius::SM))
                                    .bg(theme::c(if in_slice {
                                        theme::ACCENT
                                    } else if *count > 0 {
                                        theme::HIST_BAR
                                    } else {
                                        theme::BORDER
                                    })),
                            )
                            .child(
                                theme::type_meta(div().text_color(theme::c(theme::TEXT)))
                                    .child(format!("{count}")),
                            )
                            .child(
                                theme::type_caption(div().text_color(if in_slice {
                                    theme::c(theme::ACCENT)
                                } else {
                                    theme::c(theme::TEXT_DIM)
                                }))
                                .child(if *count == 0 {
                                    String::new()
                                } else if in_slice {
                                    "当前切片".into()
                                } else {
                                    "查看".into()
                                }),
                            )
                    }),
            ),
        )
}
