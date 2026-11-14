//! 主题：一套皮的**全部**外观 token（配色 / 排版 / 尺寸 / 布局）+ 落盘。
//!
//! ## 这一层解决什么
//!
//! 上一版只有 13 个语义色，渲染层还在用各模块的 `const` 取色 —— 那样加皮
//! 只能改卡片缩略图，界面不会变。这一版把「一套皮」定义成完整的 token 表：
//!
//! - `Palette`：约 50 个语义色槽，覆盖渲染层原来散落的 127 个字面量；
//! - `Type`：字体族、正文倍率、行高、标题字重；
//! - `Metrics`：圆角倍率、栏宽、标题栏高、气泡内边距；
//! - `Layout`：`Classic`（三栏）/ `NoRail`（收掉图标条）。
//!
//! 加一套皮 = 在 [`ThemeName`] 加变体 + 在 `skin()` 里给一份 `Skin`，
//! **设置页和各渲染模块一行都不用改**。
//!
//! ## 怎么取当前皮
//!
//! [`ActiveSkin`] 是 GPUI 的 `Global`，**唯一真相**：启动时从 `theme.toml`
//! 读、设置页点卡片时换。但渲染层有 50 多个自由函数（`pill` / `nav_row` /
//! `theme_thumb` …）拿不到 `cx`，给每个都加一个 `t: Skin` 参数会把签名和
//! 调用点全拖下水。
//!
//! 所以 `Desktop::render` 开头调 [`begin_frame`] 把当前皮拍成一份帧内快照
//! （`thread_local!` 的 `Cell<Skin>`），渲染函数统一 `let t = cur();`，
//! 然后 `rgb(t.p.panel)` / `t.fs(12.)` / `t.r(8.)` / `t.d(6.)`。
//! GPUI 的 `text_style_stack` 本身就是这么干的；快照**不是第二个真相**，
//! 每帧从 Global 重取，测试 / example 没开帧时回默认皮。
//!
//! [`ActiveSkin::set`] 会同时把字体族 / 基础字号 / 明暗 / 圆角同步给
//! `gpui-component::Theme`（它管 `Input` / `Textarea` / `TitleBar` 这些组件），
//! 并 `refresh_windows()`。我们自己手搭的 `div` 走 `Skin`。
//!
//! ## 三条边界（别破）
//!
//! - **盘面（`board_art`）不跟主题走。** 那是照抄 qrd3wh 面板页的 Flexoki dark
//!   画布，换皮会把「看着像但不是」变成「压根不像」；只有它的**宽度基准**
//!   跟着 `Metrics` 走（`pane_width`），因为栏宽变了它得让开。
//! - 主题落盘在 `~/.config/nousdesk/theme.toml`，**不写 `desktop.toml`**：那份是
//!   连接配置（只有 host / board 两个键），便携包里它排在 exe 旁边、可能只读。
//! - 读坏了 / 名字不认识 → **回默认，不报错**。UI 偏好不该挡住启动；
//!   `desktop.toml` 写坏要报错，两者严格程度不同。
//!
//! ## 默认皮不许变样
//!
//! `Skin::DEFAULT` 的每个值都是从原来那些 `const` / 字面量**原样抄**的，
//! `default_skin_matches_shipped_palette` 这条测试钉住它们。接线之后第一件事
//! 是截图对比：默认皮必须和 v0.2.1 逐像素一致，否则就是接线改了观感。

use std::cell::Cell;
use std::path::PathBuf;

use gpui_kit::*;

use crate::config::APP_DIR;

/// 主题文件的文件名（在 `APP_DIR` 下）。
const FILE: &str = "theme.toml";

// ── 配色 ────────────────────────────────────────────────────────────────

/// 一套皮的颜色。
///
/// 字段是**语义**不是位置。有些槽看着重复（`text` / `text_2` / `muted` / `faint`），
/// 是因为原来 127 个字面量里确实有四种灰度各管一件事；合并会让默认皮变样。
///
/// `*_tint` 结尾的是 **0xRRGGBBAA** 值（给 `rgba()` 用，不是 `rgb()`）——
/// GPUI 没有 CSS 的 `color-mix()`，半透明色只能预先给全。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// 最左图标条 / 设置页左轨的深底。
    pub sidebar: u32,
    /// 图标条按钮 hover。
    pub sidebar_hover: u32,
    /// 图标条菜单展开时的底色。
    pub sidebar_open: u32,
    /// 图标条上没选中的图标。
    pub sidebar_idle: u32,
    /// 会话列表底色。
    pub list_bg: u32,
    /// 聊天区底色。
    pub chat_bg: u32,
    /// 卡片 / 浮层 / 标题栏 / 输入区的底。
    pub panel: u32,
    /// 次一级面板（配置面板侧栏、树）。
    pub panel_alt: u32,
    /// 输入框 / 搜索框的底。
    pub surface: u32,
    /// 菜单 / 对话框的底。
    pub overlay: u32,
    /// 我方气泡。
    pub bubble_me: u32,
    /// 对方气泡。
    pub bubble_other: u32,

    /// 正文。
    pub text: u32,
    /// 次一级正文。
    pub text_2: u32,
    /// 次要文字（时间、预览、说明）。
    pub muted: u32,
    /// 更淡的说明 / 占位符。
    pub faint: u32,
    /// 主色上的字。
    pub on_accent: u32,
    /// 深色条上的字（未选中）。
    pub on_sidebar: u32,

    /// 常规分隔线 / 边框。
    pub line: u32,
    /// 更明显的边框（按钮开关、选中树行）。
    pub line_strong: u32,
    /// 极淡的边框（气泡、卡片）。
    pub line_soft: u32,

    /// 主色（选中、发送、可点的按钮）。
    pub accent: u32,
    /// 主色按下 / 悬停。
    pub accent_hover: u32,
    /// 主色上的浅底（选中态、激活 tab）。
    pub accent_soft: u32,
    /// 主色的深字版本（绿字、成功提示文字）。
    pub accent_fg: u32,
    /// 主色的半透明底（选中会话、抽屉罩子等）。**0xRRGGBBAA**。
    pub accent_tint: u32,
    /// 主色浅底的 hover。
    pub accent_soft_hover: u32,

    /// 错误。
    pub danger: u32,
    /// 错误的浅底。
    pub danger_soft: u32,
    /// 错误浅底上的字。
    pub danger_fg: u32,
    /// 警告。
    pub warn: u32,
    /// 警告浅底。
    pub warn_soft: u32,
    /// 成功。
    pub ok: u32,
    /// 成功深字（干净仓库、已连接）。
    pub ok_fg: u32,
    /// 链接 / 分支跳转。
    pub link: u32,
    /// 未读红点。
    pub badge: u32,

    /// hover 的通用浅灰底。
    pub hover: u32,
    /// 选中行的底。
    pub selected: u32,
    /// 禁用态的底 / 字。
    pub disabled: u32,
    /// 头像 / 占位块。
    pub avatar: u32,
    /// 抽屉 / 遮罩。**0xRRGGBBAA**，给 `rgba()`。
    pub scrim: u32,
    /// 浮层投影。**0xRRGGBBAA**，给 `rgba()`。
    pub shadow: u32,
    /// 透明（边框不画时用）。**0xRRGGBBAA**。
    pub transparent: u32,
    /// 代码块 / 行内代码底。
    pub code_bg: u32,
}

