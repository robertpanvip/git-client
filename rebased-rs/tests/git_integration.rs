use std::path::PathBuf;
use std::process::Command;

use rebased_rs::git::{
    filter_commits, load_repo_data, parse_unified_diff, ChangeStatus, DiffLineKind,
    RebaseActionKind, Repository, DEFAULT_LOG_LIMIT,
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

#[test]
fn tag_workflow() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let head = repo.log(5).expect("log")[0].id.clone();
    let head_id = head.as_str().to_string();

    repo.create_tag("v1.0", None, None).expect("lightweight tag");
    repo.create_tag("v2.0", Some("HEAD"), Some("release two"))
        .expect("annotated tag");

    let tags = repo.tags().expect("tags");
    assert_eq!(tags.len(), 2);
    let v1 = tags.iter().find(|t| t.name == "v1.0").expect("v1");
    assert_eq!(v1.commit_id, head_id);
    let v2 = tags.iter().find(|t| t.name == "v2.0").expect("v2");
    assert_eq!(v2.commit_id, head_id, "annotated tag peels to commit");

    repo.delete_tag("v1.0").expect("delete tag");
    let tags = repo.tags().expect("tags after delete");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "v2.0");
}

#[test]
fn cherry_pick_and_revert() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.git(&["branch", "feature"]);
    temp.git(&["checkout", "feature"]);
    temp.write("feature.txt", "feature work\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "feature work"]);
    temp.git(&["checkout", "main"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let feature_commit = {
        let log = repo.log(10).expect("log");
        log.iter()
            .find(|c| c.subject == "feature work")
            .expect("feature commit")
            .id
            .clone()
    };

    std::thread::sleep(std::time::Duration::from_millis(1100));
    repo.cherry_pick(feature_commit.as_str()).expect("cherry-pick");
    let log = repo.log(10).expect("log after cherry-pick");
    let picked = log.iter().find(|c| c.subject == "feature work").expect("picked");
    assert_ne!(picked.id, feature_commit, "new sha after cherry-pick");
    assert!(temp.path.join("feature.txt").exists());

    repo.revert(picked.id.as_str()).expect("revert");
    let log = repo.log(10).expect("log after revert");
    assert_eq!(log[0].subject, "Revert \"feature work\"");
    assert!(!temp.path.join("feature.txt").exists(), "revert removed the file");
}

#[test]
fn reset_modes() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let initial = repo.log(10).expect("log")[1].id.clone();

    repo.reset_to(initial.as_str(), rebased_rs::git::ResetMode::Soft)
        .expect("soft reset");
    let status = repo.status().expect("status");
    assert!(
        status.changes.iter().any(|c| c.path == "b.txt" && c.staged),
        "soft reset keeps changes staged"
    );

    repo.reset_to(initial.as_str(), rebased_rs::git::ResetMode::Hard)
        .expect("hard reset");
    let status = repo.status().expect("status");
    assert!(status.changes.is_empty());
    assert!(!temp.path.join("b.txt").exists());
    assert_eq!(repo.log(10).expect("log").len(), 1);
}

#[test]
fn diff_parsing() {
    let temp = TempRepo::new();
    temp.write("file.txt", "one\ntwo\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let repo = Repository::open(&temp.path).expect("open repo");

    let empty = repo.diff_head(None).expect("diff head clean");
    assert!(parse_unified_diff(&empty).is_empty());

    temp.write("file.txt", "one\nTWO\nthree\n");
    let unstaged = repo.diff_unstaged(None).expect("diff unstaged");
    let files = parse_unified_diff(&unstaged);
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_eq!(file.path, "file.txt");
    assert!(!file.is_new && !file.is_deleted && !file.is_binary);
    assert_eq!(file.hunks.len(), 1);
    let lines = &file.hunks[0].lines;
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].kind, DiffLineKind::Context);
    assert_eq!(lines[0].content, "one");
    assert_eq!(lines[0].old_no, Some(1));
    assert_eq!(lines[0].new_no, Some(1));
    assert_eq!(lines[1].kind, DiffLineKind::Deleted);
    assert_eq!(lines[1].content, "two");
    assert_eq!(lines[1].old_no, Some(2));
    assert_eq!(lines[1].new_no, None);
    assert_eq!(lines[2].kind, DiffLineKind::Added);
    assert_eq!(lines[2].content, "TWO");
    assert_eq!(lines[2].old_no, None);
    assert_eq!(lines[2].new_no, Some(2));
    assert_eq!(lines[3].kind, DiffLineKind::Added);
    assert_eq!(lines[3].content, "three");
    assert_eq!(lines[3].old_no, None);
    assert_eq!(lines[3].new_no, Some(3));

    repo.add(&["file.txt"]).expect("add");
    let staged = repo.diff_staged(None).expect("diff staged");
    let files = parse_unified_diff(&staged);
    assert_eq!(files.len(), 1);
    assert!(files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| l.kind == DiffLineKind::Added && l.content == "TWO"));

    repo.commit("update file", false).expect("commit");
    let after = repo.diff_head(None).expect("diff head after commit");
    assert!(parse_unified_diff(&after).is_empty());

    let shown = repo.show_diff("HEAD", None).expect("show diff");
    let files = parse_unified_diff(&shown);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "file.txt");
    assert!(files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| l.kind == DiffLineKind::Context && l.content == "one"));
    assert!(files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| l.kind == DiffLineKind::Added && l.content == "TWO"));
}

