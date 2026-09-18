use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Bounds, Context, Div, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, UniformListScrollHandle, Window, WindowBounds, WindowOptions, div, px, size,
};
use gpui_kit::component::{
    ActiveTheme, Icon, Root,
    input::{InputEvent, InputState, TextareaState},
    list::ListState,
    theme::Theme,
    theme::ThemeMode,
};
use rebased_rs::git::{
    CancelToken, DEFAULT_LOG_LIMIT, GitBackend, GitError, ProgressHandle, RepoData,
    load_repo_data_filtered, open_backend,
};

use crate::ui::commit_list::{LogData, LogDelegate};
use crate::ui::components::{SplitDrag, SplitSide, drag_overlay, v_handle};
use crate::ui::i18n::{self, tr};
use crate::ui::icons;
use crate::ui::settings;
use crate::ui::theme;

mod actions;
mod conflicts;
mod detail;
mod detail_view;
mod diff_window;
mod files;
mod rebase;
mod shelves;
mod sidebar;
mod state;
mod toolbar;
mod use_cases;

pub(crate) use state::{
    AppState, ConfirmAction, DiffSource, MainView, PromptKind, RebaseFlow, SidebarMode,
};
use use_cases::{reload_conflict_state, sync_repo_state};

struct Loaded {
    repo: Arc<dyn GitBackend>,
    data: RepoData,
}

