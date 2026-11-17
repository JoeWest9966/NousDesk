//! 置顶的哑客户端：真相在 qrd3wh（内核）的 Sessions 文档。
//!
//! ```text
//! GET  /v1/sessions                → items[] 里每条带 pinned（内核已排好：置顶在前）
//! POST /v1/sessions/:id/pin        → {pinned, requestId} → {id, pinned, changed, requestId, version}
//! ```
//!
//! 桌面端不存一份置顶表（`decisions.md`：宁可多一次 HTTP，也不要两份互不相干的置顶表）。
//! `requestId` 是幂等锚：同一条重放十次只落第一次的提交，回包里的 `version` 不涨。
//! `version` 是这条会话置顶日志里的第几次真提交 —— 不是 durable 的 commit_seq。
//! 地址来自宿主的 `__nous/resolve/qrd3wh`，不写进配置、不写进常量。

use std::time::Duration;

use serde::Deserialize;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum PinError {
    #[error("网络：{0}")]
    Http(#[from] reqwest::Error),
    #[error("URL 拼不起来：{0}")]
    Url(#[from] url::ParseError),
    #[error("内核回 {status}：{message}")]
    Engine { status: u16, message: String },
}

/// `GET /v1/sessions` 的整份清单。我们只用 `id` + `pinned`，别的字段原样透传。
#[derive(Debug, Clone, Deserialize)]
pub struct PinSessions {
    pub items: Vec<PinSession>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PinSession {
    pub id: i64,
    /// 缺了当 `false`：内核哪天不发这个字段，也不该整份清单解析失败。
    #[serde(default)]
    pub pinned: bool,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// `POST /v1/sessions/:id/pin` 的回包。`version` 是这条会话置顶日志的第几次真提交：
/// 新决定 +1，重放同锚原样返回（不涨）。durable 的 commit_seq 不对外，它是提交线共用的。
#[derive(Debug, Clone, Deserialize)]
pub struct PinReceipt {
    pub id: i64,
    pub pinned: bool,
    pub changed: bool,
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(default)]
    pub version: u64,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct PinClient {
    http: reqwest::Client,
    /// qrd3wh 的地址，保证以 `/` 结尾。来源只能是 `__nous/resolve`。
    base: Url,
}

impl PinClient {
    pub fn new(base: &str, timeout: Duration) -> Result<Self, PinError> {
        let mut url = Url::parse(base)?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        let http = reqwest::Client::builder().timeout(timeout).build()?;
        Ok(Self { http, base: url })
    }

    fn url(&self, path: &str) -> Result<Url, PinError> {
        Ok(self.base.join(path.trim_start_matches('/'))?)
    }

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, PinError> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(PinError::Engine {
                status: status.as_u16(),
                message: text.chars().take(200).collect(),
            });
        }
        serde_json::from_str(&text).map_err(|e| PinError::Engine {
            status: status.as_u16(),
            message: format!("JSON 不像话：{e}"),
        })
    }

    /// 置顶的会话 id，**数组顺序就是显示顺序**（内核已经按 pins 排好了）。
    pub async fn pinned_ids(&self) -> Result<Vec<i64>, PinError> {
        let resp = self.http.get(self.url("v1/sessions")?).send().await?;
        let parsed: PinSessions = self.json(resp).await?;
        Ok(parsed
            .items
            .into_iter()
            .filter(|s| s.pinned)
            .map(|s| s.id)
            .collect())
    }

    /// 置顶 / 取消置顶。`request_id` 是幂等锚，调用方必须给一个稳定的值。
    pub async fn set_pin(
        &self,
        id: i64,
        pinned: bool,
        request_id: &str,
    ) -> Result<PinReceipt, PinError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            pinned: bool,
            #[serde(rename = "requestId")]
            request_id: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/pin"))?)
            .json(&Body {
                pinned,
                request_id,
            })
            .send()
            .await?;
        self.json(resp).await
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 只挑出置顶的_并保持内核顺序() {
        let raw = r#"{"items":[
            {"id":710,"pinned":true},
            {"id":496,"pinned":true},
            {"id":621,"pinned":false}
        ]}"#;
        let parsed: PinSessions = serde_json::from_str(raw).unwrap();
        let ids: Vec<i64> = parsed
            .items
            .into_iter()
            .filter(|s| s.pinned)
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, vec![710, 496]);
    }

    #[test]
    fn 少字段的条目也能解析() {
        let raw = r#"{"items":[{"id":1,"pinned":true},{"id":2}]}"#;
        let parsed: PinSessions = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.items.len(), 2);
        assert!(!parsed.items[1].pinned);
    }
}
