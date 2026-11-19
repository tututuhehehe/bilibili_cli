//! proto 模块移植桩：protobuf wire 编解码 + App gRPC 搜索/详情卡解析。
//! 实现前必读 rust/tasks/00-shared-rules.md 与 rust/tasks/proto.md。
//!
//! bridge 标记约定（对照 src/pyconv.rs）：
//! - Python `bytes` 参数/返回 → `{"$bytes":[u8..]}`
//! - int 键 dict → `{"$dict":[[k,v]..]}`；tuple → `{"$tuple":[..]}`；None → null

#![forbid(unsafe_code)]

/// Port of `_pv` (`bilibili_cli.py:1122`). varint 编码。
pub fn _pv(n: i64) -> Result<serde_json::Value, String> {
    let _ = &n;
    unimplemented!("port `_pv` from bilibili_cli.py")
}

/// Port of `_ps` (`bilibili_cli.py:1134`). string 字段编码。
pub fn _ps(num: i64, v: String) -> Result<serde_json::Value, String> {
    let _ = (&num, &v);
    unimplemented!("port `_ps` from bilibili_cli.py")
}

/// Port of `_pi` (`bilibili_cli.py:1140`). int 字段编码。
pub fn _pi(num: i64, n: i64) -> Result<serde_json::Value, String> {
    let _ = (&num, &n);
    unimplemented!("port `_pi` from bilibili_cli.py")
}

/// Port of `_pm` (`bilibili_cli.py:1145`). 嵌套 message 字段编码。
pub fn _pm(num: i64, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let _ = (&num, &body);
    unimplemented!("port `_pm` from bilibili_cli.py")
}

/// Port of `_pread` (`bilibili_cli.py:1150`). wire 字节 → {field: [(wire_type, value)..]}。
pub fn _pread(buf: serde_json::Value) -> Result<serde_json::Value, String> {
    let _ = &buf;
    unimplemented!("port `_pread` from bilibili_cli.py")
}

/// Port of `_ptext` (`bilibili_cli.py:1190`).
///
/// `default` 在源码中默认 `""`。
pub fn _ptext(fields: serde_json::Value, num: i64, default: String) -> Result<String, String> {
    let _ = (&fields, &num, &default);
    unimplemented!("port `_ptext` from bilibili_cli.py")
}

/// Port of `_pint` (`bilibili_cli.py:1198`).
///
/// `default` 在源码中默认 `0`。
pub fn _pint(fields: serde_json::Value, num: i64, default: i64) -> Result<i64, String> {
    let _ = (&fields, &num, &default);
    unimplemented!("port `_pint` from bilibili_cli.py")
}

/// Port of `_pmsg` (`bilibili_cli.py:1206`). 取嵌套 message 的 fields dict。
pub fn _pmsg(fields: serde_json::Value, num: i64) -> Result<serde_json::Value, String> {
    let _ = (&fields, &num);
    unimplemented!("port `_pmsg` from bilibili_cli.py")
}

/// Port of `_prep` (`bilibili_cli.py:1213`). 取 repeated 字段的值列表。
pub fn _prep(fields: serde_json::Value, num: i64) -> Result<Vec<serde_json::Value>, String> {
    let _ = (&fields, &num);
    unimplemented!("port `_prep` from bilibili_cli.py")
}

/// Port of `_strip_em` (`bilibili_cli.py:1265`). 标题去高亮/emote。
pub fn _strip_em(t: String) -> Result<String, String> {
    let _ = &t;
    unimplemented!("port `_strip_em` from bilibili_cli.py")
}

/// Port of `_dur_seconds` (`bilibili_cli.py:1269`). "25:7"/"68:44" → 秒。
pub fn _dur_seconds(text: String) -> Result<i64, String> {
    let _ = &text;
    unimplemented!("port `_dur_seconds` from bilibili_cli.py")
}

/// Port of `_video_from_item` (`bilibili_cli.py:1280`). SearchItem → 视频卡 dict 或 None。
pub fn _video_from_item(item: serde_json::Value) -> Result<serde_json::Value, String> {
    let _ = &item;
    unimplemented!("port `_video_from_item` from bilibili_cli.py")
}

/// Port of `_user_from_item` (`bilibili_cli.py:1310`). SearchItem → UP 主卡 dict 或 None。
pub fn _user_from_item(item: serde_json::Value) -> Result<serde_json::Value, String> {
    let _ = &item;
    unimplemented!("port `_user_from_item` from bilibili_cli.py")
}
