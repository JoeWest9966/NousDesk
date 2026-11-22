# wire — 我们和 qrd3wh 之间的线

引擎是 `qrd3wh`（Nous 的 session 块）。这份抄的是**此刻**的协议，
字段是从引擎 `server.js` 和真实消费方 `bjes6b` 里读出来的，不是文档里的。

**这一份会漂。** 引擎加字段是常态。我们只依赖下面「我们依赖」列出来的字段，
其余字段一律 `serde_json::Value` 收着透传，不碰、不 `deny_unknown_fields`。

---


## 这份合同**不完整**（2026-11-18 摸清）

`crates/nous-engine/src` 实际调用的端点数与本文件写了多少条**不相等**（这两个数一直在变，
别抄具体数字 —— 抄了就会在某次改动后变成假话）。要看真数，直接问代码：

```sh
rg -o 'v1/[a-z0-9/{}:_-]*' crates/nous-engine/src | sort -u | wc -l   # 客户端在调
```

正文没写、代码在用的那些，列在文末「合同外调用」。

这不是谁偷懒：本文件抄的是「我们**依赖**的字段」，而下面这些端点我们是**整包转发**
（`serde_json::Value` 进、`Value` 出，不看里面），所以当初没写进来。但后果是 ——
**只看这份文件会以为某些调用没有依据**。改客户端调用前先扫一眼文末那节。

## 与 Rust 版 Nous 的关系

Nous 已重写成 Rust（一个进程 `nous-app`，`board` / `block` / `resolve` 都没了）。

**别再手工维护「哪条做没做」的表。** 那张表一定是快照，追不上引擎：2026-11-21 它还写着
「`presets` 先不做」「`archive` 能力表关掉」「`messages` 没回复」，而这三样当时都已经上线。
照抄它的结论就是拿过期清单当活列表。

**唯一权威是运行时**：`GET /v1/capabilities` → `modules[].enabled` + `reason.code`。
客户端开机打一次，入口灰不灰只在 `crates/nousdesk/src/capability.rs` 判一处；
不知道自己该要什么的时候，问这个端点，别查文档、别猜。

这份文件只记**我们依赖哪些字段**（下文各节），不记模块的完成度。

---

## 通用

- 会话 id 是 **数字**（不是字符串）。`GET /v1/sessions/:id/...` 里的 `:id` 是数字。
- 列表里 `busy` 是 `"queued" | "running" | <task state kind> | null`，不是布尔。
- 错误形状统一 `{ error: string }`，状态码跟着语义走（400 / 404 / 409 / 500…）。

## 我们用到的端点

### GET /v1/sessions

会话**树**（不是列表）。返回 `{ items: [...] }`，**新的在前**。

每条的形状：

```
{ id, name, parent: {id, at} | null, owner, model: "provider/modelId",
  thinkingLevel, thinkingLevels: string[], skills: string[], template: string|null,
  readOnly: bool, busy, updatedAt: number | null, last: {id, kind, text} | null,
  archived: null | {at, gen, purged} }
```

我们依赖：`id` `name` `parent` `model` `thinkingLevel` `readOnly` `busy` `updatedAt` `last` `archived` `pinned`。

- `pinned` 由内核算好：置顶在前、其余保持原序。客户端只读它分组，不自己存置顶表。

- `archived` 是**归档墓碑**，`null` = 没归档过（捞回之后也回到 `null`），对象 = `{at, gen, purged}`，
  `at` 是 epoch **毫秒**。老引擎整个字段缺席时也是 `null` —— 别当布尔用，也别拿它判断
  「这一行要不要显示」（那个由引擎默认藏掉，见下面 `archive`）。

- `updatedAt` 是这条会话**最后一次落库**的时刻，epoch **毫秒**，可能是 `null`。
  `null` 不是 bug：引擎只记「自己看着它写的那些」，重启前的老会话就是空的 ——
  显示相对年龄（5m / 3h / 2d）就用它，**别用 `last.id` 假装时间**，也别拿文件 mtime 冒充
  （拷贝一份盘会让每条会话都变成「刚刚」）。

- `parent != null` = 这是一条 **fork 分支**。bjes6b 手机端把它全滤掉，只认主线。
  桌面端要不要显示分支是产品决定，别在客户端里猜 —— 真要做 fork 走 `POST /v1/sessions/:id/fork`。
- `last` 是最后一条 entry 的摘要（`{id, kind, text}`），用来画列表预览。它可能为 null。
- `skills` / `template` / `thinkingLevels` / `extensions` 是**生效值**，不是请求值 —— 拿来画勾，
  别自己猜。引擎改这些字段，列表预览不受影响，只有详情面板受影响。

### GET /v1/sessions/:id/entries

历史。返回 `{ items: [{id, kind, text, parent}], next }`，**新的在前**，客户端要翻正。

- `kind` 有 `pi.user` / `pi.assistant` / `pi.system` / `pi.tool-result` 等。桌面端至少处理
  `pi.user`（右气泡）和 `pi.assistant`（左气泡），其余降级显示，别崩。
- `parent` 是 entry 的父指针，和会话树的 `parent` 不是一回事。
- `next` 是分页游标，`?limit=` 上限 1000。

### POST /v1/sessions/:id/messages

body `{ content, requestId?, waitMs?, whenBusy? }`。

- `requestId` 是幂等锚：重连重发同一句拿到同一次提交，不会答两遍。**桌面端一定要传**。
- 不传 `waitMs` 时立即回 `202 { submissionId, status: "queued" }`，真正的流走 SSE。
  桌面端就按这条来：发消息 → 拿 202 → 靠 SSE 收内容。别用 `waitMs` 等同步。

