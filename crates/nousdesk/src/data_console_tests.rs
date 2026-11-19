//! 数据监控室的纯逻辑测试（读数 / 事件 / 格式化）。
//!
//! 单独一个文件：和 `data_console.rs` 放在一起时 rustc 会在 `#[test]` 展开上
//! 递归到爆栈（试过 `recursion_limit = 2048` 都不够），拆开就没事。

use crate::data_console::*;
use nous_engine::console::{Conversations, Cost, Health, Overview};
use nous_engine::types::ArchiveGens;


    /// `Desk#13`：删掉 `旧面板取数路径` 取数后，那五屏只能按 capabilities 灰掉，
    /// 不许再画一行「等面板给数据」的红字。`Stats` 是 #12 的活路，本票不接。
    #[test]
    fn removed_data_screens_are_gated() {
        for screen in [Screen::Overview, Screen::Convs, Screen::Replay, Screen::Cost, Screen::Health] {
            assert!(!crate::data_console::screen_has_live_data(screen),
                "{screen:?} 的旧 旧面板取数路径 取数已删，不该报有数据");
        }
        // 归档屏走 /v1/archive，仍是灰入口但数据通道没删。
        assert!(crate::data_console::screen_has_live_data(Screen::Archive));
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
    fn 会话按_token_从多到少() {
        let c = convs();
        let rows = c.list_by_tokens();
        assert_eq!(rows[0].id, 2);
        assert_eq!(rows[1].id, 14);
    }
