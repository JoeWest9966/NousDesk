//! Template 页（最早叫「通讯录」）。从模板开新会话是客户端唯一「从零起步」的路。
//!
//! 版面是两栏：**类别树**（含段切换）+ **预设列表**。段有两条，同一个私仓按段放：
//!
//! - `Session-Template`：一条 = 一组「开一条会话时用什么」（**能开**）
//! - `Ultimate-Template`：一条 = 触发器 + 参数 + 引用的会话模板（**这一段只读**，
//!   启用触发器的动作用哪个端点还没定，别在客户端发明一个）
//!
//! 每段各有自己的类别词表（`categories.json` 住在段目录里），所以换段 = 换词表 + 换条目 +
//! 换仓状态。**零计数的类别必须画出来**（建了看不见比灰着更糟）。
//!
//! 判据全部来自引擎（`wire.md`）：`specError` 红、`resolved.*.state == "invalid"`
//! 开不了、`tracked === false` 盘上有仓里没。客户端不自己拼规则 ——
//! 这里唯一自己算的是**类别计数**，因为引擎不提供（算法：含后代、同一条预设按路径去重）。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin { crate::theme::cur() }

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, Size};
// 完整的 lucide 目录在 assets 里（component 的 IconName 只是一小撮兼容子集）。
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use nous_engine::EngineError;
use nous_engine::types::{CreateSession, CreatedSession, NewPreset, Preset, Presets};

use crate::app::{Bar, Desktop, Tab};

/// 两段。`(段的 id, 段标题)` —— id 是 API 的路径段，标题只做人读的按钮字。
/// 标题引擎也会给（`Presets::title`），但按钮在数据到之前就得有字，所以这里留一份。
///
/// 标题**直接等于 API 段名**：用户平时就按 `Session-Template` / `Ultimate-Template` 叫，
/// 界面再翻译一次只会多一套词。「终极模板」这种叫法只留在注释和文档里。
pub(crate) const SECTIONS: [(&str, &str); 2] = [
    ("Session-Template", "Session-Template"),
    ("Ultimate-Template", "Ultimate-Template"),
];

/// 圆/方块里放名字时先截断：`text_overflow` 在居中容器里会**两头**都裁，
/// 长名字会变成 `ad-only-review` 那种看着像坏了的形状。
fn short_name(name: &str, max: usize) -> String {
    let n = name.chars().count();
    if n <= max {
        return name.to_string();
    }
    let mut s: String = name.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// 未分类不是词表里的类别，是我们自己的兜底桶。用这个哨兵值放进同一个「当前类别」槽。
const UNCATEGORIZED: &str = "∅";

/// 点类别树的一行要选什么。三种只有一种「不是路径」：未分类。
#[derive(Clone)]
enum Pick {
    All,
    Cat(String),
    Uncat,
}

/// 状态行的小字色。
/// 原 `MUTED = 0x999999`；由 muted 这个语义槽接管。
fn muted() -> u32 { t().p.muted }
/// 原 `BAD = 0xd32f2f`；由 danger 这个语义槽接管。
fn bad() -> u32 { t().p.danger }
/// 原 `WARN = 0xc77700`；由 warn 这个语义槽接管。
fn warn() -> u32 { t().p.warn }
/// 原 `SELECTED_BG = 0xd8ecd9`；由 accent_soft 这个语义槽接管。
fn selected_bg() -> u32 { t().p.accent_soft }
/// 原 `SELECTED_FG = 0x14602a`；由 accent_fg 这个语义槽接管。
fn selected_fg() -> u32 { t().p.accent_fg }
/// 能开的行：微信通讯录那种「头像方块」的底色。
/// 原 `AVATAR_BG = 0x7ba7d7`；由 avatar 这个语义槽接管。
fn avatar_bg() -> u32 { t().p.avatar }
/// 状态 pill（描边，不是实心）：边框和字同色，底透明。
///
/// 实心 pill 在列表里出现两个以上就会花；纯文字又扫不到。描边是 demo 里挑的那一档
/// （`demo/template-page/lifecycle.html` 组 ③ 方案 2）。
fn status_pill(text: &str, color: u32) -> AnyElement {
    // 外面套一层 `.flex().items_center()`：pill 的父级是列，列默认 `align_items: stretch`
    // 会把它拉成整行宽（踩过一次，看着像一块黄色横幅）。
    div()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
        .flex_none()
        .h(t().d(18.))
        .px(t().d(7.))
        .flex()
        .items_center()
        .rounded(t().r(9.))
        .border(t().d(1.))
        .border_color(rgb(color))
        .text_size(t().fs(10.))
        .text_color(rgb(color))
        .child(text.to_string()),
        )
        .into_any_element()
}

/// 左栏（树）比列表深一档，才看得出是两块。
/// 原 `TREE_BG = 0xf2f2f2`；由 hover 这个语义槽接管。
fn tree_bg() -> u32 { t().p.hover }

/// 一段的全部状态：数据 / 在拉 / 拉失败。
pub(crate) struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub data: Option<Presets>,
    pub loading: bool,
    pub error: Option<String>,
}

impl Section {
    pub(crate) fn new(i: usize) -> Self {
        let (id, title) = SECTIONS[i];
        Self {
            id,
            title,
            data: None,
            loading: false,
            error: None,
        }
    }
}

/// 类别树的一行。**顺序就是引擎给的顺序**（父在子前），客户端不重排。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatNode {
    pub path: String,
    pub label: String,
    pub depth: usize,
    pub count: usize,
    pub has_children: bool,
}

/// 「新建一条」的表单草稿。`Desktop::new_preset` 是 `Some` 时右栏被它占着。
///
/// 三个输入框现建现扔：每次点「新建」都是干净的一张白纸，不用手动清空。
pub(crate) struct NewPresetDraft {
    pub name: Entity<InputState>,
    pub summary: Entity<InputState>,
    pub category: Entity<InputState>,
    /// 起手式：`true` = 跟设备默认模型起手（引擎填 `spec.model`）；`false` = 空白。
    pub from_device: bool,
    pub busy: bool,
    /// 本地先挡的那道错（名字空 / 摘要空）。引擎的 400 也落这儿。
    pub err: Option<String>,
}

/// 「新建类别」弹层。`input` 是 `Some` = 开着。
///
/// 类别是这一段的**词表**（`categories.json`），不是一个字段值 —— 所以要单独一条
/// 端点，不能混进「新建预设」的表单里。
#[derive(Default)]
pub(crate) struct NewCatDraft {
    pub input: Option<Entity<InputState>>,
    pub busy: bool,
    pub err: Option<String>,
}

/// 类别这一层的弹层：改名（输入框）或删除（确认）。
///
/// **删除会连带摘挂载**（引擎侧 `mutateCategories` 会把所有挂在它下面、含子类别的预设
/// 的 `category` 一起摘掉），所以文案必须把「几条会变成未分类」说出来，别写成「删一个空类别」。
pub(crate) enum CatDialog {
    Rename {
        name: String,
        input: Entity<InputState>,
        busy: bool,
        err: Option<String>,
    },
    Delete {
        name: String,
        /// 会被摘掉挂载的预设条数（含子类别）。
        count: usize,
        busy: bool,
        err: Option<String>,
    },
}

/// 「复制成新的」弹层。源模板的原文**在点确定时才读** —— 不然打开弹层就得先发
/// 三四个请求，还可能在用户填名字的时候源那边被改了。
pub(crate) struct CopyDraft {
    /// 从哪条复制。
    pub from: String,
    /// 新名字（默认 `<原名>-copy`，用户改）。
    pub name: Entity<InputState>,
    pub busy: bool,
    pub err: Option<String>,
}

/// 「导入」弹层。文件选完才出现：名字还能改（它就是目录名，也是引擎里的主键）。
pub(crate) struct ImportDraft {
    /// 从哪个文件来（显示给人看，出错也知道去改哪个）。
    pub from: std::path::PathBuf,
    /// 文件里的清单原文。
    pub spec: serde_json::Value,
    /// 说明全文（原 `PRESET.md`），可空。
    pub body: Option<String>,
    pub name: Entity<InputState>,
    pub busy: bool,
    pub err: Option<String>,
}

/// 「重命名」弹层。名字就是目录名，所以改它等于搬家 —— 引擎连引用一起改，
/// 这里只要一个新名字。
pub(crate) struct RenameDraft {
    pub from: String,
    pub to: Entity<InputState>,
    pub busy: bool,
    pub err: Option<String>,
}

/// 导出文件的信封版本。读的时候看这个键在不在：在 = 我们的包（有 `spec` / `body`），
/// 不在 = 裸清单（别人手工写的 `preset.json` 也直接能导进来）。
pub(crate) const EXPORT_ENVELOPE: &str = "nousdeskExport";

/// 一条模板导成什么样：`{nousdeskExport, section, name, spec, body}`。
/// 纯函数，好测 —— 导出这事的形状不该埋在 async 里。
pub(crate) fn export_doc(
    section: &str,
    name: &str,
    spec: serde_json::Value,
    body: Option<String>,
) -> serde_json::Value {
    serde_json::json!({
        EXPORT_ENVELOPE: 1,
        "section": section,
        "name": name,
        "spec": spec,
        "body": body,
    })
}

/// 拆一个导入文件：`(清单, 说明)`。带信封的和裸清单都认 —— 别人扔一份 `preset.json`
/// 过来也该能导，那是这个功能最常见的用法。
pub(crate) fn import_doc(
    doc: serde_json::Value,
) -> Result<(serde_json::Value, Option<String>), String> {
    if !doc.is_object() {
        return Err("这个文件不是一份模板（顶层不是 JSON 对象）".into());
    }
    if doc.get(EXPORT_ENVELOPE).is_some() {
        let spec = doc
            .get("spec")
            .cloned()
            .ok_or_else(|| "导出包里没有 `spec`".to_string())?;
        if !spec.is_object() {
            return Err("导出包里的 `spec` 不是一个对象".into());
        }
        let body = doc
            .get("body")
            .and_then(|b| b.as_str())
            .map(|s| s.to_string());
        return Ok((spec, body));
    }
    Ok((doc, None))
}

/// 导出时的默认目录：有 `~/下载` / `~/Downloads` 就去那儿，否则家目录。
pub(crate) fn export_dir() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    for d in ["下载", "Downloads"] {
        let p = home.join(d);
        if p.is_dir() {
            return p;
        }
    }
    home
}

/// 模板名当文件名用之前把路径分隔符那些换掉（名字里带 `/` 的模板是合法的，比如 `写作/周报`）。
pub(crate) fn sanitize_file_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() {
        "template".to_string()
    } else {
        s
    }
}

/// 过滤一条模板：名字 / 摘要 / 类别，任一包含就算命中（不分大小写）。
///
/// 类别也搜 —— 人在列表里看见的是「写作/周报」，按它找比记住模板名自然。
pub(crate) fn matches_query(p: &Preset, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    p.name.to_lowercase().contains(&q)
        || p.summary.to_lowercase().contains(&q)
        || p.category.iter().any(|c| c.to_lowercase().contains(&q))
}

/// 「改类别」弹层的草稿。`picked` 是**整份**类别（勾掉就删），保存时整份发过去。
pub(crate) struct CatEditDraft {
    pub name: String,
    pub picked: Vec<String>,
    pub busy: bool,
    pub err: Option<String>,
}

impl Desktop {
    // ── 数据 ────────────────────────────────────────────────

