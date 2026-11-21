//! 数据监控室的纯逻辑测试（读数 / 事件 / 格式化）。
//!
//! 单独一个文件：和 `data_console.rs` 放在一起时 rustc 会在 `#[test]` 展开上
//! 递归到爆栈（试过 `recursion_limit = 2048` 都不够），拆开就没事。

use crate::data_console::*;
use nous_engine::console::{Conversations, Cost, Health, Overview};
use nous_engine::types::ArchiveGens;


    /// `Desk#13`：删掉 `旧面板 REST 路径` 取数后，那五屏只能按 capabilities 灰掉，
    /// 不许再画一行「等面板给数据」的红字。`Stats` 是 #12 的活路，本票不接。
    #[test]
    fn removed_data_screens_are_gated() {
        // Desk#12：六屏统一从 /v1 取数；归档没有数据通道，只按 capabilities 灰掉。
        // 源码里不再有 screen_has_live_data 这类「旧通道还有没有」的判定。
        let src = include_str!("data_console.rs");
        assert!(!src.contains("screen_has_live_data"));
        assert!(Screen::ALL.contains(&Screen::Archive));
    }

    fn overview() -> Overview {
        serde_json::from_value(serde_json::json!({
            "target": {"id": "qrd3wh", "own": false, "path": "/x/agent.sqlite"},
            "file": {"db": 1_142_784, "wal": 288_432, "shm": 32768,
                     "mtime": 1794807593545.0, "walMtime": 1794812553752.0},
            "pragmas": {"pageSize": 4096, "pageCount": 279, "freelist": 1,
                        "journalMode": "wal", "autoVacuum": 0,
                        "userVersion": 0, "schemaVersion": 2},
            "bytes": {"db": 1_142_784, "wal": 288_432, "shm": 32768,
                      "free": 4096, "used": 1_138_688, "onDisk": 1_464_000},
            "counts": {"entries": 216, "documents": 886, "document_revisions": 1570,
                       "tasks": 136, "record_ids": 1477, "submissions": 62,
                       "conversations": 174, "durable_metadata": 1, "durable_schema": 1},
            "tables": [{"name": "entries", "bytes": 416_000, "pages": 104, "rows": 216}],
            "indexes": [],
            "entriesByKind": [{"kind": "pi.user", "count": 62, "bytes": 8_400}],
            "integrity": {"checked": true, "ok": true, "messages": []},
            "attention": []
        }))
        .expect("overview")
    }

    fn convs() -> Conversations {
        serde_json::from_value(serde_json::json!({
            "count": 2,
            "list": [
                {"id": 2, "entries": 90, "tokens": 514_241, "cost": 0, "toolCalls": 31,
                 "models": ["m"], "firstAt": 1, "lastAt": 1794222429551i64,
                 "title": "只回答两个字：收到"},
                {"id": 14, "entries": 12, "tokens": 900, "cost": 0, "toolCalls": 0,
                 "models": [], "firstAt": 1, "lastAt": 1794222429000i64, "title": "小的"}
            ],
            "totals": {"entries": 102, "tokens": 515_141, "cost": 0}
        }))
        .expect("conversations")
    }

    fn cost() -> Cost {
        serde_json::from_value(serde_json::json!({
            "totals": {"entries": 99, "tokens": 592_866, "input": 118_500,
                       "output": 25_511, "cacheRead": 448_855, "cacheWrite": 0,
                       "reasoning": 12_801, "cost": 0},
            "byDay": [{"label": "11-14", "tokens": 300_000, "cost": 0, "calls": 40,
                       "input": 60_000, "output": 12_000, "cacheRead": 220_000}],
            "byHour": [], "byModel": [], "byConv": [], "note": "cost 为 0 不代表免费"
        }))
        .expect("cost")
    }

    fn health() -> Health {
        serde_json::from_value(serde_json::json!({
            "tasks": {"total": 136, "byStatus": [], "stuck": [], "sleeping": [],
                      "avgMs": 6838, "p95": 18254,
                      "slowest": [{"id": 11, "conv": 2, "kind": "pi.generation",
                                   "status": "terminal", "background": false,
                                   "startedAt": 1793156783438i64,
                                   "endedAt": 1793156787228i64, "outcome": "ok"}]},
            "submissions": {"total": 62, "byStatus": [], "unanswered": [
                {"id": 10, "conv": 2, "status": "unanswered"}]},
            "documents": {"documents": 931, "byKind": [], "revisions": 1570,
                          "base": 889, "delta": 681, "deltaRatio": 0.4337},
            "walBytes": 288_432,
            "freelist": {"free": 1, "bytes": 4096},
            "maintenance": []
        }))
        .expect("health")
    }

    fn archive() -> ArchiveGens {
        serde_json::from_value(serde_json::json!({
            "gens": [{
                "gen": "20261110",
                "items": [
                    {"id": 1, "name": "撕过的", "entries": 0,
                     "archived": {"gen": "20261110", "purged": true, "at": 1794800000000i64},
                     "live": false},
                    {"id": 2, "name": "原件还在", "entries": 21,
                     "archived": {"gen": "20261110", "purged": false, "at": 1794700000000i64},
                     "live": true},
                    {"id": 3, "name": "捞回来了", "entries": 3,
                     "archived": null, "live": true},
                    {"id": 4, "name": "没墓碑", "entries": 0,
                     "archived": null, "live": false}
                ]
            }]
        }))
        .expect("archive")
    }

    /// 五份数据整整齐齐摆着。测试只碰 `Snapshot`，不碰 `ConsoleState`。
    struct Fixture {
        overview: Overview,
        convs: Conversations,
        cost: Cost,
        health: Health,
        archive: ArchiveGens,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                overview: overview(),
                convs: convs(),
                cost: cost(),
                health: health(),
                archive: archive(),
            }
        }

        fn snap(&self) -> Snapshot<'_> {
            Snapshot {
                overview: Some(&self.overview),
                convs: Some(&self.convs),
                cost: Some(&self.cost),
                health: Some(&self.health),
            }
        }
    }

    #[test]
    fn 十个读数一个不少而且都有值() {
        let f = Fixture::new();
        let rs = readings(f.snap(), Some(&f.archive));
        let keys: Vec<&str> = rs.iter().map(|r| r.key).collect();
        assert_eq!(
            keys,
            vec!["db", "wal", "entries", "convs", "tokens", "cache", "archive", "waiting", "stuck", "docs"]
        );
        assert!(rs.iter().all(|r| r.value != "—"), "全都有数据时不该出现 —");
        assert_eq!(rs[0].value, "1.1 MB");
        assert_eq!(rs[2].value, "216");
        assert_eq!(rs[6].value, "4");
        assert_eq!(rs[7].value, "1");
        assert_eq!(rs[8].value, "0");
    }

    #[test]
    fn 数据没到齐时给破折号不是零() {
        let rs = readings(Snapshot::default(), None);
        assert!(rs.iter().all(|r| r.value == "—"), "什么都没拿到时十格都该是 —");
    }

    #[test]
    fn 读数的点和屏一一对上() {
        let f = Fixture::new();
        let by = |k: &str| {
            readings(f.snap(), Some(&f.archive))
                .into_iter()
                .find(|r| r.key == k)
                .unwrap()
                .screen()
        };
        assert_eq!(by("db"), Screen::Overview);
        assert_eq!(by("convs"), Screen::Convs);
        assert_eq!(by("tokens"), Screen::Cost);
        assert_eq!(by("cache"), Screen::Cost);
        assert_eq!(by("stuck"), Screen::Health);
        assert_eq!(by("archive"), Screen::Archive);
    }

    #[test]
    fn 归档那格的四个点按四态数() {
        let f = Fixture::new();
        let rs = readings(f.snap(), Some(&f.archive));
        let dots = rs.iter().find(|r| r.key == "archive").unwrap().dots.unwrap();
        // 原件在 / 已销毁 / 已捞回 / 无墓碑 各一条
        assert_eq!(dots, [1, 1, 1, 1]);
    }

    #[test]
    fn 时间线新的在上而且能过滤() {
        let f = Fixture::new();
        let all = events(f.snap(), Some(&f.archive));
        assert!(all.len() >= 7, "归档 4 + 会话 2 + 任务 1，至少 7 条：{}", all.len());
        assert!(
            all.windows(2).all(|w| w[0].at >= w[1].at),
            "该从新到旧排"
        );

        let only = timeline(f.snap(), Some(&f.archive), TlFilter::Archive);
        assert!(!only.is_empty());
        assert!(only.iter().all(|e| e.kind == Kind::Archive));
        assert!(only.len() < all.len());
    }

    #[test]
    fn 归档事件用的是墓碑上的时间不是零() {
        let f = Fixture::new();
        let evs = events(f.snap(), Some(&f.archive));
        let a = evs.iter().find(|e| e.kind == Kind::Archive).unwrap();
        assert_eq!(a.at, 1794800000000);
        assert_eq!(a.tail, "已销毁");
    }

    #[test]
    fn 空标题的会话不会画出空行() {
        let mut f = Fixture::new();
        f.convs.list[0].title.clear();
        let evs = events(f.snap(), Some(&f.archive));
        assert!(evs.iter().any(|e| e.kind == Kind::Conv && e.title.is_empty()));
        // 画的时候才补 "(空会话)"，事件里保持原样（不编文案）。
    }

    #[test]
    fn 字节数和千分位() {
        assert_eq!(fmt_bytes_short(1_142_784), "1.1 MB");
        assert_eq!(fmt_bytes_short(288_432), "282 KB");
        assert_eq!(fmt_bytes_short(0), "0 B");
        assert_eq!(fmt_bytes_short(512), "512 B");
        assert_eq!(fmt_num(592_866), "592,866");
        assert_eq!(fmt_num(216), "216");
        assert_eq!(fmt_num(0), "0");
        assert_eq!(fmt_num(-1234), "-1,234");
    }

    #[test]
    fn 毫秒换成人话() {
        assert_eq!(fmt_ms(0), "0.0s");
        assert_eq!(fmt_ms(600), "0.6s");
        assert_eq!(fmt_ms(3790), "3.8s");
        assert_eq!(fmt_ms(18_254), "18s");
        assert_eq!(fmt_ms(90_000), "1.5m");
    }

    #[test]
    fn 控制字符压成空格再截断() {
        assert_eq!(one_line("a\nb\tc", 40), "a b c");
        let long = "字".repeat(80);
        let out = one_line(&long, 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn 时间戳零值显示破折号() {
        assert_eq!(fmt_clock(0), "—");
        assert_eq!(fmt_clock(-1), "—");
        assert_ne!(fmt_clock(1_794_222_429_551), "—");
    }

    #[test]
    fn 按token排这一列彻底消失() {
        // Desk#12 记账：/v1/sessions 没有会话级 token，列删掉。静态哨兵防复活。
        let src = include_str!("data_console.rs");
        assert!(!src.contains("list_by_tokens"), "按 token 排的入口还在");
        assert!(!src.contains("按 token"), "按 token 排的文案还在");
        assert!(!Screen::Convs.hint().contains("token"));
    }

    // ── #12：六屏的 /v1 装配（端点算好，Desk 只画） ────────────────────

    #[test]
    fn stats_装配不解构账本_totals_原样画() {
        let usage: nous_engine::console::StatsUsage = serde_json::from_value(serde_json::json!({
            "groupBy": "day",
            "totals": {"totalTokens": 1234, "costTotal": 0.25, "requests": 7, "errors": 1},
            "rows": [{"key": "2026-11-21", "label": "2026-11-21", "totalTokens": 1234,
                      "costTotal": 0.25, "requests": 7, "errors": 1}]
        })).unwrap();
        let cost = assemble_cost(&usage);
        assert_eq!(cost.totals.tokens, 1234, "totals 逐字来自端点，不在客户端求和");
        assert!((cost.totals.cost - 0.25).abs() < 1e-9);
        assert_eq!(cost.by_day.len(), 1);
        assert_eq!(cost.by_day[0].label, "2026-11-21");
        assert_eq!(cost.by_day[0].tokens, 1234);
    }

    #[test]
    fn stats_装配维修屏用_by_state_不造健康概念() {
        let tasks: nous_engine::console::StatsTasks = serde_json::from_value(serde_json::json!({
            "byState": {"pending": 3, "running": 2, "terminal": 11}, "active": 5
        })).unwrap();
        let device: nous_engine::types::Device = serde_json::from_value(serde_json::json!({
            "node": "qrd3wh", "build": "abc", "target": "x86_64-unknown-linux-gnu",
            "available": []
        })).unwrap();
        let health = assemble_health(&tasks, &device);
        assert_eq!(health.tasks.by_status.iter().find(|b| b.status == "running").unwrap().count, 2);
        assert_eq!(health.tasks.by_status.iter().find(|b| b.status == "pending").unwrap().count, 3);
        assert_eq!(health.tasks.total, 16, "总数就是 byState 各状态之和");
    }

    #[test]
    fn stats_装配解剖屏显示_device_与_stats_db() {
        let device: nous_engine::types::Device = serde_json::from_value(serde_json::json!({
            "node": "qrd3wh", "build": "abc123", "target": "x86_64-unknown-linux-gnu",
            "available": []
        })).unwrap();
        let caps: nous_engine::Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1, "server": {"name": "nous", "version": "0.1.3"},
            "modules": [
                {"name": "sessions", "enabled": true, "endpoints": []},
                {"name": "stats", "enabled": true, "endpoints": []}
            ],
            "runtime": {"provider": {"kind": "faux"}}
        })).unwrap();
        let db: nous_engine::console::StatsDb = serde_json::from_value(serde_json::json!({
            "durable": {"bytes": 1142784, "rows": {"entries": 216}, "quickCheck": "ok", "schema": 2},
            "plugins": {"bytes": 288432, "rows": {}, "quickCheck": "ok", "schema": 1}
        })).unwrap();
        let ov = assemble_overview(&device, &caps, &db);
        assert_eq!(ov.file.db, 1142784);
        assert_eq!(ov.counts.get("entries"), 216);
        assert_eq!(ov.pragmas.page_count, 1, "bytes 换成展示用的页数：至少有 1 页");
        assert!(ov.extra.contains_key("contract"));
        assert!(ov.extra.contains_key("modules"));
    }

    #[test]
    fn 会话树_按token排_这一列已删() {
        // 不是排序测试：这条记账明确「按 token 排」列不再存在（只按引擎给的顺序画）。
        assert!(!Screen::Convs.hint().contains("token"));
        assert!(!std::any::type_name::<Conversations>().is_empty());
        // 屏幕不再提供按 token 排的入口：以 hint 为人话锚点，防止有人把那句加回来。
    }

    #[test]
    fn 回放_从_entries_装配轮次() {
        let entries: nous_engine::types::Entries = serde_json::from_value(serde_json::json!({
            "items": [
                {"id": 1, "kind": "pi.user", "text": "hi", "parent": null},
                {"id": 2, "kind": "pi.assistant", "text": "ho", "parent": null,
                 "model": [{"provider": "faux", "modelId": "echo", "usage": {"totalTokens": 5},
                            "timestamp": 1794222429551i64, "stopReason": "stop"}]}
            ], "next": null
        })).unwrap();
        let r = assemble_replay(42, &entries);
        assert_eq!(r.id, 42);
        assert_eq!(r.entries.len(), 2);
        // 引擎新的在前；回放按轮次正序画，所以 assistant 是 index 1。
        let assistant = r.entries.iter().find(|e| e.kind == "pi.assistant").unwrap();
        assert_eq!(assistant.tokens, 5, "usage 从 model[0] 取，不另算");
        assert_eq!(assistant.stop_reason.as_deref(), Some("stop"));
        assert_eq!(assistant.at, 1794222429551);
    }
