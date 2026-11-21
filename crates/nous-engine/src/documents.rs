//! `/v1/documents/*` 的哑客户端（Desk#16，甲案）。
//!
//! 引擎侧 12 条端点是同一套泛型 handler：`kind` / `section` / `name` 都是
//! 路径段，客户端**不认业务规则**，只转发。回包一律 `serde_json::Value`
//! 整包收着透传；未知字段不许丢（`AGENTS.md`：不许 `deny_unknown_fields`）。
//!
//! token 注入和 `EngineClient` / `PinClient` 共用 `crate::auth`（`坑.md` 100）：
//! 第四个自建 client 再拼一次 `Bearer` 就是第四次 401 事故。
//! `PUT` 的 `If-Match` 走 header，不混进 body。

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, IF_MATCH};
use serde_json::Value;
use url::Url;

use crate::EngineError;

/// 12 条路径模板（`METHOD`, `v1/...`）。Desk#14 的路由自检拼表用，
/// 和引擎 `nous_documents::routes::DOCUMENT_ROUTES` 逐条对齐。
pub const DOCUMENT_PATHS: &[(&str, &str)] = &[
    ("GET", "v1/documents/{kind}"),
    ("GET", "v1/documents/{kind}/{section}/{name}"),
    ("PUT", "v1/documents/{kind}/{section}/{name}"),
    ("POST", "v1/documents/{kind}/{section}"),
    ("POST", "v1/documents/{kind}/{section}/{name}/rename"),
    ("POST", "v1/documents/{kind}/{section}/{name}/publish"),
    ("POST", "v1/documents/{kind}/{section}/{name}/reload"),
    ("GET", "v1/documents/{kind}/{section}/{name}/validate"),
    ("GET", "v1/documents/{kind}/{section}/{name}/history"),
    ("GET", "v1/documents/{kind}/{section}/{name}/history/diff"),
    ("GET", "v1/documents/{kind}/{section}/{name}/pending"),
    ("GET", "v1/documents/{kind}/{section}/{name}/pending/diff"),
];

pub fn document_paths() -> &'static [(&'static str, &'static str)] {
    DOCUMENT_PATHS
}

/// `PUT` 撞版本时从 `EngineError` 解析出的冲突现场。协议层只做**解析**，
/// 文案和版式归 UI：这里把引擎的 `409` / `error` / 磁盘 `etag` 原样交回。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentConflict {
    pub status: u16,
    /// 引擎 `{error}` 原文，不许翻译 / 改写。
    pub message: String,
    /// 磁盘当前版本：body 的 `etag`（服务端也常放 `ETag` 头，但 `EngineError`
    /// 只收 body —— 写在这里的才是可测试、可显示的真相）。
    pub disk_etag: Option<String>,
}

/// 把 `EngineError::Engine { status: 409, .. }` 拆出 `{error}` 与磁盘 `etag`。
/// 非 409（401 / 500 / 网络错）返回 `None` —— 不许把别的失败说成「别人先改了」。
/// 409 但 body 不是 JSON 也照样返回，message 用原文、`disk_etag` 为 `None`。
pub fn document_conflict(err: &crate::EngineError) -> Option<DocumentConflict> {
    match err {
        crate::EngineError::Engine { status: 409, body, .. } => {
            let value = body
                .as_deref()
                .and_then(|b| serde_json::from_str::<Value>(b).ok());
            let disk_etag = value
                .as_ref()
                .and_then(|v| {
                    v.get("etag")
                        .or_else(|| v.get("ETag"))
                        .or_else(|| v.get("version"))
                })
                .and_then(|e| {
                    e.as_str()
                        .map(str::to_owned)
                        .or_else(|| e.as_u64().map(|n| n.to_string()))
                });
            let message = value
                .as_ref()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .or_else(|| body.clone())
                .unwrap_or_default();
            Some(DocumentConflict { status: 409, message, disk_etag })
        }
        _ => None,
    }
}

