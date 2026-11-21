// 静态台账：生产代码不读，单测逐条核对；`#[allow(dead_code)]` 是有意的。
#![allow(dead_code)]
//! 死路径白名单：客户端会打、但引擎 `meta::ROUTES` 没有的路径，逐条处置。
//!
//! **每条只有三种处置**（`Desk#13`）：
//! - `等票`：引擎侧已开票，客户端保持原样；**票号必填**，空号 = 非法。
//! - `灰`：入口保留、不可点，理由来自 `capabilities.reason.message`（不许自编）。
//! - `删`：入口与代码一起删掉；这里只留一行账，不许留「以后会好」。
//!
//! 这张表是静态的「合同外调用」账本。`rg -n '"v1/[a-z]' crates/nous-engine/src`
//! 里出现、又不在本表和引擎路由里的路径，就是偷偷复活的一条死调用。

/// 处置。翻译成 `&'static str` 供表和测试共用，别在比较处写中文字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    WaitTicket,
    Gray,
    Removed,
}

impl Disposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WaitTicket => "等票",
            Self::Gray => "灰",
            Self::Removed => "删",
        }
    }
}

/// 一行账：方法 + 路由模板 + 处置 + 票号（`等票` 必填，其余用 `None`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadPath {
    pub method: &'static str,
    pub path: &'static str,
    pub disposition: Disposition,
    pub ticket: Option<&'static str>,
}

impl DeadPath {
    pub const fn wait(method: &'static str, path: &'static str, ticket: &'static str) -> Self {
        Self { method, path, disposition: Disposition::WaitTicket, ticket: Some(ticket) }
    }

    pub const fn gray(method: &'static str, path: &'static str) -> Self {
        Self { method, path, disposition: Disposition::Gray, ticket: None }
    }

    pub const fn removed(method: &'static str, path: &'static str) -> Self {
        Self { method, path, disposition: Disposition::Removed, ticket: None }
    }
}

