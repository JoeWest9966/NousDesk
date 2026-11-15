//! 37 · 节点图：把一条预设摊成「中心 + 一圈维度 + 外圈具体项」。
//!
//! 这里只有**纯函数**：`nodes` 把数据摊开、`layout` 算坐标。画线画块在 `templates.rs`。
//!
//! 为什么不在这儿算「能不能开」：那条判据在引擎（`resolved.*.state == "invalid"`），
//! 我们只把 `state` 抄进节点颜色。`resolved` 缺了也不报错 —— 退化成纯字段图，
//! 引擎以后多想一个维度，这边自动多一个节点。

use nous_engine::types::Preset;
use serde_json::Value;

/// 节点状态。**只用来挑颜色/线型**，不参与任何判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// 引擎说钉住了。
    Ok,
    /// 走默认（`state == "auto"` 或 `value == null`）。
    Auto,
    /// 引用失效 —— 这条预设现在开不了。
    Invalid,
    /// 引擎没给 `resolved`，从 `spec` 摊的字段，状态未知。
    Unknown,
}

impl State {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_bad(self) -> bool { matches!(self, State::Invalid) }
    pub(crate) fn is_auto(self) -> bool { matches!(self, State::Auto) }
}

/// 图上一个点。`layer`：0 = 中心，1 = 维度，2 = 具体项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    pub id: String,
    /// 维度名（模型 / 思考 / …）。第二圈这里是具体项的类型。
    pub label: String,
    /// 显示值。空串 = 这一维没写（界面上写「未设」）。
    pub value: String,
    pub state: State,
    pub layer: u8,
    /// 挂在谁下面。单层之后恒为 `Some("center")`（中心自己是 `None`）——
    /// 留着是因为以后真要画第二圈时不该再改一遍结构。
    pub parent: Option<String>,
    /// 这一维是引用（指向盘上别的东西）。决定点的颜色：字段蓝、引用绿。
    pub is_ref: bool,
}

/// 第一圈的七个维度，顺序就是绕一圈的顺序。`(key, 中文名)`。
pub(crate) const DIMS: [(&str, &str); 7] = [
    ("model", "模型"),
    ("thinking", "思考"),
    ("skills", "技能"),
    ("extensions", "扩展"),
    ("template", "模板"),
    ("cwd", "目录"),
    ("machine", "机器"),
];

fn state_of(v: &Value) -> State {
    match v.get("state").and_then(Value::as_str) {
        Some("ok") => State::Ok,
        Some("auto") => State::Auto,
        Some("invalid") => State::Invalid,
        _ => State::Unknown,
    }
}

