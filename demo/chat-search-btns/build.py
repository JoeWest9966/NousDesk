#!/usr/bin/env python3
"""生成 demo/chat-search-btns/preview.html —— 单文件、字体 base64 内嵌、双击就开。

只讲一件事：**会话列表顶上、搜索框下面那一行，放几个按钮长什么样。**

页面本体不是画个"差不多"的示意图，而是照着 `crates/nousdesk/src/app.rs` 里的真实
结构抄下来的：rail 56 → 会话列 280（搜索行 + 新增按钮行 + 列表）→ 目录列 240 → 聊天区。
尺寸、字号、色值来自 `theme.rs` 的 `Skin::INDUSTRIAL`（`density = 1.05`、`radius_scale = 0`
全直角、Manrope 12px、分隔线是纯黑 1px），所以这里每一个 px 都是「真跑起来会看到的」。

按钮功能还没定，全部是占位：点了只切高亮，不做任何事。
"""
import base64
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
FONTS = ROOT / "assets" / "fonts"
OUT = pathlib.Path(__file__).resolve().parent / "preview.html"


def b64(p: pathlib.Path) -> str:
    return base64.b64encode(p.read_bytes()).decode()


# ── 图标（从 rail_icons.rs 原样抄，24×24 viewBox，currentColor）─────────────
ICONS = {
    "chats": '<path d="M5.4 8.4a6.6 6.6 0 0 1 13.2 0v7.2a3.6 3.6 0 0 1-3.6 3.6H9a3.6 3.6 0 0 1-3.6-3.6V8.4Z"/><circle cx="9.6" cy="11.6" r="1.05" fill="currentColor" stroke="none"/><circle cx="14.2" cy="11.6" r="1.05" fill="currentColor" stroke="none"/><path d="M9.4 15.6h5.2" opacity="0.6"/><path d="M20.2 10.6h1.6v3.4h-1.6" stroke-width="1.5" opacity="0.75"/>',
    "templates": '<rect x="6.4" y="6.4" width="11.2" height="11.2" stroke-width="1.6"/><path d="M4.5 9.2H6.4M17.6 9.2H19.5M4.5 12H6.4M17.6 12H19.5M4.5 14.8H6.4M17.6 14.8H19.5" stroke-width="1.6"/><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none"/>',
    "triggers": '<circle cx="6.4" cy="12" r="2.6"/><path d="M10.4 8.2a5.4 5.4 0 0 1 0 7.6" opacity="0.9"/><path d="M13.6 6a9 9 0 0 1 0 12" opacity="0.7"/><path d="M16.8 3.8a12.6 12.6 0 0 1 0 16.4" opacity="0.45"/>',
    "resources": '<rect x="4" y="3.6" width="16" height="16.8"/><path d="M4 9.2h16M4 14.8h16"/><circle cx="6.6" cy="6.4" r="0.75" fill="currentColor" stroke="none"/><circle cx="6.6" cy="12" r="0.75" fill="currentColor" stroke="none"/><circle cx="6.6" cy="17.6" r="0.75" fill="currentColor" stroke="none"/><path d="M10 6.4h7M10 12h7M10 17.6h4.6" stroke-width="1.3" opacity="0.55"/>',
    "elements": '<circle cx="9" cy="9" r="4"/><circle cx="9" cy="9" r="2.25"/><path d="M12.9 9h1.35" stroke-width="2.8"/><path d="M11.43 12.05l.84 1.05" stroke-width="2.8"/><path d="M8.13 12.8l-.3 1.32" stroke-width="2.8"/><path d="M5.49 10.69l-1.22.59" stroke-width="2.8"/><path d="M5.49 7.31l-1.22-.59" stroke-width="2.8"/><path d="M8.13 5.2l-.3-1.32" stroke-width="2.8"/><path d="M11.43 5.95l.84-1.05" stroke-width="2.8"/><path d="M15.4 12.6l4 4"/><circle cx="20.4" cy="17.6" r="2.2" stroke-width="1.5"/>',
    "data": '<path d="M12 3.4c4.6 0 8.2 1.3 8.2 3s-3.6 3-8.2 3-8.2-1.3-8.2-3 3.6-3 8.2-3Z"/><path d="M3.8 6.4v11.2c0 1.7 3.6 3 8.2 3s8.2-1.3 8.2-3V6.4"/><path d="M3.8 12c0 1.7 3.6 3 8.2 3s8.2-1.3 8.2-3"/>',
    "search": '<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/>',
    "plus": '<path d="M12 5v14M5 12h14"/>',
    "menu": '<path d="M4 7h16M4 12h16M4 17h16"/>',
    "branch": '<circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="8" r="2"/><path d="M6 7v10M18 10c0 4-3 4-6 4"/>',
    "panel": '<rect x="4" y="4" width="16" height="16"/><path d="M9 4v16"/>',
    "history": '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>',
    "pin": '<path d="M12 17v5"/><path d="M9 10.76V7a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v3.76a2 2 0 0 0 .59 1.42l1.3 1.3A1 1 0 0 1 16.53 15H7.47a1 1 0 0 1-.7-1.71l1.3-1.3A2 2 0 0 0 9 10.76Z"/>',
    "filter": '<path d="M4 5h16l-6.4 7.6V19l-3.2-1.8v-4.6Z"/>',
    "archive": '<rect x="3" y="4" width="18" height="4"/><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8"/><path d="M10 12h4"/>',
    "clock": '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>',
    "unread": '<path d="M4 6h16v12H4z"/><path d="M4 7l8 6 8-6"/>',
    "star": '<path d="M12 3.6l2.6 5.4 5.9.8-4.3 4.1 1 5.9-5.2-2.8-5.2 2.8 1-5.9L3.5 9.8l5.9-.8Z"/>',
    "sliders": '<path d="M4 8h10M18 8h2M4 16h4M12 16h8"/><circle cx="16" cy="8" r="2"/><circle cx="10" cy="16" r="2"/>',
    "layers": '<path d="M12 3l8 4.5-8 4.5-8-4.5Z"/><path d="M4 12l8 4.5 8-4.5"/><path d="M4 16.5L12 21l8-4.5"/>',
    "grid": '<rect x="4" y="4" width="7" height="7"/><rect x="13" y="4" width="7" height="7"/><rect x="4" y="13" width="7" height="7"/><rect x="13" y="13" width="7" height="7"/>',
    "list": '<path d="M4 6h16M4 12h16M4 18h10"/>',
}


