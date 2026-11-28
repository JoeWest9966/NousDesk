#!/usr/bin/env python3
"""把 index.html 的字体和真机截图 base64 内嵌，生成可双击的单文件 preview.html。"""
import base64
import pathlib
import re

here = pathlib.Path(__file__).resolve().parent
html = (here / "index.html").read_text(encoding="utf-8")


def b64(m, mime):
    data = (here / m.group(1)).read_bytes()
    return f'url(data:{mime};base64,{base64.b64encode(data).decode()})'


# 字体
html = re.sub(r'url\((fonts/[^)]+)\) format\("woff2"\)',
              lambda m: b64(m, "font/woff2") + ' format("woff2")', html)


# 图片：src="x.png" → data URI
def img(m):
    quote, name = m.group(1), m.group(2)
    mime = "image/png" if name.endswith(".png") else "image/jpeg"
    data = (here / name).read_bytes()
    return f'src={quote}data:{mime};base64,{base64.b64encode(data).decode()}{quote}'


html = re.sub(r'src=(["\'])([^"\']+\.(?:png|jpg|jpeg))\1', img, html)

leftover = re.findall(r'(?:src|href)="(?!data:|#)([^"]+)"', html)
(here / "preview.html").write_text(html, encoding="utf-8")
print("preview.html", len(html.encode()), "bytes; 外链:", leftover or "无")
