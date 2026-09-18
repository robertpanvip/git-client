use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Div, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
};

use crate::ui::app::AppView;
use crate::ui::components::{empty_state, panel_header};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

/// 拖拽 payload：被拖动的 rebase 计划行下标。
#[derive(Clone, Copy)]
pub(crate) struct RebaseDrag(pub(crate) usize);

/// 拖拽时跟随鼠标的预览视图。
struct RebaseDragPreview(SharedString);

impl Render for RebaseDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .px(px(theme::SPACE_MD))
            .py(px(theme::SPACE_XS))
            .rounded_sm()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .text_size(px(theme::font_size_meta()))
            .shadow_md()
            .child(self.0.clone())
    }
}

impl AppView {
    pub(crate) fn render_rebase_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let Some((base, plan)) = self.state.rebase.plan_view() else {
            return div().id("rebase-panel").size_full();
        };
        let short_base = base[..base.len().min(7)].to_string();
        let mut panel = div()
            .id("rebase-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .overflow_y_scroll()
            .child(panel_header(
                tr("Interactive Rebase", "交互式变基"),
                muted,
                Vec::new(),
            ))
            .child(
                div()
                    .flex_none()
                    .px(px(theme::SPACE_MD))
                    .text_size(px(theme::font_size_meta()))
                    .text_color(muted)
                    .child(format!("{} {short_base}…", tr("onto", "变基到"))),
            )
            .child(
                div()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(muted)
                    .child(tr(
                        "Click the action to cycle Pick → Squash → Fixup → Drop → Edit → Reword. Drag rows or use ↑ ↓ to reorder.",
                        "点击操作循环切换 挑选 → 压缩 → 修整 → 丢弃 → 编辑 → 改写。拖动行或用 ↑ ↓ 调整顺序。",
                    )),
            );

        if plan.is_empty() {
            panel = panel.child(empty_state(
                tr(
                    "No commits between base and HEAD.",
                    "基点与 HEAD 之间没有提交。",
                ),
                muted,
            ));
        }

        let fg = cx.theme().foreground;
        for (index, action) in plan.iter().enumerate() {
            let kind_label = action.kind.label();
            let is_drop = action.kind == rebased_rs::git::RebaseActionKind::Drop;
            let short = &action.id[..action.id.len().min(7)];
            // 已自定义消息时展示自定义内容（IntelliJ：改写后直接显示新消息）。
            let summary = match action.message.as_deref() {
                Some(m) if !m.trim().is_empty() => format!("{short} {m}"),
                _ => format!("{short} {}", action.subject),
            };
            // on_drag 的 constructor 是 Fn，可能被多次调用，label 按次克隆。
            let drag_label: SharedString = format!("{} {short}", tr("Move", "移动")).into();
            let hover_fg = fg;
            panel = panel.child(
                div()
                    .id(("rebase-row", index))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .border_b_1()
                    .border_color(border)
                    .py(px(theme::SPACE_XS))
                    .cursor_move()
                    .hover(move |style| style.bg(theme::hover_bg(hover_fg)))
                    .drag_over::<RebaseDrag>(|style, _, _, _| {
                        style.border_color(theme::success_color())
                    })
                    .on_drag(RebaseDrag(index), move |_, _, _, cx| {
                        cx.new(|_| RebaseDragPreview(drag_label.clone()))
                    })
                    .on_drop(cx.listener(move |this, drag: &RebaseDrag, _, cx| {
                        this.move_rebase_action_to(drag.0, index, cx)
                    }))
                    .child(
                        Button::new(("rebase-kind", index))
                            .ghost()
                            .compact()
                            .min_w(px(theme::REBASE_KIND_WIDTH))
                            // Drop 是破坏性操作，与菜单/按钮体系一致用危险色。
                            .when(is_drop, |b| b.danger())
                            .label(kind_label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cycle_rebase_action(index, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(theme::font_size_body()))
                            .text_ellipsis()
                            .overflow_hidden()
                            .child(summary),
                    )
                    .child(
                        Button::new(("rebase-edit", index))
                            .ghost()
                            .compact()
                            .icon(Ic::Edit)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_rebase_edit(index, window, cx)
                            })),
                    )
                    .child(
                        Button::new(("rebase-up", index))
                            .ghost()
                            .compact()
                            .label("↑")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_rebase_action(index, -1, cx)
                            })),
                    )
                    .child(
                        Button::new(("rebase-down", index))
                            .ghost()
                            .compact()
                            .label("↓")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.move_rebase_action(index, 1, cx)
                            })),
                    ),
            );
        }

        panel.child(
            div()
                .flex()
                .flex_col()
                .gap(px(theme::SPACE_MD))
                .mt(px(theme::SPACE_SM))
                .child(
                    Checkbox::new("rebase-autosquash")
                        .checked(self.state.rebase_autosquash)
                        .label(tr(
                            "Autosquash fixup!/squash! commits",
                            "自动压缩 fixup!/squash! 提交",
                        ))
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.toggle_rebase_autosquash(*checked, cx)
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(theme::SPACE_MD))
                        .child(
                            Button::new("rebase-start")
                                .primary()
                                .compact()
                                .label(tr("Start Rebase", "开始变基"))
                                .on_click(cx.listener(|this, _, _, cx| this.apply_rebase(cx))),
                        )
                        .child(
                            Button::new("rebase-cancel")
                                .ghost()
                                .compact()
                                .label(tr("Cancel", "取消"))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_rebase(cx))),
                        ),
                ),
        )
    }
}
