mod blame;
mod compare;
mod conflicts;
mod dialogs;
mod diff;
mod history;
mod palette;
mod rebase;

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, input::Input};

use crate::ui::branch_tree::{BranchPick, render_branch_tree};
use crate::ui::components::empty_state;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
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

    /// Git 日志视图（图2）左栏：顶部「分支或标签」搜索框 + 分支树。
    pub(crate) fn render_branch_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let on_pick: BranchPick = {
            let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
            std::sync::Arc::new(move |branch: Option<String>, app: &mut gpui::App| {
                let _ = weak.update(app, |this, cx| this.set_branch_filter(branch, cx));
            })
        };
        let query = self.branch_query.read(cx).value().to_string();
        div()
            .w(px(theme::LOG_BRANCH_PANEL_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::bg_main())
            .border_r_1()
            .border_color(theme::separator())
            .child(
                div().flex_none().p(px(theme::SPACE_XS)).child(
                    Input::new(&self.branch_query)
                        .with_size(Size::Small)
                        .prefix(Icon::new(Ic::Search).text_color(cx.theme().muted_foreground))
                        .cleanable(true),
                ),
            )
            .child(
                div()
                    .id("branch-tree")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_branch_tree(
                        &self.state.branch_entries,
                        self.state.current_branch.as_deref(),
                        self.state.head_id.as_deref(),
                        self.state.filter_branch.as_deref(),
                        &query,
                        Some(&on_pick),
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// 工作区视图（图1）右栏：单栏只读代码预览。
    /// 选中变更文件后展示该文件的 diff（布局沿用全局「并排 / 统一」开关），
    /// 未选中时给出空态提示——对齐原版「左 = 变更 + 提交信息，右 = 预览」。
    pub(crate) fn render_preview_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let base = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::bg_main())
            .p(px(theme::SPACE_MD));
        if self.state.sidebar == SidebarMode::Diff {
            return base.child(self.render_diff_panel(cx)).into_any_element();
        }
        base.child(empty_state(
            tr("Select a changed file to preview", "选择变更文件以预览"),
            cx.theme().muted_foreground,
        ))
        .into_any_element()
    }
}
