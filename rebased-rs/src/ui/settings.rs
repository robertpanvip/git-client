//! 应用设置的持久化：所有偏好（语言、主题等）集中存放在单一 config 文件，
//! 按 `key=value` 行存储；写入时合并保留其他键，避免各项设置互相覆盖。

use gpui_kit::component::theme::ThemeMode;
use std::path::PathBuf;

/// 应用数据目录：Windows 优先 %APPDATA%，其余平台沿用 XDG 约定。
/// config 文件、启动日志、panic 日志统一存放于此。
pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("APPDATA").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir).join("rebased-rs"));
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })?;
    Some(base.join("rebased-rs"))
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config"))
}

pub(crate) fn read_config_value(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    let prefix = format!("{key}=");
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .map(str::trim)
            .map(str::to_string)
    })
}

pub(crate) fn write_config_value(key: &str, value: &str) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let prefix = format!("{key}=");
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with(&prefix))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    lines.push(format!("{key}={value}"));
    let _ = std::fs::write(path, lines.join("\n") + "\n");
}

/// 启动时恢复主题模式；无记录返回 None（保持默认浅色主题）。
pub fn load_theme_mode() -> Option<ThemeMode> {
    match read_config_value("theme").as_deref() {
        Some("dark") => Some(ThemeMode::Dark),
        Some("light") => Some(ThemeMode::Light),
        _ => None,
    }
}

pub fn persist_theme_mode(mode: ThemeMode) {
    write_config_value("theme", mode.name());
}

/// 启动时恢复字体设置：窗口字号增量 + 编辑器/等宽字号（缺省用主题默认基线）。
pub fn load_font_settings() {
    if let Some(delta) = read_config_value("ui_font_delta").and_then(|v| v.parse::<i32>().ok()) {
        crate::ui::theme::set_ui_font_delta(delta);
    }
    if let Some(size) = read_config_value("editor_font_size").and_then(|v| v.parse::<f32>().ok())
    {
        crate::ui::theme::set_editor_font_size(size);
    }
}

/// 持久化窗口字号增量（px，可为负）。
pub fn persist_ui_font_delta(delta: i32) {
    write_config_value("ui_font_delta", &delta.to_string());
}

/// 持久化编辑器/等宽字号（0.5px 步进）。
pub fn persist_editor_font_size(size: f32) {
    write_config_value("editor_font_size", &format!("{size:.1}"));
}

/// 提交设置：作者覆盖（IDEA「作者(A)」，空 = 使用仓库配置）。
pub fn load_commit_author() -> String {
    read_config_value("commit_author").unwrap_or_default()
}

pub fn persist_commit_author(author: &str) {
    write_config_value("commit_author", author);
}

/// 提交设置：Sign-off 提交（`--signoff`）。
pub fn load_commit_signoff() -> bool {
    read_config_value("commit_signoff").is_some_and(|value| value == "1")
}

pub fn persist_commit_signoff(on: bool) {
    write_config_value("commit_signoff", if on { "1" } else { "0" });
}

/// 提交设置：检查组开关（`key` 为 `commit_checks` 配置段内的一个名字）。
fn load_check(key: &str, default: bool) -> bool {
    read_config_value(&format!("commit_check_{key}"))
        .map(|value| value == "1")
        .unwrap_or(default)
}

fn persist_check(key: &str, on: bool) {
    write_config_value(&format!("commit_check_{key}"), if on { "1" } else { "0" });
}

/// 启动时恢复提交检查组开关（缺省沿用 IDEA 默认值）。
pub fn load_commit_checks() -> crate::ui::app::CommitChecks {
    use crate::ui::app::CommitChecks;
    let defaults = CommitChecks::default();
    CommitChecks {
        update_copyright: load_check("copyright", defaults.update_copyright),
        reformat_code: load_check("reformat", defaults.reformat_code),
        rearrange_code: load_check("rearrange", defaults.rearrange_code),
        optimize_imports: load_check("imports", defaults.optimize_imports),
        cleanup: load_check("cleanup", defaults.cleanup),
        check_dependencies: load_check("dependencies", defaults.check_dependencies),
        run_configuration: load_check("run_config", defaults.run_configuration),
        analyze_code: load_check("analyze", defaults.analyze_code),
        check_todo: load_check("todo", defaults.check_todo),
        run_advanced_after_commit: load_check("advanced", defaults.run_advanced_after_commit),
        always_use_server: load_check("server", defaults.always_use_server),
    }
}

pub fn persist_commit_checks(checks: &crate::ui::app::CommitChecks) {
    persist_check("copyright", checks.update_copyright);
    persist_check("reformat", checks.reformat_code);
    persist_check("rearrange", checks.rearrange_code);
    persist_check("imports", checks.optimize_imports);
    persist_check("cleanup", checks.cleanup);
    persist_check("dependencies", checks.check_dependencies);
    persist_check("run_config", checks.run_configuration);
    persist_check("analyze", checks.analyze_code);
    persist_check("todo", checks.check_todo);
    persist_check("advanced", checks.run_advanced_after_commit);
    persist_check("server", checks.always_use_server);
}

/// 追加一条启动/运行诊断日志到 `<数据目录>/startup.log`；任何失败静默忽略，
/// 日志属于尽力而为的观测手段，不允许影响主流程。
pub fn log_event(message: &str) {
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("startup.log"))
        {
            use std::io::Write;
            let ts = chrono::Utc::now().to_rfc3339();
            let _ = writeln!(f, "[{ts}] {message}");
        }
    }
}

#[cfg(test)]
static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) fn config_test_lock() -> std::sync::MutexGuard<'static, ()> {
    CONFIG_LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_roundtrip() {
        let _guard = config_test_lock();
        persist_theme_mode(ThemeMode::Dark);
        assert!(matches!(load_theme_mode(), Some(ThemeMode::Dark)));
        persist_theme_mode(ThemeMode::Light);
        assert!(matches!(load_theme_mode(), Some(ThemeMode::Light)));
    }

    #[test]
    fn write_preserves_other_keys() {
        let _guard = config_test_lock();
        write_config_value("lang", "zh");
        write_config_value("theme", "dark");
        write_config_value("lang", "en");
        assert_eq!(read_config_value("lang").as_deref(), Some("en"));
        assert_eq!(read_config_value("theme").as_deref(), Some("dark"));
    }

    #[test]
    fn font_settings_roundtrip() {
        let _guard = config_test_lock();
        persist_ui_font_delta(2);
        persist_editor_font_size(14.0);
        load_font_settings();
        assert_eq!(crate::ui::theme::ui_font_delta(), 2);
        assert_eq!(crate::ui::theme::editor_font_size(), 14.0);
        // 恢复默认，避免污染其它测试读取的全局状态。
        persist_ui_font_delta(0);
        persist_editor_font_size(crate::ui::theme::DEFAULT_FONT_SIZE_MONO);
        load_font_settings();
        assert_eq!(crate::ui::theme::ui_font_delta(), 0);
        assert_eq!(crate::ui::theme::editor_font_size(), crate::ui::theme::DEFAULT_FONT_SIZE_MONO);
    }
}
