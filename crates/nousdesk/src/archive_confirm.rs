//! 「归档并撕原件」危险确认弹层。
//!
//! 纯 UI 的一层：不认引擎、不发 HTTP、不碰 `Desktop` 的状态。调用方注入
//! 回调，本模块只决定「关掉还是把 `(id, name)` 交给回调」。
//!
//! **手输名字才解锁**：前面一步是归档（可逆），这一步撕原件不可撤销 —— 必须
//! 让用户再动手打一遍会话名，红色按钮才亮。输入框由调用方注入，本模块只问
//! 「输入框里是不是这个名字」（[`PurgeConfirm::matches_typed`]，纯函数）。
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

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
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
    /// `true` = 先归档成功再撕（右键一步到位）；`false` = 已经归档过，只撕。
    pub chain_archive: bool,
}

/// 弹层状态。`None` = 没开；`Some` = 盖在最上层等用户表态。
#[derive(Default)]
pub struct PurgeConfirm {
    pending: Option<(i64, String)>,
    /// `true` = 这次要先归档再撕（从右键「归档并撕原件」来的）；
    /// `false` = 已经归档过，只撕（从归档库页来的）。
    chain_archive: bool,
    /// 刚打开、输入框还没清。渲染帧里 `take_fresh()` 拿一次就作废 ——
    /// 和 `rename_box` 同一个套路：清值 / 抢焦点要 `&mut Window`，打开回调手上没有。
    fresh: std::cell::Cell<bool>,
}

impl PurgeConfirm {
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求确认。重复请求覆盖前一条 —— 弹层永远只代表「最后点的那一个」。
    ///
    /// `chain_archive` 见字段说明：右键来的是 true（先归档再撕），
    /// 归档库页来的是 false（已经归档过，只撕）。
    pub fn request(&mut self, session_id: i64, session_name: impl Into<String>, chain_archive: bool) {
        self.pending = Some((session_id, session_name.into()));
        self.chain_archive = chain_archive;
        self.fresh.set(true);
    }

