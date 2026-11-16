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

/// 终极模板那一段的 id（引擎侧的路径段）。
const ULTIMATE: &str = "Ultimate-Template";

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 检查器四个 tab。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorTab {
    Json,
    Effective,
    Issues,
    /// 仓里这份预设还没提交的改动（引擎 `/pending` + `/pending/diff`）。
    Diff,
    /// 这条预设提交过哪些次（引擎 `/history`）。
    History,
    Changes,
}

impl EditorTab {
    const ALL: [Self; 6] = [
        Self::Json,
        Self::Effective,
        Self::Issues,
        Self::Diff,
        Self::History,
        Self::Changes,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Effective => "生效值",
            Self::Issues => "问题",
            Self::Diff => "待提交",
            Self::History => "历史",
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
    /// 终极模板：基本信息（摘要 / 类别 / 启用）
    UBase,
    /// 终极模板：触发器和它的参数
    UTrigger,
    /// 终极模板：引用哪条会话模板
    USession,
}

impl Group {
    /// 会话模板 / 终极模板是两套字段，左导航也就两套。
    fn all(ultimate: bool) -> &'static [Self] {
        if ultimate {
            &[Self::UBase, Self::UTrigger, Self::USession]
        } else {
            &[Self::Model, Self::Ctx, Self::Ext, Self::Perm, Self::Run]
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Model => "模型",
            Self::Ctx => "上下文",
            Self::Ext => "扩展",
            Self::Perm => "权限",
            Self::Run => "运行",
            Self::UBase => "基本",
            Self::UTrigger => "触发器",
            Self::USession => "会话",
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
    /// 终极模板：触发器（`/v1/triggers`）
    Trigger,
    /// 终极模板：会话模板（Session-Template 那一段的预设名）
    Session,
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
            Self::Trigger => "选触发器",
            Self::Session => "选会话模板",
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
    /// `(名字, 摘要, 参数定义)` —— 参数表单照着它画，不是写死的。
    pub triggers: Vec<(String, String, Vec<nous_engine::types::TriggerParam>)>,
    /// Session-Template 那一段的预设名（终极模板的 `session` 引用的就是它们）。
    pub session_templates: Vec<PickItem>,
    pub err: Option<String>,
}

/// 命令面板（Ctrl+K）。一个输入框 + 一列命令，↑↓ 选、回车执行。
pub(crate) struct Palette {
    pub input: Entity<InputState>,
    /// 当前选中第几行（过滤之后的下标）。
    pub sel: usize,
}

/// 命令面板里能做的一件事。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cmd {
    Group(Group),
    Tab(EditorTab),
    Save,
    Publish,
    Reload,
    Validate,
    Back,
}

/// 面板里的一行。
#[derive(Debug, Clone)]
pub(crate) struct CmdRow {
    pub label: String,
    /// 右边那行小字（快捷键 / 说明）。
    pub hint: String,
    pub cmd: Cmd,
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
    /// 这一段的清单文件名（`preset.json` / `ultimate.json`）—— 由段的 `manifest` 决定，
    /// 客户端不写死；打开时从 `/files` 清单里挑，之后保存 / diff / history 都认它。
    pub manifest: String,
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
    /// Ctrl+K 命令面板。
    pub palette: Option<Palette>,
    /// 仓里这一段有什么没提交（`/pending`）。
    pub pending: Option<nous_engine::types::Pending>,
    /// 当前选中那个文件的 `before` / `after`。
    pub diff: Option<nous_engine::types::FileDiff>,
    /// 正在拉 diff 的那个文件路径（相对段目录）。
    pub diff_pick: Option<String>,
    pub diff_loading: bool,
    /// 这条预设的提交历史（新到旧）。
    pub history: Option<Vec<nous_engine::types::Commit>>,
    /// 选中那次提交的 hash。
    pub hist_pick: Option<String>,
    /// 选中那次提交的改动。
    pub hist_diff: Option<nous_engine::types::FileDiff>,
    pub hist_loading: bool,
    /// 候选池。
    pub candidates: Candidates,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            open: false,
            section: String::new(),
            name: String::new(),
            manifest: "preset.json".to_string(),
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
            palette: None,
            pending: None,
            diff: None,
            diff_pick: None,
            diff_loading: false,
            history: None,
            hist_pick: None,
            hist_diff: None,
            hist_loading: false,
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

