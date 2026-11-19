//! 下载胶水：`_curl_dl`（续传/重试/半成品必删）、`_merge`（ffmpeg）、`_save_subs`。
//! 逐字对齐 bilibili_cli.py:615 / 640 / 768。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use serde_json::Value;
use sign::port as sign_;

use super::curl::{CurlSpec, curl};
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
        let summary = last.as_ref().map(|e| e.summary.clone()).unwrap_or_default();
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
        return Err(CliError::new(
            "missing",
            "没有 ffmpeg，无法合并音视频（brew install ffmpeg）",
        )
        .into());
    };
    let p = Command::new(&ff)
        .args(["-y", "-loglevel", "error", "-i"])
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
pub fn save_subs(bvid: &str, cid: i64, stem: &Path, creds: &Value, referer: &str) {
    let cookie = match sign_::cookie_header(creds.clone()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let j = match fetch_json(
        &format!("https://api.bilibili.com/x/player/v2?bvid={bvid}&cid={cid}"),
        &super::http::FetchSpec::new()
            .referer(Some(referer))
            .cookie(&cookie),
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
        let lang = if lang.is_empty() {
            format!("sub{i}")
        } else {
            lang
        };
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
            println!(
                "  字幕 {}",
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            );
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

// ------------------------------------------------------------------ 分段并行

use super::curl::UA_WEB;

/// 分段阈值：小于它的流直接单流（ADR-014：32MB）。
const PARALLEL_THRESHOLD: u64 = 32 * 1024 * 1024;
/// 并行上限（实测 8 以上收益边际、连接面翻倍，ADR-014）。
const MAX_PARALLEL: i64 = 8;

/// `Range: bytes=0-0` 探测流总大小；无 Range 支持/失败 → None。
fn content_total(url: &str, referer: &str) -> Option<u64> {
    let out = Command::new("curl")
        .args([
            "-sS",
            "--compressed",
            "--http2",
            "--max-time",
            "20",
            "-A",
            UA_WEB,
            "-H",
            &format!("Referer: {referer}"),
            "-H",
            "Accept: application/json, text/plain, */*",
            "-r",
            "0-0",
            "-o",
            "/dev/null",
            "-D",
            "-",
        ])
        .arg(url)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&out.stdout);
    for line in head.lines() {
        if line.to_ascii_lowercase().starts_with("content-range:") {
            let total = line.rsplit('/').next()?.trim();
            return total.parse().ok();
        }
    }
    None
}

/// 单段下载：3 次整段重试；最终失败删本段半成品。
fn download_part(
    url: &str,
    dest: &Path,
    referer: &str,
    start: u64,
    end: u64,
) -> Result<(), CliError> {
    let want = end - start + 1;
    let mut last = CliError::new("network", "下载失败（未知原因）");
    for i in 0..3 {
        let dest_str = dest.to_string_lossy().into_owned();
        let spec = CurlSpec::new()
            .referer(Some(referer))
            .timeout(3600)
            .save_to(&dest_str)
            .range(start, end);
        match curl(url, &spec) {
            Ok(_) => {
                let got = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                if got == want {
                    return Ok(());
                }
                last = CliError::new("empty", format!("分段大小不符（{got} ≠ {want}）"));
            }
            Err(e) => last = e,
        }
        let got = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        eprintln!(
            "  · 分段 [{start}-{end}] 中断（第 {}/3 次，已 {} MB）：{} → 整段重下",
            i + 1,
            mb(got),
            last.summary
        );
        sleep(Duration::from_secs(1));
    }
    let _ = std::fs::remove_file(dest);
    Err(last)
}

fn concat_parts(parts: &[PathBuf], dest: &Path, total: u64) -> Result<(), String> {
    let mut out = std::fs::File::create(dest).map_err(|e| format!("{e}"))?;
    for p in parts {
        let mut f = std::fs::File::open(p).map_err(|e| format!("{e}"))?;
        std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
    }
    out.flush().map_err(|e| format!("{e}"))?;
    let got = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    if got != total {
        return Err(format!("拼接大小不符（{got} ≠ {total}）"));
    }
    Ok(())
}

/// 分段并行下载一条流（ADR-014）。
/// parallel ≤ 1 / 文件小于阈值 / 无 Range 支持 → 单流；
/// 任一段最终失败或拼接异常 → 删全部半成品 → 回退单流（自带重试与必删语义）。
pub fn download_track(url: &str, dest: &Path, referer: &str, parallel: i64) -> Result<(), Failure> {
    if parallel <= 1 {
        return curl_dl(url, dest, referer);
    }
    let Some(total) = content_total(url, referer) else {
        return curl_dl(url, dest, referer);
    };
    if total < PARALLEL_THRESHOLD {
        return curl_dl(url, dest, referer);
    }
    let n = parallel.clamp(2, MAX_PARALLEL) as u64;
    let parts: Vec<(u64, u64)> = (0..n)
        .map(|i| (total * i / n, total * (i + 1) / n - 1))
        .collect();
    let part_paths: Vec<PathBuf> = (0..n)
        .map(|i| PathBuf::from(format!("{}.part{i}", dest.to_string_lossy())))
        .collect();

    let results: Vec<Result<(), CliError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = parts
            .iter()
            .zip(part_paths.iter())
            .map(|((s, e), pp)| scope.spawn(move || download_part(url, pp, referer, *s, *e)))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(CliError::new("unknown", "分段线程崩溃")))
            })
            .collect()
    });

    if results.iter().all(|r| r.is_ok()) {
        let joined = concat_parts(&part_paths, dest, total);
        for p in &part_paths {
            let _ = std::fs::remove_file(p);
        }
        if let Err(e) = joined {
            eprintln!("  · 拼接异常：{e} → 回退单流");
            let _ = std::fs::remove_file(dest);
            return curl_dl(url, dest, referer);
        }
        return Ok(());
    }
    for p in &part_paths {
        let _ = std::fs::remove_file(p);
    }
    eprintln!("  · 分段下载失败 → 回退单流");
    curl_dl(url, dest, referer)
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    /// 403 流：探测失败 → 回退单流 → 失败 → 半成品必删。
    /// 需联网，手动跑：cargo test -- --ignored
    #[test]
    #[ignore = "需联网；手动跑"]
    fn track_403_falls_back_to_single_and_cleans() {
        let dest = std::env::temp_dir().join("bcli_dl_track_404.m4s");
        std::fs::write(&dest, b"stale").unwrap();
        let r = download_track(
            "https://i0.hdslb.com/bfs/archive/__definitely_missing_404__.bin",
            &dest,
            "https://www.bilibili.com/",
            4,
        );
        assert!(r.is_err(), "403 流必须失败");
        assert!(!dest.exists(), "回退单流失败后半成品必须删除");
        assert!(
            !std::env::temp_dir()
                .join("bcli_dl_track_404.m4s.part0")
                .exists(),
            "分段半成品必须删除"
        );
    }
}
