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
// 可选中文本。`div().child(String)` 是死的 StyledText —— 鼠标拖不动，也就复制不出来。
use gpui_kit::component::text::TextView;
use gpui_kit::prelude::FluentBuilder as _;
// `on_hover` / `on_click` 这些不是 Div 自带的方法，是这条 trait 给的，得显式拿进来。
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use smol::Timer;
use nous_engine::resolve::resolve_engine;
use nous_engine::pins::PinClient;
use nous_engine::types::{Entry, Session, Unread};
use nous_engine::EngineClient;

pub(crate) mod scrollbar;

/// 哪块面板的滚动条。加新面板就往这儿加一个分支。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bar {
    Chat,
    List,
    Presets,
}

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
    /// 左栏会话列表的滚动句柄（列表以前是 `overflow_hidden`，压根不能滚）。
    list_scroll: ScrollHandle,
    /// 模板页（通讯录）的滚动句柄。
    pub(crate) presets_scroll: ScrollHandle,
    /// 每个面板两位：鼠标在不在上面 / 滚动条在不在场（淡出那 250ms 里也得在场）。
    chat_hover: bool,
    chat_bar: bool,
    list_hover: bool,
    list_bar: bool,
    pub(crate) presets_hover: bool,
    pub(crate) presets_bar: bool,
    /// 左栏当前页：聊天 / 通讯录（模板）。
    pub(crate) tab: Tab,
    /// 窗口在不在前台。轮询只在聚焦时跑，后台一律不动。
    focused: bool,
    /// 当前常连着 SSE 的会话。`None` = 没连。切会话就换一个，不叠加。
    stream_for: Option<i64>,
    /// 展开了的工具卡片，按 entry id 记。**纯内存**：它不是事实，是「我眼下想看哪块」。
    pub(crate) open_tools: std::collections::HashSet<String>,
    /// 正在生成的这一轮里调过的工具。只有实时流里有名字（历史里的 `pi.tool-result`
    /// 只存输出，见 `坑.md`）。
    pub(crate) live_tools: Vec<crate::live::ToolRun>,
    /// 每个会话的未读条数和水位。引擎按 EntryId 数好的，这边只画。
    pub(crate) unread: Unread,
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
    /// 「新对话」盘面。盖在三栏之上的一整页（外形照 qrd3wh 的 hb7k3j 页）。
    pub(crate) newchat: crate::board::NewChat,
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
            list_scroll: ScrollHandle::new(),
            presets_scroll: ScrollHandle::new(),
            chat_hover: false,
            chat_bar: false,
            list_hover: false,
            list_bar: false,
            presets_hover: false,
            presets_bar: false,
            tab: Tab::Chats,
            focused: true,
            stream_for: None,
            open_tools: std::collections::HashSet::new(),
            live_tools: vec![],
            unread: Unread::default(),
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
            newchat: crate::board::NewChat::new(window, cx),
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
                        // 调试用：`NOUS_OPEN_BOARD=1` 一启动就开在新对话盘面上，
                        // 省得每次手点。只是开关，不是功能。
                        if std::env::var("NOUS_OPEN_BOARD").is_ok() {
                            log::info!("NOUS_OPEN_BOARD：直接开在新对话盘面上");
                            this.newchat.open = true;
                            this.board_load(cx);
                            // 再给一个梯子：直接开在某个弹层上，省得每次手点。
                            match std::env::var("NOUS_OPEN_SHEET").as_deref() {
                                Ok("model") => {
                                    this.newchat.sheet = Some(crate::board::Sheet::Model)
                                }
                                Ok("skills") => {
                                    this.newchat.sheet = Some(crate::board::Sheet::Skills)
                                }
                                Ok("prompt") => {
                                    this.newchat.sheet = Some(crate::board::Sheet::Prompt)
                                }
                                Ok("dir") => this.newchat.sheet = Some(crate::board::Sheet::Dir),
                                Ok("machine") => {
                                    this.newchat.sheet = Some(crate::board::Sheet::Machine)
                                }
                                _ => {}
                            }
                        }
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
                // 会话和未读一起拉：两个都是小 JSON，分两次反而多一次 RTT。
                let got = h
                    .spawn(async move {
                        let sessions = engine.sessions().await;
                        let unread = engine.unread().await;
                        (sessions, unread)
                    })
                    .await;
                let Ok((sessions, unread)) = got else { continue };
                let Ok(list) = sessions else {
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
                    // 未读变了也要重画（红点上不上、消不消）。
                    if let Ok(u) = unread
                        && this.unread != u
                    {
                        this.unread = u;
                        cx.notify();
                    }
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
                    // 人正停在某个会话上：新进来的也算读过，红点不该一直挂着。
                    if let Some(id) = this.selected {
                        this.mark_read(id, cx);
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
        self.live_tools.clear();
        self.open_tools.clear();
        // 选中就常连：别的写出方（另一个终端、面板页）进这一条，这边会自己长出来。
        self.start_stream(cx);
        self.mark_read(id, cx);
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


    /// 「我读到这儿了」。有未读才推，别每 3 秒空推一次水位。
    ///
    /// 回执里带着引擎算好的 `unread: 0`，直接抄回表里 —— 水位的真相在服务端，
    /// 客户端不自己减。
    fn mark_read(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.unread.count(id) == 0 {
            return;
        }
        let Some(engine) = self.engine.clone() else { return };
        let Some(h) = crate::TOKIO.get().cloned() else { return };
        log::info!("标已读：会话 {id}（{} 条未读）", self.unread.count(id));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(r)) = h.spawn(async move { engine.mark_read(id).await }).await {
                let _ = this.update(cx, |this, cx| {
                    this.unread.apply(&r);
                    cx.notify();
                });
            }
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
        self.live_tools.clear();
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
                    // 工具卡片：这一轮调过哪些工具，SSE 里是带名字的（`ToolRun.name`）。
                    // 以前 `Drained.tools` 没人看，所以 `live.rs` 里那两个字段一直是死代码。
                    this.live_tools = drained.tools;
                    this.scroll.scroll_to_bottom();
                    if drained.done {
                        // 一轮结束：交出去，历史以引擎为准重拉一遍。流不关，继续等下一轮。
                        this.streaming = false;
                        idle_snapshot = false;
                        reload = true;
                        // 转成历史之前先收起来：历史版会按引擎的说法重画一遍。
                        this.live_tools.clear();
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

    /// 悬停 → 「滚动条在不在场」。
    ///
    /// 移开不能马上撤元素：撤了就看不到那 250ms 的淡出，会「啪」一下消失。
    /// 所以先留着，等淡出跑完再收 —— 期间要是又进来了，就当成淡入继续用。
    pub(crate) fn bar_hover(&mut self, which: Bar, hovered: bool, cx: &mut Context<Self>) {
        let (hover, shown) = match which {
            Bar::Chat => (&mut self.chat_hover, &mut self.chat_bar),
            Bar::List => (&mut self.list_hover, &mut self.list_bar),
            Bar::Presets => (&mut self.presets_hover, &mut self.presets_bar),
        };
        if *hover == hovered {
            return;
        }
        *hover = hovered;
        *shown = true;
        cx.notify();
        if hovered {
            return;
        }
        cx.spawn(async move |this, cx| {
            Timer::after(scrollbar::FADE).await;
            this.update(cx, |this, cx| {
                let still_hovered = match which {
                    Bar::Chat => this.chat_hover,
                    Bar::List => this.list_hover,
                    Bar::Presets => this.presets_hover,
                };
                if still_hovered {
                    return;
                }
                match which {
                    Bar::Chat => this.chat_bar = false,
                    Bar::List => this.list_bar = false,
                    Bar::Presets => this.presets_bar = false,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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
            .child(
                div()
                    .p(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child("Nous")
                    // 建会话的唯一入口。以前只能从模板起，现在多一条「从零配」的路。
                    .child(
                        div()
                            .id("new-chat-btn")
                            .px(px(8.))
                            .py(px(3.))
                            .rounded(px(4.))
                            .text_size(px(12.))
                            .text_color(rgb(WECHAT_GREEN))
                            .hover(|s| s.bg(rgb(0xe8f8ee)))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.board_open(window, cx);
                            }))
                            .child("+ 新对话"),
                    ),
            )
            .child(div().px(px(12.)).child(self.status_line(cx)))
            .child(
                div()
                    .id("session-scroll-area")
                    .relative()
                    .flex_1()
                    // `min_h(0)`：flex 项默认 `min-height: auto`，不加这行这个「视口」
                    // 会被内容撑到真实几万像素高，永远不需要滚（滚动条也就不会出现）。
                    .min_h(px(0.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.bar_hover(Bar::List, *hovered, cx)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .id("session-scroll")
                            .overflow_y_scroll()
                            .track_scroll(&self.list_scroll)
                            .children(rows),
                    )
                    .child(scrollbar::overlay(
                        "session-scrollbar",
                        &self.list_scroll,
                        self.list_bar,
                        self.list_hover,
                    )),
            )
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
            // 正在生成这一轮调的工具：SSE 里带名字，所以是「bash · hostname; pwd」。
            for t in &self.live_tools {
                out.push(self.tool_card(&format!("live-{}", t.id), Some(&t.name), &t.output, cx));
            }
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
                    // `on_hover` 挂在 `Stateful` 上，所以这里先给个 id。
                    .id("chat-scroll-area")
                    .relative()
                    .flex_1()
                    // 鼠标在这块内容上 → 滚动条出现（demo 的 3 号：平时一条线都没有）。
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.bar_hover(Bar::Chat, *hovered, cx)
                    }))
                    .min_w(px(0.))
                    // `min_h(0)` 是上面那条 `min_w(0)` 的竖版：不加它，这个「视口」
                    // 会被内容撑到十几万像素高，内容就不需要滚了，滚动条也不再出现。
                    .min_h(px(0.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .id("chat-scroll")
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .py(px(8.))
                            .children(bubbles),
                    )
                    .child(scrollbar::overlay(
                        "chat-scrollbar",
                        &self.scroll,
                        self.chat_bar,
                        self.chat_hover,
                    )),
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
            // 一行里三样东西：📌 / 名字 / 分支角标 / 时间。左半边 `flex_1` 占满并 truncate，
            // 右半边 `flex_none` 顶到右边 —— 谁都不会被谁顶出去（比 absolute 好使，也不会重叠）。
            .child(
                div()
                    .flex()
                    .w_full()
                    .items_center()
                    .gap(px(6.))
                    // 📌 用 lucide 的 Pin（AllAssets 里有）
                    .when(pinned, |el| {
                        el.child(
                            Icon::new(IconName::Pin)
                                .text_color(rgb(0x2e7d32))
                                .size(px(13.)),
                        )
                    })
                    .child(div().flex_1().min_w(px(0.)).truncate().child(s.name.clone()))
                    // 别用 `⑂`：雅黑里没这个字形，会退化成别的样子。
                    .when(self.descendant_count(id) > 0, |el| {
                        el.child(
                            div()
                                .flex_none()
                                .text_size(px(11.))
                                .text_color(rgb(WECHAT_GREEN))
                                .child(format!("{} 分支", self.descendant_count(id))),
                        )
                    })
                    .children(when_text(s.updated_at).map(|t| {
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(t)
                    })),
            )
            // 第二行只有一个出口：左边是「正在输入…」或预览（`flex_1` + truncate），
            // 右边是未读红点（`flex_none`）。
            //
            // 早先拆成两个分支、红点挂在 `flex_col` 上，结果红点被拉伸成整条红带
            // —— 列容器里加子元素就是铺满宽度，红点得装在一个 `justify_end` 的横排里。
            .child(
                div()
                    .flex()
                    .w_full()
                    .items_center()
                    .gap(px(6.))
                    .child(if busy {
                        // 忙闲：微信那句「正在输入…」+ 一个小圆点。比一个「忙」字可读。
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(div().flex_none().size(px(6.)).rounded(px(3.)).bg(rgb(WECHAT_GREEN)))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(WECHAT_GREEN))
                                    .child("正在输入…"),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(preview)
                            .into_any_element()
                    })
                    .children(self.unread_badge(id)),
            )
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

    /// 未读数红点。0 条就返回 `None`（别画个「0」在那儿）。
    ///
    /// `99+` 封顶：微信就是这么干的，条数到三位数对谁都没信息量。
    fn unread_badge(&self, id: i64) -> Option<AnyElement> {
        let n = self.unread.count(id);
        if n == 0 {
            return None;
        }
        Some(
            div()
                .flex_none()
                .min_w(px(18.))
                .h(px(18.))
                .px(px(5.))
                .rounded(px(9.))
                .bg(rgb(0xfa5151))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.))
                .text_color(rgb(0xffffff))
                .child(if n > 99 { "99+".to_string() } else { n.to_string() })
                .into_any_element(),
        )
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

    /// 工具卡片（甲案：一行细灰条，点开在**块内**滚动）。
    ///
    /// 为什么非要卡片：工具输出是**机器说的话**，混在气泡里既读不出来，也会把聊天区撑坏
    /// （一整段 `ls -l` 能撑得比窗口还宽，见 `坑.md` 16）。
    ///
    /// 标题里那个名字只有实时流给得起；历史里的 `pi.tool-result` 只有输出，
    /// 所以老会话显示「工具输出 · N 行」—— **不从输出内容反推是 bash 还是 read**，那是算命。
    fn tool_card(
        &self,
        key: &str,
        name: Option<&str>,
        raw: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = strip_harness(raw);
        let open = self.open_tools.contains(key);
        let lines = text.lines().count().max(1);
        let bytes = text.len();
        // 失败判据只用引擎自己的标记：剥完壳以 `[error]` 开头。
        let failed = text.trim_start().starts_with("[error]");
        let title = name.map(|n| n.to_string()).unwrap_or_else(|| "工具输出".to_string());
        let k = key.to_string();
        let for_copy = text.clone();

        // 外层是列（卡片头一行、正文在下面），但卡片头自己包一层 `flex_row`：
        // 直接在列里放「一行东西」，它会被拉成整行宽，甲案那股「一小条」的味道就没了
        // （跟坑.md 19 同一个病）。
        div()
            .flex()
            .flex_col()
            .items_start()
            .px(px(12.))
            .py(px(4.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(
                div()
                    .id(ElementId::Name(format!("tool-{key}").into()))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    // 不收宽度上限：长标题会被外层挤成一条缝。`max_w` 是给整条卡片的。
                    .max_w(px(520.))
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(rgb(0xe3e3e3))
                    .bg(rgb(0xffffff))
                    .cursor_pointer()
                    .hover(|el| el.bg(rgb(0xfafafa)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        let opened = this.open_tools.insert(k.clone());
                        if !opened {
                            this.open_tools.remove(&k);
                        }
                        log::info!("工具卡片 {k} -> {}", if opened { "展开" } else { "收起" });
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(if open { "▾" } else { "▸" }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .size(px(6.))
                            .rounded(px(3.))
                            .bg(rgb(if failed { 0xfa5151 } else { WECHAT_GREEN })),
                    )
                    .child(div().flex_none().text_size(px(12.5)).child(title))
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(if failed && lines == 1 {
                                "失败".to_string()
                            } else if failed {
                                format!("失败 · {lines} 行")
                            } else {
                                format!("{lines} 行 · {}", size_text(bytes))
                            }),
                    )
                    // 复制整段。代码块里划选后按 Ctrl+C 不一定会复制（键盘上下文分不到
                    // TextView 上），所以给一个明面出口 —— 长输出本来就是「整段拿走」。
                    .child(
                        div()
                            .id(ElementId::Name(format!("copy-{key}").into()))
                            .flex_none()
                            .px(px(6.))
                            .py(px(1.))
                            .rounded(px(4.))
                            .text_size(px(11.))
                            .text_color(rgb(WECHAT_GREEN))
                            .cursor_pointer()
                            .hover(|el| el.bg(rgb(0xe8f7ee)))
                            .on_click(cx.listener(move |_this, _: &ClickEvent, _w, cx| {
                                // 不拦的话这一下会连带把卡片展开/收起
                                cx.stop_propagation();
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    for_copy.clone(),
                                ));
                                log::info!("复制了工具输出：{} 字", for_copy.len());
                            }))
                            .child("复制"),
                    ),
                ),
            )
            .when(open, |el| {
                el.child(
                    div()
                        .id(ElementId::Name(format!("tool-body-{key}").into()))
                        .mt(px(2.))
                        // 宽高都用**固定值**，不用 `max_*`：`max_*` 在「内容比它大」时扛不住，
                        // 一行 487 行的代码能直接铺满整个聊天区（gpui 里 `min-width:auto`
                        // 那一套，见坑.md 16）。
                        .w(px(560.))
                        .h(px(190.))
                        .overflow_y_scroll()
                        // 不折行：等宽代码一折行缩进就全乱了，宁可让它在块里横向滚
                        .whitespace_nowrap()
                        .px(px(10.))
                        .py(px(8.))
                        .border_l_2()
                        .border_color(rgb(0xcfcfcf))
                        .bg(rgb(0xfbfbfb))
                        .font_family("Consolas")
                        .text_size(px(11.5))
                        .text_color(rgb(0x3c3c3c))
                        .child(selectable_code(&format!("tb-{key}"), &text)),
                )
            })
            .into_any_element()
    }

    fn bubble(&self, e: &Entry, cx: &mut Context<Self>) -> AnyElement {
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
                        .min_w(px(0.))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(BUBBLE_ME))
                        .child(selectable(&format!("u-{}", e.id), &text, 0x1a1a1a)),
                )
                .into_any_element(),
            "pi.assistant" => div()
                .flex()
                .justify_start()
                .px(px(12.))
                .py(px(4.))
                .child(
                    div()
                        .max_w(px(480.))
                        .min_w(px(0.))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(BUBBLE_OTHER))
                        .child(selectable(&format!("a-{}", e.id), &text, 0x1a1a1a)),
                )
                .into_any_element(),
            "pi.tool-result" => self.tool_card(&e.id.to_string(), None, &text, cx),
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
                )
                .into_any_element(),
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

/// 一段**可选中**的文本。
///
/// GPUI 里 `div().child("…")` 画出来的是死的 StyledText，鼠标拖不动 —— 整个界面
/// 一个字都复制不出来就是这个原因。`TextView` 才有选区（拖选、双击选词、Ctrl+C）。
///
/// 现在按 markdown 解析，但内容先**转义**过，所以看起来还是纯文本 ——
/// 等哪天要真渲染 markdown，把 `md_plain()` 去掉就行，组件这边一行不用改。
fn selectable(id: &str, text: &str, color: u32) -> TextView {
    TextView::markdown(ElementId::Name(format!("t-{id}").into()), md_plain(text))
        .selectable(true)
        .text_size(px(13.5))
        .text_color(rgb(color))
}

/// 一段**可选中**的等宽文本（工具输出、代码）。
///
/// 包成代码块：markdown 才不会把 `#`、`- `、`|` 这些当成标题/列表/表格去排版，
/// 空白也原样保留。围栏长度跟着内容走，输出里本身有 ``` 也不会把块切断。
fn selectable_code(id: &str, text: &str) -> TextView {
    TextView::markdown(
        ElementId::Name(format!("tc-{id}").into()),
        fenced_code(text),
    )
    .selectable(true)
    .text_size(px(11.5))
    .font_family("Consolas")
    .text_color(rgb(0x3c3c3c))
}

/// 把纯文本转义成「markdown 里看着还是纯文本」。
///
/// 只转义会被当成语法的字符。**这是临时手段** —— 真要做 markdown 渲染时，这一步整个删掉。
fn md_plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        // 行首的块级标记：`# 标题`、`> 引用`、`- ` / `* ` / `1. ` 列表
        let mut rest = line;
        loop {
            let hit = ["#", ">", "-", "*", "+"]
                .iter()
                .find(|m| rest.starts_with(&format!("{m} ")))
                .copied();
            match hit {
                Some(m) => {
                    out.push('\\');
                    out.push_str(m);
                    rest = &rest[m.len()..];
                }
                None => break,
            }
        }
        for ch in rest.chars() {
            // 行内语法：强调、代码、链接、表格、删除线
            if matches!(ch, '\\' | '*' | '_' | '`' | '[' | ']' | '|' | '~') {
                out.push('\\');
            }
            out.push(ch);
        }
    }
    out
}

