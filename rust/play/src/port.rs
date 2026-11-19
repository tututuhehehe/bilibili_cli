//! play 模块移植：权益判定 + 通道可播性 + 流挑选。
//! 实现前必读 rust/tasks/00-shared-rules.md 与 rust/tasks/play.md。
#![forbid(unsafe_code)]

use serde_json::{Value, json};

// ------------------------------------------------------------------ 原版模块级常量（bilibili_cli.py:59-65、499-501）

/// QN 档位表；仅做 `QN.get(best, str(best))` 查找，与顺序无关。
const QN: &[(i64, &str)] = &[
    (127, "8K"),
    (126, "杜比视界"),
    (125, "HDR"),
    (120, "4K"),
    (116, "1080P60"),
    (112, "1080P+"),
    (80, "1080P"),
    (74, "720P60"),
    (64, "720P"),
    (32, "480P"),
    (16, "360P"),
];

/// `ACCESS_TEXT`（bilibili_cli.py:499-501）：state → 文案。
fn access_text(state: &str) -> &'static str {
    match state {
        "yes" => "可播放",
        "limited" => "仅低清晰度（未登录或权益不足）",
        "no" => "无播放权限",
        _ => "未知",
    }
}

// ------------------------------------------------------------------ JSON 域的 Python 语义小工具

/// `dict.get(key)`；非 dict（真实域不会出现）一律按缺键处理。
fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_object().and_then(|o| o.get(key))
}

/// Python 真值：None/False/0/""/[]/{} 为假，其余为真（NaN 为真）。
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `truthy(d.get(key))`。
fn truthy_at(v: &Value, key: &str) -> bool {
    get(v, key).is_some_and(truthy)
}

/// Python `x or 0` 的数值面：缺失/假值为 0，数字取其值。
///
/// 真值非数字时原版会在比较处抛 TypeError（如 `'x' > 0`）；view/playurl 响应里
/// 这些字段只会是 int 或缺失，按 0 处理（见汇报「已知差异」）。
fn num_or_zero(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Python `==` 的数值面：数字跨整型/浮点相等（7.0 == 7 为真），其余为假。
fn num_eq(v: Option<&Value>, n: f64) -> bool {
    matches!(v, Some(Value::Number(x)) if x.as_f64() == Some(n))
}

/// Python `v == "字串"`：仅同串相等（`None == "free"` 为假）。
fn str_eq(v: Option<&Value>, s: &str) -> bool {
    matches!(v, Some(Value::String(x)) if x == s)
}

/// Python 类型名，拼接 TypeError 文案用。
fn py_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// Python `str(float)` 的近似：整数值浮点带 `.0`（`str(80.0)` == `"80.0"`），
/// 其余同 Rust Display。该域没有录制用例（best_quality/label 只出现 int/str）。
fn py_str_float(f: f64) -> String {
    if f == f.trunc() && f.is_finite() && f.abs() <= 9.007_199_254_740_992e15 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

/// f-string 的 `str(v)`：覆盖 JSON 标量域；list/dict 走原版 repr，真实域不出现。
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => py_str_float(n.as_f64().unwrap_or(f64::NAN)),
        },
        _ => serde_json::to_string(v).unwrap_or_default(),
    }
}

/// Python `QN.get(best, str(best))`：整数值（含 80.0）按档位表取名，否则按 Python str()。
fn qn_or_str(best: f64) -> String {
    if best == best.trunc() && best.is_finite() && best.abs() <= 9.007_199_254_740_992e15 {
        let i = best as i64;
        match QN.iter().find(|(q, _)| *q == i) {
            Some((_, label)) => label.to_string(),
            None => i.to_string(),
        }
    } else {
        py_str_float(best)
    }
}

// ------------------------------------------------------------------ playurl 流挑选的内部工具

/// `dash[key]` 过滤出带主 URL 的流（`baseUrl`/`base_url` 双写法都要读）。
fn streams<'a>(dash: &'a Value, key: &str) -> Vec<&'a Value> {
    match get(dash, key) {
        Some(Value::Array(list)) => list
            .iter()
            .filter(|s| truthy_at(s, "baseUrl") || truthy_at(s, "base_url"))
            .collect(),
        _ => Vec::new(),
    }
}

