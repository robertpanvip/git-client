//! 独立 diff 对比窗口（对齐原版 IntelliJ：变更差异在独立窗口打开，而非内嵌侧栏）。
//!
//! 该视图与主窗口 `AppView` 完全解耦：自行持有 repo 与 diff 来源元数据，
//! 可独立切换「忽略空白 / 并排视图」并执行 hunk 级暂存/取消暂存后再刷新自身。

use std::collections::HashSet;

use gpui::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    px, size,
};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    ActiveTheme, Root,
    button::{Button, ButtonVariants},
};
use rebased_rs::git::{FileDiff, GitBackend, parse_unified_diff};

use crate::ui::components::{empty_state, error_state};
use crate::ui::diff_view::{HunkAction, scroll_hunk_into_view, step_hunk};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

use super::DiffSource;

/// 打开独立 diff 窗口所需的完整上下文（数据 + git 后端 + 显示开关）。
pub(crate) struct DiffWindowSpec {
    pub(crate) repo: std::sync::Arc<dyn GitBackend>,
    pub(crate) title: String,
    pub(crate) source: DiffSource,
    pub(crate) path: Option<String>,
    pub(crate) commit_id: String,
    pub(crate) ignore_whitespace: bool,
    pub(crate) side_by_side: bool,
    pub(crate) files: Vec<FileDiff>,
}

pub(crate) struct DiffWindowView {
    repo: std::sync::Arc<dyn GitBackend>,
    title: String,
    source: DiffSource,
    path: Option<String>,
    commit_id: String,
    ignore_whitespace: bool,
    side_by_side: bool,
    files: Vec<FileDiff>,
    error: Option<String>,
    /// 折叠的 hunk 集合（(file_index, hunk_index)），与主窗口 diff_folded 同语义。
    folded: HashSet<(usize, usize)>,
    /// F7 / Shift+F7 导航的当前位置。
    diff_nav: Option<(usize, usize)>,
    /// 滚动容器句柄，供 hunk 导航程序化滚动定位。
    scroll: ScrollHandle,
}

