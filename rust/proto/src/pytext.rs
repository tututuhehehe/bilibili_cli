//! Python str 语义的文本辅助：`isdigit` / `\d` / `int()` / `strip()` / utf-8 replace。
//!
//! 与原版逐字对齐的部分：
//! - `_strip_em` 的 `.strip()`：Python `str.isspace` 集合（Unicode White_Space + U+001C-001F）。
//! - `_dur_seconds` 的 `re.findall(r"(\d+):(\d+)(?::(\d+))?")`：`\d` = Unicode Nd，
//!   `findall` 左起不重叠扫描，取最后一个匹配。
//! - `Item.param.isdigit()`：True = 全部字符为 Nd 或 No 类（Python `str.isdigit`）；
//!   `int()` 只接受 Nd——No 字符会让原版抛 `ValueError`，port 同样返回 `Err("ValueError: ...")`。
//! - Nd/No 范围表由本机 Python 的 `unicodedata`（Unicode 13.0.0）生成，离线内嵌。

#![forbid(unsafe_code)]

/// Nd 类十进制数字范围：(起点, 终点, 起点的数字值)。区间内数字值 = 偏移 + 表值。
const ND_RANGES: &[(u32, u32, u32)] = &[
    // unicodedata 13.0.0
    (0x0030, 0x0039, 0),
    (0x0660, 0x0669, 0),
    (0x06F0, 0x06F9, 0),
    (0x07C0, 0x07C9, 0),
    (0x0966, 0x096F, 0),
    (0x09E6, 0x09EF, 0),
    (0x0A66, 0x0A6F, 0),
    (0x0AE6, 0x0AEF, 0),
    (0x0B66, 0x0B6F, 0),
    (0x0BE6, 0x0BEF, 0),
    (0x0C66, 0x0C6F, 0),
    (0x0CE6, 0x0CEF, 0),
    (0x0D66, 0x0D6F, 0),
    (0x0DE6, 0x0DEF, 0),
    (0x0E50, 0x0E59, 0),
    (0x0ED0, 0x0ED9, 0),
    (0x0F20, 0x0F29, 0),
    (0x1040, 0x1049, 0),
    (0x1090, 0x1099, 0),
    (0x17E0, 0x17E9, 0),
    (0x1810, 0x1819, 0),
    (0x1946, 0x194F, 0),
    (0x19D0, 0x19D9, 0),
    (0x1A80, 0x1A89, 0),
    (0x1A90, 0x1A99, 0),
    (0x1B50, 0x1B59, 0),
    (0x1BB0, 0x1BB9, 0),
    (0x1C40, 0x1C49, 0),
    (0x1C50, 0x1C59, 0),
    (0xA620, 0xA629, 0),
    (0xA8D0, 0xA8D9, 0),
    (0xA900, 0xA909, 0),
    (0xA9D0, 0xA9D9, 0),
    (0xA9F0, 0xA9F9, 0),
    (0xAA50, 0xAA59, 0),
    (0xABF0, 0xABF9, 0),
    (0xFF10, 0xFF19, 0),
    (0x104A0, 0x104A9, 0),
    (0x10D30, 0x10D39, 0),
    (0x11066, 0x1106F, 0),
    (0x110F0, 0x110F9, 0),
    (0x11136, 0x1113F, 0),
    (0x111D0, 0x111D9, 0),
    (0x112F0, 0x112F9, 0),
    (0x11450, 0x11459, 0),
    (0x114D0, 0x114D9, 0),
    (0x11650, 0x11659, 0),
    (0x116C0, 0x116C9, 0),
    (0x11730, 0x11739, 0),
    (0x118E0, 0x118E9, 0),
    (0x11950, 0x11959, 0),
    (0x11C50, 0x11C59, 0),
    (0x11D50, 0x11D59, 0),
    (0x11DA0, 0x11DA9, 0),
    (0x16A60, 0x16A69, 0),
    (0x16B50, 0x16B59, 0),
    (0x1D7CE, 0x1D7FF, 0),
    (0x1E140, 0x1E149, 0),
    (0x1E2F0, 0x1E2F9, 0),
    (0x1E950, 0x1E959, 0),
    (0x1FBF0, 0x1FBF9, 0),
];

