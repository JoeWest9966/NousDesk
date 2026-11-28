//! 「资源」页：方案 03 —— 一个图标，三段并列（技能 / 提示词 / 扩展）。
//!
//! 外观稿见 `demo/resource-pages/preview.html` 的 03 号。这一刀是**只读**：
//! 拉 / 切 / 筛 / 搜 / 看详情，写操作（发布 / 归档 / 文件编辑）留第二刀。
//!
//! ## 为什么三段并列
//!
//! 技能和提示词同一个模子（两个私库、两套端点，但都是「纯文字 + 类别 + 可发布」），
//! 共用一套树 / 列表 / 详情。以前它们裹在「内容」段里再挂一个子开关：那一层不装
//! 任何东西、只多一次点击（2026-11-24 用户判断「内容这个没用」）—— 删掉，三个平铺。
//! 扩展不是列表 —— 有 `catalog` / `builtin` / `variants` / `archived` / `installed`
//! 五类东西，装 / 卸 / 归档是三件不同的事（核心三条 403）。硬塞成一个列表等于把
//! 五种东西涂成同一种颜色，见 `decisions.md` 2026-11-16 那条。
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
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable as _, Size};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::types::{
    ExtCategories, Extensions, Prompt, Prompts, RepoArchive, RepoVariants, Skill, Skills, TrashList,
};
use nous_engine::{ellipsis_middle, pending_unit_count, Repo, RepoPending, RepoStatus};

use crate::app::Desktop;
use crate::category::{CatNode, build_tree, is_under};

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 底部仓片的徽标：GitHub mark（内联 SVG，填色版）。不请求 github.com、不抓头像。
const GH_MARK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M12 2C6.48 2 2 6.58 2 12.25c0 4.53 2.87 8.37 6.84 9.73.5.09.68-.22.68-.48v-1.7c-2.78.62-3.37-1.37-3.37-1.37-.45-1.18-1.11-1.5-1.11-1.5-.91-.63.07-.62.07-.62 1 .07 1.53 1.06 1.53 1.06.89 1.57 2.34 1.12 2.91.85.09-.66.35-1.12.63-1.38-2.22-.26-4.55-1.14-4.55-5.06 0-1.12.39-2.03 1.03-2.75-.1-.26-.45-1.3.1-2.71 0 0 .84-.28 2.75 1.05a9.4 9.4 0 0 1 5 0c1.91-1.33 2.75-1.05 2.75-1.05.55 1.41.2 2.45.1 2.71.64.72 1.03 1.63 1.03 2.75 0 3.93-2.34 4.8-4.57 5.05.36.32.68.94.68 1.9v2.82c0 .27.18.58.69.48A10.02 10.02 0 0 0 22 12.25C22 6.58 17.52 2 12 2Z"/></svg>"##;

/// 有远端时那个圆按钮里的箭头（右上）。
const ARROW_UP_RIGHT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5 8.5 3.5M8.5 3.5H4.6M8.5 3.5v3.9"/></svg>"##;

/// 分支在仓片 meta 行里最多占几个字符（300px 列 + 10px mono）。超了走中间省略，
/// 完整值进 tooltip。18 能让 `feat/skills-upstream-source` 变成 `feat/skills…source`。
const BRANCH_MAX_CHARS: usize = 18;

/// 能交给系统浏览器打开的 URL：**只认 http/https**，且必须有主机段。
///
/// 这条纯函数是那个 ↗ 的唯一判据 —— 引擎回什么就开什么等于让远端替我们决定
/// `xdg-open` 的 argv（`file:` / `javascript:` / 空串都该被挡在门外）。
/// 返回 `None` 时 UI 不打开、也不弹「已打开」。
pub(crate) fn external_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    if url.is_empty() || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &url[8..]
    } else if lower.starts_with("http://") {
        &url[7..]
    } else {
        return None;
    };
    // `https://` / `https:///x` / `https://?q` 都没有主机，不是链接。
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return None;
    }
    Some(url.to_string())
}

