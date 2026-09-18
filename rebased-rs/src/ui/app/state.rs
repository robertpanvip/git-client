use std::sync::Arc;

use rebased_rs::git::{
    BlameGroup, Branch, CancelToken, Change, Commit, ConflictFile, ConflictHunk, FileDiff,
    HunkChoice, MergeMode, RebaseAction, ReflogEntry, Remote, StashEntry, Tag,
};

use crate::ui::i18n::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SidebarMode {
    Workspace,
    Detail,
    Diff,
    Blame,
    Compare,
    Rebase,
    Conflicts,
    Shelve,
    History,
    Reflog,
}

impl SidebarMode {
    /// 该面板的默认宽度（对齐原版实测值）。
    /// 面板切换时重置为该值，此后允许用户拖拽分隔条覆盖。
    pub(crate) fn default_width(self) -> f32 {
        use crate::ui::theme;
        match self {
            Self::Workspace | Self::Detail | Self::Shelve => theme::DETAIL_PANEL_WIDTH,
            Self::Rebase => theme::REBASE_PANEL_WIDTH,
            Self::Diff
            | Self::Blame
            | Self::Compare
            | Self::Conflicts
            | Self::History
            | Self::Reflog => theme::WIDE_PANEL_WIDTH,
        }
    }

    /// 该面板允许拖到的最小宽度。
    pub(crate) fn min_width(self) -> f32 {
        use crate::ui::theme;
        match self {
            Self::Workspace | Self::Detail | Self::Shelve => theme::MIN_RIGHT_PANEL_WIDTH,
            _ => theme::MIN_WIDE_PANEL_WIDTH,
        }
    }
}

/// 当前 diff 面板内容来源，决定 hunk 按钮行为：
/// Staged = `git diff --cached`（按钮为 Unstage），Unstaged = `git diff`（按钮为 Stage），
/// Commit = `git show`（只读，无按钮）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiffSource {
    Staged,
    Unstaged,
    Commit,
}

#[derive(Clone)]
pub(crate) enum PromptKind {
    NewBranch {
        start_point: Option<String>,
    },
    NewTag {
        commit_id: String,
    },
    /// 编辑已存在 tag 的 annotated 消息（git 以同 commit `-f` 重建实现）。
    EditTag {
        name: String,
        commit_id: String,
    },
    Stash,
    Reword {
        commit_id: String,
    },
    /// 把提交 squash 进其父提交；输入为空时采用 git 默认合并消息。
    Squash {
        commit_id: String,
    },
    RenameBranch,
    RenameBranchByName {
        name: String,
    },
    MergeMessage {
        name: String,
    },
    Reset {
        commit_id: String,
    },
    RebaseEdit {
        index: usize,
    },
    GoTo,
    FilterAuthor,
    /// 添加远程仓库：两个输入框分别为 remote 名字与 URL。
    AddRemote,
    /// 为指定分支设置上游：输入形如 `origin/main`。
    SetUpstream {
        branch: String,
    },
    Confirm(ConfirmAction),
}

#[derive(Clone)]
pub(crate) enum ConfirmAction {
    ForcePush,
    DeleteBranch {
        name: String,
    },
    DeleteTag {
        name: String,
    },
    RemoveRemote {
        name: String,
    },
    DropHeadCommit,
    UndoHeadCommit,
    /// 丢弃非 HEAD 提交（丢弃其更改）。
    DropCommit {
        commit_id: String,
    },
    /// 撤销非 HEAD 提交（保留其差异为暂存更改）。
    UncommitCommit {
        commit_id: String,
    },
    DiscardChanges {
        path: String,
    },
}

