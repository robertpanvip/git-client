//! Diff 渲染（统一 / 并排三栏两种视图）。
//!
//! Rebased 对齐 IntelliJ merge diff 的**三栏模型**：
//! - 左栏 = old / Yours（源文件 / 当前分支原始行，带 old_no gutter）
//! - **中间栏 = Result（自动合并结果或用户选择结果，无 gutter——核心创新点）**
//! - 右栏 = new / Theirs（目标文件 / 合入分支原始行，带 new_no gutter）
//!
//! 统一视图（`side_by_side = false`）仍是经典 unified diff：两列 gutter + 一列正文。
//!
//! 设计约束：
//! - 并排三栏的每列正文都 `flex_1 + overflow_hidden`，保证三栏在任何视口下同时可见，
//!   不会出现「右半滑出视口」的上下观感
//! - 中间 Result 栏的内容由左右栏差异**自动派生**（见 `side_by_side_rows` 内规则）：
//!   Context 行三栏相同、Deleted 行结果为空、Added 行结果为 new、修改行结果为 new
//! - 行高、字号、间距一律引用 theme token，禁止裸 `px(...)`

use std::sync::Arc;

use gpui::{App, Div, Hsla, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants};
use rebased_rs::git::{DiffLine, DiffLineKind, FileDiff, Hunk};

use crate::ui::theme;
use crate::ui::theme::{
    added_color, added_line_bg, binary_color, deleted_color, deleted_line_bg, empty_half_bg,
    gutter_bg, gutter_border, hunk_bg, stripe_bg, transparent,
};

/// hunk 级暂存回调：(file_index, hunk_index, app)。
pub type HunkAction = Arc<dyn Fn(usize, usize, &mut App)>;

fn line_fg(kind: DiffLineKind, base: Hsla) -> Hsla {
    match kind {
        DiffLineKind::Added => added_color(),
        DiffLineKind::Deleted => deleted_color(),
        DiffLineKind::Context | DiffLineKind::HunkHeader => base,
    }
}

fn status_badge(file: &FileDiff) -> (&'static str, Hsla) {
    if file.is_new {
        ("A", added_color())
    } else if file.is_deleted {
        ("D", deleted_color())
    } else if file.is_binary {
        ("B", binary_color())
    } else {
        ("M", crate::ui::theme::modified_color())
    }
}

/// 将 hunk 内的行配对成并排**三栏行**。配对逻辑：
/// - Context → 三栏相同（左=中=右）
/// - 连续 Deleted 段与紧跟的连续 Added 段按序 zip，短侧补 None
/// - 独立 Added 段（无前导 Delete）→ 左空、中=new、右=Added
/// - 独立 Deleted 段（无后继 Add）→ 左=Deleted、中空、右空
///
/// **中间 Result 的派生规则**（merge diff 语义）：
/// | 左 | 右 | 中间 Result | 原因 |
/// |---|---|---|---|
/// | Context(L) | Context(R) | L.content | 不变行，结果保留原样 |
/// | Deleted | None | "" | 删除生效，结果中无此行 |
/// | None | Added(R) | R.content | 新增生效，结果中出现新行 |
/// | Deleted | Added(R) | R.content | 修改 = 删旧+加新，结果是新行 |
pub(crate) fn side_by_side_rows(
    hunk: &Hunk,
) -> Vec<(Option<&DiffLine>, String, Option<&DiffLine>)> {
    let lines = &hunk.lines;
    let mut rows: Vec<(Option<&DiffLine>, String, Option<&DiffLine>)> = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        match lines[idx].kind {
            DiffLineKind::Context => {
                let content = lines[idx].content.clone();
                rows.push((Some(&lines[idx]), content, Some(&lines[idx])));
                idx += 1;
            }
            DiffLineKind::Deleted => {
                let del_start = idx;
                while idx < lines.len() && lines[idx].kind == DiffLineKind::Deleted {
                    idx += 1;
                }
                let del_end = idx;
                let mut add_end = del_end;
                while add_end < lines.len() && lines[add_end].kind == DiffLineKind::Added {
                    add_end += 1;
                }
                let dels = &lines[del_start..del_end];
                let adds = &lines[del_end..add_end];
                let n = dels.len().max(adds.len());
                for i in 0..n {
                    let left = dels.get(i);
                    let right = adds.get(i);
                    let middle = match (left, right) {
                        (Some(_), Some(r)) => r.content.clone(), // 修改: 新行取代旧行
                        (Some(_), None) => String::new(),        // 纯删除: 结果中无此行
                        (None, Some(r)) => r.content.clone(),    // 纯新增
                        (None, None) => String::new(),
                    };
                    rows.push((left, middle, right));
                }
                idx = add_end;
            }
            DiffLineKind::Added => {
                let add_start = idx;
                while idx < lines.len() && lines[idx].kind == DiffLineKind::Added {
                    idx += 1;
                }
                for line in &lines[add_start..idx] {
                    rows.push((None, line.content.clone(), Some(line)));
                }
            }
            DiffLineKind::HunkHeader => idx += 1,
        }
    }
    rows
}

