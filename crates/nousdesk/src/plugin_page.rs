//! 插件页的条目分类、分段与「详情能看到什么」——纯函数，不碰 GPUI、不碰 HTTP。
//!
//! 两份原料，各管一半，别混：
//! - `GET /v1/capabilities` 的 `modules[]`：**端点模块**（15 个）。有 enabled / vendor /
//!   version / layer / crate / endpoints。列表里那个「15 个模块」统计行也只数这一份。
//! - `GET /v1/modules/graph` 的 `nodes`：**全部插件**（31 个）。多出来的 17 个不在
//!   capabilities 里，客户端没有它们的元数据 —— 只有名字，和边反查出来的包名。
//!
//! 所以这里只做三件事：
//!
//! 1. **分类**：capabilities 里有 = 模块；图里有、capabilities 里没有 = 依赖层的**库**
//!    （或 [`UNREGISTERED`] 里那几个「自带端点却没进清单」的）。分类是**推导**，
//!    不维护第二份模块名单。
//! 2. **分段**：库按被依赖数降序（同数按名字），列表照这个顺序排 —— 被用得最多的先看见。
//! 3. **诚实的空**：库没有版本 / 层 / 必需 / 端点，这里给 `None` / 空，UI 显示 `—`，
//!    不拿 caps 的 server.version 或「0 端点」冒充。**库唯一的硬事实是「图里有它」**。
//!
//! 不算的东西：不做传递闭包、不做中文别名、不给库编版本号。

use std::collections::BTreeMap;

use nous_engine::{Capabilities, Module};

use crate::plugin_graph::Relations;

/// 自带端点、却没进 `capabilities.modules` 的插件名 —— **客户端暂时补的一笔账**。
///
/// 为什么需要这张表：内核 `AboutME_ModuleRelationshipNetwork` 的 `GET /v1/modules/graph`
/// 真能打到（200），但它既不在 `capabilities.modules` 里、也不在 `/v1/_routes` 的 165 条里，
/// 只在关系图的 `nodes` 里露一个名字。于是「图里有、caps 里没有」这个推导条件分不出
/// 「库」和「未登记的模块」——引擎侧没有任何一份清单能给出答案（#21 第三处，待拍板）。
///
/// **只在拍板之前存在**：`#21` 若走「补登记」，它会进 caps 的 modules，
/// [`Index::build`] 的「在 caps 里就是模块」会把这条表压过去；若走「明确不登记」，
/// 删掉这一行 + 把 `Kind::Unregistered` 的 `未登记琥珀` 一并去掉。
const UNREGISTERED: &[&str] = &["AboutME_ModuleRelationshipNetwork"];

/// 一个插件条目到底是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `capabilities.modules` 的成员：自带端点，客户端有入口或至少登记了一行。
    Module,
    /// 依赖层：图里有、`capabilities` 里没有 —— 只有被人依赖 / 依赖别人的关系。
    Lib,
    /// 自带端点却没进 capabilities 的插件（见 [`UNREGISTERED`]）。
    Unregistered,
}

impl Kind {
    /// 详情键值表「类型」那一行的值。库的 `plugin.toml kind` 客户端拿不到，但
    /// 「它是库」这件事是确定的，所以这一格不是 `—`。
    pub(crate) fn type_text(self) -> &'static str {
        match self {
            Kind::Lib => "依赖（库）",
            Kind::Unregistered => "未登记",
            Kind::Module => "—",
        }
    }
}

/// 分类结果：名字 → 种类，外加排好序的两段名单。
///
/// 模块那段**不在这里存**：它的顺序是 capabilities 的顺序（引擎说了算），
/// 抄一份进这里就等于第二份真相。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Index {
    kinds: BTreeMap<String, Kind>,
    libs: Vec<String>,
    unregistered: Vec<String>,
    /// 图里节点的包名（从入边的 `via` 反推）。模块不用它 —— `capabilities` 自己有
    /// `crate`；这是库唯一拿得到的「来源 crate」。
    packages: BTreeMap<String, String>,
}

