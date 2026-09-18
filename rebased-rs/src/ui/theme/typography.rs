//! 排版 tokens：字号、行高、字重与语义化排版角色。
//!
//! 面板不得自行挑选 `text_xs` / `text_sm`，而应使用此处定义的**角色**：
//! 同类信息在全应用使用同一角色，避免"同一字段在不同面板字号不同"。

use gpui::{Div, FontWeight, Styled, px};

// ---------- 字号 ----------
/// 次要信息（时间、哈希、计数、作者、hint）。
pub const FONT_SIZE_META: f32 = 12.0;
/// 正文 / 列表行主文本（提交 subject、文件路径、菜单项）。
pub const FONT_SIZE_BODY: f32 = 13.0;
/// 强调文本（对话框标题、面板主标题）。
pub const FONT_SIZE_TITLE: f32 = 14.0;
/// 数值/代码（等宽场景）。
pub const FONT_SIZE_MONO: f32 = 12.0;

// ---------- 行高 ----------
pub const LINE_HEIGHT_META: f32 = 16.0;
pub const LINE_HEIGHT_BODY: f32 = 18.0;
pub const LINE_HEIGHT_TITLE: f32 = 20.0;

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
            TextRole::Body => FONT_SIZE_BODY,
            TextRole::Meta => FONT_SIZE_META,
            TextRole::SectionHeader => FONT_SIZE_META,
            TextRole::DialogTitle => FONT_SIZE_TITLE,
            TextRole::Mono => FONT_SIZE_MONO,
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
