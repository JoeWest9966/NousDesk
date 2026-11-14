//! 左栏底部的「≡ 更多」菜单，和它打开的设置页（占右栏整页）。
//!
//! 菜单只放**真能点**的条目：设置 / 重新连接 / 打开配置文件夹 / 版本信息。
//! 不出「消息通知」「快捷键」「退出登录」这种假入口 —— 没实现的东西摆在那儿，
//! 用户点一次就等于被晃一次。
//!
//! 设置里只有 `host` 和 `board` 两个可编辑项。引擎地址**不**给输入框：
//! 它只能从 `__nous/resolve` 拿，写进配置就有了第二个真相。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin { crate::theme::cur() }

use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, Sizable as _, Size};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::Desktop;
use crate::config::Config;

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
                    .child(menu_row(
                        "menu-archive",
                        IconName::Archive,
                        "归档库",
                        true,
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
            (format!("已连接 · {}", self.conn.board), good())
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

    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> AnyElement {
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
                    .child(self.settings_pane(cx)),
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

    fn settings_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let body: AnyElement = match self.settings_tab {
            SettingsTab::Connection => self.connection_pane(cx),
            SettingsTab::Theme => self.theme_pane(cx),
            SettingsTab::About => self.about_pane(),
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
                    .p(t().d(22.))
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
            .max_w(t().d(460.))
            .child(div().text_size(t().fs(15.)).child("连接"))
            .child(
                div()
                    .text_size(t().fs(11.5))
                    .text_color(rgb(muted()))
                    .child("改完保存会写到下面那个文件，然后重新 resolve 一次。"),
            )
            .child(field("宿主地址 host", Input::new(&self.host_input)))
            .child(field("Board", Input::new(&self.board_input)))
            .child(readonly(
                "引擎地址（只读）",
                self.engine_url
                    .clone()
                    .unwrap_or_else(|| "未连接 —— 先 resolve 到才会显示".into()),
                "由 <host>/p/<board>/<self>/__nous/resolve/<engine> 现拿，不写进配置。",
            ))
            .child(readonly(
                "配置文件（保存写这里）",
                target,
                "查找顺序：NOUS_DESKTOP_CONFIG → exe 旁 desktop.toml → %APPDATA%\\nousdesk\\",
            ))
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
            .w(skin.d(196.))
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

    fn about_pane(&self) -> AnyElement {
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

        div()
            .flex()
            .flex_col()
            .gap(t().d(14.))
            .max_w(t().d(520.))
            .child(div().text_size(t().fs(15.)).child("关于"))
            .child(
                div()
                    .text_size(t().fs(11.5))
                    .text_color(rgb(muted()))
                    .child("引擎版本来自 GET /v1/device 的 node —— 出问题先对版本。"),
            )
            .child(kv("客户端", format!("v{}", env!("CARGO_PKG_VERSION"))))
            .child(kv(
                "引擎",
                self.engine_node.clone().unwrap_or_else(|| "未知".into()),
            ))
            .child(kv(
                "引擎地址",
                self.engine_url.clone().unwrap_or_else(|| "未连接".into()),
            ))
            .child(kv("配置文件", cfg_path))
            .child(kv("日志", log_path))
            .into_any_element()
    }

    // ── 保存 ────────────────────────────────────────────────

    /// 把输入框里的两个值写回配置文件，然后重连。
    ///
    /// 校验只挡「一定错」的：空值、host 不带协议、board 带斜杠。剩下的交给 resolve 报错 ——
    /// 客户端猜地址对不对就等于有了第二个真相。
    pub(crate) fn save_settings(&mut self, cx: &mut Context<Self>) {
        let host = self.host_input.read(cx).value().trim().to_string();
        let board = self.board_input.read(cx).value().trim().to_string();

        let bad = |msg: String| Some((msg, true));
        let problem = if host.is_empty() {
            bad("host 不能为空".into())
        } else if !(host.starts_with("http://") || host.starts_with("https://")) {
            bad("host 要以 http:// 或 https:// 开头".into())
        } else if board.is_empty() {
            bad("board 不能为空".into())
        } else if board.contains('/') {
            bad("board 里不能有 /".into())
        } else {
            None
        };
        if let Some(msg) = problem {
            self.settings_msg = Some(msg);
            cx.notify();
            return;
        }

        let next = Config { host, board };
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
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.muted))
                .child(label),
        )
        .child(div().w_full().child(input))
        .into_any_element()
}

fn readonly(label: &'static str, value: String, note: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(5.))
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.muted))
                .child(label),
        )
        .child(
            div()
                .p(t().d(8.))
                .rounded(t().r(5.))
                .bg(rgb(t().p.surface))
                .border(t().d(1.))
                .border_color(rgb(t().p.line))
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.text_2))
                .child(value),
        )
        .child(div().text_size(t().fs(10.5)).text_color(rgb(faint())).child(note))
        .into_any_element()
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
