use gpui::{Context, Entity, Window};
use gpui_kit::component::list::{ListEvent, ListState};

use rebased_rs::git::{Commit, MergeMode, RebaseActionKind, ResetMode};

use crate::ui::commit_list::LogDelegate;
use crate::ui::i18n::tr;

use super::{use_cases, AppView, DiffSource, PromptKind, SidebarMode};

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

    pub(crate) fn open_staged_diff(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_staged_diff(repo.as_ref(), &mut self.state, path) {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    pub(crate) fn open_unstaged_diff(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_unstaged_diff(repo.as_ref(), &mut self.state, path) {
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

    /// 切换「忽略空白」开关，并按当前 diff 来源重新加载。
    pub(crate) fn toggle_ignore_whitespace(&mut self, cx: &mut Context<Self>) {
        self.state.ignore_whitespace = !self.state.ignore_whitespace;
        let path = self.state.diff_path.clone();
        match self.state.diff_source {
            Some(DiffSource::Staged) => self.open_staged_diff(path, cx),
            Some(DiffSource::Unstaged) => self.open_unstaged_diff(path, cx),
            Some(DiffSource::Commit) => {
                let Some(id) = self.state.diff_commit.clone() else {
                    cx.notify();
                    return;
                };
                self.open_commit_diff(id, None, cx);
            }
            None => cx.notify(),
        }
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

    pub(crate) fn open_file_history(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_file_history(repo.as_ref(), &mut self.state, path) {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    pub(crate) fn open_reflog(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = use_cases::open_reflog(repo.as_ref(), &mut self.state) {
            self.state.error = Some(e.to_string());
        }
        cx.notify();
    }

    /// Amend 开启时，把当前 HEAD 的完整提交消息预填到消息输入框。
    pub(crate) fn prefill_amend_message(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.head_message() {
            Ok(message) if !message.is_empty() => {
                self.message_input
                    .update(cx, |state, cx| state.set_value(&message, window, cx));
            }
            _ => {}
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

    pub(crate) fn open_rename_branch_by_name(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prompt_input
            .update(cx, |state, cx| state.set_value(&name, window, cx));
        self.state.prompt = Some(PromptKind::RenameBranchByName { name });
        cx.notify();
    }

    pub(crate) fn open_merge_message(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .state
            .current_branch
            .clone()
            .unwrap_or_else(|| "HEAD".to_string());
        let prefill = format!("Merge branch '{name}' into {target}");
        self.prompt_input
            .update(cx, |state, cx| state.set_value(&prefill, window, cx));
        self.state.prompt = Some(PromptKind::MergeMessage { name });
        cx.notify();
    }

    pub(crate) fn open_diff_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let Some(path) = self.state.diff_path.clone() else {
            return;
        };
        match repo.worktree_file_content(&path) {
            Ok(content) => {
                self.diff_edit_input
                    .update(cx, |state, cx| state.set_value(&content, window, cx));
                self.state.diff_editing = true;
            }
            Err(e) => {
                self.state.error =
                    Some(format!("{} {path}: {e}", tr("Cannot edit", "无法编辑")));
            }
        }
        cx.notify();
    }

    pub(crate) fn save_diff_edit(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.diff_path.clone() else {
            return;
        };
        let content = self.diff_edit_input.read(cx).value().to_string();
        self.state.diff_editing = false;
        let message = format!("{} {path}", tr("Saved", "已保存"));
        self.run_op(&message, move |repo| {
            repo.write_worktree_file(&path, &content)
        }, cx);
    }

    pub(crate) fn cancel_diff_edit(&mut self, cx: &mut Context<Self>) {
        self.state.diff_editing = false;
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
        let message = format!("{} {short}", tr("Reset to", "已重置到"));
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
                    self.state.error =
                        Some(tr("Branch name is empty", "分支名称为空").to_string());
                } else {
                    let message = format!("{} {input}", tr("Created branch", "已创建分支"));
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
                    self.state.error =
                        Some(tr("Tag name is empty", "标签名称为空").to_string());
                } else {
                    let message = format!("{} {input}", tr("Created tag", "已创建标签"));
                    self.run_op(
                        &message,
                        move |repo| repo.create_tag(&input, Some(&commit_id), None),
                        cx,
                    );
                }
            }
            PromptKind::EditTag { name, commit_id } => {
                if input.is_empty() {
                    self.state.error =
                        Some(tr("Tag message is empty", "标签消息为空").to_string());
                } else {
                    let message = format!("{} {name}", tr("Updated tag", "已更新标签"));
                    self.run_op(
                        &message,
                        move |repo| repo.recreate_tag(&name, &commit_id, &input),
                        cx,
                    );
                }
            }
            PromptKind::Stash => {
                let message = if input.is_empty() { None } else { Some(input) };
                self.run_op(
                    tr("Stashed", "已贮藏"),
                    move |repo| repo.stash_push(message.as_deref(), true),
                    cx,
                );
            }
            PromptKind::Reword { commit_id } => {
                if input.is_empty() {
                    self.state.error =
                        Some(tr("Commit message is empty", "提交信息为空").to_string());
                } else {
                    let message = format!(
                        "{} {}",
                        tr("Reworded", "已改写"),
                        &commit_id[..commit_id.len().min(7)]
                    );
                    self.run_op(
                        &message,
                        move |repo| repo.reword_commit(&commit_id, &input),
                        cx,
                    );
                }
            }
            PromptKind::RenameBranch => {
                let Some(old) = self.state.current_branch.clone() else {
                    self.state.error =
                        Some(tr("No current branch", "没有当前分支").to_string());
                    cx.notify();
                    return;
                };
                if input.is_empty() {
                    self.state.error =
                        Some(tr("Branch name is empty", "分支名称为空").to_string());
                } else {
                    let message = format!("{} {old} → {input}", tr("Renamed", "已重命名"));
                    self.run_op(&message, move |repo| repo.rename_branch(&old, &input), cx);
                }
            }
            PromptKind::RenameBranchByName { name } => {
                if input.is_empty() {
                    self.state.error =
                        Some(tr("Branch name is empty", "分支名称为空").to_string());
                } else {
                    let message = format!("{} {name} → {input}", tr("Renamed", "已重命名"));
                    self.run_op(&message, move |repo| repo.rename_branch(&name, &input), cx);
                }
            }
            PromptKind::MergeMessage { name } => {
                let message = if input.is_empty() { None } else { Some(input) };
                self.merge_branch_into_current(name, MergeMode::NoFastForward, message, cx);
            }
            PromptKind::Reset { .. } | PromptKind::Confirm(_) => {}
            PromptKind::RebaseEdit { index } => {
                if input.is_empty() {
                    self.state.error =
                        Some(tr("Commit message is empty", "提交信息为空").to_string());
                } else if let super::RebaseFlow::Planning { plan, .. } = &mut self.state.rebase
                    && let Some(action) = plan.get_mut(index)
                {
                    action.kind = RebaseActionKind::Reword;
                    action.message = Some(input);
                }
            }
            PromptKind::GoTo => {
                if input.is_empty() {
                    self.state.error =
                        Some(tr("Revision is empty", "修订版本为空").to_string());
                } else {
                    self.goto_revision(input, cx);
                }
            }
            PromptKind::FilterAuthor => {
                self.set_author_filter(input, cx);
            }
            PromptKind::SetUpstream { branch } => {
                if input.is_empty() {
                    self.state.error = Some(tr("Upstream is empty", "上游为空").to_string());
                    cx.notify();
                } else {
                    self.set_branch_upstream(branch, input, cx);
                }
            }
            PromptKind::AddRemote => {
                let url = self.prompt_input2.read(cx).value().trim().to_string();
                self.prompt_input2
                    .update(cx, |state, cx| state.set_value("", window, cx));
                if input.is_empty() || url.is_empty() {
                    self.state.error = Some(
                        tr(
                            "Remote name and URL are required",
                            "远程名称和 URL 为必填项",
                        )
                        .to_string(),
                    );
                } else {
                    let message = format!("{} {input}", tr("Added remote", "已添加远程"));
                    self.run_op(&message, move |repo| repo.remote_add(&input, &url), cx);
                }
            }
        }
        cx.notify();
    }
}
