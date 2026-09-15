use gpui::{actions, App, AppContext, ClipboardItem, Context, KeyBinding, Window};
use gpui_kit::base::IndexPath;

use rebased_rs::git::{Change, MergeMode, DEFAULT_LOG_LIMIT};

use super::{use_cases::commit_selected, AppView, ConfirmAction, SidebarMode};

actions!(
    rebased_rs,
    [
        CommitSelected,
        PushBranch,
        PullBranch,
        RefreshRepo,
        CloseOverlay,
        SelectPrevCommit,
        SelectNextCommit,
    ]
);

/// 全局快捷键。焦点在输入框/列表内时，组件自身的绑定（光标移动、列表上下键、
/// Esc 取消）更具体、优先生效；未被消费的按键（如输入框内按 Esc）会向上传播
/// 到这里，因此输入框中按 Esc 也能关闭弹窗。
pub(crate) fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CloseOverlay, None),
        KeyBinding::new("ctrl-enter", CommitSelected, None),
        KeyBinding::new("ctrl-shift-k", PushBranch, None),
        KeyBinding::new("ctrl-t", PullBranch, None),
        KeyBinding::new("f5", RefreshRepo, None),
        KeyBinding::new("ctrl-r", RefreshRepo, None),
        KeyBinding::new("up", SelectPrevCommit, None),
        KeyBinding::new("down", SelectNextCommit, None),
    ]);
}

impl AppView {
    pub(crate) fn toggle_stage(&mut self, change: &Change, cx: &mut Context<Self>) {
        let path = change.path.clone();
        if change.staged {
            self.run_op("Unstaged", move |repo| repo.reset(&[path.as_str()]), cx);
        } else {
            self.run_op("Staged", move |repo| repo.add(&[path.as_str()]), cx);
        }
    }

    pub(crate) fn toggle_change_selection(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(pos) = self
            .state
            .selected_changes
            .iter()
            .position(|selected| selected == path)
        {
            self.state.selected_changes.remove(pos);
        } else {
            self.state.selected_changes.push(path.to_string());
        }
        cx.notify();
    }

