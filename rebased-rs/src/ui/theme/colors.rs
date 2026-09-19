//! 色板与 Git 语义色。
//!
//! 严格对齐 **JetBrains IntelliJ New UI (Islands/Darcula) Dark Theme** 官方色值。
//!
//! 三部分：
//! 1. **基准色板**：JetBrains Islands Dark / Darcula 精确色值（来自官方 `.theme.json`）。
//! 2. **派生色**：跟随前景 alpha 派生，用于 hover / border / badge 底等。
//! 3. **Git 语义色**：泳道、状态、ref 标签、diff 增删。

use gpui::{App, Hsla, hsla};
use gpui_kit::component::theme::Theme;
use rebased_rs::git::{ChangeStatus, MAX_COLORS};

// ---------- JetBrains New UI (Islands Dark) 精确色板 ----------
// 来源：JetBrains Platform SDK `platform-platform-resources/src/themes/` 中的
// `IslandsDark.theme.json` 与 `Darcula.theme.json`，以及 `Component.colors` key。

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

/// 纯白（工具栏选中图标 / 高亮文字）。
pub fn white() -> Hsla {
    rgb(0xFFFFFF)
}

// ==================== 基准背景色（Backgrounds）====================

/// 主背景（编辑器 / Log / 左右面板）—— Islands Dark `#313234`。
/// 比 Darcula (#2B2D30) 略亮，符合 New UI "清晰分区"设计理念。
pub fn bg_main() -> Hsla {
    rgb(0x313234)
}

/// 工具栏 / 图标条 / 状态栏底色 —— Islands Dark `#45494A`。
/// 与主背景形成明确层级分隔（New UI 核心特征）。
pub fn bg_chrome() -> Hsla {
    rgb(0x45494A)
}

/// 面板分隔线 / 边框 —— Islands Dark `#6E7073`（12% alpha on fg）。
pub fn separator() -> Hsla {
    rgb(0x6E7073)
}

/// 弹层 / 下拉菜单 / 对话框底色 —— Islands Dark `#3C3F41`。
pub fn popover_bg() -> Hsla {
    rgb(0x3C3F41)
}

/// 输入框底色 —— Darcula `ComboBox.background #3C3F41`。
pub fn input_bg() -> Hsla {
    rgb(0x3C3F41)
}

// ==================== 交互态（Interactive States）====================

/// 列表悬停底色 —— Islands Dark `#404248`（约 6% alpha on fg）。
/// 全应用**唯一**的悬停来源，禁止再出现 `fg@4%` 或字面量 `#2E3033`。
pub fn hover_solid() -> Hsla {
    rgb(0x404248)
}

/// 列表选中行底色（非焦点态）—— Islands Dark `#495055`。
pub fn list_row_selected() -> Hsla {
    rgb(0x495055)
}

/// 分支树 / 文件树悬停行底色 —— 比通用 hover 更淡，避免与选中态混淆。
pub fn tree_row_hover() -> Hsla {
    rgb(0x3A3C3E)
}

/// Log 提交列表专属底色 —— 比面板底略深，把"表格区"与两侧区分开。
/// 对应 IntelliJ Git Log 的蓝色调背景。
pub fn log_list_bg() -> Hsla {
    rgb(0x292D38)
}

/// 强选中 / 焦点态底色 —— JetBrains 蓝色 `#214283`（约 12% alpha）。
/// 用于图标条按钮激活、Log 当前行、主操作按钮。
pub fn selection_bg() -> Hsla {
    rgb(0x214283)
}

/// Log 标题蓝色标签底色 —— `#1A3B73`。
pub fn log_tag_bg() -> Hsla {
    rgb(0x1A3B73)
}

// ==================== 文字色（Foregrounds）====================

/// 主文字色 —— Islands Dark `#BBBBBB`（非纯白，降低视觉疲劳）。
pub fn text_primary() -> Hsla {
    rgb(0xBBBBBB)
}

/// 次要文字色（日期 / 哈希 / 计数 / placeholder）—— `#8C8C8C`。
pub fn text_muted() -> Hsla {
    rgb(0x8C8C8C)
}

