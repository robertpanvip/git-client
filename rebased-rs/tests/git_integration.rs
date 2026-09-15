use std::path::PathBuf;
use std::process::Command;

use rebased_rs::git::{
    autosquash_plan, conflict_hunks, filter_commits, load_repo_data, parse_unified_diff,
    ChangeStatus, ConflictKind, DiffLineKind, HunkChoice, RebaseActionKind, Repository,
    DEFAULT_LOG_LIMIT,
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
fn remote_and_upstream_workflow() {
    // 建一个裸仓库充当远程。
    let origin_path = std::env::temp_dir().join(format!(
        "rebased-rs-test-origin-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&origin_path).expect("create origin dir");
    let output = Command::new("git")
        .args(["init", "--bare", "-b", "main"])
        .current_dir(&origin_path)
        .output()
        .expect("init bare");
    assert!(output.status.success(), "bare init failed");

    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let url = origin_path.to_string_lossy().to_string();
    assert!(repo.remotes().expect("remotes").is_empty(), "no remotes yet");

    repo.remote_add("origin", &url).expect("add remote");
    let remotes = repo.remotes().expect("remotes after add");
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].name, "origin");
    assert_eq!(remotes[0].url, url);

    // 推送并建立 upstream。
    temp.git(&["push", "-u", "origin", "main"]);
    let branches = repo.branches().expect("branches");
    let main = branches.iter().find(|b| b.name == "main").expect("main");
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));

    // unset 后 upstream 清空，可重新设置。
    repo.unset_upstream("main").expect("unset upstream");
    let branches = repo.branches().expect("branches after unset");
    let main = branches.iter().find(|b| b.name == "main").expect("main");
    assert_eq!(main.upstream, None);
    repo.set_upstream("main", "origin/main").expect("set upstream");
    let branches = repo.branches().expect("branches after set");
    let main = branches.iter().find(|b| b.name == "main").expect("main");
    assert_eq!(main.upstream.as_deref(), Some("origin/main"));

    // prune：远程删掉 feature 后清理本地过期的远程引用。
    temp.git(&["branch", "feature"]);
    temp.git(&["push", "origin", "feature"]);
    // 直接在裸仓库删分支（等价于服务端删除），本地 remote-tracking ref 因此过期。
    let output = Command::new("git")
        .args(["update-ref", "-d", "refs/heads/feature"])
        .current_dir(&origin_path)
        .output()
        .expect("delete remote ref");
    assert!(output.status.success(), "delete remote ref failed");
    assert!(
        repo.branches()
            .expect("branches")
            .iter()
            .any(|b| b.name == "origin/feature"),
        "stale remote branch before prune"
    );
    repo.remote_prune("origin").expect("prune");
    assert!(
        !repo
            .branches()
            .expect("branches after prune")
            .iter()
            .any(|b| b.name == "origin/feature"),
        "remote branch gone after prune"
    );

    // remove remote。
    repo.remote_remove("origin").expect("remove remote");
    assert!(
        repo.remotes().expect("remotes").is_empty(),
        "no remotes after remove"
    );

    let _ = std::fs::remove_dir_all(&origin_path);
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

    let empty = repo.diff_head(None, false).expect("diff head clean");
    assert!(parse_unified_diff(&empty).is_empty());

    temp.write("file.txt", "one\nTWO\nthree\n");
    let unstaged = repo.diff_unstaged(None, false).expect("diff unstaged");
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
    let staged = repo.diff_staged(None, false).expect("diff staged");
    let files = parse_unified_diff(&staged);
    assert_eq!(files.len(), 1);
    assert!(files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .any(|l| l.kind == DiffLineKind::Added && l.content == "TWO"));

    repo.commit("update file", false).expect("commit");
    let after = repo.diff_head(None, false).expect("diff head after commit");
    assert!(parse_unified_diff(&after).is_empty());

    let shown = repo.show_diff("HEAD", None, false).expect("show diff");
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
    let out = repo.diff_unstaged(Some("brand_new.txt"), false).expect("diff");
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

#[test]
fn reword_head_commit_via_amend() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "original message"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let head = repo.log(10).expect("log")[0].id.0.clone();

    repo.reword_commit(&head, "reworded message").expect("reword");

    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].subject, "reworded message");
    assert_ne!(log[0].id.0, head);
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "one\n");
}

