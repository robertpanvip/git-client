use gpui::{
    AnyElement, Context, Div, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, Window, WindowAppearance, div, px,
};
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Textarea,
    theme::{Theme, ThemeMode},
};

use rebased_rs::git::{MergeMode, ResetMode};

use crate::ui::app::{AppView, CommitChecks, PromptKind};
use crate::ui::components::{cancel_button, dialog_field, dialog_footer, dialog_shell};
use crate::ui::i18n::{self, Language, tr};
use crate::ui::icons::Ic;
use crate::ui::settings;
use crate::ui::theme;

impl AppView {
    pub(crate) fn render_prompt_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let kind = self.state.prompt.clone()?;
        // 对话框打开后的首帧聚焦输入框（IntelliJ 行为）；标志随开框置位、
        // 此处一次性消费，避免每帧抢走用户在对话框其它控件上的焦点。
        if self.state.prompt_focus_pending {
            self.state.prompt_focus_pending = false;
            self.prompt_input
                .update(cx, |state, cx| state.focus(window, cx));
        }
        // 提交设置对话框首帧回显「作者(A)」输入框（开框同步一次，避免每帧重置光标）。
        if self.state.prompt_author_sync_pending {
            self.state.prompt_author_sync_pending = false;
            let author = self.state.commit_author.clone();
            self.commit_author_input.update(cx, |state, cx| {
                if state.value() != author {
                    state.set_value(&author, window, cx);
                }
            });
        }
        let muted = cx.theme().muted_foreground;
        let (title, hint): (String, String) = match &kind {
            PromptKind::NewBranch { start_point } => (
                tr("New branch", "新建分支").to_string(),
                match start_point {
                    Some(point) => format!(
                        "{} {}",
                        tr("From commit", "从提交"),
                        &point[..point.len().min(7)]
                    ),
                    None => tr("From current HEAD", "从当前 HEAD").to_string(),
                },
            ),
            PromptKind::NewTag { commit_id } => (
                tr("New tag", "新建标签").to_string(),
                format!(
                    "{} {}",
                    tr("On commit", "在提交"),
                    &commit_id[..commit_id.len().min(7)]
                ),
            ),
            PromptKind::EditTag { name, commit_id } => (
                tr("Edit tag message", "编辑标签信息").to_string(),
                format!(
                    "{} {name} @ {} — {}",
                    tr("Rebuild", "重建"),
                    &commit_id[..commit_id.len().min(7)],
                    tr("with a new annotated message", "使用新的附注信息")
                ),
            ),
            PromptKind::Stash => (
                tr("Stash changes", "贮藏更改").to_string(),
                tr(
                    "Optional message; choose whether to include untracked files and keep the index.",
                    "可选信息；可选择是否包含未跟踪文件以及保留暂存区。",
                )
                .to_string(),
            ),
            PromptKind::Reword { commit_id } => (
                tr("Reword commit", "改写提交").to_string(),
                format!(
                    "{} {}",
                    tr("New message for", "新的提交信息："),
                    &commit_id[..commit_id.len().min(7)]
                ),
            ),
            PromptKind::Squash { commit_id } => (
                tr("Squash commit", "压缩到父提交").to_string(),
                format!(
                    "{} {} — {}",
                    tr("Squash", "压缩"),
                    &commit_id[..commit_id.len().min(7)],
                    tr(
                        "optionally set a combined message; leave empty to keep git's default.",
                        "可选设置合并后的信息；留空则使用 git 默认信息。",
                    ),
                ),
            ),
            PromptKind::RenameBranch => (
                tr("Rename branch", "重命名分支").to_string(),
                match &self.state.current_branch {
                    Some(name) => format!(
                        "{} {name} →",
                        tr("Rename current branch", "重命名当前分支")
                    ),
                    None => tr("No current branch", "没有当前分支").to_string(),
                },
            ),
            PromptKind::RenameBranchByName { name } => (
                tr("Rename branch", "重命名分支").to_string(),
                format!("{} {name} →", tr("Rename", "重命名")),
            ),
            PromptKind::MergeMessage { name } => (
                tr("Merge message", "合并信息").to_string(),
                format!(
                    "{} {name} (no ff):",
                    tr("Merge commit message for merging", "合并提交信息（非快进）")
                ),
            ),
            PromptKind::RebaseEdit { .. } => (
                tr("Edit commit message", "编辑提交信息").to_string(),
                tr(
                    "Set the message used when this commit is reworded (or merged by squash).",
                    "设置此提交改写（或被 squash 合并）时使用的信息。",
                )
                .to_string(),
            ),
            PromptKind::Reset { commit_id } => (
                tr("Reset current branch to here", "重置当前分支到此处").to_string(),
                format!(
                    "{} {}. {}",
                    tr("Move the current branch to", "将当前分支移动到"),
                    &commit_id[..commit_id.len().min(7)],
                    tr(
                        "Pick a mode: Soft keeps everything staged, Mixed keeps changes unstaged, Hard discards all changes.",
                        "选择模式：Soft 保留全部更改并暂存，Mixed 保留更改但取消暂存，Hard 丢弃所有更改。",
                    ),
                ),
            ),
            PromptKind::GoTo => (
                tr("Go to commit", "跳转到提交").to_string(),
                tr(
                    "Enter a hash, branch or tag name to select it in the log.",
                    "输入哈希、分支或标签名以在日志中定位。",
                )
                .to_string(),
            ),
            PromptKind::AddRemote => (
                tr("Add remote", "添加远程仓库").to_string(),
                tr(
                    "Enter the remote name (e.g. origin) and its URL.",
                    "输入远程名称（如 origin）及其 URL。",
                )
                .to_string(),
            ),
            PromptKind::SetUpstream { branch } => (
                tr("Set upstream", "设置上游").to_string(),
                format!(
                    "{} {branch} (e.g. origin/main):",
                    tr("Upstream of", "上游分支：")
                ),
            ),
            PromptKind::Settings => (
                tr("Settings", "设置").to_string(),
                tr(
                    "Changes apply immediately and are saved automatically.",
                    "更改即时生效并自动保存。",
                )
                .to_string(),
            ),
            PromptKind::CommitSettings => (
                tr("Commit Settings", "提交设置").to_string(),
                tr(
                    "Options apply to the next Commit / Commit and Push.",
                    "选项对下一次「提交 / 提交并推送」生效。",
                )
                .to_string(),
            ),
            PromptKind::Confirm(action) => (action.title(), action.hint()),
        };

