//! 资源页的**写操作**：新建 / 编辑 / 发布 / 归档。
//!
//! 只读那一刀在 `resources.rs`（树 / 列表 / 详情）。这一刀把详情头上的按钮接上真接口：
//!
//! | 动作 | 端点 | 落在 `nous-engine` 的 |
//! |---|---|---|
//! | 新建 | `POST /v1/{kind}/create` | `skill_create` / `prompt_create` / `extension_create` |
//! | 打开编辑器 | `GET /v1/{kind}/files` + `GET /v1/{kind}/file` | `resource_file_list` / `resource_file_read` |
//! | 存草稿 | `PUT /v1/{kind}/file`（带 `base`） | `resource_save` |
//! | 发布 | `POST /v1/{kind}/publish` | `resource_publish` |
//! | 归档 / 恢复 / 真删 | `POST /v1/{kind}/{op}` | `resource_req` |
//!
//! 四条落地决定（`decisions.md` 2026-11-16）：
//! 1. 三张新建表各说各的必填项，右边**实时预览落盘内容**；
//! 2. 建完停在「未发布」并选中，**不自动发布**；提示词建完直接进编辑器；
//! 3. 发布**三个仓各走各的**，不做一键全发；
//! 4. 编辑是**整页工作台**，不是右栏抽屉。
//!
//! 一条铁律：**校验和文案都在引擎**。这里只做「必填项空没空」这种提交前的兜底，
//! 引擎回什么就显示什么（`EngineError::Engine{message}` 原样），不自己编错误话术。

use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;
use nous_engine::resource::{NewExtension, NewPrompt, NewSkill, Repo, ResourceFileEntry, SaveFile};

use crate::app::{Desktop, NoticeKind};
use crate::resources::Selected;

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 三种条目。`Section` + `Content` 是**布局**（两段 + 子开关），这个是**数据**（哪个仓）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Skill,
    Prompt,
    Ext,
}

impl Kind {
    pub(crate) fn repo(self) -> Repo {
        match self {
            Self::Skill => Repo::Skills,
            Self::Prompt => Repo::Prompts,
            Self::Ext => Repo::Extensions,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Skill => "技能",
            Self::Prompt => "提示词",
            Self::Ext => "扩展",
        }
    }

    /// 编辑器打开的第一个文件：正文优先，不是先看清单。
    fn primary_file(self) -> &'static str {
        match self {
            Self::Skill => "SKILL.md",
            Self::Prompt => "PROMPT.md",
            Self::Ext => "index.js",
        }
    }
}

impl Selected {
    pub(crate) fn kind_of(&self) -> Kind {
        match self {
            Self::Skill(_) => Kind::Skill,
            Self::Prompt(_) => Kind::Prompt,
            Self::Ext(_) => Kind::Ext,
        }
    }

    pub(crate) fn name_of(&self) -> &str {
        match self {
            Self::Skill(n) | Self::Prompt(n) | Self::Ext(n) => n,
        }
    }
}

/// 四个互斥视图。浏览是只读那一刀画的三栏，其余三个占满列表 + 详情。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum View {
    #[default]
    Browse,
    New,
    Edit,
    Publish,
}

/// 新建表单。三个仓的字段集不一样，所以「那个必填的长文本」共用 `text` 一个输入框，
/// 只是标签和落盘位置随 `kind` 变（技能 → `description`，其余 → `summary`）。
pub(crate) struct NewForm {
    pub(crate) kind: Kind,
    pub(crate) name: Entity<InputState>,
    pub(crate) text: Entity<InputState>,
    /// 只有提示词用：正文可以由调用方给，不给就落一个 `# 名字` 的标题。
    pub(crate) body: Entity<InputState>,
    pub(crate) category: Entity<InputState>,
    pub(crate) busy: bool,
    pub(crate) err: Option<String>,
}

/// 一次编辑会话：文件清单 + 当前文件正文 + 并发闸。
pub(crate) struct EditSession {
    pub(crate) kind: Kind,
    pub(crate) name: String,
    pub(crate) files: Vec<ResourceFileEntry>,
    pub(crate) active: String,
    pub(crate) editor: Entity<TextareaState>,
    /// 写回时当 `base`；撞了就是 409，用户得先重读（别丢输入）。
    pub(crate) etag: String,
    /// 载入时的正文 —— 判「有没有改动」就比这个。
    pub(crate) saved: String,
    pub(crate) loading: bool,
    pub(crate) busy: bool,
    pub(crate) err: Option<String>,
}

impl EditSession {
    pub(crate) fn dirty(&self, current: &str) -> bool {
        is_dirty(&self.saved, current)
    }
}

/// 「有没有改动」= 正文跟载入时不一样。抽成纯函数，好在没有 GPUI 上下文的地方单测。
pub(crate) fn is_dirty(saved: &str, current: &str) -> bool {
    saved != current
}

/// 发布面板：三个仓各自的忙碌 / 成功 / 失败。**不合并**成一个总状态。
#[derive(Default)]
pub(crate) struct PublishState {
    pub(crate) busy: [bool; 3],
    pub(crate) done: [bool; 3],
    pub(crate) err: [Option<String>; 3],
}

impl PublishState {
    pub(crate) fn idx(repo: Repo) -> usize {
        match repo {
            Repo::Skills => 0,
            Repo::Prompts => 1,
            Repo::Extensions => 2,
        }
    }
}

/// 确认层要干的那件事。不可逆的动作（真删）走这里，不在列表上直接下手。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmAct {
    Archive,
    Restore,
    Purge,
    /// 编辑器里有没存的改动，关掉前问一句。
    DiscardEdit,
}

pub(crate) struct Confirm {
    pub(crate) kind: Kind,
    pub(crate) name: String,
    pub(crate) act: ConfirmAct,
    pub(crate) busy: bool,
    pub(crate) err: Option<String>,
}

impl Confirm {
    pub(crate) fn title(&self) -> String {
        match self.act {
            ConfirmAct::Archive => format!("归档 {}？", self.name),
            ConfirmAct::Restore => format!("恢复 {}？", self.name),
            ConfirmAct::Purge => format!("真删 {}？", self.name),
            ConfirmAct::DiscardEdit => "有没存的改动".into(),
        }
    }

    pub(crate) fn body(&self) -> String {
        match self.act {
            ConfirmAct::Archive => "会 git mv 到 .archive/ 并 commit + push —— 别人也看不见了。想回来点「恢复」，内容还在。".into(),
            ConfirmAct::Restore => "从 .archive/ 搬回仓库并 commit + push —— 回到「已发布」。".into(),
            ConfirmAct::Purge => "不可逆。会 git rm -r 掉归档区那份并 commit + push —— 恢复不了。".into(),
            ConfirmAct::DiscardEdit => "这些改动只在编辑器里，关掉就没了。".into(),
        }
    }

