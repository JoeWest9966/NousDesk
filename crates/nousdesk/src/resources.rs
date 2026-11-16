//! 「资源」页：方案 03 —— 一个图标，两段（内容 / 扩展）。
//!
//! 外观稿见 `demo/resource-pages/preview.html` 的 03 号。这一刀是**只读**：
//! 拉 / 切 / 筛 / 搜 / 看详情，写操作（发布 / 归档 / 文件编辑）留第二刀。
//!
//! ## 为什么两段
//!
//! 内容是「技能 + 提示词」：同一个模子（引擎里两个 api 文件互相点名），共用一套
//! 树 / 列表 / 详情。扩展不是列表 —— 有 `catalog` / `builtin` / `variants` /
//! `archived` / `installed` 五类东西，装 / 卸 / 归档是三件不同的事（核心三条 403）。
//! 硬塞成一个列表等于把五种东西涂成同一种颜色，见 `decisions.md` 2026-11-16 那条。
//!
//! ## 数据来源
//!
//! 三份 GET 全在 `nous-engine` 里（`skills()` / `prompts()` / `extensions()`），
//! 并发拉，各自落地 —— 一份挂了只写它自己的红字，别把整页染红（同 `data_console`）。
//! 类别树复用 `crate::category::build_tree`，不第三次手写 count + sort。
//!
//! ## 不做什么
//!
//! - 不在客户端算「这条能不能用」：`load.state` 是引擎给的，照抄。
//! - 不混会话里的「这次带哪些技能」：那是 `POST /v1/sessions` 的字段，已经做完（盘面）。
//! - 第一刀不写盘、不发写请求，一个字节都不写引擎。

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Sizable as _, Size};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::types::{Extension, Extensions, Prompt, Prompts, Skill, Skills};

use crate::app::Desktop;
use crate::category::{CatNode, build_tree, is_under};

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 大段：内容 / 扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Section {
    #[default]
    Content,
    Extensions,
}

/// 内容段里的小类：技能 / 提示词。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Content {
    #[default]
    Skills,
    Prompts,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Self::Content => "内容",
            Self::Extensions => "扩展",
        }
    }
}

impl Content {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skills => "技能",
            Self::Prompts => "提示词",
        }
    }
}

/// 列表选中项。用名字（不是下标）：刷新之后行序可能变，id 才不会指错行。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Selected {
    Skill(String),
    Prompt(String),
    Ext(String),
}

/// 扩展列表里的一行。五种来源都归一成一行，但 `kind` 只用来画 chip ——
/// 行为不合并（装 / 卸 / 归档三件不同的事，第二刀才接）。
#[derive(Debug, Clone)]
pub(crate) struct ExtRow {
    pub(crate) name: String,
    pub(crate) summary: Option<String>,
    pub(crate) category: Vec<String>,
    pub(crate) kind: ExtKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtKind {
    /// 仓库里、本机装了、加载正常。
    On,
    /// 仓库里但本机没装。
    Off,
    /// 加载失败（引擎给了 `load.state == "failed"`）。
    Failed,
    /// 引擎自带的，不在仓库里。
    Builtin,
    /// 本机 checkout 的变体（`<本体>@<id>`）。
    Variant,
    /// `.archive/` 里，别人也看不见。
    Archived,
}

impl ExtKind {
    fn label(self) -> &'static str {
        match self {
            Self::On => "启用中",
            Self::Off => "本机没装",
            Self::Failed => "加载失败",
            Self::Builtin => "内置",
            Self::Variant => "变体",
            Self::Archived => "已归档",
        }
    }

    fn color(self) -> u32 {
        match self {
            Self::On => t().p.accent,
            Self::Failed => t().p.danger,
            _ => t().p.muted,
        }
    }
}

