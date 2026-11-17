//! qrd3wh `/v1/*` 的协议类型。
//!
//! 纪律：
//! - 未知字段用 `#[serde(flatten)] extra` 收着，原样透传，**不** `deny_unknown_fields`。
//! - 这里只有引擎回的形状，没有业务规则。能否启动、是否生效，引擎都算好了，别在这里再算一遍。
//! - `gen` 是 Rust 2024 的保留字，键名仍是 `"gen"`，代码里写 `r#gen`。

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
    /// 这条会话的工作目录。内核 `GET /v1/sessions` 每一项都带 —— 面板要显示当前值，
    /// 不能只从 `extra` 里摸。
    pub cwd: Option<String>,
    /// 跑在哪台机器上，`null` = 本机。
    pub machine: Option<String>,
    /// `"queued" | "running" | <task state kind> | null`，不是布尔。
    pub busy: Option<serde_json::Value>,
    /// 最后一次落库的时刻，epoch **毫秒**。可能是 null：引擎只记自己看着写过的那些，
    /// 重启前的老会话就是空的。null 就是「不知道」，别拿文件 mtime 之类冒充。
    pub updated_at: Option<i64>,
    pub last: Option<SessionLast>,
    /// 归档墓碑。可为 `null` / 对象；老引擎整个字段缺席时也是 `None`。
    /// `?archived=1` 的全量会话树每一项都带，别拿它当布尔。
    #[serde(default)]
    pub archived: Option<ArchiveTombstone>,
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

// ── 归档库（`GET /v1/archive` 与会话树上的墓碑）──────────────────────────
//
// 全部字段用 Option / #[serde(default)] 兜底：引擎加字段 / 缺字段 / 给 null 都不许解析炸。
// 这里是哑类型，四态（原件在 / 已销毁 / 已捞回 / 无墓碑）由调用方按引擎给的值推，
// 不在这层算业务规则。

