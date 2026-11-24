import re
src=open('/opt/NousDesk-elements-groups/demo/elements/preview.html').read()
fonts=''.join(re.findall(r'@font-face\{[^}]*\}',src))
html=r'''<!doctype html><html lang=zh><meta charset=utf-8><title>元素库 · 组→包 · mono · 980</title><style>FONTS
*{box-sizing:border-box;margin:0}
body{font-family:Inter,"Noto Sans CJK SC",sans-serif;font-size:12.5px;background:#222;color:#fff}
.bar{position:sticky;top:0;z-index:9;background:#333;padding:8px 14px;display:flex;gap:10px;align-items:center;flex-wrap:wrap;font-size:12px}
.bar button{background:#000;color:#fff;border:1px solid #fff;padding:4px 10px;cursor:pointer;font:inherit}.bar button.on{background:#fff;color:#000}
#desc{color:#d2d2d2;flex-basis:100%}
.stage{padding:20px;display:flex;justify-content:center}
.win{background:#000;height:1168px;display:flex;flex-direction:column;border:1px solid #444}
.title{height:34px;border-bottom:1px solid #fff;display:flex;align-items:center;padding:0 12px;color:#8a8a8a;font-size:11px;gap:8px}
.title .demo{border:1px solid #fff;color:#fff;padding:0 4px;font-size:10px}
.body{flex:1;display:flex;min-height:0}
.rail{width:56px;border-right:1px solid #fff;display:flex;flex-direction:column;align-items:center;padding-top:12px;gap:14px}
.rail i{width:22px;height:22px;border:1px solid #6b6b6b;display:block}.rail i.on{border-color:#fff;background:#1f1f1f}
.col{border-right:1px solid #fff;display:flex;flex-direction:column;flex:none;min-height:0}
.search{margin:12px 12px 6px;border:1px solid #2a2a2a;padding:5px 8px;color:#8a8a8a;font-size:11.5px}
.cap{padding:14px 10px 6px;font-size:10px;color:#8a8a8a;letter-spacing:.06em;display:flex}.cap span{flex:1}
.list{flex:1;overflow:auto;padding:0 8px 16px}
.row{display:flex;gap:8px;align-items:center;padding:6px 10px;cursor:pointer;position:relative}
.row:hover{background:#141414}.row.sel{background:#1f1f1f}.row.sel:before{content:"";position:absolute;left:0;top:6px;bottom:6px;width:2px;background:#fff}
.row .n{flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.row .g{font-size:12.5px}.row .p{font-size:11.5px}
.row small{color:#8a8a8a;font-size:10px}
.tag{font-size:10px;border:1px solid #fff;padding:0 4px}
.bad .n{color:#ff4d4d}.badtag{font-size:10px;color:#ff4d4d}
.foot{border-top:1px solid #2a2a2a;padding:8px 12px;font-size:10px;color:#8a8a8a;font-family:JetBrainsMono,monospace}
.main{flex:1;min-width:0;display:flex;flex-direction:column}
.top{height:54px;border-bottom:1px solid #fff;display:flex;align-items:center;padding:0 16px;gap:10px}
.top b{font-size:15px;font-weight:600}.top .sp{flex:1}.top .k{color:#8a8a8a;font-size:11px}
.btn{border:1px solid #fff;padding:4px 10px;font-size:11.5px;cursor:pointer}.btn.dis{border-color:#5f5f5f;color:#5f5f5f}.btn.pri{background:#fff;color:#000}
.back{cursor:pointer;font-size:12px;padding-right:6px}
.hint{margin:12px 16px 0;border:1px solid #fff;padding:8px 10px;font-size:11.5px}
.form{padding:16px;overflow:auto}
.f{margin-bottom:14px}.f label{display:block;color:#8a8a8a;font-size:10.5px;margin-bottom:5px}
.f .v{border:1px solid #2a2a2a;padding:7px 9px}.f .v.area{height:220px;font-family:JetBrainsMono,monospace;font-size:11.5px;color:#d2d2d2;white-space:pre-wrap}
.chips{display:flex;flex-wrap:wrap;gap:6px}.chip{border:1px solid #fff;padding:2px 8px;font-size:11px}.chip.bad{border-color:#ff4d4d;color:#ff4d4d}
.crumb{color:#8a8a8a;font-size:11px}
</style>
<div class=bar><b>元素库 · 组 → 包（mono 皮 · 真窗口 980×1168）</b>
<span>窗口</span><button data-w=980>980（你现在这台）</button><button data-w=1600>1600</button>
<span>方案</span><button data-n=A>A 三栏挤进 980</button><button data-n=B>B 两栏 + 下钻</button><button data-n=C>C 组栏 + 包进详情</button>
<span id=desc></span></div>
<div class=stage><div class=win id=win></div></div>
<script>
const G=[{id:'frontend',name:'前端',lock:1,pk:['react-rules','sql-rules']},{id:'backend',name:'后端',pk:['api-errors']}];
const P={'react-rules':{name:'React 规则',kind:'rules',body:'- 组件一律函数式\n- hooks 不放条件里'},'api-errors':{name:'API 错误码',kind:'rules',body:'4xx 归客户端，5xx 归服务端'}};
let W=980,N='A',g='frontend',p=null,lv=1;
const D={A:'A：三栏原样塞进 980：组 180 / 包 220 / 详情 524。交互不变，代价是详情最窄，表单要收一点。',
B:'B：沿用 0.9.20 模板页的「窄窗下钻」（decisions.md）：左边一栏先是组，点组后同一栏换成该组的包（顶上「‹ 组」），详情照旧 644 宽，和模板页肌肉记忆一致。',
C:'C：左栏只放组（262，和现在一样宽），组详情里列出它的包，点包在右边原位换成包详情、顶部「‹ 前端 ›」回去。没有中间列，最省宽度。'};
const lockTag=x=>x.lock?'<span class=tag>锁</span>':'';
function rail(){return '<div class=rail><i></i><i></i><i class=on></i><i></i><i></i><i></i></div>'}
function foot(){return '<div class=foot>/opt/Nous-Elements · HEAD 132e86d</div>'}
function gRows(){return G.map(x=>{const bad=x.pk.some(k=>!P[k]);return `<div class="row ${g==x.id?'sel':''} ${bad?'bad':''}" onclick="g='${x.id}';p=null;lv=1;r()"><span class="n g">${x.name} <small>${x.id}</small></span>${lockTag(x)}<small>${x.pk.length}</small></div>`}).join('')}
function pRows(){const x=G.find(y=>y.id==g);return x.pk.map(k=>{const ok=P[k];return `<div class="row ${p==k?'sel':''} ${ok?'':'bad'}" onclick="${ok?`p='${k}';lv=2;r()`:''}"><span class="n p">${k}${ok?` <small>${P[k].name}</small>`:''}</span>${ok?'':'<span class=badtag>断链</span>'}</div>`}).join('')}
function groupCol(w){return `<div class=col style="width:${w}px"><div class=search>⌕ 搜组</div><div class=cap><span>组 · ${G.length}</span>＋</div><div class=list>${gRows()}</div>${foot()}</div>`}
function pkgCol(w,back){const x=G.find(y=>y.id==g);return `<div class=col style="width:${w}px"><div class=search>⌕ 在「${x.name}」里搜</div><div class=cap><span>${back?'<span class=back onclick="lv=0;r()">‹ 组</span>':''}${x.name} 的包 · ${x.pk.length}</span>＋</div><div class=list>${pRows()}</div>${foot()}</div>`}
function groupDetail(cards){const x=G.find(y=>y.id==g);return `<div class=main><div class=top><b>${x.name}</b><span class=k>groups/${x.id}.json</span><span class=sp></span><span class=btn>${x.lock?'解锁':'锁定'}</span><span class="btn ${x.lock?'dis':''}">删除</span><span class="btn ${x.lock?'dis':'pri'}">${x.lock?'已锁定':'保存并提交'}</span></div>${x.lock?'<div class=hint>已锁定 改不了也删不掉 —— 点顶栏「解锁」再改</div>':''}<div class=form><div class=f><label>id</label><div class=v>${x.id}</div></div><div class=f><label>名称</label><div class=v>${x.name}</div></div><div class=f><label>包 · ${x.pk.length}</label>${cards?x.pk.map(k=>P[k]?`<div class=row style="border:1px solid #2a2a2a;margin-bottom:6px" onclick="p='${k}';lv=2;r()"><span class="n p">${k} <small>${P[k].name}</small></span><small>›</small></div>`:`<div class="row bad" style="border:1px solid #ff4d4d;margin-bottom:6px"><span class="n p">${k}</span><span class=badtag>断链</span></div>`).join(''):'<div class=chips>'+x.pk.map(k=>`<span class="chip ${P[k]?'':'bad'}">${k}</span>`).join('')+'</div>'}</div></div></div>`}
function pkgDetail(crumb){const q=P[p];return `<div class=main><div class=top>${crumb?`<span class="back crumb" onclick="p=null;lv=1;r()">‹ ${G.find(y=>y.id==g).name} ›</span>`:''}<b>${p}</b><span class=k>packages/${p}.json</span><span class=sp></span><span class=btn>锁定</span><span class=btn>删除</span><span class="btn pri">保存并提交</span></div><div class=form><div class=f><label>名称</label><div class=v>${q.name}</div></div><div class=f><label>类型</label><div class=v>${q.kind}</div></div><div class=f><label>正文</label><div class="v area">${q.body}</div></div><div class=f><label>被这些组引用</label><div class=chips>${G.filter(y=>y.pk.includes(p)).map(y=>`<span class=chip>${y.name}</span>`).join('')}</div></div></div></div>`}
function r(){const w=document.getElementById('win');w.style.width=W+'px';let h=rail();
 if(W>=1000){h+=groupCol(220)+pkgCol(280)+(p?pkgDetail():groupDetail())}
 else if(N=='A'){h+=groupCol(180)+pkgCol(220)+(p?pkgDetail():groupDetail())}
 else if(N=='B'){h+=(lv==0?groupCol(280):pkgCol(280,1))+(p?pkgDetail():groupDetail())}
 else {h+=groupCol(262)+(p?pkgDetail(1):groupDetail(1))}
 w.innerHTML='<div class=title>NousDesk <span class=demo>DEMO</span></div><div class=body>'+h+'</div>';
 document.querySelectorAll('[data-w]').forEach(b=>b.classList.toggle('on',+b.dataset.w==W));
 document.querySelectorAll('[data-n]').forEach(b=>b.classList.toggle('on',b.dataset.n==N));
 document.getElementById('desc').textContent=W>=1000?'1600 宽：三套都落到同一个三栏（组 220 / 包 280 / 详情撑满）；差别只在 980。':D[N]}
document.querySelectorAll('[data-w]').forEach(b=>b.onclick=()=>{W=+b.dataset.w;r()});
document.querySelectorAll('[data-n]').forEach(b=>b.onclick=()=>{N=b.dataset.n;r()});
document.onkeydown=e=>{if('123'.includes(e.key)){N='ABC'['123'.indexOf(e.key)];r()}};
r();
</script></html>'''
open('/opt/NousDesk-elements-groups/demo/elements-groups/preview.html','w').write(html.replace('FONTS',fonts))
