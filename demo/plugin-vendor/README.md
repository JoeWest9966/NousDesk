# 插件列表副标题 = 供应商 · 四套方案（mono 皮）

设置 → 关于 → 插件。当前列表每行是「英文模块名」+「英文模块名」两遍（`plugin_title()`
直接透传模块名，副标题又写了一遍 `module.name`），看起来像坏了。这一版把**第二行换成
供应商（出品方 vendor）**，数据来自 `/v1/capabilities` 的 `modules[].vendor`
（内核 `plugin.toml` 的 `vendor`，规则 10：`nous` / `data` / `element`）。

## 怎么看

双击 `preview.html`（单文件、字体已内嵌，双击就开，无端口）。右下角切换四套：

- **A 纯供应商** —— 第二行只有 `nous` / `data` 原文，mono 10px muted；没有
  `plugin.toml` 的模块给 `—`。行高不变、列表不跳。
- **B 出品方 · 供应商** —— 同一值加中文前缀「出品方 · data」，一眼知道这行是什么，
  代价是多 4 个字宽。
- **C 供应商胶囊** —— 供应商做成 1px 边框小胶囊，层级最清楚，列表密度略高。
- **D 同行弱化** —— 供应商挪到模块名前面当弱前缀，副标题让给启用状态；省一行高度。

## 数据是真的

九行取自 `docs/plugins.md` 主表里真实存在的模块，含三个边界样本：

- `data-manager` → `data`（唯一一个非 nous 的 vendor，也是这次改动的由来）
- `Element_Models` → `element`（模块名带下划线 + 大写，验证 mono 字体不串行）
- `presets` / `triggers` / `archive` → `vendor: null`（声明式 / 外挂，没有 `plugin.toml`），
  界面必须给 `—`，不许编一个 `nous` 顶上

## 设计依据

- 皮：`~/.config/nousdesk/theme.toml` → `theme = "mono"`（`Skin::MONO`）：纯黑底
  `#000000`、白字 `#ffffff`、白线、`radius_scale = 0` 全直角、`density = 1.05`。
- 字体：Inter 400/500/600 + JetBrains Mono 400/500（`assets/fonts/`，随包字体）。
- 骨架复刻 `settings.rs`：48px 头部、170px 设置导航（选中 = `accent_soft` 底 + 白字 600）、
  22px 内边距内容区、插件列表 292px + 详情右栏。
- 列表行与 `plugin_row()` 同构：状态点 6px → 名称 + 副标题（2px 间距）→ 右侧版本 + 端点数。

## 改 demo

`preview.template.html` 是源文件，`preview.html` 由它注入内嵌字体生成：

```bash
python3 - <<'PY'
import base64, pathlib
from fontTools.ttLib import TTFont
out = pathlib.Path('/tmp/fonts-woff2'); out.mkdir(exist_ok=True)
faces = []
for name, weight in [('inter-400',400),('inter-500',500),('inter-600',600),
                     ('jetbrainsmono-400',400),('jetbrainsmono-500',500)]:
    f = TTFont(pathlib.Path('assets/fonts')/f'{name}.ttf'); f.flavor = 'woff2'
    dst = out/f'{name}.woff2'; f.save(dst)
    family = 'Inter' if name.startswith('inter') else 'JBMono'
    b64 = base64.b64encode(dst.read_bytes()).decode()
    faces.append(f'@font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{b64}) format("woff2"); font-weight: {weight}; font-display: swap; }}')
tpl = pathlib.Path('demo/plugin-vendor/preview.template.html').read_text(encoding='utf-8')
pathlib.Path('demo/plugin-vendor/preview.html').write_text(tpl.replace('__FONTS__', "\n".join(faces)), encoding='utf-8')
PY
```

demo 不进构建、不碰 `Cargo.toml`；选定方案后写进 `decisions.md` 再动 Rust。
