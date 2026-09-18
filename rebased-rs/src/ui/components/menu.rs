//! 菜单原语：统一菜单项的三栏结构（图标列 · 标签列 · 快捷键列）与危险色。
//!
//! **问题背景**：原实现全部使用 `PopupMenuItem::new(...).on_click(...)`，因此
//! - 依赖组件只在用 `.action()` 注册时才渲染快捷键列 → 本项目**从未出现快捷键列**；
//! - 无任何菜单项调用 `.icon()` → 除 `.checked()` 被动触发的勾选列外**没有图标列**；
//! - 破坏性操作（Delete / Drop / Revert / Force Push / Discard）与普通项同样式，**没有危险色**。
//!
//! 这里用 `PopupMenuItem::element()` 自定义渲染，在**不引入 Action 体系**的前提下
//! 补齐三栏结构与危险色，保持既有 `on_click` 回调风格不变。

use gpui::{
    App, ClickEvent, Div, ElementId, Hsla, InteractiveElement, ParentElement, SharedString,
    Stateful, Styled, Window, div, px,
};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    menu::{PopupMenu, PopupMenuItem},
};

use crate::ui::icons::Ic;
use crate::ui::theme;

/// 标准菜单项：图标列 + 标签列 + 快捷键列（可选）+ 危险色（可选）+ 勾选态（可选）。
///
/// - `shortcut`：展示用快捷键文本。应从 [`crate::ui::components::shortcuts`] 取，
///   避免与 `KeyBinding` 漂移。
/// - `danger`：破坏性/数据丢失操作为 `true`，文字着危险色。
/// - `checked`：左侧图标列替换为勾选标记（用于"当前分支"等状态项）。
pub fn menu_item<F>(
    icon: Ic,
    label: impl Into<SharedString>,
    shortcut: Option<&str>,
    danger: bool,
    checked: bool,
    on_click: F,
) -> PopupMenuItem
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let label: SharedString = label.into();
    let shortcut: Option<SharedString> = shortcut.map(SharedString::from);

    PopupMenuItem::element(move |_window, cx| {
        let fg = if danger {
            theme::danger_color()
        } else {
            cx.theme().foreground
        };
        let accent = if danger {
            theme::danger_color().opacity(0.75)
        } else {
            cx.theme().muted_foreground
        };

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .gap(px(theme::SPACE_LG))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::FONT_SIZE_BODY))
                    .text_color(fg)
                    .child(label.clone()),
            );

        if let Some(shortcut) = shortcut.clone() {
            row = row.child(
                div()
                    .flex_none()
                    .text_size(px(theme::FONT_SIZE_META))
                    .text_color(accent)
                    .child(shortcut),
            );
        }

        row
    })
    .icon(icon)
    .checked(checked)
    .on_click(on_click)
}

/// 无图标的自定义渲染项（用于 `name → url` 这类展示行）。
pub fn menu_text(label: impl Into<SharedString>, muted: impl Into<SharedString>) -> PopupMenuItem {
    let label: SharedString = label.into();
    let muted: SharedString = muted.into();
    PopupMenuItem::element(move |_window, cx| {
        h_flex()
            .w_full()
            .min_w_0()
            .gap(px(theme::SPACE_MD))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::FONT_SIZE_BODY))
                    .child(label.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(theme::FONT_SIZE_META))
                    .text_color(cx.theme().muted_foreground)
                    .child(muted.clone()),
            )
    })
}

/// 菜单分组标题（不可点击）。
pub fn menu_section(label: impl Into<SharedString>) -> PopupMenuItem {
    PopupMenuItem::label(label)
}

/// 菜单分隔线。
pub fn menu_separator() -> PopupMenuItem {
    PopupMenuItem::separator()
}

/// 统一菜单容器宽度，避免过窄导致快捷键列换行。
pub fn menu_width(menu: PopupMenu) -> PopupMenu {
    menu.min_w(px(theme::MENU_MIN_WIDTH))
        .max_w(px(theme::MENU_MAX_WIDTH))
}

/// 非弹出式的菜单行（命令面板 / VCS 快切弹层等自绘列表）。
///
/// 与 [`menu_item`] 共用同一套三栏结构、行高与内边距——面板内的"动作列表"
/// 与右键菜单因此看起来是同一个控件，而不是两套视觉。
pub fn menu_row(
    id: impl Into<ElementId>,
    fg: Hsla,
    icon: Ic,
    label: impl Into<SharedString>,
    shortcut: Option<&str>,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let mut row = div()
        .id(id)
        .w_full()
        .h(px(theme::MENU_ROW_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .px(px(theme::SPACE_SM))
        .rounded(px(theme::RADIUS))
        .cursor_pointer()
        .hover(move |style| style.bg(theme::hover_bg(fg)))
        // 图标列：固定宽度，与弹出菜单的图标列对齐。
        .child(
            div()
                .flex_none()
                .w(px(theme::ICON_SIZE_MD))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).with_size(Size::Small)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(theme::FONT_SIZE_BODY))
                .child(label),
        );
    if let Some(shortcut) = shortcut {
        row = row.child(
            div()
                .flex_none()
                .text_size(px(theme::FONT_SIZE_META))
                .text_color(theme::text_muted())
                .child(SharedString::from(shortcut)),
        );
    }
    row
}

/// `h_flex` 别名：`PopupMenuItem::element` 的闭包内使用，避免重复导入。
fn h_flex() -> gpui::Div {
    div().flex().flex_row().items_center()
}
