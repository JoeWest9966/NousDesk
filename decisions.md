# decisions — 选了什么、为什么、什么时候该反悔

追加式，一行一条。新决定加在**顶部**，别改写旧条目。

---

## 2026-11-10 · 左栏底部「≡ 更多」菜单 + 全局设置占右栏整页

**为什么**：聊天应用需要一个「不占版面」的全局入口，微信桌面版的做法是窄条底部一个三横杠。
设置页占**右栏整页**（左栏列表不动），改连接和翻会话可以交替做，不用开第二个窗口。
跟输入区那颗 DNA 图标分得清：**那颗改的是「这条对话」，这个改的是「连着谁」**。
**做了什么**：`crates/nousdesk/src/settings.rs`。菜单只放真能点的条目（设置 / 重新连接 /
打开配置文件夹 / 版本），**不摆「消息通知」「快捷键」「退出登录」这种假入口**；设置里只有
`host` 和 `board`，引擎地址仍是只读展示（`resolve` 拿的）。
**保存写哪份**：写「当前生效的那份」配置；一份都没有才新建 `%APPDATA%\nousdesk\desktop.toml`。
不能永远写 `%APPDATA%` —— 便携包 exe 旁那份优先级更高，写了等于没写。
**反悔点**：菜单要加条目时，没实现的别先摆上去；设置页要加第三个分组时先想清楚
它是不是该留在这一页（这里的真相是配置文件，不是应用状态）。

## 2026-11-09 · 通讯录改成「段 + 类别树」两栏

**为什么**：通讯录原来只有一栏平铺模板，类别（`category`）一个像素都没用上 —— 而盘上已经有
「工程 / 写作 / 写作/周报 / Basics」，`/v1/presets/*` 也早就把 `categories` 和 `repo` 返回了。
模板会长到几十条，平铺就没法找了。
**做了什么**：聊天页那栏不动，通讯录变成**两栏**：左 224 类别树（含段切换、计数、折叠、段脚注），
右 316 条目列表（头像 + 名字 + 摘要 + 类别 chip + 状态）。三条跟着定下来：

- **段在树顶切**（`Session-Template` / `Ultimate-Template`），换段 = 换词表 + 换条目 + 换仓状态。
  段按钮下面只写条数，不写「真实 / 演示」这类来源字。
- **多类别两处都出现**（不做「主类别归属」）：`deep-dive` 挂在工程和写作，两边都看得见。
  数据本来就是多值的，藏起来等于客户端替数据做决定。
- **零计数的类别灰着露出**、未分类单独一行垫底（不藏，建了看不见比灰着更糟）。
- 切段时**类别位置能留就留**：新段也有同一个路径就停在那儿，没有才回「全部」。

**反悔点**：条目多到一屏放不下、或者类别开始出现三层以上，就把树换成可搜索的（现在只有折叠）。
**代价**：类别计数是客户端算的（引擎不给）。算法钉在 `templates.rs::facet_tree` 的测试里：
含后代、同一条预设按路径去重。**它是派生量，不是第二份事实源** —— 别为它加缓存、别存盘。

## 2026-11-09 · 终极模板那一段先只读

**为什么**：`Ultimate-Template` 一条 = 触发器 + 参数 + 引用的会话模板。**点它该发生什么协议里没有**：
是启用/停用触发器（改 `enabled`），还是立刻试跑一次？两个动作的端点、幂等、权限都不一样。
客户端自己发明一个，等于把「协议是哑的」那条规矩破在第一个用例上。
**做了什么**：段切换、类别树、条目形状、`⚡ trigger` / `→ session` 两颗 chip 全做了，但**行不接点击**。
**反悔点**：引擎定了启用/试跑的端点那天，接上就行 —— 版面不用动。

## 2026-11-09 · 宿主地址从 tailnet 换回内网

**为什么**：走 tailnet 的那台在自家局域网里绕远、还多一个依赖；`192.168.1.7` 是同一台机器
（j1900）的局域网地址，直连更快也更少一层会坏的东西。
**做了什么**：`DEFAULT_HOST` 改 `http://192.168.1.7:4790`；README / AGENTS 跟着改；
CI 的 secret `NOUS_HOST` 也换成这个（发布包里的 `desktop.toml` 由它生成）。
**反悔点**：这是「本项目的机器」的地址，公库里的默认值对别人没意义 —— 换机器/换网段时
要同步改 `config.rs`、README、AGENTS 和那个 secret，四处。

