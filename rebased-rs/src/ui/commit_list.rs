use std::sync::Arc;

use chrono::{DateTime, Local};
use gpui::{
    App, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Task, WeakEntity, Window, div, px,
};
use gpui_kit::base::IndexPath;
use gpui_kit::component::{
    ActiveTheme,
    list::{ListDelegate, ListItem, ListState},
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
};
use rebased_rs::git::{Commit, Graph, build_graph, filter_commits};

use crate::ui::app::{AppView, ConfirmAction, PromptKind};
use crate::ui::components::{badge, empty_state, ref_style};
use crate::ui::graph_view::{ROW_HEIGHT, lane_canvas};
use crate::ui::i18n::tr;

pub struct LogData {
    pub commits: Vec<Commit>,
    pub graph: Graph,
}

impl LogData {
    pub fn new(commits: Vec<Commit>) -> Self {
        let graph = build_graph(&commits);
        Self { commits, graph }
    }

    pub fn filtered(&self, query: &str) -> Self {
        let commits = filter_commits(&self.commits, query)
            .into_iter()
            .cloned()
            .collect();
        Self::new(commits)
    }
}

pub struct LogDelegate {
    data: Option<Arc<LogData>>,
    visible: Option<Arc<LogData>>,
    selected: Option<IndexPath>,
    /// AppView 的弱引用，用于 ref 徽章菜单与双击 checkout 联动应用层动作。
    app: Option<WeakEntity<AppView>>,
}

impl LogDelegate {
    pub fn new() -> Self {
        Self {
            data: None,
            visible: None,
            selected: None,
            app: None,
        }
    }

    pub fn set_app(&mut self, app: WeakEntity<AppView>) {
        self.app = Some(app);
    }

    pub fn set_data(&mut self, data: LogData) {
        self.data = Some(Arc::new(data));
        self.visible = self.data.clone();
        self.selected = None;
    }

    pub fn commit_at(&self, row: usize) -> Option<Commit> {
        self.visible
            .as_ref()
            .and_then(|data| data.commits.get(row))
            .cloned()
    }

    pub fn find_commit(&self, id: &str) -> Option<Commit> {
        let data = self.data.as_ref()?;
        data.commits.iter().find(|c| c.id.0 == id).cloned()
    }

    /// 当前可见（含过滤后）的提交数量，供键盘导航使用。
    pub fn visible_count(&self) -> usize {
        self.visible.as_ref().map_or(0, |data| data.commits.len())
    }

    fn rebuild(&mut self, query: &str) {
        let Some(data) = self.data.clone() else {
            return;
        };
        self.visible = if query.trim().is_empty() {
            Some(data)
        } else {
            Some(Arc::new(data.filtered(query)))
        };
        self.selected = None;
    }
}

