use gpui::{
    Context, Div, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
};

use crate::ui::app::AppView;
use crate::ui::components::{empty_state, list_row, panel_header, row_icon_button};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_shelve_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let mut panel = div()
            .id("shelve-panel")
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .size_full()
            .overflow_y_scroll()
            .child(panel_header(tr("Shelves", "搁置"), muted, Vec::new()));

        if self.state.shelves.is_empty() {
            panel = panel.child(empty_state(
                tr(
                    "Nothing on the shelf. Use Shelve in the commit composer.",
                    "搁置区为空。在提交区使用“搁置”。",
                ),
                muted,
            ));
        }

        for entry in &self.state.shelves {
            let index = entry.index;
            panel = panel.child(
                list_row(format!("shelve-{index}"), fg)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(theme::FONT_SIZE_BODY))
                            .text_ellipsis()
                            .overflow_hidden()
                            .child(entry.message.clone()),
                    )
                    .child(
                        row_icon_button(
                            ("shelve-apply", index),
                            Ic::Unshelve,
                            tr("Unshelve", "恢复搁置"),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.unshelve_at(index, cx))),
                    )
                    .child(
                        row_icon_button(("shelve-drop", index), Ic::Delete, tr("Drop", "丢弃"))
                            .danger()
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.drop_shelve_at(index, cx)),
                            ),
                    ),
            );
        }

        panel.child(
            Button::new("shelve-reload")
                .ghost()
                .compact()
                .label(tr("Reload", "刷新"))
                .on_click(cx.listener(|this, _, _, cx| this.reload_shelves(cx))),
        )
    }
}
