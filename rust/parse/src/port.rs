//! parse 模块移植：目标识别、清晰度映射、日期/时长/计数格式化、安全文件名。
//! 逐函数等价翻译自 `bilibili_cli.py`（行号见 rust/tasks/parse.md）。
//! 源码失败路径走 `die()`（SystemExit，退出码 2）——单元录制覆盖不了，移植为
//! `Err`（载荷 = die 的 msg 原文，app 层负责打印与退出码），见已知差异。
#![forbid(unsafe_code)]

use crate::pystr;

use serde_json::{Value, json};

/// die() 文案：search.bilibili.com 专属报错，判断先于 BV/av/space 各分支。
const SEARCH_DIE_MSG: &str = "这是 B 站的「搜索结果页」，不是 UP 主主页，里面没有 uid。\n  \
     正确做法：点进该 UP 主的主页，复制 space.bilibili.com/数字 的链接；\n  \
     或者直接给数字 uid（例如 23947287）。";

/// Port of `parse_target` (`bilibili_cli.py:431`). uid / BV / 链接识别。
///
/// 原函数对无法识别的目标 `die()`（SystemExit 退出码 2）。该路径**不在单元录制内**
/// （录制进程会被 SystemExit 杀掉），移植为 `Err`，完整文案由 app 层用例
/// （`bcli list 垃圾串` 等）覆盖；`search.bilibili.com` 的专属文案也在此处。
pub fn parse_target(t: String) -> Result<Value, String> {
    // t = t.strip().strip('"').strip("'")
    let s = pystr::strip_ws(&t);
    let s = pystr::strip_char(s, '"');
    let s = pystr::strip_char(s, '\'');

    if s.contains("search.bilibili.com") {
        return Err(SEARCH_DIE_MSG.to_string());
    }
    if let Some(bvid) = find_bv(s) {
        return Ok(json!({"type": "video", "bvid": bvid}));
    }
    if let Some(aid) = find_av_or_aid(s)? {
        return Ok(json!({"type": "video", "aid": aid}));
    }
    if let Some(uid) = find_space_uid(s) {
        return Ok(json!({"type": "user", "uid": uid}));
    }
    if pystr::is_digit_str(s) {
        return Ok(json!({"type": "user", "uid": s}));
    }
    Err(format!(
        "无法识别 {}：既不是 uid 也不是 BV 号\n  \
         可用形式：数字 uid / space.bilibili.com/<uid>... / BV 号或视频链接\n  \
         提示：链接里有 ? 时请用引号包住，否则 shell 会当成通配符报错",
        pystr::py_repr(s)
    ))
}

/// `re.search(r"(BV[0-9A-Za-z]{10})", t)`：最左起点，BV 后恰 10 个 ASCII 字母数字。
fn find_bv(s: &str) -> Option<String> {
    let cs: Vec<char> = s.chars().collect();
    for i in 0..cs.len() {
        if cs[i] == 'B'
            && cs.get(i + 1) == Some(&'V')
            && i + 12 <= cs.len()
            && cs[i + 2..i + 12].iter().all(|c| c.is_ascii_alphanumeric())
        {
            return Some(cs[i..i + 12].iter().collect());
        }
    }
    None
}

/// `re.search(r"(?:av|aid=)(\d+)", t)`：同位置先试 av 再试 aid=；
/// `\d` 是 Unicode 数字集（全角可匹配），数字串交 `int()` 解析。
fn find_av_or_aid(s: &str) -> Result<Option<i64>, String> {
    let cs: Vec<char> = s.chars().collect();
    for (i, &c) in cs.iter().enumerate() {
        if c == 'a'
            && cs.get(i + 1) == Some(&'v')
            && let Some(run) = digit_run(&cs, i + 2)
        {
            return pystr::py_int(&run).map(Some);
        }
        if c == 'a'
            && cs.get(i + 1) == Some(&'i')
            && cs.get(i + 2) == Some(&'d')
            && cs.get(i + 3) == Some(&'=')
            && let Some(run) = digit_run(&cs, i + 4)
        {
            return pystr::py_int(&run).map(Some);
        }
    }
    Ok(None)
}

