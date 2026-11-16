//! 左栏第二行预览：把模型输出里的 markdown 记号摘掉，只留人读的字。
//!
//! 那一行是**单行摘要**（微信式的），给的是 `last.text` 原文 —— 模型动不动就回
//! `**加粗**`、`# 标题`、`` `cmd` ``、代码围栏，原样摊平之后列表里全是 `**` 和 ```
//! （真实数据里就有：三条会话的预览是 ```` ```\n9996696.xyz\n/root\n``` ````）。
//!
//! 这里只做「摘记号」，**不做渲染**：不解析嵌套、不认表格、不碰 HTML。
//! 原则是**宁可少摘，不要误删** —— 认不准的记号原样留着，因为预览里少几个星号是小事，
//! 把 `snake_case`、`a*b`、`<harness>` 这类工具输出吃掉了才是真的丢信息。

/// 一行预览。
///
/// 顺序有讲究：先按行处理（围栏 / 标题 / 列表记号都是**行首**语义），再把每行摘完的行内
/// 记号，最后才知道要拼成一行。
pub fn plain(text: &str) -> String {
    let mut out = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        // 围栏行整行扔掉，围栏里的内容照收（工具输出就是靠它包着的）。
        if is_fence(line) {
            continue;
        }
        let line = strip_heading(line);
        let line = strip_line_marker(line);
        let line = strip_inline(line);
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&line);
    }
    out
}

fn is_fence(line: &str) -> bool {
    line.starts_with("```") || line.starts_with("~~~")
}

/// `### 标题` → `标题`。要**先看行首**再看后面有没有空格：`#include` 那种不是标题。
fn strip_heading(line: &str) -> &str {
    let level = line.len() - line.trim_start_matches('#').len();
    if level == 0 || level > 6 {
        return line;
    }
    let rest = &line[level..];
    if rest.is_empty() {
        return "";
    }
    if !rest.starts_with(' ') {
        // `#include <stdio.h>` 这种：不是标题，原样留着。
        return line;
    }
    rest.trim_start()
}

/// 行首的列表 / 引用记号：`- ` `* ` `+ ` `> ` `1. ` `1) `。
fn strip_line_marker(line: &str) -> &str {
    for m in ["> ", "- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(m) {
            return rest.trim_start();
        }
    }
    // 有序列表：数字 + `.` / `)` + 空格。只认一到三位数字，`9996696.xyz` 不能被吃掉。
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=3).contains(&digits) {
        let rest = &line[digits..];
        for m in [". ", ") "] {
            if let Some(r) = rest.strip_prefix(m) {
                return r.trim_start();
            }
        }
    }
    line
}

/// 行内记号：`` `code` `` 的反引号、`**粗**` / `__粗__`、`[字](url)`、`![说明](图)`。
///
/// **单星号 / 单下划线不动**：`a*b`、`snake_case`、`2*3` 在工具输出里到处都是，
/// 摘了就是改内容；`*斜体*` 带着星号也比把词切断强。
fn strip_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(ch) = rest.chars().next() {
        match ch {
            '`' => {
                rest = &rest[1..];
            }
            '*' | '_' => {
                // 只有成对的 `**` / `__` 才是加粗，摘掉；单个的留着。
                // （`*` 后面只能是 `*`、`_` 后面只能是 `_`，所以合成一条判定。）
                if rest.starts_with("**") || rest.starts_with("__") {
                    rest = &rest[2..];
                } else {
                    out.push(ch);
                    rest = &rest[1..];
                }
            }
            '!' if rest.starts_with("![") => {
                // 图片：只留方括号里的说明，把 `(地址)` 扔掉。
                if let Some((alt, after)) = bracket(rest, "![") {
                    out.push_str(&alt);
                    rest = after;
                } else {
                    out.push(ch);
                    rest = &rest[1..];
                }
            }
            '[' => {
                // 链接：留方括号里的字。
                if let Some((text, after)) = bracket(rest, "[") {
                    out.push_str(&text);
                    rest = after;
                } else {
                    out.push(ch);
                    rest = &rest[1..];
                }
            }
            _ => {
                out.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `[字](地址)` / `![说明](地址)`：返回 `(括号里的字, 地址之后的位置)`。
/// 形状不对（没有 `](`、没有 `)`）就返回 `None`，调用方原样留着。
fn bracket<'a>(s: &'a str, open: &str) -> Option<(String, &'a str)> {
    let body = s.strip_prefix(open)?;
    let close = body.find(']')?;
    let text = &body[..close];
    let after = body[close + 1..].strip_prefix('(')?;
    let paren = after.find(')')?;
    Some((text.to_string(), &after[paren + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 代码围栏整行扔掉_里面的话留着() {
        assert_eq!(plain("```\n9996696.xyz\n/root\n```"), "9996696.xyz /root");
        assert_eq!(plain("```bash\nls -la\n```"), "ls -la");
        assert_eq!(plain("~~~\nx\n~~~"), "x");
    }

    #[test]
    fn 行内反引号摘掉() {
        assert_eq!(
            plain("用 bash 跑 `hostname; pwd`，把输出贴给我"),
            "用 bash 跑 hostname; pwd，把输出贴给我"
        );
    }

    #[test]
    fn 成对的加粗摘掉_单个星号不动() {
        assert_eq!(plain("**重要**：别删 __这个__"), "重要：别删 这个");
        // 认不准的一律留着：`snake_case`、`a*b`、`2*3` 都是内容。
        assert_eq!(plain("snake_case 和 a*b 和 2*3"), "snake_case 和 a*b 和 2*3");
    }

    #[test]
    fn 标题引用列表的行首记号摘掉() {
        assert_eq!(plain("## 小结\n> 引用一句\n- 第一条\n* 第二条\n1. 第三条"),
                   "小结 引用一句 第一条 第二条 第三条");
        // `#include` 不是标题。
        assert_eq!(plain("#include <stdio.h>"), "#include <stdio.h>");
    }

    #[test]
    fn 数字开头的正文别被当列表() {
        // 四位以上不认（`9996696.xyz` 是主机名）；一位数字后面没空格也不认。
        assert_eq!(plain("9996696.xyz /root"), "9996696.xyz /root");
        assert_eq!(plain("3.5 秒"), "3.5 秒");
        assert_eq!(plain("1. 真的列表"), "真的列表");
    }

    #[test]
    fn 链接留字_图片留说明() {
        assert_eq!(plain("见 [文档](https://x.y/z)，图：![架构图](/a.png)"),
                   "见 文档，图：架构图");
        // 形状不对就原样留着，别把方括号吃掉。
        assert_eq!(plain("[没闭合 和 (没括号)"), "[没闭合 和 (没括号)");
    }

    #[test]
    fn 摊平成一行且不留前后空格() {
        assert_eq!(plain("  第一行\n\n第二行 \t 第三行  "), "第一行 第二行 第三行");
        assert_eq!(plain(""), "");
        assert_eq!(plain("   \n\t\n"), "");
    }

    #[test]
    fn 工具输出里的尖括号别动() {
        // `<harness>` / `</harness>` 是真内容（不是 HTML），摘了就丢信息。
        assert_eq!(
            plain("Tool failed:\n```\n<harness>\n[error] boom\n</harness>\n```"),
            "Tool failed: <harness> [error] boom </harness>"
        );
    }
}
