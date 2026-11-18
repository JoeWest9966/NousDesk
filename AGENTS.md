# NousDesk

Nous 的**桌面客户端**。GPUI + `gpui-kit` 原生应用，不是网页、不跑 agent。

**一句话**：内核在另一台机器上（`nous-app`），这里只做两件事 —— **皮**（微信的骨架 +
Tesla 的设计语言）和**哑协议层**（`crates/nous-engine`）。一个地址、一个 token、一堆
`/v1/*`，中间没有 resolve、没有 board。

> **平台：只做 Linux**（2026-11-12 起）。开发机是 Debian 13 + GNOME **Wayland**。
> Windows 的源码与打包配置（`windows.yml`、packager 元数据）**原地保留、冻结**：
> 不删、不改、不再出包；要重启 Windows 线，先把这份文档和 `README.md` 的平台段改回来。

> **UI 框架钉死：[longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)**
> （`gpui-kit = "0.9"`，crates.io 同名包，Apache-2.0）。
> 不许换框架、不许绕开它直接用 `gpui-pre-*`、不许把 Tauri / Slint / egui 加进来。
> 组件怎么用一律以那个仓库为准：<https://github.com/longbridge/gpui-kit> · <https://gpui-kit.com>。
> 要换框架先说清楚为什么，写进 `decisions.md`。

## 两台机器、一个地址

```
NousDesk（你的桌面）──HTTP + Bearer token──▶ nous-app（j1900:4789，另一台机器）
     一个地址，来自 desktop.toml           /v1/* · 数据 /var/lib/nous/
```

- 客户端**永远不和引擎同机**，所以全部走 HTTP；引擎绑 `0.0.0.0:4789`，必须带 token。
- 客户端不跑引擎、不是引擎的一部分、不往 j1900 部署。
- 连不上先分三层：地址通不通 → token 对不对（无/错都 401）→ contract 对不对（界面会出
  「版本不匹配」横幅）。探针是 `GET /v1/device`（带 token），没有 `/health`。

## 开发宗旨

**体验好、性能好、灵活性强 —— 不惜增加开发成本。**

- **体验好**：不到 60fps 的动画不上、输入延迟能感觉到就先量再改；按钮点下去没反应、
  等半天没反馈，都算 bug。中文输入法、剪贴板、HiDPI、窗口拖拽缩放是首屏体验的一部分。
- **性能好**：虚拟列表、增量渲染、SSE 合帧、避免每秒重建整棵元素树 —— 宁可多写一层缓存和
  状态机。慢要慢得可解释（有 profiling 数据），不能慢得莫名其妙。
- **灵活性强**：协议层/UI 层分家（`nous-engine` vs `nousdesk`）、配置与常量分开、后端可换。

代价（多写测试、多拆一层、多做一次测量）**算开发成本，不算超支**；反过来，为了省事牺牲
体验/性能/灵活性，要先写进 `decisions.md` 并给出反悔点。

## 设计参考：微信的骨架 + Tesla 的设计语言

**信息架构照微信桌面版，排版 / 字体 / 配色 / 功能设计照 Tesla。** 不是「做个微信皮肤」，
也不是「做个特斯拉风 App」——两个都要，各管一半。

- **骨架（照微信）**：窄图标条 → 列表 → 主内容区三栏；会话行的头像 + 摘要 + 时间 + 未读角标；
  右键菜单、置顶、搜索、`≡ 更多` 的位置。用户已经会操作了，挪一个位置就是白丢一层肌肉记忆。
- **设计语言（照 Tesla）**：几何无衬线（Inter / Manrope 这一路）、字重字号当层级、黑白灰打底
  加一个强调色、大留白少分隔线、动效短直有目的、**先删后加**（同一个动作只留一个入口，
  能用默认值解决的不做成设置项，不可逆的操作要多走一步）。
- **打架时**：习惯动作（点哪儿、右键出什么）听微信；长相和手感（字号、间距、颜色、反馈节奏）
  听 Tesla。判不下来写进 `decisions.md`。

