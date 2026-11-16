/* 资源页 03 · 交互原型
 *
 * 这不是截图拼的对照稿：真三栏、真状态机，点段/点行/改输入框/新建/保存/发布都真的响应。
 *
 * 三条跟 Rust 对齐的硬规矩（demo 跟真机不一致就等于没做）：
 *   1. 类别树 / 计数 / 排序 = crates/nousdesk/src/category.rs 的 build_tree + is_under；
 *      计数含后代，同一条目按祖先去重。
 *   2. 段计数 = resources.rs::counts —— 技能不算隐藏的，提示词全算，扩展只算 catalog。
 *   3. 校验文案 = 引擎原话（skills-api.js / prompts-api.js / server.js 的 checkName 与
 *      createExtension），不在这里泛化成「输入不合法」。
 *
 * 纯函数挂在 window.P 上，`build-preview.py --test` 会用 node 跑一遍全组合。
 */
(function () {
  'use strict';

  // ── 数据 ──────────────────────────────────────────────────────────────

  const D = (typeof window !== 'undefined' && window.D) || globalThis.D;

  const CORE_EXT = ['nous', 'skills', 'prompts'];
  const NAME_RE = /^[a-z0-9][a-z0-9._-]*$/;
  const MAX_NAME = 64;

  const SKILL_RESERVED = ['categories.json', 'index.json', 'index.mjs', 'nous.json', 'SKILL.md', 'README.md', 'sources.json'];
  const PROMPT_RESERVED = ['categories.json', 'index.json', 'index.mjs', 'settings.json', 'PROMPT.md', 'nous.json', 'README.md'];

  const SKELETON_JS = `// __NAME__ —— __SUMMARY__
//
// 一个扩展 = 一个目录 + 一份清单（nous.json）+ 一个入口（这个文件）。
// 现在它是空的：只声明名字，什么都不装。装载不会红，但它也什么都不会做。
//
// 让它干活，从这三件事里挑一件开始：
//   1. 往系统提示词里加一段（sections）
//   2. 加工具（tools）
//   3. 要宿主的东西（nous.json 的 needs）
//
// 写完点「发布」：先过 node --check 和 index.mjs，才 commit + push。
export default ({ defineExtension }) => defineExtension({ name: "__NAME__" });
`;

  // ── 纯函数：类别树（照 category.rs） ────────────────────────────────

  function ancestors(path) {
    const parts = String(path).split('/').filter(Boolean);
    return parts.map((_, i) => parts.slice(0, i + 1).join('/'));
  }

  function isUnder(value, root) {
    return value === root || String(value).startsWith(root + '/');
  }

  function lastSeg(path) {
    const p = String(path).split('/');
    return p[p.length - 1] || path;
  }

  function depthOf(path) {
    return Math.max(0, String(path).split('/').filter(Boolean).length - 1);
  }

  /**
   * 词表 + 每条目的类别 → 缩进树（含后代计数、同条目去重）。
   * 顺序跟词表；词表里没有的补在后面，按「浅的在前、同层字面」。
   */
  function buildTree(categories, itemCats) {
    const order = [];
    const seen = new Set();
    for (const c of categories) {
      for (const a of ancestors(c)) {
        if (!seen.has(a)) { seen.add(a); order.push(a); }
      }
    }
    const counts = new Map(order.map((p) => [p, 0]));
    const extra = new Set();
    for (const cats of itemCats) {
      const hit = new Set();
      for (const c of cats) for (const a of ancestors(c)) hit.add(a);
      for (const a of hit) {
        if (!counts.has(a)) { counts.set(a, 0); extra.add(a); }
        counts.set(a, counts.get(a) + 1);
      }
    }
    const tail = [...extra].sort((a, b) => depthOf(a) - depthOf(b) || a.localeCompare(b, 'zh-CN'));
    const paths = [...order.filter((p) => !extra.has(p)), ...tail];

    return paths.map((path) => {
      const hasChildren = paths.some((p) => p !== path && isUnder(p, path));
      const node = { path, label: lastSeg(path), depth: depthOf(path), count: counts.get(path) || 0 };
      node.hasChildren = hasChildren;
      return node;
    });
  }

  // ── 纯函数：行（照 resources.rs 的三个 *_rows） ─────────────────────

  function metaOf(row) { return row.extKind ? row.extKind : null; }

  function normalize() {
    const st = { skills: [], prompts: [], ext: [], archivedPrompt: [] };

    // 技能：隐藏的算 hidden，变体各占一行
    const skillItems = D.skills.items.map((s) => ({
      kind: 'skills',
      name: s.name,
      desc: s.description || '',
      category: s.category || [],
      hidden: !!s.hidden,
      tracked: !!s.tracked,
      fileCount: s.files || 2,
      variant: false,
      loadError: null,
      version: null,
      sections: [],
      needs: {},
    }));
    for (const [base, list] of Object.entries(D.skills.variants || {})) {
      for (const v of list) {
        skillItems.push({
          kind: 'skills',
          name: v.key,
          desc: `「${base}」的变体（分支 ${v.branch}）`,
          category: [],
          hidden: false,
          tracked: false,
          fileCount: 2,
          variant: true,
          loadError: v.load && v.load.state !== 'ok' ? v.load.error : null,
          version: null,
          sections: [],
          needs: {},
        });
      }
    }
    st.skills = skillItems;

    // 提示词
    st.prompts = D.prompts.items.map((p) => ({
      kind: 'prompts',
      name: p.name,
      desc: p.summary || p.description || '',
      category: p.category || [],
      hidden: false,
      tracked: !!p.tracked,
      isDefault: D.prompts.default === p.name,
      fileCount: 2,
      variant: false,
      loadError: null,
      version: null,
      sections: [],
      needs: {},
    }));

    // 扩展：五类来源归一成一行，但 kind 只用于画 chip
    const installed = new Set(D.exts.installed || []);
    const load = D.exts.load || {};
    const rows = [];
    for (const c of D.exts.catalog || []) {
      const bad = load[c.name] && load[c.name].state === 'failed';
      rows.push({
        kind: 'ext',
        name: c.name,
        desc: c.summary || '',
        category: c.category || [],
        extKind: !installed.has(c.name) ? 'off' : bad ? 'failed' : 'on',
        tracked: !!c.tracked,
        version: c.version || null,
        sections: c.sections || [],
        needs: c.needs || {},
        loadError: bad ? load[c.name].error : null,
        fileCount: (D.exts.files && D.exts.files[c.name] || []).length || 2,
        variant: false,
      });
    }
    for (const name of D.exts.builtin || []) {
      rows.push({
        kind: 'ext', name, desc: '引擎自带的扩展，不在仓库里',
        category: [], extKind: 'builtin', tracked: true, version: null,
        sections: [], needs: {}, loadError: null, fileCount: 0, variant: false,
      });
    }
    for (const [base, list] of Object.entries(D.exts.variants || {})) {
      for (const v of list) {
        const bad = v.load && v.load.state !== 'ok';
        rows.push({
          kind: 'ext', name: v.key,
          desc: `「${base}」的本机变体（${v.dir}）`,
          category: [], extKind: bad ? 'failed' : 'variant',
          tracked: false, version: v.load && v.load.version || null,
          sections: [], needs: {}, loadError: bad ? v.load.error : null,
          fileCount: 2, variant: true,
        });
      }
    }
    for (const a of D.exts.archived || []) {
      rows.push({
        kind: 'ext', name: a.name, desc: a.summary || '',
        category: a.category || [], extKind: 'archived', tracked: false,
        version: a.version || null, sections: [], needs: {},
        loadError: null, fileCount: 0, variant: false,
      });
    }
    st.ext = rows;
    return st;
  }

  const BASE = normalize();

  function visibleSkills(skills, cat, q) {
    const out = skills.filter((s) => {
      if (cat === '@hidden') return s.hidden;
      if (s.hidden) return false;
      if (!cat) return true;
      if (cat === '∅') return s.category.length === 0;
      return s.category.some((c) => isUnder(c, cat));
    });
    return out
      .filter((s) => matches(s, q))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function promptRows(prompts, cat, q) {
    return prompts
      .filter((p) => {
        if (!cat) return true;
        if (cat === '∅') return p.category.length === 0;
        if (cat === '@default') return p.isDefault;
        return p.category.some((c) => isUnder(c, cat));
      })
      .filter((p) => matches(p, q))
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  function extRows(rows, cat, q) {
    return rows
      .filter((r) => {
        if (!cat) return true;
        if (cat === '∅') return r.category.length === 0;
        if (cat === '@on') return r.extKind === 'on';
        if (cat === '@bad') return r.extKind === 'failed';
        if (cat === '@builtin') return r.extKind === 'builtin';
        if (cat === '@variant') return r.variant;
        if (cat === '@archived') return r.extKind === 'archived';
        return r.category.some((c) => isUnder(c, cat));
      })
      .filter((r) => matches(r, q))
      .sort((a, b) => {
        const rank = (x) => ({ on: 0, builtin: 1, failed: 2, off: 3, archived: 4 })[x] ?? 5;
        return rank(a.extKind) - rank(b.extKind) || a.name.localeCompare(b.name);
      });
  }

  function matches(row, q) {
    const t = String(q || '').trim().toLowerCase();
    if (!t) return true;
    return row.name.toLowerCase().includes(t) || String(row.desc || '').toLowerCase().includes(t);
  }

  function counts(st) {
    return {
      skills: st.skills.filter((s) => !s.hidden && !s.variant).length,
      prompts: st.prompts.length,
      ext: st.ext.filter((r) => !r.variant && r.extKind !== 'archived' && r.extKind !== 'builtin').length,
    };
  }

  // ── 纯函数：新建校验（引擎原话） ───────────────────────────────────

  function checkName(raw, kind) {
    const name = String(raw || '').trim();
    const what = kind === 'skills' ? '技能' : kind === 'prompts' ? '模板' : '扩展';
    if (!name) return `${what}名不能为空`;
    if (kind === 'ext') {
      if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) return `扩展名不合法：${name}`;
      if (CORE_EXT.includes(name)) {
        return `${name} 是引擎启动路径上硬编码 import 的核心扩展，不能动`;
      }
      return null;
    }
    if (name.length > MAX_NAME) return `${what}名太长（最多 ${MAX_NAME} 字）：${name}`;
    if (!NAME_RE.test(name)) {
      return `${what}名只认小写字母、数字、点、下划线、连字符，且以字母或数字开头：${name}`;
    }
    const reserved = kind === 'skills' ? SKILL_RESERVED : PROMPT_RESERVED;
    if (reserved.includes(name)) return `「${name}」是仓库自己的文件名，不能当${what}名`;
    return null;
  }

  /** 新建时的校验：返回 { field, msg }[]，顺序跟表单顺序一致。 */
  function validateNew(form, all) {
    const errs = [];
    const nameErr = checkName(form.name, form.kind);
    if (nameErr) errs.push({ field: 'name', msg: nameErr });

    if (form.kind === 'skills' && !String(form.description || '').trim()) {
      errs.push({ field: 'description', msg: '技能名: description 不能空 —— 模型靠它判断该不该用这条技能' });
    }
    if (form.kind === 'prompts' && !String(form.summary || '').trim()) {
      errs.push({ field: 'summary', msg: '模板名: summary 不能空 —— 人靠这一行决定选不选它' });
    }
    if (form.kind === 'ext') {
      const s = String(form.summary || '').trim();
      if (!s) errs.push({ field: 'summary', msg: '一句话说明不能空 —— 卡片上就靠它认人' });
      else if (s.length > 200) errs.push({ field: 'summary', msg: '说明太长了（200 字以内）' });
    }

    // 撞名三连：工作区 / 归档区
    const name = String(form.name || '').trim();
    if (name && !nameErr) {
      const pool = form.kind === 'skills' ? all.skills : form.kind === 'prompts' ? all.prompts : all.ext;
      const exists = pool.some((r) => r.name === name);
      if (exists) {
        errs.push({
          field: 'name',
          msg: form.kind === 'ext'
            ? `${name}/ 已经在了（清单里可能叫别的）`
            : `${name}/ 已经在了 —— 换个名字，或者直接去改它`,
        });
      } else if (form.kind !== 'ext' && form.kind === 'skills' && isUnder(form.group || '', name)) {
        // 占位：分组不在这里做
      }
      const archived = form.kind === 'ext' && (D.exts.archived || []).some((a) => a.name === name);
      if (archived) {
        errs.push({
          field: 'name',
          msg: `.archive/${name} 还留着 —— 点「已删除」里的「恢复」把它捡回来，别建第二个`,
        });
      }
    }
    return errs;
  }

  /** 落盘文件的真实形状（照引擎 createSkill / createPrompt / createExtension）。 */
  function landingFiles(form) {
    const name = String(form.name || '').trim() || '<name>';
    const cats = String(form.category || '')
      .split(/[,，\s]+/)
      .map((s) => s.trim())
      .filter(Boolean);
    const dir = (root) => `${root}/${name}/`;

    if (form.kind === 'skills') {
      const desc = String(form.description || '').trim() || '<description>';
      const front = [
        '---',
        `name: ${name}`,
        `description: ${JSON.stringify(desc)}`,
        '---',
        '',
        `# ${name}`,
        '',
      ].join('\n');
      return [
        { path: dir(D.skills.root) + 'SKILL.md', text: front },
        {
          path: dir(D.skills.root) + 'nous.json',
          text: JSON.stringify({ name, summary: '', category: cats }, null, 2) + '\n',
        },
      ];
    }
    if (form.kind === 'prompts') {
      const body = String(form.body || '').trim() ? form.body : `# ${name}\n`;
      return [
        { path: dir(D.prompts.root) + 'PROMPT.md', text: body },
        {
          path: dir(D.prompts.root) + 'nous.json',
          text: JSON.stringify({ name, summary: String(form.summary || '').trim(), category: cats }, null, 2) + '\n',
        },
      ];
    }
    const summary = String(form.summary || '').trim() || '<一句话说明>';
    const manifest = {
      name,
      version: '0.1.0',
      file: 'index.js',
      entry: 'default',
      sections: [],
      summary,
      needs: {},
      ...(cats.length ? { category: cats } : {}),
    };
    return [
      { path: dir(D.exts.root) + 'nous.json', text: JSON.stringify(manifest, null, 2) + '\n' },
      {
        path: dir(D.exts.root) + 'index.js',
        text: SKELETON_JS.replace(/__NAME__/g, name).replace(/__SUMMARY__/g, summary.replace(/\n/g, ' ')),
      },
    ];
  }

  // ── 文件树（真数据优先，缺的按仓库约定补，README 里有说明） ────────

  const EXTRA_BY_COUNT = {
    skills: ['references/notes.md', 'references/examples.md', 'assets/checklist.md'],
    prompts: [],
    ext: ['README.md'],
  };

  function filesOf(row) {
    const known = (D[row.kind === 'ext' ? 'exts' : row.kind].files || {})[row.name] || [];
    const texts = (D[row.kind === 'ext' ? 'exts' : row.kind].text || {});
    const out = known.map((f) => ({
      path: f.path,
      size: f.size,
      text: texts[`${row.name}/${f.path}`] != null ? texts[`${row.name}/${f.path}`] : null,
    }));
    const have = new Set(out.map((f) => f.path));
    const primary = row.kind === 'skills' ? 'SKILL.md' : row.kind === 'prompts' ? 'PROMPT.md' : 'index.js';
    for (const p of [primary, 'nous.json']) {
      if (out.length >= 2 && !have.has(p) && out.length < (row.fileCount || 2)) {
        // 只有凑数时才补：优先把主文件摆出来
      }
    }
    if (out.length === 0) {
      out.push({ path: primary, size: null, text: texts[`${row.name}/${primary}`] ?? null });
      out.push({ path: 'nous.json', size: null, text: null });
    }
    // 正文排在清单前面：打开编辑器先看内容，不是先看元数据
    out.sort((x, y) => (x.path === 'nous.json' ? 1 : 0) - (y.path === 'nous.json' ? 1 : 0));
    const extra = EXTRA_BY_COUNT[row.kind] || [];
    for (const p of extra) {
      if (out.length >= (row.fileCount || 2)) break;
      if (!have.has(p)) out.push({ path: p, size: null, text: null });
    }
    return out;
  }

  /** 没有真文本时，按引擎的落盘形状合成一份 —— 不让预览空着。 */
  function synthText(row, path) {
    const cats = row.category || [];
    if (path === 'nous.json') {
      const manifest = row.kind === 'ext'
        ? { name: row.name, version: row.version || '0.1.0', file: 'index.js', entry: 'default', sections: row.sections || [], summary: row.desc || '', needs: row.needs || {}, ...(cats.length ? { category: cats } : {}) }
        : { name: row.name, summary: row.kind === 'skills' ? '' : (row.desc || ''), category: cats };
      return JSON.stringify(manifest, null, 2) + '\n';
    }
    if (path === 'SKILL.md') {
      return `---\nname: ${row.name}\ndescription: ${JSON.stringify(row.desc || '')}\n---\n\n# ${row.name}\n\n（正文还没拉过 —— 点「编辑」才取文件内容。）\n`;
    }
    if (path === 'PROMPT.md') return `# ${row.name}\n\n（正文还没拉过 —— 点「编辑」才取文件内容。）\n`;
    if (path === 'index.js') {
      return `// ${row.name} —— ${row.desc || ''}\n//\n// 入口文件。demo 里只摆形状，真实内容在编辑器里取。\nexport default ({ defineExtension }) => defineExtension({ name: "${row.name}" });\n`;
    }
    return `（${path} 的内容 demo 里没取，README 写了原因。）\n`;
  }

  // ── 状态 ────────────────────────────────────────────────────────────

  const S = {
    section: 'content',       // content | ext
    content: 'skills',        // skills | prompts
    cat: { skills: null, prompts: null, ext: null },
    q: { skills: '', prompts: '', ext: '' },
    sel: null,                // { kind, name }
    view: 'browse',           // browse | new | edit | publish
    edit: null,               // { row, files, active, saved: {}, dirty }
    form: null,               // 新建表单
    formErrors: [],
    flash: null,              // { tone, text }
    published: {},            // 哪个仓刚发布过
    publishError: {},         // 哪个仓发布失败（引擎原文）
    loading: false,
    repoError: { skills: null, prompts: null, ext: null },
    dialog: null,             // { title, text, confirm, tone, onYes }
    savedRows: null,          // 新建出来的条目（内存里，刷新就没了）
    fileView: null,           // 详情里选中的文件 path
  };

  let st = cloneData(BASE);

  function cloneData(x) {
    return JSON.parse(JSON.stringify(x));
  }

  function rowsNow() {
    return {
      skills: st.skills,
      prompts: st.prompts,
      ext: st.ext,
    };
  }

  function activeKind() {
    return S.section === 'ext' ? 'ext' : S.content;
  }

  function slotKey() {
    return activeKind();
  }

  function currentRows() {
    const k = activeKind();
    const cat = S.cat[k];
    const q = S.q[k];
    if (k === 'ext') return extRows(st.ext, cat, q);
    if (k === 'prompts') return promptRows(st.prompts, cat, q);
    return visibleSkills(st.skills, cat, q);
  }

  function findRow(kind, name) {
    const pool = kind === 'ext' ? st.ext : st[kind];
    return pool.find((r) => r.name === name) || null;
  }

  // ── 渲染工具 ───────────────────────────────────────────────────────

  const esc = (s) => String(s == null ? '' : s)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

  const svg = {
    chat: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 8.9 8.9 0 0 1-4-.9L3 21l1.9-4.6A8.4 8.4 0 0 1 12 3.1a8.4 8.4 0 0 1 9 8.4z"/></svg>',
    tpl: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3.5" y="3.5" width="17" height="17" rx="2.5"/><path d="M3.5 9h17M9 9v11.5"/></svg>',
    bolt: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M13 2 4.5 13.5H11L10 22l8.5-11.5H12L13 2z"/></svg>',
    layers: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="m12 2.5 9 5-9 5-9-5 9-5z"/><path d="m3 12.5 9 5 9-5M3 17l9 5 9-5"/></svg>',
    db: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><ellipse cx="12" cy="5.5" rx="8" ry="3"/><path d="M4 5.5v13c0 1.7 3.6 3 8 3s8-1.3 8-3v-13"/><path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/></svg>',
    search: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 4.5 4.5"/></svg>',
    gear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-1.8-.3 1.6 1.6 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1A1.6 1.6 0 0 0 9 19.4a1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0 .3-1.8 1.6 1.6 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1A1.6 1.6 0 0 0 4.6 9a1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.6 1.6 0 0 0 9 4.6 1.6 1.6 0 0 0 10 3.1V3a2 2 0 1 1 4 0v.1a1.6 1.6 0 0 0 1 1.5 1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0-.3 1.8 1.6 1.6 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1z"/></svg>',
  };

  function pipFor(kind) {
    // 三个仓各一个点：琥珀 = 有未提交改动，红 = 有条目加载失败
    if (kind === 'skills') {
      const bad = st.skills.some((s) => s.loadError);
      const dirty = st.skills.some((s) => !s.tracked && !s.variant);
      if (bad) return '<i class="pip bad"></i>';
      if (dirty) return '<i class="pip dirty"></i>';
      return '';
    }
    if (kind === 'prompts') {
      const bad = st.prompts.some((p) => p.loadError);
      const dirty = st.prompts.some((p) => !p.tracked);
      if (bad) return '<i class="pip bad"></i>';
      if (dirty) return '<i class="pip dirty"></i>';
      return '';
    }
    const bad = st.ext.some((r) => r.extKind === 'failed');
    const dirty = D.exts.repo && D.exts.repo.drift;
    if (bad) return '<i class="pip bad"></i>';
    if (dirty) return '<i class="pip dirty"></i>';
    return '';
  }

  // ── 渲染：外壳 ─────────────────────────────────────────────────────

  function renderRail() {
    const item = (id, icon, on, dot) =>
      `<button data-nav="${id}" class="${on ? 'on' : ''}" title="${id}">${icon}${dot ? '<i class="dot"></i>' : ''}</button>`;
    return `<nav class="rail">
      <div class="logo">N</div>
      ${item('chats', svg.chat, false, false)}
      ${item('templates', svg.tpl, false, false)}
      ${item('triggers', svg.bolt, false, false)}
      ${item('resources', svg.layers, true, true)}
      ${item('data', svg.db, false, false)}
      <div class="spacer"></div>
      ${item('settings', svg.gear, false, false)}
    </nav>`;
  }

  function renderTop() {
    return `<header class="topbar">
      <h1>资源</h1>
      <span class="sub">${(() => { const c = counts(st); return `${c.skills + c.prompts} 条技能 / 提示词 · ${c.ext} 条扩展`; })()}</span>
      <div class="spacer"></div>
      <button class="btn ghost sm" data-act="reload">刷新</button>
      <button class="btn ghost sm" data-act="toggle-shots">镜头 <span class="mono" style="opacity:.6">?</span></button>
    </header>`;
  }

  function renderSeg() {
    const c = counts(st);
    const segBtn = (key, label, n, pip) => {
      const on = (key === 'ext') === (S.section === 'ext');
      return `<button data-seg="${key}" class="${on ? 'on' : ''}">${label}<span class="n">${n}</span>${pip}</button>`;
    };
    const subBtn = (key, label, n) => {
      const on = S.section === 'content' && S.content === key;
      return `<button data-sub="${key}" class="${on ? 'on' : ''}">${label}<span class="n">${n}</span>${pipFor(key)}</button>`;
    };
    return `<div class="seg">
        ${segBtn('content', '内容', c.skills + c.prompts, '')}
        ${segBtn('ext', '扩展', c.ext, pipFor('ext'))}
      </div>
      ${S.section === 'content'
        ? `<div class="sub">
             ${subBtn('skills', '技能', c.skills)}
             ${subBtn('prompts', '提示词', c.prompts)}
           </div>`
        : '<div style="height:6px"></div>'}`;
  }

  function renderTree() {
    const k = activeKind();
    if (S.repoError[k]) {
      return `<div class="col-tree">
        ${renderSeg()}
        <div class="tree-empty">这一仓没拉到，类别树也没了。<br>点中间那列「重试这一仓」。</div>
      </div>`;
    }
    if (S.loading) {
      return `<div class="col-tree">${renderSeg()}${renderSearch(k)}
        <div class="tree-empty">正在拉 /v1/${k === 'ext' ? 'extensions' : k} …</div></div>`;
    }
    const cat = S.cat[k];
    let nodes = [];
    let special = [];

    if (k === 'ext') {
      nodes = buildTree(D.exts.categories || [], st.ext.map((r) => r.category));
      special = [
        ['@on', '启用中', st.ext.filter((r) => r.extKind === 'on').length],
        ['@bad', '加载失败', st.ext.filter((r) => r.extKind === 'failed').length],
        ['@builtin', '内置', st.ext.filter((r) => r.extKind === 'builtin').length],
        ['@variant', '变体', st.ext.filter((r) => r.variant).length],
        ['@archived', '已归档', st.ext.filter((r) => r.extKind === 'archived').length],
      ];
    } else if (k === 'skills') {
      const real = st.skills.filter((r) => !r.variant);
      nodes = buildTree(D.skills.categories || [], real.map((r) => r.category));
      special = [
        ['∅', '未分类', real.filter((s) => !s.hidden && s.category.length === 0).length],
        ['@hidden', '隐藏（模型看不见）', real.filter((s) => s.hidden).length],
      ];
    } else {
      nodes = buildTree(D.prompts.categories || [], st.prompts.map((r) => r.category));
      special = [
        ['∅', '未分类', st.prompts.filter((p) => p.category.length === 0).length],
        ['@default', `默认  ${D.prompts.default || '—'}`, null],
      ];
    }

    const allOn = cat === null;
    let html = `<button class="trow ${allOn ? 'on' : ''}" data-cat="">
        <span class="tw"></span><span class="lbl">全部</span>
        <span class="n">${currentKindTotal(k)}</span>
      </button>`;

    for (const n of nodes) {
      html += `<button class="trow ${cat === n.path ? 'on' : ''}" data-cat="${esc(n.path)}" style="padding-left:${8 + n.depth * 13}px">
        <span class="tw"></span>
        <span class="lbl">${esc(n.label)}</span>
        <span class="n">${n.count}</span>
      </button>`;
    }
    if (special.length) {
      html += '<div class="trow sep" style="padding:0"></div>';
      for (const [key, label, n] of special) {
        html += `<button class="trow special ${cat === key ? 'on' : ''}" data-cat="${esc(key)}">
          <span class="tw"></span><span class="lbl">${esc(label)}</span>
          ${n == null ? '' : `<span class="n">${n}</span>`}
        </button>`;
      }
    }
    if (!nodes.length && !special.length) {
      html += '<div class="tree-empty">这一仓还没有类别。</div>';
    }

    return `<div class="col-tree">
      ${renderSeg()}
      ${renderSearch(k)}
      <div class="tree">${html}</div>
    </div>`;
  }

  function renderSearch(k) {
    const q = S.q[k];
    return `<div class="searchbox">
      ${svg.search}
      <input id="q" data-q="${k}" placeholder="搜名字或摘要" value="${esc(q)}">
      ${q ? '<button class="x" data-act="clear-q">×</button>' : ''}
    </div>`;
  }

  function currentKindTotal(k) {
    if (k === 'ext') return st.ext.length;
    if (k === 'skills') return st.skills.filter((s) => !s.hidden).length;
    return st.prompts.length;
  }

  function renderList() {
    const k = activeKind();
    const rows = currentRows();
    const title = k === 'ext' ? '扩展' : k === 'skills' ? '技能' : '提示词';
    const catLabel = S.cat[k] ? catName(k, S.cat[k]) : '全部';

    const narrow = window.innerWidth < 1080 || S.forceNarrow;
    let compact = '';
    if (narrow) {
      compact = `<div style="padding:8px 10px 0">${renderSeg()}${renderSearch(k)}</div>`;
    }
    let head = `<div class="list-head">
      <span class="t">${title}</span>
      <span class="n">${S.loading ? '加载中…' : `${rows.length} 条 · ${esc(catLabel)}`}</span>
      <span class="spacer"></span>
      ${k === 'ext'
        ? '<button class="btn primary sm" data-act="new">+ 新建</button>'
        : '<button class="btn primary sm" data-act="new">+ 新建</button>'}
    </div>`;

    let body = '';
    if (S.loading) {
      body = Array.from({ length: 5 }, () => `<div class="skel">
        <div class="b a"></div><div style="flex:1"><div class="b l1"></div><div class="b l2"></div></div>
      </div>`).join('');
    } else if (S.repoError[k]) {
      body = `<div class="empty"><b>这一仓没拉到</b>${esc(S.repoError[k])}<br><br>
        <button class="btn ghost sm" data-act="reload">重试这一仓</button></div>`;
    } else if (!rows.length) {
      const hasQuery = !!S.q[k] || !!S.cat[k];
      body = `<div class="empty"><b>${hasQuery ? '这些条件下没有条目' : '这一仓还是空的'}</b>
        ${hasQuery ? '换个类别，或者清掉搜索词。' : '点右上「+ 新建」建第一条。'}</div>`;
    } else {
      body = rows.map((r) => rowHtml(r)).join('');
    }
    return `<div class="col-list">${compact}${head}<div class="rows">${body}</div></div>`;
  }

  function catName(k, key) {
    if (key === '∅') return '未分类';
    if (key === '@hidden') return '隐藏';
    if (key === '@default') return '默认';
    const map = { '@on': '启用中', '@bad': '加载失败', '@builtin': '内置', '@variant': '变体', '@archived': '已归档' };
    return map[key] || key;
  }

  function stateChip(r) {
    if (r.loadError) return '<span class="chip bad">加载失败</span>';
    if (r.kind === 'ext') {
      if (r.extKind === 'archived') return '<span class="chip arch">已归档</span>';
      if (r.extKind === 'builtin') return '<span class="chip">内置</span>';
      if (r.extKind === 'off') return '<span class="chip">本机没装</span>';
      if (r.variant) return '<span class="chip ok">变体</span>';
      return r.tracked ? '' : '<span class="chip dirty">未发布</span>';
    }
    if (r.hidden) return '<span class="chip arch">隐藏</span>';
    if (r.isDefault) return '<span class="chip ok">默认</span>';
    return r.tracked ? '' : '<span class="chip dirty">未发布</span>';
  }

  function rowHtml(r) {
    const on = S.sel && S.sel.kind === r.kind && S.sel.name === r.name;
    const initials = (r.name.replace(/[^a-z0-9]/gi, '')[0] || '?').toUpperCase();
    const avCls = r.loadError ? 'bad'
      : r.extKind === 'archived' ? 'arch'
        : r.extKind === 'off' ? 'off'
          : r.kind === 'ext' ? 'ext' : r.kind === 'skills' ? 'skill' : 'prompt';
    const cats = r.category.slice(0, 2).map((c) => `<span class="chip cat">${esc(lastSeg(c))}</span>`).join('');
    return `<div class="r ${on ? 'on' : ''}" data-row="${esc(r.kind)}|${esc(r.name)}">
      <div class="av ${avCls}">${initials}</div>
      <div class="body2">
        <div class="name"><span class="nm">${esc(r.name)}</span>${stateChip(r)}</div>
        <div class="desc">${esc(r.desc || '（没有说明）')}</div>
        <div class="meta">${cats}</div>
      </div>
    </div>`;
  }

  // ── 渲染：详情 ─────────────────────────────────────────────────────

  function renderDetail() {
    if (!S.sel) {
      return `<div class="col-detail"><div class="dplaceholder">
        选一行看详情。<br>
        新建 / 编辑 / 发布都在这一页，不用切到别的窗口。
      </div></div>`;
    }
    const r = findRow(S.sel.kind, S.sel.name);
    if (!r) return `<div class="col-detail"><div class="dplaceholder">这一条不见了（刷新过了？）</div></div>`;

    const files = filesOf(r);
    const activeFile = S.fileView && files.some((f) => f.path === S.fileView) ? S.fileView : files[0].path;
    const f = files.find((x) => x.path === activeFile);
    const text = (f && f.text) != null ? f.text : synthText(r, activeFile);

    const acts = [];
    acts.push('<button class="btn primary sm" data-act="edit">编辑</button>');
    if (r.loadError) acts.push('<button class="btn ghost sm" data-act="retry">重试装载</button>');
    if (r.kind === 'ext') {
      if (r.extKind === 'archived') {
        acts.push('<button class="btn ghost sm" data-act="restore">恢复</button>');
        acts.push('<button class="btn danger sm" data-act="purge">真删</button>');
      } else if (r.extKind !== 'builtin') {
        if (r.extKind === 'off') acts.push('<button class="btn ghost sm" data-act="install">安装</button>');
        else acts.push('<button class="btn ghost sm" data-act="uninstall">卸载</button>');
        acts.push('<button class="btn ghost sm" data-act="archive">归档</button>');
      }
    } else {
      acts.push('<button class="btn ghost sm" data-act="archive">归档</button>');
    }
    acts.push('<button class="btn ghost sm" data-act="publish">发布</button>');

    const kv = [];
    if (r.kind === 'ext') {
      kv.push(['版本', r.version || '—']);
      kv.push(['类别', r.category.length ? r.category.join(' · ') : '未分类']);
      kv.push(['段', r.sections.length ? r.sections.join('、') : '不注入提示词']);
      kv.push(['needs', Object.keys(r.needs || {}).length ? Object.keys(r.needs).join('、') : '不要宿主的东西']);
      kv.push(['出处', r.extKind === 'builtin' ? '引擎自带（不在仓库里）' : r.tracked ? '仓库里（已提交）' : '未提交']);
    } else {
      kv.push(['类别', r.category.length ? r.category.join(' · ') : '未分类']);
      kv.push(['状态', r.loadError ? '加载失败' : '可加载']);
      kv.push(['出处', r.tracked ? '仓库里（已提交）' : '未提交']);
      kv.push(['文件', `${files.length} 个`]);
      if (r.kind === 'skills') kv.push(['提示词里', r.hidden ? '看不见（隐藏）' : '看得见']);
      if (r.isDefault) kv.push(['默认', '是（settings.json 的 default）']);
    }

    const notes = [];
    if (r.loadError) {
      notes.push(`<div class="note bad">装载失败，引擎原话：<br><code>${esc(r.loadError)}</code><br>
        ${r.kind === 'ext' ? '目录在盘上，所以它还是「装着」的 —— 修好入口文件再点「重试装载」。' : '名册里没有它，多半是树被删了。'}</div>`);
    }
    if (r.extKind === 'builtin') {
      notes.push('<div class="note">内置扩展是引擎硬编码 import 的 —— 卸载会 403，也不在仓库里，发不出去。</div>');
    }
    if (r.kind === 'ext' && CORE_EXT.includes(r.name)) {
      notes.push('<div class="note warn">核心扩展（引擎启动就 import）—— 卸载会 403。<b>发布</b>仍然可以：它是仓库里的文件。</div>');
    }
    if (r.extKind === 'archived') {
      notes.push('<div class="note">在 `.archive/` 里 —— 别人也看不见。「恢复」把它搬回仓库并提交，「真删」会一起提交删掉。</div>');
    }
    if (r.kind === 'ext' && r.name === 'prompts') {
      notes.push('<div class="note">这个扩展每次只能选<b>一条</b>提示词模板（单选）—— 会话里选了它，技能那栏也一样单选。</div>');
    }
    if (r.hidden) notes.push('<div class="note">隐藏：模型看不见它，勾上也不进提示词。</div>');
    if (r.isDefault) notes.push('<div class="note">这是仓库默认值（`settings.json` 的 `default`）—— 只用来预勾，不是「用户选的那条」。</div>');
    if (r.variant) notes.push('<div class="note">本机变体：内容在它自己那条分支上，不是复制。改它不影响本体。</div>');
    if (!r.tracked && !r.variant && r.extKind !== 'builtin' && r.extKind !== 'archived') {
      notes.push('<div class="note warn">还没提交 —— 只在这台机器上看得见。点「发布」才会进别人的视野。</div>');
    }

    return `<div class="col-detail">
      <div class="dscroll">
        <div class="dhead">
          <div class="id">
            <h2>${esc(r.name)}${stateChip(r)}</h2>
            <div class="who">${esc(r.kind === 'ext' ? '扩展' : r.kind === 'skills' ? 'skill' : 'prompt')}:${esc(r.name)}${r.category.length ? ' · ' + esc(r.category.join(' · ')) : ''}</div>
          </div>
          <div class="acts">${acts.join('')}</div>
        </div>
        <div class="dbody">
          ${r.desc ? `<div class="kv"><div class="k">${r.kind === 'skills' ? '描述' : '摘要'}</div><div class="v">${esc(r.desc)}</div></div>` : ''}
          ${kv.map(([k2, v]) => `<div class="kv"><div class="k">${esc(k2)}</div><div class="v">${esc(v)}</div></div>`).join('')}

          <div class="sec">
            <div class="h">文件</div>
            <div class="filelist">
              ${files.map((ff) => `<div class="frow ${ff.path === activeFile ? 'on' : ''}" data-file="${esc(ff.path)}">
                <span class="p mono">${esc(ff.path)}</span>
                <span class="s">${ff.size != null ? ff.size + ' B' : ''}</span>
              </div>`).join('')}
            </div>
            <div class="preview">
              <div class="ph mono">${esc(activeFile)}</div>
              <pre>${highlight(activeFile, text)}</pre>
            </div>
          </div>

          ${notes.join('')}
        </div>
      </div>
    </div>`;
  }

  /** 一点点着色就够，别在这里写 Markdown 解析器。 */
  function highlight(path, text) {
    let t = esc(text);
    if (path.endsWith('.json')) {
      t = t.replace(/"([^"]+)":/g, '<span class="k">"$1"</span>:');
      return t;
    }
    if (path.endsWith('.md')) {
      if (t.startsWith('---')) {
        const end = t.indexOf('\n---', 3);
        if (end > 0) {
          t = `<span class="fm">${t.slice(0, end + 4)}</span>${t.slice(end + 4)}`;
        }
      }
      t = t.replace(/^(#{1,3} .*)$/gm, '<span class="h1">$1</span>');
      return t;
    }
    return t;
  }

  // ── 渲染：新建 ─────────────────────────────────────────────────────

  function renderNew() {
    const fm = S.form;
    const kindLabel = fm.kind === 'skills' ? '技能' : fm.kind === 'prompts' ? '提示词' : '扩展';
    const files = landingFiles(fm);
    const errs = S.formErrors;
    const errOf = (f) => errs.find((e) => e.field === f);

    const field = (id, label, req, hint, input) => {
      const e = errOf(id);
      return `<div class="field ${e ? 'bad' : ''}">
        <label>${label}${req ? '<span class="req">*</span>' : ''}</label>
        ${hint ? `<div class="hint">${hint}</div>` : ''}
        ${input}
        ${e ? `<div class="err">${esc(e.msg)}</div>` : ''}
      </div>`;
    };

    let form = '';
    form += field('name', '名字', true,
      fm.kind === 'ext'
        ? '小写字母 / 数字 / 连字符，建完就是目录名，不能改。核心三条（nous / skills / prompts）不让用。'
        : `正则 <code>^[a-z0-9][a-z0-9._-]*$</code>，≤ 64 字。名字就是目录名，新建那一刻定型。`,
      `<input type="text" id="f-name" data-f="name" value="${esc(fm.name)}" placeholder="weekly-digest" autocomplete="off">`);

    if (fm.kind === 'skills') {
      form += field('description', 'description', true,
        '模型靠这一行判断该不该用这条技能 —— 引擎会拒空的。写进 <code>SKILL.md</code> 的 frontmatter。',
        `<textarea id="f-description" data-f="description" placeholder="把这几天的会议、邮件、聊天整理成一份周报">${esc(fm.description)}</textarea>`);
    }
    if (fm.kind === 'prompts') {
      form += field('summary', 'summary', true,
        '人靠这一行决定选不选它 —— 写进 <code>nous.json</code>。正文进编辑器里写。',
        `<textarea id="f-summary" data-f="summary" placeholder="审查时按标准 + 规格两条轴走，不混成一股感觉">${esc(fm.summary)}</textarea>`);
      form += field('body', '正文（可空）', false,
        '空着就落一个 <code># 名字</code> 的标题 —— 标题也会进提示词。',
        `<textarea id="f-body" data-f="body" class="mono" placeholder="正文会整段进提示词 —— 空着就落一个 # 名字 的标题">${esc(fm.body || '')}</textarea>`);
    }
    if (fm.kind === 'ext') {
      form += field('summary', '一句话说明', true,
        '卡片上就靠它认人 —— 必填，≤ 200 字。写进 <code>nous.json</code> 的 <code>summary</code>。',
        `<textarea id="f-summary" data-f="summary" placeholder="把这几天的提交整理成一张能贴出去的图">${esc(fm.summary)}</textarea>`);
      form += `<div class="field"><label>说明长度</label>
        <div class="hint">${String(fm.summary || '').trim().length} / 200</div></div>`;
    }

    form += field('category', '类别（逗号分隔，可空）', false,
      `写进 <code>nous.json</code> 的 <code>category</code>，父类别不存在会自动补进词表。`,
      `<input type="text" id="f-category" data-f="category" value="${esc(fm.category || '')}" placeholder="写作, 效率" autocomplete="off">`);

    const errList = errs.length
      ? `<div class="note bad" style="margin-bottom:12px">引擎会这么拒（${errs.length} 条）：<br>${errs.map((e) => '· ' + esc(e.msg)).join('<br>')}</div>`
      : '';

    return `<div class="page">
      <div class="phead">
        <div class="id">
          <h2>新建${kindLabel}</h2>
          <div class="sub">必填项三张表各不相同；右边是<b>马上要落盘的内容</b>，改一个字就跟着变。</div>
        </div>
        <div class="acts">
          <button class="btn ghost sm" data-act="cancel-new">取消</button>
        </div>
      </div>
      <div class="form2">
        <div class="fcol">${errList}${form}</div>
        <div class="pvcol">
          <div class="pv-title">落盘预览</div>
          <div class="pv-sub">跟引擎 <code>create${kindLabel}</code> 写出来的一模一样：${fm.kind === 'skills'
            ? 'description 在 <code>SKILL.md</code> 的 frontmatter；<code>nous.json</code> 里 summary 是空串。'
            : fm.kind === 'prompts'
              ? '<code>PROMPT.md</code> 是纯正文（没有 frontmatter）；summary 在 <code>nous.json</code>。'
              : '清单 <code>nous.json</code> + 一个空壳入口 <code>index.js</code>；建完不提交。'}</div>
          ${files.map((f) => `<div class="pv-file">
            <div class="path">${esc(f.path)}</div>
            <pre>${esc(f.text)}</pre>
          </div>`).join('')}
        </div>
      </div>
      <div class="formfoot">
        <div class="msg ${errs.length ? 'bad' : 'idle'}">${errs.length ? errs.length + ' 条过不去，改完才能建' : '建完停在「未发布」，不自动发布'}</div>
        <button class="btn ghost sm" data-act="cancel-new">取消</button>
        <button class="btn primary" data-act="submit-new">${fm.kind === 'prompts' ? '建，去编辑正文' : '建，去编辑'}</button>
      </div>
    </div>`;
  }

  // ── 渲染：编辑工作台 ───────────────────────────────────────────────

  function renderEdit() {
    const e = S.edit;
    const files = e.files;
    const act = e.active;
    const cur = files.find((f) => f.path === act) || files[0];
    const dirty = cur.text !== e.saved[cur.path];

    const groups = {};
    for (const f of files) {
      const seg = f.path.includes('/') ? f.path.split('/')[0] : '';
      (groups[seg] = groups[seg] || []).push(f);
    }

    let tree = '';
    for (const [g, list] of Object.entries(groups)) {
      if (g) tree += `<div class="grp">${esc(g)}/</div>`;
      for (const f of list) {
        const d = f.text !== e.saved[f.path];
        tree += `<button class="f ${f.path === act ? 'on' : ''}" data-efile="${esc(f.path)}">
          <span class="p mono">${esc(g ? f.path.slice(g.length + 1) : f.path)}</span>
          ${d ? '<i class="dot"></i>' : ''}
        </button>`;
      }
    }
    tree += `<button class="addf" data-act="add-file">+ 新文件</button>`;

    const added = cur.text.split('\n').length - (e.saved[cur.path] || '').split('\n').length;
    const diff = added >= 0 ? `<span class="a">+${added} 行</span>` : `<span class="d">${added} 行</span>`;

    return `<div class="page">
      <div class="phead">
        <div class="id">
          <h2>${esc(e.row.name)} ${e.row.tracked ? '' : '<span class="chip dirty">未发布</span>'}</h2>
          <div class="sub">整页工作台：左边文件树、中间正文、右边状态。不是盖在列表上的右栏抽屉。</div>
        </div>
        <div class="acts">
          <button class="btn ghost sm" data-act="close-edit">关掉</button>
        </div>
      </div>
      <div class="edit">
        <div class="ftree">${tree}</div>
        <div class="editor">
          <div class="eh">
            <span class="path">${esc(cur.path)}</span>
            <span class="spacer"></span>
            ${dirty ? '<span class="chip dirty">有改动</span>' : '<span class="chip">已保存</span>'}
            <button class="btn ghost sm" data-act="save-file">存草稿</button>
          </div>
          <textarea id="etext" data-efile-text="${esc(cur.path)}" spellcheck="false">${esc(cur.text)}</textarea>
        </div>
        <div class="estatus">
          <div>
            <div class="h">状态</div>
            <div class="row"><span class="k">发布</span><span>${e.row.tracked ? '已发布（HEAD 里有）' : '未发布（只在这台机器上）'}</span></div>
            <div class="row"><span class="k">装载</span><span>${e.row.loadError ? '失败' : '正常'}</span></div>
          </div>
          <div>
            <div class="h">改动</div>
            <div class="row"><span class="k">这个文件</span><span class="diff">${dirty ? diff : '没动'}</span></div>
            <div class="row"><span class="k">整条</span><span>${files.filter((f) => f.text !== e.saved[f.path]).length} 个文件有改动</span></div>
          </div>
          <div>
            <div class="h">落盘位置</div>
            <div class="row"><span class="k">仓库</span><span class="mono" style="font-size:10.5px;word-break:break-all">${esc(repoRoot(e.row.kind))}/${esc(e.row.name)}/</span></div>
          </div>
          ${e.row.kind === 'ext' ? `<div class="note">改完先过 <code>node --check</code> 才发得出去 —— 入口文件语法错会在发布那一关卡住。</div>` : ''}
          <div class="note">存草稿只落盘、不提交。要进别人的视野得去「发布」。</div>
        </div>
      </div>
      <div class="formfoot">
        <div class="msg idle">存草稿 = 只写盘。关掉会问一句「有没存的改动」。</div>
        <button class="btn ghost sm" data-act="close-edit">关掉</button>
        <button class="btn primary" data-act="save-all">保存</button>
      </div>
    </div>`;
  }

  function repoRoot(kind) {
    return kind === 'skills' ? D.skills.root : kind === 'prompts' ? D.prompts.root : D.exts.root;
  }

  // ── 渲染：发布 ─────────────────────────────────────────────────────

  function renderPublish() {
    const repos = [
      { kind: 'skills', label: '技能', root: D.skills.root, repo: D.skills.repo },
      { kind: 'prompts', label: '提示词', root: D.prompts.root, repo: D.prompts.repo },
      { kind: 'ext', label: '扩展', root: D.exts.root, repo: D.exts.repo },
    ];
    const cards = repos.map((r) => {
      const pending = pendingOf(r.kind);
      const out = S.published[r.kind];
      const err = S.publishError[r.kind];
      const blocked = r.kind === 'ext' && r.repo && r.repo.pullError;
      return `<div class="pubcard ${blocked ? 'blocked' : ''} ${out ? 'out' : ''}">
        <div class="info">
          <div class="nm">${r.label}
            ${pending ? `<span class="chip dirty">${pending} 处待发布</span>` : '<span class="chip ok">没有待发布</span>'}
            ${blocked ? '<span class="chip dirty">和远端分叉</span>' : ''}
          </div>
          <div class="path">${esc(r.root)}</div>
          <div class="st">${out
            ? '已提交并推送。'
            : err
              ? err
              : blocked
                ? 'pull 失败，发布前得先解决分叉：<span class="mono">当前分支没有跟踪信息</span>'
                : `HEAD ${esc((r.repo && (r.repo.head || r.repo.lock) || '').slice(0, 8))} · 干净`}</div>
        </div>
        <button class="btn ${blocked ? 'ghost' : 'primary'} sm" data-pub="${r.kind}" ${out ? 'disabled' : ''}>
          ${out ? '已发布' : '发布' + r.label}
        </button>
      </div>`;
    }).join('');

    return `<div class="page">
      <div class="phead">
        <div class="id">
          <h2>发布</h2>
          <div class="sub">三个仓各走各的，不做一键全发 —— 谁失败只染谁，别的照发。</div>
        </div>
        <div class="acts"><button class="btn ghost sm" data-act="close-publish">关掉</button></div>
      </div>
      <div class="pubwrap">
        ${cards}
        <div class="note">为什么不放「一键全发」：三个仓是三件独立的事。<code>/opt/Nous-extend</code> 当前分叉且 pull 失败 —— 一键发会卡死在这里，技能 / 提示词也发不出去。</div>
        <div class="note">发布 = 内容闸 + 重扫 <code>index.json</code> + commit + push，一步。闸不过就一个字节都不动（<code>index.json</code> 也不重扫，否则会留下「扫了一半」的假象）。</div>
      </div>
    </div>`;
  }

  function pendingOf(kind) {
    if (kind === 'skills') return st.skills.filter((s) => !s.tracked && !s.variant).length;
    if (kind === 'prompts') return st.prompts.filter((p) => !p.tracked).length;
    return (D.exts.repo && D.exts.repo.drift) ? 1 : 0;
  }

  // ── 渲染：对话框 / 提示 ────────────────────────────────────────────

  function renderDialog() {
    if (!S.dialog) return '';
    const d = S.dialog;
    return `<div class="scrim" data-act="scrim-close"><div class="dlg">
      <h3>${esc(d.title)}</h3>
      <p>${d.text}</p>
      <div class="acts">
        <button class="btn ghost sm" data-act="dlg-no">取消</button>
        <button class="btn ${d.tone === 'danger' ? 'danger' : 'primary'} sm" data-act="dlg-yes">${esc(d.confirm || '确定')}</button>
      </div>
    </div></div>`;
  }

  function renderFlash() {
    if (!S.flash) return '';
    return `<div class="banner ${S.flash.tone || 'warn'}">${esc(S.flash.text)}</div>`;
  }

  // ── 镜头索引 ───────────────────────────────────────────────────────

  const SHOTS = [
    { g: '浏览' },
    { id: 'skills', t: '技能列表', d: '03 两段：内容段的技能' },
    { id: 'prompts', t: '提示词列表', d: '子开关切过去' },
    { id: 'ext', t: '扩展列表', d: '五类来源归一成行' },
    { id: 'cat', t: '点类别', d: '树里选一行，计数含后代' },
    { id: 'search', t: '搜索命中', d: '按名字 / 摘要过滤' },
    { id: 'empty', t: '空结果', d: '搜不到时的样子' },
    { id: 'hidden', t: '隐藏技能', d: '模型看不见的那一档' },
    { id: 'default', t: '默认提示词', d: 'settings.json 的 default' },
    { id: 'extbad', t: '扩展·加载失败', d: '变体装不上，芯片是红的' },
    { id: 'extarch', t: '扩展·已归档', d: '.archive/ 里的那条' },
    { id: 'extbuiltin', t: '扩展·内置', d: '不在仓库里，装不了删不掉' },
    { id: 'extcore', t: '扩展·核心三条', d: '卸载会 403，发布可以' },
    { id: 'loading', t: '加载中', d: '骨架屏' },
    { id: 'repoerr', t: '一个仓挂了', d: '只染它自己，别的照常' },
    { g: '新建' },
    { id: 'new-skill', t: '新建技能', d: '必填 description' },
    { id: 'new-prompt', t: '新建提示词', d: '必填 summary，正文可空' },
    { id: 'new-ext', t: '新建扩展', d: '必填说明 ≤ 200' },
    { id: 'new-badname', t: '名字非法', d: '引擎原话，不是「输入不合法」' },
    { id: 'new-empty', t: '必填空', d: '三张表的必填各不相同' },
    { id: 'new-dup', t: '撞名', d: '已经在了 —— 换个名字' },
    { id: 'new-arch', t: '撞归档', d: '点「恢复」捡回来，别建第二个' },
    { id: 'new-core', t: '核心名被挡', d: 'nous / skills / prompts' },
    { g: '建完' },
    { id: 'created', t: '建完 · 未发布', d: '选中新条目，不自动发布' },
    { g: '编辑' },
    { id: 'edit', t: '编辑工作台', d: '文件树 + 正文 + 状态' },
    { id: 'edit-dirty', t: '有改动', d: '脏点 + 行数差' },
    { id: 'edit-guard', t: '没存就关', d: '拦一下再走' },
    { g: '发布' },
    { id: 'publish', t: '发布面板', d: '三仓各一个按钮' },
    { id: 'pub-ok', t: '技能发成功', d: '回执 + 待发布清零' },
    { id: 'pub-ext', t: '扩展发失败', d: '闸不过：pull 失败原文' },
    { g: '危险操作' },
    { id: 'arch-confirm', t: '归档确认', d: '多走一步才够得到' },
    { id: 'purge-confirm', t: '真删确认', d: '不可逆，说清后果' },
    { g: '窗口' },
    { id: 'narrow', t: '窄窗', d: '< 1080 收掉类别树' },
    { id: 'run', t: '产品数据 → rsp', d: '真数据从哪儿来' },
  ];

  function renderShots() {
    if (S.shotsHidden) return '';
    let html = '';
    for (const s of SHOTS) {
      if (s.g) { html += `<div class="g">${esc(s.g)}</div>`; continue; }
      html += `<button data-shot="${s.id}" class="${S.shotId === s.id ? 'on' : ''}">
        <div class="t">${esc(s.t)}</div><div class="d">${esc(s.d)}</div>
      </button>`;
    }
    return `<aside class="shots">
      <div class="sh">镜头 <span class="spacer"></span><span class="kbd">?</span><button data-act="toggle-shots" style="color:#888">×</button></div>
      <div class="list">${html}</div>
    </aside>`;
  }

  // ── 主渲染 ─────────────────────────────────────────────────────────

  function render() {
    const active = document.activeElement;
    const aid = active && active.id;
    const selStart = active && active.selectionStart;
    const selEnd = active && active.selectionEnd;

    let main;
    if (S.view === 'new') main = renderNew();
    else if (S.view === 'edit') main = renderEdit();
    else if (S.view === 'publish') main = renderPublish();
    else main = `${renderTree()}${renderList()}${renderDetail()}`;

    const narrow = window.innerWidth < 1080 || S.forceNarrow;
    const shift = S.shotsHidden ? '' : 'padding-right:280px';
    document.getElementById('root').innerHTML = `<div class="app ${narrow ? 'narrow' : ''}" style="${shift}">
      ${renderRail()}
      <div class="main">
        ${renderTop()}
        ${renderFlash()}
        <div class="body">${main}</div>
      </div>
      ${renderShots()}
      ${renderDialog()}
    </div>`;

    if (aid) {
      const el = document.getElementById(aid);
      if (el) {
        el.focus();
        try { if (selStart != null && el.setSelectionRange) el.setSelectionRange(selStart, selEnd); } catch (e) { /* 非文本控件 */ }
      }
    }
  }

  // ── 动作 ───────────────────────────────────────────────────────────

  function select(kind, name) {
    S.sel = { kind, name };
    S.fileView = null;
    S.view = 'browse';
  }

  function startNew(kind) {
    S.view = 'new';
    S.form = { kind, name: '', description: '', summary: '', body: '', category: '' };
    S.formErrors = [];
    S.flash = null;
  }

  function submitNew() {
    const fm = S.form;
    const errs = validateNew(fm, rowsNow());
    S.formErrors = errs;
    if (errs.length) { render(); return; }

    const name = fm.name.trim();
    const cats = String(fm.category || '').split(/[,，\s]+/).map((s) => s.trim()).filter(Boolean);
    const row = {
      kind: fm.kind === 'ext' ? 'ext' : fm.kind,
      name,
      desc: fm.kind === 'skills' ? fm.description.trim() : fm.summary.trim(),
      category: cats,
      hidden: false,
      tracked: false,
      fileCount: 2,
      variant: false,
      loadError: null,
      version: fm.kind === 'ext' ? '0.1.0' : null,
      sections: [],
      needs: {},
    };
    if (fm.kind === 'skills') st.skills.push(row);
    else if (fm.kind === 'prompts') st.prompts.push(row);
    else st.ext.push({ ...row, extKind: 'off' });

    // 建完：停在未发布 + 选中新条目；提示词建完直接进编辑器（正文是它的主要内容）
    S.section = fm.kind === 'ext' ? 'ext' : 'content';
    if (fm.kind !== 'ext') S.content = fm.kind;
    S.cat[activeKind()] = null;
    S.q[activeKind()] = '';
    select(activeKind(), name);
    S.shotId = 'created';
    S.flash = {
      tone: 'ok',
      text: `建好了：${name} 停在「未发布」—— 没提交，只在这台机器上看得见。`,
    };
    if (fm.kind === 'prompts') {
      startEdit(findRow('prompts', name));
    }
    render();
  }

  function startEdit(row) {
    const files = filesOf(row).map((f) => ({
      path: f.path,
      text: f.text != null ? f.text : synthText(row, f.path),
    }));
    S.view = 'edit';
    S.edit = {
      row,
      files,
      active: files[0].path,
      saved: Object.fromEntries(files.map((f) => [f.path, f.text])),
    };
  }

  function editDirty() {
    const e = S.edit;
    if (!e) return false;
    return e.files.some((f) => f.text !== e.saved[f.path]);
  }

  function closeEdit() {
    if (editDirty()) {
      S.dialog = {
        title: '有没存的改动',
        text: '这些改动只在编辑器里，关掉就没了。要先存草稿吗？',
        confirm: '不存，关掉',
        tone: 'danger',
        onYes: () => {
          S.view = 'browse';
          S.edit = null;
          S.dialog = null;
          S.flash = { tone: 'warn', text: '关掉了，改动没落盘。' };
          render();
        },
      };
      render();
      return;
    }
    S.view = 'browse';
    S.edit = null;
    render();
  }

  function saveAll() {
    const e = S.edit;
    for (const f of e.files) e.saved[f.path] = f.text;
    S.flash = { tone: 'ok', text: `已存草稿：${e.files.length} 个文件写进 ${repoRoot(e.row.kind)}/${e.row.name}/（还没提交）。` };
    render();
  }

  function publish(kind) {
    if (kind === 'ext') {
      S.publishError.ext = '发布失败，引擎原话：' +
        (D.exts.repo && D.exts.repo.pullError ? D.exts.repo.pullError.split('\n')[0] : '当前分支没有跟踪信息。');
      S.flash = { tone: 'bad', text: '扩展没发出去：仓库和远端分叉，pull 失败。技能 / 提示词不受影响。' };
      render();
      return;
    }
    if (kind === 'skills') st.skills.forEach((s) => { if (!s.variant) s.tracked = true; });
    if (kind === 'prompts') st.prompts.forEach((p) => { p.tracked = true; });
    S.published[kind] = true;
    S.publishError[kind] = null;
    S.flash = { tone: 'ok', text: `${kind === 'skills' ? '技能' : '提示词'}仓已提交并推送。` };
    render();
  }

  // ── 镜头：每个镜头就是一组状态 ─────────────────────────────────────

  function jump(id) {
    S.shotId = id;
    S.view = 'browse';
    S.dialog = null;
    S.flash = null;
    S.loading = false;
    S.repoError = { skills: null, prompts: null, ext: null };
    S.section = 'content';
    S.content = 'skills';
    S.cat = { skills: null, prompts: null, ext: null };
    S.q = { skills: '', prompts: '', ext: '' };
    S.published = {};
    S.publishError = {};
    st = cloneData(BASE);
    S.edit = null;
    S.form = null;
    S.formErrors = [];
    S.sel = { kind: 'skills', name: 'code-review' };
    S.fileView = null;
    S.forceNarrow = false;

    switch (id) {
      case 'skills': break;
      case 'prompts':
        S.content = 'prompts'; S.sel = { kind: 'prompts', name: 'smart-ass' }; break;
      case 'ext':
        S.section = 'ext'; S.sel = { kind: 'ext', name: 'nous' }; break;
      case 'cat':
        S.cat.skills = '工程'; S.sel = { kind: 'skills', name: 'code-review' }; break;
      case 'search':
        S.q.skills = '审查'; S.sel = { kind: 'skills', name: 'code-review' }; break;
      case 'empty':
        S.q.skills = 'zzz 没有这个'; S.sel = null; break;
      case 'hidden':
        S.cat.skills = '@hidden'; S.sel = { kind: 'skills', name: 'ask-matt' }; break;
      case 'default':
        S.content = 'prompts'; S.cat.prompts = '@default'; S.sel = { kind: 'prompts', name: 'smart-ass' }; break;
      case 'extbad':
        S.section = 'ext'; S.cat.ext = '@bad'; S.sel = { kind: 'ext', name: '123@123' }; break;
      case 'extarch':
        S.section = 'ext'; S.cat.ext = '@archived'; S.sel = { kind: 'ext', name: 'demo' }; break;
      case 'extbuiltin':
        S.section = 'ext'; S.cat.ext = '@builtin'; S.sel = { kind: 'ext', name: 'coding-tools' }; break;
      case 'extcore':
        S.section = 'ext'; S.sel = { kind: 'ext', name: 'prompts' }; break;
      case 'loading':
        S.loading = true; S.sel = null; break;
      case 'repoerr':
        S.repoError.prompts = '拉 /v1/prompts 失败：connection refused（引擎没在 4790 上）';
        S.content = 'prompts'; S.sel = null; break;
      case 'new-skill':
        S.view = 'new';
        S.form = { kind: 'skills', name: 'weekly-digest', description: '把这几天的会议、邮件、聊天整理成一份周报', summary: '', body: '', category: '写作, 效率' };
        break;
      case 'new-prompt':
        S.view = 'new';
        S.form = { kind: 'prompts', name: 'strict-review', description: '', summary: '审查时按标准 + 规格两条轴走，不混成一股感觉', body: '', category: '工程' };
        break;
      case 'new-ext':
        S.view = 'new';
        S.form = { kind: 'ext', name: 'weekly-card', description: '', summary: '把一周的提交整理成一张能贴出去的图', body: '', category: '工具' };
        break;
      case 'new-badname':
        S.view = 'new';
        S.form = { kind: 'skills', name: 'Weekly Digest', description: '整理周报', summary: '', body: '', category: '' };
        S.formErrors = validateNew(S.form, rowsNow());
        break;
      case 'new-empty':
        S.view = 'new';
        S.form = { kind: 'skills', name: '', description: '', summary: '', body: '', category: '' };
        break;
      case 'new-dup':
        S.view = 'new';
        S.form = { kind: 'skills', name: 'code-review', description: '再看一遍', summary: '', body: '', category: '' };
        S.formErrors = validateNew(S.form, rowsNow());
        break;
      case 'new-arch':
        S.section = 'ext';
        S.view = 'new';
        S.form = { kind: 'ext', name: 'demo', description: '', summary: '想重建一个 demo', body: '', category: '' };
        S.formErrors = validateNew(S.form, rowsNow());
        break;
      case 'new-core':
        S.section = 'ext';
        S.view = 'new';
        S.form = { kind: 'ext', name: 'skills', description: '', summary: '想造一个叫 skills 的', body: '', category: '' };
        S.formErrors = validateNew(S.form, rowsNow());
        break;
      case 'created': {
        S.section = 'content'; S.content = 'skills';
        const row = {
          kind: 'skills', name: 'weekly-digest',
          desc: '把这几天的会议、邮件、聊天整理成一份周报',
          category: ['写作', '效率'], hidden: false, tracked: false,
          fileCount: 2, variant: false, loadError: null, version: null, sections: [], needs: {},
        };
        st.skills.push(row);
        S.sel = { kind: 'skills', name: 'weekly-digest' };
        break;
      }
      case 'edit':
        startEdit(findRow('skills', 'code-review'));
        break;
      case 'edit-dirty': {
        startEdit(findRow('skills', 'code-review'));
        const f = S.edit.files[0];
        f.text = f.text + '\n## 第三条\n别把「标准」和「规格」写成同一段。\n';
        break;
      }
      case 'edit-guard': {
        startEdit(findRow('skills', 'code-review'));
        const f0 = S.edit.files[0];
        f0.text = f0.text + '\n## 第三条\n写了一半就想关。\n';
        S.dialog = {
          title: '有没存的改动',
          text: '这些改动只在编辑器里，关掉就没了。要先存草稿吗？',
          confirm: '不存，关掉', tone: 'danger', onYes: null,
        };
        break;
      }
      case 'publish':
        S.view = 'publish';
        break;
      case 'pub-ok':
        S.view = 'publish';
        st.skills.forEach((s) => { if (!s.variant) s.tracked = true; });
        S.published.skills = true;
        break;
      case 'pub-ext':
        S.view = 'publish';
        S.publishError.ext = '发布失败，引擎原话：当前分支没有跟踪信息。';
        break;
      case 'arch-confirm':
        S.sel = { kind: 'skills', name: 'scratch-notes' };
        S.dialog = {
          title: '归档 scratch-notes？',
          text: '会 <code>git mv</code> 到 <code>.archive/</code> 并 commit + push —— 别人也看不见了。<br>想回来点「恢复」，内容还在。',
          confirm: '归档', tone: 'warn', onYes: null,
        };
        break;
      case 'purge-confirm':
        S.section = 'ext'; S.cat.ext = '@archived';
        S.sel = { kind: 'ext', name: 'demo' };
        S.dialog = {
          title: '真删 demo？',
          text: '<b>不可逆。</b>会 <code>git rm -r .archive/demo</code> 并 commit + push —— 连归档区那份也没了，恢复不了。',
          confirm: '真删', tone: 'danger', onYes: null,
        };
        break;
      case 'narrow': S.forceNarrow = true; break;
      case 'run': break;
      default: break;
    }
    render();
  }

  // ── 事件 ───────────────────────────────────────────────────────────

  function onInput(e) {
    const t = e.target;
    if (t.dataset.q) {
      S.q[t.dataset.q] = t.value;
      render();
      return;
    }
    if (t.dataset.f) {
      S.form[t.dataset.f] = t.value;
      // 输入时不重算错误，免得边打边红；提交时才算
      renderLivePreviewOnly();
      return;
    }
    if (t.dataset.efileText) {
      const e2 = S.edit;
      const f = e2.files.find((x) => x.path === t.dataset.efileText);
      if (f) {
        f.text = t.value;
        renderStatusOnly();
      }
      return;
    }
  }

  /** 改表单时只重画预览半边，别把输入框焦点/光标弄丢。 */
  function renderLivePreviewOnly() {
    const r = document.querySelector('.form2 > .pvcol');
    if (!r || !S.form) { render(); return; }
    const files = landingFiles(S.form);
    r.innerHTML = `<div class="pv-title">落盘预览</div>
      <div class="pv-sub">改一个字这里就跟着变。</div>
      ${files.map((f) => `<div class="pv-file">
        <div class="path">${esc(f.path)}</div>
        <pre>${esc(f.text)}</pre>
      </div>`).join('')}`;
  }

  /** 编辑器里打字：只更新右栏状态和脏点，不重画 textarea。 */
  function renderStatusOnly() {
    const e = S.edit;
    const _ = e.files.filter((f) => f.text !== e.saved[f.path]).length;
    const box = document.querySelector('.estatus');
    if (!box) { render(); return; }
    const cur = e.files.find((f) => f.path === e.active);
    const dirty = cur.text !== e.saved[cur.path];
    const eh = document.querySelector('.editor .eh');
    if (eh) {
      const chips = eh.querySelectorAll('.chip');
      if (chips[0]) {
        chips[0].className = dirty ? 'chip dirty' : 'chip';
        chips[0].textContent = dirty ? '有改动' : '已保存';
      }
    }
    const rowsEls = box.querySelectorAll('.row .diff');
    if (rowsEls[0]) {
      const added = cur.text.split('\n').length - (e.saved[cur.path] || '').split('\n').length;
      rowsEls[0].innerHTML = dirty ? (added >= 0 ? `<span class="a">+${added} 行</span>` : `<span class="d">${added} 行</span>`) : '没动';
    }
    const whole = box.querySelectorAll('.row');
    if (whole[3]) {
      whole[3].innerHTML = `<span class="k">整条</span><span>${e.files.filter((f) => f.text !== e.saved[f.path]).length} 个文件有改动</span>`;
    }
    // 文件树上的脏点
    for (const btn of document.querySelectorAll('.ftree .f')) {
      const p = btn.dataset.efile;
      const f = e.files.find((x) => x.path === p);
      const d = f && f.text !== e.saved[f.path];
      const dot = btn.querySelector('.dot');
      if (d && !dot) btn.insertAdjacentHTML('beforeend', '<i class="dot"></i>');
      if (!d && dot) dot.remove();
    }
  }

  function onClick(e) {
    const t = e.target.closest('[data-act],[data-seg],[data-sub],[data-cat],[data-row],[data-file],[data-efile],[data-shot],[data-pub],[data-nav]');
    if (!t) return;

    if (t.dataset.nav) {
      S.flash = { tone: 'warn', text: `「${t.dataset.nav}」不在这份 demo 里 —— 只画资源页。` };
      render();
      return;
    }
    if (t.dataset.seg) {
      S.section = t.dataset.seg === 'ext' ? 'ext' : 'content';
      if (S.section === 'content') S.content = 'skills';
      S.sel = null; S.fileView = null; S.view = 'browse';
      render(); return;
    }
    if (t.dataset.sub) {
      S.content = t.dataset.sub; S.section = 'content';
      S.sel = null; S.fileView = null; S.view = 'browse';
      render(); return;
    }
    if (t.dataset.cat !== undefined) {
      S.cat[activeKind()] = t.dataset.cat || null;
      S.sel = null; S.fileView = null;
      render(); return;
    }
    if (t.dataset.row) {
      const [kind, name] = t.dataset.row.split('|');
      select(kind, name);
      render(); return;
    }
    if (t.dataset.file) { S.fileView = t.dataset.file; render(); return; }
    if (t.dataset.efile) {
      S.edit.active = t.dataset.efile;
      render(); return;
    }
    if (t.dataset.shot) { jump(t.dataset.shot); return; }
    if (t.dataset.pub) { publish(t.dataset.pub); return; }

    switch (t.dataset.act) {
      case 'toggle-shots':
        S.shotsHidden = !S.shotsHidden; render(); return;
      case 'reload':
        S.loading = true; S.repoError = { skills: null, prompts: null, ext: null };
        render();
        setTimeout(() => { S.loading = false; S.flash = { tone: 'ok', text: '三份 GET 并发拉完了（demo 里是假的，真机上各自落地）。' }; render(); }, 700);
        return;
      case 'clear-q':
        S.q[activeKind()] = ''; render(); return;
      case 'new':
        startNew(activeKind() === 'ext' ? 'ext' : S.content); render(); return;
      case 'cancel-new':
        S.view = 'browse'; S.form = null; S.formErrors = []; render(); return;
      case 'submit-new':
        submitNew(); return;
      case 'edit': {
        const r = findRow(S.sel.kind, S.sel.name);
        if (r) { startEdit(r); render(); }
        return;
      }
      case 'close-edit': closeEdit(); return;
      case 'save-all': saveAll(); return;
      case 'save-file':
        S.flash = { tone: 'ok', text: '存了。' }; render(); return;
      case 'close-publish':
        S.view = 'browse'; render(); return;
      case 'publish':
        S.view = 'publish'; render(); return;
      case 'add-file':
        S.flash = { tone: 'warn', text: '新文件要填路径（相对仓库根）—— 这一刀只做「改已有文件」。' };
        render(); return;
      case 'retry':
        S.flash = { tone: 'ok', text: '重试装载：remount 了扩展注册表（demo 里不真跑）。' };
        render(); return;
      case 'install':
        if (S.sel) { const r = findRow('ext', S.sel.name); if (r) r.extKind = 'on'; }
        S.flash = { tone: 'ok', text: '装上了 —— 立刻重装注册表，不用等 30 秒轮询。' };
        render(); return;
      case 'uninstall':
        if (S.sel && CORE_EXT.includes(S.sel.name)) {
          S.flash = { tone: 'bad', text: `403：${S.sel.name} 是引擎启动路径上硬编码 import 的核心扩展，不能动` };
          render(); return;
        }
        if (S.sel) { const r = findRow('ext', S.sel.name); if (r) r.extKind = 'off'; }
        S.flash = { tone: 'warn', text: '卸载了：目录还在仓库里，只是这台不装。' };
        render(); return;
      case 'archive':
        S.dialog = {
          title: `归档 ${S.sel ? S.sel.name : ''}？`,
          text: '会 <code>git mv</code> 到 <code>.archive/</code> 并 commit + push —— 别人也看不见了。<br>想回来点「恢复」，内容还在。',
          confirm: '归档', tone: 'warn',
          onYes: () => {
            const r = findRow(S.sel.kind, S.sel.name);
            if (r) { r.extKind = 'archived'; r.tracked = false; }
            S.dialog = null;
            S.flash = { tone: 'warn', text: '归档了：搬进 .archive/ 并提交。' };
            render();
          },
        };
        render(); return;
      case 'restore':
        S.dialog = {
          title: `恢复 ${S.sel ? S.sel.name : ''}？`,
          text: '从 <code>.archive/</code> 搬回仓库并 commit + push —— 回到「已发布」。',
          confirm: '恢复', tone: 'primary',
          onYes: () => {
            const r = findRow('ext', S.sel.name);
            if (r) { r.extKind = 'off'; r.tracked = true; }
            S.dialog = null;
            S.flash = { tone: 'ok', text: '恢复回来了。' };
            render();
          },
        };
        render(); return;
      case 'purge':
        S.dialog = {
          title: `真删 ${S.sel ? S.sel.name : ''}？`,
          text: '<b>不可逆。</b>会 <code>git rm -r</code> 掉归档区那份并 commit + push —— 恢复不了。',
          confirm: '真删', tone: 'danger',
          onYes: () => {
            st.ext = st.ext.filter((r) => !(r.name === S.sel.name && r.extKind === 'archived'));
            S.dialog = null; S.sel = null;
            S.flash = { tone: 'bad', text: '真删了，归档区那份也没了。' };
            render();
          },
        };
        render(); return;
      case 'dlg-yes':
        if (S.dialog && S.dialog.onYes) S.dialog.onYes();
        else { S.dialog = null; render(); }
        return;
      case 'dlg-no':
      case 'scrim-close':
        S.dialog = null; render(); return;
      default:
        return;
    }
  }

  function onKey(e) {
    if (e.target.matches('input, textarea')) {
      if (e.key === 'Escape' && S.view === 'new') { S.view = 'browse'; S.form = null; render(); }
      return;
    }
    if (e.key === '?') { S.shotsHidden = !S.shotsHidden; render(); return; }
    if (e.key === 'Escape') {
      if (S.dialog) { S.dialog = null; render(); return; }
      if (S.view !== 'browse') { S.view = 'browse'; render(); return; }
      return;
    }
    if (e.key === '1') { S.section = 'content'; S.content = 'skills'; S.sel = null; render(); return; }
    if (e.key === '2') { S.section = 'ext'; S.sel = null; render(); return; }
    if (e.key === '3') { S.section = 'content'; S.content = 'prompts'; S.sel = null; render(); return; }
    if (e.key === 'n') { startNew(activeKind() === 'ext' ? 'ext' : S.content); render(); return; }
    if (e.key === 'e' && S.sel) { const r = findRow(S.sel.kind, S.sel.name); if (r) { startEdit(r); render(); } return; }
    if (e.key === 'p') { S.view = 'publish'; render(); return; }
  }

  // ── 启动 ───────────────────────────────────────────────────────────

  function boot() {
    document.addEventListener('input', onInput, true);
    document.addEventListener('click', onClick, true);
    document.addEventListener('keydown', onKey);
    const hash = location.hash.replace('#', '');
    jump(hash && SHOTS.some((s) => s.id === hash) ? hash : 'skills');
  }

  window.P = {
    ancestors, isUnder, buildTree, visibleSkills, promptRows, extRows,
    counts, checkName, validateNew, landingFiles, filesOf, synthText,
    state: S, data: () => st, jump, render, SHOTS,
  };

  if (document.getElementById('root')) boot();
})();
