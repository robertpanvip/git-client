# UI_INCONSISTENCIES.md — 全 UI 一致性审计与修复台账

> 审计基准：IntelliJ IDEA New UI (Islands Dark) / Rebased Desktop。
> 证据来源：全量代码扫描（theme、components、app、git 数据层）。
> 状态标记：⬜ 未修 / 🔧 修复中 / ✅ 已修。

## 汇总表

| ID | Area | Current Problem | Expected Behavior | Root Cause | Fix | Status |
|----|------|-----------------|-------------------|------------|-----|--------|
| F1 | Diff 徽章 | `diff_view.rs::status_badge()` 本地 4 分支映射，Renamed/Copied/TypeChanged/Conflicted 全部兜底成 "M"+橙 | 与 Changes 面板同一 `status_color()` 语义（R 黄、C 青、U 品红…） | `FileDiff` 不携带 `ChangeStatus`，只有 3 个 bool；UI 被迫重建第二份状态映射 | `FileDiff` 增加 `status: Option<ChangeStatus>`，diff 解析层回填；徽章改调 `theme::status_color` | ✅ |
| F2 | Untracked 色 | untracked 文件在 Changes 面板为 "?" 灰绿，打开 diff 后徽章变 "A"+绿 | untracked ≠ added，跨面板语义一致 | `append_untracked_diffs` 伪造 "new file mode" 头，解析后 `is_new=true`，状态信息丢失 | 合成 diff 时打 `status=Untracked` 标记 | ✅（随 F1） |
| F3 | 状态映射 | 三份平行状态→色代码（diff_view 徽章 / editor_view 行条 / diff 行前景） | 文件级统一 `status_color()`；行级收口 theme | 各面板 inline match | diff_view 徽章删除本地映射；行级保留（与 theme 同源值） | ✅ |
| F4 | 语义色源头 | `deleted_color()` 裸 hsla，注释称与 danger 同源但未函数复用 | 与 added/modified 一致的派生模式 | 色板两种实现模式混用 | 从 `danger_color()` 派生（降饱和），调 danger 色板时联动 | ✅ |
| F5 | 文件树状态色 | Files 文件树完全不着 Git 状态色（只有 选中白/目录 fg/文件 fg@0.88） | 变更文件名按状态着色（IDEA 项目树行为） | `render_file_tree` 只收 `&[String]`，状态数据进渲染前被丢弃 | 传入 `status_of: &HashMap<String, ChangeStatus>`（由 `state.changes` 构造），文件名按 `theme::status_color` 着色；选中态仍白 | ✅ |
| F6 | 导入路径 | changes.rs / detail_view.rs 经 `graph_view::status_color` re-export 取色 | 直接从 `theme` 导入语义色 | 历史遗留中间层 re-export | 改为 `use crate::ui::theme::status_color`；保留 re-export 兼容 | ✅ |
| I-1 | 侧栏顺序 | 图标条顺序 Commit → Files → History → Log，Project/File Tree 屈居第 2 | IntelliJ New UI：Project 工具窗图标居首 | 入口按「工作区优先」假设硬编码 | Files(Project) 提到 Commit 之前 | ✅ |
| I-2 | 侧栏 active | strip-commit 看 `main_view`、strip-history 看 `sidebar`，两套判定维度 | 每个工具窗按钮 active 只由自身可见性决定 | MainView 与 SidebarMode 双状态机 | 保持现状但统一注释说明（结构性重构风险大，另行任务） | ⬜ |
| I-5 | 文件树徽标 | 文件树无状态徽标（仅着色，随 F5 修复） | 行尾状态字母徽标（可选增强） | 同 F5 | F5 已完成着色；徽标不做（避免列表过宽） | ✅ |
| I-6 | 类型图标彩色 | 单色 alpha-mask 管线，类型图标靠形状区分，非 IDEA 彩色图标 | 彩色文件类型图标 | 渲染管线单色 | 引入 gpui `img()` 彩色管线（Embedded 资产 → SVG 原色光栅化）：rust_embed 增挂 `file_types/**`，27 个 JetBrains 官方图标（expui 暗色变体 + rust/python 插件 + folder）；`icons.rs` 新增 `colored_file_icon`/`file_type_icon`/`folder_icon`（16×16 `FILE_TYPE_ICON_SIZE` token），file_tree / file_tab_bar / changes 三处接入 | ✅ |
| I-7 | Changes 状态列 | 状态列是字母（M/A/U）而非「按状态着色的类型图标」 | 同 I-6 | 同 I-6 | 行首字母槽替换为官方彩色类型图标；状态字母移至行尾、仍用 `status_color` 着色（与文件名/文件树同源，不依赖纯颜色分辨） | ✅ |
| I-8 | Changes 层级 | 组头与子行零缩进差（同一 `ROW_PADDING_X`）、同高 24px | 组头可折叠 + 子行缩进一级 | `group_header_controls`/`list_row` 未预留 indent 槽位 | 组头加 chevron + 折叠状态；子行加 `TREE_INDENT` 缩进 | ✅ |
| I-9 | Changes 折叠 | 组头无展开/折叠，分组纯渲染期切片 | 每组可折叠（▾/▸），状态持久于会话内 | 无每组展开状态字段 | `changes_collapsed: HashSet<String>` 状态 | ✅ |
| T-1 | 文件 Tab | 无 Tab 模型：`files_selected: Option<String>` 单值，打开新文件覆盖旧的 | IDE 式文件 Tab：去重激活、× 关闭、邻位选中、空态回退 | state 只有单值选中，无 `open_tabs` 集合 | `open_tabs: Vec<String>`（active 复用 `files_selected`）+ `open_file` 插当前 Tab 右侧 + `close_file_tab`（右邻→左邻→空态）；新 Tab 组件 `file_tab_bar`（横向滚动/图标/×/active 下划线），编辑器顶部常驻 | ✅ |
| T-2 | Tab 组件误用 | `components/tabs.rs` 是分段控件（Changes/Shelve 页签），无关闭按钮插槽 | 文件 Tab 栏独立原语 | 组件语义不同 | 新增 `components/file_tab_bar.rs`（`file_tab_bar` + `TabActivate`/`TabClose` + `neighbor_after_close` 纯函数及单测） | ✅ |
| B-1 | 关闭注解 | 侧栏 Blame 面板 × 只切视图不清 state：`blame_groups`/`blame_path` 残留，旧数据复活 | 关闭=清除状态；重开=重新拉取 | `sidebar_back` 通用返回无对应 clear（对照 `clear_detail` 存在） | 新增 `clear_blame` use_case；`sidebar_back` 检测 `SidebarMode::Blame` 时调用；× 按钮与 Esc 两入口共用此路径 | ✅ |
| B-2 | 关闭注解（编辑器） | 编辑器右键「Close Annotations」链路完整（state→重载→渲染） | — | — | 无需修改（审计确认无断点） | ✅ |
| D-1 | Diff 折叠 | hunk 不可折叠 | IDEA diff 每 hunk 可折叠 | 无折叠状态 | `diff_folded: HashSet<(usize,usize)>` + chevron 按钮（主窗口 + 独立窗口），展开态渲染 body，折叠态仅 header；单测覆盖高度/偏移 | ✅ |
| D-2 | Diff 导航 | 无上一处/下一处 hunk 导航 | F7 / Shift+F7 + 面板 ↑↓ 按钮 | 无导航 action 与索引 | `NextDiffHunk`/`PrevDiffHunk`（F7/Shift+F7）+ 独立窗口 ↑↓ 工具栏按钮；`step_hunk`/`scroll_hunk_into_view` 共享算术 + `track_scroll` 程序化滚动；diff 重载时校验/重置导航与折叠 | ✅ |
| C-1 | Conflict 导航 | 冲突 hunk 卡片无上一处/下一处 | prev/next 在卡片间滚动并高亮当前卡片 | 无导航索引，卡片为变高元素无法用 offset 定位 | 面板头 ↑↓ 按钮 + `step_conflict_index` 循环算术（含空/过期索引防御）+ `conflict_list: ListState`（GPUI `list()` 虚拟滚动）`scroll_to_reveal_item` 定位 + 当前卡片 `primary_blue` 描边高亮；选文件/清选/不在合并态时同步 `reset` 与导航索引；单测覆盖循环与过期索引 | ✅ |
| S-1 | 间距散点 | `.opacity()` 系数 6 档散落（0.4/0.5/0.6/0.7/0.75/0.8） | 收敛为 theme 派生函数 | 派生色未建 token | `colors.rs` 新增「文本层级派生（透明度收敛点）」节：`gutter_number_fg`(0.7)/`meta_faint`(0.8)/`empty_faint`(0.75)/`ghost_text`(0.5)/`danger_faint()`(0.75)；11 处调用点全部改引语义函数（menu/empty_state/diff/editor/blame/commit_list/changes），调用点零魔法系数；空态基色统一为 `muted_foreground`（消除 fg×0.4 与 muted×0.6 两处离群） | ✅ |
| E-1 | Empty/Error | 各面板空态文案/样式基本统一（`empty_state` 已被 Diff/Files 使用）；Error 面板间形式不一（diff 窗口文本 vs 主窗 toast/banner） | 统一 Empty/Error 原语 | 部分面板未走 `empty_state`；Files 错误态使用 emoji "⚠️"；`empty_hint` 死代码构成第二套空态 | `error_state()` 原语（与 `empty_state` 同构：同 token 布局/字号/间距，标题错误色 + 灰色详情，按钮由调用方 `.child()` 追加）；files.rs / log.rs / diff_window.rs 三处手写错误态与 loading 态全部切换；删除违规 emoji 与死代码 `empty_hint` | ✅ |

