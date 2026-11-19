//! proto 模块移植：protobuf wire 编解码 + App gRPC 搜索/详情卡解析。
//! 逐函数等价翻译自 `bilibili_cli.py`（行号见各函数），语义与文案逐字对齐。
//!
//! bridge 标记约定（对照 src/pyconv.rs）：
//! - Python `bytes` 参数/返回 → `{"$bytes":[u8..]}`
//! - int 键 dict → `{"$dict":[[k,v]..]}`；tuple → `{"$tuple":[..]}`；None → null
//!
//! 与原版的唯一显式差异：负数进 `_pv`/`_pi`/`_ps`/`_pm` 的字段号或值会让原版死循环
//!（`n >>= 7` 永远不为 0），port 返回 `Err("negative int: the original hangs here")`。
//! 详见任务书 `rust/tasks/proto.md` 与 `.rust-anything/timed-out.json`。

#![forbid(unsafe_code)]

use serde_json::{Value, json};

use crate::pb;
use crate::pytext;

const HANGS: &str = "negative int: the original hangs here";

/// varint 编码循环：`b = n & 0x7F; n >>= 7; …`。非负 i128 域内与 Python 逐字节一致。
fn pv_i128(mut n: i128) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (n & 0x7F) as u8;
        n >>= 7;
        out.push(b | if n != 0 { 0x80 } else { 0 });
        if n == 0 {
            return out;
        }
    }
}

/// Port of `_pv` (`bilibili_cli.py:1122`). varint 编码。
pub fn _pv(n: i64) -> Result<Value, String> {
    if n < 0 {
        return Err(HANGS.into());
    }
    Ok(json!({ "$bytes": pv_i128(n as i128) }))
}

/// Port of `_ps` (`bilibili_cli.py:1134`). string 字段编码。
pub fn _ps(num: i64, v: String) -> Result<Value, String> {
    if num < 0 {
        return Err(HANGS.into());
    }
    let tag = ((num as i128) << 3) | 2;
    let b = v.into_bytes();
    let mut out = pv_i128(tag);
    out.extend(pv_i128(b.len() as i128));
    out.extend(b);
    Ok(json!({ "$bytes": out }))
}

/// Port of `_pi` (`bilibili_cli.py:1140`). int 字段编码。
pub fn _pi(num: i64, n: i64) -> Result<Value, String> {
    if num < 0 || n < 0 {
        return Err(HANGS.into());
    }
    let mut out = pv_i128((num as i128) << 3);
    out.extend(pv_i128(n as i128));
    Ok(json!({ "$bytes": out }))
}

/// Port of `_pm` (`bilibili_cli.py:1145`). 嵌套 message 字段编码。
pub fn _pm(num: i64, body: Value) -> Result<Value, String> {
    if num < 0 {
        return Err(HANGS.into());
    }
    let body = bytes_arg(&body)?;
    let tag = ((num as i128) << 3) | 2;
    let mut out = pv_i128(tag);
    out.extend(pv_i128(body.len() as i128));
    out.extend(body);
    Ok(json!({ "$bytes": out }))
}

/// Port of `_pread` (`bilibili_cli.py:1150`). wire 字节 → {field: [(wiretype, value)..]}。
pub fn _pread(buf: Value) -> Result<Value, String> {
    let buf = bytes_arg(&buf)?;
    let fields = pb::pread(&buf)?;
    Ok(pb::fields_to_value(&fields))
}

/// Port of `_ptext` (`bilibili_cli.py:1190`).
///
/// `default` 在源码中默认 `""`。
pub fn _ptext(fields: Value, num: i64, default: String) -> Result<String, String> {
    let f = pb::fields_from_value(&fields)?;
    pb::ptext(&f, num, &default)
}

/// Port of `_pint` (`bilibili_cli.py:1198`).
///
/// `default` 在源码中默认 `0`。bridge 返回类型是 i64：varint 值 ≥ 2^63 时返回
/// `Err("OverflowError: int too large for the port")`（原版会给出大整数，属 bridge
/// i64 域的边界，见汇报的已知差异）。
pub fn _pint(fields: Value, num: i64, default: i64) -> Result<i64, String> {
    let f = pb::fields_from_value(&fields)?;
    match pb::pint(&f, num, default as i128) {
        v if v >= i64::MIN as i128 && v <= i64::MAX as i128 => Ok(v as i64),
        _ => Err("OverflowError: int too large for the port".into()),
    }
}

