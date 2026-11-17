//! 对象浏览视图：工具栏 + 面包屑 + 虚拟滚动表格（触底增量加载）。

use crate::state::{AppModel, SortKey};
use crate::{i18n, theme};
use crate::views::widgets;
use gpui::{
    div, prelude::*, px, uniform_list, App, ClickEvent, Context, IntoElement, ParentElement,
    Styled, WeakEntity, Window,
};
use s3_core::ObjectEntry;

/// 列宽（表头与行共用，确保逐列对齐；随字号缩放）。
struct Cols;
impl Cols {
    fn select() -> gpui::Pixels {
        px(24. * theme::font_scale())
    }
    /// 名称列内图标位（固定宽，表头同样占位，保证文字起点一致）。
    fn icon() -> gpui::Pixels {
        px(14. * theme::font_scale())
    }
    fn size() -> gpui::Pixels {
        px(90. * theme::font_scale())
    }
    fn time() -> gpui::Pixels {
        px(150. * theme::font_scale())
    }
    /// 操作列：容纳 预览/下载/复制链接/删除 四个按钮（基准 220px）。
    fn actions() -> gpui::Pixels {
        px(220. * theme::font_scale())
    }
}

pub fn render(model: &AppModel, cx: &Context<AppModel>) -> impl IntoElement {
    let Some(browser) = &model.browser else {
        return div();
    };

    let mut col = div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w_0()
        .p_4()
        .gap_3();

    // ---- 工具栏 ----
    let selected_count = browser.selected.len();

    let mut toolbar = div().flex().flex_row().items_center().gap_2().flex_wrap();

    // 返回上级
    toolbar = toolbar.child(widgets::button(
        "btn-parent",
        "←",
        cx.listener(|m: &mut AppModel, _: &ClickEvent, _w, cx| {
            m.go_parent(cx);
        }),
    ));

    // 面包屑：bucket / seg1 / seg2（分隔符用 Kami 的「·」而非斜杠）
    let mut crumbs = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .flex_1()
        .min_w_0()
        .overflow_x_hidden();
    let root_label = browser.bucket.clone();
    crumbs = crumbs.child(
        div()
            .id("crumb-root")
            .px_1()
            .rounded_sm()
            .font_family(theme::FONT_SERIF)
            .text_color(theme::text())
            .cursor_pointer()
            .hover(|el| el.text_color(theme::accent()))
            .child(root_label)
            .on_click(cx.listener(
                move |m: &mut AppModel, _: &ClickEvent, _w, cx| {
                    m.navigate_to(String::new(), cx);
                },
            )),
    );
    if !browser.prefix.is_empty() {
        let mut acc = String::new();
        for seg in browser.prefix.split('/').filter(|s| !s.is_empty()) {
            acc.push_str(seg);
            acc.push('/');
            let target = acc.clone();
            crumbs = crumbs
                .child(
                    div()
                        .text_color(theme::text_faint())
                        .child("·"),
                )
                .child(
                    div()
                        .id(gpui::SharedString::from(format!("crumb-{}", target)))
                        .px_1()
                        .rounded_sm()
                        .font_family(theme::FONT_SERIF)
                        .text_color(theme::text_dim())
                        .cursor_pointer()
                        .hover(|el| el.text_color(theme::accent()))
                        .child(seg.to_string())
                        .on_click(cx.listener(
                            move |m: &mut AppModel, _: &ClickEvent, _w, cx| {
                                m.navigate_to(target.clone(), cx);
                            },
                        )),
                );
        }
    }
    toolbar = toolbar.child(crumbs);

    // 操作按钮
    toolbar = toolbar
        .child(widgets::primary_button(
            "btn-upload",
            i18n::t(i18n::Key::Upload),
            cx.listener(|m: &mut AppModel, _: &ClickEvent, _w, cx| {
                m.prompt_upload(cx);
            }),
        ))
        .child(widgets::button(
            "btn-new-folder",
            i18n::t(i18n::Key::NewFolder),
            cx.listener(
                |m: &mut AppModel, _: &ClickEvent, w, cx| {
                    m.open_folder_modal(w, cx);
                },
            ),
        ))
        .when(selected_count > 0, |el| {
            let label = format!(
                "{} ({selected_count})",
                i18n::t(i18n::Key::DeleteSelected)
            );
            el.child(widgets::danger_button(
                "btn-del-selected",
                label,
                cx.listener(|m: &mut AppModel, _: &ClickEvent, _w, cx| {
                    m.ask_delete_selected(cx);
                }),
            ))
        })
        .child(widgets::button(
            "btn-refresh-objects",
            i18n::t(i18n::Key::Refresh),
            cx.listener(|m: &mut AppModel, _: &ClickEvent, _w, cx| {
                m.refresh_objects(cx);
            }),
        ));
    col = col.child(toolbar);

    if let Some(err) = &browser.error {
        col = col.child(widgets::error_box(err.clone()));
    }

    if browser.loading && browser.entries.is_empty() {
        col = col.child(widgets::empty_state(i18n::t(i18n::Key::Loading)));
        return col;
    }
    if browser.entries.is_empty() && browser.error.is_none() {
        col = col.child(widgets::empty_state(i18n::t(i18n::Key::ObjectEmpty)));
        return col;
    }

    // ---- 表头（可点击排序：当前列高亮 + 方向箭头；列结构与行完全一致）----
    let (cur_key, cur_desc) = (browser.sort, browser.sort_desc);
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .text_size(px(theme::font_caption()))
            .text_color(theme::text_mid())
            .border_b_1()
            .border_color(theme::hairline())
            .child(div().w(Cols::select()).flex_shrink_0())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    // 与行的图标位同宽：表头文字与行文字同一起点
                    .child(div().w(Cols::icon()).flex_shrink_0())
                    .child(sortable_header(
                        "th-sort-name",
                        i18n::t(i18n::Key::BucketName),
                        SortKey::Name,
                        cur_key == SortKey::Name,
                        cur_desc,
                        cx,
                    )),
            )
            .child(
                div().w(Cols::size()).flex_shrink_0().child(sortable_header(
                    "th-sort-size",
                    i18n::t(i18n::Key::Size),
                    SortKey::Size,
                    cur_key == SortKey::Size,
                    cur_desc,
                    cx,
                )),
            )
            .child(
                div().w(Cols::time()).flex_shrink_0().child(sortable_header(
                    "th-sort-time",
                    i18n::t(i18n::Key::LastModified),
                    SortKey::Time,
                    cur_key == SortKey::Time,
                    cur_desc,
                    cx,
                )),
            )
            .child(div().w(Cols::actions()).flex_shrink_0()),
    );

    // ---- 虚拟滚动行 ----
    let weak = cx.entity().downgrade();
    let item_count = browser.entries.len();
    let loading_more = browser.loading_more;
    let has_more = browser.next_token.is_some();

    let list = uniform_list("objects", item_count, move |range, _w, cx| {
        render_rows(weak.clone(), range, cx)
    })
    .flex_1()
    .min_h_0();

    col = col.child(list);

    if loading_more {
        col = col.child(
            div()
                .flex()
                .justify_center()
                .py_1()
                .text_size(px(theme::font_caption()))
                .text_color(theme::text_dim())
                .child(i18n::t(i18n::Key::LoadingMore)),
        );
    } else if has_more {
        col = col.child(div().h(px(8.)));
    }

    col
}