## 数据层补充（非 UI 但为 UI 根因）

| ID | 问题 | 修复 |
|----|------|------|
| G-1 | `FileDiff`（git/types.rs）缺 `status` 字段 | 增加 `status: Option<ChangeStatus>`；`parse_unified_diff` 在 `rename from/to` 时置 `Renamed`；untracked 合成 diff 置 `Untracked`；`new file mode` 置 `Added`；`deleted file mode` 置 `Deleted` |
| G-2 | `open_file` 逻辑内联在 `AppView`，不走 use_case 层 | 保持（tab 化后收敛为 `open_file_tab` 方法，位于 files.rs） |

## 修复执行顺序（P0→P1→P2）

1. ✅ P0：Git 状态色统一（F1/F2/F3/F4/F6 + G-1）
2. ✅ P0：侧栏层级（I-1）
3. ✅ P0：Changes 层级 + 折叠（I-8/I-9）
4. ✅ P0：文件树状态色（F5/I-5）
5. ✅ P0：文件 Tab 模型（T-1/T-2）
6. ✅ P1：关闭注解状态清除（B-1）
7. ✅ P1：Diff 折叠 + F7 导航（D-1/D-2）
8. ✅ P1：Conflict 导航（C-1）
9. ✅ P2：彩色类型图标管线（I-6/I-7）、间距 token 收敛（S-1）、Error 形式统一（E-1）

