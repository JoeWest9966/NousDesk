//! 「New Manual」盘面的美术件：调色板、尺寸换算、节点矩形、连线几何。
//!
//! 2026-11-15 起盘面是**轨道**（方案 07）：四张参数卡围着中央一颗 `Create`，
//! 虚线把「谁喂给谁」连出来。不再是 Flexoki 四象限 + 椭圆 —— 那套图的色块
//! 在深色底上只有 14% alpha，缩到右格里会糊成一块脏砖红。
//!
//! 三条纪律：
//! - 颜色只用下面这些常量，别在渲染代码里现编色值。
//! - 尺寸一律先换算成**右格的 cqmin** 再落到 px：盘面只占聊天区那一格。
//! - 节点矩形、圆心、dock 宽度**只在这里算一次**：弹层要贴着节点弹，
//!   两边必须同一个公式，各算各的就会错位。

use gpui_kit::*;

// ── 黑灰打底 + 一个白强调色（Tesla 那一路）────────────────────────────
pub(crate) const BG: u32 = 0x0a0a0b;
/// 节点卡片底。
pub(crate) const PANEL: u32 = 0x121214;
pub(crate) const SURFACE: u32 = 0x18181c;
pub(crate) const BORDER: u32 = 0x232326;
/// 选中态边线。
pub(crate) const LINE: u32 = 0x3a3a42;
pub(crate) const FG: u32 = 0xe9e7e4;
pub(crate) const MUTED: u32 = 0x77736e;
pub(crate) const FAINT: u32 = 0x56534f;
/// 中央那颗圆、选中态、落库主按钮。
pub(crate) const ACCENT: u32 = 0xf4f2ef;
/// 连线。
pub(crate) const WIRE: u32 = 0x2b2b30;
/// `ready` 那行字。
pub(crate) const OK: u32 = 0x6f9c6a;

/// 等宽字。Windows 上 `Consolas` 一定有；`Cascadia Code` 不一定装了。
pub(crate) const MONO: &str = "Consolas";

/// 尺寸换算。**基准是盘面（右格）自己的短边**，等价于 CSS 的 `1cqmin`。
#[derive(Clone, Copy)]
pub(crate) struct M {
    pub cqmin: f32,
    /// 右格宽高。节点左右边距、dock 宽度吃这个百分比。
    pub w: f32,
    pub h: f32,
}

/// 盘面所在的那一格 = 窗口减去图标条和左栏。
pub(crate) fn pane_width(win_w: f32, tab: crate::app::Tab) -> f32 {
    let t = crate::theme::cur();
    let left = match tab {
        crate::app::Tab::Chats => t.rail_w + t.list_w,
        crate::app::Tab::Templates => t.rail_w + t.preset_tree_w + t.template_list_w,
    };
    (win_w - left).max(1.)
}

impl M {
    /// `size` = 整个窗口的 viewport。盘面打开时它顶替 `render_chat`
    /// （连那条 48px 的会话标题栏一起换掉），所以右格高度就是整窗高度。
    pub fn new(size: Size<Pixels>, tab: crate::app::Tab) -> Self {
        let w = pane_width(f32::from(size.width), tab);
        let h = f32::from(size.height).max(1.);
        Self {
            cqmin: w.min(h).max(1.),
            w,
            h,
        }
    }

    /// `12cqmin` 这种写法：单位是容器短边的 1%。
    pub fn v(&self, cqmin: f32) -> Pixels {
        px(cqmin * self.cqmin / 100.)
    }

    /// `clamp(lo, kcqmin, hi)`。
    pub fn f(&self, cqmin: f32, lo: f32, hi: f32) -> Pixels {
        px((cqmin * self.cqmin / 100.).clamp(lo, hi))
    }
}

// ── 四张卡 / 中央圆 / dock 的几何 ────────────────────────────────────

/// 相对右格左上角的矩形，单位 px。
#[derive(Clone, Copy)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.
    }
}

/// 四个角上那张卡。位置和「轨道」demo 一一对应。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Model,
    Context,
    Ext,
    Perms,
}

/// 卡片宽度：右格的 23.5%，夹在 190–268。
pub(crate) fn node_w(m: &M) -> f32 {
    (m.w * 0.235).clamp(190., 268.)
}

/// 卡片高度是**写死**的：弹层要靠 `bottom()` 贴着卡片弹，卡片高度自适应
/// 时弹层就会飘。三张单行卡 112，Context 因为塞了两个子按钮高一点。
pub(crate) fn node_h(slot: Slot) -> f32 {
    match slot {
        Slot::Context => 148.,
        _ => 112.,
    }
}

