//! 数据监控室五条明细端点的 **wire 形状契约**：内核真实输出 → Desk 展示结构体。
//!
//! 夹具 `fixtures/kernel_stats/*.json` 是 `nous-stats`（feat/stats-parity）在
//! `fixtures/agent.small.sqlite` 上**真跑出来的** JSON，不是人手写的样本。
//! 这里逐份反序列化，任何字段名/类型漂移都会让 Desk 编译期结构体收不下 ——
//! 这是「Desk 拿得到、画得出」这条线的机器判据。
//!
//! 更新夹具的方式写在同目录 `README.md`。

use nous_engine::console::{StatsConvs, StatsCost, StatsHealth, StatsOverview, StatsReplay};

fn load(name: &str) -> serde_json::Value {
    let raw = match name {
        "overview" => include_str!("fixtures/kernel_stats/stats_overview.json"),
        "conversations" => include_str!("fixtures/kernel_stats/stats_conversations.json"),
        "conversation" => include_str!("fixtures/kernel_stats/stats_conversation.json"),
        "cost" => include_str!("fixtures/kernel_stats/stats_cost.json"),
        "health" => include_str!("fixtures/kernel_stats/stats_health.json"),
        other => panic!("没有这份夹具：{other}"),
    };
    serde_json::from_str(raw).expect("夹具必须是合法 JSON")
}

#[test]
fn overview_wire_deserializes_into_desk_struct() {
    let ov: StatsOverview = serde_json::from_value(load("overview")).expect("overview 形状");
    // dbstat 真页数与真字节都进来了（不是占位 0）。
    assert!(ov.pragmas.page_size > 0, "page_size={}", ov.pragmas.page_size);
    assert!(ov.pragmas.page_count > 0, "page_count={}", ov.pragmas.page_count);
    assert!(ov.file.db > 0);
    assert_eq!(ov.pragmas.schema_version, 2);
    assert!(!ov.tables.is_empty(), "tables 不该空");
    assert!(
        ov.tables.iter().any(|t| t.bytes > 0 && t.pages > 0),
        "至少一张表有 dbstat 真字节/真页"
    );
    assert!(ov.counts.get("entries") > 0);
    assert!(ov.integrity.checked, "integrity 应真跑过");
}

#[test]
fn conversations_wire_deserializes_into_desk_struct() {
    let c: StatsConvs = serde_json::from_value(load("conversations")).expect("conversations 形状");
    assert!(c.count > 0);
    assert!(!c.list.is_empty());
    assert!(c.totals.entries > 0, "totals 现在有来源了");
    let row = &c.list[0];
    assert!(row.id > 0);
    // 每会话的聚合列（不再是占位零）。
    assert!(row.entries > 0, "entries 列应 > 0");
    assert!(row.tokens > 0, "tokens 列应 > 0");
    assert!(!row.title.is_empty());
}

#[test]
fn conversation_wire_deserializes_into_desk_struct() {
    let r: StatsReplay = serde_json::from_value(load("conversation")).expect("conversation 形状");
    assert!(r.id > 0);
    assert!(!r.entries.is_empty());
    // tasks / documents / prompt 三块回放屏都要画，必须都有来源。
    assert!(!r.tasks.is_empty(), "回放的任务应有来源");
    assert!(!r.prompt.messages.is_empty(), "prompt 重建应有来源");
    // entry 的 seq 是真 commit_seq（不是数组下标兜底）。
    assert!(r.entries.iter().any(|e| e.seq > 0));
    assert!(r.entries.iter().any(|e| e.kind.starts_with("pi.")));
}

#[test]
fn cost_wire_deserializes_into_desk_struct() {
    let c: StatsCost = serde_json::from_value(load("cost")).expect("cost 形状");
    // totals 的 usage 分项（拉片缓存命中率吃它）。
    assert!(c.totals.tokens > 0);
    assert!(c.totals.input > 0, "input 分项应有值");
    assert!(!c.by_day.is_empty(), "byDay 不该空");
    assert!(!c.by_hour.is_empty(), "byHour 现在有来源了");
    assert!(!c.by_model.is_empty());
    assert!(!c.by_conv.is_empty());
    assert!(!c.note.is_empty(), "cost=0 不代表免费那句必须带");
}

#[test]
fn health_wire_deserializes_into_desk_struct() {
    let h: StatsHealth = serde_json::from_value(load("health")).expect("health 形状");
    assert!(h.tasks.total > 0);
    assert!(!h.tasks.by_status.is_empty());
    // 维修屏的明细（卡住/最慢/提交/文档）都要有来源。
    assert!(!h.submissions.unanswered.is_empty(), "未答提交应有来源");
    assert!(h.documents.revisions > 0, "document revisions 应有来源");
    // 维修工单是命令清单，不是空。
    assert!(h.maintenance.len() >= 4);
    assert!(h.maintenance.iter().any(|m| m.cmd.contains("wal_checkpoint")));
}
