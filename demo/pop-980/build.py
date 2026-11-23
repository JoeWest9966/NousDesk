#!/usr/bin/env python3
"""生成 demo/pop-980/preview.html —— 单文件、字体 base64 内嵌、双击就开。

只讲一件事：980 宽（右格 644）下，点开某个选项的那张弹层**太小、看不清**，
三套摆法各给一个解法。盘面本身不动（就是现在合进去的竖轴 D）。
"""
import base64
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
FONTS = ROOT / "assets" / "fonts"
OUT = pathlib.Path(__file__).resolve().parent / "preview.html"


def b64(p):
    return base64.b64encode(p.read_bytes()).decode()


F = dict(
    m400=b64(FONTS / "manrope-400.ttf"),
    m500=b64(FONTS / "manrope-500.ttf"),
    m700=b64(FONTS / "manrope-700.ttf"),
    j400=b64(FONTS / "jetbrainsmono-400.ttf"),
)

SKILLS = [
    ("读文件", "read-file", "一次性把文件内容带进上下文", True),
    ("写文件", "write-file", "落盘之前会先问你一次", True),
    ("跑命令", "shell", "在引擎那台机器上执行", True),
    ("联网", "web", "抓网页正文，不走缓存", False),
    ("记忆", "memory", "跨会话记住你的偏好", False),
    ("任务清单", "todo", "自己拆步骤、勾进度", False),
    ("日程", "calendar", "只读，不外发", False),
    ("通知", "notify", "跑完给你弹一条", False),
]


def rows(n=8, cls=""):
    out = []
    for i, (name, slug, desc, on) in enumerate(SKILLS[:n]):
        out.append(
            f'<div class="row{cls}"><div class="tx"><div class="nm">{name}'
            f'<span class="slug">{slug}</span></div><div class="ds">{desc}</div></div>'
            f'<span class="box{" on" if on else ""}">{"✓" if on else ""}</span></div>'
        )
    return "\n".join(out)


RAIL = r"""<div class="rail">
  <div class="ic act"><svg viewBox="0 0 24 24"><path d="M20 12a8 8 0 1 1-3.2-6.4"/><path d="M21 4v5h-5"/></svg></div>
  <div class="ic"><svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 9h8M8 13h5"/></svg></div>
  <div class="ic"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 8v8M8 12h8"/></svg></div>
  <div class="ic"><svg viewBox="0 0 24 24"><path d="M12 4l8 4.5v7L12 20l-8-4.5v-7z"/><path d="M12 12l8-3.5M12 12v8M12 12L4 8.5"/></svg></div>
  <div class="ic"><svg viewBox="0 0 24 24"><ellipse cx="12" cy="7" rx="7" ry="3"/><path d="M5 7v10c0 1.7 3.1 3 7 3s7-1.3 7-3V7"/></svg></div>
  <div class="sp"></div>
  <div class="ic"><svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg></div>
</div>"""

LIST = r"""<div class="list">
  <div class="srow"><div class="search">⌕ 搜索会话</div><div class="plus">+</div></div>
  <div class="empty"><b>一条会话都还没有</b>点右上 ＋ 建第一条</div>
</div>"""

# 盘面：竖轴四行（被点的那一行高亮）
def axis(active="model"):
    def row(k, v, s, on=False, warn=False):
        cls = "node" + (" set" if on else "") + (" hot" if k == active else "")
        return (f'<div class="{cls}"><span class="k">{k}</span>'
                f'<div class="v{" warn" if warn else ""}">{v}</div>'
                f'<div class="s">{s}</div><span class="go">›</span></div>')
    return (row("model", "claude-sonnet-4.5", '<span class="mono">anthropic</span> · 思考 <span class="mono">off</span>', True)
            + row("extensions", "0 / 0 个扩展", "核心自带 4 个 · 其余在这里勾")
            + row("context", "smart-ass", "0 个技能 · 模板与知识", True)
            + row("permissions", "可写 · 文件 + 命令", "能改文件、能跑 shell", True))


