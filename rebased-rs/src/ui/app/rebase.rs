use gpui::Context;

use super::{AppView, SidebarMode};

impl AppView {
    pub(crate) fn start_rebase(&mut self, base: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.rebase_todos(&base) {
            Ok(plan) => {
                self.rebase_base = base;
                self.rebase_plan = plan;
                self.sidebar = SidebarMode::Rebase;
                self.error = None;
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    pub(crate) fn cycle_rebase_action(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(action) = self.rebase_plan.get_mut(index) {
            action.kind = action.kind.next();
            cx.notify();
        }
    }

    pub(crate) fn move_rebase_action(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        let target = index as isize + delta;
        if target < 0 || target >= self.rebase_plan.len() as isize {
            return;
        }
        self.rebase_plan.swap(index, target as usize);
        cx.notify();
    }

    pub(crate) fn apply_rebase(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let base = self.rebase_base.clone();
        let plan = self.rebase_plan.clone();
        if plan.is_empty() {
            return;
        }
        match repo.rebase_run(&base, &plan) {
            Ok(()) => {
                self.error = None;
                self.status_message =
                    format!("Rebased onto {}", &base[..base.len().min(7)]).into();
                self.rebase_plan.clear();
                self.rebase_base.clear();
                self.refresh(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                self.rebase_in_progress = repo.is_rebase_in_progress();
                if self.rebase_in_progress {
                    self.sidebar = SidebarMode::Workspace;
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn cancel_rebase(&mut self, cx: &mut Context<Self>) {
        self.rebase_plan.clear();
        self.rebase_base.clear();
        self.sidebar = SidebarMode::Workspace;
        cx.notify();
    }

    pub(crate) fn abort_rebase(&mut self, cx: &mut Context<Self>) {
        self.run_op("Rebase aborted", |repo| repo.rebase_abort(), cx);
    }

    pub(crate) fn continue_rebase(&mut self, cx: &mut Context<Self>) {
        self.run_op("Rebase continued", |repo| repo.rebase_continue(), cx);
    }
}
