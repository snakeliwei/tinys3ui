//! 通用小组件（Kami 风格）：按钮、空态、警示框、进度条、字段行。
//!
//! 按钮为填充型（Kami 否决 outline-only），危险操作用批准的暖棕警示色。
//! 圆角同心：卡片 8px，控件 6px。

use crate::{text_input::TextInput, theme};
use gpui::{
    div, prelude::*, px, ClickEvent, ElementId, Entity, IntoElement, ParentElement, SharedString,
    Styled, Window,
};

/// 普通按钮：ivory 底 + 强 hairline 描边 + 近黑文字（Kami 静默次按钮）。
pub fn button<I, H>(id: I, label: impl Into<SharedString>, handler: H) -> gpui::Stateful<gpui::Div>
where
    I: Into<ElementId>,
    H: Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .flex_shrink_0()
        .border_1()
        .border_color(theme::hairline())
        .bg(theme::panel())
        .text_size(px(theme::font_caption()))
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|el| {
            el.bg(theme::hover())
                .border_color(theme::text_faint())
        })
        .child(label.into())
        .on_click(move |ev, w, cx| handler(ev, w, cx))
}

/// 主操作按钮：墨蓝填充 + 纸白文字。
pub fn primary_button<I, H>(
    id: I,
    label: impl Into<SharedString>,
    handler: H,
) -> gpui::Stateful<gpui::Div>
where
    I: Into<ElementId>,
    H: Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .flex_shrink_0()
        .bg(theme::accent())
        .text_color(theme::panel())
        .text_size(px(theme::font_caption()))
        .cursor_pointer()
        .hover(|el| el.bg(theme::accent_light()))
        .child(label.into())
        .on_click(move |ev, w, cx| handler(ev, w, cx))
}

/// 危险操作按钮：暖棕警示（Kami 唯一批准的语义暖色）。
pub fn danger_button<I, H>(
    id: I,
    label: impl Into<SharedString>,
    handler: H,
) -> gpui::Stateful<gpui::Div>
where
    I: Into<ElementId>,
    H: Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .flex_shrink_0()
        .bg(theme::warn_fg())
        .text_color(theme::panel())
        .text_size(px(theme::font_caption()))
        .cursor_pointer()
        .hover(|el| el.bg(theme::warn_fg()).opacity(0.85))
        .child(label.into())
        .on_click(move |ev, w, cx| handler(ev, w, cx))
}

/// 紧凑的行内操作按钮：无底无边，悬停浮出。
pub fn icon_button<I, H>(
    id: I,
    label: impl Into<SharedString>,
    handler: H,
) -> gpui::Stateful<gpui::Div>
where
    I: Into<ElementId>,
    H: Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
{
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .flex_shrink_0()
        .rounded_sm()
        .text_size(px(theme::font_caption()))
        .text_color(theme::text_faint())
        .cursor_pointer()
        .hover(|el| {
            el.bg(theme::hover())
                .text_color(theme::text())
        })
        .child(label.into())
        .on_click(move |ev, w, cx| handler(ev, w, cx))
}

/// 居中空态（衬线，纸面气质）。
pub fn empty_state(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_color(theme::text_faint())
        .text_size(px(theme::font_title()))
        .font_family(theme::FONT_SERIF)
        .child(text.into())
}

/// 警示框：暖桃底 + 暖棕标题（错误也用暖警示，不用大红）。
pub fn error_box(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_4()
        .py_3()
        .rounded_md()
        .bg(theme::warn_bg())
        .border_1()
        .border_color(theme::warn_fg())
        .child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::warn_fg())
                .child(crate::i18n::t(crate::i18n::Key::Error)),
        )
        .child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text())
                .child(text.into()),
        )
}

/// 进度条：warm-sand 轨道 + 墨蓝填充。
pub fn progress_bar(fraction: f32) -> impl IntoElement {
    let clamped = fraction.clamp(0.0, 1.0);
    div()
        .flex_1()
        .h(px(4.))
        .rounded_full()
        .bg(theme::hover())
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .rounded_full()
                .bg(theme::accent())
                .w(gpui::relative(clamped)),
        )
}

/// 表单字段：标签 + ivory 输入底 + 强 hairline。
pub fn field(label: impl Into<SharedString>, input: Entity<TextInput>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_dim())
                .child(label.into()),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::hairline())
                .text_size(px(theme::font_body()))
                .child(input),
        )
}
