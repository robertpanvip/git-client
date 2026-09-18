//! 输入控件 re-export 与语义化包装。
//!
//! 关键约束：**单行语义的字段必须用单行控件**。原实现把 NewBranch / Rename / GoTo /
//! SetUpstream / EditTag 等单行字段统一用 64px 多行 `Textarea`，
//! 导致 Enter 被解释为换行而非"确认"，与 IntelliJ 的对话框行为相反。
//!
//! 注意：placeholder 属于**状态**（`InputState` / `TextareaState` 构造时设置），
//! 不是元素上的方法。

use gpui::px;
use gpui_kit::component::input::{Input, Textarea};

use crate::ui::theme;

pub use gpui_kit::component::input::{InputState, TextareaState};

/// 单行输入（Branch 名 / Tag 名 / upstream / 修订号 等）。
pub fn text_field(input: &gpui::Entity<InputState>) -> Input {
    Input::new(input).h(px(theme::INPUT_HEIGHT_SINGLE))
}

/// 多行输入（提交信息 / squash 合并信息等允许换行的场景）。
pub fn text_area(input: &gpui::Entity<TextareaState>) -> Textarea {
    Textarea::new(input).h(px(theme::INPUT_HEIGHT_MULTI))
}

/// 紧凑高度的多行控件（需要与单行字段对齐时）。
pub fn compact_area(input: &gpui::Entity<TextareaState>) -> Textarea {
    Textarea::new(input).h(px(theme::INPUT_HEIGHT_SINGLE))
}
