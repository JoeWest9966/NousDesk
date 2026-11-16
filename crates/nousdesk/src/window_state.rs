//! 窗口几何落盘：上次关窗时多大、在哪儿、是不是最大化。
//!
//! ## 为什么单独一个文件
//!
//! `desktop.toml` 是「连哪个宿主」（便携包里排在 exe 旁边，可能只读），
//! `theme.toml` 是皮。窗口几何是**机器状态**不是用户偏好 —— 换台机器、换块屏它本来就该变，
//! 所以放 `$XDG_STATE_HOME/nousdesk/window.toml`（默认 `~/.local/state/nousdesk/`），
//! 跟「这份机器的状态」一个意思。没有 `XDG_STATE_HOME` 就退回 `~/.local/share/nousdesk/`
//! （跟日志同目录）。
//!
//! ## Wayland 上位置可能不生效
//!
//! 合成器决定窗口放哪儿，GNOME 不认客户端的定位请求 —— 所以 `x` / `y` 只在 X11 上真生效，
//! 尺寸和最大化两边都生效。**这不是 bug**，别去协议里绕；真要做「回到上次那块屏」
//! 得等合成器给 API（或者用户在 GNOME 里自己记）。
//!
//! ## 读坏了不报错
//!
//! 没文件 / 写坏了 / 尺寸不是数 → 一律回 `None`，调用方用「1280×800 居中」。
//! UI 偏好不该挡住启动（跟 `theme.rs` 同一条口径）；`desktop.toml` 写坏要报错，两者严格程度不同。

use std::path::PathBuf;

use gpui_kit::*;

use crate::{MIN_WINDOW_H, MIN_WINDOW_W};

/// 窗口文件的名字（在 state 目录下的 `nousdesk/` 里）。
const FILE: &str = "window.toml";
/// 跟日志同一个目录名。
const DIR: &str = "nousdesk";

/// 上次关窗时的几何（逻辑像素）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    /// 左上角。没记过 → `None`（回居中）。Wayland 上会被忽略，见文件头。
    pub x: Option<f32>,
    pub y: Option<f32>,
    /// 最大化要单独记：恢复尺寸和「是不是最大化」是两件事。
    ///
    /// 全屏也当最大化存 —— 下次开在最大化，而不是一开就占满整块屏（那样连标题栏都看不见）。
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1280.,
            height: 800.,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

impl WindowState {
    /// 窗口文件在哪儿（父目录可能还不存在）。
    pub fn path() -> Option<PathBuf> {
        let base = dirs::state_dir().or_else(dirs::data_local_dir)?;
        Some(base.join(DIR).join(FILE))
    }

    /// 读上次的几何。**任何失败都回 `None`**：没文件、没目录、写坏了、尺寸不是有限数。
    pub fn load() -> Option<Self> {
        Self::path().and_then(|path| Self::load_from(&path))
    }

    /// 落盘。失败只返回错误给调用方记日志，不弹给用户。
    pub fn save(self) -> Result<PathBuf, String> {
        let path = Self::path().ok_or_else(|| "找不到 state 目录".to_string())?;
        self.save_to(&path)?;
        Ok(path)
    }

    /// 从 GPUI 的窗口状态转一份。最大化 / 全屏里带的都是**恢复尺寸**，照收。
    pub fn from_bounds(bounds: WindowBounds) -> Self {
        let (rect, maximized) = match bounds {
            WindowBounds::Windowed(rect) => (rect, false),
            WindowBounds::Maximized(rect) => (rect, true),
            WindowBounds::Fullscreen(rect) => (rect, true),
        };
        Self {
            width: rect.size.width.as_f32(),
            height: rect.size.height.as_f32(),
            x: Some(rect.origin.x.as_f32()),
            y: Some(rect.origin.y.as_f32()),
            maximized,
        }
        .normalized()
    }

    /// 转成打开窗口的参数。记过位置就用，没记过 / 位置不可信 → 居中（要 `cx` 才知道屏幕多大）。
    pub fn to_window_bounds(self, cx: &mut App) -> WindowBounds {
        let size = size(px(self.width), px(self.height));
        let rect = match (self.x, self.y) {
            (Some(x), Some(y)) => Bounds {
                origin: point(px(x), px(y)),
                size,
            },
            _ => {
                // `centered` 返回的是带屏幕坐标的 Windowed，只借它的 origin。
                let WindowBounds::Windowed(centered) = WindowBounds::centered(size, cx) else {
                    unreachable!("centered 只会返回 Windowed")
                };
                centered
            }
        };
        if self.maximized {
            WindowBounds::Maximized(rect)
        } else {
            WindowBounds::Windowed(rect)
        }
    }

