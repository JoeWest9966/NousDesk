//! 元素库（组 / 包）：`/v1/elements*` 的读写形状。
//!
//! 单独一个模块而不是塞进 `types.rs`：`types.rs` 是各页的 GET 名册，这里是
//! **一份 JSON 一个元素**的读写面 —— 两段（`groups/` / `packages/`）共用一副骨架，
//! 而且带写入回执与幂等锚。变化的节奏跟名册不一样。
//!
//! 端点（引擎 `crates/nous/server/src/elements.rs`）：
//! - `GET /v1/elements`                      → `{ kind, repo, groups[], packages[], resolved }`
//! - `GET /v1/elements/{kind}`               → `{ kind, items[] }`
//! - `GET /v1/elements/{kind}/{id}`          → `{ kind, id, fields, etag }`
//! - `PUT /v1/elements/{kind}/{id}`          → `{ fields, base?, requestId? }`
//! - `POST /v1/elements/{kind}`              → 同 PUT，`id` 从 `fields.id` 取，201
//! - `DELETE /v1/elements/{kind}/{id}`       → `?base=&requestId=`
//! - `POST /v1/elements/push`                → `{ kind, pushed, repo }`
//!
//! **未知字段一律 `flatten` 收着**：引擎以后加字段不能让这边编译不过，也不许
//! `deny_unknown_fields`（AGENTS.md 工程规矩）。

use std::collections::HashMap;

use serde_json::Value;

/// 两段。落到路径上是复数（`groups` / `packages`），跟盘上目录名一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementKind {
    Groups,
    Packages,
}

impl ElementKind {
    /// 路径段 / 回包里的 `kind`。单数也收（引擎认 `group` / `package`），这里只发复数。
    pub fn dir(self) -> &'static str {
        match self {
            Self::Groups => "groups",
            Self::Packages => "packages",
        }
    }

    /// 中文名：界面上的段标题。
    pub fn label(self) -> &'static str {
        match self {
            Self::Groups => "组",
            Self::Packages => "包",
        }
    }

    pub fn from_dir(dir: &str) -> Option<Self> {
        match dir {
            "groups" | "group" => Some(Self::Groups),
            "packages" | "package" => Some(Self::Packages),
            _ => None,
        }
    }
}

