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

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::types::{
    ExtCategories, Extensions, Prompt, Prompts, RepoArchive, RepoVariants, Skill, Skills, TrashList,
};

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
pub(crate) enum Selected {
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
    /// 「清单是 TS 时代的 JS」这件事本身，与档位正交：`nous` / `skills` / `prompts`
    /// 三条核心里装的还是老 JS 插件，它们**是内置的**，只是跑不起来。
    /// 以前把换代当档位（legacy 压过 core），于是真机上「内置 0 条、待重写 4 条」——
    /// 看着像引擎没带核心扩展，其实带了两件事：内置于它，待重写于它们。
    pub(crate) legacy: bool,
}

impl ExtRow {
    /// 行尾那个 chip：说什么 + 用什么颜色 + 要不要当危险色画。
    pub(crate) fn chip(&self) -> (&'static str, u32, bool) {
        if self.legacy {
            return (ExtKind::Legacy.label(), t().p.danger, true);
        }
        (self.kind.label(), self.kind.color(), self.kind == ExtKind::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtKind {
    /// 仓库里、本机装了、加载正常。
    On,
    /// 仓库里但本机没装。
    Off,
    /// 加载失败（引擎给了 `load.state == "failed"`，或 `items[].kind == "invalid"`）。
    Failed,
    /// 引擎自带的，不在仓库里。
    Builtin,
    /// 本机 checkout 的变体（`<本体>@<id>`）。
    Variant,
    /// `.archive/` 里，别人也看不见。
    Archived,
    /// TS 时代（`qrd3wh`）的 JS 插件：清单都不合法，`index.js` 在新运行时里没有解释器。
    ///
    /// 这档不是「坏了」——是**换代**：装 / 卸都救不了，只能按 `nous-ext-sdk` 重写。
    /// 把它涂成「加载失败」会让人反复点「重装」，每点一次都是同一个错。
    Legacy,
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
            Self::Legacy => "待重写（旧 JS）",
        }
    }

    fn color(self) -> u32 {
        match self {
            Self::On => t().p.accent,
            Self::Failed | Self::Legacy => t().p.danger,
            _ => t().p.muted,
        }
    }
}

/// 这一页的全部状态。**只有这一页持有**，不塞进 `Desktop`。
pub(crate) struct ResourceState {
    pub(crate) section: Section,
    pub(crate) content: Content,
    pub(crate) skills: Option<Skills>,
    pub(crate) prompts: Option<Prompts>,
    pub(crate) exts: Option<Extensions>,
    /// 变体绑定表（`GET /v1/repos/extend/variants`，Nous #76）。
    pub(crate) ext_variants: Option<RepoVariants>,
    /// 归档区（`GET /v1/repos/extend/archive`）：`.archive/` 里有什么。
    pub(crate) ext_archive: Option<RepoArchive>,
    /// 类别词表（`GET /v1/extensions/categories`）。
    pub(crate) ext_cats: Option<ExtCategories>,
    /// 垃圾站（`GET /v1/extensions/trash`）：卸载过、还没真删的源码目录。
    /// 没有它，「卸载」和「扔掉」在界面上是同一个按钮 —— 点了就再也找不回来。
    pub(crate) ext_trash: Option<TrashList>,
    /// 三个 tab 各自的类别筛选，切来切去能回到原来那一行。
    cat: [Option<String>; 3],
    /// 三段各自的搜索框。**各存一份**，切段时词还在（跟 `cat` 同款语义）。
    /// 用 `InputState`（不是裸 String）才有中文输入法 / 剪贴板 / 一键清空 —— 这些是
    /// 第一屏体验，跟左栏会话搜索同一套组件。
    search: [Entity<InputState>; 3],
    pub(crate) selected: Option<Selected>,
    loading: bool,
    error: Option<String>,

    // ── 写操作（第二刀）─────────────────────────────────────────────
    /// 当前是浏览 / 新建 / 编辑 / 发布。四个视图**互斥**，都占满列表 + 详情那一块，
    /// 不叠在列表上面 —— 「编辑是整页工作台，不是右栏抽屉」。
    pub(crate) view: crate::resource_write::View,
    pub(crate) form: Option<crate::resource_write::NewForm>,
    /// 变体表单（开变体 / 改分支）。与 `form` 互斥，跟 `view` 一起活着。
    pub(crate) variant: Option<crate::resource_write::VariantForm>,
    /// 改类别表单。
    pub(crate) category: Option<crate::resource_write::CategoryForm>,
    /// 类别词表管理（增 / 改名 / 删）。
    pub(crate) cats: Option<crate::resource_write::CategoriesForm>,
    pub(crate) edit: Option<crate::resource_write::EditSession>,
    pub(crate) publish: crate::resource_write::PublishState,
    /// 归档 / 真删 / 丢掉改动 的确认层。不可逆的动作必须先过这一步。
    pub(crate) confirm: Option<crate::resource_write::Confirm>,
    /// 表单 / 编辑 / 发布这三个视图自己的输入订阅，跟着资源页一起活。
    pub(crate) write_subs: Vec<Subscription>,
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
            ext_variants: None,
            ext_archive: None,
            ext_cats: None,
            ext_trash: None,
            cat: [None, None, None],
            search: [
                box_for("搜技能：名字 / 描述"),
                box_for("搜提示词：名字 / 摘要"),
                box_for("搜扩展：名字 / 摘要"),
            ],
            selected: None,
            loading: false,
            error: None,
            view: crate::resource_write::View::Browse,
            form: None,
            variant: None,
            category: None,
            cats: None,
            edit: None,
            publish: Default::default(),
            confirm: None,
            write_subs: Vec::new(),
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

