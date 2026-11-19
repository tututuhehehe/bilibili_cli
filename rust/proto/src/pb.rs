//! protobuf wire 值在 port 内部的表示，以及与 bridge 标记（`$bytes`/`$dict`/`$tuple`/`$int`）
//! 的互转。原版 `_pread` 的值域是任意精度 int：内部用 i128 承载（wire 上 varint 在
//! shift>70 报错前最多贡献 ~2^77 位，i128 全覆盖），超出 u64 的值走 `$int` 十进制标记，
//! 过桥后仍是精确的 Python 大整数。

#![forbid(unsafe_code)]

use serde_json::{Map, Value, json};

/// 2^53：bridge 的 `$int` 标记在此之上启用（driver `enc` 同阈值，两侧各自过一遍 Python
/// int 后规范化一致，port 只需保证数值精确）。
const BIG: i128 = 9_007_199_254_740_992;

/// wire 字段的值：varint（int）或定长/长度前缀字节。
#[derive(Clone)]
pub enum PVal {
    Int(i128),
    Bytes(Vec<u8>),
}

/// `_pread` 的返回结构：{字段号(int 键): [(wiretype, 值)…]}，键按首次出现排序。
#[derive(Default)]
pub struct Fields {
    entries: Vec<(i128, Vec<(i128, PVal)>)>,
}

impl Fields {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `fields.get(num)`；空列表（原版 `if not lst` 的 falsy）视同缺失，返回 None。
    pub fn get(&self, num: i128) -> Option<&[(i128, PVal)]> {
        self.entries
            .iter()
            .find(|(k, lst)| *k == num && !lst.is_empty())
            .map(|(_, lst)| lst.as_slice())
    }

    /// `out.setdefault(f, []).append((wt, v))`。
    pub fn push(&mut self, f: i128, wt: i128, v: PVal) {
        match self.entries.iter_mut().find(|(k, _)| *k == f) {
            Some((_, lst)) => lst.push((wt, v)),
            None => self.entries.push((f, vec![(wt, v)])),
        }
    }
}

/// varint 读一个：越界 `IndexError`，shift>70 `ValueError`（原版 11 字节含续位即报）。
pub fn rv(buf: &[u8], mut pos: usize) -> Result<(i128, usize), String> {
    let mut r: i128 = 0;
    let mut shift: u32 = 0;
    loop {
        let Some(&c) = buf.get(pos) else {
            return Err("IndexError: index out of range".into());
        };
        pos += 1;
        r |= ((c & 0x7F) as i128) << shift;
        if c & 0x80 == 0 {
            return Ok((r, pos));
        }
        shift += 7;
        if shift > 70 {
            return Err("ValueError: varint 过长".into());
        }
    }
}

/// `_pread`（`bilibili_cli.py:1150`）：严格 wire 循环。wt=1/5 截断不报错（Python 切片语义）。
pub fn pread(buf: &[u8]) -> Result<Fields, String> {
    let mut out = Fields::default();
    let n = buf.len();
    let mut i = 0usize;
    while i < n {
        let (key, next) = rv(buf, i)?;
        i = next;
        let f = key >> 3;
        let wt = key & 7;
        let v = match wt {
            0 => {
                let (v, next) = rv(buf, i)?;
                i = next;
                PVal::Int(v)
            }
            2 => {
                let (ln, next) = rv(buf, i)?;
                i = next;
                if ln > (n - i) as i128 {
                    return Err(format!("ValueError: 字段 {f} 越界"));
                }
                let ln = ln as usize;
                let v = PVal::Bytes(buf[i..i + ln].to_vec());
                i += ln;
                v
            }
            5 => {
                let end = (i + 4).min(n);
                let v = PVal::Bytes(buf[i..end].to_vec());
                i += 4;
                v
            }
            1 => {
                let end = (i + 8).min(n);
                let v = PVal::Bytes(buf[i..end].to_vec());
                i += 8;
                v
            }
            _ => return Err(format!("ValueError: 未知 wiretype {wt}")),
        };
        out.push(f, wt, v);
    }
    Ok(out)
}

/// 任意精度 int → bridge Value：i64 域内普通数字，u64 域普通无符号数字，再往上 `$int`。
pub fn int_value(v: i128) -> Value {
    if v >= i64::MIN as i128 && v <= i64::MAX as i128 {
        json!(v as i64)
    } else if v >= 0 && v <= u64::MAX as i128 {
        json!(v as u64)
    } else {
        json!({ "$int": v.to_string() })
    }
}

/// Value → i128：普通数字或 `$int` 标记。
pub fn as_int(v: &Value) -> Option<i128> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i as i128)
            } else {
                n.as_u64().map(|u| u as i128)
            }
        }
        Value::Object(o) if o.len() == 1 => o.get("$int")?.as_str()?.parse().ok(),
        _ => None,
    }
}

fn pval_to_value(v: &PVal) -> Value {
    match v {
        PVal::Int(i) => int_value(*i),
        PVal::Bytes(b) => json!({ "$bytes": b }),
    }
}

fn pval_from_value(v: &Value) -> Result<PVal, String> {
    match v {
        Value::Object(o) if o.len() == 1 && o.contains_key("$bytes") => {
            let bytes = o["$bytes"].as_array().ok_or("TypeError: bad $bytes")?;
            let mut out = Vec::with_capacity(bytes.len());
            for b in bytes {
                out.push(
                    b.as_u64()
                        .filter(|&x| x <= 255)
                        .ok_or("TypeError: bad $bytes")? as u8,
                );
            }
            Ok(PVal::Bytes(out))
        }
        other => as_int(other)
            .map(PVal::Int)
            .ok_or_else(|| "TypeError: wire 值必须是 int 或 bytes".into()),
    }
}

