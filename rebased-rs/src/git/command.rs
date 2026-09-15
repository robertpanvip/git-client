use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::error::{GitError, Result};

/// 后台网络操作的共享进度槽，保存最新一行进度文本。
pub type ProgressHandle = Arc<Mutex<Option<String>>>;
/// 后台网络操作的取消令牌；置位后子进程被终止。
pub type CancelToken = Arc<AtomicBool>;

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
        self.execute_env(args, &[])
    }

    pub fn execute_env(&self, args: &[&str], envs: &[(&str, &str)]) -> Result<CommandOutput> {
        let mut cmd = Command::new(&self.git_path);
        cmd.current_dir(&self.workdir)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C");
        for (key, value) in envs {
            cmd.env(key, value);
        }
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

    /// 从 stdin 喂入内容执行 git 命令（如 `git apply --cached -`）。
    pub fn run_with_stdin(&self, args: &[&str], input: &str) -> Result<()> {
        let mut cmd = Command::new(&self.git_path);
        cmd.current_dir(&self.workdir)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| GitError::new("failed to open git stdin"))?;
            stdin.write_all(input.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(GitError::with_stderr(
                format!("git {} failed", args.first().copied().unwrap_or("")),
                String::from_utf8(output.stderr)?,
            ))
        }
    }

    /// 启动 git 子进程并支持进度与取消：stderr 逐行解析（兼容 `\r` 原地刷新的
    /// 进度行），最新进度写入 `progress`；`cancel` 置位后终止子进程并返回取消错误。
    pub fn run_with_control(
        &self,
        args: &[&str],
        progress: ProgressHandle,
        cancel: CancelToken,
    ) -> Result<()> {
        let mut cmd = Command::new(&self.git_path);
        cmd.current_dir(&self.workdir)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let stderr = child.stderr.take();
        let stderr_tail = thread::spawn(move || {
            let mut tail: VecDeque<String> = VecDeque::new();
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(std::result::Result::ok) {
                    // git 用 `\r` 原地刷新进度行，取最后一段即最新状态
                    let Some(seg) = line.rsplit('\r').find(|seg| !seg.trim().is_empty()) else {
                        continue;
                    };
                    let trimmed = seg.trim();
                    if !trimmed.contains('%') {
                        if tail.len() == 8 {
                            tail.pop_front();
                        }
                        tail.push_back(trimmed.to_string());
                    }
                    let text: String = trimmed.chars().take(80).collect();
                    if let Ok(mut slot) = progress.lock() {
                        *slot = Some(text);
                    }
                }
            }
            tail
        });

        // 主线程轮询：优先检测自然退出，取消请求则终止子进程
        let status = loop {
            if let Some(exit) = child.try_wait()? {
                break exit;
            }
            if cancel.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stderr_tail.join();
                return Err(GitError::new("operation cancelled"));
            }
            thread::sleep(Duration::from_millis(100));
        };
        let tail = stderr_tail.join().unwrap_or_default();

        if status.success() {
            return Ok(());
        }
        let detail = tail.into_iter().collect::<Vec<_>>().join("\n");
        Err(GitError::with_stderr(
            format!("git {} failed", args.first().copied().unwrap_or("")),
            if detail.is_empty() {
                "unknown error".to_string()
            } else {
                detail
            },
        ))
    }
}
