//! 预设行右键菜单的模型（Template 页）。
//!
//! 和 `archive_menu.rs` 同一个分工：这里只放**菜单长什么样、点下去产出哪个动作**，
//! 不认引擎、不发 HTTP、不弹确认框。绘制走 gpui-kit 0.9 的 `ContextMenuExt` +
//! `PopupMenu`（子菜单 / 翻边 / Esc / 点空白关掉都由库管）。
//!
//! 条目照 demo 里挑的那套（`demo/template-page/lifecycle.html` 组 ① 方案 1）：
//! 开对话 / 改类别 / 编辑（本版还没做编辑器，画成禁用并写清原因）/ ─── /
//! 复制目录路径 / 重扫这一段 / ─── / 归档… / 彻底删掉…

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;

/// 这一帧的皮。和 `archive_menu.rs` / `settings.rs` 同一个写法。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 「彻底删掉」的副标题。归档可还原，这一项不可逆 —— 副标题就要说出来。
pub const PURGE_HINT: &str = "不可逆";

/// 菜单作用于哪条预设。`section` 是 API 段名（`Session-Template` / `Ultimate-Template`），
/// `dir` 用来「复制目录路径」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetTarget {
    pub section: String,
    pub name: String,
    pub dir: String,
    /// `false` = 盘上有、仓里没。归档一条没进仓的预设引擎也认，只是提示要说实话。
    pub tracked: bool,
}

impl PresetTarget {
    pub fn new(
        section: impl Into<String>,
        name: impl Into<String>,
        dir: impl Into<String>,
        tracked: bool,
    ) -> Self {
        Self {
            section: section.into(),
            name: name.into(),
            dir: dir.into(),
            tracked,
        }
    }
}

/// 菜单的出口。接线在 `templates.rs`：每一项都必须有可见结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetMenuAction {
    Open { name: String },
    /// 打开编辑器（C1 工作台）。
    Edit { name: String },
    /// 复制成新的：读源模板的原文改个名字再落一份（连 `PRESET.md` 一起）。
    Copy { name: String },
    ChangeCategory { name: String },
    CopyDir { dir: String },
    Reload,
    Archive { name: String },
    Purge { name: String },
}

/// 菜单里的一个可执行项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetMenuEntry {
    Open,
    ChangeCategory,
    Edit,
    Copy,
    CopyDir,
    Reload,
    Archive,
    Purge,
}

/// 一行：可执行项或分隔线。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuLine {
    Item(PresetMenuEntry),
    Separator,
}

/// 菜单全文。顺序就是屏幕上从上到下的顺序。
pub fn lines() -> Vec<MenuLine> {
    vec![
        MenuLine::Item(PresetMenuEntry::Open),
        MenuLine::Item(PresetMenuEntry::ChangeCategory),
        MenuLine::Item(PresetMenuEntry::Edit),
        MenuLine::Item(PresetMenuEntry::Copy),
        MenuLine::Separator,
        MenuLine::Item(PresetMenuEntry::CopyDir),
        MenuLine::Item(PresetMenuEntry::Reload),
        MenuLine::Separator,
        MenuLine::Item(PresetMenuEntry::Archive),
        MenuLine::Item(PresetMenuEntry::Purge),
    ]
}

/// 看到的那行字。
pub fn label(entry: PresetMenuEntry) -> &'static str {
    match entry {
        PresetMenuEntry::Open => "开一条新对话",
        PresetMenuEntry::ChangeCategory => "改类别…",
        PresetMenuEntry::Edit => "编辑",
        PresetMenuEntry::Copy => "复制成新的…",
        PresetMenuEntry::CopyDir => "复制目录路径",
        PresetMenuEntry::Reload => "重扫这一段",
        PresetMenuEntry::Archive => "归档…",
        PresetMenuEntry::Purge => "彻底删掉…",
    }
}

/// 行尾的灰色副标题。没副标题就不挂那一格。
pub fn hint(entry: PresetMenuEntry) -> Option<&'static str> {
    match entry {
        PresetMenuEntry::Purge => Some(PURGE_HINT),
        PresetMenuEntry::Copy => Some("连说明一起"),
        _ => None,
    }
}

