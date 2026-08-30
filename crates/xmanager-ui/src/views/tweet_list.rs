//! Virtualized tweet list with checkbox selection, kind & rates.

use crate::app::{chip_label, library_empty_copy, AppState, LibraryEmptyKind};
use crate::theme::{self, space};
use crate::widgets::{btn, checkbox_mark, empty_mark, kind_color, removable_chip, truncate_text};
use gpui::{div, prelude::*, px, uniform_list, Context, Div, SharedString, Window};
use xmanager_core::{SortField, SortOrder, Tweet};

const COL_CHECK: f32 = 32.0;
const COL_KIND: f32 = 32.0;
const COL_DATE: f32 = 56.0;
const COL_VIEWS: f32 = 56.0;
const CARD_ROW_H: f32 = 88.0;
const TABLE_ROW_H: f32 = 56.0;
const CONTENT_LINES_H: f32 = 40.0;

fn header_cell(label: &str, width: f32) -> Div {
    theme::type_meta(
        div()
            .w(px(width))
            .flex_none()
            .px(px(space::SM))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme::c(theme::TEXT_MUTED)),
    )
    .child(label.to_string())
}

fn sort_header(
    label: &'static str,
    width: f32,
    field: SortField,
    state: &AppState,
    cx: &mut Context<AppState>,
) -> impl gpui::IntoElement {
    let active = state.applied_filter.sort == field;
    let arrow = if !active {
        ""
    } else if state.applied_filter.order == SortOrder::Asc {
        " ↑"
    } else {
        " ↓"
    };
    theme::type_meta(
        div()
            .id(SharedString::from(format!("sort-header-{label}")))
            .w(px(width))
            .flex_none()
            .px(px(space::SM))
            .font_weight(if active {
                gpui::FontWeight::SEMIBOLD
            } else {
                gpui::FontWeight::MEDIUM
            })
            .text_color(if active {
                theme::c(theme::ACCENT)
            } else {
                theme::c(theme::TEXT_MUTED)
            })
            .cursor_pointer()
            .hover(|s| s.text_color(theme::c(theme::TEXT)))
            .on_click(cx.listener(move |this, _, _window, cx| this.apply_sort_header(field, cx))),
    )
    .child(format!("{label}{arrow}"))
}

fn cell(text: impl Into<SharedString>, width: f32, muted: bool) -> Div {
    theme::type_body(
        div()
            .w(px(width))
            .flex_none()
            .px(px(space::SM))
            .text_color(if muted {
                theme::c(theme::TEXT_MUTED)
            } else {
                theme::c(theme::TEXT)
            })
            .overflow_hidden()
            .whitespace_nowrap(),
    )
    .child(text.into())
}

fn row_bg(focused: bool, selected: bool, ix: usize) -> gpui::Rgba {
    if focused {
        theme::c(theme::BG_SELECTED)
    } else if selected {
        theme::c(theme::BG_ROW_SELECTED)
    } else if ix % 2 == 0 {
        theme::c(theme::BG_ROW)
    } else {
        theme::c(theme::BG_ROW_ALT)
    }
}

fn labeled_field(label: &str, value: impl Into<SharedString>, color: gpui::Rgba) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(color)
                .overflow_hidden()
                .whitespace_nowrap()
                .child(value.into()),
        )
}

fn table_header(state: &AppState, cx: &mut Context<AppState>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(40.))
        .px(px(space::SM))
        .bg(theme::c(theme::BG_PANEL))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .child(header_cell("", COL_CHECK))
        .child(header_cell("类型", COL_KIND))
        .child(
            div().flex_1().px(px(space::SM)).child(
                theme::type_meta(
                    div()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme::c(theme::TEXT_MUTED)),
                )
                .child("内容"),
            ),
        )
        .child(sort_header("日期", COL_DATE, SortField::Date, state, cx))
        .child(sort_header("曝光", COL_VIEWS, SortField::Views, state, cx))
}

fn skeleton_bar(width: f32, muted: bool) -> Div {
    div()
        .w(px(width))
        .h(px(10.))
        .rounded_sm()
        .bg(theme::c(if muted {
            theme::CHIP
        } else {
            theme::BG_ELEVATED
        }))
}

fn skeleton_cell(bar_width: f32, col_width: f32, muted: bool) -> Div {
    div()
        .w(px(col_width))
        .flex_none()
        .px_1()
        .flex()
        .items_center()
        .child(skeleton_bar(bar_width, muted))
}

