#[cfg(test)]
mod tests {
    use crate::resources::*;
    use nous_engine::types::{
        ArchivedUnit, ExtItem, Extension, Extensions, Prompt, Prompts, RepoArchive, RepoVariants,
        Skill, Skills, VariantBinding,
    };

    /// 老形状的一份 `Extensions`（没有 `items`）：`ext_rows` 的回退分支用它。
    fn exts_only(view: &Extensions) -> ExtView<'_> {
        ExtView { exts: view, variants: None, archive: None }
    }

    fn skill(name: &str, desc: &str, cats: &[&str], hidden: bool) -> Skill {
        let mut s = Skill {
            name: name.into(),
            description: Some(desc.into()),
            category: cats.iter().map(|c| c.to_string()).collect(),
            hidden,
            tracked: Some(true),
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
            tracked: Some(true),
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
            items: vec![],
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
        let all = ext_rows(exts_only(&exts), None, "");
        assert_eq!(all.len(), 2, "一条 catalog + 一条 builtin");
        assert_eq!(all[0].kind, ExtKind::On);
        assert_eq!(all[1].kind, ExtKind::Builtin);
        let builtin = ext_rows(exts_only(&exts), Some("@builtin"), "");
        assert_eq!(builtin.len(), 1);
        assert_eq!(builtin[0].name, "coding-tools");
    }

    /// 引擎 0.1.9 的 `items[]`：`kind` 决定档位，`category` / `summary` 直接可画。
    /// 这条钉住两件事：老 JS 插件不能落进「加载失败」（那样会让人反复点重装），
    /// 以及 `items` 非空时**不再**去看老形状（否则段计数和列表会打架）。
    #[test]
    fn items_形状按_kind_分档() {
        let exts = Extensions {
            items: vec![
                item("native-one", "native", "running", Some("工具"), Some("跑着的")),
                item("old-js", "legacy-js", "crashed", None, None),
                item("broken", "invalid", "crashed", None, None),
                item("off-one", "native", "disabled", None, None),
            ],
            // 老形状故意留着垃圾：items 非空时它必须完全不参与。
            catalog: vec![Extension {
                name: "ghost".into(), version: None, summary: None, sections: vec![],
                category: vec![], is_default: false, tracked: false, file: None,
                entry: None, needs: None, extra: Default::default(),
            }],
            ..Default::default()
        };
        let all = ext_rows(exts_only(&exts), None, "");
        assert_eq!(all.len(), 4, "有 items 就只画 items");
        assert_eq!(all[0].name, "native-one");
        assert_eq!(all[0].kind, ExtKind::On);
        assert_eq!(all[1].kind, ExtKind::Legacy);
        assert_eq!(all[2].kind, ExtKind::Failed);
        assert_eq!(all[3].kind, ExtKind::Failed, "没跑起来的不冒充「启用中」");

        assert_eq!(ext_rows(exts_only(&exts), Some("@legacy"), "").len(), 1);
        assert_eq!(ext_rows(exts_only(&exts), Some("@on"), "").len(), 1);
        assert_eq!(ext_rows(exts_only(&exts), None, "跑着").len(), 1, "搜索看 summary");
        assert_eq!(counts(None, None, Some(exts_only(&exts))).2, 4);
    }

    /// 真机上的样子：核心三条装的还是 TS 时代的 JS 插件（`kind == "legacy-js"` +
    /// `core == true`）。它**是内置的**，只是跑不起来 —— 档位与「待重写」这个标记正交。
    #[test]
    fn 核心的旧_js_进内置档_chip_仍写待重写() {
        let exts = Extensions {
            items: vec![
                ExtItem { core: true, ..item("nous", "legacy-js", "crashed", None, None) },
                ExtItem { core: true, ..item("skills", "legacy-js", "crashed", None, None) },
                item("123", "legacy-js", "crashed", None, None),
            ],
            ..Default::default()
        };
        let all = ext_rows(exts_only(&exts), None, "");
        assert_eq!(all[0].kind, ExtKind::Builtin, "核心先判：引擎自带的就在「内置」里");
        assert!(all[0].legacy, "但它是老 JS，chip 要写「待重写」");
        assert_eq!(all[0].chip().0, "待重写（旧 JS）");
        assert_eq!(all[2].kind, ExtKind::Legacy, "不是核心的老 JS 才单独成档");

        // 桶里的数要能加得起来：内置 2 + 待重写 1 = 全部 3。
        assert_eq!(ext_rows(exts_only(&exts), Some("@builtin"), "").len(), 2);
        assert_eq!(ext_rows(exts_only(&exts), Some("@legacy"), "").len(), 1);
        assert_eq!(counts(None, None, Some(exts_only(&exts))).2, 3);
        assert_eq!(all[0].chip().2, true, "危险色：这一档确实要人动手");
    }

    fn item(
        name: &str,
        kind: &str,
        state: &str,
        category: Option<&str>,
        summary: Option<&str>,
    ) -> ExtItem {
        ExtItem {
            name: name.into(),
            version: "0.1.0".into(),
            kind: kind.into(),
            category: category.map(Into::into),
            summary: summary.map(Into::into),
            loaded: kind == "native",
            core: false,
            pid: None,
            state: Some(state.into()),
            error: None,
            extra: Default::default(),
        }
    }

    /// TS 时代的「变体」与「已归档」两档现在各有各的端点（`/v1/repos/extend/*`）：
    /// 合成在一处做，五档才齐。核心三条靠 `core` 进「内置」，不是靠名字硬编码。
    #[test]
    fn 变体与归档各成一档_核心三条进内置档() {
        let exts = Extensions {
            items: vec![
                item("nous", "native", "running", Some("工具"), Some("核心")),
                ExtItem {
                    core: true,
                    ..item("skills", "native", "running", None, None)
                },
                item("mine", "native", "running", None, None),
            ],
            ..Default::default()
        };
        let variants = RepoVariants {
            variants: [(
                "extend".to_string(),
                [(
                    "mine@abc".to_string(),
                    VariantBinding { root: "/tmp/wt".into(), branch: "variant/extend/abc".into(), extra: Default::default() },
                )]
                .into_iter()
                .collect(),
            )]
            .into_iter()
            .collect(),
            extra: Default::default(),
        };
        let archive = RepoArchive {
            items: vec![ArchivedUnit { name: "old-card".into(), extra: Default::default() }],
            extra: Default::default(),
        };
        let view = ExtView { exts: &exts, variants: Some(&variants), archive: Some(&archive) };

        let all = ext_rows(view, None, "");
        assert_eq!(all.len(), 5, "3 条清单 + 1 个变体 + 1 条归档");
        let kind_of = |name: &str| all.iter().find(|r| r.name == name).map(|r| r.kind);
        assert_eq!(kind_of("skills"), Some(ExtKind::Builtin), "core=true 就是内置档");
        assert_eq!(kind_of("mine"), Some(ExtKind::On));
        assert_eq!(kind_of("mine@abc"), Some(ExtKind::Variant));
        assert_eq!(kind_of("old-card"), Some(ExtKind::Archived));

        assert_eq!(ext_rows(view, Some("@variant"), "").len(), 1);
        assert_eq!(ext_rows(view, Some("@archived"), "").len(), 1);
        assert_eq!(ext_rows(view, Some("@builtin"), "").len(), 1);
        // 段按钮上的数字 = 列表能看到的行数，两边不许打架。
        assert_eq!(counts(None, None, Some(view)).2, 5);

        // 三份数据缺两份时只少那两档，已有档位不许跟着变空。
        let only_items = ext_rows(exts_only(&exts), None, "");
        assert_eq!(only_items.len(), 3);
    }

    /// 卸载过的（内核 `kind/state = absent`）落在「本机没装」那一档：
    /// 只有「安装 / 改类别」可做，文件面一律不给（目录在 trash 里）。
    #[test]
    fn 卸载过的落进本机没装档() {
        let exts = Extensions {
            items: vec![ExtItem { kind: "absent".into(), state: Some("absent".into()), ..item("gone-one", "absent", "absent", None, None) }],
            ..Default::default()
        };
        let all = ext_rows(exts_only(&exts), None, "");
        assert_eq!(all.len(), 1, "卸载过的不许从列表里蒸发");
        assert_eq!(all[0].kind, ExtKind::Off);
        assert!(!file_backed(Some(all[0].kind)), "文件面按名字找只会 404");
    }

    /// 「编辑 / 发布 / 复制」走文件面，只对源码根里真有的目录成立。
    /// 已归档（在 `.archive/`）和旧 JS（清单都不合法）点进去只会 404。
    #[test]
    fn 文件面动作只给源码根里的档位() {
        assert!(file_backed(None), "技能 / 提示词段照旧全给");
        assert!(file_backed(Some(ExtKind::On)));
        assert!(
            !file_backed(Some(ExtKind::Off)),
            "本机没装：目录在 trash 里，源码根里没有它"
        );
        assert!(file_backed(Some(ExtKind::Builtin)));
        assert!(
            !file_backed(Some(ExtKind::Variant)),
            "变体是另一棵工作树，源码根里没有这个目录"
        );
        assert!(file_backed(Some(ExtKind::Failed)), "坏清单还能改，所以给编辑");
        assert!(!file_backed(Some(ExtKind::Archived)));
        assert!(!file_backed(Some(ExtKind::Legacy)));
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

    /// `Desk#13` 返工：能力型端点不许删，入口按 capabilities 灰；
    /// 判定只在 `capability.rs`，这里用同一纯函数核对三个仓的等票端点。
    #[test]
    fn write_entries_are_wait_ticket_and_gray_until_caps_enable_them() {
        let caps: nous_engine::Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.3" },
            "modules": [
                { "name": "Skill", "enabled": true, "endpoints": ["GET /v1/skills"] },
                { "name": "prompts", "enabled": true, "endpoints": ["GET /v1/prompts"] },
                { "name": "Nous_Extend", "enabled": false,
                  "reason": { "code": "frozen", "message": "扩展运行时已冻结" } },
            ],
            "runtime": { "provider": { "kind": "faux" } }
        })).unwrap();
        for repo in ["skills", "prompts", "extensions"] {
            let reason = crate::capability::endpoint_disabled_reason(&caps, "POST", &format!("/v1/{repo}/create"))
                .unwrap_or_else(|| panic!("{repo}/create 等票期间不该可点"));
            assert!(!reason.is_empty(), "灰理由不能是空串");
        }
    }

    /// 引擎 0.1.8 起三仓文件面真的声明了：同一条判定必须放行（否则线上还是灰的）。
    #[test]
    fn declared_write_endpoints_are_enabled() {
        let mut modules = Vec::new();
        for (name, segment) in [("Skill", "skills"), ("prompts", "prompts"), ("Nous_Extend", "extensions")] {
            modules.push(serde_json::json!({
                "name": name,
                "enabled": true,
                "endpoints": [
                    format!("POST /v1/{segment}/create"),
                    format!("GET /v1/{segment}/files"),
                    format!("GET /v1/{segment}/file"),
                    format!("PUT /v1/{segment}/file"),
                    format!("POST /v1/{segment}/publish"),
                ],
            }));
        }
        let caps: nous_engine::Capabilities = serde_json::from_value(serde_json::json!({
            "contract": 1,
            "server": { "name": "nous", "version": "0.1.8" },
            "modules": modules,
            "runtime": { "provider": { "kind": "faux" } }
        }))
        .unwrap();
        for repo in ["skills", "prompts", "extensions"] {
            for (method, op) in [("GET", "files"), ("GET", "file"), ("PUT", "file"), ("POST", "create"), ("POST", "publish")] {
                let path = format!("/v1/{repo}/{op}");
                assert_eq!(
                    crate::capability::endpoint_disabled_reason(&caps, method, &path),
                    None,
                    "{method} {path} 已声明，入口不该再灰"
                );
            }
        }
    }

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

