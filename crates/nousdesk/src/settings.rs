//! 左栏底部的「≡ 更多」菜单，和它打开的设置页（占右栏整页）。
//!
//! 菜单只放**真能点**的条目：设置 / 重新连接 / 打开配置文件夹 / 版本信息。
//! 不出「消息通知」「快捷键」「退出登录」这种假入口 —— 没实现的东西摆在那儿，
//! 用户点一次就等于被晃一次。
//!
//! 设置里只有 `host` 一个可编辑项。引擎地址**就这一个**，不再从别处解析。
//! token 走环境变量 / 配置文件 / token 文件，见 README。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin { crate::theme::cur() }

/// Desk#17：设置页「默认模型」这一行的**纯状态**（渲染和点击都读它）。
/// `models` = `GET /v1/models` 的结果；`gate_reason` = capability 给的人话理由原文
/// （来自 `Desktop::endpoint_disabled_reason` 通道，UI 不自己判 code）。
pub(crate) struct DefaultModelRow {
    pub enabled: bool,
    /// 当前默认模型 key `provider/modelId`（`None` = 编目里没指）。
    pub current: Option<String>,
    /// 可选项，同样是 key。空 = 编目空。
    pub items: Vec<String>,
    /// 不可用时给人看的一句人话；可用时 `None`。
    pub reason: Option<String>,
}

pub(crate) fn default_model_row(
    models: Option<&nous_engine::types::Models>,
    gate_reason: Option<String>,
) -> DefaultModelRow {
    if let Some(reason) = gate_reason {
        return DefaultModelRow {
            enabled: false,
            current: None,
            items: vec![],
            reason: Some(reason),
        };
    }
    let Some(models) = models else {
        return DefaultModelRow {
            enabled: false,
            current: None,
            items: vec![],
            reason: Some("没连上引擎，读不到模型编目".into()),
        };
    };
    let items: Vec<String> = models
        .items
        .iter()
        .map(|m| format!("{}/{}", m.provider, m.model_id))
        .collect();
    if items.is_empty() {
        return DefaultModelRow {
            enabled: false,
            current: None,
            items,
            reason: Some("引擎没有模型编目".into()),
        };
    }
    DefaultModelRow {
        enabled: true,
        current: models.default_key(),
        items,
        reason: None,
    }
}

/// 连接页那句查找顺序。Linux 只做 Linux，这里只写用户真正会遇到的路径。
pub(crate) const CONFIG_PATH_HINT: &str =
    "查找顺序：NOUS_DESKTOP_CONFIG → exe 旁 desktop.toml → ~/.config/nousdesk/";

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, Size};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::Desktop;
use crate::config::Config;

/// 关于页的二级菜单。默认「概览」，插件页独立成页（demo 01）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AboutTab {
    Overview,
    Plugins,
}

/// 原 `MUTED = 0x999999`；由 muted 这个语义槽接管。
fn muted() -> u32 { t().p.muted }
/// 原 `FAINT = 0xaaaaaa`；由 faint 这个语义槽接管。
fn faint() -> u32 { t().p.faint }
/// 原 `BAD = 0xd32f2f`；由 danger 这个语义槽接管。
fn bad() -> u32 { t().p.danger }
/// 原 `GOOD = 0x079c50`；由 ok 这个语义槽接管。
fn good() -> u32 { t().p.ok }
/// 原 `RAIL_DARK = 0x2e2e2e`；由 sidebar 这个语义槽接管。
fn rail_dark() -> u32 { t().p.sidebar }

/// 设置页左边那几项。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    Connection,
    /// 一套皮。现在只有「默认」，卡片由 `theme::ThemeName::ALL` 生成。
    Theme,
    /// 哪些事该响。走的是 `sound.rs` 的 `Prefs`，落盘在 `sound.toml`。
    Sound,
    About,
}

impl Desktop {
    // ── 菜单状态 ────────────────────────────────────────────