def pane(inner, active="model"):
    return f"""<div class="pane"><div class="d">
  <div class="hd"><h2>新会话</h2><span class="esc">ESC 取消</span></div>
  <div class="axis">{axis(active)}</div>
  <div class="foot">
    <div class="name"><span class="k">name</span><input value="asi" placeholder="留空自动命名"></div>
    <div class="chips"><span class="chip on">本机</span><span class="chip">work</span><span class="chip">~/src/api</span></div>
    <div class="create">创建会话 <span class="hint">⏎</span></div>
  </div>
  {inner}
</div></div>"""


def win(inner, active="model"):
    return f"""<div class="win">
  <div class="titlebar"><span class="name">NousDesk</span><span class="grip"><i></i><i></i><i></i></span></div>
  <div class="body">{RAIL}{LIST}{pane(inner, active)}</div>
</div>"""


# ---------- A：撑满右格 ----------
A = win(f"""
<div class="scrim a"></div>
<div class="pop a">
  <div class="ph">
    <div><h3>上下文 · 技能</h3><p>勾完按「完成」· 多选</p></div>
    <span class="x">✕</span>
  </div>
  <div class="rows">{rows()}</div>
  <div class="pf"><span class="esc">Esc 关闭 · 点空白处也关</span><span class="done">完成 ›</span></div>
</div>""")

# ---------- B：底部上滑 sheet ----------
B = win(f"""
<div class="scrim b"></div>
<div class="pop b">
  <div class="grab"><i></i></div>
  <div class="ph">
    <div><h3>上下文 · 技能</h3><p>勾的是这次会话额外带哪些 · 核心自带的不在这里</p></div>
    <span class="x">✕</span>
  </div>
  <div class="srch"><span>⌕</span><span class="ph2">搜技能</span></div>
  <div class="rows">
    <div class="grp">本次已勾 3</div>
    {rows(3, " b")}
    <div class="grp">其余 5</div>
    {rows(8, " b")[rows(3, " b").__len__():]}
  </div>
  <div class="pf"><span class="esc">Esc 关闭</span><span class="done">完成 · 8 个里勾了 3 个 ›</span></div>
</div>""")

# ---------- C：整格换页 ----------
C = win(f"""
<div class="whole">
  <div class="wh"><span class="back">‹ 盘面</span><h3>上下文 · 技能</h3><span class="cnt">勾了 3 / 8</span></div>
  <div class="rows">{rows()}</div>
  <div class="pf"><span class="esc">Esc 回盘面</span><span class="done">用这 3 个 · 返回 ›</span></div>
</div>""")