        // 每个分支产出「正文 + 底部动作区」两部分：
        // 动作区交给 `dialog_footer`，保证全应用的按钮顺序/间距一致，
        // 正文单独滚动（超长内容不再把对话框撑出屏幕）。
        // 必填类对话框在输入为空时禁用主按钮（IntelliJ 默认按钮语义）；
        // Stash / Squash / MergeMessage 的输入可选，不禁用。
        let input_empty = self.prompt_input.read(cx).value().trim().is_empty();
        let input2_empty = self.prompt_input2.read(cx).value().trim().is_empty();
        let requires_input = matches!(
            kind,
            PromptKind::NewBranch { .. }
                | PromptKind::NewTag { .. }
                | PromptKind::EditTag { .. }
                | PromptKind::Reword { .. }
                | PromptKind::RenameBranch
                | PromptKind::RenameBranchByName { .. }
                | PromptKind::GoTo
                | PromptKind::SetUpstream { .. }
        );
        let (body, footer): (Div, AnyElement) =
            match &kind {
                PromptKind::Reset { commit_id } => {
                    let target = commit_id.clone();
                    let body = div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_MD))
                        .child(
                            Button::new("reset-soft")
                                .primary()
                                .label(tr(
                                    "Soft — keep all changes staged",
                                    "Soft — 保留全部更改并暂存",
                                ))
                                .on_click(cx.listener({
                                    let target = target.clone();
                                    move |this, _, _, cx| {
                                        this.reset_branch_to(target.clone(), ResetMode::Soft, cx)
                                    }
                                })),
                        )
                        .child(
                            Button::new("reset-mixed")
                                .label(tr(
                                    "Mixed — keep changes unstaged",
                                    "Mixed — 保留更改但不暂存",
                                ))
                                .on_click(cx.listener({
                                    let target = target.clone();
                                    move |this, _, _, cx| {
                                        this.reset_branch_to(target.clone(), ResetMode::Mixed, cx)
                                    }
                                })),
                        )
                        .child(
                            Button::new("reset-hard")
                                .danger()
                                .label(tr("Hard — discard all changes", "Hard — 丢弃所有更改"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.reset_branch_to(target.clone(), ResetMode::Hard, cx)
                                })),
                        );
                    // 重置的三种模式本身就是主操作，无需额外确认按钮。
                    (body, div().into_any_element())
                }
                PromptKind::Stash => (
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_LG))
                        .child(Textarea::new(&self.prompt_input).h(px(theme::INPUT_HEIGHT_MULTI)))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(theme::SPACE_MD))
                                .child(
                                    Checkbox::new("stash-keep-index")
                                        .checked(self.state.prompt_stash_keep_index)
                                        .label(tr("Keep staged changes", "保留暂存区更改"))
                                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                            this.state.prompt_stash_keep_index = *checked;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Checkbox::new("stash-untracked")
                                        .checked(self.state.prompt_stash_include_untracked)
                                        .label(tr("Include untracked files", "包含未跟踪文件"))
                                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                            this.state.prompt_stash_include_untracked = *checked;
                                            cx.notify();
                                        })),
                                ),
                        ),
                    dialog_footer(
                        cancel_button("prompt-cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx)))
                            .into_any_element(),
                        Button::new("prompt-ok")
                            .primary()
                            .label(tr("Stash", "贮藏"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_prompt(window, cx)),
                            ),
                    )
                    .into_any_element(),
                ),
                PromptKind::MergeMessage { .. } => {
                    let current_mode = self.state.prompt_merge_mode;
                    fn mode_button(
                        cx: &mut Context<crate::ui::app::AppView>,
                        id: String,
                        label: SharedString,
                        active: bool,
                        mode: MergeMode,
                    ) -> Button {
                        let color = if active { "primary" } else { "ghost" };
                        let mut btn = Button::new(id).label(label);
                        if color == "primary" {
                            btn = btn.primary();
                        } else {
                            btn = btn.ghost();
                        }
                        btn.on_click(cx.listener(move |this, _, _, cx| {
                            this.state.prompt_merge_mode = mode;
                            cx.notify();
                        }))
                    }
                    let body = div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_LG))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(theme::SPACE_MD))
                                .child(mode_button(
                                    cx,
                                    "merge-mode-default".to_string(),
                                    tr("Default", "默认").into(),
                                    current_mode == MergeMode::Default,
                                    MergeMode::Default,
                                ))
                                .child(mode_button(
                                    cx,
                                    "merge-mode-noff".to_string(),
                                    tr("No FF", "非快进").into(),
                                    current_mode == MergeMode::NoFastForward,
                                    MergeMode::NoFastForward,
                                ))
                                .child(mode_button(
                                    cx,
                                    "merge-mode-ffonly".to_string(),
                                    tr("FF only", "仅快进").into(),
                                    current_mode == MergeMode::FastForwardOnly,
                                    MergeMode::FastForwardOnly,
                                )),
                        )
                        .child(Textarea::new(&self.prompt_input).h(px(theme::INPUT_HEIGHT_MULTI)));
                    let footer = dialog_footer(
                        cancel_button("prompt-cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx)))
                            .into_any_element(),
                        Button::new("prompt-ok")
                            .primary()
                            .label(tr("Merge", "合并"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_prompt(window, cx)),
                            ),
                    )
                    .into_any_element();
                    (body, footer)
                }
                PromptKind::Confirm(action) => {
                    let action = action.clone();
                    let label = action.confirm_label();
                    (
                        div(),
                        dialog_footer(
                            cancel_button("prompt-cancel")
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx)))
                                .into_any_element(),
                            Button::new("prompt-confirm")
                                .danger()
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.confirm_action(action.clone(), cx)
                                })),
                        )
                        .into_any_element(),
                    )
                }
                PromptKind::AddRemote => (
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_MD))
                        .child(dialog_field(
                            tr("Remote name", "远程名称"),
                            Textarea::new(&self.prompt_input).h(px(theme::INPUT_HEIGHT_SINGLE)),
                            muted,
                        ))
                        .child(dialog_field(
                            tr("Remote URL", "远程 URL"),
                            Textarea::new(&self.prompt_input2).h(px(theme::INPUT_HEIGHT_SINGLE)),
                            muted,
                        )),
                    dialog_footer(
                        cancel_button("prompt-cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx)))
                            .into_any_element(),
                        Button::new("prompt-ok")
                            .primary()
                            .label(tr("Add", "添加"))
                            // 远程名称与 URL 均必填。
                            .disabled(input_empty || input2_empty)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_prompt(window, cx)),
                            ),
                    )
                    .into_any_element(),
                ),
                PromptKind::Settings => {
                    let is_dark = cx.theme().is_dark();
                    let is_zh = i18n::current() == Language::Zh;
                    let delta = theme::ui_font_delta();
                    let editor_size = theme::editor_font_size();
                    let body = div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_LG))
                        .child(settings_section_label(tr("Appearance", "外观")))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(theme::SPACE_MD))
                                .child(settings_option_button(
                                    cx,
                                    "settings-theme-light",
                                    tr("Light", "浅色").into(),
                                    !is_dark,
                                    |_, window, cx| {
                                        Theme::change(ThemeMode::Light, Some(window), cx);
                                        cx.set_window_appearance(Some(WindowAppearance::Light));
                                        settings::persist_theme_mode(ThemeMode::Light);
                                        cx.refresh_windows();
                                    },
                                ))
                                .child(settings_option_button(
                                    cx,
                                    "settings-theme-dark",
                                    tr("Dark", "深色").into(),
                                    is_dark,
                                    |_, window, cx| {
                                        Theme::change(ThemeMode::Dark, Some(window), cx);
                                        theme::apply_jetbrains_palette(cx);
                                        cx.set_window_appearance(Some(WindowAppearance::Dark));
                                        settings::persist_theme_mode(ThemeMode::Dark);
                                        cx.refresh_windows();
                                    },
                                )),
                        )
                        .child(settings_section_label(tr("Language", "语言")))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(theme::SPACE_MD))
                                .child(settings_option_button(
                                    cx,
                                    "settings-lang-en",
                                    "English".into(),
                                    !is_zh,
                                    |_, _, cx| {
                                        if i18n::current() != Language::En {
                                            i18n::set_current(Language::En);
                                            cx.refresh_windows();
                                        }
                                    },
                                ))
                                .child(settings_option_button(
                                    cx,
                                    "settings-lang-zh",
                                    "中文".into(),
                                    is_zh,
                                    |_, _, cx| {
                                        if i18n::current() != Language::Zh {
                                            i18n::set_current(Language::Zh);
                                            cx.refresh_windows();
                                        }
                                    },
                                )),
                        )
                        .child(dialog_field(
                            tr("Window font size", "窗口字号"),
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(theme::SPACE_SM))
                                .child(settings_font_button(
                                    cx,
                                    "settings-ui-minus",
                                    "−",
                                    FontTarget::Ui,
                                    SettingsStep::Decrease,
                                    delta <= theme::UI_FONT_DELTA_MIN,
                                ))
                                .child(settings_value_text(format!("{delta:+} px")))
                                .child(settings_font_button(
                                    cx,
                                    "settings-ui-plus",
                                    "+",
                                    FontTarget::Ui,
                                    SettingsStep::Increase,
                                    delta >= theme::UI_FONT_DELTA_MAX,
                                )),
                            muted,
                        ))
                        .child(dialog_field(
                            tr("Editor font size", "编辑器字号"),
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(theme::SPACE_SM))
                                .child(settings_font_button(
                                    cx,
                                    "settings-editor-minus",
                                    "−",
                                    FontTarget::Editor,
                                    SettingsStep::Decrease,
                                    editor_size <= theme::EDITOR_FONT_SIZE_MIN,
                                ))
                                .child(settings_value_text(format!("{editor_size:.1} px")))
                                .child(settings_font_button(
                                    cx,
                                    "settings-editor-plus",
                                    "+",
                                    FontTarget::Editor,
                                    SettingsStep::Increase,
                                    editor_size >= theme::EDITOR_FONT_SIZE_MAX,
                                )),
                            muted,
                        ));
                    // 设置项即时生效，底部只需一个「完成」。
                    let footer = dialog_footer(
                        div().into_any_element(),
                        Button::new("settings-close")
                            .primary()
                            .label(tr("Done", "完成"))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx))),
                    )
                    .into_any_element();
                    (body, footer)
                }
                PromptKind::CommitSettings => {
                    // 提交设置（提交区右侧齿轮）：按 IDEA Settings→Git 的
                    // 「Git / 提交检查 / 高级 提交检查 / 在提交之后」分组还原。
                    // 作者与 Sign-off 直接作用于下一次提交参数；检查组作为
                    // 持久化偏好呈现（IDE 功能在独立客户端中不适用）。
                    let border = cx.theme().border;
                    let link = cx.theme().link;
                    let checks = self.state.commit_checks;
                    let body = div()
                        .flex()
                        .flex_col()
                        .gap(px(theme::SPACE_LG))
                        // ── Git ─────────────────────────────
                        .child(commit_section_header(tr("Git", "Git"), border))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(theme::SPACE_MD))
                                .pl(px(theme::SPACE_LG))
                                .child(
                                    div()
                                        .flex_none()
                                        .text_size(px(theme::font_size_body()))
                                        .child(tr("Author(A):", "作者(A):")),
                                )
                                .child(
                                    Textarea::new(&self.commit_author_input)
                                        .h(px(theme::INPUT_HEIGHT_SINGLE))
                                        .flex_1(),
                                ),
                        )
                        .child(commit_check_row(
                            "commit-settings-signoff",
                            self.state.commit_signoff,
                            tr("Sign-off commit(G)", "Sign-off 提交(G)"),
                            None,
                            cx.listener(|this, checked: &bool, _, cx| {
                                this.state.commit_signoff = *checked;
                                crate::ui::settings::persist_commit_signoff(*checked);
                                cx.notify();
                            }),
                        ))
                        .child(commit_check_row(
                            "commit-settings-amend",
                            self.state.amend,
                            tr("Amend previous commit", "修正上一次提交"),
                            None,
                            cx.listener(|this, checked: &bool, _, cx| {
                                this.state.amend = *checked;
                                cx.notify();
                            }),
                        ))
                        .child(commit_check_row(
                            "commit-settings-allow-empty",
                            self.state.allow_empty_commit_message,
                            tr("Allow empty commit message", "允许空提交信息"),
                            None,
                            cx.listener(|this, checked: &bool, _, cx| {
                                this.state.allow_empty_commit_message = *checked;
                                cx.notify();
                            }),
                        ))
                        // ── 提交检查 ────────────────────────
                        .child(commit_section_header(
                            tr("Before Commit", "提交检查"),
                            border,
                        ))
                        .children(commit_check_group(
                            cx,
                            &checks,
                            link,
                            &[
                                (
                                    "commit-check-copyright",
                                    tr("Update copyright", "更新版权"),
                                    None,
                                    CheckField::UpdateCopyright,
                                ),
                                (
                                    "commit-check-reformat",
                                    tr("Reformat code(R)", "重新设置代码格式(R)"),
                                    None,
                                    CheckField::ReformatCode,
                                ),
                                (
                                    "commit-check-rearrange",
                                    tr("Rearrange code(N)", "重新整理代码(N)"),
                                    None,
                                    CheckField::RearrangeCode,
                                ),
                                (
                                    "commit-check-imports",
                                    tr("Optimize imports(O)", "优化 import(O)"),
                                    None,
                                    CheckField::OptimizeImports,
                                ),
                                (
                                    "commit-check-cleanup",
                                    tr("Cleanup(L)", "清理(L)"),
                                    Some(tr("Choose profile", "选择配置文件")),
                                    CheckField::Cleanup,
                                ),
                                (
                                    "commit-check-dependencies",
                                    tr("Check for vulnerabilities", "检查恶意依赖项"),
                                    None,
                                    CheckField::CheckDependencies,
                                ),
                            ],
                        ))
                        // ── 高级 提交检查 ───────────────────
                        .child(commit_section_header(
                            tr("Advanced Commit Checks", "高级 提交检查"),
                            border,
                        ))
                        .children(commit_check_group(
                            cx,
                            &checks,
                            link,
                            &[
                                (
                                    "commit-check-run-config",
                                    tr("Run configuration", "运行配置"),
                                    Some(tr("Choose configuration", "选择配置")),
                                    CheckField::RunConfiguration,
                                ),
                                (
                                    "commit-check-analyze",
                                    tr("Analyze code(A)", "分析代码(A)"),
                                    Some(tr("Choose profile", "选择配置文件")),
                                    CheckField::AnalyzeCode,
                                ),
                                (
                                    "commit-check-todo",
                                    tr("Check TODO", "检查 TODO"),
                                    Some(tr("Configure", "配置")),
                                    CheckField::CheckTodo,
                                ),
                            ],
                        ))
                        .child(commit_check_row(
                            "commit-check-advanced",
                            checks.run_advanced_after_commit,
                            tr("Run advanced checks after commit", "提交完成后运行高级检查"),
                            None,
                            cx.listener(|this, checked: &bool, _, cx| {
                                this.state.commit_checks.run_advanced_after_commit = *checked;
                                crate::ui::settings::persist_commit_checks(
                                    &this.state.commit_checks,
                                );
                                cx.notify();
                            }),
                        ))
                        .child(
                            div()
                                .pl(px(theme::SPACE_LG * 2.0 + theme::SPACE_MD))
                                .text_size(px(theme::font_size_meta()))
                                .text_color(muted)
                                .child(tr(
                                    "Failed checks will not prevent the commit",
                                    "检查失败不会阻止提交",
                                )),
                        )
                        // ── 在提交之后 ──────────────────────
                        .child(commit_section_header(
                            tr("After Commit", "在提交之后"),
                            border,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(theme::SPACE_MD))
                                .pl(px(theme::SPACE_LG))
                                .child(
                                    div()
                                        .flex_none()
                                        .text_size(px(theme::font_size_body()))
                                        .child(tr("Upload files to:", "将文件上传到:")),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(180.0))
                                        .h(px(theme::INPUT_HEIGHT_SINGLE))
                                        .rounded(px(theme::RADIUS_SM))
                                        .border_1()
                                        .border_color(theme::input_border())
                                        .bg(theme::input_bg())
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(theme::SPACE_SM))
                                        .pl(px(theme::SPACE_MD))
                                        .pr(px(theme::SPACE_MD))
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(px(theme::font_size_body()))
                                                .child("<无>"),
                                        )
                                        .child(
                                            Icon::new(Ic::ChevronDown)
                                                .with_size(Size::XSmall)
                                                .text_color(muted),
                                        ),
                                )
                                .child(
                                    Button::new("commit-settings-upload-more")
                                        .ghost()
                                        .compact()
                                        .label("…"),
                                ),
                        )
                        .child(commit_check_row(
                            "commit-check-server",
                            checks.always_use_server,
                            tr(
                                "Always use selected server or server group",
                                "始终使用选定服务器或服务器组",
                            ),
                            None,
                            cx.listener(|this, checked: &bool, _, cx| {
                                this.state.commit_checks.always_use_server = *checked;
                                crate::ui::settings::persist_commit_checks(
                                    &this.state.commit_checks,
                                );
                                cx.notify();
                            }),
                        ));
                    let footer = dialog_footer(
                        div().into_any_element(),
                        Button::new("commit-settings-done")
                            .primary()
                            .label(tr("Done", "完成"))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx))),
                    )
                    .into_any_element();
                    (body, footer)
                }
                _ => {
                    let ok_label = match &kind {
                        PromptKind::NewBranch { .. } => tr("Create", "创建"),
                        PromptKind::NewTag { .. } => tr("Tag", "打标签"),
                        PromptKind::EditTag { .. } => tr("Save", "保存"),
                        PromptKind::Stash => tr("Stash", "贮藏"),
                        PromptKind::Reword { .. } | PromptKind::RebaseEdit { .. } => {
                            tr("Reword", "改写")
                        }
                        PromptKind::Squash { .. } => tr("Squash", "压缩"),
                        PromptKind::RenameBranch => tr("Rename", "重命名"),
                        PromptKind::GoTo => tr("Go", "跳转"),
                        PromptKind::SetUpstream { .. } => tr("Set", "设置"),
                        _ => tr("OK", "确定"),
                    };
                    (
                        div().flex().flex_col().gap(px(theme::SPACE_LG)).child(
                            Textarea::new(&self.prompt_input).h(px(theme::INPUT_HEIGHT_MULTI)),
                        ),
                        dialog_footer(
                            cancel_button("prompt-cancel")
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt(cx)))
                                .into_any_element(),
                            Button::new("prompt-ok")
                                .primary()
                                .label(ok_label)
                                .disabled(requires_input && input_empty)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_prompt(window, cx)
                                })),
                        )
                        .into_any_element(),
                    )
                }
            };

        Some(
            dialog_shell(title, Some(hint.into()), body, footer)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .into_any_element(),
        )
    }
}

