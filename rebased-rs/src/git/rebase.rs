use std::path::{Path, PathBuf};

use super::command::GitCommand;
use super::error::{GitError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseActionKind {
    Pick,
    Squash,
    Fixup,
    Drop,
    Edit,
    Reword,
}

impl RebaseActionKind {
    pub fn keyword(self) -> &'static str {
        match self {
            RebaseActionKind::Pick => "pick",
            RebaseActionKind::Squash => "squash",
            RebaseActionKind::Fixup => "fixup",
            RebaseActionKind::Drop => "drop",
            RebaseActionKind::Edit => "edit",
            RebaseActionKind::Reword => "reword",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RebaseActionKind::Pick => "Pick",
            RebaseActionKind::Squash => "Squash",
            RebaseActionKind::Fixup => "Fixup",
            RebaseActionKind::Drop => "Drop",
            RebaseActionKind::Edit => "Edit",
            RebaseActionKind::Reword => "Reword",
        }
    }

    pub fn next(self) -> Self {
        match self {
            RebaseActionKind::Pick => RebaseActionKind::Squash,
            RebaseActionKind::Squash => RebaseActionKind::Fixup,
            RebaseActionKind::Fixup => RebaseActionKind::Drop,
            RebaseActionKind::Drop => RebaseActionKind::Edit,
            RebaseActionKind::Edit => RebaseActionKind::Reword,
            RebaseActionKind::Reword => RebaseActionKind::Pick,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseAction {
    pub id: String,
    pub subject: String,
    pub kind: RebaseActionKind,
    /// 计划编辑器里为该提交自定义的消息。非空时用于 Reword / Squash 的消息改写。
    pub message: Option<String>,
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
                message: None,
            })
        })
        .collect()
}

/// subject 以 `squash!` / `fixup!` 开头时返回对应的合并动作。
fn autosquash_kind(subject: &str) -> Option<RebaseActionKind> {
    if subject.starts_with("squash!") {
        Some(RebaseActionKind::Squash)
    } else if subject.starts_with("fixup!") {
        Some(RebaseActionKind::Fixup)
    } else {
        None
    }
}

/// 剥离一层 `fixup!` / `squash!` 前缀，得到 autosquash 的目标提交主题。
fn autosquash_target(subject: &str) -> Option<String> {
    let rest = subject
        .strip_prefix("fixup!")
        .or_else(|| subject.strip_prefix("squash!"))?;
    Some(rest.trim_start().to_string())
}

/// 按 `git rebase --autosquash` 的语义重排计划：subject 以 `fixup!` / `squash!`
/// 开头的提交被移到目标提交（完整 subject 匹配剥离前缀后的内容）之后，并把 kind
/// 改为 Fixup / Squash；找不到目标的提交保持原位。函数幂等，重复应用结果不变。
///
/// 不使用 git 原生 `--autosquash` 标志：该流程的 todo 文件由
/// [`run`] 通过 `GIT_SEQUENCE_EDITOR=cp` 整体注入，会覆盖 git 的自动重排，
/// 因此等价逻辑在 Rust 侧完成，便于测试与预览。
pub fn autosquash_plan(plan: Vec<RebaseAction>) -> Vec<RebaseAction> {
    let mut result: Vec<RebaseAction> = Vec::with_capacity(plan.len());
    // 完整 subject -> result 中对应合并组的末尾下标。已移位的 fixup 项也会登记，
    // 这样 `fixup! fixup! xxx` 的链式目标能继续命中。
    let mut heads: Vec<(String, usize)> = Vec::new();
    for mut action in plan {
        let kind = autosquash_kind(&action.subject);
        let target = kind.as_ref().and_then(|_| {
            let root = autosquash_target(&action.subject).unwrap_or_default();
            heads
                .iter()
                .rev()
                .find(|(s, _)| *s == root)
                .map(|(_, i)| *i)
        });
        match (kind, target) {
            (Some(kind), Some(at)) => {
                let root = autosquash_target(&action.subject).unwrap_or_default();
                let subject = action.subject.clone();
                action.kind = kind;
                result.insert(at + 1, action);
                // 插入点之后的组头下标整体后移；目标组的末尾推进到新位置，
                // 后续同目标的 fixup 才能按 todo 原顺序接在后面（保证幂等）。
                for head in heads.iter_mut() {
                    if head.1 > at {
                        head.1 += 1;
                    }
                }
                if let Some(head) = heads.iter_mut().find(|(s, _)| *s == root) {
                    head.1 = at + 1;
                }
                heads.push((subject, at + 1));
            }
            _ => {
                let subject = action.subject.clone();
                result.push(action);
                heads.push((subject, result.len() - 1));
            }
        }
    }
    result
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

pub fn copy_editor(path: &Path) -> String {
    let rendered = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"");
    format!("cp \"{rendered}\"")
}

