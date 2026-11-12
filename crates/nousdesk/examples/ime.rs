//! 输入法（IME）样品间。
//!
//! 只做一件事：把**生产用的那个** `Textarea` 单独摆出来，旁边实时显示它现在的内容，
//! 加上终端里每变一次打一行 `TEXT: "…"`。中文输入法有没有生效，看这两处就知道：
//!
//! - 组字（还没上屏）时框里应该在动、且有下划线；屏幕上方的绿字是**已上屏**的内容。
//! - 上屏后终端会多一行 `TEXT:`。
//!
//! 为什么不直接拿整只 app 验：整只 app 要先连宿主、点进一条会话、找到输入框，
//! 每个环节都能把「输入法坏了」和「别的东西坏了」搅在一起。IME 是平台层的事，
//! 拿最小的一块验，坏了一眼就能定位。
//!
//! 跑：`cargo run -p nousdesk --example ime`
//! X11/ibus 的跑法见 `AGENTS.md`（Xvfb + ibus-daemon + xdotool）。

use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::*;

struct Probe {
    input: Entity<TextareaState>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 已上屏的内容 —— 组字过程中这里**不该**变（mark 还没 commit）。
        let committed = self.input.read(cx).value().to_string();

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .bg(rgb(0xededed))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(rgb(0x1a7f37))
                    .child(format!("committed: {committed:?}")),
            )
            .child(
                div()
                    .flex_1()
                    .p(px(10.))
                    .rounded(px(8.))
                    .bg(rgb(0xffffff))
                    .child(Textarea::new(&self.input).h_full()),
            )
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(480.), px(260.)), cx)),
                titlebar: Some(TitlebarOptions {
                    title: Some("ime 样品间".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let input = cx.new(|cx| TextareaState::new(window, cx));
                // 一开窗就聚焦，否则敲字进不去，还得先点一下。
                window.focus(&input.read(cx).focus_handle(cx), cx);

                let watched = input.clone();
                cx.new(move |cx| {
                    // 内容一变就打一行：组字、上屏、删字都看得见。
                    cx.observe(&watched, |_, input, cx| {
                        println!("TEXT: {:?}", input.read(cx).value().to_string());
                        cx.notify();
                    })
                    .detach();
                    Probe {
                        input: watched.clone(),
                    }
                })
            })
            .expect("开窗");
        });
}
