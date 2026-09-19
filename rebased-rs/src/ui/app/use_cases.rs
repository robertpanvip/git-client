use std::sync::Arc;

use rebased_rs::git::{
    Commit, GitBackend, GitError, Graph, HunkChoice, RepoData, conflict_hunks, parse_unified_diff,
};
#[cfg(test)]
use rebased_rs::git::{DEFAULT_LOG_LIMIT, load_repo_data, open_backend};

use crate::ui::editor_view::{EditorContent, build_content};

use super::state::{AppState, DiffSource, RebaseFlow, SidebarMode};

pub(crate) fn commit_with_autoadd(
    repo: &dyn GitBackend,
    message: &str,
    amend: bool,
) -> Result<(), GitError> {
    if !amend {
        let status = repo.status()?;
        if status.changes.iter().all(|change| !change.staged) {
            repo.add_all()?;
        }
    }
    repo.commit(message, amend)
}

pub(crate) fn commit_selected(
    repo: &dyn GitBackend,
    message: &str,
    amend: bool,
    paths: &[String],
) -> Result<(), GitError> {
    if paths.is_empty() {
        commit_with_autoadd(repo, message, amend)
    } else {
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        repo.commit_paths(message, &refs, amend)
    }
}

pub(crate) fn sync_repo_state(
    repo: &dyn GitBackend,
    state: &mut AppState,
    data: RepoData,
) -> Result<(Vec<Commit>, Graph), GitError> {
    let RepoData {
        commits,
        graph,
        status,
        branches,
        tags,
        remotes,
    } = data;
    state.repo_root = repo.root().to_string_lossy().to_string();
    // HEAD 必须是仓库真实的 HEAD：日志可能被分支/作者/时间过滤，
    // `commits.first()` 在被过滤时并不等于 HEAD。
    state.head_id = repo
        .rev_parse("HEAD")
        .ok()
        .filter(|id| !id.is_empty())
        .or_else(|| commits.first().map(|commit| commit.id.0.clone()));
    state.changes = status.changes;
    state.branch_entries = Arc::new(branches.clone());
    state.remotes = Arc::new(remotes);
    let current = branches.iter().find(|branch| branch.is_current());
    state.ahead = current.map_or(0, |branch| branch.ahead);
    state.behind = current.map_or(0, |branch| branch.behind);
    let names: Vec<String> = branches
        .iter()
        .filter(|branch| !branch.is_remote)
        .map(|branch| branch.name.clone())
        .collect();
    state.branches = Arc::new(names);
    state.current_branch = current
        .map(|branch| branch.name.clone())
        .or_else(|| repo.current_branch_name().ok());
    state.current_upstream = current.and_then(|branch| branch.upstream.clone());
    state.tags = Arc::new(tags);
    // 上次提交信息：提交区输入框上方展示（对齐 IDEA 的 last commit 提示）。
    state.last_commit_message = repo
        .head_message()
        .ok()
        .map(|message| message.trim_end().to_string())
        .filter(|message| !message.is_empty());
    state.merge_in_progress = repo.is_merge_in_progress();
    sync_rebase_flow(repo, state);
    let rebase_in_progress = state.rebase.in_progress();
    if (rebase_in_progress || state.merge_in_progress)
        && let Err(e) = reload_conflict_state(repo, state)
    {
        state.error = Some(e.to_string());
    }
    sync_views_on_refresh(state, &commits, rebase_in_progress);
    Ok((commits, graph))
}

fn sync_rebase_flow(repo: &dyn GitBackend, state: &mut AppState) {
    let live = repo.is_rebase_in_progress();
    state.rebase = match &state.rebase {
        RebaseFlow::Running { base, plan } | RebaseFlow::Stopped { base, plan, .. } => {
            if live {
                match repo.rebase_stopped_commit() {
                    Some(stopped_at) => RebaseFlow::Stopped {
                        base: base.clone(),
                        plan: plan.clone(),
                        stopped_at,
                    },
                    None => RebaseFlow::Running {
                        base: base.clone(),
                        plan: plan.clone(),
                    },
                }
            } else {
                RebaseFlow::Idle
            }
        }
        _ if live => match repo.rebase_stopped_commit() {
            Some(stopped_at) => RebaseFlow::Stopped {
                base: String::new(),
                plan: Vec::new(),
                stopped_at,
            },
            None => RebaseFlow::Running {
                base: String::new(),
                plan: Vec::new(),
            },
        },
        other => other.clone(),
    };
}

