//! 滚动条：**鼠标进内容区才浮出来的那根细黑条**（demo 里的 3 号，macOS 那种）。
//!
//! 这里只定「长什么样、什么时候出现」。滚动、拖拽、命中区域都是 `Scrollbar` 元素
//! 自己做的 —— 自己再画一根是重复造滑轮，而且拖起来一定比它差。
//!
//! 用法：父元素 `.relative()`（顺手也是滚动视口），滚动容器和 `overlay(..)`
//! 是同一个父元素下的兄弟。
//!
//! 三个踩过的坑：
//!
//! 1. **不要用 `.viewport_from_layout()`**。`Scrollbar` 的 `request_layout` 把自己
//!    设成 `absolute` + 100% x 100%，尺寸要等布局，而 `prepaint` 里算蒙版时就要用
//!    这个尺寸 —— 拿到 0 就整块被裁掉，表现是「一条线都不画」，看着像没生效。
//!    改成把滚动句柄的 `bounds()` 递进去（上一帧的真实像素），立刻就有了。
//! 2. 库里那个 `ScrollbarMode::Hover` 在我们这套布局里没反应（悬停状态是它自己
//!    在鼠标事件里更新的，等不到）。所以显不显示由外面决定，这里固定 `Always`。
//! 3. 元素留不留由 `shown` 决定：看不见的时候**必须**真的不渲染，因为透明的它
//!    会插一块铺满整页的 hitbox，把底下内容的悬停/点击全挡掉。

use std::time::Duration;

use gpui_kit::AnimationExt as _;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarAxis, ScrollbarMode};
use gpui_kit::*;
use smol::Timer;

/// 淡入淡出时长 —— 和 demo 里那 0.25 秒的 css transition 对齐。
pub const FADE: Duration = Duration::from_millis(250);

/// 一根滚动条的两态：鼠标在不在这个面板上 / 元素在不在场。
///
/// 一个滚动视口一份（`Desktop::bars` 里按 [`crate::app::Bar`] 索引）。
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct BarState {
    /// 元素在不在场（含淡出那 250ms）。
    pub shown: bool,
    /// 鼠标在不在这个面板上（决定淡入还是淡出）。
    pub hover: bool,
}

impl BarState {
    /// 悬停进 / 出 → 「滚动条在不在场」。
    ///
    /// 移开不能马上撤元素：撤了就看不到那 250ms 的淡出，会「啪」一下消失。
    /// 所以先留着，等淡出跑完再收 —— 期间要是又进来了，就当成淡入继续用。
    ///
    /// `get` 是「从实体里再取一次这根条」：状态可能属于 `Desktop`，也可能属于
    /// 自己持有一份状态的子面板（`config_panel::Panel`），所以不写死取法。
    pub fn hover<T: 'static>(
        &mut self,
        now_hovered: bool,
        cx: &mut Context<T>,
        get: impl Fn(&mut T) -> &mut BarState + Copy + 'static,
    ) {
        if self.hover == now_hovered {
            return;
        }
        self.hover = now_hovered;
        self.shown = true;
        cx.notify();
        if now_hovered {
            return;
        }
        cx.spawn(async move |this, cx| {
            Timer::after(FADE).await;
            this.update(cx, |this, cx| {
                let bar = get(this);
                if bar.hover || !bar.shown {
                    return;
                }
                bar.shown = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// 浮层滚动条。
///
/// `id` 必须一个面板一个：默认 id 取调用点的源码位置，而我们只有这一个调用点，
/// 几个面板会撞成同一个 —— 撞了的表现是「另一个面板的滚动条不动」。
///
/// * `shown`：元素在不在场（含淡出那 250ms）
/// * `hovered`：鼠标在不在这个面板上（决定淡入还是淡出）
pub fn overlay(id: &'static str, handle: &ScrollHandle, shown: bool, hovered: bool) -> AnyElement {
    overlay_tone(id, handle, shown, hovered, Tone::auto())
}

/// 滑块画在什么底子上。
///
/// 主题给的 `scrim` 是**半透明黑**：压在浅底上是根灰线，压在深底上（New Manual 的盘面
/// 一直是固定的深色 art 调色板）等于没画 —— 所以深底那块要换成半透明白。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    OnLight,
    OnDark,
}

impl Tone {
    /// 跟着当前皮走：深色皮配半透明白，浅色皮配半透明黑。
    /// （深色皮上画黑滑块等于没画 —— 主题的 `scrim` 是黑的。）
    pub fn auto() -> Self {
        if crate::theme::cur().dark {
            Tone::OnDark
        } else {
            Tone::OnLight
        }
    }
}

/// 同 [`overlay`]，但可以指定底子深浅。
pub fn overlay_tone(
    id: &'static str,
    handle: &ScrollHandle,
    shown: bool,
    hovered: bool,
    tone: Tone,
) -> AnyElement {
    if !shown {
        return div().into_any_element();
    }
    // 深底：半透明白；浅底：主题的 scrim（半透明黑）。
    let t = crate::theme::cur();
    let (idle, hot) = match tone {
        Tone::OnLight => (rgba(t.p.scrim), rgba(t.p.shadow)),
        Tone::OnDark => (
            hsla(0., 0., 1., 0.28).into(),
            hsla(0., 0., 1., 0.45).into(),
        ),
    };
    div()
        .absolute()
        .inset_0()
        // id 里带 in/out：动画只在 id 变化时重播，带方向才会「进来淡入、出去淡出」。
        .with_animation(
            ElementId::Name(format!("{id}-{}", if hovered { "in" } else { "out" }).into()),
            Animation::new(FADE),
            move |el, t| el.opacity(if hovered { t } else { 1. - t }),
        )
        .child(
            Scrollbar::new(handle)
                .id(id)
                .axis(ScrollbarAxis::Vertical)
                .mode(ScrollbarMode::Always)
                .styles(move |s| {
                    let t = crate::theme::cur();
                    s
                        // 没有轨道。macOS 那根就是这样：只有滑块，没有一条槽。
                        .track(|t| t.bg(hsla(0., 0., 0., 0.)))
                        .track_hover(|t| t.bg(hsla(0., 0., 0., 0.)))
                        .thumb(|el| {
                            el.bg(idle)
                                .width(t.d(7.))
                                .inset(t.d(3.))
                                .radius(t.r(3.5))
                                // 内容再长也留 28px 抓得住，不然会话一多滑块细成一根线点不着。
                                .min_length(t.d(28.))
                        })
                        .thumb_hover(|el| el.bg(hot))
                        .thumb_active(|el| el.bg(hot))
                })
                .viewport_bounds(handle.bounds()),
        )
        .into_any_element()
}