### GET /v1/sessions/:id/events —— SSE

连上先推一帧 `snapshot`，然后是增量事件。**这是桌面的心跳，也是最容易写坏的一层。**

事件类型（`event.type`）：

| type | 我们依赖什么 | 备注 |
|---|---|---|
| `snapshot` | `generation?.message`；`entries[].model[].timestamp` | 重连第一帧。有正在生成的消息就续上，没有就只当「连上了」。**`entries` 是唯一的时间源**：`/v1/entries` 不给时间（引擎的 `activity.json` 只按会话记一个时刻），按天分组目录读这里 |
| `message_start` | `message.role === "assistant"` | user/system 也发，只有 assistant 该开口一个气泡 |
| `message_update` | `changes[]` | 见下 |
| `message_end` | `entry.kind === "pi.assistant"` 时收尾 | 助手气泡结束，把攒的字定稿 |
| `entry_appended` | `entry.kind` | 别的标签页 / 客户端加的东西；`pi.user` `pi.tool-result` 才补 |
| `submission` | `record.status === "unanswered"` 时弹错误 | `record.reason` / `record.detail` 是「没答上来」的原因 |
| `task_failed` | `message` | 任务挂了，弹一句 |
| `tool_execution_start` | `toolCallId` `toolName` | 开一个工具卡片 |
| `tool_execution_update` | `toolCallId` `output` | `output.append` / `output.set` 追加 / 覆盖 |

`message_update` 的 `changes[]` —— **全集来自 pi-durable 的 `MessageChange`**
（`node_modules/@earendil-works/pi-durable/dist/harness/events.d.ts`），不是猜的：

```
{ type: "text_start" | "thinking_start" | "toolcall_start", contentIndex, block }
{ type: "text_delta" | "thinking_delta", contentIndex, delta }
{ type: "toolcall_delta", contentIndex, path, delta }
{ type: "block", contentIndex, block }     → 整块替换，该块后续变化都在这一批里
{ type: "message", message }               → 整条替换，以它为准
```

**只在 `text_delta` 上动字符串是不够的**：块的开头走 `text_start`（`block.text` 已有全文），
跳过它就等于把第一段吞掉。`thinking_*` 是思考不是正文，别往气泡里塞。

其他事件的全集（同一个 d.ts 里的 `AgentEvent`）还有 `run_start` / `run_end` / `turn_start` /
`turn_end` / `inbox_update` / `agent_changed` / `usage_changed` / `auto_retry_start` /
`auto_retry_end` / `deferred_poll` / `tool_execution_end` / `compaction_start` / `compaction_end`。
我们不认识的走 `Other`（`#[serde(other)]`），加事件不该让我们编译失败。

**合帧铁律**：`text_delta` 只改内存里的 `Map<contentIndex, String>`，16ms 定时器读一次再
`cx.notify()`。一个 token 一次 notify 会在 `Entity<T>` 不匹配报错之前，先把 CPU 烧穿。

### POST /v1/sessions/:id/pin

请求体 `{pinned, requestId}`，回执 `{id, pinned, changed, requestId, version}`。
`requestId` 必填（缺了 400，幂等锚）；**重放同一个 `requestId` 回的是第一次的回执** ——
`version` 不涨说明没落第二条提交，但 `changed` 还是那一次的值（不是 false）。
会话不存在 → 404：客户端不许把它当成功（`坑.md` 98/99，`check-desk.sh` 的 `pin.missing.404` 钉着）。

### POST /v1/sessions/:id/abort

返回 `{ status: "aborted" }`。当前这轮生成停下。

### GET /v1/device

`{ self, origin, node, startedAt, model, faux, data, catalog, available }`。

我们依赖：`model`（默认模型）+ `faux`（假 provider 标志，本地验流式时用）。About 面板显示引擎版本用 `node`。

### GET /v1/selection · POST /v1/selection

`{ id }` 或 `{ id: null }` —— 「现在盯着哪个对话」。桌面端三栏同屏时，左栏高亮当前会话就用它，
**别为左栏开 N 条 SSE**。SSE 只连当前会话，其余靠 `GET /v1/sessions` 低频轮询。

### GET /v1/unread

每条会话的未读数，**平铺的 map**，一个会话一个键（**没有 `items` 包一层**）：

```
{ "14": { unread: 3, readAt: 1794212000000 }, "2": { unread: 0, readAt: null }, ... }
```

我们依赖：`unread` `readAt`。

- `unread` 是引擎按「EntryId 水位之后还有几条」数好的，**客户端不自己数**
  （条数会被 fork / compaction 弄错，水位不会）。
- `readAt` 是上次标已读的时刻（epoch ms），可能为 `null` —— 那是引擎种的水位，
  `null` 不是「读到过」，只是一个还没被 `POST /v1/read/:id` 推进过的水位。
- 覆盖**所有**会话；没有水位的会话给 `{ unread: 0, readAt: null }`。
- 跟 `GET /v1/sessions` **一起拉**（同一帧并发），别一会拉一个。

### POST /v1/read/:id

把这条会话标成读到最新，body 空。回 `{ id, readAt, unread: 0 }`。

- 不存在的会话 → 404；只有 `POST`，别的动词 → 405。
- 打开会话时打它；水位只进不退（引擎那边钉住了）。
- **别自己推水位**，也别把「已读」写本地就完事 —— 引擎才是水位的事实源，重装客户端不该丢未读。

### POST /v1/sessions/:id/fork

