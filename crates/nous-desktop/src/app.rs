//! 应用主体。第一版：连上引擎，把会话列表画进左栏。

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use nous_engine::resolve::resolve_engine;
use nous_engine::types::Session;
use nous_engine::EngineClient;

use crate::config::Config;

pub struct Desktop {
    engine: Option<Arc<EngineClient>>,
    sessions: Vec<Session>,
    error: Option<String>,
    loading: bool,
}

impl Desktop {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            engine: None,
            sessions: vec![],
            error: None,
            loading: true,
        };
        this.bootstrap(cx);
        this
    }

    fn bootstrap(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let handle = crate::TOKIO.get().cloned();
            let outcome = match handle {
                Some(h) => match h.spawn(load_engine()).await {
                    Ok(Ok(v)) => Ok(v),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(format!("任务挂了：{e}")),
                },
                None => Err("tokio 没起".to_string()),
            };

            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match outcome {
                    Ok((engine, sessions)) => {
                        this.engine = Some(engine);
                        this.sessions = sessions;
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<AnyElement> = self
            .sessions
            .iter()
            .map(|s| self.session_row(s, cx).into_any_element())
            .collect();
        v_flex()
            .w(px(280.))
            .h_full()
            .bg(cx.theme().background)
            .border_r_1()
            .child(div().p_3().child("Nous").child(div().text_xs().child("桌面端")))
            .child(div().px_3().child(self.status_line(cx)))
            .child(
                v_flex()
                    .flex_1()
                    .overflow_hidden()
                    .children(rows),
            )
    }

    fn status_line(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return div().text_sm().child("连引擎中…");
        }
        if let Some(e) = &self.error {
            return div().text_sm().text_color(cx.theme().danger).child(format!("连不上：{e}"));
        }
        div().text_sm().child(format!("{} 个会话", self.sessions.len()))
    }

    fn session_row(&self, s: &Session, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = s
            .last
            .as_ref()
            .and_then(|l| l.text.clone())
            .unwrap_or_default();
        v_flex()
            .p_2()
            .gap_0p5()
            .border_b_1()
            .child(div().child(s.name.clone()))
            .when(!preview.is_empty(), |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(preview),
                )
            })
    }
}

/// 连引擎：读配置 → resolve 地址 → 拉会话列表。全在 tokio 上。
async fn load_engine() -> Result<(Arc<EngineClient>, Vec<Session>), String> {
    let cfg = Config::load()?;
    let url = resolve_engine(&cfg.host, &cfg.board, "h58ga5", "qrd3wh")
        .await
        .map_err(|e| e.to_string())?;
    let engine = EngineClient::new(url.as_str(), Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let sessions = engine
        .sessions()
        .await
        .map_err(|e| e.to_string())?
        .items;
    Ok((Arc::new(engine), sessions))
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().size_full().bg(cx.theme().background).child(
            self.render_sidebar(cx),
        )
        .child(
            v_flex()
                .flex_1()
                .size_full()
                .items_center()
                .justify_center()
                .child("选中会话后这里显示聊天"),
        )
    }
}
