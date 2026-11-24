//! 最左列窄图标条（rail）的 6 个入口图标。
//!
//! 来源：`demo/rail-icons-cyber/preview.html`，按用户给的「组号-序号」原话取：
//! 聊天 01–13 / 模板 01–06 / 触发器 03–24 / 资源 04–08 / 元素 04–18 / 数据 04–35。
//! **编号以组号为准，不是组名** —— 所以「模板」用的是 01（聊天）那组的第 6 个。
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
    /// 模板：双机器人对话（01–06）
    Templates,
    /// 触发器：特斯拉塔（03–24）
    Triggers,
    /// 资源：齿轮 + 扳手（04–08）
    Resources,
    /// 元素：管道网络（04–18）
    Elements,
    /// 数据：AGV 仓储机器人（04–35）
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

const TEMPLATES: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="8.4" cy="10.4" r="4.7" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="6.9" cy="9.6" r="0.9" fill="currentColor"/><circle cx="9.9" cy="9.6" r="0.9" fill="currentColor"/><path d="M6.6 12.4h3.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.7"/><circle cx="17" cy="15.6" r="4.4" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="15.6" cy="14.9" r="0.85" fill="currentColor"/><circle cx="18.4" cy="14.9" r="0.85" fill="currentColor"/><path d="M15.3 17.5h3.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.7"/><path d="M13.2 6.4h2.4a1.4 1.4 0 0 1 1.4 1.4v1.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" opacity="0.5"/></svg>"##;

const TRIGGERS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M6 20.9h12" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 20.9V9.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><path d="M8.6 17.6h6.8M9.6 15.4h4.8M10.6 13.2h2.8" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="9.2" r="0.95" fill="currentColor"/><path d="M12 8.2 10 5.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.85"/><path d="M12 8.2 14 5.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.85"/><circle cx="9.4" cy="4.4" r="0.8" fill="currentColor"/><circle cx="14.6" cy="4.4" r="0.8" fill="currentColor"/></svg>"##;

const RESOURCES: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="9.6" cy="10" r="5.28" fill="none" stroke="currentColor" stroke-width="1.6"/><circle cx="9.6" cy="10" r="2.772" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M14.75 10.00L16.53 10.00" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M13.24 13.64L14.50 14.90" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M9.60 15.15L9.60 16.93" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.96 13.64L4.70 14.90" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M4.45 10.00L2.67 10.00" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.96 6.36L4.70 5.10" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M9.60 4.85L9.60 3.07" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M13.24 6.36L14.50 5.10" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><path d="M14.6 15.4 19.6 20.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="20.6" cy="21.4" r="2.2" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M16.2 17.2l1.4-1.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.7"/></svg>"##;

const ELEMENTS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M3.4 6.4h6.2v5.4h5.4v6.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><rect x="1.8" y="4.8" width="2.2" height="3.2" rx="0.5" fill="none" stroke="currentColor" stroke-width="1.7" opacity="0.7"/><path d="M20.6 4.4v5.6h-5.6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" opacity="0.7"/><rect x="19.4" y="2.8" width="2.2" height="3.2" rx="0.5" fill="none" stroke="currentColor" stroke-width="1.7" opacity="0.6"/><circle cx="9.6" cy="11.8" r="1.4" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="3.4" cy="3.4" r="0.8" fill="currentColor"/><circle cx="20.6" cy="18" r="0.8" fill="currentColor"/></svg>"##;

const DATA: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3.4" y="9.6" width="17.2" height="8" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="7" cy="18.6" r="1.8" fill="none" stroke="currentColor" stroke-width="1.7"/><circle cx="17" cy="18.6" r="1.8" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M2.8 15.6h18.4" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" opacity="0.3"/><rect x="5.4" y="3.6" width="8" height="5" rx="1" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M6.6 6h2.6" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" opacity="0.5"/><circle cx="6.6" cy="13" r="0.8" fill="currentColor"/><circle cx="9.8" cy="13" r="0.8" fill="currentColor"/></svg>"##;

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
