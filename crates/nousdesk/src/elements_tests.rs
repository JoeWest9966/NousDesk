#[cfg(test)]
mod tests {
    use crate::elements::{parse_draft, set_list, set_scalar, unused_id};
    use nous_engine::{Element, ElementKind, ElementReceipt, Elements};
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
}
