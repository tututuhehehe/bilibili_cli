//! Glue from the front door to `play-rs`：`info` 命令。
//! 逐字对齐 `cmd_info`（bilibili_cli.py:854）的输出与退出码。
//! （dl 命令的胶水也在本文件，见 `dl` 分支。）

use std::ffi::OsString;
use std::process::ExitCode;

use clap::ArgMatches;
use parse::port as parse_port;
use play::port as play_port;
use serde_json::{Map, Value, json};

use crate::glue::creds::{access_text, load_creds, qn_label};
use crate::glue::download::{download_track, merge, save_subs};
use crate::glue::err::{Failure, SCHEMA, fail_exit};
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
        "dl" => {
            let quality = matches
                .get_one::<OsString>("quality")
                .map(|s| s.to_string_lossy().into_owned());
            let codec = matches
                .get_one::<String>("codec")
                .cloned()
                .unwrap_or_else(|| "avc".into());
            let parallel = *matches.get_one::<i64>("parallel").unwrap_or(&4);
            let cover = matches.get_flag("cover");
            let out_dir = matches
                .get_one::<OsString>("out")
                .map(|s| s.to_string_lossy().into_owned());
            match cmd_dl(
                &target,
                matches.get_flag("audio"),
                quality.as_deref(),
                &codec,
                out_dir.as_deref(),
                matches.get_flag("subs"),
                matches.get_flag("keep_parts"),
                as_json,
                parallel,
                cover,
            ) {
                Ok(code) => ExitCode::from(code as u8),
                Err(f) => fail_exit(f),
            }
        }
        _ => ExitCode::from(2),
    }
}

