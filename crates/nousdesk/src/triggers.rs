//! 触发器工作台（视图 1）：清单 / 详情 / 代码 / 新建向导。
//!
//! 数据住在 **n8qkkj**（另一个块，自己的 `/v1/triggers`），不是 qrd3wh。
//! 三层是一个页面的三种深度：
//!
//! - 主界面：类别树 + 状态筛选 + 卡片网格
//! - 详情：右滑面板（盖卡片区、不盖类别栏）
//! - 代码：全宽（文件树 / 编辑 / 差异 / 发布）
//! - 新建向导：另一条支线（身份 → 参数 → 骨架 → 建）
//!
//! **保存 ≠ 发布**：保存只 `PUT /file` 落盘；发布是一个同步长请求
//! （cargo check → build → index → commit → push），中间进度拿不到，别假装五步并行。

fn t() -> crate::theme::Skin { crate::theme::cur() }

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use nous_engine::triggers::{
    ArchivedTrigger, NewTrigger, TriggerFileMeta, TriggerItem, TriggerList,
    TriggerParam, TriggerPublish,
};

use crate::app::Desktop;
use crate::config::TRIGGER_BLOCK;

const ARCHIVED: &str = "__archived__";
const UNCATEGORIZED: &str = "∅";

/// 三层 + 主界面。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Main,
    Detail,
    Code,
    Wizard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusFilter {
    All,
    Ok,
    Dirty,
    Unpublished,
    Broken,
}

impl StatusFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Ok => "已发布",
            Self::Dirty => "有改动",
            Self::Unpublished => "未发布",
            Self::Broken => "清单坏了",
        }
    }
}

pub(crate) struct CodeState {
    pub name: Option<String>,
    pub files: Vec<TriggerFileMeta>,
    pub path: Option<String>,
    pub text: Entity<TextareaState>,
    pub etag: String,
    /// HEAD 里那一版；差异视图跟它比。
    pub base: String,
    pub busy: bool,
    pub publish: Option<TriggerPublish>,
    pub publish_error: Option<String>,
    pub notice: Option<(String, bool)>,
    pub diff: bool,
    /// 只读代码视图 → 编辑（Textarea）。保存后回到只读。
    pub editing: bool,
}

pub(crate) struct Wizard {
    pub step: u8,
    pub name: Entity<InputState>,
    pub summary: Entity<InputState>,
    pub category: Entity<InputState>,
    pub busy: bool,
    pub err: Option<String>,
}

pub(crate) struct Confirm {
    pub open: bool,
    pub name: String,
    pub input: Entity<InputState>,
}

pub(crate) struct State {
    pub client: Option<Arc<nous_engine::TriggerClient>>,
    pub data: Option<TriggerList>,
    pub loading: bool,
    pub error: Option<String>,
    /// `None` = 全部；`Some(cat)` = 类别；`Some(ARCHIVED)` = 归档视图。
    pub cat: Option<String>,
    pub status: StatusFilter,
    pub search: Entity<InputState>,
    pub layer: Layer,
    pub selected: Option<String>,
    pub code: CodeState,
    pub wizard: Wizard,
    pub confirm: Confirm,
    pub busy: Option<String>,
    pub scroll: ScrollHandle,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜名字 / 说明 / 类别"));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("morning-review"));
        let summary = cx.new(|cx| {
            InputState::new(window, cx).placeholder("每天第一条会话时，按昨天的日志起一条复盘")
        });
        let category = cx.new(|cx| InputState::new(window, cx).placeholder("日常/复盘"));
        let code_text = cx.new(|cx| TextareaState::new(window, cx));
        let confirm_input = cx.new(|cx| InputState::new(window, cx).placeholder("手输名字"));
        Self {
            client: None,
            data: None,
            loading: false,
            error: None,
            cat: None,
            status: StatusFilter::All,
            search,
            layer: Layer::Main,
            selected: None,
            code: CodeState {
                name: None,
                files: vec![],
                path: None,
                text: code_text,
                etag: String::new(),
                base: String::new(),
                busy: false,
                publish: None,
                publish_error: None,
                notice: None,
                diff: false,
                editing: false,
            },
            wizard: Wizard {
                step: 1,
                name,
                summary,
                category,
                busy: false,
                err: None,
            },
            confirm: Confirm {
                open: false,
                name: String::new(),
                input: confirm_input,
            },
            busy: None,
            scroll: ScrollHandle::new(),
        }
    }

    fn current(&self) -> Option<&TriggerItem> {
        let name = self.selected.as_deref()?;
        self.data.as_ref()?.triggers.iter().find(|t| t.name == name)
    }

    fn dirty_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|d| d.triggers.iter().filter(|t| t.dirty && t.tracked).count())
            .unwrap_or(0)
    }

    fn unpublished_count(&self) -> usize {
        self.data
            .as_ref()
            .map(|d| d.triggers.iter().filter(|t| !t.tracked).count())
            .unwrap_or(0)
    }
}

// ── 数据 ───────────────────────────────────────────────────────────────────

