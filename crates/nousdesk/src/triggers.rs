//! 触发器工作台（视图 1）：清单 / 详情 / 代码 / 新建向导。
//!
//! 数据就在同一个引擎地址（`cfg.host`）上，走 `/v1/triggers`；
//! 不再有别的服务地址，也不从别处解析。
//! 三层是一个页面的三种深度：
//!
//! - 主界面：类别树 + 状态筛选 + 卡片网格
//! - 详情：右滑面板（盖卡片区、不盖类别栏）
//! - 代码：全宽（文件树 / 编辑 / 差异 / 发布）
//! - 新建向导：另一条支线（身份 → 参数 → 骨架 → 建）
//!
//! **保存 ≠ 发布**：保存只 `PUT /file` 落盘；发布是一个同步长请求
//! （cargo check → build → index → commit → push），中间进度拿不到，别假装五步并行。

fn t() -> crate::theme::Skin { crate::theme::cur() }

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use nous_engine::triggers::{
    ArchivedTrigger, TriggerFileMeta, TriggerItem, TriggerList,
    TriggerParam, TriggerPublish,
};

use crate::app::Desktop;
use crate::capability::{Feature, Gate};

/// `Desk#13`：触发器的 10 条写端点全删（引擎侧无票），只留
/// `GET /v1/triggers` + `POST /{code}/fire` + `GET /{code}/fires` 三个真端点。
/// 这个函数是那一刀的闸门账；UI 入口已经删掉，留它是单测的钉子。
#[allow(dead_code)]
pub(crate) fn trigger_write_enabled() -> bool {
    false
}

const ARCHIVED: &str = "__archived__";
const UNCATEGORIZED: &str = "∅";

/// 三层 + 主界面。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Main,
    Detail,
    Code,
    Wizard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusFilter {
    All,
    Ok,
    Dirty,
    Unpublished,
    Broken,
}

impl StatusFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Ok => "已发布",
            Self::Dirty => "有改动",
            Self::Unpublished => "未发布",
            Self::Broken => "清单坏了",
        }
    }
}

// `Desk#13`：写端点删掉后这些字段只剩只读展示/确认层还带着，
// 不再被写入路径读；保留形状是为了不动版式。
#[allow(dead_code)]
pub(crate) struct CodeState {
    pub name: Option<String>,
    pub files: Vec<TriggerFileMeta>,
    pub path: Option<String>,
    pub text: Entity<TextareaState>,
    pub etag: String,
    /// HEAD 里那一版；差异视图跟它比。
    pub base: String,
    pub busy: bool,
    pub publish: Option<TriggerPublish>,
    pub publish_error: Option<String>,
    pub notice: Option<(String, bool)>,
    pub diff: bool,
    /// 点「发布」先出这一屏检查单，确认后真跑。
    pub preflight: bool,
    /// 发布开始时刻（算「已用 37s」）；结束后换成总用时。
    pub publish_started: Option<std::time::Instant>,
    pub publish_elapsed: Option<u64>,
    /// 差异视图里被点开的那几段折叠（按折叠块起始下标记）。
    pub diff_open: std::collections::HashSet<usize>,
    /// 只读代码视图 → 编辑（Textarea）。保存后回到只读。
    pub editing: bool,
}

/// 向导第 2 步的一个参数。文本字段用 `InputState`（中文输入法要正常），
/// 类型从闭集里挑、必填是个布尔，所以不用输入框。
pub(crate) struct ParamDraft {
    pub name: Entity<InputState>,
    pub label: Entity<InputState>,
    pub help: Entity<InputState>,
    pub default: Entity<InputState>,
    pub options: Entity<InputState>,
    pub kind: String,
    pub required: bool,
}

pub(crate) struct Wizard {
    pub step: u8,
    pub name: Entity<InputState>,
    pub summary: Entity<InputState>,
    pub category: Entity<InputState>,
    /// 第 2 步「参数声明」的主从编辑：左列表 + 右详情。
    pub params: Vec<ParamDraft>,
    pub param_sel: usize,
    pub busy: bool,
    pub err: Option<String>,
}

pub(crate) struct TriggerMenu {
    pub name: String,
    pub x: f32,
    pub y: f32,
}

/// 类别节点右键菜单（和卡片右键同一套手感）。
// `Desk#13`：写端点删掉后这些字段只剩只读展示/确认层还带着，
// 不再被写入路径读；保留形状是为了不动版式。
#[allow(dead_code)]
pub(crate) struct CatMenu {
    pub label: String,
    pub x: f32,
    pub y: f32,
}

/// 新建 / 改名共用一个弹层，`rename: true` 时是改名。
// `Desk#13`：写端点删掉后这些字段只剩只读展示/确认层还带着，
// 不再被写入路径读；保留形状是为了不动版式。
#[allow(dead_code)]
pub(crate) struct CatForm {
    pub rename: bool,
    pub label: String,
    pub input: Entity<InputState>,
    pub err: Option<String>,
}

/// 类别改动攒着，等一次提交（引擎只写盘，commit + push 要显式同步）。
#[derive(Default)]
// `Desk#13`：写端点删掉后这些字段只剩只读展示/确认层还带着，
// 不再被写入路径读；保留形状是为了不动版式。
#[allow(dead_code)]
pub(crate) struct Commit {
    pub pending: usize,
    pub what: String,
    pub busy: bool,
    pub err: Option<String>,
    pub done: bool,
}

pub(crate) struct Confirm {
    pub open: bool,
    pub name: String,
    pub input: Entity<InputState>,
    /// `true` = 这次是「彻底删除」（不可逆），文案和按钮都要换一套。
    pub purge: bool,
}

pub(crate) struct State {
    pub client: Option<Arc<nous_engine::TriggerClient>>,
    pub data: Option<TriggerList>,
    pub loading: bool,
    pub error: Option<String>,
    /// `None` = 全部；`Some(cat)` = 类别；`Some(ARCHIVED)` = 归档视图。
    pub cat: Option<String>,
    pub status: StatusFilter,
    pub search: Entity<InputState>,
    pub layer: Layer,
    pub selected: Option<String>,
    pub code: CodeState,
    pub wizard: Wizard,
    pub confirm: Confirm,
    pub busy: Option<String>,
    pub menu: Option<TriggerMenu>,
    pub cat_menu: Option<CatMenu>,
    pub cat_form: Option<CatForm>,
    pub cat_del: Option<String>,
    /// 「已归档」里点开的那一条（右侧详情）。
    pub archived_detail: Option<String>,
    /// 有没保存的改动时，用户想切过去的那个文件（等一句确认）。
    pub pending_path: Option<String>,
    #[allow(dead_code)]
    pub commit: Commit,
    pub conflict: Option<String>,
    pub scroll: ScrollHandle,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜名字 / 说明 / 类别 / 参数 / 二进制"));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("morning-review"));
        let summary = cx.new(|cx| {
            InputState::new(window, cx).placeholder("每天第一条会话时，按昨天的日志起一条复盘")
        });
        let category = cx.new(|cx| InputState::new(window, cx).placeholder("日常/复盘"));
        let code_text = cx.new(|cx| TextareaState::new(window, cx));
        let confirm_input = cx.new(|cx| InputState::new(window, cx).placeholder("手输名字"));
        Self {
            client: None,
            data: None,
            loading: false,
            error: None,
            cat: None,
            status: StatusFilter::All,
            search,
            layer: Layer::Main,
            selected: None,
            code: CodeState {
                name: None,
                files: vec![],
                path: None,
                text: code_text,
                etag: String::new(),
                base: String::new(),
                busy: false,
                publish: None,
                publish_error: None,
                notice: None,
                diff: false,
                preflight: false,
                publish_started: None,
                publish_elapsed: None,
                diff_open: std::collections::HashSet::new(),
                editing: false,
            },
            wizard: Wizard {
                step: 1,
                name,
                summary,
                category,
                params: vec![],
                param_sel: 0,
                busy: false,
                err: None,
            },
            confirm: Confirm {
                open: false,
                name: String::new(),
                input: confirm_input,
                purge: false,
            },
            busy: None,
            menu: None,
            cat_menu: None,
            cat_form: None,
            cat_del: None,
            archived_detail: None,
            pending_path: None,
            commit: Commit::default(),
            conflict: None,
            scroll: ScrollHandle::new(),
        }
    }

    fn current(&self) -> Option<&TriggerItem> {
        let name = self.selected.as_deref()?;
        self.data.as_ref()?.triggers.iter().find(|t| t.name == name)
    }

    fn dirty_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|d| d.triggers.iter().filter(|t| t.dirty && t.tracked).count())
            .unwrap_or(0)
    }

    fn unpublished_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|d| d.triggers.iter().filter(|t| !t.tracked).count())
            .unwrap_or(0)
    }
}

// ── 数据 ───────────────────────────────────────────────────────────────────