/// 用系统默认浏览器打开一个链接。返回 false = 连进程都没起来（`xdg-open` 不在、
/// 被策略挡住）；进程起来了不代表浏览器真显示出来了，那一步不归我们管。
///
/// `spawn` 只传 argv、不过 shell，URL 里的 `&` / `?` 不会被二次解释。
fn open_external(url: &str) -> bool {
    let Some(url) = external_url(url) else {
        return false;
    };
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let spawned = std::process::Command::new("xdg-open").arg(&url).spawn();
    spawned.is_ok()
}

/// 三个段，平铺：技能 / 提示词 / 扩展。
///
/// 以前是「内容（技能 · 提示词）/ 扩展」两段 + 一层内容类型子开关。`内容` 那层
/// 既不装东西也不做事，只让人多点一次 —— 2026-11-24 删掉，三个并列。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Section {
    #[default]
    Skills,
    Prompts,
    Extensions,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skills => "技能",
            Self::Prompts => "提示词",
            Self::Extensions => "扩展",
        }
    }

    /// 三个段各自那份状态（类别筛选 / 搜索框）用的槽位。**只在这里排下标**，
    /// 别处一律用 `slot()`，省得再加一段时有人漏改一处。
    pub(crate) fn slot(self) -> usize {
        match self {
            Self::Skills => 0,
            Self::Prompts => 1,
            Self::Extensions => 2,
        }
    }

    /// 这一段对应的私库（`/v1/repos/{repo}/*` 用的仓名）。
    pub(crate) fn repo(self) -> Repo {
        match self {
            Self::Skills => Repo::Skills,
            Self::Prompts => Repo::Prompts,
            Self::Extensions => Repo::Extensions,
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

/// 窄窗下中间列的两层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ResCol {
    /// 当前段的条目列表（默认）。
    #[default]
    Items,
    /// 左树那一层：段切换 + 类别 + 垃圾站。
    Tree,
}

impl ResCol {
    /// 退一层：树 → 列表。到头就停着（Esc 连按不会退到别处去）。
    pub(crate) fn pop(self) -> Self {
        ResCol::Items
    }
}

/// 一个段的仓探针（底部仓片用）：`status` 画仓名 / 分支 / 路径 / 脏点 / ↗，
/// `pending` 算「N 处待发布」。两样分开存：一份挂了只降级它自己那一半。
///
/// **切换段才拉、拉到就缓存** —— 不一次 join 三个 status。`loading` / `failed`
/// 一起管住「正在拉」与「拉失败不再自动重拉」；`force` 由刷新 / 写操作触发。
#[derive(Default)]
pub(crate) struct RepoProbe {
    pub(crate) status: Option<RepoStatus>,
    pub(crate) pending: Option<RepoPending>,
    pub(crate) loading: bool,
    pub(crate) failed: bool,
}

/// 这一页的全部状态。**只有这一页持有**，不塞进 `Desktop`。
pub(crate) struct ResourceState {
    pub(crate) section: Section,
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
    /// 三个段各自的仓探针（status + pending），按段槽位存。底部仓片读它。
    pub(crate) probe: [RepoProbe; 3],
    /// 三个 tab 各自的类别筛选，切来切去能回到原来那一行。
    cat: [Option<String>; 3],
    /// 三段各自的搜索框。**各存一份**，切段时词还在（跟 `cat` 同款语义）。
    /// 用 `InputState`（不是裸 String）才有中文输入法 / 剪贴板 / 一键清空 —— 这些是
    /// 第一屏体验，跟左栏会话搜索同一套组件。
    search: [Entity<InputState>; 3],
    pub(crate) selected: Option<Selected>,
    /// 窄窗下中间列自己在哪一层（照模板页 `NarrowCol` 那套）：条目列表 ↔ 类别树。
    /// 980 宽装不下三栏，左树整块不画 —— 不往下钻的话「内容 / 扩展」段切换、
    /// 类别、垃圾站三个入口就全没了。
    pub(crate) narrow_col: ResCol,
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
            skills: None,
            prompts: None,
            exts: None,
            ext_variants: None,
            ext_archive: None,
            ext_cats: None,
            ext_trash: None,
            probe: Default::default(),
            cat: [None, None, None],
            search: [
                box_for("搜技能：名字 / 描述"),
                box_for("搜提示词：名字 / 摘要"),
                box_for("搜扩展：名字 / 摘要"),
            ],
            selected: None,
            narrow_col: ResCol::Items,
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
        self.section.slot()
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
        // 底部仓片：跟着刷新 / 写操作失效，并且只在当前段拉 —— 不一次 join 三个 status。
        self.ensure_repo_probe(force, cx);
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
        // 列表清单可能已经 hydrated（load_resources 提前返回），但仓片还得拉新的那一段。
        self.ensure_repo_probe(false, cx);
        cx.notify();
    }

    /// 当前段的仓探针：没拉过就拉一次（status + pending 并发），已有就吃缓存。
    /// `force` 把三段的缓存全清掉、只重拉当前段（写操作 / 手动刷新走这条）。
    /// 拉失败只标记 `failed`，不挡列表 —— 仓片自己降级成一行文案。
    fn ensure_repo_probe(&mut self, force: bool, cx: &mut Context<Self>) {
        let idx = self.resources.section.slot();
        if force {
            for p in self.resources.probe.iter_mut() {
                *p = RepoProbe::default();
            }
        }
        if self.resources.probe[idx].loading || self.resources.probe[idx].status.is_some() {
            return;
        }
        if self.resources.probe[idx].failed && !force {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let repo = self.resources.section.repo();
        let key = repo.repo_key().to_string();
        self.resources.probe[idx].loading = true;
        self.resources.probe[idx].failed = false;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (status, pending) = tokio::join!(
                engine.repo_status(&key),
                engine.repo_pending(&key),
            );
            let _ = this.update(cx, |this, cx| {
                let p = &mut this.resources.probe[idx];
                p.loading = false;
                match status {
                    Ok(s) => p.status = Some(s),
                    Err(e) => {
                        p.failed = true;
                        log::warn!("仓状态拉不到（{key}）：{e}");
                    }
                }
                if let Ok(pen) = pending {
                    p.pending = Some(pen);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// ↗ 圆按钮：把这一段的远端仓在系统浏览器里打开。
    ///
    /// 只在 `status.remote` 存在时才画这个按钮，所以这里收到的应该是引擎给的 origin URL；
    /// 仍然再过一遍 `external_url` —— 远端字段是数据，不是信任状。打不开就说打不开，
    /// 别假装点成功了（点下去没反应算 bug）。
    fn open_repo_remote(&mut self, url: String, cx: &mut Context<Self>) {
        if open_external(&url) {
            return;
        }
        self.notify(
            format!("打不开这个远端地址：{url}"),
            crate::app::NoticeKind::Error,
            cx,
        );
    }

    /// COPY 路径条：走现有剪贴板通道，提示里说清「复制的是引擎那台机器上的路径」——
    /// 免得用户以为复制的是自己桌面上的路径。
    fn copy_repo_path(&mut self, path: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
        self.notify(
            format!("已复制引擎那台机器上的路径：{path}"),
            crate::app::NoticeKind::Info,
            cx,
        );
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
                root = root.child(render_tree(&desk.resources, cx, false));
            }
            // 窄窗的第二层：树**只换掉中间那一列**（列表的位置），右边的详情列照旧 ——
            // 树铺满整块的话详情列会被吃掉，看类别时反而没法对照条目（2026-11-23 反馈）。
            root = if narrow && desk.resources.narrow_col == ResCol::Tree {
                root.child(render_tree(&desk.resources, cx, true))
            } else {
                root.child(render_list(desk, cx, narrow))
            };
            root.child(render_detail(desk, cx)).into_any_element()
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

fn render_tree(r: &ResourceState, cx: &mut Context<Desktop>, narrow: bool) -> AnyElement {
    let (n_skills, n_prompts, n_ext) = counts(
        r.skills.as_ref(),
        r.prompts.as_ref(),
        r.ext_view(),
    );

    // 类别树：复用 category.rs 的纯函数，不第三次写 count + sort。
    let (cat_nodes, special): (Vec<CatNode>, Vec<(&str, String)>) = match r.section {
        Section::Skills => {
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
        Section::Prompts => {
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

    // 段按钮：三个段平铺，点谁换谁的列表。**不动 `narrow_col`** —— 筛选器不是导航。
    let seg_btn = |section: Section, n: usize, id: &'static str, cx: &mut Context<Desktop>| {
        let on = r.section == section;
        div()
            .id(ElementId::Name(id.into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.resource_section(section, cx);
            }))
            .flex_1()
            .py(t().d(7.))
            .text_size(t().fs(12.))
            .text_color(rgb(if on { t().p.accent_fg } else { muted() }))
            .bg(rgb(if on { t().p.accent_soft } else { t().p.panel }))
            .child(format!("{}  {n}", section.label()))
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
        // 宽窗是固定 248 的左栏；窄窗它顶替列表那一列，宽度就跟列表一样（不铺满）。
        .when(narrow, |el| el.w(t().d(crate::app::TEMPLATE_LIST_W)))
        .when(!narrow, |el| el.w(t().d(crate::app::PRESET_TREE_W)))
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
                .flex()
                .items_center()
                .gap(t().d(6.))
                // 窄窗：这一层是「往上钻」出来的，得有回去的路 ——
                // 跟模板页那颗 `‹ 类别` 同一套（同一个位置、同一个手势）。
                .when(narrow, |el| {
                    el.child(
                        div()
                            .id("res-back-items")
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
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .child(Icon::new(IconName::ChevronLeft).size(t().d(12.)))
                            .child("条目")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.resources.narrow_col = ResCol::Items;
                                cx.notify();
                            })),
                    )
                })
                .child(
                    div()
                        .text_size(t().fs(13.))
                        .text_color(rgb(text_color()))
                        // 窄窗这一层只剩类别 —— 标题就叫类别，别写「资源」（筛选器不在这儿，
                        // 写着「资源」会让人以为段切换也在这儿，白白找一圈）。
                        .child(if narrow { "类别" } else { "资源" }),
                ),
        )
        // 筛选器（段 + 内容类型）只在宽窗跟着树走 —— 窄窗它们是列表头顶那一行，
        // 不搬进树：搬进来就变成「树里的层级」，点一下等于换列（2026-11-23 的错）。
        .when(!narrow, |el| {
            el.child(
                div()
                    .mx(t().d(10.))
                    .flex()
                    .gap(t().d(3.))
                    .rounded(t().r(7.))
                    .bg(rgb(t().p.hover))
                    .child(seg_btn(Section::Skills, n_skills, "res-seg-skills", cx))
                    .child(seg_btn(Section::Prompts, n_prompts, "res-seg-prompts", cx))
                    .child(seg_btn(Section::Extensions, n_ext, "res-seg-ext", cx)),
            )
        })
        .child(tree)
        .into_any_element()
}

/// 筛选器要做的事：切段。
///
/// **筛选器不动 `narrow_col`**：它只决定「列表里是哪一批」，不决定「现在看到哪一层」。
/// 上一版把段按钮搬进窄窗的树里，点一下就把中间列换掉 —— 那是把筛选器当成了导航。
fn filter_pill(
    label: String,
    on: bool,
    id: &'static str,
    section: Section,
    cx: &mut Context<Desktop>,
) -> AnyElement {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .h(t().d(22.))
        .px(t().d(7.))
        .rounded(t().r(6.))
        .text_size(t().fs(11.))
        .whitespace_nowrap()
        .when(on, |el| el.bg(rgb(t().p.accent_soft)).text_color(rgb(t().p.accent_fg)))
        .when(!on, |el| {
            el.text_color(rgb(t().p.text_2)).hover(|h| h.bg(rgb(t().p.hover)))
        })
        .child(label)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.resource_section(section, cx);
        }))
        .into_any_element()
}

/// 扩展段 `⋯` 菜单里的一行。纯数据：渲染和判据共用一份，别在渲染里另写一遍顺序。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtMenu {
    /// 重扫扩展目录。技能 / 提示词没有进程，改完文件即生效，所以只有这一段有。
    Reload,
    /// 类别词表：改的是「有哪些类别」，不是某条扩展挂哪个。
    Categories,
    /// 垃圾站里有几份。
    Trash(usize),
}

