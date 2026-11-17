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
            tracked: true,
            name: name.into(), description: None,
            summary: Some(format!("摘要 of {name}")),
            category: cats.iter().map(|c| c.to_string()).collect(),
            load: Some(serde_json::json!({"state": "ok"})),
            extra: Default::default(),
        };
        let prompts = Prompts {
            prompts: vec![p("smart-ass", &[]), p("deep", &["写作"])],
            categories: vec!["写作".into()], default: Some("smart-ass".into()),
            root: None,
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

// ── 写操作那一刀的纯函数 ────────────────────────────────────────────────

mod write_tests {
    use crate::resource_write::*;

    #[test]
    fn 类别输入框按逗号空白切() {
        assert_eq!(parse_categories("写作, 效率"), vec!["写作", "效率"]);
        assert_eq!(parse_categories("写作，效率"), vec!["写作", "效率"]);
        assert_eq!(parse_categories("  写作   效率  "), vec!["写作", "效率"]);
        assert_eq!(parse_categories(""), Vec::<String>::new());
        assert_eq!(parse_categories(" , ， "), Vec::<String>::new());
        // 层级用斜杠，别被切碎（引擎的类别路径就是 a/b）
        assert_eq!(parse_categories("写作/长文"), vec!["写作/长文"]);
    }

    #[test]
    fn 技能落盘时_description_在_frontmatter_而_nous_json_的_summary_是空串() {
        let f = landing_files(Kind::Skill, "weekly-digest", "整理周报", "写作, 效率", "", "/opt/Nous-skills");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].0, "/opt/Nous-skills/weekly-digest/SKILL.md");
        let sk = &f[0].1;
        assert!(sk.starts_with("---\nname: weekly-digest\n"));
        assert!(sk.contains(r#"description: "整理周报""#), "description 要在 frontmatter 里：{sk}");
        assert!(sk.trim_end().ends_with("# weekly-digest"));
        assert_eq!(f[1].0, "/opt/Nous-skills/weekly-digest/nous.json");
        let nj = &f[1].1;
        assert!(nj.contains(r#""summary": """#), "技能的 summary 是空串：{nj}");
        assert!(nj.contains("\"写作\"") && nj.contains("\"效率\""));
    }

    #[test]
    fn 提示词正文没有_frontmatter_summary_在_nous_json() {
        let f = landing_files(Kind::Prompt, "strict-review", "两条轴", "工程", "", "/opt/Nous-prompts");
        assert_eq!(f[0].0, "/opt/Nous-prompts/strict-review/PROMPT.md");
        // 正文空着就落一个标题；**绝不能有 frontmatter**（技能才有）
        assert_eq!(f[0].1, "# strict-review\n");
        assert!(!f[0].1.contains("---"), "提示词正文不许有 frontmatter");
        assert!(f[1].1.contains(r#""summary": "两条轴""#));

        // 给了正文就用它
        let f2 = landing_files(Kind::Prompt, "p", "s", "", "自己写的正文\n", "/r");
        assert_eq!(f2[0].1, "自己写的正文\n");
    }

    #[test]
    fn 扩展落盘清单加空壳入口() {
        let f = landing_files(Kind::Ext, "weekly-card", "一张图", "工具", "", "/opt/Nous-extend");
        assert_eq!(f[0].0, "/opt/Nous-extend/weekly-card/nous.json");
        let m = &f[0].1;
        for need in ["\"version\": \"0.1.0\"", "\"entry\": \"default\"", "\"sections\": []", r#""summary": "一张图""#] {
            assert!(m.contains(need), "清单缺 {need}：{m}");
        }
        assert_eq!(f[1].0, "/opt/Nous-extend/weekly-card/index.js");
        assert!(f[1].1.contains("defineExtension({ name: \"weekly-card\" })"));
    }

    #[test]
    fn 空名字在预览里占位而不是渲染成空路径() {
        let f = landing_files(Kind::Skill, "  ", "d", "", "", "/root");
        assert_eq!(f[0].0, "/root/<name>/SKILL.md");
    }

    #[test]
    fn 引号和换行要转义_不然预览是假的() {
        let f = landing_files(Kind::Skill, "x", "他说\"你好\"", "", "", "/r");
        assert!(f[0].1.contains(r#"description: "他说\"你好\"""#), "{}", f[0].1);
    }

    #[test]
    fn 必填兜底只管空着这一种() {
        assert_eq!(missing_field(Kind::Skill, "", "d"), Some("技能名不能为空"));
        assert_eq!(missing_field(Kind::Prompt, "n", ""), Some("summary 不能空 —— 人靠这一行决定选不选它"));
        assert_eq!(missing_field(Kind::Ext, "n", "  "), Some("一句话说明不能空 —— 卡片上就靠它认人"));
        assert_eq!(missing_field(Kind::Skill, "n", "d"), None);
    }

    #[test]
    fn 编辑器判脏只比正文() {
        assert!(!is_dirty("a\n", "a\n"));
        assert!(is_dirty("a\n", "a\nb\n"));
        // 行尾空格也算改动 —— 保守一点，宁可多问一句
        assert!(is_dirty("a\n", "a \n"));
    }
}
