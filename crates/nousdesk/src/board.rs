//! 「New Manual」盘面。
//!
//! 外观是**轨道**（`demo/new-manual#7` 那一套）：四张参数卡围着中央一颗 `Create`，
//! 虚线把「谁喂给谁」连出来。不再画 Cynefin 那张四象限图 —— 四块色在深色底上
//! 只有 14% alpha，缩进右格里会糊成一块脏砖红。
//!
//! 弹层是**就地弹出**（`demo/new-manual-7#3`）：贴着被点的那张卡，不居中、
//! 不把盘面糊掉 —— 选参数的时候还想看得见盘面。
//!
//! **只占右侧聊天区那一格**，不再盖住整窗：左导航和会话列表永远在，
//! 点某个会话 / Esc 就回到聊天。尺寸基准因此从整窗 vmin 换成右格 cqmin。
//!
//! 三条纪律（和 demo 对齐，不是我自己编的）：
//! - **一屏不滚**：尺寸全走右格 cqmin，右格变了跟着缩，盘面永远填满右格。
//! - **形状说语义**：模型是「供应商 › 模型 › 思考」三段语法链，画成三列；
//!   技能 / 模板 / 目录是平铺清单，因为它们不是链（集合 / 单值 / 导航）。
//! - **一个动作一个入口**：Create 只有中间那颗圆，dock 里只放名字 / 机器 / 目录。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

use std::collections::BTreeSet;

use crate::app::NoticeKind;

use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;

use nous_engine::EngineError;
use nous_engine::types::{
    ConfigPatch, CreateSession, CreatedSession, Dirs, Extensions, Machines, Model, Models, Prompt,
    Prompts, Skill, Skills,
};

use crate::app::Desktop;
use crate::board_art as art;

/// 打开着哪一层的弹层。`None` = 就在盘面上。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sheet {
    /// 会话页的「这个会话带什么」：四行摘要，点哪行就开哪张卡的弹层。
    Session,
    Model,
    Skills,
    Prompt,
    Ext,
    Dir,
    Machine,
}

/// 这个条目属不属于 `path` 这一格 —— **含子类**。
///
/// `category` 是精确标签，不是层级继承：`123` 只写了 `工具/搜索`，没写 `工具`。
/// 不做前缀匹配的话，树上就会「工具 1、搜索 1」，看着像子类比父类还满。
fn in_cat(cats: &[String], path: &str) -> bool {
    cats
        .iter()
        .any(|c| c == path || c.strip_prefix(path).is_some_and(|r| r.starts_with('/')))
}

/// 扩展那一行是「什么处境」。决定勾不勾得动、画不画虚线框。
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExtKind {
    /// 装上的普通扩展，可以勾掉。
    Pickable,
    /// 核心（`nous` / `skills` / `prompts`）：引擎**总是带上**，勾掉是假动作。
    ///
    /// 2026-11-16 实测：`POST /v1/sessions` 的 `extensions` 只能加不能减 ——
    /// 内核在 `settings.extensions` 里无条件放四个核心，请求里少写的不会删掉。
    /// 所以这一格是**锁住的**，不是「勾掉但提醒别」。
    Core,
    /// 引擎自带，不在仓库里。
    Builtin,
    /// 变体，加载失败了。
    Broken,
    /// 已归档。
    Archived,
}

/// 引擎**无条件**带上的那几条：`coding-tools` + `nous` + `skills` + `prompts`。
///
/// 不从 `installed` 推：`coding-tools` 是 `builtin`，根本不在 `installed` 里；
/// 从 `defaults` 取也不对 —— 那是「额外可装扩展」的默认勾选项（demo 里的 `123`
/// 那种），跟这四个硬编码的不是一回事。
fn core_exts(x: &nous_engine::types::Extensions) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(builtin) = x.builtin.first() {
        v.push(builtin.clone()); // coding-tools
    }
    v.extend(x.defaults.iter().cloned());
    v
}

impl ExtKind {
    /// 左树里 `@xxx` 这个 key 对应的那一格。
    fn tag(self) -> &'static str {
        match self {
            ExtKind::Pickable | ExtKind::Core => "installed",
            ExtKind::Builtin => "builtin",
            ExtKind::Broken => "broken",
            ExtKind::Archived => "archived",
        }
    }
}

/// 键盘停靠点。**只有五站**：中央那颗圆 + 四张卡。
///
/// Name 输入框故意不在队里 —— 它自己会抢焦点，把 Tab 也吞了的话，
/// 从这里就再也走不到卡片上了。想改名字用鼠标点，或者先用 Tab 走到别处。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stop {
    Create,
    Model,
    Context,
    Ext,
    Perms,
}

impl Stop {
    /// 一圈的固定顺序（顺时针：中央 → 左上 → 右上 → 左下 → 右下）。
    const RING: [Stop; 5] = [
        Stop::Create,
        Stop::Model,
        Stop::Context,
        Stop::Ext,
        Stop::Perms,
    ];

    fn step(self, back: bool) -> Stop {
        let n = Self::RING.len();
        let i = Self::RING.iter().position(|s| *s == self).unwrap_or(0);
        Self::RING[(i + if back { n - 1 } else { 1 }) % n]
    }

    fn of(slot: art::Slot) -> Stop {
        match slot {
            art::Slot::Model => Stop::Model,
            art::Slot::Context => Stop::Context,
            art::Slot::Ext => Stop::Ext,
            art::Slot::Perms => Stop::Perms,
        }
    }
}

/// 会话页「这个会话带什么」弹层里那四行的顺序。
///
/// 跟盘面的 [`Stop`] 分开：盘面那五站是「中央 + 四角」的几何环，这里是一列四行，
/// 两边站名对不上（盘面有 Create / Context，这里有 Template）。硬套一个枚举
/// 只会让两边的 `match` 各多几条走不到的分支。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SessRow {
    Model,
    Template,
    Skills,
    Perms,
}

impl SessRow {
    const RING: [SessRow; 4] = [Self::Model, Self::Template, Self::Skills, Self::Perms];

    fn step(self, back: bool) -> Self {
        let n = Self::RING.len();
        let i = Self::RING.iter().position(|r| *r == self).unwrap_or(0);
        Self::RING[(i + if back { n - 1 } else { 1 }) % n]
    }
}

/// 盘面的状态。**选择是本地草稿**：这一页还没落库，一切都在内存里，
/// 按 Create 才变成一次 `POST /v1/sessions`。
pub(crate) struct NewChat {
    pub(crate) open: bool,
    focus: FocusHandle,
    name_input: Entity<TextareaState>,
    /// 五份清单，各自独立成功/失败 —— 一份挂了不该把别的拖着一起灰。
    models: Option<Models>,
    skills: Option<Skills>,
    prompts: Option<Prompts>,
    machines: Option<Machines>,
    dirs: Option<Dirs>,
    loading: bool,
    /// 这一页自己的错误带（在椭圆底下那行 `flash` 里显示，不弹窗）。
    load_err: Option<String>,
    flash: Option<String>,

    // ── 选择 ──
    /// `(provider, modelId)`。建会话时原样发回这个对象。
    picked: Option<(String, String)>,
    /// 思考档，`off` … `max`。换模型要按模型的 `thinkingLevels` 重新夹。
    level: String,
    picked_skills: BTreeSet<String>,
    /// `None` = 明确不要模板（会以 `template: null` 发出去）。
    template: Option<String>,
    /// `None` = 本机。
    machine: Option<String>,
    dir: Option<String>,
    /// 目录浏览到哪儿（和 `dir` 是两件事：看的地方和选的地方）。
    dir_at: Option<String>,
    /// 目录弹层的那个命令行输入框（空 = 只是浏览）。
    dir_input: Entity<TextareaState>,
    /// 技能弹层顶上的搜索框。搜名字 / 描述 / 类别，**一打字就全盘搜**（左树让位）。
    skill_query: Entity<TextareaState>,
    /// 左树选中的类别路径（`工程/事故` 这种）或 [`NewChat::UNCAT`]；`None` = 全部。
    skill_cat: Option<String>,
    /// 扩展清单。五类东西混在一份响应里，见 [`nous_engine::types::Extensions`]。
    exts: Option<Extensions>,
    ext_query: Entity<TextareaState>,
    /// 扩展左树选中的那一格：`Some(ExtGrp::…)` 的字符串形式，`None` = 全部。
    ext_cat: Option<String>,
    /// 这次会话带哪些扩展。初始 = `loadable`（这台装上且能加载的）。
    picked_exts: BTreeSet<String>,

    /// 弹层现在改的是哪儿：`None` = 盘面上的草稿（按 Create 才落库），
    /// `Some(id)` = 一条已经存在的会话（改一处推一处，走 `POST /v1/sessions/:id/config`）。
    target: Option<i64>,
    /// 键盘停在哪一格。`None` = 还没动过键盘（跟鼠标点击同步）。
    kb: Option<Stop>,
    /// 会话页那个「带什么」弹层里键盘停在哪一行。跟 `kb` 是两套：那边盘面没开着。
    ss_kb: Option<SessRow>,
    /// 现在这一层弹层是不是从会话页那四行摘要里点进来的。
    /// 是的话 Esc 退回摘要，不是一路关到底 —— 改完模板想看看四行改成了什么样，
    /// 不该逼人重新点一次顶栏。
    ss_back: bool,
    /// 下一次渲染要把焦点抢到盘面上。用 `Cell` 是因为 `render_board` 只有 `&self`，
    /// 而「开一次就抢一次」这件事必须在渲染时做（那时才有 window）。
    needs_focus: std::cell::Cell<bool>,
    /// 停在 Context 站点时，`← →` 选的是里面哪一半：`false` = 技能（左），`true` = 模板（右）。
    /// 那张卡里本来就是两个不等价的子块，用一个 Enter 说不清。
    ctx_half: bool,

    /// 只读 = 不给会写的工具（引擎侧摘 `WRITE_TOOLS`）。别的都一样：读文件、查目录、
    /// 回答问题全在。默认 `false` —— 开盘面是为了干活，不是先锁上再说。
    read_only: bool,

    pub(crate) sheet: Option<Sheet>,
    sheet_scroll: ScrollHandle,
    creating: bool,
}

