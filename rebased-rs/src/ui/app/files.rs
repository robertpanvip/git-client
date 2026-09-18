//! 文件视图（`MainView::Files`）：左 = 工作区文件夹树，右 = 代码区域。
//!
//! 对齐 IntelliJ「项目」工具窗口 + 编辑器的经典组合：
//! 左栏按目录层级浏览工作区文件（`git ls-files` 的 tracked + untracked 清单），
//! 右栏显示选中文件的代码，并逐行标出相对 HEAD 的变更与最近提交（行级 blame）。
//!
//! 两侧都是 `uniform_list`（只渲染可视行），逐行 / 逐树的派生数据在装载时算好
//! 并缓存，避免每次重绘重建上万行元素。

use std::sync::Arc;

use gpui::{AnyElement, AppContext, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_kit::component::ActiveTheme;

use crate::ui::blame_view::BlameJump;
use crate::ui::components::empty_state;
use crate::ui::components::panel_header;
use crate::ui::editor_view::render_editor;
use crate::ui::file_tree::{TreeClick, TreePick, flatten_tree, render_file_tree};
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
        eprintln!("[diag] auto-expanded toplevel dir: {}", dir);
        expanded.insert(dir);
    }
}

impl AppView {
    /// 进入文件视图；首次进入（或仓库切换后）在后台装载文件清单。
    ///
    /// 即使仓库还在后台加载（`self.repo == None`），也会标记「正在加载」，
    /// 避免 `apply_data` 仓库就绪后遗漏本次装载。`apply_data` 会在
    /// `MainView::Files` 时自动补调 `load_files`。
    pub(crate) fn open_files_view(&mut self, cx: &mut Context<Self>) {
        eprintln!(
            "[diag] open_files_view files={} loading={} repo={}",
            self.state.files.len(),
            self.state.files_loading,
            self.repo.is_some()
        );
        self.state.main_view = MainView::Files;
        if let Some(repo) = self.repo.clone() {
            if self.state.files.is_empty() {
                eprintln!("[diag] open_files_view: repo ready, loading files");
                self.state.files_loading = true;
                self.do_load_files(repo, cx);
            }
        } else {
            eprintln!("[diag] open_files_view: repo not ready, will wait for apply_data");
            self.state.files_loading = true;
        }
        cx.notify();
    }

    /// 后台装载工作区文件清单（tracked + untracked，遵循 .gitignore）。
    pub(crate) fn load_files(&mut self, cx: &mut Context<Self>) {
        eprintln!(
            "[diag] load_files called: files={} loading={} repo={}",
            self.state.files.len(),
            self.state.files_loading,
            self.repo.is_some()
        );
        let Some(repo) = self.repo.clone() else {
            eprintln!("[diag] load_files early return: repo is None");
            return;
        };
        self.state.files_loading = true;
        eprintln!("[diag] load_files calling do_load_files");
        self.do_load_files(repo, cx);
    }

    /// 实际执行后台文件装载（调用方保证 `repo` 非 None 且已置位 `files_loading`）。
    fn do_load_files(&mut self, repo: Arc<dyn GitBackend>, cx: &mut Context<Self>) {
        eprintln!("[diag] do_load_files: starting background task");
        let task =
            cx.background_spawn(async move { use_cases::load_worktree_files(repo.as_ref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            eprintln!(
                "[diag] do_load_files background task completed: ok={} err={}",
                result.is_ok(),
                result.as_ref().err().map(|e| e.to_string()).unwrap_or_default()
            );
            let _ = this.update(cx, |this, cx| {
                this.state.files_loading = false;
                match result {
                    Ok(files) => {
                        eprintln!(
                            "[diag] do_load_files: loaded {} files",
                            files.len()
                        );
                        if !files.is_empty() {
                            auto_expand_toplevel_dirs(&mut this.state.files_expanded, &files);
                        }
                        this.state.files = Arc::new(files);
                        this.rebuild_rows();
                    }
                    Err(e) => {
                        eprintln!("[diag] do_load_files error: {}", e);
                        this.state.error = Some(e.to_string())
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 展开 / 折叠文件夹树中的一个目录。
    pub(crate) fn toggle_dir(&mut self, path: String, cx: &mut Context<Self>) {
        eprintln!("[diag] toggle_dir {path}");
        if !self.state.files_expanded.remove(&path) {
            self.state.files_expanded.insert(path);
        }
        self.rebuild_rows();
        cx.notify();
    }

    /// 重算文件树可见行（文件清单或展开集合变化后调用一次）。
    fn rebuild_rows(&mut self) {
        self.state.files_rows =
            Arc::new(flatten_tree(&self.state.files, &self.state.files_expanded));
    }

    /// 打开（选中）一个文件：后台装载内容、行级 blame 与相对 HEAD 的 diff。
    ///
    /// 自动刷新会对当前文件重复调用本函数；只有切换文件时才清空右栏，
    /// 否则每次刷新都会闪一下空白。
    pub(crate) fn open_file(&mut self, path: String, cx: &mut Context<Self>) {
        eprintln!("[diag] open_file {path}");
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let switched = self.state.files_selected.as_deref() != Some(path.as_str());
        self.state.files_selected = Some(path.clone());
        if switched {
            self.state.files_binary = false;
            self.state.files_deleted = false;
            self.state.files_editor = Arc::new(Default::default());
            cx.notify();
        }
        let task =
            cx.background_spawn(async move { use_cases::load_worktree_file(repo.as_ref(), &path) });
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

        if self.state.files_loading {
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
            column = column.child(div().flex_1().min_h_0().child(render_file_tree(
                &self.state.files_rows,
                self.state.files_selected.as_deref(),
                &self.tree_scroll,
                &on_pick,
                cx,
            )));
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

        base.child(panel_header(path, muted, vec![change_legend(cx)]))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(render_editor(
                        &self.state.files_editor,
                        &self.editor_scroll,
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
