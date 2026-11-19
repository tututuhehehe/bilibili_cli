//! 登录态与清晰度常量：`load_creds`（bilibili_cli.py:250）、QN 表（:59）、
//! TV_KEY/TV_SEC（:55）。

use serde_json::{Value, json};

use super::err::Failure;

/// `CREDS_PATH`：`~/.bilibili_cli/creds.json`（模式 600，路径与字段不变，ADR-008）。
fn creds_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/.bilibili_cli/creds.json")
}

/// `load_creds`：文件缺失/坏 JSON → 空对象；`BILI_SESSDATA` 环境变量兜底。
pub fn load_creds() -> Value {
    let parsed: Option<Value> = std::fs::read_to_string(creds_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let mut c = match parsed {
        Some(v @ Value::Object(_)) => v,
        _ => json!({}),
    };
    if let Ok(env) = std::env::var("BILI_SESSDATA") {
        let has_cookie = c
            .get("cookie")
            .and_then(|x| x.as_str())
            .is_some_and(|s| !s.is_empty());
        if !env.is_empty()
            && !has_cookie
            && let Some(obj) = c.as_object_mut()
        {
            obj.insert("cookie".into(), json!(format!("SESSDATA={env}")));
        }
    }
    c
}

/// `ACCESS_TEXT`（bilibili_cli.py:498）。
pub fn access_text(state: &str) -> &'static str {
    match state {
        "yes" => "可播放",
        "limited" => "仅低清晰度（未登录或权益不足）",
        "no" => "无播放权限",
        _ => "未知",
    }
}

pub const TV_KEY: &str = "4409e2ce8ffd12b8";
pub const TV_SEC: &str = "59b43e04ad6965f34319062b478f83dd";

/// `QN`（bilibili_cli.py:59）：`QN.get(id, str(id))` 语义。
pub fn qn_label(id: i64) -> String {
    match id {
        127 => "8K".into(),
        126 => "杜比视界".into(),
        125 => "HDR".into(),
        120 => "4K".into(),
        116 => "1080P60".into(),
        112 => "1080P+".into(),
        80 => "1080P".into(),
        74 => "720P60".into(),
        64 => "720P".into(),
        32 => "480P".into(),
        16 => "360P".into(),
        other => other.to_string(),
    }
}

/// `save_creds`（bilibili_cli.py:262）：tmp 写入 → chmod 600 → 原子替换。
pub fn save_creds(c: &Value) -> Result<(), Failure> {
    let path = creds_path();
    let dir = std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    std::fs::create_dir_all(&dir).map_err(|e| Failure::Raw(format!("{e}")))?;
    let tmp = format!("{path}.tmp");
    let body = serde_json::to_string_pretty(c).unwrap_or_default();
    std::fs::write(&tmp, body).map_err(|e| Failure::Raw(format!("{e}")))?;
    #[allow(clippy::unnecessary_cast)]
    std::fs::set_permissions(
        &tmp,
        std::os::unix::fs::PermissionsExt::from_mode(0o600 as u32),
    )
    .map_err(|e| Failure::Raw(format!("{e}")))?;
    std::fs::rename(&tmp, &path).map_err(|e| Failure::Raw(format!("{e}")))?;
    Ok(())
}
