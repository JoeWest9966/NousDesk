//! `StatsClient` 的两个外部行为缝：实际打出的 `/v1/stats/*` 路径与 `Authorization` 头。
//! 只打本地假引擎，绝不连活引擎。

use std::time::Duration;

use nous_engine::StatsClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct Recorded {
    method: String,
    path: String,
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
        let first = head.lines().next().unwrap_or("");
        let mut parts = first.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        sock.flush().await.unwrap();
        let _ = tx.send(Recorded { method, path, auth });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

fn client(base: &str) -> StatsClient {
    StatsClient::with_token(base, Duration::from_secs(3), Some("t0ken")).unwrap()
}

#[tokio::test]
async fn usage_hits_group_by_query_with_bearer() {
    // 票面一屏一源：账单只打 /v1/stats/usage?group_by=day|model，Desk 不算成本。
    let (base, rx) = fake_engine(200, r#"{"groupBy":"day","totals":{"totalTokens":7},"rows":[]}"#).await;
    let got = client(&base).usage("day").await.unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/v1/stats/usage?group_by=day");
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
    assert_eq!(got.totals.total_tokens, 7);
    assert_eq!(got.group_by, "day");
}

#[tokio::test]
async fn tasks_and_db_hit_their_own_paths() {
    let (base, rx) = fake_engine(200, r#"{"byState":{"running":2},"active":2}"#).await;
    let got = client(&base).tasks().await.unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.path, "/v1/stats/tasks");
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
    assert_eq!(got.by_state.get("running"), Some(&2));

    let (base, rx) = fake_engine(200, r#"{"durable":{"bytes":1024,"rows":{},"quickCheck":"ok"},"plugins":{"bytes":512,"rows":{},"quickCheck":"ok"}}"#).await;
    let got = client(&base).db().await.unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.path, "/v1/stats/db");
    assert_eq!(got.durable.bytes, 1024);
    assert_eq!(got.plugins.quick_check, "ok");
}

#[tokio::test]
async fn errors_keep_status_and_message() {
    let (base, rx) = fake_engine(409, r#"{"error":"stats not configured"}"#).await;
    let err = client(&base).db().await.unwrap_err();
    match err {
        nous_engine::EngineError::Engine { status, message } => {
            assert_eq!(status, 409);
            assert!(message.contains("stats not configured"), "原文要透出：{message}");
        }
        other => panic!("409 应该报 Engine，得到 {other:?}"),
    }
    assert_eq!(rx.await.unwrap().path, "/v1/stats/db");
}
