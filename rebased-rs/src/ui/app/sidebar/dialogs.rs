use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, Window, div, px,
};
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Textarea,
};

use rebased_rs::git::{MergeMode, ResetMode};

use crate::ui::app::{AppView, PromptKind};
use crate::ui::components::{cancel_button, dialog_field, dialog_footer, dialog_shell};
use crate::ui::i18n::tr;
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
            PromptKind::FilterAuthor => (
                tr("Filter by author", "按作者过滤").to_string(),
                tr(
                    "Show only commits whose author matches this text. Leave empty to clear the filter.",
                    "仅显示作者匹配的提交。留空以清除过滤。",
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
            PromptKind::Confirm(action) => (action.title(), action.hint()),
        };

        // 每个分支产出「正文 + 底部动作区」两部分：
        // 动作区交给 `dialog_footer`，保证全应用的按钮顺序/间距一致，
        // 正文单独滚动（超长内容不再把对话框撑出屏幕）。
        // 必填类对话框在输入为空时禁用主按钮（IntelliJ 默认按钮语义）；
        // Stash / Squash / MergeMessage / FilterAuthor 的输入可选，不禁用。
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
                        PromptKind::FilterAuthor => tr("Filter", "过滤"),
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
