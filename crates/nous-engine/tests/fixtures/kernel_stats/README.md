# kernel_stats · 数据监控室明细端点的真实输出夹具

这五份 JSON 是**内核真跑出来的**，不是人手写的形状样本：

| 文件 | 端点 | 对应 Desk 结构体 |
|---|---|---|
| `stats_overview.json` | `GET /v1/stats/overview` | `StatsOverview`（解剖屏） |
| `stats_conversations.json` | `GET /v1/stats/conversations` | `StatsConvs`（会话树屏） |
| `stats_conversation.json` | `GET /v1/stats/conversation/:id` | `StatsReplay`（回放屏） |
| `stats_cost.json` | `GET /v1/stats/cost` | `StatsCost`（账单屏） |
| `stats_health.json` | `GET /v1/stats/health` | `StatsHealth`（维修屏） |

判据在 `crates/nous-engine/tests/kernel_stats_shapes.rs`：把每份反序列化进 Desk 的展示结构体，
任何字段名/类型漂移都会让它编译不过或测试红 —— 这是「Desk 拿得到、画得出」这条线的机器判据。

## 来源

- 内核仓 `JoeWest9966/Nous`，分支 `feat/stats-parity`（数据监控室六屏明细读口）。
- 数据：内核仓 `fixtures/agent.small.sqlite`（189 会话 / 209 entries / 128 tasks 的真 fixture）。
- `health` 传入的 `now_ms = 1_700_000_000_000`（固定值，避免「卡住」判定随时钟漂）。

## 重生成

内核仓里（`feat/stats-parity`）：

```bash
# 用一个一次性 example 把五份 JSON 打到 /tmp（见该分支 crates/nous/stats/examples/dump.rs 的写法）
cargo run -q -p nous-stats --example dump -- /tmp
```

然后把 `/tmp/stats_*.json` 拷回本目录。`conversations` / `conversation` 两份会先在测试侧截断
数组（前 5 / 前 3 条），只保留字段形状，不为内容体积买单。
