use gpui::{
    list, px, AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, div,
};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    button::{Button, ButtonVariants},
};

use rebased_rs::git::{ConflictHunk, HunkChoice};

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
        // 面板头 ↑↓：在当前文件的冲突块之间循环导航并滚动定位。
        let mut actions: Vec<AnyElement> = Vec::new();
        if !self.state.conflict_hunks.is_empty() {
            actions.push(
                Button::new("conflict-prev-hunk")
                    .ghost()
                    .compact()
                    .icon(Ic::ChevronUp)
                    .tooltip(tr("Previous conflict", "上一处"))
                    .on_click(cx.listener(|this, _, _, cx| this.step_conflict_nav(false, cx)))
                    .into_any_element(),
            );
            actions.push(
                Button::new("conflict-next-hunk")
                    .ghost()
                    .compact()
                    .icon(Ic::ChevronDown)
                    .tooltip(tr("Next conflict", "下一处"))
                    .on_click(cx.listener(|this, _, _, cx| this.step_conflict_nav(true, cx)))
                    .into_any_element(),
            );
        }

        let mut panel = div()
            .id("conflicts-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .overflow_y_scroll()
            .child(panel_header(tr("Conflicts", "冲突"), muted, actions))
            .child(
                div()
                    .flex_none()
                    .px(px(theme::SPACE_MD))
                    .text_size(px(theme::font_size_meta()))
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
                    .text_size(px(theme::font_size_meta()))
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
                    .text_size(px(theme::font_size_body()))
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
                panel = panel.child(div().text_size(px(theme::font_size_meta())).text_color(muted).child(tr(
                    "No textual hunks in this file. Use the buttons above to take one side.",
                    "该文件没有文本冲突块。使用上方按钮选择一侧。",
                )));
            }
            // 冲突块卡片：变高卡片交给 GPUI list 虚拟滚动，↑↓ 导航经
            // scroll_to_reveal_item 定位，主色描边高亮当前卡片。
            let hunks = self.state.conflict_hunks.clone();
            let choices = self.state.conflict_choices.clone();
            let card_path = path.clone();
            let nav = self.state.conflict_nav;
            let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
            panel = panel.child(
                list(self.state.conflict_list.clone(), move |ix, _, _| {
                    div()
                        .pb(px(theme::SPACE_MD))
                        .child(conflict_hunk_card(
                            ix,
                            &hunks[ix],
                            choices.get(ix).copied().flatten(),
                            &card_path,
                            border,
                            muted,
                            nav == Some(ix),
                            &weak,
                        ))
                        .into_any_element()
                })
                .flex_1()
                .min_h(px(200.)),
            );
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

/// 渲染单个冲突块卡片：标题行 + 取舍按钮 + Yours / Result / Theirs 三栏视图。
/// `navigated` 为 true 时用主色描边高亮（↑↓ 导航定位的当前卡片）。
/// 按钮回调经 `weak` 走 AppView（list 的渲染闭包里拿不到 `cx.listener`）。
#[allow(clippy::too_many_arguments)]
fn conflict_hunk_card(
    index: usize,
    hunk: &ConflictHunk,
    choice: Option<HunkChoice>,
    path: &str,
    border: Hsla,
    muted: Hsla,
    navigated: bool,
    weak: &gpui::WeakEntity<AppView>,
) -> Div {
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
    let chose_ours = choice == Some(HunkChoice::Ours);
    let chose_theirs = choice == Some(HunkChoice::Theirs);
    let chose_both = choice == Some(HunkChoice::Both);
    // Result 栏实时反映当前所选取舍（Ours / Theirs / Both）。
    let result_text = if chose_ours {
        ours_text.clone()
    } else if chose_theirs {
        theirs_text.clone()
    } else if chose_both {
        if theirs_text.is_empty() {
            ours_text.clone()
        } else {
            format!("{}\n{}", ours_text, theirs_text)
        }
    } else {
        tr("— unresolved —", "— 未解决 —").to_string()
    };
    let result_label = match choice {
        Some(HunkChoice::Ours) => {
            format!("{} · {}", tr("Result", "结果"), tr("ours", "我们的"))
        }
        Some(HunkChoice::Theirs) => {
            format!("{} · {}", tr("Result", "结果"), tr("theirs", "他们的"))
        }
        Some(HunkChoice::Both) => {
            format!("{} · {}", tr("Result", "结果"), tr("both", "两者"))
        }
        None => tr("Result", "结果").to_string(),
    };
    let choose_button = |ids: (&'static str, usize),
                         label: SharedString,
                         selected: bool,
                         picked: HunkChoice| {
        let weak = weak.clone();
        Button::new(ids)
            .ghost()
            .compact()
            // 取舍状态由按钮 selected 呈现，不拼 `✓` 前缀。
            .selected(selected)
            .label(label)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.choose_hunk(index, picked, cx));
            })
    };

    div()
        .flex()
        .flex_col()
        .gap(px(theme::SPACE_SM))
        .border_1()
        .border_color(if navigated {
            theme::primary_blue()
        } else {
            border
        })
        .rounded(px(theme::RADIUS))
        .p(px(theme::SPACE_MD))
        .child(
            div()
                .text_size(px(theme::font_size_meta()))
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
                .child(choose_button(
                    ("hunk-ours", index),
                    tr("Use ours", "采用我们的").into(),
                    chose_ours,
                    HunkChoice::Ours,
                ))
                .child(choose_button(
                    ("hunk-theirs", index),
                    tr("Use theirs", "采用他们的").into(),
                    chose_theirs,
                    HunkChoice::Theirs,
                ))
                .child(choose_button(
                    ("hunk-both", index),
                    tr("Use both", "两者都采用").into(),
                    chose_both,
                    HunkChoice::Both,
                )),
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
                                .text_size(px(theme::font_size_meta()))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted)
                                .child(tr("Yours", "你的")),
                        )
                        .child(
                            div()
                                .text_size(px(theme::font_size_meta()))
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
                                .text_size(px(theme::font_size_meta()))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted)
                                .child(result_label),
                        )
                        .child(
                            div()
                                .text_size(px(theme::font_size_meta()))
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
                                .text_size(px(theme::font_size_meta()))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(muted)
                                .child(tr("Theirs", "他们的")),
                        )
                        .child(
                            div()
                                .text_size(px(theme::font_size_meta()))
                                .child(theirs_text),
                        ),
                ),
        )
}
