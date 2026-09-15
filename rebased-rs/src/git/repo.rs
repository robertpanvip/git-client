use std::path::{Path, PathBuf};

use super::branches;
use super::command::GitCommand;
use super::error::{GitError, Result};
use super::ops;
use super::status::STATUS_ARGS;
use super::types::{Branch, Change, Commit, RepoStatus, StashEntry, Tag};
use super::{blame, conflict, diff, rebase};
use conflict::{ConflictFile, HunkChoice};
use rebase::RebaseAction;

pub struct Repository {
    cmd: GitCommand,
}

impl Repository {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let cmd = GitCommand::new(&path);
        let output = cmd.execute(&["rev-parse", "--is-inside-work-tree"])?;
        if !output.success || output.stdout.trim() != "true" {
            return Err(GitError::with_stderr(
                format!("{} is not a git repository", path.display()),
                output.stderr,
            ));
        }
        Ok(Self { cmd })
    }

    pub fn root(&self) -> &Path {
        self.cmd.workdir()
    }

    pub fn log(&self, limit: usize) -> Result<Vec<Commit>> {
        let args = super::log::log_args(limit, None);
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.cmd.execute(&args)?;
        if !output.success {
            if output.stderr.contains("does not have any commits yet")
                || output.stderr.contains("bad revision 'HEAD'")
            {
                return Ok(Vec::new());
            }
            return Err(GitError::with_stderr("git log failed", output.stderr));
        }
        Ok(super::log::parse_log(&output.stdout))
    }

    pub fn status(&self) -> Result<RepoStatus> {
        let output = self.cmd.execute(&STATUS_ARGS)?;
        if !output.success {
            return Err(GitError::with_stderr("git status failed", output.stderr));
        }
        Ok(super::status::parse_status(&output.stdout))
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let args = branches::ref_args();
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.cmd.execute(&args)?;
        if !output.success {
            return Err(GitError::with_stderr("git for-each-ref failed", output.stderr));
        }
        Ok(branches::parse_refs(&output.stdout))
    }

    pub fn current_branch_name(&self) -> Result<String> {
        let output = self.cmd.execute(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if output.success {
            Ok(output.stdout.trim().to_string())
        } else {
            Ok(String::new())
        }
    }

    pub fn branches_containing(&self, commit: &str) -> Result<Vec<String>> {
        let output = self.cmd.execute(&["branch", "--format=%(refname:short)", "--contains", commit])?;
        if !output.success {
            return Err(GitError::with_stderr(
                "git branch --contains failed",
                output.stderr,
            ));
        }
        Ok(output
            .stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    pub fn add(&self, paths: &[&str]) -> Result<()> {
        ops::add(&self.cmd, paths)
    }

    pub fn add_all(&self) -> Result<()> {
        ops::add_all(&self.cmd)
    }

    pub fn show_files(&self, commit: &str) -> Result<Vec<Change>> {
        ops::show_files(&self.cmd, commit)
    }

    pub fn reset(&self, paths: &[&str]) -> Result<()> {
        ops::reset(&self.cmd, paths)
    }

    pub fn commit(&self, message: &str, amend: bool) -> Result<()> {
        ops::commit(&self.cmd, message, amend)
    }

    pub fn commit_paths(&self, message: &str, paths: &[&str], amend: bool) -> Result<()> {
        ops::commit_paths(&self.cmd, message, paths, amend)
    }

    pub fn push(&self, branch: &str, set_upstream: bool) -> Result<()> {
        ops::push(&self.cmd, "origin", branch, set_upstream)
    }

    pub fn pull(&self, branch: &str) -> Result<()> {
        ops::pull(&self.cmd, "origin", branch)
    }

    pub fn fetch(&self) -> Result<()> {
        ops::fetch(&self.cmd, None)
    }

    pub fn checkout(&self, target: &str) -> Result<()> {
        ops::checkout(&self.cmd, target)
    }

    pub fn create_branch(&self, name: &str, start_point: Option<&str>) -> Result<()> {
        ops::create_branch(&self.cmd, name, start_point)
    }

    pub fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        ops::delete_branch(&self.cmd, name, force)
    }

    pub fn stash_push(&self, message: Option<&str>, include_untracked: bool) -> Result<()> {
        ops::stash_push(&self.cmd, message, include_untracked)
    }

    pub fn stash_pop(&self) -> Result<()> {
        ops::stash_pop(&self.cmd)
    }

    pub fn discard_changes(&self, path: &str) -> Result<()> {
        ops::discard_changes(&self.cmd, path)
    }

    pub fn remove_untracked(&self, path: &str) -> Result<()> {
        ops::remove_untracked(&self.cmd, path)
    }

    pub fn cherry_pick(&self, commit: &str) -> Result<()> {
        ops::cherry_pick(&self.cmd, commit)
    }

    pub fn revert(&self, commit: &str) -> Result<()> {
        ops::revert(&self.cmd, commit)
    }

    pub fn reset_to(&self, target: &str, mode: ops::ResetMode) -> Result<()> {
        ops::reset_to(&self.cmd, target, mode)
    }

    pub fn tags(&self) -> Result<Vec<Tag>> {
        let args = [
            "for-each-ref",
            "refs/tags",
            "--format=%(refname:short)\t%(*objectname)\t%(objectname)",
        ];
        let output = self.cmd.execute(&args)?;
        if !output.success {
            return Err(GitError::with_stderr("git for-each-ref failed", output.stderr));
        }
        Ok(ops::parse_tags(&output.stdout))
    }

    pub fn create_tag(
        &self,
        name: &str,
        commit: Option<&str>,
        message: Option<&str>,
    ) -> Result<()> {
        ops::create_tag(&self.cmd, name, commit, message)
    }

    pub fn delete_tag(&self, name: &str) -> Result<()> {
        ops::delete_tag(&self.cmd, name)
    }

    pub fn diff_unstaged(&self, path: Option<&str>) -> Result<String> {
        diff::diff_unstaged(&self.cmd, path)
    }

    pub fn diff_staged(&self, path: Option<&str>) -> Result<String> {
        diff::diff_staged(&self.cmd, path)
    }

    pub fn diff_head(&self, path: Option<&str>) -> Result<String> {
        diff::diff_head(&self.cmd, path)
    }

    pub fn show_diff(&self, commit: &str, path: Option<&str>) -> Result<String> {
        diff::show_diff(&self.cmd, commit, path)
    }

    pub fn blame(&self, rev: &str, path: &str) -> Result<Vec<super::BlameGroup>> {
        let stdout = blame::blame_file(&self.cmd, rev, path)?;
        Ok(blame::parse_blame(&stdout))
    }

    pub fn rebase_todos(&self, base: &str) -> Result<Vec<RebaseAction>> {
        rebase::todos(&self.cmd, base)
    }

    pub fn rebase_run(&self, base: &str, plan: &[RebaseAction]) -> Result<()> {
        rebase::run(&self.cmd, base, plan)
    }

    pub fn rebase_abort(&self) -> Result<()> {
        rebase::abort(&self.cmd)
    }

    pub fn rebase_continue(&self) -> Result<()> {
        rebase::continue_rebase(&self.cmd)
    }

    pub fn is_rebase_in_progress(&self) -> bool {
        rebase::in_progress(&self.cmd)
    }

    pub fn conflicted_files(&self) -> Result<Vec<ConflictFile>> {
        conflict::conflicted_files(&self.cmd)
    }

    pub fn conflict_file_content(&self, path: &str) -> Result<String> {
        let full = self.cmd.workdir().join(path);
        let content = std::fs::read_to_string(&full)
            .map_err(|e| GitError::with_stderr(format!("read {} failed", path), e.to_string()))?;
        Ok(content)
    }

    pub fn resolve_conflict_markers(
        &self,
        path: &str,
        content: &str,
        choices: &[HunkChoice],
    ) -> Result<()> {
        let resolved = conflict::resolve_markers(content, choices).ok_or_else(|| {
            GitError::with_stderr("resolve failed", "choice count does not match hunks")
        })?;
        let full = self.cmd.workdir().join(path);
        std::fs::write(&full, resolved)
            .map_err(|e| GitError::with_stderr(format!("write {} failed", path), e.to_string()))?;
        conflict::stage_file(&self.cmd, path)
    }

    pub fn stage_file(&self, path: &str) -> Result<()> {
        conflict::stage_file(&self.cmd, path)
    }

    pub fn checkout_side(&self, path: &str, ours: bool) -> Result<()> {
        conflict::checkout_side(&self.cmd, path, ours)
    }

    pub fn stash_list(&self) -> Result<Vec<StashEntry>> {
        ops::stash_list(&self.cmd)
    }

    pub fn stash_apply_at(&self, index: usize) -> Result<()> {
        ops::stash_apply_at(&self.cmd, index)
    }

    pub fn stash_drop_at(&self, index: usize) -> Result<()> {
        ops::stash_drop_at(&self.cmd, index)
    }

    pub fn reword_commit(&self, commit: &str, message: &str) -> Result<()> {
        rebase::reword(&self.cmd, commit, message)
    }
}