impl DiffWindowView {
    pub(crate) fn new(spec: DiffWindowSpec, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            repo: spec.repo,
            title: spec.title,
            source: spec.source,
            path: spec.path,
            commit_id: spec.commit_id,
            ignore_whitespace: spec.ignore_whitespace,
            side_by_side: spec.side_by_side,
            files: spec.files,
            error: None,
            folded: HashSet::new(),
            diff_nav: None,
            scroll: ScrollHandle::new(),
        }
    }

    /// 按当前来源重新拉取 diff（staged/unstaged/commit），供切换开关与 hunk 操作后刷新。
    fn reload(&mut self, cx: &mut Context<Self>) {
        let stdout = match self.source {
            DiffSource::Staged => self
                .repo
                .diff_staged(self.path.as_deref(), self.ignore_whitespace),
            DiffSource::Unstaged => self
                .repo
                .diff_unstaged(self.path.as_deref(), self.ignore_whitespace),
            DiffSource::Commit => self.repo.show_diff(
                &self.commit_id,
                self.path.as_deref(),
                self.ignore_whitespace,
            ),
        };
        match stdout {
            Ok(out) => {
                self.files = parse_unified_diff(&out);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        // diff 内容变化后，折叠/导航状态只保留仍然有效的坐标。
        let hunk_counts: Vec<usize> = self.files.iter().map(|f| f.hunks.len()).collect();
        let file_count = self.files.len();
        self.folded
            .retain(|(fi, hi)| *fi < file_count && *hi < hunk_counts[*fi]);
        if let Some((fi, hi)) = self.diff_nav
            && (fi >= file_count || hi >= hunk_counts[fi])
        {
            self.diff_nav = None;
        }
        cx.notify();
    }

    fn toggle_ignore_whitespace(&mut self, cx: &mut Context<Self>) {
        self.ignore_whitespace = !self.ignore_whitespace;
        self.reload(cx);
    }

    fn toggle_view_mode(&mut self, cx: &mut Context<Self>) {
        self.side_by_side = !self.side_by_side;
        cx.notify();
    }

    fn toggle_hunk(&mut self, file_index: usize, hunk_index: usize, cx: &mut Context<Self>) {
        let Some(file) = self.files.get(file_index).cloned() else {
            return;
        };
        let result = match self.source {
            DiffSource::Unstaged => self.repo.apply_hunk_to_index(&file, hunk_index),
            DiffSource::Staged => self.repo.revert_hunk_from_index(&file, hunk_index),
            DiffSource::Commit => return,
        };
        if let Err(e) = result {
            self.error = Some(e.to_string());
            cx.notify();
            return;
        }
        self.error = None;
        self.reload(cx);
    }

    /// 「左栏内容同步到右栏」箭头：仅 Unstaged——把工作区该 hunk 还原成
    /// index 版本（`git apply -R`，不动 index），随后刷新自身。
    fn sync_hunk_from_left(
        &mut self,
        file_index: usize,
        hunk_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.source != DiffSource::Unstaged {
            return;
        }
        let Some(file) = self.files.get(file_index).cloned() else {
            return;
        };
        if let Err(e) = self.repo.revert_hunk_in_worktree(&file, hunk_index) {
            self.error = Some(e.to_string());
            cx.notify();
            return;
        }
        self.error = None;
        self.reload(cx);
    }

    fn toggle_hunk_fold(&mut self, file_index: usize, hunk_index: usize, cx: &mut Context<Self>) {
        let key = (file_index, hunk_index);
        if !self.folded.remove(&key) {
            self.folded.insert(key);
        }
        cx.notify();
    }

    /// F7 / Shift+F7 或工具栏箭头：在 hunk 间循环导航，展开目标并滚动定位。
    fn step_hunk_nav(&mut self, forward: bool, cx: &mut Context<Self>) {
        let target = step_hunk(&self.files, self.diff_nav, forward);
        let Some(target) = target else {
            return;
        };
        self.diff_nav = Some(target);
        self.folded.remove(&target);
        scroll_hunk_into_view(
            &self.scroll,
            &self.files,
            target,
            self.side_by_side,
            &self.folded,
        );
        cx.notify();
    }
}

impl Render for DiffWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_MD))
            .flex_none()
            .px(px(theme::SPACE_MD))
            .py(px(theme::SPACE_SM))
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_body()))
                    .text_color(muted)
                    .child(self.title.clone()),
            );

        // 开关态由按钮的 selected 呈现，而不是在文案里拼 `✓` / `⇔` / `≡` 字形。
        header = header.child(
            Button::new("dw-ignore-ws")
                .ghost()
                .compact()
                .selected(self.ignore_whitespace)
                .label(tr("Ignore whitespace", "忽略空白"))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_ignore_whitespace(cx))),
        );

        header = header.child(
            Button::new("dw-view-mode")
                .ghost()
                .compact()
                .selected(self.side_by_side)
                .label(tr("Side-by-side", "并排对比"))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_view_mode(cx))),
        );

        // hunk 导航（与 F7 / Shift+F7 等价的工具栏入口）。
        header = header.child(
            Button::new("dw-prev-hunk")
                .ghost()
                .compact()
                .icon(Ic::ChevronUp)
                .tooltip(tr("Previous change (Shift+F7)", "上一处（Shift+F7）"))
                .on_click(cx.listener(|this, _, _, cx| this.step_hunk_nav(false, cx))),
        );
        header = header.child(
            Button::new("dw-next-hunk")
                .ghost()
                .compact()
                .icon(Ic::ChevronDown)
                .tooltip(tr("Next change (F7)", "下一处（F7）"))
                .on_click(cx.listener(|this, _, _, cx| this.step_hunk_nav(true, cx))),
        );

        // hunk 级暂存/取消暂存按钮（仅 staged/unstaged 来源展示）。
        let weak: gpui::WeakEntity<Self> = cx.entity().downgrade();
        let hunk_controls: Option<(&'static str, crate::ui::diff_view::HunkAction)> = match self
            .source
        {
            DiffSource::Staged | DiffSource::Unstaged => {
                let label = if self.source == DiffSource::Staged {
                    "Unstage"
                } else {
                    "Stage"
                };
                Some((
                    label,
                    std::sync::Arc::new(move |file_index, hunk_index, app: &mut gpui::App| {
                        let _ = weak
                            .update(app, |this, cx| this.toggle_hunk(file_index, hunk_index, cx));
                    }),
                ))
            }
            DiffSource::Commit => None,
        };

        // 「左栏内容同步到右栏」箭头：仅 Unstaged（右栏 = 工作区当前版本）。
        let sync_action: Option<crate::ui::diff_view::HunkAction> =
            if self.source == DiffSource::Unstaged {
                let weak: gpui::WeakEntity<Self> = cx.entity().downgrade();
                Some(std::sync::Arc::new(
                    move |file_index, hunk_index, app: &mut gpui::App| {
                        let _ = weak.update(app, |this, cx| {
                            this.sync_hunk_from_left(file_index, hunk_index, cx)
                        });
                    },
                ))
            } else {
                None
            };

        let body: gpui::Stateful<gpui::Div> = if let Some(error) = &self.error {
            error_state(error.clone(), None)
                .flex_1()
                .min_h_0()
                .id("dw-error")
        } else if self.files.is_empty() {
            empty_state(tr("No changes", "无更改"), muted)
                .flex_1()
                .min_h_0()
                .id("dw-empty")
        } else {
            let fold_action: HunkAction = {
                let weak: gpui::WeakEntity<Self> = cx.entity().downgrade();
                std::sync::Arc::new(move |file_index, hunk_index, app: &mut gpui::App| {
                    let _ = weak.update(app, |this, cx| {
                        this.toggle_hunk_fold(file_index, hunk_index, cx)
                    });
                })
            };
            div()
                .id("dw-content")
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .overflow_x_scroll()
                .track_scroll(&self.scroll)
                .child(crate::ui::diff_view::render_diff_files(
                    &self.files,
                    self.side_by_side,
                    hunk_controls
                        .as_ref()
                        .map(|(label, action)| (*label, action)),
                    sync_action.as_ref(),
                    &self.folded,
                    Some(&fold_action),
                    cx,
                ))
        };

        div().flex().flex_col().min_h_0().child(header).child(body)
    }
}

/// 在新窗口打开当前 diff。数据克隆进 'static 闭包，构建独立 `DiffWindowView`。
pub(crate) fn open_diff_window(spec: DiffWindowSpec, cx: &mut gpui::App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            size(px(theme::DIFF_WINDOW_WIDTH), px(theme::DIFF_WINDOW_HEIGHT)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(spec.title.clone().into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let _ = cx.open_window(options, |w, cx| {
        let view = cx.new(|cx| DiffWindowView::new(spec, w, cx));
        cx.new(|cx| Root::new(view, w, cx).bg(cx.theme().background))
    });
}
