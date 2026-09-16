use super::command::{CancelToken, GitCommand, ProgressHandle};
use super::error::{GitError, Result};
use super::types::{Change, ChangeStatus, ReflogEntry, Remote, StashEntry, Tag};

pub fn add_all(cmd: &GitCommand) -> Result<()> {
    cmd.run_ok(&["add", "-A"])
}

pub fn add(cmd: &GitCommand, paths: &[&str]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add", "--"];
    args.extend_from_slice(paths);
    cmd.run_ok(&args)
}

pub fn reset(cmd: &GitCommand, paths: &[&str]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["reset", "HEAD", "--"];
    args.extend_from_slice(paths);
    cmd.run_ok(&args)
}

pub fn commit(cmd: &GitCommand, message: &str, amend: bool) -> Result<()> {
    let mut args = vec!["commit", "-m", message];
    if amend {
        args.push("--amend");
    }
    cmd.run_ok(&args)
}

pub fn commit_paths(cmd: &GitCommand, message: &str, paths: &[&str], amend: bool) -> Result<()> {
    add(cmd, paths)?;
    if paths.is_empty() && !amend {
        return Ok(());
    }
    let mut args: Vec<&str> = if amend {
        vec!["commit", "--amend", "-m", message, "--"]
    } else {
        vec!["commit", "-m", message, "--"]
    };
    args.extend_from_slice(paths);
    cmd.run_ok(&args)
}

pub fn push(cmd: &GitCommand, remote: &str, branch: &str, set_upstream: bool) -> Result<()> {
    let mut args = vec!["push"];
    if set_upstream {
        args.push("--set-upstream");
    }
    args.push(remote);
    args.push(branch);
    cmd.run_ok(&args)
}

pub fn push_force(cmd: &GitCommand, remote: &str, branch: &str) -> Result<()> {
    cmd.run_ok(&["push", "--force-with-lease", remote, branch])
}

pub fn push_tags(cmd: &GitCommand, remote: &str) -> Result<()> {
    cmd.run_ok(&["push", "--tags", remote])
}

pub fn rename_branch(cmd: &GitCommand, old: &str, new: &str) -> Result<()> {
    cmd.run_ok(&["branch", "-m", old, new])
}

pub fn undo_head_commit(cmd: &GitCommand) -> Result<()> {
    let has_parent = cmd
        .execute(&["rev-parse", "--verify", "--quiet", "HEAD^"])?
        .success;
    if has_parent {
        cmd.run_ok(&["reset", "--soft", "HEAD^"])
    } else {
        cmd.run_ok(&["update-ref", "-d", "HEAD"])
    }
}

pub fn drop_head_commit(cmd: &GitCommand) -> Result<()> {
    let has_parent = cmd
        .execute(&["rev-parse", "--verify", "--quiet", "HEAD^"])?
        .success;
    if !has_parent {
        return Err(GitError::with_stderr(
            "drop failed",
            "cannot drop the root commit; use rebase instead",
        ));
    }
    cmd.run_ok(&["reset", "--hard", "HEAD^"])
}

pub fn pull(cmd: &GitCommand, remote: &str, branch: &str) -> Result<()> {
    cmd.run_ok(&["pull", "--no-rebase", remote, branch])
}

pub fn fetch(cmd: &GitCommand, remote: Option<&str>) -> Result<()> {
    match remote {
        Some(r) => cmd.run_ok(&["fetch", r]),
        None => cmd.run_ok(&["fetch", "--all"]),
    }
}

pub fn push_progress(
    cmd: &GitCommand,
    remote: &str,
    branch: &str,
    set_upstream: bool,
    progress: ProgressHandle,
    cancel: CancelToken,
) -> Result<()> {
    let mut args: Vec<&str> = vec!["push", "--progress"];
    if set_upstream {
        args.push("--set-upstream");
    }
    args.push(remote);
    args.push(branch);
    cmd.run_with_control(&args, progress, cancel)
}

pub fn pull_progress(
    cmd: &GitCommand,
    remote: &str,
    branch: &str,
    progress: ProgressHandle,
    cancel: CancelToken,
) -> Result<()> {
    cmd.run_with_control(
        &["pull", "--no-rebase", "--progress", remote, branch],
        progress,
        cancel,
    )
}

