//! sign 模块移植：appkey 签名 + cookie 处理 + curl jar 解析。
//! 实现前必读 rust/tasks/00-shared-rules.md 与 rust/tasks/sign.md。
#![forbid(unsafe_code)]

/// 纯 Rust MD5（源码用 `hashlib.md5`；依赖纪律禁新增 crate，见 md5.rs 头注释）。
#[path = "md5.rs"]
mod md5;

use serde_json::{Map, Value, json};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// `bilibili_cli.py:55` 的 `APP_KEY`。
const APP_KEY: &str = "1d8b6e7d45233436";
/// `bilibili_cli.py:55` 的 `APP_SEC`。
const APP_SEC: &str = "560c52ccd288fed045859ed18bffd973";

// ------------------------------------------------------------------ 基础：Python 值的 str()/repr()

/// 单键 `$xxx` 结构标记（对照 src/pyconv.rs / python shadow 的 `_enc`/`_dec`）。
fn marker(v: &Value) -> Option<(&str, &Value)> {
    let m = v.as_object()?;
    if m.len() != 1 {
        return None;
    }
    let (k, inner) = m.iter().next()?;
    if k.starts_with('$') {
        Some((k.as_str(), inner))
    } else {
        None
    }
}

/// Python 浮点 `str()`/`repr()`（最短往返表示；`1e-05` 两位指数、整数值补 `.0`）。
fn py_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x.is_sign_positive() {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let neg = x.is_sign_negative();
    let ax = if neg { -x } else { x };
    if ax == 0.0 {
        return if neg { "-0.0".into() } else { "0.0".into() };
    }
    // Rust LowerExp 给出与 Python repr 相同的最短往返数字，尾数归一到 [1,10)。
    let sci = format!("{ax:e}");
    let Some((mant, exp)) = sci.split_once('e') else {
        return sci;
    };
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    if digits.is_empty() {
        return sci;
    }
    let exp: i32 = exp.parse().unwrap_or(0);
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    // Python repr：定点/科学计数法切换位在 (-4, 16)。
    if !(-4..16).contains(&exp) {
        // 科学计数法：d.ddde±NN（指数至少两位）。
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        if exp.abs() < 10 {
            out.push('0');
        }
        out.push_str(&exp.abs().to_string());
    } else if exp >= 0 {
        let point = exp as usize + 1;
        if digits.len() > point {
            out.push_str(&digits[..point]);
            out.push('.');
            out.push_str(&digits[point..]);
        } else {
            out.push_str(&digits);
            out.push_str(&"0".repeat(point - digits.len()));
            out.push_str(".0");
        }
    } else {
        out.push_str("0.");
        out.push_str(&"0".repeat((-exp - 1) as usize));
        out.push_str(&digits);
    }
    out
}

/// Python `repr("...")` 的引号规则：默认单引号；含 `'` 不含 `"` 时用双引号；控制字符转义。
fn py_str_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python `repr(b"...")`（可见 ASCII 原样，`\n\r\t` 转义，其余 `\xNN`）。
fn py_bytes_repr(bytes: &[u8]) -> String {
    let quote = if bytes.contains(&b'\'') && !bytes.contains(&b'"') {
        b'"'
    } else {
        b'\''
    };
    let mut out = String::from("b");
    out.push(quote as char);
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            b if b == quote => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            b => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out.push(quote as char);
    out
}

/// 容器元素的 `repr()`（`str()` 与 `repr()` 仅字符串不同）。
fn py_repr(v: &Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(py_str_repr(s)),
        _ => py_str(v),
    }
}

