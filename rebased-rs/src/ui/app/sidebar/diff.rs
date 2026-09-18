use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    input::Textarea,
};

use crate::ui::app::{AppView, DiffSource};
use crate::ui::components::{empty_state, panel_header};
use crate::ui::diff_view::{HunkAction, render_diff_files};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_diff_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let title = self.state.diff_title.clone();

        let mut actions: Vec<AnyElement> = Vec::new();
        if !self.state.diff_files.is_empty() {
            actions.push(
                Button::new("diff-ignore-ws")
                    .ghost()
                    .compact()
                    // 开关态由按钮的 selected 呈现，而不是在文案里拼 `✓` 前缀。
                    .selected(self.state.ignore_whitespace)
                    .label(tr("Ignore whitespace", "忽略空白"))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_ignore_whitespace(cx)))
                    .into_any_element(),
            );
            actions.push(
                Button::new("diff-view-mode")
                    .ghost()
                    .compact()
                    .selected(self.state.diff_side_by_side)
                    .label(tr("Side-by-side", "并排对比"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.diff_side_by_side = !this.state.diff_side_by_side;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if self.state.diff_path.is_some() {
            actions.push(
                Button::new("diff-blame")
                    .ghost()
                    .compact()
                    .label(tr("Blame", "追溯"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let path = this.state.diff_path.clone().unwrap_or_default();
                        this.open_blame(path, cx);
                    }))
                    .into_any_element(),
            );
        }
        if self.state.diff_path.is_some() && !self.state.diff_editing {
            actions.push(
                Button::new("diff-edit")
                    .ghost()
                    .compact()
                    .icon(Ic::Edit)
                    .on_click(cx.listener(|this, _, window, cx| this.open_diff_edit(window, cx)))
                    .into_any_element(),
            );
        }
        if self.state.diff_editing {
            actions.push(
                Button::new("diff-save")
                    .ghost()
                    .compact()
                    .label(tr("Save", "保存"))
                    .on_click(cx.listener(|this, _, _, cx| this.save_diff_edit(cx)))
                    .into_any_element(),
            );
            actions.push(
                Button::new("diff-cancel")
                    .ghost()
                    .compact()
                    .label(tr("Cancel", "取消"))
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_diff_edit(cx)))
                    .into_any_element(),
            );
        }
        actions.push(
            Button::new("diff-new-window")
                .ghost()
                .compact()
                .icon(Ic::Changes)
                .on_click(cx.listener(|this, _, _, cx| this.open_diff_in_new_window(cx)))
                .into_any_element(),
        );
        actions.push(
            Button::new("diff-close")
                .ghost()
                .compact()
                .icon(Ic::Close)
                .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx)))
                .into_any_element(),
        );

        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .min_h_0()
            .child(panel_header(title, muted, actions));

        if self.state.diff_editing {
            panel = panel.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(Textarea::new(&self.diff_edit_input).flex_1()),
            );
        } else if self.state.diff_files.is_empty() {
            panel = panel.child(empty_state(tr("No changes", "无更改"), muted));
        } else {
            let hunk_controls: Option<(&'static str, HunkAction)> = match self.state.diff_source {
                Some(source @ (DiffSource::Staged | DiffSource::Unstaged)) => {
                    let label = if source == DiffSource::Staged {
                        tr("Unstage", "取消暂存")
                    } else {
                        tr("Stage", "暂存")
                    };
                    let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                    Some((
                        label,
                        std::sync::Arc::new(move |file_index, hunk_index, app: &mut gpui::App| {
                            let _ = weak.update(app, |this, cx| {
                                this.toggle_hunk_stage(file_index, hunk_index, cx)
                            });
                        }),
                    ))
                }
                _ => None,
            };
            // 「左栏内容同步到右栏」箭头：仅 Unstaged（右栏 = 工作区当前版本）。
            let sync_action: Option<HunkAction> =
                if self.state.diff_source == Some(DiffSource::Unstaged) {
                    let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                    Some(std::sync::Arc::new(
                        move |file_index, hunk_index, app: &mut gpui::App| {
                            let _ = weak.update(app, |this, cx| {
                                this.sync_hunk_from_left(file_index, hunk_index, cx)
                            });
                        },
                    ))
                } else {
                    None
                };
            panel = panel.child(
                div()
                    .id("diff-content")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .child(render_diff_files(
                        &self.state.diff_files,
                        self.state.diff_side_by_side,
                        hunk_controls
                            .as_ref()
                            .map(|(label, action)| (*label, action)),
                        sync_action.as_ref(),
                        cx,
                    )),
            );
        }
        panel
    }
}
