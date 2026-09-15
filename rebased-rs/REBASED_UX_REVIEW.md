# Rebased Desktop UX 对标评审报告

> 评审日期：2026-09-15
> 评审方法：基于 rebased-rs 全部 36 个源码文件的实测走查（UI 层逐行读取 + 全仓库调用点 grep）。
> 参考对象：[DetachHead/rebased](https://github.com/DetachHead/rebased)——它是 JetBrains/intellij-community 的 fork（README 原文："It's basically just a JetBrains IDE with all the bundled plugins removed except the git integration"），因此其 UX 基线 = IntelliJ 平台 Git 工具链（vcs-log + git4idea）的交互习惯，外加一条 rebased 独有布局决策：**Git log/graph 默认位于主编辑器窗口**。
> 所有 rebased-rs 结论均来自当前代码实测，未采信 README。

---

## A. 总体完成度

| 层 | 完成度 | 依据 |
|---|---|---|
| **Git Backend** | ~90% | `git/backend.rs` 定义 60+ 个 trait 方法，覆盖日常操作全链路（log/status/stage/commit+paths/amend、分支 CRUD、tag CRUD、stash、cherry-pick/revert/reset_to(Soft/Mixed/Hard)、4 类 diff、blame、interactive rebase 状态机、merge（三档 ff 策略）+冲突解析、reword_commit、undo/drop head、branch compare、repo_digest、log --follow）。缺口：remote branches。 |
| **Desktop UI**（作为普通 Git GUI） | ~50% | 单线程主流程可走通，但零快捷键、零右键菜单、危险操作零确认、同步执行无进度、部分提交不可用。 |
| **Rebased UX**（对标 IntelliJ Git 交互习惯） | ~25% | 布局骨架"像"（graph 主区/右侧 detail/顶部 toolbar），但交互范式相反：Rebased 是"右键菜单 + 快捷键 + Branches 弹窗 + 操作后状态保持"驱动；rebased-rs 是"选中→右侧按钮列表 + 每次刷新清场"驱动。 |

**三层差距定位**：Backend > UseCase > UI，越靠近用户差距越大。Backend 有 7 个方法在 UI 零调用（commit_paths、reset_to、diff_staged、diff_unstaged、stage_file、root、current_branch_name），且有 1 个工作流断裂 bug（P0-5）。

---

## B. 对标矩阵

状态图例：✅ 基本一致 / 🟡 已实现但 UX 有差距 / 🟠 Backend 有，UI 不完整 / 🔴 缺失

### 1. 主界面结构

| 功能 | Rebased (IntelliJ 基线) | rebased-rs | 状态 | 差距 |
|---|---|---|---|---|
| Graph 主区位置 | log 默认在主编辑器窗口 | graph 占主区，右侧 sidebar | ✅ | 一致，正确理解了 Rebased 布局哲学 |
| Commit Detail | 选中即展示 author+date+完整 message+文件 | 有，含 branches_containing | 🟡 | message 全文/日期展示较弱 |
| Changes 面板 | staged/unstaged 分组 | 单列表混排，仅靠行尾图标区分 | 🟡 | 无分组标题 |
| Diff | 独立 diff viewer，可编辑 | sidebar 内只读 unified 渲染 | 🟡 | 只读可接受，无 side-by-side |
| Blame | 可点击行号看 commit | 只读渲染，不可点击 | 🟡 | 无法从 blame 跳 commit |
| Commit Composer | 勾选文件+消息框+Commit 下拉 | 底部 composer：消息框+Shelve+Amend+Commit | 🟡 | 缺文件勾选、Amend 不沿用原消息 |
| Status | 状态栏含当前分支名 | 仅 error/status + ahead/behind | 🟠 | `current_branch_name()`/`root()` backend 有、UI 未用 |
| Toolbar | IntelliJ Git 菜单 + VCS 操作组 | Branch/Tags dropdown + Fetch/Pull/Push/Stash 等 | ✅ | 平铺按钮 vs 菜单，属可接受差异 |

### 2. Commit Graph

| 功能 | Rebased | rebased-rs | 状态 | 差距 |
|---|---|---|---|---|
| Graph 绘制 | IntelliJ graph lane | canvas 曲线+着色 | ✅ | — |
| Commit 选择 | 单击选中保持 | 单击选中+加载 Detail | ✅ | — |
| Commit 双击 | 无特殊语义 | Confirm 同样只加载 Detail | ✅ | — |
| 右键菜单 | **核心操作入口** | 完全没有 | 🔴 | 与 Rebased 最大的单一交互差距 |
| Branch/Tag 徽章 | ref 徽章 | ref_badge 着色徽章 | ✅ | — |
| HEAD 展示 | ✓ | ✓ | ✅ | — |
| Graph↔Detail 联动 | 选中即更新 | ✓ | ✅ | — |
| Commit 搜索 | 过滤框+正则 | perform_search 匹配 subject/author/hash | ✅ | 基本一致 |
| Commit 过滤 | branch/user/date filter | 仅搜索子串 | 🔴 | 无结构化过滤器 |
| 是否操作核心入口 | 是（右键直达） | 否，"日志展示 + 中转站" | 🔴 | — |

### 3. Commit Context Menu

| 操作 | rebased-rs 现状 | 状态 |
|---|---|---|
| Checkout | Detail 按钮 | ✅ |
| Create Branch | Detail "Branch…" | ✅ |
| Create Tag | Detail "Tag…" | ✅ |
| Cherry-pick | Detail 按钮 | ✅ |
| Revert | Detail 按钮 | ✅ |
| Reset Current Branch to Here | backend 完整，UI 零调用 | 🟠 |
| Rebase onto Here | Detail "Rebase from here"→计划面板 | 🟡 |
| Interactive Rebase | 同上 | 🟡（缺 Reword） |
| Compare with Branch | ✓ Branches 菜单 ⇋ Compare（P2） | ✅ |
| Copy SHA | ✓ | ✅ |
| Show Diff | ✓ | ✅ |
| Undo/Drop Commit | ✓（无确认） | 🟡 |
| Reword | ✓ | ✅ |

结论：菜单项功能覆盖约 75%，但入口形态完全不同（选中→右侧按钮 vs 右键直达）。

### 4. Branch / Tag

| 功能 | rebased-rs 现状 | 状态 |
|---|---|---|
| Branch selector | toolbar dropdown，本地分支+●当前 | 🟡 |
| Checkout / Create | ✓ | ✅ |
| Rename | 仅当前分支 | 🟡 |
| Delete | ✓（无确认） | 🟡 |
| Merge | ✓ ff 三档（默认/no-ff/ff-only）（P2）；无消息编辑 | ✅ |
| Rebase onto | 分支菜单无 | 🟠 |
| Push / Pull | ✓ | ✅ |
| Force Push | ✓（无确认） | 🟡 |
| Tracking / ahead-behind | ahead/behind ✓；remote 分支不加载 | 🟡 |
| Compare branches | ✓ ahead/behind 双列表面板（P2） | ✅ |
| Tag 操作 | 新建/删除/跳转/Push all tags | 🟡 |

### 5. Workspace / Changes

| 功能 | rebased-rs 现状 | 状态 |
|---|---|---|
| Staged/Unstaged 展示 | 单列表混排 | 🟡 |
| Stage/Unstage | 整行点击 toggle | 🟡 |
| 部分文件提交 | commit_paths 完整，UI 无勾选 | 🟠 |
| Diff | Δ 恒走 diff_head，不按 staged 分流 | 🟠 |
| Revert (discard) / Untracked 删除 | ✓ | ✅ |
| Commit / Amend | ✓；Amend 须重输完整消息 | 🟡 |
| Shelve | ✓ | ✅ |

### 6. Diff / File History / Blame

| 功能 | 状态 | 说明 |
|---|---|---|
| Commit Diff | ✅ | show_diff |
| Working Tree Diff | ✅ | diff_head |
| Staged Diff | 🟠 | `diff_staged` 零调用 |
| Unstaged Diff | 🟠 | `diff_unstaged` 零调用 |
| File Diff | 🟡 | 有但不分流 |
| File History | 🔴 | backend 与 UI 均无 |
| Blame | 🟡 | 只读、不能跳 commit |
| Compare | ✅ | ahead/behind 双列表面板（P2） |

### 7. Rebase

| 功能 | rebased-rs 现状 | 状态 |
|---|---|---|
| Interactive 计划面板 | ✓ | ✅ |
| Pick / Edit / Squash / Fixup / Drop | ✓ | ✅ |
| Reword | 枚举中无（单 commit reword 另有路径） | 🟠 |
| 排序 | ↑↓ swap | 🟡 |
| Rebase onto | 仅 Detail 入口 | 🟡 |
| Continue / Abort | ✓ | ✅ |
| Conflict 状态 | Stopped 态+提示 | 🟡 |
| 评价 | 已超出"能执行 git rebase"，是全项目最接近 Rebased 体验的部分；交互密度约为 IntelliJ Interactive Rebase 的 70% | 🟡 |

### 8. Merge / Conflict

| 功能 | rebased-rs 现状 | 状态 |
|---|---|---|
| Merge | ✓ | 🟡 |
| Conflict files / Ours / Theirs / Hunk | ✓ | ✅/🟠 |
| Mark resolved / Apply | ✓ | 🟠 **resolve 后不 stage_file** |
| Continue merge | ✓ | 🟠 **未 stage 导致 continue 失败——工作流断裂** |
| Abort merge | ✓ | ✅ |
| 与 IntelliJ 差距 | 三栏合并对话框 vs 纯文本+按钮 | 🟡 |

### 9. Git 操作反馈

| 功能 | rebased-rs 现状 | 状态 |
|---|---|---|
| Loading | ✓ | 🟡 |
| Progress | 无，同步执行会冻结 UI | 🟠 |
| Success/Error | statusbar 灰/红字 | ✅ |
| 自动刷新 | ✓ 5s 轻量指纹检测，变化才全量 refresh（P2） | ✅ |
| 刷新后选中保持 | **不保持，reset_views 清场** | 🔴 |
| 失败恢复 | 基本正确 | 🟡 |

### 10. 快捷键和菜单

| 功能 | rebased-rs 现状 | 状态 |
|---|---|---|
| Keyboard shortcut | 全仓库零绑定 | 🔴 |
| Context menu | 无任何右键菜单 | 🔴 |
| Toolbar | ✓ | ✅ |
| Dialog | 5 种 Prompt 输入框 | 🟡 |
| Confirmation | 完全没有（Force push/Delete/Drop/Undo/discard 直发） | 🔴 |

---

## C. P0 —— 最影响 Rebased 使用体验的缺口

1. **Commit Graph 右键菜单**：Rebased 用户大部分 git 操作从 log 右键发起；当前必须"选中→右侧按钮"。
2. **操作后状态清场**：commit/push 一次，选中 commit、打开的 diff、sidebar 全部重置（`reset_views`）。
3. **危险操作零确认**：Force push、Delete branch、Delete tag、Drop/Undo Commit、discard 全部直接执行。
4. **部分文件提交不可用**：`commit_paths` 完整但 UI 无勾选。
5. **冲突解决工作流断裂**：resolve/take side 后不 `stage_file`，Continue Merge/Rebase 会失败。
6. **Reset Current Branch to Here 缺失**：backend `reset_to` 完整，仅缺 UI。
7. **Staged Diff 入口错位**：Δ 按钮不按 staged 分流，所见与所提交内容可能不一致。

## D. P1 —— 应继续补齐的 UX

1. 快捷键体系（commit/push/pull/刷新/Esc/↑↓）。
2. Branches 弹窗 + remote 分支 + tracking + Pull into / Rebase onto / Compare。
3. Rebase 计划加 Reword + squash 消息编辑。
4. Staged/Unstaged 分组 + IntelliJ 式勾选框。
5. 结构化过滤器（branch/user）+ Go to Hash/Branch/Tag。
6. 操作异步化 + 进度，避免 UI 冻结。
7. Amend 沿用原消息。
8. File History（backend `log --follow -- path`）。
9. Blame → commit 跳转、Detail 补全 message 全文与日期。
10. 三栏式 conflict 对话框。

## E. 暂时不要做

- 多仓库/worktree/submodule/LFS/bisect/notes；
- 可编辑 diff（在 diff 视图直接改代码）；
- Changelist 体系（用 stage 模拟部分提交即可）；
- Clone/远端管理、SSH 凭证管理；
- 插件/主题/设置系统；
- gravatar、邮件集成、GPG 签名 UI。

## F. 最终结论

**方向上：是正确地重写 Rebased；完成度上：还是"正确路线的前 30%"。**

不是普通 Rust Git GUI 的证据：graph 为第一公民的主区布局（rebased 独有决策）、Shelve 概念、Interactive Rebase 计划态/停止态状态机、hunk 粒度冲突解决、干净的三层架构。

尚未达到"Rebased 用户零学习"的证据：零右键菜单、零快捷键、零确认框、刷新清场、7 个 backend 方法悬空、冲突不 stage 的流程断裂。

**一句话**：骨架是对的，但目前是"一个理解 Rebased 的普通 Rust Git GUI"，尚未成为"Rebased 的重写"。P0 清单完成后这个判断才会反转——且 P0 六项全部不需要新增 backend 能力，纯粹是把已有能力接到符合 Rebased 直觉的入口上。

---

## 修复记录（评审后跟进）

- [x] P0-5 冲突解决后 `stage_file`（`use_cases::apply_conflict_resolutions` / `take_conflict_side`）。**如实修正**：评审称"resolve 后不 stage_file、Continue 失败"，但代码审计发现底层 `conflict::checkout_side` 与 `repo::resolve_conflict_markers` 均已含 `git add -- <path>`，工作流并未断裂。为把"解决冲突后必须 stage"的业务不变量从底层实现细节提升到应用层，仍在 use_cases 层显式调用 `stage_file`（幂等、防御式），并新增回归测试 `conflict_side_resolution_stages_file` 锁定。
- [x] P0-4 部分文件提交：变更行勾选 + `commit_paths`（`selected_changes`）
- [x] P0-7 Δ 按 staged/unstaged 分流（`open_staged_diff` / `open_unstaged_diff`）
- [x] P0-6 Reset Current Branch to Here：`PromptKind::Reset` + Soft/Mixed/Hard 对话框
- [x] P0-2 刷新状态保持：refresh 保留选中 commit / Detail / Diff / Blame
- [x] P0-3 危险操作确认对话框（ConfirmPrompt）
- [x] P0-1 Commit 右键菜单（log 行 context menu）
- [x] statusbar 显示仓库路径 + 当前分支名

### P1 修复记录

- [x] P1-8 File History：后端 `log_follow`（`git log --follow -- path`，`Repository::log_follow`）+ `SidebarMode::History` 面板 + Detail 文件行的 **H** 入口 + 测试 `log_follow_args_scoped_to_path`。
- [x] P1-7 Amend 沿用原消息：`Amend` 开启时经 `head_message()`（`git log -1 --format=%B`）预填原提交消息到输入框（`prefill_amend_message`）。
- [x] P1-9 Detail 补全：meta 行补全完整日期（`%Y-%m-%d %H:%M:%S`，`format_full_time`）与作者 email。
- [x] P1-9 Blame → commit 跳转：`render_blame` 接受 `BlameJump` 回调，点击提交元信息行跳转到该提交的 diff。
- [x] P1-4 Staged/Unstaged 分组：变更列表按 `change.staged` 分组渲染小节标题（Unstaged / Staged）。
- [x] P1-1 快捷键体系：Esc 关闭 overlay、Ctrl+Enter 提交、Ctrl+Shift+K Stash、Ctrl+T 新标签、Ctrl+R 刷新、↑↓ 日志导航（`actions.rs` register_keybindings）。
- [x] P1-2 Branches 弹窗：菜单基于完整 `Branch` 信息（`branch_entries`）——本地分支勾选标记 + tracking 文案（`name → upstream ↑ahead ↓behind`）、每个非当前本地分支 Merge into / Rebase onto… / Delete、Remote 分组 Checkout / Pull into current（fetch+merge）/ Rebase onto…。
- [x] P1-3 Rebase 计划 Reword/squash 消息编辑：`PromptKind::RebaseEdit` + `open_rebase_edit`（预填原 subject/message），确认后写入计划项 `RebaseActionKind::Reword` 的自定义消息。
- [x] P1-5 结构化过滤器 + Go to：`log_args(limit, from, author)` 支持 `--author=` 与分支范围（替代 `--all`）；toolbar 新增 Branch 范围下拉（◫）+ 作者过滤（👤）+ `→ Go to…`（`rev_parse --verify <rev>^{commit}` 解析 hash/branch/tag 并选中）。
- [x] P1-6 操作异步化 + 进度：`run_op` / `refresh` / 启动加载 / `start_rebase` / `apply_rebase` 全部改为 `background_spawn` + `cx.spawn` 回主线程；`AppState.busy` 防并发写操作并在状态栏显示 `⏳ …` 进度。
- [x] P1-10 三栏式 conflict 对话框：conflict 面板三栏呈现 Ours / Base / Theirs（`ConflictHunk` 三方内容），逐 hunk 选择 Take Ours / Take Theirs / Both。

### P2 修复记录

- [x] P2-1 Compare with Branch：后端 `Repository::compare_branches`（复用 `log_filtered` 的 range 语法：`theirs..mine` = ahead / `mine..theirs` = behind）+ trait/impl 双委托 + `AppState` 字段存 mine/theirs/ahead/behind（`SidebarMode` 保持 `Copy`，仅加 unit 变体 `Compare`）+ `render_compare_panel` 双列表（每列标注 `{branch} only (n)`，提交行可点击跳转）+ Branches 菜单本地与远程分支的 ⇋ Compare 入口（`open_branch_compare` 异步执行）。测试 `compare_branches_reports_ahead_and_behind`。
- [x] P2-2 Merge ff 选项：`git/merge.rs` 引入 `MergeMode` 三档（Default=`--no-edit` / NoFastForward=`--no-ff --no-edit` / FastForwardOnly=`--ff-only`），`merge_branch_with` 全链路传递；Branches 菜单 Merge 单项扩为三档（`⇄ Merge {name} into {target}` / `(no ff)` / `(ff only)`）；顺带修复 P1-6 漏网的 `merge_branch_into_current` 同步调用，改为 `run_op` 异步路径。测试 `merge_branch_with_ff_only_fast_forwards_without_merge_commit` / `merge_branch_with_no_ff_creates_merge_commit`。
- [x] P2-3 自动刷新：`Repository::repo_digest` 轻量指纹（`rev-parse HEAD` + `status --porcelain` 行数）+ `AppView.repo_digest` 字段 + 5s 周期后台循环——指纹变化才全量 `refresh`，busy/loading 期间跳过检测，首次检测只记基准不触发刷新，entity 释放后退出循环。测试 `repo_digest_reflects_head_and_worktree`。
- 新增 4 个集成测试（TempRepo 真实 git 仓库），共 92 个测试全部通过，clippy 无警告。