def svg(name: str, size: float = 15.0, w: float = 1.7) -> str:
    return (
        f'<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" '
        f'stroke="currentColor" stroke-width="{w}" stroke-linecap="round" '
        f'stroke-linejoin="round">{ICONS[name]}</svg>'
    )


# ── 会话数据（照着引擎里那批真实名字造：一堆「对话 N」、scratch 时间串、「真模型」）──
ROWS = [
    dict(id=2, name="真模型", prev="", pin=True, unread=0, when="6 天前", busy=False),
    dict(id=389, name="qrd3wh 部署验证", prev="镜像拉完了，systemd 起来之后 cooldown 还是 5 秒…", pin=False, unread=2, when="2 分钟前", busy=True),
    dict(id=377, name="对话 377", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=371, name="对话 371", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=366, name="scratch 11-13 19:16", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=364, name="scratch 11-13 18:02", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=358, name="对话 358", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=352, name="灯带固件回滚", prev="把 v0.9.28 的 disp 分支拉回来重新打了一次…", pin=False, unread=1, when="昨天", busy=False),
    dict(id=344, name="对话 344", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=338, name="j1900 迁移", prev="旧库还在 /var/backups 里，tar 解开对比过 schema 一致", pin=False, unread=0, when="3 天前", busy=False),
    dict(id=331, name="对话 331", prev="", pin=False, unread=0, when="", busy=False),
    dict(id=325, name="模板：只读巡检", prev="", pin=False, unread=0, when="", busy=False),
]


def session_rows() -> str:
    out = []
    for i, r in enumerate(ROWS):
        cls = "row" + (" on" if i == 1 else "")
        pin = f'<span class="pin">{svg("pin", 13.6, 1.9)}</span>' if r["pin"] else ""
        when = f'<span class="tm">{r["when"]}</span>' if r["when"] else ""
        if r["busy"]:
            line2 = '<div class="l2"><div class="p busy"><i></i>正在输入…</div></div>'
        elif r["prev"]:
            line2 = f'<div class="l2"><div class="p">{r["prev"]}</div></div>'
        else:
            line2 = f'<div class="l2"><div class="p dim">#%d · deepseek-v4-flash · map/qrd3wh</div></div>' % r["id"]
        unread = f'<span class="badge">{r["unread"]}</span>' if r["unread"] else ""
        out.append(
            f'<div class="{cls}"><div class="l1">{pin}<span class="nm">{r["name"]}</span>{when}</div>'
            f'{line2}{unread}</div>'
        )
    return "".join(out)


def nav_column() -> str:
    def group(label, n, items):
        rows = "".join(
            f'<div class="nrow{" cur" if c else ""}"><span class="dot"></span><span class="nt">{t}</span></div>'
            for t, c in items
        )
        return (
            f'<div class="ngroup"><div class="nhead"><span class="caret">▾</span>'
            f'<span class="nlabel">{label}</span><span class="ncount">{n}</span></div>{rows}</div>'
        )

    return (
        '<div class="nav">'
        '<div class="nhead-bar"><span class="nt2">你问过的</span><span class="nsub">按天 · 12 条</span></div>'
        '<div class="ntoggles"><span class="tg">折叠旧组</span><span class="tg on">自动展开</span><span class="tg on">悬停预览</span></div>'
        + group("今天", 3, [("部署完之后 cooldown 还是 5 秒", False), ("镜像拉完了，systemd 起来", True), ("先把 faux 引擎跑起来", False)])
        + group("昨天", 2, [("灯带固件回滚", False), ("旧库还在 /var/backups 里", False)])
        + group("11-20", 2, [("模板：只读巡检", False), ("对话 344", False)])
        + "</div>"
    )


