use gpui::Context;

use rebased_rs::git::{conflict_hunks, HunkChoice};

use super::{AppView, SidebarMode};

impl AppView {
    pub(crate) fn open_conflicts(&mut self, cx: &mut Context<Self>) {
        self.sidebar = SidebarMode::Conflicts;
        self.reload_conflict_state(cx);
    }

    pub(crate) fn reload_conflict_state(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.conflicted_files() {
            Ok(files) => {
                self.conflict_files = files;
                if !self.conflict_files.is_empty() {
                    let first = self.conflict_files[0].path.clone();
                    self.sidebar = SidebarMode::Conflicts;
                    self.select_conflict_file(&first, cx);
                }
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    pub(crate) fn select_conflict_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.conflict_file_content(path) {
            Ok(raw) => {
                let hunks = conflict_hunks(&raw);
                self.conflict_path = Some(path.to_string());
                self.conflict_raw = raw;
                self.conflict_choices = vec![None; hunks.len()];
                self.conflict_hunks = hunks;
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    pub(crate) fn choose_hunk(&mut self, index: usize, choice: HunkChoice, cx: &mut Context<Self>) {
        if let Some(slot) = self.conflict_choices.get_mut(index) {
            *slot = Some(choice);
            cx.notify();
        }
    }

    pub(crate) fn apply_conflict_resolutions(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let Some(path) = self.conflict_path.clone() else {
            return;
        };
        if self.conflict_choices.iter().any(|choice| choice.is_none()) {
            self.error = Some("Resolve every hunk before applying".into());
            cx.notify();
            return;
        }
        let choices: Vec<HunkChoice> = self
            .conflict_choices
            .iter()
            .flatten()
            .copied()
            .collect();
        let raw = self.conflict_raw.clone();
        match repo.resolve_conflict_markers(&path, &raw, &choices) {
            Ok(()) => {
                self.error = None;
                self.status_message = format!("Resolved {}", path).into();
                self.conflict_path = None;
                self.conflict_raw.clear();
                self.conflict_hunks.clear();
                self.conflict_choices.clear();
                self.reload_conflict_state(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    pub(crate) fn take_conflict_side(&mut self, path: String, ours: bool, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.checkout_side(&path, ours) {
            Ok(()) => {
                self.error = None;
                let side = if ours { "ours" } else { "theirs" };
                self.status_message = format!("Took {side} for {path}").into();
                if self.conflict_path.as_deref() == Some(path.as_str()) {
                    self.conflict_path = None;
                    self.conflict_raw.clear();
                    self.conflict_hunks.clear();
                    self.conflict_choices.clear();
                }
                self.reload_conflict_state(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }
}
