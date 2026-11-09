//! `EngineClient`：qrd3wh 的哑 HTTP 客户端。
//!
//! 这一层**不加业务字段、不算状态**。引擎回什么就递什么，错误原样带出来。

use std::time::Duration;

use futures_util::Stream;
use serde::Serialize;
use url::Url;

use crate::sse::{SseError, SseEvent};
use crate::types::{
    CreateSession, CreatedSession, Device, Entries, Forked, Presets, ReadReceipt, Sessions,
    Submission, Unread,
};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("网络：{0}")]
    Http(#[from] reqwest::Error),
    #[error("URL 拼不起来：{0}")]
    Url(#[from] url::ParseError),
    #[error("引擎 {status}：{message}")]
    Engine { status: u16, message: String },
}

#[derive(Debug, Clone)]
pub struct EngineClient {
    http: reqwest::Client,
    /// SSE 专用。**不能挂总超时**：挂上之后长流会在超时那一刻整根断掉，
    /// 症状是回答到一半「HTTP 流断了」（reqwest 报 `error decoding response body`），
    /// 然后重连、重新等 snapshot。只限连接阶段。
    stream: reqwest::Client,
    /// 引擎地址，保证以 `/` 结尾。来源只能是宿主的 `__nous/resolve`，别自己拼。
    base: Url,
}

impl EngineClient {
    pub fn new(base: &str, timeout: Duration) -> Result<Self, EngineError> {
        let mut url = Url::parse(base)?;
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        let http = reqwest::Client::builder().timeout(timeout).build()?;
        let stream = reqwest::Client::builder().connect_timeout(timeout).build()?;
        Ok(Self { http, stream, base: url })
    }

    fn url(&self, path: &str) -> Result<Url, EngineError> {
        Ok(self.base.join(path.trim_start_matches('/'))?)
    }

    /// 引擎那句 `{error}` 原样带出来，不包、不美化。
    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, EngineError> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let message = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .unwrap_or_else(|| text.chars().take(200).collect());
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

    pub async fn device(&self) -> Result<Device, EngineError> {
        let resp = self.http.get(self.url("v1/device")?).send().await?;
        self.json(resp).await
    }

    /// 会话树。`parent != null` 的是 fork，显示不显示由 UI 决定，这里全给。
    pub async fn sessions(&self) -> Result<Sessions, EngineError> {
        let resp = self.http.get(self.url("v1/sessions")?).send().await?;
        self.json(resp).await
    }

    /// 历史。**新的在前**，调用方自己翻正。
    pub async fn entries(&self, id: i64) -> Result<Entries, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/sessions/{id}/entries"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 每条会话的未读数，平铺的 map。跟 [`Self::sessions`] 一起拉，别一会拉一个。
    pub async fn unread(&self) -> Result<Unread, EngineError> {
        let resp = self.http.get(self.url("v1/unread")?).send().await?;
        self.json(resp).await
    }

    /// 把这条会话标成读到最新。水位只进不退，引擎那边钉住了。
    ///
    /// 别自己推水位、也别只把「已读」写本地了事 —— 重装客户端不该丢未读。
    pub async fn mark_read(&self, id: i64) -> Result<ReadReceipt, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/read/{id}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    /// 从这条会话分叉一条新的。`entry_id` 为 `None` = 从最后一条分叉。
    pub async fn fork(
        &self,
        id: i64,
        entry_id: Option<i64>,
        name: Option<&str>,
    ) -> Result<Forked, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            entry_id: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            name: Option<&'a str>,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/fork"))?)
            .json(&Body { entry_id, name })
            .send()
            .await?;
        self.json(resp).await
    }

    /// 发一条。`request_id` 是幂等锚，桌面端**必须传**（重发不会答两遍）。
    pub async fn send_message(
        &self,
        id: i64,
        content: &str,
        request_id: &str,
    ) -> Result<Submission, EngineError> {
        #[derive(Serialize)]
        struct Body<'a> {
            content: &'a str,
            request_id: &'a str,
        }
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/messages"))?)
            .json(&Body { content, request_id })
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn abort(&self, id: i64) -> Result<serde_json::Value, EngineError> {
        let resp = self
            .http
            .post(self.url(&format!("v1/sessions/{id}/abort"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn create(&self, body: &CreateSession) -> Result<CreatedSession, EngineError> {
        let resp = self
            .http
            .post(self.url("v1/sessions")?)
            .json(body)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn presets(&self, section: &str) -> Result<Presets, EngineError> {
        let resp = self
            .http
            .get(self.url(&format!("v1/presets/{section}"))?)
            .send()
            .await?;
        self.json(resp).await
    }

    pub async fn selection(&self) -> Result<serde_json::Value, EngineError> {
        let resp = self.http.get(self.url("v1/selection")?).send().await?;
        self.json(resp).await
    }

    pub async fn set_selection(&self, id: Option<i64>) -> Result<serde_json::Value, EngineError> {
        #[derive(Serialize)]
        struct Body {
            id: Option<i64>,
        }
        let resp = self
            .http
            .post(self.url("v1/selection")?)
            .json(&Body { id })
            .send()
            .await?;
        self.json(resp).await
    }

    /// SSE。拿到的是解析好的事件流，合帧是 UI 层的事。
    pub async fn events(
        &self,
        id: i64,
    ) -> Result<impl Stream<Item = Result<SseEvent, SseError>>, EngineError> {
        let resp = self
            .stream
            .get(self.url(&format!("v1/sessions/{id}/events"))?)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await?;
            return Err(EngineError::Engine {
                status,
                message: text.chars().take(200).collect(),
            });
        }
        Ok(crate::sse::parse(resp.bytes_stream()))
    }
}
