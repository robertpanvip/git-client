use std::path::Path;

use super::command::GitCommand;
use super::error::Result;
use super::types::{ChangeStatus, DiffLine, DiffLineKind, FileDiff, Hunk};

pub fn diff_unstaged(cmd: &GitCommand, path: Option<&str>, ignore_ws: bool) -> Result<String> {
    let mut args = vec!["diff", "-U3"];
    if ignore_ws {
        args.push("-w");
    }
    if let Some(p) = path {
        args.push("--");
        args.push(p);
    }
    let mut output = cmd.run(&args)?;
    append_untracked_diffs(cmd, path, &mut output);
    Ok(output)
}

pub fn diff_staged(cmd: &GitCommand, path: Option<&str>, ignore_ws: bool) -> Result<String> {
    let mut args = vec!["diff", "--cached", "-U3"];
    if ignore_ws {
        args.push("-w");
    }
    if let Some(p) = path {
        args.push("--");
        args.push(p);
    }
    cmd.run(&args)
}

pub fn diff_head(cmd: &GitCommand, path: Option<&str>, ignore_ws: bool) -> Result<String> {
    let mut args = vec!["diff", "HEAD", "-U3"];
    if ignore_ws {
        args.push("-w");
    }
    if let Some(p) = path {
        args.push("--");
        args.push(p);
    }
    let mut output = cmd.run(&args)?;
    append_untracked_diffs(cmd, path, &mut output);
    Ok(output)
}

fn append_untracked_diffs(cmd: &GitCommand, path: Option<&str>, output: &mut String) {
    let mut args = vec!["ls-files", "--others", "--exclude-standard", "-z"];
    if let Some(p) = path {
        args.push("--");
        args.push(p);
    }
    let Ok(listing) = cmd.run(&args) else {
        return;
    };
    for file in listing.split('\0').filter(|s| !s.is_empty()) {
        if let Some(chunk) = synthetic_new_file_diff(cmd.workdir(), file) {
            output.push('\n');
            output.push_str(&chunk);
        }
    }
}

