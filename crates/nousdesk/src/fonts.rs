//! 随包字体：注册一次，之后 `Skin.font` 才认得那些族名。

/// 随包发的字体。**为什么要有这一层**：`Skin.font` 只能是 GPUI 认得的
/// 真实族名，而 Tesla 那几套皮用的是 Inter / Manrope / Archivo / IBM Plex。
/// 系统里没装这些名字时，`font_family("Inter")` 会静静退回默认字体 ——
/// 换皮就只换了颜色。所以把它们嵌进二进制、启动时 `add_fonts` 注册一遍。
///
/// 只嵌**拉丁子集**（每套 3–5 个字重，合计 1.7 MB）；中文走系统的
/// Noto Sans CJK，cosmic_text 会自动做字形回退。
///
/// 这些 ttf 由 `demo/tesla-themes/fonts/*.woff2` 转出来（woff2 是 brotli 压缩的
/// sfnt，fontdb 不认）。转一次就够，别在构建期做 —— 那要多一条字体工具链依赖。
const BUNDLED_FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/archivo-400.ttf"),
    include_bytes!("../../../assets/fonts/archivo-500.ttf"),
    include_bytes!("../../../assets/fonts/archivo-600.ttf"),
    include_bytes!("../../../assets/fonts/archivo-700.ttf"),
    include_bytes!("../../../assets/fonts/ibmplexsans-300.ttf"),
    include_bytes!("../../../assets/fonts/ibmplexsans-400.ttf"),
    include_bytes!("../../../assets/fonts/ibmplexsans-500.ttf"),
    include_bytes!("../../../assets/fonts/ibmplexsans-600.ttf"),
    include_bytes!("../../../assets/fonts/inter-300.ttf"),
    include_bytes!("../../../assets/fonts/inter-400.ttf"),
    include_bytes!("../../../assets/fonts/inter-500.ttf"),
    include_bytes!("../../../assets/fonts/inter-600.ttf"),
    include_bytes!("../../../assets/fonts/inter-700.ttf"),
    include_bytes!("../../../assets/fonts/jetbrainsmono-400.ttf"),
    include_bytes!("../../../assets/fonts/jetbrainsmono-500.ttf"),
    include_bytes!("../../../assets/fonts/manrope-400.ttf"),
    include_bytes!("../../../assets/fonts/manrope-500.ttf"),
    include_bytes!("../../../assets/fonts/manrope-600.ttf"),
    include_bytes!("../../../assets/fonts/manrope-700.ttf"),
];

/// 把随包字体注册进 GPUI 的文本系统。启动时调一次；失败只记日志（字体没了
/// 不该挡住启动，最坏是那套皮的字回退成系统字体）。
pub fn install_fonts(cx: &gpui_kit::App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = BUNDLED_FONTS
        .iter()
        .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    match cx.text_system().add_fonts(fonts) {
        Ok(()) => log::info!("字体：注册了 {} 个（Inter / Manrope / Archivo / IBM Plex / JetBrains Mono）", BUNDLED_FONTS.len()),
        Err(e) => log::warn!("字体注册失败（换皮时会回退到系统字体）：{e}"),
    }
}
