//! 数据监控室的**展示类型**与 `StatsClient`（`/v1/stats/*` 哑客户端）。
//!
//! 六屏的数据只从 `/v1/*` 来：device / capabilities / sessions / entries 由
//! `EngineClient` 转发，三条 stats 由 [`StatsClient`] 转发。这里不造业务概念
//! （没有「健康」探针、不在客户端算成本），也不重新排版：屏幕仍消费原有形状。
//!
//! 未知字段一律 `Value` / `BTreeMap` 收着透传；不在这里判能力（那在 capability.rs）。

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::EngineError;

// ── 通用小件 ─────────────────────────────────────────────────────────

/// 一堆「label → 数字」的桶（byDay / byModel / byConv 共用）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CostBucket {
    pub label: String,
    #[serde(default)]
    pub tokens: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub calls: i64,
    #[serde(default)]
    pub input: i64,
    #[serde(default)]
    pub output: i64,
    #[serde(default, rename = "cacheRead")]
    pub cache_read: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ── stats 端点回包（原样收着，Desk 只画） ─────────────────────────────

/// `GET /v1/stats/usage?group_by=day|model` 的回包。端点算好 totals/rows，Desk 不重算。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsUsage {
    #[serde(default, rename = "groupBy")]
    pub group_by: String,
    #[serde(default)]
    pub totals: StatsUsageTotals,
    #[serde(default)]
    pub rows: Vec<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsUsageTotals {
    #[serde(default, rename = "totalTokens")]
    pub total_tokens: i64,
    #[serde(default, rename = "costTotal")]
    pub cost_total: f64,
    #[serde(default)]
    pub requests: i64,
    #[serde(default)]
    pub errors: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `GET /v1/stats/tasks`：`byState` 是状态 → 条数，不是客户端算出来的「健康度」。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsTasks {
    #[serde(default, rename = "byState")]
    pub by_state: BTreeMap<String, i64>,
    #[serde(default)]
    pub active: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `GET /v1/stats/db`：两个库的字节与 quickCheck。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsDb {
    #[serde(default)]
    pub durable: DbReport,
    #[serde(default)]
    pub plugins: DbReport,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DbReport {
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub rows: BTreeMap<String, i64>,
    #[serde(default, rename = "quickCheck")]
    pub quick_check: String,
    #[serde(default)]
    pub schema: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ── StatsClient ───────────────────────────────────────────────────────

/// `/v1/stats/*` 的哑客户端。**必须 `with_token`**：自建 client 一律走 `crate::auth`
/// 注入 Bearer（`坑.md` 100），否则 stats 屏全 401。
#[derive(Debug, Clone)]
pub struct StatsClient {
    http: reqwest::Client,
    base: Url,
}

impl StatsClient {
    pub fn with_token(
        base: &str,
        timeout: Duration,
        token: Option<&str>,
    ) -> Result<Self, EngineError> {
        let mut url = Url::parse(base)?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        let headers: HeaderMap = crate::auth::headers(token).map_err(|message| EngineError::Engine {
            status: 400,
            message,
            body: None,
        })?;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .default_headers(headers)
            .build()?;
        Ok(Self { http, base: url })
    }

    fn url(&self, path: &str) -> Result<Url, EngineError> {
        Ok(self.base.join(path.trim_start_matches('/'))?)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, EngineError> {
        let resp = self.http.get(self.url(path)?).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .unwrap_or_else(|| text.chars().take(200).collect());
            return Err(EngineError::Engine {
                status: status.as_u16(),
                message,
                body: Some(text),
            });
        }
        serde_json::from_str(&text).map_err(|e| EngineError::Engine {
            status: status.as_u16(),
            message: format!("JSON 不像话：{e}"),
            body: Some(text),
        })
    }

    /// `GET /v1/stats/usage?group_by=…`。`group_by` 是闭集合，别把用户输入直接拼进去。
    pub async fn usage(&self, group_by: &str) -> Result<StatsUsage, EngineError> {
        if !matches!(group_by, "day" | "model" | "conversation") {
            return Err(EngineError::Engine {
                status: 400,
                message: format!("group_by 只能是 day|model|conversation，收到 {group_by:?}"),
                body: None,
            });
        }
        self.get_json(&format!("v1/stats/usage?group_by={group_by}"))
            .await
    }

    /// `GET /v1/stats/tasks`。
    pub async fn tasks(&self) -> Result<StatsTasks, EngineError> {
        self.get_json("v1/stats/tasks").await
    }

    /// `GET /v1/stats/db`。
    pub async fn db(&self) -> Result<StatsDb, EngineError> {
        self.get_json("v1/stats/db").await
    }
}

/// 三条 stats 路径的模板（给能力表/白名单自检用；只维护这一处）。
pub const STATS_PATHS: &[(&str, &str)] = &[
    ("GET", "v1/stats/usage"),
    ("GET", "v1/stats/tasks"),
    ("GET", "v1/stats/db"),
];

// ── 六屏展示形状（/v1 数据装配后的只读画材） ───────────────────────────
//
// 名字带 `Stats` 前缀：这些不再是旧面板 REST 的形状，而是从 `/v1/*` 装配出来的。
// 字段保住是为了六屏版式一字不动（Desk#12 非目标）。

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsOverview {
    pub target: StatsTarget,
    pub file: StatsDbFiles,
    pub pragmas: StatsPragmas,
    #[serde(default)]
    pub bytes: StatsBytes,
    pub counts: StatsCounts,
    #[serde(default)]
    pub tables: Vec<StatsTable>,
    #[serde(default)]
    pub indexes: Vec<StatsIndex>,
    #[serde(default, rename = "entriesByKind")]
    pub entries_by_kind: Vec<StatsKind>,
    #[serde(default)]
    pub integrity: StatsIntegrity,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 表名 → 行数。写死字段会漏新表，所以用 map + `get` 保住渲染层的调用形状。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsCounts {
    #[serde(flatten)]
    pub by_table: BTreeMap<String, i64>,
}

impl StatsCounts {
    pub fn get(&self, table: &str) -> i64 {
        self.by_table.get(table).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsTarget {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub path: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsDbFiles {
    #[serde(default)] pub db: i64,
    #[serde(default)] pub wal: i64,
    #[serde(default)] pub shm: i64,
    #[serde(default)] pub mtime: f64,
    #[serde(default, rename = "walMtime")] pub wal_mtime: f64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsPragmas {
    #[serde(default, rename = "pageSize")] pub page_size: i64,
    #[serde(default, rename = "pageCount")] pub page_count: i64,
    #[serde(default)] pub freelist: i64,
    #[serde(default, rename = "journalMode")] pub journal_mode: String,
    #[serde(default, rename = "schemaVersion")] pub schema_version: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsBytes {
    #[serde(default)] pub free: i64,
    #[serde(default)] pub used: i64,
    #[serde(default, rename = "onDisk")] pub on_disk: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsTable {
    pub name: String,
    #[serde(default)] pub bytes: i64,
    #[serde(default)] pub pages: i64,
    #[serde(default)] pub rows: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsIndex {
    pub name: String,
    #[serde(default)] pub bytes: i64,
    #[serde(default)] pub pages: i64,
    #[serde(default)] pub table: String,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsKind {
    pub kind: String,
    #[serde(default)] pub count: i64,
    #[serde(default)] pub bytes: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsIntegrity {
    #[serde(default)] pub checked: bool,
    #[serde(default)] pub ok: bool,
    #[serde(default)] pub messages: Vec<String>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsConvs {
    #[serde(default)] pub count: i64,
    #[serde(default)] pub list: Vec<StatsConvRow>,
    #[serde(default)] pub totals: StatsConvTotals,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsConvRow {
    pub id: i64,
    #[serde(default)] pub title: String,
    #[serde(default)] pub entries: i64,
    #[serde(default)] pub tokens: i64,
    #[serde(default, rename = "toolCalls")] pub tool_calls: i64,
    #[serde(default)] pub models: Vec<String>,
    #[serde(default, rename = "firstAt")] pub first_at: i64,
    #[serde(default, rename = "lastAt")] pub last_at: i64,
    #[serde(default)] pub pinned: bool,
    #[serde(default)] pub archived: bool,
    #[serde(default)] pub read_only: bool,
    #[serde(default)] pub busy: Value,
    #[serde(default)] pub template: Option<String>,
    #[serde(default)] pub skills: Vec<String>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsConvTotals {
    #[serde(default)] pub entries: i64,
    #[serde(default)] pub tokens: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsReplay {
    pub id: i64,
    #[serde(default)] pub entries: Vec<StatsConvEntry>,
    #[serde(default)] pub tasks: Vec<StatsTask>,
    #[serde(default)] pub documents: Vec<Value>,
    #[serde(default)] pub prompt: StatsPrompt,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsConvEntry {
    pub seq: i64,
    #[serde(default)] pub kind: String,
    #[serde(default)] pub bytes: i64,
    #[serde(default)] pub at: i64,
    #[serde(default)] pub tokens: i64,
    #[serde(default)] pub text: String,
    #[serde(default)] pub is_error: bool,
    #[serde(default)] pub tool: Option<String>,
    #[serde(default, rename = "stopReason")] pub stop_reason: Option<String>,
    #[serde(default, rename = "errorMessage")] pub error_message: Option<String>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsTask {
    pub id: i64,
    #[serde(default)] pub kind: String,
    #[serde(default)] pub status: String,
    #[serde(default, rename = "startedAt")] pub started_at: i64,
    #[serde(default, rename = "endedAt")] pub ended_at: Option<i64>,
    #[serde(default)] pub conv: i64,
    #[serde(default)] pub outcome: Option<String>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsPromptMessage {
    #[serde(default)] pub id: i64,
    #[serde(default)] pub label: String,
    #[serde(default)] pub kind: String,
    #[serde(default)] pub bytes: i64,
    #[serde(default)] pub chars: i64,
    #[serde(default)] pub tokens: i64,
    #[serde(default)] pub cost: f64,
    #[serde(default)] pub head: bool,
    #[serde(default)] pub text: String,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsPrompt {
    #[serde(default)] pub messages: Vec<StatsPromptMessage>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsCost {
    #[serde(default)] pub totals: StatsCostTotals,
    #[serde(default, rename = "byDay")] pub by_day: Vec<CostBucket>,
    #[serde(default, rename = "byHour")] pub by_hour: Vec<CostBucket>,
    #[serde(default, rename = "byModel")] pub by_model: Vec<CostBucket>,
    #[serde(default, rename = "byConv")] pub by_conv: Vec<CostBucket>,
    #[serde(default)] pub note: String,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsCostTotals {
    #[serde(default)] pub tokens: i64,
    #[serde(default)] pub cost: f64,
    #[serde(default)] pub input: i64,
    #[serde(default)] pub output: i64,
    #[serde(default, rename = "cacheRead")] pub cache_read: i64,
    #[serde(default, rename = "cacheWrite")] pub cache_write: i64,
    #[serde(default)] pub reasoning: i64,
    #[serde(default)] pub entries: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

impl StatsTask {
    pub fn ms(&self) -> i64 {
        self.extra
            .get("durationMs")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                self.ended_at
                    .map(|e| e.saturating_sub(self.started_at))
            })
            .unwrap_or(0)
    }
}

impl StatsCostTotals {
    /// 输入 + 缓存读为分母，零时返回零（不是 NaN）。
    pub fn cache_hit(&self) -> f32 {
        let billed = self.input + self.cache_read;
        if billed <= 0 { 0. } else { self.cache_read as f32 / billed as f32 }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsHealth {
    #[serde(default)] pub tasks: StatsHealthTasks,
    #[serde(default)] pub submissions: StatsHealthSubs,
    #[serde(default)] pub documents: StatsHealthDocs,
    #[serde(default, rename = "walBytes")] pub wal_bytes: i64,
    #[serde(default)] pub freelist: StatsFreeList,
    #[serde(default)] pub maintenance: Vec<StatsMaintenance>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsHealthTasks {
    #[serde(default)] pub total: i64,
    #[serde(default, rename = "byStatus")] pub by_status: Vec<StatsStatusBucket>,
    #[serde(default)] pub stuck: Vec<Value>,
    #[serde(default, rename = "avgMs")] pub avg_ms: i64,
    #[serde(default)] pub p95: i64,
    #[serde(default)] pub slowest: Vec<StatsTask>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsStatusBucket {
    pub status: String,
    #[serde(default)] pub count: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsHealthSubs {
    #[serde(default)] pub total: i64,
    #[serde(default)] pub by_status: Vec<StatsStatusBucket>,
    #[serde(default)] pub unanswered: Vec<StatsUnanswered>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsUnanswered {
    #[serde(default)] pub id: i64,
    #[serde(default)] pub conv: i64,
    #[serde(default)] pub status: String,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsHealthDocs {
    #[serde(default)] pub documents: i64,
    #[serde(default)] pub revisions: i64,
    #[serde(default)] pub base: i64,
    #[serde(default)] pub delta: i64,
    #[serde(default, rename = "deltaRatio")] pub delta_ratio: f64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsFreeList {
    #[serde(default)] pub free: i64,
    #[serde(default)] pub bytes: i64,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatsMaintenance {
    pub label: String,
    #[serde(default)] pub why: String,
    #[serde(default)] pub cmd: String,
    #[serde(default)] pub danger: bool,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}

/// 老名字是 `Stats*` 的兼容别名：六屏渲染代码不动，但名字里不再藏旧面板语义。
pub type Overview = StatsOverview;
pub type Target = StatsTarget;
pub type DbFiles = StatsDbFiles;
pub type Pragmas = StatsPragmas;
pub type BytesBreakdown = StatsBytes;
pub type TableStat = StatsTable;
pub type IndexStat = StatsIndex;
pub type KindStat = StatsKind;
pub type Integrity = StatsIntegrity;
pub type Counts = StatsCounts;
pub type Conversations = StatsConvs;
pub type ConvRow = StatsConvRow;
pub type ConvTotals = StatsConvTotals;
pub type Conversation = StatsReplay;
pub type ConvEntry = StatsConvEntry;
pub type TaskRow = StatsTask;
pub type DocRow = Value;
pub type SubmissionRow = Value;
pub type Prompt = StatsPrompt;
pub type PromptMessage = StatsPromptMessage;
pub type Cost = StatsCost;
pub type CostTotals = StatsCostTotals;
pub type Health = StatsHealth;
pub type HealthTasks = StatsHealthTasks;
pub type TaskStatusBucket = StatsStatusBucket;
pub type SlowTask = StatsTask;
pub type HealthSubs = StatsHealthSubs;
pub type SubStatusBucket = StatsStatusBucket;
pub type Unanswered = StatsUnanswered;
pub type HealthDocs = StatsHealthDocs;
pub type KindCount = StatsKind;
pub type FreeList = StatsFreeList;
pub type Maintenance = StatsMaintenance;
