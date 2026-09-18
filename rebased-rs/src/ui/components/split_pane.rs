//! 分栏拖拽原语。
//!
//! **问题背景**：原实现所有分栏宽度写死在 `panels.rs`（389 / 480 / 680 字面量），
//! 完全没有拖拽调整能力。
//!
//! 实现方式：分隔条自身只负责**表达样式与光标**，拖拽状态由调用方（外壳）持有——
//! 这样组件层保持无状态，且不需要引入 gpui-component 的重量级 `dock` 体系
//! （那会要求为每个面板实现 `Panel` trait，属于架构改写）。
//!
//! 拖拽期间由外壳渲染一层覆盖全窗口的透明层承接 `on_mouse_move` / `on_mouse_up`：
//! 6px 宽的分隔条在快速拖动时容易丢失 move 事件，覆盖层可确保拖拽连续。

use gpui::{CursorStyle, Div, InteractiveElement, ParentElement, Stateful, Styled, div, px};

use crate::ui::theme;

/// 可拖拽的分隔侧。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitSide {
    /// 左侧 Commit 面板与主区之间。
    Left,
    /// 主区与右侧详情面板之间。
    Right,
}

/// 一次拖拽的起始快照。
#[derive(Clone, Copy, Debug)]
pub struct SplitDrag {
    pub side: SplitSide,
    /// 按下时的指针 x。
    pub start_x: f32,
    /// 按下时的面板宽度。
    pub start_width: f32,
}

impl SplitDrag {
    pub fn new(side: SplitSide, start_x: f32, start_width: f32) -> Self {
        Self {
            side,
            start_x,
            start_width,
        }
    }

    /// 依据当前指针位置算出新宽度，并夹取到 `[min, max]`。
    pub fn width_at(&self, current_x: f32, min: f32, max: f32) -> f32 {
        let delta = current_x - self.start_x;
        // 左侧面板变宽 = 指针右移；右侧面板变宽 = 指针左移。
        let raw = match self.side {
            SplitSide::Left => self.start_width + delta,
            SplitSide::Right => self.start_width - delta,
        };
        clamp(raw, min, max)
    }
}

pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if max < min {
        return min;
    }
    value.clamp(min, max)
}

/// 分隔条：视觉 1px，命中区 [`theme::SPLITTER_HIT_WIDTH`]，左右拖拽光标。
pub fn v_handle(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(theme::SPLITTER_HIT_WIDTH))
        .h_full()
        .cursor(CursorStyle::ResizeLeftRight)
        // 视觉中线：命中区居中画 1px，避免拖拽时面板边界"虚浮"。
        .child(div().mx_auto().w(px(1.)).h_full().bg(theme::separator()))
}

/// 拖拽期间的透明覆盖层：承接鼠标移动与释放，游标保持左右拖拽态。
pub fn drag_overlay(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .cursor(CursorStyle::ResizeLeftRight)
}
