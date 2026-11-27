#!/usr/bin/env python3
"""生成 demo/plugin-layers/preview.html（单文件、双击可开、无端口）。

数据来源：
- 模块元数据（vendor/version/layer/crate/desc/requires）：../plugin-relations/data.json
  （从内核仓各 plugin.toml 抄的真数据）
- 端点数：j1900 0.1.41 真机 `GET /v1/_routes` 按 module 分组
"""
import json
import pathlib

HERE = pathlib.Path(__file__).parent
DATA = json.loads((HERE.parent / "plugin-relations/data.json").read_text(encoding="utf-8"))

# capabilities.modules（15 个）→ module.name
MODULES = [
    "sessions", "env", "repo", "presets", "presets-admin", "Skill", "archive",
    "combos", "Trigger_Core", "data-manager", "Device", "Nous_Prompts",
    "Extend", "Models", "elements",
]
# GET /v1/_routes 真机（0.1.41，165 条）按 module 分组
ENDPOINTS = {
    "repo": 30, "Trigger_Core": 24, "presets-admin": 19, "sessions": 17,
    "Extend": 15, "presets": 13, "Device": 9, "data-manager": 8, "elements": 8,
    "Skill": 6, "Nous_Prompts": 6, "combos": 4, "archive": 3, "Models": 2, "env": 1,
}
# data.json 里没有 presets（同一 crate PresetsAdmin 报两个模块名），补一条
EXTRA = {
    "name": "presets",
    "vendor": "Nous",
    "version": "0.1.41",
    "layer": "app",
    "crate": "nous-presets-admin",
    "desc": "预设（内容真相在 git 私库）",
    "requires": ["nous-durable", "nous-presets-admin", "nous-repo", "nous-routes"],
}
SKIP = {"documents"}  # crate 已删（#90）
# 真机核对（j1900 0.1.41）：这条端点能返回 200，但不在 /v1/_routes 的 165 条里，
# 也不在 capabilities.modules —— 插件自带了端点却没进 MANIFEST，是另一件事，这里如实标出来。
UNREGISTERED = {"AboutME_ModuleRelationshipNetwork": 1}

by_name = {m["name"]: m for m in DATA if m["name"] not in SKIP}
by_name["presets"] = EXTRA


def norm(m):
    e = ENDPOINTS.get(m["name"], UNREGISTERED.get(m["name"], 0))
    return {
        "name": m["name"],
        "vendor": m["vendor"],
        "version": m["version"],
        "layer": m["layer"],
        "crate": m["crate"],
        "desc": m["desc"],
        "requires": m["requires"],
        "endpoints": e,
        "registered": m["name"] in MODULES,
        "endpoint_unlisted": m["name"] in UNREGISTERED,
    }


rows = [norm(m) for m in by_name.values()]
# 端点模块按端点数降序（跟 Desk 现有列表一致），库按名字排
modules = sorted([r for r in rows if r["registered"]], key=lambda r: (-r["endpoints"], r["name"]))
unlisted = sorted([r for r in rows if r.get("endpoint_unlisted")], key=lambda r: r["name"])
libs = sorted([r for r in rows if not r["registered"] and not r.get("endpoint_unlisted")],
              key=lambda r: r["name"])

# 依赖 / 被依赖 反查（包名 → 模块名）
pkg_to_name = {r["crate"]: r["name"] for r in rows}
deps = {r["name"]: [pkg_to_name.get(p, p) for p in r["requires"]] for r in rows}
dependents = {r["name"]: sorted(r["name"] for r in rows if r["name"] in deps[r["name"]]) for r in rows}

payload = {
    "modules": modules,
    "unlisted": unlisted,
    "libs": libs,
    "deps": deps,
    "dependents": dependents,
    "moduleCount": len(MODULES),
    "libCount": len(libs),
    # graph 节点数（扫 plugin.toml 得来）：31 = 14 个 caps 模块 + AboutME + 16 库；
    # 注意 caps 的 `presets` 与 `presets-admin` 同属一个 crate，graph 里只有一个节点。
    "graphNodes": 31,
}

