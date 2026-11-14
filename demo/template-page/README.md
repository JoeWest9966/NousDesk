# demo · Template 页右栏（只读层）

第二步（spec `docs/spec-template-page.md` §6）的外观稿，**不进构建**、不接 Cargo。
挑完把结论写进 `decisions.md`，再去改 `crates/nousdesk/src/templates.rs`。

## 怎么看

```bash
cd demo/template-page && python3 -m http.server 8765
# 浏览器打开 http://localhost:8765/
```

右上角三组按钮：**详情排版 A1/A2/A3**、**变化视图 B1/B2/B3**、**浅/深**。
左屏 = 详情，右屏 = 待同步 / diff，两屏组合着挑。也可以直接跳：

```
http://localhost:8765/#a2,b3,dark
```

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
