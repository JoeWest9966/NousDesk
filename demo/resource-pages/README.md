# demo · 技能 / 提示词 / 扩展（五套）

左栏已经有 **Template**（`Session-Template` / `Ultimate-Template`）和 **触发器**，这三页还没有：
技能、提示词、扩展。三样东西都有完整的仓库 API（下面列了），桌面端一个字节都没接。

这份 demo 只回答一个问题：**左栏怎么放、页面几栏、发布走哪条路。** 外观稿，**不进构建**。

## 怎么看

```
/opt/NousDesk/demo/resource-pages/preview.html
```

单文件，双击就开。左边五个按钮切方案（`#01`～`#05` 也能直接跳），下面是「皮」——
**默认**（微信基线）/ **琥珀**（暗色皮，验证暗色下字色和边线）。

**在窗口里也能真的点**：图标换页、类别筛选、行选中、搜名字、文件行切正文、
「≡ 菜单」、「N 处待发布 ›」打开发布层、详情里的卸载 / 归档（带确认层）。

## 五套是什么

| | 左栏 | 骨架 | 一句话 |
|---|---|---|---|
| **01** | 7 个图标 | 树 + 列表 + 详情 | 照模板页那副骨架再来三遍。每页能说自己的话，代价是三份骨架代码。 |
| **02** | 1 个「资源」图标 | 树 + 列表 + 详情 | 三段并排（技能 / 提示词 / 扩展），共用一套删除 / 发布 / 归档。 |
| **03** | 1 个「资源」图标 | 树 + 列表 + 详情 | 两段（内容 / 扩展）。技能和提示词是同一个模子，扩展不是。 |
| **04** | 7 个图标 | 列表 + 全宽工作台 | 点开一条就进编辑器：文件树 + 行号 + 保存 ≠ 发布 + 发布流水线。 |
| **05** | 不占 | 只占右栏 | `≡` 菜单进，页面顶替聊天区，会话列表不动。改动面最小，但 940px 里塞三栏。 |

每套的「这套想干什么 / 代价」写在文件左栏的说明里，跟这里同一份话。

## 三页各自要说什么（跟模板页不一样的地方）

- **技能**：模型看 `description` **自己决定用不用** —— 所以它是集合，能一起上；
  有 `disable-model-invocation`（藏起来：扫得到、勾得着、模型看不见）；
  有变体树（`code-review@123` 这种，独立一棵，可以加载失败）。
- **提示词**：**单值**。一条会话只带一条，所以列表里是单选。
  `settings.json` 的 `default` 只用来预勾，不是「用户选的那条」。
  正文（`PROMPT.md`）**整段**进系统提示词，摘要（`nous.json`）只给挑的人看。
- **扩展**：五类东西别混成一个列表 —— `catalog`（仓库里）/ `builtin`（引擎自带）/
  `variants`（本机 checkout）/ `archived`（`.archive/`）/ `installed`+`loadable`（名字清单）。
  而且**装 / 卸 / 归档是三件不同的事**：
  - 卸载 = 源码进 `data/extend-trash/` + 记进 `extensions.local.json`，**仓库不动**；
  - 归档 = `git mv` 到 `.archive/` + commit + push，**别人也看不见了**；
  - 核心三条（`nous` / `skills` / `prompts`）卸载 / 归档会被引擎 `403` 挡回来。
  - 会话里「这次带哪些」是 `POST /v1/sessions` 的字段，**跟这一页不是一回事**（已经做完了）。

## 数据是真的（2026-11-16，j1900 上的 qrd3wh）

```
GET /p/new1/qrd3wh/v1/skills       → 6 条（1 条 hidden）、类别 6 个、仓库 dirty、变体 code-review@123 加载失败
GET /p/new1/qrd3wh/v1/prompts      → 5 条（3 条未提交）、default = smart-ass
GET /p/new1/qrd3wh/v1/extensions   → 4 条 catalog + 1 条 builtin + 1 个变体（failed）+ 1 条 archived
GET .../v1/skills/file?name=code-review&path=SKILL.md        → 正文（内嵌）
GET .../v1/prompts/file?name=smart-ass&path=PROMPT.md        → 7.7 KB 正文（内嵌）
GET .../v1/skills/pending  /v1/prompts/pending               → 待发布单元（内嵌）
```

仓库状态也是真的：`/opt/Nous-skills` 和 `/opt/Nous-prompts` 都是 dirty，
`/opt/Nous-extend` 分叉且 `pull` 失败（当前分支没有跟踪信息）—— 这三条直接显示在树脚。

**不真的部分**（都是外观稿该假的）：归档 / 卸载 / 安装 / 设为默认 / 保存 点下去只弹提示；
发布流水线是四步动画；`index.js` 只放了片段；`load` 记录只有 `123` 和 `123@123` 两条真实的。

## 三页要接的接口（都在引擎里，已经能用）

```
/v1/skills     /v1/skills/{categories,category,files,file,mkdir,create,duplicate,
                pending,publish,archive,restore,purge,sync,reload,variant,upstream,sources}
/v1/prompts    /v1/prompts/{categories,category,files,file,create,
                pending,publish,archive,restore,purge,sync,reload}
/v1/extensions /v1/extensions/{categories,category,files,file,create,publish,archive,
                restore,purge,install,uninstall,reload,sync}
```

技能和提示词是同一个模子（`skills-api.js` / `prompts-api.js`，两边头注释都互相点名），
差别只有两处：技能的描述在 frontmatter 里、提示词的摘要在 `nous.json` 里；
提示词有 `default` 这个单值。

`nous-engine` 现在只有三个 GET（`skills()` / `prompts()` / `extensions()`），
类型都在 `crates/nous-engine/src/types.rs`，写接口一个都没有。

## 改 demo

```bash
cd demo/resource-pages
# 改 index.html 或 data.js，然后：
python3 build-preview.py      # 把字体和 data.js 内嵌成 preview.html
```

`index.html` 是「能改的那份」（字体走 `fonts/`，数据走 `data.js`）；
`preview.html` 是发出去的那份（单文件，无外链）。

## 挑完落地（Rust）

1. 定左栏：三个图标 / 一个「资源」图标（几段）/ 收进 `≡`。`Tab` 枚举和 `RAIL` 常量在
   `app.rs`，图标条第 2580 行那一串。
2. `nous-engine` 把 `skills` / `prompts` / `extensions` 的写接口补齐（每个仓一套：
   files / file PUT / create / publish / archive / restore / purge / sync），
   **UI 层一行 HTTP 都不碰**。
3. 页面骨架照挑中的那套；`templates.rs` 的类别树 + 列表 + 仓库脚注可以抄
   （`facet_tree` / `cat_row` / `repo_line` 那三块），别第三次重写。
4. 正文字段要能编辑的话，`editor.rs` 那套（文件树 / 行号 / diff / 发布流水线）已经是现成的；
   变体 03 / 04 就是它加一栏列表。
5. 挑完把「为什么是这套」写进 `decisions.md`，再动 Rust。

## 状态

**已定方案**：一个图标 + 三段（02），段按钮定为 B（药丸 + 脏点）—— 结论见 `decisions.md`
「2026-11-16 · 资源页（技能 / 提示词 / 扩展）定为『一个图标 + 三段』」。
落地稿在 `demo/resource-page`，对照稿在 `demo/ia-compare`；下一步开 `feat/resource-pages` 写 Rust。