/// 可点击排序的表头单元：点击该列排序，再点翻转方向；
/// 当前排序列文字用主色并显示 ↑/↓。
fn sortable_header(
    id: &'static str,
    label: &'static str,
    key: SortKey,
    active: bool,
    desc: bool,
    cx: &Context<AppModel>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .text_color(if active {
            theme::text()
        } else {
            theme::text_mid()
        })
        .hover(|el| el.text_color(theme::accent()))
        .child(div().child(label))
        .when(active, |el| {
            el.child(
                div()
                    .text_color(theme::accent())
                    .child(if desc { "↓" } else { "↑" }),
            )
        })
        .on_click(cx.listener(
            move |m: &mut AppModel, _: &ClickEvent, _w, cx| {
                m.sort_by(key, cx);
            },
        ))
}

/// 渲染一个可见范围的行（在 App 上下文中通过 weak entity 读取状态）。
fn render_rows(
    weak: WeakEntity<AppModel>,
    range: std::ops::Range<usize>,
    cx: &mut App,
) -> Vec<gpui::AnyElement> {
    weak.update(cx, |m, cx| {
        // 触底检测：接近末尾且有下一页时增量加载
        m.maybe_load_next_page(range.end, cx);
        let Some(browser) = m.browser.as_ref() else {
            return Vec::new();
        };
        let end = range.end.min(browser.entries.len());
        let slice = &browser.entries[range.start..end];
        slice
            .iter()
            .map(|entry| {
                render_row(
                    entry,
                    weak.clone(),
                    browser.selected.contains(&entry.key),
                )
            })
            .collect()
    })
    .unwrap_or_default()
}