/// 这一项此刻能不能点。
///
/// 只有两处会变灰：**编辑器还没做**（这一版），以及**终极模板段开不了会话**
/// （那一段的启用动作还没定端点，别在客户端发明一个）。
pub fn entry_enabled(entry: PresetMenuEntry, can_open: bool) -> bool {
    match entry {
        PresetMenuEntry::Open => can_open,
        _ => true,
    }
}

/// 条目 → 动作。`Edit` 是唯一不出动作的（它灰着）。
pub fn entry_action(
    entry: PresetMenuEntry,
    target: &PresetTarget,
) -> Option<PresetMenuAction> {
    let name = || target.name.clone();
    Some(match entry {
        PresetMenuEntry::Open => PresetMenuAction::Open { name: name() },
        PresetMenuEntry::ChangeCategory => PresetMenuAction::ChangeCategory { name: name() },
        PresetMenuEntry::Edit => PresetMenuAction::Edit { name: name() },
        PresetMenuEntry::Copy => PresetMenuAction::Copy { name: name() },
        PresetMenuEntry::CopyDir => PresetMenuAction::CopyDir {
            dir: target.dir.clone(),
        },
        PresetMenuEntry::Reload => PresetMenuAction::Reload,
        PresetMenuEntry::Archive => PresetMenuAction::Archive { name: name() },
        PresetMenuEntry::Purge => PresetMenuAction::Purge { name: name() },
    })
}

/// 把模型画进 `context_menu` 给的那只 `PopupMenu` 里。
///
/// `on_pick` 是调用方注入的出口（本体不知道 `Desktop` 长什么样）。
pub fn build_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
    target: &PresetTarget,
    can_open: bool,
    on_pick: impl Fn(PresetMenuAction, &mut Window, &mut App) + Clone + 'static,
) -> PopupMenu {
    let _ = (window, cx);
    let mut menu = menu;
    for line in lines() {
        menu = match line {
            MenuLine::Separator => menu.separator(),
            MenuLine::Item(entry) => menu.item(item_for(entry, target, can_open, on_pick.clone())),
        };
    }
    menu
}

/// 一个可执行项。带副标题的走自定义渲染 —— 库里 `Item` 只有一行字。
fn item_for<F>(
    entry: PresetMenuEntry,
    target: &PresetTarget,
    can_open: bool,
    on_pick: F,
) -> PopupMenuItem
where
    F: Fn(PresetMenuAction, &mut Window, &mut App) + 'static,
{
    let text = label(entry);
    let enabled = entry_enabled(entry, can_open);

    let Some(action) = entry_action(entry, target) else {
        // 现在每一项都有动作；真走到这儿说明模型和渲染对不上，画成不可点而不是悄悄少一项。
        debug_assert!(false, "{entry:?} 没有动作，不该走 item_for");
        return PopupMenuItem::label(text);
    };

    let handler = move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
        on_pick(action.clone(), window, cx)
    };

    match hint(entry) {
        Some(hint) => PopupMenuItem::element(move |_window, _cx| {
            div()
                .flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(t().d(12.))
                .child(div().child(text))
                .child(
                    div()
                        .flex_none()
                        .text_size(t().fs(11.))
                        .text_color(if matches!(entry, PresetMenuEntry::Purge) {
                            rgb(t().p.danger)
                        } else {
                            rgb(t().p.faint)
                        })
                        .child(hint),
                )
                .into_any_element()
        })
        .on_click(handler)
        .disabled(!enabled),
        None => PopupMenuItem::new(text).on_click(handler).disabled(!enabled),
    }
}

#[cfg(test)]
mod tests {
    // 别写 `use super::*;` —— 会把 `gpui_kit::*` 带进来，`#[test]` 展开时撞 recursion limit。
    use super::{
        MenuLine, PURGE_HINT, PresetMenuAction, PresetMenuEntry, PresetTarget, entry_action,
        entry_enabled, hint, label, lines,
    };

    fn target() -> PresetTarget {
        PresetTarget::new("Session-Template", "deep-dive", "/repo/presets/Session-Template/deep-dive", true)
    }

    fn entries() -> Vec<PresetMenuEntry> {
        lines()
            .into_iter()
            .filter_map(|l| match l {
                MenuLine::Item(e) => Some(e),
                MenuLine::Separator => None,
            })
            .collect()
    }

