//! 图标按钮原语。
//!
//! 统一命中区为 [`theme::ICON_BUTTON_SIZE`]（24×24），原实现图标条按钮为 32×32、
//! 组件默认图标按钮为 32×32，密度偏松。

use gpui::{InteractiveElement, ParentElement, SharedString, Styled, px};
use gpui_kit::component::button::{Button, ButtonVariants};

use crate::ui::icons::Ic;
use crate::ui::theme;

/// 纯图标按钮（无文字）。
pub fn icon_button(id: impl Into<gpui::ElementId>, icon: Ic) -> Button {
    Button::new(id)
        .ghost()
        .compact()
        .icon(icon)
        .h(px(theme::ICON_BUTTON_SIZE))
}

/// 带文字图标按钮。
pub fn labeled_icon_button(
    id: impl Into<gpui::ElementId>,
    icon: Ic,
    label: impl Into<SharedString>,
) -> Button {
    Button::new(id).ghost().compact().icon(icon).label(label)
}

/// 图标条（最左侧工具窗口竖条）按钮：正方形、选中底色、悬停淡入。
pub fn strip_button(
    id: impl Into<gpui::ElementId>,
    icon: Ic,
    active: bool,
    hover_fg: gpui::Hsla,
) -> impl gpui::IntoElement {
    let fg = if active {
        theme::white()
    } else {
        theme::text_muted()
    };
    gpui::div()
        .id(id)
        .size(px(theme::ICON_STRIP_BUTTON_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::RADIUS))
        .text_color(fg)
        .cursor_pointer()
        .when_else(
            active,
            |b| b.bg(theme::selection_bg()),
            |b| b.hover(move |s| s.bg(theme::hover_bg(hover_fg))),
        )
        .child(gpui_kit::component::Icon::new(icon))
}

/// `when/else` 便捷 trait（gpui 的 `when` 无 else 分支）。
trait WhenElse: Sized {
    fn when_else(
        self,
        cond: bool,
        yes: impl FnOnce(Self) -> Self,
        no: impl FnOnce(Self) -> Self,
    ) -> Self {
        if cond { yes(self) } else { no(self) }
    }
}

impl<T> WhenElse for T {}
