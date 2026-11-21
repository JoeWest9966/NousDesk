//! documents 端点的哑协议测试（Desk#16，甲案）。
//!
//! 外部行为缝：假 TCP 引擎原样抓 method / path / `Authorization` / `If-Match`，
//! 不连活引擎。判据 1：12 条端点的每条请求都带 `Authorization: Bearer …`
//! （`坑.md` 100：自建 client 漏 token 就静默 401）；判据 3：PUT 的
//! `If-Match` 过期要把 409 和磁盘版本原样交回，不许吞成成功。

use std::time::Duration;

use nous_engine::DocumentClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 假引擎回包表的一行：`(method, path, status, body, 额外响应头)`。
type Reply = (&'static str, &'static str, u16, String, Vec<(String, String)>);

#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    path: String,
    auth: Option<String>,
    if_match: Option<String>,
}

/// 假引擎：每个连接一次往返，按 `METHOD path` 查回包表；记录全部请求。
async fn fake_engine(
    responses: Vec<Reply>,
) -> (String, tokio::sync::mpsc::UnboundedReceiver<Recorded>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { break };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            let header_end = loop {
                let Ok(n) = sock.read(&mut tmp).await else { break None };
                if n == 0 { break None }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(pos + 4);
                }
            };
            let Some(header_end) = header_end else { continue };
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
            let first = head.lines().next().unwrap_or("");
            let mut parts = first.split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let (status, body, extra_headers) = responses
                .iter()
                .find(|(m, p, ..)| (*m == "ANY" || *m == method) && (*p == "ANY" || *p == path))
                .map(|(_, _, s, b, h)| (*s, b.clone(), h.clone()))
                .unwrap_or((404, r#"{"error":"no fixture"}"#.to_string(), vec![]));
            let header = |name: &str| {
                head.lines().find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
                })
            };
            let mut response = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len()
            );
            for (k, v) in extra_headers {
                response.push_str(&format!("{k}: {v}\r\n"));
            }
            response.push_str("\r\n");
            response.push_str(&body);
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.flush().await;
            let _ = tx.send(Recorded {
                method,
                path,
                auth: header("authorization"),
                if_match: header("if-match"),
            });
        }
    });
    (format!("http://127.0.0.1:{port}/"), rx)
}

fn client(base: &str) -> DocumentClient {
    DocumentClient::with_token(base, Duration::from_secs(3), Some("t0ken")).unwrap()
}

fn ok(body: &'static str) -> Vec<Reply> {
    vec![("ANY", "ANY", 200, body.into(), vec![])]
}

#[tokio::test]
async fn list_carries_bearer_token() {
    // 判据 1：配置里有 token 时，documents 也得带上（漏了就是 401）。
    let (base, mut rx) = fake_engine(vec![(
        "GET",
        "/v1/documents/presets",
        200,
        r#"{"kind":"presets","section":"Session-Template","items":[],"resolved":{},"future":1}"#
            .into(),
        vec![],
    )])
    .await;
    let got = client(&base).list("presets").await.unwrap();
    let req = rx.recv().await.unwrap();
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/v1/documents/presets");
    assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"));
    assert_eq!(got["future"], 1, "未知字段原样透传");
}

