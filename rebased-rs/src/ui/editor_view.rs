//! 文件视图右栏：代码区域（行号 gutter + 行变更标记 + 行级 blame）。
//!
//! 对齐 IntelliJ 编辑器「Annotate」的经典形式：
//! - **行首窄条**标记该行相对 HEAD 的变更类型（新增 / 修改 / 删除）；
//! - **blame 列**显示每行的最近提交（时间 + 作者，按提交着色），点击跳到该提交；
//! - 其后是行号 gutter 与等宽正文。
//!
//! 逐行数据在打开文件时构建一次（[`build_content`]），渲染走 `uniform_list`
//! 只画可视区——大文件不会因为「每帧构建上万行元素」而卡顿。

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    App, AnyElement, Div, Hsla, InteractiveElement, IntoElement, ListHorizontalSizingBehavior,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, UniformList,
    UniformListScrollHandle, div, px, uniform_list,
};
use gpui_kit::component::ActiveTheme;
use rebased_rs::git::{BlameGroup, DiffLineKind, FileDiff};

use crate::ui::blame_view::{BlameJump, BlameToggle};
use crate::ui::commit_list::format_time;
use crate::ui::components::{menu_item, menu_width};
use crate::ui::graph_view::lane_color;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;
use gpui_kit::component::menu::ContextMenuExt;

/// 工作区某一行相对 HEAD 的变更类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineChange {
    /// 工作区新增的行。
    Added,
    /// 与 HEAD 相比内容改变的行。
    Modified,
    /// 该行附近有被删除的行（工作区已无对应行，标记挂在相邻行上）。
    Deleted,
}

/// 单行的 blame 信息（只显示提交时间与作者，不显示哈希）。
pub(crate) struct EditorBlame {
    pub(crate) author: String,
    pub(crate) time: String,
    /// 可跳转的提交 id；未提交的行为 `None`。
    pub(crate) commit_id: Option<String>,
    /// 按提交着色的色号（同一提交同色）。
    pub(crate) color_index: usize,
}

/// 编辑器的单行渲染数据。
pub(crate) struct EditorLine {
    pub(crate) number: u32,
    pub(crate) text: String,
    pub(crate) change: Option<LineChange>,
    pub(crate) blame: Option<EditorBlame>,
}

/// 编辑器内容：逐行数据 + 最长行字符数（决定横向滚动宽度）。
#[derive(Default)]
pub(crate) struct EditorContent {
    pub(crate) lines: Vec<EditorLine>,
    pub(crate) max_chars: usize,
}

/// 构建编辑器逐行数据（内容 + 相对 HEAD 的 diff + 工作区 blame）。
pub(crate) fn build_content(
    content: &str,
    diff: &[FileDiff],
    blame: &[BlameGroup],
) -> EditorContent {
    let changes = line_changes(diff);
    let index = blame_index(blame);
    let mut lines = Vec::new();
    let mut max_chars = 0;
    for (offset, text) in content.lines().enumerate() {
        let number = offset as u32 + 1;
        max_chars = max_chars.max(text.chars().count());
        let blame = match index.get(&number) {
            Some(slot) => blame.get(*slot).map(|group| editor_blame(group, *slot)),
            None => None,
        };
        lines.push(EditorLine {
            number,
            text: text.to_string(),
            change: changes.get(&number).copied(),
            blame,
        });
    }
    EditorContent { lines, max_chars }
}

fn editor_blame(group: &BlameGroup, color_index: usize) -> EditorBlame {
    // 未提交的行由 git 标记为全 0 commit：没有可跳转的提交，只显示「未提交」。
    if group.commit_id.chars().all(|c| c == '0') {
        return EditorBlame {
            author: tr("Not committed", "未提交").to_string(),
            time: String::new(),
            commit_id: None,
            color_index,
        };
    }
    EditorBlame {
        author: group.author.clone(),
        time: format_time(group.time),
        commit_id: Some(group.commit_id.clone()),
        color_index,
    }
}

