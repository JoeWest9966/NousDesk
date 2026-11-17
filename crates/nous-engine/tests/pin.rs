//! 置顶进内核的协议测试（票 #15）。
//! 两个外部行为缝：
//! - `GET /v1/sessions` 的真实形状 → pinned_ids（顺序就是引擎排好的）；
//! - `PinClient::set_pin` 的请求构造 / 回包解析：POST /v1/sessions/:id/pin，
//!   body 带 `pinned` + `requestId`，回包带 `version`。
//!
//! 假服务只在本机监听一次，不连活引擎、不发写请求到真机。

use std::time::Duration;

use nous_engine::PinClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct Recorded {
    method: String,
    path: String,
    body: String,
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
        let _ = tx.send(Recorded { method, path, body });
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