def chat_pane() -> str:
    bub = lambda me, text: (
        f'<div class="msg {"me" if me else ""}"><div class="bubble {"bme" if me else "bot"}">{text}</div></div>'
    )
    return (
        '<div class="chat">'
        '<div class="chead">'
        '<span class="ctitle">qrd3wh 部署验证</span>'
        '<span class="cbrief">deepseek-v4-flash · 5 技能 ›</span>'
        '<span class="cinfo">主视图最近 6 条</span>'
        '<span class="cgrow"></span>'
        f'<span class="cbtn">{svg("branch", 15)} 2 分支</span>'
        f'<span class="cbtn only">{svg("panel", 15)}</span>'
        f'<span class="cbtn only">{svg("history", 15)} 历史与目录</span>'
        "</div>"
        '<div class="cbody">'
        + bub(True, "把 j1900 上那版镜像重新拉一遍，然后看 cooldown 到底是多少。")
        + bub(False, "镜像拉完了，冷启动 4.2 秒。cooldown 现在是 5 秒 —— 这个值在 /var/lib/nous/config.toml 里，不在镜像里，所以重拉不会变。")
        + '<div class="msg"><div class="tool"><div class="tool-h"><span class="tt">bash</span><span class="tc">hostname; pwd; systemctl is-active nous-app</span></div><div class="tool-b">j1900<br>/var/lib/nous<br>active</div></div></div>'
        + bub(True, "那就直接把 cooldown 改成 2 秒，重启服务再看。")
        + bub(False, "改完了，重启之后 cooldown=2s 生效。下一次触发会快 3 秒。")
        + "</div>"
        '<div class="composer">'
        '<div class="ccard">'
        '<div class="cwell">改完了，重启之后…</div>'
        '<div class="ctools">'
        f'<span class="cicon on">{svg("sliders", 18, 1.6)}</span>'
        f'<span class="cicon">{svg("layers", 18, 1.6)}</span>'
        f'<span class="cicon">{svg("archive", 18, 1.6)}</span>'
        f'<span class="cicon">{svg("star", 18, 1.6)}</span>'
        '<span class="cgrow"></span>'
        f'<span class="cicon">{svg("menu", 18, 1.6)}</span>'
        '<span class="send">发送</span>'
        "</div></div></div></div>"
    )


# ── 12 套按钮行 ───────────────────────────────────────────────────────────
# 每套：(短名, 一句话想干什么, HTML)
def ico(n, s=15.0, w=1.7):
    return svg(n, s, w)


PLANS = {}

PLANS["1"] = (
    "描边胶囊",
    "最保守：四个带框的小块，选中反白。像一排标签，不动列表的节奏。",
    '<div class="btnrow v1">'
    '<button class="p on">全部 <b>149</b></button>'
    '<button class="p">未读 <b>3</b></button>'
    '<button class="p">置顶 <b>2</b></button>'
    '<button class="p">归档</button>'
    "</div>",
)

PLANS["2"] = (
    "纯图标",
    "最省地方：四颗 28×28 图标，右端挂一个计数。不写字，靠图标认。",
    '<div class="btnrow v2">'
    f'<button class="ib on">{ico("filter", 16)}</button>'
    f'<button class="ib">{ico("unread", 16)}</button>'
    f'<button class="ib">{ico("pin", 16)}</button>'
    f'<button class="ib">{ico("archive", 16)}</button>'
    '<span class="cgrow"></span><span class="cnt">149 条</span>'
    "</div>",
)

PLANS["3"] = (
    "连体分段",
    "一整块 1px 黑框切成四段，选中的那段整块反白。读作「一个开关有四个档」。",
    '<div class="btnrow v3"><div class="seg">'
    '<button class="on">全部</button><button>未读</button><button>置顶</button><button>归档</button>'
    "</div></div>",
)

PLANS["4"] = (
    "下划线 tab",
    "不带框，只有文字；选中的那条底下一条 2px 黑线。最像浏览器/微信顶部标签。",
    '<div class="btnrow v4">'
    '<button class="tab on">全部</button><button class="tab">未读 <b>3</b></button>'
    '<button class="tab">置顶</button><button class="tab">归档</button>'
    "</div>",
)

PLANS["5"] = (
    "方块网格",
    "四个等宽方块（只有图标），各自独立描边。最像机关面板上的实体按键。",
    '<div class="btnrow v5">'
    f'<button class="sq on">{ico("list", 16)}</button>'
    f'<button class="sq">{ico("grid", 16)}</button>'
    f'<button class="sq">{ico("pin", 16)}</button>'
    f'<button class="sq">{ico("archive", 16)}</button>'
    "</div>",
)