/// Python `str(v)`，输入是 pyconv.rs 的 JSON 编码。
///
/// 未录制路径上的尽力等价：标量（str/int/float/bool/None）逐字节对齐；
/// 容器（list/tuple/dict/bytes/set）按 repr 规则生成。
fn py_str(v: &Value) -> Result<String, String> {
    let sep = |parts: Vec<String>| parts.join(", ");
    match v {
        Value::Null => Ok("None".into()),
        Value::Bool(b) => Ok(if *b { "True".into() } else { "False".into() }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_string())
            } else if let Some(u) = n.as_u64() {
                Ok(u.to_string())
            } else {
                Ok(py_float_repr(n.as_f64().unwrap_or_default()))
            }
        }
        Value::String(s) => Ok(s.clone()),
        Value::Array(items) => {
            let parts: Result<Vec<String>, _> = items.iter().map(py_repr).collect();
            Ok(format!("[{}]", sep(parts?)))
        }
        Value::Object(_) => match marker(v) {
            Some(("$num", inner)) => Ok(match inner.as_str() {
                Some("NaN") => "nan".into(),
                Some("Infinity") => "inf".into(),
                Some("-Infinity") => "-inf".into(),
                _ => "-0.0".into(), // {"$num": "-0"}
            }),
            Some(("$int", inner)) => Ok(inner.as_str().unwrap_or_default().to_string()),
            Some(("$tuple", Value::Array(items))) => {
                let parts: Result<Vec<String>, _> = items.iter().map(py_repr).collect();
                let joined = sep(parts?);
                Ok(if items.len() == 1 {
                    format!("({joined},)")
                } else {
                    format!("({joined})")
                })
            }
            Some(("$bytes", Value::Array(items))) => {
                let mut b = Vec::with_capacity(items.len());
                for it in items {
                    b.push(it.as_u64().unwrap_or_default() as u8);
                }
                Ok(py_bytes_repr(&b))
            }
            Some(("$set", Value::Array(items))) => {
                if items.is_empty() {
                    return Ok("set()".into());
                }
                let parts: Result<Vec<String>, _> = items.iter().map(py_repr).collect();
                Ok(format!("{{{}}}", sep(parts?)))
            }
            Some(("$dict", Value::Array(pairs))) => {
                let mut parts = Vec::new();
                for p in pairs.iter() {
                    if let Value::Array(kv) = p
                        && kv.len() == 2
                    {
                        parts.push(format!("{}: {}", py_repr(&kv[0])?, py_repr(&kv[1])?));
                    }
                }
                Ok(format!("{{{}}}", parts.join(", ")))
            }
            _ => {
                // 普通 dict（字符串键）：{'a': 1, 'b': 2}，插入序。
                let m = match v.as_object() {
                    Some(m) => m,
                    None => return Ok("{}".into()),
                };
                let mut parts = Vec::new();
                for (k, val) in m {
                    parts.push(format!("{}: {}", py_str_repr(k), py_repr(val)?));
                }
                Ok(format!("{{{}}}", parts.join(", ")))
            }
        },
    }
}

/// Python 类型名（错误信息用）。
fn py_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.as_i64().is_some() || n.as_u64().is_some() {
                "int"
            } else {
                "float"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => match marker(v) {
            Some(("$tuple", _)) => "tuple",
            Some(("$bytes", _)) => "bytes",
            Some(("$set", _)) => "set",
            _ => "dict",
        },
    }
}

// ------------------------------------------------------------------ 基础：dict 语义

