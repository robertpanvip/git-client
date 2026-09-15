use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled,
};
use gpui_kit::component::{button::{Button, ButtonVariants}, input::Textarea, ActiveTheme};

use rebased_rs::git::{HunkChoice, ResetMode};

use crate::ui::blame_view::render_blame;
use crate::ui::diff_view::render_diff_files;

use super::{AppView, SidebarMode};

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
                    .child("Click the action to cycle Pick → Squash → Fixup → Drop. Use ↑ ↓ to reorder."),
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
            let summary = format!(
                "{} {}",
                &action.id[..action.id.len().min(7)],
                action.subject
            );
            panel = panel.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(border)
                    .pb_1()
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
                .flex_row()
                .gap_2()
                .mt_1()
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
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("Ours:\n{}", ours_text)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("Theirs:\n{}", theirs_text)),
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

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let width = match self.state.sidebar {
            SidebarMode::Workspace => 360.,
            SidebarMode::Detail => 420.,
            SidebarMode::Diff | SidebarMode::Blame => 680.,
            SidebarMode::Rebase => 480.,
            SidebarMode::Conflicts => 680.,
            SidebarMode::Shelve => 420.,
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
            SidebarMode::Rebase => base.child(self.render_rebase_panel(cx)).into_any_element(),
            SidebarMode::Conflicts => {
                base.child(self.render_conflicts_panel(cx)).into_any_element()
            }
            SidebarMode::Shelve => base.child(self.render_shelve_panel(cx)).into_any_element(),
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
                    .child(
                        Button::new("diff-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.state.diff_files.is_empty() {
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
            panel = panel.child(
                div()
                    .id("diff-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_diff_files(&self.state.diff_files, cx)),
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
            panel = panel.child(
                div()
                    .id("blame-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_blame(&self.state.blame_groups, cx)),
            );
        }
        panel
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
            super::PromptKind::Reset { commit_id } => (
                "Reset current branch to here".to_string(),
                format!(
                    "Move the current branch to {}. Pick a mode: Soft keeps everything staged, Mixed keeps changes unstaged, Hard discards all changes.",
                    &commit_id[..commit_id.len().min(7)]
                ),
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
                    ),
            }
            super::PromptKind::Confirm(action) => {
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
                    ),
            }
            _ => {
                let ok_label = match &kind {
                    super::PromptKind::NewBranch { .. } => "Create",
                    super::PromptKind::NewTag { .. } => "Tag",
                    super::PromptKind::Stash => "Stash",
                    super::PromptKind::Reword { .. } => "Reword",
                    super::PromptKind::RenameBranch => "Rename",
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
