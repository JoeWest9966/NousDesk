# spec · Template 页升级成「模板工作台」

> 抓取时间：2026-11-14。真相源是**引擎**那一层（`qrd3wh/presets-api.js`）+ 两个面板页
> （`jtjzw5` = Ultimate-Template、`3699b9` = Session-Template）。复核方式见文末「怎么复核」。

## 0. 一句话

Template 页现在只能「看 + 开一条会话」；目标是把两个面板的**全部**能力搬进来 ——
看 / 建 / 改 / 校验 / 发布 / 归档 / 开 —— 一条不少，交互按 Tesla 的设计语言重排。

## 1. 验收口径（先定，免得事后扯皮）

| # | 口径 | 怎么算过 |
| --- | --- | --- |
| 1 | **功能对等** | 两个面板页上能点的每一条，桌面端这里有对应动作；本文「§4 功能清单」逐条打勾 |
| 2 | **不新造业务** | 名字合不合法、引用还在不在、发布闸过不过，全部问引擎；客户端不写第二套规则 |
| 3 | **不绕过协议层** | 新端点先加到 `crates/nous-engine`，UI 层一行 HTTP 都不写 |
| 4 | **不怕删** | 归档 / 还原 / 彻底删都有「删完会怎样」的清单 + 二确认 |
| 5 | **桌面端要比网页顺** | 键盘可达（无鼠标能建完一条）、409 冲突有救（重读而不是丢输入）、无整页刷新 |

## 2. 引擎端点表（`/v1/presets/<段>/…`，段 = `Session-Template` | `Ultimate-Template`）

来源：`qrd3wh/presets-api.js` 的 `handlePresets`（行 487–747）。**这一段是唯一权威**，
面板和桌面端都只是它的调用方。

| 方法 | 路径（段后） | 参数 | 作用 |
| --- | --- | --- | --- |
| GET | `/v1/presets` | — | 仓目录：每段 id / title / count |
| GET | `/<段>` | — | 这一段总览：`presets[]` + `categories[]` + `repo{head,dirty,ahead,behind}` |
| GET | `/<段>/categories` | — | 类别词表 |
| POST | `/<段>/categories` | `{op, name, to}` | 类别增 / 删 / 改名（连带改预设上的挂载） |
| POST | `/<段>/category` | `{name, category[]}` | 改**一条**预设的类别 |
| GET | `/<段>/files?name=` | — | 一条预设目录里的文件清单（path/size） |
| GET | `/<段>/file?name=&path=` | — | 读文件：`{text, etag}` |
| PUT | `/<段>/file` | `{name, path, text, base}` | 写文件。写清单文件时过**形状闸**（终极模板再加**引用闸**）；`base` 是 etag 乐观锁，冲突 409 |
| POST | `/<段>/create` | 见 §4.5 | 新建一条预设 |
| GET/POST | `/<段>/validate` | `?name=` 或 `{spec}` | 只读校验：`{ok, errors[], warnings[], spec, resolved}` |
| GET/POST | `/<段>/resolve` | 同上 | 只读解析（不带回 spec，只回 `resolved`） |
| POST | `/<段>/publish` | `{message?}` | 形状闸 → 重扫 index → commit → push。失败**一个字节都不动** |
| POST | `/<段>/archive` | `{name}` | 归档（移到 archives，可还原） |
| POST | `/<段>/restore` | `{name}` | 从归档还原 |
| POST | `/<段>/purge` | `{name}` | **真删**（活库 + 归档副本），失败回滚 |
| GET | `/<段>/pending` | — | 待同步状态（哪些文件未提交） |
| GET | `/<段>/pending/diff?path=` | — | 单文件 diff：`{before, after}`；`before: null` = HEAD 里从来没有 |
| GET | `/<段>/pending/commit?hash=` | — | 某个 commit 的详情 |
| POST | `/<段>/sync` | `{paths[], message?}` | 只提交指定文件（`publish` 的局部版本） |
| POST | `/<段>/reload` | — | 重扫名册（盘上被外部改过时用） |

