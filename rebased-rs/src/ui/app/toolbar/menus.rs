use gpui::WeakEntity;
use gpui_kit::component::menu::PopupMenu;

use crate::ui::app::AppView;
use crate::ui::components::{menu_item, menu_width, shortcuts};
use crate::ui::i18n::tr;
use crate::ui::icons::Ic;

/// 远程分支的操作子菜单：Checkout / Pull into 当前分支 / Rebase onto / Compare。
pub(crate) fn remote_branch_actions(
    menu: PopupMenu,
    name: &str,
    current: Option<String>,
    weak: WeakEntity<AppView>,
) -> PopupMenu {
    let menu = menu.item(menu_item(
        Ic::Checkout,
        format!("{} {name}", tr("Checkout", "检出")),
        None,
        false,
        false,
        {
            let weak = weak.clone();
            let name = name.to_string();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.checkout_branch(&name, cx));
            }
        },
    ));
    let pull_label = format!(
        "{} {name} → {}",
        tr("Pull into", "拉取到"),
        current.clone().unwrap_or_else(|| "HEAD".to_string())
    );
    let menu = menu.item(menu_item(
        Ic::Pull,
        pull_label,
        Some(shortcuts::PULL.label),
        false,
        false,
        {
            let weak = weak.clone();
            let name = name.to_string();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.pull_branch_into_current(name.clone(), cx)
                });
            }
        },
    ));
    let menu = menu.item(menu_item(
        Ic::Rebase,
        format!("{} {name}…", tr("Rebase onto", "变基到")),
        None,
        false,
        false,
        {
            let weak = weak.clone();
            let name = name.to_string();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.rebase_current_onto(name.clone(), cx));
            }
        },
    ));
    let compare_label = format!(
        "{} {name} ↔ {}",
        tr("Compare", "比较"),
        current.unwrap_or_else(|| "HEAD".to_string())
    );
    menu_width(
        menu.item(menu_item(Ic::Compare, compare_label, None, false, false, {
            let weak = weak.clone();
            let name = name.to_string();
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.open_branch_compare(name.clone(), cx));
            }
        })),
    )
}
