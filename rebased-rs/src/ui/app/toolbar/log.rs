use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WeakEntity, div, px,
};
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size, button::Button, input::Input, list::List,
    menu::ContextMenuExt,
};

use crate::ui::app::{AppView, ConfirmAction, PromptKind};
use crate::ui::components::{menu_item, menu_width};
use crate::ui::graph_view::graph_column_width;
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

impl AppView {
    /// 左侧 Commit 面板（对齐原版布局）：工作区变更列表 + 底部提交输入区。
    /// 宽度由外壳的可拖拽分隔条控制（见 `AppView::commit_panel_width`）。
    pub(crate) fn render_commit_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(self.commit_panel_width))
            .flex_none()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_workspace(cx))
            .child(self.render_composer(cx))
            .into_any_element()
    }

    /// Log 表头：Subject / Author / Date 三列，列宽与提交行完全一致。
    /// 原实现无表头，导致 author/date 列语义不可见。
    fn render_log_columns(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let lane_count = self.list.read(cx).delegate().lane_count();
        div()
            .h(px(theme::SECTION_HEADER_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_MD))
            .px(px(theme::SPACE_MD))
            .border_b_1()
            .border_color(cx.theme().border)
            .text_size(px(theme::FONT_SIZE_META))
            .font_weight(theme::WEIGHT_MEDIUM)
            .text_color(muted)
            .child(
                div()
                    .w(graph_column_width(lane_count))
                    .flex_none()
                    .whitespace_nowrap()
                    .child(tr("Graph", "图谱")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(tr("Subject", "主题")),
            )
            .child(
                div()
                    .w(px(theme::COL_AUTHOR_WIDTH))
                    .flex_none()
                    .whitespace_nowrap()
                    .child(tr("Author", "作者")),
            )
            .child(
                div()
                    .w(px(theme::COL_DATE_WIDTH))
                    .flex_none()
                    .whitespace_nowrap()
                    .child(tr("Date", "日期")),
            )
    }

    /// Log 主区标题行（对齐原版 41px）：折叠指示 + “Log: <分支>” 蓝底白字标签。
    fn render_log_header(&self, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let branch = self
            .state
            .filter_branch
            .clone()
            .or_else(|| self.state.current_branch.clone())
            .unwrap_or_else(|| tr("All Branches", "所有分支").to_string());
        div()
            .h(px(theme::LOG_HEADER_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_MD))
            .px(px(theme::SPACE_MD))
            .child(
                Icon::new(Ic::ChevronDown)
                    .with_size(Size::XSmall)
                    .flex_none()
                    .text_color(muted),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(theme::SPACE_XS))
                    .py(px(theme::SPACE_XS))
                    .rounded(px(theme::RADIUS_SM))
                    .bg(theme::log_tag_bg())
                    .text_size(px(theme::FONT_SIZE_META))
                    .text_color(theme::white())
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(format!("{}: {branch}", tr("Log", "日志"))),
            )
            .child(div().flex_1())
    }

    /// 蓝字过滤 chip（对齐原版 “Branch: HEAD ×”），点击清除对应过滤。
    fn render_filter_chip(
        &self,
        id: &'static str,
        label: String,
        on_clear: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let link = theme::link_blue();
        div()
            .id(id)
            .flex_none()
            .flex()
            .items_center()
            .gap(px(theme::SPACE_XS))
            .px(px(theme::SPACE_SM))
            .rounded(px(theme::RADIUS_SM))
            .text_size(px(theme::FONT_SIZE_META))
            .text_color(link)
            .cursor_pointer()
            .hover(move |s| s.bg(theme::hover_bg(link)))
            .on_click(cx.listener(move |this, _, _, cx| on_clear(this, cx)))
            .child(label)
            .child(Icon::new(Ic::Close).with_size(Size::Small))
    }

    /// Log 过滤行（对齐原版 37px）：“Text or hash” 搜索框 + 激活的过滤 chips。
    fn render_log_filter_row(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(theme::LOG_FILTER_HEIGHT))
            .flex_none()
            .border_b_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_MD))
            .px(px(theme::SPACE_MD))
            .child(
                div().w(px(theme::SEARCH_WIDTH)).flex_none().child(
                    Input::new(&self.log_query)
                        .with_size(Size::Small)
                        .prefix(Icon::new(Ic::Search).text_color(cx.theme().muted_foreground))
                        .cleanable(true)
                        .appearance(false),
                ),
            )
            .when(self.state.filter_branch.is_some(), |row| {
                let branch = self.state.filter_branch.clone().unwrap_or_default();
                row.child(self.render_filter_chip(
                    "branch-chip",
                    format!("{}: {branch}", tr("Branch", "分支")),
                    |this, cx| this.set_branch_filter(None, cx),
                    cx,
                ))
            })
            .when(!self.state.filter_author.is_empty(), |row| {
                let author = self.state.filter_author.clone();
                row.child(self.render_filter_chip(
                    "author-chip",
                    format!("{}: {author}", tr("User", "用户")),
                    |this, cx| this.set_author_filter(String::new(), cx),
                    cx,
                ))
            })
            .when(self.state.filter_since.is_some(), |row| {
                let label = self
                    .state
                    .filter_since
                    .as_ref()
                    .map(|(label, _)| label.clone())
                    .unwrap_or_default();
                row.child(self.render_filter_chip(
                    "date-chip",
                    label,
                    |this, cx| this.set_date_filter(None, cx),
                    cx,
                ))
            })
            .child(div().flex_1())
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
        // 仓库尚未打开（启动路径无效或首次加载失败）：给出明确的错误态与重开入口。
        if self.repo.is_none() {
            return div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(theme::SPACE_LG))
                .child(
                    div()
                        .max_w(px(theme::MESSAGE_MAX_WIDTH))
                        .text_center()
                        .text_color(theme::error_color())
                        .child(self.state.error.clone().unwrap_or_else(|| {
                            tr("No repository is open", "尚未打开仓库").to_string()
                        })),
                )
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
            .child(self.render_log_header(cx))
            .child(self.render_log_filter_row(cx))
            .child(self.render_log_columns(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
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
