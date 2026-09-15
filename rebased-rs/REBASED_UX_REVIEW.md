# rebased-rs · Rebased Desktop UX 对标评审（第 3 轮）

- 评审日期：2026-09-15
- 代码基线：main（本轮新增：可编辑 diff、merge 消息编辑、任意分支 rename；96 tests 全过 / clippy 干净）
- 参考对象：[DetachHead/rebased](https://github.com/DetachHead/rebased)（JetBrains IDE fork，仅保留 Git 集成）
- 评审方法：全部结论基于 rebased-rs 当前实际代码（文件 / 函数级引用），不依据项目描述；Backend 存在 ≠ 完成，必须同时核对 UI 入口、交互流程与状态反馈。
- 评审原则：**一个长期使用 Rebased 的用户第一次打开 rebased-rs，能否不经过重新学习就完成常见 Git 操作？**

---

## A. 总体完成度

| 层 | 完成度 | 依据（代码级） |
|---|---|---|
| Git Backend | ~90% | `src/git/`：log+graph / status / stage / commit / amend / branch（创建·删除·rename·checkout·本地与远程）/ tag / merge（3 模式 + `-m` 自定义消息）/ rebase（interactive todo + continue + abort）/ cherry-pick / revert / reset（3 模式）/ push（含 force）/ pull / fetch / stash / blame / file-history / worktree 文件读写 / conflict hunk 解析。缺口：远端管理（add/remove remote）、set-upstream、worktree / submodule / bisect 等低频 porcelain。 |
| Use Case 层（业务编排） | ~85% | `use_cases.rs` + `actions.rs`：统一 `run_op` 异步执行 → busy 文案 → 完成后自动刷新 → 刷新后保持当前选中；branch / author / date 三维过滤管线；compare / conflict / blame / file-history 编排完整。缺口：网络操作无进度事件、不可取消。 |
| Desktop UI | ~76% | 三栏主结构 + 工具栏 + 状态栏；commit 右键菜单 12 项；**ref 徽章右键菜单**；branch / tag / remote 下拉全操作；Changes 双分组 + 勾选 stage；Composer（Amend 预填 / Shelve）；Diff 面板（含 Edit / Save / Cancel 编辑模式）；Conflict 面板 hunk 级 Ours / Theirs / Both + 实时 Result 预览；Rebase todo 面板；9 个快捷键；6 类危险操作确认框。缺口见 P0 / P1。 |
| **Rebased UX（对标综合）** | **~65%** | 操作路径与 Rebased 同构（graph 为核心入口、右键即 Git 操作中心、branch 下拉 ≈ IntelliJ Git Branches 弹层）。差距主要在**交互密度**而非方向：~~graph 徽章不可交互~~（已补）、无并排 diff、无 3 窗格合并编辑器、进度反馈弱、快捷键覆盖面窄。 |

判断口径：Backend / Use Case 已超过"日常可用"线；与 Rebased 的真实距离在 UX 层。

---

## B. 对标矩阵

状态：✅ 基本一致 ｜ 🟡 已实现但 UX 有差距 ｜ 🟠 Backend 有、UI 不完整 ｜ 🔴 缺失

### 1. 主界面结构

| 功能 | Rebased | rebased-rs | 状态 | 差距 |
|---|---|---|---|---|
| Commit Graph 主列表 | 居中 graph + 行内徽章 | `commit_list.rs` lane_canvas 真 graph 渲染 + 三色 ref 徽章 + subject/author/time | ✅ | — |
| Branch / Tag 入口 | Git Branches / Tags 弹层 | 工具栏 Branch（含 Remote 分组）/ Tag / Filter 三个下拉 | 🟡 | 无树形分组（origin/* 未折叠） |
| Commit Detail | 右侧 meta + 父子 | `detail_view.rs` 完整时间 + author.email + 父提交导航 | ✅ | — |
| Changes / Composer / Status | 变更双分组 + 提交框 + 状态栏 | Changes 双分组 + Composer（Amend/Shelve）+ 状态栏（repo + branch + ↑↓ ahead/behind） | ✅ | — |

### 2. Commit Graph

| 功能 | Rebased | rebased-rs | 状态 | 差距 |
|---|---|---|---|---|
| Commit 选择 / 联动 | 选中刷新 Detail | 单击选中 → Detail/Diff 联动；↑↓ 键盘导航 | ✅ | — |
| 双击 = Checkout | IntelliJ 惯例 | 双击 commit 行 → `checkout_commit`（`on_click` + `click_count() >= 2`，与单击选中互不干扰） | ✅ | — |
| 右键菜单 | 12 项级 | 12 项（见 §3） | ✅ | — |
| Branch/Tag/HEAD 展示 | 行内徽章且可交互 | `ref_badge()` 三色徽章 + **右键菜单**（tag → 删除确认；本地 → Checkout(✓当前) / Push / Rename / Delete；远程 → Checkout tracking / Pull into / Compare） | ✅ | — |
| Commit 搜索 | 列表搜索 | `ListState::searchable(true)` + `filter_commits` | ✅ | — |
| Commit 过滤 | branch 过滤 | branch + author + date 三维过滤（`set_branch_filter` / `set_author_filter` / `set_date_filter`） | ✅ | 超出 Rebased 基线 |
| Graph 是操作中心？ | 是 | 是（右键 12 项 + 徽章右键 + 全部过滤入口） | ✅ | — |

### 3. Commit Context Menu（对照 Rebased 清单）

| 操作 | 状态 | 位置 |
|---|---|---|
| Checkout | ✅ | `toolbar.rs` commit 菜单 |
| Create Branch / Create Tag | ✅ | Prompt 对话框，可挂任意 commit |
| Cherry-pick / Revert | ✅ | run_op + 自动刷新 |
| Reset Current Branch to Here… | ✅ | Prompt 选模式（soft/mixed/hard） |
| Rebase Current Branch onto Here | ✅ | "Rebase from Here" |
| Copy SHA | ✅ | ClipboardItem |
| Show Diff | ✅ | "Diff" 项 → Diff 面板 |
| Reword Message… | ✅ | Prompt 预填原标题 |
| Undo Commit / Drop Commit（HEAD 限定） | ✅ | 均带确认框（UndoHeadCommit / DropHeadCommit） |
| Compare（commit 级） | 🔴 | branch 级有 Compare，commit 级无 |

### 4. Branch / Tag

| 功能 | 状态 | 差距 |
|---|---|---|
| Checkout（本地 / 远程建跟踪） | ✅ | Remote 分组 "⇥ Checkout" 直接建本地跟踪分支 |
| Create / Delete（确认） | ✅ | — |
| **Rename（任意分支）** | ✅ | 本轮新增："✎ Rename {name}…" 预填原名 → `rename_branch` |
| Merge（Default / no-ff / ff-only） | ✅ | — |
| **Merge 消息编辑** | ✅ | 本轮新增：no-ff 弹 Prompt 预填 `Merge branch 'x' into y`，空输入回退 `--no-edit` |
| Rebase onto（本地 / 远程） | ✅ | — |
| Push / Pull / Force Push（确认） | ✅ | — |
| Tracking 展示 | ✅ | 状态栏与菜单标签 ↑ahead ↓behind |
| Set upstream | 🔴 | UI 无入口（backend 亦未暴露） |
| Compare branches（本地 / 远程） | ✅ | "⇋ Compare … with {current}" |
| Tag：New on HEAD / New on commit / Delete（确认） | ✅ | — |
| Tag message 编辑 / push tags | 🟡/🔴 | 仅创建时填消息，创建后不可编辑 |

### 5. Workspace / Changes

| 功能 | 状态 | 差距 |
|---|---|---|
| Staged / Unstaged 分组 | ✅ | — |
| Stage / Unstage | ✅ | 行内 ☑ 勾选直切 |
| Untracked 文件 | ✅ | status 解析含 untracked |
| Diff / Revert（确认 DiscardChanges） | ✅ | Δ 按钮开 diff；discard 带确认 |
| Commit / Amend（预填上次消息） | ✅ | Composer `Commit ({n})` 支持多选提交 |
| Commit message 编辑 | ✅ | Textarea + Shelve 暂存 |
| Shelve（stash） | ✅ | `shelves.rs` Shelves 面板 apply / drop |
| **Hunk / 行级 stage** | 🔴 | Rebased 可在 diff 内按块暂存 |

### 6. Diff / File History / Blame

| 功能 | 状态 | 差距 |
|---|---|---|
| Commit Diff | ✅ | 只读——历史内容不可写，语义正确（`open_commit_diff` 不设 diff_path） |
| Working Tree Diff | ✅ | — |
| **可编辑文件内容** | ✅ | 本轮新增：Edit → Textarea → Save（`write_worktree_file`）/ Cancel；打开其他 diff 自动退出编辑态 |
| Staged Diff / File Diff | ✅ | — |
| File History | ✅ | 文件头按钮 + 变更行入口（`use_cases.rs::open_file_history`） |
| Blame | ✅ | blame 面板 + 点击行跳转 commit（`open_blame`） |
| Compare（分支） | ✅ | 双列 commit 对照面板 |
| 并排（side-by-side）视图 | 🔴 | 仅 unified |
| Whitespace 开关 / 行内高亮粒度 | 🟡 | 无开关 |

### 7. Rebase

| 功能 | 状态 | 差距 |
|---|---|---|
| Interactive todo 编辑 | ✅ | `panels.rs` rebase 面板：点击动作循环 Pick → Squash → Fixup → Drop → Edit → Reword（与 backend `RebaseActionKind::next()` 6 变体一致） |
| Reword | ✅ | detail 内编辑，todo 行显示 `✎ {自定义消息}`；亦为循环动作之一 |
| Edit 动作 | ✅ | 循环内含 Edit（backend + UI 循环均已覆盖） |
| 排序 | 🟡 | ↑↓ 键盘排序可用；无拖拽 |
| Rebase onto | ✅ | branch / remote / commit 右键三入口 |
| Continue / Abort | ✅ | rebase 暂停态 banner + 操作项 |
| Conflict 接入 | ✅ | 暂停态直接进 Conflict 面板 |
| Autosquash / fixup! 识别 | 🔴 | — |

**结论**：已远超"能执行 git rebase"——todo 循环、排序、reword、continue/abort、冲突接入俱全；距 Rebased 差在拖拽、Edit 循环项与 autosquash。

### 8. Merge / Conflict

| 功能 | 状态 | 差距 |
|---|---|---|
| Merge 3 模式 + 自定义消息 | ✅ | 本轮补齐消息编辑 |
| Conflict 文件列表 | ✅ | 选文件 → hunk 列表 |
| Take ours / theirs（文件级） | ✅ | 写文件 + stage 即标记 resolved |
| **Hunk 级 Ours / Theirs / Both** | ✅ | 实时 Result 预览（`result_text`），对齐 IntelliJ Merge Revisions 对话框形态 |
| Continue merge / Abort merge | ✅ | banner 常驻（merge / rebase 两种暂停态区分文案） |
| 3 窗格合并编辑器 | 🔴 | IntelliJ 核心体验，未做（列入 E 暂缓） |

### 9. Git 操作反馈

| 功能 | 状态 | 差距 |
|---|---|---|
| Loading / busy 文案 | ✅ | `run_op` 统一 busy 提示 |
| 自动刷新 + 选中保持 | ✅ | 刷新后当前 commit / 面板状态保持 |
| 错误回显 | ✅ | `state.error` 面板，操作失败 UI 状态正确恢复（busy 清除） |
| 危险操作确认 | ✅ | 6 类：ForcePush / DeleteBranch / DeleteTag / DropHeadCommit / UndoHeadCommit / DiscardChanges |
| Progress 百分比 / 取消 | 🔴 | 网络（push/pull/fetch）无进度条与取消按钮 |
| 成功反馈 | 🟡 | 静默清 busy（Rebased 亦偏静默，可接受） |

### 10. 快捷键和菜单

| 功能 | 状态 | 差距 |
|---|---|---|
| 键位绑定 | 🟡 | 9 个：esc / ctrl-enter 提交 / **ctrl-k 聚焦 Composer**（IntelliJ 提交第一入口）/ **ctrl-shift-k push** / **ctrl-t pull**（与 IntelliJ 一致）/ f5+ctrl-r 刷新 / ↑↓ 导航。缺 Alt+` VCS 弹层等第二梯队 |
| Context menu / Toolbar / Dialog | ✅ | — |
| Confirmation | ✅ | 6 类确认框全覆盖危险操作 |

---

## C. P0（最影响 Rebased 使用体验）——✅ 本轮全部完成

1. ~~**Graph 行内 ref 徽章可交互**~~ ✅ 已完成：`ref_badge()` 挂 `context_menu`（commit_list.rs），按 ref 类型区分——tag → 删除确认；本地分支 → Checkout（当前分支打 ✓）/ Push（无 upstream 自动 --set-upstream）/ Rename… / Delete…（确认框）；远程分支 → Checkout tracking / Pull into current / Compare。本地/远程判定经 `state.branch_entries` 查询，无匹配按远程降级。徽章均带唯一 element id（避免 CodeLocation 菜单 id 冲突）。
2. ~~**两个最高频肌肉记忆**~~ ✅ 已完成：Ctrl+K 聚焦 Composer（FocusComposer action + ctrl-k 绑定 + TextareaState::focus，actions.rs）；双击 commit = Checkout（行级 on_click + click_count() >= 2 → checkout_commit，与单击选中互不干扰）。

## D. P1（应继续补齐的 UX）

1. 并排 diff 视图（unified / side-by-side 切换）。
2. push / pull / fetch 的进度条与取消。
3. hunk / 行级 staging（diff 面板内选块入暂存）。
4. set upstream + remote 管理 UI（add / remove / prune）。
5. 分支下拉树形分组（origin/* 折叠）。
6. Rebase todo：拖拽排序、autosquash（Edit 已入循环 ✅）。
7. 快捷键第二梯队：Alt+` VCS 操作弹层、面板切换键、blame 打开键。
8. tag 消息编辑 / push tags。
9. 操作历史（reflog）轻量视图。

## E. 暂时不要做

1. **3 窗格合并文本编辑器**——投入大；当前 hunk 级 Ours/Theirs/Both + Result 预览已覆盖日常冲突。
2. worktree / submodule / bisect / notes / sparse-checkout UI。
3. 多仓库 dashboard、内置 terminal、LFS、hooks 编辑器、凭据管理。
4. blame 性能专项（大仓库缓存优化）——当前规模够用。

## F. 最终结论

**是——rebased-rs 已经在正确地重写 Rebased，而不是在做一个普通 Rust Git GUI。** 代码级证据：

1. **信息架构同构**：graph 中心列表（真 lane graph + 行内徽章）+ 右侧 Detail / Changes / Diff 面板 + 状态栏，对应 IntelliJ Git tool window 的骨架。
2. **"Graph 即操作中心"成立**：commit 右键 12 项覆盖 Rebased 全部清单，含 HEAD 限定的 Undo / Drop 及确认框；branch 下拉 = Git Branches 弹层（本地 / 远程分组、Checkout / Merge 3 模式 / Rebase onto / Rename / Compare / Push / Pull / Force Push / Delete）。
3. **IntelliJ 交互语法已被采纳**：Ctrl+K 提交、Ctrl+Shift+K push、Ctrl+T pull、双击 checkout、Amend 预填、Shelve、6 类危险确认、hunk 级 Ours/Theirs/Both 冲突解析、三维 commit 过滤、徽章右键菜单。
4. **P0 清零后，"零重学习完成常见操作"已成立**：commit（Ctrl+K 或 Composer）/ stage / push / pull / 建分支 / 改名 / merge（含消息）/ rebase（含 todo）/ 解决冲突 / stash / 徽章右键 Checkout / 双击 checkout，全部能在与 Rebased 相同的位置找到相同语义的入口。

剩余差距（并排 diff、3 窗格合并器、进度、快捷键广度）属于**同方向上的纵深推进**，不改变路线正确性。下一步按 P1 顺序推进即可。
