//! 「重命名会话」弹层（居中对话框，照微信桌面那套）。
//!
//! 纯 UI 的一层：不认引擎、不发 HTTP、不碰 `Desktop` 的状态。调用方注入输入框
//! 和两个回调，本模块只决定「关掉 / 空名字打回 / 把 `(id, 新名字)` 交出去」。
//!
//! 对外三个面：[`RenameBox`]（状态）、[`render_rename_box`]（渲染）、
//! [`RenameBox::request`]（打开）。
//!
//! 状态机的出口是**数据**（[`Submit`]），不是私有字段：三条路径（Esc / 取消 / 提交）
//! 各自产出什么因此可以纯单元测试，不必开窗口。
//!
//! 输入框是**外部注入**的 `Entity<InputState>`：`Desktop` 持有一份、打开时把
//! 当前名字塞进去。本模块不新建输入框 —— 那样每开一次就多一个 `Entity`，
//! 焦点还得自己追。
//!
//! 键盘 Esc 不在这里挂：项目把这个口子放在 `Desktop::new` 的 `cx.intercept_keystrokes`
//! （见 `app.rs` 里 `/` 面板、配置面板、撕原件那几段），由接线那头调
//! [`RenameBox::on_esc`]。
//!
//! 会话 id 是**数字**（`wire.md`：`id` 不是字符串），所以这里直接收 `i64`。

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::Sizable as _;
use gpui_kit::*;

/// 这一帧的皮。和 `archive_confirm.rs` / `settings.rs` 同一个写法。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 提交成功后交给调用方的东西。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameAction {
    pub session_id: i64,
    /// **已经 trim 过**的新名字。
    pub name: String,
}

/// 「提交」这一下的三种结果。
///
/// `Empty` 不是错误 —— 它是「名字还得再填」，弹层**留着**让用户接着改。
/// 空名字不让发到引擎：`wire.md` 记着引擎会 400 `name 是空的`，白跑一趟
/// 还得再弹一句错，不如在本地就拦。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submit {
    /// 压根没开着（或者已经交出去过一次）。
    Nothing,
    /// `trim` 完是空的：弹层留着，调用方给一句提示。
    Empty,
    /// 交出去，调用方去发 `POST /v1/sessions/:id/rename`。
    Go(RenameAction),
}

/// 弹层状态。`None` = 没开；`Some((id, 原名))` = 正在改那一条。
#[derive(Default)]
pub struct RenameBox {
    pending: Option<(i64, String)>,
    /// 「刚打开、输入框还没填」。
    ///
    /// 填初值和抢焦点都要 `window`，而打开弹层那条路（右键菜单回调）手上没有。
    /// 所以这里只立一个标记，等**渲染**那一帧再填 —— 那边 `&mut Window` 在手上。
    /// 和盘面的 `needs_focus` 是同一个套路（同一个坑：无 WM 的 X 环境里
    /// 到处都想抢焦点，晚一点拿反而稳）。
    fresh: std::cell::Cell<bool>,
}

impl RenameBox {
    pub fn new() -> Self {
        Self::default()
    }

    /// 打开：记下改哪一条、原来叫什么。重复请求覆盖前一条 ——
    /// 弹层永远只代表「最后点的那一个」。
    pub fn request(&mut self, session_id: i64, session_name: impl Into<String>) {
        self.pending = Some((session_id, session_name.into()));
        self.fresh.set(true);
    }

