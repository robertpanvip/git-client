use std::path::{Path, PathBuf};

use super::branches;
use super::command::{CancelToken, GitCommand, ProgressHandle};
use super::error::{GitError, Result};
use super::ops;
use super::status::STATUS_ARGS;
use super::types::{
    Branch, Change, Commit, FileDiff, ReflogEntry, Remote, RepoStatus, StashEntry, Tag,
};
use super::{blame, conflict, diff, merge, rebase};
use conflict::{ConflictFile, HunkChoice};
use rebase::RebaseAction;

pub struct Repository {
    cmd: GitCommand,
}

impl Repository {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        match std::fs::metadata(&path) {
            Err(_) => {
                return Err(GitError::new(format!(
                    "{} does not exist or is not accessible",
                    path.display()
                )));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(GitError::new(format!(
                    "{} is not a directory",
                    path.display()
                )));
            }
            Ok(_) => {}
        }
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
        self.log_filtered(limit, None, None, None)
    }

    /// 结构化过滤器版 log：`from` 限定分支（None = `--all`），`author` 按作者子串过滤，
    /// `since` 为 git 日期表达式（None = 不限时间）。
    pub fn log_filtered(
        &self,
        limit: usize,
        from: Option<&str>,
        author: Option<&str>,
        since: Option<&str>,
    ) -> Result<Vec<Commit>> {
        let args = super::log::log_args(limit, from, author, since);
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

    /// 解析任意 hash / 分支 / 标签为完整提交 id（`rev-parse --verify <rev>^{commit}`）。
    pub fn rev_parse(&self, rev: &str) -> Result<String> {
        let output = self
            .cmd
            .execute(&["rev-parse", "--verify", &format!("{rev}^{{commit}}")])?;
        if !output.success {
            return Err(GitError::with_stderr(
                format!("Unknown revision {rev}"),
                output.stderr,
            ));
        }
        Ok(output.stdout.trim().to_string())
    }

    /// 分支对比：`mine` 独有的提交（ahead）与 `theirs` 独有的提交（behind）。
    pub fn compare_branches(
        &self,
        mine: &str,
        theirs: &str,
        limit: usize,
    ) -> Result<(Vec<Commit>, Vec<Commit>)> {
        let ahead = self.log_filtered(limit, Some(&format!("{theirs}..{mine}")), None, None)?;
        let behind = self.log_filtered(limit, Some(&format!("{mine}..{theirs}")), None, None)?;
        Ok((ahead, behind))
    }

    pub fn status(&self) -> Result<RepoStatus> {
        let output = self.cmd.execute(&STATUS_ARGS)?;
        if !output.success {
            return Err(GitError::with_stderr("git status failed", output.stderr));
        }
        Ok(super::status::parse_status(&output.stdout))
    }

    /// 轻量仓库指纹：HEAD hash + 工作区变更行数，用于低频自动刷新检测。
    pub fn repo_digest(&self) -> Result<String> {
        let head = self.rev_parse("HEAD").unwrap_or_default();
        let dirty = self
            .cmd
            .execute(&["status", "--porcelain"])
            .map(|o| {
                if o.success {
                    o.stdout.lines().count()
                } else {
                    0
                }
            })
            .unwrap_or(0);
        Ok(format!("{head}:{dirty}"))
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let args = branches::ref_args();
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.cmd.execute(&args)?;
        if !output.success {
            return Err(GitError::with_stderr(
                "git for-each-ref failed",
                output.stderr,
            ));
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
        let output =
            self.cmd
                .execute(&["branch", "--format=%(refname:short)", "--contains", commit])?;
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

    pub fn push_force(&self, branch: &str) -> Result<()> {
        ops::push_force(&self.cmd, "origin", branch)
    }

    pub fn push_tags(&self) -> Result<()> {
        ops::push_tags(&self.cmd, "origin")
    }

    pub fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        ops::rename_branch(&self.cmd, old, new)
    }

    pub fn undo_head_commit(&self) -> Result<()> {
        ops::undo_head_commit(&self.cmd)
    }

    pub fn drop_head_commit(&self) -> Result<()> {
        ops::drop_head_commit(&self.cmd)
    }

    pub fn pull(&self, branch: &str) -> Result<()> {
        ops::pull(&self.cmd, "origin", branch)
    }

    pub fn fetch(&self) -> Result<()> {
        ops::fetch(&self.cmd, None)
    }

    pub fn push_with_control(
        &self,
        branch: &str,
        set_upstream: bool,
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()> {
        ops::push_progress(&self.cmd, "origin", branch, set_upstream, progress, cancel)
    }

    pub fn pull_with_control(
        &self,
        branch: &str,
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()> {
        ops::pull_progress(&self.cmd, "origin", branch, progress, cancel)
    }

    pub fn fetch_with_control(&self, progress: ProgressHandle, cancel: CancelToken) -> Result<()> {
        ops::fetch_progress(&self.cmd, None, progress, cancel)
    }

    /// 把工作区 diff 中的单个 hunk 暂存到 index。
    pub fn apply_hunk_to_index(&self, file: &FileDiff, hunk_index: usize) -> Result<()> {
        let patch = diff::hunk_patch(file, hunk_index);
        if patch.is_empty() {
            return Err(GitError::new("hunk not found"));
        }
        ops::apply_patch_cached(&self.cmd, &patch, false)
    }

    /// 把已暂存 diff 中的单个 hunk 撤回工作区。
    pub fn revert_hunk_from_index(&self, file: &FileDiff, hunk_index: usize) -> Result<()> {
        let patch = diff::hunk_patch(file, hunk_index);
        if patch.is_empty() {
            return Err(GitError::new("hunk not found"));
        }
        ops::apply_patch_cached(&self.cmd, &patch, true)
    }

    /// 把工作区中的单个 hunk 还原成 index 版本（`git apply -R`，不动 index）。
    /// 这是 diff 面板「左栏内容同步到右栏」箭头的 git 语义。
    pub fn revert_hunk_in_worktree(&self, file: &FileDiff, hunk_index: usize) -> Result<()> {
        let patch = diff::hunk_patch(file, hunk_index);
        if patch.is_empty() {
            return Err(GitError::new("hunk not found"));
        }
        ops::apply_patch_worktree(&self.cmd, &patch, true)
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

    pub fn remotes(&self) -> Result<Vec<Remote>> {
        ops::remote_list(&self.cmd)
    }

    pub fn remote_add(&self, name: &str, url: &str) -> Result<()> {
        ops::remote_add(&self.cmd, name, url)
    }

    pub fn remote_remove(&self, name: &str) -> Result<()> {
        ops::remote_remove(&self.cmd, name)
    }

    pub fn remote_prune(&self, name: &str) -> Result<()> {
        ops::remote_prune(&self.cmd, name)
    }

    /// 设置分支上游（upstream 形如 `origin/main`）。
    pub fn set_upstream(&self, branch: &str, upstream: &str) -> Result<()> {
        ops::set_upstream(&self.cmd, branch, upstream)
    }

    pub fn unset_upstream(&self, branch: &str) -> Result<()> {
        ops::unset_upstream(&self.cmd, branch)
    }

    pub fn stash_push(
        &self,
        message: Option<&str>,
        keep_index: bool,
        include_untracked: bool,
    ) -> Result<()> {
        ops::stash_push(&self.cmd, message, keep_index, include_untracked)
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
            return Err(GitError::with_stderr(
                "git for-each-ref failed",
                output.stderr,
            ));
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

    pub fn push_tag(&self, tag: &str) -> Result<()> {
        ops::push_tag(&self.cmd, "origin", tag)
    }

    pub fn recreate_tag(&self, name: &str, commit: &str, message: &str) -> Result<()> {
        ops::recreate_tag(&self.cmd, name, commit, message)
    }

    pub fn diff_unstaged(&self, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        diff::diff_unstaged(&self.cmd, path, ignore_ws)
    }

    pub fn diff_staged(&self, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        diff::diff_staged(&self.cmd, path, ignore_ws)
    }

    pub fn diff_head(&self, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        diff::diff_head(&self.cmd, path, ignore_ws)
    }

    pub fn show_diff(&self, commit: &str, path: Option<&str>, ignore_ws: bool) -> Result<String> {
        diff::show_diff(&self.cmd, commit, path, ignore_ws)
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

    pub fn rebase_stopped_commit(&self) -> Option<String> {
        rebase::stopped_commit(&self.cmd)
    }

    pub fn merge_branch(&self, branch: &str) -> Result<()> {
        self.merge_branch_with(branch, merge::MergeMode::Default)
    }

    pub fn merge_branch_with(&self, branch: &str, mode: merge::MergeMode) -> Result<()> {
        self.merge_branch_with_message(branch, mode, None)
    }

    pub fn merge_branch_with_message(
        &self,
        branch: &str,
        mode: merge::MergeMode,
        message: Option<&str>,
    ) -> Result<()> {
        merge::merge_branch(&self.cmd, branch, mode, message)
    }

    pub fn merge_continue(&self) -> Result<()> {
        merge::continue_merge(&self.cmd)
    }

    pub fn merge_abort(&self) -> Result<()> {
        merge::abort(&self.cmd)
    }

    pub fn is_merge_in_progress(&self) -> bool {
        merge::in_progress(&self.cmd)
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

    pub fn worktree_file_content(&self, path: &str) -> Result<String> {
        let full = self.cmd.workdir().join(path);
        let content = std::fs::read_to_string(&full)
            .map_err(|e| GitError::with_stderr(format!("read {} failed", path), e.to_string()))?;
        Ok(content)
    }

    /// 工作区文件清单（tracked + untracked，遵循 .gitignore），相对仓库根路径。
    ///
    /// 返回顺序为 IDEA 项目树顺序：同级目录在前、文件在后，各自按名称升序。
    pub fn worktree_files(&self) -> Result<Vec<String>> {
        let stdout = self
            .cmd
            .run(&["ls-files", "-co", "--exclude-standard", "-z"])?;
        let mut files: Vec<String> = stdout
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
            .collect();
        files.sort_by(|a, b| tree_order(a, b));
        files.dedup();
        Ok(files)
    }

    /// 工作区版本的逐行 blame（未提交行标记为全 0 commit）。
    pub fn blame_worktree(&self, path: &str) -> Result<Vec<super::BlameGroup>> {
        let stdout = blame::blame_worktree(&self.cmd, path)?;
        Ok(blame::parse_blame(&stdout))
    }

    pub fn write_worktree_file(&self, path: &str, content: &str) -> Result<()> {
        let full = self.cmd.workdir().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GitError::with_stderr(format!("mkdir for {} failed", path), e.to_string())
            })?;
        }
        std::fs::write(&full, content)
            .map_err(|e| GitError::with_stderr(format!("write {} failed", path), e.to_string()))?;
        Ok(())
    }

    pub fn checkout_side(&self, path: &str, ours: bool) -> Result<()> {
        conflict::checkout_side(&self.cmd, path, ours)
    }

    pub fn stash_list(&self) -> Result<Vec<StashEntry>> {
        ops::stash_list(&self.cmd)
    }

    /// 轻量操作历史（HEAD reflog）。
    pub fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        ops::reflog(&self.cmd, limit)
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

    pub fn fixup_commit(&self, commit: &str) -> Result<()> {
        rebase::fixup(&self.cmd, commit)
    }

    pub fn squash_commit(&self, commit: &str, message: Option<&str>) -> Result<()> {
        rebase::squash(&self.cmd, commit, message)
    }

    pub fn drop_commit(&self, commit: &str) -> Result<()> {
        rebase::drop_commit(&self.cmd, commit)
    }

    pub fn uncommit_commit(&self, commit: &str) -> Result<()> {
        rebase::uncommit_commit(&self.cmd, commit)
    }

    pub fn log_follow(&self, limit: usize, path: &str) -> Result<Vec<Commit>> {
        let args = super::log::log_follow_args(limit, path);
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.cmd.execute(&args)?;
        if !output.success {
            return Err(GitError::with_stderr(
                "git log --follow failed",
                output.stderr,
            ));
        }
        Ok(super::log::parse_log(&output.stdout))
    }

    pub fn log_path(&self, limit: usize, path: &str) -> Result<Vec<Commit>> {
        let args = super::log::log_path_args(limit, path);
        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let output = self.cmd.execute(&args)?;
        if !output.success {
            return Err(GitError::with_stderr("git log -- path failed", output.stderr));
        }
        Ok(super::log::parse_log(&output.stdout))
    }

    pub fn head_message(&self) -> Result<String> {
        super::log::full_message(&self.cmd, "HEAD")
    }
}

/// IDEA 项目树排序：同级节点目录在前、文件在后，各自按名称升序。
///
/// 相比纯字典序，该顺序同时保证「父目录总在子节点之前」的摊平不变量
/// （`ui::file_tree::flatten_tree` 依赖），并让文件夹聚合展示在文件之前：
/// 如 `src.rs`（文件）排在 `src/`（目录）之后。
fn tree_order(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let sa: Vec<&str> = a.split('/').collect();
    let sb: Vec<&str> = b.split('/').collect();
    for i in 0..sa.len().min(sb.len()) {
        let ord = sa[i].cmp(sb[i]);
        if ord == Ordering::Equal {
            continue;
        }
        return match (i + 1 == sa.len(), i + 1 == sb.len()) {
            // a 是文件而 b 是目录：目录在前。
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => ord,
        };
    }
    // 公共前缀全部相同：更短的一方是对方的祖先目录，祖先在前。
    sa.len().cmp(&sb.len())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir()
                .join(format!("rebased-rs-repo-{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            git(&path, &["init", "-b", "main"]);
            git(&path, &["config", "user.name", "Test"]);
            git(&path, &["config", "user.email", "test@example.com"]);
            Self { path }
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn commit_file(dir: &Path, name: &str, message: &str) {
        std::fs::write(dir.join(name), message).unwrap();
        git(dir, &["add", name]);
        git(dir, &["commit", "-m", message]);
    }

    fn subjects(commits: &[Commit]) -> Vec<&str> {
        commits.iter().map(|c| c.subject.as_str()).collect()
    }

    #[test]
    fn compare_branches_reports_ahead_and_behind() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "base.txt", "base");
        git(&dir.path, &["checkout", "-b", "feature"]);
        commit_file(&dir.path, "feature.txt", "feature work");
        git(&dir.path, &["checkout", "main"]);
        commit_file(&dir.path, "main.txt", "main work");

        let repo = Repository::open(&dir.path).unwrap();
        let (ahead, behind) = repo.compare_branches("main", "feature", 50).unwrap();
        assert_eq!(subjects(&ahead), ["main work"]);
        assert_eq!(subjects(&behind), ["feature work"]);
    }

    #[test]
    fn log_filtered_since_filters_by_commit_time() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "base.txt", "base");

        let repo = Repository::open(&dir.path).unwrap();
        let all = repo.log_filtered(50, None, None, None).unwrap();
        assert_eq!(subjects(&all), ["base"]);
        // 未来日期：过滤掉全部提交。
        let none = repo
            .log_filtered(50, None, None, Some("2038-01-01"))
            .unwrap();
        assert!(none.is_empty());
        // 远古日期：全部保留。
        let ancient = repo
            .log_filtered(50, None, None, Some("1970-01-01"))
            .unwrap();
        assert_eq!(subjects(&ancient), ["base"]);
    }

    #[test]
    fn merge_branch_with_ff_only_fast_forwards_without_merge_commit() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "base.txt", "base");
        git(&dir.path, &["checkout", "-b", "feature"]);
        commit_file(&dir.path, "feature.txt", "feature work");

        let repo = Repository::open(&dir.path).unwrap();
        repo.merge_branch_with("feature", merge::MergeMode::FastForwardOnly)
            .unwrap();
        let log = repo.log(5).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].subject, "feature work");
        assert_eq!(log[0].parents.len(), 1);
    }

    #[test]
    fn merge_branch_with_no_ff_creates_merge_commit() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "base.txt", "base");
        git(&dir.path, &["checkout", "-b", "feature"]);
        commit_file(&dir.path, "feature.txt", "feature work");
        git(&dir.path, &["checkout", "main"]);
        commit_file(&dir.path, "main.txt", "main work");

        let repo = Repository::open(&dir.path).unwrap();
        repo.merge_branch_with("feature", merge::MergeMode::NoFastForward)
            .unwrap();
        let log = repo.log(10).unwrap();
        // no-ff 产生合并提交，且是最新的提交：两个 parent，两侧工作都在历史里。
        assert_eq!(log[0].parents.len(), 2);
        assert_eq!(log[0].subject, "Merge branch 'feature'");
        assert!(subjects(&log).contains(&"main work"));
        assert!(subjects(&log).contains(&"feature work"));
    }

    #[test]
    fn merge_branch_with_message_uses_custom_message() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "base.txt", "base");
        git(&dir.path, &["checkout", "-b", "feature"]);
        commit_file(&dir.path, "feature.txt", "feature work");
        git(&dir.path, &["checkout", "main"]);
        commit_file(&dir.path, "main.txt", "main work");

        let repo = Repository::open(&dir.path).unwrap();
        repo.merge_branch_with_message(
            "feature",
            merge::MergeMode::NoFastForward,
            Some("custom merge message"),
        )
        .unwrap();
        let log = repo.log(10).unwrap();
        assert_eq!(log[0].parents.len(), 2);
        assert_eq!(log[0].subject, "custom merge message");
    }

    #[test]
    fn worktree_file_content_and_write_roundtrip() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "a.txt", "original");

        let repo = Repository::open(&dir.path).unwrap();
        let content = repo.worktree_file_content("a.txt").unwrap();
        assert_eq!(content, "original");

        repo.write_worktree_file("a.txt", "edited content").unwrap();
        let edited = std::fs::read_to_string(dir.path.join("a.txt")).unwrap();
        assert_eq!(edited, "edited content");

        let err = repo.worktree_file_content("missing.txt").unwrap_err();
        assert!(err.to_string().contains("read missing.txt failed"));
    }

    #[test]
    fn worktree_files_lists_tracked_and_untracked_sorted() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "README.md", "readme");
        std::fs::create_dir_all(dir.path.join("src")).unwrap();
        commit_file(&dir.path, "src/main.rs", "main");
        std::fs::write(dir.path.join("notes.txt"), "note").unwrap();
        std::fs::write(dir.path.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir_all(dir.path.join("ignored")).unwrap();
        std::fs::write(dir.path.join("ignored/skip.txt"), "skip").unwrap();

        let repo = Repository::open(&dir.path).unwrap();
        let files = repo.worktree_files().unwrap();
        // IDEA 顺序：目录段 src 聚合在前，顶层文件按名称排序在后。
        assert_eq!(
            files,
            vec!["src/main.rs", ".gitignore", "README.md", "notes.txt"]
        );
    }

    #[test]
    fn tree_order_matches_idea_conventions() {
        use std::cmp::Ordering;

        // 同级目录在前、文件在后。
        assert_eq!(tree_order("src/a.rs", "README.md"), Ordering::Less);
        // 父目录先于子节点（flatten_tree 依赖的不变量）。
        assert_eq!(tree_order("src", "src/main.rs"), Ordering::Less);
        // 目录名与文件名交叉：src/ 排在 src.rs 前。
        assert_eq!(tree_order("src/main.rs", "src.rs"), Ordering::Less);
        // 同为文件按名称升序。
        assert_eq!(tree_order("a.txt", "b.txt"), Ordering::Less);
        // 同目录下文件让位于子目录。
        assert_eq!(tree_order("src/aaa.txt", "src/bbb/c.txt"), Ordering::Greater);
        // 公共前缀相同、目录在文件前。
        assert_eq!(tree_order("src/ui/mod.rs", "src/ui.txt"), Ordering::Less);
    }

    #[test]
    fn blame_worktree_marks_uncommitted_lines() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "a.txt", "one\ntwo\n");
        std::fs::write(dir.path.join("a.txt"), "one\nchanged\nthree\n").unwrap();

        let repo = Repository::open(&dir.path).unwrap();
        let groups = repo.blame_worktree("a.txt").unwrap();
        // 行号对应工作区内容 1..=3（而非 HEAD 版本），未提交行由 git 标记为全 0 commit。
        let mut numbers: Vec<u32> = groups
            .iter()
            .flat_map(|group| group.lines.iter().map(|line| line.number))
            .collect();
        numbers.sort();
        assert_eq!(numbers, vec![1, 2, 3]);
        assert!(
            groups
                .iter()
                .any(|group| group.commit_id.chars().all(|c| c == '0')),
            "工作区中被修改的行应标记为未提交: {groups:?}"
        );
    }

    #[test]
    fn repo_digest_reflects_head_and_worktree() {
        let dir = TempRepo::new();
        commit_file(&dir.path, "base.txt", "base");
        let repo = Repository::open(&dir.path).unwrap();
        let clean = repo.repo_digest().unwrap();
        assert!(clean.ends_with(":0"));
        std::fs::write(dir.path.join("base.txt"), "dirty").unwrap();
        let dirty = repo.repo_digest().unwrap();
        assert_ne!(clean, dirty);
        assert!(dirty.ends_with(":1"));
    }
}
