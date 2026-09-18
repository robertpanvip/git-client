//! 尺寸 tokens：密度、布局、列宽、菜单、对话框、图形几何。
//!
//! 严格对齐 **JetBrains IntelliJ New UI** 官方尺寸规范。
//!
//! 面板内**禁止**出现裸 `px(...)` 布局数值，一律引用此处的命名常量。
//! 唯一例外：gpui 组件库自身的语义刻度（`px_1`/`px_2`/`gap_2`/`py_0p5`）
//! 用于微调内边距，不在此处重复定义。

// ---------- 密度（Density）----------
/// 列表行高（Log / Changes / Detail 文件列表 / 各侧栏列表统一）。
/// **IntelliJ 表格行高 = 24px**（New UI 标准）。
pub const ROW_HEIGHT: f32 = 24.0;
/// 全局列表行高的别名，语义化引用点。
pub const LIST_ROW_HEIGHT: f32 = ROW_HEIGHT;
/// 菜单行高（与列表行一致，IntelliJ 菜单 = 24px）。
pub const MENU_ROW_HEIGHT: f32 = ROW_HEIGHT;
/// ref 标签（分支 / tag / HEAD）胶囊高度。
pub const BADGE_HEIGHT: f32 = 18.0;

// ---------- 工具栏与状态栏（Toolbar & Status Bar）----------
/// 工具栏高度 —— **IntelliJ New UI = 38-40px**（比旧版 Darcula 44px 更紧凑）。
pub const TOOLBAR_HEIGHT: f32 = 38.0;
/// 状态栏高度 —— **IntelliJ = 24-26px**（New UI 紧凑模式）。
pub const STATUSBAR_HEIGHT: f32 = 24.0;
/// 分组标题 / 面板标题行高。**全应用统一**，不再区分 22 / 自绘两种。
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;
/// 兼容旧引用（等同 `SECTION_HEADER_HEIGHT`）。
pub const GROUP_HEADER_HEIGHT: f32 = SECTION_HEADER_HEIGHT;

// ---------- 文本与消息（Text & Messages）----------
/// 状态栏仓库路径段最大宽度（超出省略）。
pub const STATUS_PATH_MAX_WIDTH: f32 = 400.0;
/// 空态/错误态说明文案的最大宽度（超出换行，避免横贯整个窗口）。
pub const MESSAGE_MAX_WIDTH: f32 = 520.0;
/// 分段控件（tabs / segmented）高度。
pub const SEGMENT_HEIGHT: f32 = 20.0;

// ---------- 三栏布局（Three-Column Layout）----------
/// 左侧 Commit 面板宽度：变更列表 + 提交输入区。
pub const COMMIT_PANEL_WIDTH: f32 = 340.0;
/// Git 日志视图左栏分支树宽度（IntelliJ 实测 ≈196）。
pub const LOG_BRANCH_PANEL_WIDTH: f32 = 196.0;
/// 左侧最左图标条宽度（IntelliJ 工具窗口栏 = 40px）。
pub const ICON_STRIP_WIDTH: f32 = 40.0;
/// Log 主区标题行高度（"Log: <分支>" 蓝色标签行）。
pub const LOG_HEADER_HEIGHT: f32 = 38.0;
/// Log 主区过滤行高度（搜索框 + 过滤 chips）。
pub const LOG_FILTER_HEIGHT: f32 = 34.0;
/// 右侧详情面板宽度。
pub const DETAIL_PANEL_WIDTH: f32 = 380.0;
/// 右侧宽面板宽度（Diff / Blame / Compare / Conflicts / History / Reflog）。
pub const WIDE_PANEL_WIDTH: f32 = 680.0;
/// Rebase 面板宽度。
pub const REBASE_PANEL_WIDTH: f32 = 480.0;
/// Rebase 计划行的操作列（Pick/Squash/… 按钮）宽度，保证各行操作列对齐。
pub const REBASE_KIND_WIDTH: f32 = 76.0;

// ---------- 分栏拖拽（Splitter）----------
/// 分隔条命中宽度（视觉 1px，命中区域更宽便于拖拽）。
pub const SPLITTER_HIT_WIDTH: f32 = 6.0;
/// 各栏最小宽度，防止拖到不可用。
pub const MIN_LEFT_PANEL_WIDTH: f32 = 220.0;
pub const MIN_MAIN_PANEL_WIDTH: f32 = 320.0;
pub const MIN_RIGHT_PANEL_WIDTH: f32 = 320.0;
pub const MIN_WIDE_PANEL_WIDTH: f32 = 420.0;

