//! 复选框组件。
//!
//! - [`Checkbox`]：布尔受控复选框（gpui-kit facade，多数调用点使用）。
//! - [`TriStateCheckbox`]：受控三态复选框（全选 / 半选 / 未选），对齐
//!   IntelliJ Changes 工具窗组头复选框：组内全勾 → 勾，部分勾选 → 蓝底短横，
//!   全不选 → 空框；激活语义为「半选/全不选 → 全选，全选 → 全不选」。
//!
//! 三态组件直接包装 `gpui_kit::base` 的无样式 Checkbox（焦点、键盘
//! Enter/Space、accessibility 由 base 提供），视觉规格复刻 facade Checkbox
//! 的 XSmall：12px 指示器、1px 描边、4px 圆角、8px 白色字形经 alpha 遮罩
//! 着色并随 spring 淡入淡出。

use std::rc::Rc;

use gpui::{
    px, rems, svg, App, ClickEvent, ElementId, InteractiveElement, Interactivity, IntoElement,
    MouseButton, ParentElement, RenderOnce, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, prelude::FluentBuilder as _,
};
use gpui_kit::assets::IconNamed as _;
use gpui_kit::base::StyledExt as _;
use gpui_kit::base::spring;
use gpui_kit::base::{Checkbox as BaseCheckbox, CheckboxIndicator, CheckboxState};
use gpui_kit::component::{ActiveTheme, Sizable, Size};

use crate::ui::icons::Ic;

pub use gpui_kit::component::checkbox::Checkbox;

/// 三态复选框激活回调：入参为激活后的下一语义态。
pub type TriStateClickHandler = Rc<dyn Fn(CheckboxState, &ClickEvent, &mut Window, &mut App) + 'static>;

/// 受控三态复选框：`Unchecked` 全不选、`Indeterminate` 半选、`Checked` 全选。
///
/// 状态由调用方持有；激活时 `on_click` 收到 activated() 之后的下一语义态
/// （半选/全不选 → 全选，全选 → 全不选，即 IDEA 的三态切换语义）。
#[derive(IntoElement)]
pub struct TriStateCheckbox {
    id: ElementId,
    base: BaseCheckbox,
    style: StyleRefinement,
    state: CheckboxState,
    disabled: bool,
    size: Size,
    on_click: Option<TriStateClickHandler>,
}

impl TriStateCheckbox {
    /// 创建未选中的三态复选框。
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: BaseCheckbox::new(id.clone()),
            id,
            style: StyleRefinement::default(),
            state: CheckboxState::Unchecked,
            disabled: false,
            size: Size::XSmall,
            on_click: None,
        }
    }

    /// 设置受控语义态。
    pub fn state(mut self, state: CheckboxState) -> Self {
        self.state = state;
        self
    }

    /// 便捷构造：布尔映射为全选/全不选（清除半选）。
    pub fn checked(mut self, checked: bool) -> Self {
        self.state = if checked {
            CheckboxState::Checked
        } else {
            CheckboxState::Unchecked
        };
        self
    }

    /// 置为半选；传 `false` 不改变已有状态（半选需显式 `state` 清除）。
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        if indeterminate {
            self.state = CheckboxState::Indeterminate;
        }
        self
    }

    /// 指针与键盘激活是否失效。
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// 激活回调，参数为下一语义态：半选/全不选 → 全选，全选 → 全不选。
    pub fn on_click(
        mut self,
        handler: impl Fn(CheckboxState, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl InteractiveElement for TriStateCheckbox {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for TriStateCheckbox {}

impl Styled for TriStateCheckbox {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Sizable for TriStateCheckbox {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for TriStateCheckbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let selected = state != CheckboxState::Unchecked;
        let disabled = self.disabled;
        let radius = cx.theme().radius.min(px(4.));
        let unchecked_border = cx.theme().input;
        let checked_color = cx.theme().primary;
        let disabled_indicator_color = if selected {
            checked_color.opacity(0.5)
        } else {
            unchecked_border.opacity(0.5)
        };
        let instance_style = self.style.clone();
        let on_click = self.on_click.clone();
        let indicator_id = self.id.clone();
        let size = self.size;

        self.base
            .state(state)
            .disabled(disabled)
            .styles(|styles| {
                styles.disabled(|style| {
                    style
                        .text_color(cx.theme().muted_foreground)
                        .refine_style(&instance_style)
                })
            })
            .when_some(on_click, |this, on_click| {
                this.on_change(move |next, event, window, cx| {
                    window.prevent_default();
                    on_click(next, event, window, cx);
                })
            })
            .rounded(radius * 0.5)
            .refine_style(&self.style)
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                // 与 facade Checkbox 一致：按下不移动焦点。
                window.prevent_default();
            })
            .child(
                CheckboxIndicator::new()
                    .state(state)
                    .disabled(disabled)
                    .relative()
                    .map(|this| match size {
                        Size::XSmall => this.size_3(),
                        Size::Small => this.size_3p5(),
                        Size::Medium => this.size_4(),
                        Size::Large => this.size(rems(1.125)),
                        _ => this.size_4(),
                    })
                    .flex_shrink_0()
                    .border_1()
                    .rounded(radius)
                    .when(!selected, |this| {
                        this.bg(cx.theme().input_background())
                            .when(!disabled, |this| this.border_color(unchecked_border))
                    })
                    .styles(|styles| {
                        styles
                            .checked(|style| style.border_color(checked_color).bg(checked_color))
                            .indeterminate(|style| {
                                style.border_color(checked_color).bg(checked_color)
                            })
                            .disabled(|style| {
                                style
                                    .border_color(disabled_indicator_color)
                                    .when(selected, |style| style.bg(disabled_indicator_color))
                            })
                    })
                    .child(indicator_glyph(&indicator_id, size, state, disabled, window, cx)),
            )
    }
}

/// 指示器内的白色字形：全选 → 对勾，半选 → 短横；随 spring 淡入淡出。
fn indicator_glyph(
    id: &ElementId,
    size: Size,
    state: CheckboxState,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let visible = state != CheckboxState::Unchecked;
    let opacity = spring(
        (id.clone(), "mark"),
        if visible { 1. } else { 0. },
        cx.theme().motion_tokens().spring_control,
        window,
        cx,
    );
    let color = if disabled {
        cx.theme().primary_foreground.opacity(0.5)
    } else {
        cx.theme().primary_foreground
    };
    let path = match state {
        CheckboxState::Indeterminate => Ic::Dash.path(),
        _ => Ic::Check.path(),
    };

    svg()
        .absolute()
        .top_px()
        .left_px()
        .map(|this| match size {
            Size::XSmall => this.size_2(),
            Size::Small => this.size_2p5(),
            Size::Medium => this.size_3(),
            Size::Large => this.size_3p5(),
            _ => this.size_3(),
        })
        .text_color(color)
        .when(opacity > 0., |this| this.path(path).opacity(opacity))
}