    pub(crate) fn yes(&self) -> &'static str {
        match self.act {
            ConfirmAct::Archive => "归档",
            ConfirmAct::Restore => "恢复",
            ConfirmAct::Purge => "真删",
            ConfirmAct::DiscardEdit => "不存，关掉",
        }
    }

    pub(crate) fn danger(&self) -> bool {
        matches!(self.act, ConfirmAct::Purge | ConfirmAct::DiscardEdit)
    }
}

// ── 纯函数（好单测）────────────────────────────────────────────────────

/// 类别输入框 → 词表。逗号（中英）+ 空白都算分隔，跟引擎 `normalizeCategories` 同口径。
pub(crate) fn parse_categories(raw: &str) -> Vec<String> {
    raw.split([',', '，', '\n', '\t', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 新建时**马上要落盘的内容**。照引擎 `createSkill` / `createPrompt` / `createExtension`
/// 的真实形状写，跟 demo 里那块预览一模一样：
///
/// - 技能：`SKILL.md`（frontmatter 里 `description`）+ `nous.json`（`summary` 是空串）
/// - 提示词：`PROMPT.md`（纯正文，**没有 frontmatter**）+ `nous.json`（`summary` 在这）
/// - 扩展：`nous.json` 清单 + `index.js` 空壳入口
pub(crate) fn landing_files(
    kind: Kind,
    name: &str,
    text: &str,
    category: &str,
    body: &str,
    root: &str,
) -> Vec<(String, String)> {
    let name = name.trim();
    let name = if name.is_empty() { "<name>" } else { name };
    let cats = parse_categories(category);
    let dir = format!("{}/{}/", root.trim_end_matches('/'), name);

    // JSON 手拼而不是 `serde_json::to_string_pretty`：字段顺序要跟引擎一致，
    // 不然预览里的顺序会跟真落盘的对不上（引擎是 `{name, summary, category}`）。
    let cats_json = if cats.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = cats
            .iter()
            .map(|c| format!("    {}", json_str(c)))
            .collect();
        format!("[\n{}\n  ]", items.join(",\n"))
    };

    match kind {
        Kind::Skill => {
            let desc = if text.trim().is_empty() {
                "<description>"
            } else {
                text.trim()
            };
            let sk = format!(
                "---\nname: {name}\ndescription: {}\n---\n\n# {name}\n",
                json_str(desc)
            );
            let manifest = format!(
                "{{\n  \"name\": {},\n  \"summary\": \"\",\n  \"category\": {}\n}}\n",
                json_str(name),
                cats_json
            );
            vec![
                (format!("{dir}SKILL.md"), sk),
                (format!("{dir}nous.json"), manifest),
            ]
        }
        Kind::Prompt => {
            let body = if body.trim().is_empty() {
                format!("# {name}\n")
            } else {
                body.to_string()
            };
            let manifest = format!(
                "{{\n  \"name\": {},\n  \"summary\": {},\n  \"category\": {}\n}}\n",
                json_str(name),
                json_str(text.trim()),
                cats_json
            );
            vec![
                (format!("{dir}PROMPT.md"), body),
                (format!("{dir}nous.json"), manifest),
            ]
        }
        Kind::Ext => {
            let summary = if text.trim().is_empty() {
                "<一句话说明>"
            } else {
                text.trim()
            };
            let cat_line = if cats.is_empty() {
                String::new()
            } else {
                format!(",\n  \"category\": {}", cats_json.replace('\n', "\n  "))
            };
            let manifest = format!(
                "{{\n  \"name\": {},\n  \"version\": \"0.1.0\",\n  \"file\": \"index.js\",\n  \"entry\": \"default\",\n  \"sections\": [],\n  \"summary\": {},\n  \"needs\": {{}}{cat_line}\n}}\n",
                json_str(name),
                json_str(summary)
            );
            // 真实入口文件前面是一大段注释（引擎的 SKELETON_JS），最后一行是这个；
            // 这里只摆那一行 —— 不把引擎的注释抄一份过来跟着烂。
            let js = format!(
                "// {name} —— {summary}\n// （引擎还会在文件头附一段上手注释，这里只摆最后一行）\nexport default ({{ defineExtension }}) => defineExtension({{ name: \"{name}\" }});\n"
            );
            vec![
                (format!("{dir}nous.json"), manifest),
                (format!("{dir}index.js"), js),
            ]
        }
    }
}

/// JSON 字符串字面量（只处理这几个仓会出现的字符：引号、反斜杠、换行、tab）。
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 提交前的兜底：只挡「必填项空着」这一种，别的错误话术归引擎。
pub(crate) fn missing_field(kind: Kind, name: &str, text: &str) -> Option<&'static str> {
    if name.trim().is_empty() {
        return Some(match kind {
            Kind::Skill => "技能名不能为空",
            Kind::Prompt => "模板名不能为空",
            Kind::Ext => "扩展名不能为空",
        });
    }
    if text.trim().is_empty() {
        return Some(match kind {
            Kind::Skill => "description 不能空 —— 模型靠它判断该不该用这条技能",
            Kind::Prompt => "summary 不能空 —— 人靠这一行决定选不选它",
            Kind::Ext => "一句话说明不能空 —— 卡片上就靠它认人",
        });
    }
    None
}

// ── 动作 ───────────────────────────────────────────────────────────────

impl Desktop {
    /// `Desk#13` 返工：等票的能力型端点在引擎声明前保持灰。
    /// 判定与理由只走 `capability.rs`；这里只负责「不发请求 + 把理由给人看」。
    fn resource_write_gate(&self, method: &'static str, path: &str) -> Option<String> {
        self.endpoint_disabled_reason(method, path)
    }

    /// 新建：三个仓各开一张表。**必填项不相同**，所以不是一张通用表。
    pub(crate) fn resource_new(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let mut input = |ph: &'static str| cx.new(|cx| InputState::new(window, cx).placeholder(ph));

        let name = input(match kind {
            Kind::Skill => "weekly-digest",
            Kind::Prompt => "strict-review",
            Kind::Ext => "weekly-card",
        });
        let text = input(match kind {
            Kind::Skill => "把这几天的会议、邮件、聊天整理成一份周报",
            Kind::Prompt => "审查时按标准 + 规格两条轴走，不混成一股感觉",
            Kind::Ext => "把一周的提交整理成一张能贴出去的图",
        });
        let body = input("正文（可空；空着就落一个 # 名字 的标题）");
        let category = input("写作, 效率");

        // 四个框都要在打字时重画右边那块预览 —— 没有订阅就只有一个静止的快照。
        let mut subs = Vec::new();
        for e in [&name, &text, &body, &category] {
            subs.push(cx.subscribe_in(e, window, |_, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }

        self.resources.write_subs.extend(subs);
        self.resources.form = Some(NewForm {
            kind,
            name,
            text,
            body,
            category,
            busy: false,
            err: None,
        });
        self.resources.view = View::New;
        cx.notify();
    }

    pub(crate) fn resource_cancel_new(&mut self, cx: &mut Context<Self>) {
        self.resources.form = None;
        self.resources.view = View::Browse;
        cx.notify();
    }

    /// 提交新建。成功之后：刷新清单 → 选中新条目 → 停在「未发布」，**不发布**。
    pub(crate) fn resource_submit_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        let Some(form) = self.resources.form.as_ref() else { return };
        let kind = form.kind;
        if let Some(reason) = self.resource_write_gate("POST", &format!("/v1/{}/create", kind.repo().key())) {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let name = form.name.read(cx).value().trim().to_string();
        let text = form.text.read(cx).value().trim().to_string();
        let body = form.body.read(cx).value().to_string();
        let category = parse_categories(form.category.read(cx).value().as_ref());

        if let Some(msg) = missing_field(kind, &name, &text) {
            if let Some(f) = self.resources.form.as_mut() {
                f.err = Some(msg.to_string());
            }
            cx.notify();
            return;
        }
        if let Some(f) = self.resources.form.as_mut() {
            f.busy = true;
            f.err = None;
        }
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let repo = kind.repo();
            let (n, d) = (name.clone(), text.clone());
            let eng = engine.clone();
            let res = handle
                .spawn(async move {
                    match kind {
                        Kind::Skill => {
                            eng.skill_create(&NewSkill {
                                    name: n,
                                    description: d,
                                    category,
                                })
                                .await
                        }
                        Kind::Prompt => {
                            eng.prompt_create(&NewPrompt {
                                    name: n,
                                    summary: d,
                                    category,
                                    body: (!body.trim().is_empty()).then_some(body),
                                })
                                .await
                        }
                        Kind::Ext => {
                            eng.extension_create(&NewExtension {
                                    name: n,
                                    summary: d,
                                    category,
                                })
                                .await
                        }
                    }
                })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                let Some(f) = this.resources.form.as_mut() else { return };
                match res {
                    Ok(Ok(_)) => {
                        f.busy = false;
                        this.resources.form = None;
                        this.resources.view = View::Browse;
                        this.resources.selected =
                            Some(match kind.repo() {
                                Repo::Skills => Selected::Skill(name.clone()),
                                Repo::Prompts => Selected::Prompt(name.clone()),
                                Repo::Extensions => Selected::Ext(name.clone()),
                            });
                        // 清单重拉：新条目要出现在列表里，计数 / 类别树也跟着变。
                        this.load_resources(true, cx);
                        this.notify(
                            format!("建好了：{name} · 未发布（只在这台机器上看得见）"),
                            NoticeKind::Info,
                            cx,
                        );
                        // 提示词建完直接进编辑器：它的主要内容就是正文，落在页面上等一句话没有意义。
                        if kind == Kind::Prompt {
                            this.resource_open_edit(window, cx);
                        }
                        let _ = repo;
                    }
                    Ok(Err(e)) => {
                        f.busy = false;
                        f.err = Some(format!("{e}"));
                    }
                    Err(e) => {
                        f.busy = false;
                        f.err = Some(format!("任务挂了：{e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 打开整页编辑器：先拉文件清单，再读正文文件。
    pub(crate) fn resource_open_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let repo = self.resources.selected.as_ref()
            .map(|s| s.kind_of().repo().key()).unwrap_or("skills");
        if let Some(reason) = self.resource_write_gate("GET", &format!("/v1/{repo}/files")) {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        let Some(sel) = self.resources.selected.clone() else { return };
        let kind = sel.kind_of();
        let name = sel.name_of().to_string();

        let editor = cx.new(|cx| TextareaState::new(window, cx).placeholder("正文…"));
        // 订阅要**收起来**（丢弃 = 立刻取消，编辑器里打字就不会重画了）。
        let sub = cx.subscribe_in(&editor, window, |_, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        self.resources.write_subs.push(sub);

        self.resources.edit = Some(EditSession {
            kind,
            name: name.clone(),
            files: Vec::new(),
            active: kind.primary_file().to_string(),
            editor,
            etag: String::new(),
            saved: String::new(),
            loading: true,
            busy: false,
            err: None,
        });
        self.resources.view = View::Edit;
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let repo = kind.repo().key();
            let (n, first) = (name.clone(), kind.primary_file().to_string());
            let res = handle
                .spawn(async move {
                    let list = engine.resource_file_list(repo, &n).await?;
                    // 清单里没有主文件（比如扩展只留了 nous.json）就退回清单第一行。
                    let active = if list.files.iter().any(|f| f.path == first) {
                        first
                    } else {
                        list.ordered()
                            .first()
                            .map(|f| f.path.clone())
                            .unwrap_or(first)
                    };
                    let file = engine.resource_file_read(repo, &n, &active).await?;
                    Ok::<_, nous_engine::EngineError>((list.files, active, file))
                })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                let Some(e) = this.resources.edit.as_mut() else { return };
                match res {
                    Ok(Ok((files, active, file))) => {
                        e.loading = false;
                        e.files = files;
                        e.active = active;
                        e.etag = file.etag;
                        e.saved = file.text.clone();
                        e.editor.update(cx, |s, cx| {
                            s.set_value(file.text, window, cx);
                        });
                    }
                    Ok(Err(err)) => {
                        e.loading = false;
                        e.err = Some(format!("{err}"));
                    }
                    Err(err) => {
                        e.loading = false;
                        e.err = Some(format!("任务挂了：{err}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 切到另一个文件：先读，读到才改 `active`（不然点了个打不开的会白丢当前正文）。
    pub(crate) fn resource_pick_file(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self.engine.clone() else { return };
        let Some(e) = self.resources.edit.as_ref() else { return };
        if e.active == path {
            return;
        }
        let (kind, name) = (e.kind, e.name.clone());
        // 有未存的改动就先存下来再换文件 —— 换文件不该等于「悄悄丢掉刚才写的」。
        let current = e.editor.read(cx).value().to_string();
        let dirty = e.dirty(&current);
        let (etag, old_path) = (e.etag.clone(), e.active.clone());
        let current_saved = current.clone();

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let repo = kind.repo().key();
            let eng = engine.clone();
            let saved = if dirty {
                let (n, p, txt, base) = (name.clone(), old_path, current, etag);
                Some(handle.spawn(async move {
                    eng.resource_save(
                            repo,
                            &SaveFile { name: &n, path: &p, text: &txt, base: Some(&base) },
                        )
                        .await
                }).await)
            } else {
                None
            };
            let (n2, p2) = (name.clone(), path.clone());
            let read = handle
                .spawn(async move { engine.resource_file_read(repo, &n2, &p2).await })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                let Some(e) = this.resources.edit.as_mut() else { return };
                // 先写的那个文件成功了就把 etag 记下来，免得下次又撞。
                if let Some(Ok(Ok(_))) = saved {
                    e.saved = current_saved;
                }
                match read {
                    Ok(Ok(file)) => {
                        e.active = path;
                        e.etag = file.etag;
                        e.saved = file.text.clone();
                        e.err = None;
                        e.editor.update(cx, |s, cx| {
                            s.set_value(file.text, window, cx);
                        });
                    }
                    Ok(Err(err)) => e.err = Some(format!("{err}")),
                    Err(err) => e.err = Some(format!("任务挂了：{err}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 存草稿：只落盘、不提交。
    pub(crate) fn resource_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let repo = self.resources.edit.as_ref().map(|e| e.kind.repo().key()).unwrap_or("skills");
        if let Some(reason) = self.resource_write_gate("PUT", &format!("/v1/{repo}/file")) {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else { return };
        let Some(e) = self.resources.edit.as_ref() else { return };
        let (kind, name, path) = (e.kind, e.name.clone(), e.active.clone());
        let text = e.editor.read(cx).value().to_string();
        let etag = e.etag.clone();
        if !e.dirty(&text) {
            return;
        }
        if let Some(e) = self.resources.edit.as_mut() {
            e.busy = true;
            e.err = None;
        }
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let repo = kind.repo().key();
            let (n, p, base) = (name.clone(), path.clone(), etag.clone());
            let (txt, txt2) = (text.clone(), text.clone());
            let res = handle
                .spawn(async move {
                    engine
                        .resource_save(
                            repo,
                            &SaveFile { name: &n, path: &p, text: &txt, base: Some(&base) },
                        )
                        .await
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                let Some(e) = this.resources.edit.as_mut() else { return };
                e.busy = false;
                match res {
                    Ok(Ok(v)) => {
                        e.saved = txt2;
                        // 引擎回 `{etag}` 就换上；没回就留着旧的（下次写会 409，比静默覆盖好）。
                        if let Some(new_etag) = v.get("etag").and_then(|x| x.as_str()) {
                            e.etag = new_etag.to_string();
                        }
                        e.err = None;
                        this.notify("存好了（只写盘，还没提交）", NoticeKind::Info, cx);
                    }
                    Ok(Err(err)) => e.err = Some(format!("{err}")),
                    Err(err) => e.err = Some(format!("任务挂了：{err}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 关编辑器：脏了就问一句，别让人一失手丢掉刚写的。
    pub(crate) fn resource_close_edit(&mut self, cx: &mut Context<Self>) {
        let dirty = self.resources.edit.as_ref().is_some_and(|e| {
            let cur = e.editor.read(cx).value().to_string();
            e.dirty(&cur)
        });
        if dirty {
            let (kind, name) = self
                .resources
                .edit
                .as_ref()
                .map(|e| (e.kind, e.name.clone()))
                .unwrap_or((Kind::Skill, String::new()));
            self.resources.confirm = Some(Confirm {
                kind,
                name,
                act: ConfirmAct::DiscardEdit,
                busy: false,
                err: None,
            });
        } else {
            self.resources.edit = None;
            self.resources.view = View::Browse;
        }
        cx.notify();
    }

    pub(crate) fn resource_open_publish(&mut self, cx: &mut Context<Self>) {
        self.resources.publish = PublishState::default();
        self.resources.view = View::Publish;
        cx.notify();
    }

    /// 发布**一个**仓。三个仓各走各的，谁失败只染谁。
    pub(crate) fn resource_publish(&mut self, repo: Repo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(reason) = self.resource_write_gate("POST", &format!("/v1/{}/publish", repo.key())) {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        let i = PublishState::idx(repo);
        self.resources.publish.busy[i] = true;
        self.resources.publish.err[i] = None;
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let res = handle
                .spawn(async move { engine.resource_publish(repo.key(), None).await })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                this.resources.publish.busy[i] = false;
                match res {
                    Ok(Ok(v)) => {
                        this.resources.publish.done[i] = true;
                        // 引擎回的 warnings（比如「未发布的那几条没跟着这次提交走」）原样带上。
                        let warn = v
                            .get("warnings")
                            .and_then(|w| w.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            })
                            .unwrap_or_default();
                        this.notify(
                            if warn.is_empty() {
                                format!("{}仓：已提交并推送", repo.label())
                            } else {
                                format!("{}仓：已推送，但 {warn}", repo.label())
                            },
                            NoticeKind::Info,
                            cx,
                        );
                        this.load_resources(true, cx);
                        let _ = window;
                    }
                    Ok(Err(e)) => this.resources.publish.err[i] = Some(format!("{e}")),
                    Err(e) => this.resources.publish.err[i] = Some(format!("任务挂了：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 装 / 卸：不是「装进来的包」，是「这台带不带它」。引擎装完会立刻重装注册表，
    /// 所以这里也跟着重拉清单 —— 不然页面说装着、会话里没有，两份真相就这么长出来。
    pub(crate) fn resource_ext_toggle(
        &mut self,
        install: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        let Some(sel) = self.resources.selected.clone() else { return };
        let name = sel.name_of().to_string();
        let op = if install { "install" } else { "uninstall" };
        let body = serde_json::json!({ "name": name });

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let reload_engine = engine.clone();
            let res = handle
                .spawn(async move { engine.resource_op("extensions", op, &body).await })
                .await;

            // 装 / 卸是盘面动作，注册表要引擎自己重装一次才算数
            // （`POST /v1/extensions/reload`）。以前那句「引擎装完会立刻重装」是新内核
            // 之前的行为，现在要显式喊；不喊就是「页面说装着、会话里没有」。
            let reload = match &res {
                Ok(Ok(_)) => Some(reload_engine.ext_reload().await),
                _ => None,
            };

            let _ = this.update_in(cx, |this, window, cx| {
                match res {
                    Ok(Ok(_)) => {
                        this.notify(
                            match (install, reload.map(|r| r.is_ok())) {
                                (true, Some(true)) => format!("装上了 {name} —— 注册表已重装（优先从垃圾站取回）"),
                                (true, _) => format!("装上了 {name}（重装注册表没成功，清单可能还是旧的）"),
                                (false, _) => format!("卸载了 {name} —— 目录搬进垃圾站（data/extend-trash），装回来优先拿它"),
                            },
                            NoticeKind::Info,
                            cx,
                        );
                        this.load_resources(true, cx);
                    }
                    // 核心三条卸载会被引擎 403 挡回来：把原话显示出来，不自己编一句。
                    Ok(Err(e)) => this.notify(format!("{e}"), NoticeKind::Error, cx),
                    Err(e) => this.notify(format!("任务挂了：{e}"), NoticeKind::Error, cx),
                }
                let _ = window;
                cx.notify();
            });
        })
        .detach();
    }

    /// 重载：按盘上现状重装注册表（`POST /v1/extensions/reload`）。
    ///
    /// 为什么要有这个按钮：扩展是「盘上目录 + 运行中的进程」，手工放了文件 / 改了清单
    /// 之后，不喊一次运行时根本不知道。以前只能重启引擎 —— 那会把会话一起带走。
    pub(crate) fn resource_ext_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(reason) = self.resource_write_gate("POST", "/v1/extensions/reload") {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let res = handle.spawn(async move { engine.ext_reload().await }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match res {
                    Ok(Ok(v)) => {
                        let n = v
                            .get("reloaded")
                            .and_then(|x| x.as_array())
                            .map(|rows| rows.len())
                            .unwrap_or(0);
                        this.notify(format!("重载完成：{n} 条"), NoticeKind::Info, cx);
                        this.load_resources(true, cx);
                    }
                    // 失败的扩展会保留旧进程与旧状态（引擎语义）—— 原话照登，不自己编。
                    Ok(Err(e)) => this.notify(format!("{e}"), NoticeKind::Error, cx),
                    Err(e) => this.notify(format!("任务挂了：{e}"), NoticeKind::Error, cx),
                }
                let _ = window;
                cx.notify();
            });
        })
        .detach();
    }

    /// 复制：把源码根里的目录复制成 `<名字>-copy`（`POST /v1/extensions/duplicate`）。
    ///
    /// 不起名、不弹表单：复制是「照着这条再开一份」的第一步，名字随时能改；
    /// 多一层输入框只是把一次点击变成三次。重名由引擎挡住，原话照登。
    pub(crate) fn resource_ext_duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(reason) = self.resource_write_gate("POST", "/v1/extensions/duplicate") {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else {
            self.notify("还没连上引擎", NoticeKind::Error, cx);
            return;
        };
        let Some(sel) = self.resources.selected.clone() else { return };
        let name = sel.name_of().to_string();
        let to = format!("{name}-copy");

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let (n, target) = (name.clone(), to.clone());
            let res = handle
                .spawn(async move { engine.ext_duplicate(&n, &target).await })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                match res {
                    Ok(Ok(_)) => {
                        this.notify(format!("复制成 {to} —— 在仓库里，改完记得发布"), NoticeKind::Info, cx);
                        this.resources.selected = Some(Selected::Ext(to.clone()));
                        this.load_resources(true, cx);
                    }
                    Ok(Err(e)) => this.notify(format!("{e}"), NoticeKind::Error, cx),
                    Err(e) => this.notify(format!("任务挂了：{e}"), NoticeKind::Error, cx),
                }
                let _ = window;
                cx.notify();
            });
        })
        .detach();
    }

    /// 归档 / 恢复 / 真删：先摆确认层，确认了才发请求。
    pub(crate) fn resource_ask_confirm(&mut self, act: ConfirmAct, cx: &mut Context<Self>) {
        let Some(sel) = self.resources.selected.clone() else { return };
        self.resources.confirm = Some(Confirm {
            kind: sel.kind_of(),
            name: sel.name_of().to_string(),
            act,
            busy: false,
            err: None,
        });
        cx.notify();
    }

    pub(crate) fn resource_confirm_no(&mut self, cx: &mut Context<Self>) {
        self.resources.confirm = None;
        cx.notify();
    }

    pub(crate) fn resource_confirm_yes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let kind = self.resources.confirm.as_ref().map(|c| c.kind);
        // 归档 / 恢复 / 真删是**仓级**动作（`/v1/repos/{repo}/{op}`），不是文件面的
        // `/v1/{kind}/{op}` —— 后者在引擎里根本不存在，以前点「归档」会静默 404。
        // 三条路径逐个写死（不拼模板）：台账扫描器要看见「客户端真会打的这一条」。
        let gate_path = match kind {
            Some(Kind::Skill) => "/v1/repos/skills/archive",
            Some(Kind::Prompt) => "/v1/repos/prompts/archive",
            _ => "/v1/repos/extend/archive",
        };
        if let Some(reason) = self.resource_write_gate("POST", gate_path) {
            self.notify(reason, NoticeKind::Error, cx);
            return;
        }
        let Some(engine) = self.engine.clone() else { return };
        let Some(c) = self.resources.confirm.as_mut() else { return };
        let (kind, name, act) = (c.kind, c.name.clone(), c.act);
        if act == ConfirmAct::DiscardEdit {
            self.resources.confirm = None;
            self.resources.edit = None;
            self.resources.view = View::Browse;
            cx.notify();
            return;
        }
        c.busy = true;
        c.err = None;
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let op = match act {
                ConfirmAct::Archive => "archive",
                ConfirmAct::Restore => "restore",
                ConfirmAct::Purge => "purge",
                ConfirmAct::DiscardEdit => unreachable!(),
            };
            let repo = kind.repo().repo_key();
            let (n, body) = (name.clone(), serde_json::json!({ "name": name }));
            let res = handle
                .spawn(async move { engine.repo_op(repo, op, &body).await })
                .await;

            let _ = this.update_in(cx, |this, window, cx| {
                let Some(c) = this.resources.confirm.as_mut() else { return };
                c.busy = false;
                match res {
                    Ok(Ok(_)) => {
                        let what = match act {
                            ConfirmAct::Archive => "归档了（搬进 .archive/ 并提交）",
                            ConfirmAct::Restore => "恢复回来了",
                            ConfirmAct::Purge => "真删了，归档区那份也没了",
                            ConfirmAct::DiscardEdit => unreachable!(),
                        };
                        this.resources.confirm = None;
                        this.resources.selected = None;
                        this.load_resources(true, cx);
                        this.notify(format!("{n} {what}"), NoticeKind::Info, cx);
                        let _ = window;
                    }
                    Ok(Err(e)) => c.err = Some(format!("{e}")),
                    Err(e) => c.err = Some(format!("任务挂了：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

// ── 渲染 ───────────────────────────────────────────────────────────────

fn muted() -> u32 {
    t().p.muted
}
fn text_color() -> u32 {
    t().p.text
}

fn btn(id: &str, label: &str, primary: bool) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .h(t().d(28.))
        .px(t().d(12.))
        .rounded(t().r(7.))
        .text_size(t().fs(11.5))
        .bg(rgb(if primary { t().p.accent } else { t().p.hover }))
        .text_color(rgb(if primary { t().p.on_accent } else { t().p.text_2 }))
        .hover(|el| el.bg(rgb(if primary { t().p.accent_hover } else { t().p.line })))
        .child(label.to_string())
}

fn ghost_btn(id: &str, label: &str) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .h(t().d(28.))
        .px(t().d(10.))
        .rounded(t().r(7.))
        .border(t().d(1.))
        .border_color(rgb(t().p.line))
        .text_size(t().fs(11.5))
        .text_color(rgb(t().p.text_2))
        .hover(|el| el.bg(rgb(t().p.hover)))
        .child(label.to_string())
}

fn code_block(text: &str) -> AnyElement {
    div()
        .p(t().d(10.))
        .rounded(t().r(7.))
        .bg(rgb(t().p.code_bg))
        .border(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .text_size(t().fs(11.))
        .line_height(t().d(17.))
        .font_family("JetBrainsMono")
        .text_color(rgb(t().p.text_2))
        .child(text.to_string())
        .into_any_element()
}

fn page_head(title: &str, sub: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(t().d(3.))
        .px(t().d(22.))
        .py(t().d(14.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .text_size(t().fs(15.))
                .text_color(rgb(text_color()))
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(muted()))
                .child(sub.to_string()),
        )
}

fn field(label: &str, req: bool, hint: &str, input: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(t().d(5.))
        .child(
            div()
                .flex()
                .gap(t().d(2.))
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.text_2))
                .child(label.to_string())
                .when(req, |el| {
                    el.child(
                        div()
                            .text_size(t().fs(11.5))
                            .text_color(rgb(t().p.danger))
                            .child("*"),
                    )
                }),
        )
        .when(!hint.is_empty(), |el| {
            el.child(
                div()
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.faint))
                    .line_height(t().d(16.))
                    .child(hint.to_string()),
            )
        })
        .child(input)
        .into_any_element()
}

/// 新建：左边表单、右边落盘预览。改一个字右边跟着变（订阅在 `resource_new` 里）。
pub(crate) fn render_new(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let Some(form) = desk.resources.form.as_ref() else {
        return div().into_any_element();
    };
    let kind = form.kind;
    let busy = form.busy;
    let name = form.name.read(cx).value().to_string();
    let text = form.text.read(cx).value().to_string();
    let body = form.body.read(cx).value().to_string();
    let category = form.category.read(cx).value().to_string();
    let root = repo_root(desk, kind.repo());

    let files = landing_files(kind, &name, &text, &category, &body, &root);

    let (text_label, text_hint) = match kind {
        Kind::Skill => ("description", "模型靠这一行判断该不该用这条技能 —— 引擎会拒空的。写进 SKILL.md 的 frontmatter。"),
        Kind::Prompt => ("summary", "人靠这一行决定选不选它 —— 写进 nous.json。正文进编辑器里写。"),
        Kind::Ext => ("一句话说明", "卡片上就靠它认人 —— 必填，≤ 200 字。写进 nous.json 的 summary。"),
    };
    let name_hint = match kind {
        Kind::Ext => "小写字母 / 数字 / 连字符，建完就是目录名。核心三条（nous / skills / prompts）不让用。",
        _ => "正则 ^[a-z0-9][a-z0-9._-]*$，≤ 64 字。名字就是目录名，新建那一刻定型。",
    };

    let mut left = div()
        .flex()
        .flex_col()
        .w_full()
        .gap(t().d(15.))
        .px(t().d(22.))
        .py(t().d(18.))
        .child(field(
            "名字",
            true,
            name_hint,
            div()
                .w_full()
                .h(t().d(31.))
                .child(Input::new(&form.name).appearance(true))
                .into_any_element(),
        ))
        .child(field(
            text_label,
            true,
            text_hint,
            div()
                .w_full()
                .h(t().d(70.))
                .child(Input::new(&form.text).appearance(true))
                .into_any_element(),
        ));

    if kind == Kind::Prompt {
        left = left.child(field(
            "正文（可空）",
            false,
            "空着就落一个 # 名字 的标题 —— 标题也会进提示词。",
            div()
                .w_full()
                .h(t().d(70.))
                .child(Input::new(&form.body).appearance(true))
                .into_any_element(),
        ));
    }
    left = left.child(field(
        "类别（逗号分隔，可空）",
        false,
        "写进 nous.json 的 category，父类别不存在会自动补进词表。",
        div()
            .w_full()
            .h(t().d(31.))
            .child(Input::new(&form.category).appearance(true))
            .into_any_element(),
    ));

    if let Some(err) = form.err.as_ref() {
        left = left.child(
            div()
                .px(t().d(10.))
                .py(t().d(8.))
                .rounded(t().r(6.))
                .bg(rgb(t().p.danger_soft))
                .text_size(t().fs(11.5))
                .line_height(t().d(17.))
                .text_color(rgb(t().p.danger_fg))
                .child(err.clone()),
        );
    }

    let mut right = div()
        .flex()
        .flex_col()
        .gap(t().d(10.))
        .id("res-new-preview")
        .w(t().d(400.))
        .flex_shrink_0()
        .min_h(t().d(0.))
        .overflow_y_scroll()
        .border_l(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .bg(rgb(t().p.panel_alt))
        .px(t().d(18.))
        .py(t().d(18.))
        .child(
            div()
                .text_size(t().fs(11.))
                .text_color(rgb(muted()))
                .child("落盘预览（改一个字这里就跟着变）"),
        );
    for (path, content) in &files {
        right = right
            .child(
                div()
                    .pt(t().d(4.))
                    .text_size(t().fs(10.5))
                    .font_family("JetBrainsMono")
                    .text_color(rgb(t().p.link))
                    .child(path.clone()),
            )
            .child(code_block(content));
    }

    let foot = div()
        .flex()
        .items_center()
        .gap(t().d(10.))
        .px(t().d(22.))
        .py(t().d(11.))
        .border_t(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .flex_1()
                .text_size(t().fs(11.5))
                .text_color(rgb(muted()))
                .child(if busy {
                    "正在建…".to_string()
                } else {
                    "建完停在「未发布」，不自动发布".to_string()
                }),
        )
        .child(
            ghost_btn("res-new-cancel", "取消").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.resource_cancel_new(cx);
            })),
        )
        .child(
            btn(
                "res-new-submit",
                if kind == Kind::Prompt { "建，去编辑正文" } else { "建，去编辑" },
                true,
            )
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.resource_submit_new(window, cx);
            })),
        );

    div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w(t().d(0.))
        .bg(rgb(t().p.panel))
        .child(page_head(
            &format!("新建{}", kind.label()),
            "必填项三张表各不相同；右边是马上要落盘的内容，边填边看。",
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h(t().d(0.))
                .child(
                    div()
                        .id("res-new-form")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(t().d(0.))
                        .min_h(t().d(0.))
                        .overflow_y_scroll()
                        .child(left),
                )
                .child(right),
        )
        .child(foot)
        .into_any_element()
}

/// 编辑：整页工作台（文件树 + 正文 + 状态），不是盖在列表上的右栏抽屉。
pub(crate) fn render_edit(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let Some(e) = desk.resources.edit.as_ref() else {
        return div().into_any_element();
    };
    let current = e.editor.read(cx).value().to_string();
    let dirty = e.dirty(&current);
    let lines = current.lines().count();
    let added = lines as i64 - e.saved.lines().count() as i64;

    let mut tree = div()
        .flex()
        .flex_col()
        .gap(t().d(1.))
        .w(t().d(200.))
        .flex_shrink_0()
        .bg(rgb(t().p.list_bg))
        .border_r(t().d(1.))
        .border_color(rgb(t().p.line))
        .p(t().d(8.));
    if e.files.is_empty() {
        tree = tree.child(
            div()
                .p(t().d(8.))
                .text_size(t().fs(11.))
                .text_color(rgb(muted()))
                .child(if e.loading { "拉文件…" } else { "这个目录里没有文件" }),
        );
    }
    for f in &e.files {
        let on = f.path == e.active;
        let path = f.path.clone();
        tree = tree.child(
            div()
                .id(ElementId::Name(format!("res-file-{path}").into()))
                .flex()
                .items_center()
                .gap(t().d(6.))
                .px(t().d(8.))
                .py(t().d(5.))
                .rounded(t().r(6.))
                .text_size(t().fs(11.5))
                .bg(rgb(if on { t().p.accent_soft } else { t().p.list_bg }))
                .text_color(rgb(if on { t().p.accent_fg } else { t().p.text_2 }))
                .hover(|el| el.bg(rgb(t().p.hover)))
                .child(
                    div()
                        .flex_1()
                        .min_w(t().d(0.))
                        .truncate()
                        .child(f.path.clone()),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.resource_pick_file(path.clone(), window, cx);
                })),
        );
    }

    let status = div()
        .flex()
        .flex_col()
        .gap(t().d(12.))
        .w(t().d(230.))
        .flex_shrink_0()
        .border_l(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .px(t().d(15.))
        .py(t().d(14.))
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(muted()))
                .child("状态"),
        )
        .child(kv("装载", if e.loading { "拉文件…" } else { "正常" }))
        .child(kv("改动", &format!("{}{} 行", if added >= 0 { "+" } else { "" }, added)))
        .child(kv("并发闸", if e.etag.is_empty() { "—" } else { "已拿到 etag" }))
        .child(kv("仓库", &format!("{}/{}/", repo_root(desk, e.kind.repo()), e.name)))
        .child(
            div()
                .px(t().d(10.))
                .py(t().d(8.))
                .rounded(t().r(6.))
                .bg(rgb(t().p.hover))
                .text_size(t().fs(11.))
                .line_height(t().d(16.))
                .text_color(rgb(muted()))
                .child("存草稿只落盘、不提交。要进别人的视野得去「发布」。"),
        )
        .when_some(e.err.clone(), |el, err| {
            el.child(
                div()
                    .px(t().d(10.))
                    .py(t().d(8.))
                    .rounded(t().r(6.))
                    .bg(rgb(t().p.danger_soft))
                    .text_size(t().fs(11.))
                    .line_height(t().d(16.))
                    .text_color(rgb(t().p.danger_fg))
                    .child(err),
            )
        });

    let foot = div()
        .flex()
        .items_center()
        .gap(t().d(10.))
        .px(t().d(22.))
        .py(t().d(11.))
        .border_t(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .flex_1()
                .text_size(t().fs(11.5))
                .text_color(rgb(muted()))
                .child(if dirty {
                    "有没存的改动（关掉会问一句）".to_string()
                } else {
                    "没有改动".to_string()
                }),
        )
        .child(
            ghost_btn("res-edit-close", "关掉").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.resource_close_edit(cx);
            })),
        )
        .child(
            btn("res-edit-save", if e.busy { "存…" } else { "保存" }, true).on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.resource_save(window, cx);
                }),
            ),
        );

    div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w(t().d(0.))
        .bg(rgb(t().p.panel))
        .child(page_head(
            &format!("{} · {}", e.name, e.kind.label()),
            "整页工作台：左边文件树、中间正文、右边状态。不是盖在列表上的右栏抽屉。",
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h(t().d(0.))
                .child(tree)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(t().d(0.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(t().d(8.))
                                .px(t().d(16.))
                                .py(t().d(8.))
                                .border_b(t().d(1.))
                                .border_color(rgb(t().p.line_soft))
                                .child(
                                    div()
                                        .text_size(t().fs(11.))
                                        .font_family("JetBrainsMono")
                                        .text_color(rgb(muted()))
                                        .child(e.active.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h(t().d(0.))
                                .p(t().d(10.))
                                .child(Textarea::new(&e.editor).h_full()),
                        ),
                )
                .child(status),
        )
        .child(foot)
        .into_any_element()
}

fn kv(k: &str, v: &str) -> AnyElement {
    div()
        .flex()
        .gap(t().d(6.))
        .text_size(t().fs(11.5))
        .child(div().w(t().d(46.)).text_color(rgb(muted())).child(k.to_string()))
        .child(
            div()
                .flex_1()
                .min_w(t().d(0.))
                .text_color(rgb(text_color()))
                .child(v.to_string()),
        )
        .into_any_element()
}

/// 发布：三个仓各一个按钮，不做一键全发。
pub(crate) fn render_publish(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let mut list = div()
        .flex()
        .flex_col()
        .gap(t().d(10.))
        .p(t().d(22.));

    for repo in Repo::all() {
        let i = PublishState::idx(repo);
        let busy = desk.resources.publish.busy[i];
        let done = desk.resources.publish.done[i];
        let err = desk.resources.publish.err[i].clone();
        let pending = pending_count(desk, repo);
        let root = repo_root(desk, repo);

        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(t().d(14.))
                .px(t().d(16.))
                .py(t().d(13.))
                .rounded(t().r(9.))
                .border(t().d(1.))
                .border_color(rgb(if err.is_some() { t().p.danger } else { t().p.line }))
                .bg(rgb(if err.is_some() { t().p.danger_soft } else { t().p.panel }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t().d(2.))
                        .flex_1()
                        .min_w(t().d(0.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(t().d(7.))
                                .text_size(t().fs(12.5))
                                .text_color(rgb(text_color()))
                                .child(repo.label().to_string())
                                .child(
                                    div()
                                        .text_size(t().fs(10.5))
                                        .text_color(rgb(if pending > 0 {
                                            t().p.warn
                                        } else {
                                            t().p.accent_fg
                                        }))
                                        .child(if pending > 0 {
                                            format!("{pending} 处待发布")
                                        } else {
                                            "没有待发布".to_string()
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_size(t().fs(10.5))
                                .font_family("JetBrainsMono")
                                .text_color(rgb(muted()))
                                .child(root),
                        )
                        .when_some(err.clone(), |el, e| {
                            el.child(
                                div()
                                    .pt(t().d(3.))
                                    .text_size(t().fs(11.))
                                    .line_height(t().d(16.))
                                    .text_color(rgb(t().p.danger_fg))
                                    .child(e),
                            )
                        }),
                )
                .child(
                    btn(
                        &format!("res-pub-{}", repo.key()),
                        if done {
                            "已发布"
                        } else if busy {
                            "发布中…"
                        } else {
                            "发布"
                        },
                        !done,
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.resource_publish(repo, window, cx);
                    })),
                ),
        );
    }

    list = list.child(
        div()
            .px(t().d(12.))
            .py(t().d(9.))
            .rounded(t().r(6.))
            .bg(rgb(t().p.panel_alt))
            .text_size(t().fs(11.))
            .line_height(t().d(17.))
            .text_color(rgb(muted()))
            .child("为什么不做「一键全发」：三个仓是三件独立的事。谁失败只染谁 —— 一键发会把别人卡住的仓拖成整片红。"),
    );

    div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .min_w(t().d(0.))
        .bg(rgb(t().p.panel))
        .child(page_head(
            "发布",
            "三个仓各走各的，不做一键全发。发布 = 内容闸 + 重扫 index.json + commit + push，一步。",
        ))
        .child(
            div()
                .id("res-publish")
                .flex_1()
                .min_h(t().d(0.))
                .overflow_y_scroll()
                .child(list),
        )
        .into_any_element()
}

/// 确认层：不可逆的动作多走一步才够得到。
pub(crate) fn render_confirm(desk: &Desktop, cx: &mut Context<Desktop>) -> AnyElement {
    let Some(c) = desk.resources.confirm.as_ref() else {
        return div().into_any_element();
    };
    let busy = c.busy;
    let err = c.err.clone();
    let danger = c.danger();

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(t().p.scrim))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(t().d(10.))
                .w(t().d(420.))
                .p(t().d(20.))
                .rounded(t().r(12.))
                .bg(rgb(t().p.overlay))
                .shadow_lg()
                .child(
                    div()
                        .text_size(t().fs(13.5))
                        .text_color(rgb(text_color()))
                        .child(c.title()),
                )
                .child(
                    div()
                        .text_size(t().fs(12.))
                        .line_height(t().d(18.))
                        .text_color(rgb(t().p.text_2))
                        .child(c.body()),
                )
                .when_some(err, |el, e| {
                    el.child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.danger_fg))
                            .child(e),
                    )
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(t().d(8.))
                        .child(
                            ghost_btn("res-confirm-no", "取消").on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| {
                                    this.resource_confirm_no(cx);
                                },
                            )),
                        )
                        .child(
                            div()
                                .id("res-confirm-yes")
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(t().d(28.))
                                .px(t().d(12.))
                                .rounded(t().r(7.))
                                .text_size(t().fs(11.5))
                                .bg(rgb(if danger { t().p.danger } else { t().p.accent }))
                                .text_color(rgb(t().p.on_accent))
                                .hover(|el| el.opacity(0.9))
                                .child(if busy { "处理中…" } else { c.yes() })
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.resource_confirm_yes(window, cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}

// ── 小工具 ─────────────────────────────────────────────────────────────

/// 仓的根目录：落盘预览和状态栏都要写全路径，只写相对路径等于让人猜。
pub(crate) fn repo_root(desk: &Desktop, repo: Repo) -> String {
    let r = &desk.resources;
    match repo {
        Repo::Skills => r
            .skills
            .as_ref()
            .and_then(|s| s.root.clone())
            .unwrap_or_else(|| "…".into()),
        Repo::Prompts => r
            .prompts
            .as_ref()
            .and_then(|p| p.root.clone())
            .unwrap_or_else(|| "…".into()),
        Repo::Extensions => r
            .exts
            .as_ref()
            .and_then(|e| e.repo.as_ref())
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| "…".into()),
    }
}

/// 这个仓有几处待发布。技能 / 提示词按 `tracked==false` 数，扩展按仓库 drift。
///
/// `tracked` 是**三态**：`None` = 引擎不查 git（Nous#71 起技能恒 `null`），
/// 那不是「未提交」，不能计数 —— 否则整库都成了待发布。
fn pending_count(desk: &Desktop, repo: Repo) -> usize {
    let r = &desk.resources;
    match repo {
        Repo::Skills => r
            .skills
            .as_ref()
            .map(|s| s.skills.iter().filter(|x| x.tracked == Some(false)).count())
            .unwrap_or(0),
        Repo::Prompts => r
            .prompts
            .as_ref()
            .map(|p| p.prompts.iter().filter(|x| x.tracked == Some(false)).count())
            .unwrap_or(0),
        Repo::Extensions => r
            .exts
            .as_ref()
            .and_then(|e| e.repo.as_ref())
            .and_then(|v| v.get("drift"))
            .and_then(|v| v.as_bool())
            .map(usize::from)
            .unwrap_or(0),
    }
}

/// 这条是不是还没进 HEAD。**只看引擎给的 pending 清单**，不在客户端推算。
/// `tracked` 为 `None`（引擎不查 git）时一律当「不知道」，不当作未提交。
pub(crate) fn is_untracked(kind: Kind, name: &str, desk: &Desktop) -> bool {
    let r = &desk.resources;
    match kind {
        Kind::Skill => r
            .skills
            .as_ref()
            .and_then(|s| s.skills.iter().find(|x| x.name == name))
            .is_some_and(|s| s.tracked == Some(false)),
        Kind::Prompt => r
            .prompts
            .as_ref()
            .and_then(|p| p.prompts.iter().find(|x| x.name == name))
            .is_some_and(|p| p.tracked == Some(false)),
        Kind::Ext => r
            .exts
            .as_ref()
            .and_then(|e| e.repo.as_ref())
            .and_then(|v| v.get("drift"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}
