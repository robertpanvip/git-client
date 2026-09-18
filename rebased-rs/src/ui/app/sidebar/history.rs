use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
};

use crate::ui::app::AppView;
use crate::ui::components::{empty_state, list_row, panel_header};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_history_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let path = self.state.history_path.clone();

        let mut panel = div()
            .id("history-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .child(panel_header(
                format!("{} · {path}", tr("History", "历史")),
                muted,
                vec![
                    Button::new("history-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx)))
                        .into_any_element(),
                ],
            ));

        if self.state.history_commits.is_empty() {
            panel = panel.child(empty_state(
                tr("No history for this file", "该文件没有历史记录"),
                muted,
            ));
        } else {
            for commit in &self.state.history_commits {
                let id = commit.id.0.clone();
                let short = id[..id.len().min(7)].to_string();
                let muted_fg = muted;
                let time = crate::ui::commit_list::format_time(commit.time);
                panel = panel.child(
                    list_row(format!("history-{id}"), fg)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_commit_diff_window(id.clone(), None, cx)
                        }))
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
                                .child(commit.subject.clone()),
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

    /// reflog 轻量视图：一览 HEAD 的最近操作，点击跳到对应 commit 的 diff。
    pub(crate) fn render_reflog_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;

        let mut panel = div()
            .id("reflog-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .child(panel_header(
                format!(
                    "{} · {}",
                    tr("Reflog", "引用日志"),
                    tr("recent operations", "最近操作")
                ),
                muted,
                vec![
                    Button::new("reflog-refresh")
                        .ghost()
                        .compact()
                        .icon(Ic::Refresh)
                        .on_click(cx.listener(|this, _, _, cx| this.open_reflog(cx)))
                        .into_any_element(),
                    Button::new("reflog-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx)))
                        .into_any_element(),
                ],
            ));

        if self.state.reflog_entries.is_empty() {
            panel = panel.child(empty_state(tr("No reflog entries", "没有引用日志"), muted));
        } else {
            for entry in &self.state.reflog_entries {
                let id = entry.commit_id.clone();
                panel = panel.child(
                    list_row(format!("reflog-{}", entry.selector), fg)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_commit_diff_window(id.clone(), None, cx)
                        }))
                        .child(
                            div()
                                .flex_none()
                                .w(px(theme::COL_SELECTOR_WIDTH))
                                .text_size(px(theme::font_size_meta()))
                                .text_color(muted)
                                .child(SharedString::from(entry.selector.clone())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(theme::font_size_meta()))
                                .child(SharedString::from(entry.message.clone())),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(theme::font_size_meta()))
                                .text_color(muted)
                                .child(SharedString::from(entry.short_id.clone())),
                        ),
                );
            }
        }
        panel
    }
}