落地在 `crates/nousdesk/src/theme.rs`（默认皮 + `ModelS`/`Model3`/`Cybertruck`/`Mcu`/
`Editorial`/`NightDrive`/`Energy`/`Plaid`/`NoRail` 九套）。

## 碰外观就先出 HTML demo（硬规矩）

**凡是牵涉外观的，先做 HTML demo 让用户挑，挑完再写 Rust。** 别直接改 `*.rs` 让人对着
截图提意见 —— 那等于拿编译等待时间换来回拉锯。

- **算外观**：布局与分区、间距 / 留白、字号 / 字重 / 行高、颜色、圆角 / 边线 / 阴影、
  图标形态、动效、新控件长什么样。
- **不算外观**：协议与请求、状态机、数据模型、校验逻辑、键盘绑定的语义（但**按键提示摆哪里**算）。
- **怎么做**：放 `demo/<主题>/`（不进构建、不碰 `Cargo.toml`），静态 HTML + CSS；用**真字号真栏宽
  真字体**；多方案编号并排或可切换，每套配一句「这套想干什么」。
- **交付**：给**完整绝对路径**的单文件 `preview.html`（字体与数据内嵌，双击就开）。
  **不给端口、不给 `localhost:xxxx`**。
- **确认之后**：挑中的方案写进 `decisions.md`（为什么是这套），再动 Rust；落地后截一张图对照，
  不一致算没做完。用户说「就一个方案」也照样先出 demo。

## 三条硬规矩

1. **这不是网页。** 没有 `package.json` / vite / 前端 bundle：加了脚手架，宿主会当它是网站
   spawn 起来、给空白页、崩了留墓碑。别为此加。
2. **构建/测试在 Linux 上做。** headless 机器（ssh 进去的、CI）没 `DISPLAY` 就只跑
   `cargo check` / `clippy` / `test`；看得见的东西去自己的桌面上跑。
3. **地址只有一个来源。** `desktop.toml` 的 `host`，没有第二个。禁止把引擎路径写进代码常量、
   禁止再拼 `/p/<board>/<block>/…` —— `board` / `block` / `resolve` 那一套在 2026-11-18
   随旧面板一起没了（`#8`）。功能有没有，问 `GET /v1/capabilities`，不猜。

## 配置与启动

```toml
# ~/.config/nousdesk/desktop.toml（查找顺序：NOUS_DESKTOP_CONFIG → 二进制旁边的
# desktop.toml（发布包那份）→ ~/.config/nousdesk/；旧目录 ~/.config/nous/ 只读兜底）
host = "http://192.168.1.7:4789"
token = "…"            # 可选；也可放 ~/.config/nousdesk/token（0600），或 env NOUS_DESK_TOKEN
```

- token 取值顺序：`NOUS_DESK_TOKEN` → `desktop.toml` 的 `token` → `~/.config/nousdesk/token`。
  文件权限不是 0600 只警告、不拦（便携包那份 `desktop.toml` 可能只读）。
- 主题**不在**这份文件里：`~/.config/nousdesk/theme.toml` 的 `theme = "default"`。
- 启动打一次 `/v1/capabilities`：引擎版本进日志 + About 面板；`contract` 不匹配就出横幅
  （不猜、不降级）；`runtime.provider.kind == "faux"` 时标题栏挂 `DEMO` 徽标。

## 工程规矩（别破）

- **踩一个坑就往 `坑.md` 加一条。** 症状 → 根因 → 解，三行以内，追加到末尾。
- **追加式文档合冲突必须两边都留**（`坑.md` / `decisions.md` / `验证.md`）。不许整份取一边：
  2026-11-16 有人这么干，`坑.md` 68–71 四条从 `main` 上蒸发过。
- **SSE 合帧到 16ms 再 `cx.notify()`。** `bytes_stream` 往 buffer 塞，定时器读一次再通知 UI。
  一个 token 一次 `cx.notify()` 是这一层最容易犯的错。
- **未知字段用 `serde_json::Value` 收着原样透传**，不许 `deny_unknown_fields`。
- **协议是哑的。** 客户端只转发 `/v1/*`，不加业务字段、不在客户端算状态。类型变化先改
  `crates/nous-engine`，UI 层一行 HTTP 都不许碰。
