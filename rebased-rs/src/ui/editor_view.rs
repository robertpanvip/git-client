//! 文件视图右栏：代码区域（行号 gutter + 行变更标记 + 行级 blame）。
//!
//! 对齐 IntelliJ 编辑器「Annotate」的经典形式：
//! - **行首窄条**标记该行相对 HEAD 的变更类型（新增 / 修改 / 删除）；
//! - **blame 列**显示每行的最近提交（短哈希 + 作者，按提交着色），点击跳到该提交；
//! - 其后是行号 gutter 与等宽正文，长行横向滚动。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, Div, Hsla, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_kit::component::ActiveTheme;
use rebased_rs::git::{BlameGroup, DiffLineKind, FileDiff};

use crate::ui::blame_view::BlameJump;
use crate::ui::commit_list::short_id;
use crate::ui::graph_view::lane_color;
use crate::ui::i18n::tr;
use crate::ui::theme;

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

/// 渲染代码区域。
pub(crate) fn render_editor(
    content: &str,
    changes: &HashMap<u32, LineChange>,
    blame: &[BlameGroup],
    on_commit: Option<&BlameJump>,
    cx: &App,
) -> Div {
    let mono = cx.theme().mono_font_family.clone();
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let index = blame_index(blame);

    let lines: Vec<&str> = content.lines().collect();
    let shown = lines.len().min(theme::EDITOR_MAX_LINES);
    let max_chars = lines
        .iter()
        .take(shown)
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let code_width = (max_chars.max(1) as f32) * theme::DIFF_CHAR_WIDTH + theme::SPACE_LG;
    let row_min_w = theme::EDITOR_CHANGE_BAR_WIDTH
        + theme::EDITOR_BLAME_WIDTH
        + theme::DIFF_GUTTER_COLUMN_WIDTH
        + code_width;

    let mut container = div().flex().flex_col();
    for (offset, text) in lines.iter().take(shown).enumerate() {
        let number = offset as u32 + 1;
        let change = changes.get(&number).copied();

        let marker_bg = match change {
            Some(LineChange::Added) => theme::added_color(),
            Some(LineChange::Modified) => theme::modified_color(),
            Some(LineChange::Deleted) => theme::deleted_color(),
            None => theme::transparent(),
        };
        let row_bg = match change {
            Some(LineChange::Added) => theme::added_line_bg(),
            Some(LineChange::Modified) => theme::modified_line_bg(),
            _ => theme::transparent(),
        };

        // 未提交的行由 git 标记为全 0 commit，没有可跳转的提交——直接显示「未提交」。
        let group = index.get(&number).and_then(|slot| blame.get(*slot));
        let uncommitted = group.is_some_and(|group| group.commit_id.chars().all(|c| c == '0'));
        let (blame_bg, blame_fg, blame_text) = match (group, index.get(&number)) {
            (Some(_), Some(slot)) if uncommitted => (
                theme::badge_bg(lane_color(*slot)),
                lane_color(*slot),
                tr("Not committed", "未提交").to_string(),
            ),
            (Some(group), Some(slot)) => (
                theme::badge_bg(lane_color(*slot)),
                lane_color(*slot),
                format!("{} {}", short_id(&group.commit_id), group.author),
            ),
            _ => (theme::transparent(), muted.opacity(0.5), String::new()),
        };

        let mut blame_cell: Stateful<Div> = div()
            .id(format!("ed-blame-{number}"))
            .w(px(theme::EDITOR_BLAME_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .pl(px(theme::SPACE_SM))
            .pr(px(theme::SPACE_SM))
            .overflow_hidden()
            .whitespace_nowrap()
            .bg(blame_bg)
            .text_size(px(theme::FONT_SIZE_MONO))
            .text_color(blame_fg)
            .font_family(mono.clone())
            .child(blame_text);
        if let (Some(group), Some(jump), false) = (group, on_commit, uncommitted) {
            let id = group.commit_id.clone();
            let jump = Arc::clone(jump);
            blame_cell = blame_cell.cursor_pointer().on_click(move |_, _, app| {
                jump(id.clone(), app);
            });
        }

        container = container.child(
            div()
                .flex_none()
                .h(px(theme::DIFF_LINE_HEIGHT))
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
                        .text_size(px(theme::FONT_SIZE_MONO))
                        .text_color(muted.opacity(0.7))
                        .font_family(mono.clone())
                        .child(number.to_string()),
                )
                .child(code_cell(text.to_string(), fg, &mono, code_width)),
        );
    }
    if lines.len() > shown {
        // 超大文件只画前 N 行：明确告知还有多少行未显示，避免误以为文件到此为止。
        container = container.child(
            div()
                .flex_none()
                .h(px(theme::DIFF_LINE_HEIGHT))
                .min_w(px(row_min_w))
                .flex()
                .flex_row()
                .items_center()
                .pl(px(theme::SPACE_MD))
                .text_size(px(theme::FONT_SIZE_META))
                .text_color(muted)
                .child(format!(
                    "… {} {}",
                    lines.len() - shown,
                    tr("more lines not shown", "行未显示")
                )),
        );
    }
    container.min_w(px(row_min_w))
}

fn code_cell(content: String, color: Hsla, mono: &gpui::SharedString, min_w: f32) -> Div {
    div()
        .h_full()
        .min_w(px(min_w))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(theme::SPACE_SM))
        .text_size(px(theme::FONT_SIZE_MONO))
        .text_color(color)
        .font_family(mono.clone())
        .child(if content.is_empty() {
            " ".to_string()
        } else {
            content
        })
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
}