/// 由文件 diff 计算「工作区行号 -> 变更类型」。
///
/// 一个 hunk 内连续的删除段与紧随的添加段按序配对：配上的算 `Modified`，
/// 多出的添加行算 `Added`；多出的删除行不占工作区行号，标记挂到其后的
/// 第一行（文件末尾则挂到最后一行）。
pub(crate) fn line_changes(files: &[FileDiff]) -> HashMap<u32, LineChange> {
    let mut map: HashMap<u32, LineChange> = HashMap::new();
    for file in files {
        for hunk in &file.hunks {
            let lines = &hunk.lines;
            let mut index = 0;
            while index < lines.len() {
                match lines[index].kind {
                    DiffLineKind::Context | DiffLineKind::HunkHeader => index += 1,
                    DiffLineKind::Deleted => {
                        let del_start = index;
                        while index < lines.len() && lines[index].kind == DiffLineKind::Deleted {
                            index += 1;
                        }
                        let mut add_end = index;
                        while add_end < lines.len() && lines[add_end].kind == DiffLineKind::Added {
                            add_end += 1;
                        }
                        let dels = &lines[del_start..index];
                        let adds = &lines[index..add_end];
                        for (offset, added) in adds.iter().enumerate() {
                            if let Some(no) = added.new_no {
                                let kind = if offset < dels.len() {
                                    LineChange::Modified
                                } else {
                                    LineChange::Added
                                };
                                map.insert(no, kind);
                            }
                        }
                        if dels.len() > adds.len() {
                            // 多出的删除行：标记挂到删除位置之后的第一行（文件末尾
                            // 则回退到最后一行），已有变更标记的行不覆盖。
                            let anchor = lines
                                .get(add_end)
                                .and_then(|line| line.new_no)
                                .or_else(|| lines[..del_start].iter().rev().find_map(|l| l.new_no))
                                .unwrap_or(1);
                            map.entry(anchor).or_insert(LineChange::Deleted);
                        }
                        index = add_end;
                    }
                    DiffLineKind::Added => {
                        while index < lines.len() && lines[index].kind == DiffLineKind::Added {
                            if let Some(no) = lines[index].new_no {
                                map.insert(no, LineChange::Added);
                            }
                            index += 1;
                        }
                    }
                }
            }
        }
    }
    map
}

/// 由 blame 分组建立「行号 -> 分组下标」，供按提交着色。
pub(crate) fn blame_index(groups: &[BlameGroup]) -> HashMap<u32, usize> {
    let mut map = HashMap::new();
    for (index, group) in groups.iter().enumerate() {
        for line in &group.lines {
            map.insert(line.number, index);
        }
    }
    map
}