fn s_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `resolved` 里某一维的数组（`skills` / `extensions`）。
fn list_refs(v: &Value) -> Vec<(String, State)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|x| {
                    (
                        s_of(x, "ref").unwrap_or_else(|| "—".into()),
                        state_of(x),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 这一维是不是「引用」（指向盘上另一个东西）。
/// demo 里字段点画蓝、引用点画绿，这里就是那个分界。
pub(crate) fn is_ref_dim(key: &str) -> bool {
    matches!(key, "skills" | "extensions" | "template")
}

/// 把一条预设摊成节点。**单层**：中心 + 一圈维度，没有第二圈。
///
/// demo 的 37 就只有一层 —— 两层会把「放射状」变成「一坨」，这正是第一版不对的地方。
/// 多值维度（技能 4 个）在值里写 `deep-dive +3`，不再往下挂子节点。
pub(crate) fn nodes(p: &Preset) -> Vec<Node> {
    let mut out = vec![Node {
        id: "center".into(),
        label: p.name.clone(),
        value: p.summary.clone(),
        state: if p.spec_error.is_some() {
            State::Invalid
        } else {
            State::Ok
        },
        layer: 0,
        parent: None,
        is_ref: false,
    }];

    let Some(res) = p.resolved.as_ref().filter(|r| r.is_object()) else {
        return spec_nodes(p, out);
    };

    for (key, label) in DIMS {
        let Some(v) = res.get(key) else { continue };
        let mut state = state_of(v);
        let value = match key {
            "skills" | "extensions" => {
                let refs = list_refs(v);
                match refs.len() {
                    0 => String::new(),
                    1 => refs[0].0.clone(),
                    n => format!("{} +{}", refs[0].0, n - 1),
                }
            }
            "template" => s_of(v, "ref").unwrap_or_else(|| "跟随默认".into()),
            "cwd" | "machine" => s_of(v, "value").unwrap_or_else(|| "默认".into()),
            _ => s_of(v, "value").unwrap_or_default(),
        };

        // 「值其实是默认」但引擎仍报 ok 的那几种（template 没钉、cwd/machine 没设）：
        // 颜色跟着**看得见的字**走，不然会出现「跟随默认」配一个绿点。
        if matches!(state, State::Ok)
            && matches!(value.as_str(), "跟随默认" | "默认" | "")
        {
            state = State::Auto;
        }

        out.push(Node {
            id: format!("dim:{key}"),
            label: label.into(),
            value,
            state,
            layer: 1,
            parent: Some("center".into()),
            is_ref: is_ref_dim(key),
        });
    }

    out
}

/// `resolved` 没给时的兜底：照 `spec` 的顶层 key 摊一圈，状态一律未知。
/// 未知字段也在这儿显形 —— 不许因为不认识就吞掉。
fn spec_nodes(p: &Preset, mut out: Vec<Node>) -> Vec<Node> {
    let Some(spec) = p.spec.as_ref().and_then(Value::as_object) else {
        return out;
    };
    // `v` / `name` / `summary` / `description` / `category` 已经在中心或别处展示过了。
    const SKIP: [&str; 5] = ["v", "name", "summary", "description", "category"];
    for (k, v) in spec {
        if SKIP.contains(&k.as_str()) {
            continue;
        }
        let value = match v {
            Value::String(s) => s.clone(),
            Value::Bool(b) => if *b { "是".into() } else { "否".into() },
            Value::Null => String::new(),
            Value::Array(a) => format!("{} 项", a.len()),
            other => other.to_string(),
        };
        out.push(Node {
            id: format!("spec:{k}"),
            // 认识的维度用中文名，不认识的照抄 key —— 透传优先于好看。
            label: DIMS
                .iter()
                .find(|(dk, _)| *dk == k.as_str())
                .map(|(_, l)| (*l).to_string())
                .unwrap_or_else(|| k.clone()),
            value,
            state: State::Unknown,
            layer: 1,
            parent: Some("center".into()),
            is_ref: is_ref_dim(k),
        });
    }
    out
}

/// 摆完的点：像素坐标（相对画布左上角）。顺序与 `nodes` 一致。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    pub x: f32,
    pub y: f32,
}

/// 极坐标布局。
///
/// - 中心在画布正中。
/// - 维度圈：`DIMS` 顺序从 12 点开始顺时针均分。
/// - 具体项圈：挂在父维度角度附近，同一父的兄弟左右摊开；半径比维度圈大 60%，
///   所以即使角度撞了也不会压在一起。
///
/// 不用 force layout：每次刷新位置都在跳，等于找不到东西。
pub(crate) fn layout(nodes: &[Node], w: f32, h: f32) -> Vec<Placed> {
    // demo 37 的几何：中心圆在画布中间偏上，一圈点均匀分布在一个正圆上。
    // 用正圆不用椭圆 —— 椭圆会让「放射」看上去像被压扁的饼。
    let (cx, cy) = (w / 2., h / 2. - 10.);
    let r = (w * 0.40).min(h * 0.37);
    let tau = std::f32::consts::TAU;
    let ring = nodes.len().saturating_sub(1).max(1);

    nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            if n.layer == 0 {
                return Placed { x: cx, y: cy };
            }
            let a = -std::f32::consts::FRAC_PI_2 + tau * (i - 1) as f32 / ring as f32;
            Placed {
                x: cx + r * a.cos(),
                y: cy + r * a.sin(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn preset(resolved: Value) -> Preset {
        serde_json::from_value(json!({
            "name": "deep-dive",
            "summary": "一个技术问题挖到底",
            "description": "",
            "category": ["工程"],
            "spec": { "v": 1, "name": "deep-dive", "model": { "provider": "volc-ark" } },
            "specError": null,
            "tracked": true,
            "dir": "presets/deep-dive",
            "resolved": resolved,
        }))
        .unwrap()
    }

    #[test]
    fn 中心加七维_顺序固定() {
        let p = preset(json!({
            "model": { "state": "ok", "value": "volc-ark/x" },
            "thinking": { "state": "ok", "value": "medium" },
            "skills": [{ "state": "ok", "ref": "deep-dive" }],
            "extensions": [],
            "template": { "state": "ok", "ref": "smart-ass" },
            "cwd": { "state": "auto", "value": null },
            "machine": { "state": "auto", "value": null },
        }));
        let ns = nodes(&p);
        assert_eq!(ns[0].layer, 0);
        let dims: Vec<&str> = ns
            .iter()
            .filter(|n| n.layer == 1)
            .map(|n| n.label.as_str())
            .collect();
        assert_eq!(dims, vec!["模型", "思考", "技能", "扩展", "模板", "目录", "机器"]);
        // 空的扩展也占一个维度节点（「没有」也是信息），但不挂子节点。
        let ext = ns.iter().find(|n| n.id == "dim:extensions").unwrap();
        assert_eq!(ext.value, "");
        assert_eq!(
            ns.iter()
                .filter(|n| n.parent.as_deref() == Some("dim:extensions"))
                .count(),
            0
        );
    }

    #[test]
    fn 自动档与失效各有状态() {
        let p = preset(json!({
            "model": { "state": "invalid", "value": null },
            "cwd": { "state": "auto", "value": null },
        }));
        let ns = nodes(&p);
        let m = ns.iter().find(|n| n.id == "dim:model").unwrap();
        assert!(m.state.is_bad());
        let c = ns.iter().find(|n| n.id == "dim:cwd").unwrap();
        assert!(c.state.is_auto());
        assert_eq!(c.value, "默认");
    }

    #[test]
    fn 没有resolved就摊spec_未知字段不丢() {
        let p = preset(Value::Null);
        let ns = nodes(&p);
        assert!(ns.len() >= 2);
        // 中心看 spec_error，其余从 spec 摊出来的都是「引擎没说」的 Unknown。
        assert!(ns[1..].iter().all(|n| n.state == State::Unknown));
        // spec 里的 model 认得，用中文名。
        assert!(ns.iter().any(|n| n.label == "模型"));
    }

    #[test]
    fn 布局不重叠且在画布内() {
        let p = preset(json!({
            "model": { "state": "ok", "value": "volc-ark/x" },
            "thinking": { "state": "ok", "value": "medium" },
            "skills": [
                { "state": "ok", "ref": "a" }, { "state": "ok", "ref": "b" },
                { "state": "invalid", "ref": "c" }, { "state": "ok", "ref": "d" }
            ],
            "extensions": [{ "state": "ok", "ref": "nous" }],
            "template": { "state": "ok", "ref": "smart-ass" },
            "cwd": { "state": "auto", "value": null },
            "machine": { "state": "auto", "value": null },
        }));
        let ns = nodes(&p);
        let (w, h) = (420., 460.);
        let pts = layout(&ns, w, h);
        assert_eq!(pts.len(), ns.len());
        for q in &pts {
            assert!(q.x >= 0. && q.x <= w && q.y >= 0. && q.y <= h, "出界: {q:?}");
        }
        for i in 0..pts.len() {
            for j in (i + 1)..pts.len() {
                let d = ((pts[i].x - pts[j].x).powi(2) + (pts[i].y - pts[j].y).powi(2)).sqrt();
                // 三个以上兄弟画小圆点（直径 22），阈值按最挤的那种取。
                assert!(d > 20., "节点 {i}/{j}（{} / {}）挤在一起：{d}", ns[i].id, ns[j].id);
            }
        }
    }

    #[test]
    fn 多值维度写加号不挂子节点() {
        let p = preset(json!({
            "skills": [
                { "state": "ok", "ref": "a" }, { "state": "ok", "ref": "b" },
                { "state": "invalid", "ref": "c" }, { "state": "ok", "ref": "d" }
            ],
            "template": { "state": "ok", "ref": "smart-ass" },
        }));
        let ns = nodes(&p);
        // 单层：全部是 layer 0 或 1，没有第二圈。
        assert!(ns.iter().all(|n| n.layer <= 1));
        let sk = ns.iter().find(|n| n.id == "dim:skills").unwrap();
        assert_eq!(sk.value, "a +3");
        let tpl = ns.iter().find(|n| n.id == "dim:template").unwrap();
        assert_eq!(tpl.value, "smart-ass");
        assert!(tpl.is_ref);
        assert!(!ns.iter().find(|n| n.id == "dim:model").map_or(false, |n| n.is_ref));
    }

    #[test]
    fn 所有点在同一圈上() {
        let p = preset(json!({
            "model": { "state": "ok", "value": "volc-ark/x" },
            "thinking": { "state": "ok", "value": "medium" },
            "template": { "state": "ok", "ref": "smart-ass" },
        }));
        let ns = nodes(&p);
        let (w, h) = (640., 700.);
        let pts = layout(&ns, w, h);
        let (cx, cy) = (w / 2., h / 2. - 10.);
        let d0 = ((pts[1].x - cx).powi(2) + (pts[1].y - cy).powi(2)).sqrt();
        for q in pts.iter().skip(2) {
            let d = ((q.x - cx).powi(2) + (q.y - cy).powi(2)).sqrt();
            assert!((d - d0).abs() < 0.5, "不在同一圈上：{d} vs {d0}");
        }
        // 第一个点从 12 点开始（模型在最上面）。
        assert!(pts[1].y < cy && (pts[1].x - cx).abs() < 0.5);
    }
}
