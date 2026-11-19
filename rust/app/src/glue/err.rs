//! 错误模型：`CliError` / `LABELS` / `HINTS` / `CURL_EXIT` / `BIZ_CODES` / `report`，
//! 以及命令胶水统一的 `Failure`（CliError 或未预期错误）→ 退出码。

use std::io::Write;
use std::process::ExitCode;

pub const SCHEMA: &str = "bilibili_cli.v1";

pub struct CliError {
    pub category: &'static str,
    pub summary: String,
    pub endpoint: String,
    pub http: i64,
    pub code: Option<i64>,
    pub raw: String,
    pub channel: &'static str,
}

impl CliError {
    pub fn new(category: &'static str, summary: impl Into<String>) -> Self {
        Self {
            category,
            summary: summary.into(),
            endpoint: String::new(),
            http: 0,
            code: None,
            raw: String::new(),
            channel: "",
        }
    }
    pub fn endpoint(mut self, e: impl Into<String>) -> Self {
        self.endpoint = e.into();
        self
    }
    pub fn http(mut self, h: i64) -> Self {
        self.http = h;
        self
    }
    pub fn code(mut self, c: i64) -> Self {
        self.code = Some(c);
        self
    }
    pub fn raw(mut self, r: impl Into<String>) -> Self {
        self.raw = r.into();
        self
    }
    pub fn channel(mut self, c: &'static str) -> Self {
        self.channel = c;
        self
    }
}

/// `LABELS`（bilibili_cli.py:120）。
pub fn label(category: &str) -> &str {
    match category {
        "network" => "网络 / 域名",
        "tls" => "TLS",
        "timeout" => "超时",
        "empty" => "空响应",
        "blocked" => "风控拦截",
        "auth" => "密钥 / 登录态",
        "interface" => "接口改版",
        "rights" => "账号权益",
        "rate_limit" => "频率限制",
        "server" => "服务端",
        "missing" => "缺少依赖",
        "unknown" => "未知",
        other => return other,
    }
}

/// `HINTS`（bilibili_cli.py:94）。
pub fn hint(category: &str) -> &'static str {
    match category {
        "network" => "检查网络 / DNS；key 本身没问题",
        "tls" => "稍后重试，或换网络",
        "timeout" => "稍后重试或降低频率",
        "empty" => "重试一次",
        "blocked" => "降低频率后重试；持续则换通道",
        "auth" => "重新 `bilibili_cli login`；若刚登录仍报此错，说明接口改了鉴权",
        "interface" => "接口可能已改版：多了必填参数或返回结构变了",
        "rights" => "这是账号权益问题，不是程序问题：大会员 / 充电 / 版权内容各自需要对应许可",
        "rate_limit" => "降低频率",
        "server" => "B 站侧问题，稍后再试",
        _ => "把原始响应贴出来再判断",
    }
}

/// `report`（bilibili_cli.py:126）：逐字对齐的 stderr 报错排版。
pub fn report(e: &CliError) {
    let mut err = std::io::stderr().lock();
    let chan = if e.channel.is_empty() {
        String::new()
    } else {
        format!("（通道 {}）", e.channel)
    };
    let _ = writeln!(err, "\n✗ 失败层级：{}{chan}", label(e.category));
    let _ = writeln!(err, "  原因：{}", e.summary);
    if !e.endpoint.is_empty() {
        let _ = writeln!(err, "  接口：{}", e.endpoint);
    }
    let mut meta: Vec<String> = Vec::new();
    if e.http != 0 {
        meta.push(format!("HTTP {}", e.http));
    }
    if let Some(c) = e.code {
        meta.push(format!("业务码 {c}"));
    }
    if !meta.is_empty() {
        let _ = writeln!(err, "  细节：{}", meta.join(" · "));
    }
    if !e.raw.is_empty() {
        let flat: String = e.raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let flat: String = flat.chars().take(160).collect();
        let _ = writeln!(err, "  原文：{flat}");
    }
    let h = hint(e.category);
    if !h.is_empty() {
        let _ = writeln!(err, "  建议：{h}");
    }
}

/// `CURL_EXIT`（bilibili_cli.py:70）。
pub fn curl_exit(rc: i32) -> Option<(&'static str, &'static str)> {
    Some(match rc {
        6 => ("network", "DNS 解析失败：域名可能已停用或被屏蔽"),
        7 => ("network", "连接失败：域名或端口不可达"),
        28 => ("timeout", "请求超时：网络慢或被限速"),
        35 => ("tls", "TLS 握手失败：证书问题或中间设备干扰"),
        52 => ("empty", "服务端返回空响应"),
        56 => ("network", "接收数据失败：连接被重置（常伴随风控）"),
        60 => ("tls", "证书校验失败"),
        _ => return None,
    })
}

/// `BIZ_CODES`（bilibili_cli.py:80）：业务码 → (分类, 摘要)。
pub fn biz_code(code: i64) -> Option<(&'static str, &'static str)> {
    Some(match code {
        -101 => ("auth", "账号未登录：需要登录后再试"),
        -352 => ("blocked", "风控校验失败"),
        -412 => ("blocked", "请求被风控拦截"),
        -403 => ("auth", "访问权限不足：签名无效，或 appkey/appsec 已失效"),
        -404 => ("interface", "资源不存在：ID 错误、接口路径变更，或 appkey 无效"),
        -400 => ("interface", "请求参数不被接受：接口参数可能已改版"),
        -10403 => ("rights", "地区或权限限制：需要对应权益"),
        -509 => ("rate_limit", "请求过于频繁"),
        -799 => ("rate_limit", "请求过于频繁"),
        -500 => ("server", "服务端内部错误"),
        -1 => ("server", "应用不存在或服务暂不可用"),
        87007 => ("rights", "充电专属视频：需要给该 UP 主充电"),
        87008 => ("rights", "充电专属视频：当前账号未充电"),
        76150 => ("rights", "稿件不可见：通常是充电专属/会员专享等权限限制"),
        _ => return None,
    })
}

/// 命令胶水的统一错误：CliError 或未预期错误（Python main 的兜底分支）。
pub enum Failure {
    Cli(CliError),
    Raw(String),
}

impl From<CliError> for Failure {
    fn from(e: CliError) -> Self {
        Failure::Cli(e)
    }
}
impl From<String> for Failure {
    fn from(s: String) -> Self {
        Failure::Raw(s)
    }
}

/// `report` + main 的退出码映射（rights→7 auth→5 missing→6 其余→3）；
/// 未预期错误走 `die("未预期错误: ...", 3)` 语义。
pub fn fail_exit(f: Failure) -> ExitCode {
    match f {
        Failure::Cli(e) => {
            report(&e);
            let code = match e.category {
                "rights" => 7,
                "auth" => 5,
                "missing" => 6,
                _ => 3,
            };
            ExitCode::from(code as u8)
        }
        Failure::Raw(s) => {
            eprintln!("错误: 未预期错误: {s}");
            ExitCode::from(3)
        }
    }
}
