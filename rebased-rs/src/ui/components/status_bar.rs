//! 状态栏原语。

use gpui::{Div, Hsla, ParentElement, SharedString, Styled, div, px};

use crate::ui::theme;

/// 状态栏容器：固定高度 + chrome 底色 + 顶部 1px 分隔。
pub fn status_bar(fg: Hsla) -> Div {
    div()
        .h(px(theme::STATUSBAR_HEIGHT))
        .flex_none()
        .bg(theme::bg_chrome())
        .border_t_1()
        .border_color(crate::ui::components::separator::chrome_divider(fg))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::SPACE_MD))
        .text_size(px(theme::FONT_SIZE_META))
}

/// 状态栏左侧消息段（占据剩余宽度）。
pub fn status_message() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
}

/// 状态栏右侧信息段。
pub fn status_segment(text: impl Into<SharedString>) -> Div {
    div().flex_none().whitespace_nowrap().child(text.into())
}
