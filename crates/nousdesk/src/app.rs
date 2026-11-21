//! 应用主体。三栏：图标条 / 会话列表 / 聊天区。
//!
//! 微信的骨头 + Tesla 的设计语言（见 `AGENTS.md` 那节「设计参考」）。
//! 流式在 `live.rs`：SSE 合帧到 16ms 再 `cx.notify()`，这里只管把帧画出来。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin { crate::theme::cur() }

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, Size};
// 完整的 lucide 目录在 assets 里（component 的 IconName 只是一小撮兼容子集）。
use gpui_kit::assets::IconName;
// 可选中文本。`div().child(String)` 是死的 StyledText —— 鼠标拖不动，也就复制不出来。
// 用 gpui-base 那个原始 TextView，不是 gpui-component 的兼容包装：
// 包装层根本没开放 `code_block_highlighter`（它把高亮挂在自己的主题机制上，
// 而那个机制只在 tree-sitter 那条分支里才有），拿不到就没法给代码块上色。
use gpui_kit::base::text::TextView;
use gpui_kit::prelude::FluentBuilder as _;
// `on_hover` / `on_click` 这些不是 Div 自带的方法，是这条 trait 给的，得显式拿进来。
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::pins::PinClient;
use nous_engine::types::{ConfigPatch, Entry, Session, Unread};
use nous_engine::{EngineClient, EngineError};
use smol::Timer;

use crate::nav_ui::SessionNav;
use crate::recent::Recent;

pub(crate) mod scrollbar;

/// 窗口窄到这个宽度以下就不画「你问过的」目录列了 —— 让出来的宽度给聊天区。
///
/// 图标条 60 + 会话列表 253 + 目录列 240 + 聊天区：960 时聊天区还剩 400 出头，
/// 再窄就只剩一格，不如把目录列收起来（抽屉里照样能跳）。
pub(crate) const NAV_COLUMN_MIN_W: f32 = 960.;

/// 窗口窄到这个宽度以下，Template 页的**类别树**就从常驻一栏收进「类别」抽屉。
///
/// 图标条 56 + 类别树 248 + 模板列表 300 = 604，1280 时右栏还剩 676 够画图；
/// 960 只剩 356，640 只剩 36 —— 图和新表单就全废了。收掉类别树把这 248 让给右栏。
pub(crate) const TREE_FOLD_W: f32 = 1000.;

/// 窄窗时模板列表的宽度（宽窗用皮肤里的 `template_list_w`，默认 300）。
/// 盘面算右格宽度时也得用这一份，否则 `pane_width` 会把右格算窄 60px。
pub(crate) const TEMPLATE_LIST_W_NARROW: f32 = 240.;

// 编译期钉住两条：最小窗口宽度下目录列必须已经让位（否则「半屏」时聊天区只剩一格）；
// 最小宽度也不能小于「会话列表 + 聊天区」装得下的下限。
const _: () = assert!(NAV_COLUMN_MIN_W > crate::MIN_WINDOW_W);
const _: () = assert!(crate::MIN_WINDOW_W >= 600.);

/// 哪块面板的滚动条。加新面板就往这儿加一个分支（`ALL` 里也补一行）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Bar {
    Chat,
    List,
    Tree,
    Presets,
    /// 设置页右栏（四页共用一份滚动位置，和设置页自己的切页状态一致）。
    Settings,
    /// 归档库整页。
    Archive,
    /// 数据监控室：左列拉片 / 中列那一屏 / 右列时间线，各一份。
    DataFilm,
    DataScreen,
    DataTl,
    /// 输入框左下角那颗 DNA 打开的面板（七页正文共用）。
    Cfg,
    /// 目录列（「前几轮 / 抽屉」那条左栏）。
    Nav,
    /// 历史抽屉正文。
    Drawer,
    /// New Manual 的两张清单（模型清单 / 供应商 sheet）。
    Sheet,
    /// 分支视图的左栏。
    Branch,
    /// 输入区（写字久了要自己滚，不能把聊天区顶到零）。
    Composer,
}

impl Bar {
    /// 一个不落、顺序无所谓 —— 只用来定数组长度和初始化。
    pub(crate) const ALL: [Bar; 15] = [
        Bar::Chat,
        Bar::List,
        Bar::Tree,
        Bar::Presets,
        Bar::Settings,
        Bar::Archive,
        Bar::DataFilm,
        Bar::DataScreen,
        Bar::DataTl,
        Bar::Cfg,
        Bar::Nav,
        Bar::Drawer,
        Bar::Sheet,
        Bar::Branch,
        Bar::Composer,
    ];

    pub(crate) const COUNT: usize = Bar::ALL.len();

    pub(crate) fn idx(self) -> usize {
        self as usize
    }
}

use crate::config::Config;
use crate::live::Live;
use crate::md::{chat_md, code_copy_button, code_highlighter, md_style, strip_harness};

// 微信桌面版的几个固定色。
//
// ⚠️ 这几个是「默认主题」的取值：`theme.rs` 的 `Theme::DEFAULT` 抄了同样的数字，
// 并有测试钉住两边一致。只在这里改色 = 主题卡片的色块和界面不符。
#[allow(dead_code)]
pub(crate) const SIDEBAR_DARK: u32 = 0x2e2e2e;
#[allow(dead_code)]
pub(crate) const LIST_BG: u32 = 0xf7f7f7;
#[allow(dead_code)]
pub(crate) const CHAT_BG: u32 = 0xededed;
#[allow(dead_code)]
pub(crate) const BUBBLE_ME: u32 = 0x95ec69;
#[allow(dead_code)]
pub(crate) const BUBBLE_OTHER: u32 = 0xffffff;
pub(crate) const MUTED: u32 = 0x999999;
/// 微信的绿：左栏选中图标就是这个颜色。
pub(crate) const WECHAT_GREEN: u32 = 0x07c160;

// 三栏的固定宽度。**右格宽度靠它们算出来**（`board_art::pane_width`），
// 所以改这里就必须同步那份表 —— 不然 New Manual 盘面的 cqmin 会算错。
//
// ⚠️ 这四个是「默认主题」的栏宽：`theme.rs` 的 `Skin::DEFAULT` 抄了同样
// 的数字并有测试钉住。渲染层已经改成读 `t().rail_w` 等 token，这些常量
// 只留作测试基准，别再在别处引用。
#[allow(dead_code)]
pub(crate) const ICON_BAR_W: f32 = 60.;
#[allow(dead_code)]
pub(crate) const SESSION_LIST_W: f32 = 280.;
#[allow(dead_code)]
pub(crate) const PRESET_TREE_W: f32 = 248.;
#[allow(dead_code)]
pub(crate) const TEMPLATE_LIST_W: f32 = 300.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Chats,
    Templates,
    Triggers,
    /// 数据监控室：六个数据屏 + 归档库搬进桌面端（`data_console.rs`）。
    Data,
    /// 资源：技能 / 提示词 / 扩展（方案 03：内容 / 扩展两段）。
    Resources,
}

/// 轻提示的性质：成功绿 / 失败红。
///
/// 用枚举而不是 `bool`：两个调用点都写 `true` / `false` 的时候，谁也看不出来
/// 那个 flag 是什么意思（评审 J1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoticeKind {
    /// 做成了 / 只是告知（绿）。
    Info,
    /// 失败了（红）。
    Error,
}

/// 全局轻提示的状态：文案 + 性质 + 一个代号。
///
/// 自动收起是「每弹一条挂一个 4 秒定时器」，难点在**连弹两条**：第一条的定时器
/// 不许在第二条头上把提示收掉。所以每条提示带一个代号，定时器只收自己等的那一条。
#[derive(Debug, Default)]
pub(crate) struct NoticeState {
    shown: Option<(String, NoticeKind)>,
    token: u64,
}

impl NoticeState {
    /// 弹一条，返回这次定时器该等的代号。旧定时器回来时代号对不上，什么都不做。
    pub(crate) fn show(&mut self, text: impl Into<String>, kind: NoticeKind) -> u64 {
        self.shown = Some((text.into(), kind));
        self.token += 1;
        self.token
    }

    /// 定时器到点：收掉自己等的那一条，返回收没收。
    pub(crate) fn expire(&mut self, token: u64) -> bool {
        if token != self.token || self.shown.is_none() {
            return false;
        }
        self.shown = None;
        true
    }

    /// 眼下在场的那条（文案 + 性质）。
    pub(crate) fn current(&self) -> Option<&(String, NoticeKind)> {
        self.shown.as_ref()
    }
}

/// 一次引擎调用的失败：引擎回了 `{error}`，还是任务本身挂了（tokio 那边）。
///
/// 两类的文案来源不同 —— 前者是引擎原文（一个字都不许改），后者是我们自己的
/// 「任务挂了」—— 所以由调用方决定画到哪（轻提示 / 归档库页头）。
enum EngineFailure {
    Engine(EngineError),
    Task(String),
}

/// 左栏这一帧画出来的「键盘可达顺序」：会话 id 和它在滚动容器里的孩子序号。
///
/// 渲染时写、按键时读。为什么不在按键时另算一遍：搜索模式下中间还插了分组标题
/// （「置顶 · 2」「更早 · 147」），行号≠孩子号，两份逻辑各自维护迟早对不上；
/// 而每帧重算一份也就 149 个 `i64`。
#[derive(Default, Debug)]
struct ListIndex {
    ids: Vec<i64>,
    /// 跟 `ids` 一一对应：这一行是滚动容器的第几个孩子（分组标题也占位置）。
    slots: Vec<usize>,
}

impl ListIndex {
    fn push(&mut self, id: i64, slot: usize) {
        self.ids.push(id);
        self.slots.push(slot);
    }

    fn pos(&self, id: i64) -> Option<usize> {
        self.ids.iter().position(|x| *x == id)
    }

    /// 从 `current` 往 `delta` 拇一格。到头就停在原地（不绕回），因为列表的
    /// 首尾有语义（最上面是「最近在聊」），绕回去等于把人吓一跳。
    /// `current` 不在名单里（选中的那条被搜索过滤掉了）时：向下 → 第一条，向上 → 最后一条。
    fn step(&self, current: Option<i64>, delta: i32) -> Option<(i64, usize)> {
        if self.ids.is_empty() {
            return None;
        }
        let last = self.ids.len() - 1;
        let next = match current.and_then(|id| self.pos(id)) {
            Some(p) if delta > 0 => (p + 1).min(last),
            Some(p) => p.saturating_sub(delta.unsigned_abs() as usize),
            None if delta > 0 => 0,
            None => last,
        };
        Some((self.ids[next], self.slots[next]))
    }
}

pub struct Desktop {
    pub(crate) engine: Option<Arc<EngineClient>>,
    pub(crate) sessions: Vec<Session>,
    pub(crate) selected: Option<i64>,
    entries: Vec<Entry>,
    entries_loading: bool,
    entries_error: Option<String>,
    pub(crate) error: Option<String>,
    /// **这一轮生成失败**（模型 400、task_failed、没答上来）：画在聊天区底部，
    /// 带「重试」。跟 `error`（连引擎失败、带「改哪个配置文件」提示）不是一条通道 ——
    /// 把「模型订阅过期」说成「连不上 + 去改配置」会把用户带到沟里（真踩过）。
    turn_error: Option<String>,
    pub(crate) loading: bool,
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
    /// Template 页列表的滚动句柄。
    pub(crate) presets_scroll: ScrollHandle,
    /// Template 页类别树的滚动句柄。
    pub(crate) tree_scroll: ScrollHandle,
    /// 设置页右栏的滚动句柄（四页共用一个位置，切页不重置）。
    pub(crate) settings_scroll: ScrollHandle,
    /// 分支视图左栏的滚动句柄。
    pub(crate) branch_scroll: ScrollHandle,
    /// 归档库整页的滚动句柄（切页/刷新不重置位置）。
    archive_scroll: ScrollHandle,
    /// 输入区的滚动句柄：字写多了在卡片里滚，不再把聊天区顶成零。
    composer_scroll: ScrollHandle,
    /// 每块滚动面板两位：鼠标在不在上面 / 滚动条在不在场（淡出那 250ms 里也得在场）。
    /// 按 `Bar` 索引 —— 加面板只加枚举项，不再加一堆平行字段。
    pub(crate) bars: [scrollbar::BarState; Bar::COUNT],
    /// 输入区悬停提示两位：鼠标在不在输入区上 / 提示字在不在场（淡出那 250ms 里也得在场）。
    composer_hover: bool,
    composer_hint: bool,
    /// 本机「最近打开过哪几条」：只喂排序，见 `recent.rs` 的模块注释。
    recent: Recent,
    /// 键盘是否停在会话列表上（↑↓ 换会话、↵ 回输入框、Esc 回搜索框）。
    /// 从搜索框按 ↓、或者鼠标点一行都会把它立起来。
    list_keys: bool,
    /// 这一帧画出来的行序（供键盘导航用）。`RefCell` 是因为渲染只有 `&self`。
    list_index: std::cell::RefCell<ListIndex>,
    /// 窗口几何上一次落盘的快照 + 时间。拖拽 / 缩放时每帧都在变，
    /// 不必每帧写盘 —— 变化且距上次写过 400ms 才写。关窗时 `on_app_quit` 再兑一次。
    ///
    /// `win_geom_at` 是 `Option`：`None` = 还没写过。初始化成 `Instant::now()` 会让
    /// **第一次** render（窗口还没动过）落在节流窗口里被丢掉 —— 而窗口静止之后就没有
    /// 下一帧了，于是永远不写（第一版就这么漏了）。
    win_geom: std::cell::Cell<Option<crate::window_state::WindowState>>,
    win_geom_at: std::cell::Cell<Option<std::time::Instant>>,
    /// 关闭前那次几何落盘。`Subscription` 掉了就取消，必须留字段。
    _quit_save: Option<Subscription>,
    /// 左栏的搜索框。**单行**的 `InputState`（输入区那个是多行 `TextareaState`）。
    /// 空着 = 不过滤，列表跟没有搜索这回事一样。
    search: Entity<InputState>,
    /// 输入区 `/` 技能命令的纯 UI 状态（光标注标 / Esc / 在途 / 写失败）。
    slash: crate::slash::Slash,
    /// `GET /v1/skills` 的结果。`None` = 还没拿到 / 拿失败；面板此时给一行提示。
    skills: Option<nous_engine::types::Skills>,
    /// 面板当前过滤出的技能快照。每次 `render` 现算，键盘 ↵ 和点击都认它。
    slash_items: Vec<nous_engine::types::Skill>,
    /// `GET /v1/skills` 在不在路上。一次只发一笔，别在 render 里反复点火。
    skills_loading: bool,
    /// 清单拉失败的一句话。有它就不重试（否则「失败 → notify → 再拉」是个死循环）。
    skills_error: Option<String>,
    /// 输入区左下角那颗 DNA 图标点开的「这条对话的设置」。
    /// 这里只放界面状态（看哪页 / 清单 / 在途）；配置值本身在 `sessions` 里，那才是真相。
    cfg: crate::config_panel::Panel,
    /// 左栏当前页：聊天 / Template 页。
    pub(crate) tab: Tab,
    /// 底部「≡ 更多」菜单开着没有。
    pub(crate) menu_open: bool,
    /// 「从模板起」那张模板清单开着没有（`＋` 菜单的第二项）。
    /// 它是左栏里的一块浮层，不是第二级菜单 —— 见 `preset_pick_overlay`。
    pub(crate) preset_pick: bool,
    /// 「复制这条会话」在途。防连点：`Ctrl+D` 按住会自动重复，不挡的话一秒能建十条。
    pub(crate) copying: bool,
    /// 右栏是不是在显示全局设置（整页替换聊天区，左栏不动）。
    pub(crate) settings_open: bool,
    /// 设置页左边选的那一项。
    pub(crate) settings_tab: crate::settings::SettingsTab,
    /// 关于页的二级 tab（概览 / 插件）。
    pub(crate) about_tab: crate::settings::AboutTab,
    /// 插件页当前选中的模块名（引擎模块名，不是显示名）。
    pub(crate) about_selected: Option<String>,
    /// 窄窗时插件列表 → 详情的输入状态：详情开着时不画列表。
    pub(crate) about_detail_open: bool,
    /// 当前主题。启动时从 `theme.toml` 读进 `theme::ActiveSkin` 这个 Global；
    /// 这里留一份只是给设置页判断「哪张卡片在选中态」用。
    /// **渲染层不读这个字段**，一律 `crate::theme::cur()` 取帧内快照。
    pub(crate) theme_name: crate::theme::ThemeName,
    /// 声音偏好（`sound.toml`）。**这里是唯一的内存副本**：每次改动先落盘再改它。
    /// 渲染层不直接读它做判断 —— 判断在 `sound::chime` 里，这条只是给设置页画开关。
    pub(crate) sound: crate::sound::Prefs,
    /// 设置页底部的一行反馈：文案 + 是不是错。
    pub(crate) settings_msg: Option<(String, bool)>,
    /// 归档库页开着没有（右栏整页）。优先级：设置 > 归档库 > New Manual > 聊天。
    pub(crate) archive_open: bool,
    /// 归档库页的状态（数据 / 载入中 / 错误原文 / 选中行）。
    pub(crate) archive: crate::archive_page::ArchivePageState,
    /// 数据监控室（左列第四个入口）的全部状态：五份数据 + 屏 + 拉片档 + 时间线。
    pub(crate) console: crate::data_console::ConsoleState,
    /// 版本不匹配横幅的「详情」展开了没有（方案 B：横幅一行 + 可展开的第二行）。
    pub(crate) contract_banner_open: bool,
    /// 「资源」页（一个图标、内容 / 扩展两段）的全部状态。
    pub(crate) resources: crate::resources::ResourceState,
    /// 「撕原件」危险确认的状态机（票 #3）。本票只负责把它接到引擎上。
    pub(crate) purge_confirm: crate::archive_confirm::PurgeConfirm,
    /// 「归档并撕原件」确认框的输入框。和 `rename_input` 同一个套路：
    /// `Desktop` 持有一份复用，打开时填当前会话名，不每开一次新建 `Entity`。
    pub(crate) purge_input: Entity<InputState>,
    /// 「重命名会话」弹层（居中对话框）。输入框单独存一个复用 —— 每开一次不新建
    /// `Entity`，焦点也好管。
    pub(crate) rename_box: crate::rename_box::RenameBox,
    pub(crate) rename_input: Entity<InputState>,
    /// Template 页：归档 / 彻底删的二确认（分步两屏）。
    pub(crate) preset_confirm: crate::preset_confirm::PresetConfirm,
    /// Template 页：改类别弹层。`Some` = 开着。
    pub(crate) cat_edit: Option<crate::templates::CatEditDraft>,
    /// Template 页的编辑器（C1 工作台）。`open == false` 时整块不画。
    pub(crate) editor: crate::editor::EditorState,
    /// 启动时（`NOUS_DESKTOP_OPEN=editor`）等列表回来就自动开编辑器，只为验界面。
    pub(crate) auto_editor: bool,
    /// Template 页：正在跑的生命周期动作（归档 / 还原 / 彻底删）的一句话。
    ///
    /// 引擎那边要 commit + push，实测**几分钟**量级；这期间列表上必须常驻一行
    /// 「正在归档 asd…」，否则用户以为点空了（AGENTS：点了没反馈算 bug）。
    pub(crate) preset_busy: Option<String>,
    /// 全局轻提示（最上层）。自动收起靠定时器 + 代号，见 [`NoticeState`]。
    pub(crate) notice: NoticeState,
    /// 启动时读到的配置（保存后跟着换）。
    ///
    /// 不叫 `cfg` —— 那个名字已经被 `config_panel` 的界面状态占了。
    pub(crate) conn: Config,
    pub(crate) host_input: Entity<InputState>,
    /// 配置里那一个引擎地址 —— 只用来显示，不存盘。
    pub(crate) engine_url: Option<String>,
    /// 引擎版本（`GET /v1/device` 的 `node`）。
    pub(crate) engine_node: Option<String>,
    /// 一次连接拿到的完整设备信息。About 面板的七个字段 + DEMO 角标都读它，
    /// 不再从别处拼第二份。
    pub(crate) device: Option<nous_engine::types::Device>,
    /// 能力表：功能面的唯一真相。连接时拉一次，失败为 `None`（全部 Unknown）。
    pub(crate) caps: Option<nous_engine::Capabilities>,
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
    /// 后台心跳上一次看到的未读表。`None` = 还没有基线（刚启动 / 刚换过焦点）。
    ///
    /// **它不是第二份未读真相**：界面永远画 `self.unread`（前台轮询落下来的那份）。
    /// 这一份只回答"比上次多了几个会话"，回答完就没用了。
    bg_seen: Option<Unread>,
    /// 左栏是不是「分支视图」。默认 false = 全部主线会话。
    pub(crate) branches: bool,
    /// 分支视图的根。进视图时锁住：点分支里的会话只换右侧，列表不跟着塌。
    pub(crate) branch_of: Option<i64>,
    /// 「下次进分支视图该锁哪个根」的一次性提示。
    ///
    /// `新建分支` 成功后选中了新会话，手动点「⑂ 分支」时 `self.selected` 是**新叉**，
    /// 拿它当根会看到「还没有分支」（新叉自己没有子）。所以 fork 成功时把**父会话**
    /// 记在这儿，进视图时优先用，用完清掉 —— 一次性，不影响别的手动进入。
    fork_root_hint: Option<i64>,
    /// 分支视图里展开着的会话。纯内存，重启重置（它不是事实，是「我眼下看到哪」）。
    pub(crate) expanded: std::collections::HashSet<i64>,
    /// 这一轮是**用户自己按的停止**。abort 会让流断开，那不是错误，别弹红条。
    user_stopped: bool,
    /// 内核（qrd3wh）的置顶客户端：和会话/历史同一个地址，走 `/v1/sessions/:id/pin`。
    /// `None` = 内核还没连上 —— 这时不显示置顶功能，绝不弹错。
    pins_client: Option<Arc<PinClient>>,
    /// 置顶的会话 id，**顺序就是显示顺序**（来自内核 `GET /v1/sessions`）。
    pins: Vec<i64>,
    /// 每条会话最后一次置顶提交的版本（回包 `version`）。只用来给**下一次切换**
    /// 生成稳定的幂等锚：同一目标态重试用同一个 requestId，切回时带上版本就不撞旧锚。
    pin_seqs: std::collections::HashMap<i64, u64>,
    /// Template 页的两个段（同一个私仓，两套词表、两套条目）。
    /// 索引 0 = 会话模板（`Session-Template`），1 = 终极模板（`Ultimate-Template`）。
    pub(crate) sections: [crate::templates::Section; 2],
    /// 当前选中的段。
    pub(crate) section: usize,
    /// 终极模板的组合开关清单。**开关状态只信引擎**（`/v1/combos`），
    /// 不读 `preset.json` 的 `enabled` 自己算 —— spec §4.3 的硬规矩。
    /// `None` = 还没拉过；`Some(vec![])` 是一条组合也没有（两段共用一份，段无关）。
    pub(crate) combos: Option<Vec<nous_engine::types::Combo>>,
    /// 正在翻开关的组合名（防连点：翻到一半再点会发出两条相反的 POST）。
    pub(crate) combo_flying: Option<String>,
    /// Template 页里选中的类别路径。`None` = 全部；`Some("∅")` = 未分类（不是词表里的类别）。
    pub(crate) preset_cat: Option<String>,
    /// 折叠着的类别路径。**纯内存**：它不是事实，是「我眼下展开到哪」。
    pub(crate) preset_closed: std::collections::HashSet<String>,
    /// 正在从模板开的那条（防连点 + 状态提示）。
    pub(crate) creating: Option<String>,
    /// 右栏关系图看的是哪条预设（37 节点图）。`None` = 什么也没选，右栏给空状态。
    pub(crate) picked_preset: Option<String>,
    /// 图上点了哪个节点（`dim:model` 这种 id）。只影响那一行说明。
    pub(crate) picked_node: Option<String>,
    /// 「新建一条」表单。`Some` = 右栏被表单占着，关系图让开。
    pub(crate) new_preset: Option<crate::templates::NewPresetDraft>,
    /// 「新建类别」弹层。
    pub(crate) new_cat: crate::templates::NewCatDraft,
    /// 模板列表的过滤词。**常驻**，不是点放大镜才出来 ——
    /// 找不到东西的人不会先去点那个图标。
    pub(crate) preset_filter: Entity<InputState>,
    /// 「复制成新的」弹层。`Some` = 开着。
    pub(crate) preset_copy: Option<crate::templates::CopyDraft>,
    /// 类别改名 / 删除弹层。`Some` = 开着。
    pub(crate) cat_dialog: Option<crate::templates::CatDialog>,
    /// 导入模板弹层（文件已读进来，只等名字）。`Some` = 开着。
    pub(crate) preset_import: Option<crate::templates::ImportDraft>,
    /// 模板重命名弹层。`Some` = 开着。
    pub(crate) preset_rename: Option<crate::templates::RenameDraft>,
    /// 此刻鼠标停在哪一条模板上（排序箭头只在悬停那行露出来）。
    pub(crate) preset_hover: Option<String>,
    /// 拖拽中悬停到哪一条上（落点线画在它上面）。
    pub(crate) preset_drop: Option<String>,
    /// 正在飞的那个排序请求（同一时刻只许一个）。
    pub(crate) preset_order_flying: Option<Vec<String>>,
    /// 飞着的时候又拖了一下 —— 排在这儿，回来再发。
    pub(crate) preset_order_next: Option<Vec<String>>,
    /// 窄窗下 Template 页的类别抽屉开着没有（宽窗常驻，不用这个）。
    pub(crate) cat_drawer: bool,
    /// Template 页在看**归档区**（列表栏换成归档列表）。
    pub(crate) archive_view: bool,
    _subs: Vec<Subscription>,
    /// 「New Manual」盘面。只占右侧聊天区那一格（外形照 qrd3wh 的 hb7k3j 页）。
    pub(crate) newchat: crate::board::NewChat,
    /// 会话内跳转：按天分组的目录 + 9 号方案的历史抽屉。纯内存的「我眼下看到哪」。
    pub(crate) nav: SessionNav,
    /// 触发器工作台（清单 / 详情 / 代码 / 向导）。数据来自 n8qkkj，不是 qrd3wh。
    pub(crate) triggers: crate::triggers::State,
}

