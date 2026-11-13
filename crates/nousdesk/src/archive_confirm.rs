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
//!
//! 会话 id 是**数字**（`wire.md`：`id` 不是字符串），所以这里直接收 `i64`，
//! 不存字符串、也不在调用方再 `parse` 一次（那会留下一个永不发生的兜底分支）。

use gpui_kit::*;

/// 这一帧的皮。和 `settings.rs` / `board.rs` 同一个写法 —— 渲染层不自己存主题，
/// 统一从帧内快照取，换皮时才不会有一块漏掉。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 确认动作：调用方拿到的唯一可观察结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmAction {
    pub session_id: i64,
    pub session_name: String,
}

/// 弹层状态。`None` = 没开；`Some` = 盖在最上层等用户表态。
#[derive(Default)]
pub struct PurgeConfirm {
    pending: Option<(i64, String)>,
}

impl PurgeConfirm {
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求确认。重复请求覆盖前一条 —— 弹层永远只代表「最后点的那一个」。
    pub fn request(&mut self, session_id: i64, session_name: impl Into<String>) {
        self.pending = Some((session_id, session_name.into()));
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// 正在确认的那条会话名（标题用）。
    pub fn session_name(&self) -> Option<&str> {
        self.pending.as_ref().map(|(_, name)| name.as_str())
    }

    /// Esc：只关，不产出动作。
    pub fn on_esc(&mut self) -> Option<ConfirmAction> {
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
            .bg(rgb(t().p.scrim))
            .on_click(on_close.clone())
    };

    let title = format!("撕原件：{name}");

    let body = div()
        .id(ElementId::Name("purge-dialog".into()))
        .absolute()
        .left(t().d(0.))
        .right(t().d(0.))
        .top(t().d(0.))
        .bottom(t().d(0.))
        .flex()
        .items_center()
        .justify_center()
        // 弹窗本体自己吃掉点击，别让点一下正文就把框关了。
        .on_click(|_, _, _| {})
        .child(
            div()
                .flex()
                .flex_col()
                .w(t().d(400.))
                .rounded(t().r(10.))
                .bg(rgb(t().p.panel))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .shadow_lg()
                .overflow_hidden()
                .child(
                    div()
                        .px(t().d(20.))
                        .pt(t().d(18.))
                        .pb(t().d(12.))
                        .text_size(t().fs(14.))
                        .text_color(rgb(t().p.text))
                        .child(title),
                )
                .child(
                    div()
                        .px(t().d(20.))
                        .pb(t().d(12.))
                        .text_size(t().fs(12.5))
                        .text_color(rgb(t().p.muted))
                        .child("这一条会被真删。归档库里那份复印件也一起销毁。"),
                )
                // 红框：撕完不能撤销。
                .child(
                    div()
                        .mx(t().d(20.))
                        .mb(t().d(16.))
                        .px(t().d(10.))
                        .py(t().d(7.))
                        .rounded(t().r(5.))
                        .bg(rgb(t().p.danger_soft))
                        .border(t().d(1.))
                        .border_color(rgb(t().p.danger))
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.danger_fg))
                        .child("撕完不能撤销"),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(t().d(8.))
                        .px(t().d(20.))
                        .pb(t().d(16.))
                        .child(
                            div()
                                .id(ElementId::Name("purge-cancel".into()))
                                .on_click(on_close)
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(t().d(30.))
                                .px(t().d(14.))
                                .rounded(t().r(5.))
                                .bg(rgb(t().p.panel_alt))
                                .text_color(rgb(t().p.text_2))
                                .text_size(t().fs(12.))
                                .hover(|el| el.bg(rgb(t().p.hover)))
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id(ElementId::Name("purge-confirm".into()))
                                .on_click(on_confirm)
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(t().d(30.))
                                .px(t().d(14.))
                                .rounded(t().r(5.))
                                .bg(rgb(t().p.danger))
                                .text_color(rgb(t().p.on_accent))
                                .text_size(t().fs(12.))
                                .hover(|el| el.bg(rgb(t().p.danger_fg)))
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
    // 别写 `use super::*;` —— 那会把 `gpui_kit::*` 也带进来，
    // `#[test]` 展开时会在它的宏里撞上 recursion limit（踩过）。
    use super::{ConfirmAction, PurgeConfirm};

    /// 缝：公共状态机的动作出口。三条关闭路径都不许产出确认动作，
    /// 且重复关 / 关后再点确认也不许凭空冒出一次。
    fn assert_close_path(close: fn(&mut PurgeConfirm) -> Option<ConfirmAction>) {
        let mut dialog = PurgeConfirm::new();
        dialog.request(42, "会话归档怎么设计");
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

    #[test]
    fn esc_closes_without_confirming() {
        assert_close_path(PurgeConfirm::on_esc);
    }

    #[test]
    fn cancel_closes_without_confirming() {
        assert_close_path(PurgeConfirm::on_cancel);
    }

    /// 只有「撕掉原件」把 `(id, name)` 交出去，且原样、只一次。
    #[test]
    fn confirming_hands_session_identity_to_caller_exactly_once() {
        let mut dialog = PurgeConfirm::new();
        dialog.request(42, "会话归档怎么设计");

        assert_eq!(
            dialog.on_confirm(),
            Some(ConfirmAction {
                session_id: 42,
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
        dialog.request(1, "旧会话");
        dialog.request(2, "新会话");

        assert_eq!(
            dialog.on_confirm().map(|a| (a.session_id, a.session_name)),
            Some((2, "新会话".into()))
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