**不属于这一段、但这一页要用的**：

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET | `/v1/triggers` | 触发器目录（终极模板的「什么时候起」+ 参数表单按它声明画） |
| GET | `/v1/combos` | 组合的开关状态 + 调度器节拍（**开关以引擎为准**） |
| POST | `/v1/combos/:name/enable` `/disable` | 开 / 关一条组合（终极模板的「开着 / 关掉」） |
| GET | `/v1/origins` | 来源（复制一条模板时的出处） |
| POST | `/v1/sessions` | 开一条会话（`{preset: "<段>/<name>", name?}`） |
| POST | `/v1/selection` | 把「现在盯着哪条」切过去（建完才有 id） |
| GET | `/v1/device`、`/v1/models` | 钉模型 / 钉机器时列候选 |

## 3. 桌面端现状（2026-11-14）

| 能力 | 现状 | 位置 |
| --- | --- | --- |
| 两段切换 + 计数 | ✅ | `templates.rs` `SECTIONS` |
| 类别树（含零计数）+ 筛选 | ✅ | `facet_tree()` |
| 卡片列表 + 详情 | ✅ | 右栏 |
| 从模板开一条会话 | ✅ | `POST /v1/sessions {preset}` |
| 读 `presets` / `categories` / `repo` | ✅ | `EngineClient::presets()` |
| **新建 / 编辑 / 写文件** | ❌ | — |
| **校验（validate / resolve）** | ❌ | — |
| **发布 / 同步 / 重扫** | ❌ | — |
| **归档 / 还原 / 彻底删** | ❌ | — |
| **diff / 待同步清单** | ❌ | — |
| **改类别 / 建类别** | ❌ | — |
| **终极模板的触发器 / 参数 / 组合开关** | ❌ | — |
| **开对话前看「生效值 / 缺失引用」** | ⚠️ 只显示能不能开 | `canLaunch` 那套在面板里有，这边没有 |

一句话：**读全都有，写全没有**。

## 4. 两个参考页的功能清单

### 4.1 `3699b9` · Session-Template（功能最全，是主要参照）

- **左栏**：类别树（可收起）、筛选、`＋ 新建类别`、`新建预设`；顶栏 `重扫`。
- **主区**：卡片网格（视图可切）、空态。
- **右键菜单**：编辑 / 改类别 / 归档 / 还原 / 彻底删 / 复制目录路径 / 开一条新对话。
- **详情抽屉**：chip（类别、引用、技能）、能否启动、引用失效提示（点名哪一条）。
- **启动抽屉**：会话名输入 → 建会话 → 切 selection → 回来。
- **编辑器**（左右分栏，左边「盘面」右边「JSON」）：
  - **Model**：供应商 › 模型 › 思考档位，方向键操作。
  - **Context · Skills**：跟随默认 / 都不要 / 挑几个。
  - **Context · Prompt（模板）**：钉一条会话模板。
  - **Extensions**、**Permissions**、**Read-only**、**Working dir（工作目录）**、**Machine（机器）**。
  - **保存 = 只落盘**；**发布 = commit + push**（文案明确写着这句）。
  - JSON 栏带行号、diff 视图、错误消息区。
- **新建表单**：名字（小写字母/数字开头，只留 `. _ -`，**建完改不了**）、一句话摘要、类别（写不存在的名字就顺手建）、起手（空白 / 钉住当前设备默认模型）。
- **危险确认**：归档可逆、`purge` 不可逆，各一套文案。
- **toasts**：所有写操作的结果。

### 4.2 `jtjzw5` · Ultimate-Template（编辑器骨架已在，但只读）

- 结构：banner / 类别树 / 卡片（每张有自定义图块）/ 详情（触发器、参数、引用的会话模板）。
- 新建表单字段：**名字**、一句话说明、**触发器**（什么时候起）、**参数**（按触发器自己声明的填）、**会话模板**（起什么对话）、**类别**、**开着 / 关掉**。
- 组合开关走 `POST /v1/combos/:name/enable|disable`，页面不本地猜。
- 详情里的「引用一览 / 引用失效」要**点名**哪一条坏了。

### 4.3 两页的共同规矩（照抄，别自己发明）

