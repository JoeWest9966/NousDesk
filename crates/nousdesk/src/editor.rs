//! 模板编辑器（C1 工作台）。
//!
//! demo 定稿：`demo/template-page/nousdesk-editor.html`（左分组导航 / 中表单 / 右检查器 / 底状态栏）。
//!
//! ## 唯一真相是 JSON
//!
//! 盘上那份 `preset.json` 读进来直接变成 `serde_json::Value`（`draft`）。表单控件改的是
//! 这个 Value，JSON 那一栏显示的也是它 —— **控件的值是派生出来的，不是第二份状态**。
//! 未知键原样留着（AGENTS：`serde_json::Value` 收着透传），所以编辑器不会把引擎新加的
//! 字段吃掉。
//!
//! 保存走 `PUT /file`，`base` 是读到（或上次写回）的 etag：撞了就是 409，界面上「重读」
//! 而不是拿本地覆盖。发布是另一件事（commit + push），和保存分开两个按钮。

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::Icon;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use nous_engine::types::{PresetFile, PresetValidation};
use nous_engine::EngineError;

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 检查器四个 tab。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorTab {
    Json,
    Effective,
    Issues,
    Changes,
}

impl EditorTab {
    const ALL: [Self; 4] = [Self::Json, Self::Effective, Self::Issues, Self::Changes];

    fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Effective => "生效值",
            Self::Issues => "问题",
            Self::Changes => "改动",
        }
    }
}

/// 表单的分组（左导航）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    Model,
    Ctx,
    Ext,
    Perm,
    Run,
}

impl Group {
    const ALL: [Self; 5] = [Self::Model, Self::Ctx, Self::Ext, Self::Perm, Self::Run];

    fn label(self) -> &'static str {
        match self {
            Self::Model => "模型",
            Self::Ctx => "上下文",
            Self::Ext => "扩展",
            Self::Perm => "权限",
            Self::Run => "运行",
        }
    }

}

/// 一个文本字段正在编辑（弹层里那个输入框）。
pub(crate) struct TextEdit {
    /// 字段名（`summary` / `description` / `template` / `provider` / `modelId`）。
    pub key: String,
    pub input: Entity<InputState>,
}

/// 一个能挑的候选。`hint` 是右边那行小字（provider / 摘要 / 说明）。
#[derive(Debug, Clone)]
pub(crate) struct PickItem {
    pub name: String,
    pub hint: String,
    /// `true` = 引擎说这东西现在加载不出来 / 用不了（灰着，但还列出来）。
    pub off: bool,
}

impl PickItem {
    fn new(name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self { name: name.into(), hint: hint.into(), off: false }
    }
}

/// 挑选取值的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickKind {
    Provider,
    Model,
    Prompt,
    Skill,
    Category,
    /// 没有候选来源（`extensions`）：只有「手打一个新值」。
    Free,
}

impl PickKind {
    fn title(self) -> &'static str {
        match self {
            Self::Provider => "选供应商",
            Self::Model => "选模型",
            Self::Prompt => "选会话模板",
            Self::Skill => "选技能",
            Self::Category => "选类别",
            Self::Free => "填一个",
        }
    }
}

/// 候选池 —— 打开编辑器时拉一次，切组切字段都不再请求引擎。
///
/// 拉不到就空着（`err` 记一句），**不让编辑器因此打不开**：控件退回手打。
#[derive(Debug, Default)]
pub(crate) struct Candidates {
    /// `(provider, modelId, 显示名, 这模型支持的思考档位)`
    pub models: Vec<(String, String, String, Vec<String>)>,
    pub prompts: Vec<PickItem>,
    pub skills: Vec<PickItem>,
    pub categories: Vec<String>,
    pub err: Option<String>,
}

/// 挑值弹层（字段值点开那个）。
pub(crate) struct PickerState {
    pub kind: PickKind,
    /// 写回用的键路径，`model.provider` 这种。
    pub path: &'static [&'static str],
    /// 多选（chips）还是单选。
    pub multi: bool,
    pub filter: Entity<InputState>,
}

/// 编辑器状态。`open == false` 时整个模块不画。
pub(crate) struct EditorState {
    pub open: bool,
    pub section: String,
    pub name: String,
    /// 盘上的原文（用来算「改了什么」，也用来重读）。
    pub text: String,
    /// 读到 / 上次写回时的 etag。
    pub etag: String,
    /// 正在编辑的那份（唯一真相）。
    pub draft: serde_json::Value,
    pub loading: bool,
    pub saving: bool,
    pub publishing: bool,
    /// 任何一处的错误（读失败 / 写失败 / 409）。
    pub err: Option<String>,
    /// 409 单独记一下：文案要说「重读」而不是「重试」。
    pub conflict: bool,
    pub tab: EditorTab,
    pub group: Group,
    pub validation: Option<PresetValidation>,
    pub validating: bool,
    pub text_edit: Option<TextEdit>,
    /// 发布对话框开着没有（里面那份 commit message）。
    pub publish_msg: Option<Entity<InputState>>,
    /// 挑值弹层。
    pub picker: Option<PickerState>,
    /// 候选池。
    pub candidates: Candidates,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            open: false,
            section: String::new(),
            name: String::new(),
            text: String::new(),
            etag: String::new(),
            draft: serde_json::Value::Null,
            loading: false,
            saving: false,
            publishing: false,
            err: None,
            conflict: false,
            tab: EditorTab::Json,
            group: Group::Model,
            validation: None,
            validating: false,
            text_edit: None,
            publish_msg: None,
            picker: None,
            candidates: Candidates::default(),
        }
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 有没有没保存的改动。和盘上那份原文比字节 —— JSON 键顺序会被 `to_string` 改，
    /// 所以比的是 **Value 相等**，不是字符串相等。
    pub fn dirty(&self) -> bool {
        !self.loading && is_dirty(&self.text, &self.draft)
    }

    /// 问题条数（错误 + 警告）。
    pub fn issue_count(&self) -> (usize, usize) {
        match &self.validation {
            Some(v) => (v.errors.len(), v.warnings.len()),
            None => (0, 0),
        }
    }

    /// 读一份字段。`path` 是键路径，缺了就给空串。
    pub fn text_of(&self, path: &[&str]) -> String {
        read_text(&self.draft, path)
    }

    /// 写一份字段。空串 = 删掉这个键（「不钉」就是真的不钉，不是空字符串）。
    pub fn set_text(&mut self, path: &[&str], value: &str) {
        write_text(&mut self.draft, path, value);
    }

    pub fn bool_of(&self, path: &[&str]) -> bool {
        read_bool(&self.draft, path)
    }

    /// 开关。**关掉 = 删掉这个键**（跟「重置」一个哲学：`false` 就是默认，别在 JSON 里留一行噪音）。
    pub fn set_bool(&mut self, path: &[&str], on: bool) {
        write_bool(&mut self.draft, path, on);
    }

    /// 从 draft 里删掉一个键（支持 `model.provider` 这种嵌套路径）= 回到默认。
    pub fn remove_key(&mut self, dotted: &str) {
        del_key(&mut self.draft, dotted);
    }

    /// 字符串数组（技能 / 类别）。
    pub fn list_of(&self, key: &str) -> Vec<String> {
        read_list(&self.draft, key)
    }

    /// 数组里加一项 / 删一项。
    pub fn toggle_in_list(&mut self, key: &str, item: &str) {
        toggle_list(&mut self.draft, key, item);
    }

    /// JSON 那一栏显示的东西：格式化的 draft，带行号。
    pub fn pretty_lines(&self) -> Vec<String> {
        let text = serde_json::to_string_pretty(&self.draft).unwrap_or_default();
        text.lines().map(|l| l.to_string()).collect()
    }
}

