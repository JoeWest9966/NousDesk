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
    crate::app::Tab::Resources => t.rail_w + 214.,
    // 数据监控室自己画满三列，这一份只用来算盘面的宽度基准（不画盘面）。
    crate::app::Tab::Data => t.rail_w + 214.,
    // 元素库：图标条 + 左清单（262），右栏自己撑满。
    crate::app::Tab::Elements => t.rail_w + 262.,
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
        Self {
            w,
            h,
            layout,
            scale,
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

    /// 竖轴清单（方案 D）的设计值 → px。
    ///
    /// D 是全流式布局（`flex_1` 平分、没有绝对坐标），所以它不需要 `ox` / `oy`，
    /// 也不需要 `layout` 那三档断点 —— 只需要**一个比例**：盘面宽 / 644（那份
    /// demo 的设计基准）。窄到 0.88 就不再缩，免得中文挤成一列；宽到 1.35 封顶，
    /// 900 宽时字不会跟着摊成一片。
    pub fn k(&self, design: f32) -> Pixels {
        px(design * (self.w / 644.).clamp(0.88, 1.35))
    }
}

// ── 四张卡 / 中央圆 / dock 的几何 ────────────────────────────────────

/// 四个角上那张卡。位置和「轨道」demo 一一对应。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Model,
    Context,
    Ext,
    Perms,
}

// ── 05 环：圆外一圈虚线，三个值挂在环上 ────────────────────────────────
//
// 为什么不留在底部那条 dock：一条 720px 横贯盘面的窄条，读起来是「工具栏」，
// 和上面四张卡不是一个语法。环把名字 / 机器 / 目录收回圆边上 —— 它们是**从圆心
// 长出来的参数**，所以用同一种画法（节点 + 虚线），也所以三个 pill 的底色
// 和那颗圆一模一样（见 `solid()`）。

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
    /// 会话页的「带什么」清单：居中，不贴任何节点（会话页没有四角卡）。
    Session,
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
    let k = (m.w / 644.).clamp(0.88, 1.35);
    let w = match kind {
        PopKind::Skills | PopKind::Ext => (m.w - 56. * k).min(620.),
        PopKind::Model => (m.w - 56. * k).min(560.),
        _ => (m.w * 0.76).min(460.),
    };
    let x = (m.w - w) / 2.;
    // 挂行的弹层：从那一行的下沿往下长，看得见自己点的是哪一行。
    // 没挂行的（会话页 / 目录 / 机器）：贴画面上方，不压住下半截。
    let (anchor_y, from) = match slot {
        Some(s) => (axis_center(m, s) + 34. * k, axis_center(m, s)),
        None => ((m.h * 0.26).max(60.), m.h * 0.26),
    };
    if anchor_y + 320. > m.h - 24. {
        Pop {
            x,
            w,
            anchor: Anchor::Bottom((m.h - from + 34. * k).max(40.)),
        }
    } else {
        Pop {
            x,
            w,
            anchor: Anchor::Top(anchor_y),
        }
    }
}

/// 竖轴清单（D）：某一行的**中心 y**（相对右格顶部，标题栏已扣掉）。
///
/// 这组数字和 `board::board_axis` 里的 `pt(30) / 标题 / mt(10)` 与 foot
/// （`pt(16) + name + chips + status + 54 按钮 + pb(26)`）是同一套。弹层要贴着
/// 被点的那一行弹出来，两边各算各的就会错位 —— 所以只在这里算一次。
pub(crate) fn axis_center(m: &M, slot: Slot) -> f32 {
    let k = (m.w / 644.).clamp(0.88, 1.35);
    let top = 70. * k;
    let foot = 216. * k;
    let axis = (m.h - top - foot).max(160.);
    let i = match slot {
        Slot::Model => 0.,
        Slot::Ext => 1.,
        Slot::Context => 2.,
        Slot::Perms => 3.,
    };
    top + axis * (i + 0.5) / 4.
}


#[cfg(test)]
mod tests {
    // 显式列名，别 `use super::*`：文件顶的 `use gpui_kit::*` 会把 gpui 的
    // `test` 宏也带进来，`#[test]` 就递归展开（坑.md 记过这条）。
    use super::{axis_center, Slot, M};
    use crate::app::Tab;
    use gpui_kit::{px, Size};

    fn m(win_w: f32, win_h: f32) -> M {
        M::new(Size::new(px(win_w), px(win_h)), Tab::Chats)
    }

    #[test]
    fn 竖轴四行中心从上到下单调() {
        let m = m(980., 1168.);
        let ys: Vec<f32> = [Slot::Model, Slot::Ext, Slot::Context, Slot::Perms]
            .iter()
            .map(|s| axis_center(&m, *s))
            .collect();
        assert!(ys[0] < ys[1] && ys[1] < ys[2] && ys[2] < ys[3], "{ys:?}");
        // 都落在盘面里面（不是负数、也没出底）。
        assert!(ys[0] > 40., "{ys:?}");
        assert!(ys[3] < m.h, "{ys:?}");
    }

    #[test]
    fn 竖轴比例夹在_088_到_135_之间() {
        // 右格 = 窗口 - (rail 56 + list 280) —— 用盘面自己的宽算比例。
        let narrow = m(900., 1100.);
        let wide = m(1600., 1000.);
        let kn = f32::from(narrow.k(100.)) / 100.;
        let kw = f32::from(wide.k(100.)) / 100.;
        assert!((kn - 0.88).abs() < 0.001, "kn={kn}");
        assert!((kw - 1.35).abs() < 0.001, "kw={kw}");
    }
}
