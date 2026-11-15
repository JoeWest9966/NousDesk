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


## 完整原型（app.html）—— 功能验收稿

```
/opt/NousDesk-tpl/demo/template-page/app-preview.html
```

三栏完整界面 + 6 套皮肤（石墨 / 纸 / 单色 / 冷钢 / 琥珀 / 工业），功能都在：

- 左：图标条、段切换、类别树（含层级与零计数）、新建 / 新建类别
- 中：搜索、视图切换、卡片（未发布 / 坏了的标记）、待同步入口
- 右：**详情 / 盘面 / JSON / 对比** 四个 tab；底部分布发布、编辑、⋯ 菜单
- 弹层：发布（文件清单）、归档、彻底删除（不可逆）、开对话（会话名 + 生效值）、新建类别
- 新建：右栏整页表单（名字校验实时反馈、类别提示、起手二选一）

hash 直达：`#皮肤,tab,弹层,段`
`#graphite,board` `#paper,json` `#industrial,detail,publish` `#steel,detail,new` `#amber,detail,launch` `#graphite,diff,,ut`

`app.html` 是源码（改样式改这份），`app-preview.html` 是生成的自包含单文件。

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