impl Desktop {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 多行：3~8 行自动长高（最少 3 行：空框也占 3 行高，跟微信的写字区一样），
        // 超过就在框里滚。回车发送、Shift+回车换行。
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 8)
                // 回车=发送，不在这里插换行。少了这句，多行框里的回车会**先插一个换行**
                // 再发出去：文本被清空时看不出来，但 send() 提前返回时会留一个空行。
                .submit_on_enter(true)
            // 不设 placeholder：空框就该是空白。悬停的提示字由 composer 的 hint 层
            // 负责（Enter 发送 · Shift+Enter 换行），别在这里再叠一行永远挂着的灰字。
        });
        // 左栏的搜索框：单行，带搜索图标 + 一键清空。
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索会话"));
        let preset_filter = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜模板：名字 / 摘要 / 类别")
        });
        // 「重命名会话」那个对话框里的输入框。复用一份，打开时把当前名字塞进去。
        let rename_input = cx.new(|cx| InputState::new(window, cx).placeholder("会话名字"));
        // 资源页三段各自的搜索框（详见 resources.rs）。
        let resources = crate::resources::ResourceState::new(window, cx);
        let purge_input = cx.new(|cx| InputState::new(window, cx).placeholder("输入会话名字"));

        // 配置在这里同步读掉：设置页要立刻显示 host，不能等 tokio。
        // 读坏了也先拿默认值把窗口支起来，错误交给 connect() 报。
        let conn = Config::load().unwrap_or_default();
        let host_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://127.0.0.1:4790")
                .default_value(conn.host.clone())
        });
        // 调试用：启动就直接打开某个浮层，省得每次手点。
        // `NOUS_DESKTOP_OPEN=menu` / `=settings` / `=templates`，别的值（包括不设）就是正常启动。
        let open_at_start = std::env::var("NOUS_DESKTOP_OPEN").unwrap_or_default();
        // `/` 面板的 ↑↓ / ↵ / Esc 要在**输入框组件之前**拿到。
        // 用 interceptor（`observe_keystrokes` 太晚，`on_key_down` 挂在祖先节点上也会被
        // input 的 key binding 抢先消费）：在它里面 `cx.stop_propagation()` 就同时挡掉了
        // 组件的 Enter action（否则回车会顺手把 `/deep` 当消息发出去）。
        // 只有输入框聚焦 + 面板在场时才拦，别把全窗口的方向键都吃掉。
        let weak = cx.weak_entity();
        let subs = vec![
            cx.observe_window_activation(window, |this, window, _cx| {
                // 在后台就别每 3 秒拉一次。
                this.focused = window.is_window_active();
                // 焦点一变，后台心跳的基线就作废：
                // - 切走时作废 → 失焦前就已经跑完的那些不再补响一次；
                // - 回来时作废 → 你正看着的那堆未读不该再响。
                this.bg_seen = None;
            }),
            cx.subscribe_in(&input, window, {
                move |this, _, event: &InputEvent, window, cx| {
                    match event {
                        // 内容变了要重画，否则「发送」按钮永远是灰的。
                        // `/` 面板的高亮/收起也跟着重置（只认输入框当前内容这一份真相）。
                        InputEvent::Change => {
                            this.slash.reset();
                            cx.notify();
                        }
                        InputEvent::PressEnter { shift, .. } => {
                            if !*shift {
                                this.send(window, cx);
                            }
                        }
                        _ => {}
                    }
                }
            }),
            cx.subscribe_in(&search, window, {
                move |_, _, event: &InputEvent, _, cx| {
                    // 每敲一个字都要重排列表（纯本地过滤，不请求引擎）。
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                }
            }),
            cx.subscribe_in(&preset_filter, window, {
                move |_, _, event: &InputEvent, _, cx| {
                    // 同上：模板列表的过滤也是纯本地，敲一个字重排一次。
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                }
            }),
            // 资源页三段搜索框：纯本地过滤，敲一个字重排一次（不发请求）。
            cx.subscribe_in(&resources.search_boxes()[0], window, {
                move |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) { cx.notify(); }
                }
            }),
            cx.subscribe_in(&resources.search_boxes()[1], window, {
                move |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) { cx.notify(); }
                }
            }),
            cx.subscribe_in(&resources.search_boxes()[2], window, {
                move |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) { cx.notify(); }
                }
            }),
            cx.subscribe_in(&rename_input, window, {
                // 输入框里按回车 = 点「完成」。`InputEvent` 是**额外**的一条路：
                // Esc 走拦截器（那里要先于组件的 Esc action）。
                move |this, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { shift, .. } = event
                        && !*shift
                    {
                        this.confirm_rename(window, cx);
                    }
                }
            }),
            cx.intercept_keystrokes(move |ev, window, app| {
                let key = ev.keystroke.key.as_str();
                // 编辑器快捷键：Ctrl+S 存盘、Ctrl+K 命令面板。
                // 只在编辑器开着时接管 —— 别的地方 Ctrl+S 应该什么都不做。
                if ev.keystroke.modifiers.control
                    && !ev.keystroke.modifiers.alt
                    && !ev.keystroke.modifiers.platform
                {
                    let handled = weak
                        .update(app, |this, cx| {
                            if this.tab == Tab::Triggers
                                && this.trigger_ctrl(key, window, cx)
                            {
                                return true;
                            }
                            if !this.editor.open {
                                return false;
                            }
                            match key {
                                "s" => {
                                    this.save_editor(cx);
                                    true
                                }
                                "k" => {
                                    this.open_palette(window, cx);
                                    true
                                }
                                _ => false,
                            }
                        })
                        .unwrap_or(false);
                    if handled {
                        app.stop_propagation();
                        return;
                    }
                    // 左栏「新建」那三条：Ctrl+N 空白 / Ctrl+Shift+N 从模板起 /
                    // Ctrl+D 复制当前这条。键帽在菜单里画着，这里是真按。
                    // **只在编辑器没开时**管 —— 编辑器开着时 Ctrl+N 该是它自己的事
                    // （那条分支上面已经挡掉了，走到这儿说明编辑器没吃它）。
                    let shift = ev.keystroke.modifiers.shift;
                    let handled = weak
                        .update(app, |this, cx| {
                            this.new_entry_shortcut(key, shift, window, cx)
                        })
                        .unwrap_or(false);
                    if handled {
                        app.stop_propagation();
                    }
                    // 别的 Ctrl 组合照旧不归这里管。
                    return;
                }
                // New Manual 盘面：Tab 走站、⏎ 按下去、`← →` 在 Context 里分两半。
                // 走 interceptor 而不是挂在盘面根元素的 `on_key_down` 上 ——
                // 根元素的焦点会被输入框和窗口激活抢走，键盘流不能指望「焦点正好在根上」。
                if weak
                    .update(app, |this, cx| {
                        let ks = &ev.keystroke;
                        this.newchat.open && this.board_key(ks, cx)
                    })
                    .unwrap_or(false)
                {
                    app.stop_propagation();
                    return;
                }
                // 会话页「这个会话带什么」那四行：Tab 也要能走。它跟盘面共用一层 sheet，
                // 但盘面没开着 —— 所以是另一套停靠点（`SessRow`），不是 `board_key` 那条。
                // 插在这里是因为下面那条 `matches!` 只放行 up/down/enter/escape，
                // `tab` 会被提前挡掉。
                if weak
                    .update(app, |this, cx| this.sess_row_key(&ev.keystroke, cx))
                    .unwrap_or(false)
                {
                    app.stop_propagation();
                    return;
                }
                // 数据监控室：`T` 收起 / 展开右列时间线（键帽画在底栏）。
                // 这页没有输入框，所以不用像 `/` 面板那样挑「输入框聚焦时」才拦。
                if weak
                    .update(app, |this, cx| {
                        if this.tab != Tab::Data {
                            return false;
                        }
                        let ks = &ev.keystroke;
                        if key != "t" || ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
                            return false;
                        }
                        this.console.tl_open = !this.console.tl_open;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false)
                {
                    app.stop_propagation();
                    return;
                }
                if !matches!(key, "up" | "down" | "enter" | "escape") {
                    return;
                }
                // 修饰键组合（Ctrl+Enter 之类）不归面板管。
                if ev.keystroke.modifiers.control
                    || ev.keystroke.modifiers.alt
                    || ev.keystroke.modifiers.platform
                {
                    return;
                }
                // 触发器页：Esc 逐层退、↑↓ 选卡片、Enter 打开详情。
                if weak
                    .update(app, |this, _| this.tab == Tab::Triggers)
                    .unwrap_or(false)
                {
                    let handled = weak
                        .update(app, |this, cx| this.trigger_key(key, window, cx))
                        .unwrap_or(false);
                    if handled {
                        app.stop_propagation();
                        return;
                    }
                }
                // 配置面板开着时 Esc 收起。它没有 ↑↓ 这类键，只看 Esc，
                // 也不要求输入框聚焦 —— 面板自己的清单才是焦点可能待着的地方。
                // 命令面板盖在最上面：↑↓⏎ 和 Esc 都先给它。
                if weak
                    .update(app, |this, _cx| this.editor.palette.is_some())
                    .unwrap_or(false)
                {
                    let handled = weak
                        .update(app, |this, cx| {
                            match key {
                                "up" => this.palette_move(false, cx),
                                "down" => this.palette_move(true, cx),
                                "enter" => this.palette_run(window, cx),
                                "escape" => {
                                    this.editor.palette = None;
                                    cx.notify();
                                }
                                _ => return false,
                            }
                            true
                        })
                        .unwrap_or(false);
                    if handled {
                        app.stop_propagation();
                        return;
                    }
                }
                if key == "escape" {
                    // 会话页的「带什么」弹层：它跟盘面共用一层 sheet，但这时盘面没开着。
                    if weak
                        .update(app, |this, cx| {
                            // 规矩在 board 那边（`sheet_esc`）：子弹层退回摘要，
                            // 摘要在最上面才真关。这里只管「这个 Esc 归不归它」。
                            this.sheet_esc(cx)
                        })
                        .unwrap_or(false)
                    {
                        app.stop_propagation();
                        return;
                    }
                    // 盖在最上面的两层先拿 Esc：「撕原件」确认只关不撕；归档库页只收页。
                    // 都走 `stop_propagation`，不让 Esc 再落到下面去关别的面板。
                    let closed = weak
                        .update(app, |this, cx| {
                            // 编辑器里的小弹层比编辑器本身更靠上：先收弹层，再关编辑器。
                            if this.editor.palette.is_some() {
                                this.editor.palette = None;
                                cx.notify();
                                return true;
                            }
                            if this.editor.picker.is_some() {
                                this.editor.picker = None;
                                cx.notify();
                                return true;
                            }
                            if this.editor.text_edit.is_some() {
                                this.editor.text_edit = None;
                                cx.notify();
                                return true;
                            }
                            if this.editor.publish_msg.is_some() {
                                this.editor.publish_msg = None;
                                cx.notify();
                                return true;
                            }
                            // 编辑器是最上面一层：Esc 先关它（有改动时 close 自己会拦一次）。
                            if this.editor.open {
                                this.close_editor(false, cx);
                                return true;
                            }
                            if this.preset_confirm.is_open() {
                                this.preset_confirm.on_esc();
                                cx.notify();
                                return true;
                            }
                            if this.rename_box.is_open() {
                                this.rename_box.on_esc();
                                cx.notify();
                                return true;
                            }
                            if this.purge_confirm.is_open() {
                                this.purge_confirm.on_esc();
                                cx.notify();
                                return true;
                            }
                            if this.archive_open {
                                this.archive_open = false;
                                cx.notify();
                                return true;
                            }
                            false
                        })
                        .unwrap_or(false);
                    if closed {
                        app.stop_propagation();
                        return;
                    }
                    let closed = weak
                        .update(app, |this, cx| {
                            if !this.cfg_visible() {
                                return false;
                            }
                            this.cfg.open = false;
                            cx.notify();
                            true
                        })
                        .unwrap_or(false);
                    if closed {
                        app.stop_propagation();
                        return;
                    }
                    // 页面 / 列表层：`≡` 菜单 → 设置页 → 分支视图。
                    //
                    // 按「谁的浮层更大谁先」排：菜单是压在左栏底部的一块，设置页是换掉右栏、
                    // 分支视图是换掉左栏，所以菜单在最前。三者都有别的出口（点外 / 返回按钮 /
                    // 再点一次那个按钮），Esc 只是补齐第三条 —— 缺 Esc 的时候最难受：
                    // 明明满屏都是自己点开的东西，按 Esc 一点反应都没有。
                    let closed = weak
                        .update(app, |this, cx| {
                            // 「从模板起」那张卡：盖在左栏里、比 `≡` 菜单更靠上（它是从菜单
                            // 点出来的），所以先收它。
                            if this.preset_pick {
                                this.close_preset_pick(cx);
                                return true;
                            }
                            if this.menu_open {
                                this.close_menu(cx);
                                return true;
                            }
                            if this.settings_open {
                                this.close_settings(cx);
                                return true;
                            }
                            if this.branches {
                                this.exit_branches(cx);
                                return true;
                            }
                            false
                        })
                        .unwrap_or(false);
                    if closed {
                        app.stop_propagation();
                        return;
                    }
                    // Template 页：Esc 先收节点说明，再收整张图（两段，不是一步收光）。
                    let closed = weak
                        .update(app, |this, cx| {
                            if this.tab != Tab::Templates {
                                return false;
                            }
                            if this.cat_edit.is_some() {
                                this.cat_edit = None;
                                cx.notify();
                                return true;
                            }
                            if this.cat_drawer {
                                this.cat_drawer = false;
                                cx.notify();
                                return true;
                            }
                            if this.preset_copy.is_some() {
                                this.close_copy(cx);
                                return true;
                            }
                            if this.cat_dialog.is_some() {
                                this.close_cat_dialog(cx);
                                return true;
                            }
                            if this.preset_import.is_some() {
                                this.close_import(cx);
                                return true;
                            }
                            if this.preset_rename.is_some() {
                                this.close_preset_rename(cx);
                                return true;
                            }
                            if this.new_cat.input.is_some() {
                                this.close_new_category(cx);
                                return true;
                            }
                            // 搜索词比「选中」更强：Esc 先把搜索框清掉，别一键把整页都收了。
                            if !this.preset_filter.read(cx).value().is_empty() {
                                this.preset_filter.update(cx, |input, cx| {
                                    input.set_value("", window, cx)
                                });
                                cx.notify();
                                return true;
                            }
                            if this.new_preset.take().is_some() {
                                cx.notify();
                                return true;
                            }
                            if this.picked_node.take().is_some() {
                                cx.notify();
                                return true;
                            }
                            if this.picked_preset.take().is_some() {
                                cx.notify();
                                return true;
                            }
                            false
                        })
                        .unwrap_or(false);
                    if closed {
                        app.stop_propagation();
                        return;
                    }
                }
                let handled = weak
                    .update(app, |this, cx| {
                        if !this.slash_visible(cx) || this.slash.busy {
                            return false;
                        }
                        if !this.input.read(cx).focus_handle(cx).is_focused(window) {
                            return false;
                        }
                        match key {
                            "up" | "down" => {
                                let delta = if key == "up" { -1 } else { 1 };
                                let len = this.slash_items.len();
                                this.slash.move_by(delta, len);
                            }
                            "escape" => this.slash.dismissed = true,
                            "enter" => {
                                let ix = this.slash.cursor;
                                this.apply_skill(ix, window, cx);
                            }
                            _ => unreachable!(),
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                // 拦下的键不许再走到 input 组件：尤其 Enter，不然会把 `/deep`
                // 当成一条用户消息发出去。
                if handled {
                    app.stop_propagation();
                    return;
                }
                // 会话列表的键盘导航（↑↓ / ↵ / Esc）。排在 `/` 面板之后：
                // 正在敲命令时方向键归命令面板；抽屉开着时这一段自己让路（见 `list_nav_key`）。
                let handled = weak
                    .update(app, |this, cx| this.list_nav_key(key, window, cx))
                    .unwrap_or(false);
                if handled {
                    app.stop_propagation();
                    return;
                }
                // 全量历史抽屉：Esc 收起。它比 `/` 面板大一圈，但排在这条链的
                // **最后面** —— 抽屉里也能打字（`/`），正在敲的命令该先被 Esc 撤掉。
                // 抽屉本身没有焦点域（正文只是滚动容器），所以只能挂在窗口这一层。
                let closed = weak
                    .update(app, |this, cx| {
                        if !this.nav.drawer_open {
                            return false;
                        }
                        this.nav_close_drawer(cx);
                        true
                    })
                    .unwrap_or(false);
                if closed {
                    app.stop_propagation();
                }
            }),
        ];

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
            menu_open: open_at_start == "menu",
            preset_pick: false,
            copying: false,
            settings_open: open_at_start == "settings",
            // `NOUS_DESKTOP_OPEN=editor`：列表一拉回来就打开第一条的编辑器（验编辑器用）。
            auto_editor: open_at_start == "editor",
            settings_tab: crate::settings::SettingsTab::Connection,
            about_tab: crate::settings::AboutTab::Overview,
            about_selected: None,
            about_detail_open: false,
            theme_name: crate::theme::ThemeName::load(),
            sound: crate::sound::Prefs::load(),
            settings_msg: None,
            archive_open: false,
            archive: crate::archive_page::ArchivePageState::new(),
            console: crate::data_console::ConsoleState::new(),
            contract_banner_open: false,
            resources,
            purge_confirm: crate::archive_confirm::PurgeConfirm::new(),
            purge_input,
            rename_box: crate::rename_box::RenameBox::new(),
            rename_input, // 上面 `cx.new` 建好的那个
            preset_confirm: crate::preset_confirm::PresetConfirm::new(),
            cat_edit: None,
            preset_busy: None,
            editor: crate::editor::EditorState::new(),
            notice: NoticeState::default(),
            conn,
            host_input,
            engine_url: None,
            caps: None,
            engine_node: None,
            device: None,
            live: None,
            live_state: Live::new(),
            streaming: false,
            scroll: ScrollHandle::new(),
            list_scroll: ScrollHandle::new(),
            presets_scroll: ScrollHandle::new(),
            tree_scroll: ScrollHandle::new(),
            settings_scroll: ScrollHandle::new(),
            branch_scroll: ScrollHandle::new(),
            archive_scroll: ScrollHandle::new(),
            composer_scroll: ScrollHandle::new(),
            bars: [scrollbar::BarState::default(); Bar::COUNT],
            composer_hover: false,
            composer_hint: false,
            search,
            recent: Recent::load(),
            list_keys: false,
            list_index: std::cell::RefCell::new(ListIndex::default()),
            win_geom: std::cell::Cell::new(None),
            win_geom_at: std::cell::Cell::new(None),
            _quit_save: None,
            slash: crate::slash::Slash::default(),
            skills: None,
            slash_items: vec![],
            skills_loading: false,
            skills_error: None,
            cfg: crate::config_panel::Panel::default(),
            // `=templates` 直接落在 Template 页：验右键菜单 / 生命周期时省一次点击。
            tab: if open_at_start == "templates" {
                Tab::Templates
            } else if open_at_start == "triggers" {
                Tab::Triggers
            } else if open_at_start == "data" {
                Tab::Data
            } else if open_at_start == "resources" {
                Tab::Resources
            } else {
                Tab::Chats
            },
            focused: true,
            stream_for: None,
            open_tools: std::collections::HashSet::new(),
            live_tools: vec![],
            unread: Unread::default(),
            bg_seen: None,
            branches: false,
            branch_of: None,
            fork_root_hint: None,
            expanded: std::collections::HashSet::new(),
            user_stopped: false,
            pins_client: None,
            pins: vec![],
            pin_seqs: std::collections::HashMap::new(),
            sections: [
                crate::templates::Section::new(0),
                crate::templates::Section::new(1),
            ],
            triggers: crate::triggers::State::new(window, cx),
            section: 0,
            combos: None,
            combo_flying: None,
            preset_cat: None,
            preset_filter,
            preset_copy: None,
            cat_dialog: None,
            preset_import: None,
            preset_rename: None,
            preset_hover: None,
            preset_drop: None,
            preset_order_flying: None,
            preset_order_next: None,
            preset_closed: std::collections::HashSet::new(),
            creating: None,
            picked_preset: None,
            picked_node: None,
            new_preset: None,
            new_cat: crate::templates::NewCatDraft::default(),
            cat_drawer: false,
            archive_view: false,
            newchat: crate::board::NewChat::new(window, cx),
            nav: SessionNav::new(),
            _subs: subs,
        };
        this.bootstrap(cx);
        // 关窗前补一次几何落盘：`render` 里那 400ms 节流可能刚好错过
        // 「调完窗口立刻关掉」那一下。关窗后窗口已经没了 —— 取不到就认了。
        this._quit_save = Some(cx.on_app_quit(|_, cx| {
            let state = cx.windows().into_iter().find_map(|handle| {
                handle
                    .update(cx, |_, window, _| {
                        crate::window_state::WindowState::from_bounds(window.window_bounds())
                    })
                    .ok()
            });
            if let Some(state) = state
                && let Err(e) = state.save()
            {
                log::warn!("窗口几何没存下：{e}");
            }
            async {}
        }));
        this
    }

    /// 启动：起轮询 + 连一次。
    fn bootstrap(&mut self, cx: &mut Context<Self>) {
        // 轮询是个死循环，只能起一次 —— 所以它留在这儿，不在 connect 里。
        self.start_poll(cx);
        // 后台那条轻心跳同理：只起一次，跟着窗口活。
        self.start_attend(cx);
        self.connect(cx);
    }

    /// 连引擎。启动走一次；「重新连接」和设置页保存后也走这里。
    ///
    /// 只做「连上 + 重拉数据」，不碰轮询（见上）。
    pub(crate) fn connect(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        self.engine_url = None;
        self.device = None;
        self.caps = None;
        cx.notify();

        let cfg = self.conn.clone();
        let recent = self.recent.clone();
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let outcome = match handle {
                Some(h) => match h.spawn(load_engine(cfg, recent)).await {
                    Ok(Ok(v)) => Ok(v),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(format!("任务挂了：{e}")),
                },
                None => Err("tokio 没起".to_string()),
            };

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match outcome {
                    Ok((engine, sessions, device, url, caps)) => {
                        this.engine = Some(engine);
                        this.sessions = sessions;
                        this.engine_node = device.as_ref().and_then(|d| d.node.clone());
                        this.device = device;
                        this.engine_url = Some(url.clone());
                        this.caps = caps;
                        // 换了盘之后，原来选中的那条可能不在新引擎上 —— 别留个空壳。
                        if this
                            .selected
                            .is_some_and(|id| !this.sessions.iter().any(|s| s.id == id))
                        {
                            this.selected = None;
                            this.entries.clear();
                            this.entries_error = None;
                        }
                        // 引擎连上之前如果点过 Template 页，「还没连上引擎」那句得自己消掉。
                        this.load_all_sections(false, cx);
                        // 触发器入口由 capability 守卫；client 已落地，能力表说可用才拉。
                        this.load_triggers(cx);
                        // 直接开在数据监控室（`NOUS_DESKTOP_OPEN=data`）时，五份数据也得
                        // 自己拉一遍 —— `switch_tab` 只在「切过去」那一刻触发。
                        if this.tab == Tab::Data {
                            this.load_console(false, cx);
                        }
                        if this.tab == Tab::Resources {
                            this.load_resources(false, cx);
                        }
                        // 置顶来自内核，地址就是这一个引擎地址 ——
                        // 手机端停不停都跟置顶无关。
                        this.load_pins(cx, url.clone());
                        // `/` 面板的技能清单：失败就认了（面板给提示）。
                        this.load_skills(cx);
                        // 调试用：`NOUS_OPEN_BOARD=1` 一启动就开在 New Manual 盘面上，
                        // 省得每次手点。只是开关，不是功能。
                        if std::env::var("NOUS_OPEN_BOARD").is_ok() && !this.newchat.open {
                            log::info!("NOUS_OPEN_BOARD：直接开在 New Manual 盘面上");
                            this.newchat.open = true;
                            this.request_focus();
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

    /// 从内核拿置顶。**失败就静默降级**：`pins_client` 留 `None`，
    /// 界面上没有置顶功能，但不影响聊天。地址就是配置里那一个引擎地址 ——
    /// 不再单独 resolve 手机端那块，它停不停都跟置顶无关。
    fn load_pins(&mut self, cx: &mut Context<Self>, engine_url: String) {
        // token 跟聊天走同一条注入（`EngineClient` 那颗）：少了它，配了 token 的机器上
        // 置顶就是 401 → 这里静默降级 → 界面上「置顶等于没有」（#10）。
        let token = self.conn.resolve_token();
        let client = match PinClient::with_token(&engine_url, Duration::from_secs(8), token.as_deref())
        {
            Ok(c) => Arc::new(c),
            Err(e) => {
                log::warn!("置顶客户端建不起来：{e}");
                return;
            }
        };
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let ids = {
                let c = client.clone();
                handle.spawn(async move { c.pinned_ids().await }).await
            };
            let _ = this.update(cx, |this, cx| match ids {
                Ok(Ok(ids)) => {
                    log::info!("置顶读到了：{ids:?}");
                    this.pins_client = Some(client);
                    this.pins = ids;
                    this.pin_seqs.clear(); // 服务端重排过，旧版本不再当锚用
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

        // 幂等锚：**同一个会话、同一个目标态**就同一个 requestId —— 回包丢了再点一次
        // 也不会落第二条提交。时间戳不行：那会把重试变成新的一次决定。
        let version = self.pin_seqs.get(&id).copied().unwrap_or(0);
        let request_id = format!("desk-pin-{id}-{}-{version}", if want { "on" } else { "off" });

        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let res = handle
                .spawn(async move { client.set_pin(id, want, &request_id).await })
                .await;
            let failed = match res {
                Ok(Ok(receipt)) => {
                    if receipt.pinned == want {
                        log::info!("置顶写进去了：{id} -> {want}（version {}）", receipt.version);
                        let _ = this.update(cx, |this, _cx| {
                            this.pin_seqs.insert(id, receipt.version);
                        });
                        false
                    } else {
                        log::warn!("置顶回包不是要的状态：{id} -> {}", receipt.pinned);
                        true
                    }
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

    /// 后台那条**轻心跳**：你不在窗口前的时候，替你盯着"有会话跑完了"。
    ///
    /// ## 为什么非得有这么一条
    ///
    /// 上面那条 `start_poll` 的门槛是 `focused && !streaming` —— 窗口一失焦，
    /// 连未读都不拉了。于是"发出去、切去干别的、它跑完了"这件事，客户端**没有任何
    /// 数据源**，提示音也就无从响。这一条补的只是这个洞，别的什么都不管。
    ///
    /// ## 三条约束
    ///
    /// - **只拉 `/v1/unread`**（一个平铺 map，几百字节），不拉 `/v1/sessions`。
    /// - **只在失焦时跑**：前台那条轮询已经在拉了，两条一起跑就是白花 RTT。
    /// - **失败退避**：连不上就别每 5 秒捶一次，退到 8 秒；回前台立刻复位。
    ///
    /// ## 这是临时的
    ///
    /// `decisions.md` 2026-11-16 写了反悔点：引擎一有全局事件流
    /// （`/v1/events` 或 `sessions` 带 `lastActivityAt`），这条心跳**必须删掉**。
    /// 客户端不该为了响一声去轮询，更不该自己"猜"一轮完成了。
    fn start_attend(&mut self, cx: &mut Context<Self>) {
        const BEAT: Duration = Duration::from_secs(5);
        const BACKOFF: Duration = Duration::from_secs(8);

        cx.spawn(async move |this, cx| {
            let mut wait = BEAT;
            loop {
                Timer::after(wait).await;
                let go = this
                    .update(cx, |this, _| !this.focused && this.engine.is_some())
                    .unwrap_or(false);
                if !go {
                    // 回到前台（或还没连上）：前台那条轮询接手，这里歇着。
                    wait = BEAT;
                    continue;
                }
                let engine = match this.update(cx, |this, _| this.engine.clone()) {
                    Ok(Some(engine)) => engine,
                    _ => {
                        wait = BEAT;
                        continue;
                    }
                };
                let Some(h) = crate::TOKIO.get().cloned() else {
                    break;
                };
                match h.spawn(async move { engine.unread().await }).await {
                    Ok(Ok(unread)) => {
                        wait = BEAT;
                        let _ = this.update(cx, |this, _| this.attend(unread));
                    }
                    // 断了 / 超时：退避。不记每条 —— 人不在电脑前，日志刷屏没用。
                    _ => wait = BACKOFF,
                }
            }
        })
        .detach();
    }

    /// 一次后台心跳拿回来的未读表：只看"比上次多了几个会话"，然后决定响不响。
    ///
    /// 只管**完成**。出错不会让未读变多（一轮失败没有新消息），所以后台不响错误音 ——
    /// 那一声留给前台：你真在看的时候，红条自己会跳出来。
    fn attend(&mut self, next: Unread) {
        let prev = self.bg_seen.replace(next.clone());
        // 第一次睁眼只记基线。切走的那一刻本来就挂着一堆没读的，不该"补响"。
        let Some(prev) = prev else {
            return;
        };
        // 正在流的那条会话不算：它的"完成"由 SSE 那条路（`drained.done`）响，
        // 这里再算一次就是两声。
        let bumped = bumped_sessions(&prev, &next, self.stream_for);
        if bumped == 0 {
            return;
        }
        crate::sound::chime_background(self.sound, bumped);
    }

    /// 定时重读 `/v1/sessions`。聚焦才拉、流式中不拉。
    ///
    /// 内容没变就不 `cx.notify()` —— 每 3 秒重绘一次窗口是白烧 CPU。
    fn start_poll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_secs(3)).await;
                let go = this
                    .update(cx, |this, _| {
                        this.focused && !this.streaming && this.engine.is_some()
                    })
                    .unwrap_or(false);
                if !go {
                    continue;
                }
                let (engine, selected, recent) =
                    match this.update(cx, |this, _| {
                        (this.engine.clone(), this.selected, this.recent.clone())
                    }) {
                        Ok(v) => v,
                        Err(_) => break,
                    };
                let Some(engine) = engine else { continue };
                let Some(h) = crate::TOKIO.get().cloned() else {
                    break;
                };
                // 会话和未读一起拉：两个都是小 JSON，分两次反而多一次 RTT。
                let got = h
                    .spawn(async move {
                        let sessions = engine.sessions().await;
                        let unread = engine.unread().await;
                        (sessions, unread)
                    })
                    .await;
                let Ok((sessions, unread)) = got else {
                    continue;
                };
                let Ok(list) = sessions else {
                    // 断一下就等下一轮，不打断 UI。
                    continue;
                };

                let mut next = list.items;
                sort_sessions(&mut next, &recent);
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
                            this.sessions
                                .first()
                                .map(|s| s.name.as_str())
                                .unwrap_or("-"),
                            this.sessions.first().and_then(|s| sig_of(s).2),
                            this.sessions
                                .first()
                                .map(|s| s.busy.as_ref().map(|b| b.to_string()))
                                .unwrap_or(None),
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

    /// 人在左栏点了一条会话 = 明确离开 New Manual 盘面。
    /// **只有人点才走这里**：poll / SSE / 停止按钮那些内部刷新仍然直接调
    /// `select_session`，否则后台每 3 秒一次刷新会把人正在配的盘面悄悄关掉。
    pub(crate) fn choose_session(&mut self, id: i64, cx: &mut Context<Self>) {
        self.board_close(cx);
        self.select_session(id, false, cx);
    }

    /// 左栏的键盘导航：↑↓ 换会话、↵ 把焦点送回输入框、Esc 回搜索框。
    ///
    /// 返回 `true` = 这个键归列表管，调用方要 `stop_propagation`。
    ///
    /// 三条门（缺一个就会抢别人的键）：
    /// - **输入框在打字时让路**：多行输入里 ↑↓ 是移光标，回车是发送。
    /// - **只看会话这一页，且没有浮层**：设置页 / 模板页 / 归档库 / 编辑器 / ≡ 菜单 /
    ///   新建盘面 / 全量历史抽屉开着时，方向键都属于人家。
    /// - 要么键盘已经在列表上（`list_keys`），要么焦点在搜索框（↓ 才把它接过来）。
    fn list_nav_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.input.read(cx).focus_handle(cx).is_focused(window) {
            return false;
        }
        if self.tab != Tab::Chats
            || self.cfg_visible()
            || self.editor.open
            || self.archive_open
            || self.settings_open
            || self.menu_open
            || self.newchat.open
            || self.nav.drawer_open
        {
            return false;
        }
        let search_focus = self.search.read(cx).focus_handle(cx).is_focused(window);
        if !search_focus && !self.list_keys {
            return false;
        }
        match key {
            "down" | "up" => {
                let delta = if key == "down" { 1 } else { -1 };
                // 搜索框里按 ↑：不动。上面没东西，别把焦点往上拽。
                if !self.list_keys && delta < 0 {
                    return false;
                }
                // 从搜索框按 ↓ 进列表 = 走到第一条（不是「选中项的下一条」）：
                // 搜索框在上，往下走就是列表起点。
                let from = if self.list_keys { self.selected } else { None };
                let next = self.list_index.borrow().step(from, delta);
                self.list_keys = true;
                if let Some((id, slot)) = next {
                    let changed = self.selected != Some(id);
                    if changed {
                        self.select_session(id, false, cx);
                    }
                    // 一屏放不下 149 行：选中行要跟着键盘走（跟鼠标点击不同 ——
                    // 鼠标点的那行本来就在眼前）。滚最小量，居中不管。
                    self.list_scroll.scroll_to_item(slot);
                    if !changed {
                        cx.notify();
                    }
                }
                true
            }
            "enter" => {
                if !self.list_keys {
                    return false;
                }
                // 选中即打开（上面那条），回车只干一件事：把焦点交给输入框，
                // 接着就能打字。不给回车另加「打开」动作 —— 那会变成要按两次。
                self.list_keys = false;
                // 用组件自己的 `focus()`：它顺手把光标闪烁起起来。只 `window.focus()`
                // 焦点是过去了，但 caret 不闪 —— 看起来像没生效（这一版实锤踩过）。
                self.input
                    .update(cx, |state, cx| state.focus(window, cx));
                cx.notify();
                true
            }
            "escape" => {
                if !self.list_keys {
                    return false;
                }
                // 回搜索框：整条链路是「搜索 → ↓ 进列表 → ↑↓ 挑 → ↵ 打字 → Esc 回搜索」。
                // 不清空搜索词 —— 清空是搜索框自己那颗 ✕ 的事。
                self.list_keys = false;
                self.search
                    .update(cx, |state, cx| state.focus(window, cx));
                cx.notify();
                true
            }
            _ => false,
        }
    }

    /// 记一笔「本机刚打开过这条」，然后落盘。
    ///
    /// 写盘失败不拦人：排序退化成老样子，用户照样能用 —— 只往日志里扔一行。
    /// 文件很小（几百条 = 几 KB），点开会话本来就不是高频动作，不值得上防抖队列。
    fn touch_recent(&mut self, id: i64) {
        self.recent.touch(id);
        if let Err(e) = self.recent.save() {
            log::warn!("最近打开记不下来：{e}");
        }
    }

    pub(crate) fn select_session(&mut self, id: i64, force: bool, cx: &mut Context<Self>) {
        if self.selected == Some(id) && !force {
            return;
        }
        self.selected = Some(id);
        // 记一笔「本机什么时候打开过它」。只喂排序（`sort_sessions`），不写引擎。
        self.touch_recent(id);
        self.nav.reset();
        self.entries.clear();
        self.entries_error = None;
        self.turn_error = None;
        self.entries_loading = true;
        self.streaming = false;
        self.cfg.open = false;
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
                        // 目录默认高亮最新一轮：主视图停在底部，别从第一条亮起。
                        let groups = this.nav_groups();
                        this.nav.current = groups
                            .iter()
                            .flat_map(|g| g.rounds.iter())
                            .last()
                            .map(|r| r.index)
                            .unwrap_or(0);
                        this.scroll.scroll_to_bottom();
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
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let Some(h) = crate::TOKIO.get().cloned() else {
            return;
        };
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

    // ── 输入区 `/` 技能命令 ─────────────────────────────────────────

    /// 面板在不在场。只看输入框当前内容这一份真相（不另存 query）：
    /// 整段以 `/` 开头、后面没有空白、且没被 Esc 收起过。
    fn slash_visible(&self, cx: &App) -> bool {
        !self.slash.dismissed
            && self.selected.is_some()
            && !self.newchat.open
            && crate::slash::command(&self.input.read(cx).value()).is_some()
    }

    /// `GET /v1/skills` 只拉一次。失败也不重试 —— render 里若每次失败都 notify，
    /// 就成了「失败 → 重画 → 再拉」的死循环；记一句话，面板给提示。
    fn load_skills(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        if self.skills_loading || self.skills.is_some() || self.skills_error.is_some() {
            return;
        }
        self.skills_loading = true;
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h.spawn(async move { engine.skills().await }).await;
            let _ = this.update(cx, |this, cx| {
                this.skills_loading = false;
                match got {
                    Ok(Ok(list)) => {
                        log::info!("技能清单读到了：{} 条", list.skills.len());
                        this.skills = Some(list);
                    }
                    Ok(Err(e)) => this.skills_error = Some(format!("技能清单拉不到：{e}")),
                    Err(e) => this.skills_error = Some(format!("技能清单拉不到：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 每次 render 重算一次过滤结果。放 render 里而不是订阅里：清单是异步回来的，
    /// 回来那一刻也得重算，render 天然覆盖两种时机。
    fn refresh_slash(&mut self, cx: &App) {
        if !self.slash_visible(cx) {
            self.slash_items.clear();
            return;
        }
        let value = self.input.read(cx).value().to_string();
        let q = crate::slash::command(&value).unwrap_or("");
        let all = self
            .skills
            .as_ref()
            .map(|s| s.skills.as_slice())
            .unwrap_or(&[]);
        self.slash_items = crate::slash::matches(all, q);
        self.slash.clamp(self.slash_items.len());
    }

    /// 当前会话挂着哪些技能。**以 `sessions` 为准**（3 秒轮询会把它刷成引擎的真相），
    /// 不自己另存一份 —— 那会跟真相分叉。
    fn session_skill_names(&self) -> Vec<String> {
        self.selected
            .and_then(|id| self.sessions.iter().find(|s| s.id == id))
            .map(|s| s.skills.clone())
            .unwrap_or_default()
    }

    /// 点一项 / ↵：算出目标数组 → POST /config → 拿回生效值覆盖本地。
    ///
    /// **不做乐观更新**：会话配置是真相，写失败就还是原样，别让界面先骗人。
    fn apply_skill(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.slash.busy {
            return;
        }
        let Some(skill) = self.slash_items.get(ix) else {
            return;
        };
        let Some(id) = self.selected else { return };
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let wanted = crate::slash::toggle(&self.session_skill_names(), &skill.name);
        self.slash.busy = true;
        self.slash.error = None;
        cx.notify();
        log::info!("技能配置：会话 {id} → {wanted:?}");

        cx.spawn_in(window, async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let patch = nous_engine::types::ConfigPatch {
                skills: Some(wanted),
                ..Default::default()
            };
            let got = h
                .spawn(async move { engine.config(id, &patch).await })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.slash.busy = false;
                match got {
                    Ok(Ok(back)) => {
                        // 回包是**生效值**。覆盖本地会话，界面立刻对上真相。
                        if let Some(s) = this.sessions.iter_mut().find(|s| s.id == id) {
                            s.skills = back.skills.clone();
                        }
                        // 选完把 `/xxx` 从输入框里清掉，别让它留着挡下一次输入。
                        this.input
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        this.slash.reset();
                        this.slash.dismissed = true;
                    }
                    Ok(Err(e)) => this.slash.error = Some(format!("没写进去：{e}")),
                    Err(e) => this.slash.error = Some(format!("没写进去：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 画 `/` 面板（或 `None`）。
    fn slash_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.slash_visible(cx) {
            return None;
        }
        let empty_note = if let Some(e) = &self.skills_error {
            Some(e.as_str())
        } else if self.skills.is_none() {
            Some("技能清单加载中…")
        } else {
            None
        };
        let on_pick: crate::slash::OnPick = std::rc::Rc::new(
            cx.listener(|this, ix: &usize, window, cx| this.apply_skill(*ix, window, cx)),
        );
        Some(crate::slash::panel(
            self.slash_items.clone(),
            self.session_skill_names(),
            self.slash.cursor,
            self.slash.busy,
            self.slash.error.clone(),
            empty_note.map(str::to_string),
            on_pick,
        ))
    }

    // ── 输入区左下角那颗 DNA：这条对话的设置 ───────────────────────

    fn cur_session(&self) -> Option<&Session> {
        let id = self.selected?;
        self.sessions.iter().find(|s| s.id == id)
    }

    /// 面板在不在场。跟 `/` 面板一样：没选会话、或者在新对话盘面上，都不该出现。
    fn cfg_visible(&self) -> bool {
        self.cfg.open && self.selected.is_some() && !self.newchat.open
    }

    /// 开 / 关。打开时顺手拉清单，并把 `/` 那套收掉 —— 两个面板不共存。
    fn toggle_config(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.cfg.open {
            self.cfg.open = false;
            cx.notify();
            return;
        }
        if self.selected.is_none() || self.newchat.open {
            return;
        }
        self.cfg.open = true;
        // 每开一次都回到第一页：停在上一条会话看过的那页只会让人困惑。
        self.cfg.tab = None;
        self.cfg.error = None;
        self.slash.dismissed = true;
        self.cfg_load(cx);
        self.load_skills(cx);
        // 目录从这条会话自己的 cwd 起，不从面板上次待的地方起。
        let cwd = self.cur_session().and_then(|s| s.cwd.clone());
        self.cfg_enter_dir(cwd, cx);
        cx.notify();
    }

    /// 拉三个清单（技能跟 `/` 共用 `load_skills`，不重复拉第二遍）。
    /// 一次只发一批，失败也记一句话不重试 —— 否则 render 里就是「失败 → 重画 → 再拉」。
    fn cfg_load(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        if self.cfg.loading || self.cfg.models.is_some() {
            return;
        }
        self.cfg.loading = true;
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move {
                    let models = engine.models().await;
                    let prompts = engine.prompts().await;
                    let machines = engine.machines().await;
                    (models, prompts, machines)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cfg.loading = false;
                match got {
                    Ok((m, p, mc)) => {
                        // **逐份独立保存**：以前只在「三份全 Ok」时才落盘，
                        // 结果内核偶发 502 时成功的那两份也一起丢了，右栏整块变报错。
                        let mut errs: Vec<String> = Vec::new();
                        match m {
                            Ok(m) => {
                                log::info!("配置面板：模型 {} 个", m.items.len());
                                this.cfg.models = Some(m);
                            }
                            Err(e) => errs.push(format!("模型：{e}")),
                        }
                        match p {
                            Ok(p) => {
                                log::info!("配置面板：模板 {} 条", p.prompts.len());
                                this.cfg.prompts = Some(p);
                            }
                            Err(e) => errs.push(format!("模板：{e}")),
                        }
                        match mc {
                            Ok(mc) => {
                                log::info!("配置面板：机器 {} 台", mc.machines.len());
                                this.cfg.machines = Some(mc);
                            }
                            Err(e) => errs.push(format!("机器：{e}")),
                        }
                        // 只留一句话给「那一页」用，别再拿它遮住整块右栏。
                        this.cfg.error = (!errs.is_empty())
                            .then(|| format!("清单拉不到 —— {}", errs.join("；")));
                    }
                    Err(e) => this.cfg.error = Some(format!("清单拉不到：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 进某个目录。`None` = 从这条会话的 cwd 起。
    fn cfg_enter_dir(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        if let Some(reason) = self.dead_path_disabled_reason("v1/fs/dirs") {
            self.cfg.error = Some(reason);
            cx.notify();
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let want = path.or_else(|| self.cur_session().and_then(|s| s.cwd.clone()));
        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move { engine.dirs(want.as_deref(), None).await })
                .await;
            let _ = this.update(cx, |this, cx| {
                match got {
                    Ok(Ok(d)) => this.cfg.dirs = Some(d),
                    // 目录读不到只影响目录那一页，别把模型那份 error 也污染了。
                    Ok(Err(e)) => log::warn!("目录读不到：{e}"),
                    Err(e) => log::warn!("目录读不到：{e}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 把一份 patch 发给内核。回包是**生效值**，拿它覆盖本地会话 ——
    /// 不能自己把改的值写上去：换模型时服务端会把档位按新模型重夹一次，
    /// 那个结果只有回包里有。
    fn cfg_apply(&mut self, patch: ConfigPatch, cx: &mut Context<Self>) {
        if let Some(reason) = self.dead_path_disabled_reason("v1/sessions/{id}/config") {
            self.cfg.error = Some(reason);
            cx.notify();
            return;
        }
        let Some(id) = self.selected else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            return;
        };
        if self.cfg.busy {
            return;
        }
        self.cfg.busy = true;
        self.cfg.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let Some(h) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = h
                .spawn(async move { engine.config(id, &patch).await })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cfg.busy = false;
                match got {
                    Ok(Ok(back)) => {
                        if let Some(s) = this.sessions.iter_mut().find(|s| s.id == id) {
                            s.model = back.model.clone();
                            s.thinking_level = back.thinking_level.clone();
                            s.skills = back.skills.clone();
                            s.template = back.template.clone();
                            s.read_only = back.read_only;
                            s.cwd = back.cwd.clone();
                            s.machine = back.machine.clone();
                        }
                        log::info!(
                            "配置写进去了：会话 {id} → {}",
                            back.model.as_deref().unwrap_or("—")
                        );
                    }
                    Ok(Err(e)) => this.cfg.error = Some(format!("没写进去：{e}")),
                    Err(e) => this.cfg.error = Some(format!("没写进去：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 工具行最左边那颗。以前是个死的笑脸图标，这里换成一个真按钮。
    fn cfg_first_tool(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.cfg_visible();
        div()
            .id(ElementId::Name("tool-config".into()))
            .flex()
            .items_center()
            .justify_center()
            .size(t().d(28.))
            .rounded(t().r(6.))
            .when(open, |el| el.bg(rgba(t().p.accent_tint)))
            .hover(|s| s.cursor_pointer().bg(rgba(t().p.scrim)))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_config(window, cx);
            }))
            .child(
                Icon::new(IconName::Dna)
                    .size(t().d(18.))
                    .text_color(rgb(if open { t().p.accent_fg } else { t().p.text_2 })),
            )
            .into_any_element()
    }

    /// 把面板浮在聊天区左下角。挂在**整窗**根节点上，父容器够大，hitbox 不会被裁。
    fn cfg_overlay(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let g = crate::config_panel::Geometry::fit(
            window.viewport_size().width.as_f32(),
            window.viewport_size().height.as_f32(),
        );
        let panel = self.cfg_panel(g, cx)?;
        Some(
            div()
                .absolute()
                .left(px(g.left))
                .bottom(px(g.bottom))
                .child(panel)
                .into_any_element(),
        )
    }

    /// 画面板（或 `None`）。回调全在这里接上，`config_panel` 只管画。
    fn cfg_panel(
        &self,
        g: crate::config_panel::Geometry,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.cfg_visible() {
            return None;
        }
        let s = self.cur_session()?;

        let hooks = crate::config_panel::Hooks {
            hover: Rc::new(cx.listener(|this, hovered: &bool, _w, cx| {
                this.cfg_bar_hover(*hovered, cx)
            })),
            tab: Rc::new(cx.listener(|this, t: &crate::config_panel::Tab, _w, cx| {
                this.cfg.tab = Some(*t);
                cx.notify();
            })),
            level: Rc::new(cx.listener(|this, v: &String, _w, cx| {
                this.cfg_apply(
                    ConfigPatch {
                        thinking_level: Some(v.clone()),
                        ..Default::default()
                    },
                    cx,
                );
            })),
            model: Rc::new(cx.listener(|this, v: &(String, String), _w, cx| {
                let (provider, model_id) = v;
                // 空 modelId = 只换了供应商。模型得跟着换成这家的第一个，
                // 不然会停在别家的 modelId 上，服务端直接 400。
                let model_id = if model_id.is_empty() {
                    this.cfg
                        .models
                        .as_ref()
                        .and_then(|m| m.items.iter().find(|it| &it.provider == provider))
                        .map(|it| it.model_id.clone())
                        .unwrap_or_default()
                } else {
                    model_id.clone()
                };
                if model_id.is_empty() {
                    return;
                }
                this.cfg_apply(
                    ConfigPatch {
                        model: Some(format!("{provider}/{model_id}")),
                        ..Default::default()
                    },
                    cx,
                );
            })),
            skill: Rc::new(cx.listener(|this, name: &String, _w, cx| {
                // 内核对技能是**相对改法**（只增删差集），所以这里传完整目标数组就行，
                // 不用自己算增删 —— 跟 `/` 面板走的是同一条路。
                let wanted = crate::slash::toggle(&this.session_skill_names(), name);
                this.cfg_apply(
                    ConfigPatch {
                        skills: Some(wanted),
                        ..Default::default()
                    },
                    cx,
                );
            })),
            template: Rc::new(cx.listener(|this, v: &Option<String>, _w, cx| {
                this.cfg_apply(
                    ConfigPatch {
                        template: Some(v.clone()),
                        ..Default::default()
                    },
                    cx,
                );
            })),
            machine: Rc::new(cx.listener(|this, v: &Option<String>, _w, cx| {
                this.cfg_apply(
                    ConfigPatch {
                        machine: Some(v.clone()),
                        ..Default::default()
                    },
                    cx,
                );
            })),
            dir: Rc::new(cx.listener(|this, path: &String, _w, cx| {
                // 点目录是**导航**不是写配置：只拉那一层，不动会话的 cwd。
                this.cfg_enter_dir(Some(path.clone()), cx);
            })),
            readonly: Rc::new(cx.listener(|this, on: &bool, _w, cx| {
                this.cfg_apply(
                    ConfigPatch {
                        read_only: Some(*on),
                        ..Default::default()
                    },
                    cx,
                );
            })),
        };

        Some(crate::config_panel::render(
            &self.cfg,
            self.skills.as_ref(),
            s,
            &hooks,
            g,
        ))
    }

    // ── 列表 / 气泡 ─────────────────────────────────────────

    /// 发一条。SSE 走的是 `requestId` 幂等锚：重发不会答两遍。
    ///
    /// 返回「这一句到底有没有发出去」：`retry_last` 靠它决定红条留不留。
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // 面板在场时，回车归面板（拦键在 interceptor 里做）；这里再兜一道，
        // 防止某个路径绕过去把 `/deep` 当消息发出去。
        if self.slash_visible(cx) {
            return false;
        }
        // 生成中**不发**：回车这时只想接着写，不该把半句话塞进队列。
        // 引擎的 `whenBusy` 值域还没确认，先不传 —— 确认了只需要在这一个入口加分支。
        if self.streaming {
            log::info!("正在生成，这一句不发（回车被拦下）");
            return false;
        }
        let Some(id) = self.selected else {
            return false;
        };
        let Some(engine) = self.engine.clone() else {
            return false;
        };
        let text = self.input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return false;
        }
        // 没配凭证不是「功能关掉」：输入框照常能打、这一条照常发，
        // 只是先把 503 说在前面（Nous #44 的语义）。
        if self
            .caps
            .as_ref()
            .is_some_and(|cap| !cap.runtime.provider.configured)
        {
            let message = self
                .caps
                .as_ref()
                .map(|cap| cap.runtime.provider.message.clone())
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "没配凭证，这一条可能回 503".to_string());
            self.notify(message, NoticeKind::Info, cx);
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
        // 回执**不看焦点**：手刚离开回车，响的读就是这一刻。默认开（`sound.toml` 可关）。
        crate::sound::chime(self.sound, crate::sound::Cue::Send, true);
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
        let Some(engine) = self.engine.clone() else {
            return;
        };
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
                let Some(h) = crate::TOKIO.get().cloned() else {
                    break;
                };
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
                                        // 你看着屏幕时，红条自己会跳出来；不看着才需要声音。
                                        crate::sound::chime(
                                            this.sound,
                                            crate::sound::Cue::Error,
                                            !this.focused,
                                        );
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
                                crate::sound::chime(
                                    this.sound,
                                    crate::sound::Cue::Error,
                                    !this.focused,
                                );
                                this.turn_error = Some(err);
                            }
                            cx.notify();
                            return true;
                        }
                        this.live = Some(drained.text);
                        // 工具卡片：这一轮调过哪些工具，SSE 里是带名字的（`ToolRun.name`）。
                        // 以前 `Drained.tools` 没人看，所以 `live.rs` 里那两个字段一直是死代码。
                        this.live_tools = drained.tools;
                        // SSE 首帧带来的时间戳：按天分组目录靠它，不自己编日期。
                        if let Some(times) = drained.times {
                            this.nav.times = times;
                        }
                        this.scroll.scroll_to_bottom();
                        if drained.done {
                            // 一轮结束：交出去，历史以引擎为准重拉一遍。流不关，继续等下一轮。
                            // **只在窗口不在前台时响**：正看着它跑完，视觉反馈已经够了；
                            // 而"发出去、切去干活、回来看见跑完了"正是这一声的全部意义。
                            crate::sound::chime(
                                this.sound,
                                crate::sound::Cue::Done,
                                !this.focused,
                            );
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
        let Some(engine) = self.engine.clone() else {
            return;
        };
        if !self.streaming {
            return;
        }
        self.streaming = false;
        self.user_stopped = true;
        crate::sound::chime(self.sound, crate::sound::Cue::Stop, true);
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

    // ── 归档 / 撕原件 / 轻提示（票 #6）──────────────────────

    /// 弹一条轻提示：做成的事绿、失败红，4 秒后自己收。
    ///
    /// 只在这里点火，**不在 `render` 里点火** —— 否则就是「重画 → 再点火」的死循环。
    pub(crate) fn notify(
        &mut self,
        text: impl Into<String>,
        kind: NoticeKind,
        cx: &mut Context<Self>,
    ) {
        let token = self.notice.show(text, kind);
        cx.notify();
        cx.spawn(async move |this, cx| {
            Timer::after(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                // 期间又弹了一条：代号对不上，让它自己等。
                if this.notice.expire(token) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 引擎报错的提示：`message` 原文逐字，不加前缀、不翻译、不截断。
    fn notify_error(&mut self, err: &EngineError, cx: &mut Context<Self>) {
        self.notify(error_notice(err), NoticeKind::Error, cx);
    }

    /// 一段引擎调用的骨架：起任务 → 等 → 三种结局分开。
    ///
    /// 归档 / 撕原件 / 打捞 / 拉归档库四处原先各抄一遍同样的
    /// `Ok(Ok(_)) / Ok(Err(e)) / Err(e)` + `log::warn!` + `update`，
    /// 改一处忘三处（评审 J4）。这里只统一「怎么等、怎么区分两类失败、失败一定留日志」；
    /// 成功之后刷什么、失败画到哪（轻提示 / 归档库页头红字），由调用方注入 ——
    /// 四处在这些地方本来就不一样，别为了少写一行把它们抹平。
    ///
    /// `failed` / `task_failed` 是两句日志的固定部分（各站点主语不同，原样保留）。
    fn run_engine<T, Fut, OnOk, OnErr>(
        &mut self,
        cx: &mut Context<Self>,
        failed: &str,
        task_failed: &str,
        work: Fut,
        on_ok: OnOk,
        on_err: OnErr,
    ) where
        T: Send + 'static,
        Fut: std::future::Future<Output = std::result::Result<T, EngineError>> + Send + 'static,
        OnOk: FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        OnErr: FnOnce(&mut Self, EngineFailure, &mut Context<Self>) + 'static,
    {
        let Some(handle) = crate::TOKIO.get().cloned() else {
            return;
        };
        let failed = failed.to_owned();
        let task_failed = task_failed.to_owned();
        cx.spawn(async move |this, cx| {
            let got = handle.spawn(work).await;
            let _ = this.update(cx, |this, cx| {
                match got {
                    Ok(Ok(value)) => on_ok(this, value, cx),
                    Ok(Err(e)) => {
                        log::warn!("{failed}：{e}");
                        on_err(this, EngineFailure::Engine(e), cx);
                    }
                    Err(e) => {
                        log::warn!("{task_failed}：{e}");
                        on_err(this, EngineFailure::Task(e.to_string()), cx);
                    }
                }
                // 成功要重画（刷新过的列表），失败也要（红字 / 页头错误）。
                cx.notify();
            });
        })
        .detach();
    }

    /// 走轻提示的失败出口（归档 / 撕原件 / 打捞共用）。
    fn notify_engine_failure(&mut self, failure: EngineFailure, cx: &mut Context<Self>) {
        match failure {
            EngineFailure::Engine(e) => self.notify_error(&e, cx),
            EngineFailure::Task(e) => {
                self.notify(format!("任务挂了：{e}"), NoticeKind::Error, cx)
            }
        }
    }

    /// `Desk#13`：四条灰路径共用一个判定入口（归档 2 + 会话 archive/config/purge 3，
    /// 其中 archive 与归档库同一件事）。UI 只问这里，理由走 `disabled_reason()`。
    pub(crate) fn dead_path_disabled_reason(&self, path: &str) -> Option<String> {
        use crate::capability::Feature;
        // capabilities.reason.message 是唯一真相；端点没声明走统一 Unknown 文案。
        if let Some(caps) = self.caps.as_ref() {
            if let Some(reason) = crate::capability::endpoint_reason(caps, path) {
                return Some(reason);
            }
        }
        match path {
            "v1/archive" | "v1/archive/{id}/restore" | "v1/sessions/{id}/archive" => {
                self.disabled_reason(Feature::Archive)
            }
            "v1/sessions/{id}/purge" | "v1/sessions/{id}/config" => {
                self.disabled_reason(Feature::Sessions)
            }
            "v1/fs/dirs" => self.disabled_reason(Feature::Device),
            _ => None,
        }
    }

    /// 打开归档库（右栏整页）。入口在「≡ 更多」菜单里。
    pub(crate) fn open_archive(&mut self, cx: &mut Context<Self>) {
        if !self.feature_enabled(crate::capability::Feature::Archive) {
            self.menu_open = false;
            cx.notify();
            return;
        }
        self.menu_open = false;
        self.settings_open = false;
        self.archive_open = true;
        self.archive_page_load(cx);
        cx.notify();
    }

    /// 收起归档库，右栏回到聊天区。
    pub(crate) fn close_archive(&mut self, cx: &mut Context<Self>) {
        if self.archive_open {
            self.archive_open = false;
            cx.notify();
        }
    }

    /// 拉 `GET /v1/archive`。数据留着，只在页头标「刷新中」——
    /// 刷新时把整页清空，看着像「东西没了」。
    fn archive_page_load(&mut self, cx: &mut Context<Self>) {
        if !self.feature_enabled(crate::capability::Feature::Archive) {
            self.archive.error = self.disabled_reason(crate::capability::Feature::Archive);
            cx.notify();
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.archive.error = Some("还没连上引擎 —— 去「≡ 更多」里重新连接".to_string());
            cx.notify();
            return;
        };
        // 没有 tokio 就别把页头挂成「刷新中」—— 那个状态永远等不到回调去收。
        if crate::TOKIO.get().is_none() {
            return;
        }
        self.archive.loading = true;
        self.archive.error = None;
        cx.notify();
        self.run_engine(
            cx,
            "归档库拉不到",
            "归档库任务挂了",
            async move { engine.archive_list().await },
            |this, data, _cx| {
                this.archive.loading = false;
                this.archive.data = data;
                // 刷新后选中的那行可能已经不在了（打捞 / 撕原件之后）：
                // 别留一个指向空气的高亮。
                let selected_gone = this.archive.selected.as_ref().is_some_and(|key| {
                    !this.archive.data.gens.iter().any(|g| {
                        g.r#gen == key.r#gen && g.items.iter().any(|i| i.id == key.id)
                    })
                });
                if selected_gone {
                    this.archive.clear_selection();
                }
            },
            |this, failure, _cx| {
                // 归档库里失败画在**页头**，不弹轻提示：整页都在讲这件事，
                // 再飘一条红的会重复，而且关掉以后就找不到原因了。
                this.archive.loading = false;
                this.archive.error = Some(match failure {
                    EngineFailure::Engine(e) => error_notice(&e),
                    EngineFailure::Task(e) => format!("任务挂了：{e}"),
                });
            },
        );
    }

    /// 重拉 `/v1/sessions`（排序照 `start_poll` 那套，不另拼一份），落进 `self.sessions`。
    ///
    /// `archived`：刚归档 / 刚撕掉的那条。若它正是当前打开的会话，就按
    /// [`next_selection_after_archive`] 退到最近一条；退无可退回空态（真停 SSE），
    /// 绝不留一个「选着一条已经不存在的会话」的空壳。
    ///
    /// **故意不走 [`Self::run_engine`]**：这次静默失败只是让界面留着上一份列表，
    /// 下一轮轮询会再拉 —— 它不是「常态」，也不代表失败可以藏着：
    /// 真正需要用户知道的失败（建会话 / 发送 / 归档）走 [`Self::run_engine`] 弹提示。
    fn reload_sessions(&mut self, archived: Option<i64>, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let Some(handle) = crate::TOKIO.get().cloned() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let got = handle.spawn(async move { engine.sessions().await }).await;
            let _ = this.update(cx, |this, cx| {
                let Ok(Ok(list)) = got else {
                    // 拉失败就交给下一次轮询，不把半截状态写进列表。
                    return;
                };
                let mut next = list.items;
                sort_sessions(&mut next, &this.recent);
                this.sessions = next;
                if let Some(gone) = archived.filter(|id| this.selected == Some(*id)) {
                    match next_selection_after_archive(&this.sessions, gone) {
                        Some(id) => this.select_session(id, true, cx),
                        None => this.leave_empty(cx),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 没有可退的会话：右栏回空态，并且**真的停掉** SSE（不是只把名单清空）。
    fn leave_empty(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.entries.clear();
        self.entries_loading = false;
        self.entries_error = None;
        self.turn_error = None;
        self.live = None;
        self.live_state = Live::new();
        self.stream_for = None;
        self.streaming = false;
        self.nav.reset();
        cx.notify();
    }

    /// `归档该聊天…`：复印进归档库 + 写墓碑。
    ///
    /// 成功后重拉主列表（那一行自己消失）、刷新开着的归档库、弹一条绿提示。
    /// 失败弹引擎 `{error}` 原文（409 会列出活分支，那些字不动）。
    fn archive_session(&mut self, id: i64, name: &str, cx: &mut Context<Self>) {
        if let Some(reason) = self.dead_path_disabled_reason("v1/sessions/{id}/archive") {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        log::info!("归档会话：{name}（#{id}）");
        self.run_engine(
            cx,
            &format!("归档没成：{id}"),
            &format!("归档任务挂了：{id}"),
            async move { engine.archive(id).await },
            move |this, _done, cx| {
                this.notify("已归档 · 可在归档库捞回", NoticeKind::Info, cx);
                this.reload_sessions(Some(id), cx);
                if this.archive_open {
                    this.archive_page_load(cx);
                }
            },
            Self::notify_engine_failure,
        );
    }

    /// 确认之后真撕。**不可撤销**，所以只能从 [`PurgeConfirm::on_confirm`] 走到这里。
    fn purge_session(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(reason) = self.dead_path_disabled_reason("v1/sessions/{id}/purge") {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        log::info!("撕原件：#{id}");
        self.run_engine(
            cx,
            &format!("撕原件没成：{id}"),
            &format!("撕原件任务挂了：{id}"),
            async move { engine.purge(id).await },
            move |this, _done, cx| {
                this.notify("已撕掉原件", NoticeKind::Info, cx);
                // 撕掉的要是正开着的那条，同样退让；不在列表里就只是重拉一次。
                this.reload_sessions(Some(id), cx);
                if this.archive_open {
                    this.archive_page_load(cx);
                }
            },
            Self::notify_engine_failure,
        );
    }

    /// 危险确认里按了「撕掉原件」：拿到 `(id, name)` 才真的发请求。
    ///
    /// 两条路：
    /// - 右键「归档并撕原件…」：先归档，**归档成功**才接撕（绝不裸删）；
    /// - 归档库页「撕原件」：已经归档过，直接撕。
    fn confirm_purge(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.purge_confirm.on_confirm() else {
            cx.notify();
            return;
        };
        cx.notify();
        // 会话 id 是**数字**（`wire.md`）：确认层存的就是 `i64`，这里不 parse、
        // 也没有「不是数字」的兜底分支（评审 J2）。
        if action.chain_archive {
            self.archive_then_purge(action.session_id, cx);
        } else {
            self.purge_session(action.session_id, cx);
        }
    }

    /// 右键那一步：先归档，成功后**自动接撕原件**。
    ///
    /// 关键在「成功才接」—— 归档要是 409（忙 / 有活分支 / 没复印件），引擎的
    /// 原文照旧给用户看（`notify_engine_failure`），**绝不继续往下撕**。
    /// 这一步不是原子的：归档成了、撕万一失败，会话会留在归档库里（可打捞 / 再撕），
    /// 不是悬空状态 —— 这正是「先归档再撕」比裸删安全的地方。
    fn archive_then_purge(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        log::info!("归档并撕原件：#{id}（先归档）");
        self.run_engine(
            cx,
            &format!("归档没成，不撕：{id}"),
            &format!("归档任务挂了，不撕：{id}"),
            async move { engine.archive(id).await },
            // 归档成功 → 立刻接着撕。这里**再调一次 run_engine**，把撕的成功 /
            // 失败单独报给用户：两步各自有反馈，不会糊成一句"操作完成"。
            move |this, _done, cx| {
                this.archive_session_done(id, cx);
                this.purge_session(id, cx);
            },
            Self::notify_engine_failure,
        );
    }

    /// 归档成功后共用的收尾：重拉列表 / 刷新归档库 / 绿提示。
    /// `archive_session` 和 `archive_then_purge` 都用它，别各写一份。
    fn archive_session_done(&mut self, id: i64, cx: &mut Context<Self>) {
        self.notify("已归档 · 正在撕原件", NoticeKind::Info, cx);
        self.reload_sessions(Some(id), cx);
        if self.archive_open {
            self.archive_page_load(cx);
        }
    }

    /// 打捞：`POST /v1/archive/:id/restore`。成功后归档库 + 主列表都刷新 ——
    /// 只刷一边会让人以为没生效（徽标还是「已销毁」，或者列表里没回来）。
    fn restore_session(&mut self, id: i64, r#gen: String, cx: &mut Context<Self>) {
        if let Some(reason) = self.dead_path_disabled_reason("v1/archive/{id}/restore") {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            return;
        };
        log::info!("打捞：#{id}（gen {gen}）");
        self.run_engine(
            cx,
            &format!("打捞没成：{id}"),
            &format!("打捞任务挂了：{id}"),
            async move { engine.restore(id, &r#gen).await },
            move |this, _done, cx| {
                this.notify("已捞回 · 会话回到主列表", NoticeKind::Info, cx);
                this.archive_page_load(cx);
                this.reload_sessions(None, cx);
            },
            Self::notify_engine_failure,
        );
    }

    /// 归档库页的出口：这一页只产出动作，真正干什么在这里。
    fn archive_on_action(
        &mut self,
        action: &crate::archive_page::ArchiveAction,
        cx: &mut Context<Self>,
    ) {
        use crate::archive_page::ArchiveAction as A;
        match action {
            A::Refresh => self.archive_page_load(cx),
            A::Restore { id, r#gen } => self.restore_session(*id, r#gen.clone(), cx),
            A::Purge { id, name } => {
                // 归档库页来：已经归档过，只撕（false）。
                self.purge_confirm.request(*id, name.clone(), false);
                cx.notify();
            }
        }
    }

    /// 画归档库整页（右栏）。三个回调全在这里接，`archive_page` 只管画。
    fn render_archive(&self, cx: &mut Context<Self>) -> AnyElement {
        use crate::archive_page::{ArchiveAction, OnAction, OnClick, OnHover, OnSelect, RowKey};
        let on_action: OnAction = Rc::new(cx.listener(|this, action: &ArchiveAction, _, cx| {
            this.archive_on_action(action, cx);
        }));
        let on_select: OnSelect = Rc::new(cx.listener(|this, key: &RowKey, _, cx| {
            this.archive.select(key.clone());
            cx.notify();
        }));
        let on_back: OnClick =
            Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| this.close_archive(cx)));
        let on_hover: OnHover = Rc::new(cx.listener(|this, hovered: &bool, _, cx| {
            this.bar_hover(Bar::Archive, *hovered, cx)
        }));
        crate::archive_page::render_archive_page(
            &self.archive,
            &self.archive_scroll,
            self.bar(Bar::Archive),
            on_hover,
            on_action,
            on_select,
            on_back,
        )
    }

    /// 打开「重命名会话」。
    ///
    /// 输入框的初值和焦点**不在这里填**：这条路是从右键菜单回调上来的，手上没有
    /// `window`。只立一个 `fresh` 标记，等 `rename_overlay` 那一帧再填。
    fn open_rename(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(name) = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
        else {
            // 列表刚刷过、那条没了：说一句，别静默（AGENTS：点下去没反应算 bug）。
            self.notify("这条会话不在了", NoticeKind::Info, cx);
            return;
        };
        log::info!("重命名会话 #{id}（原名字 {name}）");
        self.rename_box.request(id, name);
        cx.notify();
    }

    /// 「重命名会话」弹层。挂在整窗这一层（和右键菜单 / 撕原件同一层）。
    fn rename_overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.rename_box.is_open() {
            return None;
        }
        // 刚打开：把原名填进输入框、全选、抢焦点 —— 打开就能直接覆盖着打。
        if let Some((_, name)) = self.rename_box.take_fresh() {
            self.rename_input.update(cx, |input, cx| {
                input.set_value(name, window, cx);
            });
            let handle = self.rename_input.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
        let close = Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.rename_box.on_cancel();
            cx.notify();
        }));
        let close_handler = close.clone();
        Some(crate::rename_box::render_rename_box(
            &self.rename_box,
            &self.rename_input,
            cx.listener(|this, _: &ClickEvent, window, cx| this.confirm_rename(window, cx)),
            move |ev, window, cx| close_handler(ev, window, cx),
        ))
    }

    /// 点「完成」/ 在输入框里按回车。
    ///
    /// 空名字**不发给引擎**（`wire.md`：引擎会回 400 `name 是空的`）：弹层留着、
    /// 给一句提示，让用户接着改。省一次白跑，也省一句看不懂的错。
    fn confirm_rename(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.rename_input.read(cx).value().to_string();
        match self.rename_box.submit(&raw) {
            crate::rename_box::Submit::Nothing => {}
            crate::rename_box::Submit::Empty => {
                self.notify("名字不能是空的", NoticeKind::Info, cx);
            }
            crate::rename_box::Submit::Go(action) => self.do_rename(action, cx),
        }
        cx.notify();
    }

    /// 真发 `POST /v1/sessions/:id/rename`。
    ///
    /// 成功后**只改本地那一行的 `name`**，不重拉列表：引擎回的就是生效值（它 trim 过），
    /// 重拉会让整列闪一下，而改名并不改变排序。
    fn do_rename(&mut self, action: crate::rename_box::RenameAction, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        let crate::rename_box::RenameAction { session_id, name } = action;
        log::info!("改名 #{session_id} → {name}");
        self.run_engine(
            cx,
            &format!("改名没成：{session_id}"),
            &format!("改名任务挂了：{session_id}"),
            async move { engine.rename(session_id, &name).await },
            move |this, renamed, cx| {
                if let Some(s) = this.sessions.iter_mut().find(|s| s.id == renamed.id) {
                    s.name = renamed.name.clone();
                }
                this.notify(
                    format!("已改名为「{}」", renamed.name),
                    NoticeKind::Info,
                    cx,
                );
            },
            Self::notify_engine_failure,
        );
    }

    /// 「撕原件」危险确认，挂在整窗这一层（和菜单 / 配置面板同一层）。
    fn purge_overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.purge_confirm.is_open() {
            return None;
        }
        // 刚打开：清空输入框、 focuses —— 让用户**亲手打一遍**名字。
        // 预填原名再让用户点确定，等于没设卡片（手输的意义就是那几下键盘）。
        if self.purge_confirm.take_fresh() {
            self.purge_input.update(cx, |input, cx| input.set_value("", window, cx));
            let handle = self.purge_input.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
        let typed = self.purge_input.read(cx).value().to_string();
        let close = Rc::new(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.purge_confirm.on_cancel();
            cx.notify();
        }));
        let close_handler = close.clone();
        Some(crate::archive_confirm::render_purge_confirm(
            &self.purge_confirm,
            &self.purge_input,
            &typed,
            cx.listener(|this, _: &ClickEvent, _, cx| this.confirm_purge(cx)),
            move |ev, window, cx| close_handler(ev, window, cx),
        ))
    }

    /// 最上层的轻提示。没有就 `None`（`children` 吃掉 `Option` 不花钱）。
    fn render_notice(&self) -> Option<AnyElement> {
        let (text, kind) = self.notice.current()?;
        let (bg, fg) = match kind {
            NoticeKind::Error => (t().p.danger_soft, t().p.danger_fg),
            NoticeKind::Info => (t().p.accent_soft, t().p.accent_fg),
        };
        Some(
            div()
                .absolute()
                .left(t().d(0.))
                .right(t().d(0.))
                .bottom(t().d(28.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .max_w(t().d(620.))
                        .px(t().d(14.))
                        .py(t().d(9.))
                        .rounded(t().r(7.))
                        .bg(rgb(bg))
                        .shadow_lg()
                        .text_size(t().fs(12.5))
                        .text_color(rgb(fg))
                        .child(text.clone()),
                )
                .into_any_element(),
        )
    }

    // ── 三个栏 ──────────────────────────────────────────────

    fn render_icon_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // 微信桌面版最左的深色窄条：聊天 / 通讯录两个入口，底部「≡ 更多」。
        div()
            .flex()
            .flex_col()
            .w(px(t().rail_w))
            .h_full()
            .bg(rgb(t().p.sidebar))
            .items_center()
            .py(t().d(10.))
            .gap(t().d(6.))
            .child(self.tab_button(IconName::MessageCircle, Tab::Chats, true, cx))
            .child(self.gated_tab_button(
                IconName::BookUser,
                Tab::Templates,
                crate::capability::Feature::Presets,
                cx,
            ))
            .child(self.gated_tab_button(
                IconName::Bolt,
                Tab::Triggers,
                crate::capability::Feature::Triggers,
                cx,
            ))
            .child(self.tab_button(IconName::Layers, Tab::Resources, true, cx))
            .child(self.tab_button(IconName::Database, Tab::Data, true, cx))
            .child(div().flex_1())
            .child(self.more_button(cx))
    }

    /// 底部那三个横杠。点和菜单同一个状态 —— 开着的时候按钮自己也亮着。
    fn more_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.menu_open;
        div()
            .id(ElementId::Name("rail-more".into()))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_menu(cx)))
            .flex()
            .items_center()
            .justify_center()
            .size(t().d(40.))
            .rounded(t().r(8.))
            .when(open, |el| el.bg(rgb(t().p.sidebar_open)))
            .hover(|el| el.bg(rgb(t().p.sidebar_hover)))
            .child(
                Icon::new(IconName::Menu)
                    .with_size(Size::Size(t().d(20.)))
                    .text_color(rgb(if open { t().p.on_accent } else { t().p.muted })),
            )
    }

    /// 灰入口：不可用时不挂点击 + 图标压灰。
    /// 这是 capability 判定的**显示层**，判定本身只在 `capability.rs`。
    ///
    /// **人话不在这里**：灰图标没有 tooltip（`Div::tooltip` 要一个 view builder，
    /// 为一个图标它不值这个钱）。理由那句话在设置页每项一行、`≡ 更多` 一行，
    /// 都来自同一个 `disabled_reason()` —— 别在注释里写「tooltip 说人话」，
    /// 那会让人以为已经有 tooltip 了（本行就是被这么写错过一次）。
    fn gated_tab_button(
        &self,
        name: IconName,
        tab: Tab,
        feature: crate::capability::Feature,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let reason = self.disabled_reason(feature);
        self.tab_button(name, tab, reason.is_none(), cx)
    }

    fn tab_button(
        &self,
        name: IconName,
        tab: Tab,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.tab == tab;
        let id = match tab {
            Tab::Chats => "tab-chats",
            Tab::Templates => "tab-templates",
            Tab::Triggers => "tab-triggers",
            Tab::Resources => "tab-resources",
            Tab::Data => "tab-data",
        };
        div()
            .id(ElementId::Name(id.into()))
            .when(enabled, |el| {
                el.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.switch_tab(tab, cx);
                }))
            })
            .when(!enabled, |el| el.opacity(0.4))
            .flex()
            .items_center()
            .justify_center()
            .size(t().d(40.))
            .rounded(t().r(8.))
            .hover(|el| el.bg(rgb(t().p.sidebar_hover)))
            .child(
                Icon::new(name)
                    .with_size(Size::Size(t().d(22.)))
                    .text_color(rgb(if active { t().p.accent } else { t().p.muted })),
            )
    }

    /// 配置面板正文的滚动条：状态跟着面板走（`cfg` 里那两份），不放 `bars` 数组 ——
    /// 面板关掉再打开，滚动位置和条子都该跟着面板一起重置。
    pub(crate) fn cfg_bar_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.cfg.body_bar.hover(hovered, cx, |this| &mut this.cfg.body_bar);
    }

    /// 悬停 → 「滚动条在不在场」。
    ///
    /// 状态机在 [`scrollbar::BarState::hover`] 里（移开要等 250ms 淡出跑完才撤元素），
    /// 这里只负责按面板挑出那一份状态。
    pub(crate) fn bar_hover(&mut self, which: Bar, hovered: bool, cx: &mut Context<Self>) {
        let i = which.idx();
        self.bars[i].hover(hovered, cx, move |this| &mut this.bars[i]);
    }

    /// 取某个面板那两位：`(元素在不在场, 鼠标在不在上面)`。
    pub(crate) fn bar(&self, which: Bar) -> (bool, bool) {
        let bar = &self.bars[which.idx()];
        (bar.shown, bar.hover)
    }

    /// 输入区的悬停提示：鼠标放上去 → 淡入一行浅灰字（「Enter 发送 · Shift+Enter 换行」），
    /// 移开 → 250ms 淡出。跟 `bar_hover` 同一套状态机，只是不经过 `Bar` 枚举。
    fn composer_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.composer_hover == hovered {
            return;
        }
        self.composer_hover = hovered;
        self.composer_hint = true;
        cx.notify();
        if hovered {
            return;
        }
        cx.spawn(async move |this, cx| {
            Timer::after(scrollbar::FADE).await;
            this.update(cx, |this, cx| {
                if this.composer_hover {
                    return;
                }
                this.composer_hint = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// 当前搜索词（已 trim）。空 = 没在搜。
    fn query(&self, cx: &Context<Self>) -> String {
        self.search.read(cx).value().trim().to_string()
    }

    /// 列表顶上那个搜索框（现在是左列最上面一行）。常驻，不藏起来 ——
    /// 会话上百条之后「找得到」比「好看」重要。
    ///
    /// `q` / `total` / `hits` 都从调用点传进来：命中口径必须跟列表用的是同一个
    /// （`search::hit`），在这里再算一遍迟早会跟列表对不上。
    ///
    /// 高度交给组件（单行的 `.h()` 只管多行，设了也不生效 —— 试过了）。
    fn render_search_box(&self, q: &str, total: usize, hits: usize) -> AnyElement {
        // 计数写在框里（`suffix`），不另起一行：不搜索报总数，搜索中报「命中 / 总数」。
        // 一行省下来就是列表的 20px —— 会话上百条时这 20px 比一个字好看。
        let count = if q.is_empty() {
            format!("{total}")
        } else {
            format!("{hits} / {total}")
        };
        Input::new(&self.search)
            .small()
            .cleanable(true)
            .prefix(
                Icon::new(IconName::Search)
                    .text_color(rgb(t().p.faint))
                    .size(t().d(14.)),
            )
            .suffix(
                div()
                    .flex_none()
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .child(count),
            )
            .into_any_element()
    }

    /// 建会话的入口：一颗 `＋`，点开是一个下拉菜单。
    ///
    /// 菜单交给库的 `DropdownMenu`（`Popover` 那层）：Esc / 点外面 / 点完自动收起都由它管，
    /// 正好满足 `decisions.md` 那条「浮层至少两条出口」，本文件不另造浮层和开合状态。
    /// 现在菜单里只有一项（New Manual = 从零配的盘面）；以后加「从模板起 / 复制这条」
    /// 只加 `.item(...)`，不用改这段的骨架。
    fn render_new_chat_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.weak_entity();
        // 菜单实体是**打开那次**才建的（gpui-kit 用 keyed state 缓存），所以这个
        // 「有没有选中会话」的判定跟着那一次打开走，不会卡在首帧的值上。
        let has_sel = self.selected.is_some();
        Button::new("new-chat-btn")
            .ghost()
            .small()
            .compact()
            .icon(Icon::new(IconName::Plus).size(t().d(15.)))
            .tooltip("新建会话")
            // `TopRight`：菜单落在按钮**下方**、右边缘对齐（`Anchor` 说的是浮层自己
            // 贴着触发器的哪条边 —— 写 `BottomRight` 会翻到按钮上方去）。
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _window, _cx| {
                let mut menu = menu.min_w(t().d(216.));
                for (i, group) in crate::new_entry::menu().into_iter().enumerate() {
                    // 组与组之间来一条分隔线；第一组上面不画（贴着菜单顶）。
                    if i > 0 {
                        menu = menu.item(PopupMenuItem::separator());
                    }
                    if let Some(title) = group.title {
                        menu = menu.item(PopupMenuItem::label(title));
                    }
                    for entry in group.items {
                        menu = menu.item(Self::new_entry_item(entry, has_sel, weak.clone()));
                    }
                }
                menu
            })
            .into_any_element()
    }

    /// 菜单里的一行：图标 + 名字 + 右边那枚键帽。
    ///
    /// 用 `PopupMenuItem::element` 而不是 `new(...)`：`new` 只能挂一串纯文字，
    /// 键帽右对齐不了。没有选中会话时「复制设置」整行灰掉，键帽换成「先选一条」——
    /// 灰着还写着 Ctrl+D 会让人以为按了有用。
    fn new_entry_item(
        entry: crate::new_entry::Entry,
        has_sel: bool,
        weak: gpui::WeakEntity<Self>,
    ) -> PopupMenuItem {
        use crate::new_entry::Action;
        let label = entry.label;
        let keys = entry.keys;
        let (icon, disabled) = match entry.action {
            Action::Manual => (IconName::PenLine, false),
            Action::FromPreset => (IconName::LayoutTemplate, false),
            // 没选中会话时「从当前这条」无从谈起：灰掉，并写清楚为什么。
            Action::CopySettings => (IconName::Copy, !has_sel),
        };
        let action = entry.action;
        PopupMenuItem::element(move |_, _| {
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w(t().d(0.))
                .child(div().truncate().child(label))
                .when(disabled, |el| {
                    el.child(
                        div()
                            .ml_auto()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.faint))
                            .child("先选一条"),
                    )
                })
                .when(!disabled, |el| el.child(Self::kbd_chip(keys)))
        })
        .icon(Icon::new(icon).size(t().d(15.)))
        .disabled(disabled)
        .on_click(move |_, window, cx| {
            if let Some(this) = weak.upgrade() {
                this.update(cx, |this, cx| this.new_entry_click(action, window, cx));
            }
        })
    }

    /// 菜单项点下去。三个动作各自只有一行，真正的活都在别处。
    pub(crate) fn new_entry_click(
        &mut self,
        action: crate::new_entry::Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::new_entry::Action;
        match action {
            Action::Manual => self.board_open(window, cx),
            Action::FromPreset => self.open_preset_pick(cx),
            Action::CopySettings => self.copy_current_settings(cx),
        }
    }

    /// 三条快捷键：`Ctrl+N` 空白 / `Ctrl+Shift+N` 从模板起 / `Ctrl+D` 复制当前这条。
    ///
    /// 在 `intercept_keystrokes` 的 Ctrl 分支里叫。返回 `false` = 这个组合不归这里管
    /// （`Ctrl+S` / `Ctrl+K` 已经先走了）。
    fn new_entry_shortcut(
        &mut self,
        key: &str,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // 编辑器是整页，开在上面时这三条不归这里管（Ctrl+N 会把它顶掉）。
        if self.editor.open {
            return false;
        }
        match (key, shift) {
            ("n", true) => {
                self.open_preset_pick(cx);
                true
            }
            ("n", false) => {
                self.board_open(window, cx);
                true
            }
            ("d", false) => {
                // 没选中就什么都不做 —— 跟菜单里灰掉那行一致（提示已经在菜单里写过了，
                // 快捷键没菜单可看，这里补一条轻提示）。
                if self.selected.is_none() {
                    self.notify("先选一条会话", NoticeKind::Info, cx);
                    return true;
                }
                self.copy_current_settings(cx);
                true
            }
            _ => false,
        }
    }

    /// 「复制这条会话」：把当前会话的设置抄成一条新会话（**不带对话**）。
    ///
    /// 请求体在 [`crate::new_entry::copy_body`]（纯函数，带测试）。这里只做
    /// 「起任务 → 三种结局分开 → 成功就切过去」，跟 `launch_preset` 一个套路。
    fn copy_current_settings(&mut self, cx: &mut Context<Self>) {
        let Some(sess) = self.cur_session().cloned() else {
            self.notify("先选一条会话", NoticeKind::Info, cx);
            return;
        };
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        if self.copying {
            return;
        }
        self.copying = true;
        cx.notify();

        let body = crate::new_entry::copy_body(&sess);
        let from = sess.name.clone();
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            // create → 紧跟一次 sessions()，一次任务里带回落库后的列表。
            let outcome: Option<Result<(nous_engine::types::CreatedSession, Vec<Session>), EngineError>> =
                match &handle {
                    Some(h) => {
                        h.spawn(async move {
                            let created = engine.create(&body).await?;
                            let sessions = engine.sessions().await?.items;
                            Ok((created, sessions))
                        })
                        .await
                        .ok()
                    }
                    _ => None,
                };
            let _ = this.update(cx, |this, cx| {
                this.copying = false;
                match outcome {
                    Some(Ok((created, sessions))) => {
                        let mut sessions = sessions;
                        sessions.sort_by(|a, b| b.id.cmp(&a.id));
                        this.sessions = sessions;
                        // 复制出来的不该还赖在盘面上。
                        this.newchat.open = false;
                        this.newchat.sheet = None;
                        this.select_session(created.id, true, cx);
                        // 引擎说缺什么要如实说，别把 201 当全须全尾（wire.md 「建会话」）。
                        let missing = created.missing.len()
                            + created.missing_extensions.len()
                            + created.missing_templates.len();
                        if missing > 0 {
                            this.notify(
                                format!("已复制「{from}」，但有 {missing} 项引擎那边找不到"),
                                NoticeKind::Error,
                                cx,
                            );
                        } else {
                            this.notify(format!("已复制「{from}」的设置"), NoticeKind::Info, cx);
                        }
                    }
                    Some(Err(e)) => this.notify_error(&e, cx),
                    None => this.notify("任务没跑完", NoticeKind::Error, cx),
                }
            });
        })
        .detach();
    }

    /// 搜索中的列表：**先过滤，再按组摆**。
    ///
    /// 顺序是固定的「置顶 → 今天 → 7 天内 → 更早」，跟内核置顶和 `updated_at` 走；
    /// 组内保持 `ordered` 里的先后（置顶组就是 `pins` 给的顺序）。
    ///
    /// 没搜到不画空列表：给一句「没有找到」比一片白好认，至少知道是搜不到而不是列表塌了。
    fn search_rows(
        &self,
        ordered: &[&Session],
        q: &str,
        index: &mut ListIndex,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        use crate::search::{Group, group_of, hit};
        let today = chrono::Local::now().date_naive();
        let mut out: Vec<AnyElement> = Vec::new();
        let mut total = 0usize;
        for g in [Group::Pinned, Group::Today, Group::Week, Group::Older] {
            let items: Vec<&Session> = ordered
                .iter()
                .copied()
                .filter(|s| hit(s, q).is_some())
                .filter(|s| group_of(s.updated_at, self.pins.contains(&s.id), today) == g)
                .collect();
            if items.is_empty() {
                continue;
            }
            total += items.len();
            out.push(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .px(t().d(10.))
                    .pt(t().d(10.))
                    .pb(t().d(4.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.faint))
                    .when(g == Group::Pinned, |el| {
                        el.child(
                            Icon::new(IconName::Pin)
                                .size(t().d(11.))
                                .text_color(rgb(t().p.ok_fg)),
                        )
                    })
                    .child(format!("{} · {}", g.label(), items.len()))
                    .into_any_element(),
            );
            for s in &items {
                // 分组标题也占一个孩子位，所以按下标要记「这一行是第几个孩子」，
                // 不能只用它在 `items` 里的位次（键盘导航要用这个换 `scroll_to_item`）。
                index.push(s.id, out.len());
                out.push(self.session_row(s, q, cx));
            }
        }
        if total == 0 {
            out.push(
                div()
                    .w_full()
                    .py(t().d(28.))
                    .text_size(t().fs(12.))
                    .text_color(rgb(t().p.faint))
                    .text_center()
                    .child("没有找到相关会话")
                    .into_any_element(),
            );
        }
        out
    }

    /// 菜单右侧那枚键帽（`Ctrl+N` 这种）。
    ///
    /// 画出来比写在文档里管用 —— 不写等于没有快捷键。
    fn kbd_chip(label: &'static str) -> AnyElement {
        div()
            .flex_none()
            .ml(t().d(6.))
            .h(t().d(16.))
            .px(t().d(5.))
            .flex()
            .items_center()
            .rounded(t().r(4.))
            .bg(rgb(t().p.hover))
            .border_1()
            .border_color(rgb(t().p.line))
            .text_size(t().fs(10.))
            .text_color(rgb(t().p.muted))
            .child(label)
            .into_any_element()
    }

    /// 一行「可能带高亮」的文本。
    ///
    /// `q` 空的时候就是一根普通的截断文本 —— 跟加搜索之前**一字不差**；
    /// 在搜的时候拆成几段，命中的那段垫一层浅绿底。
    ///
    /// 为什么不用 `StyledText::with_runs`：`TextRun` 要求显式给 `Font`，而这里拿不到
    /// 「当前继承的是哪个字体」。拆 div 横排就不用管字体，代价是超宽时直接裁掉、
    /// 不补省略号。
    fn hl_line(text: &str, q: &str, size: Option<f32>, color: Option<u32>) -> AnyElement {
        if q.trim().is_empty() {
            let mut el = div().truncate();
            if let Some(s) = size {
                el = el.text_size(t().fs(s));
            }
            if let Some(c) = color {
                el = el.text_color(rgb(c));
            }
            return el.child(text.to_string()).into_any_element();
        }
        let mut row = div().flex().min_w(t().d(0.)).overflow_hidden();
        for (part, hot) in crate::search::runs(text, q) {
            let mut seg = div().flex_none().whitespace_nowrap();
            if let Some(s) = size {
                seg = seg.text_size(t().fs(s));
            }
            if let Some(c) = color {
                seg = seg.text_color(rgb(c));
            }
            if hot {
                // 高亮的底色 / 字色跟 demo 一致（淡绿底 + 深绿字）。
                seg = seg.bg(rgb(t().p.accent_soft)).text_color(rgb(t().p.accent_fg));
            }
            row = row.child(seg.child(part));
        }
        row.into_any_element()
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
        // 性能探针：`NOUS_PERF_SESSIONS=500` 时**只在渲染这一帧**把名单换成 500 条假行。
        // 不改状态 —— 改了会被轮询冲掉，而且假 id 会漏到点击里（`fake()` 说明写在那儿）。
        let fake: Vec<Session> = match crate::perf::rows() {
            Some(n) => crate::perf::fake(&self.sessions, n),
            None => Vec::new(),
        };
        let rows_of: &[Session] = if fake.is_empty() { &self.sessions } else { &fake };
        let mainline: Vec<&Session> = rows_of
            .iter()
            .filter(|s| s.parent.is_none())
            .collect();
        let in_order: Vec<i64> = mainline.iter().map(|s| s.id).collect();
        let ordered: Vec<&Session> = order_ids(&in_order, &self.pins)
            .into_iter()
            .filter_map(|id| mainline.iter().find(|s| s.id == id).copied())
            .collect();
        let q = self.query(cx);
        // 计数：不搜索报总数，搜索时报「命中 / 总数」。命中用 `search::hit` ——
        // 跟列表同一个口径，不另算一遍（两个口径迟早会不一致）。
        let total = mainline.len();
        let hits = if q.is_empty() {
            total
        } else {
            mainline
                .iter()
                .filter(|s| crate::search::hit(s, &q).is_some())
                .count()
        };
        let mut index = ListIndex::default();
        let t_build = std::time::Instant::now();
        let rows: Vec<AnyElement> = if q.is_empty() {
            // 没在搜：老样子。**一行都不动** —— 搜索不能把常态列表的顺序改了。
            ordered
                .iter()
                .enumerate()
                .map(|(slot, s)| {
                    index.push(s.id, slot);
                    self.session_row(s, &q, cx)
                })
                .collect()
        } else {
            self.search_rows(&ordered, &q, &mut index, cx)
        };
        // 量「这一帧构建了多少行的元素树」。注意这只是 CPU 那一半：布局和绘制在 GPUI 里
        // 另算，但列表行是纯文本 + 几个色块，元素树构建就是主成本。这条数一超标，
        // 剩下两半就不用量了。
        if !crate::perf::rows().is_none() {
            crate::perf::note(rows.len(), t_build.elapsed());
        }
        // 键盘导航跟着**这一帧真的画出来的东西**走：搜索分组、过滤、排序全已经反映在这里。
        // 每帧重算一份比在按键时另算一遍便宜得多（149 个 i64），也不会两边漂。
        *self.list_index.borrow_mut() = index;
        div()
            .flex()
            .flex_col()
            .w(px(t().list_w))
            .h_full()
            // `relative`：`＋` 菜单里「从模板起」那张清单绝对定位在这一列里
            // （见 `templates::preset_pick_overlay`）。
            .relative()
            .bg(rgb(t().p.list_bg))
            .child(
                // 顶部就一行：搜索框（计数内嵌右端）+ 一颗 `＋`。
                // 「NousDesk」那行删掉了 —— 四个不带任何操作的字白占 30px；新建入口也从
                // 一行文字缩成图标，点开是菜单（见 `render_new_chat_button`）。
                div()
                    .px(t().d(12.))
                    .pt(t().d(12.))
                    .pb(t().d(8.))
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(self.render_search_box(&q, total, hits)),
                    )
                    .child(self.render_new_chat_button(cx)),
            )
            // 连引擎中 / 连不上才占一行；正常时这行根本不存在（计数已经进了搜索框）。
            .children(
                self.status_line(cx)
                    .map(|s| div().px(t().d(12.)).pb(t().d(6.)).child(s)),
            )
            .child(
                div()
                    .id("session-scroll-area")
                    .relative()
                    .flex_1()
                    // `min_h(0)`：flex 项默认 `min-height: auto`，不加这行这个「视口」
                    // 会被内容撑到真实几万像素高，永远不需要滚（滚动条也就不会出现）。
                    .min_h(t().d(0.))
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
                        self.bar(Bar::List).0,
                        self.bar(Bar::List).1,
                    )),
            )
            // 「从模板起」的模板清单：盖在列表上、压在这一列里，不铺到整窗。
            .children(self.preset_pick_overlay(cx))
            .into_any_element()
    }

    /// 聊天区：9 号方案的短主视图 + 右侧历史抽屉。目录列由 `render()` 插在它左边。
    fn render_chat(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 窄窗（< 960）时顶栏要收着点：副标题和「历史与目录」四个字一起塞，
        // 640 宽会把右边那个按钮裁掉一半（截图里只剩「历史与目」）。
        let narrow = window.viewport_size().width.as_f32() < NAV_COLUMN_MIN_W;
        // 会话页的弹层跟盘面共用同一套（`board_sheet`）。但**右格宽度不一样**：
        // 会话页比盘面多一栏「历史与目录」（`nav_ui::NAV_W`），`pane_width` 没算它
        // —— 不扣掉的话弹层会按盘面的宽度居中，整块偏到右边去。
        let mut m = crate::board_art::M::new(window.viewport_size(), self.tab);
        m.w = (m.w - crate::nav_ui::NAV_W).max(1.);
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

        let session = self.sessions.iter().find(|s| s.id == id);
        let title = session
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("会话 {id}"));
        // 顶栏那行「它带什么」。盘面上选的东西进会话之后就在这儿落脚 —— 否则
        // 四张卡看起来只是建会话时的一次性表单。
        let brief = session.map(brief_of).unwrap_or_default();

        // 9 号方案：主视图只留最近 KEEP 条。`nav.main_from` 被抽屉的桥往前推时才会变长，
        // 顶栏给一颗「收起」把它还原。
        let from = self.nav.from_of(self.entries.len());
        let older = from;
        let short = self.nav.is_short(self.entries.len());

        let status: Vec<AnyElement> = if self.entries_loading {
            vec![
                div()
                    .p(t().d(12.))
                    .text_size(t().fs(12.))
                    .child("拉历史中…")
                    .into_any_element(),
            ]
        } else if let Some(e) = &self.entries_error {
            vec![
                div()
                    .p(t().d(12.))
                    .text_size(t().fs(12.))
                    .text_color(rgb(t().p.danger))
                    .child(format!("拉历史失败：{e}"))
                    .into_any_element(),
            ]
        } else if self.entries.is_empty() {
            vec![
                div()
                    .p(t().d(12.))
                    .text_size(t().fs(12.))
                    .child("还没消息，先从模板开一条。")
                    .into_any_element(),
            ]
        } else {
            vec![]
        };

        // 主视图的气泡。前面挂「更早 N 条」入口（9 号方案的那扇门）。
        let mut main_rows: Vec<AnyElement> = Vec::new();
        if older > 0 && status.is_empty() {
            main_rows.push(crate::nav_ui::more_bar(older, cx));
            main_rows.push(
                div()
                    .mb(t().d(12.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.faint))
                    .child("下面是最近几轮 · 旧的都在抽屉里")
                    .into_any_element(),
            );
        }
        main_rows.extend(status);
        if self.entries.len() > from {
            main_rows.extend(self.nav_rows(&self.entries[from..], cx));
        }

        // 抽屉里的全量正文。只有开着时才画 —— 不然每次重绘都要解析整段历史。
        let drawer_rows: Vec<AnyElement> = if self.nav.drawer_open {
            self.nav_rows(&self.entries, cx)
        } else {
            vec![]
        };

        div()
            // 弹层（「这个会话带什么」）按右格坐标 absolute 定位，得有个定位祖先。
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            // `min_w(0)` 不能省：flex 项默认 `min-width: auto`，历史里只要有一条
            // 折不开的长行（工具输出整段），这块就被撑得比窗口还宽（坑.md 16）。
            .min_w(t().d(0.))
            .h_full()
            .bg(rgb(t().p.chat_bg))
            .child(
                div()
                    .flex()
                    .h(t().d(48.))
                    .w_full()
                    .flex_none()
                    .items_center()
                    .gap(t().d(9.))
                    .px(t().d(12.))
                    .bg(rgb(t().p.list_bg))
                    .child(div().flex_none().truncate().child(title))
                    .children((!narrow).then(|| {
                        div()
                            .id(ElementId::Name("chat-brief".into()))
                            .flex_none()
                            .max_w(t().d(300.))
                            .truncate()
                            .px(t().d(6.))
                            .py(t().d(2.))
                            .rounded(t().r(5.))
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .hover(|s| {
                                s.bg(rgb(t().p.surface)).text_color(rgb(t().p.text))
                            })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.open_session_sheet(id, cx);
                            }))
                            // 什么都没配也要有个入口：不然「加个模板」找不到地方。
                            .child(if brief.is_empty() {
                                "配置 ›".to_string()
                            } else {
                                brief
                            })
                    }))
                    .children((!narrow).then(|| {
                        div()
                            .flex_none()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(if self.nav.drawer_open {
                                "目录指向抽屉里的全量历史".to_string()
                            } else if short {
                                format!("主视图最近 {} 条", self.entries.len() - from)
                            } else {
                                format!("已展开 {} 条", self.entries.len() - from)
                            })
                    }))
                    .child(div().flex_1())
                    // 分支：分叉树唯一的入口。激活时变绿。
                    .child(self.branch_button(id, cx))
                    .children((!short).then(|| crate::nav_ui::collapse_button(cx)))
                    .child(crate::nav_ui::history_button(narrow, cx)),
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
                    .min_w(t().d(0.))
                    // 竖版的 `min_w(0)`：不加它这个「视口」会被内容撑到十几万像素高。
                    .min_h(t().d(0.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .id("chat-scroll")
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .on_scroll(
                                cx.listener(|this, _e: &ScrollEvent, _, cx| this.nav_spy(cx)),
                            )
                            .py(t().d(8.))
                            .children(main_rows),
                    )
                    .child(scrollbar::overlay(
                        "chat-scrollbar",
                        &self.scroll,
                        self.bar(Bar::Chat).0,
                        self.bar(Bar::Chat).1,
                    ))
                    .child(crate::nav_ui::drawer(
                        self.nav.drawer_open,
                        drawer_rows,
                        &self.nav.drawer_scroll,
                        self.bar(Bar::Drawer),
                        cx,
                    )),
            )
            // 这一轮的失败就地显示：红条 + 重试 + 收起。不占左栏那条「连不上引擎」的通道。
            // 按钮左锚、文字放最后（GPUI 里长文字会把它右边的东西顶出屏幕）。
            .when_some(self.turn_error.clone(), |el, message| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t().d(8.))
                        .w_full()
                        .px(t().d(12.))
                        .py(t().d(8.))
                        .bg(rgb(t().p.danger_soft))
                        .border_t(t().d(1.))
                        .border_color(rgb(t().p.danger_soft))
                        .child(
                            div()
                                .id("turn-error-retry")
                                .flex_shrink_0()
                                .px(t().d(10.))
                                .py(t().d(4.))
                                .rounded(t().r(4.))
                                .bg(rgb(t().p.danger_fg))
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.on_accent))
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
                                .px(t().d(6.))
                                .py(t().d(4.))
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.danger_fg))
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
                                .min_w(t().d(0.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.danger_fg))
                                .child(short_text(&message, 90)),
                        ),
                )
            })
            .child(crate::composer::render(
                &self.input,
                crate::composer::State {
                    streaming: self.streaming,
                    // 面板在场时发送键是灰的：这时回车归面板，点它也不该发出 `/deep`。
                    can_send: !self.slash_visible(cx)
                        && !self.input.read(cx).value().trim().is_empty(),
                },
                self.composer_hint,
                self.composer_hover,
                cx.listener(|this, hovered: &bool, _, cx| this.composer_hover(*hovered, cx)),
                &self.composer_scroll,
                self.bar(Bar::Composer),
                cx.listener(|this, hovered: &bool, _, cx| {
                    this.bar_hover(Bar::Composer, *hovered, cx)
                }),
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.send(window, cx);
                }),
                cx.listener(|this, _: &ClickEvent, _window, cx| this.stop_generation(cx)),
                self.slash_panel(cx),
                self.cfg_first_tool(cx),
            ))
            // 「这个会话带什么」那张弹层：跟盘面同一个 `board_sheet`，只是目标不同。
            .children(self.board_sheet(&m, cx))
            .into_any_element()
    }

    /// 把一段 entries 画成气泡列表（含正在生成的工具和半截回答）。
    /// 主视图和抽屉共用这一套，避免两处渲染长得不一样。
    fn nav_rows(&self, slice: &[Entry], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out: Vec<AnyElement> = slice
            .iter()
            // 空正文的气泡不画 —— 引擎对「有 entry 没内容」会给空串/null，
            // 画出来就是一个白方块（坑.md 第 7 条）。
            .filter(|e| entry_visible(e))
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
                        .px(t().d(12.))
                        .py(t().d(4.))
                        .child(
                            div()
                                .max_w(t().d(480.))
                                .px(t().d(12.))
                                .py(t().d(8.))
                                .rounded(t().r(6.))
                                .bg(rgb(t().p.bubble_other))
                                .child(live.clone()),
                        )
                        .into_any_element(),
                );
            }
        }
        out
    }

    // ── 会话内跳转：目录 + 历史抽屉（J + 9） ────────────────────────────

    /// 目录按天分组。水位取引擎给的 `readAt`，没给就不标红点。
    fn nav_groups(&self) -> Vec<crate::session_nav::NavDay> {
        let (read_at, has_unread) = self
            .selected
            .and_then(|id| self.unread.of(id))
            .map(|s| (s.read_at, s.unread > 0))
            .unwrap_or((None, false));
        self.nav.groups(&self.entries, read_at, has_unread)
    }

    /// 滚动容器里，第 `entry_index` 条历史是第几个孩子。
    ///
    /// **不能拿 entry 下标直接当孩子下标**：空正文的气泡被跳过（坑.md 7），
    /// 主视图前面还挂着「更早 N 条」和一行说明，抽屉前面挂着一条提示。
    /// 不对齐的话跳转会偏到别的轮次 —— 这是「目录点一下跳歪」的根因。
    fn nav_child_of(&self, entry_index: usize, drawer: bool) -> Option<usize> {
        if entry_index >= self.entries.len() || !entry_visible(&self.entries[entry_index]) {
            return None;
        }
        nav_child_index(
            &self.entries,
            entry_index,
            if drawer {
                0
            } else {
                self.nav.from_of(self.entries.len())
            },
            drawer,
            self.entries_loading || self.entries_error.is_some(),
        )
    }

    /// 当前高亮该跟着谁：抽屉开着就跟抽屉，否则跟主视图。
    /// **只允许一个「当前正文」** —— 两套滚动状态各跟各的是这里最容易出的错。
    pub(crate) fn nav_spy(&mut self, cx: &mut Context<Self>) {
        let groups = self.nav_groups();
        if groups.is_empty() {
            return;
        }
        let drawer = self.nav.drawer_open;
        let from = if drawer {
            0
        } else {
            self.nav.from_of(self.entries.len())
        };
        let (offset, bounds, max_offset) = if drawer {
            (
                self.nav.drawer_scroll.offset(),
                self.nav.drawer_scroll.bounds(),
                self.nav.drawer_scroll.max_offset(),
            )
        } else {
            (
                self.scroll.offset(),
                self.scroll.bounds(),
                self.scroll.max_offset(),
            )
        };
        // 顶部 80px 那条线：最后一个越过它的用户消息就是「正在读的那轮」。
        // 贴底例外 —— 那时在看最新一轮，而它未必过了那条线。
        let viewport_h = bounds.size.height.as_f32();
        let at_bottom = -offset.y.as_f32() + viewport_h >= -max_offset.y.as_f32() - 4.0;
        let line = -offset.y.as_f32() + 80.0;
        let mut current = groups[0].rounds[0].index;
        for (i, e) in self.entries.iter().enumerate().skip(from) {
            if e.kind != "pi.user" {
                continue;
            }
            let b = self.nav_child_of(i, drawer).and_then(|ix| {
                if drawer {
                    self.nav.drawer_scroll.bounds_for_item(ix)
                } else {
                    self.scroll.bounds_for_item(ix)
                }
            });
            let Some(b) = b else { continue };
            if b.top().as_f32() > line {
                break;
            }
            if let Some(r) = groups
                .iter()
                .flat_map(|g| g.rounds.iter())
                .find(|r| r.entry_index == i)
            {
                current = r.index;
            }
        }
        if at_bottom && let Some(r) = groups.iter().flat_map(|g| g.rounds.iter()).last() {
            current = r.index;
        }
        let group = groups
            .iter()
            .find(|g| g.rounds.iter().any(|r| r.index == current))
            .map(|g| g.label.clone());
        let changed = self.nav.current != current;
        self.nav.current = current;
        // 自动展开当前组：H 单做时「正文在动、目录里找不到我」的那个坑。
        if let Some(g) = group {
            self.nav.reveal(&g);
        }
        if changed {
            // 目录自己别滚离视线：当前那组跟着进视口。
            if let Some(gi) = groups
                .iter()
                .position(|g| g.rounds.iter().any(|r| r.index == current))
            {
                self.nav.nav_scroll.scroll_to_top_of_item(gi);
            }
            cx.notify();
        }
    }

    /// 目录点一行。三个落点：抽屉里定位 / 主视图直接跳 / 打开抽屉定位。
    /// 用户拍板的是第三条 —— **主视图永远 12 行**，不搞「先加载再跳」。
    pub(crate) fn nav_jump(&mut self, index: usize, _group: String, cx: &mut Context<Self>) {
        let groups = self.nav_groups();
        let Some(r) = groups
            .iter()
            .flat_map(|g| g.rounds.iter())
            .find(|r| r.index == index)
            .cloned()
        else {
            return;
        };
        self.nav.current = index;
        if self.nav.drawer_open {
            // 抽屉里是全量，但孩子下标要跳过多余行（见 nav_child_of）。
            if let Some(ix) = self.nav_child_of(r.entry_index, true) {
                self.nav.drawer_scroll.scroll_to_top_of_item(ix);
            }
            cx.notify();
            return;
        }
        let from = self.nav.from_of(self.entries.len());
        if r.entry_index >= from {
            if let Some(ix) = self.nav_child_of(r.entry_index, false) {
                self.scroll.scroll_to_top_of_item(ix);
            }
            cx.notify();
            return;
        }
        self.nav_open_drawer(Some(r.entry_index), cx);
    }

    /// 打开抽屉。`to` 是「滚到哪条 entry」，None = 接着上次看到哪儿
    /// （最近 12 行主视图里已经有了，打开抽屉是为了看旧的）。
    pub(crate) fn nav_open_drawer(&mut self, to: Option<usize>, cx: &mut Context<Self>) {
        self.nav.drawer_open = true;
        // 抽屉卸下再挂回来时句柄里的 item 尺寸全过期，所以每次都给一个新的；
        // 想「接着看」就把上次关掉时的偏移灌进去，别让用户重新滚一百条。
        self.nav.drawer_scroll = ScrollHandle::new();
        match to {
            Some(entry) => {
                let ix = self.nav_child_of(entry, true);
                if let Some(ix) = ix {
                    self.nav.drawer_scroll.scroll_to_top_of_item(ix);
                }
                self.nav.current = self
                    .nav_groups()
                    .iter()
                    .flat_map(|g| g.rounds.iter())
                    .filter(|r| r.entry_index <= entry)
                    .last()
                    .map(|r| r.index)
                    .unwrap_or(0);
            }
            None => {
                let y = self.nav.drawer_offset_y;
                if y != 0. {
                    self.nav.drawer_scroll.set_offset(point(px(0.), px(y)));
                }
            }
        }
        // 抽屉开着时目录指向全量正文，第一组先摊开。
        let groups = self.nav_groups();
        if let Some(g) = groups.first() {
            self.nav.reveal(&g.label);
        }
        cx.notify();
    }

    /// 收起抽屉。**三条出口共用这一条**：点遮罩、Esc、再点一次「历史与目录」。
    /// 关掉时记下滚到哪儿 —— 不然下次打开又从头开始。
    pub(crate) fn nav_close_drawer(&mut self, cx: &mut Context<Self>) {
        self.nav.drawer_offset_y = self.nav.drawer_scroll.offset().y.as_f32();
        self.nav.drawer_open = false;
        cx.notify();
    }

    /// 顶栏那颗按钮：开 / 关（和输入区左下角那颗 DNA 的 `toggle_config` 同一套）。
    /// 之前它只负责开，抽屉开着时再点一次等于「重新开一遍、还滚回顶部」，最反直觉的那种。
    pub(crate) fn nav_toggle_drawer(&mut self, cx: &mut Context<Self>) {
        if self.nav.drawer_open {
            self.nav_close_drawer(cx);
        } else {
            self.nav_open_drawer(None, cx);
        }
    }

    /// 抽屉头上的桥：把「目录当前高亮的那轮」加载进主视图，然后关抽屉。
    pub(crate) fn nav_to_main(&mut self, cx: &mut Context<Self>) {
        let groups = self.nav_groups();
        let entry_index = groups
            .iter()
            .flat_map(|g| g.rounds.iter())
            .find(|r| r.index == self.nav.current)
            .map(|r| r.entry_index);
        if let Some(ix) = entry_index {
            self.nav.main_from = Some(ix.saturating_sub(1));
        }
        self.nav.drawer_open = false;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    pub(crate) fn nav_collapse(&mut self, cx: &mut Context<Self>) {
        self.nav.main_from = None;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    pub(crate) fn nav_toggle_fold(&mut self, cx: &mut Context<Self>) {
        let groups = self.nav_groups();
        let keys: Vec<String> = groups
            .iter()
            .take(groups.len().saturating_sub(1))
            .map(|g| g.label.clone())
            .collect();
        let any_open = keys.iter().any(|k| !self.nav.closed.contains(k));
        for k in keys {
            if any_open {
                self.nav.closed.insert(k);
            } else {
                self.nav.closed.remove(&k);
            }
        }
        cx.notify();
    }

    pub(crate) fn nav_toggle_group(&mut self, label: String, cx: &mut Context<Self>) {
        if !self.nav.closed.remove(&label) {
            self.nav.closed.insert(label);
        }
        cx.notify();
    }

    // ── 列表 / 气泡 ─────────────────────────────────────────

    /// 只在「连引擎中 / 连不上」时占一行。
    ///
    /// 会话数量原来也在这儿（`N 个会话`），现在搬进搜索框右端（`render_search_box` 的
    /// `suffix`）—— 两行并成一行，列表多出 20px。
    fn status_line(&self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.loading {
            return Some(
                div()
                    .text_size(t().fs(12.))
                    .child("连引擎中…")
                    .into_any_element(),
            );
        }
        let e = self.error.as_ref()?;
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
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(4.))
                    .text_size(t().fs(12.))
                    .text_color(rgb(t().p.danger))
                    .child(format!("连不上：{e}"))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(hint),
                    )
                    .into_any_element(),
            )
    }

    /// 空会话那一行上摆什么。
    ///
    /// 为什么不是「有预览就用预览、没有就空着」：149 条主线里 **142 条的 `last` 真的是空的**
    /// （`/v1/sessions/:id/entries` 返回 `{"items":[]}`，不是列表漏了字段），空着等于
    /// 149 行只有名字，而其中 36 条还重名。摆上元信息至少能区分「谁开的、在哪个目录、什么模型」。
    ///
    /// 顺序按「能不能区分」排，`#id` 放最前：它是唯一一个不重复的字段。
    fn meta_line(s: &Session) -> String {
        let mut bits = vec![format!("#{}", s.id)];
        if let Some(tpl) = s.template.as_deref() {
            if !tpl.trim().is_empty() {
                bits.push(tpl.to_string());
            }
        }
        if let Some(m) = short_model(s.model.as_deref()) {
            bits.push(m);
        }
        if let Some(c) = short_cwd(s.cwd.as_deref()) {
            bits.push(c);
        }
        bits.join(" · ")
    }

    fn session_row(&self, s: &Session, q: &str, cx: &mut Context<Self>) -> AnyElement {
        // 预览是最后一条 entry 的原文，带换行/连续空白是常态（工具输出整段）。
        // 摊平成一行，配合 `.truncate()` 才是微信那种单行摘要。
        let preview = s
            .last
            .as_ref()
            .and_then(|l| l.text.as_deref())
            // markdown 记号在这一层摘掉：预览是给人扫的，不是给渲染器看的（`preview.rs`）。
            .map(|t| crate::preview::plain(t))
            .unwrap_or_default();
        let selected = self.selected == Some(s.id);
        // `busy` 是 `"queued" | "running" | <task kind> | null`，不是布尔（`wire.md`）。
        let busy = s.busy.is_some();
        let id = s.id;
        let pinned = self.pins.contains(&id);
        // 置顶和归档都在 qrd3wh，但门不能串在一起：
        // 置顶服务没连上时菜单照样要挂上，只是那一项画成禁用态（见 `archive_menu`）。
        let pins_available = self.pins_client.is_some();

        // 左竖条常驻、平时是透明色：hover 时只换颜色，不换边框宽度 ——
        // 换成「hover 才加 2px 边框」的话，鼠标划过的瞬间整行文字会右移 2px。
        // 下边线放在外层（边框只有一种颜色，跟竖条共用会被 hover 一起染上）。
        //
        // `min_h(56)`：没有第二行内容的行（只有工具输出的）也跟别人一样高，列表才成一列节奏。
        let row = div()
            .flex()
            .flex_col()
            .justify_center()
            .min_h(t().d(56.))
            .pl(t().d(8.))
            .pr(t().d(10.))
            .py(t().d(6.))
            .gap(t().d(2.))
            .border_l(t().d(2.))
            .border_color(rgba(t().p.transparent))
            .when(selected, |el| el.bg(rgb(t().p.selected)))
            // 选中态优先：鼠标划在最上面那条时不把它的底色盖掉（`hover` 是后画的，
            // 不加这个条件会盖）。
            .when(!selected, |el| {
                el.hover(|s| {
                    s.bg(rgb(t().p.row_hover))
                        .border_color(rgb(t().p.accent))
                })
            })
            // 一行里三样东西：📌 / 名字 / 分支角标 / 时间。左半边 `flex_1` 占满并 truncate，
            // 右半边 `flex_none` 顶到右边 —— 谁都不会被谁顶出去（比 absolute 好使，也不会重叠）。
            .child(
                div()
                    .flex()
                    .w_full()
                    .items_center()
                    .gap(t().d(6.))
                    // 📌 用 lucide 的 Pin（AllAssets 里有）
                    .when(pinned, |el| {
                        el.child(
                            Icon::new(IconName::Pin)
                                .text_color(rgb(t().p.ok_fg))
                                .size(t().d(13.)),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(Self::hl_line(&s.name, q, None, None)),
                    )
                    // 搜的时候多一个小标签：这一条是名字命中的还是内容命中的。
                    // 不搜的时候不占地方（`when` 整块不渲染）。
                    .when(!q.is_empty(), |el| {
                        el.children(crate::search::hit(s, q).map(|w| {
                            div()
                                .flex_none()
                                .px(t().d(3.))
                                .rounded(t().r(3.))
                                .border(t().d(1.))
                                .border_color(rgb(t().p.line_soft))
                                .text_size(t().fs(10.))
                                .text_color(rgb(t().p.faint))
                                .child(w.label())
                        }))
                    })
                    // 别用 `⑂`：雅黑里没这个字形，会退化成别的样子。
                    .when(self.descendant_count(id) > 0, |el| {
                        el.child(
                            div()
                                .flex_none()
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.accent))
                                .child(format!("{} 分支", self.descendant_count(id))),
                        )
                    })
                    .children(when_text(s.updated_at).map(|when| {
                        div()
                            .flex_none()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child(when)
                    }))
                    // 机器角标：`machine` 有值就说明这条不是本机跑的（`j1900` / `9996696`）。
                    // 元信息那行里**不再重复**它 —— 同一行写两遍等于把噪声翻倍。
                    .children(s.machine.as_deref().filter(|m| !m.trim().is_empty()).map(|m| {
                        div()
                            .flex_none()
                            .px(t().d(3.))
                            .rounded(t().r(3.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(m.to_string())
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
                    .gap(t().d(6.))
                    .child(if busy {
                        // 忙闲：微信那句「正在输入…」+ 一个小圆点。比一个「忙」字可读。
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .flex()
                            .items_center()
                            .gap(t().d(4.))
                            .child(
                                div()
                                    .flex_none()
                                    .size(t().d(6.))
                                    .rounded(t().r(3.))
                                    .bg(rgb(t().p.accent)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(12.))
                                    .text_color(rgb(t().p.accent))
                                    .child("正在输入…"),
                            )
                            .into_any_element()
                    } else if !preview.is_empty() {
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(Self::hl_line(&preview, q, Some(12.), Some(t().p.muted)))
                            .into_any_element()
                    } else {
                        // 空会话：写元信息（`#id · 模板 · 模型 · cwd`），不写「空」两个字
                        // ——「空」在 149 行里重复 142 次，跟空白一样没信息量。
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.faint))
                            .child(Self::meta_line(s))
                            .into_any_element()
                    })
                    .children(self.unread_badge(id)),
            );

        // 外层只负责两件事：下边线和点击。`.id()` 把 Div 转成 Stateful<Div>，
        // on_click 只存在于后者。
        let row = div()
            .w_full()
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_strong))
            .id(ElementId::Name(format!("sess-{id}").into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                // 点了一行，键盘也算停在列表上：接着按 ↑↓ 应该继续换会话，
                // 而不是回到搜索框重新开始。
                this.list_keys = true;
                this.choose_session(id, cx);
            }))
            .child(row);

        // 右键菜单：打开 / 置顶 / ─── / 更多 › / 重命名 / 复制名字 / ─── / 新建分支。
        // 「更多」里的归档 / 撕原件是**独立一层**的子菜单（见 `archive_menu.rs`），
        // 条目顺序、文案、动作映射都在那个模块里；这里只把产出的动作送回 `Desktop`。
        //
        // 菜单**永远挂上**：归档 / 撕原件 / 复制都走 qrd3wh，和置顶服务无关 ——
        // 早先按 `pins_client` 判了一刀，置顶一挂就整摞菜单（含「更多 › 归档」）
        // 跟着消失（评审必修 1）。置顶不可用时由 `archive_menu` 把那一条画成禁用态。
        //
        // 只能在链尾分支：`context_menu()` 返回的是 `ContextMenu<Stateful<Div>>`，
        // 换个类型就没法再串 `when`（类型对不上）。
        let this = cx.entity().downgrade();
        let target = crate::archive_menu::SessionTarget::new(id, s.name.clone());
        row.context_menu(move |menu, window, cx| {
            let this = this.clone();
            crate::archive_menu::build_menu(
                menu,
                window,
                cx,
                &target,
                pinned,
                pins_available,
                move |action, cx| {
                    let _ = this.update(cx, |this, cx| this.on_session_menu(action, cx));
                },
            )
        })
        .into_any_element()
    }

    /// 会话行右键菜单的出口（票 #5 / #6）。
    ///
    /// 每一项点下去都必须有可见结果（AGENTS：「按钮点下去没反应…都算 bug」）：
    /// 打开换会话、置顶写进内核、归档 / 撕原件走引擎、复制写剪贴板后弹确认。
    /// 重命名是唯一还没端点的项（`wire.md` 里没有改名路由），点它给一句说明，
    /// 不偷偷发请求、也不静默吞掉。
    pub(crate) fn on_session_menu(
        &mut self,
        action: crate::archive_menu::SessionMenuAction,
        cx: &mut Context<Self>,
    ) {
        use crate::archive_menu::SessionMenuAction as A;
        match action {
            A::Open { id } => self.choose_session(id, cx),
            A::TogglePin { id } => self.toggle_pin(id, cx),
            A::CopyName { name } => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(name.clone()));
                log::info!("复制会话名字：{name}");
                // 写剪贴板本身没有可见结果（Wayland 上还可能被合成器丢掉），
                // 所以必须给一句确认 —— 否则就是「点下去像没反应」。
                self.notify("已复制会话名字", NoticeKind::Info, cx);
            }
            A::CopyId { id } => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(id.to_string()));
                log::info!("复制会话 id：{id}");
                self.notify("已复制会话 id", NoticeKind::Info, cx);
            }
            A::Archive { id, name } => self.archive_session(id, &name, cx),
            A::ArchiveAndPurge { id, name } => {
                // 右键一步到位：先弹红框，确认后先归档、成功再撕（true）。
                self.purge_confirm.request(id, name, true);
                cx.notify();
            }
            A::Rename { id } => self.open_rename(id, cx),
            A::Fork { id } => self.fork_session(id, cx),
        }
    }

    /// `新建分支`：省掉 `entryId` / `name` 直接分叉 —— 引擎按「从最后一条」处理，
    /// 名字给默认（见 `wire.md` 的 `POST /v1/sessions/:id/fork`）。
    ///
    /// 成功后重拉列表（新分支要出现在分支视图里）+ 选中新会话 + 切盘上那只眼，
    /// 跟「新建盘面建完就进去」一套。失败弹引擎 `{error}` 原文。
    fn fork_session(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        log::info!("新建分支：从 #{id} 最后一条分叉");
        self.run_engine(
            cx,
            &format!("新建分支没成：{id}"),
            &format!("新建分支任务挂了：{id}"),
            async move {
                let forked = engine.fork(id, None, None).await?;
                let _ = engine.set_selection(Some(forked.id)).await;
                Ok(forked)
            },
            move |this, forked, cx| {
                this.notify(
                    format!("已新建分支 · 从 #{id} 最后一条分叉"),
                    NoticeKind::Info,
                    cx,
                );
                // 新会话在列表里是 `parent != null` 的分支：重拉列表它才出现；
                // 然后直接进去（force=true，万一点之前正好选着它也要重新载入）。
                this.reload_sessions(None, cx);
                this.choose_session(forked.id, cx);
                // 分支视图**开着**时直接把根换到父会话；关着时记一笔一次性提示，
                // 等用户点「⑂ 分支」再用 —— 那时 `self.selected` 已经是新叉，
                // 拿它当根只会显示「还没有分支」（新叉自己没子）。
                this.fork_root_hint = Some(id);
                if this.branches {
                    this.branch_of = Some(id);
                    cx.notify();
                }
            },
            Self::notify_engine_failure,
        );
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
                .min_w(t().d(18.))
                .h(t().d(18.))
                .px(t().d(5.))
                .rounded(t().r(9.))
                .bg(rgb(t().p.badge))
                .flex()
                .items_center()
                .justify_center()
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.on_accent))
                .child(if n > 99 {
                    "99+".to_string()
                } else {
                    n.to_string()
                })
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
            .gap(t().d(5.))
            .px(t().d(10.))
            .h(t().d(28.))
            .rounded(t().r(6.))
            .border_1()
            .border_color(rgb(if on { t().p.accent } else { t().p.line_strong }))
            .bg(rgb(if on { t().p.accent } else { t().p.panel }))
            .text_size(t().fs(13.))
            .text_color(rgb(if on { t().p.on_accent } else { t().p.text_2 }))
            .id(ElementId::Name("branch-toggle".into()))
            .cursor_pointer()
            .hover(|el| el.bg(rgb(if on { t().p.accent_hover } else { t().p.hover })))
            .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                if this.branches {
                    this.exit_branches(cx);
                } else {
                    // 刚 fork 过就优先锁到父会话；一次性，用完清掉。
                    let root = this
                        .fork_root_hint
                        .take()
                        .unwrap_or_else(|| this.selected.unwrap_or(id));
                    this.enter_branches(root, cx);
                }
            }))
            .child(
                Icon::new(IconName::GitBranch)
                    .with_size(Size::Size(t().d(15.)))
                    .text_color(rgb(if on { t().p.on_accent } else { t().p.text_2 })),
            )
            .child("分支")
            .children(if n > 0 {
                Some(
                    div()
                        .flex_none()
                        .px(t().d(5.))
                        .rounded(t().r(8.))
                        .bg(rgb(if on {
                            t().p.accent_hover
                        } else {
                            t().p.chat_bg
                        }))
                        .text_size(t().fs(11.))
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
        let title = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| "工具输出".to_string());
        let k = key.to_string();
        let for_copy = text.clone();

        // 外层是列（卡片头一行、正文在下面），但卡片头自己包一层 `flex_row`：
        // 直接在列里放「一行东西」，它会被拉成整行宽，甲案那股「一小条」的味道就没了
        // （跟坑.md 19 同一个病）。
        div()
            .flex()
            .flex_col()
            .items_start()
            .px(t().d(12.))
            .py(t().d(4.))
            .child(
                div().flex().flex_row().child(
                    div()
                        .id(ElementId::Name(format!("tool-{key}").into()))
                        .flex()
                        .items_center()
                        .gap(t().d(7.))
                        // 不收宽度上限：长标题会被外层挤成一条缝。`max_w` 是给整条卡片的。
                        .max_w(t().d(520.))
                        .px(t().d(10.))
                        .py(t().d(5.))
                        .rounded(t().r(5.))
                        .border_1()
                        .border_color(rgb(t().p.line_soft))
                        .bg(rgb(t().p.panel))
                        .cursor_pointer()
                        .hover(|el| el.bg(rgb(t().p.panel_alt)))
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
                                .text_size(t().fs(10.))
                                .text_color(rgb(t().p.muted))
                                .child(if open { "▾" } else { "▸" }),
                        )
                        .child(
                            div()
                                .flex_none()
                                .size(t().d(6.))
                                .rounded(t().r(3.))
                                .bg(rgb(if failed { t().p.badge } else { t().p.accent })),
                        )
                        .child(div().flex_none().text_size(t().fs(12.5)).child(title))
                        .child(
                            div()
                                .flex_none()
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.muted))
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
                                .px(t().d(6.))
                                .py(t().d(1.))
                                .rounded(t().r(4.))
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.accent))
                                .cursor_pointer()
                                .hover(|el| el.bg(rgb(t().p.accent_soft)))
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
                        .mt(t().d(2.))
                        // 宽高都用**固定值**，不用 `max_*`：`max_*` 在「内容比它大」时扛不住，
                        // 一行 487 行的代码能直接铺满整个聊天区（gpui 里 `min-width:auto`
                        // 那一套，见坑.md 16）。
                        .w(t().d(560.))
                        .h(t().d(190.))
                        .overflow_y_scroll()
                        // 不折行：等宽代码一折行缩进就全乱了，宁可让它在块里横向滚
                        .whitespace_nowrap()
                        .px(t().d(10.))
                        .py(t().d(8.))
                        .border_l_2()
                        .border_color(rgb(t().p.line_strong))
                        .bg(rgb(t().p.panel_alt))
                        .font_family("Consolas")
                        .text_size(t().fs(11.5))
                        .text_color(rgb(t().p.text_2))
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
                .px(t().d(12.))
                .py(t().d(4.))
                .child(
                    div()
                        .max_w(t().d(480.))
                        .min_w(t().d(0.))
                        .px(t().d(12.))
                        .py(t().d(8.))
                        .rounded(t().r(6.))
                        .bg(rgb(t().p.bubble_me))
                        .child(selectable(&format!("u-{}", e.id), &text, t().p.text)),
                )
                .into_any_element(),
            "pi.assistant" => div()
                .flex()
                .justify_start()
                .px(t().d(12.))
                .py(t().d(4.))
                .child(
                    div()
                        .max_w(t().d(480.))
                        .min_w(t().d(0.))
                        .px(t().d(12.))
                        .py(t().d(8.))
                        .rounded(t().r(6.))
                        .bg(rgb(t().p.bubble_other))
                        .child(selectable(&format!("a-{}", e.id), &text, t().p.text)),
                )
                .into_any_element(),
            "pi.tool-result" => self.tool_card(&e.id.to_string(), None, &text, cx),
            _ => div()
                .flex()
                .justify_center()
                .px(t().d(12.))
                .py(t().d(4.))
                .child(div().text_size(t().fs(11.)).text_color(rgb(t().p.muted)).child(text))
                .into_any_element(),
        }
    }
}