// ── 纯 JSON 操作 ───────────────────────────────────────────
//
// 这一层**不吃 `EditorState`**，只吃 `serde_json::Value`：编辑器的所有写入都从这里走，
// 测试也测这里（`EditorState` 带着一堆 Entity，`#[test]` 展开会撞递归上限）。

/// 按路径读一份字符串。缺了 / 不是标量就给空串。
fn read_text(v: &serde_json::Value, path: &[&str]) -> String {
    let mut cur = v;
    for k in path {
        match cur.get(*k) {
            Some(n) => cur = n,
            None => return String::new(),
        }
    }
    match cur {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 按路径写一份字符串。**空串 = 删键**（「不钉」就是真不钉，不是空字符串）。
fn write_text(v: &mut serde_json::Value, path: &[&str], value: &str) {
    if path.is_empty() {
        return;
    }
    if v.is_null() {
        *v = serde_json::json!({});
    }
    let mut cur = v;
    for k in &path[..path.len() - 1] {
        if !cur.is_object() {
            *cur = serde_json::json!({});
        }
        cur = cur
            .as_object_mut()
            .unwrap()
            .entry((*k).to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
    let last = path[path.len() - 1];
    let Some(obj) = cur.as_object_mut() else {
        return;
    };
    if value.trim().is_empty() {
        obj.remove(last);
    } else {
        obj.insert(last.to_string(), serde_json::Value::String(value.to_string()));
    }
}

fn read_bool(v: &serde_json::Value, path: &[&str]) -> bool {
    let mut cur = v;
    for k in path {
        match cur.get(*k) {
            Some(n) => cur = n,
            None => return false,
        }
    }
    cur.as_bool().unwrap_or(false)
}

/// 开关。**关掉 = 删键**：`false` 就是默认，别在 JSON 里留一行噪音。
fn write_bool(v: &mut serde_json::Value, path: &[&str], on: bool) {
    if !on {
        del_key(v, &path.join("."));
        return;
    }
    if path.is_empty() {
        return;
    }
    if v.is_null() {
        *v = serde_json::json!({});
    }
    if let Some(obj) = v.as_object_mut() {
        obj.insert(path[0].to_string(), serde_json::Value::Bool(true));
    }
}

/// 按点号路径删一个键（`model.provider`）。父对象留着 —— 删的是那一个键。
fn del_key(v: &mut serde_json::Value, dotted: &str) {
    let parts: Vec<&str> = dotted.split('.').collect();
    if parts.is_empty() || v.is_null() {
        return;
    }
    let mut cur = v;
    for k in &parts[..parts.len() - 1] {
        match cur.get_mut(*k) {
            Some(n) => cur = n,
            None => return,
        }
    }
    if let Some(obj) = cur.as_object_mut() {
        obj.remove(parts[parts.len() - 1]);
    }
}

fn read_list(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// 数组里加一项 / 删一项（成员就是取消，不是重复）。
fn toggle_list(v: &mut serde_json::Value, key: &str, item: &str) {
    if v.is_null() {
        *v = serde_json::json!({});
    }
    let Some(obj) = v.as_object_mut() else {
        return;
    };
    let mut list = obj
        .get(key)
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    if let Some(ix) = list
        .iter()
        .position(|x| x.as_str().map(|s| s == item).unwrap_or(false))
    {
        list.remove(ix);
    } else {
        list.push(serde_json::Value::String(item.to_string()));
    }
    obj.insert(key.to_string(), serde_json::Value::Array(list));
}

/// 有没有没保存的改动。比的是 **Value 相等**，不是字符串 —— JSON 键顺序会被
/// `to_string` 改，按字节比会永远「脏」。
fn is_dirty(text: &str, draft: &serde_json::Value) -> bool {
    if text.is_empty() {
        return false;
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(saved) => saved != *draft,
        Err(_) => true,
    }
}

// ── 数据 ──────────────────────────────────────────────────

impl crate::app::Desktop {
    /// 打开编辑器：读盘上那份 `preset.json`。
    pub(crate) fn open_editor(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let section = crate::templates::SECTIONS[self.section].0.to_string();
        self.editor = EditorState {
            open: true,
            section: section.clone(),
            name: name.clone(),
            loading: true,
            ..EditorState::new()
        };
        self.picked_preset = Some(name.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result: Option<Result<PresetFile, EngineError>> = match &handle {
                Some(h) => h
                    .spawn(async move { engine.preset_file(&section, &name, "preset.json").await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.editor.loading = false;
                match result {
                    Some(Ok(f)) => {
                        this.editor.text = f.text.clone();
                        this.editor.etag = f.etag.clone();
                        this.editor.draft = serde_json::from_str(&f.text).unwrap_or_else(|e| {
                            this.editor.err = Some(format!("preset.json 读不了：{e}"));
                            serde_json::Value::Null
                        });
                        this.editor.err = None;
                        this.editor.conflict = false;
                        this.validate_preset(cx);
                        this.load_candidates(cx);
                    }
                    Some(Err(e)) => this.editor.err = Some(format!("读不到 preset.json：{e}")),
                    None => this.editor.err = Some("任务没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 这个字段能挑的候选。`Model` 会按已经选中的供应商过滤。
    pub(crate) fn pick_items(&self, kind: PickKind) -> Vec<PickItem> {
        let c = &self.editor.candidates;
        match kind {
            PickKind::Provider => {
                let mut seen: Vec<String> = Vec::new();
                let mut out = Vec::new();
                for (prov, _, _, _) in &c.models {
                    if seen.iter().any(|s| s == prov) {
                        continue;
                    }
                    seen.push(prov.clone());
                    let n = c.models.iter().filter(|(p, ..)| p == prov).count();
                    out.push(PickItem::new(prov.clone(), format!("{n} 个模型")));
                }
                out
            }
            PickKind::Model => {
                let want = self.editor.text_of(&["model", "provider"]);
                c.models
                    .iter()
                    .filter(|(p, ..)| want.is_empty() || *p == want)
                    .map(|(p, id, label, levels)| PickItem {
                        name: id.clone(),
                        hint: format!("{p} · {} · {}", levels.join("/"), label),
                        off: false,
                    })
                    .collect()
            }
            PickKind::Prompt => c.prompts.clone(),
            PickKind::Skill => c.skills.clone(),
            PickKind::Category => c
                .categories
                .iter()
                .map(|n| PickItem::new(n.clone(), ""))
                .collect(),
            PickKind::Free => Vec::new(),
        }
    }

    /// 打开挑值弹层。
    pub(crate) fn open_picker(
        &mut self,
        kind: PickKind,
        path: &'static [&'static str],
        multi: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match kind {
                PickKind::Free => "打一个新值",
                _ => "筛选…",
            })
        });
        // 每敲一个字重排一次（纯本地过滤）。
        cx.subscribe_in(&input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        self.editor.picker = Some(PickerState { kind, path, multi, filter: input });
        cx.notify();
    }

    /// 挑中一个。
    pub(crate) fn pick_apply(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(p) = &self.editor.picker else {
            return;
        };
        let (path, multi) = (p.path, p.multi);
        if multi {
            // 已是成员就删掉（再点是取消），不是就加上 —— 弹层不关，能连着挑。
            self.editor.toggle_in_list(path[0], name);
        } else {
            self.editor.set_text(path, name);
            self.editor.picker = None;
        }
        cx.notify();
    }

    /// 拉一次候选池：模型 / 会话模板 / 技能 / 本段类别。
    ///
    /// 四个请求并行发；任何一个失败都不挡编辑器 —— 只是那一类退回「手打」。
    fn load_candidates(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let section = self.editor.section.clone();
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let out = h
                .spawn(async move {
                    let models = engine.models().await.ok();
                    let prompts = engine.prompts().await.ok();
                    let skills = engine.skills().await.ok();
                    let cats = engine.presets(&section).await.ok().map(|p| p.categories);
                    (models, prompts, skills, cats)
                })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.editor.candidates;
                if let Some((models, prompts, skills, cats)) = out {
                    if let Some(m) = models {
                        c.models = m
                            .items
                            .iter()
                            .map(|it| {
                                (
                                    it.provider.clone(),
                                    it.model_id.clone(),
                                    it.name.clone().unwrap_or_else(|| it.model_id.clone()),
                                    it.levels(),
                                )
                            })
                            .collect();
                    }
                    if let Some(p) = prompts {
                        c.prompts = p
                            .prompts
                            .iter()
                            .map(|it| {
                                PickItem::new(
                                    it.name.clone(),
                                    it.summary.clone().or(it.description.clone()).unwrap_or_default(),
                                )
                            })
                            .collect();
                    }
                    if let Some(sk) = skills {
                        c.skills = sk
                            .skills
                            .iter()
                            .map(|it| PickItem {
                                name: it.name.clone(),
                                hint: it.description.clone().unwrap_or_default(),
                                // 引擎说加载不出来 / 隐藏的，勾了也不动 —— 灰着列出来，别让人白勾。
                                off: it.hidden
                                    || it
                                        .load
                                        .as_ref()
                                        .and_then(|l| l.get("state"))
                                        .and_then(|v| v.as_str())
                                        .is_some_and(|st| st != "ok"),
                            })
                            .collect();
                    }
                    if let Some(cats) = cats {
                        c.categories = cats;
                    }
                    // 什么都拉不到才记一句（部分成功算成功）。
                    if c.models.is_empty() && c.prompts.is_empty() && c.skills.is_empty() {
                        c.err = Some("候选列表没拉到 —— 下面这些还是能手打".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 关掉编辑器。有未保存的改动时先问一句（返回 `false` = 别关）。
    pub(crate) fn close_editor(&mut self, force: bool, cx: &mut Context<Self>) -> bool {
        if !force && self.editor.dirty() {
            self.notify("有没保存的改动 —— 再按一次返回放弃", crate::app::NoticeKind::Error, cx);
            return false;
        }
        self.editor = EditorState::new();
        cx.notify();
        true
    }

    /// 让引擎校验这份草稿。用盘上那份（保存后再校验），所以只在读/写之后调。
    fn validate_preset(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let section = self.editor.section.clone();
        let name = self.editor.name.clone();
        self.editor.validating = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move { engine.validate_preset(&section, &name).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.editor.validating = false;
                if let Some(Ok(v)) = result {
                    this.editor.validation = Some(v);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 保存：只落盘（`PUT /file`，base = etag）。409 = 别人先改了。
    pub(crate) fn save_editor(&mut self, cx: &mut Context<Self>) {
        if self.editor.saving || self.editor.loading {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let section = self.editor.section.clone();
        let name = self.editor.name.clone();
        let base = self.editor.etag.clone();
        let text = serde_json::to_string_pretty(&self.editor.draft).unwrap_or_default() + "\n";
        let text_for_req = text.clone();
        self.editor.saving = true;
        self.editor.err = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move {
                        engine
                            .put_preset_file(&section, &name, "preset.json", &text_for_req, &base)
                            .await
                    })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.editor.saving = false;
                match result {
                    Some(Ok(v)) => {
                        // 写回成功：正文换成刚写的那份，etag 换成新的（拿不到就用返回值里的）。
                        this.editor.text = text.clone();
                        if let Some(etag) = v.get("etag").and_then(|e| e.as_str()) {
                            this.editor.etag = etag.to_string();
                        }
                        this.editor.conflict = false;
                        this.notify("已保存到盘上 · 还没提交", crate::app::NoticeKind::Info, cx);
                        this.validate_preset(cx);
                        this.load_section(this.section, true, cx);
                    }
                    Some(Err(EngineError::Engine { status: 409, message })) => {
                        this.editor.conflict = true;
                        this.editor.err = Some(format!("别人先改了这条（409）：{message}"));
                    }
                    Some(Err(e)) => this.editor.err = Some(format!("保存失败：{e}")),
                    None => this.editor.err = Some("任务没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 发布：commit + push。和保存分开 —— 这个会真的动仓库。
    pub(crate) fn publish_editor(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.editor.publish_msg.clone() else {
            return;
        };
        if self.editor.publishing {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let section = self.editor.section.clone();
        let msg = input.read(cx).value().trim().to_string();
        let msg = if msg.is_empty() {
            format!("调整 {}", self.editor.name)
        } else {
            msg
        };
        self.editor.publishing = true;
        self.editor.publish_msg = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move { engine.publish_preset(&section, &msg).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.editor.publishing = false;
                match result {
                    Some(Ok(v)) => {
                        let hash = v
                            .get("commit")
                            .and_then(|c| c.get("hash"))
                            .and_then(|h| h.as_str())
                            .unwrap_or("")
                            .to_string();
                        this.notify(
                            if hash.is_empty() {
                                "已发布".to_string()
                            } else {
                                format!("已发布 · {hash}")
                            },
                            crate::app::NoticeKind::Info,
                            cx,
                        );
                        this.load_section(this.section, true, cx);
                    }
                    Some(Err(e)) => this.notify(format!("发布失败：{e}"), crate::app::NoticeKind::Error, cx),
                    None => this.notify("任务没跑完", crate::app::NoticeKind::Error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 重新从盘上读（409 之后、或者被外部改过）。
    pub(crate) fn reload_editor(&mut self, cx: &mut Context<Self>) {
        if self.editor.name.is_empty() {
            return;
        }
        let name = self.editor.name.clone();
        self.open_editor(name, cx);
    }

    /// 打开某个文本字段的输入弹层。
    pub(crate) fn edit_text_field(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = self.editor.text_of(&[key]);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("留空 = 不写这个键")
                .default_value(value)
        });
        self.editor.text_edit = Some(TextEdit {
            key: key.to_string(),
            input,
        });
        cx.notify();
    }

    /// 输入弹层里点「保存」。
    pub(crate) fn commit_text_field(&mut self, cx: &mut Context<Self>) {
        let Some(te) = self.editor.text_edit.take() else {
            return;
        };
        let v = te.input.read(cx).value().to_string();
        self.editor.set_text(&[te.key.as_str()], v.trim());
        cx.notify();
    }
}

// ── 渲染 ──────────────────────────────────────────────────

/// 一个分组下要画的字段。`(题、键、控件种类)`。
enum Ctl {
    /// 文本 → 弹输入
    Text,
    /// 三/五档
    Seg(&'static [&'static str]),
    /// 开 / 关
    Switch,
    /// 单选：从候选池里挑一个（写回字符串）
    Pick(PickKind),
    /// 多选：chips + 「＋」（写回字符串数组）
    Multi(PickKind),
}

/// 一行字段：标题 + JSON 键路径 + 控件。路径是**嵌套**的（`model.provider`）。
struct FldSpec {
    label: &'static str,
    path: &'static [&'static str],
    ctl: Ctl,
}

impl crate::app::Desktop {
    pub(crate) fn render_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.editor;
        let (errs, warns) = e.issue_count();
        let dirty = e.dirty();

        let bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(8.))
            .h(t().d(46.))
            .px(t().d(14.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .id("ed-back")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(26.))
                    .rounded(t().r(6.))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.close_editor(false, cx);
                    }))
                    .child(Icon::new(IconName::ArrowLeft).size(t().d(14.)).text_color(rgb(t().p.muted))),
            )
            .child(div().text_size(t().fs(14.)).child(e.name.clone()))
            .child(
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.faint))
                    .child(e.section.clone()),
            )
            .children(if dirty {
                Some(pill(format!("未提交 · {} 处改动", changed_labels(e).len()), t().p.warn))
            } else {
                Some(pill("已保存".to_string(), t().p.ok))
            })
            .children((errs > 0).then(|| pill(format!("{errs} 错"), t().p.danger)))
            .child(div().flex_1())
            .child(
                div()
                    .id("ed-reload")
                    .px(t().d(9.))
                    .py(t().d(4.))
                    .rounded(t().r(6.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload_editor(cx)))
                    .child("重扫"),
            )
            .child(
                div()
                    .id("ed-save")
                    .flex()
                    .items_center()
                    .gap(t().d(5.))
                    .px(t().d(10.))
                    .py(t().d(4.))
                    .rounded(t().r(6.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .text_size(t().fs(11.5))
                    .text_color(rgb(t().p.text_2))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save_editor(cx)))
                    .child(if e.saving { "保存中…" } else { "保存" })
                    .child(
                        div()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.faint))
                            .child("Ctrl+S"),
                    ),
            )
            .child(
                div()
                    .id("ed-publish")
                    .px(t().d(11.))
                    .py(t().d(5.))
                    .rounded(t().r(6.))
                    .bg(rgb(t().p.accent))
                    .text_size(t().fs(11.5))
                    .text_color(rgb(t().p.on_accent))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.accent_hover)))
                    .on_click(cx.listener(|this, _, window, cx| this.open_publish_dialog(window, cx)))
                    .child(if e.publishing { "发布中…" } else { "发布…" }),
            );

        let body = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(t().d(0.))
            .child(self.editor_nav(cx))
            .child(self.editor_form(cx))
            .child(self.editor_inspector(cx));

        let status = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(8.))
            .h(t().d(30.))
            .px(t().d(14.))
            .border_t(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .bg(rgb(t().p.panel))
            .text_size(t().fs(10.5))
            .text_color(rgb(t().p.muted))
            .child(format!("{} / {}", e.section, e.name))
            .child(div().text_color(rgb(t().p.faint)).child("·"))
            .child(if dirty {
                format!("{} 处改动没保存", changed_labels(e).len())
            } else {
                "和盘上一致".to_string()
            })
            .child(div().text_color(rgb(t().p.faint)).child("·"))
            .child(
                div()
                    .text_color(rgb(if errs > 0 { t().p.danger } else { t().p.muted }))
                    .child(format!("{} 错 · {} 警", errs, warns)),
            )
            .child(div().flex_1())
            .child("保存 = 落盘，发布 = commit + push");

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_w(t().d(0.))
            .bg(rgb(t().p.list_bg))
            .child(bar)
            .children(e.err.clone().map(|msg| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(14.))
                    .py(t().d(6.))
                    .bg(rgb(t().p.danger_soft))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.danger_fg))
                    .child(msg)
                    .children(e.conflict.then(|| {
                        div()
                            .id("ed-reread")
                            .ml(t().d(8.))
                            .px(t().d(8.))
                            .py(t().d(2.))
                            .rounded(t().r(4.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.danger))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload_editor(cx)))
                            .child("重读盘上那份")
                    }))
            }))
            .child(body)
            .child(status)
            .into_any_element()
    }

    /// 左：分组 + 状态。
    fn editor_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        let (errs, _) = self.editor.issue_count();
        let mut nav = div()
            .flex()
            .flex_col()
            .w(t().d(206.))
            .flex_none()
            .bg(rgb(t().p.panel_alt))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(section_label("分组"));
        let mut grp = div().flex().flex_col().p(t().d(5.));
        for g in Group::ALL {
            let on = self.editor.group == g;
            grp = grp.child(
                div()
                    .id(ElementId::Name(format!("ed-g-{:?}", g).into()))
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(9.))
                    .py(t().d(5.))
                    .rounded(t().r(6.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.panel)))
                    .hover(|h| h.bg(rgb(t().p.panel)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.editor.group = g;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.faint))
                            .child(glyph(g)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(if on { t().p.text } else { t().p.text_2 }))
                            .child(g.label()),
                    ),
            );
        }
        nav = nav.child(grp).child(
            div()
                .h(t().d(1.))
                .mx(t().d(10.))
                .bg(rgb(t().p.line_soft)),
        );

        let mut st = div().flex().flex_col().p(t().d(5.));
        for (label, count, color, tab) in [
            ("待提交", changed_labels(&self.editor).len(), t().p.warn, EditorTab::Changes),
            ("问题", errs, t().p.danger, EditorTab::Issues),
            ("生效值", 0, t().p.faint, EditorTab::Effective),
        ] {
            st = st.child(
                div()
                    .id(ElementId::Name(format!("ed-s-{label}").into()))
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(9.))
                    .py(t().d(5.))
                    .rounded(t().r(6.))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.panel)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.editor.tab = tab;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.faint))
                            .child(match label {
                                "待提交" => "✎",
                                "问题" => "!",
                                _ => "↯",
                            }),
                    )
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.text_2))
                            .child(label),
                    )
                    .children((count > 0).then(|| {
                        div()
                            .ml_auto()
                            .text_size(t().fs(10.))
                            .font_family("JetBrainsMono")
                            .text_color(rgb(color))
                            .child(count.to_string())
                    })),
            );
        }
        nav.child(section_label("状态")).child(st).child(div().flex_1()).into_any_element()
    }

    /// 中：表单。
    fn editor_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.editor;
        let mut col = div()
            .id("ed-form")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(t().d(0.))
            .overflow_y_scroll();

        if e.loading {
            col = col.child(note("读 preset.json 中…"));
            return col.into_any_element();
        }
        if e.draft.is_null() {
            col = col.child(note("这份 preset.json 读不了，去 JSON 那一栏看看原文。"));
            return col.into_any_element();
        }

        for g in Group::ALL {
            if g != self.editor.group {
                continue;
            }
            let items: Vec<FldSpec> = match g {
                Group::Model => vec![
                    FldSpec { label: "供应商", path: &["model", "provider"], ctl: Ctl::Pick(PickKind::Provider) },
                    FldSpec { label: "模型", path: &["model", "modelId"], ctl: Ctl::Pick(PickKind::Model) },
                    FldSpec {
                        label: "思考档位",
                        path: &["thinkingLevel"],
                        ctl: Ctl::Seg(&["off", "minimal", "low", "medium", "high"]),
                    },
                ],
                Group::Ctx => vec![
                    FldSpec { label: "技能", path: &["skills"], ctl: Ctl::Multi(PickKind::Skill) },
                    FldSpec { label: "会话模板", path: &["template"], ctl: Ctl::Pick(PickKind::Prompt) },
                ],
                Group::Ext => vec![FldSpec { label: "扩展", path: &["extensions"], ctl: Ctl::Multi(PickKind::Free) }],
                Group::Perm => vec![
                    FldSpec { label: "只读", path: &["readOnly"], ctl: Ctl::Switch },
                    FldSpec { label: "类别", path: &["category"], ctl: Ctl::Multi(PickKind::Category) },
                ],
                Group::Run => vec![
                    FldSpec { label: "摘要", path: &["summary"], ctl: Ctl::Text },
                    FldSpec { label: "说明", path: &["description"], ctl: Ctl::Text },
                ],
            };

            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(16.))
                    .pt(t().d(12.))
                    .pb(t().d(4.))
                    .child(div().text_size(t().fs(11.)).child(g.label()))
                    .child(
                        div()
                            .ml_auto()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.faint))
                            .child(format!("{} 项", items.len())),
                    ),
            );
            for spec in items {
                col = col.child(self.editor_field(spec, cx));
            }
        }

        col.child(
            div()
                .px(t().d(16.))
                .py(t().d(14.))
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.faint))
                .child("控件只画常用的几个键；别的键原样留在 JSON 里，去那一栏改。"),
        )
        .into_any_element()
    }

    /// 一行字段。`key` 是 JSON 里的键名（改动列表也按它显示）。
    fn editor_field(&self, spec: FldSpec, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.editor;
        let label = spec.label.to_string();
        let path = spec.path;
        let key = path.join(".");
        let saved = serde_json::from_str::<serde_json::Value>(&e.text).ok();
        let changed = saved
            .as_ref()
            .map(|s| value_at(s, path) != value_at(&e.draft, path))
            .unwrap_or(false);

        let control: AnyElement = match spec.ctl {
            Ctl::Text => {
                let v = e.text_of(path);
                let k = key.clone();
                let shown = if v.is_empty() { "（空）".to_string() } else { v };
                div()
                    .id(ElementId::Name(format!("ed-t-{k}").into()))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(t().d(8.))
                    .px(t().d(8.))
                    .py(t().d(3.))
                    .rounded(t().r(6.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .bg(rgb(t().p.panel))
                    .cursor_pointer()
                    .hover(|h| h.border_color(rgb(t().p.muted)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_text_field(&k, window, cx)
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.5))
                            .font_family("JetBrainsMono")
                            .text_color(rgb(if shown == "（空）" {
                                t().p.faint
                            } else {
                                t().p.text_2
                            }))
                            .child(shown),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.faint))
                            .child("✎"),
                    )
                    .into_any_element()
            }
            Ctl::Seg(opts) => {
                let cur = e.text_of(path);
                let k = key.clone();
                let mut seg = div()
                    .flex()
                    .flex_row()
                    .rounded(t().r(6.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .overflow_hidden();
                for o in opts {
                    let on = cur == *o;
                    seg = seg.child(
                        div()
                            .id(ElementId::Name(format!("ed-o-{k}-{o}").into()))
                            .px(t().d(9.))
                            .py(t().d(2.))
                            .text_size(t().fs(11.))
                            .cursor_pointer()
                            .when(on, |el| {
                                el.bg(rgb(t().p.accent)).text_color(rgb(t().p.on_accent))
                            })
                            .when(!on, |el| {
                                el.text_color(rgb(t().p.muted))
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                            })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.editor.set_text(path, o);
                                cx.notify();
                            }))
                            .child(*o),
                    );
                }
                seg.into_any_element()
            }
            Ctl::Switch => {
                let on = e.bool_of(path);
                let k = key.clone();
                let knob = div()
                    .w(t().d(26.))
                    .h(t().d(15.))
                    .rounded(t().d(8.))
                    .bg(rgb(if on { t().p.accent } else { t().p.line }))
                    .p(t().d(2.))
                    .flex()
                    .when(on, |el| el.justify_end())
                    .child(
                        div()
                            .size(t().d(11.))
                            .rounded(t().d(6.))
                            .bg(rgb(if on { t().p.on_accent } else { t().p.panel })),
                    );
                div()
                    .id(ElementId::Name(format!("ed-sw-{k}").into()))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let v = this.editor.bool_of(path);
                        this.editor.set_bool(path, !v);
                        cx.notify();
                    }))
                    .child(knob)
                    .into_any_element()
            }
            Ctl::Pick(kind) => {
                let v = e.text_of(path);
                let empty = v.is_empty();
                let shown = if empty { "（未选）".to_string() } else { v };
                let k = key.clone();
                div()
                    .id(ElementId::Name(format!("ed-pk-{k}").into()))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(t().d(8.))
                    .px(t().d(8.))
                    .py(t().d(3.))
                    .rounded(t().r(6.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .bg(rgb(t().p.panel))
                    .cursor_pointer()
                    .hover(|h| h.border_color(rgb(t().p.muted)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_picker(kind, path, false, window, cx)
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.5))
                            .font_family("JetBrainsMono")
                            .text_color(rgb(if empty { t().p.faint } else { t().p.text_2 }))
                            .child(shown),
                    )
                    .child(div().text_size(t().fs(9.)).text_color(rgb(t().p.faint)).child("▾"))
                    .into_any_element()
            }
            Ctl::Multi(kind) => {
                let items = e.list_of(path[0]);
                let k = path[0].to_string();
                let mut row = div().flex().flex_row().items_center().gap(t().d(5.)).flex_wrap();
                for it in &items {
                    let name = it.clone();
                    let kk = k.clone();
                    row = row.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .px(t().d(7.))
                            .py(t().d(1.))
                            .rounded(t().d(3.))
                            .bg(rgb(t().p.hover))
                            .text_size(t().fs(10.))
                            .child(name.clone())
                            .child(
                                div()
                                    .id(ElementId::Name(format!("ed-x-{k}-{name}").into()))
                                    .cursor_pointer()
                                    .text_color(rgb(t().p.faint))
                                    .hover(|h| h.text_color(rgb(t().p.danger)))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.editor.toggle_in_list(&kk, &name);
                                        cx.notify();
                                    }))
                                    .child("✕"),
                            ),
                    );
                }
                // 「＋」就是要挑的入口：候选池里有就挑，没有（扩展）就手打。
                let kk = key.clone();
                row = row.child(
                    div()
                        .id(ElementId::Name(format!("ed-add-{kk}").into()))
                        .flex()
                        .items_center()
                        .gap(t().d(4.))
                        .px(t().d(7.))
                        .py(t().d(1.))
                        .rounded(t().d(3.))
                        .border(t().d(1.))
                        .border_color(rgb(t().p.line))
                        .text_size(t().fs(10.))
                        .text_color(rgb(t().p.muted))
                        .cursor_pointer()
                        .hover(|h| {
                            h.border_color(rgb(t().p.muted)).text_color(rgb(t().p.text))
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_picker(kind, path, true, window, cx)
                        }))
                        .child("＋"),
                );
                row.into_any_element()
            }
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(10.))
            .px(t().d(16.))
            .py(t().d(5.))
            .hover(|h| h.bg(rgb(t().p.panel_alt)))
            .child(
                div()
                    .w(t().d(96.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(t().d(5.))
                    .child(
                        div()
                            .text_size(t().fs(11.5))
                            .text_color(rgb(t().p.muted))
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.))
                            .px(t().d(3.))
                            .rounded(t().d(2.))
                            .border(t().d(1.))
                            .border_color(rgb(if changed { t().p.accent } else { t().p.line }))
                            .text_color(rgb(if changed { t().p.accent_fg } else { t().p.faint }))
                            .child(if changed { "改" } else { "默认" }),
                    ),
            )
            .child(div().flex_1().min_w(t().d(0.)).child(control))
            // 「重置」= 把这个键从 draft 里删掉。JSON 里没有 = 走默认，比写个空值干净。
            .children(changed.then(|| {
                let k = key.clone();
                div()
                    .id(ElementId::Name(format!("ed-reset-{k}").into()))
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .cursor_pointer()
                    .hover(|h| h.text_color(rgb(t().p.accent_fg)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.editor.remove_key(&k);
                        cx.notify();
                    }))
                    .child("↺ 重置")
            }))
            .into_any_element()
    }

    /// 右：检查器。
    fn editor_inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.editor;
        let (errs, warns) = e.issue_count();
        let mut tabs = div()
            .flex()
            .flex_row()
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .bg(rgb(t().p.panel));
        for tab in EditorTab::ALL {
            let on = e.tab == tab;
            let badge = match tab {
                EditorTab::Issues => errs + warns,
                EditorTab::Changes => changed_labels(e).len(),
                _ => 0,
            };
            tabs = tabs.child(
                div()
                    .id(ElementId::Name(format!("ed-tab-{:?}", tab).into()))
                    .flex()
                    .items_center()
                    .gap(t().d(3.))
                    .px(t().d(10.))
                    .py(t().d(7.))
                    .border_b(t().d(2.))
                    .border_color(rgb(if on { t().p.accent } else { t().p.panel }))
                    .text_size(t().fs(11.))
                    .text_color(rgb(if on { t().p.text } else { t().p.muted }))
                    .cursor_pointer()
                    .hover(|h| h.text_color(rgb(t().p.text)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.editor.tab = tab;
                        cx.notify();
                    }))
                    .child(tab.label())
                    .children((badge > 0).then(|| {
                        div()
                            .text_size(t().fs(9.5))
                            .font_family("JetBrainsMono")
                            .text_color(rgb(t().p.faint))
                            .child(badge.to_string())
                    })),
            );
        }

        let body: AnyElement = match e.tab {
            EditorTab::Json => {
                let mut code = div()
                    .id("ed-json")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .py(t().d(4.))
                    .overflow_y_scroll();
                let bad_line = json_error_line(e);
                for (i, line) in e.pretty_lines().iter().enumerate() {
                    let n = i + 1;
                    code = code.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(t().d(8.))
                            .px(t().d(10.))
                            .when(bad_line == Some(n), |el| el.bg(rgb(t().p.danger_soft)))
                            .child(
                                div()
                                    .w(t().d(20.))
                                    .flex_none()
                                    .text_size(t().fs(9.5))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(t().p.faint))
                                    .child(n.to_string()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(t().p.text_2))
                                    .whitespace_nowrap()
                                    .child(line.clone()),
                            ),
                    );
                }
                code.into_any_element()
            }
            EditorTab::Effective => {
                let mut col = div()
                    .id("ed-eff")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll();
                let Some(v) = &e.validation else {
                    col = col.child(note(if e.validating { "解析中…" } else { "还没校验过" }));
                    return wrapper(tabs, col.into_any_element(), cx, self);
                };
                let Some(res) = &v.resolved else {
                    col = col.child(note("引擎没回 resolved"));
                    return wrapper(tabs, col.into_any_element(), cx, self);
                };
                for (k, val) in res.as_object().map(|o| o.iter().collect::<Vec<_>>()).unwrap_or_default() {
                    let text = resolved_text(val);
                    col = col.child(kv(k, &text));
                }
                col.into_any_element()
            }
            EditorTab::Issues => {
                let mut col = div()
                    .id("ed-issues")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll();
                let items: Vec<(bool, String, String)> = e
                    .validation
                    .as_ref()
                    .map(|v| {
                        v.errors
                            .iter()
                            .map(|x| (true, issue_text(x), issue_where(x)))
                            .chain(v.warnings.iter().map(|x| (false, issue_text(x), issue_where(x))))
                            .collect()
                    })
                    .unwrap_or_default();
                if items.is_empty() {
                    col = col.child(note("没有问题 —— 形状闸和引用闸都过了"));
                }
                for (bad, text, where_) in items {
                    col = col.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(t().d(8.))
                            .px(t().d(10.))
                            .py(t().d(6.))
                            .border_b(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(if bad { t().p.danger } else { t().p.warn }))
                                    .child(if bad { "✕" } else { "!" }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(t().d(0.))
                                    .child(div().text_size(t().fs(11.)).child(text))
                                    .children((!where_.is_empty()).then(|| {
                                        div()
                                            .text_size(t().fs(9.5))
                                            .font_family("JetBrainsMono")
                                            .text_color(rgb(t().p.faint))
                                            .child(where_)
                                    })),
                            ),
                    );
                }
                col.into_any_element()
            }
            EditorTab::Changes => {
                let saved = serde_json::from_str::<serde_json::Value>(&e.text).ok();
                let mut col = div()
                    .id("ed-changes")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll();
                let mut any = false;
                for (key, before, after) in diff_pairs(e, saved.as_ref()) {
                    any = true;
                    let before0 = before.clone();
                    let k = key.clone();
                    col = col.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(7.))
                            .px(t().d(10.))
                            .py(t().d(5.))
                            .border_b(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .w(t().d(92.))
                                    .flex_none()
                                    .text_size(t().fs(10.))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(t().p.muted))
                                    .child(key),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(t().d(0.))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.text_2))
                                    .child(format!("{} → {}", before, after)),
                            )
                            .child(
                                div()
                                    .id(ElementId::Name(format!("ed-undo-{k}").into()))
                                    .text_size(t().fs(10.))
                                    .text_color(rgb(t().p.faint))
                                    .cursor_pointer()
                                    .hover(|h| h.text_color(rgb(t().p.danger)))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        restore_key(&mut this.editor, &k, &before0);
                                        cx.notify();
                                    }))
                                    .child("撤销"),
                            ),
                    );
                }
                if !any {
                    col = col.child(note("和盘上一致，没有改动"));
                }
                col.into_any_element()
            }
        };

        wrapper(tabs, body, cx, self)
    }

    /// 发布对话框：commit message。
    pub(crate) fn open_publish_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default = format!("调整 {}", self.editor.name);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(default));
        self.editor.publish_msg = Some(input);
        cx.notify();
    }

    /// 文本字段的输入弹层。
    /// 挑值弹层：筛选框 + 候选列表（多选时点一下就勾，不关）。
    pub(crate) fn picker_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.editor.picker.as_ref()?;
        let kind = p.kind;
        let multi = p.multi;
        let path = p.path;
        let input = p.filter.clone();
        let raw = input.read(cx).value().trim().to_string();
        let needle = raw.to_lowercase();
        let cur = self.editor.text_of(path);
        let cur_list = self.editor.list_of(path[0]);

        let all = self.pick_items(kind);
        let shown: Vec<PickItem> = all
            .iter()
            .filter(|it| {
                needle.is_empty()
                    || it.name.to_lowercase().contains(&needle)
                    || it.hint.to_lowercase().contains(&needle)
            })
            .cloned()
            .collect();
        // 打进去的字本身就算一个新值（候选里没有的也能填）—— 只在「没完全匹配」时出现。
        let offer_new = !raw.is_empty() && !all.iter().any(|it| it.name.eq_ignore_ascii_case(&raw));

        let mut list = div().flex().flex_col().max_h(t().d(320.)).id("ed-pk-list").overflow_y_scroll();
        if offer_new {
            let val = raw.clone();
            let is_in = cur_list.iter().any(|x| x == &val) || cur == val;
            list = list.child(
                div()
                    .id("ed-pk-new")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(14.))
                    .py(t().d(7.))
                    .cursor_pointer()
                    .bg(rgb(t().p.hover))
                    .hover(|h| h.bg(rgb(t().p.selected)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.pick_apply(&val, cx);
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.5))
                            .font_family("JetBrainsMono")
                            .child(format!("用「{raw}」")),
                    )
                    .children((multi && is_in).then(|| {
                        div().ml_auto().text_size(t().fs(10.)).text_color(rgb(t().p.ok)).child("已选")
                    })),
            );
        }
        for it in &shown {
            let name = it.name.clone();
            let on = if multi {
                cur_list.iter().any(|x| x == &name)
            } else {
                cur == name
            };
            let off = it.off;
            list = list.child(
                div()
                    .id(ElementId::Name(format!("ed-pk-{name}").into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(14.))
                    .py(t().d(6.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.selected)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.pick_apply(&name, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .flex()
                            .flex_col()
                            .gap(t().d(1.))
                            .child(
                                div()
                                    .text_size(t().fs(11.5))
                                    .font_family("JetBrainsMono")
                                    .when(off, |el| el.text_color(rgb(t().p.faint)))
                                    .when(!off && on, |el| el.text_color(rgb(t().p.text)))
                                    .when(!off && !on, |el| el.text_color(rgb(t().p.text_2)))
                                    .child(it.name.clone()),
                            )
                            .children((!it.hint.is_empty()).then(|| {
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.faint))
                                    .truncate()
                                    .child(it.hint.clone())
                            })),
                    )
                    .children(off.then(|| {
                        div().text_size(t().fs(9.)).text_color(rgb(t().p.warn)).child("用不了")
                    }))
                    .children(on.then(|| {
                        div().text_size(t().fs(10.5)).text_color(rgb(t().p.ok)).child("✓")
                    })),
            );
        }
        if shown.is_empty() && !offer_new {
            list = list.child(
                div()
                    .px(t().d(14.))
                    .py(t().d(12.))
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.faint))
                    .child(match kind {
                        PickKind::Free => "打个值，下面会出「用「…」」".to_string(),
                        _ => "没有候选 —— 上面直接打，也能当新值用".to_string(),
                    }),
            );
        }

        let scrim = div()
            .id("ed-pk-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.editor.picker = None;
                cx.notify();
            }));

        let dialog = div()
            .id("ed-pk-dialog")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(t().d(460.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(8.))
                            .px(t().d(14.))
                            .pt(t().d(12.))
                            .pb(t().d(8.))
                            .child(div().text_size(t().fs(12.5)).child(kind.title()))
                            .child(
                                div()
                                    .ml_auto()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.faint))
                                    .child(if multi {
                                        "点一下勾上，再点取消"
                                    } else {
                                        "点一个就定"
                                    }),
                            )
                            .child(
                                div()
                                    .id("ed-pk-x")
                                    .cursor_pointer()
                                    .px(t().d(4.))
                                    .text_color(rgb(t().p.faint))
                                    .hover(|h| h.text_color(rgb(t().p.text)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.editor.picker = None;
                                        cx.notify();
                                    }))
                                    .child("✕"),
                            ),
                    )
                    .child(div().px(t().d(14.)).pb(t().d(8.)).child(Input::new(&input)))
                    .child(div().h(t().d(1.)).bg(rgb(t().p.line_soft)))
                    .child(list),
            );

        Some(div().absolute().inset_0().child(scrim).child(dialog).into_any_element())
    }

    pub(crate) fn text_edit_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let te = self.editor.text_edit.as_ref()?;
        let key = te.key.clone();
        let input = te.input.clone();
        let scrim = div()
            .id("ed-te-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.editor.text_edit = None;
                cx.notify();
            }));
        let dialog = div()
            .id("ed-te-dialog")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(t().d(420.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(16.))
                            .text_size(t().fs(13.))
                            .child(format!("改 {key}")),
                    )
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(3.))
                            .pb(t().d(12.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.faint))
                            .child("留空 = 删掉这个键（不是写个空字符串）"),
                    )
                    .child(div().px(t().d(20.)).child(Input::new(&input)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .px(t().d(20.))
                            .py(t().d(14.))
                            .child(
                                div()
                                    .id("ed-te-cancel")
                                    .px(t().d(14.))
                                    .py(t().d(5.))
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .text_size(t().fs(11.5))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.editor.text_edit = None;
                                        cx.notify();
                                    }))
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("ed-te-ok")
                                    .px(t().d(16.))
                                    .py(t().d(5.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(t().p.accent))
                                    .text_color(rgb(t().p.on_accent))
                                    .text_size(t().fs(11.5))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.commit_text_field(cx);
                                    }))
                                    .child("确定"),
                            ),
                    ),
            );
        Some(div().absolute().inset_0().child(scrim).child(dialog).into_any_element())
    }

    /// 发布对话框。
    pub(crate) fn publish_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let input = self.editor.publish_msg.as_ref()?;
        let scrim = div()
            .id("ed-pub-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.editor.publish_msg = None;
                cx.notify();
            }));
        let dialog = div()
            .id("ed-pub-dialog")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(t().d(460.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(16.))
                            .text_size(t().fs(13.))
                            .child(format!("发布 {} · {}", self.editor.section, self.editor.name)),
                    )
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(3.))
                            .pb(t().d(10.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child("会先过形状闸（不过就一个字节都不动），再 commit + push。"),
                    )
                    .child(div().px(t().d(20.)).child(Input::new(input)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .px(t().d(20.))
                            .py(t().d(14.))
                            .child(
                                div()
                                    .id("ed-pub-cancel")
                                    .px(t().d(14.))
                                    .py(t().d(5.))
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .text_size(t().fs(11.5))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.editor.publish_msg = None;
                                        cx.notify();
                                    }))
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("ed-pub-go")
                                    .px(t().d(16.))
                                    .py(t().d(5.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(t().p.accent))
                                    .text_color(rgb(t().p.on_accent))
                                    .text_size(t().fs(11.5))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.publish_editor(cx);
                                    }))
                                    .child("发布"),
                            ),
                    ),
            );
        Some(div().absolute().inset_0().child(scrim).child(dialog).into_any_element())
    }
}

