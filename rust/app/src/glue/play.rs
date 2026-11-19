//! `playurl`（bilibili_cli.py:562）：app / tv 双通道取流。

use serde_json::{Value, json};
use sign::port as sign_port;

use super::creds::{TV_KEY, TV_SEC};
use super::curl::{UA_APP, UA_TV};
use super::err::{CliError, Failure};
use super::http::{FetchSpec, fetch_json};

/// 双通道取流。返回 `j["data"]`（缺失时空对象）。
/// tv 需要 `tv_access_token`；app 用 cookie（登录态决定 1080P）。
/// `j.get("data") or {}`：缺失或 null 都归空对象。
fn non_null_data(j: &Value) -> Value {
    match j.get("data") {
        Some(v) if !v.is_null() => v.clone(),
        _ => json!({}),
    }
}

pub fn playurl(
    channel: &str,
    aid: i64,
    cid: i64,
    qn: i64,
    creds: &Value,
    bvid: &str,
) -> Result<Value, Failure> {
    let _ = bvid;
    let cookie = sign_port::cookie_header(creds.clone()).map_err(Failure::Raw)?;
    match channel {
        "tv" => {
            let token = creds
                .get("tv_access_token")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if token.is_empty() {
                return Err(
                    CliError::new("auth", "TV 通道需要 TV 登录态（先执行 login --tv）")
                        .channel("tv")
                        .into(),
                );
            }
            let params = json!({
                "access_key": token,
                "actionKey": "appkey",
                "build": "36700100",
                "cid": cid,
                "fourk": "1",
                "is_proj": "1",
                "mobile_access_key": token,
                "object_id": aid,
                "ogv_aid": "",
                "platform": "ios",
                "playurl_type": "1",
                "protocol": "0",
                "qn": qn,
                "fnval": "4048",
                "fnver": "0",
            });
            let q = sign_port::sign_app(params, Some(json!(TV_KEY)), Some(json!(TV_SEC)))
                .map_err(Failure::Raw)?;
            let j = fetch_json(
                &format!("https://api.bilibili.com/x/tv/playurl?{q}"),
                &FetchSpec::new().ua(UA_TV).referer(Some("")).channel("tv"),
            )?;
            Ok(non_null_data(&j))
        }
        "app" => {
            let mut params = json!({
                "avid": aid,
                "cid": cid,
                "qn": qn,
                "fnval": "4048",
                "fnver": "0",
                "fourk": "1",
                "mobi_app": "android",
                "platform": "android",
                "build": "7730200",
            });
            let access_key = creds
                .get("access_key")
                .and_then(|k| k.as_str())
                .unwrap_or("");
            if !access_key.is_empty()
                && let Some(obj) = params.as_object_mut()
            {
                obj.insert("access_key".into(), json!(access_key));
            }
            let q = sign_port::sign_app(params, None, None).map_err(Failure::Raw)?;
            let j = fetch_json(
                &format!("https://api.bilibili.com/x/player/playurl?{q}"),
                &FetchSpec::new()
                    .ua(UA_APP)
                    .referer(Some(""))
                    .cookie(&cookie)
                    .channel("app"),
            )?;
            Ok(non_null_data(&j))
        }
        other => Err(CliError::new("interface", format!("未知通道 {other}")).into()),
    }
}
