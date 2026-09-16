use super::types::{Change, ChangeStatus, RepoStatus};

pub const STATUS_ARGS: [&str; 4] = ["status", "--porcelain", "--branch", "--untracked-files=all"];

fn status_code(x: char, y: char) -> Option<(ChangeStatus, bool)> {
    match x {
        'A' => Some((ChangeStatus::Added, true)),
        'M' => Some((ChangeStatus::Modified, true)),
        'D' => Some((ChangeStatus::Deleted, true)),
        'R' => Some((ChangeStatus::Renamed, true)),
        'C' => Some((ChangeStatus::Copied, true)),
        'T' => Some((ChangeStatus::TypeChanged, true)),
        'U' => Some((ChangeStatus::Conflicted, true)),
        _ => match y {
            'M' => Some((ChangeStatus::Modified, false)),
            'D' => Some((ChangeStatus::Deleted, false)),
            'T' => Some((ChangeStatus::TypeChanged, false)),
            'A' => Some((ChangeStatus::Added, false)),
            _ => None,
        },
    }
}

pub fn parse_status(output: &str) -> RepoStatus {
    let mut status = RepoStatus::default();
    for line in output.lines() {
        if let Some(branch_line) = line.strip_prefix("## ") {
            parse_branch_line(branch_line, &mut status);
            continue;
        }
        let bytes = line.as_bytes();
        if bytes.len() < 4 {
            continue;
        }
        let x = line[..1].chars().next().unwrap_or(' ');
        let y = line[1..2].chars().next().unwrap_or(' ');
        if x == '?' && y == '?' {
            status.changes.push(Change {
                status: ChangeStatus::Untracked,
                path: line[3..].to_string(),
                original_path: None,
                staged: false,
            });
            continue;
        }
        if let Some((code, staged)) = status_code(x, y) {
            let rest = &line[3..];
            let (original_path, path) = match rest.split_once(" -> ") {
                Some((orig, new)) => (Some(orig.to_string()), new.to_string()),
                None => (None, rest.to_string()),
            };
            status.changes.push(Change {
                status: code,
                path,
                original_path,
                staged,
            });
        }
    }
    status
}

fn parse_branch_line(line: &str, status: &mut RepoStatus) {
    if let Some(rest) = line.strip_prefix("No commits yet on ") {
        status.head_branch = rest.trim().to_string();
        status.unborn = true;
        return;
    }
    let (branch_part, track_part) = match line.find(" [") {
        Some(idx) => (&line[..idx], &line[idx + 1..]),
        None => (line, ""),
    };
    let branch = branch_part.split("...").next().unwrap_or(branch_part);
    status.head_branch = branch.trim().to_string();
    if let Some(close) = track_part.find(']') {
        let track = &track_part[1..close];
        for part in track.split(',') {
            let part = part.trim();
            if let Some(num) = part.strip_prefix("ahead ") {
                status.ahead = num.trim().parse().unwrap_or(0);
            } else if let Some(num) = part.strip_prefix("behind ") {
                status.behind = num.trim().parse().unwrap_or(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_changes() {
        let output = "\
## master...origin/master [ahead 1, behind 2]
 M src/main.rs
M  src/lib.rs
A  new.txt
D  old.txt
R  renamed_old.txt -> renamed_new.txt
?? target/
UU conflict.txt
";
        let status = parse_status(output);
        assert_eq!(status.head_branch, "master");
        assert_eq!(status.ahead, 1);
        assert_eq!(status.behind, 2);
        assert!(!status.unborn);
        assert_eq!(status.changes.len(), 7);

        assert_eq!(status.changes[0].status, ChangeStatus::Modified);
        assert!(!status.changes[0].staged);
        assert_eq!(status.changes[0].path, "src/main.rs");

        assert_eq!(status.changes[1].status, ChangeStatus::Modified);
        assert!(status.changes[1].staged);
        assert_eq!(status.changes[1].path, "src/lib.rs");

        assert_eq!(status.changes[2].status, ChangeStatus::Added);
        assert!(status.changes[2].staged);

        assert_eq!(status.changes[3].status, ChangeStatus::Deleted);

        let renamed = &status.changes[4];
        assert_eq!(renamed.status, ChangeStatus::Renamed);
        assert_eq!(renamed.original_path.as_deref(), Some("renamed_old.txt"));
        assert_eq!(renamed.path, "renamed_new.txt");
        assert_eq!(renamed.display_path(), "renamed_old.txt -> renamed_new.txt");

        assert_eq!(status.changes[5].status, ChangeStatus::Untracked);
        assert_eq!(status.changes[5].path, "target/");

        assert_eq!(status.changes[6].status, ChangeStatus::Conflicted);
    }

    #[test]
    fn parses_unborn_branch() {
        let output = "## No commits yet on main\n?? hello.rs\n";
        let status = parse_status(output);
        assert_eq!(status.head_branch, "main");
        assert!(status.unborn);
        assert_eq!(status.changes.len(), 1);
    }

    #[test]
    fn parses_branch_without_upstream() {
        let status = parse_status("## feature\n");
        assert_eq!(status.head_branch, "feature");
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
    }

    #[test]
    fn empty_output() {
        let status = parse_status("");
        assert_eq!(status.head_branch, "");
        assert!(status.changes.is_empty());
    }
}
