# h58ga5 · NousDesk

Nous 的**桌面客户端**（微信桌面版的样子）。GPUI + `gpui-kit` 原生应用，不是网页。

> **平台：只做 Linux**（2026-11-12 起）。开发机是 Debian 13 + GNOME **Wayland**。
> Windows 的源码与打包配置（`windows.yml`、packager 元数据）**原地保留、冻结**：
> 不删、不改、不再出包；要重启 Windows 线，先把这份文档和 `README.md` 的平台段改回来。
> 理由见 `decisions.md` 的「2026-11-12 · 只做 Linux」。

> **UI 框架钉死：必须用 [longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)**
> （`gpui-kit = "0.9"`，crates.io 同名包，Apache-2.0）。
> **不许换框架、不许绕开它直接用 `gpui-pre-*`、不许把 Tauri / Slint / egui 加进来。**
> 组件怎么用、文档在哪、有哪些现成的例子 —— 一律以那个仓库为准：
> <https://github.com/longbridge/gpui-kit> · 文档 <https://gpui-kit.com>。
> 要换框架先说清楚为什么，写进 `decisions.md`，别默默换。

- **引擎在 `qrd3wh`**（Nous 的 session 块）。这一块**不跑引擎，也不是引擎的一部分**。
- 手机端是 `bjes6b`（Nous-Mobile）。两边是同一个产品的两个客户端，对同一个引擎。
- 这一块是**项目本体 + 文档的落脚处**。源码放这儿，跑在你自己的桌面上。

## 开发宗旨

**体验好、性能好、灵活性强 —— 不惜增加开发成本。**

这句话是取舍的唯一标准，写在这里就是为了以后别再拿「这样写得快」当理由：

- **体验好**：不到 60fps 的动画不上、输入延迟能感觉到就先量再改；按钮点下去没反应、
  等半天没反馈，都算 bug，不算「可以以后再补」。中文输入法、剪贴板、HiDPI、
  窗口拖拽缩放在 Linux 上是首屏体验的一部分，不是边角料。
- **性能好**：虚拟列表、增量渲染、SSE 合帧、避免每秒重建整棵元素树 —— 宁可多写一层缓存和
  状态机，也不让用户看到卡顿。慢要慢得可解释（有 profiling 数据），不能慢得莫名其妙。
- **灵活性强**：协议层/UI 层分家（`nous-engine` vs `nousdesk`）、配置与常量分开、
  后端可换。宁可多一个抽象层、多几个 trait / interface，也不要为了少写一行把
  引擎地址、平台、主题写死在调用点上。

**硬要求**：上述三项要付的代价（多写测试、多拆一层、多做一次测量）**算开发成本，不算超支**；
反过来，「为了省事牺牲体验/性能/灵活性」要先写进 `decisions.md` 并给出反悔点。

## 三条硬规矩

1. **这一块不是块页面。** 没有 `package.json` —— 有了 `scripts.start` 宿主就当它是网站，
   spawn 进程、给空白页、崩了留墓碑。别为此加脚手架。
2. **构建/测试在 Linux 上做。** 主力是 Debian 13 + Wayland；headless 机器（ssh 进去的、CI）
   没 DISPLAY 就只跑 `cargo check` / `clippy` / `test`，看得见的东西去自己的桌面上跑。
   别再把 Windows 当默认目标。
3. **地址只有一个来源。** 引擎地址从宿主的 `__nous/resolve/qrd3wh` 拿，配置里只写
   **宿主地址 + board**，禁止把引擎路径写进 `Cargo.toml` / 常量 / 文档示例。

## 分支与交付（硬要求）

1. **每一件事都开分支做**，不直接在 `main` 上改：`feat/…`、`fix/…`、`docs/…`。
2. **做完就停**：本地 commit、跑完 `cargo test` / `cargo clippy` / 构建，然后把分支和
   验证方法交给用户；**不要自己合并、不要推 `main`、不要删分支**。
3. 用户人工验过、明确说「没问题」之后，才允许合并、推送、删分支；合并方式默认
   `git merge --no-ff`（保留这条分支的证据），除非用户另有要求。

例外：只改文档/注释且不动代码的琐碎改动（改错别字、补一行说明）可以直推 `main`，
但**平台、协议、配置路径**相关的文档改动仍走分支。

## 工程规矩（别破）

- **踩一个坑就往 `坑.md` 加一条。** 三行以内：症状 → 根因 → 解。追加到末尾、编号顺延，
  别改写旧条目、别写成日记。同一个坑踩第二次是这一条失灵的信号。