pub(crate) fn node_rect(m: &M, slot: Slot) -> Rect {
    let w = node_w(m);
    let side = (m.w * 0.055).clamp(10., 80.);
    let top = (m.h * 0.11).clamp(10., 150.);
    let bottom = (m.h * 0.26).clamp(10., 280.);
    let h = node_h(slot);
    let (x, y) = match slot {
        Slot::Model => (side, top),
        Slot::Context => (m.w - side - w, top),
        Slot::Ext => (side, m.h - bottom - h),
        Slot::Perms => (m.w - side - w, m.h - bottom - h),
    };
    Rect {
        x,
        y: y.max(0.),
        w,
        h,
    }
}

/// 中央那颗圆：半径是右格宽的 8.5%，夹在 56–92。
pub(crate) fn core_r(m: &M) -> f32 {
    (m.w * 0.085).clamp(56., 92.)
}

pub(crate) fn core_center(m: &M) -> (f32, f32) {
    (m.w * 0.5, m.h * 0.42)
}

pub(crate) fn dock_w(m: &M) -> f32 {
    (m.w * 0.84).min(720.)
}

pub(crate) const DOCK_H: f32 = 52.;
pub(crate) const DOCK_BOTTOM: f32 = 20.;

/// dock 的矩形（弹层要贴在它上面）。
pub(crate) fn dock_rect(m: &M) -> Rect {
    let w = dock_w(m);
    Rect {
        x: (m.w - w) / 2.,
        y: m.h - DOCK_BOTTOM - DOCK_H,
        w,
        h: DOCK_H,
    }
}

/// 一条连线：从圆心边上出发，指向卡片中心（终点会被卡片盖住，正好停在边上）。
pub(crate) fn wire(m: &M, slot: Slot) -> ((f32, f32), (f32, f32)) {
    let (cx, cy) = core_center(m);
    let r = core_r(m) + 10.;
    let n = node_rect(m, slot);
    let (tx, ty) = (n.cx(), n.cy());
    let (dx, dy) = (tx - cx, ty - cy);
    let len = (dx * dx + dy * dy).sqrt().max(1.);
    ((cx + dx / len * r, cy + dy / len * r), (tx, ty))
}

// ── 弹层（「就地弹出」）的位置 ──────────────────────────────────────

/// 弹层贴哪边：`Top` 用 `top` 定位，`Bottom` 用 `bottom` 定位（向上长）。
#[derive(Clone, Copy)]
pub(crate) enum Anchor {
    Top(f32),
    Bottom(f32),
}

pub(crate) struct Pop {
    pub x: f32,
    pub w: f32,
    pub anchor: Anchor,
}

/// 模型弹层贴在卡片**下方**；技能 / 提示词贴在卡片**左侧**（右上角右边没地方）；
/// 机器 / 目录贴在 dock **上方**、居中。
pub(crate) fn pop_rect(m: &M, slot: Option<Slot>, wide: bool) -> Pop {
    let gap = 14.;
    match slot {
        Some(Slot::Model) => {
            let n = node_rect(m, Slot::Model);
            let w = (m.w - 28.).min(if wide { 600. } else { 520. });
            let x = n.x.clamp(14., (m.w - w - 14.).max(14.));
            let top = n.bottom() + gap;
            // 底下放不下就翻到卡片上面去。
            if top + 280. > m.h - DOCK_H - DOCK_BOTTOM {
                Pop {
                    x,
                    w,
                    anchor: Anchor::Bottom(m.h - n.y + gap),
                }
            } else {
                Pop {
                    x,
                    w,
                    anchor: Anchor::Top(top),
                }
            }
        }
        Some(slot) => {
            let n = node_rect(m, slot);
            let w = 410.;
            // 优先放右侧；右边不够（右上角那张卡就是这样）就翻到左边。
            let x = if n.right() + gap + w <= m.w - 14. {
                n.right() + gap
            } else {
                (n.x - gap - w).max(14.)
            };
            Pop {
                x,
                w,
                anchor: Anchor::Top(n.y),
            }
        }
        None => {
            // 机器 / 目录：dock 上方居中。
            let d = dock_rect(m);
            let w = (m.w - 28.).min(460.);
            Pop {
                x: ((m.w - w) / 2.).max(14.),
                w,
                anchor: Anchor::Bottom(m.h - d.y + 12.),
            }
        }
    }
}
