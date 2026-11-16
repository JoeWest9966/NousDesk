# 资源页 03 · 交互原型

IA 已定 **03**：一个「资源」图标，页内两段（内容 / 扩展），内容段下再分技能 / 提示词。
这份 demo 不是截图拼的对照稿 —— 是一份**真能点**的原型：真三栏、真状态机，
点段 / 点行 / 改输入框 / 新建 / 保存 / 发布都真的响应。

## 怎么看

```
/opt/NousDesk-03/demo/resource-writes/preview.html
```

单文件，字体内嵌、无外链，双击就开。右上角「镜头」面板（或按 `?`）按组列出所有镜头，
点一条就跳过去。也可以直接在地址后面加 hash，例如 `preview.html#edit-dirty`。

键盘：

| 键 | 作用 |
|---|---|
| `1` / `2` / `3` | 内容·技能 / 扩展 / 内容·提示词 |
| `n` | 新建（跟随当前段） |
| `e` | 编辑选中的条目 |
| `p` | 发布面板 |
| `?` | 收起 / 展开镜头面板 |
| `Esc` | 关对话框 / 退出整页视图 |

## 这一版为什么这么写

上一版是九张静态卡片，看下来有三个真问题，这版一个个改掉了：

1. **面板像卡片浮在白纸上** → 现在是真窗口骨架：56px 图标条 + 段/子开关 + 类别树 248 +
   列表 300 + 详情 flex，`height: 100vh` 铺满，色值全部取自 `theme.rs` 的 `Palette::DEFAULT`。
2. **数字会打架**（`NaN` / 段写「内容 11」列表却是扩展）→ 段、子开关、列表、详情、顶栏
   现在读的是同一份状态；类别树复用 `category.rs` 的 `build_tree` 规则（含后代计数、
   同一条目按祖先去重），段计数复用 `resources.rs::counts`（技能不算隐藏、提示词全算、
   扩展只算 catalog）。
3. **只有九个镜头、没有状态** → 现在 30+ 个镜头，覆盖加载中 / 一个仓挂了 / 空结果 /
   隐藏技能 / 默认提示词 / 扩展五档 / 新建的四种被拒 / 未发布 / 编辑脏了 / 没存就关 /
   发布成功 / 发布被闸卡住 / 归档与真删确认 / 窄窗。

## 跟引擎对齐的几处（别改错）

- **技能**：`description` 写在 `SKILL.md` 的 frontmatter；`nous.json` 里 `summary` 是**空串**
  （`skills-api.js:407 createSkill`）。
- **提示词**：`PROMPT.md` 是**纯正文、没有 frontmatter**；`summary` 在 `nous.json`
  （`prompts-api.js:237 createPrompt`）。新建时正文空着就落一个 `# 名字` 的标题，
  **标题也会进提示词**，所以建完直接进编辑器。
- **扩展**：清单 `nous.json` + 空壳入口 `index.js`；建完**不提交**。
  核心三条 `nous` / `skills` / `prompts` 不让建、卸载会 403（`server.js:986 guardExtensionName`）。
- **校验文案**逐字用引擎原话（`技能名不能为空`、`技能名只认小写字母…`、
  `description 不能空 —— 模型靠它判断该不该用这条技能`、
  `一句话说明不能空 —— 卡片上就靠它认人`、
  `.archive/demo 还留着 —— 点「已删除」里的「恢复」把它捡回来，别建第二个` …），
  不在这里泛化成「输入不合法」。
- **发布**：三个仓各一个按钮，不做一键全发。`/opt/Nous-extend` 当前分叉且 pull 失败，
  所以点「发布扩展」会真的失败并显示引擎原话 —— 这就是为什么不放一键全发。

## 数据

真数据来自 j1900 的 qrd3wh（2026-11-16 抓的，`data.js`）：技能 6 条（`ask-matt` 隐藏、
`code-review@123` 是加载失败的变体）、提示词 5 条（`smart-ass` 是 default、3 条 `demo-*`
未提交）、扩展 4 条 catalog + `coding-tools` 内置 + `123@123` 失败变体 + `demo` 已归档。

**哪部分是合成的**：`code-review` 的 `SKILL.md`、`smart-ass` 的 `PROMPT.md`、
`skills` 扩展的 `index.js` 是引擎真文本。其余条目的**文件树**按仓库约定补了
`references/`、`assets/` 之类的占位路径（只为把「一个技能可以有多个文件」画出来），
文件内容在 demo 里按引擎落盘形状合成，页面上标了「正文还没拉过 —— 点『编辑』才取文件内容」。
真正实现时这些都来自 `GET /v1/{kind}/files` + `GET /v1/{kind}/file`。

## 重建

```bash
cd /opt/NousDesk-03/demo/resource-writes
python3 build-preview.py --test   # 先跑 106 条纯函数断言，再出单文件
```

改 `preview.html` 是没用的 —— 它由 `index.html` + `app.css` + `app.js` + `data.js` + `vendor/fonts/`
合成。纯函数（树 / 计数 / 过滤 / 校验 / 落盘形状）都挂在 `window.P` 上，
`build-preview.py --test` 用 node 把它们全组合跑一遍。
