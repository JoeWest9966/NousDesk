//! 「新对话」盘面的美术件：颜色、尺寸、那张图。
//!
//! 外观照抄 qrd3wh 的面板页（`/p/<board>/hb7k3j/`，Pele / Flexoki dark）。
//! 这里只放**能被抄错**的东西：色值、vmin 尺寸换算、Cynefin 那几条路径。
//!
//! 两条纪律：
//! - 颜色只用下面这些常量，别在渲染代码里现编色值 —— 那个页面整套色是一张调色板，
//!   漏一个就会「看着像但不是」。
//! - 尺寸一律先换算成 vmin 再落到 px：那个页面是**一屏不滚、随窗口缩**的，
//!   写死 px 在 1280 和 2560 上会一个挤一个散。

use gpui_kit::*;

// ── Flexoki dark（和页面 :root 一一对应）──────────────────────────────
pub(crate) const BG: u32 = 0x100f0f;
pub(crate) const PANEL: u32 = 0x1c1b1a;
pub(crate) const SURFACE: u32 = 0x282726;
pub(crate) const SURFACE_ALT: u32 = 0x343331;
pub(crate) const BORDER: u32 = 0x403e3c;
/// cliff 那条实线，也是正文色。
pub(crate) const LINE: u32 = 0xcecdc3;
pub(crate) const FG: u32 = 0xcecdc3;
pub(crate) const MUTED: u32 = 0x878580;
pub(crate) const FAINT: u32 = 0x575653;
pub(crate) const ACCENT: u32 = 0x4385be;
pub(crate) const S1: u32 = 0xd14d41;
pub(crate) const S2: u32 = 0xda702c;
pub(crate) const S3: u32 = 0xd0a215;
pub(crate) const S4: u32 = 0x879a39;
pub(crate) const S5: u32 = 0x3aa99f;
/// 象限色是 14% 叠在 panel 上的，不是实色。
pub(crate) const QUADRANT_ALPHA: f32 = 0.14;
/// 中央椭圆的底色（青，18%）。
pub(crate) const HUB_ALPHA: f32 = 0.18;

/// 等宽字。Windows 上 `Consolas` 一定有；`Cascadia Code` 不一定装了，别写在前面。
pub(crate) const MONO: &str = "Consolas";

/// 尺寸换算。`vmin` = 窗口短边（CSS 的 `1vmin` 就是这个的 1%）。
#[derive(Clone, Copy)]
pub(crate) struct M {
    pub vmin: f32,
    /// 窗口宽高。那个页面里 `32%` 这种是**相对盘面**的，不是相对 vmin ——
    /// 中央椭圆就吃这个，用 vmin 算会变成一个圆。
    pub w: f32,
    pub h: f32,
}

impl M {
    pub fn new(size: Size<Pixels>) -> Self {
        let w = f32::from(size.width);
        let h = f32::from(size.height);
        Self {
            vmin: w.min(h).max(1.),
            w,
            h,
        }
    }

    /// `12vmin` 这种写法：单位是 vmin。
    pub fn v(&self, vmin: f32) -> Pixels {
        px(vmin * self.vmin / 100.)
    }

    /// `clamp(lo, kvmin, hi)`。那个页面所有字号都长这样。
    pub fn f(&self, vmin: f32, lo: f32, hi: f32) -> Pixels {
        px((vmin * self.vmin / 100.).clamp(lo, hi))
    }
}

// ── 那张图（原样抄自 hb7k3j 的 <svg class="frame">）─────────────────────
//
// viewBox 是 `0 0 640 440`，且 `preserveAspectRatio="none"` —— 也就是**直接拉伸**：
// x 乘 w/640，y 乘 h/440，不做等比缩放。所以这里的四块色和三条虚线在窄窗口里会变形，
// 那是原设计本来的行为，不是 bug。

pub(crate) const VB_W: f32 = 640.;
pub(crate) const VB_H: f32 = 440.;

/// 左上：Model
const D_R1: &str = "M0,0L314.04,0C315.06,27.5,322.13,27.5,323.15,55C315.14,82.5,335.41,82.5,327.39,110C328.99,137.5,322.51,137.5,324.11,165C319.92,192.5,324.19,192.5,320,220C280,225.07,280,210.71,240,215.78C200,204.86,200,227.13,160,216.22C120,223.83,120,218.6,80,226.21C40,222.27,40,221.52,0,217.58Z";
/// 右上：Context
const D_R2: &str = "M314.04,0L640,0L640,222.23C600,231.87,600,212.95,560,222.59C520,213.69,520,227.54,480,218.63C440,224.66,440,213.77,400,219.8C360,218.1,360,221.69,320,220C324.19,192.5,319.92,192.5,324.11,165C322.51,137.5,328.99,137.5,327.39,110C335.41,82.5,315.14,82.5,323.15,55C322.13,27.5,315.06,27.5,314.04,0Z";
/// 左下：Extensions
const D_R4: &str = "M0,217.58C40,221.52,40,222.27,80,226.21C120,218.6,120,223.83,160,216.22C200,227.13,200,204.86,240,215.78C280,210.71,280,225.07,320,220C339.2,264,291.2,341,329.6,385C300.8,407,325.76,429,320,440L0,440Z";
/// 右下：Permissions
const D_R3: &str = "M320,220C360,221.69,360,218.1,400,219.8C440,213.77,440,224.66,480,218.63C520,227.54,520,213.69,560,222.59C600,212.95,600,231.87,640,222.23L640,440L320,440C325.76,429,300.8,407,329.6,385C291.2,341,339.2,264,320,220Z";