/// `start` 起的最长 `\d+` 串；首字符非数字则 None。
fn digit_run(cs: &[char], start: usize) -> Option<String> {
    if !cs.get(start).is_some_and(|&c| pystr::is_digit_char(c)) {
        return None;
    }
    let end = cs[start..]
        .iter()
        .position(|&c| !pystr::is_digit_char(c))
        .map_or(cs.len(), |n| start + n);
    Some(cs[start..end].iter().collect())
}

/// `re.search(r"space\.bilibili\.com/(\d+)", t)`：字面量逐位推进（re 语义，
/// 前一处不匹配时从下一字符继续），组内 uid 保持字符串。
fn find_space_uid(s: &str) -> Option<String> {
    const LIT: &str = "space.bilibili.com/";
    let mut from = 0usize;
    while let Some(rel) = s[from..].find(LIT) {
        let abs = from + rel;
        let rest = &s[abs + LIT.len()..];
        if rest.chars().next().is_some_and(pystr::is_digit_char) {
            return Some(
                rest.chars()
                    .take_while(|c| pystr::is_digit_char(*c))
                    .collect(),
            );
        }
        from = abs + 1;
    }
    None
}

/// `QN_BY_NAME`（bilibili_cli.py:59-63 反查表 + update 追加项；"1080p+" 两处同值 112）。
fn qn_by_name(k: &str) -> Option<i64> {
    Some(match k {
        "8K" => 127,
        "杜比视界" => 126,
        "HDR" => 125,
        "4K" => 120,
        "1080P60" => 116,
        "1080P+" => 112,
        "1080P" => 80,
        "720P60" => 74,
        "720P" => 64,
        "480P" => 32,
        "360P" => 16,
        "720p" => 64,
        "1080p" => 80,
        "480p" => 32,
        "360p" => 16,
        "4k" => 120,
        "1080p+" => 112,
        _ => return None,
    })
}

/// Port of `_qn_value` (`bilibili_cli.py:553`).
///
/// `name` 在源码中默认 `None`。纯数字串（Unicode 数字）直转 int；
/// 未知名 → 127；None/空串 → 127。
pub fn _qn_value(name: Option<String>) -> Result<i64, String> {
    let name = match name {
        Some(n) if !n.is_empty() => n,
        _ => return Ok(127),
    };
    let stripped = pystr::strip_ws(&name);
    let k = pystr::py_lower(stripped);
    if pystr::is_digit_str(&k) {
        return pystr::py_int(&k);
    }
    Ok(qn_by_name(stripped)
        .or_else(|| qn_by_name(&k))
        .unwrap_or(127))
}

/// Port of `iso` (`bilibili_cli.py:488`). unix 秒 → ISO8601（本机 +08:00）。
///
/// 源码 `datetime.fromtimestamp(ts, timezone.utc).astimezone().isoformat(...)`：
/// astimezone 取**本机时区**——本机 Asia/Shanghai，现代时段即 +08:00（1901 年前为
/// LMT +08:05:43、1940 年代有夏令时）。按任务书固定 +08:00；基线与 app 域
/// （视频 pubdate）全为现代时段，不受影响，详见已知差异。
pub fn iso(ts: i64) -> Result<String, String> {
    // 本机 CPython 3.9.6 实测的异常分区（与录制 oracle 同源）：
    // |ts| ≥ 6.7768036e16 → 平台 gmtime EOVERFLOW → OSError
    const GMTIME_LIMIT: i64 = 67768036191676800;
    // fromtimestamp 的合法域：0001-01-01T00:00:00Z ..= 9999-12-31T23:59:59Z
    const MIN_TS: i64 = -62135596800;
    const MAX_TS: i64 = 253402300799;
    // UTC ≥ 9999-12-31T16:00:00Z 时本地(+08:00)越过 datetime.max → OverflowError
    const OVF_TS: i64 = 253402272000;

    if ts >= GMTIME_LIMIT || ts <= -GMTIME_LIMIT {
        return Err("OSError: [Errno 84] Value too large to be stored in data type".to_string());
    }
    if !(MIN_TS..=MAX_TS).contains(&ts) {
        return Err(format!("ValueError: year {} is out of range", year_of(ts)));
    }
    if ts >= OVF_TS {
        return Err("OverflowError: date value out of range".to_string());
    }
    let local = ts + 8 * 3600; // 固定 +08:00
    let days = local.div_euclid(86400);
    let secs = local.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+08:00",
        y,
        m,
        d,
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    ))
}