pub fn fetch_progress(
    cmd: &GitCommand,
    remote: Option<&str>,
    progress: ProgressHandle,
    cancel: CancelToken,
) -> Result<()> {
    let mut args: Vec<&str> = vec!["fetch", "--progress"];
    match remote {
        Some(r) => args.push(r),
        None => args.push("--all"),
    }
    cmd.run_with_control(&args, progress, cancel)
}

/// 从 stdin 应用 patch 到 index（--cached）。
/// `reverse` 为 true 时等价 `git apply --cached -R`，用于把已暂存 hunk 撤回工作区。
pub fn apply_patch_cached(cmd: &GitCommand, patch: &str, reverse: bool) -> Result<()> {
    let mut args: Vec<&str> = vec!["apply", "--cached"];
    if reverse {
        args.push("-R");
    }
    args.push("-");
    cmd.run_with_stdin(&args, patch)
}

/// 列出远程仓库（`git remote -v` 的 fetch 行：`name\turl (fetch)`）。
pub fn remote_list(cmd: &GitCommand) -> Result<Vec<Remote>> {
    let out = cmd.run(&["remote", "-v"])?;
    let mut remotes: Vec<Remote> = Vec::new();
    for line in out.lines() {
        // 每个远程会输出 fetch / push 两行，只取 fetch 行。
        let Some(rest) = line.strip_suffix(" (fetch)") else {
            continue;
        };
        let Some((name, url)) = rest.split_once('\t') else {
            continue;
        };
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() {
            continue;
        }
        if !remotes.iter().any(|r| r.name == name) {
            remotes.push(Remote {
                name: name.to_string(),
                url: url.to_string(),
            });
        }
    }
    Ok(remotes)
}

pub fn remote_add(cmd: &GitCommand, name: &str, url: &str) -> Result<()> {
    cmd.run_ok(&["remote", "add", name, url])
}

pub fn remote_remove(cmd: &GitCommand, name: &str) -> Result<()> {
    cmd.run_ok(&["remote", "remove", name])
}

/// 清理远程已删除分支的本地引用（`git remote prune`）。
pub fn remote_prune(cmd: &GitCommand, name: &str) -> Result<()> {
    cmd.run_ok(&["remote", "prune", name])
}

/// 设置分支的上游（`git branch --set-upstream-to=`）。
pub fn set_upstream(cmd: &GitCommand, branch: &str, upstream: &str) -> Result<()> {
    cmd.run_ok(&["branch", &format!("--set-upstream-to={upstream}"), branch])
}

/// 清除分支的上游关联。
pub fn unset_upstream(cmd: &GitCommand, branch: &str) -> Result<()> {
    cmd.run_ok(&["branch", "--unset-upstream", branch])
}

pub fn checkout(cmd: &GitCommand, target: &str) -> Result<()> {
    cmd.run_ok(&["checkout", target])
}

pub fn create_branch(cmd: &GitCommand, name: &str, start_point: Option<&str>) -> Result<()> {
    match start_point {
        Some(start) => cmd.run_ok(&["branch", name, start]),
        None => cmd.run_ok(&["branch", name]),
    }
}

pub fn delete_branch(cmd: &GitCommand, name: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    cmd.run_ok(&["branch", flag, name])
}

pub fn stash_push(cmd: &GitCommand, message: Option<&str>, include_untracked: bool) -> Result<()> {
    let mut args = vec!["stash", "push"];
    if include_untracked {
        args.push("--include-untracked");
    }
    if let Some(m) = message {
        args.push("-m");
        args.push(m);
    }
    cmd.run_ok(&args)
}

pub fn stash_pop(cmd: &GitCommand) -> Result<()> {
    cmd.run_ok(&["stash", "pop"])
}

pub fn stash_list(cmd: &GitCommand) -> Result<Vec<StashEntry>> {
    let output = cmd.execute(&["stash", "list", "--format=%gd%x1f%gs"])?;
    if !output.success {
        return Err(GitError::with_stderr("git stash list failed", output.stderr));
    }
    Ok(parse_stash_list(&output.stdout))
}

