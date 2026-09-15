use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, AnyElement, AppContext, Context, Div, FontWeight, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Textarea,
    ActiveTheme,
};

use rebased_rs::git::{HunkChoice, ResetMode};

use crate::ui::blame_view::{render_blame, BlameJump};
use crate::ui::diff_view::{render_diff_files, HunkAction};

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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Interactive Rebase"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("onto {short_base}…")),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Click the action to cycle Pick → Squash → Fixup → Drop → Edit → Reword. Drag rows or use ↑ ↓ to reorder."),
            );

        if plan.is_empty() {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("No commits between base and HEAD."),
            );
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
            let drag_label: SharedString = format!("Move {short}").into();
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
                        style.border_color(hsla(0.55, 0.8, 0.55, 1.0))
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
                            .label("✎ Edit")
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
                        .label("Autosquash fixup!/squash! commits")
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
                                .label("Start Rebase")
                                .on_click(cx.listener(|this, _, _, cx| this.apply_rebase(cx))),
                        )
                        .child(
                            Button::new("rebase-cancel")
                                .ghost()
                                .compact()
                                .label("Cancel")
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Conflicts"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(if self.state.merge_in_progress {
                                "Merge is paused. Resolve conflicts, then continue the merge."
                            } else {
                                "Rebase is paused. Resolve conflicts, then continue the rebase."
                            }),
                    ),
            );

        if self.state.conflict_files.is_empty() {
            let message = if let Some(sha) = self.state.rebase.stopped_commit() {
                format!(
                    "Rebase stopped for editing at {}… Make changes, amend or commit, then click Continue Rebase.",
                    &sha[..sha.len().min(7)]
                )
            } else {
                "No conflicted files.".to_string()
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
                            .label("Take ours")
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
                            .label("Take theirs")
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
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(
                            "No textual hunks in this file. Use the buttons above to take one side.",
                        ),
                );
            }
            for (index, hunk) in self.state.conflict_hunks.iter().enumerate() {
                let ours_text = if hunk.ours.is_empty() {
                    "(empty)".to_string()
                } else {
                    hunk.ours.join("\n")
                };
                let theirs_text = if hunk.theirs.is_empty() {
                    "(empty)".to_string()
                } else {
                    hunk.theirs.join("\n")
                };
                let ours_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => "✓ Use ours".to_string(),
                    _ => "Use ours".to_string(),
                };
                let theirs_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Theirs)) => "✓ Use theirs".to_string(),
                    _ => "Use theirs".to_string(),
                };
                let both_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Both)) => "✓ Use both".to_string(),
                    _ => "Use both".to_string(),
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
                    "— unresolved —".to_string()
                };
                let result_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => "Result · ours".to_string(),
                    Some(Some(HunkChoice::Theirs)) => "Result · theirs".to_string(),
                    Some(Some(HunkChoice::Both)) => "Result · both".to_string(),
                    _ => "Result".to_string(),
                };
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(4.))
                        .p_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("Hunk {} in {}", index + 1, path)),
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
                                        .rounded(px(4.))
                                        .p_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child("Yours"),
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
                                        .rounded(px(4.))
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
                                        .rounded(px(4.))
                                        .p_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child("Theirs"),
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
                        .label("Apply Resolutions")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.apply_conflict_resolutions(cx)
                        })),
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Shelves"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Stashed workspaces. Unshelve to bring changes back."),
                    ),
            );

        if self.state.shelves.is_empty() {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(
                        "Nothing on the shelf. Use Shelve in the commit composer.",
                    ),
            );
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
                            .label("Unshelve")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.unshelve_at(index, cx)
                            })),
                    )
                    .child(
                        Button::new(("shelve-drop", index))
                            .ghost()
                            .compact()
                            .label("Drop")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.drop_shelve_at(index, cx)
                            })),
                    ),
            );
        }

        panel.child(
            Button::new("shelve-reload")
                .ghost()
                .compact()
                .label("Reload")
                .on_click(cx.listener(|this, _, _, cx| this.reload_shelves(cx))),
        )
    }

    pub(crate) fn render_compare_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let mine = self.state.compare_mine.clone();
        let theirs = self.state.compare_theirs.clone();

        let mut panel = div()
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
                            .child(format!("Compare · {mine} ←→ {theirs}")),
                    )
                    .child(
                        Button::new("compare-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        let sections = [
            (
                format!("{mine} only ({})", self.state.compare_ahead.len()),
                &self.state.compare_ahead,
            ),
            (
                format!("{theirs} only ({})", self.state.compare_behind.len()),
                &self.state.compare_behind,
            ),
        ];
        for (label, commits) in sections {
            panel = panel.child(
                div()
                    .flex_none()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(label),
            );
            if commits.is_empty() {
                panel = panel.child(
                    div()
                        .flex_none()
                        .px_2()
                        .text_xs()
                        .text_color(muted)
                        .child("None"),
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
                        .rounded(px(4.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_commit_by_id(&id, cx)
                        }))
                        .child(div().flex_none().text_xs().text_color(muted_fg).child(short))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_sm()
                                .child(subject),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(muted_fg)
                                .child(time),
                        ),
                );
            }
        }

        panel
    }

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let width = match self.state.sidebar {
            SidebarMode::Workspace => 360.,
            SidebarMode::Detail => 420.,
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
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .overflow_hidden();

        match self.state.sidebar {
            SidebarMode::Diff => base.child(self.render_diff_panel(cx)).into_any_element(),
            SidebarMode::Blame => base.child(self.render_blame_panel(cx)).into_any_element(),
            SidebarMode::Compare => base.child(self.render_compare_panel(cx)).into_any_element(),
            SidebarMode::Rebase => base.child(self.render_rebase_panel(cx)).into_any_element(),
            SidebarMode::Conflicts => {
                base.child(self.render_conflicts_panel(cx)).into_any_element()
            }
            SidebarMode::Shelve => base.child(self.render_shelve_panel(cx)).into_any_element(),
            SidebarMode::History => base.child(self.render_history_panel(cx)).into_any_element(),
            SidebarMode::Reflog => base.child(self.render_reflog_panel(cx)).into_any_element(),
            SidebarMode::Detail => match &self.state.selected {
                Some(commit) => base.child(self.render_detail(commit, cx)).into_any_element(),
                None => base.child(self.render_workspace(cx)).into_any_element(),
            },
            SidebarMode::Workspace => base.child(self.render_workspace(cx)).into_any_element(),
        }
    }

    pub(crate) fn render_diff_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let title = self.state.diff_title.clone();

        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
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
                            .child(title),
                    )
                    .when(!self.state.diff_files.is_empty(), |header| {
                        header
                            .child(
                                Button::new("diff-ignore-ws")
                                    .ghost()
                                    .label(if self.state.ignore_whitespace {
                                        "☑ Ignore whitespace"
                                    } else {
                                        "☐ Ignore whitespace"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_ignore_whitespace(cx)
                                    })),
                            )
                            .child(
                                Button::new("diff-view-mode")
                                    .ghost()
                                    .label(if self.state.diff_side_by_side {
                                        "⇔ Side-by-side"
                                    } else {
                                        "≡ Unified"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.diff_side_by_side =
                                            !this.state.diff_side_by_side;
                                        cx.notify();
                                    })),
                            )
                    })
                    .when(self.state.diff_path.is_some(), |header| {
                        header.child(
                            Button::new("diff-blame")
                                .ghost()
                                .label("Blame")
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
                                    .label("✎ Edit")
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
                                    .label("Save")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_diff_edit(cx)),
                                    ),
                            )
                            .child(
                                Button::new("diff-cancel")
                                    .ghost()
                                    .label("Cancel")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.cancel_diff_edit(cx)),
                                    ),
                            )
                    })
                    .child(
                        Button::new("diff-close")
                            .ghost()
                            .label("✕")
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
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("No changes"),
            );
        } else {
            let hunk_controls: Option<(&'static str, HunkAction)> =
                match self.state.diff_source {
                    Some(source @ (DiffSource::Staged | DiffSource::Unstaged)) => {
                        let label = if source == DiffSource::Staged {
                            "Unstage"
                        } else {
                            "Stage"
                        };
                        let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                        Some((
                            label,
                            std::sync::Arc::new(
                                move |file_index, hunk_index, app: &mut gpui::App| {
                                    let _ = weak.update(app, |this, cx| {
                                        this.toggle_hunk_stage(file_index, hunk_index, cx)
                                    });
                                },
                            ),
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
                        hunk_controls.as_ref().map(|(label, action)| (*label, action)),
                        cx,
                    )),
            );
        }
        panel
    }

    pub(crate) fn render_blame_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let path = self.state.blame_path.clone();

        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
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
                            .child(format!("Blame · {path}")),
                    )
                    .child(
                        Button::new("blame-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.state.blame_groups.is_empty() {
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("Nothing to blame"),
            );
        } else {
            let on_commit: BlameJump = {
                let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                std::sync::Arc::new(move |id, app| {
                    let _ = weak.update(app, |this, cx| {
                        this.open_commit_diff(id.clone(), None, cx)
                    });
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
                            .child(format!("History · {path}")),
                    )
                    .child(
                        Button::new("history-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.state.history_commits.is_empty() {
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("No history for this file"),
            );
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
                        .rounded(px(4.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_commit_diff(id.clone(), None, cx)
                        }))
                        .child(div().flex_none().text_xs().text_color(muted_fg).child(short))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .child(commit.subject.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(muted_fg)
                                .child(time),
                        ),
                );
            }
        }
        panel
    }

    /// reflog 轻量视图：一览 HEAD 的最近操作，点击跳到对应 commit 的 diff。
    pub(crate) fn render_reflog_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
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
                            .child("Reflog · recent operations"),
                    )
                    .child(
                        Button::new("reflog-refresh")
                            .ghost()
                            .compact()
                            .label("↻")
                            .on_click(cx.listener(|this, _, _, cx| this.open_reflog(cx))),
                    )
                    .child(
                        Button::new("reflog-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.state.reflog_entries.is_empty() {
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("No reflog entries"),
            );
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
                        .rounded(px(4.))
                        .cursor_pointer()
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
            self.palette_item("pal-commit", "Commit changes…", "Ctrl+K", cx, |this, window, cx| {
                this.on_focus_composer(&FocusComposer, window, cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-push", "Push", "Ctrl+Shift+K", cx, |this, _, cx| {
                this.do_push(cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-pull", "Pull", "Ctrl+T", cx, |this, _, cx| {
                this.do_pull(cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-fetch", "Fetch", "", cx, |this, _, cx| {
                this.run_op_progress(
                    "Fetch",
                    "Fetched",
                    |repo, progress, cancel| repo.fetch_with_control(progress, cancel),
                    cx,
                );
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-stash", "Stash changes…", "", cx, |this, _, cx| {
                this.open_prompt(super::PromptKind::Stash, cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-unstash", "Unstash latest", "", cx, |this, _, cx| {
                this.run_op("Unstashed", |repo| repo.stash_pop(), cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-branch", "New branch…", "", cx, |this, _, cx| {
                this.open_prompt(
                    super::PromptKind::NewBranch { start_point: None },
                    cx,
                );
            })
            .into_any_element(),
        );
        if let Some(head) = head {
            items.push(
                self.palette_item("pal-tag", "New tag on HEAD…", "", cx, move |this, _, cx| {
                    this.open_prompt(
                        super::PromptKind::NewTag {
                            commit_id: head.clone(),
                        },
                        cx,
                    );
                })
                .into_any_element(),
            );
        }
        items.push(
            self.palette_item("pal-goto", "Go to commit…", "", cx, |this, _, cx| {
                this.open_prompt(super::PromptKind::GoTo, cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-blame", "Blame current file", "Ctrl+Alt+B", cx, |this, _, cx| {
                this.blame_current_file(cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-reflog", "Show reflog", "", cx, |this, _, cx| {
                this.open_reflog(cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-conflicts", "Show conflicts", "Ctrl+Alt+8", cx, |this, _, cx| {
                this.open_conflicts(cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-shelves", "Show shelves", "Ctrl+Alt+6", cx, |this, _, cx| {
                this.open_shelves(cx);
            })
            .into_any_element(),
        );
        items.push(
            self.palette_item("pal-refresh", "Refresh repository", "F5", cx, |this, _, cx| {
                this.refresh(cx);
            })
            .into_any_element(),
        );

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.45))
                .flex()
                .items_start()
                .justify_center()
                .pt(px(96.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(420.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(border)
                        .bg(cx.theme().background)
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .shadow_lg()
                        .child(
                            div()
                                .px_3()
                                .pt_1()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child("VCS Operations"),
                        )
                        .child(
                            div()
                                .px_3()
                                .pb_1()
                                .text_xs()
                                .text_color(muted)
                                .child("Alt+` toggle · Esc to close"),
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
        let muted = cx.theme().muted_foreground;
        div()
            .id(SharedString::from(id))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_1p5()
            .rounded(px(6.))
            .text_sm()
            .hover(move |style| style.bg(muted.opacity(0.12)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.state.vcs_palette = false;
                run(this, window, cx);
            }))
            .child(SharedString::from(label))
            .when(!keys.is_empty(), |row| {
                row.child(div().text_xs().text_color(muted).child(SharedString::from(keys)))
            })
    }

    pub(crate) fn render_prompt_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let kind = self.state.prompt.clone()?;
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let (title, hint): (String, String) = match &kind {
            super::PromptKind::NewBranch { start_point } => (
                "New branch".to_string(),
                match start_point {
                    Some(point) => format!("From commit {}", &point[..point.len().min(7)]),
                    None => "From current HEAD".to_string(),
                },
            ),
            super::PromptKind::NewTag { commit_id } => (
                "New tag".to_string(),
                format!("On commit {}", &commit_id[..commit_id.len().min(7)]),
            ),
            super::PromptKind::EditTag { name, commit_id } => (
                "Edit tag message".to_string(),
                format!(
                    "Rebuild {name} on commit {} with a new annotated message.",
                    &commit_id[..commit_id.len().min(7)]
                ),
            ),
            super::PromptKind::Stash => (
                "Stash changes".to_string(),
                "Optional message; untracked files are included".to_string(),
            ),
            super::PromptKind::Reword { commit_id } => (
                "Reword commit".to_string(),
                format!("New message for {}", &commit_id[..commit_id.len().min(7)]),
            ),
            super::PromptKind::RenameBranch => (
                "Rename branch".to_string(),
                match &self.state.current_branch {
                    Some(name) => format!("Rename current branch {name} to:"),
                    None => "No current branch".to_string(),
                },
            ),
            super::PromptKind::RenameBranchByName { name } => (
                "Rename branch".to_string(),
                format!("Rename {name} to:"),
            ),
            super::PromptKind::MergeMessage { name } => (
                "Merge message".to_string(),
                format!("Merge commit message for merging {name} (no ff):"),
            ),
            super::PromptKind::RebaseEdit { .. } => (
                "Edit commit message".to_string(),
                "Set the message used when this commit is reworded (or merged by squash).".to_string(),
            ),
            super::PromptKind::Reset { commit_id } => (
                "Reset current branch to here".to_string(),
                format!(
                    "Move the current branch to {}. Pick a mode: Soft keeps everything staged, Mixed keeps changes unstaged, Hard discards all changes.",
                    &commit_id[..commit_id.len().min(7)]
                ),
            ),
            super::PromptKind::GoTo => (
                "Go to commit".to_string(),
                "Enter a hash, branch or tag name to select it in the log.".to_string(),
            ),
            super::PromptKind::FilterAuthor => (
                "Filter by author".to_string(),
                "Show only commits whose author matches this text. Leave empty to clear the filter.".to_string(),
            ),
            super::PromptKind::AddRemote => (
                "Add remote".to_string(),
                "Enter the remote name (e.g. origin) and its URL.".to_string(),
            ),
            super::PromptKind::SetUpstream { branch } => (
                "Set upstream".to_string(),
                format!("Upstream of {branch} (e.g. origin/main):"),
            ),
            super::PromptKind::Confirm(action) => {
                (action.title().to_string(), action.hint())
            }
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
                            .label("Soft — keep all changes staged")
                            .on_click(cx.listener({
                                let target = target.clone();
                                move |this, _, _, cx| {
                                    this.reset_branch_to(target.clone(), ResetMode::Soft, cx)
                                }
                            })),
                    )
                    .child(
                        Button::new("reset-mixed")
                            .label("Mixed — keep changes unstaged")
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
                            .label("Hard — discard all changes")
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
                            .label("Cancel")
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
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel_prompt(cx)
                                })),
                        )
                        .child(
                            Button::new("prompt-ok")
                                .primary()
                                .label("Add")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_prompt(window, cx)
                                })),
                        ),
                ),
            _ => {
                let ok_label = match &kind {
                    super::PromptKind::NewBranch { .. } => "Create",
                    super::PromptKind::NewTag { .. } => "Tag",
                    super::PromptKind::EditTag { .. } => "Save",
                    super::PromptKind::Stash => "Stash",
                    super::PromptKind::Reword { .. } | super::PromptKind::RebaseEdit { .. } => "Reword",
                    super::PromptKind::RenameBranch => "Rename",
                    super::PromptKind::GoTo => "Go",
                    super::PromptKind::FilterAuthor => "Filter",
                    super::PromptKind::SetUpstream { .. } => "Set",
                    _ => "OK",
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
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.cancel_prompt(cx)
                                    })),
                            )
                            .child(
                                Button::new("prompt-ok")
                                    .primary()
                                    .label(ok_label)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_prompt(window, cx)
                                    })),
                            ),
                    )
            }
        };

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.45))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(440.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(border)
                        .bg(cx.theme().background)
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_sm().child(SharedString::from(title)))
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
