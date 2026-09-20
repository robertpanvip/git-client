//! Git 日志视图的左栏：分支树（HEAD / 本地 / 远程）。
//!
//! 对齐原版 New UI「Git 日志」工具窗口左栏（参考截图像素实测）：
//! - 分组行：`HEAD (当前分支)` / `本地` / `远程`，远程下再按 remote 分子组
//! - 分支行带缩进与分支图标；当前日志过滤命中的分支用实测选中底色高亮
//! - 点击分支行 = 把日志过滤到该分支；点击 HEAD 或已命中分支 = 清除过滤

use std::collections::HashSet;
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

/// 分组折叠开关被点击：`key` = "local" / "remote"。
pub type GroupToggle = Arc<dyn Fn(&str, &mut App)>;

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
    /// 分组标题（group=true）是否仍可点击：仅 HEAD 行为 `true`，
    /// 用于「点击 HEAD 清除过滤」，与文档承诺一致。
    pickable: bool,
}

pub(crate) fn render_branch_tree(
    branches: &[Branch],
    current: Option<&str>,
    head: Option<&str>,
    active: Option<&str>,
    query: &str,
    collapsed: &HashSet<String>,
    on_pick: Option<&BranchPick>,
    on_toggle_group: Option<&GroupToggle>,
    cx: &App,
) -> Div {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let query = query.trim().to_lowercase();
    let hit_query = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
    // 命中高亮：过滤激活时命中过滤分支；无过滤时高亮**当前分支**
    //（IDEA 语义：日志默认展示当前分支，当前分支行始终带选中底色）。
    let is_hit = |name: &str| match active {
        Some(active) => active == name,
        None => current == Some(name),
    };
    let mut tree = div().flex().flex_col().w_full().py(px(theme::SPACE_XS));

    // HEAD：点击清除过滤，回到"全部提交"。挂在分支上时显示「当前分支」
    //（对齐原版「HEAD(当前分支)」文案），游离 HEAD（detached）时显示短哈希。
    let head_label = match current {
        Some(_) => tr("HEAD (current branch)", "HEAD(当前分支)").to_string(),
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
            // HEAD 行可点击：点击后清除分支过滤（与文档承诺一致，Bug #6）。
            pickable: true,
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
        tree = tree.child(collapsible_group_header(
            "bt-group-local",
            tr("Local", "本地"),
            "local",
            collapsed.contains("local"),
            on_toggle_group,
            muted,
        ));
        if !collapsed.contains("local") {
            for branch in &locals {
                tree = tree.child(render_row(
                    RowSpec {
                        id: format!("bt-local-{}", branch.name),
                        label: branch.name.clone(),
                        depth: 1,
                        group: false,
                        hit: is_hit(&branch.name),
                        payload: Some(branch.name.clone()),
                        pickable: false,
                    },
                    on_pick,
                    fg,
                    muted,
                ));
            }
        }
    }

    // 远程分支：按 remote 名分组（`origin/dev` -> 组 `origin`，行 `dev`）。
    let remotes: Vec<&Branch> = branches
        .iter()
        .filter(|b| b.is_remote && hit_query(&b.name))
        .collect();
    if !remotes.is_empty() {
        tree = tree.child(collapsible_group_header(
            "bt-group-remote",
            tr("Remote", "远程"),
            "remote",
            collapsed.contains("remote"),
            on_toggle_group,
            muted,
        ));
        if !collapsed.contains("remote") {
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
                        pickable: false,
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
                            hit: is_hit(&branch.name),
                            payload: Some(branch.name.clone()),
                            pickable: false,
                        },
                        on_pick,
                        fg,
                        muted,
                    ));
                }
            }
        }
    }

    tree
}

/// 可折叠分组标题行（`本地` / `远程`）：chevron + 弱化文字，整行点击折叠/展开
///（对齐 IDEA 分支树分组头的折叠交互与视觉）。
fn collapsible_group_header(
    id: &'static str,
    label: &str,
    key: &'static str,
    is_collapsed: bool,
    on_toggle: Option<&GroupToggle>,
    muted: Hsla,
) -> Stateful<Div> {
    let mut header = div()
        .id(id)
        .flex_none()
        .h(px(theme::ROW_HEIGHT))
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_XS))
        .pl(px(theme::SPACE_SM))
        .text_size(px(theme::font_size_meta()))
        .text_color(muted)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(
            if is_collapsed {
                Icon::new(Ic::ChevronRight)
            } else {
                Icon::new(Ic::ChevronDown)
            }
            .with_size(Size::XSmall)
            .flex_none(),
        )
        .child(label.to_string());
    if let Some(toggle) = on_toggle {
        let toggle = Arc::clone(toggle);
        header = header
            .cursor_pointer()
            .hover(move |s| s.bg(theme::tree_row_hover()))
            .on_click(move |_, _, app| toggle(key, app));
    }
    header
}

fn render_row(spec: RowSpec, on_pick: Option<&BranchPick>, fg: Hsla, muted: Hsla) -> Stateful<Div> {
    let RowSpec {
        id,
        label,
        depth,
        group,
        hit,
        payload,
        pickable,
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
    if let Some(pick) = on_pick.filter(|_| !group || pickable) {
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
