#!/usr/bin/env python3
"""内嵌字体 + data.js，生成可双击的单文件 preview.html。"""
import base64
import pathlib
import re

here = pathlib.Path(__file__).resolve().parent
html = (here / "index.html").read_text(encoding="utf-8")


def font(m):
    b64 = base64.b64encode((here / m.group(1)).read_bytes()).decode()
    return f'url(data:font/woff2;base64,{b64}) format("woff2")'


out = re.sub(r'url\((fonts/[^)]+)\) format\("woff2"\)', font, html)
data = (here / "data.js").read_text(encoding="utf-8").replace("</", "<\\/")
out = out.replace('<script src="data.js"></script>', f"<script>\n{data}\n</script>")
left = re.findall(r'(?:src|href)="(?!data:|#)([^"]+)"', out)
(here / "preview.html").write_text(out, encoding="utf-8")
print("preview.html", len(out.encode()), "bytes; 外链:", left or "无")
