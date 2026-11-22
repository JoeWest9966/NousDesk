//! 目录顶栏三个开关的纯逻辑测试。
//!
//! 单独一个文件：`nav_ui.rs` 的 GPUI 元素宏已经把 `#[test]` 的展开顶到
//! recursion_limit 了（同 `data_console_tests.rs` 的踩坑），拆开就没事。

use crate::nav_ui::{SessionNav, fold_targets};
use crate::session_nav::{NavDay, NavRound};

fn day(label: &str) -> NavDay {
    NavDay {
        label: label.into(),
        rounds: vec![NavRound {
            index: 0,
            entry_index: 0,
            text: "x".into(),
            at: None,
            tools: 0,
            codes: 0,
            after_read: false,
        }],
    }
}

#[test]
fn defaults_match_the_three_visual_switches() {
    let nav = SessionNav::new();
    assert!(nav.auto_reveal);
    assert!(nav.hover_preview);
    assert!(!fold_targets(&[day("周一"), day("周二")], None).is_empty());
}

#[test]
fn toggles_are_not_noops() {
    let mut nav = SessionNav::new();
    nav.toggle_auto_reveal();
    nav.toggle_hover_preview();
    assert!(!nav.auto_reveal);
    assert!(!nav.hover_preview);
    nav.toggle_auto_reveal();
    nav.toggle_hover_preview();
    assert!(nav.auto_reveal);
    assert!(nav.hover_preview);
}

#[test]
fn auto_reveal_can_be_disabled() {
    let mut nav = SessionNav::new();
    nav.closed.insert("周一".into());
    nav.reveal("周一");
    assert!(!nav.closed.contains("周一"));
    nav.closed.insert("周一".into());
    nav.auto_reveal = false;
    nav.reveal("周一");
    assert!(nav.closed.contains("周一"));
}

#[test]
fn fold_skips_last_and_current_groups() {
    let groups = [day("更早"), day("周一"), day("周二")];
    assert_eq!(fold_targets(&groups, None), vec!["更早", "周一"]);
    assert_eq!(fold_targets(&groups, Some("周一")), vec!["更早"]);
    assert!(fold_targets(&[], None).is_empty());
    assert!(fold_targets(&[day("只有一天")], None).is_empty());
}