/// 比较两次未读表：有几个会话是**新**多出未读的。
///
/// `viewing` 是当前正在流的会话 —— 它不算（那条路自己会响，见 `attend`）。
///
/// 抽成纯函数是因为这是这一层唯一容易写错的地方：少算一个就不响了，
/// 多算一个就响两声，两种都只有真机等一轮跑完才会发现。
fn bumped_sessions(prev: &Unread, next: &Unread, viewing: Option<i64>) -> usize {
    let viewing = viewing.map(|id| id.to_string());
    next.0
        .iter()
        .filter(|(id, st)| {
            if viewing.as_deref() == Some(id.as_str()) {
                return false;
            }
            // 表里没有的会话按 0 算 —— 引擎说"覆盖所有会话"，但别在上面押命。
            st.unread > prev.0.get(*id).map(|s| s.unread).unwrap_or(0)
        })
        .count()
}

/// 顶栏那行「这个会话带什么」：`deepseek-v4-flash · smart-ass · 5 技能 · 只读`。
///
/// 空的部分**不占位** —— 四个 `·` 排成一排比什么都没有还难看。`skills` 只在有选中时
/// 出现（`0 技能` 是噪声，没有就是没有）。
fn brief_of(s: &Session) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(m) = short_model(s.model.as_deref()) {
        parts.push(m);
    }
    if let Some(tpl) = s.template.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
        parts.push(tpl.to_string());
    }
    if !s.skills.is_empty() {
        parts.push(format!("{} 技能", s.skills.len()));
    }
    if s.read_only {
        parts.push("只读".into());
    }
    parts.join(" · ")
}

