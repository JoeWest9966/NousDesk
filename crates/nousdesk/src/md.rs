//! markdown 的那些脏活：进渲染器前怎么加工、代码块怎么上色、复制按钮长什么样。
//!
//! 单独成文件是为了让 `examples/mdpreview` 能拿同一份代码 —— 样式这种东西
//! 必须看着改，样品间要是自己抄一遍，看到的就不是真的那个了。

// 主题接线：md.rs 既被 bin 引用，又被 mdpreview / mdstream 这两个 example
// 用 #[path] 单独引（它们的 crate root 里没有 mod theme / mod app）。
// 所以这里不引用 crate::theme，改为自带一份极简 Skin：bin 里 cur() 给它灌值，
// example 里就是出厂默认值。字段只覆盖本模块真正用到的那些。
#[derive(Clone, Copy)]
struct MdSkin { p: MdPalette, scale: f32, dark: bool }
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct MdPalette {
    accent: u32, muted: u32, accent_soft: u32, accent_fg: u32,
    code_bg: u32, line: u32, text: u32, text_2: u32, panel: u32,
}
impl MdSkin {
    fn fs(self, n: f32) -> gpui_kit::Pixels { gpui_kit::px(n * self.scale) }
    fn d(self, n: f32) -> gpui_kit::Pixels { gpui_kit::px(n) }
    fn r(self, n: f32) -> gpui_kit::Pixels { gpui_kit::px(n) }
}
fn t() -> MdSkin {
    // 不引用 crate::theme：mdpreview / mdstream 是 #[path] 单引本文件的 example，
    // 它们的 crate root 里没有 mod theme。bin 里由 `Desktop::render` 每帧调
    // `set_md_skin()` 把当前皮灌进来；example 没灌，就是出厂默认值。
    MD_SKIN.with(|cell| cell.get())
}

thread_local! {
    static MD_SKIN: std::cell::Cell<MdSkin> = const {
        std::cell::Cell::new(MdSkin {
            p: MdPalette {
                accent: 0x07c160, muted: 0x9a9a9a,
                accent_soft: 0xe8f7ee, accent_fg: 0x0a7a3f,
                code_bg: 0xf4f4f4, line: 0xe6e6e6,
                text: 0x1a1a1a, text_2: 0x333333, panel: 0xffffff,
            },
            scale: 1.0,
            dark: false,
        })
    };
}

/// bin 每帧调一次：把当前皮的 md 相关色灌进样品间共用的那份皮肤。
/// 参数是拆开的裸值，不引用 `crate::theme::Skin` —— example 的 crate root
/// 里没有那个模块，签名里出现它就编译不过。
#[allow(clippy::too_many_arguments, dead_code)]
pub fn set_md_skin(
    accent: u32, muted: u32, accent_soft: u32, accent_fg: u32,
    code_bg: u32, line: u32, text: u32, text_2: u32, panel: u32, scale: f32, dark: bool,
) {
    MD_SKIN.set(MdSkin {
        p: MdPalette { accent, muted, accent_soft, accent_fg, code_bg, line, text, text_2, panel },
        scale,
        dark,
    });
}
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use gpui_kit::base::text::{CodeBlock, TextViewStyle};
use gpui_kit::*;

/// 微信绿（复制按钮用）。md.rs 得能单独被 example 编进来，所以颜色自己带一份。
/// 原 `COPY_GREEN = 0x07c160`；由 accent 这个语义槽接管。
fn copy_green() -> u32 { t().p.accent }

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
        .gap(t().d(6.))
        .children(lang.map(|l| div().text_size(t().fs(11.)).text_color(rgb(t().p.muted)).child(l)))
        .id(ElementId::Name(format!("code-copy-{key:x}").into()))
        .px(t().d(6.))
        .py(t().d(1.))
        .rounded(t().r(4.))
        .text_size(t().fs(11.))
        .text_color(rgb(copy_green()))
        .cursor_pointer()
        .hover(|el| el.bg(rgb(t().p.accent_soft)))
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

/// 缓存上限。一行代码一条记录，不封顶就是流式期间的泄漏 ——
/// 一个长代码块流 30 秒能塞进去上千条完整高亮结果。
const HL_CACHE_MAX: usize = 64;

type HlRanges = Vec<(Range<usize>, HighlightStyle)>;
type HlKey = (String, u64);

struct HlCache {
    map: std::collections::HashMap<HlKey, Arc<HlRanges>>,
    /// 插入顺序，满了淘汰最旧的一条。
    order: std::collections::VecDeque<HlKey>,
}

struct Highlighter {
    ss: syntect::parsing::SyntaxSet,
    theme: syntect::highlighting::Theme,
    cache: std::sync::Mutex<HlCache>,
}

