//! 扫码登录：`_render_qr` / `login_web` / `login_tv`
//! （bilibili_cli.py:277 / 305 / 375），QR 渲染链与轮询语义逐字对齐。

use std::io::IsTerminal;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use sign::port as sign_port;

use super::creds::{TV_KEY, TV_SEC, load_creds, save_creds};
use super::curl::{UA_TV, UA_WEB};
use super::download::which;
use super::err::{CliError, Failure};
use super::http::{FetchSpec, fetch_json, nav_name};

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `_render_qr`（bilibili_cli.py:277）：真终端画 ASCII；非 TTY 弹 PNG。
fn render_qr(text: &str) -> bool {
    let qr = which("qrencode");
    let tty = std::io::stdout().is_terminal();
    if let Some(qr) = qr.as_deref()
        && tty
        && let Ok(out) = Command::new(qr)
            .args(["-t", "ANSIUTF8", "-m", "1"])
            .arg(text)
            .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() && !stdout.is_empty() {
            println!("{stdout}");
            return true;
        }
    }
    if which("uv").is_some() {
        let png = "/tmp/bilibili_cli_qr.png";
        let code = "import qrcode,sys;img=qrcode.make(sys.argv[1]);img.save(sys.argv[2])";
        if let Ok(out) = Command::new("uv")
            .args(["run", "--quiet", "--with", "qrcode[pil]", "python", "-c"])
            .arg(code)
            .arg(text)
            .arg(png)
            .output()
            && out.status.success()
            && std::path::Path::new(png).is_file()
        {
            println!("（非终端环境，已生成二维码图片并打开：{png}）");
            let _ = Command::new("open").arg(png).status();
            return true;
        }
    }
    if let Some(qr) = qr.as_deref()
        && let Ok(out) = Command::new(qr)
            .args(["-t", "UTF8", "-m", "1"])
            .arg(text)
            .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() && !stdout.is_empty() {
            println!("{stdout}");
            return true;
        }
    }
    false
}

/// `login_web`（bilibili_cli.py:305）。
pub fn login_web() -> Result<u32, Failure> {
    let gen_resp = fetch_json(
        "https://passport.bilibili.com/x/passport-login/web/qrcode/generate",
        &FetchSpec::new(),
    )?;
    let data = gen_resp.get("data").cloned().unwrap_or(json!({}));
    let url = data
        .get("url")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let key = data
        .get("qrcode_key")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if url.is_empty() || key.is_empty() {
        return Err(CliError::new("interface", "二维码接口返回结构异常")
            .raw(
                serde_json::to_string(&gen_resp)
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>(),
            )
            .into());
    }

    println!("请用「哔哩哔哩」App 扫码登录：\n");
    if !render_qr(&url) {
        println!("（无法渲染二维码，请手动在浏览器打开并在 App 内确认）\n{url}\n");
    }

    let deadline = Instant::now() + Duration::from_secs(180);
    let mut last = String::new();
    let jar = "/tmp/bilibili_cli_login.jar";
    let _ = std::fs::remove_file(jar);
    loop {
        if Instant::now() >= deadline {
            return Err(CliError::new("timeout", "等待扫码超时（180 秒）").into());
        }
        sleep(Duration::from_secs(2));
        // poll 必须在 cookie jar 下请求，Set-Cookie（SESSDATA）才会落到 jar 里
        let j = fetch_json(
            &format!(
                "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={key}"
            ),
            &FetchSpec::new().jar(jar),
        )?;
        let d = j.get("data").cloned().unwrap_or(json!({}));
        let code = d.get("code").and_then(|x| x.as_i64());
        let msg = d
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if !msg.is_empty() && msg != last {
            println!("  状态：{msg}");
            last = msg;
        }
        if code == Some(0) {
            let pairs = sign_port::_read_jar(jar.to_string()).map_err(Failure::Raw)?;
            let sess = pairs.get("SESSDATA").and_then(|x| x.as_str()).unwrap_or("");
            if sess.is_empty() {
                let raw = match pairs.as_object() {
                    Some(o) if !o.is_empty() => o.keys().cloned().collect::<Vec<_>>().join(", "),
                    _ => "jar 为空".to_string(),
                };
                return Err(CliError::new(
                    "auth",
                    "登录成功但没拿到 SESSDATA：cookie 上报链路异常",
                )
                .raw(raw)
                .into());
            }
            let cookie_str = pairs
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("")))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            let mut c = load_creds();
            if let Some(obj) = c.as_object_mut() {
                obj.insert("cookie".into(), json!(cookie_str));
                obj.insert("cookie_map".into(), pairs.clone());
                obj.insert("saved_at".into(), json!(now_ts()));
            }
            save_creds(&c)?;
            let who = nav_name(&cookie_str)?;
            let who_disp = if who.is_empty() {
                String::new()
            } else {
                format!("：{who}")
            };
            println!("\n✓ 网页登录成功{who_disp}");
            let n = pairs.as_object().map(|o| o.len()).unwrap_or(0);
            println!("  cookie 已存 ~/.bilibili_cli/creds.json（{n} 项，权限 600）");
            return Ok(0);
        }
        if code == Some(86038) {
            return Err(CliError::new("auth", "二维码已过期，请重新执行 login").into());
        }
    }
}