PLANS["6"] = (
    "带键帽",
    "每颗按钮右边挂一枚键帽（⌘1…）。给「以后要能用键盘切」留位置。",
    '<div class="btnrow v6">'
    f'<button class="b on">{ico("list", 13)}全部<span class="kb">⌘1</span></button>'
    f'<button class="b">{ico("unread", 13)}未读<span class="kb">⌘2</span></button>'
    f'<button class="b">{ico("pin", 13)}置顶<span class="kb">⌘3</span></button>'
    "</div>",
)

PLANS["7"] = (
    "大卡片",
    "两颗 40px 高卡，主副两行。占地方但每项都能写清楚「是什么 + 多少条」。",
    '<div class="btnrow v7">'
    f'<button class="card2 on">{ico("list", 14)}<b>全部</b><i>149 条</i></button>'
    f'<button class="card2">{ico("clock", 14)}<b>最近</b><i>3 条未读</i></button>'
    f'<button class="card2">{ico("pin", 14)}<b>置顶</b><i>2 条</i></button>'
    "</div>",
)

PLANS["8"] = (
    "左侧色条",
    "每颗按钮左边一条 3px 竖条，选中那条变黑。跟会话行的选中样式同一句话。",
    '<div class="btnrow v8">'
    '<button class="lb on">全部</button><button class="lb">未读</button>'
    '<button class="lb">置顶</button><button class="lb">归档</button>'
    "</div>",
)

PLANS["9"] = (
    "极简符号",
    "四个几何符号，无框无字，选中的只是变深。近乎看不见，最不打扰列表。",
    '<div class="btnrow v9">'
    '<button class="sy on">▤</button><button class="sy">▥</button>'
    '<button class="sy">▦</button><button class="sy">⌗</button>'
    '<span class="cgrow"></span><span class="sy-label">筛选</span>'
    "</div>",
)

PLANS["10"] = (
    "黑底一条",
    "整行铺满黑底反白。最重、最像「这是工具栏不是列表的一部分」。",
    '<div class="btnrow v10">'
    f'<button class="dk on">{ico("list", 14)}全部</button>'
    f'<button class="dk">{ico("unread", 14)}未读</button>'
    f'<button class="dk">{ico("pin", 14)}置顶</button>'
    '<span class="cgrow"></span><span class="dk-cnt">149</span>'
    "</div>",
)

PLANS["11"] = (
    "下拉 + 主按钮",
    "只留两个控件：左边一个「全部 ▾」下拉，右边一颗黑底主按钮。最少的东西。",
    '<div class="btnrow v11">'
    '<button class="sel">全部<span class="cv">▾</span></button>'
    '<span class="cgrow"></span>'
    f'<button class="primary">{ico("filter", 14)}筛选</button>'
    "</div>",
)

PLANS["12"] = (
    "微缩一行",
    "11px 小字 + 圆点分隔，一行塞六项。密度最高，视觉重量最轻。",
    '<div class="btnrow v12">'
    '<button class="tiny on">全部 149</button><i class="sd">·</i>'
    '<button class="tiny">未读 3</button><i class="sd">·</i>'
    '<button class="tiny">置顶 2</button><i class="sd">·</i>'
    '<button class="tiny">归档</button><i class="sd">·</i>'
    '<button class="tiny">模板</button>'
    "</div>",
)