#[test]
fn new_file_diff_marks_added() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("brand_new.txt", "brand\nnew\n");

    let repo = Repository::open(&temp.path).expect("open repo");
    let out = repo.diff_unstaged(Some("brand_new.txt")).expect("diff");
    let files = parse_unified_diff(&out);
    assert_eq!(files.len(), 1);
    assert!(files[0].is_new);
    assert_eq!(files[0].path, "brand_new.txt");
    let added: Vec<_> = files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind == DiffLineKind::Added)
        .collect();
    assert_eq!(added.len(), 2);
}

#[test]
fn blame_parsing() {
    let temp = TempRepo::new();
    temp.write("file.txt", "line1\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "first"]);
    temp.write("file.txt", "line1\nline2\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let groups = repo.blame("HEAD", "file.txt").expect("blame");
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].author, "Test User");
    assert_eq!(groups[0].filename, "file.txt");
    assert_eq!(groups[0].lines.len(), 1);
    assert_eq!(groups[0].lines[0].number, 1);
    assert_eq!(groups[0].lines[0].content, "line1");
    assert_ne!(groups[0].commit_id, groups[1].commit_id);
    assert_eq!(groups[1].lines[0].number, 2);
    assert_eq!(groups[1].lines[0].content, "line2");
}

#[test]
fn interactive_rebase_drop_and_squash() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    temp.write("c.txt", "c\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "third"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 3);
    let base = log[2].id.0.clone();

    let mut plan = repo.rebase_todos(&base).expect("todos");
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].subject, "second");
    assert_eq!(plan[1].subject, "third");
    assert!(plan.iter().all(|a| a.kind == RebaseActionKind::Pick));

    plan[1].kind = RebaseActionKind::Squash;
    repo.rebase_run(&base, &plan).expect("rebase run");

    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 2);
    assert!(temp.path.join("b.txt").exists());
    assert!(temp.path.join("c.txt").exists());
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn interactive_rebase_drop() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    temp.write("c.txt", "c\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "third"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let base = repo.log(10).expect("log")[2].id.0.clone();

    let mut plan = repo.rebase_todos(&base).expect("todos");
    plan[0].kind = RebaseActionKind::Drop;
    repo.rebase_run(&base, &plan).expect("rebase run");

    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 2);
    assert!(!temp.path.join("b.txt").exists());
    assert!(temp.path.join("c.txt").exists());
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn rebase_conflict_and_abort() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.git(&["checkout", "-b", "feature"]);
    temp.write("a.txt", "feature\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "feature edit"]);
    temp.git(&["checkout", "main"]);
    temp.write("a.txt", "main\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "main edit"]);
    temp.git(&["checkout", "feature"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let plan = repo.rebase_todos("main").expect("todos");
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].subject, "feature edit");

    assert!(repo.rebase_run("main", &plan).is_err());
    assert!(repo.is_rebase_in_progress());

    repo.rebase_abort().expect("abort");
    assert!(!repo.is_rebase_in_progress());
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "feature\n");
}
