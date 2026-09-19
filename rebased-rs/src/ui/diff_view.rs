//! Diff 渲染（统一 / 并排两种视图）。
//!
//! 对齐 IntelliJ diff 编辑器的结构：
//! - **独立行号 gutter**：固定列宽 [`DIFF_GUTTER_COLUMN_WIDTH`]、独立底色、右侧分隔线，
//!   与正文分离（原实现把行号拼进正文文本里，列宽随内容漂移）；
//! - 固定行高 [`diff_line_height()`]，不是由字号+padding 自动撑开；
//! - 正文不换行，超宽时整体**横向滚动**（按最长行估算内容宽度）；
//! - hunk 头独立成行（hunk 底色 + 上下分隔线 + 右侧 Stage/Unstage）。
//!
//! 并排视图为**两栏**：左 = old（旧行，`old_no` gutter），右 = new（新行，
//! `new_no` gutter）。两栏宽度与内容长度解耦（基线宽 + flex 平分），任何
//! 视口下都同时可见，长行在各自半宽内裁剪——不会出现某一半被挤出视口
//! 造成「上下排列」的观感。
//!
//! 两个 surface（右侧面板 / 独立窗口）共用本模块，保证呈现一致。

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{App, Div, Hsla, ParentElement, Point, ScrollHandle, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants};
use rebased_rs::git::{DiffLine, DiffLineKind, FileDiff, Hunk};

use crate::ui::icons::Ic;
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

