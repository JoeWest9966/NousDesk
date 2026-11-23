//! 「元素库」页：方案 01 —— 资料库（两段清单 + 右栏详情，表单 / JSON 双视图）。
//!
//! 外观稿：`demo/elements/preview.html` 的 `#01`（用户从中挑的这一套）。
//! 骨架照旧面板 `k3skws` 块：**左清单（组在上、包在下）+ 右详情**，没有第三栏。
//!
//! ## 唯一真相是那份 JSON
//!
//! 选中一条 → 引擎给的 `fields` 就是 `draft`（`serde_json::Value`）。表单输入框、
//! JSON 编辑框都只是它的**两个视图**：
//!
//! - 表单改一个字段 → 写进 `draft`（只覆盖那个键，未知键一个不动）
//! - 切到 JSON → 从 `draft` 重新序列化（不是从输入框回读）
//! - JSON 里改了 → 保存时 `parse` 回 `draft`，解析失败就在本地标红、不发请求
//!
//! 这条是 `editor.rs`（模板编辑器）同一条准则。两份状态一定会在某次切换后分叉，
//! 而分叉的症状是「明明改了却没保存」—— 最难查的那种。
//!
//! ## 不做的事
//!
//! - **不校验业务**：`id` / `name` 必填、`id` 与文件名一致这些判定在引擎
//!   （`crates/nous/elements`），客户端只把 400 的 `{field, problem}` 原样显示。
//! - **不自己算断链**：`resolved.missing` 是引擎给的；这里只负责把它涂红。
//! - **不猜能力**：`Feature::Elements` 走 `capability.rs` 那一处；模块不在清单里
//!   就是灰入口，不做「先发了看 404」。
//! - 没有 rename：`id` 就是文件名，改名 = 新建 + 删旧的（引擎语义）。

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::{Element, ElementKind, ElementReceipt, Elements};

use crate::app::Desktop;

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

fn text_color() -> u32 {
    t().p.text
}
fn muted() -> u32 {
    t().p.muted
}
fn line_color() -> u32 {
    t().p.line
}

/// 右栏画什么。两个视图看的是同一份 `draft`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Mode {
    /// 表单：字段一行一个，能直接改。
    #[default]
    Form,
    /// JSON：整份原文，能直接改（保存时解析）。
    Json,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Form => "表单",
            Self::Json => "JSON",
        }
    }
}

/// 写动作的幂等锚。一次用户动作一个；重放同一个值引擎回第一次的回执（`坑.md` 99）。
fn new_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("desk-{n}-{}", std::process::id())
}

/// 元素库页的状态。所有字段都是「看一眼就懂」的：没有隐藏的第二份真相。
pub(crate) struct ElementState {
    /// `GET /v1/elements` 的回包。`None` = 还没拉到（或连不上）。
    pub data: Option<Elements>,
    pub loading: bool,
    /// 整页级的错（连不上 / 500）。单条坏 JSON 不写这里，它在行上。
    pub error: Option<String>,
    /// 当前选中：`(段, id)`。存 id 不存下标 —— 刷新之后行序会变。
    pub selected: Option<(ElementKind, String)>,
    pub mode: Mode,
    /// 正在编辑的那份 JSON。选中一条时从引擎回包抄进来。
    pub draft: Option<serde_json::Value>,
    /// `draft` 是从哪一版来的（写回的 `base`）。
    pub base: Option<String>,
    /// JSON 视图里解析失败的那一句（本地校验，不发请求）。
    pub json_error: Option<String>,
    pub saving: bool,
    /// 删除的二次确认（不可逆动作要多走一步）。
    pub confirm_delete: bool,

    pub search: Entity<InputState>,
    pub form_name: Entity<InputState>,
    pub form_desc: Entity<InputState>,
    pub form_body: Entity<TextareaState>,
    /// 组的引用（`packages`），逗号分隔一行。**不是** chip 编辑器 —— 一行够用，
    /// 而 chip 编辑器要另写一套拖拽 / 删除的交互，收益不值。
    pub form_refs: Entity<InputState>,
    /// 包的标签（`tags`），逗号分隔一行。
    pub form_tags: Entity<InputState>,
    pub json: Entity<TextareaState>,
}

impl ElementState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let input = |cx: &mut Context<Desktop>| cx.new(|cx| InputState::new(window, cx));
        Self {
            data: None,
            loading: false,
            error: None,
            selected: None,
            mode: Mode::Form,
            draft: None,
            base: None,
            json_error: None,
            saving: false,
            confirm_delete: false,
            search: input(cx),
            form_name: input(cx),
            form_desc: input(cx),
            form_body: cx.new(|cx| TextareaState::new(window, cx)),
            form_refs: input(cx),
            form_tags: input(cx),
            json: cx.new(|cx| TextareaState::new(window, cx)),
        }
    }

    /// 已拉过且没报错 —— 切回来不重拉（跟 `load_resources` 一个口径）。
    pub(crate) fn hydrated(&self) -> bool {
        self.data.is_some()
    }

    fn selected_element(&self) -> Option<&Element> {
        let (kind, id) = self.selected.as_ref()?;
        self.data.as_ref()?.find(*kind, id)
    }

    /// 搜索词。唯一真相是这个 `InputState`，不另存一份。
    fn query(&self, cx: &App) -> String {
        self.search.read(cx).value().trim().to_lowercase()
    }
}