/// 垃圾站的条目名是引擎切的 `<name>-<epochms>`，客户端只管显示；
/// 时间口径只说「多久以前」——毫秒对人没有意义，但「刚刚 / 3 天前」有意义。
#[test]
fn 垃圾站时间口径() {
    use crate::resource_write::stamp_label;
    assert_eq!(stamp_label(0), "时间不详");
    let now = std::time::SystemTime::now();
    let ms = |back: u64| {
        now.duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64 - back
    };
    assert_eq!(stamp_label(ms(30_000)), "刚刚");
    assert_eq!(stamp_label(ms(5 * 60_000)), "5 分钟前");
    assert_eq!(stamp_label(ms(3 * 3_600_000)), "3 小时前");
    assert_eq!(stamp_label(ms(2 * 86_400_000)), "2 天前");
}

/// `GET /v1/extensions/trash` 的回包形状（抄 j1900 真机那条）：
/// `name` 从目录名切好、`has_manifest` 决定「装回来」有没有意义。
#[test]
fn 垃圾站回包形状() {
    let raw = serde_json::json!({
        "items": [{
            "name": "123",
            "entry": "123-1795392796720",
            "trashed_at": 1795392796720u64,
            "has_manifest": true,
            "dir": "data/extend-trash/123-1795392796720"
        }]
    });
    let list: nous_engine::types::TrashList = serde_json::from_value(raw).unwrap();
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].name, "123");
    assert_eq!(list.items[0].entry, "123-1795392796720");
    assert!(list.items[0].has_manifest);
    // 引擎将来多回字段不许炸（哑协议：未知字段原样收着）。
    let extra: nous_engine::types::TrashList =
        serde_json::from_value(serde_json::json!({"items": [], "next": 1})).unwrap();
    assert!(extra.items.is_empty());
}

