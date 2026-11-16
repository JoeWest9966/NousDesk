//! 行级差异：给触发器代码页的「差异」面板用。
//!
//! 只做一件事 —— 把两段文本对齐成 `Same / Add / Del` 三色行，带新旧行号。
//! 不做单词级、不做 git 的启发式位移（`--histogram` 那种），因为这里比的是
//! 「磁盘上的旧文件 vs 编辑框里的新文件」，同一份文件的骨架几乎不变，
//! 纯 LCS 已经够准；真遇到大改也只会多几段红的绿的，不会错位。

/// 一行是什么。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Same,
    Add,
    Del,
}

#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub kind: Kind,
    /// 旧文件里的行号（从 1 起）；`Add` 没有。
    pub old_no: Option<usize>,
    /// 新文件里的行号；`Del` 没有。
    pub new_no: Option<usize>,
    pub text: String,
}

/// 两边都抽成「行」的视图。末尾换行不算一行（和编辑器、git 一致）。
fn lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return vec![];
    }
    let mut v: Vec<&str> = text.split('\n').collect();
    if v.last() == Some(&"") {
        v.pop();
    }
    v
}

/// 超过这个规模就退化成「旧的全删、新的全加」。
///
/// LCS 是 O(n·m) 内存；一个几千行的 `nous.json` 会吃掉几十 MB，不值得。
/// 触发器的代码文件基本都在千行以内，真超了宁可给一份「整块替换」也不卡界面。
const MAX_LCS: usize = 3000;