> ⚠️ 2026-09-19 勘误：T-1/T-2/B-1/D-1/D-2/C-1 曾被误标为 ✅（上一会话更新了
> 台账但代码未落地），本次全量代码复核后已改回真实状态，并按上述顺序重新实施。
> 截至 2026-09-19：重新实施已完成至 C-1（P0 全部 + P1 全部，check/test/clippy 全绿），
> 剩余 P2 项（I-6/I-7/S-1/E-1）均已收敛为共享原语落地。

## R 系列：IDEA 精调（2026-09-19，P2 之后）

> 背景：P0/P1/P2 收敛后整体已接近，但对照 IDEA 仍有精度差距。
> 逐区审计结论与修复：

| ID | Area | 审计结论（根因） | 修复 | Status |
|----|------|------------------|------|--------|
| R-1 | Typography | 双 token 源：dimensions `font_size_*` 原始刻度与 typography 语义别名并存，约百处调用点直连原始刻度；`TextRole` 零采纳；badge 处 `font_size_xs() - 2.0` 魔法算术；commit hash 列缺 mono 字体族 | 原始刻度锁 `pub(crate)`（仅供 typography 派生），全部调用点收敛到语义别名（`font_size_meta/body/title/code`）；新增 `font_size_badge()`；hash 列 `font_size_code()` + `.font_family(mono)`；diff/editor 7 处 mono 直连改 `font_size_code()` | ✅ |
| R-2 | Toolbar | `render_toolbar` 手写根容器，缺 IDEA 底部 1px 分隔线；`components::toolbar()` 为零调用死原语 | 根容器切换到共享原语 `toolbar(fg)`（chrome 底色 + `border_b_1` + `chrome_divider`）；原语间距对齐实际值（gap XS + px SM），视觉零回归；工具栏构造唯一口径 | ✅ |
| R-3 | Commit Graph | 几何已对齐（半径/弯道/合并双节点），差距在配色：泳道统一 s=0.43/l=0.42，深色底上发灰，非 JetBrains 暗色高饱和风格 | `HUES` 统一明度 → `LANES` 每色独立 (s, l)（玫红/琥珀/草绿/天蓝/珊瑚/青/紫/橙）；保留语义色相：lane0 品红=HEAD、lane2 绿=本地分支、lane5 青=远程分支 | ✅ |
| R-4 | Detail | 动作栏 4 组按钮扁平挂在外层（gap SM），无分组容器，组间距与组内距无差 | 每组包进共享 `toolbar_group()`（组内 gap XS），组间 `v_separator`；4 组语义：提交操作（cherry-pick/revert/undo/drop）、历史改写（rebase/reset/reword/checkout）、ref 操作（branch/tag/copy-sha）、查看（diff/compare） | ✅ |
| R-5 | Log Filter UI | 双 caret：内层 `Button` 设 `dropdown_caret(true)`，而 `DropdownButton` 自带 popup 半区已渲染 caret（gpui-component `dropdown_button.rs:194`）；双展示模型：按钮 label 显示选中值（main/alice/This week）的同时又渲染「Branch: main ×」等 chip，同一条件重复呈现 | 组件语义归位：删除 3 处内层 `dropdown_caret`（caret 责任归 popup 半区，点击区域/菜单行为不变；其余 3 处 DropdownButton 用法本就干净）；统一为 IDEA 语义：按钮固定显示过滤器名 Branch/User/Date，选中值只走 active filter chip（点击清除即恢复默认名）；菜单 checked 状态与 All Branches/Users/Time 清除逻辑不动 | ✅ |
| R-6 | Graph Geometry | 几何基准未文档化且列宽不对称：`graph_column_width = LANE_WIDTH*n + DOT_RADIUS` 右侧多 4px 与左侧 5px 不对称；elbow 半径 3px 在 24px 行高上偏紧显歪斜；merge 空心圆挖空色硬编码 `bg_main()`，而 Log 列表实际背景是 `log_list_bg()` → 节点中心色差 | 确立唯一几何模型（模块文档化）：lane center `x = lane*LANE_WIDTH + LANE_WIDTH/2`（整数像素）、dot 圆心 `(lane_x(lane), row_center)`、线中心线过 lane center、转折「竖直→elbow→水平→elbow→竖直」关于 row center 对称；验证 GPUI `curve_to(to, ctrl)` = 二次贝塞尔（终点在前），调用序正确；列宽收敛为 `LANE_WIDTH*n`（两侧各 5px 对称），Graph-Subject 间距由行 `gap` 独立控制与泳道数解耦；elbow 半径 3→5px（`ROW_HEIGHT/5`，IDEA 档）；merge 挖空色改为 `lane_canvas` 接收 `log_list_bg()`，不再依赖 `bg_main()` | ✅ |

