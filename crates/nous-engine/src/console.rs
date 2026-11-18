//! d6s4v1（`agent.sqlite` 的 X 光机）的哑客户端。
//!
//! 它**不是引擎**：Rust 版 `/v1/*` 由 `EngineClient` 负责，这个面板走宿主自己的 `/api/*`。
//! 地址同样只有一个来源 —— 配置里那一个 host，别在调用点拼路径。
//!
//! ```text
//! GET api/overview                → 文件 / pragmas / 表 / 索引 / integrity
//! GET api/conversations           → 全部会话一行一条
//! GET api/conversation?id=N       → 一条会话的 entries / tasks / documents / prompt
//! GET api/cost                    → 账本：总量 + byDay / byHour / byModel / byConv
//! GET api/health                  → 维修：卡住的任务 / 没人接的提交 / WAL / 维护建议
//! ```
//!
//! 规矩和 [`crate::client`] 一样：**只转发**，不聚合、不算派生状态；未知字段用
//! `Value` / `flatten` 收着透传 —— 面板每加一个读数，这边不该编译不过。
//!
//! 每份响应里还带一段 `diagrams`（mermaid 源码），桌面端不渲 mermaid，
//! 但照样收着：哪天要做「导出这张图」就不用改协议层。

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use url::Url;



#[derive(Debug, thiserror::Error)]
pub enum ConsoleError {
    #[error("网络：{0}")]
    Http(#[from] reqwest::Error),
    #[error("URL 拼不起来：{0}")]
    Url(#[from] url::ParseError),
    #[error("面板回 {status}：{message}")]
    Panel { status: u16, message: String },
}

/// 面板崩的时候回的是整页 HTML（502 网关那种）。原样塞进界面会把红框撑爆，
/// 所以压平空白、去掉标签，只留一句能读的。
fn flatten(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(240));
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag && !c.is_control() => out.push(c),
            _ => {}
        }
    }
    let flat = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 160 {
        let mut s: String = flat.chars().take(159).collect();
        s.push('…');
        s
    } else {
        flat
    }
}

// ── 通用小件 ─────────────────────────────────────────────────────────

/// 一堆「label → 数字」的桶（byDay / byHour / byModel / byConv 共用）。
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

// ── overview ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Overview {
    pub target: Target,
    pub file: DbFiles,
    pub pragmas: Pragmas,
    #[serde(default)]
    pub bytes: BytesBreakdown,
    pub counts: Counts,
    #[serde(default)]
    pub tables: Vec<TableStat>,
    #[serde(default)]
    pub indexes: Vec<IndexStat>,
    #[serde(default, rename = "entriesByKind")]
    pub entries_by_kind: Vec<KindStat>,
    #[serde(default)]
    pub integrity: Integrity,
    /// 面板给的「值得看一眼」的条目，形状还没定死 —— 原样收着。
    #[serde(default)]
    pub attention: Vec<Value>,
    #[serde(default)]
    pub diagrams: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub id: String,
    #[serde(default)]
    pub own: bool,
    pub path: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `db` / `wal` / `shm` 三个文件的大小与时间戳。`mtime` 是带小数的 epoch 秒。
