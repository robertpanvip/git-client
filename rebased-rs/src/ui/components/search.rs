//! 搜索框原语。
//!
//! **问题背景**：原实现 `Input::new(...).appearance(false)` 会同时关闭背景、边框
//! **与焦点环**（依赖组件中焦点环的渲染条件包含 `self.appearance`），因此 Log 搜索框
//! 完全没有可见焦点态。这里默认保留外观以恢复焦点可见性。
//!
//! placeholder 由 `InputState` 持有，在此不再重复传参。

use gpui::{Styled, px};
use gpui_kit::component::{
    Icon, Sizable, Size,
    input::{Input, InputState},
};

use crate::ui::icons::Ic;
use crate::ui::theme;

/// Log / 列表过滤搜索框：内置搜索图标前缀、可一键清空、保留焦点环。
pub fn search_input(input: &gpui::Entity<InputState>) -> Input {
    Input::new(input)
        .w(px(theme::SEARCH_WIDTH))
        .with_size(Size::Small)
        .prefix(
            Icon::new(Ic::Search)
                .with_size(Size::Small)
                .text_color(theme::text_muted()),
        )
        .cleanable(true)
}

/// 撑满宽度的搜索框（命令面板 / 弹层顶部）。
pub fn palette_search(input: &gpui::Entity<InputState>) -> Input {
    Input::new(input)
        .w_full()
        .with_size(Size::Small)
        .prefix(
            Icon::new(Ic::Search)
                .with_size(Size::Small)
                .text_color(theme::text_muted()),
        )
        .cleanable(true)
}