/// 包成代码块（围栏比内容里最长的那串反引号还长）。
fn fenced_code(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat((longest + 1).max(3));
    format!("{fence}\n{text}\n{fence}")
}

/// 剥掉引擎给工具输出套的壳：`<harness>…</harness>`。
///
/// 壳里通常是 `[error] …` 这类标记。壳本身对用户没有信息量 —— 谁调的、调什么，
/// 卡片标题里已经说了；一眼看见的是「输出长什么样」。
fn strip_harness(raw: &str) -> String {
    let t = raw.trim();
    let t = t.strip_prefix("<harness>").unwrap_or(t);
    let t = t.strip_suffix("</harness>").unwrap_or(t);
    t.trim().to_string()
}

/// 给人看的体积：`12 B` / `1.9 KB`。
fn size_text(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

/// 列表右上角那格时间。`updatedAt` 为 null 就返回 `None` —— **不编时间**
/// （引擎没记过就是没记过，拿文件 mtime 冒充是错的）。
///
/// 微信的规矩：今天给钟点、昨天给「昨天」、一周内给几天前、今年给月日、再往前给年月日。
pub(crate) fn when_text(updated_at: Option<i64>) -> Option<String> {
    use chrono::{Datelike as _, Local, TimeZone as _};
    let t = Local.timestamp_millis_opt(updated_at?).single()?;
    let now = Local::now();
    let days = (now.date_naive() - t.date_naive()).num_days();
    Some(match days {
        0 => t.format("%H:%M").to_string(),
        1 => "昨天".to_string(),
        d if (2..7).contains(&d) => format!("{} 天前", d),
        _ if t.year() == now.year() => format!("{}月{}日", t.month(), t.day()),
        _ => format!("{}/{}/{}", t.year(), t.month(), t.day()),
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(CHAT_BG))
            .child(self.render_icon_bar(cx))
            .child(match self.tab {
                Tab::Chats => self.render_session_list(cx).into_any_element(),
                Tab::Templates => self.render_templates(cx),
            })
            .child(self.render_chat(cx));

        // 新对话是**一整页**，不是三栏里的一格：那个页面的尺寸全走 vmin、一屏不滚，
        // 塞进聊天区会把四块象限挤成一团。所以它盖在整窗上，关掉才回到三栏。
        div()
            .relative()
            .size_full()
            .child(shell)
            .children(self.newchat.open.then(|| {
                // 明确绝对定位：靠子元素的 `size_full` 撑满有时会拿不到父高，
                // 结果只有 canvas 画上去了、底下的深色没铺 —— 看着像「盘面没开」。
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(self.render_board(window, cx))
            }))
    }
}

// 排序写错一次能烦一星期，所以拿数字直接钉住。
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{order_ids, when_text};

    /// 时间列的三档：今天给钟点、昨天给「昨天」、今年更早给月日。
    /// 都拿「现在」往前推，不写死日期 —— 写死的话过一天就红。
    #[test]
    fn when_text_三档() {
        let now = chrono::Local::now();
        let today = now.timestamp_millis();
        let hhmm = when_text(Some(today)).unwrap();
        assert_eq!(hhmm.len(), 5, "今天的格式是 HH:MM，拿到 {hhmm:?}");

        let y = (now - chrono::Duration::days(1)).timestamp_millis();
        assert_eq!(when_text(Some(y)).unwrap(), "昨天");

        let five = (now - chrono::Duration::days(5)).timestamp_millis();
        assert_eq!(when_text(Some(five)).unwrap(), "5 天前");
    }

    /// `updatedAt == null` 必须给 `None`：引擎没记过就是没记过，
    /// 这格空着才对，不许拿别的时间填。
    #[test]
    fn when_text_没时间就是空() {
        assert!(when_text(None).is_none());
    }

    #[test]
    fn 工具输出剥掉_harness_壳() {
        use super::strip_harness;
        assert_eq!(
            strip_harness("<harness>
[error] boom
</harness>"),
            "[error] boom"
        );
        assert_eq!(strip_harness("j1900
/root"), "j1900
/root");
        // 只有一边的壳也剥（引擎写半截的时候别把它当正文）
        assert_eq!(strip_harness("<harness>ok"), "ok");
    }

    #[test]
    fn 体积给人看() {
        use super::size_text;
        assert_eq!(size_text(12), "12 B");
        assert_eq!(size_text(2048), "2.0 KB");
    }

    #[test]
    fn 纯文本在_markdown_里还是纯文本() {
        use super::md_plain;
        // 这几个字符不转义就会被当成语法：`**粗体**`、行首 `# `、`- `、`|` 表格
        assert_eq!(md_plain("**这条要有回复**"), r"\*\*这条要有回复\*\*");
        assert_eq!(md_plain("# 标题"), r"\# 标题");
        assert_eq!(md_plain("- 一项"), r"\- 一项");
        assert_eq!(md_plain("a|b"), r"a\|b");
        // 普通行原样
        assert_eq!(md_plain("收 到 了"), "收 到 了");
    }

    #[test]
    fn 代码围栏跟着内容变长() {
        use super::fenced_code;
        assert_eq!(fenced_code("ls"), "```
ls
```");
        // 输出里本身有三连反引号：围栏要更长，不然块会被提前切断
        assert_eq!(fenced_code("```
x"), "````
```
x
````");
        assert_eq!(fenced_code("```
x"), r"````
```
x
````");
    }

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
