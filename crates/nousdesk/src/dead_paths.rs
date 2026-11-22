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

/// 死路径族（与 `/var/tmp/audit/D-desk.md` §2.5 逐条对齐；条目随引擎落活而减少）。
///
/// `等票` / `灰` / `删` 的交叉口径按票面：presets 等 `Desk#11`、combos 等 `#61`、
/// archive 与 sessions/{config,archive,purge} 灰、fs/dirs 灰（Nous#68）、
/// 三仓的文件面已随 Nous 0.1.8 落活（移进 `ENGINE_ROUTES`）、触发器工程面随 Nous#95 落活、
/// 只有旧拓扑 `/{op}` 与 `旧面板 REST 路径` 是删。能力型端点一律不删。
pub const DEAD_PATHS: &[DeadPath] = &[
    // presets 旧 18 条（含 `{op}` / `categories` 泛化）已随 Desk#11 从客户端消失。
    // 台账对应条目整块删除 —— 不改成第四种状态（brief / AGENTS.md）。
    // 发布预留（Nous#75）：私库 git 写端点，夹具里还没有，等票。
    DeadPath::wait("POST", "v1/repos/presets/commit", "Nous#75"),
    DeadPath::wait("GET", "v1/repos/presets/history", "Nous#75"),
    // combos 已落活路由（`GET /v1/combos/{section}` + `PATCH /v1/combos/{section}/{name}`，
    // 见 `ENGINE_ROUTES`）：旧版的 `GET /v1/combos` 与 `POST /v1/combos/:name/:action`
    // 已从客户端消失，台账那两条整块删除。
    // 三仓的 `create/files/file/publish` 与 `GET /v1/extensions` 已在引擎 0.1.8 落活
    // （Nous#75 那批，见 `ENGINE_ROUTES`）—— 从等票台账移走；`/ {op}` 三条旧拓扑化石
    // 仍是删，留在下面。
    DeadPath::removed("POST", "v1/skills/{op}"),
    DeadPath::removed("POST", "v1/prompts/{op}"),
    DeadPath::removed("POST", "v1/extensions/{op}"),
    // triggers 的工程面（Nous#95）已随 0.1.12 落活：`/v1/trigger-projects/*` 13 条
    // 移进 `ENGINE_ROUTES`，死路径台账整块删除 —— 不改成第四种状态。
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
    // Nous 0.1.8：三个编辑页的文件面（清单 / 读写 / 新建 / 发布）。
    ("GET", "v1/skills/files"),
    ("GET", "v1/skills/file"),
    ("PUT", "v1/skills/file"),
    ("POST", "v1/skills/create"),
    ("POST", "v1/skills/publish"),
    ("GET", "v1/prompts"),
    ("GET", "v1/prompts/files"),
    ("GET", "v1/prompts/file"),
    ("PUT", "v1/prompts/file"),
    ("POST", "v1/prompts/create"),
    ("POST", "v1/prompts/publish"),
    ("GET", "v1/extensions"),
    ("GET", "v1/extensions/files"),
    ("GET", "v1/extensions/file"),
    ("PUT", "v1/extensions/file"),
    ("POST", "v1/extensions/create"),
    ("POST", "v1/extensions/publish"),
    // Nous 0.1.10：资源页恢复 TS 时代那一套要的读面。
    // 类别词表与 reload 早就在引擎里；私库的归档区读面是 0.1.10 新加的
    // （写面 archive/restore/purge 一直在，缺读 → 「归档了就再也看不见」）。
    ("GET", "v1/extensions/categories"),
    ("POST", "v1/extensions/reload"),
    ("POST", "v1/extensions/duplicate"),
    ("GET", "v1/repos/extend/variants"),
    ("GET", "v1/repos/extend/archive"),
    // 仓级生命周期：三个内容仓共用一套路由，仓名写在路径里而不是 `{repo}` 模板 ——
    // 台账要的是「客户端真会打的这一条」。
    ("POST", "v1/repos/extend/archive"),
    ("POST", "v1/repos/extend/restore"),
    ("POST", "v1/repos/extend/purge"),
    ("POST", "v1/repos/extend/variants/bind"),
    ("POST", "v1/repos/extend/variants/branch"),
    ("POST", "v1/repos/extend/variants/unbind"),
    ("POST", "v1/repos/extend/variants/publish"),
    ("POST", "v1/repos/extend/variants/prune"),
    ("POST", "v1/repos/skills/archive"),
    ("POST", "v1/repos/skills/restore"),
    ("POST", "v1/repos/skills/purge"),
    ("POST", "v1/repos/prompts/archive"),
    ("POST", "v1/repos/prompts/restore"),
    ("POST", "v1/repos/prompts/purge"),
    // Desk#14：recovery 夹具里没有 GET /v1/extensions，已从活路由移入死路径账本。
    ("GET", "v1/triggers"),
    ("POST", "v1/triggers/{code}/fire"),
    ("GET", "v1/triggers/{code}/fires"),
    // Nous#95：触发器工程面（n8qkkj 的宿主）13 条。与上面的定义面不是一层。
    ("GET", "v1/trigger-projects"),
    ("GET", "v1/trigger-projects/files"),
    ("GET", "v1/trigger-projects/file"),
    ("PUT", "v1/trigger-projects/file"),
    ("POST", "v1/trigger-projects/create"),
    ("POST", "v1/trigger-projects/publish"),
    ("POST", "v1/trigger-projects/archive"),
    ("POST", "v1/trigger-projects/restore"),
    ("POST", "v1/trigger-projects/purge"),
    ("GET", "v1/trigger-projects/categories"),
    ("POST", "v1/trigger-projects/categories"),
    ("POST", "v1/trigger-projects/category"),
    ("POST", "v1/trigger-projects/sync"),
    ("GET", "v1/capabilities"),
    // stats（Nous#62）：recovery 分支真有这三条，客户端可以打，不进死路径白名单。
    ("GET", "v1/stats/usage"),
    ("GET", "v1/stats/tasks"),
    ("GET", "v1/stats/db"),
    // 数据监控室六屏复原：明细读口（同属 stats 插件）。
    ("GET", "v1/stats/overview"),
    ("GET", "v1/stats/conversations"),
    ("GET", "v1/stats/conversation/{id}"),
    ("GET", "v1/stats/cost"),
    ("GET", "v1/stats/health"),
    // Nous#90：documents 没有产品面（内容真相只有四个私库，见 decisions 2026-11-21）。
    // Desk 不再调用；这些条目等引擎侧下线后整块删除。
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
    // 服务端 presets 新契约路由（#60 / Desk#11）：夹具 12 条 + sections + order。
    // `PUT /v1/presets/{section}/order` 夹具里也在，之前 ENGINE_ROUTES 漏抄，补上。
    ("GET", "v1/presets/model"),
    ("GET", "v1/presets/{section}"),
    ("PUT", "v1/presets/{section}/order"),
    ("GET", "v1/presets/{section}/categories"),
    ("POST", "v1/presets/{section}/create"),
    ("GET", "v1/presets/{section}/{name}"),
    ("PUT", "v1/presets/{section}/{name}"),
    ("PATCH", "v1/presets/{section}/{name}"),
    ("DELETE", "v1/presets/{section}/{name}"),
    ("POST", "v1/presets/{section}/{name}/copy"),
    ("GET", "v1/presets/{section}/{name}/revisions"),
    ("GET", "v1/presets/{section}/{name}/revisions/{n}"),
    ("POST", "v1/presets/{section}/{name}/rollback"),
    // combos（Nous 0.1.7）：开关面按段取，旧的全局 GET 与 enable/disable 两条动作已取消。
    ("GET", "v1/combos/{section}"),
    ("PATCH", "v1/combos/{section}/{name}"),
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

    /// Nous 0.1.8：三仓的 `create/files/file/publish` 已落活 —— 活路由表必须有，
    /// 死路径账本里不许再留；触发器工程面（Nous#95）也已落活。
    #[test]
    fn resource_file_endpoints_and_trigger_projects_are_live() {
        let resources: Vec<_> = ["skills", "prompts", "extensions"]
            .into_iter()
            .flat_map(|kind| {
                [
                    ("POST", "create"),
                    ("GET", "files"),
                    ("GET", "file"),
                    ("PUT", "file"),
                    ("POST", "publish"),
                ]
                .into_iter()
                .map(move |(method, op)| (method, format!("v1/{kind}/{op}")))
            })
            .collect();
        for (method, path) in resources {
            assert!(
                ENGINE_ROUTES.iter().any(|(m, p)| *m == method && *p == path),
                "{method} {path} 已落活却没进活路由表"
            );
            assert!(
                !DEAD_PATHS.iter().any(|e| e.method == method && e.path == path),
                "{method} {path} 已落活，不许还挂在死路径账本里"
            );
        }
        let triggers = [
            ("GET", "v1/trigger-projects"),
            ("GET", "v1/trigger-projects/files"),
            ("GET", "v1/trigger-projects/file"),
            ("PUT", "v1/trigger-projects/file"),
            ("POST", "v1/trigger-projects/create"),
            ("POST", "v1/trigger-projects/publish"),
            ("POST", "v1/trigger-projects/archive"),
            ("POST", "v1/trigger-projects/restore"),
            ("POST", "v1/trigger-projects/purge"),
            ("GET", "v1/trigger-projects/categories"),
            ("POST", "v1/trigger-projects/categories"),
            ("POST", "v1/trigger-projects/category"),
            ("POST", "v1/trigger-projects/sync"),
        ];
        for (method, path) in triggers {
            assert!(
                ENGINE_ROUTES.iter().any(|(m, p)| *m == method && *p == path),
                "{method} {path} 已落活却没进活路由表"
            );
            assert!(
                !DEAD_PATHS.iter().any(|e| e.method == method && e.path == path),
                "{method} {path} 已落活，不许还挂在死路径台账里"
            );
        }
        let removed: Vec<_> = DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Removed).collect();
        assert_eq!(removed.len(), 8, "删只允许 3 条旧拓扑化石 + 5 条旧面板");
        assert_eq!(removed.iter().filter(|e| e.path.ends_with("/{op}")).count(), 3, "三仓旧拓扑化石应在删里");
        assert!(removed.iter().all(|e| e.path.starts_with("api/") || e.path.ends_with("/{op}")), "删里混进能力型端点");
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
        assert_eq!(
            fixture["routes"].as_array().unwrap().len(),
            152,
            "0.1.15 整份重抓 139 + 触发器工程面 13 条（Nous#95）"
        );
        assert!(fixture["_provenance"].get("commit").is_some());
    }

    /// 夹具里的路由模板 + 把 `{repo}` 展开成四个真仓名。
    ///
    /// 引擎侧 `/v1/repos/{repo}/…` 是**一条**路由带路径参数，客户端打的却是
    /// `/v1/repos/extend/archive` 这种具体地址。只按模板字面比，两边永远对不上，
    /// 于是「客户端在用的路径」会被误判成漏账。
    fn fixture_engine_paths() -> std::collections::BTreeSet<String> {
        let raw = include_str!("../../nous-engine/tests/fixtures/routes.json");
        let fixture: serde_json::Value = serde_json::from_str(raw).unwrap();
        let mut engine = std::collections::BTreeSet::new();
        for route in fixture["routes"].as_array().unwrap() {
            let method = route["method"].as_str().unwrap().to_string();
            let path = crate::capability::normalize_path(route["path"].as_str().unwrap());
            engine.insert(format!("{method} {path}"));
            if path.contains("{repo}") {
                for repo in ["presets", "skills", "prompts", "extend"] {
                    engine.insert(format!("{method} {}", path.replace("{repo}", repo)));
                }
            }
        }
        engine
    }

    /// Desk#14 判据：每条客户端活路径（method + path）必须在夹具里。
    #[test]
    fn live_client_paths_are_in_routes_fixture() {
        let engine = fixture_engine_paths();
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
        let engine = fixture_engine_paths();
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
    fn catalog_covers_every_dead_path_family() {
        assert_eq!(
            DEAD_PATHS.len(),
            16,
            "27 条减去触发器工程面 11 条（Nous#95 落活，移进 ENGINE_ROUTES）"
        );
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::WaitTicket).count(),
            2,
            "剩 presets 2 条等票"
        );
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Gray).count(),
            6,
            "archive 2 + sessions 3 + fs/dirs 1"
        );
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Removed).count(),
            8,
            "旧拓扑化石 3 + 旧面板 REST 路径 5；能力型端点一律不删"
        );
    }
}