impl Palette {
    /// 「默认」：微信基线（骨架照微信桌面版，取值照渲染层原来的常量 / 字面量抄）。
    pub const DEFAULT: Palette = Palette {
        sidebar: 0x2e2e2e,
        sidebar_hover: 0x3a3a3a,
        sidebar_open: 0x3f3f3f,
        sidebar_idle: 0x9a9a9a,
        list_bg: 0xf7f7f7,
        chat_bg: 0xededed,
        panel: 0xffffff,
        panel_alt: 0xfafafa,
        surface: 0xf6f6f6,
        overlay: 0xffffff,
        bubble_me: 0x95ec69,
        bubble_other: 0xffffff,

        text: 0x1a1a1a,
        text_2: 0x333333,
        muted: 0x999999,
        faint: 0xaaaaaa,
        on_accent: 0xffffff,
        on_sidebar: 0x9a9a9a,

        line: 0xe6e6e6,
        line_strong: 0xe2e2e2,
        line_soft: 0xeeeeee,

        accent: 0x07c160,
        accent_hover: 0x06ad56,
        accent_soft: 0xe8f7ee,
        accent_fg: 0x0a7a3f,
        accent_tint: 0x07c16022,
        accent_soft_hover: 0xdcf0e3,

        danger: 0xd32f2f,
        danger_soft: 0xfff1f0,
        danger_fg: 0xc0392b,
        warn: 0xc77700,
        warn_soft: 0xfff8ee,
        ok: 0x079c50,
        ok_fg: 0x2e7d32,
        link: 0x0b6bcb,
        badge: 0xfa5151,

        hover: 0xf2f2f2,
        selected: 0xd9d9d9,
        disabled: 0xb2b2b2,
        avatar: 0x7ba7d7,
        scrim: 0x0000002a,
        shadow: 0x00000014,
        transparent: 0x00000000,
        code_bg: 0xf4f4f4,
    };
}

// ── 排版 / 尺寸 ─────────────────────────────────────────────────────────

/// 窗口骨架。加新布局要同步 `board_art::pane_width`。
#[allow(dead_code)] // NoRail 是给 demo 09 预留的，等那套皮落地才会被构造。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// 图标条 + 会话列表 + 右栏（出厂）。
    #[default]
    Classic,
    /// 收掉图标条，菜单折进会话列表标题行（demo 09）。
    NoRail,
}

/// 一套皮的全部外观 token。`Copy`，渲染函数里随手传。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Skin {
    pub p: Palette,
    pub layout: Layout,

    /// 界面字体族名。`""` = **沿用 GPUI 默认**（出厂观感，Linux 上会由
    /// gpui-component 解析成系统字体）。换皮时给一个真实存在的族名
    /// （`Noto Sans CJK SC`，或启动时 `add_fonts` 注册过的名字）——
    /// GPUI 没有 CSS 的 fallback 链，别写 `"Inter, sans-serif"`，
    /// 也别直接写 `.SystemUIFont`（那个名字在 Linux 上加载不到字体，
    /// 会让整片文本消失，踩过）。
    pub font: &'static str,
    /// 等宽字体族名。`""` 同样表示沿用默认。
    pub mono: &'static str,
    /// 正文字号（px）。只作为 `fs()` 之外的基准和组件主题的 font_size。
    pub body: f32,
    /// 正文行高倍率。`0.0` = **不施加行高，沿用 GPUI 默认**（出厂观感）。
    /// 换皮给 1.45 / 1.6 之类；绝对 px 由 `lh()` 算出。
    pub line_height: f32,
    /// 字号倍率：所有 `text_size(px(x))` 走 [`Skin::fs`] 乘上它。
    /// 默认 1.0 —— 出厂那些 11 / 12 / 13 一个都不动。
    pub type_scale: f32,
    /// 圆角倍率：所有 `rounded(px(x))` 走 [`Skin::r`] 乘上它。
    /// 直角皮给 0，大圆角皮给 1.4 之类。
    pub radius_scale: f32,
    /// 密度倍率：行间距 / 内边距里的 `px` 走 [`Skin::d`]。
    pub density: f32,
    /// 明暗：喂给 `gpui-component::Theme`，决定 `Input` / `Textarea` 的默认配色。
    pub dark: bool,

    /// 图标条宽（0 = 不画，见 `Layout::NoRail`）。
    pub rail_w: f32,
    /// 会话列表宽。
    pub list_w: f32,
    /// Template 页类别树宽。
    pub preset_tree_w: f32,
    /// Template 页预设列表宽。
    pub template_list_w: f32,
    /// 聊天区标题栏高。
    pub head_h: f32,
}

