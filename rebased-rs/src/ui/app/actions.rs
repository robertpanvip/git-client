use gpui::{Action, App, AppContext, ClipboardItem, Context, KeyBinding, Window, actions};
use gpui_kit::base::IndexPath;

use rebased_rs::git::{Change, DEFAULT_LOG_LIMIT, MergeMode};

use crate::ui::diff_view::{scroll_hunk_into_view, step_hunk};

use super::{AppView, ConfirmAction, DiffSource, SidebarMode, use_cases::commit_selected};

actions!(
    rebased_rs,
    [
        CommitSelected,
        PushBranch,
        PullBranch,
        RefreshRepo,
        CloseOverlay,
        ConfirmPrompt,
        SelectPrevCommit,
        SelectNextCommit,
        FocusComposer,
        ToggleVcsPalette,
        BlameCurrentFile,
        NextDiffHunk,
        PrevDiffHunk,
    ]
);

/// Ctrl+Alt+数字：直接切换到对应侧栏面板（对标 JetBrains Alt+数字工具窗口）。
/// `no_json`：本应用不从 JSON 加载键位，省去 serde/schemars 依赖。
#[derive(Clone, PartialEq, Debug, Action)]
#[action(namespace = rebased_rs, no_json)]
pub struct SelectSidebarPanel(pub u8);