/// Python `dict(params)`：保持插入序的 (key, value) 对。
///
/// - 对象 / `$dict` 标记 → 映射拷贝；
/// - list / tuple / set 标记 → 逐元素取 (k, v) 二元组，重复键后者覆盖前者、位置不变；
/// - 其余按源码语义返回 Err（TypeError / ValueError，文案对齐 CPython）。
fn dict_pairs(params: &Value) -> Result<Vec<(Value, Value)>, String> {
    let items: Vec<Value> = match params {
        Value::Object(m) => {
            if let Some(Value::Array(pairs)) = m.get("$dict").filter(|_| m.len() == 1) {
                let mut out: Vec<(Value, Value)> = Vec::new();
                for p in pairs.iter() {
                    let (k, v) = pair_item(p)?;
                    set_pair(&mut out, k, v);
                }
                return Ok(out);
            }
            let mut out = Vec::with_capacity(m.len());
            for (k, v) in m {
                out.push((Value::String(k.clone()), v.clone()));
            }
            return Ok(out);
        }
        Value::Array(items) => items.clone(),
        v if marker(v).is_some_and(|(k, _)| matches!(k, "$tuple" | "$set")) => marker(v)
            .and_then(|(_, inner)| inner.as_array().cloned())
            .unwrap_or_default(),
        Value::String(s) => {
            // dict("abc") 逐字符迭代：首个元素长度 1 → ValueError；dict("") → {}。
            if s.is_empty() {
                return Ok(Vec::new());
            }
            return Err(
                "ValueError: dictionary update sequence element #0 has length 1; 2 is required"
                    .into(),
            );
        }
        v if marker(v).is_some_and(|(k, _)| k == "$bytes") => {
            // dict(b"...") 迭代出 int 元素；空 bytes → {}。
            let items = marker(v)
                .and_then(|(_, inner)| inner.as_array())
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                return Ok(Vec::new());
            }
            return Err(
                "TypeError: cannot convert dictionary update sequence element #0 to a sequence"
                    .into(),
            );
        }
        v => {
            return Err(format!(
                "TypeError: '{}' object is not iterable",
                py_type_name(v)
            ));
        }
    };
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match pair_item(item) {
            Ok((k, v)) => set_pair(&mut out, k, v),
            Err(e) => return Err(retry_element_err(e, i)),
        }
    }
    Ok(out)
}

/// `dict()` 的单个元素 → (k, v)。错误文案先按 #0 生成，由调用方按真实下标改写。
fn pair_item(item: &Value) -> Result<(Value, Value), String> {
    let seq: Option<Vec<Value>> = match item {
        Value::Array(kv) => Some(kv.clone()),
        v if marker(v).is_some_and(|(k, _)| k == "$tuple") => {
            marker(v).and_then(|(_, inner)| inner.as_array().cloned())
        }
        Value::String(s) => Some(s.chars().map(|c| Value::String(c.into())).collect()),
        _ => None,
    };
    match seq {
        Some(kv) if kv.len() == 2 => Ok((kv[0].clone(), kv[1].clone())),
        Some(kv) => Err(format!(
            "ValueError: dictionary update sequence element #0 has length {}; 2 is required",
            kv.len()
        )),
        None => Err(
            "TypeError: cannot convert dictionary update sequence element #0 to a sequence".into(),
        ),
    }
}

/// 把 `#0` 形式的错误文案替换成真实元素下标。
fn retry_element_err(e: String, i: usize) -> String {
    e.replacen("#0", &format!("#{i}"), 1)
}

/// `p[k] = v`：已有键原位覆盖（保持首次出现的位置），否则追加到末尾。
fn set_pair(pairs: &mut Vec<(Value, Value)>, k: Value, v: Value) {
    for (ek, ev) in pairs.iter_mut() {
        if *ek == k {
            *ev = v;
            return;
        }
    }
    pairs.push((k, v));
}

// ------------------------------------------------------------------ 基础：sorted(p) 的键序

/// Python 键比较类别：str 之间按码点序；int/bool 精确数值序；float 参与数值比较。
enum KeyClass {
    Str(String),
    Num(f64),
}

fn key_class(v: &Value) -> Option<KeyClass> {
    match v {
        Value::String(s) => Some(KeyClass::Str(s.clone())),
        Value::Bool(b) => Some(KeyClass::Num(if *b { 1.0 } else { 0.0 })),
        Value::Number(n) => n.as_f64().map(KeyClass::Num),
        _ => None,
    }
}

/// `sorted(p)` 的等价排序；混合不可比较类型 → TypeError（文案对齐 CPython）。
fn sorted_order(pairs: &[(Value, Value)]) -> Result<Vec<usize>, String> {
    let mut idx: Vec<usize> = (0..pairs.len()).collect();
    let mut err: Option<String> = None;
    idx.sort_by(|&a, &b| {
        if err.is_some() {
            return std::cmp::Ordering::Equal;
        }
        match (key_class(&pairs[a].0), key_class(&pairs[b].0)) {
            (Some(KeyClass::Str(x)), Some(KeyClass::Str(y))) => x.as_str().cmp(y.as_str()),
            (Some(KeyClass::Num(x)), Some(KeyClass::Num(y))) => {
                x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
            }
            (x, y) => {
                let left = match x {
                    Some(KeyClass::Str(_)) => "str",
                    _ => py_type_name(&pairs[a].0),
                };
                let right = match y {
                    Some(KeyClass::Str(_)) => "str",
                    _ => py_type_name(&pairs[b].0),
                };
                err = Some(format!(
                    "TypeError: '<' not supported between instances of '{left}' and '{right}'"
                ));
                std::cmp::Ordering::Equal
            }
        }
    });
    match err {
        Some(e) => Err(e),
        None => Ok(idx),
    }
}

