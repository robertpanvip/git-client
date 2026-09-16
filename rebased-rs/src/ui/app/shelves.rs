use gpui::Context;

use crate::ui::i18n::tr;

use super::{use_cases, AppView, SidebarMode};

impl AppView {
    pub(crate) fn open_shelves(&mut self, cx: &mut Context<Self>) {
        self.state.sidebar = SidebarMode::Shelve;
        self.reload_shelves(cx);
    }

    pub(crate) fn reload_shelves(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match use_cases::reload_shelves(repo.as_ref(), &mut self.state) {
            Ok(()) => cx.notify(),
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn unshelve_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.stash_apply_at(index) {
            Ok(()) => {
                self.state.error = None;
                self.state.status_message = tr("Unshelved", "已恢复搁置").to_string();
                self.refresh(cx);
                self.state.sidebar = SidebarMode::Shelve;
                self.reload_shelves(cx);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn drop_shelve_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.stash_drop_at(index) {
            Ok(()) => {
                self.state.error = None;
                self.state.status_message = tr("Dropped shelve", "已丢弃搁置").to_string();
                self.reload_shelves(cx);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
                cx.notify();
            }
        }
    }
}
