//! 模态：连接表单、新建桶、确认对话框。
//! 以绝对定位遮罩覆盖全窗口，点击遮罩关闭。

use crate::state::{AppModel, Modal};
use crate::{i18n, theme};
use crate::views::widgets;
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Styled};

pub fn render(model: &AppModel, cx: &Context<AppModel>) -> impl IntoElement {
    let is_image = matches!(model.modal, Modal::ImagePreview { .. });
    let card = match &model.modal {
        Modal::None => div(),
        Modal::ConnectionForm { .. } => connection_form(model, cx),
        Modal::CreateBucket { .. } => create_bucket(model, cx),
        Modal::CreateFolder { .. } => create_folder(model, cx),
        Modal::Confirm { .. } => confirm(model, cx),
        Modal::ImagePreview { .. } => image_preview(model, cx),
    };

    div()
        .id("modal-overlay")
        .absolute()
        .size_full()
        .top_0()
        .left_0()
        .flex()
        .items_center()
        .justify_center()
        // 暖黑遮罩（Kami deep-dark #141413 半透明，不用纯黑）
        .bg(gpui::rgba(0x14141399))
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|m: &mut AppModel, _: &gpui::MouseDownEvent, _w, cx| {
                // 点击遮罩关闭（点卡片内部不会冒泡到这里，因为卡片 on_mouse_down 拦截）
                m.close_modal(cx);
            }),
        )
        .child(
            div()
                .id("modal-card")
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation(); // 阻止冒泡到遮罩的关闭逻辑
                })
                .when(is_image, |el| {
                    // 图片预览：大画布、不受表单卡片尺寸约束
                    el.max_w(px(920.))
                        .min_w(px(520.))
                        .max_h(gpui::px(760.))
                })
                .when(!is_image, |el| el.min_w(px(420.)).max_w(px(520.)))
                .flex()
                .flex_col()
                .gap_4()
                .p_5()
                .rounded_lg()
                .bg(theme::panel())
                .border_1()
                .border_color(theme::hairline())
                .shadow_lg()
                .child(card),
        )
}

fn modal_title(title: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(theme::font_title()))
        .font_family(theme::FONT_SERIF)
        .font_weight(theme::WEIGHT_TITLE)
        .text_color(theme::text())
        .child(title.into())
}

fn connection_form(model: &AppModel, cx: &Context<AppModel>) -> gpui::Div {
    let Modal::ConnectionForm {
        fields,
        test_result,
        error,
        ..
    } = &model.modal
    else {
        unreachable!()
    };

    let mut card = div().flex().flex_col().gap_3();
    card = card.child(modal_title(i18n::t(i18n::Key::NewConnection)));

    let f = fields.read(cx);
    card = card
        .child(widgets::field(
            i18n::t(i18n::Key::ConnectionName),
            f.name.clone(),
        ))
        .child(widgets::field(i18n::t(i18n::Key::Endpoint), f.endpoint.clone()))
        .child(widgets::field(
            i18n::t(i18n::Key::RegionOptional),
            f.region.clone(),
        ))
        .child(widgets::field(i18n::t(i18n::Key::AccessKey), f.ak.clone()))
        .child(widgets::field(i18n::t(i18n::Key::SecretKey), f.sk.clone()))
        .child(
            div()
                .id("toggle-path-style")
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .text_size(px(theme::font_caption()))
                .child(div().text_color(if f.path_style {
                    theme::accent()
                } else {
                    theme::text_dim()
                }).child(if f.path_style { "☑" } else { "☐" }))
                .child(div().text_color(theme::text_dim()).child(i18n::t(i18n::Key::PathStyle)))
                .on_click(cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.toggle_path_style(cx);
                })),
        );

    if let Some(result) = test_result {
        card = card.child(match result {
            Ok(()) => div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::accent())
                .child(i18n::t(i18n::Key::TestOk)),
            Err(msg) => div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::warn_fg())
                .child(format!("{}: {msg}", i18n::t(i18n::Key::TestFail))),
        });
    }
    if let Some(err) = error {
        card = card.child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::warn_fg())
                .child(err.clone()),
        );
    }

    card = card.child(
        div()
            .flex()
            .flex_row()
            .gap_2()
            .justify_end()
            .child(widgets::button(
                "btn-test-conn",
                i18n::t(i18n::Key::TestConnection),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.test_connection(cx);
                }),
            ))
            .child(widgets::button(
                "btn-cancel-modal",
                i18n::t(i18n::Key::Cancel),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.close_modal(cx);
                }),
            ))
            .child(widgets::primary_button(
                "btn-save-conn",
                i18n::t(i18n::Key::Save),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.save_connection(cx);
                }),
            )),
    );
    card
}