impl Index {
    /// 用 capabilities 的模块表和关系图建索引。图还没拉到（`None`）时只有模块那一层：
    /// 库这一段等图到了再出现，不拿「空」冒充「没有库」。
    pub(crate) fn build(modules: &[Module], rel: Option<&Relations>) -> Self {
        let mut kinds: BTreeMap<String, Kind> = modules
            .iter()
            .map(|m| (m.name.clone(), Kind::Module))
            .collect();
        let mut libs: Vec<String> = Vec::new();
        let mut unregistered: Vec<String> = Vec::new();
        let mut dependents: BTreeMap<String, usize> = BTreeMap::new();
        let mut packages: BTreeMap<String, String> = BTreeMap::new();

        if let Some(rel) = rel {
            for name in &rel.nodes {
                if kinds.contains_key(name) {
                    continue;
                }
                let unlisted = UNREGISTERED.contains(&name.as_str());
                kinds.insert(name.clone(), if unlisted { Kind::Unregistered } else { Kind::Lib });
                // 排序只看被依赖数：被用得最多的库排前面（「谁都得用」的那几个先露面）。
                dependents.insert(name.clone(), rel.dependents(name).len());
                if let Some(pkg) = rel.package_of(name) {
                    packages.insert(name.clone(), pkg.to_string());
                }
                if unlisted {
                    unregistered.push(name.clone());
                } else {
                    libs.push(name.clone());
                }
            }
        }
        // 同数按名字：同样的图给同样的顺序，快照比对不抖。
        libs.sort_by(|a, b| {
            dependents
                .get(b)
                .cmp(&dependents.get(a))
                .then_with(|| a.cmp(b))
        });
        unregistered.sort();

        Index {
            kinds,
            libs,
            unregistered,
            packages,
        }
    }

    /// 某个条目的详情原料。模块去 `capabilities` 里找自己那一行；库 / 未登记只有名字 + 包名。
    /// 名字两份清单里都没有（选中项被删）→ `None`，由 UI 回落。
    pub(crate) fn view<'a>(&'a self, name: &'a str, caps: &'a Capabilities) -> Option<View<'a>> {
        match self.kind(name)? {
            Kind::Module => caps
                .modules
                .iter()
                .find(|m| m.name == name)
                .map(|m| View::module(m, &caps.server.version)),
            other => Some(View::graph_only(
                name,
                other,
                self.packages.get(name).map(String::as_str),
            )),
        }
    }

    /// 这个名字属于哪一类。不在任何一份清单里 → `None`（选中的模块被删掉时走回落）。
    pub(crate) fn kind(&self, name: &str) -> Option<Kind> {
        self.kinds.get(name).copied()
    }

    /// 依赖层那一段（已按被依赖数排序）。
    pub(crate) fn libs(&self) -> &[String] {
        &self.libs
    }

    /// 未登记那一段（没进 capabilities、但自带端点）。
    pub(crate) fn unregistered(&self) -> &[String] {
        &self.unregistered
    }
}

/// 详情的原料：模块来自 capabilities，库只有图给的名字和边反推的包名。
///
/// 两种条目走**同一个**详情渲染，所以这里把「有没有这个字段」压成 `Option`，
/// UI 只负责把 `None` 显示成 `—`。库不许借用模块的 `caps.server.version` 冒充版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct View<'a> {
    pub name: &'a str,
    pub kind: Kind,
    pub vendor: Option<&'a str>,
    pub version: Option<&'a str>,
    /// 「类型」那一行：模块是引擎报的 `kind`（`app` / `platform` …），库是「依赖（库）」。
    pub type_field: Option<&'a str>,
    pub layer: Option<&'a str>,
    pub required: Option<bool>,
    pub crate_name: Option<&'a str>,
    pub description: Option<&'a str>,
    /// 模块自报的「为什么没开」（`capabilities.modules[].reason`）。库没有这一格。
    pub reason: Option<&'a nous_engine::Reason>,
    /// 引擎自报的运行时开关。库不在 capabilities 里 → `None`（UI 显示 `—`，不是「未启用」）。
    pub enabled: Option<bool>,
    /// 引擎声明的端点。库恒为空 —— UI 那句「库不提供端点」不是从这里数出来的。
    pub endpoints: &'a [String],
}

