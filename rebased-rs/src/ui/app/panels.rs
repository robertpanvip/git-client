use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Div, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Textarea,
};

use rebased_rs::git::{HunkChoice, ResetMode};

use crate::ui::blame_view::{BlameJump, render_blame};
use crate::ui::components::{empty_state, group_header};
use crate::ui::diff_view::{HunkAction, render_diff_files};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

use super::actions::FocusComposer;
use super::{AppView, DiffSource, SidebarMode};

/// 拖拽 payload：被拖动的 rebase 计划行下标。
#[derive(Clone, Copy)]
pub(crate) struct RebaseDrag(pub(crate) usize);

/// 拖拽时跟随鼠标的预览视图。
struct RebaseDragPreview(SharedString);

impl Render for RebaseDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_0p5()
            .rounded_sm()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_xs()
            .shadow_md()
            .child(self.0.clone())
    }
}

impl AppView {
    pub(crate) fn render_rebase_panel(&self, cx: &mut Context<Self>) -> Div {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let Some((base, plan)) = self.state.rebase.plan_view() else {
            return div().size_full();
        };
        let short_base = base[..base.len().min(7)].to_string();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .child(group_header(tr("Interactive Rebase", "交互式变基"), muted))
            .child(
                div()
                    .flex_none()
                    .px_2()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("{} {short_base}…", tr("onto", "变基到"))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(tr(
                        "Click the action to cycle Pick → Squash → Fixup → Drop → Edit → Reword. Drag rows or use ↑ ↓ to reorder.",
                        "点击操作循环切换 挑选 → 压缩 → 修整 → 丢弃 → 编辑 → 改写。拖动行或用 ↑ ↓ 调整顺序。",
                    )),
            );

        if plan.is_empty() {
            panel = panel.child(div().text_xs().text_color(muted).child(tr(
                "No commits between base and HEAD.",
                "基点与 HEAD 之间没有提交。",
            )));
        }

        for (index, action) in plan.iter().enumerate() {
            let kind_label = action.kind.label();
            let short = &action.id[..action.id.len().min(7)];
            // 已自定义消息时给出提示，并展示自定义内容而非原标题。
            let summary = match action.message.as_deref() {
                Some(m) if !m.trim().is_empty() => format!("{short} ✎ {m}"),
                _ => format!("{short} {}", action.subject),
            };
            // on_drag 的 constructor 是 Fn，可能被多次调用，label 按次克隆。
            let drag_label: SharedString = format!("{} {short}", tr("Move", "移动")).into();
            panel = panel.child(
                div()
                    .id(("rebase-row", index))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(border)
                    .pb_1()
                    .cursor_move()
                    .drag_over::<RebaseDrag>(|style, _, _, _| {
                        style.border_color(theme::success_color())
                    })
                    .on_drag(RebaseDrag(index), move |_, _, _, cx| {
                        cx.new(|_| RebaseDragPreview(drag_label.clone()))
                    })
                    .on_drop(cx.listener(move |this, drag: &RebaseDrag, _, cx| {
                        this.move_rebase_action_to(drag.0, index, cx)
                    }))
                    .child(
                        Button::new(("rebase-kind", index))
                            .ghost()
                            .compact()
                            .label(kind_label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cycle_rebase_action(index, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_ellipsis()
                            .overflow_hidden()
                            .child(summary),
                    )
                    .child(
                        Button::new(("rebase-edit", index))
                            .ghost()
                            .compact()
                            .icon(Ic::Edit)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_rebase_edit(index, window, cx)
                            })),
                    )
                    .child(
                        Button::new(("rebase-up", index))
                            .ghost()
                            .compact()
                            .label("↑")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_rebase_action(index, -1, cx)
                            })),
                    )
                    .child(
                        Button::new(("rebase-down", index))
                            .ghost()
                            .compact()
                            .label("↓")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_rebase_action(index, 1, cx)
                            })),
                    ),
            );
        }

        panel.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .mt_1()
                .child(
                    Checkbox::new("rebase-autosquash")
                        .checked(self.state.rebase_autosquash)
                        .label(tr(
                            "Autosquash fixup!/squash! commits",
                            "自动压缩 fixup!/squash! 提交",
                        ))
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.toggle_rebase_autosquash(*checked, cx)
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            Button::new("rebase-start")
                                .primary()
                                .compact()
                                .label(tr("Start Rebase", "开始变基"))
                                .on_click(cx.listener(|this, _, _, cx| this.apply_rebase(cx))),
                        )
                        .child(
                            Button::new("rebase-cancel")
                                .ghost()
                                .compact()
                                .label(tr("Cancel", "取消"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_rebase(cx))),
                        ),
                ),
        )
    }

    pub(crate) fn render_conflicts_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let mut panel = div()
            .id("conflicts-panel")
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .overflow_y_scroll()
            .child(group_header(tr("Conflicts", "冲突"), muted))
            .child(div().flex_none().px_2().text_xs().text_color(muted).child(
                if self.state.merge_in_progress {
                    tr(
                        "Merge is paused. Resolve conflicts, then continue the merge.",
                        "合并已暂停。解决冲突后继续合并。",
                    )
                } else {
                    tr(
                        "Rebase is paused. Resolve conflicts, then continue the rebase.",
                        "变基已暂停。解决冲突后继续变基。",
                    )
                },
            ));

        if self.state.conflict_files.is_empty() {
            let message = if let Some(sha) = self.state.rebase.stopped_commit() {
                format!(
                    "{} {}… {}",
                    tr("Rebase stopped for editing at", "变基在此处暂停编辑："),
                    &sha[..sha.len().min(7)],
                    tr(
                        "Make changes, amend or commit, then click Continue Rebase.",
                        "修改、修正或提交后，点击“继续变基”。",
                    )
                )
            } else {
                tr("No conflicted files.", "没有冲突文件。").to_string()
            };
            panel = panel.child(div().text_xs().text_color(muted).child(message));
        }

        for (index, file) in self.state.conflict_files.iter().enumerate() {
            let selected = self.state.conflict_path.as_deref() == Some(file.path.as_str());
            let label = if selected {
                format!("● {}", file.path)
            } else {
                file.path.clone()
            };
            panel = panel.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("conflict-file", index))
                            .ghost()
                            .compact()
                            .label(label)
                            .on_click(cx.listener({
                                let path = file.path.clone();
                                move |this, _, _, cx| this.select_conflict_file(&path, cx)
                            })),
                    )
                    .child(
                        Button::new(("take-ours", index))
                            .ghost()
                            .compact()
                            .label(tr("Take ours", "采用我们的"))
                            .on_click(cx.listener({
                                let path = file.path.clone();
                                move |this, _, _, cx| {
                                    this.take_conflict_side(path.clone(), true, cx)
                                }
                            })),
                    )
                    .child(
                        Button::new(("take-theirs", index))
                            .ghost()
                            .compact()
                            .label(tr("Take theirs", "采用他们的"))
                            .on_click(cx.listener({
                                let path = file.path.clone();
                                move |this, _, _, cx| {
                                    this.take_conflict_side(path.clone(), false, cx)
                                }
                            })),
                    ),
            );
        }

        if let Some(path) = self.state.conflict_path.clone() {
            if self.state.conflict_hunks.is_empty() {
                panel = panel.child(div().text_xs().text_color(muted).child(tr(
                    "No textual hunks in this file. Use the buttons above to take one side.",
                    "该文件没有文本冲突块。使用上方按钮选择一侧。",
                )));
            }
            for (index, hunk) in self.state.conflict_hunks.iter().enumerate() {
                let ours_text = if hunk.ours.is_empty() {
                    tr("(empty)", "（空）").to_string()
                } else {
                    hunk.ours.join("\n")
                };
                let theirs_text = if hunk.theirs.is_empty() {
                    tr("(empty)", "（空）").to_string()
                } else {
                    hunk.theirs.join("\n")
                };
                let ours_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => {
                        format!("✓ {}", tr("Use ours", "采用我们的"))
                    }
                    _ => tr("Use ours", "采用我们的").to_string(),
                };
                let theirs_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Theirs)) => {
                        format!("✓ {}", tr("Use theirs", "采用他们的"))
                    }
                    _ => tr("Use theirs", "采用他们的").to_string(),
                };
                let both_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Both)) => {
                        format!("✓ {}", tr("Use both", "两者都采用"))
                    }
                    _ => tr("Use both", "两者都采用").to_string(),
                };
                let chose_ours = self
                    .state
                    .conflict_choices
                    .get(index)
                    .is_some_and(|c| c == &Some(HunkChoice::Ours));
                let chose_theirs = self
                    .state
                    .conflict_choices
                    .get(index)
                    .is_some_and(|c| c == &Some(HunkChoice::Theirs));
                let last_choice = self.state.conflict_choices.get(index).copied().flatten();
                // Result 栏实时反映当前所选取舍（Ours / Theirs / Both）。
                let result_text = if chose_ours {
                    ours_text.clone()
                } else if chose_theirs {
                    theirs_text.clone()
                } else if last_choice == Some(HunkChoice::Both) {
                    if theirs_text.is_empty() {
                        ours_text.clone()
                    } else {
                        format!("{}\n{}", ours_text, theirs_text)
                    }
                } else {
                    tr("— unresolved —", "— 未解决 —").to_string()
                };
                let result_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => {
                        format!("{} · {}", tr("Result", "结果"), tr("ours", "我们的"))
                    }
                    Some(Some(HunkChoice::Theirs)) => {
                        format!("{} · {}", tr("Result", "结果"), tr("theirs", "他们的"))
                    }
                    Some(Some(HunkChoice::Both)) => {
                        format!("{} · {}", tr("Result", "结果"), tr("both", "两者"))
                    }
                    _ => tr("Result", "结果").to_string(),
                };
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(theme::RADIUS))
                        .p_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!(
                                    "{} {} · {}",
                                    tr("Hunk", "冲突块"),
                                    index + 1,
                                    path
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1()
                                .child(
                                    Button::new(("hunk-ours", index))
                                        .ghost()
                                        .compact()
                                        .label(ours_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Ours, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("hunk-theirs", index))
                                        .ghost()
                                        .compact()
                                        .label(theirs_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Theirs, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("hunk-both", index))
                                        .ghost()
                                        .compact()
                                        .label(both_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Both, cx)
                                        })),
                                ),
                        )
                        // 三栏合并视图：左侧 Yours（当前分支）、中间 Result（合并结果）、
                        // 右侧 Theirs（合入分支的变更）。
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_stretch()
                                .gap_1()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .border_1()
                                        .border_color(border)
                                        .rounded(px(theme::RADIUS))
                                        .p_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child(tr("Yours", "你的")),
                                        )
                                        .child(div().text_xs().child(ours_text)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .border_1()
                                        .border_color(border)
                                        .rounded(px(theme::RADIUS))
                                        .p_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child(result_label),
                                        )
                                        .child(div().text_xs().child(result_text)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .border_1()
                                        .border_color(border)
                                        .rounded(px(theme::RADIUS))
                                        .p_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child(tr("Theirs", "他们的")),
                                        )
                                        .child(div().text_xs().child(theirs_text)),
                                ),
                        ),
                );
            }
            if !self.state.conflict_hunks.is_empty() {
                panel = panel.child(
                    Button::new("conflict-apply")
                        .primary()
                        .compact()
                        .label(tr("Apply Resolutions", "应用解决结果"))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.apply_conflict_resolutions(cx)),
                        ),
                );
            }
        }

        panel
    }

    pub(crate) fn render_shelve_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let muted = cx.theme().muted_foreground;
        let mut panel = div()
            .id("shelve-panel")
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .overflow_y_scroll()
            .child(group_header(tr("Shelves", "搁置"), muted));

        if self.state.shelves.is_empty() {
            panel = panel.child(div().text_xs().text_color(muted).child(tr(
                "Nothing on the shelf. Use Shelve in the commit composer.",
                "搁置区为空。在提交区使用“搁置”。",
            )));
        }

        for entry in &self.state.shelves {
            let index = entry.index;
            panel = panel.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_ellipsis()
                            .overflow_hidden()
                            .child(entry.message.clone()),
                    )
                    .child(
                        Button::new(("shelve-apply", index))
                            .ghost()
                            .compact()
                            .label(tr("Unshelve", "恢复搁置"))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.unshelve_at(index, cx)),
                            ),
                    )
                    .child(
                        Button::new(("shelve-drop", index))
                            .ghost()
                            .compact()
                            .label(tr("Drop", "丢弃"))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.drop_shelve_at(index, cx)),
                            ),
                    ),
            );
        }

        panel.child(
            Button::new("shelve-reload")
                .ghost()
                .compact()
                .label(tr("Reload", "刷新"))
                .on_click(cx.listener(|this, _, _, cx| this.reload_shelves(cx))),
        )
    }

    pub(crate) fn render_compare_panel(&self, cx: &mut Context<Self>) -> Div {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let mine = self.state.compare_mine.clone();
        let theirs = self.state.compare_theirs.clone();

        let mut panel = div().flex().flex_col().gap_2().size_full().min_h_0().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .flex_none()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("{} · {mine} ←→ {theirs}", tr("Compare", "比较"))),
                )
                .child(
                    Button::new("compare-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                ),
        );

        let sections = [
            (
                format!(
                    "{} {mine} ({})",
                    tr("Only in", "仅存在于"),
                    self.state.compare_ahead.len()
                ),
                &self.state.compare_ahead,
            ),
            (
                format!(
                    "{} {theirs} ({})",
                    tr("Only in", "仅存在于"),
                    self.state.compare_behind.len()
                ),
                &self.state.compare_behind,
            ),
        ];
        for (label, commits) in sections {
            panel = panel.child(group_header(label, muted));
            if commits.is_empty() {
                panel = panel.child(
                    div()
                        .flex_none()
                        .px_2()
                        .text_xs()
                        .text_color(muted)
                        .child(tr("None", "无")),
                );
            }
            for commit in commits.iter() {
                let id = commit.id.0.clone();
                let short = id[..id.len().min(7)].to_string();
                let subject = commit.subject.clone();
                let muted_fg = muted;
                let time = crate::ui::commit_list::format_time(commit.time);
                panel = panel.child(
                    div()
                        .id(format!("compare-{id}"))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_0p5()
                        .rounded(px(theme::RADIUS))
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme::hover_bg(fg)))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.select_commit_by_id(&id, cx)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(muted_fg)
                                .child(short),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_sm()
                                .child(subject),
                        )
                        .child(div().flex_none().text_xs().text_color(muted_fg).child(time)),
                );
            }
        }

        panel
    }

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let width = match self.state.sidebar {
            SidebarMode::Workspace | SidebarMode::Detail => 420.,
            SidebarMode::Diff | SidebarMode::Blame | SidebarMode::Compare => 680.,
            SidebarMode::Rebase => 480.,
            SidebarMode::Conflicts => 680.,
            SidebarMode::Shelve => 420.,
            SidebarMode::History | SidebarMode::Reflog => 680.,
        };
        let base = div()
            .w(px(width))
            .flex_none()
            .border_l_1()
            .border_color(border)
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
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

    pub(crate) fn render_diff_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let title = self.state.diff_title.clone();

        let mut panel = div().flex().flex_col().gap_2().min_h_0().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .flex_none()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .text_color(muted)
                        .child(title),
                )
                .when(!self.state.diff_files.is_empty(), |header| {
                    header
                        .child(
                            Button::new("diff-ignore-ws")
                                .ghost()
                                .compact()
                                .label(if self.state.ignore_whitespace {
                                    format!("✓ {}", tr("Ignore whitespace", "忽略空白"))
                                } else {
                                    tr("Ignore whitespace", "忽略空白").to_string()
                                })
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.toggle_ignore_whitespace(cx)),
                                ),
                        )
                        .child(
                            Button::new("diff-view-mode")
                                .ghost()
                                .compact()
                                .label(if self.state.diff_side_by_side {
                                    tr("⇔ Side-by-side", "⇔ 并排对比")
                                } else {
                                    tr("≡ Unified", "≡ 统一视图")
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state.diff_side_by_side = !this.state.diff_side_by_side;
                                    cx.notify();
                                })),
                        )
                })
                .when(self.state.diff_path.is_some(), |header| {
                    header.child(
                        Button::new("diff-blame")
                            .ghost()
                            .compact()
                            .label(tr("Blame", "追溯"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let path = this.state.diff_path.clone().unwrap_or_default();
                                this.open_blame(path, cx);
                            })),
                    )
                })
                .when(
                    self.state.diff_path.is_some() && !self.state.diff_editing,
                    |header| {
                        header.child(
                            Button::new("diff-edit")
                                .ghost()
                                .compact()
                                .icon(Ic::Edit)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_diff_edit(window, cx)
                                })),
                        )
                    },
                )
                .when(self.state.diff_editing, |header| {
                    header
                        .child(
                            Button::new("diff-save")
                                .ghost()
                                .compact()
                                .label(tr("Save", "保存"))
                                .on_click(cx.listener(|this, _, _, cx| this.save_diff_edit(cx))),
                        )
                        .child(
                            Button::new("diff-cancel")
                                .ghost()
                                .compact()
                                .label(tr("Cancel", "取消"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_diff_edit(cx))),
                        )
                })
                .child(
                    Button::new("diff-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                ),
        );

        if self.state.diff_editing {
            panel = panel.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(Textarea::new(&self.diff_edit_input).flex_1()),
            );
        } else if self.state.diff_files.is_empty() {
            panel = panel.child(empty_state(tr("No changes", "无更改"), muted));
        } else {
            let hunk_controls: Option<(&'static str, HunkAction)> = match self.state.diff_source {
                Some(source @ (DiffSource::Staged | DiffSource::Unstaged)) => {
                    let label = if source == DiffSource::Staged {
                        tr("Unstage", "取消暂存")
                    } else {
                        tr("Stage", "暂存")
                    };
                    let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                    Some((
                        label,
                        std::sync::Arc::new(move |file_index, hunk_index, app: &mut gpui::App| {
                            let _ = weak.update(app, |this, cx| {
                                this.toggle_hunk_stage(file_index, hunk_index, cx)
                            });
                        }),
                    ))
                }
                _ => None,
            };
            panel = panel.child(
                div()
                    .id("diff-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_diff_files(
                        &self.state.diff_files,
                        self.state.diff_side_by_side,
                        hunk_controls
                            .as_ref()
                            .map(|(label, action)| (*label, action)),
                        cx,
                    )),
            );
        }
        panel
    }

    pub(crate) fn render_blame_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let path = self.state.blame_path.clone();

        let mut panel = div().flex().flex_col().gap_2().min_h_0().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .flex_none()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .text_color(muted)
                        .child(format!("{} · {path}", tr("Blame", "追溯"))),
                )
                .child(
                    Button::new("blame-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                ),
        );

        if self.state.blame_groups.is_empty() {
            panel = panel.child(empty_state(tr("Nothing to blame", "无追溯信息"), muted));
        } else {
            let on_commit: BlameJump = {
                let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                std::sync::Arc::new(move |id, app| {
                    let _ =
                        weak.update(app, |this, cx| this.open_commit_diff(id.clone(), None, cx));
                })
            };
            panel = panel.child(
                div()
                    .id("blame-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_blame(&self.state.blame_groups, Some(&on_commit), cx)),
            );
        }
        panel
    }

    pub(crate) fn render_history_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let path = self.state.history_path.clone();

        let mut panel = div()
            .id("history-panel")
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("{} · {path}", tr("History", "历史"))),
                    )
                    .child(
                        Button::new("history-close")
                            .ghost()
                            .compact()
                            .icon(Ic::Close)
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.state.history_commits.is_empty() {
            panel = panel.child(empty_state(
                tr("No history for this file", "该文件没有历史记录"),
                muted,
            ));
        } else {
            for commit in &self.state.history_commits {
                let id = commit.id.0.clone();
                let short = id[..id.len().min(7)].to_string();
                let muted_fg = muted;
                let time = crate::ui::commit_list::format_time(commit.time);
                panel = panel.child(
                    div()
                        .id(format!("history-{id}"))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_0p5()
                        .rounded(px(theme::RADIUS))
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme::hover_bg(fg)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_commit_diff(id.clone(), None, cx)
                        }))
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(muted_fg)
                                .child(short),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .child(commit.subject.clone()),
                        )
                        .child(div().flex_none().text_xs().text_color(muted_fg).child(time)),
                );
            }
        }
        panel
    }

    /// reflog 轻量视图：一览 HEAD 的最近操作，点击跳到对应 commit 的 diff。
    pub(crate) fn render_reflog_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;

        let mut panel = div()
            .id("reflog-panel")
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_sm()
                            .text_color(muted)
                            .child(format!(
                                "{} · {}",
                                tr("Reflog", "引用日志"),
                                tr("recent operations", "最近操作")
                            )),
                    )
                    .child(
                        Button::new("reflog-refresh")
                            .ghost()
                            .compact()
                            .icon(Ic::Refresh)
                            .on_click(cx.listener(|this, _, _, cx| this.open_reflog(cx))),
                    )
                    .child(
                        Button::new("reflog-close")
                            .ghost()
                            .compact()
                            .icon(Ic::Close)
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.state.reflog_entries.is_empty() {
            panel = panel.child(empty_state(tr("No reflog entries", "没有引用日志"), muted));
        } else {
            for entry in &self.state.reflog_entries {
                let id = entry.commit_id.clone();
                panel = panel.child(
                    div()
                        .id(format!("reflog-{}", entry.selector))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_0p5()
                        .rounded(px(theme::RADIUS))
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme::hover_bg(fg)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_commit_diff(id.clone(), None, cx)
                        }))
                        .child(
                            div()
                                .flex_none()
                                .w(px(76.))
                                .text_xs()
                                .text_color(muted)
                                .child(SharedString::from(entry.selector.clone())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .child(SharedString::from(entry.message.clone())),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(muted)
                                .child(SharedString::from(entry.short_id.clone())),
                        ),
                );
            }
        }
        panel
    }

    /// Alt+` VCS 操作快切弹层（对标 JetBrains VCS Operations Popup）：
    /// 一览全部高频 VCS 动作并附带键位提示，动作执行后自动关闭。
    pub(crate) fn render_vcs_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.state.vcs_palette {
            return None;
        }
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let head = self.state.head_id.clone();

        let mut items: Vec<AnyElement> = Vec::new();
        items.push(
            self.palette_item(
                "pal-commit",
                tr("Commit changes…", "提交更改…"),
                "Ctrl+K",
                cx,
                |this, window, cx| {
                    this.on_focus_composer(&FocusComposer, window, cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-push",
                tr("Push", "推送"),
                "Ctrl+Shift+K",
                cx,
                |this, _, cx| {
                    this.do_push(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-pull",
                tr("Pull", "拉取"),
                "Ctrl+T",
                cx,
                |this, _, cx| {
                    this.do_pull(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-fetch", tr("Fetch", "抓取"), "", cx, |this, _, cx| {
                this.run_op_progress(
                    tr("Fetch", "抓取"),
                    tr("Fetched", "已抓取"),
                    |repo, progress, cancel| repo.fetch_with_control(progress, cancel),
                    cx,
                );
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-stash",
                tr("Stash changes…", "贮藏更改…"),
                "",
                cx,
                |this, _, cx| {
                    this.open_prompt(super::PromptKind::Stash, cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-unstash",
                tr("Unstash latest", "恢复最近的贮藏"),
                "",
                cx,
                |this, _, cx| {
                    this.run_op(tr("Unstashed", "已恢复贮藏"), |repo| repo.stash_pop(), cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-branch",
                tr("New branch…", "新建分支…"),
                "",
                cx,
                |this, _, cx| {
                    this.open_prompt(super::PromptKind::NewBranch { start_point: None }, cx);
                },
            )
            .into_any_element(),
        );
        if let Some(head) = head {
            items.push(
                self.palette_item(
                    "pal-tag",
                    tr("New tag on HEAD…", "在 HEAD 上新建标签…"),
                    "",
                    cx,
                    move |this, _, cx| {
                        this.open_prompt(
                            super::PromptKind::NewTag {
                                commit_id: head.clone(),
                            },
                            cx,
                        );
                    },
                )
                .into_any_element(),
            );
        }
        items.push(
            self.palette_item(
                "pal-goto",
                tr("Go to commit…", "跳转到提交…"),
                "",
                cx,
                |this, _, cx| {
                    this.open_prompt(super::PromptKind::GoTo, cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-blame",
                tr("Blame current file", "追溯当前文件"),
                "Ctrl+Alt+B",
                cx,
                |this, _, cx| {
                    this.blame_current_file(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-reflog",
                tr("Show reflog", "显示引用日志"),
                "",
                cx,
                |this, _, cx| {
                    this.open_reflog(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-conflicts",
                tr("Show conflicts", "显示冲突"),
                "Ctrl+Alt+8",
                cx,
                |this, _, cx| {
                    this.open_conflicts(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-shelves",
                tr("Show shelves", "显示搁置"),
                "Ctrl+Alt+6",
                cx,
                |this, _, cx| {
                    this.open_shelves(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-refresh",
                tr("Refresh repository", "刷新仓库"),
                "F5",
                cx,
                |this, _, cx| {
                    this.refresh(cx);
                },
            )
            .into_any_element(),
        );

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(theme::overlay_bg())
                .flex()
                .items_start()
                .justify_center()
                .pt(px(96.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(420.))
                        .rounded(px(theme::RADIUS_LG))
                        .border_1()
                        .border_color(border)
                        .bg(cx.theme().background)
                        .p_2()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .shadow_lg()
                        .child(group_header(tr("VCS Operations", "VCS 操作"), muted))
                        .child(
                            div()
                                .px_2()
                                .pb_1()
                                .text_xs()
                                .text_color(muted)
                                .child(tr("Alt+` toggle · Esc to close", "Alt+` 切换 · Esc 关闭")),
                        )
                        .child(div().flex().flex_col().gap_0p5().children(items)),
                )
                .into_any_element(),
        )
    }

    /// 快切弹层的单行动作条目：名称居左、键位提示居右。
    fn palette_item(
        &self,
        id: &'static str,
        label: &'static str,
        keys: &'static str,
        cx: &mut Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        div()
            .id(SharedString::from(id))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px_2()
            .py_1p5()
            .rounded(px(theme::RADIUS))
            .text_sm()
            .hover(move |style| style.bg(theme::hover_bg(fg)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.state.vcs_palette = false;
                run(this, window, cx);
            }))
            .child(SharedString::from(label))
            .when(!keys.is_empty(), |row| {
                row.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(SharedString::from(keys)),
                )
            })
    }

    pub(crate) fn render_prompt_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let kind = self.state.prompt.clone()?;
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let (title, hint): (String, String) = match &kind {
            super::PromptKind::NewBranch { start_point } => (
                tr("New branch", "新建分支").to_string(),
                match start_point {
                    Some(point) => format!(
                        "{} {}",
                        tr("From commit", "从提交"),
                        &point[..point.len().min(7)]
                    ),
                    None => tr("From current HEAD", "从当前 HEAD").to_string(),
                },
            ),
            super::PromptKind::NewTag { commit_id } => (
                tr("New tag", "新建标签").to_string(),
                format!(
                    "{} {}",
                    tr("On commit", "在提交"),
                    &commit_id[..commit_id.len().min(7)]
                ),
            ),
            super::PromptKind::EditTag { name, commit_id } => (
                tr("Edit tag message", "编辑标签信息").to_string(),
                format!(
                    "{} {name} @ {} — {}",
                    tr("Rebuild", "重建"),
                    &commit_id[..commit_id.len().min(7)],
                    tr("with a new annotated message", "使用新的附注信息")
                ),
            ),
            super::PromptKind::Stash => (
                tr("Stash changes", "贮藏更改").to_string(),
                tr(
                    "Optional message; untracked files are included",
                    "可选信息；未跟踪文件也会被包含",
                )
                .to_string(),
            ),
            super::PromptKind::Reword { commit_id } => (
                tr("Reword commit", "改写提交").to_string(),
                format!(
                    "{} {}",
                    tr("New message for", "新的提交信息："),
                    &commit_id[..commit_id.len().min(7)]
                ),
            ),
            super::PromptKind::RenameBranch => (
                tr("Rename branch", "重命名分支").to_string(),
                match &self.state.current_branch {
                    Some(name) => format!(
                        "{} {name} →",
                        tr("Rename current branch", "重命名当前分支")
                    ),
                    None => tr("No current branch", "没有当前分支").to_string(),
                },
            ),
            super::PromptKind::RenameBranchByName { name } => (
                tr("Rename branch", "重命名分支").to_string(),
                format!("{} {name} →", tr("Rename", "重命名")),
            ),
            super::PromptKind::MergeMessage { name } => (
                tr("Merge message", "合并信息").to_string(),
                format!(
                    "{} {name} (no ff):",
                    tr("Merge commit message for merging", "合并提交信息（非快进）")
                ),
            ),
            super::PromptKind::RebaseEdit { .. } => (
                tr("Edit commit message", "编辑提交信息").to_string(),
                tr(
                    "Set the message used when this commit is reworded (or merged by squash).",
                    "设置此提交改写（或被 squash 合并）时使用的信息。",
                )
                .to_string(),
            ),
            super::PromptKind::Reset { commit_id } => (
                tr("Reset current branch to here", "重置当前分支到此处").to_string(),
                format!(
                    "{} {}. {}",
                    tr("Move the current branch to", "将当前分支移动到"),
                    &commit_id[..commit_id.len().min(7)],
                    tr(
                        "Pick a mode: Soft keeps everything staged, Mixed keeps changes unstaged, Hard discards all changes.",
                        "选择模式：Soft 保留全部更改并暂存，Mixed 保留更改但取消暂存，Hard 丢弃所有更改。",
                    ),
                ),
            ),
            super::PromptKind::GoTo => (
                tr("Go to commit", "跳转到提交").to_string(),
                tr(
                    "Enter a hash, branch or tag name to select it in the log.",
                    "输入哈希、分支或标签名以在日志中定位。",
                )
                .to_string(),
            ),
            super::PromptKind::FilterAuthor => (
                tr("Filter by author", "按作者过滤").to_string(),
                tr(
                    "Show only commits whose author matches this text. Leave empty to clear the filter.",
                    "仅显示作者匹配的提交。留空以清除过滤。",
                )
                .to_string(),
            ),
            super::PromptKind::AddRemote => (
                tr("Add remote", "添加远程仓库").to_string(),
                tr(
                    "Enter the remote name (e.g. origin) and its URL.",
                    "输入远程名称（如 origin）及其 URL。",
                )
                .to_string(),
            ),
            super::PromptKind::SetUpstream { branch } => (
                tr("Set upstream", "设置上游").to_string(),
                format!(
                    "{} {branch} (e.g. origin/main):",
                    tr("Upstream of", "上游分支：")
                ),
            ),
            super::PromptKind::Confirm(action) => (action.title(), action.hint()),
        };

        let body: Div = match &kind {
            super::PromptKind::Reset { commit_id } => {
                let target = commit_id.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Button::new("reset-soft")
                            .primary()
                            .label(tr(
                                "Soft — keep all changes staged",
                                "Soft — 保留全部更改并暂存",
                            ))
                            .on_click(cx.listener({
                                let target = target.clone();
                                move |this, _, _, cx| {
                                    this.reset_branch_to(target.clone(), ResetMode::Soft, cx)
                                }
                            })),
                    )
                    .child(
                        Button::new("reset-mixed")
                            .label(tr(
                                "Mixed — keep changes unstaged",
                                "Mixed — 保留更改但不暂存",
                            ))
                            .on_click(cx.listener({
                                let target = target.clone();
                                move |this, _, _, cx| {
                                    this.reset_branch_to(target.clone(), ResetMode::Mixed, cx)
                                }
                            })),
                    )
                    .child(
                        Button::new("reset-hard")
                            .danger()
                            .label(tr("Hard — discard all changes", "Hard — 丢弃所有更改"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reset_branch_to(target.clone(), ResetMode::Hard, cx)
                            })),
                    )
            }
            super::PromptKind::Confirm(action) => {
                let action = action.clone();
                let label = action.confirm_label();
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("prompt-cancel")
                            .ghost()
                            .label(tr("Cancel", "取消"))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx))),
                    )
                    .child(
                        Button::new("prompt-confirm")
                            .danger()
                            .label(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.confirm_action(action.clone(), cx)
                            })),
                    )
            }
            super::PromptKind::AddRemote => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Textarea::new(&self.prompt_input).h(px(32.)))
                .child(Textarea::new(&self.prompt_input2).h(px(32.)))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("prompt-cancel")
                                .ghost()
                                .label(tr("Cancel", "取消"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx))),
                        )
                        .child(
                            Button::new("prompt-ok")
                                .primary()
                                .label(tr("Add", "添加"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_prompt(window, cx)
                                })),
                        ),
                ),
            _ => {
                let ok_label = match &kind {
                    super::PromptKind::NewBranch { .. } => tr("Create", "创建"),
                    super::PromptKind::NewTag { .. } => tr("Tag", "打标签"),
                    super::PromptKind::EditTag { .. } => tr("Save", "保存"),
                    super::PromptKind::Stash => tr("Stash", "贮藏"),
                    super::PromptKind::Reword { .. } | super::PromptKind::RebaseEdit { .. } => {
                        tr("Reword", "改写")
                    }
                    super::PromptKind::RenameBranch => tr("Rename", "重命名"),
                    super::PromptKind::GoTo => tr("Go", "跳转"),
                    super::PromptKind::FilterAuthor => tr("Filter", "过滤"),
                    super::PromptKind::SetUpstream { .. } => tr("Set", "设置"),
                    _ => tr("OK", "确定"),
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Textarea::new(&self.prompt_input).h(px(64.)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("prompt-cancel")
                                    .ghost()
                                    .label(tr("Cancel", "取消"))
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx))),
                            )
                            .child(Button::new("prompt-ok").primary().label(ok_label).on_click(
                                cx.listener(|this, _, window, cx| this.confirm_prompt(window, cx)),
                            )),
                    )
            }
        };

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(theme::overlay_bg())
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(440.))
                        .rounded(px(theme::RADIUS_LG))
                        .border_1()
                        .border_color(border)
                        .bg(cx.theme().background)
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(SharedString::from(title)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(SharedString::from(hint)),
                        )
                        .child(body),
                )
                .into_any_element(),
        )
    }
}
