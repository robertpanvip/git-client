//! 间距语义化别名（全部引用 [`super::dimensions`] 中的 `SPACE_*` 常量）。
//!
//! 面板内的 padding / gap 应引用此处语义化名称，而非直接使用裸数值或
//! gpui 的 `px_1` / `gap_2` 等低级刻度——后者以 Tailwind 命名，语义不明确且容易漂移。
//!
//! 实际数值定义在 [`super::dimensions`]，此处仅提供**语义化别名**，
//! 让调用点意图更清晰（如 `ROW_PADDING_X` 比 `SPACE_MD` 更表达"列表行水平内边距"）。

use super::dimensions::{SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS};

// ---------- 语义化别名（推荐在新代码中使用） ----------
/// 列表行水平内边距。
pub const ROW_PADDING_X: f32 = SPACE_MD;
/// 列表行垂直内边距。
pub const ROW_PADDING_Y: f32 = SPACE_XS;
/// 面板内容内边距。
pub const PANEL_PADDING: f32 = SPACE_MD;
/// 列表容器内边距。
pub const LIST_PADDING: f32 = SPACE_XS;
/// 列表行之间的间隙。
pub const LIST_GAP: f32 = SPACE_XS;
/// 对话框内边距。
pub const DIALOG_PADDING: f32 = SPACE_XL;
/// 对话框标题与正文之间的间距。
pub const DIALOG_GAP: f32 = SPACE_LG;
