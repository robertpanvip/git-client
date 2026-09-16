//! 全局设计 tokens：密度、间距、圆角与 Git 语义色。
//! 组件代码一律从此取值，避免散落的 magic number。

// tokens 为设计系统预留项，未即时引用属预期。
#![allow(dead_code)]

use gpui::{hsla, Hsla};
use rebased_rs::git::{ChangeStatus, MAX_COLORS};

// ---------- 密度（IntelliJ 风格紧凑行） ----------
pub const ROW_HEIGHT: f32 = 24.0;
pub const TOOLBAR_HEIGHT: f32 = 36.0;
pub const STATUSBAR_HEIGHT: f32 = 24.0;
pub const GROUP_HEADER_HEIGHT: f32 = 22.0;

// ---------- 图形泳道 ----------
pub const LANE_WIDTH: f32 = 14.0;
pub const DOT_RADIUS: f32 = 2.5;
pub const LINE_WIDTH: f32 = 1.5;

// ---------- 圆角 ----------
pub const RADIUS_SM: f32 = 2.0;
pub const RADIUS: f32 = 4.0;
pub const RADIUS_LG: f32 = 6.0;

// ---------- 间距（4px 网格） ----------
pub const SPACE_XS: f32 = 2.0;
pub const SPACE_SM: f32 = 4.0;
pub const SPACE_MD: f32 = 8.0;
pub const SPACE_LG: f32 = 12.0;
pub const SPACE_XL: f32 = 16.0;

// ---------- 前景派生色（跟随主题前景，亮暗自适应） ----------
pub fn transparent() -> Hsla {
    hsla(0.0, 0.0, 0.5, 0.0)
}

pub fn hover_bg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.06)
}

pub fn selected_bg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.12)
}

pub fn stripe_bg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.04)
}

pub fn badge_bg(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.15)
}

pub fn border_color(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.12)
}

pub fn overlay_bg() -> Hsla {
    hsla(0.0, 0.0, 0.0, 0.45)
}

// ---------- Git 语义色 ----------
const HUES: [f32; MAX_COLORS] = [0.58, 0.0, 0.33, 0.83, 0.13, 0.45, 0.65, 0.95];

pub fn lane_color(index: usize) -> Hsla {
    hsla(HUES[index % MAX_COLORS], 0.65, 0.55, 1.0)
}

pub fn status_color(status: &ChangeStatus) -> Hsla {
    match status {
        ChangeStatus::Added => added_color(),
        ChangeStatus::Modified => modified_color(),
        ChangeStatus::Deleted => deleted_color(),
        ChangeStatus::Renamed => renamed_color(),
        ChangeStatus::Copied => copied_color(),
        ChangeStatus::TypeChanged => type_changed_color(),
        ChangeStatus::Conflicted => conflicted_color(),
        ChangeStatus::Untracked => untracked_color(),
    }
}

pub fn added_color() -> Hsla {
    lane_color(2)
}

pub fn deleted_color() -> Hsla {
    lane_color(1)
}

pub fn modified_color() -> Hsla {
    hsla(0.11, 0.8, 0.55, 1.0)
}

pub fn renamed_color() -> Hsla {
    lane_color(4)
}

pub fn copied_color() -> Hsla {
    lane_color(5)
}

pub fn type_changed_color() -> Hsla {
    lane_color(7)
}

pub fn conflicted_color() -> Hsla {
    lane_color(3)
}

pub fn untracked_color() -> Hsla {
    lane_color(6)
}

pub fn binary_color() -> Hsla {
    hsla(0.58, 0.7, 0.55, 1.0)
}

pub fn error_color() -> Hsla {
    hsla(0.0, 0.75, 0.55, 1.0)
}

pub fn success_color() -> Hsla {
    hsla(0.31, 0.6, 0.5, 1.0)
}

pub fn head_color() -> Hsla {
    lane_color(0)
}

pub fn tag_color() -> Hsla {
    lane_color(2)
}

pub fn remote_color() -> Hsla {
    lane_color(5)
}

// ---------- Diff ----------
pub fn added_line_bg() -> Hsla {
    hsla(0.31, 0.6, 0.42, 0.14)
}

pub fn deleted_line_bg() -> Hsla {
    hsla(0.0, 0.65, 0.5, 0.13)
}

pub fn empty_half_bg() -> Hsla {
    hsla(0.0, 0.0, 0.5, 0.05)
}

pub fn hunk_bg() -> Hsla {
    hsla(0.58, 0.7, 0.55, 0.1)
}
