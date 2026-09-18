use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit::component::ActiveTheme;

use crate::ui::app::actions::FocusComposer;
use crate::ui::app::{AppView, PromptKind};
use crate::ui::components::{group_header, menu_row};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    /// Alt+` VCS 操作快切弹层（对标 JetBrains VCS Operations Popup）：
    /// 一览全部高频 VCS 动作并附带键位提示，动作执行后自动关闭。
    pub(crate) fn render_vcs_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.state.vcs_palette {
            return None;
        }
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let head = self.state.head_id.clone();

        let mut items: Vec<AnyElement> = Vec::new();
        items.push(
            self.palette_item(
                "pal-commit",
                Ic::Commit,
                tr("Commit changes…", "提交更改…"),
                "Ctrl+K",
                cx,
                |this, window, cx| {
                    this.on_focus_composer(&FocusComposer, window, cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-push",
                Ic::Push,
                tr("Push", "推送"),
                "Ctrl+Shift+K",
                cx,
                |this, _, cx| {
                    this.do_push(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-pull",
                Ic::Pull,
                tr("Pull", "拉取"),
                "Ctrl+T",
                cx,
                |this, _, cx| {
                    this.do_pull(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-fetch",
                Ic::Fetch,
                tr("Fetch", "抓取"),
                "",
                cx,
                |this, _, cx| {
                    this.run_op_progress(
                        tr("Fetch", "抓取"),
                        tr("Fetched", "已抓取"),
                        |repo, progress, cancel| repo.fetch_with_control(progress, cancel),
                        cx,
                    );
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-stash",
                Ic::Shelve,
                tr("Stash changes…", "贮藏更改…"),
                "",
                cx,
                |this, _, cx| {
                    this.state.prompt_stash_keep_index = false;
                    this.state.prompt_stash_include_untracked = true;
                    this.open_prompt(PromptKind::Stash, cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-unstash",
                Ic::Unshelve,
                tr("Unstash latest", "恢复最近的贮藏"),
                "",
                cx,
                |this, _, cx| {
                    this.run_op(tr("Unstashed", "已恢复贮藏"), |repo| repo.stash_pop(), cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-branch",
                Ic::Branch,
                tr("New branch…", "新建分支…"),
                "",
                cx,
                |this, _, cx| {
                    this.open_prompt(PromptKind::NewBranch { start_point: None }, cx);
                },
            )
            .into_any_element(),
        );
        if let Some(head) = head {
            items.push(
                self.palette_item(
                    "pal-tag",
                    Ic::Tag,
                    tr("New tag on HEAD…", "在 HEAD 上新建标签…"),
                    "",
                    cx,
                    move |this, _, cx| {
                        this.open_prompt(
                            PromptKind::NewTag {
                                commit_id: head.clone(),
                            },
                            cx,
                        );
                    },
                )
                .into_any_element(),
            );
        }
        items.push(
            self.palette_item(
                "pal-goto",
                Ic::Search,
                tr("Go to commit…", "跳转到提交…"),
                "",
                cx,
                |this, _, cx| {
                    this.open_prompt(PromptKind::GoTo, cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-blame",
                Ic::Blame,
                tr("Blame current file", "追溯当前文件"),
                "Ctrl+Alt+B",
                cx,
                |this, _, cx| {
                    this.blame_current_file(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-reflog",
                Ic::History,
                tr("Show reflog", "显示引用日志"),
                "",
                cx,
                |this, _, cx| {
                    this.open_reflog(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-conflicts",
                Ic::Conflict,
                tr("Show conflicts", "显示冲突"),
                "Ctrl+Alt+8",
                cx,
                |this, _, cx| {
                    this.open_conflicts(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-shelves",
                Ic::Shelve,
                tr("Show shelves", "显示搁置"),
                "Ctrl+Alt+6",
                cx,
                |this, _, cx| {
                    this.open_shelves(cx);
                },
            )
            .into_any_element(),
        );
        items.push(
            self.palette_item(
                "pal-refresh",
                Ic::Refresh,
                tr("Refresh repository", "刷新仓库"),
                "F5",
                cx,
                |this, _, cx| {
                    this.refresh(cx);
                },
            )
            .into_any_element(),
        );

        Some(
            div()
                .absolute()
                .inset_0()
                .bg(theme::overlay_bg())
                .flex()
                .items_start()
                .justify_center()
                .pt(px(theme::PALETTE_TOP))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(theme::PALETTE_WIDTH))
                        .max_h(px(theme::PALETTE_MAX_HEIGHT))
                        .rounded(px(theme::RADIUS_LG))
                        .border_1()
                        .border_color(border)
                        .bg(theme::popover_bg())
                        .p(px(theme::SPACE_XS))
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_XS))
                        .shadow_lg()
                        .child(group_header(tr("VCS Operations", "VCS 操作"), muted))
                        .child(
                            div()
                                .px(px(theme::SPACE_SM))
                                .pb(px(theme::SPACE_XS))
                                .text_size(px(theme::FONT_SIZE_META))
                                .text_color(muted)
                                .child(tr("Alt+` toggle · Esc to close", "Alt+` 切换 · Esc 关闭")),
                        )
                        .child(
                            div()
                                .id("vcs-palette-list")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .flex()
                                .flex_col()
                                .gap(px(theme::SPACE_XS))
                                .children(items),
                        ),
                )
                .into_any_element(),
        )
    }

    /// 快切弹层的单行动作条目：图标列 + 名称 + 键位提示。
    ///
    /// 复用菜单行原语 [`menu_row`]，与右键菜单保持同一行高/图标列/快捷键列。
    fn palette_item(
        &self,
        id: &'static str,
        icon: Ic,
        label: &'static str,
        keys: &'static str,
        cx: &mut Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        menu_row(id, fg, icon, label, (!keys.is_empty()).then_some(keys)).on_click(cx.listener(
            move |this, _, window, cx| {
                this.state.vcs_palette = false;
                run(this, window, cx);
            },
        ))
    }
}
