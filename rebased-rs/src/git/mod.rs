pub mod backend;
pub mod blame;
pub mod branches;
pub mod command;
pub mod conflict;
pub mod diff;
pub mod error;
pub mod graph;
pub mod log;
pub mod merge;
pub mod ops;
pub mod rebase;
pub mod repo;
pub mod repo_data;
pub mod status;
pub mod types;

pub use backend::{GitBackend, open_backend};
pub use blame::parse_blame;
pub use command::{CancelToken, GitCommand, ProgressHandle};
pub use conflict::{
    ConflictFile, ConflictHunk, ConflictKind, ConflictSection, HunkChoice, conflict_hunks,
    parse_conflict_markers, resolve_markers,
};
pub use diff::{hunk_patch, parse_unified_diff};
pub use error::{GitError, Result};
pub use graph::{Graph, GraphRow, MAX_COLORS, RowEdge, build_graph};
pub use merge::MergeMode;
pub use ops::ResetMode;
pub use rebase::{RebaseAction, RebaseActionKind, autosquash_plan};
pub use repo::Repository;
pub use repo_data::{
    DEFAULT_LOG_LIMIT, RepoData, filter_commits, load_repo_data, load_repo_data_filtered,
};
pub use types::{
    Author, BlameGroup, BlameLine, Branch, Change, ChangeStatus, Commit, CommitId, DiffLine,
    DiffLineKind, FileDiff, Hunk, ReflogEntry, Remote, RepoStatus, StashEntry, Tag,
};
