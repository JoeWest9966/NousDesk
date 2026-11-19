//! n8qkkj（触发器管理块）的哑客户端。
//!
//! **为什么是另一个 client**：触发器的清单 / 文件 / 发布 / 归档都住在 n8qkkj，
//! 不在 qrd3wh 的 `/v1/*` 里。地址同样只能从宿主的 `__nous/resolve/n8qkkj` 拿。
//!
//! 这一层不加业务字段、不算状态：清单里 `tracked / dirty / error` 都是 n8qkkj 算好的，
//! 客户端只画；发布是**一个同步长请求**（cargo check → build → index → commit → push），
//! 中间进度拿不到，别在 UI 上假装五步并行。

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use url::Url;

use crate::EngineError;

/// 触发器参数声明。`type` 是封闭词表：epoch / number / string / text / boolean / select。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerParam {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl TriggerParam {
    /// 卡片上那一行字：`label` 没有就退回名字。
    pub fn title(&self) -> &str {
        self.label.as_deref().filter(|s| !s.is_empty()).unwrap_or(&self.name)
    }
}

/// 一个触发器 = 一个目录 = 一个独立 Rust 项目。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerItem {
    pub name: String,
    pub version: String,
    pub summary: String,
    #[serde(default)]
    pub category: Vec<String>,
    #[serde(default)]
    pub binary: Option<String>,
    #[serde(default)]
    pub params: Vec<TriggerParam>,
    #[serde(default)]
    pub tracked: bool,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub dirty_files: usize,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 触发器仓的状态（n8qkkj 的 `repo` 块）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerRepo {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub ahead: i64,
    #[serde(default)]
    pub behind: i64,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub dirty_count: usize,
}