/// 一条元素。`state == "problem"` 时 `fields` / `etag` 是 `null`，`detail` 给原文。
///
/// 坏 JSON 只标红它自己 —— 同一目录里别的照列（引擎语义，客户端不许「整页红」）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Element {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub id: String,
    /// 整份 JSON（含未知字段）。读不出来时是 `null`。
    #[serde(default)]
    pub fields: Option<Value>,
    #[serde(default)]
    pub etag: Option<String>,
    /// `ok` / `problem`；缺省按 `ok`（老引擎/未来形状都不炸）。
    #[serde(default)]
    pub state: String,
    /// 坏文件的一句话（解析错误原文 + 路径）。
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Element {
    /// 这一份是不是读不出来（红行）。
    pub fn broken(&self) -> bool {
        self.state == "problem" || self.fields.is_none()
    }

    /// 界面上的名字：`name` → `id` → 空串。**不从 id 编名字**（那是引擎的事）。
    pub fn name(&self) -> &str {
        self.fields
            .as_ref()
            .and_then(|f| f.get("name"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.id)
    }

    /// `desc` 那一行；没有就空。
    pub fn desc(&self) -> Option<&str> {
        self.fields
            .as_ref()
            .and_then(|f| f.get("desc"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    /// 组里引用的包 id（顺序照 JSON，不重排）。
    pub fn packages(&self) -> Vec<&str> {
        self.fields
            .as_ref()
            .and_then(|f| f.get("packages"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }

    /// 包的正文（`body`）；组没有正文。
    pub fn body(&self) -> Option<&str> {
        self.fields
            .as_ref()
            .and_then(|f| f.get("body"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    /// 包的 `kind`（`prompt` / `checklist` / …）。界面只把它当标签画，不分支行为。
    pub fn package_kind(&self) -> Option<&str> {
        self.fields
            .as_ref()
            .and_then(|f| f.get("kind"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    }

    pub fn tags(&self) -> Vec<&str> {
        self.fields
            .as_ref()
            .and_then(|f| f.get("tags"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }

    /// 锁没锁（`locked: true`）。**判定只在引擎**（`/lock` 端点），这边只读 ——
    /// 界面拿它决定“看起来能不能改”，真正的拦截在服务端 423。
    pub fn locked(&self) -> bool {
        self.fields
            .as_ref()
            .and_then(|f| f.get("locked"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

/// 私库那一块：根路径 + 在不在 + git 三态。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementRepo {
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub present: bool,
    /// 没装配时的一句话（指到环境变量），界面逐字显示。
    #[serde(default)]
    pub missing: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub dirty: Option<bool>,
    #[serde(default)]
    pub ahead: Option<i64>,
    #[serde(default)]
    pub behind: Option<i64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// 引用解析：`groups[组 id][包 id] = { state: ok|missing }`，外加一句扁平清单。
///
/// 用 `Value` 收着而不是定型成两层 map：`missing` 是 `["组/包", …]`，而 `groups`
/// 的键是动态的 —— 定型只会让「引擎加一层」时这边编译不过。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementResolved {
    #[serde(default)]
    pub groups: Value,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ElementResolved {
    /// 这个组里哪些包 id 是断链（界面标红用，顺序照引擎给的 `missing`）。
    pub fn missing_in(&self, group: &str) -> Vec<String> {
        let prefix = format!("{group}/");
        self.missing
            .iter()
            .filter_map(|m| m.strip_prefix(&prefix).map(str::to_string))
            .collect()
    }

    /// 某一组里某个包在不在。
    pub fn state_of(&self, group: &str, pkg: &str) -> Option<&str> {
        self.groups
            .get(group)?
            .get(pkg)?
            .get("state")?
            .as_str()
    }
}

/// `GET /v1/elements` 的回包：两段全量 + 引用解析 + 仓状态。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Elements {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub repo: ElementRepo,
    #[serde(default)]
    pub groups: Vec<Element>,
    #[serde(default)]
    pub packages: Vec<Element>,
    #[serde(default)]
    pub resolved: ElementResolved,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Elements {
    pub fn section(&self, kind: ElementKind) -> &[Element] {
        match kind {
            ElementKind::Groups => &self.groups,
            ElementKind::Packages => &self.packages,
        }
    }

    /// 按 id 找一条（选中项跨刷新保住用）。
    pub fn find(&self, kind: ElementKind, id: &str) -> Option<&Element> {
        self.section(kind).iter().find(|e| e.id == id)
    }
}

/// `GET /v1/elements/{kind}` 的回包（只齐一段）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementList {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub items: Vec<Element>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// `GET /v1/elements/{kind}/{id}` 的回包：整份字段 + 内容指纹。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementFile {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub fields: Value,
    #[serde(default)]
    pub etag: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// 写回执（PUT / POST / DELETE / lock 共用一个形状，删除少了 `created` / `etag`）。
///
/// `commit: None` = 内容与盘上一致，没有制造空提交（不是错误）。
/// `replay: true` = 同一个 `requestId` 的重放，`commit` 也是 `None`，一个字节都没写。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementReceipt {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub created: bool,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    /// `pushed` 是字符串族：`ok` / `failed` / `unknown` / `skipped`。
    #[serde(default)]
    pub pushed: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub replay: bool,
    /// 只 `POST …/lock` 会给：操作之后的状态与“这次真的改了没”。
    #[serde(default)]
    pub locked: Option<bool>,
    #[serde(default)]
    pub changed: Option<bool>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ElementReceipt {
    /// 给轻提示的一句话：提交了什么、推没推上去。
    pub fn summary(&self, verb: &str) -> String {
        if self.replay {
            return format!("{verb}：同一个 requestId 已提交过，这次没有重复写");
        }
        let Some(commit) = self.commit.as_deref() else {
            return format!("{verb}：内容与盘上一致，没有产生新提交");
        };
        let short = commit.get(..7).unwrap_or(commit);
        match self.pushed.as_deref() {
            Some("ok") => format!("{verb} · 提交 {short} · 已推送"),
            Some("failed") => format!("{verb} · 提交 {short} · 推送失败（本地已提交）"),
            _ => format!("{verb} · 提交 {short}"),
        }
    }
}

/// `POST /v1/elements/push` 的回包。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementPush {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub pushed: Option<String>,
    #[serde(default)]
    pub repo: ElementRepo,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
