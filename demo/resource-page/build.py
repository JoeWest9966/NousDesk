#!/usr/bin/env python3
"""从 base.html（= 上一份 demo 里 02 那一套）拼出 index.html：样式/图标/渲染函数沿用，正文重写。

这一份只做 02 的那一套 IA（左栏一个「资源」图标 + 三段），
镜头从「挑 IA」换成「把每一屏画细」：三页主界面 / 三个新建 / 发布 / 编辑 / 空态。
"""
import pathlib

here = pathlib.Path(__file__).resolve().parent
src = (here / "base.html").read_text(encoding="utf-8").split("\n")
# base.html 是 02 那份的**冻结快照**（1445 行）。下面的 span(…) 按行号切，
# 换了 base 就会切错 —— 所以先钉住行数，改坏了当场炸，别产出半份 HTML。
assert len(src) == 1446, f"base.html 应该是 1446 行（含末尾空行），现在是 {len(src)} 行：span 的行号要重算"


def span(a, b):
    """1-indexed, inclusive."""
    return "\n".join(src[a - 1:b])


# ── 沿用旧文件的部分 ─────────────────────────────────────────────
head = span(1, 389)                # <head> + <style>（到 </style>）—— 新的 CSS 另起一段接在后面
body_sprite = span(391, 424)       # <body> + svg symbol 仓库
helpers = span(516, 585)           # esc / ic / cat / inCat / catNodes / kb / md / repoLine / pendingUnits
tab_meta = span(610, 615)          # TAB_META
trees = span(636, 710)             # catRow / groupSep / skillsTree / promptsTree / extTree / treeHTML / tabCount
lists = span(712, 1080)            # rows / rowHTML / listHTML / 三个 detail / emptyDetail
toast = span(1384, 1396)           # toast()

CSS = r"""
/* ── 段按钮：三种形态（02 的段就是三个仓，脏/失败得看得见）───── */
.segs .lbl { display: flex; align-items: center; gap: 4px; }
.sg-dot { width: 5px; height: 5px; border-radius: 50%; background: var(--warn); display: inline-block; }
.sg-dot.bad { background: var(--danger); }
.segs.inline button { flex-direction: row; align-items: center; justify-content: flex-start;
  gap: 5px; padding: 5px 7px; font-size: 11.5px; white-space: nowrap; overflow: hidden; }
.segs.inline button .ic { width: 13px; height: 13px; }
.segs.inline button .t { white-space: nowrap; }
.segs.inline button .n { margin-left: auto; font-size: 9.5px; }

/* ── 新建：表单 + 建出来的东西 ────────────────────────────── */
.dialog.wide { width: 580px; }
.fields { padding: 4px 18px 8px; display: flex; flex-direction: column; gap: 10px; }
.field { display: flex; flex-direction: column; gap: 4px; }
.field > label { font-size: 10.5px; color: var(--muted); display: flex; align-items: center; gap: 4px; }
.field > label .req { color: var(--danger); }
.field input[type=text], .field select, .field textarea {
  border: 1px solid var(--line); border-radius: calc(6px * var(--r)); background: var(--surface);
  padding: 6px 8px; font: inherit; font-size: 11.5px; color: var(--ink); outline: none; width: 100%; }
.field textarea { min-height: 66px; resize: vertical; line-height: 1.6; }
.field input:focus, .field select:focus, .field textarea:focus { border-color: var(--accent); background: var(--panel); }
.field .hint { font-size: 10px; color: var(--faint); line-height: 1.6; }
.catbar { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
.catbar button { height: 20px; padding: 0 7px; border: 1px solid var(--line); border-radius: calc(4px * var(--r));
  background: transparent; font: inherit; font-size: 10px; color: var(--muted); cursor: pointer; }
.catbar button.on { background: var(--accent-soft); border-color: transparent; color: var(--accent-fg); }
.catbar input { flex: 1; min-width: 90px; border: 0; border-bottom: 1px dashed var(--line-2); background: transparent;
  font: inherit; font-size: 10.5px; color: var(--ink); outline: none; height: 20px; }
.gen { margin: 2px 18px 10px; border: 1px solid var(--line-soft); border-radius: calc(7px * var(--r)); overflow: hidden; }
.gen .t { display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--panel2);
  border-bottom: 1px solid var(--line-soft); font-size: 10px; color: var(--muted); }
.gen .t .ic { width: 12px; height: 12px; }
.gen pre { margin: 0; padding: 8px 10px; font-family: JBMono, monospace; font-size: 9.5px; line-height: 1.7;
  color: var(--ink2); background: var(--code); overflow: auto; max-height: 150px; }
.err-slot { padding: 0 18px 6px; font-size: 10.5px; color: var(--danger-fg); min-height: 15px; }

/* ── 发布：三个仓 ─────────────────────────────────────────── */
.repos { display: flex; flex-direction: column; gap: 8px; padding: 2px 18px 10px; }
.repo { border: 1px solid var(--line); border-radius: calc(8px * var(--r)); overflow: hidden; }
.repo .rh { display: flex; align-items: center; gap: 6px; padding: 8px 10px; background: var(--panel2); }
.repo .rh .ic { width: 13px; height: 13px; color: var(--muted); }
.repo .rh .nm { font-size: 11.5px; color: var(--ink2); }
.repo .rh .h { margin-left: auto; font-family: JBMono, monospace; font-size: 9.5px; color: var(--faint); }
.repo .rb { padding: 8px 10px; display: flex; flex-direction: column; gap: 5px; }
.repo .u { display: flex; align-items: center; gap: 6px; font-size: 10.5px; color: var(--ink2); }
.repo .u .ic { width: 11px; height: 11px; color: var(--faint); flex: 0 0 11px; }
.repo .u .mono { font-size: 10px; }
.repo .u .n { margin-left: auto; font-family: JBMono, monospace; font-size: 9.5px; color: var(--muted); }
.repo .foot { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border-top: 1px solid var(--line-soft); }

/* ── 编辑：整页 ───────────────────────────────────────────── */
.bar { height: 42px; display: flex; align-items: center; gap: 8px; padding: 0 14px;
  border-bottom: 1px solid var(--line-soft); }
.crumb { display: flex; align-items: center; gap: 5px; font-size: 11.5px; color: var(--muted); }
.crumb .ic { width: 11px; height: 11px; color: var(--faint); }
.crumb .cur { color: var(--ink2); }
.crumb button { border: 0; background: none; font: inherit; color: var(--link); cursor: pointer; padding: 0; }

/* ── 空态 / 坏态 ──────────────────────────────────────────── */
.states { flex: 1; padding: 16px; display: flex; flex-direction: column; gap: 12px; background: var(--chat); }
.states-head { padding: 2px 2px 0; }
.states-head h3 { font-size: 14px; font-weight: 500; color: var(--ink2); }
.states-head p { font-size: 10.5px; color: var(--muted); line-height: 1.7; margin-top: 3px; }
.sgrid { display: grid; grid-template-columns: 1fr 1fr; grid-auto-rows: minmax(190px, auto); gap: 12px; }
.state { border: 1px solid var(--line); border-radius: calc(9px * var(--r)); background: var(--panel);
  padding: 13px 14px; display: flex; flex-direction: column; gap: 8px; min-height: 0; }
.state h4 { font-size: 10px; font-weight: 400; color: var(--faint); letter-spacing: .06em;
  display: flex; align-items: center; gap: 6px; }
.state .box { flex: 1; display: flex; flex-direction: column; align-items: flex-start;
  justify-content: flex-start; gap: 7px; padding-top: 4px; }
.state .box.center { align-items: center; text-align: center; }
.state .t { font-size: 12px; color: var(--ink2); }
.state .d { font-size: 10.5px; color: var(--muted); line-height: 1.75; }
.state .d .mono { font-size: 9.5px; }
"""

