use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, size, AnyElement, Bounds, AppContext, ClipboardItem, Context, Div, Entity,
    FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, Render, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window, WindowBounds,
    WindowOptions,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants, DropdownButton},
    input::{Textarea, TextareaState},
    list::{List, ListEvent, ListState},
    menu::PopupMenuItem,
    ActiveTheme, Root,
};
use rebased_rs::git::{
    conflict_hunks, load_repo_data, parse_unified_diff, BlameGroup, Change, ChangeStatus, Commit,
    ConflictFile, ConflictHunk, FileDiff, GitError, HunkChoice, RebaseAction, RepoData,
    Repository, StashEntry, Tag, DEFAULT_LOG_LIMIT,
};

use crate::ui::blame_view::render_blame;
use crate::ui::commit_list::{format_time, LogData, LogDelegate};
use crate::ui::diff_view::render_diff_files;
use crate::ui::graph_view::{lane_color, status_color};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarMode {
    Workspace,
    Detail,
    Diff,
    Blame,
    Rebase,
    Conflicts,
    Shelve,
}

#[derive(Clone)]
enum PromptKind {
    NewBranch { start_point: Option<String> },
    NewTag { commit_id: String },
    Stash,
    Reword { commit_id: String },
    RenameBranch,
}

struct Loaded {
    repo: Arc<Repository>,
    data: RepoData,
}

fn open_and_load(path: &Path) -> Result<Loaded, GitError> {
    let repo = Repository::open(path)?;
    let data = load_repo_data(&repo, DEFAULT_LOG_LIMIT)?;
    Ok(Loaded {
        repo: Arc::new(repo),
        data,
    })
}

