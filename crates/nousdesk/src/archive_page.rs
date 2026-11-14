//! 归档库（右栏整页）：`GET /v1/archive` 那张表。
//!
//! 引擎那边「归档 = 复印进 `gen-YYYYMMDD.sqlite` + 写墓碑」「撕原件 = 真删」都做完了
//! （`j1900:/opt/Nous/content/new1/map/qrd3wh/归档.md`），这一页只做三件事：
//! 按 `gen` 分组、把每条推成四态、给出「打捞 / 撕原件」两个口子。
//!
//! 三条边界：
//!
//! - **不联网。** 动作只产出 [`ArchiveAction`]（外加「选中 / 返回」两个纯回调），
//!   谁发请求、发完怎么刷新，都在 `app.rs`（票 #6）。
//! - **状态只算一次**，就在 [`item_status`] 里，判据照面板页 `mcfpqm` 的 `stateOf`
//!   （四态只看 item 自己，不看主列表）。别在别处再推一遍 —— 四态一旦有两份实现就会分叉。
//! - **「彻底删除」不在这页的词汇里。** 这里只有「撕原件」（引擎的 purge）；
//!   主列表和右键菜单永远不出现那个词 —— 这是内核和 `mcfpqm` 定下的规矩。
//!
//! 形状照设计稿「归档库页面 L」（`design/archive-entry-demos`：`demo/archive-entry/index.html`）：
//! 页头 + 列头（会话 / 状态 / 操作）+ 按天分组，每组一行日期标题，每行「名字 + 摘要 + 状态徽标 + 动作」。
//! 容器形状照 `settings::render_settings`（右栏整页，左栏列表不动）。

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, Size};
use gpui_kit::*;

use nous_engine::types::{ArchiveGen, ArchiveGens, ArchiveItem};

// ── 外观 ─────────────────────────────────────────────────────────────
//
// 颜色 / 字号 / 圆角 / 间距全走主题 token（`crate::theme`），模块里不再存一份
// `const` 颜色 —— 换皮时这一页要跟着变，写死的字面量会漏下来。

/// 这一帧的皮。和 `settings.rs` / `archive_confirm.rs` 同一个写法 —— 渲染层不自己
/// 存主题，统一从帧内快照取，换皮时才不会有一块漏掉。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 状态那格 / 动作那格的宽度（列头和行共用，别各写各的）。
/// 列宽本身是**布局常量**，不是间距 —— 用的时候包一层 `t().d(..)` 让它跟着密度缩放。
const STATUS_W: f32 = 64.;
const ACT_W: f32 = 88.;

// ── 回调 ─────────────────────────────────────────────────────────────
//
// 这页只管画：动作、选中、返回都交出去，`app.rs` 决定干什么。
// 全部用 `Rc<dyn Fn>`（和 `config_panel.rs` 一个套路）：`cx.listener` 拿到的闭包
// 就是这个形状，模块之间不用互相认识对方的类型。参数一律传 `&T`，正好对上
// `cx.listener` 的 `|this, arg: &T, window, cx|`。
/// 一颗按钮的点击处理器（`cx.listener` 包一层、或手写闭包，都是这个形状）。
pub type OnClick = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
pub type OnAction = Rc<dyn Fn(&ArchiveAction, &mut Window, &mut App)>;
pub type OnSelect = Rc<dyn Fn(&RowKey, &mut Window, &mut App)>;
/// 收起这一页（回聊天区）。入口/开关在 #6，这里只留出口。
pub type OnBack = OnClick;
/// 鼠标进 / 出这一页的视口（`true` = 进来了）—— 滚动条靠它决定淡入淡出。
pub type OnHover = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

// ── 动作 ─────────────────────────────────────────────────────────────

/// 这一页能发起的动作。**这里不发请求** —— 枚举出去，`app.rs` 接。
///
/// `r#gen` 是打捞必须带的那一批（`POST /v1/archive/:id/restore` 的 body `{"gen": …}`；
/// `gen` 是 Rust 2024 保留字，所以字段写 `r#gen`，和引擎类型同一个写法），
/// `name` 是撕原件危险确认标题要用的会话名（确认弹层是票 #3 的模块）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveAction {
    Restore { id: i64, r#gen: String },
    Purge { id: i64, name: String },
    Refresh,
}