fn sync_views_on_refresh(state: &mut AppState, commits: &[Commit], rebase_in_progress: bool) {
    let selection_alive = state
        .selected
        .as_ref()
        .is_some_and(|selected| commits.iter().any(|commit| commit.id.0 == selected.id.0));
    if !selection_alive {
        state.selected = None;
        state.detail_files.clear();
        state.detail_branches.clear();
    }
    if state.selected.is_none() && state.sidebar == SidebarMode::Detail {
        state.sidebar = SidebarMode::Workspace;
    }
    if !(rebase_in_progress || state.merge_in_progress) {
        state.conflict_files.clear();
        state.conflict_path = None;
        state.conflict_hunks.clear();
        state.conflict_choices.clear();
        state.conflict_raw.clear();
        state.conflict_list.reset(0);
        state.conflict_nav = None;
    }
    state
        .selected_changes
        .retain(|path| state.changes.iter().any(|change| &change.path == path));
    state.prompt = None;
}

pub(crate) fn load_commit_detail(
    repo: &dyn GitBackend,
    state: &mut AppState,
    commit: Commit,
) -> Result<(), GitError> {
    let id = commit.id.0.clone();
    let files = repo.show_files(&id)?;
    let branches = repo.branches_containing(&id)?;
    state.selected = Some(commit);
    state.detail_files = files;
    state.detail_branches = branches;
    state.sidebar = SidebarMode::Detail;
    state.error = None;
    Ok(())
}

pub(crate) fn clear_detail(state: &mut AppState) {
    state.selected = None;
    state.detail_files.clear();
    state.detail_branches.clear();
    state.sidebar = SidebarMode::Workspace;
}

/// 关闭 Blame 面板时清除其数据：关闭 = 清状态，重开 = 重新拉取（B-1），
/// 避免残留旧数据在仓库变化后「复活」（对照 [`clear_detail`] 的语义）。
pub(crate) fn clear_blame(state: &mut AppState) {
    state.blame_groups.clear();
    state.blame_path = String::new();
}

pub(crate) fn open_staged_diff(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: Option<String>,
) -> Result<(), GitError> {
    let stdout = repo.diff_staged(path.as_deref(), state.ignore_whitespace)?;
    state.diff_files = parse_unified_diff(&stdout);
    // 新 diff 内容使旧的折叠/导航坐标失效，统一重置。
    state.diff_folded.clear();
    state.diff_nav = None;
    state.diff_title = match &path {
        Some(p) => format!("Diff · staged · {p}"),
        None => "Diff · staged".to_string(),
    };
    state.diff_path = path;
    state.diff_editing = false;
    state.diff_source = Some(DiffSource::Staged);
    state.diff_commit = None;
    state.sidebar = SidebarMode::Diff;
    state.error = None;
    Ok(())
}

pub(crate) fn open_unstaged_diff(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: Option<String>,
) -> Result<(), GitError> {
    let stdout = repo.diff_unstaged(path.as_deref(), state.ignore_whitespace)?;
    state.diff_files = parse_unified_diff(&stdout);
    state.diff_folded.clear();
    state.diff_nav = None;
    state.diff_title = match &path {
        Some(p) => format!("Diff · unstaged · {p}"),
        None => "Diff · unstaged".to_string(),
    };
    state.diff_path = path;
    state.diff_source = Some(DiffSource::Unstaged);
    state.sidebar = SidebarMode::Diff;
    state.error = None;
    Ok(())
}

pub(crate) fn open_commit_diff(
    repo: &dyn GitBackend,
    state: &mut AppState,
    commit_id: String,
    path: Option<String>,
) -> Result<(), GitError> {
    let short = &commit_id[..commit_id.len().min(7)];
    let stdout = repo.show_diff(&commit_id, path.as_deref(), state.ignore_whitespace)?;
    state.diff_files = parse_unified_diff(&stdout);
    state.diff_folded.clear();
    state.diff_nav = None;
    state.diff_title = match &path {
        Some(p) => format!("{short} · {p}"),
        None => format!("Commit {short}"),
    };
    state.diff_path = None;
    state.diff_editing = false;
    state.diff_source = Some(DiffSource::Commit);
    state.diff_commit = Some(commit_id);
    state.sidebar = SidebarMode::Diff;
    state.error = None;
    Ok(())
}

