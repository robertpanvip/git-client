use gpui::{ClipboardItem, Context, Window};

use rebased_rs::git::Change;

use super::AppView;

impl AppView {
    pub(crate) fn toggle_stage(&mut self, change: &Change, cx: &mut Context<Self>) {
        let path = change.path.clone();
        if change.staged {
            self.run_op("Unstaged", move |repo| repo.reset(&[path.as_str()]), cx);
        } else {
            self.run_op("Staged", move |repo| repo.add(&[path.as_str()]), cx);
        }
    }

    pub(crate) fn do_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message_input.read(cx).value().to_string();
        if message.trim().is_empty() {
            self.error = Some("Commit message is empty".into());
            cx.notify();
            return;
        }
        let amend = self.amend;
        self.message_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.amend = false;
        self.run_op(
            "Committed",
            move |repo| {
                if !amend {
                    let status = repo.status()?;
                    if status.changes.iter().all(|change| !change.staged) {
                        repo.add_all()?;
                    }
                }
                repo.commit(&message, amend)
            },
            cx,
        );
    }

    pub(crate) fn do_push(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.current_branch.clone() else {
            self.error = Some("No current branch".into());
            cx.notify();
            return;
        };
        let set_upstream = self.current_upstream.is_none();
        self.run_op("Pushed", move |repo| repo.push(&branch, set_upstream), cx);
    }

    pub(crate) fn do_pull(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.current_branch.clone() else {
            self.error = Some("No current branch".into());
            cx.notify();
            return;
        };
        self.run_op("Pulled", move |repo| repo.pull(&branch), cx);
    }

    pub(crate) fn checkout_branch(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Checked out {name}");
        self.run_op(&message, move |repo| repo.checkout(&name), cx);
    }

    pub(crate) fn select_commit_by_id(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.list.read(cx).delegate().find_commit(id) {
            Some(commit) => self.load_commit_detail(commit, cx),
            None => {
                self.error = Some(format!("Commit {id} not in loaded history").into());
                cx.notify();
            }
        }
    }

    pub(crate) fn cherry_pick_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.selected.clone() else {
            return;
        };
        let id = commit.id.0.clone();
        let message = format!("Cherry-picked {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.cherry_pick(&id), cx);
    }

    pub(crate) fn revert_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.selected.clone() else {
            return;
        };
        let id = commit.id.0.clone();
        let message = format!("Reverted {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.revert(&id), cx);
    }

    pub(crate) fn delete_branch(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Deleted branch {name}");
        self.run_op(&message, move |repo| repo.delete_branch(&name, false), cx);
    }

    pub(crate) fn delete_tag(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let message = format!("Deleted tag {name}");
        self.run_op(&message, move |repo| repo.delete_tag(&name), cx);
    }

    pub(crate) fn merge_branch_into_current(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else {
            return;
        };
        match repo.merge_branch(&name) {
            Ok(()) => {
                self.error = None;
                self.status_message = format!("Merged {name}").into();
                self.refresh(cx);
            }
            Err(e) => {
                self.error = Some(e.to_string().into());
                self.refresh(cx);
            }
        }
    }

    pub(crate) fn abort_merge(&mut self, cx: &mut Context<Self>) {
        self.run_op("Merge aborted", |repo| repo.merge_abort(), cx);
    }

    pub(crate) fn continue_merge_op(&mut self, cx: &mut Context<Self>) {
        self.run_op("Merge continued", |repo| repo.merge_continue(), cx);
    }

    pub(crate) fn undo_head(&mut self, cx: &mut Context<Self>) {
        self.run_op(
            "Undid HEAD commit (changes kept staged)",
            |repo| repo.undo_head_commit(),
            cx,
        );
    }

    pub(crate) fn drop_head(&mut self, cx: &mut Context<Self>) {
        self.run_op("Dropped HEAD commit", |repo| repo.drop_head_commit(), cx);
    }

    pub(crate) fn force_push_current(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.current_branch.clone() else {
            self.error = Some("No current branch".into());
            cx.notify();
            return;
        };
        self.run_op(
            "Force pushed (with lease)",
            move |repo| repo.push_force(&branch),
            cx,
        );
    }

    pub(crate) fn push_all_tags(&mut self, cx: &mut Context<Self>) {
        self.run_op("Pushed all tags", |repo| repo.push_tags(), cx);
    }

    pub(crate) fn copy_commit_sha(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.selected.clone() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(commit.id.0));
        self.error = None;
        self.status_message = "Commit SHA copied".into();
        cx.notify();
    }
}