/// Python `max(pool, key=(id, height, bandwidth))`：三元组取最大，并列取先出现者。
fn pick_best<'a>(pool: &[&'a Value]) -> Option<&'a Value> {
    let mut best: Option<&'a Value> = None;
    let mut best_key = (f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for v in pool {
        let key = (
            num_or_zero(get(v, "id")),
            num_or_zero(get(v, "height")),
            num_or_zero(get(v, "bandwidth")),
        );
        if best.is_none() || key > best_key {
            best = Some(v);
            best_key = key;
        }
    }
    best
}

/// Python `max(audios, key=bandwidth)`：并列取先出现者。
fn pick_audio<'a>(audios: &[&'a Value]) -> Option<&'a Value> {
    let mut best: Option<&'a Value> = None;
    let mut best_bw = f64::NEG_INFINITY;
    for a in audios {
        let bw = num_or_zero(get(a, "bandwidth"));
        if best.is_none() || bw > best_bw {
            best = Some(a);
            best_bw = bw;
        }
    }
    best
}

// ------------------------------------------------------------------ 移植实现

/// Port of `entitlement_of` (`bilibili_cli.py:502`). view 数据 → 免费/会员/充电/付费判定。
pub fn entitlement_of(d: serde_json::Value) -> Result<serde_json::Value, String> {
    // rights = d.get("rights") or {}
    let empty = json!({});
    let rights = match get(&d, "rights") {
        Some(r) if truthy(r) => r,
        _ => &empty,
    };
    let upower = truthy_at(&d, "is_upower_exclusive");
    let charging = upower || truthy_at(rights, "elec");
    let vip_only = ["pay", "movie", "arc_pay"]
        .iter()
        .any(|k| truthy_at(rights, k));
    let paid = truthy_at(rights, "ugc_pay");
    // 优先级：charging > paid > vip > free
    let (kind, label) = if charging {
        ("charging", "充电专属")
    } else if paid {
        ("paid", "付费内容")
    } else if vip_only {
        ("vip", "会员专享")
    } else {
        ("free", "免费")
    };
    // flags 恒定五键，缺失取 None（JSON null）；值原样透传（0 保持 0，不转 bool）
    let flag = |k: &str| get(rights, k).cloned().unwrap_or(Value::Null);
    Ok(json!({
        "kind": kind,
        "label": label,
        "vip_only": vip_only,
        "charging_only": charging,
        "paid": paid,
        "flags": {
            "pay": flag("pay"),
            "ugc_pay": flag("ugc_pay"),
            "arc_pay": flag("arc_pay"),
            "elec": flag("elec"),
            "movie": flag("movie"),
        },
        "upower_exclusive": upower,
    }))
}

/// Port of `access_of` (`bilibili_cli.py:524`).
///
/// `ent` 在源码中默认 `None`。
pub fn access_of(
    probes: Vec<serde_json::Value>,
    ent: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    // ok = [p for p in probes if p.get("ok")]；live = [p for p in ok if (p.get("tracks") or 0) > 0]
    let ok: Vec<&Value> = probes.iter().filter(|p| truthy_at(p, "ok")).collect();
    let live: Vec<&Value> = ok
        .iter()
        .copied()
        .filter(|p| num_or_zero(get(p, "tracks")) > 0.0)
        .collect();
    if !live.is_empty() {
        let mut best = f64::NEG_INFINITY;
        for p in &live {
            best = best.max(num_or_zero(get(p, "quality_id")));
        }
        let state = if best >= 64.0 { "yes" } else { "limited" };
        // via = "/".join(p["channel"] for p in live)：缺键按原版 KeyError
        let mut via = String::new();
        for (i, p) in live.iter().enumerate() {
            if i > 0 {
                via.push('/');
            }
            let channel = get(p, "channel").ok_or_else(|| "KeyError: 'channel'".to_string())?;
            via.push_str(channel.as_str().ok_or_else(|| {
                format!(
                    "TypeError: sequence item {i}: expected str instance, {} found",
                    py_type_name(channel)
                )
            })?);
        }
        return Ok(json!({
            "state": state,
            "text": access_text(state),
            "best_quality": qn_or_str(best),
            "via": via,
        }));
    }
    if !ok.is_empty() {
        // if ent and ent.get("kind") != "free"：ent 为空 dict 时同样跳过
        let blocked = ent
            .as_ref()
            .filter(|e| truthy(e))
            .is_some_and(|e| !str_eq(get(e, "kind"), "free"));
        if blocked {
            // ent['label']：缺键按原版 KeyError
            let label = ent
                .as_ref()
                .and_then(|e| get(e, "label"))
                .ok_or_else(|| "KeyError: 'label'".to_string())?;
            return Ok(json!({
                "state": "no",
                "text": access_text("no"),
                "reason": format!("{}：通道未返回可用的视频流", py_str(label)),
            }));
        }
        return Ok(json!({"state": "unknown", "text": "通道未返回可用的视频流"}));
    }
    let fails: Vec<&Value> = probes.iter().filter(|p| !truthy_at(p, "ok")).collect();
    // rights_fail = [p for p in fails if p.get("category") == "rights"]
    if let Some(p) = fails.iter().find(|p| str_eq(get(p, "category"), "rights")) {
        return Ok(json!({
            "state": "no",
            "text": access_text("no"),
            "reason": get(p, "reason").cloned().unwrap_or(Value::Null),
        }));
    }
    if !fails.is_empty() && fails.iter().all(|p| str_eq(get(p, "category"), "auth")) {
        return Ok(json!({
            "state": "unknown",
            "text": "需要登录后才能判断",
            "reason": get(fails[0], "reason").cloned().unwrap_or(Value::Null),
        }));
    }
    // reason = fails[0].get("reason") if fails else ""
    Ok(json!({
        "state": "unknown",
        "text": access_text("unknown"),
        "reason": fails
            .first()
            .and_then(|p| get(p, "reason"))
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    }))
}

