use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WeakEntity,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants, DropdownButton},
    input::Textarea,
    list::List,
    menu::PopupMenuItem,
    ActiveTheme,
};

use rebased_rs::git::{Change, ChangeStatus};

use crate::ui::graph_view::{lane_color, status_color};

use super::{AppView, PromptKind};

impl AppView {
    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let branches = self.state.branches.clone();
        let tags = self.state.tags.clone();
        let current = self.state.current_branch.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let tag_weak = weak.clone();
        let branch_label = current
            .clone()
            .unwrap_or_else(|| "main".to_string());

        div()
            .h(px(44.))
            .flex_none()
            .border_b_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .child(
                DropdownButton::new("branch-menu")
                    .button(Button::new("branch-button").ghost().label(branch_label))
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu;
                        for name in branches.iter() {
                            let label = if Some(name.as_str()) == current.as_deref() {
                                format!("● {name}")
                            } else {
                                name.clone()
                            };
                            let weak = weak.clone();
                            let name = name.clone();
                            result = result.item(PopupMenuItem::new(label).on_click(
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.checkout_branch(&name, cx)
                                    });
                                },
                            ));
                        }
                        result = result.separator();
                        result = result.item(PopupMenuItem::new("＋ New branch…").on_click({
                            let weak = weak.clone();
                            move |_, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.open_prompt(PromptKind::NewBranch { start_point: None }, cx)
                                });
                            }
                        }));
                        result = result.item(
                            PopupMenuItem::new("✎ Rename current branch…").on_click({
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::RenameBranch, cx)
                                    });
                                }
                            }),
                        );
                        result = result.item(
                            PopupMenuItem::new("⇪ Force push (with lease)").on_click({
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.force_push_current(cx)
                                    });
                                }
                            }),
                        );
                        for name in branches.iter() {
                            if Some(name.as_str()) == current.as_deref() {
                                continue;
                            }
                            let weak = weak.clone();
                            let name = name.clone();
                            let merge_label = format!(
                                "⇄ Merge {name} into {}",
                                current.clone().unwrap_or_else(|| "HEAD".to_string())
                            );
                            result = result.item(PopupMenuItem::new(merge_label).on_click({
                                let weak = weak.clone();
                                let name = name.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.merge_branch_into_current(name.clone(), cx)
                                    });
                                }
                            }));
                            result = result.item(PopupMenuItem::new(format!("✕ {name}")).on_click(
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.delete_branch(&name, cx)
                                    });
                                },
                            ));
                        }
                        result
                    }),
            )
            .child(
                DropdownButton::new("tag-menu")
                    .button(Button::new("tag-button").ghost().label("Tags"))
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(PopupMenuItem::new("＋ New tag on HEAD…").on_click({
                            let weak = tag_weak.clone();
                            move |_, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.open_prompt(
                                        PromptKind::NewTag { commit_id: "HEAD".to_string() },
                                        cx,
                                    )
                                });
                            }
                        }));
                        result = result.item(
                            PopupMenuItem::new("⇪ Push all tags to origin").on_click({
                                let weak = tag_weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| this.push_all_tags(cx));
                                }
                            }),
                        );
                        if !tags.is_empty() {
                            result = result.separator();
                            for tag in tags.iter() {
                                let weak = tag_weak.clone();
                                let commit_id = tag.commit_id.clone();
                                let label = tag.name.clone();
                                result = result.item(PopupMenuItem::new(label).on_click(
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.select_commit_by_id(&commit_id, cx)
                                        });
                                    },
                                ));
                            }
                        }
                        result
                    }),
            )
            .child(
                Button::new("fetch")
                    .ghost()
                    .label("Fetch")
                    .on_click(cx.listener(|this, _, _, cx| this.run_op("Fetched", |repo| repo.fetch(), cx))),
            )
            .child(
                Button::new("pull")
                    .ghost()
                    .label("Pull")
                    .on_click(cx.listener(|this, _, _, cx| this.do_pull(cx))),
            )
            .child(
                Button::new("push")
                    .ghost()
                    .label("Push")
                    .on_click(cx.listener(|this, _, _, cx| this.do_push(cx))),
            )
            .child(
                Button::new("stash")
                    .ghost()
                    .label("Stash")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_prompt(PromptKind::Stash, cx)
                    })),
            )
            .child(
                Button::new("unstash")
                    .ghost()
                    .label("Unstash")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_op("Unstashed", |repo| repo.stash_pop(), cx)
                    })),
            )
            .child(
                Button::new("refresh")
                    .ghost()
                    .label("Refresh")
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(
                Button::new("conflicts")
                    .ghost()
                    .label("Conflicts")
                    .on_click(cx.listener(|this, _, _, cx| this.open_conflicts(cx))),
            )
            .child(
                Button::new("shelves")
                    .ghost()
                    .label("Shelves")
                    .on_click(cx.listener(|this, _, _, cx| this.open_shelves(cx))),
            )
            .when(self.state.rebase.in_progress(), |bar| {
                bar.child(
                    Button::new("rebase-abort")
                        .danger()
                        .compact()
                        .label("Abort Rebase")
                        .on_click(cx.listener(|this, _, _, cx| this.abort_rebase(cx))),
                )
                .child(
                    Button::new("rebase-continue")
                        .danger()
                        .compact()
                        .label("Continue Rebase")
                        .on_click(cx.listener(|this, _, _, cx| this.continue_rebase(cx))),
                )
            })
            .when(self.state.merge_in_progress, |bar| {
                bar.child(
                    Button::new("merge-abort")
                        .danger()
                        .compact()
                        .label("Abort Merge")
                        .on_click(cx.listener(|this, _, _, cx| this.abort_merge(cx))),
                )
                .child(
                    Button::new("merge-continue")
                        .danger()
                        .compact()
                        .label("Continue Merge")
                        .on_click(cx.listener(|this, _, _, cx| this.continue_merge_op(cx))),
                )
            })
    }

    pub(crate) fn render_commit_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.loading {
            return div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Loading repository...")
                .into_any_element();
        }
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(List::new(&self.list))
            .into_any_element()
    }

    pub(crate) fn render_change_row(&self, index: usize, change: &Change, cx: &mut Context<Self>) -> AnyElement {
        let fg = cx.theme().foreground;
        let color = status_color(&change.status);
        let path = change.display_path();
        let staged = change.staged;
        let is_untracked = change.status == ChangeStatus::Untracked;
        let change_for_click = change.clone();
        let diff_path = change.path.clone();

        let mut row = div()
            .id(format!("change-{index}"))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(4.))
            .cursor_pointer()
            .hover(move |style| style.bg(hsla(fg.h, fg.s, fg.l, 0.07)))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_stage(&change_for_click, cx)))
            .child(
                div()
                    .w(px(14.))
                    .flex_none()
                    .text_xs()
                    .text_color(color)
                    .child(change.status.short_label()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_sm()
                    .child(path),
            );

        row = row.child(
            Button::new(format!("chg-diff-{index}"))
                .ghost()
                .compact()
                .label("Δ")
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let path = diff_path.clone();
                    this.open_diff_worktree(Some(path), cx);
                })),
        );
        if !staged && !is_untracked {
            let discard_path = change.path.clone();
            row = row.child(
                Button::new(format!("chg-discard-{index}"))
                    .ghost()
                    .compact()
                    .label("↩")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        let path = discard_path.clone();
                        let message = format!("Discarded {path}");
                        this.run_op(&message, move |repo| repo.discard_changes(&path), cx);
                    })),
            );
        }
        if is_untracked {
            let remove_path = change.path.clone();
            row = row.child(
                Button::new(format!("chg-remove-{index}"))
                    .ghost()
                    .compact()
                    .label("✕")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        let path = remove_path.clone();
                        let message = format!("Removed {path}");
                        this.run_op(&message, move |repo| repo.remove_untracked(&path), cx);
                    })),
            );
        }

        row.child(
            div()
                .flex_none()
                .w(px(12.))
                .text_sm()
                .text_color(if staged { cx.theme().muted_foreground } else { lane_color(2) })
                .child(if staged { "−" } else { "+" }),
        )
        .into_any_element()
    }

    pub(crate) fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let fg = cx.theme().foreground;
        let count = self.state.changes.len();

        let rows: Vec<AnyElement> = self
            .state
            .changes
            .iter()
            .enumerate()
            .map(|(index, change)| self.render_change_row(index, change, cx))
            .collect();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Changes ({count})")),
            )
            .child(
                div()
                    .id("changes-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .px_1()
                    .children(rows)
                    .when(count == 0, |container| {
                        container.child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .text_color(fg.opacity(0.4))
                                .child("Working tree clean"),
                        )
                    }),
            )
    }

    pub(crate) fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let amend_label = if self.state.amend { "✓ Amend" } else { "Amend" };
        div()
            .flex_none()
            .border_t_1()
            .border_color(border)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(Textarea::new(&self.message_input).h(px(72.)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("shelve")
                            .ghost()
                            .label("Shelve…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_prompt(PromptKind::Stash, cx)
                            })),
                    )
                    .child(
                        Button::new("amend")
                            .ghost()
                            .label(amend_label)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.amend = !this.state.amend;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("commit")
                            .primary()
                            .label("Commit")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.do_commit(window, cx)
                            })),
                    ),
            )
    }

    pub(crate) fn render_statusbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        div()
            .h(px(28.))
            .flex_none()
            .border_t_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .text_xs()
            .child(
                match (&self.state.error, self.state.status_message.is_empty()) {
                    (Some(error), _) => div()
                        .text_color(hsla(0.0, 0.75, 0.55, 1.0))
                        .child(error.clone())
                        .into_any_element(),
                    (None, false) => div()
                        .text_color(muted)
                        .child(self.state.status_message.clone())
                        .into_any_element(),
                    (None, true) => div().into_any_element(),
                },
            )
            .child(div().flex_1())
            .when(self.state.ahead > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↑{}", self.state.ahead)))
            })
            .when(self.state.behind > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↓{}", self.state.behind)))
            })
    }
}
