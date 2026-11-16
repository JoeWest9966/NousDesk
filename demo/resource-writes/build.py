#!/usr/bin/env python3
"""03 骨架 + 四条落地决定 的 demo。

底子是资源页 03（一个图标，两段：内容 / 扩展），从 `resources.rs` 的只读骨架照抄。
新增的是用户采纳的四条：
  1. 新建三张表各说各的必填项，右边实时预览落盘内容
  2. 建完停在「未发布」并选中新条目；提示词建完进编辑器
  3. 发布三个仓各走各的，不做一键全发
  4. 编辑是整页工作台，不是右栏抽屉

CSS 方言 / 字体 / 图标 / 真数据全部沿用 resource-page 的冻结素材。
"""
import base64
import pathlib
import re

here = pathlib.Path(__file__).resolve().parent
rp = here / "vendor"
src = (rp / "base.html").read_text(encoding="utf-8").split("\n")
assert len(src) == 1446, f"base.html 行数变了：{len(src)}"

CSS_BASE = "\n".join(src[7:388])   # <style> 之后到 </style> 之前（不含 </style>）
SPRITE = "\n".join(src[392:423])   # svg symbols
DATA = (here / "data.js").read_text(encoding="utf-8")

CSS_EXTRA = r"""
/* ── 03 外壳 ─────────────────────────────────────────────── */
.wrap { display:flex; height:100vh; background:#0a0b0c; color:var(--ink); }
.stage { flex:1; display:grid; place-items:center; overflow:hidden;
  background:radial-gradient(1100px 700px at 55% 42%, #15181a 0%, #0a0b0c 72%); }
.scaler { width:1280px; height:800px; transform-origin:top left; }
.side { width:270px; flex:0 0 270px; border-left:1px solid #202428; background:#111315;
  overflow-y:auto; padding:14px 13px; scrollbar-width:none; }
.side::-webkit-scrollbar { width:0; }
.side h2 { font-size:12px; margin:0 0 8px; }
.side h3 { font-size:10.5px; color:#e7e9ea; margin:0 0 6px; }
.side .lead { font-size:9.5px; color:#5f676d; letter-spacing:.06em; text-transform:uppercase; margin:12px 0 6px; }
.side .lead:first-child { margin-top:0; }
.side p, .side li { font-size:10px; line-height:1.75; color:#98a0a6; margin:0 0 6px; }
.side b { color:#e7e9ea; font-weight:500; }
.side code { font-family:JBMono,monospace; font-size:9.5px; color:#c8d0d6; }
.side .shot { border:1px solid #262a2e; border-radius:6px; padding:9px 10px; margin-bottom:8px; cursor:pointer; }
.side .shot:hover { border-color:#4c545a; }
.side .shot.on { border-color:#7ec8ff; }
.side .shot .n { font-family:JBMono,monospace; color:#7ec8ff; font-size:9.5px; }
.side .shot .t { font-size:10.5px; color:#e7e9ea; margin-top:3px; }
.topbar { height:46px; flex:0 0 46px; border-bottom:1px solid #202428; display:flex; align-items:center;
  padding:0 16px; gap:12px; }
.topbar h1 { font-size:13px; font-weight:500; }
.topbar .sub { font-size:10.5px; color:#8a9298; }
.topbar .tabs { display:flex; gap:4px; margin-left:auto; }
.topbar .tabs button { height:25px; padding:0 10px; border:1px solid #2a2f33; border-radius:5px;
  background:transparent; color:#8a9298; font:inherit; font-size:10.5px; cursor:pointer; }
.topbar .tabs button.on { background:#1b1f22; color:#fff; }

/* 03 的两段 + 子开关（从 resources.rs 的 render_tree 照抄的形状） */
.segs03 { display:flex; gap:2px; margin:9px 8px 7px; padding:3px; border-radius:9px; background:var(--line); }
.segs03 button { flex:1; height:26px; border:0; border-radius:7px; background:transparent;
  font:inherit; font-size:10.5px; color:var(--muted); cursor:pointer; }
.segs03 button.on { background:var(--panel); color:var(--ink); }
.sub03 { display:flex; gap:4px; margin:-3px 8px 7px; }
.sub03 button { flex:1; height:21px; border:1px solid var(--line); background:transparent; border-radius:5px;
  font:inherit; font-size:10px; color:var(--muted); cursor:pointer; }
.sub03 button.on { background:var(--accent-soft); border-color:transparent; color:var(--accent-fg); }

/* 新建表单（整页工作台中央） */
.wb { display:grid; grid-template-columns:1fr 1fr; gap:14px; padding:18px 20px; overflow-y:auto; align-content:start; }
.formcard { border:1px solid var(--line); border-radius:8px; background:var(--panel); padding:14px 16px; }
.formcard h3 { font-size:12px; font-weight:500; margin:0 0 4px; }
.formcard .hint { font-size:10px; color:var(--muted); line-height:1.7; margin-bottom:12px; }
.field { margin-bottom:11px; }
.field label { display:block; font-size:10.5px; color:var(--ink2); margin-bottom:4px; }
.field label .req { color:#f7768e; }
.field input, .field textarea { width:100%; box-sizing:border-box; border:1px solid var(--line); border-radius:6px;
  background:var(--bg); color:var(--ink); font:inherit; font-size:11px; padding:7px 9px; outline:none; resize:none; }
.field input:focus, .field textarea:focus { border-color:var(--accent); }
.field textarea { min-height:56px; line-height:1.6; }
.field .err { font-size:10px; color:#f7768e; margin-top:3px; min-height:12px; }
.field .tip { font-size:9.5px; color:var(--faint); margin-top:3px; }
.previewcard { border:1px solid var(--line); border-radius:8px; background:var(--bg); padding:12px 14px; }
.previewcard .cap { font-size:10px; color:var(--muted); letter-spacing:.04em; margin-bottom:8px; }
.previewcard pre { font-family:JBMono,monospace; font-size:10px; line-height:1.75; color:#c8d0d6;
  white-space:pre-wrap; word-break:break-all; margin:0; }
.previewcard .path { font-family:JBMono,monospace; font-size:9.5px; color:#7ec8ff; margin-bottom:7px; }
.wbfoot { display:flex; gap:8px; align-items:center; padding:0 20px 16px; }
.wbfoot .st { font-size:10.5px; color:var(--muted); margin-right:auto; }
.wbfoot .st b { color:#ffb454; }
.btn2 { border:1px solid var(--line); background:var(--panel); color:var(--ink); border-radius:6px;
  height:28px; padding:0 13px; font:inherit; font-size:11px; cursor:pointer; }
.btn2.pri { background:var(--accent); color:#0a0b0c; border-color:transparent; font-weight:600; }
.btn2.dis { opacity:.45; cursor:not-allowed; }

/* 未发布态 / 发布面板 */
.unpub { display:inline-flex; align-items:center; gap:4px; padding:2px 7px; border-radius:5px;
  background:rgba(255,180,84,.12); color:#ffb454; font-size:10px; }
.pubpanel { display:flex; flex-direction:column; gap:8px; }
.pubrow { display:flex; align-items:center; gap:10px; border:1px solid var(--line); border-radius:7px;
  padding:10px 12px; background:var(--panel); }
.pubrow .nm { font-family:JBMono,monospace; font-size:11px; min-width:90px; }
.pubrow .st { font-size:10.5px; color:var(--muted); flex:1; }
.pubrow .st b { color:#ffb454; }
.pubrow button { height:26px; padding:0 11px; border:1px solid var(--line); background:transparent;
  color:var(--ink); border-radius:5px; font:inherit; font-size:10.5px; cursor:pointer; }
.pubrow button:hover { border-color:var(--accent); color:var(--accent-fg); }
.pubnote { font-size:10px; color:var(--muted); line-height:1.7; border-left:2px solid var(--accent);
  padding-left:9px; margin:4px 0 0; }
"""
here_ts = None  # keep script linear

