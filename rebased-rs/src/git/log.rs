use super::error::Result;
use super::types::{Author, Commit, CommitId};

pub const FIELD_SEP: char = '\u{1f}';
pub const RECORD_SEP: char = '\u{1e}';

pub const LOG_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1f%D%x1e";

/// `from` 为 Some 时只列出该 rev 可达的提交，否则 `--all`；`author` 按作者过滤；
/// `since` 为 git 日期表达式（如 `midnight`、`1 week ago`），按提交时间过滤。
pub fn log_args(
    limit: usize,
    from: Option<&str>,
    author: Option<&str>,
    since: Option<&str>,
    path: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "log".to_string(),
        format!("--max-count={limit}"),
        "--format=".to_string() + LOG_FORMAT,
        "--date-order".to_string(),
    ];
    match from {
        Some(rev) => args.push(rev.to_string()),
        None => args.push("--all".to_string()),
    }
    if let Some(author) = author {
        args.push(format!("--author={author}"));
    }
    if let Some(since) = since {
        args.push(format!("--since={since}"));
    }
    if let Some(path) = path {
        args.push("--".to_string());
        args.push(path.to_string());
    }
    args
}

/// History of commits touching a single path（`--follow -- path`）。
pub fn log_follow_args(limit: usize, path: &str) -> Vec<String> {
    vec![
        "log".to_string(),
        format!("--max-count={limit}"),
        "--format=".to_string() + LOG_FORMAT,
        "--follow".to_string(),
        "--".to_string(),
        path.to_string(),
    ]
}

/// History of commits touching a path（`-- path`，目录/路径过滤，不做 rename 跟随）。
pub fn log_path_args(limit: usize, path: &str) -> Vec<String> {
    vec![
        "log".to_string(),
        format!("--max-count={limit}"),
        "--format=".to_string() + LOG_FORMAT,
        "--".to_string(),
        path.to_string(),
    ]
}

/// Full message（%B）of a revision，例如 HEAD 用于 Amend 预填。
pub fn full_message(cmd: &super::command::GitCommand, revision: &str) -> Result<String> {
    let output = cmd.run(&["log", "-1", "--format=%B", revision])?;
    Ok(output.trim().to_string())
}

pub fn parse_log(output: &str) -> Vec<Commit> {
    let mut commits = Vec::new();
    for record in output.split(RECORD_SEP) {
        let record = record.trim_start_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = record.split(FIELD_SEP).collect();
        if fields.len() < 8 {
            continue;
        }
        let id = fields[0].trim();
        if id.is_empty() {
            continue;
        }
        let parents: Vec<CommitId> = fields[1]
            .split_whitespace()
            .map(|p| CommitId(p.to_string()))
            .collect();
        let author = Author {
            name: fields[2].to_string(),
            email: fields[3].to_string(),
        };
        let time = fields[4].trim().parse().unwrap_or(0);
        let subject = fields[5].to_string();
        let body = fields[6].trim_end_matches(['\n', '\r']).to_string();
        let refs: Vec<String> = fields[7]
            .split(FIELD_SEP)
            .flat_map(|r| r.split(", "))
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        commits.push(Commit {
            id: CommitId(id.to_string()),
            parents,
            author,
            time,
            subject,
            body,
            refs,
        });
    }
    commits
}

pub fn cat_file(cmd: &super::command::GitCommand, revision: &str) -> Result<Commit> {
    let output = cmd.run(&["show", "-s", &format!("--format={LOG_FORMAT}"), revision])?;
    parse_log(&output)
        .into_iter()
        .next()
        .ok_or_else(|| super::error::GitError::new(format!("unknown revision {revision}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> String {
        let mut s = String::new();
        s.push_str("1111111111111111111111111111111111111111");
        s.push('\u{1f}');
        s.push_str(
            "2222222222222222222222222222222222222222 3333333333333333333333333333333333333333",
        );
        s.push('\u{1f}');
        s.push_str("Alice");
        s.push('\u{1f}');
        s.push_str("alice@example.com");
        s.push('\u{1f}');
        s.push_str("1700000000");
        s.push('\u{1f}');
        s.push_str("Merge branch 'feature'");
        s.push('\u{1f}');
        s.push_str("Long body\nsecond line");
        s.push('\u{1f}');
        s.push_str("HEAD -> master, origin/master, tag: v1.0");
        s.push('\u{1e}');
        s.push('\n');
        s.push_str("4444444444444444444444444444444444444444");
        s.push('\u{1f}');
        s.push('\u{1f}');
        s.push_str("Bob");
        s.push('\u{1f}');
        s.push_str("bob@example.com");
        s.push('\u{1f}');
        s.push_str("1699999999");
        s.push('\u{1f}');
        s.push_str("initial commit");
        s.push('\u{1f}');
        s.push('\u{1f}');
        s.push('\u{1e}');
        s.push('\n');
        s
    }

    #[test]
    fn parses_records() {
        let commits = parse_log(&sample());
        assert_eq!(commits.len(), 2);

        let first = &commits[0];
        assert_eq!(
            first.id.as_str(),
            "1111111111111111111111111111111111111111"
        );
        assert_eq!(first.parents.len(), 2);
        assert!(first.is_merge());
        assert_eq!(first.author.name, "Alice");
        assert_eq!(first.time, 1700000000);
        assert_eq!(first.subject, "Merge branch 'feature'");
        assert_eq!(first.body, "Long body\nsecond line");
        assert_eq!(
            first.refs,
            vec!["HEAD -> master", "origin/master", "tag: v1.0"]
        );

        let second = &commits[1];
        assert!(second.is_root());
        assert!(!second.is_merge());
        assert_eq!(second.subject, "initial commit");
        assert!(second.refs.is_empty());
    }

    #[test]
    fn skips_empty_records() {
        assert!(parse_log("").is_empty());
        assert!(parse_log("\n\n\u{1e}\n").is_empty());
    }

    #[test]
    fn short_id() {
        let id = CommitId("1234567890abcdef".to_string());
        assert_eq!(id.short(), "1234567");
    }

    #[test]
    fn log_follow_args_scoped_to_path() {
        let args = log_follow_args(50, "src/main.rs");
        assert_eq!(args[0], "log");
        assert_eq!(args[1], "--max-count=50");
        assert!(args[2].starts_with("--format="));
        assert_eq!(args[3], "--follow");
        assert_eq!(args[4], "--");
        assert_eq!(args[5], "src/main.rs");
    }

    #[test]
    fn log_path_args_scoped_without_follow() {
        let args = log_path_args(50, "src/ui");
        assert_eq!(args[0], "log");
        assert_eq!(args[1], "--max-count=50");
        assert!(args[2].starts_with("--format="));
        assert_eq!(args[3], "--");
        assert_eq!(args[4], "src/ui");
        assert!(!args.iter().any(|a| a == "--follow"));
    }

    #[test]
    fn log_args_includes_since() {
        let args = log_args(50, None, None, Some("midnight"), None);
        assert!(args.contains(&"--since=midnight".to_string()));

        let args = log_args(50, None, None, None, None);
        assert!(!args.iter().any(|a| a.starts_with("--since")));
    }
}