/// 把 documents `/history` 回包喂进现有提交历史面板（`Vec<Commit>`）。
/// 面板版式不变，只是数据源多一条；字段不齐时报错，不许画一个假提交。
pub fn documents_history_commits(value: &Value) -> Result<Vec<crate::types::Commit>, EngineError> {
    serde_json::from_value(value.get("commits").cloned().unwrap_or(Value::Null))
        .map_err(|e| EngineError::Engine {
            status: 200,
            message: format!("documents/history 形状不像话：{e}"),
            body: None,
        })
}

/// 把 documents `/history/diff` 或 `/pending/diff` 回包喂进现有 `FileDiff` 面板。
/// 引擎给的是两份**全文 JSON**（不是文本），面板按文本行画 —— 这里格式化成
/// 稳定的 pretty JSON；原本就是字符串的照原样给。
pub fn documents_file_diff(value: &Value) -> Result<crate::types::FileDiff, EngineError> {
    let text = |v: Option<&Value>| -> Option<String> {
        match v {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(other) => Some(serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())),
        }
    };
    let path = value.get("path").and_then(|p| p.as_str())
        .or_else(|| value.get("name").and_then(|n| n.as_str()))
        .unwrap_or("");
    Ok(crate::types::FileDiff {
        path: path.to_string(),
        before: text(value.get("before")),
        after: text(value.get("after")),
        extra: value.as_object()
            .map(|o| o.iter().filter(|(k, _)| !matches!(k.as_str(), "path" | "before" | "after"))
                .map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default(),
    })
}

/// 把 documents `/pending` 回包喂进现有待提交面板（`Pending`）。
pub fn documents_pending(value: &Value) -> Result<crate::types::Pending, EngineError> {
    serde_json::from_value(value.clone()).map_err(|e| EngineError::Engine {
        status: 200,
        message: format!("documents/pending 形状不像话：{e}"),
        body: None,
    })
}

/// 12 条 documents 端点的哑客户端。**必须 `with_token`**：自建 client 一律走
/// `crate::auth` 注入 Bearer（`坑.md` 100），否则配置里有 token 也照样 401。
#[derive(Debug, Clone)]
pub struct DocumentClient {
    http: reqwest::Client,
    base: Url,
}

impl DocumentClient {
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

    fn url(&self, kind: &str, tail: &str) -> Result<Url, EngineError> {
        // 路径段用 URL 编码：名字里有空格 / 中文 / `/` 都不该改路由形状。
        let kind = url::form_urlencoded::byte_serialize(kind.as_bytes()).collect::<String>();
        let tail = tail
            .split('/')
            .map(|seg| url::form_urlencoded::byte_serialize(seg.as_bytes()).collect::<String>())
            .collect::<Vec<_>>()
            .join("/");
        let path = if tail.is_empty() {
            format!("v1/documents/{kind}")
        } else {
            format!("v1/documents/{kind}/{tail}")
        };
        Ok(self.base.join(&path)?)
    }

    async fn json(&self, resp: reqwest::Response) -> Result<Value, EngineError> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .unwrap_or_else(|| text.chars().take(300).collect());
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

