//! Desk#14：`GET /v1/_routes` 的哑客户端。fail-open：拿不到就不显示漂移。
//!
//! 外部行为缝：客户端带 Bearer 抓一次，回包 routes 数组能解析；
//! 老引擎没这条端点时返回 Err，调用方不许阻塞连接。

use std::time::Duration;

use nous_engine::EngineClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn routes_probe_sends_bearer_and_parses_routes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = r#"{"routes":[{"module":"sessions","method":"GET","path":"/v1/sessions"}],"modules":[]}"#;
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        let head_end = loop {
            let n = sock.read(&mut tmp).await.unwrap();
            assert!(n > 0);
            buf.extend_from_slice(&tmp[..n]);
            if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break p + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        assert!(head.starts_with("GET /v1/_routes "), "探针路径：{head}");
        assert!(head.to_lowercase().contains("authorization: bearer tok-14"));
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
    });

    let client = EngineClient::with_token(
        &format!("http://127.0.0.1:{port}/"),
        Duration::from_secs(2),
        Some("tok-14"),
    )
    .unwrap();
    let snapshot = client.routes().await.unwrap();
    assert_eq!(snapshot.routes.len(), 1);
    assert_eq!(snapshot.routes[0].path, "/v1/sessions");
}

#[tokio::test]
async fn routes_probe_on_404_is_an_error_not_a_panic() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut tmp = [0u8; 512];
        let _ = sock.read(&mut tmp).await.unwrap();
        sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });
    let client = EngineClient::with_token(
        &format!("http://127.0.0.1:{port}/"),
        Duration::from_secs(2),
        None,
    )
    .unwrap();
    assert!(client.routes().await.is_err());
}