- **只吃 `gpui-kit` 的重导出。** 别在自己的 `Cargo.toml` 里再写一条 `gpui-pre`。
- **能力判定只有一处。** 哪个入口灰掉、为什么灰，全在 `crates/nousdesk/src/capability.rs`
  （`Feature` → `probe_path`/`module_name` → `gate`）；UI 只消费 `disabled_reason()`，
  判定函数只有一份。灰掉 = 不挂点击 + 键盘 Action 也拦住；理由另一处给（设置页每项一行、
  `≡ 更多` 一行），别在注释里写「tooltip 说人话」而实际没挂。

## 分支与交付（硬要求）

1. **每一件事都开分支做**：`feat/…`、`fix/…`、`docs/…`。不直接在 `main` 上改。
2. **做完就停**：本地 commit、跑完 `cargo test` / `cargo clippy` / 构建 / `scripts/check-desk.sh`，
   把分支和验证方法交给用户；**不自己合并、不推 `main`、不删分支**。
3. 用户人工验过、明确说「没问题」之后才合并、推送、删分支；默认 `git merge --no-ff`。
4. **一个 worktree 一个分支；`/opt/NousDesk` 只放 `main`。**
   `git worktree add /opt/NousDesk-<主题> -b feat/<主题> origin/main` → 干完 `--no-ff` 并进
   `main` → `git worktree remove`。理由：多人/多会话共用一个工作树时，未提交的改动会一直挂着 ——
   别人切不动、删不掉，`git add` 还能把别人的半成品卷进自己的提交（真发生过）。

例外：只改文档/注释的琐碎改动可以直推 `main`，但**平台、协议、配置路径**相关的仍走分支。

## 怎么验

```bash
cargo check                                  # 日常
cargo test --workspace                       # 单元 + 集成
cargo clippy --workspace --all-targets       # 与 main 基线比条数，别新增
scripts/check-desk.sh                        # 一个地址 + capabilities 的自动验收（真引擎 faux + 负向哨兵）
scripts/package-linux.sh                     # 出 dist/ 下的 tar.gz + deb
```

`check-desk.sh` 要一个能跑的 `nous-app`（`NOUS_APP_BIN=…` 指定，默认 `./target/debug/nous-app`）。
它起一个 faux 引擎、建会话、收 SSE 三帧、看 assistant entry，再打负向哨兵（错 token 必须 401，
且**会话条数与 entries 条数都不能变**）。`SKIP` 打印不是通过。

出包脚本是**唯一**的打包真相：`.github/workflows/linux.yml` 只装依赖、跑测试、调这个脚本。

### 窗口装饰（Linux）

```bash
cargo run                                        # 真桌面上看一眼
rg 标题栏 ~/.local/share/nousdesk/desktop.log   # 日志那行「装饰=… 最小化=… 最大化=…」
```

GNOME 不给 server-side decorations，所以标题栏由我们画（`src/chrome.rs`，只在
`target_os = "linux"` 编译）。日志该是 `装饰=Client … 最小化=true 最大化=true`；
裸 Xvfb 上是 `装饰=Server`（按钮本来就不画，那是预期）。肉眼四件事：拖标题栏能移窗 /
双击最大化还原 / 三个按钮有反应 / 任务栏里窗口名是 `NousDesk`。

### 中文输入法（IME）

```bash
scripts/check-ime.sh                 # 协议层：enable / set_cursor_rectangle / commit
cargo run -p nousdesk --example ime  # 人工：敲拼音，看组字下划线和上屏
```

少了 `enable` 输入法根本不认这个窗口；少了矩形，候选框飘到左上角；两种都不崩，只是打不出中文。
上屏只能人验：Wayland 下没有进程能伪装成输入法（`zwp_text_input_v3` 只接受合成器的
`commit_string`）。X11 / XWayland 走 `zed-xim`（XIM），排查前先分清后端
（`WAYLAND_DEBUG=1` 有输出就是 Wayland）。

### 剪贴板

```bash
scripts/check-clipboard.sh                # 读 + 粘贴（自动）
cargo run -p nousdesk --example clipboard # 写（人工）：Ctrl+A、Ctrl+C，再 wl-paste
```