fn synthetic_new_file_diff(workdir: &Path, file: &str) -> Option<String> {
    let bytes = std::fs::read(workdir.join(file)).ok()?;
    if bytes.starts_with(&[0u8]) {
        return Some(format!(
            "diff --git a/{file} b/{file}\nnew file mode 100644\nuntracked file\nBinary files a/{file} and b/{file} differ\n"
        ));
    }
    let content = String::from_utf8_lossy(&bytes);
    let count = content.lines().count();
    let mut out = format!(
        "diff --git a/{file} b/{file}\nnew file mode 100644\nuntracked file\nindex 0000000..1111111\n--- /dev/null\n+++ b/{file}\n@@ -0,0 +1,{count} @@\n"
    );
    for line in content.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

pub fn show_diff(
    cmd: &GitCommand,
    commit: &str,
    path: Option<&str>,
    ignore_ws: bool,
) -> Result<String> {
    let mut args = vec!["show", "--format=", "-U3"];
    if ignore_ws {
        args.push("-w");
    }
    args.push(commit);
    if let Some(p) = path {
        args.push("--");
        args.push(p);
    }
    let output = cmd.run(&args)?;
    Ok(output)
}

fn strip_path_prefix(raw: &str) -> &str {
    raw.strip_prefix("b/")
        .or_else(|| raw.strip_prefix("a/"))
        .unwrap_or(raw)
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let rest = line.trim_start_matches('@').trim();
    let old_part = rest.split_whitespace().next()?;
    let new_part = rest.split_whitespace().nth(1)?;
    let old_start = old_part
        .trim_start_matches('-')
        .split(',')
        .next()?
        .parse()
        .ok()?;
    let new_start = new_part
        .trim_start_matches('+')
        .split(',')
        .next()?
        .parse()
        .ok()?;
    Some((old_start, new_start))
}

pub fn parse_unified_diff(stdout: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current: Option<FileDiff> = None;
    let mut hunk: Option<Hunk> = None;
    let mut old_no: u32 = 0;
    let mut new_no: u32 = 0;

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(mut file) = current.take() {
                if let Some(h) = hunk.take() {
                    file.hunks.push(h);
                }
                files.push(file);
            }
            let (a_side, b_side) = split_diff_git_paths(rest);
            let old_path = a_side.map(|a| strip_path_prefix(&a).to_string());
            let path = b_side
                .map(|b| strip_path_prefix(&b).to_string())
                .unwrap_or_default();
            current = Some(FileDiff {
                path,
                old_path,
                is_new: false,
                is_deleted: false,
                is_binary: false,
                status: None,
                hunks: Vec::new(),
            });
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        if line.starts_with("new file mode") {
            file.is_new = true;
            file.status = Some(ChangeStatus::Added);
        } else if line.starts_with("deleted file mode") {
            file.is_deleted = true;
            file.status = Some(ChangeStatus::Deleted);
        } else if line.starts_with("untracked file") {
            // 合成 untracked diff 的标记行：状态语义优先于 new file 推导。
            file.status = Some(ChangeStatus::Untracked);
        } else if line.starts_with("Binary files") {
            file.is_binary = true;
        } else if let Some(rest) = line.strip_prefix("rename from ") {
            file.old_path = Some(rest.to_string());
            file.status = Some(ChangeStatus::Renamed);
        } else if let Some(rest) = line.strip_prefix("rename to ") {
            file.path = rest.to_string();
            file.status = Some(ChangeStatus::Renamed);
        } else if let Some(rest) = line.strip_prefix("copy from ") {
            file.old_path = Some(rest.to_string());
            file.status = Some(ChangeStatus::Copied);
        } else if let Some(rest) = line.strip_prefix("copy to ") {
            file.path = rest.to_string();
            file.status = Some(ChangeStatus::Copied);
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            if rest != "/dev/null" {
                file.path = strip_path_prefix(rest).to_string();
            }
        } else if let Some(rest) = line.strip_prefix("--- ") {
            if rest != "/dev/null" {
                file.old_path = Some(strip_path_prefix(rest).to_string());
            }
        } else if line.starts_with("@@ ") {
            if let Some(h) = hunk.take() {
                file.hunks.push(h);
            }
            if let Some((old_start, new_start)) = parse_hunk_header(line) {
                old_no = old_start;
                new_no = new_start;
                hunk = Some(Hunk {
                    header: line.to_string(),
                    lines: Vec::new(),
                });
            }
        } else if hunk.is_some() {
            let hunk_ref = hunk.as_mut().expect("hunk checked");
            let parsed = if let Some(content) = line.strip_prefix('+') {
                let out = DiffLine {
                    kind: DiffLineKind::Added,
                    old_no: None,
                    new_no: Some(new_no),
                    content: content.to_string(),
                };
                new_no += 1;
                Some(out)
            } else if let Some(content) = line.strip_prefix('-') {
                let out = DiffLine {
                    kind: DiffLineKind::Deleted,
                    old_no: Some(old_no),
                    new_no: None,
                    content: content.to_string(),
                };
                old_no += 1;
                Some(out)
            } else if let Some(content) = line.strip_prefix(' ') {
                let out = DiffLine {
                    kind: DiffLineKind::Context,
                    old_no: Some(old_no),
                    new_no: Some(new_no),
                    content: content.to_string(),
                };
                old_no += 1;
                new_no += 1;
                Some(out)
            } else {
                None
            };
            if let Some(out) = parsed {
                hunk_ref.lines.push(out);
            }
        }
    }

    if let Some(mut file) = current.take() {
        if let Some(h) = hunk.take() {
            file.hunks.push(h);
        }
        files.push(file);
    }
    files
}

fn split_diff_git_paths(rest: &str) -> (Option<String>, Option<String>) {
    if let Some(marker) = rest.find(" b/") {
        let a = rest[..marker].strip_prefix("a/").map(str::to_string);
        let b = rest[marker + 1..].to_string();
        (a, Some(b))
    } else {
        (None, None)
    }
}

