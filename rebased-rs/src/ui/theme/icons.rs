//! 图标尺寸 tokens。
//!
//! gpui 组件库的 `Size` 枚举（XSmall/Small/Medium）映射到固定像素，
//! 面板应使用此处常量，避免同一层级图标在不同面板尺寸不一。

/// 12px · 行内极小图标（菜单图标列、徽章前缀、状态点）。
pub const ICON_SIZE_XS: f32 = 12.0;
/// 14px · 列表行内图标（行尾操作按钮）。
pub const ICON_SIZE_SM: f32 = 14.0;
/// 16px · 工具栏 / 图标条图标。
pub const ICON_SIZE_MD: f32 = 16.0;
/// 20px · 空态 / 大按钮图标。
pub const ICON_SIZE_LG: f32 = 20.0;