fn render_row(
    entry: &ObjectEntry,
    weak: WeakEntity<AppModel>,
    is_selected: bool,
) -> gpui::AnyElement {
    let open_key = entry.key.clone();
    let select_key = entry.key.clone();
    let download_key = entry.key.clone();
    let url_key = entry.key.clone();
    let delete_key = entry.key.clone();
    let is_dir = entry.is_dir;

    // Kami「无信息的 chrome 一律去掉」：目录/文件用文字符号不用彩色 emoji
    let icon = if is_dir { "▸" } else { "·" };
    let size_text = if is_dir {
        "-".to_string()
    } else {
        human_size(entry.size)
    };

    div()
        .id(gpui::SharedString::from(format!("obj-{}", entry.key)))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_1p5()
        .border_b_1()
        .border_color(theme::border_soft())
        .when(is_selected, |el| el.bg(theme::accent_tint()))
        .hover(|el| el.bg(theme::hover()))
        .child(
            div()
                .id(gpui::SharedString::from(format!("sel-{}", entry.key)))
                .w(Cols::select())
                .flex()
                .flex_shrink_0()
                .items_center()
                .cursor_pointer()
                .text_size(px(theme::font_caption()))
                .text_color(if is_selected {
                    theme::accent()
                } else {
                    theme::text_faint()
                })
                .child(if is_selected { "●" } else { "○" })
                .on_click({
                    let weak = weak.clone();
                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let _ = weak.update(cx, |m, cx| {
                            m.toggle_select(select_key.clone(), cx)
                        });
                    }
                }),
        )
        .child(
            div()
                .id(gpui::SharedString::from(format!("name-{}", entry.key)))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .flex_1()
                .min_w_0()
                .child(
                    // 图标固定宽：行与行、行与表头的文字起点完全一致
                    div()
                        .w(Cols::icon())
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .text_color(if is_dir {
                            theme::text_mid()
                        } else {
                            theme::text_faint()
                        })
                        .child(icon),
                )
                .child(
                    div()
                        .w_full()
                        .overflow_x_hidden()
                        .whitespace_nowrap()
                        .text_color(if is_dir {
                            theme::text_mid()
                        } else {
                            theme::text()
                        })
                        // 长文件名缩略：前段 + "…" + 尾段（保留扩展名可辨）
                        .child(truncate_name(&entry.name, 40)),
                )
                .cursor_pointer()
                .on_click({
                    let weak = weak.clone();
                    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let _ = weak.update(cx, |m, cx| {
                            if is_dir {
                                m.open_dir(open_key.clone(), cx);
                            } else {
                                // 点文件名：图片预览，其余选中
                                if crate::state::is_image_key(&open_key) {
                                    m.preview_object(open_key.clone(), cx);
                                } else {
                                    m.toggle_select(open_key.clone(), cx);
                                }
                            }
                        });
                    }
                }),
        )
        .child(
            div()
                .w(Cols::size())
                .flex_shrink_0()
                .text_color(theme::text_faint())
                .child(size_text),
        )
        .child(
            div()
                .w(Cols::time())
                .flex_shrink_0()
                .text_color(theme::text_faint())
                .child(super::buckets::fmt_epoch(entry.last_modified_epoch)),
        )
        .child(
            div()
                .w(Cols::actions())
                .flex()
                .flex_shrink_0()
                .flex_row()
                .gap_1()
                .justify_end()
                .when(!is_dir, |el| {
                    let is_img = crate::state::is_image_key(&entry.key);
                    el.when(is_img, |el| {
                        el.child(widgets::icon_button(
                            gpui::SharedString::from(format!("btn-pv-{}", entry.key)),
                            i18n::t(i18n::Key::Preview),
                            Box::new({
                                let weak = weak.clone();
                                let preview_key = entry.key.clone();
                                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                    let _ = weak.update(cx, |m, cx| {
                                        m.preview_object(preview_key.clone(), cx)
                                    });
                                }
                            }),
                        ))
                    })
                    .child(widgets::icon_button(
                        gpui::SharedString::from(format!("btn-dl-{}", download_key)),
                        i18n::t(i18n::Key::Download),
                        Box::new({
                            let weak = weak.clone();
                            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                let _ = weak.update(cx, |m, cx| {
                                    m.prompt_download(download_key.clone(), cx)
                                });
                            }
                        }),
                    ))
                    .child(widgets::icon_button(
                        gpui::SharedString::from(format!("btn-url-{}", url_key)),
                        i18n::t(i18n::Key::CopyUrl),
                        Box::new({
                            let weak = weak.clone();
                            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                let _ = weak.update(cx, |m, cx| {
                                    m.copy_presigned_url(url_key.clone(), cx)
                                });
                            }
                        }),
                    ))
                })
                .child(widgets::icon_button(
                    gpui::SharedString::from(format!("btn-del-{}", delete_key)),
                    i18n::t(i18n::Key::Delete),
                    Box::new({
                        let weak = weak.clone();
                        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            let _ = weak.update(cx, |m, cx| {
                                m.ask_delete_key(delete_key.clone(), cx)
                            });
                        }
                    }),
                )),
        )
        .into_any_element()
}

/// 人性化的字节大小。
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f32;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// 长文件名缩略：超过 max 字符时保留前段 + "…" + 尾段（尾部含扩展名）。
pub fn truncate_name(name: &str, max: usize) -> String {
    let count = name.chars().count();
    if count <= max {
        return name.to_string();
    }
    let head_len = max.saturating_sub(12);
    let head: String = name.chars().take(head_len).collect();
    let tail: String = name
        .chars()
        .skip(count.saturating_sub(10))
        .collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn truncate_name_keeps_head_and_tail() {
        assert_eq!(truncate_name("a.jpg", 40), "a.jpg");
        let long = "v".repeat(80) + ".png";
        let cut = truncate_name(&long, 40);
        assert!(cut.starts_with('v'));
        assert!(cut.ends_with(".png"));
        assert!(cut.contains('…'));
        assert!(cut.chars().count() < 45);
        // CJK 安全
        let cjk = "照".repeat(60);
        assert!(truncate_name(&cjk, 40).contains('…'));
    }
}