impl Desktop {
    /// 启动 / 重连时按 capabilities 决定是否拉触发器清单。
    pub(crate) fn load_triggers(&mut self, cx: &mut Context<Self>) {
        let cfg = self.conn.clone();
        let blocked = match self.feature_gate(Feature::Triggers) {
            Some(Gate::Enabled) => None,
            Some(Gate::Disabled { code, reason }) => {
                Some(crate::capability::disabled_text(code, &reason.message))
            }
            Some(Gate::Unknown) | None => Some(crate::capability::disabled_text(
                nous_engine::ReasonCode::Unknown,
                "服务端未声明 triggers",
            )),
        };
        if let Some(message) = blocked {
            self.triggers.error = Some(message);
            cx.notify();
            return;
        }
        // token 跟聊天、置顶走同一条注入：少了它，配了 token 的机器上触发器永远回 401
        // （`坑.md` 100 —— 第三个自建 client 又漏了一次）。
        let token = self.conn.resolve_token();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let client = match nous_engine::TriggerClient::with_token(
                cfg.host.as_str(),
                Duration::from_secs(30),
                token.as_deref(),
            ) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.triggers.error = Some(format!("触发器客户端建不起来：{e}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let loaded = {
                let c = client.clone();
                handle.spawn(async move { c.list().await }).await
            };
            let _ = this.update(cx, |this, cx| {
                this.triggers.client = Some(client);
                this.triggers.loading = false;
                match loaded {
                    Ok(Ok(data)) => {
                        this.triggers.data = Some(data);
                        this.triggers.error = None;
                    }
                    Ok(Err(e)) => this.triggers.error = Some(format!("拉触发器失败：{e}")),
                    Err(e) => this.triggers.error = Some(format!("任务挂了：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_trigger_list(&mut self, force: bool, cx: &mut Context<Self>) {
        if !force && (self.triggers.loading || self.triggers.data.is_some()) {
            return;
        }
        let Some(client) = self.triggers.client.clone() else {
            return;
        };
        self.triggers.loading = true;
        self.triggers.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let res = handle.spawn(async move { client.list().await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.loading = false;
                match res {
                    Ok(Ok(data)) => {
                        this.triggers.data = Some(data);
                        this.triggers.error = None;
                    }
                    Ok(Err(e)) => this.triggers.error = Some(format!("拉触发器失败：{e}")),
                    Err(e) => this.triggers.error = Some(format!("任务挂了：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn notice(&mut self, text: impl Into<String>, bad: bool, cx: &mut Context<Self>) {
        let token = self.notice.show(text, if bad { crate::app::NoticeKind::Error } else { crate::app::NoticeKind::Info });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(4))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.notice.expire(token) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    // ── 层 / 选中 ──────────────────────────────────────────

    pub(crate) fn trigger_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        self.triggers.layer = layer;
        if layer != Layer::Detail {
            self.triggers.confirm.open = false;
        }
        cx.notify();
    }

    fn select_trigger(&mut self, name: &str, cx: &mut Context<Self>) {
        log::info!("触发器：选中 {name}");
        self.triggers.selected = Some(name.to_string());
        self.triggers.layer = Layer::Detail;
        cx.notify();
    }

    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.triggers.layer = Layer::Main;
        cx.notify();
    }

    // ── 代码层 ─────────────────────────────────────────────

    // ── 归档 / 同步 ────────────────────────────────────────
    // `Desk#13`：10 条写端点已删，代码层的打开/保存/发布一并删掉；
    // 只留清单和只读详情。

    // ── 归档 / 同步 ────────────────────────────────────────

    // ── 右键菜单 / 冲突 / 键盘 ─────────────────────────────

    pub(crate) fn trigger_menu_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = self.triggers.menu.as_ref() else {
            return div().into_any_element();
        };
        let (x, y) = (menu.x, menu.y);
        let open = menu.name.clone();
        let _code = menu.name.clone();
        let _arch = menu.name.clone();
        let copy = menu.name.clone();
        div()
            .absolute()
            .size_full()
            .child(
                div()
                    .absolute()
                    .size_full()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.triggers.menu = None;
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(t().d(170.))
                    .flex()
                    .flex_col()
                    .p(t().d(4.))
                    .rounded(t().r(7.))
                    .bg(rgb(t().p.overlay))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .id("tm-open")
                            .flex()
                            .items_center()
                            .px(t().d(10.))
                            .h(t().d(30.))
                            .rounded(t().r(5.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.text))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.triggers.menu = None;
                                this.select_trigger(&open, cx);
                            }))
                            .child("打开详情"),
                    )
                    .child(
                        div()
                            .id("tm-code")
                            .flex()
                            .items_center()
                            .px(t().d(10.))
                            .h(t().d(30.))
                            .rounded(t().r(5.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.text))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.triggers.menu = None;
                                this.trigger_write_unavailable(cx);
                            }))
                            .child("改代码"),
                    )
                    .child(
                        div()
                            .id("tm-arch")
                            .flex()
                            .items_center()
                            .px(t().d(10.))
                            .h(t().d(30.))
                            .rounded(t().r(5.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.danger))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.triggers.menu = None;
                                this.trigger_write_unavailable(cx);
                            }))
                            .child("归档"),
                    )
                    .child(
                        div()
                            .id("tm-copy")
                            .flex()
                            .items_center()
                            .px(t().d(10.))
                            .h(t().d(30.))
                            .rounded(t().r(5.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.text_2))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.triggers.menu = None;
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()));
                                cx.notify();
                            }))
                            .child("复制名字"),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn trigger_conflict_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(message) = self.triggers.conflict.as_ref() else {
            return div().into_any_element();
        };
        let message = message.clone();
        let name = self.triggers.code.name.clone();
        let path = self.triggers.code.path.clone();
        div()
            .absolute()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(t().p.scrim))
            .child(
                div()
                    .w(t().d(430.))
                    .p(t().d(18.))
                    .rounded(t().r(9.))
                    .bg(rgb(t().p.overlay))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(t().fs(13.))
                            .text_color(rgb(t().p.text))
                            .child("文件被改过了"),
                    )
                    .child(
                        div()
                            .mt(t().d(8.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text_2))
                            .child(message),
                    )
                    .child(
                        div()
                            .mt(t().d(6.))
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child("你的改动还在编辑框里。重新加载会丢掉它。"),
                    )
                    .child(
                        div()
                            .mt(t().d(14.))
                            .flex()
                            .justify_end()
                            .gap(t().d(8.))
                            .child(
                                div()
                                    .id("tc-reload")
                                    .px(t().d(12.))
                                    .h(t().d(28.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.text_2))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.triggers.conflict = None;
                                        if let (Some(_n), Some(_p)) = (name.clone(), path.clone()) {
                                            this.trigger_write_unavailable(cx);
                                        } else {
                                            cx.notify();
                                        }
                                    }))
                                    .child("重新加载"),
                            )
                            .child(
                                div()
                                    .id("tc-force")
                                    .px(t().d(12.))
                                    .h(t().d(28.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .bg(rgb(t().p.accent))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.on_accent))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.triggers.conflict = None;
                                        this.trigger_write_unavailable(cx);
                                    }))
                                    .child("用我的覆盖"),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// `Desk#13` 后所有写入口的统一出口：不再发请求，只把 capabilities 的理由
    /// 说给用户。写端点已删（引擎侧无票），留着这层是为了不让按钮变成「点了没反应」。
    fn trigger_write_unavailable(&mut self, cx: &mut Context<Self>) {
        let reason = self
            .disabled_reason(Feature::Triggers)
            .unwrap_or_else(|| "写操作已下线（引擎侧无票）".to_string());
        self.notice(reason, true, cx);
    }

    pub(crate) fn trigger_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let _ = window;
        if key == "escape" {
            if self.triggers.archived_detail.is_some() {
                self.triggers.archived_detail = None;
                cx.notify();
                return true;
            }
            if self.triggers.cat_form.is_some() || self.triggers.cat_del.is_some() {
                self.triggers.cat_form = None;
                self.triggers.cat_del = None;
                cx.notify();
                return true;
            }
            if self.triggers.cat_menu.is_some() {
                self.triggers.cat_menu = None;
                cx.notify();
                return true;
            }
            if self.triggers.menu.is_some() {
                self.triggers.menu = None;
                cx.notify();
                return true;
            }
            if self.triggers.conflict.is_some() {
                self.triggers.conflict = None;
                cx.notify();
                return true;
            }
        }
        match self.triggers.layer {
            Layer::Main => match key {
                "up" | "down" => {
                    let items = self.filtered_triggers(cx);
                    if items.is_empty() {
                        return false;
                    }
                    let step = if key == "down" { 1 } else { -1 };
                    let cur = self
                        .triggers
                        .selected
                        .as_ref()
                        .and_then(|s| items.iter().position(|i| &i.name == s))
                        .map(|i| i as isize)
                        .unwrap_or(if step > 0 { -1 } else { 0 });
                    let next = (cur + step).clamp(0, items.len() as isize - 1);
                    self.triggers.selected = Some(items[next as usize].name.clone());
                    cx.notify();
                    true
                }
                "enter" => {
                    if let Some(name) = self.triggers.selected.clone() {
                        self.select_trigger(&name, cx);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
            Layer::Detail => {
                if key == "escape" {
                    self.triggers.layer = Layer::Main;
                    cx.notify();
                    true
                } else {
                    false
                }
            }
            Layer::Code => {
                if key == "escape" {
                    if self.triggers.code.editing {
                        self.triggers.code.editing = false;
                    } else {
                        self.triggers.layer = Layer::Detail;
                    }
                    cx.notify();
                    true
                } else {
                    false
                }
            }
            Layer::Wizard => {
                if key == "escape" {
                    self.triggers.layer = Layer::Main;
                    cx.notify();
                    true
                } else {
                    false
                }
            }
        }
    }

    pub(crate) fn trigger_ctrl(
        &mut self,
        key: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match key {
            "s" if self.triggers.layer == Layer::Code && self.triggers.code.editing => {
                self.trigger_write_unavailable(cx);
                true
            }
            "p" if matches!(self.triggers.layer, Layer::Code | Layer::Detail) => {
                self.trigger_write_unavailable(cx);
                true
            }
            _ => false,
        }
    }


    // ── 向导 ───────────────────────────────────────────────

    fn open_wizard(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.triggers.wizard.step = 1;
        self.triggers.wizard.err = None;
        // 每次都是一张白纸 —— 不会再预置 `at|epoch|…` 那种样例。
        self.triggers.wizard.params.clear();
        self.triggers.wizard.param_sel = 0;
        self.triggers.layer = Layer::Wizard;
        cx.notify();
    }

    fn wizard_step(&mut self, step: u8, cx: &mut Context<Self>) {
        self.triggers.wizard.step = step.clamp(1, 3);
        self.triggers.wizard.err = None;
        cx.notify();
    }

    // ── 向导第 2 步：参数声明（左列表 + 右详情）──────────

    fn wizard_param_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut input = |ph: &'static str| cx.new(|cx| InputState::new(window, cx).placeholder(ph));
        self.triggers.wizard.params.push(ParamDraft {
            name: input("参数名"),
            label: input("给人看的标签"),
            help: input("什么时候用这个参数"),
            default: input("默认值（可空）"),
            options: input("选项，逗号分隔"),
            kind: "string".to_string(),
            required: false,
        });
        self.triggers.wizard.param_sel = self.triggers.wizard.params.len() - 1;
        cx.notify();
    }

    fn wizard_param_del(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.triggers.wizard.params.len() {
            self.triggers.wizard.params.remove(idx);
        }
        let len = self.triggers.wizard.params.len();
        if self.triggers.wizard.param_sel >= len && len > 0 {
            self.triggers.wizard.param_sel = len - 1;
        }
        cx.notify();
    }

    fn wizard_param_select(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.triggers.wizard.param_sel = idx;
        cx.notify();
    }

    fn wizard_param_set_kind(&mut self, idx: usize, kind: &'static str, cx: &mut Context<Self>) {
        if let Some(d) = self.triggers.wizard.params.get_mut(idx) {
            d.kind = kind.to_string();
        }
        cx.notify();
    }

    fn wizard_param_toggle_required(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(d) = self.triggers.wizard.params.get_mut(idx) {
            d.required = !d.required;
        }
        cx.notify();
    }

}

/// 向导第 2 步的参数类型闭集（引擎认这六种）。只读展示仍要用它。
pub(crate) const PARAM_KINDS: [&str; 6] = ["epoch", "number", "string", "text", "boolean", "select"];

#[cfg(test)]
mod param_tests {
    use super::PARAM_KINDS;

    #[test]
    fn trigger_write_endpoints_are_closed() {
        assert!(!super::trigger_write_enabled(), "10 条写端点已删，不该说还能写");
    }

    #[test]
    fn kinds_match_engine_closed_set() {
        // 客户端这张表写错一个，建出来的触发器就被引擎 422 拒掉 —— 钉住它。
        assert_eq!(PARAM_KINDS, ["epoch", "number", "string", "text", "boolean", "select"]);
    }
}

fn param_json(d: &ParamDraft, cx: &Context<Desktop>) -> Option<serde_json::Value> {
    let name = d.name.read(cx).value().trim().to_string();
    if name.is_empty() {
        return None;
    }
    let label = d.label.read(cx).value().trim().to_string();
    let help = d.help.read(cx).value().trim().to_string();
    let def = d.default.read(cx).value().trim().to_string();
    let mut obj = serde_json::json!({
        "name": name,
        "type": d.kind,
        "required": d.required,
    });
    if !label.is_empty() {
        obj["label"] = serde_json::Value::String(label);
    }
    if !help.is_empty() {
        obj["help"] = serde_json::Value::String(help);
    }
    if !def.is_empty() {
        obj["default"] = match d.kind.as_str() {
            "number" => def.parse::<f64>().map(serde_json::Value::from).unwrap_or_else(|_| serde_json::Value::String(def.clone())),
            "boolean" => serde_json::Value::Bool(def == "true"),
            _ => serde_json::Value::String(def),
        };
    }
    if d.kind == "select" {
        let options: Vec<String> = d
            .options
            .read(cx)
            .value()
            .split([',', '，'])
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .collect();
        if !options.is_empty() {
            obj["options"] = serde_json::json!(options);
        }
    }
    Some(obj)
}

/// 向导第 2 步的参数声明 → 引擎认的 JSON 形状（纯函数，只读预览仍用）。
fn params_json(drafts: &[ParamDraft], cx: &Context<Desktop>) -> Vec<serde_json::Value> {
    drafts.iter().filter_map(|d| param_json(d, cx)).collect()
}

// ── 渲染 ───────────────────────────────────────────────────────────────────

