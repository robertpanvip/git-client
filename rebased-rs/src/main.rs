// Windows release 构建以 GUI 子系统运行，不弹控制台黑框（debug 构建保留控制台便于看日志）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

use std::path::PathBuf;

fn main() {
    install_panic_hook();
    ui::settings::log_event("process started");

    let path = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let path = PathBuf::from(&path);
    if !path.exists() {
        ui::settings::log_event(&format!("path does not exist: {}", path.display()));
        eprintln!("rebased-rs: path does not exist: {}", path.display());
        std::process::exit(2);
    }
    ui::settings::log_event(&format!("opening repo path: {}", path.display()));
    ui::run(path);
    ui::settings::log_event("ui::run returned");
}

/// GUI 崩溃兜底：把 panic 现场追加到用户数据目录的 panic.log，
/// 避免运行期崩溃无痕迹（release 构建同样保留 unwind + hook，日志兜底有效）。
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        if let Some(dir) = ui::settings::config_dir() {
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
        }
    }));
}
