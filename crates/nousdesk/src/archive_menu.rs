//! 左栏会话行的右键菜单模型（票 #5）。
//!
//! 这里只放**菜单长什么样、点下去产出哪个动作**：不认引擎、不发 HTTP、不弹确认框。
//! 归档 / 撕原件的执行留给接线那票 —— [`SessionMenuAction`] 就是交给它的出口。
//!
//! ## 为什么没有自己的浮层 / 状态机
//!
//! 绘制走 gpui-kit 0.9 的原生 `ContextMenuExt` + `PopupMenu::submenu`：
//!
//! - 子菜单是**独立的一层**（`PopupMenuItem::Submenu` 里挂着另一个 `Entity<PopupMenu>`，
//!   父菜单只是被选中、不被替换）；
//! - 悬停即展开（`render_item` 的 `on_hover` → `selected_index` → 选中项渲染子菜单），
//!   点击也展开（`activate` → `_select_submenu`），两条路都不关父菜单；
//! - 右侧放不下就翻到左边（`update_submenu_menu_anchor` 拿窗口 viewport 宽度比）；
//! - Esc / 点空白 / 点任一可执行项关掉整摞（`dismiss` 会顺着 `parent_menu` 往上传）；
//! - `separator()` 幂等、`menu_with_disabled` 才有禁用态。
//!
//! 这些都在库里，本模块不重复造。本模块负责库不管的那一半：
//! **条目顺序、文案、副标题，以及「哪一项 → 哪个动作」**。所以可测的缝是纯函数
//! （[`top_lines`] / [`more_lines`] / [`label`] / [`hint`] / [`entry_action`]），
//! 不开窗口、不合成鼠标。

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;

/// 这一帧的皮。和 `settings.rs` / `archive_confirm.rs` 同一个写法。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 菜单作用于哪条会话。`id` 进动作，`name` 只用于「归档 / 撕原件」这类要说话的入口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTarget {
    pub id: i64,
    pub name: String,
}

impl SessionTarget {
    pub fn new(id: i64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

/// 菜单的出口。接线那票按这个 match：能立即做的（打开 / 置顶 / 复制）现在就有分支，
/// 归档 / 撕原件 / 重命名是**待接协议**的占位，不许静默丢。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionMenuAction {
    Open {
        id: i64,
    },
    TogglePin {
        id: i64,
    },
    Archive {
        id: i64,
        name: String,
    },
    Purge {
        id: i64,
        name: String,
    },
    Rename {
        id: i64,
    },
    CopyName {
        name: String,
    },
    CopyId {
        id: i64,
    },
    /// 整摞关掉（Esc / 点空白）。不是可点的条目，是「什么都没选」的显式出口。
    Close,
}

/// 菜单里的一个可执行项。`More` 不产动作（它只开子菜单），`Dismiss` 是关闭出口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMenuEntry {
    Open,
    TogglePin,
    More,
    Rename,
    CopyName,
    Archive,
    Purge,
    CopyId,
    /// 不是菜单里的一行：整摞关掉（Esc / 点空白）的统一出口。
    ///
    /// 收起这件事由库做（`PopupMenu::dismiss` 沿 `parent_menu` 往上传），
    /// 所以这一支在 `app.rs` 里没有构造点 —— 留着是给 [`entry_action`] 一个
    /// 诚实的「什么都没选」出口，接线那票的 match 也因此是穷尽的。
    #[allow(dead_code)]
    Dismiss,
}

/// 一行：可执行项或分隔线。顺序照设计稿 B 卡。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuLine {
    Item(SessionMenuEntry),
    Separator,
}

/// 父菜单：打开 / 置顶 / ─── / 更多 › / 重命名 / 复制名字。
///
/// 置顶态只改文案（「置顶该聊天」↔「取消置顶」），不改形状 —— 所以这里不收 `pinned`。
pub fn top_lines() -> Vec<MenuLine> {
    vec![
        MenuLine::Item(SessionMenuEntry::Open),
        MenuLine::Item(SessionMenuEntry::TogglePin),
        MenuLine::Separator,
        MenuLine::Item(SessionMenuEntry::More),
        MenuLine::Item(SessionMenuEntry::Rename),
        MenuLine::Item(SessionMenuEntry::CopyName),
    ]
}