HTML_HEAD = """<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>NousDesk · 资源页 03 · 写操作落地稿</title>
<style>
""" + CSS_BASE + "\n" + CSS_EXTRA + "\n</style>\n</head>\n<body>\n" + SPRITE + "\n"

SHELL = """
<div class="wrap">
  <div>
    <div class="topbar"><h1>资源页 · 写操作落地稿</h1>
      <span class="sub">03 骨架（内容 / 扩展两段）+ 你采纳的四条 · 1280×800 · 真数据</span></div>
    <div class="stage"><div class="scaler" id="scaler"><div class="app" id="app" data-skin="default"></div></div></div>
  </div>
  <aside class="side" id="side"></aside>
</div>
"""

JS = r"""
const V = () => ({ rail:'one2' });
const esc = s => String(s ?? '').replace(/[&<>"]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const ic = (n,cls='') => `<svg class="ic ${cls}"><use href="#i-${n}"/></svg>`;
const kb = n => n==null?'—':n<1024?`${n} B`:`${(n/1024).toFixed(1)} KB`;

const S = { shot:'new-skill', section:'content', content:'skills',
  form:{
    skill:{name:'weekly-digest',description:'把这几天的会议、邮件、聊天整理成一份周报',category:'写作, 效率'},
    prompt:{name:'strict-review',summary:'审查时按标准 + 规格两条轴走，不混成一股感觉',category:'工程'},
    ext:{name:'my-tools',summary:'给会话加几个顺手的工具入口'}
  } };

/* ── 九个镜头：4 个是新增的写操作屏，5 个沿用只读骨架 ──── */
const SHOTS = [
  ['new-skill','新建 · 技能','三张表各说各的必填：技能要 description'],
  ['new-prompt','新建 · 提示词','必填 summary；建完直接进编辑器'],
  ['new-ext','新建 · 扩展','必填 name + 说明 ≤ 200；核心三条不让用'],
  ['created','建完 · 未发布','新条目停在未发布并选中；不自动发布'],
  ['edit','编辑 · 整页工作台','整页画布；左边文件树，右边正文，不是抽屉'],
  ['publish','发布 · 三个仓各走各','技能 / 提示词 / 扩展各一个按钮'],
  ['readonly-skill','只读 · 技能','03 骨架：内容 → 技能'],
  ['readonly-prompt','只读 · 提示词','内容 → 提示词，共用一套列表/详情'],
  ['readonly-ext','只读 · 扩展','扩展段：catalog / builtin / variants 五类'],
];
const shotT = () => SHOTS.find(s=>s[0]===S.shot)[1];

/* ── 校验：照引擎原话，不泛化 ─────────────────────────── */
const NAME_RE = /^[a-z0-9][a-z0-9._-]*$/;
const CORE = ['nous','skills','prompts'];
function validate(kind,f){
  const e={};
  if(!f.name.trim()) e.name='名字不能空';
  else if(f.name.length>64) e.name='名字最多 64 字';
  else if(!NAME_RE.test(f.name)) e.name='只能用小写字母、数字、点、下划线、连字符，且以字母数字开头';
  if(kind==='skill'&&!f.description.trim()) e.description='description 必填';
  if(kind==='prompt'&&!f.summary.trim()) e.summary='summary 必填';
  if(kind==='ext'){
    if(CORE.includes(f.name)) e.name=`${f.name} 是核心扩展，不能新建`;
    if(f.summary.length>200) e.summary='说明最多 200 字';
  }
  return e;
}
function setForm(kind,key,val){ S.form[kind][key]=val; render(); }

/* ── 落盘预览：三张表各写各的文件 ─────────────────────── */
function previewOf(kind,f){
  const escJson = v => JSON.stringify(v,null,2);
  if(kind==='skill'){
    const cats=f.category.trim()?f.category.split(/[,，\s]+/).filter(Boolean):[];
    return {path:'/opt/Nous-skills/'+ (f.name||'<name>') +'/nous.json',
      text:escJson({name:f.name||'<name>',description:f.description||'<description>',category:cats})};
  }
  if(kind==='prompt'){
    const cats=f.category.trim()?f.category.split(/[,，\s]+/).filter(Boolean):[];
    return {path:'/opt/Nous-prompts/'+ (f.name||'<name>') +'/nous.json',
      text:'---\n'+cats.map(c=>'  - '+c).join('\n')+(cats.length?'\n':'')+'---\n\n# '+(f.name||'<name>')+'\n\n'+(f.summary||'<summary>')};
  }
  return {path:'/opt/Nous-extend/extensions/'+ (f.name||'<name>') +'/nous.json',
    text:escJson({name:f.name||'<name>',description:f.summary||'<summary>'})};
}

/* ── 左侧 rail ─────────────────────────────────────────── */
function rail(){
  const items=[['chats','chat','聊天'],['templates','book','模板'],['triggers','bolt','触发器'],['res','layers','资源'],['data','db','数据']];
  return `<aside class="rail">${items.map(([id,n,t])=>`<button class="rbtn ${id==='res'?'on':''}" title="${t}">${ic(n)}</button>`).join('')}
    <div class="rsp"></div><button class="rbtn">${ic('menu')}</button></aside>`;
}
function segs03(){
  const onContent=S.section==='content';
  return `<div class="segs03"><button class="${onContent?'on':''}" data-act="sec:content">内容 ${D.skills.items.length+D.prompts.items.length}</button>
    <button class="${!onContent?'on':''}" data-act="sec:ext">扩展 ${D.exts.catalog.length}</button></div>`+
    (onContent?`<div class="sub03"><button class="${S.content==='skills'?'on':''}" data-act="con:skills">技能</button>
    <button class="${S.content==='prompts'?'on':''}" data-act="con:prompts">提示词</button></div>`:'');
}

/* ── 各屏 ─────────────────────────────────────────────── */
function tree03(){
  const counts={skills:D.skills.items.length,prompts:D.prompts.items.length,ext:D.exts.catalog.length};
  const cats=S.content==='skills'?D.skills.categories:S.content==='prompts'?D.prompts.categories:D.exts.categories;
  return `<aside class="tree">${segs03()}<div class="tree-body scroll">
    <div class="cat on"><span class="nm">全部</span><span class="n">${counts[S.content]}</span></div>
    ${cats.map(c=>`<div class="cat"><span class="nm">${esc(c)}</span><span class="n">${Math.floor(counts[S.content]/2)}</span></div>`).join('')}
    </div><div class="tree-foot"><div class="frow"><span class="dot warn"></span><span>有未提交的改动</span></div></div></aside>`;
}
function list03(){
  const kind=S.content==='skills'?'skills':S.content==='prompts'?'prompts':'ext';
  const rows=kind==='skills'?D.skills.items:kind==='prompts'?D.prompts.items:D.exts.catalog;
  const nm=r=>r.name;
  return `<div class="list"><div class="list-head"><div class="list-title"><h3>${kind==='skills'?'技能':kind==='prompts'?'提示词':'扩展'}</h3>
    <span class="n">${rows.length} 条</span><span class="acts"><button class="btn sm pri" data-act="new:${kind}">${ic('plus')}新建</button></span></div></div>
    <div class="list-body scroll">${rows.map(r=>`<div class="row on" data-act="sel:${esc(nm(r))}">
      <div class="av">${esc(nm(r)[0].toUpperCase())}</div><div class="main"><div class="nm"><span class="t mono">${esc(nm(r))}</span></div>
      <div class="sum">${esc(r.description||r.summary||'')}</div></div></div>`).join('')}</div></div>`;
}
function detail03(){
  return `<div class="detail"><div class="det-head"><div class="top"><h2 class="mono">code-review</h2><span class="chip accent">已发布</span></div>
    <div class="sub"><span class="mono">skill:code-review</span><span>·</span><span>工程</span></div></div>
    <div class="det-body scroll"><div class="det-inner"><dl class="metas"><dt>摘要</dt><dd>审查从某个点以来的改动；标准 + 规格两条轴</dd></dl>
    <div class="sec-title">文件</div><div class="files"><div class="file on">${ic('file')}<span class="p">SKILL.md</span><span class="sz">413 B</span></div></div>
    <div class="sec-title">SKILL.md</div><div class="prose"><p>有人让你 review 一个分支、一个 PR、或者「看看我这几天改的」。</p></div>
    </div></div></div>`;
}
function readonlyPage(){
  return rail()+`<div class="page">${tree03()}${list03()}${detail03()}</div>`;
}

const CFG={skill:{label:'技能',dir:'Nous-skills',req:'description'},prompt:{label:'提示词',dir:'Nous-prompts',req:'summary'},ext:{label:'扩展',dir:'Nous-extend',req:'说明'}};
function newForm(kind){
  const f=S.form[kind], err=validate(kind,f), pv=previewOf(kind,f), good=Object.keys(err).length===0;
  const catField=kind!=='ext'?`<div class="field"><label>类别 <span class="tip" style="display:inline">（逗号分隔，可空）</span></label>
    <input data-k="${kind}" data-k="category" value="${esc(f.category)}" placeholder="工程, 写作"></div>`:'';
  return rail()+`<div class="page"><div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
    <div class="det-head"><div class="top"><h2>新建${CFG[kind].label}</h2><span class="chip line" style="margin-left:auto">建完停在「未发布」</span></div>
    <div class="sub">必填项各表不同；右边是<strong>马上要落盘的内容</strong>，边填边看。</div></div>
    <div class="wb"><div class="formcard"><h3>这条${CFG[kind].label}是什么</h3>
      <div class="hint">名字规则照引擎原话：<code>^[a-z0-9][a-z0-9._-]*$</code>，≤ 64 字。</div>
      <div class="field"><label>名字 <span class="req">*</span></label>
        <input data-k="${kind}" data-f="name" value="${esc(f.name)}" placeholder="my-${kind}">
        <div class="err">${err.name||''}</div></div>
      ${kind==='skill'?`<div class="field"><label>description <span class="req">*</span></label>
        <textarea data-k="${kind}" data-f="description" placeholder="一句话说清这条技能干什么">${esc(f.description)}</textarea>
        <div class="err">${err.description||''}</div></div>`:''}
      ${kind==='prompt'?`<div class="field"><label>summary <span class="req">*</span></label>
        <textarea data-k="${kind}" data-f="summary" placeholder="一句话说清这条提示词往哪个方向纠">${esc(f.summary)}</textarea>
        <div class="err">${err.summary||''}</div></div>`:''}
      ${kind==='ext'?`<div class="field"><label>说明 <span class="req">*</span></label>
        <textarea data-k="${kind}" data-f="summary" placeholder="这条扩展往系统提示词塞什么">${esc(f.summary)}</textarea>
        <div class="err">${err.summary||''}</div><div class="tip">≤ 200 字。核心三条 <code>nous/skills/prompts</code> 不让新建。</div></div>`:''}
      ${catField}</div>
      <div class="previewcard"><div class="cap">落盘预览 · 改一个字这里就跟着变</div>
        <div class="path">${esc(pv.path)}</div><pre>${esc(pv.text)}</pre></div></div>
    <div class="wbfoot"><div class="st">${good?'<b>都填对了</b> · 建完这条会停在未发布并选中':`还差：${Object.values(err).join('、')}`}</div>
      <button class="btn2" data-act="cancel">取消</button>
      <button class="btn2 pri ${good?'':'dis'}" data-act="create:${kind}">建，去编辑</button></div>
    </div></div>`;
}
function created(){
  return rail()+`<div class="page"><div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
    <div class="det-head"><div class="top"><h2 class="mono">weekly-digest</h2><span class="unpub">● 未发布</span></div>
    <div class="sub">新建完停在这一态，<strong>不自动发布</strong>。新条目已选中，直接进编辑器。</div></div>
    <div class="wb" style="grid-template-columns:1fr 1fr">
      <div class="formcard"><h3>刚建好的这条</h3><div class="hint">新条目已选中并带「未发布」；列表里其它条目是普通态。</div>
        <div class="list" style="border:0"><div class="list-head"><div class="list-title"><h3>技能</h3><span class="n">${D.skills.items.length+1} 条</span></div></div>
        <div class="list-body scroll">${D.skills.items.concat([{name:'weekly-digest',description:'把这几天的会议、邮件、聊天整理成一份周报'}]).map(r=>`<div class="row ${r.name==='weekly-digest'?'on':''}">
          <div class="av">${esc(r.name[0].toUpperCase())}</div><div class="main"><div class="nm"><span class="t mono">${esc(r.name)}</span>${r.name==='weekly-digest'?'<span class="unpub">● 未发布</span>':''}</div>
          <div class="sum">${esc(r.description||'')}</div></div></div>`).join('')}</div></div></div>
      <div class="previewcard"><div class="cap">下一步</div>
        <div style="display:flex;flex-direction:column;gap:9px;margin-top:4px">
          <button class="btn2 pri" data-act="shot:edit">进编辑器写内容 →</button>
          <button class="btn2" data-act="shot:publish">去发布面板（单独发这个仓）</button>
          <div class="pubnote">提示词的「建完进编辑器」是同样的流程 —— 它的正文就是编辑器里的内容；
          技能的 <code>SKILL.md</code>、扩展的清单也在这步写。</div></div></div></div>
    </div></div>`;
}
function edit(){
  return rail()+`<div class="page"><div style="flex:1;display:flex;flex-direction:column;overflow:hidden">
    <div class="det-head"><div class="top"><h2 class="mono">weekly-digest</h2><span class="unpub">● 未发布</span>
      <span class="chip line" style="margin-left:auto">整页工作台 · 不是抽屉</span></div>
      <div class="sub">左边文件树、中间正文、右边状态；<strong>占满主内容区</strong>，不盖在列表上。</div></div>
    <div style="flex:1;display:grid;grid-template-columns:160px 1fr 240px;gap:0;min-height:0">
      <div class="tree" style="border-right:1px solid var(--line)"><div class="tree-body scroll">
        <div class="cat on"><span class="nm">nous.json</span></div><div class="cat on"><span class="nm">SKILL.md</span></div>
        <div class="cat"><span class="nm">+ 新文件</span></div></div></div>
      <div style="padding:14px 18px;overflow-y:auto;background:var(--panel)">
        <div class="cap" style="font-size:10px;color:var(--muted);margin-bottom:8px">SKILL.md</div>
        <textarea style="width:100%;height:340px;box-sizing:border-box;background:var(--bg);color:var(--ink);border:1px solid var(--line);border-radius:6px;padding:12px;font:12px/1.8 JBMono,monospace"># weekly-digest

把这几天的会议、邮件、聊天整理成一份周报。

## 何时用
周五下午，或者有人问「我这周干了啥」。

## 两条规矩
1. 只写做成的事，不写计划。
2. 每条附一个可点开的链接。</textarea></div>
      <div style="border-left:1px solid var(--line);padding:14px 15px;overflow-y:auto;background:var(--bg)">
        <div class="lead" style="font-size:9.5px;color:var(--faint);text-transform:uppercase">状态</div>
        <p style="font-size:10.5px;line-height:1.8;color:var(--muted)">● 未发布<br>改动还没写进仓，发布前不会出现在别人那边。</p>
        <div class="lead" style="font-size:9.5px;color:var(--faint);text-transform:uppercase;margin-top:12px">改动</div>
        <p style="font-size:10.5px;line-height:1.8;color:var(--muted)">SKILL.md · +8 行<br>nous.json · 没动</p>
        <button class="btn2" style="margin-top:10px" data-act="noop">存草稿</button></div></div>
    <div class="wbfoot"><div class="st">改完回来列表里，这条带琥珀脏点（有未提交改动）。</div>
      <button class="btn2" data-act="noop">放弃改动</button><button class="btn2 pri" data-act="noop">保存</button></div>
    </div></div>`;
}
function publish(){
  const rows=[['技能','/opt/Nous-skills','2 处待发布','发布技能'],['提示词','/opt/Nous-prompts','3 处待发布','发布提示词'],['扩展','/opt/Nous-extend','和远端分叉（pull 失败）','发布扩展']];
  return rail()+`<div class="page"><div style="flex:1;display:flex;flex-direction:column;overflow:hidden;padding:18px 20px">
    <h2 style="font-size:14px;font-weight:500;margin:0 0 4px">发布</h2>
    <div class="sub" style="font-size:10.5px;color:var(--muted);margin-bottom:14px">三个仓各走各的，<strong>不做一键全发</strong> —— 谁失败只染谁，别的照发。</div>
    <div class="pubpanel">${rows.map(([label,dir,st,btn])=>`<div class="pubrow">
      <span class="nm">${label}</span><span class="mono" style="font-size:9.5px;color:var(--faint)">${dir}</span>
      <span class="st"><b>●</b> ${st}</span><button data-act="noop">${btn}</button></div>`).join('')}</div>
    <div class="pubnote" style="margin-top:14px">为什么不做一键全发：三个仓是三件独立的事，
      <code>/opt/Nous-extend</code> 当前分叉且 pull 失败 —— 一键发会卡死在扩展上，技能 / 提示词也发不出去。</div>
    </div></div>`;
}

function render(){
  const app=document.getElementById('app');
  const sh=S.shot;
  app.innerHTML= sh==='new-skill'?newForm('skill') : sh==='new-prompt'?newForm('prompt') : sh==='new-ext'?newForm('ext')
    : sh==='created'?created() : sh==='edit'?edit() : sh==='publish'?publish() : readonlyPage();
  if(sh.startsWith('readonly')){
    const [_,kind]=sh.split('-'); S.section=kind==='ext'?'ext':'content'; if(kind!=='ext')S.content=kind;
  }
  renderSide();
}
function renderSide(){
  const side=document.getElementById('side');
  side.innerHTML=`<h2>镜头</h2>${SHOTS.map(([id,t,d])=>`<div class="shot ${S.shot===id?'on':''}" data-act="shot:${id}">
    <div class="n">${id}</div><div class="t">${t}</div></div>`).join('')}
    <div class="lead">你采纳的四条，落在哪</div>
    <p><b>①</b> 新建三张表各说各的必填，右边实时预览落盘内容 → 镜头 <code>new-*</code>。</p>
    <p><b>②</b> 建完停在未发布并选中；提示词建完进编辑器 → 镜头 <code>created</code>。</p>
    <p><b>③</b> 发布三个仓各走各的，不做一键全发 → 镜头 <code>publish</code>。</p>
    <p><b>④</b> 编辑是整页工作台，不是右栏抽屉 → 镜头 <code>edit</code>。</p>
    <div class="lead">03 骨架（跟只读第一刀一致）</div>
    <p>一个「资源」图标，页内两段：内容（技能 / 提示词子开关）+ 扩展。
    技能 / 提示词共用一套树 / 列表 / 详情；扩展五类不硬合成一种。</p>`;
}
document.addEventListener('click',e=>{
  const el=e.target.closest('[data-act]'); if(!el)return;
  const a=el.dataset.act;
  if(a.startsWith('shot:')){ goto(a.slice(5)); return; }
  if(a.startsWith('new:')){ goto('new-'+a.slice(4)); return; }
  if(a.startsWith('sec:')){ S.section=a.slice(4); render(); return; }
  if(a.startsWith('con:')){ S.content=a.slice(4); render(); return; }
  if(a==='create:skill'||a==='create:prompt'||a==='create:ext'){ goto('created'); return; }
  if(a==='cancel'){ goto('readonly-'+S.content); return; }
  if(a==='noop') return;
});
document.addEventListener('input',e=>{
  const el=e.target, k=el.dataset.k, f=el.dataset.f;
  if(k&&f){ S.form[k][f]=el.value; render();
    const again=document.querySelector(`[data-k="${k}"][data-f="${f}"]`);
    if(again){ again.focus(); again.setSelectionRange(again.value.length,again.value.length); } }
});
function goto(id){ if(!SHOTS.some(s=>s[0]===id))return; S.shot=id; location.hash=id; render(); }
window.addEventListener('hashchange',()=>{ const id=location.hash.slice(1); if(SHOTS.some(s=>s[0]===id)){S.shot=id;render();} });
window.addEventListener('DOMContentLoaded',()=>{ const id=location.hash.slice(1); if(SHOTS.some(s=>s[0]===id)){S.shot=id;render();} });

function fit(){
  const st=document.querySelector('.stage'), sc=document.getElementById('scaler');
  const s=Math.max(.4,Math.min(1,(st.clientWidth-28)/1280,(st.clientHeight-28)/800));
  sc.style.width=(1280*s)+'px'; sc.style.height=(800*s)+'px';
  document.getElementById('app').style.transform='scale('+s+')';
}
window.addEventListener('resize',fit);
const initId=location.hash.slice(1); if(SHOTS.some(x=>x[0]===initId))S.shot=initId;
render(); fit();
"""

out = HTML_HEAD + SHELL + "<script>\n" + DATA + "\n</script>\n<script>\n" + JS + "\n</script>\n</body>\n</html>\n"
(here / "index.html").write_text(out, encoding="utf-8")


def font(m):
    b64 = base64.b64encode((rp / "fonts" / m.group(1).split("/")[-1]).read_bytes()).decode()
    return f'url(data:font/woff2;base64,{b64}) format("woff2")'


prev = re.sub(r'url\((fonts/[^)]+)\) format\("woff2"\)', font, out)
prev = prev.replace('<script src="data.js"></script>', "")
left = re.findall(r'(?:src|href)="(?!data:|#)([^"]+)"', prev)
(here / "preview.html").write_text(prev, encoding="utf-8")
print("index.html", len(out.encode()), "bytes")
print("preview.html", len(prev.encode()), "bytes; 外链:", left or "无")