    pub(crate) fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        cx.notify();
    }

    pub(crate) fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu_open {
            self.menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.archive_open = false;
        self.settings_open = true;
        self.settings_msg = None;
        cx.notify();
    }

    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.settings_msg = None;
        cx.notify();
    }

    // ── 菜单 ────────────────────────────────────────────────

    /// 菜单浮层：一层透明罩子（点空白收起）+ 菜单本体。
    ///
    /// 罩子必须排在菜单**之前**画，GPUI 是后者盖前者。
    pub(crate) fn render_menu_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let scrim = div()
            .id(ElementId::Name("menu-scrim".into()))
            .absolute()
            .inset_0()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_menu(cx)));

        div()
            .absolute()
            .inset_0()
            .child(scrim)
            .child(self.menu_body(cx))
            .into_any_element()
    }

    fn menu_body(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .absolute()
            .left(t().d(58.))
            .bottom(t().d(14.))
            .w(t().d(266.))
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .rounded(t().r(9.))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .shadow_lg()
            .overflow_hidden()
            .child(self.menu_header())
            .child(
                div()
                    .p(t().d(6.))
                    .child(group_label("应用"))
                    .child(menu_row(
                        "menu-settings",
                        IconName::Settings,
                        "设置",
                        true,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.open_settings(cx)),
                    ))
                    .children(
                        self.disabled_reason(crate::capability::Feature::Presets)
                            .map(|reason| {
                                menu_row(
                                    "menu-templates-disabled",
                                    IconName::BookUser,
                                    format!("模板 · {reason}"),
                                    false,
                                    |_, _, _| {},
                                )
                            }),
                    )
                    .children(
                        self.disabled_reason(crate::capability::Feature::Triggers)
                            .map(|reason| {
                                menu_row(
                                    "menu-triggers-disabled",
                                    IconName::Bolt,
                                    format!("触发器 · {reason}"),
                                    false,
                                    |_, _, _| {},
                                )
                            }),
                    )
                    // `needs_credentials` 是四个 code 里唯一要人去做事的：
                    // 给一个真能点的「去设置」，别只写一句人话让人自己找。
                    .children(
                        [
                            crate::capability::Feature::Presets,
                            crate::capability::Feature::Archive,
                            crate::capability::Feature::Combos,
                            crate::capability::Feature::Triggers,
                        ]
                        .into_iter()
                        .any(|f| {
                            self.disabled_code(f)
                                == Some(nous_engine::ReasonCode::NeedsCredentials)
                        })
                        .then(|| {
                            menu_row(
                                "menu-needs-credentials",
                                IconName::Settings,
                                "去设置凭证",
                                true,
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.settings_tab = SettingsTab::Connection;
                                    this.open_settings(cx);
                                }),
                            )
                        }),
                    )
                    .child(menu_row(
                        "menu-archive",
                        IconName::Archive,
                        self.disabled_reason(crate::capability::Feature::Archive)
                            .map(|r| format!("归档库 · {r}"))
                            .unwrap_or_else(|| "归档库".to_string()),
                        self.feature_enabled(crate::capability::Feature::Archive),
                        cx.listener(|this, _: &ClickEvent, _, cx| this.open_archive(cx)),
                    )),
            )
            .child(
                div()
                    .p(t().d(6.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(group_label("连接"))
                    .child(menu_row(
                        "menu-reconnect",
                        IconName::RefreshCw,
                        if self.loading {
                            "连接中…"
                        } else {
                            "重新连接"
                        },
                        !self.loading,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.connect(cx)),
                    ))
                    .child(menu_row(
                        "menu-reveal-config",
                        IconName::FolderOpen,
                        "打开配置文件夹",
                        true,
                        |_, _, _| reveal_config(),
                    )),
            )
            .child(
                div()
                    .p(t().d(6.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(group_label("关于"))
                    .child(info_row("版本", format!("v{}", env!("CARGO_PKG_VERSION"))))
                    .child(info_row(
                        "引擎",
                        self.engine_node.clone().unwrap_or_else(|| "未知".into()),
                    ))
                    .child(info_row(
                        "服务",
                        self.caps
                            .as_ref()
                            .map(|c| c.server.name.clone())
                            .unwrap_or_else(|| "未知".into()),
                    ))
                    .child(info_row(
                        "引擎版本",
                        self.caps
                            .as_ref()
                            .map(|c| c.server.version.clone())
                            .unwrap_or_else(|| "未知".into()),
                    ))
                    .child(info_row(
                        "合同",
                        self.caps
                            .as_ref()
                            .map(|c| format!("v{}", c.contract))
                            .unwrap_or_else(|| "未知".into()),
                    )),
            )
            .into_any_element()
    }

    fn menu_header(&self) -> AnyElement {
        let (text, color) = if self.loading {
            ("连接中…".to_string(), muted())
        } else if self.error.is_some() {
            ("连不上 · 去设置看看".to_string(), bad())
        } else {
            ("已连接".to_string(), good())
        };

        div()
            .flex()
            .items_center()
            .gap(t().d(10.))
            .p(t().d(14.))
            .bg(rgb(t().p.list_bg))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(36.))
                    .rounded(t().r(6.))
                    .bg(rgb(rail_dark()))
                    .text_color(rgb(t().p.on_accent))
                    .child("N"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(2.))
                    .child(div().text_size(t().fs(13.)).child("NousDesk"))
                    .child(div().text_size(t().fs(11.)).text_color(rgb(color)).child(text)),
            )
            .into_any_element()
    }

    // ── 设置页（右栏整页） ──────────────────────────────────

    pub(crate) fn render_settings(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .size_full()
            .bg(rgb(t().p.panel))
            .child(self.settings_header(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(self.settings_nav(cx))
                    .child(self.settings_pane(window, cx)),
            )
            .into_any_element()
    }

    fn settings_header(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap(t().d(10.))
            .h(t().d(48.))
            .flex_none()
            .px(t().d(14.))
            .bg(rgb(t().p.list_bg))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(
                // 返回：回到聊天区。左栏列表一直在，这里只收起右栏这一页。
                div()
                    .id(ElementId::Name("settings-back".into()))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_settings(cx)))
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .px(t().d(6.))
                    .py(t().d(4.))
                    .rounded(t().r(5.))
                    .text_size(t().fs(12.))
                    .text_color(rgb(t().p.text_2))
                    .hover(|el| el.bg(rgb(t().p.line_soft)))
                    .child(Icon::new(IconName::ArrowLeft).with_size(Size::Size(t().d(14.))))
                    .child("返回聊天"),
            )
            .child(div().w(t().d(8.)))
            .child(div().text_size(t().fs(13.)).child("设置"))
            .child(div().flex_1())
            .into_any_element()
    }

    fn settings_nav(&self, cx: &mut Context<Self>) -> AnyElement {
        let tabs = [
            (SettingsTab::Connection, "连接"),
            (SettingsTab::Theme, "主题"),
            (SettingsTab::Sound, "声音"),
            (SettingsTab::About, "关于"),
        ];
        let items: Vec<AnyElement> = tabs
            .into_iter()
            .map(|(tab, label)| {
                let on = self.settings_tab == tab;
                div()
                    .id(ElementId::Name(format!("settings-tab-{label}").into()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.settings_tab = tab;
                        cx.notify();
                    }))
                    .px(t().d(10.))
                    .py(t().d(8.))
                    .rounded(t().r(6.))
                    .text_size(t().fs(12.5))
                    .when(on, |el| {
                        el.bg(rgb(t().p.accent_soft))
                            .text_color(rgb(t().p.accent))
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!on, |el| {
                        el.text_color(rgb(t().p.text_2)).hover(|h| h.bg(rgb(t().p.line_soft)))
                    })
                    .child(label)
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(t().d(2.))
            .w(t().d(170.))
            .flex_none()
            .p(t().d(10.))
            .bg(rgb(t().p.panel_alt))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .children(items)
            .into_any_element()
    }

    fn settings_pane(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        // 窄窗（半屏）时右栏只剩一百多像素：内边距收一收，别让「卡片最小宽 + 内边距」
        // 又把页面撑出窗口（坑.md 57）。
        let narrow = window.viewport_size().width.as_f32() < 900.;
        let pad = if narrow { t().d(14.) } else { t().d(22.) };
        let body: AnyElement = match self.settings_tab {
            SettingsTab::Connection => self.connection_pane(cx),
            SettingsTab::Theme => self.theme_pane(cx),
            SettingsTab::Sound => self.sound_pane(cx),
            SettingsTab::About => self.about_pane(narrow, cx),
        };

        div()
            // `on_hover` 挂在 `Stateful` 上：先给 id。
            .id(ElementId::Name("settings-viewport".into()))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(t().d(0.))
            // 鼠标在这块内容上 → 滚动条出现（和会话列表/聊天区同一套：平时一条线都没有）。
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.bar_hover(crate::app::Bar::Settings, *hovered, cx)
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .min_w(t().d(0.))
                    .id(ElementId::Name("settings-scroll".into()))
                    .overflow_y_scroll()
                    .track_scroll(&self.settings_scroll)
                    .p(pad)
                    .child(body),
            )
            .child(crate::app::scrollbar::overlay(
                "settings-scrollbar",
                &self.settings_scroll,
                self.bar(crate::app::Bar::Settings).0,
                self.bar(crate::app::Bar::Settings).1,
            ))
            .into_any_element()
    }

    fn connection_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let target = Config::save_target()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|e| e);

        let msg = self.message_view(crate::theme::cur());

        div()
            .flex()
            .flex_col()
            .gap(t().d(14.))
            .min_w(t().d(0.))
            .max_w(t().d(460.))
            .child(div().text_size(t().fs(15.)).child("连接"))
            .child(
                div()
                    .text_size(t().fs(11.5))
                    .text_color(rgb(muted()))
                    .child("改完保存会写到下面那个文件，然后重新连一次。"),
            )
            .child(field("引擎地址 host", Input::new(&self.host_input)))
            .child(readonly(
                "当前连接",
                self.engine_url
                    .clone()
                    .unwrap_or_else(|| "未连接 —— 保存后会重连".into()),
                "一个地址 + /v1/capabilities，不再从别处解析。",
            ))
            .child(readonly(
                "配置文件（保存写这里）",
                target,
                CONFIG_PATH_HINT,
            ))
            .child(self.default_model_row(cx))
            .children(msg)
            .child(
                div()
                    .flex()
                    .gap(t().d(8.))
                    .child(pill(
                        "settings-save",
                        "保存并重连",
                        true,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.save_settings(cx)),
                    ))
                    .child(pill(
                        "settings-reconnect",
                        if self.loading {
                            "连接中…"
                        } else {
                            "重新连接"
                        },
                        false,
                        cx.listener(|this, _: &ClickEvent, _, cx| this.connect(cx)),
                    )),
            )
            .into_any_element()
    }

    /// 设置页「引擎」这一节里的「默认模型」行。
    /// 有编目 → 一颗下拉按钮列 `models.items`，选中即 `PUT /v1/default`；
    /// 编目空 / 能力灰 / 没连引擎 → 整行禁用 + 人话理由（只消费 `endpoint_disabled_reason`）。
    fn default_model_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let gate_reason = self.endpoint_disabled_reason("PUT", "/v1/default");
        let row = default_model_row(self.default_models.as_ref(), gate_reason);
        let button: AnyElement = if row.enabled {
            let weak = cx.weak_entity();
            let items = row.items.clone();
            Button::new("settings-default-model")
                .small()
                .label(row.current.clone().unwrap_or_else(|| "选择默认模型".into()))
                .dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, _window, _cx| {
                    let mut menu = menu.min_w(t().d(220.));
                    for key in &items {
                        let key = key.clone();
                        let weak = weak.clone();
                        menu = menu.item(PopupMenuItem::new(key.clone()).on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| this.set_default_model(key.clone(), cx));
                        }));
                    }
                    menu
                })
                .into_any_element()
        } else {
            Button::new("settings-default-model-disabled")
                .small()
                .label(row.current.clone().unwrap_or_else(|| "默认模型不可用".into()))
                .disabled(true)
                .into_any_element()
        };
        let note = row.reason.unwrap_or_else(|| "改动立刻写回引擎编目，重启 Desk 仍然生效。".into());
        div()
            .flex()
            .flex_col()
            .gap(t().d(5.))
            .w_full()
            .min_w(t().d(0.))
            .child(
                div()
                    .text_size(t().fs(11.5))
                    .text_color(rgb(t().p.muted))
                    .child("默认模型"),
            )
            .child(button)
            .child(div().text_size(t().fs(10.5)).text_color(rgb(faint())).child(note))
            .into_any_element()
    }

    /// 设置页底部那行反馈。连接页和主题页都要显示，所以抽出来。
    fn message_view(&self, t: crate::theme::Skin) -> Option<AnyElement> {
        self.settings_msg.as_ref().map(|(text, bad)| {
            div()
                .text_size(t.fs(11.5))
                .text_color(rgb(if *bad { t.p.danger } else { t.p.ok }))
                .child(text.clone())
                .into_any_element()
        })
    }

    /// 「主题」页：把 `ThemeName::ALL` 铺成卡片。加一套皮只改 `theme.rs`，这里不动。
    fn theme_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = crate::theme::cur();
        let cards: Vec<AnyElement> = crate::theme::ThemeName::ALL
            .iter()
            .copied()
            .map(|name| self.theme_card(name, t, cx))
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(t.d(14.))
            .min_w(t.d(0.))
            .max_w(t.d(600.))
            .child(div().text_size(t.fs(15.)).child("主题"))
            .child(
                div()
                    .text_size(t.fs(11.5))
                    .text_color(rgb(t.p.muted))
                    .child("一套主题就是一整套配色与排版。点一下即生效；新皮做好会长在下面。"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .min_w(t.d(0.))
                    .gap(t.d(12.))
                    .children(cards),
            )
            .children(self.message_view(t))
            .child(
                div()
                    .text_size(t.fs(10.5))
                    .text_color(rgb(t.p.faint))
                    .child("主题存在 ~/.config/nousdesk/theme.toml，不写进 desktop.toml。"),
            )
            .into_any_element()
    }

    fn theme_card(
        &self,
        name: crate::theme::ThemeName,
        skin: crate::theme::Skin,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let card = name.skin();
        let on = self.theme_name == name;

        div()
            .id(ElementId::Name(format!("theme-card-{}", name.id()).into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_theme(name, cx)))
            .flex()
            .flex_col()
            .gap(skin.d(8.))
            // 理想宽 196，放不下就缩（最窄 88）。**不能写成 `flex_1`**：那会让一排塞
            // 满六七张、每张只剩八九十宽，宽窗下反而更难挑（第一版就是这么错的）。
            .w(skin.d(196.))
            .flex_shrink(1.)
            .min_w(skin.d(88.))
            .p(skin.d(10.))
            .rounded(skin.r(8.))
            .bg(rgb(skin.p.panel))
            .border(skin.d(2.))
            .border_color(rgb(if on { card.p.accent } else { skin.p.line_soft }))
            .hover(move |el| {
                el.border_color(rgb(if on { card.p.accent } else { skin.p.line_strong }))
            })
            .child(theme_thumb(card, skin))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(skin.d(6.))
                    .child(div().text_size(skin.fs(12.)).child(name.label()))
                    .child(div().flex_1())
                    .children(if on {
                        Some(
                            div()
                                .text_size(skin.fs(10.5))
                                .text_color(rgb(card.p.ok))
                                .child("使用中"),
                        )
                    } else {
                        None
                    }),
            )
            .into_any_element()
    }

    /// 选一套皮：落盘 → 换 Global（会刷新所有窗口）。
    /// 写不进去就只报错、不换 —— 否则重启又跳回去，比不换更糟。
    pub(crate) fn pick_theme(&mut self, name: crate::theme::ThemeName, cx: &mut Context<Self>) {
        match name.save() {
            Ok(path) => {
                self.theme_name = name;
                crate::theme::ActiveSkin::set(cx, name);
                self.settings_msg = Some((
                    format!("主题：{}（写入 {}）", name.label(), path.display()),
                    false,
                ));
            }
            Err(e) => self.settings_msg = Some((e, true)),
        }
        cx.notify();
    }

    /// 声音页。版面照 `demo/sound-settings` 选中的 **02「两组分块」**：
    /// **提醒**（你不在看的时候叫你）和**手感**（你正在操作时的回执）是两个浅底卡片。
    ///
    /// 两组分开摆不是为了好看 —— 它们的判据本来就不同：提醒要判焦点（前台不响），
    /// 手感永远响（手刚离开回车）。合成一个开关会让这两种意思糊成一团。
    fn sound_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = crate::theme::cur();
        let p = self.sound;

        let path = crate::sound::Prefs::path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "（拿不到配置目录）".into());

        div()
            .flex()
            .flex_col()
            .gap(t.d(14.))
            .min_w(t.d(0.))
            .max_w(t.d(520.))
            .child(div().text_size(t.fs(15.)).child("声音"))
            .child(div().text_size(t.fs(11.5)).text_color(rgb(t.p.muted)).child(
                "屏幕上的事归屏幕，屏幕外的事归这一声。手机端不跟着响 —— 这是桌面自己的偏好。",
            ))
            // 总开关：单独一块，它管的是全部
            .child(sound_card(vec![sound_row(
                t,
                "sound-master",
                "声音提示",
                "关掉之后一个都不响。未读红点、列表顺序照旧。",
                // 总开关自己不是一种声音：试听给的是最常用的那一声。
                crate::sound::Cue::Done,
                Some(p.master),
                cx.listener(|this, on: &bool, _w, cx| {
                    let mut next = this.sound;
                    next.master = *on;
                    this.set_sound(next, cx);
                }),
            )]))
            .child(sound_section_label(t, "提醒 · 你没在看的时候"))
            .child(sound_card(vec![
                sound_row(
                    t,
                    "sound-done",
                    "一轮生成完成",
                    "会话跑完了，而窗口不在前台。",
                    crate::sound::Cue::Done,
                    Some(p.done),
                    cx.listener(|this, on: &bool, _w, cx| {
                        let mut next = this.sound;
                        next.set(crate::sound::Cue::Done, *on);
                        this.set_sound(next, cx);
                    }),
                ),
                sound_row(
                    t,
                    "sound-error",
                    "生成出错",
                    "模型报错、工具失败、引擎断线 —— 不用分开，都算「白等了」。",
                    crate::sound::Cue::Error,
                    Some(p.error),
                    cx.listener(|this, on: &bool, _w, cx| {
                        let mut next = this.sound;
                        next.set(crate::sound::Cue::Error, *on);
                        this.set_sound(next, cx);
                    }),
                ),
                // 这一行照 demo 留着，但**不画开关**：引擎没有「等你拍板」这个信号，
                // 摆一个点得动的开关等于骗人。有信号了再把它换成 `sound_row`。
                sound_row_pending(
                    t,
                    "轮到你拍板",
                    "工具等你批准、问题等你回答。引擎目前没有这个信号，所以没有开关。",
                ),
            ]))
            .child(sound_section_label(t, "手感 · 你在操作的时候"))
            .child(sound_card(vec![
                sound_row(
                    t,
                    "sound-send",
                    "发送一条消息",
                    "很短的一声，和气泡出现同时。",
                    crate::sound::Cue::Send,
                    Some(p.send),
                    cx.listener(|this, on: &bool, _w, cx| {
                        let mut next = this.sound;
                        next.set(crate::sound::Cue::Send, *on);
                        this.set_sound(next, cx);
                    }),
                ),
                sound_row(
                    t,
                    "sound-stop",
                    "停止生成",
                    "一声往下落的「落定」。默认关 —— 停止是少做事，不需要回执。",
                    crate::sound::Cue::Stop,
                    Some(p.stop),
                    cx.listener(|this, on: &bool, _w, cx| {
                        let mut next = this.sound;
                        next.set(crate::sound::Cue::Stop, *on);
                        this.set_sound(next, cx);
                    }),
                ),
            ]))
            .child(sound_note(t))
            .children(self.message_view(t))
            .child(
                div()
                    .text_size(t.fs(10.5))
                    .text_color(rgb(t.p.faint))
                    .child(format!("声音存在 {path}，不写进 desktop.toml。")),
            )
            .into_any_element()
    }

    /// 改一份声音偏好：**先落盘，成功了才改内存**。
    ///
    /// 和 `pick_theme` 一个规矩：写不进去就报错、开关弹回原位 —— 装成功重启又跳回去，
    /// 比不改更让人恼火。
    pub(crate) fn set_sound(&mut self, next: crate::sound::Prefs, cx: &mut Context<Self>) {
        match next.save() {
            Ok(_) => {
                self.sound = next;
                self.settings_msg = None;
            }
            Err(e) => self.settings_msg = Some((format!("声音设置没写进去：{e}"), true)),
        }
        cx.notify();
    }

    /// 关于页：顶部二级 segmented + 概览 / 插件两页（demo 01）。
    fn about_pane(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let t = t();
        let module_count = self.caps.as_ref().map(|c| c.modules.len()).unwrap_or(0);
        let head = div()
            .flex()
            .items_center()
            .gap(t.d(12.))
            .pb(t.d(2.))
            .child(div().text_size(t.fs(15.)).child("关于"))
            .child({
                let tabs = [
                    (AboutTab::Overview, "概览".to_string()),
                    (AboutTab::Plugins, format!("插件 {module_count}")),
                ];
                let mut row = div()
                    .flex()
                    .items_center()
                    .p(t.d(2.))
                    .gap(t.d(2.))
                    .rounded(t.r(6.))
                    .bg(rgb(t.p.surface))
                    .border(t.d(1.))
                    .border_color(rgb(t.p.line_soft));
                for (tab, label) in tabs {
                    let on = self.about_tab == tab;
                    row = row.child(
                        div()
                            .id(ElementId::Name(format!("about-tab-{tab:?}").into()))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.about_tab = tab;
                                cx.notify();
                            }))
                            .px(t.d(10.))
                            .py(t.d(4.))
                            .rounded(t.r(5.))
                            .text_size(t.fs(11.5))
                            .when(on, |el| {
                                el.bg(rgb(t.p.accent_soft))
                                    .text_color(rgb(t.p.accent_fg))
                                    .font_weight(FontWeight::SEMIBOLD)
                            })
                            .when(!on, |el| {
                                el.text_color(rgb(t.p.text_2))
                                    .hover(|h| h.bg(rgb(t.p.line_soft)))
                            })
                            .child(label),
                    );
                }
                row
            });

        let body = match self.about_tab {
            AboutTab::Overview => self.about_overview(),
            AboutTab::Plugins => self.about_plugins(narrow, cx),
        };
        div()
            .flex()
            .flex_col()
            .gap(t.d(14.))
            .min_w(t.d(0.))
            .child(head)
            .child(body)
            .into_any_element()
    }

    /// 概览页：概览保留现有键值，外加 provider（caps.runtime.provider）。
    /// `services` 只在 extra 里，按票面不做、不在 UI 猜。
    fn about_overview(&self) -> AnyElement {
        let t = t();
        let log_path = dirs::data_local_dir()
            .map(|d| {
                d.join(crate::config::APP_DIR)
                    .join("desktop.log")
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| "（拿不到日志目录）".into());
        let cfg_path = Config::path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "（没有配置文件，用的是默认值）".to_string());
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let facts = device_facts(self.device.as_ref(), now_ms as i128);
        let unknown = || "未知".to_string();
        let service = self
            .caps
            .as_ref()
            .map(|c| c.server.version.clone())
            .unwrap_or_else(unknown);
        let provider = self
            .caps
            .as_ref()
            .map(|c| {
                let p = &c.runtime.provider;
                if p.message.is_empty() {
                    p.kind.clone()
                } else {
                    format!("{} · {}", p.kind, p.message)
                }
            })
            .unwrap_or_else(|| "未知".into());

        div()
            .flex()
            .flex_col()
            .gap(t.d(14.))
            .min_w(t.d(0.))
            .max_w(t.d(520.))
            .child(
                div()
                    .text_size(t.fs(11.5))
                    .text_color(rgb(muted()))
                    .child("引擎信息来自 GET /v1/device —— 出问题先对版本和构建号。"),
            )
            .child(kv("客户端", format!("v{}", env!("CARGO_PKG_VERSION"))))
            .child(kv("引擎（node）", facts.node.clone().unwrap_or_else(unknown)))
            .child(kv("引擎版本", service))
            .child(kv("合同", format!("v{}", crate::capability::CLIENT_CONTRACT)))
            .child(kv("provider", provider))
            .child(kv("构建（build）", facts.build.clone().unwrap_or_else(unknown)))
            .child(kv("目标（target）", facts.target.clone().unwrap_or_else(unknown)))
            .child(kv("已运行", facts.started_at.clone().unwrap_or_else(unknown)))
            .child(kv("默认模型", facts.model.clone().unwrap_or_else(unknown)))
            .child(kv("可用模型", format!("{} 个", facts.available)))
            .child(kv(
                "数据来源",
                match facts.faux {
                    Some(true) => "演示数据（faux）".into(),
                    Some(false) => "真实引擎".into(),
                    None => unknown(),
                },
            ))
            .child(kv(
                "引擎地址",
                self.engine_url.clone().unwrap_or_else(|| "未连接".into()),
            ))
            .child(kv("配置文件", cfg_path))
            .child(kv("日志", log_path))
            .into_any_element()
    }

    /// 插件页：左列表 + 右详情；窄窗退化成互斥的两屏（详情顶部「← 插件」）。
    fn about_plugins(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        let t = t();
        let Some(caps) = self.caps.as_ref() else {
            return div()
                .text_size(t.fs(11.5))
                .text_color(rgb(muted()))
                .child("没有 capabilities —— 先到「连接」页连上引擎。")
                .into_any_element();
        };

        // 选中项：未连上 / 模块被删掉时回落到第一条。
        let selected = self
            .about_selected
            .as_deref()
            .and_then(|name| caps.modules.iter().position(|m| m.name == name))
            .unwrap_or(0);
        let show_detail = !narrow || self.about_detail_open;

        let mut list = div().flex().flex_col().gap(t.d(2.));
        if narrow && show_detail {
            // 窄窗详情自己带「← 插件」，列表收起。
        } else {
            for (i, module) in caps.modules.iter().enumerate() {
                list = list.child(plugin_row(module, &caps.server.version, i == selected, cx));
            }
        }
        let list = div()
            .flex()
            .flex_col()
            .gap(t.d(10.))
            .min_w(t.d(0.))
            .when(!narrow, |el| el.w(t.d(268.)))
            .child(div().text_size(t.fs(11.5)).text_color(rgb(muted())).child(format!(
                "{} 个模块 · {} 启用 · {} 未启用 · {} 端点",
                caps.modules.len(),
                caps.modules.iter().filter(|m| m.enabled).count(),
                caps.modules.iter().filter(|m| !m.enabled).count(),
                caps.modules.iter().map(|m| m.endpoints.len()).sum::<usize>(),
            )))
            .child(list);

        if !show_detail {
            return list.into_any_element();
        }
        let module = &caps.modules[selected];
        let detail = self.about_plugin_detail(module, caps, narrow, cx);
        if narrow {
            detail
        } else {
            div()
                .flex()
                .gap(t.d(18.))
                .min_w(t.d(0.))
                .child(list)
                .child(
                    div()
                        .flex_1()
                        .min_w(t.d(0.))
                        .border_l(t.d(1.))
                        .border_color(rgb(t.p.line_soft))
                        .pl(t.d(18.))
                        .child(detail),
                )
                .into_any_element()
        }
    }

    /// 单个模块详情（demo 01 右栏）。缺省字段按票面降级：
    /// 版本 = caps.server.version，类型 / 层 = —，crate / 说明 = —。
    fn about_plugin_detail(
        &self,
        module: &nous_engine::Module,
        caps: &nous_engine::Capabilities,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = t();
        let unknown = "—";
        let version = module.version.as_deref().unwrap_or(&caps.server.version);
        let kind = module.kind.as_deref().unwrap_or(unknown);
        let layer = module.layer.as_deref().unwrap_or(unknown);
        let required = match module.required {
            Some(true) => "是",
            Some(false) => "否",
            None => unknown,
        };
        let crate_name = module.crate_name.as_deref().unwrap_or(unknown);
        let description = module.description.as_deref().unwrap_or(unknown);
        let registered = crate::capability::module_feature(module.name.as_str());
        let label = registered
            .map(|f| f.label().to_string())
            .unwrap_or_else(|| module.name.clone());

        // 状态文案只走 disabled_text()：不在这里写第二套 if enabled。
        let reason = registered
            .and_then(|f| self.feature_gate(f))
            .map(|g| match g {
                crate::capability::Gate::Enabled => None,
                crate::capability::Gate::Disabled { code, reason } => {
                    Some(crate::capability::disabled_text(code, &reason.message))
                }
                crate::capability::Gate::Unknown => Some(crate::capability::disabled_text(
                    nous_engine::ReasonCode::Unknown,
                    "服务端未声明这个功能",
                )),
            })
            .unwrap_or_else(|| {
                if module.enabled {
                    None
                } else {
                    module.reason.as_ref().map(|r| {
                        crate::capability::disabled_text(r.code, &r.message)
                    })
                }
            });
        let state = if module.enabled { "启用" } else { "未启用" };

        let mut detail = div()
            .flex()
            .flex_col()
            .gap(t.d(10.))
            .min_w(t.d(0.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.d(4.))
                    .child(div().text_size(t.fs(15.)).child(label))
                    .child(
                        div()
                            .text_size(t.fs(10.5))
                            .text_color(rgb(muted()))
                            .child(module.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(t.d(8.))
                    .child(div().text_size(t.fs(11.5)).text_color(rgb(muted())).child(state))
                    .child(
                        div()
                            .text_size(t.fs(11.5))
                            .text_color(rgb(muted()))
                            .child(format!("v{version} · 合同 v{}", caps.contract)),
                    ),
            );
        if let Some(reason) = reason {
            detail = detail.child(
                div()
                    .p(t.d(8.))
                    .rounded(t.r(5.))
                    .bg(rgb(t.p.warn_soft))
                    .text_size(t.fs(11.))
                    .text_color(rgb(t.p.text_2))
                    .child(reason),
            );
        }
        detail = detail
            .child(plugin_meta("类型", kind.to_string()))
            .child(plugin_meta("层", layer.to_string()))
            .child(plugin_meta("必需", required.to_string()))
            .child(plugin_meta("来源 crate", crate_name.to_string()))
            .child(plugin_meta(
                "客户端登记",
                match registered {
                    Some(_) => "已登记 · capability.rs".to_string(),
                    None => "未登记".to_string(),
                },
            ))
            .child(
                div()
                    .text_size(t.fs(10.5))
                    .text_color(rgb(faint()))
                    .child(description.to_string()),
            );

        let mut eps = div().flex().flex_col().gap(t.d(1.));
        eps = eps.child(
            div()
                .text_size(t.fs(10.))
                .text_color(rgb(faint()))
                .child(format!("端点 · {}", module.endpoints.len())),
        );
        for endpoint in &module.endpoints {
            let (method, path) = crate::capability::split_endpoint(endpoint)
                .unwrap_or((endpoint.as_str(), ""));
            eps = eps.child(
                div()
                    .flex()
                    .gap(t.d(8.))
                    .text_size(t.fs(10.5))
                    .child(
                        div()
                            .w(t.d(38.))
                            .flex_none()
                            .text_color(rgb(if method == "POST" { t.p.accent_fg } else { muted() }))
                            .child(method.to_string()),
                    )
                    .child(div().text_color(rgb(t.p.text_2)).child(path.to_string())),
            );
        }
        detail = detail.child(eps);

        if narrow {
            div()
                .flex()
                .flex_col()
                .gap(t.d(10.))
                .child(
                    div()
                        .id(ElementId::Name("about-plugin-back".into()))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.about_detail_open = false;
                            cx.notify();
                        }))
                        .px(t.d(6.))
                        .py(t.d(4.))
                        .text_size(t.fs(12.))
                        .text_color(rgb(t.p.text_2))
                        .hover(|h| h.bg(rgb(t.p.line_soft)))
                        .child("← 插件"),
                )
                .child(detail)
                .into_any_element()
        } else {
            detail.into_any_element()
        }
    }

    // ── 保存 ────────────────────────────────────────────────

    /// 把输入框里的 host 写回配置文件，然后重连。
    ///
    /// 校验只挡「一定错」的：空值、host 不带协议。剩下的交给连接时报 ——
    /// 客户端猜地址对不对就等于有了第二个真相。
    pub(crate) fn save_settings(&mut self, cx: &mut Context<Self>) {
        let host = self.host_input.read(cx).value().trim().to_string();

        let bad = |msg: String| Some((msg, true));
        let problem = if host.is_empty() {
            bad("host 不能为空".into())
        } else if !(host.starts_with("http://") || host.starts_with("https://")) {
            bad("host 要以 http:// 或 https:// 开头".into())
        } else {
            None
        };
        if let Some(msg) = problem {
            self.settings_msg = Some(msg);
            cx.notify();
            return;
        }

        let next = Config { host, token: self.conn.token.clone() };
        match next.save() {
            Ok(path) => {
                self.conn = next;
                self.settings_msg = Some((format!("已写入 {}", path.display()), false));
                self.connect(cx);
            }
            Err(e) => {
                self.settings_msg = Some((e, true));
                cx.notify();
            }
        }
    }
}

