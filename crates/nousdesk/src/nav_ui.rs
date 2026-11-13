//! 会话内跳转目录 + 历史抽屉的**渲染层**。
//!
//! 规矩（照面板块那条左列）：目录是**目录，不是清单** —— 不产生状态、不存盘、
//! 不写回引擎。数据源只有 `entries`（`GET /v1/entries`）和引擎 SSE 快照里的时间戳。
//!
//! 布局：会话列表 280px 不动 → 目录列 240px → 聊天区。
//! 抽屉只盖聊天区，**不盖目录列**：抽屉开着时目录仍然指向抽屉里的全量正文。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin { crate::theme::cur() }

use std::collections::{HashMap, HashSet};

use gpui_kit::component::hover_card::HoverCard;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use nous_engine::types::Entry;

use crate::session_nav::{NavDay, NavRound, answer_preview, build_nav, main_from, one_line};

const NAV_W: f32 = 240.;
const DRAWER_W: f32 = 0.86;
/// 原 `DAY_MUTED = 0x9a9a9a`；由 muted 这个语义槽接管。
fn day_muted() -> u32 { t().p.muted }
/// 原 `SOFT_GREEN = 0xe8f6ec`；由 accent_soft 这个语义槽接管。
fn soft_green() -> u32 { t().p.accent_soft }

/// 目录 + 抽屉的全部状态。都是**纯内存**的「我眼下看到哪」，不是事实。
#[derive(Default)]
pub struct SessionNav {
    /// 「全部」还是「只显示主视图里有的」。用户拍板：**全部**。
    /// 目录的职责是让你看见「这趟聊了什么」，跟着主视图缩反而失去意义。
    /// 真要改成只显示最近，就是 `groups().filter(r.entry_index >= from)` 一处的事。
    pub scope_all: bool,
    /// 主视图从哪个 entry 开始。默认 `main_from(entries.len())`。
    /// 点抽屉里的「把这一轮加载进主视图」会往前提，顶栏给「收起」。
    pub main_from: Option<usize>,
    /// 抽屉开着吗。
    pub drawer_open: bool,
    /// 折叠着的日期组（key 是 `NavDay.label`）。
    pub closed: HashSet<String>,
    /// 当前高亮的是第几轮。
    pub current: usize,
    /// 目录列自己的滚动句柄（跟随高亮时让它别跑出视口）。
    pub nav_scroll: ScrollHandle,
    /// 抽屉里全量正文的滚动句柄。
    pub drawer_scroll: ScrollHandle,
    /// SSE 首帧带来的 entry 时间戳：id → epoch ms。
    pub times: HashMap<String, i64>,
}

impl SessionNav {
    pub fn new() -> Self {
        Self {
            scope_all: true,
            ..Default::default()
        }
    }

    /// 换会话：抽屉收起、折叠清空、时间清掉（等新会话的快照）。
    pub fn reset(&mut self) {
        self.main_from = None;
        self.drawer_open = false;
        self.closed.clear();
        self.current = 0;
        self.times.clear();
        self.drawer_scroll = ScrollHandle::new();
    }

    pub fn from_of(&self, len: usize) -> usize {
        self.main_from.unwrap_or_else(|| main_from(len))
    }

    pub fn is_short(&self, len: usize) -> bool {
        self.main_from.is_none() || self.from_of(len) == main_from(len)
    }

    pub fn groups(&self, entries: &[Entry], read_at: Option<i64>, has_unread: bool) -> Vec<NavDay> {
        let all = build_nav(entries, &self.times, read_at, has_unread);
        if self.scope_all {
            return all;
        }
        let from = self.from_of(entries.len());
        all.into_iter()
            .filter_map(|mut g| {
                g.rounds.retain(|r| r.entry_index >= from);
                (!g.rounds.is_empty()).then_some(g)
            })
            .collect()
    }

    /// 跟随高亮：正文滚到哪，当前那组自动摊开（J 里补掉 H 的那个坑）。
    pub fn reveal(&mut self, group: &str) {
        if self.closed.remove(group) {
            // 手动折的被自动展开覆盖；用户手动展开的组不会自动收回去。
        }
    }
}

