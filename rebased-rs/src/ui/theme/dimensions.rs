//! 尺寸 tokens：密度、布局、列宽、菜单、对话框、图形几何。
//!
//! 面板内**禁止**出现裸 `px(...)` 布局数值，一律引用此处的命名常量。

// ---------- 密度 ----------
/// 列表行高（Log / Changes / Detail 文件列表 / 各侧栏列表统一）。
/// 24 = IntelliJ 表格行高（原实现 Log 实测 34px，超出 42%）。
pub const ROW_HEIGHT: f32 = 24.0;
/// 全局列表行高的别名，语义化引用点。
pub const LIST_ROW_HEIGHT: f32 = ROW_HEIGHT;
/// 菜单行高（与列表行一致，原实现 26px）。
pub const MENU_ROW_HEIGHT: f32 = ROW_HEIGHT;
/// ref 标签（分支 / tag / HEAD）胶囊高度。
pub const BADGE_HEIGHT: f32 = 18.0;
/// 工具栏高度（对齐原版实测 44）。
pub const TOOLBAR_HEIGHT: f32 = 44.0;
/// 状态栏高度（原实现 31px，IntelliJ 为 26px 量级）。
pub const STATUSBAR_HEIGHT: f32 = 26.0;
/// 分组标题 / 面板标题行高。**全应用统一**，不再区分 22 / 自绘两种。
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;
/// 兼容旧引用（等同 `SECTION_HEADER_HEIGHT`）。
pub const GROUP_HEADER_HEIGHT: f32 = SECTION_HEADER_HEIGHT;

/// 状态栏仓库路径段最大宽度（超出省略）。
pub const STATUS_PATH_MAX_WIDTH: f32 = 420.0;
/// 空态/错误态说明文案的最大宽度（超出换行，避免横贯整个窗口）。
pub const MESSAGE_MAX_WIDTH: f32 = 560.0;
/// 分段控件（tabs / segmented）高度。
pub const SEGMENT_HEIGHT: f32 = 20.0;

// ---------- 三栏布局 ----------
/// 左侧 Commit 面板宽度：变更列表 + 提交输入区。
pub const COMMIT_PANEL_WIDTH: f32 = 344.0;
/// 左侧最左图标条宽度。
pub const ICON_STRIP_WIDTH: f32 = 40.0;
/// Log 主区标题行高度（“Log: <分支>” 蓝色标签行，实测 41px）。
pub const LOG_HEADER_HEIGHT: f32 = 41.0;
/// Log 主区过滤行高度（搜索框 + 过滤 chips，实测 37px）。
pub const LOG_FILTER_HEIGHT: f32 = 37.0;
/// 右侧详情面板宽度（实测 389）。
pub const DETAIL_PANEL_WIDTH: f32 = 389.0;
/// 右侧宽面板宽度（Diff / Blame / Compare / Conflicts / History / Reflog）。
pub const WIDE_PANEL_WIDTH: f32 = 680.0;
/// Rebase 面板宽度。
pub const REBASE_PANEL_WIDTH: f32 = 480.0;
/// Rebase 计划行的操作列（Pick/Squash/… 按钮）宽度，保证各行操作列对齐。
pub const REBASE_KIND_WIDTH: f32 = 76.0;

// ---------- 分栏拖拽 ----------
/// 分隔条命中宽度（视觉 1px，命中区域更宽便于拖拽）。
pub const SPLITTER_HIT_WIDTH: f32 = 6.0;
/// 各栏最小宽度，防止拖到不可用。
pub const MIN_LEFT_PANEL_WIDTH: f32 = 220.0;
pub const MIN_MAIN_PANEL_WIDTH: f32 = 320.0;
pub const MIN_RIGHT_PANEL_WIDTH: f32 = 320.0;
pub const MIN_WIDE_PANEL_WIDTH: f32 = 420.0;

// ---------- 图形泳道 ----------
/// 单条泳道占宽。IntelliJ 参考实现单列内容宽 ≈22px（`2*GRAPH_NODE_WIDTH + HGAP`）。
pub const LANE_WIDTH: f32 = 18.0;
/// 提交节点半径。IntelliJ `GRAPH_NODE_WIDTH = JBUI.scale(8)` → 直径 8px。
pub const DOT_RADIUS: f32 = 4.0;
/// 连线宽度。
pub const LINE_WIDTH: f32 = 1.5;

