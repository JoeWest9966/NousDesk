//! 类别路径 → 缩进树（带计数）。触发器和模板共用。
//!
//! 这两个页面各写过一份：模板那边是 `facet_tree`，触发器那边在 `trigger_cats()` 里
//! 手搓了一个 `BTreeMap` 计数 + `cats.sort()`。**计数语义一模一样**（同一条目对它的每个
//! 祖先各贡献一次），而排序不一样（模板按词表、父在子前；触发器纯字母序）——
//! 这种「一样的东西写两遍、只有一处悄悄不一样」就是要合掉的东西：
//! 改一处永远漏一处，而漏的那一处不会有任何报错。

/// 树里的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatNode {
    pub path: String,
    pub label: String,
    /// 层级，0 = 顶层。渲染时按它缩进。
    pub depth: usize,
    pub count: usize,
    pub has_children: bool,
}

/// `a/b/c` → `["a", "a/b", "a/b/c"]`。
pub(crate) fn ancestors(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    (1..=parts.len()).map(|n| parts[..n].join("/")).collect()
}

/// `a` 是不是 `root` 或它的后代。
pub(crate) fn is_under(value: &str, root: &str) -> bool {
    value == root || value.starts_with(&format!("{root}/"))
}

pub(crate) fn depth_of(path: &str) -> usize {
    path.split('/')
        .filter(|s| !s.is_empty())
        .count()
        .saturating_sub(1)
}

pub(crate) fn last_seg(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// 扁平的类别路径 + 每个条目的类别 → 缩进树（带计数）。
///
/// **顺序跟着调用方给的词表走**：`categories` 由引擎排好（父在子前、逐段 zh-CN），
/// 这里只补缺 —— 条目上挂着、词表里没有的路径。补进来的按「浅的在前、同层按字面」排，
/// 只为稳定，不假装那是 zh-CN 排序（那样会和词表那部分自相矛盾）。
///
/// 计数含后代、**同一个条目按路径去重**：一条挂了 `工程` 和 `工程/审查`，`工程` 只算它一次。
pub(crate) fn build_tree<'a>(
    categories: &[String],
    items: impl Iterator<Item = &'a [String]>,
) -> Vec<CatNode> {
    let mut order: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    for c in categories {
        for a in ancestors(c) {
            if seen.insert(a.clone()) {
                order.push(a);
            }
        }
    }

    // 词表里没有、但条目上挂着的路径（词表没同步时不该整棵树空掉 / 条目消失）。
    let mut extra: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    for cats in items {
        let mut mine: std::collections::HashSet<String> = Default::default();
        for c in cats {
            for a in ancestors(c) {
                if !seen.contains(&a) && !extra.contains(&a) {
                    extra.push(a.clone());
                }
                mine.insert(a);
            }
        }
        for a in mine {
            *counts.entry(a).or_default() += 1;
        }
    }
    extra.sort_by(|a, b| depth_of(a).cmp(&depth_of(b)).then_with(|| a.cmp(b)));
    order.extend(extra);

    order
        .iter()
        .map(|path| CatNode {
            path: path.clone(),
            label: last_seg(path),
            depth: depth_of(path),
            count: counts.get(path).copied().unwrap_or(0),
            has_children: order.iter().any(|o| o != path && is_under(o, path)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CatNode, ancestors, build_tree, is_under};

    fn cats(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ancestors_walks_down_every_level() {
        assert_eq!(ancestors("a/b/c"), ["a", "a/b", "a/b/c"]);
        assert_eq!(ancestors("a"), ["a"]);
        assert!(ancestors("").is_empty());
        assert!(ancestors("/").is_empty());
    }

    #[test]
    fn is_under_matches_whole_segments_only() {
        assert!(is_under("工程/审查", "工程"));
        assert!(is_under("工程", "工程"));
        // 「工程x」不是「工程」的子类别 —— 前缀不是这么比的。
        assert!(!is_under("工程x", "工程"));
    }

    /// 词表里有、条目没挂的类别也得出现（建了就得看得见），计数 0。
    #[test]
    fn vocabulary_wins_even_with_zero_count() {
        let tree = build_tree(&cats(&["写作", "写作/周报"]), std::iter::empty());
        assert_eq!(
            tree.iter().map(|n| (n.path.as_str(), n.count)).collect::<Vec<_>>(),
            [("写作", 0), ("写作/周报", 0)]
        );
        assert!(tree[0].has_children);
        assert_eq!(tree[1].depth, 1);
        assert_eq!(tree[1].label, "周报");
    }

    /// 词表里没有、条目上挂着的路径要补进来，且父在子前。
    #[test]
    fn paths_on_items_fill_in_the_gaps() {
        let item = cats(&["研发/调试"]);
        let tree = build_tree(&[], std::iter::once(item.as_slice()));
        assert_eq!(
            tree.iter().map(|n| n.path.as_str()).collect::<Vec<_>>(),
            ["研发", "研发/调试"]
        );
        assert_eq!(tree[0].count, 1, "父类别带子类别");
        assert_eq!(tree[1].count, 1);
    }

    /// 一条同时挂 `工程` 和 `工程/审查`：`工程` 只算它一次，不是两次。
    #[test]
    fn one_item_counts_once_per_ancestor() {
        let a = cats(&["工程", "工程/审查"]);
        let b = cats(&["工程/审查"]);
        let tree = build_tree(&[], [a.as_slice(), b.as_slice()].into_iter());
        let at = |p: &str| tree.iter().find(|n: &&CatNode| n.path == p).unwrap().count;
        assert_eq!(at("工程"), 2);
        assert_eq!(at("工程/审查"), 2);
    }
}
