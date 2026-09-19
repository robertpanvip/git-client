use std::path::Path;

use super::command::{CancelToken, ProgressHandle};
use super::conflict::{ConflictFile, HunkChoice};
use super::error::Result;
use super::merge::MergeMode;
use super::ops::ResetMode;
use super::rebase::RebaseAction;
use super::repo::Repository;
use super::types::{
    BlameGroup, Branch, Change, Commit, FileDiff, ReflogEntry, Remote, RepoStatus, StashEntry, Tag,
};

/// Storage-agnostic facade over a git repository.
///
/// The UI layer only depends on this trait, so alternative backends
/// (gitoxide, libgit2) can be introduced later without touching UI code.
pub trait GitBackend: Send + Sync {
    fn root(&self) -> &Path;
    fn log(&self, limit: usize) -> Result<Vec<Commit>>;
    /// 结构化过滤器版 log：`from` 限定分支（None = `--all`），`author` 按作者子串过滤，
    /// `since` 为 git 日期表达式（None = 不限时间）。
    fn log_filtered(
        &self,
        limit: usize,
        from: Option<&str>,
        author: Option<&str>,
        since: Option<&str>,
    ) -> Result<Vec<Commit>>;
    /// 解析任意 hash / 分支 / 标签为完整提交 id，用于 Go to 功能。
    fn rev_parse(&self, rev: &str) -> Result<String>;

    fn compare_branches(
        &self,
        mine: &str,
        theirs: &str,
        limit: usize,
    ) -> Result<(Vec<Commit>, Vec<Commit>)>;

    fn status(&self) -> Result<RepoStatus>;

    /// 轻量仓库指纹（HEAD + 工作区状态行数），用于自动刷新检测。
    fn repo_digest(&self) -> Result<String>;

