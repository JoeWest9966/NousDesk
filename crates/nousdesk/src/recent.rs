//! 本机「哪几条会话我最近点开过」—— **只给左栏排序用**。
//!
//! 为什么需要它：引擎的 `/v1/sessions` 对一条 entry 都没有的会话不给任何时间
//! （`updatedAt` / `last` 都是 `null`，实测 149 条主线里 134 条如此），于是这 134 条
//! 之间只能按会话 id 排 —— 那是「什么时候建的」，不是「我什么时候碰过它」。
//!
//! 边界（`decisions.md` 2026-11-16「本机最近打开」那条）：
//! - 只存 `会话 id → 打开时间`。不存消息、不存标题、**不写引擎、不上报**；
//! - 引擎给了内容（`last`）就以引擎为准，这份只排在「引擎没话说」的那一堆里；
//! - 换机器就没了 —— 这是**这台机器的使用痕迹**，不是数据，跟置顶（写穿 bjes6b）
//!   不是一回事，所以两者不该混在一个文件里。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const DIR: &str = "nousdesk";
const FILE: &str = "recent.json";
/// 留多少条。149 条主线全记也就 4 KB，512 够覆盖好几轮「清空重来」，
/// 又不至于让文件无限长（点开一条就写一次盘，写的东西得有个上限）。
const KEEP: usize = 512;

/// 会话 id → 最后一次打开它的时间（unix 秒）。
#[derive(Clone, Debug, Default)]
pub struct Recent {
    map: HashMap<i64, u64>,
}

impl Recent {
    /// `~/.local/share/nousdesk/recent.json`（Windows 是 `%LOCALAPPDATA%`）。
    pub fn path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|dir| dir.join(DIR).join(FILE))
    }

    /// **任何失败都回空表**：没文件、没目录、写坏了 —— 排序退化成老样子，不能拦启动。
    pub fn load() -> Self {
        Recent::path()
            .map(|path| Recent::load_from(&path))
            .unwrap_or_default()
    }

    /// 记下「现在打开了 `id`」。已经记过就刷新时间。
    pub fn touch(&mut self, id: i64) {
        self.remember(id, now_secs());
    }

    /// 指定时间记账。测试和 `touch` 共用一条路，免得测试碰真时钟。
    pub(crate) fn remember(&mut self, id: i64, ts: u64) {
        self.map.insert(id, ts);
        self.trim();
    }

    pub fn ts(&self, id: i64) -> Option<u64> {
        self.map.get(&id).copied()
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Recent::path().ok_or_else(|| "找不到数据目录".to_string())?;
        self.save_to(&path)?;
        Ok(path)
    }

    /// 超过上限就只留最新的那些。`retain` 之后还是可能略多，无所谓 —— 下一轮会再收。
    fn trim(&mut self) {
        if self.map.len() <= KEEP {
            return;
        }
        let mut all: Vec<(i64, u64)> = self.map.iter().map(|(k, v)| (*k, *v)).collect();
        all.sort_by_key(|(_, ts)| std::cmp::Reverse(*ts));
        all.truncate(KEEP);
        self.map = all.into_iter().collect();
    }

    // ── 可测的两步（不碰真实路径 / 时钟）────────────────────────────

    pub(crate) fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Recent::default();
        };
        // 盘上是 `{"2": 1794222429, "14": …}`，值也可能是浮点（手改过 / 别的工具写的），
        // 收不下的键值单个丢掉就行，别让一行坏数据废掉整份排序。
        let Ok(raw) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text)
        else {
            return Recent::default();
        };
        let mut recent = Recent::default();
        for (key, value) in raw {
            let (Ok(id), Some(ts)) = (key.parse::<i64>(), value.as_u64()) else {
                continue;
            };
            recent.map.insert(id, ts);
        }
        recent
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("建不了目录 {}：{e}", dir.display()))?;
        }
        let map: serde_json::Map<String, serde_json::Value> = self
            .map
            .iter()
            .map(|(id, ts)| (id.to_string(), serde_json::Value::from(*ts)))
            .collect();
        let text = serde_json::to_string(&map).map_err(|e| format!("序列化失败：{e}"))?;
        std::fs::write(path, text).map_err(|e| format!("写不了 {}：{e}", path.display()))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    // 只取需要的名字：`use super::*` 会把一堆宏再展开一层（见 theme.rs 里那条注释）。
    use super::Recent;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nousdesk-recent-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join("recent.json")
    }

    #[test]
    fn 存了能读回来() {
        let path = tmp("roundtrip");
        let mut recent = Recent::default();
        recent.remember(2, 1_700_000_000);
        recent.remember(1539, 1_700_000_100);
        recent.save_to(&path).unwrap();

        let back = Recent::load_from(&path);
        assert_eq!(back.ts(2), Some(1_700_000_000));
        assert_eq!(back.ts(1539), Some(1_700_000_100));
        assert_eq!(back.ts(999), None);
    }

    #[test]
    fn 没文件或者写坏了都回空表() {
        assert!(Recent::load_from(&tmp("missing")).ts(1).is_none());

        let path = tmp("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ 这不是 json").unwrap();
        assert!(Recent::load_from(&path).ts(1).is_none());
    }

    /// 混进一行坏数据不能把整份记录废掉（手改过文件、别的工具写过都可能）。
    #[test]
    fn 坏掉的单行只丢那一条() {
        let path = tmp("mixed");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"2":1700000000,"坏键":5,"14":1.5,"25":"x"}"#).unwrap();
        let back = Recent::load_from(&path);
        assert_eq!(back.ts(2), Some(1_700_000_000));
        assert_eq!(back.ts(14), None, "浮点收不下就丢");
        assert_eq!(back.ts(25), None, "字符串收不下就丢");
    }

    /// 点开的条数没有上限的话文件会一直长；这条钉住「超出就只留最新的」。
    #[test]
    fn 超过上限只留最新的() {
        let mut recent = Recent::default();
        for i in 0..(super::KEEP as i64 + 20) {
            recent.remember(i, 1_000 + i as u64);
        }
        assert_eq!(recent.ts(0), None, "最老的被挤掉");
        assert!(recent.ts(super::KEEP as i64 + 19).is_some(), "最新的还在");
        assert_eq!(
            recent.map.len(),
            super::KEEP,
            "留下的正好是上限那么多条"
        );
    }
}
