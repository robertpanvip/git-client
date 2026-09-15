use gpui::{Context, Entity, Window};
use gpui_kit::component::list::{ListEvent, ListState};

use rebased_rs::git::{Commit, parse_unified_diff};

use crate::ui::commit_list::LogDelegate;

use super::{AppView, PromptKind};

impl AppView {
    pub(crate) fn load_commit_detail(&mut self, commit: Commit, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let id = commit.id.0.clone();
        let files = repo.show_files(&id);
        let branches = repo.branches_containing(&id);
        match (files, branches) {
            (Ok(files), Ok(branches)) => {
                self.selected = Some(commit);
                self.detail_files = files;
                self.detail_branches = branches;
                self.sidebar = super::SidebarMode::Detail;
                self.error = None;
            }
            (Err(e), _) | (_, Err(e)) => {
                self.error = Some(e.to_string().into());
            }
        }
        cx.notify();
    }

    pub(crate) fn clear_detail(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.detail_files.clear();
        self.detail_branches.clear();
        self.sidebar = super::SidebarMode::Workspace;
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
        match repo.diff_head(path.as_deref()) {
            Ok(stdout) => {
                self.diff_files = parse_unified_diff(&stdout);
                self.diff_title = match &path {
                    Some(p) => format!("Diff · {p}"),
                    None => "Diff · working tree".to_string(),
                };
                self.diff_path = path;
                self.sidebar = super::SidebarMode::Diff;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string().into()),
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
        let short = &commit_id[..commit_id.len().min(7)];
        match repo.show_diff(&commit_id, path.as_deref()) {
            Ok(stdout) => {
                self.diff_files = parse_unified_diff(&stdout);
                self.diff_title = match &path {
                    Some(p) => format!("{short} · {p}"),
                    None => format!("Commit {short}"),
                };
                self.diff_path = None;
                self.sidebar = super::SidebarMode::Diff;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string().into()),
        }
        cx.notify();
    }

    pub(crate) fn open_blame(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.blame("HEAD", &path) {
            Ok(groups) => {
                self.blame_groups = groups;
                self.blame_path = path;
                self.sidebar = super::SidebarMode::Blame;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string().into()),
        }
        cx.notify();
    }

    pub(crate) fn sidebar_back(&mut self, cx: &mut Context<Self>) {
        self.sidebar = if self.selected.is_some() {
            super::SidebarMode::Detail
        } else {
            super::SidebarMode::Workspace
        };
        cx.notify();
    }

    pub(crate) fn open_prompt(&mut self, kind: PromptKind, cx: &mut Context<Self>) {
        self.prompt = Some(kind);
        cx.notify();
    }

    pub(crate) fn cancel_prompt(&mut self, cx: &mut Context<Self>) {
        self.prompt = None;
        cx.notify();
    }

    pub(crate) fn confirm_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.prompt.clone() else {
            return;
        };
        let input = self.prompt_input.read(cx).value().trim().to_string();
        self.prompt_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.prompt = None;
        match kind {
            PromptKind::NewBranch { start_point } => {
                if input.is_empty() {
                    self.error = Some("Branch name is empty".into());
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
                    self.error = Some("Tag name is empty".into());
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
                    self.error = Some("Commit message is empty".into());
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
                let Some(old) = self.current_branch.clone() else {
                    self.error = Some("No current branch".into());
                    cx.notify();
                    return;
                };
                if input.is_empty() {
                    self.error = Some("Branch name is empty".into());
                } else {
                    let message = format!("Renamed {old} to {input}");
                    self.run_op(&message, move |repo| repo.rename_branch(&old, &input), cx);
                }
            }
        }
        cx.notify();
    }
}
