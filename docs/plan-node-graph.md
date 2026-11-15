# 落地 37 · 节点图（Template 页右栏关系图）

> 2026-11-14 选定。demo 见 `demo/template-page/app-preview.html` → 排版 37。
> 这份是**施工方案**，不是验收口径；验收口径仍是 `docs/spec-template-page.md`。

## 一句话

Template 页从两栏变三栏：选中一条预设，右栏画出它的**关系图** ——
中心是这条预设，一圈是它钉住的维度（模型 / 思考档 / 技能 / 扩展 / 模板 / 目录 / 机器 / 只读），
每个节点的颜色就是引擎给的 `resolved.*.state`。点节点看细节。

## 为什么是它

不是因为它好看，因为**它是唯一一张图能同时回答三个问题**：「这条钉了什么」「哪些走默认」
「哪条引用现在坏了」。列表三行说不清，JSON 面板看得懂但看不懂。

数据天然是图：`Preset.resolved` 里每个维度自带 `state`（`ok` / `auto` / `invalid`）
和 `ref`/`value`，客户端只需要画，不需要算 —— 符合「协议是哑的」。

## 图长什么样

```
                    ┌─ 扩展 ─┐
        ┌─ 技能 ─┐  │  nous  │  ┌─ 模板 ─┐
        │deep-dive│ └────────┘  │smart-ass│
        └────────┘             └────────┘
                 ╲    ╭──────╮    ╱
                   ───┤deep- ├───        ← 中心：名字 + 摘要首行
                 ╱    │ dive │    ╲
        ┌─ 模型 ─┐   ╰──────╯   ┌─ 目录 ─┐
        │volc-ark│              │  默认  │
        │  ╱flash│              └────────┘
        └────────┘      ┌─ 机器 ─┐
                        │  本机  │
                        └────────┘
```

- **中心**：`preset.name`（16–18px）+ 下面一行状态（能开 / 开不了的原因）。
- **第一圈**：7 个固定维度，角度均分，从 12 点开始顺时针：
  `模型 → 思考 → 技能 → 扩展 → 模板 → 目录 → 机器`（只读作为中心下方的徽章，不占角度）。
- **第二圈**：`skills[]` / `extensions[]` 有几个就挂几个小节点，用短线连到父节点 ——
  第一圈是「维度」，第二圈是「这一维里具体的那个东西」。
- **线的颜色 = 子节点的 state**：`ok` 用 `line`，`auto` 虚线（走默认），`invalid` 用 `danger`。
- 节点只用两种形状：中心圆角矩形、外圈圆角小卡。**不画图标** —— 图标在这层是噪音。

## 节点的数据来源

| 节点 | 字段 | 显示 | 颜色 |
| --- | --- | --- | --- |
| 模型 | `resolved.model.value` | `volc-ark/deepseek-v4-flash` 折两行 | `state` |
| 思考 | `resolved.thinking.value` | `medium` | `state` |
| 技能 | `resolved.skills[].ref` | 每个一个小子节点 | 各自 `state` |
| 扩展 | `resolved.extensions[].ref` | 同上；空则不画 | 各自 `state` |
| 模板 | `resolved.template.ref` | `smart-ass` / `跟随默认` | `state` |
| 目录 | `resolved.cwd.value` | 路径末段 / `默认` | `state` |
| 机器 | `resolved.machine.value` | 名字 / `本机` | `state` |
| 只读 | `resolved.readOnly` | `只读` / 不画 | — |

- `state == "auto"` 且 `value == null` → 显示「跟随默认」，虚线连。
- `resolved` 是 `null`（引擎没给）→ 整张图退化成纯字段图（照 `spec` 摊），不报错。
- `spec_error.is_some()` → 中心节点描红边 + 一句「spec 写坏了，图不代表实际行为」。
- 未知字段（`spec` 里我们不认识的 key）→ 挂在第二圈，灰字透传，**不丢**。

## 绘制：用 canvas，不用 DOM 摆位

复用 `board.rs` 里已有的套路（`board_canvas` / `paint_path`）：