/// X11 窗口图标（Windows 走 exe 内嵌资源、macOS 走 app bundle，均不经过此路径）。
fn load_window_icon() -> Option<Arc<image::RgbaImage>> {
    image::load_from_memory(include_bytes!("../../../assets/icon.png"))
        .ok()
        .map(|img| Arc::new(img.into_rgba8()))
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
    /// Log 过滤行搜索输入（“Text or hash”），变化时转发 ListState::set_query。
    log_query: Entity<InputState>,
    /// Git 日志视图左栏分支树顶部的“分支或标签”搜索输入。
    branch_query: Entity<InputState>,
    /// 分支部件弹窗顶部的「搜索分支和操作」输入。
    branch_popup_query: Entity<InputState>,
    /// 文件视图左栏（文件夹树）的滚动位置。
    tree_scroll: UniformListScrollHandle,
    /// 文件视图右栏（代码区域）的滚动位置。
    editor_scroll: UniformListScrollHandle,
    message_input: Entity<TextareaState>,
    prompt_input: Entity<TextareaState>,
    /// AddRemote 对话框的第二个输入框（remote URL）。
    prompt_input2: Entity<TextareaState>,
    diff_edit_input: Entity<TextareaState>,
    /// 左栏（Commit 面板）宽度，用户可拖拽分隔条调整。
    commit_panel_width: f32,
    /// 右栏（详情 / Diff / Rebase…）宽度，用户可拖拽调整；
    /// 切换侧栏面板时重置为该面板默认宽度。
    right_panel_width: f32,
    /// 上一次渲染时的侧栏面板，用于检测面板切换并重置宽度。
    last_sidebar_mode: SidebarMode,
    /// 进行中的分栏拖拽快照（Some = 正在拖拽，渲染透明覆盖层）。
    split_drag: Option<SplitDrag>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    fn new(repo_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 搜索框由过滤行自绘（对齐原版 37px 行高），ListState 关闭内置搜索 UI；
        // set_query 仍会触发 perform_search，delegate 的过滤逻辑不变。
        let list = cx.new(|cx| ListState::new(LogDelegate::new(), window, cx).searchable(false));
        let log_query =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("Text or hash", "文本或哈希")));
        let branch_query =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("Branch or tag", "分支或标签")));
        let branch_popup_query = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr("Search branches and actions", "搜索分支和操作"))
        });
        let message_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(tr("Commit message", "提交信息"))
                .soft_wrap(true)
        });
        let prompt_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(tr("Name / message", "名称 / 消息"))
                .soft_wrap(false)
        });
        let prompt_input2 = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("URL")
                .soft_wrap(false)
        });
        let diff_edit_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(tr("File content", "文件内容"))
                .soft_wrap(true)
        });
        let subscriptions = vec![
            cx.subscribe_in(&list, window, Self::on_list_event),
            cx.subscribe_in(&log_query, window, Self::on_log_query_changed),
            // 分支树搜索只影响该栏的渲染，输入变化重绘即可。
            cx.subscribe_in(&branch_query, window, |_, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            // 分支弹窗内的搜索同理：仅驱动弹窗内容过滤。
            cx.subscribe_in(&branch_popup_query, window, |_, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        list.update(cx, |list, cx| list.focus(window, cx));

        let this = Self {
            repo_path,
            repo: None,
            repo_digest: String::new(),
            state: AppState::default(),
            list,
            log_query,
            branch_query,
            branch_popup_query,
            message_input,
            prompt_input,
            prompt_input2,
            diff_edit_input,
            commit_panel_width: theme::COMMIT_PANEL_WIDTH,
            right_panel_width: SidebarMode::Workspace.default_width(),
            last_sidebar_mode: SidebarMode::Workspace,
            tree_scroll: UniformListScrollHandle::new(),
            editor_scroll: UniformListScrollHandle::new(),
            split_drag: None,
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

    /// Log 过滤行输入变化：转发给 ListState 触发 delegate 的 perform_search。
    fn on_log_query_changed(
        &mut self,
        _entity: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            let query = self.log_query.read(cx).value().to_string();
            self.list
                .update(cx, |list, cx| list.set_query(&query, window, cx));
        }
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
        // 文件视图开启时同步刷新文件树与当前文件，避免磁盘变化后显示陈旧内容。
        if self.state.main_view == MainView::Files {
            self.load_files(cx);
            if let Some(path) = self.state.files_selected.clone() {
                self.open_file(path, cx);
            }
        }
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let author = self.state.filter_author.trim().to_string();
        let author = if author.is_empty() {
            None
        } else {
            Some(author)
        };
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
        cx.spawn(async move |this, cx| match task.await {
            Ok(data) => {
                let _ = this.update(cx, |this, cx| this.apply_data(data, cx));
            }
            Err(e) => {
                let _ = this.update(cx, |this, cx| {
                    this.state.error = Some(e.to_string());
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// 弹出系统“选择文件夹”对话框，把窗口切换到用户选中的本地 Git 仓库。
    fn open_repo_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 有后台写操作时忽略，避免切换仓库与进行中的操作互相踩踏。
        if self.state.busy.is_some() {
            return;
        }
        // 切换仓库后旧的文本/哈希过滤不再适用：清空搜索框。
        // InputState::set_value 内部关闭了事件发射，不会触发 Change，
        // 因此还需手动 set_query 让 delegate 立即回到全量列表。
        self.log_query
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.list
            .update(cx, |list, cx| list.set_query("", window, cx));
        // set_parent 只借用 window 句柄（内部立即转为可 Copy 的 RawWindowHandle），
        // 对话框可以安全地移到后台线程 await。
        let dialog = rfd::AsyncFileDialog::new()
            .set_title(tr("Open Project", "打开项目"))
            .set_parent(&*window);
        let task = cx.background_spawn(async move {
            dialog
                .pick_folder()
                .await
                .map(|handle| handle.path().to_path_buf())
        });
        cx.spawn(async move |this, cx| {
            let Some(path) = task.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.switch_repo(path, cx));
        })
        .detach();
    }

    /// 切换到新的仓库：整体重置应用状态（过滤器/选中/面板），后台重载全部数据。
    fn switch_repo(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.state.busy.is_some() || path == self.repo_path {
            return;
        }
        self.repo_path = path;
        self.repo = None;
        self.repo_digest = String::new();
        self.state = AppState::default();
        cx.notify();
        let task = cx.background_spawn({
            let repo_path = self.repo_path.clone();
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
                    Err(e) => {
                        this.state.error = Some(e.to_string());
                        cx.notify();
                    }
                }
            });
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
                        this.handle_op_failure(e, cx);
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
                                        this.handle_op_failure(e, cx);
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

    /// 操作失败后的统一处理：pull/merge 因分支冲突失败时，仓库其实已进入
    /// 冲突状态——识别后把报错换成友好提示、自动打开冲突面板并刷新仓库视图；
    /// 无冲突则原样展示 git 错误。
    fn handle_op_failure(&mut self, error: GitError, cx: &mut Context<Self>) {
        if let Some(repo) = self.repo.clone() {
            let conflicts = repo.as_ref().conflicted_files().unwrap_or_default();
            if !conflicts.is_empty() {
                let n = conflicts.len();
                let _ = reload_conflict_state(repo.as_ref(), &mut self.state);
                self.state.error = Some(format!(
                    "{} ({}): {}",
                    tr(
                        "Sync stopped by branch conflicts, resolve them in the Conflicts panel",
                        "同步因分支冲突停止，请在冲突面板解决后提交",
                    ),
                    n,
                    error
                ));
                self.refresh(cx);
                return;
            }
        }
        self.state.error = Some(error.to_string());
        cx.notify();
    }

    /// 左侧 40px 图标条（对齐原版 New UI 竖条）：Git / History / Shelve 快捷入口。
    fn render_icon_strip(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(theme::ICON_STRIP_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme::SPACE_XS))
            .pt(px(theme::SPACE_SM))
            .bg(theme::bg_chrome())
            .border_r_1()
            .border_color(theme::separator())
            .child(self.render_strip_button(
                "strip-files",
                icons::Ic::Folder,
                self.state.main_view == MainView::Files,
                |this, cx| this.open_files_view(cx),
                cx,
            ))
            .child(self.render_strip_button(
                "strip-git",
                icons::Ic::Changes,
                self.state.main_view == MainView::Log,
                |this, cx| {
                    // 主窗口左下角的 Git 图标：在「工作区（图1）」与「Git 日志（图2）」间切换。
                    eprintln!(
                        "[diag] strip-git clicked, main_view={:?}",
                        this.state.main_view
                    );
                    this.state.main_view = match this.state.main_view {
                        MainView::Workspace => MainView::Log,
                        MainView::Log | MainView::Files => MainView::Workspace,
                    };
                    cx.notify();
                },
                cx,
            ))
            .child(self.render_strip_button(
                "strip-history",
                icons::Ic::History,
                matches!(self.state.sidebar, SidebarMode::History),
                |this, cx| {
                    // 文件视图不承载侧栏面板：切回工作区视图再打开该面板。
                    eprintln!("[diag] strip-history clicked");
                    if this.state.main_view == MainView::Files {
                        this.state.main_view = MainView::Workspace;
                    }
                    this.state.sidebar = SidebarMode::History;
                    cx.notify();
                },
                cx,
            ))
            .child(self.render_strip_button(
                "strip-shelve",
                icons::Ic::Shelve,
                matches!(self.state.sidebar, SidebarMode::Shelve),
                |this, cx| {
                    if this.state.main_view == MainView::Files {
                        this.state.main_view = MainView::Workspace;
                    }
                    this.state.sidebar = SidebarMode::Shelve;
                    cx.notify();
                },
                cx,
            ))
    }

    /// 图标条单按钮：选中态蓝底白字，未选中悬停淡入。
    fn render_strip_button(
        &self,
        id: &'static str,
        icon: icons::Ic,
        active: bool,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let fg = if active {
            theme::white()
        } else {
            theme::text_muted()
        };
        div()
            .id(id)
            .size(px(theme::ICON_STRIP_BUTTON_SIZE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS_SM))
            .text_color(fg)
            .cursor_pointer()
            .when(active, |b| b.bg(theme::selection_bg()))
            .when(!active, |b| b.hover(move |s| s.bg(theme::hover_bg(fg))))
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
            .child(Icon::new(icon))
    }

    /// 左栏分隔条：拖动改变 Commit 面板宽度。
    fn render_left_splitter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_handle("split-left").on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.split_drag = Some(SplitDrag::new(
                    SplitSide::Left,
                    f32::from(event.position.x),
                    this.commit_panel_width,
                ));
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }

    /// 右栏分隔条：拖动改变右侧面板宽度。
    fn render_right_splitter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_handle("split-right").on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.split_drag = Some(SplitDrag::new(
                    SplitSide::Right,
                    f32::from(event.position.x),
                    this.right_panel_width,
                ));
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }

    /// 拖拽期间的全窗口覆盖层：承接 move / up。
    /// 6px 命中区在快速拖动时容易丢失事件，覆盖层确保拖拽连续。
    fn render_split_overlay(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.split_drag.as_ref()?;
        Some(
            drag_overlay("split-overlay")
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                    let x = f32::from(event.position.x);
                    let Some(drag) = this.split_drag else {
                        return;
                    };
                    let window_width = f32::from(window.bounds().size.width);
                    // 主区最小宽度必须保留，两侧面板共同让位。
                    let reserved = theme::ICON_STRIP_WIDTH
                        + theme::SPLITTER_HIT_WIDTH * 2.0
                        + theme::MIN_MAIN_PANEL_WIDTH;
                    match drag.side {
                        SplitSide::Left => {
                            let max = (window_width - reserved - this.right_panel_width)
                                .max(theme::MIN_LEFT_PANEL_WIDTH);
                            this.commit_panel_width =
                                drag.width_at(x, theme::MIN_LEFT_PANEL_WIDTH, max);
                        }
                        SplitSide::Right => {
                            let min = this.state.sidebar.min_width();
                            let max = (window_width - reserved - this.commit_panel_width).max(min);
                            this.right_panel_width = drag.width_at(x, min, max);
                        }
                    }
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.split_drag = None;
                        cx.notify();
                    }),
                ),
        )
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 切换侧栏面板时，右栏宽度回到该面板的默认值（其后用户拖拽可覆盖）。
        if self.last_sidebar_mode != self.state.sidebar {
            self.last_sidebar_mode = self.state.sidebar;
            self.right_panel_width = self.state.sidebar.default_width();
        }
        let dragging = self.split_drag.is_some();
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
            .on_action(cx.listener(Self::on_confirm_prompt))
            .on_action(cx.listener(Self::on_select_prev_commit))
            .on_action(cx.listener(Self::on_select_next_commit))
            .on_action(cx.listener(Self::on_focus_composer))
            .on_action(cx.listener(Self::on_toggle_vcs_palette))
            .on_action(cx.listener(Self::on_blame_current_file))
            .on_action(cx.listener(Self::on_select_sidebar_panel))
            .child(self.render_toolbar(cx))
            .child({
                let mut row = div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(self.render_icon_strip(cx));
                if self.state.main_view == MainView::Log {
                    // 图2 Git 日志视图：分支树 | 提交列表 | 提交详情。
                    // 分支树为固定窄栏（原版 196），故不挂左分隔条。
                    row = row
                        .child(self.render_branch_column(cx))
                        .child(self.render_commit_panel(cx))
                        .child(self.render_right_splitter(cx))
                        .child(self.render_sidebar(cx));
                } else if self.state.main_view == MainView::Files {
                    // 文件视图：文件夹树（固定窄栏）| 代码区域。
                    row = row
                        .child(self.render_files_tree_column(cx))
                        .child(self.render_files_editor(cx));
                } else {
                    // 图1 工作区视图：左 = 变更 + 提交信息，右 = 单栏只读预览。
                    // Blame / Compare / Rebase 等宽面板仍以右栏形式出现。
                    row = row
                        .child(self.render_commit_sidebar(cx))
                        .child(self.render_left_splitter(cx))
                        .child(self.render_preview_panel(cx));
                    if !matches!(
                        self.state.sidebar,
                        SidebarMode::Workspace | SidebarMode::Diff
                    ) {
                        row = row
                            .child(self.render_right_splitter(cx))
                            .child(self.render_sidebar(cx));
                    }
                }
                row
            })
            .child(self.render_statusbar(cx))
            .children(if dragging {
                self.render_split_overlay(cx)
            } else {
                None
            })
            .children(self.render_vcs_palette(cx))
            .children(self.render_branch_popup(cx))
            .children(self.render_prompt_overlay(window, cx))
    }
}

