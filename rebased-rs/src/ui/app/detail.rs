use gpui::{Context, Entity, Window};
use gpui_kit::component::list::{ListEvent, ListState};

use rebased_rs::git::Commit;

use crate::ui::commit_list::LogDelegate;

use super::{use_cases, AppView, PromptKind, SidebarMode};

impl AppView {
    pub(crate) fn load_commit_detail(&mut self, commit: Commit, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::load_commit_detail(repo.as_ref(), &mut self.state, commit) {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    pub(crate) fn clear_detail(&mut self, cx: &mut Context<Self>) {
        use_cases::clear_detail(&mut self.state);
        cx.notify();
    }

    pub(crate) fn on_list_event(
        &mut self,
        _entity: &Entity<ListState<LogDelegate>>,
        event: &ListEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
                let commit = self.list.read(cx).delegate().commit_at(ix.row);
                if let Some(commit) = commit {
                    let is_same = self
                        .state
                        .selected
                        .as_ref()
                        .is_some_and(|current| current.id.0 == commit.id.0);
                    if !is_same {
                        self.load_commit_detail(commit, cx);
                    }
                }
            }
            ListEvent::Cancel => self.clear_detail(cx),
        }
    }

    pub(crate) fn open_diff_worktree(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_worktree_diff(repo.as_ref(), &mut self.state, path) {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    pub(crate) fn open_commit_diff(
        &mut self,
        commit_id: String,
        path: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_commit_diff(repo.as_ref(), &mut self.state, commit_id, path)
        {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    pub(crate) fn open_blame(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_blame(repo.as_ref(), &mut self.state, path) {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    pub(crate) fn sidebar_back(&mut self, cx: &mut Context<Self>) {
        self.state.sidebar = if self.state.selected.is_some() {
            SidebarMode::Detail
        } else {
            SidebarMode::Workspace
        };
        cx.notify();
    }

    pub(crate) fn open_prompt(&mut self, kind: PromptKind, cx: &mut Context<Self>) {
        self.state.prompt = Some(kind);
        cx.notify();
    }

    pub(crate) fn cancel_prompt(&mut self, cx: &mut Context<Self>) {
        self.state.prompt = None;
        cx.notify();
    }

    pub(crate) fn reset_branch_to(
        &mut self,
        target: String,
        mode: ResetMode,
        cx: &mut Context<Self>,
    ) {
        self.state.prompt = None;
        let short = target[..target.len().min(7)].to_string();
        let message = format!("Reset to {short}");
        self.run_op(&message, move |repo| repo.reset_to(&target, mode), cx);
    }

    pub(crate) fn confirm_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.state.prompt.clone() else {
            return;
        };
        let input = self.prompt_input.read(cx).value().trim().to_string();
        self.prompt_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.state.prompt = None;
        match kind {
            PromptKind::NewBranch { start_point } => {
                if input.is_empty() {
                    self.state.error = Some("Branch name is empty".to_string());
                } else {
                    let message = format!("Created branch {input}");
                    self.run_op(
                        &message,
                        move |repo| {
                            repo.create_branch(&input, start_point.as_deref())?;
                            repo.checkout(&input)
                        },
                        cx,
                    );
                }
            }
            PromptKind::NewTag { commit_id } => {
                if input.is_empty() {
                    self.state.error = Some("Tag name is empty".to_string());
                } else {
                    let message = format!("Created tag {input}");
                    self.run_op(
                        &message,
                        move |repo| repo.create_tag(&input, Some(&commit_id), None),
                        cx,
                    );
                }
            }
            PromptKind::Stash => {
                let message = if input.is_empty() { None } else { Some(input) };
                self.run_op(
                    "Stashed",
                    move |repo| repo.stash_push(message.as_deref(), true),
                    cx,
                );
            }
            PromptKind::Reword { commit_id } => {
                if input.is_empty() {
                    self.state.error = Some("Commit message is empty".to_string());
                } else {
                    let message = format!("Reworded {}", &commit_id[..commit_id.len().min(7)]);
                    self.run_op(
                        &message,
                        move |repo| repo.reword_commit(&commit_id, &input),
                        cx,
                    );
                }
            }
            PromptKind::RenameBranch => {
                let Some(old) = self.state.current_branch.clone() else {
                    self.state.error = Some("No current branch".to_string());
                    cx.notify();
                    return;
                };
                if input.is_empty() {
                    self.state.error = Some("Branch name is empty".to_string());
                } else {
                    let message = format!("Renamed {old} to {input}");
                    self.run_op(&message, move |repo| repo.rename_branch(&old, &input), cx);
                }
            }
            PromptKind::Reset { .. } | PromptKind::Confirm(_) => {}
        }
        cx.notify();
    }
}
