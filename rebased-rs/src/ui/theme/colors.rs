//! 色板与 Git 语义色。
//!
//! 三部分：
//! 1. **基准色板**：IntelliJ New UI Dark 实测色值（`rgb()` 直接抄录）。
//! 2. **派生色**：跟随前景 alpha 派生，用于亮暗自适应（hover / border / badge 底）。
//! 3. **Git 语义色**：泳道、状态、ref 标签、diff 增删。

use gpui::{App, Hsla, hsla};
use gpui_kit::component::theme::Theme;
use rebased_rs::git::{ChangeStatus, MAX_COLORS};

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

/// 纯白（工具栏选中图标 / Log 标签文字）。
pub fn white() -> Hsla {
    rgb(0xFFFFFF)
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

/// 弹层/对话框底色。
pub fn popover_bg() -> Hsla {
    rgb(0x2B2D30)
}

/// 输入框底色。
pub fn input_bg() -> Hsla {
    rgb(0x2B2D30)
}

/// 列表悬停底色（JetBrains New UI Dark hover）。
pub fn hover_solid() -> Hsla {
    rgb(0x2E3033)
}

/// 列表选中行底色（图1 变更列表 / 图2 提交列表 / 分支树 实测 #33353B）。
pub fn list_row_selected() -> Hsla {
    rgb(0x33353B)
}

/// 分支树悬停行底色（参考截图实测 #27282A）。
pub fn tree_row_hover() -> Hsla {
    rgb(0x27282A)
}

/// Log 提交列表底色（图2 实测 #1D2336）。
/// 比面板底色 `bg_main` 略亮，把"表格区"与两侧面板区分开。
pub fn log_list_bg() -> Hsla {
    rgb(0x1D2336)
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

/// 更弱的文字色（禁用 / 占位）。
pub fn text_disabled() -> Hsla {
    rgb(0x6F737A)
}

/// 链接 / 分支 chip 文字蓝。
pub fn link_blue() -> Hsla {
    rgb(0x548AF7)
}

/// 主按钮蓝（Commit / Commit and Push）。
pub fn primary_blue() -> Hsla {
    rgb(0x3574F0)
}

/// 危险色（破坏性操作文字 / 危险按钮）。数据丢失类操作统一使用。
pub fn danger_color() -> Hsla {
    rgb(0xDB5C5C)
}

/// 危险色按钮底色。
pub fn danger_solid() -> Hsla {
    rgb(0xC94F4F)
}

/// 键盘焦点环。
pub fn focus_ring() -> Hsla {
    rgb(0x365880)
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
        t.list_active = list_row_selected();
        // 选中行不使用额外描边：焦点态改由 focus ring / 行首指示条表达，
        // 避免键盘焦点与"选中"在视觉上不可区分（原实现设为 transparent 会
        // 让两者完全同形）。
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
        t.input = input_bg();
        t.muted = bg_chrome();
        t.muted_foreground = text_muted();
        t.popover = popover_bg();
        t.popover_foreground = text_primary();
        t.link = link_blue();
        t.link_hover = rgb(0x6B9AF5);
        t.ring = focus_ring();
        t.primary = primary_blue();
        t.primary_hover = rgb(0x2F66CE);
        t.primary_active = rgb(0x2857B0);
        t.primary_foreground = white();
        t.danger = danger_solid();
        t.danger_foreground = white();
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

/// 行悬停底色。**全应用唯一**的"按前景派生"悬停来源。
pub fn hover_bg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.06)
}

/// 行选中底色（用于 List 组件之外的轻量列表）。
pub fn selected_bg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.12)
}

/// 交替行底纹（blame / 长列表斑马纹）。
pub fn stripe_bg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.04)
}

pub fn badge_bg(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.15)
}

/// 实色胶囊标签底色（ref 标签）：颜色更实，贴近 IntelliJ Log 的胶囊观感。
pub fn badge_solid_bg(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.28)
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

// ---------- ref 标签语义色 ----------
// IntelliJ Log 中三类 ref 视觉互异：HEAD/当前分支、本地分支、远程分支、tag。
// 原实现把「本地分支」并入「远程分支」同色，此处拆开。
/// HEAD / 当前分支。
pub fn head_color() -> Hsla {
    lane_color(0)
}

/// 普通本地分支。
pub fn branch_local_color() -> Hsla {
    lane_color(2)
}

/// 远程分支。
pub fn branch_remote_color() -> Hsla {
    lane_color(5)
}

/// tag：中性灰，与「新增」绿（同为 lane_color(2)）明确区分。
pub fn tag_color() -> Hsla {
    rgb(0xB0B2B6)
}

/// 兼容旧调用点：原 `remote_color` 语义即"非 HEAD 的 ref"。
pub fn remote_color() -> Hsla {
    branch_remote_color()
}

// ---------- Diff ----------
pub fn added_line_bg() -> Hsla {
    hsla(0.31, 0.6, 0.42, 0.14)
}

pub fn deleted_line_bg() -> Hsla {
    hsla(0.0, 0.65, 0.5, 0.13)
}

/// 编辑器「已修改行」底色（与 [`modified_color`] 同色相的行底色）。
pub fn modified_line_bg() -> Hsla {
    hsla(0.11, 0.8, 0.55, 0.12)
}

/// 文件树中文件行的文字色（比文件夹名略弱，形成层级）。
pub fn file_tree_file_fg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.88)
}

pub fn empty_half_bg() -> Hsla {
    hsla(0.0, 0.0, 0.5, 0.05)
}

pub fn hunk_bg() -> Hsla {
    hsla(0.58, 0.7, 0.55, 0.1)
}

/// 行号 gutter 底色（比正文略深，形成独立列）。
pub fn gutter_bg() -> Hsla {
    hsla(0.0, 0.0, 0.0, 0.16)
}

/// gutter 与正文之间的分隔线。
pub fn gutter_border() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.08)
}
