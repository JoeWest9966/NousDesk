# 资源页 03 · 写操作落地稿

IA 已定 **03**（一个「资源」图标，两段：内容 / 扩展），见 `feat/resource-pages`
那条线的 `decisions.md` 2026-11-16 条目和 `resources.rs` 的只读第一刀。
这份 demo 是把**你采纳的四条写操作建议**画成可点镜头，确认观感后才动 Rust / `nous-engine`。

## 怎么看

```
/opt/NousDesk-03/demo/resource-writes/preview.html
```

单文件，双击就开。右边栏点镜头，或用 URL hash 直接定位，例如
`preview.html#edit`。九个镜头：

| 镜头 | 看什么 |
|---|---|
| `#new-skill` | 新建技能：必填 `description`，右边预览 `nous.json` |
| `#new-prompt` | 新建提示词：必填 `summary`（写 `nous.json`），`PROMPT.md` 是纯正文 |
| `#new-ext` | 新建扩展：必填说明 ≤ 200，核心三条不让用，预览 `nous.json` |
| `#created` | 建完停在「未发布」并选中新条目，不自动发布；提示词建完进编辑器 |
| `#edit` | 编辑是整页工作台：文件树 + 正文 + 状态，不是右栏抽屉 |
| `#publish` | 三个仓各一个发布按钮，不做一键全发 |
| `#readonly-*` | 03 只读骨架的三个段（技能 / 提示词 / 扩展），跟第一刀一致 |

## 四条各落在哪

1. **三张表各说各的必填 + 实时预览落盘内容**：`#new-skill` / `#new-prompt` / `#new-ext`。
   技能写 `SKILL.md`（frontmatter 里 `description` 必填）+ `nous.json`（只有 name/summary:""/category）；
   提示词写 `PROMPT.md`（纯正文，**无 frontmatter**）+ `nous.json`（`summary` 必填）；
   扩展写 `nous.json`（说明 ≤ 200、核心三条不让用）。改一个字预览跟着变。
2. **建完停在未发布 + 选中新条目**：`#created`。琥珀「未发布」chip，不自动发布。
   提示词的「建完进编辑器」是同样流程 —— 正文就是编辑器里的内容。
3. **发布三个仓各走各的**：`#publish`。技能 / 提示词 / 扩展各一个按钮；
   `/opt/Nous-extend` 当前分叉且 pull 失败，一键全发会卡死在这里拖累另两个。
4. **编辑是整页工作台**：`#edit`。占满主内容区，不是盖在列表上的右栏抽屉。

## 数据 / 样式

真数据沿用 j1900 引擎（2026-11-16），样式方言 / 字体 / 图标沿用 `resource-page` 的冻结素材
（放在 `vendor/`，demo 不进构建、不碰 `Cargo.toml`）。

## 重建

```bash
cd demo/resource-writes
python3 build.py   # vendor/base.html + data.js → index.html + preview.html
```
