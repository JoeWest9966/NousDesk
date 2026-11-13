//! 剪贴板样品间。
//!
//! 只做一件事：把「读系统剪贴板 / 写系统剪贴板 / Textarea 里的粘贴」这三段分开验，
//! 每段打一行给人看。窗口里还是**生产用的那个 `Textarea`**，所以真按 `Ctrl+V`
//! 也能在同一块地方验。
//!
//! 为什么需要它：剪贴板在 Linux 上是**跨进程协商**（wl_data_device + 一堆 mime 类型），
//! 坏了的表现是「粘贴没反应」，跟输入法坏了、焦点丢了、模型挂了长得一模一样。
//! 把平台层单独钉住，剩下的才轮得到怀疑上层。
//!
//! 跑：`cargo run -p nousdesk --example clipboard`
//! 外面的对照工具是 `wl-copy` / `wl-paste`（Debian：`apt install wl-clipboard`）。
//!
//! 顺序（时间写在代码里，一眼对得上）：
//!
//! ```text
//! t=1s  读系统剪贴板     → 终端 READ1: Some("…")    ← 别的程序写进去的，我们应该读到
//! t=2s  往 Textarea 粘贴 → 终端 TEXT: "…"           ← 平台读 + 组件插入，两段都通才有这行
//! ```
//!
//! **写剪贴板为什么不在这儿自动做**：Wayland 的 `set_selection` 要带上最近一次输入事件的
//! serial（键盘/指针都行）。脚本里没有任何输入事件，合成器会直接丢掉那个请求 —— 看起来
//! 就像「写剪贴板坏了」。所以写的那一段留给真人：按 `Ctrl+A` 再 `Ctrl+C`，然后
//! 终端里 `wl-paste` 看结果。

use std::time::Duration;

use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::*;

struct Probe {
    input: Entity<TextareaState>,
    /// 上一次打出去的内容。文本没变就别重复打（一次通知会触发好几次渲染）。
    last: String,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.input.read(cx).value().to_string();
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
                    .child(format!("committed: {text:?}")),
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
                    title: Some("clipboard 样品间".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let input = cx.new(|cx| TextareaState::new(window, cx));
                window.focus(&input.read(cx).focus_handle(cx), cx);

                let watched = input.clone();
                let probe = cx.new(move |cx| {
                    cx.observe(&watched, |this: &mut Probe, input, cx| {
                        let now = input.read(cx).value().to_string();
                        if now != this.last {
                            this.last = now.clone();
                            println!("TEXT: {now:?}");
                        }
                        cx.notify();
                    })
                    .detach();
                    Probe {
                        input: watched.clone(),
                        last: String::new(),
                    }
                });

                // 两段自动验证：读 → 粘贴。间隔 1s 是给 wl-copy 留的时间。
                println!(
                    "（自动两段验完后，手工验写：Ctrl+A 选中，Ctrl+C 复制，终端里跑 wl-paste）"
                );
                let target = probe.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let got = cx.update(|cx| cx.read_from_clipboard().and_then(|i| i.text()));
                    println!("READ1: {got:?}");

                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    // 走的是 Textarea 自己的粘贴路径（读平台剪贴板 → 插到光标处）。
                    let _ = cx.update(|cx| {
                        target.update(cx, |_, cx| {
                            cx.dispatch_action(&gpui_kit::base::input::Paste);
                        })
                    });
                })
                .detach();

                probe
            })
            .expect("开窗");
        });
}
