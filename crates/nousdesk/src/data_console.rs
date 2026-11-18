//! 左列第四个入口：**数据监控室** —— 把面板 `d6s4v1`（`agent.sqlite` 的 X 光机）
//! 和 `mcfpqm`（归档库）搬进桌面端。
//!
//! 三列，宽度从设计稿来（`design/d9-*`，跑在 GPUI 里的这一版是 D9-G）：
//!
//! ```text
//! ┌──────────┬────────────────────────────┬───────────────┐
//! │ 拉片 320 │ 六个屏（解剖/会话树/回放/…）│ 时间线 392    │
//! │ 十个读数 │ 一个整页，切 tab           │ 可收成 48 竖条 │
//! └──────────┴────────────────────────────┴───────────────┘
//! ```
//!
//! 三条规矩（跟面板那边对齐，别自己发明）：
//!
//! - **这页不算业务状态**。数字全是面板算好的，这边只格式化、排布。
//!   唯一“算”的是时间线：把五份数据里的事件按时间排一排 —— 那也是纯展示聚合。
//! - **归档四态只有一份实现**：[`crate::archive_page::item_status`]。这里不重写判据。
//! - **失败要留话**。哪一份挂了就那一屏写红字，其余照画；不整页白屏。

use std::rc::Rc;
use std::time::Duration;

use chrono::{Local, TimeZone as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::console::{
    ConsoleClient, ConsoleError, Conversation as Replay, Conversations, ConvRow, Cost, CostBucket,
    Health, Overview,
};
use nous_engine::types::{ArchiveGens, ArchiveItem};

use crate::app::{Bar, Desktop};
use crate::archive_page::{ArchiveStatus, item_status};

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

pub const PAGE_TITLE: &str = "数据监控室";
/// 拉片列宽（三档：宽卡 / 胶卷格 / 只剩数字）。
const FILM_W_WIDE: f32 = 320.;
const FILM_W_TIGHT: f32 = 236.;
const FILM_W_NANO: f32 = 64.;
/// 时间线列宽；收起来只剩一条竖排标签。
const TL_W: f32 = 392.;
const TL_W_FOLDED: f32 = 48.;

// ── 屏 ───────────────────────────────────────────────────────────────

/// 中列那六屏。顺序就是标签顺序 —— 「解剖 → 会话树 → 回放 → 账单 → 维修 → 归档」
/// 是从「库里有什么」到「出了什么事」再到「撕掉的东西还回不回得来」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Overview,
    Convs,
    Replay,
    Cost,
    Health,
    Archive,
}

impl Screen {
    pub const ALL: [Screen; 6] = [
        Screen::Overview,
        Screen::Convs,
        Screen::Replay,
        Screen::Cost,
        Screen::Health,
        Screen::Archive,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Screen::Overview => "解剖",
            Screen::Convs => "会话树",
            Screen::Replay => "回放",
            Screen::Cost => "账单",
            Screen::Health => "维修",
            Screen::Archive => "归档",
        }
    }

    /// 这一屏在讲什么 —— 挂在小标题后面，省得用户猜「维修」是修什么。
    pub fn hint(self) -> &'static str {
        match self {
            Screen::Overview => "文件 / pragmas / 表 / 索引 / integrity",
            Screen::Convs => "一条会话一行，按 token 排",
            Screen::Replay => "一条会话里每一轮发生了什么",
            Screen::Cost => "token 从哪来、缓存命中多少",
            Screen::Health => "卡住的任务 / 没人接的提交 / WAL",
            Screen::Archive => "撕掉的原件还回不回得来",
        }
    }
}

/// 拉片的三档：宽卡（03 原样）/ 胶卷格（值 + 一行）/ 只剩数值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilmSize {
    #[default]
    Wide,
    Tight,
    Nano,
}

impl FilmSize {
    pub const ALL: [FilmSize; 3] = [FilmSize::Wide, FilmSize::Tight, FilmSize::Nano];

    pub fn label(self) -> &'static str {
        match self {
            FilmSize::Wide => "宽",
            FilmSize::Tight => "窄",
            FilmSize::Nano => "数",
        }
    }

    pub fn width(self) -> f32 {
        match self {
            FilmSize::Wide => FILM_W_WIDE,
            FilmSize::Tight => FILM_W_TIGHT,
            FilmSize::Nano => FILM_W_NANO,
        }
    }
}

/// 时间线的过滤档。跟面板的五个来源一一对应，不多不少。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlFilter {
    #[default]
    All,
    Archive,
    Conv,
    Task,
    Cost,
}

impl TlFilter {
    pub const ALL: [TlFilter; 5] = [
        TlFilter::All,
        TlFilter::Archive,
        TlFilter::Conv,
        TlFilter::Task,
        TlFilter::Cost,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TlFilter::All => "全部",
            TlFilter::Archive => "归档",
            TlFilter::Conv => "会话",
            TlFilter::Task => "任务",
            TlFilter::Cost => "账本",
        }
    }
}

/// 时间线上一条事件的来源。画的时候用它上色，过滤也认它。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Archive,
    Conv,
    Task,
    Cost,
}

impl Kind {
    pub fn filter(self) -> TlFilter {
        match self {
            Kind::Archive => TlFilter::Archive,
            Kind::Conv => TlFilter::Conv,
            Kind::Task => TlFilter::Task,
            Kind::Cost => TlFilter::Cost,
        }
    }

    pub fn label(self) -> &'static str {
        self.filter().label()
    }
}

/// 时间线一条。`at` 是 epoch 毫秒。
#[derive(Debug, Clone)]
pub struct Event {
    pub kind: Kind,
    pub at: i64,
    pub title: String,
    pub detail: String,
    /// 右边那行小字（「已销毁」「原件在」…）。
    pub tail: String,
    /// 点它跳到哪一屏。
    pub screen: Screen,
}

// ── 读数 ─────────────────────────────────────────────────────────────

/// 拉片上的一个读数。`frac` 是那条迷你条的占比（0..=1），`dots` 是归档的四态点。
#[derive(Debug, Clone)]
pub struct Reading {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub unit: &'static str,
    pub note: String,
    pub frac: f32,
    /// 只有「归档」这一格有：四种状态各几个，按 gone / here / back / orphan 排。
    pub dots: Option<[usize; 4]>,
}

impl Reading {
    /// 点这个读数跳到哪一屏。跟设计稿里「点读数换屏」一致。
    pub fn screen(&self) -> Screen {
        match self.key {
            "db" | "wal" | "docs" => Screen::Overview,
            "entries" | "convs" => Screen::Convs,
            "tokens" | "cache" => Screen::Cost,
            "waiting" | "stuck" => Screen::Health,
            "archive" => Screen::Archive,
            _ => Screen::Overview,
        }
    }
}

// ── 状态 ─────────────────────────────────────────────────────────────

/// 四份面板数据的只读快照。`readings` / `events` 都是纯函数，只认这个 ——
/// 不认 `ConsoleState`（那玩意带 `ScrollHandle`，进测试会把 rustc 撑爆）。
#[derive(Clone, Copy, Default)]
pub struct Snapshot<'a> {
    pub overview: Option<&'a Overview>,
    pub convs: Option<&'a Conversations>,
    pub cost: Option<&'a Cost>,
    pub health: Option<&'a Health>,
}

/// 这一页的全部状态。五份数据各一个槽：谁的请求挂了只影响谁。
#[derive(Default)]
pub struct ConsoleState {
    pub overview: Option<Overview>,
    pub convs: Option<Conversations>,
    pub replay: Option<Replay>,
    pub cost: Option<Cost>,
    pub health: Option<Health>,
    pub archive: Option<ArchiveGens>,

    /// 四份面板数据正在拉（回放单独算）。
    pub loading: bool,
    /// 拉失败的原话。原样显示，不翻译不加前缀。
    pub error: Option<String>,
    /// 回放那一条单独的错误（它按需拉，失败不该把整页染红）。
    pub replay_error: Option<String>,

    pub screen: Screen,
    pub film: FilmSize,
    /// 当前选中的读数 key（拉片高亮 + 顶栏说明）。
    pub picked: &'static str,
    /// 时间线展开着吗。默认展开 —— 收起是个「我看够了」的动作，不该是默认。
    pub tl_open: bool,
    pub tl_filter: TlFilter,
    /// 回放看的是哪条会话。`None` 时选会话树里最大的那条。
    pub replay_id: Option<i64>,

    pub film_scroll: ScrollHandle,
    pub screen_scroll: ScrollHandle,
    pub tl_scroll: ScrollHandle,
}

impl ConsoleState {
    pub fn new() -> Self {
        Self {
            screen: Screen::Overview,
            film: FilmSize::Wide,
            picked: "db",
            tl_open: true,
            tl_filter: TlFilter::All,
            ..Default::default()
        }
    }