#[test]
fn reword_middle_commit_via_rebase() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "first"]);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    temp.write("a.txt", "two\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    temp.write("a.txt", "three\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "third"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let log = repo.log(10).expect("log");
    assert_eq!(log[0].subject, "third");
    let target = log[1].id.0.clone();

    repo.reword_commit(&target, "renamed second").expect("reword");

    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].subject, "third");
    assert_eq!(log[1].subject, "renamed second");
    assert_eq!(log[2].subject, "first");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "three\n");
}

#[test]
fn shelve_stash_roundtrip() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    temp.write("a.txt", "shelved work\n");

    repo.stash_push(Some("my shelve"), false).expect("stash push");
    let entries = repo.stash_list().expect("stash list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].message, "On main: my shelve");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "one\n");

    repo.stash_apply_at(0).expect("stash apply");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "shelved work\n");

    repo.stash_drop_at(0).expect("stash drop");
    assert!(repo.stash_list().expect("stash list").is_empty());
}

fn setup_conflicted_repo() -> (TempRepo, Repository) {
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
    assert!(repo.rebase_run("main", &plan).is_err());
    assert!(repo.is_rebase_in_progress());
    (temp, repo)
}

#[test]
fn conflict_take_theirs_and_stage() {
    let (temp, repo) = setup_conflicted_repo();

    let conflicts = repo.conflicted_files().expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "a.txt");
    assert_eq!(conflicts[0].kind, ConflictKind::BothModified);

    repo.checkout_side("a.txt", false).expect("take theirs");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "feature\n");
    assert!(repo.conflicted_files().expect("conflicts").is_empty());
}

#[test]
fn conflict_markers_resolve_loop() {
    let (temp, repo) = setup_conflicted_repo();

    let raw = repo.conflict_file_content("a.txt").expect("read a.txt");
    let hunks = conflict_hunks(&raw);
    assert_eq!(hunks.len(), 1);
    assert_eq!(hunks[0].ours, vec!["main"]);
    assert_eq!(hunks[0].theirs, vec!["feature"]);

    repo.resolve_conflict_markers("a.txt", &raw, &[HunkChoice::Theirs])
        .expect("resolve markers");

    assert!(repo.conflicted_files().expect("conflicts").is_empty());
    repo.rebase_continue().expect("continue rebase");
    assert!(!repo.is_rebase_in_progress());
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "feature\n");
}

#[test]
fn merge_branch_fast_forward() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.git(&["checkout", "-b", "feature"]);
    temp.write("feature.txt", "feature work\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "feature work"]);
    temp.git(&["checkout", "main"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    assert!(!repo.is_merge_in_progress());

    repo.merge_branch("feature").expect("merge");
    assert!(!repo.is_merge_in_progress());

    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "feature work");
    let branches = repo.branches().expect("branches");
    let main = branches.iter().find(|b| b.name == "main").expect("main");
    let feature = branches
        .iter()
        .find(|b| b.name == "feature")
        .expect("feature");
    assert_eq!(main.commit_id, feature.commit_id, "fast-forward moved main");
    assert!(temp.path.join("feature.txt").exists());
}

#[test]
fn merge_conflict_resolve_and_continue() {
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

    let repo = Repository::open(&temp.path).expect("open repo");
    assert!(repo.merge_branch("feature").is_err(), "merge conflicts");
    assert!(repo.is_merge_in_progress());

    let conflicts = repo.conflicted_files().expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "a.txt");

    repo.checkout_side("a.txt", true).expect("take ours");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "main\n");
    assert!(repo.conflicted_files().expect("conflicts").is_empty());

    repo.merge_continue().expect("continue merge");
    assert!(!repo.is_merge_in_progress());
    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 4);
    assert_eq!(log[0].subject, "Merge branch 'feature'");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "main\n");
}

