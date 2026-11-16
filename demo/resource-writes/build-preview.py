#!/usr/bin/env python3
"""把 index.html + app.css + app.js + data.js + 字体 塞成单文件 preview.html。

为什么要内联：交付给用户的是「双击就开」的一份文件，不能让人先起 server。
（AGENTS.md：不给端口、不给 localhost。）

用法：
    python3 build-preview.py          # 出 preview.html
    python3 build-preview.py --test   # 先用 node 跑纯函数全组合，再出文件
"""

import base64
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE / "preview.html"

FONTS = {
    "Inter": "vendor/fonts/inter-400.woff2",
    "JetBrainsMono": "vendor/fonts/jetbrainsmono-400.woff2",
}


def font_css() -> str:
    parts = []
    for family, rel in FONTS.items():
        p = HERE / rel
        if not p.exists():
            print(f"warning: 缺字体 {rel}", file=sys.stderr)
            continue
        b64 = base64.b64encode(p.read_bytes()).decode()
        parts.append(
            "@font-face{font-family:'%s';font-style:normal;font-weight:400;"
            "font-display:block;src:url(data:font/woff2;base64,%s) format('woff2')}"
            % (family, b64)
        )
    return "\n".join(parts)


def build() -> None:
    css = (HERE / "app.css").read_text(encoding="utf-8")
    app = (HERE / "app.js").read_text(encoding="utf-8")
    data = (HERE / "data.js").read_text(encoding="utf-8")

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>资源页 03 · 交互原型</title>
<style>
{font_css()}
{css}
</style>
</head>
<body>
<div id="root"></div>
<script>
{data}
</script>
<script>
{app}
</script>
</body>
</html>
"""
    OUT.write_text(html, encoding="utf-8")

    # 外链必须为空：单文件的意义就在这儿
    ext = re.findall(r'(?:src|href)="(?!#|data:)([^"]+)"', html)
    print(f"preview.html {OUT.stat().st_size} bytes; 外链: {ext or '无'}")
    if ext:
        sys.exit(f"还有外链：{ext}")


def run_tests() -> None:
    """纯函数全组合：两种段 × 三个类别 × 空/有搜索词 × 三张表的校验。"""
    js = r"""
const fs = require('fs');
const path = require('path');
const dir = process.argv[2];
global.window = global;                 // app.js 里 window.D / window.P 都落到 global 上
global.document = { getElementById: () => null };
global.location = { hash: '' };
eval(fs.readFileSync(path.join(dir, 'data.js'), 'utf8'));
eval(fs.readFileSync(path.join(dir, 'app.js'), 'utf8'));
let P = global.window.P;

let n = 0;
function ok(cond, msg) { n++; if (!cond) { console.error('FAIL: ' + msg); process.exitCode = 1; } }

// 1. 树：祖先 / 后代
ok(JSON.stringify(P.ancestors('a/b/c')) === '["a","a/b","a/b/c"]', 'ancestors');
ok(P.isUnder('a/b', 'a') && !P.isUnder('ab', 'a'), 'isUnder');

// 2. 树计数：一条挂 工程 和 工程/审查，工程只算它一次
const tree = P.buildTree(['工程', '工程/审查', '写作'], [['工程'], ['工程/审查'], ['写作']]);
const byPath = Object.fromEntries(tree.map(t => [t.path, t]));
ok(byPath['工程'].count === 2, '工程含后代 = 2，得到 ' + byPath['工程'].count);
ok(byPath['工程/审查'].count === 1, '工程/审查 = 1');
ok(byPath['写作'].count === 1, '写作 = 1');
ok(byPath['工程'].hasChildren === true, '工程 hasChildren');
ok(byPath['工程/审查'].depth === 1, 'depth');
ok(byPath['工程/审查'].label === '审查', 'label 是最后一段');

// 3. 去重：同一条目同时挂父和子，父只算一次
const t2 = P.buildTree([], [['a', 'a/b']]);
ok(t2.find(t => t.path === 'a').count === 1, '同条目按祖先去重');

