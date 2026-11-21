//! `GET /v1/capabilities` 的强类型形状。
//!
//! 未知字段一律用 `serde_json::Value` / `#[serde(flatten)] extra` 收着原样透传；
//! **不许** `deny_unknown_fields` —— 引擎加字段这边不该编译不过。
//! 这里的规则只有「引擎说了什么」，不判断功能能不能用（那在 `capability.rs`）。

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Capabilities {
    /// 合同主版本。不匹配由 UI 出横幅，不做降级猜测。
    pub contract: u32,
    pub server: Server,
    pub modules: Vec<Module>,
    pub runtime: Runtime,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub name: String,
    pub version: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Module {
    pub name: String,
    pub enabled: bool,
    /// 引擎声明的端点清单。**这里只解析与透传，不拿它做客户端路由自检** ——
    /// 那份比对（拿 `/v1/_routes` 的真快照当夹具）由 Desk #14 接手；
    /// 在此之前别在这里半途加一个「声明了但没有调用」的草率判定。
    #[serde(default)]
    pub endpoints: Vec<String>,
    #[serde(default)]
    pub reason: Option<Reason>,
    /// Nous#88 的可选展示字段。老引擎没有 → `None`，UI 降级到 server.version /「—」，
    /// 不阻塞协议层解析，也不把这些字段当功能判定。
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(rename = "crate", default)]
    pub crate_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `reason.code` 四值封闭；未知值不解析成第五个变体 —— 老客户端不该因为服务端加了
/// 一个 code 就崩。未知值按「不可用 + 显示 message」处理，分支只在 `capability.rs` 里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    NotImplemented,
    DisabledByManifest,
    Frozen,
    NeedsCredentials,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Reason {
    pub code: ReasonCode,
    /// 给人看的文案。**只显示不解析** —— 改文案不许改行为。
    pub message: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Runtime {
    #[serde(default)]
    pub provider: Provider,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Provider {
    /// `false` 时输入**不灰**，只在发送前提示「没配凭证会 503」。
    #[serde(default)]
    pub configured: bool,
    /// `faux` = 演示模式（标题栏角标）。缺字段按 `real` 处理 —— 老引擎不能因此挂角标。
    #[serde(default = "default_provider_kind")]
    pub kind: String,
    #[serde(default)]
    pub message: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_provider_kind() -> String {
    "real".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_capabilities_shape_and_keeps_unknown_fields() {
        let body = serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.0", "build": "abc" },
            "modules": [
                { "name": "sessions", "enabled": true,
                  "endpoints": ["GET /v1/sessions"], "newField": 7 },
                { "name": "presets", "enabled": false,
                  "reason": { "code": "not_implemented", "message": "还没做", "hint": "x" } }
            ],
            "runtime": { "provider": {
                "configured": false, "kind": "faux", "message": "假回复", "newKey": true
            }, "future": 1 },
            "services": [{ "name": "engine", "base": "http://127.0.0.1:1" }]
        });
        let caps: Capabilities = serde_json::from_value(body).unwrap();
        assert_eq!(caps.contract, 1);
        assert_eq!(caps.server.version, "0.1.0");
        assert_eq!(caps.server.extra["build"], "abc");
        assert_eq!(caps.modules[0].endpoints, vec!["GET /v1/sessions"]);
        assert_eq!(caps.modules[0].extra["newField"], 7);
        assert_eq!(caps.modules[1].reason.as_ref().unwrap().code, ReasonCode::NotImplemented);
        assert_eq!(caps.modules[1].reason.as_ref().unwrap().extra["hint"], "x");
        assert_eq!(caps.runtime.provider.kind, "faux");
        assert!(!caps.runtime.provider.configured);
        assert_eq!(caps.runtime.provider.extra["newKey"], true);
        assert_eq!(caps.runtime.extra["future"], 1);
        assert_eq!(caps.extra["services"][0]["name"], "engine");
    }

    /// #18 / Nous#88：`modules[]` 的六个展示字段是可选的。
    /// 老引擎没有这些字段时不许反序列化失败，要能一路走到 UI 的降级显示。
    #[test]
    fn module_optional_display_fields_default_when_absent() {
        let caps: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [
                { "name": "old", "enabled": true, "endpoints": [] },
                { "name": "new", "enabled": true, "endpoints": [],
                  "version": "0.2.0", "kind": "编译期", "layer": "app",
                  "required": true, "crate": "nous-new", "description": "新模块" }
            ],
            "runtime": { "provider": { "configured": false } }
        }))
        .unwrap();
        let old = &caps.modules[0];
        assert!(old.version.is_none() && old.kind.is_none() && old.layer.is_none());
        assert!(old.required.is_none() && old.crate_name.is_none() && old.description.is_none());
        let new = &caps.modules[1];
        assert_eq!(new.version.as_deref(), Some("0.2.0"));
        assert_eq!(new.kind.as_deref(), Some("编译期"));
        assert_eq!(new.layer.as_deref(), Some("app"));
        assert_eq!(new.required, Some(true));
        assert_eq!(new.crate_name.as_deref(), Some("nous-new"));
        assert_eq!(new.description.as_deref(), Some("新模块"));
    }

    #[test]
    fn missing_provider_kind_defaults_to_real() {
        let caps: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.0" },
            "modules": [],
            "runtime": { "provider": { "configured": false, "message": "没凭证" } }
        }))
        .unwrap();
        assert_eq!(caps.runtime.provider.kind, "real");
    }
}
