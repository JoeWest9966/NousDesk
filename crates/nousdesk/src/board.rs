//! 「New Manual」盘面。
//!
//! 外观是**轨道**（`demo/new-manual#7` 那一套）：四张参数卡围着中央一颗 `Create`，
//! 虚线把「谁喂给谁」连出来。不再画 Cynefin 那张四象限图 —— 四块色在深色底上
//! 只有 14% alpha，缩进右格里会糊成一块脏砖红。
//!
//! 弹层是**就地弹出**（`demo/new-manual-7#3`）：贴着被点的那张卡，不居中、
//! 不把盘面糊掉 —— 选参数的时候还想看得见盘面。
//!
//! **只占右侧聊天区那一格**，不再盖住整窗：左导航和会话列表永远在，
//! 点某个会话 / Esc 就回到聊天。尺寸基准因此从整窗 vmin 换成右格 cqmin。
//!
//! 三条纪律（和 demo 对齐，不是我自己编的）：
//! - **一屏不滚**：尺寸全走右格 cqmin，右格变了跟着缩，盘面永远填满右格。
//! - **形状说语义**：模型是「供应商 › 模型 › 思考」三段语法链，画成三列；
//!   技能 / 模板 / 目录是平铺清单，因为它们不是链（集合 / 单值 / 导航）。
//! - **一个动作一个入口**：Create 只有中间那颗圆，dock 里只放名字 / 机器 / 目录。

// 主题接线：所有硬编码色 / 尺寸都走这里。`t()` 取的是 `Desktop::render`
// 每帧从 `ActiveSkin` 拍下来的帧内快照，不是第二个真相。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

use std::collections::BTreeSet;

use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::prelude::StatefulInteractiveElement as _;
use gpui_kit::*;

use nous_engine::EngineError;
use nous_engine::types::{
    CreateSession, CreatedSession, Dirs, Machines, Model, Models, Prompt, Prompts, Skill, Skills,
};

use crate::app::Desktop;
use crate::board_art as art;

/// 打开着哪一层的弹层。`None` = 就在盘面上。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sheet {
    Model,
    Skills,
    Prompt,
    Dir,
    Machine,
}

/// 盘面的状态。**选择是本地草稿**：这一页还没落库，一切都在内存里，
/// 按 Create 才变成一次 `POST /v1/sessions`。
pub(crate) struct NewChat {
    pub(crate) open: bool,
    focus: FocusHandle,
    name_input: Entity<TextareaState>,
    /// 五份清单，各自独立成功/失败 —— 一份挂了不该把别的拖着一起灰。
    models: Option<Models>,
    skills: Option<Skills>,
    prompts: Option<Prompts>,
    machines: Option<Machines>,
    dirs: Option<Dirs>,
    loading: bool,
    /// 这一页自己的错误带（在椭圆底下那行 `flash` 里显示，不弹窗）。
    load_err: Option<String>,
    flash: Option<String>,

    // ── 选择 ──
    /// `(provider, modelId)`。建会话时原样发回这个对象。
    picked: Option<(String, String)>,
    /// 思考档，`off` … `max`。换模型要按模型的 `thinkingLevels` 重新夹。
    level: String,
    picked_skills: BTreeSet<String>,
    /// `None` = 明确不要模板（会以 `template: null` 发出去）。
    template: Option<String>,
    /// `None` = 本机。
    machine: Option<String>,
    dir: Option<String>,
    /// 目录浏览到哪儿（和 `dir` 是两件事：看的地方和选的地方）。
    dir_at: Option<String>,

    pub(crate) sheet: Option<Sheet>,
    sheet_scroll: ScrollHandle,
    creating: bool,
}

