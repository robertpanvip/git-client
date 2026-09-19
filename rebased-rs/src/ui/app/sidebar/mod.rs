mod blame;
mod compare;
mod conflicts;
mod dialogs;
mod diff;
mod history;
mod palette;
mod rebase;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px, transparent_black,
};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, input::Input};

use crate::ui::branch_tree::{BranchPick, render_branch_tree};
use crate::ui::components::empty_state;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

use super::{AppView, DiffSource, SidebarMode};

impl AppView {
    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let width = self.right_panel_width;
        let base = div()
            .w(px(width))
            .h_full()
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
        let mut base = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::bg_main())
            .p(px(theme::SPACE_MD));
        if self.state.sidebar == SidebarMode::Diff {
            // 提交区双击打开的「提交:文件」Tab 条在 diff 面板之上（IDEA 编辑器 Tab 位）。
            if !self.state.commit_diff_tabs.is_empty() {
                base = base.child(self.render_commit_diff_tab_bar(cx));
            }
            return base.child(self.render_diff_panel(cx)).into_any_element();
        }
        base.child(empty_state(
            tr("Select a changed file to preview", "选择变更文件以预览"),
            cx.theme().muted_foreground,
        ))
        .into_any_element()
    }

    /// 提交区 diff Tab 条（IDEA Commit 工具窗右侧的编辑器 Tab）：
    /// diff 图标 + 「提交:文件名」标题 + 关闭 ×，active 底部下划线。
    /// Tab 以 (路径, 暂存态) 区分（同一文件可同时有已暂存/未暂存两个 Tab），
    /// 故 id 用序号而非路径，避免重复 ElementId。
    fn render_commit_diff_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let active = match self.state.diff_source {
            Some(DiffSource::Staged) => self.state.diff_path.clone().map(|path| (path, true)),
            Some(DiffSource::Unstaged) => self.state.diff_path.clone().map(|path| (path, false)),
            _ => None,
        };
        let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
        let mut bar = div()
            .id("commit-diff-tab-bar")
            .flex_none()
            .h(px(theme::FILE_TAB_BAR_HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_stretch()
            .overflow_x_scroll()
            .border_b_1()
            .border_color(theme::separator());
        for (index, (path, staged)) in self.state.commit_diff_tabs.iter().enumerate() {
            let is_active = active
                .as_ref()
                .is_some_and(|(active_path, active_staged)| {
                    active_path == path && *active_staged == *staged
                });
            let name = path.rsplit('/').next().unwrap_or(path);
            let tab_id = format!("dtab-{index}");
            let close_id = format!("dtab-close-{index}");
            let activate_weak = weak.clone();
            let close_weak = weak.clone();
            let activate_path = path.clone();
            let activate_staged = *staged;
            let close_index = index;
            bar = bar.child(
                div()
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
                    .when(!is_active, |tab| {
                        tab.hover(move |s| s.bg(theme::hover_bg(fg)))
                    })
                    .child(Icon::new(Ic::Diff).with_size(Size::XSmall))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(format!("{}:{name}", tr("Commit", "提交"))),
                    )
                    .on_click(move |_, _, app| {
                        let _ = activate_weak.update(app, |this, cx| {
                            let index = this
                                .state
                                .commit_diff_tabs
                                .iter()
                                .position(|tab| tab.0 == activate_path && tab.1 == activate_staged);
                            if let Some(index) = index {
                                this.activate_commit_diff_tab(index, cx);
                            }
                        });
                    })
                    // 关闭 × 必须用 block_mouse_except_scroll 隔离事件冒泡，
                    // 否则 × 的 click 会继续冒泡到 Tab 本体触发激活。
                    .child(
                        div()
                            .id(close_id)
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
                            .on_click(move |_, _, app| {
                                let _ = close_weak.update(app, |this, cx| {
                                    this.close_commit_diff_tab(close_index, cx);
                                });
                            }),
                    ),
            );
        }
        bar
    }
}