fn short(text: &str, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn status_of(item: &TriggerItem) -> (&'static str, u32, u32) {
    if item.error.as_deref().is_some_and(|e| !e.is_empty()) {
        ("清单坏了", t().p.danger_soft, t().p.danger)
    } else if !item.tracked {
        ("未发布", t().p.warn_soft, t().p.warn)
    } else if item.dirty {
        ("有改动", t().p.warn_soft, t().p.warn)
    } else {
        ("已发布", t().p.accent_soft, t().p.accent_fg)
    }
}

use crate::category::{CatNode, is_under};
use crate::sync_state::{SyncFacts, Tone, sync_state};

impl Desktop {
    pub(crate) fn render_triggers(&self, cx: &mut Context<Self>) -> AnyElement {
        let layer = self.triggers.layer;
        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_w(t().d(0.))
            .bg(rgb(t().p.panel))
            .child(self.trigger_top(cx))
            .child(self.trigger_body(cx));
        if layer == Layer::Detail {
            root = root
                .child(self.trigger_scrim(cx))
                .child(self.trigger_detail(cx));
        }
        if layer == Layer::Code {
            root = root.child(self.trigger_code(cx));
        }
        if layer == Layer::Wizard {
            root = root.child(self.trigger_wizard(cx));
        }
        if self.triggers.pending_path.is_some() {
            root = root.child(self.trigger_pending_overlay(cx));
        }
        if self.triggers.archived_detail.is_some() {
            root = root
                .child(self.trigger_scrim(cx))
                .child(self.trigger_archived_detail(cx));
        }
        if self.triggers.confirm.open {
            root = root
                .child(self.trigger_confirm_scrim(cx))
                .child(self.trigger_confirm(cx));
        }
        root.into_any_element()
    }

    fn trigger_top(&self, cx: &mut Context<Self>) -> AnyElement {
        let head = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.repo.as_ref())
            .and_then(|r| r.head.clone())
            .unwrap_or_else(|| "—".into());
        let branch = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.repo.as_ref())
            .and_then(|r| r.branch.clone())
            .unwrap_or_else(|| "—".into());
        let dirty = self.triggers.dirty_count();
        let unpublished = self.triggers.unpublished_count();
        let repo_state = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.repo.as_ref());
        let (sync_text, tone) = sync_state(
            repo_state.map(|r| SyncFacts::of(r.ahead, r.behind, r.dirty)),
            dirty,
            unpublished,
        );
        let sync_color = match tone {
            Tone::Ok => t().p.accent,
            Tone::Warn => t().p.warn,
            Tone::Idle => t().p.muted,
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(10.))
            .h(t().d(56.))
            .px(t().d(20.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line))
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(26.))
                    .rounded(t().r(7.))
                    .bg(rgb(t().p.surface))
                    .child(
                        Icon::new(IconName::Bolt)
                            .size(t().d(15.))
                            .text_color(rgb(t().p.text_2)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(t().fs(13.5))
                            .text_color(rgb(t().p.text))
                            .child("trigger"),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("Nous-Trigger · {branch} · {head}")),
                    ),
            )
            .child(
                div()
                    .ml(t().d(12.))
                    .h(t().d(19.))
                    .px(t().d(6.))
                    .flex()
                    .items_center()
                    .rounded(t().r(4.))
                    .bg(rgb(t().p.surface))
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(format!(
                        "{} 个触发器",
                        self.triggers.data.as_ref().map_or(0, |d| d.triggers.len())
                    )),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("trigger-sync")
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(26.))
                    .px(t().d(9.))
                    .rounded(t().r(13.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.trigger_write_unavailable(cx);
                    }))
                    .child(
                        div()
                            .size(t().d(6.))
                            .rounded(t().r(3.))
                            .bg(rgb(sync_color)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text_2))
                            .child(if self.triggers.busy.is_some() {
                                "同步中…".to_string()
                            } else {
                                sync_text
                            }),
                    ),
            )
            .child(
                div()
                    .id("trigger-refresh")
                    .flex()
                    .items_center()
                    .gap(t().d(5.))
                    .h(t().d(24.))
                    .px(t().d(8.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.load_trigger_list(true, cx);
                    }))
                    .child(
                        Icon::new(IconName::RefreshCw)
                            .size(t().d(13.))
                            .text_color(rgb(t().p.muted)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child("重扫"),
                    ),
            )
            .child(
                div()
                    .id("trigger-new")
                    .flex()
                    .items_center()
                    .gap(t().d(5.))
                    .h(t().d(28.))
                    .px(t().d(11.))
                    .rounded(t().r(6.))
                    .bg(rgb(t().p.accent))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_wizard(window, cx);
                    }))
                    .child(
                        Icon::new(IconName::Plus)
                            .size(t().d(13.))
                            .text_color(rgb(t().p.on_accent)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.on_accent))
                            .child("新建触发器"),
                    ),
            )
            .into_any_element()
    }

    fn trigger_body(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(t().d(0.))
            .child(self.trigger_cats(cx))
            .child(self.trigger_content(cx))
            .into_any_element()
    }

    fn trigger_cats(&self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.triggers.data.as_ref();
        let all = data.map_or(0, |d| d.triggers.len());
        let archived = data.map_or(0, |d| d.archived.len());
        let uncat = data
            .map(|d| d.triggers.iter().filter(|t| t.category.is_empty()).count())
            .unwrap_or(0);
        // 树和计数走 `category.rs` —— 跟模板那边**同一套**。以前这里手搓了一个
        // `BTreeMap` 计数 + `cats.sort()`：语义一模一样，只有排序不一样（那边按词表
        // 父在子前、这边纯字母序）。同样的东西写两遍、只有一处悄悄不一样，改一处就漏一处。
        let current = self.triggers.cat.clone();
        let tree: Vec<CatNode> = data
            .map(|d| {
                crate::category::build_tree(
                    &d.categories,
                    d.triggers.iter().map(|t| t.category.as_slice()),
                )
            })
            .unwrap_or_default();
        let mut rows = vec![];
        rows.push(self.trigger_cat_row(None, "全部", all, current.is_none(), cx));
        rows.push(self.trigger_cat_row(Some(UNCATEGORIZED), "未分类", uncat, current.as_deref() == Some(UNCATEGORIZED), cx));
        for node in &tree {
                let depth = node.depth;
                let label = node.label.clone();
                let count = node.count;
                let cat = &node.path;
                let on = current.as_deref() == Some(node.path.as_str());
                let c = node.path.clone();
                let right = node.path.clone();
                rows.push(
                    div()
                        .id(ElementId::Name(format!("tcat-{cat}").into()))
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(t().d(29.))
                        .pl(t().d(14. + depth as f32 * 16.))
                        .pr(t().d(14.))
                        .cursor_pointer()
                        .when(on, |el| el.bg(rgb(t().p.hover)))
                        .hover(|h| h.bg(rgb(t().p.hover)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.triggers.cat = Some(c.clone());
                            this.triggers.layer = Layer::Main;
                            cx.notify();
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                this.triggers.cat = Some(right.clone());
                                this.triggers.layer = Layer::Main;
                                this.triggers.cat_menu = Some(CatMenu {
                                    label: right.clone(),
                                    x: ev.position.x.into(),
                                    y: ev.position.y.into(),
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .text_size(t().fs(12.))
                                .text_color(rgb(if count == 0 { t().p.muted } else { t().p.text_2 }))
                                .child(label.to_string()),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(t().fs(10.))
                                .text_color(rgb(t().p.muted))
                                .child(count.to_string()),
                        )
                        .into_any_element(),
                );
            }
        rows.push(
            div()
                .id("tcat-archived")
                .mt(t().d(8.))
                .pt(t().d(8.))
                .border_t(t().d(1.))
                .border_color(rgb(t().p.line_soft))
                .flex()
                .flex_row()
                .items_center()
                .h(t().d(34.))
                .px(t().d(14.))
                .cursor_pointer()
                .when(current.as_deref() == Some(ARCHIVED), |el| el.bg(rgb(t().p.hover)))
                .hover(|h| h.bg(rgb(t().p.hover)))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.triggers.cat = Some(ARCHIVED.to_string());
                    this.triggers.layer = Layer::Main;
                    cx.notify();
                }))
                .child(
                    Icon::new(IconName::Archive)
                        .size(t().d(13.))
                        .text_color(rgb(t().p.muted)),
                )
                .child(
                    div()
                        .ml(t().d(6.))
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.text_2))
                        .child("已归档"),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(t().fs(10.))
                        .text_color(rgb(t().p.muted))
                        .child(archived.to_string()),
                )
                .into_any_element(),
        );

        div()
            .flex()
            .flex_col()
            .w(t().d(214.))
            .flex_none()
            .h_full()
            .bg(rgb(t().p.panel_alt))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(t().d(14.))
                    .pt(t().d(14.))
                    .pb(t().d(9.))
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("类别"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{}", tree.len())),
                    ),
            )
            .child(
                div()
                    .id("trigger-cats-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .children(rows)
            )
            .child(
                div()
                    .p(t().d(11.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.muted))
                    .child("/opt/Nous-Trigger\nindex.mjs 扫出来的"),
            )
            .into_any_element()
    }

    fn trigger_cat_row(
        &self,
        value: Option<&str>,
        label: &str,
        count: usize,
        on: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = match value {
            None => "tcat-all".to_string(),
            Some(v) => format!("tcat-{v}"),
        };
        let value = value.map(str::to_owned);
        div()
            .id(ElementId::Name(id.into()))
            .flex()
            .flex_row()
            .items_center()
            .h(t().d(29.))
            .px(t().d(14.))
            .cursor_pointer()
            .when(on, |el| el.bg(rgb(t().p.hover)))
            .hover(|h| h.bg(rgb(t().p.hover)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.triggers.cat = value.clone();
                this.triggers.layer = Layer::Main;
                cx.notify();
            }))
            .child(
                div()
                    .text_size(t().fs(12.))
                    .text_color(rgb(if on { t().p.text } else { t().p.text_2 }))
                    .child(label.to_string()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(count.to_string()),
            )
            .into_any_element()
    }

    fn trigger_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let filtered = self.filtered_triggers(cx);
        let archived_view = self.triggers.cat.as_deref() == Some(ARCHIVED);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(t().d(0.))
            .child(self.trigger_filters(cx))
            .child(self.trigger_tools(cx))
            .child(if archived_view {
                self.trigger_archived_list(cx)
            } else {
                self.trigger_grid(filtered, cx)
            })
            .into_any_element()
    }

    fn filtered_triggers(&self, cx: &mut Context<Self>) -> Vec<TriggerItem> {
        let Some(data) = &self.triggers.data else {
            return vec![];
        };
        let needle = self.triggers.search.read(cx).value().trim().to_lowercase();
        data.triggers
            .iter()
            .filter(|item| {
                let status = status_of(item).0;
                let by_status = match self.triggers.status {
                    StatusFilter::All => true,
                    StatusFilter::Ok => status == "已发布",
                    StatusFilter::Dirty => status == "有改动",
                    StatusFilter::Unpublished => status == "未发布",
                    StatusFilter::Broken => status == "清单坏了",
                };
                let by_cat = match self.triggers.cat.as_deref() {
                    None | Some(ARCHIVED) => true,
                    Some(UNCATEGORIZED) => item.category.is_empty(),
                    Some(cat) => item.category.iter().any(|c| is_under(c, cat)),
                };
                // 搜名字 / 说明 / 类别之外，还要能按**参数名**和**二进制名**捞 ——
                // 「哪个触发器收 epoch」这种问题不该靠翻卡片。
                let mut hay = format!(
                    "{} {} {} {}",
                    item.name,
                    item.summary,
                    item.version,
                    item.category.join(" ")
                );
                if let Some(b) = &item.binary {
                    hay.push(' ');
                    hay.push_str(b);
                }
                for p in &item.params {
                    hay.push(' ');
                    hay.push_str(&p.name);
                    hay.push(' ');
                    hay.push_str(&p.kind);
                    if let Some(l) = &p.label {
                        hay.push(' ');
                        hay.push_str(l);
                    }
                    if let Some(h) = &p.help {
                        hay.push(' ');
                        hay.push_str(h);
                    }
                    for o in &p.options {
                        hay.push(' ');
                        hay.push_str(o);
                    }
                }
                let hay = hay.to_lowercase();
                by_status && by_cat && (needle.is_empty() || hay.contains(&needle))
            })
            .cloned()
            .collect()
    }

    fn trigger_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.triggers.data.as_ref();
        let count = |f: StatusFilter| {
            data.map(|d| {
                d.triggers
                    .iter()
                    .filter(|t| match f {
                        StatusFilter::All => true,
                        StatusFilter::Ok => status_of(t).0 == "已发布",
                        StatusFilter::Dirty => status_of(t).0 == "有改动",
                        StatusFilter::Unpublished => status_of(t).0 == "未发布",
                        StatusFilter::Broken => status_of(t).0 == "清单坏了",
                    })
                    .count()
            })
            .unwrap_or(0)
        };
        let mut children: Vec<AnyElement> = vec![];
        for (filter, dot) in [
            (StatusFilter::All, None),
            (StatusFilter::Ok, Some(t().p.accent)),
            (StatusFilter::Dirty, Some(t().p.warn)),
            (StatusFilter::Unpublished, Some(t().p.warn)),
            (StatusFilter::Broken, Some(t().p.danger)),
        ] {
            let on = self.triggers.status == filter;
            let n = count(filter);
            children.push(
                div()
                    .id(ElementId::Name(format!("tfilter-{filter:?}").into()))
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(26.))
                    .px(t().d(9.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.surface)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.triggers.status = filter;
                        cx.notify();
                    }))
                    .children(dot.map(|c| {
                        div()
                            .size(t().d(5.))
                            .rounded(t().r(2.5))
                            .bg(rgb(c))
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(if on { t().p.text } else { t().p.muted }))
                            .child(filter.label()),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(n.to_string()),
                    )
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(2.))
            .h(t().d(44.))
            .px(t().d(20.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .children(children)
            .into_any_element()
    }

    fn trigger_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let n = self.filtered_triggers(cx).len();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(10.))
            .px(t().d(20.))
            .pt(t().d(12.))
            .pb(t().d(10.))
            .child(
                div()
                    .w(t().d(220.))
                    .h(t().d(28.))
                    .child(Input::new(&self.triggers.search)),
            )
            .child(
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child(format!("{n} 个 · 按最近改动")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child("保存 ≠ 发布"),
            )
            .into_any_element()
    }

    fn trigger_grid(&self, rows: Vec<TriggerItem>, cx: &mut Context<Self>) -> AnyElement {
        if let Some(e) = &self.triggers.error {
            return div()
                .flex()
                .flex_col()
                .gap(t().d(6.))
                .p(t().d(20.))
                .child(
                    div()
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.danger))
                        .child(format!("拉触发器失败：{e}")),
                )
                .child(
                    div()
                        .id("trigger-retry")
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.link))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.load_trigger_list(true, cx);
                        }))
                        .child("重试"),
                )
                .into_any_element();
        }
        if self.triggers.loading && self.triggers.data.is_none() {
            return div()
                .p(t().d(20.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("拉触发器…")
                .into_any_element();
        }
        if rows.is_empty() {
            return div()
                .p(t().d(20.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("这一档没有触发器。换个筛选，或者点「新建触发器」。")
                .into_any_element();
        }
        let cards: Vec<AnyElement> = rows
            .iter()
            .map(|item| self.trigger_card(item, cx))
            .collect();
        div()
            .id("trigger-grid-scroll")
            .flex_1()
            .min_h(t().d(0.))
            .overflow_y_scroll()
            .track_scroll(&self.triggers.scroll)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(t().d(12.))
                    .px(t().d(20.))
                    .pb(t().d(20.))
                    .children(cards),
            )
            .into_any_element()
    }

    fn trigger_card(&self, item: &TriggerItem, cx: &mut Context<Self>) -> AnyElement {
        let (label, bg, fg) = status_of(item);
        let name = item.name.clone();
        let menu_name = item.name.clone();
        // 键盘 ↑↓ 改的是 selected，卡片上得看得见，否则像没反应。
        let picked = self.triggers.selected.as_deref() == Some(item.name.as_str());
        let params = item.params.len();
        let dirty = item.dirty_files;
        let tile = tile_color(&item.name);
        let cats: Vec<String> = item.category.iter().take(2).cloned().collect();
        div()
            .id(ElementId::Name(format!("tcard-{}", item.name).into()))
            .flex()
            .flex_col()
            .w(t().d(300.))
            .min_h(t().d(148.))
            .p(t().d(13.))
            .rounded(t().r(9.))
            .border(t().d(1.))
            .border_color(rgb(if picked {
                t().p.accent
            } else if label == "清单坏了" {
                t().p.danger
            } else {
                t().p.line
            }))
            .bg(rgb(t().p.panel))
            .cursor_pointer()
            .hover(|h| h.border_color(rgb(t().p.muted)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select_trigger(&name.clone(), cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    this.triggers.selected = Some(menu_name.clone());
                    this.triggers.menu = Some(TriggerMenu {
                        name: menu_name.clone(),
                        x: f32::from(ev.position.x),
                        y: f32::from(ev.position.y),
                    });
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(t().d(9.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(30.))
                            .rounded(t().r(8.))
                            .bg(rgb(tile))
                            .text_size(t().fs(13.))
                            .text_color(rgb(t().p.on_accent))
                            .child(item.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(t().fs(11.5))
                                    .text_color(rgb(t().p.text))
                                    .child(item.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(10.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(format!("v{}", item.version)),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .h(t().d(19.))
                            .px(t().d(6.))
                            .flex()
                            .items_center()
                            .gap(t().d(4.))
                            .rounded(t().r(4.))
                            .bg(rgb(bg))
                            .child(
                                div()
                                    .size(t().d(5.))
                                    .rounded(t().r(2.5))
                                    .bg(rgb(fg)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(fg))
                                    .child(label),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(t().d(10.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .child(short(&item.summary, 46)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(t().d(4.))
                    .mt(t().d(9.))
                    .children(cats.iter().map(|c| {
                        div()
                            .h(t().d(19.))
                            .px(t().d(6.))
                            .flex()
                            .items_center()
                            .rounded(t().r(4.))
                            .bg(rgb(t().p.line_soft))
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.text_2))
                            .child(c.clone())
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(6.))
                    .mt_auto()
                    .pt(t().d(10.))
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.text_2))
                            .child(format!("{params} 参数")),
                    )
                    .child(div().flex_1())
                    .children((dirty > 0).then(|| {
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.warn))
                            .child(format!("{dirty} 个文件"))
                    })),
            )
            .into_any_element()
    }

    fn trigger_archived_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self
            .triggers
            .data
            .as_ref()
            .map(|d| d.archived.clone())
            .unwrap_or_default();
        if rows.is_empty() {
            return div()
                .p(t().d(20.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("归档里是空的。归档过的触发器会躺在这里，随时能搬回来。")
                .into_any_element();
        }
        let items: Vec<AnyElement> = rows
            .iter()
            .map(|row| self.archived_row(row, cx))
            .collect();
        div()
            .id("trigger-archived-scroll")
            .flex_1()
            .min_h(t().d(0.))
            .overflow_y_scroll()
            .p(t().d(20.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(t().r(9.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .overflow_hidden()
                    .children(items),
            )
            .into_any_element()
    }

    fn archived_row(&self, row: &ArchivedTrigger, cx: &mut Context<Self>) -> AnyElement {
        let name = row.name.clone();
        let restore = row.name.clone();
        let purge = row.name.clone();
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(t().d(10.))
            .p(t().d(12.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(30.))
                    .rounded(t().r(8.))
                    .bg(rgb(t().p.disabled))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.panel))
                    .child(row.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(t().d(0.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(7.))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(row.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(format!("v{}", row.version.clone().unwrap_or_default())),
                            ),
                    )
                    .child(
                        div()
                            .mt(t().d(3.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child(short(row.summary.as_deref().unwrap_or("（没写说明）"), 60)),
                    )
                    .child(
                        div()
                            .mt(t().d(4.))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(format!(
                                "{} · 归档于 {}",
                                row.category.join(" / "),
                                row.at.as_deref().unwrap_or("—")
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(t().d(6.))
                    .child(
                        div()
                            .id(ElementId::Name(format!("trestore-{restore}").into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(24.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.trigger_write_unavailable(cx);
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child("恢复"),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(format!("tpurge-{purge}").into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(24.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.danger_soft))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.danger_soft)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.trigger_write_unavailable(cx);
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.danger))
                                    .child("彻底删除"),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(format!("tview-{name}").into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(24.))
                            .rounded(t().r(5.))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener({
                                let open = row.name.clone();
                                move |this, _: &ClickEvent, _, cx| {
                                    this.triggers.archived_detail = Some(open.clone());
                                    cx.notify();
                                }
                            }))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(t().d(13.))
                                    .text_color(rgb(t().p.muted)),
                            ),
                    ),
            )
            .child(div().w(t().d(0.)).child(name))
            .into_any_element()
    }

    /// 「已归档」点开的那一条：元信息 + 两个动作。
    ///
    /// 归档区是唯一一个「东西已经不在了」的地方，所以这一屏的用处不是看代码
    /// （代码还在仓库的 .archive/ 里，要看直接开文件），而是**删之前确认删的是哪个**。
    fn trigger_archived_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(name) = self.triggers.archived_detail.clone() else {
            return div().into_any_element();
        };
        let row = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.archived.iter().find(|a| a.name == name))
            .cloned();
        // 现在正有一份同名的在用 → 恢复会撞车，得先说明。
        let live = self
            .triggers
            .data
            .as_ref()
            .map(|d| d.triggers.iter().any(|t| t.name == name))
            .unwrap_or(false);
        let _restore = name.clone();
        let _purge = name.clone();
        let close = name.clone();

        div()
            .absolute()
            .top(t().d(56.))
            .right_0()
            .bottom_0()
            .w(t().d(392.))
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel_alt))
            .border_l(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(9.))
                    .p(t().d(16.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(30.))
                            .rounded(t().r(8.))
                            .bg(rgb(t().p.disabled))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.panel))
                            .child(name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .text_size(t().fs(12.5))
                                    .text_color(rgb(t().p.text))
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(format!(
                                        "已归档 · v{}",
                                        row.as_ref()
                                            .and_then(|r| r.version.clone())
                                            .unwrap_or_default()
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id("tarch-close")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(24.))
                            .rounded(t().r(5.))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = &close;
                                this.triggers.archived_detail = None;
                                cx.notify();
                            }))
                            .child(Icon::new(IconName::Close).size(t().d(12.)).text_color(rgb(t().p.muted))),
                    ),
            )
            .child(
                div()
                    .id("tarch-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .p(t().d(16.))
                    .flex()
                    .flex_col()
                    .gap(t().d(12.))
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .line_height(t().d(16.))
                            .text_color(rgb(t().p.text_2))
                            .child(
                                row.as_ref()
                                    .and_then(|r| r.summary.clone())
                                    .unwrap_or_else(|| "（没写说明）".to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t().d(5.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(t().d(6.))
                                    .children((row.as_ref().map(|r| r.category.clone()).unwrap_or_default()).into_iter().map(|c| {
                                        div()
                                            .px(t().d(5.))
                                            .h(t().d(18.))
                                            .flex()
                                            .items_center()
                                            .rounded(t().r(4.))
                                            .bg(rgb(t().p.surface))
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.text_2))
                                            .child(c)
                                            .into_any_element()
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(format!(
                                        "归档于 {}",
                                        row.as_ref()
                                            .and_then(|r| r.at.clone())
                                            .unwrap_or_else(|| "—".to_string())
                                    )),
                            ),
                    )
                    .when(live, |el| {
                        el.child(
                            div()
                                .p(t().d(9.))
                                .rounded(t().r(5.))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.warn))
                                .text_size(t().fs(10.))
                                .line_height(t().d(15.))
                                .text_color(rgb(t().p.warn))
                                .child(format!(
                                    "现在仓库里已经有一份同名「{name}」在用 —— 恢复会撞车，先处理掉在用的那份。"
                                )),
                        )
                    })
                    .child(
                        div()
                            .p(t().d(9.))
                            .rounded(t().r(5.))
                            .bg(rgb(t().p.surface))
                            .text_size(t().fs(9.5))
                            .line_height(t().d(15.))
                            .text_color(rgb(t().p.muted))
                            .child("恢复 = 把它从 .archive/ 搬回仓库根、重扫索引、提交 + 推送。彻底删除 = 连备份一起删，找不回来。"),
                    ),
            )
            .child(
                div()
                    .p(t().d(16.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .flex()
                    .flex_row()
                    .gap(t().d(8.))
                    .child(
                        div()
                            .id("tarch-restore")
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_1()
                            .h(t().d(30.))
                            .rounded(t().r(6.))
                            .bg(if live { rgb(t().p.disabled) } else { rgb(t().p.accent) })
                            .cursor_pointer()
                            .when(!live, |el| {
                                el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.triggers.archived_detail = None;
                                    this.trigger_write_unavailable(cx);
                                }))
                            })
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(if live { "同名占用，先处理" } else { "恢复" }),
                            ),
                    )
                    .child(
                        div()
                            .id("tarch-purge")
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(30.))
                            .px(t().d(12.))
                            .rounded(t().r(6.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.danger_soft))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.danger_soft)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.trigger_write_unavailable(cx);
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.danger))
                                    .child("彻底删除"),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// 「这个文件改了还没保存，真要切走？」
    ///
    /// 只在编辑态且有未保存内容时出现。不是模态对话框 —— 它只挡这一下切换，
    /// 因为丢的是**当前屏幕上的文本**，别的什么都没动。
    fn trigger_pending_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(next) = self.triggers.pending_path.clone() else {
            return div().into_any_element();
        };
        let current = self.triggers.code.path.clone().unwrap_or_default();
        let _name = self.triggers.code.name.clone().unwrap_or_default();
        div()
            .absolute()
            .top(t().d(56.))
            .left(t().d(214.))
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(t().p.scrim))
            .child(
                div()
                    .w(t().d(400.))
                    .p(t().d(18.))
                    .rounded(t().r(9.))
                    .bg(rgb(t().p.overlay))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(t().fs(13.))
                            .text_color(rgb(t().p.text))
                            .child(format!("{current} 有没保存的改动")),
                    )
                    .child(
                        div()
                            .mt(t().d(6.))
                            .text_size(t().fs(10.))
                            .line_height(t().d(15.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("切到 {next} 会把编辑框里的内容换掉，改动就没了（磁盘上还是旧的那版）。")),
                    )
                    .child(
                        div()
                            .mt(t().d(14.))
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .child(
                                div()
                                    .id("tp-save-first")
                                    .px(t().d(12.))
                                    .h(t().d(28.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .bg(rgb(t().p.accent))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.on_accent))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                        this.triggers.pending_path = None;
                                        this.trigger_write_unavailable(cx);
                                    }))
                                    .child("先保存"),
                            )
                            .child(
                                div()
                                    .id("tp-discard")
                                    .px(t().d(12.))
                                    .h(t().d(28.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.danger_soft))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.danger))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.danger_soft)))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                        this.triggers.pending_path = None;
                                        this.trigger_write_unavailable(cx);
                                    }))
                                    .child("放弃改动"),
                            )
                            .child(
                                div()
                                    .id("tp-stay")
                                    .px(t().d(12.))
                                    .h(t().d(28.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.text_2))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.triggers.pending_path = None;
                                        cx.notify();
                                    }))
                                    .child("留在这"),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn trigger_scrim(&self, _cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("trigger-scrim")
            .absolute()
            .top(t().d(56.))
            .left(t().d(214.))
            .right_0()
            .bottom_0()
            .bg(rgba(t().p.scrim))
            .into_any_element()
    }

    fn trigger_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(item) = self.triggers.current().cloned() else {
            return div().into_any_element();
        };
        let (label, bg, fg) = status_of(&item);
        let _name = item.name.clone();
        let _archive_name = item.name.clone();
        let params: Vec<AnyElement> = item
            .params
            .iter()
            .map(|p| self.param_card(p))
            .collect();
        div()
            .absolute()
            .top(t().d(56.))
            .right_0()
            .bottom_0()
            .w(t().d(464.))
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .border_l(t().d(1.))
            .border_color(rgb(t().p.line))
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(t().d(20.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(t().d(10.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(t().d(40.))
                                    .rounded(t().r(9.))
                                    .bg(rgb(tile_color(&item.name)))
                                    .text_size(t().fs(16.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(item.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(t().fs(16.))
                                            .text_color(rgb(t().p.text))
                                            .child(item.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .mt(t().d(3.))
                                            .text_size(t().fs(10.5))
                                            .text_color(rgb(t().p.muted))
                                            .child(format!(
                                                "v{} · {} · 产物 {}",
                                                item.version,
                                                item.category.join(" / "),
                                                item.binary.clone().unwrap_or_else(|| item.name.clone())
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .h(t().d(19.))
                                    .px(t().d(6.))
                                    .flex()
                                    .items_center()
                                    .gap(t().d(4.))
                                    .rounded(t().r(4.))
                                    .bg(rgb(bg))
                                    .child(
                                        div()
                                            .size(t().d(5.))
                                            .rounded(t().r(2.5))
                                            .bg(rgb(fg)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(fg))
                                            .child(label),
                                    ),
                            )
                            .child(
                                div()
                                    .id("trigger-detail-close")
                                    .size(t().d(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(t().r(5.))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_detail(cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::X)
                                            .size(t().d(14.))
                                            .text_color(rgb(t().p.muted)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .mt(t().d(11.))
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.text_2))
                            .child(item.summary.clone()),
                    )
                    .child(
                        div()
                            .mt(t().d(12.))
                            .flex()
                            .flex_row()
                            .gap(t().d(7.))
                            .child(
                                div()
                                    .id("trigger-detail-code")
                                    .flex()
                                    .items_center()
                                    .gap(t().d(5.))
                                    .h(t().d(26.))
                                    .px(t().d(9.))
                                    .rounded(t().r(5.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.trigger_write_unavailable(cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Code)
                                            .size(t().d(13.))
                                            .text_color(rgb(t().p.muted)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.text_2))
                                            .child("代码 ›"),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("trigger-archive")
                                    .flex()
                                    .items_center()
                                    .gap(t().d(5.))
                                    .h(t().d(26.))
                                    .px(t().d(9.))
                                    .rounded(t().r(5.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.danger_soft))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.danger_soft)))
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.trigger_write_unavailable(cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Archive)
                                            .size(t().d(13.))
                                            .text_color(rgb(t().p.danger)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.danger))
                                            .child("归档"),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("trigger-detail-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .child(detail_section(
                        "类别",
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(t().d(5.))
                            .children(item.category.iter().map(|c| {
                                div()
                                    .h(t().d(19.))
                                    .px(t().d(6.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(4.))
                                    .bg(rgb(t().p.surface))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(c.clone())
                            }))
                            .into_any_element(),
                    ))
                    .child(detail_section(
                        "清单 · nous.json",
                        div()
                            .flex()
                            .flex_col()
                            .children(vec![
                                kv("版本", item.version.clone()),
                                kv("产物名", item.binary.clone().unwrap_or_else(|| item.name.clone())),
                                kv("目录", format!("{}/", item.name)),
                                kv("语言", "Rust · 独立项目".into()),
                            ])
                            .into_any_element(),
                    ))
                    .child(detail_section("参数", div().flex().flex_col().gap(t().d(6.)).children(params).into_any_element()))
                    .child(detail_section(
                        "仓库",
                        div()
                            .flex()
                            .flex_col()
                            .children(vec![
                                kv("发布", if item.tracked { "已进 HEAD".into() } else { "未发布 · HEAD 里没有".into() }),
                                kv("改动", if item.dirty { format!("{} 个文件没提交", item.dirty_files) } else { "没有".into() }),
                                kv("同步", "清单 8 秒内自动推；代码要发布".into()),
                            ])
                            .into_any_element(),
                    ))
                    .child(detail_section(
                        "发布检查",
                        div()
                            .flex()
                            .flex_col()
                            .children(vec![
                                kv("cargo check", if item.dirty { "有改动，发布时才会跑".into() } else { "上次发布已通过".into() }),
                                kv("产物", format!("target/debug/{}", item.binary.clone().unwrap_or_else(|| item.name.clone()))),
                            ])
                            .into_any_element(),
                    )),
            )
            .into_any_element()
    }

    fn param_card(&self, p: &TriggerParam) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .p(t().d(9.))
            .rounded(t().r(7.))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(7.))
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text))
                            .child(p.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(p.kind.clone()),
                    )
                    .children(p.required.then(|| {
                        div()
                            .h(t().d(16.))
                            .px(t().d(4.))
                            .flex()
                            .items_center()
                            .rounded(t().r(3.))
                            .bg(rgb(t().p.warn_soft))
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.warn))
                            .child("必填")
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(p.title().to_string()),
                    ),
            )
            .children(p.help.as_ref().filter(|h| !h.is_empty()).map(|h| {
                div()
                    .mt(t().d(5.))
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(h.clone())
            }))
            // 1B：default / options 用左标签右值的对齐轴 —— 跟向导里填的
            // 是同一份数据，填了就能在这里看到（原来详情页只显示 name/type/required/help）。
            .children(
                (!p.options.is_empty() || p.default.is_some()).then(|| {
                    div()
                        .mt(t().d(7.))
                        .pt(t().d(7.))
                        .border_t(t().d(1.))
                        .border_color(rgb(t().p.line))
                        .flex()
                        .flex_col()
                        .gap(t().d(3.))
                        .children(
                            p.options
                                .iter()
                                .map(|o| param_meta_row("选项", o.clone(), true))
                                .collect::<Vec<_>>(),
                        )
                        .children(
                            p.default
                                .as_ref()
                                .map(|d| param_meta_row("默认", json_scalar_text(d), true))
                                .into_iter()
                                .collect::<Vec<_>>(),
                        )
                }),
            )
            .into_any_element()
    }
}

/// 参数卡里一行「左标签 · 右值」：标签右对齐成一条轴（Tesla 那条主内容轴）。
fn param_meta_row(label: &str, value: String, accent: bool) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(t().d(10.))
        .child(
            div()
                .w(t().d(30.))
                .flex_none()
                .text_align(gpui::TextAlign::Right)
                .text_size(t().fs(9.5))
                .text_color(rgb(t().p.muted))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .text_size(t().fs(10.))
                .text_color(rgb(if accent { t().p.accent_fg } else { t().p.text_2 }))
                .child(value),
        )
        .into_any_element()
}

