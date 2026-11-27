# 插件页 · 把「依赖层」也列出来（原型 + 票）

双击 `preview.html`（单文件、无端口、字体用系统 mono/Sans，不依赖内嵌）。

- `?mode=before`（默认）：**现在的样子** —— 列表只有 15 个端点模块。点详情里带 `?` 的胶囊（`ai` / `durable` / `routes` / `wire`），会跳进一片空白，那就是问题本身。
- `?mode=after`：**改后的样子** —— 列表分三段：`端点模块 · 15` / `依赖层 · 16` / `未登记 · 1`。点 `durable` 看库的详情（层 / 版本 / 来源 crate / 依赖 / 被依赖，端点一栏写「—（库，不提供端点）」）。

## 事实（j1900 0.1.41 真机核过）

| 口径 | 数 | 来源 |
|---|---|---|
| 有 `plugin.toml` 的插件目录 | 31 | `crates/<vendor>/<module>/plugin.toml` |
| `/v1/modules/graph` 节点 | 31 | 关系网插件扫 `plugin.toml` 生成，点一下真返回 31 节点 / 96 边 |
| `capabilities.modules` | 15 | `GET /v1/capabilities`（165 条端点） |
| 列表里看得见的 | 15 | Desk 插件页只读 `capabilities.modules` |

所以：**图是完整的，列表是残缺的。** 16 个库在关系图里有节点、能画胶囊、能点，但点进去 Desk 渲染不出任何东西，因为它不在 `capabilities.modules` 里。

## 三段分别是什么

- **端点模块（15）**：自带 HTTP 端点，客户端能调用。`capabilities.modules` 的成员。
- **依赖层（16）**：`ai` / `ai-types` / `chord` / `dispatcher` / `durable` / `env-ssh` / `ext-probe` / `ext-sdk` / `fixture` / `routes` / `runtime` / `server` / `store` / `telemetry` / `tools` / `wire`。零端点（判据 4 还禁止它们出现 `axum::` / `.route(`），只有被依赖关系。
- **未登记（1）**：`AboutME_ModuleRelationshipNetwork`。它的 `GET /v1/modules/graph` 真能返回 200，但 `/v1/_routes` 的 165 条里没有它、`capabilities.modules` 也不列它 —— 自带端点却没进清单，是另一处漏项，票里顺手修。

两套名单不完全重合，别硬凑成一个数：`presets` 只在 capabilities 里（与 `presets-admin` 同 crate，graph 里只有一个节点）。

## 票

标题：`插件页：把「依赖层」也列出来（A+C，零引擎改动）`

要做：

1. 左侧列表分两段（`端点模块 · 15` / `依赖层 · 16`），数据都从**已有的** `GET /v1/modules/graph` 来。
2. 依赖层行：名字 + vendor + 版本 + 层 + `无端点` 标签，端点一栏**留空**，不写 `0 端点`。
3. 点依赖层行 → 详情页：`来源 crate` / `层` / `版本` / 依赖 / 被依赖，顶部一句「这是库，不提供端点」。
4. 关系胶囊点不动的问题顺手闭环（现在点库跳空白）。
5. 不影响 `capabilities`：`modules[]` 仍 15 个、端点仍 165 条，**不往契约里塞空端点行**。

不做的：

- 不改 `/v1/modules/graph` 的响应形状（补 `layer` / `version` / `crate` 是另一票 B）。
- 不把依赖层写进 `capabilities`。
- 不改引擎。
- 不给库出「读文档」（库的 README 现在不进 `docs/plugins/`，要不要进是第三个决定）。

## 数据来源

- 模块元数据（vendor / version / layer / crate / desc / requires）：`../plugin-relations/data.json`（从内核仓各 `plugin.toml` 抄的真数据）。
- 端点数：j1900 0.1.41 真机 `GET /v1/_routes` 按 `module` 分组。
- `AboutME` 的 1 条端点：真机 `GET /v1/modules/graph` 返回 200，但不在 `_routes` 里，上面「未登记」段如实标出。

## 改 demo

```bash
cd /opt/NousDesk && python3 demo/plugin-layers/build.py
```

`build.py` 里 `MODULES` / `ENDPOINTS` / `UNREGISTERED` 是抄来的真值，改数据改这三处。
demo 不进构建、不碰 `Cargo.toml`。