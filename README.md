# NousDesk

Nous 的桌面客户端。**骨架照微信桌面版，设计语言照 Tesla**，原生应用，不是网页。

- **引擎**：Nous。这个客户端只做皮 + 一个哑的协议层，不跑 agent。
  - **现状**：连的是 **Rust 版引擎 `nous-app` 的一个地址**，启动拉一次 `/v1/capabilities`，
    按声明开功能面；未声明的一律当没有。
  - **旧版（已下线）**：TS 版 `qrd3wh` / `bjes6b` / `d6s4v1` / `n8qkkj` 靠 `__nous/resolve`
    换地址那套已经删掉（`resolve.rs` 不在了）。**见下面「换引擎」那一节。**
- **客户端只有这一个**：`bjes6b`（Nous-Mobile）被 Rust 版 Nous 取代 —— 手机端那个仓会被删，
  置顶、触发器这些以前写穿到 `bjes6b` 的东西都改由 Nous 内核承担。
- **技术**：Rust + GPUI，UI 框架是 [longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)（`gpui-kit 0.9`）。没有 webview，没有 PWA。
- **平台**：只做 **Linux**（开发机：Debian 13 trixie + GNOME **Wayland**，AMD 核显）。
  Windows 的源码与打包配置**原地保留**（`.github/workflows/windows.yml`、`[package.metadata.packager]`），
  但不再出包、不保证能跑 —— 要用的话得自己装 Windows 工具链验一遍。

> 2026-11-12 决定：机器都换 Linux 了，Windows 那条从主线降成「冻结存档」，见 `decisions.md`。

## 设计参考：微信 + Tesla

**信息架构听微信，长相和手感听 Tesla。**

- **骨架（微信）**：窄图标条 → 列表 → 聊天区三栏；会话行的头像 / 摘要 / 时间 / 未读角标；
  右键菜单、置顶、搜索、`≡ 更多` 的位置。这些是用户已经会的动作，不随便挪。
- **设计语言（Tesla）**：几何无衬线字体、大留白、黑白灰 + 一个强调色、少分隔线、
  短而有目的的动效；功能上先删后加 —— 一个动作一个入口，能用默认值的不做设置项。

分不清时以「用户肌肉记忆照微信、观感手感照 Tesla」为准，判不下来的写进 `decisions.md`。
细则见 `AGENTS.md` 那节「设计参考」。

## 开发宗旨

**体验好、性能好、灵活性强 —— 不惜增加开发成本。**

取舍时先问这三条，再问「写起来快不快」。为此多写测试、多拆一层、多做一次测量都是成本的一部分，
不算超支；反过来说「为了省事牺牲体验/性能/灵活性」要先写进 `decisions.md`。细则见 `AGENTS.md`。

## 跑起来

```bash
cargo check          # 只检查，不跑（开发机 headless 也能跑）
cargo run            # 自己的桌面上（Wayland 或 X11 都行）
cargo build --release -p nousdesk   # 出包：target/release/nousdesk
```

配置查找顺序（先到先得）：