/// 中间那条竖虚线（上）
const D_B1: &str = "M314.04,0C315.06,27.5,322.13,27.5,323.15,55C315.14,82.5,335.41,82.5,327.39,110C328.99,137.5,322.51,137.5,324.11,165C319.92,192.5,324.19,192.5,320,220";
/// 中间那条横虚线（左）
const D_B2: &str = "M0,217.58C40,221.52,40,222.27,80,226.21C120,218.6,120,223.83,160,216.22C200,227.13,200,204.86,240,215.78C280,210.71,280,225.07,320,220";
/// 中间那条横虚线（右）
const D_B3: &str = "M320,220C360,221.69,360,218.1,400,219.8C440,213.77,440,224.66,480,218.63C520,227.54,520,213.69,560,222.59C600,212.95,600,231.87,640,222.23";
/// 那条崖（2px 实线）：图上唯一一条「跨不过去」的线。
const D_CLIFF: &str = "M320,220C339.2,264,291.2,341,329.6,385C300.8,407,325.76,429,320,440";

/// 一条要画的东西：路径 + 颜色。
pub(crate) struct Shape {
    pub path: Path<Pixels>,
    pub color: Hsla,
}

/// 把那张图按当前尺寸摊成一串 `Shape`。
///
/// 画在 `canvas` 的 paint 里（`window.paint_path`）—— 那个页面的图本来就是矢量，
/// 用 `svg()` 元素反而画不了：它只走 alpha 蒙版，一个颜色，四块色会糊成一块。
pub(crate) fn shapes(bounds: Bounds<Pixels>) -> Vec<Shape> {
    let sx = f32::from(bounds.size.width) / VB_W;
    let sy = f32::from(bounds.size.height) / VB_H;
    let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let mut out = Vec::with_capacity(8);

    for (d, color) in [(D_R1, S1), (D_R2, S2), (D_R3, S3), (D_R4, S4)] {
        if let Some(path) = parse(d, sx, sy, ox, oy, PathBuilder::fill()) {
            out.push(Shape {
                path,
                color: rgb(color).opacity(QUADRANT_ALPHA).into(),
            });
        }
    }
    for d in [D_B1, D_B2, D_B3] {
        // 虚线是 6 3、1px，`vector-effect: non-scaling-stroke` 等价于「线宽不跟着拉伸」。
        let b = PathBuilder::stroke(px(1.)).dash_array(&[px(6.), px(3.)]);
        if let Some(path) = parse(d, sx, sy, ox, oy, b) {
            out.push(Shape {
                path,
                color: rgb(BORDER).into(),
            });
        }
    }
    if let Some(path) = parse(D_CLIFF, sx, sy, ox, oy, PathBuilder::stroke(px(2.))) {
        out.push(Shape {
            path,
            color: rgb(LINE).into(),
        });
    }
    out
}

/// 把一条 SVG `d`（只认绝对 `M/L/C/Z`）摊成 `PathBuilder` 的调用。
///
/// 那个页面的路径全是 `M x,y` / `L x,y` / `C c1x,c1y c2x,c2y x,y` / `Z`，
/// 没有相对坐标、没有弧、没有简写 —— 所以这里不引 SVG 解析库，手写一个够用的。
fn parse(d: &str, sx: f32, sy: f32, ox: f32, oy: f32, style: PathBuilder) -> Option<Path<Pixels>> {
    // 按命令字母切段，每段里按 `,` / 空白切数字。
    let mut segments: Vec<(char, Vec<f64>)> = Vec::new();
    let mut cur_cmd = 'M';
    let mut cur = String::new();
    let flush = |cmd: char, s: &str, out: &mut Vec<(char, Vec<f64>)>| {
        let nums: Vec<f64> = s
            .split([',', ' '])
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();
        if !nums.is_empty() {
            out.push((cmd, nums));
        }
    };
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            flush(cur_cmd, &cur, &mut segments);
            cur.clear();
            cur_cmd = ch;
        } else {
            cur.push(ch);
        }
    }
    flush(cur_cmd, &cur, &mut segments);

    let (sx, sy) = (sx as f64, sy as f64);
    let (ox, oy) = (ox as f64, oy as f64);
    let mut b = style;
    for (cmd, mut nums) in segments {
        let p = |n: &mut Vec<f64>| -> Point<Pixels> {
            let x = if n.is_empty() { 0. } else { n.remove(0) };
            let y = if n.is_empty() { 0. } else { n.remove(0) };
            point(px((x * sx + ox) as f32), px((y * sy + oy) as f32))
        };
        match cmd {
            // 一条 `d` 里可能有多个坐标对（`L 1,2 3,4` 等价于两次 line_to），逐个吃完。
            'M' => {
                while nums.len() >= 2 {
                    b.move_to(p(&mut nums));
                }
            }
            'L' => {
                while nums.len() >= 2 {
                    b.line_to(p(&mut nums));
                }
            }
            'C' => {
                while nums.len() >= 6 {
                    let c1 = p(&mut nums);
                    let c2 = p(&mut nums);
                    let to = p(&mut nums);
                    b.cubic_bezier_to(to, c1, c2);
                }
            }
            'Z' => b.close(),
            _ => {}
        }
    }
    b.build().ok()
}
