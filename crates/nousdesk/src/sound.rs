//! 提示音：什么时候响、响什么、怎么响。
//!
//! ## 三条边界（别破，理由在 `decisions.md` 2026-11-16 那条）
//!
//! 1. **音频是叶子能力。** 不引 `cpal` / `rodio` —— 那等于把 ALSA 设备热插拔、
//!    PipeWire 兼容性、后台线程 panic 全塞进 UI 进程，而我们要的只是"响 300 毫秒"。
//!    改成 spawn 系统播放器：`pw-play` → `paplay` → `aplay` 按顺序挑第一个存在的。
//! 2. **播不出来就是静音。** 没播放器、没声卡、起不来、被杀 —— 一律只记日志。
//!    用户正在发消息的时候弹一句"音频设备打不开"是纯粹的伤害。
//! 3. **声音是偏好，不是事实。** 落盘在 `~/.config/nousdesk/sound.toml`，
//!    不写 `desktop.toml`（那份是连接配置，便携包里可能只读）。
//!
//! ## 声音文件从哪来
//!
//! `assets/sounds/*.wav` 由 `scripts/gen-sounds.py` 生成（音型 = 二音上行/下行），
//! `include_bytes!` 进二进制，第一次要响的时候写到 `~/.local/share/nousdesk/sounds/`。
//! **不能直接放 assets 目录**：发布包里 assets 不跟着走，`include_bytes!` 是唯一真相。
//!
//! ## 不做什么
//!
//! 不做音频解码、不做淡出、不做设备选择、不做音量滑杆（跟系统音量走）。
//! 真要低延迟或混音，那时候再换 `rodio` —— 反悔点写在这儿，别偷偷换。

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::config::APP_DIR;

/// 偏好文件的文件名（在 `APP_DIR` 下）。
const FILE: &str = "sound.toml";

// ── 偏好 ────────────────────────────────────────────────────────────────

/// 哪些事该响。
///
/// 分成两组是因为它们是两件事：`done` / `error` 是**你不在看的时候**叫你，
/// `send` / `stop` 是**你正在操作**时给你反馈。判据不一样（前者要判焦点，
/// 后者永远响），所以别合并成一个开关。
///
/// 未知字段忽略、缺字段取默认 —— 以后加一个声音不该让老配置读不出来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// 总开关。关掉之后一个都不响，但不影响未读红点。
    pub master: bool,
    /// 一轮生成完成（只在窗口不在前台时）。
    pub done: bool,
    /// 生成出错 / 这一轮白等了（同上，只在后台）。
    pub error: bool,
    /// 发送一条消息。
    pub send: bool,
    /// 用户自己按的停止。
    ///
    /// 默认**关**：停止是"少做事"，再响一声像在确认成功。开着也不吵，但没必要默认开。
    pub stop: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            master: true,
            done: true,
            error: true,
            send: true,
            stop: false,
        }
    }
}

impl Prefs {
    /// 这个 cue 现在该不该出声（不判焦点，也不判防抖）。
    pub fn enabled(&self, cue: Cue) -> bool {
        if !self.master {
            return false;
        }
        match cue {
            Cue::Done => self.done,
            Cue::Error => self.error,
            Cue::Send => self.send,
            Cue::Stop => self.stop,
        }
    }

    /// 打开 / 关掉一个 cue。`master` 没有对应的 `Cue`，单独走 [`Self::set_master`]。
    pub fn set(&mut self, cue: Cue, on: bool) {
        match cue {
            Cue::Done => self.done = on,
            Cue::Error => self.error = on,
            Cue::Send => self.send = on,
            Cue::Stop => self.stop = on,
        }
    }

