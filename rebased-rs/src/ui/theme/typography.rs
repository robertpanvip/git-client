//! 排版 tokens：字号、行高、字重与语义化排版角色。
//!
//! 面板不得自行挑选 `text_xs` / `text_sm`，而应使用此处定义的**角色**：
//! 同类信息在全应用使用同一角色，避免"同一字段在不同面板字号不同"。
//!
//! 字号数值定义在 [`super::dimensions`]（`font_size_*()` 运行时函数，支持
//! 设置面板调整），此处提供**语义化别名**（`font_size_meta()` 等）和
//! **排版角色系统**（[`TextRole`] + [`text_role()`] 助手）。
//!
//! 行高同样是字号派生值：随窗口字号增量联动，保证文字放大后行距不拥挤。

use gpui::{Div, FontWeight, Styled, px};

use super::dimensions::{font_size_base, font_size_md, font_size_mono, font_size_sm};

// ---------- 字号语义化别名 ----------
/// 次要信息（时间、哈希、计数、作者、hint）—— 对应小字号（默认 12px）。
pub fn font_size_meta() -> f32 {
    font_size_sm()
}
/// 正文 / 列表行主文本（提交 subject、文件路径、菜单项）—— 对应基础字号（默认 13px）。
pub fn font_size_body() -> f32 {
    font_size_base()
}
/// 强调文本（对话框标题、面板主标题）—— 对应中等字号（默认 14px）。
pub fn font_size_title() -> f32 {
    font_size_md()
}
/// 数值/代码（等宽场景）—— 对应等宽字号（默认 12.5px）。
pub fn font_size_code() -> f32 {
    font_size_mono()
}
/// 图标角标计数（右上 9+ 徽标）—— 比 meta 再小一档（meta − 2，默认 10px），
/// 随设置字号联动。禁止在面板里写 `font_size_meta() - 2.0` 之类算术。
pub fn font_size_badge() -> f32 {
    font_size_meta() - 2.0
}

// ---------- 行高（字号派生，随窗口字号增量联动）----------
pub fn line_height_meta() -> f32 {
    font_size_meta() + 4.0
}
pub fn line_height_body() -> f32 {
    font_size_body() + 5.0
}
pub fn line_height_title() -> f32 {
    font_size_title() + 6.0
}

// ---------- 字重 ----------
pub const WEIGHT_REGULAR: FontWeight = FontWeight::NORMAL;
/// 分组标题、选中项、当前分支。
pub const WEIGHT_MEDIUM: FontWeight = FontWeight::MEDIUM;
pub const WEIGHT_BOLD: FontWeight = FontWeight::SEMIBOLD;

/// 语义化排版角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    /// 列表行主文本（提交 subject、文件路径、菜单项）。
    Body,
    /// 列表行次要文本（作者、时间、哈希、计数）。
    Meta,
    /// 分组 / 面板标题。
    SectionHeader,
    /// 对话框 / 弹层标题。
    DialogTitle,
    /// 等宽（diff 内容、blame 行号）。
    Mono,
}

impl TextRole {
    pub fn size(self) -> f32 {
        match self {
            TextRole::Body => font_size_body(),
            TextRole::Meta => font_size_meta(),
            TextRole::SectionHeader => font_size_meta(),
            TextRole::DialogTitle => font_size_title(),
            TextRole::Mono => font_size_code(),
        }
    }

    pub fn weight(self) -> FontWeight {
        match self {
            TextRole::SectionHeader | TextRole::DialogTitle => WEIGHT_MEDIUM,
            _ => WEIGHT_REGULAR,
        }
    }
}

/// 把排版角色应用到元素上。面板代码统一走此助手，不直接写 `text_xs` / `text_sm`。
pub fn text_role(role: TextRole, el: Div) -> Div {
    el.text_size(px(role.size())).font_weight(role.weight())
}