/// 「更多 ›」的子菜单：归档 / 撕原件 / ─── / 复制会话 id。
pub fn more_lines() -> Vec<MenuLine> {
    vec![
        MenuLine::Item(SessionMenuEntry::Archive),
        MenuLine::Item(SessionMenuEntry::Purge),
        MenuLine::Separator,
        MenuLine::Item(SessionMenuEntry::CopyId),
    ]
}

/// 看到的那行字。`pinned` 只影响置顶那一项。
pub fn label(entry: SessionMenuEntry, pinned: bool) -> &'static str {
    match entry {
        SessionMenuEntry::Open => "打开",
        SessionMenuEntry::TogglePin => {
            if pinned {
                "取消置顶"
            } else {
                "置顶该聊天"
            }
        }
        SessionMenuEntry::More => "更多",
        SessionMenuEntry::Rename => "重命名",
        SessionMenuEntry::CopyName => "复制名字",
        SessionMenuEntry::Archive => "归档该聊天…",
        SessionMenuEntry::Purge => "撕原件…",
        SessionMenuEntry::CopyId => "复制会话 id",
        SessionMenuEntry::Dismiss => "",
    }
}

/// 行尾的灰色副标题。`撕原件` 写清前置条件，但**不禁用** ——
/// 点下去该走归档流程或把引擎的 409 原文给用户，不是灰在那儿让人猜。
pub fn hint(entry: SessionMenuEntry) -> Option<&'static str> {
    match entry {
        SessionMenuEntry::Purge => Some("先归档"),
        _ => None,
    }
}

/// 条目 → 动作。`None` 只有一个来源：`More` 只开子菜单，不出动作。
pub fn entry_action(entry: SessionMenuEntry, target: &SessionTarget) -> Option<SessionMenuAction> {
    let id = target.id;
    let name = || target.name.clone();
    Some(match entry {
        SessionMenuEntry::Open => SessionMenuAction::Open { id },
        SessionMenuEntry::TogglePin => SessionMenuAction::TogglePin { id },
        SessionMenuEntry::More => return None,
        SessionMenuEntry::Rename => SessionMenuAction::Rename { id },
        SessionMenuEntry::CopyName => SessionMenuAction::CopyName { name: name() },
        SessionMenuEntry::Archive => SessionMenuAction::Archive { id, name: name() },
        SessionMenuEntry::Purge => SessionMenuAction::Purge { id, name: name() },
        SessionMenuEntry::CopyId => SessionMenuAction::CopyId { id },
        SessionMenuEntry::Dismiss => SessionMenuAction::Close,
    })
}

/// 把模型画进 `context_menu` 给的那只 `PopupMenu` 里。
///
/// `on_pick` 是调用方注入的出口（本体不知道 `Desktop` 长什么样）。子菜单要 `'static`，
/// 所以它是 `Clone` 的：每一项各拿一份。
pub fn build_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
    target: &SessionTarget,
    pinned: bool,
    on_pick: impl Fn(SessionMenuAction, &mut App) + Clone + 'static,
) -> PopupMenu {
    let mut menu = menu;

    for line in top_lines() {
        menu = match line {
            MenuLine::Separator => menu.separator(),
            // 「更多」：子菜单独立一层，父菜单不关（见模块头）。
            MenuLine::Item(SessionMenuEntry::More) => {
                let target = target.clone();
                let on_pick = on_pick.clone();
                menu.submenu(
                    label(SessionMenuEntry::More, pinned),
                    window,
                    cx,
                    move |sub, _window, _cx| {
                        let mut sub = sub;
                        for line in more_lines() {
                            sub = match line {
                                MenuLine::Separator => sub.separator(),
                                MenuLine::Item(entry) => {
                                    sub.item(item_for(entry, &target, pinned, on_pick.clone()))
                                }
                            };
                        }
                        sub
                    },
                )
            }
            MenuLine::Item(entry) => menu.item(item_for(entry, target, pinned, on_pick.clone())),
        };
    }

    menu
}

