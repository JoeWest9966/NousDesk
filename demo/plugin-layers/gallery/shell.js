// 真实 Desk 插件页骨架 + 通用渲染件（各方案只改「列表怎么排 / 详情怎么摆」）
window.Shell = (function(){
  const D = window.D;
  const by = new Map(D.nodes.map(n=>[n.name,n]));

  const esc = s => String(s).replace(/[&<>"]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));

  function chrome({no, title, thesis, tabs, statExtra}) {
    const t = tabs || [{label:'概览', on:false}, {label:'插件', n:D.counts.capsModules, on:true}];
    return `
    <div class="banner"><span class="no">${no}</span><b>${esc(title)}</b>
      <span class="sp"></span><span style="color:#5b5b5b;font-size:11px">${esc(thesis||'')}</span>
      <a class="back" href="index.html">← 10 套</a></div>
    <div class="win">
      <div class="tb"><span>NousDesk</span><span class="sp"></span>
        <span>—</span><i></i><i></i><i></i></div>
      <div class="head"><span class="bk">← 返回聊天</span><span class="sp" style="flex:1"></span>
        <span class="ttl">设置</span></div>
      <div class="body">
        <div class="nav">
          <div class="it">连接</div><div class="it on">关于</div>
        </div>
        <div class="pane">
          <div class="tabs">${t.map(x=>`<div class="t${x.on?' on':''}">${esc(x.label)}${x.n!=null?`<span class="n">${x.n}</span>`:''}</div>`).join('')}</div>
          <div class="statline">
            <span><b>${D.counts.capsModules}</b> 个模块</span>
            <span><b>${D.counts.capsModules}</b> 启用</span>
            <span><b>0</b> 未启用</span>
            <span><b>${D.counts.endpoints}</b> 端点</span>
            <span class="sp"></span>${statExtra||''}
          </div>
          <div class="cols" id="cols"></div>
        </div>
      </div>
    </div>`;
  }

  function vend(n){ return `<span class="vend">${esc(n.vendor)}</span>`; }

  function li(n, {meta, flag, extraClass}={}) {
    const m = meta!=null ? meta : `v${esc(n.version)} <span class="on">状态 启用</span> ${n.n} 端点`;
    return `<div class="li ${n.kind}${extraClass?' '+extraClass:''}" data-name="${esc(n.name)}">
      <div class="l1"><span class="nm">${esc(n.name)}</span>${flag||''}${vend(n)}</div>
      <div class="l2">${m}</div></div>`;
  }

  function sec(title, n, right){
    return `<div class="sec"><span>${esc(title)}</span>
      <span class="n">${right!=null?right:(n!=null?n:'')}</span></div>`;
  }

  function chip(name) {
    const n = by.get(name);
    if (!n) return `<span class="chip dead">${esc(name)}</span>`;
    return `<span class="chip ${n.kind}" data-chip="${esc(name)}" title="${esc(n.name)} · ${n.kind}">${esc(name)}</span>`;
  }

  function kv(rows){
    return `<div class="kv">${rows.map(([k,v,mono])=>
      `<div class="k">${esc(k)}</div><div class="v${mono?' mono':''}">${mono?esc(v):v}</div>`).join('')}</div>`;
  }

  // 详情公共头部 + 账本（真实那套：状态/版本/类型/层/必需/来源 crate）
  function head(n){
    return `<div class="dt-h"><span class="nm">${esc(n.name)}</span>
        <span class="doc">读文档</span></div>
      <div class="dt-vend">${vend(n)}
        <span class="pill ${n.kind==='module'?'mod':n.kind==='unlisted'?'unl':'lib'}">${
          n.kind==='module'?n.n+' 端点':n.kind==='unlisted'?'1 端点 · 未登记':'库 · 零端点'}</span>
        <span class="pill v">v${esc(n.version)}</span><span class="pill">${esc(n.layer)}</span></div>`;
  }

  function ledger(n){
    return kv([
      ['状态','启用'],
      ['版本', n.version, true],
      ['类型', n.kind==='lib' ? '依赖（库）' : '编译期'],
      ['层', n.layer],
      ['必需','是'],
      ['来源 crate', n.pkg, true],
    ]);
  }

  function eps(n, limit){
    if (!n.eps.length) return `<div class="eps"><div class="empty">这个库没有端点</div></div>`;
    const xs = limit ? n.eps.slice(0, limit) : n.eps;
    return `<div class="eps">${xs.map(([m,p])=>
      `<div class="e"><b>${esc(m)}</b><span>${esc(p)}</span></div>`).join('')}${
      limit && n.eps.length>limit ? `<div class="e"><b></b><span>… 还有 ${n.eps.length-limit} 条</span></div>`:''}</div>`;
  }

  function rel(n, {limit}={}){
    const deps = n.deps, inn = n.dependents;
    const cap = xs => limit ? xs.slice(0,limit) : xs;
    return `<div class="rel">
      <div class="rh"><span>关系 <span class="n">依赖 ${deps.length} · 被依赖 ${inn.length}</span></span></div>
      <div class="chips">${deps.length?cap(deps).map(chip).join(''):'<span class="empty">没声明依赖</span>'}${
        limit && deps.length>limit?`<span class="chip dead">+${deps.length-limit}</span>`:''}</div>
      <div style="height:8px"></div>
      <div class="chips">${inn.length?cap(inn).map(chip).join(''):'<span class="empty">没人依赖它</span>'}${
        limit && inn.length>limit?`<span class="chip dead">+${inn.length-limit}</span>`:''}</div>
    </div>`;
  }

  // 挂载：demo 自己渲染 list / detail
  function mount({no, title, thesis, tabs, statExtra}, render) {
    document.body.innerHTML = `<div class="stage">${chrome({no,title,thesis,tabs,statExtra})}</div>`;
    const cols = document.getElementById('cols');
    const api = { D, by, cols };
    api.onPick = cb => cols.addEventListener('click', e => {
      const c = e.target.closest('.chip[data-chip], .li[data-name]');
      if (!c) return;
      cb(c.dataset.chip || c.dataset.name);
    });
    render(api);
    return api;
  }

  return {mount, chrome, li, sec, chip, kv, head, ledger, eps, rel, vend, esc, by, D};
})();