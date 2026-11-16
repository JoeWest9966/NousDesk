#[cfg(test)]
mod tests {
    use crate::resources::*;
    use nous_engine::types::{Skill, Skills, Prompt, Prompts, Extension, Extensions};

    fn skill(name: &str, desc: &str, cats: &[&str], hidden: bool) -> Skill {
        let mut s = Skill {
            name: name.into(),
            description: Some(desc.into()),
            category: cats.iter().map(|c| c.to_string()).collect(),
            hidden,
            tracked: true,
            dir: None, file: None, load: None, extra: Default::default(),
        };
        s.load = Some(serde_json::json!({"state": "ok"}));
        s
    }

    #[test]
    fn 隐藏技能不进主列表但能单列() {
        let skills = Skills {
            skills: vec![
                skill("ask-matt", "router", &[], true),
                skill("code-review", "审改动", &["工程"], false),
                skill("deep-dive", "深挖", &["写作", "写作/长文"], false),
            ],
            categories: vec!["工程".into(), "写作".into(), "写作/长文".into()],
            root: None, extra: Default::default(),
        };
        let all = visible_skills(&skills, None, "");
        assert_eq!(all.len(), 2, "隐藏的不在主列表");
        assert_eq!(all[0].name, "code-review");
        let hidden = hidden_skills(&skills);
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].name, "ask-matt");
    }

    #[test]
    fn 类别筛选含后代_搜名字和描述() {
        let skills = Skills {
            skills: vec![
                skill("code-review", "审查改动", &["工程"], false),
                skill("postmortem", "事故复盘", &["工程/事故"], false),
                skill("weekly", "周报", &["写作"], false),
            ],
            categories: vec!["工程".into(), "工程/事故".into(), "写作".into()],
            root: None, extra: Default::default(),
        };
        let eng = visible_skills(&skills, Some("工程"), "");
        assert_eq!(eng.len(), 2, "父类别含后代");
        let q = visible_skills(&skills, None, "复盘");
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].name, "postmortem", "搜描述也算命中");
        let none = visible_skills(&skills, Some("∅"), "");
        assert!(none.is_empty(), "三条都挂了类别");
    }

    #[test]
    fn 默认提示词有标记_筛选能捞出() {
        let p = |name: &str, cats: &[&str]| Prompt {
            name: name.into(), description: None,
            summary: Some(format!("摘要 of {name}")),
            category: cats.iter().map(|c| c.to_string()).collect(),
            load: Some(serde_json::json!({"state": "ok"})),
            extra: Default::default(),
        };
        let prompts = Prompts {
            prompts: vec![p("smart-ass", &[]), p("deep", &["写作"])],
            categories: vec!["写作".into()], default: Some("smart-ass".into()),
            extra: Default::default(),
        };
        let def = visible_prompts(&prompts, Some("@default"), "");
        assert_eq!(def.len(), 1);
        assert_eq!(def[0].name, "smart-ass");
        let q = visible_prompts(&prompts, None, "deep");
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].name, "deep");
    }

    #[test]
    fn 扩展五类各有行_筛选对得上() {
        let exts = Extensions {
            installed: vec!["nous".into()],
            loadable: vec!["nous".into()],
            builtin: vec!["coding-tools".into()],
            defaults: vec!["nous".into()],
            categories: vec![],
            protected: vec![],
            catalog: vec![Extension {
                name: "nous".into(), version: None, summary: Some("核心".into()),
                sections: vec![], category: vec![], is_default: true, tracked: true,
                file: None, entry: None, needs: None, extra: Default::default(),
            }],
            archived: vec![],
            variants: Default::default(),
            load: Default::default(),
            repo: None, nameplates: None, local: None, live: None, extra: Default::default(),
        };
        let all = ext_rows(&exts, None, "");
        assert_eq!(all.len(), 2, "一条 catalog + 一条 builtin");
        assert_eq!(all[0].kind, ExtKind::On);
        assert_eq!(all[1].kind, ExtKind::Builtin);
        let builtin = ext_rows(&exts, Some("@builtin"), "");
        assert_eq!(builtin.len(), 1);
        assert_eq!(builtin[0].name, "coding-tools");
    }

    #[test]
    fn 段切换计数是可见条数不是总条数() {
        let skills = Skills {
            skills: vec![skill("a", "", &[], false), skill("hidden", "", &[], true)],
            categories: vec![], root: None, extra: Default::default(),
        };
        let (n, _, _) = counts(Some(&skills), None, None);
        assert_eq!(n, 1, "隐藏的不算进段按钮");
    }
}