    /// 五份数据的一份只读快照。渲染和测试都走它 —— 单测不该为了算十个读数
    /// 去造一个带 `ScrollHandle` 的 `ConsoleState`（那玩意一进 `#[test]`
    /// 就把 rustc 的宏展开递归撑爆，试过 2048 也不够）。
    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            overview: self.overview.as_ref(),
            convs: self.convs.as_ref(),
            cost: self.cost.as_ref(),
            health: self.health.as_ref(),
        }
    }

    pub fn readings(&self, archive: Option<&ArchiveGens>) -> Vec<Reading> {
        readings(self.snapshot(), archive)
    }

    /// 按 key 取读数（给顶栏 / 底栏那句「选中了什么」）。
    pub fn reading(&self, key: &str, archive: Option<&ArchiveGens>) -> Option<Reading> {
        self.readings(archive).into_iter().find(|r| r.key == key)
    }

    pub fn events(&self, archive: Option<&ArchiveGens>) -> Vec<Event> {
        events(self.snapshot(), archive)
    }

    pub fn timeline(&self, archive: Option<&ArchiveGens>) -> Vec<Event> {
        timeline(self.snapshot(), archive, self.tl_filter)
    }

    /// 五份数据都拿到手了吗（回放除外 —— 它是按需的）。
    pub fn hydrated(&self) -> bool {
        self.overview.is_some()
            && self.convs.is_some()
            && self.cost.is_some()
            && self.health.is_some()
    }

}

/// 十个读数。缺数据的那条给 `—`，不给 0 —— 0 是「真的没有」，`—` 是「还没拿到」。
pub fn readings(s: Snapshot<'_>, archive: Option<&ArchiveGens>) -> Vec<Reading> {
        let ov = s.overview;
        let cost = s.cost;
        let health = s.health;
        let convs = s.convs;

        let db = ov.map(|o| o.file.db).unwrap_or(0);
        let wal = ov.map(|o| o.file.wal).unwrap_or(0);
        let entries = ov.map(|o| o.counts.get("entries")).unwrap_or(0);
        let conv_n = ov.map(|o| o.counts.get("conversations")).unwrap_or(0);
        let pages = ov.map(|o| o.pragmas.page_count).unwrap_or(0);
        let page_size = ov.map(|o| o.pragmas.page_size).unwrap_or(0);

        let (offline_dots, archive_total) = match archive {
            Some(gens) => {
                let mut dots = [0usize; 4];
                let mut total = 0;
                for g in &gens.gens {
                    for item in &g.items {
                        total += 1;
                        dots[status_idx(item_status(item))] += 1;
                    }
                }
                (Some(dots), total)
            }
            None => (None, 0),
        };

        let unanswered = health.map(|h| h.submissions.unanswered.len()).unwrap_or(0);
        let stuck = health.map(|h| h.tasks.stuck.len()).unwrap_or(0);
        let docs = health.map(|h| h.documents.revisions).unwrap_or(0);
        let base = health.map(|h| h.documents.base).unwrap_or(0);
        let delta = health.map(|h| h.documents.delta).unwrap_or(0);

        let tokens = cost.map(|c| c.totals.tokens).unwrap_or(0);
        let out_tokens = cost.map(|c| c.totals.output).unwrap_or(0);
        let cache_read = cost.map(|c| c.totals.cache_read).unwrap_or(0);
        let input = cost.map(|c| c.totals.input).unwrap_or(0);
        let hit = cost.map(|c| c.totals.cache_hit()).unwrap_or(0.);
        let calls = cost.map(|c| c.totals.entries).unwrap_or(0);

        let missing = "—".to_string();
        let num = |n: i64| if ov.is_some() { fmt_num(n) } else { missing.clone() };

        vec![
            Reading {
                key: "db",
                label: "库文件",
                value: if ov.is_some() {
                    fmt_bytes_short(db)
                } else {
                    missing.clone()
                },
                unit: "",
                note: format!("{} 页 · {}B", fmt_num(pages), fmt_num(page_size)),
                frac: if db > 0 { 0.72 } else { 0. },
                dots: None,
            },
            Reading {
                key: "wal",
                label: "WAL",
                value: if ov.is_some() {
                    fmt_bytes_short(wal)
                } else {
                    missing.clone()
                },
                unit: "",
                note: if ov.is_some() && wal > 0 {
                    "还没 checkpoint".to_string()
                } else if ov.is_some() {
                    "干净".to_string()
                } else {
                    missing.clone()
                },
                frac: if wal > 0 { 0.2 } else { 0. },
                dots: None,
            },
            Reading {
                key: "entries",
                label: "ENTRIES",
                value: num(entries),
                unit: "条",
                note: if entries > 0 {
                    format!("平均 {} B / 条", fmt_num(db.max(entries) / entries))
                } else {
                    missing.clone()
                },
                frac: 0.55,
                dots: None,
            },
            Reading {
                key: "convs",
                label: "会话",
                value: num(conv_n),
                unit: "条",
                note: match convs {
                    Some(c) => format!(
                        "{} entries · {} 条带工具",
                        fmt_num(c.totals.entries),
                        c.list.iter().filter(|r| r.tool_calls > 0).count()
                    ),
                    None => missing.clone(),
                },
                frac: 0.3,
                dots: None,
            },
            Reading {
                key: "tokens",
                label: "TOKENS",
                value: if cost.is_some() {
                    fmt_num(tokens)
                } else {
                    missing.clone()
                },
                unit: "",
                note: if cost.is_some() {
                    format!("{} 次生成 · 输出 {}", fmt_num(calls), fmt_num(out_tokens))
                } else {
                    missing.clone()
                },
                frac: tokens as f32 / 800_000.,
                dots: None,
            },
            Reading {
                key: "cache",
                label: "缓存命中",
                value: if cost.is_some() {
                    format!("{:.0}%", hit * 100.)
                } else {
                    missing.clone()
                },
                unit: "",
                note: if cost.is_some() {
                    format!("读 {} / 入 {}", fmt_num(cache_read), fmt_num(input))
                } else {
                    missing.clone()
                },
                frac: hit,
                dots: None,
            },
            Reading {
                key: "archive",
                label: "归档",
                value: if archive.is_some() {
                    fmt_num(archive_total as i64)
                } else {
                    missing.clone()
                },
                unit: "条",
                note: match archive {
                    Some(g) => {
                        let gens = g.gens.len();
                        format!("{gens} 代 · 四态")
                    }
                    None => missing.clone(),
                },
                frac: 0.4,
                dots: offline_dots,
            },
            Reading {
                key: "waiting",
                label: "待答输入",
                value: if health.is_some() {
                    fmt_num(unanswered as i64)
                } else {
                    missing.clone()
                },
                unit: "条",
                note: match health {
                    Some(h) => format!("{} submission 里", fmt_num(h.submissions.total)),
                    None => missing.clone(),
                },
                frac: (unanswered as f32 / 30.).min(1.),
                dots: None,
            },
            Reading {
                key: "stuck",
                label: "卡住的任务",
                value: if health.is_some() {
                    fmt_num(stuck as i64)
                } else {
                    missing.clone()
                },
                unit: "个",
                note: if health.is_some() && stuck == 0 {
                    "没有活挂着的".to_string()
                } else if health.is_some() {
                    "要去看一眼".to_string()
                } else {
                    missing.clone()
                },
                frac: if stuck > 0 { 1. } else { 0. },
                dots: None,
            },
            Reading {
                key: "docs",
                label: "文档 REVISION",
                value: if health.is_some() {
                    fmt_num(docs)
                } else {
                    missing.clone()
                },
                unit: "条",
                note: if health.is_some() {
                    format!("{} base / {} delta", fmt_num(base), fmt_num(delta))
                } else {
                    missing.clone()
                },
                frac: if docs > 0 { delta as f32 / docs as f32 + 0.35 } else { 0. },
                dots: None,
            },
        ]
    }

/// 时间线事件。纯展示聚合：归档条目 + 会话的最后一条 + 最慢的任务 + 按天账本。
pub fn events(s: Snapshot<'_>, archive: Option<&ArchiveGens>) -> Vec<Event> {
        let mut out: Vec<Event> = Vec::new();

        if let Some(gens) = archive {
            for g in &gens.gens {
                for item in &g.items {
                    let status = item_status(item);
                    out.push(Event {
                        kind: Kind::Archive,
                        at: tombstone_at(item),
                        title: item.name.clone(),
                        detail: format!("{} 条 entries · {}", item.entries, status.label()),
                        tail: status.label().to_string(),
                        screen: Screen::Archive,
                    });
                }
            }
        }

        if let Some(c) = s.convs {
            for row in &c.list {
                out.push(Event {
                    kind: Kind::Conv,
                    at: row.last_at,
                    title: one_line(&row.title, 60),
                    detail: format!("{} entries · {} tokens", row.entries, fmt_num(row.tokens)),
                    tail: format!("#{}", row.id),
                    screen: Screen::Convs,
                });
            }
        }

        if let Some(h) = s.health {
            for task in &h.tasks.slowest {
                out.push(Event {
                    kind: Kind::Task,
                    at: task.started_at,
                    title: format!("#{} · {}", task.id, task.kind),
                    detail: format!(
                        "{} · {} · {}",
                        task.status,
                        task.outcome.as_deref().unwrap_or("—"),
                        fmt_ms(task.ms())
                    ),
                    tail: format!("#{}", task.conv),
                    screen: Screen::Health,
                });
            }
        }

        if let Some(c) = s.cost {
            for day in &c.by_day {
                // 账本按天，没有「几点」—— 放在当天正午，免得全挤在 00:00。
                out.push(Event {
                    kind: Kind::Cost,
                    at: day.start_of_day_ms(),
                    title: format!("{} · {} tokens", day.label, fmt_num(day.tokens)),
                    detail: format!(
                        "{} 次调用 · 入 {} / 出 {}",
                        fmt_num(day.calls),
                        fmt_num(day.input),
                        fmt_num(day.output)
                    ),
                    tail: "账本".to_string(),
                    screen: Screen::Cost,
                });
            }
        }

        // 新的在上。同一毫秒的（账本那批）用类型名兜底排一下，免得每次重画顺序都变。
        out.sort_by(|a, b| {
            b.at
                .cmp(&a.at)
                .then_with(|| a.kind.label().cmp(b.kind.label()))
                .then_with(|| a.title.cmp(&b.title))
        });
        out
    }