/// `GET /v1/archive`：`{ gens: [{ gen, items: [...] }] }`。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArchiveGens {
    #[serde(default)]
    pub gens: Vec<ArchiveGen>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 一批归档（`gen-YYYYMMDD.sqlite`）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArchiveGen {
    /// 日期串，如 `"20261110"`。`gen` 是 Rust 2024 保留字，字段写作 `r#gen`。
    #[serde(default)]
    pub r#gen: String,
    #[serde(default)]
    pub items: Vec<ArchiveItem>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 归档库里的一条会话复印件。
///
/// `parent` / `entries` 用和 `/v1/sessions` 同一套形状（`SessionParent` / 数字），
/// 不在 UI 层再从裸 `Value` 猜（评审 J5）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArchiveItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    /// 分叉来源（`{id, at}`），`null` / 缺席 = 不是分支。
    #[serde(default)]
    pub parent: Option<SessionParent>,
    /// 复印下来的消息条数，不是布尔。缺字段 / 没有条目都按 0。
    #[serde(default)]
    pub entries: u64,
    /// 最后一句摘要，可为 null / 对象 / 缺席。
    #[serde(default)]
    pub last: Option<serde_json::Value>,
    /// 墓碑：`null` = 没归档过；对象 = 归档/撕原件的记录。
    #[serde(default)]
    pub archived: Option<ArchiveTombstone>,
    /// 原件是否还在（引擎算好的，客户端不推）。
    #[serde(default)]
    pub live: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 归档墓碑：`{ gen, purged, at }`。`at` 可为 null；三个键都可能整块缺席。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ArchiveTombstone {
    #[serde(default)]
    pub r#gen: String,
    #[serde(default)]
    pub purged: bool,
    #[serde(default)]
    pub at: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
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

/// `GET /v1/presets/:section` 的 `repo` 块 —— **这一段的**仓状态。
/// `dirty` 按段算（另一段没写完不该让这一段变脏），`ahead` / `behind` 是整仓的。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // 字段是契约，客户端现在只画 head / dirty
pub struct PresetsRepo {
    /// HEAD 的 sha（短），`null` = 仓读不了。
    pub head: Option<String>,
    pub dirty: bool,
    pub ahead: Option<i64>,
    pub behind: Option<i64>,
    /// 仓读不开时引擎给的一句话。
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presets {
    pub section: String,
    pub title: Option<String>,
    pub presets: Vec<Preset>,
    /// 这一段**全部**类别路径（词表 ∪ 预设挂着的），引擎已排好序：父在子前、逐段 `localeCompare('zh-CN')`。
    /// 里面包含零计数的类别 —— 建了就得看得见，客户端不许滤掉。
    #[serde(default)]
    pub categories: Vec<String>,
    /// 这一段的仓状态。整段拉不到时是 `None`，不是错误。
    #[serde(default)]
    pub repo: Option<PresetsRepo>,
    /// 归档区里的条目（`<段>/.archive/<名>`）。**归档不删**，还原就是搬回来。
    #[serde(default)]
    pub archived: Vec<ArchivedPreset>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/presets/<段>/files?name=` —— 这条预设目录里有哪些文件。
///
/// **清单文件名不写死**：段的 `manifest` 住在仓根的 `sections.json` 里
/// （`Session-Template` 是 `preset.json`，`Ultimate-Template` 是 `ultimate.json`），
/// 客户端照这个清单挑，别自己记。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFiles {
    pub name: String,
    #[serde(default)]
    pub files: Vec<PresetFileMeta>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFileMeta {
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/presets/<段>/pending`。仓状态 + **待提交单元**（一条预设一个 unit）。
///
/// `units[].files[].added/removed` 是引擎已经数好的行数 —— 客户端不自己数。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub head: Option<String>,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub ahead: u32,
    #[serde(default)]
    pub behind: u32,
    pub remote: Option<String>,
    #[serde(default)]
    pub units: Vec<PendingUnit>,
    /// 还没推上去的提交（形状没在 UI 用过，先原样收着）。
    #[serde(default)]
    pub ahead_commits: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Pending {
    /// 某条预设的待提交单元（编辑器只关心自己那一条）。
    pub fn unit(&self, key: &str) -> Option<&PendingUnit> {
        self.units.iter().find(|u| u.key == key)
    }
}

/// 一条预设的待提交改动。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingUnit {
    /// 单元键 = 预设目录名。
    pub key: String,
    pub kind: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub tracked: bool,
    #[serde(default)]
    pub files: Vec<PendingFile>,
    #[serde(default)]
    pub added: u32,
    #[serde(default)]
    pub removed: u32,
    #[serde(default)]
    pub code: bool,
    #[serde(default)]
    pub has_deleted: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 单元里的一个文件。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingFile {
    /// 相对**段目录**的路径，`asd/preset.json` 这种。
    pub path: String,
    /// `M` 改 / `A` 新 / `D` 删。
    pub status: String,
    pub kind: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub added: u32,
    #[serde(default)]
    pub removed: u32,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/triggers`。触发器 = 一段「什么时候发起」的声明，参数由它自己定。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Triggers {
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    pub root: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    pub name: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    pub version: Option<String>,
    /// 这个触发器要哪些参数 —— **终极模板的参数表单是照着它画的**，不是写死的。
    #[serde(default)]
    pub params: Vec<TriggerParam>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerParam {
    pub name: String,
    /// `epoch` / `string` / `number` / `boolean` / … 认不出来的一律当文本。
    #[serde(rename = "type")]
    pub ty: String,
    pub label: Option<String>,
    #[serde(default)]
    pub required: bool,
    pub help: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
    pub default: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 归档区里的一条（搬进 `<段>/.archive/<名>` 了，仓里还在）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedPreset {
    pub name: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    /// 引擎给的归档时间（形状不固定，原样收着）。
    pub archived_at: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 一次提交（`GET /v1/presets/<段>/history`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub hash: String,
    /// 短 hash（7 位），列表上显示的。
    pub short: String,
    /// ISO 时间戳（`2026-11-15T19:42:44+08:00`）。
    pub at: Option<String>,
    pub author: Option<String>,
    pub message: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/presets/<段>/history?name=`。这条预设（目录）的提交历史，新到旧。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetHistory {
    pub name: String,
    /// 相对仓根的路径，`Session-Template/asd`。
    pub path: Option<String>,
    #[serde(default)]
    pub commits: Vec<Commit>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/presets/<段>/pending/diff?path=`。两份**全文**，`null` = 那边没有这个文件。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/presets/<段>/file?name=&path=` —— 一个文件的原文 + 乐观锁。
///
/// `etag` 是写回时的 `base`：撞了就是 409，用户得先重读（别丢输入）。
#[derive(Debug, Clone, Deserialize)]
pub struct PresetFile {
    pub name: String,
    pub path: String,
    pub text: String,
    pub etag: String,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// `GET|POST /v1/presets/<段>/validate` 的回包。
///
/// `errors` / `warnings` 的形状由引擎定（可能是字符串，也可能是 `{path, message}`），
/// 所以这里只当作 `Value` 收着，UI 层自己挑一句话显示 —— 不在这层猜形状。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PresetValidation {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub errors: Vec<serde_json::Value>,
    #[serde(default)]
    pub warnings: Vec<serde_json::Value>,
    #[serde(default)]
    pub resolved: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// 一条预设的生命周期动作。三个都是 `POST /v1/presets/<段>/<op>`，体是 `{name}`。
///
/// 语义差别（`wire.md` / 面板文案）：归档**可还原**（移到 `archives/`），彻底删**不可逆**
/// （活库 + 归档副本一起没），而且要过 git 回滚。客户端不自己模拟这两者。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetOp {
    Archive,
    Restore,
    Purge,
}

impl PresetOp {
    /// 路径最后一段。别在这里加逻辑分支 —— 引擎认的就是这三个字。
    pub fn path(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Restore => "restore",
            Self::Purge => "purge",
        }
    }

    /// 是不是不可逆的那一个。UI 拿它决定要不要上「输入名字解锁」那档重锤。
    pub fn irreversible(self) -> bool {
        matches!(self, Self::Purge)
    }
}

/// `POST /v1/presets/<段>/create` 的请求体。
///
/// 只带「建的时候就知道的」四样：名字、摘要、类别、起手式。`spec` 整份原样落的路
/// 这一刀不做（表单里没有盘面），`from: "device"` 让引擎拿设备默认模型填 `spec.model`。
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewPreset {
    pub name: String,
    /// **不能空** —— 引擎直接 400（人靠这一行决定开不开）。
    pub summary: String,
    #[serde(default)]
    pub category: Vec<String>,
    /// `"device"` = 跟设备默认模型起手；不传 = 空白（所有可选键都不写）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// 整份 spec 原样落。**复制一条模板**就靠它（读源模板的原文改个名字发过来）。
    /// 引擎只管形状，并且会用 `name` / `summary` / `category` 覆盖掉里面同名的三个键。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<serde_json::Value>,
    /// 给人看的说明书（`PRESET.md`）。**永不进提示词**，空 = 不落。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// `POST /v1/presets/<段>/categories` 的请求体：类别词表的增 / 删 / 改名。
#[derive(Debug, Clone, Serialize)]
pub struct CategoryOp {
    /// `"add"` | `"rename"` | `"remove"`。
    pub op: String,
    pub name: String,
    /// 只有 `rename` 用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// `POST /v1/presets/<段>/rename` 的入参。**只改名字**：目录（`git mv`，保历史）、
/// 清单里的 `name`、以及引用了它的终极模板的 `session` —— 后两件引擎做，客户端别自己改。
#[derive(Debug, Clone, Serialize)]
pub struct RenamePreset {
    pub name: String,
    pub to: String,
}

/// `POST /v1/presets/<段>/rename` 的回包。
///
/// `refs` 是**连带改掉的终极模板**（名字）。界面得说出来 —— 改名顺手动了别人的东西，
/// 不说一声就是偷偷改。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamedPreset {
    #[serde(default)]
    pub ok: bool,
    pub name: String,
    pub to: String,
    /// 引擎连 `pushed` 一起回（推没推上去）。`None` = 没远端 / 没东西可推。
    #[serde(default)]
    pub pushed: Option<bool>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub commit: serde_json::Value,
}

/// `POST /v1/presets/<段>/order` 的入参：**完整顺序**（界面上拖完那一下的全量），
/// 不是「谁挪到谁前面」—— 一次落盘一次提交，服务端不用重放一串移动。
#[derive(Debug, Clone, Serialize)]
pub struct ReorderPreset {
    pub order: Vec<String>,
}

/// `POST /v1/presets/<段>/order` 的回包。`order` 是**盘上真正存下来的那份**（服务端会把
/// 已经不存在的名字滤掉），客户端拿它覆盖本地的乐观顺序。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderedPreset {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub pushed: Option<bool>,
}

/// `POST /v1/presets/<段>/categories` 的回包。引擎顺手把新的名册 / 词表带回来 ——
/// 客户端**别自己拼**（改名会连带改预设上的挂载，本地猜必错）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoriesResult {
    #[serde(default)]
    pub categories: Vec<String>,
    /// 引擎顺带回的名册。**形状和 `Presets` 端点不一样** —— 这里没有 `tracked`
    /// 和 `resolved`（那两个只在 `GET /presets/:section` 里补）。所以客户端不解析它，
    /// 拿到 `categories` 之后重拉一次那一段，别拿半份名册覆盖本地完整数据。
    #[serde(default)]
    pub presets: Vec<serde_json::Value>,
    /// 引擎说碰了几个文件。**是数字**（不是文件名数组），当 `Value` 收着最稳。
    #[serde(default)]
    pub touched: serde_json::Value,
    #[serde(default)]
    pub load_error: Option<String>,
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

/// `POST /v1/sessions/:id/rename` 的响应（200）。
///
/// 引擎只回这两个字段 —— 名字是**它存下来的那个**（trim 过、空的话它会拒绝并回 400
/// `{"error":"name 是空的"}`）。客户端拿 `name` 覆盖本地，别把请求值直接写上去。
#[derive(Debug, Clone, Deserialize)]
pub struct Renamed {
    pub id: i64,
    pub name: String,
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

/// `GET /v1/extensions` —— 扩展清单页吃这个。
///
/// 这里有**五类不同的东西**，别当成一个数组：
/// `catalog`（仓库里的）/ `builtin`（引擎自带，只有名字）/ `variants`（本机 checkout）/
/// `archived`（`.archive/` 里）/ 以及 `installed` + `loadable` 这两个名字清单。
#[derive(Debug, Clone, Deserialize)]
pub struct Extensions {
    #[serde(default)]
    pub installed: Vec<String>,
    #[serde(default)]
    pub loadable: Vec<String>,
    /// 引擎自带的（`coding-tools`），不在仓库里，也没有 summary。
    #[serde(default)]
    pub builtin: Vec<String>,
    /// 引擎启动路径上硬编码 `import` 的那三条。`uninstall` 会 403。
    /// **会话级勾选不受这个限制** —— 这里不是卸载，是不带。
    #[serde(default)]
    pub defaults: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub protected: Vec<String>,
    #[serde(default)]
    pub catalog: Vec<Extension>,
    #[serde(default)]
    pub archived: Vec<ArchivedExtension>,
    /// `{ "123": [{ id, key, dir, branch, installed, load: {state,error} }] }`
    #[serde(default)]
    pub variants: HashMap<String, Vec<ExtVariant>>,
    /// 只用来显示「加载失败」的那几条：`{ "123": {state,error,version} }`。
    #[serde(default)]
    pub load: HashMap<String, ExtLoad>,
    pub repo: Option<serde_json::Value>,
    pub nameplates: Option<serde_json::Value>,
    pub local: Option<serde_json::Value>,
    pub live: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub name: String,
    pub version: Option<String>,
    pub summary: Option<String>,
    /// 往系统提示词里塞哪几段。扩展真正的价值就在这儿。
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default)]
    pub category: Vec<String>,
    /// 仓库里标了 `default` 的（`nous` / `skills` / `prompts`）。
    #[serde(default, rename = "default")]
    pub is_default: bool,
    #[serde(default)]
    pub tracked: bool,
    pub file: Option<String>,
    pub entry: Option<serde_json::Value>,
    pub needs: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedExtension {
    pub name: String,
    pub version: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    pub archived_at: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtVariant {
    pub id: Option<String>,
    pub key: Option<String>,
    pub dir: Option<String>,
    pub branch: Option<String>,
    #[serde(default)]
    pub unmerged: u32,
    #[serde(default)]
    pub installed: bool,
    pub load: Option<ExtLoad>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtLoad {
    pub state: Option<String>,
    pub error: Option<String>,
    pub version: Option<String>,
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
    /// 私库根目录：落盘预览和状态栏要写全路径，只写相对路径等于让人猜。
    #[serde(default)]
    pub root: Option<String>,
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
    /// `false` = 还没进 HEAD。**引擎在这个 GET 里就给了**，不用另打 `/pending`。
    #[serde(default)]
    pub tracked: bool,
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

/// `POST /v1/sessions/:id/config` 的回包。**回的是生效值，不是请求值** ——
/// 服务端先 `configure` 再读 agent 状态回来，前端拿它重画就不会跟真相分叉。
///
/// 所以面板改完一项，**必须拿这个回包去覆盖本地会话**，而不是自己把改的值写上去：
/// 比如换了模型，服务端会把档位按新模型重夹一次，那个结果只能从这里拿到。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    pub id: i64,
    #[serde(default)]
    pub skills: Vec<String>,
    pub model: Option<String>,
    pub thinking_level: Option<serde_json::Value>,
    pub template: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub machine: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// `POST /v1/sessions/:id/config` 的**请求**体。
///
/// 每个字段都是 `Option`，但这里有两层不同的「不传」：
/// - `Option<String>` / `Option<Vec<_>>` / `Option<bool>`：`None` = 这次不动它。
/// - `Option<Option<String>>`：内核里 `null` 是**一个有效选择**（模板：明确不要；
///   机器：回本机；目录：清回缺省）。所以外层 `None` = 不动，`Some(None)` = 传 `null`，
///   `Some(Some(v))` = 换成 `v`。塌成一层就会把「不要」和「没说」混成一件事 ——
///   内核那句注释专门写了这个坑。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    /// `"provider/modelId"`，比如 `Qoder/DeepSeek-Flash`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
    /// **相对改法**：内核只增删差集，不是把完整数组写回去 ——
    /// 写完整数组会把这条会话的扩展选择冻住，宿主以后往默认选择里加东西就再也进不来了。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
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
    /// 上一级。引擎在 `/` 时不给这个键。**自己算 `rsplit('/')` 会算错根**，
    /// 所以照抄它。
    #[serde(default)]
    pub parent: Option<String>,
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

/// `GET /v1/combos` 里的一条。开着没开由引擎算（`enabled` 缺省 true），客户端不自己读 spec。
#[derive(Debug, Clone, Deserialize)]
pub struct Combo {
    pub name: String,
    pub trigger: Option<String>,
    pub params: Option<serde_json::Value>,
    pub session: Option<String>,
    /// 引擎算好的开关状态。缺省 true —— 和 `presets.js` 的 `spec.enabled !== false` 一个字不差。
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 为什么跑不起来（写坏了 / 引用没了）。空串 = 没问题。
    #[serde(default)]
    pub problem: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool { true }

/// `GET /v1/combos` 的整包。调度器节拍也在里面（引擎给的，客户端只显示）。
#[derive(Debug, Clone, Deserialize)]
pub struct Combos {
    #[serde(default)]
    pub combos: Vec<Combo>,
    pub scheduler: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `POST /v1/combos/:name/enable|disable` 的回包。
#[derive(Debug, Clone, Deserialize)]
pub struct ComboChanged {
    pub combo: Combo,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