/// 渲染代码区域（`uniform_list` 只构建可视区的行）。
///
/// `toggle_blame` 提供右键菜单的注解开关联调：开启时菜单显示「关闭注解」，
/// 关闭时显示「使用 Git 追溯注解」（对齐 IDEA 的 gutter 右键行为）。
pub(crate) fn render_editor(
    content: &Arc<EditorContent>,
    scroll: &UniformListScrollHandle,
    on_commit: Option<&BlameJump>,
    blame_enabled: bool,
    toggle_blame: Option<&BlameToggle>,
    cx: &App,
) -> UniformList {
    let mono = cx.theme().mono_font_family.clone();
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let code_width = (content.max_chars.max(1) as f32) * theme::diff_char_width() + theme::SPACE_LG;
    let row_min_w = theme::EDITOR_CHANGE_BAR_WIDTH
        + theme::EDITOR_BLAME_WIDTH
        + theme::DIFF_GUTTER_COLUMN_WIDTH
        + code_width;
    let content = Arc::clone(content);
    let on_commit = on_commit.cloned();
    let toggle_blame = toggle_blame.cloned();

    uniform_list(
        "editor-lines",
        content.lines.len(),
        move |range: Range<usize>, _window, _cx| {
            range
                .map(|index| {
                    render_line(
                        &content.lines[index],
                        &mono,
                        fg,
                        muted,
                        row_min_w,
                        on_commit.as_ref(),
                        blame_enabled,
                        toggle_blame.as_ref(),
                    )
                })
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(scroll)
    // uniform_list 默认样式只有 overflow，不约束尺寸：必须显式撑满父容器
    // 剩余空间，否则内容为空（如切换文件的瞬间）时高度塌陷、右栏一片空白。
    .flex_1()
    // 长行超出视口时整表横向滚动（与 diff 视图同策略）。
    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
}

#[allow(clippy::too_many_arguments)]
fn render_line(
    line: &EditorLine,
    mono: &SharedString,
    fg: Hsla,
    muted: Hsla,
    row_min_w: f32,
    on_commit: Option<&BlameJump>,
    blame_enabled: bool,
    toggle_blame: Option<&BlameToggle>,
) -> AnyElement {
    let marker_bg = match line.change {
        Some(LineChange::Added) => theme::added_color(),
        Some(LineChange::Modified) => theme::modified_color(),
        Some(LineChange::Deleted) => theme::deleted_color(),
        None => theme::transparent(),
    };
    let row_bg = match line.change {
        Some(LineChange::Added) => theme::added_line_bg(),
        Some(LineChange::Modified) => theme::modified_line_bg(),
        _ => theme::transparent(),
    };

    let (blame_bg, author_fg, author, time, commit_id) = match &line.blame {
        Some(blame) => (
            theme::badge_bg(lane_color(blame.color_index)),
            lane_color(blame.color_index),
            blame.author.clone(),
            blame.time.clone(),
            blame.commit_id.clone(),
        ),
        None => (
            theme::transparent(),
            muted.opacity(0.5),
            String::new(),
            String::new(),
            None,
        ),
    };

    let mut blame_cell: Stateful<Div> = div()
        .id(("ed-blame", line.number as usize))
        .w(px(theme::EDITOR_BLAME_WIDTH))
        .h_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_SM))
        .px(px(theme::SPACE_SM))
        .overflow_hidden()
        .whitespace_nowrap()
        .bg(blame_bg)
        .text_size(px(theme::font_size_mono()))
        .font_family(mono.clone())
        .child(div().flex_none().text_color(author_fg).child(author))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_color(muted.opacity(0.8))
                .child(time),
        );
    if let (Some(id), Some(jump)) = (commit_id, on_commit) {
        let jump = Arc::clone(jump);
        blame_cell = blame_cell
            .cursor_pointer()
            .on_click(move |_, _, app| jump(id.clone(), app));
    }

    let row = div()
        .flex_none()
        .h(px(theme::diff_line_height()))
        .min_w(px(row_min_w))
        .flex()
        .flex_row()
        .items_center()
        .bg(row_bg)
        .child(
            div()
                .w(px(theme::EDITOR_CHANGE_BAR_WIDTH))
                .h_full()
                .flex_none()
                .bg(marker_bg),
        )
        .child(blame_cell)
        .child(
            div()
                .w(px(theme::DIFF_GUTTER_COLUMN_WIDTH))
                .h_full()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .pr(px(theme::SPACE_XS))
                .bg(theme::gutter_bg())
                .border_r_1()
                .border_color(theme::gutter_border())
                .text_size(px(theme::font_size_mono()))
                .text_color(muted.opacity(0.7))
                .font_family(mono.clone())
                .child(line.number.to_string()),
        )
        .child(
            div()
                .h_full()
                .min_w(px(code_width_of(line)))
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .pl(px(theme::SPACE_SM))
                .text_size(px(theme::font_size_mono()))
                .text_color(fg)
                .font_family(mono.clone())
                .child(if line.text.is_empty() {
                    " ".to_string()
                } else {
                    line.text.clone()
                }),
        );

    let Some(toggle) = toggle_blame else {
        return row.into_any_element();
    };
    let toggle = Arc::clone(toggle);
    row.context_menu(move |menu, _window, _app| {
        let toggle = Arc::clone(&toggle);
        let m = if blame_enabled {
            menu.item(menu_item(
                Ic::Close,
                tr("Close Annotations", "关闭注解"),
                None,
                false,
                false,
                move |_, _, app| toggle(false, app),
            ))
        } else {
            menu.item(menu_item(
                Ic::Blame,
                tr("Annotate with Git", "使用 Git 追溯注解"),
                None,
                false,
                false,
                move |_, _, app| toggle(true, app),
            ))
        };
        menu_width(m)
    })
    .into_any_element()
}

/// 单行正文的最小宽度（按字符数估算，空行也保留一格）。
fn code_width_of(line: &EditorLine) -> f32 {
    (line.text.chars().count().max(1) as f32) * theme::diff_char_width() + theme::SPACE_LG
}

#[cfg(test)]
mod tests {
    use rebased_rs::git::{DiffLine, Hunk};

    use super::*;