/// 全局唯一的一份。**只造一次**。
///
/// 这条踩过坑：一开始每次 `render` 都调 `code_highlighter()`，于是每帧都
/// `SyntaxSet::load_defaults_newlines()` —— 把几百 KB 的语法/正则全反序列化一遍，
/// 一帧几十毫秒，肉眼看到的就是「流式卡成幻灯片」。而且每次都是新的 `Arc`，
/// 框架靠 Arc 判等做的「这块没变就别重算」也一起失效了。
fn highlighter() -> &'static Highlighter {
    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;

    static HL: OnceLock<Highlighter> = OnceLock::new();
    HL.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        Highlighter {
            ss: SyntaxSet::load_defaults_newlines(),
            // 挑最素的一个主题：高亮一热闹就比气泡还抢眼，聊天里它该退到背景去
            theme: ts.themes["InspiredGitHub"].clone(),
            cache: std::sync::Mutex::new(HlCache {
                map: std::collections::HashMap::new(),
                order: std::collections::VecDeque::new(),
            }),
        }
    })
}

/// 把代码切成「已经稳定的部分」和「还在长的半行」。
///
/// 流式的时候最后一行每帧都在变。把整段当一段重新高亮，缓存键（内容哈希）就**每帧都变**，
/// 等于后面挂了张永远命不中的表，每帧全量跑一遍 syntect。
/// 按最后一个换行切一刀，稳定部分只在「多出一整行」时才变，命中率立刻上去。
fn split_stable(code: &str) -> (&str, &str) {
    match code.rfind('\n') {
        Some(i) => code.split_at(i + 1),
        None => ("", code),
    }
}