`set_selection` 在 Wayland 上必须带最近一次输入事件的 serial，所以**写**只能人工验：
`wl-paste` 有内容 = 平台写通；空 = 先看程序有没有拿到焦点。mime 只声明
`text/plain;charset=utf-8` / `UTF8_STRING`，中文乱码先怀疑这条，不是渲染。

### headless 机器上看窗口

GPUI 能在 Xvfb 里跑，三个前提：mesa ≥ 25.0.7（22.x 的 llvmpipe Vulkan WSI 会黑屏且无报错）、
跑一个 compositor（`picom --backend xrender`，GPUI 用 depth32 带 alpha 的 Visual）、
GPU 程序不用指定（GPUI 自己枚举 adapter）。

```bash
Xvfb :99 -screen 0 1280x800x24 &
DISPLAY=:99 picom --backend xrender &
DISPLAY=:99 ./target/debug/nousdesk &     # 跑产物，别用 cargo run（换 HOME 会触发 rustup 重装）
DISPLAY=:99 import -window root /tmp/shot.png
```

这台机器的全局 `CARGO_TARGET_DIR` 指向仓外，产物不在项目的 `target/` 里 —— 跑脚本一律
`env -u CARGO_TARGET_DIR`，跑产物就写绝对路径。

## 置顶的真相

在**引擎**里（会话的 `pinned`）。桌面端不自己存一份 —— 写穿 `POST /v1/sessions/:id/pin`
（body `{pinned, requestId}`，`requestId` 必填、是幂等锚），读 `GET /v1/sessions` 的 `pinned`
（内核已排好：置顶在前，**别自己再排一次**）。

**重放同一个 `requestId`，回来的就是第一次的回执**：`version` 不涨说明没落第二条提交，
但 `changed` 还是那一次的值（不是 `false`）。所以幂等证据看 `version` + 顺序，别看 `changed`
（`坑.md` 99；票面那句「重放 → `changed:false`」当时写错了，内核语义比它更硬）。
会话不存在 → 404，客户端不许当成功。

宁可多一次 HTTP、坏了 404 响得很明显，也不要两份互不相干的置顶表。不再写手机端那块 ——
它停不停都不该让置顶坏掉。

**协议层已验证**（2026-11-18，faux 引擎，`scripts/check-desk.sh` 的 12 条 `pin.*`）：
写进去 → `pinned=true` 且排最前 → 同锚重放 `version` 不涨、顺序不动 → 取消回原顺序 →
不存在的会话 404。**真桌面那一眼（右键菜单立刻上浮 / 回落）待补。**

旧的菜单行为是 2026-11-09 在 Windows 真机验的（那时还写穿 `bjes6b`）：点得动、失败会回滚。
合成鼠标点不动就走 `PostMessage`（见 `坑.md` 23）—— 这条验法仍然有效。


## 文档

- `README.md` —— 给人看的：这是什么、怎么跑。
- `wire.md` —— 我们依赖引擎 `/v1/*` 的哪些字段、引擎改了会怎么坏。它**不完整**：
  末尾有「合同外调用」一节列着客户端在用、正文没写的端点。改客户端调用前先扫那节，
  否则会以为某个调用"没有合同依据"。
- `decisions.md` —— 选了什么 + 为什么 + 什么时候该反悔。追加式。
- `坑.md` —— 开发中踩的每个坑。症状 / 根因 / 解，三行以内。
- `验证.md` —— 哪些东西真机验过（带日期和怎么验），以及还没验的清单。
  **动手前先看一眼：验过的别再验**，除非你改了它依赖的那段代码。

## Agent skills

### Issue tracker

GitHub Issues（`JoeWest9966/NousDesk`，用 `gh` CLI）。See `docs/agents/issue-tracker.md`.

### Triage labels

五个 canonical 角色名与仓库标签同名（`ready-for-agent` 等）。See `docs/agents/triage-labels.md`.

### Domain docs

single-context；领域词表以根目录 `AGENTS.md` / `wire.md` / `decisions.md` 为准。See `docs/agents/domain.md`.