// ── 拉取 ───────────────────────────────────────────────────────────────

impl Desktop {
    /// 切进元素库页 / 手动刷新：一份 GET 拉两段（引擎一次给全）。
    pub(crate) fn load_elements(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.elements_state.loading {
            return;
        }
        if !force && self.elements_state.hydrated() {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.elements_state.error = Some("还没连上引擎 —— 去「≡ 更多」里重新连接".into());
            cx.notify();
            return;
        };
        self.elements_state.loading = true;
        self.elements_state.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = engine.elements().await;
            let _ = this.update(cx, |this, cx| {
                this.elements_state.loading = false;
                match result {
                    Ok(data) => {
                        this.elements_state.error = None;
                        // 选中项跨刷新保住：id 还在就留着，没了就清空（不猜「大概指哪条」）。
                        let keep = this
                            .elements_state
                            .selected
                            .as_ref()
                            .is_some_and(|(kind, id)| data.find(*kind, id).is_some());
                        if !keep {
                            this.elements_state.selected = None;
                            this.elements_state.draft = None;
                            this.elements_state.base = None;
                        }
                        this.elements_state.data = Some(data);
                        this.sync_forms(cx);
                    }
                    Err(e) => this.elements_state.error = Some(format!("元素库：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 选一条：抄进 `draft`，再把两个视图都对齐到它。
    pub(crate) fn select_element(
        &mut self,
        kind: ElementKind,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.elements_state.confirm_delete = false;
        self.elements_state.json_error = None;
        self.elements_state.selected = Some((kind, id));
        self.elements_state.mode = Mode::Form;
        self.sync_forms_windowed(window, cx);
        cx.notify();
    }

    /// 从当前选中项刷新 `draft` + 两个视图。选中项没了就清空。
    fn sync_forms(&mut self, cx: &mut Context<Self>) {
        // 没有 window 的路径（后台刷新回来）只对齐 draft；文本框由 render 前的
        // `sync_forms_windowed` 补。少一次 set_value 不会丢数据 —— 它只是视图。
        let element = self.elements_state.selected_element().cloned();
        let state = &mut self.elements_state;
        state.draft = element.as_ref().and_then(|e| e.fields.clone());
        state.base = element.as_ref().and_then(|e| e.etag.clone());
        cx.notify();
    }

    fn sync_forms_windowed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_forms(cx);
        let (name, desc, body, refs, tags, json) = {
            let st = &self.elements_state;
            let Some(fields) = st.draft.as_ref() else {
                (String::new(), String::new(), String::new(), String::new(), String::new(), "{}".to_string())
            };
            (
                str_field(fields, "name"),
                str_field(fields, "desc"),
                str_field(fields, "body"),
                list_field(fields, "packages"),
                list_field(fields, "tags"),
                serde_json::to_string_pretty(fields).unwrap_or_else(|_| "{}".into()),
            )
        };
        let st = &mut self.elements_state;
        st.form_name.update(cx, |s, cx| s.set_value(&name, window, cx));
        st.form_desc.update(cx, |s, cx| s.set_value(&desc, window, cx));
        st.form_body.update(cx, |s, cx| s.set_value(&body, window, cx));
        st.form_refs.update(cx, |s, cx| s.set_value(&refs, window, cx));
        st.form_tags.update(cx, |s, cx| s.set_value(&tags, window, cx));
        st.json.update(cx, |s, cx| s.set_value(&json, window, cx));
        cx.notify();
    }

    /// 切视图。切到 JSON 时从 `draft` 重新序列化（不回读表单输入框）。
    pub(crate) fn set_element_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        if self.elements_state.mode == mode {
            return;
        }
        self.elements_state.mode = mode;
        if mode == Mode::Json {
            if let Some(fields) = self.elements_state.draft.as_ref() {
                let text = serde_json::to_string_pretty(fields).unwrap_or_else(|_| "{}".into());
                self.elements_state.json.update(cx, |s, cx| s.set_value(&text, window, cx));
            }
            self.elements_state.json_error = None;
        }
        cx.notify();
    }

    /// 保存并提交：PUT（有 `base` 就是 If-Match）。成功 / 失败都回一句话。
    pub(crate) fn save_element(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.elements_state.saving {
            return;
        }
        let Some((kind, id)) = self.elements_state.selected.clone() else {
            return;
        };
        // JSON 视图下先以文本框为准（用户可能在那一栏改过，又没切回表单）。
        if self.elements_state.mode == Mode::Json {
            let text = self.elements_state.json.read(cx).value().to_string();
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => {
                    self.elements_state.draft = Some(normalize_object(v));
                    self.elements_state.json_error = None;
                }
                Err(e) => {
                    // 本地就拦住：一份坏 JSON 发出去，引擎只会回 400，
                    // 而 400 的那句话远不如编辑器里这行字好懂。
                    self.elements_state.json_error = Some(format!("JSON 解析失败：{e}"));
                    cx.notify();
                    return;
                }
            }
        }
        let Some(fields) = self.elements_state.draft.clone() else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let base = self.elements_state.base.clone();
        let request_id = new_request_id();
        self.elements_state.saving = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = engine
                .element_put(kind, &id, &fields, base.as_deref(), Some(&request_id))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.elements_state.saving = false;
                match result {
                    Ok(receipt) => {
                        this.notice_element(&receipt, "已保存", cx);
                        this.load_elements(true, cx);
                        this.sync_forms_windowed(window, cx);
                    }
                    Err(e) => this.notify_error(&e, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 新建：在当前段里造一份最小合法 JSON。`id` 先给个占位，用户自己改。
    pub(crate) fn new_element(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.elements_state.selected.as_ref().map(|(k, _)| *k).or_else(|| {
            // 没选中就按「组优先」—— 左清单第一段就是组。
            Some(ElementKind::Groups)
        }) else {
            return;
        };
        let id = unused_id(kind, self.elements_state.data.as_ref());
        let fields = match kind {
            ElementKind::Groups => serde_json::json!({
                "id": id,
                "name": "",
                "desc": "",
                "packages": [],
            }),
            ElementKind::Packages => serde_json::json!({
                "id": id,
                "name": "",
                "kind": "prompt",
                "desc": "",
                "body": "",
                "tags": [],
            }),
        };
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let request_id = new_request_id();
        let kind_for_receipt = kind;
        cx.spawn(async move |this, cx| {
            let result = engine.element_create(kind_for_receipt, &fields, Some(&request_id)).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(receipt) => {
                        this.notice_element(&receipt, "已新建", cx);
                        this.elements_state.selected =
                            Some((kind_for_receipt, receipt.id.clone()));
                        this.elements_state.mode = Mode::Form;
                        this.load_elements(true, cx);
                        this.sync_forms_windowed(window, cx);
                    }
                    Err(e) => {
                        this.notify_error(&e, cx)
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 删除（二次确认之后）。不可逆：走 DELETE，`base` 一起带上防误删别人的新版。
    pub(crate) fn delete_element(&mut self, cx: &mut Context<Self>) {
        let Some((kind, id)) = self.elements_state.selected.clone() else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let base = self.elements_state.base.clone();
        let request_id = new_request_id();
        self.elements_state.confirm_delete = false;
        self.elements_state.saving = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = engine
                .element_delete(kind, &id, base.as_deref(), Some(&request_id))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.elements_state.saving = false;
                match result {
                    Ok(receipt) => {
                        this.notice_element(&receipt, "已删除", cx);
                        if this
                            .elements_state
                            .selected
                            .as_ref()
                            .is_some_and(|(_, sel)| sel == &id)
                        {
                            this.elements_state.selected = None;
                            this.elements_state.draft = None;
                            this.elements_state.base = None;
                        }
                        this.load_elements(true, cx);
                    }
                    Err(e) => this.notify_error(&e, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 手动补推。推不上去不改本地提交 —— 回执里 `pushed` 说清楚就行。
    pub(crate) fn push_elements(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = engine.elements_push().await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(push) => {
                        let text = match push.pushed.as_deref() {
                            Some("ok") => "已推送".to_string(),
                            Some(other) => format!("推送没成：{other}（本地提交还在）"),
                            None => "推送：引擎没说结果".to_string(),
                        };
                        this.notify(text, crate::app::NoticeKind::Info, cx);
                        this.load_elements(true, cx);
                    }
                    Err(e) => this.notify_error(&e, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 写回执翻译成一句人话。`commit: null` 不是错误（内容没变 / 重放）。
    fn notice_element(&mut self, receipt: &ElementReceipt, verb: &str, cx: &mut Context<Self>) {
        self.notify(receipt.summary(verb), crate::app::NoticeKind::Info, cx);
    }
}

// ── draft 的小工具 ─────────────────────────────────────────────────────

/// 表单里改了一个标量键（`name` / `desc` / `body`）。
pub(crate) fn on_scalar(desk: &mut Desktop, key: &str, value: String) {
    write_scalar(desk, key, value);
}

/// 表单里改了一个数组（`packages` / `tags`，逗号分隔一行）。
pub(crate) fn on_list(desk: &mut Desktop, key: &str, value: String) {
    write_list(desk, key, value);
}

/// JSON 编辑框改了：解析成功就换 `draft`，失败只记一句（保存时才拦）。
///
/// 不在这里每帧拦 —— 输入到一半一定不合法，红了只会让人以为已经出错。
pub(crate) fn on_json(desk: &mut Desktop, text: &str) {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => {
            desk.elements_state.draft = Some(normalize_object(v));
            desk.elements_state.json_error = None;
        }
        Err(e) => {
            desk.elements_state.json_error = Some(format!("JSON 解析失败：{e}"));
        }
    }
}

/// 只覆盖一个标量键：`draft` 里别的键（包括引擎新加的未知键）一个不动。
///
/// 空串**不删键**：`desc: ""` 与「没有 desc」是两件事，删不删由用户决定
/// （AGENTS：服务端一个键都不删，客户端也别替它删）。
fn write_scalar(desk: &mut Desktop, key: &str, value: String) {
    let Some(fields) = desk.elements_state.draft.as_mut() else {
        return;
    };
    let Some(map) = fields.as_object_mut() else {
        return;
    };
    map.insert(key.to_string(), serde_json::Value::String(value));
}

/// 逗号分隔的一行 → 数组。中文逗号也认（输入法就在中文状态）。
fn write_list(desk: &mut Desktop, key: &str, value: String) {
    let Some(fields) = desk.elements_state.draft.as_mut() else {
        return;
    };
    let Some(map) = fields.as_object_mut() else {
        return;
    };
    let items: Vec<serde_json::Value> = value
        .split([',', '，'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::Value::String(s.to_string()))
        .collect();
    map.insert(key.to_string(), serde_json::Value::Array(items));
}

fn str_field(fields: &serde_json::Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn list_field(fields: &serde_json::Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// JSON 视图里写了数组 / 标量也要能存：整份不是对象就包一层 `{"value": …}`？
/// 不行 —— 元素本来就是一个对象。这里只做一件事：非对象原样返回，保存时引擎会 400，
/// 那句话比客户端自造的更准。
fn normalize_object(v: serde_json::Value) -> serde_json::Value {
    v
}

/// 新建时先占一个没人用的 id：`new-group` / `new-group-2` …
fn unused_id(kind: ElementKind, data: Option<&Elements>) -> String {
    let base = match kind {
        ElementKind::Groups => "new-group",
        ElementKind::Packages => "new-package",
    };
    let taken = |candidate: &str| {
        data.is_some_and(|d| d.find(kind, candidate).is_some())
    };
    if !taken(base) {
        return base.to_string();
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    base.to_string()
}

// ── 渲染 ───────────────────────────────────────────────────────────────

pub(crate) fn render(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let st = &desk.elements_state;
    let mut root = div().flex().flex_row().flex_1().h_full().min_w(t().d(0.));
    root = root
        .child(render_side(desk, cx))
        .child(render_detail(desk, cx));
    if st.confirm_delete {
        return div()
            .relative()
            .flex()
            .flex_row()
            .flex_1()
            .h_full()
            .min_w(t().d(0.))
            .child(root)
            .child(render_delete_confirm(desk, cx))
            .into_any_element();
    }
    root.into_any_element()
}

/// 左清单：组一段、包一段。段的标题上带条数 —— 空着也显示（「0 个包」是信息）。
fn render_side(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let st = &desk.elements_state;
    let q = st.query(cx);
    let data = st.data.as_ref();

    let mut col = div()
        .w(t().d(262.))
        .h_full()
        .flex()
        .flex_col()
        .border_r(t().d(1.))
        .border_color(rgb(line_color()))
        .bg(rgb(t().p.list_bg));

    col = col.child(
        div().px(t().d(12.)).pt(t().d(12.)).pb(t().d(6.)).child(
            Input::new(&st.search)
                .small()
                .cleanable(true)
                .prefix(
                    Icon::new(IconName::Search)
                        .text_color(rgb(t().p.faint))
                        .size(t().d(14.)),
                ),
        ),
    );

    if let Some(err) = st.error.as_deref() {
        col = col.child(
            div()
                .mx(t().d(12.))
                .my(t().d(8.))
                .p(t().d(10.))
                .rounded(t().r(7.))
                .bg(rgb(t().p.danger_soft))
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.danger_fg))
                .child(err.to_string()),
        );
    }

    let mut list = div()
        .id("elements-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(t().d(0.))
        .overflow_y_scroll()
        .px(t().d(8.))
        .pb(t().d(16.));

    for kind in [ElementKind::Groups, ElementKind::Packages] {
        let rows: Vec<&Element> = data
            .map(|d| {
                d.section(kind)
                    .iter()
                    .filter(|e| matches_query(e, &q))
                    .collect()
            })
            .unwrap_or_default();
        let total = data.map(|d| d.section(kind).len()).unwrap_or(0);
        list = list.child(section_cap(kind, total, rows.len(), &q));
        for row in rows {
            list = list.child(element_row(kind, row, st, desk, cx));
        }
        if rows.is_empty() && q.is_empty() && total == 0 {
            list = list.child(
                div()
                    .px(t().d(10.))
                    .pb(t().d(8.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.faint))
                    .child(match kind {
                        ElementKind::Groups => "还没有组。放一个 groups/<id>.json 进私库，或点「新建」。",
                        ElementKind::Packages => "还没有包。",
                    }),
            );
        }
    }

    col.child(list).child(render_repo_foot(desk, cx)).into_any_element()
}

fn section_cap(kind: ElementKind, total: usize, shown: usize, q: &str) -> AnyElement {
    let label = if q.is_empty() {
        format!("{} · {total}", kind.label())
    } else {
        format!("{} · {shown}/{total}", kind.label())
    };
    div()
        .px(t().d(10.))
        .pt(t().d(14.))
        .pb(t().d(6.))
        .text_size(t().fs(10.))
        .text_color(rgb(t().p.faint))
        .child(label)
        .into_any_element()
}

fn matches_query(e: &Element, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let hay = format!(
        "{} {} {} {}",
        e.id,
        e.name(),
        e.desc().unwrap_or_default(),
        e.body().unwrap_or_default()
    )
    .to_lowercase();
    hay.contains(q)
}

/// 一行。组 / 包同一副骨架，靠缩进 + 字号分两层（照 demo 01），不画两套组件。
fn element_row(
    kind: ElementKind,
    e: &Element,
    st: &ElementState,
    desk: &Desktop,
    cx: &mut Context<Desktop>,
) -> AnyElement {
    let on = st
        .selected
        .as_ref()
        .is_some_and(|(k, id)| *k == kind && id == &e.id);
    let broken = e.broken();
    // 组里引用了不存在的包：这一行也该红（不然「这组是坏的」要在点进去之后才知道）。
    let dangling = kind == ElementKind::Groups
        && desk
            .elements_state
            .data
            .as_ref()
            .map(|d| !d.resolved.missing_in(&e.id).is_empty())
            .unwrap_or(false);

    let name_color = if broken || dangling {
        t().p.danger
    } else if on {
        t().p.accent
    } else if kind == ElementKind::Groups {
        text_color()
    } else {
        t().p.text_2
    };

    // 行尾：组给「引用数」，包给类型标签（`提示词` / `清单` / …）。
    let tail = match kind {
        ElementKind::Groups => format!("{}", e.packages().len()),
        ElementKind::Packages => match e.package_kind() {
            Some("prompt") => "提示词".to_string(),
            Some("checklist") => "清单".to_string(),
            Some(other) => other.to_string(),
            None => String::new(),
        },
    };

    let id = e.id.clone();
    div()
        .id(ElementId::Name(format!("el-row-{kind:?}-{}", e.id).into()))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.select_element(kind, id.clone(), window, cx);
        }))
        .when(kind == ElementKind::Packages, |el| el.pl(t().d(20.)))
        .flex()
        .items_center()
        .gap(t().d(8.))
        .px(t().d(10.))
        .py(t().d(6.))
        .rounded(t().r(6.))
        .when(on, |el| el.bg(rgb(t().p.selected)))
        .hover(|el| el.bg(rgb(t().p.hover)))
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .text_size(t().fs(if kind == ElementKind::Groups { 12.5 } else { 11.5 }))
                .text_color(rgb(name_color))
                .child(e.name().to_string()),
        )
        .child(
            div()
                .flex_none()
                .text_size(t().fs(10.))
                .text_color(rgb(if broken || dangling { t().p.danger } else { t().p.faint }))
                .child(if broken { "坏 JSON".to_string() } else { tail }),
        )
        .into_any_element()
}

/// 左下角那行：私库路径 + git 三态。没装配就直接说怎么装。
fn render_repo_foot(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let repo = desk.elements_state.data.as_ref().map(|d| &d.repo);
    let (text, color) = match repo {
        None => ("—".to_string(), muted()),
        Some(r) if !r.present => (
            r.missing.clone().unwrap_or_else(|| format!("私库不存在：{}", r.root)),
            t().p.danger,
        ),
        Some(r) => {
            let mut bits = vec![r.branch.clone().unwrap_or_else(|| "main".into())];
            if r.dirty == Some(true) {
                bits.push("未提交".into());
            }
            if let Some(ahead) = r.ahead.filter(|n| *n > 0) {
                bits.push(format!("未推送 {ahead}"));
            }
            if let Some(behind) = r.behind.filter(|n| *n > 0) {
                bits.push(format!("落后 {behind}"));
            }
            (bits.join(" · "), muted())
        }
    };
    let dirty_or_ahead = repo.is_some_and(|r| {
        r.dirty == Some(true) || r.ahead.is_some_and(|n| n > 0)
    });
    div()
        .flex()
        .items_center()
        .gap(t().d(8.))
        .px(t().d(12.))
        .py(t().d(8.))
        .border_t(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .text_size(t().fs(10.))
                .text_color(rgb(color))
                .child(text),
        )
        .when(dirty_or_ahead, |el| {
            el.child(
                div()
                    .id("el-push")
                    .flex_none()
                    .px(t().d(7.))
                    .py(t().d(3.))
                    .rounded(t().r(5.))
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.accent_fg))
                    .bg(rgb(t().p.accent_soft))
                    .hover(|el| el.bg(rgb(t().p.accent_soft_hover)))
                    .child("推送")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.push_elements(cx))),
            )
        })
        .into_any_element()
}

/// 右栏：顶条（表单 / JSON + 动作） + 正文。
fn render_detail(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let st = &desk.elements_state;
    let Some(element) = st.selected_element() else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_w(t().d(0.))
            .bg(rgb(t().p.panel))
            .child(render_topbar(desk, false, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(t().d(40.))
                    .text_size(t().fs(12.5))
                    .text_color(rgb(muted()))
                    .child("左边选一条看详情。组是引用，包是原子 —— 一份 JSON 一个元素。"),
            )
            .into_any_element();
    };

    let kind = st.selected.as_ref().map(|(k, _)| *k).unwrap_or(ElementKind::Groups);
    let body = match st.mode {
        Mode::Form => render_form(desk, element, kind, cx),
        Mode::Json => render_json(desk, cx),
    };

    div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w(t().d(0.))
        .bg(rgb(t().p.panel))
        .child(render_topbar(desk, true, cx))
        .child(body)
        .into_any_element()
}

fn render_topbar(desk: &Desktop, has_selection: bool, cx: &mut Context<Desktop>) -> AnyElement {
    let st = &desk.elements_state;
    let mode_btn = |mode: Mode, cx: &mut Context<Desktop>| {
        let on = st.mode == mode;
        div()
            .id(ElementId::Name(format!("el-mode-{:?}", mode).into()))
            .px(t().d(9.))
            .py(t().d(5.))
            .rounded(t().r(6.))
            .text_size(t().fs(11.5))
            .text_color(rgb(if on { t().p.accent } else { muted() }))
            .when(on, |el| el.font_weight(FontWeight::SEMIBOLD))
            .hover(|el| el.bg(rgb(t().p.hover)))
            .child(mode.label())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.set_element_mode(mode, window, cx);
            }))
    };

    let mut bar = div()
        .flex()
        .items_center()
        .gap(t().d(10.))
        .h(t().d(54.))
        .px(t().d(22.))
        .border_b(t().d(1.))
        .border_color(rgb(line_color()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t().d(1.))
                .child(
                    div()
                        .text_size(t().fs(12.))
                        .text_color(rgb(text_color()))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("元素库"),
                )
                .child(
                    div()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(muted()))
                        .child("组与包 · 一份 JSON 一个元素"),
                ),
        )
        .child(div().flex_1());

    if has_selection {
        bar = bar
            .child(mode_btn(Mode::Form, cx))
            .child(mode_btn(Mode::Json, cx));
    }

    bar = bar.child(
        div()
            .id("el-new")
            .px(t().d(11.))
            .py(t().d(6.))
            .rounded(t().r(6.))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .text_size(t().fs(11.5))
            .text_color(rgb(t().p.text_2))
            .hover(|el| el.bg(rgb(t().p.hover)))
            .child("新建")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.new_element(window, cx);
            })),
    );

    if has_selection {
        bar = bar.child(
            div()
                .id("el-delete")
                .px(t().d(9.))
                .py(t().d(6.))
                .rounded(t().r(6.))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.danger))
                .hover(|el| el.bg(rgb(t().p.danger_soft)))
                .child("删除")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.elements_state.confirm_delete = true;
                    cx.notify();
                })),
        );
        let saving = st.saving;
        bar = bar.child(
            div()
                .id("el-save")
                .px(t().d(15.))
                .py(t().d(8.))
                .rounded(t().r(6.))
                .bg(rgb(if saving { t().p.disabled } else { t().p.accent }))
                .text_size(t().fs(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(t().p.on_accent))
                .when(!saving, |el| {
                    el.hover(|el| el.bg(rgb(t().p.accent_hover)))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.save_element(window, cx);
                        }))
                })
                .child(if saving { "保存中…" } else { "保存并提交" }),
        );
    }

    bar.into_any_element()
}

