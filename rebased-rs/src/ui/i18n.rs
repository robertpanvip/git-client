//! 极简国际化：`tr("English", "中文")` 内联双语，按当前语言返回其一。
//! 语言全局生效，切换后调用 `cx.notify()`/`cx.refresh()` 重绘即可。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

static LANG: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    En,
    Zh,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Zh => "中文",
        }
    }

    fn from_u8(v: u8) -> Language {
        if v == 1 { Language::Zh } else { Language::En }
    }
}

pub fn current() -> Language {
    Language::from_u8(LANG.load(Ordering::Relaxed))
}

pub fn set_current(lang: Language) {
    LANG.store(match lang {
        Language::Zh => 1,
        Language::En => 0,
    }, Ordering::Relaxed);
    persist(lang);
}

pub fn toggle() -> Language {
    let next = match current() {
        Language::En => Language::Zh,
        Language::Zh => Language::En,
    };
    set_current(next);
    next
}

/// 按当前语言挑一个文案。参数均为 `&'static str` 字面量，返回值直接借用其一。
pub fn tr(en: &'static str, zh: &'static str) -> &'static str {
    match current() {
        Language::Zh => zh,
        Language::En => en,
    }
}

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

/// 启动时读取持久化语言设置；文件缺失或损坏时保持默认英文。
pub fn load_persisted() {
    let Some(path) = config_path() else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("lang=") {
            let lang = match value.trim() {
                "zh" => Language::Zh,
                _ => Language::En,
            };
            LANG.store(match lang {
                Language::Zh => 1,
                Language::En => 0,
            }, Ordering::Relaxed);
        }
    }
}

fn persist(lang: Language) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let value = match lang {
        Language::Zh => "zh",
        Language::En => "en",
    };
    let _ = std::fs::write(path, format!("lang={value}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tr_picks_by_language() {
        set_current(Language::En);
        assert_eq!(tr("Fetch", "拉取"), "Fetch");
        set_current(Language::Zh);
        assert_eq!(tr("Fetch", "拉取"), "拉取");
        set_current(Language::En);
    }

    #[test]
    fn persist_roundtrip() {
        set_current(Language::Zh);
        load_persisted();
        assert_eq!(current(), Language::Zh);
        set_current(Language::En);
        load_persisted();
        assert_eq!(current(), Language::En);
    }
}
