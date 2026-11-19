//! CPython 3.9 `str` 语义复刻，仅服务 parse 模块的六个移植函数。
//!
//! Unicode 表由本机 Python 3.9.6（Unicode 13.0）实测生成，与录制 oracle 同源：
//! 空白集（`str.strip` 与 re `\s` 共用）、`isdigit` 集（`Nd` + Numeric_Type=Digit，
//! 同 re `\d`）、`Nd` 数值表（`int()` 的解析域）、repr() 的转义集。
//! 未赋值码位（Cn）不在 repr 转义表内（Python 会转义）——输入域达不到，见汇报。
#![forbid(unsafe_code)]

use std::fmt::Write as _;

/// Unicode 空白：CPython `_PyUnicode_IsWhitespace`，`str.strip()` 与 re `\s` 共用。
static WS_RANGES: [(u32, u32); 10] = [
    (0x9, 0xD),
    (0x1C, 0x20),
    (0x85, 0x85),
    (0xA0, 0xA0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
];

/// `str.isdigit()` 的字符集：`Nd` + Numeric_Type=Digit（上标、圆圈数字等），同 re `\d`。
static DIGIT_RANGES: [(u32, u32); 80] = [
    (0x30, 0x39),
    (0xB2, 0xB3),
    (0xB9, 0xB9),
    (0x660, 0x669),
    (0x6F0, 0x6F9),
    (0x7C0, 0x7C9),
    (0x966, 0x96F),
    (0x9E6, 0x9EF),
    (0xA66, 0xA6F),
    (0xAE6, 0xAEF),
    (0xB66, 0xB6F),
    (0xBE6, 0xBEF),
    (0xC66, 0xC6F),
    (0xCE6, 0xCEF),
    (0xD66, 0xD6F),
    (0xDE6, 0xDEF),
    (0xE50, 0xE59),
    (0xED0, 0xED9),
    (0xF20, 0xF29),
    (0x1040, 0x1049),
    (0x1090, 0x1099),
    (0x1369, 0x1371),
    (0x17E0, 0x17E9),
    (0x1810, 0x1819),
    (0x1946, 0x194F),
    (0x19D0, 0x19DA),
    (0x1A80, 0x1A89),
    (0x1A90, 0x1A99),
    (0x1B50, 0x1B59),
    (0x1BB0, 0x1BB9),
    (0x1C40, 0x1C49),
    (0x1C50, 0x1C59),
    (0x2070, 0x2070),
    (0x2074, 0x2079),
    (0x2080, 0x2089),
    (0x2460, 0x2468),
    (0x2474, 0x247C),
    (0x2488, 0x2490),
    (0x24EA, 0x24EA),
    (0x24F5, 0x24FD),
    (0x24FF, 0x24FF),
    (0x2776, 0x277E),
    (0x2780, 0x2788),
    (0x278A, 0x2792),
    (0xA620, 0xA629),
    (0xA8D0, 0xA8D9),
    (0xA900, 0xA909),
    (0xA9D0, 0xA9D9),
    (0xA9F0, 0xA9F9),
    (0xAA50, 0xAA59),
    (0xABF0, 0xABF9),
    (0xFF10, 0xFF19),
    (0x104A0, 0x104A9),
    (0x10A40, 0x10A43),
    (0x10D30, 0x10D39),
    (0x10E60, 0x10E68),
    (0x11052, 0x1105A),
    (0x11066, 0x1106F),
    (0x110F0, 0x110F9),
    (0x11136, 0x1113F),
    (0x111D0, 0x111D9),
    (0x112F0, 0x112F9),
    (0x11450, 0x11459),
    (0x114D0, 0x114D9),
    (0x11650, 0x11659),
    (0x116C0, 0x116C9),
    (0x11730, 0x11739),
    (0x118E0, 0x118E9),
    (0x11950, 0x11959),
    (0x11C50, 0x11C59),
    (0x11D50, 0x11D59),
    (0x11DA0, 0x11DA9),
    (0x16A60, 0x16A69),
    (0x16B50, 0x16B59),
    (0x1D7CE, 0x1D7FF),
    (0x1E140, 0x1E149),
    (0x1E2F0, 0x1E2F9),
    (0x1E950, 0x1E959),
    (0x1F100, 0x1F10A),
    (0x1FBF0, 0x1FBF9),
];

/// `Nd` 十进制数字：`int()` 的解析域。每段恰 10 宽、以数字 0 起止（数学字母数字区
/// 已按 10 位拆段），数值 = 码位 − 段起点。
static ND_RANGES: [(u32, u32); 65] = [
    (0x30, 0x39),
    (0x660, 0x669),
    (0x6F0, 0x6F9),
    (0x7C0, 0x7C9),
    (0x966, 0x96F),
    (0x9E6, 0x9EF),
    (0xA66, 0xA6F),
    (0xAE6, 0xAEF),
    (0xB66, 0xB6F),
    (0xBE6, 0xBEF),
    (0xC66, 0xC6F),
    (0xCE6, 0xCEF),
    (0xD66, 0xD6F),
    (0xDE6, 0xDEF),
    (0xE50, 0xE59),
    (0xED0, 0xED9),
    (0xF20, 0xF29),
    (0x1040, 0x1049),
    (0x1090, 0x1099),
    (0x17E0, 0x17E9),
    (0x1810, 0x1819),
    (0x1946, 0x194F),
    (0x19D0, 0x19D9),
    (0x1A80, 0x1A89),
    (0x1A90, 0x1A99),
    (0x1B50, 0x1B59),
    (0x1BB0, 0x1BB9),
    (0x1C40, 0x1C49),
    (0x1C50, 0x1C59),
    (0xA620, 0xA629),
    (0xA8D0, 0xA8D9),
    (0xA900, 0xA909),
    (0xA9D0, 0xA9D9),
    (0xA9F0, 0xA9F9),
    (0xAA50, 0xAA59),
    (0xABF0, 0xABF9),
    (0xFF10, 0xFF19),
    (0x104A0, 0x104A9),
    (0x10D30, 0x10D39),
    (0x11066, 0x1106F),
    (0x110F0, 0x110F9),
    (0x11136, 0x1113F),
    (0x111D0, 0x111D9),
    (0x112F0, 0x112F9),
    (0x11450, 0x11459),
    (0x114D0, 0x114D9),
    (0x11650, 0x11659),
    (0x116C0, 0x116C9),
    (0x11730, 0x11739),
    (0x118E0, 0x118E9),
    (0x11950, 0x11959),
    (0x11C50, 0x11C59),
    (0x11D50, 0x11D59),
    (0x11DA0, 0x11DA9),
    (0x16A60, 0x16A69),
    (0x16B50, 0x16B59),
    (0x1D7CE, 0x1D7D7),
    (0x1D7D8, 0x1D7E1),
    (0x1D7E2, 0x1D7EB),
    (0x1D7EC, 0x1D7F5),
    (0x1D7F6, 0x1D7FF),
    (0x1E140, 0x1E149),
    (0x1E2F0, 0x1E2F9),
    (0x1E950, 0x1E959),
    (0x1FBF0, 0x1FBF9),
];

/// repr() 的转义集：Cc/Cf/Co/Zl/Zp 与 Zs（除空格）。未赋值码位（Cn）不在此表。
static NONPRINT_RANGES: [(u32, u32); 27] = [
    (0x0, 0x1F),
    (0x7F, 0xA0),
    (0xAD, 0xAD),
    (0x600, 0x605),
    (0x61C, 0x61C),
    (0x6DD, 0x6DD),
    (0x70F, 0x70F),
    (0x8E2, 0x8E2),
    (0x1680, 0x1680),
    (0x180E, 0x180E),
    (0x2000, 0x200F),
    (0x2028, 0x202F),
    (0x205F, 0x2064),
    (0x2066, 0x206F),
    (0x3000, 0x3000),
    (0xE000, 0xF8FF),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x13438),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
    (0xF0000, 0xFFFFD),
    (0x100000, 0x10FFFD),
];