#[test]
fn undo_head_commit_keeps_changes() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let head_before = repo.log(10).expect("log")[0].id.0.clone();

    repo.undo_head_commit().expect("undo head");
    let log = repo.log(10).expect("log after undo");
    assert_eq!(log.len(), 1);
    assert_ne!(log[0].id.0, head_before);
    let status = repo.status().expect("status");
    assert!(
        status
            .changes
            .iter()
            .any(|c| c.path == "b.txt" && c.staged),
        "undo keeps changes staged"
    );

    let root = TempRepo::new();
    root.write("a.txt", "a\n");
    root.git(&["add", "."]);
    root.git(&["commit", "-m", "root"]);
    let root_repo = Repository::open(&root.path).expect("open root repo");
    root_repo.undo_head_commit().expect("undo root commit");
    assert!(root_repo.log(10).expect("log").is_empty(), "unborn head");
}

#[test]
fn drop_head_commit_discards() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    repo.drop_head_commit().expect("drop head");
    let log = repo.log(10).expect("log after drop");
    assert_eq!(log.len(), 1);
    assert!(!temp.path.join("b.txt").exists(), "drop discards the commit");
    assert!(repo.status().expect("status").changes.is_empty());

    let root = TempRepo::new();
    root.write("a.txt", "a\n");
    root.git(&["add", "."]);
    root.git(&["commit", "-m", "root"]);
    let root_repo = Repository::open(&root.path).expect("open root repo");
    assert!(
        root_repo.drop_head_commit().is_err(),
        "cannot drop the root commit"
    );
}

#[test]
fn rebase_edit_stops_and_continues() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "first"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    temp.write("c.txt", "c\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "third"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let base = repo.log(10).expect("log")[2].id.0.clone();

    let mut plan = repo.rebase_todos(&base).expect("todos");
    plan[0].kind = RebaseActionKind::Edit;
    assert!(repo.rebase_run(&base, &plan).is_err(), "edit stops rebase");
    assert!(repo.is_rebase_in_progress());

    repo.rebase_stopped_commit()
        .expect("REBASE_HEAD present at edit stop");
    let head_count = temp
        .git(&["rev-list", "--count", "HEAD"])
        .trim()
        .parse::<usize>()
        .expect("rev-list count");
    assert_eq!(head_count, 2, "HEAD is detached on the replayed second");
    let head_subject = temp.git(&["log", "-1", "--format=%s", "HEAD"]);
    assert_eq!(head_subject.trim(), "second", "stopped on the edited commit");

    temp.write("a.txt", "edited\n");
    repo.add(&["a.txt"]).expect("add edit");
    repo.commit("second edited", true).expect("amend at stop");
    repo.rebase_continue().expect("continue rebase");
    assert!(!repo.is_rebase_in_progress());

    let log = repo.log(10).expect("log after edit rebase");
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].subject, "third");
    assert_eq!(log[1].subject, "second edited");
    assert_eq!(log[2].subject, "first");
    let content = std::fs::read_to_string(temp.path.join("a.txt")).expect("read a.txt");
    assert_eq!(content, "edited\n");
}

