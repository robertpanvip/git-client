//! 文件视图左栏：工作区文件夹树（文件夹 + 文件）。
//!
//! 对齐 IntelliJ「项目」工具窗口的经典树形呈现：
//! - 目录行 = 展开箭头 + IDEA 文件夹图标，点击切换展开；
//! - 文件行 = IDEA 文本文件图标，点击在右栏打开；
//! - 缩进按层级递增，选中文件用统一的列表选中底色高亮。
//!
//! 折叠状态下的后代节点整体不出现（[`flatten_tree`] 负责摊平）。
//! 摊平在渲染时现算（纯内存遍历，微秒级），不引入缓存——
//! 缓存方案曾因与渲染脱节导致整棵树显示为空。

use std::collections::HashSet;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Div, Hsla, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::{Icon, Sizable, Size};

use crate::ui::icons::Ic;
use crate::ui::theme;

/// 树行的点击语义。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TreeClick {
    /// 切换目录展开状态。
    Dir(String),
    /// 打开文件。
    File(String),
}

pub type TreePick = Arc<dyn Fn(TreeClick, &mut App)>;

/// 一条可见的树行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TreeRow {
    /// 仓库相对路径（目录行即目录路径）。
    pub path: String,
    /// 行末显示的名字（不含父目录）。
    pub name: String,
    /// 缩进层级（顶层 = 0）。
    pub depth: usize,
    pub is_dir: bool,
    /// 目录行是否已展开（文件行恒为 false）。
    pub expanded: bool,
}

/// 把扁平路径列表摊平成可见行。
///
/// `files` 必须是按 IDEA 项目树顺序排好的列表（目录段优先于文件段、
/// 同级按名称升序，由 `git::Repository::worktree_files` 排序得到）：
/// 该顺序保证父目录总在子节点之前出现。`expanded` 之外的目录视为折叠，
/// 其整棵子树都不产生行。
pub(crate) fn flatten_tree(files: &[String], expanded: &HashSet<String>) -> Vec<TreeRow> {
    let mut rows: Vec<TreeRow> = Vec::new();
    let mut seen_dirs: HashSet<String> = HashSet::new();

    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        let mut prefix = String::new();
        let mut hidden = false;
        for (index, part) in parts.iter().enumerate() {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);

            if index + 1 == parts.len() {
                if !hidden {
                    rows.push(TreeRow {
                        path: file.clone(),
                        name: (*part).to_string(),
                        depth: index,
                        is_dir: false,
                        expanded: false,
                    });
                }
                break;
            }

            let is_expanded = expanded.contains(&prefix);
            let first_sight = seen_dirs.insert(prefix.clone());
            if first_sight && !hidden {
                rows.push(TreeRow {
                    path: prefix.clone(),
                    name: (*part).to_string(),
                    depth: index,
                    is_dir: true,
                    expanded: is_expanded,
                });
            }
            if !is_expanded {
                hidden = true;
            }
        }
    }

    rows
}

/// 渲染文件夹树（渲染时实时摊平，与 v0.6.0 一致）。
pub(crate) fn render_file_tree(
    files: &[String],
    expanded: &HashSet<String>,
    selected: Option<&str>,
    on_pick: &TreePick,
    cx: &App,
) -> Div {
    let fg = cx.theme().foreground;
    let mut tree = div().flex().flex_col().w_full().py(px(theme::SPACE_XS));
    for row in flatten_tree(files, expanded) {
        tree = tree.child(render_row(row, selected, on_pick, fg));
    }
    tree
}

fn render_row(row: TreeRow, selected: Option<&str>, on_pick: &TreePick, fg: Hsla) -> Stateful<Div> {
    let indent = theme::SPACE_SM + theme::TREE_INDENT * row.depth as f32;
    let is_selected = !row.is_dir && selected == Some(row.path.as_str());
    let text_color = if is_selected {
        theme::white()
    } else if row.is_dir {
        fg
    } else {
        theme::file_tree_file_fg(fg)
    };
    // IDEA 项目树：目录用文件夹图标，文件按扩展名显示对应类型图标。
    let icon = if row.is_dir {
        Ic::Folder
    } else {
        crate::ui::icons::file_icon(&row.path)
    };
    let click = if row.is_dir {
        TreeClick::Dir(row.path.clone())
    } else {
        TreeClick::File(row.path.clone())
    };
    let pick = Arc::clone(on_pick);

    let mut element = div()
        .id(format!("ft-{}", row.path))
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
        .overflow_hidden()
        .cursor_pointer()
        .child(
            // 展开箭头与 IDEA 项目树一致：展开时朝下、折叠时朝右。
            div()
                .w(px(theme::ICON_BUTTON_SIZE))
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .when(row.is_dir, |arrow| {
                    arrow.child(Icon::new(if row.expanded {
                        Ic::ChevronDown
                    } else {
                        Ic::ChevronRight
                    }))
                }),
        )
        .child(Icon::new(icon).with_size(Size::Small))
        .child(row.name.clone());
    if is_selected {
        element = element.bg(theme::list_row_selected());
    } else {
        element = element.hover(|style| style.bg(theme::tree_row_hover()));
    }
    element.on_click(move |_, _, app| pick(click.clone(), app))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| path.to_string()).collect()
    }

    fn expanded(dirs: &[&str]) -> HashSet<String> {
        dirs.iter().map(|dir| dir.to_string()).collect()
    }

    fn labels(rows: &[TreeRow]) -> Vec<(usize, &str, bool)> {
        rows.iter()
            .map(|row| (row.depth, row.name.as_str(), row.is_dir))
            .collect()
    }

    #[test]
    fn collapsed_dirs_hide_their_subtree() {
        let rows = flatten_tree(
            &files(&["README.md", "src/main.rs", "src/ui/mod.rs"]),
            &expanded(&[]),
        );
        assert_eq!(
            labels(&rows),
            vec![(0, "README.md", false), (0, "src", true)]
        );
    }

    #[test]
    fn expanded_dir_reveals_children_once() {
        let rows = flatten_tree(
            &files(&["README.md", "src/main.rs", "src/ui/mod.rs"]),
            &expanded(&["src"]),
        );
        assert_eq!(
            labels(&rows),
            vec![
                (0, "README.md", false),
                (0, "src", true),
                (1, "main.rs", false),
                (1, "ui", true),
            ]
        );
    }

    #[test]
    fn nested_expansion_emits_deep_rows() {
        let rows = flatten_tree(&files(&["src/ui/mod.rs"]), &expanded(&["src", "src/ui"]));
        assert_eq!(
            labels(&rows),
            vec![(0, "src", true), (1, "ui", true), (2, "mod.rs", false)]
        );
        assert_eq!(rows[2].path, "src/ui/mod.rs");
    }

    #[test]
    fn file_path_is_repo_relative() {
        let rows = flatten_tree(&files(&["a/b/c.txt"]), &expanded(&["a", "a/b"]));
        assert_eq!(rows[2].path, "a/b/c.txt");
        assert!(!rows[2].is_dir);
    }
}
