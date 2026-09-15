use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    div, px, size, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, WindowBounds, WindowOptions,
};
use gpui_kit::component::{input::TextareaState, list::ListState, ActiveTheme, Root};
use rebased_rs::git::{
    load_repo_data, open_backend, GitBackend, GitError, RepoData, DEFAULT_LOG_LIMIT,
};

use crate::ui::commit_list::{LogData, LogDelegate};

mod actions;
mod conflicts;
mod detail;
mod detail_view;
mod panels;
mod rebase;
mod shelves;
mod state;
mod toolbar;
mod use_cases;

pub(crate) use state::{AppState, ConfirmAction, PromptKind, RebaseFlow, SidebarMode};
use use_cases::sync_repo_state;

struct Loaded {
    repo: Arc<dyn GitBackend>,
    data: RepoData,
}

fn open_and_load(path: &Path) -> Result<Loaded, GitError> {
    let repo = open_backend(path)?;
    let data = load_repo_data(repo.as_ref(), DEFAULT_LOG_LIMIT)?;
    Ok(Loaded { repo, data })
}

pub struct AppView {
    repo_path: PathBuf,
    repo: Option<Arc<dyn GitBackend>>,
    state: AppState,
    list: Entity<ListState<LogDelegate>>,
    message_input: Entity<TextareaState>,
    prompt_input: Entity<TextareaState>,
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
            state: AppState::default(),
            list,
            message_input,
            prompt_input,
            _subscriptions: subscriptions,
        };

        match open_and_load(&this.repo_path) {
            Ok(loaded) => {
                this.repo = Some(loaded.repo);
                this.apply_data(loaded.data, cx);
                this.state.loading = false;
            }
            Err(e) => {
                this.state.loading = false;
                this.state.error = Some(e.to_string());
            }
        }
        this
    }

    fn apply_data(&mut self, data: RepoData, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match sync_repo_state(repo.as_ref(), &mut self.state, data) {
            Ok((commits, graph)) => {
                self.list.update(cx, |list, cx| {
                    list.delegate_mut().set_data(LogData { commits, graph });
                    cx.notify();
                });
            }
            Err(e) => self.state.error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match load_repo_data(repo.as_ref(), DEFAULT_LOG_LIMIT) {
            Ok(data) => self.apply_data(data, cx),
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    fn run_op(
        &mut self,
        message: &str,
        op: impl FnOnce(&dyn GitBackend) -> Result<(), GitError>,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match op(repo.as_ref()) {
            Ok(()) => {
                self.state.error = None;
                self.state.status_message = message.to_string();
                self.refresh(cx);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
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