// ------------------------------------------------------------------ 基础：urlencode / md5

/// Python `urllib.parse.quote_plus(str(x), safe='')`：
/// 字母数字与 `_.-~` 原样，空格 → `+`，其余 UTF-8 字节 `%XX`（大写）。
fn quote_plus(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `urllib.parse.urlencode(p)`：插入序（不排序），`str(k)` 与 `str(v)` 逐对编码。
fn urlencode(pairs: &[(Value, Value)]) -> Result<String, String> {
    let mut parts = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        parts.push(format!(
            "{}={}",
            quote_plus(&py_str(k)?),
            quote_plus(&py_str(v)?)
        ));
    }
    Ok(parts.join("&"))
}

// ------------------------------------------------------------------ 签名核心

/// `sign_app` 的固定时钟核心（任务书要求的可注入 ts 私有函数）。
///
/// 等价源码：
/// ```python
/// p = dict(params); p["appkey"] = key; p["ts"] = ts
/// q = "&".join(f"{k}={p[k]}" for k in sorted(p))
/// sign = md5((q + sec).encode()).hexdigest(); p["sign"] = sign
/// return urlencode(p)   # 插入序，非排序序
/// ```
/// 返回 `(q, md5, urlencoded)`，供固定 ts 基准逐字节校验。
fn sign_with_ts(
    params: &Value,
    key: &Value,
    sec: &str,
    ts: i64,
) -> Result<(String, String, String), String> {
    let mut p = dict_pairs(params)?;
    set_pair(&mut p, json!("appkey"), key.clone());
    set_pair(&mut p, json!("ts"), json!(ts));

    let order = sorted_order(&p)?;
    let mut q = String::new();
    for (n, &i) in order.iter().enumerate() {
        if n > 0 {
            q.push('&');
        }
        q.push_str(&py_str(&p[i].0)?);
        q.push('=');
        q.push_str(&py_str(&p[i].1)?);
    }

    let sign = md5::hex_digest(format!("{q}{sec}").as_bytes());
    set_pair(&mut p, json!("sign"), json!(sign.clone()));

    Ok((q, sign, urlencode(&p)?))
}

// ------------------------------------------------------------------ 导出函数

/// Port of `sign_app` (`bilibili_cli.py:239`). 参数 + appkey + ts → md5 签名 query。
///
/// 源码内部读 `time.time()`，输出随时间变化，**无法录制比对**：
/// 等价性由 tests 里的固定时钟基准（`fixtures/sign_fixed_ts.json`）校验——
/// 签名核心拆成可注入 ts 的 `sign_with_ts`，本函数只取当前时钟后调用它。
/// **运行时时钟路径未录制，由固定 ts 基准覆盖。**
///
/// `key` / `sec` 缺省（None）时用源码默认 `APP_KEY` / `APP_SEC`（bilibili_cli.py:55）。
pub fn sign_app(params: Value, key: Option<Value>, sec: Option<Value>) -> Result<String, String> {
    let key = key.unwrap_or_else(|| json!(APP_KEY));
    let sec = match sec {
        Some(Value::String(s)) => s,
        Some(other) => {
            return Err(format!(
                "TypeError: can only concatenate str (not \"{}\") to str",
                py_type_name(&other)
            ));
        }
        None => APP_SEC.to_string(),
    };
    let ts = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };
    let (_, _, encoded) = sign_with_ts(&params, &key, &sec, ts)?;
    Ok(encoded)
}

