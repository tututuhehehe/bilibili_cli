//! sign 模块移植桩：appkey 签名 + cookie 处理 + curl jar 解析。
//! 实现前必读 rust/tasks/00-shared-rules.md 与 rust/tasks/sign.md。
#![forbid(unsafe_code)]

/// Port of `sign_app` (`bilibili_cli.py:239`). 参数 + appkey + ts → md5 签名 query。
///
/// 源码内部读 `time.time()`，输出随时间变化，**无法录制比对**：
/// 等价性由 tests 里的固定时钟基准（`fixtures/sign_fixed_ts.json`）校验——
/// 把签名核心拆成可注入 ts 的内部函数，`sign_app` 只是取当前时钟后调用它。
///
/// `key` / `sec` 在源码中分别默认 `APP_KEY` / `APP_SEC`。
pub fn sign_app(
    params: serde_json::Value,
    key: Option<serde_json::Value>,
    sec: Option<serde_json::Value>,
) -> Result<String, String> {
    let _ = (&params, &key, &sec);
    unimplemented!("port `sign_app` from bilibili_cli.py")
}

/// Port of `cookie_header` (`bilibili_cli.py:271`). creds dict → Cookie 头（无则空串）。
pub fn cookie_header(c: serde_json::Value) -> Result<String, String> {
    let _ = &c;
    unimplemented!("port `cookie_header` from bilibili_cli.py")
}

/// Port of `_read_jar` (`bilibili_cli.py:352`). 解析 curl cookie jar 文本 → cookie dict。
///
/// 注意 `#HttpOnly_` 前缀行必须剥前缀而不是当注释跳过，否则丢 SESSDATA。
pub fn _read_jar(path: String) -> Result<serde_json::Value, String> {
    let _ = &path;
    unimplemented!("port `_read_jar` from bilibili_cli.py")
}