/// 左栏顺序：**置顶的在前**（顺序照 `pins` 给的），其余保持原来那套顺序。
///
/// 纯函数（只吃 id），所以能直接拿数字做单测 —— 排序这种事写错一次能烦一星期。
/// `"volc-ark/deepseek-v4-flash-260425"` → `"deepseek-v4-flash"`。
///
/// 摘掉两样：provider 前缀（最后一截才是模型名），和尾部 `-260425` 这种快照日期
/// （引擎把日期直接挂在名字上，留着会让 `deepseek-v4-flash-260425` 这种长串把整行吃满）。
/// `"undefined/undefined"` 是引擎真给过的值，当作没有 —— 画出来是纯噪声。
fn short_model(model: Option<&str>) -> Option<String> {
    let tail = model?
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty() && *p != "undefined")
        .next_back()?;
    if let Some((head, digits)) = tail.rsplit_once('-') {
        if digits.len() == 6 && digits.chars().all(|c| c.is_ascii_digit()) && !head.is_empty() {
            return Some(head.to_string());
        }
    }
    Some(tail.to_string())
}

/// `/opt/Nous/content/new1/map/qrd3wh` → `…/map/qrd3wh`；`/root` → `/root`。
///
/// 会话列表里 90% 的 `cwd` 是同一个目录，只要末两段就够区分，前面全部省略成 `…`。
fn short_cwd(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd?.trim();
    if cwd.is_empty() {
        return None;
    }
    let parts: Vec<&str> = cwd.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return Some(cwd.to_string());
    }
    Some(format!("…/{}", parts[parts.len() - 2..].join("/")))
}

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