/// 表单视图：kicker / 大标题 / 导语 / 路径行 / 细线 / 字段 / 细线 / 正文。
///
/// 字号层级就是层级本身（demo 01）：30px 细细的标题、15px 导语、10px 小标签 ——
/// 不靠框线和色块堆。
fn render_form(
    desk: &Desktop,
    element: &Element,
    kind: ElementKind,
    cx: &mut Context<Desktop>,
) -> AnyElement {
    let st = &desk.elements_state;
    let path = format!(
        "{}/{}.json",
        kind.dir(),
        element.id
    );
    let etag = element.etag.clone().unwrap_or_default();
    let (fingerprint, fp_color) = if etag.is_empty() {
        ("读不出来".to_string(), t().p.danger)
    } else {
        (format!("内容指纹 {}", short_hash(&etag)), muted())
    };

    let mut doc = div()
        .id("el-doc")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(t().d(0.))
        .overflow_y_scroll()
        .px(t().d(52.))
        .pt(t().d(38.))
        .pb(t().d(40.));

    doc = doc
        .child(
            div()
                .text_size(t().fs(10.))
                .text_color(rgb(t().p.faint))
                .child(match kind {
                    ElementKind::Groups => "组 · GROUP",
                    ElementKind::Packages => "包 · PACKAGE",
                }),
        )
        .child(
            div()
                .mt(t().d(10.))
                .text_size(t().fs(30.))
                .font_weight(FontWeight::LIGHT)
                .text_color(rgb(if element.broken() { t().p.danger } else { text_color() }))
                .child(if element.name().is_empty() {
                    "（还没名字）".to_string()
                } else {
                    element.name().to_string()
                }),
        )
        .child(
            div()
                .mt(t().d(8.))
                .max_w(t().d(640.))
                .text_size(t().fs(15.))
                .line_height(t().d(26.))
                .text_color(rgb(t().p.text_2))
                .child(element.desc().unwrap_or("").to_string()),
        )
        .child(
            div()
                .mt(t().d(12.))
                .flex()
                .gap(t().d(10.))
                .text_size(t().fs(10.5))
                .child(div().text_color(rgb(t().p.faint)).child(path))
                .child(div().text_color(rgb(fp_color)).child(fingerprint)),
        );

    if let Some(detail) = element.detail.as_deref() {
        doc = doc.child(
            div()
                .mt(t().d(14.))
                .max_w(t().d(640.))
                .p(t().d(10.))
                .rounded(t().r(7.))
                .bg(rgb(t().p.danger_soft))
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.danger_fg))
                .child(detail.to_string()),
        );
    }

    doc = doc.child(div().h(t().d(1.)).mt(t().d(30.)).mb(t().d(26.)).bg(rgb(line_color())));

    // 字段：ID 只读（它就是文件名，改名 = 新建 + 删旧的），别的都能改。
    doc = doc.child(field_label("ID · 等于文件名"));
    doc = doc.child(
        div()
            .mt(t().d(5.))
            .text_size(t().fs(12.))
            .font_family("JetBrainsMono")
            .text_color(rgb(t().p.text_2))
            .child(element.id.clone()),
    );

    doc = doc.child(div().mt(t().d(22.)).child(field_label("NAME")));
    doc = doc.child(div().mt(t().d(6.)).max_w(t().d(620.)).child(Input::new(&st.form_name).small()));

    doc = doc.child(div().mt(t().d(22.)).child(field_label("DESC")));
    doc = doc.child(div().mt(t().d(6.)).max_w(t().d(620.)).child(Input::new(&st.form_desc).small()));

    match kind {
        ElementKind::Groups => {
            doc = doc.child(
                div()
                    .mt(t().d(22.))
                    .child(field_label("PACKAGES · 引用的包 id，逗号分隔")),
            );
            doc = doc.child(
                div()
                    .mt(t().d(6.))
                    .max_w(t().d(620.))
                    .child(Input::new(&st.form_refs).small()),
            );
            // 断链点名（只显示引擎算出来的，不自己比对）。
            if let Some(data) = st.data.as_ref() {
                let missing = data.resolved.missing_in(&element.id);
                if !missing.is_empty() {
                    doc = doc.child(
                        div()
                            .mt(t().d(10.))
                            .flex()
                            .flex_wrap()
                            .gap(t().d(12.))
                            .text_size(t().fs(11.5))
                            .text_color(rgb(t().p.danger))
                            .children(missing.into_iter().map(|id| {
                                div()
                                    .child(format!("{id}（断链：packages/{id}.json 不存在）"))
                                    .into_any_element()
                            })),
                    );
                }
            }
        }
        ElementKind::Packages => {
            doc = doc.child(
                div()
                    .mt(t().d(22.))
                    .child(field_label("正文 · BODY")),
            );
            doc = doc.child(
                div()
                    .mt(t().d(6.))
                    .max_w(t().d(680.))
                    .h(t().d(160.))
                    .child(
                        Textarea::new(&st.form_body)
                            .size_full()
                            .font_family("JetBrainsMono"),
                    ),
            );
            doc = doc.child(div().mt(t().d(22.)).child(field_label("TAGS · 逗号分隔")));
            doc = doc.child(
                div()
                    .mt(t().d(6.))
                    .max_w(t().d(620.))
                    .child(Input::new(&st.form_tags).small()),
            );
            // 属于哪些组：引用是「组 → 包」单向的，这里只是把它反过来画一眼。
            if let Some(data) = st.data.as_ref() {
                let owners: Vec<String> = data
                    .groups
                    .iter()
                    .filter(|g| g.packages().iter().any(|p| *p == element.id))
                    .map(|g| g.name().to_string())
                    .collect();
                doc = doc.child(div().mt(t().d(22.)).child(field_label("属于")));
                doc = doc.child(
                    div()
                        .mt(t().d(6.))
                        .text_size(t().fs(12.))
                        .text_color(rgb(if owners.is_empty() { muted() } else { t().p.text_2 }))
                        .child(if owners.is_empty() {
                            "未归组".to_string()
                        } else {
                            owners.join(" · ")
                        }),
                );
            }
        }
    }

    doc.into_any_element()
}

