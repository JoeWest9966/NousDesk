//! 能力面判定：唯一一处决定「某个用户功能能不能用」。
//!
//! 真相只有 `GET /v1/capabilities` 这一份：模块声明关着 / 没实现 / 冻结，
//! 入口就灰掉。这里不碰 HTTP、不碰 GPUI —— 纯函数 + 一张路径映射表。

use nous_engine::Capabilities;
use nous_engine::types::Device;
use nous_engine::{Module, Reason, ReasonCode};

/// 用户能看见的功能面。名字按 Nous #47 的清单名（注意是 `Ext`，不是 `Extensions`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Sessions,
    Presets,
    Archive,
    Combos,
    Triggers,
    Skills,
    Prompts,
    Selection,
    Device,
    Ext,
}

impl Feature {
    /// 这个功能的代表路径：UI 入口守卫走 [`gate_for_path`]，
    /// 于是 41 条路径映射表不是死代码，而是每次灰入口都过一遍。
    fn probe_path(self) -> (&'static str, &'static str) {
        match self {
            Self::Sessions => ("GET", "v1/sessions"),
            Self::Presets => ("GET", "v1/presets/{section}"),
            Self::Archive => ("GET", "v1/archive"),
            Self::Combos => ("GET", "v1/combos"),
            Self::Triggers => ("GET", "v1/triggers"),
            Self::Skills => ("GET", "v1/skills"),
            Self::Prompts => ("GET", "v1/prompts"),
            Self::Selection => ("GET", "v1/selection"),
            Self::Device => ("GET", "v1/device"),
            Self::Ext => ("GET", "v1/extensions"),
        }
    }

    pub fn module_name(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Presets => "presets",
            Self::Archive => "archive",
            Self::Combos => "combos",
            Self::Triggers => "triggers",
            Self::Skills => "skills",
            Self::Prompts => "prompts",
            Self::Selection => "selection",
            Self::Device => "device",
            Self::Ext => "ext",
        }
    }
}

/// 判定结果。`Unknown` = 清单里没有这个功能 → **不可用**，不做降级猜测。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate<'a> {
    Enabled,
    Disabled { code: ReasonCode, reason: &'a Reason },
    Unknown,
}

/// `Gate::Disabled` 那句话只按 `code` 分支；`reason.message` 只显示、不解析。
pub fn disabled_text(code: ReasonCode, message: &str) -> String {
    match code {
        ReasonCode::NotImplemented => "服务端未提供（版本太旧？）".to_string(),
        ReasonCode::DisabledByManifest => "发行版没装这个模块".to_string(),
        ReasonCode::Frozen => "已冻结".to_string(),
        ReasonCode::NeedsCredentials => format!("{message}（去设置里配置凭证）"),
        ReasonCode::Unknown => format!("{message}（服务端未声明这个功能）"),
    }
}

/// 唯一的判定处。UI 里不许再写第二处 `if module_enabled(...)`。
pub fn gate<'a>(caps: &'a Capabilities, feature: Feature) -> Gate<'a> {
    match caps.modules.iter().find(|m| m.name == feature.module_name()) {
        Some(Module { enabled: true, .. }) => Gate::Enabled,
        Some(Module {
            enabled: false,
            reason: Some(reason),
            ..
        }) => Gate::Disabled {
            code: reason.code,
            reason,
        },
        Some(Module {
            enabled: false,
            reason: None,
            ..
        }) => Gate::Unknown,
        None => Gate::Unknown,
    }
}