body `{ entryId?, name? }`。`entryId` 省略 = **从最后一条分叉**。回 201 `{ id, at }`，
`id` 是新会话，`at` 是在父会话的哪条 entry 上分的叉。

- 分出来的会话在列表里带 `parent: {id, at}`。
- 名字不给就沿用引擎的默认名（「对话 44」这种），要区分就自己带 `name`。

### POST /v1/sessions/:id/rename

body `{"name":"…"}`。回 200 `{"id":514,"name":"新的名字"}`。

- **名字是引擎说了算的**：它 trim（`"  改过的名字  "` → `"改过的名字"`），trim 完是空
  就 400 `{"error":"name 是空的"}`。回包里的 `name` 才是生效值，客户端拿它覆盖本地。
- 空名字不能靠「发个空串试试」来判 —— 前端提交前就拦住，不然白跑一趟还弹一句错。
- 改名**没有回执的时间字段**：回包里只有 `{id, name}`，客户端就地改本地那一行的 `name`
  即可（`updatedAt` 该不该跟着动没验过，别在这里编）。

### 会话归档：`archive` / `purge` / 归档库 / `restore`

**这一版引擎起有**：内核 2026-11-10 起（ADR 0001）；客户端 v0.4.0 起接。
归档 = 把会话复印进 `data/archives/gen-YYYYMMDD.sqlite` + 写墓碑，**不是删**；
撕原件 = 真删（活库那行 + 那份复印件）。四个方法在 `crates/nous-engine` 里都有
（`archive` / `purge` / `archive_list` / `restore`），UI 层一行 HTTP 都不碰。

#### POST /v1/sessions/:id/archive

body 空。回 200 `{"gen":"20261113","ids":[699],"rows":17}`。

- `gen` 是 8 位 `YYYYMMDD`，`rows` 是复印进去的条数。**客户端不读回包**（`Value` 收着），
  刷新靠重拉 `/v1/sessions` 和 `/v1/archive` —— 别拿回包自己改本地列表。
- 归档后 `/v1/sessions` **默认不返回**该会话（那一行自己消失）；`?archived=1` 才给全量，
  每条带 `archived` 墓碑。「归档后行消失」只有这一个来源，客户端不许自己滤。
- 失败：忙 / 分支里还有活后代 → 409 `{error}`，正文会列出活分支。原样显示，别改写。

#### POST /v1/sessions/:id/purge

body 空。回 200 `{"purged":[699],"gen":"20261113","rows":6}`。**真删**，不可撤销。

- **必须先归档**；会话闲置、分支里没有活后代也是引擎的前置条件。不满足返回 409 / 400，
  形状还是 `{error}`，照原文显示。

#### GET /v1/archive

回 `{"gens":[{"gen":"YYYYMMDD","items":[…]}]}`，一个 `gen` = 一份 `gen-YYYYMMDD.sqlite`，
组标题就写 `11月10日 · gen-20261110.sqlite`。item 形状：

```
{ id, parent: {id, at} | null, entries: number, last: {id, kind, text} | null,
  name, archived: null | {at, gen, purged}, live: bool }
```

我们依赖：`id` `name` `archived` `live` `parent` `entries` `last`。

- `archived` 为 `null` ⇔ **没有墓碑**（捞回之后被抹掉）；有对象才谈「归档过 / 撕过」。
- `live` 是「活库里这行还在」，**引擎算好的**，客户端不推。
- 四态判据（客户端依赖它，照面板页 `mcfpqm` 的 `stateOf`；四态在真引擎上都实测出现过）：

| 判据（按顺序判） | 态 | 界面 | 给什么动作 |
|---|---|---|---|
| `archived.purged` | 已销毁 | 「已销毁」 | 打捞 |
| `live && archived` | 原件在 | 「原件在」 | 撕原件 |
| `live && !archived` | 已捞回 | 「已捞回」 | 无 |
| 其余 | 无墓碑 | 「无墓碑」 | 打捞 |

`purged` 必须**先判**：已销毁那条 `live` 也是 false 且 `archived` 也在，顺序反了会推成「无墓碑」。
四态**只看 item 自己**，别拿「id 在不在主列表里」当判据 —— 两份判据一定会分叉。

#### POST /v1/archive/:id/restore

body `{"gen":"YYYYMMDD"}`（哪一份复印件，取自 `GET /v1/archive`）。
回 200 `{"restored":[699],"gen":"20261113","rows":17}`。

- 捞回来之后 `/v1/sessions` 里那条**真的回来了**；`/v1/archive` 里它的 `archived` 变 `null`、
  `live` 仍是 true（「已捞回」）。
- **对「原件还在」（`here`）的会话返回 500，不是 409**：
  `{"error":"会话 699 已经在活库里 —— 不用捞"}`。所以 UI 只在「已销毁 / 无墓碑」上给「打捞」——
  这个约束是引擎强制的，不是产品挑的。
- 四个动作的回包都是同一形状 `{<动作过去分词>: [ids], gen, rows}`，客户端一律不读。

### GET /v1/presets/:section

模板（通讯录）那一页的数据。**段（section）不是只有一个**：同一个私仓按段放，
`Session-Template`（会话模板）和 `Ultimate-Template`（终极模板）各自一套端点、一套数据、
**一套类别词表**（`categories.json` 住在段目录里）。

返回 `{ section, title, repo, presets: [...], archived, categories, ... }`。

每条 preset：

```
{ name, summary, description, category, spec, specError, tracked, dir, resolved }
```

我们依赖：`name` `summary` `category` `spec` `specError` `tracked` `resolved`
\+ 顶层的 `categories` `repo`。

