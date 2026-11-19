//! Glue from the front door to `proto-rs`：`search` 命令。
//! 逐字对齐 `cmd_search`（bilibili_cli.py:1409）的输出与退出码。

use std::ffi::OsString;
use std::process::ExitCode;

use clap::ArgMatches;
use serde_json::{json, Map, Value};

use crate::glue::err::{fail_exit, Failure, SCHEMA};
use crate::glue::grpc::grpc_search;

pub fn run(command: &str, matches: &ArgMatches) -> ExitCode {
    debug_assert_eq!(command, "search");
    let keyword = matches
        .get_one::<OsString>("keyword")
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let n = (*matches.get_one::<i64>("limit").unwrap_or(&20)).clamp(1, 100) as usize;
    let sort = matches
        .get_one::<String>("sort")
        .cloned()
        .unwrap_or_else(|| "totalrank".into());
    let users = matches.get_flag("users");
    let desc = matches.get_flag("desc");
    let as_json = matches.get_flag("json");

    match cmd_search(&keyword, n, &sort, users, desc, as_json) {
        Ok(code) => ExitCode::from(code as u8),
        Err(f) => fail_exit(f),
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

/// `SORT_LABEL`（bilibili_cli.py:1115）。
fn sort_label(sort: &str) -> &str {
    match sort {
        "totalrank" => "综合",
        "play" => "最多播放",
        "new" => "最新",
        "dm" => "最多弹幕",
        "stow" => "最多收藏",
        other => other,
    }
}

fn cmd_search(
    keyword: &str,
    n: usize,
    sort: &str,
    users: bool,
    desc: bool,
    as_json: bool,
) -> Result<u32, Failure> {
    let mut items = grpc_search(keyword, n, sort, users, desc)?;
    if items.is_empty() {
        if as_json {
            let mut env = Map::new();
            env.insert("schema".into(), json!(SCHEMA));
            env.insert("type".into(), json!("search"));
            env.insert("keyword".into(), json!(keyword));
            env.insert("count".into(), json!(0));
            env.insert("items".into(), json!([]));
            println!(
                "{}",
                serde_json::to_string_pretty(&Value::Object(env)).unwrap_or_default()
            );
        } else {
            eprintln!("没有结果：{keyword}");
        }
        return Ok(4);
    }

    if as_json {
        for one in items.iter_mut() {
            let pubdate = i64_of(one, "pubdate");
            if pubdate != 0 {
                let iso_s = parse::port::iso(pubdate).map_err(Failure::Raw)?;
                if let Some(obj) = one.as_object_mut() {
                    obj.insert("published_at".into(), json!(iso_s));
                    obj.insert("published_date".into(), json!(&iso_s[..10]));
                }
            }
        }
        let mut env = Map::new();
        env.insert("schema".into(), json!(SCHEMA));
        env.insert("type".into(), json!("search"));
        env.insert("keyword".into(), json!(keyword));
        env.insert("sort".into(), json!(sort));
        env.insert("users".into(), json!(users));
        env.insert("count".into(), json!(items.len()));
        env.insert("items".into(), Value::Array(items));
        println!(
            "{}",
            serde_json::to_string_pretty(&Value::Object(env)).unwrap_or_default()
        );
        return Ok(0);
    }

    let head = if users { "UP 主" } else { "视频" };
    let suffix = if users || sort == "totalrank" {
        String::new()
    } else {
        format!("（排序：{}）", sort_label(sort))
    };
    println!("「{keyword}」{head} {} 条{suffix}", items.len());

    for (idx, one) in items.iter().enumerate() {
        let i = idx + 1;
        if users {
            let name = s_of(one, "name");
            let mid = one.get("mid").cloned().unwrap_or(json!(0));
            println!("\n{i:2}. {name}   <mid={mid}>");
            let fans = parse::port::_fmt_count(i64_of(one, "fans")).map_err(Failure::Raw)?;
            let archives = i64_of(one, "archives");
            let level = i64_of(one, "level");
            println!("    粉丝 {fans} · 投稿 {archives} · Lv{level}");
            let official = s_of(one, "official");
            if !official.is_empty() {
                println!("    认证：{official}");
            }
            let sign = s_of(one, "sign");
            if !sign.is_empty() {
                let sign60: String = sign.chars().take(60).collect();
                println!("    签名：{sign60}");
            }
        } else {
            let title = s_of(one, "title");
            let bvid = s_of(one, "bvid");
            println!("\n{i:2}. {title}   <{bvid}>");
            let dur_text = s_of(one, "duration_text");
            let dur_sec = i64_of(one, "duration_sec");
            let mut dur = if dur_sec != 0 {
                parse::port::fmt_dur(dur_sec as f64).map_err(Failure::Raw)?
            } else {
                dur_text.clone()
            };
            if dur_text.contains("集") {
                dur = dur_text;
            }
            let author = s_of(one, "author");
            let play = parse::port::_fmt_count(i64_of(one, "play")).map_err(Failure::Raw)?;
            let pubdate = i64_of(one, "pubdate");
            let date = if pubdate != 0 {
                parse::port::iso(pubdate).map_err(Failure::Raw)?[..10].to_string()
            } else {
                "-".to_string()
            };
            println!("    {author} · {play}播放 · {dur} · {date}");
            if desc {
                let d = s_of(one, "desc");
                if !d.is_empty() {
                    let d120: String = d.chars().take(120).collect::<String>().replace('\n', " / ");
                    println!("    简介：{d120}");
                    let st = one.get("stats").cloned().unwrap_or(json!({}));
                    let favorite =
                        parse::port::_fmt_count(i64_of(&st, "favorite")).map_err(Failure::Raw)?;
                    let like = parse::port::_fmt_count(i64_of(&st, "like")).map_err(Failure::Raw)?;
                    let coin = parse::port::_fmt_count(i64_of(&st, "coin")).map_err(Failure::Raw)?;
                    println!("    收藏 {favorite} · 点赞 {like} · 投币 {coin}");
                }
            }
        }
    }
    Ok(0)
}
