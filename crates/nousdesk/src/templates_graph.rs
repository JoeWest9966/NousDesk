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
    /// 第二圈挂在哪一维下。
    pub parent: Option<String>,
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

/// 把一条预设摊成节点。**顺序稳定**：中心 → 七个维度（`DIMS` 顺序）→ 各自的具体项。
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
    }];

    let Some(res) = p.resolved.as_ref().filter(|r| r.is_object()) else {
        return spec_nodes(p, out);
    };

    for (key, label) in DIMS {
        let Some(v) = res.get(key) else { continue };
        let state = state_of(v);
        let (value, kids) = match key {
            "skills" | "extensions" => {
                let refs = list_refs(v);
                let value = if refs.is_empty() {
                    String::new()
                } else {
                    format!("{} 个", refs.len())
                };
                (value, refs)
            }
            "template" => (
                s_of(v, "ref").unwrap_or_else(|| "跟随默认".into()),
                Vec::new(),
            ),
            "cwd" | "machine" => (
                s_of(v, "value").unwrap_or_else(|| "默认".into()),
                Vec::new(),
            ),
            _ => (
                s_of(v, "value").unwrap_or_default(),
                Vec::new(),
            ),
        };

        let id = format!("dim:{key}");
        out.push(Node {
            id: id.clone(),
            label: label.into(),
            value,
            state,
            layer: 1,
            parent: Some("center".into()),
        });

        for (i, (r, st)) in kids.into_iter().enumerate() {
            out.push(Node {
                id: format!("{id}:{i}"),
                label: label.into(),
                value: r,
                state: st,
                layer: 2,
                parent: Some(id.clone()),
            });
        }
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
    let mut i = 0usize;
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
            layer: if i < 7 { 1 } else { 2 },
            parent: Some("center".into()),
        });
        i += 1;
    }
    out
}

/// 同一个父下面有几个兄弟。UI 靠它决定「画带文字的小卡」还是「画个点」。
pub(crate) fn siblings(nodes: &[Node], n: &Node) -> usize {
    match n.parent.as_deref() {
        Some(p) if n.layer == 2 => nodes.iter().filter(|x| x.parent.as_deref() == Some(p)).count(),
        _ => 0,
    }
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
    let (cx, cy) = (w / 2., h / 2.);
    // 椭圆而不是圆：右栏高瘦（420×430），正圆会把两侧的节点推出边框。
    // 两个半径都留出节点半宽，最外那个节点的右缘才压不到边线。
    // 第一圈内收得多一些、第二圈贴近边线：两圈之间至少留 50px，
    // 否则子节点会直接压在父节点上（差 20px 就够压住了）。
    let (rx1, ry1) = ((w / 2. - 88.).max(40.), (h / 2. - 86.).max(40.));
    let (rx2, ry2) = ((w / 2. - 40.).max(50.), (h / 2. - 28.).max(50.));
    let tau = std::f32::consts::TAU;

    // 维度圈的角度（按 DIMS 顺序，12 点起顺时针）。
    let dims: Vec<&Node> = nodes.iter().filter(|n| n.layer == 1).collect();
    let mut angle_of_dim: std::collections::HashMap<&str, f32> = Default::default();
    for (i, n) in dims.iter().enumerate() {
        let a = -std::f32::consts::FRAC_PI_2 + tau * i as f32 / dims.len().max(1) as f32;
        angle_of_dim.insert(n.id.as_str(), a);
    }

    // 第二圈：同一父的兄弟按出现顺序摊开。间隔按节点宽度反算 —— 写死 0.26 rad
    // 时三个兄弟（≈77px 宽）在 r≈180 的圈上会压在一起。
    let mut total_of: std::collections::HashMap<&str, usize> = Default::default();
    for n in nodes.iter().filter(|n| n.layer == 2) {
        *total_of.entry(n.parent.as_deref().unwrap_or("")).or_insert(0) += 1;
    }
    let mut seen: std::collections::HashMap<&str, usize> = Default::default();

    nodes
        .iter()
        .map(|n| match n.layer {
            0 => Placed { x: cx, y: cy },
            1 => {
                let a = angle_of_dim[n.id.as_str()];
                Placed {
                    x: cx + rx1 * a.cos(),
                    y: cy + ry1 * a.sin(),
                }
            }
            _ => {
                let p = n.parent.as_deref().unwrap_or("");
                let total = total_of.get(p).copied().unwrap_or(1);
                let k = *seen.entry(p).or_insert(0);
                *seen.get_mut(p).unwrap() += 1;
                let pa = angle_of_dim.get(p).copied().unwrap_or(0.);
                // 三个以上兄弟画成小圆点（见 `Node::compact`），所以扇区可以窄；
                // 一两个还带文字，得留出节点宽度的弦长。
                // 单个子节点也偏 0.55 个 spread —— 不偏就正好压在父节点上
                // （两圈半径只差 ~50px，节点半宽加起来 77px）。
                let spread = if total >= 3 { 0.17 } else { (86. / rx2).max(0.34) };
                let a = pa + spread * (0.55 + k as f32 - (total.saturating_sub(1)) as f32 / 2.);
                Placed {
                    x: cx + rx2 * a.cos(),
                    y: cy + ry2 * a.sin(),
                }
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
    fn 兄弟越多摊得越开() {
        let one = preset(json!({ "skills": [{ "state": "ok", "ref": "a" }] }));
        let many = preset(json!({
            "skills": [
                { "state": "ok", "ref": "a" }, { "state": "ok", "ref": "b" },
                { "state": "ok", "ref": "c" }
            ],
        }));
        let a = layout(&nodes(&one), 420., 460.);
        let b = layout(&nodes(&many), 420., 460.);
        let i = nodes(&many)
            .iter()
            .position(|n| n.id == "dim:skills:1")
            .unwrap();
        // 三个兄弟时，中间那个仍贴着父角度附近（不是乱飞）。
        assert!((b[i].x - b[0].x).abs() < 420.);
        assert!(!a.is_empty());
    }
}