    /// 文件在哪儿（`~/.config/nousdesk/sound.toml`）。
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join(APP_DIR).join(FILE))
    }

    /// 读偏好。**任何失败都回默认**：没文件、没目录、写坏了 —— 静音不该是
    /// 「配置读不出来」的惩罚，而默认值就是我们要的样子。
    pub fn load() -> Self {
        Prefs::path()
            .map(|p| Prefs::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_else(|e| {
            log::warn!("sound.toml 读不动（{e}），按默认值走");
            Self::default()
        })
    }

    /// 落盘。**先写盘再改内存** —— 和主题页一个规矩：写不进去就说，别装成功
    /// （否则重启又跳回去，比不改更糟）。
    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Prefs::path().ok_or_else(|| "拿不到配置目录".to_string())?;
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Err(format!("建目录失败：{e}"));
        }
        let text = toml::to_string_pretty(self).map_err(|e| format!("序列化失败：{e}"))?;
        std::fs::write(&path, text).map_err(|e| format!("写入失败：{e}"))?;
        Ok(path)
    }
}

// ── 声音 ────────────────────────────────────────────────────────────────

/// 一个响的时机。四种，多一种都算噪音。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Done,
    Error,
    Send,
    Stop,
}

impl Cue {
    /// 只在测试里用：挨个过一遍「总开关」「槽位对得上」那些断言。
    #[cfg(test)]
    pub const ALL: &'static [Cue] = &[Cue::Done, Cue::Error, Cue::Send, Cue::Stop];

    fn index(self) -> usize {
        match self {
            Cue::Done => 0,
            Cue::Error => 1,
            Cue::Send => 2,
            Cue::Stop => 3,
        }
    }

    fn file(self) -> &'static str {
        match self {
            Cue::Done => "done.wav",
            Cue::Error => "error.wav",
            Cue::Send => "send.wav",
            Cue::Stop => "stop.wav",
        }
    }

    /// 同一个 cue 的最短重响间隔。
    ///
    /// 两个用途：连点「发送」不该变成机关枪；一条流里要是连着重放两次 `done`，
    /// 那也不是两件事。**60 秒的跨会话合并不在这儿** —— 那要靠后台心跳数
    /// 未读有几个会话，前台这条流根本不知道别的会话发生了什么。
    fn gate(self) -> Duration {
        match self {
            Cue::Done | Cue::Error => Duration::from_millis(2000),
            Cue::Send | Cue::Stop => Duration::from_millis(200),
        }
    }
}

/// 随包的声音文件。**别改成从 assets 目录读**：发布包里没有那个目录。
const ASSETS: &[(&str, &[u8])] = &[
    ("done.wav", include_bytes!("../../../assets/sounds/done.wav")),
    ("error.wav", include_bytes!("../../../assets/sounds/error.wav")),
    ("send.wav", include_bytes!("../../../assets/sounds/send.wav")),
    ("stop.wav", include_bytes!("../../../assets/sounds/stop.wav")),
];

/// 播放器优先级：PipeWire 原生 → PulseAudio 兼容层 → 直连 ALSA。
///
/// `pw-play` 排第一是因为这台机器（和 GNOME 默认）是 PipeWire；`aplay` 兜底
/// 给没有声音服务器的环境（裸 X、最小容器），它不连服务、直接开设备。
const PLAYERS: &[&str] = &["pw-play", "paplay", "aplay"];

/// 后台完成音的合并窗口。见 [`chime_background`]。
const MERGE_WINDOW: Duration = Duration::from_secs(60);

/// 播放器进程最多活多久。超过就杀 —— 卡住的 `paplay` 会一直占着设备，
/// 下一次播放就排不上队，表现成「提示音响了一次之后再也不响」。
const PLAY_TIMEOUT: Duration = Duration::from_millis(1500);

/// 「该响就响」。`allowed` 由调用方算：**此刻该不该打扰你**。
///
/// - 完成 / 出错：窗口在前台就不响（你正看着它跑完，视觉反馈已经够了）。
/// - 发送 / 停止：永远 `true`（你刚动过手，声音是那次动作的回执）。
pub fn chime(prefs: Prefs, cue: Cue, allowed: bool) {
    if !allowed || !prefs.enabled(cue) || !gate(cue) {
        return;
    }
    play(cue);
}

