//! Virtualized tweet list with checkbox selection, kind & rates.

use crate::app::AppState;
use crate::theme;
use crate::widgets::{checkbox_mark, truncate_text};
use gpui::{div, prelude::*, px, uniform_list, Div, SharedString, Window};
use xmanager_core::{PostKind, Tweet};

const COL_CHECK: f32 = 32.0;
const COL_KIND: f32 = 36.0;
const COL_DATE: f32 = 108.0;
const COL_VIEWS: f32 = 64.0;
const COL_LIKES: f32 = 48.0;
const COL_BM: f32 = 48.0;
const COL_LIKE_R: f32 = 56.0;
const COL_BM_R: f32 = 56.0;
const COL_ENG_R: f32 = 56.0;
const CARD_ROW_H: f32 = 92.0;
const TABLE_ROW_H: f32 = 40.0;

fn header_cell(label: &str, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .px_1()
        .text_sm()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::c(theme::TEXT_MUTED))
        .child(label.to_string())
}

fn cell(text: impl Into<SharedString>, width: f32, muted: bool) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .px_1()
        .text_sm()
        .text_color(if muted {
            theme::c(theme::TEXT_MUTED)
        } else {
            theme::c(theme::TEXT)
        })
        .overflow_hidden()
        .whitespace_nowrap()
        .child(text.into())
}

fn kind_color(kind: PostKind) -> gpui::Rgba {
    match kind {
        PostKind::Original => theme::c(theme::SUCCESS),
        PostKind::Reply => theme::c(theme::ACCENT),
        PostKind::Retweet => theme::c(theme::WARNING),
        PostKind::Quote => theme::c(theme::QUOTE),
    }
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

fn table_header() -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(36.))
        .px_2()
        .bg(theme::c(theme::BG_ELEVATED))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .child(header_cell("", COL_CHECK))
        .child(header_cell("类型", COL_KIND))
        .child(header_cell("日期", COL_DATE))
        .child(header_cell("曝光", COL_VIEWS))
        .child(header_cell("赞", COL_LIKES))
        .child(header_cell("藏", COL_BM))
        .child(header_cell("赞率", COL_LIKE_R))
        .child(header_cell("藏率", COL_BM_R))
        .child(header_cell("互率", COL_ENG_R))
        .child(
            div()
                .flex_1()
                .px_1()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme::c(theme::TEXT_MUTED))
                .child("内容"),
        )
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
        .child(skeleton_cell(DATE_W[ix], COL_DATE, muted))
        .child(skeleton_cell(VIEWS_W[ix], COL_VIEWS, muted))
        .child(skeleton_cell(28., COL_LIKES, muted))
        .child(skeleton_cell(28., COL_BM, muted))
        .child(skeleton_cell(32., COL_LIKE_R, muted))
        .child(skeleton_cell(32., COL_BM_R, muted))
        .child(skeleton_cell(32., COL_ENG_R, muted))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .px_1()
                .flex()
                .items_center()
                .child(skeleton_bar(CONTENT_W[ix], muted)),
        )
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
                .gap_1()
                .px_4()
                .py_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::c(theme::TEXT))
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::c(theme::TEXT_MUTED))
                        .child(detail),
                ),
        )
        .children((0..8).map(skeleton_row))
}

fn empty_state(state: &AppState) -> Div {
    let first_load = state.loading && state.all_tweets.is_empty();
    let (title, detail) = if first_load {
        ("正在拉取…", "首次同步完成后会显示结果。")
    } else if !state.credentials_ok {
        (
            "尚未配置凭证",
            "请在项目根目录配置 .env（X_API_KEY 等），然后点击侧栏「刷新状态」。",
        )
    } else if state.all_tweets.is_empty() {
        (
            "暂无数据",
            "点击右上角「拉取并分析」同步你的推文。旧数据会在刷新时保留到完成。",
        )
    } else {
        (
            "没有匹配结果",
            "当前筛选条件下为空。可移除工具栏条件标签，或打开筛选抽屉调整。",
        )
    };
    if first_load {
        return first_load_skeleton(title, detail);
    }
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_2()
        .px_6()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme::c(theme::TEXT))
                .child(title),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_sm()
                .text_color(theme::c(theme::TEXT_MUTED))
                .child(detail),
        )
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
    let text_preview = truncate_text(&tweet.text.replace('\n', " "), 64);
    let views = tweet.views();
    let likes = tweet.public_metrics.like_count;
    let bookmarks = tweet.public_metrics.bookmark_count;
    let like_r = Tweet::format_rate(tweet.like_rate());
    let bm_r = Tweet::format_rate(tweet.bookmark_rate());
    let eng_r = Tweet::format_rate(tweet.engagement_rate());
    let date = tweet.display_date();

    div()
        .id(SharedString::from(format!("tweet-row-{id}")))
        .flex()
        .flex_row()
        .items_center()
        .h(px(TABLE_ROW_H))
        .px_2()
        .bg(row_bg(focused, selected, ix))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
        .on_click(cx.listener(move |this, _, _w, cx| {
            this.focus_tweet(&row_id, cx);
        }))
        .child(row_checkbox(id, selected, cx))
        .child(
            div()
                .w(px(COL_KIND))
                .flex_none()
                .px_1()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(kind_color(kind))
                .child(kind.short()),
        )
        .child(cell(date, COL_DATE, true))
        .child(cell(format!("{views}"), COL_VIEWS, false))
        .child(cell(format!("{likes}"), COL_LIKES, false))
        .child(cell(format!("{bookmarks}"), COL_BM, false))
        .child(cell(like_r, COL_LIKE_R, false))
        .child(cell(bm_r, COL_BM_R, false))
        .child(cell(eng_r, COL_ENG_R, false))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .px_1()
                .text_xs()
                .text_color(theme::c(theme::TEXT))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(text_preview),
        )
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
    let text_preview = truncate_text(&tweet.text.replace('\n', " "), 80);
    let views = tweet.views();
    let likes = tweet.public_metrics.like_count;
    let bookmarks = tweet.public_metrics.bookmark_count;
    let date = tweet.display_date();

    div()
        .id(SharedString::from(format!("tweet-row-{id}")))
        .flex()
        .flex_row()
        .items_start()
        .h(px(CARD_ROW_H))
        .px_2()
        .py_2()
        .gap_1()
        .bg(row_bg(focused, selected, ix))
        .border_b_1()
        .border_color(theme::c(theme::BORDER))
        .cursor_pointer()
        .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
        .on_click(cx.listener(move |this, _, _w, cx| {
            this.focus_tweet(&row_id, cx);
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
                        .child(labeled_field("日期", date, theme::c(theme::TEXT_MUTED))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(labeled_field(
                            "曝光",
                            format!("{views}"),
                            theme::c(theme::TEXT),
                        ))
                        .child(labeled_field(
                            "赞",
                            format!("{likes}"),
                            theme::c(theme::TEXT),
                        ))
                        .child(labeled_field(
                            "藏",
                            format!("{bookmarks}"),
                            theme::c(theme::TEXT),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .min_w(px(0.))
                        .child(
                            div()
                                .text_xs()
                                .flex_none()
                                .text_color(theme::c(theme::TEXT_MUTED))
                                .child("内容"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_xs()
                                .text_color(theme::c(theme::TEXT))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(text_preview),
                        ),
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
        .bg(theme::c(theme::BG))
        .when(!as_cards, |el| el.child(table_header()))
        .child(if count == 0 {
            empty_state(state).into_any_element()
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
