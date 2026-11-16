#!/usr/bin/env python3
"""把 index.html 的 woff2 引用换成 data URI，生成可双击的单文件 preview.html。"""
import base64, pathlib, re

here = pathlib.Path(__file__).resolve().parent
html = (here / "index.html").read_text(encoding="utf-8")

def inline(m):
    data = base64.b64encode((here / m.group(1)).read_bytes()).decode()
    return f'url(data:font/woff2;base64,{data}) format("woff2")'

out = re.sub(r'url\((fonts/[^)]+)\) format\("woff2"\)', inline, html)
(here / "preview.html").write_text(out, encoding="utf-8")
print("preview.html", len(out.encode()), "bytes")
