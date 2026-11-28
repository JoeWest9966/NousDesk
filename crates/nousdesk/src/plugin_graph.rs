//! 插件页的「关系」：模块依赖图的正反索引 + 一次拉取的状态机。
//!
//! 原料是内核 `GET /v1/modules/graph`（`AboutME_ModuleRelationshipNetwork` 插件把各
//! `plugin.toml [requires]` 投影成图）。这一层只做三件事，**不碰 UI、不碰 HTTP**
//! （HTTP 在 `EngineClient::module_graph`）：
//!
//! 1. **索引**：引擎只发一次 `from → to` 的边，正查（它依赖谁）反查（谁依赖它）都在这里建；
//!    反查不能指望引擎再发一份 —— 那就是第二个真相。
//! 2. **诚实**：边的两端必须都在 `nodes` 里；对不上的边计数留着（`dangling_edges`），
//!    不静默补、也不静默丢。`unmapped`（被依赖但没有对应模块的包）原样透给 UI。
//! 3. **状态机**：只拉一次；老引擎 404 是「没有这条端点」，跟「加载失败」分开说 ——
//!    没有图就整块不显示，不画一张「谁都不依赖谁」的假图。
//!
//! 不算的东西：不做传递闭包、不做环检测（内核断言无环，这里只做一跳）。真出现环也只是
//! 一跳列表里各出现一次，不会递归。

use std::collections::{BTreeMap, BTreeSet};

use nous_engine::{EngineError, ModuleGraph};

/// 一条边的一侧：对面那个模块，以及「为什么有这条边」的包名。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub module: String,
    /// `plugin.toml [requires]` 的键（包名）。老引擎可能给空串。
    pub via: String,
}

/// 一张建好索引的图。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relations {
    /// 生成这张图的引擎 release（构建期内嵌那份清单对应的版本）。
    pub generated_for: String,
    pub nodes: BTreeSet<String>,
    pub edges: usize,
    /// 被依赖、但在 `nodes` 里找不到对应模块的包名（引擎原样给的清单）。
    pub unmapped: Vec<String>,
    /// 两端有一端不在 `nodes` 里的边数。>0 说明引擎的图自相矛盾，UI 不该假装没看见。
    pub dangling_edges: usize,
    out: BTreeMap<String, Vec<Edge>>,
    inn: BTreeMap<String, Vec<Edge>>,
    /// 节点 → 提供它的包名。**不是新数据**：`via` 本来就是「被用到的那一端是哪个包」，
    /// 所以任何一条入边都能反推出目标节点的包名。库的「来源 crate」靠它，
    /// 因为 `capabilities` 里没有库的条目。没人依赖的节点没有入边 → 这里没有它。
    packages: BTreeMap<String, String>,
}

impl Relations {
    /// 建正反索引。同一对 `(from, to)` 只留一条（`via` 取字典序最小那个，
    /// 保证「同样的图给同样的列表」，快照比对不会因为注册顺序抖）。
    pub fn build(graph: &ModuleGraph) -> Self {
        let nodes: BTreeSet<String> = graph.nodes.iter().cloned().collect();
        let mut out: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
        let mut inn: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
        let mut dangling_edges = 0usize;
        let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
        let mut packages: BTreeMap<String, String> = BTreeMap::new();

        for edge in &graph.edges {
            // 自环没有意义（`cargo` 也不许），真出现就是清单写错了 —— 不索引，但算悬空。
            if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) || edge.from == edge.to {
                dangling_edges += 1;
                continue;
            }
            if !seen.insert((edge.from.as_str(), edge.to.as_str())) {
                continue;
            }
            let via = edge.via.clone();
            if !via.is_empty() {
                // 同一个目标只会由一个包提供；真出现两个 `via`，取字典序最小的那个，
                // 保证同样的图给同样的包名（快照比对不抖）。
                packages
                    .entry(edge.to.clone())
                    .and_modify(|cur| {
                        if via < *cur {
                            *cur = via.clone();
                        }
                    })
                    .or_insert_with(|| via.clone());
            }
            out.entry(edge.from.clone()).or_default().push(Edge {
                module: edge.to.clone(),
                via: via.clone(),
            });
            inn.entry(edge.to.clone()).or_default().push(Edge {
                module: edge.from.clone(),
                via,
            });
        }
        for list in out.values_mut().chain(inn.values_mut()) {
            list.sort();
            list.dedup();
        }

