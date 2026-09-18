//! 设计系统组件层。
//!
//! 面板**只应**通过本模块的原语构建界面，不得自行写 magic number、
//! ad-hoc 颜色，或重复实现同一视觉元素。
//!
//! 模块划分：
//! - `button` / `icon_button`：按钮与图标按钮
//! - `toolbar` / `status_bar` / `tabs` / `separator`：结构容器
//! - `input` / `search` / `checkbox`：输入控件
//! - `badge` / `section_header` / `list_row` / `empty_state`：列表与信息展示
//! - `menu` / `context_menu` / `popup`：菜单与弹层
//! - `dialog`：对话框
//! - `split_pane`：可拖拽分栏
//! - `tooltip`：提示
//! - `shortcuts`：快捷键单一事实来源
//!
//! 扁平 re-export 是设计系统的**公开面**：部分原语当前尚未被面板引用，
//! 属于为后续面板（Phase 7 次要 Git 表面）预留的接口，因此允许未被导入。
//! dead_code 同理：lib target 中它们是 pub API，仅 bin target 的死代码
//! 分析会误报（与 `icons.rs` 对图标枚举的处理一致）。

#![allow(dead_code)]
#![allow(unused_imports)]

pub mod badge;
pub mod button;
pub mod checkbox;
pub mod context_menu;
pub mod dialog;
pub mod empty_state;
pub mod icon_button;
pub mod input;
pub mod list_row;
pub mod menu;
pub mod popup;
pub mod search;
pub mod section_header;
pub mod separator;
pub mod shortcuts;
pub mod split_pane;
pub mod status_bar;
pub mod tabs;
pub mod toolbar;
pub mod tooltip;

// ---------- 扁平 re-export（调用方直接 `components::xxx`） ----------
pub use badge::{badge, chip, ref_style, ref_style_with_remotes};
pub use button::DropdownButton;
pub use checkbox::Checkbox;
pub use context_menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
pub use dialog::{cancel_button, dialog_field, dialog_footer, dialog_shell};
pub use empty_state::{empty_hint, empty_state, empty_state_with};
pub use icon_button::{icon_button, labeled_icon_button};
pub use list_row::{list_row, selected, static_row};
pub use menu::{menu_item, menu_row, menu_section, menu_separator, menu_text, menu_width};
pub use section_header::{group_header, group_header_controls, panel_header};
pub use separator::{h_separator, panel_divider, v_separator};
pub use split_pane::{SplitDrag, SplitSide, drag_overlay, v_handle};
pub use status_bar::{status_bar, status_message, status_segment};
pub use toolbar::{toolbar, toolbar_group, toolbar_label};
pub use tooltip::{row_icon_button, with_tooltip};