fn wrapper(
    tabs: gpui_kit::Div,
    body: AnyElement,
    _cx: &mut Context<crate::app::Desktop>,
    _d: &crate::app::Desktop,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .w(t().d(348.))
        .flex_none()
        .bg(rgb(t().p.panel_alt))
        .border_l(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(tabs)
        .child(body)
        .into_any_element()
}

fn pill(text: String, color: u32) -> AnyElement {
    div()
        .flex()
        .items_center()
        .h(t().d(18.))
        .px(t().d(7.))
        .rounded(t().d(9.))
        .border(t().d(1.))
        .border_color(rgb(color))
        .text_size(t().fs(10.))
        .text_color(rgb(color))
        .child(text)
        .into_any_element()
}

fn section_label(text: &str) -> AnyElement {
    div()
        .px(t().d(12.))
        .py(t().d(8.))
        .text_size(t().fs(9.5))
        .text_color(rgb(t().p.faint))
        .child(text.to_uppercase())
        .into_any_element()
}

fn note(text: &str) -> AnyElement {
    div()
        .p(t().d(12.))
        .text_size(t().fs(11.))
        .text_color(rgb(t().p.muted))
        .child(text.to_string())
        .into_any_element()
}

fn kv(k: &str, v: &str) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .gap(t().d(9.))
        .px(t().d(11.))
        .py(t().d(5.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .w(t().d(76.))
                .flex_none()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.faint))
                .child(k.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .text_size(t().fs(10.5))
                .font_family("JetBrainsMono")
                .text_color(rgb(t().p.text_2))
                .child(v.to_string()),
        )
        .into_any_element()
}

