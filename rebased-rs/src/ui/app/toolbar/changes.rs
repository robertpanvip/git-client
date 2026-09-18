use std::sync::atomic::Ordering;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    Stateful, StatefulInteractiveElement, Styled, WeakEntity, div, px,
};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    button::{Button, ButtonVariants},
    input::Textarea,
    menu::ContextMenuExt,
};

use rebased_rs::git::{Change, ChangeStatus};

use crate::ui::app::{AppView, ChangesTab, ConfirmAction, PromptKind};
use crate::ui::components::{
    Checkbox, DropdownButton, empty_state, group_header_controls, list_row, menu_item,
    menu_width, row_icon_button, selected, status_bar, status_message, status_segment,
};
use crate::ui::components::tabs::{segment, segmented};
use crate::ui::graph_view::status_color;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_change_row(
        &self,
        index: usize,
        change: &Change,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let color = status_color(&change.status);
        // 文件名与父目录分开渲染：文件名按状态着色且永不省略，父目录灰色、可被压缩。
        let (dir, mut name) = match change.path.rsplit_once('/') {
            Some((dir, name)) => (format!("{dir}/"), name.to_string()),
            None => (String::new(), change.path.clone()),
        };
        if let Some(original) = &change.original_path {
            let old_name = original.rsplit('/').next().unwrap_or(original.as_str());
            if old_name != name {
                name = format!("{old_name} → {name}");
            }
        }
        let staged = change.staged;
        let is_untracked = change.status == ChangeStatus::Untracked;
        let change_for_click = change.clone();
        let diff_path = change.path.clone();
        let checkbox_path = change.path.clone();
        let checked = self.state.selected_changes.contains(&change.path);
        let weak = cx.entity().downgrade();
        let checkbox_weak = weak.clone();

        let menu_change = change.clone();
        let menu_path = change.path.clone();
        let row = list_row(format!("change-{index}"), fg)
            .gap(px(theme::SPACE_SM))
            .when(checked, |row| selected(row, true, fg))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_stage(&change_for_click, cx)))
            .child(
                Checkbox::new(format!("chg-select-{index}"))
                    .checked(checked)
                    .with_size(Size::XSmall)
                    .on_click(move |_, _, app| {
                        app.stop_propagation();
                        let _ = checkbox_weak.update(app, |this, cx| {
                            this.toggle_change_selection(&checkbox_path, cx);
                        });
                    }),
            )
            // 状态标记列：固定宽度，与列表中的图标槽对齐。
            // （IntelliJ 用按状态着色的文件类型图标表达同一信息；本项目未引入
            //  文件类型图标集，改用同色的状态字母占位。）
            .child(
                div()
                    .w(px(theme::ICON_SIZE_SM))
                    .flex_none()
                    .text_size(px(theme::font_size_meta()))
                    .font_weight(theme::WEIGHT_MEDIUM)
                    .text_color(color)
                    .child(change.status.short_label()),
            )
            // 文件名（按状态着色，始终可见） + 父目录（灰色，先被压缩省略）。
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .text_size(px(theme::font_size_body()))
                            .text_color(color)
                            .child(name),
                    )
                    .when(!dir.is_empty(), |cell| {
                        cell.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(theme::font_size_meta()))
                                .text_color(muted)
                                .child(dir),
                        )
                    }),
            );

        let row = row.child(
            row_icon_button(
                format!("chg-diff-{index}"),
                Ic::Diff,
                tr("Show Diff", "查看差异"),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                let path = diff_path.clone();
                if staged {
                    this.open_staged_diff_window(Some(path), cx);
                } else {
                    this.open_unstaged_diff_window(Some(path), cx);
                }
            })),
        );
        let row = if !staged && !is_untracked {
            let discard_path = change.path.clone();
            row.child(
                row_icon_button(
                    format!("chg-discard-{index}"),
                    Ic::Revert,
                    tr("Rollback…", "回滚…"),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_prompt(
                        PromptKind::Confirm(ConfirmAction::DiscardChanges {
                            path: discard_path.clone(),
                        }),
                        cx,
                    );
                })),
            )
        } else {
            row
        };
        let row = if is_untracked {
            let remove_path = change.path.clone();
            row.child(
                row_icon_button(
                    format!("chg-remove-{index}"),
                    Ic::Delete,
                    tr("Delete", "删除"),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let path = remove_path.clone();
                    let message = format!("{} {path}", tr("Removed", "已移除"));
                    this.run_op(&message, move |repo| repo.remove_untracked(&path), cx);
                })),
            )
        } else {
            row
        };

        let menu = row.context_menu(move |menu, _window, _app| {
            let weak = weak.clone();
            let path = menu_path.clone();
            let stage_label = if menu_change.staged {
                tr("Unstage", "取消暂存")
            } else {
                tr("Stage", "暂存")
            };
            let stage_icon = if menu_change.staged {
                Ic::Undo
            } else {
                Ic::Add
            };
            let weak_for_stage = weak.clone();
            let change_for_stage = menu_change.clone();
            let weak_for_diff = weak.clone();
            let path_for_diff = path.clone();
            let mut m = menu
                .item(menu_item(
                    stage_icon,
                    stage_label,
                    None,
                    false,
                    false,
                    move |_, _, app| {
                        let _ = weak_for_stage.update(app, |this, cx| {
                            this.toggle_stage(&change_for_stage, cx);
                        });
                    },
                ))
                .item(menu_item(
                    Ic::Diff,
                    tr("Show Diff", "查看差异"),
                    None,
                    false,
                    false,
                    move |_, _, app| {
                        let _ = weak_for_diff.update(app, |this, cx| {
                            if menu_change.staged {
                                this.open_staged_diff_window(Some(path_for_diff.clone()), cx);
                            } else {
                                this.open_unstaged_diff_window(Some(path_for_diff.clone()), cx);
                            }
                        });
                    },
                ));
            if !menu_change.staged && !is_untracked {
                let weak_for_discard = weak.clone();
                let path_for_discard = path.clone();
                m = m.item(menu_item(
                    Ic::Revert,
                    tr("Rollback…", "回滚…"),
                    None,
                    true,
                    false,
                    move |_, _, app| {
                        let _ = weak_for_discard.update(app, |this, cx| {
                            this.open_prompt(
                                PromptKind::Confirm(ConfirmAction::DiscardChanges {
                                    path: path_for_discard.clone(),
                                }),
                                cx,
                            );
                        });
                    },
                ));
            }
            if is_untracked {
                let weak_for_remove = weak.clone();
                let path_for_remove = path.clone();
                m = m.item(menu_item(
                    Ic::Delete,
                    tr("Delete", "删除"),
                    None,
                    true,
                    false,
                    move |_, _, app| {
                        let _ = weak_for_remove.update(app, |this, cx| {
                            let path = path_for_remove.clone();
                            let message = format!("{} {path}", tr("Removed", "已移除"));
                            this.run_op(&message, move |repo| repo.remove_untracked(&path), cx);
                        });
                    },
                ));
            }
            menu_width(m)
        });

        menu.into_any_element()
    }

    /// 一个变更分组（更改 / 未进行版本管理的文件）：标题行 + 文件行。
    ///
    /// 标题行的前导复选框对该组做全选/全不选，尾部显示条目数——与 IntelliJ
    /// 变更列表的分组行一致（组名左侧可勾选、右侧计数）。
    fn changes_section(
        &self,
        id: &'static str,
        label: &'static str,
        group: &[&Change],
        start_index: usize,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if group.is_empty() {
            return Vec::new();
        }
        let muted = cx.theme().muted_foreground;
        let all_selected = group
            .iter()
            .all(|change| self.state.selected_changes.contains(&change.path));
        let paths: Vec<String> = group.iter().map(|change| change.path.clone()).collect();
        let weak = cx.entity().downgrade();

        let header = group_header_controls(
            label,
            muted,
            Some(
                Checkbox::new(id)
                    .checked(all_selected)
                    .with_size(Size::XSmall)
                    .on_click(move |_, _, app| {
                        app.stop_propagation();
                        let _ = weak.update(app, |this, cx| {
                            this.set_changes_selection(&paths, !all_selected, cx)
                        });
                    })
                    .into_any_element(),
            ),
            Some(
                div()
                    .flex_none()
                    .child(group.len().to_string())
                    .into_any_element(),
            ),
        );

        let mut out: Vec<AnyElement> = Vec::with_capacity(group.len() + 1);
        out.push(header.into_any_element());
        for (offset, change) in group.iter().enumerate() {
            out.push(self.render_change_row(start_index + offset, change, cx));
        }
        out
    }

    /// 变更面板：Commit / Shelve 两个页签（对齐 IDEA 的 Commit 工具窗口）。
    pub(crate) fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let tab = self.state.changes_tab;

        let changes_segment = segment(
            "tab-changes",
            tr("Changes", "变更"),
            tab == ChangesTab::Changes,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.state.changes_tab = ChangesTab::Changes;
            cx.notify();
        }));
        let shelve_segment = segment(
            "tab-shelve",
            tr("Shelve", "贮藏"),
            tab == ChangesTab::Shelve,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.state.changes_tab = ChangesTab::Shelve;
            this.reload_shelves(cx);
        }));

        let header = segmented(vec![changes_segment, shelve_segment]).flex_1().when(
            tab == ChangesTab::Changes,
            |bar| {
                // 变更页签工具栏（对齐 IDEA Commit 工具窗口）：回滚 / 刷新 /
                // 无提示搁置。回滚作用于勾选的已跟踪文件（未勾选则回滚全部）。
                bar.child(
                    row_icon_button("chg-rollback", Ic::Revert, tr("Rollback", "回滚"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let selected = &this.state.selected_changes;
                            let paths: Vec<String> = this
                                .state
                                .changes
                                .iter()
                                .filter(|change| {
                                    change.status != ChangeStatus::Untracked
                                        && (selected.is_empty()
                                            || selected.contains(&change.path))
                                })
                                .map(|change| change.path.clone())
                                .collect();
                            if paths.is_empty() {
                                return;
                            }
                            this.open_prompt(
                                PromptKind::Confirm(ConfirmAction::RollbackChanges { paths }),
                                cx,
                            );
                        })),
                )
                .child(
                    row_icon_button("chg-reload", Ic::Refresh, tr("Refresh", "刷新")).on_click(
                        cx.listener(|this, _, _, cx| this.refresh(cx)),
                    ),
                )
                .child(
                    row_icon_button(
                        "chg-shelve-silent",
                        Ic::Shelve,
                        tr("Shelve Silently", "无提示搁置"),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        // 无弹窗、无信息：全部更改（含未跟踪）直接搁置。
                        this.run_op(
                            tr("Shelved silently", "已无提示搁置"),
                            |repo| repo.stash_push(None, false, true),
                            cx,
                        );
                    })),
                )
            },
        )
        .when(
            tab == ChangesTab::Shelve,
            |bar| {
                bar.child(
                    row_icon_button("shelve-unstash", Ic::Unshelve, tr("Unstash", "恢复贮藏"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run_op(tr("Unstashed", "已恢复贮藏"), |repo| repo.stash_pop(), cx)
                        })),
                )
                .child(
                    row_icon_button("shelve-reload", Ic::Refresh, tr("Reload", "刷新")).on_click(
                        cx.listener(|this, _, _, cx| this.reload_shelves(cx)),
                    ),
                )
            },
        );

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .pr(px(theme::SPACE_XS))
                    .child(header),
            )
            .when(tab == ChangesTab::Changes, |panel| {
                panel.child(self.render_changes_list(cx))
            })
            .when(tab == ChangesTab::Shelve, |panel| {
                panel.child(self.render_shelve_tab(cx))
            })
    }

    /// 「变更」页签内容：「更改」+「未进行版本管理的文件」两组（对齐 IDEA
    /// 变更列表的 Default changelist + Unversioned Files 结构）。
    ///
    /// 已暂存与未暂存的已跟踪文件同属「更改」（提交按勾选路径执行，与
    /// IntelliJ 一致，不依赖暂存区），未跟踪文件单列一组。
    fn render_changes_list(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let count = self.state.changes.len();

        let changes: Vec<&Change> = self
            .state
            .changes
            .iter()
            .filter(|change| change.status != ChangeStatus::Untracked)
            .collect();
        let untracked: Vec<&Change> = self
            .state
            .changes
            .iter()
            .filter(|change| change.status == ChangeStatus::Untracked)
            .collect();

        let mut rows = self.changes_section(
            "chg-select-all-changes",
            tr("Changes", "更改"),
            &changes,
            0,
            cx,
        );
        rows.extend(self.changes_section(
            "chg-select-all-unversioned",
            tr("Unversioned Files", "未进行版本管理的文件"),
            &untracked,
            changes.len(),
            cx,
        ));

        div()
            .id("changes-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pt(px(theme::SPACE_SM))
            .children(rows)
            .when(count == 0, |container| {
                container.child(empty_state(
                    tr("Working tree clean", "工作区干净"),
                    fg.opacity(0.4),
                ))
            })
    }

    /// 「贮藏」页签内容：贮藏条目列表 + 空态（原独立 Shelve 面板并入此页签）。
    fn render_shelve_tab(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        div()
            .id("shelve-tab-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pt(px(theme::SPACE_SM))
            .when(self.state.shelves.is_empty(), |list| {
                list.child(empty_state(
                    tr(
                        "Nothing shelved. Use Shelve… in the commit composer.",
                        "暂无贮藏。在提交区使用“搁置…”。",
                    ),
                    muted,
                ))
            })
            .children(self.state.shelves.iter().map(|entry| {
                let index = entry.index;
                list_row(format!("shelve-{index}"), fg)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(theme::font_size_body()))
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
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.unshelve_at(index, cx)
                        })),
                    )
                    .child(
                        row_icon_button(("shelve-drop", index), Ic::Delete, tr("Drop", "丢弃"))
                            .danger()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.drop_shelve_at(index, cx)
                            })),
                    )
            }))
    }

    pub(crate) fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let amend = tr("Amend", "修正提交");
        let selected_count = self.state.selected_changes.len();
        let commit_label = if selected_count > 0 {
            format!("{} ({selected_count})", tr("Commit", "提交"))
        } else {
            tr("Commit", "提交").to_string()
        };
        let push_weak: WeakEntity<Self> = cx.entity().downgrade();
        let shelve_weak = push_weak.clone();
        let settings_weak = push_weak.clone();
        div()
            .flex_none()
            .border_t_1()
            .border_color(border)
            .p(px(theme::PANEL_PADDING))
            .flex()
            .flex_col()
            .gap(px(theme::SPACE_SM))
            // 上次提交（对齐 IDEA：输入框上方显示上一条提交信息的首行）。
            .children(self.state.last_commit_message.as_ref().map(|message| {
                let first_line = message.lines().next().unwrap_or_default().to_string();
                div()
                    .flex_none()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_XS))
                    .text_size(px(theme::font_size_meta()))
                    .text_color(muted)
                    .child(Icon::new(Ic::Commit).with_size(Size::XSmall))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(first_line),
                    )
            }))
            .child(Textarea::new(&self.message_input).h(px(theme::INPUT_HEIGHT_MULTI)))
            // 选项行：修正提交为**复选框**（原位保留 IntelliJ 的选项语义），
            // IDEA 把提交选项放在按钮上方而不是塞进按钮文案里。
            .child(
                Checkbox::new("composer-amend")
                    .checked(self.state.amend)
                    .with_size(Size::XSmall)
                    .label(amend)
                    .on_click(cx.listener(|this, checked, window, cx| {
                        this.state.amend = *checked;
                        if *checked {
                            this.prefill_amend_message(window, cx);
                        }
                        cx.notify();
                    })),
            )
            // 主操作：全宽 primary 分裂按钮（对齐 IDEA 的 Commit ▾ 规范），
            // ▾ 下拉承载 提交并推送 / 贮藏 等次级动作，不再摆 ghost 按钮挤占一行。
            .child(
                DropdownButton::new("composer-commit-split")
                    .button(
                        Button::new("composer-commit")
                            .primary()
                            .label(commit_label)
                            .flex_1()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.do_commit(window, cx)
                            })),
                    )
                    .primary()
                    .flex_1()
                    .dropdown_menu(move |menu, _window, _cx| {
                        let result = menu.item(menu_item(
                            Ic::Push,
                            tr("Commit and Push…", "提交并推送…"),
                            None,
                            false,
                            false,
                            {
                                let weak = push_weak.clone();
                                move |_, window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.do_commit_and_push(window, cx)
                                    });
                                }
                            },
                        ));
                        result.item(menu_item(
                            Ic::Shelve,
                            tr("Shelve…", "贮藏…"),
                            None,
                            false,
                            false,
                            {
                                let weak = shelve_weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::Stash, cx)
                                    });
                                }
                            },
                        ))
                        .item(menu_item(
                            Ic::Settings,
                            tr("Commit Settings…", "提交设置…"),
                            None,
                            false,
                            false,
                            {
                                let weak = settings_weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::CommitSettings, cx)
                                    });
                                }
                            },
                        ))
                    }),
            )
    }

    pub(crate) fn render_statusbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let mut bar = status_bar(cx.theme().foreground).child(
            status_message().child(
                match (
                    &self.state.error,
                    self.state.busy.clone(),
                    self.state.status_message.is_empty(),
                ) {
                    (Some(error), _, _) => div()
                        .text_color(theme::error_color())
                        .child(error.clone())
                        .into_any_element(),
                    (None, Some(busy), _) => {
                        let mut busy_row = div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme::SPACE_SM))
                            .text_color(muted);
                        match &self.state.progress_text {
                            Some(text) => busy_row = busy_row.child(format!("{busy}… {text}")),
                            None => busy_row = busy_row.child(format!("{busy}…")),
                        }
                        if let Some(token) = self.state.cancel_token.clone() {
                            busy_row = busy_row.child(
                                Button::new("cancel-op")
                                    .ghost()
                                    .compact()
                                    .text_size(px(theme::font_size_meta()))
                                    .label(tr("Cancel", "取消"))
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
            ),
        );

        if !self.state.repo_root.is_empty() {
            bar = bar.child(
                status_segment(self.state.repo_root.clone())
                    .max_w(px(theme::STATUS_PATH_MAX_WIDTH))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_color(muted),
            );
        }
        let branch = self
            .state
            .current_branch
            .clone()
            .unwrap_or_else(|| tr("HEAD (detached)", "HEAD（分离状态）").to_string());
        bar = bar.child(
            status_segment(branch)
                .text_color(muted)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(theme::SPACE_XS))
                .child(Icon::new(Ic::Branch).with_size(Size::XSmall)),
        );
        if self.state.ahead > 0 {
            bar = bar.child(status_segment(format!("↑{}", self.state.ahead)).text_color(muted));
        }
        if self.state.behind > 0 {
            bar = bar.child(status_segment(format!("↓{}", self.state.behind)).text_color(muted));
        }
        bar
    }
}
