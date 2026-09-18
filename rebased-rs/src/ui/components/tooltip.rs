//! 提示（Tooltip）原语。
//!
//! 原实现中部分行内图标按钮带 tooltip、部分不带。此处统一构造器，避免遗漏。
//! 依赖组件的 `Button::tooltip` 接受 `impl Into<SharedString>`。

use gpui::{SharedString, Styled, px};
use gpui_kit::component::button::{Button, ButtonVariants};

use crate::ui::theme;

/// 给按钮附加提示文本。
pub fn with_tooltip(button: Button, text: impl Into<SharedString>) -> Button {
    button.tooltip(text)
}

/// 行内图标按钮 + 提示（列表行尾动作的统一形态）。
pub fn row_icon_button(
    id: impl Into<gpui::ElementId>,
    icon: crate::ui::icons::Ic,
    tooltip: impl Into<SharedString>,
) -> Button {
    Button::new(id)
        .ghost()
        .compact()
        .h(px(theme::ICON_BUTTON_SIZE))
        .icon(icon)
        .tooltip(tooltip)
}