impl ArchiveAction {
    pub fn label(&self) -> &'static str {
        match self {
            ArchiveAction::Restore { .. } => "打捞",
            ArchiveAction::Purge { .. } => "撕原件",
            ArchiveAction::Refresh => "刷新",
        }
    }

    /// 是不是危险动作（红字）。只有撕原件是。
    pub fn is_danger(&self) -> bool {
        matches!(self, ArchiveAction::Purge { .. })
    }
}

// ── 四态 ─────────────────────────────────────────────────────────────

/// 一条归档件的状态。页面只认这四种，算的地方只有 [`item_status`]。
///
/// 名字照 `mcfpqm` 的 `stateOf`（`gone` / `here` / `back` / `orphan`）翻的，
/// 判断一字没改。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveStatus {
    /// 原件在（`here`）：归档过（有墓碑）且引擎说 `live`，活库里那条还在 —— 可以撕。
    Live,
    /// 已销毁（`gone`）：墓碑上 `purged: true`，活库里原件已经撕了，只剩这份复印件。
    Purged,
    /// 已捞回（`back`）：`live` 且墓碑被抹了（`archived: null`），复印件照旧留着。
    Restored,
    /// 无墓碑（`orphan`）：复印件在、活库没有、也没归档记录 —— 盘被人动过。
    Orphan,
}

impl ArchiveStatus {
    /// 给人看的四个字（和 `mcfpqm` 的徽标一字不差）。
    pub fn label(self) -> &'static str {
        match self {
            ArchiveStatus::Live => "原件在",
            ArchiveStatus::Purged => "已销毁",
            ArchiveStatus::Restored => "已捞回",
            ArchiveStatus::Orphan => "无墓碑",
        }
    }

    /// 四态的全集。排版 / 筛选器真要用的时候再加，别提前摆着等（评审 J3）。
    #[cfg(test)]
    const ALL: [ArchiveStatus; 4] = [
        ArchiveStatus::Live,
        ArchiveStatus::Purged,
        ArchiveStatus::Restored,
        ArchiveStatus::Orphan,
    ];
}

/// 一条归档件的状态。**纯函数**，四态的唯一算法。
///
/// 照抄 `mcfpqm` 的 `stateOf`（`/tmp/nn/content/new1/map/mcfpqm/index.html:164-178`）：
///
/// ```js
/// if (item.archived?.purged) return "gone";        // 撕过原件
/// if (item.live && item.archived) return "here";   // 归档过、原件还在
/// if (item.live) return "back";                    // 捞回来了、墓碑抹了
/// return "orphan";                                 // 复印件在、活库没有、也没墓碑
/// ```
///
/// **只看 item 自己。** 别拿主列表（`GET /v1/sessions`）的 id 去掺和 ——
/// 那份列表默认就藏掉归档会话，掺进来只会把「已捞回」判成「无墓碑」。
pub fn item_status(item: &ArchiveItem) -> ArchiveStatus {
    if item.archived.as_ref().is_some_and(|t| t.purged) {
        return ArchiveStatus::Purged;
    }
    if item.live && item.archived.is_some() {
        return ArchiveStatus::Live;
    }
    if item.live {
        return ArchiveStatus::Restored;
    }
    ArchiveStatus::Orphan
}

/// 一行该给哪几个动作。**纯函数**，和四态一一对应（照 `mcfpqm` 的按钮条件）：
///
/// - 原件在（`here`）：只有 `撕原件`；
/// - 已销毁 / 无墓碑（`gone` / `orphan`）：只有 `打捞`；
/// - 已捞回（`back`）：没东西可做，一个动作都不给。
pub fn row_actions(
    item: &ArchiveItem,
    gen_name: &str,
    status: ArchiveStatus,
) -> Vec<ArchiveAction> {
    match status {
        ArchiveStatus::Live => vec![ArchiveAction::Purge {
            id: item.id,
            name: item.name.clone(),
        }],
        ArchiveStatus::Purged | ArchiveStatus::Orphan => vec![ArchiveAction::Restore {
            id: item.id,
            r#gen: gen_name.to_string(),
        }],
        ArchiveStatus::Restored => Vec::new(),
    }
}