    #[test]
    fn menu_is_the_nine_lines_from_the_demo() {
        assert_eq!(
            lines(),
            vec![
                MenuLine::Item(PresetMenuEntry::Open),
                MenuLine::Item(PresetMenuEntry::ChangeCategory),
                MenuLine::Item(PresetMenuEntry::Edit),
                MenuLine::Item(PresetMenuEntry::Copy),
                MenuLine::Separator,
                MenuLine::Item(PresetMenuEntry::CopyDir),
                MenuLine::Item(PresetMenuEntry::Reload),
                MenuLine::Separator,
                MenuLine::Item(PresetMenuEntry::Archive),
                MenuLine::Item(PresetMenuEntry::Purge),
            ]
        );
    }

    #[test]
    fn labels_match_the_demo_copy() {
        assert_eq!(label(PresetMenuEntry::Open), "开一条新对话");
        assert_eq!(label(PresetMenuEntry::ChangeCategory), "改类别…");
        assert_eq!(label(PresetMenuEntry::Edit), "编辑");
        assert_eq!(label(PresetMenuEntry::CopyDir), "复制目录路径");
        assert_eq!(label(PresetMenuEntry::Reload), "重扫这一段");
        assert_eq!(label(PresetMenuEntry::Archive), "归档…");
        assert_eq!(label(PresetMenuEntry::Purge), "彻底删掉…");
    }

    /// 「彻底删掉」可点但要标明不可逆；「编辑」不再灰着（编辑器已经接上了）。
    #[test]
    fn purge_says_it_is_irreversible_and_edit_is_enabled() {
        assert!(entry_enabled(PresetMenuEntry::Purge, true));
        assert_eq!(hint(PresetMenuEntry::Purge), Some(PURGE_HINT));
        assert_eq!(PURGE_HINT, "不可逆");

        assert!(entry_enabled(PresetMenuEntry::Edit, true));
        assert!(entry_enabled(PresetMenuEntry::Edit, false));
        assert_eq!(hint(PresetMenuEntry::Edit), None);
    }

    /// 终极模板段开不了会话：「开一条新对话」变灰，但**不许从菜单里消失**
    /// （形状稳定 —— 菜单条目跟着数据变，用户每次都得重新找）。
    #[test]
    fn open_is_greyed_out_but_kept_on_the_ultimate_section() {
        assert!(entries().contains(&PresetMenuEntry::Open));
        assert!(!entry_enabled(PresetMenuEntry::Open, false));
        assert!(entry_enabled(PresetMenuEntry::Open, true));
        for e in entries() {
            if e != PresetMenuEntry::Open {
                assert!(
                    entry_enabled(e, false),
                    "{e:?} 不该因为开不了会话而变灰"
                );
            }
        }
    }

    #[test]
    fn entries_map_to_actions_with_the_target_identity() {
        let target = target();
        assert_eq!(
            entry_action(PresetMenuEntry::Open, &target),
            Some(PresetMenuAction::Open {
                name: "deep-dive".into()
            })
        );
        assert_eq!(
            entry_action(PresetMenuEntry::ChangeCategory, &target),
            Some(PresetMenuAction::ChangeCategory {
                name: "deep-dive".into()
            })
        );
        assert_eq!(
            entry_action(PresetMenuEntry::CopyDir, &target),
            Some(PresetMenuAction::CopyDir {
                dir: "/repo/presets/Session-Template/deep-dive".into()
            })
        );
        assert_eq!(
            entry_action(PresetMenuEntry::Reload, &target),
            Some(PresetMenuAction::Reload)
        );
        assert_eq!(
            entry_action(PresetMenuEntry::Archive, &target),
            Some(PresetMenuAction::Archive {
                name: "deep-dive".into()
            })
        );
        assert_eq!(
            entry_action(PresetMenuEntry::Purge, &target),
            Some(PresetMenuAction::Purge {
                name: "deep-dive".into()
            })
        );
    }

    /// 「编辑」现在真有动作（打开 C1 工作台），不再是个占位。
    #[test]
    fn edit_opens_the_editor() {
        assert_eq!(
            entry_action(PresetMenuEntry::Edit, &target()),
            Some(PresetMenuAction::Edit {
                name: "deep-dive".into()
            })
        );
    }
}
