# demo · 资源页中间列底部 · 仓片（14 套 + 3 个退化态）

一个问题：**资源页中间列（300px 列表列）底部要不要放「这一段是哪个私库」，放成什么样。**

外观稿，**不进构建、不碰 `Cargo.toml`**。

```
/opt/NousDesk/demo/repo-footer/preview.html
```

单文件双击就开（字体 + 真机截图都 base64 内嵌，无外链）。

## 这一版换了语法

第一版是「一行状态字」，像开发者工具的状态栏 —— 太庸俗。这一版按 Tesla 那套重做：

| 用什么 | 代替什么 |
|---|---|
| **hairline 三档浓度**（14% / 28% / 55%） | 分隔线、卡片边框、悬停态 —— 不用彩色 |
| **9px 全大写 + 0.18em 字距** 当标签（`BRANCH` / `LOCAL` / `SKILLS · MAIN`） | 12px 灰字小标题 |
| **JBMono tabular 数字** | `2 处待发布` 这种带单位的散文 |
| **实心点 / 空心点** 表示脏 | 琥珀/绿色 —— MONO 皮里 `warn` / `ok` / `accent` 全是白，只有 `danger` 是红 |
| **动效 120ms**：箭头位移 1px、框亮一档、白块填充 | 缩放、弹跳、位移超过 4px |
| **留白换高度**：最低 40px | 27px 的挤压条 |

骨架、字号仍照真机：rail 60 / 树 248 / 列表 **300** / 详情，行 12.5 / 11 / 行高 16 / padding 7·12；
皮是 `Palette::MONO`（黑底 + 白骨架线），「默认皮（白）」按钮只用来验同一套在白底也成立。

## 14 套

| # | 名字 | 一句话 |
|---|---|---|
| 01 | 铭牌 plate | 9px 大写标签 + 13px 仓名 + 方角箭头，左上 26px 白线当铭牌下划线。最静 |
| 02 | 播放条 now playing | 28px 方框徽标（右下 5px 白点 = 脏）+ 两行 + 26px 圆按钮 |
| 03 | 点阵仪表 dot gauge | 6 个小方块：白 = 已发布、暗 = 未发布；分支在左、仓名在右 |
| 04 | 等宽排印 editorial | 整条 JBMono，下面 8.5px 全大写注释行（owner / 短 sha） |
| 05 | 参数表 spec sheet | 左 8.5px 大写键、右 mono 值，三行 BRANCH / STATE / REMOTE |
| 06 | 刻度尺 gauge | 一排 1px 刻度，白段 = 未发布占比，边界一根长针 |
| 07 | 浮动胶囊 pill | 不贴底，30px 圆角胶囊浮在列表底上方 6px |
| 08 | 角标 badge | 平时只有右下角 20px 方标，悬停升起整条 |
| 09 | 拆角 fold | 右上角 15px 折角（两段短边 + 一道斜线） |
| 10 | 进轨 track | 10 段 3px 细轨，白段 = 未发布 |
| 11 | 层叠 stack | 底卡上露出两张「卡的边」，暗示背后还有仓库历史 / 待发布 |
| 12 | 双侧对置 split | 左本地路径、右远端地址，中间一根 22px 竖线 |
| 13 | 折叠卡 expanded sheet | 收起 28px；展开是把手 + 三行键值（BRANCH / PENDING / REMOTE） |
| 14 | bento 双格 | 左仓名、右 15px 大数字 2 + ↗，中间竖 hairline |

退化态（同一套语法）：

- **A 没有远端** —— 键帽 `⌘C` + 本地路径 + 8.5px `LOCAL ONLY`。不画点不动的 ↗。
- **B 状态读不到** —— 虚线改表格 + 5px 红点 + 引擎原话（9px mono）+ 下划线的 `RETRY`。
- **C 空仓** —— 虚线空方框 + `NO COMMITS YET`。不说 main、不说干净，因为这两个还不存在。

## 数据从哪来

j1900 上逐字核过（2026-11-28）：

```
GET /v1/repos/{skills,prompts,extend}/status
→ {root, head, branch, ahead, behind, dirty, locked}

skills  main                     a2adba9  dirty
prompts main                     5b8c7f5  dirty
extend  feat/skills-upstream-source  07b9e0b  dirty   ← 扩展仓不在 main 上
```

- 待发布条数走清单 `tracked`（客户端已有 `pending_count`），不用另打 `/pending`；
  引擎侧 `units` 数是技能 2 / 提示词 5 / 扩展 7 —— **两处口径要同一个**。
- **`dirty` 是布尔，不是数字**，所以 03 / 06 / 10 / 14 里的「2」都按清单行数算。

### 缺的一块：`remote`

`status` 回包**没有 remote URL**（只有 presets 的 `repo_view` 带）。
所以带 ↗ 的那几套今天没有地址可跳。三条路：

- **A**（内核）`/v1/repos/{repo}/status` 补 `remote` —— `nous_repo::remote_url()` 已写好
  （`crates/Nous/Repo/src/views.rs:133`），只差进回包，但在另一个仓、要走分支。
- **B**（客户端）改成页内仓库工作台（历史 / 待发布 / 上游同步），零内核改动，代价是新做一页。
- **C**（退化）A：只给本地路径 + 复制。

## 我的倾向

**01 铭牌**（最静、最像 Tesla 的铭牌）+ 点击进页内仓库工作台；
「2 处待发布」用 **03 的点阵**塞进铭牌第二行右侧（4px 方块 ×6，白=已发布）。
也就是：`SKILLS · MAIN · ▪▪▫▫▫▫` 一行标签，下面 `Nous-skills`，右边方角箭头。

理由：01 的信息层级最干净（标签 → 值），点阵把「脏」变成可读的数量而不是散文；
13 折叠卡信息最全但最重，适合放到「点开之后」而不是常驻底栏。

## 重新生成

```bash
python3 build-preview.py   # index.html → preview.html（字体 + desk-real.png base64 内嵌）
```

`desk-real.png` 是真机截图（1240×820）。重截见 `AGENTS.md`「headless 机器上看窗口」：
Xvfb + picom + 真产物，`NOUS_WINDOW_SIZE=1240x820`、`NOUS_DESKTOP_OPEN=resources`，
并且 **`env -u WAYLAND_DISPLAY`**（不然 GPUI 走 Wayland，Xvfb 的 root 全黑 —— 这次踩过）。
