use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
};

use crate::ui::app::AppView;
use crate::ui::components::{group_header, list_row, panel_header};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_compare_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let mine = self.state.compare_mine.clone();
        let theirs = self.state.compare_theirs.clone();

        let mut panel = div()
            .id("compare-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .child(panel_header(
                format!("{} · {mine} ←→ {theirs}", tr("Compare", "比较")),
                muted,
                vec![
                    Button::new("compare-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx)))
                        .into_any_element(),
                ],
            ));

        let sections = [
            (
                format!(
                    "{} {mine} ({})",
                    tr("Only in", "仅存在于"),
                    self.state.compare_ahead.len()
                ),
                &self.state.compare_ahead,
            ),
            (
                format!(
                    "{} {theirs} ({})",
                    tr("Only in", "仅存在于"),
                    self.state.compare_behind.len()
                ),
                &self.state.compare_behind,
            ),
        ];
        for (label, commits) in sections {
            panel = panel.child(group_header(label, muted));
            if commits.is_empty() {
                panel = panel.child(
                    div()
                        .flex_none()
                        .px(px(theme::SPACE_MD))
                        .text_size(px(theme::font_size_meta()))
                        .text_color(muted)
                        .child(tr("None", "无")),
                );
            }
            for commit in commits.iter() {
                let id = commit.id.0.clone();
                let short = id[..id.len().min(7)].to_string();
                let subject = commit.subject.clone();
                let muted_fg = muted;
                let time = crate::ui::commit_list::format_time(commit.time);
                panel = panel.child(
                    list_row(format!("compare-{id}"), fg)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.select_commit_by_id(&id, cx)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(theme::font_size_meta()))
                                .text_color(muted_fg)
                                .child(short),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(theme::font_size_body()))
                                .child(subject),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(theme::font_size_meta()))
                                .text_color(muted_fg)
                                .child(time),
                        ),
                );
            }
        }

        panel
    }
}