// ---------- 图形泳道（Graph Geometry）----------
/// 单条泳道占宽。IntelliJ 参考实现单列内容宽 ≈22px（`2*GRAPH_NODE_WIDTH + HGAP`）。
pub const LANE_WIDTH: f32 = 18.0;
/// 提交节点半径。**IntelliJ `GRAPH_NODE_WIDTH = JBUI.scale(8)` → 直径 8px → 半径 4px**。
pub const DOT_RADIUS: f32 = 4.0;
/// 连线宽度（IntelliJ = 1.5px）。
pub const LINE_WIDTH: f32 = 1.5;

// ---------- 列表列宽（List Column Widths）----------
/// 状态列（A/M/D/R… 图标）宽度。
pub const COL_STATUS_WIDTH: f32 = 16.0;
/// Log 作者列宽度（容纳 "John Doe" 或中文名）。
pub const COL_AUTHOR_WIDTH: f32 = 96.0;
/// Log 日期列宽度（右对齐，容纳 `yyyy/M/d HH:mm` 与「昨天 HH:mm」）。
pub const COL_DATE_WIDTH: f32 = 104.0;
/// Log 短哈希列宽度（7 位短 hash）。
pub const COL_HASH_WIDTH: f32 = 64.0;
/// Reflog selector 列宽度（容纳 `HEAD@{1234}`）。
pub const COL_SELECTOR_WIDTH: f32 = 92.0;
/// 树形缩进步长（文件树 / 分支树层级缩进）。
pub const TREE_INDENT: f32 = 16.0;

// ---------- 文件视图（File View：文件夹树 / 编辑器）----------
/// 文件视图左栏（文件夹树）宽度（IntelliJ Project 工具窗口 ≈250-280）。
pub const FILE_TREE_PANEL_WIDTH: f32 = 260.0;
/// 编辑器行变更标记条宽度（行首窄色条）。
pub const EDITOR_CHANGE_BAR_WIDTH: f32 = 3.0;
/// 编辑器行级 blame 列宽度（提交时间 + 作者）。
pub const EDITOR_BLAME_WIDTH: f32 = 200.0;

// ---------- 图标尺寸（Icon Sizes）----------
/// 图标条 / 工具栏图标按钮边长（**IntelliJ = 24×24**）。
pub const ICON_BUTTON_SIZE: f32 = 24.0;
/// 图标条按钮（含 4px 内边距）外框边长（**IntelliJ = 32×32**）。
pub const ICON_STRIP_BUTTON_SIZE: f32 = 32.0;

// ---------- 输入 / 搜索（Input & Search）----------
/// Log 搜索框宽度（IntelliJ 搜索框偏宽，便于输入过滤条件）。
pub const SEARCH_WIDTH: f32 = 220.0;
/// 单行输入高度（IntelliJ ComboBox / TextField = 28px）。
pub const INPUT_HEIGHT_SINGLE: f32 = 28.0;
/// 多行输入（提交信息 / 对话框正文）高度（IntelliJ TextArea 默认 ≈72-80）。
pub const INPUT_HEIGHT_MULTI: f32 = 76.0;

// ---------- 菜单（Menus）----------
/// 菜单最小宽度（IntelliJ 菜单偏宽，避免快捷键列换行）。
pub const MENU_MIN_WIDTH: f32 = 220.0;
/// 菜单最大宽度（与依赖组件默认一致）。
pub const MENU_MAX_WIDTH: f32 = 500.0;
/// 菜单最大高度，超出滚动。
pub const MENU_MAX_HEIGHT: f32 = 420.0;

// ---------- 对话框 / 弹层（Dialogs & Popovers）----------
/// 标准对话框宽度（IntelliJ 对话框默认 ≈460-500）。
pub const DIALOG_WIDTH: f32 = 480.0;
/// 对话框最大高度，超出后正文滚动。
pub const DIALOG_MAX_HEIGHT: f32 = 560.0;
/// 命令面板宽度（IntelliJ Palette ≈480-520）。
pub const PALETTE_WIDTH: f32 = 500.0;
/// 命令面板顶部偏移（距窗口顶部）。
pub const PALETTE_TOP: f32 = 96.0;
/// 命令面板最大高度。
pub const PALETTE_MAX_HEIGHT: f32 = 480.0;
/// 分支部件弹窗（"搜索分支和操作"）宽度。
pub const BRANCH_POPUP_WIDTH: f32 = 360.0;

