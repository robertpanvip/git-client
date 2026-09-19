//! 日志详情面板（三栏布局最右栏），对齐 IDEA「Commit Details」参照截图：
//!
//! - **上方 = 变更文件树**：按目录层级聚合（单链目录折叠为 `packages/vue`
//!   形式），目录行 = 展开箭头 + 彩色文件夹图标 + `(N 个文件)` 计数，
//!   文件行 = JetBrains 官方彩色类型图标 + 文件名，点击打开该提交内 Diff；
//! - **下方 = 提交信息**：主题行（加粗 + HEAD 徽标）→ 短哈希 · 作者 · 日期
//!   → 包含分支（彩色分支名）→ 标签。
//!
//! 树渲染时实时构建（纯内存遍历），不缓存——与工作区文件树（file_tree.rs）
//! 的「渲染时摊平」策略一致，避免缓存与渲染脱节。

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px, relative,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    Icon,
};

use rebased_rs::git::{Commit, Tag};

use crate::ui::commit_list::format_full_time;
use crate::ui::components::{badge, ref_style};
use crate::ui::i18n::tr;
use crate::ui::icons::{Ic, file_type_icon, folder_icon};
use crate::ui::theme;
use crate::ui::theme::{ROW_HEIGHT, SPACE_MD, SPACE_SM, TREE_INDENT};

use super::{AppView, ConfirmAction, PromptKind};

/// 详情面板文件树的节点（按目录层级聚合）。
struct DetailNode {
    /// 展示名；目录在单链折叠后可能形如 `packages/vue`。
    name: String,
    /// 目录 = 目录完整路径；文件 = 仓库相对路径。
    path: String,
    is_dir: bool,
    /// 目录下文件总数（含各级后代）；文件恒为 1。
    file_count: usize,
    children: Vec<DetailNode>,
}

impl DetailNode {
    fn dir(name: String, path: String) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            file_count: 0,
            children: Vec::new(),
        }
    }
}

/// 把一条文件路径插入树中（`prefix` 为已走过层级的完整路径）。
///
/// 每次进入本函数即代表「当前节点子树下多了一个文件」，
/// 因此顶部无条件 `file_count += 1`，文件 / 目录两条分支都正确。
fn insert_path(node: &mut DetailNode, prefix: &str, parts: &[&str]) {
    let part = parts[0];
    let path = if prefix.is_empty() {
        part.to_string()
    } else {
        format!("{prefix}/{part}")
    };
    // 目录节点累计其子树内文件总数（供目录行的「N 个文件」计数）。
    node.file_count += 1;
    if parts.len() == 1 {
        node.children.push(DetailNode {
            name: part.to_string(),
            path,
            is_dir: false,
            file_count: 1,
            children: Vec::new(),
        });
        return;
    }
    if let Some(existing) = node
        .children
        .iter_mut()
        .find(|child| child.is_dir && child.name == part)
    {
        insert_path(existing, &path, &parts[1..]);
    } else {
        // `path` 即该目录的完整路径，直接作为子层级的 prefix。
        let mut dir = DetailNode::dir(part.to_string(), path.clone());
        insert_path(&mut dir, &path, &parts[1..]);
        node.children.push(dir);
    }
}

/// 同级排序：目录在前、文件在后，组内按名称（不区分大小写）升序。
fn sort_nodes(nodes: &mut Vec<DetailNode>) {
    nodes.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for node in nodes {
        sort_nodes(&mut node.children);
    }
}

/// 单链折叠：目录若只有一个子节点且子节点也是目录，则合并展示名
/// （`packages` + `vue` → `packages/vue`），路径取深层路径。
fn collapse_chains(node: &mut DetailNode) {
    if node.is_dir {
        while node.children.len() == 1 && node.children[0].is_dir {
            let mut child = node.children.pop().expect("len checked");
            node.name = format!("{}/{}", node.name, child.name);
            node.path = std::mem::take(&mut child.path);
            node.children = std::mem::take(&mut child.children);
        }
    }
    for child in &mut node.children {
        collapse_chains(child);
    }
}

/// 由扁平变更列表构建（已排序 + 已折叠的）文件树根层节点。
fn build_detail_tree(paths: &[String]) -> Vec<DetailNode> {
    let mut root = DetailNode::dir(String::new(), String::new());
    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        insert_path(&mut root, "", &parts);
    }
    sort_nodes(&mut root.children);
    for node in &mut root.children {
        collapse_chains(node);
    }
    root.children
}

