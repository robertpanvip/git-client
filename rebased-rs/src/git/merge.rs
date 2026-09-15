use super::command::GitCommand;
use super::error::{GitError, Result};

pub fn merge_branch(cmd: &GitCommand, branch: &str) -> Result<()> {
    cmd.run_ok(&["merge", "--no-edit", branch])
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
