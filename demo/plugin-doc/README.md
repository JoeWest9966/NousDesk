# 插件页读文档 · 三套方案

设置 → 关于 → 插件，右栏加「读插件文档」。正文来自引擎新端点
`GET /v1/plugins/{name}/readme`（内核仓 `docs/plugins.md` 规则 7：每个插件一份 ≤300 字
中文 README，随产物发布到 `docs/plugins/`，改文档不用重编译）。

## 怎么看

双击 `preview.html`（单文件、字体已内嵌，无端口、无依赖）。顶部切换条是三套方案：

- **A · 文档即主体**：插件详情先给 300 字正文，端点 / 类型 / crate 收进「开发者信息」折叠。
- **B · 双页签**：「文档 / 详情」两个页签，默认文档，右上角切。
- **C · 浮层阅读**：详情页保持现状，右上「读文档」开 620 宽浮层。

三套都按真字号（11 / 11.5 / 12.5 / 15）、真栏宽（左列表 268、右栏留白 18）和默认皮的
色值画（`crates/nousdesk/src/theme.rs` 的 `Palette::DEFAULT`）。

## 改 demo

`preview.template.html` 是源文件，`preview.html` 由它注入内嵌字体生成：

```bash
python3 - <<'PY'
import base64, pathlib
base = pathlib.Path('demo/template-page/fonts')
faces = [('Inter','inter-400.woff2',400),('Inter','inter-500.woff2',500),
         ('Inter','inter-600.woff2',600),('Inter','inter-700.woff2',700),
         ('JBMono','jetbrainsmono-400.woff2',400)]
rules = []
for family, fname, weight in faces:
    b64 = base64.b64encode((base/fname).read_bytes()).decode()
    rules.append(f'@font-face {{ font-family: {family}; src: url(data:font/woff2;base64,{b64}) format("woff2"); font-weight: {weight}; font-display: swap; }}')
tpl = pathlib.Path('demo/plugin-doc/preview.template.html').read_text(encoding='utf-8')
pathlib.Path('demo/plugin-doc/preview.html').write_text(tpl.replace('__FONTS__', "\n".join(rules)), encoding='utf-8')
PY
```

demo 不进构建、不碰 `Cargo.toml`；选定方案后写进 `decisions.md` 再动 Rust。
