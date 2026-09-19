//! 极简国际化：`tr("English", "中文")` 内联双语，按当前语言返回其一。
//! 语言全局生效，切换后调用 `cx.notify()`/`cx.refresh()` 重绘即可。

use std::sync::atomic::{AtomicU8, Ordering};

use crate::ui::settings::{read_config_value, write_config_value};

static LANG: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    En,
    Zh,
}

impl Language {
    fn from_u8(v: u8) -> Language {
        if v == 1 { Language::Zh } else { Language::En }
    }
}

/// 系统偏好语言：locale 以 zh 开头（zh-CN/zh-TW/…）→ 中文，否则英文。
fn detect_system_language() -> Language {
    let detected = sys_locale::get_locale().unwrap_or_default();
    if detected.to_lowercase().starts_with("zh") {
        Language::Zh
    } else {
        Language::En
    }
}

pub fn current() -> Language {
    Language::from_u8(LANG.load(Ordering::Relaxed))
}

pub fn set_current(lang: Language) {
    LANG.store(
        match lang {
            Language::Zh => 1,
            Language::En => 0,
        },
        Ordering::Relaxed,
    );
    persist(lang);
}

/// 按当前语言挑一个文案。参数均为 `&'static str` 字面量，返回值直接借用其一。
pub fn tr(en: &'static str, zh: &'static str) -> &'static str {
    match current() {
        Language::Zh => zh,
        Language::En => en,
    }
}

/// 启动时解析语言：持久化设置优先；首次启动（无配置）跟随系统 locale。
pub fn load_persisted() {
    let mut resolved = detect_system_language();
    if let Some(value) = read_config_value("lang") {
        resolved = match value.trim() {
            "zh" => Language::Zh,
            "en" => Language::En,
            _ => resolved,
        };
    }
    LANG.store(
        match resolved {
            Language::Zh => 1,
            Language::En => 0,
        },
        Ordering::Relaxed,
    );
}

fn persist(lang: Language) {
    let value = match lang {
        Language::Zh => "zh",
        Language::En => "en",
    };
    write_config_value("lang", value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tr_picks_by_language() {
        // `set_current` 会写全局 LANG 并落盘配置；`persist_roundtrip` 同样在改这两者。
        // 两者必须串行，否则并发下断言会读到对方刚写入的语言（间歇性失败）。
        let _guard = crate::ui::settings::config_test_lock();
        set_current(Language::En);
        assert_eq!(tr("Fetch", "拉取"), "Fetch");
        set_current(Language::Zh);
        assert_eq!(tr("Fetch", "拉取"), "拉取");
        set_current(Language::En);
    }

    #[test]
    fn persist_roundtrip() {
        let _guard = crate::ui::settings::config_test_lock();
        set_current(Language::Zh);
        load_persisted();
        assert_eq!(current(), Language::Zh);
        set_current(Language::En);
        load_persisted();
        assert_eq!(current(), Language::En);
    }
}
