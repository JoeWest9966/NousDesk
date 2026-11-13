//! 主题：一套皮的全部颜色 + 落盘。
//!
//! 现在只有「默认」一套 —— 这是**骨架**。加第二套皮 = 在这个文件里加一个
//! `ThemeName` 变体和它的 `Theme`，设置页的「主题」里自动多一张卡片，
//! 不需要动设置页的代码。
//!
//! 三条边界：
//! - **盘面（`board_art`）不跟主题走。** 那是照抄 qrd3wh 面板页的 Flexoki dark
//!   画布，换皮会把「看着像但不是」变成「压根不像」。
//! - 主题落盘在 `~/.config/nousdesk/theme.toml`，**不写 `desktop.toml`**：那份是
//!   连接配置（只有 host / board 两个键），而便携包的 `desktop.toml` 排在 exe 旁边，
//!   可能只读 —— 换主题不该去动解压目录，也不该污染那个「只有两个键」的契约。
//! - 读坏了 / 名字不认识 → **回默认，不报错**。主题是 UI 偏好，不该挡住启动；
//!   `desktop.toml` 写坏要报错，两者严格程度不同，别搞混。
//!
//! 颜色当前还是各模块里的 `const`（渲染层尚未接过来）。`Theme` 的取值就是照那些
//! 常量抄的，`默认主题的取值和出厂常量一致` 这条测试钉住它们不许悄悄漂。

use std::path::PathBuf;

use crate::config::APP_DIR;

/// 主题文件的文件名（在 `APP_DIR` 下）。
const FILE: &str = "theme.toml";

/// 一套皮的颜色。
///
/// 字段是**语义**不是位置：加第二套皮时照这些名字填，别按「第几栏」取名。
/// 字段集合会随着渲染层接线慢慢长 —— 现在是主要的十三种，够画出一张差异明显的卡。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// 最左图标条的深色。
    pub sidebar: u32,
    /// 会话列表的底色。
    pub list_bg: u32,
    /// 聊天区的底色。
    pub chat_bg: u32,
    /// 我方气泡。
    pub bubble_me: u32,
    /// 对方气泡。
    pub bubble_other: u32,
    /// 正文。
    pub text: u32,
    /// 次要文字。
    pub muted: u32,
    /// 更淡的说明文字。
    pub faint: u32,
    /// 分隔线 / 边框。
    pub line: u32,
    /// 主色（选中、发送、可点的按钮）。
    pub accent: u32,
    /// 主色按下 / 悬停。
    pub accent_hover: u32,
    /// 错误。
    pub danger: u32,
    /// 成功。
    pub ok: u32,
}

impl Theme {
    /// 「默认」：微信桌面版那套。取值照 `app.rs` / `settings.rs` 的出厂常量抄。
    pub const DEFAULT: Theme = Theme {
        sidebar: 0x2e2e2e,
        list_bg: 0xf7f7f7,
        chat_bg: 0xededed,
        bubble_me: 0x95ec69,
        bubble_other: 0xffffff,
        text: 0x2b2b2b,
        muted: 0x999999,
        faint: 0xaaaaaa,
        line: 0xeeeeee,
        accent: 0x07c160,
        accent_hover: 0x06ad56,
        danger: 0xd32f2f,
        ok: 0x079c50,
    };
}

impl Default for Theme {
    fn default() -> Self {
        Theme::DEFAULT
    }
}

/// 主题的名字。**落盘存的是 `id()`**，改了 id 老用户的主题就丢 —— 别改。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Default,
}

impl ThemeName {
    /// 设置页按这个顺序列卡片。加皮只改这里和 `theme()`。
    pub const ALL: &'static [ThemeName] = &[ThemeName::Default];

