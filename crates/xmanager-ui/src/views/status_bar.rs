//! Bottom status bar.

use crate::app::AppState;
use crate::theme;
use gpui::{div, prelude::*, px, Div};

pub fn render_status_bar(state: &AppState) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .h(px(28.))
        .px_3()
        .bg(theme::c(theme::BG_ELEVATED))
        .border_t_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(format!(
                    "共 {} 条 · 筛选 {} · 已选 {} · 候选 {}",
                    state.all_tweets.len(),
                    state.filtered.len(),
                    state.selected.len(),
                    state.cleanup_candidates.len()
                )),
        )
        .when(state.loading, |el| {
            el.child(
                div()
                    .text_xs()
                    .text_color(theme::c(theme::ACCENT))
                    .child("● 处理中…"),
            )
        })
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(theme::c(theme::TEXT))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(state.status_msg.clone()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(format!(
                    "布局 {} · J/K 上下 · 空格勾选 · / 筛选 · 1–3 切页 · Esc 关闭",
                    state.layout_mode.label_zh()
                )),
        )
        .child(
            div()
                .text_xs()
                .text_color(if state.credentials_ok {
                    theme::c(theme::SUCCESS)
                } else {
                    theme::c(theme::WARNING)
                })
                .child(state.credentials_msg.clone()),
        )
}
