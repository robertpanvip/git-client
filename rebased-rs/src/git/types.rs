use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitId(pub String);

impl CommitId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> &str {
        &self.0[..self.0.len().min(7)]
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: CommitId,
    pub parents: Vec<CommitId>,
    pub author: Author,
    pub time: i64,
    pub subject: String,
    pub body: String,
    pub refs: Vec<String>,
}

impl Commit {
    pub fn is_root(&self) -> bool {
        self.parents.is_empty()
    }

    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

impl ChangeStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ChangeStatus::Added => "Added",
            ChangeStatus::Modified => "Modified",
            ChangeStatus::Deleted => "Deleted",
            ChangeStatus::Renamed => "Renamed",
            ChangeStatus::Copied => "Copied",
            ChangeStatus::TypeChanged => "Type changed",
            ChangeStatus::Untracked => "Untracked",
            ChangeStatus::Conflicted => "Conflicted",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            ChangeStatus::Added => "A",
            ChangeStatus::Modified => "M",
            ChangeStatus::Deleted => "D",
            ChangeStatus::Renamed => "R",
            ChangeStatus::Copied => "C",
            ChangeStatus::TypeChanged => "T",
            ChangeStatus::Untracked => "?",
            ChangeStatus::Conflicted => "U",
        }
    }

    pub fn from_letter(c: char) -> Self {
        match c {
            'A' => ChangeStatus::Added,
            'D' => ChangeStatus::Deleted,
            'R' => ChangeStatus::Renamed,
            'C' => ChangeStatus::Copied,
            'T' => ChangeStatus::TypeChanged,
            '?' | '!' => ChangeStatus::Untracked,
            'U' => ChangeStatus::Conflicted,
            _ => ChangeStatus::Modified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub status: ChangeStatus,
    pub path: String,
    pub original_path: Option<String>,
    pub staged: bool,
}

impl Change {
    pub fn display_path(&self) -> String {
        match &self.original_path {
            Some(orig) => format!("{orig} -> {}", self.path),
            None => self.path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub full_name: String,
    pub commit_id: CommitId,
    pub is_head: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
}

/// 远程仓库条目（`git remote -v` 的 name + fetch url）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

impl Branch {
    pub fn is_current(&self) -> bool {
        self.is_head && !self.is_remote
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoStatus {
    pub head_branch: String,
    pub unborn: bool,
    pub changes: Vec<Change>,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub commit_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub index: usize,
    pub ref_name: String,
    pub message: String,
}

/// `git reflog` 的一条记录（轻量操作历史）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
    /// 选择器，如 `HEAD@{0}`。
    pub selector: String,
    /// 指向的完整 commit id。
    pub commit_id: String,
    /// 短哈希，显示用。
    pub short_id: String,
    /// reflog 消息，如 `commit: fix bug`。
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Deleted,
    HunkHeader,
}

impl DiffLineKind {
    pub fn prefix(&self) -> &'static str {
        match self {
            DiffLineKind::Context => " ",
            DiffLineKind::Added => "+",
            DiffLineKind::Deleted => "-",
            DiffLineKind::HunkHeader => "@",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub content: String,
    /// 该行在源文件末尾无换行（`\ No newline at end of file`）。重建 hunk patch 时需还原，
    /// 否则 `git apply` 会改变文件末尾换行（见 diff.rs::hunk_patch）。
    pub no_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDiff {
    pub path: String,
    pub old_path: Option<String>,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_binary: bool,
    /// 真实文件模式（如 `100755` 可执行、`120000` 符号链接）。解析自 `new file mode` /
    /// `deleted file mode` / `new mode` 行；为 `None` 时 `hunk_patch` 兜底为 `100644`。
    pub mode: Option<String>,
    /// 变更前的文件模式（纯 chmod 时与 `mode` 不同）。解析自 `old mode` 行。
    pub old_mode: Option<String>,
    /// 文件级 Git 状态（与 Changes 面板同源）：diff 徽章据此取色，
    /// 未标注（`None`）时 UI 按 `is_new`/`is_deleted` 兜底推导。
    pub status: Option<ChangeStatus>,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    pub number: u32,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameGroup {
    pub commit_id: String,
    pub author: String,
    pub time: i64,
    pub filename: String,
    pub lines: Vec<BlameLine>,
}
