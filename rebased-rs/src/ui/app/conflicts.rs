use gpui::Context;

use rebased_rs::git::HunkChoice;

use super::{use_cases, AppView, SidebarMode};

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
                self.reload_conflict_state(cx);
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
                self.reload_conflict_state(cx);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }
}