impl Skin {
    /// 「默认」= v0.2.1 出厂外观。**改这里的数字就等于改默认皮**，别顺手调。
    pub const DEFAULT: Skin = Skin {
        p: Palette::DEFAULT,
        layout: Layout::Classic,
        font: "",
        mono: "",
        body: 13.,
        line_height: 0.0,
        type_scale: 1.0,
        radius_scale: 1.0,
        density: 1.0,
        dark: false,
        rail_w: 60.,
        list_w: 280.,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 56.,
    };

    /// 这套皮在「字体 / 字号 / 行高 / 圆角 / 密度 / 明暗」上是否还是出厂值。
    /// 用来决定要不要把组件主题推一遍 —— 换了颜色但没换排版时，也不该动
    /// `Input` / `Textarea` 的文字度量。
    #[inline]
    pub fn is_shipped(self) -> bool {
        self.font.is_empty()
            && self.mono.is_empty()
            && self.line_height == 0.0
            && self.type_scale == 1.0
            && self.radius_scale == 1.0
            && self.density == 1.0
            && !self.dark
    }

    /// 缩放后的字号。所有 `text_size` 都走这里，换皮才真的会改字大小。
    #[inline]
    pub fn fs(self, px: f32) -> Pixels {
        gpui_kit::px(px * self.type_scale)
    }

    /// 缩放后的圆角。所有 `rounded` 都走这里。
    #[inline]
    pub fn r(self, px: f32) -> Pixels {
        gpui_kit::px(px * self.radius_scale)
    }

    /// 缩放后的间距（内边距、gap）。
    #[inline]
    pub fn d(self, px: f32) -> Pixels {
        gpui_kit::px(px * self.density)
    }

    /// 行高（绝对 px，喂 `.line_height`）。`line_height == 0` 时返回 0，
    /// 调用方据此判断「不注入」。
    #[inline]
    pub fn lh(self) -> Pixels {
        gpui_kit::px(self.body * self.line_height)
    }

    /// 给根 `div` 压一层默认文字样式：字体族 + 行高。
    ///
    /// GPUI 的 `Styled` 没有 `with_text_style`（那是 Window 的方法），所以
    /// 调用方分别用 `.font_family(self.font)` / `.line_height(self.lh())`。
    /// 这个方法留着给以后要一次性铺 `TextStyleRefinement` 的地方用。
    #[allow(dead_code)]
    pub fn text_style(self) -> TextStyleRefinement {
        TextStyleRefinement {
            font_family: Some(self.font.into()),
            line_height: Some(self.lh().into()),
            ..Default::default()
        }
    }

    /// 颜色取好的 `Rgba`，少写一层 `rgb()`。
    #[allow(dead_code)]
    #[inline]
    pub fn c(self, slot: impl FnOnce(Palette) -> u32) -> Rgba {
        rgb(slot(self.p))
    }

    /// 带 alpha 的槽（0xRRGGBBAA），少写一层 `rgba()`。
    #[allow(dead_code)]
    #[inline]
    pub fn a(self, slot: impl FnOnce(Palette) -> u32) -> Rgba {
        rgba(slot(self.p))
    }
}

impl Default for Skin {
    fn default() -> Self {
        Skin::DEFAULT
    }
}

// ── 当前皮（GPUI Global）────────────────────────────────────────────────

/// 当前生效的皮。**唯一真相**：启动时从 `theme.toml` 读、设置页点卡片时换。
#[derive(Debug, Clone, Copy)]
pub struct ActiveSkin {
    pub name: ThemeName,
    pub skin: Skin,
}

impl Global for ActiveSkin {}

thread_local! {
    /// 帧内快照：`Desktop::render` 每帧从 `ActiveSkin` 重取。
    /// 没开帧（单测 / example）时是 `Skin::DEFAULT`。
    static CURRENT: Cell<Skin> = const { Cell::new(Skin::DEFAULT) };
}

/// 每帧开头调一次：把 Global 里的当前皮拍进帧内快照。
/// 只有 `Desktop::render` 调，别在别处调 —— 嵌套 render 会互相踩。
#[inline]
pub fn begin_frame(cx: &App) {
    let s = cx
        .try_global::<ActiveSkin>()
        .map(|g| g.skin)
        .unwrap_or_default();
    CURRENT.set(s);
}

/// 帧内当前皮。`Copy`，随便取。
#[inline]
pub fn cur() -> Skin {
    CURRENT.get()
}

impl ActiveSkin {
    /// 启动时装进去。读坏 / 不认识 → 默认。
    pub fn load() -> Self {
        let name = ThemeName::load();
        ActiveSkin {
            name,
            skin: name.skin(),
        }
    }

    /// 换皮：更新 Global + 帧内快照 + 同步组件主题 + 让所有窗口重画。
    ///
    /// 不在这里落盘 —— 调用方（设置页）先落盘成功再叫这个函数，
    /// 免得写不进去却把界面换了，重启又跳回去。
    pub fn set(cx: &mut App, name: ThemeName) {
        let skin = name.skin();
        cx.set_global(ActiveSkin { name, skin });
        CURRENT.set(skin);
        skin.apply(cx);
    }
}