/// 过滤后的时间线。
pub fn timeline(
    s: Snapshot<'_>,
    archive: Option<&ArchiveGens>,
    filter: TlFilter,
) -> Vec<Event> {
    events(s, archive)
        .into_iter()
        .filter(|e| filter == TlFilter::All || e.kind.filter() == filter)
        .collect()
}

/// 四态在 dots 里的下标。顺序是 `archive_page::ArchiveStatus` 的声明顺序：
/// 原件在 / 已销毁 / 已捞回 / 无墓碑。
fn status_idx(s: ArchiveStatus) -> usize {
    match s {
        ArchiveStatus::Live => 0,
        ArchiveStatus::Purged => 1,
        ArchiveStatus::Restored => 2,
        ArchiveStatus::Orphan => 3,
    }
}

/// 归档事件该排在哪一刻：墓碑上那个 `at`（可以是数字、数字串、缺席）。
fn tombstone_at(item: &ArchiveItem) -> i64 {
    item.archived
        .as_ref()
        .and_then(|t| t.at.as_ref())
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0)
}

// ── 小工具 ───────────────────────────────────────────────────────────

/// 1024 进制、一位小数。1.1 MB / 282 KB / 912 B。
pub fn fmt_bytes_short(n: i64) -> String {
    let n = n.max(0) as f64;
    if n >= 1024. * 1024. {
        format!("{:.1} MB", n / 1024. / 1024.)
    } else if n >= 1024. {
        format!("{:.0} KB", n / 1024.)
    } else {
        format!("{:.0} B", n)
    }
}

/// 千分位。592866 → 592,866。
pub fn fmt_num(n: i64) -> String {
    let neg = n < 0;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

/// 一行里的换行/制表都压成空格，再截断。会话标题里有控制字符（`坑.md`）。
pub fn one_line(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = flat.chars().count();
    if n <= max {
        return flat;
    }
    let mut s: String = flat.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

/// epoch 毫秒 → 「11-14 17:53」（本地时区）。0 / 负数回 "—"。
pub fn fmt_clock(ms: i64) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    match Local.timestamp_millis_opt(ms).single() {
        Some(dt) => dt.format("%m-%d %H:%M").to_string(),
        None => "—".to_string(),
    }
}

/// 毫秒 → 「3.8s」/「12s」/「1.2m」。
pub fn fmt_ms(ms: i64) -> String {
    let ms = ms.max(0);
    if ms >= 60_000 {
        format!("{:.1}m", ms as f64 / 60_000.)
    } else if ms >= 10_000 {
        format!("{:.0}s", ms as f64 / 1000.)
    } else {
        format!("{:.1}s", ms as f64 / 1000.)
    }
}

/// 账本按天分组时，面板只给 `label`（`10-28` 这种），没有时间戳。
/// 时间线要排序，只能自己补一个当天 12:00 的毫秒值 —— 解不出来就排到最后。
pub(crate) trait BucketAt {
    fn start_of_day_ms(&self) -> i64;
}

impl BucketAt for CostBucket {
    fn start_of_day_ms(&self) -> i64 {
        let md = self.label.trim();
        let now = Local::now();
        let year = now.date_naive().format("%Y").to_string();
        for fmt in ["%Y-%m-%d", "%m-%d"] {
            let src = if fmt == "%m-%d" {
                format!("{year}-{md}")
            } else {
                md.to_string()
            };
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&src, "%Y-%m-%d") {
                let dt = d.and_hms_opt(12, 0, 0).unwrap_or_default();
                return Local
                    .from_local_datetime(&dt)
                    .single()
                    .map(|d| d.timestamp_millis())
                    .unwrap_or(0);
            }
        }
        0
    }
}

// ── 拉取 ─────────────────────────────────────────────────────────────