pub(crate) fn diff(old: &str, new: &str) -> Vec<Line> {
    let a = lines(old);
    let b = lines(new);
    let (n, m) = (a.len(), b.len());

    if n == 0 || m == 0 || n > MAX_LCS || m > MAX_LCS || n.saturating_mul(m) > 4_000_000 {
        return naive(&a, &b, n, m);
    }

    // lcs[i][j] = a[i..] 与 b[j..] 的最长公共子序列长度
    let w = m + 1;
    let mut lcs = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * w + j] = if a[i] == b[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }

    let mut out = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(Line {
                kind: Kind::Same,
                old_no: Some(i + 1),
                new_no: Some(j + 1),
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * w + j] >= lcs[i * w + j + 1] {
            out.push(Line {
                kind: Kind::Del,
                old_no: Some(i + 1),
                new_no: None,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            out.push(Line {
                kind: Kind::Add,
                old_no: None,
                new_no: Some(j + 1),
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    while i < n {
        out.push(Line {
            kind: Kind::Del,
            old_no: Some(i + 1),
            new_no: None,
            text: a[i].to_string(),
        });
        i += 1;
    }
    while j < m {
        out.push(Line {
            kind: Kind::Add,
            old_no: None,
            new_no: Some(j + 1),
            text: b[j].to_string(),
        });
        j += 1;
    }
    out
}

fn naive(a: &[&str], b: &[&str], n: usize, m: usize) -> Vec<Line> {
    let mut out = Vec::with_capacity(n + m);
    for (i, line) in a.iter().enumerate() {
        out.push(Line {
            kind: Kind::Del,
            old_no: Some(i + 1),
            new_no: None,
            text: line.to_string(),
        });
    }
    for (j, line) in b.iter().enumerate() {
        out.push(Line {
            kind: Kind::Add,
            old_no: None,
            new_no: Some(j + 1),
            text: line.to_string(),
        });
    }
    out
}

/// 面板头上那行 `+3 −1 · 2 处折叠` 里的两个数。
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stat {
    pub added: usize,
    pub removed: usize,
}

pub(crate) fn stat(lines: &[Line]) -> Stat {
    let mut s = Stat::default();
    for l in lines {
        match l.kind {
            Kind::Add => s.added += 1,
            Kind::Del => s.removed += 1,
            Kind::Same => {}
        }
    }
    s
}

/// 一行渲染条目：要么是真的一行，要么是一段被折叠的「未变」。
#[derive(Clone, Debug)]
pub(crate) enum Row {
    Line(usize),
    /// 折叠掉 `count` 行，`start` 是这个折叠块在 `lines` 里的起始下标（做展开的 key）。
    Fold { start: usize, count: usize },
}

/// 连续未变超过 `keep*2+1` 行时才折叠：留头留尾各 `keep` 行当上下文。
pub(crate) fn rows(lines: &[Line], keep: usize, expanded: &std::collections::HashSet<usize>) -> Vec<Row> {
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind != Kind::Same {
            out.push(Row::Line(i));
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && lines[i].kind == Kind::Same {
            i += 1;
        }
        let run = i - start;
        // 头尾各留 keep 行；中间这段才折
        if run > keep * 2 + 1 && !expanded.contains(&start) {
            for k in 0..keep {
                out.push(Row::Line(start + k));
            }
            out.push(Row::Fold {
                start,
                count: run - keep * 2,
            });
            for k in (start + run - keep)..(start + run) {
                out.push(Row::Line(k));
            }
        } else {
            for k in start..(start + run) {
                out.push(Row::Line(k));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn kinds(v: &[Line]) -> Vec<Kind> {
        v.iter().map(|l| l.kind).collect()
    }

    #[test]
    fn identical_is_all_same() {
        let d = diff("a\nb\nc\n", "a\nb\nc\n");
        assert_eq!(kinds(&d), vec![Kind::Same; 3]);
        assert_eq!(stat(&d), Stat { added: 0, removed: 0 });
    }

    #[test]
    fn empty_new_file_removes_everything() {
        let d = diff("a\nb\n", "");
        assert_eq!(kinds(&d), vec![Kind::Del, Kind::Del]);
        assert_eq!(stat(&d), Stat { added: 0, removed: 2 });
        assert_eq!(d[0].old_no, Some(1));
        assert_eq!(d[0].new_no, None);
    }

    #[test]
    fn empty_old_file_adds_everything() {
        let d = diff("", "x\ny\nz");
        assert_eq!(kinds(&d), vec![Kind::Add; 3]);
        assert_eq!(d[2].new_no, Some(3));
    }

    #[test]
    fn changed_line_is_del_then_add() {
        let d = diff("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(
            kinds(&d),
            vec![Kind::Same, Kind::Del, Kind::Add, Kind::Same]
        );
        assert_eq!(stat(&d), Stat { added: 1, removed: 1 });
        // 删的那行带旧号、加的那行带新号，另一边是空的
        assert_eq!((d[1].old_no, d[1].new_no), (Some(2), None));
        assert_eq!((d[2].old_no, d[2].new_no), (None, Some(2)));
        assert_eq!(d[2].text, "B");
    }

    #[test]
    fn pure_insert_keeps_context_same() {
        let d = diff("a\nc\n", "a\nb\nc\n");
        assert_eq!(kinds(&d), vec![Kind::Same, Kind::Add, Kind::Same]);
        assert_eq!(d[1].text, "b");
        assert_eq!(d[1].new_no, Some(2));
    }

    #[test]
    fn trailing_newline_is_not_a_line() {
        assert_eq!(kinds(&diff("a\n", "a")), vec![Kind::Same]);
        assert_eq!(kinds(&diff("a", "a\n")), vec![Kind::Same]);
    }

    #[test]
    fn long_unchanged_run_folds_with_keep() {
        let old = (0..40).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n");
        let new = format!("{old}\nextra");
        let d = diff(&old, &new);
        let r = rows(&d, 2, &HashSet::new());
        let fold = r
            .iter()
            .find_map(|x| match x {
                Row::Fold { count, .. } => Some(*count),
                _ => None,
            })
            .expect("该折一段");
        // 41 行未变总数：40 旧 + 1？实际 40 行全同 + 尾部 extra 是 Add
        assert_eq!(fold, 40 - 4);
        // 展开后不再有折叠
        let mut open = HashSet::new();
        open.insert(0usize);
        assert!(rows(&d, 2, &open).iter().all(|x| matches!(x, Row::Line(_))));
    }

    #[test]
    fn rows_keep_short_runs_unfolded() {
        let d = diff("a\nb\nc\nd\ne\n", "a\nb\nX\nd\ne\n");
        let r = rows(&d, 2, &HashSet::new());
        assert!(r.iter().all(|x| matches!(x, Row::Line(_))));
    }
}