    fn line(kind: DiffLineKind, old: Option<u32>, new: Option<u32>, content: &str) -> DiffLine {
        DiffLine {
            kind,
            old_no: old,
            new_no: new,
            content: content.to_string(),
        }
    }

    fn file(hunks: Vec<Hunk>) -> FileDiff {
        FileDiff {
            path: "a.txt".to_string(),
            old_path: None,
            is_new: false,
            is_deleted: false,
            is_binary: false,
            hunks,
        }
    }

    #[test]
    fn paired_delete_add_is_modified() {
        let changes = line_changes(&[file(vec![Hunk {
            header: "@@ -1,2 +1,2 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "old"),
                line(DiffLineKind::Added, None, Some(1), "new"),
                line(DiffLineKind::Context, Some(2), Some(2), "tail"),
            ],
        }])]);
        assert_eq!(changes.get(&1), Some(&LineChange::Modified));
        assert_eq!(changes.get(&2), None);
    }

    #[test]
    fn extra_added_lines_are_added() {
        let changes = line_changes(&[file(vec![Hunk {
            header: "@@ -1,1 +1,3 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "old"),
                line(DiffLineKind::Added, None, Some(1), "new"),
                line(DiffLineKind::Added, None, Some(2), "extra"),
            ],
        }])]);
        assert_eq!(changes.get(&1), Some(&LineChange::Modified));
        assert_eq!(changes.get(&2), Some(&LineChange::Added));
    }

    #[test]
    fn pure_addition_is_added() {
        let changes = line_changes(&[file(vec![Hunk {
            header: "@@ -1,1 +1,2 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Context, Some(1), Some(1), "head"),
                line(DiffLineKind::Added, None, Some(2), "new"),
            ],
        }])]);
        assert_eq!(changes.get(&2), Some(&LineChange::Added));
        assert_eq!(changes.get(&1), None);
    }

    #[test]
    fn deletion_at_end_anchors_on_last_line() {
        let changes = line_changes(&[file(vec![Hunk {
            header: "@@ -1,2 +1,1 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Context, Some(1), Some(1), "head"),
                line(DiffLineKind::Deleted, Some(2), None, "gone"),
            ],
        }])]);
        assert_eq!(changes.get(&1), Some(&LineChange::Deleted));
    }

    #[test]
    fn blame_index_maps_line_numbers_to_groups() {
        let groups = vec![
            BlameGroup {
                commit_id: "a".repeat(40),
                author: "Alice".to_string(),
                time: 1,
                filename: "a.txt".to_string(),
                lines: vec![
                    rebased_rs::git::BlameLine {
                        number: 1,
                        content: "one".to_string(),
                    },
                    rebased_rs::git::BlameLine {
                        number: 2,
                        content: "two".to_string(),
                    },
                ],
            },
            BlameGroup {
                commit_id: "b".repeat(40),
                author: "Bob".to_string(),
                time: 2,
                filename: "a.txt".to_string(),
                lines: vec![rebased_rs::git::BlameLine {
                    number: 3,
                    content: "three".to_string(),
                }],
            },
        ];
        let index = blame_index(&groups);
        assert_eq!(index.get(&1), Some(&0));
        assert_eq!(index.get(&2), Some(&0));
        assert_eq!(index.get(&3), Some(&1));
        assert_eq!(index.get(&4), None);
    }

    #[test]
    fn build_content_attaches_changes_and_blame_without_hash() {
        let diff = vec![file(vec![Hunk {
            header: "@@ -1,2 +1,2 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "old"),
                line(DiffLineKind::Added, None, Some(1), "new"),
                line(DiffLineKind::Context, Some(2), Some(2), "tail"),
            ],
        }])];
        let groups = vec![BlameGroup {
            commit_id: "c".repeat(40),
            author: "Alice".to_string(),
            time: 1_700_000_000,
            filename: "a.txt".to_string(),
            lines: vec![rebased_rs::git::BlameLine {
                number: 1,
                content: "new".to_string(),
            }],
        }];
        let content = build_content("new\ntail\n", &diff, &groups);
        assert_eq!(content.lines.len(), 2);
        assert_eq!(content.lines[0].change, Some(LineChange::Modified));
        let blame = content.lines[0].blame.as_ref().expect("首行应有 blame");
        assert_eq!(blame.author, "Alice");
        assert!(!blame.time.is_empty());
        assert!(!blame.author.contains(&"c".repeat(8)));
    }
}
