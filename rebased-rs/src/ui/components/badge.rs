//! 徽章 / 标签原语。
//!
//! 两类：
//! - [`badge`]：低饱和底（15% alpha），用于状态、计数等**内联标记**。
//! - [`chip`]：实色胶囊，用于 Log 行内 **ref 标签**（分支 / tag），贴近 IntelliJ 观感。

use gpui::{Div, Hsla, InteractiveElement, ParentElement, SharedString, Stateful, Styled, div, px};

use crate::ui::theme;

/// git ref 名称 → (显示文本, 语义色)。
///
/// 无法得知 remote 列表时的**启发式**判断：含 `/` 视为远程分支。
/// 需要精确区分时应使用 [`ref_style_with_remotes`]。
pub fn ref_style(name: &str) -> (String, Hsla) {
    ref_style_inner(name, None)
}

/// 精确版：传入仓库的 remote 名列表，可正确区分「本地分支名含 `/`」与「远程分支」。
pub fn ref_style_with_remotes(name: &str, remotes: &[String]) -> (String, Hsla) {
    ref_style_inner(name, Some(remotes))
}

fn ref_style_inner(name: &str, remotes: Option<&[String]>) -> (String, Hsla) {
    if let Some(tag) = name.strip_prefix("tag: ") {
        return (tag.to_string(), theme::tag_color());
    }
    if let Some(branch) = name.strip_prefix("HEAD -> ") {
        return (branch.to_string(), theme::head_color());
    }
    if name == "HEAD" {
        return ("HEAD".to_string(), theme::head_color());
    }
    let is_remote = match remotes {
        Some(remotes) => remotes
            .iter()
            .any(|remote| name.starts_with(&format!("{remote}/"))),
        // 无 remote 信息时退化为"含 `/` 即远程"。
        None => name.contains('/'),
    };
    let color = if is_remote {
        theme::branch_remote_color()
    } else {
        theme::branch_local_color()
    };
    (name.to_string(), color)
}

/// 低饱和底内联徽章（状态 / 计数）。
pub fn badge(id: SharedString, label: impl Into<SharedString>, color: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(theme::SPACE_SM))
        .rounded(px(theme::RADIUS))
        .bg(theme::badge_bg(color))
        .text_size(px(theme::font_size_meta()))
        .text_color(color)
        .child(label.into())
}

/// 实色胶囊 ref 标签（分支 / tag / HEAD）。
///
/// 底色为 ref 语义色的实色淡化，文字用主文字色——与原版 Log 中的 ref 标签一致
/// （原实现用语义色当文字色，对比度不足且与行内文字抢层级）。
pub fn chip(id: SharedString, label: impl Into<SharedString>, color: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(theme::BADGE_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .px(px(theme::SPACE_XS))
        .rounded(px(theme::RADIUS_SM))
        .bg(theme::badge_solid_bg(color))
        .text_size(px(theme::font_size_meta()))
        .text_color(theme::text_primary())
        .overflow_hidden()
        .whitespace_nowrap()
        .child(label.into())
}