/// 一个可执行项。带副标题的（目前只有「撕原件…」）走自定义渲染 ——
/// 库里 `Item` 只有一行字，没地方挂副标题。
fn item_for<F>(
    entry: SessionMenuEntry,
    target: &SessionTarget,
    pinned: bool,
    on_pick: F,
) -> PopupMenuItem
where
    F: Fn(SessionMenuAction, &mut App) + 'static,
{
    let text = label(entry, pinned);
    let Some(action) = entry_action(entry, target) else {
        // 只有 `More` 会走到这儿（它由 `build_menu` 单独处理）。真到了这里说明
        // 模型和渲染对不上了，画成不可点的说明行而不是悄悄少一项。
        debug_assert!(false, "{entry:?} 没有动作，不该走 item_for");
        return PopupMenuItem::label(text);
    };

    let handler = move |_: &ClickEvent, _: &mut Window, cx: &mut App| on_pick(action.clone(), cx);

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
                        .text_color(rgb(t().p.faint))
                        .child(hint),
                )
                .into_any_element()
        })
        .on_click(handler)
        // 显式写出来：「撕原件…」永远可点，前置条件归引擎判（409 原文给用户看）。
        .disabled(false),
        None => PopupMenuItem::new(text).on_click(handler),
    }
}

#[cfg(test)]
mod tests {
    // 别写 `use super::*;` —— 那会把 `gpui_kit::*` 也带进来，
    // `#[test]` 展开时会在它的宏里撞上 recursion limit（踩过）。
    use super::{
        MenuLine, SessionMenuAction, SessionMenuEntry, SessionTarget, entry_action, hint, label,
        more_lines, top_lines,
    };

    fn target() -> SessionTarget {
        SessionTarget::new(302, "对话 302")
    }

    /// 票 #5 / 设计稿 B 卡的形状：打开 / 置顶 / ─── / 更多 › / 重命名 / 复制名字。
    #[test]
    fn top_menu_is_the_six_lines_from_the_design() {
        assert_eq!(
            top_lines(),
            vec![
                MenuLine::Item(SessionMenuEntry::Open),
                MenuLine::Item(SessionMenuEntry::TogglePin),
                MenuLine::Separator,
                MenuLine::Item(SessionMenuEntry::More),
                MenuLine::Item(SessionMenuEntry::Rename),
                MenuLine::Item(SessionMenuEntry::CopyName),
            ]
        );
    }

    /// 「更多 ›」的二级菜单：归档 / 撕原件 / ─── / 复制会话 id。
    #[test]
    fn more_submenu_is_three_items_with_a_separator() {
        assert_eq!(
            more_lines(),
            vec![
                MenuLine::Item(SessionMenuEntry::Archive),
                MenuLine::Item(SessionMenuEntry::Purge),
                MenuLine::Separator,
                MenuLine::Item(SessionMenuEntry::CopyId),
            ]
        );
    }

    #[test]
    fn labels_match_the_design_copy() {
        assert_eq!(label(SessionMenuEntry::Open, false), "打开");
        assert_eq!(label(SessionMenuEntry::More, false), "更多");
        assert_eq!(label(SessionMenuEntry::Rename, false), "重命名");
        assert_eq!(label(SessionMenuEntry::CopyName, false), "复制名字");
        assert_eq!(label(SessionMenuEntry::Archive, false), "归档该聊天…");
        assert_eq!(label(SessionMenuEntry::Purge, false), "撕原件…");
        assert_eq!(label(SessionMenuEntry::CopyId, false), "复制会话 id");
    }

