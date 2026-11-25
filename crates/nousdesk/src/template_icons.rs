//! 模板页「会话模板 / 终极模板」分段按钮上的两个图标。
//!
//! 和 rail 那 6 个（`rail_icons.rs`）是一家人：模板入口是一枚**芯片**，这两个必须一眼认成
//! 同一套 —— 24×24、线宽 1.6、圆头圆角、一个实心点当重音、短横当「引脚」。
//!
//! 为什么不进 `IconName`：那里是 lucide 的线条，混进来就串味了。走 `Icon::data` 内嵌
//! 源码，`currentColor` 交给 `Icon::text_color` —— 换皮 / 灰化 / hover 全走原来那条上色路径。
//!
//! 挑的是 `demo/template-icons/icons.html` 里的 02 / 19（用户 2026-11-24 定的）：
//! 会话 = 芯片里一个**播放三角**（一段对话的开场）；终极 = **四面引脚 + 实心核心**
//! （触发 / 参数 / 引用都插进同一颗芯）。两个都是芯片底，区别在「芯里是什么」。

/// 会话模板：芯片 + 左右引脚 + 芯里一个播放三角。
pub(crate) const SESSION: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="6.4" y="6.4" width="11.2" height="11.2" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M4.5 9.2H6.4M4.5 12H6.4M4.5 14.8H6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M17.6 9.2H19.5M17.6 12H19.5M17.6 14.8H19.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M10.8 9.4 14.4 12l-3.6 2.6z" fill="currentColor" stroke="none"/></svg>"##;

/// 终极模板：芯片 + **四面**引脚 + 中心实心核。
pub(crate) const ULTIMATE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="6.4" y="6.4" width="11.2" height="11.2" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M4.5 9.2H6.4M4.5 12H6.4M4.5 14.8H6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M17.6 9.2H19.5M17.6 12H19.5M17.6 14.8H19.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M9.2 4.5V6.4M12 4.5V6.4M14.8 4.5V6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M9.2 17.6V19.5M12 17.6V19.5M14.8 17.6V19.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="1.7" fill="currentColor"/></svg>"##;

/// 段 id → 图标。`SECTIONS` 的顺序是引擎的段名，别在调用处再写一次 `if i == 0`。
pub(crate) fn for_section(section: &str) -> &'static [u8] {
    match section {
        "Ultimate-Template" => ULTIMATE,
        _ => SESSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两份都得是能直接喂给 `Icon::data` 的完整 SVG，且跟 rail 同一套画法。
    #[test]
    fn both_are_family_style_svgs() {
        for (name, bytes) in [("session", SESSION), ("ultimate", ULTIMATE)] {
            let s = std::str::from_utf8(bytes).expect("svg 必须是 utf-8");
            assert!(s.starts_with("<svg "), "{name}: 缺 <svg 开头");
            assert!(s.ends_with("</svg>"), "{name}: 缺 </svg> 结尾");
            assert!(s.contains(r#"viewBox="0 0 24 24""#), "{name}: viewBox 不是 24");
            assert!(s.contains("currentColor"), "{name}: 没有 currentColor");
            assert!(s.contains(r#"stroke-width="1.6""#), "{name}: 线宽不是 1.6");
            assert!(!s.contains("#000"), "{name}: 出现写死的黑色");
        }
        assert_ne!(SESSION, ULTIMATE, "两个段必须是两个图标");
    }

    /// 段名到图标：终极段拿 ULTIMATE，别的一律 SESSION。
    #[test]
    fn section_picks_the_right_glyph() {
        assert_eq!(for_section("Ultimate-Template"), ULTIMATE);
        assert_eq!(for_section("Session-Template"), SESSION);
    }
}