/// serde_json::Value 显示成一行短文本：标量取原值，复合类型是紧凑 JSON。
fn json_scalar_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn tile_color(name: &str) -> u32 {
    // 浅色皮用深色头像，深色皮必须提亮 —— 否则 mono / amber 下头像是一团黑。
    const LIGHT: [u32; 6] = [0x1f6f5c, 0x4b5d7a, 0x7a5b32, 0x6b4b6f, 0x3f6b7a, 0x7a4b4b];
    const DARK: [u32; 6] = [0x3a9a82, 0x7189b0, 0xa8874f, 0x95739a, 0x5f97a8, 0xa86f6f];
    let mut h: u32 = 0;
    for ch in name.chars() {
        h = h.wrapping_mul(31).wrapping_add(ch as u32);
    }
    let palette = if is_dark(t().p.panel) { &DARK } else { &LIGHT };
    palette[(h as usize) % palette.len()]
}

fn is_dark(color: u32) -> bool {
    let r = (color >> 16) & 0xff;
    let g = (color >> 8) & 0xff;
    let b = color & 0xff;
    (r * 299 + g * 587 + b * 114) / 1000 < 128
}

fn detail_section(title: &str, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .px(t().d(20.))
        .py(t().d(14.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .mb(t().d(8.))
                .text_size(t().fs(10.))
                .text_color(rgb(t().p.text_2))
                .child(title.to_string()),
        )
        .child(body)
        .into_any_element()
}

fn kv(key: &str, value: String) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(t().d(10.))
        .py(t().d(3.))
        .child(
            div()
                .w(t().d(78.))
                .flex_none()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.muted))
                .child(key.to_string()),
        )
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.text_2))
                .child(value),
        )
        .into_any_element()
}