/// 这一页的全部状态。**只有这一页持有**，不塞进 `Desktop`。
pub(crate) struct ResourceState {
    section: Section,
    content: Content,
    skills: Option<Skills>,
    prompts: Option<Prompts>,
    exts: Option<Extensions>,
    /// 三个 tab 各自的类别筛选，切来切去能回到原来那一行。
    cat: [Option<String>; 3],
    /// 三段各自的搜索框。**各存一份**，切段时词还在（跟 `cat` 同款语义）。
    /// 用 `InputState`（不是裸 String）才有中文输入法 / 剪贴板 / 一键清空 —— 这些是
    /// 第一屏体验，跟左栏会话搜索同一套组件。
    search: [Entity<InputState>; 3],
    selected: Option<Selected>,
    loading: bool,
    error: Option<String>,
}

impl ResourceState {
    /// 三个搜索框的订阅源，交给 `Desktop::new` 收进 `_subs`（只在那里拿得到 window）。
    pub(crate) fn search_boxes(&self) -> &[Entity<InputState>; 3] {
        &self.search
    }

    /// 必须拿到 `window` 才能建 `InputState`，所以不是 `Default`。
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let mut box_for = |ph: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(ph))
        };
        Self {
            section: Section::default(),
            content: Content::default(),
            skills: None,
            prompts: None,
            exts: None,
            cat: [None, None, None],
            search: [
                box_for("搜技能：名字 / 描述"),
                box_for("搜提示词：名字 / 摘要"),
                box_for("搜扩展：名字 / 摘要"),
            ],
            selected: None,
            loading: false,
            error: None,
        }
    }


    fn slot(&self) -> usize {
        match self.section {
            Section::Extensions => 2,
            Section::Content => match self.content {
                Content::Skills => 0,
                Content::Prompts => 1,
            },
        }
    }

    fn hydrated(&self) -> bool {
        self.skills.is_some() && self.prompts.is_some() && self.exts.is_some()
    }
}

// ── 行的数据（纯函数，好单测）─────────────────────────────────────────