impl Skin {
    /// 把字体 / 明暗 / 圆角 / 基础字号同步给 `gpui-component::Theme`。
    ///
    /// 我们手搭的 `div` 走 `Skin`，但 `Input` / `Textarea` / `TitleBar` /
    /// `Icon` 是组件，它们读自己的全局 Theme。不推这一把，换皮之后
    /// 设置页那两个输入框还是白的。
    pub fn apply(self, cx: &mut App) {
        use gpui_kit::component::theme::{Theme, ThemeMode};

        // **出厂皮一个字段都不推。** 组件主题里的 font_size(16) / radius /
        // font_family(系统解析过的那个) 是 v0.2.1 的出厂值；动任何一项都会让
        // Input / Textarea / TitleBar 乃至整片文字的度量变样（真踩过：会话列表
        // 第二行整片消失）。只要外观 token 还和出厂一致就不推。
        if self.is_shipped() {
            return;
        }

        if !cx.has_global::<Theme>() {
            cx.set_global(Theme::default());
        }
        // **先切明暗，再改颜色。** `Theme::update` 里改 `mode` 会连带加载该模式的
        // 主题配置，把同一个闭包里刚写的颜色全盖掉（源码注释里那句 "edit colors
        // in a second update after switching mode"）。所以拆两步。
        let mode = if self.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        if Theme::global(cx).mode != mode {
            Theme::change(mode, None, cx);
        }

        Theme::update(cx, |t| {
            // 把我们的语义色推给组件（Input / Textarea / Button / Scrollbar 读这些）。
            // 不推的话深色皮下输入框还是白的、按钮还是组件默认的蓝紫。
            // 只动「皮肤真的换了」的槽，出厂皮在上面已经 early-return 了。
            let p = self.p;
            t.colors.background = rgb(p.chat_bg).into();
            t.colors.foreground = rgb(p.text).into();
            t.colors.border = rgb(p.line).into();
            t.colors.input = rgb(p.surface).into();
            t.colors.muted = rgb(p.hover).into();
            t.colors.muted_foreground = rgb(p.muted).into();
            t.colors.primary = rgb(p.accent).into();
            t.colors.primary_hover = rgb(p.accent_hover).into();
            t.colors.primary_active = rgb(p.accent_hover).into();
            t.colors.primary_foreground = rgb(p.on_accent).into();
            t.colors.accent = rgb(p.accent_soft).into();
            t.colors.accent_foreground = rgb(p.accent).into();
            t.colors.ring = rgb(p.accent).into();
            t.colors.selection = rgb(p.accent_soft).into();
            t.colors.caret = rgb(p.accent).into();
            t.colors.danger = rgb(p.danger).into();
            t.colors.danger_foreground = rgb(p.on_accent).into();
            t.colors.list = rgb(p.panel).into();
            t.colors.list_hover = rgb(p.hover).into();
            t.colors.list_active = rgb(p.selected).into();
            t.colors.popover = rgb(p.overlay).into();
            t.colors.popover_foreground = rgb(p.text).into();
            t.colors.scrollbar = rgb(p.transparent).into();
            t.colors.scrollbar_thumb = rgba(p.scrim).into();
            t.colors.scrollbar_thumb_hover = rgba(p.shadow).into();
            t.colors.secondary = rgb(p.hover).into();
            t.colors.secondary_hover = rgb(p.line_soft).into();
            t.colors.secondary_foreground = rgb(p.text_2).into();

            // 空字符串 = 保留组件原本解析好的系统字体；别写 `.SystemUIFont`
            // 本身（Linux 上那个名字加载不到字体，整片文本会消失）。
            if !self.font.is_empty() {
                t.font_family = self.font.into();
            }
            if !self.mono.is_empty() {
                t.mono_font_family = self.mono.into();
            }
            if self.type_scale != 1.0 {
                t.font_size = self.fs(self.body);
                t.mono_font_size = self.fs(12.);
            }
            if self.radius_scale != 1.0 {
                t.radius = self.r(6.);
                t.radius_lg = self.r(12.);
            }
            t.shadow = false;
        });
        cx.refresh_windows();
    }
}

// ── 主题名 / 落盘 ──────────────────────────────────────────────────────

/// 主题的名字。**落盘存的是 `id()`**，改了 id 老用户的主题就丢 —— 别改。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Default,
    /// Model S · 深色
    ModelS,
    /// Model 3 · 浅色
    Model3,
    /// Cybertruck · 冷钢
    Cybertruck,
    /// MCU · 车载大屏
    Mcu,
    /// Editorial · 官网白
    Editorial,
    /// 夜间驾驶 · 低亮
    NightDrive,
    /// Energy · 储能
    Energy,
    /// Plaid · 红黑
    Plaid,
    /// Model S · 无图标条
    NoRail,
}