/// 错误原文可能很长（引擎把上游 400 的整段 JSON 递上来）。红条只给人看前 90 个字，
/// 全文在 `%LOCALAPPDATA%` 下的 `nousdesk/desktop.log` 里。
fn short_text(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if out.chars().count() < text.chars().count() {
        out.push('…');
    }
    out
}

/// 连引擎：拿配置 → 直连这一个地址 → 拉会话列表 + 设备信息 + 能力表。全在 tokio 上。
///
/// 配置**从调用方传进来**，不在这里再读一次盘：设置页刚保存完就重连，
/// 重新读盘会让「保存了什么」和「连了什么」有可能对不上。
async fn load_engine(
    cfg: Config,
    recent: Recent,
) -> Result<
    (
        Arc<EngineClient>,
        Vec<Session>,
        Option<nous_engine::types::Device>,
        String,
        Option<nous_engine::Capabilities>,
    ),
    String,
> {
    let token = cfg.resolve_token();
    let engine = EngineClient::with_token(&cfg.host, Duration::from_secs(10), token.as_deref())
        .map_err(|e| e.to_string())?;
    let sessions = engine.sessions().await.map_err(|e| e.to_string())?.items;
    let mut sessions = sessions;
    sort_sessions(&mut sessions, &recent);
    // 设备信息只用于「关于」展示 + DEMO 角标，拿不到不妨碍连接。
    let device = engine.device().await.ok();
    // 能力表拿不到就当没有：所有功能面按 Unknown 灰掉，不猜。
    let caps = engine.capabilities().await.ok();
    Ok((
        Arc::new(engine),
        sessions,
        device,
        cfg.host.clone(),
        caps,
    ))
}

