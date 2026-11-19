//! Glue from the front door to `proto-rs`：`search` 命令。
//! 逐字对齐 `cmd_search`（bilibili_cli.py:1409）的输出与退出码。

use std::ffi::OsString;
use std::process::ExitCode;

use clap::ArgMatches;
use serde_json::{Map, Value, json};

use crate::glue::err::{Failure, SCHEMA, fail_exit};
use crate::glue::grpc::grpc_search;

pub fn run(command: &str, matches: &ArgMatches) -> ExitCode {
    if command == "doctor" {
        return match cmd_doctor() {
            Ok(code) => ExitCode::from(code as u8),
            Err(f) => fail_exit(f),
        };
    }
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

fn cmd_doctor() -> Result<u32, Failure> {
    use crate::glue::creds::load_creds;
    use crate::glue::download::which;
    use crate::glue::err::report;
    use crate::glue::http::{nav_name, user_videos};

    let mut ok = true;
    println!("环境检查：");
    for name in ["curl", "ffmpeg", "qrencode", "uv"] {
        let mark = if which(name).is_some() { "✓" } else { "·" };
        println!("  {mark} {name}");
    }
    let ff = which("ffmpeg").is_some();
    println!("  {} ffmpeg（合并音视频必需）", if ff { "✓" } else { "✗" });
    ok = ok && ff;

    let c = load_creds();
    println!("登录态：");
    let cookie = c
        .get("cookie")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if !cookie.is_empty() {
        let who = nav_name(&cookie)?;
        let who_disp = if who.is_empty() {
            String::new()
        } else {
            format!("（{who}）")
        };
        let saved = c.get("saved_at").and_then(|x| x.as_i64()).unwrap_or(0);
        let saved_disp = if saved != 0 {
            parse::port::iso(saved).map_err(Failure::Raw)?[..10].to_string()
        } else {
            "未知".to_string()
        };
        println!("  ✓ 网页 cookie{who_disp} · 保存于 {saved_disp}");
    } else {
        println!("  · 未登录网页（只能拿 480P）");
    }
    if c
        .get("tv_access_token")
        .and_then(|x| x.as_str())
        .is_some_and(|t| !t.is_empty())
    {
        println!("  ✓ TV access_token（TV 通道可用）");
    } else {
        println!("  · 未登录 TV（TV 通道会跳过）");
    }

    match user_videos("23947287", 3) {
        Ok(v) => println!(
            "环境接口：✓ 投稿接口连通（样例 {} 条）",
            v.get("total").and_then(|x| x.as_i64()).unwrap_or(0)
        ),
        Err(Failure::Cli(e)) => {
            println!("环境接口：✗");
            report(&e);
            ok = false;
        }
        Err(other) => return Err(other),
    }

    let t0 = std::time::Instant::now();
    match grpc_search("露营", 1, "totalrank", false, false) {
        Ok(hits) => {
            let dt = t0.elapsed().as_secs_f64();
            if let Some(first) = hits.first() {
                let t24: String = first
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(24)
                    .collect();
                println!("搜索接口：✓ gRPC 搜索连通（{dt:.1}s，样例「{t24}」）");
            } else {
                println!("搜索接口：✗ gRPC 返回空（接口可能改版）");
                ok = false;
            }
        }
        Err(Failure::Cli(e)) => {
            println!("搜索接口：✗");
            report(&e);
            ok = false;
        }
        Err(other) => return Err(other),
    }
    Ok(if ok { 0 } else { 3 })
}

fn s_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
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
                    let like =
                        parse::port::_fmt_count(i64_of(&st, "like")).map_err(Failure::Raw)?;
                    let coin =
                        parse::port::_fmt_count(i64_of(&st, "coin")).map_err(Failure::Raw)?;
                    println!("    收藏 {favorite} · 点赞 {like} · 投币 {coin}");
                }
            }
        }
    }
    Ok(0)
}
