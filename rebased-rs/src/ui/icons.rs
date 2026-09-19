//! JetBrains 官方 UI 图标（https://intellij-icons.jetbrains.design ，Apache-2.0）。
//! SVG 位于 `assets/icons/`，`Ic::Branch` → `icons/branch.svg`，随主题文字颜色着色。
//!
//! 注意：不使用 `icon_named!` 宏——本 crate 的 gpui-pre 后端中
//! `#[derive(IntoElement)]` 面向 View 类型，与该宏按 RenderOnce 生成的
//! 展开不兼容；这里只实现 `IconNamed`，经 `Icon::new(Ic::Xxx)` 渲染。

use std::borrow::Cow;

use gpui::{Img, img, px, AssetSource, Result, SharedString, Styled};

use crate::ui::theme;

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
    Check,
    Checkout,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    Close,
    Commit,
    Compare,
    Conflict,
    Copy,
    Dash,
    Delete,
    Diff,
    Edit,
    /// 预览 / 可见性（原版 Log 工具栏眼睛按钮）。
    Eye,
    Fetch,
    File,
    FileCss,
    FileHtml,
    FileImage,
    FileJs,
    FileJson,
    FileLock,
    FileMd,
    FilePy,
    FileRs,
    FileTs,
    FileToml,
    FileYaml,
    Filter,
    Folder,
    History,
    Language,
    Merge,
    MoreHorizontal,
    Pull,
    /// 执行 / 抓取（原版 Log 工具栏圆圈播放按钮）。
    PlayCircle,
    Push,
    Rebase,
    Refresh,
    Revert,
    /// 本地分支 ref 徽章：金色双 tag 轮廓（原版 Log 同款）。
    RefTagLocal,
    /// 远程分支 ref 徽章：紫罗兰单 tag 轮廓（原版 Log 同款）。
    RefTagRemote,
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
            Ic::Check => "icons/check.svg",
            Ic::Checkout => "icons/checkout.svg",
            Ic::ChevronDown => "icons/chevronDown.svg",
            Ic::ChevronRight => "icons/chevronRight.svg",
            Ic::ChevronUp => "icons/chevronUp.svg",
            Ic::Close => "icons/close.svg",
            Ic::Commit => "icons/commit.svg",
            Ic::Compare => "icons/compare.svg",
            Ic::Conflict => "icons/conflict.svg",
            Ic::Copy => "icons/copy.svg",
            Ic::Dash => "icons/dash.svg",
            Ic::Delete => "icons/delete.svg",
            Ic::Diff => "icons/diff.svg",
            Ic::Edit => "icons/edit.svg",
            Ic::Eye => "icons/eye.svg",
            Ic::Fetch => "icons/fetch.svg",
            Ic::File => "icons/file.svg",
            Ic::FileCss => "icons/fileCss.svg",
            Ic::FileHtml => "icons/fileHtml.svg",
            Ic::FileImage => "icons/fileImage.svg",
            Ic::FileJs => "icons/fileJs.svg",
            Ic::FileJson => "icons/fileJson.svg",
            Ic::FileLock => "icons/fileLock.svg",
            Ic::FileMd => "icons/fileMd.svg",
            Ic::FilePy => "icons/filePy.svg",
            Ic::FileRs => "icons/fileRs.svg",
            Ic::FileTs => "icons/fileTs.svg",
            Ic::FileToml => "icons/fileToml.svg",
            Ic::FileYaml => "icons/fileYaml.svg",
            Ic::Filter => "icons/filter.svg",
            Ic::Folder => "icons/folder.svg",
            Ic::History => "icons/history.svg",
            Ic::Language => "icons/language.svg",
            Ic::Merge => "icons/merge.svg",
            Ic::MoreHorizontal => "icons/moreHorizontal.svg",
            Ic::Pull => "icons/pull.svg",
            Ic::PlayCircle => "icons/playCircle.svg",
            Ic::Push => "icons/push.svg",
            Ic::Rebase => "icons/rebase.svg",
            Ic::Refresh => "icons/refresh.svg",
            Ic::Revert => "icons/revert.svg",
            Ic::RefTagLocal => "icons/refTagLocal.svg",
            Ic::RefTagRemote => "icons/refTagRemote.svg",
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
#[include = "file_types/**/*.svg"]
struct IconFiles;

/// 按文件扩展名 / 约定文件名选择 JetBrains 官方**彩色**文件类型图标的
/// 资产路径，未识别的回退通用文件图标（New UI expui 暗色变体，
/// [`img`](gpui::img) 经 SVG 光栅化按原色渲染，不走单色 alpha 管线）。
///
/// IDEA 全局只有这一套类型图标：项目树、编辑器 Tab、Changes 列表等
/// 所有出现文件名的位置共用，本项目同样只在 [`file_type_icon`] 一处构造。
pub(crate) fn colored_file_icon(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "file_types/rust.svg",
        "toml" | "lock" => "file_types/toml.svg",
        "md" | "markdown" => "file_types/markdown.svg",
        "json" | "jsonc" | "json5" => "file_types/json.svg",
        "yaml" | "yml" => "file_types/yaml.svg",
        "py" | "pyi" | "pyw" => "file_types/python.svg",
        "js" | "mjs" | "cjs" | "jsx" => "file_types/javaScript.svg",
        "ts" | "mts" | "cts" | "tsx" => "file_types/typeScript.svg",
        "html" | "htm" => "file_types/html.svg",
        "xhtml" => "file_types/xhtml.svg",
        "xml" | "svg" => "file_types/xml.svg",
        "css" | "scss" | "sass" | "less" => "file_types/css.svg",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => "file_types/image.svg",
        "txt" => "file_types/text.svg",
        "properties" => "file_types/properties.svg",
        "csv" | "tsv" => "file_types/csv.svg",
        "sh" | "bash" | "zsh" => "file_types/shell.svg",
        "sql" => "file_types/sql.svg",
        "patch" | "diff" => "file_types/patch.svg",
        "gradle" => "file_types/gradle.svg",
        "zip" | "jar" | "tar" | "gz" | "xz" | "7z" | "rar" => "file_types/archive.svg",
        "dockerfile" => "file_types/docker.svg",
        "editorconfig" => "file_types/editorConfig.svg",
        "gitignore" | "gitattributes" => "file_types/gitignore.svg",
        _ => match name.to_ascii_lowercase().as_str() {
            // 无扩展名的 IDE 约定文件（.gitignore / Dockerfile 等）。
            "dockerfile" => "file_types/docker.svg",
            ".gitignore" | ".gitattributes" | ".gitmodules" => "file_types/gitignore.svg",
            ".editorconfig" => "file_types/editorConfig.svg",
            _ => "file_types/anyType.svg",
        },
    }
}

/// 渲染 JetBrains 官方彩色文件类型图标（16×16，IDEA 全局同一套）。
pub(crate) fn file_type_icon(path: &str) -> Img {
    img(colored_file_icon(path))
        .flex_none()
        .w(px(theme::FILE_TYPE_ICON_SIZE))
        .h(px(theme::FILE_TYPE_ICON_SIZE))
}

/// 渲染 JetBrains 官方彩色文件夹图标（New UI 打开 / 折叠共用同形）。
pub(crate) fn folder_icon() -> Img {
    img("file_types/folder.svg")
        .flex_none()
        .w(px(theme::FILE_TYPE_ICON_SIZE))
        .h(px(theme::FILE_TYPE_ICON_SIZE))
}

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
