# 资源页 / 扩展段落差清单（0.9.5 = TS 内核时代的完成态 → 现在）

**一句话**：UI 一行没删（`resources.rs` 从 1239 行涨到 1366 行，五档 / 类别树 / 详情 /
确认层 / 装-卸-归档按钮全在），**是数据链路换代后断了**。所以这一刀不是「重做页面」，
是「把数据接回同一套页面」。

对照的完成态是 Desk `0.9.5`（`22dc05e`，TS 内核时代最后一版）。判据一律是「那一版能做的，
现在也能做」。

## 一、旧 `Extensions` 的七个字段 → 新引擎的数据源

| 旧字段（`/v1/extensions` 0.9.x） | 现在从哪来 | 状态 |
|---|---|---|
| `catalog[]`（仓里有什么：name/summary/category/sections/file/entry） | `items[]`（`name/version/kind/category/summary/loaded/pid/state/error`） | ✅ 已接（`feat/ext-items-fields`） |
| `load{name:{state,error}}` | `items[].state` / `items[].error` / `kind=="invalid"` | ✅ 已接 |
| `installed[]` | `items[].loaded` + `state=="running"` | ✅ 已接 |
| `categories[]`（类别词表） | `GET /v1/extensions/categories` | ✅ 已接 |
| `builtin[]`（引擎硬编码 import 的 `coding-tools`） | 新引擎三条核心（`nous`/`skills`/`prompts`）也在源码根里，只是 `uninstall` 会 403 → `items[].core`（0.1.10 新增） | ✅ 已接 |
| `variants{family:[{id,key,dir,branch,installed,load}]}` | `GET /v1/repos/extend/variants`（#76，与技能同一套） | ✅ 已接 |
| `archived[{name,summary,category}]` | `POST /v1/repos/extend/{archive,restore,purge}` 写面 + `GET /v1/repos/{repo}/archive` 读面（0.1.10 新增） | ✅ 已接 |
| `protected[]` / `defaults[]` | 核心三条的 403 文本里；`defaults` 已无意义（会话内 `extensions` 只加不减那条随 TS 内核一起没了） | ➖ 不恢复 |

## 二、内核要补的两件（改现有 `nous-ext` / `nous-server`，不新增 crate）

1. `GET /v1/extensions` 的 `items[]` 补 `core: bool`（核心三条，`uninstall` 会 403）——
   让「内置」档重新有数据源。`trashed: bool` 顺带给出（垃圾站里有同名的可打捞副本，
   `install` 会优先取它），否则「安装」按下去人不知道会发生什么。
2. 新增 `GET /v1/repos/{repo}/archive` → `{ "items": [{ "name": "…" }] }`，读 `.archive/`。
   四个仓共用（`presets` / `skills` / `prompts` / `extend`），写端点早已存在
   （`archive` / `restore` / `purge`），缺的只是「列出来」。同一次在 `meta::ROUTES` 落一行
   —— 那张表是 capabilities / `_routes` 的唯一真相。

## 三、Desk 要接的五条（都是已有端点，一行 HTTP 不加业务）

1. `GET /v1/extensions/categories` —— 类别树词表（`items[].category` 只是每条自带的那一个）。
2. `GET /v1/repos/extend/variants` —— 「变体」档（`<本体>@<id>`）。
3. `GET /v1/repos/extend/archive` —— 「已归档」档（恢复 / 真删按钮早就在，只是没有行可点）。
4. `GET /v1/repos/extend/status` —— 详情页的「出处」三态（已提交 / 有改动 / 未提交），
   与技能 / 提示词同一套。
5. `POST /v1/extensions/reload` —— 装 / 卸之后不再靠「重拉清单」猜，直接让引擎重装注册表
   （原来那句 `立刻重装注册表` 是内核自己做的，现在要显式喊一次）。

五条都接上了；另外顺手修了一条**真 bug**：归档 / 恢复 / 真删以前打 `/v1/{kind}/{op}`
（引擎里没有这条路由，点下去静默 404），现在走仓级 `/v1/repos/{repo}/{op}`。

## 四、验收：0.9.5 能做的，现在也能做

外部判据（`scripts/check-desk.sh` 加 `ext.*` 段，真 `nous-app` faux）：

- 五档齐全：`on` / `bad` / `builtin` / `variant` / `archived` 各至少一条；
- 类别树取词表而不是从条目里瞎凑；
- 「归档 → 已归档档出现 → 恢复 → 回到在册」往返一次；
- 核心三条 `uninstall` 仍是 403，且文案照登引擎原话；
- `items[]` 里 `loaded=false` 的条目**不许静默消失**（这条已有单测，别在重构里丢）。

人工那一眼（真桌面 + j1900）：

- 第 4 个 tab 叫「资源」，三段（内容 / 技能·提示词 / 扩展）都在，段计数和列表一致；
- 扩展段：五档行、类别树、搜索、详情、确认层、（归档 / 恢复 / 真删 / 装 / 卸 / 发布）；
- 旧 JS 插件（5 条）落在「待重写」档，不给装 / 卸按钮，详情照登引擎原话。

## 五、明确不做（避免把清单做胖）

- 不新建 crate：扩展运行时（`nous-ext`）与管理面（`nous-ext` + `server::ext_admin`）都已存在。
- 不恢复 `defaults` / `sections`（会话级「只加不减」随 TS 内核一起作废）；要恢复得先改会话契约。
- 不碰插件源码：`/opt/Nous-extend` 那 5 个 JS 插件要按 `nous-ext-sdk` 重写，是内容仓的活，
  与页面无关，单列一票。
- `create` / `duplicate` / `category`（#84）这一刀不接入口：TS 时代的 Desk 也没有，
  「一样不少」不包含它们；要加另开一票，连带能力表与台账一起动。

## 六、第二刀：动作补齐 + 建会话那张卡（2026-11-22 晚）

**资源页这一刀补的**：

- `重载`（`POST /v1/extensions/reload`）与 `复制`（`POST /v1/extensions/duplicate` → `<name>-copy`）；
- 「编辑 / 发布 / 复制」只给**源码根里真有的档位**：已归档（`.archive/`）、旧 JS（清单不合法）、
  变体（另一棵 worktree）都不给 —— 点下去只会 404，而 404 长得像「没有这个扩展」；
- 装 / 卸文案跟上垃圾站语义：0.1.9 起卸载把目录搬进 `data/extend-trash`，安装优先从那里取回
  （旧文案说「目录还在仓库里」，那是 TS 内核的行为）。

**顺手发现并修掉的（同一个病根，在别的页）**：

- **建会话页的扩展卡与弹层**也在读 `catalog` / `installed` / `builtin` / `defaults` 四份老投影 ——
  新内核不回，于是卡片画成 `0/0`、弹层整块空白（不报错，看着像「没有扩展」）。现在全部改吃
  `items[]`：核心 = `core`、能勾 = `native` + 跑着、装载不上 = `invalid` / `legacy-js`；
  预勾 = 核心 + 能跑的（旧逻辑读 `loadable`，同理为空，等于新建会话默认不带扩展）。
- 树里「已归档」那一格在建会话页去掉了：这页本来也不拉归档区，留着永远是 0。