    /// 扩展页的三份数据合成一个视图。没拉到清单就没有视图（其余两份单独来也没用）。
    /// 当前选中那条的名字（三段的选中都是名字，不是下标）。
    pub(crate) fn selected_name(&self) -> String {
        match self.selected.as_ref() {
            Some(Selected::Ext(n)) | Some(Selected::Skill(n)) | Some(Selected::Prompt(n)) => n.clone(),
            None => String::new(),
        }
    }

    pub(crate) fn ext_view(&self) -> Option<ExtView<'_>> {
        self.exts.as_ref().map(|exts| ExtView {
            exts,
            variants: self.ext_variants.as_ref(),
            archive: self.ext_archive.as_ref(),
        })
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
///
/// **引擎 0.1.9 起给的是 `items[]`**（`{name, version, kind, category, summary,
/// loaded, pid, state, error}`）：它只说「盘上这一条现在是什么状态」，不再分
/// catalog / builtin / variants / archived 五份。所以有 `items` 就用 `items`，
/// 老形状只在拿不到 `items` 时兜底 —— 两条路只留一个真相，不合并两套再猜。
/// 扩展页要的三份数据：运行时清单 + 变体绑定表 + 归档区。
///
/// 三者不是一份响应：`GET /v1/extensions` 说的是「进程现在怎么样」，
/// `/v1/repos/extend/{variants,archive}` 说的是「仓库里还有什么」。以前 TS 内核把
/// 它们塞在一份响应里，现在分开了 —— 合成在这里做，别让渲染层知道有三份。
#[derive(Clone, Copy)]
pub(crate) struct ExtView<'a> {
    pub(crate) exts: &'a Extensions,
    pub(crate) variants: Option<&'a RepoVariants>,
    pub(crate) archive: Option<&'a RepoArchive>,
}

impl<'a> ExtView<'a> {
    fn variant_count(&self) -> usize {
        self.variants.map(|v| v.family("extend").count()).unwrap_or(0)
    }

    fn archived_count(&self) -> usize {
        self.archive.map(|a| a.items.len()).unwrap_or(0)
    }

    /// 变体行的名字：绑定表按 `<id>` 记账，不记「本体是谁」—— 本体看详情里的分支。
    fn variant_rows(&self) -> Vec<ExtRow> {
        let Some(variants) = self.variants else { return Vec::new() };
        variants
            .family("extend")
            .map(|(id, binding)| ExtRow {
                name: id.clone(),
                summary: Some(if binding.branch.is_empty() {
                    "extend 的变体".to_string()
                } else {
                    format!("变体分支 {}", binding.branch)
                }),
                category: vec![],
                kind: ExtKind::Variant,
                legacy: false,
            })
            .collect()
    }

    fn archived_rows(&self) -> Vec<ExtRow> {
        let Some(archive) = self.archive else { return Vec::new() };
        archive
            .items
            .iter()
            .map(|unit| ExtRow {
                name: unit.name.clone(),
                // 归档物只有名字：`.archive/` 里没有清单、没有版本。
                summary: Some("已归档（在 .archive/ 里，恢复回来才知道是什么）".to_string()),
                category: vec![],
                kind: ExtKind::Archived,
                legacy: false,
            })
            .collect()
    }
}

pub(crate) fn ext_rows(src: ExtView<'_>, cat: Option<&str>, q: &str) -> Vec<ExtRow> {
    let exts = src.exts;
    let mut out = Vec::new();
    if !exts.items.is_empty() {
        for item in &exts.items {
            // 判档顺序有讲究：**核心先判** —— 引擎自带的就该在「内置」里，
            // 哪怕它装的是老 JS（那是 chip 上的一句「待重写」，不是另一个档）。
            // 然后才是换代 / 清单不合法 / 进程在不在。
            let legacy = item.kind == "legacy-js";
            let kind = if item.core {
                ExtKind::Builtin
            } else if legacy {
                ExtKind::Legacy
            } else if item.kind == "absent" {
                // 卸载过的：目录不在源码根里（搬进了 trash），但名字还在删除名单上。
                // 内核 0.1.16 起 `items` 会把它们列成 `absent` —— 不然「安装」没有入口。
                ExtKind::Off
            } else if item.kind == "invalid" {
                ExtKind::Failed
            } else if item.state.as_deref() == Some("running") {
                ExtKind::On
            } else {
                ExtKind::Failed
            };
            out.push(ExtRow {
                name: item.name.clone(),
                // 清单不合法时没有 summary，引擎给的 error 才是人能看懂的那句话。
                summary: item.summary.clone().or_else(|| item.error.clone()),
                // 新形状里 category 是一个字符串（清单里就是 Option<String>）。
                category: item.category.clone().into_iter().collect(),
                kind,
                legacy,
            });
        }
        out.extend(src.variant_rows());
        out.extend(src.archived_rows());
        return filter_ext_row(out, cat, q);
    }
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
            legacy: false,
        });
    }
    for name in &exts.builtin {
        out.push(ExtRow {
            name: name.clone(),
            summary: None,
            category: vec![],
            kind: ExtKind::Builtin,
            legacy: false,
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
                legacy: false,
            });
        }
    }
    for a in &exts.archived {
        out.push(ExtRow {
            name: a.name.clone(),
            summary: a.summary.clone(),
            category: a.category.clone(),
            kind: ExtKind::Archived,
            legacy: false,
        });
    }
    filter_ext_row(out, cat, q)
}