impl Desktop {
    /// 拉这一页要的一切：面板四份 + 归档库。
    ///
    /// 四份**并发**拉（内网也快，但串行就是白等）；谁挂了只写谁的红字，
    /// 不让一份 500 把整页染红 —— 维修页挂了还看得了解剖页。
    pub(crate) fn load_console(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.console.loading {
            return;
        }
        if !force && self.console.hydrated() && self.console.archive.is_some() {
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.console.error = Some("还没连上引擎 —— 去「≡ 更多」里重新连接".to_string());
            cx.notify();
            return;
        };
        let cfg = self.conn.clone();
        self.console.loading = true;
        self.console.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let got = handle
                .spawn(async move {
                    let client =
                        ConsoleClient::new(&cfg.host, Duration::from_secs(15))?;
                    let (ov, cv, co, he) = tokio::join!(
                        client.overview(),
                        client.conversations(),
                        client.cost(),
                        client.health()
                    );
                    let arch = engine.archive_list().await;
                    Ok::<_, ConsoleError>((ov, cv, co, he, arch))
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.console.loading = false;
                match got {
                    Ok(Ok((ov, cv, co, he, arch))) => {
                        let mut errs: Vec<String> = Vec::new();
                        // 四份各自落地：一份 500 只让它自己那屏写红字，别的照画。
                        match ov {
                            Ok(v) => this.console.overview = Some(v),
                            Err(e) => {
                                log::warn!("数据监控室 · 解剖：{e}");
                                errs.push(format!("解剖：{e}"));
                            }
                        }
                        match cv {
                            Ok(v) => this.console.convs = Some(v),
                            Err(e) => {
                                log::warn!("数据监控室 · 会话树：{e}");
                                errs.push(format!("会话树：{e}"));
                            }
                        }
                        match co {
                            Ok(v) => this.console.cost = Some(v),
                            Err(e) => {
                                log::warn!("数据监控室 · 账单：{e}");
                                errs.push(format!("账单：{e}"));
                            }
                        }
                        match he {
                            Ok(v) => this.console.health = Some(v),
                            Err(e) => {
                                log::warn!("数据监控室 · 维修：{e}");
                                errs.push(format!("维修：{e}"));
                            }
                        }
                        match arch {
                            Ok(v) => this.console.archive = Some(v),
                            Err(e) => {
                                log::warn!("数据监控室 · 归档：{e}");
                                errs.push(format!("归档：{e}"));
                            }
                        }
                        this.console.error = (!errs.is_empty()).then(|| errs.join(" · "));
                    }
                    Ok(Err(e)) => {
                        this.console.error = Some(e.to_string());
                        log::warn!("数据监控室 · 拉取任务：{e}");
                    }
                    Err(e) => {
                        this.console.error = Some(e.to_string());
                        log::warn!("数据监控室 · 拉取任务挂了：{e}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 回放那一条按需拉。切到「回放」屏或点了别的会话才发请求。
    pub(crate) fn load_replay(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.console.replay.as_ref().is_some_and(|r| r.id == id) {
            return;
        }
        let Some(handle) = crate::TOKIO.get().cloned() else {
            return;
        };
        let cfg = self.conn.clone();
        self.console.replay_id = Some(id);
        self.console.replay_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = handle
                .spawn(async move {
                    let client =
                        ConsoleClient::new(&cfg.host, Duration::from_secs(20))?;
                    client.conversation(id).await
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match got {
                    Ok(Ok(v)) => {
                        this.console.replay = Some(v);
                        this.console.replay_error = None;
                    }
                    Ok(Err(e)) => {
                        log::warn!("数据监控室 · 回放 #{id}：{e}");
                        this.console.replay_error = Some(e.to_string());
                    }
                    Err(e) => {
                        log::warn!("数据监控室 · 回放任务 #{id}：{e}");
                        this.console.replay_error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

// ── 渲染：整页 ───────────────────────────────────────────────────────

impl Desktop {
    pub(crate) fn render_data_console(&self, cx: &mut Context<Self>) -> AnyElement {
        let archive = self.console.archive.as_ref();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w(t().d(0.))
            .bg(rgb(t().p.panel))
            .child(self.console_top(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(self.film_column(cx))
                    .child(self.screen_column(cx))
                    .child(if self.console.tl_open {
                        self.timeline_column(cx)
                    } else {
                        self.timeline_rail(cx)
                    }),
            )
            .child(self.console_foot(archive))
            .into_any_element()
    }

    /// 顶栏：标题 + 只读标记 + 拉片档位 + 刷新。
    fn console_top(&self, cx: &mut Context<Self>) -> AnyElement {
        let loading = self.console.loading;
        div()
            .flex()
            .items_center()
            .gap(t().d(10.))
            .h(t().d(46.))
            .flex_none()
            .px(t().d(14.))
            .bg(rgb(t().p.panel))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(t().d(8.))
                    .child(
                        div()
                            .text_size(t().fs(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(PAGE_TITLE),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child(if loading {
                                "agent.sqlite · 拉取中…".to_string()
                            } else {
                                "agent.sqlite · 只读".to_string()
                            }),
                    ),
            )
            .child(badge("readOnly", t().p.accent_soft, t().p.accent_fg))
            .child(div().flex_1())
            .child(
                // 三档只改这一列的宽度，不动内容 —— 「数和窄」是同一份读数的三种密度。
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(2.))
                    .children(FilmSize::ALL.iter().enumerate().map(|(i, s)| {
                        let on = self.console.film == *s;
                        let size = *s;
                        div()
                            .id(ElementId::Name(format!("dc-film-{i}").into()))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.console.film = size;
                                cx.notify();
                            }))
                            .px(t().d(7.))
                            .py(t().d(2.))
                            .rounded(t().r(4.))
                            .text_size(t().fs(10.5))
                            .cursor_pointer()
                            .text_color(rgb(if on { t().p.accent_fg } else { t().p.muted }))
                            .when(on, |el| el.bg(rgb(t().p.accent_soft)))
                            .when(!on, |el| el.hover(|el| el.bg(rgb(t().p.hover))))
                            .child(s.label())
                    })),
            )
            .child(link_button(
                "dc-refresh",
                if loading { "刷新中…" } else { "刷新" },
                cx,
                |this, cx| this.load_console(true, cx),
            ))
            .into_any_element()
    }

    /// 底栏：选中了什么 + 键盘提示 + 快照时间 / schema。
    fn console_foot(&self, archive: Option<&ArchiveGens>) -> AnyElement {
        let picked = self.console.reading(self.console.picked, archive);
        let schema = self
            .console
            .overview
            .as_ref()
            .map(|o| o.pragmas.schema_version)
            .unwrap_or(0);
        let when = self
            .console
            .overview
            .as_ref()
            .map(|o| o.file.wal_mtime.max(o.file.mtime) as i64)
            .unwrap_or(0);
        div()
            .flex()
            .items_center()
            .gap(t().d(14.))
            .h(t().d(24.))
            .flex_none()
            .px(t().d(12.))
            .bg(rgb(t().p.panel))
            .border_t(t().d(1.))
            .border_color(rgb(t().p.line))
            .text_size(t().fs(9.5))
            .text_color(rgb(t().p.faint))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .child("选中")
                    .child(
                        div()
                            .text_color(rgb(t().p.text_2))
                            .child(match &picked {
                                Some(r) => format!("{} · {}{}", r.label, r.value, r.unit),
                                None => "—".to_string(),
                            }),
                    ),
            )
            .child(div().flex_1())
            .child("点读数换屏 · 点事件跳屏 · T 收起时间线")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .child("改动于")
                    .child(
                        div()
                            .text_color(rgb(t().p.text_2))
                            .child(if when > 0 { fmt_clock(when) } else { "—".into() }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(4.))
                    .child("schema")
                    .child(
                        div()
                            .text_color(rgb(t().p.text_2))
                            .child(format!("v{schema}")),
                    ),
            )
            .into_any_element()
    }
}

// ── 渲染：左列（拉片）────────────────────────────────────────────────

impl Desktop {
    fn film_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let archive = self.console.archive.as_ref();
        let readings = self.console.readings(archive);
        let n = readings.len();
        let size = self.console.film;
        let cards: Vec<AnyElement> = readings
            .iter()
            .map(|r| self.film_card(r, size, cx))
            .collect();

        div()
            .relative()
            .flex()
            .flex_col()
            .w(t().d(size.width()))
            .flex_none()
            .min_h(t().d(0.))
            .bg(rgb(t().p.panel_alt))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(30.))
                    .flex_none()
                    .px(t().d(10.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child(div().flex().items_center().gap(t().d(4.)).child("读数").child(
                        div().text_color(rgb(t().p.text_2)).child(format!("{n}")),
                    ))
                    .child(div().flex_1())
                    .child(
                        // 档位按钮：宽 / 窄 / 数。放这儿而不是顶栏，因为改的是这一列。
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(2.))
                            .children(FilmSize::ALL.iter().enumerate().map(|(i, s)| {
                                let on = *s == size;
                                let pick = *s;
                                div()
                                    .id(ElementId::Name(format!("dc-strip-{i}").into()))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.console.film = pick;
                                        cx.notify();
                                    }))
                                    .px(t().d(5.))
                                    .py(t().d(1.))
                                    .rounded(t().r(3.))
                                    .cursor_pointer()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(if on {
                                        t().p.accent_fg
                                    } else {
                                        t().p.muted
                                    }))
                                    .when(on, |el| el.bg(rgb(t().p.accent_soft)))
                                    .when(!on, |el| el.hover(|el| el.bg(rgb(t().p.hover))))
                                    .child(s.label())
                            })),
                    ),
            )
            .child(
                div()
                    // 齿孔那一条：胶片片的边。纯装饰，不接任何事件。
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(perforation(size != FilmSize::Nano))
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h(t().d(0.))
                            .min_w(t().d(0.))
                            .id("dc-film-scroll")
                            .overflow_y_scroll()
                            .track_scroll(&self.console.film_scroll)
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.bar_hover(Bar::DataFilm, *hovered, cx)
                            }))
                            .children(cards),
                    )
                    .child(crate::app::scrollbar::overlay(
                        "dc-film-bar",
                        &self.console.film_scroll,
                        self.bar(Bar::DataFilm).0,
                        self.bar(Bar::DataFilm).1,
                    )),
            )
            .into_any_element()
    }

    /// 一张胶片格。三档只改字号和内边距，内容顺序一致。
    fn film_card(&self, r: &Reading, size: FilmSize, cx: &mut Context<Self>) -> AnyElement {
        let on = self.console.picked == r.key;
        let key = r.key;
        let screen = r.screen();
        let big = match size {
            FilmSize::Wide => 19.,
            FilmSize::Tight => 15.,
            FilmSize::Nano => 12.,
        };

        let mut card = div()
            .id(ElementId::Name(format!("dc-film-card-{key}").into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.console.picked = key;
                this.console.screen = screen;
                if screen == Screen::Replay {
                    let id = this.console.replay_id.or_else(|| {
                        this.console
                            .convs
                            .as_ref()
                            .and_then(|c| c.list.first().map(|r| r.id))
                    });
                    if let Some(id) = id {
                        this.load_replay(id, cx);
                    }
                }
                cx.notify();
            }))
            .relative()
            .flex()
            .flex_col()
            .gap(t().d(4.))
            .mx(t().d(6.))
            .my(t().d(3.))
            .p(t().d(8.))
            .rounded(t().r(5.))
            .cursor_pointer()
            .border(t().d(1.))
            .border_color(rgb(if on { t().p.accent } else { t().p.line_soft }))
            .when(on, |el| el.bg(rgb(t().p.accent_soft)))
            .when(!on, |el| {
                el.bg(rgb(t().p.panel))
                    .hover(|el| el.border_color(rgb(t().p.line_strong)))
            });

        if size == FilmSize::Nano {
            // 只剩一列数字：label 缩成两三个字，值大、单位小。
            return card
                .items_center()
                .gap(t().d(1.))
                .px(t().d(2.))
                .child(
                    div()
                        .text_size(t().fs(8.5))
                        .text_color(rgb(t().p.muted))
                        .child(nano_label(r.label)),
                )
                .child(
                    div()
                        .text_size(t().fs(big))
                        .font_weight(FontWeight::MEDIUM)
                        .child(r.value.clone()),
                )
                .child(div().text_size(t().fs(8.)).text_color(rgb(t().p.faint)).child(r.unit))
                .into_any_element();
        }

        card = card
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(r.label),
                    )
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(t().d(2.))
                            .child(
                                div()
                                    .text_size(t().fs(big))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(r.value.clone()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(r.unit),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .child(r.note.clone()),
            );

        if let Some(dots) = r.dots {
            card = card.child(four_dots(dots));
        } else if size == FilmSize::Wide {
            card = card.child(mini_bar(r.frac));
        }

        card.into_any_element()
    }
}

/// 收起 / 极窄的时候，标签缩成两个字以内，不然一列 64px 全被文字吃了。
fn nano_label(label: &str) -> String {
    match label {
        "库文件" => "库".into(),
        "缓存命中" => "缓存".into(),
        "待答输入" => "待答".into(),
        "卡住的任务" => "卡住".into(),
        "文档 REVISION" => "文档".into(),
        other => other.chars().take(2).collect(),
    }
}

