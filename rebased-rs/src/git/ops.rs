use super::command::GitCommand;
use super::error::{GitError, Result};
use super::types::{Change, ChangeStatus, Tag};

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

pub fn pull(cmd: &GitCommand, remote: &str, branch: &str) -> Result<()> {
    cmd.run_ok(&["pull", remote, branch])
}

pub fn fetch(cmd: &GitCommand, remote: Option<&str>) -> Result<()> {
    match remote {
        Some(r) => cmd.run_ok(&["fetch", r]),
        None => cmd.run_ok(&["fetch", "--all"]),
    }
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
}