    /// 切左栏入口。切到 Template 页时顺手拉当前段（拉过就不重拉）。
    pub(crate) fn switch_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        if tab == Tab::Templates {
            self.load_all_sections(false, cx);
        }
        if tab == Tab::Triggers {
            self.load_trigger_list(false, cx);
        }
        cx.notify();
    }

    /// 拉当前段。已拉到且非 force 就不动。
    pub(crate) fn load_presets(&mut self, force: bool, cx: &mut Context<Self>) {
        self.load_section(self.section, force, cx);
    }

    /// 两段都拉一遍。段按钮下面要写真实条数，「—」看着像坏了。
    ///
    /// 副作用是想要的：引擎**连上之前**打开过 Template 页会留下「还没连上引擎」，
    /// 这里跑一遍会把它清掉（`load_section` 开头就 `error = None`）。
    pub(crate) fn load_all_sections(&mut self, force: bool, cx: &mut Context<Self>) {
        for i in 0..SECTIONS.len() {
            self.load_section(i, force, cx);
        }
    }

    pub(crate) fn load_section(&mut self, i: usize, force: bool, cx: &mut Context<Self>) {
        let sec = &mut self.sections[i];
        if sec.loading {
            return;
        }
        if !force && sec.data.is_some() {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.sections[i].error = Some("还没连上引擎".into());
            cx.notify();
            return;
        };
        let section_id = SECTIONS[i].0;
        self.sections[i].loading = true;
        self.sections[i].error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result: Option<Result<Presets, EngineError>> = match &handle {
                Some(h) => h
                    .spawn(async move { engine.presets(section_id).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.sections[i].loading = false;
                match result {
                    Some(Ok(p)) => {
                        // 切段时「能留就留」：数据到了才发现新段没有那个类别，就回「全部」。
                        if let Some(cat) = this.preset_cat.clone() {
                            if cat != UNCATEGORIZED {
                                let tree = facet_tree(&p.categories, &p.presets);
                                if !tree.iter().any(|n| n.path == cat) {
                                    this.preset_cat = None;
                                }
                            }
                        }
                        this.sections[i].data = Some(p);
                        // 验界面用：数据一到就把第一条的编辑器拉起来。
                        // 只在第一段（Session-Template）里挑 —— 终极模板那边可能有空壳，
                        // 打开会直接撞 404。
                        if this.auto_editor && i == 0 {
                            this.auto_editor = false;
                            if let Some(first) = this.sections[i]
                                .data
                                .as_ref()
                                .and_then(|d| d.presets.first())
                                .map(|p| p.name.clone())
                            {
                                this.open_editor(first, cx);
                            }
                        }
                    }
                    Some(Err(e)) => this.sections[i].error = Some(e.to_string()),
                    None => this.sections[i].error = Some("任务没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 换段。段按钮只改「看哪一段」，不动别的栏。
    ///
    /// **能留就留**：新段里还有同一个类别路径就停在那儿，没有才回「全部」。
    /// 数据还没到就先留着，等 `load_section` 回来再判 —— 否则切过去总是被扔回列表顶。
    pub(crate) fn switch_section(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.section == i || i >= SECTIONS.len() {
            return;
        }
        self.section = i;
        self.preset_closed.clear();
        self.drop_picked();
        if let Some(d) = &self.sections[i].data {
            let tree = facet_tree(&d.categories, &d.presets);
            self.preset_cat = keep_category(&self.preset_cat, &tree);
        }
        self.load_section(i, false, cx);
        cx.notify();
    }

    /// 选类别（`None` = 全部，`Some("∅")` = 未分类）。
    fn pick_category(&mut self, cat: Option<String>, cx: &mut Context<Self>) {
        // 点类别 = 回到普通列表（归档视图只有一个口袋，不该和类别树并排）。
        self.archive_view = false;
        self.preset_cat = cat;
        // 换了类别，右栏那条多半不在视图里了 —— 留着它会显示「这条已经不在列表里」，
        // 像坏了。宁可让它回到空状态。
        self.drop_picked();
        cx.notify();
    }

    /// 清掉右栏的选中（换段 / 换类别 / Esc 都走这里，别在各处抄一遍）。
    fn drop_picked(&mut self) {
        self.picked_preset = None;
        self.picked_node = None;
    }

    fn toggle_closed(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.preset_closed.remove(&path) {
            self.preset_closed.insert(path);
        }
        cx.notify();
    }

    /// 从模板开一条会话：`POST /v1/sessions { preset: "Session-Template/<name>", name }`。
    /// 成功后重拉会话列表、选中新会话、切回聊天页。
    /// 点列表一行：右栏那张关系图换到这条上。**不开会话**（那是行尾圆按钮的事）。
    /// 打开「新建一条」表单。右栏被表单占住，关系图让开。
    pub(crate) fn open_new_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_preset.is_some() {
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("deep-dive"));
        let summary = cx.new(|cx| {
            InputState::new(window, cx).placeholder("一句话说清这条模板干什么")
        });
        let category = cx.new(|cx| {
            InputState::new(window, cx).placeholder("工程 —— 可写 写作/周报 分层")
        });
        self.new_preset = Some(NewPresetDraft {
            name,
            summary,
            category,
            from_device: false,
            busy: false,
            err: None,
        });
        self.picked_preset = None;
        self.picked_node = None;
        cx.notify();
    }

    /// 收起表单（取消 / Esc / 建完）。
    pub(crate) fn close_new_preset(&mut self, cx: &mut Context<Self>) {
        if self.new_preset.take().is_some() {
            cx.notify();
        }
    }

    /// 切起手式。`from_device` 只影响发给引擎的 `from` 字段。
    pub(crate) fn set_new_from(&mut self, device: bool, cx: &mut Context<Self>) {
        if let Some(d) = self.new_preset.as_mut() {
            d.from_device = device;
            cx.notify();
        }
    }

    /// 真的建。**本地只挡「名字空 / 摘要空」两道**，其余（重名、在归档区、形状闸）
    /// 一律交给引擎 —— 它那边判得更全，本地再写一遍只会两边慢慢不一致。
    pub(crate) fn submit_new_preset(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.new_preset.as_ref() else {
            return;
        };
        if draft.busy {
            return;
        }
        let name = draft.name.read(cx).value().trim().to_string();
        let summary = draft.summary.read(cx).value().trim().to_string();
        let raw_cat = draft.category.read(cx).value().trim().to_string();
        let from_device = draft.from_device;

        let local_err = if name.is_empty() {
            Some("名字不能空 —— 它就是落盘时的目录名".to_string())
        } else if summary.is_empty() {
            Some("摘要不能空 —— 人靠这一行决定开不开它".to_string())
        } else {
            None
        };
        if let Some(e) = local_err {
            if let Some(d) = self.new_preset.as_mut() {
                d.err = Some(e);
            }
            cx.notify();
            return;
        }

        let Some(engine) = self.engine.clone() else {
            if let Some(d) = self.new_preset.as_mut() {
                d.err = Some("还没连上引擎".into());
            }
            cx.notify();
            return;
        };

        let section_id = SECTIONS[self.section].0;
        let sec_idx = self.section;
        let body = NewPreset {
            name: name.clone(),
            summary,
            // 逗号分一项，空项扔掉。分层路径（`写作/周报`）原样发，引擎自己拆。
            category: raw_cat
                .split([',', '，'])
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect(),
            from: from_device.then(|| "device".to_string()),
            spec: None,
            body: None,
        };
        if let Some(d) = self.new_preset.as_mut() {
            d.busy = true;
            d.err = None;
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result: Option<Result<serde_json::Value, EngineError>> = match &handle {
                Some(h) => h
                    .spawn(async move { engine.create_preset(section_id, &body).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(_)) => {
                        // 建完就收表单、重拉这一段（新行得出现），并把它选中。
                        this.new_preset = None;
                        this.picked_preset = Some(name);
                        this.picked_node = None;
                        this.load_section(sec_idx, true, cx);
                    }
                    Some(Err(e)) => {
                        if let Some(d) = this.new_preset.as_mut() {
                            d.busy = false;
                            d.err = Some(e.to_string());
                        }
                    }
                    None => {
                        if let Some(d) = this.new_preset.as_mut() {
                            d.busy = false;
                            d.err = Some("任务没跑完".into());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── 新建类别 ────────────────────────────────────────────

    pub(crate) fn open_new_category(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_cat.input.is_some() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("写作/周报 —— 写斜杠就是分层")
        });
        self.new_cat = NewCatDraft {
            input: Some(input),
            busy: false,
            err: None,
        };
        cx.notify();
    }

    pub(crate) fn close_new_category(&mut self, cx: &mut Context<Self>) {
        if self.new_cat.input.take().is_some() {
            self.new_cat.busy = false;
            self.new_cat.err = None;
            cx.notify();
        }
    }

    /// 打开「复制成新的」：只记源模板名，原文**点确定才读**。
    pub(crate) fn open_copy(&mut self, from: String, window: &mut Window, cx: &mut Context<Self>) {
        // 先给一个能直接用的名字（`原名-copy`），大多时候改两个字就够了。
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("新名字（就是目录名）")
                .default_value(format!("{from}-copy"))
        });
        // 弹层里的输入框必须走组件自己的 focus（`window.focus(&handle)` 只是半个焦点，坑.md 72）。
        input.update(cx, |st, cx| st.focus(window, cx));
        self.preset_copy = Some(CopyDraft {
            from,
            name: input,
            busy: false,
            err: None,
        });
        cx.notify();
    }

    pub(crate) fn close_copy(&mut self, cx: &mut Context<Self>) {
        if self.preset_copy.take().is_some() {
            cx.notify();
        }
    }

    /// 复制一条模板：读源目录的清单原文 + `PRESET.md`，改个名字重新落一份。
    ///
    /// 为什么不请求「复制目录」的端点：引擎的创建口本来就收整份 `spec`（`body.spec`），
    /// 它会覆盖 `name`/`summary`/`category` 三个键 —— 拷贝这件事不需要引擎再开一条路。
    /// 这也意味着**新模板的其余字段原样带过去**（模型、ctx、工具权限…），这正是「做变体」要的。
    pub(crate) fn submit_copy(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.preset_copy.as_ref() else {
            return;
        };
        if draft.busy {
            return;
        }
        let name = draft.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            if let Some(d) = self.preset_copy.as_mut() {
                d.err = Some("名字不能空 —— 它就是目录名".into());
            }
            cx.notify();
            return;
        }
        let from = draft.from.clone();
        let Some(engine) = self.engine.clone() else {
            if let Some(d) = self.preset_copy.as_mut() {
                d.err = Some("还没连上引擎".into());
            }
            cx.notify();
            return;
        };
        let section_id = SECTIONS[self.section].0;
        let sec_idx = self.section;
        if let Some(d) = self.preset_copy.as_mut() {
            d.busy = true;
            d.err = None;
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let out: Result<(), String> = async {
                // 1) 源目录里有哪些文件 → 挑清单（段的清单名不写死，见 `pick_manifest`）。
                let files = engine
                    .preset_files(section_id, &from)
                    .await
                    .map_err(|e| e.to_string())?;
                let manifest = crate::editor::pick_manifest(&files.files);
                let src = engine
                    .preset_file(section_id, &from, &manifest)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut spec: serde_json::Value =
                    serde_json::from_str(&src.text).map_err(|e| format!("源模板不是合法 JSON：{e}"))?;
                // 2) 摘要 / 类别跟着原文走（`create` 会用这两个字段覆盖 spec 里的同名键）。
                let summary = spec
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let category: Vec<String> = spec
                    .get("category")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|c| c.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                // 3) 说明书（`PRESET.md`）也带过去 —— 复制完发现说明没了，等于丢一半。
                let body = if files.files.iter().any(|f| f.path == "PRESET.md") {
                    engine
                        .preset_file(section_id, &from, "PRESET.md")
                        .await
                        .ok()
                        .map(|f| f.text)
                } else {
                    None
                };
                // 4) 落一份新的。`spec` 原样发，引擎覆盖 name/summary/category。
                let body = NewPreset {
                    name: name.clone(),
                    summary,
                    category,
                    from: None,
                    spec: Some(spec.take()),
                    body,
                };
                engine
                    .create_preset(section_id, &body)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            .await;

            this.update(cx, |this, cx| {
                match out {
                    Ok(()) => {
                        this.preset_copy = None;
                        this.notify(
                            format!("已复制成「{name}」"),
                            crate::app::NoticeKind::Info,
                            cx,
                        );
                        // 复制完停在新的那条上：看得见才算复制成功。
                        this.preset_cat = None;
                        this.archive_view = false;
                        this.load_section(sec_idx, true, cx);
                        this.picked_preset = Some(name.clone());
                    }
                    Err(e) => {
                        if let Some(d) = this.preset_copy.as_mut() {
                            d.busy = false;
                            d.err = Some(e);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 建类别。引擎回的是**整段重算过**的词表 + 名册，直接覆盖本地。
    pub(crate) fn submit_new_category(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.new_cat.input.clone() else {
            return;
        };
        if self.new_cat.busy {
            return;
        }
        let name = input.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.new_cat.err = Some("类别名不能空".into());
            cx.notify();
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.new_cat.err = Some("还没连上引擎".into());
            cx.notify();
            return;
        };
        let section_id = SECTIONS[self.section].0;
        let sec_idx = self.section;
        self.new_cat.busy = true;
        self.new_cat.err = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let body = nous_engine::types::CategoryOp {
                op: "add".into(),
                name: name.clone(),
                to: None,
            };
            let handle = crate::TOKIO.get().cloned();
            let result: Option<Result<nous_engine::types::CategoriesResult, EngineError>> =
                match &handle {
                    Some(h) => h
                        .spawn(async move { engine.mutate_categories(section_id, &body).await })
                        .await
                        .ok(),
                    _ => None,
                };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(r)) => {
                        this.new_cat = NewCatDraft::default();
                        // 只信 `categories`（词表就是这条端点的事）。名册那份是**半成品**，
                        // 不覆盖 —— 重拉一次那一段，拿带 `tracked`/`resolved` 的完整数据。
                        if let Some(d) = this.sections[sec_idx].data.as_mut() {
                            d.categories = r.categories;
                        }
                        this.preset_closed.clear();
                        // 建完就停在那个类别上：看不见刚建的类别等于没建。
                        this.preset_cat = Some(name);
                        this.load_section(sec_idx, true, cx);
                    }
                    Some(Err(e)) => {
                        this.new_cat.busy = false;
                        this.new_cat.err = Some(e.to_string());
                    }
                    None => {
                        this.new_cat.busy = false;
                        this.new_cat.err = Some("任务没跑完".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 打开类别的「改名」弹层。
    pub(crate) fn open_cat_rename(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("新类别名")
                .default_value(name.clone())
        });
        // 弹层输入框走组件自己的 focus（坑.md 72）。
        input.update(cx, |st, cx| st.focus(window, cx));
        self.cat_dialog = Some(CatDialog::Rename {
            name,
            input,
            busy: false,
            err: None,
        });
        cx.notify();
    }

    /// 打开类别的「删除」弹层。`count` 是会被摘掉挂载的条数（含子类别）。
    pub(crate) fn open_cat_delete(&mut self, name: String, count: usize, cx: &mut Context<Self>) {
        self.cat_dialog = Some(CatDialog::Delete {
            name,
            count,
            busy: false,
            err: None,
        });
        cx.notify();
    }

    pub(crate) fn close_cat_dialog(&mut self, cx: &mut Context<Self>) {
        if self.cat_dialog.take().is_some() {
            cx.notify();
        }
    }

    /// 类别改名 / 删除。两者都走 `mutate_categories`，回包带着**重算过**的词表和名册。
    ///
    /// 回包里的 `presets` 是**半成品**（没有 `tracked` / `resolved`），所以只拿 `categories`，
    /// 名册照样重拉 —— 跟新建类别那条一个道理。
    pub(crate) fn submit_cat_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.cat_dialog.as_ref() else {
            return;
        };
        // 先把要用的东西全拷出来，**别**让这只 `&` 活到后面的 `as_mut()`。
        let (is_rename, from, input, to) = match d {
            CatDialog::Rename { name, input, busy, .. } => {
                if *busy {
                    return;
                }
                (
                    true,
                    name.clone(),
                    Some(input.clone()),
                    input.read(cx).value().trim().to_string(),
                )
            }
            CatDialog::Delete { name, busy, .. } => {
                if *busy {
                    return;
                }
                (false, name.clone(), None, String::new())
            }
        };

        if is_rename && to.is_empty() {
            if let Some(CatDialog::Rename { err, .. }) = self.cat_dialog.as_mut() {
                *err = Some("新名字不能空".into());
            }
            cx.notify();
            return;
        }
        if is_rename && to == from {
            self.close_cat_dialog(cx);
            return;
        }

        let body = nous_engine::types::CategoryOp {
            op: if is_rename { "rename" } else { "remove" }.into(),
            name: from.clone(),
            to: is_rename.then(|| to.clone()),
        };
        let Some(engine) = self.engine.clone() else {
            if let Some(d) = self.cat_dialog.as_mut() {
                match d {
                    CatDialog::Rename { err, .. } | CatDialog::Delete { err, .. } => {
                        *err = Some("还没连上引擎".into())
                    }
                }
            }
            cx.notify();
            return;
        };
        let _ = input;
        if let Some(d) = self.cat_dialog.as_mut() {
            match d {
                CatDialog::Rename { busy, err, .. } | CatDialog::Delete { busy, err, .. } => {
                    *busy = true;
                    *err = None;
                }
            }
        }
        cx.notify();

        let section_id = SECTIONS[self.section].0;
        let sec_idx = self.section;
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move { engine.mutate_categories(section_id, &body).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(r)) => {
                        this.cat_dialog = None;
                        if let Some(d) = this.sections[sec_idx].data.as_mut() {
                            d.categories = r.categories;
                        }
                        // 当前选中的类别被改名 / 删掉了，就跟着走（否则列表停在一条不存在的类别上）。
                        let cur = this.preset_cat.clone().unwrap_or_default();
                        if cur == from || cur.starts_with(&format!("{from}/")) {
                            let tail = &cur[from.len()..];
                            this.preset_cat = if !is_rename {
                                None
                            } else if tail.is_empty() {
                                Some(to.clone())
                            } else {
                                Some(format!("{to}{tail}"))
                            };
                        }
                        this.load_section(sec_idx, true, cx);
                    }
                    Some(Err(e)) => {
                        if let Some(d) = this.cat_dialog.as_mut() {
                            match d {
                                CatDialog::Rename { busy, err, .. }
                                | CatDialog::Delete { busy, err, .. } => {
                                    *busy = false;
                                    *err = Some(e.to_string());
                                }
                            }
                        }
                    }
                    None => {
                        if let Some(d) = this.cat_dialog.as_mut() {
                            match d {
                                CatDialog::Rename { busy, err, .. }
                                | CatDialog::Delete { busy, err, .. } => {
                                    *busy = false;
                                    *err = Some("任务没跑完".into());
                                }
                            }
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 导出一条模板成一个文件。不改仓里任何东西 —— 备份 / 发给别人用这个。
    ///
    /// 走平台的「另存为」对话框；取消就静默（用户自己的动作，不用回一句废话）。
    pub(crate) fn export_preset(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let section = SECTIONS[self.section].0.to_string();
        self.notify("正在读那条模板…", crate::app::NoticeKind::Info, cx);
        cx.spawn(async move |this, cx| {
            let out: Result<serde_json::Value, String> = async {
                let files = engine
                    .preset_files(&section, &name)
                    .await
                    .map_err(|e| e.to_string())?;
                let manifest = crate::editor::pick_manifest(&files.files);
                let src = engine
                    .preset_file(&section, &name, &manifest)
                    .await
                    .map_err(|e| e.to_string())?;
                let spec: serde_json::Value = serde_json::from_str(&src.text)
                    .map_err(|e| format!("清单不是合法 JSON：{e}"))?;
                let body = if files.files.iter().any(|f| f.path == "PRESET.md") {
                    engine
                        .preset_file(&section, &name, "PRESET.md")
                        .await
                        .ok()
                        .map(|f| f.text)
                } else {
                    None
                };
                Ok(export_doc(&section, &name, spec, body))
            }
            .await;

            let _ = this.update(cx, |this, cx| {
                let doc = match out {
                    Ok(d) => d,
                    Err(e) => {
                        this.notify(format!("导不出来：{e}"), crate::app::NoticeKind::Error, cx);
                        return;
                    }
                };
                // 「另存为」得回前台，所以放在 update 里开。
                let dir = export_dir();
                let suggested = format!("{}.json", sanitize_file_name(&name));
                let rx = cx.prompt_for_new_path(&dir, Some(&suggested));
                cx.spawn(async move |this, cx| {
                    let picked = match rx.await {
                        Ok(Ok(Some(p))) => p,
                        Ok(Ok(None)) => return, // 用户取消
                        Ok(Err(e)) => {
                            let _ = this.update(cx, |this, cx| {
                                this.notify(
                                    format!("打不开保存对话框：{e}"),
                                    crate::app::NoticeKind::Error,
                                    cx,
                                )
                            });
                            return;
                        }
                        Err(_) => return,
                    };
                    let text = match serde_json::to_string_pretty(&doc) {
                        Ok(t) => t + "\n",
                        Err(e) => {
                            let _ = this.update(cx, |this, cx| {
                                this.notify(
                                    format!("拼不出文件内容：{e}"),
                                    crate::app::NoticeKind::Error,
                                    cx,
                                )
                            });
                            return;
                        }
                    };
                    let res = std::fs::write(&picked, text);
                    let _ = this.update(cx, |this, cx| match res {
                        Ok(()) => this.notify(
                            format!("已导出到 {}", picked.display()),
                            crate::app::NoticeKind::Info,
                            cx,
                        ),
                        Err(e) => this.notify(
                            format!("写不进 {}：{e}", picked.display()),
                            crate::app::NoticeKind::Error,
                            cx,
                        ),
                    });
                })
                .detach();
            });
        })
        .detach();
    }

    /// 选一份导出文件，读出来之后弹「导入为…」（名字还能改，目录名就是它）。
    pub(crate) fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let opts = PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选一份导出的模板 .json".into()),
        };
        let rx = cx.prompt_for_paths(opts);
        let _ = window;
        cx.spawn(async move |this, cx| {
            let picked = match rx.await {
                Ok(Ok(Some(mut v))) if !v.is_empty() => v.remove(0),
                Ok(Ok(_)) => return, // 取消
                Ok(Err(e)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.notify(
                            format!("打不开文件对话框：{e}"),
                            crate::app::NoticeKind::Error,
                            cx,
                        )
                    });
                    return;
                }
                Err(_) => return,
            };
            let parsed: Result<(serde_json::Value, Option<String>), String> =
                std::fs::read_to_string(&picked)
                    .map_err(|e| format!("读不了 {}：{e}", picked.display()))
                    .and_then(|text| {
                        serde_json::from_str::<serde_json::Value>(&text)
                            .map_err(|e| format!("不是合法 JSON：{e}"))
                    })
                    .and_then(import_doc);

            let _ = this.update_in(cx, |this, window, cx| {
                let (spec, body) = match parsed {
                    Ok(v) => v,
                    Err(e) => {
                        this.notify(format!("导不进来：{e}"), crate::app::NoticeKind::Error, cx);
                        return;
                    }
                };
                // 默认名：文件里写的是 `name` → 文件名（去掉扩展名）→ 兜底一个。
                let fallback = picked
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let name = spec
                    .get("name")
                    .and_then(|n| n.as_str())
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| n.to_string())
                    .unwrap_or(fallback);
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("导进来叫什么")
                        .default_value(name)
                });
                input.update(cx, |st, cx| st.focus(window, cx));
                this.preset_import = Some(ImportDraft {
                    from: picked,
                    spec,
                    body,
                    name: input,
                    busy: false,
                    err: None,
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_import(&mut self, cx: &mut Context<Self>) {
        if self.preset_import.take().is_some() {
            cx.notify();
        }
    }

    /// 落一份新的：`spec` 原样发，引擎用 name / summary / category 覆盖同名键。
    /// 摘要和类别从清单里读 —— 导进来的包不额外要求用户再填一遍。
    pub(crate) fn submit_import(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.preset_import.as_ref() else {
            return;
        };
        if draft.busy {
            return;
        }
        let name = draft.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            if let Some(d) = self.preset_import.as_mut() {
                d.err = Some("名字不能空 —— 它就是目录名".into());
            }
            cx.notify();
            return;
        }
        let spec = draft.spec.clone();
        let body = draft.body.clone();
        let Some(engine) = self.engine.clone() else {
            if let Some(d) = self.preset_import.as_mut() {
                d.err = Some("还没连上引擎".into());
            }
            cx.notify();
            return;
        };
        let section_id = SECTIONS[self.section].0;
        let sec_idx = self.section;
        let summary = spec
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let category: Vec<String> = spec
            .get("category")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(d) = self.preset_import.as_mut() {
            d.busy = true;
            d.err = None;
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let req = NewPreset {
                name: name.clone(),
                summary,
                category,
                from: None,
                spec: Some(spec),
                body,
            };
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move { engine.create_preset(section_id, &req).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(_)) => {
                        this.preset_import = None;
                        this.notify(
                            format!("已导入「{name}」"),
                            crate::app::NoticeKind::Info,
                            cx,
                        );
                        this.preset_cat = None;
                        this.archive_view = false;
                        this.load_section(sec_idx, true, cx);
                        this.picked_preset = Some(name.clone());
                    }
                    Some(Err(e)) => {
                        if let Some(d) = this.preset_import.as_mut() {
                            d.busy = false;
                            d.err = Some(e.to_string());
                        }
                    }
                    None => {
                        if let Some(d) = this.preset_import.as_mut() {
                            d.busy = false;
                            d.err = Some("任务没跑完".into());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 重命名弹层。默认值就是原名 —— 改两个字比重打一遍快。
    pub(crate) fn open_preset_rename(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("新名字：小写字母 / 数字开头，只留 . _ -")
                .default_value(name.clone())
        });
        input.update(cx, |st, cx| st.focus(window, cx));
        self.preset_rename = Some(RenameDraft {
            from: name,
            to: input,
            busy: false,
            err: None,
        });
        cx.notify();
    }

    pub(crate) fn close_preset_rename(&mut self, cx: &mut Context<Self>) {
        if self.preset_rename.take().is_some() {
            cx.notify();
        }
    }

    /// 改名字。**不在客户端拼目录 / 改清单 / 更新引用** —— 那是引擎一次做的三件事，
    /// 少做一件就是红（目录和 `name` 对不上，或者引用它的终极模板指向一个不存在的名字）。
    pub(crate) fn submit_preset_rename(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.preset_rename.as_ref() else {
            return;
        };
        if draft.busy {
            return;
        }
        let from = draft.from.clone();
        let to = draft.to.read(cx).value().trim().to_string();
        if to.is_empty() {
            if let Some(d) = self.preset_rename.as_mut() {
                d.err = Some("新名字不能空 —— 它就是目录名".into());
            }
            cx.notify();
            return;
        }
        if to == from {
            if let Some(d) = self.preset_rename.as_mut() {
                d.err = Some("新名字和旧名字一样".into());
            }
            cx.notify();
            return;
        }
        let Some(engine) = self.engine.clone() else {
            if let Some(d) = self.preset_rename.as_mut() {
                d.err = Some("还没连上引擎".into());
            }
            cx.notify();
            return;
        };
        let section_id = SECTIONS[self.section].0;
        let sec_idx = self.section;
        if let Some(d) = self.preset_rename.as_mut() {
            d.busy = true;
            d.err = None;
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let body = nous_engine::types::RenamePreset {
                name: from.clone(),
                to: to.clone(),
            };
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => {
                    let engine = engine.clone();
                    h.spawn(async move { engine.rename_preset(section_id, &body).await })
                        .await
                        .ok()
                }
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(done)) => {
                        this.preset_rename = None;
                        // 引用了它的终极模板是引擎连带改的 —— 不说一声就是偷偷动了别人的东西。
                        let extra = if done.refs.is_empty() {
                            String::new()
                        } else {
                            format!("（顺带改了引用它的 {} 条终极模板：{}）", done.refs.len(), done.refs.join("、"))
                        };
                        this.notify(
                            format!("已改名为「{to}」{extra}"),
                            crate::app::NoticeKind::Info,
                            cx,
                        );
                        this.load_section(sec_idx, true, cx);
                        this.picked_preset = Some(to.clone());
                    }
                    Some(Err(e)) => {
                        if let Some(d) = this.preset_rename.as_mut() {
                            d.busy = false;
                            d.err = Some(e.to_string());
                        }
                    }
                    None => {
                        if let Some(d) = this.preset_rename.as_mut() {
                            d.busy = false;
                            d.err = Some("任务没跑完".into());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 重命名弹层。
    pub(crate) fn preset_rename_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let draft = self.preset_rename.as_ref()?;
        let from = draft.from.clone();
        let busy = draft.busy;
        let err = draft.err.clone();

        let scrim = div()
            .id("rename-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_preset_rename(cx)));

        let mut card = div()
            .flex()
            .flex_col()
            .w(t().d(400.))
            .rounded(t().r(10.))
            .bg(rgb(t().p.panel))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(18.))
                    .text_size(t().fs(14.))
                    .text_color(rgb(t().p.text))
                    .child(format!("重命名「{from}」")),
            )
            .child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(6.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .child("目录跟着改名（git 认得出是改名，历史不断）；挂在它上面的终极模板会一起指着新名字。"),
            )
            .child(div().px(t().d(20.)).pt(t().d(12.)).child(Input::new(&draft.to)));
        if let Some(e) = err {
            card = card.child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(8.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.danger_fg))
                    .child(e),
            );
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .child(scrim)
                .child(
                    div()
                        .id("rename-dialog")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            cx.stop_propagation()
                        })
                        .child(card.child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap(t().d(8.))
                                .px(t().d(20.))
                                .py(t().d(16.))
                                .child(
                                    div()
                                        .id("rename-cancel")
                                        .flex()
                                        .items_center()
                                        .h(t().d(28.))
                                        .px(t().d(12.))
                                        .rounded(t().r(6.))
                                        .text_size(t().fs(12.))
                                        .text_color(rgb(t().p.text_2))
                                        .cursor_pointer()
                                        .hover(|h| h.bg(rgb(t().p.hover)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.close_preset_rename(cx)
                                        }))
                                        .child("取消"),
                                )
                                .child(
                                    div()
                                        .id("rename-ok")
                                        .flex()
                                        .items_center()
                                        .h(t().d(28.))
                                        .px(t().d(14.))
                                        .rounded(t().r(6.))
                                        .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                                        .text_size(t().fs(12.))
                                        .text_color(rgb(t().p.on_accent))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.submit_preset_rename(cx)
                                        }))
                                        .child(if busy { "改名中…" } else { "改名" }),
                                ),
                        )),
                )
                .into_any_element(),
        )
    }

    /// 「导入」弹层：文件已经读进来了，这里只剩「叫什么」。
    pub(crate) fn import_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let draft = self.preset_import.as_ref()?;
        let from = draft.from.display().to_string();
        let err = draft.err.clone();
        let busy = draft.busy;

        let scrim = div()
            .id("import-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_import(cx)));

        let mut boxed = div()
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
                    .pt(t().d(18.))
                    .text_size(t().fs(14.))
                    .text_color(rgb(t().p.text))
                    .child("导入模板"),
            )
            .child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(6.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .child(format!("来自 {from}")),
            )
            .child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(12.))
                    .child(Input::new(&draft.name)),
            );
        if let Some(e) = err {
            boxed = boxed.child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(8.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.danger_fg))
                    .child(e),
            );
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .child(scrim)
                .child(
                    div()
                        .id("import-dialog")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            cx.stop_propagation()
                        })
                        .child(boxed.child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_end()
                                .gap(t().d(8.))
                                .px(t().d(20.))
                                .py(t().d(16.))
                                .child(
                                    div()
                                        .id("import-cancel")
                                        .flex()
                                        .items_center()
                                        .h(t().d(28.))
                                        .px(t().d(12.))
                                        .rounded(t().r(6.))
                                        .text_size(t().fs(12.))
                                        .text_color(rgb(t().p.text_2))
                                        .cursor_pointer()
                                        .hover(|h| h.bg(rgb(t().p.hover)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.close_import(cx)
                                        }))
                                        .child("取消"),
                                )
                                .child(
                                    div()
                                        .id("import-ok")
                                        .flex()
                                        .items_center()
                                        .h(t().d(28.))
                                        .px(t().d(14.))
                                        .rounded(t().r(6.))
                                        .bg(rgb(if busy {
                                            t().p.disabled
                                        } else {
                                            t().p.accent
                                        }))
                                        .text_size(t().fs(12.))
                                        .text_color(rgb(t().p.on_accent))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.submit_import(cx)
                                        }))
                                        .child(if busy { "导入中…" } else { "导入" }),
                                ),
                        )),
                )
                .into_any_element(),
        )
    }

    /// 窄窗下的「类别」抽屉：把常驻的那一栏原样搬到一个左滑的浮层里。
    ///
    /// 只在 `TREE_FOLD_W` 以下出现（宽窗类别树本来就常驻，抽屉永远关着）。
    pub(crate) fn cat_drawer_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.cat_drawer {
            return None;
        }
        let scrim = div()
            .id("cat-drawer-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.cat_drawer = false;
                cx.notify();
            }));

        // 抽屉里的东西**就是**那一栏本身：同一份状态、同一套点击逻辑，
        // 不另开一套「窄屏专用类别列表」。
        let panel = div()
            .id("cat-drawer")
            .absolute()
            .top(t().d(0.))
            .bottom(t().d(0.))
            .left(px(t().rail_w))
            .shadow_lg()
            .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
            .child(self.render_preset_tree(cx));

        Some(
            div()
                .absolute()
                .inset_0()
                .child(scrim)
                .child(panel)
                .into_any_element(),
        )
    }

    /// 弹层本体。罩子点掉 = 关闭；弹窗吃掉点击。
    pub(crate) fn new_cat_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let input = self.new_cat.input.clone()?;
        let busy = self.new_cat.busy;
        let err = self.new_cat.err.clone();
        let sec = SECTIONS[self.section].1;

        let scrim = div()
            .id("newcat-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_new_category(cx)));

        let dialog = div()
            .id("newcat-dialog")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            // **必须 stop**：不拦的话同一次点击会继续传给它下面的罩子，
            // 表现成「点『建』没反应、弹窗还自己关了」。
            .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(t().d(380.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(18.))
                            .text_size(t().fs(14.))
                            .text_color(rgb(t().p.text))
                            .child("新建类别"),
                    )
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(4.))
                            .pb(t().d(14.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(format!(
                                "{sec} 的词表。写 写作/周报 就是分层路径。"
                            )),
                    )
                    .child(div().px(t().d(20.)).child(Input::new(&input)))
                    .children(err.map(|e| {
                        div()
                            .px(t().d(20.))
                            .pt(t().d(8.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.danger_fg))
                            .child(e)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .px(t().d(20.))
                            .py(t().d(16.))
                            .child(
                                div()
                                    .id("newcat-cancel")
                                    .flex()
                                    .items_center()
                                    .h(t().d(28.))
                                    .px(t().d(12.))
                                    .rounded(t().r(6.))
                                    .text_size(t().fs(12.))
                                    .text_color(rgb(t().p.text_2))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_new_category(cx)
                                    }))
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("newcat-ok")
                                    .flex()
                                    .items_center()
                                    .h(t().d(28.))
                                    .px(t().d(14.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                                    .text_size(t().fs(12.))
                                    .text_color(rgb(t().p.on_accent))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.submit_new_category(cx)
                                    }))
                                    .child(if busy { "建…" } else { "建" }),
                            ),
                    ),
            );

        Some(div().absolute().inset_0().child(scrim).child(dialog).into_any_element())
    }

    /// 类别弹层：改名（一个输入框）或删除（几句实话 + 红按钮）。
    pub(crate) fn cat_dialog_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.cat_dialog.as_ref()?;
        let (title, sub, danger, input, busy, err) = match dialog {
            CatDialog::Rename { name, input, busy, err } => (
                format!("重命名类别「{name}」"),
                "子类别跟着改（`写作/周报` → `新名/周报`），挂在它下面的模板也一起改。".to_string(),
                false,
                Some(input.clone()),
                *busy,
                err.clone(),
            ),
            CatDialog::Delete { name, count, busy, err } => (
                format!("删除类别「{name}」"),
                if *count == 0 {
                    "这个类别下面没有模板，删掉就完了。".to_string()
                } else {
                    format!("**{count} 条模板**会变成未分类（子类别一起删）。模板本身不会没，只是不再属于这个类别。")
                },
                true,
                None,
                *busy,
                err.clone(),
            ),
        };

        let scrim = div()
            .id("catdlg-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_cat_dialog(cx)));

        let mut dialog_el = div()
            .flex()
            .flex_col()
            .w(t().d(400.))
            .rounded(t().r(10.))
            .bg(rgb(t().p.panel))
            .border(t().d(1.))
            .border_color(rgb(if danger { t().p.danger } else { t().p.line }))
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(18.))
                    .text_size(t().fs(14.))
                    .text_color(rgb(if danger { t().p.danger } else { t().p.text }))
                    .child(title),
            )
            .child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(6.))
                    .pb(t().d(14.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .child(sub.replace("**", "")),
            );
        if let Some(input) = input {
            dialog_el = dialog_el.child(div().px(t().d(20.)).child(Input::new(&input)));
        }
        if let Some(e) = err {
            dialog_el = dialog_el.child(
                div()
                    .px(t().d(20.))
                    .pt(t().d(8.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.danger_fg))
                    .child(e),
            );
        }

        let boxed = div()
            .id("catdlg-dialog")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .on_click(|_: &ClickEvent, _: &mut Window, cx: &mut App| cx.stop_propagation())
            .child(
                dialog_el.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(t().d(8.))
                        .px(t().d(20.))
                        .py(t().d(16.))
                        .child(
                            div()
                                .id("catdlg-cancel")
                                .flex()
                                .items_center()
                                .h(t().d(28.))
                                .px(t().d(12.))
                                .rounded(t().r(6.))
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.text_2))
                                .cursor_pointer()
                                .hover(|h| h.bg(rgb(t().p.hover)))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_cat_dialog(cx)
                                }))
                                .child("取消"),
                        )
                        .child(
                            div()
                                .id("catdlg-ok")
                                .flex()
                                .items_center()
                                .h(t().d(28.))
                                .px(t().d(14.))
                                .rounded(t().r(6.))
                                .bg(rgb(if busy {
                                    t().p.disabled
                                } else if danger {
                                    t().p.danger
                                } else {
                                    t().p.accent
                                }))
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.on_accent))
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.submit_cat_dialog(cx)
                                }))
                                .child(if busy {
                                    "处理中…"
                                } else if danger {
                                    "删掉"
                                } else {
                                    "改名"
                                }),
                        ),
                ),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .child(scrim)
                .child(boxed)
                .into_any_element(),
        )
    }

    /// 「复制成新的」弹层。一个输入框（新名字）+ 两句说明。
    pub(crate) fn copy_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let draft = self.preset_copy.as_ref()?;
        let input = draft.name.clone();
        let busy = draft.busy;
        let err = draft.err.clone();
        let from = draft.from.clone();

        let scrim = div()
            .id("copy-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_copy(cx)));

        let dialog = div()
            .id("copy-dialog")
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
                    .w(t().d(380.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(18.))
                            .text_size(t().fs(14.))
                            .text_color(rgb(t().p.text))
                            .child(format!("从「{from}」复制一条")),
                    )
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(4.))
                            .pb(t().d(14.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child("字段、模型、工具权限、说明都照抄，只换名字。名字就是目录名。"),
                    )
                    .child(div().px(t().d(20.)).child(Input::new(&input)))
                    .children(err.map(|e| {
                        div()
                            .px(t().d(20.))
                            .pt(t().d(8.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.danger_fg))
                            .child(e)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .px(t().d(20.))
                            .py(t().d(16.))
                            .child(
                                div()
                                    .id("copy-cancel")
                                    .flex()
                                    .items_center()
                                    .h(t().d(28.))
                                    .px(t().d(12.))
                                    .rounded(t().r(6.))
                                    .text_size(t().fs(12.))
                                    .text_color(rgb(t().p.text_2))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_copy(cx)
                                    }))
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("copy-ok")
                                    .flex()
                                    .items_center()
                                    .h(t().d(28.))
                                    .px(t().d(14.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                                    .text_size(t().fs(12.))
                                    .text_color(rgb(t().p.on_accent))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.submit_copy(cx)
                                    }))
                                    .child(if busy { "复制…" } else { "复制" }),
                            ),
                    ),
            );

        Some(div().absolute().inset_0().child(scrim).child(dialog).into_any_element())
    }

    // ── 生命周期：右键菜单 / 改类别 / 归档·彻底删 ───────────────
    //
    // 菜单条目和动作映射在 `preset_menu.rs`，两屏确认在 `preset_confirm.rs`；
    // 这里只做三件事：把动作接到引擎上、把弹层摆出来、把结果说给用户。

    /// 右键菜单的出口。每一项都必须有可见结果（AGENTS：点了没反应算 bug）。
    pub(crate) fn on_preset_menu(
        &mut self,
        action: crate::preset_menu::PresetMenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::preset_menu::PresetMenuAction as A;
        match action {
            A::Open { name } => self.launch_preset(name, cx),
            A::Edit { name } => self.open_editor(name, cx),
            A::ChangeCategory { name } => {
                let picked = self
                    .sections
                    .get(self.section)
                    .and_then(|s| s.data.as_ref())
                    .and_then(|d| d.presets.iter().find(|p| p.name == name))
                    .map(|p| p.category.clone())
                    .unwrap_or_default();
                self.cat_edit = Some(CatEditDraft {
                    name,
                    picked,
                    busy: false,
                    err: None,
                });
                cx.notify();
            }
            A::Copy { name } => self.open_copy(name, window, cx),
            A::Rename { name } => self.open_preset_rename(name, window, cx),
            A::Export { name } => self.export_preset(name, cx),
            A::CopyDir { dir } => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(dir.clone()));
                self.notify(format!("目录路径已复制：{dir}"), crate::app::NoticeKind::Info, cx);
            }
            A::Reload => self.reload_section(cx),
            A::Archive { name } => {
                let sec = SECTIONS[self.section].0.to_string();
                self.preset_confirm
                    .request(nous_engine::types::PresetOp::Archive, sec, name);
                cx.notify();
            }
            A::Purge { name } => {
                let sec = SECTIONS[self.section].0.to_string();
                self.preset_confirm
                    .request(nous_engine::types::PresetOp::Purge, sec, name);
                cx.notify();
            }
        }
    }

    /// 重扫这一段（盘上被外部改过时用）。回包不信，直接重拉。
    fn reload_section(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let sec_idx = self.section;
        let section_id = SECTIONS[sec_idx].0;
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move { engine.reload_presets(section_id).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(_)) => {
                        this.load_section(sec_idx, true, cx);
                        this.notify("已重扫这一段", crate::app::NoticeKind::Info, cx);
                    }
                    Some(Err(e)) => this.notify(format!("重扫失败：{e}"), crate::app::NoticeKind::Error, cx),
                    None => this.notify("重扫没跑完", crate::app::NoticeKind::Error, cx),
                }
            });
        })
        .detach();
    }

    /// 第二屏那个真按钮：把 `(op, section, name)` 交给引擎，成功就重拉这一段。
    pub(crate) fn run_preset_op(
        &mut self,
        op: nous_engine::types::PresetOp,
        name: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(what) = self.preset_busy.clone() {
            self.notify(format!("{what} 还没做完"), crate::app::NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", crate::app::NoticeKind::Error, cx);
            return;
        };
        let sec_idx = self.section;
        let section_id = SECTIONS[sec_idx].0.to_string();
        let name2 = name.clone();
        let verb = match op {
            nous_engine::types::PresetOp::Archive => "归档",
            nous_engine::types::PresetOp::Restore => "还原",
            nous_engine::types::PresetOp::Purge => "彻底删掉",
        };
        self.preset_busy = Some(format!("正在{verb} {name}"));
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move { engine.preset_op(&section_id, op, &name2).await })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.preset_busy = None;
                match result {
                    Some(Ok(_)) => {
                        // 归档 / 彻底删之后那一条必须从列表里消失：选中和节点说明一起收。
                        if this.picked_preset.as_deref() == Some(name.as_str()) {
                            this.picked_preset = None;
                            this.picked_node = None;
                        }
                        this.load_section(sec_idx, true, cx);
                        let what = match op {
                            nous_engine::types::PresetOp::Archive => "归档",
                            nous_engine::types::PresetOp::Restore => "还原",
                            nous_engine::types::PresetOp::Purge => "彻底删掉",
                        };
                        this.notify(format!("已{what} {name}"), crate::app::NoticeKind::Info, cx);
                    }
                    Some(Err(e)) => this.notify(format!("没做成：{e}"), crate::app::NoticeKind::Error, cx),
                    None => this.notify("任务没跑完", crate::app::NoticeKind::Error, cx),
                }
            });
        })
        .detach();
    }

    /// 改类别弹层的勾选切换。
    pub(crate) fn toggle_cat_edit(&mut self, cat: String, cx: &mut Context<Self>) {
        if let Some(d) = self.cat_edit.as_mut() {
            if let Some(ix) = d.picked.iter().position(|c| *c == cat) {
                d.picked.remove(ix);
            } else {
                d.picked.push(cat);
            }
            d.err = None;
            cx.notify();
        }
    }

    /// 改类别：整份发过去（不是增量）。
    pub(crate) fn submit_cat_edit(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.cat_edit.as_ref() else {
            return;
        };
        if draft.busy {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            if let Some(d) = self.cat_edit.as_mut() {
                d.err = Some("还没连上引擎".into());
            }
            cx.notify();
            return;
        };
        let name = draft.name.clone();
        let picked = draft.picked.clone();
        let sec_idx = self.section;
        let section_id = SECTIONS[sec_idx].0.to_string();
        if let Some(d) = self.cat_edit.as_mut() {
            d.busy = true;
            d.err = None;
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let result = match &handle {
                Some(h) => h
                    .spawn(async move {
                        engine.set_preset_category(&section_id, &name, &picked).await
                    })
                    .await
                    .ok(),
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                match result {
                    Some(Ok(_)) => {
                        this.cat_edit = None;
                        this.load_section(sec_idx, true, cx);
                    }
                    Some(Err(e)) => {
                        if let Some(d) = this.cat_edit.as_mut() {
                            d.busy = false;
                            d.err = Some(e.to_string());
                        }
                    }
                    None => {
                        if let Some(d) = this.cat_edit.as_mut() {
                            d.busy = false;
                            d.err = Some("任务没跑完".into());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 改类别弹层：这一段的词表逐行勾选。
    pub(crate) fn cat_edit_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let draft = self.cat_edit.as_ref()?;
        let cats: Vec<String> = self
            .sections
            .get(self.section)
            .and_then(|s| s.data.as_ref())
            .map(|d| d.categories.clone())
            .unwrap_or_default();
        let name = draft.name.clone();
        let picked = draft.picked.clone();
        let busy = draft.busy;
        let err = draft.err.clone();

        let scrim = div()
            .id("catedit-scrim")
            .absolute()
            .inset_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.cat_edit = None;
                cx.notify();
            }));

        let mut rows = div().flex().flex_col().gap(t().d(2.));
        for cat in &cats {
            let on = picked.iter().any(|c| c == cat);
            let c2 = cat.clone();
            rows = rows.child(
                div()
                    .id(ElementId::Name(format!("catedit-{cat}").into()))
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(8.))
                    .py(t().d(5.))
                    .rounded(t().r(4.))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.toggle_cat_edit(c2.clone(), cx);
                    }))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(14.))
                            .rounded(t().r(3.))
                            .border(t().d(1.))
                            .border_color(rgb(if on { t().p.accent } else { t().p.line }))
                            .bg(rgb(if on { t().p.accent } else { t().p.panel }))
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.on_accent))
                            .children(on.then(|| "✓".to_string())),
                    )
                    .child(div().text_size(t().fs(12.)).child(cat.clone())),
            );
        }
        if cats.is_empty() {
            rows = rows.child(
                div()
                    .px(t().d(8.))
                    .py(t().d(6.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.faint))
                    .child("这一段还没有类别 —— 先在左栏建一个"),
            );
        }

        let dialog = div()
            .id("catedit-dialog")
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
                    .w(t().d(360.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(18.))
                            .text_size(t().fs(14.))
                            .child(format!("改类别：{name}")),
                    )
                    .child(
                        div()
                            .px(t().d(20.))
                            .pt(t().d(4.))
                            .pb(t().d(12.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child("勾几个都行，一个不勾就是未分类。"),
                    )
                    .child(div().px(t().d(14.)).child(rows))
                    .children(err.map(|e| {
                        div()
                            .px(t().d(20.))
                            .pt(t().d(8.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.danger_fg))
                            .child(e)
                    }))
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
                                    .id("catedit-cancel")
                                    .px(t().d(14.))
                                    .py(t().d(6.))
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .text_size(t().fs(12.))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.cat_edit = None;
                                        cx.notify();
                                    }))
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("catedit-save")
                                    .px(t().d(16.))
                                    .py(t().d(6.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                                    .text_color(rgb(if busy {
                                        t().p.faint
                                    } else {
                                        t().p.on_accent
                                    }))
                                    .text_size(t().fs(12.))
                                    .when(!busy, |el| {
                                        el.cursor_pointer()
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.submit_cat_edit(cx);
                                            }))
                                    })
                                    .child(if busy { "保存中…" } else { "保存" }),
                            ),
                    ),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .child(scrim)
                .child(dialog)
                .into_any_element(),
        )
    }

    /// 归档 / 彻底删的两屏确认。
    pub(crate) fn preset_confirm_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.preset_confirm.is_open() {
            return None;
        }
        // 三个回调要求 `Clone`（同一个闭包要给罩子 / 按钮各一份），
        // `cx.listener` 出来的不实现 —— 和 `purge_overlay` 一样套一层 `Rc`。
        let close = std::rc::Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.preset_confirm.on_esc();
            cx.notify();
        }));
        let ack = std::rc::Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.preset_confirm.toggle_ack();
            cx.notify();
        }));
        let fwd = std::rc::Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.preset_confirm.advance();
            cx.notify();
        }));
        let back = std::rc::Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.preset_confirm.back();
            cx.notify();
        }));
        let close2 = close.clone();
        let ack2 = ack.clone();
        let fwd2 = fwd.clone();
        let back2 = back.clone();
        Some(crate::preset_confirm::render_preset_confirm(
            &self.preset_confirm,
            move |ev, window, cx| close2(ev, window, cx),
            move |ev, window, cx| ack2(ev, window, cx),
            move |ev, window, cx| fwd2(ev, window, cx),
            move |ev, window, cx| back2(ev, window, cx),
            cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some((op, _section, name)) = this.preset_confirm.take() {
                    this.run_preset_op(op, name, cx);
                }
                cx.notify();
            }),
        ))
    }

    fn pick_preset(&mut self, preset_name: String, cx: &mut Context<Self>) {
        if self.picked_preset.as_deref() == Some(preset_name.as_str()) {
            // 再点一次 = 收起右栏，来回切不会卡在一条上。
            self.picked_preset = None;
        } else {
            self.picked_preset = Some(preset_name);
        }
        self.picked_node = None;
        cx.notify();
    }

    fn launch_preset(&mut self, preset_name: String, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        // 防连点：开一条的时候别的行先不动。
        if self.creating.is_some() {
            return;
        }
        let section_id = self.sections[self.section].id;
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
            preset: Some(format!("{section_id}/{preset_name}")),
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
                        this.sections[this.section].error = None;
                        this.tab = Tab::Chats;
                        // 从模板建完也回聊天：盘面不该留在右格里。
                        this.newchat.open = false;
                        this.newchat.sheet = None;
                        this.select_session(created.id, true, cx);
                    }
                    Some(Err(e)) => {
                        this.sections[this.section].error = Some(format!("开不了：{e}"))
                    }
                    None => this.sections[this.section].error = Some("任务没跑完".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── 渲染：类别树 ────────────────────────────────────────

    /// Template 页的左栏：段切换 + 类别树 + 段脚注。
    /// 段按钮下面那行：**新建**（实心）+ **类别**（描边）。
    ///
    /// 这两个不是一类东西：新建建一条预设（落盘成一个目录），类别改这一段的词表
    /// （`categories.json`）—— 所以别合并成「＋」下拉，两条路的分叉点就在这儿。
    fn preset_actions(&self, cx: &mut Context<Self>) -> AnyElement {

        div()
            .flex()
            .flex_row()
            .gap(t().d(6.))
            .px(t().d(14.))
            .pb(t().d(10.))
            .child(
                div()
                    .id("new-preset")
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(t().d(4.))
                    .h(t().d(24.))
                    .px(t().d(9.))
                    .rounded(t().r(6.))
                    .bg(rgb(t().p.accent))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.on_accent))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.accent_hover)))
                    .on_click(cx.listener(|this, _, window, cx| this.open_new_preset(window, cx)))
                    .child(
                        Icon::new(IconName::Plus)
                            .size(t().d(12.))
                            .text_color(rgb(t().p.on_accent)),
                    )
                    .child("新建"),
            )
            .child(
                div()
                    .id("new-category")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(t().d(24.))
                    .px(t().d(9.))
                    .rounded(t().r(6.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.text_2))
                    .cursor_pointer()
                    .hover(|h| {
                        h.border_color(rgb(t().p.text_2))
                            .text_color(rgb(t().p.text))
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.open_new_category(window, cx)))
                    .child("＋ 类别"),
            )
            .child(
                div()
                    .id("import-preset")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(t().d(24.))
                    .px(t().d(9.))
                    .rounded(t().r(6.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.text_2))
                    .cursor_pointer()
                    .hover(|h| {
                        h.border_color(rgb(t().p.text_2))
                            .text_color(rgb(t().p.text))
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.open_import(window, cx)))
                    .child("导入…"),
            )
            .into_any_element()
    }

    pub(crate) fn render_preset_tree(&self, cx: &mut Context<Self>) -> AnyElement {
        let sec = &self.sections[self.section];
        let tree = sec
            .data
            .as_ref()
            .map(|d| facet_tree(&d.categories, &d.presets))
            .unwrap_or_default();
        let uncat = sec
            .data
            .as_ref()
            .map(|d| d.presets.iter().filter(|p| p.category.is_empty()).count())
            .unwrap_or(0);
        let total = sec.data.as_ref().map_or(0, |d| d.presets.len());

        // 段按钮：两块并排，选中的白底。字给了两行 —— 名字 + 条数（只要数字，别写「真实」「演示」）。
        let segments = div()
            .flex()
            .flex_row()
            .gap(t().d(2.))
            // 段名是完整的 API 段名（`Ultimate-Template` 17 个字符），左右边距收窄一点
            // 给按钮多让 4px；真要挤也只会缩字距，不会折行（下面 `whitespace_nowrap`）。
            .mx(t().d(8.))
            .mb(t().d(10.))
            .p(t().d(3.))
            .rounded(t().r(9.))
            .bg(rgb(t().p.line))
            .children((0..SECTIONS.len()).map(|i| {
                let active = i == self.section;
                let count = self.sections[i]
                    .data
                    .as_ref()
                    .map_or("—".to_string(), |d| format!("{} 条", d.presets.len()));
                div()
                    .id(ElementId::Name(format!("section-{i}").into()))
                    .flex()
                    .flex_col()
                    .items_center()
                    // 两块**等宽**：`flex_1` 按内容长，`Ultimate-Template` 比
                    // `Session-Template` 宽，药丸就会一长一短。宽度写 0 + grow 才等分。
                    .w(px(0.))
                    .min_w(px(0.))
                    .flex_grow(1.)
                    .py(t().d(4.))
                    .rounded(t().r(7.))
                    .when(active, |el| el.bg(rgb(t().p.panel)))
                    .when(!active, |el| el.hover(|h| h.bg(rgb(t().p.line_soft))))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.switch_section(i, cx);
                    }))
                    .child(
                        div()
                            // 11px + 不折行：17 个字符在加宽后的按钮里正好单行。
                            .text_size(t().fs(11.))
                            .whitespace_nowrap()
                            .text_color(rgb(if active { t().p.text } else { t().p.muted }))
                            .child(SECTIONS[i].1),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(if active { t().p.ok_fg } else { t().p.faint }))
                            .child(count),
                    )
            }));

        // 全部 + 类别 + 未分类
        let mut rows: Vec<AnyElement> = Vec::new();
        rows.push(self.cat_row(
            "▦",
            "全部",
            total,
            0,
            self.preset_cat.is_none(),
            None,
            Pick::All,
            cx,
        ));
        for node in &tree {
            // 父级折了 → 整枝不画
            let hidden = ancestors(&node.path)
                .iter()
                .take(node.depth)
                .any(|a| self.preset_closed.contains(a));
            if hidden {
                continue;
            }
            let mine = self.preset_cat.as_deref() == Some(node.path.as_str());
            let caret = node
                .has_children
                .then(|| (node.path.clone(), self.preset_closed.contains(&node.path)));
            rows.push(self.cat_row(
                "",
                &node.label,
                node.count,
                node.depth,
                mine,
                caret,
                Pick::Cat(node.path.clone()),
                cx,
            ));
        }
        rows.push(self.cat_row(
            "·",
            "未分类",
            uncat,
            0,
            self.preset_cat.as_deref() == Some(UNCATEGORIZED),
            None,
            Pick::Uncat,
            cx,
        ));

        let repo_line = match (&sec.data, &sec.error) {
            (Some(d), _) => match &d.repo {
                Some(r) => {
                    let head: String = r.head.as_deref().unwrap_or("—").chars().take(7).collect();
                    if r.dirty {
                        (format!("● 有未提交的改动 · {head}"), warn())
                    } else {
                        (format!("● 仓干净 · {head}"), t().p.ok_fg)
                    }
                }
                None => ("● 仓状态未知".to_string(), muted()),
            },
            (None, Some(_)) => ("● 拉不到这一段".to_string(), bad()),
            (None, _) => (format!("● 正在拉 {}", sec.id), muted()),
        };

        div()
            .flex()
            .flex_col()
            .w(px(t().preset_tree_w))
            .h_full()
            .flex_none()
            .bg(rgb(tree_bg()))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .px(t().d(14.))
                    .pt(t().d(14.))
                    .pb(t().d(8.))
                    .child(div().text_size(t().fs(13.)).child("Template"))
                    .child(
                        div()
                            .ml_auto()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.faint))
                            .child(format!("{} 类", tree.len())),
                    ),
            )
            .child(segments)
            .child(self.preset_actions(cx))
            .child(
                div()
                    .id("preset-tree-area")
                    .relative()
                    .flex_1()
                    .min_h(t().d(0.))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.bar_hover(Bar::Tree, *hovered, cx)
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .id(ElementId::Name("preset-tree-scroll".into()))
                            .overflow_y_scroll()
                            .track_scroll(&self.tree_scroll)
                            .px(t().d(8.))
                            .pb(t().d(8.))
                            .children(rows),
                    )
                    .child(crate::app::scrollbar::overlay(
                        "preset-tree-scrollbar",
                        &self.tree_scroll,
                        self.bar(crate::app::Bar::Tree).0,
                        self.bar(crate::app::Bar::Tree).1,
                    )),
            )
            .child({
                let n = self.sections[self.section]
                    .data
                    .as_ref()
                    .map(|d| d.archived.len())
                    .unwrap_or(0);
                let on = self.archive_view;
                div()
                    .id("preset-archive-entry")
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
                    .when(on, |el| el.bg(rgb(t().p.hover)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.archive_view = !this.archive_view;
                        cx.notify();
                    }))
                    .child(
                        Icon::new(IconName::Archive)
                            .size(t().d(13.))
                            .text_color(rgb(t().p.faint)),
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
                            .text_color(rgb(t().p.faint))
                            .child(n.to_string()),
                    )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(2.))
                    .px(t().d(12.))
                    .py(t().d(8.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(repo_line.1))
                            .child(repo_line.0),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.faint))
                            // 段名已经在按钮和右栏标题上了，脚注只留 API 段名 ——
                            // 以前写「会话模板」是翻译，现在标题就是段名，会撞成两个一样的词。
                            .child(sec.id.to_string()),
                    ),
            )
            .into_any_element()
    }

    /// 类别树的一行。**两段各自一个 hitbox**：箭头管折叠，名字管选类别 ——
    /// 不做嵌套点击（谁吃事件说不清，见 `坑.md` 那条「两套 Div」的同源问题）。
    #[allow(clippy::too_many_arguments)]
    fn cat_row(
        &self,
        glyph: &str,
        label: &str,
        count: usize,
        depth: usize,
        is_current: bool,
        caret: Option<(String, bool)>,
        pick: Pick,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let caret_cell: AnyElement = match caret {
            Some((path, closed)) => div()
                .id(ElementId::Name(format!("caret-{path}").into()))
                .flex()
                .items_center()
                .justify_center()
                .w(t().d(14.))
                .h(t().d(20.))
                .rounded(t().r(4.))
                .hover(|h| h.bg(rgb(t().p.hover)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.toggle_closed(path.clone(), cx)
                }))
                .child(
                    Icon::new(if closed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .with_size(Size::Size(t().d(12.)))
                    .text_color(rgb(t().p.faint)),
                )
                .into_any_element(),
            None => div().w(t().d(14.)).into_any_element(),
        };

        let name = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(6.))
            .flex_1()
            .min_w(t().d(0.))
            .when(!glyph.is_empty(), |el| {
                el.child(
                    div()
                        .w(t().d(14.))
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.muted))
                        .child(glyph.to_string()),
                )
            })
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .text_size(t().fs(13.))
                    .text_color(rgb(if count == 0 {
                        t().p.faint
                    } else if is_current {
                        selected_fg()
                    } else {
                        t().p.text_2
                    }))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(t().fs(11.))
                    .text_color(rgb(if count == 0 { t().p.faint } else { t().p.muted }))
                    .child(count.to_string()),
            );

        // **点击挂在整行上**（跟会话列表那些行同一个写法）：子元素上的 `on_click` 在这套布局里
        // 收不到事件（踩过，见 `坑.md` 37）。箭头是行的子元素，它自己那一小块也接点击 ——
        // 收不到就算了，主交互（选类别）不受影响。
        // 菜单用的路径要先摘出来：下面 `on_click` 那个闭包会 `move` 掉整个 `pick`。
        let menu_path = match &pick {
            Pick::Cat(path) => Some(path.clone()),
            _ => None,
        };

        let row = div()
            .id(ElementId::Name(format!("cat-{}-{depth}", label).into()))
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(6.))
            .pl(px(8. + depth as f32 * 12.))
            .pr(t().d(8.))
            .h(t().d(28.))
            .rounded(t().r(6.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| match &pick {
                Pick::All => this.pick_category(None, cx),
                Pick::Uncat => this.pick_category(Some(UNCATEGORIZED.into()), cx),
                Pick::Cat(path) => this.pick_category(Some(path.clone()), cx),
            }))
            .when(is_current, |el| el.bg(rgb(selected_bg())))
            .when(!is_current, |el| el.hover(|h| h.bg(rgb(t().p.hover))))
            .child(caret_cell)
            .child(name);

        // 右键：改名 / 删除。只有 `Pick::Cat` 才挂 —— 「全部」「未分类」不在词表里，
        // 对它们做这两件事没有意义（也就不该出现一个点了没反应的菜单）。
        match menu_path {
            Some(path) => {
                let this = cx.entity().downgrade();
                row.context_menu(move |menu, _window, _cx| {
                    let ren = this.clone();
                    let del = this.clone();
                    let p1 = path.clone();
                    let p2 = path.clone();
                    menu.item(
                        PopupMenuItem::new("改名…").on_click(
                            move |_: &ClickEvent, window, cx: &mut App| {
                                let p = p1.clone();
                                let _ = ren
                                    .update(cx, |this, cx| this.open_cat_rename(p, window, cx));
                            },
                        ),
                    )
                    .item(PopupMenuItem::new("删除…").on_click(
                        move |_: &ClickEvent, _window, cx: &mut App| {
                            let p = p2.clone();
                            let _ = del.update(cx, |this, cx| this.open_cat_delete(p, count, cx));
                        },
                    ))
                })
                .into_any_element()
            }
            None => row.into_any_element(),
        }
    }

    // ── 渲染：预设列表 ──────────────────────────────────────

    /// Template 页的右栏：类别标题 + 条目列表。
    /// 中栏：这一段的模板卡片列表。
    ///
    /// `narrow` = 窄窗（类别树已经收进抽屉）。这时列表自己也收窄一点，
    /// 头顶补一个「类别」按钮把抽屉叫出来 —— 否则这一页就没有换类别的入口了。
    pub(crate) fn render_templates(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let sec = &self.sections[self.section];
        let uncat = sec
            .data
            .as_ref()
            .map(|d| d.presets.iter().filter(|p| p.category.is_empty()).count())
            .unwrap_or(0);

        // 过滤词：搜索是**全段**搜，忽略当前类别 —— 找不到东西的人不该还得先切对类别。
        let query = self.preset_filter.read(cx).value().trim().to_string();
        let searching = !query.is_empty() && !self.archive_view;

        let (title, count) = if searching {
            (
                "搜索".to_string(),
                sec.data
                    .as_ref()
                    .map_or(0, |d| d.presets.iter().filter(|p| matches_query(p, &query)).count()),
            )
        } else if self.archive_view {
            // 归档视图的标题：别顶着「全部 5 条」在那儿列 1 条归档，人会对不上。
            (
                "已归档".to_string(),
                sec.data.as_ref().map_or(0, |d| d.archived.len()),
            )
        } else {
            match &self.preset_cat {
            None => (
                "全部".to_string(),
                sec.data.as_ref().map_or(0, |d| d.presets.len()),
            ),
            Some(c) if c == UNCATEGORIZED => ("未分类".to_string(), uncat),
            Some(c) => (
                c.clone(),
                sec.data.as_ref().map_or(0, |d| {
                    d.presets.iter().filter(|p| in_category(p, c)).count()
                }),
            ),
            }
        };

        let body: Vec<AnyElement> = if self.archive_view {
            // 归档区：搬进 `.archive/` 的那些。归档不删文件，所以这儿能还原。
            match &sec.data {
                Some(d) if !d.archived.is_empty() => d
                    .archived
                    .iter()
                    .map(|a| self.preset_archived_row(a, cx))
                    .collect(),
                Some(_) => vec![note("归档区是空的 —— 归档过的预设会落在这儿，能还原或彻底删。")],
                None => vec![note("还没数据")],
            }
        } else if sec.loading && sec.data.is_none() {
            vec![note("拉模板中…")]
        } else if let Some(e) = &sec.error {
            vec![
                note(&format!("拉模板失败：{e}")),
                // 失败给个重试入口。
                div()
                    .id(ElementId::Name("presets-retry".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.load_presets(true, cx);
                    }))
                    .px(t().d(12.))
                    .py(t().d(4.))
                    .text_size(t().fs(12.))
                    .text_color(rgb(t().p.link))
                    .child("重试")
                    .into_any_element(),
            ]
        } else {
            match &sec.data {
                None => vec![note("还没数据")],
                Some(d) => {
                    let rows: Vec<&Preset> = if searching {
                        d.presets
                            .iter()
                            .filter(|p| matches_query(p, &query))
                            .collect()
                    } else {
                        match &self.preset_cat {
                            None => d.presets.iter().collect(),
                            Some(c) if c == UNCATEGORIZED => {
                                d.presets.iter().filter(|p| p.category.is_empty()).collect()
                            }
                            Some(c) => d.presets.iter().filter(|p| in_category(p, c)).collect(),
                        }
                    };
                    if rows.is_empty() {
                        if searching {
                            vec![empty_hint(&format!("「{query}」"), sec.title, 0)]
                        } else {
                            vec![empty_hint(&title, sec.title, d.categories.len())]
                        }
                    } else {
                        rows.into_iter()
                            .map(|p| self.preset_row(p, cx).into_any_element())
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
            // 窄窗再收 60px 给右栏：240 还放得下两行摘要，右栏能多出小半屏。
            .w(px(if narrow {
                crate::app::TEMPLATE_LIST_W_NARROW
            } else {
                t().template_list_w
            }))
            .h_full()
            .flex_none()
            .bg(rgb(t().p.list_bg))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(6.))
                    .px(t().d(14.))
                    .pt(t().d(14.))
                    .pb(t().d(4.))
                    .children(narrow.then(|| {
                        div()
                            .id("cat-drawer-btn")
                            .flex()
                            .items_center()
                            .gap(t().d(3.))
                            .h(t().d(20.))
                            .px(t().d(6.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.text_2))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.cat_drawer = true;
                                cx.notify();
                            }))
                            .child(Icon::new(IconName::Menu).size(t().d(12.)))
                            .child("类别")
                    }))
                    .child(div().text_size(t().fs(15.)).child(title.clone()))
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(muted()))
                            .child(format!("{count} 条")),
                    )
                    // 窄窗时类别树收进抽屉，「新建」也跟着没了 —— 在这儿补回来。
                    .children(narrow.then(|| {
                        div()
                            .id("new-preset-narrow")
                            .ml_auto()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(t().d(3.))
                            .h(t().d(22.))
                            .px(t().d(8.))
                            .rounded(t().r(6.))
                            .bg(rgb(t().p.accent))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.on_accent))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_new_preset(window, cx)
                            }))
                            .child(
                                Icon::new(IconName::Plus)
                                    .size(t().d(11.))
                                    .text_color(rgb(t().p.on_accent)),
                            )
                            .child("新建")
                    })),
            )
            // 搜索框常驻：模板到几十条之后，靠类别树翻不如直接打两个字。
            // 归档视图里不出现 —— 那是个小口袋，塞个搜索框反而让人以为能搜归档。
            .children((!self.archive_view).then(|| {
                div()
                    .mx(t().d(14.))
                    .mb(t().d(6.))
                    .child(Input::new(&self.preset_filter))
            }))
            // 生命周期动作跑着的时候常驻一行：引擎那边要 commit + push，
            // 实测几分钟 —— 不给这一行，用户会以为点了没反应又点第二次。
            .children(self.preset_busy.clone().map(|what| {
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(7.))
                    .mx(t().d(14.))
                    .mb(t().d(9.))
                    .px(t().d(9.))
                    .py(t().d(6.))
                    .rounded(t().r(4.))
                    .bg(rgb(t().p.warn_soft))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.warn))
                    .child(what)
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child("引擎在提交，可能要几分钟"),
                    )
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(t().d(6.))
                    .items_center()
                    .px(t().d(14.))
                    .pb(t().d(10.))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(sec.title),
                    )
                    .when(self.preset_cat.is_some(), |el| {
                        el.child(
                            div()
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.faint))
                                .child("·"),
                        )
                        .child(
                            div()
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.muted))
                                .child("含子级"),
                        )
                    }),
            )
            .child(
                div()
                    .id("presets-scroll-area")
                    .relative()
                    .flex_1()
                    // 和左栏/聊天区同一条规矩：flex 项不写 `min_h(0)` 就会被内容撑爆，
                    // 撑爆了就不需要滚，滚动条也就永远不出现。
                    .min_h(t().d(0.))
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
                        self.bar(crate::app::Bar::Presets).0,
                        self.bar(crate::app::Bar::Presets).1,
                    )),
            )
            .into_any_element()
    }

    /// 归档区的一行：名字 + 摘要，右边「还原」「彻底删」。
    ///
    /// 归档只是把目录搬进 `.archive/`，**文件一个都没删**；彻底删才是真的没了，
    /// 所以那个按钮走两屏确认（和列表里的「彻底删掉…」同一个面板）。
    fn preset_archived_row(
        &self,
        a: &nous_engine::types::ArchivedPreset,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = a.name.clone();
        let busy = self.preset_busy.is_some();
        let restore_name = name.clone();
        let purge_name = name.clone();
        let section = SECTIONS[self.section].0.to_string();
        div()
            .id(ElementId::Name(format!("arch-{}", a.name).into()))
            .flex()
            .flex_col()
            .gap(t().d(3.))
            .px(t().d(14.))
            .py(t().d(8.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .hover(|h| h.bg(rgb(t().p.hover)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .text_size(t().fs(12.5))
                            .font_family("JetBrainsMono")
                            .text_color(rgb(t().p.text_2))
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(format!("arch-restore-{}", name).into()))
                            .flex_none()
                            .px(t().d(8.))
                            .py(t().d(2.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(if busy { t().p.faint } else { t().p.text_2 }))
                            .cursor_pointer()
                            .hover(|h| h.border_color(rgb(t().p.muted)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if this.preset_busy.is_some() {
                                    return;
                                }
                                this.archive_view = false;
                                this.run_preset_op(
                                    nous_engine::types::PresetOp::Restore,
                                    restore_name.clone(),
                                    cx,
                                );
                            }))
                            .child("还原"),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(format!("arch-purge-{}", name).into()))
                            .flex_none()
                            .px(t().d(8.))
                            .py(t().d(2.))
                            .rounded(t().r(5.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(if busy { t().p.faint } else { t().p.danger }))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.danger_soft)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if this.preset_busy.is_some() {
                                    return;
                                }
                                // 彻底删不可逆：走两屏确认，别让人手一滑就没。
                                this.preset_confirm.request(
                                    nous_engine::types::PresetOp::Purge,
                                    section.clone(),
                                    purge_name.clone(),
                                );
                                cx.notify();
                            }))
                            .child("彻底删"),
                    ),
            )
            .children((!a.summary.clone().unwrap_or_default().is_empty()).then(|| {
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.faint))
                    .truncate()
                    .child(a.summary.clone().unwrap_or_default())
            }))
            .into_any_element()
    }

    fn preset_row(&self, p: &Preset, cx: &mut Context<Self>) -> AnyElement {
        // `Ultimate-Template` 这一段只读：启用触发器的动作还没定，点它什么也不该发生。
        let can_open = self.sections[self.section].id == SECTIONS[0].0;
        let ok = can_open && launchable(p);
        let name = p.name.clone();
        let pick_name = p.name.clone();
        let creating = self.creating.as_deref() == Some(p.name.as_str());
        let picked = self.picked_preset.as_deref() == Some(p.name.as_str());
        let cat = self.preset_cat.clone().unwrap_or_default();

        // 状态：判据全是引擎算好的，客户端只挑颜色和形状（描边 pill，见 demo 组 ③ 方案 2）。
        let status: Option<(&str, u32)> = if p.spec_error.is_some() {
            Some(("spec 写坏了", bad()))
        } else if has_invalid(p) {
            Some(("引用失效，现在开不了", bad()))
        } else if !p.tracked {
            Some(("盘上有、仓里没", warn()))
        } else {
            None
        };

        let row = div()
            .relative()
            .flex()
            .gap(t().d(10.))
            .p(t().d(10.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_strong))
            // 行永远是 Stateful，且**点行 = 选中**（看右栏那张图），不是直接开会话。
            // 开会话挪到行尾那个圆按钮上 —— 微信里点联系人是看资料，不是发消息。
            .id(ElementId::Name(format!("preset-{}", p.name).into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.pick_preset(pick_name.clone(), cx);
            }))
            .hover(|h| h.bg(rgb(t().p.line_soft)))
            .when(creating, |el| el.bg(rgb(t().p.accent_soft)))
            .when(picked, |el| el.bg(rgb(t().p.accent_soft)))
            // 选中的那一行左侧一条竖线：颜色之外还得有个不用辨认形状的信号。
            .when(picked, |el| {
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(t().d(7.))
                        .bottom(t().d(7.))
                        .w(t().d(2.))
                        .rounded(t().r(1.))
                        .bg(rgb(t().p.accent))
                        // 120ms：竖条是「选中了」的信号，短到不需要等它长完。
                        .with_animation(
                            ElementId::Name(format!("pick-bar-{}", p.name).into()),
                            Animation::new(std::time::Duration::from_millis(120))
                                .with_easing(ease_out_quint()),
                            |el, delta| el.opacity(delta),
                        ),
                )
            })
            .child(
                // 头像方块：能开的用蓝底白字首字，开不了的置灰。
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(36.))
                    .rounded(t().r(6.))
                    .bg(rgb(if ok { avatar_bg() } else { t().p.disabled }))
                    .text_color(rgb(if ok { t().p.on_accent } else { t().p.faint }))
                    .text_size(t().fs(16.))
                    .child(p.name.chars().next().unwrap_or('?').to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(3.))
                    .flex_1()
                    .min_w(t().d(0.))
                    .overflow_hidden()
                    .child(
                        // 名字一行、摘要一行、chip 一行 —— 列表里不折行（折了行高就跳，扫不动）。
                        div()
                            .whitespace_nowrap()
                            .text_overflow(TextOverflow::Truncate("…".into()))
                            .child(p.name.clone()),
                    )
                    .when(!p.summary.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.muted))
                                .whitespace_nowrap()
                                .text_overflow(TextOverflow::Truncate("…".into()))
                                .child(p.summary.clone()),
                        )
                    })
                    .child(self.chips(p, &cat))
                    .children(status.map(|(text, color)| status_pill(text, color))),
            )
            // 行尾：能开的行给一个「开一条新对话」的圆按钮。
            // 行体只负责选中 —— 两个动作分开，多一次辨认，少一堆误触。
            .when(ok, |el| {
                el.child(
                    div()
                        .id(ElementId::Name(format!("open-{}", p.name).into()))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(t().d(26.))
                        .rounded(t().r(6.))
                        .text_color(rgb(muted()))
                        .hover(|h| h.bg(rgb(t().p.line)).text_color(rgb(t().p.link)))
                        .child(
                            Icon::new(IconName::MessageCirclePlus)
                                .with_size(Size::Size(t().d(15.))),
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.launch_preset(name.clone(), cx);
                        })),
                )
            });

        // 右键菜单：条目顺序 / 文案 / 动作映射在 `preset_menu.rs`。
        // 只能在链尾挂：`context_menu()` 返回 `ContextMenu<Stateful<Div>>`，换个类型就串不下去。
        let this = cx.entity().downgrade();
        let target = crate::preset_menu::PresetTarget::new(
            SECTIONS[self.section].0,
            p.name.clone(),
            p.dir.clone(),
            p.tracked,
        );
        row.context_menu(move |menu, window, cx| {
            let this = this.clone();
            crate::preset_menu::build_menu(menu, window, cx, &target, ok, move |action, window, cx| {
                let _ = this.update(cx, |this, cx| this.on_preset_menu(action, window, cx));
            })
        })
        .into_any_element()
    }

    /// 一行小 chip：当前类别 / 也属于 / 只读 / `Ultimate-Template` 的触发器与引用。
    fn chips(&self, p: &Preset, current: &str) -> AnyElement {
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(4.))
            .overflow_hidden();
        let mut any = false;

        if !current.is_empty() && p.category.iter().any(|c| c == current) {
            row = row.child(chip(current, selected_bg(), selected_fg()));
            any = true;
        } else if current.is_empty() {
            // 「全部」下不写「也属于」（那时没有当前类别可对照），只把类别平铺出来。
            for c in &p.category {
                row = row.child(chip(c, t().p.line_soft, t().p.text_2));
                any = true;
            }
        }
        for c in p
            .category
            .iter()
            .filter(|c| !current.is_empty() && c.as_str() != current)
        {
            row = row.child(chip(&format!("也属于 {c}"), t().p.warn_soft, t().p.faint));
            any = true;
        }
        if read_only(p) {
            row = row.child(chip("只读", t().p.hover, t().p.muted));
            any = true;
        }
        if self.sections[self.section].id == SECTIONS[1].0 {
            if let Some(trigger) = spec_str(p, "trigger") {
                row = row.child(chip(&format!("⚡ {trigger}"), t().p.warn_soft, t().p.warn));
                any = true;
            }
            if let Some(session) = spec_str(p, "session") {
                row = row.child(chip(&format!("→ {session}"), t().p.accent_soft, t().p.ok_fg));
                any = true;
            }
        }
        if !any {
            return div().into_any_element();
        }
        row.into_any_element()
    }
}