#[derive(Debug, Clone, Deserialize)]
pub struct DbFiles {
    #[serde(default)]
    pub db: i64,
    #[serde(default)]
    pub wal: i64,
    #[serde(default)]
    pub shm: i64,
    #[serde(default)]
    pub mtime: f64,
    #[serde(default, rename = "walMtime")]
    pub wal_mtime: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pragmas {
    #[serde(default, rename = "pageSize")]
    pub page_size: i64,
    #[serde(default, rename = "pageCount")]
    pub page_count: i64,
    #[serde(default)]
    pub freelist: i64,
    #[serde(default, rename = "journalMode")]
    pub journal_mode: String,
    #[serde(default, rename = "autoVacuum")]
    pub auto_vacuum: i64,
    #[serde(default, rename = "userVersion")]
    pub user_version: i64,
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Pragmas {
    /// 页数 × 页大小。`page_count` 缺了就当 0（面板偶尔只给 bytes）。
    pub fn db_bytes(&self) -> i64 {
        self.page_size.saturating_mul(self.page_count)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BytesBreakdown {
    #[serde(default)]
    pub db: i64,
    #[serde(default)]
    pub wal: i64,
    #[serde(default)]
    pub shm: i64,
    #[serde(default)]
    pub free: i64,
    #[serde(default)]
    pub used: i64,
    #[serde(default, rename = "onDisk")]
    pub on_disk: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 每张表的行数。字段名是**表名**，所以只能用 map 收 —— 写死就漏了新表。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Counts {
    #[serde(flatten)]
    pub by_table: BTreeMap<String, i64>,
}

impl Counts {
    pub fn get(&self, table: &str) -> i64 {
        self.by_table.get(table).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableStat {
    pub name: String,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub pages: i64,
    #[serde(default)]
    pub rows: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexStat {
    pub name: String,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub pages: i64,
    #[serde(default)]
    pub table: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KindStat {
    pub kind: String,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub bytes: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Integrity {
    #[serde(default)]
    pub checked: bool,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub messages: Vec<String>,
}

// ── conversations ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Conversations {
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub list: Vec<ConvRow>,
    #[serde(default)]
    pub totals: ConvTotals,
    #[serde(default)]
    pub diagrams: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConvRow {
    pub id: i64,
    #[serde(default, rename = "ownerConv")]
    pub owner_conv: Option<i64>,
    #[serde(default, rename = "ownerTask")]
    pub owner_task: Option<i64>,
    #[serde(default)]
    pub entries: i64,
    #[serde(default)]
    pub tokens: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default, rename = "toolCalls")]
    pub tool_calls: i64,
    #[serde(default)]
    pub models: Vec<String>,
    /// epoch 毫秒。缺了当 0（时间线画不出来那条，但整份清单不该解析失败）。
    #[serde(default, rename = "firstAt")]
    pub first_at: i64,
    #[serde(default, rename = "lastAt")]
    pub last_at: i64,
    #[serde(default)]
    pub title: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConvTotals {
    #[serde(default)]
    pub entries: i64,
    #[serde(default)]
    pub tokens: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ── conversation?id=N ────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Conversation {
    pub id: i64,
    #[serde(default, rename = "ownerConv")]
    pub owner_conv: Option<i64>,
    #[serde(default, rename = "ownerTask")]
    pub owner_task: Option<i64>,
    #[serde(default)]
    pub entries: Vec<ConvEntry>,
    #[serde(default)]
    pub tasks: Vec<TaskRow>,
    #[serde(default)]
    pub documents: Vec<DocRow>,
    #[serde(default)]
    pub submissions: Vec<SubmissionRow>,
    #[serde(default)]
    pub prompt: Prompt,
    #[serde(default)]
    pub diagrams: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConvEntry {
    pub id: i64,
    #[serde(default)]
    pub conv: i64,
    #[serde(default)]
    pub seq: i64,
    #[serde(default)]
    pub head: Option<i64>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub tokens: i64,
    #[serde(default)]
    pub cost: f64,
    /// usage 的结构随 provider 变 —— 只用它的数字就别在这儿写死字段。
    #[serde(default)]
    pub usage: Value,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default, rename = "toolCalls")]
    pub tool_calls: Value,
    #[serde(default, rename = "isError")]
    pub is_error: bool,
    #[serde(default)]
    pub ms: Option<i64>,
    #[serde(default, rename = "stopReason")]
    pub stop_reason: Option<String>,
    #[serde(default, rename = "toolsAdded")]
    pub tools_added: Value,
    #[serde(default)]
    pub chars: i64,
    #[serde(default)]
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskRow {
    pub id: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default, rename = "startedAt")]
    pub started_at: Option<i64>,
    #[serde(default, rename = "endedAt")]
    pub ended_at: Option<i64>,
    #[serde(default)]
    pub ms: Option<i64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocRow {
    pub id: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub revisions: i64,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default, rename = "createdAt")]
    pub created_at: i64,
    #[serde(default)]
    pub retired: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmissionRow {
    pub id: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default, rename = "requestId")]
    pub request_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Prompt {
    #[serde(default, rename = "upTo")]
    pub up_to: Option<i64>,
    #[serde(default)]
    pub messages: Vec<PromptMessage>,
    #[serde(default)]
    pub totals: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromptMessage {
    pub id: i64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub chars: i64,
    #[serde(default)]
    pub tokens: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub head: bool,
    #[serde(default)]
    pub text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ── cost ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Cost {
    #[serde(default)]
    pub totals: CostTotals,
    #[serde(default, rename = "byDay")]
    pub by_day: Vec<CostBucket>,
    #[serde(default, rename = "byHour")]
    pub by_hour: Vec<CostBucket>,
    #[serde(default, rename = "byModel")]
    pub by_model: Vec<CostBucket>,
    #[serde(default, rename = "byConv")]
    pub by_conv: Vec<CostBucket>,
    /// 「cost 为 0 不等于免费」那句原文 —— 显示时别自己编。
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub diagrams: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CostTotals {
    #[serde(default)]
    pub entries: i64,
    #[serde(default)]
    pub tokens: i64,
    #[serde(default)]
    pub input: i64,
    #[serde(default)]
    pub output: i64,
    #[serde(default, rename = "cacheRead")]
    pub cache_read: i64,
    #[serde(default, rename = "cacheWrite")]
    pub cache_write: i64,
    #[serde(default)]
    pub reasoning: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl CostTotals {
    /// 输入里有多大比例是命中缓存的。总量为 0 时返回 0，别算出 NaN。
    pub fn cache_hit(&self) -> f32 {
        let billed = self.input + self.cache_read;
        if billed <= 0 {
            return 0.;
        }
        self.cache_read as f32 / billed as f32
    }
}

// ── health ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Health {
    #[serde(default)]
    pub tasks: HealthTasks,
    #[serde(default)]
    pub submissions: HealthSubs,
    #[serde(default)]
    pub documents: HealthDocs,
    #[serde(default, rename = "walBytes")]
    pub wal_bytes: i64,
    #[serde(default)]
    pub freelist: FreeList,
    #[serde(default)]
    pub maintenance: Vec<Maintenance>,
    #[serde(default)]
    pub diagrams: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HealthTasks {
    #[serde(default)]
    pub total: i64,
    #[serde(default, rename = "byStatus")]
    pub by_status: Vec<TaskStatusBucket>,
    #[serde(default)]
    pub stuck: Vec<Value>,
    #[serde(default)]
    pub sleeping: Vec<Value>,
    #[serde(default, rename = "avgMs")]
    pub avg_ms: i64,
    #[serde(default)]
    pub p95: i64,
    #[serde(default)]
    pub slowest: Vec<SlowTask>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskStatusBucket {
    pub status: String,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub sample: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlowTask {
    pub id: i64,
    #[serde(default)]
    pub conv: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub background: bool,
    #[serde(default, rename = "startedAt")]
    pub started_at: i64,
    #[serde(default, rename = "endedAt")]
    pub ended_at: Option<i64>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HealthSubs {
    #[serde(default)]
    pub total: i64,
    #[serde(default, rename = "byStatus")]
    pub by_status: Vec<SubStatusBucket>,
    #[serde(default)]
    pub unanswered: Vec<Unanswered>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubStatusBucket {
    pub status: String,
    #[serde(default)]
    pub n: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Unanswered {
    pub id: i64,
    #[serde(default)]
    pub conv: i64,
    #[serde(default)]
    pub status: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HealthDocs {
    #[serde(default)]
    pub documents: i64,
    #[serde(default, rename = "byKind")]
    pub by_kind: Vec<KindCount>,
    #[serde(default)]
    pub revisions: i64,
    #[serde(default)]
    pub base: i64,
    #[serde(default)]
    pub delta: i64,
    /// delta 占 revisions 的比例（0..1）。面板算好了，别在这边除。
    #[serde(default, rename = "deltaRatio")]
    pub delta_ratio: f64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KindCount {
    pub kind: String,
    #[serde(default)]
    pub n: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FreeList {
    #[serde(default)]
    pub free: i64,
    #[serde(default)]
    pub bytes: i64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Maintenance {
    pub label: String,
    #[serde(default)]
    pub why: String,
    #[serde(default)]
    pub cmd: String,
    /// 危险动作用红字画，别让「VACUUM」看起来跟「看日志」一样轻。
    #[serde(default)]
    pub danger: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ── 客户端 ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ConsoleClient {
    http: reqwest::Client,
    /// 已经带到 `<host>/api/` 的基地址。
    base: Url,
}

impl ConsoleClient {
    /// `host` 是配置里那一个地址（`http://127.0.0.1:4790`）。
    pub fn new(host: &str, timeout: Duration) -> Result<Self, ConsoleError> {
        let mut url = Url::parse(host)?;
        let prefix = url.path().trim_end_matches('/').to_string();
        url.set_path(&format!("{prefix}/api/"));
        let http = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(Self { http, base: url })
    }

    fn url(&self, path: &str) -> Result<Url, ConsoleError> {
        Ok(self.base.join(path.trim_start_matches('/'))?)
    }

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, ConsoleError> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .unwrap_or_else(|| text.chars().take(200).collect());
            return Err(ConsoleError::Panel {
                status: status.as_u16(),
                message: flatten(&message),
            });
        }
        serde_json::from_str(&text).map_err(|e| ConsoleError::Panel {
            status: status.as_u16(),
            message: format!("JSON 不像话：{e}"),
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ConsoleError> {
        let resp = self.http.get(self.url(path)?).send().await?;
        self.json(resp).await
    }

    pub async fn overview(&self) -> Result<Overview, ConsoleError> {
        self.get("overview").await
    }

    pub async fn conversations(&self) -> Result<Conversations, ConsoleError> {
        self.get("conversations").await
    }

    pub async fn conversation(&self, id: i64) -> Result<Conversation, ConsoleError> {
        self.get(&format!("conversation?id={id}")).await
    }

    pub async fn cost(&self) -> Result<Cost, ConsoleError> {
        self.get("cost").await
    }

    pub async fn health(&self) -> Result<Health, ConsoleError> {
        self.get("health").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 地址按_host_拼成面板_api() {
        let c = ConsoleClient::new("http://192.168.1.7:4790", Duration::from_secs(1))
            .expect("拼得起来");
        assert_eq!(
            c.url("overview").unwrap().as_str(),
            "http://192.168.1.7:4790/api/overview"
        );
    }

    #[test]
    fn 宿主带尾巴斜杠和路径也不双斜杠() {
        let c = ConsoleClient::new("http://h:1/nous/", Duration::from_secs(1)).unwrap();
        assert_eq!(
            c.url("cost").unwrap().as_str(),
            "http://h:1/nous/api/cost"
        );
    }

    #[test]
    fn 未知字段收着不炸() {
        let json = r#"{
            "target": {"id": "qrd3wh", "own": false, "path": "/x/agent.sqlite", "新字段": 1},
            "file": {"db": 10, "wal": 2, "shm": 1, "mtime": 1.5, "walMtime": 2.5},
            "pragmas": {"pageSize": 4096, "pageCount": 2, "freelist": 0,
                        "journalMode": "wal", "autoVacuum": 0,
                        "userVersion": 0, "schemaVersion": 2},
            "counts": {"entries": 216, "将来加的表": 3},
            "tables": [], "indexes": [], "entriesByKind": [],
            "integrity": {"checked": true, "ok": true, "messages": []},
            "attention": [{"随便": "什么"}],
            "以后才有的顶层字段": [1, 2]
        }"#;
        let o: Overview = serde_json::from_str(json).expect("多字段不该解析失败");
        assert_eq!(o.counts.get("entries"), 216);
        assert_eq!(o.counts.get("将来加的表"), 3);
        assert_eq!(o.counts.get("没这张表"), 0);
        assert!(o.extra.contains_key("以后才有的顶层字段"));
        assert_eq!(o.pragmas.db_bytes(), 8192);
    }

    #[test]
    fn 账本命中率按输入_加_缓存读算() {
        let t = CostTotals {
            input: 118_500,
            cache_read: 448_855,
            ..Default::default()
        };
        let hit = t.cache_hit();
        assert!((hit - 0.7909).abs() < 0.001, "算出来是 {hit}");
    }

    #[test]
    fn 输入为零时命中率是零不是_nan() {
        assert_eq!(CostTotals::default().cache_hit(), 0.);
    }

    #[test]
    fn 会话缺_models_和_title_也能解析() {
        let json = r#"{"count":1,"list":[{"id":7,"entries":3,"tokens":0,"cost":0,
                        "toolCalls":0,"firstAt":1,"lastAt":2}],
                        "totals":{"entries":3,"tokens":0,"cost":0}}"#;
        let c: Conversations = serde_json::from_str(json).unwrap();
        assert_eq!(c.list[0].id, 7);
        assert!(c.list[0].title.is_empty());
        assert!(c.list[0].models.is_empty());
    }

    #[test]
    fn 回放里_usage_形状随便给都不炸() {
        let json = r#"{"id":2,"entries":[
            {"id":9,"conv":2,"seq":3,"kind":"pi.user","role":"user","bytes":118,
             "at":1,"tokens":0,"cost":0,"usage":null,"text":"hi"},
            {"id":10,"conv":2,"seq":4,"kind":"pi.assistant","role":"assistant","bytes":9,
             "at":2,"tokens":5,"cost":0,"usage":{"input":3,"cacheRead":2,"怪字段":true},
             "toolCalls":[{"name":"bash"}],"text":"ho"}]}"#;
        let c: Conversation = serde_json::from_str(json).unwrap();
        assert_eq!(c.entries.len(), 2);
        assert_eq!(c.entries[1].usage["cacheRead"], 2);
        assert_eq!(c.entries[1].tool_calls[0]["name"], "bash");
        assert!(c.prompt.messages.is_empty());
    }

    #[test]
    fn 维修页缺字段也只是零而不是整页失败() {
        let json = r#"{"tasks":{"total":136,"byStatus":[],"stuck":[],"sleeping":[],
                        "avgMs":6838,"p95":18254,"slowest":[]},
                        "submissions":{"total":62,"byStatus":[],"unanswered":[]},
                        "documents":{"documents":931,"byKind":[],"revisions":1624,
                        "base":943,"delta":681,"deltaRatio":0.4193},
                        "walBytes":486192,"freelist":{"free":1,"bytes":4096},
                        "maintenance":[]}"#;
        let h: Health = serde_json::from_str(json).unwrap();
        assert_eq!(h.tasks.total, 136);
        assert_eq!(h.documents.delta, 681);
        assert_eq!(h.maintenance.len(), 0);
    }
}
