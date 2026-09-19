//! 文件视图（`MainView::Files`）：左 = 工作区文件夹树，右 = 代码区域。
//!
//! 对齐 IntelliJ「项目」工具窗口 + 编辑器的经典组合：
//! 左栏按目录层级浏览工作区文件（`git ls-files` 的 tracked + untracked 清单），
//! 右栏显示选中文件的代码，并逐行标出相对 HEAD 的变更与最近提交（行级 blame）。
//!
//! 左栏为文件树（渲染时实时摊平，与 v0.6.0 一致），右栏显示代码区域，
//! 并逐行标出相对 HEAD 的变更与最近提交（行级 blame）。

use std::sync::Arc;

use gpui::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement, Styled, div,
    px,
};
use gpui::StatefulInteractiveElement;
use gpui_kit::component::{ActiveTheme, button::Button};

use crate::ui::blame_view::BlameJump;
use crate::ui::blame_view::BlameToggle;
use crate::ui::components::{empty_state, error_state};
use crate::ui::components::file_tab_bar::{TabActivate, TabClose, file_tab_bar, neighbor_after_close};
use crate::ui::components::panel_header;
use crate::ui::editor_view::render_editor;
use crate::ui::file_tree::{TreeClick, TreePick, render_file_tree};
use crate::ui::i18n::tr;
use crate::ui::theme;

use rebased_rs::git::GitBackend;

use super::{AppView, MainView, use_cases};

/// 文件加载成功后自动展开第一层目录，避免用户看到空树。
fn auto_expand_toplevel_dirs(expanded: &mut std::collections::HashSet<String>, files: &[String]) {
    let mut toplevel_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for file in files {
        if let Some(slash_pos) = file.find('/') {
            let dir = &file[..slash_pos];
            if !dir.is_empty() {
                toplevel_dirs.insert(dir.to_string());
            }
        }
    }
    for dir in toplevel_dirs {
        expanded.insert(dir);
    }
}

impl AppView {
    /// 进入文件视图；首次进入（或仓库切换后）在后台装载文件清单。
    pub(crate) fn open_files_view(&mut self, cx: &mut Context<Self>) {
        self.state.main_view = MainView::Files;
        if self.state.files.is_empty() && !self.state.files_loading {
            self.load_files(cx);
        }
        cx.notify();
    }

    /// 后台装载工作区文件清单（tracked + untracked，遵循 .gitignore）。
    pub(crate) fn load_files(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        self.state.files_loading = true;
        self.do_load_files(repo, cx);
    }