        let mut unmapped = graph.unmapped.clone();
        unmapped.sort();
        unmapped.dedup();

        Relations {
            generated_for: graph.generated_for.clone(),
            edges: seen.len(),
            nodes,
            unmapped,
            dangling_edges,
            out,
            inn,
            packages,
        }
    }

    /// 这个模块在这张图里吗。不在 = 图比 `capabilities` 旧（或新），UI 要说实话。
    pub fn contains(&self, name: &str) -> bool {
        self.nodes.contains(name)
    }

    /// 它依赖谁（按模块名排序）。
    pub fn deps(&self, name: &str) -> &[Edge] {
        self.out.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 谁依赖它（按模块名排序）。
    pub fn dependents(&self, name: &str) -> &[Edge] {
        self.inn.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 这个节点是哪个包提供的（从入边的 `via` 反推）。
    /// 只对「被至少一个模块依赖」的节点有值 —— 没人依赖的库拿不到，UI 显示 `—`。
    pub fn package_of(&self, name: &str) -> Option<&str> {
        self.packages.get(name).map(String::as_str)
    }
}

/// 关系块的状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum State {
    /// 还没请求过。
    #[default]
    Idle,
    /// 请求在路上。
    Loading,
    Ready(Relations),
    /// 引擎没有这条端点（0.1.41 之前的产物）：整块不显示，不拿「空」冒充「没有依赖」。
    NoEndpoint,
    /// 网络 / 5xx / 回包不像话。可以再点一次重试。
    Failed(String),
}

/// 一次拉取的状态机 + 缓存。和 `plugin_doc` 同一套规矩：只请求一次、代次过期就丢。
#[derive(Debug, Default)]
pub struct PluginGraph {
    state: State,
    epoch: u64,
}

impl PluginGraph {
    pub fn state(&self) -> &State {
        &self.state
    }

    /// 拿这份关系的入口：只有 `Idle` / `Failed` 会真去请求。
    /// 返回这次请求的代次；`None` = 已有结论（`Ready` / `NoEndpoint` / `Loading`）。
    pub fn begin(&mut self) -> Option<u64> {
        match self.state {
            State::Idle | State::Failed(_) => {}
            State::Loading | State::Ready(_) | State::NoEndpoint => return None,
        }
        self.state = State::Loading;
        self.epoch += 1;
        Some(self.epoch)
    }

    /// 写回结果。代次对不上（已经换引擎 / 重置过）就丢掉。
    pub fn finish(&mut self, epoch: u64, result: Result<ModuleGraph, EngineError>) {
        if epoch != self.epoch {
            return;
        }
        self.state = match result {
            Ok(graph) => State::Ready(Relations::build(&graph)),
            // axum 的空 404 = 路由不存在（老引擎），不是「模块不认识」。
            Err(EngineError::Engine { status: 404, .. }) => State::NoEndpoint,
            Err(e) => State::Failed(e.to_string()),
        };
    }

    /// 不走 HTTP 的失败（没连上引擎 / tokio 没起 / 任务挂了）。
    pub fn fail(&mut self, epoch: u64, message: String) {
        if epoch != self.epoch {
            return;
        }
        self.state = State::Failed(message);
    }