    /// 落盘用的 id（小写、稳定）。
    pub fn id(self) -> &'static str {
        match self {
            ThemeName::Default => "default",
        }
    }

    /// 设置页上给人看的名字。
    pub fn label(self) -> &'static str {
        match self {
            ThemeName::Default => "默认",
        }
    }

    /// 这套皮的颜色。
    pub fn theme(self) -> Theme {
        match self {
            ThemeName::Default => Theme::DEFAULT,
        }
    }

    /// 从落盘的 id 认回来。认不出 → `None`（调用方回默认，不报错）。
    pub fn from_id(id: &str) -> Option<Self> {
        ThemeName::ALL.iter().copied().find(|t| t.id() == id)
    }

    /// 主题文件在哪儿（`~/.config/nousdesk/theme.toml`）。
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join(APP_DIR).join(FILE))
    }

    /// 读当前主题。**任何失败都回默认**：没文件、没目录、写坏了、id 不认识。
    pub fn load() -> Self {
        ThemeName::path()
            .map(|path| ThemeName::load_from(&path))
            .unwrap_or_default()
    }

    /// 写到当前主题文件（父目录不存在就建）。
    pub fn save(self) -> Result<PathBuf, String> {
        let path = ThemeName::path().ok_or_else(|| "找不到配置目录".to_string())?;
        self.save_to(&path)?;
        Ok(path)
    }

    // ── 可测的两步（不动环境变量）──────────────────────────

    fn load_from(path: &std::path::Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return ThemeName::default();
        };
        #[derive(serde::Deserialize)]
        struct File {
            theme: Option<String>,
        }
        // 未知字段照收（`toml` 默认忽略），以后加字段不会让老文件读不出来。
        match toml::from_str::<File>(&text) {
            Ok(file) => file
                .theme
                .as_deref()
                .and_then(ThemeName::from_id)
                .unwrap_or_default(),
            Err(_) => ThemeName::default(),
        }
    }

    fn save_to(self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("建不了目录 {}：{e}", dir.display()))?;
        }
        let text = format!("theme = \"{}\"\n", self.id());
        std::fs::write(path, text).map_err(|e| format!("写不了 {}：{e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nousdesk-theme-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("nested").join("theme.toml")
    }

    /// 默认皮的取值不许和出厂常量漂开 —— 渲染层现在还在用那些 `const`，
    /// 两边一旦不一致，接线那天就是满屏颜色跳变。
    #[test]
    fn default_theme_matches_shipped_colors() {
        let t = Theme::DEFAULT;
        assert_eq!(t.sidebar, crate::app::SIDEBAR_DARK, "侧栏");
        assert_eq!(t.list_bg, crate::app::LIST_BG, "列表底");
        assert_eq!(t.chat_bg, crate::app::CHAT_BG, "聊天底");
        assert_eq!(t.bubble_me, crate::app::BUBBLE_ME, "我方气泡");
        assert_eq!(t.bubble_other, crate::app::BUBBLE_OTHER, "对方气泡");
        assert_eq!(t.muted, crate::app::MUTED, "次要文字");
        assert_eq!(t.accent, crate::app::WECHAT_GREEN, "主色");
    }

    /// 每个 id 都要能往返，且 `ALL` 不许有漏登记的名字。
    #[test]
    fn every_name_round_trips_through_id() {
        for name in ThemeName::ALL {
            assert_eq!(ThemeName::from_id(name.id()), Some(*name), "{}", name.id());
            assert!(!name.label().is_empty(), "{} 没有给人看的名字", name.id());
        }
        assert_eq!(ThemeName::from_id("no-such-theme"), None);
    }

    /// 存 → 读回来还是同一个；文件里只有一个键。
    #[test]
    fn save_then_load_round_trips() {
        let path = tmp("round-trip");
        ThemeName::Default.save_to(&path).expect("写得进去");

        let text = std::fs::read_to_string(&path).expect("读得回来");
        assert_eq!(text.trim(), "theme = \"default\"", "文件内容：{text}");
        assert_eq!(ThemeName::load_from(&path), ThemeName::Default);

        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    /// 丢文件 / 写坏 / id 不认识，一律回默认 —— UI 偏好不该把窗口挡在门外。
    #[test]
    fn broken_or_missing_file_falls_back_to_default() {
        let missing = tmp("missing");
        assert_eq!(ThemeName::load_from(&missing), ThemeName::Default);

        let bad = tmp("bad-toml");
        std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
        std::fs::write(&bad, "这不是 toml {{{").unwrap();
        assert_eq!(ThemeName::load_from(&bad), ThemeName::Default);

        let unknown = tmp("unknown-id");
        std::fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        std::fs::write(&unknown, "theme = \"neon-lime\"\n").unwrap();
        assert_eq!(ThemeName::load_from(&unknown), ThemeName::Default);

        // 以后加字段：老文件里多出来的键不许让读回到默认。
        let extra = tmp("extra-keys");
        std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
        std::fs::write(&extra, "theme = \"default\"\nother = 1\n").unwrap();
        assert_eq!(ThemeName::load_from(&extra), ThemeName::Default);

        for p in [&bad, &unknown, &extra] {
            let _ = std::fs::remove_dir_all(p.parent().unwrap().parent().unwrap());
        }
    }
}
