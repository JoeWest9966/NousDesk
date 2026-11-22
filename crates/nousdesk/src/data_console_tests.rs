//! 数据监控室的纯逻辑测试（读数 / 事件 / 格式化）。
//!
//! 单独一个文件：和 `data_console.rs` 放在一起时 rustc 会在 `#[test]` 展开上
//! 递归到爆栈（试过 `recursion_limit = 2048` 都不够），拆开就没事。

use crate::data_console::*;
use nous_engine::console::{Conversation as Replay, Conversations, Cost, Health, Overview};
use nous_engine::types::ArchiveGens;


    /// `Desk#12` 判据 5 的负向哨兵：归档取数门必须是纯函数，能力关时**不发**
    /// `archive_list`。把 `archive` 从灰色改成「直接拉」，这条立刻红。
    #[test]
    fn archive_fetch_is_gated_by_capabilities() {
        let caps_off: nous_engine::Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1, "server": {"name": "nous", "version": "0.1.3"},
            "modules": [
                {"name": "archive", "enabled": false,
                 "reason": {"code": "not_implemented", "message": "归档模块尚未落地"},
                 "endpoints": []}
            ],
            "runtime": {"provider": {"kind": "faux"}}
        })).expect("caps off");
        let caps_on: nous_engine::Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1, "server": {"name": "nous", "version": "0.1.3"},
            "modules": [
                {"name": "archive", "enabled": true, "endpoints": []}
            ],
            "runtime": {"provider": {"kind": "faux"}}
        })).expect("caps on");

        assert!(!archive_should_fetch(Some(&caps_off)), "能力关：不发 archive_list");
        assert!(archive_should_fetch(Some(&caps_on)), "能力开：真拉 archive_list");
        assert!(!archive_should_fetch(None), "没连引擎：不发 archive_list");

        // 调用点绑定：只测纯函数不够 —— 有人把 `load_console` 改成「直接拉」
        // 而把函数留在那里当摆设，行为就变了却没人发现。这里钉住唯一的取数门。
        let src = include_str!("data_console.rs");
        assert!(
            src.contains("archive_should_fetch(self.caps.as_ref())"),
            "归档取数没走纯函数门（可能被改成了直接拉）"
        );
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

    // ── 六屏：端点算好，Desk 直接反序列化（不再有 assemble_* ） ──────────

    #[test]
    fn stats_账单直接反序列化_cost_端点() {
        // 形状对齐引擎 `GET /v1/stats/cost`；Desk 只画，不重算。
        let cost: Cost = serde_json::from_value(serde_json::json!({
            "totals": {"entries": 7, "tokens": 1234, "input": 100, "output": 50,
                       "cacheRead": 200, "cacheWrite": 3, "reasoning": 4, "cost": 0.25},
            "byDay": [{"label": "11-21", "tokens": 1234, "cost": 0.25, "calls": 7,
                       "input": 100, "output": 50, "cacheRead": 200}],
            "byHour": [{"label": "11-21 03h", "tokens": 1234, "cost": 0.25, "calls": 7,
                        "input": 100, "output": 50, "cacheRead": 200}],
            "byModel": [],
            "byConv": [],
            "note": "cost 为 0 不代表免费"
        })).unwrap();
        assert_eq!(cost.totals.tokens, 1234);
        assert_eq!(cost.totals.input, 100);
        assert_eq!(cost.totals.cache_read, 200);
        assert_eq!(cost.by_day.len(), 1);
        assert_eq!(cost.by_day[0].label, "11-21");
        assert!(!cost.by_hour.is_empty(), "byHour 现在有来源了");
        assert!(!cost.note.is_empty());
    }

    #[test]
    fn stats_维修屏直接反序列化_health_端点() {
        let health: Health = serde_json::from_value(serde_json::json!({
            "tasks": {"total": 16,
                      "byStatus": [{"status": "running", "count": 2, "sample": []},
                                   {"status": "pending", "count": 3, "sample": []}],
                      "stuck": [{"id": 1, "ageMs": 700000}], "sleeping": 0,
                      "avgMs": 1000, "p95": 2000, "slowest": []},
            "submissions": {"total": 5, "byStatus": [], "unanswered": [{"id": 9, "conv": 1, "status": "unanswered"}]},
            "documents": {"documents": 2, "byKind": [], "revisions": 4, "base": 2, "delta": 2, "deltaRatio": 0.5},
            "walBytes": 4096,
            "freelist": {"free": 1, "bytes": 4096},
            "maintenance": [{"label": "备份", "why": "", "cmd": "cp", "danger": false}]
        })).unwrap();
        assert_eq!(health.tasks.total, 16);
        assert_eq!(health.tasks.by_status.iter().find(|b| b.status == "running").unwrap().count, 2);
        assert_eq!(health.tasks.stuck.len(), 1, "卡住的任务现在有来源了");
        assert_eq!(health.submissions.unanswered.len(), 1);
        assert_eq!(health.documents.revisions, 4);
        assert_eq!(health.wal_bytes, 4096);
    }

    #[test]
    fn stats_解剖屏直接反序列化_overview_端点() {
        let ov: Overview = serde_json::from_value(serde_json::json!({
            "target": {"id": "nous.db", "path": "/var/lib/nous/nous.db"},
            "file": {"db": 1142784, "wal": 0, "shm": 0, "mtime": 0.0, "walMtime": 0.0},
            "pragmas": {"pageSize": 4096, "pageCount": 279, "freelist": 0, "journalMode": "wal", "schemaVersion": 2},
            "bytes": {"db": 1142784, "wal": 0, "shm": 0, "free": 0, "used": 1142784, "onDisk": 1142784},
            "counts": {"entries": 216, "conversations": 120},
            "tables": [{"name": "entries", "bytes": 800000, "pages": 200, "rows": 216}],
            "indexes": [{"name": "entries_by_conversation", "bytes": 40000, "pages": 10, "table": "entries"}],
            "entriesByKind": [{"kind": "pi.assistant", "count": 91, "bytes": 85579}],
            "integrity": {"checked": true, "ok": true, "messages": []},
            "attention": []
        })).unwrap();
        assert_eq!(ov.file.db, 1142784);
        assert_eq!(ov.pragmas.page_count, 279, "真页数，不是假 1");
        assert_eq!(ov.counts.get("entries"), 216);
        assert_eq!(ov.tables.len(), 1);
        assert_eq!(ov.tables[0].bytes, 800000, "dbstat 真字节");
        assert_eq!(ov.indexes.len(), 1);
        assert_eq!(ov.entries_by_kind.len(), 1);
        assert!(ov.integrity.ok);
    }

    #[test]
    fn 回放直接反序列化_conversation_端点() {
        // 形状对齐引擎 `GET /v1/stats/conversation/:id`。
        let r: Replay = serde_json::from_value(serde_json::json!({
            "id": 42,
            "ownerConv": null,
            "ownerTask": null,
            "entries": [
                {"id": 1, "conv": 42, "seq": 1, "head": null, "kind": "pi.user", "bytes": 10,
                 "at": 1794222420000i64, "role": "user", "tokens": 0, "cost": 0,
                 "toolCalls": [], "isError": false},
                {"id": 2, "conv": 42, "seq": 2, "head": null, "kind": "pi.assistant", "bytes": 20,
                 "at": 1794222429551i64, "role": "assistant", "model": "echo", "tokens": 5, "cost": 0,
                 "usage": {"input": 5, "output": 0, "cacheRead": 0, "totalTokens": 5, "cost": 0},
                 "toolCalls": [], "isError": false, "stopReason": "stop"}
            ],
            "tasks": [{"id": 7, "kind": "pi.agent", "status": "terminal", "startedAt": 1, "endedAt": 2001, "ms": 2000, "conv": 42, "outcome": "done"}],
            "documents": [{"id": 3, "kind": "pi.agent", "key": "k", "revisions": 2, "bytes": 100, "createdAt": 1, "retired": false}],
            "submissions": [{"id": 9, "status": "unanswered", "requestId": null}],
            "prompt": {"messages": [{"id": 1, "label": "user", "kind": "pi.user", "chars": 10, "tokens": 0, "cost": 0, "head": false, "text": "hi"}],
                       "totals": {"messages": 1, "chars": 10, "toolChars": 0, "approxTokens": 3, "realInputTokens": 5}}
        })).unwrap();
        assert_eq!(r.id, 42);
        assert_eq!(r.entries.len(), 2);
        let assistant = r.entries.iter().find(|e| e.kind == "pi.assistant").unwrap();
        assert_eq!(assistant.tokens, 5);
        assert_eq!(assistant.stop_reason.as_deref(), Some("stop"));
        assert_eq!(assistant.at, 1794222429551);
        assert_eq!(r.tasks.len(), 1, "回放屏的任务现在有来源了");
        assert_eq!(r.documents.len(), 1);
        assert_eq!(r.prompt.messages.len(), 1, "prompt 重建现在有来源了");
    }