// ── 纯函数（能单测的都在这儿） ─────────────────────────────

/// `写作/周报` → `["写作", "写作/周报"]`（含自己）。
pub(crate) fn ancestors(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    (1..=parts.len()).map(|n| parts[..n].join("/")).collect()
}

/// `a` 是不是 `root` 或它的后代。
pub(crate) fn is_under(value: &str, root: &str) -> bool {
    value == root || value.starts_with(&format!("{root}/"))
}

fn depth_of(path: &str) -> usize {
    path.split('/')
        .filter(|s| !s.is_empty())
        .count()
        .saturating_sub(1)
}

fn last_seg(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// 扁平类别表 → 缩进树（带计数）。
///
/// **顺序跟着引擎走**：`categories` 已经是「父在子前、逐段 zh-CN」排好的，这里只补缺（预设上挂着、
/// 词表里没有的路径）。补进来的按「浅的在前、同层按字面」排 —— 只为稳定，不假装那是 zh-CN 排序。
///
/// 计数含后代、**同一条预设按路径去重**：一条预设同时挂 `工程` 和 `工程/审查`，`工程` 只算它一次。
/// 零计数的类别保留（词表里有、还没挂东西）—— 建了看不见比灰着更糟。
pub(crate) fn facet_tree(categories: &[String], presets: &[Preset]) -> Vec<CatNode> {
    let mut order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    for c in categories {
        for a in ancestors(c) {
            if seen.insert(a.clone()) {
                order.push(a);
            }
        }
    }
    let mut extra: Vec<String> = Vec::new();
    for p in presets {
        for c in &p.category {
            for a in ancestors(c) {
                if !seen.contains(&a) && !extra.contains(&a) {
                    extra.push(a);
                }
            }
        }
    }
    extra.sort_by(|a, b| depth_of(a).cmp(&depth_of(b)).then_with(|| a.cmp(b)));
    order.extend(extra);

    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    for p in presets {
        let mut mine: std::collections::HashSet<String> = Default::default();
        for c in &p.category {
            for a in ancestors(c) {
                mine.insert(a);
            }
        }
        for a in mine {
            *counts.entry(a).or_default() += 1;
        }
    }

    order
        .iter()
        .map(|path| CatNode {
            path: path.clone(),
            label: last_seg(path),
            depth: depth_of(path),
            count: counts.get(path).copied().unwrap_or(0),
            has_children: order.iter().any(|o| o != path && is_under(o, path)),
        })
        .collect()
}

/// 切段时那个类别能不能留：新段的树里有同一个路径（或它就是未分类）就留，否则回「全部」。
fn keep_category(cat: &Option<String>, tree: &[CatNode]) -> Option<String> {
    match cat.as_deref() {
        None => None,
        Some(UNCATEGORIZED) => Some(UNCATEGORIZED.to_string()),
        Some(p) if tree.iter().any(|n| n.path == p) => Some(p.to_string()),
        _ => None,
    }
}

/// 这条预设算不算在 `cat` 这一类里（父级天然包含整枝）。
fn in_category(p: &Preset, cat: &str) -> bool {
    p.category.iter().any(|c| is_under(c, cat))
}

/// 这条预设是不是只读。`readOnly` 住在 `spec` 里（顶层没有这个字段）。
fn read_only(p: &Preset) -> bool {
    p.spec
        .as_ref()
        .and_then(|s| s.get("readOnly"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 从 `spec` 里读一个字符串字段（`Ultimate-Template` 的 `trigger` / `session`）。
fn spec_str(p: &Preset, key: &str) -> Option<String> {
    p.spec
        .as_ref()
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn pick_note(text: String, color: u32) -> AnyElement {
    div()
        .px(t().d(10.))
        .py(t().d(14.))
        .text_size(t().fs(11.5))
        .text_color(rgb(color))
        .child(text)
        .into_any_element()
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

fn chip(text: &str, bg: u32, fg: u32) -> AnyElement {
    div()
        .px(t().d(5.))
        .h(t().d(16.))
        .rounded(t().r(3.))
        .bg(rgb(bg))
        .text_size(t().fs(10.))
        .text_color(rgb(fg))
        .child(text.to_string())
        .into_any_element()
}

fn note(text: &str) -> AnyElement {
    div()
        .p(t().d(12.))
        .text_size(t().fs(12.))
        .text_color(rgb(muted()))
        .child(text.to_string())
        .into_any_element()
}

/// 这一类/这一段里一条都没有时的出路。分三种归因，别都写成「没有数据」。
fn empty_hint(title: &str, section_title: &str, category_count: usize) -> AnyElement {
    let (head, sub) = if title == "全部" {
        (
            format!("「{section_title}」这一段还没有东西"),
            format!("词表里已经有 {category_count} 个类别 —— 去面板建第一条，它就会长出来"),
        )
    } else if title == "未分类" {
        (
            "每条都挂上类别了".to_string(),
            "未分类这一档空着挺好".to_string(),
        )
    } else {
        (
            format!("「{title}」下还没有预设"),
            "词表里先建的类别，还没挂东西".to_string(),
        )
    };
    div()
        .p(t().d(20.))
        .flex()
        .flex_col()
        .gap(t().d(6.))
        .items_center()
        .child(
            div()
                .text_size(t().fs(12.5))
                .text_color(rgb(t().p.muted))
                .child(head),
        )
        .child(
            div()
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.muted))
                .child(sub),
        )
        .into_any_element()
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    // **别写 `use super::*;`**：`super` 链上有 `gpui_kit::*`，它带一个 `test` 宏，
    // 会把 `#[test]` 抢过去（报「recursion limit reached while expanding #[test]」）。
    // `app.rs` 里的测试也是只挑要用的东西，同一个原因。
    use super::{
        EXPORT_ENVELOPE, Preset, UNCATEGORIZED, export_doc, facet_tree, import_doc, in_category,
        keep_category, matches_query, sanitize_file_name,
    };

    /// 导出成什么样：信封 + 清单 + 说明。`section` / `name` 留着，人打开文件能看懂。
    #[test]
    fn export_wraps_the_manifest_and_the_readme() {
        let doc = export_doc(
            "Session-Template",
            "scratch",
            serde_json::json!({"name": "scratch", "summary": "随手记"}),
            Some("# 说明".into()),
        );
        assert_eq!(doc[EXPORT_ENVELOPE], 1);
        assert_eq!(doc["section"], "Session-Template");
        assert_eq!(doc["name"], "scratch");
        assert_eq!(doc["spec"]["summary"], "随手记");
        assert_eq!(doc["body"], "# 说明");

        // 没有 PRESET.md 的模板也导得出来，`body` 是 null。
        let bare = export_doc("Session-Template", "x", serde_json::json!({}), None);
        assert!(bare["body"].is_null());
    }

    /// 导入两种形状都收：我们的包，以及**别人手写的一份裸 `preset.json`**。
    #[test]
    fn import_takes_our_envelope_and_a_bare_manifest() {
        let pkg = serde_json::json!({
            EXPORT_ENVELOPE: 1,
            "section": "Session-Template",
            "name": "scratch",
            "spec": {"name": "scratch", "summary": "s"},
            "body": "说明",
        });
        let (spec, body) = import_doc(pkg).expect("包该认");
        assert_eq!(spec["summary"], "s");
        assert_eq!(body.as_deref(), Some("说明"));

        // 裸清单：最常见的「别人扔一份 json 过来」。
        let bare = serde_json::json!({"name": "asd", "summary": "旧模板"});
        let (spec, body) = import_doc(bare).expect("裸清单该认");
        assert_eq!(spec["name"], "asd");
        assert_eq!(body, None);

        // 坏东西要说人话，不能静默导进一个空模板。
        assert!(import_doc(serde_json::json!("不是对象")).is_err());
        assert!(import_doc(serde_json::json!({EXPORT_ENVELOPE: 1})).is_err());
        assert!(
            import_doc(serde_json::json!({EXPORT_ENVELOPE: 1, "spec": [1, 2]})).is_err()
        );
    }

    /// 名字里的 `/` 不能带进文件名（`写作/周报` 是合法的类别名，也是合法的模板名）。
    #[test]
    fn file_name_for_export_drops_path_separators() {
        assert_eq!(sanitize_file_name("写作/周报"), "写作-周报");
        assert_eq!(sanitize_file_name("a\\b:c*d"), "a-b-c-d");
        assert_eq!(sanitize_file_name(".hidden"), "hidden");
        assert_eq!(sanitize_file_name("   "), "template");
    }

    /// 过滤词命中：名字 / 摘要 / 类别，任一包含就算（不分大小写）。
    /// 空的过滤词**永远为真** —— 否则列表在没人搜的时候会整片空掉。
    #[test]
    fn query_hits_name_summary_and_category() {
        let mut p = preset("deep-dive", &["写作/周报"]);
        p.summary = "深挖一个话题".into();
        assert!(matches_query(&p, ""), "空词应该全中");
        assert!(matches_query(&p, "  "), "空白词也不算搜");
        assert!(matches_query(&p, "DEEP"), "名字不分大小写");
        assert!(matches_query(&p, "深挖"), "摘要也能搜");
        assert!(matches_query(&p, "周报"), "类别也要能搜 —— 那是列表里看得见的东西");
        assert!(!matches_query(&p, "flash"), "完全不相干的词不该命中");
        assert!(!matches_query(&p, "写作 周报"), "整串匹配，不做分词");
    }

    fn preset(name: &str, categories: &[&str]) -> Preset {
        Preset {
            name: name.into(),
            summary: String::new(),
            description: String::new(),
            category: categories.iter().map(|c| c.to_string()).collect(),
            spec: None,
            spec_error: None,
            tracked: true,
            dir: String::new(),
            resolved: None,
            extra: Default::default(),
        }
    }

    fn cats(list: &[&str]) -> Vec<String> {
        list.iter().map(|c| c.to_string()).collect()
    }

    /// 顺序完全跟引擎给的那份走（父在子前），客户端不重排。
    #[test]
    fn 树顺序跟引擎一样() {
        let tree = facet_tree(
            &cats(&["工程", "写作", "写作/周报", "Basics"]),
            &[preset("a", &["工程"])],
        );
        let paths: Vec<&str> = tree.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, ["工程", "写作", "写作/周报", "Basics"]);
        assert_eq!(tree[2].depth, 1);
        assert_eq!(tree[2].label, "周报");
        assert!(tree[1].has_children);
        assert!(!tree[2].has_children);
    }

    /// 零计数的类别也得在（Basics 原来是这种）。
    #[test]
    fn 零计数类别不丢() {
        let tree = facet_tree(&cats(&["工程", "Basics"]), &[preset("a", &["工程"])]);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[1].count, 0);
    }

    /// 计数含后代、同一条预设按路径去重。
    #[test]
    fn 计数含后代且去重() {
        let presets = [
            preset("a", &["工程", "工程/审查"]),
            preset("b", &["工程"]),
            preset("c", &["写作/周报"]),
        ];
        let tree = facet_tree(&cats(&["工程", "工程/审查", "写作", "写作/周报"]), &presets);
        let count = |p: &str| tree.iter().find(|n| n.path == p).unwrap().count;
        assert_eq!(count("工程"), 2, "a 挂了父级和子级，只算一次");
        assert_eq!(count("工程/审查"), 1);
        assert_eq!(count("写作"), 1, "只挂子级的也算进父级");
        assert_eq!(count("写作/周报"), 1);
    }

    /// 预设上挂着、词表里没有的路径要补出来（引擎漏了也不能让条目消失）。
    #[test]
    fn 词表缺了也能补() {
        let tree = facet_tree(&cats(&["工程"]), &[preset("a", &["写作/周报"])]);
        let paths: Vec<&str> = tree.iter().map(|n| n.path.as_str()).collect();
        assert_eq!(paths, ["工程", "写作", "写作/周报"]);
        assert_eq!(tree[2].count, 1);
    }

    #[test]
    fn 父级筛选包含整枝() {
        let p = preset("a", &["写作/周报"]);
        assert!(in_category(&p, "写作"));
        assert!(in_category(&p, "写作/周报"));
        assert!(!in_category(&p, "写作/规格"));
        // 前缀相同但不是一层：`写作/周` 不能命中 `写作/周报`
        assert!(!in_category(&p, "写作/周"));
    }
    /// 切段：新段有同一个类别就留，没有就回全部；未分类是哨兵，永远能留。
    #[test]
    fn 切段时类别能留就留() {
        let tree = facet_tree(&cats(&["工程", "日常"]), &[]);
        assert_eq!(
            keep_category(&Some("工程".into()), &tree),
            Some("工程".into())
        );
        assert_eq!(keep_category(&Some("写作".into()), &tree), None);
        assert_eq!(
            keep_category(&Some(UNCATEGORIZED.into()), &tree),
            Some(UNCATEGORIZED.into())
        );
        assert_eq!(keep_category(&None, &tree), None);
    }
}

// ── 渲染：右栏关系图（37 节点图） ──────────────────────────

impl Desktop {
/// 打开 / 收起 `＋` 菜单里「从模板起」那张清单。
///
/// 它是**左栏里的一块浮层**，不是第二级菜单（demo 里 B 方案定的）：选中一条就直接开，
/// 不再进 New Manual 盘面 —— 盘面那一圈「选模型 / 选技能 / 选目录」模板自己会带。
pub(crate) fn open_preset_pick(&mut self, cx: &mut Context<Self>) {
    self.preset_pick = true;
    // 拉当前段的清单（拉过就不重拉）。`Session-Template` 才是「能开」的那一段。
    self.load_presets(false, cx);
    cx.notify();
}

pub(crate) fn close_preset_pick(&mut self, cx: &mut Context<Self>) {
    if !self.preset_pick {
        return;
    }
    self.preset_pick = false;
    cx.notify();
}

/// 「从模板起」那张卡。挂在左列里（`render_session_list` 的最后），没有就返回 `None`。
///
/// 三条出口都在：点遮罩、`Esc`（走 `intercept_keystrokes` 那条链）、再点一次菜单项。
pub(crate) fn preset_pick_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
    if !self.preset_pick {
        return None;
    }
    let sec = &self.sections[self.section];
    let mut rows: Vec<AnyElement> = Vec::new();

    if let Some(err) = &sec.error {
        rows.push(pick_note(format!("清单拉不到：{err}"), bad()));
    } else if sec.data.is_none() {
        rows.push(pick_note("拉模板清单…".into(), muted()));
    } else if let Some(d) = &sec.data {
        if d.presets.is_empty() {
            rows.push(pick_note("这一段还没有模板".into(), muted()));
        } else {
            for p in &d.presets {
                rows.push(self.preset_pick_row(p, cx));
            }
        }
    }

    Some(
        div()
            .id("preset-pick-scrim")
            .absolute()
            .inset_0()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_preset_pick(cx)))
            .child(
                div()
                    // 贴着 `＋` 下面那块地：左列宽 280，卡片左右各留 8。
                    .absolute()
                    .left(t().d(8.))
                    .right(t().d(8.))
                    .top(t().d(46.))
                    .max_h(t().d(380.))
                    .flex()
                    .flex_col()
                    .bg(rgb(t().p.panel))
                    .rounded(t().r(9.))
                    .border_1()
                    .border_color(rgb(t().p.line_strong))
                    .shadow_md()
                    .overflow_hidden()
                    // 卡片里的空白（标题下、行之间）没有 hitbox：不挡的话点上去会落到罩子上，
                    // 挑模板时手一抖就关（跟历史抽屉同一条，见 `nav_ui::drawer`）。
                    .block_mouse_except_scroll()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(6.))
                            .px(t().d(10.))
                            .h(t().d(30.))
                            .border_b_1()
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .text_size(t().fs(11.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("从模板起 · 选一条直接开会话"),
                            )
                            .child(
                                div()
                                    .ml_auto()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.faint))
                                    .child("Esc 关"),
                            ),
                    )
                    .child(
                        div()
                            .id("preset-pick-scroll")
                            .flex_1()
                            .min_h(t().d(0.))
                            .overflow_y_scroll()
                            .children(rows),
                    ),
            )
            .into_any_element(),
    )
}

