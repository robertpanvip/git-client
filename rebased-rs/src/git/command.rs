use std::path::{Path, PathBuf};
use std::process::Command;

use super::error::{GitError, Result};

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub struct GitCommand {
    workdir: PathBuf,
    git_path: String,
}

impl GitCommand {
    pub fn new(workdir: impl Into<PathBuf>) -> Self {
        Self {
            workdir: workdir.into(),
            git_path: "git".to_string(),
        }
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub fn execute(&self, args: &[&str]) -> Result<CommandOutput> {
        let mut cmd = Command::new(&self.git_path);
        cmd.current_dir(&self.workdir)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C");
        let output = cmd.output()?;
        Ok(CommandOutput {
            stdout: String::from_utf8(output.stdout)?,
            stderr: String::from_utf8(output.stderr)?,
            success: output.status.success(),
        })
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        let out = self.execute(args)?;
        if out.success {
            Ok(out.stdout)
        } else {
            Err(GitError::with_stderr(
                format!("git {} failed", args.first().copied().unwrap_or("")),
                out.stderr,
            ))
        }
    }

    pub fn run_ok(&self, args: &[&str]) -> Result<()> {
        self.run(args).map(|_| ())
    }
}