## 2026-11-08 · 仓库改为公开（MIT）

**为什么**：公库的 Actions 分钟数免费 —— Windows release build 每次 20 分钟，私有库是实打实计费；
这个项目也没有必须藏的东西。
**做了什么**：删掉硬编码的私有宿主地址（默认 host 改 `http://127.0.0.1:4790`）、
测试里的真实 URL/绝对路径换假、补 MIT LICENSE、**改写 git 历史并删库重建**
（force push 不够：GitHub 不回收对象，旧 commit 靠 SHA 仍可访问）。
**反悔点**：真要藏什么的那天，别指望删文件 —— 得换仓库。

## 2026-11-08 · UI 里跑 HTTP 的桥：一个 tokio runtime 挂全局

**为什么**：GPUI 的 executor 不是 tokio，而 nous-engine 的 HTTP（reqwest）全在 tokio 上。
main 里起一个 runtime、Handle 存 `OnceLock`，UI 的 `cx.spawn` 里用 `handle.spawn(...)` 跑 HTTP，
结果回传再 `cx.notify()`。
**反悔点**：以后要高频请求（SSE 重连、轮询），统一从 `crate::TOKIO` 走，别到处开新 runtime。

## 2026-11-08 · 用 GPUI + gpui-kit，不用 Tauri / Slint

**为什么**：原生、无 webview，`gpui-kit` 0.9.0 已上 crates.io，Apache-2.0，生产可用
（Longbridge Pro 在跑），组件里有 `bubble` / `message` / `stream-markdown` / `virtual_list` / `dock`。
**反悔点**：Linux 端中文输入法（ibus / fcitx）验下来真的不能用，再退回 Tauri。
**当时放弃的**：Tauri（webview，不真原生）；Slint（自绘，文本排版 / IME 全是自己写，工作量 3-4 倍）。

## 2026-11-08 · 这是独立项目，不是块页面

**为什么**：桌面客户端是常驻应用（托盘、通知、全局快捷键、关窗不退），不是浏览器里的一张网页。
宿主「打开」是开标签页，在这里跑不了。
**反悔点**：将来要一个跨平台又不想碰原生 build 时，回浏览器方案（bjes6b 加个 `/d/`）。
**代价**：文档页（`__nous/doc`）是这个块的正门，不是应用本体。

## 2026-11-08 · 不建 BFF，客户端直连引擎

**为什么**：原生应用不是宿主子进程，没有免费的反代。多一层 BFF 就是多一个进程要管、
多一层会漂的协议。
**反悔点**：引擎给客户端的形状越来越丑 / 需要聚合多端点时，加一个薄 BFF，但只做转发。
**代价**：`/v1/*` 的线协议是硬耦合，我们成了引擎的第三个消费者（bjes6b、3699b9、我们）。

## 2026-11-08 · 置顶的单源在 bjes6b

**为什么**：不搞两份互不相干的置顶表。桌面端跨块写 `POST /p/demo/bjes6b/api/sessions/:id/pin`，
地址从 `resolve` 拿。坏了 404 响得很明显，好过悄悄分叉。
**反悔点**：bjes6b 那个接口成了私有依赖、老变，就改成桌面端自己存 `~/.config/nousdesk/pins.json`，
并在这里写明「两个端置顶不共享，故意的」。

## 2026-11-12 · 只做 Linux，Windows 冻结成存档

**为什么**：手头的机器全换成 Linux 了（Debian 13 + GNOME Wayland，AMD 核显）。同时维护两条线，
结果是两边都只验一半；而且 Windows 那条线的验证成本（真机 + WiX 下载偶发 504）已经压过了收益。

**做了什么**：

- 发布路径改成 Linux：`cargo build --release -p nousdesk` → `target/release/nousdesk`，
  旁边放一份 `desktop.toml` 打成 `nousdesk-linux-x64.tar.gz`。`.deb` 等真需要再上（打包脚本比代码脆）。