    pub(crate) fn do_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message_input.read(cx).value().to_string();
        if message.trim().is_empty() {
            self.state.error = Some("Commit message is empty".to_string());
            cx.notify();
            return;
        }
        let amend = self.state.amend;
        let selected = self.state.selected_changes.clone();
        self.message_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.state.amend = false;
        self.run_op(
            "Committed",
            move |repo| commit_selected(repo, &message, amend, &selected),
            cx,
        );
    }

    pub(crate) fn do_push(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
            cx.notify();
            return;
        };
        let set_upstream = self.state.current_upstream.is_none();
        self.run_op("Pushed", move |repo| repo.push(&branch, set_upstream), cx);
    }

    pub(crate) fn do_pull(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
            cx.notify();
            return;
        };
        self.run_op("Pulled", move |repo| repo.pull(&branch), cx);
    }

    pub(crate) fn checkout_branch(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Checked out {name}");
        self.run_op(&message, move |repo| repo.checkout(&name), cx);
    }

    /// 把指定分支（通常是远程分支）的最新提交合入当前分支：先 fetch 再 merge。
    pub(crate) fn pull_branch_into_current(&mut self, name: String, cx: &mut Context<Self>) {
        let target = self
            .state
            .current_branch
            .clone()
            .unwrap_or_else(|| "HEAD".to_string());
        let message = format!("Pulled {name} into {target}");
        self.run_op(
            &message,
            move |repo| {
                repo.fetch()?;
                repo.merge_branch(&name)
            },
            cx,
        );
    }

    /// 以指定分支为基打开交互式 rebase 计划（Rebase current onto …）。
    pub(crate) fn rebase_current_onto(&mut self, base: String, cx: &mut Context<Self>) {
        self.start_rebase(base, cx);
    }

    /// 设置分支范围过滤器并重新加载日志（None = 所有分支）。
    pub(crate) fn set_branch_filter(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        self.state.filter_branch = branch;
        self.refresh(cx);
    }

    /// 设置作者过滤器并重新加载日志（空串 = 不过滤）。
    pub(crate) fn set_author_filter(&mut self, author: String, cx: &mut Context<Self>) {
        self.state.filter_author = author;
        self.refresh(cx);
    }

    /// 设置日期过滤器并重新加载日志（None = 不限时间）。
    pub(crate) fn set_date_filter(
        &mut self,
        filter: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.state.filter_since = filter;
        self.refresh(cx);
    }

    /// Go to Hash/Branch/Tag：把输入解析为提交 id 并在日志中选中。
    pub(crate) fn goto_revision(&mut self, input: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.rev_parse(&input) {
            Ok(id) => self.select_commit_by_id(&id, cx),
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn select_commit_by_id(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.list.read(cx).delegate().find_commit(id) {
            Some(commit) => self.load_commit_detail(commit, cx),
            None => {
                self.state.error = Some(format!("Commit {id} not in loaded history"));
                cx.notify();
            }
        }
    }

    /// 打开分支对比面板：ahead 为 mine（当前分支）独有，behind 为 theirs 独有。
    pub(crate) fn open_branch_compare(&mut self, theirs: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if self.state.busy.is_some() {
            return;
        }
        let mine = self
            .state
            .current_branch
            .clone()
            .unwrap_or_else(|| "HEAD".to_string());
        self.state.busy = Some(format!("Comparing {mine} with {theirs}"));
        cx.notify();
        let task = cx.background_spawn(async move {
            let compare = repo.compare_branches(&mine, &theirs, DEFAULT_LOG_LIMIT);
            (mine, theirs, compare)
        });
        cx.spawn(async move |this, cx| {
            let (mine, theirs, compare) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.busy = None;
                match compare {
                    Ok((ahead, behind)) => {
                        this.state.compare_mine = mine;
                        this.state.compare_theirs = theirs;
                        this.state.compare_ahead = ahead;
                        this.state.compare_behind = behind;
                        this.state.sidebar = SidebarMode::Compare;
                        this.state.error = None;
                    }
                    Err(e) => this.state.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cherry_pick_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.state.selected.clone() else {
            return;
        };
        self.cherry_pick_commit(commit.id.0, cx);
    }

    pub(crate) fn cherry_pick_commit(&mut self, id: String, cx: &mut Context<Self>) {
        let message = format!("Cherry-picked {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.cherry_pick(&id), cx);
    }

    pub(crate) fn revert_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.state.selected.clone() else {
            return;
        };
        self.revert_commit(commit.id.0, cx);
    }

    pub(crate) fn revert_commit(&mut self, id: String, cx: &mut Context<Self>) {
        let message = format!("Reverted {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.revert(&id), cx);
    }

    pub(crate) fn checkout_commit(&mut self, id: String, cx: &mut Context<Self>) {
        let message = format!("Checked out {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.checkout(&id), cx);
    }

    pub(crate) fn delete_branch(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Deleted branch {name}");
        self.run_op(&message, move |repo| repo.delete_branch(&name, false), cx);
    }

    pub(crate) fn delete_tag(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Deleted tag {name}");
        self.run_op(&message, move |repo| repo.delete_tag(&name), cx);
    }

    pub(crate) fn merge_branch_into_current(
        &mut self,
        name: String,
        mode: MergeMode,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let op_message = match mode {
            MergeMode::Default => format!("Merged {name}"),
            MergeMode::NoFastForward => format!("Merged {name} (no ff)"),
            MergeMode::FastForwardOnly => format!("Fast-forwarded {name}"),
        };
        self.run_op(&op_message, move |repo| {
            repo.merge_branch_with_message(&name, mode, message.as_deref())
        }, cx);
    }

    pub(crate) fn abort_merge(&mut self, cx: &mut Context<Self>) {
        self.run_op("Merge aborted", |repo| repo.merge_abort(), cx);
    }

    pub(crate) fn continue_merge_op(&mut self, cx: &mut Context<Self>) {
        self.run_op("Merge continued", |repo| repo.merge_continue(), cx);
    }

    pub(crate) fn undo_head(&mut self, cx: &mut Context<Self>) {
        self.run_op(
            "Undid HEAD commit (changes kept staged)",
            |repo| repo.undo_head_commit(),
            cx,
        );
    }

    pub(crate) fn drop_head(&mut self, cx: &mut Context<Self>) {
        self.run_op("Dropped HEAD commit", |repo| repo.drop_head_commit(), cx);
    }

    pub(crate) fn force_push_current(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
            cx.notify();
            return;
        };
        self.run_op(
            "Force pushed (with lease)",
            move |repo| repo.push_force(&branch),
            cx,
        );
    }

    pub(crate) fn push_all_tags(&mut self, cx: &mut Context<Self>) {
        self.run_op("Pushed all tags", |repo| repo.push_tags(), cx);
    }

    pub(crate) fn copy_commit_id(&mut self, id: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(id));
        self.state.error = None;
        self.state.status_message = "Commit SHA copied".to_string();
        cx.notify();
    }

    pub(crate) fn copy_commit_sha(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.state.selected.clone() else {
            return;
        };
        self.copy_commit_id(commit.id.0, cx);
    }

    pub(crate) fn confirm_action(&mut self, action: ConfirmAction, cx: &mut Context<Self>) {
        match action {
            ConfirmAction::ForcePush => self.force_push_current(cx),
            ConfirmAction::DeleteBranch { name } => self.delete_branch(&name, cx),
            ConfirmAction::DeleteTag { name } => self.delete_tag(&name, cx),
            ConfirmAction::DropHeadCommit => self.drop_head(cx),
            ConfirmAction::UndoHeadCommit => self.undo_head(cx),
            ConfirmAction::DiscardChanges { path } => {
                let message = format!("Discarded {path}");
                self.run_op(&message, move |repo| repo.discard_changes(&path), cx);
            }
        }
    }

    pub(crate) fn on_commit_selected(
        &mut self,
        _: &CommitSelected,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.prompt.is_some() {
            self.confirm_prompt(window, cx);
            return;
        }
        self.do_commit(window, cx);
    }

    pub(crate) fn on_push_branch(
        &mut self,
        _: &PushBranch,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.do_push(cx);
    }

    pub(crate) fn on_pull_branch(
        &mut self,
        _: &PullBranch,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.do_pull(cx);
    }

    pub(crate) fn on_refresh_repo(
        &mut self,
        _: &RefreshRepo,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh(cx);
    }

    pub(crate) fn on_close_overlay(
        &mut self,
        _: &CloseOverlay,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.prompt.is_some() {
            self.cancel_prompt(cx);
            return;
        }
        if matches!(
            self.state.sidebar,
            SidebarMode::Diff | SidebarMode::Blame | SidebarMode::History
        ) {
            self.sidebar_back(cx);
        }
    }

    pub(crate) fn on_select_prev_commit(
        &mut self,
        _: &SelectPrevCommit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_commit_selection(-1, window, cx);
    }

    pub(crate) fn on_select_next_commit(
        &mut self,
        _: &SelectNextCommit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_commit_selection(1, window, cx);
    }

    /// 沿提交历史移动键盘选择（↑↓），并联动 Detail 视图。
    fn move_commit_selection(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.list.read(cx).delegate().visible_count();
        if count == 0 {
            return;
        }
        let current = self
            .list
            .read(cx)
            .selected_index()
            .map_or(-1, |ix| ix.row as isize);
        let next = (current + delta).clamp(0, count as isize - 1) as usize;
        if next as isize == current {
            return;
        }
        self.list.update(cx, |list, cx| {
            list.set_selected_index(
                Some(IndexPath {
                    section: 0,
                    row: next,
                    column: 0,
                }),
                window,
                cx,
            );
            list.scroll_to_selected_item(window, cx);
        });
        if let Some(commit) = self.list.read(cx).delegate().commit_at(next) {
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
}
