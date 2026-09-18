//! 列表行原语。
//!
//! **问题背景**：原实现同一"列表行"在不同面板有三套写法——
//! `px_2 py_0p5 rounded RADIUS hover`（Compare/History/Reflog/变更行）、
//! 无 padding/hover/圆角（Shelve 行、Conflicts 文件行）、以及 List 组件托管的行。
//! 这里把行样式收敛为一个原语，使密度、悬停、选中、圆角全应用一致。

use gpui::{Div, ElementId, Hsla, InteractiveElement, Stateful, Styled, div, px};

use crate::ui::theme;

/// 标准列表行：固定行高 + 统一内边距/圆角 + 悬停底色。
///
/// 调用方只需再 `.child(...)` 与 `.on_click(...)`。
pub fn list_row(id: impl Into<ElementId>, fg: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(theme::LIST_ROW_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::ROW_PADDING_X))
        .rounded(px(theme::RADIUS))
        .cursor_pointer()
        .hover(move |style| style.bg(theme::hover_bg(fg)))
}

/// 非交互的静态行（无 cursor / 无 hover）。
pub fn static_row(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(theme::LIST_ROW_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::ROW_PADDING_X))
}

/// 给行附加选中底色（与 List 组件的 `list_active` 保持同一语义）。
pub fn selected(row: Stateful<Div>, selected: bool, fg: Hsla) -> Stateful<Div> {
    if selected {
        row.bg(theme::selected_bg(fg))
    } else {
        row
    }
}