CSS_PLANS = """
/* ── 按钮行各自的样子（切换 body[data-v] 生效）────────────────────────── */
.btnrow{display:flex;align-items:center;gap:6.3px;padding:0 12.6px 8.4px;flex-wrap:nowrap;
  min-width:0;overflow:hidden}
.cgrow{flex:1;min-width:0}
.btnrow button{font:inherit;cursor:pointer;border:0;background:transparent;color:var(--text2)}
.btnrow button b{font-weight:600;font-variant-numeric:tabular-nums}

/* 1 描边胶囊 */
.v1 .p{border:1px solid var(--line2);padding:4.2px 8.4px;font-size:12px;line-height:1.2;
  white-space:nowrap;display:flex;gap:4px;align-items:center}
.v1 .p b{font-size:10.5px;color:var(--muted)}
.v1 .p.on{background:var(--text);color:#fff;border-color:var(--text)}
.v1 .p.on b{color:rgba(255,255,255,.7)}
.v1 .p:hover:not(.on){background:var(--hover);border-color:var(--text2)}

/* 2 纯图标 */
.v2 .ib{width:28px;height:28px;display:grid;place-items:center;color:var(--text2)}
.v2 .ib.on{background:var(--text);color:#fff}
.v2 .ib:hover:not(.on){background:var(--hover);color:var(--text)}
.v2 .cnt{font-size:11px;color:var(--muted);font-variant-numeric:tabular-nums;padding-right:2px}

/* 3 连体分段 */
.v3 .seg{display:flex;flex:1;min-width:0;border:1px solid var(--text)}
.v3 .seg button{flex:1;min-width:0;height:25.2px;font-size:12px;color:var(--text2);
  border-left:1px solid var(--text);white-space:nowrap}
.v3 .seg button:first-child{border-left:0}
.v3 .seg button.on{background:var(--text);color:#fff}
.v3 .seg button:hover:not(.on){background:var(--hover)}

/* 4 下划线 tab */
.v4{border-bottom:1px solid var(--line2);padding-bottom:0;gap:14.7px}
.v4 .tab{padding:2px 0 7.35px;border-bottom:2.1px solid transparent;font-size:12.5px;
  margin-bottom:-1.05px;color:var(--text2)}
.v4 .tab b{font-size:10.5px;color:var(--muted)}
.v4 .tab.on{color:var(--text);border-bottom-color:var(--text)}
.v4 .tab:hover{color:var(--text)}

/* 5 方块网格 */
.v5{gap:6.3px}
.v5 .sq{flex:1;min-width:0;height:33.6px;display:grid;place-items:center;
  border:1px solid var(--line2);color:var(--text2)}
.v5 .sq.on{background:var(--text);color:#fff;border-color:var(--text)}
.v5 .sq:hover:not(.on){border-color:var(--text);color:var(--text)}

/* 6 带键帽 */
.v6{gap:2.1px}
.v6 .b{display:flex;align-items:center;gap:3.15px;height:25.2px;padding:0 5.25px;
  border:1px solid var(--line2);font-size:11.5px;white-space:nowrap}
.v6 .b.on{background:var(--text);color:#fff;border-color:var(--text)}
.v6 .b:hover:not(.on){border-color:var(--text2);background:var(--hover)}
.v6 .kb{font-family:mono;font-size:9px;padding:0 3px;line-height:1.7;
  border:1px solid var(--line2);color:var(--muted)}
.v6 .b.on .kb{border-color:rgba(255,255,255,.32);color:rgba(255,255,255,.86)}
.v6 .only{padding:0 5.25px;margin-left:auto}

/* 7 大卡片 */
.v7{gap:6.3px}
.v7 .card2{flex:1;min-width:0;height:42px;display:flex;flex-direction:column;
  align-items:center;justify-content:center;gap:1px;border:1px solid var(--line2);
  color:var(--text2);padding:0 2.1px}
.v7 .card2.on{background:var(--text);color:#fff;border-color:var(--text)}
.v7 .card2:hover:not(.on){border-color:var(--text);background:var(--hover);color:var(--text)}
.v7 .card2 b{font-size:11px;font-weight:500;white-space:nowrap;line-height:1.2}
.v7 .card2 i{font-style:normal;font-size:9.5px;color:var(--muted);white-space:nowrap;line-height:1.2}
.v7 .card2.on i{color:rgba(255,255,255,.64)}

/* 8 左侧色条 */
.v8{gap:2.1px}
.v8 .lb{border-left:3.15px solid var(--faint);padding:4.2px 9.45px;font-size:12px;
  white-space:nowrap;color:var(--text2)}
.v8 .lb.on{border-left-color:var(--text);color:var(--text);font-weight:500}
.v8 .lb:hover{background:var(--hover);border-left-color:var(--text2)}

/* 9 极简符号 */
.v9{gap:2.1px}
.v9 .sy{width:26.25px;height:24.15px;display:grid;place-items:center;font-size:15.75px;
  color:var(--muted);line-height:1}
.v9 .sy.on{color:var(--text)}
.v9 .sy:hover{color:var(--text);background:var(--hover)}
.v9 .sy-label{font-size:11px;color:var(--faint);padding-right:2px}

/* 10 黑底一条 */
.v10{background:var(--text);color:#fff;padding:6.3px 12.6px;gap:2.1px}
.v10 .dk{display:flex;align-items:center;gap:4.2px;padding:3.15px 8.4px;font-size:12px;
  color:rgba(255,255,255,.62);white-space:nowrap}
.v10 .dk.on{color:#fff;background:rgba(255,255,255,.16)}
.v10 .dk:hover{color:#fff}
.v10 .dk-cnt{font-size:11px;color:rgba(255,255,255,.5);font-variant-numeric:tabular-nums}

/* 11 下拉 + 主按钮 */
.v11 .sel{display:flex;align-items:center;gap:5.25px;height:28.35px;padding:0 8.4px;
  border:1px solid var(--line2);font-size:12px;color:var(--text)}
.v11 .sel:hover{border-color:var(--text)}
.v11 .cv{font-size:9px;color:var(--muted)}
.v11 .primary{display:flex;align-items:center;gap:5.25px;height:28.35px;padding:0 12.6px;
  background:var(--text);color:#fff;font-size:12px}
.v11 .primary:hover{background:var(--accent-hover)}

/* 12 微缩一行 */
.v12{gap:0;align-items:center;padding-bottom:7.35px}
.v12 .tiny{font-size:11px;color:var(--muted);padding:2.1px 3.15px;white-space:nowrap}
.v12 .tiny.on{color:var(--text);font-weight:500}
.v12 .tiny:hover{color:var(--text);text-decoration:underline;text-underline-offset:2px}
.v12 .sd{font-style:normal;color:var(--faint);font-size:11px}
"""

