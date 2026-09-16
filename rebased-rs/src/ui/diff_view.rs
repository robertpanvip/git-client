use std::sync::Arc;

use gpui::{App, Div, Hsla, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants};
use rebased_rs::git::{DiffLine, DiffLineKind, FileDiff, Hunk};

use crate::ui::theme::{
    added_color, added_line_bg, binary_color, deleted_color, deleted_line_bg, empty_half_bg,
    hunk_bg, stripe_bg, transparent,
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

/// 将 hunk 内的行配对成并排行：连续 Deleted 段与紧跟的连续 Added 段按序 zip，
/// 短的一侧补 None（渲染为空半行）；Context 行自成一对（左右同一行）。
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
                let mut add_end = del_end;
                while add_end < lines.len() && lines[add_end].kind == DiffLineKind::Added {
                    add_end += 1;
                }
                let dels = &lines[del_start..del_end];
                let adds = &lines[del_end..add_end];
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

/// 并排视图半行：`old_side = true` 为左半（旧行），否则为右半（新行）。
fn sbs_half(
    line: Option<&DiffLine>,
    old_side: bool,
    mono: SharedString,
    fg: Hsla,
    muted: Hsla,
) -> Div {
    let mut half = div().flex_1().min_w_0().flex().flex_row().px_2().py(px(1.));
    let Some(line) = line else {
        return half
            .bg(empty_half_bg())
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .text_color(muted.opacity(0.5))
                    .font_family(mono.clone())
                    .child("      "),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_xs()
                    .font_family(mono)
                    .child(" "),
            );
    };
    let bg = match line.kind {
        DiffLineKind::Added if !old_side => added_line_bg(),
        DiffLineKind::Deleted if old_side => deleted_line_bg(),
        _ => transparent(),
    };
    let no = if old_side { line.old_no } else { line.new_no };
    let no_str = no
        .map(|n| format!("{n:>4} "))
        .unwrap_or_else(|| "     ".to_string());
    let content = if line.content.is_empty() {
        " ".to_string()
    } else {
        line.content.clone()
    };
    half = half
        .bg(bg)
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(muted.opacity(0.7))
                .font_family(mono.clone())
                .child(no_str),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_xs()
                .text_color(line_fg(line.kind, fg))
                .font_family(mono)
                .child(content),
        );
    half
}

fn sbs_row(
    left: Option<&DiffLine>,
    right: Option<&DiffLine>,
    mono: &SharedString,
    fg: Hsla,
    muted: Hsla,
) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .child(sbs_half(left, true, mono.clone(), fg, muted))
        .child(sbs_half(right, false, mono.clone(), fg, muted))
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

    let mut container = div().flex().flex_col().gap_2();

    for (file_index, file) in files.iter().enumerate() {
        let (badge, badge_color) = status_badge(file);
        let mut block = div()
            .flex_none()
            .rounded(px(crate::ui::theme::RADIUS_LG))
            .border_1()
            .border_color(border)
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .bg(stripe_bg(fg))
                    .child(div().text_xs().text_color(badge_color).child(badge))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_xs()
                            .child(file.path.clone()),
                    ),
            );

        for (hunk_index, hunk) in file.hunks.iter().enumerate() {
            let mut header_row = div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_0p5()
                .text_xs()
                .text_color(muted)
                .bg(hunk_bg())
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
                    .text_xs()
                    .label(label)
                    .on_click(move |_, _, app| action(file_index, hunk_index, app)),
                );
            }
            block = block.child(header_row);

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
                    let color = line_fg(line.kind, fg);
                    let old_no = line
                        .old_no
                        .map(|n| format!("{n:>4}"))
                        .unwrap_or_else(|| "    ".to_string());
                    let new_no = line
                        .new_no
                        .map(|n| format!("{n:>4}"))
                        .unwrap_or_else(|| "    ".to_string());
                    let content = if line.content.is_empty() {
                        " ".to_string()
                    } else {
                        line.content.clone()
                    };
                    block = block.child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .px_2()
                            .py(px(1.))
                            .bg(bg)
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(muted.opacity(0.7))
                                    .font_family(mono.clone())
                                    .child(format!("{old_no} {new_no}  ")),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_xs()
                                    .text_color(color)
                                    .font_family(mono.clone())
                                    .child(content),
                            ),
                    );
                }
            }
        }

        container = container.child(block);
    }

    container
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
}