/// 目录整列。`entries` 用来算每行的浮窗摘要（回答首行）。
pub fn nav_column(
    entries: &[Entry],
    groups: &[NavDay],
    nav: &SessionNav,
    main_from: usize,
    drawer_open: bool,
    cx: &mut Context<crate::app::Desktop>,
) -> AnyElement {
    let total: usize = groups.iter().map(|g| g.rounds.len()).sum();
    let previews: HashMap<usize, String> = groups
        .iter()
        .flat_map(|g| g.rounds.iter())
        .filter_map(|r| preview_text(entries, groups, r.index).map(|p| (r.index, p)))
        .collect();
    let list =
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .id("nav-scroll")
            .overflow_y_scroll()
            .track_scroll(&nav.nav_scroll)
            .px(t().d(6.))
            .py(t().d(8.))
            .children(groups.iter().enumerate().map(|(gi, g)| {
                day_group(gi, g, groups, &previews, nav, main_from, drawer_open, cx)
            }));

    div()
        .flex()
        .flex_col()
        .w(px(NAV_W))
        .h_full()
        .flex_none()
        .bg(rgb(t().p.panel_alt))
        .border_r_1()
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .flex()
                .items_center()
                .gap(t().d(6.))
                .px(t().d(12.))
                .py(t().d(8.))
                .border_b_1()
                .border_color(rgb(t().p.line_soft))
                .child(
                    div()
                        .text_size(t().fs(11.5))
                        .text_color(rgb(t().p.text_2))
                        .child("你问过的"),
                )
                .child(
                    div()
                        .text_size(t().fs(11.))
                        .text_color(rgb(day_muted()))
                        .child(format!("按天 · {total} 条")),
                ),
        )
        .child(nav_toggles(nav, cx))
        .child(list)
        .into_any_element()
}

/// 目录顶上三个状态开关。
fn nav_toggles(nav: &SessionNav, cx: &mut Context<crate::app::Desktop>) -> AnyElement {
    let fold_on = !nav.closed.is_empty();
    div()
        .flex()
        .flex_wrap()
        .gap(t().d(5.))
        .px(t().d(8.))
        .py(t().d(6.))
        .border_b_1()
        .border_color(rgb(t().p.line_soft))
        .child(toggle(
            "折叠旧组",
            fold_on,
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.nav_toggle_fold(cx);
            }),
        ))
        .child(toggle(
            "自动展开",
            true,
            cx.listener(|_: &mut crate::app::Desktop, _: &ClickEvent, _, _| {}),
        ))
        .child(toggle(
            "悬停预览",
            true,
            cx.listener(|_: &mut crate::app::Desktop, _: &ClickEvent, _, _| {}),
        ))
        .into_any_element()
}

fn toggle(
    label: &str,
    on: bool,
    action: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id(ElementId::Name(format!("nav-toggle-{label}").into()))
        .flex_none()
        .px(t().d(6.))
        .py(t().d(2.))
        .rounded(t().r(10.))
        .border_1()
        .border_color(rgb(if on { t().p.accent_soft } else { t().p.line_soft }))
        .bg(rgb(if on { soft_green() } else { t().p.panel }))
        .text_size(t().fs(10.5))
        .text_color(rgb(if on { t().p.accent_fg } else { t().p.muted }))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(if on { t().p.accent_soft_hover } else { t().p.hover })))
        .on_click(action)
        .child(label.to_string())
        .into_any_element()
}

