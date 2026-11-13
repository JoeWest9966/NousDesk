//! 「撕原件」危险确认弹层（票 #3）。
//!
//! 纯 UI 的一层：不认引擎、不发 HTTP、不碰 `Desktop` 的状态。调用方注入
//! 回调，本模块只决定「关掉还是把 `(id, name)` 交给回调」。
//!
//! 对外三个面：[`PurgeConfirm`]（状态）、[`render_purge_confirm`]（渲染）、
//! [`PurgeConfirm::request`]（请求确认）。
//!
//! 状态机的出口是**数据**（`Option<ConfirmAction>`），不是私有字段：
//! 「关闭路径不产生确认动作」因此可以纯单元测试，不必开窗口。
//!
//! 键盘 Esc 不在这里挂：项目把这个口子放在 `Desktop::new` 的
//! `cx.intercept_keystrokes`（见 `app.rs` 里 `/` 面板与配置面板那两段），
//! 由接线那票调 [`PurgeConfirm::on_esc`]。

use gpui_kit::*;

/// 危险红。和 `settings.rs` 的 `BAD` 同一个值 —— 全项目只有这一种「危险」。
const BAD: u32 = 0xd32f2f;
/// 危险红的浅底（红框）。
const BAD_SOFT: u32 = 0xfdecec;
const MUTED: u32 = 0x999999;
const PANEL: u32 = 0xffffff;
const LINE: u32 = 0xe6e6e6;
const TEXT: u32 = 0x2b2b2b;

/// 确认动作：调用方拿到的唯一可观察结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmAction {
    pub session_id: String,
    pub session_name: String,
}

/// 弹层状态。`None` = 没开；`Some` = 盖在最上层等用户表态。
#[derive(Default)]
pub struct PurgeConfirm {
    pending: Option<(String, String)>,
}

