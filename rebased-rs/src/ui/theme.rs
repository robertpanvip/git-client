//! 全局设计 tokens：密度、间距、圆角与 Git 语义色。
//! 组件代码一律从此取值，避免散落的 magic number。

// tokens 为设计系统预留项，未即时引用属预期。
#![allow(dead_code)]

use gpui::{App, Hsla, hsla};
use gpui_kit::component::theme::Theme;
use rebased_rs::git::{ChangeStatus, MAX_COLORS};

// ---------- 密度（对齐原版实测：工具栏 44 / 列表行 26 / 状态栏 31） ----------
pub const ROW_HEIGHT: f32 = 26.0;
pub const TOOLBAR_HEIGHT: f32 = 44.0;
pub const STATUSBAR_HEIGHT: f32 = 31.0;
pub const GROUP_HEADER_HEIGHT: f32 = 22.0;

// ---------- 三栏布局（对齐原版实测：左面板 344 / 图标条 40 / 右面板 389） ----------
/// 左侧 Commit 面板宽度：变更列表 + 提交输入区。
pub const COMMIT_PANEL_WIDTH: f32 = 344.0;
/// 左侧最左图标条宽度。
pub const ICON_STRIP_WIDTH: f32 = 40.0;
/// Log 主区标题行高度（“Log: <分支>” 蓝色标签行，实测 41px）。
pub const LOG_HEADER_HEIGHT: f32 = 41.0;
/// Log 主区过滤行高度（搜索框 + 过滤 chips，实测 37px）。
pub const LOG_FILTER_HEIGHT: f32 = 37.0;
/// 右侧详情面板宽度。
pub const DETAIL_PANEL_WIDTH: f32 = 389.0;

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

// ---------- JetBrains New UI Dark 精确色板（原版截图像素采样） ----------
/// `0xRRGGBB` -> gpui `Hsla`，便于直接抄录原版实测色值。
pub fn rgb(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return hsla(0.0, 0.0, l, 1.0);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    hsla(h / 6.0, s, l, 1.0)
}

/// 主背景（Log / 左栏 / 右栏，实测 #191A1C）。
pub fn bg_main() -> Hsla {
    rgb(0x191A1C)
}

/// 工具栏 / 图标条 / 状态栏底色（实测 #26282C）。
pub fn bg_chrome() -> Hsla {
    rgb(0x26282C)
}

/// 面板分隔线（实测 1px #26282C）。
pub fn separator() -> Hsla {
    bg_chrome()
}

/// 列表悬停底色（JetBrains New UI Dark hover）。
pub fn hover_solid() -> Hsla {
    rgb(0x2E3033)
}

/// 列表选中底色（实测 #2A4371）。
pub fn selection_bg() -> Hsla {
    rgb(0x2A4371)
}

/// Log 标题蓝色标签底色（实测 #233558）。
pub fn log_tag_bg() -> Hsla {
    rgb(0x233558)
}

/// 主文字色（JetBrains New UI Dark foreground）。
pub fn text_primary() -> Hsla {
    rgb(0xDFE1E5)
}

/// 次要文字色（日期 / 哈希 / 计数）。
pub fn text_muted() -> Hsla {
    rgb(0x9DA0A8)
}

/// 链接 / 分支 chip 文字蓝。
pub fn link_blue() -> Hsla {
    rgb(0x548AF7)
}

/// 主按钮蓝（Commit / Commit and Push）。
pub fn primary_blue() -> Hsla {
    rgb(0x3574F0)
}

/// 把实测色板写入 gpui-component 全局主题，并同步 Base 层。
/// 必须在 `Theme::change` 之后调用（change 会用 registry 配置覆盖 colors）。
pub fn apply_jetbrains_palette(cx: &mut App) {
    {
        let t = Theme::global_mut(cx);
        t.background = bg_main();
        t.foreground = text_primary();
        t.secondary = bg_chrome();
        t.secondary_foreground = text_primary();
        t.secondary_hover = hover_solid();
        t.title_bar = bg_chrome();
        t.title_bar_border = bg_chrome();
        t.status_bar = bg_chrome();
        t.status_bar_border = bg_chrome();
        t.sidebar = bg_main();
        t.sidebar_foreground = text_primary();
        t.tab_bar = bg_main();
        t.tab_bar_segmented = bg_main();
        t.tab = bg_main();
        t.tab_active = bg_main();
        t.tab_active_foreground = text_primary();
        t.tab_foreground = text_muted();
        // Theme 自身的 list 字段是 ListSettings（列表组件设置），遮蔽了
        // Deref 到 ThemeColor 的同名颜色字段，必须显式走 colors。
        t.colors.list = transparent();
        t.list_even = transparent();
        t.list_head = bg_main();
        t.list_hover = hover_solid();
        t.list_active = selection_bg();
        t.list_active_border = transparent();
        t.table = transparent();
        t.table_even = transparent();
        t.table_hover = hover_solid();
        t.table_active = selection_bg();
        t.table_active_border = transparent();
        t.selection = selection_bg();
        t.accent = hover_solid();
        t.accent_foreground = text_primary();
        t.border = separator();
        t.input = rgb(0x2B2D30);
        t.muted = bg_chrome();
        t.muted_foreground = text_muted();
        t.popover = rgb(0x2B2D30);
        t.popover_foreground = text_primary();
        t.link = link_blue();
        t.link_hover = rgb(0x6B9AF5);
        t.ring = rgb(0x365880);
        t.primary = primary_blue();
        t.primary_hover = rgb(0x2F66CE);
        t.primary_active = rgb(0x2857B0);
        t.primary_foreground = rgb(0xFFFFFF);
        t.scrollbar = transparent();
        t.scrollbar_thumb = rgb(0x4B4D51);
        t.scrollbar_thumb_hover = rgb(0x5A5D63);
    }
    Theme::sync_base(cx);
}

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
/// 泳道色相（首色对齐原版品红 #993D69，其余按 JetBrains log 色系排布）。
const HUES: [f32; MAX_COLORS] = [0.92, 0.13, 0.33, 0.61, 0.0, 0.45, 0.55, 0.78];

pub fn lane_color(index: usize) -> Hsla {
    hsla(HUES[index % MAX_COLORS], 0.43, 0.42, 1.0)
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
