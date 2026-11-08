//! Nous-Desktop 占位入口。
//!
//! 三栏布局、会话、输入、置顶都还没写。先让窗口起得来，再去桌面上堆肉。
//! headless 上 GPUI 起不来，别在这台机器上 `cargo run`。

use gpui_kit::component::*;
use gpui_kit::*;

pub struct Desktop;

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child("Nous-Desktop — 三栏还没写")
    }
}

fn main() {
    gpui_kit::application().run(move |cx| {
        // 用任何 GPUI Component 之前必须先 init。
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Desktop))
            .expect("Failed to open window");
    });
}