    /// 换引擎 / 关设置页后清空：图和引擎版本绑在一起，缓存不能跨引擎用。
    pub fn reset(&mut self) {
        self.epoch += 1;
        self.state = State::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::{PluginGraph, Relations, State};
    use nous_engine::EngineError;

    fn graph() -> nous_engine::ModuleGraph {
        serde_json::from_value(serde_json::json!({
            "generatedFor": "0.1.41",
            "nodes": ["server", "Extend", "env", "repo", "routes", "orphan"],
            "edges": [
                { "from": "server", "to": "Extend", "via": "library-extend" },
                { "from": "Extend", "to": "repo", "via": "nous-repo" },
                { "from": "Extend", "to": "env", "via": "nous-env" },
                { "from": "Extend", "to": "repo", "via": "nous-repo" },
                { "from": "repo", "to": "env", "via": "nous-env" },
                { "from": "ghost", "to": "env", "via": "nous-ghost" },
                { "from": "env", "to": "env", "via": "self" }
            ],
            "unmapped": ["nous-fixture", "aaa-extra"]
        }))
        .unwrap()
    }

    #[test]
    fn 正反索引都把对端排好序并且不重复() {
        let r = Relations::build(&graph());
        assert_eq!(r.generated_for, "0.1.41");
        assert_eq!(r.edges, 4, "去重之后 4 条真边（重复的那条 + 自环 + 悬空不算）");
        let deps: Vec<_> = r.deps("Extend").iter().map(|e| e.module.as_str()).collect();
        assert_eq!(deps, ["env", "repo"], "依赖按模块名排序");
        assert_eq!(r.deps("Extend")[1].via, "nous-repo", "via 是包名");
        let inn: Vec<_> = r.dependents("env").iter().map(|e| e.module.as_str()).collect();
        assert_eq!(inn, ["Extend", "repo"], "被依赖是反查出来的，不靠引擎再发一份");
        assert_eq!(r.dependents("Extend")[0].module, "server");
    }

    #[test]
    fn 悬空与自环计入但不进索引() {
        let r = Relations::build(&graph());
        assert_eq!(r.dangling_edges, 2, "ghost 不在 nodes、env→env 自环");
        assert!(r.deps("ghost").is_empty());
        assert!(r.dependents("env").iter().all(|e| e.module != "env"));
    }

    #[test]
    fn 不在图里的模块给空而不是猜() {
        let r = Relations::build(&graph());
        assert!(!r.contains("sessions"));
        assert!(r.deps("sessions").is_empty());
        assert!(r.dependents("sessions").is_empty());
        assert!(r.contains("orphan"), "孤立节点也是节点");
    }

    #[test]
    fn 包名从入边的_via_反推() {
        let r = Relations::build(&graph());
        // `via` 就是目标端提供的包名，入边能反推——库的「来源 crate」靠它。
        assert_eq!(r.package_of("env"), Some("nous-env"));
        assert_eq!(r.package_of("repo"), Some("nous-repo"));
        assert_eq!(r.package_of("Extend"), Some("library-extend"));
        // 没人依赖、也没有入边的节点：不知道就是不知道。
        assert_eq!(r.package_of("orphan"), None);
        // 悬空边的 `to` 不在 nodes 里 → 不算进包名表。
        assert_eq!(r.package_of("ghost"), None);
    }

    #[test]
    fn unmapped_原样透出且有序() {
        let r = Relations::build(&graph());
        assert_eq!(r.unmapped, ["aaa-extra", "nous-fixture"]);
    }

    #[test]
    fn 空图不崩() {
        let empty: nous_engine::ModuleGraph =
            serde_json::from_value(serde_json::json!({})).unwrap();
        let r = Relations::build(&empty);
        assert_eq!(r.edges, 0);
        assert!(r.generated_for.is_empty());
        assert!(r.deps("anything").is_empty());
    }

    #[test]
    fn 只请求一次_失败可以重试() {
        let mut g = PluginGraph::default();
        let epoch = g.begin().expect("第一次要请求");
        assert_eq!(g.begin(), None, "在路上不重复请求");
        g.finish(epoch, Err(EngineError::Engine {
            status: 500,
            message: "boom".into(),
            body: None,
        }));
        assert!(matches!(g.state(), State::Failed(_)));
        assert!(g.begin().is_some(), "失败之后允许重试");
        assert!(g.begin().is_none(), "重试在路上也不许再发");
    }

    #[test]
    fn 老引擎的404是没端点不是加载失败() {
        let mut g = PluginGraph::default();
        let epoch = g.begin().unwrap();
        g.finish(epoch, Err(EngineError::Engine {
            status: 404,
            message: String::new(),
            body: Some(String::new()),
        }));
        assert_eq!(*g.state(), State::NoEndpoint);
        assert!(g.begin().is_none(), "没端点就不再问第二次");
    }

    #[test]
    fn 重置之后晚到的回包不算数() {
        let mut g = PluginGraph::default();
        let epoch = g.begin().unwrap();
        g.reset();
        g.finish(epoch, Ok(graph()));
        assert_eq!(*g.state(), State::Idle, "代次过期，回包丢弃");
        assert!(g.begin().is_some(), "重置之后可以再问");
    }
}