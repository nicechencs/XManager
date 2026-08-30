//! Minimal GPUI widgets built on `div()`, using theme type and space tokens.

use crate::theme::{self, control, radius, space};
use gpui::{
    div, img, prelude::*, px, App, ClickEvent, Div, InteractiveElement, Rgba, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window,
};
use xmanager_core::PostKind;

pub fn app_logo() -> Div {
    div()
        .size(px(28.))
        .rounded(px(radius::MD))
        .overflow_hidden()
        .flex_none()
        .child(img("logo.png").size(px(28.)))
}

/// Card chrome: hairline + light shadow + 8px radius.
pub fn surface_card<E: Styled>(el: E) -> E {
    el.bg(theme::c(theme::BG_ELEVATED))
        .border_1()
        .border_color(theme::c(theme::BORDER))
        .rounded(px(radius::LG))
        .shadow_sm()
}

/// Primary (filled) vs ghost (outline) vs disabled.
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
        theme::c(theme::BG_ELEVATED)
    };
    let fg = if !enabled {
        theme::c(theme::DISABLED_TEXT)
    } else if primary {
        theme::c(theme::TEXT_ON_ACCENT)
    } else {
        theme::c(theme::TEXT)
    };
    let border = if !enabled {
        theme::c(theme::BORDER)
    } else if primary {
        theme::c(theme::ACCENT)
    } else {
        theme::c(theme::BORDER_STRONG)
    };

    theme::type_label(
        div()
            .id(id.into())
            .flex()
            .items_center()
            .justify_center()
            .h(px(control::HEIGHT))
            .px(px(space::MD))
            .rounded(px(radius::MD))
            .bg(bg)
            .text_color(fg)
            .border_1()
            .border_color(border),
    )
    .when(enabled, |el| {
        el.cursor_pointer()
            .hover(|s| {
                s.bg(if primary {
                    theme::c(theme::ACCENT_HOVER)
                } else {
                    theme::c(theme::BG_HOVER)
                })
            })
            .active(|s| s.opacity(0.88))
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
    theme::type_label(
        div()
            .id(id.into())
            .flex()
            .items_center()
            .justify_center()
            .h(px(control::HEIGHT))
            .px(px(space::MD))
            .rounded(px(radius::MD))
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
            .border_1()
            .border_color(if enabled {
                theme::c(theme::DANGER)
            } else {
                theme::c(theme::BORDER)
            }),
    )
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
    theme::type_body(
        div()
            .id(id.into())
            .flex()
            .items_center()
            .justify_center()
            .h(px(control::CHIP_HEIGHT))
            .px(px(space::SM))
            .rounded(px(radius::MD))
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
            }),
    )
    .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
    .on_click(on_click)
    .child(label.into())
}

pub fn stat_card(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Div {
    surface_card(
        div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .min_w(px(104.))
            .px(px(space::MD))
            .py(px(space::SM)),
    )
    .child(theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED))).child(label.into()))
    .child(theme::type_title(div().text_color(theme::c(theme::TEXT))).child(value.into()))
}

pub fn section_label(text: impl Into<SharedString>) -> Div {
    theme::type_label(div().text_color(theme::c(theme::TEXT_MUTED))).child(text.into())
}

pub fn checkbox_mark(checked: bool) -> Div {
    theme::type_caption(
        div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(16.))
            .rounded(px(radius::SM))
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
            .text_color(theme::c(theme::TEXT_ON_ACCENT)),
    )
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
    let range_label = format!("[{min}–{max}] · 0 为不限");
    let value_label = if value == 0 {
        "不限".to_string()
    } else {
        value.to_string()
    };

    div()
        .flex()
        .items_center()
        .gap(px(space::XS))
        .child(
            div()
                .id(SharedString::from(id_minus))
                .flex()
                .items_center()
                .justify_center()
                .size(px(control::ICON))
                .rounded(px(radius::SM))
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
            theme::type_body(
                div()
                    .min_w(px(56.))
                    .px(px(space::SM))
                    .py(px(space::XS))
                    .rounded(px(radius::SM))
                    .bg(theme::c(theme::BG))
                    .border_1()
                    .border_color(theme::c(theme::BORDER))
                    .text_color(theme::c(theme::TEXT)),
            )
            .child(value_label),
        )
        .child(
            div()
                .id(SharedString::from(id_plus))
                .flex()
                .items_center()
                .justify_center()
                .size(px(control::ICON))
                .rounded(px(radius::SM))
                .bg(theme::c(theme::CHIP))
                .text_color(theme::c(theme::TEXT))
                .cursor_pointer()
                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                .on_click(move |_, window, cx| {
                    on_plus(step as i64, window, cx);
                })
                .child("+"),
        )
        .child(theme::type_caption(div().text_color(theme::c(theme::TEXT_DIM))).child(range_label))
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

pub fn kind_color(kind: PostKind) -> Rgba {
    match kind {
        PostKind::Original => theme::c(theme::SUCCESS),
        PostKind::Reply => theme::c(theme::ACCENT),
        PostKind::Retweet => theme::c(theme::WARNING),
        PostKind::Quote => theme::c(theme::QUOTE),
    }
}