pub fn run(repo_path: PathBuf) {
    i18n::load_persisted();
    settings::log_event("entering gpui application");
    gpui_kit::application()
        .with_assets(icons::AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            actions::register_keybindings(cx);
            if std::env::var_os("GPUI_DISABLE_DIRECT_COMPOSITION").is_some() {
                settings::log_event("GPUI_DISABLE_DIRECT_COMPOSITION is set");
            }
            // 像素级对齐原版：默认 JetBrains New UI Dark；用户显式保存过则用保存值。
            let mode = settings::load_theme_mode().unwrap_or(ThemeMode::Dark);
            settings::log_event(&format!("theme mode: {}", mode.name()));
            Theme::change(mode, None, cx);
            theme::apply_jetbrains_palette(cx);
            let bounds = Bounds::centered(
                None,
                size(px(theme::MAIN_WINDOW_WIDTH), px(theme::MAIN_WINDOW_HEIGHT)),
                cx,
            );
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                icon: load_window_icon(),
                ..Default::default()
            };
            settings::log_event("opening main window");
            cx.spawn(async move |cx| {
                match cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| AppView::new(repo_path, window, cx));
                    cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
                }) {
                    Ok(_) => settings::log_event("main window opened"),
                    Err(err) => settings::log_event(&format!("failed to open window: {err:#}")),
                }
            })
            .detach();
        });
}
