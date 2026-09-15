use std::sync::Arc;

use rebased_rs::git::{
    BlameGroup, Branch, Change, Commit, ConflictFile, ConflictHunk, FileDiff, HunkChoice,
    RebaseAction, StashEntry, Tag,
};

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
}

#[derive(Clone)]
pub(crate) enum PromptKind {
    NewBranch { start_point: Option<String> },
    NewTag { commit_id: String },
    Stash,
    Reword { commit_id: String },
    RenameBranch,
        RenameBranchByName { name: String },
        MergeMessage { name: String },
    Reset { commit_id: String },
    RebaseEdit { index: usize },
    GoTo,
    FilterAuthor,
    Confirm(ConfirmAction),
}

#[derive(Clone)]
pub(crate) enum ConfirmAction {
    ForcePush,
    DeleteBranch { name: String },
    DeleteTag { name: String },
    DropHeadCommit,
    UndoHeadCommit,
    DiscardChanges { path: String },
}

impl ConfirmAction {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::ForcePush => "Force push",
            Self::DeleteBranch { .. } => "Delete branch",
            Self::DeleteTag { .. } => "Delete tag",
            Self::DropHeadCommit => "Drop HEAD commit",
            Self::UndoHeadCommit => "Undo HEAD commit",
            Self::DiscardChanges { .. } => "Discard changes",
        }
    }

    pub(crate) fn hint(&self) -> String {
        match self {
            Self::ForcePush => "This rewrites the remote branch history. Commits that only exist on the remote may be lost.".to_string(),
            Self::DeleteBranch { name } => {
                format!("Branch {name} will be deleted permanently.")
            }
            Self::DeleteTag { name } => format!("Tag {name} will be deleted permanently."),
            Self::DropHeadCommit => "The HEAD commit will be removed from history. Its changes are lost.".to_string(),
            Self::UndoHeadCommit => {
                "The HEAD commit will be undone. Its changes stay staged in the working tree.".to_string()
            }
            Self::DiscardChanges { path } => {
                format!("All uncommitted changes in {path} will be lost.")
            }
        }
    }

    pub(crate) fn confirm_label(&self) -> &'static str {
        match self {
            Self::ForcePush => "Force push",
            Self::DeleteBranch { .. } | Self::DeleteTag { .. } => "Delete",
            Self::DropHeadCommit => "Drop",
            Self::UndoHeadCommit => "Undo",
            Self::DiscardChanges { .. } => "Discard",
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
            Self::Planning { base, plan } | Self::Failed { base, plan, .. } => {
                Some((base, plan))
            }
            _ => None,
        }
    }
}

pub(crate) struct AppState {
    pub(crate) branches: Arc<Vec<String>>,
    /// 本地 + 远程分支的完整信息（tracking/ahead/behind），供 Branches 菜单展示。
    pub(crate) branch_entries: Arc<Vec<Branch>>,
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
    pub(crate) blame_groups: Vec<BlameGroup>,
    pub(crate) blame_path: String,
    pub(crate) prompt: Option<PromptKind>,
    pub(crate) rebase: RebaseFlow,
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
    pub(crate) status_message: String,
    pub(crate) error: Option<String>,
    pub(crate) loading: bool,
    /// 正在后台执行的 git 操作描述（Some = 忙碌，同时防止并发写操作）。
    pub(crate) busy: Option<String>,
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
            sidebar: SidebarMode::Workspace,
            diff_files: Vec::new(),
            diff_title: String::new(),
            diff_path: None,
            diff_editing: false,
            blame_groups: Vec::new(),
            blame_path: String::new(),
            prompt: None,
            rebase: RebaseFlow::Idle,
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
            status_message: "Ready".to_string(),
            error: None,
            loading: true,
            busy: None,
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