impl<'a> View<'a> {
    /// 模块：字段照 capabilities 抄，缺的就是 `None`。
    /// 版本是唯一有回落的字段（老引擎不发 `version` 时用 `caps.server.version`）——
    /// 库**没有**这个回落：它不在 capabilities 里，借引擎版本会变成一句假话。
    pub(crate) fn module(module: &'a Module, version_fallback: &'a str) -> Self {
        View {
            name: module.name.as_str(),
            kind: Kind::Module,
            vendor: module.vendor.as_deref(),
            version: Some(module.version.as_deref().unwrap_or(version_fallback)),
            type_field: module.kind.as_deref(),
            layer: module.layer.as_deref(),
            required: module.required,
            crate_name: module.crate_name.as_deref(),
            description: module.description.as_deref(),
            reason: module.reason.as_ref(),
            enabled: Some(module.enabled),
            endpoints: &module.endpoints,
        }
    }

    /// 图里有、capabilities 里没有的条目：名字 + 种类，外加从边的 `via` 反推的包名
    /// （`Extend → durable via nous-durable`，所以 durable 的包名就是 `nous-durable`）。
    /// 其余字段一律 `None` —— 客户端没有它们，就不编。
    /// 「读文档」这一颗挂不挂。库的 README 不进 `docs/plugins/`（#21「不做」），
    /// 未登记的模块引擎不认识（点了只会得到「引擎不认识这个模块名」）——所以只给模块。
    pub(crate) fn has_readme(&self) -> bool {
        self.kind == Kind::Module
    }

    pub(crate) fn graph_only(name: &'a str, kind: Kind, package: Option<&'a str>) -> Self {
        View {
            name,
            kind: if kind == Kind::Module { Kind::Lib } else { kind },
            vendor: None,
            version: None,
            type_field: Some(kind.type_text()),
            layer: None,
            required: None,
            crate_name: package,
            description: None,
            reason: None,
            enabled: None,
            endpoints: &[],
        }
    }
}

/// 列表行右侧那两行字。
///
/// 模块：`v{x} 状态 启用` / `17 端点`（#19：标签 + 空格 + 值，不用 `·` 糊成一句）。
/// 库：`库` / `零端点` —— 票面钉死「不写 `0 端点`」：库不是「数出来 0 条」，
/// 它按定义就不提供端点，用「0」会把「还没数」和「结构上没有」混成一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RowMeta {
    /// 上行的标签部分（值为 `None` 时整行就是它）。
    pub top_label: String,
    /// 上行的值（模块是状态 `启用` / `未启用`；库没有这一格）。
    pub top_value: Option<String>,
    pub bottom: String,
}