/// 文件头状态徽章：与 Changes 面板**同一语义源头** `theme::status_color`——
/// 重命名显示 R 黄、冲突显示 U 品红、untracked 显示 ? 灰绿，
/// 不再是列表 "R"/diff "M" 的两套口径。仅当数据未标注状态时按 bool 兜底。
fn status_badge(file: &FileDiff) -> (&'static str, Hsla) {
    if let Some(status) = file.status {
        return (status.short_label(), crate::ui::theme::status_color(&status));
    }
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

/// 将 hunk 内的行配对成并排两栏行：连续 Deleted 段与紧跟的连续 Added 段
/// 按序 zip，短的一侧补 None（渲染为空半行）；Context 行自成一对
/// （左右同一行）；独立 Added 段左侧补 None。
/// 注意：删除段与新增段之间若夹着 Context 行，这些 Context 会先作为对齐行输出，
/// 随后删除段仍与新增段配对（见 Bug #12），避免新增沦为右栏"孤立新增"。
pub(crate) fn side_by_side_rows(hunk: &Hunk) -> Vec<(Option<&DiffLine>, Option<&DiffLine>)> {
    let lines = &hunk.lines;
    let mut rows: Vec<(Option<&DiffLine>, Option<&DiffLine>)> = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        match lines[idx].kind {
            DiffLineKind::Context => {
                rows.push((Some(&lines[idx]), Some(&lines[idx])));
                idx += 1;
            }
            DiffLineKind::Deleted => {
                let del_start = idx;
                while idx < lines.len() && lines[idx].kind == DiffLineKind::Deleted {
                    idx += 1;
                }
                let del_end = idx;
                // 删除段与新增段之间可能夹着 Context 行：先把这些 Context 作为
                // 左右对齐的同行输出，再让删除段与紧跟的新增段按序配对，
                // 避免新增被当成右栏"孤立新增"而错位（Bug #12）。
                while idx < lines.len() && lines[idx].kind == DiffLineKind::Context {
                    rows.push((Some(&lines[idx]), Some(&lines[idx])));
                    idx += 1;
                }
                let mut add_end = idx;
                while add_end < lines.len() && lines[add_end].kind == DiffLineKind::Added {
                    add_end += 1;
                }
                let dels = &lines[del_start..del_end];
                let adds = &lines[idx..add_end];
                for i in 0..dels.len().max(adds.len()) {
                    rows.push((dels.get(i), adds.get(i)));
                }
                idx = add_end;
            }
            DiffLineKind::Added => {
                let add_start = idx;
                while idx < lines.len() && lines[idx].kind == DiffLineKind::Added {
                    idx += 1;
                }
                for line in &lines[add_start..idx] {
                    rows.push((None, Some(line)));
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
        .text_size(px(crate::ui::theme::font_size_code()))
        .text_color(crate::ui::theme::gutter_number_fg(muted))
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
        .text_size(px(crate::ui::theme::font_size_code()))
        .text_color(color)
        .font_family(mono.clone())
        .child(if content.is_empty() {
            " ".to_string()
        } else {
            content
        })
}

/// 并排两栏正文单元格：`flex_1` 填满所在半宽、底色连续；最小宽度
/// 只取基线值而非内容实际宽度——左右两半因此永远不会被长行撑开
/// 导致一半滑出视口（那会呈现「上下排列」的观感），长行在半宽内裁剪。
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
        .text_size(px(crate::ui::theme::font_size_code()))
        .text_color(color)
        .font_family(mono.clone())
        .child(if content.is_empty() {
            " ".to_string()
        } else {
            content
        })
}

/// 并排两栏的半行：`left_side = true` 为左半（旧行，`old_no` gutter），
/// 否则为右半（新行，`new_no` gutter）。半宽与内容长度解耦
/// （基线宽 + flex 平分），保证两半始终并排可见。
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
        .h(px(crate::ui::theme::diff_line_height()))
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

/// 并排两栏行：左右各一个半行（gutter + 正文），总宽基线
/// `DIFF_GUTTER_WIDTH + 2 × BASE`。视口足够时两半 flex 平分整行，
/// 长行在半宽内裁剪，不触发整行撑开。
fn sbs_row(
    left: Option<&DiffLine>,
    right: Option<&DiffLine>,
    mono: &SharedString,
    fg: Hsla,
    muted: Hsla,
) -> Div {
    div()
        .flex_none()
        .min_w(px(
            crate::ui::theme::DIFF_GUTTER_WIDTH + 2.0 * crate::ui::theme::DIFF_SBS_CODE_BASE_WIDTH
        ))
        .flex()
        .flex_row()
        .child(sbs_side(left, true, mono, fg, muted))
        .child(sbs_side(right, false, mono, fg, muted))
}

/// 渲染 diff 内容（统一 / 并排两栏）。
/// - `hunk_controls`: Stage/Unstage 按钮（Staged/Unstaged 来源）
/// - `sync_action`: 「左栏内容同步到右栏」箭头（对齐 IntelliJ change marker 的 revert
///   箭头），点击后把右栏（当前版本）该 hunk 还原成左栏（基线）内容。仅 Unstaged 来源
///   提供——Staged 的等价操作是 Unstage，Commit 只读。
/// - `folded`: 已折叠的 hunk 集合（IDEA diff：折叠后只剩 header 行）
/// - `fold_action`: hunk header chevron 回调；两个 surface 各自桥接到自己的状态。
pub fn render_diff_files(
    files: &[FileDiff],
    side_by_side: bool,
    hunk_controls: Option<(&'static str, &HunkAction)>,
    sync_action: Option<&HunkAction>,
    folded: &HashSet<(usize, usize)>,
    fold_action: Option<&HunkAction>,
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
                            .text_size(px(crate::ui::theme::font_size_meta()))
                            .text_color(badge_color)
                            .child(badge),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(px(crate::ui::theme::font_size_meta()))
                            .child(file.path.clone()),
                    ),
            );

        for (hunk_index, hunk) in file.hunks.iter().enumerate() {
            let is_folded = folded.contains(&(file_index, hunk_index));
            // 行宽 = 行号 gutter 区 + 正文最窄宽度。并排两栏按基线宽计
            // （与内容长度解耦，保证左右两半同时可见）；统一视图按内容
            // 实际宽度计（长行靠横向滚动查看）。折叠的 hunk 不参与撑宽：
            // 收起后不再为隐藏内容保留横向滚动范围。
            let min_w = if is_folded {
                0.0
            } else {
                code_min_width(max_line_width(hunk))
            };
            let row_min_w = if is_folded {
                0.0
            } else if side_by_side {
                crate::ui::theme::DIFF_GUTTER_WIDTH
                    + 2.0 * crate::ui::theme::DIFF_SBS_CODE_BASE_WIDTH
            } else {
                crate::ui::theme::DIFF_GUTTER_WIDTH + min_w
            };
            content_min_w = content_min_w.max(row_min_w);
            // header 左段：chevron（IDEA diff 折叠开关）+ hunk 头文本。
            let mut header_left = div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(crate::ui::theme::SPACE_XS));
            if let Some(action) = fold_action {
                let action = Arc::clone(action);
                header_left = header_left.child(
                    Button::new(SharedString::from(format!(
                        "hunk-fold-{file_index}-{hunk_index}"
                    )))
                    .ghost()
                    .compact()
                    .icon(if is_folded {
                        Ic::ChevronRight
                    } else {
                        Ic::ChevronDown
                    })
                    .accessibility_label(if is_folded {
                        "Expand hunk"
                    } else {
                        "Collapse hunk"
                    })
                    .on_click(move |_, _, app| action(file_index, hunk_index, app)),
                );
            }
            header_left = header_left.child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(mono.clone())
                    .child(hunk.header.clone()),
            );
            let mut header_row = div()
                .flex_none()
                .h(px(crate::ui::theme::SECTION_HEADER_HEIGHT))
                .min_w(px(row_min_w))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px(px(crate::ui::theme::SPACE_SM))
                .text_size(px(crate::ui::theme::font_size_code()))
                .text_color(muted)
                .bg(hunk_bg())
                .border_t_1()
                .border_b_1()
                .border_color(crate::ui::theme::border_color(fg))
                .child(header_left);
            if sync_action.is_some() || hunk_controls.is_some() {
                // 右端按钮组：同步箭头（若有）+ Stage/Unstage（若有）。
                let mut actions_row = div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(crate::ui::theme::SPACE_XS));
                if let Some(action) = sync_action {
                    let action = Arc::clone(action);
                    actions_row = actions_row.child(
                        Button::new(SharedString::from(format!(
                            "hunk-sync-{file_index}-{hunk_index}"
                        )))
                        .ghost()
                        .compact()
                        .icon(Ic::Undo)
                        .accessibility_label("Sync hunk from left")
                        .on_click(move |_, _, app| action(file_index, hunk_index, app)),
                    );
                }
                if let Some((label, action)) = hunk_controls {
                    let action = Arc::clone(action);
                    actions_row = actions_row.child(
                        Button::new(SharedString::from(format!(
                            "hunk-{file_index}-{hunk_index}"
                        )))
                        .ghost()
                        .compact()
                        .label(label)
                        .on_click(move |_, _, app| action(file_index, hunk_index, app)),
                    );
                }
                header_row = header_row.child(actions_row);
            }
            block = block.child(header_row);

            // 折叠的 hunk：header 之后不再渲染内容行（IDEA diff 行为）。
            if !is_folded {
                if side_by_side {
                    for (left, right) in side_by_side_rows(hunk) {
                        block = block.child(sbs_row(left, right, &mono, fg, muted));
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
                                .h(px(crate::ui::theme::diff_line_height()))
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
        }

        container = container.child(block);
    }

    container.min_w(px(content_min_w))
}

/// 单个 hunk 的渲染高度：折叠时只剩 header 行；展开时
/// header + 内容行数 × 行高（并排视图按配对后的行数计）。
/// F7 导航的滚动定位与渲染共用同一算术，保证偏移不漂移。
pub(crate) fn hunk_height(hunk: &Hunk, side_by_side: bool, folded: bool) -> f32 {
    if folded {
        return crate::ui::theme::SECTION_HEADER_HEIGHT;
    }
    let rows = if side_by_side {
        side_by_side_rows(hunk).len()
    } else {
        hunk.lines.len()
    };
    crate::ui::theme::SECTION_HEADER_HEIGHT + rows as f32 * crate::ui::theme::diff_line_height()
}

/// 文件块的渲染高度：文件头 + 全部 hunk + 底部分隔线
/// （block 高度 auto，border 在内容之外另加 1px）。
fn file_block_height(
    file: &FileDiff,
    file_index: usize,
    side_by_side: bool,
    folded: &HashSet<(usize, usize)>,
) -> f32 {
    crate::ui::theme::SECTION_HEADER_HEIGHT
        + 1.0
        + file
            .hunks
            .iter()
            .enumerate()
            .map(|(hi, h)| hunk_height(h, side_by_side, folded.contains(&(file_index, hi))))
            .sum::<f32>()
}

/// hunk 顶边在滚动内容坐标系中的 y 偏移：前序文件块（含块间 gap）+ 本文件头 + 前序 hunk。
pub(crate) fn hunk_offset_y(
    files: &[FileDiff],
    target: (usize, usize),
    side_by_side: bool,
    folded: &HashSet<(usize, usize)>,
) -> f32 {
    let (target_file, target_hunk) = target;
    let mut y = 0.0;
    for (fi, file) in files.iter().enumerate() {
        if fi == target_file {
            y += crate::ui::theme::SECTION_HEADER_HEIGHT;
            for (hi, hunk) in file.hunks.iter().enumerate() {
                if hi == target_hunk {
                    return y;
                }
                y += hunk_height(hunk, side_by_side, folded.contains(&(fi, hi)));
            }
            return y;
        }
        y += file_block_height(file, fi, side_by_side, folded) + crate::ui::theme::SPACE_MD;
    }
    y
}

/// hunk 的展开顺序坐标（文件内从上到下、文件间按序）。
fn hunk_positions(files: &[FileDiff]) -> Vec<(usize, usize)> {
    files
        .iter()
        .enumerate()
        .flat_map(|(fi, f)| (0..f.hunks.len()).map(move |hi| (fi, hi)))
        .collect()
}

/// F7 / Shift+F7 的目标 hunk：从 current 起循环前进/后退；
/// 无 diff 或无 hunk 返回 None（current 不在列表时从头/尾开始）。
pub(crate) fn step_hunk(
    files: &[FileDiff],
    current: Option<(usize, usize)>,
    forward: bool,
) -> Option<(usize, usize)> {
    let positions = hunk_positions(files);
    if positions.is_empty() {
        return None;
    }
    let at = current.and_then(|c| positions.iter().position(|&p| p == c));
    let next = match (at, forward) {
        (Some(i), true) => (i + 1) % positions.len(),
        (Some(i), false) => (i + positions.len() - 1) % positions.len(),
        (None, true) => 0,
        (None, false) => positions.len() - 1,
    };
    Some(positions[next])
}

/// 把目标 hunk 顶边滚到视口顶部（F7 定位语义），保留横向偏移。
/// 垂直偏移为负值（向下滚动 y 越负），越界由容器在绘制时 clamp。
pub(crate) fn scroll_hunk_into_view(
    handle: &ScrollHandle,
    files: &[FileDiff],
    target: (usize, usize),
    side_by_side: bool,
    folded: &HashSet<(usize, usize)>,
) {
    let y = hunk_offset_y(files, target, side_by_side, folded);
    handle.set_offset(Point::new(handle.offset().x, px(-y)));
}

/// 估算正文所需最小宽度（按最长行字符数 × 单字符步进）。
fn code_min_width(max_chars: usize) -> f32 {
    let chars = max_chars.max(1) as f32;
    chars * crate::ui::theme::diff_char_width() + crate::ui::theme::SPACE_LG
}

/// 行在编辑器中的显示宽度（列数）：tab 按 4 列制表位展开，CJK/宽字符计 2 列。
/// 用于估算横向滚动宽度，避免含 tab / 宽字符的长行被按字符数低估而横向裁切（Bug #9）。
fn display_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    let mut col = 0usize;
    for c in s.chars() {
        if c == '\t' {
            col += 4 - (col % 4);
        } else {
            col += c.width().unwrap_or(0).max(1);
        }
    }
    col
}