/// Port of `cookie_header` (`bilibili_cli.py:271`). creds dict → Cookie 头（无则空串）。
///
/// 源码：`c.get("cookie") or ""`。非字符串真值在源码里会原样返回；
/// 受导出类型（string）约束，此处返回其 `str(v)`（见汇报「已知差异」）。
pub fn cookie_header(c: Value) -> Result<String, String> {
    let v = match dict_get(&c, "cookie")? {
        Some(v) => v,
        None => return Ok(String::new()),
    };
    if is_falsy(&v) {
        return Ok(String::new());
    }
    match v {
        Value::String(s) => Ok(s),
        other => py_str(&other),
    }
}

/// `c.get(k)`：对象或 `$dict` 标记（非字符串键的 dict）里查字符串键。
fn dict_get(c: &Value, key: &str) -> Result<Option<Value>, String> {
    match c {
        Value::Object(m) => {
            if let Some(Value::Array(pairs)) = m.get("$dict").filter(|_| m.len() == 1) {
                for p in pairs.iter() {
                    if let Value::Array(kv) = p
                        && kv.len() == 2
                        && let Value::String(k) = &kv[0]
                        && k == key
                    {
                        return Ok(Some(kv[1].clone()));
                    }
                }
                return Ok(None);
            }
            Ok(m.get(key).cloned())
        }
        v => Err(format!(
            "AttributeError: '{}' object has no attribute 'get'",
            py_type_name(v)
        )),
    }
}

/// Python 真值判定（`x or ""`）：仅假值走空串分支。
fn is_falsy(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !*b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f == 0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(m) => match marker(v) {
            Some(("$num", inner)) => matches!(inner.as_str(), Some("-0")),
            Some((_, Value::Array(items))) => items.is_empty(),
            _ => m.is_empty(),
        },
    }
}

/// Port of `_read_jar` (`bilibili_cli.py:352`). 解析 curl cookie jar 文本 → cookie dict。
///
/// 注意 `#HttpOnly_` 前缀行必须剥前缀而不是当注释跳过，否则丢 SESSDATA；
/// 过期字段、域名列按源码语义一概忽略（只取 parts[5]/parts[6]）。
pub fn _read_jar(path: String) -> Result<Value, String> {
    let mut pairs: Map<String, Value> = Map::new();
    if !Path::new(&path).exists() {
        return Ok(Value::Object(pairs));
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return Err(io_error(&e, &path)),
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => return Err(utf8_error(&e.utf8_error(), e.as_bytes())),
    };
    // open(encoding="utf-8") 默认 universal newlines：\r\n 与单独 \r 都按 \n 处理。
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    for raw in text.split('\n') {
        // for raw in f: 每行带 \n；split('\n') 等价剥掉，rstrip("\n") 语义一致。
        if line_blank(raw) {
            continue;
        }
        let line = match raw.strip_prefix("#HttpOnly_") {
            Some(rest) => rest,
            None if raw.starts_with('#') => continue,
            None => raw,
        };
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 7 {
            pairs.insert(parts[5].to_string(), json!(parts[6]));
        }
    }
    Ok(Value::Object(pairs))
}

/// Python `str.strip()` 的空白集（含 0x1c-0x1f、0x85，Rust trim 缺这几个）。
fn line_blank(line: &str) -> bool {
    line.chars().all(|c| {
        matches!(c,
            '\u{9}'..='\u{d}' | '\u{1c}'..='\u{1f}' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}'
            | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
    })
}

/// open() 的 OSError 文案（未录制路径，按 CPython 消息格式对齐）。
fn io_error(e: &std::io::Error, path: &str) -> String {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => {
            format!("FileNotFoundError: [Errno 2] No such file or directory: '{path}'")
        }
        ErrorKind::PermissionDenied => {
            format!("PermissionError: [Errno 13] Permission denied: '{path}'")
        }
        ErrorKind::IsADirectory => {
            format!("IsADirectoryError: [Errno 21] Is a directory: '{path}'")
        }
        _ => format!("OSError: {e}"),
    }
}