impl Desktop {
    fn trigger_code(&self, cx: &mut Context<Self>) -> AnyElement {
        let code = &self.triggers.code;
        // 2B：选中态升级成「左竖条 + 底色」；diff 增删右对齐成一列；
        // 编辑态改了没保存的文件挂一个琥珀圆点（跟 git dirty 是两码事）。
        let editing_dirty = code.editing
            && code.text.read(cx).value().as_ref() != code.base.as_str();
        let file_tabs: Vec<AnyElement> = code
            .files
            .iter()
            .map(|f| {
                let _path = f.path.clone();
                let on = code.path.as_deref() == Some(f.path.as_str());
                let unsaved = on && editing_dirty;
                // 右列：有增删就显示 +N −M，否则显示字节数；末尾再挂脏圆点。
                let right: AnyElement = if f.added != 0 || f.removed != 0 {
                    div()
                        .flex()
                        .items_center()
                        .gap(t().d(5.))
                        .child(
                            div()
                                .text_size(t().fs(9.))
                                .text_color(rgb(t().p.ok))
                                .child(format!("+{}", f.added)),
                        )
                        .child(
                            div()
                                .text_size(t().fs(9.))
                                .text_color(rgb(t().p.danger))
                                .child(format!("−{}", f.removed)),
                        )
                        .when(unsaved, |el| {
                            el.child(
                                div()
                                    .size(t().d(6.))
                                    .rounded(t().r(3.))
                                    .bg(rgb(t().p.accent)),
                            )
                        })
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .items_center()
                        .gap(t().d(5.))
                        .child(
                            div()
                                .text_size(t().fs(9.))
                                .text_color(rgb(t().p.muted))
                                .child(format!("{} B", f.size)),
                        )
                        .when(unsaved, |el| {
                            el.child(
                                div()
                                    .size(t().d(6.))
                                    .rounded(t().r(3.))
                                    .bg(rgb(t().p.accent)),
                            )
                        })
                        .into_any_element()
                };
                div()
                    .id(ElementId::Name(format!("tfile-{}", f.path).into()))
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(27.))
                    .px(t().d(7.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .border_l(t().d(2.))
                    .border_color(rgba(if on { t().p.accent } else { t().p.transparent }))
                    .when(on, |el| el.bg(rgb(t().p.hover)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        let _name = this.triggers.code.name.clone().unwrap_or_default();
                        this.trigger_write_unavailable(cx);
                    }))
                    .child(
                        Icon::new(IconName::File)
                            .size(t().d(11.))
                            .text_color(rgb(t().p.muted)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .text_size(t().fs(10.))
                            .text_color(rgb(if on { t().p.text_2 } else { t().p.muted }))
                            .child(f.path.clone()),
                    )
                    .child(right)
                    .into_any_element()
            })
            .collect();

        let body: AnyElement = if code.path.is_none() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("选一个文件")
                .into_any_element()
        } else if code.diff {
            self.trigger_diff(cx)
        } else if code.editing {
            div()
                .id("trigger-code-editor")
                .flex_1()
                .min_h(t().d(0.))
                .p(t().d(10.))
                .child(
                    Textarea::new(&code.text)
                        .size_full()
                        .font_family("JetBrainsMono"),
                )
                .into_any_element()
        } else {
            let text = code.text.read(cx).value().to_string();
            div()
                .id("trigger-code-readonly")
                .flex_1()
                .min_h(t().d(0.))
                .overflow_scroll()
                .p(t().d(10.))
                .child(code_block(&text))
                .into_any_element()
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .h(t().d(50.))
                    .px(t().d(12.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .id("trigger-code-back")
                            .flex()
                            .items_center()
                            .gap(t().d(4.))
                            .h(t().d(28.))
                            .px(t().d(8.))
                            .rounded(t().r(6.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.trigger_back_to_detail(cx);
                            }))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(t().d(13.))
                                    .text_color(rgb(t().p.muted)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child("触发器"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(format!(
                                "{} / {}",
                                code.name.clone().unwrap_or_default(),
                                code.path.clone().unwrap_or_default()
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("保存只落盘 · 发布才 commit + push"),
                    )
                    .child(
                        div()
                            .id("trigger-code-edit")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(26.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.triggers.code.editing = !this.triggers.code.editing;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(if code.editing { "只读" } else { "编辑" }),
                            ),
                    )
                    .child(
                        div()
                            .id("trigger-code-save")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(26.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.trigger_write_unavailable(cx);
                            }))
                            .child(
                                Icon::new(IconName::Save)
                                    .size(t().d(12.))
                                    .text_color(rgb(t().p.muted)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child("保存"),
                            ),
                    )
                    .child(
                        div()
                            .id("trigger-code-publish-top")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(26.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .bg(rgb(if code.busy { t().p.disabled } else { t().p.accent }))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if !this.triggers.code.busy {
                                    this.trigger_write_unavailable(cx);
                                }
                            }))
                            .child(
                                Icon::new(IconName::Upload)
                                    .size(t().d(12.))
                                    .text_color(rgb(t().p.on_accent)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(if code.busy { "发布中…" } else { "发布" }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(t().d(194.))
                            .flex_none()
                            .h_full()
                            .bg(rgb(t().p.panel_alt))
                            .border_r(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .p(t().d(8.))
                            .child(
                                div()
                                    .px(t().d(6.))
                                    .py(t().d(4.))
                                    .text_size(t().fs(9.))
                                    .text_color(rgb(t().p.muted))
                                    .child("文件"),
                            )
                            .child(
                                div()
                                    .id("trigger-files-scroll")
                                    .flex_1()
                                    .min_h(t().d(0.))
                                    .overflow_y_scroll()
                                    .children(file_tabs),
                            )
                            .children(code.notice.as_ref().map(|(text, bad)| {
                                div()
                                    .p(t().d(6.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(if *bad { t().p.danger } else { t().p.accent_fg }))
                                    .child(text.clone())
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(t().d(8.))
                                    .h(t().d(34.))
                                    .px(t().d(12.))
                                    .border_b(t().d(1.))
                                    .border_color(rgb(t().p.line_soft))
                                    .child(
                                        div()
                                            .text_size(t().fs(10.))
                                            .text_color(rgb(t().p.muted))
                                            .child(format!(
                                                "{} / {}",
                                                code.name.clone().unwrap_or_default(),
                                                code.path.clone().unwrap_or_default()
                                            )),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .id("trigger-diff-toggle")
                                            .h(t().d(22.))
                                            .px(t().d(8.))
                                            .flex()
                                            .items_center()
                                            .rounded(t().r(4.))
                                            .bg(rgb(if code.diff { t().p.hover } else { t().p.panel }))
                                            .border(t().d(1.))
                                            .border_color(rgb(t().p.line))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.triggers.code.diff = !this.triggers.code.diff;
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_size(t().fs(10.))
                                                    .text_color(rgb(t().p.text_2))
                                                    .child(if code.diff { "编辑" } else { "差异" }),
                                            ),
                                    ),
                            )
                            .child(body),
                    )
                    .child(self.trigger_publish_panel(cx)),
            )
            .into_any_element()
    }

    fn trigger_back_to_detail(&mut self, cx: &mut Context<Self>) {
        self.triggers.layer = Layer::Detail;
        cx.notify();
    }

    fn trigger_diff(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::diff::{self as d, Kind, Row};
        let text = self.triggers.code.text.read(cx).value().to_string();
        let old = self.triggers.code.base.clone();
        let lines = d::diff(&old, &text);
        let st = d::stat(&lines);
        let rows = d::rows(&lines, 3, &self.triggers.code.diff_open);
        let folded = rows.iter().filter(|r| matches!(r, Row::Fold { .. })).count();

        // 头上那行：`+7 −3`；干净时说「和 HEAD 一样」。
        let head = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(8.))
            .px(t().d(10.))
            .h(t().d(26.))
            .flex_none()
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(
                div()
                    .text_size(t().fs(10.))
                    .text_color(rgb(if st.added > 0 { t().p.accent_fg } else { t().p.muted }))
                    .child(format!("+{}", st.added)),
            )
            .child(
                div()
                    .text_size(t().fs(10.))
                    .text_color(rgb(if st.removed > 0 { t().p.danger } else { t().p.muted }))
                    .child(format!("−{}", st.removed)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.muted))
                    .child(if st.added == 0 && st.removed == 0 {
                        "和 HEAD 里的这一版一模一样".to_string()
                    } else if folded > 0 {
                        format!("{folded} 处未变已折叠 · 点开看")
                    } else {
                        "相对 HEAD 的改动".to_string()
                    }),
            );

        let no_col = |n: Option<usize>| {
            div()
                .w(t().d(30.))
                .flex_none()
                .pr(t().d(7.))
                .text_right()
                .text_size(t().fs(9.))
                .text_color(rgb(t().p.muted))
                .child(n.map(|n| n.to_string()).unwrap_or_default())
        };

        let body: Vec<AnyElement> = rows
            .into_iter()
            .map(|row| match row {
                Row::Fold { start, count } => div()
                    .id(ElementId::Name(format!("tdiff-fold-{start}").into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(t().d(20.))
                    .px(t().d(6.))
                    .rounded(t().r(4.))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.triggers.code.diff_open.insert(start);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.accent_fg))
                            .child(format!("⋯ {count} 行未变，点开")),
                    )
                    .into_any_element(),
                Row::Line(i) => {
                    let line = &lines[i];
                    let (tint, fg) = match line.kind {
                        Kind::Add => (Some(t().p.accent_soft), t().p.accent_fg),
                        Kind::Del => (None, t().p.danger),
                        Kind::Same => (None, t().p.text_2),
                    };
                    div()
                        .flex()
                        .flex_row()
                        .min_h(t().d(19.))
                        .when_some(tint, |el, c| el.bg(rgb(c)))
                        // 删的行在左边来一条红，不铺底色 —— 一屏里红块太多就没法读了
                        .when(line.kind == Kind::Del, |el| {
                            el.border_l(t().d(2.)).border_color(rgb(t().p.danger))
                        })
                        .when(line.kind == Kind::Add, |el| {
                            el.border_l(t().d(2.)).border_color(rgb(t().p.accent))
                        })
                        .child(no_col(line.old_no))
                        .child(no_col(line.new_no))
                        .child(
                            div()
                                .w(t().d(11.))
                                .flex_none()
                                .text_size(t().fs(9.5))
                                .text_color(rgb(fg))
                                .child(match line.kind {
                                    Kind::Add => "+",
                                    Kind::Del => "−",
                                    Kind::Same => "",
                                }),
                        )
                        .child(
                            div()
                                .whitespace_nowrap()
                                .text_size(t().fs(10.5))
                                .text_color(rgb(fg))
                                .child(if line.text.is_empty() {
                                    " ".to_string()
                                } else {
                                    line.text.to_string()
                                }),
                        )
                        .into_any_element()
                }
            })
            .collect();

        div()
            .id("trigger-diff-scroll")
            .flex_1()
            .min_h(t().d(0.))
            .flex()
            .flex_col()
            .bg(rgb(t().p.code_bg))
            .child(head)
            .child(
                div()
                    .id("trigger-diff-body")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_scroll()
                    .p(t().d(6.))
                    .child(div().flex().flex_col().children(body)),
            )
            .into_any_element()
    }

    /// 「发布」按下去之前那一屏：这次会提交什么、仓库什么状态、大概要等多久。
    fn trigger_preflight_block(&self, _cx: &mut Context<Self>) -> AnyElement {
        let code = &self.triggers.code;
        let item = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.triggers.iter().find(|t| Some(&t.name) == code.name.as_ref()))
            .cloned();
        let repo = self.triggers.data.as_ref().and_then(|d| d.repo.clone());
        let name = code.name.clone().unwrap_or_default();
        let running = code.busy;
        let elapsed = code
            .publish_started
            .map(|s| s.elapsed().as_secs())
            .or(code.publish_elapsed);

        let human = |n: u64| -> String {
            if n >= 1024 * 1024 {
                format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
            } else if n >= 1024 {
                format!("{:.1} KB", n as f64 / 1024.0)
            } else {
                format!("{n} B")
            }
        };

        let file_rows: Vec<AnyElement> = code
            .files
            .iter()
            .map(|f| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .py(t().d(2.5))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.text_2))
                            .child(f.path.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(6.))
                            .when(f.changed, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(t().d(4.))
                                        .when(f.added > 0, |el| {
                                            el.child(
                                                div()
                                                    .text_size(t().fs(9.5))
                                                    .text_color(rgb(t().p.accent_fg))
                                                    .child(format!("+{}", f.added)),
                                            )
                                        })
                                        .when(f.removed > 0, |el| {
                                            el.child(
                                                div()
                                                    .text_size(t().fs(9.5))
                                                    .text_color(rgb(t().p.danger))
                                                    .child(format!("−{}", f.removed)),
                                            )
                                        }),
                                )
                            })
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(human(f.size)),
                            ),
                    )
                    .into_any_element()
            })
            .collect();
        let file_count = code.files.len();

        let repo_line = match &repo {
            Some(r) => {
                let mut bits = vec![r.branch.clone().unwrap_or_else(|| "—".into())];
                if r.ahead > 0 {
                    bits.push(format!("领先 {} 个提交", r.ahead));
                }
                if r.behind > 0 {
                    bits.push(format!("落后 {} 个提交", r.behind));
                }
                bits.push(if r.dirty {
                    format!("仓库有 {} 个未提交改动", r.dirty_count)
                } else {
                    "工作区干净".to_string()
                });
                bits.join(" · ")
            }
            None => "拿不到仓库状态".to_string(),
        };

        div()
            .p(t().d(14.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .flex()
            .flex_col()
            .gap(t().d(9.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("将要提交"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{file_count} 个文件 + index.json")),
                    ),
            )
            .child(div().flex().flex_col().children(file_rows))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .py(t().d(2.5))
                    .child(
                        div()
                            .flex_1()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.accent_fg))
                            .child("index.json"),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(if item.as_ref().map(|i| i.tracked).unwrap_or(false) {
                                format!("改写 {name} 这一条")
                            } else {
                                format!("新增 {name} 这一条")
                            }),
                    ),
            )
            .child(
                div()
                    .pt(t().d(7.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .text_size(t().fs(9.5))
                    .line_height(t().d(14.))
                    .text_color(rgb(t().p.muted))
                    .child(repo_line),
            )
            .child(
                div()
                    .text_size(t().fs(9.5))
                    .line_height(t().d(14.))
                    .text_color(rgb(t().p.muted))
                    .child(
                        "整条一般 1–3 分钟。任何一步炸了就停在那一步：check / build 失败等于什么都没提交，\
                         推送失败则本地提交留着，不会丢。",
                    ),
            )
            .when(running || elapsed.is_some(), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(t().d(6.))
                        .child(
                            div()
                                .text_size(t().fs(10.))
                                .text_color(rgb(if running { t().p.warn } else { t().p.accent_fg }))
                                .child(if running {
                                    format!("发布中 · 已用 {}s", elapsed.unwrap_or(0))
                                } else {
                                    format!("上次用时 {}s", elapsed.unwrap_or(0))
                                }),
                        )
                        .child(
                            div()
                                .text_size(t().fs(9.5))
                                .text_color(rgb(t().p.muted))
                                .child(if running {
                                    "引擎一次跑完才回话，这里只能给计时"
                                } else {
                                    ""
                                }),
                        ),
                )
            })
            .into_any_element()
    }

    fn trigger_publish_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let item = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.triggers.iter().find(|t| Some(&t.name) == self.triggers.code.name.as_ref()))
            .cloned();
        let busy = self.triggers.code.busy;
        let publish = self.triggers.code.publish.clone();
        let err = self.triggers.code.publish_error.clone();
        let steps = ["cargo check", "cargo build", "node index.mjs", "commit", "push"];
        let step_rows: Vec<AnyElement> = steps
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let done = publish.is_some() || (busy && i == 0);
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(t().d(8.))
                    .py(t().d(4.5))
                    .child(
                        div()
                            .w(t().d(16.))
                            .flex_none()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if done { t().p.accent_fg } else { t().p.muted }))
                            .child(format!("{:02}", i + 1)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.text_2))
                                    .child(*label),
                            )
                            .child(
                                div()
                                    .mt(t().d(2.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(match *label {
                                        "cargo check" => "编不过就不提交 · 一般 20–90s",
                                        "cargo build" => "产物落 target/debug · 30–120s",
                                        "node index.mjs" => "重扫索引 · 秒级",
                                        "commit" => "只 add 这个触发器 + index.json",
                                        _ => "推到 origin/main",
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if done { t().p.accent_fg } else { t().p.muted }))
                            .child(if busy { "跑…".to_string() } else if done { "通过".to_string() } else { "待".to_string() }),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .w(t().d(372.))
            .flex_none()
            .h_full()
            .bg(rgb(t().p.panel_alt))
            .border_l(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .p(t().d(16.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.text))
                            .child(format!(
                                "发布 {}",
                                self.triggers.code.name.clone().unwrap_or_default()
                            )),
                    )
                    .child(
                        div()
                            .mt(t().d(3.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child("五步跑完才推；失败就停在出错那一步。"),
                    ),
            )
            .child(
                div()
                    .id("trigger-publish-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .child(self.trigger_preflight_block(cx))
                    .child(
                        div()
                            .p(t().d(14.))
                            .border_b(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .mb(t().d(8.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.muted))
                                            .child("流水线"),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.muted))
                                            .child(if busy {
                                                "发布中…".to_string()
                                            } else if let Some(p) = &publish {
                                                format!(
                                                    "{}{}",
                                                    if p.pushed { "已推送" } else { "未推送" },
                                                    p.head.clone().map(|h| format!(" · {h}")).unwrap_or_default()
                                                )
                                            } else {
                                                "待发布".to_string()
                                            }),
                                    ),
                            )
                            .child(div().flex().flex_col().children(step_rows)),
                    )
                    .child(
                        div()
                            .p(t().d(14.))
                            .border_b(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .mb(t().d(8.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("参数声明"),
                            )
                            .children(item.as_ref().map(|it| {
                                let cards: Vec<AnyElement> = it.params.iter().map(|p| self.param_card(p)).collect();
                                div().flex().flex_col().gap(t().d(6.)).children(cards).into_any_element()
                            }))
                            .children(item.is_none().then(|| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("没有参数")
                            })),
                    )
                    .child(
                        div()
                            .p(t().d(14.))
                            .child(
                                div()
                                    .mb(t().d(6.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("结果"),
                            )
                            .children(err.as_ref().map(|e| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.danger))
                                    .child(e.clone())
                            }))
                            .children(publish.as_ref().map(|p| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(if p.pushed { t().p.accent_fg } else { t().p.warn }))
                                    .child(format!(
                                        "committed={} pushed={} ahead={} {}",
                                        p.committed,
                                        p.pushed,
                                        p.ahead,
                                        p.push_error
                                    ))
                            }))
                            .children((err.is_none() && publish.is_none()).then(|| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("点下面的「发布」开始。")
                            })),
                    ),
            )
            .child(
                div()
                    .p(t().d(14.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .when(self.triggers.code.preflight && !busy, |el| {
                        el.flex()
                            .flex_row()
                            .gap(t().d(8.))
                            .child(
                                div()
                                    .id("trigger-preflight-cancel")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .flex_1()
                                    .h(t().d(32.))
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.triggers.code.preflight = false;
                                        cx.notify();
                                    }))
                                    .child("再看一眼"),
                            )
                            .child(
                                div()
                                    .id("trigger-preflight-go")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(t().d(6.))
                                    .flex_1()
                                    .h(t().d(32.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(t().p.accent))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.trigger_write_unavailable(cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Upload)
                                            .size(t().d(14.))
                                            .text_color(rgb(t().p.on_accent)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.on_accent))
                                            .child("开始发布"),
                                    ),
                            )
                    })
                    .when(!self.triggers.code.preflight || busy, |el| {
                        el.child(
                            div()
                                .id("trigger-publish-btn")
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(t().d(6.))
                                .h(t().d(32.))
                                .rounded(t().r(6.))
                                .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if this.triggers.code.busy {
                                        return;
                                    }
                                    // 先出检查单，别让人对着一个五步流水线干等。
                                    this.triggers.code.preflight = true;
                                    cx.notify();
                                }))
                                .child(
                                    Icon::new(IconName::Upload)
                                        .size(t().d(14.))
                                        .text_color(rgb(t().p.on_accent)),
                                )
                                .child(
                                    div()
                                        .text_size(t().fs(11.))
                                        .text_color(rgb(t().p.on_accent))
                                        .child(if busy { "发布中…" } else { "发布…" }),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Desktop {
    fn trigger_wizard(&self, cx: &mut Context<Self>) -> AnyElement {
        let step = self.triggers.wizard.step;
        let busy = self.triggers.wizard.busy;
        let err = self.triggers.wizard.err.clone();
        let steps = ["身份", "参数声明", "骨架预览", "建"];
        let step_bar: Vec<AnyElement> = steps
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let n = (i + 1) as u8;
                let on = n == step;
                let done = n < step;
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .child(
                        div()
                            .size(t().d(20.))
                            .rounded(t().r(10.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(if on {
                                t().p.accent
                            } else if done {
                                t().p.accent_soft
                            } else {
                                t().p.panel
                            }))
                            .border(t().d(1.))
                            .border_color(rgb(if on {
                                t().p.accent
                            } else if done {
                                t().p.accent_soft
                            } else {
                                t().p.line
                            }))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if on {
                                t().p.on_accent
                            } else if done {
                                t().p.accent_fg
                            } else {
                                t().p.muted
                            }))
                            .child(if done { "✓".to_string() } else { format!("{n:02}") }),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(if on { t().p.text } else { t().p.muted }))
                            .child(*label),
                    )
                    .children((i + 1 < steps.len()).then(|| {
                        div()
                            .w(t().d(44.))
                            .h(t().d(1.))
                            .mx(t().d(10.))
                            .bg(rgb(t().p.line))
                    }))
                    .into_any_element()
            })
            .collect();

        let form: AnyElement = match step {
            1 => div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(t().fs(16.))
                        .text_color(rgb(t().p.text))
                        .child("先给它一个身份"),
                )
                .child(
                    div()
                        .mt(t().d(5.))
                        .text_size(t().fs(11.))
                        .text_color(rgb(t().p.muted))
                        .child("目录名 = name = 产物名，三合一，建完改不了。"),
                )
                .child(wizard_field("名字", "建完改不了", div().h(t().d(31.)).child(Input::new(&self.triggers.wizard.name)).into_any_element()))
                .child(wizard_field("一句话说明", "卡片上就靠它认人", div().h(t().d(31.)).child(Input::new(&self.triggers.wizard.summary)).into_any_element()))
                .child(wizard_field("类别", "可空 = 未分类；写个不存在的名字就顺手建了", div().h(t().d(31.)).child(Input::new(&self.triggers.wizard.category)).into_any_element()))
                .into_any_element(),
            2 => self.wizard_params_step(cx),
            _ => div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(t().fs(16.))
                        .text_color(rgb(t().p.text))
                        .child("建之前最后看一眼"),
                )
                .child(
                    div()
                        .mt(t().d(5.))
                        .text_size(t().fs(11.))
                        .text_color(rgb(t().p.muted))
                        .child("三份文件已经生成，形状校验通过。建完只在工作区，不 commit、不 push。"),
                )
                .child(
                    div()
                        .mt(t().d(16.))
                        .flex()
                        .flex_col()
                        .gap(t().d(6.))
                        .child(check_line("名字合格，目录不存在"))
                        .child(check_line("必填四项齐全（name / version / summary / category）"))
                        .child(check_line("参数声明合法（epoch / select + options）"))
                        .child(check_line("骨架能编过（println 一行 JSON）")),
                )
                .child(
                    div()
                        .mt(t().d(16.))
                        .p(t().d(10.))
                        .rounded(t().r(7.))
                        .bg(rgb(t().p.warn_soft))
                        .text_size(t().fs(10.))
                        .text_color(rgb(t().p.warn))
                        .child("建完是「未发布」：没有产物、不在 HEAD，调度器看不到它。发布才会 check + build + index + commit + push。"),
                )
                .into_any_element(),
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(10.))
                    .h(t().d(52.))
                    .px(t().d(20.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .id("trigger-wizard-cancel")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(24.))
                            .px(t().d(8.))
                            .rounded(t().r(5.))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.trigger_layer(Layer::Main, cx);
                            }))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(t().d(13.))
                                    .text_color(rgb(t().p.muted)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.muted))
                                    .child("取消"),
                            ),
                    )
                    .child(div().flex().flex_row().items_center().gap(t().d(0.)).children(step_bar))
                    .child(div().flex_1())
                    .child(
                        div()
                            .h(t().d(19.))
                            .px(t().d(6.))
                            .flex()
                            .items_center()
                            .rounded(t().r(4.))
                            .bg(rgb(t().p.surface))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("建完是未发布"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(
                        div()
                            .id("trigger-wizard-form")
                            .flex()
                            .flex_col()
                            .w(t().d(470.))
                            .flex_none()
                            .h_full()
                            .overflow_y_scroll()
                            .p(t().d(22.))
                            .border_r(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .child(form)
                            .children(err.as_ref().map(|e| {
                                div()
                                    .mt(t().d(12.))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.danger))
                                    .child(e.clone())
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .bg(rgb(t().p.panel_alt))
                            .child(
                                div()
                                    .p(t().d(16.))
                                    .border_b(t().d(1.))
                                    .border_color(rgb(t().p.line_soft))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(t().d(8.))
                                            .child(
                                                div()
                                                    .text_size(t().fs(11.))
                                                    .text_color(rgb(t().p.text))
                                                    .child("预览"),
                                            )
                                            .child(
                                                div()
                                                    .h(t().d(16.))
                                                    .px(t().d(5.))
                                                    .flex()
                                                    .items_center()
                                                    .rounded(t().r(3.))
                                                    .bg(rgb(t().p.surface))
                                                    .text_size(t().fs(9.))
                                                    .text_color(rgb(t().p.muted))
                                                    .child(short(self.triggers.wizard.name.read(cx).value().as_ref(), 40)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .mt(t().d(3.))
                                            .text_size(t().fs(10.))
                                            .text_color(rgb(t().p.muted))
                                            .child("参数一改，这里跟着变。"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("trigger-wizard-preview")
                                    .flex_1()
                                    .min_h(t().d(0.))
                                    .overflow_y_scroll()
                                    .p(t().d(20.))
                                    .child(code_block(&self.wizard_preview(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .h(t().d(56.))
                    .px(t().d(20.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(match step {
                                1 => "身份 · 下一步填参数".to_string(),
                                2 => "参数声明 · 下一步看骨架".to_string(),
                                _ => "4 项校验通过 · 建完是未发布".to_string(),
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("trigger-wizard-prev")
                            .h(t().d(28.))
                            .px(t().d(11.))
                            .flex()
                            .items_center()
                            .rounded(t().r(6.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let step = this.triggers.wizard.step;
                                if step <= 1 {
                                    this.trigger_layer(Layer::Main, cx);
                                } else {
                                    this.wizard_step(step - 1, cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(if step <= 1 { "取消" } else { "上一步" }),
                            ),
                    )
                    .child(
                        div()
                            .id("trigger-wizard-next")
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(28.))
                            .px(t().d(12.))
                            .rounded(t().r(6.))
                            .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                if this.triggers.wizard.busy {
                                    return;
                                }
                                let step = this.triggers.wizard.step;
                                if step < 3 {
                                    this.wizard_step(step + 1, cx);
                                } else {
                                    this.trigger_write_unavailable(cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(match step {
                                        1 => "下一步：参数",
                                        2 => "下一步：看骨架",
                                        _ if busy => "建…",
                                        _ => "建 morning-review",
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// 向导里那两行参数（演示版固定）。建的时候原样写进 nous.json。
    /// 第 2 步：左边参数清单，右边是选中那条的全部字段（demo 方案 C）。
    fn wizard_params_step(&self, cx: &mut Context<Self>) -> AnyElement {
        let drafts = &self.triggers.wizard.params;
        let sel = self.triggers.wizard.param_sel.min(drafts.len().saturating_sub(1));

        // 左列：一个一行，必填给个红点，类型用徽章色区分。
        let mut rows: Vec<AnyElement> = vec![];
        for (i, d) in drafts.iter().enumerate() {
            let name = d.name.read(cx).value().trim().to_string();
            let on = i == sel;
            let ep = d.kind == "epoch";
            let sel_kind = d.kind == "select";
            rows.push(
                div()
                    .id(ElementId::Name(format!("wp-row-{i}").into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(27.))
                    .px(t().d(9.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.surface)).border_l(t().d(2.)).border_color(rgb(t().p.accent)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.wizard_param_select(i, cx);
                    }))
                    .child(
                        div()
                            .size(t().d(5.))
                            .rounded(t().r(3.))
                            .flex_none()
                            .bg(rgb(if d.required { t().p.danger } else { t().p.line })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(if name.is_empty() { t().p.muted } else { t().p.text_2 }))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(if name.is_empty() { "未命名".to_string() } else { name }),
                    )
                    .child(
                        div()
                            .px(t().d(4.))
                            .h(t().d(15.))
                            .flex()
                            .items_center()
                            .rounded(t().r(3.))
                            .bg(rgb(if ep {
                                t().p.accent_soft
                            } else if sel_kind {
                                t().p.warn_soft
                            } else {
                                t().p.surface
                            }))
                            .text_size(t().fs(8.5))
                            .text_color(rgb(if ep {
                                t().p.accent_fg
                            } else if sel_kind {
                                t().p.warn
                            } else {
                                t().p.muted
                            }))
                            .child(d.kind.clone()),
                    )
                    .into_any_element(),
            );
        }
        rows.push(
            div()
                .id("wp-add")
                .flex()
                .items_center()
                .h(t().d(28.))
                .px(t().d(9.))
                .mt(t().d(2.))
                .border_t(t().d(1.))
                .border_color(rgb(t().p.line_soft))
                .text_size(t().fs(10.))
                .text_color(rgb(t().p.muted))
                .cursor_pointer()
                .hover(|h| h.bg(rgb(t().p.hover)).text_color(rgb(t().p.accent_fg)))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.wizard_param_add(window, cx);
                }))
                .child("＋ 加一个参数")
                .into_any_element(),
        );

        // 右列：选中参数的全字段。
        let detail: AnyElement = if let Some(d) = drafts.get(sel) {
            let kind_tabs: Vec<AnyElement> = PARAM_KINDS
                .iter()
                .map(|k| {
                    let kstatic: &'static str = k;
                    let on = d.kind == *k;
                    div()
                        .id(ElementId::Name(format!("wp-kind-{k}").into()))
                        .px(t().d(7.))
                        .h(t().d(22.))
                        .flex()
                        .items_center()
                        .rounded(t().r(4.))
                        .bg(rgb(if on { t().p.accent } else { t().p.surface }))
                        .text_size(t().fs(9.5))
                        .text_color(rgb(if on { t().p.on_accent } else { t().p.text_2 }))
                        .cursor_pointer()
                        .hover(|h| h.bg(rgb(if on { t().p.accent } else { t().p.hover })))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.wizard_param_set_kind(sel, kstatic, cx);
                        }))
                        .child(*k)
                        .into_any_element()
                })
                .collect();
            let kind_hint = match d.kind.as_str() {
                "epoch" => "UTC 秒；页面上按本地时间挑",
                "number" => "整数或小数",
                "string" => "单行文本",
                "text" => "多行文本",
                "boolean" => "是 / 否",
                "select" => "从固定几项里选，下面填选项",
                _ => "",
            };
            div()
                .flex()
                .flex_col()
                .gap(t().d(9.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(t().d(7.))
                        .child(
                            div()
                                .text_size(t().fs(11.5))
                                .text_color(rgb(t().p.text))
                                .child("参数字段"),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("wp-del")
                                .size(t().d(21.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(t().r(4.))
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.muted))
                                .cursor_pointer()
                                .hover(|h| h.bg(rgb(t().p.danger_soft)).text_color(rgb(t().p.danger)))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.wizard_param_del(sel, cx);
                                }))
                                .child("✕"),
                        ),
                )
                .child(wizard_field("参数名", "写进 nous.json 的 name", div().h(t().d(28.)).child(Input::new(&d.name)).into_any_element()))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t().d(5.))
                        .child(
                            div()
                                .text_size(t().fs(10.5))
                                .text_color(rgb(t().p.text_2))
                                .child("类型"),
                        )
                        .child(div().flex().flex_row().flex_wrap().gap(t().d(5.)).children(kind_tabs))
                        .child(
                            div()
                                .text_size(t().fs(9.5))
                                .text_color(rgb(t().p.muted))
                                .child(kind_hint.to_string()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(t().d(7.))
                        .h(t().d(24.))
                        .child(
                            div()
                                .id("wp-required")
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(t().d(15.))
                                .rounded(t().r(3.))
                                .border(t().d(1.))
                                .border_color(rgb(if d.required { t().p.accent } else { t().p.line }))
                                .bg(rgb(if d.required { t().p.accent } else { t().p.surface }))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.wizard_param_toggle_required(sel, cx);
                                }))
                                .when(d.required, |el| {
                                    el.child(Icon::new(IconName::Check).size(t().d(10.)).text_color(rgb(t().p.on_accent)))
                                }),
                        )
                        .child(
                            div()
                                .text_size(t().fs(10.5))
                                .text_color(rgb(t().p.text_2))
                                .child("必填 · 不给就报错"),
                        ),
                )
                .child(wizard_field("标签", "卡片上给人看的名字", div().h(t().d(28.)).child(Input::new(&d.label)).into_any_element()))
                .when(d.kind == "select", |el| {
                    el.child(wizard_field("选项", "逗号分隔；只有 select 用得上", div().h(t().d(28.)).child(Input::new(&d.options)).into_any_element()))
                })
                .child(wizard_field("说明", "会显示在参数卡上", div().h(t().d(28.)).child(Input::new(&d.help)).into_any_element()))
                .child(wizard_field("默认值", "可空；number 填数字，boolean 填 true / false", div().h(t().d(28.)).child(Input::new(&d.default)).into_any_element()))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.muted))
                .child("左边还没有参数 —— 空着也能建，收不到参数就是了。")
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(t().d(8.))
                    .child(
                        div()
                            .text_size(t().fs(16.))
                            .text_color(rgb(t().p.text))
                            .child("这个触发器要人填什么"),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child("左边选一个，右边改它的字段"),
                    ),
            )
            .child(
                div()
                    .mt(t().d(12.))
                    .flex()
                    .flex_row()
                    .h(t().d(310.))
                    .min_h(t().d(310.))
                    .rounded(t().r(7.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .overflow_hidden()
                    .child(
                        div()
                            .id("wp-list-scroll")
                            .w(t().d(196.))
                            .flex_none()
                            .flex_1()
                            .min_h(t().d(0.))
                            .overflow_y_scroll()
                            .bg(rgb(t().p.panel_alt))
                            .border_r(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .children(rows),
                    )
                    .child(
                        div()
                            .id("wp-detail-scroll")
                            .flex_1()
                            .min_w(t().d(0.))
                            .min_h(t().d(0.))
                            .overflow_y_scroll()
                            .p(t().d(14.))
                            .child(detail),
                    ),
            )
            .into_any_element()
    }

    fn wizard_preview(&self, cx: &mut Context<Self>) -> String {
        let name = {
            let v = self.triggers.wizard.name.read(cx).value().trim().to_string();
            if v.is_empty() { "morning-review".to_string() } else { v }
        };
        let summary = {
            let v = self.triggers.wizard.summary.read(cx).value().trim().to_string();
            if v.is_empty() { "每天第一条会话时，按昨天的日志起一条复盘".to_string() } else { v }
        };
        let category = self.triggers.wizard.category.read(cx).value().trim().to_string();
        let mut lines = vec![
            "{".to_string(),
            format!("  \"name\": \"{name}\","),
            "  \"version\": \"0.1.0\",".to_string(),
            format!("  \"summary\": \"{summary}\","),
            format!("  \"category\": [\"{category}\"],"),
            format!("  \"binary\": \"{name}\","),
            "  \"params\": [".to_string(),
        ];
        let parsed = params_json(&self.triggers.wizard.params, cx);
        for (i, p) in parsed.iter().enumerate() {
            let comma = if i + 1 == parsed.len() { "" } else { "," };
            lines.push(format!("    {}{comma}", p));
        }
        lines.push("  ]".to_string());
        lines.push("}".to_string());
        lines.join("\n")
    }

    fn trigger_confirm_scrim(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("trigger-confirm-scrim")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.triggers.confirm.open = false;
                cx.notify();
            }))
            .into_any_element()
    }

    fn trigger_confirm(&self, cx: &mut Context<Self>) -> AnyElement {
        let name = self.triggers.confirm.name.clone();
        let purge = self.triggers.confirm.purge;
        let matches = self.triggers.confirm.input.read(cx).value().trim() == name;
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(t().d(420.))
                    .flex()
                    .flex_col()
                    .p(t().d(20.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.danger_soft))
                    .shadow_lg()
                    .child(
                        div()
                            .text_size(t().fs(14.))
                            .text_color(rgb(t().p.text))
                            .child(if purge {
                                format!("彻底删除 {name}？")
                            } else {
                                format!("归档 {name}？")
                            }),
                    )
                    .child(
                        div()
                            .mt(t().d(7.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child(if purge {
                                format!(
                                    "它会从 .archive/{name}/ 里删掉并提交 + 推送 —— 这是唯一一种连备份都不留的操作，删了就找不回来了。只是不想看见它的话，用「归档」就够。"
                                )
                            } else {
                                format!(
                                    "整个项目目录会移到 .archive/{name}/ 并提交 + 推送。GitHub 上不再有它，但备份还在仓库里，随时能恢复。"
                                )
                            }),
                    )
                    .child(
                        div()
                            .mt(t().d(13.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text_2))
                            .child(format!("手输名字 {name} 确认")),
                    )
                    .child(
                        div()
                            .mt(t().d(6.))
                            .h(t().d(30.))
                            .child(Input::new(&self.triggers.confirm.input)),
                    )
                    .child(
                        div()
                            .mt(t().d(14.))
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .child(
                                div()
                                    .id("trigger-confirm-cancel")
                                    .h(t().d(28.))
                                    .px(t().d(11.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.triggers.confirm.open = false;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.text_2))
                                            .child("取消"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("trigger-confirm-ok")
                                    .h(t().d(28.))
                                    .px(t().d(11.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .bg(rgb(if matches { t().p.danger } else { t().p.disabled }))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if this.triggers.confirm.input.read(cx).value().trim()
                                            == this.triggers.confirm.name
                                        {
                                            if this.triggers.confirm.purge {
                                                let _n = this.triggers.confirm.name.clone();
                                                this.triggers.confirm.open = false;
                                                this.triggers.archived_detail = None;
                                                this.trigger_write_unavailable(cx);
                                            } else {
                                                this.trigger_write_unavailable(cx);
                                            }
                                        }
                                    }))
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.on_accent))
                                            .child(if purge { "确认删除" } else { "归档并推送" }),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn check_line(text: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t().d(6.))
        .child(
            Icon::new(IconName::Check)
                .size(t().d(12.))
                .text_color(rgb(t().p.accent)),
        )
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.text_2))
                .child(text.to_string()),
        )
        .into_any_element()
}

fn wizard_field(label: &str, hint: &str, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .mt(t().d(16.))
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(t().d(7.))
                .child(
                    div()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.text_2))
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.muted))
                        .child(hint.to_string()),
                ),
        )
        .child(div().mt(t().d(6.)).child(body))
        .into_any_element()
}

fn code_block(text: &str) -> AnyElement {
    let lines: Vec<AnyElement> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            div()
                .flex()
                .flex_row()
                .min_h(t().d(19.))
                .child(
                    div()
                        .w(t().d(38.))
                        .flex_none()
                        .pr(t().d(9.))
                        .text_right()
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.muted))
                        .child(format!("{}", i + 1)),
                )
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.text_2))
                        .child(if line.is_empty() { " ".to_string() } else { line.to_string() }),
                )
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .p(t().d(8.))
        .rounded(t().r(8.))
        .border(t().d(1.))
        .border_color(rgb(t().p.line))
        .bg(rgb(t().p.code_bg))
        .children(lines)
        .into_any_element()
}

