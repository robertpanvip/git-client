mod changes;
mod log;
mod menus;

use gpui::prelude::FluentBuilder;
use gpui::{
    Anchor, Context, IntoElement, ParentElement, Styled, WeakEntity, WindowAppearance, div, px,
};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants, DropdownButton},
    menu::{DropdownMenu, PopupMenuItem},
    theme::{Theme, ThemeMode},
};

use rebased_rs::git::{Branch, MergeMode};

use crate::ui::components::{menu_item, menu_width, v_separator};
use crate::ui::i18n::{self, Language, tr};
use crate::ui::icons::Ic;
use crate::ui::settings;
use crate::ui::theme;

use super::{AppView, ConfirmAction, PromptKind};

pub(super) use menus::remote_branch_actions;

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
        let fg = cx.theme().foreground;
        let branch_entries = self.state.branch_entries.clone();
        let tags = self.state.tags.clone();
        let current = self.state.current_branch.clone();
        let current_upstream = self.state.current_upstream.clone();
        let remote_list = self.state.remotes.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let tag_weak = weak.clone();
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
            .bg(theme::bg_chrome())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_XS))
            .px(px(theme::SPACE_SM))
            .child(
                DropdownButton::new("branch-menu")
                    .button(
                        Button::new("branch-button")
                            .ghost()
                            .compact()
                            .icon(Ic::Branch)
                            .label(branch_label.clone())
                            .tooltip(format!(
                                "{}: {branch_label}",
                                tr("Search branches and actions", "搜索分支和操作")
                            ))
                            // 主按钮 = 可搜索分支弹窗；右侧 ▾ 仍打开完整分支管理菜单。
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_branch_popup(window, cx)
                            })),
                    )
                    .dropdown_menu(move |menu, window, cx| {
                        let mut result = menu;
                        // 本地分支：点击切换，当前分支打勾并展示 tracking 状态。
                        for branch in branch_entries.iter().filter(|b| !b.is_remote) {
                            let label = Self::tracking_label(branch);
                            let is_current = branch.is_current();
                            let weak = weak.clone();
                            let name = branch.name.clone();
                            result = result.item(menu_item(
                                Ic::Checkout,
                                label,
                                None,
                                false,
                                is_current,
                                move |_, _, cx| {
                                    let _ =
                                        weak.update(cx, |this, cx| this.checkout_branch(&name, cx));
                                },
                            ));
                        }
                        result = result.separator();
                        result = result.item(menu_item(
                            Ic::Add,
                            tr("New Branch…", "新建分支…"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(
                                            PromptKind::NewBranch { start_point: None },
                                            cx,
                                        )
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Edit,
                            tr("Rename Current Branch…", "重命名当前分支…"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::RenameBranch, cx)
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Push,
                            tr("Force Push (with lease)", "强制推送（含保护检查）"),
                            None,
                            true,
                            false,
                            {
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(
                                            PromptKind::Confirm(ConfirmAction::ForcePush),
                                            cx,
                                        )
                                    });
                                }
                            },
                        ));
                        // 当前分支的 upstream 管理：设置 / 取消。
                        if let Some(name) = current.clone() {
                            result = result.item(menu_item(
                                Ic::Pull,
                                format!("{} {name}…", tr("Set upstream of", "设置上游分支：")),
                                None,
                                false,
                                false,
                                {
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
                                },
                            ));
                            if let Some(upstream) = current_upstream.clone() {
                                result = result.item(menu_item(
                                    Ic::Pull,
                                    format!(
                                        "{} {name} ({upstream})",
                                        tr("Unset upstream of", "取消上游分支：")
                                    ),
                                    None,
                                    false,
                                    false,
                                    {
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.unset_branch_upstream(name.clone(), cx)
                                            });
                                        }
                                    },
                                ));
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
                                result = result.item(menu_item(
                                    Ic::Merge,
                                    merge_label,
                                    None,
                                    false,
                                    false,
                                    {
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update_in(cx, |this, window, cx| {
                                                if mode == MergeMode::NoFastForward {
                                                    this.open_merge_message(
                                                        name.clone(),
                                                        window,
                                                        cx,
                                                    );
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
                                    },
                                ));
                            }
                            result = result.item(menu_item(
                                Ic::Rebase,
                                format!("{} {name}…", tr("Rebase onto", "变基到")),
                                None,
                                false,
                                false,
                                {
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.rebase_current_onto(name.clone(), cx)
                                        });
                                    }
                                },
                            ));
                            let compare_label = format!(
                                "{} {name} ↔ {}",
                                tr("Compare", "比较"),
                                current.clone().unwrap_or_else(|| "HEAD".to_string())
                            );
                            result = result.item(menu_item(
                                Ic::Compare,
                                compare_label,
                                None,
                                false,
                                false,
                                {
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_branch_compare(name.clone(), cx)
                                        });
                                    }
                                },
                            ));
                            result = result.item(menu_item(
                                Ic::Edit,
                                format!("{} {name}…", tr("Rename", "重命名")),
                                None,
                                false,
                                false,
                                {
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
                                },
                            ));
                            result = result.item(menu_item(
                                Ic::Delete,
                                format!("{} {name}", tr("Delete", "删除")),
                                None,
                                true,
                                false,
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
                            // 每个本地分支的 upstream 管理。
                            let name = branch.name.clone();
                            result = result.item(menu_item(
                                Ic::Pull,
                                format!("{} {name}…", tr("Set upstream of", "设置上游分支：")),
                                None,
                                false,
                                false,
                                {
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
                                },
                            ));
                            if let Some(upstream) = branch.upstream.clone() {
                                result = result.item(menu_item(
                                    Ic::Pull,
                                    format!(
                                        "{} {name} ({upstream})",
                                        tr("Unset upstream of", "取消上游分支：")
                                    ),
                                    None,
                                    false,
                                    false,
                                    {
                                        let weak = weak.clone();
                                        let name = name.clone();
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |this, cx| {
                                                this.unset_branch_upstream(name.clone(), cx)
                                            });
                                        }
                                    },
                                ));
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
                        result = result.item(menu_item(
                            Ic::Add,
                            tr("Add Remote…", "添加远程仓库…"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(PromptKind::AddRemote, cx)
                                    });
                                }
                            },
                        ));
                        for remote in remote_list.iter() {
                            let label = format!("{} → {}", remote.name, remote.url);
                            result = result.item(PopupMenuItem::label(&label));
                            let prune_label = format!(
                                "{} {}",
                                tr("Prune remote branches of", "清理远程分支："),
                                remote.name
                            );
                            result = result.item(menu_item(
                                Ic::Fetch,
                                prune_label,
                                None,
                                false,
                                false,
                                {
                                    let weak = weak.clone();
                                    let name = remote.name.clone();
                                    move |_, _, cx| {
                                        let _ = weak
                                            .update(cx, |this, cx| this.prune_remote(&name, cx));
                                    }
                                },
                            ));
                            let remove_label = format!(
                                "{} {}",
                                tr("Remove remote", "移除远程仓库："),
                                remote.name
                            );
                            result = result.item(menu_item(
                                Ic::Delete,
                                remove_label,
                                None,
                                true,
                                false,
                                {
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
                                },
                            ));
                        }
                        menu_width(result)
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
                        let mut result = menu.item(menu_item(
                            Ic::Tag,
                            tr("New Tag on HEAD…", "在 HEAD 上新建标签…"),
                            None,
                            false,
                            false,
                            {
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
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Push,
                            tr("Push All Tags to origin", "推送所有标签到 origin"),
                            None,
                            false,
                            false,
                            {
                                let weak = tag_weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| this.push_all_tags(cx));
                                }
                            },
                        ));
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
                                        let result = menu
                                            .item(menu_item(
                                                Ic::Search,
                                                tr("Select Commit", "定位提交"),
                                                None,
                                                false,
                                                false,
                                                {
                                                    let weak = weak.clone();
                                                    let commit_id = commit_id.clone();
                                                    move |_, _, cx| {
                                                        let _ = weak.update(cx, |this, cx| {
                                                            this.select_commit_by_id(&commit_id, cx)
                                                        });
                                                    }
                                                },
                                            ))
                                            .item(menu_item(
                                                Ic::Push,
                                                tr("Push Tag to origin", "推送标签到 origin"),
                                                None,
                                                false,
                                                false,
                                                {
                                                    let weak = weak.clone();
                                                    let label = label.clone();
                                                    move |_, _, cx| {
                                                        let _ = weak.update(cx, |this, cx| {
                                                            this.push_tag(label.clone(), cx)
                                                        });
                                                    }
                                                },
                                            ))
                                            .item(menu_item(
                                                Ic::Edit,
                                                tr("Edit Message…", "编辑标签信息…"),
                                                None,
                                                false,
                                                false,
                                                {
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
                                                },
                                            ))
                                            .item(menu_item(
                                                Ic::Delete,
                                                tr("Delete Tag…", "删除标签…"),
                                                None,
                                                true,
                                                false,
                                                {
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
                                                },
                                            ));
                                        menu_width(result)
                                    },
                                );
                            }
                        }
                        menu_width(result)
                    }),
            )
            .child(v_separator(fg))
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
                Button::new("open")
                    .ghost()
                    .compact()
                    .icon(Ic::Folder)
                    .tooltip(tr("Open Project", "打开项目"))
                    .on_click(cx.listener(|this, _, window, cx| this.open_repo_dialog(window, cx))),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .compact()
                    .icon(Ic::Settings)
                    .tooltip(tr("Settings", "设置"))
                    .dropdown_menu_with_anchor(Anchor::TopRight, |menu, _window, cx| {
                        let is_zh = i18n::current() == Language::Zh;
                        let is_dark = cx.theme().is_dark();
                        let result = menu
                            .item(menu_item(
                                Ic::Language,
                                "English",
                                None,
                                false,
                                !is_zh,
                                |_, _, cx| {
                                    if i18n::current() != Language::En {
                                        i18n::set_current(Language::En);
                                        cx.refresh_windows();
                                    }
                                },
                            ))
                            .item(menu_item(
                                Ic::Language,
                                "中文",
                                None,
                                false,
                                is_zh,
                                |_, _, cx| {
                                    if i18n::current() != Language::Zh {
                                        i18n::set_current(Language::Zh);
                                        cx.refresh_windows();
                                    }
                                },
                            ))
                            .separator()
                            .item(menu_item(
                                Ic::Settings,
                                tr("Light Theme", "浅色主题"),
                                None,
                                false,
                                !is_dark,
                                |_, window, cx| {
                                    Theme::change(ThemeMode::Light, Some(window), cx);
                                    cx.set_window_appearance(Some(WindowAppearance::Light));
                                    settings::persist_theme_mode(ThemeMode::Light);
                                    cx.refresh_windows();
                                },
                            ))
                            .item(menu_item(
                                Ic::Settings,
                                tr("Dark Theme", "深色主题"),
                                None,
                                false,
                                is_dark,
                                |_, window, cx| {
                                    Theme::change(ThemeMode::Dark, Some(window), cx);
                                    theme::apply_jetbrains_palette(cx);
                                    cx.set_window_appearance(Some(WindowAppearance::Dark));
                                    settings::persist_theme_mode(ThemeMode::Dark);
                                    cx.refresh_windows();
                                },
                            ));
                        menu_width(result)
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
}
