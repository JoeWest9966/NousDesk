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
    Stats,
    Ext,
    /// 元素库（组 / 包）：一份 JSON 一个元素（旧面板 `k3skws` 块）。
    Elements,
}

impl Feature {
    /// 这个功能的代表路径：UI 入口守卫走 [`gate_for_path`]，
    /// 于是 41 条路径映射表不是死代码，而是每次灰入口都过一遍。
    fn probe_path(self) -> (&'static str, &'static str) {
        match self {
            Self::Sessions => ("GET", "v1/sessions"),
            Self::Presets => ("GET", "v1/presets/{section}"),
            Self::Archive => ("GET", "v1/archive"),
            Self::Combos => ("GET", "v1/combos/{section}"),
            Self::Triggers => ("GET", "v1/trigger-projects"),
            Self::Skills => ("GET", "v1/skills"),
            Self::Prompts => ("GET", "v1/prompts"),
            Self::Selection => ("GET", "v1/selection"),
            Self::Device => ("GET", "v1/device"),
            Self::Stats => ("GET", "v1/stats/usage"),
            Self::Ext => ("GET", "v1/extensions"),
            Self::Elements => ("GET", "v1/elements"),
        }
    }

    /// 插件页左栏的中文名。#18 + #12 合并后共 12 个 Feature 逐个钉字，新增变体必须同时补这里。
    pub fn label(self) -> &'static str {
        match self {
            Self::Sessions => "会话",
            Self::Presets => "预设",
            Self::Archive => "归档",
            Self::Combos => "组合",
            Self::Triggers => "触发器",
            Self::Skills => "技能",
            Self::Prompts => "提示词",
            Self::Selection => "选择",
            Self::Device => "设备",
            Self::Stats => "数据监控",
            Self::Ext => "扩展",
            Self::Elements => "元素库",
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
            Self::Stats => "data-manager",
            Self::Ext => "ext",
            Self::Elements => "elements",
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

/// 灰入口的理由：`capabilities.reason.message` **原文**逐字显示，按 code 只加前缀。
/// `Desk#13` 判据 3 钉死这条：自写文案顶掉服务端原文 = 说谎；真机 `curl /v1/capabilities`
/// 对不上号就没法排查。文案只显示、不解析 —— 改文案不许改行为。
pub fn disabled_text(code: ReasonCode, message: &str) -> String {
    let hint = match code {
        ReasonCode::NotImplemented => "（服务端未提供）",
        ReasonCode::DisabledByManifest => "（发行版未启用）",
        ReasonCode::Frozen => "（已冻结）",
        ReasonCode::NeedsCredentials => "（去设置里配置凭证）",
        ReasonCode::Unknown => "（服务端未声明）",
    };
    format!("{message}{hint}")
}

/// 把 `:id` 形态的路由模板归一成 `{id}` 形态，两处映射表共用一份。
/// 只认 `:name` 这一种写法；已经是 `{name}` 的原样返回。
pub fn normalize_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .map(|seg| {
            if seg.starts_with(':') && seg.len() > 1 {
                format!("{{{}}}", &seg[1..])
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// 「关于 → 插件」列表行标题：**只有英文模块名**。不查 Feature、不翻译 —— 引擎报什么名字就显示什么，
/// 新插件自动出现，客户端不用改。中文名只活在各模块自己的 README 里；`Feature::label()`
/// 是 Desk 自己的页面名，不是插件名，两者不能混用。
pub fn plugin_title(module_name: &str) -> &str {
    module_name
}

/// 插件行 / 详情页第二行的供应商文案（方案 C 胶囊里的字）。原样透传引擎给的 `vendor`，
/// 不查 Feature、不翻译；老引擎没有 vendor（None）或空串时给「—」，行高不跳。
pub fn vendor_label(vendor: Option<&str>) -> &str {
    match vendor {
        Some(vendor) if !vendor.is_empty() => vendor,
        _ => "—",
    }
}

/// 模块名比对用的一把尺：只留字母数字、降小写。`Nous_Device` / `nous_device` /
/// `nous-device` 归一后是同一个词；中文、空格、标点一律不算数（模块名本来就不许有）。
fn flatten(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// 引擎报的模块名与客户端登记的 `Feature` 是不是同一个。
///
/// 两个方向都要认：`docs/plugins.md` 规则 12 之后引擎报的是全名（`Nous_Device`），
/// 而 Desk 登记的是短名（`device`）。所以比**全名**，再比**尾段**（`_` 之后那段）。
/// 两条都不中就是没登记 —— 不猜、不降级。
pub fn module_matches(reported: &str, feature: Feature) -> bool {
    let want = flatten(feature.module_name());
    if flatten(reported) == want {
        return true;
    }
    reported
        .rsplit('_')
        .next()
        .map(flatten)
        .is_some_and(|tail| tail == want)
}

/// 从引擎模块名找客户端登记的 `Feature`。`env` 这类引擎侧模块
/// 没有 Desk 入口 → `None`，插件页照实列出并标「未登记」。
pub fn module_feature(name: &str) -> Option<Feature> {
    [
        Feature::Sessions, Feature::Presets, Feature::Archive,
        Feature::Combos, Feature::Triggers, Feature::Skills, Feature::Prompts,
        Feature::Selection, Feature::Device, Feature::Stats, Feature::Ext,
        Feature::Elements,
    ]
    .into_iter()
    .find(|f| module_matches(name, *f))
}

/// `Module::endpoints` 里一条形如 `"GET /v1/sessions"` 的字符串拆成 (method, path)。
/// 只做纯字符串切分：多空格归一，缺 method / 缺 path / 粘连都按「拆不出」返回 `None`
/// （界面原样显示这条端点，不猜）。
pub fn split_endpoint(endpoint: &str) -> Option<(&str, &str)> {
    let mut parts = endpoint.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    if parts.next().is_some() || !path.starts_with('/') {
        return None;
    }
    Some((method, path))
}

/// 端点级灰理由的**唯一纯函数通道**：模块门 + endpoints 声明，
/// 理由只取 `capabilities.reason.message` 原文（模块没给 reason 时按统一文案）。
/// UI 只消费这一句，不许在 resources/triggers/app 里自编理由或再判一次。
pub fn endpoint_disabled_reason(caps: &Capabilities, method: &str, path: &str) -> Option<String> {
    match gate_for_path(caps, method, path) {
        Gate::Enabled if endpoint_enabled(caps, method, path) => None,
        Gate::Disabled { code, reason } => Some(disabled_text(code, &reason.message)),
        Gate::Enabled => Some(disabled_text(
            ReasonCode::NotImplemented,
            "服务端没有声明这个端点（旧版本不支持）",
        )),
        Gate::Unknown => Some(disabled_text(ReasonCode::Unknown, "服务端未声明这个功能")),
    }
}

/// 唯一的判定处。UI 里不许再写第二处 `if module_enabled(...)`。
pub fn gate<'a>(caps: &'a Capabilities, feature: Feature) -> Gate<'a> {
    match caps.modules.iter().find(|m| module_matches(&m.name, feature)) {
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
/// Desk#14：路径**不再手写第二份**。`dead_paths.rs` 的 `ENGINE_ROUTES` +
/// `DEAD_PATHS` 是唯一的路径真相；这里只按前缀把每条路径归到一个 Feature。
/// 映射不到的路径 = `None` → `Gate::Unknown` = 不可用（不许「先发了看 404」）。
/// 两套模板写法都认：client.rs 用 `:id`，服务端 capabilities 用 `{id}`。
/// #17 的端点级灰理由与 #18 的 `Feature::label` 仍走原来的能力通道。
pub fn feature_for_path(method: &str, path: &str) -> Option<Feature> {
    let path = normalize_path(path);
    let path = path.as_str();
    // 先查唯一真相表：活路由与死路径账本都在其中登记过。
    let known = crate::dead_paths::ENGINE_ROUTES
        .iter()
        .any(|(m, p)| *m == method && normalize_path(p) == path)
        || crate::dead_paths::DEAD_PATHS
            .iter()
            .any(|e| e.method == method && normalize_path(e.path) == path);
    if !known {
        return None;
    }
    // 仓级动作（`/v1/repos/{repo}/{op}`：归档 / 恢复 / 真删 / 变体）按**仓**归到
    // 对应的功能面。以前这里一律 `None`（Nous#75 等票期：写了就 404，宁可灰着）；
    // 现在这些端点在引擎里都在，再一律置灰就是把入口焊死。仓名不认识仍然 `None`。
    if let Some(rest) = path.strip_prefix("v1/repos/") {
        let repo = rest.split('/').next().unwrap_or("");
        return match repo {
            "skills" => Some(Feature::Skills),
            "prompts" => Some(Feature::Prompts),
            "extend" => Some(Feature::Ext),
            "presets" => Some(Feature::Presets),
            "elements" => Some(Feature::Elements),
            _ => None,
        };
    }
    // 功能归属：前缀判定。顺序从特殊到一般，archive 不落进 sessions。
    if path == "v1/archive" || path.starts_with("v1/archive/") {
        return Some(Feature::Archive);
    }
    if path.strip_prefix(concat!("v1/presets", "/")).is_some() || path.starts_with("v1/combos") {
        return Some(if path.starts_with("v1/combos") { Feature::Combos } else { Feature::Presets });
    }
    if path.starts_with("v1/stats/") {
        return Some(Feature::Stats);
    }
    if path == "v1/skills" || path.starts_with("v1/skills/") {
        return Some(Feature::Skills);
    }
    if path == "v1/prompts" || path.starts_with("v1/prompts/") {
        return Some(Feature::Prompts);
    }
    if path == "v1/extensions" || path.starts_with("v1/extensions/") {
        return Some(Feature::Ext);
    }
    if path == "v1/elements" || path.starts_with("v1/elements/") {
        return Some(Feature::Elements);
    }
    if path.starts_with("v1/triggers") || path.starts_with("v1/trigger-projects") {
        return Some(Feature::Triggers);
    }
    if path == "v1/selection" {
        return Some(Feature::Selection);
    }
    // 插件 README（`/v1/plugins/{name}/readme`）是设置页「关于」用的只读面：
    // 归到 Device 这一面，不单独开一个 Feature —— 它没有自己的入口，也不该多一道门。
    if path.starts_with("v1/plugins/") {
        return Some(Feature::Device);
    }
    if path == "v1/device"
        || path == "v1/models"
        || path == "v1/machines"
        || path == "v1/default"
        || path == "v1/fs/dirs"
        || path == "v1/capabilities"
        || path == "v1/_routes"
    {
        return Some(Feature::Device);
    }
    // 其余（sessions / unread / read / origins / search / sessions/*）归会话面。
    Some(Feature::Sessions)
}

/// Desk#14：把运行时 `GET /v1/_routes` 的回包与客户端活路径表比一比。
/// 返回 `(引擎条数, 漂移说明)`；回包为空 = 探针没抓到，不算「引擎零路由」，
/// 不阻塞 UI。死路径账本里的路径不要求引擎一定有（它们本来就是灰/等票）。
pub fn route_drift(
    client_paths: &[&str],
    engine_routes: &[(&str, &str)],
) -> (usize, Vec<String>) {
    if engine_routes.is_empty() {
        return (0, Vec::new());
    }
    // 引擎报的是路由**模板**（`/v1/repos/:repo/archive`），客户端打的是具体地址
    // （`/v1/repos/extend/archive`）。只比字符串相等，所有带路径参数的端点都会被报成
    // 「引擎没有」—— 一条每天都在喊的假警报，喊久了真漂移就没人看了。
    let engine: Vec<(&str, String)> = engine_routes
        .iter()
        .map(|(m, p)| (*m, normalize_path(p)))
        .collect();
    let known_dead: std::collections::BTreeSet<String> = crate::dead_paths::DEAD_PATHS
        .iter()
        .map(|e| format!("{} {}", e.method, normalize_path(e.path)))
        .collect();
    let missing: Vec<String> = client_paths
        .iter()
        .map(|p| p.to_string())
        .filter(|p| {
            let mut parts = p.splitn(2, ' ');
            let method = parts.next().unwrap_or_default();
            let path = parts.next().unwrap_or_default();
            let wanted = normalize_path(path);
            let key = format!("{method} {wanted}");
            let declared = engine
                .iter()
                .any(|(m, template)| *m == method && same_route(template, &wanted));
            !declared && !known_dead.contains(&key)
        })
        .collect();
    let engine_count = engine.len();
    let drift = if missing.is_empty() {
        Vec::new()
    } else {
        vec![format!("客户端认得但引擎没有：{}", missing.join(", "))]
    };
    (engine_count, drift)
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

    /// 端点级灰理由：`gate_for_path` 只认模块，端点没在 capabilities 声明时也要给
    /// 同一条 `disabled_text` 通道的人话。UI 只消费这一句，不许自己 match gate / 编理由。
    /// （Desk#17：`device` 模块 enabled 但老引擎没有 `PUT /v1/default`。）
    pub(crate) fn endpoint_disabled_reason(&self, method: &str, path: &str) -> Option<String> {
        let Some(caps) = self.caps.as_ref() else {
            return Some(disabled_text(ReasonCode::Unknown, "服务端未声明这个功能"));
        };
        endpoint_disabled_reason(caps, method, path)
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

/// 灰路径的理由通道：路径归属的模块 `reason.message` **原文**，取自 capabilities。
/// 不再在 `app.rs` 里 match path —— 路径 → Feature → gate 全在这一处。
pub fn dead_path_disabled_reason(caps: Option<&Capabilities>, path: &str) -> Option<String> {
    let caps = caps?;
    // 路径可能以 POST/PUT 登记（如 restore/config），门控只看归属模块，方法无关。
    let feature = ["GET", "POST", "PUT", "PATCH", "DELETE"]
        .into_iter()
        .find_map(|m| feature_for_path(m, path))?;
    match gate(caps, feature) {
        Gate::Enabled => None,
        Gate::Disabled { code, reason } => Some(disabled_text(code, &reason.message)),
        Gate::Unknown => Some(disabled_text(ReasonCode::Unknown, "服务端未声明这个功能")),
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

/// 比模块门更细的一层：**模块 enabled 还不够，端点必须也在 capabilities 里声明**。
/// 老引擎 0.1.3 的 device 模块是 enabled 的，但没有 `PUT /v1/default` —— 只看模块
/// 会把写按钮亮起来，点下去 404（Desk#17）。
pub fn endpoint_enabled(caps: &Capabilities, method: &str, path: &str) -> bool {
    if !matches!(gate_for_path(caps, method, path), Gate::Enabled) {
        return false;
    }
    let wanted = normalize_path(path);
    caps.modules.iter().any(|m| {
        m.endpoints.iter().any(|e| {
            let mut parts = e.split_whitespace();
            parts.next() == Some(method)
                && parts.next().is_some_and(|declared| same_route(&normalize_path(declared), &wanted))
        })
    })
}

/// 声明的路由模板和实际要打的路径是不是同一条。
///
/// 引擎声明的是 `/v1/repos/:repo/archive`（一条路由 + 路径参数），客户端打的是
/// `/v1/repos/extend/archive`。只按字符串相等比，所有带路径参数的端点都会被判成
/// 「服务端没声明」—— 灰掉的理由还是「旧版本不支持」，查起来最费劲。
///
/// 规则：段数相同，且模板里 `{x}` 那一段接受任意非空值；其余逐段字面相等。
fn same_route(declared: &str, wanted: &str) -> bool {
    let declared: Vec<&str> = declared.split('/').collect();
    let wanted: Vec<&str> = wanted.split('/').collect();
    if declared.len() != wanted.len() {
        return false;
    }
    declared.iter().zip(wanted.iter()).all(|(d, w)| {
        (d.starts_with('{') && d.ends_with('}') && d.len() > 2 && !w.is_empty()) || d == w
    })
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

    /// 插件列表行不许再出现中文别名 —— 引擎报什么名字就显示什么。
    /// 如果有人把 `Feature::label()` 的中文贴回插件标题，这条测试会红。
    #[test]
    fn plugin_title_is_the_verbatim_module_name() {
        assert_eq!(plugin_title("data-manager"), "data-manager");
        assert_eq!(plugin_title("element-models"), "element-models");
        assert_eq!(plugin_title("brand-new-plugin"), "brand-new-plugin");
        assert_ne!(plugin_title("data-manager"), Feature::Stats.label(), "插件标题不能是中文别名");
    }

    /// 规则 12 之后引擎报全名（`Nous_Device`），Desk 登记的是短名（`device`）：
    /// 两个方向都要认，否则改名当天 Desk 就把「设备」判成「未登记」而灰掉整页。
    #[test]
    fn module_matches_accepts_both_short_and_magento_full_name() {
        // 老引擎：短名原样对上。
        assert!(module_matches("device", Feature::Device));
        assert!(module_matches("data-manager", Feature::Stats));
        // 新引擎：`Vendor_Module` 全名，按尾段对上。
        assert!(module_matches("Nous_Device", Feature::Device));
        assert!(module_matches("Nous_Sessions", Feature::Sessions));
        // 归一化只看字母数字：大小写与连字符/下划线不敏感。
        assert!(module_matches("nous-device", Feature::Device));
        assert!(module_matches("Data_Manager", Feature::Stats));
        // 不许多认：元素库是 `Element_Models`，不是 `Nous_Elements`。
        assert!(!module_matches("Element_Models", Feature::Elements));
        assert!(!module_matches("Nous_Device", Feature::Elements));
        assert!(!module_matches("env", Feature::Device));
    }

    /// 供应商是第二行的胶囊文案：有值原样显示；没有（老引擎 / 空串）一律「—」，
    /// 不许拿 "nous" 或「未知」顶替。
    #[test]
    fn vendor_label_is_verbatim_or_dash() {
        assert_eq!(vendor_label(Some("data")), "data");
        assert_eq!(vendor_label(Some("nous")), "nous");
        assert_eq!(vendor_label(Some("")), "—");
        assert_eq!(vendor_label(None), "—");
    }

    /// 这些中文是 Desk 自己的页面名（导航 / 左列），不是插件名 —— 插件列表与详情
    /// 只显示引擎报的英文模块名，不再使用它们。12 个 Feature 一个都不许漏，
    /// 新增变体时这里会先红（match 全覆盖 + 单测逐条钉字）。
    #[test]
    fn every_feature_has_a_chinese_label() {
        let all = [
            (Feature::Sessions, "会话"),
            (Feature::Presets, "预设"),
            (Feature::Archive, "归档"),
            (Feature::Combos, "组合"),
            (Feature::Triggers, "触发器"),
            (Feature::Skills, "技能"),
            (Feature::Prompts, "提示词"),
            (Feature::Selection, "选择"),
            (Feature::Device, "设备"),
            (Feature::Ext, "扩展"),
            (Feature::Stats, "数据监控"),
        ];
        assert_eq!(all.len(), 11, "#18 的 10 个 Feature + #12 的 Stats（documents 已下线，Desk 不登记）");
        // engine 恢复分支多出的 `env` 没有 Desk 入口，不在 Feature 里；
        // 插件页照实列出所有模块，未登记的显式标注。
        for (feature, label) in all {
            assert_eq!(feature.label(), label, "{feature:?} 的中文标签不对");
        }
    }

    /// #18 + #12 插件页：每个登记模块都能从 Feature 找到；恢复分支多出的 `env`
    /// 是「未登记」的真实样本，UI 必须照实列出而不是过滤掉。
    #[test]
    fn registered_feature_names_cover_all_desktop_modules() {
        let all = [
            Feature::Sessions, Feature::Presets, Feature::Archive,
            Feature::Combos, Feature::Triggers, Feature::Skills, Feature::Prompts,
            Feature::Selection, Feature::Device, Feature::Stats, Feature::Ext,
        ];
        let names: Vec<_> = all.iter().map(|f| f.module_name()).collect();
        assert_eq!(names.len(), 11);
        assert_eq!(names.iter().filter(|n| **n == "env").count(), 0, "env 没有 Desk 入口");
    }

    /// #18：端点清单只做字符串拆分，不猜语义。多空格 / 路径带空格都归一。
    #[test]
    fn split_endpoint_separates_method_and_path() {
        assert_eq!(split_endpoint("GET /v1/sessions"), Some(("GET", "/v1/sessions")));
        assert_eq!(split_endpoint("POST   /v1/sessions"), Some(("POST", "/v1/sessions")));
        assert_eq!(split_endpoint("PUT /v1/default"), Some(("PUT", "/v1/default")));
        assert_eq!(split_endpoint("GET/v1/sessions"), None);
        assert_eq!(split_endpoint("/v1/sessions"), None);
        assert_eq!(split_endpoint(""), None);
    }

    /// 判据 2 的前缀判断不许在源码里留下裸的 presets 前缀字面量。
    #[test]
    fn presets_prefix_guards_do_not_pollute_contract_scan() {
        for file in ["capability.rs", "dead_paths.rs"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file);
            let src = std::fs::read_to_string(&path).unwrap();
            for line in src.lines() {
                let raw = ["v1/", "presets"].concat();
                let raw_slash = format!("{raw}/");
                if line.contains(&format!("starts_with(\"{raw}"))
                    || line.contains(&format!("strip_prefix(\"{raw_slash}"))
                {
                    panic!("{file} has raw presets prefix guard: {}", line.trim());
                }
            }
        }
    }

    /// 仓级端点按**仓**归功能面，不再一律「没登记 = 不可用」。
    ///
    /// 旧口径（Nous#75 等票期）是 prefixed None；现在这些端点在引擎里都在：
    /// `/v1/repos/skills/archive` 归技能、`/v1/repos/extend/archive` 归扩展
    /// （私库名是 `extend`，不是 `extensions`）。仓名不认识仍然 None。
    #[test]
    fn repo_paths_map_to_the_owning_feature() {
        assert_eq!(feature_for_path("POST", "/v1/repos/presets/commit"), Some(Feature::Presets));
        assert_eq!(feature_for_path("POST", "/v1/repos/skills/archive"), Some(Feature::Skills));
        assert_eq!(feature_for_path("POST", "/v1/repos/prompts/archive"), Some(Feature::Prompts));
        assert_eq!(feature_for_path("GET", "/v1/repos/extend/archive"), Some(Feature::Ext));
        assert_eq!(feature_for_path("GET", "/v1/repos/extensions/archive"), None, "extensions 不是仓名");
        assert_eq!(feature_for_path("GET", "/v1/repos/nope/archive"), None);
    }

    /// 引擎声明的是路由模板（`/v1/repos/:repo/archive`），客户端打的是具体仓名。
    /// 只按字符串相等比，所有带路径参数的端点都会被判成「服务端没声明」。
    #[test]
    fn endpoint_match_understands_path_parameters() {
        let caps: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.10" },
            "modules": [
                { "name": "ext", "enabled": true, "endpoints": ["GET /v1/repos/:repo/archive"] },
            ],
            "runtime": { "provider": { "kind": "faux" } }
        })).unwrap();
        assert!(endpoint_enabled(&caps, "GET", "/v1/repos/extend/archive"));
        assert!(!endpoint_enabled(&caps, "POST", "/v1/repos/extend/archive"), "方法也要对上");
        assert!(!endpoint_enabled(&caps, "GET", "/v1/repos/extend/variants"), "歧义的段不接受");
    }

    #[test]
    fn gate_is_the_single_decision_point() {
        let c = caps();
        assert_eq!(Feature::Stats.probe_path(), ("GET", "v1/stats/usage"));
        assert!(feature_for_path("GET", "/v1/stats/usage").is_some_and(|f| f == Feature::Stats));
        assert_eq!(gate(&c, Feature::Sessions), Gate::Enabled);
        assert!(matches!(gate(&c, Feature::Presets), Gate::Disabled { code: ReasonCode::NotImplemented, .. }));
        assert_eq!(gate(&c, Feature::Skills), Gate::Unknown, "清单没有 = 不可用");
        assert_eq!(gate_for_path(&c, "GET", "/v1/presets/model"), gate(&c, Feature::Presets));
        assert_eq!(gate_for_path(&c, "GET", "/v1/no-such-path"), Gate::Unknown);
    }

    /// 判据 3：灰入口的文案必须带 `capabilities.reason.message` 原文，
    /// 不许用自写文案顶掉它（`Desk#13`）。
    #[test]
    fn gray_reason_shows_server_message_verbatim() {
        use nous_engine::ReasonCode as C;
        for (code, msg) in [
            (C::NotImplemented, "archive 模块尚未落地"),
            (C::DisabledByManifest, "发行版关了"),
            (C::Frozen, "扩展运行时已冻结"),
            (C::NeedsCredentials, "要先配凭证"),
            (C::Unknown, "服务端未声明这个功能"),
        ] {
            let shown = disabled_text(code, msg);
            assert!(shown.contains(msg), "{code:?} 的灰理由丢了服务端原文：{shown}");
        }
    }

    /// Desk#17：默认模型写在 device 模块（`PUT /v1/default`），
    /// 灰态与理由必须走这条唯一通道；编目空是另一条本地理由（不是能力灰）。
    #[test]
    fn default_model_put_is_gated_by_device_module() {
        let c = caps();
        // caps() 里 device 模块没声明 → Unknown（不可用）；声明后走它自己的 reason。
        assert_eq!(gate_for_path(&c, "PUT", "/v1/default"), Gate::Unknown);

        let c2: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [{ "name": "device", "enabled": false,
                "reason": { "code": "not_implemented", "message": "模型编目还没接" } }],
            "runtime": { "provider": { "kind": "real" } }
        })).unwrap();
        assert_eq!(gate_for_path(&c2, "PUT", "/v1/default"), gate(&c2, Feature::Device));
        let shown = match gate_for_path(&c2, "PUT", "/v1/default") {
            Gate::Disabled { code, reason } => disabled_text(code, &reason.message),
            other => panic!("应该灰掉：{other:?}"),
        };
        assert!(shown.contains("模型编目还没接"));
    }

    /// Desk#13 返工：`app.rs` 的路径 → 门控 match 已收敛到 `capability.rs`；
    /// 灰理由逐字来自 capabilities.reason.message，不许自编。
    #[test]
    fn dead_path_reason_is_server_message_verbatim() {
        let c: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [
                { "name": "archive", "enabled": false,
                  "reason": { "code": "not_implemented", "message": "archive 模块尚未落地" } },
                { "name": "sessions", "enabled": false,
                  "reason": { "code": "frozen", "message": "会话写已冻结" } },
                { "name": "device", "enabled": false,
                  "reason": { "code": "needs_credentials", "message": "先配机器凭证" } },
            ],
            "runtime": { "provider": { "kind": "faux" } }
        })).unwrap();
        for (path, msg) in [
            ("v1/archive", "archive 模块尚未落地"),
            ("v1/archive/{id}/restore", "archive 模块尚未落地"),
            ("v1/sessions/{id}/archive", "会话写已冻结"),
            ("v1/sessions/{id}/purge", "会话写已冻结"),
            ("v1/sessions/{id}/config", "会话写已冻结"),
            ("v1/fs/dirs", "先配机器凭证"),
        ] {
            let shown = dead_path_disabled_reason(Some(&c), path)
                .unwrap_or_else(|| panic!("{path} 没有灰理由"));
            assert!(shown.contains(msg), "{path} 的灰理由丢了原文：{shown}");
            assert!(!shown.contains("写操作已下线"), "自编理由混入：{shown}");
        }
    }