/// 一段代码跑一遍 syntect，产出「字节区间 → 前景色」。认不出语言就 `None`（不猜）。
fn highlight_one(code: &str, lang: &str, hl: &Highlighter) -> Option<Arc<HlRanges>> {
    use syntect::easy::HighlightLines;
    use syntect::util::LinesWithEndings;

    let syntax = hl.ss.find_syntax_by_token(lang)?;
    let mut h = HighlightLines::new(syntax, &hl.theme);
    let mut out: HlRanges = Vec::new();
    let mut base = 0usize; // 当前行在整段代码里的起始字节
    for line in LinesWithEndings::from(code) {
        let ranges = h.highlight_line(line, &hl.ss).ok()?;
        let mut col = 0usize;
        for (style, piece) in ranges {
            let c = style.foreground;
            let argb = ((c.r as u32) << 24) | ((c.g as u32) << 16) | ((c.b as u32) << 8) | 0xff;
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
    Some(Arc::new(out))
}

/// 带缓存的高亮：命中就返回**同一个 Arc**（框架拿它判「这块没变」），没命中才真跑。
fn hl_cached(code: &str, lang: &str) -> Option<Arc<HlRanges>> {
    if code.is_empty() {
        return Some(Arc::new(Vec::new()));
    }
    let hl = highlighter();
    let key: HlKey = (lang.to_string(), hash_of(code));
    if let Some(hit) = hl.cache.lock().unwrap().map.get(&key) {
        return Some(hit.clone());
    }
    let fresh = highlight_one(code, lang, hl)?;
    let mut c = hl.cache.lock().unwrap();
    while c.order.len() >= HL_CACHE_MAX {
        if let Some(old) = c.order.pop_front() {
            c.map.remove(&old);
        }
    }
    c.map.insert(key.clone(), fresh.clone());
    c.order.push_back(key);
    Some(fresh)
}

/// 代码块语法高亮：syntect 跑一遍，产出「字节区间 → 颜色」。
///
/// 四件要注意的事：
/// 1. 这回调**每帧**都会被调用（视图每帧重建），所以结果按「语言 + 内容哈希」缓存 ——
///    但只缓存**稳定的部分**（见 `split_stable`），流式时那条半行按单独一段算。
/// 2. 缓存有上限（`HL_CACHE_MAX`），否则流式期间一直插新键，内存只涨不跌。
/// 3. 认不出语言（没有 `lang`、或语法集里没这个 token）就如实不高亮 —— 不猜。
/// 4. 只取前景色，字号字重全交给主题：高亮不该改变排版的节奏。
///
/// 返回的 `Arc` 必须是**同一个**（`highlighter()` 是静态的），否则框架每帧都当它是新东西。
pub fn code_highlighter() -> Arc<HlFn> {
    static CB: OnceLock<Arc<HlFn>> = OnceLock::new();
    CB.get_or_init(|| {
        Arc::new(|block: &CodeBlock| {
            let Some(lang) = block.lang() else {
                return Vec::new();
            };
            let code = block.code().to_string();
            let (stable, tail) = split_stable(&code);

            let mut out: HlRanges = match hl_cached(stable, &lang) {
                Some(r) => r.as_ref().clone(),
                None => return Vec::new(), // 语言不认识，如实不上色
            };
            // 半行单独高亮：它自己的「行首状态」未必对（比如正处在一个长字符串里），
            // 但只影响这一行，看得过去；等它长完成为一个整行就并进稳定部分。
            if let Some(r) = hl_cached(tail, &lang) {
                let off = stable.len();
                for (range, style) in r.iter() {
                    out.push((off + range.start..off + range.end, *style));
                }
            }
            out
        })
    })
    .clone()
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
    let t = t();
    let mut code = StyleRefinement::default();
    code.overflow.x = Some(Overflow::Scroll);
    code.text.white_space = Some(WhiteSpace::Nowrap);
    // **不能直接用 `TextViewStyle::default()`** —— 那个是浅色（白底黑字）。
    // 深色皮上整块代码、行内代码会变成白砖，字还看不见（真踩过）。
    // 这里有几项就得从当前皮里拿几项。
    TextViewStyle::default()
        .with_paragraph_gap(rems(0.42))
        .with_code_block(code)
        .with_foreground(rgb(t.p.text).into())
        .with_muted_foreground(rgb(t.p.muted).into())
        .with_link(rgb(t.p.accent).into())
        .with_selection(rgb(t.p.accent_soft).into())
        .with_code_background(rgb(t.p.code_bg).into())
        .with_border(rgb(t.p.line).into())
        .with_inline_code(HighlightStyle {
            background_color: Some(rgb(t.p.accent_soft).into()),
            color: Some(rgb(t.p.accent_fg).into()),
            ..Default::default()
        })
        .with_dark(t.dark)
}

#[cfg(test)]
mod tests {
    use super::{HL_CACHE_MAX, code_highlighter, highlighter, hl_cached, split_stable};

    fn split(s: &str) -> (String, String) {
        let (stable, tail) = split_stable(s);
        (stable.to_string(), tail.to_string())
    }

    #[test]
    fn split_stable_只切出没写完的那半行() {
        // 正常情况：最后一个换行之后就是半行
        let (stable, tail) = split("a\nb\nccc");
        assert_eq!(stable, "a\nb\n");
        assert_eq!(tail, "ccc");

        // 刚好停在换行上：整段都稳定，半行为空
        let (_, tail) = split("a\nb\n");
        assert_eq!(tail, "");

        // 一行都没有（刚吐出第一个字符）：全算半行
        let (stable, tail) = split("abc");
        assert_eq!(stable, "");
        assert_eq!(tail, "abc");

        // 中文别切在半个字上（split_at 必须落在字符边界）
        let (stable, tail) = split("中文\n再一行");
        assert_eq!(stable, "中文\n");
        assert_eq!(tail, "再一行");
    }

    #[test]
    fn 认得出的语言真的上色_认不出的如实不上色() {
        let code = "fn main() {\n    let s = \"hi\";\n}\n";
        let ranges = hl_cached(code, "rust").expect("rust 该认得");
        assert!(!ranges.is_empty(), "rust 代码一个区间都没有 = 没高亮");
        // 至少得有两种颜色，否则「关键字和字符串同色」= 等于没高亮
        let mut colors: Vec<String> = ranges
            .iter()
            .filter_map(|(_, st)| st.color.map(|c| format!("{c:?}")))
            .collect();
        colors.sort();
        colors.dedup();
        assert!(colors.len() >= 2, "只有一种颜色，看不出来高亮：{colors:?}");

        // 语言不认识就不上色 —— 不猜
        assert!(hl_cached(code, "no-such-lang-xyz").is_none());
    }

    #[test]
    fn 同样的内容第二次命中缓存_拿到同一个_Arc() {
        let code = "let n = 42;\n";
        let a = hl_cached(code, "rust").expect("第一次");
        let b = hl_cached(code, "rust").expect("第二次");
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "缓存没命中：每秒几十帧全量跑 syntect 就是这么来的"
        );
        // 高亮器本身也必须是同一个实例（框架靠它判断「这块没变」）
        assert!(std::sync::Arc::ptr_eq(
            &code_highlighter(),
            &code_highlighter()
        ));
    }
    #[test]
    fn 缓存有上限_流式流久了也不会一直涨() {
        for i in 0..(HL_CACHE_MAX * 3) {
            let _ = hl_cached(&format!("let v{i} = {i};\n"), "rust");
        }
        let hl = highlighter();
        let n = hl.cache.lock().unwrap().map.len();
        assert!(n <= HL_CACHE_MAX, "缓存涨到 {n} 条，上限是 {HL_CACHE_MAX}");
    }
}
