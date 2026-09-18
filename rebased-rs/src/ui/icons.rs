//! JetBrains 官方 UI 图标（https://intellij-icons.jetbrains.design ，Apache-2.0）。
//! SVG 位于 `assets/icons/`，`Ic::Branch` → `icons/branch.svg`，随主题文字颜色着色。
//!
//! 注意：不使用 `icon_named!` 宏——本 crate 的 gpui-pre 后端中
//! `#[derive(IntoElement)]` 面向 View 类型，与该宏按 RenderOnce 生成的
//! 展开不兼容；这里只实现 `IconNamed`，经 `Icon::new(Ic::Xxx)` 渲染。

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// JetBrains 图标集（变体名 = 图标文件名的 PascalCase）。
/// 部分变体暂未被 UI 引用，保留完整映射供后续扩展。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Ic {
    Abort,
    Add,
    Blame,
    Branch,
    Changes,
    Checkout,
    ChevronDown,
    Close,
    Commit,
    Compare,
    Conflict,
    Copy,
    Delete,
    Diff,
    Edit,
    Fetch,
    File,
    Filter,
    Folder,
    FolderOpen,
    History,
    Language,
    Merge,
    MoreHorizontal,
    Pull,
    Push,
    Rebase,
    Refresh,
    Revert,
    Search,
    Settings,
    Shelve,
    Tag,
    Undo,
    Unshelve,
    User,
}

impl gpui_kit::assets::IconNamed for Ic {
    fn path(self) -> SharedString {
        match self {
            Ic::Abort => "icons/abort.svg",
            Ic::Add => "icons/add.svg",
            Ic::Blame => "icons/blame.svg",
            Ic::Branch => "icons/branch.svg",
            Ic::Changes => "icons/changes.svg",
            Ic::Checkout => "icons/checkout.svg",
            Ic::ChevronDown => "icons/chevronDown.svg",
            Ic::Close => "icons/close.svg",
            Ic::Commit => "icons/commit.svg",
            Ic::Compare => "icons/compare.svg",
            Ic::Conflict => "icons/conflict.svg",
            Ic::Copy => "icons/copy.svg",
            Ic::Delete => "icons/delete.svg",
            Ic::Diff => "icons/diff.svg",
            Ic::Edit => "icons/edit.svg",
            Ic::Fetch => "icons/fetch.svg",
            Ic::File => "icons/file.svg",
            Ic::Filter => "icons/filter.svg",
            Ic::Folder => "icons/folder.svg",
            Ic::FolderOpen => "icons/folderOpen.svg",
            Ic::History => "icons/history.svg",
            Ic::Language => "icons/language.svg",
            Ic::Merge => "icons/merge.svg",
            Ic::MoreHorizontal => "icons/moreHorizontal.svg",
            Ic::Pull => "icons/pull.svg",
            Ic::Push => "icons/push.svg",
            Ic::Rebase => "icons/rebase.svg",
            Ic::Refresh => "icons/refresh.svg",
            Ic::Revert => "icons/revert.svg",
            Ic::Search => "icons/search.svg",
            Ic::Settings => "icons/settings.svg",
            Ic::Shelve => "icons/shelve.svg",
            Ic::Tag => "icons/tag.svg",
            Ic::Undo => "icons/undo.svg",
            Ic::Unshelve => "icons/unshelve.svg",
            Ic::User => "icons/user.svg",
        }
        .into()
    }
}

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct IconFiles;

/// 资产源：优先命中本地 JetBrains 图标，未命中回退 gpui-kit 内置图标
/// （组件库 spinner/chevron 等默认图标依赖内置路径）。
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(file) = IconFiles::get(path) {
            return Ok(Some(file.data));
        }
        Ok(gpui_kit::assets::Assets.load(path).ok().flatten())
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut list: Vec<SharedString> = IconFiles::iter()
            .filter(|name| name.starts_with(path))
            .map(SharedString::from)
            .collect();
        list.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(list)
    }
}