/// 一个日期组。
fn day_group(
    gi: usize,
    g: &NavDay,
    groups: &[NavDay],
    previews: &HashMap<usize, String>,
    nav: &SessionNav,
    main_from: usize,
    drawer_open: bool,
    cx: &mut Context<crate::app::Desktop>,
) -> AnyElement {
    let closed = nav.closed.contains(&g.label);
    let label = g.label.clone();
    let count = g.rounds.len();
    div()
        .flex()
        .flex_col()
        .mb(t().d(4.))
        .child(
            div()
                .id(ElementId::Name(format!("nav-group-{gi}").into()))
                .flex()
                .items_center()
                .gap(t().d(6.))
                .w_full()
                .px(t().d(8.))
                .py(t().d(6.))
                .rounded(t().r(4.))
                .cursor_pointer()
                .hover(|el| el.bg(rgb(t().p.hover)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.nav_toggle_group(label.clone(), cx);
                }))
                .child(
                    div()
                        .text_size(t().fs(10.))
                        .text_color(rgb(day_muted()))
                        .child(if closed { "▸" } else { "▾" }),
                )
                .child(
                    div()
                        .text_size(t().fs(11.5))
                        .text_color(rgb(day_muted()))
                        .child(g.label.clone()),
                )
                .child(
                    div()
                        .ml_auto()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.faint))
                        .child(count.to_string()),
                ),
        )
        .when(!closed, |el| {
            el.children(g.rounds.iter().map(|r| {
                nav_row(
                    r,
                    g,
                    groups,
                    previews,
                    nav.current,
                    main_from,
                    drawer_open,
                    cx,
                )
            }))
        })
        .into_any_element()
}

/// 目录一行：序号 + 两行截断的问题 + 轮次小标记 + 悬停浮窗。
fn nav_row(
    r: &NavRound,
    g: &NavDay,
    _groups: &[NavDay],
    previews: &HashMap<usize, String>,
    current: usize,
    main_from: usize,
    drawer_open: bool,
    cx: &mut Context<crate::app::Desktop>,
) -> AnyElement {
    let on = r.index == current;
    let old = r.entry_index < main_from && !drawer_open;
    let idx = r.index;
    let label = g.label.clone();
    let marks = marks(r);
    let where_to = if drawer_open {
        "抽屉里定位"
    } else if !old {
        "主视图里直接跳"
    } else {
        "打开抽屉定位"
    };
    let preview = previews.get(&idx).cloned();
    let day = g.label.clone();
    let text = r.text.clone();
    let meta = meta_line(r, &day);

    let trigger = div()
        .id(ElementId::Name(format!("nav-round-{idx}").into()))
        .flex()
        .items_center()
        .gap(t().d(7.))
        .w_full()
        .px(t().d(8.))
        .py(t().d(6.))
        .rounded(t().r(4.))
        // 透明**必须**用 rgba：`rgb(0x00000000)` 会被当成不透明白->黑
        // （gpui 的 rgb 只吃 24 位），真踩过：目录里半数列变成黑块。
        .when(on, |el| el.bg(rgb(soft_green())))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(if on { t().p.accent_soft_hover } else { t().p.hover })))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.nav_jump(idx, label.clone(), cx);
        }))
        .child(
            div()
                .flex_none()
                .w(t().d(18.))
                .text_size(t().fs(11.))
                .text_color(rgb(if on { t().p.accent } else { t().p.faint }))
                .child(format!("{}", idx + 1)),
        )
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(t().fs(12.5))
                .text_color(rgb(if on || !old { t().p.text_2 } else { t().p.faint }))
                .child(one_line(&r.text)),
        )
        .children(marks);

    HoverCard::new(format!("nav-peek-{idx}"))
        // `Anchor` 指的是**浮窗自己贴着触发器的哪一角**，不是「浮窗去哪边」：
        // `LeftCenter` → Placement::Right，浮窗落在目录右边；写成 RightCenter
        // 会跑到左边盖住会话列表（真踩过）。
        .anchor(Anchor::LeftCenter)
        .open_delay(std::time::Duration::from_millis(300))
        .close_delay(std::time::Duration::from_millis(120))
        .trigger_style(StyleRefinement::default().w_full())
        .trigger(trigger)
        .content(move |_state, _window, _cx| {
            div()
                .w(t().d(310.))
                .p(t().d(11.))
                .bg(rgb(t().p.panel))
                .rounded(t().r(6.))
                .border_1()
                .border_color(rgb(t().p.line))
                .text_size(t().fs(12.))
                .child(
                    div()
                        .mb(t().d(6.))
                        .flex()
                        .gap(t().d(6.))
                        .child(pill("悬停预览"))
                        .child(pill(where_to)),
                )
                .child(div().mb(t().d(5.)).text_color(rgb(t().p.text)).child(format!(
                    "{} · {}",
                    idx + 1,
                    text
                )))
                .child(
                    div().mb(t().d(7.)).text_color(rgb(t().p.text_2)).child(
                        preview
                            .clone()
                            .unwrap_or_else(|| "（这一轮还没有回答）".into()),
                    ),
                )
                .child(
                    div()
                        .pt(t().d(6.))
                        .border_t_1()
                        .border_color(rgb(t().p.line_soft))
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.faint))
                        .child(meta.clone()),
                )
        })
        .into_any_element()
}