/// 全局快捷键。焦点在输入框/列表内时，组件自身的绑定（光标移动、列表上下键、
/// Esc 取消）更具体、优先生效；未被消费的按键（如输入框内按 Esc）会向上传播
/// 到这里，因此输入框中按 Esc 也能关闭弹窗。
pub(crate) fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CloseOverlay, None),
        // 对话框打开时 Enter 确认（IntelliJ 默认按钮行为）；多行输入内 Enter
        // 被组件消费为换行，不会传播到这里。
        KeyBinding::new("enter", ConfirmPrompt, None),
        KeyBinding::new("ctrl-enter", CommitSelected, None),
        KeyBinding::new("ctrl-k", FocusComposer, None),
        KeyBinding::new("ctrl-shift-k", PushBranch, None),
        KeyBinding::new("ctrl-t", PullBranch, None),
        KeyBinding::new("f5", RefreshRepo, None),
        KeyBinding::new("ctrl-r", RefreshRepo, None),
        KeyBinding::new("up", SelectPrevCommit, None),
        KeyBinding::new("down", SelectNextCommit, None),
        KeyBinding::new("alt-backtick", ToggleVcsPalette, None),
        KeyBinding::new("ctrl-alt-b", BlameCurrentFile, None),
        // IDEA diff 导航：F7 下一处、Shift+F7 上一处（循环）。
        KeyBinding::new("f7", NextDiffHunk, None),
        KeyBinding::new("shift-f7", PrevDiffHunk, None),
        KeyBinding::new("ctrl-alt-1", SelectSidebarPanel(1), None),
        KeyBinding::new("ctrl-alt-2", SelectSidebarPanel(2), None),
        KeyBinding::new("ctrl-alt-3", SelectSidebarPanel(3), None),
        KeyBinding::new("ctrl-alt-4", SelectSidebarPanel(4), None),
        KeyBinding::new("ctrl-alt-5", SelectSidebarPanel(5), None),
        KeyBinding::new("ctrl-alt-6", SelectSidebarPanel(6), None),
        KeyBinding::new("ctrl-alt-7", SelectSidebarPanel(7), None),
        KeyBinding::new("ctrl-alt-8", SelectSidebarPanel(8), None),
        KeyBinding::new("ctrl-alt-9", SelectSidebarPanel(9), None),
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

    /// hunk 级暂存/取消暂存（diff 面板按钮）：
    /// Unstaged 来源 → Stage（`git apply --cached`）；Staged 来源 → Unstage（`-R` 撤回）。
    /// git apply 毫秒级完成，与 open_*_diff 一样在 UI 线程同步执行，完成后重开当前 diff 刷新面板。
    pub(crate) fn toggle_hunk_stage(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self.state.diff_source else {
            return;
        };
        let Some(file) = self.state.diff_files.get(file_index).cloned() else {
            return;
        };
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let result = match source {
            DiffSource::Unstaged => repo.apply_hunk_to_index(&file, hunk_index),
            DiffSource::Staged => repo.revert_hunk_from_index(&file, hunk_index),
            DiffSource::Commit => return,
        };
        if let Err(e) = result {
            self.state.error = Some(e.to_string());
            cx.notify();
            return;
        }
        self.state.error = None;
        let path = self.state.diff_path.clone();
        match source {
            DiffSource::Unstaged => self.open_unstaged_diff(path, cx),
            DiffSource::Staged => self.open_staged_diff(path, cx),
            DiffSource::Commit => {}
        }
    }

    /// hunk 级「左栏内容同步到右栏」（diff 面板箭头按钮，对齐 IntelliJ
    /// change marker 的 revert 箭头）：Unstaged 来源 = 把工作区该 hunk
    /// 还原成 index 版本（`git apply -R`，不动 index）；Staged/Commit 只读
    /// 不提供（Staged 的等价操作是 Unstage）。完成后重开当前 diff 刷新。
    pub(crate) fn sync_hunk_from_left(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self.state.diff_source else {
            return;
        };
        if source != DiffSource::Unstaged {
            return;
        }
        let Some(file) = self.state.diff_files.get(file_index).cloned() else {
            return;
        };
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if let Err(e) = repo.revert_hunk_in_worktree(&file, hunk_index) {
            self.state.error = Some(e.to_string());
            cx.notify();
            return;
        }
        self.state.error = None;
        let path = self.state.diff_path.clone();
        self.open_unstaged_diff(path, cx);
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

    /// 批量设置一组变更的勾选状态（分组标题上的"全选"复选框）。
    pub(crate) fn set_changes_selection(
        &mut self,
        paths: &[String],
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        for path in paths {
            let pos = self
                .state
                .selected_changes
                .iter()
                .position(|current| current == path);
            if selected {
                if pos.is_none() {
                    self.state.selected_changes.push(path.clone());
                }
            } else if let Some(pos) = pos {
                self.state.selected_changes.remove(pos);
            }
        }
        cx.notify();
    }

    pub(crate) fn do_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message_input.read(cx).value().to_string();
        // 「允许空提交信息」开启时跳过 IDEA 的空信息检查（提交设置项）。
        if message.trim().is_empty() && !self.state.allow_empty_commit_message {
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

    /// 提交并推送（IDEA Commit 按钮下拉里的 Commit and Push）：单个后台闭包串行完成，
    /// 保证 busy 状态下不会与后续 push 产生并发写竞争。
    pub(crate) fn do_commit_and_push(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = self.message_input.read(cx).value().to_string();
        if message.trim().is_empty() && !self.state.allow_empty_commit_message {
            self.state.error = Some("Commit message is empty".to_string());
            cx.notify();
            return;
        }
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
            cx.notify();
            return;
        };
        let set_upstream = self.state.current_upstream.is_none();
        let amend = self.state.amend;
        let selected = self.state.selected_changes.clone();
        self.message_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.state.amend = false;
        self.run_op(
            "Committed & pushed",
            move |repo| {
                commit_selected(repo, &message, amend, &selected)?;
                repo.push(&branch, set_upstream)
            },
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

    /// 推送任意本地分支；无 upstream 时附带 --set-upstream。
    pub(crate) fn push_branch(&mut self, name: String, set_upstream: bool, cx: &mut Context<Self>) {
        let message = if set_upstream {
            format!("Pushed {name} (set upstream)")
        } else {
            format!("Pushed {name}")
        };
        self.run_op(&message, move |repo| repo.push(&name, set_upstream), cx);
    }

    pub(crate) fn do_pull(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
            cx.notify();
            return;
        };
        self.run_op_progress(
            "Pull",
            "Pulled",
            move |repo, progress, cancel| repo.pull_with_control(&branch, progress, cancel),
            cx,
        );
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

    pub(crate) fn remove_remote(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Removed remote {name}");
        self.run_op(&message, move |repo| repo.remote_remove(&name), cx);
    }

    pub(crate) fn prune_remote(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Pruned remote {name}");
        self.run_op(&message, move |repo| repo.remote_prune(&name), cx);
    }

    /// 设置分支上游（upstream 形如 `origin/main`）。
    pub(crate) fn set_branch_upstream(
        &mut self,
        branch: String,
        upstream: String,
        cx: &mut Context<Self>,
    ) {
        let message = format!("Set upstream of {branch} to {upstream}");
        self.run_op(
            &message,
            move |repo| repo.set_upstream(&branch, &upstream),
            cx,
        );
    }

    pub(crate) fn unset_branch_upstream(&mut self, branch: String, cx: &mut Context<Self>) {
        let message = format!("Unset upstream of {branch}");
        self.run_op(&message, move |repo| repo.unset_upstream(&branch), cx);
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
        self.run_op(
            &op_message,
            move |repo| repo.merge_branch_with_message(&name, mode, message.as_deref()),
            cx,
        );
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

    pub(crate) fn drop_commit_named(&mut self, commit: String, cx: &mut Context<Self>) {
        let short = commit[..commit.len().min(7)].to_string();
        let message = format!("Dropped {short}");
        self.run_op(&message, move |repo| repo.drop_commit(&commit), cx);
    }

    pub(crate) fn uncommit_commit_named(&mut self, commit: String, cx: &mut Context<Self>) {
        let short = commit[..commit.len().min(7)].to_string();
        let message = format!("Uncommitted {short} (changes kept staged)");
        self.run_op(&message, move |repo| repo.uncommit_commit(&commit), cx);
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

    pub(crate) fn push_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        let message = format!("Pushed tag {tag}");
        self.run_op(&message, move |repo| repo.push_tag(&tag), cx);
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
            ConfirmAction::RemoveRemote { name } => self.remove_remote(&name, cx),
            ConfirmAction::DropHeadCommit => self.drop_head(cx),
            ConfirmAction::UndoHeadCommit => self.undo_head(cx),
            ConfirmAction::DropCommit { commit_id } => self.drop_commit_named(commit_id, cx),
            ConfirmAction::UncommitCommit { commit_id } => {
                self.uncommit_commit_named(commit_id, cx);
            }
            ConfirmAction::DiscardChanges { path } => {
                let message = format!("Discarded {path}");
                self.run_op(&message, move |repo| repo.discard_changes(&path), cx);
            }
            ConfirmAction::RollbackChanges { paths } => {
                let message = format!("Rolled back {} file(s)", paths.len());
                self.run_op(&message, move |repo| {
                    for path in &paths {
                        repo.discard_changes(path)?;
                    }
                    Ok(())
                }, cx);
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

    /// 对话框打开时 Enter 触发主按钮；无对话框时是普通按键，不做任何事。
    pub(crate) fn on_confirm_prompt(
        &mut self,
        _: &ConfirmPrompt,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.prompt.is_some() {
            self.confirm_prompt(window, cx);
        }
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

    /// F7 / 面板 ↓：定位下一个 hunk（目标已折叠时自动展开）。
    pub(crate) fn on_next_diff_hunk(
        &mut self,
        _: &NextDiffHunk,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_diff_hunk(true, cx);
    }

    /// Shift+F7 / 面板 ↑：定位上一个 hunk（目标已折叠时自动展开）。
    pub(crate) fn on_prev_diff_hunk(
        &mut self,
        _: &PrevDiffHunk,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_diff_hunk(false, cx);
    }

    /// hunk 循环导航共用体：更新导航游标 → 展开目标 → 算术滚动定位 → 重绘。
    pub(crate) fn step_diff_hunk(&mut self, forward: bool, cx: &mut Context<Self>) {
        let target = {
            let files = &self.state.diff_files;
            step_hunk(files, self.state.diff_nav, forward)
        };
        let Some(target) = target else {
            return;
        };
        self.state.diff_nav = Some(target);
        self.state.diff_folded.remove(&target);
        scroll_hunk_into_view(
            &self.diff_scroll,
            &self.state.diff_files,
            target,
            self.state.diff_side_by_side,
            &self.state.diff_folded,
        );
        cx.notify();
    }

    /// hunk header chevron：折叠/展开（IDEA diff）。集合外 = 展开。
    pub(crate) fn toggle_hunk_fold(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        cx: &mut Context<Self>,
    ) {
        let key = (file_index, hunk_index);
        if !self.state.diff_folded.remove(&key) {
            self.state.diff_folded.insert(key);
        }
        cx.notify();
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
        if self.state.vcs_palette {
            self.state.vcs_palette = false;
            cx.notify();
            return;
        }
        if self.state.branch_popup {
            self.state.branch_popup = false;
            cx.notify();
            return;
        }
        if matches!(
            self.state.sidebar,
            SidebarMode::Diff | SidebarMode::Blame | SidebarMode::History
        ) {
            self.sidebar_back(cx);
        }
    }

    /// Alt+`：开关 VCS 操作快切弹层（对标 JetBrains VCS Operations Popup）。
    /// 打开时先关掉 prompt——两个模态互斥，避免焦点与按键分发混乱。
    pub(crate) fn on_toggle_vcs_palette(
        &mut self,
        _: &ToggleVcsPalette,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.vcs_palette = !self.state.vcs_palette;
        if self.state.vcs_palette && self.state.prompt.is_some() {
            self.cancel_prompt(cx);
        }
        cx.notify();
    }

    /// 工具栏分支部件主按钮：开关「搜索分支和操作」弹层。
    /// 打开时清空搜索并关掉互斥的 VCS 快切 / 对话框。
    pub(crate) fn toggle_branch_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = !self.state.branch_popup;
        self.state.branch_popup = open;
        if open {
            self.state.vcs_palette = false;
            if self.state.prompt.is_some() {
                self.cancel_prompt(cx);
            }
            self.branch_popup_query
                .update(cx, |state, cx| state.set_value("", window, cx));
        }
        cx.notify();
    }

    /// 对「正在查看的文件」打开 blame：优先当前 blame 文件，其次当前 diff 文件。
    pub(crate) fn blame_current_file(&mut self, cx: &mut Context<Self>) {
        let path = if self.state.blame_path.is_empty() {
            self.state.diff_path.clone()
        } else {
            Some(self.state.blame_path.clone())
        };
        match path {
            Some(path) => self.open_blame(path, cx),
            None => {
                self.state.error = Some("No file selected to blame".to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn on_blame_current_file(
        &mut self,
        _: &BlameCurrentFile,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.blame_current_file(cx);
    }

    /// Ctrl+Alt+1..9：直接切换侧栏面板。1 工作区 2 详情 3 Diff 4 历史
    /// 5 分支对比 6 Shelve 7 Rebase 8 冲突 9 Blame（Blame 走「当前文件」回退）。
    /// 6 的 Shelve 已并入变更面板页签（对齐 IDEA），打开该页签。
    pub(crate) fn on_select_sidebar_panel(
        &mut self,
        action: &SelectSidebarPanel,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action.0 {
            9 => self.blame_current_file(cx),
            6 => self.open_shelves(cx),
            n @ 1..=8 => {
                self.state.sidebar = match n {
                    1 => SidebarMode::Workspace,
                    2 => SidebarMode::Detail,
                    3 => SidebarMode::Diff,
                    4 => SidebarMode::History,
                    5 => SidebarMode::Compare,
                    7 => SidebarMode::Rebase,
                    _ => SidebarMode::Conflicts,
                };
                cx.notify();
            }
            _ => {}
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

    /// Ctrl+K：聚焦提交信息输入框（IntelliJ 惯例，直接进入提交流）。
    pub(crate) fn on_focus_composer(
        &mut self,
        _: &FocusComposer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.message_input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        cx.notify();
    }

    /// 沿提交历史移动键盘选择（↑↓），并联动 Detail 视图。
    fn move_commit_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
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