/// 行号 gutter 单元格：固定列宽、右对齐、独立底色。
fn gutter_cell(no: Option<u32>, mono: &SharedString, muted: Hsla) -> Div {
    div()
        .w(px(crate::ui::theme::DIFF_GUTTER_COLUMN_WIDTH))
        .h_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .pr(px(crate::ui::theme::SPACE_XS))
        .bg(gutter_bg())
        .border_r_1()
        .border_color(gutter_border())
        .text_size(px(crate::ui::theme::FONT_SIZE_MONO))
        .text_color(muted.opacity(0.7))
        .font_family(mono.clone())
        .child(no.map(|n| n.to_string()).unwrap_or_default())
}

/// 正文单元格：不换行、不收缩，宽度由 `min_w` 保证横向滚动（统一视图）。
fn code_cell(content: String, color: Hsla, mono: &SharedString, min_w: f32) -> Div {
    div()
        .h_full()
        .min_w(px(min_w))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(crate::ui::theme::SPACE_SM))
        .text_size(px(crate::ui::theme::FONT_SIZE_MONO))
        .text_color(color)
        .font_family(mono.clone())
        .child(if content.is_empty() {
            " ".to_string()
        } else {
            content
        })
}

/// 并排三栏正文单元格（通用）：`flex_1` + 基线 `min_w` + 半宽内裁剪。
/// 三栏共享此原语，与内容长度解耦保证三栏始终同时可见。
fn sbs_code(content: String, color: Hsla, mono: &SharedString) -> Div {
    div()
        .h_full()
        .min_w(px(crate::ui::theme::DIFF_SBS_CODE_BASE_WIDTH))
        .flex_1()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(crate::ui::theme::SPACE_SM))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(crate::ui::theme::FONT_SIZE_MONO))
        .text_color(color)
        .font_family(mono.clone())
        .child(if content.is_empty() {
            " ".to_string()
        } else {
            content
        })
}

/// 并排三栏的**左/右栏**（带 gutter）：
/// - left_side=true → 左栏（old/Yours，行号取自 `old_no`）
/// - left_side=false → 右栏（new/Theirs，行号取自 `new_no`）
fn sbs_side(
    line: Option<&DiffLine>,
    left_side: bool,
    mono: &SharedString,
    fg: Hsla,
    muted: Hsla,
) -> Div {
    let mut half = div()
        .flex_1()
        .min_w_0()
        .h(px(crate::ui::theme::DIFF_LINE_HEIGHT))
        .flex()
        .flex_row()
        .items_center();
    let Some(line) = line else {
        return half
            .bg(empty_half_bg())
            .child(gutter_cell(None, mono, muted))
            .child(sbs_code(String::new(), fg, mono));
    };
    let bg = match line.kind {
        DiffLineKind::Added if !left_side => added_line_bg(),
        DiffLineKind::Deleted if left_side => deleted_line_bg(),
        _ => transparent(),
    };
    let no = if left_side { line.old_no } else { line.new_no };
    half = half
        .bg(bg)
        .child(gutter_cell(no, mono, muted))
        .child(sbs_code(line.content.clone(), line_fg(line.kind, fg), mono));
    half
}