impl Default for LogDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl ListDelegate for LogDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.visible.as_ref().map_or(0, |data| data.commits.len())
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.rebuild(query);
        cx.notify();
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
        cx.notify();
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let data = self.visible.as_ref()?;
        let commit = data.commits.get(ix.row)?;
        let selected = self.selected == Some(ix);
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let app = self.app.clone();

        let mut row = div()
            .id(SharedString::from(format!("commit-row-{}", ix.row)))
            .h(px(ROW_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .overflow_hidden();

        let graph_row = data.graph.rows.get(ix.row).cloned();
        row = row.child(lane_canvas(graph_row, data.graph.lane_count));

        let mut refs_col = div().flex_none().flex().flex_row().items_center().gap_1();
        for (i, ref_name) in commit.refs.iter().enumerate() {
            let badge_id = SharedString::from(format!("ref-badge-{}-{}", ix.row, i));
            refs_col = refs_col.child(ref_badge(&badge_id, ref_name, app.clone()));
        }
        row = row.child(refs_col);

        // 列式布局：graph | refs | subject(flex) | author(固定列) | date(固定列)，
        // 与原版对齐——多行提交时 author/date 始终纵向对齐。
        row = row
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_sm()
                    .text_color(fg)
                    .child(commit.subject.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(96.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_xs()
                    .text_color(muted)
                    .child(commit.author.name.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(72.))
                    .text_xs()
                    .text_color(muted)
                    .child(format_time(commit.time)),
            );

        // 双击 commit = Checkout（IntelliJ 惯例）。行选中由 ListState 统一处理，
        // 这里只拦截双击，与单击选择互不干扰。
        row = row.on_click({
            let app = app.clone();
            let commit_id = commit.id.0.clone();
            move |click: &ClickEvent, _: &mut Window, cx: &mut App| {
                if click.click_count() < 2 {
                    return;
                }
                if let Some(app) = app.as_ref().and_then(|app| app.upgrade()) {
                    let id = commit_id.clone();
                    app.update(cx, |this, cx| this.checkout_commit(id, cx));
                }
            }
        });

        Some(ListItem::new(ix.row).selected(selected).px_2().child(row))
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_state(
            tr("No commits", "没有提交"),
            cx.theme().muted_foreground.opacity(0.6),
        )
    }
}

fn ref_badge(id: &str, name: &str, app: Option<WeakEntity<AppView>>) -> impl IntoElement {
    let (label, color) = ref_style(name);

    let name = name.to_string();
    badge(SharedString::from(id.to_string()), label, color).context_menu(
        move |menu, _window, cx| match &app {
            Some(app) => build_ref_menu(menu, &name, app, cx),
            None => menu,
        },
    )
}

/// ref 徽章的右键菜单：tag 可删除；分支按本地/远程给出对应操作
/// （与 toolbar 分支菜单能力对齐），纯 HEAD 徽章无操作。
fn build_ref_menu(menu: PopupMenu, name: &str, app: &WeakEntity<AppView>, cx: &App) -> PopupMenu {
    if let Some(tag) = name.strip_prefix("tag: ") {
        let tag = tag.to_string();
        return menu.item(
            PopupMenuItem::new(format!("✕ Delete tag {tag}…")).on_click({
                let app = app.clone();
                move |_, _, cx| {
                    let _ = app.update(cx, |this, cx| {
                        this.open_prompt(
                            PromptKind::Confirm(ConfirmAction::DeleteTag { name: tag.clone() }),
                            cx,
                        )
                    });
                }
            }),
        );
    }

    let branch = name.strip_prefix("HEAD -> ").unwrap_or(name).to_string();
    let entry = app.upgrade().and_then(|view| {
        view.read(cx)
            .state
            .branch_entries
            .iter()
            .find(|b| b.name == branch)
            .cloned()
    });
    if entry.as_ref().is_none_or(|b| b.is_remote) {
        menu.item(
            PopupMenuItem::new(format!("⇥ Checkout {branch} (tracking)")).on_click({
                let app = app.clone();
                let branch = branch.clone();
                move |_, _, cx| {
                    let _ = app.update(cx, |this, cx| this.checkout_branch(&branch, cx));
                }
            }),
        )
        .item(
            PopupMenuItem::new(format!("⇄ Pull {branch} into current")).on_click({
                let app = app.clone();
                let branch = branch.clone();
                move |_, _, cx| {
                    let _ = app.update(cx, |this, cx| {
                        this.pull_branch_into_current(branch.clone(), cx)
                    });
                }
            }),
        )
        .item(
            PopupMenuItem::new(format!("⇋ Compare {branch} with current")).on_click({
                let app = app.clone();
                let branch = branch.clone();
                move |_, _, cx| {
                    let _ = app.update(cx, |this, cx| this.open_branch_compare(branch.clone(), cx));
                }
            }),
        )
    } else {
        let is_current = entry.as_ref().is_some_and(|b| b.is_current());
        let has_upstream = entry.as_ref().is_none_or(|b| b.upstream.is_some());
        menu.item(
            PopupMenuItem::new(format!("✓ Checkout {branch}"))
                .checked(is_current)
                .on_click({
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| this.checkout_branch(&branch, cx));
                    }
                }),
        )
        .item(PopupMenuItem::new(format!("⇪ Push {branch}")).on_click({
            let app = app.clone();
            let branch = branch.clone();
            move |_, _, cx| {
                let _ = app.update(cx, |this, cx| {
                    this.push_branch(branch.clone(), has_upstream, cx)
                });
            }
        }))
        .item(PopupMenuItem::new(format!("✎ Rename {branch}…")).on_click({
            let app = app.clone();
            let branch = branch.clone();
            move |_, _, cx| {
                let _ = app.update_in(cx, |this, window, cx| {
                    this.open_rename_branch_by_name(branch.clone(), window, cx)
                });
            }
        }))
        .item(PopupMenuItem::new(format!("✕ Delete {branch}…")).on_click({
            let app = app.clone();
            let branch = branch.clone();
            move |_, _, cx| {
                let _ = app.update(cx, |this, cx| {
                    this.open_prompt(
                        PromptKind::Confirm(ConfirmAction::DeleteBranch {
                            name: branch.clone(),
                        }),
                        cx,
                    )
                });
            }
        }))
    }
}

pub(crate) fn format_time(secs: i64) -> String {
    match DateTime::from_timestamp(secs, 0) {
        Some(time) => time.with_timezone(&Local).format("%m-%d %H:%M").to_string(),
        None => String::new(),
    }
}

/// 完整日期时间（含年份与秒），用于 Detail 头部的提交时间展示。
pub(crate) fn format_full_time(secs: i64) -> String {
    match DateTime::from_timestamp(secs, 0) {
        Some(time) => time
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        None => String::new(),
    }
}
