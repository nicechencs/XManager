//! Virtualized tweet list with checkbox selection, kind & rates.

use crate::app::AppState;
use crate::theme;
use crate::widgets::{checkbox_mark, truncate_text};
use gpui::{
    div, px, uniform_list, Context, Div, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
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

pub fn render_tweet_list(state: &AppState, cx: &mut Context<AppState>) -> Div {
    let count = state.filtered.len();

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .bg(theme::c(theme::BG))
        .child(
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
                ),
        )
        .child(if count == 0 {
            let (title, detail) = if state.loading && state.all_tweets.is_empty() {
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
                .into_any_element()
        } else {
            uniform_list(
                "tweet-list",
                count,
                cx.processor(|this, range, _window: &mut Window, cx| {
                    let mut items = Vec::new();
                    for ix in range {
                        let Some(tweet): Option<Tweet> = this.filtered.get(ix).cloned() else {
                            continue;
                        };
                        let selected = this.selected.contains(&tweet.id);
                        let id = tweet.id.clone();
                        let row_id = id.clone();
                        let checkbox_id = id.clone();
                        let focused = this.focused_tweet_id.as_deref() == Some(tweet.id.as_str());
                        let row_bg = if focused {
                            theme::c(theme::BG_SELECTED)
                        } else if selected {
                            theme::c(theme::BG_ROW_SELECTED)
                        } else if ix % 2 == 0 {
                            theme::c(theme::BG_ROW)
                        } else {
                            theme::c(theme::BG_ROW_ALT)
                        };
                        let kind = tweet.kind();
                        let text_preview = truncate_text(&tweet.text.replace('\n', " "), 64);
                        let views = tweet.views();
                        let likes = tweet.public_metrics.like_count;
                        let bookmarks = tweet.public_metrics.bookmark_count;
                        let like_r = Tweet::format_rate(tweet.like_rate());
                        let bm_r = Tweet::format_rate(tweet.bookmark_rate());
                        let eng_r = Tweet::format_rate(tweet.engagement_rate());
                        let date = tweet.display_date();

                        items.push(
                            div()
                                .id(SharedString::from(format!("tweet-row-{id}")))
                                .flex()
                                .flex_row()
                                .items_center()
                                .h(px(40.))
                                .px_2()
                                .bg(row_bg)
                                .border_b_1()
                                .border_color(theme::c(theme::BORDER))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::c(theme::BG_HOVER)))
                                .on_click(cx.listener(move |this, _, _w, cx| {
                                    this.focus_tweet(&row_id, cx);
                                }))
                                .child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "tweet-check-{checkbox_id}"
                                        )))
                                        .w(px(COL_CHECK))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .on_click(cx.listener({
                                            let id = id.clone();
                                            move |this, _, _w, cx| {
                                                // Keep checkbox selection independent from row focus.
                                                cx.stop_propagation();
                                                this.toggle_selected(&id, cx);
                                            }
                                        }))
                                        .child(checkbox_mark(selected)),
                                )
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
                                .into_any_element(),
                        );
                    }
                    items
                }),
            )
            .flex_1()
            .h_full()
            .into_any_element()
        })
}