    /// 按路径写一份任意 JSON（数字 / 布尔这些 `set_text` 写不了的）。
    pub fn set_json(&mut self, path: &[&str], val: serde_json::Value) {
        if path.is_empty() {
            return;
        }
        if self.draft.is_null() {
            self.draft = serde_json::json!({});
        }
        let mut cur = &mut self.draft;
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
        if let Some(obj) = cur.as_object_mut() {
            obj.insert(path[path.len() - 1].to_string(), val);
        }
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

/// 一行 diff 是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffOp {
    Same,
    Add,
    Del,
}

/// 行级 diff（LCS）。文件都不大（几百行），DP 就够，不用拉一个 diff 库进来。
///
/// 返回 `(是什么, 这一行的原文)`，按着老文件在前新文件在后的顺序摊平。
fn line_diff(before: &str, after: &str) -> Vec<(DiffOp, String)> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let (n, m) = (a.len(), b.len());
    // dp[i][j] = a[i..] 和 b[j..] 的 LCS 长度
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((DiffOp::Same, a[i].to_string()));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push((DiffOp::Del, a[i].to_string()));
            i += 1;
        } else {
            out.push((DiffOp::Add, b[j].to_string()));
            j += 1;
        }
    }
    while i < n {
        out.push((DiffOp::Del, a[i].to_string()));
        i += 1;
    }
    while j < m {
        out.push((DiffOp::Add, b[j].to_string()));
        j += 1;
    }
    out
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
            // 两段是两套字段，左导航的默认组也跟着换。
            group: if section == ULTIMATE {
                Group::UBase
            } else {
                Group::Model
            },
            ..EditorState::new()
        };
        self.picked_preset = Some(name.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            // ① 清单文件叫什么（`preset.json` / `ultimate.json`）—— 问清单，不猜。
            let manifest = match &handle {
                Some(h) => {
                    let (e, s, n) = (engine.clone(), section.clone(), name.clone());
                    match h.spawn(async move { e.preset_files(&s, &n).await }).await {
                        Ok(Ok(files)) => pick_manifest(&files.files),
                        _ => "preset.json".to_string(),
                    }
                }
                _ => "preset.json".to_string(),
            };
            // ② 读它
            let result: Option<Result<PresetFile, EngineError>> = match &handle {
                Some(h) => {
                    let (e, s, n, m) = (engine.clone(), section.clone(), name.clone(), manifest.clone());
                    h.spawn(async move { e.preset_file(&s, &n, &m).await })
                        .await
                        .ok()
                }
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.editor.loading = false;
                match result {
                    Some(Ok(f)) => {
                        this.editor.manifest = manifest.clone();
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
                        this.load_pending(cx);
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
            PickKind::Trigger => c
                .triggers
                .iter()
                .map(|(name, summary, params)| PickItem::new(name.clone(), format!("{summary}（{} 个参数）", params.len())))
                .collect(),
            PickKind::Session => c.session_templates.clone(),
            PickKind::Free => Vec::new(),
        }
    }

    /// 开命令面板（Ctrl+K）。
    pub(crate) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜命令：保存 / 发布 / 分组 / 检查器"));
        cx.subscribe_in(&input, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                // 换了关键字，选中的那条得回到第一行。
                cx.notify();
            }
        });
        // 面板一开就能打字，不用先点一下输入框。走组件自己的 focus（坑.md 72）。
        input.update(cx, |st, cx| st.focus(window, cx));
        self.editor.palette = Some(Palette { input, sel: 0 });
        cx.notify();
    }

    /// 面板里现在该列哪些命令（按输入框内容过滤）。
    pub(crate) fn palette_rows(&self, cx: &Context<Self>) -> Vec<CmdRow> {
        let q = self
            .editor
            .palette
            .as_ref()
            .map(|p| p.input.read(cx).value().trim().to_lowercase())
            .unwrap_or_default();
        let mut all = vec![
            CmdRow {
                label: "保存到盘上".into(),
                hint: "Ctrl+S · 还没提交".into(),
                cmd: Cmd::Save,
            },
            CmdRow {
                label: "发布…".into(),
                hint: "commit + push，会真的动仓库".into(),
                cmd: Cmd::Publish,
            },
            CmdRow {
                label: "重读盘上那份".into(),
                hint: "丢掉没保存的改动".into(),
                cmd: Cmd::Reload,
            },
            CmdRow {
                label: "再校验一次".into(),
                hint: "走引擎 /validate".into(),
                cmd: Cmd::Validate,
            },
            CmdRow {
                label: "返回模板列表".into(),
                hint: "Esc（有改动会拦一次）".into(),
                cmd: Cmd::Back,
            },
        ];
        let ultimate = self.editor.section == ULTIMATE;
        for &g in Group::all(ultimate) {
            all.push(CmdRow {
                label: format!("分组 · {}", g.label()),
                hint: String::new(),
                cmd: Cmd::Group(g),
            });
        }
        for t in EditorTab::ALL {
            all.push(CmdRow {
                label: format!("检查器 · {}", t.label()),
                hint: String::new(),
                cmd: Cmd::Tab(t),
            });
        }
        if q.is_empty() {
            return all;
        }
        all.into_iter()
            .filter(|r| {
                r.label.to_lowercase().contains(&q) || r.hint.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// ↑ / ↓ 在面板的清单里挪。
    pub(crate) fn palette_move(&mut self, down: bool, cx: &mut Context<Self>) {
        let n = self.palette_rows(cx).len();
        let Some(p) = self.editor.palette.as_mut() else {
            return;
        };
        if n == 0 {
            p.sel = 0;
        } else if down {
            p.sel = (p.sel + 1) % n;
        } else {
            p.sel = (p.sel + n - 1) % n;
        }
        cx.notify();
    }

    /// 回车：跑选中的那条。
    pub(crate) fn palette_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.palette_rows(cx);
        let sel = self.editor.palette.as_ref().map(|p| p.sel).unwrap_or(0);
        let Some(row) = rows.get(sel).cloned() else {
            self.editor.palette = None;
            cx.notify();
            return;
        };
        self.editor.palette = None;
        match row.cmd {
            Cmd::Group(g) => self.editor.group = g,
            Cmd::Tab(t) => {
                self.editor.tab = t;
                if t == EditorTab::Diff && self.editor.pending.is_none() {
                    self.load_pending(cx);
                }
                if t == EditorTab::History && self.editor.history.is_none() {
                    self.load_history(cx);
                }
            }
            Cmd::Save => self.save_editor(cx),
            Cmd::Publish => self.open_publish_dialog(window, cx),
            Cmd::Reload => self.reload_editor(cx),
            Cmd::Validate => self.validate_preset(cx),
            Cmd::Back => {
                self.close_editor(false, cx);
            }
        }
        cx.notify();
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
        // 筛选框拿焦点：字段一点开就能打关键字。
        input.update(cx, |st, cx| st.focus(window, cx));
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

    /// 拉这一段仓里没提交的东西（编辑器「待提交」那一栏的目录）。
    pub(crate) fn load_pending(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let section = self.editor.section.clone();
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move { engine.pending(&section).await })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                let Some(Ok(p)) = got else {
                    return;
                };
                // 之前选的那个文件可能已经提交掉了 —— 不在清单里就重选第一个。
                let files: Vec<String> = p
                    .unit(&this.editor.name)
                    .map(|u| u.files.iter().map(|f| f.path.clone()).collect())
                    .unwrap_or_default();
                let keep = this
                    .editor
                    .diff_pick
                    .as_ref()
                    .is_some_and(|cur| files.iter().any(|f| f == cur));
                this.editor.pending = Some(p);
                if !keep {
                    this.editor.diff = None;
                    this.editor.diff_pick = None;
                }
                if let Some(first) = files.first().cloned() {
                    if !keep {
                        this.pick_diff_file(&first, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 拉这条预设的提交历史（新到旧）。
    pub(crate) fn load_history(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let section = self.editor.section.clone();
        let name = self.editor.name.clone();
        self.editor.hist_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move { engine.preset_history(&section, &name, 30).await })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                this.editor.hist_loading = false;
                match got {
                    Some(Ok(h)) => {
                        this.editor.history = Some(h.commits);
                        // 默认看最近那次 —— 打开就看到改了什么，不用先点一下。
                        let first = this
                            .editor
                            .history
                            .as_ref()
                            .and_then(|c| c.first())
                            .map(|c| c.hash.clone());
                        if let Some(hash) = first {
                            this.pick_commit(&hash, cx);
                        }
                    }
                    Some(Err(e)) => this.editor.err = Some(format!("读历史失败：{e}")),
                    None => this.editor.err = Some("读历史没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 看某一次提交改了什么（这一段清单文件那一份）。
    pub(crate) fn pick_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let section = self.editor.section.clone();
        let name = self.editor.name.clone();
        let manifest = self.editor.manifest.clone();
        let hash = hash.to_string();
        self.editor.hist_pick = Some(hash.clone());
        self.editor.hist_diff = None;
        self.editor.hist_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move {
                    engine
                        .preset_commit_diff(&section, &name, &hash, &manifest)
                        .await
                })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                this.editor.hist_loading = false;
                match got {
                    Some(Ok(d)) => this.editor.hist_diff = Some(d),
                    Some(Err(e)) => this.editor.err = Some(format!("读那次改动失败：{e}")),
                    None => this.editor.err = Some("读那次改动没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 读一个文件的 `before` / `after`（相对段目录的路径）。
    pub(crate) fn pick_diff_file(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let section = self.editor.section.clone();
        let path = path.to_string();
        self.editor.diff_pick = Some(path.clone());
        self.editor.diff = None;
        self.editor.diff_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move { engine.pending_diff(&section, &path).await })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                this.editor.diff_loading = false;
                match got {
                    Some(Ok(d)) => this.editor.diff = Some(d),
                    Some(Err(e)) => this.editor.err = Some(format!("读改动失败：{e}")),
                    None => this.editor.err = Some("读改动没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
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
                    let triggers = engine.triggers().await.ok();
                    // 终极模板的 `session` 引用的是 Session-Template 那一段 —— 候选也从那儿来。
                    let sessions = engine.presets("Session-Template").await.ok();
                    (models, prompts, skills, cats, triggers, sessions)
                })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.editor.candidates;
                if let Some((models, prompts, skills, cats, triggers, sessions)) = out {
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
                    if let Some(t) = triggers {
                        c.triggers = t
                            .triggers
                            .iter()
                            .map(|it| {
                                (
                                    it.name.clone(),
                                    it.summary.clone().unwrap_or_default(),
                                    it.params.clone(),
                                )
                            })
                            .collect();
                    }
                    if let Some(sp) = sessions {
                        c.session_templates = sp
                            .presets
                            .iter()
                            .map(|it| PickItem::new(it.name.clone(), it.summary.clone()))
                            .collect();
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
        let manifest = self.editor.manifest.clone();
        let text = serde_json::to_string_pretty(&self.editor.draft).unwrap_or_default() + "\n";
        let text_for_req = text.clone();
        let manifest_for_req = manifest.clone();
        self.editor.saving = true;
        self.editor.err = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move {
                        engine
                            .put_preset_file(&section, &name, &manifest_for_req, &text_for_req, &base)
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
                        this.load_pending(cx);
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
        // `key` 可能是嵌套路径（`params.epoch`）—— 拆成路径读，否则读到的是空串。
        let path: Vec<&str> = key.split('.').collect();
        let value = self.editor.text_of(&path);
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
        let v = v.trim();
        if let Some(param) = te.key.strip_prefix("params.") {
            // 参数得按触发器说的类型写：`epoch` 是数字，写成字符串引擎不认。
            let ty = self
                .editor
                .text_of(&["trigger"])
                .split_whitespace()
                .next()
                .map(|t| t.to_string())
                .and_then(|t| {
                    self.editor
                        .candidates
                        .triggers
                        .iter()
                        .find(|(n, _, _)| *n == t)
                        .and_then(|(_, _, defs)| {
                            defs.iter().find(|d| d.name == param).map(|d| d.ty.clone())
                        })
                })
                .unwrap_or_else(|| "string".to_string());
            let path = ["params", param];
            if v.is_empty() {
                self.editor.remove_key(&format!("params.{param}"));
            } else {
                self.editor.set_json(&path, typed_json(&ty, v));
            }
        } else {
            let path: Vec<&str> = te.key.split('.').collect();
            self.editor.set_text(&path, v);
        }
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
        for &g in Group::all(self.editor.section == ULTIMATE) {
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

        let ultimate = self.editor.section == ULTIMATE;
        for &g in Group::all(ultimate) {
            if g != self.editor.group {
                continue;
            }
            let items: Vec<FldSpec> = match (ultimate, g) {
                (true, Group::UBase) => vec![
                    FldSpec { label: "摘要", path: &["summary"], ctl: Ctl::Text },
                    FldSpec { label: "类别", path: &["category"], ctl: Ctl::Multi(PickKind::Category) },
                    FldSpec { label: "启用", path: &["enabled"], ctl: Ctl::Switch },
                ],
                (true, Group::UTrigger) => vec![
                    FldSpec { label: "触发器", path: &["trigger"], ctl: Ctl::Pick(PickKind::Trigger) },
                ],
                (true, Group::USession) => vec![
                    FldSpec { label: "会话模板", path: &["session"], ctl: Ctl::Pick(PickKind::Session) },
                ],
                (false, g) => match g {
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
                    _ => Vec::new(),
                },
                // 极端情况（组和段对不上，比如刚切段那一下）就当没有字段。
                (true, _) => Vec::new(),
            };
            // 终极模板的「参数」不在这张静态表里 —— 键名和控件都跟着触发器变，下一段单独画。
            let params_block = ultimate && g == Group::UTrigger;

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
            if params_block {
                col = col.child(self.params_editor(cx));
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

    /// 终极模板的「参数」：**键名和控件都来自触发器的定义**，不是写死的。
    ///
    /// 触发器可以有一堆参数（`epoch` 这种），也可以一个都没有 —— 都按定义画。
    fn params_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.editor;
        let trigger = e.text_of(&["trigger"]);
        let Some((_, summary, defs)) = e
            .candidates
            .triggers
            .iter()
            .find(|(n, _, _)| *n == trigger)
        else {
            return note(if trigger.is_empty() {
                "先在上面选一个触发器，它的参数会长在这儿。"
            } else {
                "这个触发器的定义没拉到 —— 参数只能去 JSON 那栏改。"
            });
        };
        if defs.is_empty() {
            return note("这个触发器不要参数，选好就能用。");
        }

        let mut col = div()
            .flex()
            .flex_col()
            .gap(t().d(1.))
            .child(
                div()
                    .px(t().d(16.))
                    .pt(t().d(12.))
                    .pb(t().d(4.))
                    .text_size(t().fs(11.))
                    .child(format!("参数 · {summary}")),
            );
        for p in defs {
            let pname = p.name.clone();
            let ty = p.ty.clone();
            let label = p.label.clone().unwrap_or_else(|| p.name.clone());
            let path = ["params".to_string(), pname.clone()];
            let pathref: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
            let key = pathref.join(".");
            let cur = value_at(&e.draft, &pathref).cloned();
            let shown = match &cur {
                None | Some(serde_json::Value::Null) => String::new(),
                Some(serde_json::Value::String(t)) => t.clone(),
                Some(other) => other.to_string(),
            };
            // 秒数旁边给一句人话 —— 存进去是 UTC 秒，人不该心算。
            let human = if ty == "epoch" {
                shown
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .map(local_time)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let changed = cur.is_some();

            let mut control = div()
                .id(ElementId::Name(format!("ed-pv-{key}").into()))
                .flex()
                .items_center()
                .gap(t().d(8.))
                .px(t().d(8.))
                .py(t().d(3.))
                .rounded(t().r(6.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .bg(rgb(t().p.panel))
                .cursor_pointer()
                .hover(|h| h.border_color(rgb(t().p.muted)))
                .on_click(cx.listener({
                    let key = key.clone();
                    move |this, _, window, cx| this.edit_text_field(&key, window, cx)
                }))
                .child(
                    div()
                        .text_size(t().fs(11.5))
                        .font_family("JetBrainsMono")
                        .text_color(rgb(if shown.is_empty() {
                            t().p.faint
                        } else {
                            t().p.text_2
                        }))
                        .child(if shown.is_empty() {
                            "（没填）".to_string()
                        } else {
                            shown.clone()
                        }),
                );
            if !human.is_empty() {
                control = control.child(
                    div()
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.faint))
                        .child(format!("= 本地 {human}")),
                );
            }
            control = control.child(div().text_size(t().fs(9.)).text_color(rgb(t().p.faint)).child("✎"));

            col = col.child(
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
                            .children((!p.required).then(|| {
                                div()
                                    .text_size(t().fs(9.))
                                    .text_color(rgb(t().p.faint))
                                    .child("选填")
                            })),
                    )
                    .child(div().flex_1().min_w(t().d(0.)).child(control))
                    .children(changed.then(|| {
                        let k = key.clone();
                        div()
                            .id(ElementId::Name(format!("ed-pr-{k}").into()))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.faint))
                            .cursor_pointer()
                            .hover(|h| h.text_color(rgb(t().p.accent_fg)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.editor.remove_key(&k);
                                cx.notify();
                            }))
                            .child("↺ 清掉")
                    })),
            );
            if let Some(help) = &p.help {
                col = col.child(
                    div()
                        .px(t().d(16.))
                        .pl(t().d(122.))
                        .pb(t().d(3.))
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.faint))
                        .child(help.clone()),
                );
            }
        }
        col.into_any_element()
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
                EditorTab::Diff => pending_file_count(e),
                EditorTab::History => e.history.as_ref().map(|h| h.len()).unwrap_or(0),
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
                        // 这两个 tab 的数据问引擎要，「第一次点开」才打那一次请求。
                        if tab == EditorTab::Diff && this.editor.pending.is_none() {
                            this.load_pending(cx);
                        }
                        if tab == EditorTab::History && this.editor.history.is_none() {
                            this.load_history(cx);
                        }
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
            EditorTab::Diff => {
                let mut col = div()
                    .id("ed-diff")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll();
                let unit = e.pending.as_ref().and_then(|p| p.unit(&e.name));
                let Some(unit) = unit else {
                    col = col.child(note(if e.pending.is_some() {
                        "这份没有待提交的改动 —— 盘上和 HEAD 一样。"
                    } else {
                        "还没读到仓状态（引擎那边 /pending 没回）。"
                    }));
                    return wrapper(tabs, col.into_any_element(), cx, self);
                };

                // ① 这条预设改了哪些文件（引擎数好的增删行数，客户端不再数一遍）
                col = col.child(section_label(&format!(
                    "待提交 · {} 个文件",
                    unit.files.len()
                )));
                for f in &unit.files {
                    let path = f.path.clone();
                    let picked = e.diff_pick.as_deref() == Some(f.path.as_str());
                    let (mark, mark_color) = match f.status.as_str() {
                        "A" => ("A", t().p.ok),
                        "D" => ("D", t().p.danger),
                        _ => ("M", t().p.warn),
                    };
                    col = col.child(
                        div()
                            .id(ElementId::Name(format!("ed-df-{path}").into()))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(7.))
                            .px(t().d(12.))
                            .py(t().d(4.))
                            .cursor_pointer()
                            .when(picked, |el| el.bg(rgb(t().p.selected)))
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.pick_diff_file(&path, cx);
                            }))
                            .child(
                                div()
                                    .w(t().d(10.))
                                    .flex_none()
                                    .text_size(t().fs(9.5))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(mark_color))
                                    .child(mark),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(t().d(0.))
                                    .truncate()
                                    .text_size(t().fs(10.5))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(t().p.text_2))
                                    .child(f.path.replace(&format!("{}/", e.name), "")),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(t().fs(9.5))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(t().p.ok))
                                    .child(format!("+{}", f.added)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(t().fs(9.5))
                                    .font_family("JetBrainsMono")
                                    .text_color(rgb(t().p.danger))
                                    .child(format!("−{}", f.removed)),
                            ),
                    );
                }

                // ② 选中那个文件的逐行 diff
                col = col.child(div().h(t().d(1.)).bg(rgb(t().p.line_soft)));
                match &e.diff {
                    None if e.diff_loading => {
                        col = col.child(note("读那份文件的改动…"));
                    }
                    None => {
                        col = col.child(note("点上面一个文件看改了哪几行。"));
                    }
                    Some(d) => {
                        let (mut add, mut del) = (0, 0);
                        for (op, _) in line_diff(
                            d.before.as_deref().unwrap_or(""),
                            d.after.as_deref().unwrap_or(""),
                        ) {
                            match op {
                                DiffOp::Add => add += 1,
                                DiffOp::Del => del += 1,
                                DiffOp::Same => {}
                            }
                        }
                        col = col.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(t().d(8.))
                                .px(t().d(12.))
                                .py(t().d(6.))
                                .child(
                                    div()
                                        .text_size(t().fs(10.))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(t().p.faint))
                                        .child(d.path.clone()),
                                )
                                .children((d.before.is_none()).then(|| {
                                    div()
                                        .text_size(t().fs(9.))
                                        .text_color(rgb(t().p.ok))
                                        .child("新文件")
                                }))
                                .child(
                                    div()
                                        .ml_auto()
                                        .text_size(t().fs(9.5))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(t().p.ok))
                                        .child(format!("+{add}")),
                                )
                                .child(
                                    div()
                                        .text_size(t().fs(9.5))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(t().p.danger))
                                        .child(format!("−{del}")),
                                ),
                        );
                        col = col.child(diff_lines(d.before.as_deref(), d.after.as_deref()));
                    }
                }
                col.into_any_element()
            }
            EditorTab::History => {
                let mut col = div()
                    .id("ed-hist")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll();
                match &e.history {
                    None if e.hist_loading => col = col.child(note("读提交历史…")),
                    None => col = col.child(note("还没读到历史（引擎那边 /history 没回）。")),
                    Some(list) if list.is_empty() => {
                        col = col.child(note("这条还没提交过 —— 盘上有，HEAD 里没有。"));
                    }
                    Some(list) => {
                        col = col.child(section_label(&format!("提交过 {} 次", list.len())));
                        for c in list {
                            let hash = c.hash.clone();
                            let picked = e.hist_pick.as_deref() == Some(c.hash.as_str());
                            col = col.child(
                                div()
                                    .id(ElementId::Name(format!("ed-hc-{}", c.short).into()))
                                    .flex()
                                    .flex_col()
                                    .gap(t().d(2.))
                                    .px(t().d(12.))
                                    .py(t().d(5.))
                                    .cursor_pointer()
                                    .when(picked, |el| el.bg(rgb(t().p.selected)))
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _, cx| {
                                            this.pick_commit(&hash, cx);
                                        },
                                    ))
                                    .child(
                                        div()
                                            .text_size(t().fs(10.5))
                                            .when(picked, |el| el.text_color(rgb(t().p.text)))
                                            .when(!picked, |el| el.text_color(rgb(t().p.text_2)))
                                            .truncate()
                                            .child(c.message.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(t().d(6.))
                                            .child(
                                                div()
                                                    .text_size(t().fs(9.))
                                                    .font_family("JetBrainsMono")
                                                    .text_color(rgb(t().p.faint))
                                                    .child(c.short.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(t().fs(9.))
                                                    .text_color(rgb(t().p.faint))
                                                    .child(
                                                        c.at
                                                            .as_deref()
                                                            .map(stamp)
                                                            .unwrap_or_default(),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(t().fs(9.))
                                                    .text_color(rgb(t().p.faint))
                                                    .child(
                                                        c.author.clone().unwrap_or_default(),
                                                    ),
                                            ),
                                    ),
                            );
                        }
                    }
                }

                // 选中那次改了什么（只画 preset.json，别的文件去「待提交」看）。
                col = col.child(div().h(t().d(1.)).bg(rgb(t().p.line_soft)));
                match &e.hist_diff {
                    Some(d) => {
                        let (mut add, mut del) = (0, 0);
                        for (op, _) in line_diff(
                            d.before.as_deref().unwrap_or(""),
                            d.after.as_deref().unwrap_or(""),
                        ) {
                            match op {
                                DiffOp::Add => add += 1,
                                DiffOp::Del => del += 1,
                                DiffOp::Same => {}
                            }
                        }
                        col = col.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(t().d(8.))
                                .px(t().d(12.))
                                .py(t().d(6.))
                                .child(
                                    div()
                                        .text_size(t().fs(10.))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(t().p.faint))
                                        .child(d.path.clone()),
                                )
                                .children((d.before.is_none()).then(|| {
                                    div()
                                        .text_size(t().fs(9.))
                                        .text_color(rgb(t().p.ok))
                                        .child("新文件")
                                }))
                                .child(
                                    div()
                                        .ml_auto()
                                        .text_size(t().fs(9.5))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(t().p.ok))
                                        .child(format!("+{add}")),
                                )
                                .child(
                                    div()
                                        .text_size(t().fs(9.5))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(t().p.danger))
                                        .child(format!("−{del}")),
                                ),
                        );
                        col = col.child(diff_lines(
                            d.before.as_deref(),
                            d.after.as_deref(),
                        ));
                    }
                    None if e.hist_loading => col = col.child(note("读那次改动…")),
                    None => col = col.child(note("点上面一条看那次改了什么。")),
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
    /// 命令面板（Ctrl+K）：输入框 + 一列命令。
    pub(crate) fn palette_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.editor.palette.as_ref()?;
        let input = p.input.clone();
        let rows = self.palette_rows(cx);
        let sel = if rows.is_empty() {
            0
        } else {
            p.sel.min(rows.len() - 1)
        };

        let mut list = div().flex().flex_col().max_h(t().d(420.)).id("ed-pm-list").overflow_y_scroll();
        if rows.is_empty() {
            list = list.child(note("没有匹配的命令。"));
        }
        for (i, row) in rows.iter().enumerate() {
            let on = i == sel;
            let ix = i;
            list = list.child(
                div()
                    .id(ElementId::Name(format!("ed-pm-{i}").into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(10.))
                    .px(t().d(14.))
                    .py(t().d(6.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.selected)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if let Some(p) = this.editor.palette.as_mut() {
                            p.sel = ix;
                        }
                        this.palette_run(window, cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .text_size(t().fs(11.5))
                            .when(on, |el| el.text_color(rgb(t().p.text)))
                            .when(!on, |el| el.text_color(rgb(t().p.text_2)))
                            .child(row.label.clone()),
                    )
                    .children((!row.hint.is_empty()).then(|| {
                        div()
                            .flex_none()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.faint))
                            .child(row.hint.clone())
                    })),
            );
        }

        let scrim = div()
            .id("ed-pm-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.editor.palette = None;
                cx.notify();
            }));

        let dialog = div()
            .id("ed-pm-dialog")
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
                    .w(t().d(560.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(div().px(t().d(14.)).py(t().d(10.)).child(Input::new(&input)))
                    .child(div().h(t().d(1.)).bg(rgb(t().p.line_soft)))
                    .child(list)
                    .child(div().h(t().d(1.)).bg(rgb(t().p.line_soft)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(10.))
                            .px(t().d(14.))
                            .py(t().d(7.))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.faint))
                            .child("上下键选")
                            .child("回车执行")
                            .child("Esc 收")
                            .child(
                                div()
                                    .ml_auto()
                                    .child(format!("{} 条", rows.len())),
                            ),
                    ),
            );

        Some(div().absolute().inset_0().child(scrim).child(dialog).into_any_element())
    }

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

