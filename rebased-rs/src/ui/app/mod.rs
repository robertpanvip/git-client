use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    div, px, size, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, WindowBounds, WindowOptions,
};
use gpui_kit::component::{input::TextareaState, list::ListState, ActiveTheme, Root};
use rebased_rs::git::{
    load_repo_data, BlameGroup, Change, Commit, ConflictFile, ConflictHunk, FileDiff, GitError,
    HunkChoice, RebaseAction, RepoData, Repository, StashEntry, Tag, DEFAULT_LOG_LIMIT,
};

use crate::ui::commit_list::{LogData, LogDelegate};

mod actions;
mod conflicts;
mod detail;
mod detail_view;
mod panels;
mod rebase;
mod shelves;
mod toolbar;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarMode {
    Workspace,
    Detail,
    Diff,
    Blame,
    Rebase,
    Conflicts,
    Shelve,
}

#[derive(Clone)]
pub(crate) enum PromptKind {
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
