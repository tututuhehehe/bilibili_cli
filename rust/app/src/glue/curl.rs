//! `curl`（bilibili_cli.py:153）：shell 出系统 curl，逐参数对齐。

use std::process::Command;

use super::err::{CliError, curl_exit};

pub const UA_WEB: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
pub const UA_APP: &str =
    "Mozilla/5.0 BiliDroid/7.73.0 (bbcallen@gmail.com) os/android model/Pixel up=1.0";
pub const UA_TV: &str = "Mozilla/5.0 BiliDroid/7.73.0";

pub struct CurlSpec<'a> {
    pub ua: Option<&'a str>,
    pub referer: Option<&'a str>,
    pub timeout: u64,
    pub cookie: &'a str,
    pub post: Option<&'a str>,
    pub save_to: Option<&'a str>,
    pub resume: bool,
    pub quiet: bool,
    pub jar: &'a str,
    pub range: Option<(u64, u64)>,
}

impl<'a> CurlSpec<'a> {
    /// 默认值对齐源码：ua=UA_WEB、referer=根站、timeout=30。
    pub fn new() -> Self {
        Self {
            ua: None,
            referer: Some("https://www.bilibili.com/"),
            timeout: 30,
            cookie: "",
            post: None,
            save_to: None,
            resume: false,
            quiet: false,
            jar: "",
            range: None,
        }
    }
    pub fn referer(mut self, r: Option<&'a str>) -> Self {
        self.referer = r;
        self
    }
    pub fn timeout(mut self, t: u64) -> Self {
        self.timeout = t;
        self
    }
    pub fn save_to(mut self, d: &'a str) -> Self {
        self.save_to = Some(d);
        self
    }
    pub fn resume(mut self, r: bool) -> Self {
        self.resume = r;
        self
    }
    pub fn range(mut self, s: u64, e: u64) -> Self {
        self.range = Some((s, e));
        self
    }
}

fn endpoint_of(url: &str) -> String {
    let url = url.split('?').next().unwrap_or(url);
    url.replace("https://", "")
}

/// 返回 (body, http_status)；save_to 时 body 为空。
pub fn curl(url: &str, spec: &CurlSpec) -> Result<(String, i64), CliError> {
    let ua = spec.ua.unwrap_or(UA_WEB);
    let mut cmd: Vec<String> = vec![
        "curl".into(),
        "-sS".into(),
        "--compressed".into(),
        "--http2".into(),
        "--max-time".into(),
        spec.timeout.to_string(),
        "-A".into(),
        ua.into(),
    ];
    if let Some(r) = spec.referer
        && !r.is_empty()
    {
        cmd.push("-H".into());
        cmd.push(format!("Referer: {r}"));
    }
    cmd.push("-H".into());
    cmd.push("Accept: application/json, text/plain, */*".into());
    if !spec.jar.is_empty() {
        cmd.push("-c".into());
        cmd.push(spec.jar.into());
        cmd.push("-b".into());
        cmd.push(spec.jar.into());
    }
    if !spec.cookie.is_empty() {
        cmd.push("-H".into());
        cmd.push(format!("Cookie: {}", spec.cookie));
    }
    if let Some(p) = spec.post {
        cmd.push("-X".into());
        cmd.push("POST".into());
        cmd.push("--data".into());
        cmd.push(p.into());
    }
    if let Some(dest) = spec.save_to {
        cmd.push("-o".into());
        cmd.push(dest.into());
        cmd.push("--retry".into());
        cmd.push("3".into());
        cmd.push("--retry-delay".into());
        cmd.push("2".into());
        if spec.resume {
            cmd.push("-C".into());
            cmd.push("-".into());
        }
        if let Some((s, e)) = spec.range {
            cmd.push("-r".into());
            cmd.push(format!("{s}-{e}"));
        }
        cmd.push("-w".into());
        cmd.push(if spec.quiet {
            "%{http_code}".into()
        } else {
            "%{http_code} %{size_download}".into()
        });
    } else {
        cmd.push("-w".into());
        cmd.push("\n__HTTP__%{http_code}".into());
    }
    cmd.push(url.into());

    let out = Command::new(&cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| {
            CliError::new("network", format!("curl 启动失败：{e}")).endpoint(endpoint_of(url))
        })?;
    let rc = out.status.code().unwrap_or(-1);
    if rc != 0 {
        let stderr_txt = String::from_utf8_lossy(&out.stderr);
        let trimmed: String = stderr_txt.trim().chars().take(120).collect();
        let (cat, msg) = match curl_exit(rc) {
            Some((c, m)) => (c, m.to_string()),
            None => ("network", format!("curl 退出码 {rc}")),
        };
        let summary = if msg.starts_with("curl 退出码") {
            format!("{msg}：{trimmed}")
        } else {
            msg
        };
        return Err(CliError::new(cat, summary).endpoint(endpoint_of(url)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();

    if let Some(dest) = spec.save_to {
        let mut parts = stdout.split_whitespace();
        let http: i64 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        if http >= 400 {
            return Err(CliError::new("network", format!("下载返回 HTTP {http}"))
                .endpoint(endpoint_of(url))
                .http(http));
        }
        let _ = dest;
        return Ok((String::new(), http));
    }

    if let Some(pos) = stdout.rfind("\n__HTTP__") {
        let body = &stdout[..pos];
        let tail = &stdout[pos + "\n__HTTP__".len()..];
        let http: i64 = tail.trim().parse().unwrap_or(0);
        return Ok((body.to_string(), http));
    }
    Ok((stdout, 0))
}

impl Default for CurlSpec<'_> {
    fn default() -> Self {
        Self::new()
    }
}
