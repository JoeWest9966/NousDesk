# 资源页 IA 对照 · 02 vs 03

`demo/resource-pages` 五套里先挑了 **02**（三段并排），但另一条线 `feat/resource-pages`
落了 **03**（两段 + 子开关）的决定并写了一半 Rust。两套打架，谁合 main 都会撞。
这份 demo **不比好不好看，只把 IA 差别摆出来**，看过再拍板。

## 怎么看

```
/opt/NousDesk-respages/demo/ia-compare/preview.html
```

单文件，双击就开。上面一个开关切 **02 · 三段并排** / **03 · 两段 + 子开关**（或按 `1` / `2`）。
**窗口尺寸、数据、样式、列表、详情完全一样，只换树顶那一段分段器。** 差别处画了绿色虚线圈。

右边一栏是两种走法的账：到技能/提示词要点几次、段按钮长什么样、各自的好处和代价。

## 唯一差别：树顶分段器

- **02**：`技能 | 提示词 | 扩展` 三段平级 → 到任何一段点 1 次。
- **03**：`内容 | 扩展` 两段；选「内容」再露出 `技能 | 提示词` 子开关 → 到技能/提示词点 2 次，
  到扩展点 1 次。

数据层完全一样：三个仓（skills / prompts / extensions）的写端点不因 IA 改变。
差别只在 UI 骨架和段切换 —— 定了我才能接接口。

## 两条线的原文

- **02**（`demo/resource-page`，本 worktree）：`decisions.md` 2026-11-16「资源页定为『一个图标 + 三段』」+
  段按钮 B。理由：三类资料平级、各自的脏点一眼看清谁坏；代价是三段看不出「技能和提示词其实一回事」。
- **03**（`/opt/NousDesk-res`，`feat/resource-pages`）：`decisions.md` 2026-11-16
  「资源三页选 03：一个「资源」图标，两段（内容 / 扩展）」。理由：按**数据形状**分——
  技能和提示词是同一个模子（后端两个 api 互相点名），共用一套骨架、代码只写一遍；
  代价是「提示词在内容里」要教一次、到技能/提示词多点一下。

对应的 Rust 现状：`feat/resource-pages` 里 `crates/nousdesk/src/resources.rs` 已写 948 行（03 骨架，只读），
还没接写接口；**本 worktree 没有动那份代码**。

## 数据

跟前两份 demo 同一份真数据（j1900 引擎 2026-11-16）：技能 6 条、提示词 5 条、
扩展 4 条 + 内置 + 变体 + 归档，三个仓各自 dirty 状态。

## 改 demo

```bash
cd demo/ia-compare
python3 build.py            # base.html + 分段器逻辑 → index.html
python3 build-preview.py    # 字体 + data.js 内嵌 → preview.html
```