HTML = r"""<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8">
<title>弹层太小 · 980 宽 · 三套解法</title>
<style>
@font-face{font-family:Manrope;src:url(data:font/ttf;base64,__M400__) format("truetype");font-weight:100 500;font-display:block}
@font-face{font-family:Manrope;src:url(data:font/ttf;base64,__M500__) format("truetype");font-weight:501 600;font-display:block}
@font-face{font-family:Manrope;src:url(data:font/ttf;base64,__M700__) format("truetype");font-weight:601 900;font-display:block}
@font-face{font-family:JetBrainsMono;src:url(data:font/ttf;base64,__J400__) format("truetype");font-weight:100 900;font-display:block}
*{box-sizing:border-box;margin:0;padding:0}
:root{--rail:#e9e6e0;--list:#f4f2ee;--bg:#f4f2ee;--panel:#fff;--fg:#14150f;--fg2:#5f5d55;
--muted:#8d8a82;--faint:#b9b6ad;--line:#14150f;--line-soft:#d8d5cd;--ok:#1d6b46}
html,body{background:#0b0b0c;color:#f4f3f1;font:14px/1.55 Manrope,"Noto Sans CJK SC",sans-serif}
body{padding:26px 26px 60px}
header{max-width:1100px;margin:0 auto 18px}
header h1{font-size:15px;font-weight:700;letter-spacing:.02em}
header p{margin-top:9px;font-size:12.5px;line-height:1.8;color:#8b8884;max-width:1000px}
header p b{color:#e8e5e0;font-weight:600}
header ul{margin:10px 0 0 16px;font-size:12.5px;line-height:1.85;color:#8b8884}
header li b{color:#ded9d2;font-weight:600}
code{font:11px JetBrainsMono,monospace;color:#9ecf96;background:#141416;padding:1px 5px;border-radius:4px}
nav{display:flex;flex-wrap:wrap;gap:8px;align-items:center;max-width:1100px;margin:0 auto 20px}
nav button{font:12px Manrope,sans-serif;color:#9a968f;background:#17171a;border:1px solid #26262a;border-radius:999px;padding:7px 14px;cursor:pointer}
nav button[aria-selected="true"]{background:#f4f3f1;border-color:#f4f3f1;color:#0b0b0c;font-weight:600}
nav .zoom{display:flex;align-items:center;gap:7px;font:11px JetBrainsMono,monospace;color:#6f6d69;margin-left:auto}
nav .zoom input{width:130px;accent-color:#8fbf88}
.stage{max-width:1100px;margin:0 auto}
.scheme{display:none;flex-direction:column;gap:12px}
.scheme.on{display:flex}
.cap{font:11.5px/1.7 JetBrainsMono,monospace;color:#8b8884}
.cap b{color:#ded9d2;font-weight:400}
.holder{width:980px;height:1168px;transform-origin:top left}
.win{width:980px;height:1168px;background:var(--bg);color:var(--fg);border-radius:10px;overflow:hidden;display:flex;flex-direction:column;font:13px/1.5 Manrope,"Noto Sans CJK SC",sans-serif;position:relative}
.titlebar{height:34px;flex:none;display:flex;align-items:center;gap:8px;padding:0 12px;background:var(--list);border-bottom:1px solid var(--line-soft)}
.titlebar .name{font:600 11.5px Manrope;letter-spacing:.02em}
.titlebar .grip{display:flex;gap:6px;margin-left:auto}
.titlebar .grip i{width:9px;height:9px;border-radius:50%;background:#fff;border:1px solid var(--line-soft)}
.body{flex:1;display:flex;min-height:0}
.rail{width:56px;flex:none;background:var(--rail);display:flex;flex-direction:column;align-items:center;padding:14px 0 12px;gap:6px}
.rail .ic{width:34px;height:34px;border-radius:9px;display:flex;align-items:center;justify-content:center;color:#6b6860}
.rail .ic svg{width:19px;height:19px;stroke:currentColor;fill:none;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round}
.rail .ic.act{background:#fff;color:var(--fg);box-shadow:0 0 0 1px var(--line-soft)}
.rail .sp{flex:1}
.list{width:280px;flex:none;background:var(--list);border-right:1px solid var(--line-soft);display:flex;flex-direction:column;padding:13px 14px;gap:10px}
.list .srow{display:flex;gap:7px}
.list .search{flex:1;height:30px;border:1px solid var(--line-soft);border-radius:7px;display:flex;align-items:center;gap:6px;padding:0 9px;color:var(--muted);font-size:12px;background:var(--panel)}
.list .plus{width:30px;height:30px;border-radius:7px;background:var(--fg);color:#fff;display:flex;align-items:center;justify-content:center;font-size:15px}
.list .empty{margin-top:8px;color:var(--muted);font-size:12px;line-height:1.75}
.list .empty b{display:block;color:var(--fg2);font-weight:500;margin-bottom:2px}
.pane{flex:1;min-width:0;background:var(--bg);position:relative;overflow:hidden}
/* ---- 盘面（竖轴，跟线上一致） ---- */
.d{padding:30px 34px 26px;display:flex;flex-direction:column;height:100%}
.d .hd{display:flex;align-items:baseline;gap:10px}
.d .hd h2{font:700 21px/1.2 Manrope;letter-spacing:-.01em}
.d .hd .esc{margin-left:auto;font:10.5px JetBrainsMono,monospace;color:var(--faint)}
.d .axis{margin-top:10px;position:relative;flex:1;display:flex;flex-direction:column;min-height:0}
.d .axis::before{content:"";position:absolute;left:5px;top:0;bottom:0;width:1px;background:var(--line-soft)}
.d .node{flex:1;position:relative;padding:0 12px 0 30px;display:flex;flex-direction:column;justify-content:center;border-radius:9px}
.d .node::before{content:"";position:absolute;left:0;top:50%;margin-top:-6px;width:11px;height:11px;border-radius:50%;background:var(--bg);border:1.5px solid var(--line)}
.d .node.set::before{background:var(--fg)}
.d .node.hot{background:#fff}
.d .k{font:10.5px/1 JetBrainsMono,monospace;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}
.d .v{margin-top:6px;font:600 19px/1.25 Manrope;letter-spacing:-.012em}
.d .v.warn{color:#a8281c}
.d .s{margin-top:3px;font-size:12px;color:var(--muted)}
.d .s .mono{font-family:JetBrainsMono,monospace;font-size:11.5px}
.d .go{position:absolute;right:0;top:14px;color:var(--faint);font-size:16px}
.d .foot{margin-top:auto;border-top:1px solid var(--line-soft);padding-top:16px;display:flex;flex-direction:column;gap:12px}
.d .name{display:flex;align-items:center;gap:10px}
.d .name .k{flex:none}
.d .name input{flex:1;border:0;border-bottom:1px solid var(--line-soft);background:transparent;font:500 15px Manrope;color:var(--fg);padding:5px 0;outline:none}
.d .chips{display:flex;gap:7px}
.d .chip{font:11.5px JetBrainsMono,monospace;color:var(--fg2);border:1px solid var(--line-soft);border-radius:999px;padding:4px 11px;background:var(--panel)}
.d .chip.on{background:var(--fg);color:#fff;border-color:var(--fg)}
.d .create{margin-top:4px;height:54px;border-radius:10px;background:var(--fg);color:#fff;display:flex;align-items:center;justify-content:center;gap:10px;font:600 14.5px Manrope}
.d .create .hint{font:10.5px JetBrainsMono,monospace;color:#9a978f}
/* ---- 弹层通用 ---- */
.scrim{position:absolute;inset:0}
.scrim.a{background:rgba(5,5,6,.20)}
.scrim.b{background:rgba(5,5,6,.34)}
.scrim.c{background:rgba(244,242,238,.80)}
.pop{position:absolute;background:var(--panel);display:flex;flex-direction:column;overflow:hidden}
.ph{padding:18px 20px 15px;border-bottom:1px solid var(--line-soft);display:flex;align-items:flex-start;gap:10px;flex:none}
.ph h3{font:700 19px/1.2 Manrope;letter-spacing:-.01em}
.ph p{margin-top:6px;font-size:12.5px;color:var(--muted)}
.x{margin-left:auto;font-size:13px;color:var(--muted);line-height:1}
.rows{overflow:hidden}
.row{display:flex;align-items:center;gap:14px;padding:15px 20px;border-bottom:1px solid #efeee9}
.row .tx{min-width:0}
.nm{font:600 15.5px/1.35 Manrope}
.nm .slug{margin-left:8px;font:11px JetBrainsMono,monospace;color:var(--faint);font-weight:400}
.ds{margin-top:4px;font-size:12.5px;color:var(--muted)}
.box{width:19px;height:19px;flex:none;margin-left:auto;border:1.5px solid var(--line);border-radius:4px;display:flex;align-items:center;justify-content:center;font-size:12px}
.box.on{background:var(--fg);color:#fff;border-color:var(--fg)}
.pf{flex:none;padding:13px 20px;border-top:1px solid var(--line-soft);display:flex;align-items:center;justify-content:space-between}
.esc{font:11px JetBrainsMono,monospace;color:var(--faint)}
.done{font:600 13px Manrope;padding:7px 14px;border-radius:7px;border:1px solid var(--fg);background:rgba(20,21,15,.06)}
/* ---- A ---- */
.pop.a{left:16px;right:16px;top:352px;border-radius:12px;border:1px solid var(--line);box-shadow:0 18px 46px rgba(20,21,15,.16)}
/* ---- B ---- */
.pop.b{left:0;right:0;bottom:0;height:918px;border-radius:16px 16px 0 0;box-shadow:0 -16px 50px rgba(20,21,15,.22)}
.pop.b .grab{padding:9px 0 5px;display:flex;justify-content:center;flex:none}
.pop.b .grab i{width:42px;height:4px;border-radius:99px;background:var(--line-soft)}
.pop.b .ph{padding:10px 24px 16px;border-bottom:0}
.pop.b .ph h3{font-size:22px}
.pop.b .ph p{font-size:13px}
.srch{margin:2px 24px 6px;height:42px;border:1px solid var(--line-soft);border-radius:9px;display:flex;align-items:center;gap:9px;padding:0 13px;color:var(--muted);font-size:14px;flex:none}
.srch .ph2{font-size:14px;color:var(--faint)}
.grp{padding:16px 24px 8px;font:11px JetBrainsMono,monospace;letter-spacing:.12em;text-transform:uppercase;color:var(--faint)}
.pop.b .row{padding:14px 24px;border-bottom:1px solid #f1f0eb}
.pop.b .nm{font-size:17px}
.pop.b .ds{font-size:13.5px}
.pop.b .box{width:21px;height:21px;font-size:13px}
.pop.b .pf{padding:14px 24px 18px}
.pop.b .done{font-size:14px;padding:9px 16px}
/* ---- C：整格换页 ---- */
.whole{position:absolute;inset:0;background:var(--bg);display:flex;flex-direction:column;box-shadow:inset 0 0 0 0 transparent;animation:slide .18s ease-out}
@keyframes slide{from{transform:translateX(10px);opacity:.6}to{transform:none;opacity:1}}
.whole .wh{flex:none;display:flex;align-items:center;gap:12px;padding:26px 30px 18px;border-bottom:1px solid var(--line)}
.whole .back{display:flex;align-items:center;gap:5px;font:13px Manrope;color:var(--fg2);padding:6px 11px;border-radius:8px;border:1px solid var(--line-soft);background:var(--panel)}
.whole .wh h3{font:700 19px/1.2 Manrope;letter-spacing:-.01em}
.whole .cnt{margin-left:auto;font:11.5px JetBrainsMono,monospace;color:var(--muted)}
.whole .rows{flex:1;padding:0 30px;overflow:hidden}
.whole .row{padding:16px 0;border-bottom:1px solid var(--line-soft)}
.whole .nm{font-size:16px}
.whole .ds{font-size:13px}
.whole .box{width:20px;height:20px;font-size:13px}
.whole .pf{flex:none;padding:16px 30px;border-top:1px solid var(--line-soft);display:flex;align-items:center;justify-content:space-between;background:var(--panel)}
.whole .done{font-size:13.5px;padding:8px 16px}
footer{max-width:1100px;margin:38px auto 0;font:11.5px/1.75 JetBrainsMono,monospace;color:#6f6d69}
footer b{color:#c9c4bb;font-weight:400}
</style></head>
<body>
<header>
  <h1>弹层太小 · 980 宽（右格 644）· 三套解法</h1>
  <p>盘面不动（就是现在这版竖轴）。只改<b>点开某一项之后的那张弹层</b>。现在的问题是字号跟着「小卡片」的尺度算：
  标题 <code>12.5px</code>、提示 <code>10px</code>、正文 <code>13.5px</code>、底栏那句话 <code>8.5px</code> ——
  在 1x 屏上就是看不清；而盘面主文案是 <code>19px</code>，进去一下反而更小，落差很怪。</p>
  <ul>
    <li><b>A · 撑满右格</b> —— 弹层左右各留 16，宽 612（原来是 460–588 还居中），字号抬到 15.5 / 12.5 / 19。改的是 <code>pop_rect</code> 的宽度公式 + 字号档位。</li>
    <li><b>B · 底部上滑 sheet</b> —— 贴满宽、从底部长上来 836 高，圆角只留上两角，行高 62、名字 17px。像手机上的 sheet，一屏给足。</li>
    <li><b>C · 整格换页</b> —— 不要浮层：点一行就把右格整个换成这一项的选择页（左上角 <code>‹ 盘面</code> 回去）。没有遮罩也没有压住谁，字号能给到 16。</li>
  </ul>
</header>
<nav>
  <button data-k="a" aria-selected="true">A · 撑满右格</button>
  <button data-k="b">B · 底部 sheet</button>
  <button data-k="c">C · 整格换页</button>
  <span class="zoom"><span>缩放</span><input type="range" min="30" max="100" value="62" id="zoom"><span id="zv">62%</span></span>
</nav>
<div class="stage">
<section class="scheme on" data-k="a"><div class="cap"><b>A · 撑满右格</b> —— 左右各 16、宽 612；标题 19、正文 15.5、说明 12.5；行 15px 内边距，一屏 8 行不用滚。</div><div class="holder">__A__</div></section>
<section class="scheme" data-k="b"><div class="cap"><b>B · 底部上滑 sheet</b> —— 贴满宽、836 高、上圆角 16；标题 22、名字 17、说明 13.5；带搜索与分组。最顺手，也最"重"。</div><div class="holder">__B__</div></section>
<section class="scheme" data-k="c"><div class="cap"><b>C · 整格换页</b> —— 不叠浮层，右格整页换成选择页：顶部 <code>‹ 盘面</code> + 标题，行高 16px 正文 30px 左右边距，底部一条落地按钮。看不见盘面，但没有任何字号妥协。</div><div class="holder">__C__</div></section>
</div>
<footer><b>怎么挑</b> —— 关心「一进去还能看见盘面」就选 A；关心「一屏把八行技能都点完」选 B；想彻底不要浮层、每次点开都当成进了一个新页面，选 C。</footer>
<script>
const S=document.querySelectorAll('.scheme'),N=document.querySelectorAll('nav button');
function go(k){S.forEach(s=>s.classList.toggle('on',s.dataset.k===k));N.forEach(b=>b.setAttribute('aria-selected',b.dataset.k===k))}
N.forEach(b=>b.onclick=()=>go(b.dataset.k));
addEventListener('keydown',e=>{const m={'1':'a','2':'b','3':'c'}[e.key];if(m)go(m)});
if(location.hash){const h=location.hash.slice(1);if(['a','b','c'].includes(h))go(h)}
const z=document.getElementById('zoom'),zv=document.getElementById('zv');
function fit(){const v=+z.value;zv.textContent=v+'%';document.querySelectorAll('.holder').forEach(h=>{h.style.transform='scale('+v/100+')';h.style.height=(1168*v/100)+'px'})}
z.oninput=fit;fit();
</script></body></html>"""

for k, v in (("__A__", A), ("__B__", B), ("__C__", C)):
    HTML = HTML.replace(k, v)
for k, v in (("__M400__", F["m400"]), ("__M500__", F["m500"]), ("__M700__", F["m700"]), ("__J400__", F["j400"])):
    HTML = HTML.replace(k, v)

OUT.write_text(HTML)
print(f"{OUT}  {OUT.stat().st_size/1024:.0f} KB")
