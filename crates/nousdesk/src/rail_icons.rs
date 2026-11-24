//! 最左列窄图标条（rail）的 6 个入口图标。
//!
//! 来源：`demo/rail-icons-cyber/preview.html`，按用户给的「组号-序号」原话取：
//! 聊天 01–13 / 模板 02–01 / 触发器 03–18 / 资源 04–03 / 元素 04–28 / 数据 06–01。
//! **编号以组号为准，不是组名** —— 两段的数字就是选择，别按主题名"纠正"。
//!
//! 不走 `IconName`：lucide 里没有这套「机器人 / 赛博」线条。这里用
//! `Icon::data` 内嵌源码，`currentColor` 交给 `Icon::text_color` 替换 ——
//! active / 常态 / 灰化仍走原来那条上色路径，rail 里不用感知颜色。
//!
//! 改图标只改这个文件：每个常量是一份 24×24 的完整 SVG，**别动 viewBox**，
//! 也别忘了 `currentColor`（写成固定色 = 换主题不跟着变）。

/// 6 个入口图标。`svg()` 给的是完整 SVG 源码。
#[derive(Clone, Copy)]
pub(crate) enum RailIcon {
    /// 聊天：机器人侧脸 + 侧插口（01–13）
    Chats,
    /// 模板：芯片（02–01）
    Templates,
    /// 触发器：光子脉冲（03–18）
    Triggers,
    /// 资源：服务器机架（04–03）
    Resources,
    /// 元素：齿轮 + 螺栓备件（04–28）
    Elements,
    /// 数据：数据库圆柱（06–01）
    Data,
}

impl RailIcon {
    pub(crate) fn svg(self) -> &'static [u8] {
        match self {
            Self::Chats => CHATS,
            Self::Templates => TEMPLATES,
            Self::Triggers => TRIGGERS,
            Self::Resources => RESOURCES,
            Self::Elements => ELEMENTS,
            Self::Data => DATA,
        }
    }
}

const CHATS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M5.4 8.4a6.6 6.6 0 0 1 13.2 0v7.2a3.6 3.6 0 0 1-3.6 3.6H9a3.6 3.6 0 0 1-3.6-3.6V8.4Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="9.6" cy="11.6" r="1.05" fill="currentColor"/><circle cx="14.2" cy="11.6" r="1.05" fill="currentColor"/><path d="M9.4 15.6h5.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.6"/><path d="M20.2 10.6h1.6v3.4h-1.6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.75"/></svg>"##;

const TEMPLATES: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="6.4" y="6.4" width="11.2" height="11.2" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M4.5 9.20H6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M17.6 9.20H19.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M4.5 12.00H6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M17.6 12.00H19.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M4.5 14.80H6.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M17.6 14.80H19.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/></svg>"##;

const TRIGGERS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="6.4" cy="12" r="2.6" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M10.4 8.2a5.4 5.4 0 0 1 0 7.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.9"/><path d="M13.6 6a9 9 0 0 1 0 12" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.7"/><path d="M16.8 3.8a12.6 12.6 0 0 1 0 16.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.45"/></svg>"##;

const RESOURCES: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="3.6" width="16" height="16.8" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M4 9.2h16M4 14.8h16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="6.6" cy="6.4" r="0.75" fill="currentColor"/><circle cx="6.6" cy="12" r="0.75" fill="currentColor"/><circle cx="6.6" cy="17.6" r="0.75" fill="currentColor"/><path d="M10 6.4h7M10 12h7M10 17.6h4.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" opacity="0.55"/></svg>"##;

const ELEMENTS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="9" cy="9" r="4.0" fill="none" stroke="currentColor" stroke-width="1.6"/><circle cx="9" cy="9" r="2.25" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M12.90 9.00L14.25 9.00" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M11.43 12.05L12.27 13.10" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M8.13 12.80L7.83 14.12" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.49 10.69L4.27 11.28" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.49 7.31L4.27 6.72" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M8.13 5.20L7.83 3.88" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M11.43 5.95L12.27 4.90" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M15.4 12.6 19.4 16.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="20.4" cy="17.6" r="2.2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M16.6 14 18 12.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.6"/><path d="M6.4 18.4 8.6 20.6" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" opacity="0.5"/><circle cx="5.4" cy="17.4" r="0.9" fill="currentColor"/></svg>"##;

const DATA: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3.4c4.6 0 8.2 1.3 8.2 3s-3.6 3-8.2 3-8.2-1.3-8.2-3 3.6-3 8.2-3Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><path d="M3.8 6.4v11.2c0 1.7 3.6 3 8.2 3s8.2-1.3 8.2-3V6.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><path d="M3.8 12c0 1.7 3.6 3 8.2 3s8.2-1.3 8.2-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, RailIcon); 6] = [
        ("chats", RailIcon::Chats),
        ("templates", RailIcon::Templates),
        ("triggers", RailIcon::Triggers),
        ("resources", RailIcon::Resources),
        ("elements", RailIcon::Elements),
        ("data", RailIcon::Data),
    ];

    /// 六份都得是能直接喂给 `Icon::data` 的完整 SVG。
    #[test]
    fn every_icon_is_a_self_contained_svg() {
        for (name, icon) in ALL {
            let s = std::str::from_utf8(icon.svg()).expect("svg 必须是 utf-8");
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

    /// 6 个入口各有一份，别复制粘贴成同一个。
    #[test]
    fn icons_are_distinct() {
        let mut seen: Vec<&[u8]> = Vec::new();
        for (_, icon) in ALL {
            assert!(!seen.contains(&icon.svg()), "有两份图标是同一个");
            seen.push(icon.svg());
        }
    }
}
