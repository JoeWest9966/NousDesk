#!/usr/bin/env python3
"""v02/index.html → v02/preview.html：字体（上级 fonts/）和真机截图（上级 png）base64 内嵌。"""
import base64
import pathlib
import re

here = pathlib.Path(__file__).resolve().parent
root = here.parent
html = (here / "index.html").read_text(encoding="utf-8")


def b64(path: pathlib.Path) -> str:
    return base64.b64encode(path.read_bytes()).decode()


def font(m):
    p = (here / m.group(1)).resolve()
    return f'url(data:font/woff2;base64,{b64(p)}) format("woff2")'


html = re.sub(r'url\((\.\./fonts/[^)]+)\) format\("woff2"\)', font, html)


def img(m):
    quote, name = m.group(1), m.group(2)
    p = (here / name).resolve()
    mime = "image/png" if str(p).endswith(".png") else "image/jpeg"
    return f'src={quote}data:{mime};base64,{b64(p)}{quote}'


html = re.sub(r'src=(["\'])([^"\']+\.(?:png|jpg|jpeg))\1', img, html)
leftover = re.findall(r'(?:src|href)="(?!data:|#)([^"]+)"', html)
(here / "preview.html").write_text(html, encoding="utf-8")
print("preview.html", len(html.encode()), "bytes; 外链:", leftover or "无")
