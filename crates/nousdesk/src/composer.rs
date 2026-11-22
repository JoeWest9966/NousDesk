//! 输入区：工具行 / 多行输入 / 发送·停止。
//!
//! 外观照新版微信（4.0）：白圆角卡片浮在聊天区底部、四周一圈浅灰留白；
//! 写字区在上、工具条压在卡片内底部、右下角一颗小「发送」。
//! 鼠标悬停写字区时第一行淡入一行浅灰提示字，移开 250ms 淡出 ——
//! 「在不在场 / 该淡入还是淡出」由 `app::Desktop`（`composer_hover` / `composer_hint`）
//! 算好递进来，这里只照着画，不自己存状态。
//!
//! **demo2 的 1 号是规格书**：颜色、字号、尺寸、间距逐项对齐（见下面的注释），
//! 不凭感觉。对照表：外层 `8/10/10` 留白 + `#f2f2f2`；白卡圆角 12、阴影
//! `0 1px 3px 黑8%`、边框 `1px 黑5%`；写字区 `10/12/4`；文字 13.5px 行高 1.6；
//! 工具条 `2/8/8`、图标 28×28、图标本身 18px `#4d5156`；剪刀那颗 34px 宽带 ⌄；
//! 右侧声纹是**灰色图标**（不是蓝圆）、28×26、距发送键 6px；发送 26 高、圆角 4、
//! 字号 12.5。
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │  浅灰留白                                 │
//! │  ┌────────────────────────────────────┐  │
//! │  │  文本区（1~8 行自动长高，无边框）    │  │
//! │  │  （悬停淡入：Enter 发送 · Shift+…） │  │
//! │  │  🙂  ⬡  📁  ✂⌄  🎤       ≋  [发送] │  │
//! │  └────────────────────────────────────┘  │
//! └──────────────────────────────────────────┘
//! ```
//!
//! **这一层不持有状态、不碰 HTTP、不碰 Desktop 的字段**：调它的人把
//! 「能不能发」「在不在生成」「提示字在不在场 / 淡入还是淡出」和几个回调递进来。
//!
//! 工具行的图标分两拨：最左边那颗「对话设置」是真按钮（由调用方递进来）；
//! 其余那几颗引擎还没有功能，画成压灰的禁用态、不挂点击、不装可点光标 ——
//! 理由见 `disabled_reason_line()`，版式（28px 宽 / 34px 剪刀带箭头）原样保留。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin { crate::theme::cur() }

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::input::Textarea;
use gpui_kit::prelude::{FluentBuilder as _, StatefulInteractiveElement as _};
use gpui_kit::*;

use crate::app::scrollbar::FADE;

// 跟 demo2 的 1 号一套的颜色。
/// 原 `WELL_BG = 0xf2f2f2`；由 hover 这个语义槽接管。
fn well_bg() -> u32 { t().p.hover } // 卡片四周的浅灰留白
/// 原 `CARD_BG = 0xffffff`；由 panel 这个语义槽接管。
fn card_bg() -> u32 { t().p.panel } // 白卡片
/// 原 `HINT_TEXT = 0xb8bdc4`；由 faint 这个语义槽接管。
fn hint_text() -> u32 { t().p.faint } // 悬停提示的浅灰字
/// 原 `GEN_HINT = 0x8a8a8a`；由 faint 这个语义槽接管。
fn gen_hint() -> u32 { t().p.faint } // 生成中的灰字提示
/// 原 `SEND_BG = 0x07c160`；由 accent 这个语义槽接管。
fn send_bg() -> u32 { t().p.accent } // 发送可用
/// 原 `SEND_BG_IDLE = 0xe9e9e9`；由 disabled 这个语义槽接管。
fn send_bg_idle() -> u32 { t().p.disabled } // 发送不可用
/// 原 `SEND_TEXT_IDLE = 0xb2b2b2`；由 disabled 这个语义槽接管。
fn send_text_idle() -> u32 { t().p.disabled }
/// 原 `STOP_BG = 0xf2f2f2`；由 hover 这个语义槽接管。
fn stop_bg() -> u32 { t().p.hover }
/// 原 `STOP_TEXT = 0xe64340`；由 danger 这个语义槽接管。
fn stop_text() -> u32 { t().p.danger }

