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
use nous_engine::resource::{pending_unit_count, Repo};
use nous_engine::types::{CreateSession, Entries};
use nous_engine::{ElementKind, EngineClient, EngineError, PinClient, PinError, StatsClient};

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
    eprintln!("用法: desk_probe <host> <token> [--negative] [--elements <私库根>]");
    std::process::exit(2);
}

/// 元素库的删除判据：**盘上那份文件必须真的不见了**。
///
/// 锁的是 2026-11-24 真机上那个坑 —— 客户端只拿得到一次快照的 `etag`，盘上被别人改过
/// 就永远 409。协议层的边界只能在这里钉：过期 `base` = 409 且文件**原地不动**；
/// 正确 `base` = 真的删掉 + 一条 `chore(...): rm`。
///
/// 种子在 `scripts/check-desk.sh`（临时 git 私库：`probe-group` / `probe-pack`）。
async fn elements_checks(client: &EngineClient, repo: &str) {
    let group = format!("{repo}/groups/probe-group.json");
    let pack = format!("{repo}/packages/probe-pack.json");

    let list = client.elements().await.expect("元素库可读");
    let Some(item) = list.groups.iter().find(|e| e.id == "probe-group").cloned() else {
        println!("ASSERT FAIL elements.list.seeded | 私库里没有 probe-group（种子没落？）");
        std::process::exit(1);
    };
    assert_line(
        item.etag.is_some(),
        "elements.list.etag",
        item.etag.clone().unwrap_or_default(),
    );

    // 过期 base：409，且文件一个字不许动（否则「防误删别人的新版」就是句空话，
    // 而客户端那边的 409 分支也就永远走不到）。
    let stale = client
        .element_delete(
            ElementKind::Groups,
            "probe-group",
            Some("stale-base-not-on-disk"),
            Some("probe-el-stale"),
        )
        .await;
    let stale_status = match &stale {
        Err(EngineError::Engine { status, .. }) => Some(*status),
        _ => None,
    };
    assert_line(
        stale_status == Some(409),
        "elements.stale-base.409",
        format!("{stale:?}"),
    );
    assert_line(
        std::path::Path::new(&group).exists(),
        "elements.stale-base.kept",
        &group,
    );

    // 正确 base：删掉 + 一条提交（回执里 `commit` 就是提交，没提交就说明没真删）。
    let receipt = client
        .element_delete(
            ElementKind::Groups,
            "probe-group",
            item.etag.as_deref(),
            Some("probe-el-rm"),
        )
        .await
        .expect("带正确 base 的删除要成功");
    assert_line(
        receipt.commit.is_some(),
        "elements.delete.commit",
        format!("{:?}", receipt.commit),
    );
    assert_line(
        !std::path::Path::new(&group).exists(),
        "elements.delete.file-gone",
        &group,
    );

    let after = client.elements().await.expect("元素库可读");
    assert_line(
        after.find(ElementKind::Groups, "probe-group").is_none(),
        "elements.delete.listed-gone",
        format!("剩 {} 组", after.groups.len()),
    );

    // 同一个 requestId 重放：不许产生第二个提交。
    // 内核现在回 404（`remove` 先 `read_file` 再查幂等锚）；哪天改成 `replay:true`
    // 也照样过 —— 这里钉的是「不再写一次」，不是某一句话。
    let replay = client
        .element_delete(
            ElementKind::Groups,
            "probe-group",
            item.etag.as_deref(),
            Some("probe-el-rm"),
        )
        .await;
    let replay_ok = matches!(&replay, Err(EngineError::Engine { status: 404, .. }))
        || matches!(&replay, Ok(r) if r.replay);
    assert_line(
        replay_ok,
        "elements.delete.replay-no-second-write",
        match &replay {
            Ok(r) => format!("replay={} commit={:?}", r.replay, r.commit),
            Err(e) => e.to_string(),
        },
    );
    assert_line(
        !std::path::Path::new(&group).exists(),
        "elements.delete.replay-still-gone",
        &group,
    );

    // 不带 base 的删除（坏元素 `etag:null`、以及别人已经顺手删了的情形）：也要真删掉。
    let receipt = client
        .element_delete(ElementKind::Packages, "probe-pack", None, Some("probe-el-pack"))
        .await
        .expect("不带 base 的删除要成功");
    assert_line(
        receipt.commit.is_some() && !std::path::Path::new(&pack).exists(),
        "elements.delete.nobase",
        &pack,
    );
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| usage());
    let token = args.next().unwrap_or_else(|| usage());
    let mut negative = false;
    // 私库根：给了就把「删除要真从盘上消失」也钉上（要读盘，所以得知道在哪）。
    let mut elements_repo: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--negative" => negative = true,
            "--elements" => elements_repo = args.next(),
            _ => usage(),
        }
    }

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
    // 字段名 2026-11-23 从 `items` 改成了 `presets`（跟 `Presets` 类型上的
    // `alias = "items"` 一个口径）。这里两边都认 —— 只认 `items` 的话，
    // 这条断言从改名那天起就一直是红的（不是端点坏了）。
    let items = roster_json
        .get("presets")
        .or_else(|| roster_json.get("items"))
        .and_then(|i| i.as_array());
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
    let usage_raw = stats.usage_raw("day").await.expect("GET /v1/stats/usage 200");
    let totals_ok = nous_engine::console::usage_totals_present(&usage_raw);
    assert_line(
        totals_ok,
        "stats.usage.200.totals",
        format!(
            "totals key={} totalTokens={}",
            usage_raw.get("totals").is_some(),
            usage_raw.get("totals").and_then(|t| t.get("totalTokens"))
                .and_then(|v| v.as_number()).map(|n| n.to_string()).unwrap_or_else(|| "缺失或非数字".into())
        ),
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

    // ── 扩展页的三份数据 ───────────────────────────────────────────
    //
    // 资源页要恢复 TS 时代那一套（五档 / 类别树 / 归档 / 变体），靠的是这三份读面。
    // 少一份不是「显示不全」而是**静默变空档**：页面看起来正常，只是那一行永远没有。
    // 引擎那边的种子在 `scripts/check-desk.sh`（临时 NOUS_EXTEND：一条原生 + 一条
    // TS 时代 JS 清单），所以这里能钉住 `kind` 的真实取值。
    let exts = good.extensions().await.expect("扩展清单可读");
    assert_line(
        exts.items.iter().any(|item| item.kind == "native"),
        "ext.items.kind.native",
        format!("{} 条", exts.items.len()),
    );
    assert_line(
        exts.items.iter().any(|item| item.kind == "legacy-js"),
        "ext.items.kind.legacy_js",
        exts.items
            .iter()
            .filter(|item| item.kind == "legacy-js")
            .map(|item| item.name.clone())
            .collect::<Vec<_>>()
            .join(","),
    );
    // `core` 只有种子里真有核心扩展时才为真；探针的临时源码根里没有，所以这里只报数，
    // 不做断言（`core` 的判据在内核 `nous-ext` 的单测里，不靠这一条兜底）。
    println!(
        "INFO ext.items.core_count | {}",
        exts.items.iter().filter(|item| item.core).count()
    );

    let cats = good.ext_categories().await.expect("类别词表可读");
    assert_line(true, "ext.categories.200", cats.categories.len());

    let archive = good.ext_archive().await.expect("归档区可读");
    assert_line(true, "ext.archive.200", format!("{} 条", archive.items.len()));

    let variants = good.ext_variants().await.expect("变体绑定表可读");
    assert_line(
        true,
        "ext.variants.200",
        variants.family("extend").count(),
    );

    // ── 底部仓片：#23 的三条判据 ──────────────────────────────────────
    //
    // `scripts/check-desk.sh` 把 `$WORK/extend` 做成一个 git 仓：分支 `feat/probe`
    // （非 main）、没有 remote、脏，且有 2 处内容改动 + 2 个仓级文件（artifacts /
    // deleted-extensions.json）—— 后两个不许算进「处」。
    let st = good.repo_status("extend").await.expect("extend status 可读");
    assert_line(
        st.branch.as_deref() == Some("feat/probe"),
        "repo.status.branch.non_main",
        format!("branch={:?}", st.branch),
    );
    assert_line(
        st.remote.is_none(),
        "repo.status.no_remote",
        format!("remote={:?}", st.remote),
    );
    assert_line(st.dirty, "repo.status.dirty", st.dirty);
    let pen = good.repo_pending("extend").await.expect("extend pending 可读");
    let count = pending_unit_count(Repo::Extensions, &pen.units);
    assert_line(
        count == 2,
        "repo.pending.extend_count",
        format!("{count} 处（raw {} 个单元，含仓级文件）", pen.units.len()),
    );

    // ── 元素库：删除必须真的从私库盘上消失 ─────────────────────────
    match elements_repo.as_deref() {
        Some(repo) => elements_checks(&good, repo).await,
        None => println!(
            "SKIP elements.* | 没给 --elements <私库根>：删除判据要读盘，跳过（不算通过）"
        ),
    }

    println!("DONE");
}