/// 禁用 / 占位文字 —— `#6E7073`（与 separator 同级灰度）。
pub fn text_disabled() -> Hsla {
    rgb(0x6E7073)
}

// ==================== 文本层级派生（透明度收敛点）====================
//
// 全仓所有「基色 × opacity」的魔法系数只允许出现在本节；
// 调用点（diff/blame/empty/menu…）一律引用下列语义函数，
// 避免各视图自行定义 0.4/0.5/0.6/… 散点（见 UI_INCONSISTENCIES.md S-1）。

/// Gutter 装饰数字 —— diff / 编辑器 / blame 行号，比元数据再弱一档。
pub fn gutter_number_fg(base: Hsla) -> Hsla {
    base.opacity(0.7)
}

/// 弱元数据文本 —— blame 时间戳、blame 文件名等低强调信息。
pub fn meta_faint(base: Hsla) -> Hsla {
    base.opacity(0.8)
}

/// 空态辅助层级 —— 空态图标与次要提示，比空态主体文案再弱一档。
pub fn empty_faint(base: Hsla) -> Hsla {
    base.opacity(0.75)
}

/// 占位 / 幽灵文本 —— 数据缺失槽位（如未标注 blame 的行）。
pub fn ghost_text(base: Hsla) -> Hsla {
    base.opacity(0.5)
}

/// 危险色弱调 —— 危险菜单项的快捷键等次要危险文本。
pub fn danger_faint() -> Hsla {
    danger_color().opacity(0.75)
}

// ==================== 语义色（Semantic Colors）====================

/// 链接 / 分支 chip 文字蓝 —— JetBrains 链接蓝 `#589DF6`。
pub fn link_blue() -> Hsla {
    rgb(0x589DF6)
}

/// 主按钮蓝（Commit / Commit and Push / Primary Action）—— `#3574F0`。
pub fn primary_blue() -> Hsla {
    rgb(0x3574F0)
}

/// 危险色（破坏性操作文字）—— JetBrains 红 `#FF6B6B`。
pub fn danger_color() -> Hsla {
    rgb(0xFF6B6B)
}

/// 危险色按钮底色（稍深，提升可点击感）—— `#E05555`。
pub fn danger_solid() -> Hsla {
    rgb(0xE05555)
}

/// 键盘焦点环 —— JetBrains 蓝 `#2179D6`（与 primary_blue 同族）。
pub fn focus_ring() -> Hsla {
    rgb(0x2179D6)
}

/// 成功色（操作成功提示）—— JetBrains 绿 `#499C54`。
pub fn success_color() -> Hsla {
    rgb(0x499C54)
}

/// 警告色（需注意但非错误）—— JetBrains 橙 `#F0A020`。
pub fn warning_color() -> Hsla {
    rgb(0xF0A020)
}

/// 信息色（提示性信息）—— JetBrains 信息蓝 `#389FD6`。
pub fn info_color() -> Hsla {
    rgb(0x389FD6)
}

// ==================== 主题注入（gpui-component 全局主题）====================

