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
    /// 右格宽高（px）。
    pub w: f32,
    pub h: f32,
    /// 当前用哪档布局。宽度掉下去就换档，而不是把同一套硬塞。
    pub layout: Layout,
    /// 设计值 → px 的比例。**全部几何只吃这一个数** —— 页面永远不会变形。
    pub scale: f32,
    /// 内容框在右格里的居中偏移。
    pub ox: f32,
    pub oy: f32,
}

/// 三档布局。断点按右格**宽**（不是窗口宽）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Layout {
    /// ≥ 780：四角卡 + 环 + 三颗 pill。
    Wide,
    /// 620–780：去掉环，卡缩到 pane 的 32%，三颗 pill 横排贴在圆下。
    Compact,
    /// < 620：竖排一列，允许纵向滚动。
    Stack,
}

impl Layout {
    /// 设计基准：Wide 是 1280 窗口下好看的那版；Compact 是它去掉环之后的压缩版。
    pub fn base(self) -> (f32, f32) {
        match self {
            Layout::Wide => (940., 764.),
            Layout::Compact => (700., 640.),
            Layout::Stack => (300., 700.),
        }
    }
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
        crate::app::Tab::Triggers => t.rail_w + 214.,
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
        let layout = if w >= 780. {
            Layout::Wide
        } else if w >= 620. {
            Layout::Compact
        } else {
            Layout::Stack
        };
        let (bw, bh) = layout.base();
        // Wide / Compact 同时受宽高约束（等比，永不变形）；Stack 是竖排列表，
        // 高度靠滚动解决，所以只按宽度缩，而且不许缩得太小。
        let scale = match layout {
            Layout::Stack => (w / bw).clamp(0.85, 1.4),
            _ => (w / bw).min(h / bh).clamp(0.2, 1.6),
        };
        let (sw, sh) = (bw * scale, bh * scale);
        Self {
            w,
            h,
            layout,
            scale,
            ox: ((w - sw) / 2.).max(0.),
            oy: ((h - sh) / 2.).max(0.),
        }
    }

    /// 设计值 → px。**盘面所有尺寸都走这里**，只乘一个 `scale`。
    pub fn s(&self, design: f32) -> Pixels {
        px(design * self.scale)
    }

    /// 旧的 cqmin 写法，保留兼容：1 单位 = 设计基准短边的 1%。
    pub fn v(&self, cqmin: f32) -> Pixels {
        self.s(cqmin * 7.4)
    }

    /// `clamp(lo, kcqmin, hi)`，三个数都跟 scale 走，最后兜一个 9px 下限
    /// —— 缩小到半屏时字不能变成天书。
    pub fn f(&self, cqmin: f32, lo: f32, hi: f32) -> Pixels {
        px((cqmin * 7.4).clamp(lo, hi) * self.scale)
            .max(px(9.))
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

/// 四张卡的设计尺寸（相对设计基准，不乘 scale）。宽高比固定 220:92 ≈ 0.42，
/// 宽变了高跟着变 —— 以前一个按宽一个写死，宽屏上就成了 268×112 的扁条。
pub(crate) fn node_size(m: &M, slot: Slot) -> (f32, f32) {
    let (w, h) = match m.layout {
        Layout::Wide => (220., if slot == Slot::Context { 116. } else { 92. }),
        Layout::Compact => (322., if slot == Slot::Context { 150. } else { 128. }),
        Layout::Stack => (240., 64.),
    };
    (w * m.scale, h * m.scale)
}

pub(crate) fn node_rect(m: &M, slot: Slot) -> Rect {
    let (w, h) = node_size(m, slot);
    // 设计值（未乘 scale）：四角的位置。
    let (side, top, bottom) = match m.layout {
        Layout::Wide => (52., 78., 150.),
        Layout::Compact => (21., 38., 102.),
        Layout::Stack => (16., 16., 16.),
    };
    let (bx, by) = (side * m.scale, top * m.scale);
    let (bw, bh) = Layout::base(m.layout);
    let (sw, sh) = (bw * m.scale, bh * m.scale);
    let (x, y) = match slot {
        Slot::Model => (bx, by),
        Slot::Context => (sw - bx - w, by),
        Slot::Ext => (bx, sh - bottom * m.scale - h),
        Slot::Perms => (sw - bx - w, sh - bottom * m.scale - h),
    };
    Rect {
        x: m.ox + x,
        y: m.oy + y,
        w,
        h,
    }
}

/// 中央那颗圆：半径 74（设计值）。
pub(crate) fn core_r(m: &M) -> f32 {
    let r = match m.layout {
        Layout::Wide => 74.,
        Layout::Compact => 48.,
        Layout::Stack => 46.,
    };
    r * m.scale
}

/// 圆心。Wide 在设计坐标 (470, 320)；Compact 让开上下两排卡，落在中间那段的正中。
pub(crate) fn core_center(m: &M) -> (f32, f32) {
    let (cx, cy) = match m.layout {
        Layout::Wide => (470., 320.),
        Layout::Compact => (350., 250.),
        Layout::Stack => (150., 60.),
    };
    (m.ox + cx * m.scale, m.oy + cy * m.scale)
}

// ── 05 环：圆外一圈虚线，三个值挂在环上 ────────────────────────────────
//
// 为什么不留在底部那条 dock：一条 720px 横贯盘面的窄条，读起来是「工具栏」，
// 和上面四张卡不是一个语法。环把名字 / 机器 / 目录收回圆边上 —— 它们是**从圆心
// 长出来的参数**，所以用同一种画法（节点 + 虚线），也所以三个 pill 的底色
// 和那颗圆一模一样（见 `solid()`）。

/// 环的半径（设计值）。**正圆**：rx = ry，不再一个吃宽一个吃高（那样越宽越扁）。
pub(crate) fn ring_r(m: &M) -> f32 {
    150. * m.scale
}

/// 环的中心与半径。Compact / Stack 不画环。
pub(crate) fn ring(m: &M) -> (f32, f32, f32, f32) {
    let (cx, cy) = core_center(m);
    let r = ring_r(m);
    (cx, cy, r, r)
}

pub(crate) fn has_ring(m: &M) -> bool {
    m.layout == Layout::Wide
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

fn foot_size(m: &M, f: Foot) -> (f32, f32) {
    let (w, h): (f32, f32) = match m.layout {
        Layout::Wide => match f {
            Foot::Name => (200., 30.),
            Foot::Machine => (96., 30.),
            Foot::Dir => (140., 30.),
        },
        // 紧凑档：三颗横排，宽度收窄。
        Layout::Compact => match f {
            Foot::Name => (200., 28.),
            Foot::Machine => (66., 28.),
            Foot::Dir => (86., 28.),
        },
        Layout::Stack => match f {
            Foot::Name => (110., 30.),
            Foot::Machine => (72., 30.),
            Foot::Dir => (96., 30.),
        },
    };
    // 窄右格里别让两颗撞上。
    let w = w.min(m.w / m.scale * 0.34);
    (w * m.scale, h * m.scale)
}

pub(crate) fn foot_rect(m: &M, f: Foot) -> Rect {
    let (w, h) = foot_size(m, f);
    let (cx, cy) = core_center(m);
    let (x, y) = match m.layout {
        Layout::Wide => {
            let r = ring_r(m);
            // 140° / 40° / 90°：挂在环的下半圈，左右两颗更靠外。
            let (kx, ky) = (0.766_044_4, 0.642_787_6);
            match f {
                Foot::Machine => (cx - r * kx, cy + r * ky),
                Foot::Dir => (cx + r * kx, cy + r * ky),
                Foot::Name => (cx, cy + r),
            }
        }
        _ => {
            // 圆下方横排一行，三颗一起居中。
            let gap = 6. * m.scale;
            let (wm, wn, wd) = (
                foot_size(m, Foot::Machine).0,
                foot_size(m, Foot::Name).0,
                foot_size(m, Foot::Dir).0,
            );
            let total = wm + wn + wd + gap * 2.;
            let left = cx - total / 2.;
            let y = cy + core_r(m) + 18. * m.scale;
            let x = match f {
                Foot::Machine => left + wm / 2.,
                Foot::Name => left + wm + gap + wn / 2.,
                Foot::Dir => left + wm + gap + wn + gap + wd / 2.,
            };
            (x, y)
        }
    };
    Rect {
        x: x - w / 2.,
        y: y - h / 2.,
        w,
        h,
    }
}

/// 环底下那行状态的位置（`top`）。
pub(crate) fn status_y(m: &M) -> f32 {
    let n = foot_rect(m, Foot::Name);
    n.bottom() + 14. * m.scale
}

/// 一条连线：从圆心边上出发，指向卡片中心（终点会被卡片盖住，正好停在边上）。
pub(crate) fn card_wire(m: &M, slot: Slot) -> ((f32, f32), (f32, f32)) {
    let (cx, cy) = core_center(m);
    let r = core_r(m) + 10. * m.scale;
    let n = node_rect(m, slot);
    let (tx, ty) = (n.cx(), n.cy());
    ((cx, cy), (tx, ty)).into_wire(cx, cy, r, tx, ty)
}

/// 圆边 → 环上某个值的一条短线（终点藏在 pill 底下）。
pub(crate) fn foot_wire(m: &M, f: Foot) -> ((f32, f32), (f32, f32)) {
    let (cx, cy) = core_center(m);
    let r = core_r(m) + 4. * m.scale;
    let t = foot_rect(m, f);
    let (tx, ty) = (t.cx(), t.cy());
    ((cx, cy), (tx, ty)).into_wire(cx, cy, r, tx, ty)
}

/// 把「圆心 → 目标」裁成「圆边 → 目标」。
trait Wire {
    fn into_wire(self, cx: f32, cy: f32, r: f32, tx: f32, ty: f32) -> ((f32, f32), (f32, f32));
}

impl Wire for ((f32, f32), (f32, f32)) {
    fn into_wire(self, cx: f32, cy: f32, r: f32, tx: f32, ty: f32) -> ((f32, f32), (f32, f32)) {
        let (dx, dy) = (tx - cx, ty - cy);
        let len = (dx * dx + dy * dy).sqrt().max(1.);
        ((cx + dx / len * r, cy + dy / len * r), (tx, ty))
    }
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

/// 弹层各自的脾气 —— 宽度、贴哪边、要不要翻边，全在这里定。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopKind {
    /// 模型：贴卡片下方，宽 600。
    Model,
    /// 技能：左树 + 右列表，要 620 才站得下两栏。
    Skills,
    /// 模板这类单列：410。
    Narrow,
    /// 扩展：左树（状态 + 类别）+ 右列表，跟技能一样要 620。
    Ext,
    /// 目录：命令面板（左列表 + 右预览），600。
    Dir,
    /// 机器：460。
    Machine,
}

/// 模型弹层贴在卡片**下方**；技能 / 提示词贴在卡片**左侧**（右上角右边没地方）；
/// 机器 / 目录贴在 dock **上方**、居中。
pub(crate) fn pop_rect(m: &M, slot: Option<Slot>, kind: PopKind) -> Pop {
    let gap = 14.;
    // 竖排档没有四角卡、也没有环 —— `node_rect` / `foot_rect` 在这里是流式布局，
    // 拿它们的坐标去贴只会把弹层甩到格子外。这一档统一居中贴顶。
    if matches!(m.layout, Layout::Stack) {
        let w = (m.w - 24.).max(180.);
        return Pop {
            x: (m.w - w) / 2.,
            w,
            anchor: Anchor::Top(20.),
        };
    }
    match slot {
        Some(Slot::Model) if kind == PopKind::Model => {
            let n = node_rect(m, Slot::Model);
            let w = (m.w - 28.).min(600.);
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
            let w = match kind {
                PopKind::Skills | PopKind::Ext => (m.w - 28.).min(620.),
                _ => 410.,
            };
            // 优先放右侧；右边不够（右上角那张卡就是这样）就翻到左边。
            let x = if n.right() + gap + w <= m.w - 14. {
                n.right() + gap
            } else {
                (n.x - gap - w).max(14.)
            };
            // 竖向：先试「从卡片上沿往下长」；下半部分那两张卡（扩展在左下）
            // 下面根本不够，就翻到卡片上方、向上长。不然只剩 150px 高。
            let down = n.y;
            if down + 320. > m.h - 24. && n.y > 300. {
                Pop {
                    x,
                    w,
                    anchor: Anchor::Bottom(m.h - n.y + gap),
                }
            } else {
                Pop {
                    x,
                    w,
                    anchor: Anchor::Top(down),
                }
            }
        }
        None => {
            // 机器 / 目录：贴着被点的那颗 pill 弹（就地弹出的同一条规矩）。
            let f = if kind == PopKind::Dir {
                Foot::Dir
            } else {
                Foot::Machine
            };
            let t = foot_rect(m, f);
            // 目录弹层是命令面板（左列表 + 右预览），比机器列表要宽一格。
            let w = (m.w - 28.).min(if kind == PopKind::Dir { 600. } else { 460. });
            let x = (t.cx() - w / 2.).clamp(14., (m.w - w - 14.).max(14.));
            Pop {
                x,
                w,
                anchor: Anchor::Bottom(m.h - t.y + 10.),
            }
        }
    }
}