// ---------- 列表列宽 ----------
/// 状态列（A/M/D/R…）宽度。
pub const COL_STATUS_WIDTH: f32 = 16.0;
/// Log 作者列宽度。
pub const COL_AUTHOR_WIDTH: f32 = 120.0;
/// Log 日期列宽度（右对齐，容纳 `yyyy-MM-dd HH:mm`）。
pub const COL_DATE_WIDTH: f32 = 104.0;
/// Reflog selector 列宽度（容纳 `HEAD@{1234}`）。
pub const COL_SELECTOR_WIDTH: f32 = 92.0;
/// 树形缩进步长。
pub const TREE_INDENT: f32 = 14.0;

// ---------- 图标 ----------
/// 图标条 / 工具栏图标按钮边长。
pub const ICON_BUTTON_SIZE: f32 = 24.0;
/// 图标条按钮（含 4px 内边距）外框边长。
pub const ICON_STRIP_BUTTON_SIZE: f32 = 32.0;

// ---------- 输入 / 搜索 ----------
/// Log 搜索框宽度。
pub const SEARCH_WIDTH: f32 = 220.0;
/// 单行输入高度。
pub const INPUT_HEIGHT_SINGLE: f32 = 28.0;
/// 多行输入（提交信息 / 对话框正文）高度。
pub const INPUT_HEIGHT_MULTI: f32 = 72.0;

// ---------- 菜单 ----------
/// 菜单最小宽度（IntelliJ 菜单偏宽，避免快捷键列换行）。
pub const MENU_MIN_WIDTH: f32 = 220.0;
/// 菜单最大宽度（与依赖组件默认一致）。
pub const MENU_MAX_WIDTH: f32 = 500.0;
/// 菜单最大高度，超出滚动。
pub const MENU_MAX_HEIGHT: f32 = 420.0;

// ---------- 对话框 / 弹层 ----------
/// 标准对话框宽度。
pub const DIALOG_WIDTH: f32 = 460.0;
/// 对话框最大高度，超出后正文滚动。
pub const DIALOG_MAX_HEIGHT: f32 = 560.0;
/// 命令面板宽度。
pub const PALETTE_WIDTH: f32 = 480.0;
/// 命令面板顶部偏移。
pub const PALETTE_TOP: f32 = 96.0;
/// 命令面板最大高度。
pub const PALETTE_MAX_HEIGHT: f32 = 460.0;

// ---------- Diff ----------
/// 行号 gutter 单列宽度（容纳 4 位行号）。
pub const DIFF_GUTTER_COLUMN_WIDTH: f32 = 34.0;
/// 统一视图 gutter 总宽（old + new 两列）。
pub const DIFF_GUTTER_WIDTH: f32 = DIFF_GUTTER_COLUMN_WIDTH * 2.0;
/// 并排视图每半正文的基线宽度：与内容长度解耦，保证左右两半在任何视口
/// 宽度下都同时可见（IntelliJ 行为）；超宽行在半宽内裁剪，而非把右半
/// 挤出视口造成「上下排列」的观感。
pub const DIFF_SBS_CODE_BASE_WIDTH: f32 = 160.0;
/// diff 单行高度（编辑器行高，比列表行更密）。
pub const DIFF_LINE_HEIGHT: f32 = 18.0;
/// 等宽字体在 [`FONT_SIZE_MONO`] 下的单字符近似步进，用于估算横向滚动宽度。
pub const DIFF_CHAR_WIDTH: f32 = 7.5;

// ---------- 窗口 ----------
/// 主窗口初始尺寸。
pub const MAIN_WINDOW_WIDTH: f32 = 1440.0;
pub const MAIN_WINDOW_HEIGHT: f32 = 900.0;
/// 独立 Diff 窗口初始尺寸。
pub const DIFF_WINDOW_WIDTH: f32 = 1280.0;
pub const DIFF_WINDOW_HEIGHT: f32 = 860.0;