/// No 类其他数字范围（`isdigit` 为真但 `int()` 拒收）：仅判存在，不取值。
const NO_RANGES: &[(u32, u32)] = &[
    (0x00B2, 0x00B3),
    (0x00B9, 0x00B9),
    (0x00BC, 0x00BE),
    (0x09F4, 0x09F9),
    (0x0B72, 0x0B77),
    (0x0BF0, 0x0BF2),
    (0x0C78, 0x0C7E),
    (0x0D58, 0x0D5E),
    (0x0D70, 0x0D78),
    (0x0F2A, 0x0F33),
    (0x1369, 0x137C),
    (0x17F0, 0x17F9),
    (0x19DA, 0x19DA),
    (0x2070, 0x2070),
    (0x2074, 0x2079),
    (0x2080, 0x2089),
    (0x2150, 0x215F),
    (0x2189, 0x2189),
    (0x2460, 0x249B),
    (0x24EA, 0x24FF),
    (0x2776, 0x2793),
    (0x2CFD, 0x2CFD),
    (0x3192, 0x3195),
    (0x3220, 0x3229),
    (0x3248, 0x324F),
    (0x3251, 0x325F),
    (0x3280, 0x3289),
    (0x32B1, 0x32BF),
    (0xA830, 0xA835),
    (0x10107, 0x10133),
    (0x10175, 0x10178),
    (0x1018A, 0x1018B),
    (0x102E1, 0x102FB),
    (0x10320, 0x10323),
    (0x10858, 0x1085F),
    (0x10879, 0x1087F),
    (0x108A7, 0x108AF),
    (0x108FB, 0x108FF),
    (0x10916, 0x1091B),
    (0x109BC, 0x109BD),
    (0x109C0, 0x109CF),
    (0x109D2, 0x109FF),
    (0x10A40, 0x10A48),
    (0x10A7D, 0x10A7E),
    (0x10A9D, 0x10A9F),
    (0x10AEB, 0x10AEF),
    (0x10B58, 0x10B5F),
    (0x10B78, 0x10B7F),
    (0x10BA9, 0x10BAF),
    (0x10CFA, 0x10CFF),
    (0x10E60, 0x10E7E),
    (0x10F1D, 0x10F26),
    (0x10F51, 0x10F54),
    (0x10FC5, 0x10FCB),
    (0x11052, 0x11065),
    (0x111E1, 0x111F4),
    (0x1173A, 0x1173B),
    (0x118EA, 0x118F2),
    (0x11C5A, 0x11C6C),
    (0x11FC0, 0x11FD4),
    (0x16B5B, 0x16B61),
    (0x16E80, 0x16E96),
    (0x1D2E0, 0x1D2F3),
    (0x1D360, 0x1D378),
    (0x1E8C7, 0x1E8CF),
    (0x1EC71, 0x1ECAB),
    (0x1ECAD, 0x1ECAF),
    (0x1ECB1, 0x1ECB4),
    (0x1ED01, 0x1ED2D),
    (0x1ED2F, 0x1ED3D),
    (0x1F100, 0x1F10C),
];

/// 字符的 Nd 数字值（`unicodedata.digit`），非 Nd 返回 `None`。
pub fn nd_digit(c: char) -> Option<u32> {
    let cp = c as u32;
    for &(lo, hi, dv) in ND_RANGES {
        if cp >= lo && cp <= hi {
            return Some(dv + cp - lo);
        }
    }
    None
}

/// 是否 No 类（`isdigit` 计入、`int()` 拒收的字符）。
pub fn is_no(c: char) -> bool {
    let cp = c as u32;
    NO_RANGES.iter().any(|&(lo, hi)| cp >= lo && cp <= hi)
}

/// Python `str.isdigit`：非空且全部字符为 Nd 或 No。
pub fn is_digit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| nd_digit(c).is_some() || is_no(c))
}

/// 全 Nd 字符串 → ASCII 十进制串；含任何非 Nd 字符返回 `None`
/// （`int()` 只吃 Nd：No 字符在 Python 里抛 `ValueError`）。
pub fn digit_string(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push((b'0' + nd_digit(c)? as u8) as char);
    }
    Some(out)
}

/// Python `str.isspace`（White_Space + U+001C-001F）。
fn is_py_space(c: char) -> bool {
    matches!(c as u32,
        0x09..=0x0D | 0x1C..=0x1F | 0x20 | 0x85 | 0xA0
        | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000)
}

/// Python `str.strip()`：两端剥掉全部 `isspace` 字符；全空白返回空串。
pub fn py_strip(s: &str) -> &str {
    let start = s
        .char_indices()
        .find(|&(_, c)| !is_py_space(c))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let end = s
        .char_indices()
        .rev()
        .find(|&(_, c)| !is_py_space(c))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    if start >= end {
        return "";
    }
    &s[start..end]
}