1. `NOUS_DESKTOP_CONFIG` 环境变量指向的文件（调试用）
2. **二进制旁边**的 `desktop.toml` —— 发布包里带一份，解压即用
3. `~/.config/nousdesk/desktop.toml`（= `$XDG_CONFIG_HOME/nousdesk/`；Windows 是 `%APPDATA%\nousdesk\`）
4. `~/.config/nous/desktop.toml` —— 改名前的旧目录，**只读**：老用户的配置还能接上，
   写回时仍然写回它（不搬家，免得用户以为配置丢了）

```toml
host = "http://192.168.1.7:4789"
# token = "…"     # 可选；也可以不写在这里，见下面
```

一个都没有也能开：用默认值（`192.168.1.7:4789`），连不上时左栏会把该改的路径显示出来。

**老配置里的 `board = "…"` 会被警告并忽略**，不会让升级后的客户端开不起来（便携包那份
`desktop.toml` 是手工放的，不能因为升级就开不了）。

### token 取值顺序（只有一个注入点）

```
NOUS_DESK_TOKEN 环境变量  →  desktop.toml 的 token = "…"  →  ~/.config/nousdesk/token
```

- `EngineClient::new` / `with_token` 在 **http 和 sse 两个 reqwest builder** 上各挂一次
  `Authorization: Bearer …`；41 个调用点谁也不许再拼这个头。
- 文件兜底是给便携包那份**只读** `desktop.toml` 用的。
- `~/.config/nousdesk/token` 权限不是 `0600` 只警告、不拦启动（容器里常见），建议
  `chmod 600 ~/.config/nousdesk/token`。
- 服务端那边的 token 在 `~/.config/nous/token`（`nous-app --print-token` 可打印）。

启动时直连 `host`，拉一次 `GET /v1/capabilities`；`contract` 主版本不匹配出顶部横幅，
`modules[].enabled=false` 的入口灰掉并写人话，`runtime.provider.kind == "faux"` 时标题栏
挂「演示模式」。`configured=false` **不灰输入框**，只是发送前提示可能 503。

## 换引擎：TS 版 Nous → Rust 版 Nous（2026-11-18 已切）

Nous 已经整体重写成 Rust（`/opt/Nous`，一个 `nous-app`）。这次重写的
关键差异**不在语言，在形状**：

| | TS 版（已下线） | Rust 版（现在连的） |
|---|---|---|
| 进程 | 一个功能一个块（`qrd3wh` / `bjes6b` / `d6s4v1` / `n8qkkj`） | **一个** `nous-app` |
| 寻址 | `__nous/resolve/<block>`（三个块名常量） | **一个地址** + `/v1/capabilities` 服务表 |
| 扩展 | 单独进程 + 文件 | 三种装载形态（编译期内置 / 运行时外挂 / 声明式） |
| 发行版 | 一份代码全开 | 一张清单（`features` / 模块声明决定装了什么） |

**切换已完成的范围**：桌面端仍留着 41 条调用路径，服务端按 `capabilities` 声明开放
会话面 / `documents` / 设备族；`presets` / `archive` / `combos` / `triggers` / `ext`
在清单里关着，入口灰掉但**调用点保留**，等后续模块落地一次性切换。

| Nous 的票 | 内容 | 这边怎么用 |
|---|---|---|
| `#47` | 7 端点 + `GET /v1/capabilities` | 功能面唯一真相；未知 → 不可用 |
| `#48` | `documents` + `/v1/_routes` | 能力表已声明，编辑器暂不接 |
| `#49` | 砍 `board` / `block` / `resolve` | 本仓库删 `resolve.rs`，配置只剩 host(+可选 token) |
| `#44` / `#51` | 最小轮次 + faux provider | 验收脚本能收到一条确定回复；faux 挂「演示模式」角标 |

自动验收：`scripts/check-desk.sh`（起真 `nous-app` + `desk_probe`，含错 token 哨兵）；
真机四眼见 `验证.md`。**未声明的功能不许猜着调用** —— 这是这一版的硬边界。

## 出包（Linux）

```bash
scripts/package-linux.sh                  # 编 release + 出 dist/ 下两个包
scripts/package-linux.sh --no-build       # 复用已有的 target/release/nousdesk
NOUS_HOST=http://192.168.1.7:4789 scripts/package-linux.sh   # 顺手把宿主地址写进便携包
```

| 产物 | 怎么用 |
| --- | --- |
| `dist/nousdesk-<版本>-linux-x64.tar.gz` | `tar -xzf` 后直接跑 `nousdesk/nousdesk`；二进制旁边放一份 `desktop.toml` 就是这份安装的配置 |
| `dist/nousdesk_<版本>_amd64.deb` | `sudo apt install ./…deb`：装到 `/usr/bin/nousdesk`，带 `.desktop` 菜单项和 hicolor 图标 |

打 tag `v*` 时 `.github/workflows/linux.yml` 跑同一份脚本并把两个包发到 Release；手动 `workflow_dispatch` 也能出包（不发 Release）。

### 按提交取货（不想等冷编译）

推 main 时 CI 会自动出一份**按提交号命名**的包，挂到滚动预发布 `ci`（只留最近 5 份）：

```sh
gh release view ci                                   # 最近 5 个提交的包
gh release download ci -p 'nousdesk-<sha8>*'
```

急着要某一份时不用等 CI（它是 4 核，一轮 11–15 分钟）：本机/开发机出包后

```sh
scripts/package-linux.sh && scripts/publish-ci.sh     # 编 + 挂到 ci（同名覆盖）
scripts/publish-ci.sh --dry-run                       # 只看要干什么
```

两条路**落同一个名字**，所以先后到无所谓；`ci` 里的是「某个提交」，没有版本号语义 ——
版本只认 `v*` 的 Release。
包里不带宿主地址：不给 `NOUS_HOST` 就带一份 `desktop.toml.example`，内网地址不进公共产物。

## 结构