fn create_bucket(model: &AppModel, cx: &Context<AppModel>) -> gpui::Div {
    let Modal::CreateBucket { input, error } = &model.modal else {
        unreachable!()
    };
    let mut card = div().flex().flex_col().gap_3();
    card = card.child(modal_title(i18n::t(i18n::Key::NewBucket)));
    card = card.child(widgets::field(
        i18n::t(i18n::Key::BucketName),
        input.clone(),
    ));
    if let Some(err) = error {
        card = card.child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::warn_fg())
                .child(err.clone()),
        );
    }
    card = card.child(
        div()
            .flex()
            .flex_row()
            .gap_2()
            .justify_end()
            .child(widgets::button(
                "btn-cancel-modal",
                i18n::t(i18n::Key::Cancel),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.close_modal(cx);
                }),
            ))
            .child(widgets::primary_button(
                "btn-create-bucket",
                i18n::t(i18n::Key::Create),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.create_bucket(cx);
                }),
            )),
    );
    card
}

fn create_folder(model: &AppModel, cx: &Context<AppModel>) -> gpui::Div {
    let Modal::CreateFolder { input, error } = &model.modal else {
        unreachable!()
    };
    let mut card = div().flex().flex_col().gap_3();
    card = card.child(modal_title(i18n::t(i18n::Key::NewFolder)));
    card = card.child(widgets::field(
        i18n::t(i18n::Key::BucketName),
        input.clone(),
    ));
    if let Some(err) = error {
        card = card.child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::warn_fg())
                .child(err.clone()),
        );
    }
    card = card.child(
        div()
            .flex()
            .flex_row()
            .gap_2()
            .justify_end()
            .child(widgets::button(
                "btn-cancel-modal",
                i18n::t(i18n::Key::Cancel),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.close_modal(cx);
                }),
            ))
            .child(widgets::primary_button(
                "btn-create-folder",
                i18n::t(i18n::Key::Create),
                cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                    m.create_folder(cx);
                }),
            )),
    );
    card
}

fn confirm(model: &AppModel, cx: &Context<AppModel>) -> gpui::Div {
    let Modal::Confirm { title, message, .. } = &model.modal else {
        unreachable!()
    };
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(modal_title(title.clone()))
        .child(
            div()
                .text_size(px(theme::font_body()))
                .text_color(theme::text_dim())
                .child(message.clone()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .justify_end()
                .child(widgets::button(
                    "btn-cancel-modal",
                    i18n::t(i18n::Key::Cancel),
                    cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                        m.close_modal(cx);
                    }),
                ))
                .child(widgets::danger_button(
                    "btn-confirm-modal",
                    i18n::t(i18n::Key::Confirm),
                    cx.listener(|m: &mut AppModel, _: &gpui::ClickEvent, _w, cx| {
                        m.confirm_modal(cx);
                    }),
                )),
        )
}

fn image_preview(model: &AppModel, _cx: &Context<AppModel>) -> gpui::Div {
    let Modal::ImagePreview {
        key,
        loading,
        image,
        error,
    } = &model.modal
    else {
        unreachable!()
    };

    let name = key.rsplit('/').next().unwrap_or(key);

    let mut card = div()
        .flex()
        .flex_col()
        .gap_3()
        .items_center()
        // 预览卡片为深色画布（图片任意底色都读得清）
        .bg(theme::sidebar())
        .rounded_lg()
        .p_3()
        .child(
            div()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_faint())
                .max_w(px(860.))
                .overflow_x_hidden()
                .child(crate::views::objects::truncate_name(name, 60)),
        );

    if *loading {
        card = card.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(360.))
                .text_color(theme::text_faint())
                .child(i18n::t(i18n::Key::Loading)),
        );
    } else if let Some(err) = error {
        card = card.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(360.))
                .text_color(theme::warn_fg())
                .child(err.clone()),
        );
    } else if let Some(render_image) = image {
        card = card.child(
            gpui::img(render_image.clone())
                .max_w(px(880.))
                .max_h(px(660.))
                .object_fit(gpui::ObjectFit::Contain),
        );
    }

    card
}
