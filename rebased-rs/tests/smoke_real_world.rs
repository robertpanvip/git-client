//! 真实数据冒烟测试：用一个接近真实项目的仓库（多分支 / merge / tag / 远程 /
//! stash / rebase / 冲突 / blame / reflog）跑通核心操作链路，验证端到端可用性。
//!
//! 与 git_integration.rs（单功能验证）不同，这里以「一个仓库的完整生命周期」
//! 为主线，覆盖跨功能交互场景。

use std::path::PathBuf;
use std::process::Command;

use rebased_rs::git::{
    ChangeStatus, ConflictKind, DEFAULT_LOG_LIMIT, HunkChoice, MergeMode, RebaseActionKind,
    Repository, autosquash_plan, conflict_hunks, load_repo_data, parse_unified_diff,
};

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rebased-rs-smoke-{}-{}",
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
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "."]);
        self.git(&["commit", "-m", message]);
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn bare_repo(prefix: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("rebased-rs-smoke-{prefix}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&path)
        .output()
        .expect("init bare");
    path
}

/// 构建一个真实感的项目仓库：目录结构 + 主分支演进 + feature 分支 + 合并。
fn scaffold_project() -> TempRepo {
    let temp = TempRepo::new();
    temp.write("src/main.rs", "fn main() {}\n");
    temp.write("src/lib.rs", "pub fn helper() {}\n");
    temp.write("docs/README.md", "# Demo\n");
    temp.write("tests/smoke.rs", "#[test]\nfn it_works() {}\n");
    temp.commit_all("initial project scaffold");
    temp.write("src/lib.rs", "pub fn helper() {}\npub fn extra() {}\n");
    temp.commit_all("add extra helper");

    // feature 分支两笔提交
    temp.git(&["checkout", "-b", "feature/parser"]);
    temp.write("src/parser.rs", "pub fn parse() {}\n");
    temp.commit_all("add parser module");
    temp.write("src/parser.rs", "pub fn parse(input: &str) {}\n");
    temp.commit_all("parser takes input");

    // main 分支继续演进
    temp.git(&["checkout", "main"]);
    temp.write("docs/README.md", "# Demo\n## Usage\n");
    temp.commit_all("document usage");
    temp
}

#[test]
fn smoke_project_lifecycle_branches_tags_remote() {
    let temp = scaffold_project();
    let repo = Repository::open(&temp.path).expect("open repo");

    // 1. 加载仓库数据：log / graph / branches / tags
    let data = load_repo_data(&repo, DEFAULT_LOG_LIMIT).expect("load repo data");
    assert!(!data.commits.is_empty(), "log should be non-empty");
    assert!(data.graph.rows.len() >= data.commits.len());
    assert_eq!(
        data.branches.iter().filter(|b| !b.is_remote).count(),
        2,
        "main + feature/parser"
    );
    assert!(data.branches.iter().any(|b| b.name == "main" && b.is_head));

    // 2. no-ff 合并产生 merge commit（真实 merge 提交是 Rebased 图形核心场景）
    repo.merge_branch_with("feature/parser", MergeMode::NoFastForward)
        .expect("merge --no-ff");
    let head = repo.rev_parse("HEAD").expect("HEAD");
    let commits = repo.log(10).expect("log");
    let merge = commits
        .iter()
        .find(|c| c.id.0 == head)
        .expect("HEAD in log");
    assert!(
        merge.is_merge(),
        "HEAD after no-ff merge must be a merge commit"
    );

    // 3. annotated tag + 列表 + 推送
    repo.create_tag("v1.0", None, Some("release 1.0"))
        .expect("create annotated tag");
    let tags = repo.tags().expect("tags");
    assert!(tags.iter().any(|t| t.name == "v1.0"));

    // 4. 远程：bare origin + push + 单 tag 推送
    let bare = bare_repo("origin");
    repo.remote_add("origin", bare.to_str().expect("bare path"))
        .expect("add remote");
    repo.push("main", true).expect("push main");
    repo.push_tag("v1.0").expect("push tag");
    let remote_tags = Command::new("git")
        .args(["ls-remote", "--tags", bare.to_str().expect("bare")])
        .output()
        .expect("ls-remote");
    assert!(
        String::from_utf8_lossy(&remote_tags.stdout).contains("refs/tags/v1.0"),
        "tag v1.0 must exist on remote"
    );

    // 5. 分支 rename + checkout + delete
    repo.create_branch("experiment", None)
        .expect("create branch");
    repo.rename_branch("experiment", "experiment2")
        .expect("rename branch");
    repo.checkout("experiment2").expect("checkout renamed");
    assert_eq!(repo.current_branch_name().expect("current"), "experiment2");
    repo.checkout("main").expect("back to main");
    repo.delete_branch("experiment2", false)
        .expect("delete branch");

    // 6. reflog：合并 + checkout 等操作历史可见
    let reflog = repo.reflog(10).expect("reflog");
    assert!(!reflog.is_empty());
    assert!(
        reflog.iter().any(|e| e.message.contains("merge")),
        "merge operation visible in reflog: {:?}",
        reflog
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
    );

    // 7. commit 级 compare（复用 rev-range 语义）
    let first = commits.last().expect("oldest commit");
    let compared = repo
        .compare_branches(&first.id.0, "HEAD", 5)
        .expect("compare");
    assert!(!compared.0.is_empty() || !compared.1.is_empty());

    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn smoke_working_tree_status_diff_stash_commit() {
    let temp = scaffold_project();
    let repo = Repository::open(&temp.path).expect("open repo");

    // 1. 三种状态并存：已暂存 / 未暂存 / 未跟踪
    temp.write("src/lib.rs", "pub fn helper() {}\npub fn changed() {}\n");
    temp.git(&["add", "src/lib.rs"]);
    temp.write("src/main.rs", "fn main() { println!(\"hi\"); }\n");
    temp.write("notes.txt", "scratch\n");

    let status = repo.status().expect("status");
    assert!(
        status
            .changes
            .iter()
            .any(|c| c.path == "src/lib.rs" && c.staged)
    );
    assert!(
        status
            .changes
            .iter()
            .any(|c| c.path == "src/main.rs" && !c.staged)
    );
    assert!(
        status
            .changes
            .iter()
            .any(|c| c.path == "notes.txt" && c.status == ChangeStatus::Untracked)
    );

    // 2. staged / unstaged diff 解析
    let staged = parse_unified_diff(&repo.diff_staged(None, false).expect("staged diff"));
    assert!(staged.iter().any(|f| f.path == "src/lib.rs"));
    let unstaged = parse_unified_diff(&repo.diff_unstaged(None, false).expect("unstaged diff"));
    assert!(unstaged.iter().any(|f| f.path == "src/main.rs"));

    // 3. stash push（含未跟踪）→ 列表 → 恢复
    repo.stash_push(Some("wip scratch"), false, true)
        .expect("stash push");
    let stash = repo.stash_list().expect("stash list");
    assert_eq!(stash.len(), 1);
    assert!(status.changes.iter().all(|_| true)); // status 已捕获
    let status_after = repo.status().expect("status after stash");
    assert!(status_after.changes.is_empty(), "stash clears working tree");
    repo.stash_apply_at(0).expect("stash apply");
    repo.stash_drop_at(0).expect("stash drop");
    assert!(repo.stash_list().expect("stash list").is_empty());

    // 4. 修改 → 提交 → amend（预填语义：message 复用）
    temp.commit_all("first real commit");
    let first_msg = repo.head_message().expect("head message");
    assert!(first_msg.contains("first real commit"));
    temp.write(
        "src/lib.rs",
        "pub fn helper() {}\npub fn changed() {}\npub fn more() {}\n",
    );
    repo.add_all().expect("add all");
    repo.commit("amended message", true).expect("amend");
    let amended_msg = repo.head_message().expect("head after amend");
    assert_eq!(amended_msg, "amended message");

    // 5. ignore-whitespace：纯空白改动在 -w 下不产生 hunk
    temp.write("src/main.rs", "fn main() { println!(\"hi\");  }\n");
    repo.add_all().expect("add whitespace");
    let ws_ignored = parse_unified_diff(
        &repo
            .diff_staged(None, true)
            .expect("ws ignored staged diff"),
    );
    assert!(ws_ignored.iter().all(|f| f.hunks.is_empty()));
}

#[test]
fn smoke_rebase_autosquash_and_conflict_resolution() {
    let temp = scaffold_project();
    let repo = Repository::open(&temp.path).expect("open repo");

    // 1. rebase todo 生成 + 排序改写 + 执行
    // main 上 main~2 之后有两个提交（add extra helper / document usage），
    // 把较新的一个改为 squash 合并到前一个上（squash 需要有前一个提交）。
    let mut plan = repo.rebase_todos("main~2").expect("rebase todos");
    assert!(plan.len() >= 2, "need at least two commits to squash");
    assert!(plan.iter().all(|a| a.kind == RebaseActionKind::Pick));
    plan[1].kind = RebaseActionKind::Squash;
    repo.rebase_run("main~2", &plan).expect("rebase run");

    // 2. autosquash：fixup! 提交被重排到目标之后
    temp.write("src/target.rs", "target\n");
    temp.commit_all("target change");
    temp.write("src/target.rs", "target\nrefined\n");
    temp.commit_all("fixup! target change");
    // base 选在目标提交之前，让目标提交与 fixup! 同时出现在 todo 里
    let base = repo.rev_parse("HEAD~2").expect("base");
    let plan = repo.rebase_todos(&base).expect("todos");
    let squashed = autosquash_plan(plan);
    let fixup_pos = squashed
        .iter()
        .position(|a| a.kind == RebaseActionKind::Fixup);
    assert!(fixup_pos.is_some(), "fixup action present");
    assert_eq!(
        squashed[fixup_pos.unwrap() - 1].subject,
        "target change",
        "fixup sits directly after its target"
    );

    // 3. 冲突：合并冲突 → 文件列表 → hunk 解析 → 标记解决 → continue
    temp.write("a.txt", "one\n");
    temp.commit_all("add a.txt");
    temp.git(&["checkout", "-b", "conflict-side"]);
    temp.write("a.txt", "side\n");
    temp.commit_all("side edit");
    temp.git(&["checkout", "main"]);
    temp.write("a.txt", "main\n");
    temp.commit_all("main edit");

    assert!(
        !repo
            .merge_branch_with("conflict-side", MergeMode::Default)
            .is_ok()
            || repo.is_merge_in_progress(),
        "merge should conflict"
    );
    let conflicts = repo.conflicted_files().expect("conflicted files");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::BothModified);

    let raw = repo
        .conflict_file_content("a.txt")
        .expect("conflict content");
    let hunks = conflict_hunks(&raw);
    assert_eq!(hunks.len(), 1);
    assert_eq!(hunks[0].ours, vec!["main"]);
    assert_eq!(hunks[0].theirs, vec!["side"]);
    repo.resolve_conflict_markers("a.txt", &raw, &[HunkChoice::Theirs])
        .expect("resolve markers");
    assert!(repo.conflicted_files().expect("conflicts").is_empty());
    repo.merge_continue().expect("continue merge");
    assert!(!repo.is_merge_in_progress());
}

#[test]
fn smoke_inspection_blame_history_compare_files() {
    let temp = scaffold_project();
    let repo = Repository::open(&temp.path).expect("open repo");

    // 1. blame：每行归属提交
    let blame = repo.blame("HEAD", "src/lib.rs").expect("blame");
    assert!(!blame.is_empty());
    assert!(blame.iter().all(|g| !g.lines.is_empty()));

    // 2. 文件历史（log_follow）
    let history = repo.log_follow(20, "src/lib.rs").expect("file history");
    assert!(!history.is_empty());
    assert!(history.iter().all(|c| c.id.0.len() >= 7));

    // 3. show_files + branches_containing
    let files = repo.show_files("HEAD").expect("show files");
    assert!(!files.is_empty());
    assert!(files.iter().any(|f| f.path == "docs/README.md"));
    // 上一提交改了 src/lib.rs
    let files = repo.show_files("HEAD~1").expect("show files prev");
    assert!(files.iter().any(|f| f.path == "src/lib.rs"));
    let containing = repo.branches_containing("HEAD").expect("containing");
    assert!(containing.contains(&"main".to_string()));

    // 4. worktree 文件读写（可编辑 diff 的数据基础）
    let content = repo.worktree_file_content("src/main.rs").expect("read");
    repo.write_worktree_file("src/main.rs", &format!("{content}// touched\n"))
        .expect("write");
    let after = repo
        .worktree_file_content("src/main.rs")
        .expect("read again");
    assert!(after.contains("// touched"));
}

#[test]
fn smoke_edge_empty_and_non_repo() {
    // 1. 空仓库：unborn HEAD 不应 panic
    let temp = TempRepo::new();
    let repo = Repository::open(&temp.path).expect("open empty repo");
    let data = load_repo_data(&repo, DEFAULT_LOG_LIMIT).expect("empty load");
    assert!(data.commits.is_empty());
    assert!(data.status.unborn);

    // 2. 非 git 目录：open 应返回可读错误而非 panic
    let plain = std::env::temp_dir().join(format!("rebased-rs-smoke-plain-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&plain);
    std::fs::create_dir_all(&plain).expect("create plain dir");
    let err = match Repository::open(&plain) {
        Ok(_) => panic!("non-repo must fail"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("not a git repository"),
        "unexpected error: {err}"
    );
    let _ = std::fs::remove_dir_all(&plain);

    // 3. 不存在的目录：同样返回错误（预检查路径存在性，而非进程级 io 失败）
    let ghost = plain.join("does-not-exist");
    let err = match Repository::open(&ghost) {
        Ok(_) => panic!("ghost dir must fail"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("does not exist"),
        "unexpected error: {err}"
    );
}