// 4. 真实数据：段计数跟 resources.rs::counts 一致
const c = P.counts(P.data());
ok(typeof c.skills === 'number' && typeof c.ext === 'number', 'counts 形状');

// 5. 校验：名字规矩
ok(P.checkName('', 'skills') === '技能名不能为空', '空名');
ok(P.checkName('ABC', 'skills').includes('只认小写字母'), '大写被挡');
ok(P.checkName('a'.repeat(65), 'skills').includes('太长'), '超长被挡');
ok(P.checkName('nous.json', 'skills').includes('仓库自己的文件名'), '保留名被挡');
ok(P.checkName('weekly-digest', 'skills') === null, '合法名通过');
ok(P.checkName('skills', 'ext').includes('核心扩展'), '扩展核心名被挡');
ok(P.checkName('bad_name', 'ext').includes('扩展名不合法'), '扩展下划线被挡');

// 6. 三张表的必填各不相同
const skill = P.validateNew({ kind: 'skills', name: 'x1', description: '', summary: '', category: '' }, { skills: [], prompts: [], ext: [] });
ok(skill.some(e => e.msg.includes('description 不能空')), '技能必填 description');
const prompt = P.validateNew({ kind: 'prompts', name: 'x1', description: '', summary: '', category: '' }, { skills: [], prompts: [], ext: [] });
ok(prompt.some(e => e.msg.includes('summary 不能空')), '提示词必填 summary');
const ext = P.validateNew({ kind: 'ext', name: 'x1', summary: '', category: '' }, { skills: [], prompts: [], ext: [] });
ok(ext.some(e => e.msg.includes('一句话说明不能空')), '扩展必填说明');
const extLong = P.validateNew({ kind: 'ext', name: 'x1', summary: 'x'.repeat(201), category: '' }, { skills: [], prompts: [], ext: [] });
ok(extLong.some(e => e.msg.includes('200 字以内')), '扩展说明超长');

// 7. 撞名 / 撞归档
const dup = P.validateNew({ kind: 'skills', name: 'code-review', description: 'd' }, { skills: [{ name: 'code-review' }], prompts: [], ext: [] });
ok(dup.some(e => e.msg.includes('已经在了')), '撞名被挡');
const arch = P.validateNew({ kind: 'ext', name: 'demo', summary: 's' }, { skills: [], prompts: [], ext: [] });
ok(arch.some(e => e.msg.includes('.archive/demo 还留着')), '撞归档被挡');

// 8. 落盘形状（照引擎 createSkill / createPrompt / createExtension）
const sk = P.landingFiles({ kind: 'skills', name: 'weekly-digest', description: '周报', category: '写作, 效率' });
ok(sk[0].path.endsWith('/SKILL.md'), '技能主文件是 SKILL.md');
ok(sk[0].text.includes('description: "周报"'), 'description 在 frontmatter');
const skJson = JSON.parse(sk[1].text);
ok(skJson.summary === '' && skJson.name === 'weekly-digest', '技能的 nous.json summary 是空串');
ok(skJson.category.length === 2, '类别去逗号');

const pr = P.landingFiles({ kind: 'prompts', name: 'strict-review', summary: '两条轴', category: '工程' });
ok(pr[0].path.endsWith('/PROMPT.md'), '提示词主文件是 PROMPT.md');
ok(pr[0].text === '# strict-review\n', '提示词空正文落一个标题');
ok(!pr[0].text.includes('---'), '提示词正文没有 frontmatter');
ok(JSON.parse(pr[1].text).summary === '两条轴', 'summary 在 nous.json');

const ex = P.landingFiles({ kind: 'ext', name: 'weekly-card', summary: '一张图', category: '工具' });
ok(ex[0].path.endsWith('/nous.json'), '扩展清单是 nous.json');
const manifest = JSON.parse(ex[0].text);
ok(manifest.version === '0.1.0' && manifest.entry === 'default', '清单骨架字段');
ok(manifest.summary === '一张图', '说明进 summary');
ok(ex[1].text.includes('name: "weekly-card"'), 'index.js 替换了名字');

