//! 快捷键单一事实来源。
//!
//! **问题背景**：原实现中键位有两个来源——`app/actions.rs` 的 `KeyBinding`
//! 与命令面板里硬编码的显示字符串（如 `panels.rs` 写 "Ctrl+K"），二者已经漂移。
//!
//! 此处把每个快捷键的 **绑定串** 与 **展示串** 定义在一起：
//! - `app/actions.rs` 注册时取 `.binding`
//! - 菜单 / 命令面板展示时取 `.label`
//!
//! 新增快捷键必须同时在此登记，否则无法在菜单中展示。

/// 一个快捷键的两个表示。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// 传给 `KeyBinding::new` 的绑定串，如 `ctrl-shift-k`。
    pub binding: &'static str,
    /// 界面展示串，如 `Ctrl+Shift+K`。
    pub label: &'static str,
}

pub const COMMIT: Shortcut = Shortcut {
    binding: "ctrl-enter",
    label: "Ctrl+Enter",
};
pub const FOCUS_COMPOSER: Shortcut = Shortcut {
    binding: "ctrl-k",
    label: "Ctrl+K",
};
pub const PUSH: Shortcut = Shortcut {
    binding: "ctrl-shift-k",
    label: "Ctrl+Shift+K",
};
pub const PULL: Shortcut = Shortcut {
    binding: "ctrl-t",
    label: "Ctrl+T",
};
pub const REFRESH_F5: Shortcut = Shortcut {
    binding: "f5",
    label: "F5",
};
pub const REFRESH_CTRL_R: Shortcut = Shortcut {
    binding: "ctrl-r",
    label: "Ctrl+R",
};
pub const CLOSE_OVERLAY: Shortcut = Shortcut {
    binding: "escape",
    label: "Esc",
};
pub const VCS_PALETTE: Shortcut = Shortcut {
    binding: "alt-backtick",
    label: "Alt+`",
};
pub const BLAME_CURRENT_FILE: Shortcut = Shortcut {
    binding: "ctrl-alt-b",
    label: "Ctrl+Alt+B",
};
pub const PANEL_CHANGES: Shortcut = Shortcut {
    binding: "ctrl-alt-1",
    label: "Ctrl+Alt+1",
};
pub const PANEL_LOG: Shortcut = Shortcut {
    binding: "ctrl-alt-2",
    label: "Ctrl+Alt+2",
};
pub const PANEL_HISTORY: Shortcut = Shortcut {
    binding: "ctrl-alt-3",
    label: "Ctrl+Alt+3",
};
pub const PANEL_SHELVES: Shortcut = Shortcut {
    binding: "ctrl-alt-6",
    label: "Ctrl+Alt+6",
};
pub const PANEL_CONFLICTS: Shortcut = Shortcut {
    binding: "ctrl-alt-8",
    label: "Ctrl+Alt+8",
};