/// 胶片片基：一列小方孔。`on = false`（只剩数字那档）时收窄到 6px，不加孔。
fn perforation(on: bool) -> AnyElement {
    let w = if on { 12. } else { 6. };
    let holes: Vec<AnyElement> = if on {
        (0..70)
            .map(|_| {
                div()
                    .w(t().d(5.))
                    .h(t().d(6.))
                    .rounded(t().r(1.))
                    .bg(rgb(t().p.sidebar))
                    .flex_none()
                    .into_any_element()
            })
            .collect()
    } else {
        Vec::new()
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(t().d(9.))
        .w(t().d(w))
        .flex_none()
        .pt(t().d(6.))
        .overflow_hidden()
        .bg(rgb(t().p.panel_alt))
        .children(holes)
        .into_any_element()
}

/// 迷你柱：`frac` 铺满 = 100%。不做动画 —— 这一列每秒都会被重画。
fn mini_bar(frac: f32) -> AnyElement {
    let f = frac.clamp(0., 1.);
    div()
        .h(t().d(3.))
        .w_full()
        .rounded(t().r(2.))
        .bg(rgb(t().p.line_soft))
        .child(
            div()
                .h_full()
                .w(t().d(200.) * f)
                .rounded(t().r(2.))
                .bg(rgb(t().p.accent)),
        )
        .into_any_element()
}

/// 归档那格的四态点：原件在 / 已销毁 / 已捞回 / 无墓碑。
fn four_dots(counts: [usize; 4]) -> AnyElement {
    let colors = [t().p.accent, t().p.danger, t().p.warn, t().p.faint];
    let mut row = div().flex().items_center().gap(t().d(3.));
    for (i, n) in counts.iter().enumerate() {
        let shown = (*n).min(8);
        for _ in 0..shown {
            row = row.child(
                div()
                    .w(t().d(4.))
                    .h(t().d(4.))
                    .rounded(t().r(2.))
                    .bg(rgb(colors[i])),
            );
        }
    }
    if counts.iter().all(|n| *n == 0) {
        row = row.child(div().text_size(t().fs(8.5)).text_color(rgb(t().p.faint)).child("—"));
    }
    row.into_any_element()
}

// ── 渲染：中列（六屏）────────────────────────────────────────────────

impl Desktop {
    fn screen_column(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(t().d(0.))
            .min_h(t().d(0.))
            .bg(rgb(t().p.panel))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(self.screen_tabs(cx))
            .child(self.screen_body(cx))
            .into_any_element()
    }

    /// 六个 tab + 当前屏的一句说明。说明写在右边，省一行高度。
    fn screen_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let cur = self.console.screen;
        div()
            .flex()
            .items_center()
            .gap(t().d(8.))
            .h(t().d(30.))
            .flex_none()
            .px(t().d(12.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(
                div()
                    .text_size(t().fs(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(cur.label()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(t().d(0.))
                    .overflow_hidden()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .child(format!("· {}", cur.hint())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(1.))
                    .children(Screen::ALL.iter().enumerate().map(|(i, s)| {
                        let on = *s == cur;
                        let pick = *s;
                        div()
                            .id(ElementId::Name(format!("dc-tab-{i}").into()))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.console.screen = pick;
                                if pick == Screen::Replay {
                                    let id = this.console.replay_id.or_else(|| {
                                        this.console
                                            .convs
                                            .as_ref()
                                            .and_then(|c| c.list_by_tokens().first().map(|r| r.id))
                                    });
                                    if let Some(id) = id {
                                        this.load_replay(id, cx);
                                    }
                                }
                                cx.notify();
                            }))
                            .px(t().d(7.))
                            .py(t().d(2.))
                            .rounded(t().r(3.))
                            .cursor_pointer()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if on { t().p.accent_fg } else { t().p.muted }))
                            .when(on, |el| el.bg(rgb(t().p.accent_soft)))
                            .when(!on, |el| el.hover(|el| el.bg(rgb(t().p.hover))))
                            .child(s.label())
                    })),
            )
            .into_any_element()
    }

    fn screen_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let screen = self.console.screen;
        let body: AnyElement = match screen {
            Screen::Overview => self.screen_overview(),
            Screen::Convs => self.screen_convs(cx),
            Screen::Replay => self.screen_replay(cx),
            Screen::Cost => self.screen_cost(),
            Screen::Health => self.screen_health(),
            Screen::Archive => self.screen_archive(cx),
        };

        let err = self.console.error.clone();
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .min_w(t().d(0.))
            .id("dc-screen-scroll")
            .overflow_y_scroll()
            .track_scroll(&self.console.screen_scroll)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.bar_hover(Bar::DataScreen, *hovered, cx)
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(12.))
                    .p(t().d(14.))
                    .children(err.map(|e| error_box(&e)))
                    .child(body),
            )
            .child(crate::app::scrollbar::overlay(
                "dc-screen-bar",
                &self.console.screen_scroll,
                self.bar(Bar::DataScreen).0,
                self.bar(Bar::DataScreen).1,
            ))
            .into_any_element()
    }
}

// ── 屏 1：解剖 ───────────────────────────────────────────────────────

impl Desktop {
    fn screen_overview(&self) -> AnyElement {
        let Some(o) = self.console.overview.as_ref() else {
            return empty_note("还没拿到 overview", "点右上「刷新」再试；连不上就看底栏那句原话。");
        };

        // 六格摘要：库 / WAL / 空闲 / 表·行 / entries / schema。
        let cells: Vec<AnyElement> = vec![
            stat_cell("库文件", fmt_bytes_short(o.file.db), format!("{} 页", fmt_num(o.pragmas.page_count))),
            stat_cell("WAL", fmt_bytes_short(o.file.wal), if o.file.wal > 0 { "未 checkpoint".into() } else { "干净".into() }),
            stat_cell("空闲页", fmt_bytes_short(o.bytes.free), format!("{} 页", fmt_num(o.pragmas.freelist))),
            stat_cell("表 · 行", format!("{}", o.tables.len()), format!("{} 行", fmt_num(o.tables.iter().map(|t| t.rows).sum()))),
            stat_cell("ENTRIES", fmt_num(o.counts.get("entries")), format!("{} 个 kind", o.entries_by_kind.len())),
            stat_cell("SCHEMA", format!("v{}", o.pragmas.schema_version), o.pragmas.journal_mode.clone()),
        ];

        let tables: Vec<AnyElement> = o
            .tables
            .iter()
            .map(|tb| {
                let frac = if o.file.db > 0 {
                    tb.bytes as f32 / o.file.db as f32
                } else {
                    0.
                };
                table_row(&tb.name, tb.rows, tb.bytes, tb.pages, frac)
            })
            .collect();

        let kinds: Vec<AnyElement> = o
            .entries_by_kind
            .iter()
            .map(|k| {
                let frac = if o.counts.get("entries") > 0 {
                    k.count as f32 / o.counts.get("entries") as f32
                } else {
                    0.
                };
                table_row(&k.kind, k.count, k.bytes, 0, frac)
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(t().d(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(t().d(8.))
                    .children(cells),
            )
            .child(integrity_line(o))
            .child(path_line(&o.target.path))
            .child(section("表 · 按页", tables, true))
            .child(section("ENTRIES.KIND", kinds, true))
            .into_any_element()
    }
}

// ── 屏 2：会话树 ─────────────────────────────────────────────────────

impl Desktop {
    fn screen_convs(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(c) = self.console.convs.as_ref() else {
            return empty_note("还没拿到会话清单", "这份挂了不影响其它五屏。");
        };
        let picking = self.console.replay_id;
        let rows: Vec<AnyElement> = c
            .list_by_tokens()
            .iter()
            .map(|r| self.conv_row(r, picking == Some(r.id), cx))
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(t().d(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(10.))
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .child(format!("{} 条会话", fmt_num(c.count)))
                    .child(format!("· {} entries", fmt_num(c.totals.entries)))
                    .child(format!("· {} tokens", fmt_num(c.totals.tokens)))
                    .child(div().flex_1())
                    .child("按 token 从多到少"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(t().r(5.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .children(rows),
            )
            .into_any_element()
    }

    fn conv_row(&self, row: &ConvRow, on: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = row.id;
        div()
            .id(ElementId::Name(format!("dc-conv-{id}").into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.console.screen = Screen::Replay;
                this.load_replay(id, cx);
                cx.notify();
            }))
            .flex()
            .items_center()
            .gap(t().d(10.))
            .px(t().d(10.))
            .py(t().d(6.))
            .cursor_pointer()
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .when(on, |el| el.bg(rgb(t().p.accent_soft)))
            .when(!on, |el| el.hover(|el| el.bg(rgb(t().p.hover))))
            .child(
                div()
                    .w(t().d(46.))
                    .flex_none()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.muted))
                    .child(format!("#{id}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(t().d(0.))
                    .overflow_hidden()
                    .text_size(t().fs(11.))
                    .child(if row.title.is_empty() {
                        "(空会话)".to_string()
                    } else {
                        one_line(&row.title, 80)
                    }),
            )
            .child(num_cell(fmt_num(row.entries), "entries"))
            .child(num_cell(fmt_num(row.tokens), "tokens"))
            .child(num_cell(fmt_num(row.tool_calls), "tools"))
            .child(
                div()
                    .w(t().d(92.))
                    .flex_none()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .child(fmt_clock(row.last_at)),
            )
            .into_any_element()
    }
}

// ── 屏 3：回放 ───────────────────────────────────────────────────────

impl Desktop {
    fn screen_replay(&self, _cx: &mut Context<Self>) -> AnyElement {
        if let Some(e) = self.console.replay_error.as_deref() {
            return error_box(e);
        }
        let Some(r) = self.console.replay.as_ref() else {
            return empty_note(
                "还没选会话",
                "去「会话树」点一条，或者在左列点「会话」读数。回放是按需拉的。",
            );
        };
        let entries: Vec<AnyElement> = r
            .entries
            .iter()
            .map(|e| {
                let kind_color = if e.kind.contains("user") {
                    t().p.muted
                } else if e.is_error {
                    t().p.danger
                } else if e.tool.is_some() {
                    t().p.warn
                } else {
                    t().p.accent
                };
                div()
                    .flex()
                    .items_start()
                    .gap(t().d(8.))
                    .py(t().d(4.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .w(t().d(34.))
                            .flex_none()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.faint))
                            .child(format!("#{}", e.seq)),
                    )
                    .child(
                        div()
                            .w(t().d(3.))
                            .h(t().d(14.))
                            .flex_none()
                            .rounded(t().r(2.))
                            .bg(rgb(kind_color)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t().d(2.))
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(t().d(6.))
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.muted))
                                            .child(e.kind.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.faint))
                                            .child(if e.tokens > 0 {
                                                format!("{} tok", fmt_num(e.tokens))
                                            } else {
                                                format!("{} B", fmt_num(e.bytes))
                                            }),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_size(t().fs(9.))
                                            .text_color(rgb(t().p.faint))
                                            .child(fmt_clock(e.at)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(one_line(&e.text, 320)),
                            ),
                    )
                    .into_any_element()
            })
            .collect();

        let slowest_tasks: Vec<AnyElement> = r.tasks.iter().take(6).map(|task| -> AnyElement {
            div().flex().items_center().gap(t().d(8.))
                .child(div().w(t().d(40.)).flex_none().text_size(t().fs(9.)).text_color(rgb(t().p.muted)).child(format!("#{}", task.id)))
                .child(div().flex_1().text_size(t().fs(10.)).child(task.kind.clone()))
                .child(div().text_size(t().fs(9.5)).text_color(rgb(t().p.faint)).child(task.status.clone()))
                .child(div().w(t().d(54.)).flex_none().text_size(t().fs(9.5)).text_color(rgb(t().p.muted)).child(fmt_ms(task.ms.unwrap_or(0))))
                .into_any_element()
        }).collect();

        div()
            .flex()
            .flex_col()
            .gap(t().d(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(10.))
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("会话 #{}", r.id)),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(10.))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{} entries", fmt_num(r.entries.len() as i64)))
                            .child(format!("· {} tasks", r.tasks.len()))
                            .child(format!("· {} docs", r.documents.len()))
                            .child(format!("· {} prompt 条", r.prompt.messages.len())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(entries),
            )
            .child(section("这条会话的任务", slowest_tasks, false))
            .into_any_element()
    }
}