/// 生成一份 POSIX `sh` 脚本，用作 `rebase -i` 的 `GIT_EDITOR`。
///
/// git 会按 todo 顺序，为每一条 `reword` / `squash` / `fixup` 调用一次编辑器（传入
/// 待改写的消息文件 `$1`）。脚本用一个计数器文件记录「这是第几次调用」，再按 `entries`
/// 中对应的自定义消息覆盖文件——`entries` 与 plan 里 reword/squash/fixup 的出现顺序
/// 一一对应，某条没有自定义消息则为 `None`（保持 git 默认行为，不清空文件）。
///
/// 为什么不用「按提交主题匹配」：reword 会改变主题，导致后续 squash/fixup 的自定义
/// 消息因主题对不上而静默失效；同主题多条 entry 还会互相覆盖。按调用次序匹配则无此问题。
pub fn editor_script(entries: &[Option<String>]) -> String {
    let mut out = String::from("#!/bin/sh\n");
    // 计数器文件路径从环境变量注入；缺失时从 0 开始（每次 rebase 用唯一文件名，不会残留）。
    out.push_str("STATE=\"${REBASED_EDIT_CNT:-}\"\n");
    out.push_str("n=0\n[ -f \"$STATE\" ] && n=$(cat \"$STATE\" 2>/dev/null || echo 0)\n");
    out.push_str("case $n in\n");
    for (i, msg) in entries.iter().enumerate() {
        out.push_str(&format!("  {i})"));
        match msg {
            Some(m) => out.push_str(&format!(
                " printf '%s\\n' {} > \"$1\";;",
                shell_single_quoted(m)
            )),
            None => out.push_str(" :;;"),
        }
        out.push('\n');
    }
    out.push_str("esac\n");
    out.push_str("n=$((n+1))\n");
    out.push_str("printf '%s' \"$n\" > \"$STATE\"\n");
    out
}

/// 用合法的 shell 单引号字面量包装字符串，内嵌单引号按 `'\"'\"'` 拼接转义。
fn shell_single_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// 把路径转成可放进 shell 命令的带引号字符串（借鉴 copy_editor 的归一化）。
pub fn quote_path(path: &Path) -> String {
    let rendered = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"");
    format!("\"{rendered}\"")
}

