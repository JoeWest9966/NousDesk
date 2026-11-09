//! 应用主体。三栏：图标条 / 会话列表 / 聊天区。
//!
//! 微信桌面版的骨头。聊天区的流式（SSE）还没接，先做历史气泡。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::{Icon, Sizable as _, Size};
// 完整的 lucide 目录在 assets 里（component 的 IconName 只是一小撮兼容子集）。
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use smol::Timer;
use nous_engine::resolve::resolve_engine;
use nous_engine::types::{Entry, Session};
use nous_engine::pins::PinClient;
use nous_engine::EngineClient;

use crate::config::{Config, ENGINE_BLOCK, MOBILE_BLOCK, SELF_BLOCK};
use crate::live::Live;

// 微信桌面版的几个固定色。
const SIDEBAR_DARK: u32 = 0x2e2e2e;
pub(crate) const LIST_BG: u32 = 0xf7f7f7;
const CHAT_BG: u32 = 0xededed;
const BUBBLE_ME: u32 = 0x95ec69;
const BUBBLE_OTHER: u32 = 0xffffff;
pub(crate) const MUTED: u32 = 0x999999;
/// 微信的绿：左栏选中图标就是这个颜色。
pub(crate) const WECHAT_GREEN: u32 = 0x07c160;

actions!(nous_desktop, [TogglePinAction]);

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
    /// **这一轮生成失败**（模型 400、task_failed、没答上来）：画在聊天区底部，
    /// 带「重试」。跟 `error`（连引擎失败、带「改哪个配置文件」提示）不是一条通道 ——
    /// 把「模型订阅过期」说成「连不上 + 去改配置」会把用户带到沟里（真踩过）。
    turn_error: Option<String>,
    loading: bool,
    input: Entity<TextareaState>,
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
    /// 左栏是不是「分支视图」。默认 false = 全部主线会话。
    pub(crate) branches: bool,
    /// 分支视图的根。进视图时锁住：点分支里的会话只换右侧，列表不跟着塌。
    pub(crate) branch_of: Option<i64>,
    /// 分支视图里展开着的会话。纯内存，重启重置（它不是事实，是「我眼下看到哪」）。
    pub(crate) expanded: std::collections::HashSet<i64>,
    /// 这一轮是**用户自己按的停止**。abort 会让流断开，那不是错误，别弹红条。
    user_stopped: bool,
    /// 手机端那块（bjes6b）的客户端：**只用来读写置顶**。
    /// `None` = 那个块没部署 / 连不上 —— 这时不显示置顶功能，绝不弹错。
    pins_client: Option<Arc<PinClient>>,
    /// 置顶的会话 id，**顺序就是显示顺序**（来自 `pins.json`）。
    pins: Vec<i64>,
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
        // 多行：1~8 行自动长高，超过就在框里滚。回车发送、Shift+回车换行。
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 8)
                // 回车=发送，不在这里插换行。少了这句，多行框里的回车会**先插一个换行**
                // 再发出去：文本被清空时看不出来，但 send() 提前返回时会留一个空行。
                .submit_on_enter(true)
                .placeholder("输入消息，回车发送 · Shift+回车换行")
        });
        let subs = vec![
            cx.observe_window_activation(window, |this, window, _cx| {
                // 在后台就别每 3 秒拉一次。
                this.focused = window.is_window_active();
            }),
            cx.subscribe_in(&input, window, {
            move |this, _, event: &InputEvent, window, cx| {
                match event {
                    // 内容变了要重画，否则「发送」按钮永远是灰的。
                    InputEvent::Change => cx.notify(),
                    InputEvent::PressEnter { shift, .. } => {
                        if !*shift {
                            this.send(window, cx);
                        }
                    }
                    _ => {}
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
            turn_error: None,
            loading: true,
            input,
            live: None,
            live_state: Live::new(),
            streaming: false,
            scroll: ScrollHandle::new(),
            tab: Tab::Chats,
            focused: true,
            stream_for: None,
            branches: false,
            branch_of: None,
            expanded: std::collections::HashSet::new(),
            user_stopped: false,
            pins_client: None,
            pins: vec![],
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
                        // 置顶单独去 bjes6b 拿（单源在它那儿）。
                        this.load_pins(cx);
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 连 bjes6b 拿置顶。**失败就静默降级**：`pins_client` 留 `None`，
    /// 界面上没有置顶功能，但不影响聊天 —— 客户端不该因为别人的块没装就红一片。
    fn load_pins(&mut self, cx: &mut Context<Self>) {
        let Some(host) = pins_host() else { return };
        let board = pins_board();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let url = match handle
                .spawn(async move {
                    nous_engine::resolve::resolve_engine(&host, &board, SELF_BLOCK, MOBILE_BLOCK).await
                })
                .await
            {
                Ok(Ok(url)) => url,
                Ok(Err(e)) => {
                    log::info!("手机端那块 resolve 不到，置顶功能关掉：{e}");
                    return;
                }
                Err(e) => {
                    log::info!("resolve 任务挂了，置顶功能关掉：{e}");
                    return;
                }
            };
            let client = match PinClient::new(url.as_str(), Duration::from_secs(8)) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    log::warn!("置顶客户端建不起来：{e}");
                    return;
                }
            };
            let ids = {
                let c = client.clone();
                handle.spawn(async move { c.pinned_ids().await }).await
            };
            let _ = this.update(cx, |this, cx| match ids {
                Ok(Ok(ids)) => {
                    log::info!("置顶读到了：{ids:?}");
                    this.pins_client = Some(client);
                    this.pins = ids;
                    cx.notify();
                }
                Ok(Err(e)) => log::info!("置顶读不到，功能关掉：{e}"),
                Err(e) => log::info!("置顶任务挂了，功能关掉：{e}"),
            });
        })
        .detach();
    }

    /// 置顶 / 取消置顶。**乐观更新**：界面先动，写失败再退回去（手机端同一套语义）。
    fn toggle_pin(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(client) = self.pins_client.clone() else {
            return;
        };
        let want = !self.pins.contains(&id);
        let before = self.pins.clone();
        if want {
            self.pins.retain(|x| *x != id);
            self.pins.insert(0, id); // 新置顶的排最前 —— 和微信一致
        } else {
            self.pins.retain(|x| *x != id);
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let res = handle
                .spawn(async move { client.set_pin(id, want).await })
                .await;
            let failed = match res {
                Ok(Ok(())) => {
                    log::info!("置顶写进去了：{id} -> {want}");
                    false
                }
                Ok(Err(e)) => {
                    log::warn!("置顶没写进去：{id} -> {want}：{e}");
                    true
                }
                Err(e) => {
                    log::warn!("置顶任务挂了：{id}：{e}");
                    true
                }
            };
            if failed {
                // 回滚：绝不显示「已置顶」然后其实没存上。
                let _ = this.update(cx, |this, cx| {
                    this.pins = before;
                    cx.notify();
                });
            }
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
        self.turn_error = None;
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
    ///
    /// 返回「这一句到底有没有发出去」：`retry_last` 靠它决定红条留不留。
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // 生成中**不发**：回车这时只想接着写，不该把半句话塞进队列。
        // 引擎的 `whenBusy` 值域还没确认，先不传 —— 确认了只需要在这一个入口加分支。
        if self.streaming {
            log::info!("正在生成，这一句不发（回车被拦下）");
            return false;
        }
        let Some(id) = self.selected else { return false };
        let Some(engine) = self.engine.clone() else { return false };
        let text = self.input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return false;
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
        // **不换 Live**：常连的合帧循环靠 `Arc::ptr_eq` 认身份，换掉会让循环退场。
        self.live_state.reset_round();
        self.streaming = true;
        self.turn_error = None;
        self.user_stopped = false;
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
        true
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
        cx.spawn(async move |this, cx| {
        // 看门狗：这一轮最后一条「有新东西」的时刻。
        let mut last_change = std::time::Instant::now();
        // 收到过「此刻没在生成」的快照。上游报错时失败事件可能早于我们订阅，
        // 之后这条流只剩一个空 snapshot —— 不设上限就会永远停在「停止」。
        let mut idle_snapshot = false;
        loop {
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
                    // 换过会话的旧循环：立刻退场，别去改新会话的状态。
                    if !this.live_state.same_round(&live) {
                        log::info!("旧的合帧循环退场");
                        return false;
                    }
                    let Some(drained) = drained else {
                        // 没有新事件：把「永远等下去」变成一句能重试的话。
                        if this.streaming {
                            let limit = if idle_snapshot {
                                Duration::from_secs(8)
                            } else {
                                Duration::from_secs(30)
                            };
                            if last_change.elapsed() > limit {
                                this.live = None;
                                this.streaming = false;
                                if !this.user_stopped {
                                    let why = if idle_snapshot {
                                        "这一轮没有跑起来（引擎那边没有正在生成的消息）"
                                    } else {
                                        "30 秒没有任何输出"
                                    };
                                    log::warn!("{why}，判定本轮失败");
                                    this.turn_error = Some(format!("{why}，可以重试"));
                                }
                                cx.notify();
                            }
                        }
                        return true;
                    };
                    last_change = std::time::Instant::now();
                    if drained.idle_snapshot && drained.text.is_empty() && !drained.done {
                        idle_snapshot = true;
                    }
                    if let Some(err) = drained.error {
                        this.live = None;
                        this.streaming = false;
                        if this.user_stopped {
                            // 用户按了停止：流被掐断是**预期**，别弹红条。
                            log::info!("停止后流断开，按预期忽略：{err}");
                        } else {
                            log::warn!("生成失败：{err}");
                            this.turn_error = Some(err);
                        }
                        cx.notify();
                        return true;
                    }
                    this.live = Some(drained.text);
                    this.scroll.scroll_to_bottom();
                    if drained.done {
                        // 一轮结束：交出去，历史以引擎为准重拉一遍。流不关，继续等下一轮。
                        this.streaming = false;
                        idle_snapshot = false;
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
        }
        })
        .detach();
    }

    /// 「停止」：`POST /v1/sessions/:id/abort`，把当前这一轮掐掉。
    ///
    /// 先改 UI 再发请求（按钮立刻变回「发送」，不等网络）；掐完以**引擎历史为准**
    /// 重拉一次 —— 引擎已经落下的半截回答要显示出来。失败只记日志：停止是「少做事」，
    /// 网络抖一下没必要吓用户。
    fn stop_generation(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected else { return };
        let Some(engine) = self.engine.clone() else { return };
        if !self.streaming {
            return;
        }
        self.streaming = false;
        self.user_stopped = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            match handle.spawn(async move { engine.abort(id).await }).await {
                Ok(Ok(resp)) => log::info!("停止成功：{resp}"),
                Ok(Err(e)) => log::warn!("停止失败（不打扰用户）：{e}"),
                Err(e) => log::warn!("停止任务没跑完：{e}"),
            }
            let _ = this.update(cx, |this, cx| {
                this.live = None;
                this.select_session(id, true, cx);
            });
        })
        .detach();
    }

    /// 「重试」：把最后一条用户消息原样再发一遍（新的 `requestId`）。
    fn retry_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .entries
            .iter()
            .rev()
            .find(|e| e.kind == "pi.user")
            .and_then(|e| e.text.clone());
        match text {
            Some(text) if !text.trim().is_empty() => {
                self.input
                    .update(cx, |input, cx| input.set_value(text, window, cx));
                // **发出去了才收红条**：send() 可能被「正在生成」拦下，先清掉会让用户
                // 以为重试成功了（真踩过）。
                if self.send(window, cx) {
                    self.turn_error = None;
                } else {
                    log::warn!("重试没发出去（可能还在生成中）");
                }
                cx.notify();
            }
            _ => {
                self.turn_error = None;
                cx.notify();
            }
        }
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

    fn render_session_list(&self, cx: &mut Context<Self>) -> AnyElement {
        // 分支视图是左列的另一种内容：同一块地方，换一份名单。
        if self.branches {
            if let Some(root) = self.branch_of {
                return self.render_branch_list(root, cx);
            }
        }
        // 默认只画主线：`parent != null` 的是 fork 分支，只在分支视图里出现 ——
        // 不然同一条分支会在两个地方各出现一次。
        //
        // 置顶排序只在**主线**这一份名单里做（分支视图按树画，不掺置顶）。
        let mainline: Vec<&Session> = self.sessions.iter().filter(|s| s.parent.is_none()).collect();
        let in_order: Vec<i64> = mainline.iter().map(|s| s.id).collect();
        let rows: Vec<AnyElement> = order_ids(&in_order, &self.pins)
            .into_iter()
            .filter_map(|id| mainline.iter().find(|s| s.id == id))
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
            .into_any_element()
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
            // `min_w(0)` 不能省：flex 项默认 `min-width: auto`，历史里只要有一条
            // 折不开的长行（工具输出整段），这块就被撑得比窗口还宽，
            // 顶部 `justify_between` 的右半边全跑到窗口外面（症状：按钮"有时候不见了"）。
            .min_w(px(0.))
            .h_full()
            .bg(rgb(CHAT_BG))
            .child(
                div()
                    .flex()
                    .h(px(48.))
                    .w_full()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .bg(rgb(LIST_BG))
                    .child(div().flex_none().truncate().child(title))
                    // 分支：分叉树唯一的入口。激活时变绿 —— 绿 = 左列现在看到的是分支。
                    .child(self.branch_button(id, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .id("chat-scroll")
                    .min_w(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .py(px(8.))
                    .children(bubbles),
            )
            // 这一轮的失败就地显示：红条 + 重试 + 收起。不占左栏那条「连不上引擎」的通道。
            //
            // 按钮**左锚**、文字放最后：GPUI 里长文字（不换行）按内容宽度算，
            // 谁在它右边谁就被顶出屏幕（试过，右端按钮整颗消失）。
            .when_some(self.turn_error.clone(), |el, message| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .w_full()
                        .px(px(12.))
                        .py(px(8.))
                        .bg(rgb(0xfff1f0))
                        .border_t(px(1.))
                        .border_color(rgb(0xf3c9c6))
                        .child(
                            div()
                                .id("turn-error-retry")
                                .flex_shrink_0()
                                .px(px(10.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .bg(rgb(0xc0392b))
                                .text_size(px(12.))
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.retry_last(window, cx)
                                }))
                                .child("重 试"),
                        )
                        .child(
                            div()
                                .id("turn-error-dismiss")
                                .flex_shrink_0()
                                .px(px(6.))
                                .py(px(4.))
                                .text_size(px(12.))
                                .text_color(rgb(0x8a6d6a))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.turn_error = None;
                                    cx.notify();
                                }))
                                .child("收起"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(px(12.))
                                .text_color(rgb(0xc0392b))
                                .child(short_text(&message, 90)),
                        ),
                )
            })
            .child(crate::composer::render(
                &self.input,
                crate::composer::State {
                    streaming: self.streaming,
                    can_send: !self.input.read(cx).value().trim().is_empty(),
                },
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.send(window, cx);
                }),
                cx.listener(|this, _: &ClickEvent, _window, cx| this.stop_generation(cx)),
            ))
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
            .child(format!(
                "{} 个会话",
                self.sessions.iter().filter(|s| s.parent.is_none()).count()
            ))
    }

    fn session_row(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        // 预览是最后一条 entry 的原文，带换行/连续空白是常态（工具输出整段）。
        // 摊平成一行，配合 `.truncate()` 才是微信那种单行摘要。
        let preview = s
            .last
            .as_ref()
            .and_then(|l| l.text.as_deref())
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        let selected = self.selected == Some(s.id);
        // `busy` 是 `"queued" | "running" | <task kind> | null`，不是布尔（`wire.md`）。
        let busy = s.busy.is_some();
        let id = s.id;
        let pinned = self.pins.contains(&id);
        let can_pin = self.pins_client.is_some();

        let row = div()
            .flex()
            .flex_col()
            .justify_center()
            // 固定下限：没有预览的行（新会话、只有工具输出的）也跟别人一样高，列表才成一列节奏。
            .min_h(px(56.))
            .px(px(10.))
            .py(px(6.))
            .gap(px(2.))
            .border_b(px(1.))
            .border_color(rgb(0xe0e0e0))
            .when(selected, |el| el.bg(rgb(0xd9d9d9)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    // 📌 用 lucide 的 Pin（AllAssets 里有）
                    .when(pinned, |el| {
                        el.child(
                            Icon::new(IconName::Pin)
                                .text_color(rgb(0x2e7d32))
                                .size(px(13.)),
                        )
                    })
                    .child(div().truncate().child(s.name.clone())),
            )
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
                        .truncate()
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .child(preview),
                )
            })
            // 有分支就在右下角挂条数：这是「进分支视图」唯一的线索，别省。
            .when(self.descendant_count(id) > 0, |el| {
                el.child(
                    div()
                        .absolute()
                        .right(px(8.))
                        .bottom(px(8.))
                        .flex_none()
                        .px(px(6.))
                        .rounded(px(8.))
                        .bg(rgb(0xe4f8ea))
                        .text_size(px(11.))
                        .text_color(rgb(WECHAT_GREEN))
                        .child(format!("{} 条分支", self.descendant_count(id))),
                )
            })
            // `.id()` 把 Div 转成 Stateful<Div>，on_click 只存在于后者。
            .id(ElementId::Name(format!("sess-{id}").into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.select_session(id, false, cx);
            }));

        // 右键菜单：置顶 / 取消置顶。菜单项发的是同一个 action，每行自己的 listener
        // 捕获自己的 id，所以 action 不用带参数。
        //
        // 只能在链尾分支：`context_menu()` 返回的是 `ContextMenu<Stateful<Div>>`，
        // 换个类型就没法再串 `when`（类型对不上）。
        if can_pin {
            row.on_action(cx.listener(move |this, _: &TogglePinAction, _window, cx| {
                this.toggle_pin(id, cx);
            }))
            .context_menu(move |menu, _window, _cx| {
                let label = if pinned { "取消置顶" } else { "置顶该聊天" };
                menu.menu(label, Box::new(TogglePinAction))
            })
            .into_any_element()
        } else {
            row.into_any_element()
        }
    }

    /// 聊天区顶部那颗「⑂ 分支」。
    ///
    /// 点一下：左列从「全部会话」切成「这条会话的分支」；再点一下切回去。
    /// 没有分支也照样画（灰的），点进去是空状态「还没有分支」——
    /// 按钮时有时无会让人以为功能坏了。
    fn branch_button(&self, id: i64, cx: &mut Context<Self>) -> AnyElement {
        // 数字说的是**当前这个视图**有几条分支：开了分支视图就报根的数字，
        // 不然点到一条末端分支时角标会变成 0，看着像「视图空了」。
        let n = match (self.branches, self.branch_of) {
            (true, Some(root)) => self.descendant_count(root),
            _ => self.descendant_count(id),
        };
        let on = self.branches;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(5.))
            .px(px(10.))
            .h(px(28.))
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(if on { WECHAT_GREEN } else { 0xd0d0d0 }))
            .bg(rgb(if on { WECHAT_GREEN } else { 0xffffff }))
            .text_size(px(13.))
            .text_color(rgb(if on { 0xffffff } else { 0x333333 }))
            .id(ElementId::Name("branch-toggle".into()))
            .cursor_pointer()
            .hover(|el| el.bg(rgb(if on { 0x06a854 } else { 0xf2f2f2 })))
            .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                if this.branches {
                    this.exit_branches(cx);
                } else {
                    this.enter_branches(this.selected.unwrap_or(id), cx);
                }
            }))
            .child(
                Icon::new(IconName::GitBranch)
                    .with_size(Size::Size(px(15.)))
                    .text_color(rgb(if on { 0xffffff } else { 0x555555 })),
            )
            .child("分支")
            .children(if n > 0 {
                Some(
                    div()
                        .flex_none()
                        .px(px(5.))
                        .rounded(px(8.))
                        .bg(rgb(if on { 0x0aa85a } else { 0xededed }))
                        .text_size(px(11.))
                        .child(format!("{n}")),
                )
            } else {
                None
            })
            .into_any_element()
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