- Windows 的东西**一行不删**：`.github/workflows/windows.yml`、`[package.metadata.packager]`、
  `%APPDATA%\nous` 的兼容读全留着。workflow 只摘掉 `tags: v*` 的触发 —— 源码保留、出包手动。
- 冻结点：最后一个 Windows 可构建状态打 tag `windows-last`（回滚 `git switch windows-last` 就行）。

**代价**：Wayland 上更脆的三件事（中文输入法 / 剪贴板 / 字体回退）得在这边重新踩一遍，`坑.md` 里的 Windows
条目不删（那是历史证据，也是将来重启这条线的起点）。

**反悔点**：真有人要 Windows 版时，先把 `windows.yml` 的 `tags: ["v*"]` 加回来，再从 `windows-last`
往前补验证；`README.md` / `AGENTS.md` 里的平台段一起改。

## 2026-11-12 · Linux 打包用 tar.gz + dpkg-deb，不用 cargo-packager

**为什么**：`cargo-packager` 的 Linux 格式（deb/appimage）还是实验性，msi/nsis 才是它的主场；
而 `tar -czf` + `dpkg-deb --build` 各二十行、能读能改，产物一眼看得懂。
出包脚本放仓库里（`scripts/package-linux.sh`），CI 只负责装依赖、跑测试、调它。

**做了什么**：便携 tar.gz（二进制 + `desktop.toml.example` + README/LICENSE + 图标/desktop 文件）、
`.deb`（`/usr/bin/nousdesk` + `.desktop` + hicolor 256 图标 + doc）；`[profile.release] strip = "symbols"`
把 66MB 砍到 47MB。宿主地址走 `NOUS_HOST` 环境变量，公共产物里不带内网 IP。

**代价**：依赖列表是手写的（`libvulkan1` / `libegl1` / `libwayland-client0` 都是运行期 dlopen，
不写死会白屏）；`Installed-Size` 也得自己算。真出问题要拿 `dpkg-shlibdeps` 校一遍，那一步还没做。

**反悔点**：要发 arm64 / AppImage / Flatpak 时，脚本拆成 `package-linux.sh`（共用）+ 每种格式一个函数，
不要把分支堆进一个脚本里；不再用打包脚本而改回 cargo-packager 时，先量它的 Linux 输出到底能不能用。

## 2026-11-13 · Linux 自绘标题栏（用 gpui-kit 的 TitleBar）

**为什么**：GNOME/Wayland **不给 server-side decorations**，GPUI 拿不到 SSD 就退回 client 侧，
而退回之后它自己不画任何东西 —— 结果窗口没有标题、没有最小化/最大化/关闭，只能靠
GNOME 概览或快捷键关掉。这是 Linux 版最显眼的一处「不像个应用」。

**做了什么**：`crates/nousdesk/src/chrome.rs`，只在 `target_os = "linux"` 编译；
`WindowOptions` 在 Linux 上显式要 `WindowDecorations::Client` 并把标题栏交给
`TitleBar::window_options()`，窗口标题元数据仍写 `NousDesk`（任务栏/概览读的是它）。
Windows 那条线原样用系统标题栏。

**为什么不自己拿 div 拼**：拖动（`start_window_move`）、双击最大化、右键窗口菜单、
三个按钮的点击、以及「WM 到底支持哪些按钮」（`window.window_controls()`）都得自己对一遍；
`TitleBar` 已经把这套和「当前是不是 client 装饰」的判断做完了 —— 在 X11 有 WM 给 SSD 时
它会自动不画按钮，避免出现两个关闭键。

**代价**：窗口圆角、阴影、边框都得自己画（GPUI 的 CSD 不给）；X11 无 WM（比如裸 Xvfb）
时装饰报告成 Server，按钮不渲染 —— 那是预期行为，不是 bug。HiDPI 下的标题栏高度
（34px 是逻辑像素）还没在 150% 缩放下量过。

**反悔点**：什么时候 gpui-kit 的 Linux CSD 自己画标题栏了，就删掉 `chrome.rs` 挂回去；
或者要走 GNOME 的 adwaita 风格，改用 libadwaita 的 window controls 布局（那要重新量尺寸）。
