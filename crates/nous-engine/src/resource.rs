//! 三个资源仓（技能 / 提示词 / 扩展）共用的文件读写形状。
//!
//! 为什么单独一个模块而不是塞进 `types.rs`：`types.rs` 是 `/v1/*` 的 **GET** 回包
//! （技能名册、提示词私库、扩展清单），这里放的是**写入面** —— 读写单个文件、新建。
//! 两者变化的节奏不一样：名册随引擎版本动，写入面随「仓库里的文件怎么摆」动。
//!
//! 端点照 qrd3wh 源码：
//! - `GET /v1/{kind}/files?name=`          → `{ name, files: [{ path, size }] }`
//! - `GET /v1/{kind}/file?name=&path=`     → `{ name, path, text, etag }`
//! - `PUT /v1/{kind}/file`                 `{ name, path, text, base? }`
//! - `POST /v1/{kind}/create`              每个仓一套必填项（见下面三个 `New*`）
//!
//! `etag` 是写回时的 `base`：撞了就是 409，用户得先重读 —— 别拿它当缓存键，
//! 它的唯一用途就是「我这版是基于哪一版改的」。

use std::collections::HashMap;

/// `GET /v1/repos/{repo}/status`：这一段私库在本机的 git 状态。
///
/// 内核目前回 `{root, head, branch, ahead, behind, dirty, locked}`；`remote` 是
/// **软依赖**（内核补了才有），字段缺失 = 没有远端 —— 不猜、也不降级成「猜一个」。
/// 未知字段一律透传，内核对 `RepoStatus` 加字段这边不跟着改。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RepoStatus {
    /// 仓根在本机的绝对路径（`/opt/Nous-skills` 这种）。路径条直接画它。
    #[serde(default)]
    pub root: String,
    /// HEAD 短 sha。`null` = 仓读不了。
    #[serde(default)]
    pub head: Option<String>,
    /// 当前分支名。分离头时也可能 `null`。
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub ahead: u32,
    #[serde(default)]
    pub behind: u32,
    /// 有没有未提交改动。布尔，不是数字 —— 处数看 `pending`，别拿它当计数。
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub locked: bool,
    /// `origin` 的 URL。内核补上后才存在；缺失 = 没有远端（不画 ↗）。
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/repos/{repo}/pending` 回包里 `units[]` 的一条：按**顶层目录 / 文件**
/// 分组的一处待提交改动。`state` 是 `tracked` / `untracked` / `unpushed`。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoPendingUnit {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub ahead_commits: u32,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/repos/{repo}/pending` 的回包：`{ units: [...] }`。
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct RepoPending {
    #[serde(default)]
    pub units: Vec<RepoPendingUnit>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 私库根下这些**顶层名**是仓级簿记文件，不是一条可发布的内容单元 ——
/// 算「N 处待发布」时排除掉，否则扩展仓永远多算两三处。名单只在这里写一次。
pub const REPO_LEVEL_NAMES: &[&str] = &[
    "artifacts",
    "deleted-extensions.json",
    "categories.json",
    "index.json",
];

/// 这个顶层名是不是仓级簿记文件（`artifacts/` 目录、几个索引 / 删除名单 JSON）。
pub fn is_repo_level_file(name: &str) -> bool {
    let name = name.trim().trim_end_matches('/');
    REPO_LEVEL_NAMES.iter().any(|x| x.eq_ignore_ascii_case(name))
}

/// 一个仓有几「处」待发布：扩展只数内容目录（仓级文件不算），技能 / 提示词数全部单元。
///
/// 判据（j1900 真数据，2026-11）：skills `main` 2 处、prompts `main` 5 处、
/// extend `feat/skills-upstream-source` 5 处（`pending` 原始 7 条，扣掉 `artifacts/`
/// 与 `deleted-extensions.json` 两个仓级单元）。
pub fn pending_unit_count(repo: Repo, units: &[RepoPendingUnit]) -> usize {
    match repo {
        Repo::Extensions => units.iter().filter(|u| !is_repo_level_file(&u.name)).count(),
        Repo::Skills | Repo::Prompts => units.len(),
    }
}

/// 中间省略：`max_chars` 个字符放不下时，保留前约 2/3、后约 1/3，中间换成 `…`。
///
/// 按**字符**数（不是字节）算，中文一个算一个。前缀留多、后缀留少，是因为
/// 分支名开头（`feat/`）比结尾更能说明它是什么分支。
/// `ellipsis_middle("feat/skills-upstream-source", 18) == "feat/skills…source"`。
pub fn ellipsis_middle(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    if max_chars == 1 {
        return "…".to_string();
    }
    let keep = max_chars - 1;
    let head = keep * 2 / 3;
    let tail = keep - head;
    let mut out = String::with_capacity(max_chars);
    out.extend(&chars[..head]);
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// 文件列表里的一行。`size` 引擎给了就用，没给（比如目录占位）就空着。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResourceFileEntry {
    pub path: String,
    #[serde(default)]
    pub size: Option<u64>,
    /// 引擎以后加字段不能让这边编译不过（`deny_unknown_fields` 是禁区）。
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// `GET /v1/{kind}/files` 的回包。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResourceFiles {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub files: Vec<ResourceFileEntry>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ResourceFiles {
    /// 主文件排在最前面（正文优先），清单类文件垫后 —— 打开编辑器先看内容，不是先看元数据。
    pub fn ordered(&self) -> Vec<&ResourceFileEntry> {
        let mut out: Vec<&ResourceFileEntry> = self.files.iter().collect();
        out.sort_by_key(|f| {
            let is_meta = f.path.ends_with("nous.json") || f.path.ends_with(".gitkeep");
            (is_meta, f.path.clone())
        });
        out
    }
}

/// `GET /v1/{kind}/file` 的回包：正文 + 并发闸。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResourceFile {
    #[serde(default)]
    pub name: String,
    pub path: String,
    pub text: String,
    /// 写回时当 `base`；引擎认的是「你改的是不是这一版」。
    #[serde(default)]
    pub etag: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 新建技能：`description` 必填，落在 `SKILL.md` 的 frontmatter 里。
#[derive(Debug, Clone, serde::Serialize)]
pub struct NewSkill {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<String>,
}

/// 新建提示词：`summary` 必填，落在 `nous.json`；`PROMPT.md` 是纯正文（没有 frontmatter）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct NewPrompt {
    pub name: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<String>,
    /// 可省：不给就落一个 `# <name>` 的标题（标题也会进提示词）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// 新建扩展：说明必填（≤ 200），核心三条 `nous` / `skills` / `prompts` 引擎会 403。
#[derive(Debug, Clone, serde::Serialize)]
pub struct NewExtension {
    pub name: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<String>,
}

/// `PUT /v1/{kind}/file` 的 body。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SaveFile<'a> {
    pub name: &'a str,
    pub path: &'a str,
    pub text: &'a str,
    /// 省掉就是「照着磁盘上那版硬写」（引擎允许，但会盖掉别人的改动）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<&'a str>,
}

