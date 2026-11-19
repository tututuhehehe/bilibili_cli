//! REST 层：`fetch_json` / `view` / `user_videos` / `user_name`
//! （bilibili_cli.py:204 / 456 / 464 / 479），错误分类逐字对齐。

use serde_json::{json, Map, Value};

use sign::port;

use super::curl::{curl, CurlSpec, UA_APP};
use super::err::{biz_code, CliError, Failure};

#[derive(Default)]
pub struct FetchSpec<'a> {
    pub ua: Option<&'a str>,
    pub referer: Option<&'a str>,
    pub cookie: &'a str,
    pub post: Option<&'a str>,
    pub channel: &'static str,
    pub allow_codes: &'a [i64],
}

impl<'a> FetchSpec<'a> {
    /// 默认对齐源码：ua=UA_WEB、referer=根站。
    pub fn new() -> Self {
        Self {
            ua: None,
            referer: Some("https://www.bilibili.com/"),
            cookie: "",
            post: None,
            channel: "",
            allow_codes: &[],
        }
    }
    pub fn ua(mut self, ua: &'a str) -> Self {
        self.ua = Some(ua);
        self
    }
    pub fn referer(mut self, r: Option<&'a str>) -> Self {
        self.referer = r;
        self
    }
    pub fn cookie(mut self, c: &'a str) -> Self {
        self.cookie = c;
        self
    }
    pub fn channel(mut self, c: &'static str) -> Self {
        self.channel = c;
        self
    }
}

fn endpoint_of(url: &str) -> String {
    url.split('?').next().unwrap_or(url).replace("https://", "")
}

fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `fetch_json`（bilibili_cli.py:204）。allow_codes 里的业务码不当作错误。
pub fn fetch_json(url: &str, spec: &FetchSpec) -> Result<Value, Failure> {
    let mut cs = CurlSpec::new();
    if let Some(ua) = spec.ua {
        cs.ua = Some(ua);
    }
    cs.referer = spec.referer;
    cs.cookie = spec.cookie;
    cs.post = spec.post;
    let (body, http) = curl(url, &cs)?;
    let endpoint = endpoint_of(url);

    if http >= 500 {
        return Err(CliError::new("server", format!("HTTP {http}：服务端错误"))
            .endpoint(endpoint)
            .http(http)
            .raw(take_chars(&body, 200))
            .channel(spec.channel)
            .into());
    }
    if http == 403 || http == 412 || http == 429 || body.trim_start().starts_with('<') {
        return Err(CliError::new(
            "blocked",
            format!("HTTP {http}，返回的是网页/拦截页而不是 JSON"),
        )
        .endpoint(endpoint)
        .http(http)
        .raw(take_chars(&body, 200))
        .channel(spec.channel)
        .into());
    }
    let j: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return Err(CliError::new("unknown", "返回内容不是 JSON")
                .endpoint(endpoint)
                .http(http)
                .raw(take_chars(&body, 200))
                .channel(spec.channel)
                .into());
        }
    };

    let code = j.get("code").and_then(|c| c.as_i64());
    if code == Some(0) || code.is_some_and(|c| spec.allow_codes.contains(&c)) {
        return Ok(j);
    }
    let (cat, msg, raw) = match code.and_then(|c| biz_code(c)) {
        Some((cat, m)) => (
            cat,
            m.to_string(),
            j.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("None")
                .to_string(),
        ),
        None => {
            let c_disp = code.map(|c| c.to_string()).unwrap_or_else(|| "None".into());
            let m_disp = j
                .get("message")
                .map(|m| match m {
                    Value::String(s) => s.clone(),
                    Value::Null => "None".into(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| "None".into());
            (
                "unknown",
                format!("业务码 {c_disp}: {m_disp}"),
                j.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string(),
            )
        }
    };
    Err(CliError::new(cat, msg)
        .endpoint(endpoint)
        .http(http)
        .code(code.unwrap_or(0))
        .raw(raw)
        .channel(spec.channel)
        .into())
}

/// `view`（bilibili_cli.py:456）。
pub fn view(bvid: &str) -> Result<Value, Failure> {
    let j = fetch_json(
        &format!("https://api.bilibili.com/x/web-interface/view?bvid={bvid}"),
        &FetchSpec::new(),
    )?;
    match j.get("data") {
        Some(d @ Value::Object(_)) => Ok(d.clone()),
        _ => Err(CliError::new("interface", "view 接口没有 data 字段")
            .raw(take_chars(&serde_json::to_string(&j).unwrap_or_default(), 200))
            .into()),
    }
}

/// `user_videos`（bilibili_cli.py:464）：appkey 签名的投稿游标接口，匿名可用。
/// `n` 保持 i64 以复刻 Python 切片语义（负数 = 从尾部砍）。
pub fn user_videos(uid: &str, n: i64) -> Result<Value, Failure> {
    let params = json!({
        "vmid": uid,
        "order": "pubdate",
        "ps": n.max(5).to_string(),
        "pn": "1",
        "mobi_app": "android",
        "platform": "android",
        "build": "7730200",
        "c_locale": "zh_CN",
        "s_locale": "zh_CN",
    });
    let q = port::sign_app(params, None, None).map_err(Failure::Raw)?;
    let url = format!("https://app.bilibili.com/x/v2/space/archive/cursor?{q}");
    let j = fetch_json(
        &url,
        &FetchSpec::new().ua(UA_APP).referer(Some("")),
    )?;
    let d = j.get("data");
    let items = d
        .and_then(|d| d.get("item"))
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    let raw_len = items.len();
    let raw: Vec<Value> = items;
    let items: Vec<Value> = raw
        .into_iter()
        .filter(|it| {
            it.get("goto").and_then(|g| g.as_str()) == Some("av")
                && it.get("bvid").and_then(|b| b.as_str()).is_some_and(|b| !b.is_empty())
        })
        .take({
            let keep = if n >= 0 {
                n as usize
            } else {
                raw_len.saturating_sub((-n) as usize)
            };
            keep
        })
        .collect();
    let d_obj = j.get("data").is_some_and(|x| x.is_object());
    let has_item = j.get("data").and_then(|x| x.get("item")).is_some();
    if !d_obj || !has_item {
        let keys: Vec<String> = d
            .and_then(|x| x.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        return Err(CliError::new("interface", "投稿接口结构变了（缺少 data.item）")
            .raw(take_chars(
                &serde_json::to_string(&keys).unwrap_or_default(),
                160,
            ))
            .into());
    }
    let total = d
        .and_then(|x| x.get("count"))
        .and_then(|c| c.as_i64())
        .unwrap_or(items.len() as i64);
    let mut out = Map::new();
    out.insert("total".into(), json!(total));
    out.insert("items".into(), Value::Array(items));
    Ok(Value::Object(out))
}

/// `user_name`（bilibili_cli.py:479）：失败返回空串（不报错）。
pub fn user_name(uid: &str) -> String {
    let res = fetch_json(
        &format!("https://api.bilibili.com/x/web-interface/card?mid={uid}"),
        &FetchSpec::new().referer(Some(&format!("https://space.bilibili.com/{uid}/"))),
    );
    match res {
        Ok(j) => j
            .pointer("/data/card/name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string(),
        Err(_) => String::new(),
    }
}