pub fn run(cmd: &GitCommand, base: &str, plan: &[RebaseAction]) -> Result<()> {
    if plan.is_empty() {
        return Err(GitError::with_stderr("rebase aborted", "empty rebase plan"));
    }
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("rebased-rs-todo-{}", unique));
    std::fs::write(&tmp, render_todo(plan))
        .map_err(|e| GitError::with_stderr("failed to write rebase todo", e.to_string()))?;
    let editor = copy_editor(&tmp);

    // 收集计划中自定义的消息：非空时生成一份 GIT_EDITOR 脚本，让 git 在 reword /
    // squash 时按消息文件首行匹配并替换成自定义消息；为空则沿用默认的 no-op 编辑器。
    // 注意 git 在 squash 时打开编辑器收到的是「合并后的消息」，其首行等于所属合并组
    // 的第一个目标提交主题，因此 squash/fixup 的自定义消息要按目标提交来匹配。
    // 按 plan 顺序，为每条 reword/squash/fixup 记录其自定义消息（无则 None）。
    // git 会按此顺序逐一调用 GIT_EDITOR，脚本据此把第 n 次调用映射到第 n 条消息，
    // 避免 reword 改主题后 squash/fixup 的自定义消息因主题不匹配而静默失效。
    let mut ordered: Vec<Option<String>> = Vec::new();
    for action in plan {
        match action.kind {
            RebaseActionKind::Reword | RebaseActionKind::Squash | RebaseActionKind::Fixup => {
                ordered.push(
                    action
                        .message
                        .as_deref()
                        .filter(|m| !m.trim().is_empty())
                        .map(|m| m.to_string()),
                );
            }
            _ => {}
        }
    }
    let (message_editor, script_path, counter_path) = if ordered.iter().all(|m| m.is_none()) {
        ("true".to_string(), None, None)
    } else {
        let sp = std::env::temp_dir().join(format!("rebased-rs-editor-{}", unique));
        let cp = std::env::temp_dir().join(format!("rebased-rs-editcnt-{}", unique));
        std::fs::write(&sp, editor_script(&ordered)).map_err(|e| {
            GitError::with_stderr("failed to write rebase message editor", e.to_string())
        })?;
        (format!("sh {}", quote_path(&sp)), Some(sp), Some(cp))
    };
    let counter_owned = counter_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let mut env: Vec<(&str, &str)> = vec![
        ("GIT_SEQUENCE_EDITOR", editor.as_str()),
        ("GIT_EDITOR", message_editor.as_str()),
    ];
    if let Some(cs) = &counter_owned {
        // 计数器文件：脚本据此把第 n 次编辑器调用映射到 plan 中第 n 条 reword/squash/fixup。
        env.push(("REBASED_EDIT_CNT", cs));
    }
    let output = cmd.execute_env(&["rebase", "-i", base], &env);
    let _ = std::fs::remove_file(&tmp);
    if let Some(sp) = &script_path {
        let _ = std::fs::remove_file(sp);
    }
    if let Some(cp) = &counter_path {
        let _ = std::fs::remove_file(cp);
    }
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
    let tmp_todo = std::env::temp_dir().join(format!("rebased-rs-reword-todo-{}", unique));
    let tmp_msg = std::env::temp_dir().join(format!("rebased-rs-reword-msg-{}", unique));
    std::fs::write(&tmp_todo, todo)
        .map_err(|e| GitError::with_stderr("failed to write reword todo", e.to_string()))?;
    std::fs::write(&tmp_msg, message)
        .map_err(|e| GitError::with_stderr("failed to write reword message", e.to_string()))?;
    let seq_editor = copy_editor(&tmp_todo);
    let msg_editor = copy_editor(&tmp_msg);
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