fn skeleton_row(ix: usize) -> Div {
    const CONTENT_W: [f32; 8] = [172., 244., 128., 216., 188., 148., 232., 160.];
    const DATE_W: [f32; 8] = [76., 84., 64., 80., 70., 88., 72., 78.];
    const VIEWS_W: [f32; 8] = [36., 28., 40., 32., 24., 38., 30., 34.];
    let muted = ix % 2 == 1;
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(TABLE_ROW_H))
        .px_2()
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .child(skeleton_cell(14., COL_CHECK, muted))
        .child(skeleton_cell(20., COL_KIND, muted))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .px_1()
                .flex()
                .items_center()
                .child(skeleton_bar(CONTENT_W[ix], muted)),
        )
        .child(skeleton_cell(DATE_W[ix], COL_DATE, muted))
        .child(skeleton_cell(VIEWS_W[ix], COL_VIEWS, muted))
}

fn first_load_skeleton(title: &'static str, detail: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::XS))
                .px(px(space::LG))
                .py(px(space::MD))
                .child(theme::type_title(div().text_color(theme::c(theme::TEXT))).child(title))
                .child(
                    theme::type_body(div().text_color(theme::c(theme::TEXT_MUTED))).child(detail),
                ),
        )
        .children((0..8).map(skeleton_row))
}

fn empty_state(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let copy = library_empty_copy(
        state.loading,
        state.all_tweets.len(),
        state.filtered.len(),
        state.last_synced_at.is_some(),
        state.last_fetch_failed,
        state.credential_layout,
    );
    if copy.kind == LibraryEmptyKind::Loading {
        return first_load_skeleton("正在拉取…", "首次同步完成后会显示结果。");
    }
    let need_refresh = matches!(
        copy.kind,
        LibraryEmptyKind::CredentialsMissing | LibraryEmptyKind::CredentialsSwapped
    );
    let need_fetch = matches!(
        copy.kind,
        LibraryEmptyKind::NeverSynced
            | LibraryEmptyKind::FetchFailed
            | LibraryEmptyKind::AccountEmpty
    );
    let no_match = copy.kind == LibraryEmptyKind::FilteredEmpty;
    let enabled = !state.loading;
    let chip_els: Vec<_> = if no_match {
        state
            .applied_chips()
            .into_iter()
            .enumerate()
            .map(|(idx, chip)| {
                let label = chip_label(&chip);
                removable_chip(
                    format!("empty-chip-{idx}"),
                    label,
                    enabled,
                    cx.listener(move |this, _, _window, cx| {
                        this.remove_applied_chip(chip.clone(), cx);
                    }),
                )
                .into_any_element()
            })
            .collect()
    } else {
        Vec::new()
    };
    let has_chips = !chip_els.is_empty();
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(space::MD))
        .px(px(space::XL))
        .child(empty_mark(copy.kind.mark()))
        .child(theme::type_title(div().text_color(theme::c(theme::TEXT))).child(copy.title))
        .child(
            theme::type_body(
                div()
                    .max_w(px(440.))
                    .text_color(theme::c(theme::TEXT_MUTED)),
            )
            .child(copy.detail),
        )
        .when(has_chips, |el| {
            el.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(space::SM))
                    .children(chip_els),
            )
        })
        .when(need_refresh, |el| {
            el.child(btn(
                "empty-refresh",
                "刷新状态",
                true,
                !state.loading,
                cx.listener(|this, _, _window, cx| this.refresh_whoami(cx)),
            ))
        })
        .when(need_fetch, |el| {
            el.child(btn(
                "empty-fetch",
                "拉取并分析",
                true,
                !state.loading,
                cx.listener(|this, _, _window, cx| this.fetch_tweets(cx)),
            ))
        })
        .when(no_match, |el| {
            el.child(btn(
                "empty-clear-filters",
                "清除筛选",
                true,
                !state.loading,
                cx.listener(|this, _, _window, cx| {
                    this.filter_draft = crate::app::FilterDraft::unrestricted();
                    this.apply_filters(cx);
                    cx.notify();
                }),
            ))
        })
}