- `categories` 是**这一段的全部类别路径**，引擎已排好序（父在子前、逐段 `localeCompare('zh-CN')`），
  里面**包含零计数的类别**（词表里先建好、还没挂东西的）—— 客户端不许滤掉。
- `category` 是字符串数组，`/` 表示层级：`写作/周报` 自动蕴含父级 `写作`。一条可以挂多个。
- `repo` 是**这一段的**仓状态 `{ head, dirty, ahead, behind, error }`；`dirty` 按段算，
  `ahead`/`behind` 是整仓的。`head` 可能是 `null`（仓读不了）—— 别拿它当字符串硬切。
- `specError` → 这条自己写坏了，红。
- `resolved.*.state === "invalid"` → 引用失效，现在开不了。
- `tracked === false` → 盘上有、仓里没。
- **能否启动的判据全是引擎算好的**，客户端不自己拼规则。加了规则就等于可以跟引擎吵起来。
  （唯一客户端自己算的是**类别计数**：引擎不给，算法是「含后代 + 同一条预设按路径去重」。
  那是派生量，不是第二份事实源。）
- 能开的行：`POST /v1/sessions { preset: "Session-Template/<name>", name }`，`name` 带时间。

#### 终极模板那一段是另一种东西（`spec` 里长这样）

```
{ v, name, summary, description, category, trigger, params, session, enabled }
```

- `trigger` 是触发器名字，`params` 是它的参数（对象：字符串 / 数字 / 布尔）。
- `session` 指向 `Session-Template` 段里的一条会话模板。
- **这一段现在只读**：`enabled` 怎么切、`trigger` 用哪个端点启用，协议都还没定。
  桌面端只显示（`⚡ trigger` / `→ session`），点它什么也不发生。要动它先定端点。

#### POST /v1/presets/:section/rename

改一条模板的名字。body `{ name, to }`。

名字 = 目录名 = 主键（形状校验要求 `preset.json` 里的 `name` **等于**目录名），所以
**一次改名引擎同时动三处**，客户端一件都不许自己动手：

1. 目录 —— `git mv`，git 认得出是改名（历史不断）；未发布的（盘上有、HEAD 里没）退回直接搬。
2. 清单里的 `name`。
3. **引用了它的终极模板的 `session`** —— `ultimate.json` 的 `session` 是**名字字符串**，
   不改的话那条终极模板立刻是「会话模板不在 Session-Template 里」，而改名的人不知道去哪修。

回包 `{ ok, commit, pushed, name, to, refs }`，`refs` 是**被动到的终极模板名**（连带改的）。
界面要把 `refs` 说出来 —— 改名顺手动了别人的东西，不说一声就是偷偷改。

- 只有 POST；`to` 走和新建同一套名字规则（`400`）、目标已存在 `409`、源不存在 `404`。
#### GET / POST /v1/presets/:section/order

手动排序。**全局一份**（所有类别共用一套顺序），跟通讯录一样。

`order.json` 住在段目录里，`{ v: 1, order: ["第一条", "第二条", …] }`。

- **它不能写进 `preset.json`**：`PRESET_KEYS` / `ULTIMATE_KEYS` 是白名单，多一个键就报
  「不认识这个键」—— 而它们报得对（拼错的键等于那条设置静默没生效）。
- 是**文件不是目录**：`scan` 和仓里的 `index.mjs` 都只认目录，不会把它当成一条模板。
- `GET` 回 `{ order: [...] }`；`POST` 收 `{ order: [...] }` —— **完整顺序**，不是「谁挪到
  谁前面」。一次落盘、一次提交，服务端不重放移动。
- 回包的 `order` 是**盘上真正存下来的那份**（服务端会把盘上已经不存在的名字滤掉）。
- `GET /presets/:section` 的 `presets[]` **已经按它排好** —— 客户端别自己再排一遍。
  表里没有的条目排在后面、按名字（新模板位置稳定，不会每次刷新换一处）。
- 读不到 / 读坏的 `order.json` 一律当「没有顺序」：排序挂了不该让整段打不开。

- **索引不跟着这次提交走**：`regenIndex` 会把「新建了还没发布」的条目也扫进 `index.json`，
  一起提交就成了「仓说我有它、实际推不上去」。留成 `M index.json`，下次发布收。

---

### GET /v1/models

`{ default: {provider, modelId} | null, items: [...] }`。`items[]` 每条：

```
{ provider, modelId, name, thinkingLevels: string[], contextWindow, maxTokens }
```

- `default` 是**盘上那只眼当前指着的**模型，不是我们的默认值。
- `thinkingLevels` 从少想到多（`off` … `max`），**每个模型不一样**。换模型必须按新模型的表
  重新夹档位（`off` 一定要在表里，没有就退成 `off`）。
- 242 个模型是常态 —— 模型那一列必须能滚，别想着一次画完。

### GET /v1/skills

`{ root, categories: string[], repo, skills: [...] }`。技能那条：

```
{ name, path, description, dir, file, hidden, category: string[], tracked, load: { state } }
```

- **只列 `load.state == "ok" && !hidden`**。`hidden` 是 `disable-model-invocation`，
  勾上也不会动；`state != ok` 的加载不出来。
- `tracked` 是**三态，不是布尔**：`null` = 引擎不查 git（Nous#71 起恒 `null`，
  Nous 自己的 `wire.md` 写的是「可空，当前不提供」）。`null` ≠ `false`，别拿它数「待发布」。
  客户端的 `Skill.tracked` 因此是 `Option<bool>`；写成 `bool` 会让一个 `null` 把整份清单打回 200 报错。
- `categories` 只是词表，和技能身上挂的 `category` 取并集才是栏里该有的那些。