/// 将 `commit` 合并进其父提交（通过 `rebase -i`）：`Fixup` 丢弃原提交信息，
/// `Squash` 保留（并可用 `message` 覆盖）合并后的提交信息。该提交必须是
/// rebase 计划中紧随其父的一条（即逻辑上的直接子提交）。
fn combine_into_parent(
    cmd: &GitCommand,
    commit: &str,
    kind: RebaseActionKind,
    message: Option<&str>,
) -> Result<()> {
    match kind {
        RebaseActionKind::Fixup | RebaseActionKind::Squash => {}
        _ => {
            return Err(GitError::with_stderr(
                "combine failed",
                "unsupported combine kind",
            ));
        }
    }
    let full = full_sha(cmd, commit)?;
    let parent = full_sha(cmd, &format!("{full}^"))?;
    let base_todos = todos(cmd, &parent)?;
    let Some(target_idx) = base_todos.iter().position(|action| action.id == full) else {
        return Err(GitError::with_stderr(
            "combine failed",
            format!("{commit} is not found in the rebase plan"),
        ));
    };
    if target_idx == 0 {
        return Err(GitError::with_stderr(
            "combine failed",
            "cannot combine a commit into a commit that has no pick before it",
        ));
    }
    let mut todo = String::new();
    // squash/fixup 必须紧跟其吸收者之后，因此把它并到前一个 `pick` 上。
    let mut plan: Vec<RebaseAction> = base_todos[..target_idx].to_vec();
    plan.push(RebaseAction {
        kind,
        message: message.map(str::to_string).filter(|m| !m.trim().is_empty()),
        ..base_todos[target_idx].clone()
    });
    for tail in &base_todos[target_idx + 1..] {
        plan.push(tail.clone());
    }
    for action in &plan {
        let short = &action.id[..action.id.len().min(10)];
        todo.push_str(&format!(
            "{} {short} {}\n",
            action.kind.keyword(),
            action.subject
        ));
    }
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tmp_todo = std::env::temp_dir().join(format!("rebased-rs-combine-todo-{}", unique));
    std::fs::write(&tmp_todo, todo)
        .map_err(|e| GitError::with_stderr("failed to write combine todo", e.to_string()))?;
    let seq_editor = copy_editor(&tmp_todo);
    // 按 plan 中 reword/squash/fixup 的出现顺序注入自定义消息（调用次序匹配），
    // 与 run() 保持一致，避免 reword 改主题后 squash 消息按主题匹配而失效。
    let combine_ordered: Vec<Option<String>> = plan
        .iter()
        .map(|a| match a.kind {
            RebaseActionKind::Reword | RebaseActionKind::Squash | RebaseActionKind::Fixup => a
                .message
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .map(|m| m.to_string()),
            _ => None,
        })
        .collect();
    let (msg_editor, combine_script, combine_counter) = if combine_ordered.iter().all(|m| m.is_none())
    {
        ("true".to_string(), None, None)
    } else {
        let sp = std::env::temp_dir().join(format!("rebased-rs-combine-editor-{}", unique));
        let cp = std::env::temp_dir().join(format!("rebased-rs-combine-cnt-{}", unique));
        std::fs::write(&sp, editor_script(&combine_ordered)).map_err(|e| {
            GitError::with_stderr("failed to write combine message editor", e.to_string())
        })?;
        (format!("sh {}", quote_path(&sp)), Some(sp), Some(cp))
    };
    let counter_owned = combine_counter
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());
    let mut env: Vec<(&str, &str)> = vec![
        ("GIT_SEQUENCE_EDITOR", seq_editor.as_str()),
        ("GIT_EDITOR", msg_editor.as_str()),
    ];
    if let Some(cs) = &counter_owned {
        env.push(("REBASED_EDIT_CNT", cs));
    }
    let output = cmd.execute_env(&["rebase", "-i", &parent], &env);
    let _ = std::fs::remove_file(&tmp_todo);
    if let Some(sp) = &combine_script {
        let _ = std::fs::remove_file(sp);
    }
    if let Some(cp) = &combine_counter {
        let _ = std::fs::remove_file(cp);
    }
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

/// 把 `commit` 的压力并入其父提交，补充信息保留（git 默认合并两者消息）。
pub fn fixup(cmd: &GitCommand, commit: &str) -> Result<()> {
    combine_into_parent(cmd, commit, RebaseActionKind::Fixup, None)
}

/// 把 `commit` squash 进其父提交，可选覆盖合并后的提交信息。
pub fn squash(cmd: &GitCommand, commit: &str, message: Option<&str>) -> Result<()> {
    combine_into_parent(cmd, commit, RebaseActionKind::Squash, message)
}