/// Desk#14 判据 A：扫源码字面量，逐条进账（夹具 / DEAD_PATHS / 客户端清单）。
/// 新增一条 `v1/...` 活路径却没进账，这里 FAIL 并打印 文件:行。
#[cfg(test)]
mod source_literal_audit {
    use std::path::{Path, PathBuf};

    /// 从字面量里取出路径部分。只认 `v1/` 开头（允许前导 `/`），丢掉 query；
    /// `:id` 归一成 `{id}`，`format!` 的匿名占位 `{}` 归一成 `{op}`。
    fn path_from_literal(literal: &str) -> Option<String> {
        let trimmed = literal.trim();
        let has_placeholder = trimmed.starts_with("v1/{") || trimmed.starts_with("/v1/{");
        if !trimmed.starts_with("v1/") && !trimmed.starts_with("/v1/") { return None; }
        let path = trimmed.split(['?', '#', ' ', '，', '。']).next().unwrap_or(trimmed);
        // `v1/archive/` 这类前缀判断常量不是一条路径。
        if path.ends_with('/') && !has_placeholder { return None; }
        // format! 的 `{{op}}` 经过 normalize 会变成 `{{op}}` 这种双花括号，
        // 只认归一后保留单花括号的模板；双花括号由源码原文分支展开。
        if path.contains("{{") && !has_placeholder { return None; }
        let normalized = crate::capability::normalize_path(path)
            .split('/')
            .map(|seg| if seg == "{}" { "{op}".to_string() } else { seg.to_string() })
            .collect::<Vec<_>>()
            .join("/");
        Some(normalized)
    }

