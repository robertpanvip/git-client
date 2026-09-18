//! 对话框原语。
//!
//! **问题背景**：原实现（`panels.rs::render_prompt_overlay`）有以下缺口——
//! 固定 `w(440.)`、**无 max-height 与滚动**、无 Enter 默认按钮、打开不聚焦输入、
//! 无内联校验（错误只写状态栏）、主按钮在输入非法时仍可点击。
//!
//! 本模块提供统一的对话框骨架，把上述约束固化在原语里，避免每个对话框各写一遍。

use gpui::{
    AnyElement, Div, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::button::{Button, ButtonVariants};

use crate::ui::theme;

/// 对话框尺寸变体。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogWidth {
    /// 标准宽度（表单类）。
    Standard,
    /// 宽（冲突/对比类）。
    Wide,
}

impl DialogWidth {
    fn px(self) -> f32 {
        match self {
            DialogWidth::Standard => theme::DIALOG_WIDTH,
            DialogWidth::Wide => theme::DIALOG_WIDTH * 1.6,
        }
    }
}

/// 标准对话框骨架：遮罩 + 卡片（标题 / 说明 / 可滚动正文 / 底部动作）。
///
/// - 正文超出 [`theme::DIALOG_MAX_HEIGHT`] 时**内部滚动**，不再被静默裁剪。
/// - 点击遮罩不关闭（模态语义，与 IntelliJ 一致），Esc 由全局 `CloseOverlay` 处理。
pub fn dialog_shell(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    body: impl IntoElement,
    footer: impl IntoElement,
) -> Div {
    dialog_shell_with_width(DialogWidth::Standard, title, description, body, footer)
}

pub fn dialog_shell_with_width(
    width: DialogWidth,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    body: impl IntoElement,
    footer: impl IntoElement,
) -> Div {
    let title: SharedString = title.into();

    let mut card = div()
        .w(px(width.px()))
        .max_h(px(theme::DIALOG_MAX_HEIGHT))
        .flex()
        .flex_col()
        .rounded(px(theme::RADIUS_LG))
        .border_1()
        .bg(cx_popover())
        .p(px(theme::DIALOG_PADDING))
        .gap(px(theme::DIALOG_GAP))
        .child(
            div()
                .flex_none()
                .text_size(px(theme::font_size_title()))
                .font_weight(theme::WEIGHT_MEDIUM)
                .child(title),
        );

    if let Some(description) = description {
        card = card.child(
            div()
                .flex_none()
                .text_size(px(theme::font_size_meta()))
                .child(description),
        );
    }

    div()
        .absolute()
        .inset_0()
        .bg(theme::overlay_bg())
        .flex()
        .items_center()
        .justify_center()
        .child(
            card.child(
                div()
                    .id("dialog-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(theme::SPACE_LG))
                    .child(body),
            )
            .child(div().flex_none().flex().flex_col().child(footer)),
        )
}

/// 底部动作区：次按钮在左、主按钮在右。
pub fn dialog_footer(cancel: AnyElement, confirm: Button) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(theme::SPACE_MD))
        .child(cancel)
        .child(confirm)
}

/// 标准「取消」按钮。
pub fn cancel_button(id: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .label(crate::ui::i18n::tr("Cancel", "取消"))
}

/// 带标签的字段（单行语义）。
pub fn dialog_field(label: impl Into<SharedString>, control: impl IntoElement, muted: Hsla) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(theme::SPACE_XS))
        .child(
            div()
                .text_size(px(theme::font_size_meta()))
                .text_color(muted)
                .child(label.into()),
        )
        .child(control)
}

/// 内联校验错误行。
#[allow(dead_code)]
pub fn dialog_error(message: Option<SharedString>) -> Option<Div> {
    message.map(|message| {
        div()
            .flex_none()
            .text_size(px(theme::font_size_meta()))
            .text_color(theme::danger_color())
            .child(message)
    })
}

/// 对话框卡片底色（无 `Context` 时取全局主题）。
fn cx_popover() -> Hsla {
    theme::popover_bg()
}