/// 行尾摘要 —— 名字下面那行小灰字，兜底顺序照 `mcfpqm`：
///
/// ```js
/// node.last?.text?.replace(/\s+/g, " ").slice(0, 72)
///   || (node.parent ? `← #${node.parent.id} 分叉` : (node.entries ? `${node.entries} 条记录` : "空会话"))
/// ```
///
/// `parent` / `entries` / `last` 的形状由 `nous_engine::types::ArchiveItem` 收，
/// 这里不猜 JSON（评审 J5：协议形状不许在 UI 层手摸）。认不出就往下掉，
/// 最后一档是「空会话」。
pub fn row_summary(item: &ArchiveItem) -> String {
    if let Some(text) = item
        .last
        .as_ref()
        .and_then(|last| last.get("text"))
        .and_then(|t| t.as_str())
    {
        // 压空白再截 72 个**字符**（按字节截会把中文劈成乱码）。
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !flat.is_empty() {
            return flat.chars().take(SUMMARY_CHARS).collect();
        }
    }
    if let Some(parent) = &item.parent {
        return format!("← #{} 分叉", parent.id);
    }
    if item.entries > 0 {
        return format!("{} 条记录", item.entries);
    }
    "空会话".to_string()
}

/// 摘要截断长度（`mcfpqm` 的 `slice(0, 72)`）。
const SUMMARY_CHARS: usize = 72;

// ── 分组标题 ─────────────────────────────────────────────────────────

/// 组标题：`11月10日 · gen-20261110.sqlite`。
///
/// `gen` 是引擎的日期串（`20261110`）。也接受文件名形状（`gen-20261110.sqlite`）
/// 和 `2026-11-10`，方便别处复用。日期解析走 `chrono` —— 手写月份表那种东西
/// 迟早把 2 月写成 30 天。
///
/// 解析不出来（`20261301`、`probe` 这种）**不编日期**：原样当名字，
/// 文件名那截照旧给出来，页面不会因为一条脏数据变成空白。
pub fn gen_group_title(gen_name: &str) -> String {
    let trimmed = gen_name.trim();
    let stem = trimmed
        .trim_start_matches("gen-")
        .trim_end_matches(".sqlite")
        .trim();

    // 破折号写法归一成 8 位，后面只认一种形状。
    let normalized = match chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
        Ok(day) => day.format("%Y%m%d").to_string(),
        Err(_) => stem.to_string(),
    };
    let file = format!("gen-{normalized}.sqlite");

    match chrono::NaiveDate::parse_from_str(&normalized, "%Y%m%d") {
        Ok(day) => {
            use chrono::Datelike as _;
            // 月份/日期都补前导零：`11月07日` / `01月05日`（设计稿就这么写）。
            format!("{:02}月{:02}日 · {file}", day.month(), day.day())
        }
        Err(_) => format!("{normalized} · {file}"),
    }
}

// ── 页面状态 ─────────────────────────────────────────────────────────

/// 一行的身份：`gen` + 会话 id。同一条理论上只会出现在一个 `gen` 里，
/// 但选中态用整对做 key，省得赌这个假设。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowKey {
    pub r#gen: String,
    pub id: i64,
}

/// 归档库这一页的全部状态：数据、载入中、错误原文、选中态。
///
/// `#[derive(Default)]`：`ArchiveGens` 是哑类型，缺字段就是空列表 ——
/// 没数据的页面和拉失败要长得不一样，所以失败另存 `error`，不塞进 `data`。
#[derive(Debug, Clone, Default)]
pub struct ArchivePageState {
    /// `GET /v1/archive` 的原样数据。
    pub data: ArchiveGens,
    /// 正在拉。空数据时正文显示「载入中」，有数据时只把页头标成刷新中。
    pub loading: bool,
    /// 失败文案：**引擎原文**。客户端不改写 —— 409 里那些活分支的名字有用。
    pub error: Option<String>,
    /// 选中的那行（点击用）。
    pub selected: Option<RowKey>,
}

impl ArchivePageState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 一条都没有（不含 loading / error 的语义）。
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// 全部 `gen` 加起来的条数。
    pub fn total(&self) -> usize {
        self.data.gens.iter().map(|g| g.items.len()).sum()
    }

    /// 这条现在是什么状态（四态的唯一入口）。
    pub fn status_of(&self, item: &ArchiveItem) -> ArchiveStatus {
        item_status(item)
    }

    /// 选中某行。已经选中的再点一次不取消 —— 选中是「当前行」，不是开关。
    pub fn select(&mut self, key: RowKey) {
        self.selected = Some(key);
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }
}

// ── 页面 ─────────────────────────────────────────────────────────────

