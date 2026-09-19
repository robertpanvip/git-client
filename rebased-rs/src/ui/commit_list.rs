use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Datelike, Local};
use gpui::{
    App, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Task, WeakEntity, Window, div, px,
};
use gpui_kit::base::IndexPath;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, Size,
    list::{ListDelegate, ListItem, ListState},
    menu::{ContextMenuExt, PopupMenu},
};
use rebased_rs::git::{Commit, Graph, build_graph, filter_commits};

use crate::ui::app::{AppView, ConfirmAction, PromptKind};
use crate::ui::components::{empty_state, menu_item, menu_width, ref_style_with_remotes, shortcuts};
use crate::ui::graph_view::{ROW_HEIGHT, lane_canvas};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;
use crate::ui::theme;

pub struct LogData {
    pub commits: Vec<Commit>,
    pub graph: Graph,
    /// 远程 tip 的**严格祖先**（原版在作者名后缀 `*` 标记这些提交）。
    pub starred: HashSet<String>,
}

impl LogData {
    pub fn new(commits: Vec<Commit>) -> Self {
        let mut graph = build_graph(&commits);
        // 原版语义：远程 ref 所在提交及其祖先 = 已推送；
        // 可达但不在远程的提交 = 未推送（青色节点），其余 = 已推送（紫色节点）。
        // 无远程仓库时全部按已推送渲染（原版单色紫）。
        let mut remote_tips: Vec<&Commit> = Vec::new();
        for commit in commits.iter() {
            let has_remote = commit
                .refs
                .iter()
                .any(|name| name != "HEAD" && !name.starts_with("HEAD -> ") && name.contains('/'));
            if has_remote {
                remote_tips.push(commit);
            }
        }
        let mut pushed: HashSet<String> = HashSet::new();
        let mut stack: Vec<&Commit> = remote_tips.iter().copied().collect();
        while let Some(commit) = stack.pop() {
            if !pushed.insert(commit.id.0.clone()) {
                continue;
            }
            for parent in &commit.parents {
                if let Some(parent_commit) =
                    commits.iter().find(|c| c.id.0 == parent.0)
                {
                    stack.push(parent_commit);
                }
            }
        }
        // 严格祖先 = 从远程 tip 的父提交开始再走一遍可达集。
        let mut starred: HashSet<String> = HashSet::new();
        let mut stack: Vec<&Commit> = remote_tips
            .iter()
            .copied()
            .flat_map(|tip| {
                tip.parents
                    .iter()
                    .filter_map(|parent| commits.iter().find(|c| c.id.0 == parent.0))
            })
            .collect();
        while let Some(commit) = stack.pop() {
            if !starred.insert(commit.id.0.clone()) {
                continue;
            }
            for parent in &commit.parents {
                if let Some(parent_commit) = commits.iter().find(|c| c.id.0 == parent.0) {
                    stack.push(parent_commit);
                }
            }
        }
        // 重绘泳道配色：按推送状态二色（原版），连线与节点同色。
        let has_remotes = !remote_tips.is_empty();
        for (row, commit) in graph.rows.iter_mut().zip(commits.iter()) {
            let color = if has_remotes && !pushed.contains(&commit.id.0) {
                theme::LANE_UNPUSHED
            } else {
                theme::LANE_PUSHED
            };
            row.color = color;
            for edge in row.edges.iter_mut() {
                edge.color = color;
            }
        }
        Self {
            commits,
            graph,
            starred,
        }
    }

    pub fn filtered(&self, query: &str) -> Self {
        let commits = filter_commits(&self.commits, query)
            .into_iter()
            .cloned()
            .collect();
        Self::new(commits)
    }
}

pub struct LogDelegate {
    data: Option<Arc<LogData>>,
    visible: Option<Arc<LogData>>,
    selected: Option<IndexPath>,
    /// `.*` 模式（原版搜索框同款开关）：开启后按通配符匹配（`*` 任意段）。
    pub(crate) regex: bool,
    /// AppView 的弱引用，用于 ref 徽章菜单与双击 checkout 联动应用层动作。
    app: Option<WeakEntity<AppView>>,
}

impl LogDelegate {
    pub fn new() -> Self {
        Self {
            data: None,
            visible: None,
            selected: None,
            regex: false,
            app: None,
        }
    }

    pub fn set_regex(&mut self, regex: bool) {
        self.regex = regex;
    }