/// `bytes.decode("utf-8", "replace")`：无效序列按 Unicode「最大子部分」替换为 U+FFFD
/// （CPython 与 Rust `from_utf8_lossy` 同策略，已对拍）。
pub fn decode_replace(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// `re.findall(r"(\d+):(\d+)(?::(\d+))?"` 的匹配结果：(组1, 组2, 组3 或 None)。
/// 左起逐位尝试、匹配后从末尾继续、互不重叠，组内贪婪、组3 整体可选。
pub fn dur_matches(text: &str) -> Vec<(String, String, Option<String>)> {
    let chars: Vec<char> = text.chars().collect();
    let nd = |c: char| nd_digit(c).is_some();
    let run = |from: usize| -> usize {
        let mut k = from;
        while k < chars.len() && nd(chars[k]) {
            k += 1;
        }
        k
    };
    let digits = |from: usize, to: usize| -> String {
        chars[from..to]
            .iter()
            .map(|&c| (b'0' + nd_digit(c).expect("checked") as u8) as char)
            .collect()
    };

    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let d1_end = run(i);
        if d1_end == i || d1_end >= chars.len() || chars[d1_end] != ':' {
            i += 1;
            continue;
        }
        let d2_start = d1_end + 1;
        let d2_end = run(d2_start);
        if d2_end == d2_start {
            i += 1;
            continue;
        }
        let g3 = if d2_end < chars.len()
            && chars[d2_end] == ':'
            && d2_end + 1 < chars.len()
            && nd(chars[d2_end + 1])
        {
            let d3_end = run(d2_end + 1);
            Some((digits(d2_end + 1, d3_end), d3_end))
        } else {
            None
        };
        let end = g3.as_ref().map_or(d2_end, |&(_, e)| e);
        out.push((
            digits(i, d1_end),
            digits(d2_start, d2_end),
            g3.map(|(d, _)| d),
        ));
        i = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_strip_is_python_isspace() {
        // U+3000 全角空格、C0 分隔符（Python 认空格，Rust trim 不认）
        assert_eq!(py_strip("　露营　"), "露营");
        assert_eq!(py_strip("\x1c边缘\x1f"), "边缘");
        assert_eq!(py_strip("\t\n\r\u{0b}\u{0c} "), "");
        assert_eq!(py_strip("  x  "), "x");
        assert_eq!(py_strip(" x "), "x"); // U+00A0
        assert_eq!(py_strip(" y "), "y"); // U+2028/U+2029
    }

    #[test]
    fn isdigit_covers_nd_and_no() {
        assert!(is_digit("123"));
        assert!(is_digit("１２３")); // 全角 Nd
        assert!(is_digit("٣٤")); // 阿拉伯-印度 Nd
        assert!(is_digit("²")); // 上标 No：isdigit 真、int() 拒收
        assert!(!is_digit(""));
        assert!(!is_digit("12a"));
        // No 处在串中也让 digit_string 失败（对应原版 int() 的 ValueError）
        assert!(is_digit("1²"));
        assert_eq!(digit_string("1²"), None);
        assert_eq!(digit_string("１２３").as_deref(), Some("123"));
    }

    #[test]
    fn dur_matches_unicode_and_last() {
        let m = dur_matches("共25集 742:44");
        assert_eq!(m.len(), 1);
        assert_eq!(
            (m[0].0.as_str(), m[0].1.as_str(), m[0].2.as_deref()),
            ("742", "44", None)
        );
        assert_eq!(dur_matches("１:２").len(), 1); // 全角也计
        assert_eq!(dur_matches("10:20:30:40").len(), 1); // 30:40 不再成第二段
        assert_eq!(dur_matches("1:2 3:4:5").len(), 2);
        assert_eq!(dur_matches("68::44").len(), 0);
        assert_eq!(dur_matches("垃圾").len(), 0);
    }

    #[test]
    fn decode_replace_matches_cpython() {
        // 实测 CPython utf-8 replace（最大子部分策略），已对拍
        assert_eq!(
            decode_replace(b"abc\xff\xed\xa0\x80\xe0\xa0def"),
            "abc\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}def"
        );
        assert_eq!(decode_replace(b"\xe4\xb8"), "\u{fffd}");
        assert_eq!(
            decode_replace(&[0xf4, 0x90, 0x80, 0x80]),
            "\u{fffd}\u{fffd}\u{fffd}\u{fffd}"
        );
    }
}