impl ThemeName {
    /// 设置页按这个顺序列卡片。加皮只改这里和 `skin()`。
    pub const ALL: &'static [ThemeName] = &[
        ThemeName::Default,
        ThemeName::ModelS,
        ThemeName::Model3,
        ThemeName::Cybertruck,
        ThemeName::Mcu,
        ThemeName::Editorial,
        ThemeName::NightDrive,
        ThemeName::Energy,
        ThemeName::Plaid,
        ThemeName::NoRail,
    ];

    /// 落盘用的 id（小写、稳定）。
    pub fn id(self) -> &'static str {
        match self {
            ThemeName::Default => "default",
            ThemeName::ModelS => "model-s",
            ThemeName::Model3 => "model-3",
            ThemeName::Cybertruck => "cybertruck",
            ThemeName::Mcu => "mcu",
            ThemeName::Editorial => "editorial",
            ThemeName::NightDrive => "night-drive",
            ThemeName::Energy => "energy",
            ThemeName::Plaid => "plaid",
            ThemeName::NoRail => "no-rail",
        }
    }

    /// 设置页上给人看的名字。
    pub fn label(self) -> &'static str {
        match self {
            ThemeName::Default => "默认",
            ThemeName::ModelS => "Model S · 深色",
            ThemeName::Model3 => "Model 3 · 浅色",
            ThemeName::Cybertruck => "Cybertruck · 冷钢",
            ThemeName::Mcu => "MCU · 车载大屏",
            ThemeName::Editorial => "Editorial · 官网白",
            ThemeName::NightDrive => "夜间驾驶 · 低亮",
            ThemeName::Energy => "Energy · 储能",
            ThemeName::Plaid => "Plaid · 红黑",
            ThemeName::NoRail => "Model S · 无图标条",
        }
    }

    /// 这套皮的全部 token。
    pub fn skin(self) -> Skin {
        match self {
            ThemeName::Default => Skin::DEFAULT,
    // Model S · 深色
    ThemeName::ModelS => Skin {
        p: Palette {
            sidebar: 0x1e2228,
            sidebar_hover: 0x33373c,
            sidebar_open: 0x404449,
            sidebar_idle: 0x7e858d,
            list_bg: 0x1a1e24,
            chat_bg: 0x171a20,
            panel: 0x1e2228,
            panel_alt: 0x1c2026,
            surface: 0x15181d,
            overlay: 0x1e2228,
            bubble_me: 0x3a434e,
            bubble_other: 0x1f242b,
            text: 0xf4f4f4,
            text_2: 0xb6bcc3,
            muted: 0x7e858d,
            faint: 0x575c64,
            on_accent: 0xffffff,
            on_sidebar: 0x7e858d,
            line: 0x282e36,
            line_strong: 0x39414a,
            line_soft: 0x22262e,
            accent: 0xe82127,
            accent_hover: 0xea4b50,
            accent_soft: 0x3e2228,
            accent_soft_hover: 0x532228,
            accent_fg: 0xe82127,
            accent_tint: 0xe8212722,
            danger: 0xff6b61,
            danger_soft: 0x422e31,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x3d372b,
            ok: 0x37c26b,
            ok_fg: 0x37c26b,
            link: 0x6e96ff,
            badge: 0xe82127,
            hover: 0x26292f,
            selected: 0x2b323c,
            disabled: 0x454a51,
            avatar: 0x8d2127,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x12151a,
        },
        layout: Layout::Classic,
        font: "Inter",
        mono: "JetBrains Mono",
        body: 13.5,
        line_height: 1.62,
        type_scale: 1.0,
        radius_scale: 1.0,
        density: 1.0,
        dark: true,
        rail_w: 60.0,
        list_w: 280.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 56.0,
    },
    // Model 3 · 浅色
    ThemeName::Model3 => Skin {
        p: Palette {
            sidebar: 0xf5f5f5,
            sidebar_hover: 0xdfdfe0,
            sidebar_open: 0xd1d2d3,
            sidebar_idle: 0x8c9298,
            list_bg: 0xf7f7f7,
            chat_bg: 0xffffff,
            panel: 0xf5f5f5,
            panel_alt: 0xf6f6f6,
            surface: 0xffffff,
            overlay: 0xf5f5f5,
            bubble_me: 0x171a20,
            bubble_other: 0xf2f2f2,
            text: 0x171a20,
            text_2: 0x4e545b,
            muted: 0x8c9298,
            faint: 0xb8bbbf,
            on_accent: 0xffffff,
            on_sidebar: 0x8c9298,
            line: 0xeaeaea,
            line_strong: 0xd6d6d6,
            line_soft: 0xf2f2f2,
            accent: 0x171a20,
            accent_hover: 0x171a20,
            accent_soft: 0xd1d2d3,
            accent_soft_hover: 0xbbbcbe,
            accent_fg: 0x171a20,
            accent_tint: 0x171a2022,
            danger: 0xd32f2f,
            danger_soft: 0xf0d5d5,
            danger_fg: 0xd32f2f,
            warn: 0xc77700,
            warn_soft: 0xeee1ce,
            ok: 0x0f9d58,
            ok_fg: 0x0f9d58,
            link: 0x0b6bcb,
            badge: 0xe82127,
            hover: 0xefefef,
            selected: 0xededed,
            disabled: 0xcbced1,
            avatar: 0x7ba7d7,
            scrim: 0x0000002a,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0xf6f6f6,
        },
        layout: Layout::Classic,
        font: "Manrope",
        mono: "JetBrains Mono",
        body: 13.5,
        line_height: 1.62,
        type_scale: 1.0,
        radius_scale: 0.75,
        density: 1.1429,
        dark: false,
        rail_w: 60.0,
        list_w: 288.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 58.0,
    },
    // Cybertruck · 冷钢
    ThemeName::Cybertruck => Skin {
        p: Palette {
            sidebar: 0x111415,
            sidebar_hover: 0x25292a,
            sidebar_open: 0x323536,
            sidebar_idle: 0x6b767b,
            list_bg: 0x0e1112,
            chat_bg: 0x0b0d0e,
            panel: 0x111415,
            panel_alt: 0x101214,
            surface: 0x080a0b,
            overlay: 0x111415,
            bubble_me: 0x1f292c,
            bubble_other: 0x141a1c,
            text: 0xdde3e6,
            text_2: 0x98a3a8,
            muted: 0x6b767b,
            faint: 0x474e52,
            on_accent: 0x04171c,
            on_sidebar: 0x6b767b,
            line: 0x232a2c,
            line_strong: 0x374144,
            line_soft: 0x1a1f21,
            accent: 0x6fd7ee,
            accent_hover: 0x85d9ec,
            accent_soft: 0x203338,
            accent_soft_hover: 0x29474d,
            accent_fg: 0x6fd7ee,
            accent_tint: 0x6fd7ee22,
            danger: 0xff6b61,
            danger_soft: 0x372221,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x322b1b,
            ok: 0x7fd4a8,
            ok_fg: 0x7fd4a8,
            link: 0x6e96ff,
            badge: 0xff5f45,
            hover: 0x1a1c1d,
            selected: 0x171d1f,
            disabled: 0x363c3f,
            avatar: 0x457f8c,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x080a0b,
        },
        layout: Layout::Classic,
        font: "Archivo",
        mono: "JetBrains Mono",
        body: 13.0,
        line_height: 1.6,
        type_scale: 0.963,
        radius_scale: 0.0,
        density: 1.0,
        dark: true,
        rail_w: 60.0,
        list_w: 286.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 54.0,
    },
    // MCU · 车载大屏
    ThemeName::Mcu => Skin {
        p: Palette {
            sidebar: 0x0b0b0c,
            sidebar_hover: 0x232324,
            sidebar_open: 0x323233,
            sidebar_idle: 0x7a8086,
            list_bg: 0x070708,
            chat_bg: 0x000000,
            panel: 0x0b0b0c,
            panel_alt: 0x09090a,
            surface: 0x0a0a0b,
            overlay: 0x0b0b0c,
            bubble_me: 0x262a31,
            bubble_other: 0x141517,
            text: 0xffffff,
            text_2: 0xb9bec4,
            muted: 0x7a8086,
            faint: 0x4c4f53,
            on_accent: 0xffffff,
            on_sidebar: 0x7a8086,
            line: 0x1c1d20,
            line_strong: 0x2c2e32,
            line_soft: 0x111214,
            accent: 0x3e6ae1,
            accent_hover: 0x6588e7,
            accent_soft: 0x131a2e,
            accent_soft_hover: 0x182443,
            accent_fg: 0x3e6ae1,
            accent_tint: 0x3e6ae122,
            danger: 0xff6b61,
            danger_soft: 0x321a1a,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x2d2313,
            ok: 0x3bc46a,
            ok_fg: 0x3bc46a,
            link: 0x6e96ff,
            badge: 0x3e6ae1,
            hover: 0x121212,
            selected: 0x15161a,
            disabled: 0x373a3c,
            avatar: 0x273f81,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x0a0a0b,
        },
        layout: Layout::Classic,
        font: "Inter",
        mono: "JetBrains Mono",
        body: 14.5,
        line_height: 1.68,
        type_scale: 1.0741,
        radius_scale: 1.5,
        density: 1.2857,
        dark: true,
        rail_w: 72.0,
        list_w: 320.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 68.0,
    },
    // Editorial · 官网白
    ThemeName::Editorial => Skin {
        p: Palette {
            sidebar: 0xffffff,
            sidebar_hover: 0xe6e6e6,
            sidebar_open: 0xd8d8d8,
            sidebar_idle: 0x9a9a9a,
            list_bg: 0xffffff,
            chat_bg: 0xffffff,
            panel: 0xffffff,
            panel_alt: 0xffffff,
            surface: 0xffffff,
            overlay: 0xffffff,
            bubble_me: 0x0a0a0a,
            bubble_other: 0xffffff,
            text: 0x0a0a0a,
            text_2: 0x4a4a4a,
            muted: 0x9a9a9a,
            faint: 0xc0c0c0,
            on_accent: 0xffffff,
            on_sidebar: 0x9a9a9a,
            line: 0xececec,
            line_strong: 0xdcdcdc,
            line_soft: 0xf3f3f3,
            accent: 0xe82127,
            accent_hover: 0xbc1c21,
            accent_soft: 0xfbdbdc,
            accent_soft_hover: 0xf9c5c7,
            accent_fg: 0xe82127,
            accent_tint: 0xe8212722,
            danger: 0xd32f2f,
            danger_soft: 0xf8dede,
            danger_fg: 0xd32f2f,
            warn: 0xc77700,
            warn_soft: 0xf6e9d6,
            ok: 0x0f9d58,
            ok_fg: 0x0f9d58,
            link: 0x0b6bcb,
            badge: 0xe82127,
            hover: 0xeeeeee,
            selected: 0xf7f7f7,
            disabled: 0xd2d2d2,
            avatar: 0x7ba7d7,
            scrim: 0x0000002a,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0xfafafa,
        },
        layout: Layout::Classic,
        font: "Archivo",
        mono: "JetBrains Mono",
        body: 15.0,
        line_height: 1.75,
        type_scale: 1.1111,
        radius_scale: 0.25,
        density: 1.4286,
        dark: false,
        rail_w: 64.0,
        list_w: 300.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 68.0,
    },
    // 夜间驾驶 · 低亮
    ThemeName::NightDrive => Skin {
        p: Palette {
            sidebar: 0x0c0e0f,
            sidebar_hover: 0x1d2021,
            sidebar_open: 0x282a2c,
            sidebar_idle: 0x5a6268,
            list_bg: 0x0a0b0c,
            chat_bg: 0x08090a,
            panel: 0x0c0e0f,
            panel_alt: 0x0b0c0e,
            surface: 0x060708,
            overlay: 0x0c0e0f,
            bubble_me: 0x1a2022,
            bubble_other: 0x101314,
            text: 0xb9c0c6,
            text_2: 0x8a9299,
            muted: 0x5a6268,
            faint: 0x3b4044,
            on_accent: 0x0a0c0d,
            on_sidebar: 0x5a6268,
            line: 0x191c1e,
            line_strong: 0x272c2f,
            line_soft: 0x131516,
            accent: 0x7e8d99,
            accent_hover: 0x8a97a2,
            accent_soft: 0x1e2225,
            accent_soft_hover: 0x2a2f33,
            accent_fg: 0x7e8d99,
            accent_tint: 0x7e8d9922,
            danger: 0xff6b61,
            danger_soft: 0x331d1c,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x2e2616,
            ok: 0x5b8c6e,
            ok_fg: 0x5b8c6e,
            link: 0x6e96ff,
            badge: 0x8a6e60,
            hover: 0x141617,
            selected: 0x121516,
            disabled: 0x2d3134,
            avatar: 0x4b545b,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x060708,
        },
        layout: Layout::Classic,
        font: "IBM Plex Sans",
        mono: "JetBrains Mono",
        body: 13.0,
        line_height: 1.72,
        type_scale: 0.963,
        radius_scale: 1.0,
        density: 1.0,
        dark: true,
        rail_w: 58.0,
        list_w: 280.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 54.0,
    },
    // Energy · 储能
    ThemeName::Energy => Skin {
        p: Palette {
            sidebar: 0x0e1b1e,
            sidebar_hover: 0x243133,
            sidebar_open: 0x313e40,
            sidebar_idle: 0x67888a,
            list_bg: 0x0a1214,
            chat_bg: 0x0b1416,
            panel: 0x0e1b1e,
            panel_alt: 0x0c1619,
            surface: 0x091012,
            overlay: 0x0e1b1e,
            bubble_me: 0x12332e,
            bubble_other: 0x10211f,
            text: 0xe6f5f2,
            text_2: 0x9ec0bc,
            muted: 0x67888a,
            faint: 0x445c5e,
            on_accent: 0x04211c,
            on_sidebar: 0x67888a,
            line: 0x1b2e31,
            line_strong: 0x2b474b,
            line_soft: 0x152427,
            accent: 0x2ce1c0,
            accent_hover: 0x51e5ca,
            accent_soft: 0x133b38,
            accent_soft_hover: 0x164e48,
            accent_fg: 0x2ce1c0,
            accent_tint: 0x2ce1c022,
            danger: 0xff6b61,
            danger_soft: 0x352829,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x303122,
            ok: 0x2ce1c0,
            ok_fg: 0x2ce1c0,
            link: 0x6e96ff,
            badge: 0x2ce1c0,
            hover: 0x1a2425,
            selected: 0x12282a,
            disabled: 0x34484a,
            avatar: 0x1e8877,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x081012,
        },
        layout: Layout::Classic,
        font: "Manrope",
        mono: "JetBrains Mono",
        body: 13.5,
        line_height: 1.64,
        type_scale: 1.0,
        radius_scale: 1.0,
        density: 1.0714,
        dark: true,
        rail_w: 62.0,
        list_w: 284.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 58.0,
    },
    // Plaid · 红黑
    ThemeName::Plaid => Skin {
        p: Palette {
            sidebar: 0x080808,
            sidebar_hover: 0x212121,
            sidebar_open: 0x303030,
            sidebar_idle: 0x6e6e6e,
            list_bg: 0x050505,
            chat_bg: 0x000000,
            panel: 0x080808,
            panel_alt: 0x060606,
            surface: 0x060606,
            overlay: 0x080808,
            bubble_me: 0xe82127,
            bubble_other: 0x141414,
            text: 0xffffff,
            text_2: 0xb5b5b5,
            muted: 0x6e6e6e,
            faint: 0x444444,
            on_accent: 0xffffff,
            on_sidebar: 0x6e6e6e,
            line: 0x1a1a1a,
            line_strong: 0x2e2e2e,
            line_soft: 0x101010,
            accent: 0xe82127,
            accent_hover: 0xed4d52,
            accent_soft: 0x2c0c0d,
            accent_soft_hover: 0x420e10,
            accent_fg: 0xe82127,
            accent_tint: 0xe8212722,
            danger: 0xff6b61,
            danger_soft: 0x301816,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x2b2110,
            ok: 0x3bc46a,
            ok_fg: 0x3bc46a,
            link: 0x6e96ff,
            badge: 0xe82127,
            hover: 0x121212,
            selected: 0x121212,
            disabled: 0x323232,
            avatar: 0x831619,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x0a0a0a,
        },
        layout: Layout::Classic,
        font: "Inter",
        mono: "JetBrains Mono",
        body: 13.5,
        line_height: 1.6,
        type_scale: 1.0,
        radius_scale: 0.25,
        density: 1.0,
        dark: true,
        rail_w: 60.0,
        list_w: 280.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 54.0,
    },
    // Model S · 无图标条
    ThemeName::NoRail => Skin {
        p: Palette {
            sidebar: 0x1b1e22,
            sidebar_hover: 0x303337,
            sidebar_open: 0x3d3f43,
            sidebar_idle: 0x79818a,
            list_bg: 0x181b1f,
            chat_bg: 0x15171a,
            panel: 0x1b1e22,
            panel_alt: 0x1a1c20,
            surface: 0x121417,
            overlay: 0x1b1e22,
            bubble_me: 0x3a434e,
            bubble_other: 0x1d2127,
            text: 0xedeff1,
            text_2: 0xaab2b9,
            muted: 0x79818a,
            faint: 0x53595f,
            on_accent: 0xffffff,
            on_sidebar: 0x79818a,
            line: 0x262a2f,
            line_strong: 0x333941,
            line_soft: 0x202327,
            accent: 0xe82127,
            accent_hover: 0xe94a4f,
            accent_soft: 0x3c1e23,
            accent_soft_hover: 0x501f23,
            accent_fg: 0xe82127,
            accent_tint: 0xe8212722,
            danger: 0xff6b61,
            danger_soft: 0x3f2a2c,
            danger_fg: 0xff6b61,
            warn: 0xe0a33a,
            warn_soft: 0x3b3326,
            ok: 0x37c26b,
            ok_fg: 0x37c26b,
            link: 0x6e96ff,
            badge: 0xe82127,
            hover: 0x242629,
            selected: 0x232830,
            disabled: 0x42474c,
            avatar: 0x8c2025,
            scrim: 0x00000066,
            shadow: 0x00000014,
            transparent: 0x00000000,
            code_bg: 0x111316,
        },
        layout: Layout::NoRail,
        font: "Inter",
        mono: "JetBrains Mono",
        body: 13.5,
        line_height: 1.65,
        type_scale: 1.0,
        radius_scale: 1.0,
        density: 1.2857,
        dark: true,
        rail_w: 60.0,
        list_w: 328.0,
        preset_tree_w: 248.,
        template_list_w: 340.,
        head_h: 56.0,
    },
        }
    }

    /// 从落盘的 id 认回来。认不出 → `None`（调用方回默认，不报错）。
    pub fn from_id(id: &str) -> Option<Self> {
        ThemeName::ALL.iter().copied().find(|t| t.id() == id)
    }

    /// 主题文件在哪儿（`~/.config/nousdesk/theme.toml`）。
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join(APP_DIR).join(FILE))
    }

    /// 读当前主题。**任何失败都回默认**：没文件、没目录、写坏了、id 不认识。
    pub fn load() -> Self {
        ThemeName::path()
            .map(|path| ThemeName::load_from(&path))
            .unwrap_or_default()
    }

    /// 写到当前主题文件（父目录不存在就建）。
    pub fn save(self) -> Result<PathBuf, String> {
        let path = ThemeName::path().ok_or_else(|| "找不到配置目录".to_string())?;
        self.save_to(&path)?;
        Ok(path)
    }

    // ── 可测的两步（不动环境变量）──────────────────────────

    fn load_from(path: &std::path::Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return ThemeName::default();
        };
        #[derive(serde::Deserialize)]
        struct File {
            theme: Option<String>,
        }
        // 未知字段照收（`toml` 默认忽略），以后加字段不会让老文件读不出来。
        match toml::from_str::<File>(&text) {
            Ok(file) => file
                .theme
                .as_deref()
                .and_then(ThemeName::from_id)
                .unwrap_or_default(),
            Err(_) => ThemeName::default(),
        }
    }

    fn save_to(self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("建不了目录 {}：{e}", dir.display()))?;
        }
        let text = format!("theme = \"{}\"\n", self.id());
        std::fs::write(path, text).map_err(|e| format!("写不了 {}：{e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    // 只取需要的名字：`use super::*` 会把 gpui 的一堆宏再展开一层，
    // 和 #[test] 一起把宏递归深度顶到爆（真踩过，rustc 直接 SIGSEGV）。
    use super::{Layout, Skin, ThemeName};
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nousdesk-theme-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join("theme.toml")
    }

    /// 默认皮的取值不许和出厂常量漂开 —— 接线那天一旦不一致就是满屏跳色。
    #[test]
    fn default_skin_matches_shipped_palette() {
        let p = Skin::DEFAULT.p;
        assert_eq!(p.sidebar, crate::app::SIDEBAR_DARK, "侧栏");
        assert_eq!(p.list_bg, crate::app::LIST_BG, "列表底");
        assert_eq!(p.chat_bg, crate::app::CHAT_BG, "聊天底");
        assert_eq!(p.bubble_me, crate::app::BUBBLE_ME, "我方气泡");
        assert_eq!(p.bubble_other, crate::app::BUBBLE_OTHER, "对方气泡");
        assert_eq!(p.muted, crate::app::MUTED, "次要文字");
        assert_eq!(p.accent, crate::app::WECHAT_GREEN, "主色");
        assert_eq!(Skin::DEFAULT.rail_w, crate::app::ICON_BAR_W);
        assert_eq!(Skin::DEFAULT.list_w, crate::app::SESSION_LIST_W);
        assert_eq!(Skin::DEFAULT.preset_tree_w, crate::app::PRESET_TREE_W);
        assert_eq!(Skin::DEFAULT.template_list_w, crate::app::TEMPLATE_LIST_W);
    }

    /// 默认皮的字号 / 圆角 / 密度倍率必须是 1，否则等于偷偷改了出厂观感。
    #[test]
    fn default_skin_is_pixel_neutral() {
        let t = Skin::DEFAULT;
        assert_eq!(t.type_scale, 1.0);
        assert_eq!(t.radius_scale, 1.0);
        assert_eq!(t.density, 1.0);
        assert_eq!(t.fs(12.).as_f32(), 12.);
        assert_eq!(t.r(8.).as_f32(), 8.);
        assert_eq!(t.d(6.).as_f32(), 6.);
        assert_eq!(t.layout, Layout::Classic);
        // 出厂皮不推组件主题：动 font_size / radius 会让整片文字度量变样。
        assert!(t.is_shipped());
    }

    /// 换皮这条链真的通：把一份改过色 / 字号 / 圆角 / 布局的 Skin 塞进帧内
    /// 快照，`cur()` 立刻能取到，`fs/r/d` 也跟着变。渲染层每个模块都用 `t()`
    /// 取这份值，所以这条测试就是「加皮会真的换界面」的最小证明。
    #[test]
    fn frame_snapshot_drives_every_render_module() {
        let mut dark = Skin::DEFAULT;
        dark.p.chat_bg = 0x171a20;
        dark.p.text = 0xf4f4f4;
        dark.type_scale = 1.15;
        dark.radius_scale = 0.0;
        dark.density = 1.2;
        dark.layout = Layout::NoRail;
        dark.dark = true;

        super::CURRENT.set(dark);
        let got = super::cur();
        assert_eq!(got.p.chat_bg, 0x171a20);
        assert_eq!(got.p.text, 0xf4f4f4);
        assert!((got.fs(12.).as_f32() - 13.8).abs() < 0.001);
        assert_eq!(got.r(8.).as_f32(), 0.0);
        assert!((got.d(6.).as_f32() - 7.2).abs() < 0.001);
        assert_eq!(got.layout, Layout::NoRail);
        assert!(!got.is_shipped(), "换了排版就不该再走「不推组件主题」那条路");

        // 恢复，别污染同进程里别的测试。
        super::CURRENT.set(Skin::DEFAULT);
    }

    /// 每个 id 都要能往返，且 `ALL` 不许有漏登记的名字；每套都得有皮肤。
    #[test]
    fn every_name_round_trips_through_id() {
        for name in ThemeName::ALL {
            assert_eq!(ThemeName::from_id(name.id()), Some(*name), "{}", name.id());
            assert!(!name.label().is_empty(), "{} 没有给人看的名字", name.id());
            let _ = name.skin();
        }
        assert_eq!(ThemeName::from_id("no-such-theme"), None);
    }

    /// 存 → 读回来还是同一个；文件里只有一个键。
    #[test]
    fn save_then_load_round_trips() {
        let path = tmp("round-trip");
        ThemeName::Default.save_to(&path).expect("写得进去");

        let text = std::fs::read_to_string(&path).expect("读得回来");
        assert_eq!(text.trim(), "theme = \"default\"", "文件内容：{text}");
        assert_eq!(ThemeName::load_from(&path), ThemeName::Default);

        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    /// 丢文件 / 写坏 / id 不认识，一律回默认 —— UI 偏好不该把窗口挡在门外。
    #[test]
    fn broken_or_missing_file_falls_back_to_default() {
        let missing = tmp("missing");
        assert_eq!(ThemeName::load_from(&missing), ThemeName::Default);

        let bad = tmp("bad-toml");
        std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
        std::fs::write(&bad, "这不是 toml {{{").unwrap();
        assert_eq!(ThemeName::load_from(&bad), ThemeName::Default);

        let unknown = tmp("unknown-id");
        std::fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        std::fs::write(&unknown, "theme = \"neon-lime\"\n").unwrap();
        assert_eq!(ThemeName::load_from(&unknown), ThemeName::Default);

        // 以后加字段：老文件里多出来的键不许让读回到默认。
        let extra = tmp("extra-keys");
        std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
        std::fs::write(&extra, "theme = \"default\"\nother = 1\n").unwrap();
        assert_eq!(ThemeName::load_from(&extra), ThemeName::Default);

        for p in [&bad, &unknown, &extra] {
            let _ = std::fs::remove_dir_all(p.parent().unwrap().parent().unwrap());
        }
    }
}