/// 这一条还能不能走**文件面**（`/v1/{kind}/{file,file,publish,create}`）。
///
/// 文件面认的是源码根里的目录名。已归档的住在 `.archive/`，旧 JS 插件虽然还在根里
/// 但连清单都不合法 —— 两档都不该挂「编辑 / 发布 / 复制」：点下去只会 404，
/// 而 404 长得像「没有这个扩展」，查起来最费劲。
pub(crate) fn file_backed(kind: Option<ExtKind>) -> bool {
    // 认的是源码根里**真有这个目录**：已归档在 `.archive/`、变体在另一棵 worktree、
    // 本机没装（`Off`，目录被搬进 trash）三档按名字找都是 404 —— 都收着不给。
    // 清单不合法（`Failed`）反而要给：目录在，编辑就是修清单那条路。
    !matches!(
        kind,
        Some(ExtKind::Archived)
            | Some(ExtKind::Legacy)
            | Some(ExtKind::Variant)
            | Some(ExtKind::Off)
    )
}

/// 列表的筛选：状态桶（`@on` / `@bad` / …）+ 类别 + 搜索词。
///
/// 从 `ext_rows` 拆出来：两种数据形状（`items[]` 与老的五份）走同一条筛选，
/// 分开写就会出现「一边能搜、一边不能」。
fn filter_ext_row(out: Vec<ExtRow>, cat: Option<&str>, q: &str) -> Vec<ExtRow> {
    let q_lower = q.trim().to_lowercase();
    out.into_iter()
        .filter(|r| match cat {
            None => true,
            Some("@on") => r.kind == ExtKind::On,
            Some("@bad") => r.kind == ExtKind::Failed,
            Some("@builtin") => r.kind == ExtKind::Builtin,
            Some("@variant") => r.kind == ExtKind::Variant || r.kind == ExtKind::Failed,
            Some("@archived") => r.kind == ExtKind::Archived,
            Some("@legacy") => r.kind == ExtKind::Legacy,
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
///
/// 扩展的计数只数**能看见的那一份**：有 `items` 就数 `items`（盘上真相），
/// 没有才退回老形状的 `catalog`。两个都数是「段写 4、列表 7」的老毛病。
pub(crate) fn counts(skills: Option<&Skills>, prompts: Option<&Prompts>, ext: Option<ExtView<'_>>) -> (usize, usize, usize) {
    (
        skills.map(|s| s.skills.iter().filter(|x| !x.hidden).count()).unwrap_or(0),
        prompts.map(|p| p.prompts.len()).unwrap_or(0),
        ext.map(|v| {
            if v.exts.items.is_empty() {
                v.exts.catalog.len()
            } else {
                // 段上那个数字 = 列表里能看到的行数（清单 + 变体 + 归档）。
                v.exts.items.len() + v.variant_count() + v.archived_count()
            }
        })
        .unwrap_or(0),
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
            // 六份并发：一份 500 不该把另两份一起拖成白屏。扩展那三份同理 ——
            // 变体 / 归档端点旧引擎没有（404），不能因此把扩展清单本身也画成空。
            let (skills, prompts, exts, variants, archive, cats, trash) = tokio::join!(
                engine.skills(),
                engine.prompts(),
                engine.extensions(),
                engine.ext_variants(),
                engine.ext_archive(),
                engine.ext_categories(),
                engine.ext_trash(),
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
                // 这三份失败不进 `error`（页面照常可看），但也不许各自编个默认值装作成功：
                // 没拿到就是 `None`，视图里那一档自然空着。
                this.resources.ext_variants = variants.ok();
                this.resources.ext_archive = archive.ok();
                this.resources.ext_cats = cats.ok();
                this.resources.ext_trash = trash.ok();
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
    use crate::resource_write::View;

    // 新建 / 编辑 / 发布都是**整页**：占满列表 + 详情那一块，不叠在列表上面。
    // 浏览才是三栏。四个视图互斥，同一时刻只有一个在画。
    let body = match desk.resources.view {
        View::New => crate::resource_write::render_new(desk, cx),
        View::Edit => crate::resource_write::render_edit(desk, cx),
        View::Publish => crate::resource_write::render_publish(desk, cx),
        View::Variant => crate::resource_write::render_variant(desk, cx),
        View::Category => crate::resource_write::render_category(desk, cx),
        View::Categories => crate::resource_write::render_categories(desk, cx),
        View::Trash => crate::resource_write::render_trash(desk, cx),
        View::Browse => {
            let mut root = div().flex().flex_row().flex_1().h_full().min_w(t().d(0.));
            if !narrow {
                root = root.child(render_tree(&desk.resources, cx));
            }
            root.child(render_list(desk, cx))
                .child(render_detail(desk, cx))
                .into_any_element()
        }
    };

    // 确认层压在整页上面（归档 / 恢复 / 真删 / 丢掉改动）。
    // 它要盖住整块内容，所以只在有确认时套一层壳 —— 平时不多一层 flex，少一个
    // 「空的 flex_1 抢宽度」的机会（坑.md 64/94）。
    if desk.resources.confirm.is_some() {
        return div()
            .relative()
            .flex()
            .flex_row()
            .flex_1()
            .h_full()
            .min_w(t().d(0.))
            .child(body)
            .child(crate::resource_write::render_confirm(desk, cx))
            .into_any_element();
    }
    body
}

fn render_tree(r: &ResourceState, cx: &mut Context<Desktop>) -> AnyElement {
    let (n_skills, n_prompts, n_ext) = counts(
        r.skills.as_ref(),
        r.prompts.as_ref(),
        r.ext_view(),
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
            // 类别树要的是「这个仓定义了哪几个类别」：优先取词表
            // （`GET /v1/extensions/categories`，住在私库里、随发布走），没有再退回
            // 老形状的 `Extensions.categories`。一个类别下暂时没条目也要留在树上 ——
            // 只按条目凑，空类别会悄悄消失。
            let cats: &[String] = r
                .ext_cats
                .as_ref()
                .map(|c| c.categories.as_slice())
                .filter(|c| !c.is_empty())
                .or_else(|| exts.map(|e| e.categories.as_slice()))
                .unwrap_or(&[]);
            // 每条一个类别列表，交给 `build_tree` 去数。
            let items: Vec<Vec<String>> = exts
                .map(|e| {
                    if e.items.is_empty() {
                        e.catalog.iter().map(|x| x.category.clone())
                            .chain(e.archived.iter().map(|x| x.category.clone()))
                            .collect()
                    } else {
                        e.items
                            .iter()
                            .map(|x| x.category.clone().into_iter().collect())
                            .collect()
                    }
                })
                .unwrap_or_default();
            (
                build_tree(cats, items.iter().map(|v| v.as_slice())),
                vec![
                    ("@on", "启用中".into()),
                    ("@bad", "加载失败".into()),
                    ("@legacy", "待重写".into()),
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
    // 垃圾站单独一行：它不是一个筛选，是另一页（卸载过的东西不在清单里，
    // 混进类别树会被当成「另一个类别」）。
    if r.section == Section::Extensions {
        tree = tree.child(trash_row(r, cx));
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

/// 垃圾站那一行。数字是「有几份」——同一扩展卸载过两次就是两份，别拿扩展数糊弄。
fn trash_row(r: &ResourceState, cx: &mut Context<Desktop>) -> AnyElement {
    let n = r.ext_trash.as_ref().map(|t| t.items.len()).unwrap_or(0);
    let on = r.view == crate::resource_write::View::Trash;
    // **空着也显示**：它是「另一页」的入口，不是筛选。藏起来的时候人只会觉得
    // 「这版本没做垃圾站」——2026-11-23 在真机上就被这么问过。
    div()
        .id("res-trash")
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
            this.resource_open_trash(window, cx);
        }))
        .flex()
        .px(t().d(10.))
        .py(t().d(5.))
        .mt(t().d(6.))
        .rounded(t().r(5.))
        .text_size(t().fs(12.))
        .text_color(rgb(if on { t().p.accent_fg } else { muted() }))
        .when(on, |el| el.bg(rgb(t().p.accent_soft)))
        .hover(|el| el.bg(rgb(t().p.row_hover)))
        .child(format!("垃圾站  {n}"))
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

    // 「+ 新建」跟着当前段走：在技能段建技能、在扩展段建扩展。
    let new_kind = match (r.section, r.content) {
        (Section::Extensions, _) => crate::resource_write::Kind::Ext,
        (Section::Content, Content::Skills) => crate::resource_write::Kind::Skill,
        (Section::Content, Content::Prompts) => crate::resource_write::Kind::Prompt,
    };
    list = list.child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(t().d(12.))
            .pt(t().d(10.))
            .child(
                div()
                    .text_size(t().fs(12.))
                    .text_color(rgb(text_color()))
                    .child(match r.section {
                        Section::Content => r.content.label(),
                        Section::Extensions => "扩展",
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    // 「重载」只在扩展段：技能 / 提示词没有进程，改完文件即生效。
                    // 扩展是盘上目录 + 运行中的进程，手工放了文件之后要喊一次才对得上。
                    .when(r.section == Section::Extensions, |el| {
                        el.child(
                            div()
                                .id("res-ext-reload")
                                .flex()
                                .items_center()
                                .h(t().d(24.))
                                .px(t().d(9.))
                                .rounded(t().r(6.))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.line))
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.text_2))
                                .hover(|el| el.bg(rgb(t().p.hover)))
                                .child("重载")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.resource_ext_reload(window, cx);
                                })),
                        )
                    })
                    // 词表管理：改的是「有哪些类别」，不是某条扩展挂哪个类别。
                    .when(r.section == Section::Extensions, |el| {
                        el.child(
                            div()
                                .id("res-ext-cats")
                                .flex()
                                .items_center()
                                .h(t().d(24.))
                                .px(t().d(9.))
                                .rounded(t().r(6.))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.line))
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.text_2))
                                .hover(|el| el.bg(rgb(t().p.hover)))
                                .child("类别")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.resource_open_categories(window, cx);
                                })),
                        )
                    })
                    .child(
                        div()
                            .id("res-new")
                            .flex()
                            .items_center()
                            .h(t().d(24.))
                            .px(t().d(9.))
                            .rounded(t().r(6.))
                            .bg(rgb(t().p.accent))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.on_accent))
                            .hover(|el| el.bg(rgb(t().p.accent_hover)))
                            .child("+ 新建")
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.resource_new(new_kind, window, cx);
                            })),
                    ),
            ),
    );

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
                            // `tracked` 三态：`None` = 引擎不查 git，不挂 chip（不撒谎）。
                            let chip = if bad {
                                "加载失败"
                            } else if s.tracked == Some(false) {
                                "未提交"
                            } else {
                                ""
                            };
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
                    body = body.child(placeholder(r, RowKind::Prompts, &q, cat, cx));
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
                    body = body.child(placeholder(r, RowKind::Skills, &q, cat, cx));
                }
            }
        },
        Section::Extensions => {
            if let Some(view) = r.ext_view() {
                for row in ext_rows(view, cat, &q) {
                    let (chip, chip_color, chip_danger) = row.chip();
                    body = body.child(row_btn(
                        &row.name,
                        row.summary.as_deref().unwrap_or(""),
                        chip,
                        chip_danger,
                        chip_color,
                        Selected::Ext(row.name.clone()),
                        &r.selected,
                        cx,
                    ));
                }
            } else {
                body = body.child(placeholder(r, RowKind::Exts, &q, cat, cx));
            }
        }
    }
    list.child(body).into_any_element()
}