PICKER = r"""<aside class="picker">
  <h1>资源页 · 02 落地稿</h1>
  <div class="sub">02 定的是信息架构：左栏一个图标、页内三段、树 + 列表 + 详情。
    这一份把每一屏画细 —— 新建、发布、编辑、空态。</div>
  <div class="plist" id="plist"></div>
  <div class="pnote" id="pnote"></div>
  <div class="skinbar"><span>段</span>
    <button data-seg="a" class="on" title="药丸：名字 + 条数">A</button>
    <button data-seg="b" title="药丸 + 脏点">B</button>
    <button data-seg="c" title="图标 + 名字 + 条数">C</button>
  </div>
  <div class="skinbar"><span>皮</span>
    <button data-skin="default" class="on">默认</button>
    <button data-skin="amber">琥珀</button>
  </div>
  <div class="pfoot">
    1280×800 · 真数据（j1900 引擎 · 2026-11-16）<br>
    <span class="hint">在窗口里也能点：</span>三段切换、类别、行、<br>
    新建、发布、编辑、空态卡片。
  </div>
</aside>
"""

STAGE = r"""<main class="stage">
  <div class="scaler" id="scaler">
    <div class="app" id="app" data-skin="default"></div>
  </div>
</main>
"""

JS = r"""
/* ── 02 那一套写死 ─────────────────────────────────────────────
   左栏：聊天 / 模板 / 触发器 / 资源 / 数据 / ≡。页内三段，不再切 IA。
   镜头（左侧 01–09）只是「停在哪一屏」，不是五套方案。 */
const FIXED = { rail: 'one3', skel: '3col', entry: 'skills' };

const SCREENS = [
  { id: 'skills', name: '技能 · 主界面', tab: 'skills',
    note: `<b>默认落点。</b>树的上半是类别（父类含子类），下半是<b>状态</b>：变体 / 藏起来的 ——
      这两类不属于任何类别，塞进类别树就会和「工程」这种真类别混成一层。
      行里那句摘要不是装饰：<em>description 会进模型的技能清单</em>，模型靠它决定用不用。` },
  { id: 'prompts', name: '提示词 · 主界面', tab: 'prompts',
    note: `<b>单值，所以列表是单选形状。</b>「引擎默认那条」单独成组 ——
      默认 ≠ 我选的：<code>settings.json</code> 只负责给新会话预勾。
      树脚那句「N 处待发布」把三个仓的脏状态分开显示，不合并成一个总数。` },
  { id: 'ext', name: '扩展 · 主界面', tab: 'ext',
    note: `<b>状态在上、类别在下。</b>来这页的人多半是「有个坏的」或「想关一条」，不是为了按类别逛。
      五类状态各自可点：启用 / 内置 / 加载失败 / 变体 / 已归档。详情里「清单说 sections X、
      进程里真装了 X 段」那一行是<b>对账</b> —— 清单是手写的，只有这一行能揭穿它。` },
  { id: 'new-skill', name: '新建 · 技能', tab: 'skills', sheet: 'skill',
    note: `<b>名字 = 目录名 = 会话里的键，建完不能改</b>，所以表单上头先说这件事。
      description 必填（引擎直接 400：「模型靠它判断该不该用这条技能」）。
      建出来是<b>未经发布</b>的骨架：<code>SKILL.md</code> + <code>nous.json</code>，
      发布之前别人 clone 看不到它。` },
  { id: 'new-prompt', name: '新建 · 提示词', tab: 'prompts', sheet: 'prompt',
    note: `摘要必填（「人靠这一行决定选不选它」），正文可以空 —— 空就落一个 <code># 名字</code> 标题。
      <b>建完该直接进编辑器</b>：只落一个标题的模板能装载，但选它等于没选。` },
  { id: 'new-ext', name: '新建 · 扩展', tab: 'ext', sheet: 'ext',
    note: `<b>骨架故意什么都不装。</b><code>sections: []</code> = 不往提示词里塞东西，
      <code>entry: "default"</code> 就一个空函数。这是刻意的：新扩展默默往提示词里加一段，
      是最难查的那种惊喜。要干活得自己写 <code>index.js</code>。` },
  { id: 'publish', name: '发布 · 三个仓', tab: 'skills', sheet: 'publish',
    note: `<b>三个仓各自发布，没有「一键全发」。</b>三份闸、三个 remote；
      合并成一次 commit 之后，「技能闸过了、扩展闸没过」就说不清该退哪一半。
      扩展还多一条路：词表 / 清单走 <code>sync</code>，代码走 <code>publish</code>。` },
  { id: 'edit', name: '编辑 · 正文', tab: 'skills', edit: true,
    note: `<b>浏览在列表，改动在这一屏。</b>顶上一条面包屑回得去；
      右边永远写着「保存 ≠ 发布」，因为这两件事在这个仓里真的分开
      （保存只落盘，发布要过闸 + 重扫 index.json + commit + push）。` },
  { id: 'states', name: '空态 · 坏态', tab: 'skills', states: true,
    note: `<b>四种空/坏，各有一条出路。</b>没有一格是「请稍后重试」了事：
      仓拉不到给重试、名册空给新建、一条坏掉给错误原文 + 重扫、搜不到给「换个词」。` },
];

const S = {
  screen: (location.hash || '').replace('#', ''),
  segStyle: 'a',
  skin: 'default',
  tab: 'skills',
  cat: {},
  sel: { skills: 'code-review', prompts: 'smart-ass', ext: 'skills' },
  q: '',
  openFile: {},
  menu: false,
  pub: {},
};
if (!SCREENS.some((s) => s.id === S.screen)) S.screen = 'skills';
{ const first = SCREENS.find((s) => s.id === S.screen); if (first && first.tab) S.tab = first.tab; }

const screenOf = () => SCREENS.find((s) => s.id === S.screen);

function go(id) {
  S.screen = id;
  const sc = screenOf();
  if (sc.tab) S.tab = sc.tab;
  S.q = '';
  history.replaceState(null, '', '#' + id);
  render();
}

/* ── 图标条 ───────────────────────────────────────────────── */
function railHTML() {
  const items = [['chats', 'chat', '聊天'], ['templates', 'book', '模板'], ['triggers', 'bolt', '触发器'],
                 ['res', 'layers', '技能 / 提示词 / 扩展'], ['data', 'db', '数据监控室']];
  return `<aside class="rail">
    ${items.map(([id, icon, title]) =>
      `<button class="rbtn ${id === 'res' ? 'on' : ''}" data-act="rail:${id}" title="${title}">${ic(icon)}</button>`).join('')}
    <div class="rsp"></div>
    <button class="rbtn ${S.menu ? 'open' : ''}" data-act="menu" title="更多">${ic('menu')}</button>
  </aside>`;
}

/* ── 三段。三种形态只差这一块 ─────────────────────────────── */
const SEGS = [
  ['skills', '技能', 'spark'],
  ['prompts', '提示词', 'quote'],
  ['ext', '扩展', 'puzzle'],
];
function segBad(kind) {
  if (kind === 'skills') return { dirty: D.skills.repo.dirty, bad: 0 };
  if (kind === 'prompts') return { dirty: D.prompts.repo.dirty, bad: 0 };
  return { dirty: D.exts.repo.drift, bad: D.exts.catalog.concat(Object.keys(D.exts.variants)).length ? 1 : 0 };
}
function segCount(kind) {
  if (kind === 'skills') return D.skills.items.filter((s) => !s.hidden).length;
  if (kind === 'prompts') return D.prompts.items.length;
  return extRows().length;
}
function segHTML() {
  if (S.segStyle === 'c') {
    return `<div class="segs inline">${SEGS.map(([id, label, icon]) =>
      `<button data-act="seg:${id}" class="${S.tab === id ? 'on' : ''}">${ic(icon)}
        <span class="t">${label}</span><span class="n">${segCount(id)}</span></button>`).join('')}</div>`;
  }
  return `<div class="segs">${SEGS.map(([id, label]) => {
    const b = segBad(id);
    const dot = S.segStyle === 'b' && b.dirty
      ? `<i class="sg-dot ${b.bad ? 'bad' : ''}" title="${b.bad ? '有加载失败的条目' : '有未提交的改动'}"></i>` : '';
    return `<button data-act="seg:${id}" class="${S.tab === id ? 'on' : ''}">
      <span class="lbl">${label}${dot}</span><span class="n">${segCount(id)}</span></button>`;
  }).join('')}</div>`;
}

/* 详情头里塞一颗「编辑」：三个 detail 的头部结构一样（一个 .acts），插一刀就够。 */
function withEdit(html) {
  return html.replace('<span class="acts">',
    `<span class="acts"><button class="btn sm ghost" data-act="edit">${ic('code')}编辑</button>`);
}

/* ── 三栏 ─────────────────────────────────────────────────── */
function mainHTML() {
  const detail = withEdit(
    S.tab === 'skills' ? detailSkill(selOf('skills'))
    : S.tab === 'prompts' ? detailPrompt(selOf('prompts'))
    : extDetail(selOf('ext')));
  return `<div class="page">${treeHTML(FIXED)}${listHTML(FIXED)}${detail}</div>`;
}

/* ── 新建：三张表 ─────────────────────────────────────────── */
const NAME_RE = /^[a-z0-9][a-z0-9._-]*$/;
const CORE = ['nous', 'skills', 'prompts'];

function formHTML(kind) {
  const meta = {
    skill: { title: '新建技能', act: 'create:skill', ok: '建（未发布）' },
    prompt: { title: '新建提示词', act: 'create:prompt', ok: '建（未发布）' },
    ext: { title: '新建扩展', act: 'create:ext', ok: '建（未发布）' },
  }[kind];
  const cats = kind === 'skill' ? D.skills.categories
    : kind === 'prompt' ? D.prompts.categories : D.exts.categories;
  const catbar = `<div class="catbar">${cats.slice(0, 6).map((c) =>
    `<button data-act="pickcat" data-cat="${esc(c)}">${esc(c)}</button>`).join('')}
    <input data-f="cat" placeholder="+ 新类别"></div>`;
  const fields = kind === 'skill' ? `
      <div class="field"><label>名字 <span class="req">*</span>
        <span class="hint" style="margin-left:auto">小写 · 目录名 · 会话里的键</span></label>
        <input type="text" data-f="name" placeholder="weekly-report" autocomplete="off">
        <div class="hint">建完<b>不能改名</b>（会话里存的就是这个名字）。不要了就把目录删掉。</div></div>
      <div class="field"><label>分组<span class="hint">（可选：仓里现成的抽屉，比如 deprecated）</span></label>
        <select data-f="group"><option value="">（不放分组）</option></select></div>
      <div class="field"><label>一句话描述 <span class="req">*</span></label>
        <input type="text" data-f="description" placeholder="把这周的会议、提交、聊天整理成一份能直接发给老板的周报" autocomplete="off">
        <div class="hint">进 <code>description</code> 和 <code>SKILL.md</code> 的 frontmatter。
          模型的技能清单里只有这一行 —— 写不清楚它就挑不中。</div></div>
      <div class="field"><label>类别<span class="hint">（会写进类目词表）</span></label>${catbar}</div>`
    : kind === 'prompt' ? `
      <div class="field"><label>名字 <span class="req">*</span>
        <span class="hint" style="margin-left:auto">小写 · 目录名 · 会话里的键</span></label>
        <input type="text" data-f="name" placeholder="incident-voice" autocomplete="off"></div>
      <div class="field"><label>摘要 <span class="req">*</span></label>
        <input type="text" data-f="summary" placeholder="事故复盘时用的一种语气：只写看得见的事实" autocomplete="off">
        <div class="hint">住在 <code>nous.json</code>。它不进提示词 —— <b>只给挑模板的人看</b>，
          所以写「什么时候选它」比写「它是干什么的」有用。</div></div>
      <div class="field"><label>正文<span class="hint">（可空；空就落一个标题）</span></label>
        <textarea data-f="body" placeholder="# incident-voice&#10;&#10;只写看得见的事实……"></textarea>
        <div class="hint">这里写的东西<b>整段</b>进系统提示词 —— 不是摘要，也不是名字。</div></div>
      <div class="field"><label>类别<span class="hint">（会写进类目词表）</span></label>${catbar}</div>`
    : `
      <div class="field"><label>名字 <span class="req">*</span>
        <span class="hint" style="margin-left:auto">小写 · 目录名 · defineExtension 的 name</span></label>
        <input type="text" data-f="name" placeholder="disk-guard" autocomplete="off">
        <div class="hint">核心三条（<code>nous</code> / <code>skills</code> / <code>prompts</code>）不能新建。</div></div>
      <div class="field"><label>一句话说明 <span class="req">*</span>
        <span class="hint" style="margin-left:auto">≤ 200 字</span></label>
        <input type="text" data-f="summary" placeholder="磁盘快满的时候先喊一声，别等写不进去" autocomplete="off">
        <div class="hint">卡片上就靠它认人。它<b>不进</b>提示词。</div></div>
      <div class="field"><label>类别<span class="hint">（会写进类目词表）</span></label>${catbar}</div>`;
  return `<div class="sheet"><div class="scrim" data-act="close"></div>
    <div class="dialog" id="dlg-${kind}">
      <div class="dh">${meta.title}<span class="chip warn" style="margin-left:8px">未发布</span></div>
      <div class="db">${kind === 'ext'
        ? '落盘 = 未发布。发布要过 <code>node --check</code> 和 <code>index.mjs</code>，然后在扩展仓 commit + push。'
        : '落盘 = 未发布。目录已经能被引擎扫到（会话里勾得到），但没进 <code>HEAD</code> —— 发布之后才算真的一条。'}</div>
      <div class="fields">${fields}</div>
      <div class="gen"><div class="t">${ic('file')}建出来是什么</div><pre></pre></div>
      <div class="err-slot"></div>
      <div class="df"><button class="btn sm" data-act="close">取消</button>
        <button class="btn sm pri" data-act="${meta.act}">${meta.ok}</button></div>
    </div></div>`;
}

function readForm(kind) {
  const dlg = document.getElementById('dlg-' + kind);
  const get = (f) => (dlg.querySelector(`[data-f="${f}"]`)?.value ?? '').trim();
  return { name: get('name'), summary: get('summary'), description: get('description'),
    body: get('body'), group: get('group'), cat: get('cat') };
}
function validate(kind, f) {
  if (!f.name) return { ok: false, err: '名字不能空' };
  if (!NAME_RE.test(f.name)) return { ok: false, err: '名字只能用小写字母、数字和 . _ -，且不能以符号开头' };
  if (f.name.length > 64) return { ok: false, err: '名字 ≤ 64 字符（它同时是目录名和会话里的键）' };
  if (kind === 'ext') {
    if (CORE.includes(f.name)) return { ok: false, err: `${f.name} 是核心扩展（引擎启动路径上硬编码 import）—— 不能新建` };
    if (!f.summary) return { ok: false, err: '一句话说明不能空 —— 卡片上就靠它认人' };
    if (f.summary.length > 200) return { ok: false, err: '说明太长了（200 字以内）' };
  }
  if (kind === 'skill' && !f.description)
    return { ok: false, err: 'description 不能空 —— 模型靠它判断该不该用这条技能' };
  if (kind === 'prompt' && !f.summary)
    return { ok: false, err: 'summary 不能空 —— 人靠这一行决定选不选它' };
  return { ok: true, err: '' };
}
function previewOf(kind, f) {
  const name = f.name || '<名>';
  if (kind === 'skill') {
    return ['---', `name: ${name}`,
      `description: ${JSON.stringify(f.description || '<一句话描述>')}`, '---', '', `# ${name}`, ''].join('\n')
      + '\n// nous.json\n{\n  "name": ' + JSON.stringify(name) + ',\n  "summary": "",\n  "category": []\n}';
  }
  if (kind === 'prompt') {
    return (f.body || `# ${name}`) + '\n\n// nous.json\n{\n  "name": ' + JSON.stringify(name)
      + ',\n  "summary": ' + JSON.stringify(f.summary || '<摘要>') + ',\n  "category": []\n}';
  }
  return `// ${name} —— ${f.summary || '<一句话说明>'}
//
// 一个扩展 = 一个目录 + 一份清单（nous.json）+ 一个入口（这个文件）。
// 现在它是空的：只声明名字，什么都不装。
//
// 1. 往系统提示词里加一段：sections: [section("${name}", () => "…")]
// 2. 加工具：tools: [defineTool({ … })]
// 3. 要宿主的东西：nous.json 的 needs 里写键名（api / dir / registry …）
export default ({ defineExtension }) => defineExtension({ name: "${name}" });

// nous.json
{
  "name": "${name}", "version": "0.1.0", "file": "index.js",
  "entry": "default", "sections": [], "summary": "${f.summary || '<一句话说明>'}",
  "needs": {}
}`;
}
function bindForm(kind) {
  const dlg = document.getElementById('dlg-' + kind);
  if (!dlg) return;
  let touched = false;
  const pre = dlg.querySelector('.gen pre');
  const slot = dlg.querySelector('.err-slot');
  const ok = dlg.querySelector(`[data-act="create:${kind}"]`);
  const refresh = () => {
    const f = readForm(kind);
    const v = validate(kind, f);
    if (pre) pre.textContent = previewOf(kind, f);
    if (slot) slot.textContent = touched && !v.ok ? v.err : '';
    if (ok) ok.disabled = !v.ok;
    return v;
  };
  dlg.querySelectorAll('input, select, textarea').forEach((el) => {
    el.addEventListener('input', () => { touched = true; refresh(); });
    el.addEventListener('change', () => { touched = true; refresh(); });
  });
  dlg.querySelectorAll('.catbar button').forEach((b) => b.addEventListener('click', (e) => {
    e.preventDefault();
    b.classList.toggle('on');
  }));
  refresh();
}
function createFrom(kind) {
  const f = readForm(kind);
  const v = validate(kind, f);
  if (!v.ok) {
    const slot = document.querySelector(`#dlg-${kind} .err-slot`);
    if (slot) slot.textContent = v.err;
    return;
  }
  if (kind === 'skill') {
    D.skills.items.unshift({ name: f.name, description: f.description, hidden: false, tracked: false, files: 2,
      category: f.cat ? [f.cat] : [] });
    S.sel.skills = f.name;
    go('skills');
  } else if (kind === 'prompt') {
    D.prompts.items.unshift({ name: f.name, summary: f.summary, bytes: (f.body || `# ${f.name}`).length,
      category: f.cat ? [f.cat] : [], tracked: false });
    S.sel.prompts = f.name;
    go('prompts');
  } else {
    go('ext');
  }
  toast(`已建 ${f.name} · 未发布 —— 盘上有、仓里没`);
}

/* ── 发布：三个仓 ─────────────────────────────────────────── */
function unitLabel(u) {
  return u.key === '__categories' ? '类目词表' : (u.name || u.key);
}

function publishHTML() {
  const repos = [
    { key: 'skills', name: 'Nous-skills', root: D.skills.root, head: D.skills.repo.head,
      dirty: D.skills.repo.dirty, units: D.skills.pending },
    { key: 'prompts', name: 'Nous-prompts', root: D.prompts.root, head: D.prompts.repo.head,
      dirty: D.prompts.repo.dirty, units: D.prompts.pending },
    { key: 'ext', name: 'Nous-extend', root: D.exts.root, head: D.exts.repo.head,
      dirty: D.exts.repo.drift, units: [], pullError: D.exts.repo.pullError },
  ];
  return `<div class="sheet"><div class="scrim" data-act="close"></div>
    <div class="dialog wide" id="dlg-publish">
      <div class="dh">发布 · 三个仓各走各的</div>
      <div class="db">三份闸、三个 remote，<b>不做「一键全发」</b>。
        技能闸过了、扩展闸没过的时候，合成一次 commit 就说不清该退哪一半。</div>
      <div class="repos">${repos.map((r) => `<div class="repo">
        <div class="rh">${ic('git')}<span class="nm">${r.name}</span>
          <span class="chip ${r.dirty ? 'warn' : 'accent'}">${r.dirty ? '有改动' : '干净'}</span>
          <span class="h">${short(r.head)}</span></div>
        <div class="rb">${r.units.length
          ? r.units.map((u) => `<div class="u">${ic('file')}<span class="mono">${esc(unitLabel(u))}</span>
              <span class="n">${u.files.map((f) => `${f.status} ${f.path}`).join(' · ')}${u.added ? ` +${u.added}` : ''}${u.removed ? ` −${u.removed}` : ''}</span></div>`).join('')
          : r.pullError
            ? `<div class="u" style="color:var(--danger-fg)">${ic('alert')}
                <span>pull 失败：${esc((r.pullError || '').split('\n')[0])}</span></div>
               <div class="u" style="color:var(--muted)">先修上游（<code>git branch --set-upstream-to</code>），再谈发布。</div>`
            : `<div class="u" style="color:var(--muted)">没有待发布的单元</div>`}</div>
        <div class="foot">
          <span class="hint" style="font-size:9.5px;color:var(--faint)">${esc(r.root)}</span>
          <button class="btn sm ${r.units.length ? 'pri' : ''}" data-act="publish-repo:${r.key}"
            ${r.units.length ? '' : 'disabled'} style="margin-left:auto">
            ${r.key === 'ext' ? '同步词表 / 清单' : `发布 ${r.units.length} 处`}</button></div>
      </div>`).join('')}</div>
      <div class="err-slot" style="color:var(--muted)">扩展仓还有一条单独的路：代码走 <code>publish</code>，
        词表和清单走 <code>sync</code> —— 两者都只提交自己那一类文件。</div>
      <div class="df"><button class="btn sm" data-act="close">关了</button></div>
    </div></div>`;
}

/* ── 编辑 ─────────────────────────────────────────────────── */
function editCtx() {
  const tab = S.tab;
  const name = selOf(tab);
  const files = tab === 'skills'
    ? (D.skills.files[name] || [{ path: 'nous.json', size: null }, { path: 'SKILL.md', size: null }])
    : tab === 'prompts'
      ? (D.prompts.files[name] || [{ path: 'nous.json', size: null }, { path: 'PROMPT.md', size: null }])
      : (D.exts.files[name] || [{ path: 'nous.json', size: null }]);
  const open = S.openFile[name] || (files.find((f) => /\.(md|js)$/.test(f.path)) || files[0]).path;
  const text = tab === 'skills' ? D.skills.text[`${name}/${open}`]
    : tab === 'prompts' ? D.prompts.text[`${name}/${open}`]
    : D.exts.snippet[`${name}/${open}`];
  return { tab, name, files, open, text: text || '// 这份 demo 没内嵌这个文件的正文\n' };
}
function editHTML() {
  const e = editCtx();
  const label = TAB_META[e.tab].title;
  const units = e.tab === 'ext' ? [] : pendingUnits(e.tab);
  const lines = e.text.split('\n');
  let inFront = lines[0]?.trim() === '---';
  const code = lines.map((l, i) => {
    const kind = inFront ? 'fm' : (/^#{1,3}\s/.test(l) ? 'h' : '');
    if (inFront && i > 0 && l.trim() === '---') inFront = false;
    return `<div class="ln ${kind}"><span class="no">${i + 1}</span><span class="tx">${esc(l) || ' '}</span></div>`;
  }).join('');
  const r = repoLine(e.tab === 'ext' ? 'ext' : e.tab);
  return `<div class="page">
    <div class="wb" style="flex-direction:column">
      <div class="bar">
        <div class="crumb">
          <button data-act="back">资源</button>${ic('chev-r')}
          <span>${label}</span>${ic('chev-r')}
          <span class="cur mono">${esc(e.name)}</span>${ic('chev-r')}
          <span class="cur mono">${esc(e.open)}</span></div>
        <div class="grow"></div>
        <span class="chip mono">保存 ≠ 发布</span>
        <button class="btn sm ghost" data-act="save">${ic('check')}保存 <span class="kbd">Ctrl+S</span></button>
        <button class="btn sm ${units.length ? 'pri' : ''}" data-act="publish">${ic('upload')}发布${units.length ? ` ${units.length} 处` : ''}</button>
      </div>
      <div style="display:flex;flex:1;min-height:0">
        <div class="wb-files">
          <div class="h">${esc(e.name)}</div>
          ${e.files.map((f) => `<div class="f ${f.path === e.open ? 'on' : ''}" data-act="file:${esc(e.name)}:${esc(f.path)}">
            ${ic(/\.md$/.test(f.path) ? 'file' : 'code')}<span class="mono">${esc(f.path)}</span></div>`).join('')}
          <div class="foot"><div><span class="dot ${r.dot}"></span> ${r.text}</div>
            <div class="mono" style="font-size:9.5px;margin-top:2px">${r.mono}</div></div>
        </div>
        <div class="wb-main">
          <div class="wb-bar"><span class="tab on">${ic('file')}<span>${esc(e.open)}</span><span class="x">${ic('x')}</span></span>
            <div class="grow"></div>
            <span class="mono" style="font-size:10px;color:var(--faint)">${kb((e.files.find((f) => f.path === e.open) || {}).size)} · UTF-8</span></div>
          <div class="code scroll">${code}</div>
        </div>
        <div class="pub">
          <div class="h">${ic('upload')}发布<span class="grow"></span>
            <span class="mono" style="font-size:10px;color:var(--muted)">${short(D[e.tab === 'ext' ? 'exts' : e.tab].repo.head)}</span></div>
          <div class="body">
            <div class="note-line">发布是一步到位的：闸过了才重扫 <code>index.json</code>，然后 commit + push。
              闸不过<b>不动 index.json</b> —— 不然会话里会看见一条根本不存在的条目。</div>
            <div class="sec-title" style="margin:2px 0 4px">待发布 ${units.length}</div>
            ${units.map((u) => `<div class="unit"><div class="nm">${esc(unitLabel(u))}
              ${u.tracked ? '' : '<span class="chip warn">新</span>'}</div>
              <div class="fl">${u.files.map((f) => `<span>${f.status === '??' ? 'A' : f.status} ${esc(f.path)}
                ${u.added ? `<span class="add">+${u.added}</span>` : ''}${u.removed ? ` <span class="del">−${u.removed}</span>` : ''}</span>`).join('')}</div>
            </div>`).join('') || '<div class="note-line">这个仓现在没有待发布的单元。</div>'}
            <div class="steps">${['内容闸', '重扫 index.json', 'commit', 'push'].map((x, i) =>
              `<div class="step"><span class="k">${i + 1}</span>${x}</div>`).join('')}</div>
            <div class="note-line" style="margin-top:2px">第一关是内容闸：<code>SKILL.md</code> 的 frontmatter、
              名字和目录对得上、<code>nous.json</code> 能解析。不过就整棵不动。</div>
          </div>
        </div>
      </div>
    </div>
  </div>`;
}

/* ── 空态 / 坏态 ──────────────────────────────────────────── */
function statesHTML() {
  return `<div class="page"><div class="states">
    <div class="states-head"><h3>四种空 / 坏，各有一条出路</h3>
      <p>每一格都是<b>真会碰到</b>的状态。共同规矩：别写「请稍后重试」——要么给一颗能点的按钮，要么说清楚为什么这里什么都没有。</p></div>
    <div class="sgrid">
    <div class="state"><h4>${ic('alert')}仓拉不到 · GET /v1/skills 500</h4>
      <div class="box">
        <div class="t">拉技能失败</div>
        <div class="d mono">connect ECONNREFUSED 127.0.0.1:4790</div>
        <button class="btn sm" data-act="retry">重试</button>
        <div class="d">不自动重试 —— 失败就重试会变成「失败 → 重画 → 再拉」的死循环。
          给一颗按钮，人自己点。</div>
      </div></div>
    <div class="state"><h4>${ic('spark')}名册空 · 0 条</h4>
      <div class="box center">
        <div class="t">这个仓还没有技能</div>
        <div class="d">一个目录一条（<code>SKILL.md</code> + <code>nous.json</code>）。<br>
          建完是「未发布」，发布之后别人 clone 才看得到。</div>
        <button class="btn sm pri" data-act="new">${ic('plus')}建第一条</button>
      </div></div>
    <div class="state"><h4>${ic('git')}一条加载失败 · load.state = failed</h4>
      <div class="box">
        <div class="t mono">code-review@123</div>
        <div class="d mono" style="color:var(--danger-fg)">名册里没有这一条（树没了？）</div>
        <div class="d">这一条不消失 —— 它是一棵<b>变体树</b>，删了目录也还在。给两件事：</div>
        <div style="display:flex;gap:6px"><button class="btn sm" data-act="rescan">重新扫描</button>
          <button class="btn sm danger" data-act="drop">拆掉变体树</button></div>
      </div></div>
    <div class="state"><h4>${ic('search')}搜不到 / 类别是空的</h4>
      <div class="box">
        <label class="search" style="width:100%">${ic('search')}
          <input placeholder="weekly-reprot（拼错一个字母）"></label>
        <div class="d">没搜到「weekly-reprot」—— <b>搜索是全段搜、忽略当前类别</b>，
          所以只给一句「换个词」，不提示「这个类别里没有」。</div>
        <div class="d" style="border-top:1px solid var(--line-soft);padding-top:7px">
          空类别不藏：<span class="chip">实验</span> <span class="chip">Basics · 234</span>
          灰着留着 —— 藏了你就不知道自己该往哪儿归类。</div>
      </div></div>
    </div>
  </div></div>`;
}

/* ── 拼起来 ───────────────────────────────────────────────── */
/* 筛过之后如果选中那条已经不在列表里，就选第一条 —— 别让左边列表和右边详情互相打脸。 */
function syncSelection() {
  const tab = S.tab;
  const rows = tab === 'skills' ? skillsRows() : tab === 'prompts' ? promptsRows() : filteredExtRows();
  if (!rows.length) return;
  const nameOf = (r) => r.skill?.name || r.prompt?.name || r.ext?.name;
  if (!rows.some((r) => nameOf(r) === selOf(tab))) S.sel[tab] = nameOf(rows[0]);
}

function render() {
  const sc = screenOf();
  document.title = `NousDesk · 资源页 · ${sc.id} ${sc.name}`;
  syncSelection();
  const app = document.getElementById('app');
  app.dataset.skin = S.skin;
  let body = sc.states ? statesHTML() : sc.edit ? editHTML() : mainHTML();
  if (!sc.edit && !sc.states) body = `<div class="page">${treeHTML(FIXED)}${listHTML(FIXED)}${withEdit(
    S.tab === 'skills' ? detailSkill(selOf('skills'))
    : S.tab === 'prompts' ? detailPrompt(selOf('prompts'))
    : extDetail(selOf('ext')))}</div>`;
  app.innerHTML = railHTML() + body
    + (sc.sheet ? (sc.sheet === 'publish' ? publishHTML() : formHTML(sc.sheet)) : '')
    + (S.menu ? menuHTML() : '');
  const q = document.getElementById('q');
  if (q) q.addEventListener('input', (ev) => { S.q = ev.target.value; render(); });
  if (sc.sheet === 'publish') bindPublish(); else if (sc.sheet) bindForm(sc.sheet);
}

function menuHTML() {
  return `<div class="scrim" data-act="menu-off"></div>
    <div class="menu">
      <div class="gl">应用</div>
      <div class="it" data-act="noop">${ic('settings')}设置<span class="kbd">Ctrl+,</span></div>
      <div class="it" data-act="noop">${ic('archive')}归档库</div>
      <div class="gl">连接</div>
      <div class="it" data-act="noop">${ic('refresh')}重新连接</div>
      <div class="it" data-act="noop">${ic('folder')}打开配置文件夹</div>
      <div class="sep"></div>
      <div class="info"><span>版本</span><span class="v">v0.8.0</span></div>
      <div class="info"><span>引擎</span><span class="v">node v24.10.0</span></div>
    </div>`;
}

function bindPublish() {
  document.querySelectorAll('[data-act^="publish-repo:"]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const key = btn.dataset.act.split(':')[1];
      const name = { skills: 'Nous-skills', prompts: 'Nous-prompts', ext: 'Nous-extend' }[key];
      const steps = key === 'ext'
        ? ['词表 / 清单', 'commit', 'push']
        : ['内容闸', '重扫 index.json', 'commit', 'push'];
      let i = 0;
      btn.disabled = true;
      btn.textContent = `${name} · ${steps[0]}…`;
      const tick = () => {
        i += 1;
        if (i < steps.length) { btn.textContent = `${name} · ${steps[i]}…`; setTimeout(tick, 420); return; }
        btn.textContent = '已发布';
        toast(`${name} 推上去了 · ${short(D[key === 'ext' ? 'exts' : key].repo.head)} → origin/main`);
      };
      setTimeout(tick, 420);
    });
  });
}

function listBodyOnly() {
  const tab = S.tab;
  let rows;
  if (tab === 'skills') rows = skillsRows().map((r) => rowHTML('skill', r, selOf('skills') === (r.skill?.name || r.name)));
  else if (tab === 'prompts') rows = promptsRows().map((r) => rowHTML('prompt', r, selOf('prompts') === (r.prompt?.name || r.name)));
  else rows = filteredExtRows().map((r) => rowHTML('ext', r, selOf('ext') === r.ext.name));
  return rows.join('') || `<div class="note">${S.q ? `没搜到「${esc(S.q)}」` : '这一类是空的'}</div>`;
}

/* ── 交互 ─────────────────────────────────────────────────── */
document.addEventListener('click', (e) => {
  const el = e.target.closest('[data-act]');
  if (!el) return;
  const act = el.dataset.act;

  if (act.startsWith('rail:')) {
    const id = act.slice(5);
    if (id === 'res') { go(S.tab === 'skills' ? 'skills' : S.tab === 'prompts' ? 'prompts' : 'ext'); }
    else toast('这一份 demo 只画资源页');
    return;
  }
  if (act.startsWith('seg:')) {
    const id = act.slice(4);
    S.tab = id; S.cat = {}; S.q = '';
    const sc = screenOf();
    if (sc.sheet) go(id); else { S.screen = id; history.replaceState(null, '', '#' + id); render(); }
    return;
  }
  if (act.startsWith('cat:')) { setCat(act.slice(4)); return; }
  if (act.startsWith('sel:')) {
    const name = act.slice(4);
    S.sel[S.tab] = name;
    render(); return;
  }
  if (act.startsWith('file:')) {
    S.openFile[act.split(':')[1]] = act.split(':').slice(2).join(':');
    render(); return;
  }
  if (act.startsWith('publish-repo:')) return;   // bindPublish 自己处理

  switch (act) {
    case 'menu': S.menu = !S.menu; render(); return;
    case 'menu-off': S.menu = false; render(); return;
    case 'close': {
      const sc = screenOf();
      go(sc.tab || S.tab);
      return;
    }
    case 'new': go(S.tab === 'skills' ? 'new-skill' : S.tab === 'prompts' ? 'new-prompt' : 'new-ext'); return;
    case 'publish': go('publish'); return;
    case 'edit': go('edit'); return;
    case 'back': go(S.tab === 'skills' ? 'skills' : S.tab === 'prompts' ? 'prompts' : 'ext'); return;
    case 'create:skill': createFrom('skill'); return;
    case 'create:prompt': createFrom('prompt'); return;
    case 'create:ext': createFrom('ext'); return;
    case 'retry': toast('又拉了一次 · 还是连不上（错了就该看得见，别装作没事）'); return;
    case 'rescan': toast('重新扫了一遍 · 名册里 5 条，变体里 1 条加载失败'); return;
    case 'save': toast('存到盘上了 —— 还没发布'); return;
    case 'setdefault': toast(`已把 ${selOf('prompts')} 设为默认 —— 引擎会校验这条真存在`); return;
    case 'install': toast('从 data/extend-trash/ 搬回来了 · 重新扫过一遍'); return;
    case 'uninstall': toast(`${selOf('ext')} 已从本机卸掉 · 仓库没动`); return;
    case 'archive': toast('归档 = git mv 到 .archive/ + commit + push（这一份 demo 只弹提示）'); return;
    case 'drop': toast('拆掉变体树 · 删目录 + 删分支'); return;
    case 'noop': toast('这一下是演示，没接真接口'); return;
  }
});

/* 挑镜头那一列 */
const plist = document.getElementById('plist');
plist.innerHTML = SCREENS.map((s, i) =>
  `<button data-screen="${s.id}" class="${s.id === S.screen ? 'on' : ''}"><b>${String(i + 1).padStart(2, '0')}</b><span>${s.name}</span></button>`).join('');
function syncNote() {
  const sc = screenOf();
  document.getElementById('pnote').innerHTML = sc.note;
  [...plist.children].forEach((b) => b.classList.toggle('on', b.dataset.screen === S.screen));
}
plist.addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (!b) return;
  go(b.dataset.screen);
  syncNote();
});
document.querySelectorAll('.skinbar [data-seg]').forEach((b) => b.addEventListener('click', () => {
  S.segStyle = b.dataset.seg;
  document.querySelectorAll('[data-seg]').forEach((x) => x.classList.toggle('on', x === b));
  render();
}));
document.querySelectorAll('.skinbar [data-skin]').forEach((b) => b.addEventListener('click', () => {
  S.skin = b.dataset.skin;
  document.querySelectorAll('[data-skin]').forEach((x) => x.classList.toggle('on', x === b));
  render();
}));
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    const sc = screenOf();
    if (S.menu) { S.menu = false; render(); }
    else if (sc.sheet) go(sc.tab || S.tab);
    else if (sc.edit || sc.states) go(S.tab === 'skills' ? 'skills' : S.tab === 'prompts' ? 'prompts' : 'ext');
  }
});
window.addEventListener('hashchange', () => {
  const h = location.hash.slice(1);
  if (SCREENS.some((s) => s.id === h) && h !== S.screen) { S.screen = h; const sc = screenOf(); if (sc.tab) S.tab = sc.tab; syncNote(); render(); }
});

/* 缩放 */
const scaler = document.getElementById('scaler');
function fit() {
  const stage = document.querySelector('.stage');
  const s = Math.max(0.4, Math.min(1, (stage.clientWidth - 44) / 1280, (stage.clientHeight - 44) / 800));
  scaler.style.width = (1280 * s) + 'px';
  scaler.style.height = (800 * s) + 'px';
  document.getElementById('app').style.transform = 'scale(' + s + ')';
}
window.addEventListener('resize', fit);

syncNote();
render();
fit();
"""

out = "\n".join([
    head.replace("<title>NousDesk · 技能 / 提示词 / 扩展 · 五套</title>",
                 "<title>NousDesk · 资源页 · 02 落地稿</title>")
        + "\n<style>" + CSS + "</style>\n</head>",
    body_sprite,
    PICKER,
    STAGE,
    '<script src="data.js"></script>',
    '<script>',
    helpers,
    tab_meta,
    trees,
    lists,
    toast,
    JS,
    '</script>',
    '</body>',
    '</html>',
])

(here / "index.html").write_text(out, encoding="utf-8")
print("index.html", len(out.encode()), "bytes")
