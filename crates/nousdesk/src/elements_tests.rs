#[cfg(test)]
mod tests {
    use crate::elements::{
        apply_written, conflict_hint, engine_status, parse_draft, remove_written, set_list,
        set_scalar, unused_id,
    };
    use nous_engine::{Element, ElementKind, ElementReceipt, Elements, EngineError};
    use serde_json::json;

    /// 引擎回一条元素的最简形状：`fields` + `etag` + `state`。
    fn element(id: &str, fields: serde_json::Value) -> Element {
        serde_json::from_value(json!({
            "kind": "groups",
            "id": id,
            "fields": fields,
            "etag": "abc12345",
            "state": "ok",
        }))
        .expect("形状要能反序列化")
    }

    #[test]
    fn 改一个字段不动别的键() {
        // 引擎以后加的未知键必须原样留着 —— 客户端替它删就等于吃掉新协议。
        let mut fields = json!({
            "id": "frontend",
            "name": "前端",
            "packages": ["react-rules"],
            "futureField": {"nested": [1, 2, 3]},
        });
        set_scalar(&mut fields, "name", "前端组".into());
        assert_eq!(fields["name"], json!("前端组"));
        assert_eq!(
            fields["futureField"],
            json!({"nested": [1, 2, 3]}),
            "未知键一个都不能动"
        );
        assert_eq!(fields["packages"], json!(["react-rules"]), "别的字段也别碰");
    }

    #[test]
    fn 空串不删键() {
        // `desc: ""` 与「没有 desc」是两件事：删不删由用户决定，不由输入框决定。
        let mut fields = json!({"id": "g", "name": "G", "desc": "原来的"});
        set_scalar(&mut fields, "desc", String::new());
        assert_eq!(fields.get("desc"), Some(&json!("")));
    }

    #[test]
    fn 数组字段认中文逗号和空格() {
        let mut fields = json!({"id": "g"});
        set_list(&mut fields, "packages", " react-rules, css-tokens，sql-rules ,,".into());
        assert_eq!(fields["packages"], json!(["react-rules", "css-tokens", "sql-rules"]));
        set_list(&mut fields, "packages", "   ".into());
        assert_eq!(fields["packages"], json!([]), "全空 = 空数组，不是缺键");
    }

    #[test]
    fn 非对象原样不动() {
        // 不是对象就不是合法元素：不在这里「修」它，保存时引擎给准确的那句话。
        let mut fields = json!(["a", "b"]);
        set_scalar(&mut fields, "name", "x".into());
        set_list(&mut fields, "tags", "x".into());
        assert_eq!(fields, json!(["a", "b"]));
    }

    #[test]
    fn json_解析失败带原文() {
        assert!(parse_draft("{\"a\": 1}").is_ok());
        let err = parse_draft("{\"a\": 1,}").expect_err("尾逗号不合法");
        assert!(err.starts_with("JSON 解析失败："), "{err}");
        assert!(err.contains("line"), "serde 的行号要留着，别自己编：{err}");
    }

    #[test]
    fn new_id_skips_taken_ones() {
        let data: Elements = serde_json::from_value(json!({
            "kind": "elements",
            "repo": {"root": "/tmp/x", "present": true},
            "groups": [
                {"id": "new-group", "fields": {"id": "new-group", "name": "A"}, "state": "ok"},
                {"id": "new-group-2", "fields": {"id": "new-group-2", "name": "B"}, "state": "ok"},
            ],
            "packages": [],
            "resolved": {"groups": {}, "missing": []},
        }))
        .expect("回包形状");
        assert_eq!(unused_id(ElementKind::Groups, Some(&data)), "new-group-3");
        assert_eq!(unused_id(ElementKind::Packages, Some(&data)), "new-package");
        assert_eq!(unused_id(ElementKind::Groups, None), "new-group");
    }

    #[test]
    fn 坏元素是红的且不算一条正常数据() {
        let broken: Element = serde_json::from_value(json!({
            "kind": "groups",
            "id": "oops",
            "fields": null,
            "etag": null,
            "state": "problem",
            "detail": "expected `,` at line 3",
        }))
        .expect("坏文件也要能反序列化");
        assert!(broken.broken());
        assert_eq!(broken.name(), "oops", "读不出来时退回 id，不编名字");
        assert!(broken.packages().is_empty(), "坏的没有引用可读");

        let ok = element("frontend", json!({"id": "frontend", "name": "前端", "packages": ["a"]}));
        assert!(!ok.broken());
        assert_eq!(ok.name(), "前端");
        assert_eq!(ok.packages(), vec!["a"]);
    }

    #[test]
    fn 断链只标那一条() {
        let data: Elements = serde_json::from_value(json!({
            "kind": "elements",
            "repo": {"root": "/tmp/x", "present": true},
            "groups": [],
            "packages": [],
            "resolved": {"groups": {}, "missing": ["frontend/gone", "other/ghost"]},
        }))
        .expect("回包形状");
        assert_eq!(data.resolved.missing_in("frontend"), vec!["gone"]);
        assert!(data.resolved.missing_in("nope").is_empty());
    }

