//! `scripts/check-desk.sh` 的验收探针：和生产 UI 走同一个 `EngineClient`。
//!
//! 只打真 `nous-app`（faux provider + token + 临时库），不连任何线上引擎。
//! 每条断言打一行 `ASSERT …`，末行 `DONE`。`--negative` 是负向哨兵：
//! 错 token 必须 401，且库里的 entries 条数不许变。

use std::time::Duration;

use futures_util::StreamExt;
use nous_engine::sse::SseEvent;
use nous_engine::types::{CreateSession, Entries};
use nous_engine::{EngineClient, EngineError};

fn assert_line(ok: bool, label: &str, detail: impl std::fmt::Display) {
    println!("ASSERT {} {} | {}", if ok { "OK" } else { "FAIL" }, label, detail);
    if !ok {
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("用法: desk_probe <host> <token> [--negative]");
    std::process::exit(2);
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| usage());
    let token = args.next().unwrap_or_else(|| usage());
    let negative = matches!(args.next().as_deref(), Some("--negative"));

    let good = EngineClient::with_token(&host, Duration::from_secs(10), Some(&token))
        .expect("客户端建得起来");

    if negative {
        // 负向哨兵：先记下好 token 看到的会话与 entries 数量，错 token 一律 401，
        // 再看好 token 读到的数量没变 —— 错 token 不许产生任何副作用。
        let before = good.sessions().await.expect("好 token 可读").items.len();
        let bad = EngineClient::with_token(&host, Duration::from_secs(10), Some("wrong-token"))
            .expect("客户端建得起来");
        let status = match bad.capabilities().await {
            Err(EngineError::Engine { status, .. }) => status,
            other => {
                println!("ASSERT FAIL wrong-token-401 | 拿到了 {other:?}");
                std::process::exit(1);
            }
        };
        assert_line(status == 401, "wrong-token-401", format!("HTTP {status}"));
        // 连 SSE 也走同一个 token 注入（sse builder），错 token 必须被 401 挡住。
        let sse_status = match bad.events(1).await {
            Err(EngineError::Engine { status, .. }) => status,
            Ok(_) => {
                println!("ASSERT FAIL wrong-token-sse-401 | 错 token 竟然连上了 SSE");
                std::process::exit(1);
            }
            Err(other) => {
                println!("ASSERT FAIL wrong-token-sse-401 | 拿到了 {other:?}");
                std::process::exit(1);
            }
        };
        assert_line(sse_status == 401, "wrong-token-sse-401", sse_status);
        let after_list = good.sessions().await.expect("好 token 仍可读");
        let after = after_list.items.len();
        assert_line(
            after == before,
            "sessions-unchanged",
            format!("{before} -> {after}"),
        );
        // 票面要的是「会话条数 + entries 条数都不变」两样。只比会话条数会漏掉
        // 「某个写路径被错 token 触发了」这种副作用：会话数没变，但多了几条 entry。
        if let Some(id) = after_list.items.first().map(|s| s.id) {
            let guards_before = good.entries(id).await.expect("好 token 可读 entries").items.len();
            // 用错 token 再打一次 entries（401），然后再读一遍数。
            let _ = bad.entries(id).await;
            let guards_after = good.entries(id).await.expect("好 token 仍可读 entries").items.len();
            assert_line(
                guards_after == guards_before,
                "entries-unchanged",
                format!("会话 {id}: {guards_before} -> {guards_after}"),
            );
        } else {
            // 没有会话就没有 entries 可数 —— 明说跳过，不假装验过。
            println!("SKIP entries-unchanged | 引擎上一条会话都没有，没有 entries 可数");
        }
        println!("DONE");
        return;
    }

    // ── capabilities：功能面的唯一真相 ───────────────────────────────
    let caps = good.capabilities().await.expect("capabilities 可读");
    let sessions = caps.modules.iter().find(|m| m.name == "sessions");
    let presets = caps.modules.iter().find(|m| m.name == "presets");
    assert_line(
        sessions.is_some_and(|m| m.enabled),
        "capabilities.sessions.enabled",
        format!("{sessions:?}"),
    );
    assert_line(
        presets.is_some_and(|m| {
            !m.enabled
                && m.reason.as_ref().is_some_and(|r| r.code == nous_engine::ReasonCode::NotImplemented)
        }),
        "capabilities.presets.not_implemented",
        format!("{presets:?}"),
    );
    assert_line(
        caps.runtime.provider.kind == "faux",
        "capabilities.provider.faux",
        &caps.runtime.provider.kind,
    );

    // ── 建会话 → 列表里至少一条 ────────────────────────────────────
    let before = good.sessions().await.expect("会话列表可读").items.len();
    let made = good
        .create(&CreateSession {
            name: Some("desk-probe".into()),
            model: Some(serde_json::json!({
                "provider": "faux",
                "modelId": "echo",
            })),
            template: Some(None),
            ..Default::default()
        })
        .await
        .expect("建会话 201");
    let id = made.id;
    let list = good.sessions().await.expect("会话列表可读").items;
    assert_line(
        list.len() > before,
        "sessions-created",
        format!("{before} -> {}", list.len()),
    );

    // ── SSE：先订阅、等 snapshot，再发消息 ──────────────────────────
    let mut stream = good.events(id).await.expect("SSE 200").boxed_local();
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("SSE 首帧及时")
        .expect("SSE 有首帧")
        .expect("首帧可解析");
    assert_line(
        matches!(first, SseEvent::Snapshot { .. }),
        "sse.snapshot",
        format!("{first:?}").chars().take(80).collect::<String>(),
    );

    let submission = good
        .send_message(id, "ping", "desk-probe-1")
        .await
        .expect("发消息 202");
    assert_line(
        submission.status == "accepted" || !submission.status.is_empty(),
        "message-202",
        &submission.status,
    );

    let (mut starts, mut appended, mut ends) = (0usize, 0usize, 0usize);
    let mut done_reason = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline && (starts == 0 || appended == 0 || ends == 0) {
        let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("SSE 帧及时");
        let Some(frame) = frame else { break };
        let frame = frame.expect("SSE 帧可解析");
        match frame {
            SseEvent::MessageStart { .. } => starts += 1,
            SseEvent::EntryAppended { entry } => {
                appended += 1;
                println!(
                    "FRAME entry_appended reason={:?}",
                    entry.as_ref().and_then(|e| e.extra.get("reason"))
                );
            }
            SseEvent::MessageEnd { entry } => {
                ends += 1;
                done_reason = entry
                    .as_ref()
                    .and_then(|e| e.extra.get("reason"))
                    .and_then(|v| v.as_str())
                    == Some("done");
                println!(
                    "FRAME message_end reason={:?}",
                    entry.as_ref().and_then(|e| e.extra.get("reason"))
                );
            }
            _ => {}
        }
    }
    assert_line(starts > 0, "sse.message_start", starts);
    assert_line(appended > 0, "sse.entry_appended", appended);
    assert_line(ends > 0, "sse.message_end", ends);

    // ── 历史里必须有一条 reason=done 的 assistant entry ─────────────
    let mut found = false;
    let mut last = Vec::new();
    for _ in 0..40 {
        let entries: Entries = good.entries(id).await.expect("历史可读");
        last = entries
            .items
            .iter()
            .filter(|e| e.kind == "pi.assistant")
            .map(|e| format!("{:?}", e.extra))
            .collect();
        // 服务端 `render_entries_page` 当前不投影 `reason` 字段；终态 reason
        // 由 `message_end` 那一帧给出（同一条 assistant entry）。这里两边都验：
        // 历史里确实落了 assistant entry，且同一轮的终态 reason == done。
        if done_reason
            && entries.items.iter().any(|e| {
                e.kind == "pi.assistant" && e.text.as_deref().is_some_and(|t| !t.is_empty())
            })
        {
            found = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_line(found, "entries.assistant.done", format!("{id} {last:?}"));
    println!("DONE");
}
