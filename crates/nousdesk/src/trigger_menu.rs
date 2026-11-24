//! 触发器行的右键菜单（Trigger 页）。
//!
//! 和 `preset_menu.rs` 同一个分工：这里只放**菜单长什么样、点下去产出哪个动作**，
//! 不认引擎、不发 HTTP、不弹确认框。绘制走 gpui-kit 0.9 的 `ContextMenuExt` +
//! `PopupMenu`（翻边 / Esc / 点空白关掉都由库管）。
//!
//! 条目照模板页那套的骨架（`preset_menu::lines`）：动作 → 分隔 → 复制 → 分隔 → 归档。
//! 触发器有自己的动作（打开详情 / 改代码 / 改类别…），名字取自这一页屏幕上真有的东西，
//! 不发明没有端点支撑的项。

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;

/// 这一帧的皮。和 `preset_menu.rs` / `archive_menu.rs` 同一个写法。
fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 菜单作用于哪一行。触发器只有名字（目录名 = 二进制名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerTarget {
    pub name: String,
}

impl TriggerTarget {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// 菜单的出口。接线在 `triggers.rs` 的 `on_trigger_menu`：每一项都必须有可见结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerMenuAction {
    /// 打开右侧详情。
    Open { name: String },
    /// 打开 `src/main.rs`（代码编辑器那一层）。
    Code { name: String },
    /// 勾选类别（模板页同一个弹层形态）。
    ChangeCategory { name: String },
    /// 复制名字 —— 拿去跟人说话、写日志比复制目录路径常用。
    CopyName { name: String },
    /// 归档（可还原）。
    Archive { name: String },
}

/// 菜单里的一个可执行项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerMenuEntry {
    Open,
    Code,
    ChangeCategory,
    CopyName,
    Archive,
}

/// 一行：可执行项或分隔线。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuLine {
    Item(TriggerMenuEntry),
    Separator,
}

/// 菜单全文。顺序就是屏幕上从上到下的顺序：
/// 看（详情）/ 改（代码、类别）/ ─── / 拿（名字）/ ─── / 收（归档）。
pub fn lines() -> Vec<MenuLine> {
    vec![
        MenuLine::Item(TriggerMenuEntry::Open),
        MenuLine::Item(TriggerMenuEntry::Code),
        MenuLine::Item(TriggerMenuEntry::ChangeCategory),
        MenuLine::Separator,
        MenuLine::Item(TriggerMenuEntry::CopyName),
        MenuLine::Separator,
        MenuLine::Item(TriggerMenuEntry::Archive),
    ]
}

/// 看到的那行字。
pub fn label(entry: TriggerMenuEntry) -> &'static str {
    match entry {
        TriggerMenuEntry::Open => "打开详情",
        TriggerMenuEntry::Code => "改代码",
        TriggerMenuEntry::ChangeCategory => "改类别…",
        TriggerMenuEntry::CopyName => "复制名字",
        TriggerMenuEntry::Archive => "归档…",
    }
}

/// 行尾的灰色副标题。没副标题就不挂那一格。
pub fn hint(entry: TriggerMenuEntry) -> Option<&'static str> {
    match entry {
        TriggerMenuEntry::Archive => Some("可还原"),
        TriggerMenuEntry::ChangeCategory => Some("可多选"),
        _ => None,
    }
}

/// 条目 → 动作。
pub fn entry_action(entry: TriggerMenuEntry, target: &TriggerTarget) -> TriggerMenuAction {
    let name = || target.name.clone();
    match entry {
        TriggerMenuEntry::Open => TriggerMenuAction::Open { name: name() },
        TriggerMenuEntry::Code => TriggerMenuAction::Code { name: name() },
        TriggerMenuEntry::ChangeCategory => TriggerMenuAction::ChangeCategory { name: name() },
        TriggerMenuEntry::CopyName => TriggerMenuAction::CopyName { name: name() },
        TriggerMenuEntry::Archive => TriggerMenuAction::Archive { name: name() },
    }
}

/// 把模型画进 `context_menu` 给的那只 `PopupMenu` 里。
///
/// `on_pick` 是调用方注入的出口（本体不知道 `Desktop` 长什么样）。
pub fn build_menu(
    menu: PopupMenu,
    _window: &mut Window,
    _cx: &mut Context<PopupMenu>,
    target: &TriggerTarget,
    on_pick: impl Fn(TriggerMenuAction, &mut Window, &mut App) + Clone + 'static,
) -> PopupMenu {
    let mut menu = menu;
    for line in lines() {
        menu = match line {
            MenuLine::Separator => menu.separator(),
            MenuLine::Item(entry) => menu.item(item_for(entry, target, on_pick.clone())),
        };
    }
    menu
}

/// 一个可执行项。带副标题的走自定义渲染 —— 库里 `Item` 只有一行字。
fn item_for<F>(entry: TriggerMenuEntry, target: &TriggerTarget, on_pick: F) -> PopupMenuItem
where
    F: Fn(TriggerMenuAction, &mut Window, &mut App) + 'static,
{
    let text = label(entry);
    let action = entry_action(entry, target);
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
                        .text_color(if matches!(entry, TriggerMenuEntry::Archive) {
                            rgb(t().p.danger)
                        } else {
                            rgb(t().p.faint)
                        })
                        .child(hint),
                )
                .into_any_element()
        })
        .on_click(handler),
        None => PopupMenuItem::new(text).on_click(handler),
    }
}

#[cfg(test)]
mod tests {
    // 别 `use super::*`：文件顶部那份 `gpui_kit::*` 里也有个 `test` 宏，
    // glob 进来会把 `#[test]` 抢走（报「recursion limit reached while expanding #[test]」）。
    use super::{
        entry_action, hint, label, lines, MenuLine, TriggerMenuAction, TriggerMenuEntry,
        TriggerTarget,
    };

    #[test]
    fn every_item_has_a_label_and_an_action() {
        for line in lines() {
            let MenuLine::Item(entry) = line else { continue };
            assert!(!label(entry).is_empty(), "{entry:?} 没字");
            let target = TriggerTarget::new("dep-alert");
            match entry_action(entry, &target) {
                TriggerMenuAction::Open { name }
                | TriggerMenuAction::Code { name }
                | TriggerMenuAction::ChangeCategory { name }
                | TriggerMenuAction::CopyName { name }
                | TriggerMenuAction::Archive { name } => assert_eq!(name, "dep-alert"),
            }
        }
    }

    #[test]
    fn archive_is_the_only_destructive_item_and_the_last_one() {
        let items: Vec<TriggerMenuEntry> = lines()
            .into_iter()
            .filter_map(|line| match line {
                MenuLine::Item(entry) => Some(entry),
                MenuLine::Separator => None,
            })
            .collect();
        assert_eq!(items.last().copied(), Some(TriggerMenuEntry::Archive));
        assert_eq!(
            items
                .iter()
                .filter(|entry| hint(**entry).is_some_and(|h| h == "可还原"))
                .count(),
            1
        );
    }
}