    #[test]
    fn 回执的三句话各说各的() {
        let commit = |commit: Option<&str>, pushed: Option<&str>, replay: bool| {
            serde_json::from_value::<ElementReceipt>(json!({
                "kind": "groups",
                "id": "frontend",
                "commit": commit,
                "pushed": pushed,
                "replay": replay,
            }))
            .expect("回包形状")
        };
        let s = commit(Some("abcdef1234"), Some("ok"), false).summary("已保存");
        assert!(s.contains("abcdef1") && s.contains("已推送"), "{s}");

        let s = commit(Some("abcdef1234"), Some("failed"), false).summary("已保存");
        assert!(s.contains("推送失败"), "推不上去要说清楚本地已经提交：{s}");

        // 内容没变 / 重放都不是错误：`commit: null` 不许画成失败。
        let s = commit(None, None, false).summary("已保存");
        assert!(s.contains("没有产生新提交"), "{s}");
        let s = commit(None, None, true).summary("已保存");
        assert!(s.contains("没有重复写"), "{s}");
    }

    /// 一份两段齐全的空清单。
    fn empty() -> Elements {
        serde_json::from_value(json!({
            "kind": "elements",
            "repo": {"root": "/tmp/x", "present": true},
            "groups": [],
            "packages": [],
            "resolved": {"groups": {}, "missing": []},
        }))
        .expect("回包形状")
    }

    #[test]
    fn 写成功就把回执并进清单() {
        // 保存后不等 GET：指纹和内容当场就是新的，否则回填表单会读到旧快照
        // （2026-11-24 真机：存进去的 Z 在输入框里退回旧值，再敲一个字把 Z 吃掉）。
        let mut data = empty();
        data.groups.push(element("a", json!({"id": "a", "name": "A"})));
        data.groups.push(element("c", json!({"id": "c", "name": "C"})));
        data.packages.push(element("pkg", json!({"id": "pkg", "name": "P"})));

        apply_written(
            &mut data,
            ElementKind::Groups,
            "a",
            &json!({"id": "a", "name": "A2", "future": 1}),
            Some("newetag"),
        );
        let a = data.find(ElementKind::Groups, "a").expect("那条还在");
        assert_eq!(a.name(), "A2", "内容换成刚发出去的那份");
        assert_eq!(a.etag.as_deref(), Some("newetag"), "指纹跟上回执");
        assert_eq!(data.groups.len(), 2, "不是插一条新的");
        assert_eq!(data.find(ElementKind::Groups, "c").unwrap().name(), "C");
        assert_eq!(data.packages.len(), 1, "另一段一条不动");
        assert_eq!(data.find(ElementKind::Packages, "pkg").unwrap().name(), "P");

        // 新建：插进去并保持引擎的 id 序（否则下次刷新行会跳）。
        apply_written(
            &mut data,
            ElementKind::Groups,
            "b",
            &json!({"id": "b", "name": "B"}),
            Some("etag-b"),
        );
        let ids: Vec<&str> = data.groups.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"], "按 id 排序");

        // 指纹没给（引擎没回 etag）：不编一个假的，删除就会走「不带 base」。
        apply_written(&mut data, ElementKind::Groups, "b", &json!({"id": "b"}), None);
        assert_eq!(data.find(ElementKind::Groups, "b").unwrap().etag, None);
    }

    #[test]
    fn 删成功就就地摘掉那一条() {
        let mut data = empty();
        data.groups.push(element("a", json!({"id": "a"})));
        data.packages.push(element("a", json!({"id": "a"})));
        remove_written(&mut data, ElementKind::Groups, "a");
        assert!(data.find(ElementKind::Groups, "a").is_none());
        assert!(
            data.find(ElementKind::Packages, "a").is_some(),
            "同名的包是另一段，不该跟着消失"
        );
    }

    #[test]
    fn 删除回执不拿内容一致糊弄() {
        // 删除场景下「内容与盘上一致，没有产生新提交」是句错话：盘上确实变了。
        let receipt = |commit: Option<&str>, replay: bool| {
            serde_json::from_value::<ElementReceipt>(json!({
                "kind": "groups",
                "id": "a",
                "commit": commit,
                "replay": replay,
            }))
            .expect("回包形状")
        };
        let s = receipt(Some("abcdef1234"), false).summary_deleted();
        assert!(s.starts_with("已删除") && s.contains("abcdef1"), "{s}");
        let s = receipt(None, false).summary_deleted();
        assert!(s.contains("已删除") && !s.contains("内容与盘上一致"), "{s}");
        let s = receipt(None, true).summary_deleted();
        assert!(s.contains("没有重复写"), "{s}");
    }

    #[test]
    fn 冲突提示说清下一步() {
        let conflict = EngineError::Engine {
            status: 409,
            message: "If-Match 不符：groups/a.json 已被改过".into(),
            body: None,
        };
        assert_eq!(engine_status(&conflict), Some(409));
        let hint = conflict_hint(&conflict, "删除");
        assert!(hint.contains("已被改过"), "引擎原话逐字留着：{hint}");
        assert!(hint.contains("重新读取") && hint.contains("「删除」"), "{hint}");

        let gone = EngineError::Engine {
            status: 404,
            message: "没有这份组：groups/a".into(),
            body: None,
        };
        let hint = conflict_hint(&gone, "删除");
        assert!(hint.contains("不在了"), "404 不是冲突，别叫用户再点一次：{hint}");

        // 非 2xx 但跟冲突无关（500）：状态码照样能读出来，调用方自己决定不做冲突处理。
        let boom = EngineError::Engine {
            status: 500,
            message: "git 挂了".into(),
            body: None,
        };
        assert_eq!(engine_status(&boom), Some(500));
    }
}
