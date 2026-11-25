//! 会话列表搜索框下面那一行，后两颗按钮的图标。
//!
//! 来源：`demo/chat-btns-bot/preview.html` 的第 1 套（用户 2026-11-25 定的）。
//! 前两颗粒子不在这个文件 —— 它们是模板页分段按钮上那两枚芯片，
//! 直接复用 [`crate::template_icons`] 的 `SESSION` / `ULTIMATE`，不另画一份。
//!
//! 和 `rail_icons` / `template_icons` 是同一家人：24×24、`currentColor`、
//! `Icon::data` 内嵌源码，颜色交给 `Icon::text_color`（换皮 / hover 才跟着变）。
//! 线宽 **1.8**（不是 rail 的 1.7）—— 这两枚按 24px 画，demo 上量的就是这个值。

/// 升降箭头：一根杆 + 向上箭翼 + 双层底座。
pub(crate) const JACK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 14.4V3.6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M7.4 8.6 12 3.6l4.6 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.4 17.2h13.2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M7.2 20.4h9.6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

/// 机柜：三层抽屉，每层一块实心把手。
pub(crate) const RACK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="4.2" width="14" height="15.6" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M5 9.6h14M5 15h14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><rect x="9" y="6.2" width="6" height="1.8" fill="currentColor" stroke="none"/><rect x="9" y="11.6" width="6" height="1.8" fill="currentColor" stroke="none"/><rect x="9" y="17" width="3.6" height="1.8" fill="currentColor" stroke="none"/></svg>"##;

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, &[u8]); 2] = [("jack", JACK), ("rack", RACK)];

    /// 两份都得是能直接喂给 `Icon::data` 的完整 SVG。
    #[test]
    fn every_icon_is_a_self_contained_svg() {
        for (name, bytes) in ALL {
            let s = std::str::from_utf8(bytes).expect("svg 必须是 utf-8");
            assert!(s.starts_with("<svg "), "{name}: 缺 <svg 开头");
            assert!(s.ends_with("</svg>"), "{name}: 缺 </svg> 结尾");
            assert!(
                s.contains(r#"viewBox="0 0 24 24""#),
                "{name}: viewBox 不是 24"
            );
            assert!(
                s.contains("currentColor"),
                "{name}: 没有 currentColor，换主题不会跟着变色"
            );
            assert!(!s.contains("#000"), "{name}: 出现写死的黑色");
        }
    }

    /// 两颗是两块不同的形状。
    #[test]
    fn icons_are_distinct() {
        assert_ne!(JACK, RACK, "两个按钮画成了同一个图标");
    }
}
