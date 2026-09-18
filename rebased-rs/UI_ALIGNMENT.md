# rebased-rs · UI 对齐评审（Rebased Desktop）

- 参考对象：[DetachHead/rebased](https://github.com/DetachHead/rebased) —— 基于 IntelliJ platform 的 Git 客户端（"a git client based on the IntelliJ platform"），本体是移除全部捆绑插件、仅保留 Git 集成的 JetBrains IDE，外加少量 UI tweaks。因此**视觉基准 = IntelliJ "New UI" 深色主题下的 Git 工具窗口**。
- 参考源码：`/workspace/rebased-source`（sparse checkout，仅 `plugins/git4idea/backend/src`）。
- 被评审代码：`/workspace/rebased-rs`，基线 commit `0cb1685` + 工作区未提交改动。
- 评审日期：2026-09-18
- 评审方法：**逐文件阅读实际实现代码**，所有结论附 `file:line`。凡未读到的代码不做判断。菜单/按钮/输入框的像素值来自实际依赖组件库源码（`gpui-component 0.6.1` 的 `menu/popup_menu.rs`、`button/button.rs`、`input/input.rs`、`sizing.rs`），非推测。
- 本文件**不采纳** `REBASED_UX_REVIEW.md` 的完成度百分比。该文档的 P0/P1 口径几乎全部是**功能覆盖度**（"后端有 + UI 有入口"），与本任务要求的 **visual structure / interaction model** 不是同一件事。本文件对其结论作独立复核。

---

## 0. 证据规则与总体结论

### 0.1 判定分级

| 标记 | 含义 |
|---|---|
| ✅ | 结构与视觉均已对齐，无需改动 |
| 🟡 | 已有实现，但视觉/交互参数与基准不符（数值级差距） |
| 🟠 | 结构缺失或半成品（如无 resizable、无滚动、无选中态） |
| 🔴 | 完全缺失 |

### 0.2 总体结论（先说最重要的一句）

**rebased-rs 当前不是"另一个 GPUI Git 客户端"，但也不是"Rebased 的 UI 用 GPUI 重写"。**

它已经具备正确的**信息架构**（icon strip → Changes/Composer → Log graph → 右侧 Detail/Diff 三栏 + 工具栏 + 状态栏），并且 `theme.rs` 里确实沉淀了一批从原版截图采样的尺寸（工具栏 44 / 行高 26 / 状态栏 31 / 左栏 344 / 图标条 40 / 右栏 389 / Log 头 41 / 过滤行 37）。**骨架方向是对的。**

但**设计系统没有真正建立**，导致三类系统性缺陷：

1. **Token 被大面积绕过**：真正决定观感的数值散落在各面板字面量里 —— 面板宽 `680.`/`480.`（`panels.rs:687-691`）、搜索框 `200.0`（`toolbar.rs:964`）、命令面板 `420.`/`pt(96.)`（`panels.rs:1325,1329`）、对话框 `440.`/输入 `64.`（`panels.rs:1573,1765`）、状态列 `14.`（`toolbar.rs:1378`）、`+/-` 列 `12.`（`toolbar.rs:1514`）、author `96.`/date `72.`（`commit_list.rs:181,191`）、reflog selector `76.`（`panels.rs:1102`）。`theme::ROW_HEIGHT` 被 Changes 行完全忽略。
2. **同一视觉元素在不同面板各写一遍**：面板标题有 2 种写法（自绘 `text_sm` vs `group_header` `text_xs`）、列表行有 3 种（`px_2 py_0p5 rounded hover` vs 无 padding 无 hover vs checkbox 行）、subject 字号有 2 种（`text_sm` vs `text_xs`）、悬停底色有 3 个来源（`hover_solid #2E3033` token / `fg@6%` 派生 / `fg@4%` stripe）、分隔线有 2 套（`theme.border #26282C` vs `fg@12%`）。
3. **关键交互态缺失**：面板**全部不可拖拽调整宽度**（无 `resizable`）；Rebase/Compare/History/Reflog **不可滚动**（无 `overflow_y_scroll`，被侧栏 `overflow_hidden` 静默裁剪）；菜单**无图标列、无快捷键列**（全部用 `on_click` 而非 `.action()`）；**所有菜单里的破坏性操作（Delete/Drop/Revert/Force Push/Discard）都不是危险色**；对话框**无 max-height、无滚动、无 Enter 默认按钮、无自动聚焦、无内联校验、主按钮不禁用**；diff **无独立 gutter（无 gutter 底色与分隔线）、无横向滚动、无 hunk 折叠、无变更间导航**；命令面板**无搜索框、无键盘导航**；Log **无表头**。

### 0.3 与 `REBASED_UX_REVIEW.md` 的分歧（复核结论）

| 该文档结论 | 复核结论 | 证据 |
|---|---|---|
| "Desktop UI ~90%" | **功能覆盖度 ~90% 成立；视觉/交互对齐度显著低于此** | 见 §1 矩阵：35 个领域中 4 个 ✅、17 个 🟡、11 个 🟠、3 个 🔴 |
| "Rebased UX 对标综合 ~95%，核心 UX ~98%" | **仅对"操作路径同构"成立；不适用于视觉结构** | 菜单无图标列/快捷键列、破坏项无危险色、对话框无 Enter/聚焦/校验、面板不可拖拽、多处不可滚动 |
| "三色 ref 徽章 ✅" | 实为**两类**：本地分支与远程分支同色 | `components/mod.rs:18-20` 的 else 分支不区分 local/remote |
| "Detail ✅" | 结构存在，但**宽度不可调、metadata 缺 committer/父提交、文件列表无右键菜单、非虚拟化** | `panels.rs:693-702`、`detail_view.rs:161-167,419-428` |
| "Side-by-side ✅ / Whitespace ✅" | 功能成立，但**并排无中缝、无同步滚动、diff 无独立 gutter、无横向滚动** | `diff_view.rs:149-162`；全仓无 `overflow_x` |
| "19 快捷键 ✅" | 键位存在，但**命令面板里的键位文本是硬编码字符串、与真实 `KeyBinding` 是两套来源** | `panels.rs:1387` vs `actions.rs:33-56` |

---

## 1. 35 领域对齐矩阵

状态：✅ 已对齐 ｜ 🟡 有实现但参数不符 ｜ 🟠 结构缺失/半成品 ｜ 🔴 缺失

| Area | Rebased behavior/design | Current implementation | Gap | Priority |
|------|-------------------------|------------------------|-----|----------|
| 1. Application shell | 三栏 + 工具栏 + 状态栏；**所有分栏边界可拖拽**；面板标题统一 | `app/mod.rs:548-584` 三栏骨架正确；`panels.rs:693-702` 宽度写死、**无 resizable** | 🟠 无 splitter、无拖拽光标、无宽度持久化；面板标题两套写法 | P0 |
| 2. Window chrome | IntelliJ 自绘标题栏 / 无系统装饰（New UI 可用自定义标题栏）；最小尺寸约束 | `app/mod.rs:603-608` 仅设 window_bounds + icon | 🟠 无 `TitlebarOptions`、无最小尺寸、无 Linux/Win 自绘标题栏 | P2 |
| 3. Toolbar | 高 40；图标按钮 24×24 分组，组间 1px 分隔；右侧固定动作（设置/刷新） | `toolbar.rs:66-74` 高 44、`gap_1`、`px_2`；分支/标签/远程/过滤全部为**带文字的 composite button** | 🟡 高度 44 vs 40；按钮为 text+icon 复合体，非图标工具栏；无分组分隔线（`v_separator` 仅用于 detail） | P1 |
| 4. Sidebar | 最左 40px 工具窗口图标条 + 侧栏；图标条按钮 24×24、选中蓝底、hover 淡入 | `app/mod.rs:469-545` 图标条 40px、按钮 32×32、选中 `selection_bg`、hover `fg@6%` | 🟡 按钮 32 vs 24；hover 用派生色而非 token；仅 3 项（Git/History/Shelve），缺 Rebase/Conflict 等入口 | P1 |
| 5. Commit graph | 节点**直径 8px**；单泳道内容宽 ≈22px；直角 elbow 折线（ROUND cap / BEVEL join）；区分 SIMPLE/DOUBLE(merge)/EDIT 节点 | `graph_view.rs:12-96`：节点 5px（`DOT_RADIUS=2.5`）、泳道 14px、线宽 1.5px、跨列用两段贝塞尔 S 形；**merge 无区分**（`is_merge()` 从未被引用） | 🟠 节点小 37.5%；无 merge 双节点；无 HEAD/edit 节点样式；无 elbow | P0 |
| 6. Commit rows | Log 有**表头**（Subject/Author/Date）；行高 24；日期右对齐且含年份/本地化；author 含 email 可选 | `commit_list.rs:146-213`：行高 26（实测渲染 34，因 `ListItem` 自带 `py_1` 叠加）、列 graph\|refs\|subject(flex)\|author 96px\|date 72px；date **左对齐**、格式 `%m-%d %H:%M`；**无表头** | 🟠 无表头；实测行高 34 与 theme 注释 26 不符；列宽魔法数字；date 左对齐且过窄 | P0 |
| 7. Branch labels | 本地/远程/**tag 三色互异**实色胶囊；当前分支加粗；带分支图标 | `components/mod.rs:11-34`：15% alpha 叠底、4px 圆角、`text_xs`、无图标；**本地分支与远程分支同色**（`remote_color`） | 🟠 缺 local/remote 区分；透明底非实色胶囊；无图标、无字重区分 | P0 |
| 8. Tag labels | 灰色调胶囊 + tag 图标；与分支视觉分层 | `components/mod.rs:12-13` 复用 `badge()`，色 = `lane_color(2)`；**与 `added_color()` 完全同色** | 🟡 tag 色与"新增"语义色撞色；无图标 | P1 |
| 9. HEAD indicator | HEAD 所在提交行有独立强调；HEAD 标签与分支标签语义区分 | `commit_list.rs:133-214` **行本身无任何 HEAD 高亮**；仅靠 `%D` 的 `HEAD -> x` 前缀换色；`head_id` 取 `commits.first()`（`use_cases.rs:53`）——因 log 用 `--all`，**可能不是真 HEAD** | 🔴 无 HEAD 行强调 + `head_id` 语义错误 | P0 |
| 10. Commit detail | 多分区（Commit/文件/元信息）；author+committer+父提交+签名；文件树 + 右键菜单；宽度可调 | `detail_view.rs:92-429`：宽度 389 固定、subject `text_sm`+MEDIUM、metadata 单行 `短SHA · author · 时间 · email`、扁平文件列表（非虚拟化、无右键）、15 个平铺 flex_wrap 文字按钮 | 🟠 缺 committer/父提交/merge 双父；非虚拟化、无文件右键；宽度不可调；header 区不可滚动 | P1 |
| 11. Changes panel | **目录树**（可折叠）；Space 切换包含、Enter 打开 diff；批量 stage/unstage；行高 24；状态用图标 | `toolbar.rs:1526-1582` 扁平 `Vec<Change>`、无树、无缩进；行内 checkbox + **14px 文字状态列**（A/M/D…）+ 路径 + 尾部 `+/-` 12px 列；无键盘导航、无批量操作；行高自动 | 🟠 无树、无键盘、无批量；状态为字母非图标；`−` 用 `muted` 而 `+` 用 `added_color`（不一致） | P0 |
| 12. Composer | Amend 为**复选框**；Commit / Commit-and-Push **分裂按钮** + 选项齿轮 + 消息历史下拉；编辑器可拉伸 | `toolbar.rs:1586-1644`：`Textarea` 固定 64px；按钮行 `Shelve… / Amend / Commit` 三个平铺；**Amend 是文字按钮 + 手工拼 `"✓ "`**，非 Checkbox | 🟠 无分裂按钮、无齿轮、无历史；Amend 无复选框语义/无 Space；textarea 不可拉伸 | P1 |
| 13. Diff panel | 独立 **gutter**（底色+分隔线+change bar）；横向滚动或软换行；hunk 可折叠、有 chevron；F7/Shift+F7 变更导航；工具栏含 prev/next/whitespace/settings 分组 | `diff_view.rs:164-281`：行号以 `{:>4}` 文本拼在内容同一节点内、无 gutter 底色/分隔；全仓无 `overflow_x`；hunk 头仅 `text_xs` 文本条；无折叠；内联与独立窗口**工具栏/边框/本地化不一致**（窗口 hunk 按钮硬编码英文 `diff_window.rs:167-185`） | 🟠 无 gutter、无横滚、无折叠、无导航；两 surface 不一致 | P0 |
| 14. Branch popup | 顶部 SpeedSearch 过滤框；行有分支/folder 图标；破坏项危险色；行高 24 | `toolbar.rs:76-453` 无搜索框、无图标列；Merge 三项每分支重复三行；`max_w(500px)`、行高 26 | 🟠 无 SpeedSearch、无图标列、无快捷键列、破坏项非危险色 | P1 |
| 15. Tag popup | tag 图标；Checkout Tag / Compare 等项；破坏项危险色 | `toolbar.rs:456-578` 每 tag 一个 submenu（Select/Push/Edit/Delete）；无图标、无 checked、无危险色 | 🟡 项集偏少、无图标、无危险色 | P2 |
| 16. Remote popup | 独立 Remotes 管理界面；fetch/push URL 分栏；Edit Remote | `toolbar.rs:402-450` 内联在分支菜单尾部；`name → url` 为**不可点击的 disabled label 行**；Add Remote 用两个 32px `Textarea`（占位符 `URL` 硬编码未 i18n，`app/mod.rs:97-101`） | 🟠 无独立界面、无 Edit Remote、URL 不可选、对话框控件误用 | P2 |
| 17. Context menus | **图标列 + 标签列 + 快捷键列**三栏；破坏项红色；分隔符 1px | 全部 `PopupMenuItem::on_click` → **永无快捷键列**；图标列仅在 `.checked()` 时出现；破坏项无红色；分隔符实为 `border_b(px(2.))`（`popup_menu.rs:1219`）；ref 徽章菜单用 Unicode 字形 `✕✓⇥⇄⇋⇪✎` 代替图标 | 🟠 缺两列、破坏色、分隔粗一倍、图标体系分裂 | P0 |
| 18. Dialogs | 宽度合适、有 max-height 与滚动、**Enter = 默认按钮**、打开即聚焦输入、内联校验、主按钮校验不过则禁用 | `panels.rs:1392-1822`：固定 `w(440.)`、**无 max_h/无滚动**、无 Enter（仅 `ctrl-enter` 且走全局 `CommitSelected`）、无自动聚焦、无内联错误（写 `state.error`）、主按钮不禁用；单行语义输入却统一用 64px `Textarea` | 🟠 5 项交互缺失；控件类型与语义不符 | P0 |
| 19. Rebase UI | 独立 Rebase 对话框（含 options）；交互式 todo 表有**表头 + 固定行高 + 下拉/按钮选择动作 + 插入线拖拽指示** | `panels.rs:49-203`：无对话框；面板**无 `overflow_y_scroll`**；无表头、无固定行高、无选中/焦点态；动作为"点击循环"六个状态；拖拽仅整行边框变绿、无插入线 | 🟠 不可滚动、无表头/行高、动作选择方式不同、拖拽指示不符 | P1 |
| 20. Conflict UI | 专用 Merge 工具（三窗格 + gutter）；差异高亮配色；上一处/下一处导航；键盘 | `panels.rs:205-508`：文件行**无 hover/无选中底色**（选中靠 `●` 文本前缀）；三栏卡片；`added_line_bg/deleted_line_bg/hunk_bg` token **定义但未使用**；无导航、无快捷键 | 🟠 选中态用文本、差异配色未用、无导航 | P1 |
| 21. Stash / Shelves | 独立 Shelve/Unstash 对话框；shelf 有内容预览；删除需确认；命名统一 | `panels.rs:510-573`：行**无 padding/无 hover/无圆角/无分隔**；Drop **无危险色、无确认**；命名三套混用（Shelves/Unshelve vs Stash/Unstash）；无预览 | 🟠 行样式与其它面板不一致、Drop 无确认、命名混乱 | P1 |
| 22. Reflog | 可滚动列表；列头；右键菜单 | `panels.rs:1031-1127`：**无 `overflow_y_scroll`（不可滚动）**；selector 写死 76px；无列头、无选中态、无右键 | 🟠 不可滚动 + 无列头/选中/右键 | P1 |
| 23. Blame | 左侧窄色块 gutter + 独立作者列；meta 行可交互（跳转/复制/diff） | `blame_view.rs:15-113`：整行铺 `badge_bg`（非窄 gutter）；行号靠空格串 `"{:>4}  "` 对齐；meta 行**无 hover/无右键**；`.pt_1()` 被 `.py_0p5()` 覆盖 | 🟡 无 gutter、无列对齐、meta 无交互、样式冗余 | P2 |
| 24. File history | 可滚动；列头；右键（Show Diff / Annotate） | `panels.rs:943-1028`：**无 `overflow_y_scroll`**；subject `text_xs`（与 Compare 的 `text_sm` 不一致）；无列头/选中/右键 | 🟠 不可滚动 + 字号不一致 | P1 |
| 25. Compare | 真实 diff（文件级）；方向指示；双栏 | `panels.rs:575-680`：仅"仅在 A / 仅在 B"两组提交列表，**无文件级 diff**；不可滚动；无列头/选中；inline 复制了 history/reflog 的行渲染代码 | 🟠 非 diff、不可滚动、代码重复三份 | P2 |
| 26. Search | 搜索框有可见焦点态；结果**高亮匹配文本**；结果计数 | `toolbar.rs:952-1005`：`Input::appearance(false)` → **无边框、无底色、无焦点环**（`input.rs:589-602`）；宽度字面量 200px；**不高亮匹配**、无计数 | 🟠 无焦点态、无高亮、宽度硬编码 | P1 |
| 27. Command palette | **顶部搜索框**；图标列；↑↓ 选择 + Enter 执行；分组标题 | `panels.rs:1131-1352`：**无搜索框、无图标、无键盘导航**；项键位是硬编码字符串（与 `actions.rs:33-56` 两套来源，已漂移：面板写 "Ctrl+K"、Fetch 留空但实际无绑定）；`pt(96.)`/`w(420.)` 字面量 | 🟠 缺搜索/键盘/图标；键位来源分裂 | P1 |
| 28. Status bar | 高 24-26；左侧消息，右侧固定信息组（分支/同步）；**1px 上分隔** | `toolbar.rs:1647-1736`：高 31、`border_t_1`、`text_xs`；左侧 error/busy(+进度+取消)/status 三态；右侧 repo_root(≤420px) + `⎇ branch` + `↑n`/`↓n` | 🟡 高度 31 vs 24-26；`⎇` 用 Unicode 字形非图标；repo_root 420px 字面量 | P1 |
| 29. Loading states | 骨架/进度条 + 可取消；加载中禁用交互 | `toolbar.rs:1008-1018` 纯文本"Loading repository..."；`state.busy` 文本 + `⏳` 字形；有 `CancelToken` | 🟡 无骨架/无 spinner；`⏳` 非图标；无加载态禁用 | P2 |
| 30. Error states | 内联错误条 / 通知，可关闭，带重试 | `state.error` 在状态栏单行显示（`toolbar.rs:1667-1670`）；仓库级错误在 `render_commit_panel` 显示 + Open Project（`toolbar.rs:1019-1045`）；**无关闭、无重试** | 🟡 无关闭/无重试/无堆栈展开 | P2 |
| 31. Empty states | 统一居中空态 + 图标 + 次要说明 | **两套**：`empty_state()`（居中、`text_sm`，用于 Diff/Blame/History/Reflog）vs 左对齐 `text_xs` muted 裸文本（Rebase/Conflicts/Shelve/Compare） | 🟡 两套空态 | P2 |
| 32. Hover states | 全应用同一 hover token | **三个来源并存**：`theme.list_hover=#2E3033`（List 组件）、`theme::hover_bg(fg)=fg@6%`（多数面板行）、`theme::stripe_bg(fg)=fg@4%`（blame） | 🟡 hover 底色不统一 | P1 |
| 33. Focus states | 键盘焦点环（`theme.ring`） | **几乎不存在**：仅依赖组件默认；`list_active_border` 被设为 transparent（`theme.rs:153`）→ 键盘焦点与选中不可区分；搜索框无焦点环（`appearance(false)`） | 🟠 缺焦点体系 | P0 |
| 34. Selection states | 统一选中底色 + 可选边框 | **五种**：List 组件 `selection_bg`、图标条 `selection_bg`、Compare/History/Reflog 只有 hover 无选中、Conflicts 用 `●` 文本、Shelve 无 | 🟠 选中态不统一且部分缺失 | P1 |
| 35. Keyboard interaction | 全键盘可用：列表导航、Space/Enter、菜单三栏导航、对话框 Enter/Esc、diff F7 | 19 个全局绑定（`actions.rs:33-56`）；但 Changes 列表无绑定、命令面板无键盘、对话框无 Enter、diff 无导航、独立 diff 窗口**未注册任何 keybinding（Esc 不关闭）** | 🟠 键盘覆盖不完整 | P0 |

---

## 2. 逐区域量化明细

本节记录**实际测量值**，作为实施依据。完整逐项报告见附录 A（含每个数值的 `file:line`）。

### 2.1 设计 token 现状

`theme.rs` 已定义的 token（303 行）：

| 类别 | 现有 token | 值 |
|---|---|---|
| 密度 | `ROW_HEIGHT` / `TOOLBAR_HEIGHT` / `STATUSBAR_HEIGHT` / `GROUP_HEADER_HEIGHT` | 26 / 44 / 31 / 22 |
| 布局 | `COMMIT_PANEL_WIDTH` / `ICON_STRIP_WIDTH` / `LOG_HEADER_HEIGHT` / `LOG_FILTER_HEIGHT` / `DETAIL_PANEL_WIDTH` | 344 / 40 / 41 / 37 / 389 |
| 图 | `LANE_WIDTH` / `DOT_RADIUS` / `LINE_WIDTH` | 14 / 2.5 / 1.5 |
| 圆角 | `RADIUS_SM` / `RADIUS` / `RADIUS_LG` | 2 / 4 / 6 |
| 间距 | `SPACE_XS..XL` | 2 / 4 / 8 / 12 / 16 |
| 色板 | `bg_main` `bg_chrome` `separator` `hover_solid` `selection_bg` `log_tag_bg` `text_primary` `text_muted` `link_blue` `primary_blue` | `#191A1C` `#26282C` `#26282C` `#2E3033` `#2A4371` `#233558` `#DFE1E5` `#9DA0A8` `#548AF7` `#3574F0` |

**缺口**：

- 缺 typography token（无字号/字重/字体族常量，全靠 `text_xs`/`text_sm` 框架缩放）。
- 缺 icon size token（图标尺寸散落在组件默认与 `Size::Small/XSmall`）。
- 缺 gutter / 列宽 / 菜单 / 对话框 / 空态尺寸 token。
- 缺 local/remote/tag 三分支语义色（现仅 head/tag/remote 三色，local 落进 remote）。
- `SPACE_*` 几乎未被使用（仅 `SPACE_LG` 见于 `components/mod.rs:41`），实际全部用 gpui 语义刻度 `px_1/px_2/gap_2/py_0p5`。
- `ROW_HEIGHT` 被 Changes 行忽略（仅 `graph_view.rs`/`commit_list.rs` 引用）。

### 2.2 系统性问题清单（按影响排序）

| # | 问题 | 证据 | 影响面 |
|---|---|---|---|
| S1 | 面板**不可拖拽调整宽度**，全部写死 | `panels.rs:693-702`；无 `resizable` 调用 | 全应用 |
| S2 | 菜单**无图标列、无快捷键列** | 全部 `PopupMenuItem::on_click`（`toolbar.rs:1078+`、`commit_list.rs:245+`）；图标列仅在 `.checked()` 时由组件补出 | 全部菜单 |
| S3 | 菜单破坏性操作**无危险色** | Delete/Drop/Revert/Force Push/Discard 均为普通 26px 行 | 全部菜单 |
| S4 | 对话框缺 Enter 默认键 / 自动聚焦 / 内联校验 / 禁用主按钮 / max-height+滚动 | `panels.rs:1392-1822` | 全部对话框 |
| S5 | 面板标题两套写法 | 自绘 `text_sm` muted（`panels.rs:747,908,968,1055,594`）vs `group_header` 22px `text_xs`（`panels.rs:61,215,521`） | 8 个侧栏面板 |
| S6 | 列表行样式三套 | `px_2 py_0p5 rounded hover`（`toolbar.rs:1353-1363`、`panels.rs:644,992,1084`）vs 无 padding/hover（`panels.rs:530-562`、`panels.rs:260-293`）vs checkbox 行 | 9 个面板 |
| S7 | hover 底色三来源 | `#2E3033` token vs `fg@6%`（`theme.rs:187`）vs `fg@4%`（`theme.rs:195`） | 全应用 |
| S8 | 选中态五种且部分缺失 | 见 §1 第 34 行 | 9 个面板 |
| S9 | 分隔线两套 | `theme.border #26282C`（面板）vs `fg@12%`（`components/mod.rs:37-43` v_separator） | 全应用 |
| S10 | 4 个侧栏面板**不可滚动** | Rebase(`panels.rs:49`) / Compare(`575`) / History(`943`) / Reflog(`1031`) 均无 `overflow_y_scroll`，被 `panels.rs:702 overflow_hidden` 裁剪 | 4 个面板 |
| S11 | Unicode 字形代替图标 | `✕ ✓ ⇥ ⇄ ⇋ ⇪ ✎ ● ↑ ↓ › ⎇ − + ⏳ ⇔ ≡`（`commit_list.rs:246-331`、`panels.rs:250,317,771,773`、`toolbar.rs:908,1521,1679,1716`） | 全应用 |
| S12 | 列宽/尺寸魔法数字 | `14.` `12.` `96.` `72.` `76.` `200.` `420.` `440.` `480.` `680.` `64.` `96.`(pt) | 全应用 |
| S13 | `head_id` 语义错误（`--all` 下 `commits.first()` ≠ HEAD） | `use_cases.rs:53` | Detail / 菜单 HEAD 分支 |
| S14 | 两处硬编码 `rgb(0xFFFFFF)` | `toolbar.rs:917`（Log 标签）、`app/mod.rs:527`（图标条激活） | 2 处 |
| S15 | 内联 diff 与独立 diff 窗口不一致 | 工具栏动作集、头部 chrome、hunk 按钮本地化（`diff_window.rs:167-185` 硬编码英文） | Diff |
| S16 | `theme.rs` 内部颜色体系不自洽 | `added/deleted/renamed` 复用 `lane_color(i)`，而 `modified/binary/error/success` 为 ad-hoc `hsla` 字面量 | 全应用 |

### 2.3 关键尺寸实测（与基准的差距）

| 元素 | 基准（IntelliJ New UI / rebased-source） | 现状 | 差距 |
|---|---|---|---|
| 图节点直径 | `GRAPH_NODE_WIDTH = JBUI.scale(8)` = 8px（`GitRebaseCommitsTableView.kt:55-56`） | 5px（`theme.rs:31`） | **-37.5%** |
| 图单列内容宽 | ≈22px（`2*GRAPH_NODE_WIDTH + DEFAULT_HGAP`，同文件 101-105） | 14px 泳道间距 | 偏窄，节点与间距比例倒置 |
| 线宽 | 1.5px | 1.5px | ✅ |
| 菜单行高 | 24px | 26px（`popup_menu.rs:1188`） | +2px |
| 菜单分隔符 | 1px | 2px（`popup_menu.rs:1219`） | ×2 |
| 菜单容器内边距 | — | `p_1`(4px) + `gap_y_0p5`(2px)，`min_w rems(8)`≈128px，`max_w 500px` | — |
| 列表行高（Log） | 24px | 26 + 8（`ListItem` 自带 `py_1`）= **34px 实测** | **+42%** |
| 状态栏 | 24-26px | 31px（`theme.rs:14`） | +5px |
| 工具栏 | 40px | 44px（`theme.rs:13`） | +4px |
| 图标条按钮 | 24×24 | 32×32（`app/mod.rs:533`） | +33% |
| 图标按钮（组件） | — | 32×32（`button.rs:599-601`） | — |
| 普通按钮高 | — | 32px；`compact` 时 `px_2`(8px)（`button.rs:612-615`） | — |
| 输入框（Small） | — | 高 24px、`px` 8px、`py` 2px、`text_sm`（`sizing.rs:261-268`） | — |
| 对话框宽 | — | 440px 固定 | 无 max-h |

> 注：`TOOLBAR_HEIGHT=44`、`ROW_HEIGHT=26`、`STATUSBAR_HEIGHT=31` 三值在 `theme.rs:11-14` 注释中标注为"对齐原版实测"。本次审计**没有可用的原版截图像素采样能力**（无法读取 `screenshot.png` 像素），故对这三个值**暂不单方面改写**，仅在 §4 中标为"需视觉复核"。真正可确证的结构性差距（节点 8px、菜单行高 24、列表行高实测 34、分隔符 2px）优先处理。

---

## 3. 实施计划（映射到任务阶段）

### P0（外壳与最高频表面，先做）

1. **设计系统落地**（Phase 2）：`theme/` 拆分为 `colors / typography / spacing / radius / dimensions / icons`；补齐缺失 token（gutter、列宽、菜单、对话框、空态、local/remote/tag 三色、icon size、字号字重）；`components/` 扩充为原语集（`button / icon_button / toolbar / input / search / checkbox / badge / list_row / section_header / tabs / popup / context_menu / dialog / tooltip / separator / split_pane / status_bar`）。
2. **应用外壳**（Phase 3）：所有分栏接入 `split_pane`（可拖拽 + 最小宽度）；统一面板标题为 `section_header`；统一列表行/选中/悬停/分隔线/空态；状态栏重构。
3. **菜单与对话框原语**（Phase 6）：菜单统一 24px 行高 + 1px 分隔 + 图标列 + 快捷键列 + 危险色；对话框统一宽度/内边距/标题层级的 max-height + 滚动 + Enter 默认 + 自动聚焦 + 内联校验 + 禁用主按钮。
4. **Commit Graph**（Phase 4）：节点 8px、泳道几何重算、merge 双节点、HEAD 强调、行高修正（去掉 `ListItem` 额外 `py_1`）、ref 徽章三色实色胶囊。
5. **Diff**（Phase 5）：独立 gutter（底色 + 分隔线）、横向滚动、hunk 折叠、F7/Shift+F7 导航、两 surface 统一（含本地化与工具栏）。
6. **焦点体系**（Phase 3/6）：启用 `list_active_border`，为搜索框开启可见焦点态，统一 `focus ring`。

### P1

- Changes 目录树 + 键盘（Space/Enter）+ 批量 stage；Composer 分裂按钮 + 真实 Amend Checkbox + 可拉伸编辑器；Log 表头 + 日期右对齐 + author 列宽；侧栏面板滚动一致性；命令面板搜索框 + 键盘导航 + `Kbd` 组件；会话内所有破坏性操作危险色 + 确认；Reflog/History/Compare 列头与可滚动；Rebase 面板可滚动 + 表头 + 固定行高 + 下拉动作选择；Conflict 差异配色 + 上一处/下一处。

### P2

- 窗口 chrome（自定义标题栏/最小尺寸）；Remote 独立管理界面；Tag 菜单项集补全；Blame gutter 与列对齐；Compare 真实文件级 diff；Loading 骨架；Error 可关闭/重试。

---

## 4. 架构约束遵守声明

本次改为**纯 UI 层改造**，不触碰：

- `GitBackend` trait 与 `Repository`（`src/git/`）——除为 UI 需要新增只读查询方法外不改；
- 系统 git CLI 调用链，不引入 `git2` / `gitoxide`；
- Git 领域模型（`src/git/types.rs`）不做无关改动；
- 既有 Git 功能一律保留；
- 分层保持 `UI → UseCase → Operation → GitBackend → Repository → system git`。

Phase 9 的文件拆分（`panels.rs` 1823 行 / `toolbar.rs` 1790 行 → `ui/app/{shell,toolbar/,sidebar/,graph/,changes/,detail/,diff/,branches/,rebase/,conflicts/,shelves/,reflog/,dialogs/,menus/}`）属**搬迁 + 重命名**，不改变分层与调用方向。

---

## 附录 A · 逐项实测数据来源

完整报告（含每个数值的 `file:line`）由三轮独立代码审计产出，覆盖：

- 组 A（领域 14-27）：`toolbar.rs` / `panels.rs` / `shelves.rs` / `conflicts.rs` / `rebase.rs` / `state.rs` / `blame_view.rs` / `commit_list.rs` / `components/mod.rs` / `theme.rs` / `icons.rs` + 依赖组件库 `gpui-component 0.6.1`（`menu/popup_menu.rs`、`menu/menu_item.rs`、`menu/context_menu.rs`、`button/button.rs`、`button/dropdown_button.rs`、`input/input.rs`、`sizing.rs`）与 `gpui-base/theme_tokens.rs`。
- 组 B（领域 11-13）：`toolbar.rs`（Changes/Composer 实际位于此，非 `panels.rs`）/ `diff_view.rs` / `app/diff_window.rs` / `app/detail.rs`。
- 组 C（领域 5-10）：`graph_view.rs` / `commit_list.rs` / `app/detail_view.rs` / `app/detail.rs` / `components/mod.rs` / `theme.rs` / `git/graph.rs` + `gpui-component` 的 `list_item.rs`/`list.rs` + 参考 `GitRebaseCommitsTableView.kt`。

关键量化结论摘要：

- 菜单行高 26px、内边距 `px(8.)`、容器 `p_1` + `gap_y_0p5`、`min_w rems(8)`、`max_w 500px`、分隔符 `border_b(px(2.))`、勾选图标仅在有左图标列时出现、快捷键列**仅在用 `.action()` 注册时出现**（本项目全用 `on_click`）。
- 按钮：普通 32px 高 / `px_2p5`(10px)，`compact` → `px_2`(8px)；纯图标按钮 32×32。
- 输入框 Small：24px 高、`input_px` 8px、`input_py` 2px、`text_sm`；`appearance(false)` 会同时关闭背景、边框与**焦点环**。
- 列表项：`ListItem` 自带 `py_1`(4px×2)，调用点只覆盖 `px_2` → Log 行实测 34px。
- Log 行选中：`theme.list_active`（本项目映射为 `selection_bg #2A4371`）+ `list_active_border`（本项目设为 transparent → **无焦点框**）；悬停仅在未选中时生效，用 `theme.list_hover`（`#2E3033`）。
- 图：`lane_x = 14*lane + 7`；节点为 5×5 `paint_quad` + `corner_radii(2.5)`（等效圆）；`graph_width = 14*lane_count + 5`；跨列边为两段 `curve_to` 贝塞尔；**merge 无任何特殊处理**（`Commit::is_merge()` 在 UI 层零引用）。
- Detail：宽 389、外层 `p_2`/`gap_2`/`overflow_hidden`；metadata 单行 `text_xs` muted；文件行 `px_2 py_0p5 rounded(4)` + 14px 状态列 + `flex_1` 路径 + Blame/History 图标按钮；文件列表非虚拟化；按钮区 15 项 `flex_wrap` + 3 处 `v_separator`(1×12px)。
- Diff：文件块 `rounded(6)` + `border_1` + `overflow_hidden`，头 `px_2 py_1 bg(stripe_bg)`；hunk 头 `px_2 py_0p5 text_xs bg(hunk_bg)`；行 `px_2 py(1.)`；统一视图行号 `format!("{:>4} {:>4}  ")`；并排为两个 `flex_1` 半栏**无中缝**；无 `overflow_x`；无选择/焦点态；独立窗口 1280×860、`border_b_1` 头部、无 keybinding。
