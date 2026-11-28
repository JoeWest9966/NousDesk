//! 给已有触发器分配类别（`POST /v1/trigger-projects/category`）。
//!
//! 界面之前只有类别树的新建 / 改名 / 删除，缺「把这条触发器挂到哪几类」的入口；
//! 补入口的同时钉住两件事：
//! - 请求真的是 `POST /v1/trigger-projects/category`，body 是 `{name, category: [...]}`；
//! - 回包按 `TriggerCatalog` 解析（新清单 + 新词表），404 要变成 Err ——
//!   会话不存在当成功，界面会显示「已保存」而盘上什么都没变。

use std::time::Duration;

use nous_engine::TriggerClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Debug)]
struct Recorded {
    method: String,
    path: String,
    body: String,
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
        let mut lines = head.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let len: usize = lines
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse().ok())
                    .flatten()
            })
            .unwrap_or(0);
        let mut body_bytes = buf[header_end..].to_vec();
        while body_bytes.len() < len {
            let n = sock.read(&mut tmp).await.unwrap();
            assert!(n > 0, "body 没收完连接就断了");
            body_bytes.extend_from_slice(&tmp[..n]);
        }
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(response.as_bytes()).await.unwrap();
        sock.flush().await.unwrap();
        let _ = tx.send(Recorded {
            method,
            path,
            body: String::from_utf8_lossy(&body_bytes[..len]).to_string(),
        });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

#[tokio::test]
async fn set_category_posts_name_and_paths() {
    let (base, rx) = fake_engine(
        200,
        r#"{"catalog":[{"name":"dep-alert","version":"0.1.0","summary":"部署异常时收集日志。","category":["工程","工程/发布"]}],"categories":["工程","工程/发布"]}"#,
    )
    .await;
    let got = TriggerClient::with_token(&base, Duration::from_secs(3), Some("t0ken"))
        .unwrap()
        .set_category("dep-alert", &["工程".to_string(), "工程/发布".to_string()])
        .await
        .unwrap();
    assert_eq!(got.catalog.len(), 1);
    assert_eq!(got.catalog[0].category, ["工程", "工程/发布"]);
    assert_eq!(got.categories, ["工程", "工程/发布"]);

    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/trigger-projects/category");
    let sent: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(sent["name"], "dep-alert");
    assert_eq!(sent["category"], serde_json::json!(["工程", "工程/发布"]));
}

#[tokio::test]
async fn empty_category_means_uncategorized() {
    // 「未分类」= 空数组，而不是省略字段；省略了引擎会以为没改。
    let (base, rx) = fake_engine(200, r#"{"catalog":[],"categories":[]}"#).await;
    TriggerClient::with_token(&base, Duration::from_secs(3), Some("t0ken"))
        .unwrap()
        .set_category("dep-alert", &[])
        .await
        .unwrap();
    let sent: serde_json::Value = serde_json::from_str(&rx.await.unwrap().body).unwrap();
    assert_eq!(sent["category"], serde_json::json!([]));
}

#[tokio::test]
async fn missing_project_is_an_error() {
    // 404 不能当成功：不然界面报「已保存」，盘上的 nous.json 一个字没动。
    let (base, rx) = fake_engine(404, r#"{"error":"没有这个触发器项目：gone"}"#).await;
    let err = TriggerClient::with_token(&base, Duration::from_secs(3), Some("t0ken"))
        .unwrap()
        .set_category("gone", &["工程".to_string()])
        .await
        .unwrap_err();
    match err {
        nous_engine::EngineError::Engine { status, message, .. } => {
            assert_eq!(status, 404);
            assert!(message.contains("gone"), "回包原文要透出来：{message}");
        }
        other => panic!("404 应该报 Engine，得到 {other:?}"),
    }
    assert_eq!(rx.await.unwrap().path, "/v1/trigger-projects/category");
}