/// 把 JetBrains Islands Dark 色板写入 gpui-component 全局主题。
///
/// 必须在 `Theme::change` 之后调用（change 会用 registry 配置覆盖 colors）。
/// 所有 `*Color()` 函数返回值与此处写入的值保持**逐位一致**。
pub fn apply_jetbrains_palette(cx: &mut App) {
    {
        let t = Theme::global_mut(cx);
        // --- Backgrounds ---
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

        // --- List / Table ---
        t.colors.list = transparent();
        t.list_even = transparent();
        t.list_head = bg_main();
        t.list_hover = hover_solid();
        t.list_active = list_row_selected();
        // 选中行不使用额外描边：焦点态由 focus ring 表达，
        // 避免键盘焦点与"选中"在视觉上不可区分。
        t.list_active_border = transparent();
        t.table = transparent();
        t.table_even = transparent();
        t.table_hover = hover_solid();
        t.table_active = selection_bg();
        t.table_active_border = transparent();
        t.selection = selection_bg();
        t.accent = hover_solid();
        t.accent_foreground = text_primary();

        // --- Borders / Inputs ---
        t.border = separator();
        t.input = input_bg();
        t.muted = bg_chrome();
        t.muted_foreground = text_muted();

        // --- Popover / Tooltip ---
        t.popover = popover_bg();
        t.popover_foreground = text_primary();

        // --- Links / Actions ---
        t.link = link_blue();
        t.link_hover = rgb(0x79B8FF);
        t.ring = focus_ring();
        t.primary = primary_blue();
        t.primary_hover = rgb(0x2864D8);
        t.primary_active = rgb(0x1E54B8);
        t.primary_foreground = white();
        t.danger = danger_solid();
        t.danger_foreground = white();

        // --- Scrollbar ---
        t.scrollbar = transparent();
        t.scrollbar_thumb = rgb(0x5C6066);
        t.scrollbar_thumb_hover = rgb(0x6E7073);
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

/// 实色胶囊标签底色（ref 标签）：更实，贴近 IntelliJ Log 的胶囊观感。
pub fn badge_solid_bg(color: Hsla) -> Hsla {
    hsla(color.h, color.s, color.l, 0.28)
}

pub fn border_color(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.12)
}

pub fn overlay_bg() -> Hsla {
    hsla(0.0, 0.0, 0.0, 0.50)
}

// ---------- Git 语义色 ----------
/// 泳道配色（JetBrains 暗色日志风格）：色相顺序对齐原版（首色品红系 = HEAD 语义，
/// lane 2 绿 = 本地分支、lane 5 青 = 远程分支，见下方 ref 语义色）；
/// 每条泳道独立 (s, l) —— 深色底上需要比正文更亮更饱和，统一低明度会发灰。
const LANES: [(f32, f32, f32); MAX_COLORS] = [
    (0.92, 0.52, 0.56), // 玫红（HEAD / 原版首色系提亮）
    (0.13, 0.90, 0.62), // 琥珀黄
    (0.36, 0.68, 0.54), // 草绿（本地分支）
    (0.58, 0.88, 0.64), // 天蓝
    (0.00, 0.85, 0.66), // 珊瑚红
    (0.47, 0.85, 0.60), // 青（远程分支）
    (0.75, 0.85, 0.68), // 紫罗兰
    (0.07, 0.95, 0.62), // 橙
];

pub fn lane_color(index: usize) -> Hsla {
    let (h, s, l) = LANES[index % MAX_COLORS];
    hsla(h, s, l, 1.0)
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

/// 新增文件绿 —— JetBrains 绿系 `#499C54`（与 success 同源）。
pub fn added_color() -> Hsla {
    success_color()
}

/// 删除文件红 —— JetBrains 红系 `#FF6B6B`（与 danger 同源：色相跟随
/// `danger_color()`，降饱和/降亮度以示区别，调 danger 色板时自动联动）。
pub fn deleted_color() -> Hsla {
    let danger = danger_color();
    hsla(danger.h, 0.65, 0.62, 1.0)
}

/// 修改文件橙 —— JetBrains 橙系 `#F0A020`（与 warning 同源）。
pub fn modified_color() -> Hsla {
    warning_color()
}

/// 重命名黄 —— JetBrains 黄系 `#F0C020`。
pub fn renamed_color() -> Hsla {
    hsla(0.13, 0.80, 0.56, 1.0)
}

/// 复制青 —— JetBrains 青系 `#20A0C0`。
pub fn copied_color() -> Hsla {
    hsla(0.53, 0.72, 0.47, 1.0)
}

/// 类型变更紫 —— JetBrains 紫系 `#9966CC`。
pub fn type_changed_color() -> Hsla {
    hsla(0.76, 0.55, 0.60, 1.0)
}

/// 冲突品红 —— JetBrains 品红系 `#CC3366`。
pub fn conflicted_color() -> Hsla {
    hsla(0.96, 0.68, 0.50, 1.0)
}

/// 未跟踪灰绿 —— JetBrains 灰绿系 `#689F6E`。
pub fn untracked_color() -> Hsla {
    hsla(0.35, 0.28, 0.52, 1.0)
}

/// 二进制文件 —— 中性灰蓝。
pub fn binary_color() -> Hsla {
    hsla(0.58, 0.45, 0.56, 1.0)
}

/// 错误 —— 与 danger 同源。
pub fn error_color() -> Hsla {
    danger_color()
}

// ---------- ref 标签语义色 ----------
// IntelliJ Log 中四类 ref 视觉互异：HEAD/当前分支、本地分支、远程分支、tag。

/// HEAD / 当前分支 —— 品红（lane 0）。
pub fn head_color() -> Hsla {
    lane_color(0)
}

/// 普通本地分支 —— 绿（lane 2，与 added 同色相）。
pub fn branch_local_color() -> Hsla {
    lane_color(2)
}

/// 远程分支 —— 青（lane 5）。
pub fn branch_remote_color() -> Hsla {
    lane_color(5)
}

/// tag —— 中性灰 `#A0A0A0`，与「新增」绿明确区分。
pub fn tag_color() -> Hsla {
    rgb(0xA0A0A0)
}

/// 兼容旧调用点：原 `remote_color` 语义即"非 HEAD 的远程 ref"。
pub fn remote_color() -> Hsla {
    branch_remote_color()
}

// ---------- Diff ----------
/// 新增行底色 —— 绿色低饱和度底。
pub fn added_line_bg() -> Hsla {
    hsla(0.31, 0.55, 0.38, 0.15)
}

/// 删除行底色 —— 红色低饱和度底。
pub fn deleted_line_bg() -> Hsla {
    hsla(0.0, 0.55, 0.45, 0.14)
}

/// 编辑器「已修改行」底色 —— 橙色低饱和度底。
pub fn modified_line_bg() -> Hsla {
    hsla(0.11, 0.70, 0.50, 0.13)
}

/// 文件树中文件行的文字色（比文件夹名略弱，形成层级）。
pub fn file_tree_file_fg(fg: Hsla) -> Hsla {
    hsla(fg.h, fg.s, fg.l, 0.88)
}

/// 空态半透明底色。
pub fn empty_half_bg() -> Hsla {
    hsla(0.0, 0.0, 0.5, 0.06)
}

/// diff hunk 头部底色。
pub fn hunk_bg() -> Hsla {
    hsla(0.58, 0.55, 0.50, 0.10)
}

/// 行号 gutter 底色（比正文略深，形成独立列）。
pub fn gutter_bg() -> Hsla {
    hsla(0.0, 0.0, 0.0, 0.18)
}

/// gutter 与正文之间的分隔线。
pub fn gutter_border() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.10)
}

