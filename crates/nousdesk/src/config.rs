//! 配置：`~/.config/nousdesk/desktop.toml`（或 `NOUS_DESKTOP_CONFIG` 指的文件）。
//!
//! 只有两个键：`host`（引擎/宿主的那一个地址）和可选 `token`。
//! 引擎地址**不**从别处推导 —— 目标态就是这一个地址 + `/v1/capabilities`。
//!
//! **文件不存在时不报错，用默认值。** 装好就能开，别让用户先去手写 toml。
//! （默认值是「这台机器上的 Nous」，换机器就改那一份文件。）

use std::path::PathBuf;

/// 用户配置 / 日志的目录名（`%APPDATA%\nousdesk\`、`%LOCALAPPDATA%\nousdesk\`）。
pub const APP_DIR: &str = "nousdesk";

/// 改名前的目录名（`nous`）。**只读不写**：老用户的 `desktop.toml` 还得能读到。
pub const LEGACY_APP_DIR: &str = "nous";

/// 没有配置文件时用的默认宿主。这是**宿主**地址，不是引擎地址。
///
/// 默认指向本机 —— 对任何跑着 Nous 的机器都成立。走 tailnet / 反代 / 域名的
/// 写进 `desktop.toml`，别改这里（这里是所有人共用的默认值）。
pub const DEFAULT_HOST: &str = "http://192.168.1.7:4790";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
    /// 引擎地址，可带路径前缀（如 `https://…/nous`）。
    pub host: String,
    /// 可选 token。取值顺序：环境变量 → 这里 → `~/.config/nousdesk/token`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            token: None,
        }
    }
}

impl Config {
    /// 读配置。一个都不存在 → 用默认值（不是错误）。存在但写坏了 → 报错，别静默用默认。
    ///
    /// 查找顺序（先到先得）：
    /// 1. `NOUS_DESKTOP_CONFIG` —— 调试用
    /// 2. **exe 旁边**的 `desktop.toml` —— 便携式。发布包里就带一份，解压即用
    /// 3. `%APPDATA%\nousdesk\desktop.toml`（Linux 是 `~/.config`）—— 装机后的个人配置
    ///    4. `%APPDATA%\nous\desktop.toml` —— 改名前的旧目录，只用来接住老用户
    pub fn load() -> Result<Self, String> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };

        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("读不了 {}：{e}", path.display())),
        };

        // 老配置里的 `board` 不能让升级后的客户端开不起来：先当通用 toml 读，
        // 警告（stderr + 日志）后忽略这个键，再按现在的形状解析。
        if let Ok(raw) = toml::from_str::<toml::Value>(&text)
            && raw.get("board").is_some()
        {
            eprintln!(
                "nousdesk: warning: {} 里的 board 已废弃（现在只有一个地址），已忽略",
                path.display()
            );
            log::warn!("{} 里的 board 已废弃，已忽略", path.display());
        }
        let cfg: Config =
            toml::from_str(&text).map_err(|e| format!("{} 不是合法 toml：{e}", path.display()))?;
        if cfg.host.trim().is_empty() {
            return Err(format!("{} 里的 host 是空的", path.display()));
        }
        Ok(cfg)
    }

    /// token 的唯一解析顺序：环境变量 → `desktop.toml` → `~/.config/nousdesk/token`。
    ///
    /// 文件兜底是为了便携包那份只读 `desktop.toml`；权限不是 0600 只警告、不拦。
    pub fn resolve_token(&self) -> Option<String> {
        if let Ok(token) = std::env::var("NOUS_DESK_TOKEN")
            && !token.trim().is_empty()
        {
            return Some(token.trim().to_string());
        }
        if let Some(token) = self.token.as_deref()
            && !token.trim().is_empty()
        {
            return Some(token.trim().to_string());
        }
        let path = dirs::config_dir()?.join(APP_DIR).join("token");
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
        let token = text.trim().to_string();
        (!token.is_empty()).then_some(token)
    }

    /// 实际用的那一个配置文件（不存在就是 None，会用默认值）。
    pub fn path() -> Option<PathBuf> {
        Self::candidates().into_iter().find(|p| p.is_file())
    }

    /// 保存会写到哪个文件：**当前生效的那份**；一份都没有就新建用户配置那一级。
    ///
    /// 写「生效的那份」而不是永远写用户配置：便携包的 `desktop.toml` 排在用户配置之前，
    /// 写死 `%APPDATA%` 的话用户改完保存会发现毫无变化。
    pub fn save_target() -> Result<PathBuf, String> {
        if let Some(path) = Self::path() {
            return Ok(path);
        }
        dirs::config_dir()
            .map(|dir| dir.join(APP_DIR).join("desktop.toml"))
            .ok_or_else(|| "找不到配置目录（%APPDATA%）".to_string())
    }

    /// 把这两个键写回去。**不会**顺手拉别的键进来 —— 这个文件就只有这两行。
    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Self::save_target()?;
        self.save_to(&path)?;
        Ok(path)
    }

    /// 写到指定文件（父目录不存在就建）。分成两步是为了能直接测，不用动环境变量。
    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("建不了目录 {}：{e}", dir.display()))?;
        }
        let text = toml::to_string(self).map_err(|e| format!("写不出来：{e}"))?;
        std::fs::write(path, text).map_err(|e| format!("写不了 {}：{e}", path.display()))
    }

    /// 所有候选路径，按优先级。exe 旁边那份排在用户配置之前 —— 便携包要能覆盖掉旧配置。
    pub fn candidates() -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(explicit) = std::env::var("NOUS_DESKTOP_CONFIG") {
            out.push(PathBuf::from(explicit));
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            out.push(dir.join("desktop.toml"));
        }
        if let Some(config) = dirs::config_dir() {
            out.push(config.join(APP_DIR).join("desktop.toml"));
            // 改名前的目录：排在后面，新的那份一存在就盖过它。
            out.push(config.join(LEGACY_APP_DIR).join("desktop.toml"));
        }
        out
    }
}