fn max_line_width(hunk: &Hunk) -> usize {
    hunk.lines
        .iter()
        .map(|line| display_width(&line.content))
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
            no_newline: false,
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
        assert!(rows.iter().all(|(l, r)| l.is_some() && r.is_some()));
        assert_eq!(rows[0].0.unwrap().content, "a");
        assert_eq!(rows[0].1.unwrap().content, "a");
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
        assert_eq!(rows[0].0.unwrap().content, "old1");
        assert_eq!(rows[0].1.unwrap().content, "new1");
        assert_eq!(rows[1].0.unwrap().content, "old2");
        assert_eq!(rows[1].1.unwrap().content, "new2");
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
        assert_eq!(rows[0].1.unwrap().content, "new1");
        assert!(rows[1].1.is_none());
        assert!(rows[2].1.is_none());
    }

    #[test]
    fn side_by_side_pairs_delete_across_context() {
        // 删除段与新增段之间夹一段 Context 时，新增段仍应与删除段按序配对，
        // 而不应沦为右栏"孤立新增"（Bug #12）。
        let hunk = Hunk {
            header: "@@ -1,3 +1,3 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "old"),
                line(DiffLineKind::Context, Some(2), Some(2), "keep"),
                line(DiffLineKind::Added, None, Some(1), "new"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0.unwrap().content, "keep");
        assert_eq!(rows[0].1.unwrap().content, "keep");
        assert_eq!(rows[1].0.unwrap().content, "old");
        assert_eq!(rows[1].1.unwrap().content, "new");
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
        assert!(rows.iter().all(|(l, r)| l.is_none() && r.is_some()));
    }

    #[test]
    fn side_by_side_pure_deletes_right_none() {
        let hunk = Hunk {
            header: "@@ -1,2 +1,0 @@".to_string(),
            lines: vec![
                line(DiffLineKind::Deleted, Some(1), None, "gone1"),
                line(DiffLineKind::Deleted, Some(2), None, "gone2"),
            ],
        };
        let rows = side_by_side_rows(&hunk);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(l, r)| l.is_some() && r.is_none()));
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
        assert!(code_min_width(max_line_width(&long)) > code_min_width(max_line_width(&short)));
        assert_eq!(max_line_width(&long), 200);
    }

    fn ctx(n: u32) -> DiffLine {
        line(DiffLineKind::Context, Some(n), Some(n), "x")
    }

    fn hunk_with(lines: Vec<DiffLine>) -> Hunk {
        Hunk {
            header: "@@ -1 +1 @@".to_string(),
            lines,
        }
    }

    fn file_with(hunks: Vec<Hunk>) -> FileDiff {
        FileDiff {
            path: "a.txt".to_string(),
            old_path: None,
            is_new: false,
            is_deleted: false,
            is_binary: false,
            mode: None,
            old_mode: None,
            status: None,
            hunks,
        }
    }

    #[test]
    fn hunk_height_follows_view_mode_and_fold() {
        let hunk = hunk_with(vec![ctx(1), ctx(2)]);
        let line_h = crate::ui::theme::diff_line_height();
        let header_h = crate::ui::theme::SECTION_HEADER_HEIGHT;
        // 并排视图：两行 Context 配对成 2 行。
        assert_eq!(hunk_height(&hunk, true, false), header_h + 2.0 * line_h);
        // 统一视图按原始行数计。
        assert_eq!(hunk_height(&hunk, false, false), header_h + 2.0 * line_h);
        // 折叠时只剩 header。
        assert_eq!(hunk_height(&hunk, true, true), header_h);
    }

    #[test]
    fn hunk_offset_y_sums_preceding_blocks_and_gap() {
        let files = vec![
            file_with(vec![hunk_with(vec![ctx(1)])]),
            file_with(vec![hunk_with(vec![ctx(1)])]),
        ];
        let folded = HashSet::new();
        let line_h = crate::ui::theme::diff_line_height();
        let header_h = crate::ui::theme::SECTION_HEADER_HEIGHT;
        // 第一个 hunk 顶边 = 本文件头高度。
        assert_eq!(hunk_offset_y(&files, (0, 0), true, &folded), header_h);
        // 第二文件的 hunk = 文件块 0（文件头 + hunk + 底线）+ 块间 gap + 文件头。
        let block0 = header_h + 1.0 + (header_h + line_h);
        assert_eq!(
            hunk_offset_y(&files, (1, 0), true, &folded),
            block0 + crate::ui::theme::SPACE_MD + header_h
        );
    }

    #[test]
    fn folded_preceding_hunk_shifts_offset_up() {
        let files = vec![file_with(vec![
            hunk_with(vec![ctx(1), ctx(2), ctx(3)]),
            hunk_with(vec![ctx(1)]),
        ])];
        let open = HashSet::new();
        let mut folded = HashSet::new();
        folded.insert((0, 0));
        let delta =
            hunk_offset_y(&files, (0, 1), true, &folded) - hunk_offset_y(&files, (0, 1), true, &open);
        // 前序 hunk 折叠后，后续 hunk 上移正好 3 行的高度。
        assert_eq!(delta, -3.0 * crate::ui::theme::diff_line_height());
    }

    #[test]
    fn step_hunk_cycles_forward_and_backward() {
        let files = vec![
            file_with(vec![hunk_with(vec![ctx(1)])]),
            file_with(vec![hunk_with(vec![ctx(1)]), hunk_with(vec![ctx(1)])]),
        ];
        // 无当前：前进从头开始、后退从末尾开始。
        assert_eq!(step_hunk(&files, None, true), Some((0, 0)));
        assert_eq!(step_hunk(&files, None, false), Some((1, 1)));
        // 循环：末尾前进回到开头。
        assert_eq!(step_hunk(&files, Some((1, 1)), true), Some((0, 0)));
        assert_eq!(step_hunk(&files, Some((0, 0)), false), Some((1, 1)));
        // 常规前进。
        assert_eq!(step_hunk(&files, Some((0, 0)), true), Some((1, 0)));
        // 无 hunk 时返回 None。
        assert_eq!(step_hunk(&[file_with(vec![])], None, true), None);
    }
}
