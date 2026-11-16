//! nous-engine — qrd3wh `/v1/*` 的哑客户端。
//!
//! 三层：
//! - [`types`]：引擎回的形状（未知字段透传）。
//! - [`sse`]：SSE 帧解析。
//! - [`client`]：HTTP 转发（qrd3wh：会话/历史/流）。
//! - [`resource`]：三个资源仓（技能 / 提示词 / 扩展）的**文件读写**形状。
//! - [`pins`]：置顶的写穿客户端（bjes6b：`pins.json` 是单源）。
//! - [`console`]：d6s4v1 面板的哑客户端（`/p/<board>/d6s4v1/api/*`，不是引擎）。
//!
//! 不认业务规则，不碰 GPUI。UI 层要 16ms 合帧，那归 UI 层。

pub mod client;
pub mod console;
pub mod pins;
pub mod resolve;
pub mod resource;
pub mod sse;
pub mod triggers;
pub mod types;

pub use client::{EngineClient, EngineError};
pub use console::{ConsoleClient, ConsoleError};
pub use pins::{PinClient, PinError};
pub use resource::{NewExtension, NewPrompt, NewSkill, Repo, ResourceFile, ResourceFileEntry, ResourceFiles, SaveFile};
pub use sse::{Change, SseError, SseEvent};
pub use triggers::TriggerClient;
