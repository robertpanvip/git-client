use gpui::Context;

use super::{use_cases, AppView, RebaseFlow, SidebarMode};

impl AppView {
    pub(crate) fn start_rebase(&mut self, base: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match use_cases::load_rebase_plan(repo.as_ref(), &mut self.state, &base) {
            Ok(()) => cx.notify(),
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn cycle_rebase_action(&mut self, index: usize, cx: &mut Context<Self>) {
        let RebaseFlow::Planning { plan, .. } = &mut self.state.rebase else {
            return;
        };
        if let Some(action) = plan.get_mut(index) {
            action.kind = action.kind.next();
            cx.notify();
        }
    }

    pub(crate) fn move_rebase_action(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        let RebaseFlow::Planning { plan, .. } = &mut self.state.rebase else {
            return;
        };
        let target = index as isize + delta;
        if target < 0 || target >= plan.len() as isize {
            return;
        }
        plan.swap(index, target as usize);
        cx.notify();
    }

    pub(crate) fn apply_rebase(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let RebaseFlow::Planning { base, plan } = self.state.rebase.clone() else {
            return;
        };
        if plan.is_empty() {
            return;
        }
        match repo.rebase_run(&base, &plan) {
            Ok(()) => {
                self.state.error = None;
                self.state.status_message =
                    format!("Rebased onto {}", &base[..base.len().min(7)]);
                self.state.rebase = RebaseFlow::Idle;
                self.refresh(cx);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
                if repo.is_rebase_in_progress() {
                    self.state.rebase = RebaseFlow::Stopped {
                        base,
                        plan,
                        stopped_at: repo.rebase_stopped_commit().unwrap_or_default(),
                    };
                    self.state.sidebar = SidebarMode::Workspace;
                } else {
                    self.state.rebase = RebaseFlow::Failed {
                        base,
                        plan,
                        message: e.to_string(),
                    };
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn cancel_rebase(&mut self, cx: &mut Context<Self>) {
        self.state.rebase = RebaseFlow::Idle;
        self.state.sidebar = SidebarMode::Workspace;
        cx.notify();
    }

    pub(crate) fn abort_rebase(&mut self, cx: &mut Context<Self>) {
        self.run_op("Rebase aborted", |repo| repo.rebase_abort(), cx);
    }

    pub(crate) fn continue_rebase(&mut self, cx: &mut Context<Self>) {
        self.run_op("Rebase continued", |repo| repo.rebase_continue(), cx);
    }
}