- **SSE 合帧到 16ms 再 `cx.notify()`。** `bytes_stream` 往 buffer 里塞，定时器读一次再通知 UI。
  一个 token 一次 `cx.notify()` 是这一层最容易犯的错，和 bjes6b 手机上那条 rAF 规矩一个字不差。
- **未知字段用 `serde_json::Value` 收着原样透传。** 不许 `deny_unknown_fields` —— 那个 flag
  会让引擎每加一个字段这边就编译不过。
- **协议是哑的。** 客户端只转发 `/v1/*`，不加业务字段、不在客户端算状态。
  `/v1/*` 的类型变化先改 `crates/nous-engine`，UI 层一行 HTTP 都不许碰。
- **只吃 `gpui-kit` 的重导出。** 永远别在自己的 `Cargo.toml` 里再写一条 `gpui-pre`，
  否则版本对不上时会在 `Entity<T>` 不匹配上耗一整天。

## 置顶的真相

在 **bjes6b**（`data/pins.json`）。桌面端不自己存一份 —— 写穿过去走
`POST /p/demo/bjes6b/api/sessions/:id/pin`，地址从 `resolve` 拿。宁可多一次 HTTP、
坏了 404 响得很明显，也不要两份互不相干的置顶表。要改这条，见 `decisions.md`。

**已验证**（2026-11-09，Windows 真机）：右键菜单「置顶该聊天 / 取消置顶」点得动，
写入 `pins.json` 后左栏顺序跟着变，失败会回滚。合成鼠标点不动就走 `PostMessage`（见 `坑.md` 23）。

## 启动 / 开发

```bash
cargo check          # 这台机器上唯一的日常
cargo run            # 你自己的桌面上
```

### headless 机器上看窗口（本机 99966 已配好）

GPUI 能在 Xvfb 里跑，三个坑都踩过了：

1. **mesa 必须 ≥ 25.0.7**（bookworm-backports）。22.3.6 的 llvmpipe Vulkan WSI
   会让窗口黑屏/灰屏（swapchain 内容落不到窗口），症状是截图纯黑且无报错。
2. **要跑一个 compositor**（`picom --backend xrender`）：GPUI 用 depth32 带 alpha 的
   Visual，没有 compositor 合成，画面一样出不来。
3. **GPU 程序不需要指定**：GPUI 自己枚举并测试 adapter，选 llvmpipe (Vulkan)。

```bash
Xvfb :99 -screen 0 1280x800x24 &
DISPLAY=:99 picom --backend xrender &
DISPLAY=:99 target/debug/nousdesk &
DISPLAY=:99 import -window root /tmp/shot.png   # 截图看
```

注意 `CARGO_TARGET_DIR` 在这台机器上全局指向 `/opt/Warehousetui/target`，
二进制不在项目的 `target/` 里。

配置查找顺序：`NOUS_DESKTOP_CONFIG` → **二进制旁边的 `desktop.toml`**（发布包带的那份）
→ `~/.config/nousdesk/desktop.toml`（旧目录 `~/.config/nous/` 只读兜底）。
发布包里的 `desktop.toml` 目前是手工放（Linux 出包流程见 `decisions.md`）；
Windows 那条 CI 还在仓库里，但只手动触发，不再是发布路径。
源码里的默认值 `DEFAULT_HOST` 是「一个配置文件都没有」时的兜底，现在指向内网 `192.168.1.7:4790`。
只有两个键：

```toml
host = "http://192.168.1.7:4790"
board = "new1"
```

启动时打一次 `<host>/p/<board>/h58ga5/__nous/resolve/qrd3wh` 拿引擎地址，然后直连引擎。
引擎版本进日志 + About 面板 —— 以后「怎么突然坏了」先看是不是引擎先动的。

## 文档

- `README.md` —— 给人看的：这是什么、怎么跑。
- `wire.md` —— **唯一会漂的那份**：我们依赖了引擎 `/v1/*` 的哪些字段、引擎改了会怎么坏。
- `decisions.md` —— 选了什么 + 为什么 + 什么时候该反悔。一行一条，追加式。
- `坑.md` —— **开发中踩的每个坑**。症状 / 根因 / 解，三行以内。踩到新的就加。
- `验证.md` —— **哪些东西真机验过**（带日期和怎么验的），以及还没验的清单。
  **动手前先看一眼：验过的别再验**，除非用户主动要求、或者你改了它依赖的那段代码。
