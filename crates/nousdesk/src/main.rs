//! NousDesk 入口。三栏布局、会话、输入、置顶都在 app.rs 里。
//
// 正式包不要控制台黑框：不加这句，Windows 会给进程配一个 console，
// GPUI 的 INFO 日志（字体/GPU/D3D feature level）就打在那个黑窗里。
// debug 构建保留控制台，方便本地看日志。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod board;
mod board_art;
mod branches;
// Linux 自绘标题栏（GNOME 不给 server-side decorations）。其它平台不挂这个文件。
#[cfg(target_os = "linux")]
mod chrome;
mod composer;
mod config;
mod config_panel;
mod live;
mod md;
mod nav_ui;
mod search;
mod session_nav;
mod settings;
mod slash;
mod templates;
mod theme;

use std::io::Write as _;
use std::sync::OnceLock;

use gpui_kit::component::TitleBar;
use gpui_kit::*;

/// GPUI 的 executor 不是 tokio，而 nous-engine 的 HTTP 全在 tokio 上。
/// 所以起一个 tokio runtime，把 Handle 挂全局，UI 里要用 HTTP 就从这里 spawn。
pub static TOKIO: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// 日志：**落盘**，不再只怼控制台。
///
/// release 包是 GUI 子系统（`windows_subsystem = "windows"`），进程没有 console，
/// `eprintln!` 写进去等于丢进 null —— 真出「Failed to create D3D device」这种致命
/// 问题，界面上看不到、事后也没证据。所以统一写文件：
/// `%LOCALAPPDATA%\nousdesk\desktop.log`（Linux 是 `~/.local/share/nousdesk/`）。
///
/// 超过 2 MiB 就轮转一次（desktop.log → desktop.log.1）：留最近两次启动的日志，
/// 够查「刚才那次为什么没起来」。debug 构建照旧同时打控制台，本地开发看得见。
fn init_log() {
    struct FileLog {
        file: std::sync::Mutex<Option<std::fs::File>>,
        also_console: bool,
    }

    impl log::Log for FileLog {
        fn enabled(&self, meta: &log::Metadata) -> bool {
            meta.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record) {
            if !self.enabled(record.metadata()) {
                return;
            }
            let line = format!(
                "[{}] {} ({})\n",
                record.level(),
                record.args(),
                record.target()
            );
            if let Ok(mut slot) = self.file.lock()
                && let Some(file) = slot.as_mut()
            {
                let _ = file.write_all(line.as_bytes());
            }
            if self.also_console {
                let _ = std::io::stderr().write_all(line.as_bytes());
            }
        }

        fn flush(&self) {
            if let Ok(mut slot) = self.file.lock()
                && let Some(file) = slot.as_mut()
            {
                let _ = file.flush();
            }
        }
    }

    let _ = log::set_boxed_logger(Box::new(FileLog {
        file: std::sync::Mutex::new(open_log_file()),
        also_console: cfg!(debug_assertions),
    }));
    log::set_max_level(log::LevelFilter::Info);
}

/// 打开日志文件。拿不到目录、没权限 → None：日志丢了也不能拦住启动。
fn open_log_file() -> Option<std::fs::File> {
    const MAX_BYTES: u64 = 2 * 1024 * 1024;

    let dir = dirs::data_local_dir()?.join("nousdesk");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("desktop.log");
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        let _ = std::fs::rename(&path, dir.join("desktop.log.1"));
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()
}

/// 把显示器清单打进日志（HiDPI / 多显示器排查用）。
///
/// 只记「有几块屏、每块逻辑尺寸和位置」，不记序列号之类的标识。GPUI 的
/// `PlatformDisplay` **不给每块屏的 scale_factor**（只有 `Window::scale_factor()`，
/// 而且它随窗口当前所在屏变），所以缩放单独在窗口里记，别在这儿猜。
fn log_display_env(cx: &mut App) {
    let displays = cx.displays();
    log::info!("显示器：{} 块", displays.len());
    for d in displays {
        let b = d.bounds();
        log::info!(
            "  显示区域=({:.0},{:.0}) {:.0}×{:.0} 逻辑像素",
            b.origin.x.as_f32(),
            b.origin.y.as_f32(),
            b.size.width.as_f32(),
            b.size.height.as_f32(),
        );
    }
}

fn main() {
    init_log();

    let rt = tokio::runtime::Runtime::new().expect("起 tokio");
    TOKIO.set(rt.handle().clone()).ok();
    // enter 让 main 线程也在 runtime 上下文里；gpui run 会阻塞在这里，rt 一直活着。
    let _guard = rt.enter();

    // AllAssets 而不是 Assets：后者只嵌了 106 个「组件自用」的图标，图标名不在那
    // 106 个里就静静地什么都不画（踩过一次）。全量 1830 个 SVG 一共 2.8MB。
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            // 显示环境打一行日志：缩放 / 多显示器出问题时，先知道当时有几块屏、
            // 各自的逻辑尺寸和位置，别靠猜。Wayland 下 scale_factor 由窗口所在屏决定，
            // 窗口拖到另一块屏会变 —— 这里记的是启动那一刻的主屏值。
            log_display_env(cx);
            // 窗口：标题 NousDesk，1280x800 居中。桌面聊天应用的第一屏。
            //
            // **Linux 自画标题栏**（见 decisions.md）：GNOME 不给 server-side decorations，
            // 走 client 侧的话窗口就没标题栏/关闭按钮。所以拿 gpui-kit 的 `TitleBar` 自己画
            // 一条 —— 拖动、双击最大化、三个窗口按钮都是它自带的（Linux 上走点击，
            // Windows 上走 `window_control_area`）。Windows 那条线冻结了，仍用系统标题栏。
            let mut options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(800.)), cx)),
                ..TitleBar::window_options()
            };
            #[cfg(target_os = "linux")]
            {
                options.window_decorations = Some(WindowDecorations::Client);
                // 标题栏是我们自己画的，但**标题这个元数据还得给**：GNOME 的概览 / 任务栏
                // 读的是它，不是我们画的那行字。给空串的话窗口在任务栏里没有名字。
                options.titlebar = Some(TitlebarOptions {
                    title: Some(crate::chrome::BRAND.into()),
                    appears_transparent: true,
                    ..Default::default()
                });
            }
            #[cfg(not(target_os = "linux"))]
            {
                options.titlebar = Some(TitlebarOptions {
                    // 开发版和「装好的那份」窗口标题一样，任务栏里根本分不出来
                    // （这台机器上还同时开着别人工作区的构建）。debug 构建带上标记。
                    title: Some(if cfg!(debug_assertions) {
                        "NousDesk（开发版）".into()
                    } else {
                        "NousDesk".into()
                    }),
                    ..Default::default()
                });
            }
            gpui_kit::open_window(options, cx, |window, cx| {
                log::info!(
                    "窗口：缩放={:.2} 视口={:.0}×{:.0} 逻辑像素",
                    window.scale_factor(),
                    window.viewport_size().width.as_f32(),
                    window.viewport_size().height.as_f32(),
                );
                cx.new(|cx| app::Desktop::new(window, cx))
            })
            .expect("Failed to open window");
        });
}