// ── 小件 ────────────────────────────────────────────────────

// ── 声音页的小件 ────────────────────────────────────────────

/// 组标签（「提醒 · 你没在看的时候」这种）。10.5px + 淡色 + 一点字距，
/// 靠字号和留白分区，不靠色块 —— 这是皮里的规矩。
fn sound_section_label(t: crate::theme::Skin, text: &'static str) -> AnyElement {
    div()
        .mt(t.d(6.))
        .px(t.d(2.))
        .text_size(t.fs(10.5))
        .text_color(rgb(t.p.faint))
        .child(text)
        .into_any_element()
}

/// 一组浅底卡片。里面几行由调用方给，行间的发丝线由 `sound_row` 自己画
/// （最后一行不画 —— 卡片内边距下面紧跟一条线会像「还有内容」）。
fn sound_card(rows: Vec<AnyElement>) -> AnyElement {
    let t = t();
    // 行间的发丝线由这里插，不让每一行自己判断"我是不是最后一行" ——
    // 那要多一个参数，而且加一行就得改上一行。
    let mut body: Vec<AnyElement> = Vec::with_capacity(rows.len() * 2);
    for (i, row) in rows.into_iter().enumerate() {
        if i > 0 {
            body.push(
                div()
                    .flex_none()
                    .h(t.d(1.))
                    .w_full()
                    .bg(rgb(t.p.line_soft))
                    .into_any_element(),
            );
        }
        body.push(row);
    }
    div()
        .flex()
        .flex_col()
        .px(t.d(12.))
        .py(t.d(2.))
        .rounded(t.r(8.))
        .bg(rgb(t.p.surface))
        .children(body)
        .into_any_element()
}