fn field_label(text: &str) -> AnyElement {
    div()
        .text_size(t().fs(9.5))
        .text_color(rgb(t().p.faint))
        .child(text.to_string())
        .into_any_element()
}

/// 内容指纹太长，头 8 位够认人。
fn short_hash(etag: &str) -> String {
    etag.chars().take(8).collect()
}

/// JSON 视图：原文可改 + 底部一行本地校验。
///
/// 用 `Textarea` 而不是只读高亮：这一栏**是**写入口（demo 01 的 B 模式），
/// 只读的话「改一个未知字段」就只能去命令行。高亮交给语法着色那一栏（`render_json_preview`
/// 没有——这里就用等宽 + 主题色，选中复制靠 `Textarea` 自己的选区）。
fn render_json(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let st = &desk.elements_state;
    let err = st.json_error.clone();
    let lines = st
        .json
        .read(cx)
        .value()
        .lines()
        .count()
        .max(1);

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(t().d(0.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(t().d(8.))
                .px(t().d(28.))
                .pt(t().d(20.))
                .pb(t().d(10.))
                .text_size(t().fs(10.))
                .text_color(rgb(t().p.faint))
                .child("JSON · 与表单是同一份")
                .child(div().flex_1())
                .child(format!("{lines} 行")),
        )
        .child(
            div()
                .id("el-json")
                .flex_1()
                .min_h(t().d(0.))
                .mx(t().d(22.))
                .mb(t().d(10.))
                .child(
                    Textarea::new(&st.json)
                        .size_full()
                        .font_family("JetBrainsMono"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(t().d(14.))
                .px(t().d(28.))
                .py(t().d(9.))
                .border_t(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(10.5))
                .child(match err {
                    Some(e) => div().text_color(rgb(t().p.danger)).child(e),
                    None => div().text_color(rgb(t().p.ok)).child("JSON 能解析"),
                })
                .child(div().flex_1())
                .child(
                    div()
                        .text_color(rgb(muted()))
                        .child("保存会提交一条 · push 尽力而为"),
                ),
        )
        .into_any_element()
}

/// 删除的二次确认。盖在整页上，**不给**「回车就删」—— 不可逆的动作要多走一步。
fn render_delete_confirm(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let id = desk
        .elements_state
        .selected
        .as_ref()
        .map(|(_, id)| id.clone())
        .unwrap_or_default();
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(t().p.scrim))
        .child(
            div()
                .w(t().d(360.))
                .flex()
                .flex_col()
                .gap(t().d(12.))
                .p(t().d(20.))
                .rounded(t().r(10.))
                .bg(rgb(t().p.overlay))
                .shadow_lg()
                .child(
                    div()
                        .text_size(t().fs(13.))
                        .text_color(rgb(text_color()))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("删掉 {id}？")),
                )
                .child(
                    div()
                        .text_size(t().fs(11.5))
                        .line_h(t().d(18.))
                        .text_color(rgb(muted()))
                        .child("文件会从私库里删掉，并且提交一条。历史还在 git 里，但这页上看不到了。"),
                )
                .child(
                    div()
                        .flex()
                        .gap(t().d(8.))
                        .justify_end()
                        .child(
                            div()
                                .id("el-del-cancel")
                                .px(t().d(12.))
                                .py(t().d(7.))
                                .rounded(t().r(6.))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.line))
                                .text_size(t().fs(11.5))
                                .text_color(rgb(t().p.text_2))
                                .child("留下")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.elements_state.confirm_delete = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("el-del-ok")
                                .px(t().d(12.))
                                .py(t().d(7.))
                                .rounded(t().r(6.))
                                .bg(rgb(t().p.danger))
                                .text_size(t().fs(11.5))
                                .text_color(rgb(t().p.on_accent))
                                .child("删掉并提交")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.delete_element(cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}