### GET /v1/prompts

`{ default: string | null, categories: string[], prompts: [...] }`。模板那条至少有
`{ name, description?, category?, load: { state } }`。

- `tracked` 同技能：可空，`null` = 不提供（`Prompt.tracked` 是 `Option<bool>`）。

- 和技能不是一回事：**模板是单值**（每轮都带的那份规则），技能是集合。所以列表里
  模板用单选框、技能用复选框 —— 形状本身在说能选几个。
- 仓库 `settings.json` 的默认值只用来**预勾**，选不选仍然是人的事。

### GET /v1/machines

`{ at, machines: [{ name, user, port, ips: [{addr,label}], addr, note, tags }] }`。

- 没有 `machine` 字段 = **本机**。名字要和创建请求里的 `machine` 对得上。

### GET /v1/fs/dirs?path=

`{ path, dirs: [{ name, path }] }`。**列的是 qrd3wh 那台机器上的目录。**

- 不带 `path` = 引擎给的起点（board 的 map 根）；`path: ""` 是起点，不是「没有路径」。
- 浏览发生在引擎进程里：引擎以后跑在远程机器上，这一页一行都不用改。

### POST /v1/sessions —— 建会话的完整 body

```jsonc
{
  "name": "…",                        // 可省，省了引擎给「对话 N」
  "model": {"provider":"…","modelId":"…"},
  "thinkingLevel": "off|minimal|low|medium|high|xhigh|max",
  "skills": ["code-review", "…"],     // 可省 = 不挂
  "template": "审查一段改动" | null,   // **总是发**：null 是「明确不要」
  "extensions": ["…"],                // **只能加，不能减**：见下面那条 ⚠
  "cwd": "/opt/Nous/content/new1/map",
  "machine": "j1900"                  // 缺省 = 本机
}
```

- 这几个键是 **camelCase**（`thinkingLevel` / `cwd` / `machine`）。写成 snake_case
  不会报错，只会**悄悄不生效** —— `crates/nous-engine/tests/parse.rs` 里有条测试钉着它。
- `template` 省掉 ≠ 不要：省掉服务端会用仓库默认值，于是「我明明点了无模板又给我塞回来」。
- 201 回来带 `missing` / `missingExtensions` / `missingTemplates`：引擎那边找不到的东西，
  客户端要如实说，别当成功。
- 建完再 `POST /v1/selection { id }` 把盘上那只眼切过去（切不过去不该把会话判死）。

- ⚠ **`extensions` 只能加，不能减**（2026-11-16 实测，不是推断）：内核
  `createConversation` 的 `settings.extensions` 无条件带
  `coding-tools + nous + skills + prompts`，请求里的 `extensions` 只用来**额外 add**。
  所以 `[]` 不是「一个都不要」而是「就用默认四个」，勾掉核心三条也不生效。
  只有**勾上非核心扩展**真会生效；`missingExtensions` 会如实点名找不到的 id。
  **别把空数组当「清空」写进文档或 UI** —— 桌面端 `ExtKind::Core` 那几行能取消勾选是
  一个「界面说能做、引擎做不到」的假动作，详见 `decisions.md` 那条。
  反悔点（要真支持减）：改 qrd3wh 的 `settings.extensions`，让它按请求清单来；
  改完回来把这句话和上面那段 body 注释一起改掉。

- **「复制设置」抄不了扩展**：`GET /v1/sessions` 的每一项里**没有**这条会话生效的扩展清单
  （只有建会话的 201 响应 `CreatedSession.extensions` 才有）。所以复制时只能省掉
  `extensions` 这个键 = 引擎给默认（全开），跟原来那条可能不一样。要补上得先让会话对象带上它。


## 引擎改了会怎么坏（按我们依赖的字段）

| 引擎改动 | 我们会怎样 |
|---|---|
| 删 `busy` / 换成布尔 | 列表忙闲角标消失或全红 |
| 删 `last` / 换形状 | 列表预览空了 |
| `parent` 语义变 | fork 过滤逻辑失效 |
| 删 `updatedAt` / 单位换成秒 | 列表时间全空，或者每条都显示成 1970 |
| `categories` 里把零计数的类别滤掉 | 通讯录里「建了但还没挂东西」的类别消失 |
| `category` 的 `/` 改成另一种分隔符 | 层级树断成一层，父级筛选丢失整枝 |
| `repo` 整块没了 / `head` 变必填 | 段脚注显示「仓状态未知」，最坏是解析失败 |
| 终极模板的 `trigger` / `session` 改名 | 那一段的 chip 没了（只是少显示，不报错） |
| `/v1/unread` 改回 `{items:[]}` / 改成数组 | 未读角标全灭（形状对不上） |
| `unread` 改成客户端自己数 | 水位退化成条数，fork 之后未读数永久错位 |
| `POST /v1/read/:id` 语义变（不再推到最新） | 未读点了不消，或者一次点掉不该消的 |
| SSE 事件改名 / `changes[]` 换结构 | 气泡不更新，流式死掉，症状是「能发出去但一个字不出来」 |
| `requestId` 幂等被去掉 | 重发会答两遍 |
| `/v1/selection` 语义变 | 三栏高亮错位 |
| `archive` 不再从 `/v1/sessions` 藏掉 | 归档后那一行不消失，看着像没成功（我们不在本地滤） |
| `archived` 墓碑字段没了 / 恒为 `null` | 四态全退化成「无墓碑」，「原件在」和「已销毁」都认不出来 |
| `archived.purged` 改名 / 换语义 | 撕过的显示成「原件在」，会让人重复撕 |
| `live` 没了 / 恒为 true | 「已捞回」和「无墓碑」颠倒，该给「打捞」的不给 |
| `restore` 收 `gen` 改成收别的 | 打捞 400，归档库页点了没反应 |