BASE_CSS = """
:root{
  --sidebar:#e9e6e0; --sidebar-hover:#dedad3; --row-hover:#eeebe5; --sidebar-open:#d6d2c9;
  --list-bg:#f4f2ee; --chat-bg:#f4f2ee; --panel:#ffffff; --panel-alt:#efece7;
  --bubble-me:#efece7; --bubble-other:#ffffff;
  --text:#14150f; --text2:#5f5d55; --muted:#8d8a82; --faint:#b9b6ad;
  --line:#14150f; --line2:#d8d5cd;
  --hover:#efece7; --selected:#e6e2da; --accent:#14150f; --accent-hover:#2a2b22;
  --danger:#a8281c; --ok:#1d6b46;
}
*{box-sizing:border-box}
*::-webkit-scrollbar{width:8px;height:8px}
*::-webkit-scrollbar-track{background:transparent}
*::-webkit-scrollbar-thumb{background:rgba(20,21,15,.20)}
*::-webkit-scrollbar-thumb:hover{background:rgba(20,21,15,.34)}
html,body{margin:0;height:100%}
body{background:#cfcbc3;color:var(--text);font-family:manrope,"Noto Sans CJK SC","PingFang SC",sans-serif;
  font-size:12px;-webkit-font-smoothing:antialiased;display:flex;flex-direction:column;height:100vh;overflow:hidden}
svg{display:block;flex:none}

/* 顶部切换条（不属于被审的页面） */
.topbar{flex:none;background:var(--panel);border-bottom:1px solid var(--line2);
  padding:9px 14px;display:flex;flex-wrap:wrap;gap:10px 14px;align-items:center;z-index:9}
.topbar h1{font-size:12.5px;font-weight:600;margin:0;white-space:nowrap}
.topbar .sub{font-size:11px;color:var(--muted);white-space:nowrap}
.vchips{display:flex;flex-wrap:wrap;gap:4px}
.vchips button{font:inherit;font-size:11px;padding:3.15px 7.35px;border:1px solid var(--line2);
  background:transparent;color:var(--text2);cursor:pointer;white-space:nowrap}
.vchips button.on{background:var(--text);color:#fff;border-color:var(--text)}
.legend{flex:none;padding:7px 14px;background:var(--panel-alt);border-bottom:1px solid var(--line2);
  font-size:11.5px;color:var(--text2);display:flex;gap:8px;align-items:baseline}
.legend b{color:var(--text);font-weight:600}

/* 被审的窗口 */
.stage{flex:1;min-height:0;display:flex;justify-content:center;align-items:flex-start;
  padding:14px;overflow:auto}
.win{width:1240px;height:100%;min-height:640px;max-height:820px;display:flex;overflow:hidden;
  background:var(--chat-bg);border:1px solid #a8a49b;
  box-shadow:0 16px 44px rgba(0,0,0,.22);flex:none}

/* rail */
.rail{width:56px;flex:none;background:var(--sidebar);display:flex;flex-direction:column;
  align-items:center;padding:10.5px 0;gap:6.3px}
.rail .rb{width:40px;height:40px;display:grid;place-items:center;color:var(--muted);cursor:pointer}
.rail .rb.on{background:var(--sidebar-open);color:var(--text)}
.rail .rb:hover{background:var(--sidebar-hover);color:var(--text)}
.rail .grow{flex:1}

/* 会话列 */
.col{width:280px;flex:none;display:flex;flex-direction:column;min-height:0;background:var(--list-bg)}
.head{display:flex;align-items:center;gap:6.3px;padding:12.6px 12.6px 8.4px;min-width:0}
.box{flex:1;min-width:0;display:flex;align-items:center;gap:6.3px;height:28.35px;
  padding:0 8.4px;border:1px solid var(--line2);background:var(--panel);color:var(--faint)}
.box .q{flex:1;min-width:0;font-size:12px;color:var(--text2);overflow:hidden;
  text-overflow:ellipsis;white-space:nowrap}
.box .n{flex:none;font-size:11px;color:var(--muted);font-variant-numeric:tabular-nums}
.plus{flex:none;width:28.35px;height:28.35px;display:grid;place-items:center;
  color:var(--text2);cursor:pointer}
.plus:hover{background:var(--hover);color:var(--text)}

.list{flex:1;min-height:0;overflow-y:auto;overflow-x:hidden}
.list::-webkit-scrollbar{width:8px}
.list::-webkit-scrollbar-thumb{background:rgba(20,21,15,.18)}
.row{display:flex;flex-direction:column;justify-content:center;min-height:58.8px;
  padding:6.3px 10.5px 6.3px 8.4px;gap:2.1px;border-left:2.1px solid transparent;
  border-bottom:1.05px solid var(--line);cursor:pointer;position:relative}
.row.on{background:var(--selected)}
.row:hover:not(.on){background:var(--row-hover);border-left-color:var(--text)}
.l1{display:flex;align-items:center;gap:6.3px;width:100%;min-width:0}
.pin{flex:none;display:grid;place-items:center;color:var(--ok)}
.nm{flex:1;min-width:0;font-size:13px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.tm{flex:none;font-size:11px;color:var(--muted);font-variant-numeric:tabular-nums}
.l2{display:flex;align-items:center;gap:6.3px;width:100%;min-width:0}
.l2 .p{flex:1;min-width:0;font-size:12px;color:var(--muted);overflow:hidden;
  text-overflow:ellipsis;white-space:nowrap}
.l2 .p.dim{color:var(--faint);font-family:mono;font-size:10.5px}
.l2 .p.busy{color:var(--text);display:flex;align-items:center;gap:4.2px}
.l2 .p.busy i{width:6.3px;height:6.3px;background:var(--text);flex:none}
.badge{position:absolute;right:10.5px;bottom:9px;min-width:14.7px;height:14.7px;
  background:var(--danger);color:#fff;font-size:10px;display:grid;place-items:center;
  padding:0 3px;line-height:1}

/* 目录列 */
.nav{width:240px;flex:none;display:flex;flex-direction:column;min-height:0;
  background:var(--panel-alt);border-right:1px solid var(--line2);overflow-y:auto}
.nhead-bar{display:flex;align-items:center;gap:6.3px;padding:8.4px 12.6px;
  border-bottom:1px solid var(--line2)}
.nt2{font-size:11.5px;color:var(--text2)}
.nsub{font-size:11px;color:var(--muted)}
.ntoggles{display:flex;flex-wrap:wrap;gap:5.25px;padding:6.3px 8.4px;
  border-bottom:1px solid var(--line2)}
.tg{font-size:10.5px;color:var(--muted);border:1px solid var(--line2);padding:1.05px 5.25px}
.tg.on{color:var(--text);border-color:var(--text2);background:var(--panel)}
.ngroup{margin-bottom:4.2px;padding-top:4.2px}
.nhead{display:flex;align-items:center;gap:6.3px;padding:6.3px 8.4px;cursor:pointer}
.nhead:hover{background:var(--hover)}
.caret{font-size:10px;color:var(--muted)}
.nlabel{font-size:11.5px;color:var(--muted)}
.ncount{margin-left:auto;font-size:10.5px;color:var(--faint)}
.nrow{display:flex;align-items:flex-start;gap:6.3px;padding:4.2px 8.4px 4.2px 14.7px;
  cursor:pointer}
.nrow:hover{background:var(--hover)}
.nrow .dot{width:4.2px;height:4.2px;background:var(--faint);flex:none;margin-top:5.25px}
.nt{font-size:11.5px;color:var(--text2);overflow:hidden;text-overflow:ellipsis;
  display:-webkit-box;-webkit-line-clamp:1;line-height:1.45}
.nrow.cur{background:var(--selected)}
.nrow.cur .dot{background:var(--text)}
.nrow.cur .nt{color:var(--text)}

/* 聊天区 */
.chat{flex:1;min-width:0;display:flex;flex-direction:column;background:var(--chat-bg);min-height:0}
.chead{height:50.4px;flex:none;display:flex;align-items:center;gap:9.45px;padding:0 12.6px;
  background:var(--list-bg);min-width:0}
.ctitle{font-size:12.5px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;flex:none;max-width:200px}
.cbrief{flex:none;font-size:11px;color:var(--muted);padding:2.1px 6.3px;max-width:200px;
  white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.cbrief:hover{background:var(--panel);color:var(--text)}
.cinfo{flex:none;font-size:11px;color:var(--muted);white-space:nowrap}
.cgrow{flex:1;min-width:0}
.cbtn{flex:none;display:flex;align-items:center;gap:5.25px;height:29.4px;padding:0 10.5px;
  border:1px solid var(--line);background:var(--panel);font-size:12px;color:var(--text2);
  white-space:nowrap;cursor:pointer}
.cbtn:hover{background:var(--hover)}
.cbtn.only{padding:0 8.4px}
.cbody{flex:1;min-height:0;overflow-y:auto;padding:8.4px 0}
.cbody::-webkit-scrollbar{width:8px}
.cbody::-webkit-scrollbar-thumb{background:rgba(20,21,15,.18)}
.msg{display:flex;padding:4.2px 12.6px}
.msg.me{justify-content:flex-end}
.bubble{max-width:504px;padding:8.4px 12.6px;font-size:12.5px;line-height:1.55;
  border:1px solid var(--line2)}
.bme{background:var(--bubble-me);border-color:var(--line2)}
.bot{background:var(--bubble-other);border-color:var(--line2)}
.tool{border:1px solid var(--line);max-width:504px;background:var(--panel)}
.tool-h{display:flex;gap:8.4px;align-items:center;padding:3.15px 8.4px;
  border-bottom:1px solid var(--line2);font-family:mono;font-size:10.5px}
.tt{color:var(--text);flex:none}
.tc{color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.tool-b{padding:6.3px 8.4px;font-family:mono;font-size:10.5px;color:var(--text2);line-height:1.6}

/* 输入区 */
.composer{flex:none;background:var(--hover);padding:8.4px 10.5px 10.5px}
.ccard{background:var(--panel);border:1px solid rgba(0,0,0,.05);
  box-shadow:0 1px 3px rgba(0,0,0,.08)}
.cwell{padding:10.5px 12.6px 4.2px;font-size:13.5px;line-height:20px;color:var(--faint);min-height:44px}
.ctools{display:flex;align-items:center;gap:2.1px;padding:2.1px 8.4px 8.4px}
.cicon{width:29.4px;height:29.4px;display:grid;place-items:center;color:var(--muted)}
.cicon.on{color:var(--text)}
.cicon:hover{background:var(--hover);color:var(--text)}
.send{height:27.3px;padding:0 12.6px;display:grid;place-items:center;background:var(--text);
  color:#fff;font-size:12.5px;margin-left:6.3px}
"""

