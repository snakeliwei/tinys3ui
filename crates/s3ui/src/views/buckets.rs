//! 主区：桶列表视图。

use crate::state::AppModel;
use crate::{i18n, theme};
use crate::views::widgets;
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Styled};

/// 列宽（表头与行共用；随字号缩放）。
struct BucketCols;
impl BucketCols {
    fn time() -> gpui::Pixels {
        px(170. * theme::font_scale())
    }
    fn action() -> gpui::Pixels {
        px(44. * theme::font_scale())
    }
}

pub fn render(model: &AppModel, cx: &Context<AppModel>) -> impl IntoElement {
    let conn = model.current_connection();
    let mut col = div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w_0()
        .p_4()
        .gap_3();

    // 头部：连接信息 + 操作按钮
    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .flex_1()
                .child(
                    div()
                        .text_size(px(theme::font_head()))
                        .font_family(theme::FONT_SERIF)
                        .font_weight(theme::WEIGHT_TITLE)
                        .text_color(theme::text())
                        .child(conn.map(|c| c.name.clone()).unwrap_or_default()),
                )
                .child(
                    div()
                        .text_size(px(theme::font_caption()))
                        .text_color(theme::text_faint())
                        .child(conn.map(|c| c.endpoint.clone()).unwrap_or_default()),
                ),
        );
    if conn.is_some() {
        header = header
            .child(widgets::button(
                "btn-refresh-buckets",
                i18n::t(i18n::Key::Refresh),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.refresh_buckets(cx);
                }),
            ))
            .child(widgets::primary_button(
                "btn-new-bucket",
                i18n::t(i18n::Key::NewBucket),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, w, cx| {
                    m.open_create_bucket_modal(w, cx);
                }),
            ));
    }
    col = col.child(header);

    if let Some(err) = &model.buckets_error {
        col = col.child(widgets::error_box(err.clone()));
    }

    if model.buckets_loading && model.buckets.is_empty() {
        col = col.child(widgets::empty_state(i18n::t(i18n::Key::Loading)));
    } else if model.buckets.is_empty() && model.buckets_error.is_none() {
        col = col.child(widgets::empty_state(i18n::t(i18n::Key::NoBuckets)));
    } else {
        // 表头（hairline 底边分隔数据——Kami「减法规则」；列宽与行一致）
        col = col.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_4()
                .px_4()
                .py_2()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_mid())
                .border_b_1()
                .border_color(theme::hairline())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(i18n::t(i18n::Key::BucketName)),
                )
                .child(div().w(BucketCols::time()).child(i18n::t(i18n::Key::CreatedAt)))
                .child(div().w(BucketCols::action())),
        );

        for (bucket_ix, bucket) in model.buckets.iter().enumerate() {
            let name = bucket.name.clone();
            let name_for_delete = bucket.name.clone();
            let is_selected = model.selected_bucket.as_deref() == Some(bucket.name.as_str());
            col = col.child(
                div()
                    .id(gpui::SharedString::from(format!("bucket-{}", bucket.name)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(theme::border_soft())
                    .cursor_pointer()
                    .when(is_selected, |el| el.bg(theme::accent_tint()))
                    .hover(|el| el.bg(theme::hover()))
                    .on_click(cx.listener(
                        move |m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                            m.select_bucket(name.clone(), cx);
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_x_hidden()
                            .child(bucket.name.clone()),
                    )
                    .child(
                        div()
                            .w(BucketCols::time())
                            .text_color(theme::text_faint())
                            .child(fmt_epoch(bucket.created_epoch)),
                    )
                    .child(
                        div()
                            .w(BucketCols::action())
                            .flex()
                            .justify_end()
                            .child(widgets::icon_button(
                                ("btn-del-bucket", bucket_ix),
                                "✕",
                                cx.listener(
                                    move |m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                                        m.ask_delete_bucket(name_for_delete.clone(), cx);
                                    },
                                ),
                            )),
                    ),
            );
        }
    }

    col
}

/// epoch 秒 → `YYYY-MM-DD HH:MM UTC`。
pub fn fmt_epoch(secs: Option<i64>) -> String {
    match secs {
        Some(secs) => chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "-".into()),
        None => "-".into(),
    }
}