    fn branches(&self) -> Result<Vec<Branch>>;
    fn current_branch_name(&self) -> Result<String>;
    fn branches_containing(&self, commit: &str) -> Result<Vec<String>>;
    fn add(&self, paths: &[&str]) -> Result<()>;
    fn add_all(&self) -> Result<()>;
    fn show_files(&self, commit: &str) -> Result<Vec<Change>>;
    fn reset(&self, paths: &[&str]) -> Result<()>;
    fn commit(&self, message: &str, amend: bool) -> Result<()>;
    fn commit_paths(&self, message: &str, paths: &[&str], amend: bool) -> Result<()>;
    fn push(&self, branch: &str, set_upstream: bool) -> Result<()>;
    fn push_force(&self, branch: &str) -> Result<()>;
    fn push_tags(&self) -> Result<()>;
    fn rename_branch(&self, old: &str, new: &str) -> Result<()>;
    fn undo_head_commit(&self) -> Result<()>;
    fn drop_head_commit(&self) -> Result<()>;
    fn pull(&self, branch: &str) -> Result<()>;
    fn fetch(&self) -> Result<()>;
    /// 带进度与取消的网络操作：`progress` 实时保存最新一行进度文本，
    /// `cancel` 置位后子进程被终止并返回取消错误。
    fn push_with_control(
        &self,
        branch: &str,
        set_upstream: bool,
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()>;
    fn pull_with_control(
        &self,
        branch: &str,
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()>;
    fn fetch_with_control(&self, progress: ProgressHandle, cancel: CancelToken) -> Result<()>;
    /// 把工作区 diff 中的单个 hunk 暂存到 index（`git apply --cached`）。
    fn apply_hunk_to_index(&self, file: &FileDiff, hunk_index: usize) -> Result<()>;
    /// 把已暂存 diff 中的单个 hunk 撤回工作区（`git apply --cached -R`）。
    fn revert_hunk_from_index(&self, file: &FileDiff, hunk_index: usize) -> Result<()>;
    /// 把工作区中的单个 hunk 还原成 index 版本（`git apply -R`，不动 index）。
    fn revert_hunk_in_worktree(&self, file: &FileDiff, hunk_index: usize) -> Result<()>;
    fn checkout(&self, target: &str) -> Result<()>;
    fn create_branch(&self, name: &str, start_point: Option<&str>) -> Result<()>;
    fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
    /// 列出远程仓库（name + fetch url）。
    fn remotes(&self) -> Result<Vec<Remote>>;
    fn remote_add(&self, name: &str, url: &str) -> Result<()>;
    fn remote_remove(&self, name: &str) -> Result<()>;
    /// 清理远程已删除分支的本地引用。
    fn remote_prune(&self, name: &str) -> Result<()>;
    /// 设置分支上游（upstream 形如 `origin/main`）。
    fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()>;
    fn unset_upstream(&self, branch: &str) -> Result<()>;
    fn stash_push(
        &self,
        message: Option<&str>,
        keep_index: bool,
        include_untracked: bool,
    ) -> Result<()>;
    fn stash_pop(&self) -> Result<()>;
    fn discard_changes(&self, path: &str) -> Result<()>;
    fn remove_untracked(&self, path: &str) -> Result<()>;
    fn cherry_pick(&self, commit: &str) -> Result<()>;
    fn revert(&self, commit: &str) -> Result<()>;
    fn reset_to(&self, target: &str, mode: ResetMode) -> Result<()>;
    fn tags(&self) -> Result<Vec<Tag>>;
    fn create_tag(&self, name: &str, commit: Option<&str>, message: Option<&str>) -> Result<()>;
    fn delete_tag(&self, name: &str) -> Result<()>;
    fn push_tag(&self, tag: &str) -> Result<()>;
    fn recreate_tag(&self, name: &str, commit: &str, message: &str) -> Result<()>;
    fn diff_unstaged(&self, path: Option<&str>, ignore_ws: bool) -> Result<String>;
    fn diff_staged(&self, path: Option<&str>, ignore_ws: bool) -> Result<String>;
    fn diff_head(&self, path: Option<&str>, ignore_ws: bool) -> Result<String>;
    fn show_diff(&self, commit: &str, path: Option<&str>, ignore_ws: bool) -> Result<String>;
    fn blame(&self, rev: &str, path: &str) -> Result<Vec<BlameGroup>>;
    fn rebase_todos(&self, base: &str) -> Result<Vec<RebaseAction>>;
    fn rebase_run(&self, base: &str, plan: &[RebaseAction]) -> Result<()>;
    fn rebase_abort(&self) -> Result<()>;
    fn rebase_continue(&self) -> Result<()>;
    fn is_rebase_in_progress(&self) -> bool;
    fn rebase_stopped_commit(&self) -> Option<String>;
    fn merge_branch(&self, branch: &str) -> Result<()>;
    fn merge_branch_with(&self, branch: &str, mode: MergeMode) -> Result<()>;
    fn merge_branch_with_message(
        &self,
        branch: &str,
        mode: MergeMode,
        message: Option<&str>,
    ) -> Result<()>;
    fn merge_continue(&self) -> Result<()>;
    fn merge_abort(&self) -> Result<()>;
    fn is_merge_in_progress(&self) -> bool;
    fn conflicted_files(&self) -> Result<Vec<ConflictFile>>;
    fn conflict_file_content(&self, path: &str) -> Result<String>;
    fn resolve_conflict_markers(
        &self,
        path: &str,
        content: &str,
        choices: &[HunkChoice],
    ) -> Result<()>;
    fn stage_file(&self, path: &str) -> Result<()>;
    fn worktree_file_content(&self, path: &str) -> Result<String>;
    /// 工作区文件清单（tracked + untracked，遵循 .gitignore），相对仓库根路径。
    fn worktree_files(&self) -> Result<Vec<String>>;
    /// 工作区版本的逐行 blame（未提交行标记为全 0 commit）。
    fn blame_worktree(&self, path: &str) -> Result<Vec<BlameGroup>>;
    fn write_worktree_file(&self, path: &str, content: &str) -> Result<()>;
    fn checkout_side(&self, path: &str, ours: bool) -> Result<()>;
    fn stash_list(&self) -> Result<Vec<StashEntry>>;
    /// 轻量操作历史（HEAD reflog）。
    fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>>;
    fn stash_apply_at(&self, index: usize) -> Result<()>;
    fn stash_drop_at(&self, index: usize) -> Result<()>;
    fn reword_commit(&self, commit: &str, message: &str) -> Result<()>;
    /// 把 `commit` fixup 进其父提交（丢弃该提交原信息）。
    fn fixup_commit(&self, commit: &str) -> Result<()>;
    /// 把 `commit` squash 进其父提交，可选覆盖合并后的提交信息。
    fn squash_commit(&self, commit: &str, message: Option<&str>) -> Result<()>;
    /// 从历史中删除 `commit`（丢弃其更改）。
    fn drop_commit(&self, commit: &str) -> Result<()>;
    /// 删除 `commit` 但保留其差异为暂存的工作区更改（非 HEAD Uncommit）。
    fn uncommit_commit(&self, commit: &str) -> Result<()>;
    /// History of commits touching `path`（follow renames）。
    fn log_follow(&self, limit: usize, path: &str) -> Result<Vec<Commit>>;
    /// History of commits touching `path`（目录/路径过滤，不做 rename 跟随）。
    fn log_path(&self, limit: usize, path: &str) -> Result<Vec<Commit>>;
    /// Full message（%B）of HEAD，用于 Amend 预填原提交消息。
    fn head_message(&self) -> Result<String>;
}

impl GitBackend for Repository {
    fn root(&self) -> &Path {
        Repository::root(self)
    }

    fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        Repository::log(self, limit)
    }

    fn log_filtered(
        &self,
        limit: usize,
        from: Option<&str>,
        author: Option<&str>,
        since: Option<&str>,
    ) -> Result<Vec<Commit>> {
        Repository::log_filtered(self, limit, from, author, since)
    }

    fn rev_parse(&self, rev: &str) -> Result<String> {
        Repository::rev_parse(self, rev)
    }

    fn compare_branches(
        &self,
        mine: &str,
        theirs: &str,
        limit: usize,
    ) -> Result<(Vec<Commit>, Vec<Commit>)> {
        Repository::compare_branches(self, mine, theirs, limit)
    }

    fn status(&self) -> Result<RepoStatus> {
        Repository::status(self)
    }

    fn repo_digest(&self) -> Result<String> {
        Repository::repo_digest(self)
    }

    fn branches(&self) -> Result<Vec<Branch>> {
        Repository::branches(self)
    }

    fn current_branch_name(&self) -> Result<String> {
        Repository::current_branch_name(self)
    }

    fn branches_containing(&self, commit: &str) -> Result<Vec<String>> {
        Repository::branches_containing(self, commit)
    }

    fn add(&self, paths: &[&str]) -> Result<()> {
        Repository::add(self, paths)
    }

    fn add_all(&self) -> Result<()> {
        Repository::add_all(self)
    }

    fn show_files(&self, commit: &str) -> Result<Vec<Change>> {
        Repository::show_files(self, commit)
    }

    fn reset(&self, paths: &[&str]) -> Result<()> {
        Repository::reset(self, paths)
    }

    fn commit(&self, message: &str, amend: bool) -> Result<()> {
        Repository::commit(self, message, amend)
    }

    fn commit_paths(&self, message: &str, paths: &[&str], amend: bool) -> Result<()> {
        Repository::commit_paths(self, message, paths, amend)
    }

    fn push(&self, branch: &str, set_upstream: bool) -> Result<()> {
        Repository::push(self, branch, set_upstream)
    }

    fn push_force(&self, branch: &str) -> Result<()> {
        Repository::push_force(self, branch)
    }

    fn push_tags(&self) -> Result<()> {
        Repository::push_tags(self)
    }

    fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        Repository::rename_branch(self, old, new)
    }

