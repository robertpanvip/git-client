use gpui::{div, hsla, px, App, Div, Hsla, ParentElement, Styled};
use gpui_kit::component::ActiveTheme;
use rebased_rs::git::{DiffLineKind, FileDiff};

fn added_line_bg() -> Hsla {
    hsla(0.31, 0.6, 0.42, 0.14)
}

fn deleted_line_bg() -> Hsla {
    hsla(0.0, 0.65, 0.5, 0.13)
}

fn hunk_bg() -> Hsla {
    hsla(0.58, 0.7, 0.55, 0.1)
}

fn line_fg(kind: DiffLineKind, base: Hsla) -> Hsla {
    match kind {
        DiffLineKind::Added => hsla(0.31, 0.6, 0.45, 1.0),
        DiffLineKind::Deleted => hsla(0.0, 0.7, 0.55, 1.0),
        DiffLineKind::Context | DiffLineKind::HunkHeader => base,
    }
}

fn status_badge(file: &FileDiff) -> (&'static str, Hsla) {
    if file.is_new {
        ("A", hsla(0.31, 0.6, 0.45, 1.0))
    } else if file.is_deleted {
        ("D", hsla(0.0, 0.7, 0.55, 1.0))
    } else if file.is_binary {
        ("B", hsla(0.58, 0.7, 0.55, 1.0))
    } else {
        ("M", hsla(0.11, 0.8, 0.55, 1.0))
    }
}

pub fn render_diff_files(files: &[FileDiff], cx: &App) -> Div {
    let mono = cx.theme().mono_font_family.clone();
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let border = cx.theme().border;

    let mut container = div().flex().flex_col().gap_2();

    for file in files {
        let (badge, badge_color) = status_badge(file);
        let mut block = div()
            .flex_none()
            .rounded(px(6.))
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
                    .bg(hsla(fg.h, fg.s, fg.l, 0.05))
                    .child(
                        div()
                            .text_xs()
                            .text_color(badge_color)
                            .child(badge),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_xs()
                            .child(file.path.clone()),
                    ),
            );

        for hunk in &file.hunks {
            block = block.child(
                div()
                    .flex_none()
                    .px_2()
                    .py_0p5()
                    .text_xs()
                    .text_color(muted)
                    .bg(hunk_bg())
                    .font_family(mono.clone())
                    .child(hunk.header.clone()),
            );

            for line in &hunk.lines {
                let bg = match line.kind {
                    DiffLineKind::Added => added_line_bg(),
                    DiffLineKind::Deleted => deleted_line_bg(),
                    DiffLineKind::Context | DiffLineKind::HunkHeader => hsla(0.0, 0.0, 0.5, 0.0),
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

        container = container.child(block);
    }

    container
}