| R-7 | 交互正确性 | 两个「关不掉」同属 GPUI 事件/身份机制误用：① Tab × 关不掉——GPUI bubble 阶段把 click 派发给命中链上全部 hitbox（子→父），× 的 on_click 关闭后 Tab 本体 on_click（activate）随即将刚关闭的文件重新插回 tabs（close_button 原注释「事件命中最深元素」假设错误）；② 关闭注解菜单失效——编辑器行 div 无 `.id()` 且 `.context_menu()` 对所有行是同一调用点，ContextMenuExt 回退 `ElementId::CodeLocation(caller)`，而 uniform_list 不为虚拟行提供独立 id 上下文（gpui-pre element.rs 仅当元素有 id 才压栈派生 GlobalElementId）→ 所有可见行共享同一 ContextMenuState（同一 Rc<RefCell>：hitbox/open/position/menu_view），open 时每行各渲染一个重叠浮层菜单，点击在 N 个实例间竞争失效。旁证：chip（badge.rs 有 id）/list_row（有 id）/log.rs 单实例容器的右键菜单均正常 | ① close_button 加 `.block_mouse_except_scroll()`（阻断非滚动鼠标事件冒泡到 Tab 本体，保留滚轮穿透）；② 行 div 加 `.id(("ed-line", line.number as usize))`，ContextMenuExt 派生唯一 Name id，每行获得独立菜单状态 | ✅ |

