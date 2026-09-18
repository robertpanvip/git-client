//! 圆角 tokens（语义化别名，引用 [`super::dimensions`] 中的 `RADIUS_*` 常量）。

use super::dimensions::{RADIUS_LG, RADIUS_MD, RADIUS_SM};

/// 2px · 极小元素（chip、内联标签）—— 对应 `RADIUS_SM`。
/// 注意：IntelliJ New UI 最小圆角通常为 4px，2px 仅用于极小装饰性元素。
pub const RADIUS_XS: f32 = 2.0;
/// 4px · 列表行、输入框、菜单项 —— 对应 `RADIUS_SM` (4px)。
pub const RADIUS: f32 = RADIUS_SM;
/// 6px · 卡片、文件块、对话框 —— 对应 `RADIUS_MD` (6px)。
pub const RADIUS_CARD: f32 = RADIUS_MD;
/// 8px · 大对话框、大卡片、弹层 —— 对应 `RADIUS_LG` (8px)。
pub const RADIUS_DIALOG: f32 = RADIUS_LG;