/// 设置面板分组小标题。
fn settings_section_label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(theme::font_size_meta()))
        .font_weight(theme::WEIGHT_MEDIUM)
        .child(text.into())
}

/// IDEA 设置页分组头：左侧分组名 + 右侧延伸到底的细分隔线（截图样式）。
fn commit_section_header(text: impl Into<SharedString>, border: Hsla) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_MD))
        .child(
            div()
                .flex_none()
                .text_size(px(theme::font_size_body()))
                .font_weight(theme::WEIGHT_MEDIUM)
                .child(text.into()),
        )
        .child(div().flex_1().h(px(1.0)).bg(border))
}

/// 提交设置里的单个复选框行（缩进一级，对齐 IDEA 设置页层级）。
///
/// `on_click` 直接接收 `cx.listener(...)` 的产物（`Fn(&bool, &mut Window, &mut App)`）；
/// `link` 为 IDEA 里复选框右侧的蓝色配置链接（如「选择配置文件」），纯展示。
fn commit_check_row(
    id: &'static str,
    checked: bool,
    label: impl Into<SharedString>,
    link: Option<(&'static str, Hsla)>,
    on_click: impl Fn(&bool, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(theme::SPACE_LG))
        .pl(px(theme::SPACE_LG))
        .child(
            Checkbox::new(id)
                .checked(checked)
                .label(label.into())
                .on_click(on_click),
        )
        .children(link.map(|(text, color)| commit_settings_link(text, color)))
        .into_any_element()
}

/// 「提交检查」组内的蓝色链接文字（IDEA 中指向对应配置页；纯展示）。
fn commit_settings_link(text: &'static str, link: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(px(theme::font_size_body()))
        .text_color(link)
        .child(text)
}

/// 提交检查开关字段（用于批量构造复选框行时定位到具体字段）。
#[derive(Clone, Copy)]
enum CheckField {
    UpdateCopyright,
    ReformatCode,
    RearrangeCode,
    OptimizeImports,
    Cleanup,
    CheckDependencies,
    RunConfiguration,
    AnalyzeCode,
    CheckTodo,
}

impl CheckField {
    fn get(self, checks: &CommitChecks) -> bool {
        match self {
            Self::UpdateCopyright => checks.update_copyright,
            Self::ReformatCode => checks.reformat_code,
            Self::RearrangeCode => checks.rearrange_code,
            Self::OptimizeImports => checks.optimize_imports,
            Self::Cleanup => checks.cleanup,
            Self::CheckDependencies => checks.check_dependencies,
            Self::RunConfiguration => checks.run_configuration,
            Self::AnalyzeCode => checks.analyze_code,
            Self::CheckTodo => checks.check_todo,
        }
    }

    fn set(self, checks: &mut CommitChecks, value: bool) {
        match self {
            Self::UpdateCopyright => checks.update_copyright = value,
            Self::ReformatCode => checks.reformat_code = value,
            Self::RearrangeCode => checks.rearrange_code = value,
            Self::OptimizeImports => checks.optimize_imports = value,
            Self::Cleanup => checks.cleanup = value,
            Self::CheckDependencies => checks.check_dependencies = value,
            Self::RunConfiguration => checks.run_configuration = value,
            Self::AnalyzeCode => checks.analyze_code = value,
            Self::CheckTodo => checks.check_todo = value,
        }
    }
}

/// 批量构造「提交检查」组的复选框行；点击即写回 state 并持久化。
/// `items` 的第三项为复选框右侧的蓝色链接文字（无则 None）。
fn commit_check_group(
    cx: &mut Context<AppView>,
    checks: &CommitChecks,
    link: Hsla,
    items: &[(&'static str, &'static str, Option<&'static str>, CheckField)],
) -> Vec<AnyElement> {
    items
        .iter()
        .map(|(id, label, link_text, field)| {
            let field = *field;
            commit_check_row(
                id,
                field.get(checks),
                *label,
                link_text.map(|text| (text, link)),
                cx.listener(move |this, checked: &bool, _, cx| {
                    field.set(&mut this.state.commit_checks, *checked);
                    crate::ui::settings::persist_commit_checks(&this.state.commit_checks);
                    cx.notify();
                }),
            )
        })
        .collect()
}

/// 设置面板互斥选项按钮（当前项 primary 高亮，其余 ghost）。
fn settings_option_button(
    cx: &mut Context<AppView>,
    id: &'static str,
    label: SharedString,
    active: bool,
    on_click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Button {
    let mut btn = Button::new(id).label(label);
    btn = if active { btn.primary() } else { btn.ghost() };
    btn.on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
}

/// 字号步进方向。
#[derive(Clone, Copy)]
enum SettingsStep {
    Decrease,
    Increase,
}

impl SettingsStep {
    fn delta(self) -> i32 {
        match self {
            Self::Decrease => -1,
            Self::Increase => 1,
        }
    }
}

/// 字号调节目标：窗口字号（1px 步进）/ 编辑器字号（0.5px 步进）。
#[derive(Clone, Copy)]
enum FontTarget {
    Ui,
    Editor,
}

/// 设置面板的字号步进按钮：点击立即生效并持久化，全局刷新窗口。
fn settings_font_button(
    cx: &mut Context<AppView>,
    id: &'static str,
    label: &'static str,
    target: FontTarget,
    step: SettingsStep,
    disabled: bool,
) -> Button {
    let mut btn = Button::new(id).ghost().compact().label(label);
    if disabled {
        btn = btn.disabled(true);
    }
    btn.on_click(cx.listener(move |_, _, _, cx| {
        match target {
            FontTarget::Ui => {
                theme::set_ui_font_delta(theme::ui_font_delta() + step.delta());
                settings::persist_ui_font_delta(theme::ui_font_delta());
            }
            FontTarget::Editor => {
                let size = theme::editor_font_size() + step.delta() as f32 * 0.5;
                theme::set_editor_font_size(size);
                settings::persist_editor_font_size(theme::editor_font_size());
            }
        }
        cx.refresh_windows();
    }))
}

/// 步进按钮之间的当前值显示（固定最小宽度，避免加减时按钮跳动）。
fn settings_value_text(text: String) -> Div {
    div()
        .min_w(px(64.0))
        .flex()
        .justify_center()
        .text_size(px(theme::font_size_body()))
        .child(text)
}
