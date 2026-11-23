//! 编辑器校验的协议缝：`POST /v1/presets/:section/validate`。
//!
//! 以前编辑器拿 `PUT ?dry_run=1` 顶，那个回包只有 `{valid, wrote}` —— 「生效值」那页
//! 永远显示「引擎没回 resolved」。这条测试钉住三件事：
//! - 打的是 `/validate`，体里是**草稿**（`spec`），不是 `name`（那会读到盘上旧值）；
//! - `ok` / `resolved` 原样交回 UI，不在客户端算第二遍；
//! - 老内核的 `valid` 也认（换内核那一下不炸）。
//!
//! 假服务只在本机监听一次，不连活引擎。

use std::time::Duration;

use nous_engine::EngineClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct Recorded {
    method: String,
    path: String,
    body: String,
    auth: Option<String>,
}

async fn fake_engine(response_body: &'static str) -> (String, oneshot::Receiver<Recorded>) {
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
        let len = head
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                if k.eq_ignore_ascii_case("content-length") {
                    v.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        while buf.len() < header_end + len {
            let n = sock.read(&mut tmp).await.unwrap();
            assert!(n > 0, "body 没收完连接就断了");
            buf.extend_from_slice(&tmp[..n]);
        }
        let body = String::from_utf8_lossy(&buf[header_end..header_end + len]).to_string();
        let auth = head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("authorization")
                .then(|| v.trim().to_string())
        });
        let first = head.lines().next().unwrap_or("");
        let mut parts = first.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        sock.flush().await.unwrap();
        let _ = tx.send(Recorded { method, path, body, auth });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

fn client(base: &str) -> EngineClient {
    EngineClient::new(base, Duration::from_secs(3)).unwrap()
}

/// 草稿走 `POST`，路径是 `/validate`，体里带 `spec`（不是 `name`）。
#[tokio::test]
async fn validate_posts_the_draft_spec() {
    let (base, rx) = fake_engine(
        r#"{"ok":true,"errors":[],"warnings":[],"spec":{"name":"deep-dive"},"resolved":{"model":{"state":"ok","value":"volc-ark/x"}}}"#,
    )
    .await;
    let draft = serde_json::json!({"name": "deep-dive", "v": 1, "model": {"provider": "volc-ark"}});
    let v = client(&base)
        .preset_validate("Session-Template", &draft)
        .await
        .unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/presets/Session-Template/validate");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["spec"], draft, "校验的是草稿原文");
    assert!(body.get("name").is_none(), "带 name 会读盘上旧值");
    assert!(v.ok);
    assert_eq!(v.resolved.unwrap()["model"]["state"], "ok");
}

/// 形状闸失败时 `resolved` 是 null：别把它当 `Some(Null)` 画一堆空行。
#[tokio::test]
async fn null_resolved_means_absent() {
    let (base, rx) = fake_engine(
        r#"{"ok":false,"errors":["v 得是 1"],"warnings":[],"spec":null,"resolved":null}"#,
    )
    .await;
    let v = client(&base)
        .preset_validate("Session-Template", &serde_json::json!({}))
        .await
        .unwrap();
    let _ = rx.await.unwrap();
    assert!(!v.ok);
    assert_eq!(v.errors.len(), 1);
    assert!(v.resolved.is_none());
}

/// 老内核只有 `valid`：同一个意思，照样读得出来。
#[tokio::test]
async fn old_kernel_valid_key_still_parses() {
    let (base, rx) = fake_engine(r#"{"valid":true,"errors":[],"warnings":[]}"#).await;
    let v = client(&base)
        .preset_validate("Ultimate-Template", &serde_json::json!({"v": 1}))
        .await
        .unwrap();
    let _ = rx.await.unwrap();
    assert!(v.ok);
    assert!(v.resolved.is_none());
}