/// 验证实例的日志文件名。
///
/// 默认 `desktop.log` —— 发布版一个字都不变。**只有**显式设了 `NOUS_INSTANCE`
/// 才换成 `desktop-<id>.log`（轮转也带 id）。并行开发时每人一个实例、一份日志：
/// 别的会话按进程名清理不会误杀（那靠二进制改名，见 `坑.md 83`），日志也不会
/// 几个实例互相追加、查不出哪行是谁写的。
///
/// id 只取 `[A-Za-z0-9_-]`，别的字符一律换成 `-`：它要进文件名，别让 env 里的
/// 空格 / 斜杠 / 点把路径搞坏（`../` 那种更不能进）。空 id 当成没设。
pub fn log_file_name() -> &'static str {
    // env 只读一次：日志路径在进程启动时定下来，运行中不该变。
    // 缓存 &'static str 是为了给 open_log_file 省掉一次分配和生命周期参数。
    use std::sync::OnceLock;
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        log_file_name_for(std::env::var("NOUS_INSTANCE").ok().as_deref())
    })
    .as_str()
}

/// 纯函数：给定原始 id，算出日志文件名。单独拎出来是为了能直接测，不碰 env。
///
/// id 只取 `[A-Za-z0-9_-]`，别的字符一律换成 `-`：它要进文件名，别让 env 里的
/// 空格 / 斜杠 / 点把路径搞坏（`../` 那种更不能进）。空 / 没有 → `desktop.log`。
fn log_file_name_for(raw: Option<&str>) -> String {
    match raw {
        Some(id) if !id.is_empty() => {
            let clean: String = id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect();
            format!("desktop-{clean}.log")
        }
        _ => "desktop.log".to_string(),
    }
}

