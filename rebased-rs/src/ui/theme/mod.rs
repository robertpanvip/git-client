//! 全局设计 tokens：色板、排版、间距、圆角、尺寸与图标。
//!
//! 组件与面板一律从此取值，禁止在面板内写 magic number 或 ad-hoc 颜色。
//!
//! 目录结构：
//! - [`colors`]：IntelliJ New UI Dark 色板 + Git 语义色 + 派生色
//! - [`typography`]：字号 / 行高 / 字重 / 排版角色
//! - [`spacing`]：4px 网格间距
//! - [`radius`]：圆角刻度
//! - [`dimensions`]：密度、布局、列宽、菜单、对话框、图形几何
//! - [`icons`]：图标尺寸刻度
//!
//! tokens 为设计系统预留项，未即时引用属预期。

#![allow(dead_code)]

mod colors;
mod dimensions;
mod icons;
mod radius;
mod spacing;
mod typography;

pub use colors::*;
pub use dimensions::*;
pub use icons::*;
pub use radius::*;
pub use spacing::*;
pub use typography::*;
