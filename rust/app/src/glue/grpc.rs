//! gRPC 编排：`grpc_call` / `grpc_view` / `grpc_search`
//! （bilibili_cli.py:1217 / 1331 / 1352），走系统 curl，proto 单元负责编解码。

use std::fs;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use proto::{pb, port};
use serde_json::{Map, Value, json};

use super::err::{CliError, Failure};

pub const GRPC_HOST: &str = "https://grpc.biliapi.net";
pub const SVC_SEARCH: &str = "/bilibili.polymer.app.search.v1.Search/SearchAll";
pub const SVC_SEARCH_TYPE: &str = "/bilibili.polymer.app.search.v1.Search/SearchByType";
pub const SVC_VIEW: &str = "/bilibili.app.view.v1.View/View";
pub const UA_GRPC: &str = "grpc-java-okhttp/1.0";
pub const SEARCH_TYPE_USER: i64 = 2;

fn bytes_of(v: &Value) -> Result<Vec<u8>, String> {
    let arr = v
        .get("$bytes")
        .and_then(|b| b.as_array())
        .ok_or_else(|| format!("proto encode 返回了非 $bytes 标记：{v}"))?;
    arr.iter()
        .map(|x| {
            x.as_i64()
                .map(|n| n as u8)
                .ok_or_else(|| format!("$bytes 内非数值元素：{x}"))
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(
                // 字节级取值：str 按字节偏移切多字节字符会 panic，这里先 from_utf8
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            )
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn temp_paths(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir();
    (
        dir.join(format!("{tag}.req")),
        dir.join(format!("{tag}.out")),
    )
}

/// `grpc_call`（bilibili_cli.py:1217）：unary 请求，返回去帧后的 message 字节。
pub fn grpc_call(path: &str, payload: &[u8], timeout: u64) -> Result<Vec<u8>, Failure> {
    let frame = {
        let mut f = Vec::with_capacity(payload.len() + 5);
        f.push(0u8);
        f.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        f.extend_from_slice(payload);
        f
    };
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = format!("bcli_grpc_{}_{n}", std::process::id());
    let (req_path, out_path) = temp_paths(&tag);
    fs::write(&req_path, &frame).map_err(|e| Failure::Raw(format!("{e}")))?;

    let out = Command::new("curl")
        .args([
            "-sS",
            "--http2",
            "--max-time",
            &timeout.to_string(),
            "-X",
            "POST",
            "--data-binary",
            &format!("@{}", req_path.display()),
            "-H",
            "Content-Type: application/grpc",
            "-H",
            "TE: trailers",
            "-H",
            &format!("User-Agent: {UA_GRPC}"),
            "-D",
            "-",
            "-o",
            &format!("{}", out_path.display()),
        ])
        .arg(GRPC_HOST.to_string() + path)
        .output()
        .map_err(|e| {
            let _ = fs::remove_file(&req_path);
            let _ = fs::remove_file(&out_path);
            Failure::Raw(format!("curl 启动失败：{e}"))
        })?;
    let _ = fs::remove_file(&req_path);
    let body = fs::read(&out_path).unwrap_or_default();
    let _ = fs::remove_file(&out_path);

    let rc = out.status.code().unwrap_or(-1);
    if rc != 0 {
        let (cat, msg) = match super::err::curl_exit(rc) {
            Some((c, m)) => (c, m.to_string()),
            None => ("network", format!("curl 退出码 {rc}")),
        };
        return Err(CliError::new(cat, format!("gRPC 请求失败：{msg}"))
            .endpoint(path)
            .channel("app")
            .into());
    }

    let head = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut status = String::new();
    let mut message = String::new();
    for line in head.lines() {
        let low = line.to_ascii_lowercase();
        if low.starts_with("grpc-status:") {
            status = line
                .split_once(':')
                .map(|(_, v)| v.trim())
                .unwrap_or("")
                .into();
        } else if low.starts_with("grpc-message:") {
            let raw = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
            message = percent_decode(raw);
        }
    }
    if !status.is_empty() && status != "0" {
        let code: i64 = status.parse().map_err(|_| {
            Failure::Raw(format!("ValueError: invalid literal for int(): {status:?}"))
        })?;
        let cat = match code {
            3 | 12 => "interface",
            8 => "rate_limit",
            13 | 14 => "server",
            _ => "unknown",
        };
        return Err(CliError::new(
            cat,
            format!(
                "gRPC {code}：{}",
                if message.is_empty() {
                    "无说明".into()
                } else {
                    message
                }
            ),
        )
        .endpoint(path)
        .code(code)
        .channel("app")
        .into());
    }

    if body.len() < 5 {
        return Err(CliError::new("empty", "gRPC 返回空响应")
            .endpoint(path)
            .channel("app")
            .into());
    }
    let flag = body[0];
    let ln = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
    let data = body[5..(5 + ln).min(body.len())].to_vec();
    if flag != 0 {
        return Err(CliError::new("unknown", "gRPC 返回了压缩帧")
            .endpoint(path)
            .channel("app")
            .into());
    }
    Ok(data)
}

/// `grpc_view`（bilibili_cli.py:1331）：详情卡（简介 + 完整统计）。
pub fn grpc_view(bvid: &str) -> Result<Value, Failure> {
    let req = bytes_of(&port::_ps(2, bvid.to_string()).map_err(Failure::Raw)?)?;
    let top = pb::pread(&grpc_call(SVC_VIEW, &req, 20)?).map_err(Failure::Raw)?;
    let arc = pb::pmsg(&top, 1).map_err(Failure::Raw)?;
    let stat = pb::pmsg(&arc, 23).map_err(Failure::Raw)?;
    let mut stats = Map::new();
    stats.insert("play".into(), pb::int_value(pb::pint(&stat, 2, 0)));
    stats.insert("danmaku".into(), pb::int_value(pb::pint(&stat, 3, 0)));
    stats.insert("reply".into(), pb::int_value(pb::pint(&stat, 4, 0)));
    stats.insert("favorite".into(), pb::int_value(pb::pint(&stat, 5, 0)));
    stats.insert("coin".into(), pb::int_value(pb::pint(&stat, 6, 0)));
    stats.insert("share".into(), pb::int_value(pb::pint(&stat, 7, 0)));
    stats.insert("like".into(), pb::int_value(pb::pint(&stat, 10, 0)));
    let mut out = Map::new();
    out.insert(
        "desc".into(),
        json!(pb::ptext(&arc, 10, "").map_err(Failure::Raw)?),
    );
    out.insert("duration_sec".into(), pb::int_value(pb::pint(&arc, 16, 0)));
    out.insert("pubdate".into(), pb::int_value(pb::pint(&arc, 8, 0)));
    out.insert("stats".into(), Value::Object(stats));
    Ok(Value::Object(out))
}

/// `grpc_search`（bilibili_cli.py:1352）：SearchAll / SearchByType 分页抓取。
pub fn grpc_search(
    keyword: &str,
    want: usize,
    sort: &str,
    users: bool,
    with_desc: bool,
) -> Result<Vec<Value>, Failure> {
    let mut out: Vec<Value> = Vec::new();
    let mut nxt = String::new();
    let per: i64 = 20;
    let mut pages = 0;
    while out.len() < want && pages < 5 {
        pages += 1;
        let mut pag = bytes_of(&port::_pi(1, per).map_err(Failure::Raw)?)?;
        if !nxt.is_empty() {
            pag.extend_from_slice(&bytes_of(
                &port::_ps(2, nxt.clone()).map_err(Failure::Raw)?,
            )?);
        }
        let fields = if users {
            let mut req = bytes_of(&port::_pi(1, SEARCH_TYPE_USER).map_err(Failure::Raw)?)?;
            req.extend_from_slice(&bytes_of(
                &port::_ps(2, keyword.to_string()).map_err(Failure::Raw)?,
            )?);
            req.extend_from_slice(&bytes_of(
                &port::_pm(7, json!({ "$bytes": pag })).map_err(Failure::Raw)?,
            )?);
            pb::pread(&grpc_call(SVC_SEARCH_TYPE, &req, 20)?).map_err(Failure::Raw)?
        } else {
            let order = sort_app(sort);
            let mut req = bytes_of(&port::_ps(1, keyword.to_string()).map_err(Failure::Raw)?)?;
            req.extend_from_slice(&bytes_of(&port::_pi(2, order).map_err(Failure::Raw)?)?);
            req.extend_from_slice(&bytes_of(
                &port::_pm(10, json!({ "$bytes": pag })).map_err(Failure::Raw)?,
            )?);
            pb::pread(&grpc_call(SVC_SEARCH, &req, 20)?).map_err(Failure::Raw)?
        };
        let (cards, pag_field) = if users {
            (pb::prep(&fields, 6), 7)
        } else {
            (pb::prep(&fields, 4), 11)
        };

        let mut got = 0usize;
        for raw in &cards {
            let item_bytes = bytes_of(raw).map_err(Failure::Raw)?;
            let item = pb::pread(&item_bytes).map_err(Failure::Raw)?;
            let item_val = pb::fields_to_value(&item);
            let one = if users {
                port::_user_from_item(item_val).map_err(Failure::Raw)?
            } else {
                port::_video_from_item(item_val).map_err(Failure::Raw)?
            };
            if one.is_null() {
                continue;
            }
            out.push(one);
            got += 1;
        }
        nxt = pb::ptext(&pb::pmsg(&fields, pag_field).map_err(Failure::Raw)?, 1, "")
            .map_err(Failure::Raw)?;
        if nxt.is_empty() || got == 0 {
            break;
        }
        sleep(Duration::from_millis(300));
    }

    out.truncate(want);
    if with_desc && !users {
        for one in out.iter_mut() {
            let bvid = one
                .get("bvid")
                .and_then(|b| b.as_str())
                .unwrap_or("")
                .to_string();
            if bvid.is_empty() {
                continue;
            }
            match grpc_view(&bvid) {
                Ok(d) => {
                    if let Some(obj) = one.as_object_mut() {
                        obj.insert("desc".into(), d.get("desc").cloned().unwrap_or(json!("")));
                        obj.insert(
                            "stats".into(),
                            d.get("stats").cloned().unwrap_or(json!(null)),
                        );
                        let cur = obj
                            .get("duration_sec")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        if cur == 0
                            && let Some(dv) = d.get("duration_sec")
                        {
                            obj.insert("duration_sec".into(), dv.clone());
                        }
                    }
                }
                Err(_) => {
                    if let Some(obj) = one.as_object_mut() {
                        obj.insert("desc".into(), json!(""));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// `SORT_APP`（bilibili_cli.py:1117）。
fn sort_app(sort: &str) -> i64 {
    match sort {
        "totalrank" | "default" => 0,
        "play" | "click" => 1,
        "new" | "pubdate" => 2,
        "dm" | "danmaku" => 3,
        "stow" | "favorite" => 4,
        _ => 0,
    }
}