#[test]
fn rename_branch_repoints() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.git(&["checkout", "-b", "topic"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "topic work"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let topic_head = repo.log(10).expect("log")[0].id.0.clone();

    repo.rename_branch("topic", "renamed-topic").expect("rename");

    let branches = repo.branches().expect("branches");
    assert!(
        !branches.iter().any(|b| b.name == "topic"),
        "old name is gone"
    );
    let renamed = branches
        .iter()
        .find(|b| b.name == "renamed-topic")
        .expect("renamed branch");
    assert_eq!(renamed.commit_id.0, topic_head, "rename keeps the tip");

    let status = repo.status().expect("status");
    assert_eq!(
        status.head_branch, "renamed-topic",
        "renaming the current branch updates HEAD"
    );
}

#[test]
fn force_push_and_push_tags() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let bare = std::env::temp_dir().join(format!(
        "rebased-rs-bare-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    temp.git(&["init", "--bare", bare.to_str().expect("utf8 path")]);
    temp.git(&["remote", "add", "origin", bare.to_str().expect("utf8 path")]);

    let repo = Repository::open(&temp.path).expect("open repo");
    repo.push("main", true).expect("initial push");

    temp.git(&["commit", "--amend", "-m", "amended history"]);
    assert!(
        repo.push("main", false).is_err(),
        "plain push rejects non-fast-forward"
    );
    repo.push_force("main").expect("force push");

    let bare_head = Command::new("git")
        .arg("--git-dir")
        .arg(&bare)
        .args(["log", "-1", "--format=%s", "main"])
        .output()
        .expect("inspect bare head");
    assert!(bare_head.status.success());
    assert_eq!(
        String::from_utf8_lossy(&bare_head.stdout).trim(),
        "amended history"
    );

    repo.create_tag("v1", None, None).expect("create tag");
    repo.push_tags().expect("push tags");
    let bare_tags = Command::new("git")
        .arg("--git-dir")
        .arg(&bare)
        .args(["tag"])
        .output()
        .expect("inspect bare tags");
    assert_eq!(String::from_utf8_lossy(&bare_tags.stdout).trim(), "v1");

    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn push_single_tag_pushes_only_that_ref() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let bare = std::env::temp_dir().join(format!(
        "rebased-rs-bare-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    temp.git(&["init", "--bare", bare.to_str().expect("utf8 path")]);
    temp.git(&["remote", "add", "origin", bare.to_str().expect("utf8 path")]);

    let repo = Repository::open(&temp.path).expect("open repo");
    repo.push("main", true).expect("initial push");

    repo.create_tag("keep", None, None).expect("create keep");
    repo.create_tag("ship", Some("HEAD"), Some("ship it"))
        .expect("create ship");
    repo.push_tag("ship").expect("push single tag");

    let bare_tags = Command::new("git")
        .arg("--git-dir")
        .arg(&bare)
        .args(["tag"])
        .output()
        .expect("inspect bare tags");
    assert_eq!(
        String::from_utf8_lossy(&bare_tags.stdout).trim(),
        "ship",
        "only the requested tag should reach the remote"
    );

    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn recreate_tag_rewrites_message_on_same_commit() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let head = repo.log(1).expect("log")[0].id.0.clone();
    repo.create_tag("v1", Some(&head), Some("old message"))
        .expect("create tag");

    repo.recreate_tag("v1", &head, "new message")
        .expect("recreate tag");

    let shown = Command::new("git")
        .current_dir(&temp.path)
        .args(["tag", "-l", "--format=%(contents)", "v1"])
        .output()
        .expect("read tag message");
    assert_eq!(
        String::from_utf8_lossy(&shown.stdout).trim(),
        "new message",
        "recreate_tag replaces the annotated message"
    );
}

#[test]
fn reflog_lists_head_operations() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "b\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let entries = repo.reflog(10).expect("reflog");

    assert!(!entries.is_empty(), "reflog should record the commits");
    assert_eq!(entries[0].selector, "HEAD@{0}");
    assert_eq!(entries[0].message, "commit: second");
    assert_eq!(entries[0].commit_id.len(), 40);
    assert!(
        entries.iter().any(|e| e.message == "commit (initial): initial"),
        "the root commit's reflog message marks it as initial"
    );
}

#[test]
fn interactive_rebase_squash_merges_messages() {
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
    plan[1].kind = RebaseActionKind::Squash;
    repo.rebase_run(&base, &plan).expect("rebase run");

    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "second", "squash keeps first message as subject");
    assert!(
        log[0].body.contains("third"),
        "squash keeps second message in body, got {:?}",
        log[0].body
    );
    assert!(temp.path.join("b.txt").exists());
    assert!(temp.path.join("c.txt").exists());
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn interactive_rebase_fixup_discards_message() {
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
    plan[1].kind = RebaseActionKind::Fixup;
    repo.rebase_run(&base, &plan).expect("rebase run");

    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "second", "fixup keeps first message");
    assert!(
        !log[0].body.contains("third"),
        "fixup discards second message, got {:?}",
        log[0].body
    );
    assert!(temp.path.join("b.txt").exists());
    assert!(temp.path.join("c.txt").exists());
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn commit_rejects_empty_staging() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    assert!(
        repo.commit("empty commit", false).is_err(),
        "commit with nothing staged must fail"
    );

    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 1, "no commit must be created");
    assert_eq!(log[0].subject, "initial");
}

#[test]
fn ignored_files_excluded_from_status() {
    let temp = TempRepo::new();
    temp.write(".gitignore", "ignored.log\nbuild/\n");
    temp.write("tracked.txt", "tracked\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("ignored.log", "noise\n");
    std::fs::create_dir_all(temp.path.join("build")).expect("mkdir build");
    temp.write("build/artifact.o", "blob\n");

    let repo = Repository::open(&temp.path).expect("open repo");
    let status = repo.status().expect("status");
    assert!(status.changes.is_empty(), "ignored entries must not appear");

    let data = load_repo_data(&repo, DEFAULT_LOG_LIMIT).expect("load");
    assert!(data.status.changes.is_empty(), "snapshot must hide ignored files");
}

#[test]
fn merge_creates_two_parent_commit() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.git(&["checkout", "-b", "feature"]);
    temp.write("feature.txt", "feature\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "feature work"]);
    temp.git(&["checkout", "main"]);
    temp.write("main.txt", "main\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "main work"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    repo.merge_branch("feature").expect("merge");

    let log = repo.log(10).expect("log");
    assert_eq!(log.len(), 4, "log --all counts both parent branches");
    let head = &log[0];
    assert_eq!(head.parents.len(), 2, "diverged merge yields two parents");
    assert_ne!(log[1].id, log[2].id);
    assert!(head.parents.contains(&log[1].id));
    assert!(head.parents.contains(&log[2].id));

    let subjects: Vec<&str> = [log[1].subject.as_str(), log[2].subject.as_str()].to_vec();
    assert!(subjects.contains(&"feature work"));
    assert!(subjects.contains(&"main work"));
    assert!(temp.path.join("feature.txt").exists());
    assert!(temp.path.join("main.txt").exists());
}

#[test]
fn rebase_reword_injects_custom_message() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "first"]);
    temp.write("b.txt", "two\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    temp.write("c.txt", "three\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "third"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let base = repo.log(10).expect("log")[2].id.0.clone();

    // 规划阶段为中间提交注入自定义消息（Reword + message），验证能真实改写提交消息。
    let mut plan = repo.rebase_todos(&base).expect("todos");
    assert_eq!(plan.len(), 2);
    plan[0].kind = RebaseActionKind::Reword;
    plan[0].message = Some("rewritten second".to_string());
    repo.rebase_run(&base, &plan).expect("rebase run");

    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].subject, "third");
    assert_eq!(log[1].subject, "rewritten second", "custom message applied");
    assert_eq!(log[2].subject, "first");
    assert!(temp.path.join("b.txt").exists());
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn rebase_squash_injects_custom_message() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "two\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "second"]);
    temp.write("c.txt", "three\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "third"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let base = repo.log(10).expect("log")[2].id.0.clone();

    let mut plan = repo.rebase_todos(&base).expect("todos");
    // 把后一个提交 squash 进前一个，并覆盖合并后的消息。
    plan[1].kind = RebaseActionKind::Squash;
    plan[1].message = Some("squashed into one\n\ncombined body".to_string());
    repo.rebase_run(&base, &plan).expect("rebase run");

    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "squashed into one", "squash custom message applied");
    assert!(log[0].body.contains("combined body"));
    assert!(temp.path.join("b.txt").exists());
    assert!(temp.path.join("c.txt").exists());
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn rebase_autosquash_reorders_fixup_commits() {
    let temp = TempRepo::new();
    temp.write("a.txt", "one\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);
    temp.write("b.txt", "two\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "add feature"]);
    temp.write("c.txt", "three\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "unrelated"]);
    temp.write("b.txt", "two\nfixed\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "fixup! add feature"]);

    let repo = Repository::open(&temp.path).expect("open repo");
    let base = repo.log(10).expect("log")[3].id.0.clone();

    let plan = repo.rebase_todos(&base).expect("todos");
    assert_eq!(plan.len(), 3);
    assert_eq!(plan[0].subject, "add feature");
    assert_eq!(plan[1].subject, "unrelated");
    assert_eq!(plan[2].subject, "fixup! add feature");

    // autosquash 重排：fixup 移到目标提交之后，kind 改为 Fixup。
    let plan = autosquash_plan(plan);
    assert_eq!(plan[0].subject, "add feature");
    assert_eq!(plan[1].subject, "fixup! add feature");
    assert_eq!(plan[1].kind, RebaseActionKind::Fixup);
    assert_eq!(plan[2].subject, "unrelated");
    assert_eq!(plan[2].kind, RebaseActionKind::Pick);

    repo.rebase_run(&base, &plan).expect("rebase run");

    // fixup 合并后剩 3 个提交，fixup 的改动并入目标提交。
    let log = repo.log(10).expect("log after rebase");
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].subject, "unrelated");
    assert_eq!(log[1].subject, "add feature");
    assert_eq!(log[2].subject, "initial");
    let merged = std::fs::read_to_string(temp.path.join("b.txt")).expect("read b.txt");
    assert!(merged.contains("fixed"), "fixup content merged into target");
    assert!(!repo.is_rebase_in_progress());
}

#[test]
fn fetch_updates_remote_refs() {
    let temp = TempRepo::new();
    temp.write("a.txt", "a\n");
    temp.git(&["add", "."]);
    temp.git(&["commit", "-m", "initial"]);

    let suffix = format!(
        "{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let bare = std::env::temp_dir().join(format!("rebased-rs-fetch-bare-{suffix}"));
    let clone = std::env::temp_dir().join(format!("rebased-rs-fetch-clone-{suffix}"));

    temp.git(&["init", "--bare", "--initial-branch=main", bare.to_str().expect("utf8 path")]);
    temp.git(&["remote", "add", "origin", bare.to_str().expect("utf8 path")]);

    let repo = Repository::open(&temp.path).expect("open repo");
    repo.push("main", true).expect("initial push");

    let clone_out = Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(bare.to_str().expect("utf8 path"))
        .arg(&clone)
        .env("GIT_AUTHOR_NAME", "Test User")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test User")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("clone");
    assert!(
        clone_out.status.success(),
        "clone failed: {}",
        String::from_utf8_lossy(&clone_out.stderr)
    );

    std::fs::write(clone.join("remote.txt"), "remote work\n").expect("write");
    let steps: [&[&str]; 3] = [
        &["add", "."],
        &["commit", "-m", "remote work"],
        &["push", "origin", "main"],
    ];
    for args in steps {
        let out = Command::new("git")
            .current_dir(&clone)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Test User")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test User")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("run git in clone");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let clone_head = Command::new("git")
        .arg("-C")
        .arg(&clone)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("clone head");
    let clone_head = String::from_utf8_lossy(&clone_head.stdout).trim().to_string();

    repo.fetch().expect("fetch");

    let origin_main = Command::new("git")
        .arg("-C")
        .arg(&temp.path)
        .args(["rev-parse", "origin/main"])
        .output()
        .expect("origin ref");
    assert!(origin_main.status.success(), "origin/main must exist");
    assert_eq!(
        String::from_utf8_lossy(&origin_main.stdout).trim(),
        clone_head,
        "fetch advanced origin/main"
    );

    let status = repo.status().expect("status");
    assert_eq!(status.head_branch, "main");

    let _ = std::fs::remove_dir_all(&bare);
    let _ = std::fs::remove_dir_all(&clone);
}

#[test]
fn diff_ignore_whitespace_suppresses_whitespace_only_changes() {
    let temp = TempRepo::new();
    let repo = Repository::open(&temp.path).expect("open repo");

    temp.write("ws.txt", "alpha line\nbeta line\n");
    temp.git(&["add", "ws.txt"]);
    temp.git(&["commit", "-m", "add ws.txt"]);

    // 只改空白：行内容非空白部分不变
    temp.write("ws.txt", "  alpha   line\n\tbeta line\n");
    temp.git(&["add", "ws.txt"]);

    let normal = repo.diff_staged(Some("ws.txt"), false).expect("normal diff");
    let normal_files = parse_unified_diff(&normal);
    assert!(
        !normal_files.iter().all(|f| f.hunks.is_empty()),
        "normal diff should show whitespace-only change"
    );

    let ignored = repo.diff_staged(Some("ws.txt"), true).expect("ignored diff");
    let ignored_files = parse_unified_diff(&ignored);
    assert!(
        ignored_files.iter().all(|f| f.hunks.is_empty()),
        "-w diff should suppress whitespace-only change: {ignored}"
    );
}
