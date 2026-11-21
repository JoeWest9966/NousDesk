//! 触发器客户端带不带 token（`坑.md` 100）。
//!
//! 现象：配了 token 的机器上点「触发器」，界面写 `拉触发器失败：内核回 401`，
//! 看起来像「触发器没实现」，其实是 `TriggerClient` 是第二个自建 reqwest client，
//! token 挂在 `EngineClient` 那颗上，它一个都没带。
//!
//! 两个外部行为缝：
//! - 带 token 时 `Authorization: Bearer …` 必须**真的出现在线上**（假引擎原样抓头）；
//! - 401 要变成 `Err(status=401)`，不许被吞成空清单。
//!
//! 这里**不测**「没 token 时不发头」：`new()` 走的是 `auth::resolve`，会从
//! `NOUS_DESK_TOKEN` / token 文件兜底 —— 开发机上那个文件永远存在，测出来是环境噪声，
//! 不是代码行为。

use std::time::Duration;

use nous_engine::TriggerClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct Recorded {
    path: String,
    /// `Authorization` 头原样。缺了就是 `None`。
    auth: Option<String>,
}

async fn fake_engine(status: u16, body: impl Into<String>) -> (String, oneshot::Receiver<Recorded>) {
    let body = body.into();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        let header_end = loop {
            let n = sock.read(&mut tmp).await.unwrap();
            assert!(n > 0, "header 没收完连接就断了");
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let auth = head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("authorization")
                .then(|| v.trim().to_string())
        });
        let path = head
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        sock.flush().await.unwrap();
        let _ = tx.send(Recorded { path, auth });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

#[tokio::test]
async fn list_carries_bearer_token() {
    // 判据：配置里（desktop.toml / 环境 / token 文件）有 token，触发器也得带上。
    let (base, rx) = fake_engine(200, r#"{"triggers":[]}"#).await;
    TriggerClient::with_token(&base, Duration::from_secs(3), Some("t0ken"))
        .unwrap()
        .list()
        .await
        .unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.path, "/v1/triggers");
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
}

#[tokio::test]
async fn unauthorized_is_an_error_not_an_empty_list() {
    // 401 必须是 Err：以前它静默降级成「没有触发器」，界面看着像功能没做。
    let (base, rx) = fake_engine(401, r#"{"error":"unauthorized"}"#).await;
    let err = TriggerClient::with_token(&base, Duration::from_secs(3), Some("bad"))
        .unwrap()
        .list()
        .await
        .unwrap_err();
    match err {
        nous_engine::EngineError::Engine { status, message, .. } => {
            assert_eq!(status, 401);
            assert!(message.contains("unauthorized"), "回包原文要透出来：{message}");
        }
        other => panic!("401 应该报 Engine，得到 {other:?}"),
    }
    assert_eq!(rx.await.unwrap().path, "/v1/triggers");
}
