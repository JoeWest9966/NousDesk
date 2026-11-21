//! presets 形状检查：清单文件挑选、触发器参数定型、校验报告展示、409 文案。
//!
//! 从 `editor.rs` 拆出来的**另一半**：这一半只认 `serde_json::Value` 与
//! `PresetFileMeta`，不认 `EditorState` / GPUI / Entity —— 所以 `#[test]` 能直接展开
//! （`EditorState` 带一堆 Entity，在 `#[test]` 里会撞递归上限，`坑.md` 68）。
//! 通用文档编辑器（状态、JSON 读写、diff / history / pending、渲染）仍在 `editor.rs`。

/// 从目录清单里挑出这一段的清单文件。
///
/// 段的 `manifest` 住在仓根的 `sections.json` 里（`Session-Template` = `preset.json`，
/// `Ultimate-Template` = `ultimate.json`），引擎不额外报这个名字 —— 所以照清单挑：
/// 先认那两个老名字，再退到「任意一个顶层的 .json」。

/// UTC 秒 → 本地时间 `11-16 10:41`。引擎收的 `epoch` 参数是 UTC 秒，
/// 让人心算是不可能的，所以填的时候就把人话摆在旁边。
pub(crate) fn local_time(secs: i64) -> String {
    match chrono::DateTime::from_timestamp(secs, 0) {
        Some(d) => d
            .with_timezone(&chrono::Local)
            .format("%m-%d %H:%M")
            .to_string(),
        None => String::new(),
    }
}

/// 把输入框里的字按触发器声明的类型变成 JSON 值 —— 写成字符串引擎不认。
pub(crate) fn typed_json(ty: &str, raw: &str) -> serde_json::Value {
    match ty {
        "epoch" | "integer" | "int" => raw
            .parse::<i64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or_else(|_| serde_json::json!(raw)),
        "number" | "float" => raw
            .parse::<f64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or_else(|_| serde_json::json!(raw)),
        "boolean" | "bool" => {
            serde_json::json!(matches!(raw, "true" | "1" | "是" | "yes" | "on"))
        }
        _ => serde_json::json!(raw),
    }
}

/// ISO 时间戳压成 `11-15 19:42`。认不出来就原样给（别因为格式怪就把那行藏了）。
pub(crate) fn stamp(at: &str) -> String {
    let s = at.trim();
    if s.len() < 16 || s.as_bytes().get(10) != Some(&b'T') {
        return s.to_string();
    }
    format!("{} {}", &s[5..10], &s[11..16])
}

/// 按路径取一份值（`model.provider` 这种嵌套）。
pub(crate) fn value_at<'a>(v: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    Some(cur)
}

/// 引擎给的错误/警告可能是字符串，也可能是 `{path, message}`。
pub(crate) fn issue_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other
            .get("message")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| other.to_string()),
    }
}

pub(crate) fn issue_where(v: &serde_json::Value) -> String {
    v.get("path")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// `resolved` 里一项变成一行字。
pub(crate) fn resolved_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => a
            .iter()
            .map(resolved_text)
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Object(o) => {
            if let Some(val) = o.get("value") {
                let state = o.get("state").and_then(|s| s.as_str()).unwrap_or("");
                let val = resolved_text(val);
                if state == "ok" {
                    val
                } else {
                    format!("{val}（{state}）")
                }
            } else {
                o.iter()
                    .map(|(k, v)| format!("{k}={}", resolved_text(v)))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
        other => other.to_string(),
    }
}

/// `If-Match` 过期的用户文案（判据 3）。`disk_etag` 是引擎回包里原样的磁盘版本，
/// 没给就不谎报；`message` 是 `{error}` 原文，不许翻译 / 改写。
pub(crate) fn conflict_text(disk_etag: Option<&str>, message: &str) -> String {
    match disk_etag {
        Some(etag) => format!("别人先改了这条（409）：磁盘版本 {etag} · {message}"),
        None => format!("别人先改了这条（409）：{message}"),
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{conflict_text, local_time, stamp, typed_json};

    #[test]
    fn params_are_typed_by_the_trigger_definition() {
        assert_eq!(typed_json("epoch", "1800000060"), serde_json::json!(1800000060));
        assert_eq!(typed_json("integer", "7"), serde_json::json!(7));
        assert_eq!(typed_json("number", "1.5"), serde_json::json!(1.5));
        assert_eq!(typed_json("boolean", "是"), serde_json::json!(true));
        assert_eq!(typed_json("boolean", "false"), serde_json::json!(false));
        // 认不出来的类型当文本，别把它弄丢。
        assert_eq!(typed_json("whatever", "abc"), serde_json::json!("abc"));
        // 数字类型填了非数字：退回字符串，至少不丢人的输入。
        assert_eq!(typed_json("epoch", "明天"), serde_json::json!("明天"));
    }

    /// UTC 秒要能翻成人话；翻不出来就空着（别画个假时间）。
    #[test]
    fn epoch_seconds_turn_into_local_time() {
        let s = local_time(1_800_000_060);
        assert_eq!(s.len(), 11, "MM-DD HH:MM 是 11 个字符，实际 {s:?}");
        assert_eq!(&s[2..3], "-");
        assert_eq!(&s[8..9], ":");
        assert_eq!(local_time(i64::MAX), "");
    }

    /// ISO 时间压成 `MM-DD HH:MM`；认不出就原样给（不藏那行）。
    #[test]
    fn stamps_are_shortened_but_never_dropped() {
        assert_eq!(stamp("2026-11-15T19:42:44+08:00"), "11-15 19:42");
        assert_eq!(stamp("2026-01-02T03:04:05Z"), "01-02 03:04");
        assert_eq!(stamp(""), "");
        assert_eq!(stamp("昨天"), "昨天");
    }

    /// 判据 3 的文案缝：409 要说「别人先改了」，并把磁盘版本原文带上。
    /// 没有磁盘版本也不许谎报一个。
    #[test]
    fn conflict_text_carries_disk_version() {
        assert_eq!(
            conflict_text(Some("r7"), "If-Match 不匹配"),
            "别人先改了这条（409）：磁盘版本 r7 · If-Match 不匹配"
        );
        assert_eq!(
            conflict_text(None, "no document writer"),
            "别人先改了这条（409）：no document writer"
        );
    }
}
