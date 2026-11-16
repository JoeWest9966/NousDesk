//! 面板（d6s4v1）五个接口的真响应快照，钉住「形状」。
//!
//! 快照来自 2026-11-14 的真库（j1900 `/p/new1/d6s4v1/api/*`），只裁短了
//! 正文（`text` 截 60 字）和列表长度（会话 4 条 / entries 5 条），字段一个没删。
//!
//! 这里**不联网**：CI 上没有宿主、也没有这块板子。真要连真库看，跑
//! [`真实面板能解析`]（`#[ignore]`，手工 `cargo test -- --ignored`）。

use nous_engine::console::ConsoleClient;

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
    std::fs::read_to_string(format!("{path}{name}")).expect("fixture 读不到")
}

#[test]
fn overview_文件表索引都在() {
    let o: nous_engine::console::Overview =
        serde_json::from_str(&fixture("console_overview.json")).expect("解析");
    assert_eq!(o.target.id, "qrd3wh");
    assert!(o.target.path.ends_with("agent.sqlite"));
    assert_eq!(o.pragmas.page_size, 4096);
    // 文件字节数和「页数 × 页大小」**不**总相等：WAL 模式下 db 文件的 mtime/长度
    // 会落后几页（真库上就差了 2 页 = 8192B）。别断言相等，只断言同一个量级。
    let drift = (o.file.db - o.pragmas.db_bytes()).abs();
    assert!(
        drift <= 16 * o.pragmas.page_size,
        "文件 {} B 和页算出来的 {} B 差得太远（{drift} B）",
        o.file.db,
        o.pragmas.db_bytes()
    );
    assert_eq!(o.tables.len(), 9);
    assert!(o.tables.iter().any(|t| t.name == "entries" && t.rows > 0));
    assert_eq!(o.indexes.len(), 17);
    assert!(o.integrity.checked && o.integrity.ok);
    assert!(o.counts.get("entries") > 0);
    assert!(o.entries_by_kind.iter().any(|k| k.kind == "pi.user"));
}

#[test]
fn conversations_有总量也有单条() {
    let c: nous_engine::console::Conversations =
        serde_json::from_str(&fixture("console_conversations.json")).expect("解析");
    assert_eq!(c.count as usize, c.list.len().max(c.count as usize));
    assert!(c.totals.tokens > 0);
    let first = &c.list[0];
    assert!(first.id > 0);
    assert!(first.last_at >= first.first_at);
}

#[test]
fn conversation_回放有_entries_任务文档和_prompt() {
    let c: nous_engine::console::Conversation =
        serde_json::from_str(&fixture("console_conversation.json")).expect("解析");
    assert!(!c.entries.is_empty());
    // seq 递增 —— 时间线要按这个画，读错了顺序就会倒着播。
    let seqs: Vec<i64> = c.entries.iter().map(|e| e.seq).collect();
    assert!(seqs.windows(2).all(|w| w[0] <= w[1]), "seq 该是递增的：{seqs:?}");
    assert!(!c.tasks.is_empty());
    assert!(!c.prompt.messages.is_empty());
    assert!(c.prompt.messages[0].text.chars().count() > 0);
}

#[test]
fn cost_有总量和四组桶() {
    let c: nous_engine::console::Cost = serde_json::from_str(&fixture("console_cost.json")).expect("解析");
    assert!(c.totals.tokens > 0);
    assert!(c.totals.cache_read > 0);
    assert!(!c.by_day.is_empty());
    assert!(!c.by_model.is_empty());
    assert!(!c.by_conv.is_empty());
    assert!(c.totals.cache_hit() > 0.5, "库里的缓存命中率该过半");
    assert!(!c.note.is_empty(), "「cost 为 0 不等于免费」那句得留着");
}

#[test]
fn health_有别_status_和慢任务() {
    let h: nous_engine::console::Health =
        serde_json::from_str(&fixture("console_health.json")).expect("解析");
    assert!(h.tasks.total > 0);
    assert!(!h.tasks.by_status.is_empty());
    assert!(!h.tasks.slowest.is_empty());
    assert!(h.documents.revisions >= h.documents.base);
    assert!(!h.maintenance.is_empty());
}

/// 手工：连真宿主的这块板子，五个接口都能解析。CI 上别跑。
#[test]
#[ignore = "要真宿主：cargo test -p nous-engine --test console_fixtures -- --ignored"]
fn 真实面板能解析() {
    let host = std::env::var("NOUS_HOST").unwrap_or_else(|_| "http://192.168.1.7:4790".into());
    let board = std::env::var("NOUS_BOARD").unwrap_or_else(|_| "new1".into());
    let c = ConsoleClient::new(&host, &board, std::time::Duration::from_secs(8)).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let o = c.overview().await.expect("overview");
        assert!(o.counts.get("entries") > 0);
        let cv = c.conversations().await.expect("conversations");
        let id = cv.list.first().map(|r| r.id).expect("库里至少一条会话");
        let one = c.conversation(id).await.expect("conversation");
        assert_eq!(one.id, id);
        let cost = c.cost().await.expect("cost");
        assert!(cost.totals.tokens > 0);
        let h = c.health().await.expect("health");
        assert!(h.tasks.total > 0);
    });
}