/// 左栏顺序：**置顶的在前**（顺序照 `pins` 给的），其余保持原来那套顺序。
///
/// 纯函数（只吃 id），所以能直接拿数字做单测 —— 排序这种事写错一次能烦一星期。
fn order_ids(in_order: &[i64], pins: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::with_capacity(in_order.len());
    for id in pins {
        if in_order.contains(id) && !out.contains(id) {
            out.push(*id);
        }
    }
    for id in in_order {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

/// 读配置拿「宿主地址 / 盘名」，供置顶那次 resolve 用。读不到就用默认值。
fn pins_host() -> Option<String> {
    Config::load().ok().map(|c| c.host)
}

fn pins_board() -> String {
    Config::load()
        .map(|c| c.board)
        .unwrap_or_else(|_| Config::default().board)
}

/// 错误原文可能很长（引擎把上游 400 的整段 JSON 递上来）。红条只给人看前 90 个字，
/// 全文在 `%LOCALAPPDATA%` 下的 `nous/desktop.log` 里。
fn short_text(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if out.chars().count() < text.chars().count() {
        out.push('…');
    }
    out
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
    sessions.sort_by(|a, b| activity_of(b).cmp(&activity_of(a)).then_with(|| b.id.cmp(&a.id)));
}

/// 一行的「长相」。轮询时只比这个，不变就不 `cx.notify()`。
/// 活跃度：最后一条 entry 的 id。没有 entry 的新会话退化成会话 id。
pub(crate) fn activity_of(s: &Session) -> i64 {
    s.last.as_ref().and_then(|l| l.id.as_i64()).unwrap_or(s.id)
}

/// 相对年龄（`5 分钟前` / `3 天前`）。`updatedAt` 为 null 就返回 None —— **不编时间**。
pub(crate) fn ago(updated_at: Option<i64>) -> Option<String> {
    let ts = updated_at?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as i64;
    let mins = (now - ts) / 60_000;
    Some(match mins {
        m if m < 1 => "刚刚".to_string(),
        m if m < 60 => format!("{m} 分钟前"),
        m if m < 60 * 24 => format!("{} 小时前", m / 60),
        m => format!("{} 天前", m / 1440),
    })
}

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

// 排序写错一次能烦一星期，所以拿数字直接钉住。
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::order_ids;

    #[test]
    fn 置顶的排前面_且按置顶顺序() {
        // 原顺序 3,2,1；置顶顺序 1,3（新置顶的在最前）
        assert_eq!(order_ids(&[3, 2, 1], &[1, 3]), vec![1, 3, 2]);
    }

    #[test]
    fn 置顶里已经不存在的会话被忽略() {
        assert_eq!(order_ids(&[5, 4], &[99, 4]), vec![4, 5]);
    }

    #[test]
    fn 没有置顶就保持原顺序() {
        assert_eq!(order_ids(&[3, 2, 1], &[]), vec![3, 2, 1]);
    }

    #[test]
    fn 重复置顶不会重复出现() {
        assert_eq!(order_ids(&[1, 2], &[2, 2]), vec![2, 1]);
    }
}