fn glyph(g: Group) -> &'static str {
    match g {
        Group::Model => "◈",
        Group::Ctx => "▤",
        Group::Ext => "◇",
        Group::Perm => "⚿",
        Group::Run => "▶",
    }
}

/// 按路径取一份值（`model.provider` 这种嵌套）。
fn value_at<'a>(v: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    Some(cur)
}

/// 引擎给的错误/警告可能是字符串，也可能是 `{path, message}`。
fn issue_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other
            .get("message")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| other.to_string()),
    }
}

fn issue_where(v: &serde_json::Value) -> String {
    v.get("path")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// `resolved` 里一项变成一行字。
fn resolved_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => a
            .iter()
            .map(resolved_text)
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Object(o) => {
            if let Some(val) = o.get("value") {
                let state = o.get("state").and_then(|s| s.as_str()).unwrap_or("");
                let val = resolved_text(val);
                if state == "ok" {
                    val
                } else {
                    format!("{val}（{state}）")
                }
            } else {
                o.iter()
                    .map(|(k, v)| format!("{k}={}", resolved_text(v)))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
        other => other.to_string(),
    }
}

/// 改动列表：`(键, 旧值, 新值)`，只列真的变了的。
fn changed_labels(e: &EditorState) -> Vec<String> {
    let saved = serde_json::from_str::<serde_json::Value>(&e.text).ok();
    diff_pairs(e, saved.as_ref())
        .into_iter()
        .map(|(k, _, _)| k)
        .collect()
}

fn diff_pairs(
    e: &EditorState,
    saved: Option<&serde_json::Value>,
) -> Vec<(String, String, String)> {
    let Some(saved) = saved else {
        return Vec::new();
    };
    let show = |v: Option<&serde_json::Value>| match v {
        None => "（无）".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) => "（空）".to_string(),
        Some(serde_json::Value::Array(a)) => {
            let s = a
                .iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if s.is_empty() {
                "（空）".to_string()
            } else {
                s
            }
        }
        Some(other) => other.to_string(),
    };
    let mut out = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    if let Some(o) = saved.as_object() {
        keys.extend(o.keys().cloned());
    }
    if let Some(o) = e.draft.as_object() {
        for k in o.keys() {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
    }
    for k in keys {
        let a = saved.get(&k);
        let b = e.draft.get(&k);
        if a != b {
            out.push((k.clone(), show(a), show(b)));
        }
    }
    out
}

/// 撤销某一项：有旧值就写回去，没有就把键删掉。
fn restore_key(e: &mut EditorState, key: &str, before: &str) {
    if e.draft.is_null() {
        return;
    }
    let Some(obj) = e.draft.as_object_mut() else {
        return;
    };
    if before == "（无）" {
        obj.remove(key);
        return;
    }
    obj.insert(key.to_string(), serde_json::Value::String(before.to_string()));
}

/// JSON 那一栏要标红的行：拿错误里的 `path` 找不到行号，就退化成「有错就标整份」。
fn json_error_line(e: &EditorState) -> Option<usize> {
    let v = e.validation.as_ref()?;
    let first = v.errors.first()?;
    let path = first.get("path").and_then(|p| p.as_str())?;
    let key = path.rsplit('.').next()?;
    let lines = e.pretty_lines();
    lines
        .iter()
        .position(|l| l.contains(&format!("\"{key}\"")))
        .map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    // 别写 `use super::*;` —— 会把 `gpui_kit::*` 带进来，`#[test]` 展开时撞 recursion limit
    // （坑.md 69）。
    use super::{del_key, is_dirty, read_text, toggle_list, write_bool, write_text};

    fn v(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn nested_path_reads_and_writes() {
        let mut d = v(r#"{"model":{"provider":"volc-ark"}}"#);
        assert_eq!(read_text(&d, &["model", "provider"]), "volc-ark");
        write_text(&mut d, &["model", "modelId"], "deepseek-v4");
        assert_eq!(d["model"]["modelId"], "deepseek-v4");
        assert_eq!(d["model"]["provider"], "volc-ark");
    }

    #[test]
    fn empty_text_deletes_the_key_not_writes_empty_string() {
        let mut d = v(r#"{"template":"smart-ass"}"#);
        write_text(&mut d, &["template"], "");
        assert!(d.get("template").is_none());
    }

    #[test]
    fn removing_a_nested_key_keeps_the_parent() {
        let mut d = v(r#"{"model":{"provider":"volc-ark","modelId":"x"}}"#);
        del_key(&mut d, "model.provider");
        assert_eq!(d["model"]["modelId"], "x");
        assert!(d["model"].get("provider").is_none());
    }

    #[test]
    fn switching_off_deletes_the_key() {
        let mut d = v(r#"{"name":"a"}"#);
        write_bool(&mut d, &["readOnly"], true);
        assert_eq!(d["readOnly"], true);
        write_bool(&mut d, &["readOnly"], false);
        assert!(d.get("readOnly").is_none());
        assert_eq!(d["name"], "a");
    }

    #[test]
    fn reordered_keys_are_not_dirty() {
        let d = v(r#"{"b":1,"a":2}"#);
        assert!(!is_dirty(r#"{"a":2,"b":1}"#, &d));
        assert!(is_dirty(r#"{"a":2}"#, &d));
    }

    #[test]
    fn adding_an_array_member_is_dirty_and_deduped() {
        let mut d = v(r#"{"category":["工程"]}"#);
        let saved = r#"{"category":["工程"]}"#;
        assert!(!is_dirty(saved, &d));
        toggle_list(&mut d, "category", "写作");
        assert_eq!(d["category"], serde_json::json!(["工程", "写作"]));
        assert!(is_dirty(saved, &d));
        toggle_list(&mut d, "category", "写作");
        assert_eq!(d["category"], serde_json::json!(["工程"]));
        assert!(!is_dirty(saved, &d));
    }
}
