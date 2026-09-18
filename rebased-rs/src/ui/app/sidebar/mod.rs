mod blame;
mod compare;
mod conflicts;
mod dialogs;
mod diff;
mod history;
mod palette;
mod rebase;
mod shelves;

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_kit::component::ActiveTheme;

use crate::ui::components::empty_state;
use crate::ui::i18n::tr;
use crate::ui::theme;

use super::{AppView, SidebarMode};

impl AppView {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let width = self.right_panel_width;
        let base = div()
            .w(px(width))
            .flex_none()
            .p(px(theme::SPACE_MD))
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .overflow_hidden();

        match self.state.sidebar {
            SidebarMode::Diff => base.child(self.render_diff_panel(cx)).into_any_element(),
            SidebarMode::Blame => base.child(self.render_blame_panel(cx)).into_any_element(),
            SidebarMode::Compare => base.child(self.render_compare_panel(cx)).into_any_element(),
            SidebarMode::Rebase => base.child(self.render_rebase_panel(cx)).into_any_element(),
            SidebarMode::Conflicts => base
                .child(self.render_conflicts_panel(cx))
                .into_any_element(),
            SidebarMode::Shelve => base.child(self.render_shelve_panel(cx)).into_any_element(),
            SidebarMode::History => base.child(self.render_history_panel(cx)).into_any_element(),
            SidebarMode::Reflog => base.child(self.render_reflog_panel(cx)).into_any_element(),
            // 工作区变更列表已固定在左侧 Commit 面板，右侧统一承载提交详情。
            SidebarMode::Workspace | SidebarMode::Detail => match &self.state.selected {
                Some(commit) => base
                    .child(self.render_detail(commit, cx))
                    .into_any_element(),
                None => base
                    .child(empty_state(
                        tr("Select a commit to see details", "选择提交以查看详情"),
                        cx.theme().muted_foreground,
                    ))
                    .into_any_element(),
            },
        }
    }
}