/// 从解析后的 FileDiff 重建单个 hunk 的 patch 文本（含文件级头），
/// 供 `git apply --cached [-R] -` 使用。
/// 注意：parse_unified_diff 会丢弃 "\ No newline at end of file" 行，
/// 跨文件末尾改动的 hunk 重建后不含该标记（P1 已知限制）。
pub fn hunk_patch(file: &FileDiff, hunk_index: usize) -> String {
    let Some(hunk) = file.hunks.get(hunk_index) else {
        return String::new();
    };
    let old_seg = file.old_path.as_deref().unwrap_or(&file.path);
    let mut out = format!("diff --git a/{old_seg} b/{}\n", file.path);
    if file.is_new {
        out.push_str("new file mode 100644\n");
    }
    if file.is_deleted {
        out.push_str("deleted file mode 100644\n");
    }
    let old_path = if file.is_new {
        "/dev/null".to_string()
    } else {
        format!("a/{old_seg}")
    };
    let new_path = if file.is_deleted {
        "/dev/null".to_string()
    } else {
        format!("b/{}", file.path)
    };
    out.push_str(&format!("--- {old_path}\n"));
    out.push_str(&format!("+++ {new_path}\n"));
    out.push_str(&hunk.header);
    out.push('\n');
    for line in &hunk.lines {
        out.push_str(line.kind.prefix());
        out.push_str(&line.content);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1234567..89abcde 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,5 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
+    println!(\"extra\");
 }
+
";

    #[test]
    fn parses_basic_diff() {
        let files = parse_unified_diff(SAMPLE);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "src/lib.rs");
        assert!(!file.is_new);
        assert!(!file.is_binary);
        assert_eq!(file.hunks.len(), 1);
        let lines = &file.hunks[0].lines;
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].kind, DiffLineKind::Context);
        assert_eq!(lines[0].old_no, Some(1));
        assert_eq!(lines[0].new_no, Some(1));
        assert_eq!(lines[1].kind, DiffLineKind::Deleted);
        assert_eq!(lines[1].content, "    println!(\"old\");");
        assert_eq!(lines[1].old_no, Some(2));
        assert_eq!(lines[1].new_no, None);
        assert_eq!(lines[2].kind, DiffLineKind::Added);
        assert_eq!(lines[2].new_no, Some(2));
        assert_eq!(lines[3].kind, DiffLineKind::Added);
        assert_eq!(lines[3].new_no, Some(3));
        assert_eq!(lines[4].kind, DiffLineKind::Context);
        assert_eq!(lines[4].content, "}");
        assert_eq!(lines[4].old_no, Some(3));
        assert_eq!(lines[4].new_no, Some(4));
        assert_eq!(lines[5].kind, DiffLineKind::Added);
        assert_eq!(lines[5].content, "");
        assert_eq!(lines[5].new_no, Some(5));
    }

    #[test]
    fn parses_new_file() {
        let out = "\
diff --git a/created.txt b/created.txt
new file mode 100644
index 0000000..1111111
--- /dev/null
+++ b/created.txt
@@ -0,0 +1,2 @@
+hello
+world
";
        let files = parse_unified_diff(out);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "created.txt");
        assert!(file.is_new);
        assert_eq!(file.hunks[0].lines.len(), 2);
        assert_eq!(file.hunks[0].lines[0].new_no, Some(1));
    }

    #[test]
    fn rebuilds_hunk_patch() {
        let files = parse_unified_diff(SAMPLE);
        let patch = hunk_patch(&files[0], 0);
        assert!(
            patch.starts_with("diff --git a/src/lib.rs b/src/lib.rs\n"),
            "patch should start with file header: {patch}"
        );
        assert!(patch.contains("--- a/src/lib.rs\n"));
        assert!(patch.contains("+++ b/src/lib.rs\n"));
        assert!(patch.contains("@@ -1,4 +1,5 @@\n"));
        assert!(patch.contains(" fn main() {\n"));
        assert!(patch.contains("-    println!(\"old\");\n"));
        assert!(patch.contains("+    println!(\"new\");\n"));
        assert!(patch.ends_with("+\n"));
    }

    #[test]
    fn hunk_patch_out_of_range_is_empty() {
        let files = parse_unified_diff(SAMPLE);
        assert_eq!(hunk_patch(&files[0], 9), "");
    }

    #[test]
    fn rebuilds_new_file_hunk_patch() {
        let files = parse_unified_diff(
            "diff --git a/created.txt b/created.txt\nnew file mode 100644\n--- /dev/null\n+++ b/created.txt\n@@ -0,0 +1,2 @@\n+hello\n+world\n",
        );
        let patch = hunk_patch(&files[0], 0);
        assert!(patch.contains("new file mode 100644\n"));
        assert!(patch.contains("--- /dev/null\n"));
        assert!(patch.contains("+++ b/created.txt\n"));
        assert!(patch.ends_with("+world\n"));
    }

    #[test]
    fn parses_deleted_and_binary_and_rename() {
        let out = "\
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
--- a/gone.txt
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/img.png b/img.png
index 111..222 100644
Binary files a/img.png and b/img.png differ
diff --git a/old_name.rs b/new_name.rs
similarity index 90%
rename from old_name.rs
rename to new_name.rs
--- a/old_name.rs
+++ b/new_name.rs
@@ -1 +1 @@
-same
+same
";
        let files = parse_unified_diff(out);
        assert_eq!(files.len(), 3);
        assert!(files[0].is_deleted);
        assert_eq!(files[0].path, "gone.txt");
        assert!(files[1].is_binary);
        assert_eq!(files[1].path, "img.png");
        assert_eq!(files[2].old_path.as_deref(), Some("old_name.rs"));
        assert_eq!(files[2].path, "new_name.rs");
        assert_eq!(files[2].hunks.len(), 1);
    }

    #[test]
    fn empty_input_yields_no_files() {
        assert!(parse_unified_diff("").is_empty());
    }

    #[test]
    fn hunk_header_parsing() {
        assert_eq!(parse_hunk_header("@@ -1,4 +1,5 @@ fn"), Some((1, 1)));
        assert_eq!(parse_hunk_header("@@ -0,0 +1 @@"), Some((0, 1)));
        assert_eq!(parse_hunk_header("@@ -12 +12 @@"), Some((12, 12)));
    }
}
