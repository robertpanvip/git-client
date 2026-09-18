use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
};

use rebased_rs::git::{Change, Commit, Tag};

use crate::ui::commit_list::format_full_time;
use crate::ui::components::{badge, group_header, ref_style, v_separator};
use crate::ui::graph_view::status_color;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

use super::{AppView, ConfirmAction, PromptKind};

impl AppView {
    pub(crate) fn render_detail_file_row(
        &self,
        index: usize,
        change: &Change,
        commit_id: &str,
        fg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = status_color(&change.status);
        let path = change.display_path();
        let diff_path = change.path.clone();
        let blame_path = change.path.clone();
        let history_path = change.path.clone();
        let file_commit_id = commit_id.to_string();

        div()
            .id(format!("detail-file-{index}"))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_MD))
            .px(px(theme::SPACE_MD))
            .py(px(theme::SPACE_XS))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .hover(move |style| style.bg(theme::hover_bg(fg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_commit_diff(file_commit_id.clone(), Some(diff_path.clone()), cx)
            }))
            .child(
                div()
                    .w(px(theme::ICON_SIZE_SM))
                    .flex_none()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(color)
                    .child(change.status.short_label()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_meta()))
                    .child(path),
            )
            .child(
                Button::new(format!("detail-blame-{index}"))
                    .ghost()
                    .compact()
                    .icon(Ic::Blame)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_blame(blame_path.clone(), cx);
                    })),
            )
            .child(
                Button::new(format!("detail-history-{index}"))
                    .ghost()
                    .compact()
                    .icon(Ic::History)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_file_history(history_path.clone(), cx);
                    })),
            )
            .into_any_element()
    }

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

        let file_rows: Vec<AnyElement> = self
            .state
            .detail_files
            .iter()
            .enumerate()
            .map(|(index, change)| self.render_detail_file_row(index, change, &commit_id, fg, cx))
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(theme::SPACE_MD))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap(px(theme::SPACE_SM))
                            .child(
                                div()
                                    .text_size(px(theme::font_size_body()))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(fg)
                                    .child(commit.subject.clone()),
                            )
                            .when(is_head, |head_row| {
                                head_row.child(badge(
                                    SharedString::from("detail-head"),
                                    "HEAD",
                                    theme::head_color(),
                                ))
                            }),
                    )
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
                    .child(format!(
                        "{} · {} · {} · {}",
                        &commit.id.0[..commit.id.0.len().min(7)],
                        commit.author.name,
                        format_full_time(commit.time),
                        commit.author.email
                    )),
            )
            .when(!commit.body.is_empty(), |detail| {
                detail.child(
                    div()
                        .flex_none()
                        .text_size(px(theme::font_size_meta()))
                        .text_color(muted)
                        .whitespace_normal()
                        .child(commit.body.clone()),
                )
            })
            .when(!self.state.detail_branches.is_empty(), |detail| {
                let branch_row = div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .children(self.state.detail_branches.iter().map(|branch| {
                        let (label, color) = ref_style(branch);
                        badge(
                            SharedString::from(format!("detail-branch-{branch}")),
                            label,
                            color,
                        )
                    }));
                detail.child(branch_row)
            })
            .when(!commit_tags.is_empty(), |detail| {
                detail.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(px(theme::SPACE_SM))
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
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .flex_none()
                    .child(
                        Button::new("detail-cherry-pick")
                            .ghost()
                            .compact()
                            .label(tr("Cherry-pick", "摘取提交"))
                            .on_click(cx.listener(|this, _, _, cx| this.cherry_pick_selected(cx))),
                    )
                    .child(
                        Button::new("detail-revert")
                            .ghost()
                            .compact()
                            .label(tr("Revert", "回滚"))
                            .on_click(cx.listener(|this, _, _, cx| this.revert_selected(cx))),
                    )
                    .when(is_head, |row| {
                        row.child(
                            Button::new("detail-undo-commit")
                                .ghost()
                                .compact()
                                .icon(Ic::Undo)
                                .label(tr("Undo Commit", "撤销提交"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_prompt(
                                        PromptKind::Confirm(ConfirmAction::UndoHeadCommit),
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("detail-drop-commit")
                                .danger()
                                .compact()
                                .icon(Ic::Delete)
                                .label(tr("Drop Commit", "丢弃提交"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_prompt(
                                        PromptKind::Confirm(ConfirmAction::DropHeadCommit),
                                        cx,
                                    );
                                })),
                        )
                    })
                    .child(v_separator(fg))
                    .child(
                        Button::new("detail-rebase")
                            .ghost()
                            .compact()
                            .label(tr("Rebase from here", "从这里变基"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let base = this
                                    .state
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.start_rebase(base, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-reset")
                            .ghost()
                            .compact()
                            .label(tr("Reset…", "重置…"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let commit_id = this
                                    .state
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_prompt(PromptKind::Reset { commit_id }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-reword")
                            .ghost()
                            .compact()
                            .label(tr("Reword…", "改写…"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let commit_id = this
                                    .state
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_prompt(PromptKind::Reword { commit_id }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-checkout")
                            .ghost()
                            .compact()
                            .label(tr("Checkout", "检出"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let Some(commit) = this.state.selected.clone() else {
                                    return;
                                };
                                let id = commit.id.0.clone();
                                let short = &id[..id.len().min(7)];
                                let message = format!("{} {short}", tr("Checked out", "已检出"));
                                this.run_op(&message, move |repo| repo.checkout(&id), cx);
                            })),
                    )
                    .child(v_separator(fg))
                    .child(
                        Button::new("detail-branch")
                            .ghost()
                            .compact()
                            .label(tr("Branch…", "新建分支…"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let start_point =
                                    this.state.selected.as_ref().map(|c| c.id.0.clone());
                                this.open_prompt(PromptKind::NewBranch { start_point }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-tag")
                            .ghost()
                            .compact()
                            .label(tr("Tag…", "新建标签…"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let commit_id = this
                                    .state
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_prompt(PromptKind::NewTag { commit_id }, cx);
                            })),
                    )
                    .child(
                        Button::new("detail-copy-sha")
                            .ghost()
                            .compact()
                            .icon(Ic::Copy)
                            .label(tr("Copy SHA", "复制 SHA"))
                            .on_click(cx.listener(|this, _, _, cx| this.copy_commit_sha(cx))),
                    )
                    .child(v_separator(fg))
                    .child(
                        Button::new("detail-diff")
                            .ghost()
                            .compact()
                            .label(tr("Diff", "查看差异"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let id = this
                                    .state
                                    .selected
                                    .as_ref()
                                    .map(|c| c.id.0.clone())
                                    .unwrap_or_default();
                                this.open_commit_diff(id, None, cx);
                            })),
                    )
                    .when(!is_head, |row| {
                        row.child(
                            Button::new("detail-compare")
                                .ghost()
                                .compact()
                                .icon(Ic::Compare)
                                .label(tr("Compare", "比较"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let id = this
                                        .state
                                        .selected
                                        .as_ref()
                                        .map(|c| c.id.0.clone())
                                        .unwrap_or_default();
                                    this.open_branch_compare(id, cx);
                                })),
                        )
                    }),
            )
            .child(group_header(
                format!(
                    "{} ({})",
                    tr("Files", "文件"),
                    self.state.detail_files.len()
                ),
                muted,
            ))
            .child(
                div()
                    .id("detail-files")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(file_rows),
            )
    }
}