/// 画这一页（右栏整页）。调用方给状态 + 出口 + 滚动句柄，别的都在这儿。
///
/// `scroll` / `bar` / `on_hover` 是那条浮出来的滚动条：句柄归调用方（切页不丢位置），
/// 在不在场由 `bar` 说，悬停进出来自 `on_hover`。
pub fn render_archive_page(
    state: &ArchivePageState,
    scroll: &ScrollHandle,
    bar: (bool, bool),
    on_hover: OnHover,
    on_action: OnAction,
    on_select: OnSelect,
    on_back: OnBack,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .size_full()
        .bg(rgb(t().p.panel))
        .child(header(state, on_action.clone(), on_back))
        .child(column_head())
        .child(body(state, scroll, bar, on_hover, on_action, on_select))
        .into_any_element()
}

fn header(state: &ArchivePageState, on_action: OnAction, on_back: OnBack) -> AnyElement {
    let count = if state.loading && state.is_empty() {
        "载入中…".to_string()
    } else if state.loading {
        format!("{} 条 · 刷新中…", state.total())
    } else {
        format!("{} 条", state.total())
    };

    let refresh = on_action.clone();
    let refresh: OnClick =
        Rc::new(move |_ev, window, cx| refresh(&ArchiveAction::Refresh, window, cx));

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
            // 返回：右栏回到聊天区。左栏列表一直在，这里只收起这一页。
            div()
                .id(ElementId::Name("archive-back".into()))
                .on_click(move |ev, window, cx| on_back(ev, window, cx))
                .flex()
                .items_center()
                .gap(t().d(4.))
                .px(t().d(6.))
                .py(t().d(4.))
                .rounded(t().r(5.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.text_2))
                .cursor_pointer()
                .hover(|el| el.bg(rgb(t().p.hover)))
                .child(Icon::new(IconName::ArrowLeft).with_size(Size::Size(t().d(14.))))
                .child("返回聊天"),
        )
        .child(div().w(t().d(8.)))
        .child(
            div()
                .text_size(t().fs(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(PAGE_TITLE),
        )
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.muted))
                .child(count),
        )
        .child(div().flex_1())
        .child(link_button(
            ElementId::Name("archive-refresh".into()),
            "刷新",
            false,
            refresh,
        ))
        .into_any_element()
}

/// 列头（会话 / 状态 / 操作）。宽度和 [`row`] 里那两格对齐，别各写各的。
fn column_head() -> AnyElement {
    div()
        .flex()
        .items_center()
        .h(t().d(26.))
        .flex_none()
        .px(t().d(14.))
        .bg(rgb(t().p.panel))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .text_size(t().fs(10.5))
        .text_color(rgb(t().p.faint))
        .child(div().flex_1().child("会话"))
        .child(div().w(t().d(STATUS_W)).flex_none().child("状态"))
        .child(div().w(t().d(ACT_W)).flex_none().child("操作"))
        .into_any_element()
}

fn body(
    state: &ArchivePageState,
    scroll: &ScrollHandle,
    bar: (bool, bool),
    on_hover: OnHover,
    on_action: OnAction,
    on_select: OnSelect,
) -> AnyElement {
    let mut rows: Vec<AnyElement> = Vec::new();

    if let Some(err) = state.error.as_deref() {
        rows.push(error_box(err));
    }

    if state.is_empty() {
        if state.loading {
            rows.push(note(NOTE_LOADING));
        } else if state.error.is_none() {
            rows.push(note(NOTE_EMPTY));
            rows.push(note(NOTE_EMPTY_HINT));
        }
    } else {
        for group in &state.data.gens {
            if group.items.is_empty() {
                continue;
            }
            rows.push(group_head(group));
            for item in &group.items {
                rows.push(row(state, group, item, &on_action, &on_select));
            }
        }
    }

    div()
        // `on_hover` 挂在 `Stateful` 上：先给 id。
        .id(ElementId::Name("archive-viewport".into()))
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(t().d(0.))
        .on_hover(move |hovered: &bool, window, cx| on_hover(hovered, window, cx))
        .child(
            div()
                .id(ElementId::Name("archive-scroll".into()))
                .flex()
                .flex_col()
                .flex_1()
                .min_h(t().d(0.))
                .overflow_y_scroll()
                .track_scroll(scroll)
                .children(rows),
        )
        .child(crate::app::scrollbar::overlay(
            "archive-scrollbar",
            scroll,
            bar.0,
            bar.1,
        ))
        .into_any_element()
}

