//! Bottom status bar — web-app footer.

use crate::app::{credentials_line_healthy, AppState};
use crate::theme::{self, space};
use crate::widgets::status_pill;
use gpui::{div, prelude::*, px, Div};

pub fn render_status_bar(state: &AppState) -> Div {
    let creds_ok = credentials_line_healthy(state.credential_layout, state.current_user.is_some());
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::MD))
        .h(px(40.))
        .px(px(space::LG))
        .bg(theme::c(theme::BG_PANEL))
        .border_t_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED))).child(format!(
                "共 {} 条 · 筛选 {} · 已选 {} · 候选 {}",
                state.all_tweets.len(),
                state.filtered.len(),
                state.selected.len(),
                state.cleanup_candidates.len()
            )),
        )
        .when(state.loading, |el| {
            el.child(theme::type_meta(div().text_color(theme::c(theme::ACCENT))).child("● 处理中…"))
        })
        .child(
            theme::type_meta(
                div()
                    .flex_1()
                    .text_color(theme::c(theme::TEXT))
                    .overflow_hidden()
                    .whitespace_nowrap(),
            )
            .child(state.status_msg.clone()),
        )
        .child(
            theme::type_meta(div().text_color(theme::c(theme::TEXT_DIM))).child(format!(
                "布局 {} · J/K 上下 · 空格勾选 · / 筛选 · 1–3 切页 · Esc 关闭",
                state.layout_mode.label_zh()
            )),
        )
        .child(status_pill(creds_ok, state.credentials_msg.clone()))
}
