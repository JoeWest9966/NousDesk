//! 类别**词表**的写端点：`POST /v1/presets/:section/categories`。
//!
//! 桌面端的「新建类别」以前是死的 —— 客户端里写着「服务端没有独立的类别写端点」，
//! 点了只会弹一句错误。引擎其实一直有这条（`op: add / rename / remove`），只是没接。
//! 这条测试钉住三件事：
//! - 打的是 `/categories`（复数，词表），不是 `/category`（单数，给一条预设挂类别）；
//! - 体里是 `{op, name, to}`，`add` 时 `to` 是 `null`；
//! - 回包的 `categories` 原样交给 UI（它拿这份直接覆盖本地词表，不自己拼）。
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
        let _ = tx.send(Recorded { method, path, body });
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

fn client(base: &str) -> EngineClient {
    EngineClient::new(base, Duration::from_secs(3)).unwrap()
}

/// 新建：`POST /categories`，体 `{op:"add", name, to:null}`，回包的词表原样交回。
#[tokio::test]
async fn add_posts_to_the_vocabulary_endpoint() {
    let (base, rx) = fake_engine(
        r#"{"section":"Session-Template","categories":["Basics","写作","写作/周报","工程"],"presets":[],"touched":4}"#,
    )
    .await;
    let r = client(&base)
        .preset_categories_op("Session-Template", "add", "写作/周报", None)
        .await
        .unwrap();
    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/presets/Session-Template/categories");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["op"], "add");
    assert_eq!(body["name"], "写作/周报", "路径是整条（父级 + 名字），不是最后一段");
    assert!(body["to"].is_null(), "add 不该带 to");
    assert!(r.categories.iter().any(|c| c == "写作/周报"), "词表原样交回 UI");
}

/// 改名要带 `to`：少了它就是引擎侧的 400，客户端不该自己补默认值。
#[tokio::test]
async fn rename_carries_the_target_name() {
    let (base, rx) = fake_engine(r#"{"section":"Session-Template","categories":["Basics","工程"],"presets":[],"touched":2}"#).await;
    let _ = client(&base)
        .preset_categories_op("Session-Template", "rename", "写作", Some("写作与文档"))
        .await
        .unwrap();
    let req = rx.await.unwrap();
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body["op"], "rename");
    assert_eq!(body["name"], "写作");
    assert_eq!(body["to"], "写作与文档");
}
