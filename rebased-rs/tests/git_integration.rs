use std::path::PathBuf;
use std::process::Command;

use rebased_rs::git::{
    filter_commits, load_repo_data, ChangeStatus, Repository, DEFAULT_LOG_LIMIT,
};

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rebased-rs-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        let repo = Self { path };
        repo.git(&["init", "-b", "main"]);
        repo.git(&["config", "user.name", "Test User"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Test User")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test User")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8")
    }

    fn write(&self, name: &str, content: &str) {
        std::fs::write(self.path.join(name), content).expect("write file");
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn full_workflow() {
    let temp = TempRepo::new();
    temp.write("hello.txt", "first\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial commit"]);

    let repo = Repository::open(&temp.path).expect("open repo");

    let log = repo.log(50).expect("log");
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].subject, "initial commit");
    assert!(log[0].is_root());

    let status = repo.status().expect("status");
    assert_eq!(status.head_branch, "main");
    assert!(!status.unborn);
    assert!(status.changes.is_empty());

    temp.write("hello.txt", "changed\n");
    temp.write("new_file.txt", "new\n");

    let status = repo.status().expect("status");
    assert_eq!(status.changes.len(), 2);
    assert!(status
        .changes
        .iter()
        .any(|c| c.status == ChangeStatus::Untracked && c.path == "new_file.txt"));
    assert!(status
        .changes
        .iter()
        .any(|c| c.status == ChangeStatus::Modified && c.path == "hello.txt"));

    repo.add(&["hello.txt"]).expect("add");
    let status = repo.status().expect("status");
    assert!(status
        .changes
        .iter()
        .any(|c| c.path == "hello.txt" && c.staged));

    repo.commit_paths("update hello", &["hello.txt"], false)
        .expect("commit paths");

    let status = repo.status().expect("status");
    assert_eq!(status.changes.len(), 1);
    assert_eq!(status.changes[0].path, "new_file.txt");
    assert_eq!(status.changes[0].status, ChangeStatus::Untracked);

    let log = repo.log(50).expect("log");
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "update hello");
    assert_eq!(log[0].parents.len(), 1);
    assert_eq!(log[0].parents[0], log[1].id);

    let branches = repo.branches().expect("branches");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "main");
    assert!(branches[0].is_current());
    assert_eq!(branches[0].commit_id, log[0].id);

    repo.create_branch("feature", Some("main")).expect("create branch");
    repo.checkout("feature").expect("checkout");

    let status = repo.status().expect("status");
    assert_eq!(status.head_branch, "feature");

    let branches = repo.branches().expect("branches");
    assert_eq!(branches.len(), 2);
    let feature = branches.iter().find(|b| b.name == "feature").expect("feature");
    assert!(feature.is_current());
    let main = branches.iter().find(|b| b.name == "main").expect("main");
    assert!(!main.is_current());

    repo.checkout("main").expect("checkout back");
    let status = repo.status().expect("status");
    assert_eq!(status.head_branch, "main");

    let amend_before = repo.log(10).expect("log");
    temp.write("hello.txt", "changed more\n");
    repo.add(&["hello.txt"]).expect("add");
    repo.commit("amend me", true).expect("commit");
    let amend_after = repo.log(10).expect("log");
    assert_eq!(amend_after.len(), amend_before.len() + 1);
    let amended = amend_after
        .iter()
        .find(|c| c.subject == "amend me")
        .expect("amended commit");
    let previous = amend_before
        .iter()
        .find(|c| c.subject == "update hello")
        .expect("previous commit");
    assert_ne!(amended.id, previous.id);

    temp.write("hello.txt", "even more\n");
    repo.add(&["hello.txt"]).expect("add");
    let status = repo.status().expect("status");
    assert!(status
        .changes
        .iter()
        .any(|c| c.path == "hello.txt" && c.staged));

    repo.reset(&["hello.txt"]).expect("reset");
    let status = repo.status().expect("status");
    assert_eq!(status.changes.len(), 2);
    let hello = status
        .changes
        .iter()
        .find(|c| c.path == "hello.txt")
        .expect("hello change");
    assert_eq!(hello.status, ChangeStatus::Modified);
    assert!(!hello.staged);
}

#[test]
fn empty_repo_is_handled() {
    let temp = TempRepo::new();
    temp.write("readme.md", "hi\n");

    let repo = Repository::open(&temp.path).expect("open repo");
    let log = repo.log(50).expect("log on unborn head");
    assert!(log.is_empty());

    let status = repo.status().expect("status");
    assert_eq!(status.head_branch, "main");
    assert!(status.unborn);
    assert_eq!(status.changes.len(), 1);
    assert_eq!(status.changes[0].status, ChangeStatus::Untracked);
}

#[test]
fn load_repo_data_snapshot() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "first"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    temp.git(&["branch", "feature"]);
    temp.write("dirty.txt", "uncommitted\n");

    let repo = Repository::open(&temp.path).expect("open repo");
    let data = load_repo_data(&repo, DEFAULT_LOG_LIMIT).expect("load");

    assert_eq!(data.commits.len(), 2);
    assert_eq!(data.graph.rows.len(), 2);
    assert_eq!(data.graph.lane_count, 1);
    assert_eq!(data.status.head_branch, "main");
    assert_eq!(data.status.changes.len(), 1);
    assert_eq!(data.status.changes[0].path, "dirty.txt");
    assert_eq!(data.branches.len(), 2);

    let visible = filter_commits(&data.commits, "second");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].subject, "second");
}

#[test]
fn branches_containing_resolves_commits() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.git(&["branch", "feature"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let log = repo.log(10).expect("log");
    let first = log.iter().find(|c| c.subject == "initial").expect("first");
    let second = log.iter().find(|c| c.subject == "second").expect("second");

    let containing_second = repo.branches_containing(second.id.as_str()).expect("contains second");
    assert_eq!(containing_second, vec!["main".to_string()]);

    let mut containing_first = repo.branches_containing(first.id.as_str()).expect("contains first");
    containing_first.sort();
    assert_eq!(containing_first, vec!["feature".to_string(), "main".to_string()]);

    assert!(repo.branches_containing("deadbeefdeadbeef").is_err());
}

#[test]
fn open_rejects_non_repo() {
    let dir = std::env::temp_dir().join(format!("rebased-rs-norepo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let result = Repository::open(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(result.is_err());
}
