use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, size, AnyElement, Bounds, AppContext, Context, Div, Entity, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, WeakEntity, Window, WindowBounds, WindowOptions,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants, DropdownButton},
    input::{Textarea, TextareaState},
    list::{List, ListEvent, ListState},
    menu::PopupMenuItem,
    ActiveTheme, Root,
};
use rebased_rs::git::{
    load_repo_data, Change, Commit, GitError, RepoData, Repository, DEFAULT_LOG_LIMIT,
};

use crate::ui::commit_list::{format_time, LogData, LogDelegate};
use crate::ui::graph_view::{lane_color, status_color};

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
    branches: Arc<Vec<String>>,
    current_branch: Option<String>,
    current_upstream: Option<String>,
    changes: Vec<Change>,
    ahead: u32,
    behind: u32,
    amend: bool,
    selected: Option<Commit>,
    detail_files: Vec<Change>,
    detail_branches: Vec<String>,
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
        let subscriptions = vec![cx.subscribe_in(&list, window, Self::on_list_event)];

        let mut this = Self {
            repo_path,
            repo: None,
            list,
            message_input,
            branches: Arc::new(Vec::new()),
            current_branch: None,
            current_upstream: None,
            changes: Vec::new(),
            ahead: 0,
            behind: 0,
            amend: false,
            selected: None,
            detail_files: Vec::new(),
            detail_branches: Vec::new(),
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
        } = data;
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
        self.selected = None;
        self.detail_files.clear();
        self.detail_branches.clear();
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

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let branches = self.branches.clone();
        let current = self.current_branch.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
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
                Button::new("refresh")
                    .ghost()
                    .label("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
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
        let muted = cx.theme().muted_foreground;
        let color = status_color(&change.status);
        let path = change.display_path();
        let staged = change.staged;
        let change = change.clone();
        let change_for_click = change.clone();

        div()
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
            )
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .text_color(if staged { muted } else { lane_color(2) })
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

    fn render_detail(&self, commit: &Commit, cx: &mut Context<Self>) -> Div {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;

        let file_rows: Vec<AnyElement> = self
            .detail_files
            .iter()
            .map(|change| {
                let color = status_color(&change.status);
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_0p5()
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
                            .child(change.display_path()),
                    )
                    .into_any_element()
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
                        .text_color(lane_color(5))
                        .child(format!("∟ {}", self.detail_branches.join(", "))),
                )
            })
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

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = cx.theme().border;
        let base = div()
            .w(px(360.))
            .flex_none()
            .border_l_1()
            .border_color(border)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .overflow_hidden();

        match &self.selected {
            Some(commit) => base.child(self.render_detail(commit, cx)).into_any_element(),
            None => base.child(self.render_workspace(cx)).into_any_element(),
        }
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let amend_label = if self.amend {
            "✓ Amend"
        } else {
            "Amend"
        };

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
            .child(match (&self.error, &self.status_message.is_empty()) {
                (Some(error), _) => div()
                    .text_color(hsla(0.0, 0.75, 0.55, 1.0))
                    .child(error.clone())
                    .into_any_element(),
                (None, false) => div()
                    .text_color(muted)
                    .child(self.status_message.clone())
                    .into_any_element(),
                (None, true) => div().into_any_element(),
            })
            .child(div().flex_1())
            .when(self.ahead > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↑{}", self.ahead)))
            })
            .when(self.behind > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↓{}", self.behind)))
            })
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
