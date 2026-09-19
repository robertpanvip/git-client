use gpui::Context;

use rebased_rs::git::HunkChoice;

use super::{AppView, SidebarMode, use_cases};

/// ↑↓ 循环导航的纯算术：`count` 为冲突块总数，`current` 为上次定位索引。
/// 过期索引（换文件 / 重载后残留）按无定位处理，从边缘开始。
pub(crate) fn step_conflict_index(
    count: usize,
    current: Option<usize>,
    forward: bool,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let at = current.filter(|&c| c < count);
    Some(match (at, forward) {
        (Some(i), true) => (i + 1) % count,
        (Some(i), false) => (i + count - 1) % count,
        (None, true) => 0,
        (None, false) => count - 1,
    })
}

impl AppView {
    pub(crate) fn open_conflicts(&mut self, cx: &mut Context<Self>) {
        self.state.sidebar = SidebarMode::Conflicts;
        self.reload_conflict_state(cx);
    }

    pub(crate) fn reload_conflict_state(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match use_cases::reload_conflict_state(repo.as_ref(), &mut self.state) {
            Ok(()) => cx.notify(),
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn select_conflict_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match use_cases::select_conflict_file(repo.as_ref(), &mut self.state, path) {
            Ok(()) => cx.notify(),
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn choose_hunk(&mut self, index: usize, choice: HunkChoice, cx: &mut Context<Self>) {
        if let Some(slot) = self.state.conflict_choices.get_mut(index) {
            *slot = Some(choice);
            cx.notify();
        }
    }

    /// 面板头 ↑↓：在当前文件的冲突块之间循环导航，
    /// 由 GPUI list 的 `scroll_to_reveal_item` 滚动定位并高亮当前卡片。
    pub(crate) fn step_conflict_nav(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(next) = step_conflict_index(
            self.state.conflict_hunks.len(),
            self.state.conflict_nav,
            forward,
        ) else {
            return;
        };
        self.state.conflict_nav = Some(next);
        self.state.conflict_list.scroll_to_reveal_item(next);
        cx.notify();
    }

    pub(crate) fn apply_conflict_resolutions(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let Some(path) = self.state.conflict_path.clone() else {
            return;
        };
        if self
            .state
            .conflict_choices
            .iter()
            .any(|choice| choice.is_none())
        {
            self.state.error = Some("Resolve every hunk before applying".to_string());
            cx.notify();
            return;
        }
        let choices: Vec<HunkChoice> = self
            .state
            .conflict_choices
            .iter()
            .flatten()
            .copied()
            .collect();
        match use_cases::apply_conflict_resolutions(repo.as_ref(), &mut self.state, choices) {
            Ok(()) => {
                self.state.error = None;
                self.state.status_message = format!("Resolved {}", path);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn take_conflict_side(&mut self, path: String, ours: bool, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match use_cases::take_conflict_side(repo.as_ref(), &mut self.state, &path, ours) {
            Ok(()) => {
                self.state.error = None;
                let side = if ours { "ours" } else { "theirs" };
                self.state.status_message = format!("Took {side} for {path}");
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::step_conflict_index;

    #[test]
    fn conflict_nav_cycles_forward_and_backward() {
        assert_eq!(step_conflict_index(3, None, true), Some(0));
        assert_eq!(step_conflict_index(3, Some(0), true), Some(1));
        assert_eq!(step_conflict_index(3, Some(1), true), Some(2));
        assert_eq!(step_conflict_index(3, Some(2), true), Some(0));
        assert_eq!(step_conflict_index(3, None, false), Some(2));
        assert_eq!(step_conflict_index(3, Some(2), false), Some(1));
        assert_eq!(step_conflict_index(3, Some(0), false), Some(2));
    }

    #[test]
    fn conflict_nav_handles_empty_and_stale_index() {
        assert_eq!(step_conflict_index(0, None, true), None);
        assert_eq!(step_conflict_index(0, Some(4), false), None);
        // 换文件 / 重载后残留的过期索引：按无定位处理，从边缘开始。
        assert_eq!(step_conflict_index(2, Some(9), true), Some(0));
        assert_eq!(step_conflict_index(2, Some(9), false), Some(1));
    }
}