/// 归档掉 `gone` 之后，右栏该退到哪一条。
///
/// `sessions` 是**已经排好序**的列表（`sort_sessions` + 置顶那一套，跟左栏看到的一致），
/// 取第一条不是它的 = 「最近一条能说话的」。`busy` 的不排除：在跑的会话也能看。
/// 取不到就 `None`，调用方据此回空态（清 entries + 停 SSE）。
pub(crate) fn next_selection_after_archive(sessions: &[Session], gone: i64) -> Option<i64> {
    sessions.iter().find(|s| s.id != gone).map(|s| s.id)
}

/// 引擎错误的提示语：[`EngineError::Engine`] 的 `message` **原文**逐字给用户
/// （409 会列出活分支，那些字有用），不加前缀、不翻译、不截断。
/// 网络 / URL 层没有引擎的 `message`，用 `Display` —— 那种错误没有「原文」可言。
pub(crate) fn error_notice(err: &EngineError) -> String {
    match err {
        EngineError::Engine { message, .. } => message.clone(),
        other => other.to_string(),
    }
}

/// 会话排序。**聊过的一律在上面，只有壳子的在下面** —— 两堆分开排。
///
/// 以前是一个比较器把两套刻度混着比：`last.id`（全局单调的 entry 序号，真聊过的会话是
/// 几百）跟会话 id（空会话是 1500+）。结果 **15 条真聊过的会话全部掉到 111 名以后**，
/// 上面 110 行是 134 条空壳（2026-11-16 量出来的，见 `验证.md`）。这不是调参能救的，
/// 是两个序列根本不可比。
///
/// 现在是两层：
///
/// 1. **有 `last` 的**：按 `last.id` 倒序 = 最后说话的在最上面。`EntryId` 全局单调
///    （durable 的 `conversation-tails` 就靠这条），实测也成立：session 465 → 483、
///    session 2 → 411。所以这一层**不需要时间**。
/// 2. **没有 `last` 的**：先看本机「最近打开」（`recent.rs`，只在这台机器上），
///    再退化成会话 id 倒序（= 建的先后）。引擎对这 134 条什么时间都不给，
///    本地那份是唯一能让「我刚点开的那条」浮上来的东西。
///
/// 置顶再叠在这一切之上（`order_ids`），不在这个比较器里。
fn sort_sessions(sessions: &mut [Session], recent: &Recent) {
    sessions.sort_by_key(|s| std::cmp::Reverse(sort_key(s, recent)));
}

