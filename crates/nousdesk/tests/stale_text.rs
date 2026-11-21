//! 判据 2 的仓库级哨兵：过期机制描述不许再进源码。
//! 文档 / demo / 测试夹具里的历史记录不算（它们是记录，不是代码里的说法）。

use std::path::Path;

const ROOTS: &[&str] = &[
    concat!(env!("CARGO_MANIFEST_DIR"), "/src"),
    concat!(env!("CARGO_MANIFEST_DIR"), "/../nous-engine/src"),
];

fn source_files() -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for root in ROOTS {
        walk(Path::new(root), &mut out);
    }
    out
}

#[test]
fn no_stale_paths_in_source() {
    // 字面量拆开写：判据 2 要求旧面板 REST 路径字面量零命中，哨兵自己不能犯规。
    const BANNED: &[&str] = &["%APPDATA%", "__nous/resolve", "d6s4v1", concat!("/", "api/")];
    let mut hits = Vec::new();
    for path in source_files() {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (i, line) in text.lines().enumerate() {
            for needle in BANNED {
                if line.contains(needle) {
                    hits.push(format!("{}:{}: {needle} —— {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }
    assert!(hits.is_empty(), "过期路径描述还在源码里：\n{}", hits.join("\n"));
}

/// 判据 3：三态（空 / 在等 / 坏了）的文案不许退回空话。
///
/// 起因：这四个页面曾各写各的 —— `resources.rs` 写「没有数据」、`templates.rs` 写「还没数据」、
/// 会话列表干脆什么都没有。用户分不出「空」「在等」「坏了」，因为三种状态长得一样、说的也一样。
/// 现在统一走 `states.rs`，这条哨兵负责**别让空话爬回来**。
///
/// 只查代码行：注释里可以写「不要写加载中」，那是在说规矩，不是在说给人看的话。
#[test]
fn no_filler_state_words_in_code() {
    // 拆开写，免得哨兵自己被命中。
    let banned = [
        format!("没有{}", "数据"),
        format!("暂{}", "无"),
        format!("加载{}", "中"),
        "敬请期待".to_string(),
        "Coming soon".to_string(),
    ];
    let mut hits = Vec::new();
    for path in source_files() {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (i, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for needle in &banned {
                if line.contains(needle.as_str()) {
                    hits.push(format!(
                        "{}:{}: {needle} —— {}",
                        path.display(),
                        i + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "三态文案退回空话了（该说清在等什么 / 为什么空 / 错在哪）：\n{}",
        hits.join("\n")
    );
}
