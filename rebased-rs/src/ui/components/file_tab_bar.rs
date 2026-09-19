//! 文件 Tab 条（编辑器顶部）：IDEA 编辑器 Tab 的对应原语。
//!
//! - 横向可滚动：Tab 多时不换行、不压缩，超出部分滚动查看；
//! - 每 Tab = 文件类型图标 + 文件名 + 关闭 ×；
//! - active Tab：文字高亮 + 底部 accent 下划线（IDEA New UI 样式）；
//! - 点击 Tab 本体激活（调用方接 `open_file` 语义，已打开文件不重建），
//!   点击 × 关闭（邻位选中逻辑在调用方，见 [`neighbor_after_close`]）。
//!
//! 本组件不持有状态：打开顺序与 active 由调用方的 `open_tabs` /
//! `files_selected` 单一来源提供。

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Div, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement, Styled,
    div, px, transparent_black,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::{Icon, Sizable, Size};

use crate::ui::icons::Ic;
use crate::ui::theme;

/// 激活一个 Tab（点击 Tab 本体）。
pub type TabActivate = Arc<dyn Fn(String, &mut App)>;
/// 关闭一个 Tab（点击 ×）。
pub type TabClose = Arc<dyn Fn(String, &mut App)>;

/// 关闭 `pos` 处 Tab 后应激活的索引（针对**已移除后**的列表）：
/// IDEA 语义 = 优先右邻、无右邻取左邻；列表已空则回到空态（None）。
pub fn neighbor_after_close(len_after: usize, pos: usize) -> Option<usize> {
    if len_after == 0 {
        None
    } else {
        Some(pos.min(len_after - 1))
    }
}

/// 渲染文件 Tab 条。`paths` = 打开顺序（左→右），`active` = 当前 Tab 路径。
pub fn file_tab_bar(
    paths: &[String],
    active: Option<&str>,
    on_activate: &TabActivate,
    on_close: &TabClose,
    cx: &App,
) -> Stateful<Div> {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let mut bar = div()
        .id("file-tab-bar")
        .flex_none()
        .h(px(theme::FILE_TAB_BAR_HEIGHT))
        .w_full()
        .flex()
        .flex_row()
        .items_stretch()
        .overflow_x_scroll()
        .border_b_1()
        .border_color(theme::separator())
        .bg(theme::bg_main());
    for path in paths {
        // shadow 成 owned：on_click 闭包要求 'static，&T.clone() 会借出引用。
        let path = path.clone();
        let is_active = active == Some(path.as_str());
        let name = path.rsplit('/').next().unwrap_or(&path);
        let activate = Arc::clone(on_activate);
        let close = Arc::clone(on_close);
        let close_path = path.clone();
        let tab_id = format!("ftab-{}", path);
        let close_id = format!("ftab-close-{}", path);
        let tab = div()
            .id(tab_id)
            .flex_none()
            .h_full()
            .max_w(px(theme::FILE_TAB_MAX_WIDTH))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_SM))
            .px(px(theme::SPACE_MD))
            .text_size(px(theme::font_size_meta()))
            .cursor_pointer()
            .overflow_hidden()
            // 底部下划线槽位：非 active 用透明占位，避免选中时高度跳动。
            .border_b_2()
            .border_color(if is_active {
                theme::focus_ring()
            } else {
                transparent_black()
            })
            .text_color(if is_active { fg } else { muted })
            .when(!is_active, |tab| tab.hover(move |s| s.bg(theme::hover_bg(fg))))
            .child(crate::ui::icons::file_type_icon(&path))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(name.to_string()),
            )
            .on_click(move |_, _, app| activate(path.clone(), app))
            .child(close_button(close_id, muted, fg, move |app| {
                close(close_path.clone(), app)
            }));
        bar = bar.child(tab);
    }
    bar
}

/// Tab 内的关闭 ×。GPUI 的事件按命中 hitbox 全链路派发（子→父冒泡），
/// 必须用 block_mouse_except_scroll 隔离，否则 × 的 click 会继续冒泡到
/// Tab 本体触发 activate，把刚关闭的文件重新打开。
fn close_button(
    id: String,
    muted: gpui::Hsla,
    fg: gpui::Hsla,
    on_click: impl Fn(&mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .block_mouse_except_scroll()
        .flex_none()
        .h(px(theme::ICON_BUTTON_SIZE))
        .w(px(theme::ICON_BUTTON_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::RADIUS_SM))
        .cursor_pointer()
        .text_color(muted)
        .hover(move |s| s.bg(theme::hover_bg(fg)).text_color(fg))
        .child(Icon::new(Ic::Close).with_size(Size::XSmall))
        .on_click(move |_, _, app| on_click(app))
}

#[cfg(test)]
mod tests {
    use super::neighbor_after_close;

    #[test]
    fn empty_list_returns_none() {
        assert_eq!(neighbor_after_close(0, 0), None);
    }

    #[test]
    fn prefers_right_neighbor() {
        // 关闭第 0 个后剩 2 个 → 激活原右邻（新列表的第 0 个）。
        assert_eq!(neighbor_after_close(2, 0), Some(0));
    }

    #[test]
    fn falls_back_to_left_neighbor() {
        // 关闭最后一个后 → 左邻。
        assert_eq!(neighbor_after_close(2, 2), Some(1));
        assert_eq!(neighbor_after_close(1, 1), Some(0));
    }
}
