//! 工具栏原语。

use gpui::{Div, Hsla, ParentElement, SharedString, Styled, div, px};

use crate::ui::theme;

/// 顶部工具栏容器：固定高度 + chrome 底色 + 底部 1px 分隔。
pub fn toolbar(fg: Hsla) -> Div {
    div()
        .h(px(theme::TOOLBAR_HEIGHT))
        .flex_none()
        .bg(theme::bg_chrome())
        .border_b_1()
        .border_color(crate::ui::components::separator::chrome_divider(fg))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_SM))
        .px(px(theme::SPACE_MD))
}

/// 工具栏内的一组动作（组间用 `separator::v_separator` 分隔）。
pub fn toolbar_group() -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_XS))
}

/// 工具栏中的只读标签（仓库名 / 分支等）。
pub fn toolbar_label(text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_size(px(theme::FONT_SIZE_META))
        .child(text.into())
}