/// 清单里的一行。开不了的那几条也画出来、只是灰着并写原因 —— 藏起来会让人
/// 以为模板没了（跟 Template 页同一条）。
fn preset_pick_row(&self, p: &Preset, cx: &mut Context<Self>) -> AnyElement {
    let open = launchable(p);
    let name = p.name.clone();
    let summary = p.summary.clone();
    div()
        .id(ElementId::Name(format!("pick-{}", p.name).into()))
        .flex()
        .flex_col()
        .gap(t().d(1.))
        .px(t().d(10.))
        .py(t().d(6.))
        .when(open, |el| {
            el.cursor_pointer()
                .hover(|s| s.bg(rgb(t().p.hover)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    // 选中就开：关卡片 → 走 Template 页那条一模一样的老路
                    // （`POST /v1/sessions { preset }` + 名字带时间）。
                    this.close_preset_pick(cx);
                    this.launch_preset(name.clone(), cx);
                }))
        })
        .child(
            div()
                .text_size(t().fs(12.5))
                .text_color(rgb(if open { t().p.text } else { t().p.faint }))
                .child(p.name.clone()),
        )
        .child(
            div()
                .text_size(t().fs(11.))
                .text_color(rgb(if open { t().p.muted } else { t().p.faint }))
                .truncate()
                .child(if open {
                    summary
                } else if p.spec_error.is_some() {
                    "这条自己写坏了（JSON / 形状）".to_string()
                } else {
                    "引用失效，现在开不了".to_string()
                }),
        )
        .into_any_element()
}


    /// Template 页第三栏：选中那条预设的**关系图**。
    ///
    /// 图是「看一眼就走」的：中心 = 这条预设，一圈 = 它钉住的维度，外圈 = 具体的
    /// 技能/扩展。节点颜色直接抄引擎的 `resolved.*.state` —— 这边不算「能不能开」。
    pub(crate) fn render_new_preset(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(draft) = self.new_preset.as_ref() else {
            return div().into_any_element();
        };
        let sec = SECTIONS[self.section].1;
        let busy = draft.busy;
        let err = draft.err.clone();
        let has_err = err.is_some();
        let from_device = draft.from_device;
        let name_ok = !draft.name.read(cx).value().trim().is_empty();
        let sum_ok = !draft.summary.read(cx).value().trim().is_empty();
        let done: u8 = u8::from(name_ok) + u8::from(sum_ok);
        let pct = f32::from(done) / 2.;
        let can = name_ok && sum_ok && !busy;
        // 滚动容器（`overflow_y_scroll`）在 gpui 里会把内容按 min-content 收缩 ——
        // 640 宽时清单只剩 20px（标题「新建模板」被裁成一个「新」）。给它一个
        // 显式宽度就老实了：右栏 = 窗口 - 图标条 - 左边那一两栏。
        let vw = f32::from(window.viewport_size().width);
        let folder = vw < crate::app::TREE_FOLD_W;
        let used = t().rail_w
            + if folder {
                crate::app::TEMPLATE_LIST_W_NARROW
            } else {
                t().preset_tree_w + t().template_list_w
            };
        let col_w = (vw - used).max(240.);
        // 类别候选：这一段的词表（前 5 个）。点一下就是把它填进输入框 ——
        // 不是另一套状态，输入框里那串字始终是唯一的真相。
        let cats: Vec<String> = self.sections[self.section]
            .data
            .as_ref()
            .map(|d| d.categories.iter().take(5).cloned().collect())
            .unwrap_or_default();

        // 一行清单。`done` 只决定圈和标题颜色，行里的控件不受影响。
        let item = |done: bool, title: &'static str, hint: Option<&'static str>, body: AnyElement| {
            div()
                .flex()
                .flex_row()
                .gap(t().d(12.))
                .py(t().d(13.))
                .border_b(t().d(1.))
                .border_color(rgb(t().p.line_soft))
                .child(
                    div()
                        .w(t().d(16.))
                        .h(t().d(16.))
                        .mt(t().d(2.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(t().r(8.))
                        .when(done, |e| {
                            e.bg(rgb(t().p.accent))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.accent))
                        })
                        .when(!done, |e| {
                            e.border(t().d(1.)).border_color(rgb(t().p.line))
                        })
                        .children(done.then(|| {
                            div()
                                .text_size(t().fs(10.))
                                .line_height(px(10.))
                                .text_color(rgb(t().p.on_accent))
                                .child("✓")
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(t().d(0.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(t().d(6.))
                                .child(
                                    div()
                                        .text_size(t().fs(12.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(if done { t().p.text } else { t().p.text_2 }))
                                        .child(title),
                                )
                                .children(hint.map(|h| {
                                    div()
                                        .text_size(t().fs(10.))
                                        .text_color(rgb(t().p.faint))
                                        .child(h)
                                })),
                        )
                        .child(body),
                )
                .into_any_element()
        };

        let pill = |text: String, on: bool| {
            div()
                .flex()
                .items_center()
                .h(t().d(22.))
                .px(t().d(9.))
                .rounded(t().r(11.))
                .border(t().d(1.))
                .border_color(rgb(if on { t().p.accent } else { t().p.line }))
                .bg(rgb(if on { t().p.accent } else { t().p.panel }))
                .text_size(t().fs(11.))
                .text_color(rgb(if on { t().p.on_accent } else { t().p.text_2 }))
                .child(text)
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_w(t().d(0.))
            .bg(rgb(t().p.chat_bg))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(t().d(40.))
                    .px(t().d(26.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .gap(t().d(8.))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{sec} · 新建")),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.faint))
                            .child(if done >= 2 {
                                "必填项都填了".to_string()
                            } else {
                                format!("必填 {} 项 · 还差 {} 项", 2, 2 - done)
                            }),
                    ),
            )
            .child(
                div()
                    .id("new-preset-body")
                    .flex_1()
                    .w(px(col_w))
                    .min_w(t().d(0.))
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .px(t().d(26.))
                    .py(t().d(20.))
                    .child(
                        div()
                            .w_full()
                            .max_w(t().d(540.))
                            .flex()
                            .flex_col()
                            // 标题 + 百分比
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_end()
                                    .gap(t().d(12.))
                                    .child(
                                        div()
                                            .text_size(t().fs(20.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("新建模板"),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.muted))
                                            .child(format!("{}%", (pct * 100.) as i32)),
                                    ),
                            )
                            // 进度条：一条细线，两条 flex 按比例分宽度
                            .child(
                                div()
                                    .mt(t().d(12.))
                                    .mb(t().d(18.))
                                    .h(t().d(3.))
                                    .w_full()
                                    .rounded(t().r(2.))
                                    .bg(rgb(t().p.line_soft))
                                    .flex()
                                    .child(
                                        div()
                                            .h_full()
                                            .flex_grow(pct)
                                            .bg(rgb(t().p.accent)),
                                    )
                                    .child(div().h_full().flex_grow(1. - pct)),
                            )
                            .child(item(
                                name_ok,
                                "名字",
                                None,
                                div().pt(t().d(8.)).child(Input::new(&draft.name)).into_any_element(),
                            ))
                            .child(item(
                                sum_ok,
                                "摘要",
                                Some(if sum_ok { "" } else { "必填" }),
                                div().pt(t().d(8.)).child(Input::new(&draft.summary)).into_any_element(),
                            ))
                            .child(item(
                                false,
                                "类别",
                                Some("可改 · 留空 = 未分类"),
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(t().d(8.))
                                    .pt(t().d(8.))
                                    .child(Input::new(&draft.category))
                                    .children((!cats.is_empty()).then(|| {
                                        div()
                                            .flex()
                                            .flex_row()
                                            .flex_wrap()
                                            .gap(t().d(6.))
                                            .children(cats.iter().enumerate().map(|(i, c)| {
                                                let fill = c.clone();
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("new-cat-{i}").into(),
                                                    ))
                                                    .flex()
                                                    .items_center()
                                                    .h(t().d(22.))
                                                    .px(t().d(9.))
                                                    .rounded(t().r(11.))
                                                    .border(t().d(1.))
                                                    .border_color(rgb(t().p.line))
                                                    .text_size(t().fs(11.))
                                                    .text_color(rgb(t().p.text_2))
                                                    .cursor_pointer()
                                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, window, cx| {
                                                            if let Some(d) = this.new_preset.as_ref() {
                                                                let fill = fill.clone();
                                                                d.category.update(cx, |s, cx| {
                                                                    s.set_value(fill, window, cx)
                                                                });
                                                            }
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child(c.clone())
                                            }))
                                    }))
                                    .into_any_element(),
                            ))
                            .child(item(
                                false,
                                "起手式",
                                Some("可改"),
                                div()
                                    .pt(t().d(8.))
                                    .flex()
                                    .flex_row()
                                    .gap(t().d(6.))
                                    .child(
                                        div()
                                            .id("new-from-blank")
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.set_new_from(false, cx)
                                            }))
                                            .child(pill("空白 —— 一条都不写".to_string(), !from_device)),
                                    )
                                    .child(
                                        div()
                                            .id("new-from-device")
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.set_new_from(true, cx)
                                            }))
                                            .child(pill("跟设备默认模型".to_string(), from_device)),
                                    )
                                    .into_any_element(),
                            )),
                    ),
            )
            // 底栏：创建（没填完时置灰，但点了会说出缺哪一项）/ 提示 / 取消
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(10.))
                    .px(t().d(26.))
                    .py(t().d(14.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .id("new-preset-submit")
                            .flex()
                            .items_center()
                            .h(t().d(30.))
                            .px(t().d(16.))
                            .rounded(t().r(6.))
                            .bg(rgb(if can { t().p.accent } else { t().p.disabled }))
                            .text_size(t().fs(12.))
                            .text_color(rgb(if can { t().p.on_accent } else { t().p.text }))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.submit_new_preset(cx)
                            }))
                            .child(if busy { "建…" } else { "创建" }),
                    )
                    .children(err.map(|e| {
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.danger_fg))
                            .child(e)
                    }))
                    .children((!can && !has_err).then(|| {
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.faint))
                            .child("名字和摘要是必填，填完就能建")
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("new-preset-cancel")
                            .flex()
                            .items_center()
                            .h(t().d(30.))
                            .px(t().d(12.))
                            .rounded(t().r(6.))
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.text_2))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_new_preset(cx)))
                            .child("取消"),
                    ),
            )
            // `window` 只在类别 chip 里用到（写输入框）；不用的话编译器会警告。
            .when(false, |e| e.child(format!("{}", window.is_window_active())))
            .into_any_element()
    }

    pub(crate) fn render_detail(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dw = t().detail_w;
        if dw <= 0. {
            return div().into_any_element();
        }

        // 窄窗别把 `detail_w`（默认 420）当最小宽：640 宽时右栏只剩 344，
        // 顶着 420 的下限就会把顶栏右边那半截（「开一条新对话」按钮）裁掉。
        let min_w = if window.viewport_size().width.as_f32() < crate::app::TREE_FOLD_W {
            t().d(0.)
        } else {
            px(dw)
        };
        let shell = |inner: AnyElement| {
            div()
                .flex()
                .flex_col()
                // 右栏吃掉剩余宽度（`detail_w` 是最小宽）：窗口更宽时图居中留白，
                // 而不是在右边露出一条没人用的墙。
                .flex_1()
                .w_full()
                .min_w(min_w)
                .h_full()
                .bg(rgb(t().p.panel))
                .border_l(t().d(1.))
                .border_color(rgb(t().p.line))
                .child(inner)
                .into_any_element()
        };

        let Some(name) = self.picked_preset.clone() else {
            return shell(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(t().d(6.))
                    .size_full()
                    .text_center()
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(muted()))
                            .child("选一条模板"),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.faint))
                            .child("右边会画出它钉了什么、哪些走默认"),
                    )
                    .into_any_element(),
            );
        };

        let sec = &self.sections[self.section];
        if sec.loading && sec.data.is_none() {
            return shell(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(8.))
                    .p(t().d(16.))
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(muted()))
                            .child("拉这一段的模板中…"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t().d(8.))
                            .children((0..3).map(|i| {
                                div()
                                    .h(t().d(34.))
                                    .w_full()
                                    .rounded(t().r(7.))
                                    .bg(rgb(t().p.panel_alt))
                                    .opacity(0.9 - i as f32 * 0.22)
                            }))
                            // 脉动只在这一层：它是「在等」的信号，不是装饰。
                            // 900ms 一轮，明暗 0.55–0.95 —— 再快像坏掉，再慢像卡住。
                            .with_animation(
                                ElementId::Name("graph-skeleton".into()),
                                Animation::new(std::time::Duration::from_millis(900))
                                    .repeat()
                                    .with_easing(pulsating_between(0.55, 0.95)),
                                |el, delta| el.opacity(delta),
                            ),
                    )
                    .into_any_element(),
            );
        }
        let Some(p) = sec
            .data
            .as_ref()
            .and_then(|d| d.presets.iter().find(|x| x.name == name))
        else {
            return shell(
                div()
                    .p(t().d(16.))
                    .text_size(t().fs(12.))
                    .text_color(rgb(muted()))
                    .child("这条已经不在列表里了")
                    .into_any_element(),
            );
        };

        let nodes = crate::templates_graph::nodes(p);
        // 右栏是 `flex_1`，实际宽度跟窗口走 —— 半径必须按**真实宽度**算。
        let vw = f32::from(window.viewport_size().width);
        let vh = f32::from(window.viewport_size().height);
        let rail = t().rail_w;
        // 窄窗时左两栏跟渲染那边一样（类别树进抽屉、列表 240）—— 否则这里会
        // 以为右栏有 420，图按 396 画，右边一截直接推到窗口外。
        let folder = vw < crate::app::TREE_FOLD_W;
        let used = rail
            + if folder { 0. } else { t().preset_tree_w }
            + if folder {
                crate::app::TEMPLATE_LIST_W_NARROW
            } else {
                t().template_list_w
            };
        let col = (vw - used).max(if folder { 240. } else { dw });
        let (gw, gh) = (
            (col - t().d(24.).as_f32()).max(220.),
            (vh - t().d(150.).as_f32()).max(300.),
        );
        let pts = crate::templates_graph::layout(&nodes, gw, gh);
        let (cx0, cy0) = (gw / 2., gh / 2. - 10.);
        let r0 = (gw * 0.40).min(gh * 0.37);

        // demo 37 的点色是 Flexoki 的固定三色，**不跟皮肤走** —— 这是这张图的识别度。
        // 换了工业皮之后，蓝色描边在暖纸底上照样是蓝的，这才对。
        const FX_BLUE: u32 = 0x4385be;
        const FX_GREEN: u32 = 0x879a39;
        const FX_RED: u32 = 0xd14d41;

        let dot_color = |n: &crate::templates_graph::Node| -> u32 {
            match n.state {
                crate::templates_graph::State::Invalid => FX_RED,
                crate::templates_graph::State::Auto => t().p.muted,
                crate::templates_graph::State::Unknown => t().p.muted,
                // 引用用绿、字段用蓝 —— demo 里那个「两种颜色」就是这个意思。
                crate::templates_graph::State::Ok => {
                    if n.is_ref {
                        FX_GREEN
                    } else {
                        FX_BLUE
                    }
                }
            }
        };

        // 连线：中心 → 每个点。demo 里是实线、直接穿过标签文字，不避让。
        // demo 里两层环和连线都是 `var(--line)` —— 工业皮下就是纯黑，
        // 在浅色皮（flexoki-paper）里它自己会变浅。跟着皮肤走，别写死。
        let ring: gpui::Hsla = rgb(t().p.line).into();
        let segs: Vec<(f32, f32, gpui::Hsla)> = nodes
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, n)| {
                let c = match n.state {
                    crate::templates_graph::State::Invalid => FX_RED,
                    _ => t().p.line,
                };
                (
                    pts[i].x,
                    pts[i].y,
                    rgb(c).opacity(if n.state.is_auto() { 0.45 } else { 1.0 }).into(),
                )
            })
            .collect();

        let canvas_layer = div()
            .id(ElementId::Name("graph-bg".into()))
            .absolute()
            .top_0()
            .left_0()
            .w(px(gw))
            .h(px(gh))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if this.picked_node.take().is_some() {
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |_, _, _| (cx0, cy0, r0, segs.clone(), ring),
                    |bounds, (cx, cy, r, segs, ring), window, _| {
                        let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                        let circle = |rad: f32| {
                            let mut v = Vec::with_capacity(120);
                            for i in 0..120 {
                                let a = std::f32::consts::TAU * i as f32 / 120.;
                                v.push(point(
                                    px(ox + cx + rad * a.cos()),
                                    px(oy + cy + rad * a.sin()),
                                ));
                            }
                            v
                        };
                        // 两层点线环（demo 的 0.5 / 1.0）。
                        for k in [0.48f32, 1.0] {
                            let mut b =
                                PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(5.)]);
                            b.add_polygon(&circle(r * k), true);
                            if let Ok(path) = b.build() {
                                window.paint_path(path, ring);
                            }
                        }
                        for (x, y, color) in segs {
                            let mut b = PathBuilder::stroke(px(1.));
                            b.move_to(point(px(ox + cx), px(oy + cy)));
                            b.line_to(point(px(ox + x), px(oy + y)));
                            if let Ok(path) = b.build() {
                                window.paint_path(path, color);
                            }
                        }
                    },
                )
                .size_full(),
            );

        // 点 + 悬浮标签。中心圆单独一块（它是实心的，不是空心点）。
        let mut layer = div().absolute().top_0().left_0().w(px(gw)).h(px(gh));

        layer = layer.child(
            div()
                .id(ElementId::Name("graph-center".into()))
                .absolute()
                .left(px(cx0 - 31.))
                .top(px(cy0 - 31.))
                .size(px(62.))
                .rounded_full()
                .bg(rgb(t().p.text))
                .flex()
                .items_center()
                .justify_center()
                .px(px(6.))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.picked_node = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .text_size(t().fs(9.))
                        .text_color(rgb(t().p.on_accent))
                        .whitespace_nowrap()
                        .text_overflow(TextOverflow::Truncate("…".into()))
                        // 62px 的圆放不下长名字，两边都会被裁（`read-only-review` 会变成
                        // "ad-only-review"）。按 demo 的做法先截到 9 个字。
                        .child(short_name(&p.name, 9)),
                ),
        );

        for i in 1..nodes.len() {
            let n = &nodes[i];
            let (x, y) = (pts[i].x, pts[i].y);
            let left_side = x < cx0;
            let color = dot_color(n);
            let picked = self.picked_node.as_deref() == Some(n.id.as_str());
            let dot_id = n.id.clone();

            layer = layer.child(
                div()
                    .id(ElementId::Name(format!("gd-{}", n.id).into()))
                    .absolute()
                    .left(px(x - 6.))
                    .top(px(y - 6.))
                    .size(px(12.))
                    .rounded_full()
                    .border(px(if picked { 3. } else { 2. }))
                    .border_color(rgb(color))
                    .bg(rgb(t().p.panel))
                    .hover(|h| h.bg(rgb(color)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.picked_node = Some(dot_id.clone());
                        cx.notify();
                    })),
            );

            // 标签：左半圈右对齐到点的左侧，右半圈左对齐到点的右侧（demo 是这样）。
            // **但窄右栏放不下时就翻到另一边**：640 宽时右栏只有 344，最右那个点
            // 右侧只剩 20px，标签会画出窗口（截图里「思考 medium」就是这么没的）。
            let left_room = x - 16.;
            let right_room = gw - x - 16.;
            let on_left = if left_side {
                left_room >= 40.
            } else {
                right_room < 40.
            };
            let (lw, lx) = if on_left {
                let w = left_room.clamp(24., 132.);
                (w, (x - 10. - w).max(0.))
            } else {
                let w = right_room.clamp(24., 132.);
                (w, (x + 10.).min(gw - w))
            };
            let label_id = n.id.clone();
            layer = layer.child(
                div()
                    .id(ElementId::Name(format!("gl-{}", n.id).into()))
                    .absolute()
                    .left(px(lx))
                    .top(px(y - 17.))
                    .w(px(lw))
                    .h(px(34.))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(1.))
                    .when(on_left, |el| el.items_end())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.picked_node = Some(label_id.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(t().fs(8.5))
                            .text_color(rgb(muted()))
                            .whitespace_nowrap()
                            .text_overflow(TextOverflow::Truncate("…".into()))
                            .child(n.label.clone()),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(if picked { t().p.accent } else { t().p.text }))
                            .whitespace_nowrap()
                            .text_overflow(TextOverflow::Truncate("…".into()))
                            .child(if n.value.is_empty() {
                                "未设".to_string()
                            } else {
                                n.value.clone()
                            }),
                    ),
            );
        }

        let open_name = p.name.clone();
        let edit_name = p.name.clone();
        let can_open = sec.id == SECTIONS[0].0 && launchable(p);

        shell(
            div()
                .flex()
                .flex_col()
                .size_full()
                // 头：名字 17px 与摘要**同一行**（demo 的 baseline 排版），
                // 开会话的按钮挪到这一行右端，不再占一整条底栏。
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(t().d(10.))
                        .px(t().d(14.))
                        .pt(t().d(12.))
                        .pb(t().d(4.))
                        .child(
                            div()
                                .flex_none()
                                .text_size(t().fs(17.))
                                .font_weight(FontWeight::BOLD)
                                .child(p.name.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(t().d(0.))
                                .text_size(t().fs(11.))
                                .text_color(rgb(muted()))
                                .whitespace_nowrap()
                                .text_overflow(TextOverflow::Truncate("…".into()))
                                .child(p.summary.clone()),
                        )
                        // 「编辑」放在开会话左边：改内容比再开一条更常做。
                        .child(
                            div()
                                .id(ElementId::Name("detail-edit".into()))
                                .flex_none()
                                .px(t().d(9.))
                                .py(t().d(5.))
                                .rounded(t().r(6.))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.line))
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.text_2))
                                .cursor_pointer()
                                .hover(|h| h.bg(rgb(t().p.hover)))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.open_editor(edit_name.clone(), cx);
                                }))
                                .child("编辑"),
                        )
                        .when(can_open, |el| {
                            el.child(
                                div()
                                    .flex_none()
                                    .px(t().d(10.))
                                    .py(t().d(5.))
                                    .rounded(t().r(6.))
                                    .bg(rgb(t().p.text))
                                    .text_color(rgb(t().p.on_accent))
                                    .text_size(t().fs(11.))
                                    .id(ElementId::Name("detail-open".into()))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.launch_preset(open_name.clone(), cx);
                                    }))
                                    .child("开一条新对话"),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .w_full()
                        .min_h(t().d(0.))
                        .items_center()
                        .justify_center()
                        .when(nodes.len() <= 1, |el| {
                            el.child(
                                div()
                                    .max_w(t().d(240.))
                                    .text_center()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(muted()))
                                    .child("这一条没有可摊开的关系数据 —— 引擎没给 resolved，spec 也没写多少"),
                            )
                        })
                        .when(nodes.len() > 1, |el| {
                            let w = gw;
                            let h = gh;
                            el.child(
                                div()
                                    .relative()
                                    .w(px(w))
                                    .h(px(h))
                                    .child(canvas_layer)
                                    .child(layer)
                                    .with_animation(
                                        ElementId::Name(format!("graph-fade-{}", p.name).into()),
                                        Animation::new(std::time::Duration::from_millis(160))
                                            .with_easing(ease_out_quint()),
                                        |el, delta| el.opacity(delta),
                                    ),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
