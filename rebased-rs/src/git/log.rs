use super::error::Result;
use super::types::{Author, Commit, CommitId};

pub const FIELD_SEP: char = '\u{1f}';
pub const RECORD_SEP: char = '\u{1e}';

pub const LOG_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1f%D%x1e";

pub fn log_args(limit: usize, from: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "log".to_string(),
        format!("--max-count={limit}"),
        "--format=".to_string() + LOG_FORMAT,
        "--date-order".to_string(),
        "--all".to_string(),
    ];
    if let Some(rev) = from {
        args.push(rev.to_string());
    }
    args
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
        s.push_str("2222222222222222222222222222222222222222 3333333333333333333333333333333333333333");
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
        assert_eq!(first.id.as_str(), "1111111111111111111111111111111111111111");
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
}
