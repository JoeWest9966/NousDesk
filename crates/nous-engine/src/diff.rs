//! Desk#11：模板编辑页的展示层文本 diff。
//!
//! 引擎没有 `pending/diff` / `history/diff` 了。「当前原文 vs 第 n 版原文」的差异
//! 由客户端算 —— 这是**展示层计算**，不是状态计算，不违反「协议是哑的」。
//! 只做行级 LCS：同一份 preset.json 的骨架几乎不变，纯 LCS 足够；大文件退化为
//! 全删全加，宁可给一份「整块替换」也不卡 UI。

/// 一行的差异类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Same,
    Add,
    Del,
}

/// 一行输出。`old_no` / `new_no` 从 1 起；`Add` 没有旧号，`Del` 没有新号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub old_no: Option<usize>,
    pub new_no: Option<usize>,
    pub text: String,
}

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

/// 超过这个规模就退化成「旧的全删、新的全加」（LCS 是 O(n·m)）。
const MAX_LCS: usize = 3000;

/// 对两段原文做行级 diff。输出顺序：从文件头到尾，Same / Del / Add 交错。
pub fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a = lines(old);
    let b = lines(new);
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n > MAX_LCS || m > MAX_LCS || n.saturating_mul(m) > 4_000_000 {
        return naive(&a, &b);
    }
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
            out.push(DiffLine { kind: DiffKind::Same, old_no: Some(i + 1), new_no: Some(j + 1), text: a[i].to_string() });
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * w + j] >= lcs[i * w + j + 1] {
            out.push(DiffLine { kind: DiffKind::Del, old_no: Some(i + 1), new_no: None, text: a[i].to_string() });
            i += 1;
        } else {
            out.push(DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(j + 1), text: b[j].to_string() });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffLine { kind: DiffKind::Del, old_no: Some(i + 1), new_no: None, text: a[i].to_string() });
        i += 1;
    }
    while j < m {
        out.push(DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(j + 1), text: b[j].to_string() });
        j += 1;
    }
    out
}

fn naive(a: &[&str], b: &[&str]) -> Vec<DiffLine> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    for (i, line) in a.iter().enumerate() {
        out.push(DiffLine { kind: DiffKind::Del, old_no: Some(i + 1), new_no: None, text: (*line).to_string() });
    }
    for (j, line) in b.iter().enumerate() {
        out.push(DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(j + 1), text: (*line).to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 增行：上下文两行 Same，中间插进一行 Add，断言行号和内容。
    #[test]
    fn added_line_has_exact_text_and_position() {
        let d = diff_lines("alpha\ncharlie\n", "alpha\nbravo\ncharlie\n");
        assert_eq!(d.len(), 3);
        assert_eq!(d[0], DiffLine { kind: DiffKind::Same, old_no: Some(1), new_no: Some(1), text: "alpha".into() });
        assert_eq!(d[1], DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(2), text: "bravo".into() });
        assert_eq!(d[2], DiffLine { kind: DiffKind::Same, old_no: Some(2), new_no: Some(3), text: "charlie".into() });
    }

    /// 删行：中间那行 Del，行号只带旧侧。
    #[test]
    fn removed_line_has_exact_text_and_position() {
        let d = diff_lines("alpha\nbravo\ncharlie\n", "alpha\ncharlie\n");
        assert_eq!(d.len(), 3);
        assert_eq!(d[1], DiffLine { kind: DiffKind::Del, old_no: Some(2), new_no: None, text: "bravo".into() });
        assert_eq!(d[2], DiffLine { kind: DiffKind::Same, old_no: Some(3), new_no: Some(2), text: "charlie".into() });
    }

    /// 改行 = 一行 Del + 一行 Add，两侧行号相同，内容分别是旧值 / 新值。
    #[test]
    fn changed_line_is_del_then_add_with_both_values() {
        let d = diff_lines("alpha\nold\ncharlie\n", "alpha\nnew\ncharlie\n");
        assert_eq!(d.len(), 4);
        assert_eq!(d[1], DiffLine { kind: DiffKind::Del, old_no: Some(2), new_no: None, text: "old".into() });
        assert_eq!(d[2], DiffLine { kind: DiffKind::Add, old_no: None, new_no: Some(2), text: "new".into() });
    }
}
