//! 预设「归档 / 彻底删」的二确认（Template 页）。
//!
//! 形态是 demo 里挑的**分步两屏**（`demo/template-page/lifecycle.html` 组 ② 方案 6）：
//! 第一屏把后果说清 + 勾一句「我知道」，第二屏才是那个真按钮。两屏共用一个弹层，
//! 切屏不重开 —— 所以我们自己拿 `step`，不用两次 `request`。
//!
//! 这个模块只管状态和文案，不发 HTTP、不认识 `Desktop`：`on_confirm` 把
//! `(op, section, name)` 原样交出去，且**只交一次**。

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use nous_engine::types::PresetOp;

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 正在确认的那一条。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    op: PresetOp,
    section: String,
    name: String,
}

/// 弹层状态。`pending == None` = 没开。
///
/// `step`：1 = 说后果 + 勾选；2 = 真按钮。`back` 回 1 时**保留勾选**（人已经表过态了，
/// 让他再勾一遍只是烦）。
#[derive(Default)]
pub struct PresetConfirm {
    pending: Option<Pending>,
    step: u8,
    ack: bool,
}

impl PresetConfirm {
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求确认。重复请求覆盖前一条，并且**重置到第一屏** ——
    /// 上一个弹层勾的和这个不是一个东西，不能顺着用。
    pub fn request(&mut self, op: PresetOp, section: impl Into<String>, name: impl Into<String>) {
        self.pending = Some(Pending {
            op,
            section: section.into(),
            name: name.into(),
        });
        self.step = 1;
        self.ack = false;
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    pub fn step(&self) -> u8 {
        self.step
    }

    pub fn acked(&self) -> bool {
        self.ack
    }

    pub fn op(&self) -> Option<PresetOp> {
        self.pending.as_ref().map(|p| p.op)
    }

    pub fn preset_name(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.name.as_str())
    }

    pub fn section(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.section.as_str())
    }

    /// 勾 / 取消那句「我知道」。
    pub fn toggle_ack(&mut self) {
        self.ack = !self.ack;
    }

    /// 第一屏的「下一步」能不能点。没勾就不许过 —— 这是两屏存在的**唯一**理由。
    pub fn can_advance(&self) -> bool {
        self.is_open() && self.step == 1 && self.ack
    }

    /// 第一屏 → 第二屏。没勾返回 `false`（调用方据此不动）。
    pub fn advance(&mut self) -> bool {
        if self.can_advance() {
            self.step = 2;
            true
        } else {
            false
        }
    }

    /// 第二屏 → 第一屏（勾选保留）。
    pub fn back(&mut self) {
        if self.step == 2 {
            self.step = 1;
        }
    }

    /// Esc / 取消 / 点罩子：只关，不产出动作。
    pub fn on_esc(&mut self) {
        self.pending = None;
        self.ack = false;
        self.step = 1;
    }

    /// 第二屏那个真按钮。**只在第二屏**才交出去，而且只交一次。
    pub fn take(&mut self) -> Option<(PresetOp, String, String)> {
        if self.step != 2 {
            return None;
        }
        let p = self.pending.take()?;
        self.ack = false;
        self.step = 1;
        Some((p.op, p.section, p.name))
    }
}

/// 弹层标题。
pub fn title(op: PresetOp) -> &'static str {
    match op {
        PresetOp::Archive => "归档这条模板？",
        PresetOp::Restore => "还原这条模板？",
        PresetOp::Purge => "彻底删掉这条模板？",
    }
}

/// 第一屏正文：**先说后果**，不先说操作。
pub fn body(op: PresetOp) -> &'static str {
    match op {
        PresetOp::Archive => {
            "整个目录会移到 archives/ 下面，这一段的名册会重扫。它不在列表里了，但随时能还原。"
        }
        PresetOp::Restore => "会从 archives/ 挪回活库，重新出现在列表里。",
        PresetOp::Purge => {
            "活库里的整个目录会删掉；archives/ 里如果有它的副本也会一起删。这一步之后只能靠 git 找回。"
        }
    }
}

/// 第一屏那个勾选框旁边的字。**说清不可逆的是什么**，不说「我同意」。
pub fn ack_text(op: PresetOp) -> &'static str {
    match op {
        PresetOp::Archive => "我知道它会从列表里消失，但能在归档里找回来。",
        PresetOp::Restore => "我知道它会重新出现在这一段的列表里。",
        PresetOp::Purge => "我知道这一步不能撤销，也知道 git 之外没有别的副本。",
    }
}

/// 第二屏的确认按钮字。
pub fn confirm_label(op: PresetOp) -> &'static str {
    match op {
        PresetOp::Archive => "归档",
        PresetOp::Restore => "还原",
        PresetOp::Purge => "彻底删掉",
    }
}

