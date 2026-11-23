#!/usr/bin/env python3
"""生成 demo/new-chat-980/preview.html —— 单文件、字体 base64 内嵌、双击就开。

几何照 crates/nousdesk/src/board_art.rs 抄：窗口 980×1168 → 标题栏 34 →
三栏 56(rail) + 280(list) + 644(pane)，内容高 1134。这一次四套都是全新的排法，
上一版（四角卡 / 撑满轨道 / 单列表单 / 修留白）全部作废。
"""
import base64
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
FONTS = ROOT / "assets" / "fonts"
OUT = pathlib.Path(__file__).resolve().parent / "preview.html"


def b64(p: pathlib.Path) -> str:
    return base64.b64encode(p.read_bytes()).decode()


manrope400 = b64(FONTS / "manrope-400.ttf")
manrope500 = b64(FONTS / "manrope-500.ttf")
manrope700 = b64(FONTS / "manrope-700.ttf")
mono400 = b64(FONTS / "jetbrainsmono-400.ttf")

HTML = r"""<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>新建会话 · 980 宽 · 四套新排法</title>
<style>
@font-face{font-family:Manrope;src:url(data:font/ttf;base64,__MANROPE400__) format("truetype");font-weight:100 500;font-display:block}
@font-face{font-family:Manrope;src:url(data:font/ttf;base64,__MANROPE500__) format("truetype");font-weight:501 600;font-display:block}
@font-face{font-family:Manrope;src:url(data:font/ttf;base64,__MANROPE700__) format("truetype");font-weight:601 900;font-display:block}
@font-face{font-family:JetBrainsMono;src:url(data:font/ttf;base64,__MONO400__) format("truetype");font-weight:100 900;font-display:block}

*{box-sizing:border-box;margin:0;padding:0}
:root{
  --rail:#e9e6e0;--list:#f4f2ee;--bg:#f4f2ee;--panel:#fff;
  --fg:#14150f;--fg2:#5f5d55;--muted:#8d8a82;--faint:#b9b6ad;
  --line:#14150f;--line-soft:#d8d5cd;--ok:#1d6b46;--danger:#a8281c;
}
html,body{background:#0b0b0c;color:#f4f3f1;font:14px/1.55 Manrope,"Noto Sans CJK SC",sans-serif;-webkit-font-smoothing:antialiased}
body{padding:26px 26px 60px}
header{max-width:1100px;margin:0 auto 20px}
header h1{font-size:15px;font-weight:700;letter-spacing:.02em}
header p{margin-top:9px;font-size:12.5px;line-height:1.75;color:#8b8884;max-width:980px}
header p b{color:#e8e5e0;font-weight:600}
header code{font:11px JetBrainsMono,monospace;color:#9ecf96;background:#141416;padding:1px 5px;border-radius:4px}
nav{display:flex;flex-wrap:wrap;gap:8px;align-items:center;max-width:1100px;margin:0 auto 22px}
nav button{font:12px Manrope,sans-serif;color:#9a968f;background:#17171a;border:1px solid #26262a;border-radius:999px;padding:7px 14px;cursor:pointer;transition:.12s}
nav button:hover{color:#ded9d2;border-color:#3a3a40}
nav button[aria-selected="true"]{background:#f4f3f1;border-color:#f4f3f1;color:#0b0b0c;font-weight:600}
nav .zoom{display:flex;align-items:center;gap:7px;font:11px JetBrainsMono,monospace;color:#6f6d69;margin-left:auto}
nav .zoom input{width:130px;accent-color:#8fbf88}
.stage{max-width:1100px;margin:0 auto}
.scheme{display:none;flex-direction:column;gap:14px}
.scheme.on{display:flex}
.cap{font:11.5px/1.6 JetBrainsMono,monospace;color:#8b8884}
.cap b{color:#ded9d2;font-weight:400}
.holder{width:980px;height:1168px;transform-origin:top left}
.win{width:980px;height:1168px;background:var(--bg);color:var(--fg);border-radius:10px;overflow:hidden;display:flex;flex-direction:column;font:13px/1.5 Manrope,"Noto Sans CJK SC",sans-serif}
.titlebar{height:34px;flex:none;display:flex;align-items:center;gap:8px;padding:0 12px;background:var(--list);border-bottom:1px solid var(--line-soft)}
.titlebar .name{font:600 11.5px Manrope,sans-serif;letter-spacing:.02em}
.titlebar .grip{display:flex;gap:6px;margin-left:auto}
.titlebar .grip i{width:9px;height:9px;border-radius:50%;background:#fff;border:1px solid var(--line-soft)}
.body{flex:1;display:flex;min-height:0}

/* ---- 三栏骨架 ---- */
.rail{width:56px;flex:none;background:var(--rail);display:flex;flex-direction:column;align-items:center;padding:14px 0 12px;gap:6px}
.rail .ic{width:34px;height:34px;border-radius:9px;display:flex;align-items:center;justify-content:center;color:#6b6860}
.rail .ic svg{width:19px;height:19px;stroke:currentColor;fill:none;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round}
.rail .ic.act{background:#fff;color:var(--fg);box-shadow:0 0 0 1px var(--line-soft)}
.rail .sp{flex:1}
.list{width:280px;flex:none;background:var(--list);border-right:1px solid var(--line-soft);display:flex;flex-direction:column;padding:13px 14px;gap:10px}
.list .row{display:flex;gap:7px}
.list .search{flex:1;height:30px;border:1px solid var(--line-soft);border-radius:7px;display:flex;align-items:center;gap:6px;padding:0 9px;color:var(--muted);font-size:12px;background:var(--panel)}
.list .plus{width:30px;height:30px;border-radius:7px;background:var(--fg);color:#fff;display:flex;align-items:center;justify-content:center;font-size:15px}
.list .empty{margin-top:8px;color:var(--muted);font-size:12px;line-height:1.75}
.list .empty b{display:block;color:var(--fg2);font-weight:500;margin-bottom:2px}
.pane{flex:1;min-width:0;background:var(--bg);position:relative;overflow:hidden}

/* ---- D 竖轴 ---- */
.d{padding:30px 34px 26px;display:flex;flex-direction:column;height:100%}
.d .hd{display:flex;align-items:baseline;gap:10px}
.d .hd h2{font:700 21px/1.2 Manrope;letter-spacing:-.01em}
.d .hd .esc{margin-left:auto;font:10.5px JetBrainsMono,monospace;color:var(--faint)}
.d .axis{margin-top:10px;position:relative;flex:1;display:flex;flex-direction:column;min-height:0}
.d .axis::before{content:"";position:absolute;left:5px;top:0;bottom:0;width:1px;background:var(--line-soft)}
.d .node{flex:1;position:relative;padding:0 12px 0 30px;display:flex;flex-direction:column;justify-content:center;cursor:pointer;border-radius:9px;transition:background .12s}
.d .node:hover{background:#fff}
.d .node::before{content:"";position:absolute;left:0;top:50%;margin-top:-6px;width:11px;height:11px;border-radius:50%;background:var(--bg);border:1.5px solid var(--line)}
.d .node.set::before{background:var(--fg)}
.d .node:hover .v{text-decoration:underline;text-decoration-thickness:1px;text-underline-offset:3px}
.d .k{font:10.5px/1 JetBrainsMono,monospace;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}
.d .v{margin-top:6px;font:600 19px/1.25 Manrope;letter-spacing:-.012em}
.d .v.warn{color:var(--danger)}
.d .s{margin-top:3px;font-size:12px;color:var(--muted)}
.d .s .mono{font-family:JetBrainsMono,monospace;font-size:11.5px}
.d .go{position:absolute;right:0;top:14px;color:var(--faint);font-size:16px}
.d .foot{margin-top:auto;border-top:1px solid var(--line-soft);padding-top:16px;display:flex;flex-direction:column;gap:12px}
.d .name{display:flex;align-items:center;gap:10px}
.d .name .k{flex:none}
.d .name input{flex:1;border:0;border-bottom:1px solid var(--line-soft);background:transparent;font:500 15px Manrope;color:var(--fg);padding:5px 0;outline:none}
.d .name input::placeholder{color:var(--faint)}
.d .chips{display:flex;gap:7px}
.d .chip{font:11.5px JetBrainsMono,monospace;color:var(--fg2);border:1px solid var(--line-soft);border-radius:999px;padding:4px 11px;background:var(--panel)}
.d .chip.on{background:var(--fg);color:#fff;border-color:var(--fg)}
.d .create{margin-top:4px;height:54px;border-radius:10px;background:var(--fg);color:#fff;display:flex;align-items:center;justify-content:center;gap:10px;font:600 14.5px Manrope;letter-spacing:.02em;cursor:pointer}
.d .create .hint{font:10.5px JetBrainsMono,monospace;color:#9a978f}

/* ---- E 终端 ---- */
.e{padding:24px;height:100%;display:flex}
.e .term{flex:1;background:#14150f;border-radius:12px;display:flex;flex-direction:column;overflow:hidden;box-shadow:0 18px 40px rgba(20,21,15,.18)}
.e .bar{height:38px;flex:none;display:flex;align-items:center;gap:7px;padding:0 14px;border-bottom:1px solid #2a2b23}
.e .bar i{width:9px;height:9px;border-radius:50%;background:#3a3b32}
.e .bar .t{margin-left:8px;font:11px JetBrainsMono,monospace;color:#7d7a6e}
.e .bar .right{margin-left:auto;font:11px JetBrainsMono,monospace;color:#5c5a50}
.e .scr{flex:1;padding:22px 22px;font:13.5px/2.35 JetBrainsMono,monospace;color:#cfccc1;overflow:hidden;display:flex;flex-direction:column}
.e .cmd{color:#8fbf88}
.e .dim{color:#6d6a5f}
.e .ln{display:flex;gap:14px;cursor:pointer;margin:0 -8px;padding:0 8px;border-radius:6px}
.e .ln:hover{background:#1f2019}
.e .ln .k{width:118px;flex:none;color:#8b887a}
.e .ln .v{color:#f2efe6}
.e .ln .x{margin-left:auto;color:#5c5a50}
.e .ln .x.ok{color:#8fbf88}
.e .ln .blank{border-bottom:1px dashed #4a4b3f;min-width:150px;display:inline-block;color:#7d7a6e}
.e .caret{display:inline-block;width:8px;height:15px;background:#8fbf88;vertical-align:-2px;animation:blink 1.1s steps(2) infinite}
@keyframes blink{50%{opacity:0}}
.e .foot{flex:none;border-top:1px solid #2a2b23;padding:14px 18px;display:flex;align-items:center;gap:12px}
.e .btn{height:42px;padding:0 22px;border-radius:8px;background:#f2efe6;color:#14150f;display:flex;align-items:center;gap:9px;font:600 13px JetBrainsMono,monospace;letter-spacing:.08em;cursor:pointer}
.e .btn b{font-weight:600}
.e .note{font:11px JetBrainsMono,monospace;color:#6d6a5f}

/* ---- F 白纸 + 底部 dock ---- */
.f{padding:0;height:100%;display:flex;flex-direction:column}
.f .top{padding:28px 34px 0;display:flex;align-items:center}
.f .top h2{font:700 22px/1.2 Manrope;letter-spacing:-.015em}
.f .top .right{margin-left:auto;display:flex;align-items:center;gap:14px;font:11px JetBrainsMono,monospace;color:var(--faint)}
.f .top .x{width:26px;height:26px;border-radius:7px;border:1px solid var(--line-soft);display:flex;align-items:center;justify-content:center;color:var(--muted);background:var(--panel);font-size:14px;cursor:pointer}
.f .say{flex:1;min-height:0;padding:0 34px;display:flex;flex-direction:column;justify-content:center}
.f .say textarea{width:100%;height:210px;border:0;background:transparent;resize:none;outline:none;font:400 19px/1.75 Manrope;color:var(--fg);letter-spacing:-.005em}
.f .say textarea::placeholder{color:var(--faint)}
.f .hint{margin-top:8px;font-size:12px;color:var(--faint)}
.f .sugg{margin-top:34px;display:flex;flex-direction:column;gap:2px}
.f .sg{display:flex;align-items:center;gap:12px;padding:11px 10px;border-radius:8px;color:var(--muted);font-size:13px;cursor:pointer}
.f .sg:hover{background:#fff;color:var(--fg)}
.f .sg .i{font:10.5px JetBrainsMono,monospace;color:var(--faint)}
.f .dock{border-top:1px solid var(--line-soft);padding:11px 18px;display:flex;align-items:center;gap:6px;flex-wrap:nowrap;background:var(--list);flex:none}
.f .pill{display:flex;align-items:center;gap:6px;height:30px;padding:0 10px;border-radius:999px;border:1px solid var(--line-soft);background:var(--panel);font-size:11.5px;color:var(--fg2);cursor:pointer;white-space:nowrap}
.f .pill:hover{border-color:var(--fg);color:var(--fg)}
.f .pill b{font-weight:600;color:var(--fg)}
.f .pill .m{font:11px JetBrainsMono,monospace;color:var(--muted)}
.f .pill .dot{width:5px;height:5px;border-radius:50%;background:var(--faint)}
.f .pill .dot.on{background:var(--ok)}
.f .go{margin-left:auto;width:40px;height:40px;border-radius:50%;background:var(--fg);color:#fff;display:flex;align-items:center;justify-content:center;font-size:16px;cursor:pointer;flex:none}
.f .go span{transform:translateY(-1px)}

/* ---- G 配置单 ---- */
.g{padding:30px 34px 28px;height:100%;display:flex;flex-direction:column}
.g .hd{display:flex;align-items:baseline}
.g .hd h2{font:700 21px/1.2 Manrope;letter-spacing:-.01em}
.g .hd .st{margin-left:auto;font:10.5px JetBrainsMono,monospace;color:var(--faint);letter-spacing:.1em}
.g .grid{margin-top:18px;display:grid;grid-template-columns:1fr 1fr;grid-auto-rows:186px;gap:1px;background:var(--line-soft);border:1px solid var(--line-soft)}
.g .cell{background:var(--panel);padding:22px 20px 20px;display:flex;flex-direction:column;justify-content:space-between}
.g .perms{display:flex;align-items:center;gap:7px;margin-top:18px}
.g .perms .k{font:10.5px JetBrainsMono,monospace;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);width:104px}
.g .tg{font:11.5px JetBrainsMono,monospace;color:var(--faint);border:1px solid var(--line-soft);border-radius:6px;padding:5px 10px}
.g .tg.on{color:var(--fg);border-color:var(--fg);background:var(--panel)}
.g .cell .k{font:10px/1 JetBrainsMono,monospace;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}
.g .cell .v{font:700 27px/1.05 Manrope;letter-spacing:-.03em}
.g .cell .v sm{font:600 14px Manrope;color:var(--muted);letter-spacing:0}
.g .cell .s{margin-top:6px;font-size:11.5px;color:var(--muted)}
.g .cell .s.mono{font-family:JetBrainsMono,monospace;font-size:11px}
.g .cell.warn .v{color:var(--danger)}
.g .spec{margin-top:22px;border-top:1px solid var(--line);}
.g .spec .r{display:flex;align-items:baseline;gap:12px;padding:13px 2px;border-bottom:1px solid var(--line-soft);cursor:pointer}
.g .spec .r:hover{background:#fff}
.g .spec .r .k{width:104px;flex:none;font:10.5px JetBrainsMono,monospace;letter-spacing:.1em;text-transform:uppercase;color:var(--muted)}
.g .spec .r .v{font:500 13.5px Manrope;color:var(--fg)}
.g .spec .r .x{margin-left:auto;font:11px JetBrainsMono,monospace;color:var(--faint)}
.g .foot{margin-top:auto;padding-top:18px;display:flex;align-items:center;gap:12px}
.g .foot .name{flex:1;display:flex;align-items:center;gap:10px}
.g .foot .name .k{font:10.5px JetBrainsMono,monospace;letter-spacing:.1em;text-transform:uppercase;color:var(--muted)}
.g .foot input{flex:1;border:0;border-bottom:1px solid var(--line-soft);background:transparent;font:500 15px Manrope;color:var(--fg);padding:6px 0;outline:none}
.g .foot input::placeholder{color:var(--faint)}
.g .create{height:52px;padding:0 30px;border-radius:10px;background:var(--fg);color:#fff;display:flex;align-items:center;gap:12px;font:600 14px Manrope;cursor:pointer}
.g .create .mono{font:10.5px JetBrainsMono,monospace;color:#9a978f}

footer{max-width:1100px;margin:40px auto 0;font:11.5px/1.7 JetBrainsMono,monospace;color:#6f6d69}
footer b{color:#c9c4bb;font-weight:400}
</style>
</head>
<body>
<header>
  <h1>新建会话 · 980×1168 · 四套全新排法</h1>
  <p>
    骨架照旧（rail 56 + 会话列表 280 + 盘面 <b>644×1134</b>），皮照旧（industrial：黑线白卡、Manrope + JetBrains Mono）。
    <b>上一版的四角卡片 + 中间圆 + 虚线全部不要了</b>，四套是四种完全不同的信息结构，从「参数怎么摆」重新起。
  </p>
</header>

<nav>
  <button data-k="d" aria-selected="true">D · 竖轴清单</button>
  <button data-k="e">E · 终端邀请</button>
  <button data-k="f">F · 白纸 + Dock</button>
  <button data-k="g">G · 配置单</button>
  <span class="zoom"><span>缩放</span><input type="range" min="30" max="100" value="52" id="zoom"><span id="zv">52%</span></span>
</nav>

<div class="stage">

<!-- ================= D · 竖轴清单 ================= -->
<section class="scheme on" data-k="d">
  <div class="cap"><b>D · 竖轴清单</b> —— 一条竖轴串四个节点，从上往下就是一次「填参数」；脚下一行命名 + 创建。竖向空间吃满，没有一处是装饰。</div>
  <div class="holder"><div class="win">
    <div class="titlebar"><span class="name">NousDesk</span><span class="grip"><i></i><i></i><i></i></span></div>
    <div class="body">
      <div class="rail">
        <div class="ic act"><svg viewBox="0 0 24 24"><path d="M20 12a8 8 0 1 1-3.2-6.4"/><path d="M21 4v5h-5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 9h8M8 13h5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 8v8M8 12h8"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M12 4l8 4.5v7L12 20l-8-4.5v-7z"/><path d="M12 12l8-3.5M12 12v8M12 12L4 8.5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><ellipse cx="12" cy="7" rx="7" ry="3"/><path d="M5 7v10c0 1.7 3.1 3 7 3s7-1.3 7-3V7"/></svg></div>
        <div class="sp"></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg></div>
      </div>
      <div class="list">
        <div class="row"><div class="search">⌕ 搜索会话</div><div class="plus">+</div></div>
        <div class="empty"><b>一条会话都还没有</b>点右上 ＋ 建第一条</div>
      </div>
      <div class="pane"><div class="d">
        <div class="hd"><h2>新会话</h2><span class="esc">ESC 取消</span></div>
        <div class="axis">
          <div class="node set"><span class="k">model</span><div class="v">claude-sonnet-4.5</div><div class="s"><span class="mono">anthropic</span> · 思考 <span class="mono">off</span></div><span class="go">›</span></div>
          <div class="node"><span class="k">extensions</span><div class="v">0 个技能</div><div class="s">1 个加载失败 · 见设置</div><span class="go">›</span></div>
          <div class="node set"><span class="k">context</span><div class="v">smart-ass</div><div class="s">随会话注入的知识与语气</div><span class="go">›</span></div>
          <div class="node set"><span class="k">permissions</span><div class="v">可写 · 文件 + 命令</div><div class="s">能改文件、能跑 shell</div><span class="go">›</span></div>
        </div>
        <div class="foot">
          <div class="name"><span class="k">name</span><input value="asi" placeholder="留空自动命名"></div>
          <div class="chips"><span class="chip on">本机</span><span class="chip">work</span><span class="chip">~/src/api</span></div>
          <div class="create">创建会话 <span class="hint">⏎</span></div>
        </div>
      </div></div>
    </div>
  </div></div>
</section>

<!-- ================= E · 终端邀请 ================= -->
<section class="scheme" data-k="e">
  <div class="cap"><b>E · 终端邀请</b> —— 把「新建」当成敲一条命令：黑卡里七行参数，点哪行改哪行，底部一颗反白 <code>[ CREATE ]</code>。最像 Tesla 的那种冷。</div>
  <div class="holder"><div class="win">
    <div class="titlebar"><span class="name">NousDesk</span><span class="grip"><i></i><i></i><i></i></span></div>
    <div class="body">
      <div class="rail">
        <div class="ic act"><svg viewBox="0 0 24 24"><path d="M20 12a8 8 0 1 1-3.2-6.4"/><path d="M21 4v5h-5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 9h8M8 13h5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 8v8M8 12h8"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M12 4l8 4.5v7L12 20l-8-4.5v-7z"/><path d="M12 12l8-3.5M12 12v8M12 12L4 8.5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><ellipse cx="12" cy="7" rx="7" ry="3"/><path d="M5 7v10c0 1.7 3.1 3 7 3s7-1.3 7-3V7"/></svg></div>
        <div class="sp"></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg></div>
      </div>
      <div class="list">
        <div class="row"><div class="search">⌕ 搜索会话</div><div class="plus">+</div></div>
        <div class="empty"><b>一条会话都还没有</b>点右上 ＋ 建第一条</div>
      </div>
      <div class="pane"><div class="e"><div class="term">
        <div class="bar"><i></i><i></i><i></i><span class="t">nous · new session</span><span class="right">980×1134</span></div>
        <div class="scr">
          <div class="cmd">$ nous new<span class="caret"></span></div>
          <div class="ln"><span class="k">model</span><span class="v">claude-sonnet-4.5</span><span class="x ok">anthropic · think off</span></div>
          <div class="ln"><span class="k">extensions</span><span class="v">0 loaded</span><span class="x">1 failed</span></div>
          <div class="ln"><span class="k">context</span><span class="v">smart-ass</span><span class="x">prompt</span></div>
          <div class="ln"><span class="k">permissions</span><span class="v">writable</span><span class="x">files + shell</span></div>
          <div class="ln"><span class="k">name</span><span class="v"><span class="blank">asi</span></span><span class="x">optional</span></div>
          <div class="ln"><span class="k">host</span><span class="v">本机</span><span class="x">work available</span></div>
          <div class="dim">── 6 fields ready ───────────────────────────</div>
          <div class="dim"># 上次用的是 claude-sonnet-4.5 · ~/src/api · 3 小时前</div>
          <div class="dim" style="margin-top:auto">点任意一行改它 · ⏎ 直接创建 · ESC 取消</div>
        </div>
        <div class="foot"><div class="btn"><b>[ CREATE ]</b></div><span class="note">⏎ 直接建，先不填也能建</span></div>
      </div></div></div>
    </div>
  </div></div>
</section>

<!-- ================= F · 白纸 + Dock ================= -->
<section class="scheme" data-k="f">
  <div class="cap"><b>F · 白纸 + Dock</b> —— 先把「第一句话」摆在正中，参数全退到脚下一条 dock；参数不再抢戏，想改再点。</div>
  <div class="holder"><div class="win">
    <div class="titlebar"><span class="name">NousDesk</span><span class="grip"><i></i><i></i><i></i></span></div>
    <div class="body">
      <div class="rail">
        <div class="ic act"><svg viewBox="0 0 24 24"><path d="M20 12a8 8 0 1 1-3.2-6.4"/><path d="M21 4v5h-5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 9h8M8 13h5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 8v8M8 12h8"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M12 4l8 4.5v7L12 20l-8-4.5v-7z"/><path d="M12 12l8-3.5M12 12v8M12 12L4 8.5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><ellipse cx="12" cy="7" rx="7" ry="3"/><path d="M5 7v10c0 1.7 3.1 3 7 3s7-1.3 7-3V7"/></svg></div>
        <div class="sp"></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg></div>
      </div>
      <div class="list">
        <div class="row"><div class="search">⌕ 搜索会话</div><div class="plus">+</div></div>
        <div class="empty"><b>一条会话都还没有</b>点右上 ＋ 建第一条</div>
      </div>
      <div class="pane"><div class="f">
        <div class="top"><h2>新会话</h2><div class="right"><span>ESC</span><span class="x">×</span></div></div>
        <div class="say">
          <textarea placeholder="说点什么 —— 或者直接按 ⏎ 建一条空会话"></textarea>
          <div class="hint">⏎ 创建 · ⇧⏎ 换行 · 参数在脚下</div>
          <div class="sugg">
            <div class="sg"><span class="i">01</span>读一下 wire.md，告诉我客户端调了哪些端点</div>
            <div class="sg"><span class="i">02</span>检查 j1900 上的引擎版本和 contract</div>
            <div class="sg"><span class="i">03</span>把昨天那个分支的 diff 过一遍</div>
          </div>
        </div>
        <div class="dock">
          <div class="pill"><span class="dot on"></span><b>claude-sonnet-4.5</b><span class="m">anthropic</span></div>
          <div class="pill"><span class="dot"></span>技能 <b>0</b><span class="m">/ 5</span></div>
          <div class="pill"><span class="dot on"></span><b>smart-ass</b><span class="m">context</span></div>
          <div class="pill"><span class="dot on"></span>可写 <span class="m">files+shell</span></div>
          <div class="go"><span>↑</span></div>
        </div>
      </div></div>
    </div>
  </div></div>
</section>

<!-- ================= G · 配置单 ================= -->
<section class="scheme" data-k="g">
  <div class="cap"><b>G · 配置单</b> —— 四块大数字压在上面（0/5、5 个技能…），下面一张两列参数表逐项可点。信息量最大，一眼扫完再动手。</div>
  <div class="holder"><div class="win">
    <div class="titlebar"><span class="name">NousDesk</span><span class="grip"><i></i><i></i><i></i></span></div>
    <div class="body">
      <div class="rail">
        <div class="ic act"><svg viewBox="0 0 24 24"><path d="M20 12a8 8 0 1 1-3.2-6.4"/><path d="M21 4v5h-5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8 9h8M8 13h5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M12 8v8M8 12h8"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M12 4l8 4.5v7L12 20l-8-4.5v-7z"/><path d="M12 12l8-3.5M12 12v8M12 12L4 8.5"/></svg></div>
        <div class="ic"><svg viewBox="0 0 24 24"><ellipse cx="12" cy="7" rx="7" ry="3"/><path d="M5 7v10c0 1.7 3.1 3 7 3s7-1.3 7-3V7"/></svg></div>
        <div class="sp"></div>
        <div class="ic"><svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg></div>
      </div>
      <div class="list">
        <div class="row"><div class="search">⌕ 搜索会话</div><div class="plus">+</div></div>
        <div class="empty"><b>一条会话都还没有</b>点右上 ＋ 建第一条</div>
      </div>
      <div class="pane"><div class="g">
        <div class="hd"><h2>新会话</h2><span class="st">SPEC</span></div>
        <div class="grid">
          <div class="cell"><div class="k">skills</div><div class="v">0<sm> / 5</sm></div><div class="s">1 个加载失败</div></div>
          <div class="cell"><div class="k">mode</div><div class="v">可写<sm></sm></div><div class="s">files + shell</div></div>
          <div class="cell"><div class="k">model</div><div class="v" style="font-size:18px;line-height:1.25">claude-sonnet-4.5</div><div class="s mono">anthropic · think off</div></div>
          <div class="cell"><div class="k">context</div><div class="v" style="font-size:18px;line-height:1.25">smart-ass</div><div class="s">知识与语气</div></div>
        </div>
        <div class="spec">
          <div class="r"><span class="k">host</span><span class="v">本机</span><span class="x">work ›</span></div>
          <div class="r"><span class="k">dir</span><span class="v">~/src/api</span><span class="x">改 ›</span></div>
          <div class="r"><span class="k">name</span><span class="v">asi</span><span class="x">改 ›</span></div>
          <div class="r"><span class="k">thinking</span><span class="v">off</span><span class="x">改 ›</span></div>
          <div class="r"><span class="k">context win</span><span class="v">200k tokens</span><span class="x">改 ›</span></div>
        </div>
        <div class="perms"><span class="k">allow</span><span class="tg on">读文件</span><span class="tg on">写文件</span><span class="tg on">跑命令</span><span class="tg">网络</span></div>
        <div class="foot">
          <div class="name"><span class="k">name</span><input value="asi" placeholder="留空自动命名"></div>
          <div class="create">创建会话 <span class="mono">⏎</span></div>
        </div>
      </div></div>
    </div>
  </div></div>
</section>

</div>

<footer>
  文件：<b>/opt/NousDesk/demo/new-chat-980/preview.html</b> · 单文件、字体 base64 内嵌、双击就开 · 数字键 1–4 切换
</footer>

<script>

const btns = [...document.querySelectorAll('nav button')];
const schemes = [...document.querySelectorAll('.scheme')];
function pick(k){
  btns.forEach(b => b.setAttribute('aria-selected', b.dataset.k === k ? 'true' : 'false'));
  schemes.forEach(s => s.classList.toggle('on', s.dataset.k === k));
  if (location.hash.slice(1) !== k) history.replaceState(null, '', '#' + k);
}
const start = location.hash.slice(1);
if (['d','e','f','g'].includes(start)) pick(start);
btns.forEach(b => b.onclick = () => pick(b.dataset.k));
document.addEventListener('keydown', e => {
  const i = '1234'.indexOf(e.key);
  if (i >= 0) pick(btns[i].dataset.k);
});
const zoom = document.getElementById('zoom'), zv = document.getElementById('zv');
function apply(){
  const z = zoom.value / 100;
  zv.textContent = zoom.value + '%';
  document.querySelectorAll('.holder').forEach(h => {
    h.style.transform = `scale(${z})`;
    h.style.height = (1168 * z) + 'px';
  });
}
zoom.oninput = apply; apply();
</script>
</body>
</html>
"""

html = (HTML
        .replace("__MANROPE400__", manrope400)
        .replace("__MANROPE500__", manrope500)
        .replace("__MANROPE700__", manrope700)
        .replace("__MONO400__", mono400))
OUT.write_text(html)
print(f"wrote {OUT} {len(html) / 1024:.0f} KB")