/// `⋯` 里有哪些项、按什么顺序。
///
/// 垃圾站**空着不进菜单**：它在树里有一行、且常显（2026-11-23 的可见性判据），
/// 空着再给一个菜单项就是同一页两个入口。菜单这一项只在「刚卸载完、想回去看」
/// 时有用 —— 那时它必然非空。
pub(crate) fn ext_menu_items(trash_len: usize) -> Vec<ExtMenu> {
    let mut items = vec![ExtMenu::Reload, ExtMenu::Categories];
    if trash_len > 0 {
        items.push(ExtMenu::Trash(trash_len));
    }
    items
}

/// 扩展段的段工具：一颗 `⋯`，动作进浮层菜单。
///
/// 0.9.27 及以前是并排三颗（重载 / 垃圾站 / 类别）。它们和段 pill 落在同一条
/// 视觉带上、又是两个层级，扫一眼像「段按钮被顶掉了」（2026-11-24 用户提的）；
/// 并排还会把 `+ 新建` 挤到窄窗边上。菜单交给 gpui-kit 的 `DropdownMenu`：
/// Esc / 点外面 / 翻边都由库管，本文件不另造浮层和开合状态。
fn more_btn(r: &ResourceState, cx: &mut Context<Desktop>) -> AnyElement {
    // 技能 / 提示词没有进程、词表、垃圾站：不画一颗点了没反应的按钮。
    if r.section != Section::Extensions {
        return div().into_any_element();
    }
    let trash_len = r.ext_trash.as_ref().map(|t| t.items.len()).unwrap_or(0);
    let items = ext_menu_items(trash_len);
    let this = cx.entity().downgrade();
    Button::new("res-ext-more")
        .ghost()
        .small()
        .compact()
        .icon(Icon::new(IconName::Ellipsis).size(t().d(15.)))
        .tooltip("扩展的更多动作")
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _window, _cx| {
            let mut menu = menu.min_w(t().d(158.));
            for item in items.clone() {
                menu = match item {
                    ExtMenu::Reload => {
                        let this = this.clone();
                        menu.item(PopupMenuItem::new("重载").on_click(
                            move |_: &ClickEvent, window, cx: &mut App| {
                                let _ = this
                                    .update(cx, |this, cx| this.resource_ext_reload(window, cx));
                            },
                        ))
                    }
                    ExtMenu::Categories => {
                        let this = this.clone();
                        menu.item(PopupMenuItem::new("类别词表…").on_click(
                            move |_: &ClickEvent, window, cx: &mut App| {
                                let _ = this.update(cx, |this, cx| {
                                    this.resource_open_categories(window, cx)
                                });
                            },
                        ))
                    }
                    ExtMenu::Trash(n) => {
                        let this = this.clone();
                        menu.item(PopupMenuItem::separator()).item(
                            PopupMenuItem::new(format!("垃圾站 {n}")).on_click(
                                move |_: &ClickEvent, window, cx: &mut App| {
                                    let _ = this.update(cx, |this, cx| {
                                        this.resource_open_trash(window, cx)
                                    });
                                },
                            ),
                        )
                    }
                };
            }
            menu
        })
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
            // 窄窗：点类别 = 往下钻到条目。不钻的话点了看不出变化（树还占着这一列）。
            this.resources.narrow_col = ResCol::Items;
            cx.notify();
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
            this.resources.narrow_col = ResCol::Items;
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

