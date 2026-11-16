# demo · 触发器工作台（01 + 03 + 07）

把「卡片 + 详情」「代码 / 发布」「新建向导」三层合成一个工作台。外观稿，**不进构建**。

## 怎么看

```
/opt/NousDesk/demo/trigger-workbench/preview.html
```

左边四个按钮是四层深度的快捷入口（`#main` / `#detail` / `#code` / `#wizard` 也能直接跳）。

**在窗口里也能真的点**：

- 点任意卡片 → 详情（第二层）
- 详情「代码 ›」→ 代码工作区（第三层）
- 代码层「← 触发器」→ 回详情
- 顶栏「新建触发器」→ 向导（另一条支线）
- 向导：身份 → 参数 → 骨架 → 建；点「建」回主界面，新卡片 `morning-review` 会插到最前
- 代码层点「发布」→ 五步流水线依次跑，编译器输出一行行落下来
- 详情「归档」→ 手输名字的确认层
- 顶栏同步 chip → 待同步文件 popover

## 三层是什么

```
主界面（卡片 + 类别 + 筛选 + 同步）
  └─ 详情（右滑，盖卡片区、不盖类别栏）
       └─ 代码（全宽：文件树 / 编辑 / 差异 / 发布流水线）
  └─ 新建向导（全宽：身份 → 参数 → 骨架 → 建）
```

层级：主界面 0 → 详情 20/21 → 代码 30 → 向导 31 → 确认 40。rail 始终在最左，不参与覆盖。

## 覆盖的功能（视图 1）

- 状态四态：`已发布 / 有改动 / 未发布 / 清单坏了`
- 类别树（含零计数）、搜索、状态筛选、同步 chip（待同步 / 有改动）
- 详情：摘要、类别、清单、参数声明、仓库、发布检查
- 代码：文件树、行号、编辑 / 差异、`+N −N`、`⌘S` / `Alt+D`
- 保存 ≠ 发布：`cargo check → build → index.mjs → 只 add 这一个 → commit → push`
- 发布流水线五步 + 编译器输出 + push 拒绝时的提示
- 归档：手输名字；文案区分「归档」和「彻底删除」
- 新建：身份 → 参数逐行编辑 → 三份文件预览 → 建（未发布）
- 8 秒类别同步 popover

## 数据

- 真实：`single-shot v0.1.0`、3 个归档、`HEAD 8517477`、`main`、干净。
- 演示扩展：`daily-digest`（详情 / 代码 / 发布的主角）、`ci-watch`、`disk-guard`、`pr-review`、`release-notes`、`inbox-triage`、`weather-brief`、`link-checker`，以及它们的状态和脏文件数。`morning-review` 是向导建出来的那张新卡。

## 改 demo

```bash
cd demo/trigger-workbench
# 改 index.html
python3 build-preview.py     # 重新生成自包含的 preview.html
```

## 挑完落地（Rust）

1. 多 resolve 一个块：`n8qkkj`（触发器项目）；`qrd3wh` 的 `/v1/triggers` 只是只读投影。
2. `Tab::Triggers` + `triggers.rs`；`nous-engine` 加 n8qkkj 客户端
   （`/v1/triggers`、`files`、`file` GET/PUT、`publish`、`archive`、`restore`、`purge`、`create`、`category`、`categories`、`sync`）。
3. 代码层是最大的一块：行号、diff（与 HEAD 比）、`+N −N`、etag 409、快捷键。
4. 发布要能显示五步进度和 422 的 `stage`（check / build）、push 失败的「本地已提交」。
5. 向导要能按 `params` 声明逐行生成控件（epoch / number / string / text / boolean / select）。
6. 确认层手输名字；未发布 vs 已发布文案分开。
7. 挑完把「为什么是这套层次」写进 `decisions.md`。