fn pill(text: &str) -> AnyElement {
    div()
        .flex_none()
        .px(t().d(5.))
        .py(t().d(1.))
        .rounded(t().r(3.))
        .bg(rgb(soft_green()))
        .text_size(t().fs(10.))
        .text_color(rgb(t().p.accent_fg))
        .child(text.to_string())
        .into_any_element()
}

fn meta_line(r: &NavRound, day: &str) -> String {
    let mut bits = vec![day.to_string()];
    bits.push(if r.tools > 0 {
        format!("{} 次工具", r.tools)
    } else {
        "纯聊天".into()
    });
    if r.codes > 0 {
        bits.push("含代码块".into());
    }
    if r.after_read {
        bits.push("未读从这里开始".into());
    }
    bits.join(" · ")
}

fn marks(r: &NavRound) -> Vec<AnyElement> {
    let mut out = Vec::new();
    if r.tools > 0 {
        out.push(mark(r.tools.to_string()));
    }
    if r.codes > 0 {
        out.push(mark("{}".into()));
    }
    if r.after_read {
        out.push(
            div()
                .flex_none()
                .size(t().d(5.))
                .rounded(t().r(3.))
                .bg(rgb(t().p.badge))
                .into_any_element(),
        );
    }
    out
}

fn mark(text: String) -> AnyElement {
    div()
        .flex_none()
        .px(t().d(3.))
        .py(t().d(1.))
        .rounded(t().r(3.))
        .bg(rgb(t().p.code_bg))
        .text_size(t().fs(10.))
        .text_color(rgb(t().p.muted))
        .child(text)
        .into_any_element()
}

/// 9 号方案的「更早 N 条」入口。画在主视图顶部。
pub fn more_bar(older: usize, cx: &mut Context<crate::app::Desktop>) -> AnyElement {
    div()
        .id("nav-more")
        .flex()
        .items_center()
        .gap(t().d(9.))
        .w_full()
        .px(t().d(11.))
        .py(t().d(7.))
        .mb(t().d(10.))
        .rounded(t().r(5.))
        .border_1()
        .border_color(rgb(t().p.accent_soft))
        .bg(rgb(t().p.accent_soft))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(soft_green())))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.nav_open_drawer(None, cx)))
        .child(
            div()
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.accent_fg))
                .child(format!("更早的 {older} 条 · 打开历史")),
        )
        .child(
            div()
                .ml_auto()
                .text_size(t().fs(11.))
                .text_color(rgb(t().p.ok_fg))
                .child("主视图只留最近几轮 →"),
        )
        .into_any_element()
}

