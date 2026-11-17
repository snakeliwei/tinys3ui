//! 渲染总入口与状态栏。

pub mod buckets;
pub mod modals;
pub mod objects;
pub mod sidebar;
pub mod transfers;
pub mod widgets;

use crate::state::{AppModel, Modal, TransferStatus};
use crate::{i18n, theme};
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Styled};
use std::time::Duration;

/// 应用根元素。
pub fn root(model: &AppModel, cx: &Context<AppModel>) -> gpui::Div {
    let mut root = div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .bg(theme::bg())
        .text_color(theme::text())
        .text_size(px(theme::font_body()))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h_0()
                .child(sidebar::render(model, cx))
                .child(main_area(model, cx)),
        );

    if model.transfers_open {
        root = root.child(transfers::render(model, cx));
    }

    root = root.child(status_bar(model, cx));

    if !matches!(model.modal, Modal::None) {
        root = root.child(modals::render(model, cx));
    }

    root
}

fn main_area(model: &AppModel, cx: &Context<AppModel>) -> gpui::AnyElement {
    if model.browser.is_some() {
        objects::render(model, cx).into_any_element()
    } else if model.selected_conn.is_some() {
        buckets::render(model, cx).into_any_element()
    } else {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_w_0()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                crate::assets::logo_img(px(64. * theme::font_scale()))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .text_size(px(theme::font_title()))
                    .font_family(theme::FONT_SERIF)
                    .text_color(theme::text_dim())
                    .child(i18n::t(i18n::Key::SelectHint)),
            )
            .child(widgets::primary_button(
                "btn-new-conn-empty",
                i18n::t(i18n::Key::NewConnection),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, w, cx| {
                    m.open_connection_form(None, w, cx);
                }),
            ))
            .into_any_element()
    }
}

fn status_bar(model: &AppModel, cx: &Context<AppModel>) -> gpui::Div {
    let active = model
        .transfers
        .iter()
        .filter(|t| t.status == TransferStatus::Active)
        .count();

    // 左侧：临时通知或当前视图信息
    let notice = model
        .notice
        .as_ref()
        .filter(|(_, at)| at.elapsed() < Duration::from_secs(5))
        .map(|(msg, _)| msg.clone());

    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_4()
        .py_1()
        .text_size(px(theme::font_caption()))
        .text_color(theme::text_faint())
        .bg(theme::sidebar())
        .border_t_1()
        .border_color(theme::hairline());

    let mut left = div().flex().flex_row().items_center().gap_2();
    if let Some(notice) = notice {
        left = left.child(
            div()
                .text_color(theme::accent())
                .child(notice),
        );
    } else if let Some(err) = &model.config_error {
        left = left.child(div().text_color(theme::warn_fg()).child(err.clone()));
    }

    bar = bar.child(left);

    let mut right = div().flex().flex_row().items_center().gap_3();
    if !model.transfers.is_empty() || model.transfers_open {
        right = right.child(
            div()
                .id("sb-transfers")
                .cursor_pointer()
                .when(model.transfers_open, |el| {
                    el.text_color(theme::text())
                })
                .hover(|el| el.text_color(theme::accent()))
                .on_click(cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.toggle_transfers_panel(cx);
                }))
                .child(format!(
                    "{}{}",
                    i18n::t(i18n::Key::Transfers),
                    if active > 0 {
                        format!(" ({active})")
                    } else {
                        String::new()
                    }
                )),
        );
    }
    // 字号调节：A− / 100% / A+
    right = right
        .child(
            div()
                .id("sb-font-smaller")
                .cursor_pointer()
                .text_color(theme::text_dim())
                .hover(|el| el.text_color(theme::accent()))
                .on_click(cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.adjust_font_scale(-5, cx);
                }))
                .child("A−"),
        )
        .child(
            div()
                .id("sb-font-pct")
                .min_w(px(38.))
                .flex()
                .justify_center()
                .text_color(theme::text_faint())
                .child(format!("{}%", model.font_scale_pct)),
        )
        .child(
            div()
                .id("sb-font-larger")
                .cursor_pointer()
                .text_color(theme::text_mid())
                .hover(|el| el.text_color(theme::accent()))
                .on_click(cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.adjust_font_scale(5, cx);
                }))
                .child("A+"),
        );
    right = right.child(
        div()
            .id("sb-lang")
            .cursor_pointer()
            .hover(|el| el.text_color(theme::accent()))
            .on_click(cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                m.toggle_lang(cx);
            }))
            .child(i18n::lang_name()),
    );
    bar = bar.child(right);

    bar
}