fn s_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
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
    let pages = d
        .get("pages")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut probes: Vec<Value> = Vec::new();
    for ch in ["app", "tv"] {
        match playurl(ch, aid, cid, 127, &creds, &bvid) {
            Ok(pd) => {
                let dash = pd.get("dash").cloned().unwrap_or(json!({}));
                let vids = dash
                    .get("video")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .unwrap_or_default();
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
    let access =
        play_port::access_of(probes.clone(), Some(pay_flags.clone())).map_err(Failure::Raw)?;

    let pubdate = i64_of(&d, "pubdate");
    let iso_s = parse_port::iso(pubdate).map_err(Failure::Raw)?;
    let dur = i64_of(&d, "duration");
    let owner = d.get("owner").cloned().unwrap_or(json!({}));
    let mut out = Map::new();
    out.insert("schema".into(), json!(SCHEMA));
    out.insert("id".into(), json!(bvid));
    out.insert("aid".into(), json!(aid));
    out.insert("cid".into(), json!(cid));
    out.insert(
        "title".into(),
        d.get("title").cloned().unwrap_or(json!(null)),
    );
    out.insert(
        "url".into(),
        json!(format!("https://www.bilibili.com/video/{bvid}")),
    );
    let mut author = Map::new();
    author.insert(
        "uid".into(),
        owner.get("mid").cloned().unwrap_or(json!(null)),
    );
    author.insert(
        "name".into(),
        owner.get("name").cloned().unwrap_or(json!(null)),
    );
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
    let best_q = access
        .get("best_quality")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    println!("{title}");
    println!(
        "  {author_name} · {} · {} · {bvid}",
        &iso_s[..10],
        out.get("duration_text")
            .and_then(|x| x.as_str())
            .unwrap_or("")
    );
    let kind = pay_flags
        .get("kind")
        .and_then(|x| x.as_str())
        .unwrap_or("free");
    if kind != "free" {
        println!(
            "  🏷 内容类型：{}",
            pay_flags
                .get("label")
                .and_then(|x| x.as_str())
                .unwrap_or("")
        );
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
            let pdur =
                parse_port::fmt_dur(p.get("duration").and_then(|x| x.as_i64()).unwrap_or(0) as f64)
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
                    if p.get("has_audio")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(false)
                    {
                        "有"
                    } else {
                        "无"
                    }
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

// ------------------------------------------------------------------ dl

use crate::glue::err::{CliError, report};

fn default_out_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/Documents/bilibili_content")
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// play 单元的 `Err("CliError: ...")` → rights 类 CliError（原版唯一抛点）。
fn port_cli_err(msg: String) -> Failure {
    match msg.strip_prefix("CliError: ") {
        Some(summary) => Failure::Cli(CliError::new("rights", summary.to_string())),
        None => Failure::Raw(msg),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_dl(
    target: &str,
    audio_only: bool,
    quality: Option<&str>,
    codec: &str,
    out_dir: Option<&str>,
    subs: bool,
    keep_parts: bool,
    as_json: bool,
    parallel: i64,
    cover: bool,
) -> Result<u32, Failure> {
    let t = match parse_port::parse_target(target.to_string()) {
        Ok(v) => v,
        Err(msg) => return Err(Failure::Die(msg)),
    };
    if s_of(&t, "type") != "video" {
        return Err(Failure::Die(
            "dl 需要视频链接或 BV 号（列投稿请用 list）".into(),
        ));
    }
    let bvid = s_of(&t, "bvid");
    let d = view(&bvid)?;
    let aid = i64_of(&d, "aid");
    let cid = i64_of(&d, "cid");
    let title_raw = d.get("title").and_then(|x| x.as_str()).unwrap_or("");
    let title = if title_raw.is_empty() {
        bvid.clone()
    } else {
        title_raw.to_string()
    };
    let mut pub_ = i64_of(&d, "pubdate");
    if pub_ == 0 {
        pub_ = now_ts();
    }
    let owner_obj = d.get("owner").cloned().unwrap_or(json!({}));
    let owner_name = owner_obj
        .get("name")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let want_qn = parse_port::_qn_value(quality.map(|s| s.to_string())).map_err(Failure::Raw)?;
    let creds = load_creds();

    let ent0 = play_port::entitlement_of(d.clone()).map_err(Failure::Raw)?;
    let mut data: Option<Value> = None;
    let mut used = "";
    let mut failures: Vec<(String, CliError)> = Vec::new();
    for ch in ["app", "tv"] {
        match playurl(ch, aid, cid, want_qn, &creds, &bvid) {
            Ok(pd) => {
                data = Some(pd);
                used = ch;
                break;
            }
            Err(Failure::Cli(e)) => failures.push((ch.to_string(), e)),
            Err(other) => return Err(other),
        }
    }
    let Some(data) = data else {
        eprintln!(
            "\n🏷 内容类型：{}",
            ent0.get("label").and_then(|x| x.as_str()).unwrap_or("")
        );
        eprintln!("TV 与 APP 两个通道都失败：");
        for (_, e) in &failures {
            report(e);
        }
        let any_rights = failures.iter().any(|(_, e)| e.category == "rights");
        return Ok(if any_rights { 7 } else { 3 });
    };

    let dash = data.get("dash").cloned().unwrap_or_else(|| json!({}));
    let dash_empty = !dash.as_object().is_some_and(|o| !o.is_empty());
    if dash_empty {
        return Err(CliError::new(
            "rights",
            format!("[{used}] playurl 未返回 dash 流：可能是版权/权益限制"),
        )
        .channel(if used == "app" {
            "app"
        } else if used == "tv" {
            "tv"
        } else {
            ""
        })
        .into());
    }
    let picked = play_port::pick_streams(dash, want_qn, codec.to_string()).map_err(port_cli_err)?;
    let (video, audio) = match picked.get("$tuple").and_then(|x| x.as_array()) {
        Some(arr) if arr.len() == 2 => (arr[0].clone(), arr[1].clone()),
        _ => return Err(Failure::Raw("pick_streams 返回了非 $tuple 标记".into())),
    };

    let qn_id = video.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
    let qn_text = qn_label(qn_id);
    let mut ent = play_port::entitlement_of(d.clone()).map_err(Failure::Raw)?;
    let state = if qn_id >= 64 || quality.is_some() {
        "yes"
    } else {
        "limited"
    };
    let mut my_access = Map::new();
    my_access.insert("state".into(), json!(state));
    my_access.insert("text".into(), json!(access_text(state)));
    my_access.insert("best_quality".into(), json!(qn_text));
    my_access.insert("via".into(), json!(used));
    if let Some(o) = ent.as_object_mut() {
        o.insert("my_access".into(), Value::Object(my_access));
    }
    if qn_id < 64 && quality.is_none() {
        eprintln!(
            "⚠ 当前只拿到 {qn_text}（{qn_id}）：账号权益或登录态不足。试试 `bilibili_cli login` / `login --tv`"
        );
    }
    if ent.get("kind").and_then(|x| x.as_str()).unwrap_or("free") != "free" {
        eprintln!(
            "  🏷 {} · {}",
            ent.get("label").and_then(|x| x.as_str()).unwrap_or(""),
            ent.get("my_access")
                .and_then(|m| m.get("text"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
        );
    }

    let out_root = out_dir
        .map(|s| s.to_string())
        .unwrap_or_else(default_out_dir);
    let owner_dir = parse_port::safe_name(owner_name.clone(), 80).map_err(Failure::Raw)?;
    let base = std::path::Path::new(&out_root).join(&owner_dir);
    std::fs::create_dir_all(&base).map_err(|e| Failure::Raw(format!("{e}")))?;
    let date = parse_port::iso(pub_).map_err(Failure::Raw)?;
    let date = &date[..10];
    let stem = format!(
        "{date}_{}",
        parse_port::safe_name(title.clone(), 80).map_err(Failure::Raw)?
    );
    let referer = format!("https://www.bilibili.com/video/{bvid}");

    let out;
    if audio_only {
        if audio.as_object().is_none_or(|o| o.is_empty()) {
            return Err(
                CliError::new("rights", format!("[{used}] 没有独立音频轨，无法只下音频"))
                    .channel(if used == "app" {
                        "app"
                    } else if used == "tv" {
                        "tv"
                    } else {
                        ""
                    })
                    .into(),
            );
        }
        let kbps = audio.get("bandwidth").and_then(|x| x.as_i64()).unwrap_or(0) / 1000;
        println!("→ [{used}] 音频 · {kbps} kbps");
        out = base.join(format!("{stem}.m4a"));
        download_track(
            &play_port::_url_of(audio.clone()).map_err(Failure::Raw)?,
            &out,
            &referer,
            parallel,
        )?;
    } else {
        let vtmp = base.join(format!(".{stem}.video.m4s"));
        out = base.join(format!("{stem}.mp4"));
        println!(
            "→ [{used}] 视频 {qn_text} · {}x{} · {}",
            video.get("width").and_then(|x| x.as_i64()).unwrap_or(0),
            video.get("height").and_then(|x| x.as_i64()).unwrap_or(0),
            video.get("codecs").and_then(|x| x.as_str()).unwrap_or("")
        );
        download_track(
            &play_port::_url_of(video.clone()).map_err(Failure::Raw)?,
            &vtmp,
            &referer,
            parallel,
        )?;
        if audio.as_object().is_some_and(|o| !o.is_empty()) {
            let atmp = base.join(format!(".{stem}.audio.m4s"));
            download_track(
                &play_port::_url_of(audio.clone()).map_err(Failure::Raw)?,
                &atmp,
                &referer,
                parallel,
            )?;
            merge(&vtmp, &atmp, &out)?;
            if !keep_parts {
                let _ = std::fs::remove_file(&vtmp);
                let _ = std::fs::remove_file(&atmp);
            }
        } else {
            std::fs::rename(&vtmp, &out).map_err(|e| Failure::Raw(format!("{e}")))?;
        }
    }

    let out_str = out.to_string_lossy().into_owned();
    let iso_pub = parse_port::iso(pub_).map_err(Failure::Raw)?;
    let dur = i64_of(&d, "duration");
    let desc_raw = d.get("desc").and_then(|x| x.as_str()).unwrap_or("");
    let mut meta = Map::new();
    meta.insert("schema".into(), json!(SCHEMA));
    meta.insert("id".into(), json!(bvid));
    meta.insert("aid".into(), json!(aid));
    meta.insert("cid".into(), json!(cid));
    meta.insert("title".into(), json!(title));
    meta.insert(
        "url".into(),
        json!(format!("https://www.bilibili.com/video/{bvid}")),
    );
    let mut author = Map::new();
    author.insert(
        "uid".into(),
        owner_obj.get("mid").cloned().unwrap_or(json!(null)),
    );
    author.insert("name".into(), json!(owner_name));
    meta.insert("author".into(), Value::Object(author));
    meta.insert("published_at".into(), json!(iso_pub));
    meta.insert("published_date".into(), json!(&iso_pub[..10]));
    meta.insert("duration_sec".into(), json!(dur));
    meta.insert(
        "duration_text".into(),
        json!(parse_port::fmt_dur(dur as f64).map_err(Failure::Raw)?),
    );
    meta.insert("quality".into(), json!(qn_text));
    meta.insert("quality_id".into(), json!(qn_id));
    meta.insert(
        "codec".into(),
        video.get("codecs").cloned().unwrap_or(json!(null)),
    );
    meta.insert("channel".into(), json!(used));
    meta.insert("entitlement".into(), ent.clone());
    meta.insert("audio_only".into(), json!(audio_only));
    meta.insert(
        "desc".into(),
        json!(desc_raw.chars().take(2000).collect::<String>()),
    );
    meta.insert("file".into(), json!(out_str));
    meta.insert(
        "cover".into(),
        json!(d.get("pic").and_then(|x| x.as_str()).unwrap_or("")),
    );
    meta.insert(
        "downloaded_at".into(),
        json!(parse_port::iso(now_ts()).map_err(Failure::Raw)?),
    );

    let side = match out_str.rsplit_once('.') {
        Some((head, _)) => format!("{head}.info.json"),
        None => format!("{out_str}.info.json"),
    };
    std::fs::write(
        &side,
        serde_json::to_string_pretty(&Value::Object(meta.clone())).unwrap_or_default(),
    )
    .map_err(|e| Failure::Raw(format!("{e}")))?;

    if cover {
        let pic = d.get("pic").and_then(|x| x.as_str()).unwrap_or("");
        if !pic.is_empty() {
            let cpath = base.join(format!("{stem}.jpg"));
            match download_track(pic, &cpath, &referer, 1) {
                Ok(_) => println!(
                    "  封面 {}",
                    cpath
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                Err(Failure::Cli(e)) => {
                    eprintln!("  · 封面下载失败：{}（不影响主产物）", e.summary);
                }
                Err(other) => return Err(other),
            }
        }
    }
    if subs {
        let stem_path = match side.rsplit_once('.') {
            Some((head, _)) => head.to_string(),
            None => side.clone(),
        };
        save_subs(
            &bvid,
            cid,
            std::path::Path::new(&stem_path),
            &creds,
            &referer,
        );
    }

    let size_mb = std::fs::metadata(&out)
        .map(|m| m.len() as f64 / 1048576.0)
        .unwrap_or(0.0);
    if as_json {
        let mut full = meta;
        full.insert("bytes".into(), json!((size_mb * 1048576.0) as i64));
        println!(
            "{}",
            serde_json::to_string_pretty(&Value::Object(full)).unwrap_or_default()
        );
    } else {
        println!("✓ {out_str}  ({size_mb:.1} MB)");
        println!(
            "  元数据 {}",
            std::path::Path::new(&side)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
    }
    Ok(0)
}
