//! 409「别人先改了」的统一解析。
//!
//! 引擎的乐观锁（`If-Match` / `etag`）在冲突时回 409，body 带磁盘当前版本。
//! 这里只做**拆包**：`{error}` 原文 + 磁盘 `etag`，字段名按引擎实际回包
//! （`etag` / `ETag` / `version` 三个都认，别的原样透传）。
//!
//! 非 409 一律 `None` —— 401 / 500 / 网络错不许被说成「别人先改了」。

use serde_json::Value;

use crate::EngineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub status: u16,
    /// 引擎 `{error}` 原文，不许翻译 / 改写。
    pub message: String,
    /// 磁盘当前版本名（`etag`）。body 没给就是 `None`。
    pub disk_etag: Option<String>,
    /// body 里没被上面两个字段吃掉的其它键，原样留着（未知字段不丢）。
    pub extra: serde_json::Map<String, Value>,
}

/// 把 `EngineError::Engine { status: 409, .. }` 拆成 [`Conflict`]。
pub fn conflict_of(err: &EngineError) -> Option<Conflict> {
    match err {
        EngineError::Engine { status: 409, body, .. } => {
            let value = body
                .as_deref()
                .and_then(|b| serde_json::from_str::<Value>(b).ok());
            let disk_etag = value.as_ref().and_then(|v| {
                v.get("etag")
                    .or_else(|| v.get("ETag"))
                    .or_else(|| v.get("version"))
                    .and_then(|e| {
                        e.as_str()
                            .map(str::to_owned)
                            .or_else(|| e.as_u64().map(|n| n.to_string()))
                    })
            });
            let message = value
                .as_ref()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                .or_else(|| body.clone())
                .unwrap_or_default();
            let extra = value
                .as_ref()
                .and_then(|v| v.as_object())
                .map(|o| {
                    o.iter()
                        .filter(|(k, _)| {
                            !matches!(k.as_str(), "etag" | "ETag" | "version" | "error")
                        })
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default();
            Some(Conflict { status: 409, message, disk_etag, extra })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(status: u16, body: Option<&str>) -> EngineError {
        EngineError::Engine {
            status,
            message: String::new(),
            body: body.map(str::to_owned),
        }
    }

    #[test]
    fn 只有_409_才算冲突() {
        assert!(conflict_of(&err(401, None)).is_none());
        assert!(conflict_of(&err(500, Some("{}"))).is_none());
        // 409 但 body 不是 JSON 也照样给结论（message 用原文、无 etag），不崩。
        let c = conflict_of(&err(409, Some("not json"))).expect("409 要辨认");
        assert_eq!(c.message, "not json");
        assert!(c.disk_etag.is_none());
    }

    #[test]
    fn etag_三种键名与数字版本都认() {
        for (body, want) in [
            (r#"{"error":"stale","etag":"abc"}"#, "abc"),
            (r#"{"error":"stale","ETag":"abc"}"#, "abc"),
            (r#"{"error":"stale","version":"abc"}"#, "abc"),
            (r#"{"error":"stale","version":7}"#, "7"),
        ] {
            let c = conflict_of(&err(409, Some(body))).unwrap();
            assert_eq!(c.disk_etag.as_deref(), Some(want), "{body}");
        }
        // body 里没有版本名 → None，不许编一个。
        let c = conflict_of(&err(409, Some(r#"{"error":"stale"}"#))).unwrap();
        assert!(c.disk_etag.is_none());
    }

    #[test]
    fn 正文原样保留且未知字段不丢() {
        let c = conflict_of(&err(409, Some(r#"{"error":"别人先改了","disk":{"a":1}}"#))).unwrap();
        assert_eq!(c.message, "别人先改了");
        assert!(c.extra.contains_key("disk"));
    }
}
