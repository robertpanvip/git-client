//! 标题行原语。
//!
//! **问题背景**：原实现有两套面板标题写法——自绘 `text_sm` muted 行（Diff/Blame/
//! History/Reflog/Compare）与 `group_header` 的 22px `text_xs` 行（Rebase/Conflicts/
//! Shelve）。这里统一为 [`section_header`] / [`panel_header`] 两个原语。

use gpui::{AnyElement, Div, Hsla, ParentElement, SharedString, Styled, div, px};

use crate::ui::theme;

/// 列表内分组标题（Changes / Staged / Unstaged / Branches…）。
pub fn group_header(label: impl Into<SharedString>, muted: Hsla) -> Div {
    group_header_controls(label, muted, None, None)
}

/// 带前导控件（如"全选"复选框）与尾部计数（如 `12`）的分组标题。
///
/// 三者的字号/字重/行高与 [`group_header`] 完全一致，只是多了可选槽位，
/// 避免各面板自行拼装标题行导致密度与内边距漂移。
pub fn group_header_controls(
    label: impl Into<SharedString>,
    muted: Hsla,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
) -> Div {
    let mut header = div()
        .flex_none()
        .h(px(theme::SECTION_HEADER_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_SM))
        .px(px(theme::ROW_PADDING_X))
        .text_size(px(theme::font_size_meta()))
        .font_weight(theme::WEIGHT_MEDIUM)
        .text_color(muted);
    if let Some(leading) = leading {
        header = header.child(leading);
    }
    header = header.child(
        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(label.into()),
    );
    if let Some(trailing) = trailing {
        header = header.child(trailing);
    }
    header
}

/// 面板标题（侧栏各面板顶部）。
///
/// 与分组标题同字号/字重（12px Medium），差别在于带右侧动作与标题省略。
/// 原实现此处为 `text_sm`（14px），与分组标题 12px 并存 → 全应用统一为 12px。
pub fn panel_header(title: impl Into<SharedString>, muted: Hsla, actions: Vec<AnyElement>) -> Div {
    let mut header = div()
        .flex_none()
        .h(px(theme::SECTION_HEADER_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .text_size(px(theme::font_size_meta()))
        .font_weight(theme::WEIGHT_MEDIUM)
        .text_color(muted)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(title.into()),
        );
    for action in actions {
        header = header.child(action);
    }
    header
}
