use gpui::Context;

use super::{AppView, SidebarMode};

impl AppView {
    pub(crate) fn open_shelves(&mut self, cx: &mut Context<Self>) {
        self.sidebar = SidebarMode::Shelve;
        self.reload_shelves(cx);
    }

    pub(crate) fn reload_shelves(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.stash_list() {
            Ok(entries) => {
                self.shelves = entries;
                cx.notify();
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
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
                self.error = None;
                self.status_message = "Unshelved".into();
                self.refresh(cx);
                self.sidebar = SidebarMode::Shelve;
                self.reload_shelves(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
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
                self.error = None;
                self.status_message = "Dropped shelve".into();
                self.reload_shelves(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }
}