    fn undo_head_commit(&self) -> Result<()> {
        Repository::undo_head_commit(self)
    }

    fn drop_head_commit(&self) -> Result<()> {
        Repository::drop_head_commit(self)
    }

    fn pull(&self, branch: &str) -> Result<()> {
        Repository::pull(self, branch)
    }

    fn fetch(&self) -> Result<()> {
        Repository::fetch(self)
    }

    fn push_with_control(
        &self,
        branch: &str,
        set_upstream: bool,
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()> {
        Repository::push_with_control(self, branch, set_upstream, progress, cancel)
    }

    fn pull_with_control(
        &self,
        branch: &str,
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()> {
        Repository::pull_with_control(self, branch, progress, cancel)
    }

    fn fetch_with_control(&self, progress: ProgressHandle, cancel: CancelToken) -> Result<()> {
        Repository::fetch_with_control(self, progress, cancel)
    }

    fn apply_hunk_to_index(&self, file: &FileDiff, hunk_index: usize) -> Result<()> {
        Repository::apply_hunk_to_index(self, file, hunk_index)
    }

    fn revert_hunk_from_index(&self, file: &FileDiff, hunk_index: usize) -> Result<()> {
        Repository::revert_hunk_from_index(self, file, hunk_index)
    }

    fn revert_hunk_in_worktree(&self, file: &FileDiff, hunk_index: usize) -> Result<()> {
        Repository::revert_hunk_in_worktree(self, file, hunk_index)
    }

    fn checkout(&self, target: &str) -> Result<()> {
        Repository::checkout(self, target)
    }

    fn create_branch(&self, name: &str, start_point: Option<&str>) -> Result<()> {
        Repository::create_branch(self, name, start_point)
    }

    fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        Repository::delete_branch(self, name, force)
    }

    fn remotes(&self) -> Result<Vec<Remote>> {
        Repository::remotes(self)
    }

    fn remote_add(&self, name: &str, url: &str) -> Result<()> {
        Repository::remote_add(self, name, url)
    }

    fn remote_remove(&self, name: &str) -> Result<()> {
        Repository::remote_remove(self, name)
    }

    fn remote_prune(&self, name: &str) -> Result<()> {
        Repository::remote_prune(self, name)
    }

    fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()> {
        Repository::set_upstream(self, branch, upstream)
    }

    fn unset_upstream(&self, branch: &str) -> Result<()> {
        Repository::unset_upstream(self, branch)
    }

    fn stash_push(
        &self,
        message: Option<&str>,
        keep_index: bool,
        include_untracked: bool,
    ) -> Result<()> {
        Repository::stash_push(self, message, keep_index, include_untracked)
    }

    fn stash_pop(&self) -> Result<()> {
        Repository::stash_pop(self)
    }

    fn discard_changes(&self, path: &str) -> Result<()> {
        Repository::discard_changes(self, path)
    }

    fn remove_untracked(&self, path: &str) -> Result<()> {
        Repository::remove_untracked(self, path)
    }

    fn cherry_pick(&self, commit: &str) -> Result<()> {
        Repository::cherry_pick(self, commit)
    }

    fn revert(&self, commit: &str) -> Result<()> {
        Repository::revert(self, commit)
    }

    fn reset_to(&self, target: &str, mode: ResetMode) -> Result<()> {
        Repository::reset_to(self, target, mode)
    }

    fn tags(&self) -> Result<Vec<Tag>> {
        Repository::tags(self)
    }

    fn create_tag(&self, name: &str, commit: Option<&str>, message: Option<&str>) -> Result<()> {
        Repository::create_tag(self, name, commit, message)
    }

    fn delete_tag(&self, name: &str) -> Result<()> {
        Repository::delete_tag(self, name)
    }

    fn push_tag(&self, tag: &str) -> Result<()> {
        Repository::push_tag(self, tag)
    }

    fn recreate_tag(&self, name: &str, commit: &str, message: &str) -> Result<()> {
        Repository::recreate_tag(self, name, commit, message)
    }

    fn diff_unstaged(&self, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        Repository::diff_unstaged(self, path, ignore_ws)
    }

    fn diff_staged(&self, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        Repository::diff_staged(self, path, ignore_ws)
    }

    fn diff_head(&self, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        Repository::diff_head(self, path, ignore_ws)
    }

    fn show_diff(&self, commit: &str, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        Repository::show_diff(self, commit, path, ignore_ws)
    }

    fn blame(&self, rev: &str, path: &str) -> Result<Vec<BlameGroup>> {
        Repository::blame(self, rev, path)
    }

    fn rebase_todos(&self, base: &str) -> Result<Vec<RebaseAction>> {
        Repository::rebase_todos(self, base)
    }

    fn rebase_run(&self, base: &str, plan: &[RebaseAction]) -> Result<()> {
        Repository::rebase_run(self, base, plan)
    }

    fn rebase_abort(&self) -> Result<()> {
        Repository::rebase_abort(self)
    }

    fn rebase_continue(&self) -> Result<()> {
        Repository::rebase_continue(self)
    }

    fn is_rebase_in_progress(&self) -> bool {
        Repository::is_rebase_in_progress(self)
    }

    fn rebase_stopped_commit(&self) -> Option<String> {
        Repository::rebase_stopped_commit(self)
    }

    fn merge_branch(&self, branch: &str) -> Result<()> {
        Repository::merge_branch(self, branch)
    }

    fn merge_branch_with(&self, branch: &str, mode: MergeMode) -> Result<()> {
        Repository::merge_branch_with(self, branch, mode)
    }

    fn merge_branch_with_message(
        &self,
        branch: &str,
        mode: MergeMode,
        message: Option<&str>,
    ) -> Result<()> {
        Repository::merge_branch_with_message(self, branch, mode, message)
    }

    fn merge_continue(&self) -> Result<()> {
        Repository::merge_continue(self)
    }

    fn merge_abort(&self) -> Result<()> {
        Repository::merge_abort(self)
    }

    fn is_merge_in_progress(&self) -> bool {
        Repository::is_merge_in_progress(self)
    }

    fn conflicted_files(&self) -> Result<Vec<ConflictFile>> {
        Repository::conflicted_files(self)
    }

    fn conflict_file_content(&self, path: &str) -> Result<String> {
        Repository::conflict_file_content(self, path)
    }

    fn resolve_conflict_markers(
        &self,
        path: &str,
        content: &str,
        choices: &[HunkChoice],
    ) -> Result<()> {
        Repository::resolve_conflict_markers(self, path, content, choices)
    }

    fn stage_file(&self, path: &str) -> Result<()> {
        Repository::stage_file(self, path)
    }

    fn worktree_file_content(&self, path: &str) -> Result<String> {
        Repository::worktree_file_content(self, path)
    }

    fn worktree_files(&self) -> Result<Vec<String>> {
        Repository::worktree_files(self)
    }

    fn blame_worktree(&self, path: &str) -> Result<Vec<BlameGroup>> {
        Repository::blame_worktree(self, path)
    }

    fn write_worktree_file(&self, path: &str, content: &str) -> Result<()> {
        Repository::write_worktree_file(self, path, content)
    }

    fn checkout_side(&self, path: &str, ours: bool) -> Result<()> {
        Repository::checkout_side(self, path, ours)
    }

    fn stash_list(&self) -> Result<Vec<StashEntry>> {
        Repository::stash_list(self)
    }

    fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        Repository::reflog(self, limit)
    }

