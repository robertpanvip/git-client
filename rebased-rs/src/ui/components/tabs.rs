//! 分段控件（Tabs）原语。
//!
//! 用途：diff 的 Unified / Side-by-side 切换、Rebase 动作选择等**互斥选项**。
//! 原实现用普通按钮切换 `.primary()` / `.ghost()` 表达选中，缺少选中指示与键盘语义。

use gpui::prelude::FluentBuilder;
use gpui::{App, Div, InteractiveElement, ParentElement, SharedString, Stateful, Styled, div, px};
use gpui_kit::component::ActiveTheme;

use crate::ui::theme;

/// 一个选项：(稳定 id, 展示文本)。
pub struct TabOption {
    pub id: SharedString,
    pub label: SharedString,
}

impl TabOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// 分段控件容器（自身不处理状态）。
pub fn segmented(children: Vec<Stateful<Div>>) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_XS))
        .rounded(px(theme::RADIUS))
        .bg(theme::hover_bg(theme::text_primary()))
        .p(px(theme::SPACE_XS))
        .children(children)
}

/// 分段控件中的单个选项。
pub fn segment(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    div()
        .id(id)
        .flex_none()
        .h(px(theme::SEGMENT_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .px(px(theme::SPACE_SM))
        .rounded(px(theme::RADIUS_SM))
        .text_size(px(theme::font_size_meta()))
        .cursor_pointer()
        .text_color(if active { fg } else { muted })
        .when(active, |this| this.bg(theme::selection_bg()))
        .when(!active, |this| {
            this.hover(move |s| s.bg(theme::hover_bg(fg)))
        })
        .child(label.into())
}