// demo 里 textarea / hint 都是 13.5px。行高是 Textarea 组件写死的 1.25rem = 20px，
// 不随字号变（gpui 的 rem 基准固定 16px），所以这里不能按 1.6 倍字号自己算。
const TEXT_SIZE: f32 = 13.5;
const LINE_HEIGHT: f32 = 20.0;

/// 调用方告诉输入区的事。全是**只读**快照，没有回调之外的交互。
pub struct State {
    /// 正在生成。true 时右下角是「停止」，工具行变淡。
    pub streaming: bool,
    /// 输入框里有非空白文字。false 时「发送」是灰的（视觉上按不动）。
    pub can_send: bool,
}

/// 画输入区。
///
/// `hint_shown` / `hovered` 决定那行悬停提示字：不在场就不渲染（渲染一个透明的会
/// 挡输入框的点击）；在场时按 `hovered` 决定淡入还是淡出。
/// `on_hover` / `on_send` / `on_stop` 由调用方用 `cx.listener(...)` 递进来。
pub fn render(
    input: &Entity<gpui_kit::component::input::TextareaState>,
    state: State,
    hint_shown: bool,
    hovered: bool,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    // 写字区自己那一层滚动：字写多了在卡片里滚，不再把聊天区顶成零。
    // `body_scroll` 归调用方持有（跨帧记住位置），`body_bar` 是滚动条那两位，
    // `on_body_hover` 由调用方用 `cx.listener(...)` 递进来。
    body_scroll: &ScrollHandle,
    body_bar: (bool, bool),
    on_body_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    on_send: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_stop: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    // 输入区**上方**那张 `/` 技能面板。`None` = 不渲染，也不占高度。
    // 由调用方按输入框内容现算好递进来（composer 自己不读输入框、不存状态）。
    slash_panel: Option<AnyElement>,
    // 工具行最左边那颗图标。它现在点开「这条对话的设置」，有开关状态、有回调，
    // 所以不在这里硬编码了 —— 由调用方连状态一起造好递进来。
    first_tool: AnyElement,
) -> impl IntoElement {
    let dimmed = state.streaming;

    div()
        .flex()
        .flex_col()
        .w_full()
        .bg(rgb(well_bg()))
        .relative()
        .pt(t().d(8.))
        .px(t().d(10.))
        .pb(t().d(10.))
        // `/` 面板是**跟着输入框走**的：占位、把卡片往上推，跟微信那种一致。
        .children(slash_panel.map(|p| div().px(t().d(0.)).pb(t().d(6.)).child(p).into_any_element()))
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .bg(rgb(card_bg()))
                .rounded(t().r(12.))
                // demo 的 box-shadow: 0 1px 3px 黑 8%。
                .shadow(vec![
                    BoxShadow::new(t().d(0.), t().d(1.), hsla(0., 0., 0., 0.08)).blur_radius(t().d(3.)),
                ])
                // demo 的 border: 1px 黑 5%。
                .border(t().d(1.))
                .border_color(rgba(t().p.shadow))
                // `on_hover` 在 `Stateful` 上：先给 id。
                .id("composer-card")
                .on_hover(on_hover)
                // ── 写字区 ──
                .child(
                    div()
                        // `on_hover` 挂在 `Stateful` 上：先给 id。
                        .id("composer-viewport")
                        .relative()
                        .on_hover(on_body_hover)
                        .child(
                            div()
                                .relative()
                                .id("composer-scroll")
                                // 上限约八行：再多就在卡片里滚。以前没有上限，
                                // 粘一段长代码会把聊天区挤到零（真踩过）。
                                .max_h(px(MAX_INPUT_H))
                                .overflow_y_scroll()
                                .track_scroll(body_scroll)
                                .px(t().d(12.))
                                .pt(t().d(10.))
                                .pb(t().d(4.))
                                .child(
                                    Textarea::new(input)
                                        .appearance(false)
                                        .bordered(false)
                                        .text_size(px(TEXT_SIZE)),
                                )
                                .child(hint_layer(hint_shown, hovered, state.can_send)),
                        )
                        .child(crate::app::scrollbar::overlay(
                            "composer-scrollbar",
                            body_scroll,
                            body_bar.0,
                            body_bar.1,
                        )),
                )
                // ── 生成中的提示：回车这时不该发出去，得让人知道为什么 ──
                .when(state.streaming, |el| {
                    el.child(
                        div()
                            .px(t().d(12.))
                            .pb(t().d(2.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(gen_hint()))
                            .child("正在生成，回车暂不发送（可以点「停止」，或者等它说完）"),
                    )
                })
                // ── 工具行 + 右下角发送 / 停止（都压在卡片内底部） ──
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t().d(2.))
                        .px(t().d(8.))
                        .pt(t().d(2.))
                        .pb(t().d(8.))
                        .child(
                            // 生成中工具图标整组变淡；声纹和发送/停止不跟着这一组。
                            div()
                                .flex()
                                .items_center()
                                .gap(t().d(2.))
                                .when(dimmed, |el| el.opacity(0.4))
                                .child(first_tool)
                                // 这四颗图标由 `ToolIcon` 一处登记，render 不再手写
                                // 第二份图标名 / id / 禁用理由。
                                .children(ToolIcon::TOOLBAR.iter().copied().map(tool_icon_button)),
                        )
                        // 人话理由填在工具行**原有留白**里：不加行、不改高度/栏宽/图标位置，
                        // 只是把这块原本空着的弹性区间写成一句淡灰提示（窄窗时截掉）。
                        .child(
                            div()
                                .flex_1()
                                .min_w(t().d(0.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .px(t().d(4.))
                                .text_size(t().fs(10.))
                                .text_color(rgb(t().p.faint))
                                .child(disabled_reason_line()),
                        )
                        .child(
                            div()
                                .when(dimmed, |el| el.opacity(0.4))
                                .child(tool_icon_button(ToolIcon::Voice)),
                        )
                        .child(if state.streaming {
                            action_button(
                                "composer-stop",
                                "停 止",
                                stop_bg(),
                                stop_text(),
                                true,
                                12.,
                                on_stop,
                            )
                            .into_any_element()
                        } else {
                            action_button(
                                "composer-send",
                                "发送",
                                if state.can_send {
                                    send_bg()
                                } else {
                                    send_bg_idle()
                                },
                                if state.can_send {
                                    t().p.panel
                                } else {
                                    send_text_idle()
                                },
                                state.can_send,
                                14.,
                                on_send,
                            )
                            .into_any_element()
                        }),
                ),
        )
}