/// 把客户端发出的每条 `/v1/*` 路径映射到用户功能面。
///
/// **唯一一张表**：`nous-engine` 里 41 条调用路径逐条落在这里。映射不到的路径
/// = `None` → `Gate::Unknown` = 不可用（不许「先发了看 404」）。
/// 路径用路由模板形态（`:id` / `{kind}` 按 client.rs 实际写法），与服务端
/// capabilities 的 `endpoints` 共用同一套模板标识符。
pub fn feature_for_path(method: &str, path: &str) -> Option<Feature> {
    use Feature::*;
    // 两套模板写法都认：client.rs 用 `:id`，服务端 capabilities 用 `{id}`。
    let normalized = path
        .trim_start_matches('/')
        .split('/')
        .map(|seg| {
            if seg.starts_with(':') && seg.len() > 1 {
                format!("{{{}}}", &seg[1..])
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let path = normalized.as_str();
    match (method, path) {
        // sessions（10）
        ("GET", "v1/sessions")
        | ("POST", "v1/sessions")
        | ("GET", "v1/unread")
        | ("GET", "v1/sessions/{id}/entries")
        | ("POST", "v1/sessions/{id}/messages")
        | ("GET", "v1/sessions/{id}/events")
        | ("POST", "v1/sessions/{id}/abort")
        | ("POST", "v1/sessions/{id}/fork")
        | ("POST", "v1/sessions/{id}/rename")
        | ("POST", "v1/read/{id}")
        // #49 把置顶写穿并从会话模块声明；归档 / 撕原件 / 配置仍走会话页，
        // 服务端虽未声明（灰入口不调用），映射表也要认得它们。
        | ("POST", "v1/sessions/{id}/pin")
        | ("POST", "v1/sessions/{id}/archive")
        | ("POST", "v1/sessions/{id}/purge")
        | ("POST", "v1/sessions/{id}/config") => Some(Sessions),
        // device（4）
        ("GET", "v1/device")
        | ("GET", "v1/models")
        | ("GET", "v1/machines")
        | ("GET", "v1/fs/dirs") => Some(Device),
        // skills / prompts / ext（7 条 × 3）
        ("GET", "v1/skills") | ("POST", "v1/skills/create") | ("PUT", "v1/skills/file")
        | ("POST", "v1/skills/{op}") | ("POST", "v1/skills/publish")
        | ("GET", "v1/skills/files") | ("GET", "v1/skills/file") => Some(Skills),
        ("GET", "v1/prompts") | ("POST", "v1/prompts/create") | ("PUT", "v1/prompts/file")
        | ("POST", "v1/prompts/{op}") | ("POST", "v1/prompts/publish")
        | ("GET", "v1/prompts/files") | ("GET", "v1/prompts/file") => Some(Prompts),
        ("GET", "v1/extensions") | ("POST", "v1/extensions/create") | ("PUT", "v1/extensions/file")
        | ("POST", "v1/extensions/{op}") | ("POST", "v1/extensions/publish")
        | ("GET", "v1/extensions/files") | ("GET", "v1/extensions/file") => Some(Ext),
        // selection（2）
        ("GET", "v1/selection") | ("POST", "v1/selection") => Some(Selection),
        // presets（16）
        (_, p) if p.starts_with("v1/presets/") => Some(Presets),
        // combos（2）
        ("GET", "v1/combos") | ("POST", "v1/combos/{name}/{action}") => Some(Combos),
        // triggers（1）
        ("GET", "v1/triggers") => Some(Triggers),
        // archive（2）
        ("GET", "v1/archive") | ("POST", "v1/archive/{id}/restore") => Some(Archive),
        _ => None,
    }
}

impl crate::app::Desktop {
    /// UI 取判定的**唯一入口**。没有能力表时一律 `Unknown`（= 不可用）。
    pub(crate) fn feature_gate(&self, feature: Feature) -> Option<Gate<'_>> {
        self.caps.as_ref().map(|caps| gate(caps, feature))
    }

    /// 给渲染层的一句话：可用 → `None`；不可用 → 按 `code` 分支的人话。
    /// UI 只拿这一句去灰入口 / 出 tooltip，不许自己再判 `code`。
    pub(crate) fn disabled_reason(&self, feature: Feature) -> Option<String> {
        match self.feature_gate(feature) {
            Some(Gate::Enabled) => None,
            Some(Gate::Disabled { code, reason }) => Some(disabled_text(code, &reason.message)),
            Some(Gate::Unknown) | None => Some(disabled_text(
                ReasonCode::Unknown,
                "服务端未声明这个功能",
            )),
        }
    }

    /// 需要凭证的入口除了灰掉，还要给一个能跳到设置的按钮。
    pub(crate) fn disabled_code(&self, feature: Feature) -> Option<ReasonCode> {
        match self.feature_gate(feature) {
            Some(Gate::Disabled { code, .. }) => Some(code),
            _ => None,
        }
    }

    pub(crate) fn feature_enabled(&self, feature: Feature) -> bool {
        let Some(caps) = self.caps.as_ref() else {
            return false;
        };
        let (method, path) = feature.probe_path();
        matches!(gate_for_path(caps, method, path), Gate::Enabled)
    }

    /// 演示角标只认一个判定（[`is_demo_mode`]），device 在场时以它为准。
    pub(crate) fn demo_mode(&self) -> bool {
        is_demo_mode(self.caps.as_ref(), self.device.as_ref())
    }

    /// 合同主版本不匹配时出横幅，不猜。
    pub(crate) fn contract_mismatch(&self) -> bool {
        self.caps
            .as_ref()
            .is_some_and(|c| contract_mismatch(c, CLIENT_CONTRACT))
    }

    /// 引擎自报的合同主版本（横幅上要显示的那个数）。没连上 → `None`。
    pub(crate) fn engine_contract(&self) -> Option<u32> {
        self.caps.as_ref().map(|c| c.contract)
    }
}

/// 本客户端认的合同主版本。**只有这一处**：横幅上的「客户端 contract N」也从这里取，
/// 别在 UI 里再写一个 1 —— 涨版本时改一处，另一处忘改就是自欺的横幅。
pub const CLIENT_CONTRACT: u32 = 1;

/// 客户端与引擎的主版本不匹配时出横幅，不做降级猜测。
pub fn contract_mismatch(caps: &Capabilities, client_major: u32) -> bool {
    caps.contract != client_major
}

/// 纯函数版角标判定：**一处真相，两种来源**。
///
/// `device.faux` 是引擎直报的「这一份数据是不是假的」（#15 判据 3）：在场时压过
/// `caps.runtime.provider.kind` —— 后者说的是「provider 用没用假实现」，两者冲突时
/// 界面上的 DEMO 角标该跟着数据走。device 拿不到（旧引擎 / 连接失败）才用 caps 兜底。
pub fn is_demo_mode(caps: Option<&Capabilities>, device: Option<&Device>) -> bool {
    if let Some(faux) = device.and_then(|d| d.faux) {
        return faux;
    }
    caps.is_some_and(|c| c.runtime.provider.kind == "faux")
}

/// 调用前唯一守卫：查这张表 → 查 capability。
pub fn gate_for_path<'a>(caps: &'a Capabilities, method: &str, path: &str) -> Gate<'a> {
    match feature_for_path(method, path) {
        Some(feature) => gate(caps, feature),
        None => Gate::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> Capabilities {
        serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.0" },
            "modules": [
                { "name": "sessions", "enabled": true, "endpoints": [] },
                { "name": "presets", "enabled": false,
                  "reason": { "code": "not_implemented", "message": "documents 模块落地前不提供（#48）" } },
                { "name": "archive", "enabled": false,
                  "reason": { "code": "frozen", "message": "已冻结" } },
                { "name": "combos", "enabled": false,
                  "reason": { "code": "disabled_by_manifest", "message": "发行版没装" } },
                { "name": "triggers", "enabled": false,
                  "reason": { "code": "needs_credentials", "message": "要先配凭证" } },
                { "name": "ext", "enabled": false,
                  "reason": { "code": "frozen", "message": "扩展运行时已冻结" } },
            ],
            "runtime": { "provider": { "configured": false, "kind": "real", "message": "没凭证" } }
        }))
        .unwrap()
    }

    #[test]
    fn gate_is_the_single_decision_point() {
        let c = caps();
        assert_eq!(gate(&c, Feature::Sessions), Gate::Enabled);
        assert!(matches!(gate(&c, Feature::Presets), Gate::Disabled { code: ReasonCode::NotImplemented, .. }));
        assert_eq!(gate(&c, Feature::Skills), Gate::Unknown, "清单没有 = 不可用");
        assert_eq!(gate_for_path(&c, "GET", "/v1/presets/model"), gate(&c, Feature::Presets));
        assert_eq!(gate_for_path(&c, "GET", "/v1/no-such-path"), Gate::Unknown);
    }

    #[test]
    fn disabled_text_only_branches_on_code() {
        assert_eq!(disabled_text(ReasonCode::NotImplemented, "随便改的文案"), "服务端未提供（版本太旧？）");
        assert_eq!(disabled_text(ReasonCode::DisabledByManifest, "随便"), "发行版没装这个模块");
        assert_eq!(disabled_text(ReasonCode::Frozen, "随便"), "已冻结");
        assert!(disabled_text(ReasonCode::NeedsCredentials, "要 token").contains("设置"));
    }

    #[test]
    fn demo_badge_only_for_faux() {
        let mut c = caps();
        let real = nous_engine::types::Device {
            self_: None, origin: None, node: None, started_at: None, model: None,
            faux: None, data: None, catalog: None, available: vec![],
            extra: Default::default(),
        };
        assert!(!is_demo_mode(Some(&c), None), "real / 缺省不挂角标");
        assert!(!is_demo_mode(Some(&c), Some(&real)), "device 没说 faux 就不挂");

        c.runtime.provider.kind = "faux".into();
        assert!(is_demo_mode(Some(&c), None), "caps 说 faux，没有 device 时兜底");

        // 两个真相源冲突：device 优先（#15 判据 3）。
        let real_device = nous_engine::types::Device { faux: Some(false), ..real.clone() };
        assert!(!is_demo_mode(Some(&c), Some(&real_device)), "device.faux=false 压过 caps");
        let demo_device = nous_engine::types::Device { faux: Some(true), ..real };
        assert!(is_demo_mode(Some(&caps()), Some(&demo_device)), "device.faux=true 压过 caps real");
    }

    #[test]
    fn contract_major_mismatch_is_not_guessed() {
        let c = caps();
        assert!(!contract_mismatch(&c, 1));
        assert!(contract_mismatch(&c, 2));
    }

    /// 判据 5：41 条调用路径一条都不许漏。这里列的是 client.rs 实际打出的路径模板。
    #[test]
    fn all_41_client_paths_are_mapped_to_a_feature() {
        const PATHS: &[(&str, &str)] = &[
            ("GET", "/v1/device"), ("GET", "/v1/models"), ("GET", "/v1/skills"),
            ("GET", "/v1/prompts"), ("GET", "/v1/extensions"), ("GET", "/v1/machines"),
            ("GET", "/v1/fs/dirs"), ("GET", "/v1/sessions"),
            ("GET", "/v1/sessions/:id/entries"), ("GET", "/v1/unread"),
            ("POST", "/v1/read/:id"), ("POST", "/v1/sessions/:id/fork"),
            ("POST", "/v1/sessions/:id/messages"), ("POST", "/v1/sessions/:id/abort"),
            ("POST", "/v1/sessions/:id/rename"), ("POST", "/v1/sessions/:id/archive"),
            ("GET", "/v1/archive"), ("POST", "/v1/archive/:id/restore"),
            ("POST", "/v1/sessions/:id/purge"), ("POST", "/v1/sessions"),
            ("POST", "/v1/sessions/:id/config"),
            ("GET", "/v1/presets/:section"), ("POST", "/v1/presets/:section/create"),
            ("POST", "/v1/presets/:section/:op"), ("GET", "/v1/presets/:section/files"),
            ("GET", "/v1/presets/:section/file"), ("GET", "/v1/presets/:section/history"),
            ("GET", "/v1/presets/:section/history/diff"), ("GET", "/v1/presets/:section/pending"),
            ("GET", "/v1/presets/:section/pending/diff"), ("PUT", "/v1/presets/:section/file"),
            ("GET", "/v1/presets/:section/validate"), ("POST", "/v1/presets/:section/publish"),
            ("POST", "/v1/presets/:section/reload"), ("POST", "/v1/presets/:section/category"),
            ("POST", "/v1/presets/:section/order"), ("POST", "/v1/presets/:section/rename"),
            ("POST", "/v1/presets/:section/categories"), ("GET", "/v1/combos"),
            ("POST", "/v1/combos/:name/:action"), ("GET", "/v1/triggers"),
            ("GET", "/v1/selection"), ("POST", "/v1/selection"),
            ("GET", "/v1/sessions/:id/events"),
            // 资源仓：resource_root 只给 skills / prompts / extensions 三种字面量。
            ("POST", "/v1/skills/create"), ("PUT", "/v1/skills/file"),
            ("POST", "/v1/skills/{op}"), ("POST", "/v1/skills/publish"),
            ("GET", "/v1/skills/files"), ("GET", "/v1/skills/file"),
            ("POST", "/v1/prompts/create"), ("PUT", "/v1/prompts/file"),
            ("POST", "/v1/prompts/{op}"), ("POST", "/v1/prompts/publish"),
            ("GET", "/v1/prompts/files"), ("GET", "/v1/prompts/file"),
            ("POST", "/v1/extensions/create"), ("PUT", "/v1/extensions/file"),
            ("POST", "/v1/extensions/{op}"), ("POST", "/v1/extensions/publish"),
            ("GET", "/v1/extensions/files"), ("GET", "/v1/extensions/file"),
        ];
        assert!(PATHS.len() >= 41, "清单本身不能缩水");
        for (method, path) in PATHS {
            assert!(feature_for_path(method, path).is_some(), "{method} {path} 没映射");
        }
    }
}
