//! Git 日志视图的左栏：分支树（HEAD / 本地 / 远程）。
//!
//! 对齐原版 New UI「Git 日志」工具窗口左栏（参考截图像素实测）：
//! - 分组行：`HEAD (当前分支)` / `本地` / `远程`，远程下再按 remote 分子组
//! - 分支行带缩进与分支图标；当前日志过滤命中的分支用实测选中底色高亮
//! - 点击分支行 = 把日志过滤到该分支；点击 HEAD 或已命中分支 = 清除过滤

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Div, Hsla, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::{Icon, Sizable, Size};
use rebased_rs::git::Branch;

use crate::ui::commit_list::short_id;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

/// 分支被点击：`Some(name)` = 过滤到该分支，`None` = 清除过滤。
pub type BranchPick = Arc<dyn Fn(Option<String>, &mut App)>;

/// 单行的渲染规格。
struct RowSpec {
    id: String,
    label: String,
    /// 缩进层级（每级 `TREE_INDENT`）。
    depth: usize,
    /// 该行是否为分组标题（弱化文字、无图标、不可点）。
    group: bool,
    /// 日志过滤是否命中该行（命中 = 选中底色）。
    hit: bool,
    /// 点击后写入过滤器的分支名；`None` = 清除过滤。
    payload: Option<String>,
}

pub(crate) fn render_branch_tree(
    branches: &[Branch],
    current: Option<&str>,
    head: Option<&str>,
    active: Option<&str>,
    query: &str,
    on_pick: Option<&BranchPick>,
    cx: &App,
) -> Div {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let query = query.trim().to_lowercase();
    let hit_query = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
    let mut tree = div().flex().flex_col().w_full().py(px(theme::SPACE_XS));

    // HEAD：点击清除过滤，回到"全部提交"。挂在分支上时显示分支名，
    // 游离 HEAD（detached）时显示短哈希——与原版一致。
    let head_label = match current {
        Some(name) => format!("HEAD ({name})"),
        None => match head {
            Some(id) => format!("HEAD ({})", short_id(id)),
            None => "HEAD".to_string(),
        },
    };
    tree = tree.child(render_row(
        RowSpec {
            id: "bt-head".to_string(),
            label: head_label,
            depth: 0,
            group: true,
            hit: active.is_none(),
            payload: None,
        },
        on_pick,
        fg,
        muted,
    ));

    // 本地分支。
    let locals: Vec<&Branch> = branches
        .iter()
        .filter(|b| !b.is_remote && hit_query(&b.name))
        .collect();
    if !locals.is_empty() {
        tree = tree.child(group_header(tr("Local", "本地"), muted));
        for branch in &locals {
            tree = tree.child(render_row(
                RowSpec {
                    id: format!("bt-local-{}", branch.name),
                    label: branch.name.clone(),
                    depth: 1,
                    group: false,
                    hit: active == Some(branch.name.as_str()),
                    payload: Some(branch.name.clone()),
                },
                on_pick,
                fg,
                muted,
            ));
        }
    }

    // 远程分支：按 remote 名分组（`origin/dev` -> 组 `origin`，行 `dev`）。
    let remotes: Vec<&Branch> = branches
        .iter()
        .filter(|b| b.is_remote && hit_query(&b.name))
        .collect();
    if !remotes.is_empty() {
        tree = tree.child(group_header(tr("Remote", "远程"), muted));
        let mut groups: Vec<(String, Vec<&Branch>)> = Vec::new();
        for branch in &remotes {
            let (remote, _) = branch
                .name
                .split_once('/')
                .unwrap_or((branch.name.as_str(), ""));
            match groups.iter_mut().find(|(name, _)| name == remote) {
                Some((_, list)) => list.push(branch),
                None => groups.push((remote.to_string(), vec![branch])),
            }
        }
        for (remote, list) in groups {
            tree = tree.child(render_row(
                RowSpec {
                    id: format!("bt-remote-{remote}"),
                    label: remote.clone(),
                    depth: 1,
                    group: true,
                    hit: false,
                    payload: None,
                },
                None,
                fg,
                muted,
            ));
            for branch in list {
                let short = branch
                    .name
                    .split_once('/')
                    .map(|(_, short)| short)
                    .unwrap_or(branch.name.as_str());
                tree = tree.child(render_row(
                    RowSpec {
                        id: format!("bt-remote-{}-{}", remote, branch.name),
                        label: short.to_string(),
                        depth: 2,
                        group: false,
                        hit: active == Some(branch.name.as_str()),
                        payload: Some(branch.name.clone()),
                    },
                    on_pick,
                    fg,
                    muted,
                ));
            }
        }
    }

    tree
}

/// 分组标题行（`本地` / `远程`）：弱化文字、不可点。
fn group_header(label: &str, muted: Hsla) -> Div {
    div()
        .flex_none()
        .h(px(theme::ROW_HEIGHT))
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(theme::SPACE_SM))
        .text_size(px(theme::font_size_meta()))
        .text_color(muted)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(label.to_string())
}

fn render_row(spec: RowSpec, on_pick: Option<&BranchPick>, fg: Hsla, muted: Hsla) -> Stateful<Div> {
    let RowSpec {
        id,
        label,
        depth,
        group,
        hit,
        payload,
    } = spec;
    let indent = theme::SPACE_SM + theme::TREE_INDENT * depth as f32;
    let text_color = if hit {
        theme::white()
    } else if group {
        muted
    } else {
        fg
    };
    let mut row = div()
        .id(id)
        .flex_none()
        .h(px(theme::ROW_HEIGHT))
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_XS))
        .pl(px(indent))
        .pr(px(theme::SPACE_SM))
        .text_size(px(theme::font_size_body()))
        .text_color(text_color)
        .whitespace_nowrap()
        .overflow_hidden();
    if !group {
        row = row.child(Icon::new(Ic::Branch).with_size(Size::Small));
    }
    row = row.child(label);
    if hit {
        row = row.bg(theme::list_row_selected());
    }
    if let Some(pick) = on_pick.filter(|_| !group) {
        let pick = Arc::clone(pick);
        // 点到已命中的分支 = 清除过滤（IntelliJ 同行为）。
        let next = if hit { None } else { payload };
        row = row
            .cursor_pointer()
            .when(!hit, |r| r.hover(|s| s.bg(theme::tree_row_hover())))
            .on_click(move |_, _, app| pick(next.clone(), app));
    }
    row
}