/// UnicodeDecodeError 文案（含 Python 的 invalid start byte / continuation byte 判别）。
fn utf8_error(e: &std::str::Utf8Error, bytes: &[u8]) -> String {
    let pos = e.valid_up_to();
    let byte = bytes.get(pos).copied().unwrap_or_default();
    let why = match e.error_len() {
        None => "unexpected end of data",
        Some(_) if matches!(byte, 0xc2..=0xf4) => "invalid continuation byte",
        Some(_) => "invalid start byte",
    };
    format!("UnicodeError: 'utf-8' codec can't decode byte 0x{byte:02x} in position {pos}: {why}")
}

// ------------------------------------------------------------------ 固定时钟基准 + 用例测试

#[cfg(test)]
mod tests {
    use super::*;

    /// fixtures/sign_fixed_ts.json：6 条 ts=1700000000 基准，q / md5 / urlencoded 逐字节比对。
    #[test]
    fn fixed_ts_baseline() -> Result<(), String> {
        let f: Value = serde_json::from_str(include_str!("../fixtures/sign_fixed_ts.json"))
            .map_err(|e| e.to_string())?;
        let key = f["app_key"]
            .as_str()
            .ok_or("fixtures 缺 app_key")?
            .to_string();
        let sec = f["app_sec"]
            .as_str()
            .ok_or("fixtures 缺 app_sec")?
            .to_string();
        let ts = f["fixed_ts"].as_i64().ok_or("fixtures 缺 fixed_ts")?;
        let cases = f["cases"].as_array().ok_or("fixtures 缺 cases")?;
        for (i, case) in cases.iter().enumerate() {
            let params = case["params"].clone();
            let (q, sign, enc) = sign_with_ts(&params, &json!(key), &sec, ts)?;
            assert_eq!(q, case["q"].as_str().unwrap_or_default(), "case {i}: q");
            assert_eq!(
                sign,
                case["md5"].as_str().unwrap_or_default(),
                "case {i}: md5"
            );
            assert_eq!(
                enc,
                case["urlencoded"].as_str().unwrap_or_default(),
                "case {i}: urlencoded"
            );
        }
        Ok(())
    }

    /// 任务书要求补充的用例：int/float 字符串化（Python repr 一致）、大整数标记、
    /// `-0.0`/`True`/`None` 标记、urlencode 特殊字符。
    /// 期望值由原版逻辑（ts 固定 1700000000）离线生成。
    #[test]
    fn extra_stringification_and_markers() -> Result<(), String> {
        let key = json!("1d8b6e7d45233436");
        let sec = "560c52ccd288fed045859ed18bffd973";
        let cases: Vec<(Value, &str, &str, &str)> = vec![
            (
                json!({"n": 42, "f": 1.5}),
                "appkey=1d8b6e7d45233436&f=1.5&n=42&ts=1700000000",
                "bcc1d79db6840a9f64eaa729691ecfdb",
                "n=42&f=1.5&appkey=1d8b6e7d45233436&ts=1700000000&sign=bcc1d79db6840a9f64eaa729691ecfdb",
            ),
            (
                json!({"a": 1e20, "b": 0.1, "c": -2.5, "d": 1e-05, "e": 123456789012345678.0,
                       "g": 100.0, "h": 0.0001, "i": 1e16, "j": 1e15}),
                "a=1e+20&appkey=1d8b6e7d45233436&b=0.1&c=-2.5&d=1e-05&e=1.2345678901234568e+17&g=100.0&h=0.0001&i=1e+16&j=1000000000000000.0&ts=1700000000",
                "28947c577139bab33579b609ab55c241",
                "a=1e%2B20&b=0.1&c=-2.5&d=1e-05&e=1.2345678901234568e%2B17&g=100.0&h=0.0001&i=1e%2B16&j=1000000000000000.0&appkey=1d8b6e7d45233436&ts=1700000000&sign=28947c577139bab33579b609ab55c241",
            ),
            (
                // 2**60：pyconv 走 $int 标记，str() 与 Python 大整数一致
                json!({"big": {"$int": "1152921504606846976"}}),
                "appkey=1d8b6e7d45233436&big=1152921504606846976&ts=1700000000",
                "e43190b17b3eea6e2d700f9477f579b6",
                "big=1152921504606846976&appkey=1d8b6e7d45233436&ts=1700000000&sign=e43190b17b3eea6e2d700f9477f579b6",
            ),
            (
                json!({"neg0": {"$num": "-0"}, "t": true, "nl": null, "s": "a b&c=d~e.f_g-h"}),
                "appkey=1d8b6e7d45233436&neg0=-0.0&nl=None&s=a b&c=d~e.f_g-h&t=True&ts=1700000000",
                "73da55c109591083374627e116c111ae",
                "neg0=-0.0&t=True&nl=None&s=a+b%26c%3Dd~e.f_g-h&appkey=1d8b6e7d45233436&ts=1700000000&sign=73da55c109591083374627e116c111ae",
            ),
        ];
        for (params, want_q, want_sign, want_enc) in cases {
            let (q, sign, enc) = sign_with_ts(&params, &key, sec, 1700000000)?;
            assert_eq!(q, want_q, "q of {params}");
            assert_eq!(sign, want_sign, "sign of {params}");
            assert_eq!(enc, want_enc, "urlencoded of {params}");
        }
        Ok(())
    }

