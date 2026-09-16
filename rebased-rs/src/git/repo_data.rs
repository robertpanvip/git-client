use super::backend::GitBackend;
use super::error::Result;
use super::graph::{Graph, build_graph};
use super::types::{Branch, Commit, RepoStatus, Tag};

pub const DEFAULT_LOG_LIMIT: usize = 500;

pub struct RepoData {
    pub commits: Vec<Commit>,
    pub graph: Graph,
    pub status: RepoStatus,
    pub branches: Vec<Branch>,
    pub tags: Vec<Tag>,
    pub remotes: Vec<super::types::Remote>,
}

pub fn load_repo_data(repo: &dyn GitBackend, log_limit: usize) -> Result<RepoData> {
    load_repo_data_filtered(repo, log_limit, None, None, None)
}

/// 带结构化过滤器的加载：`branch` 限定提交范围（None = 所有分支），
/// `author` 按作者匹配（None / 空串 = 不过滤），`since` 为 git 日期表达式（None = 不限时间）。
pub fn load_repo_data_filtered(
    repo: &dyn GitBackend,
    log_limit: usize,
    branch: Option<&str>,
    author: Option<&str>,
    since: Option<&str>,
) -> Result<RepoData> {
    let commits = repo.log_filtered(log_limit, branch, author, since)?;
    let graph = build_graph(&commits);
    let status = repo.status()?;
    let branches = repo.branches()?;
    let tags = repo.tags()?;
    let remotes = repo.remotes().unwrap_or_default();
    Ok(RepoData {
        commits,
        graph,
        status,
        branches,
        tags,
        remotes,
    })
}

pub fn filter_commits<'a>(commits: &'a [Commit], query: &str) -> Vec<&'a Commit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return commits.iter().collect();
    }
    commits
        .iter()
        .filter(|c| {
            c.subject.to_lowercase().contains(&query)
                || c.author.name.to_lowercase().contains(&query)
                || c.id.as_str().to_lowercase().contains(&query)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::types::{Author, CommitId};
    use super::*;

    fn commit(id: &str, subject: &str, author: &str) -> Commit {
        Commit {
            id: CommitId(id.to_string()),
            parents: Vec::new(),
            author: Author {
                name: author.to_string(),
                email: format!("{}@x", author),
            },
            time: 0,
            subject: subject.to_string(),
            body: String::new(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn empty_query_returns_all() {
        let commits = vec![
            commit("a1", "feat: one", "alice"),
            commit("b2", "fix: two", "bob"),
        ];
        assert_eq!(filter_commits(&commits, "  ").len(), 2);
        assert_eq!(filter_commits(&commits, "").len(), 2);
    }

    #[test]
    fn filters_by_subject_case_insensitive() {
        let commits = vec![
            commit("a1", "Feat: Add Button", "alice"),
            commit("b2", "fix bug", "bob"),
        ];
        let hits = filter_commits(&commits, "button");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id.as_str(), "a1");
    }

    #[test]
    fn filters_by_author_and_id() {
        let commits = vec![
            commit("abc123", "one", "alice"),
            commit("def456", "two", "bob"),
        ];
        assert_eq!(filter_commits(&commits, "BOB").len(), 1);
        assert_eq!(filter_commits(&commits, "DEF").len(), 1);
        assert_eq!(filter_commits(&commits, "zzz").len(), 0);
    }
}