/// 一天的标题：`11月10日 · gen-20261110.sqlite` + 这批几条。
fn group_head(group: &ArchiveGen) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t().d(8.))
        .flex_none()
        .px(t().d(14.))
        .pt(t().d(14.))
        .pb(t().d(6.))
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.text_2))
                .child(gen_group_title(&group.r#gen)),
        )
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.faint))
                .child(format!("{} 条", group.items.len())),
        )
        .child(div().flex_1())
        .into_any_element()
}

fn row(
    state: &ArchivePageState,
    group: &ArchiveGen,
    item: &ArchiveItem,
    on_action: &OnAction,
    on_select: &OnSelect,
) -> AnyElement {
    let status = state.status_of(item);
    let key = RowKey {
        r#gen: group.r#gen.clone(),
        id: item.id,
    };
    let selected = state.selected.as_ref() == Some(&key);

    let name = if item.name.trim().is_empty() {
        format!("（未命名 · #{}）", item.id)
    } else {
        item.name.clone()
    };

    let acts: Vec<AnyElement> = row_actions(item, &group.r#gen, status)
        .into_iter()
        .map(|action| {
            link_button(
                ElementId::Name(
                    format!("archive-act-{}-{}-{}", group.r#gen, item.id, action.label()).into(),
                ),
                action.label(),
                action.is_danger(),
                act(action, on_action),
            )
        })
        .collect();

    let select = {
        let on_select = on_select.clone();
        let key = key.clone();
        Rc::new(move |_ev: &ClickEvent, window: &mut Window, cx: &mut App| {
            on_select(&key, window, cx)
        }) as OnClick
    };

    div()
        .id(ElementId::Name(
            format!("archive-row-{}-{}", group.r#gen, item.id).into(),
        ))
        .flex()
        .items_center()
        .flex_none()
        .px(t().d(14.))
        .py(t().d(7.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .bg(rgb(if selected { t().p.accent_soft } else { t().p.panel }))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(if selected { t().p.accent_soft } else { t().p.hover })))
        .on_click(move |ev, window, cx| select(ev, window, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(t().d(0.))
                .gap(t().d(1.))
                .child(
                    div()
                        .truncate()
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.text))
                        .child(name),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.faint))
                        .child(row_summary(item)),
                ),
        )
        .child(
            div()
                .w(t().d(STATUS_W))
                .flex_none()
                .flex()
                .items_center()
                .child(badge(status)),
        )
        .child(
            div()
                .w(t().d(ACT_W))
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .gap(t().d(10.))
                .children(acts),
        )
        .into_any_element()
}

/// 状态徽标。底色/字色取主题的语义槽（`accent_soft` / `danger_soft` / `warn_soft`）。
fn badge(status: ArchiveStatus) -> AnyElement {
    div()
        .flex_none()
        .px(t().d(6.))
        .py(t().d(1.))
        .rounded(t().r(4.))
        .bg(rgb(match status {
            ArchiveStatus::Live => t().p.accent_soft,
            ArchiveStatus::Purged => t().p.danger_soft,
            ArchiveStatus::Restored => t().p.list_bg,
            ArchiveStatus::Orphan => t().p.warn_soft,
        }))
        .text_size(t().fs(10.5))
        .text_color(rgb(match status {
            ArchiveStatus::Live => t().p.accent_fg,
            ArchiveStatus::Purged => t().p.danger,
            ArchiveStatus::Restored => t().p.muted,
            ArchiveStatus::Orphan => t().p.warn,
        }))
        .child(status.label())
        .into_any_element()
}

/// 行尾那种文字动作（`打捞` / `撕原件` / `刷新`）。
fn link_button(
    id: ElementId,
    label: &'static str,
    is_danger: bool,
    handler: OnClick,
) -> AnyElement {
    div()
        .id(id)
        .on_click(move |ev, window, cx| handler(ev, window, cx))
        .flex_none()
        .px(t().d(6.))
        .py(t().d(2.))
        .rounded(t().r(4.))
        .text_size(t().fs(11.5))
        .text_color(rgb(if is_danger { t().p.danger } else { t().p.link }))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(t().p.hover)))
        .child(label)
        .into_any_element()
}

