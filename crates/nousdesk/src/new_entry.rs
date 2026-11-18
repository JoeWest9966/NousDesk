//! 「新建入口」的纯逻辑：菜单长什么样 + 「复制这条会话」发什么。
//!
//! 这里只放**不碰窗口、不碰网络**的东西 —— 菜单结构、请求体拼装、名字拼装。
//! 画菜单在 `app.rs`（要 `t()` 和 `cx.listener`），建会话的 HTTP 也在 `app.rs`
//! （要 `cx.spawn` + `TOKIO`）。这样「复制」到底抄了哪几个字段这件事，
//! 有一条 `cargo test` 钉着，不用开到界面上看。

use nous_engine::types::{CreateSession, Session};

/// 菜单项点下去干什么。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    /// 打开 New Manual 盘面（空白）。
    Manual,
    /// 从模板起：先挑一条 `Session-Template` 预设，再 `POST /v1/sessions { preset }`。
    FromPreset,
    /// 复制当前选中这条会话的设置（**不带对话**）。
    CopySettings,
}

/// 菜单里的一行。
pub(crate) struct Entry {
    pub label: &'static str,
    /// 右边那枚键帽。**必须画出来**，不然等于没有快捷键。
    pub keys: &'static str,
    pub action: Action,
}

/// 一组（组标题 + 若干项）。`title == None` 就不画标题。
pub(crate) struct Group {
    pub title: Option<&'static str>,
    pub items: Vec<Entry>,
}

/// ＋ 菜单的样子。
///
/// 分两组而不是一条直线：`新会话` 是「从零开始」，`从当前这条` 是「从这条派生」，
/// 语义不一样；混在一起将来加第四条就没地方了（demo 里 A / B / C 三个方案挑的就是这个）。
pub(crate) fn menu() -> Vec<Group> {
    vec![
        Group {
            title: Some("新会话"),
            items: vec![
                Entry {
                    label: "空白",
                    keys: "Ctrl+N",
                    action: Action::Manual,
                },
                Entry {
                    label: "从模板起",
                    keys: "Ctrl+Shift+N",
                    action: Action::FromPreset,
                },
            ],
        },
        Group {
            title: Some("从当前这条"),
            items: vec![Entry {
                label: "复制设置",
                keys: "Ctrl+D",
                action: Action::CopySettings,
            }],
        },
    ]
}

/// `"provider/modelId"` 拆成两半。
///
/// 拆不开就返回 `None` —— **宁可让引擎用默认模型，也别发一个 provider 为空的垃圾**。
/// 会话上的 `model` 是引擎给的完整串（`volc-ark/deepseek-v3`），建会话要的是两个字段。
pub(crate) fn split_model(model: &str) -> Option<(String, String)> {
    let (provider, id) = model.split_once('/')?;
    let (provider, id) = (provider.trim(), id.trim());
    if provider.is_empty() || id.is_empty() {
        return None;
    }
    Some((provider.to_string(), id.to_string()))
}

/// 副本的名字。原名空就只写「副本」，别留一个前导的 `" · "`。
pub(crate) fn copy_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "副本".to_string()
    } else {
        format!("{name} · 副本")
    }
}