/// 画这一层。没开就返回空元素。
pub fn render_preset_confirm(
    state: &PresetConfirm,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + Clone + 'static,
    on_toggle_ack: impl Fn(&ClickEvent, &mut Window, &mut App) + Clone + 'static,
    on_advance: impl Fn(&ClickEvent, &mut Window, &mut App) + Clone + 'static,
    on_back: impl Fn(&ClickEvent, &mut Window, &mut App) + Clone + 'static,
    on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Some(op) = state.op() else {
        return div().into_any_element();
    };
    let name = state.preset_name().unwrap_or_default().to_owned();
    let section = state.section().unwrap_or_default().to_owned();
    let step = state.step();
    let acked = state.acked();
    let danger = op.irreversible();

    // 罩子先画：GPUI 后者盖前者。点罩子 = 只关。
    let scrim = {
        let on_close = on_close.clone();
        div()
            .id(ElementId::Name("preset-confirm-scrim".into()))
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(on_close)
    };

    let head = div()
        .flex()
        .items_center()
        .gap(t().d(8.))
        .child(
            div()
                .text_size(t().fs(10.))
                .text_color(rgb(if danger { t().p.danger } else { t().p.muted }))
                .child(if danger {
                    "不可撤销 · 第 1 / 2 步".replace("1", &step.to_string())
                } else {
                    format!("可还原 · 第 {step} / 2 步")
                }),
        );

    let mut body = div()
        .id(ElementId::Name("preset-confirm-body".into()))
        .flex()
        .flex_col()
        .w(t().d(430.))
        .rounded(t().r(10.))
        .bg(rgb(t().p.panel))
        .border(t().d(1.))
        .border_color(rgb(if danger {
            t().p.danger
        } else {
            t().p.line
        }))
        .shadow_lg()
        .overflow_hidden()
        // **必须 stop**：这里只是「吃掉点击」不够 —— GPUI 里同一个点击还会继续
        // 传给下面那层罩子，表现成「点勾选框 / 点下一步，弹层自己关了」（坑.md 61 同款）。
        .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t().d(10.))
                .px(t().d(22.))
                .pt(t().d(18.))
                .pb(t().d(14.))
                .child(head)
                .child(
                    div()
                        .text_size(t().fs(15.))
                        .child(div().child(title(op)).child(
                            div()
                                .mt(t().d(3.))
                                .text_size(t().fs(13.))
                                .text_color(rgb(t().p.muted))
                                .child(format!("{section} / {name}")),
                        )),
                )
                .child(
                    div()
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.muted))
                        .child(body(op)),
                )
                // 第二屏：把「刚刚确认过什么」再说一遍，别让人只看见一个红按钮。
                .when(step == 2, |el| {
                    el.child(
                        div()
                            .mt(t().d(2.))
                            .px(t().d(10.))
                            .py(t().d(8.))
                            .rounded(t().r(4.))
                            .bg(rgb(t().p.panel_alt))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(ack_text(op)),
                    )
                })
                // 第一屏：勾选。没勾过不许过。
                .when(step == 1, |el| {
                    let on_toggle_ack = on_toggle_ack.clone();
                    el.child(
                        div()
                            .id(ElementId::Name("preset-confirm-ack".into()))
                            .flex()
                            .items_start()
                            .gap(t().d(9.))
                            .mt(t().d(4.))
                            .cursor_pointer()
                            .on_click(on_toggle_ack)
                            .child(
                                div()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .mt(t().d(1.))
                                    .size(t().d(15.))
                                    .rounded(t().r(3.))
                                    .border(t().d(1.))
                                    .border_color(rgb(if acked {
                                        if danger { t().p.danger } else { t().p.accent }
                                    } else {
                                        t().p.line
                                    }))
                                    .bg(rgb(if acked {
                                        if danger { t().p.danger } else { t().p.accent }
                                    } else {
                                        t().p.panel
                                    }))
                                    .text_size(t().fs(10.))
                                    .text_color(rgb(if danger {
                                        t().p.panel
                                    } else {
                                        t().p.on_accent
                                    }))
                                    .children(acked.then(|| "✓".to_string())),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(12.))
                                    .text_color(rgb(if acked {
                                        t().p.text
                                    } else {
                                        t().p.muted
                                    }))
                                    .child(ack_text(op)),
                            ),
                    )
                }),
        );

    // 底栏：第一屏「取消 / 下一步」；第二屏「退回 / 真按钮」。
    let foot = div()
        .flex()
        .items_center()
        .justify_end()
        .gap(t().d(8.))
        .px(t().d(22.))
        .py(t().d(14.))
        .border_t(t().d(1.))
        .border_color(rgb(t().p.line));

    let foot = if step == 1 {
        let on_advance = on_advance.clone();
        let on_close = on_close.clone();
        foot.child(
            div()
                .id(ElementId::Name("preset-confirm-cancel".into()))
                .px(t().d(14.))
                .py(t().d(6.))
                .rounded(t().r(6.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(12.))
                .cursor_pointer()
                .on_click(on_close)
                .child("取消"),
        )
        .child(
            div()
                .id(ElementId::Name("preset-confirm-next".into()))
                .px(t().d(16.))
                .py(t().d(6.))
                .rounded(t().r(6.))
                // 没勾时**不置灰到看不见**：只降一档底色 + 加一条边，
                // 字还是能读的（「下一步」三个字看不见等于按钮消失）。
                .bg(rgb(if acked {
                    t().p.accent
                } else {
                    t().p.panel_alt
                }))
                .border(t().d(1.))
                .border_color(rgb(if acked { t().p.accent } else { t().p.line }))
                .text_color(rgb(if acked {
                    t().p.on_accent
                } else {
                    t().p.muted
                }))
                .text_size(t().fs(12.))
                .when(acked, |el| el.cursor_pointer().on_click(on_advance))
                .child("下一步"),
        )
    } else {
        let on_back = on_back.clone();
        foot.child(
            div()
                .id(ElementId::Name("preset-confirm-back".into()))
                .px(t().d(14.))
                .py(t().d(6.))
                .rounded(t().r(6.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(12.))
                .cursor_pointer()
                .on_click(on_back)
                .child("退回"),
        )
        .child(
            div()
                .id(ElementId::Name("preset-confirm-go".into()))
                .px(t().d(16.))
                .py(t().d(6.))
                .rounded(t().r(6.))
                .bg(rgb(if danger { t().p.danger } else { t().p.accent }))
                .text_color(rgb(if danger { t().p.panel } else { t().p.on_accent }))
                .text_size(t().fs(12.))
                .cursor_pointer()
                .on_click(on_confirm)
                .child(confirm_label(op)),
        )
    };

    body = body.child(foot);

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(scrim)
        .child(body)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{
        PresetConfirm, ack_text, body, confirm_label, title,
    };
    use nous_engine::types::PresetOp;

    fn open(op: PresetOp) -> PresetConfirm {
        let mut c = PresetConfirm::new();
        c.request(op, "Session-Template", "deep-dive");
        c
    }

    #[test]
    fn a_fresh_confirm_opens_on_step_one_with_nothing_acked() {
        let c = open(PresetOp::Purge);
        assert!(c.is_open());
        assert_eq!(c.step(), 1);
        assert!(!c.acked());
        assert!(!c.can_advance());
        assert_eq!(c.op(), Some(PresetOp::Purge));
        assert_eq!(c.preset_name(), Some("deep-dive"));
    }

    /// 两屏存在的唯一理由：没勾过不许进第二屏。
    #[test]
    fn cannot_reach_the_danger_button_without_acknowledging() {
        let mut c = open(PresetOp::Purge);
        assert!(!c.advance());
        assert_eq!(c.step(), 1);

        c.toggle_ack();
        assert!(c.can_advance());
        assert!(c.advance());
        assert_eq!(c.step(), 2);
    }

    /// 第一屏就算有人硬点也拿不到动作 —— `take` 自己再挡一道。
    #[test]
    fn take_returns_nothing_before_the_second_screen() {
        let mut c = open(PresetOp::Purge);
        c.toggle_ack();
        assert_eq!(c.take(), None);
        c.advance();
        assert_eq!(
            c.take(),
            Some((
                PresetOp::Purge,
                "Session-Template".to_string(),
                "deep-dive".to_string()
            ))
        );
        // 只交一次。
        assert_eq!(c.take(), None);
        assert!(!c.is_open());
    }

    #[test]
    fn back_keeps_the_acknowledgement_but_resets_step() {
        let mut c = open(PresetOp::Archive);
        c.toggle_ack();
        c.advance();
        c.back();
        assert_eq!(c.step(), 1);
        assert!(c.acked(), "退回第一屏不该让人重新勾一遍");
        assert!(c.can_advance());
    }

    #[test]
    fn esc_closes_and_produces_nothing() {
        let mut c = open(PresetOp::Purge);
        c.toggle_ack();
        c.advance();
        c.on_esc();
        assert!(!c.is_open());
        assert_eq!(c.take(), None);
    }

    /// 换一条重新请求：必须回到第一屏、勾选清掉 —— 上一个弹层勾的和这个不是一个东西。
    #[test]
    fn requesting_another_preset_resets_to_step_one() {
        let mut c = open(PresetOp::Purge);
        c.toggle_ack();
        c.advance();
        c.request(PresetOp::Archive, "Ultimate-Template", "weekly");
        assert_eq!(c.step(), 1);
        assert!(!c.acked());
        assert_eq!(c.preset_name(), Some("weekly"));
    }

    /// 文案：彻底删要说「不可逆 + archives 副本一起没」，归档要说「能还原」。
    #[test]
    fn copy_says_what_it_actually_does() {
        assert_eq!(title(PresetOp::Purge), "彻底删掉这条模板？");
        assert_eq!(confirm_label(PresetOp::Purge), "彻底删掉");
        assert!(body(PresetOp::Purge).contains("只能靠 git 找回"));
        assert!(ack_text(PresetOp::Purge).contains("不能撤销"));

        assert_eq!(confirm_label(PresetOp::Archive), "归档");
        assert!(body(PresetOp::Archive).contains("随时能还原"));
        assert!(ack_text(PresetOp::Archive).contains("找回来"));
    }

    #[test]
    fn purge_is_the_only_irreversible_one() {
        assert!(PresetOp::Purge.irreversible());
        assert!(!PresetOp::Archive.irreversible());
        assert!(!PresetOp::Restore.irreversible());
    }
}
