use super::command::GitCommand;
use super::error::{GitError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    BothModified,
    BothAdded,
    BothDeleted,
    AddedByUs,
    AddedByThem,
    DeletedByUs,
    DeletedByThem,
}

impl ConflictKind {
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "UU" => Some(ConflictKind::BothModified),
            "AA" => Some(ConflictKind::BothAdded),
            "DD" => Some(ConflictKind::BothDeleted),
            "AU" => Some(ConflictKind::AddedByUs),
            "UA" => Some(ConflictKind::AddedByThem),
            "DU" => Some(ConflictKind::DeletedByUs),
            "UD" => Some(ConflictKind::DeletedByThem),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ConflictKind::BothModified => "Both modified",
            ConflictKind::BothAdded => "Both added",
            ConflictKind::BothDeleted => "Both deleted",
            ConflictKind::AddedByUs => "Added by us",
            ConflictKind::AddedByThem => "Added by them",
            ConflictKind::DeletedByUs => "Deleted by us",
            ConflictKind::DeletedByThem => "Deleted by them",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictFile {
    pub path: String,
    pub kind: ConflictKind,
}

pub fn conflicted_files(cmd: &GitCommand) -> Result<Vec<ConflictFile>> {
    let output = cmd.execute(&["status", "--porcelain"])?;
    if !output.success {
        return Err(GitError::with_stderr("git status failed", output.stderr));
    }
    Ok(parse_conflicted_status(&output.stdout))
}

pub fn parse_conflicted_status(stdout: &str) -> Vec<ConflictFile> {
    stdout
        .lines()
        .filter_map(|line| {
            // 前两个字符是 XY 状态码（ASCII），其后是空格与路径；用字符切片避免多字节
            // 首字符导致 `line[..2]`/`line[3..]` 越界 panic。
            let code: String = line.chars().take(2).collect();
            let kind = ConflictKind::from_code(&code)?;
            let path: String = line.chars().skip(3).collect();
            let path = path.trim().to_string();
            if path.is_empty() {
                return None;
            }
            Some(ConflictFile { path, kind })
        })
        .collect()
}

pub fn stage_file(cmd: &GitCommand, path: &str) -> Result<()> {
    cmd.run_ok(&["add", "--", path])
}

pub fn checkout_side(cmd: &GitCommand, path: &str, ours: bool) -> Result<()> {
    let side = if ours { "--ours" } else { "--theirs" };
    cmd.run_ok(&["checkout", side, "--", path])?;
    cmd.run_ok(&["add", "--", path])
}

pub fn version_content(cmd: &GitCommand, stage: u8, path: &str) -> Result<String> {
    let rev = format!(":{stage}:{path}");
    let output = cmd.execute(&["show", &rev])?;
    if !output.success {
        return Err(GitError::with_stderr(
            format!("git show {rev} failed"),
            output.stderr,
        ));
    }
    Ok(output.stdout)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkChoice {
    Ours,
    Theirs,
    Both,
}

impl HunkChoice {
    pub fn label(self) -> &'static str {
        match self {
            HunkChoice::Ours => "Use ours",
            HunkChoice::Theirs => "Use theirs",
            HunkChoice::Both => "Use both",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictHunk {
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictSection {
    Clean(String),
    Hunk(ConflictHunk),
}

pub fn parse_conflict_markers(content: &str) -> Vec<ConflictSection> {
    let mut sections: Vec<ConflictSection> = Vec::new();
    let mut clean: Vec<String> = Vec::new();
    let mut ours: Vec<String> = Vec::new();
    let mut theirs: Vec<String> = Vec::new();
    let mut state = 0u8;
    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            state = 1;
        } else if line.starts_with("=======") && state == 1 {
            state = 2;
        } else if line.starts_with(">>>>>>>") && state == 2 {
            if !clean.is_empty() {
                sections.push(ConflictSection::Clean(clean.join("\n")));
                clean.clear();
            }
            sections.push(ConflictSection::Hunk(ConflictHunk {
                ours: std::mem::take(&mut ours),
                theirs: std::mem::take(&mut theirs),
            }));
            state = 0;
        } else {
            match state {
                0 => clean.push(line.to_string()),
                1 => ours.push(line.to_string()),
                _ => theirs.push(line.to_string()),
            }
        }
    }
    if !clean.is_empty() {
        sections.push(ConflictSection::Clean(clean.join("\n")));
    }
    sections
}

pub fn conflict_hunks(content: &str) -> Vec<ConflictHunk> {
    parse_conflict_markers(content)
        .into_iter()
        .filter_map(|section| match section {
            ConflictSection::Hunk(hunk) => Some(hunk),
            ConflictSection::Clean(_) => None,
        })
        .collect()
}

pub fn resolve_markers(content: &str, choices: &[HunkChoice]) -> Option<String> {
    let sections = parse_conflict_markers(content);
    let mut index = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for section in sections {
        match section {
            ConflictSection::Clean(text) => {
                // 逐行还原干净内容，确保冲突块之间的空行（如 `>>>>>>> b` 与 `<<<<<<< c`
                // 之间的分隔空行）不被 `join` 吞掉。
                for l in text.split('\n') {
                    lines.push(l.to_string());
                }
            }
            ConflictSection::Hunk(hunk) => {
                let choice = choices.get(index)?;
                match choice {
                    HunkChoice::Ours => lines.extend(hunk.ours.iter().cloned()),
                    HunkChoice::Theirs => lines.extend(hunk.theirs.iter().cloned()),
                    HunkChoice::Both => {
                        lines.extend(hunk.ours.iter().cloned());
                        lines.extend(hunk.theirs.iter().cloned());
                    }
                }
                index += 1;
            }
        }
    }
    if index != choices.len() {
        return None;
    }
    let mut text = lines.join("\n");
    if content.ends_with('\n') && !text.is_empty() {
        text.push('\n');
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        "top\n<<<<<<< HEAD\nours one\nours two\n=======\ntheirs one\n>>>>>>> branch\nbottom\n";

    #[test]
    fn parse_conflicted_status_codes() {
        let out = "UU a.txt\nAA b.txt\nDU c.txt\nM  d.txt\n?? e.txt\n";
        let files = parse_conflicted_status(out);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "a.txt");
        assert_eq!(files[0].kind, ConflictKind::BothModified);
        assert_eq!(files[1].kind, ConflictKind::BothAdded);
        assert_eq!(files[2].kind, ConflictKind::DeletedByUs);
    }

    #[test]
    fn parse_markers_basic() {
        let sections = parse_conflict_markers(SAMPLE);
        assert_eq!(sections.len(), 3);
        match &sections[0] {
            ConflictSection::Clean(text) => assert_eq!(text, "top"),
            _ => panic!("expected clean"),
        }
        match &sections[1] {
            ConflictSection::Hunk(hunk) => {
                assert_eq!(hunk.ours, vec!["ours one", "ours two"]);
                assert_eq!(hunk.theirs, vec!["theirs one"]);
            }
            _ => panic!("expected hunk"),
        }
        match &sections[2] {
            ConflictSection::Clean(text) => assert_eq!(text, "bottom"),
            _ => panic!("expected clean"),
        }
    }

    #[test]
    fn resolve_markers_applies_choices() {
        let choices = [HunkChoice::Theirs];
        let resolved = resolve_markers(SAMPLE, &choices).expect("resolved");
        assert_eq!(resolved, "top\ntheirs one\nbottom\n");

        let choices = [HunkChoice::Both];
        let resolved = resolve_markers(SAMPLE, &choices).expect("resolved");
        assert_eq!(resolved, "top\nours one\nours two\ntheirs one\nbottom\n");
    }

    #[test]
    fn resolve_markers_rejects_mismatch() {
        assert!(resolve_markers(SAMPLE, &[]).is_none());
        assert!(resolve_markers(SAMPLE, &[HunkChoice::Ours, HunkChoice::Ours]).is_none());
    }

    #[test]
    fn parse_markers_no_conflicts() {
        let sections = parse_conflict_markers("just\nplain\n");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0], ConflictSection::Clean("just\nplain".into()));
    }

    #[test]
    fn resolve_markers_keeps_blank_line_between_hunks() {
        // 两个相邻冲突块之间若有分隔空行，resolve 后必须保留该空行，
        // 不能被 `join` 吞掉（见 resolve_markers 的逐行重建）。
        let content = "\
<<<<<<< HEAD
A
=======
B
>>>>>>> branch1

<<<<<<< HEAD
C
=======
D
>>>>>>> branch2
";
        let choices = [HunkChoice::Ours, HunkChoice::Ours];
        let resolved = resolve_markers(content, &choices).expect("resolved");
        assert_eq!(resolved, "A\n\nC\n");
    }
}