/// 从目录清单里挑出这一段的清单文件。
///
/// 段的 `manifest` 住在仓根的 `sections.json` 里（`Session-Template` = `preset.json`，
/// `Ultimate-Template` = `ultimate.json`），引擎不额外报这个名字 —— 所以照清单挑：
/// 先认那两个老名字，再退到「任意一个顶层的 .json」。
fn pick_manifest(files: &[nous_engine::types::PresetFileMeta]) -> String {
    let json: Vec<&str> = files
        .iter()
        .map(|f| f.path.as_str())
        .filter(|p| p.ends_with(".json") && !p.contains('/'))
        .collect();
    for want in ["preset.json", "ultimate.json"] {
        if json.iter().any(|p| *p == want) {
            return want.to_string();
        }
    }
    json.first()
        .map(|p| p.to_string())
        .unwrap_or_else(|| "preset.json".to_string())
}

/// UTC 秒 → 本地时间 `11-16 10:41`。引擎收的 `epoch` 参数是 UTC 秒，
/// 让人心算是不可能的，所以填的时候就把人话摆在旁边。
fn local_time(secs: i64) -> String {
    match chrono::DateTime::from_timestamp(secs, 0) {
        Some(d) => d
            .with_timezone(&chrono::Local)
            .format("%m-%d %H:%M")
            .to_string(),
        None => String::new(),
    }
}

