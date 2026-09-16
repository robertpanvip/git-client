//! 应用设置的持久化：所有偏好（语言、主题等）集中存放在单一 config 文件，
//! 按 `key=value` 行存储；写入时合并保留其他键，避免各项设置互相覆盖。

use gpui_kit::component::theme::ThemeMode;
use std::path::PathBuf;

fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("APPDATA") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir).join("rebased-rs"));
        }
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".local").join("share"))
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
}