    /// 该不该在这一帧填初值 + 抢焦点。问一次就作废（`replace(false)`）。
    ///
    /// 返回 `Some((id, 原名))` 时调用方去 `set_value` 并 `focus`。
    pub fn take_fresh(&self) -> Option<(i64, String)> {
        if !self.fresh.replace(false) {
            return None;
        }
        self.pending
            .as_ref()
            .map(|(id, name)| (*id, name.clone()))
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// 正在改的那一条 `(id, 原名)`。调用方拿原名去填输入框的初值。
    pub fn target(&self) -> Option<(i64, &str)> {
        self.pending
            .as_ref()
            .map(|(id, name)| (*id, name.as_str()))
    }

    /// Esc：只关，不产出动作。
    pub fn on_esc(&mut self) -> Submit {
        self.dismiss()
    }

    /// 点「取消」/ 点罩子：只关，不产出动作。
    pub fn on_cancel(&mut self) -> Submit {
        self.dismiss()
    }

    /// 点「完成」：`raw` 是输入框里的原文。`trim` 后为空就打回，弹层留着。
    pub fn submit(&mut self, raw: &str) -> Submit {
        let Some((id, _)) = self.pending.as_ref() else {
            return Submit::Nothing;
        };
        let id = *id;
        let name = raw.trim();
        if name.is_empty() {
            return Submit::Empty;
        }
        // 先取出再返回：只交一次，重复点不会发两次请求。
        self.pending = None;
        Submit::Go(RenameAction {
            session_id: id,
            name: name.to_string(),
        })
    }

    fn dismiss(&mut self) -> Submit {
        self.pending = None;
        Submit::Nothing
    }
}

/// 画这一层。没开就返回一个空元素（`children` 里塞一个空 div 不花钱）。
pub fn render_rename_box(
    state: &RenameBox,
    input: &Entity<InputState>,
    on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + Clone + 'static,
) -> AnyElement {
    if !state.is_open() {
        return div().into_any_element();
    }

    // 罩子排在弹窗**之前**画：GPUI 是后者盖前者。点罩子 = 只关（和撕原件同一套）。
    let scrim = {
        let on_close = on_close.clone();
        div()
            .id(ElementId::Name("rename-scrim".into()))
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(on_close.clone())
    };

    let body = div()
        .id(ElementId::Name("rename-dialog".into()))
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        // 弹窗本体吃掉点击**并且 stop**：只「吃掉」不够 —— 同一个点击还会继续传给
        // 兄弟那层罩子，表现成「点正文，弹窗自己关了」（坑.md 61）。
        .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
        .child(
            div()
                .flex()
                .flex_col()
                .w(t().d(340.))
                .rounded(t().r(10.))
                .bg(rgb(t().p.panel))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .shadow_lg()
                .overflow_hidden()
                .child(
                    div()
                        .px(t().d(18.))
                        .pt(t().d(16.))
                        .pb(t().d(10.))
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.muted))
                        .child("重命名会话"),
                )
                .child(
                    div()
                        .mx(t().d(18.))
                        .text_size(t().fs(13.))
                        .child(Input::new(input).small()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(t().d(8.))
                        .px(t().d(18.))
                        .pt(t().d(12.))
                        .pb(t().d(16.))
                        .child(
                            div()
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.muted))
                                .child("空名字存不了"),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(t().d(8.))
                                .child(
                                    div()
                                        .id(ElementId::Name("rename-cancel".into()))
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
                                        .id(ElementId::Name("rename-confirm".into()))
                                        .on_click(on_confirm)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .h(t().d(30.))
                                        .px(t().d(14.))
                                        .rounded(t().r(5.))
                                        .bg(rgb(t().p.accent))
                                        .text_color(rgb(t().p.on_accent))
                                        .text_size(t().fs(12.))
                                        .hover(|el| el.bg(rgb(t().p.accent_fg)))
                                        .child("完成"),
                                ),
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
    use super::{RenameAction, RenameBox, Submit};

    #[test]
    fn 三条关闭路径都不产出动作() {
        for close in [
            RenameBox::on_esc as fn(&mut RenameBox) -> Submit,
            RenameBox::on_cancel as fn(&mut RenameBox) -> Submit,
        ] {
            let mut b = RenameBox::new();
            b.request(42, "真模型");
            assert!(b.is_open());
            assert_eq!(close(&mut b), Submit::Nothing, "关闭路径不该产出动作");
            assert!(!b.is_open(), "关闭后弹层应当收起");
            assert_eq!(close(&mut b), Submit::Nothing, "重复关闭也不该产出动作");
            assert_eq!(
                b.submit("新名字"),
                Submit::Nothing,
                "关掉之后再点完成不该凭空冒出一条"
            );
        }
    }

    #[test]
    fn 提交带出去的_id_是刚才请求的那一条() {
        let mut b = RenameBox::new();
        b.request(7, "旧名");
        assert_eq!(b.target(), Some((7, "旧名")), "原名要给输入框当初值");
        assert_eq!(
            b.submit("  新名字  "),
            Submit::Go(RenameAction {
                session_id: 7,
                name: "新名字".into(),
            }),
            "名字要 trim，id 要原样带出去"
        );
        assert!(!b.is_open(), "交出去之后弹层应当收起");
    }

    #[test]
    fn 提交只交一次() {
        let mut b = RenameBox::new();
        b.request(7, "旧名");
        assert!(matches!(b.submit("新名字"), Submit::Go(_)));
        assert_eq!(b.submit("再点一次"), Submit::Nothing, "连点不该发两次请求");
    }

    #[test]
    fn 空名字打回_弹层留着() {
        for raw in ["", "   ", "\t\n"] {
            let mut b = RenameBox::new();
            b.request(7, "旧名");
            assert_eq!(b.submit(raw), Submit::Empty, "trim 完是空就打回");
            assert!(b.is_open(), "打回之后弹层要留着，让用户接着改");
        }
    }

    #[test]
    fn 没开的时候提交不产出() {
        let mut b = RenameBox::new();
        assert_eq!(b.submit("新名字"), Submit::Nothing);
        assert_eq!(b.target(), None);
    }

    #[test]
    fn 重复请求覆盖前一条() {
        let mut b = RenameBox::new();
        b.request(1, "甲");
        b.request(2, "乙");
        assert_eq!(b.target(), Some((2, "乙")), "弹层只代表最后点的那一条");
    }
}