    /// 该不该在这一帧清空输入框 + 抢焦点。问一次就作废。
    pub fn take_fresh(&self) -> bool {
        self.fresh.replace(false)
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
            chain_archive: self.chain_archive,
        })
    }

    /// 输入框里打的名字和正在确认的会话**完全一致**（trim 后）才解锁按钮。
    ///
    /// 抽成纯函数是为了单测：少一个字、多一个空格、空着 —— 都得是 `false`。
    pub fn matches_typed(&self, typed: &str) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, name)| !name.is_empty() && typed.trim() == name)
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
    input: &Entity<InputState>,
    typed: &str,
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
            .bg(rgba(t().p.scrim))
            .on_click(on_close.clone())
    };

    let title = format!("归档并撕原件：{name}");

    // 这一层是**整屏**的居中容器，它同时是「点空白」的接收方：
    // 点卡片时卡片的处理器 `stop_propagation`，点到卡片外面就落到这里 → 关。
    // 反过来的写法（容器 stop、只留罩子关）会让「点空白」**永远关不掉**：
    // 容器的命中区盖满整屏，罩子根本收不到那一击（坑.md 85）。
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
        .on_click({
            let on_close = on_close.clone();
            move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                on_close(ev, window, cx);
            }
        })
        .child(
            div()
                .id(ElementId::Name("purge-card".into()))
                // 卡片自己吃掉点击：不然点正文会顺着容器关掉弹窗（坑.md 61）。
                .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
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
                        .pb(t().d(10.))
                        .text_size(t().fs(12.5))
                        .text_color(rgb(t().p.muted))
                        .child("两步连着做：先复印进归档库，再真删活库里的原件。撕完不能撤销。"),
                )
                // 红框：必须打对名字才往下走。
                .child(
                    div()
                        .mx(t().d(20.))
                        .mb(t().d(12.))
                        .px(t().d(10.))
                        .py(t().d(7.))
                        .rounded(t().r(5.))
                        .bg(rgb(t().p.danger_soft))
                        .border(t().d(1.))
                        .border_color(rgb(t().p.danger))
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.danger_fg))
                        .child("这一步会删掉原件和它的复印件 —— 把你打的会话名核对一遍。"),
                )
                // 手输名字：把名字打对，红色按钮才亮。
                .child(
                    div()
                        .px(t().d(20.))
                        .pb(t().d(14.))
                        .flex()
                        .flex_col()
                        .gap(t().d(5.))
                        .child(
                            div()
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.text_2))
                                .child(format!("输入「{name}」以确认")),
                        )
                        .child(Input::new(input).text_size(t().fs(12.))),
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
                                .on_click({
                                    let on_close = on_close.clone();
                                    move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                                        on_close(ev, window, cx);
                                        cx.stop_propagation();
                                    }
                                })
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
                        .child({
                            let ready = state.matches_typed(typed);
                            div()
                                .id(ElementId::Name("purge-confirm".into()))
                                .when(ready, |el| {
                                    el.on_click(move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                                        on_confirm(ev, window, cx);
                                        cx.stop_propagation();
                                    })
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(t().d(30.))
                                .px(t().d(14.))
                                .rounded(t().r(5.))
                                .bg(rgb(if ready { t().p.danger } else { t().p.disabled }))
                                .text_color(rgb(t().p.on_accent))
                                .text_size(t().fs(12.))
                                .when(ready, |el| el.hover(|el| el.bg(rgb(t().p.danger_fg))))
                                .child("撕掉原件")
                        }),
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
        dialog.request(42, "会话归档怎么设计", false);
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
        dialog.request(42, "会话归档怎么设计", false);

        assert_eq!(
            dialog.on_confirm(),
            Some(ConfirmAction {
                session_id: 42,
                session_name: "会话归档怎么设计".into(),
                chain_archive: false,
            })
        );
        assert_eq!(dialog.on_confirm(), None, "一次请求只产出一次确认");
        assert!(!dialog.is_open());
    }

    /// 重复请求覆盖前一条：确认时拿到的是最后请求的那条，别撕错会话。
    #[test]
    fn repeated_request_replaces_the_pending_session() {
        let mut dialog = PurgeConfirm::new();
        dialog.request(1, "旧会话", false);
        dialog.request(2, "新会话", false);

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

    /// 手输名字：只有**一字不差**才解锁，多空格 / 少字 / 空着都不行。
    #[test]
    fn typed_name_must_match_exactly() {
        let mut dialog = PurgeConfirm::new();
        dialog.request(42, "会话归档怎么设计", false);

        assert!(!dialog.matches_typed(""), "空着不能解锁");
        assert!(!dialog.matches_typed("会话归档怎么设"), "少一个字不能解锁");
        assert!(dialog.matches_typed("会话归档怎么设计"), "一字不差才解锁");
        // 前后空白算手滑，trim 一下放行（和 rename_box 的口径一致）。
        assert!(dialog.matches_typed("  会话归档怎么设计  "));
        assert!(dialog.matches_typed("会话归档怎么设计 "));
    }

    /// 右键来的确认带着 `chain_archive: true`，归档库页来的带 false ——
    /// 调用方靠这个字段决定"先归档再撕"还是"只撕"，传错了会撕错东西。
    #[test]
    fn chain_flag_flows_into_the_action() {
        let mut d = PurgeConfirm::new();
        d.request(1, "右键来的", true);
        let a = d.on_confirm().unwrap();
        assert!(a.chain_archive, "右键一步到位必须带 chain=true");

        d.request(2, "归档库来的", false);
        let a = d.on_confirm().unwrap();
        assert!(!a.chain_archive, "归档库页那条已经归档过，chain=false");
    }

    /// fresh 标记问一次就作废：不能每帧都清空输入框（用户正打到一半）。
    #[test]
    fn fresh_is_consumed_once() {
        let mut d = PurgeConfirm::new();
        assert!(!d.take_fresh(), "没请求过不该 fresh");
        d.request(1, "x", true);
        assert!(d.take_fresh(), "刚请求应当 fresh");
        assert!(!d.take_fresh(), "问过一次就作废");
    }

    #[test]
    fn empty_session_name_never_unlocks() {
        let mut dialog = PurgeConfirm::new();
        dialog.request(42, "", false);
        assert!(!dialog.matches_typed(""), "空名字的会话不该存在这种确认");
    }
}
