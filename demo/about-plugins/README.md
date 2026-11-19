# 关于页 · 插件清单（4 套版面）

双击 `preview.html`（单文件，字体已内嵌，235 KB，不需要起 server）。
`index.html` + `fonts.css` 是改的那份：改完跑 `python3 build-preview.py` 重新生成 `preview.html`。

**还没落地**。用户挑完方案 → 写进 `decisions.md` → 再改
`crates/nousdesk/src/settings.rs::about_pane`。

## 这一版在定什么

不是"要不要显示插件"，是**这 11 行摆在关于页的哪里、摆多细**。
数据全是真的：faux 引擎（`/opt/Nous-src/target/debug/nous-app`）2026-11-19 的
`GET /v1/capabilities` 原样响应。

- 模块 11 个：`sessions` `documents` `triggers` `device` `skills` `prompts` `selection`
  启用（7 个，共 34 条端点）；`ext`（frozen）+ `presets` `archive` `combos`
  （not_implemented）未启用（4 个）。
- 已有客户端类型 `nous_engine::Capabilities` / `Module` 就够，**不用加请求** ——
  `self.caps` 启动时已经拿过一份，关于页直接读。
- 缺的只有一张 `module name → 中文标签` 的表（`sessions=会话` … `ext=扩展`），
  落 `capability.rs` 的 `Feature` 旁边，别散在 UI 里。

### 版面

| # | 方案 | 一句话 | 代价 |
| --- | --- | --- | --- |
| 01 | 一列清单 | 关于页还是「一列事实」，插件是最后一段；启用在上、未启用在下 | 11 行 + 原有 5 条键值要滚一屏；停用只写「未落地」，看不到引擎原话 |
| 02 | 两栏小卡 | 一屏看完，靠卡片边界分组；未启用虚线、冻结暖底 | 关于页从 520 放宽到 686，和别的设置页不一样宽；窄窗要叠回单列 |
| 03 | 概览 + 折叠 | 默认只有一行数字 + 一排名字，原因点一下才出来 | 端点数完全不露；chip 换行后不齐，扫读比列表慢 |
| 04 | 对照表 | 中文名 + 英文名 + 状态 + 端点数四列对齐，像合同清单 | 最重，表格感把关于页做成运维面板；宽 640 |

**建议 01**：关于页这一页的用途是「出问题先对版本、对地址」，插件清单属于同一类
**排查信息** —— 哪个模块没开、为什么，要能直接读到，而不是先点开。01 的节奏和上面
五条键值一致（左标签右值），窄窗也不用改宽度。02 的一屏看完更漂亮，但要把
`about_pane` 的 `max_w(520)` 单独放宽，等于这一页破一次版式规矩；03 省空间，
可排查时多点一次就是多一次"被晃"。

## 页面里那些是背景板

左栏图标条、会话列表、设置页左 tab 都是背景板，只为说明「设置占右栏整页、关于页
只有 520 宽」。四套方案共用一个关于页的表头 + 五条键值，只有插件那一段不同。

## 这份 demo 里不许当真的

- 客户端版本 `v0.9.7`、引擎 node `j1900`、地址 `http://192.168.1.7:4789` 是照线上填的
  样本值，真实现从 `self.engine_node` / `self.engine_url` / `env!("CARGO_PKG_VERSION")` 取。
- 方案 03 的折叠是 demo 里用 JS 做的；GPUI 里要真挂点击（不是 hover），否则就是
  「看着能点、点了没反应」。
- 四个方案的端点数是 capabilities 里 `endpoints` 的长度；frozen 的 `ext` 在
  capabilities 里**不带 `endpoints` 字段**（物理路由还在，但不声明），所以显示 `—`。

## 还没定

1. 中文标签表放 `capability.rs`（`Feature::label()`）还是 `settings.rs` 里一张局部表 ——
   建议前者，能力判定只有一处，标签跟着 `module_name()` 走。
2. 未启用那组的排序：现在按 capabilities 原顺序（ext 领先），如果落地时想「冻结的排最后」，
   那是 UI 排序，不碰协议。
3. 要不要顺带显示 `contract` / `server.version`（现在左栏菜单里已经有合同号）—— 建议不加，
   关于页已经有这五项。

## 口径：这里的「插件」是什么

= `GET /v1/capabilities` 的 `modules`，也就是内核 `docs/plugins.md` 里「对用户功能面说话」
的 11 个 module。内核仓那 17 个 crate 的分层清单（kernel / platform / app / dev）
**没有任何端点吐出来**；要在桌面端显示那一层，先得在引擎加端点（另一张票，不是这次）。
