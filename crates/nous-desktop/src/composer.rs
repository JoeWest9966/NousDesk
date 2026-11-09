//! 输入区：工具行 / 多行输入 / 发送·停止。
//!
//! 微信 PC 的切法就是三层（见 `坑.md` 之外的设计笔记）：
//!
//! ```text
//! +--------------------------------------------------+
//! |  🙂   🖼   📎   ✂        ← 工具行（图标行）        |
//! +--------------------------------------------------+
//! |  文本区（1~8 行自动长高，无边框）                 |
//! +--------------------------------------------------+
//! |                            [ 发送 / 停止 ]        |
//! +--------------------------------------------------+
//! ```
//!
//! **这一层不持有状态、不碰 HTTP、不碰 Desktop 的字段**：调它的人把
//! 「能不能发」「在不在生成」和两个回调递进来。所以这里跟 `app.rs` 的
//! 其它部分（壳、列表、tab）没有一个共享的可变状态，改这里不会撞别人。
//!
//! 工具行的图标这一轮**只有样子**：表情 / 图片 / 文件 / 截图都还没接功能，
//! 点了没反应（`on_click` 里是空的 `None`）。等以后真做附件再说。

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::input::Textarea;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

// 跟聊天区一套的颜色。按钮绿比气泡绿（0x95ec69）深，那是微信主色的习惯。
const COMPOSER_BG: u32 = 0xf7f7f7;
const COMPOSER_LINE: u32 = 0xe7e7e7;
const ICON_IDLE: u32 = 0x5a5a5a;
const SEND_BG: u32 = 0x07c160;
const SEND_BG_IDLE: u32 = 0xe9e9e9;
const SEND_TEXT_IDLE: u32 = 0xb2b2b2;
const STOP_BG: u32 = 0xf2f2f2;
const STOP_TEXT: u32 = 0xe64340;
const HINT_TEXT: u32 = 0x8a8a8a;

/// 调用方告诉输入区的事。全是**只读**快照，没有回调之外的交互。
pub struct State {
    /// 正在生成。true 时右下角是「停止」，工具行变淡。
    pub streaming: bool,
    /// 输入框里有非空白文字。false 时「发送」是灰的（视觉上按不动）。
    pub can_send: bool,
}

/// 画输入区。
///
/// `on_send` / `on_stop` 由调用方用 `cx.listener(...)` 递进来 —— 这一层
/// 不 import `app::Desktop`，免得两个模块互相引用。
pub fn render(
    input: &Entity<gpui_kit::component::input::TextareaState>,
    state: State,
    on_send: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_stop: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let dimmed = state.streaming;

    div()
        .flex()
        .flex_col()
        .w_full()
        .bg(rgb(COMPOSER_BG))
        .border_t(px(1.))
        .border_color(rgb(COMPOSER_LINE))
        // ── 工具行 ──
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .px(px(12.))
                .pt(px(8.))
                .when(dimmed, |el| el.opacity(0.4))
                .child(tool_icon(IconName::FaceGrinning))
                .child(tool_icon(IconName::Image))
                .child(tool_icon(IconName::Paperclip))
                .child(tool_icon(IconName::Scissors)),
        )
        // ── 生成中的提示：回车这时不该发出去，得让人知道为什么 ──
        .when(state.streaming, |el| {
            el.child(
                div()
                    .px(px(12.))
                    .text_size(px(11.))
                    .text_color(rgb(HINT_TEXT))
                    .child("正在生成，回车暂不发送（可以点「停止」，或者等它说完）"),
            )
        })
        // ── 文本区 ──
        .child(
            div()
                .px(px(12.))
                .py(px(4.))
                .child(Textarea::new(input).appearance(false).bordered(false)),
        )
        // ── 底行：右下角的发送 / 停止 ──
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .px(px(12.))
                .pb(px(8.))
                .child(if state.streaming {
                    action_button("composer-stop", "停 止", STOP_BG, STOP_TEXT, true, on_stop)
                        .into_any_element()
                } else {
                    action_button(
                        "composer-send",
                        "发 送",
                        if state.can_send {
                            SEND_BG
                        } else {
                            SEND_BG_IDLE
                        },
                        if state.can_send {
                            0xffffff
                        } else {
                            SEND_TEXT_IDLE
                        },
                        state.can_send,
                        on_send,
                    )
                    .into_any_element()
                }),
        )
}

/// 工具行里的一颗图标。**这一轮没有功能**：hover 只变鼠标指针。
///
/// 颜色和尺寸必须自己给：直接 `.child(IconName::X)` 时用的是**窗口默认文字样式**
/// （本项目里是白字），压在 `#f7f7f7` 上就完全看不见 —— 第一次画就是这么踩的。
fn tool_icon(icon: IconName) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.))
        .id(ElementId::Name(format!("tool-{icon:?}").into()))
        .hover(|style| style.cursor_pointer())
        .child(Icon::new(icon).text_color(rgb(ICON_IDLE)).size(px(18.)))
}

/// 底行按钮。用 `div` 手搓而不是 `Button`：这里的尺寸/圆角/四态色值都是
/// 微信的，套组件反而要一层层覆盖样式。
fn action_button(
    id: &'static str,
    label: &'static str,
    bg: u32,
    fg: u32,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(32.))
        .min_w(px(88.))
        .px(px(16.))
        .rounded(px(4.))
        .bg(rgb(bg))
        .text_size(px(13.))
        .text_color(rgb(fg))
        .id(ElementId::Name(id.into()))
        .when(enabled, |el| el.cursor_pointer())
        .on_click(on_click)
        .child(label)
}
