# Permissions 卡 · 六套

打开 `index.html` 挑一套。

## 为什么这张卡只有一个开关

`/v1/permissions` 是 404，`server.js` 里 grep `permission` **零处**。引擎唯一有的权限语义是
`readOnly`（`server.js:3245`）：

```js
if (typeof body.readOnly === "boolean")
  await conversation.configure({ modelTools: body.readOnly ? { remove: WRITE_TOOLS } : null }, ctx)
```

**只读 = 不给会写的工具**。读文件、查目录、回答问题全都在，不是「半残」。

所以这张卡真能表达的就是一件事：这次会话能不能动东西。`CreateSession.readOnly` 客户端早就
定义了一直没发过，这次把它接上就完事。

## 先排除的两条

**不做弹层**：另外三张卡开弹层是因为选项多。这里只有两个选项，开弹层比选择本身还贵。

**不做成设置项**：三张邻居卡都是「一句话说清现在是什么」，这张不能变成「要操作才知道状态」的控件。

## 六套

| # | 做法 | 一句话 |
| --- | --- | --- |
| 00 | 现状 `— / soon` | 对照用 |
| 01 | 两段胶囊 | 可发现性最好，但卡里多一行控件，比邻居「重」 |
| **02** | **值 + 一句解释 + `切换 ›`（照 Model 卡）** | **跟另外三张同一个语法，整卡可点** |
| 03 | 整卡即开关，大字是状态 | 最 Tesla，但可发现性最差 |
| 04 | 底部两 pill（跟 dock 呼应） | 最像一家人，但 pill 干了两件事 |
| 05 | 一个滑块「允许写操作」 | 最省字，但两端都不是「关」 |
| 06 | 一个大词，`切换 ›` 藏在 hover | 静止最干净，可发现性最差 |

**推荐 02**：它是唯一一个既跟邻居同构、又不用人找控件的。06 跟它只差一处 —— 「切换 ›」是
一直挂着还是 hover 才出，这一处就是「干净」和「可发现」的全部差价。

## 定了之后要改的

- `NewChat` 加 `read_only: bool`（默认 `false` = 可写）
- Permissions 卡从 `board_card(..., false, ...)` 的占位改成真卡
- `board_create` 发 `read_only: Some(..)`（`CreateSession.read_only` 已存在，一直没发）
- 卡上那句话两句文案：「能改文件、能跑命令」/「只能读，改不了东西」
