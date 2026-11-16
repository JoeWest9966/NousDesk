#!/usr/bin/env python3
"""把 index.html + fonts.css 打成一个自包含的 preview.html（字体 subset 成 woff2 内嵌）。

用法：python3 demo/sound-settings/build-preview.py
输出：demo/sound-settings/preview.html（双击就开，无外部依赖、无 fetch）
"""
import base64
import pathlib
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
SRC = HERE / "index.html"
OUT = HERE / "preview.html"

FACES = [
    ("Inter", 400, "inter-400.ttf"),
    ("Inter", 500, "inter-500.ttf"),
    ("Inter", 600, "inter-600.ttf"),
    ("JetBrainsMono", 400, "jetbrainsmono-400.ttf"),
]

# 只需要拉丁基本区 + 常用标点；中文交给系统 CJK 字体（Noto Sans CJK 一个就二十多兆，塞不进来）。
UNICODES = "U+0020-007E,U+00A0-00FF,U+2010-2015,U+2018-201D,U+2026,U+2027,U+2030,U+2039-203A,U+2044,U+20AC,U+2122,U+2190-2193,U+2260,U+2264-2265,U+25A0-25CF"


def face(css_family: str, weight: int, filename: str) -> str:
    src = REPO / "assets" / "fonts" / filename
    with tempfile.NamedTemporaryFile(suffix=".woff2") as tmp:
        subprocess.run(
            [
                sys.executable, "-m", "fontTools.subset", str(src),
                f"--output-file={tmp.name}",
                "--flavor=woff2",
                f"--unicodes={UNICODES}",
                "--layout-features=kern,liga,calt,tnum",
                "--no-hinting",
                "--desubroutinize",
            ],
            check=True, capture_output=True,
        )
        b64 = base64.b64encode(pathlib.Path(tmp.name).read_bytes()).decode()
    return (
        f"@font-face{{font-family:'{css_family}';font-weight:{weight};font-style:normal;"
        f"font-display:block;src:url(data:font/woff2;base64,{b64}) format('woff2')}}"
    )


def main() -> None:
    html = SRC.read_text(encoding="utf-8")
    marker = '<link rel="stylesheet" href="fonts.css">'
    assert marker in html, "index.html 里找不到 fonts.css 的 link"
    rules = "\n".join(face(*f) for f in FACES)
    html = html.replace(marker, f"<style>\n{rules}\n</style>")
    OUT.write_text(html, encoding="utf-8")
    print(f"{OUT}  {OUT.stat().st_size / 1024:.0f} KB")


if __name__ == "__main__":
    main()
