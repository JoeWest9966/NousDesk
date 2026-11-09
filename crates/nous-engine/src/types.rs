//! qrd3wh `/v1/*` 的协议类型。
//!
//! 纪律：
//! - 未知字段用 `#[serde(flatten)] extra` 收着，原样透传，**不** `deny_unknown_fields`。
//! - 这里只有引擎回的形状，没有业务规则。能否启动、是否生效，引擎都算好了，别在这里再算一遍。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// `GET /v1/sessions` 返回的整棵树。
#[derive(Debug, Clone, Deserialize)]
pub struct Sessions {
    pub items: Vec<Session>,
}

/// 会话树上的一个节点。`parent != null` = 这是一条 fork 分支。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: i64,
    pub name: String,
    pub parent: Option<SessionParent>,
    pub owner: Option<serde_json::Value>,
    /// `"provider/modelId"`，例如 `"volc-ark/deepseek-v3"`。
    pub model: Option<String>,
    /// 生效的思考档，不是请求值。
    pub thinking_level: Option<serde_json::Value>,
    pub thinking_levels: Vec<serde_json::Value>,
    pub skills: Vec<String>,
    pub template: Option<String>,
    pub read_only: bool,
    /// `"queued" | "running" | <task state kind> | null`，不是布尔。
    pub busy: Option<serde_json::Value>,
    /// 最后一次落库的时刻，epoch **毫秒**。可能是 null：引擎只记自己看着写过的那些，
    /// 重启前的老会话就是空的。null 就是「不知道」，别拿文件 mtime 之类冒充。
    pub updated_at: Option<i64>,
    pub last: Option<SessionLast>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionParent {
    pub id: i64,
    pub at: Option<serde_json::Value>,
}

/// 列表里那条「最后一句」的摘要。`text` 可以是 null（消息没正文时）。
#[derive(Debug, Clone, Deserialize)]
pub struct SessionLast {
    pub id: serde_json::Value,
    pub kind: String,
    pub text: Option<String>,
}

