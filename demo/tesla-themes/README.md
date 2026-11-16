# 9 套 Tesla 皮

AGENTS 里写死的目标：默认皮是微信基线，`theme.rs` 还要有 Tesla 的 9 套
（`ModelS` / `Model3` / `Cybertruck` / `Mcu` / `Editorial` / `NightDrive` /
`Energy` / `Plaid` / `NoRail`）。现在一套都没有 —— 这个 demo 是挑的第一步。

**怎么用**：双击 `index.html`，上面 3×3 是九套并排，点任意一个看下面 1:1 大图
（会自动滚过去）。顶部 tab 也能切，`?skin=cybertruck` 这种 URL 参数能直接开某一套。

`00 默认`（微信基线）单独一条放在九宫格下面 —— 它是现状，不是候选，不占格子。
大图下面有「复制这一套的 CSS 变量」，挑完可以直接拿去写 `theme.rs`。

## 九套各想干什么

Tesla 的产品线本身就是九种性格，不是九种颜色。所以每套除了色值，还带三个
× 参数：圆角（`--s-r`）、密度（`--s-d`）、字号（`--s-type`）—— 只换颜色不换
手感，九套会看成一个模子刷漆。

| # | id | 明暗 | 一句话 | 关键差异 |
| --- | --- | --- | --- | --- |
| 01 | `models` | 暗 | 旗舰轿车的石墨面板：近黑底 + 冷银，克制到几乎无色 | 银 accent，唯一不抢眼的暗皮 |
| 02 | `model3` | 浅 | 展厅白：中性灰白 + 近黑 accent，把「少」做成卖点 | accent 是**近黑**，不用彩色 |
| 03 | `cybertruck` | 暗 | 不锈钢原色：钢灰 + **直角** + 一条橙 | `--s-r: 0`，全直角 |
| 04 | `mcu` | 暗 | 中控屏：深蓝黑、大字号大间距，车里边开边点 | `--s-d: 1.25`（最松） |
| 05 | `editorial` | 浅 | 杂志排版：暖纸底 + 砖红，行高宽、字号层级重 | 暖纸 + 砖红，`--s-type: 1.05` |
| 06 | `nightdrive` | 暗 | 夜路仪表：黑 + 低饱和琥珀，不刺眼 | 文字是暖白，不是纯白 |
| 07 | `energy` | 暗 | Powerwall：墨绿黑 + 电光青，能量在流 | 绿底青 accent |
| 08 | `plaid` | 暗 | 性能版：纯黑 + 红灯，对比拉到最大 | `--s-d: 0.92`（最紧），纯黑底 |
| 09 | `norail` | 浅 | 无边框：纯白 + **零分隔线**，全靠留白分区 | `line` 几乎等于底色 |
| 00 | `default` | 浅 | 现状：微信基线（绿色） | 对照组 |

## 挑的时候看什么

1. **能不能一直看**（工作台是开一天的，不是看一眼的）—— 暗皮看 `nightdrive` 和
   `plaid` 差在哪：一个不刺眼、一个很刺激。
2. **accent 用在几处**：九套里 `models` / `model3` / `norail` 的 accent 是**灰/黑**，
   意思是「没有重点色」；其余六套 accent 是彩的。这是个真实的取舍 —— 彩色 accent
   一多就变成色块分区，AGENTS 里禁的那个。
3. **`cybertruck` 的直角**和 `norail` 的无线：这两个最不像微信，得看你能不能接受。

## 它们落地要改什么

每套 = `theme.rs` 里一个 `Palette` 常量（45 个 token）+ 一个 `Skin` 常量
（font / body / line_height / type_scale / radius_scale / density / dark）
+ `ThemeName` 加变体。demo 里只展示 25 个 token，剩下的（`accent_tint`、
`scrim`、`shadow`、`hover`、`disabled`…）落地时按同一套明暗关系补齐。

**选中的几套**才落 Rust —— 九套全落是九份要维护的配色，没有用户要求就不做。