/// 把输入框里的字按触发器声明的类型变成 JSON 值 —— 写成字符串引擎不认。
fn typed_json(ty: &str, raw: &str) -> serde_json::Value {
    match ty {
        "epoch" | "integer" | "int" => raw
            .parse::<i64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or_else(|_| serde_json::json!(raw)),
        "number" | "float" => raw
            .parse::<f64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or_else(|_| serde_json::json!(raw)),
        "boolean" | "bool" => {
            serde_json::json!(matches!(raw, "true" | "1" | "是" | "yes" | "on"))
        }
        _ => serde_json::json!(raw),
    }
}

/// ISO 时间戳压成 `11-15 19:42`。认不出来就原样给（别因为格式怪就把那行藏了）。
fn stamp(at: &str) -> String {
    let s = at.trim();
    if s.len() < 16 || s.as_bytes().get(10) != Some(&b'T') {
        return s.to_string();
    }
    format!("{} {}", &s[5..10], &s[11..16])
}

/// 逐行 diff（增删带底色）。`None` = 那个版本没有这个文件。
fn diff_lines(before: Option<&str>, after: Option<&str>) -> AnyElement {
    let mut col = div().flex().flex_col();
    for (op, line) in line_diff(before.unwrap_or(""), after.unwrap_or("")) {
        let (sign, fg, bg) = match op {
            DiffOp::Add => ("+", t().p.text, Some(rgba(soft(t().p.ok)))),
            DiffOp::Del => ("−", t().p.muted, Some(rgba(soft(t().p.danger)))),
            DiffOp::Same => (" ", t().p.text_2, None),
        };
        col = col.child(
            div()
                .flex()
                .flex_row()
                .gap(t().d(6.))
                .px(t().d(10.))
                .when_some(bg, |el, c| el.bg(c))
                .child(
                    div()
                        .w(t().d(8.))
                        .flex_none()
                        .text_size(t().fs(9.5))
                        .font_family("JetBrainsMono")
                        .text_color(rgb(fg))
                        .child(sign),
                )
                .child(
                    div()
                        .text_size(t().fs(10.5))
                        .font_family("JetBrainsMono")
                        .text_color(rgb(fg))
                        .child(if line.is_empty() { " ".to_string() } else { line }),
                ),
        );
    }
    col.into_any_element()
}

