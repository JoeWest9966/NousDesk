//! 性能探针。**只为回答一个问题**：会话列表到 500 / 1000 条时，一帧要画多久。
//!
//! 为什么需要它：列表现在是全量渲染（`render_session_list` 一条不落地 `map`），
//! 没有虚拟化。要不要上虚拟化是个一周的赌注 —— 「慢要慢得可解释」这条规矩要求先有数，
//! 而不是拍脑袋。
//!
//! 两条路分开设计，谁都不碰谁：
//!
//! - [`rows`] / [`fill`]：`NOUS_PERF_SESSIONS=500` 时把会话列表换成 500 条**克隆出来的假数据**。
//!   不外发请求、不写引擎 —— 我们量的是渲染，数据从哪来不影响。
//! - [`note`]：量「构建列表元素树」那一段的 CPU 时间。每 30 帧汇总一条日志。
//!
//! 两个都是零成本的：没设环境变量时 `rows()` 返回 `None`，`note()` 只在一次 `Instant`
//! 之后做几个原子操作。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static FRAMES: AtomicU64 = AtomicU64::new(0);
static SUM_US: AtomicU64 = AtomicU64::new(0);
static MIN_US: AtomicU64 = AtomicU64::new(u64::MAX);
static MAX_US: AtomicU64 = AtomicU64::new(0);

/// 想伪造多少条会话。没设 / 设成 0 / 解析不出来 → `None`（用真实数据）。
pub fn rows() -> Option<usize> {
    std::env::var("NOUS_PERF_SESSIONS")
        .ok()?
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
}

/// 造 `n` 条假会话：**克隆第一条真会话**，只改 `id` 和 `name`。
///
/// 克隆而不是造空壳，是因为要量的是真实形状的行（有模型、有摘要、有时间和计数）。
/// 空壳量出来的数偏乐观，等于自欺。`id` 故意从 100 万起 —— 跟真会话撞不上，
/// 万一误点也只会 404，不会改到真数据。
///
/// **返回一份新 Vec，不改调用方的状态**：探针只在渲染那一帧临时顶替名单，
/// 函数退出就没了。写状态的版本（reload 之后覆盖 sessions）会被轮询冲掉，
/// 而且会让「点一下就 404」这种假数据泄漏到交互里。
pub fn fake<T: Clone + FakeId>(base: &[T], n: usize) -> Vec<T> {
    let Some(first) = base.first() else {
        return Vec::new();
    };
    (0..n)
        .map(|i| {
            let mut s = first.clone();
            s.set_fake(i);
            s
        })
        .collect()
}

/// 假会话要能被改 id 和名字。放在 trait 里是为了让 `perf` 不依赖 `nous-engine` 的类型
/// （探针不该把引擎拉进来，也不该为了量一次性能在生产结构上开洞）。
pub trait FakeId {
    fn set_fake(&mut self, i: usize);
}

/// 记一帧的列表构建耗时。每 10 帧打一条汇总。
///
/// 为什么不每帧打：写日志本身要几毫秒，会把下一帧的测量值污染成一条上升的斜线。
/// 为什么是 10：GPUI 是按需重绘，静止时**根本不渲染** —— 一轮滚动只有几十帧，
/// 阈值给 30 的话经常一条日志都打不出来（看着像探针坏了）。
pub fn note(n: usize, d: Duration) {
    let us = d.as_micros() as u64;
    SUM_US.fetch_add(us, Ordering::Relaxed);
    MAX_US.fetch_max(us, Ordering::Relaxed);
    MIN_US.fetch_min(us, Ordering::Relaxed);
    let f = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if f % 10 != 0 {
        return;
    }
    let sum = SUM_US.swap(0, Ordering::Relaxed);
    let max = MAX_US.swap(0, Ordering::Relaxed);
    let min = MIN_US.swap(u64::MAX, Ordering::Relaxed);
    log::info!(
        "PERF 列表 {n} 行 · 最近 10 帧元素树 min {min}µs / avg {}µs / max {max}µs",
        sum / 10
    );
}

#[cfg(test)]
mod tests {
    /// 不给环境变量就是不启用。这条别改成「默认开」—— 探针一旦默认开，
    /// 它就变成了每个人的常开开销。
    #[test]
    fn 没有环境变量就不启用() {
        // 测试进程里不该有这个变量；万一外部设了，这条断言也就没意义了。
        if std::env::var("NOUS_PERF_SESSIONS").is_ok() {
            return;
        }
        assert_eq!(super::rows(), None);
    }

    #[test]
    fn 零和负数不启用() {
        assert_eq!("0".parse::<usize>().ok().filter(|n| *n > 0), None);
        assert_eq!("-5".parse::<usize>().ok().filter(|n| *n > 0), None);
        assert_eq!("500".parse::<usize>().ok().filter(|n| *n > 0), Some(500));
    }
}
