use std::sync::Arc;

use rebased_rs::git::{
    BlameGroup, Change, Commit, ConflictFile, ConflictHunk, FileDiff, HunkChoice, RebaseAction,
    StashEntry, Tag,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SidebarMode {
    Workspace,
    Detail,
    Diff,
    Blame,
    Rebase,
    Conflicts,
    Shelve,
}

#[derive(Clone)]
pub(crate) enum PromptKind {
    NewBranch { start_point: Option<String> },
    NewTag { commit_id: String },
    Stash,
    Reword { commit_id: String },
    RenameBranch,
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
    pub(crate) current_branch: Option<String>,
    pub(crate) current_upstream: Option<String>,
    pub(crate) tags: Arc<Vec<Tag>>,
    pub(crate) changes: Vec<Change>,
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
    pub(crate) status_message: String,
    pub(crate) error: Option<String>,
    pub(crate) loading: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            branches: Arc::new(Vec::new()),
            current_branch: None,
            current_upstream: None,
            tags: Arc::new(Vec::new()),
            changes: Vec::new(),
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
            status_message: "Ready".to_string(),
            error: None,
            loading: true,
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
            }],
        };
        assert_eq!(
            flow.plan_view().map(|(base, plan)| (base, plan.len())),
            Some(("origin/main", 1))
        );
    }
}