所以：启动时打 `GET /v1/device` 记引擎版本，About 里显示，出问题先对版本。

---

## 旧面板取数：已删（Desk#13）

数据监控室前五屏原来走旧面板的四五个只读接口，那个客户端和随附快照夹具已整文件删除。
现在这些屏按 capabilities 灰掉并给理由；`Desk#12` 已接上 `/v1/stats/*`，数据原样取、不在客户端重算。
归档屏仍走 `GET /v1/archive`（未实现，灰 + 理由）。
2026-11-21 起 Nous#67 已实现，归档屏反向为真拉；见 `decisions.md` 的相关条目。

## documents 一族（12 条，Desk#16 甲案）

`GET /v1/documents/{kind}` 列出该 kind 的文档（引擎按 kind 定 section）；
其余端点以 `{kind}/{section}/{name}` 为路径段，**客户端不解释 kind 的业务含义**。
全部经 `crates/nous-engine/src/documents.rs` 的 `DocumentClient` 转发，
自建 client 一律 `with_token`（走 `crate::auth`），未知字段原样收在 `Value` 里透传。

| 方法 | 路径 | 我们依赖的字段 / 用法 |
|---|---|---|
| `GET` | `/v1/documents/{kind}` | `{kind, section, items[], resolved}`；列表条数 > 0 即判据 2 |
| `GET` | `/v1/documents/{kind}/{section}/{name}` | `{kind, section, name, fields, etag}`；`etag` 是写回的乐观锁 |
| `PUT` | `/v1/documents/{kind}/{section}/{name}` | body = `{fields, …}`；`If-Match` **走 header**。409 = 别人先改了，回包 `etag` 是磁盘版本；客户端把 409 + 磁盘版本原样交 UI 显示 |
| `POST` | `/v1/documents/{kind}/{section}` | body 同 PUT；`201` + `{kind, section, name, etag}` |
| `POST` | `…/{name}/rename` | `{to}` → `{ok, refs, …}` |
| `POST` | `…/{name}/publish` | 通用 action，引擎只在字段上记 `action` |
| `POST` | `…/{name}/reload` | 同上 |
| `GET` | `…/{name}/validate` | `{ok, errors[], warnings[], resolved}` |
| `GET` | `…/{name}/history` | `{kind, section, name, commits[]}`；`commits[].hash` = `r{seq}`，`at/author` 可为 `null`（不是 git sha）。喂进现有提交历史面板 |
| `GET` | `…/{name}/history/diff?hash=&path=` | `{kind, section, name, path, before, after}`；两份**全文 JSON**（不是 git patch，不是文本）。`before=null` = 那边没有这个文件。喂进现有行级 diff 面板 |
| `GET` | `…/{name}/pending` | `{kind, section, name, head, dirty, units[], aheadCommits[]}`；喂进现有待提交面板 |
| `GET` | `…/{name}/pending/diff` | 同 history/diff 的形状，比的是最新 base 之后叠的 delta |

**形状适配**：引擎回的 `before/after` 是 JSON 对象；`documents_file_diff()` 用
`serde_json::to_string_pretty` 把它变成文本再画（行级 diff 面板只认文本）。
`history` 的 `commits[]` / `pending` 整包直接 `from_value` 进现有 `Commit` / `Pending`，
未知字段仍由它们各自的 `extra` 收着。

> 这 12 条**不在**「合同外调用」名单里：本节就是它们的字段合同。

## 合同外调用（客户端在用，本文件没写）

**27 条 + 资源/触发器写回族**。来源是**真引擎的行为**，不是某份文档 —— 它们整包转发
（`serde_json::Value` 进、`Value` 出），所以没进「我们依赖的字段」清单。**形状以
`crates/nous-engine/src/types.rs` 为准**（那是客户端真实期望），要改这里先改那份类型；
资源写的请求体在 `crates/nous-engine/src/resource.rs`（`NewSkill` / `NewPrompt` /
`NewExtension` / `SaveFile`），触发器写在 `crates/nous-engine/src/triggers.rs`
（`NewTrigger` / `TriggerPublish` / `TriggerMutation`）。这些端点在 `dead_paths.rs` 里
都标 `等票`，引擎未声明前 UI 灰掉、不发请求。

### 归档（2）

| 方法 | 路径 |
|---|---|
| `GET` | `/v1/archive` |
| `POST` | `/v1/archive/:id/restore` |

### 会话配置 / 归档 / 撕原件（3）

| 方法 | 路径 |
|---|---|
| `POST` | `/v1/sessions/:id/archive` |
| `POST` | `/v1/sessions/:id/purge` |
| `POST` | `/v1/sessions/:id/config` |

### 预设端点（TS 内核同形，Nous#60 恢复）