/// 一行：名字 + 说明 + [试听][开关]。
///
/// `checked == None` 表示**引擎还没有这个信号** —— 那时不画开关，只把行压成灰的。
/// 摆一个点得动的开关等于骗人（`settings.rs` 开头那条规矩）。
fn sound_row(
    t: crate::theme::Skin,
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    cue: crate::sound::Cue,
    checked: Option<bool>,
    on_switch: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t.d(14.))
        .py(t.d(9.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(t.d(0.))
                .child(div().text_size(t.fs(13.)).child(name))
                .child(
                    div()
                        .mt(t.d(2.))
                        .text_size(t.fs(11.5))
                        .text_color(rgb(t.p.muted))
                        .child(desc),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(t.d(10.))
                .flex_none()
                .child(try_button(t, id, cue))
                .child(sound_switch(
                    t,
                    id,
                    checked.unwrap_or(false),
                    // 点一下 = 取反。"变成什么值"只在这里算一次，调用方那五处
                    // 全都写的是 `set(cue, *on)`，不用各自记得取反。
                    move |_ev, w, cx| on_switch(&!checked.unwrap_or(false), w, cx),
                )),
        )
        .into_any_element()
}

/// 开关。**自己搭，不用组件库的 `Switch`。**
///
/// 组件那个取的是 `theme()` 里的 `tokens.primary`，而 `theme.rs` 只推了
/// `colors.primary`（实测出来是黑的）；它的圆角又直接吃 `theme.radius`，
/// 换到「单色」这种直角皮上整个控件会退化成两个方块。这个项目的皮本来就是
/// div 手搭的（`pill` / `nav_toggle` 都是），开关也照这个来：颜色和圆角都从
/// 当前皮拿，没有第二种真相。
///
/// 不做动画：状态变化立刻可见就够（皮的规矩是"短、直、有目的"）。
fn sound_switch(
    t: crate::theme::Skin,
    id: &'static str,
    on: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let track_h = t.d(20.);
    let knob = t.d(16.);
    div()
        .id(ElementId::Name(format!("{id}-sw").into()))
        .on_click(handler)
        .flex()
        .items_center()
        .flex_none()
        .w(t.d(36.))
        .h(track_h)
        .p(t.d(2.))
        .rounded(t.r(10.))
        .bg(rgb(if on { t.p.accent } else { t.p.disabled }))
        .cursor_pointer()
        .when(on, |el| el.justify_end())
        .hover(|el| el.opacity(0.85))
        .child(
            div()
                .flex_none()
                .w(knob)
                .h(knob)
                .rounded(t.r(8.))
                .bg(rgb(t.p.panel)),
        )
        .into_any_element()
}

/// 「引擎还没有这个信号」的那一行：不画开关、不画试听，整行压灰。
fn sound_row_pending(t: crate::theme::Skin, name: &'static str, desc: &'static str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t.d(14.))
        .py(t.d(9.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(t.d(0.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.d(8.))
                        .child(
                            div()
                                .text_size(t.fs(13.))
                                .text_color(rgb(t.p.faint))
                                .child(name),
                        )
                        .child(
                            div()
                                .px(t.d(5.))
                                .py(t.d(1.))
                                .rounded(t.r(3.))
                                .border(t.d(1.))
                                .border_color(rgb(t.p.line))
                                .text_size(t.fs(10.))
                                .text_color(rgb(t.p.faint))
                                .child("无此信号"),
                        ),
                )
                .child(
                    div()
                        .mt(t.d(2.))
                        .text_size(t.fs(11.5))
                        .text_color(rgb(t.p.faint))
                        .child(desc),
                ),
        )
        .into_any_element()
}

/// 「试听」。**穿过偏好**：这一行现在是关的也能听 —— 点它的人就是想知道那是什么声。
fn try_button(t: crate::theme::Skin, id: &'static str, cue: crate::sound::Cue) -> AnyElement {
    div()
        .id(ElementId::Name(format!("sound-try-{id}").into()))
        .on_click(move |_: &ClickEvent, _w, _cx| crate::sound::preview(cue))
        .flex()
        .items_center()
        .gap(t.d(5.))
        .h(t.d(24.))
        .px(t.d(9.))
        .rounded(t.r(5.))
        .border(t.d(1.))
        .border_color(rgb(t.p.line_strong))
        .text_size(t.fs(11.5))
        .text_color(rgb(t.p.text_2))
        .hover(move |el| {
            el.bg(rgb(t.p.accent_soft))
                .border_color(rgb(t.p.accent))
                .text_color(rgb(t.p.accent_fg))
        })
        .child("试听")
        .child(Icon::new(IconName::Play).with_size(Size::Size(t.d(10.))))
        .into_any_element()
}

/// 页脚那条"这不是开关，是规则"。写死在版面上，不给开关 —— 少一个设置项就是赚。
fn sound_note(t: crate::theme::Skin) -> AnyElement {
    div()
        .flex()
        .gap(t.d(8.))
        .pt(t.d(2.))
        .child(
            Icon::new(IconName::Info)
                .size(t.d(13.))
                .text_color(rgb(t.p.faint)),
        )
        .child(
            div()
                .flex_1()
                // 少了这一行，长句不换行、直接顶出卡片右边 —— flex 子项的 min-width
                // 默认是内容宽，不是零（`settings_pane` 里那些 `min_w(0)` 是同一个坑）。
                .min_w(t.d(0.))
                .text_size(t.fs(11.5))
                .text_color(rgb(t.p.muted))
                .child(
                    // 不能写成 `\` 续行：续行只吃掉换行，**后面那行开头的缩进会原样
                    // 进字符串**，界面上就会看到一大段空白。宁可这一行长着，也别断。
                    concat!(
                        "完成和出错只在窗口不在前台时响：你正看着它跑完，视觉反馈就够了。",
                        "发送和停止是你刚动过手，永远响。同一件事两秒内不重复，音量跟系统走。",
                    ),
                ),
        )
        .into_any_element()
}

/// 设置页的按钮。不用 `component::button::Button` —— 它的 variant 会把自定义底色
/// 压成淡色（实测 `#07c160` 出来是 20% 透明的一层淡绿），而这个项目的皮本来
/// 就是 div 手搭的，颜色自己拿着更省事。
/// 主题卡片里那张缩略图：最左图标条 + 会话列表 + 聊天区两个气泡。
///
/// 全部颜色从 `Theme` 取 —— 这是主题色目前唯一真正被用到的地方（渲染层还没接），
/// 所以加一套皮时，这张图就是第一眼能看出区别的东西。
fn theme_thumb(card: crate::theme::Skin, t: crate::theme::Skin) -> AnyElement {
    let p = card.p;
    div()
        .flex()
        .w_full()
        .h(t.d(84.))
        .rounded(t.r(4.))
        .overflow_hidden()
        .child(div().w(t.d(14.)).h_full().bg(rgb(p.sidebar)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t.d(3.))
                .w(t.d(52.))
                .h_full()
                .p(t.d(5.))
                .bg(rgb(p.list_bg))
                .child(div().h(t.d(4.)).w_full().rounded(t.r(2.)).bg(rgb(p.line)))
                .child(div().h(t.d(4.)).w_full().rounded(t.r(2.)).bg(rgb(p.line)))
                .child(div().h(t.d(4.)).w(t.d(28.)).rounded(t.r(2.)).bg(rgb(p.accent))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t.d(5.))
                .flex_1()
                .h_full()
                .p(t.d(6.))
                .bg(rgb(p.chat_bg))
                .child(
                    div()
                        .h(t.d(10.))
                        .w(t.d(36.))
                        .rounded(t.r(4.))
                        .bg(rgb(p.bubble_other)),
                )
                .child(
                    div()
                        .h(t.d(10.))
                        .w(t.d(30.))
                        .rounded(t.r(4.))
                        .bg(rgb(p.bubble_me)),
                ),
        )
        .into_any_element()
}

fn pill(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let (bg, fg, hover) = if primary {
        (t().p.accent, t().p.panel, t().p.accent_hover)
    } else {
        (t().p.hover, t().p.text_2, t().p.line_soft)
    };
    div()
        .id(ElementId::Name(id.into()))
        .on_click(handler)
        .flex()
        .items_center()
        .justify_center()
        .h(t().d(30.))
        .px(t().d(14.))
        .rounded(t().r(5.))
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .text_size(t().fs(12.))
        .hover(move |el| el.bg(rgb(hover)))
        .child(label.into())
        .into_any_element()
}

fn group_label(text: &'static str) -> AnyElement {
    div()
        .px(t().d(10.))
        .pt(t().d(4.))
        .pb(t().d(5.))
        .text_size(t().fs(10.))
        .text_color(rgb(faint()))
        .child(text)
        .into_any_element()
}

fn menu_row(
    id: &'static str,
    icon: IconName,
    label: impl Into<SharedString>,
    enabled: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id(ElementId::Name(id.into()))
        .when(enabled, |el| el.on_click(handler))
        .when(enabled, |el| el.hover(|h| h.bg(rgb(t().p.hover))))
        .flex()
        .items_center()
        .gap(t().d(10.))
        .h(t().d(32.))
        .px(t().d(10.))
        .rounded(t().r(6.))
        .when(!enabled, |el| el.opacity(0.45))
        .child(
            Icon::new(icon)
                .with_size(Size::Size(t().d(15.)))
                .text_color(rgb(t().p.muted)),
        )
        .child(div().text_size(t().fs(12.5)).child(label.into()))
        .into_any_element()
}

fn info_row(label: &'static str, value: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t().d(10.))
        .h(t().d(30.))
        .px(t().d(10.))
        .text_size(t().fs(12.))
        .child(div().text_color(rgb(t().p.text_2)).child(label))
        .child(div().flex_1())
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(muted()))
                .child(value),
        )
        .into_any_element()
}

