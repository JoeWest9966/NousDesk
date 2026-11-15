# demo · Template 页右栏（只读层）

第二步（spec `docs/spec-template-page.md` §6）的外观稿，**不进构建**、不接 Cargo。
挑完把结论写进 `decisions.md`，再去改 `crates/nousdesk/src/templates.rs`。

## 怎么看

**发给用户的是 `preview.html`** —— 单文件、字体和数据都内嵌，双击就能开：

```
/opt/NousDesk-tpl/demo/template-page/preview.html
```

改代码时用下面这套（`index.html` + `data.js` + `fonts/`），改完重新生成 `preview.html`：

```bash
cd demo/template-page && python3 -m http.server 8765   # 自己调样式时用
# 重新生成单文件（脚本见 git log，逻辑：字体转 base64 + data.js 内联）
```

右上角三组按钮：**详情排版 A1/A2/A3**、**变化视图 B1/B2/B3**、**浅/深**。
左屏 = 详情，右屏 = 待同步 / diff，两屏组合着挑。也可以直接跳：

```
http://localhost:8765/#a2,b3,dark
```


## 四套 Tesla 方向（skins）

```
/opt/NousDesk-tpl/demo/template-page/skins-preview.html
```

同一份数据、同一个右栏（620px），四套完全不同的皮肤，用左上角切换：

| | 名字 | 想做什么 |
| --- | --- | --- |
| A | MCU | 纯黑车机屏：全直角、1px 细线、白色只留给值和动作 |
| B | Site | 特斯拉官网：白底细线、两列网格、字重最轻、靠留白分组 |
| C | Steel | Cybertruck：冷灰 + 2px 结构线、状态做成块 |
| D | Data | 仪表盘：密度最高、等宽数字、表格化 |

源码 `skins.html`（改样式改这份），`skins-preview.html` 是生成出来的自包含单文件（发给用户的就是它）。
旧的那套（A1/A2/A3 + B1/B2/B3）在 `preview.html`，留着当对照，不再改。


## 完整原型（app.html）—— 12 套排版 × 6 皮肤

```
/opt/NousDesk-tpl/demo/template-page/app-preview.html
```

**重点是排版，不是颜色。** 右栏有 12 套结构完全不同的排法，切换在左上角：

| | 排版 | 结构 |
| --- | --- | --- |
| 1 | 纵向流 | 单列，标签左值右，留白分隔 |
| 2 | 表格式 | 标签列 + 值列，行高紧，左对齐 |
| 3 | 卡片网格 | 2×N 小卡，值换行 |
| 4 | 巨碑 | 52px 名字 + 大片留白，三格数据压底 |
| 5 | 铭牌 | 竖排书脊 + 001–009 编号表 + 2px 黑带 |
| 6 | 对开 | 上半反色块（身份）+ 下半双列（数据） |
| 7 | 圆环 | 270° 仪表弧 + 刻度，引用数当指针 |
| 8 | 三栏 | 报刊式三列，字段沿着列往下流 |
| 9 | 轴线 | 左侧一条竖线 + 圆点，字段挂在轴线旁 |
| 10 | 折叠 | 配置/运行/引用三组，点开折叠 |
| 11 | 检视栏 | 左宽右窄：左边名字摘要，右边一列规格 |
| 12 | 极密 | 双列 22px 行高，一屏 8 个字段 |

功能全都能点：两段切换、类别树（层级/零计数）、搜索、列表/网格、卡片选中、
详情/盘面/JSON/对比 四个 tab、发布（文件清单）、归档、彻底删除（不可逆二次确认）、
开对话（会话名 + 生效值）、新建（整页表单 + 名字实时校验）、新建类别、⋯ 菜单、Esc 关弹层。


### 13–20 · 图表式排版（pele.run / Mermaid 灵感）

pele.run/examples 把每种 Mermaid 图都做成一页，配色是 **Flexoki**（暖黑 #100f0f / 纸白 #fffcf0 /
一套柔和色），字体 Inter Variable，正文大、行高松。照这两点加了两套皮肤 + 8 套「用图的形状排信息」的排版：

| | 排版 | 借的图表 |
| --- | --- | --- |
| 13 | 看板 | Kanban：身份 / 运行 / 环境 / 引用 四列 |
| 14 | 矩形树 | Treemap：按块分配面积，模型占最大 |
| 15 | 甘特 | Gantt：每字段一条横条，长度=状态，轴是 HEAD→发布 |
| 16 | 四象限 | Quadrant：重要×已定 的四格 |
| 17 | 导图 | Mindmap：中心节点 + 贝塞尔分支 |
| 18 | 雷达 | Radar：六维（有值 1 / 缺失 0.32） |
| 19 | 流图 | Sankey：预设 → 各引用，线宽表权重 |
| 20 | 状态机 | State：草稿 → 已提交 → 已发布 → 归档，当前步高亮 |