/// 用给定 todo 内容执行一次 `rebase -i base`：todo 整体注入，失败自动 abort。
fn rebase_with_todo(cmd: &GitCommand, base: &str, todo: &str) -> Result<()> {
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("rebased-rs-edit-todo-{}", unique));
    std::fs::write(&tmp, todo)
        .map_err(|e| GitError::with_stderr("failed to write rebase todo", e.to_string()))?;
    let editor = copy_editor(&tmp);
    let output = cmd.execute_env(
        &["rebase", "-i", base],
        &[("GIT_SEQUENCE_EDITOR", editor.as_str())],
    );
    let _ = std::fs::remove_file(&tmp);
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

/// 从历史中删除 `commit`（丢弃其更改），通过 `rebase -i` 把该行改为 `drop`。
pub fn drop_commit(cmd: &GitCommand, commit: &str) -> Result<()> {
    let full = full_sha(cmd, commit)?;
    let parent = full_sha(cmd, &format!("{full}^"))?;
    let base_todos = todos(cmd, &parent)?;
    if !base_todos.iter().any(|action| action.id == full) {
        return Err(GitError::with_stderr(
            "drop failed",
            format!("{commit} is not found in the rebase plan"),
        ));
    }
    let mut todo = String::new();
    for action in &base_todos {
        let keyword = if action.id == full {
            "drop"
        } else {
            action.kind.keyword()
        };
        let short = &action.id[..action.id.len().min(10)];
        todo.push_str(&format!("{keyword} {short} {}\n", action.subject));
    }
    rebase_with_todo(cmd, &parent, &todo)
}