/// Python `fields` dict（过桥后 `$dict` 标记或空对象）→ `Fields`。
pub fn fields_from_value(v: &Value) -> Result<Fields, String> {
    let pairs: Vec<(Value, &Value)> = match v {
        Value::Object(o) if o.is_empty() => Vec::new(),
        Value::Object(o) if o.len() == 1 && o.contains_key("$dict") => o["$dict"]
            .as_array()
            .ok_or("TypeError: bad $dict")?
            .iter()
            .map(|pair| {
                let arr = pair.as_array().ok_or("TypeError: bad $dict")?;
                let (k, val) = match arr.as_slice() {
                    [k, val] => (k.clone(), val),
                    _ => return Err("TypeError: bad $dict".into()),
                };
                Ok((k, val))
            })
            .collect::<Result<_, String>>()?,
        Value::Object(_) => Vec::new(), // 纯字符串键 dict：int 键取不到，等价空 fields
        _ => return Err("TypeError: fields 必须是 dict".into()),
    };

    let mut out = Fields::default();
    for (k, val) in pairs {
        let key = as_int(&k).ok_or("TypeError: 字段号必须是 int")?;
        let lst = val.as_array().ok_or("TypeError: 字段值必须是 list")?;
        for entry in lst {
            let arr = match entry {
                Value::Object(o) if o.len() == 1 && o.contains_key("$tuple") => {
                    o["$tuple"].as_array().ok_or("TypeError: bad $tuple")?
                }
                _ => return Err("TypeError: 条目必须是 tuple".into()),
            };
            let (wt, v) = match arr.as_slice() {
                [wt, v] => (wt, v),
                _ => return Err("TypeError: 解包需要 2 元 tuple".into()),
            };
            let wt = as_int(wt).ok_or("TypeError: wiretype 必须是 int")?;
            out.push(key, wt, pval_from_value(v)?);
        }
    }
    Ok(out)
}

/// `Fields` → `$dict` 标记（键与 (wt, 值) 元组按插入序，过桥还原 Python dict）。
pub fn fields_to_value(f: &Fields) -> Value {
    let pairs: Vec<Value> = f
        .entries
        .iter()
        .map(|(k, lst)| {
            let items: Vec<Value> = lst
                .iter()
                .map(|(wt, v)| json!({ "$tuple": [int_value(*wt), pval_to_value(v)] }))
                .collect();
            json!([int_value(*k), items])
        })
        .collect();
    json!({ "$dict": pairs })
}

/// `_ptext`（`bilibili_cli.py:1190`）。
pub fn ptext(f: &Fields, num: i64, default: &str) -> Result<String, String> {
    let Some(lst) = f.get(num as i128) else {
        return Ok(default.into());
    };
    let (wt, v) = &lst[0];
    if *wt != 2 {
        return Ok(default.into());
    }
    match v {
        PVal::Bytes(b) => Ok(crate::pytext::decode_replace(b)),
        PVal::Int(i) => Err(format!(
            "AttributeError: 'int' object has no attribute 'decode' (value {i})"
        )),
    }
}

/// `_pint`（`bilibili_cli.py:1198`）的内部版：i128 全域（bridge 版再收窄到 i64）。
pub fn pint(f: &Fields, num: i64, default: i128) -> i128 {
    match f.get(num as i128) {
        Some([(wt, PVal::Int(v)), ..]) if *wt == 0 => *v,
        _ => default,
    }
}

/// `_pmsg`（`bilibili_cli.py:1206`）。
pub fn pmsg(f: &Fields, num: i64) -> Result<Fields, String> {
    match f.get(num as i128) {
        Some([(2, v), ..]) => match v {
            PVal::Bytes(b) => pread(b),
            PVal::Int(i) => Err(format!(
                "TypeError: object of type 'int' has no len() (value {i})"
            )),
        },
        _ => Ok(Fields::default()),
    }
}

/// `_prep`（`bilibili_cli.py:1213`）：wt=2 的值原样透传（int 也照传，同原版）。
pub fn prep(f: &Fields, num: i64) -> Vec<Value> {
    f.get(num as i128)
        .map(|lst| {
            lst.iter()
                .filter(|(wt, _)| *wt == 2)
                .map(|(_, v)| pval_to_value(v))
                .collect()
        })
        .unwrap_or_default()
}

/// 纯十进制数字串 → 任意精度 int Value（`int(s)`）：i128 装不下时走 `$int` 十进制
/// （去前导零后），过桥仍是精确值。
pub fn int_value_from_dec(digits: &str) -> Value {
    if digits.bytes().all(|b| b == b'0') {
        return json!(0);
    }
    match digits.parse::<i128>() {
        Ok(v) => int_value(v),
        Err(_) => json!({ "$int": digits.trim_start_matches('0') }),
    }
}

/// bridge `serde_json::Number` 归一化：i64/u64 大数在 driver `enc` 侧会变成 `$int`；
/// 测试直接比 Value 时先按同一阈值归一（`$int`/`$bytes` 标记内容不变）。
pub fn enc_normalize(v: &Value) -> Value {
    match v {
        Value::Number(n) => {
            let big = match n.as_i64() {
                Some(i) => i.unsigned_abs() as i128 > BIG || (i as i128) < -BIG,
                None => n.as_u64().is_some_and(|u| u as i128 > BIG),
            };
            if big {
                json!({ "$int": n.to_string() })
            } else {
                v.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(enc_normalize).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), enc_normalize(x)))
                .collect::<Map<String, Value>>(),
        ),
        _ => v.clone(),
    }
}