// ── 屏 4：账单 ───────────────────────────────────────────────────────

impl Desktop {
    fn screen_cost(&self) -> AnyElement {
        let Some(c) = self.console.cost.as_ref() else {
            return empty_note("还没拿到账本", "这份挂了不影响其它五屏。");
        };
        let totals = &c.totals;
        let hit = totals.cache_hit();

        let mut bar = div().flex().h(t().d(8.)).w_full().rounded(t().r(3.)).overflow_hidden();
        for (v, color) in [
            (totals.cache_read, t().p.accent),
            (totals.input, t().p.link),
            (totals.output, t().p.warn),
            (totals.reasoning, t().p.faint),
        ] {
            if totals.tokens > 0 && v > 0 {
                bar = bar.child(
                    div()
                        .h_full()
                        .w(t().d(600.) * (v as f32 / totals.tokens as f32))
                        .bg(rgb(color)),
                );
            }
        }

        let days: Vec<AnyElement> = c
            .by_day
            .iter()
            .map(|d| bucket_row(d, c.by_day.iter().map(|x| x.tokens).max().unwrap_or(1)))
            .collect();
        let models: Vec<AnyElement> = c
            .by_model
            .iter()
            .map(|m| bucket_row(m, c.by_model.iter().map(|x| x.tokens).max().unwrap_or(1)))
            .collect();
        let convs: Vec<AnyElement> = c
            .by_conv
            .iter()
            .take(12)
            .map(|m| bucket_row(m, c.by_conv.iter().map(|x| x.tokens).max().unwrap_or(1)))
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(t().d(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(t().d(8.))
                    .child(stat_cell("TOKENS", fmt_num(totals.tokens), format!("{} 次生成", fmt_num(totals.entries))))
                    .child(stat_cell("输入", fmt_num(totals.input), "provider 报的".into()))
                    .child(stat_cell("缓存读", fmt_num(totals.cache_read), format!("命中 {:.0}%", hit * 100.)))
                    .child(stat_cell("输出", fmt_num(totals.output), format!("推理 {}", fmt_num(totals.reasoning))))
                    .child(stat_cell("缓存写", fmt_num(totals.cache_write), "写进去的".into()))
                    .child(stat_cell("费用", format!("{:.2}", totals.cost), "看下面那句".into())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(6.))
                    .child(bar)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(12.))
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.faint))
                            .child(legend("缓存读", t().p.accent))
                            .child(legend("输入", t().p.link))
                            .child(legend("输出", t().p.warn))
                            .child(legend("推理", t().p.faint)),
                    ),
            )
            .child(section("按天", days, false))
            .child(section("按模型", models, false))
            .child(section("按会话（前 12）", convs, false))
            .when(!c.note.is_empty(), |el| {
                el.child(
                    div()
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.muted))
                        .child(c.note.clone()),
                )
            })
            .into_any_element()
    }
}

// ── 屏 5：维修 ───────────────────────────────────────────────────────

impl Desktop {
    fn screen_health(&self) -> AnyElement {
        let Some(h) = self.console.health.as_ref() else {
            return empty_note("还没拿到体检报告", "这份挂了不影响其它五屏。");
        };
        let stuck = h.tasks.stuck.len();
        let unanswered = h.submissions.unanswered.len();

        let statuses: Vec<AnyElement> = h
            .tasks
            .by_status
            .iter()
            .map(|s| {
                let frac = if h.tasks.total > 0 {
                    s.count as f32 / h.tasks.total as f32
                } else {
                    0.
                };
                table_row(&s.status, s.count, 0, 0, frac)
            })
            .collect();

        let slowest: Vec<AnyElement> = h
            .tasks
            .slowest
            .iter()
            .map(|task| {
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .py(t().d(3.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .w(t().d(38.))
                            .flex_none()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("#{}", task.id)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .text_size(t().fs(10.5))
                            .child(task.kind.clone()),
                    )
                    .child(
                        div()
                            .w(t().d(60.))
                            .flex_none()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("conv #{}", task.conv)),
                    )
                    .child(
                        div()
                            .w(t().d(64.))
                            .flex_none()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.faint))
                            .child(task.status.clone()),
                    )
                    .child(
                        div()
                            .w(t().d(64.))
                            .flex_none()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if TaskMs::ms(task) > 30_000 {
                                t().p.warn
                            } else {
                                t().p.text_2
                            }))
                            .child(fmt_ms(TaskMs::ms(task))),
                    )
                    .into_any_element()
            })
            .collect();

        let waiting: Vec<AnyElement> = h
            .submissions
            .unanswered
            .iter()
            .take(12)
            .map(|s| {
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .py(t().d(3.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .w(t().d(38.))
                            .flex_none()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("#{}", s.id)),
                    )
                    .child(
                        div()
                            .w(t().d(60.))
                            .flex_none()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("conv #{}", s.conv)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.text_2))
                            .child(s.status.clone()),
                    )
                    .into_any_element()
            })
            .collect();

        let docs = h.documents.delta_ratio.clamp(0., 1.);
        let maint: Vec<AnyElement> = h
            .maintenance
            .iter()
            .map(|m| {
                div()
                    .flex()
                    .flex_col()
                    .gap(t().d(3.))
                    .p(t().d(8.))
                    .rounded(t().r(5.))
                    .border(t().d(1.))
                    .border_color(rgb(if m.danger { t().p.danger_soft } else { t().p.line_soft }))
                    .when(m.danger, |el| el.bg(rgb(t().p.danger_soft)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(6.))
                            .child(
                                div()
                                    .text_size(t().fs(10.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(if m.danger { t().p.danger } else { t().p.text }))
                                    .child(m.label.clone()),
                            )
                            .when(m.danger, |el| el.child(badge("危险", t().p.danger_soft, t().p.danger))),
                    )
                    .when(!m.why.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(t().fs(9.5))
                                .text_color(rgb(t().p.muted))
                                .child(m.why.clone()),
                        )
                    })
                    .when(!m.cmd.is_empty(), |el| {
                        el.child(
                            div()
                                .rounded(t().r(3.))
                                .bg(rgb(t().p.code_bg))
                                .px(t().d(6.))
                                .py(t().d(2.))
                                .text_size(t().fs(9.5))
                                .child(m.cmd.clone()),
                        )
                    })
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap(t().d(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(t().d(8.))
                    .child(stat_cell(
                        "卡住的任务",
                        fmt_num(stuck as i64),
                        if stuck == 0 { "没有活挂着的".into() } else { "去看一眼".into() },
                    ))
                    .child(stat_cell("没人接的提交", fmt_num(unanswered as i64), format!("{} 总共", fmt_num(h.submissions.total))))
                    .child(stat_cell("平均耗时", fmt_ms(h.tasks.avg_ms), format!("p95 {}", fmt_ms(h.tasks.p95))))
                    .child(stat_cell("WAL", fmt_bytes_short(h.wal_bytes), "越涨越该 checkpoint".into()))
                    .child(stat_cell("文档", fmt_num(h.documents.documents), format!("{} 版 revision", fmt_num(h.documents.revisions))))
                    .child(stat_cell("增量占比", format!("{:.0}%", docs * 100.), format!("{} delta / {} base", fmt_num(h.documents.delta), fmt_num(h.documents.base)))),
            )
            .child(section("任务状态", statuses, false))
            .child(section("最慢的几个", slowest, false))
            .child(section("没人接的提交", waiting, false))
            .child(section("维护建议", maint, false))
            .into_any_element()
    }
}

