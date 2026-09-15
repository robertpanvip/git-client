use std::sync::Arc;

use chrono::{DateTime, Local};
use gpui_kit::base::IndexPath;
use gpui_kit::component::{
    list::{ListDelegate, ListItem, ListState},
    ActiveTheme,
};
use gpui::{div, hsla, px, App, Context, Div, IntoElement, ParentElement, Styled, Task, Window};
use rebased_rs::git::{build_graph, filter_commits, Commit, Graph};

use crate::ui::graph_view::{lane_canvas, lane_color, ROW_HEIGHT};

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
}

impl LogDelegate {
    pub fn new() -> Self {
        Self {
            data: None,
            visible: None,
            selected: None,
        }
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
        self.visible
            .as_ref()
            .map_or(0, |data| data.commits.len())
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

        let mut row = div()
            .h(px(ROW_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .overflow_hidden();

        let graph_row = data.graph.rows.get(ix.row).cloned();
        row = row.child(lane_canvas(graph_row, data.graph.lane_count));

        for ref_name in &commit.refs {
            row = row.child(ref_badge(ref_name));
        }

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
                    .max_w(px(110.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_xs()
                    .text_color(muted)
                    .child(commit.author.name.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(muted)
                    .child(format_time(commit.time)),
            );

        Some(ListItem::new(ix.row).selected(selected).px_2().child(row))
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground.opacity(0.6))
            .child("No commits")
    }
}

fn ref_badge(name: &str) -> Div {
    let (label, color) = if let Some(tag) = name.strip_prefix("tag: ") {
        (tag.to_string(), lane_color(2))
    } else if let Some(branch) = name.strip_prefix("HEAD -> ") {
        (branch.to_string(), lane_color(0))
    } else if name == "HEAD" {
        ("HEAD".to_string(), lane_color(0))
    } else {
        (name.to_string(), lane_color(5))
    };

    div()
        .flex_none()
        .px_1()
        .rounded(px(4.))
        .bg(hsla(color.h, color.s, color.l, 0.18))
        .text_xs()
        .text_color(color)
        .child(label)
}

pub(crate) fn format_time(secs: i64) -> String {
    match DateTime::from_timestamp(secs, 0) {
        Some(time) => time
            .with_timezone(&Local)
            .format("%m-%d %H:%M")
            .to_string(),
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
