use std::sync::atomic::Ordering;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement, IntoElement, ParentElement,
    Stateful, StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::base::CheckboxState;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    button::{Button, ButtonVariants, DropdownButton},
    input::Textarea,
    menu::ContextMenuExt,
};

use rebased_rs::git::{Change, ChangeStatus};

use crate::ui::app::{AppView, ChangesTab, ConfirmAction, PromptKind};
use crate::ui::components::{
    Checkbox, TriStateCheckbox, collapsible_group_header, empty_state, list_row, menu_item,
    menu_width, row_icon_button, selected, status_bar, status_message, status_segment,
};
use crate::ui::components::tabs::{segment, segmented};
use crate::ui::theme::status_color;
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
        let checkbox_path = change.path.clone();
        let checked = self.state.selected_changes.contains(&change.path);
        let weak = cx.entity().downgrade();
        let checkbox_weak = weak.clone();

        let menu_change = change.clone();
        let menu_path = change.path.clone();
        let row = list_row(format!("change-{index}"), fg)
            // 子行相对组头缩进一级：明确「更改」组与文件行的树形层级
            //（IDEA 高密度风格，左缩进 TREE_INDENT，行高/悬停保持不变）。
            .pl(px(theme::ROW_PADDING_X + theme::TREE_INDENT))
            .gap(px(theme::SPACE_SM))
            .when(checked, |row| selected(row, true, fg))
            // IDEA Commit 语义：单击 = 切换暂存，双击 = 右侧打开「提交:文件」diff Tab。
            // 双击的第一次 mouseup 会先以 count==1 派发，故单击走 240ms 延迟定时器；
            // 双击分支递增 pending_stage_gen，使未触发的单击定时任务作废（避免误暂存）。
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() >= 2 {
                    this.open_commit_diff_tab(change_for_click.path.clone(), staged, cx);
                    return;
                }
                let stage_gen = this.state.pending_stage_gen.wrapping_add(1);
                this.state.pending_stage_gen = stage_gen;
                let click_change = change_for_click.clone();
                cx.spawn(async move |this, cx| {
                    let executor = cx.background_executor().clone();
                    executor.timer(Duration::from_millis(240)).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.state.pending_stage_gen == stage_gen {
                            this.toggle_stage(&click_change, cx);
                        }
                    });
                })
                .detach();
            }))
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
            // 类型图标槽：JetBrains 官方彩色文件类型图标（IDEA Changes 同款）。
            .child(crate::ui::icons::file_type_icon(&change.path))
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
        // 行尾状态列：状态字母 + 统一状态色（与文件名着色 / 文件树状态色同源），
        // 不依赖纯颜色即可分辨状态。
        let row = row.child(
            div()
                .flex_none()
                .w(px(theme::ICON_SIZE_SM))
                .text_size(px(theme::font_size_meta()))
                .font_weight(theme::WEIGHT_MEDIUM)
                .text_color(color)
                .child(change.status.short_label()),
        );

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
    /// 标题行的前导复选框是 IDEA 式三态：组内全勾 → 勾、部分勾选 → 半选
    /// （蓝底短横）、全不选 → 空框；点击半选/空框全选、点击勾全不选。
    /// 尾部显示条目数——与 IntelliJ 变更列表的分组行一致（组名左侧可勾选、
    /// 右侧计数）。组头带 chevron，整行可点击折叠/展开；折叠时不渲染子行。
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
        let expanded = !self.state.changes_collapsed.contains(id);
        let selected_count = group
            .iter()
            .filter(|change| self.state.selected_changes.contains(&change.path))
            .count();
        let group_state = if selected_count == 0 {
            CheckboxState::Unchecked
        } else if selected_count == group.len() {
            CheckboxState::Checked
        } else {
            CheckboxState::Indeterminate
        };
        let paths: Vec<String> = group.iter().map(|change| change.path.clone()).collect();
        let weak = cx.entity().downgrade();

        let header = collapsible_group_header(
            format!("{id}-header"),
            label,
            muted,
            expanded,
            Some(
                TriStateCheckbox::new(id)
                    .state(group_state)
                    .with_size(Size::XSmall)
                    .on_click(move |next, _, _, app| {
                        app.stop_propagation();
                        let select_all = next == CheckboxState::Checked;
                        let _ = weak.update(app, |this, cx| {
                            this.set_changes_selection(&paths, select_all, cx)
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
            cx.listener(move |this, _, _, cx| {
                // 整行点击切换折叠；前导复选框已 stop_propagation，互不干扰。
                if !this.state.changes_collapsed.remove(id) {
                    this.state.changes_collapsed.insert(id.to_string());
                }
                cx.notify();
            }),
        )
        // 组间留出小间隔（首组之上还有列表自身的 pt），保持高密度。
        .mt(px(theme::SPACE_SM));

        let mut out: Vec<AnyElement> =
            Vec::with_capacity(if expanded { group.len() + 1 } else { 1 });
        out.push(header.into_any_element());
        if expanded {
            for (offset, change) in group.iter().enumerate() {
                out.push(self.render_change_row(start_index + offset, change, cx));
            }
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
                    cx.theme().muted_foreground,
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
            // 主操作区（IDEA Commit 工具窗）：「提交」primary split 按钮
            //（主体=提交，caret 下拉=提交并推送…），最右为提交设置齿轮。
            // 贮藏入口收敛在 Alt+` VCS 快切与 Shelve 页签，不再挤占提交操作区。
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_SM))
                    .child(
                        div().flex_1().min_w_0().child(
                            DropdownButton::new("composer-commit")
                                .button(
                                    Button::new("composer-commit-btn")
                                        .primary()
                                        .label(commit_label)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.do_commit(window, cx)
                                        })),
                                )
                                .dropdown_menu({
                                    let weak = cx.entity().downgrade();
                                    move |menu, _, _| {
                                        let weak = weak.clone();
                                        menu.item(menu_item(
                                            Ic::Push,
                                            tr("Commit and Push…", "提交并推送…"),
                                            None,
                                            false,
                                            false,
                                            move |_, window, app| {
                                                let _ = weak.update(app, |this, cx| {
                                                    this.do_commit_and_push(window, cx)
                                                });
                                            },
                                        ))
                                    }
                                }),
                        ),
                    )
                    .child(
                        row_icon_button(
                            "composer-commit-settings",
                            Ic::Settings,
                            tr("Commit Settings…", "提交设置…"),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_prompt(PromptKind::CommitSettings, cx)
                        })),
                    ),
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