/// 资源页中间列底部的仓片（三段共用一套）。
///
/// 形态照 `demo/repo-footer/v02` 的 03：上段是 GitHub mark、仓名、`分支 · N 处待发布`、
/// 以及「有 remote 才有」的 ↗；下段一条 hairline 隔开的路径条 + COPY。高约 70。
/// 三段只差数据：仓名 / 分支 / 路径 / 处数。
fn repo_plaque(r: &ResourceState, cx: &mut Context<Desktop>) -> AnyElement {
    let idx = r.section.slot();
    let repo = r.section.repo();
    let probe = &r.probe[idx];

    // status 拉失败：只降级这一行，不挡列表（列表照常可看）。
    if probe.failed {
        return div()
            .flex_none()
            .border_t(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .px(t().d(12.))
            .py(t().d(9.))
            .text_size(t().fs(10.))
            .text_color(rgb(muted()))
            .child("仓状态暂时拿不到")
            .into_any_element();
    }
    let Some(status) = probe.status.as_ref() else {
        // 还没拉回来：先不占位（拉到就出来，不闪一个空壳）。
        return div().into_any_element();
    };

    // 仓名：取仓根本地路径的末段（`/opt/Nous-skills` → `Nous-skills`），取不到用段标签。
    let name = status
        .root
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| repo.label().to_string());

    let full_branch = status
        .branch
        .clone()
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "—".to_string());
    let branch_disp = ellipsis_middle(&full_branch, BRANCH_MAX_CHARS);
    let count = probe
        .pending
        .as_ref()
        .map(|p| pending_unit_count(repo, &p.units));

    // 徽标：28 方框 + 16px GitHub mark；脏时右下角一颗白点。
    let mut avatar = div()
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(t().d(28.))
        .rounded(t().r(6.))
        .border(t().d(1.))
        .border_color(rgb(t().p.text).opacity(0.28))
        .bg(rgb(t().p.panel))
        .child(
            Icon::empty()
                .data(GH_MARK)
                .with_size(Size::Size(t().d(16.)))
                .text_color(rgb(t().p.text_2)),
        );
    if status.dirty {
        avatar = avatar.child(
            div()
                .absolute()
                .bottom(t().d(-2.))
                .right(t().d(-2.))
                .flex()
                .items_center()
                .justify_center()
                .size(t().d(9.))
                .rounded_full()
                .bg(rgb(t().p.panel))
                .child(div().size(t().d(5.)).rounded_full().bg(rgb(t().p.text))),
        );
    }

    // 元信息：`分支 · N 处待发布`。分支超宽走中间省略，**完整值进 tooltip**
    // （截断了才挂：没截断的 tooltip 是噪声）。
    let truncated = branch_disp != full_branch;
    let tip = full_branch.clone();
    let mut branch_el = div()
        .id(("res-repo-branch", idx))
        .font_family("JetBrainsMono")
        .text_color(rgb(muted()))
        .child(branch_disp);
    if truncated {
        // `Div::tooltip` 要一个 view builder，不接字符串 —— 这就是 #23 当时没挂上的原因。
        // 走 gpui-component 的 `Tooltip`（它自己是个 view，`build` 直接给 AnyView）。
        branch_el = branch_el.tooltip(move |window, cx| {
            Tooltip::new(tip.clone()).build(window, cx)
        });
    }
    let mut meta = div()
        .flex()
        .items_center()
        .gap(t().d(5.))
        .text_size(t().fs(10.))
        .text_color(rgb(muted()))
        .whitespace_nowrap()
        .child(branch_el);
    if let Some(n) = count {
        meta = meta
            .child(div().text_color(rgb(t().p.text).opacity(0.4)).child("·"))
            .child(
                div()
                    .text_color(rgb(t().p.text_2))
                    .child(format!("{n} 处待发布")),
            );
    }

    let body = div()
        .flex_1()
        .min_w(t().d(0.))
        .flex()
        .flex_col()
        .gap(t().d(3.))
        .child(
            div()
                .truncate()
                .text_size(t().fs(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(text_color()))
                .child(name),
        )
        .child(meta);

    let mut top = div()
        .flex()
        .items_center()
        .gap(t().d(11.))
        .px(t().d(12.))
        .pt(t().d(10.))
        .pb(t().d(9.))
        .child(avatar)
        .child(body);
    // 没有 remote 就不画那个 → 圆按钮（不猜、不降级）。有就真能点：系统浏览器打开。
    if let Some(remote) = status.remote.clone().filter(|r| !r.is_empty()) {
        let tip = remote.clone();
        top = top.child(
            div()
                .id("res-repo-open")
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .size(t().d(26.))
                .rounded_full()
                .border(t().d(1.))
                .border_color(rgb(t().p.text).opacity(0.28))
                .cursor_pointer()
                .hover(|el| el.bg(rgb(t().p.hover)))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .child(
                    Icon::empty()
                        .data(ARROW_UP_RIGHT)
                        .with_size(Size::Size(t().d(12.)))
                        .text_color(rgb(t().p.text_2)),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.open_repo_remote(remote.clone(), cx);
                })),
        );
    }

    // 下段路径条：`/opt/Nous-skills` + COPY，一条 14% hairline 隔开。
    let path = status.root.clone();
    let copy_path = path.clone();
    let bar = div()
        .flex()
        .items_center()
        .gap(t().d(6.))
        .pl(t().d(12.))
        .pr(t().d(8.))
        .pt(t().d(5.))
        .pb(t().d(6.))
        .border_t(t().d(1.))
        .border_color(rgb(t().p.text).opacity(0.14))
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .truncate()
                .text_size(t().fs(9.5))
                .font_family("JetBrainsMono")
                .text_color(rgb(muted()))
                .child(path),
        )
        .child(
            div()
                .id("res-repo-copy")
                .flex_none()
                .cursor_pointer()
                .text_size(t().fs(8.5))
                .text_color(rgb(muted()))
                .hover(|el| el.text_color(rgb(text_color())))
                .child("COPY")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.copy_repo_path(copy_path.clone(), cx);
                })),
        );

    div()
        .flex_none()
        .flex()
        .flex_col()
        .border_t(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .bg(rgb(t().p.panel))
        .child(top)
        .child(bar)
        .into_any_element()
}