/// 可加载的技能：隐藏的（`disable-model-invocation`）不进主列表，但树里有「隐藏」这一档能看。
pub(crate) fn visible_skills<'a>(skills: &'a Skills, cat: Option<&str>, q: &str) -> Vec<&'a Skill> {
    let mut out: Vec<&Skill> = skills
        .skills
        .iter()
        .filter(|s| !s.hidden)
        .filter(|s| match cat {
            None => true,
            Some("∅") => s.category.is_empty(),
            Some("@hidden") => false,
            Some(c) => s.category.iter().any(|x| is_under(x, c)),
        })
        .filter(|s| {
            q.trim().is_empty()
                || s.name.to_lowercase().contains(&q.to_lowercase())
                || s.description
                    .as_deref()
                    .is_some_and(|d| d.to_lowercase().contains(&q.to_lowercase()))
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub(crate) fn hidden_skills(skills: &Skills) -> Vec<&Skill> {
    let mut out: Vec<&Skill> = skills.skills.iter().filter(|s| s.hidden).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub(crate) fn visible_prompts<'a>(prompts: &'a Prompts, cat: Option<&str>, q: &str) -> Vec<&'a Prompt> {
    let mut out: Vec<&Prompt> = prompts
        .prompts
        .iter()
        .filter(|p| match cat {
            None => true,
            Some("∅") => p.category.is_empty(),
            Some("@default") => prompts.default.as_deref() == Some(p.name.as_str()),
            Some(c) => p.category.iter().any(|x| is_under(x, c)),
        })
        .filter(|p| {
            q.trim().is_empty()
                || p.name.to_lowercase().contains(&q.to_lowercase())
                || p.summary
                    .as_deref()
                    .is_some_and(|d| d.to_lowercase().contains(&q.to_lowercase()))
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 扩展那一页的五种东西，按「仓库 → 内置 → 变体 → 归档」摊平（同 demo）。
///
/// 顺序不是字母序：`installed` 的先按仓库目录顺序，内置 / 变体 / 归档跟上。
/// demo 里默认勾选的 `catalog` 在最上面 —— 用户找「扩展」先看仓里有什么。
pub(crate) fn ext_rows(exts: &Extensions, cat: Option<&str>, q: &str) -> Vec<ExtRow> {
    let mut out = Vec::new();
    for e in &exts.catalog {
        let failed = exts
            .load
            .get(&e.name)
            .and_then(|l| l.state.as_deref())
            == Some("failed");
        let kind = if failed {
            ExtKind::Failed
        } else if exts.installed.iter().any(|n| n == &e.name) {
            ExtKind::On
        } else {
            ExtKind::Off
        };
        out.push(ExtRow {
            name: e.name.clone(),
            summary: e.summary.clone(),
            category: e.category.clone(),
            kind,
        });
    }
    for name in &exts.builtin {
        out.push(ExtRow {
            name: name.clone(),
            summary: None,
            category: vec![],
            kind: ExtKind::Builtin,
        });
    }
    for (family, variants) in &exts.variants {
        for v in variants {
            let name = v.key.clone().unwrap_or_else(|| v.id.clone().unwrap_or_default());
            if name.is_empty() {
                continue;
            }
            let failed = exts
                .load
                .get(&name)
                .and_then(|l| l.state.as_deref())
                == Some("failed");
            let summary = exts
                .load
                .get(&name)
                .and_then(|l| l.error.clone())
                .or_else(|| Some(format!("{family} 的变体")));
            let category = exts
                .catalog
                .iter()
                .find(|e| e.name == *family)
                .map(|e| e.category.clone())
                .unwrap_or_default();
            out.push(ExtRow {
                name,
                summary,
                category,
                kind: if failed { ExtKind::Failed } else { ExtKind::Variant },
            });
        }
    }
    for a in &exts.archived {
        out.push(ExtRow {
            name: a.name.clone(),
            summary: a.summary.clone(),
            category: a.category.clone(),
            kind: ExtKind::Archived,
        });
    }
    let q_lower = q.trim().to_lowercase();
    out.into_iter()
        .filter(|r| match cat {
            None => true,
            Some("@on") => r.kind == ExtKind::On,
            Some("@bad") => r.kind == ExtKind::Failed,
            Some("@builtin") => r.kind == ExtKind::Builtin,
            Some("@variant") => r.kind == ExtKind::Variant || r.kind == ExtKind::Failed,
            Some("@archived") => r.kind == ExtKind::Archived,
            Some("∅") => r.category.is_empty(),
            Some(c) => r.category.iter().any(|x| is_under(x, c)),
        })
        .filter(|r| {
            q_lower.is_empty()
                || r.name.to_lowercase().contains(&q_lower)
                || r.summary
                    .as_deref()
                    .is_some_and(|s| s.to_lowercase().contains(&q_lower))
        })
        .collect()
}

/// 当前段能看到的条目数（段按钮上那个小数字）。
pub(crate) fn counts(skills: Option<&Skills>, prompts: Option<&Prompts>, exts: Option<&Extensions>) -> (usize, usize, usize) {
    (
        skills.map(|s| s.skills.iter().filter(|x| !x.hidden).count()).unwrap_or(0),
        prompts.map(|p| p.prompts.len()).unwrap_or(0),
        exts.map(|e| e.catalog.len()).unwrap_or(0),
    )
}

// ── 拉取 ───────────────────────────────────────────────────────────────

impl Desktop {
    /// 切进资源页 / 手动刷新：三份 GET 并发拉，各自落地。
    ///
    /// 已经拉过且没强制就不重来 —— 跟 `load_console` 一个口径；右栏没拿到之前
    /// 不该因每帧重拉把引擎打爆。引擎还没连上时只写一句话，不启动轮询。
    pub(crate) fn load_resources(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.resources.loading {
            return;
        }
        if !force && self.resources.hydrated() {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.resources.error = Some("还没连上引擎 —— 去「≡ 更多」里重新连接".into());
            cx.notify();
            return;
        };
        self.resources.loading = true;
        self.resources.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            // 三份并发：一份 500 不该把另两份一起拖成白屏。
            let (skills, prompts, exts) = tokio::join!(
                engine.skills(),
                engine.prompts(),
                engine.extensions(),
            );
            let _ = this.update(cx, |this, cx| {
                this.resources.loading = false;
                let mut errs = Vec::new();
                match skills {
                    Ok(v) => this.resources.skills = Some(v),
                    Err(e) => errs.push(format!("技能：{e}")),
                }
                match prompts {
                    Ok(v) => this.resources.prompts = Some(v),
                    Err(e) => errs.push(format!("提示词：{e}")),
                }
                match exts {
                    Ok(v) => this.resources.exts = Some(v),
                    Err(e) => errs.push(format!("扩展：{e}")),
                }
                if !errs.is_empty() {
                    this.resources.error = Some(errs.join(" · "));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn resource_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.resources.section == section {
            return;
        }
        self.resources.section = section;
        self.resources.selected = None;
        self.load_resources(false, cx);
        cx.notify();
    }

    pub(crate) fn resource_content(&mut self, content: Content, cx: &mut Context<Self>) {
        if self.resources.content == content {
            return;
        }
        self.resources.content = content;
        self.resources.selected = None;
        cx.notify();
    }

    pub(crate) fn resource_cat(&mut self, cat: Option<String>, cx: &mut Context<Self>) {
        let slot = self.resources.slot();
        if self.resources.cat[slot] == cat {
            return;
        }
        self.resources.cat[slot] = cat;
        self.resources.selected = None;
        cx.notify();
    }

    pub(crate) fn resource_select(&mut self, sel: Selected, cx: &mut Context<Self>) {
        self.resources.selected = Some(sel);
        cx.notify();
    }
}

// ── 渲染 ───────────────────────────────────────────────────────────────

fn muted() -> u32 { t().p.muted }
fn line_color() -> u32 { t().p.line }
fn text_color() -> u32 { t().p.text }

/// 资源页整页：树 + 列表 + 详情三栏。
pub(crate) fn render(desk: &Desktop, cx: &mut Context<Desktop>, narrow: bool) -> AnyElement {
    let r = &desk.resources;

    let mut root = div()
        .flex()
        .flex_row()
        .flex_1()
        .h_full()
        .min_w(t().d(0.))
        .bg(rgb(t().p.list_bg));

    if !narrow {
        root = root.child(render_tree(r, cx));
    }
    root = root
        .child(render_list(desk, cx))
        .child(render_detail(desk, cx));
    root.into_any_element()
}

fn render_tree(r: &ResourceState, cx: &mut Context<Desktop>) -> AnyElement {
    let (n_skills, n_prompts, n_ext) = counts(
        r.skills.as_ref(),
        r.prompts.as_ref(),
        r.exts.as_ref(),
    );

    // 段的子开关：内容段里再切「技能 / 提示词」。
    let sub = if r.section == Section::Content {
        let btn = |label: &'static str, n: usize, on: bool, skill: bool| {
            div()
                .id(ElementId::Name(format!("res-sub-{}", if skill { "skills" } else { "prompts" }).into()))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.resource_content(if skill { Content::Skills } else { Content::Prompts }, cx);
                }))
                .flex_1()
                .py(t().d(6.))
                    .text_size(t().fs(11.5))
                .text_color(rgb(if on { text_color() } else { muted() }))
                .when(on, |el| el.bg(rgb(t().p.hover)))
                .child(format!("{label}  {n}"))
        };
        Some(
            div()
                .flex()
                .px(t().d(10.))
                .pt(t().d(6.))
                .gap(t().d(4.))
                .child(btn("技能", n_skills, r.content == Content::Skills, true))
                .child(btn("提示词", n_prompts, r.content == Content::Prompts, false)),
        )
    } else {
        None
    };

    // 类别树：复用 category.rs 的纯函数，不第三次写 count + sort。
    let (cat_nodes, special): (Vec<CatNode>, Vec<(&str, String)>) = match r.section {
        Section::Content => match r.content {
            Content::Skills => {
                let skills = r.skills.as_ref().map(|s| s.skills.as_slice()).unwrap_or(&[]);
                let cats = r.skills.as_ref().map(|s| s.categories.as_slice()).unwrap_or(&[]);
                (
                    build_tree(cats, skills.iter().map(|s| s.category.as_slice())),
                    r.skills.as_ref().map(|s| vec![
                        ("∅", format!("未分类  {}", s.skills.iter().filter(|x| x.category.is_empty() && !x.hidden).count())),
                        ("@hidden", format!("隐藏（模型看不见）  {}", hidden_skills(s).len())),
                    ]).unwrap_or_default(),
                )
            }
            Content::Prompts => {
                let prompts = r.prompts.as_ref().map(|p| p.prompts.as_slice()).unwrap_or(&[]);
                let cats = r.prompts.as_ref().map(|p| p.categories.as_slice()).unwrap_or(&[]);
                (
                    build_tree(cats, prompts.iter().map(|p| p.category.as_slice())),
                    r.prompts.as_ref().map(|p| vec![
                        ("∅", format!("未分类  {}", p.prompts.iter().filter(|x| x.category.is_empty()).count())),
                        ("@default", format!("默认  {}", p.default.as_deref().unwrap_or("—"))),
                    ]).unwrap_or_default(),
                )
            }
        },
        Section::Extensions => {
            let exts = r.exts.as_ref();
            let cats = exts.map(|e| e.categories.as_slice()).unwrap_or(&[]);
            let items: Vec<Vec<String>> = exts
                .map(|e| {
                    e.catalog.iter().map(|x| x.category.clone())
                        .chain(e.archived.iter().map(|x| x.category.clone()))
                        .collect()
                })
                .unwrap_or_default();
            (
                build_tree(cats, items.iter().map(|v| v.as_slice())),
                vec![
                    ("@on", "启用中".into()),
                    ("@bad", "加载失败".into()),
                    ("@builtin", "内置".into()),
                    ("@variant", "变体".into()),
                    ("@archived", "已归档".into()),
                ],
            )
        }
    };

    let seg_btn = |label: &'static str, n: usize, on: bool, act: &'static str, cx: &mut Context<Desktop>| {
        div()
            .id(ElementId::Name(act.into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if act == "res-seg-ext" {
                    this.resource_section(Section::Extensions, cx);
                } else {
                    this.resource_section(Section::Content, cx);
                    this.resource_content(Content::Skills, cx);
                }
            }))
            .flex_1()
            .py(t().d(7.))
            .text_size(t().fs(12.))
            .text_color(rgb(if on { t().p.accent_fg } else { muted() }))
            .bg(rgb(if on { t().p.accent_soft } else { t().p.panel }))
            .child(format!("{label}  {n}"))
    };

    let slot = r.slot();
    let active = r.cat[slot].as_deref();
    let mut tree = div()
        .flex()
        .flex_col()
        .flex_1()
        .px(t().d(8.))
        .py(t().d(6.))
        .gap(t().d(1.))
;

    for (key, label) in &special {
        tree = tree.child(cat_row(label, active == Some(*key), 0, *key, cx));
    }
    for node in &cat_nodes {
        tree = tree.child(cat_row(
            &format!("{}  {}", node.label, node.count),
            active == Some(node.path.as_str()),
            node.depth,
            &node.path,
            cx,
        ));
    }

    div()
        .w(t().d(crate::app::PRESET_TREE_W))
        .h_full()
        .flex()
        .flex_col()
        .border_r(t().d(1.))
        .border_color(rgb(line_color()))
        .bg(rgb(t().p.panel))
        .child(
            div()
                .px(t().d(12.))
                .pt(t().d(12.))
                .pb(t().d(8.))
                .text_size(t().fs(13.))
                .text_color(rgb(text_color()))
                .child("资源"),
        )
        .child(
            div()
                .mx(t().d(10.))
                .flex()
                .gap(t().d(3.))
                .rounded(t().r(7.))
                .bg(rgb(t().p.hover))
                .child(seg_btn("内容", n_skills + n_prompts, r.section == Section::Content, "res-seg-content", cx))
                .child(seg_btn("扩展", n_ext, r.section == Section::Extensions, "res-seg-ext", cx)),
        )
        .children(sub)
        .child(tree)
        .into_any_element()
}

fn cat_row(label: &str, on: bool, depth: usize, key: &str, cx: &mut Context<Desktop>) -> AnyElement {
    let key = key.to_string();
    div()
        .id(ElementId::Name(format!("res-cat-{}", key).into()))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            let next = if this.resources.cat[this.resources.slot()].as_deref() == Some(key.as_str()) {
                None
            } else {
                Some(key.clone())
            };
            this.resource_cat(next, cx);
        }))
        .flex()
        .px(t().d(10.))
        .py(t().d(5.))
        .pl(t().d(10. + depth as f32 * 12.))
        .rounded(t().r(5.))
        .text_size(t().fs(12.))
        .text_color(rgb(if on { t().p.accent_fg } else { text_color() }))
        .when(on, |el| el.bg(rgb(t().p.accent_soft)))
        .hover(|el| el.bg(rgb(t().p.row_hover)))
        .child(label.to_string())
        .into_any_element()
}