/// 归档区的一条（`archived.json` 里的记录）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedTrigger {
    pub name: String,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/triggers`。`triggers` 和 `catalog` 是同一个东西（n8qkkj 为兼容留了两份）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerList {
    pub triggers: Vec<TriggerItem>,
    #[serde(default)]
    pub repo: Option<TriggerRepo>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub archived: Vec<ArchivedTrigger>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 改类别 / 改词表都回这一份：新的清单 + 新的类别树。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TriggerCatalog {
    #[serde(default)]
    pub catalog: Vec<TriggerItem>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerFileMeta {
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub kind: String,
    /// 相对 HEAD 改过没有。老引擎不认这几个字段 → 全 false，界面照旧显示清单。
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub added: i64,
    #[serde(default)]
    pub removed: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerFiles {
    pub files: Vec<TriggerFileMeta>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 一个可编辑文件。`base` = HEAD 里那一版，差异视图跟它比；`etag` = 保存时的并发闸。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerFile {
    pub path: String,
    #[serde(default)]
    pub kind: String,
    pub text: String,
    pub etag: String,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewTrigger {
    pub name: String,
    pub summary: String,
    pub category: Vec<String>,
}

/// 发布的结果。`committed=false` = 没有待提交的改动；`pushed=false` = 本地提交了但推送失败。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerPublish {
    #[serde(default)]
    pub committed: bool,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default)]
    pub push_error: String,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub ahead: i64,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct SaveBody<'a> {
    name: &'a str,
    path: &'a str,
    text: &'a str,
    base: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct NameBody<'a> {
    name: &'a str,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerMutation {
    #[serde(default)]
    pub pushed: bool,
    #[serde(default)]
    pub push_error: String,
    #[serde(default)]
    pub dropped: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 触发器管理块（n8qkkj）。
#[derive(Debug, Clone)]
pub struct TriggerClient {
    http: reqwest::Client,
    base: Url,
}

impl TriggerClient {
    /// token 只从 `NOUS_DESK_TOKEN` / token 文件拿；配置里那颗由 [`Self::with_token`] 传。
    pub fn new(base: &str, timeout: Duration) -> Result<Self, EngineError> {
        Self::with_token(base, timeout, None)
    }

    /// 和 `EngineClient::with_token` / `PinClient::with_token` 同一个注入点：`crate::auth`，
    /// 不在这里另拼 `Bearer`。**这是第三个 client** —— 前两个各漏过一次（`坑.md` 98、100）。
    pub fn with_token(
        base: &str,
        timeout: Duration,
        token: Option<&str>,
    ) -> Result<Self, EngineError> {
        let mut url = Url::parse(base)?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        let headers = crate::auth::headers(token).map_err(|message| EngineError::Engine {
            status: 400,
            message,
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

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, EngineError> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let value = serde_json::from_str::<serde_json::Value>(&text).ok();
            let stage = value
                .as_ref()
                .and_then(|v| v.get("stage").and_then(|s| s.as_str()))
                .map(str::to_owned);
            let mut message = value
                .as_ref()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()))
                .map(str::to_owned)
                .unwrap_or_else(|| text.chars().take(300).collect());
            if let Some(stage) = stage {
                // 发布失败时 n8qkkj 用 `stage` 说是哪一步，前端要能原样显示。
                message = format!("[{stage}] {message}");
            }
            return Err(EngineError::Engine {
                status: status.as_u16(),
                message,
            });
        }
        serde_json::from_str(&text).map_err(|e| EngineError::Engine {
            status: status.as_u16(),
            message: format!("JSON 不像话：{e}"),
        })
    }

    pub async fn list(&self) -> Result<TriggerList, EngineError> {
        let resp = self.http.get(self.url("v1/triggers")?).send().await?;
        self.json(resp).await
    }

    pub async fn files(&self, name: &str) -> Result<TriggerFiles, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!(
                "v1/triggers/files?name={}",
                urlencoding(name)
            ))?)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn file(&self, name: &str, path: &str) -> Result<TriggerFile, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!(
                "v1/triggers/file?name={}&path={}",
                urlencoding(name),
                urlencoding(path)
            ))?)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn save(
        &self,
        name: &str,
        path: &str,
        text: &str,
        base: &str,
    ) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .put(self.url("v1/triggers/file")?)
            .json(&SaveBody { name, path, text, base })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn create(&self, body: &NewTrigger) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/create")?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn publish(&self, name: &str) -> Result<TriggerPublish, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/publish")?)
            .json(&NameBody { name })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn archive(&self, name: &str) -> Result<TriggerMutation, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/archive")?)
            .json(&NameBody { name })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn restore(&self, name: &str) -> Result<TriggerMutation, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/restore")?)
            .json(&NameBody { name })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn purge(&self, name: &str) -> Result<TriggerMutation, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/purge")?)
            .json(&NameBody { name })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 把某个触发器挂到这几类下面（顺手把新类别写进词表）。
    pub async fn set_category(
        &self,
        name: &str,
        category: &[String],
    ) -> Result<TriggerCatalog, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/category")?)
            .json(&serde_json::json!({ "name": name, "category": category }))
            .send()
            .await?;
        self.json(resp).await
    }

    /// 词表增改删：`op` = `add` / `rename` / `remove`。
    ///
    /// `rename` / `remove` 会重写每个触发器 `nous.json` 的 `category`（子类一起搬 / 一起摘），
    /// 但**不提交 git** —— 提交要显式调 [`sync`](Self::sync)。
    pub async fn edit_vocab(
        &self,
        op: &str,
        name: &str,
        to: Option<&str>,
    ) -> Result<TriggerCatalog, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/categories")?)
            .json(&serde_json::json!({ "op": op, "name": name, "to": to }))
            .send()
            .await?;
        self.json(resp).await
    }

    /// 只提交词表 / 清单（不碰 `.rs`）。类别改完由页面调它。
    pub async fn sync(&self) -> Result<TriggerMutation, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/triggers/sync")?)
            .json(&serde_json::json!({}))
            .send()
            .await?;
        self.json(resp).await
    }
}

/// n8qkkj 的 query 参数只收名字 / 路径，自己转一下够用（不引 url 的 form 编码）。
fn urlencoding(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 参数名字转义() {
        assert_eq!(urlencoding("src/main.rs"), "src%2Fmain.rs");
        assert_eq!(urlencoding("nous.json"), "nous.json");
        assert_eq!(urlencoding("a b"), "a%20b");
    }

    #[test]
    fn 清单字段能落下() {
        let raw = serde_json::json!({
            "triggers": [{
                "name": "single-shot", "version": "0.1.0", "summary": "一句话",
                "category": ["时间"], "binary": "single-shot",
                "params": [{ "name": "epoch", "type": "epoch", "label": "什么时候", "required": true }],
                "tracked": true, "dirty": false, "dirtyFiles": 0, "error": ""
            }],
            "repo": { "path": "/opt/Nous-Trigger", "head": "8517477", "branch": "main", "dirty": false },
            "categories": ["时间"], "archived": []
        });
        let list: TriggerList = serde_json::from_value(raw).unwrap();
        assert_eq!(list.triggers[0].name, "single-shot");
        assert_eq!(list.triggers[0].params[0].kind, "epoch");
        assert_eq!(list.repo.unwrap().head.as_deref(), Some("8517477"));
    }

    #[test]
    fn 发布结果能落下() {
        let raw = serde_json::json!({
            "committed": true, "pushed": false, "pushError": "rejected",
            "head": "8f3a1c2", "branch": "main", "ahead": 3
        });
        let out: TriggerPublish = serde_json::from_value(raw).unwrap();
        assert!(out.committed);
        assert!(!out.pushed);
        assert_eq!(out.push_error, "rejected");
    }
}
