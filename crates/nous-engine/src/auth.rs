//! token 注入的**唯一实现**：`NOUS_DESK_TOKEN` → 显式（`desktop.toml`）→ `~/.config/nousdesk/token`。
//!
//! 为什么单拎出来：`EngineClient` 和 `PinClient` 是两个 client，各拼一次 `Bearer` 就是
//! 「配置里有 token，置顶却 401」那类事故的温床（#10：置顶 401 → `load_pins` 静默降级 →
//! 界面上置顶等于没有）。顺序和 `crates/nousdesk/src/config.rs::resolve_token` 一个字不差。

use std::path::PathBuf;

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

/// 取值顺序：环境变量 → 显式传入（配置文件）→ token 文件。空串一律当没给。
///
/// 公开给桌面端的 `Config::resolve_token` 用：两边各写一份「env → toml → 文件」
/// 就会有一天只改了一边（`坑.md` 98）。
pub fn resolve(explicit: Option<&str>) -> Option<String> {
    let env = std::env::var("NOUS_DESK_TOKEN").ok();
    choose(env.as_deref(), explicit, token_from_file())
}

/// 纯决策，好测：谁先给出非空的就算谁的。
fn choose(env: Option<&str>, explicit: Option<&str>, file: Option<String>) -> Option<String> {
    fn norm(raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }
    env.and_then(norm)
        .or_else(|| explicit.and_then(norm))
        .or_else(|| file.and_then(|t| norm(&t)))
}

/// `Authorization: Bearer …` 头。没有 token 就空表 —— 引擎没配 token 时不该 401。
///
/// 两个 client 都把它挂到 `default_headers` 上，调用点谁也不许再自己拼 `Authorization`。
pub(crate) fn headers(explicit: Option<&str>) -> Result<HeaderMap, String> {
    bearer(resolve(explicit))
}

/// 纯部分：token → 头表。没有 token 就空表 —— 引擎没配 token 时不该 401。
fn bearer(token: Option<String>) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    let Some(token) = token else {
        return Ok(headers);
    };
    let value = format!("Bearer {token}")
        .parse::<HeaderValue>()
        .map_err(|e| format!("token 不能放进 HTTP 头：{e}"))?;
    headers.insert(AUTHORIZATION, value);
    Ok(headers)
}

fn token_from_file() -> Option<String> {
    let path = config_token_path();
    let text = std::fs::read_to_string(&path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(mode) = std::fs::metadata(&path).map(|m| m.permissions().mode())
            && mode & 0o777 != 0o600
        {
            eprintln!(
                "nousdesk: warning: token 文件 {} 权限不是 0600（建议 chmod 600）",
                path.display()
            );
        }
    }
    let token = text.trim().to_owned();
    (!token.is_empty()).then_some(token)
}

fn config_token_path() -> PathBuf {
    // 不依赖 `dirs` crate（engine 层保持轻）：Linux/BSD 用 XDG_CONFIG_HOME，
    // 其它平台退回 ~/.config；和桌面端 Config::resolve_token 同一路径。
    #[cfg(unix)]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    #[cfg(not(unix))]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("nousdesk").join("token")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 环境变量压过显式和文件() {
        let got = choose(
            Some(" env-token "),
            Some("toml-token"),
            Some("file-token".into()),
        );
        assert_eq!(got.as_deref(), Some("env-token"));
    }

    #[test]
    fn 空串当没给_继续往下找() {
        let got = choose(Some("   "), Some(""), Some("file-token".into()));
        assert_eq!(got.as_deref(), Some("file-token"));
    }

    #[test]
    fn 三个都没有就是没有() {
        assert_eq!(choose(None, None, None), None);
        assert_eq!(choose(Some(""), Some(" "), Some(String::new())), None);
    }

    #[test]
    fn 没有_token_就空头表() {
        assert!(bearer(None).unwrap().is_empty());
    }

    #[test]
    fn 有_token_就是_bearer() {
        let map = bearer(Some("t0ken".into())).unwrap();
        assert_eq!(
            map.get(AUTHORIZATION).and_then(|v| v.to_str().ok()),
            Some("Bearer t0ken")
        );
    }

    #[test]
    fn 放不进头的_token_报错而不是静默丢掉() {
        assert!(bearer(Some("bad\nvalue".into())).is_err());
    }
}