JS = """
const PLANS = __PLANS__;
const body = document.body;
const chips = document.getElementById('vchips');
const legend = document.getElementById('legend');
const rowHost = document.getElementById('btnrow-host');

function paint(v){
  body.dataset.v = v;
  rowHost.innerHTML = PLANS[v].html;
  document.getElementById('lname').textContent = v + ' · ' + PLANS[v].name;
  document.getElementById('ldesc').textContent = PLANS[v].desc;
  [...chips.children].forEach(b => b.classList.toggle('on', b.dataset.v === v));
  location.hash = v;
}

Object.keys(PLANS).sort((a,b)=>+a-+b).forEach(v => {
  const b = document.createElement('button');
  b.dataset.v = v; b.textContent = v + ' ' + PLANS[v].name;
  b.onclick = () => paint(v);
  chips.appendChild(b);
});

// 点按钮只切高亮，不做任何事（功能未定）
rowHost.addEventListener('click', e => {
  const t = e.target.closest('button');
  if(!t) return;
  const scope = t.parentElement;
  [...scope.querySelectorAll('button')].forEach(b => { if(b !== t) b.classList.remove('on'); });
  t.classList.add('on');
});

window.addEventListener('hashchange', () => paint((location.hash || '').replace('#','') || '1'));
paint((location.hash || '').replace('#','') || '1');
"""


