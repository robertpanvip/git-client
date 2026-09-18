//! 按钮 re-export 与语义化构造器。

use gpui::SharedString;
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
};

use crate::ui::i18n::tr;

pub use gpui_kit::component::button::DropdownButton;

/// 主按钮（Commit / 确认 / 应用）。
pub fn primary(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).primary().label(label)
}

/// 次按钮（Cancel / 普通动作）。
pub fn secondary(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label)
}

/// 幽灵按钮（工具栏 / 行内动作）。
pub fn ghost(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().compact().label(label)
}

/// 危险按钮（丢弃 / 强制推送 / 重置 --hard）。
pub fn danger(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).danger().label(label)
}

/// 标准「取消」按钮（i18n）。
pub fn cancel(id: &'static str) -> Button {
    Button::new(id).ghost().label(tr("Cancel", "取消"))
}

/// 主按钮是否可用：统一按"输入是否有效"决定，避免非法输入仍可点击。
pub fn confirm_enabled(id: &'static str, label: impl Into<SharedString>, enabled: bool) -> Button {
    Button::new(id).primary().label(label).disabled(!enabled)
}