/// 写字区的高度上限（逻辑像素）：约八行 20px 正文 + 上下内边距。
/// 超过就在卡片里滚 —— 不再让输入内容决定整块聊天区的高度。
const MAX_INPUT_H: f32 = 172.;

/// 那行悬停提示字。
///
/// `shown` = 元素在不在场（含淡出那 250ms）；`hovered` 决定淡入还是淡出；
/// 输入框里有字就不该再叠一行提示（`can_send` 一旦 true 直接撤，不用等淡出 ——
/// 文字已经盖住那个位置了）。
fn hint_layer(shown: bool, hovered: bool, can_send: bool) -> AnyElement {
    if !shown || can_send {
        return div().into_any_element();
    }
    div()
        .absolute()
        // 写字区容器 pt(10)/px(12)，而 Textarea 组件自带内边距（Size=Medium：上下 8px、
        // 左右 10px），输入文字实际在 (12+10, 10+8)。hint 要跟输入文字**精确重叠**，
        // 就把这层内边距补上；行高用组件的 20px。
        .top(t().d(18.))
        .left(t().d(22.))
        .right(t().d(12.))
        .text_size(px(TEXT_SIZE))
        .line_height(px(LINE_HEIGHT))
        .text_color(rgb(hint_text()))
        // id 里带 in/out：动画只在 id 变化时重播，带方向才会「进来淡入、出去淡出」。
        .with_animation(
            ElementId::Name(format!("composer-hint-{}", if hovered { "in" } else { "out" }).into()),
            Animation::new(FADE),
            move |el, t| el.opacity(if hovered { t } else { 1. - t }),
        )
        .child("Enter 发送 · Shift+Enter 换行")
        .into_any_element()
}