pub(crate) fn row_meta(view: &View<'_>) -> RowMeta {
    match view.kind {
        Kind::Module => RowMeta {
            top_label: format!("v{} 状态", view.version.unwrap_or("—")),
            top_value: view.enabled.map(crate::capability::module_state_label).map(str::to_string),
            bottom: format!("{} 端点", view.endpoints.len()),
        },
        // 库没有版本（图不发）、没有状态（不在 capabilities）：上行只报种类。
        Kind::Lib => RowMeta {
            top_label: "库".to_string(),
            top_value: None,
            bottom: "零端点".to_string(),
        },
        Kind::Unregistered => RowMeta {
            top_label: "未登记".to_string(),
            top_value: None,
            bottom: "自带端点".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Index, Kind, RowMeta, View, row_meta};
    use crate::plugin_graph::Relations;

    fn module(name: &str, version: Option<&str>, enabled: bool, endpoints: &[&str]) -> nous_engine::Module {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "enabled": enabled,
            "version": version,
            "kind": "app",
            "layer": "app",
            "required": true,
            "crate": "nous-x",
            "vendor": "Nous",
            "endpoints": endpoints,
        }))
        .unwrap()
    }

    /// 0.1.41 真机形状的缩小版：caps 有 sessions / env；图里多出 durable / ai / wire /
    /// AboutME（未登记）。边只留够反查的。
    fn relations() -> Relations {
        let graph: nous_engine::ModuleGraph = serde_json::from_value(serde_json::json!({
            "generatedFor": "0.1.41",
            "nodes": ["sessions", "env", "durable", "ai", "wire", "Trigger_Core", "combos",
                      "AboutME_ModuleRelationshipNetwork", "server", "fixture"],
            "edges": [
                { "from": "sessions", "to": "durable", "via": "nous-durable" },
                { "from": "sessions", "to": "ai", "via": "nous-ai" },
                { "from": "Trigger_Core", "to": "durable", "via": "nous-durable" },
                { "from": "combos", "to": "durable", "via": "nous-durable" },
                { "from": "durable", "to": "wire", "via": "nous-wire" },
                { "from": "server", "to": "AboutME_ModuleRelationshipNetwork",
                  "via": "aboutme-module-relationship-network" }
            ],
            "unmapped": []
        }))
        .unwrap();
        Relations::build(&graph)
    }

    /// 分类只由「caps 里有没有」定：图里多出来的就是依赖层，不维护第二份模块名单。
    #[test]
    fn 图里有而_capabilities_里没有的就是库() {
        let modules = [module("sessions", Some("0.8.0"), true, &["GET /v1/sessions"])];
        let rel = relations();
        let index = Index::build(&modules, Some(&rel));

        assert_eq!(index.kind("sessions"), Some(Kind::Module));
        assert_eq!(index.kind("durable"), Some(Kind::Lib));
        assert_eq!(index.kind("env"), Some(Kind::Lib), "env 在 caps 里才是模块；这份 caps 没有它");
        assert_eq!(index.kind("nope"), None);
        assert!(index.libs().contains(&"wire".to_string()));
    }

    /// 段内顺序：被依赖多的在前，同数按名字 —— 同样的图给同样的顺序。
    #[test]
    fn 库按被依赖数降序_同数按名字() {
        let modules = [module("sessions", Some("0.8.0"), true, &[])];
        let rel = relations();
        let index = Index::build(&modules, Some(&rel));
        // durable 被 3 个依赖（sessions / Trigger_Core / combos）；ai、wire 各 1；
        // 剩下的 0 个，按名字排 —— 字母序里大写 T 在小写前面。
        // AboutME 被 server 依赖 1 次，但它是未登记，不进库这段。
        assert_eq!(
            index.libs(),
            ["durable", "ai", "wire", "Trigger_Core", "combos", "env", "fixture", "server"],
        );
    }

    /// 未登记的单独一段，不混进库 —— 「库」这个标签对它是假的。
    #[test]
    fn 未登记的插件不算库() {
        let modules: [nous_engine::Module; 0] = [];
        let index = Index::build(&modules, Some(&relations()));
        assert_eq!(index.kind("AboutME_ModuleRelationshipNetwork"), Some(Kind::Unregistered));
        assert_eq!(index.unregistered(), ["AboutME_ModuleRelationshipNetwork".to_string()]);
        assert!(!index.libs().contains(&"AboutME_ModuleRelationshipNetwork".to_string()));
    }

    /// 引擎哪天把它登记进 capabilities，这条补账就自动失效（表压不过「在 caps 里」）。
    #[test]
    fn 补登记之后未登记自动变成模块() {
        let modules = [module(
            "AboutME_ModuleRelationshipNetwork",
            Some("0.1.0"),
            true,
            &["GET /v1/modules/graph"],
        )];
        let index = Index::build(&modules, Some(&relations()));
        assert_eq!(index.kind("AboutME_ModuleRelationshipNetwork"), Some(Kind::Module));
        assert!(index.unregistered().is_empty());
    }

    /// 图没拉到（老引擎 404）就只剩模块那一段，库这段不出现 —— 不拿空冒充「没有库」。
    #[test]
    fn 没有图就没有库这一段() {
        let modules = [module("sessions", Some("0.8.0"), true, &[])];
        let index = Index::build(&modules, None);
        assert!(index.libs().is_empty());
        assert!(index.unregistered().is_empty());
        assert_eq!(index.kind("sessions"), Some(Kind::Module));
    }

    /// 模块行不变（#19 的 `标签 值`），库行不写「0 端点」。
    #[test]
    fn 行文案_模块照旧_库不写数字零() {
        let m = module("sessions", Some("0.8.0"), true, &["GET /v1/sessions", "POST /v1/sessions"]);
        assert_eq!(
            row_meta(&View::module(&m, "0.1.41")),
            RowMeta {
                top_label: "v0.8.0 状态".into(),
                top_value: Some("启用".into()),
                bottom: "2 端点".into(),
            }
        );
        // 老引擎不发 version：模块回落到 caps.server.version（库不许这么借）。
        let none = module("device", None, false, &[]);
        let meta = row_meta(&View::module(&none, "0.1.41"));
        assert_eq!(meta.top_label, "v0.1.41 状态");
        assert_eq!(meta.top_value.as_deref(), Some("未启用"));

        let lib = View::graph_only("durable", Kind::Lib, Some("nous-durable"));
        let meta = row_meta(&lib);
        assert_eq!(meta.top_label, "库");
        assert_eq!(meta.top_value, None, "库没有状态，不许显示「未启用」");
        assert_eq!(meta.bottom, "零端点");
        assert!(!meta.bottom.contains("0 端点"), "票面不许写 0 端点");
    }

    /// 库的详情不许借用引擎版本冒充版本，也不许把 caps 的 crate 贴上去。
    #[test]
    fn 库的详情字段缺就是缺() {
        let lib = View::graph_only("durable", Kind::Lib, Some("nous-durable"));
        assert_eq!(lib.kind, Kind::Lib);
        assert_eq!(lib.version, None);
        assert_eq!(lib.enabled, None, "不在 capabilities 里就没有运行时状态");
        assert_eq!(lib.layer, None);
        assert_eq!(lib.required, None);
        assert_eq!(lib.type_field, Some("依赖（库）"));
        assert_eq!(lib.crate_name, Some("nous-durable"), "包名从边的 via 反推，是硬事实");
        assert!(lib.endpoints.is_empty());

        // 没有入边的库（没人依赖它）连包名都没有：给 None，不猜。
        let orphan = View::graph_only("fixture", Kind::Lib, None);
        assert_eq!(orphan.crate_name, None);
    }

    /// 一张只带 sessions 的 capabilities（老引擎不发 version）。
    fn caps() -> nous_engine::Capabilities {
        serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.41" },
            "modules": [{
                "name": "sessions", "enabled": true, "endpoints": ["GET /v1/sessions"]
            }],
            "runtime": { "provider": { "kind": "real" } }
        }))
        .unwrap()
    }

    /// `Index::view` 是详情唯一入口：模块去 caps 里找自己那一行，库只拿名字与包名，
    /// 两者都拿不到回落 `None`（UI 选第一条模块）。
    #[test]
    fn view_按名字给出详情原料() {
        let caps = caps();
        let rel = relations();
        let index = Index::build(&caps.modules, Some(&rel));

        let m = index.view("sessions", &caps).unwrap();
        assert_eq!(m.kind, Kind::Module);
        assert_eq!(m.version, Some("0.1.41"), "模块的版本可以回落到引擎版本");
        assert_eq!(m.endpoints.len(), 1);

        let lib = index.view("durable", &caps).unwrap();
        assert_eq!(lib.kind, Kind::Lib);
        assert_eq!(lib.version, None, "库不许借 caps.server.version 冒充自己的版本");
        assert_eq!(lib.crate_name, Some("nous-durable"));
        assert!(lib.endpoints.is_empty());

        assert!(index.view("不存在", &caps).is_none());
    }

    /// 「读文档」只给模块：库不发布 README，未登记的模块引擎不认识。
    #[test]
    fn 读文档只给模块() {
        let caps = caps();
        assert!(View::module(&caps.modules[0], "0.1.41").has_readme());
        assert!(!View::graph_only("durable", Kind::Lib, None).has_readme());
        assert!(!View::graph_only("AboutME_ModuleRelationshipNetwork", Kind::Unregistered, None).has_readme());
    }

    /// `graph_only` 拿到 Module 也只会画成库：调用方别指望这里能把库升格成模块。
    #[test]
    fn graph_only_不会冒充模块() {
        assert_eq!(
            View::graph_only("durable", Kind::Module, None).kind,
            Kind::Lib
        );
        assert_eq!(
            View::graph_only("AboutME_ModuleRelationshipNetwork", Kind::Unregistered, None).kind,
            Kind::Unregistered
        );
    }
}