pub(crate) fn open_file_history(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: String,
) -> Result<(), GitError> {
    state.history_commits = repo.log_follow(100, &path)?;
    state.history_path = path;
    state.sidebar = SidebarMode::History;
    state.error = None;
    Ok(())
}

/// 目录树历史：`git log -- path`（无 `--follow`），对齐 IDEA 目录级 Show History。
pub(crate) fn open_dir_history(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: String,
) -> Result<(), GitError> {
    state.history_commits = repo.log_path(100, &path)?;
    state.history_path = path;
    state.sidebar = SidebarMode::History;
    state.error = None;
    Ok(())
}

pub(crate) fn open_blame(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: String,
) -> Result<(), GitError> {
    // 查看历史提交的文件时，blame 该提交而非永远 blame HEAD（Bug #5）。
    let rev = state
        .diff_commit
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "HEAD".to_string());
    state.blame_groups = repo.blame(&rev, &path)?;
    state.blame_path = path;
    state.sidebar = SidebarMode::Blame;
    state.error = None;
    Ok(())
}

pub(crate) fn open_reflog(repo: &dyn GitBackend, state: &mut AppState) -> Result<(), GitError> {
    state.reflog_entries = repo.reflog(200)?;
    state.sidebar = SidebarMode::Reflog;
    state.error = None;
    Ok(())
}

pub(crate) fn reload_conflict_state(
    repo: &dyn GitBackend,
    state: &mut AppState,
) -> Result<(), GitError> {
    let files = repo.conflicted_files()?;
    state.conflict_files = files;
    if !state.conflict_files.is_empty() {
        let first = state.conflict_files[0].path.clone();
        state.sidebar = SidebarMode::Conflicts;
        select_conflict_file(repo, state, &first)?;
    }
    Ok(())
}

pub(crate) fn select_conflict_file(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: &str,
) -> Result<(), GitError> {
    let raw = repo.conflict_file_content(path)?;
    let hunks = conflict_hunks(&raw);
    state.conflict_path = Some(path.to_string());
    state.conflict_raw = raw;
    let hunk_count = hunks.len();
    state.conflict_choices = vec![None; hunk_count];
    state.conflict_hunks = hunks;
    state.conflict_list.reset(hunk_count);
    state.conflict_nav = None;
    Ok(())
}

pub(crate) fn clear_conflict_selection(state: &mut AppState) {
    state.conflict_path = None;
    state.conflict_raw.clear();
    state.conflict_hunks.clear();
    state.conflict_choices.clear();
    state.conflict_list.reset(0);
    state.conflict_nav = None;
}

pub(crate) fn apply_conflict_resolutions(
    repo: &dyn GitBackend,
    state: &mut AppState,
    choices: Vec<HunkChoice>,
) -> Result<(), GitError> {
    let path = state
        .conflict_path
        .clone()
        .ok_or_else(|| GitError::new("No conflict file selected"))?;
    let raw = state.conflict_raw.clone();
    repo.resolve_conflict_markers(&path, &raw, &choices)?;
    clear_conflict_selection(state);
    reload_conflict_state(repo, state)
}

pub(crate) fn take_conflict_side(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: &str,
    ours: bool,
) -> Result<(), GitError> {
    repo.checkout_side(path, ours)?;
    repo.stage_file(path)?;
    if state.conflict_path.as_deref() == Some(path) {
        clear_conflict_selection(state);
    }
    reload_conflict_state(repo, state)
}

pub(crate) fn reload_shelves(repo: &dyn GitBackend, state: &mut AppState) -> Result<(), GitError> {
    state.shelves = repo.stash_list()?;
    Ok(())
}

/// 文件视图左栏数据：工作区文件清单（升序，仓库相对路径）。
pub(crate) fn load_worktree_files(repo: &dyn GitBackend) -> Result<Vec<String>, GitError> {
    repo.worktree_files()
}

