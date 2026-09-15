use std::path::PathBuf;

use super::command::GitCommand;
use super::error::{GitError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseActionKind {
    Pick,
    Squash,
    Fixup,
    Drop,
    Edit,
}

impl RebaseActionKind {
    pub fn keyword(self) -> &'static str {
        match self {
            RebaseActionKind::Pick => "pick",
            RebaseActionKind::Squash => "squash",
            RebaseActionKind::Fixup => "fixup",
            RebaseActionKind::Drop => "drop",
            RebaseActionKind::Edit => "edit",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RebaseActionKind::Pick => "Pick",
            RebaseActionKind::Squash => "Squash",
            RebaseActionKind::Fixup => "Fixup",
            RebaseActionKind::Drop => "Drop",
            RebaseActionKind::Edit => "Edit",
        }
    }

    pub fn next(self) -> Self {
        match self {
            RebaseActionKind::Pick => RebaseActionKind::Squash,
            RebaseActionKind::Squash => RebaseActionKind::Fixup,
            RebaseActionKind::Fixup => RebaseActionKind::Drop,
            RebaseActionKind::Drop => RebaseActionKind::Edit,
            RebaseActionKind::Edit => RebaseActionKind::Pick,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RebaseAction {
    pub id: String,
    pub subject: String,
    pub kind: RebaseActionKind,
}

pub fn todos(cmd: &GitCommand, base: &str) -> Result<Vec<RebaseAction>> {
    let range = format!("{base}..HEAD");
    let output = cmd.execute(&["log", "--reverse", "--format=%H%x1f%s", &range])?;
    if !output.success {
        return Err(GitError::with_stderr(
            "git log for rebase failed",
            output.stderr,
        ));
    }
    Ok(parse_todos(&output.stdout))
}

pub fn parse_todos(stdout: &str) -> Vec<RebaseAction> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\x1f');
            let id = parts.next()?.trim().to_string();
            let subject = parts.next()?.trim().to_string();
            if id.is_empty() {
                return None;
            }
            Some(RebaseAction {
                id,
                subject,
                kind: RebaseActionKind::Pick,
            })
        })
        .collect()
}

pub fn render_todo(plan: &[RebaseAction]) -> String {
    let mut out = String::new();
    for action in plan {
        let short = &action.id[..action.id.len().min(10)];
        out.push_str(&format!(
            "{} {} {}\n",
            action.kind.keyword(),
            short,
            action.subject
        ));
    }
    out
}

pub fn run(cmd: &GitCommand, base: &str, plan: &[RebaseAction]) -> Result<()> {
    if plan.is_empty() {
        return Err(GitError::with_stderr("rebase aborted", "empty rebase plan"));
    }
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("rebased-rs-todo-{}", unique));
    std::fs::write(&tmp, render_todo(plan))
        .map_err(|e| GitError::with_stderr("failed to write rebase todo", e.to_string()))?;
    let editor = format!("cp {}", tmp.display());
    let output = cmd.execute_env(
        &["rebase", "-i", base],
        &[
            ("GIT_SEQUENCE_EDITOR", editor.as_str()),
            ("GIT_EDITOR", "true"),
        ],
    );
    let _ = std::fs::remove_file(&tmp);
    let output = output?;
    if !output.success {
        return Err(GitError::with_stderr(
            "interactive rebase failed",
            output.stderr,
        ));
    }
    if in_progress(cmd) {
        return Err(GitError::with_stderr(
            "rebase stopped for editing",
            "rebase paused at an 'edit' action; amend or commit, then continue the rebase",
        ));
    }
    Ok(())
}

pub fn abort(cmd: &GitCommand) -> Result<()> {
    cmd.run_ok(&["rebase", "--abort"])
}

pub fn continue_rebase(cmd: &GitCommand) -> Result<()> {
    let output = cmd.execute_env(&["rebase", "--continue"], &[("GIT_EDITOR", "true")])?;
    if !output.success {
        return Err(GitError::with_stderr(
            "rebase continue failed",
            output.stderr,
        ));
    }
    Ok(())
}

fn full_sha(cmd: &GitCommand, rev: &str) -> Result<String> {
    let output = cmd.execute(&["rev-parse", rev])?;
    if !output.success {
        return Err(GitError::with_stderr(
            format!("rev-parse {rev} failed"),
            output.stderr,
        ));
    }
    Ok(output.stdout.trim().to_string())
}

