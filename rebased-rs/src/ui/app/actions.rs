use gpui::{ClipboardItem, Context, Window};

use rebased_rs::git::Change;

use super::{use_cases::commit_selected, AppView, ConfirmAction};

impl AppView {
    pub(crate) fn toggle_stage(&mut self, change: &Change, cx: &mut Context<Self>) {
        let path = change.path.clone();
        if change.staged {
            self.run_op("Unstaged", move |repo| repo.reset(&[path.as_str()]), cx);
        } else {
            self.run_op("Staged", move |repo| repo.add(&[path.as_str()]), cx);
        }
    }

    pub(crate) fn toggle_change_selection(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(pos) = self
            .state
            .selected_changes
            .iter()
            .position(|selected| selected == path)
        {
            self.state.selected_changes.remove(pos);
        } else {
            self.state.selected_changes.push(path.to_string());
        }
        cx.notify();
    }

    pub(crate) fn do_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message_input.read(cx).value().to_string();
        if message.trim().is_empty() {
            self.state.error = Some("Commit message is empty".to_string());
            cx.notify();
            return;
        }
        let amend = self.state.amend;
        let selected = self.state.selected_changes.clone();
        self.message_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.state.amend = false;
        self.run_op(
            "Committed",
            move |repo| commit_selected(repo, &message, amend, &selected),
            cx,
        );
    }

    pub(crate) fn do_push(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
            cx.notify();
            return;
        };
        let set_upstream = self.state.current_upstream.is_none();
        self.run_op("Pushed", move |repo| repo.push(&branch, set_upstream), cx);
    }

    pub(crate) fn do_pull(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
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
                self.state.error = Some(format!("Commit {id} not in loaded history"));
                cx.notify();
            }
        }
    }

    pub(crate) fn cherry_pick_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.state.selected.clone() else {
            return;
        };
        self.cherry_pick_commit(commit.id.0, cx);
    }

    pub(crate) fn cherry_pick_commit(&mut self, id: String, cx: &mut Context<Self>) {
        let message = format!("Cherry-picked {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.cherry_pick(&id), cx);
    }

    pub(crate) fn revert_selected(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.state.selected.clone() else {
            return;
        };
        self.revert_commit(commit.id.0, cx);
    }

    pub(crate) fn revert_commit(&mut self, id: String, cx: &mut Context<Self>) {
        let message = format!("Reverted {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.revert(&id), cx);
    }

    pub(crate) fn checkout_commit(&mut self, id: String, cx: &mut Context<Self>) {
        let message = format!("Checked out {}", &id[..id.len().min(7)]);
        self.run_op(&message, move |repo| repo.checkout(&id), cx);
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
                self.state.error = None;
                self.state.status_message = format!("Merged {name}");
                self.refresh(cx);
            }
            Err(e) => {
                self.state.error = Some(e.to_string());
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
        let Some(branch) = self.state.current_branch.clone() else {
            self.state.error = Some("No current branch".to_string());
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

    pub(crate) fn copy_commit_id(&mut self, id: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(id));
        self.state.error = None;
        self.state.status_message = "Commit SHA copied".to_string();
        cx.notify();
    }

    pub(crate) fn copy_commit_sha(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.state.selected.clone() else {
            return;
        };
        self.copy_commit_id(commit.id.0, cx);
    }

    pub(crate) fn confirm_action(&mut self, action: ConfirmAction, cx: &mut Context<Self>) {
        match action {
            ConfirmAction::ForcePush => self.force_push_current(cx),
            ConfirmAction::DeleteBranch { name } => self.delete_branch(&name, cx),
            ConfirmAction::DeleteTag { name } => self.delete_tag(&name, cx),
            ConfirmAction::DropHeadCommit => self.drop_head(cx),
            ConfirmAction::UndoHeadCommit => self.undo_head(cx),
            ConfirmAction::DiscardChanges { path } => {
                let message = format!("Discarded {path}");
                self.run_op(&message, move |repo| repo.discard_changes(&path), cx);
            }
        }
    }
}