- `preset.json` / `ultimate.json` 不合法 → 卡片标红，`去 JSON 那一栏修好`。
- `未发布（HEAD 里没有）` 和 `已保存 · 未提交` 是两个不同的状态，别合并。
- 类别零计数也要显示（建了就得看得见）。
- 「没有远端」也能 commit，只是不 push。

## 5. 差距表（按功能块）

| 功能块 | 面板 | 桌面端 | 差距 |
| --- | --- | --- | --- |
| 列表 / 类别树 / 筛选 | ✅ | ✅ | — |
| 详情 | ✅ | ⚠️ 只读摘要 | 补引用一览、触发器 / 参数、状态 chip |
| 新建 | ✅ | ❌ | 表单 + 校验 |
| 编辑（盘面 + JSON） | ✅ | ❌ | 最大的块 |
| diff / 待同步 | ✅ | ❌ | — |
| 发布 / 同步 / 重扫 | ✅ | ❌ | — |
| 归档 / 还原 / 彻底删 | ✅ | ❌ | 含二确认 |
| 改类别 / 建类别 | ✅ | ❌ | — |
| 组合开关 | ✅（jtjzw5） | ❌ | 依赖 `v1/combos` |
| 开对话前看生效值 / 缺失 | ✅ | ❌ | 客户端可直接算显示，但**生效值认服务端** |

## 6. 五步落地（每步单独分支，外观先出 `demo/`）

| 步 | 范围 | 端点 | 外观 demo？ | 验收 |
| --- | --- | --- | --- | --- |
| **1**（本文） | 功能清单 + 差距表 | 无 | 不需要 | 你逐条看这份表 |
| **2** | 只读层做满：详情补引用/触发器/参数/状态；待同步清单 + diff 视图 | `/v1/presets/<段>`, `/pending`, `/pending/diff`, `/v1/triggers` | **要**（右栏详情排版 + diff 样式） | 能看懂一条模板「是什么、坏没坏、改了什么」 |
| **3** | 新建 + 编辑（盘面 + JSON + Ctrl+S），先 Session-Template | `/create`, `/file`(GET/PUT), `/validate`, `/files` | **要**（编辑器布局） | 能建一条、改一条、409 有救 |
| **4** | 生命周期：发布 / 同步 / 归档 / 还原 / 彻底删 / 重扫 / 改类别 | `/publish`, `/sync`, `/archive`, `/restore`, `/purge`, `/reload`, `/categories`, `/category` | **要**（危险确认 + 状态提示） | 每一步都能说出「动的是哪些文件」 |
| **5** | 终极模板：触发器 / 参数表单 / 引用会话模板 / 组合开关 / 开对话前预览生效值 | `/create`(Ultimate), `/validate`, `/resolve`, `/v1/combos`, `/v1/origins` | **要**（终极模板编辑器） | 能建一条终极模板并开起来 |

第 2 步必须在第 3 步前（编辑器要复用它做出来的详情）；第 5 步依赖第 3 步的表单骨架。

## 7. 不做的事（明确边界）

- **不复制面板的结构**：桌面端不做「网页那套左右分栏编辑器」的像素级复刻，形态按 Tesla 重排（外观先 demo）。
- **不在客户端存模板状态**：所有清单来自引擎；本地只有「正在编辑的草稿」。
- **不做多段合并**：段就是段，`Session-Template` 和 `Ultimate-Template` 各自一套类别词表。
- **不自己算生效值**：`launch` 后显示的是服务端返回的 thinkingLevel / skills / template / missing[]。
- **不做模板的 git 操作**：桌面端只调引擎的 `publish / sync`，不跑 `git`。

## 8. 怎么复核这份 spec

```bash
ENG=$(curl -s http://192.168.1.7:4790/p/new1/h58ga5/__nous/resolve/qrd3wh | python3 -c 'import sys,json;print(json.load(sys.stdin)["url"])')
curl -s "$ENG/v1/presets" | head -c 400
curl -s "$ENG/v1/presets/Session-Template" | head -c 400
# 面板页：
#   http://192.168.1.7:4790/p/new1/3699b9/   （Session-Template）
#   http://192.168.1.7:4790/p/new1/jtjzw5/   （Ultimate-Template）
# 引擎源码（j1900）：
#   /opt/Nous/content/new1/map/qrd3wh/presets-api.js:487-747
```
