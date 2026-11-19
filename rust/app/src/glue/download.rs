//! 下载胶水：`_curl_dl`（续传/重试/半成品必删）、`_merge`（ffmpeg）、`_save_subs`。
//! 逐字对齐 bilibili_cli.py:615 / 640 / 768。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use serde_json::Value;
use sign::port as sign_;

use super::curl::{curl, CurlSpec};
use super::err::{CliError, Failure};
use super::http::fetch_json;

/// `shutil.which` 的最小实现：沿 PATH 找可执行文件。
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    for dir in path.split(':') {
        if dir.is_empty() {
            continue;
        }
        let cand = Path::new(dir).join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn mb(size: u64) -> String {
    format!("{:.1}", size as f64 / 1048576.0)
}

/// `_curl_dl`（bilibili_cli.py:615）：中断可自动续传；最终失败删除半成品，
/// 绝不留下「看起来正常」的文件。
pub fn curl_dl(url: &str, dest: &Path, referer: &str) -> Result<(), Failure> {
    let mut last: Option<CliError> = None;
    for i in 0..3 {
        match curl(
            url,
            &CurlSpec::new()
                .referer(Some(referer))
                .timeout(3600)
                .save_to(&dest.to_string_lossy())
                .resume(true),
        ) {
            Ok(_) => {
                let size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                if size < 1024 {
                    last = Some(CliError::new("empty", format!("下载文件过小（{size} B）")));
                } else {
                    return Ok(());
                }
            }
            Err(e) => last = Some(e),
        }
        let got = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        let summary = last
            .as_ref()
            .map(|e| e.summary.clone())
            .unwrap_or_default();
        eprintln!(
            "  · 下载中断（第 {}/3 次，已 {} MB）：{summary} → 续传重试",
            i + 1,
            mb(got)
        );
        sleep(Duration::from_secs(1));
    }
    let _ = std::fs::remove_file(dest);
    match last {
        Some(e) => Err(e.into()),
        None => Err(CliError::new("network", "下载失败（未知原因）").into()),
    }
}

/// `_merge`（bilibili_cli.py:640）。
pub fn merge(video: &Path, audio: &Path, out: &Path) -> Result<(), Failure> {
    let Some(ff) = which("ffmpeg") else {
        return Err(CliError::new("missing", "没有 ffmpeg，无法合并音视频（brew install ffmpeg）").into());
    };
    let p = Command::new(&ff)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-i",
        ])
        .arg(video)
        .arg("-i")
        .arg(audio)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(out)
        .output()
        .map_err(|e| Failure::Raw(format!("{e}")))?;
    if !p.status.success() {
        let stderr_txt = String::from_utf8_lossy(&p.stderr);
        let trimmed: String = stderr_txt.trim().chars().take(200).collect();
        return Err(CliError::new("unknown", format!("ffmpeg 合并失败：{trimmed}")).into());
    }
    Ok(())
}

/// `_save_subs`（bilibili_cli.py:768）：每条字幕独立容错，失败静默跳过。
pub fn save_subs(
    bvid: &str,
    cid: i64,
    stem: &Path,
    creds: &Value,
    referer: &str,
) {
    let cookie = match sign_::cookie_header(creds.clone()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let j = match fetch_json(
        &format!("https://api.bilibili.com/x/player/v2?bvid={bvid}&cid={cid}"),
        &super::http::FetchSpec::new().referer(Some(referer)).cookie(&cookie),
    ) {
        Ok(j) => j,
        Err(_) => return,
    };
    let items = j
        .pointer("/data/subtitle/subtitles")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    for (i, s) in items.iter().take(2).enumerate() {
        let lang = s
            .get("lan")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let lang = if lang.is_empty() { format!("sub{i}") } else { lang };
        let mut url = s
            .get("subtitle_url")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if url.starts_with("//") {
            url = format!("https:{url}");
        }
        if url.is_empty() {
            continue;
        }
        let body = match curl(&url, &CurlSpec::new().referer(Some(referer))) {
            Ok((b, _)) => b,
            Err(_) => continue,
        };
        let data: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let segs = data
            .get("body")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let mut lines: Vec<String> = Vec::new();
        for (idx, seg) in segs.iter().enumerate() {
            let start = seg.get("from").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let end = seg.get("to").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let content = seg.get("content").and_then(|x| x.as_str()).unwrap_or("");
            lines.push((idx + 1).to_string());
            lines.push(format!("{} --> {}", srt_ts(start), srt_ts(end)));
            lines.push(content.to_string());
            lines.push(String::new());
        }
        let path = stem.with_extension(format!("{lang}.srt"));
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = f.write_all(lines.join("\n").as_bytes());
            println!("  字幕 {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
    }
}

fn srt_ts(x: f64) -> String {
    // int(x // 3600), int(x % 3600 // 60), x % 60（Python floor 语义）
    let h = x.div_euclid(3600.0) as i64;
    let m = x.rem_euclid(3600.0).div_euclid(60.0) as i64;
    let sec = x.rem_euclid(3600.0).rem_euclid(60.0);
    format!("{h:02}:{m:02}:{:06.3}", sec).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 半成品必删（bilibili_cli.py:637 的承诺）：下载 3 轮全败后，半成品必须删除。
    /// 需联网（404/403 响应），手动跑：cargo test -- --ignored
    #[test]
    #[ignore = "需联网；手动跑"]
    fn partial_deleted_after_failed_attempts() {
        let dest = std::env::temp_dir().join("bcli_dl_partial_test.m4s");
        std::fs::write(&dest, b"garbage-partial").unwrap();
        let r = curl_dl(
            "https://i0.hdslb.com/bfs/archive/__definitely_missing_404__.bin",
            &dest,
            "https://www.bilibili.com/",
        );
        assert!(r.is_err(), "404 下载必须失败");
        assert!(!dest.exists(), "半成品必须删除，绝不留「看起来正常」的文件");
    }
}
