//! 归档协议测试。两个外部行为缝：
//! - 引擎 JSON → `nous_engine::types::*` 的解析结果；
//! - 引擎 HTTP 回包 → `EngineClient` 四个归档方法的返回值 / 错误。
//!
//! sample 来自 2026-11-12 实测 `GET /p/new1/qrd3wh/v1/archive`（只读），
//! HTTP 测试只打本地假服务，绝不连活引擎发写请求。

use std::time::Duration;

use nous_engine::types::ArchiveGens;
use nous_engine::{EngineClient, EngineError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// 活样本：`GET /v1/archive`，一条 gen、一条未撕原件也没墓碑的条目。
const LIVE_ARCHIVE: &str = r#"{"gens":[{"gen":"20261110","items":[{"id":302,"parent":{"id":296,"at":50},"entries":0,"last":null,"name":"对话 302","archived":null,"live":true}]}]}"#;

#[test]
fn parse_archive_list_live_sample() {
    let g: ArchiveGens = serde_json::from_str(LIVE_ARCHIVE).unwrap();
    assert_eq!(g.gens.len(), 1);
    assert_eq!(g.gens[0].r#gen, "20261110");
    assert_eq!(g.gens[0].items.len(), 1);
    let item = &g.gens[0].items[0];
    assert_eq!(item.id, 302);
    assert_eq!(item.name, "对话 302");
    assert!(item.live);
    assert!(item.archived.is_none());
    assert!(item.last.is_none());
    assert_eq!(item.parent.as_ref().unwrap()["id"], 296);
    assert_eq!(item.entries.as_ref().unwrap(), &serde_json::json!(0));
}

/// 墓碑对象 + 未知字段：`purged` / `at: null` 要留下，没型号的键进 extra。
#[test]
fn parse_archive_tombstone_and_unknown_fields() {
    let json = r#"{"gens":[{
      "gen": "20261110", "newGenKey": {"a": 1},
      "items": [
        {"id": 331, "name": "已撕原件", "archived": {
            "gen": "20261110", "purged": true, "at": null, "by": "mcfpqm"
         }, "live": false, "newItemKey": 7},
        {"id": 337, "name": "已捞回", "archived": {
            "gen": "20261110", "purged": false, "at": 1794222429712
         }, "live": true}
      ]
    }]}"#;

    let g: ArchiveGens = serde_json::from_str(json).unwrap();
    assert_eq!(g.extra.len(), 0);
    let group = &g.gens[0];
    assert_eq!(group.extra["newGenKey"]["a"], 1);
    assert_eq!(group.items.len(), 2);

    let purged = group.items[0].archived.as_ref().unwrap();
    assert_eq!(purged.r#gen, "20261110");
    assert!(purged.purged);
    assert_eq!(purged.extra["by"], "mcfpqm");
    assert!(!group.items[0].live);
    assert_eq!(group.items[0].extra["newItemKey"], 7);
    // at 为 null 与 at 有值是两种状态，别塌成一个布尔。
    assert!(purged.at.is_none());
    assert_eq!(
        group.items[1]
            .archived
            .as_ref()
            .unwrap()
            .at
            .as_ref()
            .unwrap(),
        &serde_json::json!(1794222429712_i64)
    );
    assert!(!group.items[1].archived.as_ref().unwrap().purged);
}