// ── 屏 6：归档 ───────────────────────────────────────────────────────

impl Desktop {
    fn screen_archive(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(gens) = self.console.archive.as_ref() else {
            return empty_note("还没拿到归档库", "这一份走的是引擎 `/v1/archive`，跟面板那四份不是一个来源。");
        };

        // 四态各几个。判据只有一份：`archive_page::item_status`。
        let mut counts = [0usize; 4];
        for g in &gens.gens {
            for item in &g.items {
                counts[status_idx(item_status(item))] += 1;
            }
        }
        let labels = ["原件在", "已销毁", "已捞回", "无墓碑"];
        let colors = [t().p.accent, t().p.danger, t().p.warn, t().p.faint];

        let mut groups: Vec<AnyElement> = Vec::new();
        for g in &gens.gens {
            if g.items.is_empty() {
                continue;
            }
            groups.push(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .pt(t().d(8.))
                    .pb(t().d(4.))
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(crate::archive_page::gen_group_title(&g.r#gen))
                    .child(format!("{} 条", g.items.len()))
                    .into_any_element(),
            );
            for item in &g.items {
                let status = item_status(item);
                let idx = status_idx(status);
                let in_replay = self.console.replay_id == Some(item.id);
                let id = item.id;
                groups.push(
                    div()
                        .id(ElementId::Name(format!("dc-arch-{}-{id}", g.r#gen).into()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.console.screen = Screen::Replay;
                            this.load_replay(id, cx);
                            cx.notify();
                        }))
                        .flex()
                        .items_center()
                        .gap(t().d(8.))
                        .px(t().d(8.))
                        .py(t().d(5.))
                        .rounded(t().r(4.))
                        .cursor_pointer()
                        .border_b(t().d(1.))
                        .border_color(rgb(t().p.line_soft))
                        .when(in_replay, |el| el.bg(rgb(t().p.accent_soft)))
                        .when(!in_replay, |el| el.hover(|el| el.bg(rgb(t().p.hover))))
                        .child(
                            div()
                                .w(t().d(44.))
                                .flex_none()
                                .text_size(t().fs(9.))
                                .text_color(rgb(t().p.faint))
                                .child(format!("#{id}")),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(t().d(0.))
                                .overflow_hidden()
                                .text_size(t().fs(10.5))
                                .child(if item.name.is_empty() {
                                    "(无名)".to_string()
                                } else {
                                    one_line(&item.name, 90)
                                }),
                        )
                        .child(
                            div()
                                .text_size(t().fs(9.5))
                                .text_color(rgb(t().p.faint))
                                .child(format!("{} entries", fmt_num(item.entries as i64))),
                        )
                        .child(
                            div()
                                .px(t().d(6.))
                                .py(t().d(1.))
                                .rounded(t().r(3.))
                                .bg(rgb(colors[idx]))
                                .text_size(t().fs(9.))
                                .text_color(rgb(t().p.on_accent))
                                .child(status.label()),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(t().d(10.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(t().d(8.))
                    .children(labels.iter().enumerate().map(|(i, label)| {
                        stat_cell(label, fmt_num(counts[i] as i64), "条".into())
                    })),
            )
            .child(
                div()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .child("四态只看这一条自己：purged → 已销毁；live 且有墓碑 → 原件在；live 没墓碑 → 已捞回；其余 → 无墓碑。"),
            )
            .child(div().flex().flex_col().children(groups))
            .into_any_element()
    }
}

// ── 渲染：右列（时间线）──────────────────────────────────────────────

impl Desktop {
    fn timeline_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let archive = self.console.archive.as_ref();
        let all = self.console.events(archive).len();
        let events = self.console.timeline(archive);
        let filter = self.console.tl_filter;
        let evs: Vec<AnyElement> = events
            .iter()
            .map(|e| {
                let kind = e.kind;
                let screen = e.screen;
                let dot = match kind {
                    Kind::Archive => t().p.accent,
                    Kind::Conv => t().p.link,
                    Kind::Task => t().p.warn,
                    Kind::Cost => t().p.faint,
                };
                div()
                    .id(ElementId::Name(format!("dc-ev-{}-{}", e.at, e.title).into()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.console.screen = screen;
                        if screen == Screen::Replay
                            && let Some(id) = this.console.replay_id
                        {
                            this.load_replay(id, cx);
                        }
                        cx.notify();
                    }))
                    .flex()
                    .flex_col()
                    .gap(t().d(3.))
                    .mx(t().d(8.))
                    .my(t().d(4.))
                    .p(t().d(8.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .bg(rgb(t().p.panel))
                    .hover(|el| el.border_color(rgb(t().p.line_strong)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(6.))
                            .child(
                                div()
                                    .px(t().d(4.))
                                    .rounded(t().r(2.))
                                    .bg(rgb(dot))
                                    .text_size(t().fs(8.5))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(kind.label()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.))
                                    .text_color(rgb(t().p.faint))
                                    .child(fmt_clock(e.at)),
                            )
                            .child(div().flex_1())
                            .when(!e.tail.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_size(t().fs(9.))
                                        .text_color(rgb(t().p.muted))
                                        .child(e.tail.clone()),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .child(one_line(&e.title, 120)),
                    )
                    .when(!e.detail.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(t().fs(9.5))
                                .text_color(rgb(t().p.muted))
                                .child(one_line(&e.detail, 160)),
                        )
                    })
                    .into_any_element()
            })
            .collect();

        div()
            .relative()
            .flex()
            .flex_col()
            .w(t().d(TL_W))
            .flex_none()
            .min_h(t().d(0.))
            .bg(rgb(t().p.panel_alt))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(30.))
                    .flex_none()
                    .px(t().d(10.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(4.))
                            .child("时间线")
                            .child(
                                div()
                                    .text_color(rgb(t().p.text_2))
                                    .child(fmt_num(events.len() as i64)),
                            )
                            .when(events.len() != all, |el| {
                                el.child(
                                    div()
                                        .text_size(t().fs(9.))
                                        .child(format!("/ {}", fmt_num(all as i64))),
                                )
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t().d(1.))
                            .children(TlFilter::ALL.iter().enumerate().map(|(i, f)| {
                                let on = *f == filter;
                                let pick = *f;
                                div()
                                    .id(ElementId::Name(format!("dc-tlf-{i}").into()))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.console.tl_filter = pick;
                                        cx.notify();
                                    }))
                                    .px(t().d(4.))
                                    .rounded(t().r(3.))
                                    .cursor_pointer()
                                    .text_size(t().fs(9.))
                                    .text_color(rgb(if on { t().p.accent_fg } else { t().p.muted }))
                                    .when(on, |el| el.bg(rgb(t().p.accent_soft)))
                                    .when(!on, |el| el.hover(|el| el.bg(rgb(t().p.hover))))
                                    .child(f.label())
                            })),
                    )
                    .child(
                        div()
                            .id("dc-tl-fold")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.console.tl_open = false;
                                cx.notify();
                            }))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(18.))
                            .rounded(t().r(4.))
                            .cursor_pointer()
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .hover(|el| el.bg(rgb(t().p.hover)))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.muted))
                                    .child("›"),
                            ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(t().d(0.))
                    .id("dc-tl-scroll")
                    .overflow_y_scroll()
                    .track_scroll(&self.console.tl_scroll)
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.bar_hover(Bar::DataTl, *hovered, cx)
                    }))
                    .children(if evs.is_empty() {
                        vec![div()
                            .p(t().d(12.))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.faint))
                            .child("这个过滤下一条都没有。")
                            .into_any_element()]
                    } else {
                        evs
                    })
                    .child(crate::app::scrollbar::overlay(
                        "dc-tl-bar",
                        &self.console.tl_scroll,
                        self.bar(Bar::DataTl).0,
                        self.bar(Bar::DataTl).1,
                    )),
            )
            .into_any_element()
    }

    /// 收起态：48px 竖条，竖排「时间线」+ 条数，点一下展开。
    fn timeline_rail(&self, cx: &mut Context<Self>) -> AnyElement {
        let archive = self.console.archive.as_ref();
        let n = self.console.timeline(archive).len();
        div()
            .id("dc-tl-rail")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.console.tl_open = true;
                cx.notify();
            }))
            .flex()
            .flex_col()
            .items_center()
            .gap(t().d(6.))
            .w(t().d(TL_W_FOLDED))
            .flex_none()
            .py(t().d(10.))
            .bg(rgb(t().p.panel_alt))
            .border_l(t().d(1.))
            .border_color(rgb(t().p.line))
            .cursor_pointer()
            .hover(|el| el.bg(rgb(t().p.hover)))
            .child(
                div()
                    .text_size(t().fs(12.))
                    .text_color(rgb(t().p.muted))
                    .child("‹"),
            )
            .child(
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child("时间线"),
            )
            .child(
                div()
                    .text_size(t().fs(9.))
                    .text_color(rgb(t().p.faint))
                    .child(format!("{n}")),
            )
            .into_any_element()
    }
}