/// 哪一段在画占位（决定「在等什么」那句话）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Skills,
    Prompts,
    Exts,
}

impl RowKind {
    fn waiting(self) -> &'static str {
        match self {
            Self::Skills => "正在等引擎回技能清单",
            Self::Prompts => "正在等引擎回提示词清单",
            Self::Exts => "正在等引擎回扩展清单",
        }
    }
}

/// 这一段没有行可画时的三态。**空 / 在等 / 坏了** 由 [`crate::states`] 统一画，
/// 这里只负责把「为什么」说对：没拉到 ≠ 拉到了但是空的 ≠ 被筛掉。
fn placeholder(
    r: &ResourceState,
    kind: RowKind,
    q: &str,
    cat: Option<&str>,
    cx: &mut Context<Desktop>,
) -> AnyElement {
    // 顺序有讲究：先判失败，再判在等，最后才是空 —— 反了会把「坏了」画成「在等」。
    if let Some(e) = r.error.as_deref() {
        // 失败一定能重拉：没有重试的失败只能靠切走再切回，那是把 bug 当操作习惯。
        return crate::states::view(
            crate::states::Phase::failed(e),
            Some(crate::states::retry_button(
                "resources-retry",
                "重新拉一次",
                cx.listener(|this, _: &ClickEvent, _, cx| this.load_resources(true, cx)),
            )),
        );
    }
    if r.loading {
        return crate::states::inline(crate::states::Phase::loading(kind.waiting()));
    }
    let filtered = !q.is_empty() || cat.is_some();
    if filtered {
        // 有数据但被筛掉了 —— 文案跟「真的一条都没有」必须不一样，否则用户会以为库空了。
        return crate::states::inline(crate::states::Phase::empty(
            "这一筛没有命中",
            Some("把关键字清掉，或者回到「全部」类别"),
        ));
    }
    let (head, why) = match kind {
        RowKind::Skills => (
            "引擎里还没有技能",
            "私库里放一个带 SKILL.md 的目录，重新拉一次就在这里",
        ),
        RowKind::Prompts => ("引擎里还没有提示词", "私库里放一个 .md，重新拉一次就在这里"),
        RowKind::Exts => ("这个引擎没有扩展", "发行版没装 ext 模块时这里一直是空的"),
    };
    crate::states::inline(crate::states::Phase::empty(head, Some(why)))
}