fn field(label: &'static str, input: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(5.))
        .w_full()
        .min_w(t().d(0.))
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.muted))
                .child(label),
        )
        .child(div().w_full().min_w(t().d(0.)).child(input))
        .into_any_element()
}

/// 插件页的一行。点击选中并进入详情（窄窗由 `about_detail_open` 控制）。
fn plugin_row(
    module: &nous_engine::Module,
    fallback_version: &str,
    selected: bool,
    cx: &mut Context<Desktop>,
) -> AnyElement {
    let t = t();
    let name = module.name.clone();
    let label = crate::capability::module_feature(&name)
        .map(|f| f.label().to_string())
        .unwrap_or_else(|| name.clone());
    let version = module.version.as_deref().unwrap_or(fallback_version);
    let state = if module.enabled { "启用" } else { "未启用" };
    let color = if module.enabled { t.p.ok } else { t.p.muted };
    div()
        .id(ElementId::Name(format!("about-plugin-{name}").into()))
        .on_click(cx.listener({
            let name = name.clone();
            move |this, _: &ClickEvent, _, cx| {
                this.about_selected = Some(name.clone());
                this.about_detail_open = true;
                cx.notify();
            }
        }))
        .flex()
        .items_center()
        .gap(t.d(8.))
        .px(t.d(8.))
        .py(t.d(7.))
        .rounded(t.r(5.))
        .when(selected, |el| el.bg(rgb(t.p.accent_soft)))
        .when(!selected, |el| el.hover(|h| h.bg(rgb(t.p.row_hover))))
        .child(
            div()
                .w(t.d(6.))
                .h(t.d(6.))
                .flex_none()
                .rounded_full()
                .bg(rgb(color)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t.d(2.))
                .min_w(t.d(0.))
                .flex_1()
                .child(
                    div()
                        .text_size(t.fs(12.))
                        .text_color(rgb(if selected { t.p.accent_fg } else { t.p.text }))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(t.fs(10.))
                        .text_color(rgb(muted()))
                        .child(name),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap(t.d(2.))
                .flex_none()
                .child(
                    div()
                        .text_size(t.fs(10.5))
                        .text_color(rgb(muted()))
                        .child(format!("v{version} · {}", state)),
                )
                .child(
                    div()
                        .text_size(t.fs(10.))
                        .text_color(rgb(faint()))
                        .child(format!("{} 端点", module.endpoints.len())),
                ),
        )
        .into_any_element()
}

fn plugin_meta(label: &'static str, value: String) -> AnyElement {
    let t = t();
    div()
        .flex()
        .gap(t.d(10.))
        .text_size(t.fs(11.5))
        .child(
            div()
                .w(t.d(72.))
                .flex_none()
                .text_color(rgb(muted()))
                .child(label),
        )
        .child(div().min_w(t.d(0.)).text_color(rgb(t.p.text_2)).child(value))
        .into_any_element()
}

fn readonly(label: &'static str, value: String, note: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(5.))
        .w_full()
        .min_w(t().d(0.))
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.muted))
                .child(label),
        )
        .child(
            div()
                .w_full()
                .min_w(t().d(0.))
                .p(t().d(8.))
                .rounded(t().r(5.))
                .bg(rgb(t().p.surface))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.text_2))
                // 引擎地址 / 配置文件路径都是不可断行的长串：窄窗下截断，不撑宽页面。
                .truncate()
                .child(value),
        )
        .child(div().text_size(t().fs(10.5)).text_color(rgb(faint())).child(note))
        .into_any_element()
}

