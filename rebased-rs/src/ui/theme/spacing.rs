//! 间距 tokens：4px 网格。
//!
//! 面板内的 padding / gap 应引用此处常量，而非直接使用 gpui 的
//! `px_1` / `gap_2` 等语义刻度——后者以 Tailwind 命名，语义不明确且容易漂移。

/// 2px · 紧贴元素之间（图标与文字、徽章内部）。
pub const SPACE_XS: f32 = 2.0;
/// 4px · 行内元素间距。
pub const SPACE_SM: f32 = 4.0;
/// 8px · 行内边距 / 元素间距（默认值）。
pub const SPACE_MD: f32 = 8.0;
/// 12px · 区块间距。
pub const SPACE_LG: f32 = 12.0;
/// 16px · 面板内边距 / 对话框内边距。
pub const SPACE_XL: f32 = 16.0;

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
