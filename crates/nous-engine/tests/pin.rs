//! 置顶进内核的协议测试（票 #15）。
//! 两个外部行为缝：
//! - `GET /v1/sessions` 的真实形状 → pinned_ids（顺序就是引擎排好的）；
//! - `PinClient::set_pin` 的请求构造 / 回包解析：POST /v1/sessions/:id/pin，
//!   body 带 `pinned` + `requestId`，回包带 `version`。
//!
//! 假服务只在本机监听一次，不连活引擎、不发写请求到真机。

use std::time::Duration;

use nous_engine::{PinClient, PinError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct Recorded {
    method: String,
    path: String,
    body: String,
    /// `Authorization` 头原样。缺了就是 `None` —— 「配置里有 token，置顶却 401」就是它。
    auth: Option<String>,
}

async fn fake_engine(
    status: u16,
    response_body: impl Into<String>,
) -> (String, oneshot::Receiver<Recorded>) {
    let response_body = response_body.into();
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
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        sock.flush().await.unwrap();
        let _ = tx.send(Recorded {
            method,
            path,
            body,
            auth,
        });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

fn client(base: &str) -> PinClient {
    PinClient::new(base, Duration::from_secs(3)).unwrap()
}

#[tokio::test]
async fn pin_posts_to_kernel_with_request_id() {
    let (base, rx) = fake_engine(
        200,
        r#"{"id":14,"pinned":true,"changed":true,"requestId":"desk-pin-14","version":3}"#,
    )
    .await;
    let receipt = client(&base)
        .set_pin(14, true, "desk-pin-14")
        .await
        .unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/sessions/14/pin");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["pinned"], true);
    assert_eq!(body["requestId"], "desk-pin-14");
    assert_eq!(receipt.version, 3);
    assert!(receipt.changed);
}

#[tokio::test]
async fn pinned_ids_reads_kernel_session_order() {
    // 引擎已经排好序：置顶在前（新置顶最前），其余按 durable 原顺序。
    let (base, rx) = fake_engine(
        200,
        r#"{"items":[
            {"id":710,"pinned":true},
            {"id":496,"pinned":true},
            {"id":621,"pinned":false}
        ]}"#,
    )
    .await;
    let ids = client(&base).pinned_ids().await.unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/v1/sessions");
    assert_eq!(ids, vec![710, 496]);
}

#[test]
fn pin_receipt_keeps_unknown_fields() {
    let raw = r#"{"id":14,"pinned":true,"changed":false,"requestId":"a","version":7,"future":1}"#;
    let r: nous_engine::pins::PinReceipt = serde_json::from_str(raw).unwrap();
    assert_eq!(r.version, 7);
    assert_eq!(r.extra["future"], 1);
}

#[tokio::test]
async fn set_pin_carries_bearer_token() {
    // 判据：配置里有 token（desktop.toml / 环境 / token 文件）时，置顶也得带上它。
    // 不带 → 引擎回 401 → `load_pins` 静默降级 → 界面上置顶又「等于没有」。
    let (base, rx) = fake_engine(
        200,
        r#"{"id":14,"pinned":true,"changed":true,"requestId":"a","version":1}"#,
    )
    .await;
    PinClient::with_token(&base, Duration::from_secs(3), Some("t0ken"))
        .unwrap()
        .set_pin(14, true, "a")
        .await
        .unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
}

#[tokio::test]
async fn pinned_ids_carries_bearer_token() {
    let (base, rx) = fake_engine(200, r#"{"items":[]}"#).await;
    PinClient::with_token(&base, Duration::from_secs(3), Some("t0ken"))
        .unwrap()
        .pinned_ids()
        .await
        .unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
}

#[tokio::test]
async fn set_pin_surfaces_404_instead_of_swallowing_it() {
    // 判据：不存在的会话必须把 404 原样带出来，不许静默降级。
    let (base, _rx) = fake_engine(404, r#"{"error":"session 42 not found"}"#).await;
    let err = client(&base)
        .set_pin(42, true, "desk-pin-42")
        .await
        .expect_err("404 不该被吞掉");
    match err {
        PinError::Engine { status, message } => {
            assert_eq!(status, 404);
            assert!(message.contains("not found"), "{message}");
        }
        other => panic!("应该是 Engine 错误，拿到 {other:?}"),
    }
}