// 9. 行过滤：隐藏技能 / 特殊档
const skills = [
  { name: 'a', category: ['工程'], hidden: false, desc: '' },
  { name: 'b', category: ['工程/审查'], hidden: false, desc: '' },
  { name: 'c', category: [], hidden: false, desc: '' },
  { name: 'd', category: [], hidden: true, desc: '' },
];
ok(P.visibleSkills(skills, null, '').map(s => s.name).join('') === 'abc', '隐藏的默认不出现');
ok(P.visibleSkills(skills, '@hidden', '').map(s => s.name).join('') === 'd', '@hidden 只给隐藏的');
ok(P.visibleSkills(skills, '工程', '').map(s => s.name).join('') === 'ab', '类别含后代');
ok(P.visibleSkills(skills, '∅', '').map(s => s.name).join('') === 'c', '未分类');
ok(P.visibleSkills(skills, null, '审').length === 0, '搜索按名字/摘要');
ok(P.visibleSkills(skills, null, 'B').map(s => s.name).join('') === 'b', '搜索不分大小写');

// 10. 扩展五档筛选
const exts = [
  { name: 'x', extKind: 'on', category: [], variant: false, desc: '' },
  { name: 'y', extKind: 'failed', category: [], variant: false, desc: '' },
  { name: 'z', extKind: 'builtin', category: [], variant: false, desc: '' },
  { name: 'w', extKind: 'archived', category: [], variant: false, desc: '' },
  { name: 'v', extKind: 'variant', category: [], variant: true, desc: '' },
];
ok(P.extRows(exts, '@on', '').map(r => r.name).join('') === 'x', '@on');
ok(P.extRows(exts, '@bad', '').map(r => r.name).join('') === 'y', '@bad');
ok(P.extRows(exts, '@builtin', '').map(r => r.name).join('') === 'z', '@builtin');
ok(P.extRows(exts, '@archived', '').map(r => r.name).join('') === 'w', '@archived');
ok(P.extRows(exts, '@variant', '').map(r => r.name).join('') === 'v', '@variant');

// 11. 真数据能过：每一条都能渲染出文件
for (const kind of ['skills', 'prompts', 'ext']) {
  const d = P.data();
  const pool = kind === 'ext' ? d.ext : d[kind];
  ok(Array.isArray(pool) && pool.length > 0, kind + ' 有数据');
}
const rows = P.data().skills;
for (const r of rows) {
  const fs2 = P.filesOf(r);
  ok(fs2.length > 0, r.name + ' 至少有文件');
  for (const f of fs2) ok(typeof P.synthText(r, f.path) === 'string', r.name + '/' + f.path + ' 能合成');
}

// 12. 镜头表每一格都跳得动（不真渲染）
for (const s of P.SHOTS) {
  if (s.g) continue;
  ok(typeof s.id === 'string' && s.id.length > 0, 'shorthand id');
}


// ── 13. 真点击流：过一遍「切段 → 选行 → 新建 → 建完 → 编辑 → 保存 → 发布」 ──
// 用最小 DOM 桩：render 只往 #root 写 innerHTML，事件从 document 上抓回来。
const root = { innerHTML: '' };
const handlers = {};
global.document = {
  getElementById: () => root,
  querySelector: () => null,
  querySelectorAll: () => [],
  addEventListener: (type, fn) => { (handlers[type] = handlers[type] || []).push(fn); },
  activeElement: null,
};
global.location = { hash: '' };
eval(fs.readFileSync(path.join(dir, 'app.js'), 'utf8'));
P = global.window.P;              // 第二次 eval 会重建 P，重新取一次
const S = P.state;

function fire(type, dataset, value) {
  const el = { dataset: dataset || {}, value: value, selectionStart: 0, selectionEnd: 0, matches: () => false };
  el.closest = () => el;
  const fns = handlers[type] || [];
  for (const fn of fns) fn({ target: el, key: '', preventDefault() {} });
}
const click = (dataset) => fire('click', dataset);
const type = (field, v) => fire('input', { f: field, q: field === '__q' ? S.content : '' }, v);

