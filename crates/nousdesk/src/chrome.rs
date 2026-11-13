//! 自绘标题栏（Linux 专用）。
//!
//! 为什么要有这一层：GNOME/Wayland **不给 server-side decorations**，而 GPUI 在
//! 拿不到 SSD 时会退回 client 侧 —— 退回之后它自己不画任何东西，于是窗口既没有标题、
//! 也没有关闭/最大化按钮（日志里那句 "Falling back to client-side decorations" 就是它）。
//! 所以这一条我们自己画。
//!
//! 用 gpui-kit 的 `TitleBar` 而不是自己拿 div 拼：
//!
//! - 拖动（`start_window_move`）、双击最大化、右键窗口菜单、最小化/最大化/关闭三个按钮，
//!   它都接好了；Linux 上按钮走 `on_click`（`window_control_area` 在 Linux 不生效）。
//! - 它会自己判断当前到底是不是 client 装饰（`window_decorations()`）：万一是
//!   X11 上有 WM 给了 SSD，它就不画按钮，避免和 WM 画的那套重一遍（两个关闭按钮）。
//! - 三个按钮是按「WM 支持什么」渲染的（`window.window_controls()`）：平铺式合成器
//!   不支持最小化就不画最小化。
//!
//! 所以这里的职责只有两件：**摆一个品牌名**、**把配色拉到和 App 一致**。

use gpui_kit::component::TitleBar;
use gpui_kit::*;

/// 显示名。窗口标题、标题栏、关于页都该用同一个 —— 搜「NousDesk」找不到窗口是最常见的投诉。
pub const BRAND: &str = "NousDesk";

/// 标题栏底色：比聊天区（`CHAT_BG`）浅一档，和左栏的白卡拉开层次。
const TITLE_BG: u32 = 0xf7f7f7;
const TITLE_FG: u32 = 0x4d5156;

/// 画标题栏。高度、拖动、按钮全交给 `TitleBar`，这里只放名字。
///
/// 第一次画的时候把「当前是哪种装饰 / WM 允许哪些按钮」打进日志：窗口没有关闭按钮时，
/// 第一个要问的就是这两个值（X11 无 WM 时是 `Server` + 全 false，那时按钮本来就该没有）。
pub fn title_bar(window: &mut Window) -> impl IntoElement {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let controls = window.window_controls();
        log::info!(
            "标题栏：装饰={:?} 最小化={} 最大化={} 全屏={} 窗口菜单={}",
            window.window_decorations(),
            controls.minimize,
            controls.maximize,
            controls.fullscreen,
            controls.window_menu
        );
    });

    TitleBar::new()
        .bg(rgb(TITLE_BG))
        // 左边留 12px 是 TitleBar 自己的 `TITLE_BAR_LEFT_PADDING`，再加一点呼吸。
        .child(
            div()
                .pl(px(2.))
                .text_size(px(12.5))
                .text_color(rgb(TITLE_FG))
                .child(BRAND),
        )
}

#[cfg(test)]
mod tests {
    // 只取需要的两个名字：`use super::*` 会把 gpui 的一堆宏再展开一层，够到递归上限。
    use super::{BRAND, TITLE_BG};

    /// 名字只能有一个来源：改一处、三处都跟着动。
    #[test]
    fn brand_is_the_product_name() {
        assert_eq!(BRAND, "NousDesk");
        // 浅色底：深色底会让左栏的白卡和标题栏撞在一起（对比度 < 1.2）。
        assert!(((TITLE_BG >> 16) & 0xff) > 0xe0, "标题栏应该是浅色底");
    }
}