/// `GET /v1/device` 的字段明细，About 面板直接画这些行。
/// 缺字段就不编造 —— 一律 `None`，界面显示「未知」。
#[derive(Debug, Clone, Default)]
pub(crate) struct DeviceFacts {
    pub node: Option<String>,
    pub build: Option<String>,
    pub target: Option<String>,
    pub started_at: Option<String>,
    pub model: Option<String>,
    pub faux: Option<bool>,
    pub available: usize,
}

/// 七个字段的归属表：判据 4 要「每个字段有主」，这里写死谁在哪一屏呈现。
/// `extra` 里那几个字段不进界面，也得写明「不用」的原因。
#[cfg(test)]
pub(crate) fn device_field_owners() -> &'static [(&'static str, &'static str)] {
    &[
        ("node", "设置页 / 关于：引擎版本"),
        ("build", "设置页 / 关于：构建号"),
        ("target", "设置页 / 关于：编译目标"),
        ("startedAt", "设置页 / 关于：引擎已运行 X"),
        ("model", "设置页 / 关于：默认模型"),
        ("available", "设置页 / 关于：可用模型数"),
        ("faux", "设置页 / 关于：演示数据 + 标题栏 DEMO 角标"),
        ("self", "不用：实例名只做服务端寻址，界面已有引擎地址"),
        ("origin", "不用：请求源地址不面向用户"),
        ("data", "不用：引擎侧数据目录，桌面端不碰"),
        ("catalog", "不用：模型目录内容由 /v1/models 提供"),
    ]
}