/// 三个仓的标识。**白名单**：拼错的仓名在发 HTTP 之前就挡掉，
/// 不然调用点会拼出一个引擎不认识的地址还假装成功。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repo {
    Skills,
    Prompts,
    Extensions,
}

impl Repo {
    /// 引擎那几个写接口认的**仓名**（`/v1/{key}/file`）。路径前缀不要往 body / query 里塞。
    pub fn key(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Prompts => "prompts",
            Self::Extensions => "extensions",
        }
    }

    /// 私库的**仓名**（`/v1/repos/{repo}/…`）：归档 / 恢复 / 真删 / 变体走这一套。
    ///
    /// 与 `key()` 不是一回事：扩展在文件面叫 `extensions`，在私库面叫 `extend`。
    /// 拿 `key()` 去拼 `v1/repos/extensions/archive` 只会 404，而且 404 长得像「没有这东西」。
    pub fn repo_key(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Prompts => "prompts",
            Self::Extensions => "extend",
        }
    }

    /// 带 `v1/` 的路径前缀，只给「拼 URL」的地方用。
    pub fn path(self) -> &'static str {
        match self {
            Self::Skills => "v1/skills",
            Self::Prompts => "v1/prompts",
            Self::Extensions => "v1/extensions",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Skills => "技能",
            Self::Prompts => "提示词",
            Self::Extensions => "扩展",
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Skills, Self::Prompts, Self::Extensions]
    }
}