/// 抽屉：右侧 86% 宽，只盖聊天区。
pub fn drawer(
    open: bool,
    children: Vec<AnyElement>,
    scroll: &ScrollHandle,
    cx: &mut Context<crate::app::Desktop>,
) -> AnyElement {
    if !open {
        return div().into_any_element();
    }
    div()
        .absolute()
        .inset_0()
        .bg(rgba(t().p.scrim))
        .child(
            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(relative(DRAWER_W))
                .bg(rgb(t().p.panel))
                .shadow_md()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .h(t().d(40.))
                        .flex()
                        .items_center()
                        .gap(t().d(10.))
                        .px(t().d(14.))
                        .border_b_1()
                        .border_color(rgb(t().p.line_soft))
                        .child(div().text_size(t().fs(12.5)).child("全量历史"))
                        .child(
                            div()
                                .text_size(t().fs(11.))
                                .text_color(rgb(day_muted()))
                                .child("按时间"),
                        )
                        .child(
                            div()
                                .id("nav-to-main")
                                .ml_auto()
                                .px(t().d(9.))
                                .py(t().d(3.))
                                .rounded(t().r(12.))
                                .border_1()
                                .border_color(rgb(t().p.line_strong))
                                .bg(rgb(t().p.accent_soft))
                                .text_size(t().fs(11.5))
                                .text_color(rgb(t().p.accent_fg))
                                .cursor_pointer()
                                .hover(|el| el.bg(rgb(soft_green())))
                                .on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| this.nav_to_main(cx)),
                                )
                                .child("把这一轮加载进主视图"),
                        )
                        .child(
                            div()
                                .id("nav-drawer-close")
                                .cursor_pointer()
                                .text_size(t().fs(12.))
                                .text_color(rgb(t().p.faint))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.nav_close_drawer(cx)
                                }))
                                .child("关闭 ✕"),
                        ),
                )
                .child(
                    div()
                        .id("nav-drawer-body")
                        .on_scroll(cx.listener(|this, _e: &ScrollEvent, _, cx| this.nav_spy(cx)))
                        .flex_1()
                        .min_h(t().d(0.))
                        .overflow_y_scroll()
                        .track_scroll(scroll)
                        .px(t().d(16.))
                        .py(t().d(10.))
                        .child(
                            div()
                                .mb(t().d(12.))
                                .px(t().d(9.))
                                .py(t().d(5.))
                                .rounded(t().r(4.))
                                .bg(rgb(t().p.panel_alt))
                                .border_1()
                                .border_color(rgb(t().p.line_soft))
                                .text_size(t().fs(11.))
                                .text_color(rgb(t().p.faint))
                                .child(
                                    "左边那列目录现在指向这里 —— 点任一轮，我滚到它；不用关抽屉。",
                                ),
                        )
                        .children(children),
                ),
        )
        .into_any_element()
}

/// 顶栏：主视图展开时出现的「收起」按钮。
pub fn collapse_button(cx: &mut Context<crate::app::Desktop>) -> AnyElement {
    div()
        .id("nav-collapse")
        .flex_none()
        .px(t().d(10.))
        .py(t().d(3.))
        .rounded(t().r(12.))
        .border_1()
        .border_color(rgb(t().p.warn_soft))
        .bg(rgb(t().p.warn_soft))
        .text_size(t().fs(11.5))
        .text_color(rgb(t().p.warn))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(t().p.warn_soft)))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.nav_collapse(cx)))
        .child("收起，回到最近")
        .into_any_element()
}

/// 顶栏：历史与目录。
pub fn history_button(cx: &mut Context<crate::app::Desktop>) -> AnyElement {
    div()
        .id("nav-history")
        .flex_none()
        .px(t().d(10.))
        .py(t().d(3.))
        .rounded(t().r(12.))
        .bg(rgb(t().p.accent))
        .text_size(t().fs(11.5))
        .text_color(rgb(t().p.on_accent))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(t().p.accent_hover)))
        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.nav_open_drawer(None, cx)))
        .child("历史与目录")
        .into_any_element()
}
/// 目录行的浮窗要的那句回答首行。用一个静态钩子从 `Desktop` 拿，
/// 避免把整个 `Context<Desktop>` 塞进 `HoverCard` 的 `'static` content 闭包。
pub fn preview_text(entries: &[Entry], groups: &[NavDay], idx: usize) -> Option<String> {
    let round = groups
        .iter()
        .flat_map(|g| g.rounds.iter())
        .find(|r| r.index == idx)?;
    answer_preview(entries, round, groups)
}