    /// 在可见（含过滤后）提交中查找指定 id 的行号。
    pub fn row_of_commit(&self, id: &str) -> Option<usize> {
        self.visible
            .as_ref()?
            .commits
            .iter()
            .position(|c| c.id.0 == id)
    }

    pub fn set_app(&mut self, app: WeakEntity<AppView>) {
        self.app = Some(app);
    }

    pub fn set_data(&mut self, data: LogData) {
        self.data = Some(Arc::new(data));
        self.visible = self.data.clone();
        self.selected = None;
    }

    pub fn commit_at(&self, row: usize) -> Option<Commit> {
        self.visible
            .as_ref()
            .and_then(|data| data.commits.get(row))
            .cloned()
    }

    pub fn find_commit(&self, id: &str) -> Option<Commit> {
        let data = self.data.as_ref()?;
        data.commits.iter().find(|c| c.id.0 == id).cloned()
    }

    /// 当前可见（含过滤后）的提交数量，供键盘导航使用。
    pub fn visible_count(&self) -> usize {
        self.visible.as_ref().map_or(0, |data| data.commits.len())
    }

    /// 已加载提交的去重作者名（日志「用户」过滤下拉的数据源，升序）。
    pub fn authors(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .data
            .as_ref()
            .map(|data| {
                data.commits
                    .iter()
                    .map(|c| c.author.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names.dedup();
        names
    }

    fn rebuild(&mut self, query: &str) {
        let Some(data) = self.data.clone() else {
            return;
        };
        self.visible = if query.trim().is_empty() {
            Some(data)
        } else if self.regex {
            Some(Arc::new(wildcard_filtered(&data, query)))
        } else {
            Some(Arc::new(data.filtered(query)))
        };
        self.selected = None;
    }
}

impl Default for LogDelegate {
    fn default() -> Self {
        Self::new()
    }
}

impl ListDelegate for LogDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.visible.as_ref().map_or(0, |data| data.commits.len())
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.rebuild(query);
        cx.notify();
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
        cx.notify();
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let data = self.visible.as_ref()?;
        let commit = data.commits.get(ix.row)?;
        let selected = self.selected == Some(ix);
        let fg = cx.theme().foreground;
        let app = self.app.clone();
        // 精确区分本地/远程分支：本地分支名可能含 `/`（如 `feature/x`），
        // 只靠名字启发式会误判，这里读仓库的 remote 列表。
        let remotes: Vec<String> = app
            .as_ref()
            .and_then(|app| app.upgrade())
            .map(|view| {
                view.read(cx)
                    .state
                    .remotes
                    .iter()
                    .map(|remote| remote.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        // HEAD 提交渲染为环形节点（原版同款）。
        let head_id = app
            .as_ref()
            .and_then(|app| app.upgrade())
            .and_then(|view| view.read(cx).state.head_id.clone());
        let is_head = head_id.as_deref() == Some(commit.id.0.as_str());
        // 远程 tip 严格祖先在作者名后缀 `*`（原版同款标记）。
        let starred = data.starred.contains(&commit.id.0);

        // merge 提交整行降为次要色（原版实测 `#6F737A`），普通行用主文字色。
        let is_merge = commit.is_merge();
        let row_fg = if is_merge {
            theme::text_muted()
        } else {
            fg
        };

        let mut row = div()
            .id(SharedString::from(format!("commit-row-{}", ix.row)))
            .h(px(ROW_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_SM))
            .overflow_hidden();

        let graph_row = data.graph.rows.get(ix.row).cloned();
        row = row.child(lane_canvas(
            graph_row,
            data.graph.lane_count,
            is_merge,
            is_head,
            // merge 环形节点的挖空色 = Log 列表真实背景，避免与容器色差。
            theme::log_list_bg(),
        ));

        // Subject 单元格 = 提交标题（flex），ref 标签列紧贴作者列（原版布局）。
        row = row.child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(theme::font_size_meta()))
                .text_color(row_fg)
                .child(commit.subject.clone()),
        );

        let mut refs_col = div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme::SPACE_SM));
        for (i, ref_name) in commit.refs.iter().enumerate() {
            let badge_id = SharedString::from(format!("ref-badge-{}-{}", ix.row, i));
            refs_col = refs_col.child(ref_badge(&badge_id, ref_name, app.clone(), &remotes));
        }
        row = row.child(refs_col);

        // 列式布局：graph | subject(flex) | refs | author | date | hash。
        // 与原版截图一致——作者加粗（未推送加 `*`），日期左对齐（今天/昨天/年月日），
        // 末列 8 位短哈希；merge 提交整行（含哈希）变暗。
        row = row
            .child(
                div()
                    .flex_none()
                    .w(px(theme::COL_AUTHOR_WIDTH))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_meta()))
                    .font_weight(theme::WEIGHT_BOLD)
                    .text_color(row_fg)
                    .child(if starred {
                        format!("{}*", commit.author.name)
                    } else {
                        commit.author.name.clone()
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(theme::COL_DATE_WIDTH))
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_meta()))
                    .text_color(row_fg)
                    .child(format_time(commit.time)),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(theme::COL_HASH_WIDTH))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(theme::font_size_code()))
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(row_fg)
                    .child(short_id(&commit.id.0).to_string()),
            );

        // 双击 commit = Checkout（IntelliJ 惯例）。行选中由 ListState 统一处理，
        // 这里只拦截双击，与单击选择互不干扰。
        row = row.on_click({
            let app = app.clone();
            let commit_id = commit.id.0.clone();
            move |click: &ClickEvent, _: &mut Window, cx: &mut App| {
                if click.click_count() < 2 {
                    return;
                }
                if let Some(app) = app.as_ref().and_then(|app| app.upgrade()) {
                    let id = commit_id.clone();
                    app.update(cx, |this, cx| this.checkout_commit(id, cx));
                }
            }
        });

        // `ListItem` 自带 `py_1`，会把行高从 24 撑到 32；这里显式归零，
        // 让行高完全由 `ROW_HEIGHT` 决定（与原版表格行高一致）。
        Some(
            ListItem::new(ix.row)
                .selected(selected)
                .px(px(theme::SPACE_MD))
                .py(px(0.))
                .child(row),
        )
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_state(tr("No commits", "没有提交"), cx.theme().muted_foreground)
    }
}

