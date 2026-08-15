//! Minimal GPUI widgets built on `div()`.

use crate::theme;
use gpui::{
    div, prelude::*, px, App, ClickEvent, Div, InteractiveElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window,
};

pub fn btn(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    primary: bool,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    let bg = if !enabled {
        theme::c(theme::DISABLED_BG)
    } else if primary {
        theme::c(theme::ACCENT)
    } else {
        theme::c(theme::CHIP)
    };
    let fg = if !enabled {
        theme::c(theme::DISABLED_TEXT)
    } else if primary {
        theme::c(theme::TEXT_ON_ACCENT)
    } else {
        theme::c(theme::TEXT)
    };

    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.))
        .px_3()
        .rounded_md()
        .bg(bg)
        .text_color(fg)
        .text_sm()
        .border_1()
        .border_color(theme::c(theme::BORDER))
        .when(enabled, |el| {
            el.cursor_pointer()
                .hover(|s| {
                    s.bg(if primary {
                        theme::c(theme::ACCENT_HOVER)
                    } else {
                        theme::c(theme::BG_HOVER)
                    })
                })
                .active(|s| s.opacity(0.85))
                .on_click(on_click)
        })
        .child(label)
}

pub fn danger_btn(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.))
        .px_3()
        .rounded_md()
        .bg(if enabled {
            theme::c(theme::DANGER)
        } else {
            theme::c(theme::DISABLED_DANGER_BG)
        })
        .text_color(if enabled {
            theme::c(theme::TEXT_ON_DANGER)
        } else {
            theme::c(theme::DISABLED_TEXT)
        })
        .text_sm()
        .border_1()
        .border_color(theme::c(theme::BORDER))
        .when(enabled, |el| {
            el.cursor_pointer()
                .hover(|s| s.bg(theme::c(theme::DANGER_HOVER)))
                .active(|s| s.opacity(0.85))
                .on_click(on_click)
        })
        .child(label)
}

pub fn toggle_chip(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .h(px(26.))
        .px_2()
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .bg(if active {
            theme::c(theme::CHIP_ACTIVE)
        } else {
            theme::c(theme::CHIP)
        })
        .text_color(if active {
            theme::c(theme::TEXT)
        } else {
            theme::c(theme::TEXT_MUTED)
        })
        .border_1()
        .border_color(if active {
            theme::c(theme::ACCENT)
        } else {
            theme::c(theme::BORDER)
        })
        .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
        .on_click(on_click)
        .child(label.into())
}

pub fn stat_card(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(96.))
        .px_3()
        .py_2()
        .rounded_md()
        .bg(theme::c(theme::BG_ELEVATED))
        .border_1()
        .border_color(theme::c(theme::BORDER))
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(label.into()),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::c(theme::TEXT))
                .child(value.into()),
        )
}

pub fn section_label(text: impl Into<SharedString>) -> Div {
    div()
        .text_sm()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::c(theme::TEXT_MUTED))
        .child(text.into())
}

pub fn checkbox_mark(checked: bool) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded_sm()
        .border_1()
        .border_color(if checked {
            theme::c(theme::ACCENT)
        } else {
            theme::c(theme::BORDER_STRONG)
        })
        .bg(if checked {
            theme::c(theme::ACCENT)
        } else {
            theme::c(theme::BG)
        })
        .text_xs()
        .text_color(theme::c(theme::TEXT_ON_ACCENT))
        .child(if checked { "✓" } else { " " })
}

pub fn stepper(
    id_prefix: &str,
    value: u64,
    step: u64,
    min: u64,
    max: u64,
    on_delta: impl Fn(i64, &mut Window, &mut App) + 'static + Clone,
) -> Div {
    let id_minus = format!("{id_prefix}-minus");
    let id_plus = format!("{id_prefix}-plus");
    let on_minus = on_delta.clone();
    let on_plus = on_delta;
    let range_label = format!("[{min}–{max}]");

    div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .id(SharedString::from(id_minus))
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded_sm()
                .bg(theme::c(theme::CHIP))
                .text_color(theme::c(theme::TEXT))
                .cursor_pointer()
                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                .on_click(move |_, window, cx| {
                    on_minus(-(step as i64), window, cx);
                })
                .child("−"),
        )
        .child(
            div()
                .min_w(px(48.))
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(theme::c(theme::BG))
                .border_1()
                .border_color(theme::c(theme::BORDER))
                .text_sm()
                .text_color(theme::c(theme::TEXT))
                .child(format!("{value}")),
        )
        .child(
            div()
                .id(SharedString::from(id_plus))
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded_sm()
                .bg(theme::c(theme::CHIP))
                .text_color(theme::c(theme::TEXT))
                .cursor_pointer()
                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                .on_click(move |_, window, cx| {
                    on_plus(step as i64, window, cx);
                })
                .child("+"),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_DIM))
                .child(range_label),
        )
}

pub fn truncate_text(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