/// `startedAt` 是 epoch 毫秒，不是时长。客户端只做一次减法显示「引擎已运行 X」，
/// 不缓存、不当真相源；拿不到 / 不是数字就返回 `None`（About 显示「未知」）。
fn started_text(v: Option<&serde_json::Value>, now_ms: i128) -> Option<String> {
    let started = v?.as_i64()? as i128;
    let secs = (now_ms - started).max(0) / 1000;
    if secs < 60 {
        Some(format!("已运行 {secs} 秒"))
    } else if secs < 3600 {
        Some(format!("已运行 {} 分钟", secs / 60))
    } else if secs < 86400 {
        Some(format!("已运行 {} 小时", secs / 3600))
    } else {
        Some(format!("已运行 {} 天", secs / 86400))
    }
}

/// 把 `Device` 压成 About 面板要显示的那几行。纯函数，好钉判据 4。
/// `now_ms` 由调用方给（`std::time::SystemTime`），测试给固定值。
pub(crate) fn device_facts(device: Option<&nous_engine::types::Device>, now_ms: i128) -> DeviceFacts {
    let Some(d) = device else { return DeviceFacts::default() };
    let str_of = |v: Option<&serde_json::Value>| -> Option<String> {
        v.and_then(|v| v.as_str()).map(str::to_string)
    };
    let model = d.model.as_ref().and_then(|m| {
        let provider = str_of(m.get("provider"))?;
        let id = str_of(m.get("modelId"))?;
        Some(format!("{provider}/{id}"))
    });
    DeviceFacts {
        node: d.node.clone(),
        build: str_of(d.extra.get("build")),
        target: str_of(d.extra.get("target")),
        started_at: started_text(d.started_at.as_ref(), now_ms),
        model,
        faux: d.faux,
        available: d.available.len(),
    }
}