    fn stash_apply_at(&self, index: usize) -> Result<()> {
        Repository::stash_apply_at(self, index)
    }

    fn stash_drop_at(&self, index: usize) -> Result<()> {
        Repository::stash_drop_at(self, index)
    }

    fn reword_commit(&self, commit: &str, message: &str) -> Result<()> {
        Repository::reword_commit(self, commit, message)
    }

    fn fixup_commit(&self, commit: &str) -> Result<()> {
        Repository::fixup_commit(self, commit)
    }

    fn squash_commit(&self, commit: &str, message: Option<&str>) -> Result<()> {
        Repository::squash_commit(self, commit, message)
    }

    fn drop_commit(&self, commit: &str) -> Result<()> {
        Repository::drop_commit(self, commit)
    }

    fn uncommit_commit(&self, commit: &str) -> Result<()> {
        Repository::uncommit_commit(self, commit)
    }

    fn log_follow(&self, limit: usize, path: &str) -> Result<Vec<Commit>> {
        Repository::log_follow(self, limit, path)
    }

    fn log_path(&self, limit: usize, path: &str) -> Result<Vec<Commit>> {
        Repository::log_path(self, limit, path)
    }

    fn head_message(&self) -> Result<String> {
        Repository::head_message(self)
    }
}

/// The CLI-based implementation is currently the only backend.
pub fn open_backend(path: impl Into<std::path::PathBuf>) -> Result<std::sync::Arc<dyn GitBackend>> {
    Ok(std::sync::Arc::new(Repository::open(path)?))
}
