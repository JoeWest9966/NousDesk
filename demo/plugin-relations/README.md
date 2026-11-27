# 插件页 · 模块关系（依赖 / 被依赖）· 三套方案（mono 皮）

设置 → 关于 → 插件，详情里加「它依赖谁、谁依赖它」。数据来自新端点
`GET /v1/modules/graph`（内核 `AboutME_ModuleRelationshipNetwork` 插件，2026-11-26 已并入
`main`），形状 `{generatedFor, nodes:[{name,crate}], edges:[{from,to,via}], unmapped}`。
`capabilities.modules` 里没有 `requires`，所以这张图是**唯一的原料**；本 demo 用同一份真数据
（32 个模块 / 97 条边，抄自内核仓各 `plugin.toml` 的 `[requires]`）。

## 怎么看

双击 `/opt/NousDesk/demo/plugin-relations/preview.html`（单文件、字体已内嵌、无端口）。

右下角两个开关：

- **关系怎么摆**：03 套方案，各有自己的详情观感；点胶囊 / 点图上的名字 / 点表格行都会
  **真的跳模块**（左侧列表跟着高亮）。
- **引擎**：`有 /v1/modules/graph` ↔ `老引擎（没有这张图）` —— 看降级长相。

## 三套

- **01 两列清单** —— 详情里两行胶囊：「依赖 · n」「被依赖 · n」，名字可点跳转。最省地方，
  跟现有 `状态 / 版本 / 层 / 来源 crate` 一个骨架；读不出「谁是枢纽」，得一个个点。
- **02 迷你图** —— 名字下方一张 216px 图：选中模块居中，左=依赖、右=被依赖，折线连过去
  （白线皮里用 `--line-soft`），每侧最多 6 个、多的 `+N`。枢纽一眼看得出来，代价是多吃
  200px 竖向空间。
- **03 角标＋表格** —— 列表每行右侧多一行 `↑2 ↓11`（不点进去就能扫出枢纽），详情换成
  一张表（模块 / 方向 / 层 / 它自己的依赖数），行可点。信息最全，代价是列表行高 +14px，
  一屏少一行。

## 降级（老引擎）

没有这张图时，**整块不出现**，只留一句「这个引擎没有 `/v1/modules/graph`」；不拿「空」
冒充「没有依赖」，也不在列表上加 `↑0 ↓0`。等 Desk 内核版本判定（`capability.rs`）到位后，
这条也可以改成「灰掉 + 理由」，看用户要不要在灰度期提示升级引擎。

## 设计依据（对齐用户当前设置，不是默认皮）

- 皮：`theme = "mono"`（`~/.config/nousdesk/theme.toml`）→ `Skin::MONO`：纯黑 `#000`、
  白字、`--line-soft #2a2a2a` 细线、**全直角**（`radius_scale = 0`）、body 12.5。
- 字体：**Inter** 400/500/600 + **JetBrains Mono** 400/500（`assets/fonts/`，随包字体）。
- 骨架：复刻 `settings.rs` 真实三件套 —— 38px 标题栏 + 48px 头部、170px 设置导航、
  22px 内边距内容区、插件页左列表 292px + 详情右栏；文案沿用已上线的供应商胶囊方案。
- 数据里 `启用` 一律按真值填 `启用`（本仓 32 个模块都装配了），`端点` 一栏没画 ——
  这一版只审「关系怎么摆」，别拿假端点数字分心。

## 改 demo

`preview.template.html` 是源文件，`data.json` 是真数据（由内核仓 `plugin.toml` 抄出），
`preview.html` 由脚本注入内嵌字体（ttf → woff2）和数据生成：

```bash
cd /opt/NousDesk && python3 - <<'PY'
import base64, json, pathlib
from fontTools.ttLib import TTFont
tmp = pathlib.Path('/tmp/rel-fonts'); tmp.mkdir(exist_ok=True)
faces = []
for name, family, weight in [('inter-400','Inter',400),('inter-500','Inter',500),('inter-600','Inter',600),
                             ('jetbrainsmono-400','JBMono',400),('jetbrainsmono-500','JBMono',500)]:
    f = TTFont(pathlib.Path('assets/fonts')/f'{name}.ttf'); f.flavor = 'woff2'
    dst = tmp/f'{name}.woff2'; f.save(dst)
    b64 = base64.b64encode(dst.read_bytes()).decode()
    faces.append(f'@font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{b64}) format("woff2"); font-weight: {weight}; font-display: swap; }}')
data = json.loads(pathlib.Path('demo/plugin-relations/data.json').read_text(encoding='utf-8'))
tpl = pathlib.Path('demo/plugin-relations/preview.template.html').read_text(encoding='utf-8')
pathlib.Path('demo/plugin-relations/preview.html').write_text(
    tpl.replace('__FONTS__', "\n".join(faces)).replace('__DATA__', json.dumps(data, ensure_ascii=False)),
    encoding='utf-8')
PY
```

demo 不进构建、不碰 `Cargo.toml`；**挑中哪套先说一声**，写进 `decisions.md` 再动 Rust
（`settings.rs` 的 `about_plugin_detail` + 新 `plugin_graph.rs` 客户端，只读端点、不算状态）。