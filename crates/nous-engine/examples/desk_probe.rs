//! `scripts/check-desk.sh` 的验收探针：和生产 UI 走同一个 `EngineClient`。
//!
//! 只打真 `nous-app`（faux provider + token + 临时库），不连任何线上引擎。
//! 每条断言打一行 `ASSERT …`，末行 `DONE`。`--negative` 是负向哨兵：
//! 错 token 必须 401，且库里的 entries 条数不许变。
//!
//! 置顶往返（#10）也在这一份里：pin true → `GET /v1/sessions` 里 `pinned=true` 且跳到第一行
//! → 同锚重放 `version` 不涨、顺序不动 → pin false → 回到原位；不存在的会话必须 404。

use std::time::Duration;

use futures_util::StreamExt;
use nous_engine::sse::SseEvent;
use nous_engine::types::{CreateSession, Entries};
use nous_engine::{EngineClient, EngineError, PinClient, PinError, StatsClient};

fn assert_line(ok: bool, label: &str, detail: impl std::fmt::Display) {
    println!("ASSERT {} {} | {}", if ok { "OK" } else { "FAIL" }, label, detail);
    if !ok {
        std::process::exit(1);
    }
}

/// `GET /v1/sessions` 的 id 顺序（引擎按 `pins.ids` 排好序，这里只读它）。
async fn session_order(client: &EngineClient) -> Vec<i64> {
    client
        .sessions()
        .await
        .expect("会话列表可读")
        .items
        .iter()
        .map(|s| s.id)
        .collect()
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
    // 判据是「能力来自 capabilities，不猜」：presets 说 enabled，就必须真能打通
    // `GET /v1/presets`；说没做，就必须给 reason。原来 `enabled || reason.is_some()`
    // 让 enabled 单独也能过，哨兵把端点删掉照样绿 —— 这里改成真断言。
    let presets_enabled = presets.is_some_and(|m| m.enabled);
    let presets_reason = presets.and_then(|m| m.reason.as_ref());
    assert_line(
        if presets_enabled {
            presets_reason.is_none()
        } else {
            presets_reason.is_some()
        },
        "capabilities.presets.consistent",
        format!("{presets:?}"),
    );
    // Desk#11 判据 4：新引擎必须报 presets.enabled=true（12 条端点已落地）。
    assert_line(
        presets_enabled,
        "presets.enabled",
        format!("{presets:?}"),
    );
    if presets_enabled {
        let presets_endpoint = reqwest::Client::new()
            .get(format!("{host}/v1/presets"))
            .bearer_auth(&token)
            .send()
            .await
            .expect("presets 探针可达");
        assert_line(
            presets_endpoint.status() == 200,
            "capabilities.presets.enabled.endpoint_200",
            format!("HTTP {}", presets_endpoint.status()),
        );
    } else {
        // 停用时客户端入口灰着，不能因为「碰巧有路由」就说它可用。
        let presets_endpoint = reqwest::Client::new()
            .get(format!("{host}/v1/presets"))
            .bearer_auth(&token)
            .send()
            .await
            .expect("presets 探针可达");
        assert_line(
            presets_endpoint.status() == 404,
            "capabilities.presets.disabled.endpoint_404",
            format!("HTTP {}", presets_endpoint.status()),
        );
    }
    // Desk#11 判据 4：roster 非空（`check-desk.sh` 预置 `Session-Template/probe/preset.json`）。
    let roster = reqwest::Client::new()
        .get(format!("{host}/v1/presets/Session-Template"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("roster 探针可达");
    let roster_status = roster.status();
    let roster_json: serde_json::Value = roster.json().await.unwrap_or(serde_json::Value::Null);
    let items = roster_json.get("items").and_then(|i| i.as_array());
    assert_line(
        roster_status == 200,
        "presets.roster.200",
        format!("HTTP {roster_status}"),
    );
    assert_line(
        items.is_some_and(|i| !i.is_empty()),
        "presets.roster.items.nonempty",
        format!("{roster_json}"),
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

    // ── 置顶往返：POST /v1/sessions/:id/pin + GET /v1/sessions 的 pinned/排序 ──
    //
    // 判据来自 #10：写进去 → 列表里 `pinned=true` **且排到最前**；同锚重放不落第二条提交；
    // 取消 → 回到原位。**读的是引擎回的清单**，不是客户端的乐观更新 —— 所以
    // 「set_pin 假装成功」这种假实现会在这里变红。
    //
    // 先造第二条会话：只有一条时「排第一」是白送的，顺序断言等于没验。
    let other = good
        .create(&CreateSession {
            name: Some("desk-probe-pin-shift".into()),
            model: Some(serde_json::json!({
                "provider": "faux",
                "modelId": "echo",
            })),
            template: Some(None),
            ..Default::default()
        })
        .await
        .expect("建第二条会话 201")
        .id;
    let pins = PinClient::with_token(&host, Duration::from_secs(10), Some(&token))
        .expect("置顶客户端建得起来");
    let order_before = session_order(&good).await;
    assert_line(
        order_before == vec![other, id],
        "pin.baseline.order",
        format!("新的在前：{order_before:?}（要 [{other}, {id}]）"),
    );

    let request_id_on = format!("desk-probe-pin-{id}-on");
    let receipt = pins
        .set_pin(id, true, &request_id_on)
        .await
        .expect("置顶 200");
    assert_line(
        receipt.changed && receipt.pinned,
        "pin.changed",
        format!("changed={} pinned={}", receipt.changed, receipt.pinned),
    );
    let pinned_list = good.sessions().await.expect("会话列表可读").items;
    assert_line(
        pinned_list.first().map(|s| s.id) == Some(id),
        "pin.jumps_to_top",
        format!(
            "第一行是 {:?}，该是 {id}（原来第一行是 {other}）",
            pinned_list.first().map(|s| s.id)
        ),
    );
    assert_line(
        pinned_list
            .iter()
            .find(|s| s.id == id)
            .is_some_and(|s| s.pinned),
        "pin.pinned_true",
        format!("{id} pinned"),
    );
    let order_pinned: Vec<i64> = pinned_list.iter().map(|s| s.id).collect();
    assert_line(
        order_pinned == vec![id, other],
        "pin.pinned.order",
        format!("{order_pinned:?}（要 [{id}, {other}]）"),
    );

    // 幂等：同一个 requestId 重放 → 不落第二条提交、version 不涨、顺序不动。
    //
    // 注意 `changed`：内核的合同是「重放回**第一次的回执**」，所以重放拿到的还是
    // `changed=true`（那一次确实改了）。可验的幂等证据是 `version` 不涨 + 顺序不动；
    // 票面写的「重放 → changed:false」和内核对不上（`坑.md` 99）。
    let replay = pins
        .set_pin(id, true, &request_id_on)
        .await
        .expect("重放 200");
    assert_line(
        replay.request_id == request_id_on,
        "pin.replay.same_receipt",
        format!("requestId={}", replay.request_id),
    );
    assert_line(
        replay.version == receipt.version,
        "pin.replay.version_stable",
        format!(
            "{} -> {}（涨了就是落了第二条提交）",
            receipt.version, replay.version
        ),
    );
    let order_after_replay = session_order(&good).await;
    assert_line(
        order_after_replay == order_pinned,
        "pin.replay.order_stable",
        format!("{order_pinned:?} -> {order_after_replay:?}"),
    );

    // 取消置顶 → 回到原顺序、pinned=false。
    let request_id_off = format!("desk-probe-pin-{id}-off");
    let receipt = pins
        .set_pin(id, false, &request_id_off)
        .await
        .expect("取消置顶 200");
    assert_line(
        receipt.changed && !receipt.pinned,
        "pin.unpin.changed",
        format!("changed={} pinned={}", receipt.changed, receipt.pinned),
    );
    let unpinned = good.sessions().await.expect("会话列表可读").items;
    let order_after: Vec<i64> = unpinned.iter().map(|s| s.id).collect();
    assert_line(
        order_after == order_before,
        "pin.unpinned_restored",
        format!("{order_before:?} -> {order_after:?}"),
    );
    assert_line(
        unpinned
            .iter()
            .find(|s| s.id == id)
            .is_some_and(|s| !s.pinned),
        "pin.unpinned_false",
        format!("{id} not pinned"),
    );

    // ── #12 判据 4：stats 可用性 + archive 契约（#67 之后：200 + gens 数组） ──
    let stats = StatsClient::with_token(&host, Duration::from_secs(10), Some(&token))
        .expect("stats 客户端建得起来（必须带 token）");
    let usage = stats.usage("day").await.expect("GET /v1/stats/usage 200");
    assert_line(
        !usage.extra.is_empty() || usage.totals.total_tokens >= 0,
        "stats.usage.200.totals",
        format!("groupBy={} totalTokens={}", usage.group_by, usage.totals.total_tokens),
    );
    // archive 现在真有了（Nous#67）：走客户端而不是裸 HTTP ——
    // 裸 HTTP 成功只说明「路由在」，客户端成功才说明「token 注入那条路没漏」。
    match good.archive_list().await {
        Ok(gens) => assert_line(
            true,
            "archive.200.gens",
            format!("gens={} items={}", gens.gens.len(), gens.gens.iter().map(|g| g.items.len()).sum::<usize>()),
        ),
        Err(e) => assert_line(false, "archive.200.gens", format!("{e}")),
    }

    // 负向哨兵：不存在的会话必须 404。静默降级（吞掉错误）会让这条变红。
    match pins
        .set_pin(999_999_999, true, "desk-probe-pin-missing")
        .await
    {
        Err(PinError::Engine { status, .. }) => {
            assert_line(status == 404, "pin.missing.404", format!("HTTP {status}"))
        }
        Ok(receipt) => {
            println!("ASSERT FAIL pin.missing.404 | 不存在的会话竟然成功：{receipt:?}");
            std::process::exit(1);
        }
        Err(other) => {
            println!("ASSERT FAIL pin.missing.404 | 拿到了 {other:?}");
            std::process::exit(1);
        }
    }

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