fn row_checkbox(id: String, selected: bool, cx: &mut Context<AppState>) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("tweet-check-{id}")))
        .w(px(COL_CHECK))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _w, cx| {
            cx.stop_propagation();
            this.toggle_selected(&id, cx);
        }))
        .child(checkbox_mark(selected))
}

fn tweet_table_row(
    tweet: &Tweet,
    selected: bool,
    focused: bool,
    ix: usize,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let id = tweet.id.clone();
    let row_id = id.clone();
    let kind = tweet.kind();
    let text_preview = truncate_text(&tweet.text.replace('\n', " "), 120);
    let views = tweet.views();
    let date = tweet.display_date_short();

    div()
        .id(SharedString::from(format!("tweet-row-{id}")))
        .flex()
        .flex_row()
        .items_center()
        .h(px(TABLE_ROW_H))
        .px(px(space::SM))
        .bg(row_bg(focused, selected, ix))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
        .on_click(cx.listener(move |this, _, _w, cx| {
            this.toggle_tweet_focus(&row_id, cx);
        }))
        .child(row_checkbox(id, selected, cx))
        .child(
            theme::type_meta(
                div()
                    .w(px(COL_KIND))
                    .flex_none()
                    .px(px(space::XS))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(kind_color(kind)),
            )
            .child(kind.short()),
        )
        .child(
            div().flex_1().min_w(px(0.)).px(px(space::SM)).child(
                theme::type_body(
                    div()
                        .h(px(CONTENT_LINES_H))
                        .text_color(theme::c(theme::TEXT))
                        .overflow_hidden(),
                )
                .child(text_preview),
            ),
        )
        .child(cell(date, COL_DATE, true))
        .child(cell(format!("{views}"), COL_VIEWS, false))
}

fn tweet_card_row(
    tweet: &Tweet,
    selected: bool,
    focused: bool,
    ix: usize,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let id = tweet.id.clone();
    let row_id = id.clone();
    let kind = tweet.kind();
    let text_preview = truncate_text(&tweet.text.replace('\n', " "), 100);
    let views = tweet.views();
    let date = tweet.display_date_short();

    div()
        .id(SharedString::from(format!("tweet-row-{id}")))
        .flex()
        .flex_row()
        .items_start()
        .h(px(CARD_ROW_H))
        .px(px(space::MD))
        .py(px(space::SM))
        .gap(px(space::SM))
        .bg(row_bg(focused, selected, ix))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
        .on_click(cx.listener(move |this, _, _w, cx| {
            this.toggle_tweet_focus(&row_id, cx);
        }))
        .child(row_checkbox(id, selected, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .gap_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(labeled_field("类型", kind.label_zh(), kind_color(kind)))
                        .child(labeled_field("日期", date, theme::c(theme::TEXT_MUTED)))
                        .child(labeled_field(
                            "曝光",
                            format!("{views}"),
                            theme::c(theme::TEXT),
                        )),
                )
                .child(
                    theme::type_body(
                        div()
                            .h(px(CONTENT_LINES_H))
                            .min_w(px(0.))
                            .text_color(theme::c(theme::TEXT))
                            .overflow_hidden(),
                    )
                    .child(text_preview),
                ),
        )
}

pub fn render_tweet_list(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let count = state.filtered.len();
    let as_cards = state.layout_mode.tweet_list_as_cards();

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .when(!as_cards, |el| el.child(table_header(state, cx)))
        .child(if count == 0 {
            empty_state(state, cx).into_any_element()
        } else {
            uniform_list(
                "tweet-list",
                count,
                cx.processor(|this, range, _window: &mut Window, cx| {
                    let as_cards = this.layout_mode.tweet_list_as_cards();
                    let mut items = Vec::new();
                    for ix in range {
                        let Some(tweet): Option<Tweet> = this.filtered.get(ix).cloned() else {
                            continue;
                        };
                        let selected = this.selected.contains(&tweet.id);
                        let focused = this.focused_tweet_id.as_deref() == Some(tweet.id.as_str());
                        if as_cards {
                            items.push(
                                tweet_card_row(&tweet, selected, focused, ix, cx)
                                    .into_any_element(),
                            );
                        } else {
                            items.push(
                                tweet_table_row(&tweet, selected, focused, ix, cx)
                                    .into_any_element(),
                            );
                        }
                    }
                    items
                }),
            )
            .flex_1()
            .h_full()
            .track_scroll(state.library_scroll.clone())
            .into_any_element()
        })
}