pub fn parse_stash_list(stdout: &str) -> Vec<StashEntry> {
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.splitn(2, '\x1f');
        let (Some(ref_name), Some(message)) = (parts.next(), parts.next()) else {
            continue;
        };
        let ref_name = ref_name.trim().to_string();
        if ref_name.is_empty() {
            continue;
        }
        let index = ref_name
            .find('{')
            .and_then(|start| {
                let end = ref_name[start + 1..].find('}')? + start + 1;
                ref_name[start + 1..end].parse::<usize>().ok()
            })
            .unwrap_or(entries.len());
        entries.push(StashEntry {
            index,
            ref_name,
            message: message.trim().to_string(),
        });
    }
    entries
}

pub fn stash_apply_at(cmd: &GitCommand, index: usize) -> Result<()> {
    let rev = format!("stash@{{{}}}", index);
    cmd.run_ok(&["stash", "apply", &rev])
}

pub fn reflog(cmd: &GitCommand, limit: usize) -> Result<Vec<ReflogEntry>> {
    let output = cmd.execute(&[
        "reflog",
        "--format=%gd\x1f%H\x1f%h\x1f%gs",
        "-n",
        &limit.to_string(),
    ])?;
    if !output.success {
        return Err(GitError::with_stderr("git reflog failed", output.stderr));
    }
    Ok(parse_reflog(&output.stdout))
}

pub fn parse_reflog(stdout: &str) -> Vec<ReflogEntry> {
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.splitn(4, '\x1f');
        let (Some(selector), Some(commit_id), Some(short_id)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let message = parts.next().unwrap_or("").trim().to_string();
        entries.push(ReflogEntry {
            selector: selector.trim().to_string(),
            commit_id: commit_id.trim().to_string(),
            short_id: short_id.trim().to_string(),
            message,
        });
    }
    entries
}

pub fn stash_drop_at(cmd: &GitCommand, index: usize) -> Result<()> {
    let rev = format!("stash@{{{}}}", index);
    cmd.run_ok(&["stash", "drop", &rev])
}

pub fn discard_changes(cmd: &GitCommand, path: &str) -> Result<()> {
    cmd.run_ok(&["checkout", "HEAD", "--", path])
}

pub fn remove_untracked(cmd: &GitCommand, path: &str) -> Result<()> {
    cmd.run_ok(&["clean", "-f", "--", path])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    fn flag(self) -> &'static str {
        match self {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        }
    }
}

pub fn cherry_pick(cmd: &GitCommand, commit: &str) -> Result<()> {
    cmd.run_ok(&["cherry-pick", commit])
}

pub fn revert(cmd: &GitCommand, commit: &str) -> Result<()> {
    cmd.run_ok(&["revert", "--no-edit", commit])
}

pub fn reset_to(cmd: &GitCommand, target: &str, mode: ResetMode) -> Result<()> {
    cmd.run_ok(&["reset", mode.flag(), target])
}

pub fn create_tag(
    cmd: &GitCommand,
    name: &str,
    commit: Option<&str>,
    message: Option<&str>,
) -> Result<()> {
    let mut args = vec!["tag"];
    if let Some(msg) = message {
        args.push("-a");
        args.push("-m");
        args.push(msg);
    }
    args.push(name);
    if let Some(c) = commit {
        args.push(c);
    }
    cmd.run_ok(&args)
}

pub fn delete_tag(cmd: &GitCommand, name: &str) -> Result<()> {
    cmd.run_ok(&["tag", "-d", name])
}

/// 推送单个 tag（refs/tags 全限定名，避免与同名分支歧义）。
pub fn push_tag(cmd: &GitCommand, remote: &str, tag: &str) -> Result<()> {
    cmd.run_ok(&["push", remote, &format!("refs/tags/{tag}")])
}

/// 编辑 annotated tag 消息：git 无原位修改，用 -f 以同 commit 重建。
pub fn recreate_tag(cmd: &GitCommand, name: &str, commit: &str, message: &str) -> Result<()> {
    cmd.run_ok(&["tag", "-f", "-a", "-m", message, name, commit])
}

