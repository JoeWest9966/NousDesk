# 插件页读文档 · 三套方案（Cybertruck 皮）

设置 → 关于 → 插件，右栏加「读插件文档」。正文来自引擎端点
`GET /v1/plugins/{name}/readme`（内核仓 `docs/plugins.md` 规则 7：每个插件一份 ≤300 字
中文 README，随产物发布到 `docs/plugins/`，改文档不用重编译）。

## 怎么看

双击 `preview.html`（单文件、字体已内嵌，双击就开，无端口）。右下角切换三套：

- **01 文档即主体** —— 插件名 → 一句定义 → 「什么时候用 / 不负责」两节；类型、crate、
  端点收进底部「技术细节」折叠。
- **02 技术侧栏** —— 文档在左，右侧 252px 常驻技术面板（类型 / 层 / 必需 / crate / 端点）。
- **03 右滑阅读** —— 详情页保持信息密度，右上「读文档」滑出整列阅读页（Esc / ✕ 关闭）。

## 设计依据（对齐用户当前设置，不是默认皮）

- 皮：`theme = "cybertruck"`（`~/.config/nousdesk/theme.toml`）→ `Skin::CYBERTRUCK`：
  钢灰 `#2a2d30 / #303336 / #34373a / #232628`、文字 `#e8eaec`、线 `#454a4e`、
  **一条橙** `#ff6b1a`、**全直角**（`radius_scale = 0`）。
- 字体：**Archivo** 400/500/600/700 + JetBrains Mono（`assets/fonts/`，随包字体）。
- 骨架：复刻 `settings.rs` 的真实三件套 —— 48px 头部（← 返回聊天 / 设置）、
  170px 设置导航（选中 = `accent_soft` 底 + 橙字 600）、22px 内边距的内容区；
  插件页左列表 268→292px、详情右栏。
- 文案与字数取自 2026-11-24 写进内核的 28 份 README（示例选了 sessions / env / ext 等，
  其余 11 个同形状）。

## 改 demo

`preview.template.html` 是源文件，`preview.html` 由它注入内嵌字体（ttf → woff2）生成：

```bash
python3 - <<'PY'
import base64, pathlib
from fontTools.ttLib import TTFont
out = pathlib.Path('/tmp/fonts-woff2'); out.mkdir(exist_ok=True)
faces = []
for name, weight in [('archivo-400',400),('archivo-500',500),('archivo-600',600),('archivo-700',700),
                     ('jetbrainsmono-400',400),('jetbrainsmono-500',500)]:
    f = TTFont(pathlib.Path('assets/fonts')/f'{name}.ttf'); f.flavor = 'woff2'
    dst = out/f'{name}.woff2'; f.save(dst)
    family = 'Archivo' if name.startswith('archivo') else 'JBMono'
    b64 = base64.b64encode(dst.read_bytes()).decode()
    faces.append(f'@font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{b64}) format("woff2"); font-weight: {weight}; font-display: swap; }}')
tpl = pathlib.Path('demo/plugin-doc/preview.template.html').read_text(encoding='utf-8')
pathlib.Path('demo/plugin-doc/preview.html').write_text(tpl.replace('__FONTS__', "\n".join(faces)), encoding='utf-8')
PY
```

demo 不进构建、不碰 `Cargo.toml`；选定方案后写进 `decisions.md` 再动 Rust。
