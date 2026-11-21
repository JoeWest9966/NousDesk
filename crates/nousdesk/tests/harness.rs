//! Desk#17 集成测试骨架：**状态机 + 请求**，不测 GUI 元素树。
//!
//! GPUI 没有 headless 元素树，断言「设置页那一行长什么样」「两个窗口真的跟着换」
//! 不在本票范围（那是 `验证.md` 里真机人工看的一眼）。这里只在公共缝上验：
//! 1. `EngineClient` 打 `GET /v1/device` + `GET /v1/capabilities` 后能进「已连接」态；
//! 2. `GET /v1/sessions` 的条数能被状态机读出来；
//! 3. 一条 capabilities 说 `false` 的入口是灰的，且理由逐字来自引擎。
//!
//! 假引擎是本机 `TcpListener`，按路径回固定 JSON，原样记录请求头；
//! 每个用例都断言 `Authorization` 头存在（`坑.md` 100：漏 token 就是 401）。
//! 不连真机、不读 `~/.config/nousdesk/token`：token 显式传给 `with_token`。

use std::collections::HashMap;
use std::time::Duration;

use nous_engine::{Capabilities, EngineClient};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    path: String,
    /// `Authorization` 头原样。缺了就是 `None`。
    auth: Option<String>,
}

/// 假引擎：**每个连接一次往返**，按 `(method, path)` 查固定回包表。
/// 表在测试里构造，收请求头用 `mpsc`；绝不连活引擎、不发写请求到真机。
async fn fake_engine(
    responses: HashMap<(&'static str, &'static str), (u16, String)>,
    tx: UnboundedSender<Recorded>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { break };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            let mut header_end = None;
            while let Ok(n) = sock.read(&mut tmp).await {
                if n == 0 { break }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    break;
                }
            }
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
            let (status, body) = responses
                .iter()
                .find(|((m, p), _)| *m == method && *p == path)
                .map(|(_, v)| v.clone())
                .unwrap_or((404, r#"{"error":"no fixture"}"#.to_string()));
            let auth = head.lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case("authorization")
                    .then(|| v.trim().to_string())
            });
            let response = format!(
                "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.flush().await;
            let _ = tx.send(Recorded { method, path, auth });
        }
    });
    format!("http://127.0.0.1:{port}/")
}

fn client(base: &str) -> EngineClient {
    EngineClient::with_token(base, Duration::from_secs(3), Some("t0ken")).unwrap()
}

fn caps_body(sessions_enabled: bool) -> String {
    let (enabled, reason) = if sessions_enabled {
        ("true", "")
    } else {
        (
            "false",
            r#","reason":{"code":"not_implemented","message":"会话模块还没实现（#17 哨兵）"}"#,
        )
    };
    format!(
        r#"{{"contract":1,"server":{{"name":"nous","version":"0.1.3"}},"modules":[{{"name":"sessions","enabled":{enabled},"endpoints":["GET /v1/sessions"]{reason}}}],"runtime":{{"provider":{{"kind":"real","configured":true}}}}}}"#
    )
}

fn fixture() -> HashMap<(&'static str, &'static str), (u16, String)> {
    HashMap::from([
        (
            ("GET", "/v1/device"),
            (200, r#"{"node":"0.1.3","faux":false,"available":[]}"#.to_string()),
        ),
        (("GET", "/v1/capabilities"), (200, caps_body(true))),
        (
            ("GET", "/v1/sessions"),
            (
                200,
                r#"{"items":[{"id":1,"name":"一","busy":false,"thinkingLevels":[],"skills":[],"readOnly":false},{"id":2,"name":"二","busy":false,"thinkingLevels":[],"skills":[],"readOnly":false}]}"#
                    .to_string(),
            ),
        ),
    ])
}

#[tokio::test]
async fn connect_reaches_connected_state_with_bearer_token() {
    let (tx, mut rx) = unbounded_channel();
    let base = fake_engine(fixture(), tx).await;
    let engine = client(&base);

    // 「已连接」= 设备信息 + 能力表都拿到，和 `app.rs::load_engine` 的缝一致。
    let device = engine.device().await.expect("device");
    let caps: Capabilities = engine.capabilities().await.expect("capabilities");
    assert_eq!(device.node.as_deref(), Some("0.1.3"));
    assert_eq!(caps.contract, 1);

    let reqs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(reqs.len(), 2, "登录探针恰好两条：device + capabilities");
    for req in &reqs {
        assert_eq!(
            req.auth.as_deref(),
            Some("Bearer t0ken"),
            "{} {} 漏了 Authorization（坑.md 100）",
            req.method,
            req.path
        );
    }
    assert!(reqs.iter().any(|r| r.path == "/v1/device"));
    assert!(reqs.iter().any(|r| r.path == "/v1/capabilities"));
}

#[tokio::test]
async fn session_list_has_the_expected_row_count() {
    let (tx, mut rx) = unbounded_channel();
    let base = fake_engine(fixture(), tx).await;
    let sessions = client(&base).sessions().await.expect("sessions");
    // 状态机只读条数：GPUI 元素树不在本票范围。
    assert_eq!(sessions.items.len(), 2);
    let reqs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].path, "/v1/sessions");
    assert_eq!(reqs[0].auth.as_deref(), Some("Bearer t0ken"));
}

#[tokio::test]
async fn disabled_module_is_gray_with_the_server_message() {
    let mut f = fixture();
    f.insert(("GET", "/v1/capabilities"), (200, caps_body(false)));
    let (tx, mut rx) = unbounded_channel();
    let base = fake_engine(f, tx).await;
    let caps: Capabilities = client(&base).capabilities().await.expect("capabilities");

    // 这里只断言「引擎说 false」这条真相被原样带回；
    // 灰态判定唯一在 crate::capability::gate/disabled_text（其单测覆盖），
    // 集成测试不复制那套判定，也不碰 GPUI 元素树。
    let module = caps.modules.iter().find(|m| m.name == "sessions").unwrap();
    assert!(!module.enabled);
    let reason = module.reason.as_ref().unwrap();
    assert_eq!(reason.message, "会话模块还没实现（#17 哨兵）");
    assert_eq!(reason.code, nous_engine::ReasonCode::NotImplemented);

    let reqs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].path, "/v1/capabilities");
    assert_eq!(reqs[0].auth.as_deref(), Some("Bearer t0ken"));
}