pub fn kind_badge(kind: PostKind) -> Div {
    theme::type_caption(
        div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(20.))
            .px(px(space::SM))
            .rounded(px(radius::MD))
            .bg(theme::c(theme::CHIP))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(kind_color(kind)),
    )
    .child(kind.label_zh().to_string())
}

pub fn metric_tile(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Div {
    surface_card(
        div()
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .min_w(px(88.))
            .flex_1()
            .px(px(space::SM))
            .py(px(space::SM)),
    )
    .child(theme::type_meta(div().text_color(theme::c(theme::TEXT_MUTED))).child(label.into()))
    .child(
        theme::type_label(
            div()
                .text_color(theme::c(theme::TEXT))
                .font_weight(gpui::FontWeight::SEMIBOLD),
        )
        .child(value.into()),
    )
}

pub fn page_heading(title: impl Into<SharedString>, subtitle: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(space::SM))
        .child(theme::type_display(div().text_color(theme::c(theme::TEXT))).child(title.into()))
        .child(
            theme::type_body(div().text_color(theme::c(theme::TEXT_MUTED))).child(subtitle.into()),
        )
}

/// Illustration-free empty block: mark + title + body + optional action slot.
pub fn empty_mark(glyph: impl Into<SharedString>) -> Div {
    theme::type_title(
        div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(control::EMPTY_MARK))
            .rounded(px(radius::LG))
            .bg(theme::c(theme::CHIP))
            .border_1()
            .border_color(theme::c(theme::BORDER))
            .text_color(theme::c(theme::TEXT_MUTED)),
    )
    .child(glyph.into())
}

pub fn status_pill(ok: bool, label: impl Into<SharedString>) -> Div {
    theme::type_meta(
        div()
            .flex()
            .items_center()
            .gap(px(space::XS))
            .h(px(control::CHIP_HEIGHT))
            .px(px(space::SM))
            .rounded(px(radius::MD))
            .bg(theme::c(theme::CHIP))
            .border_1()
            .border_color(theme::c(theme::BORDER))
            .text_color(if ok {
                theme::c(theme::SUCCESS)
            } else {
                theme::c(theme::WARNING)
            }),
    )
    .child(if ok { "●" } else { "○" })
    .child(label.into())
}

pub fn removable_chip(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    theme::type_caption(
        div()
            .id(id.into())
            .flex()
            .items_center()
            .gap(px(space::XS))
            .h(px(control::CHIP_HEIGHT))
            .px(px(space::SM))
            .rounded(px(radius::MD))
            .bg(theme::c(theme::CHIP))
            .border_1()
            .border_color(theme::c(theme::BORDER))
            .text_color(if enabled {
                theme::c(theme::TEXT)
            } else {
                theme::c(theme::DISABLED_TEXT)
            }),
    )
    .when(enabled, |el| {
        el.cursor_pointer()
            .hover(|s| {
                s.bg(theme::c(theme::BG_HOVER))
                    .border_color(theme::c(theme::ACCENT))
            })
            .on_click(on_click)
    })
    .child(label.into())
    .child(theme::type_caption(div().text_color(theme::c(theme::TEXT_MUTED))).child("×"))
}

pub fn count_badge(count: usize) -> Div {
    theme::type_caption(
        div()
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(20.))
            .h(px(18.))
            .px(px(space::XS))
            .rounded(px(radius::MD))
            .bg(if count > 0 {
                theme::c(theme::CHIP_ACTIVE)
            } else {
                theme::c(theme::CHIP)
            })
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(if count > 0 {
                theme::c(theme::TEXT)
            } else {
                theme::c(theme::TEXT_MUTED)
            }),
    )
    .child(count.to_string())
}

pub fn nav_destination(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    glyph: impl Into<SharedString>,
    compact: bool,
    active: bool,
    badge: Option<usize>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    let glyph = glyph.into();
    div()
        .id(id.into())
        .flex()
        .when(compact, |el| {
            el.flex_col()
                .items_center()
                .justify_center()
                .gap(px(space::XS))
                .py(px(space::SM))
                .px(px(space::XS))
        })
        .when(!compact, |el| {
            el.flex_row()
                .items_center()
                .gap(px(space::SM))
                .px(px(space::MD))
                .py(px(space::SM))
        })
        .w_full()
        .rounded(px(radius::MD))
        .cursor_pointer()
        .bg(if active {
            theme::c(theme::CHIP_ACTIVE)
        } else {
            theme::c(theme::BG_PANEL)
        })
        .border_1()
        .border_color(if active {
            theme::c(theme::ACCENT)
        } else {
            theme::c(theme::BG_PANEL)
        })
        .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
        .on_click(on_click)
        .child(
            theme::type_label(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .text_color(if active {
                        theme::c(theme::ACCENT)
                    } else {
                        theme::c(theme::TEXT_MUTED)
                    }),
            )
            .child(glyph),
        )
        .child(
            theme::type_label(
                div()
                    .flex_1()
                    .font_weight(if active {
                        gpui::FontWeight::SEMIBOLD
                    } else {
                        gpui::FontWeight::MEDIUM
                    })
                    .text_color(if active {
                        theme::c(theme::TEXT)
                    } else {
                        theme::c(theme::TEXT_MUTED)
                    }),
            )
            .child(label),
        )
        .when_some(badge, |el, count| el.child(count_badge(count)))
}