pub fn parse_tags(stdout: &str) -> Vec<Tag> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let peeled = parts.next().unwrap_or("").trim();
            let object = parts.next().unwrap_or("").trim();
            let commit_id = if peeled.is_empty() { object } else { peeled };
            Some(Tag {
                name,
                commit_id: commit_id.to_string(),
            })
        })
        .collect()
}

pub fn show_files(cmd: &GitCommand, commit: &str) -> Result<Vec<Change>> {
    let output = cmd.execute(&["show", "--name-status", "--format=", commit])?;
    if !output.success {
        return Err(GitError::with_stderr("git show failed", output.stderr));
    }
    Ok(parse_name_status(&output.stdout))
}

pub fn parse_name_status(stdout: &str) -> Vec<Change> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let code = parts.next()?;
            let status = ChangeStatus::from_letter(code.chars().next()?);
            match status {
                ChangeStatus::Renamed | ChangeStatus::Copied => {
                    let original_path = parts.next()?.to_string();
                    let path = parts.next()?.to_string();
                    Some(Change {
                        status,
                        path,
                        original_path: Some(original_path),
                        staged: true,
                    })
                }
                _ => {
                    let path = parts.next()?.to_string();
                    Some(Change {
                        status,
                        path,
                        original_path: None,
                        staged: true,
                    })
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_name_status_basic() {
        let out = "M\tsrc/main.rs\nA\tsrc/new.rs\nD\told.txt\n";
        let changes = parse_name_status(out);
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].status, ChangeStatus::Modified);
        assert_eq!(changes[0].path, "src/main.rs");
        assert!(changes[0].staged);
        assert_eq!(changes[1].status, ChangeStatus::Added);
        assert_eq!(changes[2].status, ChangeStatus::Deleted);
        assert!(changes.iter().all(|c| c.original_path.is_none()));
    }

    #[test]
    fn test_parse_name_status_rename() {
        let out = "R100\told_name.rs\tnew_name.rs\n";
        let changes = parse_name_status(out);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].status, ChangeStatus::Renamed);
        assert_eq!(changes[0].original_path.as_deref(), Some("old_name.rs"));
        assert_eq!(changes[0].path, "new_name.rs");
    }

    #[test]
    fn test_parse_name_status_skips_empty_lines() {
        let out = "\nM\ta.rs\n\n";
        let changes = parse_name_status(out);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "a.rs");
    }

    #[test]
    fn test_parse_stash_list_entries() {
        let out = "stash@{0}\x1fWIP on main: abc1234 work\nstash@{1}\x1fOn feature: tweak\n";
        let entries = parse_stash_list(out);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].ref_name, "stash@{0}");
        assert_eq!(entries[0].message, "WIP on main: abc1234 work");
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].message, "On feature: tweak");
    }

    #[test]
    fn test_parse_stash_list_skips_bad_lines() {
        let out = "garbage\nstash\x1ffallback ref\nstash@{2}\x1ffallback\n\n";
        let entries = parse_stash_list(out);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].message, "fallback ref");
        assert_eq!(entries[1].index, 2);
    }

    #[test]
    fn test_parse_reflog_entries() {
        let out = concat!(
            "HEAD@{0}\x1f0123456789abcdef0123456789abcdef01234567\x1f0123456\x1fcommit: fix bug\n",
            "HEAD@{1}\x1f89abcdef0123456789abcdef0123456789abcdef\x1f89abcde\x1fcheckout: moving from main to dev\n",
        );
        let entries = parse_reflog(out);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].selector, "HEAD@{0}");
        assert_eq!(entries[0].commit_id, "0123456789abcdef0123456789abcdef01234567");
        assert_eq!(entries[0].short_id, "0123456");
        assert_eq!(entries[0].message, "commit: fix bug");
        assert_eq!(entries[1].message, "checkout: moving from main to dev");
    }

    #[test]
    fn test_parse_reflog_tolerates_missing_message_and_bad_lines() {
        let out = "HEAD@{0}\x1faaa\x1faaa111\nnot-a-reflog-line\n\n";
        let entries = parse_reflog(out);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].selector, "HEAD@{0}");
        assert_eq!(entries[0].message, "");
    }
}
