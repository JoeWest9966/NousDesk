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

// ── 颜色：**全部从当前这套皮取**，不在这里另起一套 ────────────────────
//
// 盘面是聊天区的一格，皮换了它就得跟着换：底色用 `chat_bg`，卡用 `panel`，
// 虚线用 `line`，圆用 `text`（浅皮就是黑圆白字、深皮是白圆黑字 —— 五套皮都成立）。
// 强调色只用在「选中」，别拿去当圆 —— 默认皮是绿的，一颗绿圆很怪。

use crate::theme::cur;

/// 盘面底。
pub(crate) fn bg() -> u32 {
    cur().p.chat_bg
}
/// 四张卡 / 弹层的底。
pub(crate) fn panel() -> u32 {
    cur().p.panel
}
/// 卡里的子块（技能 / 提示词那两个小方块）。
pub(crate) fn surface() -> u32 {
    cur().p.surface
}
/// 边框（皮的 `line`）。
pub(crate) fn border() -> u32 {
    cur().p.line
}
/// 更重一档的边线（hover / 选中）。
pub(crate) fn line() -> u32 {
    cur().p.line_strong
}
/// 正文。
pub(crate) fn fg() -> u32 {
    cur().p.text
}
pub(crate) fn muted() -> u32 {
    cur().p.text_2
}
pub(crate) fn faint() -> u32 {
    cur().p.faint
}
/// 中央那颗圆、环上三个 pill 的底（= 正文色）。字用 [`bg`]。
pub(crate) fn solid() -> u32 {
    cur().p.text
}
/// 皮的强调色：选中、勾选、主按钮。
pub(crate) fn accent() -> u32 {
    cur().p.accent
}
/// 连线（虚线）。
pub(crate) fn wire() -> u32 {
    cur().p.line
}
/// `ready` 那行字。
pub(crate) fn ok() -> u32 {
    cur().p.ok
}
/// 危险 / 失败。
pub(crate) fn danger() -> u32 {
    cur().p.danger
}

/// 等宽字：皮的 `mono` 没给就退回 `Consolas`（Linux 上靠字体回退顶上）。
pub(crate) fn mono() -> &'static str {
    let m = cur().mono;
    if m.is_empty() {
        "Consolas"
    } else {
        m
    }
}

/// 尺寸换算。**基准是盘面（右格）自己的短边**，等价于 CSS 的 `1cqmin`。
#[derive(Clone, Copy)]
pub(crate) struct M {
    pub cqmin: f32,
    /// 右格宽高。节点左右边距、dock 宽度吃这个百分比。
    pub w: f32,
    pub h: f32,
}

/// 自绘标题栏占多高。Linux 上是我们自己画的 34px；别的平台窗口有系统装饰，
/// 客户区里不占地方。
fn title_bar_h() -> f32 {
    #[cfg(target_os = "linux")]
    {
        crate::chrome::TITLE_BAR_H
    }
    #[cfg(not(target_os = "linux"))]
    {
        0.
    }
}

/// 盘面所在的那一格 = 窗口减去图标条和左栏。
pub(crate) fn pane_width(win_w: f32, tab: crate::app::Tab) -> f32 {
    let t = crate::theme::cur();
    // 窄窗时 Template 页的类别树收进了抽屉、列表也窄一点 —— 这里得跟渲染那边
    // 用同一份宽度，否则右格会算窄 300 多像素，盘面直接画到窗口外面。
    let folder = win_w < crate::app::TREE_FOLD_W;
    let left = match tab {
        crate::app::Tab::Chats => t.rail_w + t.list_w,
        crate::app::Tab::Templates if folder => t.rail_w + crate::app::TEMPLATE_LIST_W_NARROW,
        crate::app::Tab::Templates => t.rail_w + t.preset_tree_w + t.template_list_w,
    };
    (win_w - left).max(1.)
}

