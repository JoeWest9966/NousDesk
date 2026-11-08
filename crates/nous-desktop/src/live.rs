//! 流式接收：tokio 侧读 SSE 攒进共享状态，UI 侧 16ms 取一次。
//!
//! **一个 token 一次 `cx.notify()` 是这一层最容易犯的错。** 所以分成两半：
//! - `Live::ingest`（tokio 任务里跑）：只改内存，不碰 UI。
//! - `Live::drain`（UI 侧 16ms 定时器里跑）：把攒下的文字取出来写进气泡。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nous_engine::SseEvent;

/// 共享给 tokio 任务和 UI 的那份「正在生成的消息」。
#[derive(Default)]
pub struct LiveState {
    /// contentIndex → 已累积的文字。
    blocks: HashMap<usize, String>,
    /// 有没有新内容，UI 取走之后清掉。
    dirty: bool,
    /// 这一轮生成结束了。
    done: bool,
    /// 出错 / 没答上来，给 UI 显示一句话。
    error: Option<String>,
    /// 工具卡片最近的输出（key = toolCallId）。
    tools: Vec<(String, String)>,
}

#[derive(Clone, Default)]
pub struct Live(Arc<Mutex<LiveState>>);

impl Live {
    pub fn new() -> Self {
        Self::default()
    }

    /// tokio 侧：吃一个 SSE 事件。**不许在这里碰 UI。**
    pub fn ingest(&self, event: &SseEvent) {
        let mut state = self.0.lock().unwrap();
        match event {
            SseEvent::MessageStart { message } => {
                if message.as_ref().and_then(|m| m.role.as_deref()) == Some("assistant") {
                    state.blocks.clear();
                    state.dirty = true;
                }
            }
            SseEvent::MessageUpdate { changes } => {
                for change in changes {
                    match change {
                        nous_engine::Change::TextDelta {
                            content_index,
                            delta,
                        } => {
                            state
                                .blocks
                                .entry(*content_index)
                                .or_default()
                                .push_str(delta);
                            state.dirty = true;
                        }
                        nous_engine::Change::Block {
                            content_index,
                            block,
                        } => {
                            let text = block
                                .as_ref()
                                .and_then(|b| b.get("text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or_default()
                                .to_string();
                            state.blocks.insert(*content_index, text);
                            state.dirty = true;
                        }
                        _ => {}
                    }
                }
            }
            SseEvent::MessageEnd { entry } => {
                if entry.as_ref().map(|e| e.kind.as_str()) == Some("pi.assistant") {
                    state.done = true;
                    state.dirty = true;
                }
            }
            SseEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
            } => {
                let name = tool_name.clone().unwrap_or_else(|| "工具".into());
                state.tools.push((tool_call_id.clone(), name));
                state.dirty = true;
            }
            SseEvent::ToolExecutionUpdate {
                tool_call_id,
                output,
            } => {
                let Some(output) = output else { return };
                let chunk = output
                    .get("append")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        output
                            .get("set")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                if let Some((_, text)) = state.tools.iter_mut().find(|(id, _)| id == tool_call_id) {
                    text.push_str(&chunk);
                    state.dirty = true;
                }
            }
            SseEvent::Submission { record } => {
                if record.as_ref().and_then(|r| r.status.as_deref()) == Some("unanswered") {
                    let why = record
                        .as_ref()
                        .and_then(|r| r.detail.clone().or_else(|| r.reason.clone()))
                        .unwrap_or_else(|| "没答上来".into());
                    state.error = Some(why);
                    state.done = true;
                    state.dirty = true;
                }
            }
            SseEvent::TaskFailed { message } => {
                state.error = Some(message.clone().unwrap_or_else(|| "任务挂了".into()));
                state.done = true;
                state.dirty = true;
            }
            _ => {}
        }
    }

    pub fn fail(&self, message: String) {
        let mut state = self.0.lock().unwrap();
        state.error = Some(message);
        state.done = true;
        state.dirty = true;
    }

    /// UI 侧：有变化就把整份文字取走（按 contentIndex 拼好）。没有就回 None。
    pub fn drain(&self) -> Option<Drained> {
        let mut state = self.0.lock().unwrap();
        if !state.dirty {
            return None;
        }
        state.dirty = false;
        let mut indexes: Vec<usize> = state.blocks.keys().copied().collect();
        indexes.sort_unstable();
        let text = indexes
            .into_iter()
            .filter_map(|i| state.blocks.get(&i).cloned())
            .collect::<Vec<_>>()
            .join("");
        Some(Drained {
            text,
            done: state.done,
            error: state.error.clone(),
            tools: state.tools.clone(),
        })
    }
}

pub struct Drained {
    pub text: String,
    pub done: bool,
    pub error: Option<String>,
    pub tools: Vec<(String, String)>,
}
