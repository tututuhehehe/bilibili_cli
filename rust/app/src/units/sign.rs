//! Glue from the front door to `sign-rs`：`list` 命令。
//! 逐字对齐 `cmd_list`（bilibili_cli.py:805）的输出与退出码。

use std::ffi::OsString;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use clap::ArgMatches;
use parse::port as parse_port;
use play::port as play_port;
use serde_json::{Map, Value, json};

use crate::glue::err::{Failure, SCHEMA, fail_exit};
use crate::glue::http::{user_name, user_videos, view};

pub fn run(command: &str, matches: &ArgMatches) -> ExitCode {
    if command == "login" {
        let res = if matches.get_flag("tv") {
            crate::glue::login::login_tv()
        } else {
            crate::glue::login::login_web()
        };
        return match res {
            Ok(code) => ExitCode::from(code as u8),
            Err(f) => fail_exit(f),
        };
    }
    let target = matches
        .get_one::<OsString>("target")
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let n = *matches.get_one::<i64>("limit").unwrap_or(&20);
    let as_json = matches.get_flag("json");
    let rights = matches.get_flag("rights");

    match cmd_list(&target, n, as_json, rights) {
        Ok(code) => ExitCode::from(code as u8),
        Err(f) => fail_exit(f),
    }
}

fn s_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn i64_of(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn cmd_list(target: &str, n: i64, as_json: bool, rights: bool) -> Result<u32, Failure> {
    let t = match parse_port::parse_target(target.to_string()) {
        Ok(v) => v,
        // 源码在此 die()（SystemExit，退出码 2）；Err 里就是完整 die 文案。
        Err(msg) => return Err(Failure::Die(msg)),
    };
    if s_of(&t, "type") != "user" {
        return Err(Failure::Die("list 需要 UP 主 uid 或空间链接".into()));
    }
    let uid = s_of(&t, "uid");

    let res = user_videos(&uid, n)?;
    let name = user_name(&uid);
    let mut items: Vec<Value> = Vec::new();
    for it in res
        .get("items")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default()
    {
        let bvid = s_of(&it, "bvid");
        let ctime = i64_of(&it, "ctime");
        let dur = i64_of(&it, "duration");
        let iso_s = parse_port::iso(ctime).map_err(Failure::Raw)?;
        let mut row = Map::new();
        row.insert("id".into(), json!(bvid));
        row.insert("title".into(), json!(s_of(&it, "title")));
        row.insert(
            "url".into(),
            json!(format!("https://www.bilibili.com/video/{bvid}")),
        );
        row.insert("published_at".into(), json!(iso_s));
        row.insert("published_date".into(), json!(&iso_s[..10]));
        row.insert("duration_sec".into(), json!(dur));
        row.insert(
            "duration_text".into(),
            json!(parse_port::fmt_dur(dur as f64).map_err(Failure::Raw)?),
        );
        row.insert("views".into(), json!(i64_of(&it, "play")));
        row.insert("cover".into(), json!(s_of(&it, "cover")));
        row.insert(
            "paid".into(),
            json!(
                it.get("is_ugcpay")
                    .is_some_and(|x| x.as_bool().unwrap_or(false))
            ),
        );
        let badges: Vec<Value> = it
            .get("badges")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|b| {
                b.get("text")
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| !t.is_empty())
            })
            .map(|b| json!(b.get("text").and_then(|t| t.as_str()).unwrap_or("")))
            .collect();
        row.insert("badges".into(), Value::Array(badges));

        if rights {
            // 精确判定要不要钱（每条多一次 view 调用）
            let ent = match view(&bvid)
                .and_then(|d| play_port::entitlement_of(d).map_err(Failure::Raw))
            {
                Ok(e) => {
                    let mut sub = Map::new();
                    for k in ["kind", "label", "vip_only", "charging_only", "paid"] {
                        sub.insert(k.into(), e.get(k).cloned().unwrap_or(json!(null)));
                    }
                    Value::Object(sub)
                }
                Err(Failure::Cli(e)) => json!({"kind": "unknown", "label": e.summary}),
                Err(other) => return Err(other),
            };
            row.insert("entitlement".into(), ent);
            sleep(Duration::from_millis(150));
        }
        items.push(Value::Object(row));
    }

    if as_json {
        let mut env = Map::new();
        env.insert("schema".into(), json!(SCHEMA));
        env.insert("uid".into(), json!(uid));
        let mut author = Map::new();
        author.insert("uid".into(), json!(uid));
        author.insert("name".into(), json!(name));
        env.insert("author".into(), Value::Object(author));
        env.insert(
            "total".into(),
            res.get("total").cloned().unwrap_or(json!(items.len())),
        );
        env.insert("items".into(), Value::Array(items));
        println!(
            "{}",
            serde_json::to_string_pretty(&Value::Object(env)).unwrap_or_default()
        );
        return Ok(0);
    }

    let display = if name.is_empty() {
        uid.clone()
    } else {
        name.clone()
    };
    let total = res
        .get("total")
        .and_then(|x| x.as_i64())
        .unwrap_or(items.len() as i64);
    println!(
        "{display}（uid {uid}）共 {total} 个投稿 · 最新 {} 条",
        items.len()
    );
    println!();
    for v in &items {
        let date = s_of(v, "published_date");
        let dur = s_of(v, "duration_text");
        let title44: String = s_of(v, "title").chars().take(44).collect();
        let tag = if let Some(ent) = v.get("entitlement") {
            if s_of(ent, "kind") != "free" {
                format!(" [{}]", s_of(ent, "label"))
            } else {
                String::new()
            }
        } else if v.get("paid").and_then(|x| x.as_bool()).unwrap_or(false) {
            " [付费]".to_string()
        } else {
            String::new()
        };
        println!("  {date}  {dur:>7}  {title44}{tag}");
        println!("     {}", s_of(v, "url"));
    }
    Ok(0)
}