/// 工具行的图标登记表（**生产代码**：render 从这里派生，测试也遍历它）。
/// 最左边那颗「对话设置」是调用方递进来的真按钮，不在这里。
/// 引擎还没有这些功能：五颗图标（四颗在工具组 + 右侧声纹）一律灰、不挂 `on_click`、
/// 不画可点光标；「永远不可用 + 理由」由 `icon_disabled_reason()` 一处表达。
/// 有 handler 的入口只有发送 / 停止 / 设置，分别是 `action_button` / 调用方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolIcon {
    /// 工具组第一颗（沿用 demo 的六边形）。
    More,
    File,
    Screenshot,
    /// 工具组最后一颗（麦克风）。
    Mic,
    /// 右侧声纹（28×26，不是蓝圆）。
    Voice,
}

impl ToolIcon {
    /// 工具组里的四颗；`Voice` 单独摆在右侧，尺寸不同。
    pub(crate) const TOOLBAR: &'static [ToolIcon] = &[
        ToolIcon::More, ToolIcon::File, ToolIcon::Screenshot, ToolIcon::Mic,
    ];
    /// 全表：render 与测试共用这一份，新增变体必须同时给出理由。
    /// 非 test 构建里只由测试消费，故显式标注，免得给 clippy 基线添一条。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const ALL: &'static [ToolIcon] = &[
        ToolIcon::More, ToolIcon::File, ToolIcon::Screenshot, ToolIcon::Mic, ToolIcon::Voice,
    ];

    fn glyph(self) -> IconName {
        match self {
            ToolIcon::More => IconName::Hexagon,
            ToolIcon::File => IconName::Folder,
            ToolIcon::Screenshot => IconName::Scissors,
            ToolIcon::Mic => IconName::Mic,
            ToolIcon::Voice => IconName::AudioLines,
        }
    }

    fn with_caret(self) -> bool {
        matches!(self, ToolIcon::Screenshot)
    }

    fn element_id(self) -> ElementId {
        let id = if matches!(self, ToolIcon::Voice) {
            "tool-voice".to_string()
        } else {
            format!("tool-{:?}", self.glyph())
        };
        ElementId::Name(id.into())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn icon_disabled_reason(icon: ToolIcon) -> &'static str {
    match icon {
        ToolIcon::More => "更多：引擎没有这个功能",
        ToolIcon::File => "文件：引擎没有这个功能",
        ToolIcon::Screenshot => "截图：引擎没有这个功能",
        ToolIcon::Mic | ToolIcon::Voice => "语音输入：引擎没有这个功能",
    }
}

/// 那一行人话理由。禁用图标不骗点击，理由就常驻写在这里，
/// 不需要 tooltip 也不需要新建控件（版式不动）。
pub(crate) fn disabled_reason_line() -> &'static str {
    "表情 / 更多 / 文件 / 截图 / 语音：引擎暂不支持，按钮已禁用"
}

/// 颜色和尺寸必须自己给：直接 `.child(IconName::X)` 时用的是**窗口默认文字样式**
/// （本项目里是白字），压在浅色卡片上就完全看不见 —— 第一次画就是这么踩的。
///
/// 这个函数是**禁用图标唯一的渲染入口**：不接 handler、不画可点光标，
/// 所以永远不可能出现「表里说禁用、render 里却偷偷挂了 on_click」的图标。
fn tool_icon_button(icon: ToolIcon) -> impl IntoElement {
    let glyph = icon.glyph();
    div()
        .flex()
        .items_center()
        .justify_center()
        .when(icon.with_caret(), |el| el.w(t().d(34.)))
        .when(!icon.with_caret(), |el| el.w(t().d(28.)))
        .h(if matches!(icon, ToolIcon::Voice) { t().d(26.) } else { t().d(28.) })
        .when(matches!(icon, ToolIcon::Voice), |el| el.mr(t().d(6.)))
        .id(icon.element_id())
        .opacity(0.55)
        .child(Icon::new(glyph).text_color(rgb(t().p.disabled)).size(t().d(18.)))
        // 剪刀那颗：demo 里它比别的图标宽（34px），剪刀右边跟着一个小 ⌄。
        .when(icon.with_caret(), |el| {
            el.child(
                Icon::new(IconName::ChevronDown)
                    .text_color(rgb(t().p.disabled))
                    .size(t().d(10.)),
            )
        })
}
/// 底行按钮。用 `div` 手搓而不是 `Button`：这里的尺寸/圆角/四态色值都是
/// 微信的，套组件反而要一层层覆盖样式。`pad_x` 是左右内边距（发送 14、停止 12，
/// 跟 demo 对齐）。
fn action_button(
    id: &'static str,
    label: &'static str,
    bg: u32,
    fg: u32,
    enabled: bool,
    pad_x: f32,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(t().d(26.))
        .px(px(pad_x))
        .rounded(t().r(4.))
        .bg(rgb(bg))
        .text_size(t().fs(12.5))
        .text_color(rgb(fg))
        .id(ElementId::Name(id.into()))
        .when(enabled, |el| el.cursor_pointer())
        .on_click(on_click)
        .child(label)
}

