//! 文件视图（`MainView::Files`）：左 = 工作区文件夹树，右 = 代码区域。
//!
//! 对齐 IntelliJ「项目」工具窗口 + 编辑器的经典组合：
//! 左栏按目录层级浏览工作区文件（`git ls-files` 的 tracked + untracked 清单），
//! 右栏显示选中文件的代码，并逐行标出相对 HEAD 的变更与最近提交（行级 blame）。

use std::sync::Arc;

use gpui::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_kit::component::ActiveTheme;

use crate::ui::blame_view::BlameJump;
use crate::ui::components::empty_state;
use crate::ui::components::panel_header;
use crate::ui::editor_view::{line_changes, render_editor};
use crate::ui::file_tree::{TreeClick, TreePick, render_file_tree};
use crate::ui::i18n::tr;
use crate::ui::theme;

use super::{AppView, MainView, use_cases};

impl AppView {
    /// 进入文件视图；首次进入（或仓库切换后）在后台装载文件清单。
    pub(crate) fn open_files_view(&mut self, cx: &mut Context<Self>) {
        self.state.main_view = MainView::Files;
        if self.state.files.is_empty() {
            self.load_files(cx);
        }
        cx.notify();
    }

    /// 后台装载工作区文件清单（tracked + untracked，遵循 .gitignore）。
    pub(crate) fn load_files(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let task =
            cx.background_spawn(async move { use_cases::load_worktree_files(repo.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(files) => {
                    this.state.files = Arc::new(files);
                    cx.notify();
                }
                Err(e) => {
                    this.state.error = Some(e.to_string());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 展开 / 折叠文件夹树中的一个目录。
    pub(crate) fn toggle_dir(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.state.files_expanded.remove(&path) {
            self.state.files_expanded.insert(path);
        }
        cx.notify();
    }

    /// 打开（选中）一个文件：后台装载内容、行级 blame 与相对 HEAD 的 diff。
    pub(crate) fn open_file(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        self.state.files_selected = Some(path.clone());
        self.state.files_content.clear();
        self.state.files_binary = false;
        self.state.files_blame.clear();
        self.state.files_diff.clear();
        cx.notify();
        let task =
            cx.background_spawn(async move { use_cases::load_worktree_file(repo.as_ref(), &path) });
        cx.spawn(async move |this, cx| {
            let data = task.await;
            let _ = this.update(cx, |this, cx| {
                // 期间用户可能已切换到别的文件：丢弃过期结果。
                if this.state.files_selected.as_deref() != Some(data.path.as_str()) {
                    return;
                }
                this.state.files_content = data.content;
                this.state.files_binary = data.binary;
                this.state.files_blame = data.blame;
                this.state.files_diff = data.diff;
                cx.notify();
            });
        })
        .detach();
    }

    /// 文件视图左栏：工作区文件夹树。
    pub(crate) fn render_files_tree_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let on_pick: TreePick = {
            let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
            Arc::new(move |click: TreeClick, app: &mut gpui::App| {
                let _ = weak.update(app, |this, cx| match click {
                    TreeClick::Dir(path) => this.toggle_dir(path, cx),
                    TreeClick::File(path) => this.open_file(path, cx),
                });
            })
        };

        let mut column = div()
            .w(px(theme::FILE_TREE_PANEL_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::bg_main())
            .border_r_1()
            .border_color(theme::separator())
            .child(panel_header(tr("Project", "项目"), muted, Vec::new()));

        if self.state.files.is_empty() {
            column = column.child(empty_state(
                tr("No files to show", "没有可显示的文件"),
                muted,
            ));
        } else {
            column = column.child(
                div()
                    .id("file-tree")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_file_tree(
                        &self.state.files,
                        &self.state.files_expanded,
                        self.state.files_selected.as_deref(),
                        &on_pick,
                        cx,
                    )),
            );
        }
        column.into_any_element()
    }

    /// 文件视图右栏：代码区域（行号 + 行变更标记 + 行级 blame）。
    pub(crate) fn render_files_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let base = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::bg_main());

        let Some(path) = self.state.files_selected.clone() else {
            return base
                .child(empty_state(
                    tr("Select a file to view its code", "选择文件以查看代码"),
                    muted,
                ))
                .into_any_element();
        };

        let on_commit: BlameJump = {
            let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
            Arc::new(move |id, app| {
                let _ = weak.update(app, |this, cx| this.open_commit_diff(id, None, cx));
            })
        };

        if self.state.files_binary {
            let message = if self.state.files_diff.iter().any(|file| file.is_deleted) {
                tr(
                    "This file is deleted in the working tree",
                    "该文件已在工作区中删除",
                )
            } else {
                tr("Binary file cannot be previewed", "二进制文件无法预览")
            };
            return base.child(empty_state(message, muted)).into_any_element();
        }
        let changes = line_changes(&self.state.files_diff);

        base.child(panel_header(path, muted, vec![change_legend(cx)]))
            .child(
                div()
                    .id("editor-content")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .overflow_x_scroll()
                    .child(render_editor(
                        &self.state.files_content,
                        &changes,
                        &self.state.files_blame,
                        Some(&on_commit),
                        cx,
                    )),
            )
            .into_any_element()
    }
}

/// 行变更标记图例（新增 / 修改 / 删除），说明代码区左侧窄条的语义。
fn change_legend(cx: &gpui::App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entries = [
        (theme::added_color(), tr("Added", "新增")),
        (theme::modified_color(), tr("Modified", "修改")),
        (theme::deleted_color(), tr("Deleted", "删除")),
    ];
    let mut legend = div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD));
    for (color, label) in entries {
        legend = legend.child(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(theme::SPACE_XS))
                .child(
                    div()
                        .w(px(theme::EDITOR_CHANGE_BAR_WIDTH))
                        .h(px(theme::FONT_SIZE_META))
                        .flex_none()
                        .bg(color),
                )
                .child(
                    div()
                        .text_size(px(theme::FONT_SIZE_META))
                        .text_color(muted)
                        .child(label),
                ),
        );
    }
    legend.into_any_element()
}
