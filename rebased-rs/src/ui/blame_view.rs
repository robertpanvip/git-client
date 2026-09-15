use std::sync::Arc;

use gpui::{
    div, hsla, px, App, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui_kit::component::ActiveTheme;
use rebased_rs::git::BlameGroup;

use crate::ui::commit_list::format_time;
use crate::ui::graph_view::lane_color;

pub type BlameJump = Arc<dyn Fn(String, &mut App)>;

pub fn render_blame(groups: &[BlameGroup], on_commit: Option<&BlameJump>, cx: &App) -> Div {
    let mono = cx.theme().mono_font_family.clone();
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;

    let mut container = div().flex().flex_col();

    for (index, group) in groups.iter().enumerate() {
        let stripe = if index % 2 == 0 {
            hsla(fg.h, fg.s, fg.l, 0.04)
        } else {
            hsla(0.0, 0.0, 0.5, 0.0)
        };
        let short: String = group.commit_id.chars().take(7).collect();
        let meta_color = lane_color(index);

        let mut meta: gpui::Stateful<Div> = div()
            .id(format!("blame-meta-{index}"))
            .flex_none()
            .px_2()
            .pt_1()
            .py_0p5()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .bg(hsla(meta_color.h, meta_color.s, meta_color.l, 0.12))
            .child(
                div()
                    .text_xs()
                    .font_family(mono.clone())
                    .text_color(meta_color)
                    .child(short),
            )
            .child(
                div()
                    .text_xs()
                    .child(group.author.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(format_time(group.time)),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_xs()
                    .text_color(muted.opacity(0.8))
                    .child(group.filename.clone()),
            );

        if let Some(jump) = on_commit {
            let id = group.commit_id.clone();
            let jump = jump.clone();
            meta = meta
                .cursor_pointer()
                .on_click(move |_, _, app| {
                    jump(id.clone(), app);
                });
        }

        container = container.children([meta]).children(
            group
                .lines
                .iter()
                .map(|line| {
                    let content = if line.content.is_empty() {
                        " ".to_string()
                    } else {
                        line.content.clone()
                    };
                    div()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .px_2()
                        .py(px(1.))
                        .bg(stripe)
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(muted.opacity(0.7))
                                .font_family(mono.clone())
                                .child(format!("{:>4}  ", line.number)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .font_family(mono.clone())
                                .child(content),
                        )
                })
                .collect::<Vec<_>>(),
        );
    }

    container
}