    /// 收口：尺寸不小于窗口管理器允许的下限、且必须是有限数（NaN / inf → 出厂尺寸）。
    ///
    /// 位置不夹 —— 用户可能把窗口放在第二块屏的负坐标上，夹回主屏反而是错的。
    pub fn normalized(mut self) -> Self {
        let (w, h) = if self.width.is_finite() && self.height.is_finite() {
            (self.width, self.height)
        } else {
            (Self::default().width, Self::default().height)
        };
        self.width = w.max(MIN_WINDOW_W);
        self.height = h.max(MIN_WINDOW_H);
        if !self.x.is_some_and(f32::is_finite) || !self.y.is_some_and(f32::is_finite) {
            self.x = None;
            self.y = None;
        }
        self
    }

    // ── 可测的两步（不动环境变量）──────────────────────────

    fn load_from(path: &std::path::Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let file = toml::from_str::<File>(&text).ok()?;
        Some(
            Self {
                width: file.width,
                height: file.height,
                x: file.x,
                y: file.y,
                maximized: file.maximized,
            }
            .normalized(),
        )
    }

    fn save_to(self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("建不了目录 {}：{e}", dir.display()))?;
        }
        let file = File {
            width: self.width,
            height: self.height,
            x: self.x,
            y: self.y,
            maximized: self.maximized,
        };
        let text = toml::to_string(&file).map_err(|e| format!("序列化不了：{e}"))?;
        std::fs::write(path, text).map_err(|e| format!("写不了 {}：{e}", path.display()))
    }
}

/// 盘上的形状。位置两个字段分开存：只写一个（手改坏的）就当没记过。
#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    width: f32,
    height: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y: Option<f32>,
    #[serde(default)]
    maximized: bool,
}

#[cfg(test)]
mod tests {
    use super::WindowState;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nousdesk-window-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join("window.toml")
    }

    #[test]
    fn 存了再读一模一样() {
        let path = tmp("round-trip");
        let want = WindowState {
            width: 1100.,
            height: 720.,
            x: Some(-40.),
            y: Some(120.),
            maximized: true,
        };
        want.save_to(&path).expect("写得进临时目录");
        assert_eq!(WindowState::load_from(&path), Some(want));
    }

    #[test]
    fn 没位置时读回来还是居中() {
        let path = tmp("no-pos");
        let want = WindowState {
            width: 900.,
            height: 600.,
            x: None,
            y: None,
            maximized: false,
        };
        want.save_to(&path).expect("写得进临时目录");
        assert_eq!(WindowState::load_from(&path), Some(want));
    }

    #[test]
    fn 坏文件当没读过() {
        let path = tmp("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "width = \"很宽\"\n").unwrap();
        assert_eq!(WindowState::load_from(&path), None);
        assert_eq!(WindowState::load_from(&tmp("missing")), None);
    }

    #[test]
    fn 太小或不是数就夹回下限() {
        let path = tmp("tiny");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "width = 10.0\nheight = -5.0\n").unwrap();
        let got = WindowState::load_from(&path).expect("合法 toml");
        assert_eq!((got.width, got.height), (crate::MIN_WINDOW_W, crate::MIN_WINDOW_H));

        let nan = WindowState {
            width: f32::NAN,
            height: f32::INFINITY,
            x: Some(f32::NAN),
            y: Some(10.),
            maximized: false,
        }
        .normalized();
        assert_eq!((nan.width, nan.height), (1280., 800.));
        assert_eq!((nan.x, nan.y), (None, None), "位置只要有一个不是数就整对丢掉");
    }

    #[test]
    fn 全屏当最大化存() {
        use gpui_kit::*;
        let rect = Bounds {
            origin: point(px(12.), px(34.)),
            size: size(px(1000.), px(700.)),
        };
        let got = WindowState::from_bounds(WindowBounds::Fullscreen(rect));
        assert!(got.maximized, "一开就全屏会把标题栏也吞掉，收敛成最大化");
        assert_eq!((got.width, got.height), (1000., 700.));
        assert_eq!((got.x, got.y), (Some(12.), Some(34.)));
    }
}
