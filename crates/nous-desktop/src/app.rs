//! 应用主体。三栏：图标条 / 会话列表 / 聊天区。
//!
//! 微信桌面版的骨头。聊天区的流式（SSE）还没接，先做历史气泡。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _, Size};
// 完整的 lucide 目录在 assets 里（component 的 IconName 只是一小撮兼容子集）。
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use smol::Timer;
use nous_engine::resolve::resolve_engine;
use nous_engine::types::{Entry, Session};
use nous_engine::EngineClient;

use crate::config::{Config, ENGINE_BLOCK, SELF_BLOCK};
use crate::live::Live;

// 微信桌面版的几个固定色。
const SIDEBAR_DARK: u32 = 0x2e2e2e;
pub(crate) const LIST_BG: u32 = 0xf7f7f7;
const CHAT_BG: u32 = 0xededed;
const BUBBLE_ME: u32 = 0x95ec69;
const BUBBLE_OTHER: u32 = 0xffffff;
const MUTED: u32 = 0x999999;
/// 微信的绿：左栏选中图标就是这个颜色。
const WECHAT_GREEN: u32 = 0x07c160;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Chats,
    Templates,
}

pub struct Desktop {
    pub(crate) engine: Option<Arc<EngineClient>>,
    pub(crate) sessions: Vec<Session>,
    pub(crate) selected: Option<i64>,
    entries: Vec<Entry>,
    entries_loading: bool,
    entries_error: Option<String>,
    error: Option<String>,
    loading: bool,
    input: Entity<InputState>,
    /// 正在生成的那条助手消息（流式）。
    live: Option<String>,
    /// 共享给 tokio 任务的流式状态。
    live_state: Live,
    /// 有没有一条流正在跑，防止重复点火。
    streaming: bool,
    /// 聊天区的滚动位置：新消息进来要跟着到底。
    scroll: ScrollHandle,
    /// 左栏当前页：聊天 / 通讯录（模板）。
    pub(crate) tab: Tab,
    /// 窗口在不在前台。轮询只在聚焦时跑，后台一律不动。
    focused: bool,
    /// 当前常连着 SSE 的会话。`None` = 没连。切会话就换一个，不叠加。
    stream_for: Option<i64>,
    /// 模板页数据（`GET /v1/presets/Session-Template`）。
    pub(crate) presets: Option<nous_engine::types::Presets>,
    pub(crate) presets_error: Option<String>,
    pub(crate) presets_loading: bool,
    /// 正在从模板开的那条（防连点 + 状态提示）。
    pub(crate) creating: Option<String>,
    _subs: Vec<Subscription>,
}