fn render_list(desk: &Desktop, cx: &mut Context<Desktop>, narrow: bool) -> AnyElement {
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

    // 「+ 新建」跟着当前段走：在技能段建技能、提示词段建提示词、扩展段建扩展。
    let new_kind = match r.section {
        Section::Skills => crate::resource_write::Kind::Skill,
        Section::Prompts => crate::resource_write::Kind::Prompt,
        Section::Extensions => crate::resource_write::Kind::Ext,
    };
    // 筛选器上的数字跟树里那几行**同一口径** —— 一处算（`counts`），不许两处各数一遍。
    let (n_skills, n_prompts, n_ext) = counts(r.skills.as_ref(), r.prompts.as_ref(), r.ext_view());

    // 「+ 新建」跟着当前段走：在技能段建技能、在扩展段建扩展。
    let new_btn = |cx: &mut Context<Desktop>| {
        div()
            .id("res-new")
            .flex()
            .items_center()
            .h(t().d(22.))
            .px(t().d(8.))
            .rounded(t().r(6.))
            .bg(rgb(t().p.accent))
            .text_size(t().fs(11.))
            .text_color(rgb(t().p.on_accent))
            .hover(|el| el.bg(rgb(t().p.accent_hover)))
            .child("+ 新建")
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.resource_new(new_kind, window, cx);
            }))
            .into_any_element()
    };
    // 段工具：扩展段才有一颗 `⋯`（宽窗在标题栏右边，窄窗在第二行右边），
    // 点开是 重载 / 类别词表 / 垃圾站。技能、提示词段没有这一层。
    let tools = |cx: &mut Context<Desktop>| more_btn(r, cx);

    list = list.child(if narrow {
        // 窄窗：段筛选器就待在列表头顶，**不搬进树** —— 搬进树它们就变成层级，
        // 点一下等于把这一列换掉（2026-11-23 被指出的错）。树上只有类别。
        // 三个段一行排开（技能 / 提示词 / 扩展）；第二行左边新建、右边该段的工具。
        div()
            .flex()
            .flex_col()
            .gap(t().d(6.))
            .px(t().d(10.))
            .pt(t().d(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .child(
                        div()
                            .id("res-open-tree")
                            .flex()
                            .items_center()
                            .gap(t().d(3.))
                            .h(t().d(22.))
                            .px(t().d(7.))
                            .rounded(t().r(6.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.text_2))
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .child(Icon::new(IconName::Menu).size(t().d(12.)))
                            .child("类别")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.resources.narrow_col = ResCol::Tree;
                                cx.notify();
                            })),
                    )
                    .child(filter_pill(
                        format!("技能  {n_skills}"),
                        r.section == Section::Skills,
                        "res-seg-skills",
                        Section::Skills,
                        cx,
                    ))
                    .child(filter_pill(
                        format!("提示词  {n_prompts}"),
                        r.section == Section::Prompts,
                        "res-seg-prompts",
                        Section::Prompts,
                        cx,
                    ))
                    .child(filter_pill(
                        format!("扩展  {n_ext}"),
                        r.section == Section::Extensions,
                        "res-seg-ext",
                        Section::Extensions,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .child(new_btn(cx))
                    .child(div().flex_1())
                    .child(tools(cx)),
            )
            .into_any_element()
    } else {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(t().d(6.))
            .px(t().d(12.))
            .pt(t().d(10.))
            .child(
                div()
                    .text_size(t().fs(12.))
                    .text_color(rgb(text_color()))
                    .child(r.section.label()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .child(tools(cx))
                    .child(new_btn(cx)),
            )
            .into_any_element()
    });

    // 搜索框：三段各一份，切段时词还在。suffix 写命中数（空词写总数），
    // 跟左栏会话搜索同一个样式 —— 一行省下来就是列表的 20px。
    let total = match r.section {
        Section::Skills => r.skills.as_ref().map(|s| s.skills.iter().filter(|x| !x.hidden).count()).unwrap_or(0),
        Section::Prompts => r.prompts.as_ref().map(|p| p.prompts.len()).unwrap_or(0),
        // 老形状（没有 `items`）只有 `catalog`；新形状按段按钮同一口径数。
        Section::Extensions => match r.ext_view() {
            Some(view) if view.exts.items.is_empty() => view.exts.catalog.len(),
            Some(view) => view.exts.items.len() + view.variant_count() + view.archived_count(),
            None => 0,
        },
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
        Section::Skills => {
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
                // 等的是技能清单 —— 以前这里误写成 `RowKind::Prompts`，
                // 于是「技能没拉到」会告诉用户「正在等提示词清单」。
                body = body.child(placeholder(r, RowKind::Skills, &q, cat, cx));
            }
        }
        Section::Prompts => {
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
                body = body.child(placeholder(r, RowKind::Prompts, &q, cat, cx));
            }
        }
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
    list.child(body).child(repo_plaque(r, cx)).into_any_element()
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

