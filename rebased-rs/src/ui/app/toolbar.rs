use std::sync::atomic::Ordering;

use gpui::prelude::FluentBuilder;
use gpui::{
    Anchor, AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WeakEntity, div, px,
};
use gpui_kit::component::{
    ActiveTheme, Sizable, Size,
    button::{Button, ButtonVariants, DropdownButton},
    input::Textarea,
    list::List,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    theme::{Theme, ThemeMode},
};

use rebased_rs::git::{Branch, Change, ChangeStatus, MergeMode};

use crate::ui::components::{Checkbox, empty_state, group_header, v_separator};
use crate::ui::graph_view::status_color;
use crate::ui::i18n::{self, Language, tr};
use crate::ui::icons::Ic;
use crate::ui::settings;
use crate::ui::theme;

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
        let fg = cx.theme().foreground;
        let branch_entries = self.state.branch_entries.clone();
        let tags = self.state.tags.clone();
        let current = self.state.current_branch.clone();
        let current_upstream = self.state.current_upstream.clone();
        let remote_list = self.state.remotes.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let tag_weak = weak.clone();
        let filter_weak = weak.clone();
        let branch_names: Vec<String> = branch_entries.iter().map(|b| b.name.clone()).collect();
        let filter_branch = self.state.filter_branch.clone();
        let date_weak = weak.clone();
        let filter_since = self.state.filter_since.clone();
        let mut branch_label = current.clone().unwrap_or_else(|| "main".to_string());
        if self.state.ahead > 0 {
            branch_label.push_str(&format!(" ↑{}", self.state.ahead));
        }
        if self.state.behind > 0 {
            branch_label.push_str(&format!(" ↓{}", self.state.behind));
        }

        div()
            .h(px(theme::TOOLBAR_HEIGHT))
            .flex_none()
            .border_b_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .child(
                DropdownButton::new("branch-menu")
                    .button(
                        Button::new("branch-button")
                            .secondary()
                            .outline()
                            .compact()
                            .icon(Ic::Branch)
                            .label(branch_label.clone())
                            .tooltip(format!("{}: {}", tr("Branches", "分支"), branch_label)),
                    )
                    .dropdown_menu(move |menu, window, cx| {
                        let mut result = menu;
                        // 本地分支：点击切换，当前分支打勾并展示 tracking 状态。
                        for branch in branch_entries.iter().filter(|b| !b.is_remote) {
                            let label = Self::tracking_label(branch);
                            let is_current = branch.is_current();
                            let weak = weak.clone();
                            let name = branch.name.clone();
                            result = result.item(
                                PopupMenuItem::new(label).checked(is_current).on_click(
                                    move |_, _, cx| {
                                        let _ = weak
                                            .update(cx, |this, cx| this.checkout_branch(&name, cx));
                                    },
                                ),
                            );
                        }
                        result = result.separator();
                        result = result.item(
                            PopupMenuItem::new(tr("New Branch…", "新建分支…")).on_click({
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(
                                            PromptKind::NewBranch { start_point: None },
                                            cx,
                                        )
                                    });
                                }
                            }),
                        );
                        result = result.item(
                            PopupMenuItem::new(tr("Rename Current Branch…", "重命名当前分支…"))
                                .on_click({
                                    let weak = weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(PromptKind::RenameBranch, cx)
                                        });
                                    }
                                }),
                        );
                        result = result.item(
                            PopupMenuItem::new(tr(
                                "Force Push (with lease)",
                                "强制推送（含保护检查）",
                            ))
                            .on_click({
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
                        // 当前分支的 upstream 管理：设置 / 取消。
                        if let Some(name) = current.clone() {
                            result = result.item(
                                PopupMenuItem::new(format!(
                                    "{} {name}…",
                                    tr("Set upstream of", "设置上游分支：")
                                ))
                                .on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::SetUpstream {
                                                    branch: name.clone(),
                                                },
                                                cx,
                                            )
                                        });
                                    }
                                }),
                            );
                            if let Some(upstream) = current_upstream.clone() {
                                result = result.item(
                                    PopupMenuItem::new(format!(
                                        "{} {name} ({upstream})",
                                        tr("Unset upstream of", "取消上游分支：")
                                    ))
                                    .on_click({
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.unset_branch_upstream(name.clone(), cx)
                                            });
                                        }
                                    }),
                                );
                            }
                        }
                        // 本地分支操作：Merge into / Rebase onto / Delete。
                        for branch in branch_entries.iter().filter(|b| !b.is_remote) {
                            if branch.is_current() {
                                continue;
                            }
                            let name = branch.name.clone();
                            let target = current.clone().unwrap_or_else(|| "HEAD".to_string());
                            let merge_specs = [
                                (
                                    format!("{} {name} → {target}", tr("Merge", "合并")),
                                    MergeMode::Default,
                                ),
                                (
                                    format!(
                                        "{} {name} → {target} {}",
                                        tr("Merge", "合并"),
                                        tr("(no ff)…", "（非快进）…")
                                    ),
                                    MergeMode::NoFastForward,
                                ),
                                (
                                    format!(
                                        "{} {name} → {target} {}",
                                        tr("Merge", "合并"),
                                        tr("(ff only)", "（仅快进）")
                                    ),
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
                                    "{} {name}…",
                                    tr("Rebase onto", "变基到")
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
                                "{} {name} ↔ {}",
                                tr("Compare", "比较"),
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
                            result = result.item(
                                PopupMenuItem::new(format!("{} {name}…", tr("Rename", "重命名")))
                                    .on_click({
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update_in(cx, |this, window, cx| {
                                                this.open_rename_branch_by_name(
                                                    name.clone(),
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }
                                    }),
                            );
                            result = result.item(
                                PopupMenuItem::new(format!("{} {name}", tr("Delete", "删除")))
                                    .on_click({
                                        let weak = weak.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.open_prompt(
                                                    PromptKind::Confirm(
                                                        ConfirmAction::DeleteBranch {
                                                            name: name.clone(),
                                                        },
                                                    ),
                                                    cx,
                                                )
                                            });
                                        }
                                    }),
                            );
                            // 每个本地分支的 upstream 管理。
                            let name = branch.name.clone();
                            result = result.item(
                                PopupMenuItem::new(format!(
                                    "{} {name}…",
                                    tr("Set upstream of", "设置上游分支：")
                                ))
                                .on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::SetUpstream {
                                                    branch: name.clone(),
                                                },
                                                cx,
                                            )
                                        });
                                    }
                                }),
                            );
                            if let Some(upstream) = branch.upstream.clone() {
                                result = result.item(
                                    PopupMenuItem::new(format!(
                                        "{} {name} ({upstream})",
                                        tr("Unset upstream of", "取消上游分支：")
                                    ))
                                    .on_click({
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.unset_branch_upstream(name.clone(), cx)
                                            });
                                        }
                                    }),
                                );
                            }
                        }
                        // 远程分支：按 remote 分组为树形子菜单
                        // （组 = remote 名，组内 = 分支 → 操作子菜单）。
                        let remote_branches: Vec<&Branch> =
                            branch_entries.iter().filter(|b| b.is_remote).collect();
                        if !remote_branches.is_empty() {
                            // 组闭包是 move，需绑定到本函数体的局部值，
                            // 避免捕获外层 Fn 闭包环境里的 weak/current。
                            let weak = weak.clone();
                            let current = current.clone();
                            result = result.separator();
                            result = result
                                .item(PopupMenuItem::label(tr("Remote Branches", "远程分支")));
                            // 分组顺序：先 remote_list（git remote -v 输出序），再补缺失组。
                            let mut groups: Vec<String> = remote_list
                                .iter()
                                .map(|remote| remote.name.clone())
                                .collect();
                            for branch in &remote_branches {
                                if let Some((group, _)) = branch.name.split_once('/') {
                                    let group = group.to_string();
                                    if !groups.contains(&group) {
                                        groups.push(group);
                                    }
                                }
                            }
                            for group in groups {
                                let prefix = format!("{group}/");
                                let names: Vec<String> = remote_branches
                                    .iter()
                                    .filter(|b| b.name.starts_with(&prefix))
                                    .map(|b| b.name.clone())
                                    .collect();
                                if names.is_empty() {
                                    continue;
                                }
                                // 每轮迭代 clone 独立副本供组闭包 move 捕获。
                                let group_weak = weak.clone();
                                let group_current = current.clone();
                                result =
                                    result.submenu(group, window, cx, move |menu, window, cx| {
                                        let mut menu = menu;
                                        for name in &names {
                                            let name = name.clone();
                                            // 组内条目显示去掉 remote 前缀的短名，操作用全名。
                                            let short = name
                                                .strip_prefix(&prefix)
                                                .unwrap_or(&name)
                                                .to_string();
                                            menu = menu.submenu(short, window, cx, {
                                                let current = group_current.clone();
                                                let weak = group_weak.clone();
                                                move |menu, _window, _cx| {
                                                    remote_branch_actions(
                                                        menu,
                                                        &name,
                                                        current.clone(),
                                                        weak.clone(),
                                                    )
                                                }
                                            });
                                        }
                                        menu
                                    });
                            }
                        }
                        // 远程仓库管理：Add / Prune / Remove。
                        result = result.separator();
                        result = result.item(PopupMenuItem::label(tr("Remotes", "远程仓库")));
                        result = result.item(
                            PopupMenuItem::new(tr("Add Remote…", "添加远程仓库…")).on_click({
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::AddRemote, cx)
                                    });
                                }
                            }),
                        );
                        for remote in remote_list.iter() {
                            let label = format!("{} → {}", remote.name, remote.url);
                            result = result.item(PopupMenuItem::label(&label));
                            let prune_label = format!(
                                "{} {}",
                                tr("Prune remote branches of", "清理远程分支："),
                                remote.name
                            );
                            result = result.item(PopupMenuItem::new(prune_label).on_click({
                                let weak = weak.clone();
                                let name = remote.name.clone();
                                move |_, _, cx| {
                                    let _ =
                                        weak.update(cx, |this, cx| this.prune_remote(&name, cx));
                                }
                            }));
                            let remove_label = format!(
                                "{} {}",
                                tr("Remove remote", "移除远程仓库："),
                                remote.name
                            );
                            result = result.item(PopupMenuItem::new(remove_label).on_click({
                                let weak = weak.clone();
                                let name = remote.name.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(
                                            PromptKind::Confirm(ConfirmAction::RemoveRemote {
                                                name: name.clone(),
                                            }),
                                            cx,
                                        )
                                    });
                                }
                            }));
                        }
                        result
                    }),
            )
            .child(v_separator(fg))
            .child(
                DropdownButton::new("tag-menu")
                    .button(
                        Button::new("tag-button")
                            .ghost()
                            .compact()
                            .icon(Ic::Tag)
                            .tooltip(tr("Tags", "标签")),
                    )
                    .dropdown_menu(move |menu, window, _cx| {
                        let mut result = menu.item(
                            PopupMenuItem::new(tr("New Tag on HEAD…", "在 HEAD 上新建标签…"))
                                .on_click({
                                    let weak = tag_weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::NewTag {
                                                    commit_id: "HEAD".to_string(),
                                                },
                                                cx,
                                            )
                                        });
                                    }
                                }),
                        );
                        result = result.item(
                            PopupMenuItem::new(tr(
                                "Push All Tags to origin",
                                "推送所有标签到 origin",
                            ))
                            .on_click({
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
                                result = result.submenu(
                                    label.clone(),
                                    window,
                                    _cx,
                                    move |menu, _window, _cx| {
                                        menu.item(
                                            PopupMenuItem::new(tr("Select Commit", "定位提交"))
                                                .on_click({
                                                    let weak = weak.clone();
                                                    let commit_id = commit_id.clone();
                                                    move |_, _, cx| {
                                                        let _ = weak.update(cx, |this, cx| {
                                                            this.select_commit_by_id(&commit_id, cx)
                                                        });
                                                    }
                                                }),
                                        )
                                        .item(
                                            PopupMenuItem::new(tr(
                                                "Push Tag to origin",
                                                "推送标签到 origin",
                                            ))
                                            .on_click({
                                                let weak = weak.clone();
                                                let label = label.clone();
                                                move |_, _, cx| {
                                                    let _ = weak.update(cx, |this, cx| {
                                                        this.push_tag(label.clone(), cx)
                                                    });
                                                }
                                            }),
                                        )
                                        .item(
                                            PopupMenuItem::new(tr(
                                                "Edit Message…",
                                                "编辑标签信息…",
                                            ))
                                            .on_click({
                                                let weak = weak.clone();
                                                let label = label.clone();
                                                let commit_id = commit_id.clone();
                                                move |_, _, cx| {
                                                    let _ = weak.update(cx, |this, cx| {
                                                        this.open_prompt(
                                                            PromptKind::EditTag {
                                                                name: label.clone(),
                                                                commit_id: commit_id.clone(),
                                                            },
                                                            cx,
                                                        )
                                                    });
                                                }
                                            }),
                                        )
                                        .item(
                                            PopupMenuItem::new(tr("Delete Tag…", "删除标签…"))
                                                .on_click({
                                                    let weak = weak.clone();
                                                    let label = label.clone();
                                                    move |_, _, cx| {
                                                        let _ = weak.update(cx, |this, cx| {
                                                            this.open_prompt(
                                                                PromptKind::Confirm(
                                                                    ConfirmAction::DeleteTag {
                                                                        name: label.clone(),
                                                                    },
                                                                ),
                                                                cx,
                                                            )
                                                        });
                                                    }
                                                }),
                                        )
                                    },
                                );
                            }
                        }
                        result
                    }),
            )
            .child(v_separator(fg))
            .child(
                DropdownButton::new("branch-filter-menu")
                    .button(
                        Button::new("branch-filter-button")
                            .ghost()
                            .compact()
                            .icon(Ic::Filter)
                            .tooltip(format!(
                                "{}",
                                filter_branch.clone().unwrap_or_else(|| tr(
                                    "All Branches",
                                    "所有分支"
                                )
                                .to_string())
                            )),
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(
                            PopupMenuItem::new(tr("All Branches", "所有分支"))
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
                                PopupMenuItem::new(name.clone()).checked(checked).on_click(
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_branch_filter(Some(name.clone()), cx)
                                        });
                                    },
                                ),
                            );
                        }
                        result
                    }),
            )
            .child(
                Button::new("author-filter")
                    .ghost()
                    .compact()
                    .icon(Ic::User)
                    .tooltip(format!(
                        "{}",
                        if self.state.filter_author.is_empty() {
                            tr("All Authors", "所有作者").to_string()
                        } else {
                            self.state.filter_author.clone()
                        }
                    ))
                    .on_click(
                        cx.listener(|this, _, _, cx| {
                            this.open_prompt(PromptKind::FilterAuthor, cx)
                        }),
                    ),
            )
            .child(
                DropdownButton::new("date-filter-menu")
                    .button(
                        Button::new("date-filter-button")
                            .ghost()
                            .compact()
                            .icon(Ic::History)
                            .tooltip(format!(
                                "{}",
                                filter_since
                                    .as_ref()
                                    .map(|(label, _)| label.clone())
                                    .unwrap_or_else(|| tr("All Time", "全部时间").to_string())
                            )),
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(
                            PopupMenuItem::new(tr("All Time", "全部时间"))
                                .checked(filter_since.is_none())
                                .on_click({
                                    let weak = date_weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak
                                            .update(cx, |this, cx| this.set_date_filter(None, cx));
                                    }
                                }),
                        );
                        let date_specs = [
                            (tr("Today", "今天"), "midnight"),
                            (tr("This week", "本周"), "1 week ago"),
                            (tr("This month", "本月"), "1 month ago"),
                            (tr("This year", "今年"), "1 year ago"),
                        ];
                        for (label, expr) in date_specs {
                            let checked =
                                filter_since.as_ref().map(|(_, e)| e.as_str()) == Some(expr);
                            let weak = date_weak.clone();
                            result =
                                result.item(PopupMenuItem::new(label).checked(checked).on_click(
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_date_filter(
                                                Some((label.to_string(), expr.to_string())),
                                                cx,
                                            )
                                        });
                                    },
                                ));
                        }
                        result
                    }),
            )
            .child(
                Button::new("goto")
                    .ghost()
                    .compact()
                    .icon(Ic::Search)
                    .tooltip(tr("Go to…", "跳转到…"))
                    .on_click(cx.listener(|this, _, _, cx| this.open_prompt(PromptKind::GoTo, cx))),
            )
            .child(
                Button::new("refresh")
                    .ghost()
                    .compact()
                    .icon(Ic::Refresh)
                    .tooltip(tr("Refresh", "刷新"))
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(v_separator(fg))
            .child(
                Button::new("fetch")
                    .ghost()
                    .compact()
                    .icon(Ic::Fetch)
                    .tooltip(tr("Fetch", "抓取"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_op_progress(
                            tr("Fetch", "抓取"),
                            tr("Fetched", "已抓取"),
                            |repo, progress, cancel| repo.fetch_with_control(progress, cancel),
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("pull")
                    .ghost()
                    .compact()
                    .icon(Ic::Pull)
                    .tooltip(tr("Pull", "拉取"))
                    .on_click(cx.listener(|this, _, _, cx| this.do_pull(cx))),
            )
            .child(
                Button::new("push")
                    .ghost()
                    .compact()
                    .icon(Ic::Push)
                    .tooltip(tr("Push", "推送"))
                    .on_click(cx.listener(|this, _, _, cx| this.do_push(cx))),
            )
            .child(v_separator(fg))
            .child(
                Button::new("stash")
                    .ghost()
                    .compact()
                    .icon(Ic::Shelve)
                    .tooltip(tr("Stash", "贮藏"))
                    .on_click(
                        cx.listener(|this, _, _, cx| this.open_prompt(PromptKind::Stash, cx)),
                    ),
            )
            .child(
                Button::new("unstash")
                    .ghost()
                    .compact()
                    .icon(Ic::Unshelve)
                    .tooltip(tr("Unstash", "恢复贮藏"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_op(tr("Unstashed", "已恢复贮藏"), |repo| repo.stash_pop(), cx)
                    })),
            )
            .child(v_separator(fg))
            .child(
                Button::new("conflicts")
                    .ghost()
                    .compact()
                    .icon(Ic::Conflict)
                    .tooltip(tr("Conflicts", "冲突"))
                    .on_click(cx.listener(|this, _, _, cx| this.open_conflicts(cx))),
            )
            .child(
                Button::new("shelves")
                    .ghost()
                    .compact()
                    .icon(Ic::Changes)
                    .tooltip(tr("Shelves", "搁置"))
                    .on_click(cx.listener(|this, _, _, cx| this.open_shelves(cx))),
            )
            .child(v_separator(fg))
            .child(
                Button::new("settings")
                    .ghost()
                    .compact()
                    .icon(Ic::Settings)
                    .tooltip(tr("Settings", "设置"))
                    .dropdown_menu_with_anchor(Anchor::TopRight, |menu, _window, cx| {
                        let is_zh = i18n::current() == Language::Zh;
                        let is_dark = cx.theme().is_dark();
                        menu.item(PopupMenuItem::new("English").checked(!is_zh).on_click(
                            |_, _, cx| {
                                if i18n::current() != Language::En {
                                    i18n::set_current(Language::En);
                                    cx.refresh_windows();
                                }
                            },
                        ))
                        .item(
                            PopupMenuItem::new("中文")
                                .checked(is_zh)
                                .on_click(|_, _, cx| {
                                    if i18n::current() != Language::Zh {
                                        i18n::set_current(Language::Zh);
                                        cx.refresh_windows();
                                    }
                                }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new(tr("Light Theme", "浅色主题"))
                                .checked(!is_dark)
                                .on_click(|_, window, cx| {
                                    Theme::change(ThemeMode::Light, Some(window), cx);
                                    settings::persist_theme_mode(ThemeMode::Light);
                                    cx.refresh_windows();
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr("Dark Theme", "深色主题"))
                                .checked(is_dark)
                                .on_click(|_, window, cx| {
                                    Theme::change(ThemeMode::Dark, Some(window), cx);
                                    settings::persist_theme_mode(ThemeMode::Dark);
                                    cx.refresh_windows();
                                }),
                        )
                    }),
            )
            .when(self.state.rebase.in_progress(), |bar| {
                bar.child(
                    Button::new("rebase-abort")
                        .danger()
                        .compact()
                        .label(tr("Abort Rebase", "中止变基"))
                        .on_click(cx.listener(|this, _, _, cx| this.abort_rebase(cx))),
                )
                .child(
                    Button::new("rebase-continue")
                        .danger()
                        .compact()
                        .label(tr("Continue Rebase", "继续变基"))
                        .on_click(cx.listener(|this, _, _, cx| this.continue_rebase(cx))),
                )
            })
            .when(self.state.merge_in_progress, |bar| {
                bar.child(
                    Button::new("merge-abort")
                        .danger()
                        .compact()
                        .label(tr("Abort Merge", "中止合并"))
                        .on_click(cx.listener(|this, _, _, cx| this.abort_merge(cx))),
                )
                .child(
                    Button::new("merge-continue")
                        .danger()
                        .compact()
                        .label(tr("Continue Merge", "继续合并"))
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
                .child(tr("Loading repository...", "正在加载仓库..."))
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
                    PopupMenuItem::new(format!("{} {short}", tr("Checkout", "检出"))).on_click({
                        let weak = weak.clone();
                        let id = id.clone();
                        move |_, _, cx| {
                            let _ =
                                weak.update(cx, |this, cx| this.checkout_commit(id.clone(), cx));
                        }
                    }),
                );
                result = result.item(PopupMenuItem::new(tr("New Branch…", "新建分支…")).on_click(
                    {
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
                    },
                ));
                result = result.item(PopupMenuItem::new(tr("New Tag…", "新建标签…")).on_click({
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
                result = result.item(PopupMenuItem::new(tr("Cherry-pick", "摘取提交")).on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.cherry_pick_commit(id.clone(), cx));
                    }
                }));
                result = result.item(
                    PopupMenuItem::new(tr("Revert Commit", "回滚提交")).on_click({
                        let weak = weak.clone();
                        let id = id.clone();
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.revert_commit(id.clone(), cx));
                        }
                    }),
                );
                result = result.item(
                    PopupMenuItem::new(tr("Reset Current Branch to Here…", "重置当前分支到此处…"))
                        .on_click({
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
                result = result.item(
                    PopupMenuItem::new(tr("Rebase from Here", "从这里变基")).on_click({
                        let weak = weak.clone();
                        let id = id.clone();
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.start_rebase(id.clone(), cx));
                        }
                    }),
                );
                result = result.item(
                    PopupMenuItem::new(tr("Reword Message…", "修改提交信息…")).on_click({
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
                    }),
                );
                result = result.separator();
                result = result.item(PopupMenuItem::new(tr("Diff", "查看差异")).on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ =
                            weak.update(cx, |this, cx| this.open_commit_diff(id.clone(), None, cx));
                    }
                }));
                result = result.item(
                    PopupMenuItem::new(tr("Compare with Current Branch", "与当前分支比较"))
                        .on_click({
                            let weak = weak.clone();
                            let id = id.clone();
                            move |_, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.open_branch_compare(id.clone(), cx)
                                });
                            }
                        }),
                );
                result = result.item(PopupMenuItem::new(tr("Copy SHA", "复制 SHA")).on_click({
                    let weak = weak.clone();
                    let id = id.clone();
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.copy_commit_id(id.clone(), cx));
                    }
                }));
                if is_head {
                    result =
                        result.item(PopupMenuItem::new(tr("Undo Commit", "撤销提交")).on_click({
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
                    result =
                        result.item(PopupMenuItem::new(tr("Drop Commit", "丢弃提交")).on_click({
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

    pub(crate) fn render_change_row(
        &self,
        index: usize,
        change: &Change,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fg = cx.theme().foreground;
        let color = status_color(&change.status);
        let path = change.display_path();
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
        let row = div()
            .id(format!("change-{index}"))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_0p5()
            .rounded(px(theme::RADIUS))
            .cursor_pointer()
            .hover(move |style| style.bg(theme::hover_bg(fg)))
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

        let row = row.child(
            Button::new(format!("chg-diff-{index}"))
                .ghost()
                .compact()
                .icon(Ic::Diff)
                .tooltip(tr("Show Diff", "查看差异"))
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
        let row = if !staged && !is_untracked {
            let discard_path = change.path.clone();
            row.child(
                Button::new(format!("chg-discard-{index}"))
                    .ghost()
                    .compact()
                    .icon(Ic::Revert)
                    .tooltip(tr("Rollback…", "回滚…"))
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
                Button::new(format!("chg-remove-{index}"))
                    .ghost()
                    .compact()
                    .icon(Ic::Delete)
                    .tooltip(tr("Delete", "删除"))
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
            let weak_for_stage = weak.clone();
            let change_for_stage = menu_change.clone();
            let weak_for_diff = weak.clone();
            let path_for_diff = path.clone();
            let mut m = menu
                .item(PopupMenuItem::new(stage_label).on_click(move |_, _, app| {
                    let _ = weak_for_stage.update(app, |this, cx| {
                        this.toggle_stage(&change_for_stage, cx);
                    });
                }))
                .item(PopupMenuItem::new(tr("Show Diff", "查看差异")).on_click(
                    move |_, _, app| {
                        let _ = weak_for_diff.update(app, |this, cx| {
                            if menu_change.staged {
                                this.open_staged_diff(Some(path_for_diff.clone()), cx);
                            } else {
                                this.open_unstaged_diff(Some(path_for_diff.clone()), cx);
                            }
                        });
                    },
                ));
            if !menu_change.staged && !is_untracked {
                let weak_for_discard = weak.clone();
                let path_for_discard = path.clone();
                m = m.item(PopupMenuItem::new(tr("Rollback…", "回滚…")).on_click(
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
                m = m.item(
                    PopupMenuItem::new(tr("Delete", "删除")).on_click(move |_, _, app| {
                        let _ = weak_for_remove.update(app, |this, cx| {
                            let path = path_for_remove.clone();
                            let message = format!("{} {path}", tr("Removed", "已移除"));
                            this.run_op(&message, move |repo| repo.remove_untracked(&path), cx);
                        });
                    }),
                );
            }
            m
        });

        menu.child(
            div()
                .flex_none()
                .w(px(12.))
                .text_sm()
                .text_color(if staged {
                    cx.theme().muted_foreground
                } else {
                    theme::added_color()
                })
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

        let mut section = |label: &str, group: &[&Change], accumulator: &mut Vec<AnyElement>| {
            if group.is_empty() {
                return;
            }
            accumulator.push(
                group_header(format!("{label} ({})", group.len()), muted)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .into_any_element(),
            );
            for change in group {
                accumulator.push(self.render_change_row(next_index, change, cx));
                next_index += 1;
            }
        };

        let staged: Vec<&Change> = self.state.changes.iter().filter(|c| c.staged).collect();
        let unstaged: Vec<&Change> = self.state.changes.iter().filter(|c| !c.staged).collect();
        section(tr("Unstaged", "未暂存"), &unstaged, &mut rows);
        section(tr("Staged", "已暂存"), &staged, &mut rows);

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                group_header(
                    format!("{} ({count})", tr("Changes", "变更")),
                    cx.theme().muted_foreground,
                )
                .font_weight(gpui::FontWeight::MEDIUM),
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
                        container.child(empty_state(
                            tr("Working tree clean", "工作区干净"),
                            fg.opacity(0.4),
                        ))
                    }),
            )
    }

    pub(crate) fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let amend = tr("Amend", "修正提交");
        let amend_label = if self.state.amend {
            format!("✓ {amend}")
        } else {
            amend.to_string()
        };
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
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(Textarea::new(&self.message_input).h(px(64.)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("shelve")
                            .ghost()
                            .label(tr("Shelve…", "搁置…"))
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
                            .on_click(
                                cx.listener(|this, _, window, cx| this.do_commit(window, cx)),
                            ),
                    ),
            )
    }

    pub(crate) fn render_statusbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        div()
            .h(px(theme::STATUSBAR_HEIGHT))
            .flex_none()
            .border_t_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .text_xs()
            .child(
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
                                    .compact()
                                    .text_xs()
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
            .child(div().text_color(muted).child(format!(
                    "⎇ {}",
                    self.state
                        .current_branch
                        .clone()
                        .unwrap_or_else(|| tr("HEAD (detached)", "HEAD（分离状态）").to_string())
                )))
            .when(self.state.ahead > 0, |bar| {
                bar.child(
                    div()
                        .text_color(muted)
                        .child(format!("↑{}", self.state.ahead)),
                )
            })
            .when(self.state.behind > 0, |bar| {
                bar.child(
                    div()
                        .text_color(muted)
                        .child(format!("↓{}", self.state.behind)),
                )
            })
    }
}

/// 远程分支的操作子菜单：Checkout / Pull into 当前分支 / Rebase onto / Compare。
fn remote_branch_actions(
    menu: PopupMenu,
    name: &str,
    current: Option<String>,
    weak: WeakEntity<AppView>,
) -> PopupMenu {
    let menu = menu.item(
        PopupMenuItem::new(format!("{} {name}", tr("Checkout", "检出"))).on_click({
            let weak = weak.clone();
            let name = name.to_string();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.checkout_branch(&name, cx));
            }
        }),
    );
    let pull_label = format!(
        "{} {name} → {}",
        tr("Pull into", "拉取到"),
        current.clone().unwrap_or_else(|| "HEAD".to_string())
    );
    let menu = menu.item(PopupMenuItem::new(pull_label).on_click({
        let weak = weak.clone();
        let name = name.to_string();
        move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                this.pull_branch_into_current(name.clone(), cx)
            });
        }
    }));
    let menu = menu.item(
        PopupMenuItem::new(format!("{} {name}…", tr("Rebase onto", "变基到"))).on_click({
            let weak = weak.clone();
            let name = name.to_string();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.rebase_current_onto(name.clone(), cx));
            }
        }),
    );
    let compare_label = format!(
        "{} {name} ↔ {}",
        tr("Compare", "比较"),
        current.unwrap_or_else(|| "HEAD".to_string())
    );
    menu.item(PopupMenuItem::new(compare_label).on_click({
        let weak = weak.clone();
        let name = name.to_string();
        move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| this.open_branch_compare(name.clone(), cx));
        }
    }))
}