/// 把 `commit` 从历史中删除，但把其差异保留回工作区（暂存），即 IntelliJ 的
/// Uncommit（非 HEAD 提交）。流程：先生成该提交相对其父的补丁，再 `rebase -i`
/// 丢弃该提交，最后 `git apply --cached` 还原差异。
pub fn uncommit_commit(cmd: &GitCommand, commit: &str) -> Result<()> {
    let full = full_sha(cmd, commit)?;
    let parent = full_sha(cmd, &format!("{full}^"))?;
    let patch = cmd
        .execute(&["diff", "--binary", &parent, &full])
        .map_err(|e| GitError::with_stderr("uncommit failed", e.to_string()))?;
    if !patch.success {
        return Err(GitError::with_stderr("uncommit failed", patch.stderr));
    }
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tmp_patch = std::env::temp_dir().join(format!("rebased-rs-uncommit-{}.patch", unique));
    std::fs::write(&tmp_patch, &patch.stdout)
        .map_err(|e| GitError::with_stderr("failed to write uncommit patch", e.to_string()))?;
    let drop_result = drop_commit(cmd, &full);
    if let Err(e) = drop_result {
        let _ = std::fs::remove_file(&tmp_patch);
        return Err(e);
    }
    // commit 已被移除，把差异还原进暂存区。失败时补丁文件已删除，改动不会丢失于工作区。
    let apply = cmd.run_ok(&["apply", "--cached", &tmp_patch.to_string_lossy()]);
    let _ = std::fs::remove_file(&tmp_patch);
    apply
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
                message: None,
            },
            RebaseAction {
                id: "fedcba0987".into(),
                subject: "second".into(),
                kind: RebaseActionKind::Squash,
                message: None,
            },
        ];
        let todo = render_todo(&plan);
        assert_eq!(todo, "pick 1234567890 first\nsquash fedcba0987 second\n");
    }

    #[test]
    fn autosquash_moves_fixups_after_targets() {
        let action = |id: &str, subject: &str| RebaseAction {
            id: id.into(),
            subject: subject.into(),
            kind: RebaseActionKind::Pick,
            message: None,
        };
        let plan = vec![
            action("a", "add api"),
            action("b", "add tests"),
            action("f", "fixup! add api"),
            action("c", "polish docs"),
            action("s", "squash! add api"),
        ];
        let plan = autosquash_plan(plan);
        let ids: Vec<&str> = plan.iter().map(|a| a.id.as_str()).collect();
        // 两个 fixup 目标同为 "add api"：移到目标之后并保持原有相对顺序
        assert_eq!(ids, ["a", "f", "s", "b", "c"]);
        assert_eq!(plan[1].kind, RebaseActionKind::Fixup);
        assert_eq!(plan[2].kind, RebaseActionKind::Squash);
        // 普通提交的 kind 不受影响
        assert_eq!(plan[0].kind, RebaseActionKind::Pick);
    }

    #[test]
    fn autosquash_keeps_unmatched_in_place() {
        let action = |id: &str, subject: &str| RebaseAction {
            id: id.into(),
            subject: subject.into(),
            kind: RebaseActionKind::Pick,
            message: None,
        };
        let plan = vec![action("f", "fixup! missing"), action("a", "real")];
        let plan = autosquash_plan(plan);
        let ids: Vec<&str> = plan.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["f", "a"]);
        // 找不到目标时保持原样，不强行改 kind
        assert_eq!(plan[0].kind, RebaseActionKind::Pick);
    }

    #[test]
    fn autosquash_is_idempotent() {
        let action = |id: &str, subject: &str| RebaseAction {
            id: id.into(),
            subject: subject.into(),
            kind: RebaseActionKind::Pick,
            message: None,
        };
        let plan = vec![
            action("a", "base"),
            action("f1", "fixup! base"),
            action("b", "other"),
            action("f2", "squash! base"),
        ];
        let once = autosquash_plan(plan.clone());
        assert_eq!(autosquash_plan(once.clone()), once);
    }

    #[test]
    fn autosquash_supports_chained_fixups() {
        let action = |id: &str, subject: &str| RebaseAction {
            id: id.into(),
            subject: subject.into(),
            kind: RebaseActionKind::Pick,
            message: None,
        };
        // f2 的目标是 f1（其 subject 本身以 fixup! 开头）
        let plan = vec![
            action("a", "base"),
            action("b", "feature"),
            action("f1", "fixup! feature"),
            action("f2", "fixup! fixup! feature"),
        ];
        let plan = autosquash_plan(plan);
        let ids: Vec<&str> = plan.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "f1", "f2"]);
        assert_eq!(plan[2].kind, RebaseActionKind::Fixup);
        assert_eq!(plan[3].kind, RebaseActionKind::Fixup);
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
        for _ in 0..6 {
            kind = kind.next();
        }
        assert_eq!(kind, RebaseActionKind::Pick);
        assert_eq!(RebaseActionKind::Fixup.next(), RebaseActionKind::Drop);
        assert_eq!(RebaseActionKind::Edit.next(), RebaseActionKind::Reword);
        assert_eq!(RebaseActionKind::Reword.next(), RebaseActionKind::Pick);
        assert_eq!(RebaseActionKind::Reword.keyword(), "reword");
    }

    #[test]
    fn editor_script_applies_messages_by_call_order() {
        let script = editor_script(&[
            Some("Add the API module".to_string()),
            Some("it's here".to_string()),
        ]);
        assert!(script.starts_with("#!/bin/sh\n"));
        // 按编辑器调用次序（case 分支下标）注入，而非按主题匹配。
        assert!(script.contains("case $n in"));
        assert!(script.contains("REBASED_EDIT_CNT"));
        assert!(script.contains("  0) printf '%s\\n' 'Add the API module' > \"$1\";;"));
        // 单引号被转义，整体仍是合法单引号字面量
        assert!(script.contains("it'\\''s here"));
    }

    #[test]
    fn copy_editor_normalizes_windows_paths() {
        assert_eq!(
            copy_editor(Path::new(r"C:\Users\jo doe\todo-1")),
            "cp \"C:/Users/jo doe/todo-1\""
        );
        assert_eq!(
            copy_editor(Path::new("/tmp/rebased-rs-todo-1")),
            "cp \"/tmp/rebased-rs-todo-1\""
        );
        assert_eq!(
            copy_editor(Path::new(r#"C:\we"ird"#)),
            "cp \"C:/we\\\"ird\""
        );
    }
}
