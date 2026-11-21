//! **空 / 在等 / 坏了** —— 全 app 只有这一处画它们。
//!
//! 三种状态用户看到的都是「这块空着」，但该做的事完全不同：
//!
//! | 状态 | 意思 | 用户该做什么 | 我们给什么 |
//! |---|---|---|---|
//! | [`Phase::Loading`] | 请求出去了，还没回来 | 别动，等一下 | 一行灰字说清**在等什么** |
//! | [`Phase::Empty`] | 真的没有 | 去建一个 / 去开一个 | 一句为什么 + 一个动作 |
//! | [`Phase::Failed`] | 回来了，但失败 | 重试 | 原样错误 + 重试 |
//!
//! ## 为什么要有这个文件
//!
//! 之前四个模块各写各的：`resources.rs` 的 `loading_line()` 把「在等」和「空」挤进同一个
//! 函数（`"拉清单…"` / `"没有数据"`）、`templates.rs` 有 `note()` 和 `empty_hint()` 两套、
//! `data_console.rs` 只画红字不给重试、会话列表干脆没有空态（白板一片）。
//! 后果不是「不好看」，是**用户没法判断这块是空、在等、还是坏了** —— 三种的出路不一样。
//!
//! 现在统一走这里：字号 / 内距 / 颜色 / 有没有动作只在一处定，改一次全 app 跟着变。
//!
//! ## 规矩
//!
//! - **文案必须具体**：`what` 写「正在等引擎回技能清单」，不许写「加载中」；
//!   `why` 写「这个引擎还没装 presets 模块」，不许写「暂无数据」。
//! - **只有 [`Phase::Empty`] 和 [`Phase::Failed`] 能挂动作**（[`Phase::Loading`] 挂动作等于
//!   让人点了又点）。`view()` 收到 `Loading` 时会把 action 丢掉，不靠调用方自律。
//! - 出现在列表 / 表格里那种窄地方用 [`inline()`]（一行，不换行，不占半屏）。

use gpui_kit::*;

fn t() -> crate::theme::Skin {
    crate::theme::cur()
}

/// 一块界面此刻的三态之一。**只有三种** —— 想加第四种先问自己是不是把「空」拆错了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Phase {
    /// 在等：说清**在等什么**（`"正在等引擎回技能清单"`），不是「加载中」。
    Loading { what: String },
    /// 空：一句为什么 + 可选的下一步。
    Empty { head: String, why: Option<String> },
    /// 坏了：原样错误，不翻译、不加前缀；重试由调用方给。
    Failed { error: String },
}

impl Phase {
    pub(crate) fn loading(what: impl Into<String>) -> Self {
        Self::Loading { what: what.into() }
    }

    pub(crate) fn empty(head: impl Into<String>, why: Option<impl Into<String>>) -> Self {
        Self::Empty { head: head.into(), why: why.map(Into::into) }
    }

    pub(crate) fn failed(error: impl std::fmt::Display) -> Self {
        Self::Failed { error: error.to_string() }
    }
}

/// 语义色调。测试钉它，免得哪天「失败」被画成灰的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    /// 在等 / 空：灰。
    Quiet,
    /// 坏了：红。
    Bad,
}

impl Phase {
    /// 主那一行。三态**必须不同**，否则用户分不出来。
    pub(crate) fn head(&self) -> String {
        match self {
            Self::Loading { what } => what.clone(),
            Self::Empty { head, .. } => head.clone(),
            Self::Failed { error } => error.clone(),
        }
    }

    /// 副那一行（只有空态有）。
    pub(crate) fn detail(&self) -> Option<String> {
        match self {
            Self::Empty { why, .. } => why.clone(),
            _ => None,
        }
    }

    pub(crate) fn tone(&self) -> Tone {
        match self {
            Self::Failed { .. } => Tone::Bad,
            _ => Tone::Quiet,
        }
    }

    /// 能不能挂动作：在等不许挂（点了又点就是这么来的）。
    pub(crate) fn takes_action(&self) -> bool {
        !matches!(self, Self::Loading { .. })
    }
}

fn tone_color(tone: Tone) -> u32 {
    match tone {
        Tone::Quiet => t().p.muted,
        Tone::Bad => t().p.danger,
    }
}

