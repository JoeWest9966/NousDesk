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

/// 引擎 `meta::ROUTES` 在本次审计时的 35 条（用于「白名单不许留活路径」自检）。
/// 路由模板统一写 `{id}` / `{section}` 形态，与 capabilities 的 endpoints 一致。
pub const ENGINE_ROUTES: &[(&str, &str)] = &[
    ("GET", "v1/sessions"),
    ("GET", "v1/unread"),
    ("POST", "v1/read/{id}"),
    ("GET", "v1/machines"),
    ("GET", "v1/models"),
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
    ("GET", "v1/extensions"),
    ("GET", "v1/triggers"),
    ("POST", "v1/triggers/{code}/fire"),
    ("GET", "v1/triggers/{code}/fires"),
    ("GET", "v1/capabilities"),
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
        for &(method, path) in engine_call_paths() {
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

    #[test]
    fn catalog_covers_all_fifty_dead_path_families() {
        assert_eq!(DEAD_PATHS.len(), 49, "审计 50 族按模板归类后的行数（含 2 条泛化别名）");
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::WaitTicket).count(),
            22,
            "presets 18 + 2 条泛化别名 + combos 2"
        );
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Gray).count(),
            6,
            "archive 2 + sessions 3 + fs/dirs 1"
        );
        assert_eq!(
            DEAD_PATHS.iter().filter(|e| e.disposition == Disposition::Removed).count(),
            21,
            "资源写 6 行（覆盖审计 8 族）+ triggers 10 + 旧面板 REST 路径 5"
        );
    }
}
