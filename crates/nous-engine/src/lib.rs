//! nous-engine — qrd3wh `/v1/*` 的哑客户端。
//!
//! 三层：
//! - [`types`]：引擎回的形状（未知字段透传）。
//! - [`sse`]：SSE 帧解析。
//! - [`client`]：HTTP 转发（qrd3wh：会话/历史/流）。
//! - [`resource`]：三个资源仓（技能 / 提示词 / 扩展）的**文件读写**形状。
//! - [`pins`]：置顶的哑客户端（qrd3wh：Sessions 文档里的提交日志是单源）。
//!
//! token 注入只有一处实现（`auth`）：`EngineClient` 和 `PinClient` 共用，
//! 否则「配置里有 token，置顶却 401」这种事会安静地发生一遍（#10）。
//! - [`console`]：数据监控室六屏的展示类型（旧 `旧面板客户端（已删）` 已在 #13 删除）。
//!
//! 不认业务规则，不碰 GPUI。UI 层要 16ms 合帧，那归 UI 层。

pub mod auth;
pub mod capabilities;
pub mod client;
pub mod console;
pub mod pins;
pub mod resource;
pub mod sse;
pub mod triggers;
pub mod types;

pub use capabilities::{Capabilities, Module, Provider, Reason, ReasonCode, Runtime, Server};
pub use client::{EngineClient, EngineError};
pub use client::{client_paths, CLIENT_PATHS};

pub use pins::{PinClient, PinError};
pub use resource::{NewExtension, NewPrompt, NewSkill, Repo, ResourceFile, ResourceFileEntry, ResourceFiles, SaveFile};
pub use sse::{Change, SseError, SseEvent};
pub use triggers::TriggerClient;