impl M {
    /// `size` = 整个窗口的 viewport。盘面打开时它顶替 `render_chat`
    /// （连那条 48px 的会话标题栏一起换掉），所以右格高度就是整窗高度。
    pub fn new(size: Size<Pixels>, tab: crate::app::Tab) -> Self {
        let w = pane_width(f32::from(size.width), tab);
        // 盘面画在自绘标题栏**下面**：视口高度含标题栏，得先扣掉它，
        // 不然整页几何一起下移（环顶到卡片上、状态行出屏）。
        let h = (f32::from(size.height) - title_bar_h()).max(1.);
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

// ── 05 环：圆外一圈虚线，三个值挂在环上 ────────────────────────────────
//
// 为什么不留在底部那条 dock：一条 720px 横贯盘面的窄条，读起来是「工具栏」，
// 和上面四张卡不是一个语法。环把名字 / 机器 / 目录收回圆边上 —— 它们是**从圆心
// 长出来的参数**，所以用同一种画法（节点 + 虚线），也所以三个 pill 的底色
// 和那颗圆一模一样（见 `solid()`）。

/// 环的中心与半径（中心就是那颗圆的中心）。横向吃右格宽的 16.5%，
/// 纵向吃右格高的 19.5% —— 右格通常比高宽，两个数合起来才是正的圆。
pub(crate) fn ring(m: &M) -> (f32, f32, f32, f32) {
    let (cx, cy) = core_center(m);
    let (rx, ry) = ring_r(m);
    (cx, cy, rx, ry)
}

/// 圆边 → 环上某个值的一条短线（终点藏在 pill 底下）。
pub(crate) fn ring_r(m: &M) -> (f32, f32) {
    (m.w * 0.165, m.h * 0.195)
}

/// 环上的三个值。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Foot {
    /// 环底正中：这条对话叫什么。
    Name,
    /// 左下：跑在哪台机器。
    Machine,
    /// 右下：工作目录。
    Dir,
}

pub(crate) const FOOT_H: f32 = 34.;

fn foot_w(m: &M, f: Foot) -> f32 {
    let want: f32 = match f {
        Foot::Name => 236.,
        Foot::Machine => 116.,
        Foot::Dir => 168.,
    };
    // 窄右格里别把两个 pill 撞到一起。
    want.min(m.w * 0.3)
}

/// 三个值的矩形。**只在这里算一次** —— 弹层要贴着 pill 弹，两边同一个公式。
pub(crate) fn foot_rect(m: &M, f: Foot) -> Rect {
    let (cx, cy) = core_center(m);
    let (rx, ry) = ring_r(m);
    // 140° / 40° / 90°：挂在环的下半圈，左右两颗更靠外，
    // 和环底那颗名字拉开距离（三颗挤在一起就分不清谁是谁）。
    let (kx, ky) = (0.766_044_4, 0.642_787_6);
    let (x, y) = match f {
        Foot::Machine => (cx - rx * kx, cy + ry * ky),
        Foot::Dir => (cx + rx * kx, cy + ry * ky),
        Foot::Name => (cx, cy + ry),
    };
    let w = foot_w(m, f);
    Rect {
        x: x - w / 2.,
        y: y - FOOT_H / 2.,
        w,
        h: FOOT_H,
    }
}

pub(crate) fn foot_wire(m: &M, f: Foot) -> ((f32, f32), (f32, f32)) {
    let (cx, cy) = core_center(m);
    let r = core_r(m) + 4.;
    let t = foot_rect(m, f);
    let (tx, ty) = (t.cx(), t.cy());
    let (dx, dy) = (tx - cx, ty - cy);
    let len = (dx * dx + dy * dy).sqrt().max(1.);
    ((cx + dx / len * r, cy + dy / len * r), (tx, ty))
}

/// 环底下那行状态的位置（`top`）。
pub(crate) fn status_y(m: &M) -> f32 {
    let (_, cy) = core_center(m);
    let (_, ry) = ring_r(m);
    cy + ry + FOOT_H / 2. + 14.
}

/// 一条连线：从圆心边上出发，指向卡片中心（终点会被卡片盖住，正好停在边上）。
pub(crate) fn card_wire(m: &M, slot: Slot) -> ((f32, f32), (f32, f32)) {
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
pub(crate) fn pop_rect(m: &M, slot: Option<Slot>, wide: bool, for_dir: bool) -> Pop {
    let gap = 14.;
    match slot {
        Some(Slot::Model) => {
            let n = node_rect(m, Slot::Model);
            let w = (m.w - 28.).min(if wide { 600. } else { 520. });
            let x = n.x.clamp(14., (m.w - w - 14.).max(14.));
            let top = n.bottom() + gap;
            // 底下放不下就翻到卡片上面去。
            if top + 280. > m.h - 40. {
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
            // 机器 / 目录：贴着被点的那颗 pill 弹（就地弹出的同一条规矩）。
            let f = if for_dir { Foot::Dir } else { Foot::Machine };
            let t = foot_rect(m, f);
            let w = (m.w - 28.).min(460.);
            let x = (t.cx() - w / 2.).clamp(14., (m.w - w - 14.).max(14.));
            Pop {
                x,
                w,
                anchor: Anchor::Bottom(m.h - t.y + 10.),
            }
        }
    }
}
