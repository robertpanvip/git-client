//! rebased 风格基础组件：徽章、分隔线、分组标题、空态。
//! 只做视觉标准化，业务交互（菜单/动作）留在各 panel。

use gpui::{Div, Hsla, InteractiveElement, ParentElement, SharedString, Stateful, Styled, div, px};

pub use gpui_kit::component::checkbox::Checkbox;

use crate::ui::theme;

/// git ref 名称 → (显示文本, 语义色)。
pub fn ref_style(name: &str) -> (String, Hsla) {
    if let Some(tag) = name.strip_prefix("tag: ") {
        (tag.to_string(), theme::tag_color())
    } else if let Some(branch) = name.strip_prefix("HEAD -> ") {
        (branch.to_string(), theme::head_color())
    } else if name == "HEAD" {
        ("HEAD".to_string(), theme::head_color())
    } else {
        (name.to_string(), theme::remote_color())
    }
}

/// 圆角小徽章（分支/标签/状态）。
pub fn badge(id: SharedString, label: impl Into<SharedString>, color: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px_1()
        .rounded(px(theme::RADIUS))
        .bg(theme::badge_bg(color))
        .text_xs()
        .text_color(color)
        .child(label.into())
}

/// 工具栏竖分隔线。
pub fn v_separator(fg: Hsla) -> Div {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(theme::SPACE_LG))
        .bg(theme::border_color(fg))
}

/// 面板分组标题行（如 Changes / Branches）。
pub fn group_header(label: impl Into<SharedString>, muted: Hsla) -> Div {
    div()
        .flex_none()
        .h(px(theme::GROUP_HEADER_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .px_2()
        .text_xs()
        .text_color(muted)
        .child(label.into())
}

/// 居中空态提示。
pub fn empty_state(message: impl Into<SharedString>, muted: Hsla) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(muted)
        .child(message.into())
}
