//! 底部传输队列面板。

use crate::state::{AppModel, Direction, TransferStatus};
use crate::{i18n, theme};
use crate::views::widgets;
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Styled};

pub fn render(model: &AppModel, cx: &Context<AppModel>) -> impl IntoElement {
    let active = model
        .transfers
        .iter()
        .filter(|t| t.status == TransferStatus::Active)
        .count();

    let mut panel = div()
        .flex()
        .flex_col()
        .h(px(180.))
        .bg(theme::sidebar())
        .border_t_1()
        .border_color(theme::hairline());

    // 标题行（点击折叠）
    panel = panel.child(
        div()
            .id("transfers-header")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_1p5()
            .text_size(px(theme::font_caption()))
            .text_color(theme::text_dim())
            .cursor_pointer()
            .hover(|el| el.bg(theme::hover()))
            .on_click(cx.listener(
                |m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.toggle_transfers_panel(cx);
                },
            ))
            .child(
                div()
                    .text_color(theme::text_mid())
                    .child(i18n::t(i18n::Key::Transfers)),
            )
            .when(active > 0, |el| {
                el.child(div().text_color(theme::accent()).child(format!("{active}")))
            })
            .child(div().flex_1())
            .when(model.transfers.iter().any(|t| t.status != TransferStatus::Active), |el| {
                el.child(widgets::icon_button(
                    "btn-clear-transfers",
                    "✕",
                    cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                        m.clear_finished_transfers(cx);
                    }),
                ))
            }),
    );

    if model.transfers.is_empty() {
        panel = panel.child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(theme::text_dim())
                .text_size(px(theme::font_caption()))
                .child(i18n::t(i18n::Key::NoTransfers)),
        );
    } else {
        // 传输数量可控（用户主动发起），直接渲染；过多时由高度裁剪
        let mut list = div()
            .id("transfers-list")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll();
        for item in &model.transfers {
            list = list.child(render_item(item, cx));
        }
        panel = panel.child(list);
    }

    panel
}

fn render_item(item: &crate::state::TransferItem, cx: &Context<AppModel>) -> impl IntoElement {
    let id = item.id;
    let dir_label = match item.direction {
        Direction::Upload => i18n::t(i18n::Key::TransfersUp),
        Direction::Download => i18n::t(i18n::Key::TransfersDown),
    };
    let dir_icon = match item.direction {
        Direction::Upload => "⬆",
        Direction::Download => "⬇",
    };
    let status_label = match item.status {
        TransferStatus::Active => {
            let pct = if item.total > 0 {
                (item.transferred as f32 / item.total as f32) * 100.0
            } else {
                0.0
            };
            format!("{pct:.0}%")
        }
        TransferStatus::Done => i18n::t(i18n::Key::TransfersDone).to_string(),
        TransferStatus::Failed => i18n::t(i18n::Key::TransfersFailed).to_string(),
        TransferStatus::Cancelled => i18n::t(i18n::Key::TransfersCancelled).to_string(),
    };
    let status_color = match item.status {
        TransferStatus::Active => theme::text_dim(),
        TransferStatus::Done => theme::accent(),
        TransferStatus::Failed => theme::warn_fg(),
        TransferStatus::Cancelled => theme::text_dim(),
    };
    let progress = if item.total > 0 {
        item.transferred as f32 / item.total as f32
    } else {
        0.0
    };

    div()
        .id(("transfer", id))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_1p5()
        .when(item.status == TransferStatus::Failed, |el| {
            el.bg(theme::warn_bg())
        })
        .child(
            div()
                .w(px(40.))
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_faint())
                .child(format!("{dir_icon} {dir_label}")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .w_full()
                                .overflow_x_hidden()
                                .text_size(px(theme::font_body()))
                                .child(item.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(theme::font_caption()))
                                .text_color(status_color)
                                .child(status_label),
                        ),
                )
                .child(widgets::progress_bar(progress)),
        )
        .child(
            div()
                .w(px(110.))
                .flex()
                .justify_end()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_dim())
                .child(format!(
                    "{} / {}",
                    super::objects::human_size(item.transferred),
                    if item.total > 0 {
                        super::objects::human_size(item.total)
                    } else {
                        "?".to_string()
                    }
                )),
        )
        .when(item.status == TransferStatus::Active, |el| {
            el.child(widgets::icon_button(
                ("btn-cancel-transfer", id),
                i18n::t(i18n::Key::Cancel),
                cx.listener(move |m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.cancel_transfer(id, cx);
                }),
            ))
        })
}
