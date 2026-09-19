use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, Size, input::Input};

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
                                .text_size(px(theme::font_size_meta()))
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

    /// 工具栏分支部件弹窗（IDEA 风格）：搜索框「搜索分支和操作」+
    /// 五个高频动作（更新项目/提交/推送/新建分支/签出标记或修订）+
    /// 可折叠的「本地/远程」分支分组。当前分支行展示彩色强调、
    /// ahead/behind 计数与 upstream tracking（`origin/main >`）。
    pub(crate) fn render_branch_popup(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.state.branch_popup {
            return None;
        }
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        // 当前分支强调色：深金色（JetBrains 用于标记 HEAD / 当前分支）。
        let current_color = gpui::Hsla {
            h: 48.0,
            s: 0.85,
            l: 0.62,
            a: 1.0,
        };
        let upstream_color = gpui::Hsla {
            h: 142.0,
            s: 0.65,
            l: 0.58,
            a: 1.0,
        };
        let query = self
            .branch_popup_query
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let hit = |label: &str| query.is_empty() || label.to_lowercase().contains(&query);

        type PopupAction = (
            &'static str,
            Ic,
            &'static str,
            Option<&'static str>,
            Box<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>,
        );
        let actions: Vec<PopupAction> = vec![
            (
                "branch-pop-pull",
                Ic::Pull,
                tr("Update Project", "更新项目"),
                Some("Ctrl+T"),
                Box::new(|this, _, cx| this.do_pull(cx)),
            ),
            (
                "branch-pop-commit",
                Ic::Commit,
                tr("Commit…", "提交…"),
                Some("Ctrl+K"),
                Box::new(|this, window, cx| {
                    this.on_focus_composer(&FocusComposer, window, cx);
                }),
            ),
            (
                "branch-pop-push",
                Ic::Push,
                tr("Push…", "推送…"),
                Some("Ctrl+Shift+K"),
                Box::new(|this, _, cx| this.do_push(cx)),
            ),
            (
                "branch-pop-new",
                Ic::Add,
                tr("New Branch…", "新建分支…"),
                Some("Ctrl+Alt+N"),
                Box::new(|this, _, cx| {
                    this.open_prompt(PromptKind::NewBranch { start_point: None }, cx);
                }),
            ),
            (
                "branch-pop-checkout",
                Ic::Checkout,
                tr("Checkout Tag or Revision…", "签出标记或修订…"),
                None,
                Box::new(|this, _, cx| {
                    this.open_prompt(PromptKind::NewBranch { start_point: None }, cx);
                }),
            ),
        ];

        let mut items: Vec<AnyElement> = Vec::new();
        for (id, icon, label, shortcut, run) in actions {
            if !hit(label) {
                continue;
            }
            items.push(
                menu_row(id, fg, icon, label, shortcut)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.state.branch_popup = false;
                        run(this, window, cx);
                    }))
                    .into_any_element(),
            );
        }

        // —— 本地分支（可折叠 chevron 分组） ——
        let locals: Vec<rebased_rs::git::Branch> = self
            .state
            .branch_entries
            .iter()
            .filter(|b| !b.is_remote && hit(&b.name))
            .cloned()
            .collect();
        if !locals.is_empty() {
            let locals_expanded = self.state.branch_popup_locals_expanded;
            let locals_weak = cx.entity().downgrade();
            items.push(
                div()
                    .id("branch-pop-locals-header")
                    .w_full()
                    .h(px(theme::MENU_ROW_HEIGHT))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .px(px(theme::SPACE_SM))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(move |s| s.bg(theme::hover_bg(fg)))
                    .child(Icon::new(if locals_expanded { Ic::ChevronDown } else { Ic::ChevronRight })
                        .with_size(Size::XSmall))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(theme::font_size_meta()))
                            .font_weight(theme::WEIGHT_MEDIUM)
                            .text_color(muted)
                            .child(tr("Local", "本地")),
                    )
                    .on_click(move |_, _, cx| {
                        let _ = locals_weak.update(cx, |this, _| {
                            this.state.branch_popup_locals_expanded =
                                !this.state.branch_popup_locals_expanded;
                        });
                    })
                    .into_any_element(),
            );
            if locals_expanded {
                for branch in locals {
                    items.push(self.render_branch_popup_local_row(
                        branch,
                        current_color,
                        upstream_color,
                        cx,
                    ));
                }
            }
        }

        // —— 远程分支（可折叠 chevron 分组） ——
        let remotes: Vec<rebased_rs::git::Branch> = self
            .state
            .branch_entries
            .iter()
            .filter(|b| b.is_remote && hit(&b.name))
            .cloned()
            .collect();
        if !remotes.is_empty() {
            let remotes_expanded = self.state.branch_popup_remotes_expanded;
            let remotes_weak = cx.entity().downgrade();
            items.push(
                div()
                    .id("branch-pop-remotes-header")
                    .w_full()
                    .h(px(theme::MENU_ROW_HEIGHT))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .px(px(theme::SPACE_SM))
                    .rounded(px(theme::RADIUS))
                    .cursor_pointer()
                    .hover(move |s| s.bg(theme::hover_bg(fg)))
                    .child(Icon::new(if remotes_expanded { Ic::ChevronDown } else { Ic::ChevronRight })
                        .with_size(Size::XSmall))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(theme::font_size_meta()))
                            .font_weight(theme::WEIGHT_MEDIUM)
                            .text_color(muted)
                            .child(tr("Remote", "远程")),
                    )
                    .on_click(move |_, _, cx| {
                        let _ = remotes_weak.update(cx, |this, _| {
                            this.state.branch_popup_remotes_expanded =
                                !this.state.branch_popup_remotes_expanded;
                        });
                    })
                    .into_any_element(),
            );
            if remotes_expanded {
                for branch in remotes {
                    let name = branch.name.clone();
                    items.push(
                        menu_row(
                            SharedString::from(format!("branch-pop-remote-{name}")),
                            fg,
                            Ic::Branch,
                            name.clone(),
                            None,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.branch_popup = false;
                            this.checkout_branch(&name, cx);
                        }))
                        .into_any_element(),
                    );
                }
            }
        }

        let weak = cx.entity().downgrade();
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(theme::transparent())
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = weak.update(cx, |this, _| {
                        this.state.branch_popup = false;
                    });
                })
                .child(
                    div()
                        .absolute()
                        .top(px(theme::TOOLBAR_HEIGHT + theme::SPACE_XS))
                        .left(px(theme::SPACE_SM))
                        .w(px(theme::BRANCH_POPUP_WIDTH))
                        .max_h(px(theme::PALETTE_MAX_HEIGHT))
                        .rounded(px(theme::RADIUS_LG))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(theme::popover_bg())
                        .p(px(theme::SPACE_XS))
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_XS))
                        .shadow_lg()
                        // 面板内按下不冒泡到遮罩：否则 mousedown 先触发遮罩关闭，
                        // 弹层子树在 mouseup 前被移除，行内 on_click 永远无法完成。
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex_none()
                                .px(px(theme::SPACE_XS))
                                .pb(px(theme::SPACE_XS))
                                .child(
                                    Input::new(&self.branch_popup_query)
                                        .with_size(Size::Small)
                                        .prefix(
                                            Icon::new(Ic::Search)
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .cleanable(true),
                                ),
                        )
                        .child(
                            div()
                                .id("branch-popup-list")
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

    /// 分支弹层里的本地分支行：当前分支用金色强调 + ahead/behind
    /// 计数 badge + upstream tracking（`origin/main >`）右对齐。
    fn render_branch_popup_local_row(
        &self,
        branch: rebased_rs::git::Branch,
        current_color: gpui::Hsla,
        upstream_color: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let is_current = branch.is_current();
        let name = branch.name.clone();
        let row_id = format!("branch-pop-local-{name}");
        let label_color = if is_current { current_color } else { fg };
        let row = div()
            .id(row_id)
            .w_full()
            .h(px(theme::MENU_ROW_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_MD))
            .px(px(theme::SPACE_SM))
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .hover(move |s| s.bg(theme::hover_bg(fg)))
            // 图标列：固定宽度，与弹出菜单图标列对齐。
            .child(
                div()
                    .flex_none()
                    .w(px(theme::ICON_SIZE_MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(Ic::Branch).with_size(Size::Small)),
            )
            // 标签区：当前分支色 + ahead/behind badge。
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(theme::font_size_body()))
                            .text_color(label_color)
                            .child(name.clone()),
                    ),
            );
        // 右侧附加信息：upstream tracking `origin/main >` + ahead/behind。
        let mut extras = Vec::new();
        if let Some(upstream) = &branch.upstream {
            extras.push(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(upstream_color)
                    .child(format!("{upstream} >")),
            );
        }
        let ab_text = match (branch.ahead, branch.behind) {
            (0, 0) => None,
            (a, 0) => Some(format!("↑{a}")),
            (0, b) => Some(format!("↓{b}")),
            (a, b) => Some(format!("↑{a} ↓{b}")),
        };
        if let Some(text) = ab_text {
            extras.push(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(muted)
                    .child(text),
            );
        }
        let row = row.child(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(theme::SPACE_MD))
                .children(extras),
        );
        row.on_click(cx.listener(move |this, _, _, cx| {
            this.state.branch_popup = false;
            this.checkout_branch(&name, cx);
        }))
        .into_any_element()
    }
}