/// 排序键。三段的含义见 `sort_sessions`：`(层, 层内小档, 数值)`，整套降序。
///
/// 第一段的取值：`2` = 有对话，`1` = 空会话。
fn sort_key(s: &Session, recent: &Recent) -> (u8, u8, u64) {
    if let Some(entry) = s.last.as_ref().and_then(|l| l.id.as_i64()) {
        return (2, 0, entry.max(0) as u64);
    }
    // 引擎偶尔给 `updatedAt`（毫秒）：它比「本机什么时候点开」更硬，先认它。
    // 秒级排序就够了，所以统一折成秒。
    if let Some(ms) = s.updated_at {
        return (1, 2, (ms.max(0) as u64) / 1000);
    }
    if let Some(ts) = recent.ts(s.id) {
        return (1, 1, ts);
    }
    (1, 0, s.id.max(0) as u64)
}

/// 一行的「长相」。轮询时只比这个，不变就不 `cx.notify()`。
/// 活跃度：最后一条 entry 的 id。没有 entry 的新会话退化成会话 id。
///
/// **只给「有没有新动静」用**（轮询比对、分支视图），**不要**拿它排左栏 ——
/// 左栏的排序要分层，见 `sort_sessions`。
pub(crate) fn activity_of(s: &Session) -> i64 {
    s.last.as_ref().and_then(|l| l.id.as_i64()).unwrap_or(s.id)
}

/// 一段**可选中**的文本。
///
/// GPUI 里 `div().child("…")` 画出来的是死的 StyledText，鼠标拖不动 —— 整个界面
/// 一个字都复制不出来就是这个原因。`TextView` 才有选区（拖选、双击选词、Ctrl+C）。
///
/// 真渲染 markdown：标题、粗体、列表、表格、代码块都按语法走。
/// 代码块带高亮 + 一个「复制」（见 `code_highlighter` / `code_copy_button`）。
fn selectable(id: &str, text: &str, color: u32) -> TextView {
    TextView::markdown(ElementId::Name(format!("t-{id}").into()), chat_md(text))
        .selectable(true)
        .text_size(t().fs(13.5))
        .text_color(rgb(color))
        .style(md_style())
        .code_block_actions(code_copy_button)
        // 全局同一份 Arc：框架靠这个判断「这块代码没变，别重算高亮」
        .shared_code_block_highlighter(code_highlighter())
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
    .text_size(t().fs(11.5))
    .font_family("Consolas")
    .text_color(rgb(t().p.text_2))
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
    format!(
        "{fence}
{text}
{fence}"
    )
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

/// 窗口几何变了就落盘（`Desktop` 的方法体搬出来当自由函数：`impl Desktop` 的
/// 块早在上面就结束了，为这一个方法再开一个 `impl` 不划算）。节流 400ms：
/// 拖拽 / 缩放时每帧都在变，没必要每帧写。
///
/// 节流吃掉的那一下（调完窗口立刻关）由 `on_app_quit` 兜底；
/// 真异常退出（kill -9）丢的也只是最后一次调整，不值得为它加事务。
fn save_window_geometry(desk: &Desktop, window: &Window) {
    const THROTTLE: std::time::Duration = std::time::Duration::from_millis(400);
    let state = crate::window_state::WindowState::from_bounds(window.window_bounds());
    if desk.win_geom.get() == Some(state) {
        return;
    }
    let now = std::time::Instant::now();
    if let Some(at) = desk.win_geom_at.get()
        && now.duration_since(at) < THROTTLE
    {
        return;
    }
    desk.win_geom.set(Some(state));
    desk.win_geom_at.set(Some(now));
    if let Err(e) = state.save() {
        // 写不进去不影响用，只记一行 —— 别弹提示打扰人。
        log::warn!("窗口几何没存下：{e}");
    }
}

impl Render for Desktop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 每帧把当前皮拍进帧内快照 —— 下面 50 多个自由函数（pill / nav_row /
        // theme_thumb…）拿不到 cx，统一 `crate::theme::cur()` 取。
        // GPUI 的 text_style_stack 也是这个套路；快照不是第二个真相，每帧重取。
        crate::theme::begin_frame(cx);
        // 窗口几何（大小 / 位置 / 最大化）变了就记一笔，见 `window_state.rs`。
        // 放 render 开头是因为这是唯一每帧都能拿到 `&Window` 的地方。
        save_window_geometry(self, window);
        // md.rs 是样品间共用的模块（example 也 #[path] 引它），不认识 crate::theme；
        // 每帧把当前皮的 md 相关色拆成裸值灌进去。
        {
            let s = crate::theme::cur();
            crate::md::set_md_skin(
                s.p.accent, s.p.muted, s.p.accent_soft, s.p.accent_fg,
                s.p.code_bg, s.p.line, s.p.text, s.p.text_2, s.p.panel,
                s.type_scale, s.dark,
            );
        }

        // 面板的过滤快照只在 render 里算一次：键盘 ↵ 和鼠标点都认这一份，
        // 不会「看到的」和「选中的」对不上。
        self.refresh_slash(cx);

        // 窄窗：类别树收进抽屉，那一栏不再常驻（见 `TREE_FOLD_W`）。
        let narrow_tree = window.viewport_size().width.as_f32() < TREE_FOLD_W;
        // 设置 / 归档库是**整页**：连 Template 的类别树和列表一起让位。
        // 只顶替最右一格的话，1280 下它们只剩 676，640 下只剩 36 —— 输入框里
        // 连一句完整的话都看不见（坑.md 57 是同一类问题，那条只修了滚动条）。
        let page_open = self.settings_open || self.archive_open || self.editor.open;

        let main_col: AnyElement = if page_open {
            div().into_any_element()
        } else {
            match self.tab {
                Tab::Chats => self.render_session_list(cx).into_any_element(),
                // Template 页是**两栏**：类别树 + 预设列表。跟聊天页那一栏不是一个东西。
                Tab::Templates => div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .h_full()
                    .min_w(t().d(0.))
                    .children((!narrow_tree).then(|| self.render_preset_tree(cx)))
                    .child(self.render_templates(narrow_tree, cx))
                    .child(if self.new_preset.is_some() {
                        self.render_new_preset(window, cx)
                    } else {
                        self.render_detail(window, cx)
                    })
                    .into_any_element(),
                Tab::Triggers => self.render_triggers(cx).into_any_element(),
                // 资源页自带三栏（树 / 列表 / 详情），自己画完全宽。
                Tab::Resources => crate::resources::render(self, cx, narrow_tree).into_any_element(),
                // 数据监控室是三列整页（拉片 / 六屏 / 时间线），自己画完全宽。
                Tab::Data => self.render_data_console(cx).into_any_element(),
            }
        };

        // New Manual **顶替**右侧聊天区，不再盖住整窗：左导航和会话列表永远在。
        // 尺寸基准也因此从整窗 vmin 换成右格 cqmin（见 `board_art::M`）。
        // 设置页也顶替右栏，但它优先级更高：正在改连接时别把盘面盖在上面。
        let right: AnyElement = if self.settings_open {
            self.render_settings(window, cx)
        } else if self.archive_open {
            self.render_archive(cx)
        } else if self.tab != Tab::Chats {
            // Template / 触发器页自带完整栏，聊天区这一格不画 ——
            // 留着它会和页面的第三栏叠在一起（`flex_1` 撑开之后真的叠过）。
            div().into_any_element()
        } else if self.newchat.open {
            self.render_board(window, cx)
        } else {
            self.render_chat(window, cx).into_any_element()
        };

        let skin = crate::theme::cur();
        let shell_root = div()
            .flex()
            .flex_row()
            .size_full()
            // 字体族 / 行高只在皮显式指定时才注入。默认皮两项都留空 ——
            // 强行套 `.SystemUIFont` 会让整片文本加载不到字体而消失（踩过）。
            // 字号走各处的 `t().fs(..)`，默认倍率 1.0 不动观感。
            .when(!skin.font.is_empty(), |el| el.font_family(skin.font))
            .when(skin.line_height > 0., |el| el.line_height(px(skin.lh().as_f32())))
            .bg(rgb(skin.p.chat_bg))
            .child(self.render_icon_bar(cx))
            .child(main_col)
            // 会话内跳转目录（J）：只在聊天页、选中了会话、且**盘面没开**时出现。
            // New Manual 顶替右栏时那一列会把它挤窄，所以一并让开。
            .children(
                (self.tab == Tab::Chats
                    && self.selected.is_some()
                    && !self.newchat.open
                    && !self.settings_open
                    && !self.archive_open
                    // 窄窗让位：三个纵列（图标条 + 会话列表 + 目录列）一起要 550 多像素，
                    // 窗口只有一半宽时聊天区会被挤成一格（见 坑.md 58）。
                    && window.viewport_size().width.as_f32() >= NAV_COLUMN_MIN_W)
                    .then(|| {
                        let groups = self.nav_groups();
                        if groups.is_empty() {
                            return div().into_any_element();
                        }
                        let from = self.nav.from_of(self.entries.len());
                        crate::nav_ui::nav_column(
                            &self.entries,
                            &groups,
                            &self.nav,
                            from,
                            self.nav.drawer_open,
                            self.bar(Bar::Nav),
                            cx,
                        )
                    }),
            )
            // 别人的 New Manual 盘面顶替右栏；没开时才是聊天区。
            //
            // 这层容器是**窄窗的总闸**：右栏里任何一页只要内容比窗口宽，页面自己的
            // `min-content` 就会把这一栏撑出去 —— 于是滚动视口的右边缘跑到窗口外面，
            // 滚动条跟着一起消失（见 坑.md 57）。`min_w(0)` 让它肯收缩，
            // `overflow_hidden` 把溢出裁在窗口边上，滚动条就还在窗口里。
            // Template 页的「右栏」在 `main_col` 里面（第三栏），这一格是空的。
            // **空的也别占位**：它带着 `flex_1`，会把主栏挤到一半宽 —— 640 宽时
            // 模板列表 240 之后只剩 52px，右栏的图/空状态全被压成一条（见 坑.md 64）。
            .children(
                // **资源页也有自己的第三栏**，跟模板页同理：这一格必须空着不挂，
                // 否则那个空 `flex_1` 会把主栏挤成一半宽（坑.md 64；资源页就是漏了这一条）。
                (!matches!(self.tab, Tab::Templates | Tab::Resources) || page_open).then(|| {
                    div()
                        .flex_1()
                        .min_w(t().d(0.))
                        .overflow_hidden()
                        // 点右栏（聊天区 / 输入框 / 盘面）就把「键盘停在列表上」放下，
                        // 否则在聊天区按 ↓ 会偷偷把左边选中项挪走。
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                if this.list_keys {
                                    this.list_keys = false;
                                    cx.notify();
                                }
                            }),
                        )
                        .child(if self.editor.open {
                            self.render_editor(cx)
                        } else {
                            right
                        })
                }),
            );

        // 配置面板挂在**整窗**这一层，不塞进输入区：塞进去它是 absolute 浮层，
        // 超出输入区那 130px 的部分 hitbox 会被裁掉 —— 画得出来、hover 也变色，
        // 但 on_click 一次都进不来（踩过）。
        let body = div()
            .relative()
            .size_full()
            .child(shell_root)
            // 合同主版本不匹配：明确报，不猜、不降级。只是一条横幅，不动别的布局。
            //
            // 形态（demo/desk8/preview.html 方案 B，已挑定）：淡红底 + 3px 左边条 + 深红字，
            // 右侧「详情」展开第二行（两个版本号 + 一句该怎么办）。
            // 为什么不用整条红底白字：那是「正在出事」的响度，而版本不匹配是**持续状态**，
            // 整条红挂一天会让人要么习惯它、要么把整个窗口关掉 —— 两种都糟。
            // 红色只留一条 3px 的线：眼睛扫到就停，不抢内容的体积。
            .children(self.contract_mismatch().then(|| {
                let open = self.contract_banner_open;
                let engine = self.engine_contract();
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_col()
                    .bg(rgb(t().p.danger_soft))
                    .border_l(t().d(3.))
                    .border_color(rgb(t().p.danger))
                    .text_size(t().fs(11.5))
                    .text_color(rgb(t().p.danger_fg))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(10.))
                            .px(t().d(12.))
                            .py(t().d(3.))
                            .child(match engine {
                                Some(engine) => format!(
                                    "! 客户端与引擎版本不匹配（客户端 contract {} · 引擎 {engine}）",
                                    crate::capability::CLIENT_CONTRACT
                                ),
                                None => "! 客户端与引擎版本不匹配".to_string(),
                            })
                            .child(div().flex_1())
                            .child(
                                Button::new("contract-banner-detail")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .label(if open { "收起" } else { "详情" })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.contract_banner_open = !this.contract_banner_open;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(open, |el| {
                        el.child(
                            div()
                                .px(t().d(12.))
                                .pb(t().d(4.))
                                .text_size(t().fs(11.))
                                .child(format!(
                                    "本客户端只认 contract {}；引擎报的是 {}。                                     主版本不同意味着字段形状可能已变 —— 这里不猜、不降级：                                     要么换客户端，要么把引擎换回同一代。",
                                    crate::capability::CLIENT_CONTRACT,
                                    engine.map_or_else(|| "（没连上）".to_string(), |n| n.to_string()),
                                )),
                        )
                    })
            }))
            .children(self.cfg_overlay(window, cx))
            // 「≡ 更多」菜单压在最上面（它自带一层点空白收起的罩子）。
            .children(self.menu_open.then(|| self.render_menu_overlay(cx)))
            // 触发器：卡片右键菜单 / 保存冲突（挂在整窗，坐标才是窗口坐标）
            .children(
                (self.tab == Tab::Triggers && self.triggers.menu.is_some())
                    .then(|| self.trigger_menu_overlay(cx)),
            )
            .children(
                (self.tab == Tab::Triggers && self.triggers.conflict.is_some())
                    .then(|| self.trigger_conflict_overlay(cx)),
            )
            // 「重命名会话」（居中对话框）：压在菜单之上、撕原件之下
            .children(self.rename_overlay(window, cx))
            // 「撕原件」确认（盖在菜单之上）
            .children(self.purge_overlay(window, cx))
            // 窄窗的类别抽屉（比弹层低一层：弹层永远压在它上面）
            .children(self.cat_drawer_overlay(cx))
            // 「新建类别」弹层
            .children(self.new_cat_overlay(cx))
            .children(self.copy_overlay(cx))
            .children(self.cat_dialog_overlay(cx))
            .children(self.import_overlay(cx))
            .children(self.preset_rename_overlay(cx))
            // Template 页：改类别 / 归档·彻底删的确认（最上面两层）
            .children(self.cat_edit_overlay(cx))
            .children(self.preset_confirm_overlay(cx))
            // 编辑器：文本字段输入 / 发布
            .children(self.palette_overlay(cx))
            .children(self.picker_overlay(cx))
            .children(self.text_edit_overlay(cx))
            .children(self.publish_overlay(cx))
            // 轻提示：最后一层，永远看得见。
            .children(self.render_notice());

        // Linux：内容区上面压一条自绘标题栏（GNOME 不给 SSD，见 `chrome.rs`）。
        // 其它平台仍然是系统标题栏，这里原样返回。
        #[cfg(target_os = "linux")]
        let body = div()
            .flex()
            .flex_col()
            .size_full()
            .child(crate::chrome::title_bar(window, self.demo_mode()))
            .child(div().flex_1().min_h(t().d(0.)).child(body));

        body
    }
}