/// `login_tv`（bilibili_cli.py:375）。
pub fn login_tv() -> Result<u32, Failure> {
    let params = json!({"local_id": "0"});
    let q = sign_port::sign_app(params, Some(json!(TV_KEY)), Some(json!(TV_SEC)))
        .map_err(Failure::Raw)?;
    let gen_resp = fetch_json(
        "https://passport.bilibili.com/x/passport-tv-login/qrcode/auth_code",
        &FetchSpec::new().ua(UA_TV).referer(Some("")).post(Some(&q)),
    )?;
    let d = gen_resp.get("data").cloned().unwrap_or(json!({}));
    let url = d
        .get("url")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let auth_code = d
        .get("auth_code")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if url.is_empty() || auth_code.is_empty() {
        return Err(CliError::new("interface", "TV 二维码接口返回结构异常")
            .raw(
                serde_json::to_string(&gen_resp)
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>(),
            )
            .into());
    }
    println!("请用「哔哩哔哩」App 扫码，并在手机上确认「在电视上登录」：\n");
    if !render_qr(&url) {
        println!("{url}\n");
    }

    let deadline = Instant::now() + Duration::from_secs(180);
    let mut last_tv = String::new();
    loop {
        if Instant::now() >= deadline {
            return Err(CliError::new("timeout", "等待扫码超时（180 秒）").into());
        }
        sleep(Duration::from_secs(2));
        let p = json!({"auth_code": auth_code, "local_id": "0"});
        let body = sign_port::sign_app(p, Some(json!(TV_KEY)), Some(json!(TV_SEC)))
            .map_err(Failure::Raw)?;
        let j = fetch_json(
            "https://passport.bilibili.com/x/passport-tv-login/qrcode/poll",
            &FetchSpec::new()
                .ua(UA_TV)
                .referer(Some(""))
                .post(Some(&body))
                .allow_codes(&[86039, 86090, 86101, 86038]),
        )?;
        let td = j.get("data").cloned().unwrap_or(json!({}));
        let token = td
            .get("access_token")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if !token.is_empty() {
            let mut c = load_creds();
            let mid = td
                .pointer("/token_info/mid")
                .cloned()
                .unwrap_or(json!(null));
            if let Some(obj) = c.as_object_mut() {
                obj.insert("tv_access_token".into(), json!(token));
                obj.insert(
                    "tv_refresh_token".into(),
                    td.get("refresh_token").cloned().unwrap_or(json!(null)),
                );
                obj.insert("tv_mid".into(), mid);
                obj.insert("tv_saved_at".into(), json!(now_ts()));
            }
            save_creds(&c)?;
            println!("\n✓ TV 登录成功，access_token 已存 ~/.bilibili_cli/creds.json");
            println!("  已作为降级备用通道（首选仍是 app）");
            return Ok(0);
        }
        let state = j.get("code").and_then(|x| x.as_i64()).unwrap_or(0);
        let msg = j
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if !msg.is_empty() && msg != last_tv {
            println!("  状态：{msg}");
            last_tv = msg.clone();
        }
        if matches!(state, 0 | 86039 | 86090 | 86101) {
            continue;
        }
        if state == 86038 {
            return Err(CliError::new("auth", "TV 二维码已过期，请重新执行 login --tv").into());
        }
        let msg_disp = if msg.is_empty() {
            "未知".to_string()
        } else {
            msg.clone()
        };
        return Err(CliError::new("auth", format!("TV 登录失败（{state}）：{msg_disp}")).into());
    }
}

// UA_WEB 在登录链路未直接使用（generate/poll 走默认 UA），保留常量引用关系。
#[allow(dead_code)]
const _UA_WEB_KEEP: &str = UA_WEB;

// 空的 Map 引用占位（保持与源码 import 对应）。
#[allow(dead_code)]
fn _unused(_m: &Map<String, Value>) {}