    /// 已置顶时只有置顶那一行变「取消置顶」，别的行不许跟着动。
    #[test]
    fn pinned_only_flips_the_pin_item_label() {
        assert_eq!(label(SessionMenuEntry::TogglePin, false), "置顶该聊天");
        assert_eq!(label(SessionMenuEntry::TogglePin, true), "取消置顶");

        for entry in [
            SessionMenuEntry::Open,
            SessionMenuEntry::More,
            SessionMenuEntry::Rename,
            SessionMenuEntry::CopyName,
            SessionMenuEntry::Archive,
            SessionMenuEntry::Purge,
            SessionMenuEntry::CopyId,
        ] {
            assert_eq!(
                label(entry, true),
                label(entry, false),
                "{entry:?} 的文案不该跟着置顶态变"
            );
        }
    }

    /// 「撕原件…」永远是个能点的条目（不是禁用态）：副标题写清前置条件，
    /// 主列表里也**没有**「彻底删除」—— 那个只属于归档库页。
    #[test]
    fn purge_is_always_clickable_and_the_list_never_offers_irreversible_delete() {
        let lines: Vec<MenuLine> = top_lines().into_iter().chain(more_lines()).collect();
        let entries: Vec<SessionMenuEntry> = lines
            .into_iter()
            .filter_map(|line| match line {
                MenuLine::Item(entry) => Some(entry),
                MenuLine::Separator => None,
            })
            .collect();

        assert!(
            entries.contains(&SessionMenuEntry::Purge),
            "撕原件必须在菜单里，且是普通条目"
        );
        assert!(
            entries
                .iter()
                .all(|entry| !label(*entry, false).contains("彻底删除")),
            "会话列表的菜单里不许出现「彻底删除」"
        );
        assert_eq!(hint(SessionMenuEntry::Purge), Some("先归档"));
        assert_eq!(
            entry_action(SessionMenuEntry::Purge, &target())
                .map(|a| matches!(a, SessionMenuAction::Purge { .. })),
            Some(true),
            "撕原件要产出动作，不是摆设"
        );
    }

    /// 每个可执行项产出带会话身份的动作；id / 名字原样带过去。
    #[test]
    fn entries_map_to_actions_with_the_session_identity() {
        let target = target();
        let id = target.id;
        let name = target.name.clone();

        assert_eq!(
            entry_action(SessionMenuEntry::Open, &target),
            Some(SessionMenuAction::Open { id })
        );
        assert_eq!(
            entry_action(SessionMenuEntry::TogglePin, &target),
            Some(SessionMenuAction::TogglePin { id })
        );
        assert_eq!(
            entry_action(SessionMenuEntry::Rename, &target),
            Some(SessionMenuAction::Rename { id })
        );
        assert_eq!(
            entry_action(SessionMenuEntry::CopyName, &target),
            Some(SessionMenuAction::CopyName { name: name.clone() })
        );
        assert_eq!(
            entry_action(SessionMenuEntry::Archive, &target),
            Some(SessionMenuAction::Archive {
                id,
                name: name.clone()
            })
        );
        assert_eq!(
            entry_action(SessionMenuEntry::Purge, &target),
            Some(SessionMenuAction::Purge {
                id,
                name: name.clone()
            })
        );
        assert_eq!(
            entry_action(SessionMenuEntry::CopyId, &target),
            Some(SessionMenuAction::CopyId { id })
        );
    }

    /// 「更多」只是开子菜单，自己不出动作（出了就说明父菜单会被关掉）。
    #[test]
    fn more_opens_a_submenu_instead_of_acting() {
        assert_eq!(entry_action(SessionMenuEntry::More, &target()), None);
    }

    /// Esc / 点空白：整摞关掉，出口是 `Close`。
    #[test]
    fn dismiss_closes_the_whole_stack() {
        assert_eq!(
            entry_action(SessionMenuEntry::Dismiss, &target()),
            Some(SessionMenuAction::Close)
        );
    }
}
