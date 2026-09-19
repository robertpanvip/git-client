use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WeakEntity, Window, div, px,
};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    button::{Button, ButtonVariants, DropdownButton},
    input::Input,
    list::List,
    menu::ContextMenuExt,
};

use crate::ui::app::{AppView, ChangesTab, ConfirmAction, PromptKind};
use crate::ui::components::{empty_state, error_state, menu_item, menu_width};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    /// 左侧 Commit 面板（对齐原版布局）：工作区变更列表 + 底部提交输入区。
    /// 宽度由外壳的可拖拽分隔条控制（见 `AppView::commit_panel_width`）。
    /// 贮藏页签下不显示提交输入区（对齐 IDEA：Shelve 页签无提交框）。
    pub(crate) fn render_commit_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(self.commit_panel_width))
            .flex_none()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_workspace(cx))
            .when(self.state.changes_tab == ChangesTab::Changes, |sidebar| {
                sidebar.child(self.render_composer(cx))
            })
            .into_any_element()
    }

    /// 过滤 chip（原版同款）：「分支: main ×」——标签弱色、值亮色，点击整体清除。
    fn render_filter_chip(
        &self,
        id: &'static str,
        label: String,
        value: String,
        on_clear: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted = theme::text_muted();
        let fg = theme::text_primary();
        div()
            .id(id)
            .flex_none()
            .flex()
            .items_center()
            .gap(px(theme::SPACE_XS))
            .px(px(theme::SPACE_XS))
            .rounded(px(theme::RADIUS_SM))
            .text_size(px(theme::font_size_meta()))
            .cursor_pointer()
            .hover(move |s| s.bg(theme::hover_bg(fg)))
            .on_click(cx.listener(move |this, _, _, cx| on_clear(this, cx)))
            .child(div().flex_none().text_color(muted).child(label))
            .child(div().flex_none().text_color(fg).child(value))
            .child(Icon::new(Ic::Close).with_size(Size::XSmall).text_color(muted))
    }

    /// 过滤行小工具按钮（原版工具栏同款）：24px 命中、灰图标、悬停提亮。
    fn render_log_tool_button(
        &self,
        id: &'static str,
        icon: Ic,
        _tooltip: String,
        active: bool,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let fg = if active {
            theme::white()
        } else {
            theme::text_muted()
        };
        div()
            .id(id)
            .size(px(theme::ICON_BUTTON_SIZE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS_SM))
            .text_color(fg)
            .cursor_pointer()
            .when(active, |b| b.bg(theme::selection_bg()))
            .when(!active, |b| b.hover(move |s| s.bg(theme::hover_bg(fg))))
            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                on_click(this, window, cx)
            }))
            .child(Icon::new(icon).with_size(Size::Small))
    }

    /// Log 过滤行（对齐原版）：搜索框「文本或哈希」（内嵌 `.*` / `Cc`）+
    /// 「分支: main ×」过滤 chip + 用户 / 日期 / 路径 过滤下拉 +
    /// 右侧工具按钮组（抓取 / 刷新 / 分支 / 预览 / 搜索）。
    ///
    /// 注意：caret 由 [`DropdownButton`] 自带的 popup 半区渲染，
    /// 内层 [`Button`] 不得再设 `dropdown_caret`，否则出现双箭头。
    fn render_log_filter_row(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        let branch_names: Vec<String> = self
            .state
            .branch_entries
            .iter()
            .map(|b| b.name.clone())
            .collect();
        // 「用户」下拉的数据源：已加载日志的去重作者名（原版同样从当前日志取作者集）。
        let author_names = self.list.read(cx).delegate().authors();
        let filter_branch = self.state.filter_branch.clone();
        let filter_author = self.state.filter_author.clone();
        let filter_since = self.state.filter_since.clone();
        let filter_path = self.state.filter_path.clone();
        let regex_on = self.list.read(cx).delegate().regex;
        let path_candidates = self.filter_path_candidates();

        // 原版语义：无激活值时下拉按钮固定显示过滤器名；
        // 分支过滤激活后改为「分支: main ×」chip 呈现，不再重复出下拉。
        let author_label = tr("User", "用户").to_string();
        let date_label = tr("Date", "日期").to_string();
        let path_label = tr("Path", "路径").to_string();

        let branch_weak = weak.clone();
        let branch_filter = filter_branch.clone();
        let author_weak = weak.clone();
        let author_filter = filter_author.clone();
        let date_weak = weak.clone();
        let date_filter = filter_since.clone();
        let path_weak = weak.clone();
        let path_filter = filter_path.clone();

        div()
            .h(px(theme::LOG_FILTER_HEIGHT))
            .flex_none()
            .bg(theme::log_list_bg())
            .border_b_1()
            .border_color(theme::separator())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_SM))
            .px(px(theme::SPACE_SM))
            // 搜索框：外框自绘（边框 + 内嵌 `.*` / `Cc`），对齐原版搜索框观感。
            .child(
                div()
                    .w(px(theme::SEARCH_WIDTH))
                    .h(px(theme::INPUT_HEIGHT_SINGLE - 4.0))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(theme::SPACE_XS))
                    .px(px(theme::SPACE_SM))
                    .bg(theme::input_bg())
                    .border_1()
                    .border_color(theme::input_border())
                    .rounded(px(theme::RADIUS_SM))
                    .overflow_hidden()
                    .child(
                        Icon::new(Ic::Search)
                            .with_size(Size::XSmall)
                            .flex_none()
                            .text_color(muted),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.log_query)
                                .with_size(Size::Small)
                                .appearance(false)
                                .cleanable(false),
                        ),
                    )
                    .child(self.render_log_mini_button(
                        "log-regex-toggle",
                        ".*",
                        tr("Wildcard match", "通配符匹配").to_string(),
                        regex_on,
                        |this, window, cx| this.toggle_log_regex(window, cx),
                        cx,
                    ))
                    .child(self.render_log_mini_button(
                        "log-clear",
                        "Cc",
                        tr("Clear search", "清空搜索").to_string(),
                        false,
                        |this, window, cx| this.clear_log_query(window, cx),
                        cx,
                    )),
            )
            // 分支过滤：未激活 = 下拉；激活 = 「分支: main ×」chip。
            .when_some(filter_branch.clone(), |row, branch| {
                row.child(self.render_filter_chip(
                    "branch-chip",
                    format!("{}:", tr("Branch", "分支")),
                    branch,
                    |this, cx| this.set_branch_filter(None, cx),
                    cx,
                ))
            })
            .when(filter_branch.is_none(), |row| {
                row.child(
                    DropdownButton::new("log-branch-filter")
                        .button(
                            Button::new("log-branch-filter-button")
                                .ghost()
                                .compact()
                                .label(tr("Branch", "分支"))
                            )
                        .dropdown_menu(move |menu, _window, _cx| {
                            let mut result = menu.item(menu_item(
                                Ic::Branch,
                                tr("All Branches", "所有分支"),
                                None,
                                false,
                                branch_filter.is_none(),
                                {
                                    let weak = branch_weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_branch_filter(None, cx)
                                        });
                                    }
                                },
                            ));
                            for name in branch_names.iter() {
                                let checked = branch_filter.as_deref() == Some(name.as_str());
                                let weak = branch_weak.clone();
                                let name = name.clone();
                                result = result.item(menu_item(
                                    Ic::Branch,
                                    name.clone(),
                                    None,
                                    false,
                                    checked,
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.set_branch_filter(Some(name.clone()), cx)
                                        });
                                    },
                                ));
                            }
                            menu_width(result)
                        }),
                )
            })
            .child(
                DropdownButton::new("log-author-filter")
                    .button(
                        Button::new("log-author-filter-button")
                            .ghost()
                            .compact()
                            .label(author_label)
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(menu_item(
                            Ic::User,
                            tr("All Users", "所有用户"),
                            None,
                            false,
                            author_filter.is_empty(),
                            {
                                let weak = author_weak.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_author_filter(String::new(), cx)
                                    });
                                }
                            },
                        ));
                        for name in author_names.iter() {
                            let checked = author_filter.as_str() == name.as_str();
                            let weak = author_weak.clone();
                            let name = name.clone();
                            result = result.item(menu_item(
                                Ic::User,
                                name.clone(),
                                None,
                                false,
                                checked,
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_author_filter(name.clone(), cx)
                                    });
                                },
                            ));
                        }
                        menu_width(result)
                    }),
            )
            .child(
                DropdownButton::new("log-date-filter")
                    .button(
                        Button::new("log-date-filter-button")
                            .ghost()
                            .compact()
                            .label(date_label)
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(menu_item(
                            Ic::History,
                            tr("All Time", "全部时间"),
                            None,
                            false,
                            date_filter.is_none(),
                            {
                                let weak = date_weak.clone();
                                move |_, _, cx| {
                                    let _ =
                                        weak.update(cx, |this, cx| this.set_date_filter(None, cx));
                                }
                            },
                        ));
                        let date_specs = [
                            (tr("Today", "今天"), "midnight"),
                            (tr("This week", "本周"), "1 week ago"),
                            (tr("This month", "本月"), "1 month ago"),
                            (tr("This year", "今年"), "1 year ago"),
                        ];
                        for (label, expr) in date_specs {
                            let checked =
                                date_filter.as_ref().map(|(_, e)| e.as_str()) == Some(expr);
                            let weak = date_weak.clone();
                            result = result.item(menu_item(
                                Ic::History,
                                label,
                                None,
                                false,
                                checked,
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
                        menu_width(result)
                    }),
            )
            .child(
                DropdownButton::new("log-path-filter")
                    .button(
                        Button::new("log-path-filter-button")
                            .ghost()
                            .compact()
                            .label(filter_path.clone().unwrap_or_else(|| path_label.clone()))
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        let mut result = menu.item(menu_item(
                            Ic::Folder,
                            tr("All Paths", "所有路径"),
                            None,
                            false,
                            path_filter.is_none(),
                            {
                                let weak = path_weak.clone();
                                move |_, _, cx| {
                                    let _ =
                                        weak.update(cx, |this, cx| this.set_path_filter(None, cx));
                                }
                            },
                        ));
                        for name in path_candidates.iter() {
                            let checked = path_filter.as_deref() == Some(name.as_str());
                            let weak = path_weak.clone();
                            let name = name.clone();
                            result = result.item(menu_item(
                                Ic::Folder,
                                name.clone(),
                                None,
                                false,
                                checked,
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.set_path_filter(Some(name.clone()), cx)
                                    });
                                },
                            ));
                        }
                        menu_width(result)
                    }),
            )
            .child(
                Icon::new(Ic::ChevronRight)
                    .with_size(Size::XSmall)
                    .flex_none()
                    .text_color(muted),
            )
            .child(div().flex_1())
            .child(self.render_log_tool_button(
                "log-fetch",
                Ic::PlayCircle,
                tr("Fetch", "抓取").to_string(),
                false,
                |this, _, cx| {
                    this.run_op_progress(
                        tr("Fetch", "抓取"),
                        tr("Fetched", "已抓取"),
                        |repo, progress, cancel| repo.fetch_with_control(progress, cancel),
                        cx,
                    );
                },
                cx,
            ))
            .child(self.render_log_tool_button(
                "log-refresh",
                Ic::Refresh,
                tr("Refresh", "刷新").to_string(),
                false,
                |this, _, cx| this.refresh(cx),
                cx,
            ))
            .child(self.render_log_tool_button(
                "log-branches",
                Ic::Branch,
                tr("Search branches and actions", "搜索分支和操作").to_string(),
                false,
                |this, window, cx| this.toggle_branch_popup(window, cx),
                cx,
            ))
            .child(self.render_log_tool_button(
                "log-preview",
                Ic::Eye,
                tr("Reveal HEAD commit", "跳到 HEAD 提交").to_string(),
                false,
                |this, window, cx| this.reveal_head_commit(window, cx),
                cx,
            ))
            .child(self.render_log_tool_button(
                "log-search",
                Ic::Search,
                tr("Search", "搜索").to_string(),
                false,
                |this, window, cx| this.focus_log_query(window, cx),
                cx,
            ))
    }

    /// 搜索框内嵌的迷你文本按钮（`.*` / `Cc`）。
    fn render_log_mini_button(
        &self,
        id: &'static str,
        label: &'static str,
        _tooltip: String,
        active: bool,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let fg = if active {
            theme::white()
        } else {
            theme::text_muted()
        };
        div()
            .id(id)
            .w(px(22.0))
            .h(px(18.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(theme::RADIUS_SM))
            .text_size(px(theme::font_size_xs()))
            .text_color(fg)
            .cursor_pointer()
            .when(active, |b| b.bg(theme::selection_bg()))
            .when(!active, |b| b.hover(move |s| s.bg(theme::hover_bg(fg))))
            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                on_click(this, window, cx)
            }))
            .child(label)
    }

    pub(crate) fn render_commit_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.loading {
            return empty_state(
                tr("Loading repository...", "正在加载仓库..."),
                cx.theme().muted_foreground,
            )
            .flex_1()
            .min_w_0()
            .into_any_element();
        }
        // 仓库尚未打开（启动路径无效或首次加载失败）：给出明确的错误态与重开入口。
        if self.repo.is_none() {
            return error_state(
                tr("No repository is open", "尚未打开仓库"),
                self.state.error.clone().map(gpui::SharedString::from),
            )
            .flex_1()
            .min_w_0()
            .child(
                Button::new("open-project-error")
                    .label(tr("Open Project...", "打开项目..."))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_repo_dialog(window, cx)),
                    ),
            )
            .into_any_element();
        }
        let list = self.list.clone();
        let weak: WeakEntity<Self> = cx.entity().downgrade();
        div()
            .id("commit-panel")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_log_filter_row(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(theme::log_list_bg())
                    .child(List::new(&self.list))
                    .context_menu(move |menu, _window, cx| {
                        let row = list.read(cx).right_clicked_index().map(|ix| ix.row);
                        let Some(commit) =
                            row.and_then(|row| list.read(cx).delegate().commit_at(row))
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
                        let mut result = menu.item(menu_item(
                            Ic::Checkout,
                            format!("{} {short}", tr("Checkout", "检出")),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.checkout_commit(id.clone(), cx)
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Add,
                            tr("New Branch…", "新建分支…"),
                            None,
                            false,
                            false,
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
                        result = result.item(menu_item(
                            Ic::Tag,
                            tr("New Tag…", "新建标签…"),
                            None,
                            false,
                            false,
                            {
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
                            },
                        ));
                        result = result.separator();
                        result = result.item(menu_item(
                            Ic::Copy,
                            tr("Cherry-pick", "摘取提交"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.cherry_pick_commit(id.clone(), cx)
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Revert,
                            tr("Revert Commit", "回滚提交"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak
                                        .update(cx, |this, cx| this.revert_commit(id.clone(), cx));
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Undo,
                            tr("Reset Current Branch to Here…", "重置当前分支到此处…"),
                            None,
                            true,
                            false,
                            {
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
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Rebase,
                            tr("Rebase from Here", "从这里变基"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak
                                        .update(cx, |this, cx| this.start_rebase(id.clone(), cx));
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Edit,
                            tr("Reword Message…", "修改提交信息…"),
                            None,
                            false,
                            false,
                            {
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
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Merge,
                            tr("Fixup into Parent", "fixup 到父提交"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        let short = id[..id.len().min(7)].to_string();
                                        let message = format!(
                                            "{} {short}",
                                            tr("Fixuped", "已 fixup 到父提交")
                                        );
                                        let cid = id.clone();
                                        this.run_op(
                                            &message,
                                            move |repo| repo.fixup_commit(&cid),
                                            cx,
                                        );
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Merge,
                            tr("Squash into Parent…", "squash 到父提交…"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_prompt(
                                            PromptKind::Squash {
                                                commit_id: id.clone(),
                                            },
                                            cx,
                                        );
                                    });
                                }
                            },
                        ));
                        result = result.separator();
                        result = result.item(menu_item(
                            Ic::Diff,
                            tr("Diff", "查看差异"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_commit_diff_window(id.clone(), None, cx)
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Compare,
                            tr("Compare with Current Branch", "与当前分支比较"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.open_branch_compare(id.clone(), cx)
                                    });
                                }
                            },
                        ));
                        result = result.item(menu_item(
                            Ic::Copy,
                            tr("Copy SHA", "复制 SHA"),
                            None,
                            false,
                            false,
                            {
                                let weak = weak.clone();
                                let id = id.clone();
                                move |_, _, cx| {
                                    let _ = weak
                                        .update(cx, |this, cx| this.copy_commit_id(id.clone(), cx));
                                }
                            },
                        ));
                        if is_head {
                            result = result.item(menu_item(
                                Ic::Undo,
                                tr("Undo Commit", "撤销提交"),
                                None,
                                false,
                                false,
                                {
                                    let weak = weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::Confirm(ConfirmAction::UndoHeadCommit),
                                                cx,
                                            )
                                        });
                                    }
                                },
                            ));
                            result = result.item(menu_item(
                                Ic::Delete,
                                tr("Drop Commit", "丢弃提交"),
                                None,
                                true,
                                false,
                                {
                                    let weak = weak.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::Confirm(ConfirmAction::DropHeadCommit),
                                                cx,
                                            )
                                        });
                                    }
                                },
                            ));
                        } else {
                            result = result.item(menu_item(
                                Ic::Undo,
                                tr("Uncommit Commit…", "撤销该提交…"),
                                None,
                                false,
                                false,
                                {
                                    let weak = weak.clone();
                                    let id = id.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::Confirm(
                                                    ConfirmAction::UncommitCommit {
                                                        commit_id: id.clone(),
                                                    },
                                                ),
                                                cx,
                                            )
                                        });
                                    }
                                },
                            ));
                            result = result.item(menu_item(
                                Ic::Delete,
                                tr("Drop Commit…", "丢弃该提交…"),
                                None,
                                true,
                                false,
                                {
                                    let weak = weak.clone();
                                    let id = id.clone();
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.open_prompt(
                                                PromptKind::Confirm(ConfirmAction::DropCommit {
                                                    commit_id: id.clone(),
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
            .into_any_element()
    }
}