def main() -> None:
    fonts = dict(
        M400=b64(FONTS / "manrope-400.ttf"),
        M500=b64(FONTS / "manrope-500.ttf"),
        M700=b64(FONTS / "manrope-700.ttf"),
        J400=b64(FONTS / "jetbrainsmono-400.ttf"),
    )
    fonts_css = "".join(
        f"@font-face{{font-family:{fam};src:url(data:font/ttf;base64,{fonts[key]}) format('truetype');font-weight:{w};font-style:normal}}\n"
        for key, fam, w in [
            ("M400", "manrope", 400),
            ("M500", "manrope", 500),
            ("M700", "manrope", 700),
            ("J400", "mono", 400),
        ]
    )

    rail = "".join(
        f'<span class="rb{" on" if n == "chats" else ""}">{svg(n, 24, 1.7)}</span>'
        for n in ["chats", "templates", "triggers", "resources", "elements", "data"]
    ) + (
        '<span class="grow"></span>'
        f'<span class="rb">{svg("menu", 20, 1.8)}</span>'
    )

    plans_js = "{"
    for v, (name, desc, html) in PLANS.items():
        h = html.replace("\\", "\\\\").replace("`", "\\`").replace("${", "\\${")
        plans_js += f'"{v}":{{name:`{name}`,desc:`{desc}`,html:`{h}`}},'
    plans_js += "}"

    first = PLANS["1"]

    html = f"""<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>会话列表搜索栏下方那一行 · 12 套按钮</title>
<style>
{fonts_css}
{BASE_CSS}
{CSS_PLANS}
</style>
</head>
<body data-v="1">

<div class="topbar">
  <h1>搜索栏下面那一行 · 12 套</h1>
  <div class="vchips" id="vchips"></div>
</div>
<div class="legend">
  <b id="lname">1 · {first[0]}</b>
  <span id="ldesc">{first[1]}</span>
</div>

<div class="stage">
  <div class="win">
    <div class="rail">{rail}</div>
    <div class="col">
      <div class="head">
        <div class="box">{svg("search", 14, 2)}<span class="q">搜索会话</span><span class="n">12</span></div>
        <span class="plus">{svg("plus", 15, 2)}</span>
      </div>
      <div id="btnrow-host">{first[2]}</div>
      <div class="list">{session_rows()}</div>
    </div>
    {nav_column()}
    {chat_pane()}
  </div>
</div>

<script>
{JS.replace("__PLANS__", plans_js)}
</script>
</body>
</html>
"""
    OUT.write_text(html, encoding="utf-8")
    print(f"写出 {OUT}  {len(html) / 1024:.0f} KB")


if __name__ == "__main__":
    main()