```
crates/nous-engine/   协议客户端：/v1/* 的 serde 类型 + SSE 流。UI 一行都不许进。
crates/nousdesk/      GPUI 应用：三栏布局、会话、输入、置顶。
```

分层是硬的：UI 层不碰 HTTP，协议层不碰 GPUI。以后想换 UI（Slint / 别的）只动 `nousdesk`。

## 三条规矩

1. 引擎地址就是配置里那一个 `host`，不再从 `resolve` 或块名推导。
2. 协议是哑的：不加业务字段、不在客户端算状态。字段变化先改 `nous-engine`。
3. 置顶的真相在内核 qrd3wh，桌面端读写 `/v1/sessions/:id/pin`，带 `requestId` 幂等锚，不自己存一份。
4. 功能面只认 `/v1/capabilities`；`capability.rs::gate()` 是唯一判定处。

详情见 `AGENTS.md`（给 agent 的）和 `wire.md`（协议契约）。

## New Manual（「盘面」）

点左栏右上角的「+ New Manual」——**只顶替右侧聊天区那一格**，左导航和会话列表
永远在。Esc 或点左栏任意一条会话就回到聊天。右格里**没有顶栏**。

外形是「轨道」+「环」（`demo/new-manual/#7`、`demo/new-manual-ring`）：四张参数卡
（Model / Context / Extensions / Permissions）围着中央一颗 `Create`，虚线把「谁喂给谁」
连出来；再外面一圈虚线环，**名字 / 机器 / 目录三颗 pill 挂在环上**，圆边各引一条短线过去。
点开的是**就地弹出**的选择页（`demo/new-manual-7/#3`）—— 贴在卡片或 pill 旁边，
不是居中模态，选参数的时候盘面还看得见。

- `crates/nousdesk/src/board.rs` —— 状态 + 渲染 + 五个弹层
- `crates/nousdesk/src/board_art.rs` —— 颜色（从 `Skin` 取）、cqmin 尺寸、卡片 / 环 /
  三颗 pill 的矩形、连线几何
- **盘面跟皮走**：颜色全部从 `theme::cur()` 取（底色 `chat_bg`、卡 `panel`、虚线 `line`、
  圆 `text`、环 `accent`），这里不另存一套调色板。浅皮里圆自动是黑圆白字、
  深皮里是白圆黑字 —— 五套皮都成立
- 卡片 / 环 / pill 和弹层的位置**只在 `board_art` 算一次**（`node_rect` / `foot_rect` /
  `pop_rect`）：弹层要贴着被点的那颗弹，各算各的就会错位
- 连线用 `canvas` + `PathBuilder` 画虚线；`paint_path` 吃**窗口坐标**，
  每个点都要加 `bounds.origin`（坑 34）
- **盘面只吃一个比例**：`art::M::scale`。所有设计值 × scale 落成 px，
  `scale = min(pane_w/940, pane_h/764)` —— 单一比例，窗口怎么拉都不变形。
  以前是「一半按宽、一半按高、另一半写死 px」，宽高比一变就互相打架
- **目录弹层是命令面板**：一个输入（打 `/` 或 `~` 开头 = 路径，打别的 = 在这一层里筛）、
  一条可点的面包屑、左边这一层、右边告诉你这个目录是什么（机器 / 这一层的会话数 / 子目录数）。
  路径的根是 `/`（`?path=/` 直接有 18 项），起点是 map；`?machine=` 能列**远端机器**的目录，
  切机器就整棵树换根
- **技能弹层是左树 + 右列表**：左栏是类别树（`全部 / 工程 / 事故 / 写作 / 长文 / 未分类`，
  缩进就是层级，空类灰着还在），右栏是技能。顶上一个通栏搜索框，**一打字左树就让位**
  —— 右边换成全盘命中，因为「搜」和「按类别找」是两种状态，不该叠在一起。
  搜索同时看名字、描述、类别：技能名是英文（`incident-postmortem`），人记得住的是「复盘」
- **扩展卡与弹层**：卡片上一行圆点，一条扩展一个（`4/4`、`1 个加载失败 ›`）；
  弹层是**左树 + 右列表 + 通栏搜索**，树里先放「状态」（启用中 / 内置 / 变体 / 已归档）
  再放「按类别」，一条线隔开。勾的是**这次会话带哪些扩展**（`POST /v1/sessions` 的
  `extensions`），不是本机装不装 —— 那是 `install` / `uninstall` 的事