/// 把「枚举 + 回调」包成 GPUI 能用的点击处理器。
///
/// 先 `stop_propagation`：动作按钮在可点的行里面，不拦的话点「撕原件」会顺手
/// 把那一行也选中。
fn act(action: ArchiveAction, on: &OnAction) -> OnClick {
    let on = on.clone();
    Rc::new(move |_ev, window, cx| {
        cx.stop_propagation();
        on(&action, window, cx);
    })
}

fn note(text: &'static str) -> AnyElement {
    div()
        .px(t().d(14.))
        .pt(t().d(14.))
        .text_size(t().fs(12.))
        .text_color(rgb(t().p.muted))
        .child(text)
        .into_any_element()
}

/// 失败横幅：引擎原文 + 一句客户端自己的提示（不改写原文）。
fn error_box(err: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(4.))
        .mx(t().d(14.))
        .mt(t().d(14.))
        .p(t().d(10.))
        .rounded(t().r(6.))
        .bg(rgb(t().p.danger_soft))
        .border(t().d(1.))
        .border_color(rgb(t().p.danger))
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.danger))
                .child(err.to_string()),
        )
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.muted))
                .child(NOTE_ERROR_HINT),
        )
        .into_any_element()
}

// ── 文案 ─────────────────────────────────────────────────────────────

const PAGE_TITLE: &str = "归档库";
const NOTE_LOADING: &str = "正在读归档库…";
const NOTE_EMPTY: &str = "归档库是空的。";
const NOTE_EMPTY_HINT: &str = "去会话列表里把一条对话归档，复印件就会落在这儿。";
const NOTE_ERROR_HINT: &str = "上面那句是引擎原文，客户端不改写。点「刷新」再试一次。";