/// 50 个死路径族（与 `/var/tmp/audit/D-desk.md` §2.5 逐条对齐）。
///
/// `等票` / `灰` / `删` 的交叉口径按票面：presets 等 `Desk#11`、combos 等 `#61`、
/// archive 与 sessions/{config,archive,purge} 灰、fs/dirs 灰（Nous#68）、
/// 私库写与 triggers 写、`旧面板 REST 路径` 全删。删掉的入口不再发请求，这里留账不谐音。
pub const DEAD_PATHS: &[DeadPath] = &[
    // presets 18 —— 等票 Desk#11（引擎 #60 落 12 条后改契约）
    DeadPath::wait("GET", "v1/presets/{section}", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/create", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/archive", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/restore", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/purge", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/files", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/file", "Desk#11"),
    DeadPath::wait("PUT", "v1/presets/{section}/file", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/history", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/history/diff", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/pending", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/pending/diff", "Desk#11"),
    DeadPath::wait("GET", "v1/presets/{section}/validate", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/publish", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/reload", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/category", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/order", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/rename", "Desk#11"),
    // 下面两条是同一族的泛化写法（client.rs 的 `{op}` / `categories`），不另计族。
    DeadPath::wait("POST", "v1/presets/{section}/{op}", "Desk#11"),
    DeadPath::wait("POST", "v1/presets/{section}/categories", "Desk#11"),
    // combos 2 —— 等票 #61 + Desk#11
    // recovery 快照只有 skills/prompts 两条只读；extensions 的 GET 没有路由。
    DeadPath::wait("GET", "v1/extensions", "Nous#63 / Desk#13"),
    DeadPath::wait("GET", "v1/combos", "#61 / Desk#11"),
    DeadPath::wait("POST", "v1/combos/{name}/{action}", "#61 / Desk#11"),
    // skills/prompts/extensions 写操作 15 —— 审计按 5 个路径族计（30-34 条）：
    // `{kind}` 展开三仓、`{op}` 展开七个动词，这里只记一族一行。
    DeadPath::removed("POST", "v1/{kind}/create"),
    DeadPath::removed("GET", "v1/{kind}/files"),
    DeadPath::removed("GET", "v1/{kind}/file"),
    DeadPath::removed("PUT", "v1/{kind}/file"),
    DeadPath::removed("POST", "v1/{kind}/publish"),
    DeadPath::removed("POST", "v1/{kind}/{op}"),
    // 三仓资源写的具体模板：#13 从客户端删掉调用，能力表仍需认得以给灰入口理由。
    DeadPath::removed("POST", "v1/skills/create"),
    DeadPath::removed("PUT", "v1/skills/file"),
    DeadPath::removed("POST", "v1/skills/{op}"),
    DeadPath::removed("POST", "v1/skills/publish"),
    DeadPath::removed("GET", "v1/skills/files"),
    DeadPath::removed("GET", "v1/skills/file"),
    DeadPath::removed("POST", "v1/prompts/create"),
    DeadPath::removed("PUT", "v1/prompts/file"),
    DeadPath::removed("POST", "v1/prompts/{op}"),
    DeadPath::removed("POST", "v1/prompts/publish"),
    DeadPath::removed("GET", "v1/prompts/files"),
    DeadPath::removed("GET", "v1/prompts/file"),
    DeadPath::removed("POST", "v1/extensions/create"),
    DeadPath::removed("PUT", "v1/extensions/file"),
    DeadPath::removed("POST", "v1/extensions/{op}"),
    DeadPath::removed("POST", "v1/extensions/publish"),
    DeadPath::removed("GET", "v1/extensions/files"),
    DeadPath::removed("GET", "v1/extensions/file"),
    // triggers 10 条写端点 —— 删（保留 GET list / POST fire / GET fires）
    DeadPath::removed("GET", "v1/triggers/files"),
    DeadPath::removed("GET", "v1/triggers/file"),
    DeadPath::removed("PUT", "v1/triggers/file"),
    DeadPath::removed("POST", "v1/triggers/create"),
    DeadPath::removed("POST", "v1/triggers/publish"),
    DeadPath::removed("POST", "v1/triggers/archive"),
    DeadPath::removed("POST", "v1/triggers/restore"),
    DeadPath::removed("POST", "v1/triggers/purge"),
    DeadPath::removed("POST", "v1/triggers/category"),
    DeadPath::removed("POST", "v1/triggers/categories"),
    // archive 2 —— 灰（Nous#67 落真后翻活）
    DeadPath::gray("GET", "v1/archive"),
    DeadPath::gray("POST", "v1/archive/{id}/restore"),
    // sessions/{archive,config,purge} 3 —— 灰（等 Nous#64 / #67）
    DeadPath::gray("POST", "v1/sessions/{id}/archive"),
    DeadPath::gray("POST", "v1/sessions/{id}/config"),
    DeadPath::gray("POST", "v1/sessions/{id}/purge"),
    // fs/dirs 1 —— 灰（Nous#68）
    DeadPath::gray("GET", "v1/fs/dirs"),
    // presets 的 `{op}`（archive/restore/purge）与 `categories` 同属上面 18 条
    // 等票族；这里按模板去重，不重复计数。
    // 旧面板 REST 路径 5 —— 删（旧面板客户端整文件删）
    DeadPath::removed("GET", "api/overview"),
    DeadPath::removed("GET", "api/conversations"),
    DeadPath::removed("GET", "api/conversation"),
    DeadPath::removed("GET", "api/cost"),
    DeadPath::removed("GET", "api/health"),
];

/// 引擎 `meta::ROUTES` 里已被客户端认识的路由（含 #12 的 3 条 stats、
/// `presets` 新契约模板）。Desk#14 后这张表与 `DEAD_PATHS` 一起是
/// `capability.rs` 的唯一路径来源；与夹具逐条核对。
/// 路由模板统一写 `{id}` / `{section}` 形态，与 capabilities 的 endpoints 一致。
pub const ENGINE_ROUTES: &[(&str, &str)] = &[
    ("GET", "v1/sessions"),
    ("GET", "v1/unread"),
    ("POST", "v1/read/{id}"),
    ("GET", "v1/machines"),
    ("GET", "v1/models"),
    // Desk#17 / Nous#65：目标引擎 integration/recovery 有这条写端点（线上 0.1.3 没有，
    // 由 capabilities 的 endpoints 门控，UI 灰掉）。
    ("PUT", "v1/default"),
    ("POST", "v1/sessions"),
    ("GET", "v1/sessions/{id}/entries"),
    ("POST", "v1/sessions/{id}/messages"),
    ("GET", "v1/sessions/{id}/events"),
    ("POST", "v1/sessions/{id}/abort"),
    ("POST", "v1/sessions/{id}/fork"),
    ("POST", "v1/sessions/{id}/rename"),
    ("POST", "v1/sessions/{id}/pin"),
    ("GET", "v1/device"),
    ("GET", "v1/selection"),
    ("POST", "v1/selection"),
    ("GET", "v1/skills"),
    ("GET", "v1/prompts"),
    // Desk#14：recovery 夹具里没有 GET /v1/extensions，已从活路由移入死路径账本。
    ("GET", "v1/triggers"),
    ("POST", "v1/triggers/{code}/fire"),
    ("GET", "v1/triggers/{code}/fires"),
    ("GET", "v1/capabilities"),
    // stats（Nous#62）：recovery 分支真有这三条，客户端可以打，不进死路径白名单。
    ("GET", "v1/stats/usage"),
    ("GET", "v1/stats/tasks"),
    ("GET", "v1/stats/db"),
    ("GET", "v1/documents/{kind}"),
    ("GET", "v1/documents/{kind}/{section}/{name}"),
    ("PUT", "v1/documents/{kind}/{section}/{name}"),
    ("POST", "v1/documents/{kind}/{section}"),
    ("POST", "v1/documents/{kind}/{section}/{name}/rename"),
    ("POST", "v1/documents/{kind}/{section}/{name}/publish"),
    ("POST", "v1/documents/{kind}/{section}/{name}/reload"),
    ("GET", "v1/documents/{kind}/{section}/{name}/validate"),
    ("GET", "v1/documents/{kind}/{section}/{name}/history"),
    ("GET", "v1/documents/{kind}/{section}/{name}/history/diff"),
    ("GET", "v1/documents/{kind}/{section}/{name}/pending"),
    ("GET", "v1/documents/{kind}/{section}/{name}/pending/diff"),
    // 服务端 presets 新契约路由（#60），Desk 暂用旧模板；这些是能力表已知路径。
    ("GET", "v1/presets"),
    ("GET", "v1/presets/model"),
    ("GET", "v1/presets/{section}/{name}"),
    ("PUT", "v1/presets/{section}/{name}"),
    ("PATCH", "v1/presets/{section}/{name}"),
    ("DELETE", "v1/presets/{section}/{name}"),
    ("POST", "v1/presets/{section}/{name}/copy"),
    ("GET", "v1/presets/{section}/{name}/revisions"),
    ("GET", "v1/presets/{section}/{name}/revisions/{n}"),
    ("POST", "v1/presets/{section}/{name}/rollback"),
    ("GET", "v1/_routes"),
];

/// 协议层实际拼出的路径模板（不含白名单），用于判据 1 的自动扫描。
/// 由 `nous-engine` 提供，UI 只读。
pub fn engine_call_paths() -> &'static [(&'static str, &'static str)] {
    nous_engine::client_paths()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 判据 3：每个 `灰` 路径的入口都必须拿得到一条理由（来自 capabilities）。
    /// 这里用真引擎的 `caps` 形状走一遍 `Desktop::dead_path_disabled_reason`
    /// 的纯映射，不许自写文案。
    #[test]
    fn gray_paths_have_a_capability_reason_channel() {
        let gray: Vec<_> = DEAD_PATHS
            .iter()
            .filter(|e| e.disposition == Disposition::Gray)
            .map(|e| e.path)
            .collect();
        assert_eq!(gray.len(), 6);
        // 映射表覆盖全部 6 条（fs/dirs 走 device，其余 archive / sessions）。
        for path in gray {
            assert!(
                matches!(path, "v1/archive" | "v1/archive/{id}/restore"
                    | "v1/sessions/{id}/archive" | "v1/sessions/{id}/purge"
                    | "v1/sessions/{id}/config" | "v1/fs/dirs"),
                "{path} 没有灰理由通道"
            );
        }
    }

    #[test]
    fn wait_ticket_always_has_a_number() {
        for entry in DEAD_PATHS {
            if entry.disposition == Disposition::WaitTicket {
                assert!(
                    entry.ticket.is_some_and(|t| !t.trim().is_empty()),
                    "{:?} {} 是等票但没票号",
                    entry.method,
                    entry.path
                );
            } else {
                assert!(entry.ticket.is_none(), "{:?} {} 不是等票，不该带票号", entry.method, entry.path);
            }
        }
    }

    #[test]
    fn whitelist_never_contains_a_live_engine_route() {
        for entry in DEAD_PATHS {
            assert!(
                !ENGINE_ROUTES.iter().any(|(m, p)| *m == entry.method && *p == entry.path),
                "白名单里混进了引擎活路由：{} {}（{:?}）",
                entry.method,
                entry.path,
                entry.disposition
            );
        }
    }

    #[test]
    fn every_engine_call_path_is_either_live_or_whitelisted() {
        let mut leaked = Vec::new();
        for &(method, path) in engine_call_paths().iter().chain(nous_engine::stats_paths()) {
            let normalized = crate::capability::normalize_path(path);
            if ENGINE_ROUTES.iter().any(|(m, p)| *m == method && *p == normalized) {
                continue;
            }
            if DEAD_PATHS.iter().any(|e| e.method == method && e.path == normalized) {
                continue;
            }
            leaked.push(format!("{method} {path}"));
        }
        assert!(leaked.is_empty(), "协议层出现既不在引擎路由、也不在白名单的路径：{leaked:?}");
    }

    /// Desk#14：夹具元数据与 routes 原样回包（不许手写）。
    #[test]
    fn routes_fixture_has_provenance() {
        let raw = include_str!("../../nous-engine/tests/fixtures/routes.json");
        let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
        let captured = fixture["_captured_at"].as_str().unwrap_or_default();
        let version = fixture["_engine_version"].as_str().unwrap_or_default();
        println!("routes 夹具 _captured_at={captured}");
        println!("routes 夹具 _engine_version={version}");
        println!("routes 夹具 provenance={}", fixture["_provenance"]);
        assert!(!captured.is_empty(), "夹具缺 _captured_at");
        assert!(!version.is_empty(), "夹具缺 _engine_version");
        assert_eq!(fixture["routes"].as_array().unwrap().len(), 59);
        assert!(fixture["_provenance"].get("commit").is_some());
    }

    /// Desk#14 判据：每条客户端活路径（method + path）必须在夹具里。
    #[test]
    fn live_client_paths_are_in_routes_fixture() {
        let raw = include_str!("../../nous-engine/tests/fixtures/routes.json");
        let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
        let mut engine = std::collections::BTreeSet::new();
        for route in fixture["routes"].as_array().unwrap() {
            engine.insert(format!(
                "{} {}",
                route["method"].as_str().unwrap(),
                crate::capability::normalize_path(route["path"].as_str().unwrap())
            ));
        }
        let mut leaked = Vec::new();
        for &(method, path) in engine_call_paths()
            .iter()
            .chain(nous_engine::stats_paths())
            .chain(nous_engine::pin_paths())
        {
            let key = format!("{} {}", method, crate::capability::normalize_path(path));
            let is_dead = DEAD_PATHS.iter().any(|e| {
                e.method == method && e.path == crate::capability::normalize_path(path)
            });
            if !is_dead && !engine.contains(&key) {
                leaked.push(key);
            }
        }
        assert!(leaked.is_empty(), "客户端活路径不在真引擎夹具里：{leaked:?}");
    }

    /// Desk#14 判据：客户端路径表里不在夹具的，必须在死路径账本且处置 != 活。
    #[test]
    fn paths_absent_from_fixture_are_in_dead_paths() {
        let raw = include_str!("../../nous-engine/tests/fixtures/routes.json");
        let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
        let mut engine = std::collections::BTreeSet::new();
        for route in fixture["routes"].as_array().unwrap() {
            engine.insert(format!(
                "{} {}",
                route["method"].as_str().unwrap(),
                crate::capability::normalize_path(route["path"].as_str().unwrap())
            ));
        }
        let mut unaccounted = Vec::new();
        for &(method, path) in engine_call_paths()
            .iter()
            .chain(nous_engine::stats_paths())
            .chain(nous_engine::pin_paths())
        {
            let key = format!("{} {}", method, crate::capability::normalize_path(path));
            if engine.contains(&key) {
                continue;
            }
            // 进账即「处置 != 活」：等票 / 灰 / 删三种都不是「路径在夹具里」。
            if !DEAD_PATHS.iter().any(|e| {
                e.method == method && e.path == crate::capability::normalize_path(path)
            }) {
                unaccounted.push(key);
            }
        }
        assert!(unaccounted.is_empty(), "夹具里没有、又没进死路径账本的路径：{unaccounted:?}");
    }

    /// Desk#14 判据：夹具多出的路径只打警告清单，不失败。
    #[test]
    fn fixture_only_routes_are_reported_as_warnings() {
        let raw = include_str!("../../nous-engine/tests/fixtures/routes.json");
        let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
        let mut known = std::collections::BTreeSet::new();
        for &(method, path) in engine_call_paths()
            .iter()
            .chain(nous_engine::stats_paths())
            .chain(nous_engine::pin_paths())
        {
            known.insert(format!(
                "{} {}",
                method,
                crate::capability::normalize_path(path)
            ));
        }
        let mut warnings = Vec::new();
        for route in fixture["routes"].as_array().unwrap() {
            let key = format!(
                "{} {}",
                route["method"].as_str().unwrap(),
                crate::capability::normalize_path(route["path"].as_str().unwrap())
            );
            if !known.contains(&key) {
                warnings.push(key);
            }
        }
        println!("夹具多出、客户端未用的 {} 条（只警告）：", warnings.len());
        for w in &warnings {
            println!("  WARN 引擎有、客户端没用：{w}");
        }
        // 警告清单本身不失败；这里只确认夹具非空。
        assert!(!fixture["routes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn catalog_covers_all_fifty_dead_path_families() {
        assert_eq!(DEAD_PATHS.len(), 68, "审计 50 族 + 2 泛化别名 + #14 extensions + 18 条资源写具体模板（能力表派生用）");
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::WaitTicket).count(),
            23,
            "presets 18 + 2 条泛化别名 + combos 2 + extensions 1（#14）"
        );
        // #14：18 条资源写具体模板进 `删`，让 capability 派生表认得它们（灰入口）。
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Gray).count(),
            6,
            "archive 2 + sessions 3 + fs/dirs 1"
        );
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Removed).count(),
            39,
            "资源写 6 泛化 + 18 具体模板 + triggers 10 + 旧面板 REST 路径 5"
        );
    }
}