/// Port of `_pmsg` (`bilibili_cli.py:1206`). 取嵌套 message 的 fields dict。
pub fn _pmsg(fields: Value, num: i64) -> Result<Value, String> {
    let f = pb::fields_from_value(&fields)?;
    let inner = pb::pmsg(&f, num)?;
    Ok(pb::fields_to_value(&inner))
}

/// Port of `_prep` (`bilibili_cli.py:1213`). 取 repeated 字段的值列表。
pub fn _prep(fields: Value, num: i64) -> Result<Vec<Value>, String> {
    let f = pb::fields_from_value(&fields)?;
    Ok(pb::prep(&f, num))
}

/// Port of `_strip_em` (`bilibili_cli.py:1265`). 标题去高亮（源码只剥
/// `<em class="keyword">` 与 `</em>` 两个标签，emote 短代码不动），再 `str.strip()`。
pub fn _strip_em(t: String) -> Result<String, String> {
    Ok(pytext::py_strip(&t.replace("<em class=\"keyword\">", "").replace("</em>", "")).into())
}

/// Port of `_dur_seconds` (`bilibili_cli.py:1269`). "25:7"/"68:44"/"1:02:33" → 秒。
///
/// `re.findall(r"(\d+):(\d+)(?::(\d+))?")` 取最后一个匹配；`\d` 是 Unicode Nd，
/// 全角/阿拉伯-印度数字同样计秒。不中 → 0。
pub fn _dur_seconds(text: String) -> Result<i64, String> {
    dur_seconds_i64(&text)
}

/// `_dur_seconds` 的实现，供 `_video_from_item` 内部复用；溢出同报 OverflowError。
fn dur_seconds_i64(text: &str) -> Result<i64, String> {
    let m = pytext::dur_matches(text);
    let Some((h, mnt, sec)) = m.last() else {
        return Ok(0);
    };
    let (h, mnt): (i128, i128) = (
        h.parse().map_err(|_| overflow(h))?,
        mnt.parse().map_err(|_| overflow(mnt))?,
    );
    let total = match sec {
        Some(s) => {
            let s: i128 = s.parse().map_err(|_| overflow(s))?;
            let mnt60 = mnt.checked_mul(60).ok_or_else(|| overflow(text))?;
            h.checked_mul(3600)
                .and_then(|x| x.checked_add(mnt60))
                .and_then(|x| x.checked_add(s))
        }
        None => h.checked_mul(60).and_then(|x| x.checked_add(mnt)),
    };
    match total {
        Some(v) if v >= i64::MIN as i128 && v <= i64::MAX as i128 => Ok(v as i64),
        _ => Err(overflow(text)),
    }
}

fn overflow(text: &str) -> String {
    format!("OverflowError: int too large for the port: {text:?}")
}

/// `Item.param.isdigit()` → `int(param)`：Nd 全集解析（值可任意大，`$int` 承载）；
/// 含 No 字符（上标等）时原版 `int()` 抛 `ValueError`；非数字串 → 0。
fn parse_param(text: &str) -> Result<Value, String> {
    if pytext::is_digit(text) {
        if let Some(digits) = pytext::digit_string(text) {
            return Ok(pb::int_value_from_dec(&digits));
        }
        return Err(format!(
            "ValueError: invalid literal for int() with base 10: {text:?}"
        ));
    }
    Ok(json!(0))
}

/// Port of `_video_from_item` (`bilibili_cli.py:1280`). SearchItem → 视频卡 dict 或 None。
pub fn _video_from_item(item: Value) -> Result<Value, String> {
    let f = pb::fields_from_value(&item)?;
    let card = pb::pmsg(&f, 37)?;
    if card.is_empty() {
        return Ok(Value::Null);
    }
    let share = pb::pmsg(&card, 6)?;
    let video = pb::pmsg(&share, 2)?;
    let bvid = pb::ptext(&video, 1, "")?;
    let aid = parse_param(&pb::ptext(&f, 2, "")?)?;
    let title_src = pb::ptext(&card, 1, "")?;
    let title: String = pytext::py_strip(
        &title_src
            .replace("<em class=\"keyword\">", "")
            .replace("</em>", ""),
    )
    .into();
    let dur_text = pb::ptext(&card, 12, "")?;
    let dur_sec = dur_seconds_i64(&dur_text)?;

    let mut out = serde_json::Map::new();
    out.insert("kind".into(), json!("video"));
    out.insert("bvid".into(), json!(bvid));
    out.insert("aid".into(), aid);
    out.insert("title".into(), json!(title));
    out.insert("author".into(), json!(pb::ptext(&card, 10, "")?));
    out.insert("mid".into(), pb::int_value(pb::pint(&card, 14, 0)));
    out.insert("play".into(), pb::int_value(pb::pint(&card, 8, 0)));
    out.insert("danmaku".into(), pb::int_value(pb::pint(&card, 9, 0)));
    out.insert("duration_text".into(), json!(dur_text));
    out.insert("duration_sec".into(), json!(dur_sec));
    out.insert("pubdate".into(), pb::int_value(pb::pint(&card, 23, 0)));
    out.insert(
        "url".into(),
        if bvid.is_empty() {
            json!("")
        } else {
            json!(format!("https://www.bilibili.com/video/{bvid}"))
        },
    );
    out.insert("cover".into(), json!(pb::ptext(&card, 2, "")?));
    Ok(Value::Object(out))
}