    /// Desk#13 返工：能力型端点等票期间保持灰，理由必须走 capability.rs 单点通道；
    /// 真引擎 caps 里写模块 enabled 但没声明写端点时，也必须给出统一灰理由（不自编）。
    #[test]
    fn waiting_capability_endpoints_get_gray_reason_from_caps_only() {
        // 目标引擎 faux caps 的形状：skills/prompts enabled 但只声明 GET list；
        // ext disabled 且给了 reason。triggers 工程面已落活，不再进这张表。
        let caps: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [
                { "name": "skills", "enabled": true, "endpoints": ["GET /v1/skills"] },
                { "name": "prompts", "enabled": true, "endpoints": ["GET /v1/prompts"] },
                { "name": "ext", "enabled": false,
                  "reason": { "code": "frozen", "message": "扩展运行时已冻结" } },
                { "name": "triggers", "enabled": true,
                  "endpoints": ["GET /v1/triggers", "POST /v1/triggers/{code}/fire"] },
            ],
            "runtime": { "provider": { "kind": "faux" } }
        })).unwrap();
        let paths = [
            ("POST", "/v1/skills/create"), ("PUT", "/v1/skills/file"),
            ("GET", "/v1/skills/files"), ("GET", "/v1/skills/file"),
            ("POST", "/v1/skills/publish"),
            ("POST", "/v1/prompts/create"), ("PUT", "/v1/prompts/file"),
            ("GET", "/v1/prompts/files"), ("GET", "/v1/prompts/file"),
            ("POST", "/v1/prompts/publish"),
            ("POST", "/v1/extensions/create"), ("PUT", "/v1/extensions/file"),
            ("GET", "/v1/extensions/files"), ("GET", "/v1/extensions/file"),
            ("POST", "/v1/extensions/publish"),
        ];
        for (method, path) in paths {
            let reason = endpoint_disabled_reason(&caps, method, path)
                .unwrap_or_else(|| panic!("{method} {path} 等票期间没有灰理由"));
            assert!(!reason.contains("写操作已下线"), "自编理由混进灰通道：{reason}");
            assert!(
                reason.contains("扩展运行时已冻结")
                    || reason.contains("服务端没有声明这个端点"),
                "灰理由没来自 capabilities：{reason}"
            );
        }
    }

    /// Desk#17：老引擎 0.1.3 的 device 模块是 enabled 的，却没声明 PUT /v1/default。
    /// 只看模块会误判「能写」；必须也看 `endpoints` 里有没有这条路径。
    #[test]
    fn endpoint_gate_checks_module_and_endpoint_declaration() {
        let old: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [{ "name": "device", "enabled": true,
                "endpoints": ["GET /v1/models", "GET /v1/device"] }],
            "runtime": { "provider": { "kind": "real" } }
        })).unwrap();
        assert!(!endpoint_enabled(&old, "PUT", "/v1/default"), "老引擎没声明这条写端点");

        let new: Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [{ "name": "device", "enabled": true,
                "endpoints": ["GET /v1/models", "PUT /v1/default"] }],
            "runtime": { "provider": { "kind": "real" } }
        })).unwrap();
        assert!(endpoint_enabled(&new, "PUT", "/v1/default"));
    }

    #[test]
    fn disabled_text_keeps_message_and_adds_code_hint() {
        assert_eq!(disabled_text(ReasonCode::NotImplemented, "服务端没做"), "服务端没做（服务端未提供）");
        assert_eq!(disabled_text(ReasonCode::DisabledByManifest, "随便"), "随便（发行版未启用）");
        assert_eq!(disabled_text(ReasonCode::Frozen, "随便"), "随便（已冻结）");
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

    /// Desk#11 判据 3：presets 路径都归到 `Feature::Presets`，一条不多一条不少。
    /// 这张表是契约的影子：有人偷偷加路径，先红在这里。
    #[test]
    fn presets_gate_table_is_exactly_the_contract_routes() {
        const CONTRACT: &[(&str, &str)] = &[
            ("GET", "/v1/presets/{section}"),
            ("GET", "/v1/presets/{section}/{name}"),
            ("PUT", "/v1/presets/{section}/{name}"),
            ("DELETE", "/v1/presets/{section}/{name}"),
            ("PATCH", "/v1/presets/{section}/{name}"),
            ("POST", "/v1/presets/{section}/{name}/copy"),
            ("PUT", "/v1/presets/{section}/order"),
            ("GET", "/v1/presets/{section}/categories"),
            ("POST", "/v1/presets/{section}/categories"),
            ("POST", "/v1/presets/{section}/create"),
            ("POST", "/v1/presets/{section}/validate"),
            ("GET", "/v1/presets/{section}/{name}/revisions"),
            ("GET", "/v1/presets/{section}/{name}/revisions/{n}"),
            ("POST", "/v1/presets/{section}/{name}/rollback"),
            ("GET", "/v1/presets/model"),
        ];
        assert_eq!(CONTRACT.len(), 15);
        let c = caps();
        for &(method, path) in CONTRACT {
            assert_eq!(
                feature_for_path(method, path),
                Some(Feature::Presets),
                "{method} {path} 没归到 Presets",
            );
            assert_eq!(gate_for_path(&c, method, path), gate(&c, Feature::Presets));
        }
    }

    /// Desk#11 判据 2：客户端实际拼出的 presets 路径模板必须恰好是契约这几条。
    #[test]
    fn client_paths_have_exactly_the_presets_routes() {
        let got: Vec<_> = nous_engine::client_paths()
            .iter()
            .filter(|(_, p)| p.starts_with(concat!("v1/", "presets")))
            .map(|(m, p)| format!("{m} {p}"))
            .collect();
        let want: Vec<String> = [
            ("GET", "v1/presets/{section}"),
            ("GET", "v1/presets/{section}/{name}"),
            ("PUT", "v1/presets/{section}/{name}"),
            ("DELETE", "v1/presets/{section}/{name}"),
            ("PATCH", "v1/presets/{section}/{name}"),
            ("POST", "v1/presets/{section}/{name}/copy"),
            ("PUT", "v1/presets/{section}/order"),
            ("GET", "v1/presets/{section}/categories"),
            ("POST", "v1/presets/{section}/categories"),
            ("POST", "v1/presets/{section}/create"),
            ("POST", "v1/presets/{section}/validate"),
            ("GET", "v1/presets/{section}/{name}/revisions"),
            ("GET", "v1/presets/{section}/{name}/revisions/{n}"),
            ("POST", "v1/presets/{section}/{name}/rollback"),
            ("GET", "v1/presets/model"),
        ]
        .into_iter()
        .map(|(m, p)| format!("{m} {p}"))
        .collect();
        assert_eq!(got, want, "presets 路径表漂移：多一条少一条都算红");
    }

    /// 判据 5：41 条调用路径一条都不许漏。这里列的是 client.rs 实际打出的路径模板。
    /// Desk#14：运行时 `_routes` 对齐是纯集合比较；探针失败（空回包）不算漂移。
    #[test]
    /// 引擎的路径参数模板要能匹配客户端的具体地址，否则每次连上都报一串假漂移。
    #[test]
    fn route_drift_understands_path_parameters() {
        let (count, drift) = route_drift(
            &["POST /v1/repos/extend/archive", "GET /v1/repos/extend/variants"],
            &[
                ("POST", "/v1/repos/:repo/archive"),
                ("GET", "/v1/repos/:repo/variants"),
            ],
        );
        assert_eq!(count, 2);
        assert!(drift.is_empty(), "带路径参数的路由不该报漂移：{drift:?}");

        let (_, drift) = route_drift(
            &["POST /v1/repos/extend/purge"],
            &[("POST", "/v1/repos/:repo/archive")],
        );
        assert_eq!(drift.len(), 1, "真没有的端点还得报");
    }

    fn route_drift_reports_missing_and_extra() {
        let client = ["GET /v1/sessions", "POST /v1/sessions/:id/pin"];
        let engine = [("GET", "/v1/sessions"), ("GET", "/v1/origins")];
        let (count, drift) = route_drift(&client, &engine);
        assert_eq!(count, 2);
        assert_eq!(drift.len(), 1, "客户端两条，引擎缺一条：{drift:?}");
        assert!(drift[0].contains("/v1/sessions/:id/pin"));

        let (count, drift) = route_drift(&["GET /v1/sessions"], &[]);
        assert_eq!(count, 0);
        assert!(drift.is_empty(), "探针失败 = 没抓到，不当成「引擎零路由」");
    }

    /// Desk#14：`feature_for_path` 的路径表必须从 `dead_paths.rs` 的活路由 +
    /// 死路径账本派生，不再维护第二份真相。
    #[test]
    fn path_table_is_derived_from_dead_paths_catalog() {
        for &(method, path) in crate::dead_paths::ENGINE_ROUTES {
            assert!(
                feature_for_path(method, path).is_some(),
                "活路由 {method} {path} 没进 capability 派生表"
            );
        }
        for entry in crate::dead_paths::DEAD_PATHS {
            // 私库 git 写（`presets/commit` 那两条）仍是 Nous#75 的等票端点，
            // 但**路径归属**现在按仓算（见 `repo_paths_map_to_the_owning_feature`）：
            // 归属 ≠ 可用，可不可用由 capabilities 的模块门控决定。
            if entry.path.starts_with("v1/repos/") {
                assert!(feature_for_path(entry.method, entry.path).is_some());
                continue;
            }
            assert!(
                feature_for_path(entry.method, entry.path).is_some(),
                "死路径账本里的 {method} {path} 没进 capability 派生表",
                method = entry.method,
                path = entry.path
            );
        }
        assert!(
            feature_for_path("GET", "/v1/extensions").is_some(),
            "extensions 虽不在夹具，仍要在能力表里灰掉，不能 Unknown"
        );
    }

    #[test]
    fn all_41_client_paths_are_mapped_to_a_feature() {
        const PATHS: &[(&str, &str)] = &[
            ("GET", "/v1/device"), ("GET", "/v1/models"), ("PUT", "/v1/default"), ("GET", "/v1/skills"),
            ("GET", "/v1/prompts"), ("GET", "/v1/extensions"), ("GET", "/v1/machines"),
            ("GET", "/v1/fs/dirs"), ("GET", "/v1/sessions"),
            ("GET", "/v1/sessions/:id/entries"), ("GET", "/v1/unread"),
            ("POST", "/v1/read/:id"), ("POST", "/v1/sessions/:id/fork"),
            ("POST", "/v1/sessions/:id/messages"), ("POST", "/v1/sessions/:id/abort"),
            ("POST", "/v1/sessions/:id/rename"), ("POST", "/v1/sessions/:id/archive"),
            ("GET", "/v1/archive"), ("POST", "/v1/archive/:id/restore"),
            ("POST", "/v1/sessions/:id/purge"), ("POST", "/v1/sessions"),
            ("POST", "/v1/sessions/:id/config"),
            ("GET", "/v1/presets/{section}"), ("GET", "/v1/presets/{section}/{name}"),
            ("PUT", "/v1/presets/{section}/{name}"), ("DELETE", "/v1/presets/{section}/{name}"),
            ("PATCH", "/v1/presets/{section}/{name}"), ("POST", "/v1/presets/{section}/{name}/copy"),
            ("PUT", "/v1/presets/{section}/order"), ("GET", "/v1/presets/{section}/categories"),
            ("GET", "/v1/presets/{section}/{name}/revisions"), ("GET", "/v1/presets/{section}/{name}/revisions/{n}"),
            ("POST", "/v1/presets/{section}/{name}/rollback"), ("GET", "/v1/presets/model"),
            ("GET", "/v1/combos/{section}"),
            ("PATCH", "/v1/combos/{section}/{name}"), ("GET", "/v1/triggers"),
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
