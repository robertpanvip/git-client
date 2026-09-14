use super::error::Result;
use super::graph::{build_graph, Graph};
use super::repo::Repository;
use super::types::{Branch, Commit, RepoStatus};

pub const DEFAULT_LOG_LIMIT: usize = 500;

pub struct RepoData {
    pub commits: Vec<Commit>,
    pub graph: Graph,
    pub status: RepoStatus,
    pub branches: Vec<Branch>,
}

pub fn load_repo_data(repo: &Repository, log_limit: usize) -> Result<RepoData> {
    let commits = repo.log(log_limit)?;
    let graph = build_graph(&commits);
    let status = repo.status()?;
    let branches = repo.branches()?;
    Ok(RepoData {
        commits,
        graph,
        status,
        branches,
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
        let commits = vec![commit("a1", "feat: one", "alice"), commit("b2", "fix: two", "bob")];
        assert_eq!(filter_commits(&commits, "  ").len(), 2);
        assert_eq!(filter_commits(&commits, "").len(), 2);
    }

    #[test]
    fn filters_by_subject_case_insensitive() {
        let commits = vec![commit("a1", "Feat: Add Button", "alice"), commit("b2", "fix bug", "bob")];
        let hits = filter_commits(&commits, "button");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id.as_str(), "a1");
    }

    #[test]
    fn filters_by_author_and_id() {
        let commits = vec![commit("abc123", "one", "alice"), commit("def456", "two", "bob")];
        assert_eq!(filter_commits(&commits, "BOB").len(), 1);
        assert_eq!(filter_commits(&commits, "DEF").len(), 1);
        assert_eq!(filter_commits(&commits, "zzz").len(), 0);
    }
}