- **线、环、虚线** → `canvas(...)` + `PathBuilder::stroke(1.0)`，虚线用 `.dash_array(&[3, 3])`。
  理由和 board 那处一样：GPUI 的 `border` 没有 dashed，`rounded(px)` 是绝对半径不是百分比。
- **节点** → 普通 `div`，`absolute()` 按极坐标算 `left/top`，`w(px)` 固定宽度。
  文字别画进 canvas —— 那层不参与布局和 IME。
- 容器固定 `h(px(460.))`，宽度跟右栏走；极坐标半径 = `min(w, h) * 0.34`。
- 角度算完先做一次**最小夹角去重**：第二圈节点多时，相邻角度差小于 18° 就把半径往外推一层。

## 交互

| 动作 | 结果 |
| --- | --- |
| 点列表一行 | **选中**（不再直接开会话），右栏出图 |
| 点图上的节点 | 节点高亮 + 右栏底部一行说明（`ref` / `value` / 为什么 `invalid`） |
| 点中心 | 取消选中，回到空状态 |
| 双击行 / 中心下的「开一条新对话」 | 才真的开会话 |
| 悬停节点 | 描边变 `ink2`，150ms 过渡（唯一动效） |

> ⚠️ 这条改了现有行为：现在是「点行 = 直接开对话」。改成两段之后**多一次点击**。
> 备选：行尾保留一个「↗」按钮直接开，点行体只选中。**推荐这个**（微信里点联系人是看资料，
> 不是发消息）。见下面「要你拍板」。

## 代码落点

`crates/nousdesk/src/templates.rs`（+ 新增 `templates_graph.rs`，别再把一个文件撑到 1600 行）

1. `struct Desktop` 加 `selected: Option<String>`（预设名）。
2. `preset_row` 的 `on_click` 从 `launch_preset` 改成 `select_preset`；另加行尾 `↗` 才开。
3. 新增 `render_detail(&self, cx)` → `AnyElement`，三栏里第三栏，`w(px(theme.detail_w))`。
4. 新增 `templates_graph.rs`：
   - `pub fn nodes(p: &Preset) -> Vec<Node>` —— 把 `resolved`/`spec` 摊成 `Node { label, value, state, layer, angle }`，**纯函数，可单测**。
   - `fn layout(nodes: &[Node], w: f32, h: f32) -> Vec<(Pixels, Pixels)>` —— 极坐标 + 去重，同样可单测。
   - `fn paint(...)` —— canvas 里的线。
5. 主题加 `detail_w: f32`（8 个皮肤各一份，默认 `380.`）。

单测先写这三个（`nodes` 的 auto/invalid/未知字段、`layout` 的角度不重叠），再接 UI ——
这块逻辑最容易在「引擎多想了一个字段」时悄悄画错。

## 分三刀交付

| 刀 | 内容 | 验 |
| --- | --- | --- |
| ① 静态图 | 三栏 + 选中 + 图能画出来（无交互、无动效） | `cargo run` 点一条预设，截图对照 demo 37 |
| ② 交互 | 点节点高亮 + 说明行 + `↗` 直接开 + 空状态 | 点穿每条分支，`invalid` 的预设看线是否变红 |
| ③ 手感 | 150ms 悬停过渡 + 选中行的左侧竖条 + 加载骨架 | 录 10 秒看有没有跳帧；`cargo clippy` |

每刀一个 commit，都跑 `cargo test` + `cargo clippy`。

## 要你拍板的两件事

1. **点行是选中还是直接开？** 我倾向：点行=选中（看图），行尾 `↗`=开。
   代价是老用户肌肉记忆要改一次。
2. **右栏宽度 380 够不够？** 图在 380 宽里第一圈节点会挤；要么 380 + 节点叠两层，
   要么把列表从 340 收到 300 给图腾 420。**我倾向 300 + 420。**

## 不做的

- 不做拖拽节点、不做缩放平移（这是一张「看一眼就走」的图，不是白板）。
- 不做 force-directed 布局（每次刷新位置都在跳 = 找不到东西）。
- 不画「预设之间的引用关系」（引用是技能/模板/模型给的，不是预设互引）。
- 不改协议：`resolved` 加字段这边自动多画一个节点。
