//! 右键菜单 re-export。
//!
//! 菜单项一律通过 [`crate::ui::components::menu`] 构造，以获得统一的
//! 三栏结构（图标列 · 标签列 · 快捷键列）与危险色。

pub use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