/// 判据 1 的全体覆盖：12 个公开方法逐一打假引擎，每条请求都必须带 Bearer。
/// 这里不靠 `DOCUMENT_PATHS` 常量自证 —— 每一条都实际发出去。
#[tokio::test]
async fn all_twelve_endpoints_carry_bearer_token() {
    let responses = ok(r#"{"ok":true}"#);
    let (base, mut rx) = fake_engine(responses).await;
    let c = client(&base);

    c.list("presets").await.unwrap();
    c.get("presets", "Session-Template", "a").await.unwrap();
    c.put("presets", "Session-Template", "a", &serde_json::json!({"name":"a"}), "r1")
        .await.unwrap();
    c.create("presets", "Session-Template", &serde_json::json!({"name":"a"}))
        .await.unwrap();
    c.rename("presets", "Session-Template", "a", "b").await.unwrap();
    c.publish("presets", "Session-Template", "a").await.unwrap();
    c.reload("presets", "Session-Template", "a").await.unwrap();
    c.validate("presets", "Session-Template", "a").await.unwrap();
    c.history("presets", "Session-Template", "a").await.unwrap();
    c.history_diff("presets", "Session-Template", "a", Some("r2"), Some("a"))
        .await.unwrap();
    c.pending("presets", "Session-Template", "a").await.unwrap();
    c.pending_diff("presets", "Session-Template", "a").await.unwrap();

    let mut reqs = Vec::new();
    for _ in 0..12 {
        reqs.push(rx.recv().await.unwrap());
    }
    assert_eq!(reqs.len(), 12, "12 条端点一条都不能漏");
    for req in &reqs {
        assert_eq!(req.auth.as_deref(), Some("Bearer t0ken"), "{:?}", req);
    }
    let paths: Vec<_> = reqs.iter().map(|r| (r.method.as_str(), r.path.as_str())).collect();
    assert!(paths.contains(&("PUT", "/v1/documents/presets/Session-Template/a")));
    assert!(paths.contains(&("GET", "/v1/documents/presets/Session-Template/a/history/diff?hash=r2&path=a")));
    assert!(paths.contains(&("POST", "/v1/documents/presets/Session-Template/a/rename")));
}

#[tokio::test]
async fn put_sends_if_match_header() {
    let (base, mut rx) = fake_engine(ok(r#"{"etag":"r2"}"#)).await;
    client(&base)
        .put("presets", "Session-Template", "a", &serde_json::json!({"name":"a"}), "r1")
        .await
        .unwrap();
    let req = rx.recv().await.unwrap();
    assert_eq!(req.method, "PUT");
    assert_eq!(req.if_match.as_deref(), Some("r1"), "If-Match 必须走 header");
}

#[tokio::test]
async fn stale_if_match_returns_409_with_disk_version() {
    // 判据 3：409 不能被吞成成功；磁盘版本从 body 原样带回来给 UI 显示。
    let (base, mut rx) = fake_engine(vec![(
        "ANY",
        "ANY",
        409,
        r#"{"error":"别人先改了","etag":"r7","fields":{"name":"disk"}}"#.into(),
        vec![("ETag".to_string(), "r7".to_string())],
    )])
    .await;
    let err = client(&base)
        .put("presets", "Session-Template", "a", &serde_json::json!({"name":"a"}), "r1")
        .await
        .expect_err("过期 etag 的 PUT 必须报 409");
    let parsed = nous_engine::document_conflict(&err).expect("409 要解析成结构化的冲突");
    assert_eq!(parsed.status, 409);
    assert_eq!(parsed.message, "别人先改了");
    assert_eq!(parsed.disk_etag.as_deref(), Some("r7"));
    assert_eq!(rx.recv().await.unwrap().if_match.as_deref(), Some("r1"));
}

#[test]
fn non_conflict_errors_are_not_reported_as_conflicts() {
    let err = nous_engine::EngineError::Engine { status: 401, message: "unauthorized".into(), body: None };
    assert!(nous_engine::document_conflict(&err).is_none());
}

// ── 现有 diff/preview 面板的数据源适配（不新增版式） ─────────────────

#[test]
fn document_history_feeds_existing_commit_list() {
    let raw = r#"{"kind":"presets","section":"Session-Template","name":"a","commits":[
        {"hash":"r3","short":"r3","at":null,"author":null,"message":"revision 3 (delta)","index":0}
    ]}"#;
    let commits = nous_engine::documents_history_commits(&serde_json::from_str::<serde_json::Value>(raw).unwrap()).unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].hash, "r3");
    assert_eq!(commits[0].message, "revision 3 (delta)");
}

#[test]
fn document_diff_feeds_existing_file_diff_panel() {
    let raw = r#"{"kind":"presets","section":"Session-Template","name":"a","path":"a",
        "before":{"name":"a"},"after":{"name":"b"}}"#;
    let d = nous_engine::documents_file_diff(&serde_json::from_str::<serde_json::Value>(raw).unwrap()).unwrap();
    assert_eq!(d.path, "a");
    assert_eq!(d.before.as_deref(), Some("{\n  \"name\": \"a\"\n}"));
    assert_eq!(d.after.as_deref(), Some("{\n  \"name\": \"b\"\n}"));
}

#[test]
fn document_pending_feeds_existing_pending_panel() {
    let raw = r#"{"kind":"presets","section":"Session-Template","name":"a","head":null,
        "dirty":true,"ahead":0,"behind":0,"remote":null,"units":[
            {"key":"a","kind":"presets","name":"a","tracked":true,"files":[],"added":0,"removed":0}
        ],"aheadCommits":[]}"#;
    let p = nous_engine::documents_pending(&serde_json::from_str::<serde_json::Value>(raw).unwrap()).unwrap();
    assert!(p.dirty);
    assert_eq!(p.unit("a").unwrap().files.len(), 0);
}
