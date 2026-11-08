//! 应用主体。三栏：图标条 / 会话列表 / 聊天区。
//!
//! 微信桌面版的骨头。聊天区的流式（SSE）还没接，先做历史气泡。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use nous_engine::resolve::resolve_engine;
use nous_engine::types::{Entry, Session};
use nous_engine::EngineClient;

use crate::config::Config;

// 微信桌面版的几个固定色。
const SIDEBAR_DARK: u32 = 0x2e2e2e;
const LIST_BG: u32 = 0xf7f7f7;
const CHAT_BG: u32 = 0xededed;
const BUBBLE_ME: u32 = 0x95ec69;
const BUBBLE_OTHER: u32 = 0xffffff;
const MUTED: u32 = 0x999999;

pub struct Desktop {
    engine: Option<Arc<EngineClient>>,
    sessions: Vec<Session>,
    selected: Option<i64>,
    entries: Vec<Entry>,
    entries_loading: bool,
    entries_error: Option<String>,
    error: Option<String>,
    loading: bool,
}

impl Desktop {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            engine: None,
            sessions: vec![],
            selected: None,
            entries: vec![],
            entries_loading: false,
            entries_error: None,
            error: None,
            loading: true,
        };
        this.bootstrap(cx);
        this
    }

    fn bootstrap(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let outcome = match handle {
                Some(h) => match h.spawn(load_engine()).await {
                    Ok(Ok(v)) => Ok(v),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(format!("任务挂了：{e}")),
                },
                None => Err("tokio 没起".to_string()),
            };

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match outcome {
                    Ok((engine, sessions)) => {
                        this.engine = Some(engine);
                        this.sessions = sessions;
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select_session(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.selected == Some(id) {
            return;
        }
        self.selected = Some(id);
        self.entries.clear();
        self.entries_error = None;
        self.entries_loading = true;
        cx.notify();

        let engine = self.engine.clone();
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result: Option<Result<nous_engine::types::Entries, nous_engine::EngineError>> =
                match (engine, handle) {
                    (Some(e), Some(h)) => h.spawn(async move { e.entries(id).await }).await.ok(),
                    _ => None,
                };
            let _ = this.update(cx, |this, cx| {
                this.entries_loading = false;
                match result {
                    Some(Ok(entries)) => {
                        // 引擎给的新的在前，聊天区要旧的在上。
                        this.entries = entries.items.into_iter().rev().collect();
                    }
                    Some(Err(e)) => {
                        this.entries = vec![];
                        this.entries_error = Some(e.to_string());
                    }
                    None => this.entries_error = Some("任务没跑完".to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── 三个栏 ──────────────────────────────────────────────

    fn render_icon_bar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        // 微信桌面版最左的深色窄条。图标还没画，先放占位。
        div()
            .flex()
            .flex_col()
            .w(px(60.))
            .h_full()
            .bg(rgb(SIDEBAR_DARK))
            .items_center()
            .py(px(8.))
            .child(div().size(px(32.)).rounded(px(4.)).bg(rgb(0x444444)))
    }

    fn render_session_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<AnyElement> = self
            .sessions
            .iter()
            .map(|s| self.session_row(s, cx).into_any_element())
            .collect();
        div()
            .flex()
            .flex_col()
            .w(px(280.))
            .h_full()
            .bg(rgb(LIST_BG))
            .child(div().p(px(12.)).child("Nous"))
            .child(div().px(px(12.)).child(self.status_line(cx)))
            .child(div().flex().flex_col().flex_1().overflow_hidden().children(rows))
    }

    fn render_chat(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(id) = self.selected else {
            return div()
                .flex()
                .flex_1()
                .size_full()
                .items_center()
                .justify_center()
                .child("选个会话，这里开始聊")
                .into_any_element();
        };

        let title = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("会话 {id}"));

        let bubbles: Vec<AnyElement> = if self.entries_loading {
            vec![div().p(px(12.)).text_size(px(12.)).child("拉历史中…").into_any_element()]
        } else if let Some(e) = &self.entries_error {
            vec![div()
                .p(px(12.))
                .text_size(px(12.))
                .text_color(rgb(0xd32f2f))
                .child(format!("拉历史失败：{e}"))
                .into_any_element()]
        } else if self.entries.is_empty() {
            vec![div()
                .p(px(12.))
                .text_size(px(12.))
                .child("还没消息，先从模板开一条。")
                .into_any_element()]
        } else {
            self.entries
                .iter()
                .map(|e| self.bubble(e, cx).into_any_element())
                .collect()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .bg(rgb(CHAT_BG))
            .child(
                div()
                    .flex()
                    .h(px(48.))
                    .w_full()
                    .items_center()
                    .px(px(12.))
                    .bg(rgb(LIST_BG))
                    .child(title),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .py(px(8.))
                    .children(bubbles),
            )
            .child(
                // 输入框占位：真正的输入下一轮接。
                div()
                    .flex()
                    .h(px(64.))
                    .w_full()
                    .items_center()
                    .px(px(12.))
                    .bg(rgb(LIST_BG))
                    .child(div().text_size(px(12.)).child("输入框还没写")),
            )
            .into_any_element()
    }

    // ── 列表 / 气泡 ─────────────────────────────────────────

    fn status_line(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return div().text_size(px(12.)).child("连引擎中…");
        }
        if let Some(e) = &self.error {
            return div()
                .text_size(px(12.))
                .text_color(rgb(0xd32f2f))
                .child(format!("连不上：{e}"));
        }
        div()
            .text_size(px(12.))
            .child(format!("{} 个会话", self.sessions.len()))
    }

    fn session_row(&self, s: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = s
            .last
            .as_ref()
            .and_then(|l| l.text.clone())
            .unwrap_or_default();
        let selected = self.selected == Some(s.id);
        let id = s.id;

        div()
            .flex()
            .flex_col()
            .p(px(8.))
            .gap(px(2.))
            .border_b(px(1.))
            .border_color(rgb(0xe0e0e0))
            .when(selected, |el| el.bg(rgb(0xd9d9d9)))
            .child(div().child(s.name.clone()))
            .when(!preview.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .child(preview),
                )
            })
            // `.id()` 把 Div 转成 Stateful<Div>，on_click 只存在于后者。
            .id(ElementId::Name(format!("sess-{id}").into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.select_session(id, cx);
            }))
    }

    fn bubble(&self, e: &Entry, _cx: &mut Context<Self>) -> impl IntoElement {
        let text = e.text.clone().unwrap_or_default();
        match e.kind.as_str() {
            "pi.user" => div()
                .flex()
                .justify_end()
                .px(px(12.))
                .py(px(4.))
                .child(
                    div()
                        .max_w(px(480.))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(BUBBLE_ME))
                        .child(text),
                ),
            "pi.assistant" => div()
                .flex()
                .justify_start()
                .px(px(12.))
                .py(px(4.))
                .child(
                    div()
                        .max_w(px(480.))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(BUBBLE_OTHER))
                        .child(text),
                ),
            _ => div()
                .flex()
                .justify_center()
                .px(px(12.))
                .py(px(4.))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .child(text),
                ),
        }
    }
}

/// 连引擎：读配置 → resolve 地址 → 拉会话列表。全在 tokio 上。
async fn load_engine() -> Result<(Arc<EngineClient>, Vec<Session>), String> {
    let cfg = Config::load()?;
    let url = resolve_engine(&cfg.host, &cfg.board, "h58ga5", "qrd3wh")
        .await
        .map_err(|e| e.to_string())?;
    let engine = EngineClient::new(url.as_str(), Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let sessions = engine
        .sessions()
        .await
        .map_err(|e| e.to_string())?
        .items;
    Ok((Arc::new(engine), sessions))
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(CHAT_BG))
            .child(self.render_icon_bar(cx))
            .child(self.render_session_list(cx))
            .child(self.render_chat(cx))
    }
}
