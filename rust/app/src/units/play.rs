//! Glue from the front door to `play-rs`：`info` 命令。
//! 逐字对齐 `cmd_info`（bilibili_cli.py:854）的输出与退出码。
//! （dl 命令的胶水也在本文件，见 `dl` 分支。）

use std::ffi::OsString;
use std::process::ExitCode;

use clap::ArgMatches;
use parse::port as parse_port;
use play::port as play_port;
use serde_json::{json, Map, Value};

use crate::glue::creds::{load_creds, qn_label};
use crate::glue::err::{fail_exit, Failure, SCHEMA};
use crate::glue::http::view;
use crate::glue::play::playurl;

pub fn run(command: &str, matches: &ArgMatches) -> ExitCode {
    let target = matches
        .get_one::<OsString>("target")
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let as_json = matches.get_flag("json");
    match command {
        "info" => match cmd_info(&target, as_json) {
            Ok(code) => ExitCode::from(code as u8),
            Err(f) => fail_exit(f),
        },
        _ => ExitCode::from(2),
    }
}

fn s_of(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}
fn i64_of(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn cmd_info(target: &str, as_json: bool) -> Result<u32, Failure> {
    let t = match parse_port::parse_target(target.to_string()) {
        Ok(v) => v,
        Err(msg) => return Err(Failure::Die(msg)),
    };
    if s_of(&t, "type") != "video" {
        return Err(Failure::Die("info 需要视频链接或 BV 号".into()));
    }
    let bvid = s_of(&t, "bvid");
    let d = view(&bvid)?;
    let creds = load_creds();
    let aid = i64_of(&d, "aid");
    let cid = i64_of(&d, "cid");
    let pages = d.get("pages").and_then(|x| x.as_array()).cloned().unwrap_or_default();

    let mut probes: Vec<Value> = Vec::new();
    for ch in ["app", "tv"] {
        match playurl(ch, aid, cid, 127, &creds, &bvid) {
            Ok(pd) => {
                let dash = pd.get("dash").cloned().unwrap_or(json!({}));
                let vids = dash.get("video").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                let best = vids
                    .iter()
                    .map(|v| v.get("id").and_then(|x| x.as_i64()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                let available: Vec<Value> = pd
                    .get("accept_quality")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|q| json!(qn_label(q.as_i64().unwrap_or(0))))
                    .collect();
                let mut p = Map::new();
                p.insert("channel".into(), json!(ch));
                p.insert("ok".into(), json!(true));
                p.insert("max_quality".into(), json!(qn_label(best)));
                p.insert("quality_id".into(), json!(best));
                p.insert("tracks".into(), json!(vids.len()));
                p.insert(
                    "has_audio".into(),
                    json!(dash.get("audio").is_some_and(|a| !a.is_null())),
                );
                p.insert("available".into(), Value::Array(available));
                probes.push(Value::Object(p));
            }
            Err(Failure::Cli(e)) => {
                let mut p = Map::new();
                p.insert("channel".into(), json!(ch));
                p.insert("ok".into(), json!(false));
                p.insert("category".into(), json!(e.category));
                p.insert("reason".into(), json!(e.summary));
                probes.push(Value::Object(p));
            }
            Err(other) => return Err(other),
        }
    }

    let pay_flags = play_port::entitlement_of(d.clone()).map_err(Failure::Raw)?;
    let access = play_port::access_of(probes.clone(), Some(pay_flags.clone()))
        .map_err(Failure::Raw)?;

    let pubdate = i64_of(&d, "pubdate");
    let iso_s = parse_port::iso(pubdate).map_err(Failure::Raw)?;
    let dur = i64_of(&d, "duration");
    let owner = d.get("owner").cloned().unwrap_or(json!({}));
    let mut out = Map::new();
    out.insert("schema".into(), json!(SCHEMA));
    out.insert("id".into(), json!(bvid));
    out.insert("aid".into(), json!(aid));
    out.insert("cid".into(), json!(cid));
    out.insert("title".into(), d.get("title").cloned().unwrap_or(json!(null)));
    out.insert(
        "url".into(),
        json!(format!("https://www.bilibili.com/video/{bvid}")),
    );
    let mut author = Map::new();
    author.insert("uid".into(), owner.get("mid").cloned().unwrap_or(json!(null)));
    author.insert("name".into(), owner.get("name").cloned().unwrap_or(json!(null)));
    out.insert("author".into(), Value::Object(author));
    out.insert("published_at".into(), json!(iso_s));
    out.insert("published_date".into(), json!(&iso_s[..10]));
    out.insert("duration_sec".into(), json!(dur));
    out.insert(
        "duration_text".into(),
        json!(parse_port::fmt_dur(dur as f64).map_err(Failure::Raw)?),
    );
    let pages_arr: Vec<Value> = pages
        .iter()
        .map(|p| {
            json!({
                "page": p.get("page").cloned().unwrap_or(json!(null)),
                "cid": p.get("cid").cloned().unwrap_or(json!(null)),
                "title": p.get("part").cloned().unwrap_or(json!(null)),
                "duration": p.get("duration").cloned().unwrap_or(json!(null)),
            })
        })
        .collect();
    out.insert("pages".into(), Value::Array(pages_arr.clone()));
    let mut ent = pay_flags.as_object().cloned().unwrap_or_default();
    ent.insert("my_access".into(), access.clone());
    out.insert("entitlement".into(), Value::Object(ent));
    out.insert("channels".into(), Value::Array(probes.clone()));

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&Value::Object(out)).unwrap_or_default()
        );
        return Ok(0);
    }

    // 人类输出
    let title = out.get("title").and_then(|x| x.as_str()).unwrap_or("");
    let author_name = owner.get("name").and_then(|x| x.as_str()).unwrap_or("");
    let access_text = access.get("text").and_then(|x| x.as_str()).unwrap_or("");
    let reason = access.get("reason").and_then(|x| x.as_str()).unwrap_or("");
    let best_q = access.get("best_quality").and_then(|x| x.as_str()).unwrap_or("");
    println!("{title}");
    println!("  {author_name} · {} · {} · {bvid}", &iso_s[..10], out.get("duration_text").and_then(|x| x.as_str()).unwrap_or(""));
    let kind = pay_flags.get("kind").and_then(|x| x.as_str()).unwrap_or("free");
    if kind != "free" {
        println!("  🏷 内容类型：{}", pay_flags.get("label").and_then(|x| x.as_str()).unwrap_or(""));
    }
    let mut line = format!("  🔑 当前账号：{access_text}");
    if !reason.is_empty() {
        line.push_str(&format!(" · {reason}"));
    }
    if !best_q.is_empty() {
        line.push_str(&format!(" · 最高 {best_q}"));
    }
    println!("{line}");
    if pages.len() > 1 {
        println!("  分P：{} 个", pages.len());
        for p in pages.iter().take(10) {
            let page = p.get("page").and_then(|x| x.as_i64()).unwrap_or(0);
            let part = p.get("part").and_then(|x| x.as_str()).unwrap_or("");
            let part40: String = part.chars().take(40).collect();
            let pdur = parse_port::fmt_dur(p.get("duration").and_then(|x| x.as_i64()).unwrap_or(0) as f64)
                .map_err(Failure::Raw)?;
            println!("    P{page}  {part40}  {pdur}");
        }
    }
    println!("  通道探测：");
    for p in &probes {
        let ch = p.get("channel").and_then(|x| x.as_str()).unwrap_or("");
        if p.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
            let tracks = p.get("tracks").and_then(|x| x.as_i64()).unwrap_or(0);
            if tracks > 0 {
                println!(
                    "    ✓ {:<4} 最高 {} · {} 条视频轨 · 音轨{}",
                    ch,
                    p.get("max_quality").and_then(|x| x.as_str()).unwrap_or(""),
                    tracks,
                    if p.get("has_audio").and_then(|x| x.as_bool()).unwrap_or(false) { "有" } else { "无" }
                );
            } else {
                println!("    ✓ {ch:<4} 接口通，但未返回可用视频流（权益/风控限制）");
            }
        } else {
            println!(
                "    ✗ {ch:<4} [{}] {}",
                p.get("category").and_then(|x| x.as_str()).unwrap_or(""),
                p.get("reason").and_then(|x| x.as_str()).unwrap_or("")
            );
        }
    }
    Ok(0)
}