/// 轮转名：`desktop.log` → `desktop.log.1`；`desktop-x.log` → `desktop-x.log.1`。
pub fn log_rotated_name() -> String {
    format!("{}.1", log_file_name())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 改环境变量的测试得串行：`candidates()` 现读 `dirs::config_dir()`。
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 写回去的文件必须**只有** host 一个键 —— 多出来的键会让「配置文件就一个键」变成假的。
    #[test]
    fn save_writes_only_host() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("nousdesk-cfg-{}", std::process::id()));
        let path = dir.join("nested").join("desktop.toml");
        let cfg = Config {
            host: "http://example.test:4790".into(),
            token: None,
        };

        cfg.save_to(&path).expect("写得进去（父目录会自己建）");
        let text = std::fs::read_to_string(&path).expect("读得回来");

        assert!(
            text.contains("host = \"http://example.test:4790\""),
            "host 没写对：{text}"
        );
        assert_eq!(
            text.lines().filter(|l| !l.trim().is_empty()).count(),
            1,
            "不该有第二个键：{text}"
        );

        let back: Config = toml::from_str(&text).expect("还是合法的 toml");
        assert_eq!(back.host, cfg.host);
        assert_eq!(back.token, cfg.token);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// token 顺序：env > desktop.toml > 用户目录的 token 文件（0600 只警告）。
    #[test]
    fn token_order_env_then_toml_then_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!("nousdesk-token-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join(APP_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "from-file").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join("token"), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &root);
            std::env::set_var("APPDATA", &root);
            std::env::remove_var("NOUS_DESK_TOKEN");
        }
        let cfg = Config { host: DEFAULT_HOST.into(), token: None };
        assert_eq!(cfg.resolve_token().as_deref(), Some("from-file"));
        let cfg = Config { host: DEFAULT_HOST.into(), token: Some("from-toml".into()) };
        assert_eq!(cfg.resolve_token().as_deref(), Some("from-toml"));
        unsafe { std::env::set_var("NOUS_DESK_TOKEN", "from-env") };
        assert_eq!(cfg.resolve_token().as_deref(), Some("from-env"));
        unsafe {
            std::env::remove_var("NOUS_DESK_TOKEN");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("APPDATA");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 老便携包里的 `board` 只警告、不拦启动，也不进写回去的文件。
    #[test]
    fn legacy_board_is_warned_and_ignored() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("nousdesk-board-{}", std::process::id()));
        let path = dir.join("desktop.toml");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            "host = \"http://old.test:1\"\nboard = \"new1\"\n",
        )
        .unwrap();

        // SAFETY: 本模块测试用 ENV_LOCK 串行；这里只改这一个测试进程的环境。
        unsafe { std::env::set_var("NOUS_DESKTOP_CONFIG", &path) };
        let cfg = Config::load().expect("老配置必须还能启动");
        unsafe { std::env::remove_var("NOUS_DESKTOP_CONFIG") };
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cfg.host, "http://old.test:1");
    }

    /// 改名迁移：旧的 `%APPDATA%\nous\desktop.toml` 还得读得到，新的那份一出现就盖过它。
    ///
    /// 这条钉的是改名时最容易伤的路径 —— `%APPDATA%\nous` → `%APPDATA%\nousdesk`，
    /// 老用户不该因为换了个名字就被打回默认值（打进内网 IP 才怪）。
    #[test]
    fn legacy_config_dir_is_still_read_and_new_one_wins() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!("nousdesk-migrate-{}", std::process::id()));
        let legacy = root.join(LEGACY_APP_DIR);
        let current = root.join(APP_DIR);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("desktop.toml"),
            "host = \"http://old.test:1\"\nboard = \"old\"\n",
        )
        .unwrap();

        // SAFETY: 只在测试里改、且这条与本模块另一条测试用 ENV_LOCK 串行；
        // 两个都设是了 Linux（XDG）和 Windows（APPDATA）都能跑到同一个临时目录。
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &root);
            std::env::set_var("APPDATA", &root);
        }

        // 只有旧的那份：旧目录要能接住。
        assert_eq!(Config::path(), Some(legacy.join("desktop.toml")));
        let cfg = Config::load().expect("旧的配置读得出来");
        assert_eq!(cfg.host.as_str(), "http://old.test:1");

        // 新的那份一存在，优先级就在前面。
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(
            current.join("desktop.toml"),
            "host = \"http://new.test:2\"\nboard = \"new\"\n",
        )
        .unwrap();
        assert_eq!(Config::path(), Some(current.join("desktop.toml")));
        let cfg = Config::load().expect("新的配置读得出来");
        assert_eq!(cfg.host.as_str(), "http://new.test:2");

        // 一份都没有 → 新建的那份落在新目录，不是旧目录。
        std::fs::remove_dir_all(&root).unwrap();
        let target = Config::save_target().expect("能算出保存目标");
        assert_eq!(target, current.join("desktop.toml"));

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("APPDATA");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn 默认日志名还是_desktop_log() {
        assert_eq!(super::log_file_name_for(None), "desktop.log");
        assert_eq!(super::log_file_name_for(Some("")), "desktop.log");
    }

    #[test]
    fn 有实例_id_时日志各写各的() {
        assert_eq!(super::log_file_name_for(Some("a")), "desktop-a.log");
        assert_eq!(super::log_file_name_for(Some("verify-1")), "desktop-verify-1.log");
        assert_ne!(
            super::log_file_name_for(Some("a")),
            super::log_file_name_for(Some("b"))
        );
    }

    #[test]
    fn 实例_id_里的路径字符全被换成横线() {
        // `../` 那种不能进文件名，不然实例能把日志写到别的目录去
        assert_eq!(super::log_file_name_for(Some("../evil")), "desktop----evil.log");
        assert_eq!(super::log_file_name_for(Some("a/b c")), "desktop-a-b-c.log");
        assert_eq!(super::log_file_name_for(Some("a.b")), "desktop-a-b.log");
    }
}