- **Permissions 卡**：一个二值开关，**不弹层** —— 整张卡就是那个开关。引擎里没有
  「权限」这回事（`/v1/permissions` 404），有的只是 `readOnly`（摘掉会写的工具），
  所以卡上说的是「能不能动东西」：`可写 · 能改文件、能跑命令` / `只读 · 只能读，改不了东西`。
  竖排档退化成一行 `可写`，点一下切
- **会话重命名**：左栏右键 →「重命名」→ 居中对话框，回车提交。空名字本地就拦
  （引擎会回 400）；改完只动本地那一行，不重拉列表
- **会话页顶栏「它带什么」**：`deepseek-v4-flash · smart-ass · 只读` —— 盘面上选的东西
  进会话之后就在这儿落脚。点它打开「这个会话带什么」，四行（模型 / 模板 / 技能 / 权限），
  点哪行开哪张卡的弹层，改一处推一处（`POST /v1/sessions/:id/config`）
- **键盘流**：`Tab` / `Shift+Tab` 在**五个停靠点**（圆 + 四张卡）之间走，`Enter` 按下去
  （圆 = 建会话、卡片 = 开弹层、Permissions = 切换），`← →` 在 Context 卡里换技能 / 模板
  那一半，`Esc` 先关弹层再关盘面，`⌘⏎` / `Ctrl+⏎` 任何时候直接建。
  焦点框是 **2px 强调色** —— `mono` 皮的 accent 跟边线同为白色，只有粗细能说话
- **三档断点**（按右格宽，不是窗口宽）：`≥780` 完整（四角卡 + 环 + 三 pill）；
  `620–780` 紧凑（去掉环，卡缩到 32%，三颗 pill 横排贴圆下）；
  `<620` 竖排（圆在顶、卡片一列、**允许纵向滚动** —— 这时"一屏不滚"已经不可能，
  硬塞就是卡片互相盖住）。见 `demo/new-manual-responsive`
- 弹层是遮罩一整块 + 面板 `block_mouse_except_scroll()`（坑 59）：
  少了这句，点面板里的技能行会顺手把面板关掉
- 模型是**三列铁路图**（供应商 › 模型 › 思考），技能 / 模板 / 目录是平铺清单：
  前者是有依赖的语法链，后者不是。为版式统一把语义搞错不划算
- 卡片的扩展数据来自 `GET /v1/extensions` 的 `items[]`（引擎 0.1.9 / `nous-ext` #81）：
  `kind` 是装载形态（`native` / `legacy-js` / `invalid`），`category` 才是分类；
  旧 JS 插件单列一档「待重写」，不给装 / 卸按钮（新运行时没解释器，点也是 403）
- Permissions 内核页还没做，卡片是灰的不可点 —— 不假装能选
- **中央那颗圆就是落子按钮**（`demo/new-manual-core/#1`）。三件事按同一份判定：
  没选模型 / 没连引擎 → 圆变灰、不挂点击、三颗 pill 一起变浅；提交中 → `Creating…`
  且变淡防连点；就绪 → 实心圆，环底下那行写 `ready · 本机 · map`（机器 + 目录尾巴，
  **点下去之前人该看得见自己落在哪**）
- 名字 / 机器 / 目录是环上那三颗 pill，**底色和中央那颗圆完全一样**（`fg` 当底、`bg` 当字）：
  它们不是三个独立控件，是从圆心甩出去的参数。换皮时一起换，别让 pill 自己另长一套色
- 每次开盘面都是**一只新的名字输入框**（不留上次没发出去的名字）；
  目录的默认值 = 引擎 `/v1/fs/dirs` 根底下第一个条目（`path == ""` 是 map 根，不是目录本身）
- 创建成功**直接进那条会话**，不在盘面上留一行结果：桌面端最强的反馈就是会话已经开着。
  只有两件例外走全局轻提示 —— 服务端回 `missing` / `missingTemplates` /
  `missingExtensions`（勾了没装成，点名）和失败（红字留在圆底下，原文照登）

调试两个开关（只为了不用每次手点）：

```bash
NOUS_OPEN_BOARD=1 ./target/debug/nousdesk              # 一启动就开在右格的盘面上
NOUS_OPEN_BOARD=1 NOUS_OPEN_SHEET=model ./target/debug/nousdesk    # 再直接开在某个弹层上
# sheet ∈ model | skills | prompt | dir | machine
NOUS_DESKTOP_OPEN=settings ./target/debug/nousdesk                 # 一启动就开在左栏底部「≡」的设置页
# menu | settings
```
