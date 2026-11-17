#!/usr/bin/env python3
"""把 index.html 的字体和 data.js 全塞进去，生成可双击的单文件 preview.html。"""
import json
import pathlib
import re

here = pathlib.Path(__file__).resolve().parent
html = (here / "index.html").read_text(encoding="utf-8")


def font(m):
    data = (here / m.group(1)).read_bytes()
    import base64
    b64 = base64.b64encode(data).decode()
    return f'url(data:font/woff2;base64,{b64}) format("woff2")'


out = re.sub(r'url\((fonts/[^)]+)\) format\("woff2"\)', font, html)

# data.js 内嵌：JSON 里可能有 `</script>` 之外的任何东西，替换掉结尾的 `</` 更保险。
data = (here / "data.js").read_text(encoding="utf-8").replace("</", "<\\/")
out = out.replace(
    '<script src="data.js"></script>',
    f"<script>\n{data}\n</script>",
)

# 缩略参数之类没用到的话不用管；这里只保证没有外链。
leftover = re.findall(r'(?:src|href)="(?!data:|#)([^"]+)"', out)
(here / "preview.html").write_text(out, encoding="utf-8")
print("preview.html", len(out.encode()), "bytes; 外链:", leftover or "无")