/// 把 `"skills"` / `"prompts"` / `"extensions"` 认成 `Repo`。
impl std::str::FromStr for Repo {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "skills" => Ok(Self::Skills),
            "prompts" => Ok(Self::Prompts),
            "extensions" => Ok(Self::Extensions),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 活样本：`GET /v1/skills/files?name=code-review`（2026-11-16 实测）。
    const FILES: &str = r#"{"name":"code-review","files":[{"path":"nous.json","size":112},{"path":"SKILL.md","size":413}]}"#;

    /// 活样本：`GET /v1/skills/file?name=code-review&path=SKILL.md`。
    const FILE: &str = r#"{"name":"code-review","path":"SKILL.md","text":"---\nname: code-review\n---\n","etag":"a2adba91"}"#;

    #[test]
    fn parse_files_and_put_body_on_top() {
        let f: ResourceFiles = serde_json::from_str(FILES).unwrap();
        assert_eq!(f.name, "code-review");
        assert_eq!(f.files.len(), 2);
        // 正文优先：SKILL.md 排在 nous.json 前面
        let ordered = f.ordered();
        assert_eq!(ordered[0].path, "SKILL.md");
        assert_eq!(ordered[1].path, "nous.json");
        assert_eq!(ordered[1].size, Some(112));
    }

    #[test]
    fn parse_file_keeps_text_and_etag() {
        let f: ResourceFile = serde_json::from_str(FILE).unwrap();
        assert_eq!(f.path, "SKILL.md");
        assert!(f.text.starts_with("---"));
        assert_eq!(f.etag, "a2adba91");
    }

    /// 引擎将来加字段，这边不许编译不过、也不许丢字段。
    #[test]
    fn unknown_fields_pass_through() {
        let f: ResourceFile =
            serde_json::from_str(r#"{"path":"a.md","text":"x","etag":"e","kind":"markdown","mtime":1}"#)
                .unwrap();
        assert_eq!(f.extra["kind"], "markdown");
        assert_eq!(f.extra["mtime"], 1);
        let l: ResourceFiles = serde_json::from_str(r#"{"files":[{"path":"a","size":1,"mode":"755"}]}"#).unwrap();
        assert_eq!(l.files[0].extra["mode"], "755");
    }

    /// 新建 body 的形状：空的 category 不发、body 省掉就不发这个键。
    #[test]
    fn new_bodies_skip_empty_fields() {
        let s = serde_json::to_value(NewSkill {
            name: "demo".into(),
            description: "d".into(),
            category: vec![],
        })
        .unwrap();
        assert_eq!(s, serde_json::json!({"name":"demo","description":"d"}));

        let p = serde_json::to_value(NewPrompt {
            name: "demo".into(),
            summary: "s".into(),
            category: vec!["工程".into()],
            body: None,
        })
        .unwrap();
        assert_eq!(p, serde_json::json!({"name":"demo","summary":"s","category":["工程"]}));

        let e = serde_json::to_value(NewExtension {
            name: "demo".into(),
            summary: "s".into(),
            category: vec![],
        })
        .unwrap();
        assert_eq!(e, serde_json::json!({"name":"demo","summary":"s"}));
    }

    #[test]
    fn save_body_omits_base_when_absent() {
        let with = serde_json::to_value(SaveFile {
            name: "d",
            path: "SKILL.md",
            text: "x",
            base: Some("etag1"),
        })
        .unwrap();
        assert_eq!(with["base"], "etag1");
        let without = serde_json::to_value(SaveFile {
            name: "d",
            path: "SKILL.md",
            text: "x",
            base: None,
        })
        .unwrap();
        assert!(without.get("base").is_none());
    }

    #[test]
    fn repo_paths_and_from_str() {
        assert_eq!(Repo::Skills.key(), "skills");
        assert_eq!(Repo::Extensions.key(), "extensions");
        assert_eq!(Repo::Skills.path(), "v1/skills");
        assert_eq!(Repo::Extensions.path(), "v1/extensions");
        assert_eq!("prompts".parse::<Repo>(), Ok(Repo::Prompts));
        assert!("prompt".parse::<Repo>().is_err());
    }

    /// 活样本：`GET /v1/repos/extend/status`（j1900，2026-11，尚无 remote）。
    const STATUS: &str = r#"{"root":"/opt/Nous-extend","head":"07b9e0b","branch":"feat/skills-upstream-source","ahead":0,"behind":0,"dirty":true,"locked":false}"#;

    #[test]
    fn repo_status_parses_and_remote_defaults_none() {
        let s: RepoStatus = serde_json::from_str(STATUS).unwrap();
        assert_eq!(s.root, "/opt/Nous-extend");
        assert_eq!(s.branch.as_deref(), Some("feat/skills-upstream-source"));
        assert!(s.dirty);
        assert_eq!(s.remote, None, "没有 remote 字段 = 没远端，不是错误");
        // 内核以后补 remote / 加未知字段：这边要收得住。
        let with_remote: RepoStatus = serde_json::from_str(
            r#"{"root":"/opt/Nous-skills","branch":"main","dirty":false,"remote":"https://github.com/x/y","future":1}"#,
        )
        .unwrap();
        assert_eq!(with_remote.remote.as_deref(), Some("https://github.com/x/y"));
        assert_eq!(with_remote.extra["future"], 1);
    }

    /// 活样本：`GET /v1/repos/extend/pending`（j1900，2026-11）。
    const EXT_PENDING: &str = r#"{"units":[
        {"name":"123","state":"tracked","aheadCommits":0,"paths":["123/index.js"]},
        {"name":"artifacts","state":"untracked","aheadCommits":0,"paths":["artifacts/"]},
        {"name":"deleted-extensions.json","state":"untracked","aheadCommits":0,"paths":["deleted-extensions.json"]},
        {"name":"nous","state":"tracked","aheadCommits":0,"paths":["nous/nous.json"]},
        {"name":"prompts","state":"tracked","aheadCommits":0,"paths":["prompts/nous.json"]},
        {"name":"skills","state":"tracked","aheadCommits":0,"paths":["skills/nous.json"]},
        {"name":"test","state":"tracked","aheadCommits":0,"paths":["test/index.js"]}
    ]}"#;