// 树在 boot 时按 hash='' 落到 skills
ok(S.section === 'content' && S.content === 'skills', '默认落在 内容·技能');
ok(root.innerHTML.includes('code-review'), '列表渲染出真数据');
ok(!root.innerHTML.includes('NaN'), '渲染里没有 NaN');
ok(!root.innerHTML.includes('undefined'), '渲染里没有 undefined');

// 切段
click({ seg: 'ext' });
ok(S.section === 'ext', '点「扩展」切到扩展段');
ok(root.innerHTML.includes('coding-tools'), '扩展段列出内置扩展');
click({ seg: 'content' });
ok(S.content === 'skills', '切回内容段默认是技能');
click({ sub: 'prompts' });
ok(S.content === 'prompts', '子开关切到提示词');
ok(root.innerHTML.includes('smart-ass'), '提示词列表有 smart-ass');

// 点类别 / 点行
click({ cat: '写作' });
ok(S.cat.prompts === '写作', '点类别写进筛选');
ok(root.innerHTML.includes('2 条'), '筛选后条数跟着变');
click({ cat: '' });
click({ row: 'prompts|demo-diagram' });
ok(S.sel && S.sel.name === 'demo-diagram', '点行选中');
ok(root.innerHTML.includes('未发布'), '未提交的条目带「未发布」芯片');

// 新建提示词 → 建完进编辑器
click({ act: 'new' });
ok(S.view === 'new' && S.form.kind === 'prompts', '新建跟随当前段');
type('name', 'my-demo');
type('summary', '一句话');
ok(S.form.name === 'my-demo' && S.form.summary === '一句话', '输入写进表单状态');
ok(root.innerHTML.includes('/opt/Nous-prompts/my-demo/PROMPT.md'), '预览里出现要落盘的路径');
click({ act: 'submit-new' });
ok(P.data().prompts.some((p2) => p2.name === 'my-demo'), '建出来了');
ok(S.view === 'edit', '提示词建完直接进编辑器');
ok(S.edit.row.name === 'my-demo', '编辑器打开的是新条目');
ok(S.edit.files[0].path.endsWith('PROMPT.md'), '默认打开正文，不是 nous.json');

// 输入正文 → 脏 → 保存
S.edit.files[0].text = '# my-demo\n\n正文进来了。\n';
fire('input', { efileText: S.edit.files[0].path }, S.edit.files[0].text);
click({ act: 'save-all' });
ok(S.edit.files[0].text === S.edit.saved[S.edit.files[0].path], '保存后不再是脏的');

// 没存就关 → 拦住
S.edit.files[0].text += '又改了。\n';
click({ act: 'close-edit' });
ok(S.dialog && S.dialog.title.includes('没存'), '有改动时关掉会被拦');
click({ act: 'dlg-yes' });
ok(S.view === 'browse' && S.edit === null, '确认后才真的关掉');

// 发布：技能成功、扩展被闸卡住
click({ act: 'publish' });
ok(S.view === 'publish', '进发布面板');
click({ pub: 'skills' });
ok(S.published.skills === true, '技能发布成功');
click({ pub: 'ext' });
ok(!!S.publishError.ext, '扩展发布失败（仓库分叉）');
ok(S.published.ext !== true, '失败的仓不算发布过');

// 未知仓 / 取消新建这类边角
click({ act: 'cancel-new' });
ok(S.view === 'browse', '取消新建回到浏览');

console.log(n + ' 条断言，' + (process.exitCode ? '有失败' : 'ALL OK'));
"""
    tmp = HERE / ".test-app.js"
    tmp.write_text(js, encoding="utf-8")
    try:
        subprocess.run(["node", str(tmp), str(HERE)], check=True)
    finally:
        tmp.unlink(missing_ok=True)


if __name__ == "__main__":
    if "--test" in sys.argv:
        run_tests()
    build()
