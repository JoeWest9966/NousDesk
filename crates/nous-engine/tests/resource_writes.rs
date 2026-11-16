//! 资源仓写接口的协议测试：路径 + 方法 + body 形状 + 错误透传。
//!
//! 端点来自 qrd3wh 源码（2026-11-16 读 `skills-api.js` / `prompts-api.js` /
//! `server.js` 的头注释与 dispatch），不是猜的。测试只打本地假服务，
//! **绝不连活引擎发写请求**。

use std::time::Duration;

use nous_engine::{EngineClient, EngineError};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

struct Recorded {
    method: String,
    path: String,
    body: String,
}

async fn fake_engine(status: u16, response_body: impl Into<String>) -> (String, oneshot::Receiver<Recorded>) {
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
                if k.eq_ignore_ascii_case("content-length") {
                    v.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        while buf.len() < header_end + len {
            let n = sock.read(&mut tmp).await.unwrap();
            assert!(n > 0);
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

fn client(base: &str) -> EngineClient {
    EngineClient::new(base, Duration::from_secs(3)).unwrap()
}

// ── create：三张表各走各的路径，body 原样递过去 ──────────────────────────

#[tokio::test]
async fn skill_create_posts_name_description_category() {
    let (base, rx) = fake_engine(200, r#"{"skill":{"name":"weekly-digest"}}"#).await;
    let body = json!({"name":"weekly-digest","description":"整理周报","category":["写作"]});
    let v = client(&base).skill_create(&body).await.unwrap();
    assert_eq!(v["skill"]["name"], "weekly-digest");
    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/skills/create");
    assert_eq!(serde_json::from_str::<Value>(&req.body).unwrap(), body);
}

#[tokio::test]
async fn prompt_create_posts_name_summary_category_body() {
    let (base, rx) = fake_engine(200, r#"{"prompt":{"name":"strict-review"}}"#).await;
    let body = json!({"name":"strict-review","summary":"按两条轴审查","category":["工程"],"body":"# strict-review\n"});
    let v = client(&base).prompt_create(&body).await.unwrap();
    assert_eq!(v["prompt"]["name"], "strict-review");
    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/prompts/create");
    assert_eq!(serde_json::from_str::<Value>(&req.body).unwrap(), body);
}

#[tokio::test]
async fn extension_create_posts_name_summary_category() {
    let (base, rx) = fake_engine(200, r#"{"extension":{"name":"my-tools"}}"#).await;
    let body = json!({"name":"my-tools","summary":"几个顺手的工具","category":[]});
    let v = client(&base).extension_create(&body).await.unwrap();
    assert_eq!(v["extension"]["name"], "my-tools");
    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/extensions/create");
    assert_eq!(serde_json::from_str::<Value>(&req.body).unwrap(), body);
}

// ── file PUT：同一形状，三个仓不同前缀 ──────────────────────────────────

#[tokio::test]
async fn file_put_uses_put_and_right_prefix_for_each_kind() {
    for (kind, path) in [
        ("skills", "/v1/skills/file"),
        ("prompts", "/v1/prompts/file"),
        ("extensions", "/v1/extensions/file"),
    ] {
        let (base, rx) = fake_engine(200, r#"{"ok":true}"#).await;
        let body = json!({"name":"demo","path":"SKILL.md","text":"# demo\n"});
        client(&base).resource_file_put(kind, &body).await.unwrap();
        let req = rx.await.unwrap();
        assert_eq!(req.method, "PUT", "{kind} 写文件得是 PUT");
        assert_eq!(req.path, path, "{kind} 路径不对");
        assert_eq!(serde_json::from_str::<Value>(&req.body).unwrap(), body);
    }
}

// ── publish：message 省掉时不发这个键 ───────────────────────────────────

#[tokio::test]
async fn publish_posts_to_each_repo_with_optional_message() {
    for (kind, path) in [
        ("skills", "/v1/skills/publish"),
        ("prompts", "/v1/prompts/publish"),
        ("extensions", "/v1/extensions/publish"),
    ] {
        let (base, rx) = fake_engine(200, r#"{"ok":true}"#).await;
        client(&base).resource_publish(kind, None).await.unwrap();
        let req = rx.await.unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, path);
        assert_eq!(serde_json::from_str::<Value>(&req.body).unwrap(), json!({}));

        let (base, rx) = fake_engine(200, r#"{"ok":true}"#).await;
        client(&base).resource_publish(kind, Some("我的提交")).await.unwrap();
        let req = rx.await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&req.body).unwrap(),
            json!({"message":"我的提交"})
        );
    }
}

// ── files / file GET：带 query，不丢参数 ────────────────────────────────

#[tokio::test]
async fn resource_files_gets_with_name_query() {
    for (kind, path) in [
        ("skills", "/v1/skills/files"),
        ("prompts", "/v1/prompts/files"),
        ("extensions", "/v1/extensions/files"),
    ] {
        let (base, rx) = fake_engine(200, r#"{"files":[]}"#).await;
        client(&base).resource_files(kind, "my demo").await.unwrap();
        let req = rx.await.unwrap();
        assert_eq!(req.method, "GET");
        assert!(req.path == format!("{path}?name=my%20demo")
            || req.path == format!("{path}?name=my+demo"), "query: {}", req.path);
    }
}

#[tokio::test]
async fn resource_file_gets_text_with_name_and_path_query() {
    for (kind, path) in [
        ("skills", "/v1/skills/file"),
        ("prompts", "/v1/prompts/file"),
        ("extensions", "/v1/extensions/file"),
    ] {
        let (base, rx) = fake_engine(200, r#"{"text":"demo"}"#).await;
        client(&base)
            .resource_file_get(kind, "demo", "SKILL.md")
            .await
            .unwrap();
        let req = rx.await.unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, format!("{path}?name=demo&path=SKILL.md"));
    }
}

// ── 错误：`{error}` 原样带出来，不包不美化 ──────────────────────────────

#[tokio::test]
async fn write_error_passes_through_verbatim() {
    const ERROR: &str = "weekly-digest: description 不能空 —— 模型靠它判断该不该用这条技能";
    let (base, _rx) = fake_engine(400, json!({"error": ERROR}).to_string()).await;
    let err = client(&base)
        .skill_create(&json!({"name":"weekly-digest"}))
        .await
        .err()
        .unwrap();
    match err {
        EngineError::Engine { status, message } => {
            assert_eq!(status, 400);
            assert_eq!(message, ERROR);
        }
        other => panic!("该是引擎错误，得到 {other:?}"),
    }
}

// ── 未知 kind 不该被静默拼进 URL ────────────────────────────────────────

#[tokio::test]
async fn unknown_kind_is_rejected_before_http() {
    let (base, _rx) = fake_engine(200, "{}").await;
    let err = client(&base)
        .resource_publish("unknown", None)
        .await
        .err()
        .unwrap();
    assert!(matches!(err, EngineError::Engine { status: 400, .. }));
}