impl AppView {
    pub(crate) fn render_detail(&self, commit: &Commit, cx: &mut Context<Self>) -> Div {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let commit_id = commit.id.0.clone();
        let is_head = self
            .state
            .head_id
            .as_ref()
            .is_some_and(|head| head == &commit_id);

        let commit_tags: Vec<Tag> = self
            .state
            .tags
            .iter()
            .filter(|tag| tag.commit_id == commit_id)
            .cloned()
            .collect();

        // ── 上：变更文件树 ────────────────────────────────────────────
        let paths: Vec<String> = self
            .state
            .detail_files
            .iter()
            .map(|change| change.path.clone())
            .collect();
        let tree_nodes = build_detail_tree(&paths);
        let tree_rows = self.render_detail_tree_nodes(&tree_nodes, 0, &commit_id, cx);

        let files_section = if tree_rows.is_empty() {
            div()
                .id("detail-files-empty")
                .flex_none()
                .text_size(px(theme::font_size_meta()))
                .text_color(muted)
                .child(tr("No file changes", "无文件变更"))
        } else {
            div()
                .id("detail-files-tree")
                .flex_none()
                .min_h_0()
                .max_h(relative(0.55))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .py(px(SPACE_SM))
                .children(tree_rows)
        };

        // ── 下：提交信息 ──────────────────────────────────────────────
        let short_id = &commit.id.0[..commit.id.0.len().min(7)];
        let author_line = format!(
            "{} {} {} <{}>, {}",
            short_id,
            tr("by", "作者"),
            commit.author.name,
            commit.author.email,
            format_full_time(commit.time),
        );

        let mut message = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(SPACE_MD))
            .pt(px(SPACE_MD * 2.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_wrap()
                    .gap(px(SPACE_SM))
                    .child(
                        div()
                            .text_size(px(theme::font_size_body()))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(fg)
                            .child(commit.subject.clone()),
                    )
                    .when(is_head, |row| {
                        row.child(badge(
                            SharedString::from("detail-head"),
                            "HEAD",
                            theme::head_color(),
                        ))
                    })
                    .child(div().flex_1().min_w_0())
                    .child(
                        Button::new("close-detail")
                            .ghost()
                            .compact()
                            .icon(Ic::Close)
                            .on_click(cx.listener(|this, _, _, cx| this.clear_detail(cx))),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(muted)
                    .child(author_line),
            );

        if !commit.body.is_empty() {
            message = message.child(
                div()
                    .flex_none()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(muted)
                    .whitespace_normal()
                    .child(commit.body.clone()),
            );
        }

        if !self.state.detail_branches.is_empty() {
            let count = self.state.detail_branches.len();
            // 参照截图「在 3 个分支: HEAD, main, origin/main」：标签灰、分支名彩色。
            let mut branch_line = div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(SPACE_SM))
                .flex_none()
                .text_size(px(theme::font_size_meta()))
                .child(
                    div()
                        .text_color(muted)
                        .child(format!(
                            "{} {} {}: ",
                            tr("In", "在"),
                            count,
                            tr("branches", "个分支")
                        )),
                );
            for (index, branch) in self.state.detail_branches.iter().enumerate() {
                let (_, color) = ref_style(branch);
                let label = if index + 1 == count {
                    branch.clone()
                } else {
                    format!("{branch},")
                };
                branch_line = branch_line.child(
                    div().text_color(color).child(label),
                );
            }
            message = message.child(branch_line);
        }

        if !commit_tags.is_empty() {
            message = message.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(SPACE_SM))
                    .flex_none()
                    .children(commit_tags.into_iter().map(|tag| {
                        let name = tag.name;
                        badge(
                            SharedString::from(format!("detail-tag-{name}")),
                            name.clone(),
                            theme::tag_color(),
                        )
                        .child(
                            Button::new(format!("delete-tag-{name}"))
                                .ghost()
                                .compact()
                                .icon(Ic::Delete)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.open_prompt(
                                        PromptKind::Confirm(ConfirmAction::DeleteTag {
                                            name: name.clone(),
                                        }),
                                        cx,
                                    );
                                })),
                        )
                    })),
            );
        }

        // 整体：上=文件树（占上半区、可滚动），下=提交信息，其余留白沉底。
        div()
            .flex()
            .flex_col()
            .h_full()
            .min_h_0()
            .px(px(SPACE_MD))
            .py(px(SPACE_SM))
            .child(files_section)
            .child(message)
            .child(div().flex_1().min_h_0())
    }

    /// 递归渲染详情文件树节点（目录行可折叠，文件行点击打开 Diff）。
    fn render_detail_tree_nodes(
        &self,
        nodes: &[DetailNode],
        depth: usize,
        commit_id: &str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let mut rows: Vec<AnyElement> = Vec::new();
        for node in nodes {
            let indent = SPACE_SM + TREE_INDENT * depth as f32;
            if node.is_dir {
                let collapsed = self.state.detail_tree_collapsed.contains(&node.path);
                let dir_path = node.path.clone();
                let row = div()
                    .id(format!("detail-dir-{}", node.path))
                    .flex_none()
                    .h(px(ROW_HEIGHT))
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(SPACE_SM))
                    .pl(px(indent))
                    .pr(px(SPACE_SM))
                    .text_size(px(theme::font_size_meta()))
                    .text_color(fg)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .cursor_pointer()
                    .hover(|style| style.bg(theme::tree_row_hover()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.state.detail_tree_collapsed.remove(&dir_path) {
                            this.state
                                .detail_tree_collapsed
                                .insert(dir_path.clone());
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(TREE_INDENT))
                            .flex_none()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(if collapsed {
                                Ic::ChevronRight
                            } else {
                                Ic::ChevronDown
                            })),
                    )
                    .child(folder_icon())
                    .child(node.name.clone())
                    .child(
                        div()
                            .flex_none()
                            .text_color(muted)
                            .child(format!(
                                "{} {}",
                                node.file_count,
                                if node.file_count == 1 {
                                    tr("file", "个文件")
                                } else {
                                    tr("files", "个文件")
                                }
                            )),
                    );
                // 目录行 + 子行收进一个纵向容器：目录行本身是 flex_row，
                // 子行若直接 append 会水平排开（已踩坑）。
                let mut container = div().flex().flex_col().flex_none().w_full().child(row);
                if !collapsed {
                    let child_rows =
                        self.render_detail_tree_nodes(&node.children, depth + 1, commit_id, cx);
                    for child in child_rows {
                        container = container.child(child);
                    }
                }
                rows.push(container.into_any_element());
            } else {
                let file_path = node.path.clone();
                let diff_path = node.path.clone();
                let file_commit_id = commit_id.to_string();
                rows.push(
                    div()
                        .id(format!("detail-file-{}", node.path))
                        .flex_none()
                        .h(px(ROW_HEIGHT))
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(SPACE_SM))
                        .pl(px(indent + TREE_INDENT))
                        .pr(px(SPACE_SM))
                        .text_size(px(theme::font_size_meta()))
                        .text_color(theme::file_tree_file_fg(fg))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme::tree_row_hover()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_commit_diff(file_commit_id.clone(), Some(diff_path.clone()), cx)
                        }))
                        .child(file_type_icon(&file_path))
                        .child(node.name.clone())
                        .into_any_element(),
                );
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn builds_nested_tree_with_counts() {
        let tree = build_detail_tree(&paths(&[
            "packages/vue/react-dom/client.ts",
            "packages/vue/react-dom/index.ts",
            "packages/vue/index.ts",
            "utils.ts",
        ]));
        assert_eq!(tree.len(), 2);
        assert!(tree[0].is_dir);
        // 根层同样应用单链折叠：packages 下只有 vue 一个子目录 → "packages/vue"。
        assert_eq!(tree[0].name, "packages/vue");
        assert_eq!(tree[0].file_count, 3);
        assert_eq!(tree[1].name, "utils.ts");
        assert!(!tree[1].is_dir);
    }

    #[test]
    fn collapses_single_dir_chains() {
        let tree = build_detail_tree(&paths(&[
            "packages/vue/react-dom/client.ts",
            "packages/vue/react-dom/index.ts",
            "packages/vue/index.ts",
        ]));
        // packages 下仅 vue 一个子目录 → 折叠为 "packages/vue"。
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "packages/vue");
        assert_eq!(tree[0].path, "packages/vue");
        assert_eq!(tree[0].file_count, 3);
        // vue 下有 react-dom 目录 + index.ts 文件 → 不再继续折叠。
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].name, "react-dom");
        assert_eq!(tree[0].children[0].path, "packages/vue/react-dom");
    }

    #[test]
    fn dirs_sort_before_files_and_names_case_insensitive() {
        let tree = build_detail_tree(&paths(&["b.txt", "A/inner.txt", "a.txt"]));
        let names: Vec<&str> = tree.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, vec!["A", "a.txt", "b.txt"]);
    }

    #[test]
    fn file_at_root_has_no_children() {
        let tree = build_detail_tree(&paths(&["README.md"]));
        assert_eq!(tree.len(), 1);
        assert!(!tree[0].is_dir);
        assert_eq!(tree[0].path, "README.md");
    }
}