/// Port of `pick_streams` (`bilibili_cli.py:590`).
///
/// `codec` 在源码中默认 `"avc"`。无视频流时源码抛 `CliError("rights", ...)`。
pub fn pick_streams(
    dash: serde_json::Value,
    want_qn: i64,
    codec: String,
) -> Result<serde_json::Value, String> {
    let videos = streams(&dash, "video");
    let audios = streams(&dash, "audio");
    if videos.is_empty() {
        // 原版抛 CliError("rights", ...)；Err 文案与 cases.jsonl 的 thrown 条目逐字一致
        return Err("CliError: playurl 没有返回任何视频流：通常是权益或风控限制".to_string());
    }
    // allowed = [v for v in videos if (v.get("id") or 0) <= want_qn] or videos
    let mut allowed: Vec<&Value> = videos
        .iter()
        .copied()
        .filter(|v| num_or_zero(get(v, "id")) <= want_qn as f64)
        .collect();
    if allowed.is_empty() {
        allowed = videos.clone();
    }
    // pref = CODECS.get(codec, 7)；same_codec = [v for v in allowed if v.get("codecid") == pref]
    let pref = match codec.as_str() {
        "avc" => 7.0,
        "hevc" => 12.0,
        "av1" => 13.0,
        _ => 7.0,
    };
    let same_codec: Vec<&Value> = allowed
        .iter()
        .copied()
        .filter(|v| num_eq(get(v, "codecid"), pref))
        .collect();
    let pool: &[&Value] = if same_codec.is_empty() {
        &allowed
    } else {
        &same_codec
    };
    // best = max(pool, key=(id, height, bandwidth))；pool 从非空 videos 派生，
    // None 分支实际不可达，语义上即原版的「无视频流」报错
    let best = pick_best(pool)
        .cloned()
        .ok_or_else(|| "CliError: playurl 没有返回任何视频流：通常是权益或风控限制".to_string())?;
    // audio = max(audios, key=bandwidth) if audios else {}
    let audio = pick_audio(&audios).cloned().unwrap_or_else(|| json!({}));
    Ok(json!({"$tuple": [best, audio]}))
}

/// Port of `_url_of` (`bilibili_cli.py:605`). 流 dict → 主 URL（baseUrl/base_url）。
pub fn _url_of(s: serde_json::Value) -> Result<String, String> {
    // s.get("baseUrl") or s.get("base_url") or ""
    for key in ["baseUrl", "base_url"] {
        if let Some(v) = get(&s, key).filter(|v| truthy(v)) {
            return v
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("TypeError: expected str, got {}", py_type_name(v)));
        }
    }
    Ok(String::new())
}

/// Port of `_backup_of` (`bilibili_cli.py:609`). 流 dict → 备用 URL 列表。
pub fn _backup_of(s: serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    // s.get("backupUrl") or s.get("backup_url") or []
    for key in ["backupUrl", "backup_url"] {
        if let Some(v) = get(&s, key).filter(|v| truthy(v)) {
            return v
                .as_array()
                .cloned()
                .ok_or_else(|| format!("TypeError: expected list, got {}", py_type_name(v)));
        }
    }
    Ok(Vec::new())
}