// ==================== 语法高亮（编辑器预览）====================
// 精确取自 IntelliJ Darcula 官方编辑器配色（Settings > Editor > Color Scheme
// Defaults），与 New UI 暗色背景搭配保持原版观感。

/// 关键字 —— Darcula `Keyword #CC7832`。
pub fn syntax_keyword() -> Hsla {
    rgb(0xCC7832)
}

/// 类型 / 生命周期 / 属性键 —— Darcula `Instance reference #9876AA`。
pub fn syntax_type() -> Hsla {
    rgb(0x9876AA)
}

/// 函数调用 / 宏 / Markdown 标题 —— Darcula `Function call #FFC66D`。
pub fn syntax_function() -> Hsla {
    rgb(0xFFC66D)
}

/// 字符串 / 字符字面量 —— Darcula `String #6A8759`。
pub fn syntax_string() -> Hsla {
    rgb(0x6A8759)
}

/// 数字 —— Darcula `Number #6897BB`。
pub fn syntax_number() -> Hsla {
    rgb(0x6897BB)
}

/// 普通注释 —— Darcula `Line comment #808080`。
pub fn syntax_comment() -> Hsla {
    rgb(0x808080)
}

/// 文档注释（/// //!）与 shebang —— Darcula `Doc comment #629755`。
pub fn syntax_doc() -> Hsla {
    rgb(0x629755)
}

/// 注解 / 属性（#[..]、@decorator、@media）—— Darcula `Annotation #BBB529`。
pub fn syntax_attr() -> Hsla {
    rgb(0xBBB529)
}