/// `.*` 模式过滤：把查询串当作通配符模式（`*` 匹配任意段，其余字面匹配），
/// 对 subject / author / hash 做大小写不敏感匹配（无 regex 依赖的轻量实现）。
fn wildcard_filtered(data: &LogData, query: &str) -> LogData {
    let pattern = query.trim().to_lowercase();
    let commits: Vec<Commit> = data
        .commits
        .iter()
        .filter(|c| {
            wildcard_match(&pattern, &c.subject.to_lowercase())
                || wildcard_match(&pattern, &c.author.name.to_lowercase())
                || wildcard_match(&pattern, c.id.0.to_lowercase().as_str())
        })
        .cloned()
        .collect();
    LogData::new(commits)
}

/// 单 `*` 通配符匹配（支持多个 `*`；无 `*` 时退化为子串匹配）。
fn wildcard_match(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return text.contains(pattern);
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            pi += 1;
            mark = ti;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn ref_badge(
    id: &str,
    name: &str,
    app: Option<WeakEntity<AppView>>,
    remotes: &[String],
) -> impl IntoElement {
    let (label, color) = ref_style_with_remotes(name, remotes);
    // 原版观感：tag 图标 + 文字，无底色胶囊；
    // 本地分支 = 金色双 tag 图标 + 亮文字，远程 = 紫罗兰单 tag + 弱文字。
    let is_tag = name.starts_with("tag: ");
    let is_remote = !is_tag
        && name != "HEAD"
        && !name.starts_with("HEAD -> ")
        && remotes
            .iter()
            .any(|remote| name.starts_with(&format!("{remote}/")));
    let icon = if is_tag {
        Ic::Tag
    } else if is_remote {
        Ic::RefTagRemote
    } else {
        Ic::RefTagLocal
    };
    let text = if is_remote || is_tag {
        theme::text_muted()
    } else {
        theme::text_primary()
    };

    let name = name.to_string();
    div()
        .id(SharedString::from(id.to_string()))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(theme::SPACE_XS))
        .child(Icon::new(icon).with_size(Size::Small).text_color(color))
        .child(
            div()
                .text_size(px(theme::font_size_meta()))
                .text_color(text)
                .overflow_hidden()
                .whitespace_nowrap()
                .child(label),
        )
        .context_menu(move |menu, _window, cx| {
            match &app {
                Some(app) => build_ref_menu(menu, &name, app, cx),
                None => menu,
            }
        })
}