    /// 极小 Rust 词法器：只负责把注释和字符串分开。字符串里的 `\"` / `\\`
    /// 正确跳过，原始字符串 `r#"..."#` 也认，char 字面量里的引号不误判。
    fn literals(line: &str) -> Vec<(usize, String)> {
        let b = line.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'/' if i + 1 < b.len() && b[i + 1] == b'/' => break,
                b'r' if i + 1 < b.len() && (b[i + 1] == b'"' || b[i + 1] == b'#') => {
                    let mut j = i + 1;
                    let mut hashes = 0;
                    while j < b.len() && b[j] == b'#' { hashes += 1; j += 1; }
                    if j >= b.len() || b[j] != b'"' { i += 1; continue; }
                    let start = j + 1;
                    let mut k = start;
                    while k < b.len() {
                        if b[k] == b'"' {
                            let mut n = 0;
                            while n < hashes && k + 1 + n < b.len() && b[k + 1 + n] == b'#' { n += 1; }
                            if n == hashes {
                                out.push((start, line[start..k].to_string()));
                                k += 1 + hashes;
                                break;
                            }
                        }
                        k += 1;
                    }
                    i = k.max(i + 1);
                }
                b'"' => {
                    let start = i + 1;
                    let mut k = start;
                    while k < b.len() {
                        if b[k] == b'\\' { k += 2; continue; }
                        if b[k] == b'"' { break; }
                        k += 1;
                    }
                    out.push((start, line[start..k.min(b.len())].to_string()));
                    i = (k + 1).min(b.len());
                }
                b'\'' => {
                    let mut k = i + 1;
                    while k < b.len() {
                        if b[k] == b'\\' { k += 2; continue; }
                        if b[k] == b'\'' { k += 1; break; }
                        k += 1;
                    }
                    i = k;
                }
                _ => i += 1,
            }
        }
        out
    }

    fn source_files(root: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(root).expect("源码目录可读") {
            let entry = entry.expect("目录项可读");
            let path = entry.path();
            if path.is_dir() { source_files(&path, out); }
            else if path.extension().and_then(|e| e.to_str()) == Some("rs") { out.push(path); }
        }
    }

    /// #13 判据 1 的等价物哨兵：nousdesk 侧必须**递归**扫 `src/`，
    /// 子目录（如 `src/app/scrollbar.rs`）里的 v1 字面量不能漏账。
    #[test]
    fn source_literal_audit_scans_nousdesk_subdirectories() {
        // 不引用自己的函数名做 split 锚点，免得哨兵命中自己。
        let src = include_str!("dead_paths.rs");
        assert!(
            src.contains("source_files(&manifest.join(\"src\"), &mut files)"),
            "nousdesk 侧没有递归扫 src/ 子目录"
        );
        let skip_needle = ["if path.is_dir()", " { continue; }"].concat();
        assert!(!src.contains(&skip_needle), "nousdesk 侧还在跳过子目录");
    }

    #[test]
    fn every_v1_literal_in_source_is_accounted_for() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        source_files(&manifest.join("../nous-engine/src"), &mut files);
        // #13：nousdesk 侧同样递归扫 src/**（原来是 read_dir 顶层，
        // `src/app/scrollbar.rs` 这类子目录里的字面量会漏账）。
        // 测试模块与账本自身在下面的循环里单独排除。
        source_files(&manifest.join("src"), &mut files);

        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../nous-engine/tests/fixtures/routes.json"
        )).unwrap();
        let fixture_paths: Vec<String> = fixture["routes"].as_array().unwrap().iter()
            .map(|r| crate::capability::normalize_path(r["path"].as_str().unwrap()))
            .collect();
        let client_paths: Vec<String> = nous_engine::client_paths().iter()
            .chain(nous_engine::stats_paths().iter())
            .chain(nous_engine::pin_paths().iter())
            .map(|(_, p)| crate::capability::normalize_path(p))
            .collect();
        let dead_paths: Vec<String> = super::DEAD_PATHS.iter()
            .map(|e| crate::capability::normalize_path(e.path))
            .collect();

        let mut scanned = 0usize;
        let mut leaked = Vec::new();
        let self_file = std::fs::canonicalize(manifest.join("src/dead_paths.rs")).unwrap();
        for file in &files {
            // 账本自身躺在 src/ 下，但它是断言用的生成模板，不是调用点。
            if std::fs::canonicalize(file).unwrap() == self_file { continue; }
            let text = std::fs::read_to_string(file).expect("源码可读");
            // `#[cfg(test)] mod tests` 里的断言字符串不是调用点，不进账。
            let mut in_test_mod = false;
            let mut brace_depth = 0usize;
            for (line_no, line) in text.lines().enumerate() {
                if in_test_mod {
                    brace_depth += line.matches('{').count();
                    brace_depth = brace_depth.saturating_sub(line.matches('}').count());
                    if brace_depth == 0 { in_test_mod = false; }
                    continue;
                }
                if line.contains("#[cfg(test)]") {
                    in_test_mod = true;
                    brace_depth = 0;
                    continue;
                }
                for (col, literal) in literals(line) {
                    let Some(path) = path_from_literal(&literal) else { continue };
                    let raw = literal.trim();
                    // format! 的 `{}` 归一后成了 `{op}`，但源码里本来不是路径段；
                    // 展开逻辑按 `literal` 原文判断，不让归一结果污染后续匹配。
                    let anonymous = raw.contains("v1/{}/") || raw.contains("/v1/{}/");
                    scanned += 1;
                    // 生成模板（`{repo}` / `{}`）按已知取值展开：资源仓三种 ×
                    // presets 生命周期动词；任一实际展开没进账就算漏账。
                    let expansions: Vec<String> = if path.contains("{repo}") {
                        // UI 的 `/v1/{repo}/...`：两种叫法都存在，按前缀分开取。
                        // 文件面（`/v1/{repo}/file`）是三种资源仓；私库面
                        // （`/v1/repos/{repo}/…`）是四个**仓名**，扩展在那里叫 `extend`。
                        let template = path.clone();
                        if path.starts_with("v1/repos/{repo}") {
                            ["presets", "skills", "prompts", "extend"].into_iter()
                                .map(move |repo| template.replace("{repo}", repo))
                                .collect()
                        } else {
                            ["skills", "prompts", "extensions"].into_iter()
                                .map(move |kind| template.replace("{repo}", kind))
                                .collect()
                        }
                    } else if path.contains("{}") && !path.starts_with("v1/documents/")
                        && !path.strip_prefix(concat!("v1/presets", "/")).is_some_and(|_| true) {
                        // `format!("/v1/{}/publish", kind.repo().key())`：{} 是资源仓。
                        let template = path.clone();
                        ["skills", "prompts", "extensions"].into_iter()
                            .map(move |kind| template.replace("{}", kind))
                            .collect()
                    } else if path.contains("{op}") && anonymous {
                        // `format!("/v1/{}/publish", kind.repo().key())` 已被归一成 `{op}`。
                        let template = path.clone();
                        ["skills", "prompts", "extensions"].into_iter()
                            .map(move |kind| template.replace("{op}", kind))
                            .collect()
                    } else if path.contains("{}") && path.starts_with("v1/documents/") {
                        vec![path.replace("{}", "{name}")]
                    } else if path.contains("{}") && path.strip_prefix(concat!("v1/presets", "/")).is_some_and(|_| true) {
                        ["archive", "restore", "purge"].into_iter()
                            .map(|op| path.replace("{}", op)).collect()
                    } else {
                        vec![path.clone()]
                    };
                    for expanded in expansions {
                        // `{tail}` 是路径末段的泛化：只有账本里存在**同一个泛化模板**
                        // 才算进过账（不能拿无关路径的前缀兜底，否则哨兵混得过去）。
                        let family = if expanded.ends_with("/{tail}") {
                            Some(expanded.clone())
                        } else {
                            None
                        };
                        let known = fixture_paths.contains(&expanded)
                            || dead_paths.contains(&expanded)
                            || client_paths.iter().any(|p| {
                                p == &expanded || p.strip_prefix(&expanded).is_some_and(|t| t.starts_with('/'))
                            })
                            || family.as_ref().is_some_and(|template| {
                                // `{tail}` 是任意尾段：client 清单里的 documents 12 条
                                // 都属这一族；账本里其它泛化模板按字面精确匹配。
                                if template == "v1/documents/{kind}/{tail}" {
                                    client_paths.iter().chain(fixture_paths.iter())
                                        .any(|p| p.starts_with("v1/documents/{kind}/{section}"))
                                } else {
                                    fixture_paths.iter().chain(dead_paths.iter()).chain(client_paths.iter())
                                        .any(|p| p == template)
                                }
                            })
                            // `{{op}}` 在 format! 里是字面 `{op}` 模板（资源仓三族）。
                            || expanded.contains("{{op}}") && ["skills", "prompts", "extensions"].iter()
                                .any(|kind| dead_paths.iter().any(|p| p == &format!("v1/{kind}/{{op}}")));
                        if !known {
                            leaked.push(format!("{}:{}:{} {expanded}", file.display(), line_no + 1, col));
                        }
                    }
                }
            }
        }
        assert!(scanned > 70, "扫描器失灵：只找到 {scanned} 条 v1 字面量");
        assert!(leaked.is_empty(), "源码里的 v1 路径没进夹具 / DEAD_PATHS / 客户端清单：\n{}", leaked.join("\n"));
    }
}
