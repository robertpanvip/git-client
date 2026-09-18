use super::command::GitCommand;
use super::error::Result;
use super::types::BlameGroup;
use super::types::BlameLine;

pub fn blame_file(cmd: &GitCommand, rev: &str, path: &str) -> Result<String> {
    cmd.run(&["blame", "--porcelain", rev, "--", path])
}

/// blame 工作区版本（不带 rev）：行号对应工作区文件当前内容，
/// 未提交的行由 git 标记为全 0 commit（author「Not Committed Yet」）。
pub fn blame_worktree(cmd: &GitCommand, path: &str) -> Result<String> {
    cmd.run(&["blame", "--porcelain", "--", path])
}

struct Meta {
    commit: String,
    author: String,
    time: i64,
    filename: String,
}

fn is_commit_token(token: &str) -> bool {
    token.len() == 40 && token.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn parse_blame(stdout: &str) -> Vec<BlameGroup> {
    let mut groups: Vec<BlameGroup> = Vec::new();
    let mut current: Option<BlameGroup> = None;
    let mut meta: Option<Meta> = None;
    let mut pending_final: u32 = 0;

    for line in stdout.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            if let Some(m) = meta.take() {
                if let Some(group) = current.take() {
                    groups.push(group);
                }
                current = Some(BlameGroup {
                    commit_id: m.commit,
                    author: m.author,
                    time: m.time,
                    filename: m.filename,
                    lines: vec![BlameLine {
                        number: pending_final,
                        content: content.to_string(),
                    }],
                });
            } else if let Some(group) = current.as_mut() {
                group.lines.push(BlameLine {
                    number: pending_final,
                    content: content.to_string(),
                });
            }
            continue;
        }

        let mut tokens = line.split_whitespace();
        let first = tokens.next().unwrap_or("");
        let second = tokens.next().unwrap_or("");
        let third = tokens.next().unwrap_or("");
        if is_commit_token(first) && !second.is_empty() && !third.is_empty() {
            pending_final = third.parse().unwrap_or(0);
            let is_new = current
                .as_ref()
                .is_none_or(|group| group.commit_id != first);
            if is_new {
                meta = Some(Meta {
                    commit: first.to_string(),
                    author: String::new(),
                    time: 0,
                    filename: String::new(),
                });
            }
            continue;
        }

        if let Some(m) = meta.as_mut() {
            if let Some(author) = line.strip_prefix("author ") {
                m.author = author.to_string();
            } else if let Some(time) = line.strip_prefix("author-time ") {
                m.time = time.parse().unwrap_or(0);
            } else if let Some(filename) = line.strip_prefix("filename ") {
                m.filename = filename.to_string();
            }
        }
    }

    if let Some(group) = current.take() {
        groups.push(group);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    const SAMPLE: &str = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 2
author Alice
author-mail <alice@x>
author-time 1700000000
author-tz +0000
committer Alice
committer-mail <alice@x>
committer-time 1700000000
committer-tz +0000
summary first
boundary
filename hello.txt
	first line
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 2
	second line
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 1 3 1
author Bob
author-mail <bob@x>
author-time 1700000100
author-tz +0000
committer Bob
committer-mail <bob@x>
committer-time 1700000100
committer-tz +0000
summary second
filename hello.txt
	third line
";

    #[test]
    fn parses_groups_and_metadata() {
        let groups = parse_blame(SAMPLE);
        assert_eq!(groups.len(), 2);

        assert_eq!(groups[0].commit_id, SHA_A);
        assert_eq!(groups[0].author, "Alice");
        assert_eq!(groups[0].time, 1700000000);
        assert_eq!(groups[0].filename, "hello.txt");
        assert_eq!(groups[0].lines.len(), 2);
        assert_eq!(groups[0].lines[0].number, 1);
        assert_eq!(groups[0].lines[0].content, "first line");
        assert_eq!(groups[0].lines[1].number, 2);

        assert_eq!(groups[1].commit_id, SHA_B);
        assert_eq!(groups[1].author, "Bob");
        assert_eq!(groups[1].lines.len(), 1);
        assert_eq!(groups[1].lines[0].number, 3);
        assert_eq!(groups[1].lines[0].content, "third line");
    }

    #[test]
    fn empty_input_yields_no_groups() {
        assert!(parse_blame("").is_empty());
    }

    #[test]
    fn same_commit_reappearing_starts_new_group() {
        let out = format!(
            "\
{SHA_A} 1 1 1
author Alice
author-time 100
filename f.txt
\tone
{SHA_B} 1 2 1
author Bob
author-time 200
filename f.txt
\ttwo
{SHA_A} 2 3 1
author Alice
author-time 100
filename f.txt
\tthree
"
        );
        let groups = parse_blame(&out);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[2].commit_id, SHA_A);
        assert_eq!(groups[2].lines[0].number, 3);
    }
}
