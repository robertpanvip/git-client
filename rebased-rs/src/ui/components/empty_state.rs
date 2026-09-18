//! 空态原语。
//!
//! **问题背景**：原实现有两套空态——居中的 `empty_state()`（Diff/Blame/History/
//! Reflog）与左对齐裸文本（Rebase/Conflicts/Shelve/Compare）。统一用本模块。

use gpui::{Div, Hsla, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::{Icon, Sizable, Size};

use crate::ui::icons::Ic;
use crate::ui::theme;

/// 居中空态（仅一行文案）。
pub fn empty_state(message: impl Into<SharedString>, muted: Hsla) -> Div {
    empty_state_with(None, message, None, muted)
}

/// 居中空态（图标 + 标题 + 可选次要说明）。
pub fn empty_state_with(
    icon: Option<Ic>,
    title: impl Into<SharedString>,
    hint: Option<&str>,
    muted: Hsla,
) -> Div {
    let mut body = div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::SPACE_XL))
        .text_size(px(theme::FONT_SIZE_BODY))
        .text_color(muted);
    if let Some(icon) = icon {
        body = body.child(
            Icon::new(icon)
                .with_size(Size::Medium)
                .text_color(muted.opacity(0.7)),
        );
    }
    body = body.child(div().text_center().child(title.into()));
    if let Some(hint) = hint {
        body = body.child(
            div()
                .text_size(px(theme::FONT_SIZE_META))
                .text_center()
                .text_color(muted.opacity(0.75))
                .child(hint.to_string()),
        );
    }
    body
}

/// 面板内联空态提示（非居中，用于面板正文顶部）。
pub fn empty_hint(message: impl Into<SharedString>, muted: Hsla) -> Div {
    div()
        .px(px(theme::SPACE_MD))
        .py(px(theme::SPACE_SM))
        .text_size(px(theme::FONT_SIZE_META))
        .text_color(muted)
        .child(message.into())
}
