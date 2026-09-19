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
        .text_size(px(theme::font_size_body()))
        .text_color(muted);
    if let Some(icon) = icon {
        body = body.child(
            Icon::new(icon)
                .with_size(Size::Medium)
                .text_color(theme::empty_faint(muted)),
        );
    }
    body = body.child(div().text_center().child(title.into()));
    if let Some(hint) = hint {
        body = body.child(
            div()
                .text_size(px(theme::font_size_meta()))
                .text_center()
                .text_color(theme::empty_faint(muted))
                .child(hint.to_string()),
        );
    }
    body
}

/// 居中错误态：与 [`empty_state`] 同构（同一布局 / 字号 / 间距 token），
/// 标题用错误色，可选灰色详情（原始错误串）。
/// 操作按钮（重试 / 重新打开等）由调用方以 `.child()` 追加，保持单一原语。
pub fn error_state(title: impl Into<SharedString>, detail: Option<SharedString>) -> Div {
    let mut body = div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::SPACE_XL))
        .text_center()
        .text_size(px(theme::font_size_body()))
        .text_color(theme::error_color())
        .child(title.into());
    if let Some(detail) = detail {
        body = body.child(
            div()
                .max_w(px(theme::MESSAGE_MAX_WIDTH))
                .text_center()
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(theme::font_size_meta()))
                .text_color(theme::text_disabled())
                .child(detail),
        );
    }
    body
}