    /// 实际执行后台文件装载（调用方保证 `repo` 非 None 且已置位 `files_loading`）。
    fn do_load_files(&mut self, repo: Arc<dyn GitBackend>, cx: &mut Context<Self>) {
        let task =
            cx.background_spawn(async move { use_cases::load_worktree_files(repo.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.files_loading = false;
                match result {
                    Ok(files) => {
                        if !files.is_empty() {
                            auto_expand_toplevel_dirs(&mut this.state.files_expanded, &files);
                        }
                        this.state.files = Arc::new(files);
                    }
                    Err(e) => this.state.error = Some(e.to_string()),
                }
                cx.notify();
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
    ///
    /// 自动刷新会对当前文件重复调用本函数；只有切换文件时才清空右栏，
    /// 否则每次刷新都会闪一下空白。
    pub(crate) fn open_file(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let switched = self.state.files_selected.as_deref() != Some(path.as_str());
        // Tab 模型（T-1）：激活即打开。新 Tab 插到当前 Tab 右侧（IDEA 行为）；
        // 已打开的 Tab 只激活，不重复创建。所有「打开文件」入口都经由此函数，
        // 因此文件树 / blame 跳转等入口自动纳入 Tab 管理。
        if !self.state.open_tabs.contains(&path) {
            let insert_at = match self.state.files_selected.as_ref() {
                Some(active) => self
                    .state
                    .open_tabs
                    .iter()
                    .position(|p| p == active)
                    .map_or(self.state.open_tabs.len(), |pos| pos + 1),
                None => self.state.open_tabs.len(),
            };
            self.state.open_tabs.insert(insert_at, path.clone());
        }
        self.state.files_selected = Some(path.clone());
        if switched {
            self.state.files_binary = false;
            self.state.files_deleted = false;
            self.state.files_editor = Arc::new(Default::default());
            // 与 IDEA 一致：打开新文件从顶部阅读，且不残留上一个文件的滚动位置。
            self.editor_scroll
                .scroll_to_item(0, gpui::ScrollStrategy::Top);
            cx.notify();
        }
        let blame = self.state.files_blame_enabled;
        let ignore_whitespace = self.state.ignore_whitespace;
        let task = cx.background_spawn(async move {
            use_cases::load_worktree_file(repo.as_ref(), &path, blame, ignore_whitespace)
        });
        cx.spawn(async move |this, cx| {
            let data = task.await;
            let _ = this.update(cx, |this, cx| {
                // 期间用户可能已切换到别的文件：丢弃过期结果。
                if this.state.files_selected.as_deref() != Some(data.path.as_str()) {
                    return;
                }
                this.state.files_binary = data.binary;
                this.state.files_deleted = data.deleted;
                this.state.files_editor = Arc::new(data.editor);
                cx.notify();
            });
        })
        .detach();
    }

    /// 切换 blame 注解（编辑器右键 → Annotate with Git / Close Annotations）：
    /// 置位后按当前开关重新装载当前文件，行级「提交者 + 日期」随之显示/隐藏。
    pub(crate) fn set_blame_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.state.files_blame_enabled == enabled {
            return;
        }
        self.state.files_blame_enabled = enabled;
        if let Some(path) = self.state.files_selected.clone() {
            self.open_file(path, cx);
        }
        cx.notify();
    }

    /// 关闭一个文件 Tab：关闭非当前 Tab 时激活态不变；关闭当前 Tab 时按
    /// IDEA 语义选中邻位（优先右邻、无右邻取左邻）；关闭最后一个 Tab
    /// 回到空态（清除右栏数据，见 `render_files_editor` 的空态分支）。
    pub(crate) fn close_file_tab(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(pos) = self.state.open_tabs.iter().position(|p| *p == path) else {
            return;
        };
        self.state.open_tabs.remove(pos);
        let was_active = self.state.files_selected.as_deref() == Some(path.as_str());
        if was_active {
            let neighbor = neighbor_after_close(self.state.open_tabs.len(), pos)
                .and_then(|index| self.state.open_tabs.get(index).cloned());
            match neighbor {
                Some(next) => {
                    // open_file 内部完成激活、装载与 notify。
                    self.open_file(next, cx);
                    return;
                }
                None => {
                    self.state.files_selected = None;
                    self.state.files_binary = false;
                    self.state.files_deleted = false;
                    self.state.files_editor = Arc::new(Default::default());
                }
            }
        }
        cx.notify();
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

        if let Some(ref error) = self.state.error {
            let weak = cx.entity().downgrade();
            let retry_button = Button::new("retry-files")
                .label(tr("Retry", "重试"))
                .on_click(move |_, _window, app| {
                    let _ = weak.update(app, |this, cx| {
                        this.state.error = None;
                        this.state.files = Arc::new(Vec::new());
                        this.open_files_view(cx);
                    });
                });
            column = column.child(
                error_state(
                    tr("Failed to load files", "文件加载失败"),
                    Some(gpui::SharedString::from(error.clone())),
                )
                .child(retry_button),
            );
        } else if self.state.files_loading {
            column = column.child(empty_state(
                tr("Loading files...", "正在加载文件..."),
                muted,
            ));
        } else if self.state.files.is_empty() {
            column = column.child(empty_state(
                tr("No files to show", "没有可显示的文件"),
                muted,
            ));
        } else {
            // 文件树状态色与 Changes 面板同源：path → Git 状态，由统一
            // `theme::status_color` 消费（F5：IDEA 项目树对变更文件着色）。
            let status_of: std::collections::HashMap<String, rebased_rs::git::ChangeStatus> =
                self.state
                    .changes
                    .iter()
                    .map(|change| (change.path.clone(), change.status))
                    .collect();
            column = column.child(
                div()
                    .id("file-tree")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(render_file_tree(
                        &self.state.files,
                        &self.state.files_expanded,
                        &status_of,
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
        let mut base = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme::bg_main());

        // 文件 Tab 条（T-1/T-2）：有已打开文件就常驻编辑器顶部，
        // active Tab 高亮下划线，× 关闭（邻位选中在 close_file_tab）。
        if !self.state.open_tabs.is_empty() {
            let on_activate: TabActivate = {
                let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                Arc::new(move |path, app| {
                    let _ = weak.update(app, |this, cx| this.open_file(path, cx));
                })
            };
            let on_close: TabClose = {
                let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
                Arc::new(move |path, app| {
                    let _ = weak.update(app, |this, cx| this.close_file_tab(path, cx));
                })
            };
            base = base.child(file_tab_bar(
                &self.state.open_tabs,
                self.state.files_selected.as_deref(),
                &on_activate,
                &on_close,
                cx,
            ));
        }

        let Some(path) = self.state.files_selected.clone() else {
            return base
                .child(empty_state(
                    tr("Select a file to view its code", "选择文件以查看代码"),
                    muted,
                ))
                .into_any_element();
        };

        if self.state.files_binary {
            let message = if self.state.files_deleted {
                tr(
                    "This file is deleted in the working tree",
                    "该文件已在工作区中删除",
                )
            } else {
                tr("Binary file cannot be previewed", "二进制文件无法预览")
            };
            return base.child(empty_state(message, muted)).into_any_element();
        }

        let on_commit: BlameJump = {
            let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
            Arc::new(move |id, app| {
                let _ = weak.update(app, |this, cx| this.open_commit_diff(id, None, cx));
            })
        };

        let on_toggle_blame: BlameToggle = {
            let weak: gpui::WeakEntity<AppView> = cx.entity().downgrade();
            Arc::new(move |enabled, app| {
                let _ = weak.update(app, |this, cx| this.set_blame_enabled(enabled, cx));
            })
        };

        base.child(panel_header(path, muted, vec![change_legend(cx)]))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .overflow_hidden()
                    .child(render_editor(
                        &self.state.files_editor,
                        &self.editor_scroll,
                        Some(&on_commit),
                        self.state.files_blame_enabled,
                        Some(&on_toggle_blame),
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
                        .h(px(theme::font_size_meta()))
                        .flex_none()
                        .bg(color),
                )
                .child(
                    div()
                        .text_size(px(theme::font_size_meta()))
                        .text_color(muted)
                        .child(label),
                ),
        );
    }
    legend.into_any_element()
}