/// 空白区里的一块三态。`action` 只在 `Empty` / `Failed` 时生效。
///
/// 左对齐、不居中 —— 这个 app 的留白在左，居中会让每一页的「空」看起来像另一个 app。
pub(crate) fn view(phase: Phase, action: Option<AnyElement>) -> AnyElement {
    let tone = phase.tone();
    let action = if phase.takes_action() { action } else { None };
    let mut block = div()
        .flex()
        .flex_col()
        .items_start()
        .gap(t().d(6.))
        .px(t().d(16.))
        .py(t().d(1.5 * 12.))
        .child(
            div()
                .text_size(t().fs(12.5))
                .text_color(rgb(tone_color(tone)))
                .child(phase.head()),
        );
    if let Some(detail) = phase.detail() {
        block = block.child(
            div()
                .text_size(t().fs(11.5))
                .text_color(rgb(t().p.faint))
                .child(detail),
        );
    }
    if let Some(action) = action {
        block = block.child(div().mt(t().d(6.)).child(action));
    }
    block.into_any_element()
}

/// 列表 / 表格里那一行用这个：一行字，不换行，不吃半屏。
pub(crate) fn inline(phase: Phase) -> AnyElement {
    let mut row = div().flex().flex_col().gap(t().d(3.)).px(t().d(12.)).py(t().d(10.)).child(
        div()
            .text_size(t().fs(11.5))
            .text_color(rgb(tone_color(phase.tone())))
            .child(phase.head()),
    );
    if let Some(detail) = phase.detail() {
        // 窄栏也给「为什么」：只写一句「空」等于让人自己猜。
        row = row.child(div().text_size(t().fs(11.)).text_color(rgb(t().p.faint)).child(detail));
    }
    row.into_any_element()
}

/// 一个「重试」按钮。全 app 只有这一个长相（`templates.rs` / `triggers.rs` 以前各画一个）。
pub(crate) fn retry_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id(ElementId::Name(id.into()))
        .cursor_pointer()
        .px(t().d(10.))
        .py(t().d(4.))
        .rounded(t().r(6.))
        .text_size(t().fs(11.5))
        .text_color(rgb(t().p.link))
        .hover(|s| s.bg(rgb(t().p.hover)))
        .child(label)
        .on_click(on_click)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    // 这里**不能**写 `use super::*`：`states.rs` 顶部 `use gpui_kit::*` 是 glob，
    // glob 再 glob 会把 `#[test]` 的宏展开顶到 recursion_limit（实测报「reached」）。
    // 只拿需要的那两个名字。
    use super::{Phase, Tone};

    fn loading() -> Phase {
        Phase::loading("正在等引擎回技能清单")
    }
    fn empty() -> Phase {
        Phase::empty("还没建过会话", Some("右边那颗 ＋ 建一条，它会出现在这儿"))
    }
    fn failed() -> Phase {
        Phase::failed("HTTP 500：store 打不开")
    }

    #[test]
    fn three_phases_read_differently() {
        // 用户靠这一行字区分「空 / 在等 / 坏了」，三句话不许一样。
        let heads = [loading().head(), empty().head(), failed().head()];
        assert_eq!(heads[0], "正在等引擎回技能清单");
        assert_eq!(heads[1], "还没建过会话");
        assert_eq!(heads[2], "HTTP 500：store 打不开");
        assert_ne!(heads[0], heads[1]);
        assert_ne!(heads[1], heads[2]);
    }

    #[test]
    fn only_failed_is_red() {
        assert_eq!(failed().tone(), Tone::Bad);
        assert_eq!(empty().tone(), Tone::Quiet, "空不是错，别画成红的");
        assert_eq!(loading().tone(), Tone::Quiet);
    }

    #[test]
    fn loading_never_takes_an_action() {
        // 「在等」挂重试 = 让人点了又点，这是要拦住的。
        assert!(!loading().takes_action());
        assert!(empty().takes_action());
        assert!(failed().takes_action());
    }

    #[test]
    fn detail_belongs_to_empty_only() {
        assert_eq!(empty().detail().as_deref(), Some("右边那颗 ＋ 建一条，它会出现在这儿"));
        assert_eq!(loading().detail(), None, "在等不该有两行");
        assert_eq!(failed().detail(), None);
    }
}
