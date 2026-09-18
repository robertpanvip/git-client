use gpui::{
    Context, Div, FontWeight, InteractiveElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    button::{Button, ButtonVariants},
};

use rebased_rs::git::HunkChoice;

use crate::ui::app::AppView;
use crate::ui::components::{list_row, panel_header, selected as selected_row};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_conflicts_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let mut panel = div()
            .id("conflicts-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .overflow_y_scroll()
            .child(panel_header(tr("Conflicts", "冲突"), muted, Vec::new()))
            .child(
                div()
                    .flex_none()
                    .px(px(theme::SPACE_MD))
                    .text_size(px(theme::FONT_SIZE_META))
                    .text_color(muted)
                    .child(if self.state.merge_in_progress {
                        tr(
                            "Merge is paused. Resolve conflicts, then continue the merge.",
                            "合并已暂停。解决冲突后继续合并。",
                        )
                    } else {
                        tr(
                            "Rebase is paused. Resolve conflicts, then continue the rebase.",
                            "变基已暂停。解决冲突后继续变基。",
                        )
                    }),
            );

        if self.state.conflict_files.is_empty() {
            let message = if let Some(sha) = self.state.rebase.stopped_commit() {
                format!(
                    "{} {}… {}",
                    tr("Rebase stopped for editing at", "变基在此处暂停编辑："),
                    &sha[..sha.len().min(7)],
                    tr(
                        "Make changes, amend or commit, then click Continue Rebase.",
                        "修改、修正或提交后，点击“继续变基”。",
                    )
                )
            } else {
                tr("No conflicted files.", "没有冲突文件。").to_string()
            };
            panel = panel.child(
                div()
                    .text_size(px(theme::FONT_SIZE_META))
                    .text_color(muted)
                    .child(message),
            );
        }

        for (index, file) in self.state.conflict_files.iter().enumerate() {
            let selected = self.state.conflict_path.as_deref() == Some(file.path.as_str());
            // 冲突文件行复用标准列表行：固定行高 + 悬停底色 + 选中底色，
            // 选中态由底色表达，不再用 `●` 字形前缀。
            let mut row = selected_row(
                list_row(format!("conflict-file-{index}"), fg).on_click(cx.listener({
                    let path = file.path.clone();
                    move |this, _, _, cx| this.select_conflict_file(&path, cx)
                })),
                selected,
                fg,
            )
            .child(
                div()
                    .flex_none()
                    .w(px(theme::ICON_SIZE_MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::conflicted_color())
                    .child(Icon::new(Ic::Conflict).with_size(Size::Small)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::FONT_SIZE_BODY))
                    .child(file.path.clone()),
            );
            row = row.child(
                Button::new(("take-ours", index))
                    .ghost()
                    .compact()
                    .label(tr("Take ours", "采用我们的"))
                    .on_click(cx.listener({
                        let path = file.path.clone();
                        move |this, _, _, cx| this.take_conflict_side(path.clone(), true, cx)
                    })),
            );
            row = row.child(
                Button::new(("take-theirs", index))
                    .ghost()
                    .compact()
                    .label(tr("Take theirs", "采用他们的"))
                    .on_click(cx.listener({
                        let path = file.path.clone();
                        move |this, _, _, cx| this.take_conflict_side(path.clone(), false, cx)
                    })),
            );
            panel = panel.child(row);
        }

        if let Some(path) = self.state.conflict_path.clone() {
            if self.state.conflict_hunks.is_empty() {
                panel = panel.child(div().text_size(px(theme::FONT_SIZE_META)).text_color(muted).child(tr(
                    "No textual hunks in this file. Use the buttons above to take one side.",
                    "该文件没有文本冲突块。使用上方按钮选择一侧。",
                )));
            }
            for (index, hunk) in self.state.conflict_hunks.iter().enumerate() {
                let ours_text = if hunk.ours.is_empty() {
                    tr("(empty)", "（空）").to_string()
                } else {
                    hunk.ours.join("\n")
                };
                let theirs_text = if hunk.theirs.is_empty() {
                    tr("(empty)", "（空）").to_string()
                } else {
                    hunk.theirs.join("\n")
                };
                let ours_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => {
                        format!("✓ {}", tr("Use ours", "采用我们的"))
                    }
                    _ => tr("Use ours", "采用我们的").to_string(),
                };
                let theirs_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Theirs)) => {
                        format!("✓ {}", tr("Use theirs", "采用他们的"))
                    }
                    _ => tr("Use theirs", "采用他们的").to_string(),
                };
                let both_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Both)) => {
                        format!("✓ {}", tr("Use both", "两者都采用"))
                    }
                    _ => tr("Use both", "两者都采用").to_string(),
                };
                let chose_ours = self
                    .state
                    .conflict_choices
                    .get(index)
                    .is_some_and(|c| c == &Some(HunkChoice::Ours));
                let chose_theirs = self
                    .state
                    .conflict_choices
                    .get(index)
                    .is_some_and(|c| c == &Some(HunkChoice::Theirs));
                let last_choice = self.state.conflict_choices.get(index).copied().flatten();
                // Result 栏实时反映当前所选取舍（Ours / Theirs / Both）。
                let result_text = if chose_ours {
                    ours_text.clone()
                } else if chose_theirs {
                    theirs_text.clone()
                } else if last_choice == Some(HunkChoice::Both) {
                    if theirs_text.is_empty() {
                        ours_text.clone()
                    } else {
                        format!("{}\n{}", ours_text, theirs_text)
                    }
                } else {
                    tr("— unresolved —", "— 未解决 —").to_string()
                };
                let result_label = match self.state.conflict_choices.get(index) {
                    Some(Some(HunkChoice::Ours)) => {
                        format!("{} · {}", tr("Result", "结果"), tr("ours", "我们的"))
                    }
                    Some(Some(HunkChoice::Theirs)) => {
                        format!("{} · {}", tr("Result", "结果"), tr("theirs", "他们的"))
                    }
                    Some(Some(HunkChoice::Both)) => {
                        format!("{} · {}", tr("Result", "结果"), tr("both", "两者"))
                    }
                    _ => tr("Result", "结果").to_string(),
                };
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_SM))
                        .border_1()
                        .border_color(border)
                        .rounded(px(theme::RADIUS))
                        .p(px(theme::SPACE_MD))
                        .child(
                            div()
                                .text_size(px(theme::FONT_SIZE_META))
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!(
                                    "{} {} · {}",
                                    tr("Hunk", "冲突块"),
                                    index + 1,
                                    path
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(theme::SPACE_SM))
                                .child(
                                    Button::new(("hunk-ours", index))
                                        .ghost()
                                        .compact()
                                        .label(ours_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Ours, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("hunk-theirs", index))
                                        .ghost()
                                        .compact()
                                        .label(theirs_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Theirs, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("hunk-both", index))
                                        .ghost()
                                        .compact()
                                        .label(both_label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_hunk(index, HunkChoice::Both, cx)
                                        })),
                                ),
                        )
                        // 三栏合并视图：左侧 Yours（当前分支）、中间 Result（合并结果）、
                        // 右侧 Theirs（合入分支的变更）。
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_stretch()
                                .gap(px(theme::SPACE_SM))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(theme::SPACE_SM))
                                        .border_1()
                                        .border_color(border)
                                        .rounded(px(theme::RADIUS))
                                        .p(px(theme::SPACE_SM))
                                        .child(
                                            div()
                                                .text_size(px(theme::FONT_SIZE_META))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child(tr("Yours", "你的")),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(theme::FONT_SIZE_META))
                                                .child(ours_text),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(theme::SPACE_SM))
                                        .border_1()
                                        .border_color(border)
                                        .rounded(px(theme::RADIUS))
                                        .p(px(theme::SPACE_SM))
                                        .child(
                                            div()
                                                .text_size(px(theme::FONT_SIZE_META))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child(result_label),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(theme::FONT_SIZE_META))
                                                .child(result_text),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(theme::SPACE_SM))
                                        .border_1()
                                        .border_color(border)
                                        .rounded(px(theme::RADIUS))
                                        .p(px(theme::SPACE_SM))
                                        .child(
                                            div()
                                                .text_size(px(theme::FONT_SIZE_META))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(muted)
                                                .child(tr("Theirs", "他们的")),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(theme::FONT_SIZE_META))
                                                .child(theirs_text),
                                        ),
                                ),
                        ),
                );
            }
            if !self.state.conflict_hunks.is_empty() {
                panel = panel.child(
                    Button::new("conflict-apply")
                        .primary()
                        .compact()
                        .label(tr("Apply Resolutions", "应用解决结果"))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.apply_conflict_resolutions(cx)),
                        ),
                );
            }
        }

        panel
    }
}