/// 空正文的气泡不画 —— 引擎对「有 entry 没内容」会给空串/null，
/// 画出来就是一个白方块（坑.md 第 7 条）。
/// `nav_spy` 和 `render_chat` 共用这一条，保证「滚动容器里的第几个孩子」算得对。
fn entry_visible(e: &Entry) -> bool {
    e.text
        .as_deref()
        .map(|t| !t.trim().is_empty())
        .unwrap_or(false)
}

/// 第 `entry_index` 条历史在滚动容器里是第几个孩子。
///
/// 空正文被跳过，主视图前面有「更早」+ 说明两行，抽屉前面有一行提示。
/// 纯函数（只吃 entries 和几个开关）—— 「目录点一下跳歪」这条最容易错，
/// 所以拿数字直接钉住单测，不靠肉眼对齐。
fn nav_child_index(
    entries: &[Entry],
    entry_index: usize,
    from: usize,
    drawer: bool,
    loading: bool,
) -> Option<usize> {
    if entry_index < from || entry_index >= entries.len() || !entry_visible(&entries[entry_index]) {
        return None;
    }
    let mut n = if drawer {
        1 // 抽屉里的提示行
    } else {
        let mut head = 0;
        if from > 0 && !loading && !entries.is_empty() {
            head += 2; // 「更早」条 + 说明
        }
        if loading || entries.is_empty() {
            head += 1; // 状态行
        }
        head
    };
    for e in &entries[from..entry_index] {
        if entry_visible(e) {
            n += 1;
        }
    }
    Some(n)
}

// 排序写错一次能烦一星期，所以拿数字直接钉住。
#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{
        Bar, ListIndex, NoticeKind, NoticeState, Recent, brief_of, bumped_sessions, error_notice,
        nav_child_index, next_selection_after_archive, order_ids, short_cwd, short_model,
        sort_sessions, when_text,
    };
    use nous_engine::EngineError;
    use nous_engine::types::{Entry, Session, Unread, UnreadState};
    use serde_json::json;
    use std::collections::HashMap;

    /// 造一张未读表：`&[(会话 id, 未读数)]`。
    fn unread(pairs: &[(i64, u64)]) -> Unread {
        Unread(
            pairs
                .iter()
                .map(|(id, n)| {
                    (
                        id.to_string(),
                        UnreadState {
                            unread: *n,
                            read_at: None,
                            extra: HashMap::new(),
                        },
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn 后台心跳_未读变多才算跑完() {
        // 0 → 2：跑完了。
        assert_eq!(bumped_sessions(&unread(&[(7, 0)]), &unread(&[(7, 2)]), None), 1);
        // 没变：只是又拉了一次表。
        assert_eq!(bumped_sessions(&unread(&[(7, 2)]), &unread(&[(7, 2)]), None), 0);
        // 少了（在别的窗口标了已读）：不是跑完。
        assert_eq!(bumped_sessions(&unread(&[(7, 2)]), &unread(&[(7, 1)]), None), 0);
    }

    #[test]
    fn 后台心跳_一次多了两个会话是两件事() {
        let prev = unread(&[(7, 0), (9, 3), (11, 1)]);
        let next = unread(&[(7, 1), (9, 3), (11, 2)]);
        assert_eq!(bumped_sessions(&prev, &next, None), 2);
    }

    #[test]
    fn 后台心跳_正在看的那条不算_别响两声() {
        // 会话 7 既在流、未读也涨了：这一声归 SSE 那条路（`drained.done`）。
        let prev = unread(&[(7, 0), (9, 0)]);
        let next = unread(&[(7, 1), (9, 1)]);
        assert_eq!(bumped_sessions(&prev, &next, Some(7)), 1);
        assert_eq!(bumped_sessions(&prev, &next, Some(9)), 1);
        assert_eq!(bumped_sessions(&prev, &next, Some(-1)), 2);
    }

    #[test]
    fn 后台心跳_新出现的会话当成从零涨上来() {
        // 老表里没这个键（新建的会话），按 0 算 —— 不然启动后新建会话跑完永远不响。
        assert_eq!(bumped_sessions(&unread(&[]), &unread(&[(21, 1)]), None), 1);
    }

    fn e(id: i64, kind: &str, text: Option<&str>) -> Entry {
        Entry {
            id: json!(id),
            kind: kind.into(),
            text: text.map(str::to_string),
            parent: None,
            extra: HashMap::new(),
        }
    }

    /// `Bar` 的下标就是这个面板在 `bars` 数组里的位置 —— 少一个、重一个，
    /// 表现是「另一个面板的滚动条不动了」，而不会有任何编译错误。
    #[test]
    fn 每块面板的下标都在数组范围内且互不重复() {
        let mut seen = vec![false; Bar::COUNT];
        for bar in Bar::ALL {
            let i = bar.idx();
            assert!(i < Bar::COUNT, "{bar:?} 的下标 {i} 越界了");
            assert!(!seen[i], "{bar:?} 和另一块面板撞了同一个下标 {i}");
            seen[i] = true;
        }
        assert!(seen.iter().all(|hit| *hit), "有下标没人认领：{:?}", seen);
    }

    /// 顶栏那行「带什么」：空的段不出现，别留一串悬空的 `·`。
    #[test]
    fn 会话顶栏摘要只写有内容的段() {
        let base = Session {
            id: 1,
            name: "x".into(),
            parent: None,
            owner: None,
            model: None,
            thinking_level: None,
            thinking_levels: vec![],
            skills: vec![],
            template: None,
            read_only: false,
            cwd: None,
            machine: None,
            busy: None,
            updated_at: None,
            last: None,
            archived: None,
            pinned: false,
            extra: HashMap::new(),
        };
        assert_eq!(brief_of(&base), "", "什么都没有就是空串，不是四个点");

        let full = Session {
            model: Some("volc-ark/deepseek-v4-flash-260425".into()),
            template: Some("smart-ass".into()),
            skills: vec!["a".into(), "b".into()],
            read_only: true,
            ..base.clone()
        };
        // 模型摘掉 provider 前缀和日期尾巴，这是 `short_model` 的活儿。
        assert_eq!(brief_of(&full), "deepseek-v4-flash · smart-ass · 2 技能 · 只读");

        // `0 技能` 是噪声：没有就是没有。
        let no_skill = Session {
            skills: vec![],
            ..full.clone()
        };
        assert_eq!(brief_of(&no_skill), "deepseek-v4-flash · smart-ass · 只读");
    }

    /// 滚动条默认不在场：不然一启动每块面板都挂着一根线。
    #[test]
    fn 滚动条默认不在场() {
        let bar = super::scrollbar::BarState::default();
        assert!(!bar.shown && !bar.hover);
    }

    /// 目录点一行，跳的必须是那一行 —— 空正文气泡被跳过 + 前面几行装饰，
    /// 拿 entry 下标直接当孩子下标会跳歪。
    #[test]
    fn 目录跳转的孩子下标要跳过空气泡和装饰行() {
        let es = vec![
            e(1, "pi.user", Some("第一句")),
            e(2, "pi.assistant", None), // 空正文：不画
            e(3, "pi.user", Some("第二句")),
            e(4, "pi.assistant", Some("回答")),
            e(5, "pi.user", Some("第三句")),
        ];
        // 抽屉：前面一行提示，空气泡跳过。
        assert_eq!(nav_child_index(&es, 0, 0, true, false), Some(1));
        assert_eq!(
            nav_child_index(&es, 1, 0, true, false),
            None,
            "空气泡没画出来"
        );
        assert_eq!(nav_child_index(&es, 2, 0, true, false), Some(2));
        assert_eq!(nav_child_index(&es, 4, 0, true, false), Some(4));
        // 主视图 from=3：前面「更早」+ 说明两行，从 3 开始数。
        assert_eq!(nav_child_index(&es, 4, 3, false, false), Some(3));
        assert_eq!(
            nav_child_index(&es, 2, 3, false, false),
            None,
            "不在主视图里"
        );
        // 拉历史中：只有一行状态，没有「更早」。
        assert_eq!(nav_child_index(&es, 4, 0, false, true), Some(4));
        assert_eq!(
            nav_child_index(&es, 2, 3, false, false),
            None,
            "不在主视图里"
        );
    }

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
        use crate::md::strip_harness;
        assert_eq!(
            strip_harness(
                "<harness>
[error] boom
</harness>"
            ),
            "[error] boom"
        );
        assert_eq!(
            strip_harness(
                "j1900
/root"
            ),
            "j1900
/root"
        );
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
    fn 单换行补成硬换行() {
        use crate::md::hard_breaks;
        assert_eq!(
            hard_breaks(
                "一
二"
            ),
            "一  
二"
        );
        assert_eq!(
            hard_breaks(
                "一

二"
            ),
            "一  

二"
        );
        // 代码块里的换行一个都不能碰（会改成代码内容）
        assert_eq!(
            hard_breaks(
                "```
a
b
```"
            ),
            "```
a
b
```"
        );
        // 空行不补（补了就是三个空格挂在那）
        assert_eq!(hard_breaks(""), "");
    }

    #[test]
    fn 代码围栏跟着内容变长() {
        use super::fenced_code;
        assert_eq!(
            fenced_code("ls"),
            "```
ls
```"
        );
        // 输出里本身有三连反引号：围栏要更长，不然块会被提前切断
        assert_eq!(
            fenced_code(
                "```
x"
            ),
            "````
```
x
````"
        );
        assert_eq!(
            fenced_code(
                "```
x"
            ),
            r"````
```
x
````"
        );
    }

    #[test]
    fn 置顶的排前面_且按置顶顺序() {
        // 原顺序 3,2,1；置顶顺序 1,3（新置顶的在最前）
        assert_eq!(order_ids(&[3, 2, 1], &[1, 3]), vec![1, 3, 2]);
    }

    /// 键盘导航的「下一行」：到头停住、不在名单里时从两头进。
    #[test]
    fn 键盘上下走到头就停() {
        let mut index = ListIndex::default();
        for (i, id) in [10, 20, 30].into_iter().enumerate() {
            index.push(id, i + 1); // 故意跟下标错开一位，模拟搜索模式的分组标题
        }
        assert_eq!(index.step(None, 1), Some((10, 1)), "空列表位进 = 第一条");
        assert_eq!(index.step(None, -1), Some((30, 3)), "往上进 = 最后一条");
        assert_eq!(index.step(Some(10), 1), Some((20, 2)));
        assert_eq!(index.step(Some(30), 1), Some((30, 3)), "到底停住，不绕回第一条");
        assert_eq!(index.step(Some(10), -1), Some((10, 1)), "到顶停住");
        assert_eq!(index.step(Some(99), 1), Some((10, 1)), "选中的被过滤掉了 → 从头开始");
        assert_eq!(index.step(Some(20), -1), Some((10, 1)));
    }

    #[test]
    fn 空列表怎么按都选不中东西() {
        let index = ListIndex::default();
        assert_eq!(index.step(None, 1), None);
        assert_eq!(index.step(Some(10), -1), None);
    }

    #[test]
    fn 置顶里已经不存在的会话被忽略() {
        assert_eq!(order_ids(&[5, 4], &[99, 4]), vec![4, 5]);
    }

    /// 2026-11-16 的 bug 回归：`last.id`（几百）和空会话的会话 id（1500+）混在一起比大小，
    /// 结果 15 条真聊过的会话全部掉到 111 名以后，上面 110 行是空壳。
    /// 现在分成两层 —— 空壳的 id 再大也排不到聊过的前面。
    #[test]
    fn 聊过的永远在空壳前面() {
        let mut list = vec![s(1539, None), s(2, Some(411)), s(1533, None), s(25, Some(300))];
        sort_sessions(&mut list, &Recent::default());
        let ids: Vec<i64> = list.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![2, 25, 1539, 1533], "聊过的按 last.id 倒序在前，空壳按 id 倒序在后");
    }

    /// 空壳之间：连引擎也不给时间，只能看本机点开过没有 —— 点过的那批在上，按时间倒序。
    #[test]
    fn 空壳按本机最近打开排_没点过的按_id() {
        let mut recent = Recent::default();
        recent.remember(100, 1_700_000_500);
        recent.remember(900, 1_700_000_900);
        let mut list = vec![s(1500, None), s(100, None), s(900, None), s(700, None)];
        sort_sessions(&mut list, &recent);
        let ids: Vec<i64> = list.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![900, 100, 1500, 700], "点过的按时间倒序，剩下的按 id 倒序");
    }

    /// 引擎自己给了 `updatedAt` 时以它为准 —— 本机记录只是引擎没话说时的兜底，
    /// 两边不一致不能拿本地的把引擎的按下去（否则「忘了清理 recent.json」就会盖掉真相）。
    #[test]
    fn 引擎给了时间就优先于本机记录() {
        let mut recent = Recent::default();
        recent.remember(20, 1_700_000_900);
        let engine_old = Session {
            updated_at: Some(1_700_000_100_000),
            ..s(10, None)
        };
        let mut list = vec![s(20, None), engine_old];
        sort_sessions(&mut list, &recent);
        assert_eq!(list[0].id, 10, "引擎时间（早）与本地时间（晚）不可直接比数值 —— 引擎那条先");
    }

    /// 本机记录只记「打开」这件事，不该把聊过的会话也抬到引擎上面（那会盖掉「最后说话」的排序）。
    #[test]
    fn 本机记录不会改变聊过那堆的内部顺序() {
        let mut recent = Recent::default();
        recent.remember(2, 1_700_000_900);
        let mut list = vec![s(2, Some(411)), s(25, Some(700))];
        sort_sessions(&mut list, &recent);
        let ids: Vec<i64> = list.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![25, 2], "最后说话的在上，不看本机打开时间");
    }

    #[test]
    fn 没有置顶就保持原顺序() {
        assert_eq!(order_ids(&[3, 2, 1], &[]), vec![3, 2, 1]);
    }
    #[test]
    fn 重复置顶不会重复出现() {
        assert_eq!(order_ids(&[1, 2], &[2, 2]), vec![2, 1]);
    }

    /// 元信息那行里的模型名：provider 前缀和日期后缀都要摘掉，`undefined` 当没有。
    ///
    /// 这几个值是引擎真给过的（`volc-ark/deepseek-v4-flash-260425` / `undefined/undefined`），
    /// 写死在测试里是为了哪天引擎改了命名能立刻红。
    #[test]
    fn 模型名前缀和日期后缀都摘掉() {
        assert_eq!(
            short_model(Some("volc-ark/deepseek-v4-flash-260425")).as_deref(),
            Some("deepseek-v4-flash")
        );
        assert_eq!(
            short_model(Some("Qoder/Qoder/DeepSeek-Flash")).as_deref(),
            Some("DeepSeek-Flash")
        );
        // 尾部不是 6 位数字就别当日期，名字里自带的 `-flash` 不能给削了
        assert_eq!(
            short_model(Some("ark/glm-5.3-flash")).as_deref(),
            Some("glm-5.3-flash")
        );
        assert_eq!(short_model(Some("undefined/undefined")), None);
        assert_eq!(short_model(Some("")), None);
        assert_eq!(short_model(None), None);
    }

    /// `cwd` 只留末两段：149 条里绝大多数是同一个目录，全写会把整行吃满。
    #[test]
    fn 工作目录只留末两段() {
        assert_eq!(
            short_cwd(Some("/opt/Nous/content/new1/map/qrd3wh")).as_deref(),
            Some("…/map/qrd3wh")
        );
        assert_eq!(short_cwd(Some("/root")).as_deref(), Some("/root"));
        assert_eq!(short_cwd(Some("a/b")).as_deref(), Some("a/b"));
        assert_eq!(short_cwd(Some("   ")), None);
        assert_eq!(short_cwd(None), None);
    }

    // ── 票 #6：归档接线（只测纯函数缝）──────────────────────────────

    /// 一条会话的最小形状。`last` 决定它在列表里的活跃度，跟引擎的真 JSON 同形。
    fn s(id: i64, last: Option<i64>) -> Session {
        serde_json::from_value(json!({
            "id": id,
            "name": format!("对话 {id}"),
            "parent": null,
            "owner": null,
            "model": null,
            "thinkingLevel": null,
            "thinkingLevels": [],
            "skills": [],
            "template": null,
            "readOnly": false,
            "cwd": null,
            "machine": null,
            "busy": null,
            "updatedAt": null,
            "last": last.map(|l| json!({"id": l, "kind": "pi.user", "text": "嗯"})),
        }))
        .expect("会话 JSON 要能解析")
    }

    /// 归档掉的正是当前打开的那条：退到**列表顺序**里的下一条
    /// （= 最近一条能说话的），不是 id 最大 / 最小的那条。
    #[test]
    fn 归档当前会话后退到列表里的下一条() {
        let list = vec![s(3, Some(30)), s(2, Some(20)), s(1, Some(10))];
        assert_eq!(next_selection_after_archive(&list, 3), Some(2));
        assert_eq!(
            next_selection_after_archive(&list, 1),
            Some(3),
            "退让看列表顺序，不是 id 大小"
        );
    }

    /// 一条都不剩：返回 `None`，调用方据此回空态（清 entries + 停 SSE），别留空壳。
    #[test]
    fn 归档最后一条会话后没有可退的() {
        assert_eq!(next_selection_after_archive(&[s(7, None)], 7), None);
        assert_eq!(next_selection_after_archive(&[], 7), None);
    }

    /// 连弹两条提示时，第一条的定时器回来对不上号 —— 不许把第二条提前收掉。
    #[test]
    fn 旧定时器不会收掉后弹的提示() {
        let mut notice = NoticeState::default();
        let first = notice.show("已归档 · 可在归档库捞回", NoticeKind::Info);
        let second = notice.show("会话还有活分支：304、305", NoticeKind::Error);

        assert!(!notice.expire(first), "第一条的定时器不该收掉第二条");
        assert_eq!(
            notice.current().map(|(text, kind)| (text.as_str(), *kind)),
            Some(("会话还有活分支：304、305", NoticeKind::Error)),
            "第二条要原样在场"
        );

        assert!(notice.expire(second), "第二条自己的定时器才收得掉它");
        assert!(notice.current().is_none(), "收掉之后不留空壳");
    }

    /// 引擎 409 的 `{error}` 原文逐字进提示：不加前缀、不翻译、不截断
    /// （里面那些活分支名字是用户唯一能看到的线索）。
    #[test]
    fn 引擎错误原文逐字进提示() {
        let message = "会话 302 还有活分支：304「改路由」、305「补测试」";
        let err = EngineError::Engine {
            status: 409,
            message: message.to_string(),
        };

        assert_eq!(error_notice(&err), message);
    }
}

/// 性能探针的假会话标记。实现放在这儿而不是 `perf.rs`：`perf` 不该知道 `Session`
/// 长什么样（它只负责「复制 + 计时」这一件事）。孤儿规则允许 —— trait 是本 crate 的。
impl crate::perf::FakeId for nous_engine::types::Session {
    fn set_fake(&mut self, i: usize) {
        // id 从 100 万起：跟真会话撞不上，误点也只会 404，改不到真数据。
        self.id = 1_000_000 + i as i64;
        self.name = format!("perf-{i:05}");
        // 假行一律当主线。克隆的那条万一是个分支，500 条全会带 `parent`，
        // 列表按主线过滤之后一行不剩 —— 量出来 0 行，看着像「探针没生效」。
        self.parent = None;
    }
}
