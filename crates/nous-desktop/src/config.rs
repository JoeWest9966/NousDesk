//! 配置：`~/.config/nous/desktop.toml`（或 `NOUS_DESKTOP_CONFIG` 指的文件）。
//!
//! 只有两个键。引擎地址**不**写这里 —— 它从宿主的 resolve 现拿。

use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    /// 宿主地址，可带路径前缀（如 `https://…/nous`）。
    pub host: String,
    /// 盘名。
    pub board: String,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let path = std::env::var("NOUS_DESKTOP_CONFIG")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::config_dir().map(|d| d.join("nous").join("desktop.toml")))
            .ok_or_else(|| "找不到配置目录（HOME 没设？）".to_string())?;

        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("读不了 {}：{e}", path.display()))?;
        let cfg: Config =
            toml::from_str(&text).map_err(|e| format!("{} 不是合法 toml：{e}", path.display()))?;
        if cfg.host.trim().is_empty() {
            return Err("host 不能空".into());
        }
        if cfg.board.trim().is_empty() {
            return Err("board 不能空".into());
        }
        Ok(cfg)
    }
}