fn kv(label: &'static str, value: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(3.))
        .pb(t().d(8.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.muted))
                .child(label),
        )
        .child(div().text_size(t().fs(12.)).child(value))
        .into_any_element()
}

/// 在文件管理器里打开配置文件所在目录。Windows 只发便携包，`explorer` 够用；
/// explorer 的退出码不可靠（成功也常常非 0），一律不看结果。
fn reveal_config() {
    let Ok(path) = Config::save_target() else {
        return;
    };
    let Some(dir) = path.parent().map(|d| d.to_path_buf()) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&dir).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
}

#[cfg(test)]
mod tests {
    // 不 `use super::*`：gpui 重导出的宏再展开一层会撞递归上限。
    use super::{default_model_row, device_facts, CONFIG_PATH_HINT, device_field_owners};
    use nous_engine::types::{Device, ModelRef, Models};

    fn device() -> Device {
        serde_json::from_value(serde_json::json!({
            "node": "0.1.2",
            "build": "abc123",
            "target": "x86_64-unknown-linux-gnu",
            "startedAt": 1_700_000_000_000i64,
            "model": {"provider": "volc-ark", "modelId": "deepseek-v3"},
            "faux": true,
            "data": "/var/lib/nous",
            "catalog": null,
            "available": ["a/b", "c/d"],
            "self": "engine-1",
            "origin": "http://127.0.0.1:4789",
        }))
        .unwrap()
    }

    /// Desk#17：默认模型行的「能不能选 + 理由」纯状态。
    /// 编目空 = 禁用 + 人话；能力灰 = 禁用 + 引擎原文；有编目 = 可选。
    #[test]
    fn default_model_row_state() {
        let models: Models = serde_json::from_value(serde_json::json!({
            "default": {"provider":"p","modelId":"one"},
            "items": [{"provider":"p","modelId":"one","name":"一","thinkingLevels":[]}],
        })).unwrap();
        let row = default_model_row(Some(&models), None);
        assert!(row.enabled);
        assert_eq!(row.current.as_deref(), Some("p/one"));
        assert_eq!(row.items.len(), 1);
        assert!(row.reason.is_none());

        let empty: Models = serde_json::from_value(serde_json::json!({
            "default": null, "items": [],
        })).unwrap();
        let row = default_model_row(Some(&empty), None);
        assert!(!row.enabled);
        assert_eq!(row.reason.as_deref(), Some("引擎没有模型编目"));

        let row = default_model_row(None, None);
        assert!(!row.enabled);
        assert!(row.reason.as_deref().is_some_and(|r| r.contains("没连上引擎")));

        let row = default_model_row(
            Some(&models),
            Some("默认模型还没做（#17 哨兵）".to_string()),
        );
        assert!(!row.enabled);
        assert!(row.reason.as_deref().is_some_and(|r| r.contains("默认模型还没做（#17 哨兵）")));

        // 负向哨兵：回包 default 抄进本地状态（调用方拿它覆盖 Models.default）。
        let receipt_default = ModelRef {
            provider: "p".into(), model_id: "two".into(), extra: Default::default(),
        };
        assert_eq!(receipt_default.key(), "p/two");
    }

    /// 判据 4：七个字段每一个都有主（展示 / 明确不用）。哨兵删一个字段就红。
    #[test]
    fn every_device_field_has_an_owner() {
        let facts = device_facts(Some(&device()), 1_700_000_060_000);
        assert_eq!(facts.node.as_deref(), Some("0.1.2"));
        assert_eq!(facts.build.as_deref(), Some("abc123"));
        assert_eq!(facts.target.as_deref(), Some("x86_64-unknown-linux-gnu"));
        assert!(facts.started_at.unwrap().contains("已运行"));
        assert_eq!(facts.model.as_deref(), Some("volc-ark/deepseek-v3"));
        assert_eq!(facts.available, 2);
        assert_eq!(facts.faux, Some(true));
        let owners = device_field_owners();
        for key in ["node", "build", "target", "startedAt", "model", "available", "faux"] {
            assert!(owners.iter().any(|(k, _)| *k == key), "{key} 没有渲染点 / 不用注释");
        }
        for key in ["self", "origin", "data", "catalog"] {
            assert!(owners.iter().any(|(k, why)| *k == key && why.contains("不用")), "{key} 没有交代");
        }
        let none = device_facts(None, 0);
        assert!(none.node.is_none() && none.faux.is_none() && none.started_at.is_none());
    }

    /// 判据 2：连接页那句人话不许再提 Windows 的路径变量。
    #[test]
    fn connection_hint_never_says_windows_env_var() {
        assert!(!CONFIG_PATH_HINT.contains('%'));
        assert!(CONFIG_PATH_HINT.contains("NOUS_DESKTOP_CONFIG"));
    }
}
