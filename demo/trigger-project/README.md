# demo · 触发器 · 视图 1（触发器项目）

只做「触发器（项目）」这一层。外观稿，**不进构建**、不碰 `Cargo.toml`。

## 怎么看

**发出去的是单文件**，双击就开：

```
/opt/NousDesk/demo/trigger-project/preview.html
```

左边一列点着切（`#1`…`#10` 也能直接跳）。窗口 1280×800，按屏幕缩放；自绘标题栏省略。

## 这一版覆盖了什么（不是只读目录）

按 `n8qkkj`（触发器管理块）的真实功能落，每套都按全功能画：

- 清单：卡片 / 列表 / 表格；状态 `已发布 / 有改动 / 未发布 / 清单坏了`
- 类别：树（含零计数）、新建、改名、删除、**8 秒自动同步**
- 详情：摘要、清单、参数声明、类别、仓库、发布检查
- 代码：文件树（`nous.json` / `src/main.rs` / `Cargo.toml` / `README.md`）、行号、编辑 / 差异、`+N −N`
- 保存 ≠ 发布：保存只落盘；发布是 `cargo check` → `cargo build` → `node index.mjs` → 只 add 一个触发器 + `index.json` → commit → push
- 归档：整个目录进 `.archive/`，恢复 / 彻底删除（手输名字）
- 新建：目录 + `Cargo.toml` + 能编过的 `main.rs` + `nous.json`，建完未发布
- 同步 chip：已同步 / 2 个有改动 / 待同步 / 推送中 / 推送失败 / 远端有新提交
- 右键菜单：看代码 / 改类别 / 复制信息（给 AI 的 prompt）/ 归档
- 发布流水线：五步状态、编译器输出、422 停在哪一步

每套的重点不同（避免 10 个换皮）：

| | 名字 | 重点 |
| --- | --- | --- |
| 01 | 卡片网格 + 详情浮层 | n8qkkj 网页原结构：顶栏 + 类别 + 状态筛选 + 卡片 + 详情 + 右键菜单 |
| 02 | 三栏常驻 + 代码抽屉 | 类别 / 列表 / 详情同屏，代码抽屉从底部盖上来 |
| 03 | 代码优先（IDE） | 文件 tab + 编辑 / 差异 + 行号 + 发布流水线 + 编译器输出 |
| 04 | 发布工作台（git） | 改动按触发器分组、勾选、unified diff、五道闸门、只发一个 |
| 05 | 运维表格 + 抽屉 | 12 行一屏：NAME / VERSION / CATEGORY / PARAMS / DIRTY / STATE / TRACKED / UPDATED |
| 06 | 类别与归档 | 类别树 + 右键改名 / 删除 + 归档区 + 手输名字确认 |
| 07 | 新建向导 | 身份 → 参数声明 → 骨架预览 → 建；参数逐行编辑 + 三份文件预览 |
| 08 | Inspector | 左列表 + 属性面板 + 常驻代码 + 发布历史 |
| 09 | 终端 / 控制台（暗） | 暗色 + 等宽：mono 列表、diff、发布日志、五步流水线 |
| 10 | 活动流 + 发布历史 | 按最近改动分天排、宽行、右侧发布历史时间线 |

## 数据

- **真实**：`single-shot v0.1.0`（时间 / 一次，`epoch` 参数，已发布）、3 个归档（`morning-review` / `issue-watch` / `dep-alert`）、仓库 `HEAD 8517477`、`main`、干净。
- **演示扩展**：`daily-digest`、`ci-watch`、`disk-guard`、`pr-review`、`release-notes`、`inbox-triage`、`weather-brief`、`stale-prs`、`nightly-backup`、`link-checker`，以及它们的状态、脏文件数、版本、参数声明、归档条目。这些是为了把 `broken / unpublished / dirty / ok` 和「发布 / 归档 / 类别同步」画出来，**不是盘上真有的**。
- 状态机、字段名、动作语义都照 `n8qkkj/server.js` + `app.js`：
  - 状态优先级 `broken > unpublished > dirty > ok`
  - 保存（`PUT /v1/triggers/file`）只落盘；发布（`POST /v1/triggers/publish`）才 check + build + index + commit + push
  - 归档 = `git mv` 到 `.archive/` + `archived.json` + commit + push；彻底删除才真删
  - 类别改完 8 秒 debounce 自动推（只推 `categories.json` / `index.json` / `*/nous.json`）
  - 破坏性动作要手输名字
  - 名字 = 目录名 = `nous.json.name` = 二进制名，**不改名**

## 改这份 demo

```bash
cd demo/trigger-project
# 改 index.html（源，字体走 fonts/）
python3 build-preview.py     # 重新生成自包含的 preview.html
```

## 挑完之后要动的地方（Rust）

1. 桌面端要**多 resolve 一个块**：`n8qkkj`（现在只 resolve `qrd3wh`）。
   - `GET <host>/p/<board>/h58ga5/__nous/resolve/n8qkkj` → `http://…/p/new1/n8qkkj`
   - 触发器清单 / 文件 / 保存 / 发布 / 归档全在这块；`qrd3wh` 的 `/v1/triggers` 只是只读投影，没有 `tracked / dirty / error / repo / archived`。
2. `crates/nousdesk`：`Tab` 加 `Triggers`；新页面 `triggers.rs`；`nous-engine` 加 n8qkkj 的客户端
   （`GET /v1/triggers`、`files`、`file` GET/PUT、`publish`、`archive`、`restore`、`purge`、`create`、`category`、`categories`、`sync`）。
3. 代码编辑器是最大的一块：行号、diff（与 HEAD 比）、`+N −N`、etag 409 冲突、`⌘S` / `Alt+D`。
4. 发布要能显示五步进度和 422 的 `stage`（`check` / `build`）、push 失败的「本地已提交」提示。
5. 破坏性确认层要手输名字，未发布 vs 已发布文案分开。
6. 落地前先挑上面 10 套里的一个；挑完把「为什么是这套」写进 `decisions.md`。
