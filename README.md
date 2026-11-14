# NousDesk

Nous 的桌面客户端。**骨架照微信桌面版，设计语言照 Tesla**，原生应用，不是网页。

- **引擎**：`qrd3wh`（session 引擎）。这个客户端只做皮 + 一个哑的协议层，不跑 agent。
- **手机端**：`bjes6b`（Nous-Mobile），同一套引擎，另一个客户端。
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
host = "http://192.168.1.7:4790"
board = "new1"
```

一个都没有也能开：用默认值（`192.168.1.7:4790`），连不上时左栏会把该改的路径显示出来。

启动时从 `<host>/p/<board>/h58ga5/__nous/resolve/qrd3wh` 拿引擎地址，然后直连引擎。

## 出包（Linux）

```bash
scripts/package-linux.sh                  # 编 release + 出 dist/ 下两个包
scripts/package-linux.sh --no-build       # 复用已有的 target/release/nousdesk
NOUS_HOST=http://192.168.1.7:4790 scripts/package-linux.sh   # 顺手把宿主地址写进便携包
```

| 产物 | 怎么用 |
| --- | --- |
| `dist/nousdesk-<版本>-linux-x64.tar.gz` | `tar -xzf` 后直接跑 `nousdesk/nousdesk`；二进制旁边放一份 `desktop.toml` 就是这份安装的配置 |
| `dist/nousdesk_<版本>_amd64.deb` | `sudo apt install ./…deb`：装到 `/usr/bin/nousdesk`，带 `.desktop` 菜单项和 hicolor 图标 |

打 tag `v*` 时 `.github/workflows/linux.yml` 跑同一份脚本并把两个包发到 Release；手动 `workflow_dispatch` 也能出包（不发 Release）。
包里不带宿主地址：不给 `NOUS_HOST` 就带一份 `desktop.toml.example`，内网地址不进公共产物。

## 结构

```
crates/nous-engine/   协议客户端：/v1/* 的 serde 类型 + SSE 流。UI 一行都不许进。
crates/nousdesk/      GPUI 应用：三栏布局、会话、输入、置顶。
```

分层是硬的：UI 层不碰 HTTP，协议层不碰 GPUI。以后想换 UI（Slint / 别的）只动 `nousdesk`。

## 三条规矩

1. 引擎地址只从 `resolve` 拿，不写进配置、不写进常量。
2. 协议是哑的：不加业务字段、不在客户端算状态。字段变化先改 `nous-engine`。
3. 置顶的真相在 bjes6b，桌面端跨块写，不自己存一份。

详情见 `AGENTS.md`（给 agent 的）和 `wire.md`（协议契约）。

## New Manual（「盘面」）

点左栏右上角的「+ New Manual」——**只顶替右侧聊天区那一格**，左导航和会话列表
永远在。Esc 或点左栏任意一条会话就回到聊天。外形照 qrd3wh 的面板页
`/p/<board>/hb7k3j/`（Pele / Flexoki dark）：四块象限 + 中间一颗虚线椭圆，
模型 / 上下文 / 扩展 / 权限各占一角，建会话的参数全在椭圆里。右格里**没有顶栏**。

- `crates/nousdesk/src/board.rs` —— 状态 + 渲染 + 五个弹层
- `crates/nousdesk/src/board_art.rs` —— 调色板、cqmin 尺寸、那张 Cynefin 图的路径
- 四块象限和 cliff 用 `PathBuilder` 现画（`canvas` + `window.paint_path`）：
  `svg()` 元素只走单色 alpha 蒙版，四块色会糊成一块
- 尺寸全走**右格 cqmin**（`art::M`）：右格短边 = `min(右格宽, 窗高)`，
  右格宽由 `app.rs` 里那几个栏宽常量算出来。按整窗 vmin 算会在窄右栏里挤爆。
  右格变了跟着缩，一屏不滚 —— 和那个页面一样
- 模型那一格是三列**铁路图**（供应商 › 模型 › 思考），技能 / 模板 / 目录是平铺清单：
  前者是有依赖的语法链，后者不是。为版式统一把语义搞错不划算

调试两个开关（只为了不用每次手点）：

```bash
NOUS_OPEN_BOARD=1 ./target/debug/nousdesk              # 一启动就开在右格的盘面上
NOUS_OPEN_BOARD=1 NOUS_OPEN_SHEET=model ./target/debug/nousdesk    # 再直接开在某个弹层上
# sheet ∈ model | skills | prompt | dir | machine
NOUS_DESKTOP_OPEN=settings ./target/debug/nousdesk                 # 一启动就开在左栏底部「≡」的设置页
# menu | settings
```