/// 一个 RGB 压成很淡的底（`RRGGBBAA`，给 diff 的增删行用）。
fn soft(color: u32) -> u32 {
    (color << 8) | 0x16
}

/// 这份预设有几个文件在待提交里（tab 上的徽标）。
fn pending_file_count(e: &EditorState) -> usize {
    e.pending
        .as_ref()
        .and_then(|p| p.unit(&e.name))
        .map(|u| u.files.len())
        .unwrap_or(0)
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
        Group::UBase => "◈",
        Group::UTrigger => "◷",
        Group::USession => "▤",
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
    // （坑.md 68）。
    use super::{
        DiffOp, del_key, is_dirty, line_diff, local_time, pick_manifest, read_text, stamp,
        toggle_list, typed_json, write_bool, write_text,
    };

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

    /// 改一行 = 一删一加，前后的行还算 Same（不是整份重写）。
    #[test]
    fn diff_marks_only_the_changed_line() {
        let rows = line_diff(
            "{\n  \"a\": 1\n}\n",
            "{\n  \"a\": 2\n}\n",
        );
        let same = rows.iter().filter(|(op, _)| *op == DiffOp::Same).count();
        let add = rows.iter().filter(|(op, _)| *op == DiffOp::Add).count();
        let del = rows.iter().filter(|(op, _)| *op == DiffOp::Del).count();
        assert_eq!((same, add, del), (2, 1, 1));
        assert!(rows.contains(&(DiffOp::Add, "  \"a\": 2".to_string())));
        assert!(rows.contains(&(DiffOp::Del, "  \"a\": 1".to_string())));
    }

    /// 只加一行。
    #[test]
    fn diff_of_a_pure_addition_has_no_deletions() {
        let rows = line_diff("a\nb\n", "a\nb\nc\n");
        assert_eq!(rows.iter().filter(|(op, _)| *op == DiffOp::Del).count(), 0);
        assert_eq!(rows.last(), Some(&(DiffOp::Add, "c".to_string())));
    }

    /// 新文件（before 是 null → 空串）：全是加。
    #[test]
    fn diff_of_a_new_file_is_all_additions() {
        let rows = line_diff("", "x\ny\n");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(op, _)| *op == DiffOp::Add));
    }

    /// ISO 时间压成 `MM-DD HH:MM`；认不出就原样给（不藏那行）。
    #[test]
    fn stamps_are_shortened_but_never_dropped() {
        assert_eq!(stamp("2026-11-15T19:42:44+08:00"), "11-15 19:42");
        assert_eq!(stamp("2026-01-02T03:04:05Z"), "01-02 03:04");
        assert_eq!(stamp(""), "");
        assert_eq!(stamp("昨天"), "昨天");
    }

    /// 参数按触发器声明的类型落盘：`epoch` 写成字符串引擎不认。
    fn meta(path: &str) -> nous_engine::types::PresetFileMeta {
        nous_engine::types::PresetFileMeta {
            path: path.to_string(),
            size: 1,
            extra: Default::default(),
        }
    }

    /// 清单文件名按段变（`preset.json` / `ultimate.json`），照清单挑、不写死。
    #[test]
    fn the_manifest_file_is_picked_from_the_listing() {
        assert_eq!(pick_manifest(&[meta("preset.json")]), "preset.json");
        assert_eq!(
            pick_manifest(&[meta("ultimate.json"), meta("PROMPT.md")]),
            "ultimate.json"
        );
        // 以后段里换了名字也认得。
        assert_eq!(pick_manifest(&[meta("other.json")]), "other.json");
        // 一个 json 都没有（目录刚建）：退回老名字，让后面那次读直接报 404。
        assert_eq!(pick_manifest(&[]), "preset.json");
    }

    #[test]
    fn params_are_typed_by_the_trigger_definition() {
        assert_eq!(typed_json("epoch", "1800000060"), serde_json::json!(1800000060));
        assert_eq!(typed_json("integer", "7"), serde_json::json!(7));
        assert_eq!(typed_json("number", "1.5"), serde_json::json!(1.5));
        assert_eq!(typed_json("boolean", "是"), serde_json::json!(true));
        assert_eq!(typed_json("boolean", "false"), serde_json::json!(false));
        // 认不出来的类型当文本，别把它弄丢。
        assert_eq!(typed_json("whatever", "abc"), serde_json::json!("abc"));
        // 数字类型填了非数字：退回字符串，至少不丢人的输入。
        assert_eq!(typed_json("epoch", "明天"), serde_json::json!("明天"));
    }

    /// UTC 秒要能翻成人话；翻不出来就空着（别画个假时间）。
    #[test]
    fn epoch_seconds_turn_into_local_time() {
        let s = local_time(1_800_000_060);
        assert_eq!(s.len(), 11, "MM-DD HH:MM 是 11 个字符，实际 {s:?}");
        assert_eq!(&s[2..3], "-");
        assert_eq!(&s[8..9], ":");
        assert_eq!(local_time(i64::MAX), "");
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