fn in_ranges(cp: u32, ranges: &[(u32, u32)]) -> bool {
    let i = ranges.partition_point(|&(_, hi)| hi < cp);
    matches!(ranges.get(i), Some(&(lo, hi)) if lo <= cp && cp <= hi)
}

fn is_nonprint(c: char) -> bool {
    in_ranges(c as u32, &NONPRINT_RANGES)
}

/// 单字符是否 Unicode 空白（`str.strip()` 与 re `\s` 同一集合）。
pub fn is_ws(c: char) -> bool {
    in_ranges(c as u32, &WS_RANGES)
}

/// Python `str.strip()`：去两端 Unicode 空白。
pub fn strip_ws(s: &str) -> &str {
    s.trim_matches(is_ws)
}

/// Python `str.strip(ch)`：去两端指定字符。
pub fn strip_char(s: &str, ch: char) -> &str {
    s.trim_matches(ch)
}

/// 单字符的 `str.isdigit()` 判定（`Nd` + Numeric_Type=Digit），同 re `\d`。
pub fn is_digit_char(c: char) -> bool {
    in_ranges(c as u32, &DIGIT_RANGES)
}

/// Python `str.isdigit()`：非空且每个字符都是数字。
pub fn is_digit_str(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_digit_char)
}

/// 字符的 `Nd` 数值（0..=9）；非 `Nd`（如上标 ²）为 None，与 `int()` 的解析域一致。
pub fn nd_digit(c: char) -> Option<u32> {
    let cp = c as u32;
    let i = ND_RANGES.partition_point(|&(_, hi)| hi < cp);
    match ND_RANGES.get(i) {
        Some(&(lo, hi)) if lo <= cp && cp <= hi => Some(cp - lo),
        _ => None,
    }
}

/// Python `int(s)`（s 为数字串）：全 `Nd` 才可解析，ValueError 优先于数量级；
/// 前导零（含全角）剥除后再累加；超出 i64 → OverflowError（Python 是任意精度）。
pub fn py_int(s: &str) -> Result<i64, String> {
    let invalid = || format!("ValueError: invalid literal for int() with base 10: '{s}'");
    if s.is_empty() {
        return Err(invalid());
    }
    let body: String = s.chars().skip_while(|&c| nd_digit(c) == Some(0)).collect();
    if body.is_empty() {
        return Ok(0);
    }
    if body.chars().any(|c| nd_digit(c).is_none()) {
        return Err(invalid());
    }
    let mut val: i64 = 0;
    for c in body.chars() {
        let d = match nd_digit(c) {
            Some(d) => d,
            None => return Err(invalid()),
        };
        val = match val
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(d)))
        {
            Some(v) => v,
            None => return Err("OverflowError: int too large to convert to i64".to_string()),
        };
    }
    Ok(val)
}

/// Python `str.lower()`：全 Unicode 小写映射。
pub fn py_lower(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

/// Python `repr(s)`：含单引号且不含双引号时改用双引号包裹；不可见字符转义。
/// 未赋值码位（Cn）Python 会转义、此处不转义（输入域达不到，见汇报已知差异）。
pub fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(quote);
            }
            c if is_nonprint(c) => {
                let cp = c as u32;
                if cp < 0x100 {
                    let _ = write!(out, "\\x{cp:02x}");
                } else if cp < 0x10000 {
                    let _ = write!(out, "\\u{cp:04x}");
                } else {
                    let _ = write!(out, "\\U{cp:08x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}