// ── 共用小件 ─────────────────────────────────────────────────────────

/// 会话清单按 token 从多到少 —— 「谁最贵」比「谁最新」更值得先看。
pub(crate) trait ConvSort {
    fn list_by_tokens(&self) -> Vec<&ConvRow>;
}

impl ConvSort for Conversations {
    fn list_by_tokens(&self) -> Vec<&ConvRow> {
        let mut rows: Vec<&ConvRow> = self.list.iter().collect();
        rows.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.id.cmp(&b.id)));
        rows
    }
}

fn badge(text: &str, bg: u32, fg: u32) -> AnyElement {
    div()
        .px(t().d(6.))
        .py(t().d(1.))
        .rounded(t().r(3.))
        .bg(rgb(bg))
        .text_size(t().fs(9.))
        .text_color(rgb(fg))
        .child(text.to_string())
        .into_any_element()
}

fn link_button(
    id: &'static str,
    label: &str,
    cx: &mut Context<Desktop>,
    on: impl Fn(&mut Desktop, &mut Context<Desktop>) + 'static,
) -> AnyElement {
    let on = Rc::new(on);
    div()
        .id(ElementId::Name(id.into()))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| on(this, cx)))
        .px(t().d(8.))
        .py(t().d(2.))
        .rounded(t().r(4.))
        .cursor_pointer()
        .border(t().d(1.))
        .border_color(rgb(t().p.line))
        .text_size(t().fs(10.5))
        .text_color(rgb(t().p.text_2))
        .hover(|el| el.bg(rgb(t().p.hover)))
        .child(label.to_string())
        .into_any_element()
}

/// 一格摘要：小标题 + 大数字 + 一行说明。
fn stat_cell(label: &str, value: String, note: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(2.))
        .min_w(t().d(118.))
        .p(t().d(8.))
        .rounded(t().r(5.))
        .border(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .text_size(t().fs(9.))
                .text_color(rgb(t().p.muted))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(t().fs(17.))
                .font_weight(FontWeight::MEDIUM)
                .child(value),
        )
        .child(
            div()
                .text_size(t().fs(9.))
                .text_color(rgb(t().p.faint))
                .child(note),
        )
        .into_any_element()
}

/// 一张表里的一行：名字 + 数字 ×3 + 占比条。
fn table_row(name: &str, rows: i64, bytes: i64, pages: i64, frac: f32) -> AnyElement {
    div()
        .relative()
        .flex()
        .items_center()
        .gap(t().d(8.))
        .px(t().d(8.))
        .py(t().d(4.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            // 占比画在**底下一条 2px**，不铺整行 —— 铺整行会把字盖住
            // （单色皮 / 琥珀皮的 accent_tint 是不透明的亮色）。
            div()
                .absolute()
                .bottom(t().d(0.))
                .left(t().d(0.))
                .h(t().d(2.))
                .w(t().d(400.) * frac.clamp(0., 1.))
                .bg(rgb(t().p.accent)),
        )
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .overflow_hidden()
                .text_size(t().fs(10.5))
                .child(name.to_string()),
        )
        .child(
            div()
                .w(t().d(64.))
                .flex_none()
                .text_size(t().fs(9.5))
                .text_color(rgb(t().p.text_2))
                .child(fmt_num(rows)),
        )
        .when(bytes > 0, |el| {
            el.child(
                div()
                    .w(t().d(64.))
                    .flex_none()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.muted))
                    .child(fmt_bytes_short(bytes)),
            )
        })
        .when(pages > 0, |el| {
            el.child(
                div()
                    .w(t().d(48.))
                    .flex_none()
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.faint))
                    .child(format!("{pages}p")),
            )
        })
        .into_any_element()
}

/// 账本里的一个桶：label + 条 + 数字。
fn bucket_row(b: &CostBucket, max: i64) -> AnyElement {
    let frac = if max > 0 {
        b.tokens as f32 / max as f32
    } else {
        0.
    };
    div()
        .relative()
        .flex()
        .items_center()
        .gap(t().d(8.))
        .px(t().d(8.))
        .py(t().d(4.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .absolute()
                .bottom(t().d(0.))
                .left(t().d(0.))
                .h(t().d(2.))
                .w(t().d(420.) * frac.clamp(0., 1.))
                .bg(rgb(t().p.accent)),
        )
        .child(
            div()
                .w(t().d(96.))
                .flex_none()
                .overflow_hidden()
                .text_size(t().fs(9.5))
                .text_color(rgb(t().p.text_2))
                .child(b.label.clone()),
        )
        .child(
            div()
                .w(t().d(72.))
                .flex_none()
                .text_size(t().fs(9.5))
                .child(fmt_num(b.tokens)),
        )
        .child(
            div()
                .w(t().d(48.))
                .flex_none()
                .text_size(t().fs(9.))
                .text_color(rgb(t().p.faint))
                .child(format!("{}×", b.calls)),
        )
        .child(
            div()
                .flex_1()
                .text_size(t().fs(9.))
                .text_color(rgb(t().p.faint))
                .child(format!("入 {} · 出 {} · 缓存 {}", fmt_num(b.input), fmt_num(b.output), fmt_num(b.cache_read))),
        )
        .into_any_element()
}

fn legend(label: &str, color: u32) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t().d(3.))
        .child(div().w(t().d(6.)).h(t().d(6.)).rounded(t().r(2.)).bg(rgb(color)))
        .child(label.to_string())
        .into_any_element()
}

fn num_cell(value: String, label: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_end()
        .w(t().d(64.))
        .flex_none()
        .child(div().text_size(t().fs(10.)).child(value))
        .child(
            div()
                .text_size(t().fs(8.5))
                .text_color(rgb(t().p.faint))
                .child(label),
        )
        .into_any_element()
}

/// 一个小标题 + 若干行。`head` 为真时上面加一行列表头。
fn section(title: &str, rows: Vec<AnyElement>, head: bool) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(4.))
        .child(
            div()
                .text_size(t().fs(10.))
                .text_color(rgb(t().p.muted))
                .child(title.to_string()),
        )
        .when(head, |el| {
            el.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .px(t().d(8.))
                    .text_size(t().fs(8.5))
                    .text_color(rgb(t().p.faint))
                    .child("名称")
                    .child(div().flex_1())
                    .child("行")
                    .child("占用")
                    .child("页"),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .rounded(t().r(4.))
                .border(t().d(1.))
                .border_color(rgb(t().p.line_soft))
                .children(rows),
        )
        .into_any_element()
}

fn empty_note(title: &str, hint: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(4.))
        .p(t().d(16.))
        .child(div().text_size(t().fs(11.5)).child(title.to_string()))
        .child(
            div()
                .text_size(t().fs(9.5))
                .text_color(rgb(t().p.muted))
                .child(hint.to_string()),
        )
        .into_any_element()
}

fn error_box(text: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t().d(2.))
        .p(t().d(8.))
        .rounded(t().r(5.))
        .bg(rgb(t().p.danger_soft))
        .text_size(t().fs(9.5))
        .text_color(rgb(t().p.danger))
        .child(text.to_string())
        .into_any_element()
}

fn integrity_line(o: &Overview) -> AnyElement {
    let (text, color) = if !o.integrity.checked {
        ("integrity_check 还没跑".to_string(), t().p.muted)
    } else if o.integrity.ok {
        ("integrity_check 通过".to_string(), t().p.ok_fg)
    } else {
        (format!("integrity_check 有问题：{}", o.integrity.messages.join("；")), t().p.danger)
    };
    div()
        .flex()
        .items_center()
        .gap(t().d(6.))
        .text_size(t().fs(9.5))
        .text_color(rgb(color))
        .child(if o.integrity.ok && o.integrity.checked { "✓" } else { "!" })
        .child(text)
        .into_any_element()
}

fn path_line(path: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t().d(6.))
        .text_size(t().fs(9.))
        .text_color(rgb(t().p.faint))
        .child("库")
        .child(div().child(path.to_string()))
        .into_any_element()
}

/// `SlowTask` 没带耗时，只有起止时刻 —— 借一下面板的算法（结束减开始）。
pub(crate) trait TaskMs {
    fn ms(&self) -> i64;
}

impl TaskMs for nous_engine::console::SlowTask {
    fn ms(&self) -> i64 {
        match self.ended_at {
            Some(end) if end >= self.started_at => end - self.started_at,
            _ => 0,
        }
    }
}