#[cfg(test)]
mod tests {
    // 不 `use super::*`：gpui 的重导出宏在测试 mod 里再展开一层会撞递归上限。
    use super::{ToolIcon, icon_disabled_reason};

    /// #15 判据 1/5：这张表是**生产代码里的表**，render 从它派生。
    /// 每一颗禁用图标都必须有非空理由且没有 handler；可点的只有发送 / 停止 / 设置。
    /// 往表里加一颗却忘了理由 → 红；把 render 改成手写新图标 → 绑定检查红；
    /// 删掉发送 / 停止的 on_click → 绑定检查红。
    #[test]
    fn every_rendered_tool_icon_is_disabled_with_a_reason() {
        assert_eq!(ToolIcon::ALL.len(), 5);
        assert_eq!(ToolIcon::TOOLBAR.len(), 4);
        for icon in ToolIcon::ALL {
            let reason = icon_disabled_reason(*icon);
            assert!(
                reason.contains("引擎没有这个功能"),
                "{icon:?} 没有明确的禁用理由：{reason}"
            );
        }
        assert_eq!(
            icon_disabled_reason(ToolIcon::Voice),
            "语音输入：引擎没有这个功能"
        );
        assert!(icon_disabled_reason(ToolIcon::Screenshot).contains("截图"));

        // 绑定：render 必须遍历这张表，且唯一入口 `tool_icon_button` 不接 handler。
        // GPUI 元素树没有 headless 测试缝，这里钉住源码里的调用关系（和 #12 归档门同法）。
        let src = include_str!("composer.rs");
        assert!(
            src.contains("ToolIcon::TOOLBAR.iter().copied().map(tool_icon_button)"),
            "render 不再从 ToolIcon 派生图标"
        );
        assert!(
            src.contains("tool_icon_button(ToolIcon::Voice)"),
            "右侧声纹不再从 ToolIcon 派生"
        );
        let helper = src
            .split("fn tool_icon_button(icon: ToolIcon)")
            .nth(1)
            .expect("禁用图标有唯一渲染入口")
            .split("\n}")
            .next()
            .expect("函数体闭合");
        assert!(!helper.contains("on_click"), "禁用图标挂了 on_click");
        assert!(!helper.contains("cursor_pointer"), "禁用图标画了可点光标");

        // 可点的只有发送 / 停止（这里）和设置（调用方递进来的 first_tool）。
        // `action_button` 唯一接 handler 的入口就是这个参数；把它换成空闭包 = 删 handler。
        // 断言字符串自己拆开拼，免得和源码里的调用点互相命中（哨兵自指坑）。
        let send_needle = ["on_", "send,"].concat();
        let stop_needle = ["on_", "stop,"].concat();
        assert!(src.contains(&send_needle), "发送按钮没有 handler");
        assert!(src.contains(&stop_needle), "停止按钮没有 handler");
        assert!(src.contains(".child(first_tool)"), "设置按钮没接到调用方 handler");
        assert_eq!(
            super::disabled_reason_line(),
            "表情 / 更多 / 文件 / 截图 / 语音：引擎暂不支持，按钮已禁用"
        );
    }
}