// ---------- Diff ----------
/// 行号 gutter 单列宽度（容纳 4-5 位行号 + padding）。
pub const DIFF_GUTTER_COLUMN_WIDTH: f32 = 36.0;
/// 统一视图 gutter 总宽（old + new 两列）。
pub const DIFF_GUTTER_WIDTH: f32 = DIFF_GUTTER_COLUMN_WIDTH * 2.0;
/// 并排视图每半正文的基线宽度：与内容长度解耦，保证左右两半在任何视口
/// 宽度下都同时可见（IntelliJ 行为）；超宽行在半宽内裁剪，而非把右半
/// 挤出视口造成「上下排列」的观感。
pub const DIFF_SBS_CODE_BASE_WIDTH: f32 = 160.0;
/// diff 单行高度（编辑器行高，比列表行更密 —— IntelliJ ≈18-20）。
pub const DIFF_LINE_HEIGHT: f32 = 19.0;
/// 等宽字体在 [`FONT_SIZE_MONO`] 下的单字符近似步进，用于估算横向滚动宽度。
pub const DIFF_CHAR_WIDTH: f32 = 7.6;

// ---------- 窗口（Windows）----------
/// 主窗口初始尺寸（IntelliJ 默认启动尺寸 ≈1440×900）。
pub const MAIN_WINDOW_WIDTH: f32 = 1440.0;
pub const MAIN_WINDOW_HEIGHT: f32 = 900.0;
/// 独立 Diff 窗口初始尺寸。
pub const DIFF_WINDOW_WIDTH: f32 = 1280.0;
pub const DIFF_WINDOW_HEIGHT: f32 = 860.0;

// ---------- 圆角（Border Radius）----------
/// 小圆角（按钮 / 输入框 / 小组件）—— **IntelliJ = 4-6px**。
pub const RADIUS_SM: f32 = 4.0;
/// 中等圆角（卡片 / 面板 / 弹层）—— **IntelliJ = 6-8px**。
pub const RADIUS_MD: f32 = 6.0;
/// 大圆角（对话框 / 大卡片）—— **IntelliJ = 8-12px**。
pub const RADIUS_LG: f32 = 8.0;

// ---------- 间距（Spacing）----------
/// 超小间距（图标与文字之间 / 紧凑元素组内）—— **2px**。
pub const SPACE_XS: f32 = 2.0;
/// 小间距（相关元素之间 / 列表项内）—— **4px**。
pub const SPACE_SM: f32 = 4.0;
/// 中等间距（区块之间 / 面板内分组）—— **8px**。
pub const SPACE_MD: f32 = 8.0;
/// 大间距（主要区块之间 / 面板间分隔）—— **12px**。
pub const SPACE_LG: f32 = 12.0;
/// 超大间距（独立区域之间 / 页面级分区）—— **16-20px**。
pub const SPACE_XL: f32 = 16.0;

// ---------- 字体大小（Font Sizes）----------
/// 超小字号（辅助信息 / 元数据 / 计数）—— **11px**。
pub const FONT_SIZE_XS: f32 = 11.0;
/// 小字号（次要文字 / 日期 / 哈希 / placeholder）—— **12px**。
pub const FONT_SIZE_SM: f32 = 12.0;
/// 基础字号（正文 / 列表项 / 按钮文字）—— **13px**（IntelliJ UI 字号）。
pub const FONT_SIZE_BASE: f32 = 13.0;
/// 中等字号（标题 / 面板头 / 重要标签）—— **14px**。
pub const FONT_SIZE_MD: f32 = 14.0;
/// 大字号（主标题 / 对话框标题）—— **16px**。
pub const FONT_SIZE_LG: f32 = 16.0;
/// 代码 / 等宽字体字号（编辑器 / diff / 行号 / blame）—— **12-13px**。
pub const FONT_SIZE_MONO: f32 = 12.5;
