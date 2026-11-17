//! 侧栏：连接列表。

use crate::state::AppModel;
use crate::{i18n, theme};
use crate::views::widgets;
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Styled};

pub fn render(model: &AppModel, cx: &Context<AppModel>) -> impl IntoElement {
    let mut col = div()
        .flex()
        .flex_col()
        .w(px(240. * theme::font_scale()))
        .h_full()
        .bg(theme::sidebar())
        .border_r_1()
        .border_color(theme::hairline());

    // 品牌行：logo + s3ui（衬线）
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_4()
            .pt_3()
            .child(
                crate::assets::logo_img(px(22. * theme::font_scale()))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .text_size(px(theme::font_title()))
                    .font_family(theme::FONT_SERIF)
                    .font_weight(theme::WEIGHT_TITLE)
                    .text_color(theme::text())
                    .child("s3ui"),
            ),
    );

    // 标题行：连接 + 添加按钮
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .pt_2()
            .pb_2()
            .child(
                div()
                    .text_size(px(theme::font_caption()))
                    .text_color(theme::text_faint())
                    .child(i18n::t(i18n::Key::Connections)),
            )
            .child(
                div().flex().flex_row().gap_1().child(
                    widgets::icon_button(
                        "btn-add-conn",
                        "+",
                        cx.listener(
                            move |_m: &mut AppModel, _: &gpui::ClickEvent, w, cx| {
                                _m.open_connection_form(None, w, cx);
                            },
                        ),
                    ),
                ),
            ),
    );

    if let Some(err) = &model.config_error {
        col = col.child(
            div()
                .px_4()
                .pb_2()
                .text_size(px(theme::font_caption()))
                .text_color(theme::warn_fg())
                .child(err.clone()),
        );
    }

    if model.connections.is_empty() {
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_2()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_dim())
                .child(div().child(i18n::t(i18n::Key::NoConnections)))
                .when_some(model.store.as_ref(), |el, store| {
                    el.child(div().child(format!(
                        "{} {}",
                        i18n::t(i18n::Key::ConfigHint),
                        store.path().display()
                    )))
                }),
        );
    } else {
        for (ix, cfg) in model.connections.iter().enumerate() {
            let is_selected = model.selected_conn == Some(ix);
            let editing_ix = ix;
            col = col.child(
                div()
                    .id(("conn", ix))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_4()
                    .py_2()
                    .cursor_pointer()
                    .when(is_selected, |el| el.bg(theme::accent_tint()))
                    .hover(|el| el.bg(theme::hover()))
                    .on_click(cx.listener(
                        move |m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                            m.select_connection(editing_ix, cx);
                        },
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(div().text_size(px(theme::font_body())).child(cfg.name.clone()))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_1()
                                    .child(widgets::icon_button(
                                        ("btn-edit-conn", editing_ix),
                                        "✎",
                                        cx.listener(
                                            move |m: &mut AppModel,
                                                  _: &gpui::ClickEvent,
                                                  w,
                                                  cx| {
                                                m.open_connection_form(Some(editing_ix), w, cx);
                                            },
                                        ),
                                    ))
                                    .child(widgets::icon_button(
                                        ("btn-del-conn", editing_ix),
                                        "✕",
                                        cx.listener(
                                            move |m: &mut AppModel,
                                                  _: &gpui::ClickEvent,
                                                  _w,
                                                  cx| {
                                                m.ask_delete_connection(editing_ix, cx);
                                            },
                                        ),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(theme::font_caption()))
                            .text_color(theme::text_faint())
                            .child(cfg.endpoint.clone()),
                    ),
            );
        }
    }

    col
}