#[cfg(test)]
mod tests {
    // **别写 `use super::*;`**：父模块顶上有 `use gpui_kit::*`，它带一个 `test` 宏，
    // 会把 `#[test]` 抢过去（报「recursion limit reached while expanding #[test]」）。
    // 只挑要用的名字，和 `templates.rs` / `app.rs` 的测试一个写法。
    use super::{
        ArchiveAction, ArchivePageState, ArchiveStatus, NOTE_EMPTY, NOTE_EMPTY_HINT,
        NOTE_ERROR_HINT, NOTE_LOADING, PAGE_TITLE, RowKey, gen_group_title, item_status,
        row_actions, row_summary,
    };
    use nous_engine::types::{
        ArchiveGen, ArchiveGens, ArchiveItem, ArchiveTombstone, SessionParent,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn tombstone(gen_name: &str, purged: bool) -> ArchiveTombstone {
        ArchiveTombstone {
            r#gen: gen_name.to_string(),
            purged,
            at: None,
            extra: HashMap::new(),
        }
    }

    fn item(
        id: i64,
        name: &str,
        live: bool,
        archived: Option<nous_engine::types::ArchiveTombstone>,
    ) -> ArchiveItem {
        ArchiveItem {
            id,
            name: name.to_string(),
            parent: None,
            entries: 0,
            last: None,
            archived,
            live,
            extra: HashMap::new(),
        }
    }

    fn group(gen_name: &str, items: Vec<ArchiveItem>) -> ArchiveGen {
        ArchiveGen {
            r#gen: gen_name.to_string(),
            items,
            extra: HashMap::new(),
        }
    }

    // ── 分组标题 ────────────────────────────────────────────────

    /// 票上的那条：`20261110` → `11月10日 · gen-20261110.sqlite`。
    #[test]
    fn group_title_formats_the_day_and_the_file() {
        assert_eq!(
            gen_group_title("20261110"),
            "11月10日 · gen-20261110.sqlite"
        );
    }

    /// 前导零：月和日都补两位（设计稿里是 `11月07日` / `01月05日`）。
    #[test]
    fn group_title_pads_month_and_day() {
        assert_eq!(
            gen_group_title("20261107"),
            "11月07日 · gen-20261107.sqlite"
        );
        assert_eq!(
            gen_group_title("20260105"),
            "01月05日 · gen-20260105.sqlite"
        );
    }

    /// 引擎给的是 `gen` 串，但别处传文件名/破折号写法也得认。
    #[test]
    fn group_title_accepts_file_name_and_dashed_date() {
        assert_eq!(
            gen_group_title("gen-20261110.sqlite"),
            "11月10日 · gen-20261110.sqlite"
        );
        assert_eq!(
            gen_group_title("2026-11-10"),
            "11月10日 · gen-20261110.sqlite"
        );
    }

    /// 脏数据不编日期、不 panic：文件名照旧，日期那截用原串。
    #[test]
    fn group_title_keeps_unparseable_stems_as_is() {
        assert_eq!(
            gen_group_title("20261301"),
            "20261301 · gen-20261301.sqlite"
        );
        assert_eq!(gen_group_title("probe"), "probe · gen-probe.sqlite");
    }

    // ── 四态（照 mcfpqm 的 stateOf）──────────────────────────────

    /// `archived.purged` 最优先：就算引擎还说 live，也是已销毁。
    #[test]
    fn purged_wins_over_live() {
        let it = item(302, "对话 302", true, Some(tombstone("20261110", true)));
        assert_eq!(item_status(&it), ArchiveStatus::Purged);

        let gone = item(302, "对话 302", false, Some(tombstone("20261110", true)));
        assert_eq!(item_status(&gone), ArchiveStatus::Purged);
    }

    /// `live` + 有墓碑 → 原件在（可以撕）。
    #[test]
    fn live_with_tombstone_is_original_still_here() {
        let it = item(302, "对话 302", true, Some(tombstone("20261110", false)));
        assert_eq!(item_status(&it), ArchiveStatus::Live);
    }

    /// `live` + `archived: null` → 已捞回。**判据里没有主列表**：
    /// 列表是别处的事，四态只看 item 自己。
    #[test]
    fn live_without_tombstone_is_restored() {
        let it = item(302, "对话 302", true, None);
        assert_eq!(item_status(&it), ArchiveStatus::Restored);
    }

    /// 复印件在、活库没有、墓碑也没有 → 无墓碑。
    #[test]
    fn no_tombstone_and_not_live_is_orphan() {
        let it = item(302, "对话 302", false, None);
        assert_eq!(item_status(&it), ArchiveStatus::Orphan);
    }

    /// 墓碑在、原件没了、却也没 `purged` —— `mcfpqm` 也落到最后一档。
    #[test]
    fn tombstone_without_purge_and_not_live_falls_to_orphan() {
        let it = item(302, "对话 302", false, Some(tombstone("20261110", false)));
        assert_eq!(item_status(&it), ArchiveStatus::Orphan);
    }

    // ── 动作 ────────────────────────────────────────────────────

    /// 原件在：只有撕原件（打捞不给 —— 活库那条还在）。
    #[test]
    fn live_row_offers_purge_only() {
        let it = item(302, "对话 302", true, Some(tombstone("20261110", false)));
        assert_eq!(
            row_actions(&it, "20261110", ArchiveStatus::Live),
            vec![ArchiveAction::Purge {
                id: 302,
                name: "对话 302".into()
            }]
        );
    }

    /// 已销毁 / 无墓碑：只有打捞。
    #[test]
    fn purged_and_orphan_rows_offer_restore_only() {
        let it = item(302, "对话 302", false, Some(tombstone("20261110", true)));
        let restore = vec![ArchiveAction::Restore {
            id: 302,
            r#gen: "20261110".into(),
        }];
        assert_eq!(row_actions(&it, "20261110", ArchiveStatus::Purged), restore);
        assert_eq!(row_actions(&it, "20261110", ArchiveStatus::Orphan), restore);
    }

    /// 已捞回：没东西可做，不给按钮。
    #[test]
    fn restored_row_offers_nothing() {
        let it = item(302, "对话 302", true, None);
        assert!(row_actions(&it, "20261110", ArchiveStatus::Restored).is_empty());
    }

    /// 动作只产出枚举，`r#gen` / `name` 原样带出去（调用方按它发请求、弹确认）。
    #[test]
    fn actions_carry_id_gen_and_name() {
        let it = item(7, "旧探针 2", false, Some(tombstone("20261118", true)));
        match &row_actions(&it, "20261118", ArchiveStatus::Purged)[0] {
            ArchiveAction::Restore { id, r#gen } => {
                assert_eq!(*id, 7);
                assert_eq!(r#gen, "20261118");
            }
            other => panic!("应该是打捞，实际 {other:?}"),
        }

        let live = item(
            8,
            "zz-探针（可删）",
            true,
            Some(tombstone("20261118", false)),
        );
        match &row_actions(&live, "20261118", ArchiveStatus::Live)[0] {
            ArchiveAction::Purge { id, name } => {
                assert_eq!(*id, 8);
                assert_eq!(name, "zz-探针（可删）");
            }
            other => panic!("应该是撕原件，实际 {other:?}"),
        }
    }

    // ── 行尾摘要 ────────────────────────────────────────────────

    /// 有 `last.text` 就用它：空白压成一个空格，超过 72 字截断。
    #[test]
    fn summary_prefers_last_text_flattened_and_truncated() {
        let mut it = item(302, "对话 302", true, None);
        it.last = Some(json!({"text": "  第一行\n第二行\t第三行  "}));
        assert_eq!(row_summary(&it), "第一行 第二行 第三行");

        let long = "字".repeat(100);
        it.last = Some(json!({ "text": long }));
        assert_eq!(row_summary(&it).chars().count(), 72);
    }

    /// 没有 `last.text` → `← #<parent.id> 分叉`。
    #[test]
    fn summary_falls_back_to_parent_branch() {
        let mut it = item(302, "对话 302", true, None);
        it.parent = Some(SessionParent {
            id: 296,
            at: Some(json!(50)),
        });
        assert_eq!(row_summary(&it), "← #296 分叉");
    }

    /// 再没有 → `<entries> 条记录`；`entries` 是数字不是布尔。
    #[test]
    fn summary_falls_back_to_entry_count() {
        let mut it = item(302, "对话 302", true, None);
        it.entries = 12;
        assert_eq!(row_summary(&it), "12 条记录");
    }

    /// 都没有（`entries: 0` 也算没有）→ 空会话。
    #[test]
    fn summary_falls_back_to_empty_session() {
        assert_eq!(row_summary(&item(302, "对话 302", true, None)), "空会话");

        let mut zero = item(302, "对话 302", true, None);
        zero.entries = 0;
        assert_eq!(row_summary(&zero), "空会话");

        let mut null_last = item(302, "对话 302", true, None);
        null_last.last = Some(json!({"text": null}));
        assert_eq!(row_summary(&null_last), "空会话");
    }

    // ── 文案 ────────────────────────────────────────────────────

    /// 「彻底删除」这四个字不许出现在这一页 —— 主列表/右键菜单也不许。
    /// 这里扫所有会落到屏幕上的固定文案（徽标、动作、空态、错误提示）。
    #[test]
    fn page_words_never_say_delete_forever() {
        let mut texts: Vec<&str> = vec![
            PAGE_TITLE,
            NOTE_LOADING,
            NOTE_EMPTY,
            NOTE_EMPTY_HINT,
            NOTE_ERROR_HINT,
            ArchiveAction::Restore {
                id: 1,
                r#gen: String::new(),
            }
            .label(),
            ArchiveAction::Purge {
                id: 1,
                name: String::new(),
            }
            .label(),
            ArchiveAction::Refresh.label(),
        ];
        for status in ArchiveStatus::ALL {
            texts.push(status.label());
        }
        for text in texts {
            assert!(
                !text.contains("彻底删除"),
                "这一页出现了「彻底删除」：{text}"
            );
        }
    }

    // ── 页面状态 ────────────────────────────────────────────────

    /// 条数跨 gen 求和；空数据就是空页。
    #[test]
    fn state_counts_items_across_gens() {
        let empty = ArchivePageState::new();
        assert!(empty.is_empty());
        assert_eq!(empty.total(), 0);

        let state = ArchivePageState {
            data: ArchiveGens {
                gens: vec![
                    group(
                        "20261110",
                        vec![
                            item(302, "对话 302", true, None),
                            item(303, "对话 303", false, None),
                        ],
                    ),
                    group("20261107", vec![item(201, "一次失败的尝试", true, None)]),
                ],
                extra: HashMap::new(),
            },
            ..Default::default()
        };
        assert_eq!(state.total(), 3);
        assert!(!state.is_empty());
    }

    /// 选中是按 (gen, id) 认的，不是只认 id。
    #[test]
    fn selection_is_keyed_by_gen_and_id() {
        let mut state = ArchivePageState::new();
        let key = RowKey {
            r#gen: "20261110".into(),
            id: 302,
        };
        state.select(key.clone());
        assert_eq!(state.selected, Some(key));
        assert_ne!(
            state.selected,
            Some(RowKey {
                r#gen: "20261107".into(),
                id: 302,
            })
        );
        state.clear_selection();
        assert_eq!(state.selected, None);
    }
}