/// 文件视图右栏数据：某个工作区文件的逐行渲染数据 + 空态判定。
pub(crate) struct WorktreeFileData {
    pub(crate) path: String,
    /// 文件为二进制 / 非 UTF-8，无法按文本预览。
    pub(crate) binary: bool,
    /// 该文件在工作区中已删除。
    pub(crate) deleted: bool,
    pub(crate) editor: EditorContent,
}

/// 装载文件视图右栏所需数据。
///
/// blame、diff 失败都不视为错误：未跟踪文件在 HEAD 中不存在、分支尚未诞生
/// （`HEAD` 无法解析）都属正常状态——保留空结果，代码区仍显示文件内容。
/// `blame` 为 false 时跳过 blame 计算（「关闭注解」），行数据不带提交者。
/// 逐行数据在这里（后台线程）一次算好，渲染时不再重复解析 diff / blame。
pub(crate) fn load_worktree_file(
    repo: &dyn GitBackend,
    path: &str,
    blame: bool,
    ignore_whitespace: bool,
) -> WorktreeFileData {
    let (content, read_failed) = match repo.worktree_file_content(path) {
        Ok(text) => (text, false),
        Err(_) => (String::new(), true),
    };
    let blame = if blame {
        repo.blame_worktree(path).unwrap_or_default()
    } else {
        Vec::new()
    };
    let diff = parse_unified_diff(&repo.diff_head(Some(path), ignore_whitespace).unwrap_or_default());
    let binary = read_failed || diff.iter().any(|file| file.is_binary);
    WorktreeFileData {
        path: path.to_string(),
        binary,
        deleted: diff.iter().any(|file| file.is_deleted),
        editor: build_content(&content, &diff, &blame),
    }
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
            let path =
                std::env::temp_dir().join(format!("rebased-rs-uc-{}-{nanos}", std::process::id()));
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

    #[test]
    fn commit_with_autoadd_stages_when_nothing_staged() {
        let repo_dir = TempRepo::new();
        let repo = open_backend(&repo_dir.path).unwrap();
        std::fs::write(repo_dir.path.join("a.txt"), "hello").unwrap();
        commit_with_autoadd(repo.as_ref(), "add file", false).unwrap();
        let status = repo.status().unwrap();
        assert!(status.changes.is_empty());
        let log = repo.log(5).unwrap();
        assert_eq!(log[0].subject, "add file");
    }

    #[test]
    fn sync_repo_state_maps_repo_data() {
        let repo_dir = TempRepo::new();
        git(
            &repo_dir.path,
            &["commit", "--allow-empty", "-m", "initial"],
        );
        let repo = open_backend(&repo_dir.path).unwrap();
        let data = load_repo_data(repo.as_ref(), DEFAULT_LOG_LIMIT).unwrap();
        let mut state = AppState {
            sidebar: SidebarMode::Diff,
            ..AppState::default()
        };
        let (commits, _graph) = sync_repo_state(repo.as_ref(), &mut state, data).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(
            state.head_id.as_deref(),
            commits.first().map(|commit| commit.id.0.as_str())
        );
        assert_eq!(state.current_branch.as_deref(), Some("main"));
        assert_eq!(state.sidebar, SidebarMode::Diff);
        assert_eq!(state.rebase, RebaseFlow::Idle);
        assert!(!state.merge_in_progress);
        assert_eq!(state.repo_root, repo_dir.path.to_string_lossy());
    }

    #[test]
    fn commit_selected_commits_only_chosen_paths() {
        let repo_dir = TempRepo::new();
        git(
            &repo_dir.path,
            &["commit", "--allow-empty", "-m", "initial"],
        );
        let repo = open_backend(&repo_dir.path).unwrap();
        std::fs::write(repo_dir.path.join("a.txt"), "a").unwrap();
        std::fs::write(repo_dir.path.join("b.txt"), "b").unwrap();
        commit_selected(
            repo.as_ref(),
            "partial commit",
            false,
            &["a.txt".to_string()],
        )
        .unwrap();
        let status = repo.status().unwrap();
        assert!(!status.changes.iter().any(|change| change.path == "a.txt"));
        assert!(status.changes.iter().any(|change| change.path == "b.txt"));
        let log = repo.log(5).unwrap();
        assert_eq!(log[0].subject, "partial commit");
    }

    #[test]
    fn conflict_side_resolution_stages_file() {
        let repo_dir = TempRepo::new();
        git(&repo_dir.path, &["commit", "--allow-empty", "-m", "base"]);
        git(&repo_dir.path, &["checkout", "-b", "feature"]);
        std::fs::write(repo_dir.path.join("conflict.txt"), "feature\n").unwrap();
        git(&repo_dir.path, &["add", "conflict.txt"]);
        git(&repo_dir.path, &["commit", "-m", "feature change"]);
        git(&repo_dir.path, &["checkout", "main"]);
        std::fs::write(repo_dir.path.join("conflict.txt"), "main\n").unwrap();
        git(&repo_dir.path, &["add", "conflict.txt"]);
        git(&repo_dir.path, &["commit", "-m", "main change"]);
        let repo = open_backend(&repo_dir.path).unwrap();
        assert!(repo.merge_branch("feature").is_err());
        let mut state = AppState::default();
        reload_conflict_state(repo.as_ref(), &mut state).unwrap();
        assert_eq!(state.conflict_files.len(), 1);
        let path = state.conflict_files[0].path.clone();
        take_conflict_side(repo.as_ref(), &mut state, &path, false).unwrap();
        let status = repo.status().unwrap();
        let change = status
            .changes
            .iter()
            .find(|change| change.path == path)
            .expect("theirs-resolved file must appear as staged");
        assert!(change.staged);
        assert!(repo.conflicted_files().unwrap().is_empty());
    }

    #[test]
    fn staged_and_unstaged_diffs_open() {
        let repo_dir = TempRepo::new();
        git(
            &repo_dir.path,
            &["commit", "--allow-empty", "-m", "initial"],
        );
        let repo = open_backend(&repo_dir.path).unwrap();
        std::fs::write(repo_dir.path.join("a.txt"), "hello").unwrap();
        let mut state = AppState::default();
        open_unstaged_diff(repo.as_ref(), &mut state, Some("a.txt".to_string())).unwrap();
        assert_eq!(state.sidebar, SidebarMode::Diff);
        assert!(state.diff_title.contains("unstaged"));
        assert!(!state.diff_files.is_empty());
        repo.add(&["a.txt"]).unwrap();
        open_staged_diff(repo.as_ref(), &mut state, Some("a.txt".to_string())).unwrap();
        assert_eq!(state.sidebar, SidebarMode::Diff);
        assert!(state.diff_title.contains("staged"));
        assert!(!state.diff_files.is_empty());
    }

    #[test]
    fn hunk_stage_and_unstage_roundtrip() {
        let repo_dir = TempRepo::new();
        git(
            &repo_dir.path,
            &["commit", "--allow-empty", "-m", "initial"],
        );
        let repo = open_backend(&repo_dir.path).unwrap();
        std::fs::write(
            repo_dir.path.join("a.txt"),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        )
        .unwrap();
        git(&repo_dir.path, &["add", "a.txt"]);
        git(&repo_dir.path, &["commit", "-m", "add a.txt"]);
        std::fs::write(
            repo_dir.path.join("a.txt"),
            "CHANGED1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nCHANGED10\n",
        )
        .unwrap();

        // U3 上下文下两处相距较远的改动应拆成两个 hunk
        let stdout = repo.diff_unstaged(Some("a.txt"), false).unwrap();
        let files = parse_unified_diff(&stdout);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].hunks.len(), 2);

        // Stage 第一个 hunk：只有 CHANGED1 进入 index
        repo.apply_hunk_to_index(&files[0], 0).unwrap();
        let staged = parse_unified_diff(&repo.diff_staged(Some("a.txt"), false).unwrap());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].hunks.len(), 1);
        let staged_text: Vec<&str> = staged[0].hunks[0]
            .lines
            .iter()
            .map(|line| line.content.as_str())
            .collect();
        assert!(staged_text.contains(&"CHANGED1"));
        assert!(!staged_text.contains(&"CHANGED10"));

        // Unstage 该 hunk：index 回到 HEAD，staged diff 清空
        repo.revert_hunk_from_index(&staged[0], 0).unwrap();
        let staged_after = repo.diff_staged(Some("a.txt"), false).unwrap();
        assert!(
            parse_unified_diff(&staged_after)
                .iter()
                .all(|file| file.hunks.is_empty()),
            "staged diff should be empty after unstage: {staged_after}"
        );
    }
}