`Desk#11` 曾把这一族收窄成 12 条并丢掉 `resolved` / 扁平 `spec`，导致模板页画不出关系图、
「开一条新对话」对引用失效的模板也照亮。2026-11-22 按 TS 内核（`presets.js` +
`presets-api.js`）恢复，判定由引擎的 `nous-presets-admin` 给，客户端只消费。

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/v1/presets` | 段清单：`{ root, sections: [{ id, title, count, manifest, checker }] }` |
| `GET` | `/v1/presets/:section` | overview：`{ section, title, root, repo, presets, archived, categories }` |
| `GET` | `/v1/presets/model` | 能引用的模型清单 |
| `GET` | `/v1/presets/:section/order` | 盘上的排序表 |
| `PUT` | `/v1/presets/:section/order` | 整段顺序 |
| `GET` | `/v1/presets/:section/categories` | 类别名册（词表 ∪ 挂载，父在子前） |
| `POST` | `/v1/presets/:section/categories` | 词表增 / 删 / 改名 `{ op, name, to? }` |
| `POST` | `/v1/presets/:section/category` | 改一条预设挂着的类别 `{ name, category }` |
| `POST` | `/v1/presets/:section/create` | 起手式：`spec` / `from: "device"` / 空白 |
| `POST` | `/v1/presets/:section/rename` | 改名（目录 + 清单 `name` + 引用它的终极模板 `session`） |
| `POST` | `/v1/presets/:section/archive` | 搬进 `<段>/.archive/<名>/`，body `{ name }` |
| `POST` | `/v1/presets/:section/restore` | 搬回，body `{ name }` |
| `POST` | `/v1/presets/:section/purge` | 彻底删除归档副本，body `{ name }` |
| `GET` | `/v1/presets/:section/validate` | 只读校验（`?name=`），回 `{ ok, errors, warnings, spec, resolved }` |
| `POST` | `/v1/presets/:section/validate` | 同上（草稿用 `{ spec }`） |
| `GET` | `/v1/presets/:section/resolve` | 只要解析结果 |
| `POST` | `/v1/presets/:section/resolve` | 同上 |
| `GET` | `/v1/presets/:section/files` | 一条预设目录里的文件 |
| `GET` | `/v1/presets/:section/file` | 单文件原文 `{ text, etag }` |
| `PUT` | `/v1/presets/:section/file` | 写回单文件（写清单先过形状闸） |
| `GET` | `/v1/presets/:section/pending` | 仓状态 + 待提交单元 |
| `POST` | `/v1/presets/:section/sync` | 提交这一段 |
| `POST` | `/v1/presets/:section/publish` | 形状闸 → 提交这一段 |
| `POST` | `/v1/presets/:section/reload` | 重扫 |
| `GET` | `/v1/presets/:section/:name` | 单条原文 + ETag |
| `PUT` | `/v1/presets/:section/:name` | 建或覆盖（`If-Match`；`?dry_run=1` 校验不落盘） |
| `PATCH` | `/v1/presets/:section/:name` | `{archived}` / `{name}` / `{category}` / `{order}` |
| `DELETE` | `/v1/presets/:section/:name` | 软删 = 搬进 `.archive/` |
| `POST` | `/v1/presets/:section/:name/copy` | `{name, requestId}` |
| `GET` | `/v1/presets/:section/:name/revisions` | 引擎内修订清单 |
| `GET` | `/v1/presets/:section/:name/revisions/:n` | 某一版原文 |
| `POST` | `/v1/presets/:section/:name/rollback` | `{n, requestId}` |

`presets[]` 的每行带 `name` / `summary` / `description` / `category` / `spec` / `specError` /
`tracked` / `dir` / `resolved` / `version` / `sha` / `order`；`spec` 是**整份扁平清单**
（不再包一层 `spec` 键）。写端点统一带 `requestId`，并发用 `If-Match`（ETag = version）。
归档是**搬目录**（`<段>/<名>` ↔ `<段>/.archive/<名>`），不是 `archived.json`。
发布 / 同步走 `nous-repo`（`POST /v1/repos/presets/commit` 仍是私库 git 面）。
diff 由 Desk 对「当前原文 vs 第 n 版原文」做文本计算（`crates/nous-engine/src/diff.rs`）。
`specError` 非空 = 这条写坏了，不出现在选模板列表（老引擎的 `problem` 字段同样收）。

### 私库写回（15；等票 Nous#75）

`POST /v1/{skills,prompts,extensions}/create`、`GET .../{files,file}`、
`PUT .../file`（body `SaveFile`，带 `base` 做并发闸）、`POST .../publish`
（body `{message?}`），以及三仓 `/{op}` 旧拓扑化石（不可再调用）。
UI 入口在 `resource_write.rs`，形状在 `resource.rs`。

### 触发器写回（10；等票 Nous#80 / Nous#73）

`GET /v1/triggers/{files,file}`、`PUT .../file`、`POST .../create|publish|archive|restore|purge|category|categories`。
请求/回包类型在 `triggers.rs`；`sync` 是同族的历史方法，当前 UI 不调用。
客户端另有一个 `POST /v1/triggers/sync`（`TriggerClient::sync`，`dead_paths.rs` 标灰）：
引擎未声明前入口不发请求，灰理由走 `capability.rs` 原文。

### 统计 / 默认 / 选择 / 文档（客户端在用，字段合同在正文或协议层）

| 方法 | 路径 | 形状来源 |
|---|---|---|
| `GET` | `/v1/stats/{usage,tasks,db}` | `console.rs` 的 `Stats*`；Desk#12 已接，六屏原样画 |
| `PUT` | `/v1/default` | `client.rs` + `settings.rs`；端点级 `capability::endpoint_enabled` 门控 |
| `POST` | `/v1/selection` | `client.rs` + `app.rs`；远端同步不回环 |
| `GET` / `PUT` / `POST` | `/v1/documents/{kind}[/{section}/{name}]` 及其 rename/publish/reload/validate/history/pending | `documents.rs` 的 `DocumentClient`，12 条详见上文「documents 一族」 |
| `POST` | `/v1/triggers` 写族 | 见上一节；`sync` 也在此列 |

这些端点在 `dead_paths.rs` / `ENGINE_ROUTES` 里逐个登记；`GET /v1/_routes` 的夹具是唯一外部证据。

### 其余（6）

| 方法 | 路径 | 备注 |
|---|---|---|
| `GET` | `/v1/combos/{section}` | 组合清单。**段只有一个值：`Ultimate-Template`**（`Session-Template` 的条目是「开一条会话用什么」，不是组合；传它会 `400`）。回 `{ combos: [{ name, trigger, params, session, enabled, problem, version }] }`；`enabled` / `problem` 由引擎算，`version` 是下面 `PATCH` 的 `If-Match` 料 |
| `PATCH` | `/v1/combos/{section}/{name}` | 只翻 `{enabled}`，必带 `If-Match` + `requestId`；回包是**平铺的一条**（不是 `{combo:{…}}`），带新 `version` |
| `GET` | `/v1/extensions` | **不是空清单**（0.1.9 起 `#81` 已解冻）。回 `{ items: [{name, version, kind, category, summary, loaded, pid, state, error}] }`；`kind` 是装载形态（`native` / `legacy-js` / `invalid`），**不是分类**。旧形状（`catalog` / `builtin` / `variants` / `archived`）新引擎不再回，客户端只在 `items` 为空时兜底 |
| `GET` | `/v1/triggers` | 触发器目前只有 trait，没有实现 |
| `POST` | `/v1/selection` | 写当前选择（板 / 机器 / 模型） |
| `GET` | `/v1/fs/dirs` | 列的是 **引擎那台机器**的目录，不是客户端的 |