/// 并排三栏的**中间 Result 栏**（无 gutter）：
/// 底色表达 "结果行是否存在"（空行=删除生效，透明=保留），
/// 正文 color 区分保留（base fg）vs 新增/修改（added_color）。
fn sbs_result(content: String, exists_in_result: bool, mono: &SharedString, fg: Hsla) -> Div {
    let bg = if exists_in_result {
        transparent()
    } else {
        empty_half_bg()
    };
    let color = if exists_in_result {
        fg
    } else {
        fg.opacity(0.4)
    };
    div()
        .flex_1()
        .min_w_0()
        .h(px(crate::ui::theme::DIFF_LINE_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .bg(bg)
        .child(sbs_code(content, color, mono))
}

/// 并排三栏行：gutter(两列) + 三栏 code_cell，总宽 `DIFF_GUTTER_WIDTH + 3×BASE`。
/// 视口足够时每栏 flex_1 三等分，任何一栏的长行都在各自半宽内裁剪，
/// 保证三栏同时可见（IntelliJ merge diff 行为）。
fn sbs_row3(
    left: Option<&DiffLine>,
    middle: String,
    right: Option<&DiffLine>,
    mono: &SharedString,
    fg: Hsla,
    muted: Hsla,
) -> Div {
    let result_exists = !middle.is_empty();
    div()
        .flex_none()
        .min_w(px(
            crate::ui::theme::DIFF_GUTTER_WIDTH + 3.0 * crate::ui::theme::DIFF_SBS_CODE_BASE_WIDTH
        ))
        .flex()
        .flex_row()
        .child(sbs_side(left, true, mono, fg, muted))
        .child(sbs_result(middle, result_exists, mono, fg))
        .child(sbs_side(right, false, mono, fg, muted))
}

pub fn render_diff_files(
    files: &[FileDiff],
    side_by_side: bool,
    hunk_controls: Option<(&'static str, &HunkAction)>,
    cx: &App,
) -> Div {
    let mono = cx.theme().mono_font_family.clone();
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let border = cx.theme().border;

    let mut container = div().flex().flex_col().gap(px(theme::SPACE_MD));
    // 内容最小宽度：取所有 hunk 中最宽者。滚动容器据此计算横向可滚动范围。
    let mut content_min_w: f32 = 0.0;

    for (file_index, file) in files.iter().enumerate() {
        let (badge, badge_color) = status_badge(file);
        // 文件头是**整宽条**（原版 diff 不为每个文件画圆角卡片）：
        // 无圆角/裁剪，正文超宽时才能横向滚动。
        let mut block = div()
            .flex_none()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_none()
                    .h(px(crate::ui::theme::SECTION_HEADER_HEIGHT))
                    .px(px(theme::SPACE_MD))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_MD))
                    .bg(stripe_bg(fg))
                    .child(
                        div()
                            .text_size(px(crate::ui::theme::FONT_SIZE_META))
                            .text_color(badge_color)
                            .child(badge),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(px(crate::ui::theme::FONT_SIZE_META))
                            .child(file.path.clone()),
                    ),
            );

        for (hunk_index, hunk) in file.hunks.iter().enumerate() {
            let min_w = code_min_width(max_line_chars(hunk));
            // 行宽 = gutter 列 + 正文最窄宽度。
            // - 并排三栏：DIFF_GUTTER_WIDTH(两列 gutter) + 3×BASE（左/中/右 各一栏）
            //   与内容长度解耦，三栏永远同时可见，长行半宽内裁剪。
            // - 统一视图：DIFF_GUTTER_WIDTH(两列) + 内容实际宽，长行靠横向滚动。
            let row_min_w = if side_by_side {
                crate::ui::theme::DIFF_GUTTER_WIDTH
                    + 3.0 * crate::ui::theme::DIFF_SBS_CODE_BASE_WIDTH
            } else {
                crate::ui::theme::DIFF_GUTTER_WIDTH + min_w
            };
            content_min_w = content_min_w.max(row_min_w);
            let mut header_row = div()
                .flex_none()
                .h(px(crate::ui::theme::SECTION_HEADER_HEIGHT))
                .min_w(px(row_min_w))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px(px(crate::ui::theme::SPACE_SM))
                .text_size(px(crate::ui::theme::FONT_SIZE_MONO))
                .text_color(muted)
                .bg(hunk_bg())
                .border_t_1()
                .border_b_1()
                .border_color(crate::ui::theme::border_color(fg))
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .font_family(mono.clone())
                        .child(hunk.header.clone()),
                );
            if let Some((label, action)) = hunk_controls {
                let action = Arc::clone(action);
                header_row = header_row.child(
                    Button::new(SharedString::from(format!(
                        "hunk-{file_index}-{hunk_index}"
                    )))
                    .ghost()
                    .compact()
                    .label(label)
                    .on_click(move |_, _, app| action(file_index, hunk_index, app)),
                );
            }
            block = block.child(header_row);

            if side_by_side {
                for (left, middle, right) in side_by_side_rows(hunk) {
                    block = block.child(sbs_row3(left, middle, right, &mono, fg, muted));
                }
            } else {
                for line in &hunk.lines {
                    let bg = match line.kind {
                        DiffLineKind::Added => added_line_bg(),
                        DiffLineKind::Deleted => deleted_line_bg(),
                        DiffLineKind::Context | DiffLineKind::HunkHeader => transparent(),
                    };
                    block = block.child(
                        div()
                            .flex_none()
                            .h(px(crate::ui::theme::DIFF_LINE_HEIGHT))
                            .min_w(px(crate::ui::theme::DIFF_GUTTER_WIDTH + min_w))
                            .flex()
                            .flex_row()
                            .items_center()
                            .bg(bg)
                            .child(gutter_cell(line.old_no, &mono, muted))
                            .child(gutter_cell(line.new_no, &mono, muted))
                            .child(code_cell(
                                line.content.clone(),
                                line_fg(line.kind, fg),
                                &mono,
                                min_w,
                            )),
                    );
                }
            }
        }

        container = container.child(block);
    }

    container.min_w(px(content_min_w))
}

