//! 弹层原语（命令面板 / 自定义浮层）。

use gpui::{Div, ParentElement, SharedString, Styled, div, px};

use crate::ui::theme;

/// 全屏遮罩 + 顶部对齐的浮层容器。
pub fn overlay_top(top: f32) -> Div {
    div()
        .absolute()
        .inset_0()
        .bg(theme::overlay_bg())
        .flex()
        .items_start()
        .justify_center()
        .pt(px(top))
}

/// 居中的模态遮罩。
pub fn overlay_center() -> Div {
    div()
        .absolute()
        .inset_0()
        .bg(theme::overlay_bg())
        .flex()
        .items_center()
        .justify_center()
}

/// 浮层卡片：固定宽度 + 最大高度（内部滚动由调用方决定）。
pub fn popup_card(width: f32) -> Div {
    div()
        .w(px(width))
        .max_h(px(theme::PALETTE_MAX_HEIGHT))
        .flex()
        .flex_col()
        .rounded(px(theme::RADIUS_LG))
        .border_1()
        .border_color(theme::border_color(theme::text_primary()))
        .bg(theme::popover_bg())
        .overflow_hidden()
}

/// 浮层顶部标题区。
pub fn popup_title(title: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .h(px(theme::SECTION_HEADER_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::SPACE_MD))
        .text_size(px(theme::FONT_SIZE_META))
        .font_weight(theme::WEIGHT_MEDIUM)
        .text_color(theme::text_muted())
        .child(title.into())
}
