# 触发器页 · B 方向抛光（demo，不进构建）

B 方向 = 纯桌面端、不碰引擎。两刀各给两方案，共用真数据形状
（`single-shot` 的 epoch 参数 / `select` 选项 / 改了没保存的文件）。

**预览：双击 `preview.html`**（字体已内嵌，单文件、无需起 server）。

## 第 1 刀：详情页参数卡补 default / options

`TriggerParam` 有 `default / options / help` 字段，向导能填、右侧 JSON 预览里有，
但详情页现在只显示 `name / kind / required / help / title` —— 填了看不到。

- **1A 明细 chip 流**：default / options 当小 chip 挂在卡片下方，紧凑
- **1B 对齐 kv 表**：`默认 / 选项` 当左标签右值排成一条轴，长选项换行也不乱

## 第 2 刀：代码页文件列表做出 IDE 手感

`/files` 已返回 `changed / added / removed`，preflight 里用了，但左边文件列表
只显示字节数；改了没保存的文件也看不出来。

- **2A 最小改**：只加脏圆点 + 沿用 `+N −M`
- **2B tab 手感**：左侧选中竖条 + `+N −M` 右对齐成一列 + 脏圆点

脏圆点 = 编辑态改了没保存（**不是** git dirty）；`+N −M` = 相对 HEAD 的增删。

## 挑哪个

给四个字母组合，如 `1B + 2B`。挑完才写 Rust。