/// Port of `_user_from_item` (`bilibili_cli.py:1310`). SearchItem → UP 主卡 dict 或 None。
///
/// mid 在 `Item.param`（字段 2）而不在卡里；`url` 用 param 原文（非数字也照拼）。
pub fn _user_from_item(item: Value) -> Result<Value, String> {
    let f = pb::fields_from_value(&item)?;
    let card = pb::pmsg(&f, 36)?;
    if card.is_empty() {
        return Ok(Value::Null);
    }
    let ov = pb::pmsg(&card, 8)?;
    let mid = pb::ptext(&f, 2, "")?;

    let mut out = serde_json::Map::new();
    out.insert("kind".into(), json!("user"));
    out.insert("mid".into(), parse_param(&mid)?);
    out.insert("name".into(), json!(pb::ptext(&card, 1, "")?));
    out.insert("fans".into(), pb::int_value(pb::pint(&card, 4, 0)));
    out.insert("archives".into(), pb::int_value(pb::pint(&card, 5, 0)));
    out.insert("level".into(), pb::int_value(pb::pint(&card, 14, 0)));
    out.insert("sign".into(), json!(pb::ptext(&card, 3, "")?));
    out.insert("official".into(), json!(pb::ptext(&ov, 2, "")?));
    out.insert(
        "url".into(),
        if mid.is_empty() {
            json!("")
        } else {
            json!(format!("https://space.bilibili.com/{mid}"))
        },
    );
    out.insert("avatar".into(), json!(pb::ptext(&card, 2, "")?));
    Ok(Value::Object(out))
}

/// `$bytes` 标记 → 字节。原版收 bytes；其余类型（str/int…）在 `_pread` 里本来就会
/// 抛 `TypeError`（`len()`/下标），port 同样以 `Err("TypeError: ...")` 对齐。
fn bytes_arg(v: &Value) -> Result<Vec<u8>, String> {
    match v {
        Value::Object(o) if o.len() == 1 && o.contains_key("$bytes") => {
            let arr = o["$bytes"].as_array().ok_or("TypeError: bad $bytes")?;
            let mut out = Vec::with_capacity(arr.len());
            for b in arr {
                out.push(
                    b.as_u64()
                        .filter(|&x| x <= 255)
                        .ok_or("TypeError: bad $bytes")? as u8,
                );
            }
            Ok(out)
        }
        _ => Err("TypeError: buf 必须是 bytes".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negatives_refuse_instead_of_hanging() {
        // 原版死循环（record 实测超时跳过）；port 必须显式 Err
        assert!(_pv(-1).is_err());
        assert!(_pi(2, -1).is_err());
        assert!(_pi(-2, 1).is_err());
        assert!(_ps(-1, "x".into()).is_err());
        assert!(_pm(-1, json!({ "$bytes": [8, 1] })).is_err());
    }

    #[test]
    fn varint_u64_domain_roundtrip() {
        // tag varint 到 u64 域上沿（2^64-1 的字段值）原样可读
        let mut buf = pv_i128(8); // field 1, wiretype 0
        buf.extend(pv_i128(u64::MAX as i128));
        let out = pb::pread(&buf).expect("pread");
        assert_eq!(
            pb::enc_normalize(&pb::fields_to_value(&out)),
            json!({
                "$dict": [[1, [{"$tuple": [0, {"$int": "18446744073709551615"}]}]]]
            })
        );
    }

    #[test]
    fn param_no_digit_raises_like_int() {
        let item = json!({
            "$dict": [
                [2, [{"$tuple": [2, {"$bytes": "²".as_bytes()}]}]],
                [37, [{"$tuple": [2, {"$bytes": [10, 1, 84]}]}]],
            ]
        });
        let err = _video_from_item(item).unwrap_err();
        assert!(
            err.starts_with("ValueError:"),
            "原版 int('²') 同抛 ValueError: {err}"
        );
    }
}