/// `GET /v1/sessions/:id/entries` 返回的历史。**新的在前**，客户端要翻正。
#[derive(Debug, Clone, Deserialize)]
pub struct Entries {
    pub items: Vec<Entry>,
    pub next: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub id: serde_json::Value,
    pub kind: String,
    /// 可为 null（比如空回复的 assistant 气泡）。
    pub text: Option<String>,
    pub parent: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `POST /v1/sessions/:id/messages` 的响应（不传 `waitMs` 时）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submission {
    #[serde(rename = "submissionId")]
    pub submission_id: serde_json::Value,
    pub status: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/unread`。**平铺的 map**，一个会话一个键，没有 `items` 包一层。
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Unread(pub HashMap<String, UnreadState>);

impl Unread {
    /// 按会话 id 查。键是字符串，`id.to_string()` 对上就行。
    pub fn of(&self, id: i64) -> Option<&UnreadState> {
        self.0.get(&id.to_string())
    }

    /// 没在表里的会话算 0 未读 —— 引擎说「覆盖所有会话」，但客户端别在上面押命。
    pub fn count(&self, id: i64) -> u64 {
        self.of(id).map(|s| s.unread).unwrap_or(0)
    }

    /// 把 `POST /v1/read/:id` 的回执落进表里。
    ///
    /// **照抄引擎给的数**（`unread` / `readAt`），不在客户端拿旧值减一减 ——
    /// 水位是服务端的事，这边只是把答案记下来。
    pub fn apply(&mut self, r: &ReadReceipt) {
        let st = self.0.entry(r.id.to_string()).or_default();
        st.unread = r.unread;
        st.read_at = r.read_at;
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UnreadState {
    /// 引擎按 EntryId 水位数好的条数，**客户端不自己数**。
    #[serde(default)]
    pub unread: u64,
    /// 上次标已读的时刻，epoch ms。可能为 null（水位还没被 `POST /v1/read/:id` 推过）。
    /// **null 不等于「读到过」**。
    pub read_at: Option<i64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `POST /v1/read/:id` 的回执：`{ id, readAt, unread: 0 }`。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadReceipt {
    pub id: i64,
    pub read_at: Option<i64>,
    pub unread: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `POST /v1/sessions/:id/fork` 的回执：`{ id, at }`。`id` 是分出来的新会话。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forked {
    pub id: i64,
    /// 在父会话的哪条 entry 上分的叉。
    pub at: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/device`。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub self_: Option<String>,
    pub origin: Option<String>,
    pub node: Option<String>,
    pub started_at: Option<serde_json::Value>,
    pub model: Option<serde_json::Value>,
    pub faux: Option<bool>,
    pub data: Option<String>,
    pub catalog: Option<serde_json::Value>,
    pub available: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/presets/:section` 的一条模板。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    pub summary: String,
    pub description: String,
    pub category: Vec<String>,
    pub spec: Option<serde_json::Value>,
    /// `Some(..)` = 这条自己写坏了（JSON / 形状）。
    pub spec_error: Option<String>,
    /// `false` = 盘上有、仓里没。
    pub tracked: bool,
    pub dir: String,
    /// 引用校验结果。`resolved.*.state == "invalid"` = 现在开不了。
    pub resolved: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Presets {
    pub section: String,
    pub title: Option<String>,
    pub presets: Vec<Preset>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `POST /v1/sessions` 的请求体。
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateSession {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<serde_json::Value>,
    // 这几个键是 camelCase：`hb7k3j` 的面板就是照这个名字发的，照抄，别自己翻译。
    #[serde(rename = "thinkingLevel", skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    // **总是发**（`null` 也是有效值 = 明确不要）。省掉它服务端会用仓库默认值，
    // 就会出现「我明明点了无模板，它又给我塞回来」。所以这层是 `Option<Option<_>>`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// `None` = 本机。名字要和 `GET /v1/machines` 里的 `name` 对上。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    #[serde(rename = "readOnly", skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

/// `POST /v1/sessions` 的响应（201）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedSession {
    pub id: i64,
    pub name: String,
    pub model: Option<String>,
    pub thinking_level: Option<serde_json::Value>,
    pub skills: Vec<String>,
    pub template: Option<String>,
    pub extensions: Vec<String>,
    pub read_only: bool,
    pub preset: Option<String>,
    pub missing: Vec<String>,
    #[serde(rename = "missingExtensions", default)]
    pub missing_extensions: Vec<String>,
    #[serde(rename = "missingTemplates", default)]
    pub missing_templates: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── 建会话那一页要用的四份清单（盘面四个角 + 中央两个胶囊）─────────────────
//
// 全部来自 qrd3wh：`/v1/models` `/v1/skills` `/v1/prompts` `/v1/machines` `/v1/fs/dirs`。
// 这里只描述形状，不做选择、不排序 —— 「哪些能选」是引擎算的。

/// `GET /v1/models`。`default` 是盘上那只眼睛当前指着的模型。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Models {
    pub default: Option<ModelRef>,
    #[serde(default)]
    pub items: Vec<Model>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Models {
    /// 默认模型摊成 `provider/modelId`，UI 只用这一个键（`picked` 也是它）。
    pub fn default_key(&self) -> Option<String> {
        self.default.as_ref().map(ModelRef::key)
    }
}

/// `{provider, modelId}`。建会话时**原样发回**这个对象。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRef {
    pub provider: String,
    pub model_id: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ModelRef {
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider, self.model_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub provider: String,
    pub model_id: String,
    pub name: Option<String>,
    /// 这个模型支持的思考档，**从少想到多想**。换模型要拿它重新夹当前档位。
    #[serde(default)]
    pub thinking_levels: Vec<String>,
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Model {
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider, self.model_id)
    }

    /// 给「档位只对它支持的模型出」用的：不支持思考档的模型给一个 `off`。
    pub fn levels(&self) -> Vec<String> {
        if self.thinking_levels.is_empty() {
            vec!["off".to_string()]
        } else {
            self.thinking_levels.clone()
        }
    }
}

/// `GET /v1/skills`。只列 `load.state == "ok" && !hidden` 的那些（判据在 UI，不在类型里）。
#[derive(Debug, Clone, Deserialize)]
pub struct Skills {
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub root: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    /// `true` = `disable-model-invocation`，进不了 `<available_skills>`，勾上也不动。
    #[serde(default, rename = "hidden")]
    pub hidden: bool,
    #[serde(default)]
    pub tracked: bool,
    pub dir: Option<String>,
    pub file: Option<String>,
    /// `{ state: "ok" | ... }`。不是 `ok` 的技能加载不出来，别让人勾。
    pub load: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Skill {
    pub fn loadable(&self) -> bool {
        self.load
            .as_ref()
            .and_then(|l| l.get("state"))
            .and_then(|s| s.as_str())
            == Some("ok")
    }
}

/// `GET /v1/prompts`。默认值来自仓库的 `settings.json`。
#[derive(Debug, Clone, Deserialize)]
pub struct Prompts {
    #[serde(default)]
    pub prompts: Vec<Prompt>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub default: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub name: String,
    pub description: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    pub load: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Prompt {
    pub fn loadable(&self) -> bool {
        self.load
            .as_ref()
            .and_then(|l| l.get("state"))
            .and_then(|s| s.as_str())
            == Some("ok")
    }
}

/// `GET /v1/machines`。会话跑在哪台机器上，`null` = 本机。
#[derive(Debug, Clone, Deserialize)]
pub struct Machines {
    pub at: Option<i64>,
    #[serde(default)]
    pub machines: Vec<Machine>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub name: String,
    pub addr: Option<String>,
    pub user: Option<String>,
    pub port: Option<u32>,
    #[serde(default)]
    pub ips: Vec<MachineIp>,
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MachineIp {
    pub addr: String,
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/fs/dirs`。**列的是 qrd3wh 那台机器上的目录** —— 引擎跑在远程时，
/// 这一页一行都不用改。
#[derive(Debug, Clone, Deserialize)]
pub struct Dirs {
    /// 请求时没给 `path` 就为空串（= 起点）。
    pub path: Option<String>,
    #[serde(default)]
    pub dirs: Vec<DirEntry>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DirEntry {
    /// 给眼睛看的名字（通常就是最后一段）。
    pub name: String,
    /// 规范绝对路径，选它就是把这一串发给引擎。
    pub path: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
