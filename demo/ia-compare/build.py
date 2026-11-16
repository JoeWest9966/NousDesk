#!/usr/bin/env python3
"""02 vs 03 对照稿：同一窗口、同一份真数据，只换左栏 IA。

样式/图标/数据沿用 resource-page 的冻结素材（base.html + data.js），
只重写 picker / stage / JS。生成 index.html，再 build-preview.py 出单文件。
"""
import pathlib

here = pathlib.Path(__file__).resolve().parent
base = (here / ".." / "resource-page" / "base.html").read_text(encoding="utf-8").split("\n")
assert len(base) == 1446, f"base.html 行数变了：{len(base)}"

# base.html: 1-based 389 = </style>, 390 = </head>, 391 = <body>, 393 = <svg ...>
# 保留 <head> 全部（含 </head>），紧跟我们的 CSS，再开 <body>。
head = "\n".join(base[0:389])          # 到第 389 行 </style>（不含原 </head>）
sprite = "\n".join(base[392:424])      # 第 393..424 行 = svg symbol 仓库

CSS = r"""
<style>
/* 对照稿专用：两边并排 + 差异清单 */
.cmp { flex: 1; height: 100vh; display: flex; flex-direction: column; overflow: hidden; }
.cmp-top { height: 92px; flex: 0 0 92px; border-bottom: 1px solid #202428; padding: 14px 18px 10px; }
.cmp-top h1 { font-size: 14px; font-weight: 600; }
.cmp-top .sub { font-size: 10.5px; color: #8a9298; line-height: 1.7; margin-top: 4px; }
.cmp-top .sub b { color: #e7e9ea; font-weight: 500; }
.switch { display: inline-flex; margin-top: 9px; border: 1px solid #2a2f33; border-radius: 6px; overflow: hidden; }
.switch button { height: 25px; padding: 0 13px; border: 0; background: transparent; color: #8a9298;
  font: inherit; font-size: 11px; cursor: pointer; }
.switch button.on { background: #1b1f22; color: #fff; }
.switch button.on.a { color: #7ec8ff; }
.switch button.on.b { color: #ffb454; }
.cmp-body { flex: 1; display: flex; min-height: 0; }
.cmp-stage { flex: 1; display: grid; place-items: center; overflow: hidden; padding: 18px;
  background: radial-gradient(1100px 700px at 55% 42%, #15181a 0%, #0a0b0c 72%); }
.cmp-side { width: 300px; flex: 0 0 300px; border-left: 1px solid #202428; overflow-y: auto;
  background: #111315; padding: 16px 15px 20px; }
.cmp-side::-webkit-scrollbar { width: 0; }
.cmp-side h3 { font-size: 11.5px; color: #e7e9ea; margin: 0 0 8px; }
.cmp-side .grp { margin-bottom: 16px; }
.cmp-side .lead { font-size: 10px; color: #5f676d; letter-spacing: .06em; text-transform: uppercase; margin-bottom: 6px; }
.vs-row { display: flex; gap: 8px; margin-bottom: 8px; font-size: 10.5px; line-height: 1.7; }
.vs-row .k { flex: 0 0 34px; font-weight: 600; }
.vs-row .k.a { color: #7ec8ff; }
.vs-row .k.b { color: #ffb454; }
.vs-row .t { color: #98a0a6; }
.vs-row .t em { font-style: normal; color: #d6dde2; }
.vs-card { border: 1px solid #22262a; border-radius: 7px; padding: 10px 11px; margin-bottom: 9px; }
.vs-card .hd { font-size: 11px; font-weight: 600; margin-bottom: 5px; }
.vs-card.a .hd { color: #7ec8ff; }
.vs-card.b .hd { color: #ffb454; }
.vs-card p { font-size: 10.5px; line-height: 1.75; color: #98a0a6; margin: 0 0 5px; }
.vs-card p:last-child { margin-bottom: 0; }
.vs-card b { color: #e7e9ea; font-weight: 500; }
.vs-card code { font-family: JBMono, monospace; font-size: 9.5px; color: #c8d0d6; }
.verdict { margin-top: 12px; padding: 11px; border: 1px solid #2a2f33; border-radius: 7px;
  background: #15181a; font-size: 10.5px; line-height: 1.8; color: #9aa2a8; }
.verdict b { color: #e7e9ea; }

/* 段区：02 三段并排 vs 03 两段 + 子开关（差异高亮） */
.segs-cmp { display: flex; gap: 2px; margin: 9px 8px 8px; padding: 3px; border-radius: 9px; background: var(--line); }
.segs-cmp button { flex: 1 1 0; min-width: 0; padding: 4px 2px; border: 0; border-radius: 7px; background: transparent;
  font: inherit; font-size: 10.5px; color: var(--muted); cursor: pointer; display: flex; flex-direction: column;
  align-items: center; gap: 1px; }
.segs-cmp button .n { font-size: 9.5px; color: var(--faint); }
.segs-cmp button.on { background: var(--panel); color: var(--ink); }
.segs-cmp button.on .n { color: var(--ok-fg); }
.subseg-cmp { display: flex; gap: 4px; margin: -4px 8px 8px; }
.subseg-cmp button { flex: 1; height: 21px; border: 1px solid var(--line); background: transparent;
  border-radius: 5px; font: inherit; font-size: 10px; color: var(--muted); cursor: pointer; }
.subseg-cmp button.on { background: var(--accent-soft); border-color: transparent; color: var(--accent-fg); }
.hl { outline: 1.5px dashed var(--accent); outline-offset: 1px; }
</style>
"""

