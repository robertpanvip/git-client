use super::types::{Branch, CommitId};

pub const REF_FORMAT: &str =
    "%(refname)%09%(objectname)%09%(HEAD)%09%(upstream:short)%09%(upstream:track)";

pub fn ref_args() -> Vec<String> {
    vec![
        "for-each-ref".to_string(),
        "--format".to_string(),
        REF_FORMAT.to_string(),
        "refs/heads/".to_string(),
        "refs/remotes/".to_string(),
    ]
}

pub fn parse_ref_line(line: &str) -> Option<Branch> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 3 {
        return None;
    }
    let full_name = parts[0].trim();
    if full_name.is_empty() {
        return None;
    }
    let commit_id = parts[1].trim();
    if commit_id.is_empty() {
        return None;
    }
    let is_head = parts.get(2).copied().unwrap_or("").trim() == "*";
    let upstream = parts
        .get(3)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let (ahead, behind) = parts
        .get(4)
        .map(|s| parse_track(s.trim()))
        .unwrap_or((0, 0));
    let is_remote = full_name.starts_with("refs/remotes/");
    let name = if is_remote {
        full_name.trim_start_matches("refs/remotes/").to_string()
    } else {
        full_name.trim_start_matches("refs/heads/").to_string()
    };
    Some(Branch {
        name,
        full_name: full_name.to_string(),
        commit_id: CommitId(commit_id.to_string()),
        is_head,
        is_remote,
        upstream,
        ahead,
        behind,
    })
}

pub fn parse_refs(output: &str) -> Vec<Branch> {
    let mut branches: Vec<Branch> = output.lines().filter_map(parse_ref_line).collect();
    branches.sort_by(|a, b| {
        a.is_remote
            .cmp(&b.is_remote)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    branches
}

fn parse_track(track: &str) -> (u32, u32) {
    let mut ahead = 0;
    let mut behind = 0;
    let inner = track.trim_matches(|c| c == '[' || c == ']');
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(num) = part.strip_prefix("ahead ") {
            ahead = num.trim().parse().unwrap_or(0);
        } else if let Some(num) = part.strip_prefix("behind ") {
            behind = num.trim().parse().unwrap_or(0);
        }
    }
    (ahead, behind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(full_name: &str, sha: &str, head: &str, upstream: &str, track: &str) -> String {
        format!("{full_name}\t{sha}\t{head}\t{upstream}\t{track}")
    }

    #[test]
    fn parses_local_head_branch() {
        let out = line(
            "refs/heads/master",
            "aaaa1111",
            "*",
            "origin/master",
            "[ahead 2, behind 1]",
        );
        let b = parse_ref_line(&out).expect("branch");
        assert_eq!(b.name, "master");
        assert_eq!(b.full_name, "refs/heads/master");
        assert!(b.is_head);
        assert!(!b.is_remote);
        assert!(b.is_current());
        assert_eq!(b.upstream.as_deref(), Some("origin/master"));
        assert_eq!(b.ahead, 2);
        assert_eq!(b.behind, 1);
        assert_eq!(b.commit_id.as_str(), "aaaa1111");
    }

    #[test]
    fn parses_remote_branch() {
        let out = line("refs/remotes/origin/dev", "bbbb2222", "", "", "");
        let b = parse_ref_line(&out).expect("branch");
        assert_eq!(b.name, "origin/dev");
        assert!(b.is_remote);
        assert!(!b.is_head);
        assert_eq!(b.upstream, None);
        assert_eq!(b.ahead, 0);
    }

    #[test]
    fn skips_garbage_lines() {
        assert!(parse_ref_line("").is_none());
        assert!(parse_ref_line("refs/heads/x\t").is_none());
    }

    #[test]
    fn sorts_local_first() {
        let out = format!(
            "{}\n{}\n{}\n",
            line("refs/remotes/origin/main", "c3", "", "", ""),
            line("refs/heads/feature", "c2", "", "", ""),
            line("refs/heads/main", "c1", "*", "origin/main", "[ahead 1]")
        );
        let branches = parse_refs(&out);
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "feature");
        assert_eq!(branches[1].name, "main");
        assert_eq!(branches[2].name, "origin/main");
        assert!(branches[2].is_remote);
    }

    #[test]
    fn parses_track_variants() {
        assert_eq!(parse_track("[ahead 5]"), (5, 0));
        assert_eq!(parse_track("[behind 3]"), (0, 3));
        assert_eq!(parse_track("[gone]"), (0, 0));
        assert_eq!(parse_track("[]"), (0, 0));
    }
}