impl PurgeConfirm {
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求确认。重复请求覆盖前一条 —— 弹层永远只代表「最后点的那一个」。
    pub fn request(&mut self, session_id: impl Into<String>, session_name: impl Into<String>) {
        self.pending = Some((session_id.into(), session_name.into()));
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// 正在确认的那条会话名（标题用）。
    pub fn session_name(&self) -> Option<&str> {
        self.pending.as_ref().map(|(_, name)| name.as_str())
    }

    /// 正在确认的那条会话 id。
    pub fn session_id(&self) -> Option<&str> {
        self.pending.as_ref().map(|(id, _)| id.as_str())
    }

    /// Esc：只关，不产出动作。
    pub fn on_esc(&mut self) -> Option<ConfirmAction> {
        self.dismiss()
    }

    /// 点罩子空白：只关，不产出动作。
    pub fn on_scrim_click(&mut self) -> Option<ConfirmAction> {
        self.dismiss()
    }

    /// 点「取消」：只关，不产出动作。
    pub fn on_cancel(&mut self) -> Option<ConfirmAction> {
        self.dismiss()
    }

    /// 点「撕掉原件」：把 `(id, name)` 原样交出去，并且只交一次。
    pub fn on_confirm(&mut self) -> Option<ConfirmAction> {
        let (session_id, session_name) = self.pending.take()?;
        Some(ConfirmAction {
            session_id,
            session_name,
        })
    }

    fn dismiss(&mut self) -> Option<ConfirmAction> {
        self.pending = None;
        None
    }
}

/// 画这一层。没开就返回一个空元素（`children` 里塞一个空 div 不花钱）。
///
/// `on_confirm` / `on_close` 都由调用方注入：本模块不知道 `Desktop` 长什么样，
/// 也不该知道。两个回调都是 `on_click` 的形状，直接吃 `cx.listener(...)`。
pub fn render_purge_confirm(
    state: &PurgeConfirm,
    on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + Clone + 'static,
) -> AnyElement {
    if !state.is_open() {
        return div().into_any_element();
    }
    let name = state.session_name().unwrap_or_default().to_owned();

    // 罩子排在弹窗**之前**画：GPUI 是后者盖前者。
    // 点罩子 = 只关，不产出确认动作（和 `render_menu_overlay` 同一套）。
    let scrim = {
        let on_close = on_close.clone();
        div()
            .id(ElementId::Name("purge-scrim".into()))
            .absolute()
            .inset_0()
            .bg(rgba(0x0000001f))
            .on_click(on_close.clone())
    };

    let title = format!("撕原件：{name}");

    let body = div()
        .id(ElementId::Name("purge-dialog".into()))
        .absolute()
        .left(px(0.))
        .right(px(0.))
        .top(px(0.))
        .bottom(px(0.))
        .flex()
        .items_center()
        .justify_center()
        // 弹窗本体自己吃掉点击，别让点一下正文就把框关了。
        .on_click(|_, _, _| {})
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(400.))
                .rounded(px(10.))
                .bg(rgb(PANEL))
                .border(px(1.))
                .border_color(rgb(LINE))
                .shadow_lg()
                .overflow_hidden()
                .child(
                    div()
                        .px(px(20.))
                        .pt(px(18.))
                        .pb(px(12.))
                        .text_size(px(14.))
                        .text_color(rgb(TEXT))
                        .child(title),
                )
                .child(
                    div()
                        .px(px(20.))
                        .pb(px(12.))
                        .text_size(px(12.5))
                        .text_color(rgb(MUTED))
                        .child("这一条会被真删。归档库里那份复印件也一起销毁。"),
                )
                // 红框：撕完不能撤销。
                .child(
                    div()
                        .mx(px(20.))
                        .mb(px(16.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(5.))
                        .bg(rgb(BAD_SOFT))
                        .border(px(1.))
                        .border_color(rgb(BAD))
                        .text_size(px(12.))
                        .text_color(rgb(BAD))
                        .child("撕完不能撤销"),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .px(px(20.))
                        .pb(px(16.))
                        .child(
                            div()
                                .id(ElementId::Name("purge-cancel".into()))
                                .on_click(on_close)
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(30.))
                                .px(px(14.))
                                .rounded(px(5.))
                                .bg(rgb(0xf0f0f0))
                                .text_color(rgb(0x444444))
                                .text_size(px(12.))
                                .hover(|el| el.bg(rgb(0xe4e4e4)))
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id(ElementId::Name("purge-confirm".into()))
                                .on_click(on_confirm)
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(30.))
                                .px(px(14.))
                                .rounded(px(5.))
                                .bg(rgb(BAD))
                                .text_color(rgb(0xffffff))
                                .text_size(px(12.))
                                .hover(|el| el.bg(rgb(0xb71c1c)))
                                .child("撕掉原件"),
                        ),
                ),
        );

    div()
        .absolute()
        .inset_0()
        .child(scrim)
        .child(body)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 缝：公共状态机的动作出口。关闭三条路径都不许产出确认动作，
    /// 且重复关 / 关后再点确认也不许凭空冒出一次。
    #[test]
    fn closing_paths_produce_no_confirm_action() {
        for close in [
            PurgeConfirm::on_esc as fn(&mut PurgeConfirm) -> Option<ConfirmAction>,
            PurgeConfirm::on_scrim_click,
            PurgeConfirm::on_cancel,
        ] {
            let mut dialog = PurgeConfirm::new();
            dialog.request("s-42", "会话归档怎么设计");
            assert!(dialog.is_open(), "请求确认后弹层应当在场");

            assert_eq!(close(&mut dialog), None, "关闭路径不该产出确认动作");
            assert!(!dialog.is_open(), "关闭后弹层应当收起");
            assert_eq!(close(&mut dialog), None, "重复关闭也不该产出动作");
            assert_eq!(
                dialog.on_confirm(),
                None,
                "关掉之后再点确认不该凭空冒出一次"
            );
        }
    }

    /// 只有「撕掉原件」把 `(id, name)` 交出去，且原样、只一次。
    #[test]
    fn confirming_hands_session_identity_to_caller_exactly_once() {
        let mut dialog = PurgeConfirm::new();
        dialog.request("s-42", "会话归档怎么设计");

        assert_eq!(
            dialog.on_confirm(),
            Some(ConfirmAction {
                session_id: "s-42".into(),
                session_name: "会话归档怎么设计".into(),
            })
        );
        assert_eq!(dialog.on_confirm(), None, "一次请求只产出一次确认");
        assert!(!dialog.is_open());
    }

    /// 重复请求覆盖前一条：确认时拿到的是最后请求的那条，别撕错会话。
    #[test]
    fn repeated_request_replaces_the_pending_session() {
        let mut dialog = PurgeConfirm::new();
        dialog.request("old", "旧会话");
        dialog.request("new", "新会话");

        assert_eq!(
            dialog.on_confirm().map(|a| (a.session_id, a.session_name)),
            Some(("new".into(), "新会话".into()))
        );
    }

    /// 没开的时候点确认不该产出任何东西（罩子关掉后误触）。
    #[test]
    fn confirming_when_closed_is_a_noop() {
        let mut dialog = PurgeConfirm::new();
        assert_eq!(dialog.on_confirm(), None);
        assert!(!dialog.is_open());
    }
}
