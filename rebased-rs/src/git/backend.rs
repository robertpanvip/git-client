use std::path::Path;

use super::conflict::{ConflictFile, HunkChoice};
use super::error::Result;
use super::ops::ResetMode;
use super::rebase::RebaseAction;
use super::repo::Repository;
use super::types::{
    BlameGroup, Branch, Change, Commit, RepoStatus, StashEntry, Tag,
};

/// Storage-agnostic facade over a git repository.
///
/// The UI layer only depends on this trait, so alternative backends
/// (gitoxide, libgit2) can be introduced later without touching UI code.
pub trait GitBackend: Send + Sync {
    fn root(&self) -> &Path;
    fn log(&self, limit: usize) -> Result<Vec<Commit>>;
    fn status(&self) -> Result<RepoStatus>;
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
    fn checkout(&self, target: &str) -> Result<()>;
    fn create_branch(&self, name: &str, start_point: Option<&str>) -> Result<()>;
    fn delete_branch(&self, name: &str, force: bool) -> Result<()>;
    fn stash_push(&self, message: Option<&str>, include_untracked: bool) -> Result<()>;
    fn stash_pop(&self) -> Result<()>;
    fn discard_changes(&self, path: &str) -> Result<()>;
    fn remove_untracked(&self, path: &str) -> Result<()>;
    fn cherry_pick(&self, commit: &str) -> Result<()>;
    fn revert(&self, commit: &str) -> Result<()>;
    fn reset_to(&self, target: &str, mode: ResetMode) -> Result<()>;
    fn tags(&self) -> Result<Vec<Tag>>;
    fn create_tag(&self, name: &str, commit: Option<&str>, message: Option<&str>) -> Result<()>;
    fn delete_tag(&self, name: &str) -> Result<()>;
    fn diff_unstaged(&self, path: Option<&str>) -> Result<String>;
    fn diff_staged(&self, path: Option<&str>) -> Result<String>;
    fn diff_head(&self, path: Option<&str>) -> Result<String>;
    fn show_diff(&self, commit: &str, path: Option<&str>) -> Result<String>;
    fn blame(&self, rev: &str, path: &str) -> Result<Vec<BlameGroup>>;
    fn rebase_todos(&self, base: &str) -> Result<Vec<RebaseAction>>;
    fn rebase_run(&self, base: &str, plan: &[RebaseAction]) -> Result<()>;
    fn rebase_abort(&self) -> Result<()>;
    fn rebase_continue(&self) -> Result<()>;
    fn is_rebase_in_progress(&self) -> bool;
    fn rebase_stopped_commit(&self) -> Option<String>;
    fn merge_branch(&self, branch: &str) -> Result<()>;
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
    fn checkout_side(&self, path: &str, ours: bool) -> Result<()>;
    fn stash_list(&self) -> Result<Vec<StashEntry>>;
    fn stash_apply_at(&self, index: usize) -> Result<()>;
    fn stash_drop_at(&self, index: usize) -> Result<()>;
    fn reword_commit(&self, commit: &str, message: &str) -> Result<()>;
}

impl GitBackend for Repository {
    fn root(&self) -> &Path {
        Repository::root(self)
    }

    fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        Repository::log(self, limit)
    }

    fn status(&self) -> Result<RepoStatus> {
        Repository::status(self)
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

    fn checkout(&self, target: &str) -> Result<()> {
        Repository::checkout(self, target)
    }

    fn create_branch(&self, name: &str, start_point: Option<&str>) -> Result<()> {
        Repository::create_branch(self, name, start_point)
    }

    fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        Repository::delete_branch(self, name, force)
    }

    fn stash_push(&self, message: Option<&str>, include_untracked: bool) -> Result<()> {
        Repository::stash_push(self, message, include_untracked)
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

    fn diff_unstaged(&self, path: Option<&str>) -> Result<String> {
        Repository::diff_unstaged(self, path)
    }

    fn diff_staged(&self, path: Option<&str>) -> Result<String> {
        Repository::diff_staged(self, path)
    }

    fn diff_head(&self, path: Option<&str>) -> Result<String> {
        Repository::diff_head(self, path)
    }

    fn show_diff(&self, commit: &str, path: Option<&str>) -> Result<String> {
        Repository::show_diff(self, commit, path)
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

    fn checkout_side(&self, path: &str, ours: bool) -> Result<()> {
        Repository::checkout_side(self, path, ours)
    }

    fn stash_list(&self) -> Result<Vec<StashEntry>> {
        Repository::stash_list(self)
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
}

/// The CLI-based implementation is currently the only backend.
pub fn open_backend(path: impl Into<std::path::PathBuf>) -> Result<std::sync::Arc<dyn GitBackend>> {
    Ok(std::sync::Arc::new(Repository::open(path)?))
}