PICKER = """<div class="cmp">
  <div class="cmp-top">
    <h1>资源页 IA · 02 vs 03 对照</h1>
    <div class="sub">同一窗口（1280×800）、同一份真数据，<b>只换左栏「资源」页的分段方式</b>。点上面切，或按 1 / 2。</div>
    <div class="switch"><button id="sw-a" class="on a">02 · 三段并排</button><button id="sw-b" class="b">03 · 两段 + 子开关</button></div>
  </div>
  <div class="cmp-body">
    <div class="cmp-stage"><div class="scaler" id="scaler"><div class="app" id="app" data-skin="default"></div></div></div>
    <aside class="cmp-side" id="side"></aside>
  </div>
</div>
"""

JS = r"""
/* 两种 IA，唯一差别就是这一处分段器 + 它带出来的树。
   数据、列表、详情、样式全都一样 —— 让差别自己说话。 */
const IA = {
  a: {
    name: '02 · 三段并排',
    segs: [['skills', '技能'], ['prompts', '提示词'], ['ext', '扩展']],
    sub: null,
  },
  b: {
    name: '03 · 两段 + 子开关',
    segs: [['content', '内容'], ['ext', '扩展']],
    sub: [['skills', '技能'], ['prompts', '提示词']],
  },
};

const S = { ia: 'a', tab: 'skills', cat: {}, sel: {skills:'code-review', prompts:'smart-ass', ext:'skills'} };
const V = () => ({ rail: 'one3' });
const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const ic = (n, cls='') => `<svg class="ic ${cls}"><use href="#i-${n}"/></svg>`;
const cat = () => S.cat[S.tab] ?? null;
const setCat = (c) => { S.cat[S.tab] = c; render(); };
const selOf = (t=S.tab) => S.sel[t];
function inCat(cats, c){ if(c==='∅')return !cats.length; return cats.some(x=>x===c||x.startsWith(c+'/')); }
function catNodes(list){ return list.map(p=>({path:p,depth:(p.match(/\//g)||[]).length,label:p.split('/').pop()})); }
function countIn(items,c){ return items.filter(i=>inCat(i.category||[],c)).length; }
function kb(n){ return n==null?'—':n<1024?`${n} B`:`${(n/1024).toFixed(1)} KB`; }
const short = h => (h||'—').slice(0,7);

/* ── 数据（沿用 resource-page 的 data.js）────────────────── */
function skillsRows(){
  const c=cat(); let items=D.skills.items.slice();
  if(c==='@variant') return Object.entries(D.skills.variants).flatMap(([fam,vs])=>vs.map(v=>({
    skill:{name:v.key,description:v.load?.error||'变体树',category:[],files:0,hidden:false,tracked:true,variant:v}})));
  if(c==='@hidden') return items.filter(s=>s.hidden).map(s=>({skill:s}));
  items=items.filter(s=>!s.hidden);
  if(c&&c!=='∅') items=items.filter(s=>inCat(s.category,c));
  if(c==='∅') items=items.filter(s=>!s.category.length);
  return items.map(s=>({skill:s}));
}
function promptsRows(){
  const c=cat(); let items=D.prompts.items.slice();
  if(c==='@untracked') items=items.filter(p=>!p.tracked);
  else if(c==='@default') items=items.filter(p=>p.name===D.prompts.default);
  else { if(c&&c!=='∅') items=items.filter(p=>inCat(p.category,c)); if(c==='∅') items=items.filter(p=>!p.category.length); }
  return items.map(p=>({prompt:p}));
}
function extRows(){
  const e=D.exts; const rows=e.catalog.map(c=>{
    const load=e.load[c.name]; let state='on';
    if(load&&load.state==='failed') state='bad';
    if(!e.installed.includes(c.name)) state='off';
    return {ext:c,state,variant:null};
  });
  for(const b of e.builtin) rows.push({ext:{name:b,builtin:true,category:[],sections:[]},state:'builtin'});
  for(const [fam,vs] of Object.entries(e.variants)) for(const v of vs) rows.push({
    ext:{name:v.key,family:fam,version:e.load[v.key]?.version,category:(e.catalog.find(c=>c.name===fam)||{}).category||[]},
    state:e.load[v.key]?.state==='failed'?'bad':'variant',variant:v});
  for(const a of e.archived) rows.push({ext:{...a,archived:true},state:'archived'});
  return rows;
}
function filteredExtRows(){
  const c=cat(); let rows=extRows();
  const map={'@on':'on','@builtin':'builtin','@bad':'bad','@archived':'archived'};
  if(c==='@variant') rows=rows.filter(r=>r.variant!=null);
  else if(c==='@deleted') rows=rows.filter(r=>D.exts.local?.deleted?.[r.ext.name]);
  else if(map[c]) rows=rows.filter(r=>r.state===map[c]);
  else if(c) rows=rows.filter(r=>inCat(r.ext.category||[],c));
  return rows;
}
const rowsFor = () => S.tab==='skills'?skillsRows():S.tab==='prompts'?promptsRows():filteredExtRows();
const nameOf = r => r.skill?.name||r.prompt?.name||r.ext?.name;
function syncSel(){ const rows=rowsFor(); if(!rows.length)return; if(!rows.some(r=>nameOf(r)===selOf())) S.sel[S.tab]=nameOf(rows[0]); }

/* ── 极简 markdown ───────────────────────────────────────── */
function md(text){
  const lines=String(text).split('\n'); let out=[],list=null,fence=null,front=null;
  const closeList=()=>{if(list){out.push(`</${list}>`);list=null;}};
  const inline=s=>esc(s).replace(/`([^`]+)`/g,'<code>$1</code>').replace(/\*\*([^*]+)\*\*/g,'<strong>$1</strong>');
  for(let i=0;i<lines.length;i++){
    const raw=lines[i];
    if(fence!==null){ if(raw.trim().startsWith('```')){out.push(`<pre><code>${esc(fence.join('\n'))}</code></pre>`);fence=null;} else fence.push(raw); continue; }
    if(raw.trim().startsWith('```')){closeList();fence=[];continue;}
    if(i===0&&raw.trim()==='---'){front=[];continue;}
    if(front!==null){ if(raw.trim()==='---'){out.push(`<div class="front">${esc(front.join('\n'))}</div>`);front=null;} else front.push(raw); continue; }
    const h=raw.match(/^(#{1,4})\s+(.*)$/); if(h){closeList();const lv=Math.min(h[1].length,3);out.push(`<h${lv}>${inline(h[2])}</h${lv}>`);continue;}
    const ul=raw.match(/^\s*[-*]\s+(.*)$/); if(ul){if(list!=='ul'){closeList();out.push('<ul>');list='ul';}out.push(`<li>${inline(ul[1])}</li>`);continue;}
    const ol=raw.match(/^\s*\d+\.\s+(.*)$/); if(ol){if(list!=='ol'){closeList();out.push('<ol>');list='ol';}out.push(`<li>${inline(ol[1])}</li>`);continue;}
    if(!raw.trim()){closeList();continue;} closeList(); out.push(`<p>${inline(raw)}</p>`);
  }
  if(fence) out.push(`<pre><code>${esc(fence.join('\n'))}</code></pre>`);
  return out.join('');
}

/* ── 树 ─────────────────────────────────────────────────── */
function catRow(label,count,depth,active,act){
  return `<div class="cat ${active?'on':''} ${count===0?'zero':''}" data-act="${act}" style="padding-left:${8+depth*12}px">
    <span class="nm">${esc(label)}</span><span class="n">${count}</span></div>`;
}
const groupSep=t=>`<div class="gsep">${esc(t)}</div>`;
function skillsTree(){
  const items=D.skills.items.filter(s=>!s.hidden);
  let h=catRow('全部',items.length,0,cat()===null,'cat:');
  for(const n of catNodes(D.skills.categories)) h+=catRow(n.label,countIn(items,n.path),n.depth,cat()===n.path,'cat:'+n.path);
  h+=catRow('未分类',items.filter(s=>!s.category.length).length,0,cat()==='∅','cat:∅');
  h+=groupSep('状态');
  h+=catRow('变体',Object.values(D.skills.variants).flat().length,0,cat()==='@variant','cat:@variant');
  h+=catRow('藏起来的',D.skills.items.filter(s=>s.hidden).length,0,cat()==='@hidden','cat:@hidden');
  return h;
}
function promptsTree(){
  const items=D.prompts.items;
  let h=catRow('全部',items.length,0,cat()===null,'cat:');
  for(const n of catNodes(D.prompts.categories)) h+=catRow(n.label,countIn(items,n.path),n.depth,cat()===n.path,'cat:'+n.path);
  h+=catRow('未分类',items.filter(p=>!p.category.length).length,0,cat()==='∅','cat:∅');
  h+=groupSep('状态');
  h+=catRow('盘上有、仓里没',items.filter(p=>!p.tracked).length,0,cat()==='@untracked','cat:@untracked');
  h+=catRow('引擎默认那条',1,0,cat()==='@default','cat:@default');
  return h;
}
function extTree(){
  const e=D.exts, rows=extRows(), by=fn=>rows.filter(fn).length;
  let h=catRow('全部',rows.length,0,cat()===null,'cat:');
  h+=groupSep('状态');
  h+=catRow('启用中',by(r=>r.state==='on'),0,cat()==='@on','cat:@on');
  h+=catRow('内置',by(r=>r.state==='builtin'),0,cat()==='@builtin','cat:@builtin');
  h+=catRow('加载失败',by(r=>r.state==='bad'),0,cat()==='@bad','cat:@bad');
  h+=catRow('变体',by(r=>r.variant!=null),0,cat()==='@variant','cat:@variant');
  h+=catRow('已归档',by(r=>r.state==='archived'),0,cat()==='@archived','cat:@archived');
  h+=catRow('本机已删',Object.keys(e.local?.deleted||{}).length,0,cat()==='@deleted','cat:@deleted');
  h+=groupSep('类别');
  for(const n of catNodes(e.categories)) h+=catRow(n.label,e.catalog.filter(c=>inCat(c.category||[],n.path)).length,n.depth,cat()===n.path,'cat:'+n.path);
  return h;
}

/* ── 分段器：唯一的差别在这里 ────────────────────────────── */
function segBlock(){
  const cfg=IA[S.ia];
  const segs=`<div class="segs-cmp ${S.ia==='b'?'hl':''}">${cfg.segs.map(([id,label])=>{
    const on=S.ia==='a' ? S.tab===id : (id==='content'?S.tab!=='ext':S.tab==='ext');
    const n=id==='skills'?D.skills.items.filter(s=>!s.hidden).length:id==='prompts'?D.prompts.items.length:id==='ext'?extRows().length:D.skills.items.length+D.prompts.items.length;
    return `<button data-act="seg:${id}" class="${on?'on':''}">${label}<span class="n">${n}</span></button>`;
  }).join('')}</div>`;
  let sub='';
  if(S.ia==='b' && S.tab!=='ext'){
    sub=`<div class="subseg-cmp hl">${[['skills','技能'],['prompts','提示词']].map(([id,label])=>
      `<button data-act="seg:${id}" class="${S.tab===id?'on':''}">${label}</button>`).join('')}</div>`;
  }
  return segs+sub;
}
function tabTitle(){
  if(S.ia==='a') return {skills:'技能',prompts:'提示词',ext:'扩展'}[S.tab];
  return S.tab==='ext'?'扩展':'内容';
}

/* ── 树 / 列表 / 详情 ───────────────────────────────────── */
function TAB_META_DEF(){ return {skills:{title:'技能',icon:'spark'},prompts:{title:'提示词',icon:'quote'},ext:{title:'扩展',icon:'puzzle'}}; }
function treeHTML(){
  const meta=TAB_META_DEF()[S.tab];
  const icon=S.ia==='a'?meta.icon:'layers';
  const tree=S.tab==='skills'?skillsTree():S.tab==='prompts'?promptsTree():extTree();
  const kind=S.tab==='ext'?'ext':S.tab;
  const r=kind==='ext'?{dot:'warn',text:'和远端分叉',mono:short(D.exts.repo.head)}:
    D[kind].repo.dirty?{dot:'warn',text:'有未提交的改动',mono:short(D[kind].repo.head)}:{dot:'ok',text:'仓干净',mono:short(D[kind].repo.head)};
  const pend=kind==='ext'?[]:D[kind].pending;
  return `<aside class="tree">${segBlock()}
    <div class="tree-body scroll">${tree}</div>
    <div class="tree-foot">
      <div class="frow"><span class="dot ${r.dot}"></span><span>${r.text}</span><span class="head" style="margin-left:auto">${r.mono}</span></div>
      ${pend.length?`<div class="frow" style="margin-top:6px"><button class="linkbtn" data-act="noop">${pend.length} 处待发布 ›</button></div>`:''}
      ${kind==='ext'?`<div class="frow" style="margin-top:5px"><span class="mono" style="font-size:9.5px">pull 失败 · 没有上游分支</span></div>`:''}
    </div></aside>`;
}
const STATE_CHIP={on:'<span class="chip accent">启用中</span>',bad:'<span class="chip bad">加载失败</span>',
  builtin:'<span class="chip line">内置</span>',variant:'<span class="chip line">变体</span>',
  archived:'<span class="chip line">已归档</span>',off:'<span class="chip line">本机没装</span>'};
function rowHTML(kind,obj,on){
  if(kind==='skills'){const s=obj.skill||obj;return `<div class="row ${on?'on':''}" data-act="sel:${esc(s.name)}">
    <div class="av ${s.hidden||s.variant?'off':''}">${esc(s.name[0].toUpperCase())}</div><div class="main">
    <div class="nm"><span class="t mono">${esc(s.name)}</span>${s.variant?'<span class="chip bad">加载失败</span>':''}${s.hidden?'<span class="chip line">藏起来</span>':''}${!s.tracked?'<span class="chip warn">盘上有、仓里没</span>':''}</div>
    <div class="sum">${esc(s.description)}</div><div class="meta">${s.category.length?s.category.slice(0,2).map(c=>`<span class="chip">${esc(c)}</span>`).join(''):'<span class="chip">未分类</span>'}<span class="chip mono">${s.variant?esc(s.variant.branch):s.files+' 文件'}</span></div></div></div>`;}
  if(kind==='prompts'){const p=obj.prompt||obj;return `<div class="row ${on?'on':''}" data-act="sel:${esc(p.name)}">
    <div class="av">${esc(p.name[0].toUpperCase())}</div><div class="main">
    <div class="nm"><span class="t mono">${esc(p.name)}</span>${p.name===D.prompts.default?'<span class="chip accent">默认</span>':''}${!p.tracked?'<span class="chip warn">盘上有、仓里没</span>':''}</div>
    <div class="sum">${esc(p.summary||'')}</div><div class="meta">${p.category.length?p.category.slice(0,2).map(c=>`<span class="chip">${esc(c)}</span>`).join(''):'<span class="chip">未分类</span>'}<span class="chip mono">${kb(p.bytes)}</span></div></div></div>`;}
  const {ext,state,variant}=obj; return `<div class="row ${on?'on':''}" data-act="sel:${esc(ext.name)}">
    <div class="av ${state==='bad'?'off':''}">${esc(ext.name[0].toUpperCase())}</div><div class="main">
    <div class="nm"><span class="t mono">${esc(ext.name)}</span>${ext.version?`<span class="chip mono">v${esc(ext.version)}</span>`:''}${STATE_CHIP[state]}</div>
    <div class="sum">${esc(ext.summary||(ext.builtin?'引擎自带，不在仓库里':'（没有摘要）'))}</div>
    <div class="meta">${(ext.category||[])[0]?`<span class="chip">${esc(ext.category[0])}</span>`:'<span class="chip">未分类</span>'}${(ext.sections||[]).length?`<span class="chip mono">${esc(ext.sections.join(' · '))}</span>`:''}</div></div></div>`;
}
function listHTML(){
  const rows=rowsFor(); const title=S.ia==='a'?tabTitle():(S.tab==='ext'?'扩展':tabTitle());
  const body=rows.length?rows.map(r=>rowHTML(S.tab,r,selOf()===nameOf(r))).join(''):'<div class="note">这一类是空的</div>';
  return `<div class="list"><div class="list-head"><div class="list-title"><h3>${esc(title)}</h3><span class="n">${rows.length} 条</span>
    <span class="acts"><button class="btn sm pri" data-act="noop">${ic('plus')}新建</button></span></div></div>
    <div class="list-body scroll">${body}</div></div>`;
}
function detailHTML(){
  const name=selOf();
  if(S.tab==='skills'){
    const s=D.skills.items.find(x=>x.name===name); if(!s)return emptyDetail();
    const files=D.skills.files[name]||[{path:'nous.json',size:null},{path:'SKILL.md',size:null}];
    const open='SKILL.md'; const body=D.skills.text[`${name}/${open}`];
    return `<div class="detail"><div class="det-head"><div class="top"><h2 class="mono">${esc(name)}</h2>
      ${!s.tracked?'<span class="chip warn">盘上有、仓里没</span>':'<span class="chip accent">已发布</span>'}</div>
      <div class="sub"><span class="mono">skill:${esc(name)}</span><span>·</span><span>${s.category.join(' · ')||'未分类'}</span></div></div>
      <div class="det-body scroll"><div class="det-inner"><dl class="metas"><dt>摘要</dt><dd>${esc(s.description)}</dd></dl>
      <div class="sec-title">文件</div><div class="files">${files.map(f=>`<div class="file ${f.path===open?'on':''}">${ic('file')}<span class="p">${esc(f.path)}</span><span class="sz">${kb(f.size)}</span></div>`).join('')}</div>
      <div class="sec-title">${open}</div>${body?`<div class="prose">${md(body)}</div>`:'<div class="note">（demo 只内嵌了两条正文）</div>'}</div></div></div>`;
  }
  if(S.tab==='prompts'){
    const p=D.prompts.items.find(x=>x.name===name); if(!p)return emptyDetail();
    return `<div class="detail"><div class="det-head"><div class="top"><h2 class="mono">${esc(name)}</h2>
      ${p.name===D.prompts.default?'<span class="chip accent">引擎默认</span>':''}${!p.tracked?'<span class="chip warn">盘上有、仓里没</span>':''}</div>
      <div class="sub"><span class="mono">prompt:${esc(name)}</span><span>·</span><span>${kb(p.bytes)}</span></div></div>
      <div class="det-body scroll"><div class="det-inner"><div class="banner">${ic('spark')}<div><b>单值，不是集合。</b>一条会话只带一条 —— 所以列表里是单选形状。</div></div>
      <dl class="metas"><dt>摘要</dt><dd>${esc(p.summary||'')}</dd><dt>进提示词</dt><dd>正文整段进系统提示词</dd></dl>
      <div class="sec-title">PROMPT.md</div><div class="prose">${md(D.prompts.text[`${name}/PROMPT.md`]||'# '+name)}</div></div></div></div>`;
  }
  const row=extRows().find(r=>r.ext.name===name); if(!row)return emptyDetail();
  const {ext,state}=row; const core=D.exts.defaults.includes(name);
  return `<div class="detail"><div class="det-head"><div class="top"><h2 class="mono">${esc(name)}</h2>${STATE_CHIP[state]}${core?'<span class="chip line">核心</span>':''}</div>
    <div class="sub"><span class="mono">${esc(D.exts.root)}/${esc(name)}</span></div></div>
    <div class="det-body scroll"><div class="det-inner">
    ${core?`<div class="banner">${ic('lock')}<div><b>核心三条之一。</b>卸载 / 归档会被引擎 <code>403</code> 挡回来。</div></div>`:''}
    <dl class="metas"><dt>干什么</dt><dd>${esc(ext.summary||'')}</dd>
    ${(ext.sections||[]).length?`<dt>清单说</dt><dd>往系统提示词塞 <span class="mono">${esc(ext.sections.join(', '))}</span> 段</dd>`:''}
    ${D.exts.live[name]?`<dt>进程里</dt><dd>真装了 <span class="mono">${esc(D.exts.live[name].join(', '))}</span> 段 —— 对得上</dd>`:''}</dl>
    <div class="sec-title">文件</div><div class="files">${(D.exts.files[name]||[]).map(f=>`<div class="file">${ic('code')}<span class="p">${esc(f.path)}</span><span class="sz">${kb(f.size)}</span></div>`).join('')}</div>
    </div></div></div>`;
}
function emptyDetail(){return `<div class="detail"><div class="det-body"><div class="note">左边挑一条。</div></div></div>`;}

/* ── 图标条 ─────────────────────────────────────────────── */
function railHTML(){
  const items=[['chats','chat','聊天'],['templates','book','模板'],['triggers','bolt','触发器'],['res','layers','资源'],['data','db','数据']];
  return `<aside class="rail">${items.map(([id,icn,t])=>`<button class="rbtn ${id==='res'?'on':''}" title="${t}">${ic(icn)}</button>`).join('')}
    <div class="rsp"></div><button class="rbtn">${ic('menu')}</button></aside>`;
}

function render(){
  syncSel();
  const app=document.getElementById('app');
  app.innerHTML=railHTML()+`<div class="page">${treeHTML()}${listHTML()}${detailHTML()}</div>`;
  renderSide();
}
function renderSide(){
  const side=document.getElementById('side');
  const isA=S.ia==='a';
  side.innerHTML=`
    <div class="grp"><div class="lead">当前</div><h3>${isA?'02 · 三段并排':'03 · 两段 + 子开关'}</h3>
      <div style="font-size:10.5px;color:#8a9298;line-height:1.75">
      ${isA?'树顶一个药丸条，<b style="color:#e7e9ea">技能 / 提示词 / 扩展</b> 三段平级，点哪段是哪段。'
           :'树顶两个大段 <b style="color:#e7e9ea">内容 / 扩展</b>；选「内容」后下面再露出一条 <b style="color:#e7e9ea">技能 / 提示词</b> 子开关。'}</div></div>
    <div class="grp"><div class="lead">同一件事，两种走法</div>
      <div class="vs-row"><div class="k a">到技能</div><div class="t">${isA?'点 1 次（直接点「技能」）':'点 2 次（内容 → 技能）'}</div></div>
      <div class="vs-row"><div class="k a">到提示词</div><div class="t">${isA?'点 1 次':'点 2 次'}</div></div>
      <div class="vs-row"><div class="k a">到扩展</div><div class="t">${isA?'点 1 次':'点 1 次'}</div></div>
      <div class="vs-row"><div class="k b">段按钮</div><div class="t">${isA?'3 个平级，各带脏点':'2 个 + 一条子开关'}</div></div>
    </div>
    <div class="vs-card a"><div class="hd">02 的账</div>
      <p><b>好：</b>技能 / 提示词 / 扩展一视同仁，各说各的；脏点各自挂，谁坏了一眼看见。</p>
      <p><b>代价：</b>三个都是「一类资料」，段按钮上看不出「技能和提示词其实是一回事」；<b>多一段</b>就平铺一项，左栏还是一颗图标。</p></div>
    <div class="vs-card b"><div class="hd">03 的账</div>
      <p><b>好：</b>按<b>数据形状</b>分 —— 技能和提示词是同一个模子（后端两个 api 互相点名），共用一套列表 / 详情骨架，代码只写一遍。</p>
      <p><b>代价：</b>「提示词在内容里」要教一次；子开关让两段实际是三档，段按钮 + 子开关在 248px 里排得紧；到技能 / 提示词多点一下。</p></div>
    <div class="verdict"><b>要拍的板：</b>你更在意<b>「三类资料平级、一眼看清谁坏」</b>（选 02），
      还是<b>「按后端形状复用一套骨架、少写一遍代码」</b>（选 03）？<br>
      两条线的数据层完全一样（三个仓的写端点），差别只在 UI 骨架和段切换 —— 定了我就能接接口。</div>`;
}

document.addEventListener('click',(e)=>{
  const el=e.target.closest('[data-act]'); if(!el)return;
  const act=el.dataset.act;
  if(act.startsWith('cat:')){ setCat(act.slice(4)); return; }
  if(act.startsWith('seg:')){ const id=act.slice(4); S.tab=id; S.cat={}; render(); return; }
  if(act.startsWith('sel:')){ S.sel[S.tab]=act.slice(4); render(); return; }
  if(act==='noop') return;
});
function setIA(ia){ S.ia=ia; if(ia==='b'&&S.tab==='ext'){} else if(ia==='a'&&S.tab==='content'){S.tab='skills';}
  document.getElementById('sw-a').classList.toggle('on',ia==='a');
  document.getElementById('sw-b').classList.toggle('on',ia==='b');
  render(); }
document.getElementById('sw-a').addEventListener('click',()=>setIA('a'));
document.getElementById('sw-b').addEventListener('click',()=>setIA('b'));
window.addEventListener('keydown',(e)=>{ if(e.key==='1')setIA('a'); if(e.key==='2')setIA('b'); });
const scaler=document.getElementById('scaler');
function fit(){
  const st=document.querySelector('.cmp-stage');
  const s=Math.max(0.4,Math.min(1,(st.clientWidth-36)/1280,(st.clientHeight-36)/800));
  scaler.style.width=(1280*s)+'px'; scaler.style.height=(800*s)+'px';
  document.getElementById('app').style.transform='scale('+s+')';
}
window.addEventListener('resize',fit);
render(); fit();
"""

head = head.replace("<title>NousDesk · 技能 / 提示词 / 扩展 · 五套</title>",
                    "<title>NousDesk · 资源页 IA 对照 · 02 vs 03</title>")
out = "\n".join([
    head,
    CSS,
    "</head>",
    "<body>",
    sprite,
    PICKER,
    '<script src="data.js"></script>',
    "<script>",
    JS,
    "</script>",
    "</body>",
    "</html>",
])
(here / "index.html").write_text(out, encoding="utf-8")
print("index.html", len(out.encode()), "bytes")