/// 段是三个平铺的筛选器，各占一个槽位（类别筛选 / 搜索词按槽位各存一份）。
///
/// 2026-11-24 删掉「内容」那层：它只把技能和提示词裹在一起、多一次点击，本身
/// 不装任何东西。判据钉两件事 —— 默认落在技能段，三段槽位互不相同（相同就会
/// 出现「切段时搜索词 / 类别串到另一段」）。
#[test]
fn 三段平铺各占一个槽位() {
    use crate::resources::Section;
    assert_eq!(Section::default(), Section::Skills, "默认落在技能段");
    assert_eq!(Section::Skills.label(), "技能");
    assert_eq!(Section::Prompts.label(), "提示词");
    assert_eq!(Section::Extensions.label(), "扩展");
    let mut slots = [
        Section::Skills.slot(),
        Section::Prompts.slot(),
        Section::Extensions.slot(),
    ];
    slots.sort();
    assert_eq!(slots, [0, 1, 2], "三段槽位必须互不相同");
}

/// 窄窗下钻的两层：默认在条目列表，点「分类」上树，点类别回列表。
/// `pop()` 是「退一层」的唯一入口（Esc / 返回按钮共用），到头不许来回抖。
#[test]
fn 窄窗下钻只有两层() {
    use crate::resources::ResCol;
    assert_eq!(ResCol::default(), ResCol::Items, "默认是条目列表");
    assert_eq!(ResCol::Tree.pop(), ResCol::Items);
    assert_eq!(ResCol::Items.pop(), ResCol::Items, "在列表上再退还是列表");
}