    /// Python 浮点 repr 陷阱专项：定点/科学计数法切换位、`.0` 后缀、指数两位。
    /// 期望字符串逐一与 CPython str() 核对。
    #[test]
    fn python_float_repr() {
        for (x, want) in [
            (1.5, "1.5"),
            (1e20, "1e+20"),
            (0.1, "0.1"),
            (-2.5, "-2.5"),
            (1e-05, "1e-05"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (100.0, "100.0"),
            (0.0001, "0.0001"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (9999999999999998.0, "9999999999999998.0"),
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (7.0, "7.0"),
            (2.5e-10, "2.5e-10"),
        ] {
            assert_eq!(py_float_repr(x), want, "repr({x})");
        }
    }

    /// MD5 已知向量（含空串、多块、55/56/63/64/119/120 字节 padding 边界）。
    #[test]
    fn md5_known_vectors() {
        for (input, want) in [
            (&b""[..], "d41d8cd98f00b204e9800998ecf8427e"),
            (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
            (
                b"The quick brown fox jumps over the lazy dog",
                "9e107d9d372bb6826bd81d3542a419d6",
            ),
            (
                b"appkey=1d8b6e7d45233436&ts=1700000000560c52ccd288fed045859ed18bffd973",
                "fba7fec219a0d3002d1e76b650865a85",
            ),
        ] {
            assert_eq!(md5::hex_digest(input), want);
        }
        for (n, want) in [
            (55usize, "04364420e25c512fd958a70738aa8f72"),
            (56, "668a72d5ba17f08e62dabcafad6db14b"),
            (63, "7dc2ca208106a2f703567bdff99d8981"),
            (64, "c1bb4f81d892b2d57947682aeb252456"),
            (119, "ab347a5f68c8a443cfcddc633f12c24f"),
            (120, "fb98667f98096de92620b64f46e1c5b5"),
        ] {
            let s: Vec<u8> = vec![b'x'; n];
            assert_eq!(md5::hex_digest(&s), want, "{n} bytes");
        }
    }

    /// sign_app 运行时时钟路径的冒烟检查（内容校验归固定 ts 基准）。
    #[test]
    fn sign_app_smoke() -> Result<(), String> {
        let out = sign_app(
            json!({"local_id": "0"}),
            Some(json!("4409e2ce8ffd12b8")),
            Some(json!("59b43e04ad6965f34319062b478f83dd")),
        )?;
        assert!(
            out.starts_with("local_id=0&appkey=4409e2ce8ffd12b8&ts="),
            "got {out}"
        );
        assert!(out.contains("&sign="), "got {out}");
        let default_out = sign_app(json!({"k": "v"}), None, None)?;
        assert!(
            default_out.contains("appkey=1d8b6e7d45233436"),
            "got {default_out}"
        );
        assert!(default_out.contains("ts="), "got {default_out}");
        Ok(())
    }
}