fn render_list(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let r = &desk.resources;
    let slot = r.slot();
    let cat = r.cat[slot].as_deref();
    // 搜索词的唯一真相是这个 `InputState`，跟左栏会话搜索一个套路 —— 不再另存一份。
    let q = r.search[slot].read(cx).value().trim().to_string();

    let mut list = div()
        .w(t().d(crate::app::TEMPLATE_LIST_W))
        .h_full()
        .flex()
        .flex_col()
        .border_r(t().d(1.))
        .border_color(rgb(line_color()))
        .bg(rgb(t().p.panel));

    // 搜索框：三段各一份，切段时词还在。suffix 写命中数（空词写总数），
    // 跟左栏会话搜索同一个样式 —— 一行省下来就是列表的 20px。
    let total = match r.section {
        Section::Content => match r.content {
            Content::Skills => r.skills.as_ref().map(|s| s.skills.iter().filter(|x| !x.hidden).count()).unwrap_or(0),
            Content::Prompts => r.prompts.as_ref().map(|p| p.prompts.len()).unwrap_or(0),
        },
        Section::Extensions => r.exts.as_ref().map(|e| e.catalog.len()).unwrap_or(0),
    };
    list = list.child(
        div()
            .px(t().d(10.))
            .pt(t().d(10.))
            .pb(t().d(8.))
            .child(
                Input::new(&r.search[slot])
                    .small()
                    .cleanable(true)
                    .prefix(Icon::new(IconName::Search).text_color(rgb(t().p.faint)).size(t().d(14.)))
                    .suffix(
                        div()
                            .flex_none()
                            .text_size(t().fs(11.))
                            .text_color(rgb(muted()))
                            .child(format!("{total}")),
                    ),
            ),
    );

    if let Some(e) = &r.error {
        list = list.child(
            div()
                .mx(t().d(10.))
                .mb(t().d(6.))
                .px(t().d(8.))
                .py(t().d(6.))
                .rounded(t().r(6.))
                .bg(rgb(t().p.danger_soft))
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.danger_fg))
                .child(e.clone()),
        );
    }

    let mut body = div().flex().flex_col().flex_1();

    match r.section {
        Section::Content => match r.content {
            Content::Skills => {
                if let Some(skills) = r.skills.as_ref() {
                    if cat == Some("@hidden") {
                        for s in hidden_skills(skills) {
                            body = body.child(row_btn(
                                &s.name,
                                s.description.as_deref().unwrap_or(""),
                                "隐藏",
                                false,
                                t().p.muted,
                                Selected::Skill(s.name.clone()),
                                &r.selected,
                                cx,
                            ));
                        }
                    } else {
                        for s in visible_skills(skills, cat, &q) {
                            let bad = !s.loadable();
                            let chip = if bad { "加载失败" } else if s.tracked { "" } else { "未提交" };
                            body = body.child(row_btn(
                                &s.name,
                                s.description.as_deref().unwrap_or(""),
                                chip,
                                bad,
                                if bad { t().p.danger } else { muted() },
                                Selected::Skill(s.name.clone()),
                                &r.selected,
                                cx,
                            ));
                        }
                    }
                } else {
                    body = body.child(loading_line(r.loading));
                }
            }
            Content::Prompts => {
                if let Some(prompts) = r.prompts.as_ref() {
                    for p in visible_prompts(prompts, cat, &q) {
                        let is_default = prompts.default.as_deref() == Some(p.name.as_str());
                        let chip = if is_default { "默认" } else if p.loadable() { "" } else { "加载失败" };
                        body = body.child(row_btn(
                            &p.name,
                            p.summary.as_deref().or(p.description.as_deref()).unwrap_or(""),
                            chip,
                            is_default,
                            if is_default { t().p.accent } else { muted() },
                            Selected::Prompt(p.name.clone()),
                            &r.selected,
                            cx,
                        ));
                    }
                } else {
                    body = body.child(loading_line(r.loading));
                }
            }
        },
        Section::Extensions => {
            if let Some(exts) = r.exts.as_ref() {
                for row in ext_rows(exts, cat, &q) {
                    body = body.child(row_btn(
                        &row.name,
                        row.summary.as_deref().unwrap_or(""),
                        row.kind.label(),
                        row.kind == ExtKind::Failed,
                        row.kind.color(),
                        Selected::Ext(row.name.clone()),
                        &r.selected,
                        cx,
                    ));
                }
            } else {
                body = body.child(loading_line(r.loading));
            }
        }
    }
    list.child(body).into_any_element()
}