    /// `GET /v1/documents/{kind}`（section 由引擎按 kind 决定）。
    pub async fn list(&self, kind: &str) -> Result<Value, EngineError> {
        let resp = self
            .http
            .get(self.url(kind, "")?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET /v1/documents/{kind}/{section}/{name}`。正文 + `etag` 整包原样返回。
    pub async fn get(&self, kind: &str, section: &str, name: &str) -> Result<Value, EngineError> {
        let resp = self
            .http
            .get(self.url(kind, &format!("{section}/{name}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `PUT /v1/documents/{kind}/{section}/{name}`。`if_match` 是读到（或上次写回）
    /// 的 `etag`，通过 `If-Match` header 走；过期时引擎的 409 + body 原样交回调用方。
    pub async fn put(
        &self,
        kind: &str,
        section: &str,
        name: &str,
        fields: &Value,
        if_match: &str,
    ) -> Result<Value, EngineError> {
        let value = if_match
            .parse::<HeaderValue>()
            .map_err(|e| EngineError::Engine {
                status: 400,
                message: format!("If-Match 放不进 HTTP 头：{e}"),
                body: None,
            })?;
        let resp = self
            .http
            .put(self.url(kind, &format!("{section}/{name}"))?)
            .header(IF_MATCH, value)
            .json(fields)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST /v1/documents/{kind}/{section}`（新建文档）。
    pub async fn create(&self, kind: &str, section: &str, fields: &Value) -> Result<Value, EngineError> {
        let resp = self
            .http
            .post(self.url(kind, section)?)
            .json(fields)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `POST …/{name}/rename` → `{to}`。
    pub async fn rename(
        &self,
        kind: &str,
        section: &str,
        name: &str,
        to: &str,
    ) -> Result<Value, EngineError> {
        self.action(kind, section, name, "rename", &serde_json::json!({ "to": to }))
            .await
    }

    /// `POST …/{name}/publish`。
    pub async fn publish(&self, kind: &str, section: &str, name: &str) -> Result<Value, EngineError> {
        self.action(kind, section, name, "publish", &serde_json::json!({}))
            .await
    }

    /// `POST …/{name}/reload`。
    pub async fn reload(&self, kind: &str, section: &str, name: &str) -> Result<Value, EngineError> {
        self.action(kind, section, name, "reload", &serde_json::json!({}))
            .await
    }

    async fn action(
        &self,
        kind: &str,
        section: &str,
        name: &str,
        op: &str,
        body: &Value,
    ) -> Result<Value, EngineError> {
        let resp = self
            .http
            .post(self.url(kind, &format!("{section}/{name}/{op}"))?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    /// `GET …/{name}/validate`。
    pub async fn validate(&self, kind: &str, section: &str, name: &str) -> Result<Value, EngineError> {
        self.read(kind, section, name, "validate", &[]).await
    }

    /// `GET …/{name}/history`（`{commits:[]}`，新到旧）。
    pub async fn history(&self, kind: &str, section: &str, name: &str) -> Result<Value, EngineError> {
        self.read(kind, section, name, "history", &[]).await
    }

    /// `GET …/{name}/history/diff?hash=&path=`：两份**全文**，`null` = 那边没有。
    pub async fn history_diff(
        &self,
        kind: &str,
        section: &str,
        name: &str,
        hash: Option<&str>,
        path: Option<&str>,
    ) -> Result<Value, EngineError> {
        let mut query: Vec<(String, String)> = Vec::new();
        if let Some(hash) = hash {
            query.push(("hash".to_string(), hash.to_string()));
        }
        if let Some(path) = path {
            query.push(("path".to_string(), path.to_string()));
        }
        self.read(kind, section, name, "history/diff", &query).await
    }

    /// `GET …/{name}/pending`（`{dirty, units[]}`）。
    pub async fn pending(&self, kind: &str, section: &str, name: &str) -> Result<Value, EngineError> {
        self.read(kind, section, name, "pending", &[]).await
    }

    /// `GET …/{name}/pending/diff`：base 之后叠的 delta（两份全文）。
    pub async fn pending_diff(
        &self,
        kind: &str,
        section: &str,
        name: &str,
    ) -> Result<Value, EngineError> {
        self.read(kind, section, name, "pending/diff", &[]).await
    }

    async fn read(
        &self,
        kind: &str,
        section: &str,
        name: &str,
        op: &str,
        query: &[(String, String)],
    ) -> Result<Value, EngineError> {
        let resp = self
            .http
            .get(self.url(kind, &format!("{section}/{name}/{op}"))?)
            .query(query)
            .send()
            .await?;
        self.json(resp).await
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    #[test]
    fn 路径表就是引擎那12条() {
        assert_eq!(super::DOCUMENT_PATHS.len(), 12);
    }
}
