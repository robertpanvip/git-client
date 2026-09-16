use gpui::{AppContext, Context, Window};

use rebased_rs::git::autosquash_plan;

use super::{AppView, PromptKind, RebaseFlow, SidebarMode};

impl AppView {
    pub(crate) fn start_rebase(&mut self, base: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        if self.state.busy.is_some() {
            return;
        }
        self.state.busy = Some("Loading rebase plan".to_string());
        cx.notify();
        let task = cx.background_spawn(async move {
            let plan = repo.rebase_todos(&base);
            (base, plan)
        });
        cx.spawn(async move |this, cx| {
            let (base, plan) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.busy = None;
                match plan {
                    Ok(plan) => {
                        this.state.rebase = RebaseFlow::Planning { base, plan };
                        this.state.sidebar = SidebarMode::Rebase;
                        this.state.error = None;
                    }
                    Err(e) => this.state.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
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

    pub(crate) fn move_rebase_action(
        &mut self,
        index: usize,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
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

    /// 把拖拽的项移动到目标位置（drop 目标行）。
    pub(crate) fn move_rebase_action_to(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let RebaseFlow::Planning { plan, .. } = &mut self.state.rebase else {
            return;
        };
        if from >= plan.len() || to >= plan.len() || from == to {
            return;
        }
        let action = plan.remove(from);
        plan.insert(to, action);
        cx.notify();
    }

    /// 勾选 autosquash 时立即把 fixup!/squash! 提交重排到目标之后作为预览；
    /// 操作幂等，取消勾选保留已重排的结果。
    pub(crate) fn toggle_rebase_autosquash(&mut self, on: bool, cx: &mut Context<Self>) {
        self.state.rebase_autosquash = on;
        if on && let RebaseFlow::Planning { plan, .. } = &mut self.state.rebase {
            *plan = autosquash_plan(std::mem::take(plan));
        }
        cx.notify();
    }

    /// 打开为计划项编辑消息的输入框，预填当前主题；确认后该提交会按 Reword 应用。
    pub(crate) fn open_rebase_edit(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((subject, existing)) = (match &self.state.rebase {
            RebaseFlow::Planning { plan, .. } => plan
                .get(index)
                .map(|a| (a.subject.clone(), a.message.clone().unwrap_or_default())),
            _ => None,
        }) else {
            return;
        };
        let prefill = if existing.is_empty() {
            subject
        } else {
            existing
        };
        self.prompt_input
            .update(cx, |state, cx| state.set_value(&prefill, window, cx));
        self.state.prompt = Some(PromptKind::RebaseEdit { index });
        cx.notify();
    }

    pub(crate) fn apply_rebase(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        let RebaseFlow::Planning { base, plan } = self.state.rebase.clone() else {
            return;
        };
        if plan.is_empty() || self.state.busy.is_some() {
            return;
        }
        // 开关开着时兜底再应用一次（幂等），覆盖勾选后手动调整的情况。
        let plan = if self.state.rebase_autosquash {
            autosquash_plan(plan)
        } else {
            plan
        };
        self.state.busy = Some("Rebasing commits".to_string());
        cx.notify();
        let task = cx.background_spawn(async move {
            let result = repo.rebase_run(&base, &plan);
            let stopped = result.is_err() && repo.is_rebase_in_progress();
            let stopped_at = if stopped {
                repo.rebase_stopped_commit().unwrap_or_default()
            } else {
                String::new()
            };
            (result, stopped, stopped_at, base, plan)
        });
        cx.spawn(async move |this, cx| {
            let (result, stopped, stopped_at, base, plan) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.busy = None;
                match result {
                    Ok(()) => {
                        this.state.error = None;
                        this.state.status_message =
                            format!("Rebased onto {}", &base[..base.len().min(7)]);
                        this.state.rebase = RebaseFlow::Idle;
                        this.refresh(cx);
                    }
                    Err(e) => {
                        this.state.error = Some(e.to_string());
                        if stopped {
                            this.state.rebase = RebaseFlow::Stopped {
                                base,
                                plan,
                                stopped_at,
                            };
                            this.state.sidebar = SidebarMode::Workspace;
                        } else {
                            this.state.rebase = RebaseFlow::Failed {
                                base,
                                plan,
                                message: e.to_string(),
                            };
                        }
                        cx.notify();
                    }
                }
            });
        })
        .detach();
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