> 旧的 `GET /v1/combos`（不带段）与 `POST /v1/combos/:name/enable|disable` 在 Nous 0.1.7
> 已取消 —— 前者会 **404**，后者压根没路由。`enabled` 是一个字段，不是两个动作。

### GET /v1/extensions —— 扩展清单（0.1.9 起是运行时观测，不是仓库投影）

```jsonc
{ "items": [ { "name": "nous", "version": "0.1.0",
                "kind": "native",        // native | legacy-js | invalid
                "category": "工具",      // 清单里的，一个字符串（不是数组）
                "summary": "…",
                "loaded": true,           // 清单读进来了并装载过（≠ 进程现在活着）
                "pid": 4821,
                "state": "running",      // running | crashed | disabled | …
                "error": null } ] }
```

- **`kind` 说的是装载形态，不是分类。** 0.1.8 及以前这里塞的是清单的 `category`，
  同一份响应里 `kind` 与 `category` 是同一个值 —— 客户端照它画分类树就会把
  「原生 / 老 JS / 不合法」三件事画成类别。分类看 `category`。
- **`legacy-js` = TS 时代（`qrd3wh`）的 `index.js` + `defineExtension` 插件。**
  新运行时只 `spawn` 原生产物（协议握手 + `artifacts/`），JS 没有解释器；这不是「改
  JSON 就能修」，是**整个装载形态换了一代**。引擎在这类清单的 `error` 里明说「需按
  nous-ext-sdk 重写」，客户端照原话显示，别自己编。
- **没有 `catalog` / `builtin` / `variants` / `archived` 了。** 新清单就是「源码根下一个
  目录一条」：内置三条现在也是盘上目录，归档在 `.archive/`（点开头，扫描排除）。
  客户端只在 `items` 为空（旧引擎）时回退到那七份投影。
- **`core: true` = 核心三条**（`nous` / `skills` / `prompts`）：`uninstall` 会 403。
  以前这条事实只活在 403 的文案里，界面只能等用户点一次才知道。
- **装 / 卸仍是两个端点**：`POST /v1/extensions/{install,uninstall}`（body `{name}`），
  核心三条卸载 403 —— 原话照登。装完 / 卸完要显式喊一次
  `POST /v1/extensions/reload`（`{reloaded:[…]}`）：注册表重装是引擎的动作，
  不喊就是「页面说装着、会话里没有」。
- **类别词表**：`GET /v1/extensions/categories` → `{categories:[…]}`。类别树用它，
  不要从 `items[].category` 凑 —— 一个类别下暂时没条目时，凑出来的树会把它弄丢。
- **变体 / 归档在私库面上**（`/v1/repos/extend/*`，与技能同一套）：
  `GET variants` → `{v, variants:{extend:{<id>:{root,branch}}}}` —— **族名就是私库名**
  （`skills` / `extend`）。Nous 0.1.12 之前这里是 `skill` / `extension`，客户端照路径名
  读 `variants.extend` 永远读不到内核写的那条（页面一片空，像「没有变体」）；
  `GET archive` → `{items:[{name}]}`（`.archive/` 只有名字，没有清单）；
  写面 `POST {archive,restore,purge}`。**注意仓名是 `extend`，不是 `extensions`** ——
  拿文件面的 `extensions` 去拼仓级路径只会 404，而 404 长得像「没有这东西」。
- **管理面**：`create` / `duplicate` / `category` 见 `crates/nous/server/src/ext_admin.rs`。
  Desk 当前只用到 `items`、类别词表、装 / 卸、`reload` 与私库面的归档 / 变体读。
- **`GET /v1/repos/{repo}/archive` 四个仓共用**（`presets` / `skills` / `prompts` / `extend`，
  0.1.10 新增）：返回 `.archive/` 下的目录名。写面（`archive` / `restore` / `purge`）
  一直在，缺的就是这条读 —— 没有它，「归档」等于「再也看不见」。

**规矩**：往这张表里加一条 = 客户端多了一处没有字段合同的调用。要么先在本文件写清形状，
要么确认它是整包转发并且在 `types.rs` 里有类型 —— 两者都没有的话，别加。