    #[test]
    fn repo_pending_parses_units() {
        let p: RepoPending = serde_json::from_str(EXT_PENDING).unwrap();
        assert_eq!(p.units.len(), 7);
        assert_eq!(p.units[0].name, "123");
        assert_eq!(p.units[0].state, "tracked");
    }

    /// 「处」的口径：扩展只数内容目录，仓级文件（artifacts / 三个 JSON）不算；
    /// 技能 / 提示词数全部单元（j1900 真数据：skills 2、prompts 5）。
    #[test]
    fn pending_count_excludes_repo_level_files_only_for_extend() {
        let ext: RepoPending = serde_json::from_str(EXT_PENDING).unwrap();
        assert_eq!(pending_unit_count(Repo::Extensions, &ext.units), 5, "7 条扣掉 artifacts + deleted-extensions.json");
        for level in ["artifacts", "artifacts/", "deleted-extensions.json", "categories.json", "index.json"] {
            assert!(is_repo_level_file(level), "{level} 应是仓级文件");
        }
        assert!(!is_repo_level_file("nous"), "扩展目录不是仓级文件");

        let skills = [
            RepoPendingUnit {
                name: "ask-matt".into(), state: "tracked".into(), ahead_commits: 0,
                paths: vec![], extra: Default::default(),
            },
            RepoPendingUnit {
                name: "index.json".into(), state: "tracked".into(), ahead_commits: 0,
                paths: vec![], extra: Default::default(),
            },
        ];
        // 技能段不套扩展的排除名单：index.json 也计入（对齐 j1900「skills 2 处」）。
        assert_eq!(pending_unit_count(Repo::Skills, &skills), 2);
    }

    #[test]
    fn ellipsis_middle_keeps_head_and_tail() {
        assert_eq!(ellipsis_middle("main", 18), "main");
        assert_eq!(ellipsis_middle("feat/skills-upstream-source", 18), "feat/skills…source");
        // 刚好卡上限不截。
        assert_eq!(ellipsis_middle("abcdefghijklmnopqr", 18), "abcdefghijklmnopqr");
        // 中文按字符算，不按字节。
        assert_eq!(ellipsis_middle("特性分支/很长的名字啊", 6), "特性分…字啊");
        assert_eq!(ellipsis_middle("anything", 0), "");
        assert_eq!(ellipsis_middle("abc", 1), "…");
    }
}
