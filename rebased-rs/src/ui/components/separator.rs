//! 分隔线原语。
//!
//! 原实现有两套分隔线来源但**没有语义命名**，看起来像重复实现：
//! - 面板之间用 `theme::separator()`（#26282C）
//! - 工具栏/状态栏内部用 `border_color(fg)`（fg@12%）
//!
//! 二者其实服务不同背景：`#26282C` 画在 chrome 底色上会**完全不可见**。
//! 因此这里把两者显式命名为两个角色，而不是强行合并。

use gpui::{Div, Hsla, Styled, div, px};

use crate::ui::theme;

/// 主背景之上的一体化分隔线（面板边界）。
pub fn panel_divider() -> Hsla {
    theme::separator()
}

/// chrome 底色（工具栏 / 状态栏）之上的分隔线，需要比底色更亮。
pub fn chrome_divider(fg: Hsla) -> Hsla {
    theme::border_color(fg)
}

/// 工具栏内的竖直分隔线。
pub fn v_separator(fg: Hsla) -> Div {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(theme::SPACE_LG))
        .bg(chrome_divider(fg))
}

/// 分组之间的水平分隔线（跟随 chrome 角色）。
pub fn h_separator(fg: Hsla) -> Div {
    div().flex_none().h(px(1.)).w_full().bg(chrome_divider(fg))
}
