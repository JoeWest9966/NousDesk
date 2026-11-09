//! 模板页（通讯录）。从模板开新会话是客户端唯一「从零起步」的路。
//!
//! 判据全部来自引擎（`wire.md`）：`specError` 红、`resolved.*.state == "invalid"`
//! 开不了、`tracked === false` 盘上有仓里没。客户端不自己拼规则。

use gpui_kit::component::{Icon, Sizable as _, Size};
// 完整的 lucide 目录在 assets 里（component 的 IconName 只是一小撮兼容子集）。
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use nous_engine::types::{CreatedSession, CreateSession, Preset, Presets};
use nous_engine::EngineError;

use crate::app::{Bar, Desktop, Tab};

/// `wire.md` 写死的 section：模板（通讯录）那页的数据。
const TEMPLATE_SECTION: &str = "Session-Template";

/// 状态行的小字色。
const MUTED: u32 = 0x999999;
const BAD: u32 = 0xd32f2f;
/// 能开的行：微信通讯录那种「头像方块」的底色。
const AVATAR_BG: u32 = 0x7ba7d7;

impl Desktop {
    /// 切左栏入口。切到模板页时顺手拉一次数据（拉过就不重拉）。
    pub(crate) fn switch_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        if tab == Tab::Templates {
            self.load_presets(false, cx);
        }
        cx.notify();
    }

    /// 拉 `GET /v1/presets/Session-Template`。已拉到且非 force 就不动。
    pub(crate) fn load_presets(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.presets_loading {
            return;
        }
        if !force && self.presets.is_some() {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.presets_error = Some("还没连上引擎".into());
            cx.notify();
            return;
        };
        self.presets_loading = true;
        self.presets_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result: Option<Result<Presets, EngineError>> = match &handle {
                Some(h) => h
                    .spawn(async move { engine.presets(TEMPLATE_SECTION).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.presets_loading = false;
                match result {
                    Some(Ok(p)) => this.presets = Some(p),
                    Some(Err(e)) => this.presets_error = Some(e.to_string()),
                    None => this.presets_error = Some("任务没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 从模板开一条会话：`POST /v1/sessions { preset: "Session-Template/<name>", name }`。
    /// 成功后重拉会话列表、选中新会话、切回聊天页。
    fn launch_preset(&mut self, preset_name: String, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else { return };
        // 防连点：开一条的时候别的行先不动。
        if self.creating.is_some() {
            return;
        }
        self.creating = Some(preset_name.clone());
        cx.notify();

        // wire.md：name 带时间，避免重名。
        let name = format!(
            "{} · {}",
            preset_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() % 86400)
                .unwrap_or(0)
        );
        let body = CreateSession {
            preset: Some(format!("{TEMPLATE_SECTION}/{preset_name}")),
            name: Some(name),
            ..Default::default()
        };

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            // create → 紧跟一次 sessions()，一次任务里带回落库后的列表。
            let outcome: Option<
                Result<(CreatedSession, Vec<nous_engine::types::Session>), EngineError>,
            > = match &handle {
                Some(h) => h
                    .spawn(async move {
                        let created = engine.create(&body).await?;
                        let sessions = engine.sessions().await?.items;
                        Ok((created, sessions))
                    })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.creating = None;
                match outcome {
                    Some(Ok((created, sessions))) => {
                        let mut sessions = sessions;
                        sessions.sort_by(|a, b| b.id.cmp(&a.id));
                        this.sessions = sessions;
                        this.presets_error = None;
                        this.tab = Tab::Chats;
                        this.select_session(created.id, true, cx);
                    }
                    Some(Err(e)) => this.presets_error = Some(format!("开不了：{e}")),
                    None => this.presets_error = Some("任务没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── 渲染 ────────────────────────────────────────────────

    /// 第二栏在模板 Tab 下的整块内容。
    pub(crate) fn render_templates(&self, cx: &mut Context<Self>) -> AnyElement {
        let body: Vec<AnyElement> = if self.presets_loading {
            vec![note("拉模板中…")]
        } else if let Some(e) = &self.presets_error {
            vec![
                note(&format!("拉模板失败：{e}")),
                // 失败给个重试入口。
                div()
                    .id(ElementId::Name("presets-retry".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.load_presets(true, cx);
                    }))
                    .px(px(12.))
                    .py(px(4.))
                    .text_size(px(12.))
                    .text_color(rgb(0x3a6ea5))
                    .child("重试")
                    .into_any_element(),
            ]
        } else {
            match &self.presets {
                None => vec![note("还没数据")],
                Some(p) => {
                    if p.presets.is_empty() {
                        vec![note("引擎里还没有模板")]
                    } else {
                        p.presets
                            .iter()
                            .map(|preset| self.preset_row(preset, cx).into_any_element())
                            .collect()
                    }
                }
            }
        };

        let creating_note = self
            .creating
            .as_ref()
            .map(|name| note(&format!("正在开「{name}」…")));

        div()
            .flex()
            .flex_col()
            .w(px(280.))
            .h_full()
            .bg(rgb(super::app::LIST_BG))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .p(px(12.))
                    .child("通讯录")
                    .text_size(px(15.)),
            )
            .child(div().px(px(12.)).pb(px(4.)).child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "{} 个模板",
                        self.presets.as_ref().map_or(0, |p| p.presets.len())
                    )),
            ))
            .child(
                div()
                    .id("presets-scroll-area")
                    .relative()
                    .flex_1()
                    // 和左栏/聊天区同一条规矩：flex 项不写 `min_h(0)` 就会被内容撑爆，
                    // 撑爆了就不需要滚，滚动条也就永远不出现。
                    .min_h(px(0.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.bar_hover(Bar::Presets, *hovered, cx)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .id(ElementId::Name("presets-scroll".into()))
                            .overflow_y_scroll()
                            .track_scroll(&self.presets_scroll)
                            .children(creating_note)
                            .children(body),
                    )
                    .child(crate::app::scrollbar::overlay(
                        "presets-scrollbar",
                        &self.presets_scroll,
                        self.presets_bar,
                        self.presets_hover,
                    )),
            )
            .into_any_element()
    }

    fn preset_row(&self, p: &Preset, cx: &mut Context<Self>) -> impl IntoElement {
        let ok = launchable(p);
        let name = p.name.clone();
        let creating = self.creating.as_deref() == Some(p.name.as_str());

        // 状态字：判据全是引擎算好的，客户端只挑颜色。
        let status: Option<(&str, u32)> = if p.spec_error.is_some() {
            Some(("spec 写坏了", BAD))
        } else if has_invalid(p) {
            Some(("引用失效，现在开不了", BAD))
        } else if !p.tracked {
            Some(("盘上有、仓里没", MUTED))
        } else {
            None
        };

        div()
            .flex()
            .gap(px(10.))
            .p(px(10.))
            .border_b(px(1.))
            .border_color(rgb(0xe0e0e0))
            // 行永远是 Stateful：id 挂住，能开才接 on_click。
            .id(ElementId::Name(format!("preset-{}", p.name).into()))
            .when(ok, |el| {
                el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.launch_preset(name.clone(), cx);
                }))
            })
            .when(ok, |el| el.hover(|h| h.bg(rgb(0xececec))))
            .when(creating, |el| el.bg(rgb(0xd9e8f7)))
            .child(
                // 头像方块：能开的用蓝底白字首字，开不了的置灰。
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded(px(6.))
                    .bg(rgb(if ok { AVATAR_BG } else { 0xcccccc }))
                    .text_color(rgb(if ok { 0xffffff } else { 0x888888 }))
                    .text_size(px(16.))
                    .child(p.name.chars().next().unwrap_or('?').to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .overflow_hidden()
                    .child(div().child(p.name.clone()))
                    .when(!p.summary.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(MUTED))
                                .max_w(px(180.))
                                .child(p.summary.clone()),
                        )
                    })
                    .children(
                        status
                            .map(|(text, color)| {
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(color))
                                    .child(text)
                                    .into_any_element()
                            })
                            .into_iter()
                            .collect::<Vec<_>>(),
                    ),
            )
            // 能开的行右侧画个「发起聊天」的箭头，像微信通讯录的入口。
            .when(ok, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_1()
                        .justify_end()
                        .items_center()
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .with_size(Size::Size(px(16.)))
                                .text_color(rgb(MUTED)),
                        ),
                )
            })
    }
}

/// 能开 = spec 没坏 && 没有失效引用。tracked 只影响显示，不影响可开。
fn launchable(p: &Preset) -> bool {
    p.spec_error.is_none() && !has_invalid(p)
}

/// `resolved` 里任何一项 `state == "invalid"` 就开不了。
fn has_invalid(p: &Preset) -> bool {
    p.resolved
        .as_ref()
        .and_then(|v| v.as_object())
        .map(|m| {
            m.values()
                .any(|x| x.get("state").and_then(|s| s.as_str()) == Some("invalid"))
        })
        .unwrap_or(false)
}

fn note(text: &str) -> AnyElement {
    div()
        .p(px(12.))
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.to_string())
        .into_any_element()
}