/// ref 徽章的右键菜单：tag 可删除；分支按本地/远程给出对应操作
/// （与 toolbar 分支菜单能力对齐），纯 HEAD 徽章无操作。
fn build_ref_menu(menu: PopupMenu, name: &str, app: &WeakEntity<AppView>, cx: &App) -> PopupMenu {
    if let Some(tag) = name.strip_prefix("tag: ") {
        let tag = tag.to_string();
        return menu_width(menu.item(menu_item(
            Ic::Delete,
            format!("Delete tag {tag}…"),
            None,
            true,
            false,
            {
                let app = app.clone();
                move |_, _, cx| {
                    let _ = app.update(cx, |this, cx| {
                        this.open_prompt(
                            PromptKind::Confirm(ConfirmAction::DeleteTag { name: tag.clone() }),
                            cx,
                        )
                    });
                }
            },
        )));
    }

    let branch = name.strip_prefix("HEAD -> ").unwrap_or(name).to_string();
    let entry = app.upgrade().and_then(|view| {
        view.read(cx)
            .state
            .branch_entries
            .iter()
            .find(|b| b.name == branch)
            .cloned()
    });
    if entry.as_ref().is_none_or(|b| b.is_remote) {
        menu_width(
            menu.item(menu_item(
                Ic::Checkout,
                format!("Checkout {branch} (tracking)"),
                None,
                false,
                false,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| this.checkout_branch(&branch, cx));
                    }
                },
            ))
            .item(menu_item(
                Ic::Pull,
                format!("Pull {branch} into current"),
                Some(shortcuts::PULL.label),
                false,
                false,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| {
                            this.pull_branch_into_current(branch.clone(), cx)
                        });
                    }
                },
            ))
            .item(menu_item(
                Ic::Compare,
                format!("Compare {branch} with current"),
                None,
                false,
                false,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ =
                            app.update(cx, |this, cx| this.open_branch_compare(branch.clone(), cx));
                    }
                },
            )),
        )
    } else {
        let is_current = entry.as_ref().is_some_and(|b| b.is_current());
        let has_upstream = entry.as_ref().is_none_or(|b| b.upstream.is_some());
        menu_width(
            menu.item(menu_item(
                Ic::Checkout,
                // 当前分支由勾选列表达（checked 参数），不在文案里拼 `✓`。
                format!("Checkout {branch}"),
                None,
                false,
                is_current,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| this.checkout_branch(&branch, cx));
                    }
                },
            ))
            .item(menu_item(
                Ic::Push,
                format!("Push {branch}"),
                Some(shortcuts::PUSH.label),
                false,
                false,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| {
                            this.push_branch(branch.clone(), has_upstream, cx)
                        });
                    }
                },
            ))
            .item(menu_item(
                Ic::Edit,
                format!("Rename {branch}…"),
                None,
                false,
                false,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update_in(cx, |this, window, cx| {
                            this.open_rename_branch_by_name(branch.clone(), window, cx)
                        });
                    }
                },
            ))
            .item(menu_item(
                Ic::Delete,
                format!("Delete {branch}…"),
                None,
                true,
                false,
                {
                    let app = app.clone();
                    let branch = branch.clone();
                    move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| {
                            this.open_prompt(
                                PromptKind::Confirm(ConfirmAction::DeleteBranch {
                                    name: branch.clone(),
                                }),
                                cx,
                            )
                        });
                    }
                },
            )),
        )
    }
}

/// Log/History/Compare 的日期列格式：`yyyy/M/d HH:mm`；
/// 当天与前一天用「今天 / 昨天」相对表述（对齐原版表格）。
pub(crate) fn format_time(secs: i64) -> String {
    let Some(time) = DateTime::from_timestamp(secs, 0) else {
        return String::new();
    };
    let time = time.with_timezone(&Local);
    let hm = time.format("%H:%M");
    let today = Local::now().date_naive();
    let day = time.date_naive();
    if day == today {
        return format!("{} {hm}", tr("Today", "今天"));
    }
    if Some(day) == today.pred_opt() {
        return format!("{} {hm}", tr("Yesterday", "昨天"));
    }
    format!("{}/{}/{} {hm}", time.year(), time.month(), time.day())
}

/// Log 哈希列的短 SHA（8 位，与原版一致）。
pub(crate) fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

/// 完整日期时间（含年份与秒），用于 Detail 头部的提交时间展示。
pub(crate) fn format_full_time(secs: i64) -> String {
    match DateTime::from_timestamp(secs, 0) {
        Some(time) => time
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        None => String::new(),
    }
}