pub fn reword(cmd: &GitCommand, commit: &str, message: &str) -> Result<()> {
    let full = full_sha(cmd, commit)?;
    let head = full_sha(cmd, "HEAD")?;
    if full == head {
        let staged = cmd.execute(&["diff", "--cached", "--quiet"])?;
        if !staged.success {
            return Err(GitError::with_stderr(
                "reword failed",
                "staged changes would be swept into the amend; unstage them first",
            ));
        }
        return cmd.run_ok(&["commit", "--amend", "-m", message]);
    }
    let parent = full_sha(cmd, &format!("{full}^"))?;
    let base_todos = todos(cmd, &parent)?;
    if !base_todos.iter().any(|action| action.id == full) {
        return Err(GitError::with_stderr(
            "reword failed",
            format!("{commit} is not found in the rebase plan"),
        ));
    }
    let mut todo = String::new();
    for action in base_todos {
        let keyword = if action.id == full {
            "reword"
        } else {
            action.kind.keyword()
        };
        let short = &action.id[..action.id.len().min(10)];
        todo.push_str(&format!("{keyword} {short} {}\n", action.subject));
    }
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tmp_todo = std::env::temp_dir()
        .join(format!("rebased-rs-reword-todo-{}", unique));
    let tmp_msg = std::env::temp_dir()
        .join(format!("rebased-rs-reword-msg-{}", unique));
    std::fs::write(&tmp_todo, todo)
        .map_err(|e| GitError::with_stderr("failed to write reword todo", e.to_string()))?;
    std::fs::write(&tmp_msg, message)
        .map_err(|e| GitError::with_stderr("failed to write reword message", e.to_string()))?;
    let seq_editor = format!("cp {}", tmp_todo.display());
    let msg_editor = format!("cp {}", tmp_msg.display());
    let output = cmd.execute_env(
        &["rebase", "-i", &parent],
        &[
            ("GIT_SEQUENCE_EDITOR", seq_editor.as_str()),
            ("GIT_EDITOR", msg_editor.as_str()),
        ],
    );
    let _ = std::fs::remove_file(&tmp_todo);
    let _ = std::fs::remove_file(&tmp_msg);
    match output {
        Err(err) => {
            let _ = cmd.run_ok(&["rebase", "--abort"]);
            Err(err)
        }
        Ok(out) if !out.success => {
            let _ = cmd.run_ok(&["rebase", "--abort"]);
            Err(GitError::with_stderr(
                "interactive rebase failed",
                out.stderr,
            ))
        }
        Ok(_) => Ok(()),
    }
}

pub fn in_progress(cmd: &GitCommand) -> bool {
    let Ok(out) = cmd.run(&["rev-parse", "--git-dir"]) else {
        return false;
    };
    let git_dir = PathBuf::from(out.trim());
    let base = if git_dir.is_absolute() {
        git_dir
    } else {
        cmd.workdir().join(git_dir)
    };
    base.join("rebase-merge").exists() || base.join("rebase-apply").exists()
}

pub fn stopped_commit(cmd: &GitCommand) -> Option<String> {
    let output = cmd
        .execute(&["rev-parse", "-q", "--verify", "REBASE_HEAD"])
        .ok()?;
    output.success.then(|| output.stdout.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_todo_lines() {
        let plan = vec![
            RebaseAction {
                id: "1234567890abcdef".into(),
                subject: "first".into(),
                kind: RebaseActionKind::Pick,
            },
            RebaseAction {
                id: "fedcba0987".into(),
                subject: "second".into(),
                kind: RebaseActionKind::Squash,
            },
        ];
        let todo = render_todo(&plan);
        assert_eq!(
            todo,
            "pick 1234567890 first\nsquash fedcba0987 second\n"
        );
    }

    #[test]
    fn parse_todos_defaults_to_pick() {
        let todos = parse_todos("aaa111\x1fadd file\nbbb222\x1fupdate file\n");
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].id, "aaa111");
        assert_eq!(todos[0].subject, "add file");
        assert_eq!(todos[0].kind, RebaseActionKind::Pick);
        assert_eq!(todos[1].subject, "update file");
    }

    #[test]
    fn parse_todos_skips_blank_lines() {
        assert!(parse_todos("\n\n").is_empty());
    }

    #[test]
    fn kind_cycles() {
        let mut kind = RebaseActionKind::Pick;
        for _ in 0..5 {
            kind = kind.next();
        }
        assert_eq!(kind, RebaseActionKind::Pick);
        assert_eq!(RebaseActionKind::Fixup.next(), RebaseActionKind::Drop);
        assert_eq!(RebaseActionKind::Drop.next(), RebaseActionKind::Edit);
        assert_eq!(RebaseActionKind::Edit.keyword(), "edit");
    }
}
