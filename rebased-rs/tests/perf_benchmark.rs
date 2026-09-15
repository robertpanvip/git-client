use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rebased_rs::git::{load_repo_data, Repository, DEFAULT_LOG_LIMIT};

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn with_commits(commits: usize) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rebased-rs-perf-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        let repo = Self { path };
        repo.git(&["init", "-b", "main"]);
        repo.fast_import(commits);
        repo.git(&["reset", "--hard"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf8")
    }

    fn fast_import(&self, commits: usize) {
        let mut stream = String::new();
        for i in 0..commits {
            let message = format!("commit {i}");
            let content = format!("line-{i:04}\n");
            let stamp = 1_700_000_000 + i as i64;
            stream.push_str("commit refs/heads/main\n");
            stream.push_str(&format!("mark :{}\n", i + 1));
            stream.push_str(&format!(
                "author Test User <test@example.com> {stamp} +0000\n"
            ));
            stream.push_str(&format!(
                "committer Test User <test@example.com> {stamp} +0000\n"
            ));
            stream.push_str(&format!("data {}\n{message}\n", message.len()));
            stream.push_str("M 100644 inline data.log\n");
            stream.push_str(&format!("data {}\n{content}\n", content.len()));
        }
        let mut child = Command::new("git")
            .current_dir(&self.path)
            .args(["fast-import", "--quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn git fast-import");
        child
            .stdin
            .take()
            .expect("fast-import stdin")
            .write_all(stream.as_bytes())
            .expect("write fast-import stream");
        let output = child.wait_with_output().expect("wait fast-import");
        assert!(
            output.status.success(),
            "fast-import failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn assert_within(label: &str, elapsed: Duration, budget: Duration) {
    println!("{label}: {:.1?} (budget {budget:?})", elapsed);
    assert!(
        elapsed < budget,
        "{label} took {:.1?}, budget {budget:?}",
        elapsed
    );
}

#[test]
#[ignore = "performance benchmark; run explicitly with: cargo test --test perf_benchmark -- --ignored --nocapture"]
fn large_repo_read_benchmarks() {
    const COMMITS: usize = 1000;
    const BUDGET: Duration = Duration::from_secs(10);
    let temp = TempRepo::with_commits(COMMITS);
    let repo = Repository::open(&temp.path).expect("open repo");

    let start = Instant::now();
    let log = repo.log(COMMITS + 1).expect("log");
    assert_within("log(1000 commits)", start.elapsed(), BUDGET);
    assert_eq!(log.len(), COMMITS);
    assert_eq!(log[0].subject, format!("commit {}", COMMITS - 1));

    let start = Instant::now();
    let status = repo.status().expect("status");
    assert_within("status", start.elapsed(), BUDGET);
    assert!(status.changes.is_empty());

    let start = Instant::now();
    let data = load_repo_data(&repo, DEFAULT_LOG_LIMIT).expect("load repo data");
    assert_within("load_repo_data(500)", start.elapsed(), BUDGET);
    assert_eq!(data.commits.len(), DEFAULT_LOG_LIMIT);

    let start = Instant::now();
    let diff = repo.diff_head(None, false).expect("diff head");
    assert_within("diff_head", start.elapsed(), BUDGET);
    assert!(diff.is_empty());

    let base = log[COMMITS / 2].id.0.clone();
    let start = Instant::now();
    let todos = repo.rebase_todos(&base).expect("rebase todos");
    assert_within("rebase_todos(500)", start.elapsed(), BUDGET);
    assert_eq!(todos.len(), COMMITS / 2);

    let head = log[0].id.0.clone();
    let start = Instant::now();
    let blame = repo.blame(&head, "data.log").expect("blame");
    assert_within("blame(1000 commits)", start.elapsed(), BUDGET);
    assert!(!blame.is_empty());
}