/// 缺字段 / 空库 / 最小条目：每个新结构都要有兜底，引擎少给不许解析炸。
#[test]
fn parse_archive_missing_fields() {
    let empty: ArchiveGens = serde_json::from_str("{}").unwrap();
    assert!(empty.gens.is_empty());

    let json = r#"{"gens":[
      {},
      {"gen": "20261111", "items": [{}]},
      {"gen": "20261112", "items": [{"id": 999, "archived": null}]}
    ], "newTopKey": true}"#;
    let g: ArchiveGens = serde_json::from_str(json).unwrap();
    assert_eq!(g.extra["newTopKey"], true);
    assert_eq!(g.gens.len(), 3);
    assert!(g.gens[0].r#gen.is_empty());
    assert!(g.gens[0].items.is_empty());

    let bare = &g.gens[1].items[0];
    assert_eq!(bare.id, 0);
    assert!(bare.name.is_empty());
    assert!(bare.parent.is_none());
    assert!(bare.entries.is_none());
    assert!(bare.last.is_none());
    assert!(!bare.live);

    assert_eq!(g.gens[2].r#gen, "20261112");
    assert_eq!(g.gens[2].items[0].id, 999);
    assert!(g.gens[2].items[0].archived.is_none());
}

/// `GET /v1/sessions?archived=1` 的活样本（截到一条）：节点带 `archived` 墓碑。
/// null / 对象 / 整个字段缺席都要能过。
#[test]
fn parse_session_archived_tombstone() {
    let base = r#"{"id": 302, "name": "对话 302", "readOnly": false,
        "busy": null, "thinkingLevels": [], "skills": [], "last": null, %ARCHIVED%}"#;
    let parse = |archived: &str| {
        let json = base.replace("%ARCHIVED%", archived);
        let mut s: nous_engine::types::Sessions =
            serde_json::from_str(&format!("{{\"items\":[{json}]}}")).unwrap();
        s.items.remove(0)
    };

    let live = parse(r#""archived": null"#);
    assert_eq!(live.id, 302);
    assert!(live.archived.is_none());

    let purged = parse(r#""archived": {"gen":"20261110","purged":true,"at":null}"#);
    let t = purged.archived.as_ref().unwrap();
    assert_eq!(t.r#gen, "20261110");
    assert!(t.purged);
    assert!(t.at.is_none());

    let missing = parse(r#""extraKey": 1"#);
    assert!(missing.archived.is_none());
    assert_eq!(missing.extra["extraKey"], 1);
}

// ── 缝 B：HTTP 回包 → `EngineClient` 公开方法 ───────────────────────────
// 假引擎只服务一次请求，记录 method / path / body；不连活引擎、不发写请求。

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

fn client(base: &str) -> EngineClient {
    EngineClient::new(base, Duration::from_secs(3)).unwrap()
}

/// `archive(302)` → `POST v1/sessions/302/archive`，回包原样作为 `Value` 交给调用方。
#[tokio::test]
async fn archive_posts_to_session_archive_path() {
    let (base, rx) = fake_engine(200, r#"{"ok":true,"id":302}"#).await;
    let v = client(&base).archive(302).await.unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["id"], 302);

    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/sessions/302/archive");
    assert!(req.body.is_empty());
}

/// `purge(302)` → `POST v1/sessions/302/purge`（真删，危险动作的确认在 UI 层）。
#[tokio::test]
async fn purge_posts_to_session_purge_path() {
    let (base, rx) = fake_engine(200, r#"{"ok":true}"#).await;
    let v = client(&base).purge(302).await.unwrap();
    assert_eq!(v["ok"], true);

    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/sessions/302/purge");
    assert!(req.body.is_empty());
}

/// `archive_list()` → `GET v1/archive`，回包解析成 `ArchiveGens`（不是原始 Value）。
#[tokio::test]
async fn archive_list_gets_and_parses_gens() {
    let (base, rx) = fake_engine(200, LIVE_ARCHIVE).await;
    let g = client(&base).archive_list().await.unwrap();
    assert_eq!(g.gens.len(), 1);
    assert_eq!(g.gens[0].r#gen, "20261110");
    assert_eq!(g.gens[0].items[0].id, 302);
    assert!(g.gens[0].items[0].live);

    let req = rx.await.unwrap();
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/v1/archive");
}

/// `restore(302, "20261110")` → `POST v1/archive/302/restore`，body `{"gen":"20261110"}`。
#[tokio::test]
async fn restore_posts_gen_body_to_archive_path() {
    let (base, rx) = fake_engine(200, r#"{"ok":true,"id":302}"#).await;
    let v = client(&base).restore(302, "20261110").await.unwrap();
    assert_eq!(v["ok"], true);

    let req = rx.await.unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/archive/302/restore");
    let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(body, serde_json::json!({"gen": "20261110"}));
}

/// 409 的 `{error}` 原文（含活分支列表）不许被客户端改写、删减、包一层。
/// 四个归档端点都走同一条 `self.json(resp)`，逐个钉住。
#[tokio::test]
async fn engine_error_message_is_passed_through_verbatim() {
    // 模拟引擎的 409 文案：带活分支、中文、换行。客户端一个字都不许动。
    const ERROR: &str = "会话 302 忙：有活分支 331、337\n请先把活分支停掉";
    let body = serde_json::json!({ "error": ERROR }).to_string();

    for endpoint in ["archive", "purge", "list", "restore"] {
        let (base, rx) = fake_engine(409, body.clone()).await;
        let c = client(&base);
        let err = match endpoint {
            "archive" => c.archive(302).await.err(),
            "purge" => c.purge(302).await.err(),
            "list" => c.archive_list().await.err(),
            "restore" => c.restore(302, "20261110").await.err(),
            _ => unreachable!(),
        }
        .unwrap_or_else(|| panic!("{endpoint} 该报错"));
        match err {
            EngineError::Engine { status, message } => {
                assert_eq!(status, 409);
                assert_eq!(message, ERROR, "{endpoint} 改写了引擎原文");
            }
            other => panic!("{endpoint} 错误类型不对：{other:?}"),
        }
        let req = rx.await.unwrap();
        assert!(!req.path.is_empty());
    }
}