| R-8 | Editor Blame 显隐 | R-7 修复「菜单点不动」后暴露的第二层问题：`render_line` 虽接收 `blame_enabled` 却只用于切换右键菜单文案，blame_cell 无条件 `.child()` 进行——数据层（`load_worktree_file(_, _, false)` → `line.blame=None`）只是把 cell 内容清空（透明背景 + 空文字），cell 自身仍占 `EDITOR_BLAME_WIDTH`，关闭注解后留下一条空白列、代码区不变宽；`row_min_w` 亦无条件计入 blame 宽，横向滚动长度虚大 | `blame_enabled=false` 时整列不构建不追加（IDEA 行为：列消失、代码区左移扩展）；`row_min_w` 条件计入 `EDITOR_BLAME_WIDTH` | ✅ |

| R-9 | Commit 工具窗 | 用户三项反馈：① 每文件行的「查看差异/回滚」图标按钮应移除，双击文件应在右侧开「提交：文件名」diff Tab——但原行单击直接 `toggle_stage`（真实 git 操作），无法与双击意图区分；② 变更文件应收在「变更」分组下且列表与分组间有 gap；③ 底部提交操作原为分裂按钮（Commit ▾ 下拉藏 提交并推送/设置），应平铺为双按钮 + 设置齿轮最右，设置弹出框对齐 IDEA | ① 行点击改「生成号 + 240ms 定时器」消歧：单击延迟暂存（`pending_stage_gen` 代号校验后才 `toggle_stage`），双击递增代号作废定时器并 `open_commit_diff_tab` 在预览区登记「提交：文件名」Tab（(路径, 暂存态) 去重；Tab 条序号 id 防同文件双 Tab 重复 ElementId；关闭走右邻优先/左邻回退，最后一个关闭收起 diff 面板）；Tab 条复用编辑器 Tab 样式（diff 图标 + active 下划线 + ×，`block_mouse_except_scroll` 防冒泡误激活）；行内 查看差异/回滚 按钮删除（右键菜单保留全部操作）；② 变更/未版本管理两组已由 `collapsible_group_header` 分组承载，组头 `mt(SM)` 与列表留 gap（确认既有结构满足，无需改动）；③ 提交区改「提交」primary(flex_1) + 「提交并推送」+ 右端设置齿轮；CommitSettings 弹出框对齐 IDEA 分组「提交选项 / 提交信息检查」，新增「允许空提交信息」开关，`do_commit`/`do_commit_and_push` 尊重该开关；贮藏入口收敛至 Alt+` 快切与 Shelve 页签 | ✅ |

| R-10 | 分支搜索弹层 | 工具栏分支按钮打开的弹层（`branch_popup`）已有搜索框 + 动作列表 + 本地/远程分支，但与 IDEA Git 工具窗的「分支和操作」弹层差距：① 动作列表缺「签出标记或修订...」；② 本地/远程是扁平分组 header，不是可折叠 chevron（截图里 `▼ 本地` 展开、`> 远程` 折叠）；③ 当前分支行没有强调色、ahead/behind 计数 badge、upstream tracking（`origin/main >`）显示 | ① 新增第 5 项动作「签出标记或修订...」（Ic::Checkout，无快捷键）；② 替换 `group_header` 为可折叠 chevron 分组（`branch_popup_locals_expanded` / `branch_popup_remotes_expanded` 两个布尔状态，默认展开）；③ 抽离 `render_branch_popup_local_row` 渲染本地行：当前分支金色强调（h=48, s=0.85, l=0.62）、upstream 绿色 tracking 提示（`origin/main >`）、ahead/behind badge（`↑2 ↓1`）；远程分组保留普通 `menu_row`；删除独立的「标签」分组（签出标记统一走第 5 项动作） | ✅ |

| R-11 | 历史记录入口 | 独立入口位置不对齐 IDEA：左侧竖条挂「历史记录」图标（`strip-history`）直接打开侧栏 History 面板，而 IDEA 的文件/目录历史从项目树右键 Git 子菜单触发（Show History），竖条上没有独立历史入口 | 删除 `strip-history`；文件树每行（`render_row`，含 `.id("ft-{path}")` 稳定身份，R-7 经验）追加 `context_menu` → Git 子菜单 → 显示历史记录：文件走 `log_follow`（`--follow -- path`），目录走新增 `log_path`（`-- path` 无 follow，trait + Repository + use_case `open_dir_history` + AppView 包装）；从文件视图触发时切回工作区视图再开侧栏面板（沿袭原 strip 行为，侧栏不承载于 Files 视图）；历史面板本体、Ctrl+Alt+4 快捷键、detail 面板单文件历史按钮均保留 | ✅ |

| R-12 | Code 编辑区高亮 | 编辑器纯单色渲染：逐行 `div().child(code)`，关键字/字符串/注释/数字与普通代码同色，与 IDEA Darcula 语法配色差距明显 | 新增 `ui/highlight.rs` 轻量分词器（Rust/JS/TS/Python 关键字表 + 字符串/字符/注释/数字/装饰器规则，逐字符扫描产出 `HighlightSpan`）+ Darcula 语义色板（keyword 橙 `#CC7832`、string 绿 `#6A8759`、comment 灰 `#808080`、number 蓝 `#6897BB`、fn 黄 `#FFC66D`）；`editor_view::render_line` 按行分词并把 span 渲染为着色 span，`use_cases::load_worktree_file` 传入文件路径供语言判定 | ✅ |