/// 估算正文所需最小宽度（按最长行字符数 × 单字符步进）。
fn code_min_width(max_chars: usize) -> f32 {
    let chars = max_chars.max(1) as f32;
    chars * crate::ui::theme::DIFF_CHAR_WIDTH + crate::ui::theme::SPACE_LG
}

fn max_line_chars(hunk: &Hunk) -> usize {
    hunk.lines
        .iter()
        .map(|line| line.content.chars().count())
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: DiffLineKind, old: Option<u32>, new: Option<u32>, content: &str) -> DiffLine {
        DiffLine {
            kind,
            old_no: old,
            new_no: new,
            content: content.to_string(),
        }
    }

    #[test]
    fn side_by_side_pairs_context_lines() {
        let hunk = Hunk {
            header: "@@ -1,2 +1,2 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Context, Some(1), Some(1), "a"),
                line(DiffLineKind::Context, Some(2), Some(2), "b"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 2);
        for (left, middle, right) in &rows {
            // Context 行三栏相同
            assert_eq!(left.unwrap().content, *middle);
            assert_eq!(right.unwrap().content, *middle);
        }
        assert_eq!(rows[0].0.unwrap().content, "a");
        assert_eq!(rows[0].2.unwrap().content, "a");
        assert_eq!(rows[0].1, "a");
    }

    #[test]
    fn side_by_side_pairs_equal_del_add_blocks() {
        let hunk = Hunk {
            header: "@@ -1,3 +1,3 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "old1"),
                line(DiffLineKind::Deleted, Some(2), None, "old2"),
                line(DiffLineKind::Added, None, Some(1), "new1"),
                line(DiffLineKind::Added, None, Some(2), "new2"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 2);
        // 修改配对: 左=deleted, 中=new(修改后的结果), 右=added
        assert_eq!(rows[0].0.unwrap().content, "old1");
        assert_eq!(rows[0].1, "new1");
        assert_eq!(rows[0].2.unwrap().content, "new1");
        assert_eq!(rows[1].0.unwrap().content, "old2");
        assert_eq!(rows[1].1, "new2");
        assert_eq!(rows[1].2.unwrap().content, "new2");
    }

    #[test]
    fn side_by_side_pads_shorter_side() {
        let hunk = Hunk {
            header: "@@ -1,3 +1,2 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "old1"),
                line(DiffLineKind::Deleted, Some(2), None, "old2"),
                line(DiffLineKind::Deleted, Some(3), None, "old3"),
                line(DiffLineKind::Added, None, Some(1), "new1"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 3);
        // 第 0 行: 左=old1, 中=new1(修改结果), 右=new1
        assert_eq!(rows[0].0.unwrap().content, "old1");
        assert_eq!(rows[0].1, "new1");
        assert_eq!(rows[0].2.unwrap().content, "new1");
        // 第 1、2 行: 纯删除 → 中间 Result 为空
        assert_eq!(rows[1].0.unwrap().content, "old2");
        assert!(rows[1].1.is_empty());
        assert!(rows[1].2.is_none());
        assert_eq!(rows[2].0.unwrap().content, "old3");
        assert!(rows[2].1.is_empty());
        assert!(rows[2].2.is_none());
    }

    #[test]
    fn side_by_side_lone_added_lines_left_none() {
        let hunk = Hunk {
            header: "@@ -0,0 +1,2 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Added, None, Some(1), "a"),
                line(DiffLineKind::Added, None, Some(2), "b"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 2);
        for (left, middle, right) in &rows {
            assert!(left.is_none());
            assert!(right.is_some());
            // 纯新增 → 中间 Result = 右栏 new
            assert_eq!(*middle, right.unwrap().content);
        }
    }

    #[test]
    fn side_by_side_pure_deletes_middle_empty() {
        let hunk = Hunk {
            header: "@@ -1,2 +1,0 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "gone1"),
                line(DiffLineKind::Deleted, Some(2), None, "gone2"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 2);
        for (left, middle, right) in &rows {
            assert!(left.is_some());
            assert!(right.is_none());
            assert!(middle.is_empty());
        }
    }

    #[test]
    fn code_width_grows_with_longest_line() {
        let short = Hunk {
            header: "@@ -1 +1 @@".to_string(),
            lines: vec![line(DiffLineKind::Context, Some(1), Some(1), "abc")],
        };
        let long = Hunk {
            header: "@@ -1 +1 @@".to_string(),
            lines: vec![line(
                DiffLineKind::Context,
                Some(1),
                Some(1),
                &"x".repeat(200),
            )],
        };
        assert!(code_min_width(max_line_chars(&long)) > code_min_width(max_line_chars(&short)));
        assert_eq!(max_line_chars(&long), 200);
    }
}