/// 扩展段的 `⋯` 菜单：空垃圾站时只有两项，有东西才多出「垃圾站 N」。
///
/// 判据钉两件事 —— ① 垃圾站在树里已经有一行且常显（2026-11-23），空着再给一个
/// 菜单项就是同一页两个入口；② 顺序固定（重载 → 类别词表 → 垃圾站），菜单的
/// 肌肉记忆跟段按钮一样不能乱跳。
#[test]
fn 扩展段菜单空垃圾站只有两项() {
    use crate::resources::{ExtMenu, ext_menu_items};
    assert_eq!(
        ext_menu_items(0),
        vec![ExtMenu::Reload, ExtMenu::Categories]
    );
    assert_eq!(
        ext_menu_items(3),
        vec![ExtMenu::Reload, ExtMenu::Categories, ExtMenu::Trash(3)],
        "有东西才出垃圾站，且带份数"
    );
}

/// ↗ 圆按钮只认 http/https：`xdg-open` 的 argv 不能让远端替我们决定。
///
/// 判据是**白名单**而不是黑名单 —— 新协议（`vscode://`、`file:`、`javascript:`）
/// 默认落在「不开」那边，宁可少开一个，不能把 `file:/etc/shadow` 交给系统处理。
#[test]
fn 只有http链接才会被交给系统浏览器() {
    use crate::resources::external_url;

    // 正常形态：http / https，大小写不敏感，前后空白剃掉。
    assert_eq!(
        external_url("https://github.com/JoeWest9966/Nous-skills.git"),
        Some("https://github.com/JoeWest9966/Nous-skills.git".into())
    );
    assert_eq!(
        external_url("  http://192.168.1.7:4789/x?a=1&b=2  "),
        Some("http://192.168.1.7:4789/x?a=1&b=2".into()),
        "query 里的 & 原样留着，spawn 不过 shell"
    );
    assert_eq!(
        external_url("HTTPS://GitHub.com/x"),
        Some("HTTPS://GitHub.com/x".into()),
        "协议大小写不敏感，但原串不改写"
    );

    // 不是 http/https 的一律不开。
    assert_eq!(external_url(""), None);
    assert_eq!(external_url("   "), None);
    assert_eq!(external_url("file:///etc/shadow"), None);
    assert_eq!(external_url("javascript:alert(1)"), None);
    assert_eq!(external_url("vscode://file/tmp"), None);
    assert_eq!(external_url("git@github.com:JoeWest9966/Nous-skills.git"), None, "scp 风格不是 URL");
    assert_eq!(external_url("/opt/Nous-skills"), None, "本地路径不是远端");

    // 没有主机段的不算链接（`https:///x` 交给 xdg-open 只会白开一个浏览器）。
    assert_eq!(external_url("https://"), None);
    assert_eq!(external_url("https:///x"), None);
    assert_eq!(external_url("http://?q=1"), None);

    // 带空白 / 控制字符的整串丢掉：那是拼错了，不是链接。
    assert_eq!(external_url("https://a b.com"), None);
    assert_eq!(external_url("https://a.com\nrm -rf /"), None);
}