impl Desktop {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入消息，回车发送"));
        let subs = vec![
            cx.observe_window_activation(window, |this, window, _cx| {
                // 在后台就别每 3 秒拉一次。
                this.focused = window.is_window_active();
            }),
            cx.subscribe_in(&input, window, {
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { shift, .. } = event {
                    if !*shift {
                        this.send(window, cx);
                    }
                }
            }
        })];

        let mut this = Self {
            engine: None,
            sessions: vec![],
            selected: None,
            entries: vec![],
            entries_loading: false,
            entries_error: None,
            error: None,
            loading: true,
            input,
            live: None,
            live_state: Live::new(),
            streaming: false,
            scroll: ScrollHandle::new(),
            tab: Tab::Chats,
            focused: true,
            stream_for: None,
            presets: None,
            presets_error: None,
            presets_loading: false,
            creating: None,
            _subs: subs,
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
                        // 列表是别人的地盘：定时重读，否则只在启动那一秒新鲜。
                        this.start_poll(cx);
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 定时重读 `/v1/sessions`。聚焦才拉、流式中不拉。
    ///
    /// 内容没变就不 `cx.notify()` —— 每 3 秒重绘一次窗口是白烧 CPU。
    fn start_poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(3)).await;
                let go = this
                    .update(cx, |this, _| this.focused && !this.streaming && this.engine.is_some())
                    .unwrap_or(false);
                if !go {
                    continue;
                }
                let (engine, selected) = match this
                    .update(cx, |this, _| (this.engine.clone(), this.selected))
                {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let Some(engine) = engine else { continue };
                let Some(h) = crate::TOKIO.get().cloned() else { break };
                let Ok(Ok(list)) = h.spawn(async move { engine.sessions().await }).await else {
                    // 断一下就等下一轮，不打断 UI。
                    continue;
                };

                let mut next = list.items;
                sort_sessions(&mut next);
                let _ = this.update(cx, |this, cx| {
                    // 选中的那条被别人写了：重拉它的历史。
                    let stale_entries = selected.is_some_and(|id| {
                        let before = this.sessions.iter().find(|s| s.id == id).map(sig_of);
                        let after = next.iter().find(|s| s.id == id).map(sig_of);
                        before != after
                    });
                    if this.sessions.iter().map(sig_of).ne(next.iter().map(sig_of)) {
                        this.sessions = next;
                        // 日志里能对上：「列表怎么不动了」先看这行还在不在跳；
                        // 「怎么没跳到最上面」就看置顶那条和水位。
                        log::info!(
                            "列表刷新：{} 行，置顶「{}」（last.id={:?}, busy={:?}）",
                            this.sessions.len(),
                            this.sessions.first().map(|s| s.name.as_str()).unwrap_or("-"),
                            this.sessions.first().and_then(|s| sig_of(s).2),
                            this.sessions.first().map(|s| s.busy.as_ref().map(|b| b.to_string())).unwrap_or(None),
                        );
                        cx.notify();
                    }
                    if stale_entries && let Some(id) = selected {
                        this.select_session(id, true, cx);
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn select_session(&mut self, id: i64, force: bool, cx: &mut Context<Self>) {
        if self.selected == Some(id) && !force {
            return;
        }
        self.selected = Some(id);
        self.entries.clear();
        self.entries_error = None;
        self.entries_loading = true;
        self.streaming = false;
        // 选中就常连：别的写出方（另一个终端、面板页）进这一条，这边会自己长出来。
        self.start_stream(cx);
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


    /// 发一条。SSE 走的是 `requestId` 幂等锚：重发不会答两遍。
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else { return };
        let Some(engine) = self.engine.clone() else { return };
        let text = self.input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }

        // 幂等锚：同一句话重发拿到同一次提交。
        let request_id = format!(
            "desk-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            text.len()
        );

        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });

        // 自己先当听众：先在本地插一条用户气泡，再开流。
        self.entries.push(nous_engine::types::Entry {
            id: serde_json::Value::Null,
            kind: "pi.user".into(),
            text: Some(text.clone()),
            parent: None,
            extra: Default::default(),
        });
        self.live = Some(String::new());
        self.live_state = Live::new();
        self.streaming = true;
        cx.notify();

        let live = self.live_state.clone();
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            if let Some(h) = handle {
                // 发消息：不传 waitMs，立即回 202，正文靠 SSE。
                let _ = h
                    .spawn(async move {
                        if let Err(e) = engine.send_message(id, &text, &request_id).await {
                            live.fail(e.to_string());
                        }
                    })
                    .await;
            }
            let _ = this.update(cx, |_this, cx| cx.notify());
        })
        .detach();

        self.start_stream(cx);
    }

    /// 连 SSE。**选中就一直连着**，断了 1s 重连；切会话才换。
    ///
    /// 两半：tokio 任务只往 `live_state` 里塞，UI 侧 16ms 取一次。
    fn start_stream(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected else { return };
        let Some(engine) = self.engine.clone() else { return };
        if self.stream_for == Some(id) {
            return;
        }
        self.stream_for = Some(id);
        self.live = None;
        self.live_state = Live::new();
        let live = self.live_state.clone();

        // 读流：断了就重连，直到这个会话不再被选中。
        cx.spawn(async move |this, cx| {
            loop {
                let still = this
                    .update(cx, |this, _| this.stream_for == Some(id))
                    .unwrap_or(false);
                if !still {
                    break;
                }
                let Some(h) = crate::TOKIO.get().cloned() else { break };
                let engine = engine.clone();
                let live = live.clone();
                let _ = h
                    .spawn(async move {
                        use futures_util::StreamExt as _;
                        match engine.events(id).await {
                            Ok(mut stream) => {
                                log::info!("SSE 连上：会话 {id}");
                                while let Some(item) = stream.next().await {
                                    match item {
                                        Ok(event) => live.ingest(&event),
                                        Err(e) => {
                                            log::warn!("SSE 中断：{id}：{e}");
                                            live.fail(e.to_string());
                                            break;
                                        }
                                    }
                                }
                                log::info!("SSE 结束：会话 {id}");
                            }
                            Err(e) => {
                                log::warn!("SSE 连不上：{id}：{e}");
                                live.fail(e.to_string())
                            }
                        }
                    })
                    .await;
                // 断线不刷屏：等一秒重连（已切走就在下一圈退出）。
                Timer::after(Duration::from_millis(1000)).await;
            }
        })
        .detach();

        // UI 侧 16ms 一帧：把攒下的字写进气泡。
        let live = self.live_state.clone();
        cx.spawn(async move |this, cx| loop {
            Timer::after(Duration::from_millis(16)).await;
            let alive = this
                .update(cx, |this, _| this.stream_for == Some(id))
                .unwrap_or(false);
            if !alive {
                break;
            }
            let drained = live.drain();
            let mut reload = false;
            let cont = this
                .update(cx, |this, cx| {
                    let Some(drained) = drained else {
                        return true;
                    };
                    if let Some(err) = drained.error {
                        this.error = Some(format!("生成失败：{err}"));
                        this.live = None;
                        this.streaming = false;
                        cx.notify();
                        return true;
                    }
                    this.live = Some(drained.text);
                    this.scroll.scroll_to_bottom();
                    if drained.done {
                        // 一轮结束：交出去，历史以引擎为准重拉一遍。流不关，继续等下一轮。
                        this.streaming = false;
                        reload = true;
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !cont {
                break;
            }
            if reload {
                let _ = this.update(cx, |this, cx| {
                    this.live = None;
                    this.select_session(id, true, cx);
                });
            }
        })
        .detach();
    }


    // ── 三个栏 ──────────────────────────────────────────────

    fn render_icon_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // 微信桌面版最左的深色窄条：聊天 / 通讯录两个入口。
        div()
            .flex()
            .flex_col()
            .w(px(60.))
            .h_full()
            .bg(rgb(SIDEBAR_DARK))
            .items_center()
            .py(px(10.))
            .gap(px(6.))
            .child(self.tab_button(IconName::MessageCircle, Tab::Chats, cx))
            .child(self.tab_button(IconName::BookUser, Tab::Templates, cx))
    }

    fn tab_button(
        &self,
        name: IconName,
        tab: Tab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.tab == tab;
        let id = match tab {
            Tab::Chats => "tab-chats",
            Tab::Templates => "tab-templates",
        };
        div()
            .id(ElementId::Name(id.into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.switch_tab(tab, cx);
            }))
            .flex()
            .items_center()
            .justify_center()
            .size(px(40.))
            .rounded(px(8.))
            .hover(|el| el.bg(rgb(0x3a3a3a)))
            .child(
                Icon::new(name)
                    .with_size(Size::Size(px(22.)))
                    .text_color(rgb(if active { WECHAT_GREEN } else { 0x9a9a9a })),
            )
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
            let mut out: Vec<AnyElement> = self
                .entries
                .iter()
                // 空正文的气泡不画 —— 引擎对「有 entry 没内容」会给空串/null，
                // 画出来就是一个白方块（坑.md 第 7 条）。
                .filter(|e| {
                    e.text
                        .as_deref()
                        .map(|t| !t.trim().is_empty())
                        .unwrap_or(false)
                })
                .map(|e| self.bubble(e, cx).into_any_element())
                .collect();
            // 正在生成的那条：跟助手气泡同一个样子，只是文字还在长。
            if let Some(live) = &self.live {
                if !live.is_empty() {
                    out.push(
                        div()
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
                                    .child(live.clone()),
                            )
                            .into_any_element(),
                    );
                }
            }
            out
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
                    .id("chat-scroll")
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .py(px(8.))
                    .children(bubbles),
            )
            .child(
                div()
                    .flex()
                    .h(px(72.))
                    .w_full()
                    .items_center()
                    .px(px(12.))
                    .bg(rgb(LIST_BG))
                    .child(div().flex_1().child(Input::new(&self.input))),
            )
            .into_any_element()
    }

    // ── 列表 / 气泡 ─────────────────────────────────────────

    fn status_line(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return div().text_size(px(12.)).child("连引擎中…");
        }
        if let Some(e) = &self.error {
            // 连不上就告诉用户「改哪个文件」，别让他猜。
            // 没找到配置时把**找过的地方**全列出来 —— 只说「没找到」等于让人干瞪眼。
            let hint = match Config::path() {
                Some(p) => format!("配置文件：{}", p.display()),
                None => format!(
                    "没找到配置文件，找过：{}",
                    Config::candidates()
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(" · ")
                ),
            };
            return div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .text_size(px(12.))
                .text_color(rgb(0xd32f2f))
                .child(format!("连不上：{e}"))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(0x777777))
                        .child(hint),
                );
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
        // `busy` 是 `"queued" | "running" | <task kind> | null`，不是布尔（`wire.md`）。
        let busy = s.busy.is_some();
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
            // 忙闲：预览行换成微信那句「正在输入…」+ 一个小圆点。
            // 比一个「忙」字可读，也不用新图标。
            .when(busy, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(div().size(px(6.)).rounded(px(3.)).bg(rgb(WECHAT_GREEN)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(WECHAT_GREEN))
                                .child("正在输入…"),
                        ),
                )
            })
            .when(!busy && !preview.is_empty(), |el| {
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
                this.select_session(id, false, cx);
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
    let url = resolve_engine(&cfg.host, &cfg.board, SELF_BLOCK, ENGINE_BLOCK)
        .await
        .map_err(|e| e.to_string())?;
    let engine = EngineClient::new(url.as_str(), Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let sessions = engine
        .sessions()
        .await
        .map_err(|e| e.to_string())?
        .items;
    let mut sessions = sessions;
    sort_sessions(&mut sessions);
    Ok((Arc::new(engine), sessions))
}

/// 会话排序：按 `last.id` 倒序 = 最后说话的那条在最上面。
///
/// `EntryId` 是全局单调的（durable 的 `conversation-tails` 就靠这条），实测也成立：
/// session 465 → last.id 483、session 14 → 458、session 2 → 411。所以排序**不需要时间**。
///
/// 一条 entry 都没有的新会话没有 `last`，退化成按会话 id 排（两个序列都随时间增），
/// 保证刚建的那条落在最上面。
fn sort_sessions(sessions: &mut [Session]) {
    sessions.sort_by(|a, b| {
        let ka = a.last.as_ref().and_then(|l| l.id.as_i64()).unwrap_or(a.id);
        let kb = b.last.as_ref().and_then(|l| l.id.as_i64()).unwrap_or(b.id);
        kb.cmp(&ka).then_with(|| b.id.cmp(&a.id))
    });
}

/// 一行的「长相」。轮询时只比这个，不变就不 `cx.notify()`。
fn sig_of(s: &Session) -> (i64, String, Option<i64>, Option<String>, bool) {
    (
        s.id,
        s.name.clone(),
        s.last.as_ref().and_then(|l| l.id.as_i64()),
        s.last.as_ref().and_then(|l| l.text.clone()),
        s.busy.is_some(),
    )
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(CHAT_BG))
            .child(self.render_icon_bar(cx))
            .child(match self.tab {
                Tab::Chats => self.render_session_list(cx).into_any_element(),
                Tab::Templates => self.render_templates(cx),
            })
            .child(self.render_chat(cx))
    }
}