impl ConfirmAction {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::ForcePush => tr("Force push", "强制推送").to_string(),
            Self::DeleteBranch { .. } => tr("Delete branch", "删除分支").to_string(),
            Self::DeleteTag { .. } => tr("Delete tag", "删除标签").to_string(),
            Self::RemoveRemote { .. } => tr("Remove remote", "移除远程仓库").to_string(),
            Self::DropHeadCommit => tr("Drop HEAD commit", "丢弃 HEAD 提交").to_string(),
            Self::UndoHeadCommit => tr("Undo HEAD commit", "撤销 HEAD 提交").to_string(),
            Self::DropCommit { .. } => tr("Drop commit", "丢弃提交").to_string(),
            Self::UncommitCommit { .. } => tr("Uncommit commit", "撤销提交").to_string(),
            Self::DiscardChanges { .. } => tr("Discard changes", "丢弃更改").to_string(),
        }
    }

    pub(crate) fn hint(&self) -> String {
        match self {
            Self::ForcePush => tr(
                "This rewrites the remote branch history. Commits that only exist on the remote may be lost.",
                "这将改写远程分支历史，仅存在于远程的提交可能丢失。",
            )
            .to_string(),
            Self::DeleteBranch { name } => format!(
                "{} {name} {}",
                tr("Branch", "分支"),
                tr("will be deleted permanently.", "将被永久删除。")
            ),
            Self::DeleteTag { name } => format!(
                "{} {name} {}",
                tr("Tag", "标签"),
                tr("will be deleted permanently.", "将被永久删除。")
            ),
            Self::RemoveRemote { name } => format!(
                "{} {name} {}",
                tr("Remote", "远程仓库"),
                tr("will be removed from this repository.", "将从本仓库中移除。")
            ),
            Self::DropHeadCommit => tr(
                "The HEAD commit will be removed from history. Its changes are lost.",
                "HEAD 提交将从历史中移除，其更改将丢失。",
            )
            .to_string(),
            Self::UndoHeadCommit => tr(
                "The HEAD commit will be undone. Its changes stay staged in the working tree.",
                "HEAD 提交将被撤销，其更改保留在工作区暂存中。",
            )
            .to_string(),
            Self::DropCommit { commit_id } => format!(
                "{} {} {}",
                tr("Commit", "提交"),
                &commit_id[..commit_id.len().min(7)],
                tr("will be removed from history. Its changes are lost.", "将从历史中移除，其更改将丢失。")
            ),
            Self::UncommitCommit { commit_id } => format!(
                "{} {} {}",
                tr("Commit", "提交"),
                &commit_id[..commit_id.len().min(7)],
                tr("will be removed from history. Its changes stay staged in the working tree.", "将从历史中移除，其更改保留在工作区暂存中。")
            ),
            Self::DiscardChanges { path } => format!(
                "{} ({path}) {}",
                tr("All uncommitted changes in", "所有未提交的更改"),
                tr("will be lost.", "将丢失。")
            ),
        }
    }

    pub(crate) fn confirm_label(&self) -> String {
        match self {
            Self::ForcePush => tr("Force push", "强制推送").to_string(),
            Self::DeleteBranch { .. } | Self::DeleteTag { .. } => tr("Delete", "删除").to_string(),
            Self::RemoveRemote { .. } => tr("Remove", "移除").to_string(),
            Self::DropHeadCommit => tr("Drop", "丢弃").to_string(),
            Self::UndoHeadCommit => tr("Undo", "撤销").to_string(),
            Self::DropCommit { .. } => tr("Drop", "丢弃").to_string(),
            Self::UncommitCommit { .. } => tr("Uncommit", "撤销").to_string(),
            Self::DiscardChanges { .. } => tr("Discard", "丢弃").to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum RebaseFlow {
    #[default]
    Idle,
    Planning {
        base: String,
        plan: Vec<RebaseAction>,
    },
    Running {
        base: String,
        plan: Vec<RebaseAction>,
    },
    Stopped {
        base: String,
        plan: Vec<RebaseAction>,
        stopped_at: String,
    },
    Failed {
        base: String,
        plan: Vec<RebaseAction>,
        message: String,
    },
}

impl RebaseFlow {
    pub(crate) fn in_progress(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::Stopped { .. })
    }

    pub(crate) fn stopped_commit(&self) -> Option<&str> {
        match self {
            Self::Stopped { stopped_at, .. } => Some(stopped_at),
            _ => None,
        }
    }

    pub(crate) fn plan_view(&self) -> Option<(&str, &[RebaseAction])> {
        match self {
            Self::Planning { base, plan } | Self::Failed { base, plan, .. } => Some((base, plan)),
            _ => None,
        }
    }
}

pub(crate) struct AppState {
    pub(crate) branches: Arc<Vec<String>>,
    /// 本地 + 远程分支的完整信息（tracking/ahead/behind），供 Branches 菜单展示。
    pub(crate) branch_entries: Arc<Vec<Branch>>,
    /// 远程仓库列表（name + fetch url），供 remote 管理区展示。
    pub(crate) remotes: Arc<Vec<Remote>>,
    pub(crate) current_branch: Option<String>,
    pub(crate) current_upstream: Option<String>,
    /// 作者过滤器（空串 = 不过滤），配合 Branches 范围过滤器使用。
    pub(crate) filter_author: String,
    /// 分支范围过滤器（None = 所有分支）。
    pub(crate) filter_branch: Option<String>,
    /// 日期过滤器：(展示名, git `--since` 表达式)，None = 不限时间。
    pub(crate) filter_since: Option<(String, String)>,
    pub(crate) tags: Arc<Vec<Tag>>,
    pub(crate) changes: Vec<Change>,
    pub(crate) selected_changes: Vec<String>,
    pub(crate) repo_root: String,
    pub(crate) ahead: u32,
    pub(crate) behind: u32,
    pub(crate) amend: bool,
    pub(crate) selected: Option<Commit>,
    pub(crate) detail_files: Vec<Change>,
    pub(crate) detail_branches: Vec<String>,
    pub(crate) sidebar: SidebarMode,
    pub(crate) diff_files: Vec<FileDiff>,
    pub(crate) diff_title: String,
    pub(crate) diff_path: Option<String>,
    pub(crate) diff_editing: bool,
    /// diff 视图布局：true = 并排（side-by-side），false = 统一（unified）。
    pub(crate) diff_side_by_side: bool,
    /// 当前 diff 面板的内容来源（决定 hunk 暂存按钮行为；None = 无 diff）。
    pub(crate) diff_source: Option<DiffSource>,
    /// diff 是否忽略空白变化（`git diff -w`）。
    pub(crate) ignore_whitespace: bool,
    /// 当前 commit diff 的 commit id（切换 whitespace 开关后重载用）。
    pub(crate) diff_commit: Option<String>,
    pub(crate) blame_groups: Vec<BlameGroup>,
    pub(crate) blame_path: String,
    pub(crate) prompt: Option<PromptKind>,
    /// Stash 对话框选项：保留暂存区（--keep-index）。
    pub(crate) prompt_stash_keep_index: bool,
    /// Stash 对话框选项：包含未跟踪文件（--include-untracked）。
    pub(crate) prompt_stash_include_untracked: bool,
    /// Merge 对话框选中的快进模式。
    pub(crate) prompt_merge_mode: MergeMode,
    pub(crate) rebase: RebaseFlow,
    /// Planning 时勾选的 autosquash 开关：Start 后自动重排 fixup!/squash! 提交。
    pub(crate) rebase_autosquash: bool,
    pub(crate) merge_in_progress: bool,
    pub(crate) head_id: Option<String>,
    pub(crate) conflict_files: Vec<ConflictFile>,
    pub(crate) conflict_path: Option<String>,
    pub(crate) conflict_hunks: Vec<ConflictHunk>,
    pub(crate) conflict_choices: Vec<Option<HunkChoice>>,
    pub(crate) conflict_raw: String,
    pub(crate) shelves: Vec<StashEntry>,
    pub(crate) history_path: String,
    pub(crate) history_commits: Vec<Commit>,
    /// reflog 轻量视图的条目缓存（打开面板时刷新）。
    pub(crate) reflog_entries: Vec<ReflogEntry>,
    pub(crate) status_message: String,
    pub(crate) error: Option<String>,
    pub(crate) loading: bool,
    /// 正在后台执行的 git 操作描述（Some = 忙碌，同时防止并发写操作）。
    pub(crate) busy: Option<String>,
    /// 后台网络操作（push/pull/fetch）的最新进度文本（250ms 轮询快照）。
    pub(crate) progress_text: Option<String>,
    /// 当前可取消操作的取消令牌（Some = 状态栏显示取消按钮）。
    pub(crate) cancel_token: Option<CancelToken>,
    /// Alt+` 唤起的 VCS 操作快切弹层是否可见（与 prompt 互斥）。
    pub(crate) vcs_palette: bool,
    /// 分支对比面板：mine/theirs 分支名与两侧独有提交。
    pub(crate) compare_mine: String,
    pub(crate) compare_theirs: String,
    pub(crate) compare_ahead: Vec<Commit>,
    pub(crate) compare_behind: Vec<Commit>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            branches: Arc::new(Vec::new()),
            branch_entries: Arc::new(Vec::new()),
            remotes: Arc::new(Vec::new()),
            current_branch: None,
            current_upstream: None,
            filter_author: String::new(),
            filter_branch: None,
            filter_since: None,
            tags: Arc::new(Vec::new()),
            changes: Vec::new(),
            selected_changes: Vec::new(),
            repo_root: String::new(),
            ahead: 0,
            behind: 0,
            amend: false,
            selected: None,
            detail_files: Vec::new(),
            detail_branches: Vec::new(),
            sidebar: SidebarMode::Detail,
            diff_files: Vec::new(),
            diff_title: String::new(),
            diff_path: None,
            diff_editing: false,
            diff_side_by_side: true,
            diff_source: None,
            ignore_whitespace: false,
            diff_commit: None,
            blame_groups: Vec::new(),
            blame_path: String::new(),
            prompt: None,
            prompt_stash_keep_index: false,
            prompt_stash_include_untracked: true,
            prompt_merge_mode: MergeMode::NoFastForward,
            rebase: RebaseFlow::Idle,
            rebase_autosquash: false,
            merge_in_progress: false,
            head_id: None,
            conflict_files: Vec::new(),
            conflict_path: None,
            conflict_hunks: Vec::new(),
            conflict_choices: Vec::new(),
            conflict_raw: String::new(),
            shelves: Vec::new(),
            history_path: String::new(),
            history_commits: Vec::new(),
            reflog_entries: Vec::new(),
            status_message: tr("Ready", "就绪").to_string(),
            error: None,
            loading: true,
            busy: None,
            progress_text: None,
            cancel_token: None,
            vcs_palette: false,
            compare_mine: String::new(),
            compare_theirs: String::new(),
            compare_ahead: Vec::new(),
            compare_behind: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rebased_rs::git::RebaseActionKind;

    use super::*;

    #[test]
    fn rebase_flow_defaults_to_idle() {
        assert_eq!(AppState::default().rebase, RebaseFlow::Idle);
    }

    #[test]
    fn rebase_flow_reports_progress_and_stop() {
        let flow = RebaseFlow::Stopped {
            base: "main".to_string(),
            plan: Vec::new(),
            stopped_at: "abc1234".to_string(),
        };
        assert!(flow.in_progress());
        assert_eq!(flow.stopped_commit(), Some("abc1234"));
        assert_eq!(flow.plan_view(), None);
        assert!(!RebaseFlow::Idle.in_progress());
        assert_eq!(RebaseFlow::Idle.stopped_commit(), None);
        assert_eq!(RebaseFlow::Idle.plan_view(), None);
    }

    #[test]
    fn rebase_flow_plan_view_exposes_editable_plan() {
        let flow = RebaseFlow::Planning {
            base: "origin/main".to_string(),
            plan: vec![RebaseAction {
                id: "aaa1111".to_string(),
                subject: "add file".to_string(),
                kind: RebaseActionKind::Pick,
                message: None,
            }],
        };
        assert_eq!(
            flow.plan_view().map(|(base, plan)| (base, plan.len())),
            Some(("origin/main", 1))
        );
    }
}
