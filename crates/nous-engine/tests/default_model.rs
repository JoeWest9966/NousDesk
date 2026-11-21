//! Desk#17：设置页「默认模型」走 `PUT /v1/default`（Nous#65）。
//!
//! 两个外部行为缝：
//! - `EngineClient::set_default_model` 把 `{provider, modelId}` 写到引擎，回包里的
//!   `default` 是**落盘真相**，调用方必须抄回本地状态（不能把请求值当成功）；
//! - 编目空时 UI 该灰掉并给理由，这里只测客户端能把引擎的「无编目」真相带回来。

use std::time::Duration;

use nous_engine::types::ModelRef;
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
            assert!(n > 0);
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
                k.eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        while buf.len() < header_end + len {
            let n = sock.read(&mut tmp).await.unwrap();
            if n == 0 { break }
            buf.extend_from_slice(&tmp[..n]);
        }
        let body_sent = String::from_utf8_lossy(&buf[header_end..header_end + len]).to_string();
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
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        let _ = tx.send(Recorded { method, path, body: body_sent, auth });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

#[tokio::test]
async fn set_default_model_puts_catalogued_model_and_returns_receipt() {
    let (base, rx) = fake_engine(
        200,
        r#"{"default":{"provider":"p","modelId":"m2"},"wrote":true}"#,
    )
    .await;
    let engine = nous_engine::EngineClient::with_token(
        &base,
        Duration::from_secs(3),
        Some("t0ken"),
    )
    .unwrap();
    let receipt = engine
        .set_default_model(&ModelRef {
            provider: "p".into(),
            model_id: "m2".into(),
            extra: Default::default(),
        })
        .await
        .expect("PUT /v1/default");
    let req = rx.await.unwrap();
    assert_eq!(req.method, "PUT");
    assert_eq!(req.path, "/v1/default");
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
    let sent: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(sent["provider"], "p");
    assert_eq!(sent["modelId"], "m2");
    // 回包里的 default 才是落盘真相：调用方抄它，不抄请求值。
    assert_eq!(receipt.default.key(), "p/m2");
    assert!(receipt.wrote);
}

#[tokio::test]
async fn set_default_model_keeps_engine_error_verbatim() {
    let (base, rx) = fake_engine(400, r#"{"error":"模型不在编目里"}"#).await;
    let engine = nous_engine::EngineClient::with_token(
        &base,
        Duration::from_secs(3),
        Some("t0ken"),
    )
    .unwrap();
    let err = engine
        .set_default_model(&ModelRef {
            provider: "ghost".into(),
            model_id: "ghost/nope".into(),
            extra: Default::default(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(&err, nous_engine::EngineError::Engine { status: 400, message } if message == "模型不在编目里"),
        "引擎原文要透出来：{err:?}"
    );
    let _ = rx.await.unwrap();
}