皮肤新增 `flexoki`（暗）/ `flexoki-paper`（亮），色值就是 pele 那套 Flexoki。
hash 直达：`#flexoki,detail,,st,sankey` / `#flexoki-paper,detail,,ut,radar`


### 21–30 · 更多形态

| | 排版 | 长什么样 |
| --- | --- | --- |
| 21 | 小票 | 等宽 + 点线引导 + 合计行 + 条形码，像热敏收据 |
| 22 | 命令面板 | 搜索框 + 行列表 + 快捷键提示（⌘K / ↵） |
| 23 | 仪表盘 | 3×2 小部件，数值大字 + 迷你条/圆点 |
| 24 | 时序 | 四个参与者 + 虚线生命线 + 彩色消息箭头（Mermaid sequence） |
| 25 | 旅程 | 情绪折线 + 五个阶段（建模→校验→保存→发布→开对话） |
| 26 | 泳道 | 横四条泳道（身份/运行/环境/引用），左竖排标签 |
| 27 | 鱼骨 | 中心骨架 + 四条斜骨挂字段（Ishikawa） |
| 28 | 数据包 | 十六进制的字节格，字段按 48 字节一行铺开 |
| 29 | 头版 | 报纸：双线报头 + 34px 标题 + 三栏 + 引文块 |
| 30 | 翻面卡 | 正面身份、背面字段，点「翻面」3D 翻转 |


### 31–40 · 场景式

| | 排版 | 长什么样 |
| --- | --- | --- |
| 31 | 终端 | 黑底 CLI：`$ preset show` 输出 ASCII 框 + ✓/✗ 引用检查 + `git status` |
| 32 | 代码仓 | 左文件树 + 右 JSON 语法高亮，行号、`M` 标记 |
| 33 | 对话 | 把字段变问答：我「模型」/ 它「volc-ark」，末尾打字动画 |
| 34 | 工单 | `#1234` + 状态徽章 + 右侧属性栏 + 三条评论 |
| 35 | 专辑 | 渐变封面 + 曲目列表（字段=曲目，带时长） |
| 36 | 模块卡 | 2×2 圆角卡，主卡渐变、大数字 40px、迷你柱 |
| 37 | 节点图 | 中心节点 + 放射连线 + 两层虚线环 |
| 38 | 电子表 | 行号 + 列头 + 可选中单元格（蓝框） |
| 39 | 幻灯片 | 36px 标题 + 左右箭头 + 圆点导航，**可翻 3 页** |
| 40 | 播放器 | 240px 封面 + 波形 + 进度条 + 控制键 |

hash 直达：`#皮肤,tab,弹层,段,排版`
`#graphite,detail,,st,sheet` `#paper,detail,,ut,dense` `#industrial,detail,publish,st,split`

6 套皮肤只影响颜色与线型（石墨/纸/单色/冷钢/琥珀/工业），用来验证同一套排版在不同明暗下站不站得住。

`app.html` 是源码，`app-preview.html` 是生成的自包含单文件。


## 数据是真的

| 文件 | 来源 |
| --- | --- |
| `st.json` / `ut.json` | `GET <引擎>/v1/presets/Session-Template`、`/Ultimate-Template` |
| `triggers.json` | `GET <引擎>/v1/triggers` |
| `pending-st.json` | `GET <引擎>/v1/presets/Session-Template/pending` |
| `diff-deep-dive.json` / `diff-categories.json` | `GET …/pending/diff?path=…` |

只有 `data.js` 里 `diffs.sample` 是**合成**的（真 diff 太短，看不出长变更的排版），
按钮上标了「看更长的样例」。

刷新数据：

```bash
ENG=http://192.168.1.7:4790/p/new1/qrd3wh
curl -s "$ENG/v1/presets/Session-Template" -o st.json
curl -s "$ENG/v1/presets/Session-Template/pending" -o pending-st.json
# 然后重新生成 data.js（见 git log 里那条一次性脚本）
```

## 三套详情各想干什么

- **A1 纵向流**：一行一项、留白分隔。扫得慢，但字段再多也不挤；最接近现在右栏的演进。
- **A2 表格式**：左标签右值、行高更紧。字段 10 个以上时一屏放得下。
- **A3 卡片网格**：盘面 2×4 小格，一眼看全；长值要换行，字段再多会变两屏。

## 三套变化视图

- **B1 统一 diff**：单栏 + 行号 + 淡底色，最省地方，窄栏首选。
- **B2 并排 diff**：左右两栏，改得多时好读，但 620px 里每栏只剩 300px。
- **B3 结构 diff**：先列「哪个字段变了」，答案是给人的（`thinkingLevel: medium → high`），
  要逐行看格式变化再切 B1/B2。