impl NewChat {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let name_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .placeholder("Name · 空着就用引擎给的名字")
        });
        Self {
            open: false,
            focus: cx.focus_handle(),
            name_input,
            models: None,
            skills: None,
            prompts: None,
            machines: None,
            dirs: None,
            loading: false,
            load_err: None,
            flash: None,
            picked: None,
            level: "off".into(),
            picked_skills: BTreeSet::new(),
            template: None,
            machine: None,
            dir: None,
            dir_at: None,
            sheet: None,
            sheet_scroll: ScrollHandle::new(),
            creating: false,
        }
    }

    /// 当前选中的模型（`provider/modelId`），给人看的那种写法。
    fn picked_label(&self) -> Option<String> {
        self.picked.as_ref().map(|(p, m)| format!("{p}/{m}"))
    }

    fn model(&self) -> Option<&Model> {
        let (p, m) = self.picked.as_ref()?;
        self.models
            .as_ref()?
            .items
            .iter()
            .find(|x| &x.provider == p && &x.model_id == m)
    }

    /// 这个模型支持的思考档（从少想到多想）。不支持思考档的给一个 `off`。
    fn levels(&self) -> Vec<String> {
        match self.model() {
            Some(m) => m.levels(),
            None => vec!["off".into()],
        }
    }

    /// 换模型后把档位夹回新模型的表里 —— 和 `pi-ai` 的 `clampThinkingLevel` 同序：
    /// 先往上找，找不到再往下。面板上看到的就是库里会落的值。
    fn clamp_level(&mut self) {
        let levels = self.levels();
        if levels.iter().any(|l| l == &self.level) {
            return;
        }
        let all = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
        let at = all.iter().position(|x| x == &self.level).unwrap_or(0);
        for cand in all.iter().skip(at) {
            if levels.iter().any(|l| l == cand) {
                self.level = (*cand).to_string();
                return;
            }
        }
        for cand in all.iter().take(at).rev() {
            if levels.iter().any(|l| l == cand) {
                self.level = (*cand).to_string();
                return;
            }
        }
        self.level = levels.first().cloned().unwrap_or_else(|| "off".into());
    }

    /// 能勾的技能：加载得起来、也不是 `disable-model-invocation` 那种。
    fn skill_list(&self) -> Vec<&Skill> {
        let mut out: Vec<&Skill> = self
            .skills
            .as_ref()
            .map(|s| {
                s.skills
                    .iter()
                    .filter(|x| x.loadable() && !x.hidden)
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn prompt_list(&self) -> Vec<&Prompt> {
        let mut out: Vec<&Prompt> = self
            .prompts
            .as_ref()
            .map(|p| p.prompts.iter().filter(|x| x.loadable()).collect())
            .unwrap_or_default();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// 圆底下那行状态。**只说现在差什么**，不说欢迎语，也不重复卡片上的值。
    fn hub_sub(&self) -> String {
        if let Some(err) = &self.load_err {
            return err.clone();
        }
        if !self.open {
            return String::new();
        }
        if self.creating {
            return "creating…".into();
        }
        if self.models.is_none() {
            return if self.loading {
                "loading…".into()
            } else {
                "no models".into()
            };
        }
        match self.picked {
            Some(_) => "ready".into(),
            None => "pick a model".into(),
        }
    }
}

/// 路径尾巴：`/a/b/c` → `c`。椭圆里放不下整串。
fn tail_path(path: &str) -> String {
    let p = path.trim_end_matches('/');
    p.rsplit('/').next().unwrap_or(p).to_string()
}

/// 思考档给人看的名字。`off` 在面板上就是「不思考」，不填就是不思考 —— 不撒谎。
fn level_label(level: &str) -> &'static str {
    match level {
        "off" => "不思考",
        "minimal" => "极少",
        "low" => "低",
        "medium" => "中",
        "high" => "高",
        "xhigh" => "很高",
        "max" => "最大",
        _ => "不思考",
    }
}

// ── 动作（全部挂在 `Desktop` 上：状态在这一层，盘面只是它的一块渲染）──────────

impl Desktop {
    pub(crate) fn board_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.newchat.open = true;
        self.newchat.sheet = None;
        self.newchat.flash = None;
        window.focus(&self.newchat.focus, cx);
        cx.notify();
        if self.newchat.models.is_none() && !self.newchat.loading {
            self.board_load(cx);
        }
    }

    pub(crate) fn board_close(&mut self, cx: &mut Context<Self>) {
        self.newchat.open = false;
        self.newchat.sheet = None;
        self.newchat.flash = None;
        cx.notify();
    }

    /// 五份清单一起拉，但**各自吃自己的错**：技能仓库挂了，模型照样能选。
    pub(crate) fn board_load(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.newchat.load_err = Some("还没连上引擎".into());
            cx.notify();
            return;
        };
        self.newchat.loading = true;
        self.newchat.load_err = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            type Got<T> = Result<T, EngineError>;
            let (models, skills, prompts, machines, dirs) = h
                .spawn(async move {
                    let models: Got<Models> = engine.models().await;
                    let skills: Got<Skills> = engine.skills().await;
                    let prompts: Got<Prompts> = engine.prompts().await;
                    let machines: Got<Machines> = engine.machines().await;
                    let dirs: Got<Dirs> = engine.dirs(None).await;
                    (models, skills, prompts, machines, dirs)
                })
                .await
                .unwrap_or_else(|_| {
                    (
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                        Err(EngineError::Engine {
                            status: 0,
                            message: "任务没跑完".into(),
                        }),
                    )
                });

            let _ = this.update(cx, |this, cx| {
                let b = &mut this.newchat;
                b.loading = false;
                if let Ok(m) = models {
                    // 默认模型：盘上那只眼指着的那个；没有就第一个。
                    b.picked = m
                        .default
                        .as_ref()
                        .map(|d| (d.provider.clone(), d.model_id.clone()));
                    if b.picked.is_none() {
                        b.picked = m
                            .items
                            .first()
                            .map(|x| (x.provider.clone(), x.model_id.clone()));
                    }
                    b.models = Some(m);
                    b.clamp_level();
                } else if let Err(e) = models {
                    b.load_err = Some(format!("模型清单：{e}"));
                }
                if let Ok(s) = skills {
                    b.skills = Some(s);
                }
                if let Ok(p) = prompts {
                    // 仓库默认值只用来预勾；选不选仍然是人的事。
                    b.template = p.default.clone();
                    b.prompts = Some(p);
                }
                if let Ok(mm) = machines {
                    b.machines = Some(mm);
                }
                if let Ok(d) = dirs {
                    b.dir_at = d.path.clone();
                    b.dirs = Some(d);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 点目录里的某一层：进去看一眼，**顺手把它当成选择**（省一次点击）。
    fn board_dir_enter(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        self.newchat.dir_at = Some(path.clone());
        self.newchat.dir = Some(path.clone());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            let got = h
                .spawn(async move { engine.dirs(Some(&path)).await })
                .await
                .ok();
            let _ = this.update(cx, |this, cx| {
                match got {
                    Some(Ok(d)) => {
                        this.newchat.dir_at = d.path.clone();
                        this.newchat.dirs = Some(d);
                    }
                    Some(Err(e)) => this.newchat.flash = Some(format!("列目录失败：{e}")),
                    None => this.newchat.flash = Some("列目录失败了".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Create：一次发完（模型 / 档位 / 技能 / 模板 / 目录 / 机器 / 名字），
    /// 回来再把盘上那只眼切过去，然后回列表、选中它。
    fn board_create(&mut self, cx: &mut Context<Self>) {
        if self.newchat.creating {
            return;
        }
        let Some((provider, model_id)) = self.newchat.picked.clone() else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            self.newchat.flash = Some("还没连上引擎".into());
            cx.notify();
            return;
        };

        let name = self.newchat.name_input.read(cx).value().to_string();
        let name = name.trim().to_string();
        let body = CreateSession {
            name: (!name.is_empty()).then_some(name),
            model: Some(serde_json::json!({
                "provider": provider,
                "modelId": model_id,
            })),
            thinking_level: Some(serde_json::json!(self.newchat.level)),
            skills: (!self.newchat.picked_skills.is_empty())
                .then(|| self.newchat.picked_skills.iter().cloned().collect()),
            // **总是发**：`Some(None)` 明确就是「不要模板」。
            template: Some(self.newchat.template.clone()),
            cwd: self.newchat.dir.clone(),
            machine: self.newchat.machine.clone(),
            ..Default::default()
        };

        self.newchat.creating = true;
        self.newchat.flash = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let Some(h) = handle else { return };
            let outcome = h
                .spawn(async move {
                    let created: CreatedSession = engine.create(&body).await?;
                    // 创建成功之后才切那只眼；切不过去不该把这个会话判死。
                    let _ = engine.set_selection(Some(created.id)).await;
                    let sessions = engine.sessions().await?.items;
                    Ok::<_, EngineError>((created, sessions))
                })
                .await
                .ok();

            let _ = this.update(cx, |this, cx| {
                this.newchat.creating = false;
                match outcome {
                    Some(Ok((created, sessions))) => {
                        let mut sessions = sessions;
                        sessions.sort_by(|a, b| b.id.cmp(&a.id));
                        this.sessions = sessions;
                        this.newchat.open = false;
                        this.newchat.sheet = None;
                        this.tab = crate::app::Tab::Chats;
                        this.newchat.flash = None;
                        this.select_session(created.id, true, cx);
                    }
                    Some(Err(e)) => this.newchat.flash = Some(format!("创建失败：{e}")),
                    None => this.newchat.flash = Some("创建失败了".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── 渲染 ──────────────────────────────────────────────────────────

    /// 整块盘面。**顶替**右侧聊天区那一格（`app.rs` 的 render 里换掉 `render_chat`）。
    pub(crate) fn render_board(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // 基准是**右格**不是整窗：盘面现在是三栏里的一格。
        let m = art::M::new(window.viewport_size(), self.tab);
        let b = &self.newchat;
        let model_value = b.picked_label().unwrap_or_else(|| "—".into());

        let body = div()
            .relative()
            .flex_1()
            .min_h(t().d(0.))
            .child(self.board_wires(&m))
            .child(self.board_core(&m, cx))
            .child(self.board_card(
                &m,
                art::Slot::Model,
                "board-model",
                "MODEL",
                "Model",
                model_value,
                "change ›",
                b.models.is_some(),
                cx,
                |this, _cx| this.newchat.sheet = Some(Sheet::Model),
            ))
            .child(self.board_context(&m, cx))
            .child(self.board_card(
                &m,
                art::Slot::Ext,
                "board-ext",
                "EXTENSIONS",
                "Extensions",
                "none".into(),
                "choose ›",
                true,
                cx,
                |this, cx| {
                    // 数据在 vbe4pt 块里，这一版没有端点 —— 如实说，不假装能选。
                    this.newchat.flash = Some("扩展来自扩展块（vbe4pt），这一版没接".into());
                    cx.notify();
                },
            ))
            .child(self.board_card(
                &m,
                art::Slot::Perms,
                "board-perms",
                "PERMISSIONS",
                "Permissions",
                "—".into(),
                "soon",
                false,
                cx,
                |_, _| {},
            ))
            .child(self.board_dock(&m, cx));

        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            // flex 项不写 min_w(0) 会被内容撑到比右格还宽（坑 16）。
            .min_w(t().d(0.))
            .h_full()
            .bg(rgb(art::BG))
            .text_color(rgb(art::FG))
            .text_size(m.f(1.75, 12., 16.))
            .line_height(relative(1.5))
            .track_focus(&b.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    if this.newchat.sheet.is_some() {
                        this.newchat.sheet = None;
                    } else {
                        this.board_close(cx);
                        return;
                    }
                    cx.notify();
                }
            }))
            .child(body)
            .children(self.board_sheet(&m, cx))
            .into_any_element()
    }

    /// 四条虚线：圆心 → 四张卡（终点会被卡片盖住，正好停在边上）。
    fn board_wires(&self, m: &art::M) -> AnyElement {
        let m = *m;
        let slots = [
            art::Slot::Model,
            art::Slot::Context,
            art::Slot::Ext,
            art::Slot::Perms,
        ];
        canvas(
            move |_bounds, _, _| slots.map(|s| art::wire(&m, s)),
            |bounds, wires, window, _| {
                // `paint_path` 吃的是**绝对坐标**：canvas 的原点不一定在 (0,0)，
                // 忘了加就是整条线平移一个输出区偏移（现状 hub 那颗椭圆也踩过）。
                let (ox, oy) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                for (a, b) in wires {
                    let mut pb = PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(6.)]);
                    pb.move_to(point(px(ox + a.0), px(oy + a.1)));
                    pb.line_to(point(px(ox + b.0), px(oy + b.1)));
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, rgb(art::WIRE));
                    }
                }
            },
        )
        .size_full()
        .into_any_element()
    }

    /// 中央那颗白圆：整页唯一的主动作。
    fn board_core(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let (ccx, ccy) = art::core_center(&m);
        let r = art::core_r(&m);
        let halo = 18.;
        let b = &self.newchat;
        // 状态行：`flash` 是失败（绿字和 ready 撞色，所以用红调），否则说还差什么。
        let status = b.flash.clone().map(|f| (f, rgb(0xd14d41))).or_else(|| {
            let s = b.hub_sub();
            (!s.is_empty()).then(|| (s, rgb(art::OK)))
        });

        div()
            .absolute()
            .left(px(ccx - r - halo))
            .top(px(ccy - r - halo))
            .w(px(2. * (r + halo)))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w(px(2. * (r + halo)))
                    .h(px(2. * (r + halo)))
                    .rounded_full()
                    .bg(rgb(art::ACCENT).opacity(0.035))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id(ElementId::Name("board-create".into()))
                            .size(px(2. * r))
                            .rounded_full()
                            .bg(rgb(art::ACCENT))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(art::BG))
                            .hover(|s| s.opacity(0.94))
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.board_create(cx)),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.6, 12., 17.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("+ Create"),
                            )
                            .child(
                                div()
                                    .mt(m.v(0.3))
                                    .font_family(art::MONO)
                                    .text_size(m.f(0.7, 6.5, 8.5))
                                    .text_color(rgb(0x6a6660))
                                    .child("NEW MANUAL · DISORDER"),
                            ),
                    ),
            )
            .children(status.map(|(s, color)| {
                div()
                    .mt(px(10.))
                    .w(px(2. * (r + halo)))
                    .text_center()
                    .font_family(art::MONO)
                    .text_size(m.f(1.15, 8.5, 10.5))
                    .text_color(color)
                    .child(s)
            }))
            .into_any_element()
    }

    /// 一张参数卡（Model / Extensions / Permissions 共用这个形状）。
    #[allow(clippy::too_many_arguments)]
    fn board_card(
        &self,
        m: &art::M,
        slot: art::Slot,
        id: &'static str,
        k: &'static str,
        title: &'static str,
        value: String,
        tag: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let m = *m;
        let r = art::node_rect(&m, slot);
        div()
            .id(ElementId::Name(id.into()))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .gap(m.v(0.25))
            .px(m.v(1.1))
            .py(m.v(0.85))
            .bg(rgb(art::PANEL))
            .border_1()
            .border_color(rgb(art::BORDER))
            .rounded(t().r(10.))
            .when(enabled, |el| {
                el.hover(|s| s.border_color(rgb(art::LINE)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| on_click(this, cx)))
            })
            .when(!enabled, |el| el.opacity(0.55))
            .child(
                div()
                    .font_family(art::MONO)
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::FAINT))
                    .child(k),
            )
            .child(
                div()
                    .text_size(m.f(1.6, 11., 14.))
                    .font_weight(FontWeight::BOLD)
                    .child(title),
            )
            .child(
                div()
                    .mt(m.v(0.45))
                    .min_w(t().d(0.))
                    .font_family(art::MONO)
                    .text_size(m.f(1.3, 10., 12.))
                    .text_color(rgb(art::MUTED))
                    .truncate()
                    .child(value),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(m.f(1.15, 8.5, 10.5))
                    .text_color(rgb(art::FAINT))
                    .child(tag),
            )
            .into_any_element()
    }

    /// Context 那张卡：里面是**两个**可点子块 —— 技能是集合（多选），
    /// 模板是单值（单选），塞进一个按钮里会让人以为它们一样。
    fn board_context(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        let r = art::node_rect(&m, art::Slot::Context);
        let skills = format!("{} / {}", b.picked_skills.len(), b.skill_list().len());
        let prompt = b.template.clone().unwrap_or_else(|| "none".into());
        let sub =
            |id: &'static str, k: &'static str, v: String, cx: &mut Context<Self>, open: Sheet| {
                div()
                    .id(ElementId::Name(id.into()))
                    .flex()
                    .flex_col()
                    .gap(m.v(0.2))
                    .flex_1()
                    .min_w(t().d(0.))
                    .px(m.v(0.7))
                    .py(m.v(0.45))
                    .border_1()
                    .border_color(rgb(art::BORDER))
                    .rounded(t().r(7.))
                    .bg(rgb(art::BG))
                    .hover(|s| s.border_color(rgb(art::LINE)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.newchat.sheet = Some(open);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .font_family(art::MONO)
                            .text_size(m.f(1.05, 8., 10.))
                            .text_color(rgb(art::FAINT))
                            .child(k),
                    )
                    .child(
                        div()
                            .font_family(art::MONO)
                            .text_size(m.f(1.2, 9., 11.))
                            .text_color(rgb(art::FG))
                            .truncate()
                            .child(v),
                    )
            };

        div()
            .id(ElementId::Name("board-context".into()))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .gap(m.v(0.45))
            .px(m.v(1.1))
            .py(m.v(0.85))
            .bg(rgb(art::PANEL))
            .border_1()
            .border_color(rgb(art::BORDER))
            .rounded(t().r(10.))
            .child(
                div()
                    .font_family(art::MONO)
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::FAINT))
                    .child("CONTEXT"),
            )
            .child(
                div()
                    .flex()
                    .gap(m.v(0.6))
                    .child(sub("board-skills", "SKILLS", skills, cx, Sheet::Skills))
                    .child(sub("board-prompt", "PROMPT", prompt, cx, Sheet::Prompt)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::FAINT))
                    .child("随会话注入的知识与语气"),
            )
            .into_any_element()
    }

    /// 底部一条：名字 + 机器 + 目录。Create 不在这儿 —— 它是中间那颗圆，
    /// 一个动作一个入口。
    fn board_dock(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let m = *m;
        let b = &self.newchat;
        let d = art::dock_rect(&m);
        let machine = b.machine.clone().unwrap_or_else(|| "本机".into());
        let dir = b
            .dir
            .as_ref()
            .map(|p| tail_path(p))
            .unwrap_or_else(|| "—".into());
        let chip = |id: &'static str, text: String, cx: &mut Context<Self>, sheet: Sheet| {
            div()
                .id(ElementId::Name(id.into()))
                .flex_none()
                .px(m.v(0.9))
                .py(m.v(0.4))
                .border_1()
                .border_color(rgb(art::BORDER))
                .rounded(t().r(7.))
                .font_family(art::MONO)
                .text_size(m.f(1.2, 9., 11.))
                .text_color(rgb(art::MUTED))
                .hover(|s| s.border_color(rgb(art::LINE)).text_color(rgb(art::FG)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.newchat.sheet = Some(sheet);
                    cx.notify();
                }))
                .child(format!("{text} ›"))
        };

        div()
            .absolute()
            .left(px(d.x))
            .top(px(d.y))
            .w(px(d.w))
            .h(px(d.h))
            .flex()
            .items_center()
            .gap(m.v(0.7))
            .px(m.v(0.8))
            .bg(rgb(0x111114).opacity(0.94))
            .border_1()
            .border_color(rgb(art::BORDER))
            .rounded(t().r(12.))
            .child(
                div().flex_1().min_w(t().d(0.)).px(m.v(0.6)).child(
                    Textarea::new(&b.name_input)
                        .appearance(false)
                        .bordered(false),
                ),
            )
            .child(chip("dock-machine", machine, cx, Sheet::Machine))
            .child(chip("dock-dir", dir, cx, Sheet::Dir))
            .child(
                div()
                    .flex_none()
                    .px(m.v(0.6))
                    .font_family(art::MONO)
                    .text_size(m.f(1.05, 8., 10.))
                    .text_color(rgb(art::FAINT))
                    .child("↵ CREATE"),
            )
            .into_any_element()
    }

    /// 弹层：「就地弹出」。**贴着被点的那张卡**，不是居中一个模态框 ——
    /// 选参数的时候还想看得见盘面（点空白处 / Esc 关闭）。
    fn board_sheet(&self, m: &art::M, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = *m;
        let sheet = self.newchat.sheet?;
        let (title, hint, slot) = match sheet {
            Sheet::Model => (
                "选择模型",
                "供应商 › 模型 › 思考 · 点模型即收回",
                Some(art::Slot::Model),
            ),
            Sheet::Skills => (
                "上下文 · 技能",
                "勾完按「完成」· 多选",
                Some(art::Slot::Context),
            ),
            Sheet::Prompt => (
                "上下文 · 提示词模板",
                "单值：这条每轮都带 · 再点一次取消",
                Some(art::Slot::Context),
            ),
            Sheet::Dir => ("工作目录", "列的是引擎那台机器上的目录 · 点一层进去", None),
            Sheet::Machine => ("机器", "会话跑在哪台机器上 · 默认本机", None),
        };

        let pop = art::pop_rect(&m, slot, sheet == Sheet::Model);
        let body = match sheet {
            Sheet::Model => self.sheet_model(&m, pop.w, cx),
            Sheet::Skills => self.sheet_skills(&m, cx),
            Sheet::Prompt => self.sheet_prompt(&m, cx),
            Sheet::Dir => self.sheet_dir(&m, cx),
            Sheet::Machine => self.sheet_machine(&m, cx),
        };

        let header = div()
            .flex_none()
            .flex()
            .items_start()
            .justify_between()
            .gap(m.v(1.))
            .px(m.v(1.6))
            .pt(m.v(1.3))
            .pb(m.v(0.95))
            .border_b(t().d(1.))
            .border_color(rgb(art::BORDER))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(m.v(0.25))
                    .child(
                        div()
                            .text_size(m.f(1.3, 10., 12.5))
                            .font_weight(FontWeight::BOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .italic()
                            .text_size(m.f(1.05, 8., 10.))
                            .text_color(rgb(art::MUTED))
                            .child(hint),
                    ),
            )
            .child(
                div()
                    .id(ElementId::Name("pop-close".into()))
                    .flex_none()
                    .px(m.v(0.45))
                    .py(m.v(0.05))
                    .rounded(t().r(5.))
                    .text_size(m.f(1.1, 8.5, 10.5))
                    .text_color(rgb(art::MUTED))
                    .hover(|s| s.bg(rgb(art::SURFACE)).text_color(rgb(art::FG)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.newchat.sheet = None;
                        cx.notify();
                    }))
                    .child("✕"),
            );

        let footer = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(m.v(1.4))
            .py(m.v(0.75))
            .border_t(t().d(1.))
            .border_color(rgb(art::BORDER))
            .child(
                div()
                    .font_family(art::MONO)
                    .text_size(m.f(0.8, 6.5, 8.5))
                    .text_color(rgb(art::FAINT))
                    .child("Esc 关闭 · 点空白处也关"),
            )
            .child(
                div()
                    .id(ElementId::Name("board-sheet-done".into()))
                    .px(m.v(1.0))
                    .py(m.v(0.3))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(rgb(art::ACCENT))
                    .bg(rgb(art::ACCENT).opacity(0.14))
                    .text_size(m.f(1.05, 8.5, 11.))
                    .hover(|s| s.bg(rgb(art::ACCENT).opacity(0.24)))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.newchat.sheet = None;
                        cx.notify();
                    }))
                    .child("完成 ›"),
            );

        let cap = match pop.anchor {
            art::Anchor::Top(y) => art::dock_rect(&m).y - 14. - y,
            art::Anchor::Bottom(_) => m.h * 0.62,
        };
        let max_h = cap.clamp(220., m.h * 0.62);
        let panel = div()
            .id(ElementId::Name("board-pop".into()))
            .absolute()
            .left(px(pop.x))
            .w(px(pop.w))
            .flex()
            .flex_col()
            // 显式给上限：内容多就滚，别把面板顶出右格、也别压到 dock。
            .max_h(px(max_h))
            .bg(rgb(art::PANEL))
            .border_1()
            .border_color(rgb(art::BORDER))
            .rounded(t().r(12.))
            .overflow_hidden();
        let panel = match pop.anchor {
            art::Anchor::Top(y) => panel.top(px(y)),
            art::Anchor::Bottom(y) => panel.bottom(px(y)),
        };

        // 遮罩一整块盖住右格，浮层本体用 `block_mouse_except_scroll` 把
        // 面板范围内的点击/滚轮留下 —— 见 坑.md 59（`occlude()` 会连滚轮一起挡）。
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .id(ElementId::Name("board-scrim".into()))
                .bg(rgba(0x05050647))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.newchat.sheet = None;
                    cx.notify();
                }))
                .child(
                    panel
                        .block_mouse_except_scroll()
                        .child(header)
                        .child(body)
                        .child(footer),
                )
                .into_any_element(),
        )
    }

    /// 模型弹层：三列铁路图。**这是这一页的灵魂** —— 供应商 › 模型 › 思考 是
    /// 一条有依赖的语法链，画成三个滚动的列表就看不出「先选谁再选谁」。
    fn sheet_model(&self, m: &art::M, sheet_w: f32, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let (providers, models, levels) = {
            let providers: Vec<String> = b
                .models
                .as_ref()
                .map(|x| {
                    let mut v: Vec<String> = Vec::new();
                    for it in &x.items {
                        if !v.contains(&it.provider) {
                            v.push(it.provider.clone());
                        }
                    }
                    v
                })
                .unwrap_or_default();
            let cur_provider = b
                .picked
                .as_ref()
                .map(|(p, _)| p.clone())
                .unwrap_or_default();
            let models: Vec<String> = b
                .models
                .as_ref()
                .map(|x| {
                    x.items
                        .iter()
                        .filter(|it| it.provider == cur_provider)
                        .map(|it| it.model_id.clone())
                        .collect()
                })
                .unwrap_or_default();
            (providers, models, b.levels())
        };

        let pitch = f32::from(m.v(2.6)).clamp(26., 34.);
        let rows = providers.len().max(models.len()).max(levels.len()).max(1);
        let total_h = pitch * rows as f32 + 16.;

        // 三列的横向位置：列宽按比例切，和页面一样（列窄了不换版面，只是更挤）。
        let (cols, xs) = sheet_lanes(sheet_w, f32::from(m.v(1.)));

        let trunk = rgb(art::BORDER);
        let hot = rgb(art::ACCENT);
        let mut layers: Vec<AnyElement> = Vec::new();

        // 每条链：一根竖轨 + 每行一个点 + 一根横支；选中的那条换成 accent。
        for (col, items) in [&providers, &models, &levels].into_iter().enumerate() {
            let x = xs[col] + 8.;
            let n = items.len();
            if n > 0 {
                layers.push(
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(8. + pitch / 2.))
                        .w(t().d(1.))
                        .h(px(pitch * (n.saturating_sub(1)) as f32))
                        .bg(trunk)
                        .into_any_element(),
                );
            }
            for (row, item) in items.iter().enumerate() {
                let y = 8. + row as f32 * pitch;
                let selected = match col {
                    0 => b.picked.as_ref().map(|(p, _)| p) == Some(item),
                    1 => b.picked.as_ref().map(|(_, mo)| mo) == Some(item),
                    _ => &b.level == item,
                };
                // 点和横支
                layers.push(
                    div()
                        .absolute()
                        .left(px(x - 2.))
                        .top(px(y + pitch / 2. - 2.))
                        .size(t().d(5.))
                        .rounded_full()
                        .bg(if selected { hot } else { trunk })
                        .into_any_element(),
                );
                layers.push(
                    div()
                        .absolute()
                        .left(px(x + 8.))
                        .top(px(y + pitch / 2.))
                        .w(t().d(6.))
                        .h(t().d(1.))
                        .bg(if selected { hot } else { trunk })
                        .into_any_element(),
                );

                let label = if col == 2 {
                    format!("{item} · {}", level_label(item))
                } else {
                    item.clone()
                };
                let action = match col {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                };
                layers.push(
                    div()
                        .id(ElementId::Name(format!("pill-{col}-{row}").into()))
                        .absolute()
                        .left(px(x + 16.))
                        .top(px(y))
                        .w(px(cols[col] - 30.))
                        .h(px(pitch - 6.))
                        .flex()
                        .items_center()
                        .px(t().d(8.))
                        .rounded(t().r(4.))
                        .border_1()
                        .border_color(if selected { hot } else { rgb(art::BORDER) })
                        .bg(if selected {
                            rgb(art::ACCENT).opacity(0.16)
                        } else {
                            rgb(art::SURFACE)
                        })
                        .text_size(m.f(1.45, 10., 13.))
                        .font_family(art::MONO)
                        .truncate()
                        .hover(|s| s.bg(rgb(art::SURFACE)))
                        .on_click(cx.listener({
                            let label = label.clone();
                            move |this, _: &ClickEvent, _, cx| {
                                match action {
                                    0 => {
                                        // 换供应商：模型回到这家的第一个，档位重新夹。
                                        this.newchat.picked = Some((label.clone(), String::new()));
                                        this.board_pick_first_model();
                                    }
                                    1 => {
                                        if let Some((p, _)) = this.newchat.picked.clone() {
                                            this.newchat.picked = Some((p, label.clone()));
                                            this.newchat.clamp_level();
                                        }
                                    }
                                    _ => this.newchat.level = label.clone(),
                                }
                                cx.notify();
                            }
                        }))
                        .child(label)
                        .into_any_element(),
                );
            }
        }

        div()
            // `on_hover` 挂在 `Stateful` 上：先给 id。
            .id(ElementId::Name("board-model-viewport".into()))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.bar_hover(crate::app::Bar::Sheet, *hovered, cx)
            }))
            .child(
                div()
                    .id(ElementId::Name("board-model-body".into()))
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.newchat.sheet_scroll)
                    .child(div().relative().h(px(total_h)).w_full().children(layers)),
            )
            .child(crate::app::scrollbar::overlay_tone(
                "board-model-scrollbar",
                &self.newchat.sheet_scroll,
                self.bar(crate::app::Bar::Sheet).0,
                self.bar(crate::app::Bar::Sheet).1,
                // 盘面永远是深色 art 调色板：滑块得画成半透明白，不然一根黑线看不见。
                crate::app::scrollbar::Tone::OnDark,
            ))
            .into_any_element()
    }

    /// 换供应商时，把模型换成这一家的第一个（不然会停在别家的 modelId 上）。
    fn board_pick_first_model(&mut self) {
        let Some((provider, _)) = self.newchat.picked.clone() else {
            return;
        };
        let first = self
            .newchat
            .models
            .as_ref()
            .and_then(|m| m.items.iter().find(|x| x.provider == provider))
            .map(|x| x.model_id.clone());
        if let Some(id) = first {
            self.newchat.picked = Some((provider, id));
        }
        self.newchat.clamp_level();
    }

    /// 技能弹层：平铺多选。**故意不用铁路图** —— 技能是集合不是链。
    fn sheet_skills(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let rows: Vec<AnyElement> = b
            .skill_list()
            .into_iter()
            .map(|s| {
                let name = s.name.clone();
                let picked = b.picked_skills.contains(&name);
                let desc = s
                    .description
                    .clone()
                    .unwrap_or_default()
                    .replace(['\n', '\r'], " ");
                let meta = if s.category.is_empty() {
                    String::new()
                } else {
                    s.category.join(" / ")
                };
                div()
                    .id(ElementId::Name(format!("skill-{name}").into()))
                    .flex()
                    .items_start()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.55))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if picked {
                        rgb(art::ACCENT)
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(picked, |el| el.bg(rgb(art::ACCENT).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::ACCENT).opacity(0.07)))
                    .on_click({
                        let name = name.clone();
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.newchat.picked_skills.remove(&name) {
                                this.newchat.picked_skills.insert(name.clone());
                            }
                            cx.notify();
                        })
                    })
                    .child(
                        div()
                            .mt(t().d(2.))
                            .size(m.f(1.1, 10., 14.))
                            .rounded(t().r(3.))
                            .border_1()
                            .border_color(if picked {
                                rgb(art::ACCENT)
                            } else {
                                rgb(art::BORDER)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(m.f(0.85, 8., 11.))
                            .text_color(rgb(art::ACCENT))
                            .child(if picked { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .font_family(art::MONO)
                                    .text_size(m.f(1.5, 11., 13.5))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.35, 10., 12.5))
                                    .text_color(rgb(art::MUTED))
                                    .truncate()
                                    .child(desc),
                            )
                            .when(!meta.is_empty(), |el| {
                                el.child(
                                    div()
                                        .font_family(art::MONO)
                                        .text_size(m.f(1.2, 9., 11.))
                                        .text_color(rgb(art::FAINT))
                                        .child(meta),
                                )
                            }),
                    )
                    .into_any_element()
            })
            .collect();

        self.sheet_list(
            m,
            rows,
            if b.skill_list().is_empty() {
                "这个盘上还没装技能（技能在 68kazk 那块）"
            } else {
                ""
            },
            cx,
        )
    }

    /// 模板弹层：单选。再点一次取消 —— 「明确不要模板」也是一个有效选择。
    fn sheet_prompt(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let rows: Vec<AnyElement> = b
            .prompt_list()
            .into_iter()
            .map(|p| {
                let name = p.name.clone();
                let picked = b.template.as_deref() == Some(name.as_str());
                let desc = p
                    .description
                    .clone()
                    .or_else(|| p.summary.clone())
                    .unwrap_or_default()
                    .replace(['\n', '\r'], " ");
                div()
                    .id(ElementId::Name(format!("prompt-{name}").into()))
                    .flex()
                    .items_start()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.55))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if picked {
                        rgb(art::ACCENT)
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(picked, |el| el.bg(rgb(art::ACCENT).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::ACCENT).opacity(0.07)))
                    .on_click({
                        let name = name.clone();
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.newchat.template =
                                if this.newchat.template.as_deref() == Some(name.as_str()) {
                                    None
                                } else {
                                    Some(name.clone())
                                };
                            cx.notify();
                        })
                    })
                    // 圆框 = 一次只能选一个。形状本身就在说这事。
                    .child(
                        div()
                            .mt(t().d(2.))
                            .size(m.f(1.1, 10., 14.))
                            .rounded_full()
                            .border_1()
                            .border_color(if picked {
                                rgb(art::ACCENT)
                            } else {
                                rgb(art::BORDER)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(m.f(0.5, 5., 7.)).rounded_full().bg(if picked {
                                rgb(art::ACCENT)
                            } else {
                                rgba(t().p.transparent)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .font_family(art::MONO)
                                    .text_size(m.f(1.5, 11., 13.5))
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.35, 10., 12.5))
                                    .text_color(rgb(art::MUTED))
                                    .truncate()
                                    .child(desc),
                            ),
                    )
                    .into_any_element()
            })
            .collect();

        self.sheet_list(
            m,
            rows,
            if b.prompt_list().is_empty() {
                "这个盘上还没有提示词模板（模板在 6ysg01 那块）"
            } else {
                ""
            },
            cx,
        )
    }

    /// 目录弹层：点一层进一层。**这不是勾选，是导航** —— 所以没有方框，
    /// 当前选中的那格用边框表示。
    fn sheet_dir(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let at = b.dir_at.clone().unwrap_or_default();
        let mut rows: Vec<AnyElement> = Vec::new();

        if !at.is_empty() {
            let up = at.rsplit_once('/').map(|(p, _)| p.to_string());
            let up = up.filter(|p| !p.is_empty()).unwrap_or_else(|| "/".into());
            rows.push(
                div()
                    .id(ElementId::Name("dir-up".into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.5))
                    .rounded(t().r(6.))
                    .hover(|s| s.bg(rgb(art::ACCENT).opacity(0.07)))
                    .on_click(cx.listener({
                        let up = up.clone();
                        move |this, _: &ClickEvent, _, cx| {
                            this.board_dir_enter(up.clone(), cx);
                        }
                    }))
                    .child(
                        div()
                            .font_family(art::MONO)
                            .text_size(m.f(1.4, 10., 13.))
                            .text_color(rgb(art::MUTED))
                            .child(".."),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .font_family(art::MONO)
                            .text_size(m.f(1.4, 10., 13.))
                            .text_color(rgb(art::MUTED))
                            .child(up),
                    )
                    .into_any_element(),
            );
        }

        for d in b.dirs.as_ref().map(|x| x.dirs.clone()).unwrap_or_default() {
            let path = d.path.clone();
            let picked_path = path.clone();
            let selected = b.dir.as_deref() == Some(path.as_str());
            rows.push(
                div()
                    .id(ElementId::Name(format!("dir-{}", d.name).into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.5))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if selected {
                        rgb(art::ACCENT)
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(selected, |el| el.bg(rgb(art::ACCENT).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::ACCENT).opacity(0.07)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.board_dir_enter(picked_path.clone(), cx);
                    }))
                    .child(
                        div()
                            .font_family(art::MONO)
                            .text_size(m.f(1.4, 10., 13.))
                            .text_color(rgb(art::MUTED))
                            .child("›"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .truncate()
                            .font_family(art::MONO)
                            .text_size(m.f(1.4, 10., 13.))
                            .child(path.clone()),
                    )
                    .into_any_element(),
            );
        }

        self.sheet_list(
            m,
            rows,
            if b.dirs.as_ref().map(|d| d.dirs.is_empty()).unwrap_or(true) {
                "这层没有子目录"
            } else {
                ""
            },
            cx,
        )
    }

    /// 机器弹层：本机在最上面，`None` 就是它。
    fn sheet_machine(&self, m: &art::M, cx: &mut Context<Self>) -> AnyElement {
        let b = &self.newchat;
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut all: Vec<(Option<String>, String, String)> =
            vec![(None, "本机".into(), "qrd3wh 跑在哪儿就是哪儿".into())];
        for mc in b
            .machines
            .as_ref()
            .map(|x| x.machines.clone())
            .unwrap_or_default()
        {
            let note = mc
                .note
                .clone()
                .or_else(|| mc.addr.clone())
                .unwrap_or_default();
            all.push((Some(mc.name.clone()), mc.name.clone(), note));
        }
        for (value, label, note) in all {
            let picked = b.machine == value;
            rows.push(
                div()
                    .id(ElementId::Name(format!("machine-{}", label.clone()).into()))
                    .flex()
                    .items_center()
                    .gap(m.v(0.9))
                    .w_full()
                    .px(m.v(0.8))
                    .py(m.v(0.55))
                    .rounded(t().r(6.))
                    .border_1()
                    .border_color(if picked {
                        rgb(art::ACCENT)
                    } else {
                        rgba(t().p.transparent)
                    })
                    .when(picked, |el| el.bg(rgb(art::ACCENT).opacity(0.14)))
                    .hover(|s| s.bg(rgb(art::ACCENT).opacity(0.07)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.newchat.machine = value.clone();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .mt(t().d(1.))
                            .size(m.f(1.1, 10., 14.))
                            .rounded_full()
                            .border_1()
                            .border_color(if picked {
                                rgb(art::ACCENT)
                            } else {
                                rgb(art::BORDER)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(m.f(0.5, 5., 7.)).rounded_full().bg(if picked {
                                rgb(art::ACCENT)
                            } else {
                                rgba(t().p.transparent)
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(t().d(0.))
                            .child(
                                div()
                                    .font_family(art::MONO)
                                    .text_size(m.f(1.45, 10., 13.))
                                    .truncate()
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(m.f(1.3, 9.5, 12.))
                                    .text_color(rgb(art::MUTED))
                                    .truncate()
                                    .child(note),
                            ),
                    )
                    .into_any_element(),
            );
        }
        self.sheet_list(m, rows, "", cx)
    }

    /// 弹层的正文：一个能滚的清单。空的时候把「为什么空」说出来，
    /// 别让人对着一个白框猜。
    fn sheet_list(
        &self,
        m: &art::M,
        rows: Vec<AnyElement>,
        empty: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if rows.is_empty() {
            return div()
                .flex_1()
                .min_h(t().d(120.))
                .flex()
                .items_center()
                .justify_center()
                .px(m.v(1.2))
                .py(m.v(2.4))
                .text_size(m.f(1.4, 10., 12.5))
                .text_color(rgb(art::MUTED))
                .child(empty.to_string())
                .into_any_element();
        }
        div()
            // `on_hover` 挂在 `Stateful` 上：先给 id。
            .id(ElementId::Name("board-sheet-viewport".into()))
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(t().d(0.))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.bar_hover(crate::app::Bar::Sheet, *hovered, cx)
            }))
            .child(
                div()
                    .id(ElementId::Name("board-sheet-list".into()))
                    .flex()
                    .flex_col()
                    .gap(t().d(2.))
                    .px(m.v(1.2))
                    .py(m.v(1.))
                    .flex_1()
                    .min_h(t().d(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.newchat.sheet_scroll)
                    .children(rows),
            )
            .child(crate::app::scrollbar::overlay_tone(
                "board-sheet-scrollbar",
                &self.newchat.sheet_scroll,
                self.bar(crate::app::Bar::Sheet).0,
                self.bar(crate::app::Bar::Sheet).1,
                // 盘面永远是深色 art 调色板：滑块得画成半透明白，不然一根黑线看不见。
                crate::app::scrollbar::Tone::OnDark,
            ))
            .into_any_element()
    }
}
/// 弹层里三列（供应商 / 模型 / 思考档位）的列宽和起点。
///
/// 宽度按**弹层自己的宽**算：这里写死过 720，半屏时第三列就直接落在弹层外面。
fn sheet_lanes(sheet_w: f32, pad: f32) -> ([f32; 3], [f32; 3]) {
    let width = (sheet_w - 2. * pad).max(240.);
    let cols = [width * 0.30, width * 0.42, width * 0.28];
    let xs = [0., cols[0], cols[0] + cols[1]];
    (cols, xs)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::sheet_lanes;

    /// 三列必须落在弹层宽度里。写死 720 那版在半屏窗口下第三列直接挂在弹层外。
    #[test]
    fn 弹层三列不超出弹层宽度() {
        for sheet_w in [720., 601., 500., 300.] {
            let (cols, xs) = sheet_lanes(sheet_w, 8.);
            let width = (sheet_w - 16.).max(240.);
            let right = xs[2] + cols[2];
            assert!(
                right <= width + 0.01,
                "{sheet_w} 宽的弹层里第三列右边缘 {right} 超出 {width}"
            );
            assert!(xs[1] >= cols[0] - 0.01, "{sheet_w}: 第二列盖住第一列");
        }
    }
}
