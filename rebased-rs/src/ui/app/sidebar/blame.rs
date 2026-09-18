use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
};

use crate::ui::app::AppView;
use crate::ui::blame_view::{BlameJump, render_blame};
use crate::ui::components::{empty_state, panel_header};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_blame_panel(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let path = self.state.blame_path.clone();

        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_MD))
            .min_h_0()
            .child(panel_header(
                format!("{} · {path}", tr("Blame", "追溯")),
                muted,
                vec![
                    Button::new("blame-close")
                        .ghost()
                        .compact()
                        .icon(Ic::Close)
                        .on_click(cx.listener(|this, _, _, cx| this.sidebar_back(cx)))
                        .into_any_element(),
                ],
            ));

        if self.state.blame_groups.is_empty() {
            panel = panel.child(empty_state(tr("Nothing to blame", "无追溯信息"), muted));
        } else {
            let on_commit: BlameJump = {
                let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                std::sync::Arc::new(move |id, app| {
                    let _ =
                        weak.update(app, |this, cx| this.open_commit_diff(id.clone(), None, cx));
                })
            };
            panel = panel.child(
                div()
                    .id("blame-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_blame(&self.state.blame_groups, Some(&on_commit), cx)),
            );
        }
        panel
    }
}