/// 后台那条路上的「完成」：**60 秒内只响一次**，几次跑完算一件事。
///
/// 跨会话的合并只能在这条路上做：前台那条 SSE 流只知道自己看的那个会话，
/// 它既不知道别的会话跑完了，也没必要知道。所以"合并"这件事被钉在
/// 「后台心跳 + 未读表」这一条链路上，别挪到 `chime` 里去 —— `chime` 是
/// 单事件的防抖（2 秒），两件事不一样。
///
/// 为什么是 60 秒：人不在电脑前，连续响三声等于没响。这一声的意思是
/// "有事了，回来看一眼"，不是"三件事分别发生了"。
///
/// `sessions` 只进日志。以后接上桌面通知，那行标题就是「3 个会话有新内容」。
pub fn chime_background(prefs: Prefs, sessions: usize) -> bool {
    if sessions == 0 || !prefs.enabled(Cue::Done) {
        return false;
    }
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let Ok(mut last) = LAST.lock() else {
        return false;
    };
    let now = Instant::now();
    if let Some(prev) = *last
        && now.duration_since(prev) < MERGE_WINDOW
    {
        log::info!("后台又有 {sessions} 个会话跑完；{MERGE_WINDOW:?} 内已经响过，这次不出声");
        return false;
    }
    *last = Some(now);
    log::info!("后台 {sessions} 个会话有新内容 → 响一声完成音");
    // 直接走 `play`：该不该响已经在这儿算完了，再进 `chime` 会被 2 秒防抖
    // 和 `allowed` 判一遍，等于把同一件事判两次。
    play(Cue::Done);
    true
}

/// 「试听」用：不看偏好、不看防抖，直接响。
///
/// 试听**必须**穿过偏好 —— 用户点「试听」就是想听，哪怕这个 cue 现在是关的。
pub fn preview(cue: Cue) {
    play(cue);
}

/// 真的去起一个播放器。失败就是没声，只记日志。
pub fn play(cue: Cue) {
    let Some(player) = player() else { return };
    let Some(dir) = sound_dir() else { return };
    let file = dir.join(cue.file());

    let spawned = Command::new(player)
        .arg(&file)
        // 三个都接 null：不清 stdout/stderr 的话，播放器往里写点东西就能把
        // 它的父进程（我们）挂在 `wait` 上，界面当场卡住。
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match spawned {
        Ok(child) => reap(child),
        Err(e) => log::warn!("播放器起不来（{}）：{e}", player.display()),
    }
}

/// 看着播放器退出，超时就杀。单开一个线程而不是阻塞 UI：
/// 播放要 300ms 上下，UI 线程一毫秒都不该等。
fn reap(mut child: Child) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + PLAY_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    log::warn!("播放器超过 {}ms 没退出，已杀掉", PLAY_TIMEOUT.as_millis());
                    return;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(40)),
                Err(e) => {
                    log::warn!("等播放器退出时出错：{e}");
                    return;
                }
            }
        }
    });
}

// ── 一次性探测（PATH 扫一遍，不 spawn 进程）──────────────────────────────

fn gate(cue: Cue) -> bool {
    static LAST: Mutex<[Option<Instant>; 4]> = Mutex::new([None; 4]);
    let Ok(mut last) = LAST.lock() else {
        // 锁坏了不该让人没声音。
        return true;
    };
    let now = Instant::now();
    let slot = &mut last[cue.index()];
    if let Some(prev) = *slot
        && now.duration_since(prev) < cue.gate()
    {
        return false;
    }
    *slot = Some(now);
    true
}

