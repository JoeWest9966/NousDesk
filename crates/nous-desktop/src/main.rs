//! Nous-Desktop 入口。三栏布局、会话、输入、置顶都在 app.rs 里。
//
// 正式包不要控制台黑框：不加这句，Windows 会给进程配一个 console，
// GPUI 的 INFO 日志（字体/GPU/D3D feature level）就打在那个黑窗里。
// debug 构建保留控制台，方便本地看日志。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod live;
mod config;

use std::sync::OnceLock;

use gpui_kit::*;

/// GPUI 的 executor 不是 tokio，而 nous-engine 的 HTTP 全在 tokio 上。
/// 所以起一个 tokio runtime，把 Handle 挂全局，UI 里要用 HTTP 就从这里 spawn。
pub static TOKIO: OnceLock<tokio::runtime::Handle> = OnceLock::new();

fn init_log() {
    struct StderrLog;
    impl log::Log for StderrLog {
        fn enabled(&self, meta: &log::Metadata) -> bool {
            meta.level() <= log::Level::Info
        }
        fn log(&self, record: &log::Record) {
            eprintln!("[{}] {}", record.level(), record.args());
        }
        fn flush(&self) {}
    }
    let _ = log::set_boxed_logger(Box::new(StderrLog));
    log::set_max_level(log::LevelFilter::Info);
}

fn main() {
    init_log();

    let rt = tokio::runtime::Runtime::new().expect("起 tokio");
    TOKIO.set(rt.handle().clone()).ok();
    // enter 让 main 线程也在 runtime 上下文里；gpui run 会阻塞在这里，rt 一直活着。
    let _guard = rt.enter();

    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        // 窗口：标题 Nous，1280x800 居中。桌面聊天应用的第一屏。
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(800.)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some("Nous".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| app::Desktop::new(window, cx))
        })
        .expect("Failed to open window");
    });
}
