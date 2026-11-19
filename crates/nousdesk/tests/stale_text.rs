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
    const BANNED: &[&str] = &["%APPDATA%", "__nous/resolve", "d6s4v1", "/api/"];
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
