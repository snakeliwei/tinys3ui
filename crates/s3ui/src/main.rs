// Windows 发布版不随窗口启动控制台（debug 构建保留，便于 cargo run 看日志）
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod i18n;
mod settings;
mod state;
mod text_input;
mod theme;
mod views;
#[cfg(test)]
mod ui_tests;

use assets::EmbeddedAssets;
use gpui::{
    prelude::*, px, size, App, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use state::AppModel;
use text_input::*;

fn main() {
    Application::new()
        .with_assets(EmbeddedAssets)
        .run(|cx: &mut App| {
        // 文本输入的按键绑定（作用域限定在 TextInput 上下文内）
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some("TextInput")),
            KeyBinding::new("delete", Delete, Some("TextInput")),
            KeyBinding::new("left", Left, Some("TextInput")),
            KeyBinding::new("right", Right, Some("TextInput")),
            KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
            KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
            KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
            KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
            KeyBinding::new("cmd-v", Paste, Some("TextInput")),
            KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
            KeyBinding::new("home", Home, Some("TextInput")),
            KeyBinding::new("end", End, Some("TextInput")),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("TextInput")),
        ]);

        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(i18n::t(i18n::Key::AppTitle).into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|cx| {
                        let mut model = AppModel::new();
                        // 启动传输事件监听（需要 Context）
                        model.start_event_loop(cx);
                        model
                    })
                },
            )
            .expect("打开主窗口失败");
        let _ = window;
        cx.activate(true);
    });
}
