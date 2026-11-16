//! 仓状态那句话 —— 触发器和模板两个页面共用。
//!
//! 原来只有触发器有这一段，模板页左栏底部自己写了一句「仓干净 · abc1234」，
//! 而 `ahead` / `behind` 两个字段根本没进判断。j1900 推不回 GitHub 之后那边攒了一堆
//! 没推的提交，界面照旧说「仓干净」—— 这就是这条被抽出来的原因：
//! **界面说错话比界面少一句话糟得多。**

/// 同步芯片的颜色档位。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Tone {
    /// 干净。
    Ok,
    /// 有东西要处理（推 / 发布 / 拉）。
    Warn,
    /// 这一段本身没事，但仓库里有别的改动。
    Idle,
}

/// 仓相对远端的三件事。抽成一个中性结构，触发器和模板各自把自己的 repo 映射进来 ——
/// 两个 API 的形状本来就一样（`ahead` / `behind` / `dirty`），但类型不同。
#[derive(Clone, Copy, Debug)]
pub(crate) struct SyncFacts {
    pub ahead: i64,
    pub behind: i64,
    pub dirty: bool,
}

impl SyncFacts {
    pub(crate) fn of(ahead: i64, behind: i64, dirty: bool) -> Self {
        Self {
            ahead,
            behind,
            dirty,
        }
    }
}

/// 同步芯片该写什么。
///
/// 顺序按「先解决哪个」排：落后 → 没推 → 有改动 → 未发布 → 仓库脏 → 干净。
/// `ahead > 0` 排在 `dirty > 0` 前面，是因为前者只要推一下就完事，
/// 而后者还得先发布；反过来会出现「改了 2 个」盖住「3 个提交没推」的情况，
/// 人就一直看不到自己其实一直没推上去。两个数都非零时并排写，不吞信息。
pub(crate) fn sync_state(
    repo: Option<SyncFacts>,
    dirty: usize,
    unpublished: usize,
) -> (String, Tone) {
    let ahead = repo.map(|r| r.ahead).unwrap_or(0);
    let behind = repo.map(|r| r.behind).unwrap_or(0);
    let repo_dirty = repo.map(|r| r.dirty).unwrap_or(false);
    if behind > 0 {
        return (format!("远端有新提交 · 落后 {behind}"), Tone::Warn);
    }
    if ahead > 0 {
        let mut text = format!("{ahead} 个提交没推");
        if dirty > 0 {
            text.push_str(&format!(" · {dirty} 个有改动"));
        }
        return (text, Tone::Warn);
    }
    if dirty > 0 {
        let mut text = format!("{dirty} 个有改动");
        if unpublished > 0 {
            text.push_str(&format!(" · {unpublished} 个未发布"));
        }
        return (text, Tone::Warn);
    }
    if unpublished > 0 {
        return (format!("{unpublished} 个未发布"), Tone::Warn);
    }
    if repo_dirty {
        // 走到这儿说明「这一段的条目」没有改动，但仓目录里有没提交的东西。
        // 不写「别的」—— 模板段没有逐条的改动标记，它只能看到整个段目录脏没脏，
        // 说「别的」在那一边就不成立了。
        return ("仓里有没提交的改动".to_string(), Tone::Idle);
    }
    ("已同步".to_string(), Tone::Ok)
}

#[cfg(test)]
mod tests {
    use super::{SyncFacts, Tone, sync_state};

    fn repo(ahead: i64, behind: i64, dirty: bool) -> SyncFacts {
        SyncFacts::of(ahead, behind, dirty)
    }

    #[test]
    fn clean_repo_says_synced() {
        let (text, tone) = sync_state(Some(repo(0, 0, false)), 0, 0);
        assert_eq!(text, "已同步");
        assert_eq!(tone, Tone::Ok);
    }

    /// 这条是整件事的起点：有提交没推，界面以前照样写「已同步」。
    #[test]
    fn ahead_beats_everything_but_behind() {
        let (text, tone) = sync_state(Some(repo(1, 0, false)), 0, 0);
        assert_eq!(text, "1 个提交没推");
        assert_eq!(tone, Tone::Warn);
    }

    #[test]
    fn ahead_and_dirty_are_both_written() {
        let (text, _) = sync_state(Some(repo(3, 0, false)), 2, 0);
        assert_eq!(text, "3 个提交没推 · 2 个有改动");
    }

    /// 落后永远压过领先：先拉再说，不然推不上去。
    #[test]
    fn behind_wins_over_ahead() {
        let (text, tone) = sync_state(Some(repo(2, 5, false)), 9, 9);
        assert_eq!(text, "远端有新提交 · 落后 5");
        assert_eq!(tone, Tone::Warn);
    }

    #[test]
    fn dirty_mentions_unpublished() {
        let (text, _) = sync_state(Some(repo(0, 0, false)), 2, 1);
        assert_eq!(text, "2 个有改动 · 1 个未发布");
    }

    #[test]
    fn unpublished_counts_on_its_own() {
        let (text, _) = sync_state(Some(repo(0, 0, false)), 0, 3);
        assert_eq!(text, "3 个未发布");
    }

    /// 仓脏但不是我们这一段的改动 —— 别报警，只是告知。
    #[test]
    fn repo_dirty_elsewhere_is_idle() {
        let (text, tone) = sync_state(Some(repo(0, 0, true)), 0, 0);
        assert_eq!(text, "仓里有没提交的改动");
        assert_eq!(tone, Tone::Idle);
    }

    /// 拿不到仓状态：不能说「已同步」，那就成了瞎保证。
    #[test]
    fn unknown_repo_falls_back_to_idle_or_clean() {
        let (text, _) = sync_state(None, 0, 0);
        assert_eq!(text, "已同步");
        let (text, tone) = sync_state(None, 0, 2);
        assert_eq!(text, "2 个未发布");
        assert_eq!(tone, Tone::Warn);
    }
}
