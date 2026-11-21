// `Desk#13` 后这些纯函数只被 `resources_tests.rs` 引用（钉住引擎回包形状），
// 生产代码不再调用；留着是因为它们是唯一的形状合同。
#![allow(dead_code)]
//! 私库写操作的**残骸**：只剩纯函数（给只读页面的「未发布」角标用）。
//!
//! `Desk#13`：`POST /v1/{skills,prompts,extensions}/create`、`PUT .../file`、
//! `POST .../{op}`、`POST .../publish`、`GET .../files|file` 这 15 个调用点的
//! 入口与 HTTP 代码**一并删掉**（只读私库，#63；`/{op}` 是旧面板化石）。
//! 下面这些纯函数不碰网络，仍给列表上的「未发布」角标和单测用 —— 它们
//! 描述的是引擎回包的形状，不是客户端自己的状态机。


use crate::app::Desktop;
use crate::resources::Selected;

/// 三种条目。`Section` + `Content` 是布局，这个是数据（哪个仓）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Skill,
    Prompt,
    Ext,
}

impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Skill => "技能",
            Self::Prompt => "提示词",
            Self::Ext => "扩展",
        }
    }


}

impl Selected {
    pub(crate) fn kind_of(&self) -> Kind {
        match self {
            Self::Skill(_) => Kind::Skill,
            Self::Prompt(_) => Kind::Prompt,
            Self::Ext(_) => Kind::Ext,
        }
    }

    pub(crate) fn name_of(&self) -> &str {
        match self {
            Self::Skill(n) | Self::Prompt(n) | Self::Ext(n) => n,
        }
    }
}

/// 「有没有改动」= 正文跟载入时不一样。抽成纯函数，好在没有 GPUI 上下文的地方单测。
pub(crate) fn is_dirty(saved: &str, current: &str) -> bool {
    saved != current
}


/// 类别输入框 → 词表。逗号（中英）+ 空白都算分隔，跟引擎 `normalizeCategories` 同口径。
pub(crate) fn parse_categories(raw: &str) -> Vec<String> {
    raw.split([',', '，', '\n', '\t', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 新建时**马上要落盘的内容**。照引擎 `createSkill` / `createPrompt` / `createExtension`
/// 的真实形状写，跟 demo 里那块预览一模一样：
///
/// - 技能：`SKILL.md`（frontmatter 里 `description`）+ `nous.json`（`summary` 是空串）
/// - 提示词：`PROMPT.md`（纯正文，**没有 frontmatter**）+ `nous.json`（`summary` 在这）
/// - 扩展：`nous.json` 清单 + `index.js` 空壳入口
pub(crate) fn landing_files(
    kind: Kind,
    name: &str,
    text: &str,
    category: &str,
    body: &str,
    root: &str,
) -> Vec<(String, String)> {
    let name = name.trim();
    let name = if name.is_empty() { "<name>" } else { name };
    let cats = parse_categories(category);
    let dir = format!("{}/{}/", root.trim_end_matches('/'), name);

    // JSON 手拼而不是 `serde_json::to_string_pretty`：字段顺序要跟引擎一致，
    // 不然预览里的顺序会跟真落盘的对不上（引擎是 `{name, summary, category}`）。
    let cats_json = if cats.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = cats
            .iter()
            .map(|c| format!("    {}", json_str(c)))
            .collect();
        format!("[\n{}\n  ]", items.join(",\n"))
    };

    match kind {
        Kind::Skill => {
            let desc = if text.trim().is_empty() {
                "<description>"
            } else {
                text.trim()
            };
            let sk = format!(
                "---\nname: {name}\ndescription: {}\n---\n\n# {name}\n",
                json_str(desc)
            );
            let manifest = format!(
                "{{\n  \"name\": {},\n  \"summary\": \"\",\n  \"category\": {}\n}}\n",
                json_str(name),
                cats_json
            );
            vec![
                (format!("{dir}SKILL.md"), sk),
                (format!("{dir}nous.json"), manifest),
            ]
        }
        Kind::Prompt => {
            let body = if body.trim().is_empty() {
                format!("# {name}\n")
            } else {
                body.to_string()
            };
            let manifest = format!(
                "{{\n  \"name\": {},\n  \"summary\": {},\n  \"category\": {}\n}}\n",
                json_str(name),
                json_str(text.trim()),
                cats_json
            );
            vec![
                (format!("{dir}PROMPT.md"), body),
                (format!("{dir}nous.json"), manifest),
            ]
        }
        Kind::Ext => {
            let summary = if text.trim().is_empty() {
                "<一句话说明>"
            } else {
                text.trim()
            };
            let cat_line = if cats.is_empty() {
                String::new()
            } else {
                format!(",\n  \"category\": {}", cats_json.replace('\n', "\n  "))
            };
            let manifest = format!(
                "{{\n  \"name\": {},\n  \"version\": \"0.1.0\",\n  \"file\": \"index.js\",\n  \"entry\": \"default\",\n  \"sections\": [],\n  \"summary\": {},\n  \"needs\": {{}}{cat_line}\n}}\n",
                json_str(name),
                json_str(summary)
            );
            // 真实入口文件前面是一大段注释（引擎的 SKELETON_JS），最后一行是这个；
            // 这里只摆那一行 —— 不把引擎的注释抄一份过来跟着烂。
            let js = format!(
                "// {name} —— {summary}\n// （引擎还会在文件头附一段上手注释，这里只摆最后一行）\nexport default ({{ defineExtension }}) => defineExtension({{ name: \"{name}\" }});\n"
            );
            vec![
                (format!("{dir}nous.json"), manifest),
                (format!("{dir}index.js"), js),
            ]
        }
    }
}

/// JSON 字符串字面量（只处理这几个仓会出现的字符：引号、反斜杠、换行、tab）。
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}


/// 提交前的兜底：只挡「必填项空着」这一种，别的错误话术归引擎。
pub(crate) fn missing_field(kind: Kind, name: &str, text: &str) -> Option<&'static str> {
    if name.trim().is_empty() {
        return Some(match kind {
            Kind::Skill => "技能名不能为空",
            Kind::Prompt => "模板名不能为空",
            Kind::Ext => "扩展名不能为空",
        });
    }
    if text.trim().is_empty() {
        return Some(match kind {
            Kind::Skill => "description 不能空 —— 模型靠它判断该不该用这条技能",
            Kind::Prompt => "summary 不能空 —— 人靠这一行决定选不选它",
            Kind::Ext => "一句话说明不能空 —— 卡片上就靠它认人",
        });
    }
    None
}

/// 这条是不是还没进 HEAD。**只看引擎给的 pending 清单**，不在客户端推算。
pub(crate) fn is_untracked(kind: Kind, name: &str, desk: &Desktop) -> bool {
    let r = &desk.resources;
    match kind {
        Kind::Skill => r
            .skills
            .as_ref()
            .and_then(|s| s.skills.iter().find(|x| x.name == name))
            .is_some_and(|s| !s.tracked),
        Kind::Prompt => r
            .prompts
            .as_ref()
            .and_then(|p| p.prompts.iter().find(|x| x.name == name))
            .is_some_and(|p| !p.tracked),
        Kind::Ext => r
            .exts
            .as_ref()
            .and_then(|e| e.repo.as_ref())
            .and_then(|v| v.get("drift"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}