fn loading_line(loading: bool) -> AnyElement {
    div()
        .px(t().d(12.))
        .py(t().d(10.))
        .text_size(t().fs(11.))
        .text_color(rgb(muted()))
        .child(if loading { "拉清单…" } else { "没有数据" })
        .into_any_element()
}

fn row_btn(
    name: &str,
    summary: &str,
    chip: &str,
    hot: bool,
    chip_color: u32,
    sel: Selected,
    current: &Option<Selected>,
    cx: &mut Context<Desktop>,
) -> AnyElement {
    let on = current.as_ref() == Some(&sel);
    div()
        .id(ElementId::Name(format!("res-row-{}", match &sel {
            Selected::Skill(n) | Selected::Prompt(n) | Selected::Ext(n) => n,
        }).into()))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.resource_select(sel.clone(), cx);
        }))
        .px(t().d(12.))
        .py(t().d(8.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .bg(rgb(if on { t().p.accent_soft } else { t().p.panel }))
        .hover(|el| el.bg(rgb(t().p.row_hover)))
        .child(
            div()
                .flex()
                .justify_between()
                .child(
                    div()
                        .text_size(t().fs(12.5))
                        .text_color(rgb(text_color()))
                        .child(name.to_string()),
                )
                .when(!chip.is_empty(), |el| el.child(
                    div()
                        .text_size(t().fs(10.))
                        .text_color(rgb(chip_color))
                        .child(chip.to_string()),
                )),
        )
        .child(
            div()
                .pt(t().d(3.))
                .text_size(t().fs(11.))
                .text_color(rgb(muted()))
                .line_height(t().d(16.))
                .child(summary.to_string()),
        )
        .into_any_element()
}

fn render_detail(desk: &Desktop, _cx: &mut Context<Desktop>) -> AnyElement {
    let r = &desk.resources;
    let mut body = div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .px(t().d(22.))
        .py(t().d(18.))
        .gap(t().d(10.))
;

    let Some(sel) = &r.selected else {
        body = body.child(
            div()
                .text_size(t().fs(12.))
                .text_color(rgb(muted()))
                .child("选一行看详情。第一刀只读 —— 发布 / 归档 / 编辑文件留下一刀。"),
        );
        return body.into_any_element();
    };

    match sel {
        Selected::Skill(name) => {
            if let Some(skill) = r.skills.as_ref().and_then(|s| s.skills.iter().find(|x| &x.name == name)) {
                body = body
                    .child(detail_title(name))
                    .child(detail_kv("描述", skill.description.as_deref().unwrap_or("—")))
                    .child({
                        let cats = if skill.category.is_empty() { "未分类".to_string() } else { skill.category.join(" · ") };
                        detail_kv("类别", &cats)
                    })
                    .child(detail_kv("状态", if skill.loadable() { "可加载" } else { "加载失败" }))
                    .child(detail_kv("出处", if skill.tracked { "仓库里（已提交）" } else { "未提交" }));
                if skill.hidden {
                    body = body.child(note_line("隐藏：模型看不见它，勾上也不进提示词。"));
                }
            }
        }
        Selected::Prompt(name) => {
            if let Some(p) = r.prompts.as_ref().and_then(|p| p.prompts.iter().find(|x| &x.name == name)) {
                body = body
                    .child(detail_title(name))
                    .child(detail_kv("摘要", p.summary.as_deref().or(p.description.as_deref()).unwrap_or("—")))
                    .child({
                        let cats = if p.category.is_empty() { "未分类".to_string() } else { p.category.join(" · ") };
                        detail_kv("类别", &cats)
                    })
                    .child(detail_kv("状态", if p.loadable() { "可加载" } else { "加载失败" }));
                if r.prompts.as_ref().and_then(|p| p.default.as_deref()) == Some(name.as_str()) {
                    body = body.child(note_line("这是仓库默认值（`settings.json` 的 `default`）—— 只用来预勾，不是「用户选的那条」。"));
                }
            }
        }
        Selected::Ext(name) => {
            let found = r.exts.as_ref().and_then(|exts| {
                let row = ext_rows(exts, None, "");
                row.into_iter().find(|x| &x.name == name)
            });
            if let Some(e) = found {
                body = body
                    .child(detail_title(name))
                    .child(detail_kv("类别", e.kind.label()))
                    .child(detail_kv("摘要", e.summary.as_deref().unwrap_or("—")))
                    .child(note_line("装 / 卸 / 归档是三件不同的事；核心三条（nous / skills / prompts）卸载会被引擎 403 挡回来。"));
            }
        }
    }
    body.into_any_element()
}

fn detail_title(name: &str) -> AnyElement {
    div()
        .text_size(t().fs(17.))
        .text_color(rgb(text_color()))
        .child(name.to_string())
        .into_any_element()
}

fn detail_kv(k: &str, v: &str) -> AnyElement {
    div()
        .flex()
        .gap(t().d(12.))
        .text_size(t().fs(12.))
        .child(
            div()
                .w(t().d(56.))
                .text_color(rgb(muted()))
                .child(k.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_color(rgb(text_color()))
                .child(v.to_string()),
        )
        .into_any_element()
}

fn note_line(text: &str) -> AnyElement {
    div()
        .mt(t().d(8.))
        .px(t().d(10.))
        .py(t().d(8.))
        .rounded(t().r(6.))
        .bg(rgb(t().p.hover))
        .text_size(t().fs(11.))
        .text_color(rgb(muted()))
        .child(text.to_string())
        .into_any_element()
}

