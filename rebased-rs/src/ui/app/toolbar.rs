use std::sync::atomic::Ordering;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, hsla, px, AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WeakEntity,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants, DropdownButton},
    input::Textarea,
    list::List,
    menu::{ContextMenuExt, PopupMenuItem},
    ActiveTheme,
};

use rebased_rs::git::{Branch, Change, ChangeStatus, MergeMode};

use crate::ui::graph_view::{lane_color, status_color};

use super::{AppView, ConfirmAction, PromptKind};

impl AppView {
    /// 分支菜单条目的展示文案：`name → upstream ↑ahead ↓behind`。
    fn tracking_label(branch: &Branch) -> String {
        let mut label = branch.name.clone();
        if let Some(upstream) = branch.upstream.as_deref() {
            label.push_str(&format!(" → {upstream}"));
        }
        if branch.ahead > 0 {
            label.push_str(&format!(" ↑{}", branch.ahead));
        }
        if branch.behind > 0 {
            label.push_str(&format!(" ↓{}", branch.behind));
        }
        label
    }

    pub(crate) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let branch_entries = self.state.branch_entries.clone();
        let tags = self.state.tags.clone();
        let current = self.state.current_branch.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let tag_weak = weak.clone();
        let filter_weak = weak.clone();
        let branch_names: Vec<String> = branch_entries.iter().map(|b| b.name.clone()).collect();
        let filter_branch = self.state.filter_branch.clone();
        let date_weak = weak.clone();
        let filter_since = self.state.filter_since.clone();
        let mut branch_label = current
            .clone()
            .unwrap_or_else(|| "main".to_string());
        if self.state.ahead > 0 {
            branch_label.push_str(&format!(" ↑{}", self.state.ahead));
        }
        if self.state.behind > 0 {
            branch_label.push_str(&format!(" ↓{}", self.state.behind));
        }

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
                        // 本地分支：点击切换，当前分支打勾并展示 tracking 状态。
                        for branch in branch_entries.iter().filter(|b| !b.is_remote) {
                            let label = Self::tracking_label(branch);
                            let is_current = branch.is_current();
                            let weak = weak.clone();
                            let name = branch.name.clone();
                            result = result.item(
                                PopupMenuItem::new(label)
                                    .checked(is_current)
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.checkout_branch(&name, cx)
                                        });
                                    }),
                            );
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
                                        this.open_prompt(
                                            PromptKind::Confirm(ConfirmAction::ForcePush),
                                            cx,
                                        )
                                    });
                                }
                            }),
                        );
                        // 本地分支操作：Merge into / Rebase onto / Delete。
                        for branch in branch_entries.iter().filter(|b| !b.is_remote) {
                            if branch.is_current() {
                                continue;
                            }
                            let name = branch.name.clone();
                            let target = current.clone().unwrap_or_else(|| "HEAD".to_string());
                            let merge_specs = [
                                (format!("⇄ Merge {name} into {target}"), MergeMode::Default),
                                (
                                    format!("⇄ Merge {name} into {target} (no ff)…"),
                                    MergeMode::NoFastForward,
                                ),
                                (
                                    format!("⇄ Merge {name} into {target} (ff only)"),
                                    MergeMode::FastForwardOnly,
                                ),
                            ];
                            for (merge_label, mode) in merge_specs {
                                result = result.item(PopupMenuItem::new(merge_label).on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update_in(cx, |this, window, cx| {
                                            if mode == MergeMode::NoFastForward {
                                                this.open_merge_message(name.clone(), window, cx);
                                            } else {
                                                this.merge_branch_into_current(
                                                    name.clone(),
                                                    mode,
                                                    None,
                                                    cx,
                                                );
                                            }
                                        });
                                    }
                                }));
                            }
                            result = result.item(
                                PopupMenuItem::new(format!(
                                    "⇅ Rebase onto {name}…"
                                ))
                                .on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.rebase_current_onto(name.clone(), cx)
                                        });
                                    }
                                }),
                            );
                            let compare_label = format!(
                                "⇋ Compare {name} with {}",
                                current.clone().unwrap_or_else(|| "HEAD".to_string())
                            );
                            result = result.item(PopupMenuItem::new(compare_label).on_click({
                                let weak = weak.clone();
                                let name = name.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_branch_compare(name.clone(), cx)
                                    });
                                }
                            }));
                            result = result.item(PopupMenuItem::new(format!(
                                "✎ Rename {name}…"
                            )).on_click({
                                let weak = weak.clone();
                                let name = name.clone();
                                move |_, _, cx| {
                                    let _ = weak.update_in(cx, |this, window, cx| {
                                        this.open_rename_branch_by_name(name.clone(), window, cx)
                                    });
                                }
                            }));
                            result = result.item(PopupMenuItem::new(format!("✕ {name}")).on_click(
                                {
                                    let weak = weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::Confirm(ConfirmAction::DeleteBranch {
                                                    name: name.clone(),
                                                }),
                                                cx,
                                            )
                                        });
                                    }
                                },
                            ));
                        }
                        // 远程分支：checkout / Pull into / Rebase onto。
                        let remotes: Vec<&Branch> =
                            branch_entries.iter().filter(|b| b.is_remote).collect();
                        if !remotes.is_empty() {
                            result = result.separator();
                            result = result.item(PopupMenuItem::label("Remote"));
                            for branch in remotes {
                                let name = branch.name.clone();
                                result = result.item(
                                    PopupMenuItem::new(format!("⇥ Checkout {name}")).on_click({
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.checkout_branch(&name, cx)
                                            });
                                        }
                                    }),
                                );
                                let pull_label = format!(
                                    "⇄ Pull {name} into {}",
                                    current.clone().unwrap_or_else(|| "HEAD".to_string())
                                );
                                result = result.item(PopupMenuItem::new(pull_label).on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.pull_branch_into_current(name.clone(), cx)
                                        });
                                    }
                                }));
                                result = result.item(
                                    PopupMenuItem::new(format!("⇅ Rebase onto {name}…"))
                                        .on_click({
                                            let weak = weak.clone();
                                            let name = name.clone();
                                            move |_, _, cx| {
                                                let _ = weak.update(cx, |this, cx| {
                                                    this.rebase_current_onto(name.clone(), cx)
                                                });
                                            }
                                        }),
                                );
                                let compare_label = format!(
                                    "⇋ Compare {name} with {}",
                                    current.clone().unwrap_or_else(|| "HEAD".to_string())
                                );
                                result = result.item(PopupMenuItem::new(compare_label).on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_branch_compare(name.clone(), cx)
                                        });
                                    }
                                }));
                            }
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
                DropdownButton::new("branch-filter-menu")
                    .button(
                        Button::new("branch-filter-button").ghost().label(format!(
                            "◫ {}",
                            filter_branch
                                .clone()
                                .unwrap_or_else(|| "All branches".to_string())
                        )),
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(
                            PopupMenuItem::new("All branches")
                                .checked(filter_branch.is_none())
                                .on_click({
                                    let weak = filter_weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_branch_filter(None, cx)
                                        });
                                    }
                                }),
                        );
                        for name in branch_names.iter() {
                            let checked = filter_branch.as_deref() == Some(name.as_str());
                            let weak = filter_weak.clone();
                            let name = name.clone();
                            result = result.item(
                                PopupMenuItem::new(name.clone())
                                    .checked(checked)
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_branch_filter(Some(name.clone()), cx)
                                        });
                                    }),
                            );
                        }
                        result
                    }),
            )
            .child(
                Button::new("author-filter")
                    .ghost()
                    .label(format!(
                        "👤 {}",
                        if self.state.filter_author.is_empty() {
                            "All authors".to_string()
                        } else {
                            self.state.filter_author.clone()
                        }
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_prompt(PromptKind::FilterAuthor, cx)
                    })),
            )
            .child(
                DropdownButton::new("date-filter-menu")
                    .button(
                        Button::new("date-filter-button").ghost().label(format!(
                            "📅 {}",
                            filter_since
                                .as_ref()
                                .map(|(label, _)| label.clone())
                                .unwrap_or_else(|| "All time".to_string())
                        )),
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(
                            PopupMenuItem::new("All time")
                                .checked(filter_since.is_none())
                                .on_click({
                                    let weak = date_weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_date_filter(None, cx)
                                        });
                                    }
                                }),
                        );
                        let date_specs = [
                            ("Today", "midnight"),
                            ("This week", "1 week ago"),
                            ("This month", "1 month ago"),
                            ("This year", "1 year ago"),
                        ];
                        for (label, expr) in date_specs {
                            let checked =
                                filter_since.as_ref().map(|(_, e)| e.as_str()) == Some(expr);
                            let weak = date_weak.clone();
                            result = result.item(
                                PopupMenuItem::new(label)
                                    .checked(checked)
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_date_filter(
                                                Some((label.to_string(), expr.to_string())),
                                                cx,
                                            )
                                        });
                                    }),
                            );
                        }
                        result
                    }),
            )
            .child(
                Button::new("goto")
                    .ghost()
                    .label("→ Go to…")
                    .on_click(cx.listener(|this, _, _, cx| this.open_prompt(PromptKind::GoTo, cx))),
            )
            .child(
                Button::new("fetch").ghost().label("Fetch").on_click(
                    cx.listener(|this, _, _, cx| {
                        this.run_op_progress(
                            "Fetch",
                            "Fetched",
                            |repo, progress, cancel| repo.fetch_with_control(progress, cancel),
                            cx,
                        )
                    }),
                ),
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
        let list = self.list.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        div()
            .id("commit-panel")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(List::new(&self.list))
            .context_menu(move |menu, _window, cx| {
                let row = list.read(cx).right_clicked_index().map(|ix| ix.row);
                let Some(commit) = row.and_then(|row| list.read(cx).delegate().commit_at(row))
                else {
                    return menu;
                };
                let id = commit.id.0.clone();
                let short = id[..id.len().min(7)].to_string();
                let is_head = weak
                    .upgrade()
                    .and_then(|app| app.read(cx).state.head_id.clone())
                    .as_deref()
                    == Some(id.as_str());
                let mut result = menu.item(
                    PopupMenuItem::new(format!("Checkout {short}")).on_click({
                        let weak = weak.clone();
                        let id = id.clone();
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.checkout_commit(id.clone(), cx)
                            });
                        }
                    }),
                );
                result = result.item(PopupMenuItem::new("New Branch…").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_prompt(
                                PromptKind::NewBranch {
                                    start_point: Some(id.clone()),
                                },
                                cx,
                            )
                        });
                    }
                }));
                result = result.item(PopupMenuItem::new("New Tag…").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_prompt(
                                PromptKind::NewTag {
                                    commit_id: id.clone(),
                                },
                                cx,
                            )
                        });
                    }
                }));
                result = result.separator();
                result = result.item(PopupMenuItem::new("Cherry-pick").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.cherry_pick_commit(id.clone(), cx)
                        });
                    }
                }));
                result = result.item(PopupMenuItem::new("Revert Commit").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.revert_commit(id.clone(), cx));
                    }
                }));
                result = result.item(
                    PopupMenuItem::new("Reset Current Branch to Here…").on_click({
                        let weak = weak.clone();
                        let id = id.clone();
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.open_prompt(
                                    PromptKind::Reset {
                                        commit_id: id.clone(),
                                    },
                                    cx,
                                )
                            });
                        }
                    }),
                );
                result = result.item(PopupMenuItem::new("Rebase from Here").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.start_rebase(id.clone(), cx));
                    }
                }));
                result = result.item(PopupMenuItem::new("Reword Message…").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_prompt(
                                PromptKind::Reword {
                                    commit_id: id.clone(),
                                },
                                cx,
                            )
                        });
                    }
                }));
                result = result.separator();
                result = result.item(PopupMenuItem::new("Diff").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_commit_diff(id.clone(), None, cx)
                        });
                    }
                }));
                result = result.item(PopupMenuItem::new("Copy SHA").on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.copy_commit_id(id.clone(), cx));
                    }
                }));
                if is_head {
                    result = result.item(PopupMenuItem::new("Undo Commit").on_click({
                        let weak = weak.clone();
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.open_prompt(
                                    PromptKind::Confirm(ConfirmAction::UndoHeadCommit),
                                    cx,
                                )
                            });
                        }
                    }));
                    result = result.item(PopupMenuItem::new("Drop Commit").on_click({
                        let weak = weak.clone();
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.open_prompt(
                                    PromptKind::Confirm(ConfirmAction::DropHeadCommit),
                                    cx,
                                )
                            });
                        }
                    }));
                }
                result
            })
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
        let checkbox_path = change.path.clone();
        let checked = self.state.selected_changes.contains(&change.path);

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
                Button::new(format!("chg-select-{index}"))
                    .ghost()
                    .compact()
                    .label(if checked { "☑" } else { "☐" })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_change_selection(&checkbox_path, cx);
                    })),
            )
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
                    if staged {
                        this.open_staged_diff(Some(path), cx);
                    } else {
                        this.open_unstaged_diff(Some(path), cx);
                    }
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
                        this.open_prompt(
                            PromptKind::Confirm(ConfirmAction::DiscardChanges {
                                path: discard_path.clone(),
                            }),
                            cx,
                        );
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
        let muted = cx.theme().muted_foreground;
        let count = self.state.changes.len();

        let mut rows: Vec<AnyElement> = Vec::new();
        let mut next_index = 0usize;

        let mut section = |label: &'static str, group: &[&Change], accumulator: &mut Vec<AnyElement>| {
            if group.is_empty() {
                return;
            }
            accumulator.push(
                div()
                    .flex_none()
                    .px_2()
                    .pt_2()
                    .pb_0p5()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(format!("{label} ({})", group.len()))
                    .into_any_element(),
            );
            for change in group {
                accumulator.push(self.render_change_row(next_index, change, cx));
                next_index += 1;
            }
        };

        let staged: Vec<&Change> = self.state.changes.iter().filter(|c| c.staged).collect();
        let unstaged: Vec<&Change> = self
            .state
            .changes
            .iter()
            .filter(|c| !c.staged)
            .collect();
        section("Unstaged", &unstaged, &mut rows);
        section("Staged", &staged, &mut rows);

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
        let selected_count = self.state.selected_changes.len();
        let commit_label = if selected_count > 0 {
            format!("Commit ({selected_count})")
        } else {
            "Commit".to_string()
        };
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
                            .on_click(cx.listener(|this, _, window, cx| {
                                let enabling = !this.state.amend;
                                this.state.amend = enabling;
                                if enabling {
                                    this.prefill_amend_message(window, cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("commit")
                            .primary()
                            .label(commit_label)
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
                match (
                    &self.state.error,
                    self.state.busy.clone(),
                    self.state.status_message.is_empty(),
                ) {
                    (Some(error), _, _) => div()
                        .text_color(hsla(0.0, 0.75, 0.55, 1.0))
                        .child(error.clone())
                        .into_any_element(),
                    (None, Some(busy), _) => {
                        let mut busy_row = div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .text_color(muted);
                        match &self.state.progress_text {
                            Some(text) => busy_row = busy_row.child(format!("⏳ {busy}… {text}")),
                            None => busy_row = busy_row.child(format!("⏳ {busy}…")),
                        }
                        if let Some(token) = self.state.cancel_token.clone() {
                            busy_row = busy_row.child(
                                Button::new("cancel-op")
                                    .ghost()
                                    .text_xs()
                                    .label("Cancel")
                                    .on_click(move |_, _, _| {
                                        token.store(true, Ordering::SeqCst);
                                    }),
                            );
                        }
                        busy_row.into_any_element()
                    }
                    (None, None, false) => div()
                        .text_color(muted)
                        .child(self.state.status_message.clone())
                        .into_any_element(),
                    (None, None, true) => div().into_any_element(),
                },
            )
            .child(div().flex_1())
            .when(!self.state.repo_root.is_empty(), |bar| {
                bar.child(
                    div()
                        .max_w(px(420.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(muted)
                        .child(self.state.repo_root.clone()),
                )
            })
            .child(
                div().text_color(muted).child(format!(
                    "⎇ {}",
                    self.state
                        .current_branch
                        .clone()
                        .unwrap_or_else(|| "HEAD (detached)".to_string())
                )),
            )
            .when(self.state.ahead > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↑{}", self.state.ahead)))
            })
            .when(self.state.behind > 0, |bar| {
                bar.child(div().text_color(muted).child(format!("↓{}", self.state.behind)))
            })
    }
}
