use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    div, px, size, AppContext, Bounds, Context, Entity, IntoElement, InteractiveElement,
    ParentElement, Render, Styled, Subscription, Window, WindowBounds, WindowOptions,
};
use gpui_kit::component::{input::TextareaState, list::ListState, ActiveTheme, Root};
use rebased_rs::git::{
    load_repo_data_filtered, open_backend, CancelToken, GitBackend, GitError, ProgressHandle,
    RepoData, DEFAULT_LOG_LIMIT,
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

pub(crate) use state::{AppState, ConfirmAction, DiffSource, PromptKind, RebaseFlow, SidebarMode};
use use_cases::sync_repo_state;

struct Loaded {
    repo: Arc<dyn GitBackend>,
    data: RepoData,
}

fn open_and_load(path: &Path) -> Result<Loaded, GitError> {
    let repo = open_backend(path)?;
    let data = load_repo_data_filtered(repo.as_ref(), DEFAULT_LOG_LIMIT, None, None, None)?;
    Ok(Loaded { repo, data })
}

pub struct AppView {
    repo_path: PathBuf,
    repo: Option<Arc<dyn GitBackend>>,
    /// 上次自动刷新检测到的仓库指纹，变化时才触发 refresh。
    repo_digest: String,
    pub(crate) state: AppState,
    list: Entity<ListState<LogDelegate>>,
    message_input: Entity<TextareaState>,
    prompt_input: Entity<TextareaState>,
    /// AddRemote 对话框的第二个输入框（remote URL）。
    prompt_input2: Entity<TextareaState>,
    diff_edit_input: Entity<TextareaState>,
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
        let prompt_input2 = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("URL")
                .soft_wrap(false)
        });
        let diff_edit_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("File content")
                .soft_wrap(true)
        });
        let subscriptions = vec![cx.subscribe_in(&list, window, Self::on_list_event)];
        list.update(cx, |list, cx| list.focus(window, cx));

        let this = Self {
            repo_path,
            repo: None,
            repo_digest: String::new(),
            state: AppState::default(),
            list,
            message_input,
            prompt_input,
            prompt_input2,
            diff_edit_input,
            _subscriptions: subscriptions,
        };

        // 仓库加载放到后台线程执行，避免大仓库阻塞首帧绘制。
        let task = cx.background_spawn({
            let repo_path = this.repo_path.clone();
            async move { open_and_load(&repo_path) }
        });
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.loading = false;
                match loaded {
                    Ok(loaded) => {
                        this.repo = Some(loaded.repo);
                        this.apply_data(loaded.data, cx);
                    }
                    Err(e) => this.state.error = Some(e.to_string()),
                }
            });
        })
        .detach();

        // 自动刷新：周期性计算轻量仓库指纹（HEAD + 工作区状态），变化时才全量
        // refresh；busy/loading 期间跳过检测；首次只记录基准不刷新，entity 释放后退出。
        cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            loop {
                executor.timer(std::time::Duration::from_secs(5)).await;
                let Ok((repo, last)) = this.update(cx, |this, _| {
                    let runnable = this
                        .repo
                        .clone()
                        .filter(|_| !this.state.loading && this.state.busy.is_none());
                    (runnable, this.repo_digest.clone())
                }) else {
                    break;
                };
                let Some(repo) = repo else {
                    continue;
                };
                let task = executor.spawn(async move { repo.repo_digest() });
                let Ok(digest) = task.await else {
                    continue;
                };
                if digest == last {
                    continue;
                }
                let first_check = last.is_empty();
                let _ = this.update(cx, |this, cx| {
                    this.repo_digest = digest;
                    if !first_check {
                        this.refresh(cx);
                    }
                });
            }
        })
        .detach();
        this
    }

    fn apply_data(&mut self, data: RepoData, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match sync_repo_state(repo.as_ref(), &mut self.state, data) {
            Ok((commits, graph)) => {
                // 注入自身弱引用，供 delegate 内（ref 徽章菜单 / 双击）回调应用动作。
                let weak = cx.entity().downgrade();
                self.list.update(cx, |list, cx| {
                    list.delegate_mut().set_app(weak);
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
        let author = self.state.filter_author.trim().to_string();
        let author = if author.is_empty() { None } else { Some(author) };
        let branch = self.state.filter_branch.clone();
        let since = self.state.filter_since.clone().map(|(_, expr)| expr);
        let task = cx.background_spawn(async move {
            load_repo_data_filtered(
                repo.as_ref(),
                DEFAULT_LOG_LIMIT,
                branch.as_deref(),
                author.as_deref(),
                since.as_deref(),
            )
        });
        cx.spawn(async move |this, cx| {
            match task.await {
                Ok(data) => {
                    let _ = this.update(cx, |this, cx| this.apply_data(data, cx));
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.state.error = Some(e.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn run_op(
        &mut self,
        message: &str,
        op: impl FnOnce(&dyn GitBackend) -> Result<(), GitError> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        // 已有操作在后台执行时忽略新请求，避免并发写操作互相踩踏。
        if self.state.busy.is_some() {
            return;
        }
        let message = message.to_string();
        self.state.busy = Some(message.clone());
        cx.notify();
        let task = cx.background_spawn(async move { op(repo.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.busy = None;
                match result {
                    Ok(()) => {
                        this.state.error = None;
                        this.state.status_message = message;
                        this.refresh(cx);
                    }
                    Err(e) => {
                        this.state.error = Some(e.to_string());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    /// 带进度反馈与取消的 run_op 变体：op 额外接收进度槽与取消令牌，
    /// 执行期间每 250ms 把最新进度文本同步到状态栏；取消与失败走错误显示。
    fn run_op_progress(
        &mut self,
        busy_message: &str,
        done_message: &str,
        op: impl FnOnce(&dyn GitBackend, ProgressHandle, CancelToken) -> Result<(), GitError>
            + Send
            + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        // 已有操作在后台执行时忽略新请求，避免并发写操作互相踩踏。
        if self.state.busy.is_some() {
            return;
        }
        let done_message = done_message.to_string();
        let progress: ProgressHandle = Arc::new(Mutex::new(None));
        let progress_watch = Arc::clone(&progress);
        let cancel: CancelToken = Arc::new(AtomicBool::new(false));
        self.state.progress_text = None;
        self.state.cancel_token = Some(Arc::clone(&cancel));
        self.state.busy = Some(busy_message.to_string());
        self.state.error = None;
        cx.notify();

        // 共享完成槽：后台任务结束时写入结果，轮询循环据此收尾
        let done: Arc<Mutex<Option<Result<(), GitError>>>> = Arc::new(Mutex::new(None));
        let done_slot = Arc::clone(&done);
        cx.background_spawn(async move {
            let result = op(repo.as_ref(), progress, cancel);
            if let Ok(mut slot) = done_slot.lock() {
                *slot = Some(result);
            }
        })
        .detach();

        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            loop {
                executor.timer(Duration::from_millis(250)).await;
                let (result, text) = {
                    let result = done.lock().ok().and_then(|mut slot| slot.take());
                    let text = progress_watch
                        .lock()
                        .ok()
                        .and_then(|slot| slot.as_ref().cloned());
                    (result, text)
                };
                let keep_going = this
                    .update(cx, |this, cx| {
                        if text.is_some() {
                            this.state.progress_text = text;
                            cx.notify();
                        }
                        match result {
                            Some(result) => {
                                this.state.busy = None;
                                this.state.cancel_token = None;
                                this.state.progress_text = None;
                                match result {
                                    Ok(()) => {
                                        this.state.error = None;
                                        this.state.status_message = done_message.clone();
                                        this.refresh(cx);
                                    }
                                    Err(e) => {
                                        this.state.error = Some(e.to_string());
                                        cx.notify();
                                    }
                                }
                                false
                            }
                            None => true,
                        }
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
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
            .on_action(cx.listener(Self::on_commit_selected))
            .on_action(cx.listener(Self::on_push_branch))
            .on_action(cx.listener(Self::on_pull_branch))
            .on_action(cx.listener(Self::on_refresh_repo))
            .on_action(cx.listener(Self::on_close_overlay))
            .on_action(cx.listener(Self::on_select_prev_commit))
            .on_action(cx.listener(Self::on_select_next_commit))
            .on_action(cx.listener(Self::on_focus_composer))
            .on_action(cx.listener(Self::on_toggle_vcs_palette))
            .on_action(cx.listener(Self::on_blame_current_file))
            .on_action(cx.listener(Self::on_select_sidebar_panel))
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
            .children(self.render_vcs_palette(cx))
            .children(self.render_prompt_overlay(cx))
    }
}

pub fn run(repo_path: PathBuf) {
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        actions::register_keybindings(cx);
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