/// 挑一个播放器。**只扫 PATH，不跑 `--version`**：`paplay --version` 在没连上
/// 声音服务器时会失败退出，拿它当"存不存在"的判据等于把一个能用的播放器筛掉。
fn player() -> Option<&'static PathBuf> {
    static PICKED: OnceLock<Option<PathBuf>> = OnceLock::new();
    PICKED
        .get_or_init(|| {
            let paths = std::env::var_os("PATH")?;
            for name in PLAYERS {
                for dir in std::env::split_paths(&paths) {
                    let cand = dir.join(name);
                    if cand.metadata().map(|m| m.is_file()).unwrap_or(false) {
                        log::info!("提示音播放器：{}", cand.display());
                        return Some(cand);
                    }
                }
            }
            log::info!(
                "没找到 {} 里的任何一个，提示音静音（声音开关仍可用，装一个就好）",
                PLAYERS.join(" / ")
            );
            None
        })
        .as_ref()
}

/// 把随包的声音文件摊到磁盘上（播放器只吃路径）。摊一次，之后复用。
fn sound_dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = dirs::data_local_dir()?.join(APP_DIR).join("sounds");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("建声音目录失败（{}）：{e}", dir.display());
            return None;
        }
        for (name, bytes) in ASSETS {
            let path = dir.join(name);
            // 内容一样就不重写：音色以后会调，但没必要每次启动都动四个文件。
            let same = std::fs::read(&path)
                .map(|cur| cur == *bytes)
                .unwrap_or(false);
            if !same && let Err(e) = std::fs::write(&path, bytes) {
                log::warn!("写声音文件失败（{}）：{e}", path.display());
            }
        }
        Some(dir)
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 默认值_提醒开_停止关() {
        let p = Prefs::default();
        assert!(p.master && p.done && p.error && p.send);
        assert!(!p.stop, "停止出声默认关：它是「少做事」，不需要回执");
    }

    #[test]
    fn 总开关关掉之后一个都不响() {
        let p = Prefs {
            master: false,
            ..Prefs::default()
        };
        for cue in Cue::ALL {
            assert!(!p.enabled(*cue), "{cue:?} 不该在总开关关掉时响");
        }
    }

    #[test]
    fn 每个_cue_都有自己的槽() {
        // 挨个打开，只影响自己 —— 这是 `set` 唯一容易写错的地方。
        for one in Cue::ALL {
            let mut p = Prefs {
                master: true,
                done: false,
                error: false,
                send: false,
                stop: false,
            };
            p.set(*one, true);
            for cue in Cue::ALL {
                assert_eq!(p.enabled(*cue), cue == one, "{one:?} 打开时 {cue:?} 的状态不对");
            }
        }
    }

    #[test]
    fn 配置缺字段就用默认值_加新字段不炸老文件() {
        let p: Prefs = toml::from_str("master = false").unwrap();
        assert!(!p.master);
        assert!(p.done, "没写的字段取默认，不是 false");
    }

    #[test]
    fn 多出来的字段被忽略() {
        // 以后加了配置项、又回滚版本时，不能因为多一个键就读不出来。
        let p: Prefs = toml::from_str("master = true\nloop_sound = true\n").unwrap();
        assert!(p.master);
    }

    #[test]
    fn 写坏了回默认_不报错() {
        let dir = std::env::temp_dir().join("nousdesk-sound-test-bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sound.toml");
        std::fs::write(&path, "这不是 toml = = =").unwrap();
        assert_eq!(Prefs::load_from(&path), Prefs::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 存了再读回来是同一份() {
        let dir = std::env::temp_dir().join("nousdesk-sound-test-roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sound.toml");
        let p = Prefs {
            stop: true,
            send: false,
            ..Prefs::default()
        };
        std::fs::write(&path, toml::to_string_pretty(&p).unwrap()).unwrap();
        assert_eq!(Prefs::load_from(&path), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 随包的声音文件都在() {
        // `include_bytes!` 少了文件是编译期错误，但**文件名和 cue 对不上**是运行期
        // 静音 —— 只有真响一声才发现。这条测试把它变成跑测试就发现。
        for cue in Cue::ALL {
            assert!(
                ASSETS.iter().any(|(n, b)| *n == cue.file() && !b.is_empty()),
                "{cue:?} 没有对应的随包文件"
            );
        }
    }
}
