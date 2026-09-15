use super::command::GitCommand;
use super::error::{GitError, Result};

/// Merge 的快进策略：默认（允许快进）、强制产生合并提交、仅允许快进。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeMode {
    Default,
    NoFastForward,
    FastForwardOnly,
}

pub fn merge_branch(cmd: &GitCommand, branch: &str, mode: MergeMode) -> Result<()> {
    let mut args: Vec<&str> = vec!["merge"];
    match mode {
        MergeMode::Default => args.push("--no-edit"),
        MergeMode::NoFastForward => {
            args.push("--no-ff");
            args.push("--no-edit");
        }
        MergeMode::FastForwardOnly => args.push("--ff-only"),
    }
    args.push(branch);
    cmd.run_ok(&args)
}

pub fn continue_merge(cmd: &GitCommand) -> Result<()> {
    let output = cmd.execute_env(&["merge", "--continue"], &[("GIT_EDITOR", "true")])?;
    if !output.success {
        return Err(GitError::with_stderr(
            "merge continue failed",
            output.stderr,
        ));
    }
    Ok(())
}

pub fn abort(cmd: &GitCommand) -> Result<()> {
    cmd.run_ok(&["merge", "--abort"])
}

pub fn in_progress(cmd: &GitCommand) -> bool {
    matches!(
        cmd.execute(&["rev-parse", "-q", "--verify", "MERGE_HEAD"]),
        Ok(output) if output.success
    )
}