impl NewChat {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let dir_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("/ 路径 · 或打一段名字筛这一层")
        });
        let name_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("Name")
        });
        let skill_query = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("技能名 · 描述 · 类别")
        });
        let ext_query = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("扩展名 · 描述 · 类别")
        });
        Self {
            open: false,
            focus: cx.focus_handle(),
            name_input,
            models: None,
            skills: None,
            prompts: None,
            machines: None,
            dirs: None,
            loading: false,
            load_err: None,
            flash: None,
            picked: None,
            level: "off".into(),
            picked_skills: BTreeSet::new(),
            template: None,
            machine: None,
            dir: None,
            dir_at: None,
            dir_input,
            skill_query,
            skill_cat: None,
            exts: None,
            ext_query,
            ext_cat: None,
            picked_exts: BTreeSet::new(),
            target: None,
            kb: None,
            ss_kb: None,
            ss_back: false,
            needs_focus: std::cell::Cell::new(false),
            ctx_half: false,
            read_only: false,
            sheet: None,
            sheet_scroll: ScrollHandle::new(),
            creating: false,
        }
    }

    /// 当前选中的模型（`provider/modelId`），给人看的那种写法。
    fn picked_label(&self) -> Option<String> {
        self.picked.as_ref().map(|(p, m)| format!("{p}/{m}"))
    }

    fn model(&self) -> Option<&Model> {
        let (p, m) = self.picked.as_ref()?;
        self.models
            .as_ref()?
            .items
            .iter()
            .find(|x| &x.provider == p && &x.model_id == m)
    }

    /// 这个模型支持的思考档（从少想到多想）。不支持思考档的给一个 `off`。
    fn levels(&self) -> Vec<String> {
        match self.model() {
            Some(m) => m.levels(),
            None => vec!["off".into()],
        }
    }

    /// 换模型后把档位夹回新模型的表里 —— 和 `pi-ai` 的 `clampThinkingLevel` 同序：
    /// 先往上找，找不到再往下。面板上看到的就是库里会落的值。
    fn clamp_level(&mut self) {
        let levels = self.levels();
        if levels.iter().any(|l| l == &self.level) {
            return;
        }
        let all = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
        let at = all.iter().position(|x| x == &self.level).unwrap_or(0);
        for cand in all.iter().skip(at) {
            if levels.iter().any(|l| l == cand) {
                self.level = (*cand).to_string();
                return;
            }
        }
        for cand in all.iter().take(at).rev() {
            if levels.iter().any(|l| l == cand) {
                self.level = (*cand).to_string();
                return;
            }
        }
        self.level = levels.first().cloned().unwrap_or_else(|| "off".into());
    }

    /// 能勾的技能：加载得起来、也不是 `disable-model-invocation` 那种。
    fn skill_list(&self) -> Vec<&Skill> {
        let mut out: Vec<&Skill> = self
            .skills
            .as_ref()
            .map(|s| {
                s.skills
                    .iter()
                    .filter(|x| x.loadable() && !x.hidden)
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// 技能弹层里「未分类」那一格的名字。类别路径不会长这样，直接当哨兵用。
    const UNCAT: &'static str = "未分类";

    /// 左树的全部节点：`(路径, 显示名, 缩进层级, 条数)`。
    ///
    /// 类别 = **引擎给的 `categories`**（含 `实验` / `Basics` 这种空类）∪ 技能自带的
    /// `category`。只算后者的话空类会消失，而空类恰恰是「这类还没东西」的唯一提示。
    fn skill_cats(&self) -> Vec<(String, String, usize, usize)> {
        let list = self.skill_list();
        let mut paths: Vec<String> = Vec::new();
        if let Some(sk) = self.skills.as_ref() {
            for c in &sk.categories {
                if !paths.contains(c) {
                    paths.push(c.clone());
                }
            }
        }
        for s in &list {
            for c in &s.category {
                if !paths.contains(c) {
                    paths.push(c.clone());
                }
            }
        }
        // **不排序**：引擎给 `categories` 的顺序是人排的，父类天然在子类前面
        // （`工程` → `工程/事故`）。按字符串排会把 `Basics` 顶到最前面。
        paths
            .into_iter()
            .map(|path| {
                let label = path.rsplit('/').next().unwrap_or(&path).to_string();
                let depth = path.matches('/').count();
                let count = list.iter().filter(|s| in_cat(&s.category, &path)).count();
                (path, label, depth, count)
            })
            .collect()
    }

    /// **能勾的**扩展：仓库里有、这台也装上、且不是核心。
    ///
    /// 核心四条不算 —— 引擎总是带上，客户端没有「减」的能力，
    /// 把它们算进「4/4」会让那颗数字说谎。
    fn ext_pickable(&self) -> Vec<String> {
        let Some(x) = self.exts.as_ref() else {
            return Vec::new();
        };
        let core = core_exts(x);
        x.catalog
            .iter()
            .filter(|e| x.installed.contains(&e.name))
            .filter(|e| !core.contains(&e.name))
            .map(|e| e.name.clone())
            .collect()
    }

    /// 加载失败的变体有几条 —— 卡上那个红点就靠它。
    fn ext_broken(&self) -> usize {
        self.exts
            .as_ref()
            .map(|x| {
                x.variants
                    .values()
                    .flatten()
                    .filter(|v| v.load.as_ref().and_then(|l| l.state.as_deref()) == Some("failed"))
                    .count()
            })
            .unwrap_or(0)
    }

    fn prompt_list(&self) -> Vec<&Prompt> {
        let mut out: Vec<&Prompt> = self
            .prompts
            .as_ref()
            .map(|p| p.prompts.iter().filter(|x| x.loadable()).collect())
            .unwrap_or_default();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// 圆底下那行状态。**只说现在差什么**，不说欢迎语，也不重复卡片上的值。
    ///
    /// 就绪时把「落在哪儿」一并带出来（机器 + 目录尾巴）—— 这颗圆就是落子按钮，
    /// 点下去之前人该看得见自己落在哪台机器、哪个目录。
    fn hub_sub(&self) -> String {
        if let Some(err) = &self.load_err {
            return err.clone();
        }
        if !self.open {
            return String::new();
        }
        if self.creating {
            return "creating…".into();
        }
        if self.models.is_none() {
            return if self.loading {
                "loading…".into()
            } else {
                "no models".into()
            };
        }
        match self.picked {
            Some(_) => {
                let mut parts = vec!["ready".to_string()];
                parts.push(self.machine.clone().unwrap_or_else(|| "本机".into()));
                if let Some(d) = &self.dir {
                    parts.push(tail_path(d));
                }
                parts.join(" · ")
            }
            None => "pick a model".into(),
        }
    }

    /// 选完了吗（只看选没选模型 / 是不是正在提交）。
    fn picked_ready(&self) -> bool {
        !self.creating && self.picked.is_some()
    }
}

/// 路径尾巴：`/a/b/c` → `c`。椭圆里放不下整串。
fn tail_path(path: &str) -> String {
    let p = path.trim_end_matches('/');
    p.rsplit('/').next().unwrap_or(p).to_string()
}

/// 思考档给人看的名字。`off` 在面板上就是「不思考」，不填就是不思考 —— 不撒谎。
fn level_label(level: &str) -> &'static str {
    match level {
        "off" => "不思考",
        "minimal" => "极少",
        "low" => "低",
        "medium" => "中",
        "high" => "高",
        "xhigh" => "很高",
        "max" => "最大",
        _ => "不思考",
    }
}

// ── 动作（全部挂在 `Desktop` 上：状态在这一层，盘面只是它的一块渲染）──────────

impl Desktop {
    pub(crate) fn board_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 每次开盘面都是一次新的落子：**换一只新的输入框**，不留上次没发出去的名字
        // （`TextareaState` 没有「清空」的口子，重建比绕路干净）。
        self.newchat.name_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("Name")
        });
        self.newchat.dir_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("/ 路径 · 或打一段名字筛这一层")
        });
        self.newchat.skill_query = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("技能名 · 描述 · 类别")
        });
        self.newchat.skill_cat = None;
        self.newchat.ext_query = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("扩展名 · 描述 · 类别")
        });
        self.newchat.ext_cat = None;
        self.newchat.open = true;
        self.newchat.sheet = None;
        self.newchat.target = None;
        self.newchat.flash = None;
        self.newchat.kb = None;
        self.newchat.needs_focus.set(true);
        window.focus(&self.newchat.focus, cx);
        cx.notify();
        if self.newchat.models.is_none() && !self.newchat.loading {
            self.board_load(cx);
        }
    }

    /// 下一次渲染把焦点抢到盘面上（调试入口用；正常入口在 [`NewChat::board_open`] 里）。
    pub(crate) fn request_focus(&self) {
        self.newchat.needs_focus.set(true);
    }

    /// 会话页改的是**已经存在的会话**：改一处推一处，别攒到关弹层 ——
    /// 攒着的话「改完直接按 Esc」就丢一次，而用户看不出来。
    ///
    /// 推的是全量（模型 / 档位 / 技能 / 模板 / 只读），服务端自己算差集。
    /// 回包是**生效值**（换模型会重夹档位），拿它覆盖本地，别乐观假定写成功了。
    fn cfg_push(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.newchat.target else { return };
        let Some(engine) = self.engine.clone() else { return };
        let Some((provider, model_id)) = self.newchat.picked.clone() else {
            return;
        };
        let patch = ConfigPatch {
            model: Some(format!("{provider}/{model_id}")),
            thinking_level: Some(self.newchat.level.clone()),
            skills: Some(self.newchat.picked_skills.iter().cloned().collect()),
            template: Some(self.newchat.template.clone()),
            read_only: Some(self.newchat.read_only),
            machine: None,
            cwd: None,
        };
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            let got = h.spawn(async move { engine.config(id, &patch).await }).await.ok();
            let _ = this.update(cx, |this, cx| {
                match got {
                    Some(Ok(cfg)) => {
                        // 生效值覆盖本地这条会话：别自己把「点的那份」写上去。
                        if let Some(s) = this.sessions.iter_mut().find(|s| s.id == id) {
                            s.model = cfg.model.clone();
                            s.thinking_level = cfg.thinking_level.clone();
                            s.skills = cfg.skills.clone();
                            s.template = cfg.template.clone();
                            s.read_only = cfg.read_only;
                        }
                        this.newchat.read_only = cfg.read_only;
                        if let Some(lv) = cfg.thinking_level.as_ref().and_then(|v| v.as_str()) {
                            this.newchat.level = lv.to_string();
                        }
                        this.newchat.flash = None;
                    }
                    Some(Err(e)) => {
                        this.newchat.flash = Some(format!("改配置失败：{e}"));
                    }
                    None => this.newchat.flash = Some("改配置失败了".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 在会话页打开「这个会话带什么」：把这条会话的当前值当成草稿装进 `newchat`
    /// —— 弹层只认这一套状态，省得给每张卡再写一份「只读版」。
    pub(crate) fn open_session_sheet(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.newchat.target = Some(id);
        if let Some((p, m)) = s.model.as_deref().and_then(|m| m.split_once('/')) {
            self.newchat.picked = Some((p.to_string(), m.to_string()));
        }
        if let Some(lv) = s.thinking_level.as_ref().and_then(|v| v.as_str()) {
            self.newchat.level = lv.to_string();
        }
        self.newchat.picked_skills = s.skills.iter().cloned().collect();
        self.newchat.template = s.template.clone();
        self.newchat.read_only = s.read_only;
        self.newchat.sheet = Some(Sheet::Session);
        // 每次开都从「没停过」起步：上一次停在 PERMS 的话，这次一按 Tab
        // 会从 MODEL 开始，看起来像在原地没动。
        self.newchat.ss_kb = None;
        self.newchat.ss_back = false;
        // 会话页从来没拉过清单（`board_load` 是开盘面时调的）：不拉的话点开模型 / 模板
        // 会看到「这个盘上还没有提示词模板」——而它只是没人去问过引擎。
        if self.newchat.models.is_none() && !self.newchat.loading {
            self.board_load(cx);
        }
        cx.notify();
    }

    /// 按得动 Create 吗。**按钮的长相和点击都吃这一份判定**，别两处各判一次。
    fn can_create(&self) -> bool {
        self.newchat.picked_ready() && self.engine.is_some()
    }

    pub(crate) fn board_close(&mut self, cx: &mut Context<Self>) {
        self.newchat.open = false;
        self.newchat.sheet = None;
        self.newchat.flash = None;
        cx.notify();
    }

    /// 五份清单一起拉，但**各自吃自己的错**：技能仓库挂了，模型照样能选。
    pub(crate) fn board_load(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.newchat.load_err = Some("还没连上引擎".into());
            cx.notify();
            return;
        };
        self.newchat.loading = true;
        self.newchat.load_err = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            type Got<T> = Result<T, EngineError>;
            let (models, skills, prompts, machines, dirs, exts) = h
                .spawn(async move {
                    let models: Got<Models> = engine.models().await;
                    let skills: Got<Skills> = engine.skills().await;
                    let prompts: Got<Prompts> = engine.prompts().await;
                    let machines: Got<Machines> = engine.machines().await;
                    let exts: Got<Extensions> = engine.extensions().await;
                    // start dirs 只给「map 根」这一条。拿它自己再列一次，
                    // 否则打开弹层只有孤零零一行，看着像坏了。
                    let dirs: Got<Dirs> = match engine.dirs(None, None).await {
                        Ok(d) if d.path.as_deref().unwrap_or("").is_empty() => {
                            match d.dirs.first().map(|x| x.path.clone()) {
                                Some(root) => engine.dirs(Some(&root), None).await,
                                None => Ok(d),
                            }
                        }
                        other => other,
                    };
                    (models, skills, prompts, machines, dirs, exts)
                })
                .await
                .unwrap_or_else(|_| {
                    (
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                    )
                });

            let _ = this.update(cx, |this, cx| {
                let b = &mut this.newchat;
                b.loading = false;
                if let Ok(m) = models {
                    // 默认模型：盘上那只眼指着的那个；没有就第一个。
                    b.picked = m
                        .default
                        .as_ref()
                        .map(|d| (d.provider.clone(), d.model_id.clone()));
                    if b.picked.is_none() {
                        b.picked = m
                            .items
                            .first()
                            .map(|x| (x.provider.clone(), x.model_id.clone()));
                    }
                    b.models = Some(m);
                    b.clamp_level();
                } else if let Err(e) = models {
                    b.load_err = Some(format!("模型清单：{e}"));
                }
                if let Ok(s) = skills {
                    b.skills = Some(s);
                }
                if let Ok(p) = prompts {
                    // 仓库默认值只用来预勾；选不选仍然是人的事。
                    b.template = p.default.clone();
                    b.prompts = Some(p);
                }
                if let Ok(mm) = machines {
                    b.machines = Some(mm);
                }
                if let Ok(x) = exts {
                    // 核心四条**无条件在**（引擎总是带上，勾掉是假动作，见 ExtKind::Core）。
                    // 其余 = 这台装上且能加载的。用户改过就不动。
                    if b.picked_exts.is_empty() {
                        b.picked_exts = x
                            .loadable
                            .iter()
                            .cloned()
                            .chain(core_exts(&x))
                            .collect();
                    }
                    b.exts = Some(x);
                }
                if let Ok(d) = dirs {
                    // 起始目录**同时也是选中值**：不选也是落在那里，与其显示 `—`
                    // 让人以为还没定，不如把真实落点写出来。
                    // `path` 为空串 = board map 根（不是可选目录本身），这时落点取它
                    // 底下第一个条目 —— 也就是引擎在 `/v1/device.startDirs` 里给的那个。
                    b.dir_at = d.path.clone();
                    b.dir = match d.path.as_deref() {
                        Some(p) if !p.is_empty() => Some(p.to_string()),
                        _ => d.dirs.first().map(|x| x.path.clone()),
                    };
                    b.dirs = Some(d);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 回到「这台机器」的起点：列 start dirs，再展开第一条。
    ///
    /// 换机器后走这里 —— 旧路径在对面没意义，得从它的起点重新看。
    fn board_dir_start(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let machine = self.newchat.machine.clone();
        self.newchat.dir = None;
        self.newchat.dir_at = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move {
                    match engine.dirs(None, machine.as_deref()).await {
                        Ok(d) => match d.dirs.first().map(|x| x.path.clone()) {
                            Some(root) => engine.dirs(Some(&root), machine.as_deref()).await,
                            None => Ok(d),
                        },
                        Err(e) => Err(e),
                    }
                })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                match got {
                    Some(Ok(d)) => {
                        this.newchat.dir_at = d.path.clone();
                        this.newchat.dir = d.path.clone().filter(|p| !p.is_empty());
                        this.newchat.dirs = Some(d);
                    }
                    Some(Err(e)) => this.newchat.flash = Some(format!("列目录失败：{e}")),
                    None => this.newchat.flash = Some("列目录失败了".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 点目录里的某一层：**只导航，不选中** —— 选不选由底下那颗「用这个目录」
    /// 决定。以前点即选，和那颗按钮说的是同一件事。
    fn board_dir_enter(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let machine = self.newchat.machine.clone();
        self.newchat.dir_at = Some(path.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            let got = h
                .spawn(async move { engine.dirs(Some(&path), machine.as_deref()).await })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                match got {
                    Some(Ok(d)) => {
                        this.newchat.dir_at = d.path.clone();
                        this.newchat.dirs = Some(d);
                    }
                    Some(Err(e)) => this.newchat.flash = Some(format!("列目录失败：{e}")),
                    None => this.newchat.flash = Some("列目录失败了".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Create：一次发完（模型 / 档位 / 技能 / 模板 / 目录 / 机器 / 名字），
    /// 回来再把盘上那只眼切过去，然后回列表、选中它。
    fn board_create(&mut self, cx: &mut Context<Self>) {
        if self.newchat.creating {
            return;
        }
        let Some((provider, model_id)) = self.newchat.picked.clone() else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            self.newchat.flash = Some("还没连上引擎".into());
            cx.notify();
            return;
        };

        let name = self.newchat.name_input.read(cx).value().to_string();
        let name = name.trim().to_string();
        let body = CreateSession {
            name: (!name.is_empty()).then_some(name),
            model: Some(serde_json::json!({
                "provider": provider,
                "modelId": model_id,
            })),
            thinking_level: Some(serde_json::json!(self.newchat.level)),
            skills: (!self.newchat.picked_skills.is_empty())
                .then(|| self.newchat.picked_skills.iter().cloned().collect()),
            // **总是发**：`Some(None)` 明确就是「不要模板」。
            template: Some(self.newchat.template.clone()),
            // 清单读到过就总是发（`[]` 也是「明确一条都不要」），没读到才让它用引擎默认。
            extensions: self.newchat.exts.as_ref().map(|_| {
                self.newchat.picked_exts.iter().cloned().collect::<Vec<_>>()
            }),
            cwd: self.newchat.dir.clone(),
            machine: self.newchat.machine.clone(),
            // **总是发**：`false` 也是有效值（= 明确可写）。服务端默认虽然也是可写，
            // 但让客户端自己声明一遍，省得哪天默认值改了这边跟着变。
            read_only: Some(self.newchat.read_only),
            ..Default::default()
        };

        self.newchat.creating = true;
        self.newchat.flash = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            let outcome = h
                .spawn(async move {
                    let created: CreatedSession = engine.create(&body).await?;
                    // 创建成功之后才切那只眼；切不过去不该把这个会话判死。
                    let _ = engine.set_selection(Some(created.id)).await;
                    let sessions = engine.sessions().await?.items;
                    Ok::<_, EngineError>((created, sessions))
                })
                .await
                .ok();

            let _ = this.update(cx, |this, cx| {
                this.newchat.creating = false;
                match outcome {
                    Some(Ok((created, sessions))) => {
                        let mut sessions = sessions;
                        sessions.sort_by_key(|s| std::cmp::Reverse(s.id));
                        this.sessions = sessions;
                        // 服务端夹过的档位才是生效值：认它，不认我们发上去的那份。
                        if let Some(lv) = created.thinking_level.as_ref().and_then(|v| v.as_str())
                        {
                            this.newchat.level = lv.to_string();
                        }
                        // 勾了但没装成的，别静默跳过 —— 跳进会话之后盘面就关了，
                        // 这条只能走轻提示（红），把没落下的东西点名。
                        let mut skipped: Vec<String> = Vec::new();
                        if !created.missing.is_empty() {
                            skipped.push(format!("技能：{}", created.missing.join("、")));
                        }
                        if !created.missing_templates.is_empty() {
                            skipped.push(format!(
                                "模板：{}",
                                created.missing_templates.join("、")
                            ));
                        }
                        if !created.missing_extensions.is_empty() {
                            skipped.push(format!(
                                "扩展：{}",
                                created.missing_extensions.join("、")
                            ));
                        }
                        this.newchat.flash = None;
                        if !skipped.is_empty() {
                            this.notify(
                                format!("这些没装，已跳过 —— {}", skipped.join("；")),
                                NoticeKind::Error,
                                cx,
                            );
                        }
                        this.newchat.open = false;
                        this.newchat.sheet = None;
                        this.tab = crate::app::Tab::Chats;
                        this.select_session(created.id, true, cx);
                    }
                    Some(Err(e)) => this.newchat.flash = Some(format!("创建失败：{e}")),
                    None => this.newchat.flash = Some("创建失败了".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── 键盘 ──────────────────────────────────────────────────────────

    /// 盘面的键盘。**五个停靠点**（圆 + 四张卡），Tab 一圈。
    ///
    /// 只绑这些：Tab 走站、Enter 按下去、`← →` 在 Context 里分两半、`⌘⏎` 直接建。
    /// 不做几何方向键 —— 四个角加一个中心，上下左右怎么走都能讲出一套道理，
    /// 而讲不出一套所有人不用想就能用的。
    pub(crate) fn board_key(&mut self, ks: &Keystroke, cx: &mut Context<Self>) -> bool {
        let key = ks.key.as_str();
        let mods = ks.modifiers;

        // 回车有两个名字：GPUI 自己的键位叫 `enter`，X11 传上来的 keysym 有时是 `return`。
        // 只认一个的话，键盘流在真机上会「Tab 走得了、回车按不动」。
        let enter = key == "enter" || key == "return";

        // ⌘⏎ / Ctrl+⏎：盘面上唯一值得记一个组合键的动作。
        if enter && (mods.platform || mods.control) {
            self.board_create(cx);
            return true;
        }

        // 弹层开着的时候，盘面的键位全让路（Esc 例外，它归弹层）。
        if self.newchat.sheet.is_some() {
            if key == "escape" {
                self.newchat.sheet = None;
                cx.notify();
                return true;
            }
            return false;
        }

        match key {
            "escape" => {
                self.board_close(cx);
                return true;
            }
            "tab" => {
                // 还没停过就从最后一站起步，第一次 Tab 落到圆上。
                let cur = self.newchat.kb.unwrap_or(Stop::Perms);
                self.newchat.kb = Some(cur.step(mods.shift));
                cx.notify();
                return true;
            }
            "left" | "right" if self.newchat.kb == Some(Stop::Context) => {
                self.newchat.ctx_half = key == "right";
                cx.notify();
                return true;
            }
            _ if enter => {
                let Some(stop) = self.newchat.kb else {
                    return false;
                };
                match stop {
                    Stop::Create => self.board_create(cx),
                    Stop::Model => {
                        self.newchat.sheet = Some(Sheet::Model);
                        cx.notify();
                    }
                    Stop::Context => {
                        self.newchat.sheet =
                            Some(if self.newchat.ctx_half { Sheet::Prompt } else { Sheet::Skills });
                        cx.notify();
                    }
                    Stop::Ext => {
                        self.newchat.sheet = Some(Sheet::Ext);
                        cx.notify();
                    }
                    // 只有一个来回，Enter 就是那个来回。
                    Stop::Perms => {
                        self.newchat.read_only = !self.newchat.read_only;
                        cx.notify();
                    }
                }
                return true;
            }
            _ => {}
        }
        false
    }

    /// 会话页弹层的 Esc：子弹层退回四行摘要，摘要在最上面才真关掉。
    ///
    /// 判据是 [`NewChat::ss_back`]（「这一层是从摘要点进来的」）+ `target.is_some()`
    /// （会话页才算数，盘面上 `sheet = None` 就是回盘面，不需要中间这一层）。
    ///
    /// 返回 `true` = 这个 Esc 归这里管，调用方 `stop_propagation`。
    pub(crate) fn sheet_esc(&mut self, cx: &mut Context<Self>) -> bool {
        if self.newchat.open || self.newchat.sheet.is_none() {
            return false;
        }
        if self.newchat.ss_back && self.newchat.target.is_some() {
            self.newchat.sheet = Some(Sheet::Session);
            self.newchat.ss_back = false;
        } else {
            self.newchat.sheet = None;
        }
        cx.notify();
        true
    }

    /// 会话页「这个会话带什么」弹层里的键盘流：Tab / ↑↓ 走四行，⏎ 按下去。
    ///
    /// 跟 [`Self::board_key`] 是**两套**：盘面开着时 `open` 为真，会话页弹层只在
    /// 看一条真会话时开，两者不会同时在场。共用一个 `kb` 反而会互相干扰
    /// （盘面那次 Tab 停在 Perms，回会话页一按 Tab 就从 PERMS 起步）。
    ///
    /// **接管范围是收着的**，因为这个弹层没有 scrim：下面的聊天输入框照样能聚焦。
    /// - `Tab` 弹层开着就接管 —— 否则焦点会跳到下一个可聚焦元素，人看着像「按了没反应」。
    /// - `↑↓⏎` 只在**已经动过键盘**（`ss_kb` 有值）之后才接管，不按 Tab 就完全不影响打字。
    /// - 空格不接管：它跟 Enter 一个意思，但输入框里按空格太常见了，不值得为省一个键惹这个麻烦。
    ///
    /// 返回 `false` = 这个键不归这里管，调用方照旧往下传。
    pub(crate) fn sess_row_key(&mut self, ks: &Keystroke, cx: &mut Context<Self>) -> bool {
        if self.newchat.open || self.newchat.sheet != Some(Sheet::Session) {
            return false;
        }
        let key = ks.key.as_str();
        let mods = ks.modifiers;
        // X11 传上来的回车有时叫 `return`（跟 `board_key` 里那条同一个理由）。
        let enter = key == "enter" || key == "return";

        if key == "tab" {
            // 还没停过就从最后一行起步，第一次 Tab 落到 MODEL 上。
            let cur = self.newchat.ss_kb.unwrap_or(SessRow::Perms);
            self.newchat.ss_kb = Some(cur.step(mods.shift));
            cx.notify();
            return true;
        }
        if self.newchat.ss_kb.is_none() {
            return false;
        }
        match key {
            "up" | "down" => {
                let cur = self.newchat.ss_kb.unwrap_or(SessRow::Model);
                self.newchat.ss_kb = Some(cur.step(key == "up"));
                cx.notify();
                true
            }
            _ if enter => {
                let Some(row) = self.newchat.ss_kb else {
                    return false;
                };
                match row {
                    SessRow::Model => self.newchat.sheet = Some(Sheet::Model),
                    // 「模板」这行开的是提示词弹层（`Sheet::Prompt`），跟鼠标点同一个落点。
                    SessRow::Template => self.newchat.sheet = Some(Sheet::Prompt),
                    SessRow::Skills => self.newchat.sheet = Some(Sheet::Skills),
                    SessRow::Perms => {
                        self.newchat.read_only = !self.newchat.read_only;
                        // 会话页改的是真会话：改一处推一处（跟鼠标那条一样）。
                        self.cfg_push(cx);
                    }
                }
                // 进的是子弹层就记一笔，Esc 才知道该退回摘要。
                self.newchat.ss_back = self.newchat.sheet != Some(Sheet::Session);
                cx.notify();
                true
            }
            _ => false,
        }
    }

    // ── 渲染 ──────────────────────────────────────────────────────────

    /// 整块盘面。**顶替**右侧聊天区那一格（`app.rs` 的 render 里换掉 `render_chat`）。
    pub(crate) fn render_board(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // 盘面一开就把焦点抢过来 —— 不然 `track_focus` 白挂，Tab / ⏎ 一个都收不到。
        // 走 `needs_focus` 而不是在 `board_open` 里直接 focus：那条调试路径
        // （`NOUS_OPEN_BOARD=1`）手上没有 window，两条路得落在同一处。
        // 焦点丢了就抢回来：没有窗口管理器的 X 环境里，窗口一激活 GPUI 会把元素焦点清空，
        // 结果 `track_focus` 白挂、Tab / ⏎ 一个都收不到。只在**谁都不要焦点**的时候抢 ——
        // 输入框拿着焦点时不动（否则打字打到一半焦点会被拽走）。
        let nobody = self.newchat.open && self.newchat.sheet.is_none() && window.focused(cx).is_none();
        if self.newchat.needs_focus.replace(false) || nobody {
            window.focus(&self.newchat.focus, cx);
        }
        // 基准是**右格**不是整窗：盘面现在是三栏里的一格。
        let m = art::M::new(window.viewport_size(), self.tab);
        let b = &self.newchat;
        let model_value = b.picked_label().unwrap_or_else(|| "—".into());

        // 窄到放不下「四角 + 圆 + 环」时换竖排，别把同一套硬塞进去。
        if m.layout == art::Layout::Stack {
            return div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(t().d(0.))
                .h_full()
                .bg(rgb(art::bg()))
                .text_color(rgb(art::fg()))
                .text_size(m.f(1.75, 12., 16.))
                .track_focus(&b.focus)
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                    if this.board_key(&ev.keystroke, cx) {
                        cx.stop_propagation();
                    }
                }))
                .child(self.board_stack(&m, cx))
                .children(self.board_sheet(&m, cx))
                .into_any_element();
        }

        let body = div()
            .relative()
            .flex_1()
            .min_h(t().d(0.))
            .child(self.board_wires(&m))
            .child(self.board_core(&m, cx))
            .child(self.board_card(
                &m,
                art::Slot::Model,
                "board-model",
                "MODEL",
                "Model",
                model_value,
                "change ›",
                b.models.is_some(),
                cx,
                |this, _cx| this.newchat.sheet = Some(Sheet::Model),
            ))
            .child(self.board_context(&m, cx))
            .child(self.board_ext(&m, cx))
            .child(self.board_perms(&m, cx))
            .child(self.board_feet(&m, cx))
            .children(self.board_status(&m));

        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            // flex 项不写 min_w(0) 会被内容撑到比右格还宽（坑 16）。
            .min_w(t().d(0.))
            .h_full()
            .bg(rgb(art::bg()))
            .text_color(rgb(art::fg()))
            .text_size(m.f(1.75, 12., 16.))
            .line_height(relative(1.5))
            .track_focus(&b.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if this.board_key(&ev.keystroke, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(body)
            .children(self.board_sheet(&m, cx))
            .into_any_element()
    }

    /// 所有连线：四条「圆心 → 卡片」、一圈环、三条「圆边 → 环上的值」。
    ///
    /// 一次 `canvas` 画完：它们同属一层，分开画只会多几次 paint。
    fn board_wires(&self, m: &art::M) -> AnyElement {
        let m = *m;
        struct Wires {
            cards: [((f32, f32), (f32, f32)); 4],
            ring: (f32, f32, f32, f32),
            feet: [((f32, f32), (f32, f32)); 3],
        }
        let slots = [
            art::Slot::Model,
            art::Slot::Context,
            art::Slot::Ext,
            art::Slot::Perms,
        ];
        let feet = [art::Foot::Machine, art::Foot::Name, art::Foot::Dir];
        canvas(
            move |_bounds, _, _| Wires {
                cards: slots.map(|s| art::card_wire(&m, s)),
                ring: if art::has_ring(&m) {
                    art::ring(&m)
                } else {
                    (0., 0., 0., 0.)
                },
                feet: feet.map(|f| art::foot_wire(&m, f)),
            },
            |bounds, w, window, _| {
                // `paint_path` 吃的是**绝对坐标**：canvas 的原点不一定在 (0,0)，
                // 忘了加就是整条线平移一个输出区偏移（坑 34）。
                let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                let at = |p: (f32, f32)| point(px(ox + p.0), px(oy + p.1));

                // 四条卡片连线：短划。
                for (a, b) in w.cards {
                    let mut pb = PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(6.)]);
                    pb.move_to(at(a));
                    pb.line_to(at(b));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, rgb(art::wire()));
                    }
                }

                // 环：点线（比卡片连线更短更密，才像一圈轨道而不是四条线之一）。
                // 紧凑档没有环 —— 那里放不下，硬画只会和卡片打架。
                if w.ring.2 > 0. {
                // 用折线逼近椭圆（48 段）：点线渲染下看不出多边形，但比手写
                // 四段贝塞尔少一个出错的地方。
                let (cx, cy, rx, ry) = w.ring;
                let steps = 48;
                let mut pb = PathBuilder::stroke(px(1.2)).dash_array(&[px(2.), px(6.)]);
                for i in 0..=steps {
                    let t = i as f32 / steps as f32 * std::f32::consts::TAU;
                    let p = at((cx + rx * t.cos(), cy + ry * t.sin()));
                    if i == 0 {
                        pb.move_to(p);
                    } else {
                        pb.line_to(p);
                    }
                }
                pb.close();
                if let Ok(path) = pb.build() {
                    window.paint_path(path, rgb(art::accent()));
                }
                }

                // 三条短线：圆边 → 环上的三颗珠子。
                for (a, b) in w.feet {
                    let mut pb = PathBuilder::stroke(px(1.2)).dash_array(&[px(3.), px(5.)]);
                    pb.move_to(at(a));
                    pb.line_to(at(b));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, rgb(art::accent()));
                    }
                }
            },
        )
        .size_full()
        .into_any_element()
    }

    /// 中央那颗白圆：整页唯一的主动作。
    fn board_core(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let (ccx, ccy) = art::core_center(&m);
        let r = art::core_r(&m);
        let halo = 18.;
        let b = &self.newchat;
        // 按不动和按得动**长两副样子**：灰着的时候不挂点击，点下去也不会有反应 ——
        // 状态行已经在说「差一个模型」，按钮不必再弹一次同样的字。
        let live = self.can_create();
        let busy = b.creating;
        // 紧凑 / 竖排档的圆更小，那行英文会溢出去。
        let tight = m.layout != art::Layout::Wide;

        div()
            .absolute()
            .left(px(ccx - r - halo))
            .top(px(ccy - r - halo))
            .w(px(2. * (r + halo)))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w(px(2. * (r + halo)))
                    .h(px(2. * (r + halo)))
                    .rounded_full()
                    .bg(rgb(art::solid()).opacity(0.05))
                    // 键盘停在圆上时，光环变成一圈实线框。
                    .when(b.kb == Some(Stop::Create), |el| {
                        el.border_2().border_color(rgb(art::accent()))
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id(ElementId::Name("board-create".into()))
                            .size(px(2. * r))
                            .rounded_full()
                            // 圆用正文色当底、盘面底色当字：浅皮就是黑圆白字，
                            // 深皮是白圆黑字 —— 五套皮都成立，不会长出一颗怪色按钮。
                            .bg(if live || busy {
                                rgb(art::solid())
                            } else {
                                rgb(art::surface())
                            })
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .text_color(if live || busy {
                                rgb(art::bg())
                            } else {
                                rgb(art::faint())
                            })
                            .when(busy, |el| el.opacity(0.72))
                            .when(live, |el| {
                                el.hover(|s| s.opacity(0.94))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.newchat.kb = Some(Stop::Create);
                                        this.board_create(cx)
                                    }))
                            })
                            .child(
                                div()
                                    .text_size(m.f(1.6, 12., 17.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(if busy { "Creating…" } else { "+ Create" }),
                            )
                            // 小圆放不下那行英文：紧凑/竖排档只留 `+ Create`。
                            .when(!busy && !tight, |el| {
                                el.child(
                                    div()
                                        .mt(m.v(0.3))
                                        .font_family(art::mono())
                                        .text_size(m.f(0.7, 6.5, 8.5))
                                        .text_color(rgb(art::bg()).opacity(0.55))
                                        .child("NEW MANUAL · DISORDER"),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 一张参数卡（Model / Extensions / Permissions 共用这个形状）。
    #[allow(clippy::too_many_arguments)]
    fn board_card(
        &self,
        m: &art::M,
        slot: art::Slot,
        id: &'static str,
        k: &'static str,
        title: &'static str,
        value: String,
        tag: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let m = *m;
        let r = art::node_rect(&m, slot);
        div()
            .id(ElementId::Name(id.into()))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .gap(m.v(0.25))
            .px(m.v(1.1))
            .py(m.v(0.85))
            .bg(rgb(art::panel()))
            .border_1()
            .border_color(rgb(art::border()))
            .rounded(t().r(10.))
            .when(enabled, |el| {
                el.hover(|s| s.border_color(rgb(art::line())))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        // 鼠标点哪张卡，键盘就停在哪张 —— 两种操作同一套状态。
                        this.newchat.kb = Some(Stop::of(slot));
                        on_click(this, cx);
                    }))
            })
            .when(!enabled, |el| el.opacity(0.55))
            // 焦点框压在最上面：键盘停在这一格时，别的描边都让位。
            // 2px 而不是只换色：`mono` 皮的 accent 跟边线同为白色，只有粗细能说话。
            .when(self.newchat.kb == Some(Stop::of(slot)), |el| {
                el.border_2().border_color(rgb(art::accent()))
            })
            .child(
                div()
                    .font_family(art::mono())
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::faint()))
                    .child(k),
            )
            .child(
                div()
                    .text_size(m.f(1.6, 11., 14.))
                    .font_weight(FontWeight::BOLD)
                    .child(title),
            )
            .child(
                div()
                    .mt(m.v(0.45))
                    .min_w(t().d(0.))
                    .font_family(art::mono())
                    .text_size(m.f(1.3, 10., 12.))
                    .text_color(rgb(art::muted()))
                    .truncate()
                    .child(value),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(m.f(1.15, 8.5, 10.5))
                    .text_color(rgb(art::faint()))
                    .child(tag),
            )
            .into_any_element()
    }

    /// Extensions 那张卡：一行圆点，一条扩展一个。
    ///
    /// 数字说不清这张卡 —— 「4/4」看不出里面有没有坏东西，五个点一眼就看得出来。
    fn board_ext(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        let r = art::node_rect(&m, art::Slot::Ext);
        let pickable = b.ext_pickable();
        let picked = pickable
            .iter()
            .filter(|n| b.picked_exts.contains(*n))
            .count();
        let broken = b.ext_broken();
        let builtin = b.exts.as_ref().map(|x| x.builtin.len()).unwrap_or(0);

        let dot = |on: bool, hollow: bool| {
            div()
                .size(m.s(5.))
                .rounded_full()
                .border_1()
                .border_color(rgb(if on || hollow {
                    art::accent()
                } else {
                    art::border()
                }))
                .bg(if on {
                    rgb(art::accent())
                } else {
                    rgba(t().p.transparent)
                })
        };

        div()
            .id(ElementId::Name("board-ext".into()))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .gap(m.v(0.25))
            .px(m.v(1.1))
            .py(m.v(0.85))
            .bg(rgb(art::panel()))
            .border_1()
            .border_color(rgb(art::border()))
            .rounded(t().r(10.))
            .hover(|s| s.border_color(rgb(art::line())))
            .when(b.kb == Some(Stop::Ext), |el| {
                el.border_2().border_color(rgb(art::accent()))
            })
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.newchat.kb = Some(Stop::Ext);
                this.newchat.sheet = Some(Sheet::Ext);
                cx.notify();
            }))
            .child(
                div()
                    .font_family(art::mono())
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::faint()))
                    .child("EXTENSIONS"),
            )
            .child(
                div()
                    .text_size(m.f(1.6, 11., 14.))
                    .font_weight(FontWeight::BOLD)
                    .child("Extensions"),
            )
            .child(
                div()
                    .mt(m.v(0.5))
                    .flex()
                    .items_center()
                    .gap(m.v(0.4))
                    .children(
                        pickable
                            .iter()
                            .take(7)
                            .map(|n| dot(b.picked_exts.contains(n), false)),
                    )
                    .children((0..builtin.min(2)).map(|_| dot(false, true)))
                    .children((0..broken.min(2)).map(|_| {
                        div().size(m.s(5.)).rounded_full().bg(rgb(art::accent()))
                    }))
                    .child(
                        div()
                            .ml(m.v(0.3))
                            .font_family(art::mono())
                            .text_size(m.f(1.15, 8.5, 10.))
                            .text_color(rgb(art::muted()))
                            .child(format!("{picked}/{}", pickable.len())),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(m.f(1.15, 8.5, 10.5))
                    .text_color(rgb(art::faint()))
                    .child(if broken > 0 {
                        format!("{broken} 个加载失败 ›")
                    } else {
                        "choose ›".into()
                    }),
            )
            .into_any_element()
    }

    /// 会话页的「这个会话带什么」：四行摘要，点哪行就开哪张卡的弹层。
    ///
    /// 这里**不重复一套控件** —— 模型 / 模板 / 技能三行都是入口，真正的选择还是
    /// 盘面那三张弹层（它们已经写好了搜索、树、勾选）。唯一的例外是权限：
    /// 只有一个来回，点了就切，不绕。
    fn sheet_session(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        let model = b.picked_label().unwrap_or_else(|| "—".into());
        let template = b.template.clone().unwrap_or_else(|| "无模板".into());
        let skills = format!("{} / {}", b.picked_skills.len(), b.skill_list().len());
        let ro = b.read_only;

        // 一行 = 小标题 + 值 + `›`。三行开弹层，一行直接切。
        let row = |id: &'static str,
                   row_at: SessRow,
                   k: &'static str,
                   v: String,
                   sub: Option<String>,
                   cx: &mut Context<Self>,
                   act: Option<Sheet>,
                   toggle: bool,
                   hot: bool| {
            div()
                .id(ElementId::Name(id.into()))
                .flex()
                .items_center()
                .gap(m.v(1.))
                .px(m.v(1.6))
                .py(m.v(0.85))
                .border_b(t().d(1.))
                .border_color(rgb(art::border()))
                .hover(|s| s.bg(rgb(art::surface())))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    // 鼠标点=键盘停在这儿：不然点完再按 Tab 会从别处起步。
                    this.newchat.ss_kb = Some(row_at);
                    if toggle {
                        this.newchat.read_only = !this.newchat.read_only;
                        // 会话页改的是真会话：改一处推一处。
                        this.cfg_push(cx);
                    } else if let Some(sh) = act {
                        this.newchat.sheet = Some(sh);
                        // 进的是子弹层：Esc 退回四行摘要，不是一路关到底。
                        this.newchat.ss_back = true;
                    }
                    cx.notify();
                }))
                // 焦点是 2px 整圈 accent，不是只换个底色 —— 跟盘面那五站同一个做法。
                .when(hot, |el| el.border_2().border_color(rgb(art::accent())))
                .child(
                    div()
                        .flex_none()
                        .w(m.v(9.))
                        .font_family(art::mono())
                        .text_size(m.f(0.9, 7.5, 9.))
                        .text_color(rgb(art::faint()))
                        .child(k),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(t().d(0.))
                        .flex()
                        .flex_col()
                        .gap(m.v(0.1))
                        .child(
                            div()
                                .font_family(art::mono())
                                .text_size(m.f(1.2, 9.5, 11.))
                                .text_color(rgb(art::fg()))
                                .truncate()
                                .child(v),
                        )
                        .children(sub.map(|s| {
                            div()
                                .text_size(m.f(0.9, 7.5, 9.))
                                .text_color(rgb(art::muted()))
                                .truncate()
                                .child(s)
                        })),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(m.f(1., 8., 9.5))
                        .text_color(rgb(art::muted()))
                        .child("›"),
                )
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .child(row(
                "ss-model",
                SessRow::Model,
                "MODEL",
                model,
                None,
                cx,
                Some(Sheet::Model),
                false,
                b.ss_kb == Some(SessRow::Model),
            ))
            .child(row(
                "ss-template",
                SessRow::Template,
                "TEMPLATE",
                template,
                None,
                cx,
                Some(Sheet::Prompt),
                false,
                b.ss_kb == Some(SessRow::Template),
            ))
            .child(row(
                "ss-skills",
                SessRow::Skills,
                "SKILLS",
                skills,
                None,
                cx,
                Some(Sheet::Skills),
                false,
                b.ss_kb == Some(SessRow::Skills),
            ))
            .child(row(
                "ss-perms",
                SessRow::Perms,
                "PERMS",
                if ro { "只读" } else { "可写" }.into(),
                Some(if ro {
                    "只能读，改不了东西".into()
                } else {
                    "能改文件、能跑命令".into()
                }),
                cx,
                None,
                true,
                b.ss_kb == Some(SessRow::Perms),
            ))
            .into_any_element()
    }

    /// Permissions 那张卡：一个二值开关，**不弹层**。
    ///
    /// 引擎里没有「权限」这个概念（`/v1/permissions` 是 404），只有 `readOnly` ——
    /// 它做的事是把这个会话会写的工具摘掉。所以卡上要说的就一件事：这次能不能动东西。
    /// 两个选项不值当开一个弹层，整张卡就是那个开关。
    fn board_perms(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let ro = self.newchat.read_only;
        let r = art::node_rect(&m, art::Slot::Perms);
        div()
            .id(ElementId::Name("board-perms".into()))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .gap(m.v(0.25))
            .px(m.v(1.1))
            .py(m.v(0.85))
            .bg(rgb(art::panel()))
            .border_1()
            .border_color(rgb(art::border()))
            .rounded(t().r(10.))
            .hover(|s| s.border_color(rgb(art::line())))
            .when(self.newchat.kb == Some(Stop::Perms), |el| {
                el.border_2().border_color(rgb(art::accent()))
            })
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.newchat.kb = Some(Stop::Perms);
                this.newchat.read_only = !this.newchat.read_only;
                cx.notify();
            }))
            .child(
                div()
                    .font_family(art::mono())
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::faint()))
                    .child("PERMISSIONS"),
            )
            .child(
                div()
                    .text_size(m.f(1.6, 11., 14.))
                    .font_weight(FontWeight::BOLD)
                    .child("Permissions"),
            )
            .child(
                div()
                    .mt(m.v(0.45))
                    .font_family(art::mono())
                    .text_size(m.f(1.3, 10., 12.))
                    .text_color(rgb(art::fg()))
                    .child(if ro { "只读" } else { "可写" }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(m.v(0.6))
                    .child(
                        div()
                            .min_w(t().d(0.))
                            .text_size(m.f(1.15, 8.5, 10.5))
                            .text_color(rgb(art::faint()))
                            .truncate()
                            .child(if ro {
                                "只能读，改不了东西"
                            } else {
                                "能改文件、能跑命令"
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(m.f(1.15, 8.5, 10.5))
                            .text_color(rgb(art::muted()))
                            .child("切换 ›"),
                    ),
            )
            .into_any_element()
    }

    /// Context 那张卡：里面是**两个**可点子块 —— 技能是集合（多选），
    /// 模板是单值（单选），塞进一个按钮里会让人以为它们一样。
    fn board_context(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        let r = art::node_rect(&m, art::Slot::Context);
        let skills = format!("{} / {}", b.picked_skills.len(), b.skill_list().len());
        let prompt = b.template.clone().unwrap_or_else(|| "none".into());
        let prompt_summary = b.template.as_ref().and_then(|name| {
            b.prompt_list().into_iter().find(|p| &p.name == name).and_then(|p| {
                // `summary` 是模板仓库自己写的中文一句话；`description` 常常是
                // 英文长句。先给中文那句。
                p.summary
                    .clone()
                    .or_else(|| p.description.clone())
                    .map(|d| d.replace(['\n', '\r'], " "))
            })
        });
        let sub =
            |id: &'static str,
             k: &'static str,
             v: String,
             cx: &mut Context<Self>,
             open: Sheet,
             hot: bool| {
                div()
                    .id(ElementId::Name(id.into()))
                    .flex()
                    .flex_col()
                    .gap(m.v(0.2))
                    .flex_1()
                    .min_w(t().d(0.))
                    .px(m.v(0.7))
                    .py(m.v(0.45))
                    .border_1()
                    .border_color(rgb(art::border()))
                    .rounded(t().r(7.))
                    .bg(rgb(art::bg()))
                    .hover(|s| s.border_color(rgb(art::line())))
                    .when(hot, |el| el.border_2().border_color(rgb(art::accent())))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.newchat.sheet = Some(open);
                        // 点哪一半，键盘就停在哪一半。
                        this.newchat.kb = Some(Stop::Context);
                        this.newchat.ctx_half = open == Sheet::Prompt;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family(art::mono())
                            .text_size(m.f(1.05, 8., 10.))
                            .text_color(rgb(art::faint()))
                            .child(k),
                    )
                    .child(
                        div()
                            .font_family(art::mono())
                            .text_size(m.f(1.2, 9., 11.))
                            .text_color(rgb(art::fg()))
                            .truncate()
                            .child(v),
                    )
            };

        div()
            .id(ElementId::Name("board-context".into()))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .gap(m.v(0.45))
            .px(m.v(1.1))
            .py(m.v(0.85))
            .bg(rgb(art::panel()))
            .border_1()
            .border_color(rgb(art::border()))
            .rounded(t().r(10.))
            .when(b.kb == Some(Stop::Context), |el| {
                el.border_2().border_color(rgb(art::accent()))
            })
            .child(
                div()
                    .font_family(art::mono())
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::faint()))
                    .child("CONTEXT"),
            )
            .child(
                div()
                    .flex()
                    .gap(m.v(0.6))
                    .child(sub(
                        "board-skills",
                        "SKILLS",
                        skills,
                        cx,
                        Sheet::Skills,
                        b.kb == Some(Stop::Context) && !b.ctx_half,
                    ))
                    .child(sub(
                        "board-prompt",
                        "PROMPT",
                        prompt,
                        cx,
                        Sheet::Prompt,
                        b.kb == Some(Stop::Context) && b.ctx_half,
                    )),
            )
            .child(div().flex_1())
            // 选了模板就把它的摘要顶上来 —— 这句原本是句废话装饰，
            // 现在同一个位置能回答「我到底选了个什么」。
            .child(div().flex_none().truncate().child(
                div()
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(if prompt_summary.is_some() {
                        rgb(art::muted())
                    } else {
                        rgb(art::faint())
                    })
                    .truncate()
                    .child(prompt_summary.unwrap_or_else(|| "随会话注入的知识与语气".into())),
            ))
            .into_any_element()
    }

    /// 环上三个值（05 环）：名字挂在环底，机器 / 目录挂在环的左下和右下。
    ///
    /// 三颗 pill 的底色**和中央那颗圆一样**（`solid()` 当底、`bg()` 当字）——
    /// 它们不是三个独立控件，是从圆心甩出去的三颗珠子。换皮时一起换。
    fn board_feet(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        // 没选模型时三颗珠子一起变浅 —— 一个状态，一个长相。
        let live = self.can_create() || b.creating;
        let machine = b.machine.clone().unwrap_or_else(|| "本机".into());
        let dir = b
            .dir
            .as_ref()
            .map(|p| tail_path(p))
            .unwrap_or_else(|| "—".into());

        let chip = |id: &'static str, f: art::Foot, text: String, sheet: Sheet, cx: &mut Context<Self>| {
            let r = art::foot_rect(&m, f);
            div()
                .id(ElementId::Name(id.into()))
                .absolute()
                .left(px(r.x))
                .top(px(r.y))
                .w(px(r.w))
                .h(px(r.h))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(if live {
                    rgb(art::solid())
                } else {
                    rgb(art::surface())
                })
                .text_color(if live {
                    rgb(art::bg())
                } else {
                    rgb(art::muted())
                })
                .font_family(art::mono())
                .text_size(m.f(1.2, 9., 11.))
                .when(live, |el| el.hover(|s| s.opacity(0.88)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.sheet = Some(sheet);
                    cx.notify();
                }))
                .child(text)
        };

        let n = art::foot_rect(&m, art::Foot::Name);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .absolute()
                    .left(px(n.x))
                    .top(px(n.y))
                    .w(px(n.w))
                    .h(px(n.h))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(if live {
                        rgb(art::solid())
                    } else {
                        rgb(art::surface())
                    })
                    .px(m.v(1.0))
                    // 点进来打字，键盘就把停靠点交出去 —— 不然「⏎ 建会话」会把
                    // 用户在输入框里按的回车吃掉。
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.newchat.kb = None;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .text_color(if live {
                                rgb(art::bg())
                            } else {
                                rgb(art::muted())
                            })
                            .child(Textarea::new(&b.name_input).appearance(false).bordered(false)),
                    ),
            )
            .child(chip("foot-machine", art::Foot::Machine, machine, Sheet::Machine, cx))
            .child(chip("foot-dir", art::Foot::Dir, dir, Sheet::Dir, cx))
            .into_any_element()
    }

    /// 竖排档（右格 < 620）：圆在顶、三颗 pill 一行、四张卡一列，整列可滚。
    ///
    /// 这一档「一屏不滚」已经不可能 —— 300px 宽硬要塞四角 + 环只会互相盖住，
    /// 所以干脆承认它是个列表页。
    fn board_stack(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        let live = self.can_create() || b.creating;
        let r = art::core_r(&m);
        let machine = b.machine.clone().unwrap_or_else(|| "本机".into());
        let dir = b
            .dir
            .as_ref()
            .map(|p| tail_path(p))
            .unwrap_or_else(|| "—".into());
        let model = b.picked_label().unwrap_or_else(|| "—".into());
        let skills = format!("{} / {}", b.picked_skills.len(), b.skill_list().len());
        let prompt = b.template.clone().unwrap_or_else(|| "none".into());
        let pickable = b.ext_pickable();
        let picked = pickable.iter().filter(|n| b.picked_exts.contains(*n)).count();
        let broken = b.ext_broken();
        let ext = if broken > 0 {
            format!("{picked}/{} · {broken} 坏", pickable.len())
        } else {
            format!("{picked}/{}", pickable.len())
        };

        let pill = |id: &'static str, text: String, sheet: Sheet, cx: &mut Context<Self>| {
            div()
                .id(ElementId::Name(id.into()))
                .px(m.s(11.))
                .py(m.s(6.))
                .rounded_full()
                .bg(if live { rgb(art::solid()) } else { rgb(art::surface()) })
                .text_color(if live { rgb(art::bg()) } else { rgb(art::muted()) })
                .font_family(art::mono())
                .text_size(m.f(1.45, 10., 11.5))
                .when(live, |el| el.hover(|s| s.opacity(0.88)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.sheet = Some(sheet);
                    cx.notify();
                }))
                .child(text)
        };

        #[allow(clippy::too_many_arguments)]
        let row = |id: &'static str,
                   k: &'static str,
                   title: &'static str,
                   value: String,
                   sheet: Option<Sheet>,
                   toggle: bool,
                   hot: bool,
                   cx: &mut Context<Self>| {
            let mut el = div()
                .id(ElementId::Name(id.into()))
                .w_full()
                .px(m.s(13.))
                .py(m.s(11.))
                .mb(m.s(8.))
                .bg(rgb(art::panel()))
                .border_1()
                .border_color(rgb(art::border()))
                .rounded(t().r(9.))
                .when(sheet.is_some() || toggle, |el| {
                    el.hover(|s| s.border_color(rgb(art::line())))
                })
                .when(hot, |el| el.border_2().border_color(rgb(art::accent())))
                .child(
                    div()
                        .font_family(art::mono())
                        .text_size(m.f(0.95, 8., 9.5))
                        .text_color(rgb(art::faint()))
                        .child(k),
                )
                .child(
                    div()
                        .mt(m.s(4.))
                        .text_size(m.f(1.55, 11., 13.))
                        .font_weight(FontWeight::BOLD)
                        .child(title),
                )
                .child(
                    div()
                        .mt(m.s(3.))
                        .font_family(art::mono())
                        .text_size(m.f(1.25, 9., 11.))
                        .text_color(rgb(art::muted()))
                        .truncate()
                        .child(value),
                );
            if toggle {
                // 竖排档跟宽档同一个语义：整行就是那个开关，不开弹层。
                el = el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.newchat.read_only = !this.newchat.read_only;
                    cx.notify();
                }));
            } else if let Some(sh) = sheet {
                el = el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.sheet = Some(sh);
                    cx.notify();
                }));
            }
            el
        };

        div()
            .id(ElementId::Name("board-stack".into()))
            .flex()
            .flex_col()
            .items_center()
            .gap(m.s(6.))
            .px(m.s(16.))
            .py(m.s(18.))
            .flex_1()
            .min_h(t().d(0.))
            .overflow_y_scroll()
            .scrollbar_width(px(6.))
            .child(
                div()
                    .id(ElementId::Name("board-create-stack".into()))
                    .flex_none()
                    .size(px(2. * r))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(if live || b.creating {
                        rgb(art::solid())
                    } else {
                        rgb(art::surface())
                    })
                    .when(b.kb == Some(Stop::Create), |el| {
                        el.border_2().border_color(rgb(art::accent()))
                    })
                    .text_color(if live || b.creating {
                        rgb(art::bg())
                    } else {
                        rgb(art::faint())
                    })
                    .when(live, |el| {
                        el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.board_create(cx)))
                    })
                    .child(
                        div()
                            .text_size(m.f(1.7, 12., 14.))
                            .font_weight(FontWeight::BOLD)
                            .child(if b.creating { "Creating…" } else { "+ Create" }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(m.s(6.))
                    .child(pill("stk-machine", machine.clone(), Sheet::Machine, cx))
                    .child(pill("stk-dir", dir.clone(), Sheet::Dir, cx)),
            )
            .child(
                // 名字：竖排档给它一条整宽的输入，别塞进小 pill 里。
                div()
                    .flex_none()
                    .w_full()
                    .mt(m.s(4.))
                    .px(m.s(12.))
                    .py(m.s(9.))
                    .rounded(t().r(9.))
                    .bg(rgb(art::solid()))
                    .text_color(rgb(art::bg()))
                    .child(Textarea::new(&b.name_input).appearance(false).bordered(false)),
            )
            .child(div().flex_none().h(m.s(10.)))
            .child(row(
                "stk-model",
                "MODEL",
                "Model",
                model,
                Some(Sheet::Model),
                false,
                b.kb == Some(Stop::Model),
                cx,
            ))
            .child(row(
                "stk-ctx",
                "CONTEXT",
                "Skills · Prompt",
                format!("{skills} · {prompt}"),
                Some(if b.ctx_half { Sheet::Prompt } else { Sheet::Skills }),
                false,
                b.kb == Some(Stop::Context),
                cx,
            ))
            .child(row(
                "stk-ext",
                "EXTENSIONS",
                "Extensions",
                ext,
                Some(Sheet::Ext),
                false,
                b.kb == Some(Stop::Ext),
                cx,
            ))
            .child(row(
                "stk-perm",
                "PERMISSIONS",
                "Permissions",
                if b.read_only { "只读" } else { "可写" }.into(),
                None,
                true,
                b.kb == Some(Stop::Perms),
                cx,
            ))
            // 状态行放最底下：竖排档顶部已经够挤，它在那儿会被输入框压住。
            .child(
                div()
                    .flex_none()
                    .mt(m.s(6.))
                    .mb(m.s(8.))
                    .text_center()
                    .font_family(art::mono())
                    .text_size(m.f(1.15, 9., 10.5))
                    .text_color(rgb(if b.flash.is_some() {
                        art::danger()
                    } else {
                        art::ok()
                    }))
                    .child(b.flash.clone().unwrap_or_else(|| b.hub_sub())),
            )
            .into_any_element()
    }

    /// 环底下那行状态：`flash` 是失败（红调），否则说现在差什么。
    fn board_status(&self, m: &art::M) -> Option<AnyElement> {
        let b = &self.newchat;
        let (text, color) = match (&b.flash, b.hub_sub()) {
            (Some(f), _) => (f.clone(), rgb(art::danger())),
            (None, s) if !s.is_empty() => (s, rgb(art::ok())),
            _ => return None,
        };
        let y = art::status_y(m);
        Some(
            div()
                .absolute()
                .left(px(m.w * 0.5 - m.w * 0.25))
                .top(px(y))
                .w(px(m.w * 0.5))
                .text_center()
                .font_family(art::mono())
                .text_size(m.f(1.15, 8.5, 10.5))
                .text_color(color)
                .child(text)
                .into_any_element(),
        )
    }

    /// 弹层：「就地弹出」。**贴着被点的那张卡**，不是居中一个模态框 ——
    /// 选参数的时候还想看得见盘面（点空白处 / Esc 关闭）。
    pub(crate) fn board_sheet(&self, m: &art::M, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = *m;
        let sheet = self.newchat.sheet?;
        let (title, hint, slot) = match sheet {
            Sheet::Session => (
                "这个会话带什么",
                "从盘面挑的那几项 · 点一行就改",
                None,
            ),
            Sheet::Model => (
                "选择模型",
                "供应商 › 模型 › 思考 · 点模型即收回",
                Some(art::Slot::Model),
            ),
            Sheet::Skills => (
                "上下文 · 技能",
                "勾完按「完成」· 多选",
                Some(art::Slot::Context),
            ),
            Sheet::Prompt => (
                "上下文 · 提示词模板",
                "单值：这条每轮都带 · 再点一次取消",
                Some(art::Slot::Context),
            ),
            Sheet::Ext => (
                "上下文 · 扩展",
                "勾的是这次会话额外带哪些 · 核心四条引擎总带上，勾不掉",
                Some(art::Slot::Ext),
            ),
            Sheet::Dir => ("工作目录", "列的是引擎那台机器上的目录 · 点一层进去", None),
            Sheet::Machine => ("机器", "会话跑在哪台机器上 · 默认本机", None),
        };

        let kind = match sheet {
            Sheet::Session => art::PopKind::Session,
            Sheet::Model => art::PopKind::Model,
            Sheet::Skills => art::PopKind::Skills,
            Sheet::Dir => art::PopKind::Dir,
            Sheet::Machine => art::PopKind::Machine,
            Sheet::Prompt => art::PopKind::Narrow,
            Sheet::Ext => art::PopKind::Ext,
        };
        let pop = art::pop_rect(&m, slot, kind);
        let body = match sheet {
            Sheet::Session => self.sheet_session(&m, cx),
            Sheet::Model => self.sheet_model(&m, pop.w, cx),
            Sheet::Skills => self.sheet_skills(&m, cx),
            Sheet::Prompt => self.sheet_prompt(&m, cx),
            Sheet::Ext => self.sheet_ext(&m, cx),
            Sheet::Dir => self.sheet_dir(&m, cx),
            Sheet::Machine => self.sheet_machine(&m, cx),
        };

        let header = div()
            .flex_none()
            .flex()
            .items_start()
            .justify_between()
            .gap(m.v(1.))
            .px(m.v(1.6))
            .pt(m.v(1.3))
            .pb(m.v(0.95))
            .border_b(t().d(1.))
            .border_color(rgb(art::border()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(m.v(0.25))
                    .child(
                        div()
                            .text_size(m.f(1.3, 10., 12.5))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .italic()
                            .text_size(m.f(1.05, 8., 10.))
                            .text_color(rgb(art::muted()))
                            .child(hint),
                    ),
            )
            .child(
                div()
                    .id(ElementId::Name("pop-close".into()))
                    .flex_none()
                    .px(m.v(0.45))
                    .py(m.v(0.05))
                    .rounded(t().r(5.))
                    .text_size(m.f(1.1, 8.5, 10.5))
                    .text_color(rgb(art::muted()))
                    .hover(|s| s.bg(rgb(art::surface())).text_color(rgb(art::fg())))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.newchat.sheet = None;
                        cx.notify();
                    }))
                    .child("✕"),
            );

        // 目录弹层自带底栏（路径 + 机器 + 用这个目录），整条通用 footer 让给它。
        let footer = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(m.v(1.4))
            .py(m.v(0.75))
            .border_t(t().d(1.))
            .border_color(rgb(art::border()))
            .children((sheet != Sheet::Dir).then(|| {
                div()
                    .font_family(art::mono())
                    .text_size(m.f(0.8, 6.5, 8.5))
                    .text_color(rgb(art::faint()))
                    .child("Esc 关闭 · 点空白处也关")
                    .into_any_element()
            }))
            .children((sheet != Sheet::Dir).then(|| {
                div()
                    .id(ElementId::Name("board-sheet-done".into()))
                    .px(m.v(1.0))
                    .py(m.v(0.3))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(rgb(art::accent()))
                    .bg(rgb(art::accent()).opacity(0.14))
                    .text_size(m.f(1.05, 8.5, 11.))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.24)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.newchat.sheet = None;
                        cx.notify();
                    }))
                    .child("完成 ›")
                    .into_any_element()
            }))
            // 目录弹层的那条通用 footer 整个不画。
            .when(sheet == Sheet::Dir, |el| el.h(t().d(0.)).py(t().d(0.)));

        let cap = match pop.anchor {
            // Wide：往上顶到环上那排 pill，别把三个值压住。
            // 紧凑/竖排：pill 已经缩到圆下面了，弹层可以一直长到格子底，
            // 不然 640 高的窗口里模型列表只露五行。
            art::Anchor::Top(y) => {
                if m.layout == art::Layout::Wide {
                    art::foot_rect(&m, art::Foot::Name).y - 14. - y
                } else {
                    m.h - y - 24.
                }
            }
            // 从底往上弹：上面还有多少就给多少，不然面板会顶穿格子。
            art::Anchor::Bottom(y) => (m.h - y - 16.).max(200.),
        };
        let max_h = cap.clamp(220., m.h * 0.62);
        let panel = div()
            .id(ElementId::Name("board-pop".into()))
            .absolute()
            .left(px(pop.x))
            .w(px(pop.w))
            .flex()
            .flex_col()
            // 显式给上限：内容多就滚，别把面板顶出右格、也别压到 dock。
            .max_h(px(max_h))
            .bg(rgb(art::panel()))
            .border_1()
            .border_color(rgb(art::border()))
            .rounded(t().r(12.))
            .overflow_hidden();
        let panel = match pop.anchor {
            art::Anchor::Top(y) => panel.top(px(y)),
            art::Anchor::Bottom(y) => panel.bottom(px(y)),
        };

        // 遮罩一整块盖住右格，浮层本体用 `block_mouse_except_scroll` 把
        // 面板范围内的点击/滚轮留下 —— 见 坑.md 59（`occlude()` 会连滚轮一起挡）。
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .id(ElementId::Name("board-scrim".into()))
                .bg(rgba(0x05050647))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.newchat.sheet = None;
                    cx.notify();
                }))
                .child(
                    panel
                        .block_mouse_except_scroll()
                        .child(header)
                        .child(body)
                        .child(footer),
                )
                .into_any_element(),
        )
    }

    /// 模型弹层：三列铁路图。**这是这一页的灵魂** —— 供应商 › 模型 › 思考 是
    /// 一条有依赖的语法链，画成三个滚动的列表就看不出「先选谁再选谁」。
    fn sheet_model(&self, m: &art::M, sheet_w: f32, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let (providers, models, levels) = {
            let providers: Vec<String> = b
                .models
                .as_ref()
                .map(|x| {
                    let mut v: Vec<String> = Vec::new();
                    for it in &x.items {
                        if !v.contains(&it.provider) {
                            v.push(it.provider.clone());
                        }
                    }
                    v
                })
                .unwrap_or_default();
            let cur_provider = b
                .picked
                .as_ref()
                .map(|(p, _)| p.clone())
                .unwrap_or_default();
            let models: Vec<String> = b
                .models
                .as_ref()
                .map(|x| {
                    x.items
                        .iter()
                        .filter(|it| it.provider == cur_provider)
                        .map(|it| it.model_id.clone())
                        .collect()
                })
                .unwrap_or_default();
            (providers, models, b.levels())
        };

        let pitch = f32::from(m.v(2.6)).clamp(26., 34.);
        let rows = providers.len().max(models.len()).max(levels.len()).max(1);
        let total_h = pitch * rows as f32 + 16.;

        // 三列的横向位置：列宽按比例切，和页面一样（列窄了不换版面，只是更挤）。
        let (cols, xs) = sheet_lanes(sheet_w, f32::from(m.v(1.)));

        let trunk = rgb(art::border());
        let hot = rgb(art::accent());
        let mut layers: Vec<AnyElement> = Vec::new();

        // 每条链：一根竖轨 + 每行一个点 + 一根横支；选中的那条换成 accent。
        for (col, items) in [&providers, &models, &levels].into_iter().enumerate() {
            let x = xs[col] + 8.;
            let n = items.len();
            if n > 0 {
                layers.push(
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(8. + pitch / 2.))
                        .w(t().d(1.))
                        .h(px(pitch * (n.saturating_sub(1)) as f32))
                        .bg(trunk)
                        .into_any_element(),
                );
            }
            for (row, item) in items.iter().enumerate() {
                let y = 8. + row as f32 * pitch;
                let selected = match col {
                    0 => b.picked.as_ref().map(|(p, _)| p) == Some(item),
                    1 => b.picked.as_ref().map(|(_, mo)| mo) == Some(item),
                    _ => &b.level == item,
                };
                // 点和横支
                layers.push(
                    div()
                        .absolute()
                        .left(px(x - 2.))
                        .top(px(y + pitch / 2. - 2.))
                        .size(t().d(5.))
                        .rounded_full()
                        .bg(if selected { hot } else { trunk })
                        .into_any_element(),
                );
                layers.push(
                    div()
                        .absolute()
                        .left(px(x + 8.))
                        .top(px(y + pitch / 2.))
                        .w(t().d(6.))
                        .h(t().d(1.))
                        .bg(if selected { hot } else { trunk })
                        .into_any_element(),
                );

                let label = if col == 2 {
                    format!("{item} · {}", level_label(item))
                } else {
                    item.clone()
                };
                let action = match col {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                };
                layers.push(
                    div()
                        .id(ElementId::Name(format!("pill-{col}-{row}").into()))
                        .absolute()
                        .left(px(x + 16.))
                        .top(px(y))
                        .w(px(cols[col] - 30.))
                        .h(px(pitch - 6.))
                        .flex()
                        .items_center()
                        .px(t().d(8.))
                        .rounded(t().r(4.))
                        .border_1()
                        .border_color(if selected { hot } else { rgb(art::border()) })
                        .bg(if selected {
                            rgb(art::accent()).opacity(0.16)
                        } else {
                            rgb(art::surface())
                        })
                        .text_size(m.f(1.45, 10., 13.))
                        .font_family(art::mono())
                        .truncate()
                        .hover(|s| s.bg(rgb(art::surface())))
                        .on_click(cx.listener({
                            let label = label.clone();
                            move |this, _: &ClickEvent, _, cx| {
                                match action {
                                    0 => {
                                        // 换供应商：模型回到这家的第一个，档位重新夹。
                                        this.newchat.picked = Some((label.clone(), String::new()));
                                        this.board_pick_first_model();
                                    }
                                    1 => {
                                        if let Some((p, _)) = this.newchat.picked.clone() {
                                            this.newchat.picked = Some((p, label.clone()));
                                            this.newchat.clamp_level();
                                        }
                                    }
                                    _ => this.newchat.level = label.clone(),
                                }
                                this.cfg_push(cx);
                                cx.notify();
                            }
                        }))
                        .child(label)
                        .into_any_element(),
                );
            }
        }

        div()
            // `on_hover` 挂在 `Stateful` 上：先给 id。
            .id(ElementId::Name("board-model-viewport".into()))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.bar_hover(crate::app::Bar::Sheet, *hovered, cx)
            }))
            .child(
                div()
                    .id(ElementId::Name("board-model-body".into()))
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.newchat.sheet_scroll)
                    .child(div().relative().h(px(total_h)).w_full().children(layers)),
            )
            .child(crate::app::scrollbar::overlay_tone(
                "board-model-scrollbar",
                &self.newchat.sheet_scroll,
                self.bar(crate::app::Bar::Sheet).0,
                self.bar(crate::app::Bar::Sheet).1,
                // 盘面永远是深色 art 调色板：滑块得画成半透明白，不然一根黑线看不见。
                crate::app::scrollbar::Tone::OnDark,
            ))
            .into_any_element()
    }

    /// 换供应商时，把模型换成这一家的第一个（不然会停在别家的 modelId 上）。
    fn board_pick_first_model(&mut self) {
        let Some((provider, _)) = self.newchat.picked.clone() else {
            return;
        };
        let first = self
            .newchat
            .models
            .as_ref()
            .and_then(|m| m.items.iter().find(|x| x.provider == provider))
            .map(|x| x.model_id.clone());
        if let Some(id) = first {
            self.newchat.picked = Some((provider, id));
        }
        self.newchat.clamp_level();
    }

    /// 技能弹层：平铺多选。**故意不用铁路图** —— 技能是集合不是链。
    /// 技能弹层：**左类别树 + 右列表**，顶上一个通栏搜索框。
    ///
    /// 搜索和树是两种互斥状态 —— 一打字树就让位（淡出且点不动），右边换成
    /// **全盘命中**。这样不用解释「现在是在全盘搜还是在类里搜」。
    fn sheet_skills(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let q = b.skill_query.read(cx).value().trim().to_lowercase();
        let searching = !q.is_empty();
        let cat = b.skill_cat.clone();
        let all = b.skill_list();
        let total = all.len();
        let uncat = all.iter().filter(|s| s.category.is_empty()).count();

        // 名字 / 描述 / 类别，三处任意命中就算。**描述必须搜** ——
        // 技能名是英文（incident-postmortem），人记得住的是「复盘」。
        let hit = |s: &Skill| {
            !searching
                || s.name.to_lowercase().contains(&q)
                || s
                    .description
                    .clone()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&q)
                || s.category.iter().any(|c| c.to_lowercase().contains(&q))
        };

        let rows_src: Vec<&Skill> = all
            .into_iter()
            .filter(|s| {
                if searching {
                    hit(s)
                } else {
                    match cat.as_deref() {
                        None => true,
                        Some(NewChat::UNCAT) => s.category.is_empty(),
                        Some(c) => in_cat(&s.category, c),
                    }
                }
            })
            .collect();

        let mut rows: Vec<AnyElement> = Vec::new();
        for s in rows_src {
            let name = s.name.clone();
            let picked = b.picked_skills.contains(&name);
            let desc = s
                .description
                .clone()
                .unwrap_or_default()
                .replace(['\n', '\r'], " ");
            // 同时挂父类和子类时（`[写作, 写作/长文]`），只显示最具体的那条。
            let tag = s
                .category
                .last()
                .cloned()
                .unwrap_or_else(|| NewChat::UNCAT.to_string());
            rows.push(
                div()
                    .id(ElementId::Name(format!("skill-{name}").into()))
                    .flex()
                    .items_start()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.55))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if picked {
                        rgb(art::accent())
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(picked, |el| el.bg(rgb(art::accent()).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.07)))
                    .on_click({
                        let name = name.clone();
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.newchat.picked_skills.remove(&name) {
                                this.newchat.picked_skills.insert(name.clone());
                            }
                            cx.notify();
                        })
                    })
                    .child(
                        div()
                            .mt(t().d(2.))
                            .size(m.f(1.1, 10., 14.))
                            .rounded(t().r(3.))
                            .border_1()
                            .border_color(if picked {
                                rgb(art::accent())
                            } else {
                                rgb(art::border())
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(m.f(0.85, 8., 11.))
                            .text_color(rgb(art::accent()))
                            .child(if picked { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .font_family(art::mono())
                                    .text_size(m.f(1.5, 11., 13.5))
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.35, 10., 12.5))
                                    .text_color(rgb(art::muted()))
                                    .truncate()
                                    .child(desc),
                            ),
                    )
                    // 类别挂在行尾：左树说「你在哪一类里看」，这里说「这条属于哪一类」。
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(1.1, 8., 10.))
                            .text_color(rgb(art::faint()))
                            .child(tag),
                    )
                    .into_any_element(),
            );
        }

        // ── 左树 ──────────────────────────────────────────────
        let mut tree: Vec<AnyElement> = Vec::new();
        let node = |label: &str, count: usize, depth: usize, on: bool, live: bool, id: &str| {
            div()
                .id(ElementId::Name(format!("skill-cat-{id}").into()))
                .flex()
                .items_center()
                .gap(m.v(0.45))
                .pl(m.s(6. + depth as f32 * 13.))
                .pr(m.s(6.))
                .py(m.s(3.))
                .rounded(t().r(5.))
                .font_family(art::mono())
                .text_size(m.f(1.15, 8.5, 10.5))
                .text_color(rgb(if on {
                    art::fg()
                } else if live {
                    art::muted()
                } else {
                    art::faint()
                }))
                .when(on, |el| el.bg(rgb(art::surface())).border_1())
                .when(on, |el| el.border_color(rgb(art::border())))
                .when(!on && live, |el| {
                    el.hover(|s| s.bg(rgb(art::surface())).text_color(rgb(art::fg())))
                })
                .child(label.to_string())
                .child(
                    div()
                        .ml_auto()
                        .text_size(m.f(1.0, 8., 9.5))
                        .text_color(rgb(art::faint()))
                        .child(count.to_string()),
                )
        };
        // 「全部」—— 搜索时它不参与高亮（右边已经跨界了）。
        let mut el = node("全部", total, 0, !searching && cat.is_none(), true, "all");
        if !searching {
            el = el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.newchat.skill_cat = None;
                cx.notify();
            }));
        }
        tree.push(el.into_any_element());

        for (path, label, depth, count) in b.skill_cats() {
            let on = !searching && cat.as_deref() == Some(path.as_str());
            // 空类还是能点（点了会说「这一类还没有技能」），但颜色淡一档。
            let live = count > 0;
            let mut el = node(&label, count, depth + 1, on, live, &path);
            if !searching {
                let next = Some(path.clone());
                el = el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.skill_cat = next.clone();
                    cx.notify();
                }));
            }
            tree.push(el.into_any_element());
        }
        if uncat > 0 {
            let on = !searching && cat.as_deref() == Some(NewChat::UNCAT);
            let mut el = node(NewChat::UNCAT, uncat, 1, on, true, "uncat");
            if !searching {
                el = el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.newchat.skill_cat = Some(NewChat::UNCAT.into());
                    cx.notify();
                }));
            }
            tree.push(el.into_any_element());
        }

        let empty = if searching {
            "没有名字、描述或类别含这个词的技能"
        } else if total == 0 {
            "这个盘上还没装技能（技能在 68kazk 那块）"
        } else {
            "这一类还没有技能"
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            // 通栏搜索框：打字就全盘搜，树让位。
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(m.v(0.8))
                    .mx(m.v(1.3))
                    .mt(m.v(1.1))
                    .mb(m.v(0.7))
                    .px(m.v(0.9))
                    .py(m.v(0.45))
                    .rounded(t().r(7.))
                    .bg(rgb(art::surface()))
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(1.3, 10., 12.))
                            .text_color(rgb(art::accent()))
                            .child("›"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(Textarea::new(&b.skill_query).appearance(false).bordered(false)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(t().d(0.))
                    .border_t(t().d(1.))
                    .border_color(rgb(art::border()))
                    // 左树：搜索时淡出且点不动 —— 它是「常驻状态」，不是「临时状态」。
                    // 竖排档（格宽 < 620）放不下两栏，树整个不画：类别还在每行尾部。
                    .children((m.layout != art::Layout::Stack).then(|| {
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(m.s(172.))
                            .gap(t().d(1.))
                            .py(m.s(8.))
                            .border_r(t().d(1.))
                            .border_color(rgb(art::border()))
                            .when(searching, |el| el.opacity(0.35))
                            .children(tree)
                            .into_any_element()
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .min_h(t().d(0.))
                            .child(self.sheet_list(m, rows, empty, cx)),
                    ),
            )
            .into_any_element()
    }

    /// 扩展弹层：**左树（状态在上、类别在下）+ 右列表**，顶上通栏搜索。
    ///
    /// 跟技能弹层一个语法，只多一组「状态」：`内置 / 变体 / 已归档` 不是 `category`，
    /// 是这条扩展现在的处境，所以单独摆在分类上面、用一条线隔开。
    ///
    /// 注意勾的是什么：**这次会话带哪些扩展**（`POST /v1/sessions` 的 `extensions`），
    /// 不是本机装不装 —— 那是 `install` / `uninstall` 的事，属于设置页。
    fn sheet_ext(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let Some(x) = b.exts.as_ref() else {
            return div()
                .p(m.v(2.))
                .text_size(m.f(1.4, 10., 12.5))
                .text_color(rgb(art::muted()))
                .child(if b.loading {
                    "正在读扩展清单…"
                } else {
                    "没读到扩展清单"
                })
                .into_any_element();
        };
        let q = b.ext_query.read(cx).value().trim().to_lowercase();
        let searching = !q.is_empty();
        let cat = b.ext_cat.clone();

        let n_var: usize = x.variants.values().map(|v| v.len()).sum();
        let n_all = x.installed.len() + x.builtin.len() + n_var + x.archived.len();

        let mk = |name: String,
                  ver: String,
                  desc: String,
                  secs: String,
                  tag: String,
                  kind: ExtKind,
                  picked: bool|
         -> AnyElement {
            // 核心也锁：引擎不认「减」，让它们看着能勾就是在骗人。
            let locked = kind != ExtKind::Pickable;
            let mut el = div()
                .id(ElementId::Name(format!("ext-{name}").into()))
                .flex()
                .items_start()
                .gap(m.v(0.9))
                .w_full()
                .px(m.v(0.8))
                .py(m.v(0.55))
                .rounded(t().r(6.))
                .border_1()
                .border_color(if picked && !locked {
                    rgb(art::accent())
                } else {
                    rgba(t().p.transparent)
                })
                .when(picked && !locked, |el| {
                    el.bg(rgb(art::accent()).opacity(0.14))
                })
                .when(locked, |el| el.opacity(0.62))
                .when(!locked, |el| {
                    el.hover(|s| s.bg(rgb(art::accent()).opacity(0.07)))
                });
            if !locked {
                el = el.on_click({
                    let name = name.clone();
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.newchat.picked_exts.remove(&name) {
                            this.newchat.picked_exts.insert(name.clone());
                        }
                        cx.notify();
                    })
                });
            }
            el.child(
                div()
                    .mt(t().d(2.))
                    .size(m.f(1.1, 10., 14.))
                    .rounded(t().r(3.))
                    .border_1()
                    .border_color(if picked && !locked {
                        rgb(art::accent())
                    } else if locked {
                        rgb(art::line())
                    } else {
                        rgb(art::border())
                    })
                    // 锁住的画虚线：看得出「这里本来有个开关」。
                    .when(locked, |el| el.border_dashed())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(m.f(0.85, 8., 11.))
                    .text_color(rgb(art::accent()))
                    .child(if picked { "✓" } else { "" }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(t().d(0.))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(m.v(0.5))
                            .child(
                                div()
                                    .font_family(art::mono())
                                    .text_size(m.f(1.5, 11., 13.5))
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .font_family(art::mono())
                                    .text_size(m.f(1.05, 8., 9.5))
                                    .text_color(rgb(art::faint()))
                                    .child(ver),
                            ),
                    )
                    .children((!desc.is_empty()).then(|| {
                        div()
                            .text_size(m.f(1.35, 10., 12.5))
                            .text_color(rgb(art::muted()))
                            .truncate()
                            .child(desc)
                            .into_any_element()
                    }))
                    .children((!secs.is_empty()).then(|| {
                        div()
                            .font_family(art::mono())
                            .text_size(m.f(1.05, 8., 9.5))
                            .text_color(rgb(art::faint()))
                            .truncate()
                            .child(secs)
                            .into_any_element()
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .font_family(art::mono())
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(if kind == ExtKind::Broken {
                        art::accent()
                    } else {
                        art::faint()
                    }))
                    .child(tag),
            )
            .into_any_element()
        };

        let want = |name: &str, summary: Option<&str>, cats: &[String]| -> bool {
            if searching {
                return name.to_lowercase().contains(&q)
                    || summary.unwrap_or("").to_lowercase().contains(&q)
                    || cats.iter().any(|c| c.to_lowercase().contains(&q));
            }
            true
        };
        let show = |kind: ExtKind, cats: &[String]| -> bool {
            if searching {
                return true;
            }
            match cat.as_deref() {
                None => true,
                Some(k) => match k.strip_prefix('@') {
                    Some(t) => t == kind.tag(),
                    None => in_cat(cats, k),
                },
            }
        };

        let mut rows: Vec<AnyElement> = Vec::new();

        for e in x.catalog.iter().filter(|e| x.installed.contains(&e.name)) {
            let kind = if e.is_default {
                ExtKind::Core
            } else {
                ExtKind::Pickable
            };
            if !show(kind, &e.category) || !want(&e.name, e.summary.as_deref(), &e.category) {
                continue;
            }
            rows.push(mk(
                e.name.clone(),
                e.version.clone().unwrap_or_default(),
                e.summary.clone().unwrap_or_default().replace(['\n', '\r'], " "),
                e.sections.join(" · "),
                if e.is_default {
                    "固定 · 引擎总带上".to_string()
                } else {
                    String::new()
                },
                kind,
                b.picked_exts.contains(&e.name),
            ));
        }
        for name in x.builtin.iter() {
            if !show(ExtKind::Builtin, &[]) || !want(name, None, &[]) {
                continue;
            }
            rows.push(mk(
                name.clone(),
                String::new(),
                "引擎自带，不在仓库里 · 会话里一直在".into(),
                String::new(),
                "内置".into(),
                ExtKind::Builtin,
                true,
            ));
        }
        for (family, list) in x.variants.iter() {
            for v in list {
                let name = v.key.clone().unwrap_or_else(|| family.clone());
                let failed = v.load.as_ref().and_then(|l| l.state.as_deref()) == Some("failed");
                let err = v.load.as_ref().and_then(|l| l.error.clone());
                if !show(ExtKind::Broken, &[]) || !want(&name, err.as_deref(), &[]) {
                    continue;
                }
                rows.push(mk(
                    name,
                    String::new(),
                    err.or_else(|| v.dir.clone())
                        .unwrap_or_default()
                        .replace(['\n', '\r'], " "),
                    String::new(),
                    if failed { "加载失败" } else { "变体" }.into(),
                    if failed {
                        ExtKind::Broken
                    } else {
                        ExtKind::Archived
                    },
                    false,
                ));
            }
        }
        for a in x.archived.iter() {
            if !show(ExtKind::Archived, &a.category)
                || !want(&a.name, a.summary.as_deref(), &a.category)
            {
                continue;
            }
            rows.push(mk(
                a.name.clone(),
                a.version.clone().unwrap_or_default(),
                a.summary.clone().unwrap_or_default().replace(['\n', '\r'], " "),
                String::new(),
                a.archived_at
                    .clone()
                    .map(|d| d.chars().take(10).collect::<String>())
                    .unwrap_or_else(|| "已归档".into()),
                ExtKind::Archived,
                false,
            ));
        }

        let node = |label: &str, count: usize, depth: usize, on: bool, live: bool, id: &str| {
            div()
                .id(ElementId::Name(format!("ext-cat-{id}").into()))
                .flex()
                .items_center()
                .gap(m.v(0.45))
                .pl(m.s(6. + depth as f32 * 13.))
                .pr(m.s(6.))
                .py(m.s(3.))
                .rounded(t().r(5.))
                .font_family(art::mono())
                .text_size(m.f(1.15, 8.5, 10.5))
                .text_color(rgb(if on {
                    art::fg()
                } else if live {
                    art::muted()
                } else {
                    art::faint()
                }))
                .when(on, |el| {
                    el.bg(rgb(art::surface())).border_1()
                        .border_color(rgb(art::border()))
                })
                .when(!on && live, |el| {
                    el.hover(|s| s.bg(rgb(art::surface())).text_color(rgb(art::fg())))
                })
                .child(label.to_string())
                .child(
                    div()
                        .ml_auto()
                        .text_size(m.f(1.0, 8., 9.5))
                        .text_color(rgb(art::faint()))
                        .child(count.to_string()),
                )
        };

        let mut tree: Vec<AnyElement> = Vec::new();
        let mut el = node("全部", n_all, 0, !searching && cat.is_none(), true, "all");
        if !searching {
            el = el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.newchat.ext_cat = None;
                cx.notify();
            }));
        }
        tree.push(el.into_any_element());

        for (key, label, count) in [
            ("@installed", "启用中", x.installed.len()),
            ("@builtin", "内置", x.builtin.len()),
            ("@broken", "变体", n_var),
            ("@archived", "已归档", x.archived.len()),
        ] {
            let on = !searching && cat.as_deref() == Some(key);
            let mut el = node(label, count, 1, on, count > 0, &key[1..]);
            if !searching {
                let next = Some(key.to_string());
                el = el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.ext_cat = next.clone();
                    cx.notify();
                }));
            }
            tree.push(el.into_any_element());
        }

        // 分隔线 + 「按类别」：类别是仓库里写死的元数据，跟上面那组「处境」不是一回事。
        tree.push(
            div()
                .h(t().d(1.))
                .mx(m.s(8.))
                .my(m.s(6.))
                .bg(rgb(art::border()))
                .into_any_element(),
        );
        tree.push(
            div()
                .px(m.s(8.))
                .pb(m.s(2.))
                .font_family(art::mono())
                .text_size(m.f(0.95, 7.5, 8.5))
                .text_color(rgb(art::faint()))
                .child("按类别")
                .into_any_element(),
        );
        for path in x.categories.iter() {
            let label = path.rsplit('/').next().unwrap_or(path).to_string();
            let depth = path.matches('/').count();
            // 父类含子类：`工具` 要把 `工具/搜索` 里的也算上，
            // 否则会出现「工具 1，但它的子类搜索也 1」这种自相矛盾的树。
            let count = x.catalog.iter().filter(|e| in_cat(&e.category, path)).count();
            let on = !searching && cat.as_deref() == Some(path.as_str());
            let mut el = node(&label, count, depth + 1, on, count > 0, path);
            if !searching {
                let next = Some(path.clone());
                el = el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.ext_cat = next.clone();
                    cx.notify();
                }));
            }
            tree.push(el.into_any_element());
        }

        let empty = if searching {
            "没有名字、描述或类别含这个词的扩展"
        } else {
            "这一格没有扩展"
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(m.v(0.8))
                    .mx(m.v(1.3))
                    .mt(m.v(1.1))
                    .mb(m.v(0.7))
                    .px(m.v(0.9))
                    .py(m.v(0.45))
                    .rounded(t().r(7.))
                    .bg(rgb(art::surface()))
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(1.3, 10., 12.))
                            .text_color(rgb(art::accent()))
                            .child("›"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(Textarea::new(&b.ext_query).appearance(false).bordered(false)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(t().d(0.))
                    .border_t(t().d(1.))
                    .border_color(rgb(art::border()))
                    .children((m.layout != art::Layout::Stack).then(|| {
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(m.s(172.))
                            .gap(t().d(1.))
                            .py(m.s(8.))
                            .border_r(t().d(1.))
                            .border_color(rgb(art::border()))
                            .when(searching, |el| el.opacity(0.35))
                            .children(tree)
                            .into_any_element()
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .min_h(t().d(0.))
                            .child(self.sheet_list(m, rows, empty, cx)),
                    ),
            )
            .into_any_element()
    }

    fn sheet_prompt(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let rows: Vec<AnyElement> = b
            .prompt_list()
            .into_iter()
            .map(|p| {
                let name = p.name.clone();
                let picked = b.template.as_deref() == Some(name.as_str());
                let desc = p
                    .summary
                    .clone()
                    .or_else(|| p.description.clone())
                    .unwrap_or_default()
                    .replace(['\n', '\r'], " ");
                div()
                    .id(ElementId::Name(format!("prompt-{name}").into()))
                    .flex()
                    .items_start()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.55))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if picked {
                        rgb(art::accent())
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(picked, |el| el.bg(rgb(art::accent()).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.07)))
                    .on_click({
                        let name = name.clone();
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.newchat.template =
                                if this.newchat.template.as_deref() == Some(name.as_str()) {
                                    None
                                } else {
                                    Some(name.clone())
                                };
                            this.cfg_push(cx);
                            cx.notify();
                        })
                    })
                    // 圆框 = 一次只能选一个。形状本身就在说这事。
                    .child(
                        div()
                            .mt(t().d(2.))
                            .size(m.f(1.1, 10., 14.))
                            .rounded_full()
                            .border_1()
                            .border_color(if picked {
                                rgb(art::accent())
                            } else {
                                rgb(art::border())
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(m.f(0.5, 5., 7.)).rounded_full().bg(if picked {
                                rgb(art::accent())
                            } else {
                                rgba(t().p.transparent)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .font_family(art::mono())
                                    .text_size(m.f(1.5, 11., 13.5))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.35, 10., 12.5))
                                    .text_color(rgb(art::muted()))
                                    .truncate()
                                    .child(desc),
                            ),
                    )
                    .into_any_element()
            })
            .collect();

        self.sheet_list(
            m,
            rows,
            if b.prompt_list().is_empty() {
                "这个盘上还没有提示词模板（模板在 6ysg01 那块）"
            } else {
                ""
            },
            cx,
        )
    }

    /// 目录弹层：**命令面板**形态 —— 一个输入、一条可点的面包屑、
    /// 左边是这一层、右边告诉你这个目录是什么。
    ///
    /// 「这不是勾选，是导航」这条没变：列表里点一行就是进去，
    /// 选不选它由底下那个按钮决定。
    fn sheet_dir(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let mm = *m;
        let b = &self.newchat;
        let at = b.dir_at.clone().unwrap_or_default();
        let typed = b.dir_input.read(cx).value().trim().to_string();
        // `/` 或 `~` 开头是路径（shell 的老约定），别的都是「在这一层里筛」。
        let as_path = typed.starts_with('/') || typed.starts_with('~');
        let machine_label = b.machine.clone().unwrap_or_else(|| "本机".into());
        let parent = b.dirs.as_ref().and_then(|d| d.parent.clone());

        // ── 面包屑：每一段都能点 ──
        // 根单独画一个 `/`，后面每段前面加 `/` 当分隔 —— 根自己也进 segs
        // 的话会出现「/ / opt」。
        let mut segs: Vec<(String, String)> = Vec::new();
        let mut acc = String::new();
        for part in at.trim_start_matches('/').split('/').filter(|p| !p.is_empty()) {
            acc.push('/');
            acc.push_str(part);
            segs.push((part.to_string(), acc.clone()));
        }
        let crumbs = div()
            .flex()
            .items_center()
            .gap(t().d(1.))
            .px(m.v(1.5))
            .pb(m.v(0.9))
            .overflow_hidden()
            .child(
                div()
                    .id(ElementId::Name("crumb-root".into()))
                    .flex_none()
                    .px(m.v(0.45))
                    .py(m.v(0.2))
                    .rounded(t().r(4.))
                    .font_family(art::mono())
                    .text_size(m.f(1.15, 9., 11.))
                    .text_color(rgb(art::faint()))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.08)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.board_dir_enter("/".into(), cx);
                    }))
                    .child("/"),
            )
            .children(segs.into_iter().enumerate().flat_map(|(i, (label, path))| {
                let last = path == at || at.is_empty();
                let mut out: Vec<AnyElement> = Vec::new();
                if i > 0 {
                    out.push(
                        div()
                            .flex_none()
                            .text_size(m.f(0.95, 8., 10.))
                            .text_color(rgb(art::faint()))
                            .child("/")
                            .into_any_element(),
                    );
                }
                out.push(
                    div()
                        .id(ElementId::Name(format!("crumb-{i}").into()))
                        .flex_none()
                        .max_w(mm.s(120.))
                        .truncate()
                        .px(m.v(0.45))
                        .py(m.v(0.2))
                        .rounded(t().r(4.))
                        .font_family(art::mono())
                        .text_size(m.f(1.15, 9., 11.))
                        .text_color(if last { rgb(art::fg()) } else { rgb(art::muted()) })
                        .when(last, |el| el.font_weight(FontWeight::BOLD))
                        .hover(|s| s.bg(rgb(art::accent()).opacity(0.08)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.board_dir_enter(path.clone(), cx);
                        }))
                        .child(label)
                        .into_any_element(),
                );
                out
            }));

        // ── 列表 ──
        let mut rows: Vec<AnyElement> = Vec::new();
        if as_path && !typed.is_empty() {
            let target = typed.clone();
            rows.push(
                div()
                    .id(ElementId::Name("dir-jump".into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.8))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.6))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(rgb(art::accent()))
                    .bg(rgb(art::accent()).opacity(0.10))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.18)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.board_dir_enter(target.clone(), cx);
                    }))
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(1.2, 9.5, 11.))
                            .text_color(rgb(art::accent()))
                            .child("↵"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .font_family(art::mono())
                            .text_size(m.f(1.25, 10., 11.5))
                            .child(typed.clone()),
                    )
                    .into_any_element(),
            );
        }
        if let Some(up) = parent.filter(|_| !as_path) {
            let up_go = up.clone();
            rows.push(
                div()
                    .id(ElementId::Name("dir-up".into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.8))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.45))
                    .rounded(t().r(6.))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.07)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.board_dir_enter(up_go.clone(), cx);
                    }))
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(1.1, 9., 10.5))
                            .text_color(rgb(art::faint()))
                            .child("↑"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .font_family(art::mono())
                            .text_size(m.f(1.2, 9.5, 11.))
                            .text_color(rgb(art::muted()))
                            .child(up.clone()),
                    )
                    .into_any_element(),
            );
        }
        let needle = typed.to_lowercase();
        let mut hits = 0;
        // 打的是路径时，这一层的子目录就没有意义了 —— 只留那行「跳过去」。
        for d in b
            .dirs
            .as_ref()
            .filter(|_| !(as_path && !typed.is_empty()))
            .map(|x| x.dirs.clone())
            .unwrap_or_default()
        {
            if !needle.is_empty() && !as_path && !d.name.to_lowercase().contains(&needle) {
                continue;
            }
            hits += 1;
            let path = d.path.clone();
            let pick = path.clone();
            let selected = b.dir.as_deref() == Some(path.as_str());
            let sessions_here = self
                .sessions
                .iter()
                .filter(|x| x.cwd.as_deref() == Some(path.as_str()))
                .count();
            rows.push(
                div()
                    .id(ElementId::Name(format!("dir-{}", d.name).into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.8))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.45))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if selected {
                        rgb(art::accent())
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(selected, |el| el.bg(rgb(art::accent()).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.07)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.board_dir_enter(pick.clone(), cx);
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .font_family(art::mono())
                            .text_size(m.f(1.25, 10., 11.5))
                            .child(format!("{}/", d.name)),
                    )
                    .children((sessions_here > 0).then(|| {
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(0.9, 7.5, 9.))
                            .text_color(rgb(art::faint()))
                            .child(format!("{sessions_here} 会话"))
                    }))
                    .into_any_element(),
            );
        }

        // ── 右栏：这个目录是什么 ──
        let here = self
            .sessions
            .iter()
            .filter(|x| x.cwd.as_deref() == Some(at.as_str()))
            .count();
        let n = b.dirs.as_ref().map(|d| d.dirs.len()).unwrap_or(0);
        let kv = |k: &'static str, v: String| {
            div()
                .flex()
                .justify_between()
                .gap(m.v(0.6))
                .child(
                    div()
                        .flex_none()
                        .text_size(m.f(0.95, 8., 9.5))
                        .text_color(rgb(art::faint()))
                        .child(k),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(art::mono())
                        .text_size(m.f(1.05, 8.5, 10.))
                        .child(v),
                )
        };
        let preview = div()
            .flex_none()
            .w(mm.s(196.))
            .border_l(t().d(1.))
            .border_color(rgb(art::border()))
            .px(m.v(1.3))
            .py(m.v(1.2))
            .flex()
            .flex_col()
            .gap(m.v(0.55))
            .child(
                div()
                    .font_family(art::mono())
                    .text_size(m.f(0.9, 7.5, 9.))
                    .text_color(rgb(art::faint()))
                    .child("选中目录"),
            )
            .child(
                div()
                    .font_family(art::mono())
                    .text_size(m.f(1.05, 8.5, 10.))
                    .text_color(rgb(art::fg()))
                    .child(if at.is_empty() { "—".into() } else { at.clone() }),
            )
            .child(div().h(m.v(0.4)))
            .child(kv("机器", machine_label.clone()))
            .child(kv("这一层的会话", format!("{here}")))
            .child(kv("子目录", format!("{n}")))
            .child(div().flex_1());

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            // 命令行
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(m.v(0.8))
                    .mx(m.v(1.3))
                    .mt(m.v(1.1))
                    .mb(m.v(0.5))
                    .px(m.v(0.9))
                    .py(m.v(0.45))
                    .rounded(t().r(7.))
                    .bg(rgb(art::surface()))
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(1.3, 10., 12.))
                            .text_color(rgb(art::accent()))
                            .child("›"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(Textarea::new(&b.dir_input).appearance(false).bordered(false)),
                    ),
            )
            .child(crumbs)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(t().d(0.))
                    .border_t(t().d(1.))
                    .border_color(rgb(art::border()))
                    .child(
                        div()
                            .id(ElementId::Name("dir-list".into()))
                            .flex()
                            .flex_col()
                            .gap(t().d(2.))
                            .flex_1()
                            .min_w(t().d(0.))
                            .min_h(t().d(0.))
                            .px(m.v(0.9))
                            .py(m.v(0.8))
                            .overflow_y_scroll()
                            .track_scroll(&self.newchat.sheet_scroll)
                            .children(rows)
                            .children(if hits == 0 && !as_path {
                                vec![
                                    div()
                                        .py(m.v(1.6))
                                        .text_center()
                                        .text_size(m.f(1.2, 9.5, 11.))
                                        .text_color(rgb(art::muted()))
                                        .child(if typed.is_empty() {
                                            "这一层没有子目录".to_string()
                                        } else {
                                            format!("这一层没有名字含「{typed}」的目录")
                                        })
                                        .into_any_element(),
                                ]
                            } else {
                                Vec::new()
                            }),
                    )
                    .child(preview),
            )
            // 底栏
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(m.v(1.))
                    .px(m.v(1.4))
                    .py(m.v(0.7))
                    .border_t(t().d(1.))
                    .border_color(rgb(art::border()))
                    .child(
                        div()
                            .flex_none()
                            .font_family(art::mono())
                            .text_size(m.f(0.9, 7.5, 9.))
                            .text_color(rgb(art::faint()))
                            .child(format!("{} · {machine_label}", at.clone())),
                    )
                    .child(
                        div()
                            .id(ElementId::Name("dir-use".into()))
                            .flex_none()
                            .px(m.v(1.0))
                            .py(m.v(0.3))
                            .rounded(t().r(6.))
                            .bg(rgb(art::accent()))
                            .text_color(rgb(art::bg()))
                            .text_size(m.f(1.05, 8.5, 11.))
                            .hover(|s| s.opacity(0.88))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(at) = this.newchat.dir_at.clone() {
                                    this.newchat.dir = Some(at);
                                }
                                this.newchat.sheet = None;
                                cx.notify();
                            }))
                            .child("用这个目录"),
                    ),
            )
            .into_any_element()
    }

    /// 机器弹层：本机在最上面，`None` 就是它。
    fn sheet_machine(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut all: Vec<(Option<String>, String, String)> =
            vec![(None, "本机".into(), "qrd3wh 跑在哪儿就是哪儿".into())];
        for mc in b
            .machines
            .as_ref()
            .map(|x| x.machines.clone())
            .unwrap_or_default()
        {
            let note = mc
                .note
                .clone()
                .or_else(|| mc.addr.clone())
                .unwrap_or_default();
            all.push((Some(mc.name.clone()), mc.name.clone(), note));
        }
        for (value, label, note) in all {
            let picked = b.machine == value;
            rows.push(
                div()
                    .id(ElementId::Name(format!("machine-{}", label.clone()).into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.55))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if picked {
                        rgb(art::accent())
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(picked, |el| el.bg(rgb(art::accent()).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::accent()).opacity(0.07)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        // 目录是**机器上**的路径：换机器，旧路径立刻没意义，
                        // 留着它会让 Create 发一个对面根本不存在的 cwd。
                        if this.newchat.machine != value {
                            this.newchat.machine = value.clone();
                            this.board_dir_start(cx);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .mt(t().d(1.))
                            .size(m.f(1.1, 10., 14.))
                            .rounded_full()
                            .border_1()
                            .border_color(if picked {
                                rgb(art::accent())
                            } else {
                                rgb(art::border())
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(m.f(0.5, 5., 7.)).rounded_full().bg(if picked {
                                rgb(art::accent())
                            } else {
                                rgba(t().p.transparent)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .font_family(art::mono())
                                    .text_size(m.f(1.45, 10., 13.))
                                    .truncate()
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.3, 9.5, 12.))
                                    .text_color(rgb(art::muted()))
                                    .truncate()
                                    .child(note),
                            ),
                    )
                    .into_any_element(),
            );
        }
        self.sheet_list(m, rows, "", cx)
    }

    /// 弹层的正文：一个能滚的清单。空的时候把「为什么空」说出来，
    /// 别让人对着一个白框猜。
    fn sheet_list(
        &self,
        m: &art::M,
        rows: Vec<AnyElement>,
        empty: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if rows.is_empty() {
            return div()
                .flex_1()
                .min_h(t().d(120.))
                .flex()
                .items_center()
                .justify_center()
                .px(m.v(1.2))
                .py(m.v(2.4))
                .text_size(m.f(1.4, 10., 12.5))
                .text_color(rgb(art::muted()))
                .child(empty.to_string())
                .into_any_element();
        }
        div()
            // `on_hover` 挂在 `Stateful` 上：先给 id。
            .id(ElementId::Name("board-sheet-viewport".into()))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.bar_hover(crate::app::Bar::Sheet, *hovered, cx)
            }))
            .child(
                div()
                    .id(ElementId::Name("board-sheet-list".into()))
                    .flex()
                    .flex_col()
                    .gap(t().d(2.))
                    .px(m.v(1.2))
                    .py(m.v(1.))
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.newchat.sheet_scroll)
                    .children(rows),
            )
            .child(crate::app::scrollbar::overlay_tone(
                "board-sheet-scrollbar",
                &self.newchat.sheet_scroll,
                self.bar(crate::app::Bar::Sheet).0,
                self.bar(crate::app::Bar::Sheet).1,
                // 盘面永远是深色 art 调色板：滑块得画成半透明白，不然一根黑线看不见。
                crate::app::scrollbar::Tone::OnDark,
            ))
            .into_any_element()
    }
}
/// 弹层里三列（供应商 / 模型 / 思考档位）的列宽和起点。
///
/// 宽度按**弹层自己的宽**算：这里写死过 720，半屏时第三列就直接落在弹层外面。
fn sheet_lanes(sheet_w: f32, pad: f32) -> ([f32; 3], [f32; 3]) {
    let width = (sheet_w - 2. * pad).max(240.);
    let cols = [width * 0.30, width * 0.42, width * 0.28];
    let xs = [0., cols[0], cols[0] + cols[1]];
    (cols, xs)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{SessRow, sheet_lanes};

    /// 三列必须落在弹层宽度里。写死 720 那版在半屏窗口下第三列直接挂在弹层外。
    #[test]
    fn 弹层三列不超出弹层宽度() {
        for sheet_w in [720., 601., 500., 300.] {
            let (cols, xs) = sheet_lanes(sheet_w, 8.);
            let width = (sheet_w - 16.).max(240.);
            let right = xs[2] + cols[2];
            assert!(
                right <= width + 0.01,
                "{sheet_w} 宽的弹层里第三列右边缘 {right} 超出 {width}"
            );
            assert!(xs[1] >= cols[0] - 0.01, "{sheet_w}: 第二列盖住第一列");
        }
    }

    /// 四行走一圈回到原处：少一步走不到最后一排，多一步绕圈。
    #[test]
    fn 会话页四行走一圈回到原点() {
        let mut r = SessRow::Model;
        for _ in 0..SessRow::RING.len() {
            r = r.step(false);
        }
        assert_eq!(r, SessRow::Model);
    }

    /// 反向一步 == 正向三步（四个站的时候才成立，正好钉住「环里有几个」）。
    #[test]
    fn 会话页反向一步等于正向三步() {
        let mut a = SessRow::Model;
        let mut b = SessRow::Model;
        a = a.step(true);
        for _ in 0..SessRow::RING.len() - 1 {
            b = b.step(false);
        }
        assert_eq!(a, b);
    }

    /// 还没停过时第一次 Tab 要落在**第一行**（MODEL），不是最后一行。
    /// `sess_row_key` 里是「从 Perms 起步再正着走一步」，这条钉住那个出发点的选法。
    #[test]
    fn 会话页第一次_tab_落在第一行() {
        assert_eq!(SessRow::Perms.step(false), SessRow::Model);
    }

    /// 核心扩展 = builtin 的第一个（coding-tools）+ defaults 三条。
    /// 这条钉住「从哪取」：不能从 installed 推（coding-tools 不在里面），
    /// 也不能把 catalog 里 `default: true` 的普通扩展算进来（那是「默认勾」，
    /// 跟「引擎无条件带上」不是一回事）。
    // 用 JSON 造清单：`Extensions` 每个字段都有 `#[serde(default)]`，
    // 这正是它在生产里被构造的方式（引擎的 GET 回包）。手写 14 个字段，
    // 引擎哪天加一个字段测试就编不过 —— 那是 `deny_unknown_fields` 的毛病，
    // 不该在测试里重演。
    fn exts(json: &str) -> nous_engine::types::Extensions {
        serde_json::from_str(json).expect("测试清单")
    }

    #[test]
    fn 核心扩展从_builtin_和_defaults_取() {
        let x = exts(
            r#"{
                "builtin": ["coding-tools", "另一个内置"],
                "defaults": ["nous", "skills", "prompts"],
                "installed": ["nous", "skills", "prompts", "123"]
            }"#,
        );
        assert_eq!(
            super::core_exts(&x),
            vec!["coding-tools", "nous", "skills", "prompts"]
        );
    }

    /// 引擎连 defaults 都没给（老版本 / 读到半截）时至少别 panic。
    #[test]
    fn 核心扩展在空清单上不炸() {
        assert!(super::core_exts(&exts("{}")).is_empty());
        // builtin 有但 defaults 空：只认 builtin 第一条。
        let x = exts(r#"{"builtin": ["coding-tools"]}"#);
        assert_eq!(super::core_exts(&x), vec!["coding-tools"]);
    }
}