| R-13 | Changes 三态复选框 | 分组头复选框是布尔 `Checkbox`，无法表达「部分选中」：组内勾选一部分时组头显示全不选（IDEA 为半选短横）；选中态勾形由 base 指示器承载但受控模型缺失 | 新增 `TriStateCheckbox` 组件：包装 `gpui_kit::base::Checkbox`（`CheckboxState::Unchecked/Indeterminate/Checked` 原生三态 + `activated()` 恰为 IDEA 切换语义「半选/全不选→全选，全选→全不选」），视觉复刻 facade Checkbox XSmall（12px 指示器/1px 描边/4px 圆角/白色 Check 或 Dash 字形 spring 淡入）；`changes_section` 分组头改传 `selected_count` 推导三态（0→未选、全部→勾、部分→半选），`on_click` 按下一语义态全选/清空整组 | ✅ |

| R-14 | Cherry-Pick 入口 | Log 列表右键菜单已有摘取提交（log.rs），但侧栏 History / Reflog 面板的提交行无任何右键菜单；用户预期「日志面板某一项的右键菜单」有摘取入口（reflog 摘取更是找回丢失提交的经典路径） | `sidebar/history.rs` 两面板行加 `context_menu`（行 id `history-{id}` / `reflog-{selector}` 唯一，R-7 经验）：History 行 = 检出 {short} / 摘取提交 / 回滚提交 / 查看差异 / 复制 SHA；Reflog 行 = 检出 / 摘取提交 / 查看差异 / 复制 SHA；全部复用既有 actions（`checkout_commit`/`cherry_pick_commit`/`revert_commit`/`copy_commit_id`/`open_commit_diff_window`）与 `menu_item`/`menu_width` 原语，菜单宽度对齐 log.rs | ✅ |

> R 系列状态（2026-09-19）：Toolbar ✅ / Commit Graph ✅ / Detail ✅ / Typography ✅ / Log Filter UI ✅ / Graph Geometry ✅ / 交互正确性 ✅ / Editor Blame 显隐 ✅ / Commit 工具窗 ✅ / 分支搜索弹层 ✅ / 历史记录入口 ✅ / Code 编辑区高亮 ✅ / Changes 三态复选框 ✅ / Cherry-Pick 入口 ✅。
> 验证：cargo check / clippy / test 全绿。
