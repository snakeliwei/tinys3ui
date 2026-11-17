//! UI 交互链路测试：用 gpui 的 TestAppContext 模拟真实点击，防止回归。
//!
//! 历史教训：交互元素 id 必须跨帧稳定（gpui 每帧重跑 render 函数）。
//! 动态自增 id 会导致 mouse down/up 无法匹配同一元素，click 永不触发。

use crate::state::{AppModel, Modal};
use gpui::{px, Modifiers, TestAppContext, VisualTestContext};
use s3_core::{ConnectionConfig, ConnectionStore};

/// 侧栏 "+"（新建连接）按钮的近似坐标：侧栏宽 240，
/// 品牌行（pt-3 + logo 22px = 34px）+ 标题行 pt-2，按钮在行末 px-4 边距内。
const PLUS_BUTTON: gpui::Point<gpui::Pixels> = gpui::point(px(208.), px(60.));

/// 以隔离的临时配置目录构建模型（不读用户真实 connections.json）。
/// 目录名用原子计数器：纳秒时间戳在并行测试下可能分辨率不足撞名，
/// 撞名后两个 save 会抢同一个 .tmp 文件，rename 时报 NotFound。
fn test_model() -> AppModel {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("s3ui-test-{}-{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).unwrap();
    // 空配置：侧栏为空态布局，"+" 按钮坐标固定可测
    let store = ConnectionStore::open(dir.join("connections.json"));
    store
        .save(&s3_core::ConnectionsFile::default())
        .unwrap();
    AppModel::with_store(Some(store))
}

fn open_test_window(cx: &mut TestAppContext) -> gpui::WindowHandle<AppModel> {
    cx.add_window(|_window, cx| {
        let mut model = test_model();
        model.start_event_loop(cx);
        model
    })
}

#[gpui::test]
fn clicking_plus_opens_connection_form(cx: &mut TestAppContext) {
    let window = open_test_window(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.simulate_click(PLUS_BUTTON, Modifiers::default());

    window
        .update(&mut cx, |model: &mut AppModel, _window, _app| {
            assert!(
                matches!(model.modal, Modal::ConnectionForm { .. }),
                "点击 + 后应打开连接表单，实际 modal 状态未变化"
            );
        })
        .unwrap();
}

#[gpui::test]
fn empty_state_click_does_not_panic(cx: &mut TestAppContext) {
    let window = open_test_window(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    // 无连接配置时，空态渲染 + 点击 + 打开表单的整条链路不 panic
    cx.simulate_click(PLUS_BUTTON, Modifiers::default());
    window
        .update(&mut cx, |model: &mut AppModel, _window, _app| {
            assert!(model.connections.is_empty());
            assert!(matches!(model.modal, Modal::ConnectionForm { .. }));
        })
        .unwrap();
}
