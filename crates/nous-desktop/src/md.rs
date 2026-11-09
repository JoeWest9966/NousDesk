//! markdown 的那些脏活：进渲染器前怎么加工、代码块怎么上色、复制按钮长什么样。
//!
//! 单独成文件是为了让 `examples/mdpreview` 能拿同一份代码 —— 样式这种东西
//! 必须看着改，样品间要是自己抄一遍，看到的就不是真的那个了。

use std::ops::Range;
use std::sync::{Arc, OnceLock};

use gpui_kit::base::text::{CodeBlock, TextViewStyle};
use gpui_kit::*;

/// 微信绿（复制按钮用）。md.rs 得能单独被 example 编进来，所以颜色自己带一份。
const COPY_GREEN: u32 = 0x07c160;

/// 聊天内容进渲染器前的一道加工：剥掉 harness 壳 + 把单换行变成硬换行。
pub fn chat_md(text: &str) -> String {
    hard_breaks(&strip_harness(text))
}

/// 单个换行在 markdown 里会被并成同一段，可聊天里「换行」是承诺 ——
/// 模型和自己都是随手敲回车断行，粘成一坨没法看。行尾补两个空格 = 硬换行。
///
/// 代码块里面不动：补空格就改了内容（复制出来的代码会带尾随空格）。
pub fn hard_breaks(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = String::with_capacity(text.len() + lines.len() * 2);
    let mut fence: Option<String> = None;
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        let marker: String = t.chars().take_while(|c| *c == '`' || *c == '~').collect();
        let is_fence_line = marker.chars().count() >= 3;
        if is_fence_line {
            match &fence {
                None => fence = Some(marker.clone()),
                Some(f) => {
                    // 闭合围栏要同一种字符、且不短于开头那串
                    if marker.starts_with(f.chars().next().unwrap()) && marker.len() >= f.len() {
                        fence = None;
                    }
                }
            }
        }
        out.push_str(line);
        // 最后一行不补：补了就会在文档末尾挂两个空格，复制出去还带着
        let more_to_come = i + 1 < lines.len();
        if more_to_come && fence.is_none() && !is_fence_line && !line.trim().is_empty() {
            out.push_str("  ");
        }
    }
    out
}

/// 代码块的「复制」按钮（markdown 里 ``` 围栏那块）。
pub fn code_copy_button(code: &CodeBlock, _w: &mut Window, _a: &mut App) -> AnyElement {
    let text = code.code().to_string();
    let key = hash_of(&text);
    // 语言名原本挂在代码块右上角，被我们这排按钮挤掉了 —— 自己补一个回去。
    // 有语言名才知道这是 bash 还是 python，也才知道高亮为什么长这样。
    let lang = code.lang().map(|l| l.to_string()).filter(|l| !l.is_empty());
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .children(lang.map(|l| {
            div()
                .text_size(px(11.))
                .text_color(rgb(0x9a9a9a))
                .child(l)
        }))
        .id(ElementId::Name(format!("code-copy-{key:x}").into()))
        .px(px(6.))
        .py(px(1.))
        .rounded(px(4.))
        .text_size(px(11.))
        .text_color(rgb(COPY_GREEN))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(0xe8f7ee)))
        .on_click(move |_: &ClickEvent, _w, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
            log::info!("复制了代码块：{} 字", text.len());
        })
        .child("复制")
        .into_any_element()
}

pub fn hash_of(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&s, &mut h);
    std::hash::Hasher::finish(&h)
}

type HlFn = dyn Fn(&CodeBlock) -> Vec<(Range<usize>, HighlightStyle)> + Send + Sync;

/// 代码块语法高亮：syntect 跑一遍，产出「字节区间 → 颜色」。
///
/// 三件要注意的事：
/// 1. 这回调**每帧**都会被调用（视图每帧重建），所以结果按「语言 + 内容哈希」
///    缓存在静态表里；命中就是一次哈希 + 查表，几万字的代码也不会卡。
/// 2. 认不出语言（没有 `lang`、或语法集里没这个 token）就如实不高亮 —— 不猜。
/// 3. 只取前景色，字号字重全交给主题：高亮不该改变排版的节奏。
pub fn code_highlighter() -> Arc<HlFn> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use syntect::easy::HighlightLines;
    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;
    use syntect::util::LinesWithEndings;

    type Cache = Mutex<HashMap<(String, u64), Arc<Vec<(Range<usize>, HighlightStyle)>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    // 挑最素的一个主题：高亮一热闹就比气泡还抢眼，聊天里它该退到背景去
    let theme = ts.themes["InspiredGitHub"].clone();

    Arc::new(move |block: &CodeBlock| {
        let lang = match block.lang() {
            Some(l) => l.to_string(),
            None => return Vec::new(),
        };
        let code = block.code().to_string();
        let key = (lang.clone(), hash_of(&code));
        if let Some(hit) = cache.lock().unwrap().get(&key) {
            return hit.as_ref().clone();
        }
        let syntax = match ss.find_syntax_by_token(&lang) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut hl = HighlightLines::new(syntax, &theme);
        let mut out: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut base = 0usize; // 当前行在整段代码里的起始字节
        for line in LinesWithEndings::from(code.as_str()) {
            let ranges = match hl.highlight_line(line, &ss) {
                Ok(r) => r,
                Err(_) => return Vec::new(),
            };
            let mut col = 0usize;
            for (style, piece) in ranges {
                let c = style.foreground;
                let argb =
                    ((c.r as u32) << 24) | ((c.g as u32) << 16) | ((c.b as u32) << 8) | 0xff;
                out.push((
                    base + col..base + col + piece.len(),
                    HighlightStyle {
                        color: Some(rgb(argb).into()),
                        ..Default::default()
                    },
                ));
                col += piece.len();
            }
            base += line.len();
        }
        let out = Arc::new(out);
        cache.lock().unwrap().insert(key, out.clone());
        out.as_ref().clone()
    })
}

/// 剥掉引擎给工具输出套的壳：`<harness>…</harness>`。
///
/// 壳里通常是 `[error] …` 这类标记。壳本身对用户没有信息量 —— 谁调的、调什么，
/// 卡片标题里已经说了；一眼看见的是「输出长什么样」。半截壳也要剥掉
/// （流式的时候经常只来了一半）。
pub fn strip_harness(raw: &str) -> String {
    let t = raw.trim();
    let t = t.strip_prefix("<harness>").unwrap_or(t);
    let t = t.strip_suffix("</harness>").unwrap_or(t);
    t.trim().to_string()
}

/// 聊天气泡里用的 markdown 样式。
///
/// 两处和默认值不一样，都是被真消息逼出来的：
/// 1. 段间距默认 0.75rem，聊天里显得松散 —— 一屏放不下三条消息，调紧到 0.42rem。
/// 2. 代码块**不折行、横向滚动**。折行会把缩进吃掉、把一行命令拆成两行，
///    长行还会把气泡撑宽（同一个病根：内容比容器大时容器扛不住）。
pub fn md_style() -> TextViewStyle {
    let mut code = StyleRefinement::default();
    code.overflow.x = Some(Overflow::Scroll);
    code.text.white_space = Some(WhiteSpace::Nowrap);
    TextViewStyle::default()
        .with_paragraph_gap(rems(0.42))
        .with_code_block(code)
}