pub struct AppView {
    repo_path: PathBuf,
    repo: Option<Arc<Repository>>,
    list: Entity<ListState<LogDelegate>>,
    message_input: Entity<TextareaState>,
    prompt_input: Entity<TextareaState>,
    branches: Arc<Vec<String>>,
    current_branch: Option<String>,
    current_upstream: Option<String>,
    tags: Arc<Vec<Tag>>,
    changes: Vec<Change>,
    ahead: u32,
    behind: u32,
    amend: bool,
    selected: Option<Commit>,
    detail_files: Vec<Change>,
    detail_branches: Vec<String>,
    sidebar: SidebarMode,
    diff_files: Vec<FileDiff>,
    diff_title: String,
    diff_path: Option<String>,
    blame_groups: Vec<BlameGroup>,
    blame_path: String,
    prompt: Option<PromptKind>,
    rebase_plan: Vec<RebaseAction>,
    rebase_base: String,
    rebase_in_progress: bool,
    rebase_stopped: Option<String>,
    merge_in_progress: bool,
    head_id: Option<String>,
    conflict_files: Vec<ConflictFile>,
    conflict_path: Option<String>,
    conflict_hunks: Vec<ConflictHunk>,
    conflict_choices: Vec<Option<HunkChoice>>,
    conflict_raw: String,
    shelves: Vec<StashEntry>,
    status_message: SharedString,
    error: Option<SharedString>,
    loading: bool,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    fn new(repo_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| ListState::new(LogDelegate::new(), window, cx).searchable(true));
        let message_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Commit message")
                .soft_wrap(true)
        });
        let prompt_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Name / message")
                .soft_wrap(false)
        });
        let subscriptions = vec![cx.subscribe_in(&list, window, Self::on_list_event)];

        let mut this = Self {
            repo_path,
            repo: None,
            list,
            message_input,
            prompt_input,
            branches: Arc::new(Vec::new()),
            current_branch: None,
            current_upstream: None,
            tags: Arc::new(Vec::new()),
            changes: Vec::new(),
            ahead: 0,
            behind: 0,
            amend: false,
            selected: None,
            detail_files: Vec::new(),
            detail_branches: Vec::new(),
            sidebar: SidebarMode::Workspace,
            diff_files: Vec::new(),
            diff_title: String::new(),
            diff_path: None,
            blame_groups: Vec::new(),
            blame_path: String::new(),
            prompt: None,
            rebase_plan: Vec::new(),
            rebase_base: String::new(),
            rebase_in_progress: false,
            rebase_stopped: None,
            merge_in_progress: false,
            head_id: None,
            conflict_files: Vec::new(),
            conflict_path: None,
            conflict_hunks: Vec::new(),
            conflict_choices: Vec::new(),
            conflict_raw: String::new(),
            shelves: Vec::new(),
            status_message: SharedString::from("Ready"),
            error: None,
            loading: true,
            _subscriptions: subscriptions,
        };

        match open_and_load(&this.repo_path) {
            Ok(loaded) => {
                this.repo = Some(loaded.repo);
                this.apply_data(loaded.data, cx);
                this.loading = false;
            }
            Err(e) => {
                this.loading = false;
                this.error = Some(e.to_string().into());
            }
        }
        this
    }

    fn apply_data(&mut self, data: RepoData, cx: &mut Context<Self>) {
        let RepoData {
            commits,
            graph,
            status,
            branches,
            tags,
        } = data;
        self.head_id = commits.first().map(|commit| commit.id.0.clone());
        self.changes = status.changes;
        let current = branches.iter().find(|branch| branch.is_current());
        self.ahead = current.map_or(0, |branch| branch.ahead);
        self.behind = current.map_or(0, |branch| branch.behind);
        let names: Vec<String> = branches
            .iter()
            .filter(|branch| !branch.is_remote)
            .map(|branch| branch.name.clone())
            .collect();
        self.branches = Arc::new(names);
        self.current_branch = current.map(|branch| branch.name.clone());
        self.current_upstream = current.and_then(|branch| branch.upstream.clone());
        self.tags = Arc::new(tags);
        self.selected = None;
        self.detail_files.clear();
        self.detail_branches.clear();
        self.sidebar = SidebarMode::Workspace;
        self.diff_files.clear();
        self.diff_title.clear();
        self.diff_path = None;
        self.blame_groups.clear();
        self.blame_path.clear();
        self.prompt = None;
        self.rebase_plan.clear();
        self.rebase_base.clear();
        self.conflict_files.clear();
        self.conflict_path = None;
        self.conflict_hunks.clear();
        self.conflict_choices.clear();
        self.conflict_raw.clear();
        self.rebase_in_progress = self
            .repo
            .as_ref()
            .is_some_and(|repo| repo.is_rebase_in_progress());
        self.merge_in_progress = self
            .repo
            .as_ref()
            .is_some_and(|repo| repo.is_merge_in_progress());
        self.rebase_stopped = if self.rebase_in_progress {
            self.repo
                .as_ref()
                .and_then(|repo| repo.rebase_stopped_commit())
        } else {
            None
        };
        if self.rebase_in_progress || self.merge_in_progress {
            self.reload_conflict_state(cx);
        }
        self.list.update(cx, |list, cx| {
            list.delegate_mut().set_data(LogData { commits, graph });
            cx.notify();
        });
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match load_repo_data(&repo, DEFAULT_LOG_LIMIT) {
            Ok(data) => self.apply_data(data, cx),
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn run_op(
        &mut self,
        message: &str,
        op: impl FnOnce(&Repository) -> Result<(), GitError>,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match op(&repo) {
            Ok(()) => {
                self.error = None;
                self.status_message = message.into();
                self.refresh(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn toggle_stage(&mut self, change: &Change, cx: &mut Context<Self>) {
        let path = change.path.clone();
        if change.staged {
            self.run_op("Unstaged", move |repo| repo.reset(&[path.as_str()]), cx);
        } else {
            self.run_op("Staged", move |repo| repo.add(&[path.as_str()]), cx);
        }
    }

    fn do_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message_input.read(cx).value().to_string();
        if message.trim().is_empty() {
            self.error = Some("Commit message is empty".into());
            cx.notify();
            return;
        }
        let amend = self.amend;
        self.message_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.amend = false;
        self.run_op(
            "Committed",
            move |repo| {
                if !amend {
                    let status = repo.status()?;
                    if status.changes.iter().all(|change| !change.staged) {
                        repo.add_all()?;
                    }
                }
                repo.commit(&message, amend)
            },
            cx,
        );
    }

    fn do_push(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.current_branch.clone() else {
            self.error = Some("No current branch".into());
            cx.notify();
            return;
        };
        let set_upstream = self.current_upstream.is_none();
        self.run_op("Pushed", move |repo| repo.push(&branch, set_upstream), cx);
    }

    fn do_pull(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.current_branch.clone() else {
            self.error = Some("No current branch".into());
            cx.notify();
            return;
        };
        self.run_op("Pulled", move |repo| repo.pull(&branch), cx);
    }

    fn checkout_branch(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Checked out {name}");
        self.run_op(&message, move |repo| repo.checkout(&name), cx);
    }

    fn load_commit_detail(&mut self, commit: Commit, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let id = commit.id.0.clone();
        let files = repo.show_files(&id);
        let branches = repo.branches_containing(&id);
        match (files, branches) {
            (Ok(files), Ok(branches)) => {
                self.selected = Some(commit);
                self.detail_files = files;
                self.detail_branches = branches;
                self.sidebar = SidebarMode::Detail;
                self.error = None;
            }
            (Err(e), _) | (_, Err(e)) => {
                self.error = Some(e.to_string().into());
            }
        }
        cx.notify();
    }

    fn clear_detail(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.detail_files.clear();
        self.detail_branches.clear();
        self.sidebar = SidebarMode::Workspace;
        cx.notify();
    }

    fn on_list_event(
        &mut self,
        _entity: &Entity<ListState<LogDelegate>>,
        event: &ListEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ListEvent::Select(ix) | ListEvent::Confirm(ix) => {
                let commit = self.list.read(cx).delegate().commit_at(ix.row);
                if let Some(commit) = commit {
                    let is_same = self
                        .selected
                        .as_ref()
                        .is_some_and(|current| current.id.0 == commit.id.0);
                    if !is_same {
                        self.load_commit_detail(commit, cx);
                    }
                }
            }
            ListEvent::Cancel => self.clear_detail(cx),
        }
    }

    fn open_diff_worktree(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.diff_head(path.as_deref()) {
            Ok(stdout) => {
                self.diff_files = parse_unified_diff(&stdout);
                self.diff_title = match &path {
                    Some(p) => format!("Diff · {p}"),
                    None => "Diff · working tree".to_string(),
                };
                self.diff_path = path;
                self.sidebar = SidebarMode::Diff;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string().into()),
        }
        cx.notify();
    }

    fn open_commit_diff(
        &mut self,
        commit_id: String,
        path: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let short = &commit_id[..commit_id.len().min(7)];
        match repo.show_diff(&commit_id, path.as_deref()) {
            Ok(stdout) => {
                self.diff_files = parse_unified_diff(&stdout);
                self.diff_title = match &path {
                    Some(p) => format!("{short} · {p}"),
                    None => format!("Commit {short}"),
                };
                self.diff_path = None;
                self.sidebar = SidebarMode::Diff;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string().into()),
        }
        cx.notify();
    }

    fn open_blame(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.blame("HEAD", &path) {
            Ok(groups) => {
                self.blame_groups = groups;
                self.blame_path = path;
                self.sidebar = SidebarMode::Blame;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string().into()),
        }
        cx.notify();
    }

    fn sidebar_back(&mut self, cx: &mut Context<Self>) {
        self.sidebar = if self.selected.is_some() {
            SidebarMode::Detail
        } else {
            SidebarMode::Workspace
        };
        cx.notify();
    }

    fn open_prompt(&mut self, kind: PromptKind, cx: &mut Context<Self>) {
        self.prompt = Some(kind);
        cx.notify();
    }

    fn cancel_prompt(&mut self, cx: &mut Context<Self>) {
        self.prompt = None;
        cx.notify();
    }

    fn confirm_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.prompt.clone() else {
            return;
        };
        let input = self.prompt_input.read(cx).value().trim().to_string();
        self.prompt_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.prompt = None;
        match kind {
            PromptKind::NewBranch { start_point } => {
                if input.is_empty() {
                    self.error = Some("Branch name is empty".into());
                } else {
                    let message = format!("Created branch {input}");
                    self.run_op(
                        &message,
                        move |repo| {
                            repo.create_branch(&input, start_point.as_deref())?;
                            repo.checkout(&input)
                        },
                        cx,
                    );
                }
            }
            PromptKind::NewTag { commit_id } => {
                if input.is_empty() {
                    self.error = Some("Tag name is empty".into());
                } else {
                    let message = format!("Created tag {input}");
                    self.run_op(
                        &message,
                        move |repo| repo.create_tag(&input, Some(&commit_id), None),
                        cx,
                    );
                }
            }
            PromptKind::Stash => {
                let message = if input.is_empty() { None } else { Some(input) };
                self.run_op(
                    "Stashed",
                    move |repo| repo.stash_push(message.as_deref(), true),
                    cx,
                );
            }
            PromptKind::Reword { commit_id } => {
                if input.is_empty() {
                    self.error = Some("Commit message is empty".into());
                } else {
                    let message = format!("Reworded {}", &commit_id[..commit_id.len().min(7)]);
                    self.run_op(
                        &message,
                        move |repo| repo.reword_commit(&commit_id, &input),
                        cx,
                    );
                }
            }
            PromptKind::RenameBranch => {
                let Some(old) = self.current_branch.clone() else {
                    self.error = Some("No current branch".into());
                    cx.notify();
                    return;
                };
                if input.is_empty() {
                    self.error = Some("Branch name is empty".into());
                } else {
                    let message = format!("Renamed {old} to {input}");
                    self.run_op(&message, move |repo| repo.rename_branch(&old, &input), cx);
                }
            }
        }
        cx.notify();
    }

    fn select_commit_by_id(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.list.read(cx).delegate().find_commit(id) {
            Some(commit) => self.load_commit_detail(commit, cx),
            None => {
                self.error = Some(format!("Commit {id} not in loaded history").into());
                cx.notify();
            }
        }
    }

    fn cherry_pick_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.selected.clone() else {
            return;
        };
        let id = commit.id.0.clone();
        let message = format!("Cherry-picked {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.cherry_pick(&id), cx);
    }

    fn revert_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.selected.clone() else {
            return;
        };
        let id = commit.id.0.clone();
        let message = format!("Reverted {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.revert(&id), cx);
    }

    fn start_rebase(&mut self, base: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.rebase_todos(&base) {
            Ok(plan) => {
                self.rebase_base = base;
                self.rebase_plan = plan;
                self.sidebar = SidebarMode::Rebase;
                self.error = None;
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn cycle_rebase_action(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(action) = self.rebase_plan.get_mut(index) {
            action.kind = action.kind.next();
            cx.notify();
        }
    }

    fn move_rebase_action(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        let target = index as isize + delta;
        if target < 0 || target >= self.rebase_plan.len() as isize {
            return;
        }
        self.rebase_plan.swap(index, target as usize);
        cx.notify();
    }

    fn apply_rebase(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let base = self.rebase_base.clone();
        let plan = self.rebase_plan.clone();
        if plan.is_empty() {
            return;
        }
        match repo.rebase_run(&base, &plan) {
            Ok(()) => {
                self.error = None;
                self.status_message =
                    format!("Rebased onto {}", &base[..base.len().min(7)]).into();
                self.rebase_plan.clear();
                self.rebase_base.clear();
                self.refresh(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                self.rebase_in_progress = repo.is_rebase_in_progress();
                if self.rebase_in_progress {
                    self.sidebar = SidebarMode::Workspace;
                }
                cx.notify();
            }
        }
    }

    fn cancel_rebase(&mut self, cx: &mut Context<Self>) {
        self.rebase_plan.clear();
        self.rebase_base.clear();
        self.sidebar = SidebarMode::Workspace;
        cx.notify();
    }

    fn abort_rebase(&mut self, cx: &mut Context<Self>) {
        self.run_op("Rebase aborted", |repo| repo.rebase_abort(), cx);
    }

    fn continue_rebase(&mut self, cx: &mut Context<Self>) {
        self.run_op("Rebase continued", |repo| repo.rebase_continue(), cx);
    }

    fn open_conflicts(&mut self, cx: &mut Context<Self>) {
        self.sidebar = SidebarMode::Conflicts;
        self.reload_conflict_state(cx);
    }

    fn reload_conflict_state(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.conflicted_files() {
            Ok(files) => {
                self.conflict_files = files;
                if !self.conflict_files.is_empty() {
                    let first = self.conflict_files[0].path.clone();
                    self.sidebar = SidebarMode::Conflicts;
                    self.select_conflict_file(&first, cx);
                }
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn select_conflict_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.conflict_file_content(path) {
            Ok(raw) => {
                let hunks = conflict_hunks(&raw);
                self.conflict_path = Some(path.to_string());
                self.conflict_raw = raw;
                self.conflict_choices = vec![None; hunks.len()];
                self.conflict_hunks = hunks;
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn choose_hunk(&mut self, index: usize, choice: HunkChoice, cx: &mut Context<Self>) {
        if let Some(slot) = self.conflict_choices.get_mut(index) {
            *slot = Some(choice);
            cx.notify();
        }
    }

    fn apply_conflict_resolutions(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let Some(path) = self.conflict_path.clone() else {
            return;
        };
        if self.conflict_choices.iter().any(|choice| choice.is_none()) {
            self.error = Some("Resolve every hunk before applying".into());
            cx.notify();
            return;
        }
        let choices: Vec<HunkChoice> = self
            .conflict_choices
            .iter()
            .flatten()
            .copied()
            .collect();
        let raw = self.conflict_raw.clone();
        match repo.resolve_conflict_markers(&path, &raw, &choices) {
            Ok(()) => {
                self.error = None;
                self.status_message = format!("Resolved {}", path).into();
                self.conflict_path = None;
                self.conflict_raw.clear();
                self.conflict_hunks.clear();
                self.conflict_choices.clear();
                self.reload_conflict_state(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn take_conflict_side(&mut self, path: String, ours: bool, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.checkout_side(&path, ours) {
            Ok(()) => {
                self.error = None;
                let side = if ours { "ours" } else { "theirs" };
                self.status_message = format!("Took {side} for {path}").into();
                if self.conflict_path.as_deref() == Some(path.as_str()) {
                    self.conflict_path = None;
                    self.conflict_raw.clear();
                    self.conflict_hunks.clear();
                    self.conflict_choices.clear();
                }
                self.reload_conflict_state(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn open_shelves(&mut self, cx: &mut Context<Self>) {
        self.sidebar = SidebarMode::Shelve;
        self.reload_shelves(cx);
    }

    fn reload_shelves(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.stash_list() {
            Ok(entries) => {
                self.shelves = entries;
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn unshelve_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.stash_apply_at(index) {
            Ok(()) => {
                self.error = None;
                self.status_message = "Unshelved".into();
                self.refresh(cx);
                self.sidebar = SidebarMode::Shelve;
                self.reload_shelves(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn drop_shelve_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.stash_drop_at(index) {
            Ok(()) => {
                self.error = None;
                self.status_message = "Dropped shelve".into();
                self.reload_shelves(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    fn delete_branch(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Deleted branch {name}");
        self.run_op(&message, move |repo| repo.delete_branch(&name, false), cx);
    }

    fn delete_tag(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Deleted tag {name}");
        self.run_op(&message, move |repo| repo.delete_tag(&name), cx);
    }

    fn merge_branch_into_current(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.merge_branch(&name) {
            Ok(()) => {
                self.error = None;
                self.status_message = format!("Merged {name}").into();
                self.refresh(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                self.refresh(cx);
            }
        }
    }

    fn abort_merge(&mut self, cx: &mut Context<Self>) {
        self.run_op("Merge aborted", |repo| repo.merge_abort(), cx);
    }

    fn continue_merge_op(&mut self, cx: &mut Context<Self>) {
        self.run_op("Merge continued", |repo| repo.merge_continue(), cx);
    }

    fn undo_head(&mut self, cx: &mut Context<Self>) {
        self.run_op(
            "Undid HEAD commit (changes kept staged)",
            |repo| repo.undo_head_commit(),
            cx,
        );
    }

    fn drop_head(&mut self, cx: &mut Context<Self>) {
        self.run_op("Dropped HEAD commit", |repo| repo.drop_head_commit(), cx);
    }

    fn force_push_current(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.current_branch.clone() else {
            self.error = Some("No current branch".into());
            cx.notify();
            return;
        };
        self.run_op(
            "Force pushed (with lease)",
            move |repo| repo.push_force(&branch),
            cx,
        );
    }

    fn push_all_tags(&mut self, cx: &mut Context<Self>) {
        self.run_op("Pushed all tags", |repo| repo.push_tags(), cx);
    }

    fn copy_commit_sha(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.selected.clone() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(commit.id.0));
        self.error = None;
        self.status_message = "Commit SHA copied".into();
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let branches = self.branches.clone();
        let tags = self.tags.clone();
        let current = self.current_branch.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let tag_weak = weak.clone();
        let branch_label = current
            .clone()
            .unwrap_or_else(|| "main".to_string());

        div()
            .h(px(44.))
            .flex_none()
            .border_b_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .child(
                DropdownButton::new("branch-menu")
                    .button(Button::new("branch-button").ghost().label(branch_label))
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu;
                        for name in branches.iter() {
                            let label = if Some(name.as_str()) == current.as_deref() {
                                format!("● {name}")
                            } else {
                                name.clone()
                            };
                            let weak = weak.clone();
                            let name = name.clone();
                            result = result.item(PopupMenuItem::new(label).on_click(
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.checkout_branch(&name, cx)
                                    });
                                },
                            ));
                        }
                        result = result.separator();
                        result = result.item(PopupMenuItem::new("＋ New branch…").on_click({
                            let weak = weak.clone();
                            move |_, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.open_prompt(PromptKind::NewBranch { start_point: None }, cx)
                                });
                            }
                        }));
                        result = result.item(
                            PopupMenuItem::new("✎ Rename current branch…").on_click({
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::RenameBranch, cx)
                                    });
                                }
                            }),
                        );
                        result = result.item(
                            PopupMenuItem::new("⇪ Force push (with lease)").on_click({
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.force_push_current(cx)
                                    });
                                }
                            }),
                        );
                        for name in branches.iter() {
                            if Some(name.as_str()) == current.as_deref() {
                                continue;
                            }
                            let weak = weak.clone();
                            let name = name.clone();
                            let merge_label = format!(
                                "⇄ Merge {name} into {}",
                                current.clone().unwrap_or_else(|| "HEAD".to_string())
                            );
                            result = result.item(PopupMenuItem::new(merge_label).on_click({
                                let weak = weak.clone();
                                let name = name.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.merge_branch_into_current(name.clone(), cx)
                                    });
                                }
                            }));
                            result = result.item(PopupMenuItem::new(format!("✕ {name}")).on_click(
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.delete_branch(&name, cx)
                                    });
                                },
                            ));
                        }
                        result
                    }),
            )
            .child(
                DropdownButton::new("tag-menu")
                    .button(Button::new("tag-button").ghost().label("Tags"))
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(PopupMenuItem::new("＋ New tag on HEAD…").on_click({
                            let weak = tag_weak.clone();
                            move |_, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.open_prompt(
                                        PromptKind::NewTag { commit_id: "HEAD".to_string() },
                                        cx,
                                    )
                                });
                            }
                        }));
                        result = result.item(
                            PopupMenuItem::new("⇪ Push all tags to origin").on_click({
                                let weak = tag_weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| this.push_all_tags(cx));
                                }
                            }),
                        );
                        if !tags.is_empty() {
                            result = result.separator();
                            for tag in tags.iter() {
                                let weak = tag_weak.clone();
                                let commit_id = tag.commit_id.clone();
                                let label = tag.name.clone();
                                result = result.item(PopupMenuItem::new(label).on_click(
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.select_commit_by_id(&commit_id, cx)
                                        });
                                    },
                                ));
                            }
                        }
                        result
                    }),
            )
            .child(
                Button::new("fetch")
                    .ghost()
                    .label("Fetch")
                    .on_click(cx.listener(|this, _, _, cx| this.run_op("Fetched", |repo| repo.fetch(), cx))),
            )
            .child(
                Button::new("pull")
                    .ghost()
                    .label("Pull")
                    .on_click(cx.listener(|this, _, _, cx| this.do_pull(cx))),
            )
            .child(
                Button::new("push")
                    .ghost()
                    .label("Push")
                    .on_click(cx.listener(|this, _, _, cx| this.do_push(cx))),
            )
            .child(
                Button::new("stash")
                    .ghost()
                    .label("Stash")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_prompt(PromptKind::Stash, cx)
                    })),
            )
            .child(
                Button::new("unstash")
                    .ghost()
                    .label("Unstash")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_op("Unstashed", |repo| repo.stash_pop(), cx)
                    })),
            )
            .child(
                Button::new("refresh")
                    .ghost()
                    .label("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(
                Button::new("conflicts")
                    .ghost()
                    .label("Conflicts")
                    .on_click(cx.listener(|this, _, _, cx| this.open_conflicts(cx))),
            )
            .child(
                Button::new("shelves")
                    .ghost()
                    .label("Shelves")
                    .on_click(cx.listener(|this, _, _, cx| this.open_shelves(cx))),
            )
            .when(self.rebase_in_progress, |bar| {
                bar.child(
                    Button::new("rebase-abort")
                        .danger()
                        .compact()
                        .label("Abort Rebase")
                        .on_click(cx.listener(|this, _, _, cx| this.abort_rebase(cx))),
                )
                .child(
                    Button::new("rebase-continue")
                        .danger()
                        .compact()
                        .label("Continue Rebase")
                        .on_click(cx.listener(|this, _, _, cx| this.continue_rebase(cx))),
                )
            })
            .when(self.merge_in_progress, |bar| {
                bar.child(
                    Button::new("merge-abort")
                        .danger()
                        .compact()
                        .label("Abort Merge")
                        .on_click(cx.listener(|this, _, _, cx| this.abort_merge(cx))),
                )
                .child(
                    Button::new("merge-continue")
                        .danger()
                        .compact()
                        .label("Continue Merge")
                        .on_click(cx.listener(|this, _, _, cx| this.continue_merge_op(cx))),
                )
            })
    }

    fn render_commit_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.loading {
            return div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Loading repository...")
                .into_any_element();
        }
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(List::new(&self.list))
            .into_any_element()
    }

    fn render_change_row(&self, index: usize, change: &Change, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let color = status_color(&change.status);
        let path = change.display_path();
        let staged = change.staged;
        let is_untracked = change.status == ChangeStatus::Untracked;
        let change_for_click = change.clone();
        let diff_path = change.path.clone();

        let mut row = div()
            .id(format!("change-{index}"))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(move |style| style.bg(hsla(fg.h, fg.s, fg.l, 0.07)))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_stage(&change_for_click, cx)))
            .child(
                div()
                    .w(px(14.))
                    .flex_none()
                    .text_xs()
                    .text_color(color)
                    .child(change.status.short_label()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_sm()
                    .child(path),
            );

        row = row.child(
            Button::new(format!("chg-diff-{index}"))
                .ghost()
                .compact()
                .label("Δ")
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let path = diff_path.clone();
                    this.open_diff_worktree(Some(path), cx);
                })),
        );
        if !staged && !is_untracked {
            let discard_path = change.path.clone();
            row = row.child(
                Button::new(format!("chg-discard-{index}"))
                    .ghost()
                    .compact()
                    .label("↩")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        let path = discard_path.clone();
                        let message = format!("Discarded {path}");
                        this.run_op(&message, move |repo| repo.discard_changes(&path), cx);
                    })),
            );
        }
        if is_untracked {
            let remove_path = change.path.clone();
            row = row.child(
                Button::new(format!("chg-remove-{index}"))
                    .ghost()
                    .compact()
                    .label("✕")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        let path = remove_path.clone();
                        let message = format!("Removed {path}");
                        this.run_op(&message, move |repo| repo.remove_untracked(&path), cx);
                    })),
            );
        }

        row.child(
            div()
                .flex_none()
                .w(px(12.))
                .text_sm()
                .text_color(if staged { cx.theme().muted_foreground } else { lane_color(2) })
                .child(if staged { "−" } else { "+" }),
        )
        .into_any_element()
    }

    fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let fg = cx.theme().foreground;
        let count = self.changes.len();

        let rows: Vec<AnyElement> = self
            .changes
            .iter()
            .enumerate()
            .map(|(index, change)| self.render_change_row(index, change, cx))
            .collect();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Changes ({count})")),
            )
            .child(
                div()
                    .id("changes-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .px_1()
                    .children(rows)
                    .when(count == 0, |container| {
                        container.child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .text_color(fg.opacity(0.4))
                                .child("Working tree clean"),
                        )
                    }),
            )
    }

    fn render_detail_file_row(
        &self,
        index: usize,
        change: &Change,
        commit_id: &str,
        fg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = status_color(&change.status);
        let path = change.display_path();
        let diff_path = change.path.clone();
        let blame_path = change.path.clone();
        let file_commit_id = commit_id.to_string();

        div()
            .id(format!("detail-file-{index}"))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_0p5()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(move |style| style.bg(hsla(fg.h, fg.s, fg.l, 0.07)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_commit_diff(file_commit_id.clone(), Some(diff_path.clone()), cx)
            }))
            .child(
                div()
                    .w(px(14.))
                    .flex_none()
                    .text_xs()
                    .text_color(color)
                    .child(change.status.short_label()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_xs()
                    .child(path),
            )
            .child(
                Button::new(format!("detail-blame-{index}"))
                    .ghost()
                    .compact()
                    .label("B")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_blame(blame_path.clone(), cx);
                    })),
            )
            .into_any_element()
    }

    fn render_detail(&self, commit: &Commit, cx: &mut Context<Self>) -> Div {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let commit_id = commit.id.0.clone();
        let tag_color = lane_color(5);
        let is_head = self
            .head_id
            .as_ref()
            .is_some_and(|head| head == &commit_id);

        let commit_tags: Vec<Tag> = self
            .tags
            .iter()
            .filter(|tag| tag.commit_id == commit_id)
            .cloned()
            .collect();

        let file_rows: Vec<AnyElement> = self
            .detail_files
            .iter()
            .enumerate()
            .map(|(index, change)| {
                self.render_detail_file_row(index, change, &commit_id, fg, cx)
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(fg)
                            .child(commit.subject.clone()),
                    )
                    .child(
                        Button::new("close-detail")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.clear_detail(cx))),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(muted)
                    .child(format!(
                        "{} · {} · {}",
                        &commit.id.0[..commit.id.0.len().min(7)],
                        commit.author.name,
                        format_time(commit.time)
                    )),
            )
            .when(!commit.body.is_empty(), |detail| {
                detail.child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(muted)
                        .whitespace_normal()
                        .child(commit.body.clone()),
                )
            })
            .when(!self.detail_branches.is_empty(), |detail| {
                detail.child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(tag_color)
                        .child(format!("∟ {}", self.detail_branches.join(", "))),
                )
            })
            .when(!commit_tags.is_empty(), |detail| {
                detail.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_1()
                        .flex_none()
                        .child(div().flex_none().text_xs().text_color(muted).child("Tags"))
                        .children(commit_tags.into_iter().map(|tag| {
                            let name = tag.name;
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_0p5()
                                .rounded(px(4.))
                                .px_1p5()
                                .py_0p5()
                                .bg(hsla(tag_color.h, tag_color.s, tag_color.l, 0.15))
                                .child(
                                    div().text_xs().text_color(tag_color).child(name.clone()),
                                )
                                .child(
                                    Button::new(format!("delete-tag-{name}"))
                                        .ghost()
                                        .compact()
                                        .label("✕")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.delete_tag(&name, cx);
                                        })),
                                )
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .flex_none()
                    .child(
                        Button::new("detail-cherry-pick")
                            .ghost()
                            .compact()
                            .label("Cherry-pick")
                            .on_click(cx.listener(|this, _, _, cx| this.cherry_pick_selected(cx))),
                    )
                    .child(
                        Button::new("detail-revert")
                            .ghost()
                            .compact()
                            .label("Revert")
                            .on_click(cx.listener(|this, _, _, cx| this.revert_selected(cx))),
                    )
                    .child(
                        Button::new("detail-rebase")
                            .ghost()
                            .compact()
                            .label("Rebase from here")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let base = this
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.start_rebase(base, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-reword")
                            .ghost()
                            .compact()
                            .label("Reword…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let commit_id = this
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_prompt(PromptKind::Reword { commit_id }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-diff")
                            .ghost()
                            .compact()
                            .label("Diff")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let id = this
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_commit_diff(id, None, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-branch")
                            .ghost()
                            .compact()
                            .label("Branch…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let start_point = this.selected.as_ref().map(|c| c.id.0.clone());
                                this.open_prompt(PromptKind::NewBranch { start_point }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-tag")
                            .ghost()
                            .compact()
                            .label("Tag…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let commit_id = this
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_prompt(PromptKind::NewTag { commit_id }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-checkout")
                            .ghost()
                            .compact()
                            .label("Checkout")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let Some(commit) = this.selected.clone() else {
                                    return;
                                };
                                let id = commit.id.0.clone();
                                let short = &id[..id.len().min(7)];
                                let message = format!("Checked out {short}");
                                this.run_op(&message, move |repo| repo.checkout(&id), cx);
                            })),
                    )
                    .child(
                        Button::new("detail-copy-sha")
                            .ghost()
                            .compact()
                            .label("⧉ Copy SHA")
                            .on_click(cx.listener(|this, _, _, cx| this.copy_commit_sha(cx))),
                    )
                    .when(is_head, |row| {
                        row.child(
                            Button::new("detail-undo-commit")
                                .ghost()
                                .compact()
                                .label("↶ Undo Commit")
                                .on_click(cx.listener(|this, _, _, cx| this.undo_head(cx))),
                        )
                        .child(
                            Button::new("detail-drop-commit")
                                .danger()
                                .compact()
                                .label("✕ Drop Commit")
                                .on_click(cx.listener(|this, _, _, cx| this.drop_head(cx))),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("Files ({})", self.detail_files.len())),
            )
            .child(
                div()
                    .id("detail-files")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(file_rows),
            )
    }

    fn render_rebase_panel(&self, cx: &mut Context<Self>) -> Div {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let short_base = self.rebase_base[..self.rebase_base.len().min(7)].to_string();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Interactive Rebase"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("onto {short_base}…")),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Click the action to cycle Pick → Squash → Fixup → Drop. Use ↑ ↓ to reorder."),
            );

        if self.rebase_plan.is_empty() {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("No commits between base and HEAD."),
            );
        }

        for (index, action) in self.rebase_plan.iter().enumerate() {
            let kind_label = action.kind.label();
            let summary = format!(
                "{} {}",
                &action.id[..action.id.len().min(7)],
                action.subject
            );
            panel = panel.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(border)
                    .pb_1()
                    .child(
                        Button::new(("rebase-kind", index))
                            .ghost()
                            .compact()
                            .label(kind_label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cycle_rebase_action(index, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_ellipsis()
                            .overflow_hidden()
                            .child(summary),
                    )
                    .child(
                        Button::new(("rebase-up", index))
                            .ghost()
                            .compact()
                            .label("↑")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_rebase_action(index, -1, cx)
                            })),
                    )
                    .child(
                        Button::new(("rebase-down", index))
                            .ghost()
                            .compact()
                            .label("↓")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_rebase_action(index, 1, cx)
                            })),
                    ),
            );
        }

        panel.child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .mt_1()
                .child(
                    Button::new("rebase-start")
                        .primary()
                        .compact()
                        .label("Start Rebase")
                        .on_click(cx.listener(|this, _, _, cx| this.apply_rebase(cx))),
                )
                .child(
                    Button::new("rebase-cancel")
                        .ghost()
                        .compact()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_rebase(cx))),
                ),
        )
    }

    fn render_conflicts_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let mut panel = div()
            .id("conflicts-panel")
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Conflicts"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(if self.merge_in_progress {
                                "Merge is paused. Resolve conflicts, then continue the merge."
                            } else {
                                "Rebase is paused. Resolve conflicts, then continue the rebase."
                            }),
                    ),
            );

        if self.conflict_files.is_empty() {
            let message = if let Some(sha) = &self.rebase_stopped {
                format!(
                    "Rebase stopped for editing at {}… Make changes, amend or commit, then click Continue Rebase.",
                    &sha[..sha.len().min(7)]
                )
            } else {
                "No conflicted files.".to_string()
            };
            panel = panel.child(div().text_xs().text_color(muted).child(message));
        }

        for (index, file) in self.conflict_files.iter().enumerate() {
            let selected = self.conflict_path.as_deref() == Some(file.path.as_str());
            let label = if selected {
                format!("● {}", file.path)
            } else {
                file.path.clone()
            };
            panel = panel.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("conflict-file", index))
                            .ghost()
                            .compact()
                            .label(label)
                            .on_click(cx.listener({
                                let path = file.path.clone();
                                move |this, _, _, cx| this.select_conflict_file(&path, cx)
                            })),
                    )
                    .child(
                        Button::new(("take-ours", index))
                            .ghost()
                            .compact()
                            .label("Take ours")
                            .on_click(cx.listener({
                                let path = file.path.clone();
                                move |this, _, _, cx| {
                                    this.take_conflict_side(path.clone(), true, cx)
                                }
                            })),
                    )
                    .child(
                        Button::new(("take-theirs", index))
                            .ghost()
                            .compact()
                            .label("Take theirs")
                            .on_click(cx.listener({
                                let path = file.path.clone();
                                move |this, _, _, cx| {
                                    this.take_conflict_side(path.clone(), false, cx)
                                }
                            })),
                    ),
            );
        }

        if let Some(path) = self.conflict_path.clone() {
            if self.conflict_hunks.is_empty() {
                panel = panel.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(
                            "No textual hunks in this file. Use the buttons above to take one side.",
                        ),
                );
            }
            for (index, hunk) in self.conflict_hunks.iter().enumerate() {
                let ours_text = if hunk.ours.is_empty() {
                    "(empty)".to_string()
                } else {
                    hunk.ours.join("\n")
                };
                let theirs_text = if hunk.theirs.is_empty() {
                    "(empty)".to_string()
                } else {
                    hunk.theirs.join("\n")
                };
                let ours_label = match self.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => "✓ Use ours".to_string(),
                    _ => "Use ours".to_string(),
                };
                let theirs_label = match self.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Theirs)) => "✓ Use theirs".to_string(),
                    _ => "Use theirs".to_string(),
                };
                let both_label = match self.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Both)) => "✓ Use both".to_string(),
                    _ => "Use both".to_string(),
                };
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_1()
                        .border_color(border)
                        .rounded(px(4.))
                        .p_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("Hunk {} in {}", index + 1, path)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1()
                                .child(
                                    Button::new(("hunk-ours", index))
                                        .ghost()
                                        .compact()
                                        .label(ours_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Ours, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("hunk-theirs", index))
                                        .ghost()
                                        .compact()
                                        .label(theirs_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Theirs, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("hunk-both", index))
                                        .ghost()
                                        .compact()
                                        .label(both_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Both, cx)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("Ours:\n{}", ours_text)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(format!("Theirs:\n{}", theirs_text)),
                        ),
                );
            }
            if !self.conflict_hunks.is_empty() {
                panel = panel.child(
                    Button::new("conflict-apply")
                        .primary()
                        .compact()
                        .label("Apply Resolutions")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.apply_conflict_resolutions(cx)
                        })),
                );
            }
        }

        panel
    }

    fn render_shelve_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let muted = cx.theme().muted_foreground;
        let mut panel = div()
            .id("shelve-panel")
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Shelves"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Stashed workspaces. Unshelve to bring changes back."),
                    ),
            );

        if self.shelves.is_empty() {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(
                        "Nothing on the shelf. Use Shelve in the commit composer.",
                    ),
            );
        }

        for entry in &self.shelves {
            let index = entry.index;
            panel = panel.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_ellipsis()
                            .overflow_hidden()
                            .child(entry.message.clone()),
                    )
                    .child(
                        Button::new(("shelve-apply", index))
                            .ghost()
                            .compact()
                            .label("Unshelve")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.unshelve_at(index, cx)
                            })),
                    )
                    .child(
                        Button::new(("shelve-drop", index))
                            .ghost()
                            .compact()
                            .label("Drop")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.drop_shelve_at(index, cx)
                            })),
                    ),
            );
        }

        panel.child(
            Button::new("shelve-reload")
                .ghost()
                .compact()
                .label("Reload")
                .on_click(cx.listener(|this, _, _, cx| this.reload_shelves(cx))),
        )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let width = match self.sidebar {
            SidebarMode::Workspace => 360.,
            SidebarMode::Detail => 420.,
            SidebarMode::Diff | SidebarMode::Blame => 680.,
            SidebarMode::Rebase => 480.,
            SidebarMode::Conflicts => 680.,
            SidebarMode::Shelve => 420.,
        };
        let base = div()
            .w(px(width))
            .flex_none()
            .border_l_1()
            .border_color(border)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .overflow_hidden();

        match self.sidebar {
            SidebarMode::Diff => base.child(self.render_diff_panel(cx)).into_any_element(),
            SidebarMode::Blame => base.child(self.render_blame_panel(cx)).into_any_element(),
            SidebarMode::Rebase => base.child(self.render_rebase_panel(cx)).into_any_element(),
            SidebarMode::Conflicts => {
                base.child(self.render_conflicts_panel(cx)).into_any_element()
            }
            SidebarMode::Shelve => base.child(self.render_shelve_panel(cx)).into_any_element(),
            SidebarMode::Detail => match &self.selected {
                Some(commit) => base.child(self.render_detail(commit, cx)).into_any_element(),
                None => base.child(self.render_workspace(cx)).into_any_element(),
            },
            SidebarMode::Workspace => base.child(self.render_workspace(cx)).into_any_element(),
        }
    }

    fn render_diff_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let title = self.diff_title.clone();

        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_sm()
                            .text_color(muted)
                            .child(title),
                    )
                    .when(self.diff_path.is_some(), |header| {
                        header.child(
                            Button::new("diff-blame")
                                .ghost()
                                .label("Blame")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let path = this.diff_path.clone().unwrap_or_default();
                                    this.open_blame(path, cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("diff-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.diff_files.is_empty() {
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("No changes"),
            );
        } else {
            panel = panel.child(
                div()
                    .id("diff-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_diff_files(&self.diff_files, cx)),
            );
        }
        panel
    }

    fn render_blame_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let path = self.blame_path.clone();

        let mut panel = div()
            .flex()
            .flex_col()
            .gap_2()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("Blame · {path}")),
                    )
                    .child(
                        Button::new("blame-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx))),
                    ),
            );

        if self.blame_groups.is_empty() {
            panel = panel.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(muted)
                    .child("Nothing to blame"),
            );
        } else {
            panel = panel.child(
                div()
                    .id("blame-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_blame(&self.blame_groups, cx)),
            );
        }
        panel
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let amend_label = if self.amend { "✓ Amend" } else { "Amend" };
        div()
            .flex_none()
            .border_t_1()
            .border_color(border)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(Textarea::new(&self.message_input).h(px(72.)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("shelve")
                            .ghost()
                            .label("Shelve…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_prompt(PromptKind::Stash, cx)
                            })),
                    )
                    .child(
                        Button::new("amend")
                            .ghost()
                            .label(amend_label)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.amend = !this.amend;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("commit")
                            .primary()
                            .label("Commit")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.do_commit(window, cx)
                            })),
                    ),
            )
    }

    fn render_statusbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        div()
            .h(px(28.))
            .flex_none()
            .border_t_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .text_xs()
            .child(
                match (&self.error, self.status_message.is_empty()) {
                    (Some(error), _) => div()
                        .text_color(hsla(0.0, 0.75, 0.55, 1.0))
                        .child(error.clone())
                        .into_any_element(),
                    (None, false) => div()
                        .text_color(muted)
                        .child(self.status_message.clone())
                        .into_any_element(),
                    (None, true) => div().into_any_element(),
                },
            )
            .child(div().flex_1())
            .when(self.ahead > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↑{}", self.ahead)))
            })
            .when(self.behind > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↓{}", self.behind)))
            })
    }

    fn render_prompt_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let kind = self.prompt.clone()?;
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let (title, hint, ok_label) = match &kind {
            PromptKind::NewBranch { start_point } => (
                "New branch",
                match start_point {
                    Some(point) => format!("From commit {}", &point[..point.len().min(7)]),
                    None => "From current HEAD".to_string(),
                },
                "Create",
            ),
            PromptKind::NewTag { commit_id } => (
                "New tag",
                format!("On commit {}", &commit_id[..commit_id.len().min(7)]),
                "Tag",
            ),
            PromptKind::Stash => (
                "Stash changes",
                "Optional message; untracked files are included".to_string(),
                "Stash",
            ),
            PromptKind::Reword { commit_id } => (
                "Reword commit",
                format!(
                    "New message for {}",
                    &commit_id[..commit_id.len().min(7)]
                ),
                "Reword",
            ),
            PromptKind::RenameBranch => (
                "Rename branch",
                match &self.current_branch {
                    Some(name) => format!("Rename current branch {name} to:"),
                    None => "No current branch".to_string(),
                },
                "Rename",
            ),
        };
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(hsla(0.0, 0.0, 0.0, 0.45))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(440.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(border)
                        .bg(cx.theme().background)
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_sm().child(title))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child(SharedString::from(hint)),
                        )
                        .child(Textarea::new(&self.prompt_input).h(px(64.)))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("prompt-cancel")
                                        .ghost()
                                        .label("Cancel")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cancel_prompt(cx)
                                        })),
                                )
                                .child(
                                    Button::new("prompt-ok")
                                        .primary()
                                        .label(ok_label)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.confirm_prompt(window, cx)
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .relative()
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(self.render_commit_panel(cx))
                    .child(self.render_sidebar(cx)),
            )
            .child(self.render_composer(cx))
            .child(self.render_statusbar(cx))
            .children(self.render_prompt_overlay(cx))
    }
}

pub fn run(repo_path: PathBuf) {
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        cx.spawn(async move |cx| {
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| AppView::new(repo_path, window, cx));
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("Failed to open window");
        })
        .detach();
    });
}