mod ui;

use std::path::PathBuf;

fn main() {
    install_panic_hook();

    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string());
    let path = PathBuf::from(&path);
    if !path.exists() {
        eprintln!("rebased-rs: path does not exist: {}", path.display());
        std::process::exit(2);
    }
    ui::run(path);
}

/// GUI 崩溃兜底：把 panic 现场追加到用户数据目录的 panic.log，
/// 避免运行期崩溃无痕迹（release 构建同样保留 unwind + hook，日志兜底有效）。
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let dir = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local")
                    .join("share")
            })
            .join("rebased-rs");
        let _ = std::fs::create_dir_all(&dir);
        let log = dir.join("panic.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
        {
            use std::io::Write;
            let ts = chrono::Utc::now().to_rfc3339();
            let _ = writeln!(f, "[{ts}] {info}");
        }
    }));
}