fn row_btn(
    name: &str,
    summary: &str,
    chip: &str,
    _hot: bool,
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

fn render_detail(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    use crate::resource_write::{ConfirmAct, Kind};

    let r = &desk.resources;
    let mut body = div()
        .id("res-detail-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w(t().d(0.))
        .bg(rgb(t().p.panel))
        .px(t().d(22.))
        .py(t().d(18.))
        .gap(t().d(10.));

    let Some(sel) = &r.selected else {
        body = body.child(
            div()
                .text_size(t().fs(12.))
                .text_color(rgb(muted()))
                .child("选一行看详情。新建 / 编辑 / 发布都在这一页，不用切到别的窗口。"),
        );
        return body.into_any_element();
    };

    let kind = sel.kind_of();
    let name = sel.name_of().to_string();
    // Desk#13 返工：等票期间写端点灰着，理由逐字取自 capabilities（capability.rs 单点）。
    if let Some(reason) = desk.endpoint_disabled_reason("POST", &format!("/v1/{}/publish", kind.repo().key())) {
        body = body.child(note_line(&reason));
    }

    // ── 头部：名字 + 状态 + 动作。动作按条目种类分，不是一套按钮硬套。 ──
    let mut head = div()
        .flex()
        .items_start()
        .gap(t().d(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t().d(3.))
                .flex_1()
                .min_w(t().d(0.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t().d(8.))
                        .child(detail_title(&name))
                        .when(
                            crate::resource_write::is_untracked(kind, &name, desk),
                            |el| {
                                el.child(
                                    div()
                                        .px(t().d(6.))
                                        .py(t().d(1.))
                                        .rounded(t().r(5.))
                                        .bg(rgb(t().p.warn_soft))
                                        .text_size(t().fs(10.5))
                                        .text_color(rgb(t().p.warn))
                                        .child("未发布"),
                                )
                            },
                        ),
                )
                .child(
                    div()
                        .text_size(t().fs(11.5))
                        .text_color(rgb(muted()))
                        .child(format!("{}:{}", kind.label(), name)),
                ),
        );

    // 扩展的五类各有各的动作：已归档给「恢复 / 真删」，在册的给「卸载 / 安装」+「归档」。
    // 「编辑」和「发布」走的是**文件面**（`/v1/{kind}/file`），只对源码根里真有的目录成立 ——
    // 已归档（在 `.archive/`）和旧 JS 插件点进去只会 404，所以先算档位再决定挂不挂。
    let ext_row = if kind == Kind::Ext {
        r.ext_view()
            .and_then(|v| ext_rows(v, None, "").into_iter().find(|x| x.name == name))
    } else {
        None
    };
    let ext_kind = ext_row.as_ref().map(|x| x.kind);
    // 「改类别」要把现值填进表单：类别是清单里那一行字符串（`items[].category`）。
    let ext_cat = ext_row
        .as_ref()
        .map(|x| x.category.join(" · "))
        .unwrap_or_default();
    // `None` = 不是扩展段，照旧全给（技能 / 提示词没有这些档位）。
    let file_backed = file_backed(ext_kind);

    let mut acts = div().flex().items_center().gap(t().d(6.));
    if file_backed {
        acts = acts.child(
        div()
            .id("res-act-edit")
            .flex()
            .items_center()
            .justify_center()
            .h(t().d(28.))
            .px(t().d(12.))
            .rounded(t().r(7.))
            .bg(rgb(t().p.accent))
            .text_size(t().fs(11.5))
            .text_color(rgb(t().p.on_accent))
            .hover(|el| el.bg(rgb(t().p.accent_hover)))
            .child("编辑")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.resource_open_edit(window, cx);
            })),
        );
    }
    if ext_kind == Some(ExtKind::Variant) {
        // 变体自己的三个动作（`/v1/repos/extend/variants/*`）：发布 / 解绑 / 清理。
        // 它不在源码根里，所以没有「编辑 / 卸载」，也没有 git 之外的路子。
        // 「改分支」要填新分支名，走表单不走确认层。
        acts = acts.child(
            div()
                .id("res-act-vbranch")
                .flex()
                .items_center()
                .justify_center()
                .h(t().d(28.))
                .px(t().d(10.))
                .rounded(t().r(7.))
                .bg(rgb(t().p.surface))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.text_2))
                .hover(|el| el.opacity(0.85))
                .child("改分支")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let name = this.resources.selected_name();
                    this.resource_open_variant(
                        crate::resource_write::VariantMode::Branch,
                        name,
                        window,
                        cx,
                    );
                })),
        );
        for (id, label, act, danger) in [
            ("res-act-vpub", "发布变体", ConfirmAct::PublishVariant, false),
            ("res-act-vunbind", "解绑", ConfirmAct::Unbind, false),
            ("res-act-vprune", "清理", ConfirmAct::Prune, true),
        ] {
            let (bg, fg) = if danger {
                (t().p.danger_soft, t().p.danger)
            } else {
                (t().p.surface, t().p.text_2)
            };
            acts = acts.child(
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(t().d(28.))
                    .px(t().d(10.))
                    .rounded(t().r(7.))
                    .bg(rgb(bg))
                    .text_size(t().fs(11.5))
                    .text_color(rgb(fg))
                    .hover(|el| el.opacity(0.85))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.resource_ask_confirm(act, cx);
                    })),
            );
        }
    }
    if ext_kind == Some(ExtKind::Archived) {
        acts = acts.child(
            div()
                .id("res-act-restore")
                .flex()
                .items_center()
                .justify_center()
                .h(t().d(28.))
                .px(t().d(10.))
                .rounded(t().r(7.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.text_2))
                .hover(|el| el.bg(rgb(t().p.hover)))
                .child("恢复")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.resource_ask_confirm(ConfirmAct::Restore, cx);
                })),
        );
        acts = acts.child(
            div()
                .id("res-act-purge")
                .flex()
                .items_center()
                .justify_center()
                .h(t().d(28.))
                .px(t().d(10.))
                .rounded(t().r(7.))
                .bg(rgb(t().p.danger_soft))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.danger))
                .hover(|el| el.bg(rgb(t().p.danger_soft)).opacity(0.85))
                .child("真删")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.resource_ask_confirm(ConfirmAct::Purge, cx);
                })),
        );
    } else {
        // 装 / 卸 / 归档都认源码根里的目录名。变体是另一棵 worktree、已归档在
        // `.archive/`、旧 JS 的连清单都不合法 —— 三档给了按钮也是 404，
        // 而 404 看着像「没有这个扩展」。它们各有各的入口（见上面的分支）。
        if let Some(k) = ext_kind
            && k != ExtKind::Builtin
            && k != ExtKind::Legacy
            && k != ExtKind::Variant
            && k != ExtKind::Archived
        {
            // 改类别走清单（`POST /v1/extensions/category`），不需要 git，
            // 所以源码根里的档位都给，包括本机没装的（它的清单还在仓库里）。
            let cat_now = if ext_cat.is_empty() { None } else { Some(ext_cat.clone()) };
            acts = acts.child(
                div()
                    .id("res-act-category")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(t().d(28.))
                    .px(t().d(10.))
                    .rounded(t().r(7.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .text_size(t().fs(11.5))
                    .text_color(rgb(t().p.text_2))
                    .hover(|el| el.bg(rgb(t().p.hover)))
                    .child("改类别")
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let name = this.resources.selected_name();
                        this.resource_open_category(name, cat_now.clone(), window, cx);
                    })),
            );
            let installed = k != ExtKind::Off;
            acts = acts.child(
                div()
                    .id("res-act-toggle")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(t().d(28.))
                    .px(t().d(10.))
                    .rounded(t().r(7.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .text_size(t().fs(11.5))
                    .text_color(rgb(t().p.text_2))
                    .hover(|el| el.bg(rgb(t().p.hover)))
                    .child(if installed { "卸载" } else { "安装" })
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.resource_ext_toggle(!installed, window, cx);
                    })),
            );
        }
        // 归档认的也是源码根里的目录名：变体 / 已归档 / 旧 JS / 核心四条都不该给这个按钮
        // （核心归档了引擎自己就少一块，变体与归档区的名字 git mv 也找不到）。
        let can_archive = match ext_kind {
            None => true,
            Some(k) => k == ExtKind::On,
        };
        if can_archive {
        acts = acts.child(
            div()
                .id("res-act-archive")
            .flex()
            .items_center()
            .justify_center()
            .h(t().d(28.))
            .px(t().d(10.))
            .rounded(t().r(7.))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .text_size(t().fs(11.5))
            .text_color(rgb(t().p.text_2))
            .hover(|el| el.bg(rgb(t().p.hover)))
            .child("归档")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.resource_ask_confirm(ConfirmAct::Archive, cx);
            })),
        );
        }
    }
    if kind == Kind::Ext && file_backed {
        acts = acts.child(
            div()
                .id("res-act-variant")
                .flex()
                .items_center()
                .justify_center()
                .h(t().d(28.))
                .px(t().d(10.))
                .rounded(t().r(7.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.text_2))
                .hover(|el| el.bg(rgb(t().p.hover)))
                .child("开变体")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let name = this.resources.selected_name();
                    this.resource_open_variant(
                        crate::resource_write::VariantMode::Bind,
                        name,
                        window,
                        cx,
                    );
                })),
        );
        acts = acts.child(
            div()
                .id("res-act-duplicate")
                .flex()
                .items_center()
                .justify_center()
                .h(t().d(28.))
                .px(t().d(10.))
                .rounded(t().r(7.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.text_2))
                .hover(|el| el.bg(rgb(t().p.hover)))
                .child("复制")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.resource_ext_duplicate(window, cx);
                })),
        );
    }
    if file_backed {
    acts = acts.child(
        div()
            .id("res-act-publish")
            .flex()
            .items_center()
            .justify_center()
            .h(t().d(28.))
            .px(t().d(10.))
            .rounded(t().r(7.))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .text_size(t().fs(11.5))
            .text_color(rgb(t().p.text_2))
            .hover(|el| el.bg(rgb(t().p.hover)))
            .child("发布")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.resource_open_publish(cx);
            })),
    );
    }
    head = head.child(acts);
    body = body.child(head);

    match sel {
        Selected::Skill(name) => {
            if let Some(skill) = r.skills.as_ref().and_then(|s| s.skills.iter().find(|x| &x.name == name)) {
                body = body
                    .child(detail_kv("描述", skill.description.as_deref().unwrap_or("—")))
                    .child({
                        let cats = if skill.category.is_empty() { "未分类".to_string() } else { skill.category.join(" · ") };
                        detail_kv("类别", &cats)
                    })
                    .child(detail_kv("状态", if skill.loadable() { "可加载" } else { "加载失败" }))
                    .child(detail_kv("出处", match skill.tracked {
                        Some(true) => "仓库里（已提交）",
                        Some(false) => "未提交",
                        None => "未知（引擎暂不查 git）",
                    }));
                if skill.hidden {
                    body = body.child(note_line("隐藏：模型看不见它，勾上也不进提示词。"));
                }
                if skill.tracked == Some(false) {
                    body = body.child(note_line("还没提交 —— 只在这台机器上看得见。点「发布」才会进别人的视野。"));
                }
            }
        }
        Selected::Prompt(name) => {
            if let Some(p) = r.prompts.as_ref().and_then(|p| p.prompts.iter().find(|x| &x.name == name)) {
                let untracked = p.tracked == Some(false);
                body = body
                    .child(detail_kv("摘要", p.summary.as_deref().or(p.description.as_deref()).unwrap_or("—")))
                    .child({
                        let cats = if p.category.is_empty() { "未分类".to_string() } else { p.category.join(" · ") };
                        detail_kv("类别", &cats)
                    })
                    .child(detail_kv("状态", if p.loadable() { "可加载" } else { "加载失败" }));
                if r.prompts.as_ref().and_then(|p| p.default.as_deref()) == Some(name.as_str()) {
                    body = body.child(note_line("这是仓库默认值（`settings.json` 的 `default`）—— 只用来预勾，不是「用户选的那条」。"));
                }
                if untracked {
                    body = body.child(note_line("还没提交 —— 只在这台机器上看得见。点「发布」才会进别人的视野。"));
                }
            }
        }
        Selected::Ext(name) => {
            let found = r.ext_view().and_then(|view| {
                let row = ext_rows(view, None, "");
                row.into_iter().find(|x| &x.name == name)
            });
            if let Some(e) = found {
                let cats = if e.category.is_empty() {
                    "未分类".to_string()
                } else {
                    e.category.join(" · ")
                };
                body = body
                    .child(detail_kv("类别", &cats))
                    // 归属 = 档位（启用中 / 内置 / 变体 / 已归档）。以前这一格写成「类别」，
                    // 于是「类别 启用中」—— 类别是清单里的分类，档位是盘上的状态，两回事。
                    .child(detail_kv("归属", e.kind.label()))
                    .child(detail_kv("摘要", e.summary.as_deref().unwrap_or("—")));
                // 装 / 卸 / 归档是三件不同的事，按钮分开摆；核心三条卸载会被引擎 403 挡回来。
                if e.kind == ExtKind::Archived {
                    body = body.child(note_line("在 .archive/ 里 —— 别人也看不见。「恢复」把它搬回仓库并提交，「真删」会一起提交删掉。"));
                } else if e.kind == ExtKind::Variant {
                    body = body.child(note_line(
                        "变体是另一棵工作树（`/v1/repos/extend/variants`）：路径与分支看上面的摘要。\
                         上面四个按钮是「改分支 / 发布变体 / 解绑 / 清理」：\
                         改分支要填新分支名（表单），其余三条是一句话确认。",
                    ));
                } else if e.legacy && e.kind == ExtKind::Builtin {
                    body = body
                        .child(note_line("核心扩展（nous / skills / prompts）引擎自己要跑 —— 卸载会被 403 挡回来；它们在仓库里，改得动，删不得。"))
                        .child(note_line("但它现在装的是 TS 时代的 JS 插件（index.js + defineExtension）：新运行时只 spawn 原生进程扩展，`index.js` 没有解释器，改清单也跑不起来，需按 nous-ext-sdk 重写。"));
                } else if e.kind == ExtKind::Builtin {
                    body = body.child(note_line("核心扩展（nous / skills / prompts）引擎自己要跑 —— 卸载会被 403 挡回来；它们在仓库里，改得动，删不得。"));
                } else if e.legacy {
                    // 这一档不是「装 / 卸」能治的：新运行时只 spawn 原生产物（协议握手 +
                    // artifacts），`index.js` 没有解释器。说清楚该干什么，别让人反复点重装。
                    body = body.child(note_line("TS 时代的 JS 插件（index.js + defineExtension）—— 新运行时只装载原生进程扩展，改清单也跑不起来；需按 nous-ext-sdk 重写。"));
                } else {
                    body = body.child(note_line("装 / 卸 / 归档是三件不同的事；核心三条（nous / skills / prompts）卸载会被引擎 403 挡回来。"));
                }
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
        .min_w(t().d(0.))
        .gap(t().d(12.))
        .text_size(t().fs(12.))
        .child(
            div()
                .w(t().d(56.))
                .text_color(rgb(muted()))
                .child(k.to_string()),
        )
        .child(
            // 长串（引擎的报错原文）要能在标签后面折行，不能把整行撑出窗口。
            div()
                .flex_1()
                .min_w(t().d(0.))
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

