use std::sync::Arc;

use rebased_rs::git::{
    conflict_hunks, parse_unified_diff, Commit, GitBackend, GitError, Graph, HunkChoice, RepoData,
};
#[cfg(test)]
use rebased_rs::git::{load_repo_data, open_backend, DEFAULT_LOG_LIMIT};

use super::state::{AppState, RebaseFlow, SidebarMode};

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
    } = data;
    state.head_id = commits.first().map(|commit| commit.id.0.clone());
    state.changes = status.changes;
    let current = branches.iter().find(|branch| branch.is_current());
    state.ahead = current.map_or(0, |branch| branch.ahead);
    state.behind = current.map_or(0, |branch| branch.behind);
    let names: Vec<String> = branches
        .iter()
        .filter(|branch| !branch.is_remote)
        .map(|branch| branch.name.clone())
        .collect();
    state.branches = Arc::new(names);
    state.current_branch = current.map(|branch| branch.name.clone());
    state.current_upstream = current.and_then(|branch| branch.upstream.clone());
    state.tags = Arc::new(tags);
    reset_views(state);
    state.merge_in_progress = repo.is_merge_in_progress();
    sync_rebase_flow(repo, state);
    let rebase_in_progress = state.rebase.in_progress();
    if (rebase_in_progress || state.merge_in_progress)
        && let Err(e) = reload_conflict_state(repo, state)
    {
        state.error = Some(e.to_string());
    }
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

fn reset_views(state: &mut AppState) {
    state.selected = None;
    state.detail_files.clear();
    state.detail_branches.clear();
    state.sidebar = SidebarMode::Workspace;
    state.diff_files.clear();
    state.diff_title.clear();
    state.diff_path = None;
    state.blame_groups.clear();
    state.blame_path.clear();
    state.prompt = None;
    state.conflict_files.clear();
    state.conflict_path = None;
    state.conflict_hunks.clear();
    state.conflict_choices.clear();
    state.conflict_raw.clear();
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

pub(crate) fn open_worktree_diff(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: Option<String>,
) -> Result<(), GitError> {
    let stdout = repo.diff_head(path.as_deref())?;
    state.diff_files = parse_unified_diff(&stdout);
    state.diff_title = match &path {
        Some(p) => format!("Diff · {p}"),
        None => "Diff · working tree".to_string(),
    };
    state.diff_path = path;
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
    let stdout = repo.show_diff(&commit_id, path.as_deref())?;
    state.diff_files = parse_unified_diff(&stdout);
    state.diff_title = match &path {
        Some(p) => format!("{short} · {p}"),
        None => format!("Commit {short}"),
    };
    state.diff_path = None;
    state.sidebar = SidebarMode::Diff;
    state.error = None;
    Ok(())
}

pub(crate) fn open_blame(
    repo: &dyn GitBackend,
    state: &mut AppState,
    path: String,
) -> Result<(), GitError> {
    state.blame_groups = repo.blame("HEAD", &path)?;
    state.blame_path = path;
    state.sidebar = SidebarMode::Blame;
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
    state.conflict_choices = vec![None; hunks.len()];
    state.conflict_hunks = hunks;
    Ok(())
}

pub(crate) fn clear_conflict_selection(state: &mut AppState) {
    state.conflict_path = None;
    state.conflict_raw.clear();
    state.conflict_hunks.clear();
    state.conflict_choices.clear();
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
    if state.conflict_path.as_deref() == Some(path) {
        clear_conflict_selection(state);
    }
    reload_conflict_state(repo, state)
}

pub(crate) fn reload_shelves(
    repo: &dyn GitBackend,
    state: &mut AppState,
) -> Result<(), GitError> {
    state.shelves = repo.stash_list()?;
    Ok(())
}

pub(crate) fn load_rebase_plan(
    repo: &dyn GitBackend,
    state: &mut AppState,
    base: &str,
) -> Result<(), GitError> {
    let plan = repo.rebase_todos(base)?;
    state.rebase = RebaseFlow::Planning {
        base: base.to_string(),
        plan,
    };
    state.sidebar = SidebarMode::Rebase;
    state.error = None;
    Ok(())
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
            let path = std::env::temp_dir().join(format!("rebased-rs-uc-{}-{nanos}", std::process::id()));
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
        git(&repo_dir.path, &["commit", "--allow-empty", "-m", "initial"]);
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
        assert_eq!(state.sidebar, SidebarMode::Workspace);
        assert_eq!(state.rebase, RebaseFlow::Idle);
        assert!(!state.merge_in_progress);
    }
}