impl Desktop {
    /// 启动 / 重连时 resolve `n8qkkj` 并拉清单。
    pub(crate) fn load_triggers(&mut self, cx: &mut Context<Self>) {
        let cfg = self.conn.clone();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let url = handle
                .spawn(async move {
                    nous_engine::resolve::resolve_engine(
                        &cfg.host,
                        &cfg.board,
                        crate::config::SELF_BLOCK,
                        TRIGGER_BLOCK,
                    )
                    .await
                })
                .await;
            let url = match url {
                Ok(Ok(url)) => url,
                Ok(Err(e)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.triggers.error = Some(format!("触发器块 resolve 不到：{e}"));
                        cx.notify();
                    });
                    return;
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.triggers.error = Some(format!("resolve 任务挂了：{e}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let client = match nous_engine::TriggerClient::new(url.as_str(), Duration::from_secs(30)) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.triggers.error = Some(format!("触发器客户端建不起来：{e}"));
                        cx.notify();
                    });
                    return;
                }
            };
            let loaded = {
                let c = client.clone();
                handle.spawn(async move { c.list().await }).await
            };
            let _ = this.update(cx, |this, cx| {
                this.triggers.client = Some(client);
                this.triggers.loading = false;
                match loaded {
                    Ok(Ok(data)) => {
                        this.triggers.data = Some(data);
                        this.triggers.error = None;
                    }
                    Ok(Err(e)) => this.triggers.error = Some(format!("拉触发器失败：{e}")),
                    Err(e) => this.triggers.error = Some(format!("任务挂了：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_trigger_list(&mut self, force: bool, cx: &mut Context<Self>) {
        if !force && (self.triggers.loading || self.triggers.data.is_some()) {
            return;
        }
        let Some(client) = self.triggers.client.clone() else {
            return;
        };
        self.triggers.loading = true;
        self.triggers.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else {
                return;
            };
            let res = handle.spawn(async move { client.list().await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.loading = false;
                match res {
                    Ok(Ok(data)) => {
                        this.triggers.data = Some(data);
                        this.triggers.error = None;
                    }
                    Ok(Err(e)) => this.triggers.error = Some(format!("拉触发器失败：{e}")),
                    Err(e) => this.triggers.error = Some(format!("任务挂了：{e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn notice(&mut self, text: impl Into<String>, bad: bool, cx: &mut Context<Self>) {
        let token = self.notice.show(text, if bad { crate::app::NoticeKind::Error } else { crate::app::NoticeKind::Info });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(4))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.notice.expire(token) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    // ── 层 / 选中 ──────────────────────────────────────────

    pub(crate) fn trigger_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        self.triggers.layer = layer;
        if layer != Layer::Detail {
            self.triggers.confirm.open = false;
        }
        cx.notify();
    }

    fn select_trigger(&mut self, name: &str, cx: &mut Context<Self>) {
        log::info!("触发器：选中 {name}");
        self.triggers.selected = Some(name.to_string());
        self.triggers.layer = Layer::Detail;
        cx.notify();
    }

    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.triggers.layer = Layer::Main;
        cx.notify();
    }

    // ── 代码层 ─────────────────────────────────────────────

    fn open_code(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("触发器：打开代码层 {name}");
        self.triggers.code.name = Some(name.to_string());
        self.triggers.code.files.clear();
        self.triggers.code.path = None;
        self.triggers.code.publish = None;
        self.triggers.code.publish_error = None;
        self.triggers.code.notice = None;
        self.triggers.code.base.clear();
        self.triggers.layer = Layer::Code;
        cx.notify();
        let Some(client) = self.triggers.client.clone() else { return };
        let name = name.to_string();
        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let n = name.clone();
            let res = handle.spawn(async move { client.files(&n).await }).await;
            let _ = this.update_in(cx, |this, window, cx| match res {
                Ok(Ok(files)) => {
                    this.triggers.code.files = files.files;
                    let first = this.triggers.code.files.first().map(|f| f.path.clone());
                    if let Some(path) = first {
                        this.open_trigger_file(&name, &path, window, cx);
                    }
                }
                Ok(Err(e)) => {
                    this.triggers.code.notice = Some((format!("读文件列表失败：{e}"), true));
                    cx.notify();
                }
                Err(e) => {
                    this.triggers.code.notice = Some((format!("任务挂了：{e}"), true));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn open_trigger_file(
        &mut self,
        name: &str,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.triggers.client.clone() else { return };
        let name = name.to_string();
        let path = path.to_string();
        self.triggers.code.path = Some(path.clone());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let (n, p) = (name.clone(), path.clone());
            let res = handle.spawn(async move { client.file(&n, &p).await }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match res {
                    Ok(Ok(file)) => {
                        this.triggers.code.text.update(cx, |state, cx| {
                            state.set_value(&file.text, window, cx);
                        });
                        this.triggers.code.etag = file.etag;
                        this.triggers.code.base = file.base.unwrap_or_default();
                        this.triggers.code.path = Some(file.path);
                        this.triggers.code.notice = None;
                    }
                    Ok(Err(e)) => {
                        this.triggers.code.notice = Some((format!("读文件失败：{e}"), true));
                    }
                    Err(e) => {
                        this.triggers.code.notice = Some((format!("任务挂了：{e}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_trigger_file(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let (Some(client), Some(name), Some(path)) = (
            self.triggers.client.clone(),
            self.triggers.code.name.clone(),
            self.triggers.code.path.clone(),
        ) else {
            return;
        };
        let text = self.triggers.code.text.read(cx).value().to_string();
        let etag = self.triggers.code.etag.clone();
        self.triggers.code.busy = true;
        self.triggers.code.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let res = handle
                .spawn(async move { client.save(&name, &path, &text, &etag).await })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.code.busy = false;
                match res {
                    Ok(Ok(v)) => {
                        if let Some(new_etag) = v.get("etag").and_then(|e| e.as_str()) {
                            this.triggers.code.etag = new_etag.to_string();
                        }
                        this.triggers.code.notice = Some(("已保存到磁盘 · 还没提交".into(), false));
                        this.triggers.code.editing = false;
                    }
                    Ok(Err(e)) => {
                        this.triggers.code.notice = Some((format!("保存失败：{e}"), true));
                    }
                    Err(e) => {
                        this.triggers.code.notice = Some((format!("任务挂了：{e}"), true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn publish_trigger(&mut self, cx: &mut Context<Self>) {
        let (Some(client), Some(name)) = (
            self.triggers.client.clone(),
            self.triggers.code.name.clone(),
        ) else {
            return;
        };
        self.triggers.code.busy = true;
        self.triggers.code.publish = None;
        self.triggers.code.publish_error = None;
        self.triggers.code.notice = Some(("发布中：cargo check → build → index → commit → push".into(), false));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let n = name.clone();
            let res = handle.spawn(async move { client.publish(&n).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.code.busy = false;
                match res {
                    Ok(Ok(out)) => {
                        if !out.pushed && out.committed {
                            this.triggers.code.publish_error = Some(format!(
                                "本地已提交，但推送失败：{}（点发布再试一次）",
                                out.push_error
                            ));
                        } else if !out.committed {
                            this.triggers.code.notice = Some(("没有待提交的改动".into(), false));
                        }
                        this.triggers.code.notice = Some((
                            format!(
                                "发布完成 · {} · {}",
                                out.head.clone().unwrap_or_default(),
                                if out.pushed { "已推送" } else { "未推送" }
                            ),
                            !out.pushed,
                        ));
                        this.triggers.code.publish = Some(out);
                    }
                    Ok(Err(e)) => {
                        this.triggers.code.publish_error = Some(format!("发布失败：{e}"));
                    }
                    Err(e) => {
                        this.triggers.code.publish_error = Some(format!("任务挂了：{e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        self.load_trigger_list(true, cx);
    }

    // ── 归档 / 同步 ────────────────────────────────────────

    fn ask_archive(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.triggers.confirm.name = name.to_string();
        self.triggers.confirm.open = true;
        self.triggers.confirm.input.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    fn confirm_archive(&mut self, cx: &mut Context<Self>) {
        let (Some(client), name) = (self.triggers.client.clone(), self.triggers.confirm.name.clone()) else {
            return;
        };
        self.triggers.confirm.open = false;
        self.triggers.busy = Some("归档".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let n = name.clone();
            let res = handle.spawn(async move { client.archive(&n).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.busy = None;
                match res {
                    Ok(Ok(out)) => {
                        let text = if out.dropped {
                            format!("已删掉 {name}（没发布过，仓库里本来就没有）")
                        } else if out.pushed {
                            format!("已归档 {name} · 左栏「已归档」里可以恢复")
                        } else {
                            format!("已在本地归档 {name}，但推送失败：{}", out.push_error)
                        };
                        this.notice(text, !out.pushed && !out.dropped, cx);
                        this.triggers.layer = Layer::Main;
                        this.load_trigger_list(true, cx);
                    }
                    Ok(Err(e)) => this.notice(format!("归档失败：{e}"), true, cx),
                    Err(e) => this.notice(format!("任务挂了：{e}"), true, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn restore_trigger(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(client) = self.triggers.client.clone() else { return };
        let name = name.to_string();
        self.triggers.busy = Some(format!("恢复 {name}"));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let n = name.clone();
            let res = handle.spawn(async move { client.restore(&n).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.busy = None;
                match res {
                    Ok(Ok(out)) => this.notice(
                        if out.pushed {
                            format!("{name} 已从归档搬回来")
                        } else {
                            format!("已恢复 {name}，但推送失败：{}", out.push_error)
                        },
                        !out.pushed,
                        cx,
                    ),
                    Ok(Err(e)) => this.notice(format!("恢复失败：{e}"), true, cx),
                    Err(e) => this.notice(format!("任务挂了：{e}"), true, cx),
                }
                this.load_trigger_list(true, cx);
            });
        })
        .detach();
    }

    fn purge_trigger(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(client) = self.triggers.client.clone() else { return };
        let name = name.to_string();
        self.triggers.busy = Some(format!("彻底删除 {name}"));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let n = name.clone();
            let res = handle.spawn(async move { client.purge(&n).await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.busy = None;
                match res {
                    Ok(Ok(out)) => this.notice(
                        if out.pushed {
                            format!("{name} 已彻底删除")
                        } else {
                            format!("已提交删除，但推送失败：{}", out.push_error)
                        },
                        !out.pushed,
                        cx,
                    ),
                    Ok(Err(e)) => this.notice(format!("彻底删除失败：{e}"), true, cx),
                    Err(e) => this.notice(format!("任务挂了：{e}"), true, cx),
                }
                this.load_trigger_list(true, cx);
            });
        })
        .detach();
    }

    fn sync_triggers(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.triggers.client.clone() else { return };
        self.triggers.busy = Some("同步".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            let res = handle.spawn(async move { client.sync().await }).await;
            let _ = this.update(cx, |this, cx| {
                this.triggers.busy = None;
                match res {
                    Ok(Ok(out)) => this.notice(
                        if out.pushed {
                            "词表 / 清单已提交并推送".to_string()
                        } else {
                            format!("已同步，但推送失败：{}", out.push_error)
                        },
                        !out.pushed,
                        cx,
                    ),
                    Ok(Err(e)) => this.notice(format!("同步失败：{e}"), true, cx),
                    Err(e) => this.notice(format!("任务挂了：{e}"), true, cx),
                }
                this.load_trigger_list(true, cx);
            });
        })
        .detach();
    }

    // ── 向导 ───────────────────────────────────────────────

    fn open_wizard(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.triggers.wizard.step = 1;
        self.triggers.wizard.err = None;
        self.triggers.layer = Layer::Wizard;
        cx.notify();
    }

    fn wizard_step(&mut self, step: u8, cx: &mut Context<Self>) {
        self.triggers.wizard.step = step.clamp(1, 3);
        self.triggers.wizard.err = None;
        cx.notify();
    }

    fn create_trigger(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.triggers.client.clone() else { return };
        let name = self.triggers.wizard.name.read(cx).value().trim().to_string();
        let summary = self.triggers.wizard.summary.read(cx).value().trim().to_string();
        let category = self
            .triggers
            .wizard
            .category
            .read(cx)
            .value()
            .split(['/', ',', '，'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if name.is_empty() || summary.is_empty() {
            self.triggers.wizard.err = Some("名字和一句话说明都不能空".into());
            cx.notify();
            return;
        }
        let params_raw = self.wizard_params_text();
        self.triggers.wizard.busy = true;
        self.triggers.wizard.err = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let Some(handle) = crate::TOKIO.get().cloned() else { return };
            // 1. 建目录 + 骨架（未发布）
            let create = {
                let c = client.clone();
                let body = NewTrigger {
                    name: name.clone(),
                    summary: summary.clone(),
                    category: category.clone(),
                };
                handle.spawn(async move { c.create(&body).await }).await
            };
            let created = match create {
                Ok(Ok(_)) => true,
                Ok(Err(e)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.triggers.wizard.busy = false;
                        this.triggers.wizard.err = Some(format!("建失败：{e}"));
                        cx.notify();
                    });
                    false
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.triggers.wizard.busy = false;
                        this.triggers.wizard.err = Some(format!("任务挂了：{e}"));
                        cx.notify();
                    });
                    false
                }
            };
            if !created {
                return;
            }
            // 2. 有参数就写回 nous.json（建接口只收 name/summary/category）
            let params = parse_params(&params_raw);
            if !params.is_empty() {
                let read = {
                    let c = client.clone();
                    let n = name.clone();
                    handle
                        .spawn(async move { c.file(&n, "nous.json").await })
                        .await
                };
                #[allow(clippy::collapsible_if)]
                if let Ok(Ok(file)) = read {
                    if let Ok(mut spec) =
                        serde_json::from_str::<serde_json::Value>(&file.text)
                    {
                        if let Some(obj) = spec.as_object_mut() {
                            obj.insert("params".into(), serde_json::Value::Array(params));
                        }
                        let text = serde_json::to_string_pretty(&spec).unwrap_or_default() + "\n";
                        let c = client.clone();
                        let (n, base) = (name.clone(), file.etag.clone());
                        let _ = handle
                            .spawn(async move { c.save(&n, "nous.json", &text, &base).await })
                            .await;
                    }
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.triggers.wizard.busy = false;
                this.triggers.layer = Layer::Main;
                this.notice(
                    format!("已建 {name} · 未发布 · 去代码层点发布才会进 HEAD"),
                    false,
                    cx,
                );
                this.load_trigger_list(true, cx);
                cx.notify();
            });
        })
        .detach();
    }
}

/// 向导里那几行 `name|type|label|required|options` → 触发器的 params 声明。
fn parse_params(raw: &str) -> Vec<serde_json::Value> {
    let mut out = vec![];
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() < 2 || parts[0].is_empty() {
            continue;
        }
        let name = parts[0];
        let kind = parts[1];
        let label = parts.get(2).filter(|s| !s.is_empty()).copied().unwrap_or(name);
        let required = parts.get(3).map(|s| *s == "true").unwrap_or(false);
        let options: Vec<&str> = parts
            .get(4)
            .map(|s| s.split(',').map(str::trim).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let mut param = serde_json::json!({
            "name": name,
            "type": kind,
            "label": label,
            "required": required,
        });
        if kind == "select" && !options.is_empty() {
            param["options"] = serde_json::json!(options);
        }
        out.push(param);
    }
    out
}

// ── 渲染 ───────────────────────────────────────────────────────────────────

fn short(text: &str, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn status_of(item: &TriggerItem) -> (&'static str, u32, u32) {
    if item.error.as_deref().is_some_and(|e| !e.is_empty()) {
        ("清单坏了", t().p.danger_soft, t().p.danger)
    } else if !item.tracked {
        ("未发布", t().p.warn_soft, t().p.warn)
    } else if item.dirty {
        ("有改动", t().p.warn_soft, t().p.warn)
    } else {
        ("已发布", t().p.accent_soft, t().p.accent_fg)
    }
}

fn is_under(cat: &str, root: &str) -> bool {
    cat == root || cat.starts_with(&format!("{root}/"))
}

impl Desktop {
    pub(crate) fn render_triggers(&self, cx: &mut Context<Self>) -> AnyElement {
        let layer = self.triggers.layer;
        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_w(t().d(0.))
            .bg(rgb(t().p.panel))
            .child(self.trigger_top(cx))
            .child(self.trigger_body(cx));
        if layer == Layer::Detail {
            root = root
                .child(self.trigger_scrim(cx))
                .child(self.trigger_detail(cx));
        }
        if layer == Layer::Code {
            root = root.child(self.trigger_code(cx));
        }
        if layer == Layer::Wizard {
            root = root.child(self.trigger_wizard(cx));
        }
        if self.triggers.confirm.open {
            root = root
                .child(self.trigger_confirm_scrim(cx))
                .child(self.trigger_confirm(cx));
        }
        root.into_any_element()
    }

    fn trigger_top(&self, cx: &mut Context<Self>) -> AnyElement {
        let head = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.repo.as_ref())
            .and_then(|r| r.head.clone())
            .unwrap_or_else(|| "—".into());
        let branch = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.repo.as_ref())
            .and_then(|r| r.branch.clone())
            .unwrap_or_else(|| "—".into());
        let dirty = self.triggers.dirty_count();
        let unpublished = self.triggers.unpublished_count();
        let sync_text = if dirty > 0 {
            format!("{dirty} 个有改动")
        } else if unpublished > 0 {
            format!("{unpublished} 个未发布")
        } else {
            "已同步".to_string()
        };
        let sync_color = if dirty + unpublished > 0 { t().p.warn } else { t().p.accent };

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(10.))
            .h(t().d(56.))
            .px(t().d(20.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line))
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(26.))
                    .rounded(t().r(7.))
                    .bg(rgb(t().p.surface))
                    .child(
                        Icon::new(IconName::Bolt)
                            .size(t().d(15.))
                            .text_color(rgb(t().p.text_2)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(t().fs(13.5))
                            .text_color(rgb(t().p.text))
                            .child("trigger"),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("Nous-Trigger · {branch} · {head}")),
                    ),
            )
            .child(
                div()
                    .ml(t().d(12.))
                    .h(t().d(19.))
                    .px(t().d(6.))
                    .flex()
                    .items_center()
                    .rounded(t().r(4.))
                    .bg(rgb(t().p.surface))
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(format!(
                        "{} 个触发器",
                        self.triggers.data.as_ref().map_or(0, |d| d.triggers.len())
                    )),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("trigger-sync")
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(26.))
                    .px(t().d(9.))
                    .rounded(t().r(13.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.sync_triggers(cx);
                    }))
                    .child(
                        div()
                            .size(t().d(6.))
                            .rounded(t().r(3.))
                            .bg(rgb(sync_color)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text_2))
                            .child(if self.triggers.busy.is_some() {
                                "同步中…".to_string()
                            } else {
                                sync_text
                            }),
                    ),
            )
            .child(
                div()
                    .id("trigger-refresh")
                    .flex()
                    .items_center()
                    .gap(t().d(5.))
                    .h(t().d(24.))
                    .px(t().d(8.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.load_trigger_list(true, cx);
                    }))
                    .child(
                        Icon::new(IconName::RefreshCw)
                            .size(t().d(13.))
                            .text_color(rgb(t().p.muted)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.muted))
                            .child("重扫"),
                    ),
            )
            .child(
                div()
                    .id("trigger-new")
                    .flex()
                    .items_center()
                    .gap(t().d(5.))
                    .h(t().d(28.))
                    .px(t().d(11.))
                    .rounded(t().r(6.))
                    .bg(rgb(t().p.accent))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_wizard(window, cx);
                    }))
                    .child(
                        Icon::new(IconName::Plus)
                            .size(t().d(13.))
                            .text_color(rgb(t().p.on_accent)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(t().p.on_accent))
                            .child("新建触发器"),
                    ),
            )
            .into_any_element()
    }

    fn trigger_body(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(t().d(0.))
            .child(self.trigger_cats(cx))
            .child(self.trigger_content(cx))
            .into_any_element()
    }

    fn trigger_cats(&self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.triggers.data.as_ref();
        let all = data.map_or(0, |d| d.triggers.len());
        let archived = data.map_or(0, |d| d.archived.len());
        let uncat = data
            .map(|d| d.triggers.iter().filter(|t| t.category.is_empty()).count())
            .unwrap_or(0);
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        if let Some(d) = data {
            for item in &d.triggers {
                // 同一条触发器挂了「时间」和「时间/一次」时，祖先「时间」只算一次。
                let mut paths = std::collections::HashSet::new();
                for cat in &item.category {
                    let mut path = String::new();
                    for part in cat.split('/') {
                        if !path.is_empty() {
                            path.push('/');
                        }
                        path.push_str(part);
                        paths.insert(path.clone());
                    }
                }
                for path in paths {
                    *counts.entry(path).or_default() += 1;
                }
            }
        }
        let current = self.triggers.cat.clone();
        let mut rows = vec![];
        rows.push(self.trigger_cat_row(None, "全部", all, current.is_none(), cx));
        rows.push(self.trigger_cat_row(Some(UNCATEGORIZED), "未分类", uncat, current.as_deref() == Some(UNCATEGORIZED), cx));
        if let Some(d) = data {
            for cat in &d.categories {
                let depth = cat.matches('/').count();
                let label = cat.rsplit('/').next().unwrap_or(cat);
                let count = counts.get(cat).copied().unwrap_or(0);
                let on = current.as_deref() == Some(cat.as_str());
                let c = cat.clone();
                rows.push(
                    div()
                        .id(ElementId::Name(format!("tcat-{cat}").into()))
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(t().d(29.))
                        .pl(t().d(14. + depth as f32 * 16.))
                        .pr(t().d(14.))
                        .cursor_pointer()
                        .when(on, |el| el.bg(rgb(t().p.hover)))
                        .hover(|h| h.bg(rgb(t().p.hover)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.triggers.cat = Some(c.clone());
                            this.triggers.layer = Layer::Main;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_size(t().fs(12.))
                                .text_color(rgb(if count == 0 { t().p.muted } else { t().p.text_2 }))
                                .child(label.to_string()),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(t().fs(10.))
                                .text_color(rgb(t().p.muted))
                                .child(count.to_string()),
                        )
                        .into_any_element(),
                );
            }
        }
        rows.push(
            div()
                .id("tcat-archived")
                .mt(t().d(8.))
                .pt(t().d(8.))
                .border_t(t().d(1.))
                .border_color(rgb(t().p.line_soft))
                .flex()
                .flex_row()
                .items_center()
                .h(t().d(34.))
                .px(t().d(14.))
                .cursor_pointer()
                .when(current.as_deref() == Some(ARCHIVED), |el| el.bg(rgb(t().p.hover)))
                .hover(|h| h.bg(rgb(t().p.hover)))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.triggers.cat = Some(ARCHIVED.to_string());
                    this.triggers.layer = Layer::Main;
                    cx.notify();
                }))
                .child(
                    Icon::new(IconName::Archive)
                        .size(t().d(13.))
                        .text_color(rgb(t().p.muted)),
                )
                .child(
                    div()
                        .ml(t().d(6.))
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.text_2))
                        .child("已归档"),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(t().fs(10.))
                        .text_color(rgb(t().p.muted))
                        .child(archived.to_string()),
                )
                .into_any_element(),
        );

        div()
            .flex()
            .flex_col()
            .w(t().d(214.))
            .flex_none()
            .h_full()
            .bg(rgb(t().p.panel_alt))
            .border_r(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .px(t().d(14.))
                    .pt(t().d(14.))
                    .pb(t().d(9.))
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("类别"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{}", counts.len())),
                    ),
            )
            .child(
                div()
                    .id("trigger-cats-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div()
                    .p(t().d(11.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .text_size(t().fs(9.5))
                    .text_color(rgb(t().p.muted))
                    .child("/opt/Nous-Trigger\nindex.mjs 扫出来的"),
            )
            .into_any_element()
    }

    fn trigger_cat_row(
        &self,
        value: Option<&str>,
        label: &str,
        count: usize,
        on: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = match value {
            None => "tcat-all".to_string(),
            Some(v) => format!("tcat-{v}"),
        };
        let value = value.map(str::to_owned);
        div()
            .id(ElementId::Name(id.into()))
            .flex()
            .flex_row()
            .items_center()
            .h(t().d(29.))
            .px(t().d(14.))
            .cursor_pointer()
            .when(on, |el| el.bg(rgb(t().p.hover)))
            .hover(|h| h.bg(rgb(t().p.hover)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.triggers.cat = value.clone();
                this.triggers.layer = Layer::Main;
                cx.notify();
            }))
            .child(
                div()
                    .text_size(t().fs(12.))
                    .text_color(rgb(if on { t().p.text } else { t().p.text_2 }))
                    .child(label.to_string()),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(count.to_string()),
            )
            .into_any_element()
    }

    fn trigger_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let filtered = self.filtered_triggers(cx);
        let archived_view = self.triggers.cat.as_deref() == Some(ARCHIVED);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(t().d(0.))
            .child(self.trigger_filters(cx))
            .child(self.trigger_tools(cx))
            .child(if archived_view {
                self.trigger_archived_list(cx)
            } else {
                self.trigger_grid(filtered, cx)
            })
            .into_any_element()
    }

    fn filtered_triggers(&self, cx: &mut Context<Self>) -> Vec<TriggerItem> {
        let Some(data) = &self.triggers.data else {
            return vec![];
        };
        let needle = self.triggers.search.read(cx).value().trim().to_lowercase();
        data.triggers
            .iter()
            .filter(|item| {
                let status = status_of(item).0;
                let by_status = match self.triggers.status {
                    StatusFilter::All => true,
                    StatusFilter::Ok => status == "已发布",
                    StatusFilter::Dirty => status == "有改动",
                    StatusFilter::Unpublished => status == "未发布",
                    StatusFilter::Broken => status == "清单坏了",
                };
                let by_cat = match self.triggers.cat.as_deref() {
                    None | Some(ARCHIVED) => true,
                    Some(UNCATEGORIZED) => item.category.is_empty(),
                    Some(cat) => item.category.iter().any(|c| is_under(c, cat)),
                };
                let hay = format!(
                    "{} {} {}",
                    item.name,
                    item.summary,
                    item.category.join(" ")
                )
                .to_lowercase();
                by_status && by_cat && (needle.is_empty() || hay.contains(&needle))
            })
            .cloned()
            .collect()
    }

    fn trigger_filters(&self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.triggers.data.as_ref();
        let count = |f: StatusFilter| {
            data.map(|d| {
                d.triggers
                    .iter()
                    .filter(|t| match f {
                        StatusFilter::All => true,
                        StatusFilter::Ok => status_of(t).0 == "已发布",
                        StatusFilter::Dirty => status_of(t).0 == "有改动",
                        StatusFilter::Unpublished => status_of(t).0 == "未发布",
                        StatusFilter::Broken => status_of(t).0 == "清单坏了",
                    })
                    .count()
            })
            .unwrap_or(0)
        };
        let mut children: Vec<AnyElement> = vec![];
        for (filter, dot) in [
            (StatusFilter::All, None),
            (StatusFilter::Ok, Some(t().p.accent)),
            (StatusFilter::Dirty, Some(t().p.warn)),
            (StatusFilter::Unpublished, Some(t().p.warn)),
            (StatusFilter::Broken, Some(t().p.danger)),
        ] {
            let on = self.triggers.status == filter;
            let n = count(filter);
            children.push(
                div()
                    .id(ElementId::Name(format!("tfilter-{filter:?}").into()))
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(26.))
                    .px(t().d(9.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.surface)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.triggers.status = filter;
                        cx.notify();
                    }))
                    .children(dot.map(|c| {
                        div()
                            .size(t().d(5.))
                            .rounded(t().r(2.5))
                            .bg(rgb(c))
                    }))
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(if on { t().p.text } else { t().p.muted }))
                            .child(filter.label()),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(n.to_string()),
                    )
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(2.))
            .h(t().d(44.))
            .px(t().d(20.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .children(children)
            .into_any_element()
    }

    fn trigger_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let n = self.filtered_triggers(cx).len();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(t().d(10.))
            .px(t().d(20.))
            .pt(t().d(12.))
            .pb(t().d(10.))
            .child(
                div()
                    .w(t().d(220.))
                    .h(t().d(28.))
                    .child(Input::new(&self.triggers.search)),
            )
            .child(
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child(format!("{n} 个 · 按最近改动")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(t().fs(10.5))
                    .text_color(rgb(t().p.muted))
                    .child("保存 ≠ 发布"),
            )
            .into_any_element()
    }

    fn trigger_grid(&self, rows: Vec<TriggerItem>, cx: &mut Context<Self>) -> AnyElement {
        if let Some(e) = &self.triggers.error {
            return div()
                .flex()
                .flex_col()
                .gap(t().d(6.))
                .p(t().d(20.))
                .child(
                    div()
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.danger))
                        .child(format!("拉触发器失败：{e}")),
                )
                .child(
                    div()
                        .id("trigger-retry")
                        .text_size(t().fs(12.))
                        .text_color(rgb(t().p.link))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.load_trigger_list(true, cx);
                        }))
                        .child("重试"),
                )
                .into_any_element();
        }
        if self.triggers.loading && self.triggers.data.is_none() {
            return div()
                .p(t().d(20.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("拉触发器…")
                .into_any_element();
        }
        if rows.is_empty() {
            return div()
                .p(t().d(20.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("这一档没有触发器。换个筛选，或者点「新建触发器」。")
                .into_any_element();
        }
        let cards: Vec<AnyElement> = rows
            .iter()
            .map(|item| self.trigger_card(item, cx))
            .collect();
        div()
            .id("trigger-grid-scroll")
            .flex_1()
            .min_h(t().d(0.))
            .overflow_y_scroll()
            .track_scroll(&self.triggers.scroll)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(t().d(12.))
                    .px(t().d(20.))
                    .pb(t().d(20.))
                    .children(cards),
            )
            .into_any_element()
    }

    fn trigger_card(&self, item: &TriggerItem, cx: &mut Context<Self>) -> AnyElement {
        let (label, bg, fg) = status_of(item);
        let name = item.name.clone();
        let params = item.params.len();
        let dirty = item.dirty_files;
        let tile = tile_color(&item.name);
        let cats: Vec<String> = item.category.iter().take(2).cloned().collect();
        div()
            .id(ElementId::Name(format!("tcard-{}", item.name).into()))
            .flex()
            .flex_col()
            .w(t().d(300.))
            .min_h(t().d(148.))
            .p(t().d(13.))
            .rounded(t().r(9.))
            .border(t().d(1.))
            .border_color(rgb(if label == "清单坏了" { t().p.danger } else { t().p.line }))
            .bg(rgb(t().p.panel))
            .cursor_pointer()
            .hover(|h| h.border_color(rgb(t().p.muted)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.select_trigger(&name.clone(), cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(t().d(9.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(t().d(30.))
                            .rounded(t().r(8.))
                            .bg(rgb(tile))
                            .text_size(t().fs(13.))
                            .text_color(rgb(t().p.on_accent))
                            .child(item.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(t().fs(11.5))
                                    .text_color(rgb(t().p.text))
                                    .child(item.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(10.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(format!("v{}", item.version)),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .h(t().d(19.))
                            .px(t().d(6.))
                            .flex()
                            .items_center()
                            .gap(t().d(4.))
                            .rounded(t().r(4.))
                            .bg(rgb(bg))
                            .child(
                                div()
                                    .size(t().d(5.))
                                    .rounded(t().r(2.5))
                                    .bg(rgb(fg)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(fg))
                                    .child(label),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(t().d(10.))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.muted))
                    .child(short(&item.summary, 46)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(t().d(4.))
                    .mt(t().d(9.))
                    .children(cats.iter().map(|c| {
                        div()
                            .h(t().d(19.))
                            .px(t().d(6.))
                            .flex()
                            .items_center()
                            .rounded(t().r(4.))
                            .bg(rgb(t().p.line_soft))
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.text_2))
                            .child(c.clone())
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(6.))
                    .mt_auto()
                    .pt(t().d(10.))
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.text_2))
                            .child(format!("{params} 参数")),
                    )
                    .child(div().flex_1())
                    .children((dirty > 0).then(|| {
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.warn))
                            .child(format!("{dirty} 个文件"))
                    })),
            )
            .into_any_element()
    }

    fn trigger_archived_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self
            .triggers
            .data
            .as_ref()
            .map(|d| d.archived.clone())
            .unwrap_or_default();
        if rows.is_empty() {
            return div()
                .p(t().d(20.))
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("归档里是空的。归档过的触发器会躺在这里，随时能搬回来。")
                .into_any_element();
        }
        let items: Vec<AnyElement> = rows
            .iter()
            .map(|row| self.archived_row(row, cx))
            .collect();
        div()
            .id("trigger-archived-scroll")
            .flex_1()
            .min_h(t().d(0.))
            .overflow_y_scroll()
            .p(t().d(20.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(t().r(9.))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .overflow_hidden()
                    .children(items),
            )
            .into_any_element()
    }

    fn archived_row(&self, row: &ArchivedTrigger, cx: &mut Context<Self>) -> AnyElement {
        let name = row.name.clone();
        let restore = row.name.clone();
        let purge = row.name.clone();
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(t().d(10.))
            .p(t().d(12.))
            .border_b(t().d(1.))
            .border_color(rgb(t().p.line_soft))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(t().d(30.))
                    .rounded(t().r(8.))
                    .bg(rgb(t().p.disabled))
                    .text_size(t().fs(11.))
                    .text_color(rgb(t().p.panel))
                    .child(row.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(t().d(0.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(t().d(7.))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(row.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(format!("v{}", row.version.clone().unwrap_or_default())),
                            ),
                    )
                    .child(
                        div()
                            .mt(t().d(3.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child(short(row.summary.as_deref().unwrap_or("（没写说明）"), 60)),
                    )
                    .child(
                        div()
                            .mt(t().d(4.))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(format!(
                                "{} · 归档于 {}",
                                row.category.join(" / "),
                                row.at.as_deref().unwrap_or("—")
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(t().d(6.))
                    .child(
                        div()
                            .id(ElementId::Name(format!("trestore-{restore}").into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(24.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.restore_trigger(&restore.clone(), cx);
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child("恢复"),
                            ),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(format!("tpurge-{purge}").into()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(24.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.danger_soft))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.danger_soft)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.purge_trigger(&purge.clone(), cx);
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.danger))
                                    .child("彻底删除"),
                            ),
                    ),
            )
            .child(div().w(t().d(0.)).child(name))
            .into_any_element()
    }

    fn trigger_scrim(&self, _cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("trigger-scrim")
            .absolute()
            .top(t().d(56.))
            .left(t().d(214.))
            .right_0()
            .bottom_0()
            .bg(rgba(t().p.scrim))
            .into_any_element()
    }

    fn trigger_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(item) = self.triggers.current().cloned() else {
            return div().into_any_element();
        };
        let (label, bg, fg) = status_of(&item);
        let name = item.name.clone();
        let archive_name = item.name.clone();
        let params: Vec<AnyElement> = item
            .params
            .iter()
            .map(|p| self.param_card(p))
            .collect();
        div()
            .absolute()
            .top(t().d(56.))
            .right_0()
            .bottom_0()
            .w(t().d(464.))
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .border_l(t().d(1.))
            .border_color(rgb(t().p.line))
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(t().d(20.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(t().d(10.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(t().d(40.))
                                    .rounded(t().r(9.))
                                    .bg(rgb(tile_color(&item.name)))
                                    .text_size(t().fs(16.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(item.name.chars().next().unwrap_or('?').to_uppercase().to_string()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(t().fs(16.))
                                            .text_color(rgb(t().p.text))
                                            .child(item.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .mt(t().d(3.))
                                            .text_size(t().fs(10.5))
                                            .text_color(rgb(t().p.muted))
                                            .child(format!(
                                                "v{} · {} · 产物 {}",
                                                item.version,
                                                item.category.join(" / "),
                                                item.binary.clone().unwrap_or_else(|| item.name.clone())
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .h(t().d(19.))
                                    .px(t().d(6.))
                                    .flex()
                                    .items_center()
                                    .gap(t().d(4.))
                                    .rounded(t().r(4.))
                                    .bg(rgb(bg))
                                    .child(
                                        div()
                                            .size(t().d(5.))
                                            .rounded(t().r(2.5))
                                            .bg(rgb(fg)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(fg))
                                            .child(label),
                                    ),
                            )
                            .child(
                                div()
                                    .id("trigger-detail-close")
                                    .size(t().d(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(t().r(5.))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_detail(cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::X)
                                            .size(t().d(14.))
                                            .text_color(rgb(t().p.muted)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .mt(t().d(11.))
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.text_2))
                            .child(item.summary.clone()),
                    )
                    .child(
                        div()
                            .mt(t().d(12.))
                            .flex()
                            .flex_row()
                            .gap(t().d(7.))
                            .child(
                                div()
                                    .id("trigger-detail-code")
                                    .flex()
                                    .items_center()
                                    .gap(t().d(5.))
                                    .h(t().d(26.))
                                    .px(t().d(9.))
                                    .rounded(t().r(5.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_code(&name.clone(), window, cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Code)
                                            .size(t().d(13.))
                                            .text_color(rgb(t().p.muted)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.text_2))
                                            .child("代码 ›"),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id("trigger-archive")
                                    .flex()
                                    .items_center()
                                    .gap(t().d(5.))
                                    .h(t().d(26.))
                                    .px(t().d(9.))
                                    .rounded(t().r(5.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.danger_soft))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.danger_soft)))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.ask_archive(&archive_name.clone(), window, cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Archive)
                                            .size(t().d(13.))
                                            .text_color(rgb(t().p.danger)),
                                    )
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.danger))
                                            .child("归档"),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("trigger-detail-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .child(detail_section(
                        "类别",
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(t().d(5.))
                            .children(item.category.iter().map(|c| {
                                div()
                                    .h(t().d(19.))
                                    .px(t().d(6.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(4.))
                                    .bg(rgb(t().p.surface))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(c.clone())
                            }))
                            .into_any_element(),
                    ))
                    .child(detail_section(
                        "清单 · nous.json",
                        div()
                            .flex()
                            .flex_col()
                            .children(vec![
                                kv("版本", item.version.clone()),
                                kv("产物名", item.binary.clone().unwrap_or_else(|| item.name.clone())),
                                kv("目录", format!("{}/", item.name)),
                                kv("语言", "Rust · 独立项目".into()),
                            ])
                            .into_any_element(),
                    ))
                    .child(detail_section("参数", div().flex().flex_col().gap(t().d(6.)).children(params).into_any_element()))
                    .child(detail_section(
                        "仓库",
                        div()
                            .flex()
                            .flex_col()
                            .children(vec![
                                kv("发布", if item.tracked { "已进 HEAD".into() } else { "未发布 · HEAD 里没有".into() }),
                                kv("改动", if item.dirty { format!("{} 个文件没提交", item.dirty_files) } else { "没有".into() }),
                                kv("同步", "清单 8 秒内自动推；代码要发布".into()),
                            ])
                            .into_any_element(),
                    ))
                    .child(detail_section(
                        "发布检查",
                        div()
                            .flex()
                            .flex_col()
                            .children(vec![
                                kv("cargo check", if item.dirty { "有改动，发布时才会跑".into() } else { "上次发布已通过".into() }),
                                kv("产物", format!("target/debug/{}", item.binary.clone().unwrap_or_else(|| item.name.clone()))),
                            ])
                            .into_any_element(),
                    )),
            )
            .into_any_element()
    }

    fn param_card(&self, p: &TriggerParam) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .p(t().d(9.))
            .rounded(t().r(7.))
            .border(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(7.))
                    .child(
                        div()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text))
                            .child(p.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(p.kind.clone()),
                    )
                    .children(p.required.then(|| {
                        div()
                            .h(t().d(16.))
                            .px(t().d(4.))
                            .flex()
                            .items_center()
                            .rounded(t().r(3.))
                            .bg(rgb(t().p.warn_soft))
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.warn))
                            .child("必填")
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(p.title().to_string()),
                    ),
            )
            .children(p.help.as_ref().filter(|h| !h.is_empty()).map(|h| {
                div()
                    .mt(t().d(5.))
                    .text_size(t().fs(10.))
                    .text_color(rgb(t().p.muted))
                    .child(h.clone())
            }))
            .into_any_element()
    }
}

fn tile_color(name: &str) -> u32 {
    // 浅色皮用深色头像，深色皮必须提亮 —— 否则 mono / amber 下头像是一团黑。
    const LIGHT: [u32; 6] = [0x1f6f5c, 0x4b5d7a, 0x7a5b32, 0x6b4b6f, 0x3f6b7a, 0x7a4b4b];
    const DARK: [u32; 6] = [0x3a9a82, 0x7189b0, 0xa8874f, 0x95739a, 0x5f97a8, 0xa86f6f];
    let mut h: u32 = 0;
    for ch in name.chars() {
        h = h.wrapping_mul(31).wrapping_add(ch as u32);
    }
    let palette = if is_dark(t().p.panel) { &DARK } else { &LIGHT };
    palette[(h as usize) % palette.len()]
}

fn is_dark(color: u32) -> bool {
    let r = (color >> 16) & 0xff;
    let g = (color >> 8) & 0xff;
    let b = color & 0xff;
    (r * 299 + g * 587 + b * 114) / 1000 < 128
}

fn detail_section(title: &str, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .px(t().d(20.))
        .py(t().d(14.))
        .border_b(t().d(1.))
        .border_color(rgb(t().p.line_soft))
        .child(
            div()
                .mb(t().d(8.))
                .text_size(t().fs(10.))
                .text_color(rgb(t().p.text_2))
                .child(title.to_string()),
        )
        .child(body)
        .into_any_element()
}

fn kv(key: &str, value: String) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(t().d(10.))
        .py(t().d(3.))
        .child(
            div()
                .w(t().d(78.))
                .flex_none()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.muted))
                .child(key.to_string()),
        )
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.text_2))
                .child(value),
        )
        .into_any_element()
}

impl Desktop {
    fn trigger_code(&self, cx: &mut Context<Self>) -> AnyElement {
        let code = &self.triggers.code;
        let file_tabs: Vec<AnyElement> = code
            .files
            .iter()
            .map(|f| {
                let path = f.path.clone();
                let on = code.path.as_deref() == Some(f.path.as_str());
                div()
                    .id(ElementId::Name(format!("tfile-{}", f.path).into()))
                    .flex()
                    .items_center()
                    .gap(t().d(6.))
                    .h(t().d(27.))
                    .px(t().d(7.))
                    .rounded(t().r(5.))
                    .cursor_pointer()
                    .when(on, |el| el.bg(rgb(t().p.hover)))
                    .hover(|h| h.bg(rgb(t().p.hover)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let name = this.triggers.code.name.clone().unwrap_or_default();
                        this.open_trigger_file(&name, &path.clone(), window, cx);
                    }))
                    .child(
                        Icon::new(IconName::File)
                            .size(t().d(11.))
                            .text_color(rgb(t().p.muted)),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(if on { t().p.text_2 } else { t().p.muted }))
                            .child(f.path.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(9.))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{} B", f.size)),
                    )
                    .into_any_element()
            })
            .collect();

        let body: AnyElement = if code.path.is_none() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(t().fs(12.))
                .text_color(rgb(t().p.muted))
                .child("选一个文件")
                .into_any_element()
        } else if code.diff {
            self.trigger_diff(cx)
        } else if code.editing {
            div()
                .id("trigger-code-editor")
                .flex_1()
                .min_h(t().d(0.))
                .p(t().d(10.))
                .child(
                    Textarea::new(&code.text)
                        .size_full()
                        .font_family("JetBrainsMono"),
                )
                .into_any_element()
        } else {
            let text = code.text.read(cx).value().to_string();
            div()
                .id("trigger-code-readonly")
                .flex_1()
                .min_h(t().d(0.))
                .overflow_scroll()
                .p(t().d(10.))
                .child(code_block(&text))
                .into_any_element()
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .h(t().d(50.))
                    .px(t().d(12.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .id("trigger-code-back")
                            .flex()
                            .items_center()
                            .gap(t().d(4.))
                            .h(t().d(28.))
                            .px(t().d(8.))
                            .rounded(t().r(6.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.trigger_back_to_detail(cx);
                            }))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(t().d(13.))
                                    .text_color(rgb(t().p.muted)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child("触发器"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(format!(
                                "{} / {}",
                                code.name.clone().unwrap_or_default(),
                                code.path.clone().unwrap_or_default()
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("保存只落盘 · 发布才 commit + push"),
                    )
                    .child(
                        div()
                            .id("trigger-code-edit")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(26.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.triggers.code.editing = !this.triggers.code.editing;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(if code.editing { "只读" } else { "编辑" }),
                            ),
                    )
                    .child(
                        div()
                            .id("trigger-code-save")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(26.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_trigger_file(window, cx);
                            }))
                            .child(
                                Icon::new(IconName::Save)
                                    .size(t().d(12.))
                                    .text_color(rgb(t().p.muted)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child("保存"),
                            ),
                    )
                    .child(
                        div()
                            .id("trigger-code-publish-top")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(26.))
                            .px(t().d(9.))
                            .rounded(t().r(5.))
                            .bg(rgb(if code.busy { t().p.disabled } else { t().p.accent }))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if !this.triggers.code.busy {
                                    this.publish_trigger(cx);
                                }
                            }))
                            .child(
                                Icon::new(IconName::Upload)
                                    .size(t().d(12.))
                                    .text_color(rgb(t().p.on_accent)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(if code.busy { "发布中…" } else { "发布" }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(t().d(194.))
                            .flex_none()
                            .h_full()
                            .bg(rgb(t().p.panel_alt))
                            .border_r(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .p(t().d(8.))
                            .child(
                                div()
                                    .px(t().d(6.))
                                    .py(t().d(4.))
                                    .text_size(t().fs(9.))
                                    .text_color(rgb(t().p.muted))
                                    .child("文件"),
                            )
                            .child(
                                div()
                                    .id("trigger-files-scroll")
                                    .flex_1()
                                    .min_h(t().d(0.))
                                    .overflow_y_scroll()
                                    .children(file_tabs),
                            )
                            .children(code.notice.as_ref().map(|(text, bad)| {
                                div()
                                    .p(t().d(6.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(if *bad { t().p.danger } else { t().p.accent_fg }))
                                    .child(text.clone())
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(t().d(8.))
                                    .h(t().d(34.))
                                    .px(t().d(12.))
                                    .border_b(t().d(1.))
                                    .border_color(rgb(t().p.line_soft))
                                    .child(
                                        div()
                                            .text_size(t().fs(10.))
                                            .text_color(rgb(t().p.muted))
                                            .child(format!(
                                                "{} / {}",
                                                code.name.clone().unwrap_or_default(),
                                                code.path.clone().unwrap_or_default()
                                            )),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .id("trigger-diff-toggle")
                                            .h(t().d(22.))
                                            .px(t().d(8.))
                                            .flex()
                                            .items_center()
                                            .rounded(t().r(4.))
                                            .bg(rgb(if code.diff { t().p.hover } else { t().p.panel }))
                                            .border(t().d(1.))
                                            .border_color(rgb(t().p.line))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.triggers.code.diff = !this.triggers.code.diff;
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .text_size(t().fs(10.))
                                                    .text_color(rgb(t().p.text_2))
                                                    .child(if code.diff { "编辑" } else { "差异" }),
                                            ),
                                    ),
                            )
                            .child(body),
                    )
                    .child(self.trigger_publish_panel(cx)),
            )
            .into_any_element()
    }

    fn trigger_back_to_detail(&mut self, cx: &mut Context<Self>) {
        self.triggers.layer = Layer::Detail;
        cx.notify();
    }

    fn trigger_diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let text = self.triggers.code.text.read(cx).value().to_string();
        let base: HashSet<&str> = self.triggers.code.base.lines().collect();
        let lines: Vec<AnyElement> = text
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let added = !line.trim().is_empty() && !base.contains(line);
                div()
                    .flex()
                    .flex_row()
                    .min_h(t().d(19.))
                    .when(added, |el| el.bg(rgb(t().p.accent_soft)))
                    .child(
                        div()
                            .w(t().d(38.))
                            .flex_none()
                            .pr(t().d(9.))
                            .text_right()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child(format!("{}", i + 1)),
                    )
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_size(t().fs(10.5))
                            .text_color(rgb(if added { t().p.accent_fg } else { t().p.text_2 }))
                            .child(if line.is_empty() { " ".to_string() } else { line.to_string() }),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .id("trigger-diff-scroll")
            .flex_1()
            .min_h(t().d(0.))
            .overflow_scroll()
            .bg(rgb(t().p.code_bg))
            .p(t().d(6.))
            .child(div().flex().flex_col().children(lines))
            .into_any_element()
    }

    fn trigger_publish_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let item = self
            .triggers
            .data
            .as_ref()
            .and_then(|d| d.triggers.iter().find(|t| Some(&t.name) == self.triggers.code.name.as_ref()))
            .cloned();
        let busy = self.triggers.code.busy;
        let publish = self.triggers.code.publish.clone();
        let err = self.triggers.code.publish_error.clone();
        let steps = ["cargo check", "cargo build", "node index.mjs", "commit", "push"];
        let step_rows: Vec<AnyElement> = steps
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let done = publish.is_some() || (busy && i == 0);
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(t().d(8.))
                    .py(t().d(4.5))
                    .child(
                        div()
                            .w(t().d(16.))
                            .flex_none()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if done { t().p.accent_fg } else { t().p.muted }))
                            .child(format!("{:02}", i + 1)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.text_2))
                                    .child(*label),
                            )
                            .child(
                                div()
                                    .mt(t().d(2.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child(match *label {
                                        "cargo check" => "编不过就不提交",
                                        "cargo build" => "产物落 target/debug",
                                        "node index.mjs" => "重扫索引",
                                        "commit" => "只 add 这个触发器 + index.json",
                                        _ => "origin/main",
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if done { t().p.accent_fg } else { t().p.muted }))
                            .child(if busy { "跑…".to_string() } else if done { "通过".to_string() } else { "待".to_string() }),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .w(t().d(372.))
            .flex_none()
            .h_full()
            .bg(rgb(t().p.panel_alt))
            .border_l(t().d(1.))
            .border_color(rgb(t().p.line))
            .child(
                div()
                    .p(t().d(16.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .text_size(t().fs(12.))
                            .text_color(rgb(t().p.text))
                            .child(format!(
                                "发布 {}",
                                self.triggers.code.name.clone().unwrap_or_default()
                            )),
                    )
                    .child(
                        div()
                            .mt(t().d(3.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child("五步跑完才推；失败就停在出错那一步。"),
                    ),
            )
            .child(
                div()
                    .id("trigger-publish-scroll")
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .p(t().d(14.))
                            .border_b(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .mb(t().d(8.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.muted))
                                            .child("流水线"),
                                    )
                                    .child(div().flex_1())
                                    .child(
                                        div()
                                            .text_size(t().fs(9.5))
                                            .text_color(rgb(t().p.muted))
                                            .child(if busy {
                                                "发布中…".to_string()
                                            } else if let Some(p) = &publish {
                                                format!(
                                                    "{}{}",
                                                    if p.pushed { "已推送" } else { "未推送" },
                                                    p.head.clone().map(|h| format!(" · {h}")).unwrap_or_default()
                                                )
                                            } else {
                                                "待发布".to_string()
                                            }),
                                    ),
                            )
                            .child(div().flex().flex_col().children(step_rows)),
                    )
                    .child(
                        div()
                            .p(t().d(14.))
                            .border_b(t().d(1.))
                            .border_color(rgb(t().p.line_soft))
                            .child(
                                div()
                                    .mb(t().d(8.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("参数声明"),
                            )
                            .children(item.as_ref().map(|it| {
                                let cards: Vec<AnyElement> = it.params.iter().map(|p| self.param_card(p)).collect();
                                div().flex().flex_col().gap(t().d(6.)).children(cards).into_any_element()
                            }))
                            .children(item.is_none().then(|| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("没有参数")
                            })),
                    )
                    .child(
                        div()
                            .p(t().d(14.))
                            .child(
                                div()
                                    .mb(t().d(6.))
                                    .text_size(t().fs(9.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("结果"),
                            )
                            .children(err.as_ref().map(|e| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.danger))
                                    .child(e.clone())
                            }))
                            .children(publish.as_ref().map(|p| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(if p.pushed { t().p.accent_fg } else { t().p.warn }))
                                    .child(format!(
                                        "committed={} pushed={} ahead={} {}",
                                        p.committed,
                                        p.pushed,
                                        p.ahead,
                                        p.push_error
                                    ))
                            }))
                            .children((err.is_none() && publish.is_none()).then(|| {
                                div()
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.muted))
                                    .child("点下面的「发布」开始。")
                            })),
                    ),
            )
            .child(
                div()
                    .p(t().d(14.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line_soft))
                    .child(
                        div()
                            .id("trigger-publish-btn")
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(t().d(6.))
                            .h(t().d(32.))
                            .rounded(t().r(6.))
                            .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if !this.triggers.code.busy {
                                    this.publish_trigger(cx);
                                }
                            }))
                            .child(
                                Icon::new(IconName::Upload)
                                    .size(t().d(14.))
                                    .text_color(rgb(t().p.on_accent)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(if busy { "发布中…" } else { "发布" }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Desktop {
    fn trigger_wizard(&self, cx: &mut Context<Self>) -> AnyElement {
        let step = self.triggers.wizard.step;
        let busy = self.triggers.wizard.busy;
        let err = self.triggers.wizard.err.clone();
        let steps = ["身份", "参数声明", "骨架预览", "建"];
        let step_bar: Vec<AnyElement> = steps
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let n = (i + 1) as u8;
                let on = n == step;
                let done = n < step;
                div()
                    .flex()
                    .items_center()
                    .gap(t().d(8.))
                    .child(
                        div()
                            .size(t().d(20.))
                            .rounded(t().r(10.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(if on {
                                t().p.accent
                            } else if done {
                                t().p.accent_soft
                            } else {
                                t().p.panel
                            }))
                            .border(t().d(1.))
                            .border_color(rgb(if on {
                                t().p.accent
                            } else if done {
                                t().p.accent_soft
                            } else {
                                t().p.line
                            }))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(if on {
                                t().p.on_accent
                            } else if done {
                                t().p.accent_fg
                            } else {
                                t().p.muted
                            }))
                            .child(if done { "✓".to_string() } else { format!("{n:02}") }),
                    )
                    .child(
                        div()
                            .text_size(t().fs(11.))
                            .text_color(rgb(if on { t().p.text } else { t().p.muted }))
                            .child(*label),
                    )
                    .children((i + 1 < steps.len()).then(|| {
                        div()
                            .w(t().d(44.))
                            .h(t().d(1.))
                            .mx(t().d(10.))
                            .bg(rgb(t().p.line))
                    }))
                    .into_any_element()
            })
            .collect();

        let form: AnyElement = match step {
            1 => div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(t().fs(16.))
                        .text_color(rgb(t().p.text))
                        .child("先给它一个身份"),
                )
                .child(
                    div()
                        .mt(t().d(5.))
                        .text_size(t().fs(11.))
                        .text_color(rgb(t().p.muted))
                        .child("目录名 = name = 产物名，三合一，建完改不了。"),
                )
                .child(wizard_field("名字", "建完改不了", div().h(t().d(31.)).child(Input::new(&self.triggers.wizard.name)).into_any_element()))
                .child(wizard_field("一句话说明", "卡片上就靠它认人", div().h(t().d(31.)).child(Input::new(&self.triggers.wizard.summary)).into_any_element()))
                .child(wizard_field("类别", "可空 = 未分类；写个不存在的名字就顺手建了", div().h(t().d(31.)).child(Input::new(&self.triggers.wizard.category)).into_any_element()))
                .into_any_element(),
            2 => div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(t().fs(16.))
                        .text_color(rgb(t().p.text))
                        .child("这个触发器要人填什么"),
                )
                .child(
                    div()
                        .mt(t().d(5.))
                        .text_size(t().fs(11.))
                        .text_color(rgb(t().p.muted))
                        .child("每行一个参数：name|type|label|required|options。type 只用 epoch / number / string / text / boolean / select。"),
                )
                .child(
                    div()
                        .mt(t().d(16.))
                        .child(code_block(&self.wizard_params_text())),
                )
                .child(
                    div()
                        .mt(t().d(8.))
                        .text_size(t().fs(10.))
                        .text_color(rgb(t().p.muted))
                        .child("建完会写进 nous.json 的 params；写坏了可以在代码层改。"),
                )
                .into_any_element(),
            _ => div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(t().fs(16.))
                        .text_color(rgb(t().p.text))
                        .child("建之前最后看一眼"),
                )
                .child(
                    div()
                        .mt(t().d(5.))
                        .text_size(t().fs(11.))
                        .text_color(rgb(t().p.muted))
                        .child("三份文件已经生成，形状校验通过。建完只在工作区，不 commit、不 push。"),
                )
                .child(
                    div()
                        .mt(t().d(16.))
                        .flex()
                        .flex_col()
                        .gap(t().d(6.))
                        .child(check_line("名字合格，目录不存在"))
                        .child(check_line("必填四项齐全（name / version / summary / category）"))
                        .child(check_line("参数声明合法（epoch / select + options）"))
                        .child(check_line("骨架能编过（println 一行 JSON）")),
                )
                .child(
                    div()
                        .mt(t().d(16.))
                        .p(t().d(10.))
                        .rounded(t().r(7.))
                        .bg(rgb(t().p.warn_soft))
                        .text_size(t().fs(10.))
                        .text_color(rgb(t().p.warn))
                        .child("建完是「未发布」：没有产物、不在 HEAD，调度器看不到它。发布才会 check + build + index + commit + push。"),
                )
                .into_any_element(),
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .flex_col()
            .bg(rgb(t().p.panel))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(10.))
                    .h(t().d(52.))
                    .px(t().d(20.))
                    .border_b(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .id("trigger-wizard-cancel")
                            .flex()
                            .items_center()
                            .gap(t().d(5.))
                            .h(t().d(24.))
                            .px(t().d(8.))
                            .rounded(t().r(5.))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.trigger_layer(Layer::Main, cx);
                            }))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(t().d(13.))
                                    .text_color(rgb(t().p.muted)),
                            )
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.muted))
                                    .child("取消"),
                            ),
                    )
                    .child(div().flex().flex_row().items_center().gap(t().d(0.)).children(step_bar))
                    .child(div().flex_1())
                    .child(
                        div()
                            .h(t().d(19.))
                            .px(t().d(6.))
                            .flex()
                            .items_center()
                            .rounded(t().r(4.))
                            .bg(rgb(t().p.surface))
                            .text_size(t().fs(9.5))
                            .text_color(rgb(t().p.muted))
                            .child("建完是未发布"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(t().d(0.))
                    .child(
                        div()
                            .id("trigger-wizard-form")
                            .flex()
                            .flex_col()
                            .w(t().d(470.))
                            .flex_none()
                            .h_full()
                            .overflow_y_scroll()
                            .p(t().d(22.))
                            .border_r(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .child(form)
                            .children(err.as_ref().map(|e| {
                                div()
                                    .mt(t().d(12.))
                                    .text_size(t().fs(10.5))
                                    .text_color(rgb(t().p.danger))
                                    .child(e.clone())
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(t().d(0.))
                            .bg(rgb(t().p.panel_alt))
                            .child(
                                div()
                                    .p(t().d(16.))
                                    .border_b(t().d(1.))
                                    .border_color(rgb(t().p.line_soft))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(t().d(8.))
                                            .child(
                                                div()
                                                    .text_size(t().fs(11.))
                                                    .text_color(rgb(t().p.text))
                                                    .child("预览"),
                                            )
                                            .child(
                                                div()
                                                    .h(t().d(16.))
                                                    .px(t().d(5.))
                                                    .flex()
                                                    .items_center()
                                                    .rounded(t().r(3.))
                                                    .bg(rgb(t().p.surface))
                                                    .text_size(t().fs(9.))
                                                    .text_color(rgb(t().p.muted))
                                                    .child(short(self.triggers.wizard.name.read(cx).value().as_ref(), 40)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .mt(t().d(3.))
                                            .text_size(t().fs(10.))
                                            .text_color(rgb(t().p.muted))
                                            .child("参数一改，这里跟着变。"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("trigger-wizard-preview")
                                    .flex_1()
                                    .min_h(t().d(0.))
                                    .overflow_y_scroll()
                                    .p(t().d(20.))
                                    .child(code_block(&self.wizard_preview(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(t().d(8.))
                    .h(t().d(56.))
                    .px(t().d(20.))
                    .border_t(t().d(1.))
                    .border_color(rgb(t().p.line))
                    .child(
                        div()
                            .text_size(t().fs(10.))
                            .text_color(rgb(t().p.muted))
                            .child(match step {
                                1 => "身份 · 下一步填参数".to_string(),
                                2 => "参数声明 · 下一步看骨架".to_string(),
                                _ => "4 项校验通过 · 建完是未发布".to_string(),
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("trigger-wizard-prev")
                            .h(t().d(28.))
                            .px(t().d(11.))
                            .flex()
                            .items_center()
                            .rounded(t().r(6.))
                            .border(t().d(1.))
                            .border_color(rgb(t().p.line))
                            .cursor_pointer()
                            .hover(|h| h.bg(rgb(t().p.hover)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let step = this.triggers.wizard.step;
                                if step <= 1 {
                                    this.trigger_layer(Layer::Main, cx);
                                } else {
                                    this.wizard_step(step - 1, cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.text_2))
                                    .child(if step <= 1 { "取消" } else { "上一步" }),
                            ),
                    )
                    .child(
                        div()
                            .id("trigger-wizard-next")
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(t().d(28.))
                            .px(t().d(12.))
                            .rounded(t().r(6.))
                            .bg(rgb(if busy { t().p.disabled } else { t().p.accent }))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.triggers.wizard.busy {
                                    return;
                                }
                                let step = this.triggers.wizard.step;
                                if step < 3 {
                                    this.wizard_step(step + 1, cx);
                                } else {
                                    this.create_trigger(window, cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_size(t().fs(11.))
                                    .text_color(rgb(t().p.on_accent))
                                    .child(match step {
                                        1 => "下一步：参数",
                                        2 => "下一步：看骨架",
                                        _ if busy => "建…",
                                        _ => "建 morning-review",
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// 向导里那两行参数（演示版固定）。建的时候原样写进 nous.json。
    fn wizard_params_text(&self) -> String {
        "at|epoch|每天几点|true\ntone|select|语气|false|简洁,详细,只列结论".to_string()
    }

    fn wizard_preview(&self, cx: &mut Context<Self>) -> String {
        let name = {
            let v = self.triggers.wizard.name.read(cx).value().trim().to_string();
            if v.is_empty() { "morning-review".to_string() } else { v }
        };
        let summary = {
            let v = self.triggers.wizard.summary.read(cx).value().trim().to_string();
            if v.is_empty() { "每天第一条会话时，按昨天的日志起一条复盘".to_string() } else { v }
        };
        let category = self.triggers.wizard.category.read(cx).value().trim().to_string();
        let params = self.wizard_params_text();
        let mut lines = vec![
            "{".to_string(),
            format!("  \"name\": \"{name}\","),
            "  \"version\": \"0.1.0\",".to_string(),
            format!("  \"summary\": \"{summary}\","),
            format!("  \"category\": [\"{category}\"],"),
            format!("  \"binary\": \"{name}\","),
            "  \"params\": [".to_string(),
        ];
        let parsed = parse_params(&params);
        for (i, p) in parsed.iter().enumerate() {
            let comma = if i + 1 == parsed.len() { "" } else { "," };
            lines.push(format!("    {}{comma}", p));
        }
        lines.push("  ]".to_string());
        lines.push("}".to_string());
        lines.join("\n")
    }

    fn trigger_confirm_scrim(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("trigger-confirm-scrim")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(rgba(t().p.scrim))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.triggers.confirm.open = false;
                cx.notify();
            }))
            .into_any_element()
    }

    fn trigger_confirm(&self, cx: &mut Context<Self>) -> AnyElement {
        let name = self.triggers.confirm.name.clone();
        let matches = self.triggers.confirm.input.read(cx).value().trim() == name;
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(t().d(420.))
                    .flex()
                    .flex_col()
                    .p(t().d(20.))
                    .rounded(t().r(10.))
                    .bg(rgb(t().p.panel))
                    .border(t().d(1.))
                    .border_color(rgb(t().p.danger_soft))
                    .shadow_lg()
                    .child(
                        div()
                            .text_size(t().fs(14.))
                            .text_color(rgb(t().p.text))
                            .child(format!("归档 {name}？")),
                    )
                    .child(
                        div()
                            .mt(t().d(7.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.muted))
                            .child(format!("整个项目目录会移到 .archive/{name}/ 并提交 + 推送。GitHub 上不再有它，但备份还在仓库里，随时能恢复。")),
                    )
                    .child(
                        div()
                            .mt(t().d(13.))
                            .text_size(t().fs(10.5))
                            .text_color(rgb(t().p.text_2))
                            .child(format!("手输名字 {name} 确认")),
                    )
                    .child(
                        div()
                            .mt(t().d(6.))
                            .h(t().d(30.))
                            .child(Input::new(&self.triggers.confirm.input)),
                    )
                    .child(
                        div()
                            .mt(t().d(14.))
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(t().d(8.))
                            .child(
                                div()
                                    .id("trigger-confirm-cancel")
                                    .h(t().d(28.))
                                    .px(t().d(11.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .border(t().d(1.))
                                    .border_color(rgb(t().p.line))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(t().p.hover)))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.triggers.confirm.open = false;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.text_2))
                                            .child("取消"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("trigger-confirm-ok")
                                    .h(t().d(28.))
                                    .px(t().d(11.))
                                    .flex()
                                    .items_center()
                                    .rounded(t().r(6.))
                                    .bg(rgb(if matches { t().p.danger } else { t().p.disabled }))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if this.triggers.confirm.input.read(cx).value().trim()
                                            == this.triggers.confirm.name
                                        {
                                            this.confirm_archive(cx);
                                        }
                                    }))
                                    .child(
                                        div()
                                            .text_size(t().fs(11.))
                                            .text_color(rgb(t().p.on_accent))
                                            .child("归档并推送"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn check_line(text: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t().d(6.))
        .child(
            Icon::new(IconName::Check)
                .size(t().d(12.))
                .text_color(rgb(t().p.accent)),
        )
        .child(
            div()
                .text_size(t().fs(10.5))
                .text_color(rgb(t().p.text_2))
                .child(text.to_string()),
        )
        .into_any_element()
}

fn wizard_field(label: &str, hint: &str, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .mt(t().d(16.))
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(t().d(7.))
                .child(
                    div()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.text_2))
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.muted))
                        .child(hint.to_string()),
                ),
        )
        .child(div().mt(t().d(6.)).child(body))
        .into_any_element()
}

fn code_block(text: &str) -> AnyElement {
    let lines: Vec<AnyElement> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            div()
                .flex()
                .flex_row()
                .min_h(t().d(19.))
                .child(
                    div()
                        .w(t().d(38.))
                        .flex_none()
                        .pr(t().d(9.))
                        .text_right()
                        .text_size(t().fs(9.5))
                        .text_color(rgb(t().p.muted))
                        .child(format!("{}", i + 1)),
                )
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_size(t().fs(10.5))
                        .text_color(rgb(t().p.text_2))
                        .child(if line.is_empty() { " ".to_string() } else { line.to_string() }),
                )
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .p(t().d(8.))
        .rounded(t().r(8.))
        .border(t().d(1.))
        .border_color(rgb(t().p.line))
        .bg(rgb(t().p.code_bg))
        .children(lines)
        .into_any_element()
}