HTML = """<!doctype html>
<html lang="zh-CN"><head>
<meta charset="utf-8">
<title>插件页 · 依赖层也列出来（原型）</title>
<style>
:root{
  --bg:#000; --fg:#e8e8e8; --dim:#8a8a8a; --faint:#5a5a5a;
  --line:#2a2a2a; --line-soft:#1c1c1c; --sel:#161616;
  --warn:#d9a54a; --warn-soft:#241c0c; --bad:#c2534a; --bad-soft:#241010;
  --ok:#5f9a6a; --ok-soft:#0f1a12;
}
*{box-sizing:border-box}
html,body{margin:0;background:var(--bg);color:var(--fg);
  font:12.5px/1.55 "Inter",-apple-system,"Segoe UI","Noto Sans CJK SC",sans-serif;
  -webkit-font-smoothing:antialiased}
.mono{font-family:"JetBrains Mono",ui-monospace,SFMono-Regular,Menlo,monospace}

/* ── 顶部说明 ───────────────────────────────── */
.top{padding:26px 30px 18px;border-bottom:1px solid var(--line)}
.top h1{margin:0 0 8px;font-size:19px;font-weight:600;letter-spacing:.2px}
.top p{margin:0 0 6px;color:var(--dim);max-width:960px}
.top b{color:var(--fg);font-weight:500}
.nums{display:flex;gap:26px;margin:16px 0 0;flex-wrap:wrap}
.num{display:flex;flex-direction:column;gap:2px}
.num .v{font-size:22px;font-weight:600;font-family:"JetBrains Mono",monospace}
.num .k{font-size:10.5px;color:var(--faint);letter-spacing:.4px}
.num.hi .v{color:var(--warn)}
.num.dim .v{color:var(--dim)}

.tabs{display:flex;gap:8px;margin-top:18px;align-items:center}
.tab{padding:5px 13px;border:1px solid var(--line);background:transparent;color:var(--dim);
  font:inherit;font-size:12px;cursor:pointer;border-radius:0}
.tab:hover{color:var(--fg);background:var(--sel)}
.tab.on{color:#000;background:var(--fg);border-color:var(--fg);font-weight:500}
.tabs .hint{color:var(--faint);font-size:11px;margin-left:6px}

/* ── 模拟器 ─────────────────────────────────── */
.frame{display:flex;height:calc(100vh - 300px);min-height:520px;margin:0}
.list{width:330px;flex:none;border-right:1px solid var(--line);overflow-y:auto;padding:14px 0}
.detail{flex:1;overflow-y:auto;padding:22px 26px}

.sec{padding:8px 18px 5px;font-size:10.5px;letter-spacing:.6px;color:var(--faint);
  display:flex;justify-content:space-between;align-items:baseline}
.sec .n{font-family:"JetBrains Mono",monospace}
.row{display:flex;align-items:center;gap:8px;padding:7px 18px;cursor:pointer;
  border-left:2px solid transparent}
.row:hover{background:var(--sel)}
.row.on{background:var(--sel);border-left-color:var(--fg)}
.row .nm{font-size:12.5px;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.row .vd{font-size:9.5px;color:var(--faint);border:1px solid var(--line);
  padding:0 4px;line-height:14px;flex:none}
.row .sp{flex:1}
.row .ep{font-family:"JetBrains Mono",monospace;font-size:10.5px;color:var(--dim);flex:none}
.row.lib .nm{color:var(--dim)}
.row .tag{font-size:9.5px;color:var(--faint);border:1px solid var(--line-soft);
  padding:0 4px;line-height:14px;flex:none}
.divider{height:1px;background:var(--line);margin:12px 18px 6px}
.zone-note{padding:4px 18px 8px;font-size:10.5px;color:var(--faint);line-height:1.45}

/* ── 详情 ───────────────────────────────────── */
.dh{display:flex;align-items:baseline;gap:10px;flex-wrap:wrap}
.dh .n{font-size:17px;font-weight:600}
.pill{font-size:10px;border:1px solid var(--line);padding:1px 6px;color:var(--dim)}
.pill.v{font-family:"JetBrains Mono",monospace}
.pill.bad{color:var(--bad);border-color:var(--bad);background:var(--bad-soft)}
.pill.ok{color:var(--ok);border-color:#2b3f2f;background:var(--ok-soft)}
.pill.warn{color:var(--warn);border-color:#4a3a18;background:var(--warn-soft)}
.dsub{margin-top:6px;color:var(--dim);max-width:760px}
.meta{display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));
  gap:10px 22px;margin:18px 0 0;padding:14px 0;border-top:1px solid var(--line-soft);
  border-bottom:1px solid var(--line-soft)}
.meta .k{font-size:10px;color:var(--faint);letter-spacing:.4px}
.meta .v{font-size:12px;margin-top:2px}
.meta .v.mono{font-family:"JetBrains Mono",monospace;font-size:11.5px}

.rel{margin-top:20px}
.rel h3{margin:0 0 10px;font-size:10.5px;font-weight:500;color:var(--faint);letter-spacing:.6px}
.chips{display:flex;flex-wrap:wrap;gap:6px}
.chip{font-family:"JetBrains Mono",monospace;font-size:11px;color:var(--fg);
  border:1px solid var(--line);padding:2px 7px;cursor:pointer}
.chip:hover{background:var(--line)}
.chip.invisible{color:var(--dim);border-style:dashed}
.chip.invisible::after{content:" ?";color:var(--bad);font-weight:600}
.empty{color:var(--faint);font-size:11.5px}

/* 「改前点库」的空白态 */
.voidbox{margin-top:20px;border:1px dashed var(--bad);background:var(--bad-soft);
  padding:16px 18px}
.voidbox .t{color:var(--bad);font-size:12.5px;font-weight:500;margin-bottom:6px}
.voidbox p{margin:0;color:var(--dim);font-size:11.5px}

.note{margin-top:22px;border-left:2px solid var(--line);padding:2px 0 2px 12px;
  color:var(--faint);font-size:11.5px;max-width:720px}
.note b{color:var(--dim);font-weight:500}

/* ── 底部票 ─────────────────────────────────── */
.ticket{border-top:1px solid var(--line);padding:20px 30px 30px;background:#050505}
.ticket h2{margin:0 0 10px;font-size:13px;font-weight:600;letter-spacing:.3px}
.ticket ol{margin:0;padding-left:18px;color:var(--dim);max-width:1000px}
.ticket li{margin:4px 0}
.ticket code{font-family:"JetBrains Mono",monospace;font-size:11.5px;color:var(--fg);
  background:#111;padding:1px 4px}
.no{margin-top:12px;color:var(--bad);font-size:11.5px}
</style>
</head><body>

<div class="top">
  <h1>插件页：把「依赖层」也列出来</h1>
  <p>引擎里其实有 <b>两份东西</b>：<b>端点模块</b>（自带 HTTP 端点、客户端能调用）和<b>依赖层</b>（被依赖的库，零端点）。</p>
  <p>现在 Desk 插件页只列了前者，所以 <code class="mono">ai / durable / wire / routes</code> 你在列表里找不到 —— 但关系图里明明有它们，点胶囊跳过去是<b>一片空白</b>。</p>
  <div class="nums">
    <div class="num"><span class="v" id="n-all">–</span><span class="k">有 plugin.toml 的插件</span></div>
    <div class="num dim"><span class="v" id="n-mod">–</span><span class="k">端点模块（现在能看见）</span></div>
    <div class="num hi"><span class="v" id="n-lib">–</span><span class="k">依赖层（现在看不见）</span></div>
  </div>
  <div class="tabs">
    <button class="tab on" data-mode="before">改前</button>
    <button class="tab" data-mode="after">改后</button>
    <span class="hint">点左边任意一行看右边；改前模式下点带 <span class="mono">?</span> 的胶囊，看它为什么是空的。</span>
  </div>
</div>

<div class="frame">
  <aside class="list" id="list"></aside>
  <main class="detail" id="detail"></main>
</div>

<div class="ticket">
  <h2>这张票要做的（A + C，零引擎改动）</h2>
  <ol>
    <li>插件页左侧列表分两段：<code>端点模块 · 15</code> 和 <code>依赖层 · 16</code>，数据都从<b>已有的</b> <code>GET /v1/modules/graph</code> 来（31 个节点 / 96 条边，已经在发）。</li>
    <li>依赖层的行：名字 + vendor + 版本 + 层 + <code>被 N 处依赖</code>，端点一栏留空（不是 <code>0 端点</code>）。</li>
    <li>点依赖层任意一行 → 详情页显示 <code>来源 crate</code> / <code>层</code> / <code>版本</code> / 依赖 / 被依赖，顶部一句「这是库，不提供端点」。</li>
    <li>关系胶囊现在点了会跳到一个不存在的模块 —— 改成跳进这个新详情（闭环）。</li>
    <li>不影响 <code>capabilities</code>：<code>modules[]</code> 仍是 15 个，端点仍是 165 条，<b>不往契约里塞空端点行</b>。</li>
  </ol>
  <div class="no">不在范围：改 <code>/v1/modules/graph</code> 的响应形状（补 layer/version 是另一票 B）、把依赖层写进 capabilities、改引擎。</div>
  <div class="no" style="color:var(--faint)">口径说明：31 是 <code>/v1/modules/graph</code> 的节点数（扫 <code>plugin.toml</code> 得来）；
    15 是 <code>capabilities.modules</code> 的长度；两套名单不完全重合 —— <code>presets</code>
    只在 capabilities 里（与 <code>presets-admin</code> 同 crate），<code>AboutME_ModuleRelationshipNetwork</code>
    只在 graph 里。</div>
</div>

<script>
const DATA = __DATA__;
let mode = new URLSearchParams(location.search).get("mode") === "after" ? "after" : "before";
let selected = "sessions";

const byName = {};
[...DATA.modules, ...DATA.unlisted, ...DATA.libs].forEach(m => byName[m.name] = m);
// 「这个模块在图里有没有」= 有 plugin.toml（两份都有）
// 「这个模块能不能点进详情」= 改前只认 endpoints 模块

function visibleRows() {
  return mode === "before" ? DATA.modules : [...DATA.modules, ...DATA.libs];
}

function hops(name) {
  const m = byName[name];
  return m ? m.endpoints : null;
}

function renderList() {
  const el = document.getElementById("list");
  el.innerHTML = "";
  const sec = (title, n, note) => {
    const d = document.createElement("div");
    d.className = "sec";
    d.innerHTML = `<span>${title}</span><span class="n">${n}</span>`;
    el.appendChild(d);
    if (note) {
      const p = document.createElement("div");
      p.className = "zone-note";
      p.innerHTML = note;
      el.appendChild(p);
    }
  };
  sec("端点模块", DATA.moduleCount,
      mode === "before" ? "自带端点，客户端能调用。" : "自带端点，客户端能调用。");

  const row = (m, isLib) => {
    const r = document.createElement("div");
    r.className = "row" + (isLib ? " lib" : "") + (m.name === selected ? " on" : "");
    const ep = m.endpoint_unlisted ? '<span class="tag" style="color:var(--warn);border-color:#4a3a18">未登记</span>'
             : isLib ? '<span class="tag">无端点</span>'
                     : `<span class="ep">${m.endpoints}</span>`;
    r.innerHTML = `<span class="nm">${m.name}</span>
      <span class="vd">${m.vendor}</span><span class="sp"></span>${ep}`;
    r.onclick = () => { selected = m.name; renderList(); renderDetail(); };
    el.appendChild(r);
  };

  DATA.modules.forEach(m => row(m, false));

  if (mode === "after") {
    const d = document.createElement("div");
    d.className = "divider";
    el.appendChild(d);
    sec("依赖层", DATA.libCount, "被上面那些模块依赖的库，零端点；只能看，不能调。");
    DATA.libs.forEach(m => row(m, true));

    if (DATA.unlisted.length) {
      const d2 = document.createElement("div");
      d2.className = "divider";
      el.appendChild(d2);
      sec("未登记", DATA.unlisted.length,
          "真能返回 200，但不在 capabilities 也不在 /v1/_routes —— 插件自带了端点却没进清单。");
      DATA.unlisted.forEach(m => row(m, false));
    }
  }
}

function chip(name, clickable) {
  const known = !!byName[name];
  const invisible = mode === "before" && known && byName[name].endpoints === 0;
  const c = document.createElement("span");
  c.className = "chip" + (invisible ? " invisible" : "");
  c.textContent = name;
  if (clickable) {
    c.onclick = () => {
      if (invisible) {
        // 改前：图上能看见，点进去什么都没有
        selected = name;
        renderList(); renderDetail(true);
      } else {
        selected = name; renderList(); renderDetail();
      }
    };
  }
  return c;
}

function renderDetail(deadEnd) {
  const el = document.getElementById("detail");
  const m = byName[selected];
  if (!m) { el.innerHTML = '<p class="empty">没有这个模块。</p>'; return; }
  const isLib = m.endpoints === 0;
  const deps = DATA.deps[m.name] || [];
  const inn = DATA.dependents[m.name] || [];

  let html = `<div class="dh"><span class="n">${m.name}</span>
    <span class="pill">${m.vendor}</span>
    <span class="pill v">v${m.version}</span>
    <span class="pill">${m.layer}</span>
    ${m.endpoint_unlisted ? '<span class="pill warn">1 端点 · 未登记</span>'
      : isLib ? '<span class="pill warn">依赖层 · 无端点</span>'
              : `<span class="pill ok">${m.endpoints} 端点 · 启用</span>`}</div>
    <div class="dsub">${m.desc || ""}</div>`;

  html += `<div class="meta">
    <div><div class="k">来源 crate</div><div class="v mono">${m.crate}</div></div>
    <div><div class="k">层</div><div class="v">${m.layer}</div></div>
    <div><div class="k">版本</div><div class="v mono">v${m.version}</div></div>
    <div><div class="k">${isLib ? "端点数" : "端点数"}</div>
      <div class="v">${isLib ? "—（库，不提供端点）" : m.endpoints + " 条"}</div></div>
  </div>`;

  if (m.endpoint_unlisted) {
    html += `<div class="voidbox" style="border-color:#4a3a18;background:var(--warn-soft)">
      <div class="t" style="color:var(--warn)">它有端点，但引擎没登记</div>
      <p><span class="mono">GET /v1/modules/graph</span> 在这台引擎上返回 200，可
      <span class="mono">/v1/capabilities</span> 不列它、<span class="mono">/v1/_routes</span> 的 165 条里也没有它。
      客户端只能靠「关系图里出现过」推断它存在。这是清单漏项，跟本票的显示面改造是两件事，
      但顺手一起修最省事。</p>
    </div>`;
  }

  if (deadEnd && isLib && mode === "before") {
    html += `<div class="voidbox">
      <div class="t">点进来是空的 —— 这就是现在的问题</div>
      <p>关系图（<span class="mono">/v1/modules/graph</span>）里 ${m.name} 是个正常节点，
      所以胶囊画得出来、也画得可点。但插件页的列表和详情只认 <span class="mono">capabilities.modules</span>
      那 15 个，${m.name} 不在里面 —— 于是跳过来没有任何东西可渲染。</p>
    </div>`;
  }

  html += `<div class="rel"><h3>它依赖谁（${deps.length}）</h3><div class="chips" id="c1"></div></div>`;
  html += `<div class="rel"><h3>谁依赖它（${inn.length}）</h3><div class="chips" id="c2"></div></div>`;

  if (isLib) {
    html += `<div class="note"><b>库没有「读文档」入口</b>：
      现在文档是每个插件 crate 的 README，随产物落到 <span class="mono">docs/plugins/&lt;模块&gt;.md</span>。
      依赖层要不要也出文档，是这张票之外的另一个决定。</div>`;
  }

  el.innerHTML = html;
  const c1 = document.getElementById("c1");
  if (!deps.length) c1.innerHTML = '<span class="empty">没声明依赖</span>';
  else deps.forEach(d => c1.appendChild(chip(d, true)));
  const c2 = document.getElementById("c2");
  if (!inn.length) c2.innerHTML = '<span class="empty">没人依赖它</span>';
  else inn.forEach(d => c2.appendChild(chip(d, true)));
}

function syncTabs() {
  document.querySelectorAll(".tab").forEach(x => {
    x.classList.toggle("on", x.dataset.mode === mode);
  });
}
document.querySelectorAll(".tab").forEach(b => {
  b.onclick = () => {
    mode = b.dataset.mode;
    syncTabs();
    if (mode === "before" && byName[selected] && byName[selected].endpoints === 0) {
      selected = "sessions";
    }
    renderList(); renderDetail();
  };
});

document.getElementById("n-all").textContent = DATA.graphNodes;
document.getElementById("n-mod").textContent = DATA.moduleCount;
document.getElementById("n-lib").textContent = DATA.libCount;
if (mode === "after") selected = "durable";
syncTabs();
renderList(); renderDetail();
</script>
</body></html>
"""

out = HTML.replace("__DATA__", json.dumps(payload, ensure_ascii=False, indent=1))
(HERE / "preview.html").write_text(out, encoding="utf-8")
(HERE / "data.json").write_text(json.dumps(payload, ensure_ascii=False, indent=1), encoding="utf-8")
print(f"written: {HERE/'preview.html'}  ({len(out)//1024} KB)")
print(f"modules={len(modules)} unlisted={len(unlisted)} libs={len(libs)}")