/// 「复制设置」的请求体：**抄设置，不抄对话**。
///
/// 抄的：`model` / `thinkingLevel` / `skills` / `template` / `cwd` / `machine` / `readOnly`。
/// 不抄的：
/// - `extensions`：`GET /v1/sessions` 里**没有**这条会话生效的扩展清单（只有建会话的响应
///   `CreatedSession` 才有）。省掉这个键 = 引擎给默认（全开），跟原来那条可能不一致；
///   要补得先让引擎在会话对象里带上它，别在客户端瞎猜。
/// - `preset`：那是「从模板起」的另一条路，复制设置不该顺手再挂一个模板。
///
/// `template` 是 `Some(s.template.clone())`：**总是发**（`None` 也是有效值 = 明确不要），
/// 省掉服务端会拿仓库默认值塞回来，见 `wire.md`「template 省掉 ≠ 不要」。
pub(crate) fn copy_body(s: &Session) -> CreateSession {
    CreateSession {
        name: Some(copy_name(&s.name)),
        preset: None,
        model: s.model.as_deref().and_then(split_model).map(|(provider, model_id)| {
            serde_json::json!({ "provider": provider, "modelId": model_id })
        }),
        thinking_level: s.thinking_level.clone(),
        skills: (!s.skills.is_empty()).then(|| s.skills.clone()),
        template: Some(s.template.clone()),
        extensions: None,
        cwd: s.cwd.clone(),
        machine: s.machine.clone(),
        read_only: Some(s.read_only),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sess() -> Session {
        Session {
            id: 7,
            name: "真模型".into(),
            parent: None,
            owner: None,
            model: Some("volc-ark/deepseek-v3".into()),
            thinking_level: Some(serde_json::json!("high")),
            thinking_levels: vec![],
            skills: vec!["code-review".into(), "tdd".into()],
            template: Some("审查一段改动".into()),
            read_only: false,
            cwd: Some("/opt/Nous/content/new1/map".into()),
            machine: Some("j1900".into()),
            busy: None,
            updated_at: None,
            last: None,
            archived: None,
            pinned: false,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn 模型名拆得开才发() {
        assert_eq!(
            split_model("volc-ark/deepseek-v3"),
            Some(("volc-ark".into(), "deepseek-v3".into()))
        );
        // 拆不开 / 有一半是空的：都不发，让引擎用默认模型。
        assert_eq!(split_model("deepseek-v3"), None);
        assert_eq!(split_model("/deepseek"), None);
        assert_eq!(split_model("volc-ark/"), None);
        assert_eq!(split_model(""), None);
    }

    #[test]
    fn 复制设置抄的是这几个字段() {
        let s = sess();
        let body = copy_body(&s);
        assert_eq!(body.name.as_deref(), Some("真模型 · 副本"));
        assert_eq!(
            body.model,
            Some(serde_json::json!({"provider":"volc-ark","modelId":"deepseek-v3"}))
        );
        assert_eq!(body.thinking_level, Some(serde_json::json!("high")));
        assert_eq!(
            body.skills,
            Some(vec!["code-review".to_string(), "tdd".to_string()])
        );
        assert_eq!(body.cwd.as_deref(), Some("/opt/Nous/content/new1/map"));
        assert_eq!(body.machine.as_deref(), Some("j1900"));
        assert_eq!(body.read_only, Some(false));
        // 不带对话：请求体里根本没有 entries 这个口子。
        // 不挂模板预设：那是「从模板起」那条路的事。
        assert_eq!(body.preset, None);
        // 会话对象里没有生效的扩展清单 —— 别瞎猜，省掉这个键。
        assert_eq!(body.extensions, None);
    }

    #[test]
    fn 模板总是发_技能空就不发() {
        let mut s = sess();
        s.skills = vec![];
        s.template = None;
        let body = copy_body(&s);
        // `Some(None)`：明确「不要模板」，省掉服务端会拿默认值塞回来。
        assert_eq!(body.template, Some(None));
        assert_eq!(body.skills, None);
    }

    #[test]
    fn 副本名字别留前导点() {
        assert_eq!(copy_name("真模型"), "真模型 · 副本");
        assert_eq!(copy_name("  "), "副本");
        assert_eq!(copy_name(""), "副本");
    }

    #[test]
    fn 菜单两组三项_快捷键都写出来() {
        let g = menu();
        let labels: Vec<&str> = g
            .iter()
            .flat_map(|x| x.items.iter().map(|i| i.label))
            .collect();
        assert_eq!(labels, ["空白", "从模板起", "复制设置"]);
        // 每一项都有键帽，且不重复。
        let keys: Vec<&str> = g
            .iter()
            .flat_map(|x| x.items.iter().map(|i| i.keys))
            .collect();
        assert_eq!(keys, ["Ctrl+N", "Ctrl+Shift+N", "Ctrl+D"]);
        assert_eq!(g.len(), 2);
        assert!(g.iter().all(|x| x.title.is_some()));
    }
}