/// ts 所在 UTC 年（仅用于 ValueError 文案，Hinnant 公式）。
fn year_of(ts: i64) -> i64 {
    civil_from_days(ts.div_euclid(86400)).0
}

/// 天数 → 公历年月日（Howard Hinnant 的 civil_from_days，floor 语义）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

/// Port of `fmt_dur` (`bilibili_cli.py:492`). 秒 → "39:29" 文本。
///
/// `sec = int(sec or 0)`：0/±0 → 0，int() 向零截断；Python floor 商余用
/// euclid 复刻（负数如 -5 → "-1:59:55"）。NaN/inf 对应 int() 的
/// ValueError/OverflowError。|sec| 超出 i64 域时 Rust 饱和截断（见已知差异）。
pub fn fmt_dur(sec: f64) -> Result<String, String> {
    let sec = if sec == 0.0 { 0.0 } else { sec }; // `sec or 0`
    if sec.is_nan() {
        return Err("ValueError: cannot convert float NaN to integer".to_string());
    }
    if sec.is_infinite() {
        return Err("OverflowError: cannot convert float infinity to integer".to_string());
    }
    let s = sec.trunc() as i64;
    let h = s.div_euclid(3600);
    let m = s.rem_euclid(3600) / 60;
    let s = s.rem_euclid(60);
    Ok(if h != 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    })
}

/// Port of `_fmt_count` (`bilibili_cli.py:1400`). 播放数 → "1.2万" 文本。
///
/// `n = int(n or 0)` 在 i64 绑定域内等价于原值（0 → 0）。`:.1f` 为
/// 正确舍入 + half-even（Rust `{:.1}` 同语义，基线已录 tie 用例 12500/17500）。
pub fn _fmt_count(n: i64) -> Result<String, String> {
    if n >= 100000000 {
        return Ok(format!("{:.1}亿", n as f64 / 100000000.0));
    }
    if n >= 10000 {
        return Ok(format!("{:.1}万", n as f64 / 10000.0));
    }
    Ok(n.to_string())
}

/// Port of `safe_name` (`bilibili_cli.py:651`). 标题 → 安全文件名（默认限 80 字符）。
///
/// `re.sub(r"[/\\:*?\"<>|\n\r\t]", "_", s).strip()` → 替换非法字符再去两端空白；
/// `re.sub(r"\s+", " ", s)` → Unicode 空白折叠；`s[:limit].rstrip(" .")`
/// → 按字符切片（含负索引语义）再去尾随空格与点。
pub fn safe_name(s: String, limit: i64) -> Result<String, String> {
    let replaced: String = s
        .chars()
        .map(|c| if is_fname_bad(c) { '_' } else { c })
        .collect();
    let stripped = pystr::strip_ws(&replaced);
    let collapsed = collapse_ws(stripped);
    let chars: Vec<char> = collapsed.chars().collect();
    let n = chars.len() as i64;
    let keep = if limit >= 0 {
        limit.min(n)
    } else {
        (n + limit).max(0) // Python 负索引切片 s[limit:]
    };
    let sliced: String = chars[..keep as usize].iter().collect();
    Ok(sliced.trim_end_matches([' ', '.']).to_string())
}

/// `safe_name` 替换类（ASCII，全角？！：等不在此列，原样保留）。
fn is_fname_bad(c: char) -> bool {
    matches!(
        c,
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' | '\t'
    )
}

/// `re.sub(r"\s+", " ", s)`：每个 Unicode 空白串折叠为一个半角空格。
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if pystr::is_ws(c) {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}
