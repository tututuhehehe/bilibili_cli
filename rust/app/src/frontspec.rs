//! argparse 前置层：逐字节复刻 Python argparse（3.9）对本工具命令面的
//! 接受/拒绝判定与报错排版（app-cases 的 89 条离线用例由此原生作答）。
//!
//! 语义来源（复刻器探针实测）：
//! ① 值错误（invalid choice / invalid int / expected one argument /
//!    ignored explicit argument）——左到右扫描即时报，
//!    prog = "bilibili_cli <cmd>"，usage = 子命令块
//! ② 缺必需位置参数——prog 带子命令，usage = 子命令块
//! ③ 未识别旗标/多余参数——最后报，prog 不带子命令，usage = 顶层块
//! 值语义：选项值可吞负数字（`^-\d+$`）但不吞其他旗标；长旗标支持唯一前缀缩写。

use std::ffi::OsString;

use crate::front_texts::{
    HELP_DETAIL_DL, HELP_DETAIL_DOCTOR, HELP_DETAIL_INFO, HELP_DETAIL_LIST, HELP_DETAIL_LOGIN,
    HELP_DETAIL_SEARCH, HELP_OVERVIEW, SUBHELP_DL, SUBHELP_DOCTOR, SUBHELP_HELP, SUBHELP_INFO,
    SUBHELP_LIST, SUBHELP_LOGIN, SUBHELP_SEARCH, TOP_HELP, TOP_USAGE, USAGE_DL, USAGE_DOCTOR,
    USAGE_HELP, USAGE_INFO, USAGE_LIST, USAGE_LOGIN, USAGE_SEARCH,
};

pub enum Handled {
    /// 直接打印并退出
    Exit {
        code: u8,
        stdout: &'static str,
        stderr: String,
    },
    /// 合法命令，交给 clap → units
    Pass,
}

#[derive(Clone, Copy)]
enum Kind {
    Flag,
    Str,
    Int,
    Choice(&'static [&'static str]),
}

struct Opt {
    long: &'static str,
    short: Option<char>,
    kind: Kind,
    /// argparse 报错名（如 "-n/--limit"）
    err_name: &'static str,
}

struct Cmd {
    name: &'static str,
    usage: &'static str,
    subhelp: &'static str,
    /// 必需位置参数名（doctor/help 为 None）
    required: Option<&'static str>,
    opts: &'static [Opt],
}

const SORTS: &[&str] = &["totalrank", "play", "new", "dm", "stow"];
const CODECS: &[&str] = &["avc", "hevc", "av1"];

const CMDS: &[Cmd] = &[
    Cmd {
        name: "login",
        usage: USAGE_LOGIN,
        subhelp: SUBHELP_LOGIN,
        required: None,
        opts: &[Opt {
            long: "tv",
            short: None,
            kind: Kind::Flag,
            err_name: "--tv",
        }],
    },
    Cmd {
        name: "list",
        usage: USAGE_LIST,
        subhelp: SUBHELP_LIST,
        required: Some("target"),
        opts: &[
            Opt {
                long: "limit",
                short: Some('n'),
                kind: Kind::Int,
                err_name: "-n/--limit",
            },
            Opt {
                long: "json",
                short: None,
                kind: Kind::Flag,
                err_name: "--json",
            },
            Opt {
                long: "rights",
                short: None,
                kind: Kind::Flag,
                err_name: "--rights",
            },
        ],
    },
    Cmd {
        name: "search",
        usage: USAGE_SEARCH,
        subhelp: SUBHELP_SEARCH,
        required: Some("keyword"),
        opts: &[
            Opt {
                long: "limit",
                short: Some('n'),
                kind: Kind::Int,
                err_name: "-n/--limit",
            },
            Opt {
                long: "sort",
                short: None,
                kind: Kind::Choice(SORTS),
                err_name: "--sort",
            },
            Opt {
                long: "users",
                short: None,
                kind: Kind::Flag,
                err_name: "--users",
            },
            Opt {
                long: "desc",
                short: None,
                kind: Kind::Flag,
                err_name: "--desc",
            },
            Opt {
                long: "json",
                short: None,
                kind: Kind::Flag,
                err_name: "--json",
            },
        ],
    },
    Cmd {
        name: "info",
        usage: USAGE_INFO,
        subhelp: SUBHELP_INFO,
        required: Some("target"),
        opts: &[Opt {
            long: "json",
            short: None,
            kind: Kind::Flag,
            err_name: "--json",
        }],
    },
    Cmd {
        name: "dl",
        usage: USAGE_DL,
        subhelp: SUBHELP_DL,
        required: Some("target"),
        opts: &[
            Opt {
                long: "audio",
                short: None,
                kind: Kind::Flag,
                err_name: "--audio",
            },
            Opt {
                long: "quality",
                short: None,
                kind: Kind::Str,
                err_name: "--quality",
            },
            Opt {
                long: "codec",
                short: None,
                kind: Kind::Choice(CODECS),
                err_name: "--codec",
            },
            Opt {
                long: "parallel",
                short: None,
                kind: Kind::Int,
                err_name: "--parallel",
            },
            Opt {
                long: "out",
                short: Some('o'),
                kind: Kind::Str,
                err_name: "-o/--out",
            },
            Opt {
                long: "subs",
                short: None,
                kind: Kind::Flag,
                err_name: "--subs",
            },
            Opt {
                long: "keep-parts",
                short: None,
                kind: Kind::Flag,
                err_name: "--keep-parts",
            },
            Opt {
                long: "json",
                short: None,
                kind: Kind::Flag,
                err_name: "--json",
            },
        ],
    },
    Cmd {
        name: "doctor",
        usage: USAGE_DOCTOR,
        subhelp: SUBHELP_DOCTOR,
        required: None,
        opts: &[],
    },
    Cmd {
        name: "help",
        usage: USAGE_HELP,
        subhelp: SUBHELP_HELP,
        required: None,
        opts: &[],
    },
];

/// Python `repr()` 近似（覆盖本工具域）：不可打印字符按 \xXX / \uXXXX 转义。
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            _ if c == quote => {
                out.push('\\');
                out.push(c);
            }
            _ => {
                let u = c as u32;
                let non_printable = c.is_control()
                    || matches!(u,
                        0x00a0 | 0x2000..=0x200f | 0x2028 | 0x2029 | 0x202f
                        | 0x205f..=0x2064 | 0xfeff | 0x3000);
                if non_printable {
                    if u < 0x100 {
                        out.push_str(&format!("\\x{u:02x}"));
                    } else if u <= 0xffff {
                        out.push_str(&format!("\\u{u:04x}"));
                    } else {
                        out.push_str(&format!("\\U{u:08x}"));
                    }
                } else {
                    out.push(c);
                }
            }
        }
    }
    out.push(quote);
    out
}

fn is_negative_number(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next() == Some('-')
        && chars.as_str().chars().all(|c| c.is_ascii_digit())
        && !chars.as_str().is_empty()
}

fn value_error(cmd: &Cmd, err_name: &str, msg: String) -> Handled {
    Handled::Exit {
        code: 2,
        stdout: "",
        stderr: format!(
            "{usage}bilibili_cli {name}: error: argument {err_name}: {msg}\n",
            usage = cmd.usage,
            name = cmd.name
        ),
    }
}

fn missing_error(cmd: &Cmd, arg: &str) -> Handled {
    Handled::Exit {
        code: 2,
        stdout: "",
        stderr: format!(
            "{usage}bilibili_cli {name}: error: the following arguments are required: {arg}\n",
            usage = cmd.usage,
            name = cmd.name
        ),
    }
}

fn extras_error(extras: &[String]) -> Handled {
    Handled::Exit {
        code: 2,
        stdout: "",
        stderr: format!(
            "{TOP_USAGE}bilibili_cli: error: unrecognized arguments: {}\n",
            extras.join(" ")
        ),
    }
}

/// 在命令的选项里找长旗标（含唯一前缀缩写匹配）。
fn find_long<'a>(cmd: &'a Cmd, name: &str) -> Result<Option<&'a Opt>, ()> {
    let exact = cmd.opts.iter().find(|o| o.long == name);
    if exact.is_some() {
        return Ok(exact);
    }
    let matches: Vec<&Opt> = cmd
        .opts
        .iter()
        .filter(|o| o.long.starts_with(name))
        .collect();
    match matches.len() {
        1 => Ok(Some(matches[0])),
        0 => Ok(None),
        _ => Err(()), // 歧义缩写
    }
}

/// 解析一个子命令的 argv 段（不含命令名）。
fn parse_sub(cmd: &Cmd, toks: &[String]) -> Handled {
    let mut extras: Vec<String> = Vec::new();
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0;
    let mut only_pos = false;
    while i < toks.len() {
        let t = &toks[i];
        if only_pos {
            positionals.push(t.clone());
            i += 1;
            continue;
        }
        if t == "--" {
            only_pos = true;
            i += 1;
            continue;
        }
        if t == "-h" || t == "--help" {
            return Handled::Exit {
                code: 0,
                stdout: cmd.subhelp,
                stderr: String::new(),
            };
        }
        if t == "-" {
            positionals.push(t.clone());
            i += 1;
            continue;
        }

        // 长选项：--opt[=value]
        if let Some(long) = t.strip_prefix("--") {
            let (name, inline_val) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (long, None),
            };
            if name == "help" && inline_val.is_none() {
                return Handled::Exit {
                    code: 0,
                    stdout: cmd.subhelp,
                    stderr: String::new(),
                };
            }
            let opt = match find_long(cmd, name) {
                Ok(Some(o)) => o,
                Ok(None) | Err(_) => {
                    extras.push(t.clone());
                    i += 1;
                    continue;
                }
            };
            // 取值：内联（=value）或下一个 token（可吞负数字，不吞旗标）
            let mut separate = false;
            let value: Option<String> = match opt.kind {
                Kind::Flag => {
                    if let Some(v) = inline_val {
                        return value_error(
                            cmd,
                            opt.err_name,
                            format!("ignored explicit argument {}", py_repr(v)),
                        );
                    }
                    None
                }
                Kind::Str | Kind::Int | Kind::Choice(_) => match inline_val {
                    Some(v) => Some(v.to_string()),
                    None => match toks.get(i + 1) {
                        Some(v) if !v.starts_with('-') || is_negative_number(v) => {
                            separate = true;
                            Some(v.clone())
                        }
                        _ => return value_error(cmd, opt.err_name, "expected one argument".into()),
                    },
                },
            };
            if let Some(raw) = &value {
                match opt.kind {
                    Kind::Int => {
                        if raw.parse::<i64>().is_err() {
                            return value_error(
                                cmd,
                                opt.err_name,
                                format!("invalid int value: {}", py_repr(raw)),
                            );
                        }
                    }
                    Kind::Choice(list) if !list.contains(&raw.as_str()) => {
                        let joined = list
                            .iter()
                            .map(|c| format!("'{c}'"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return value_error(
                            cmd,
                            opt.err_name,
                            format!("invalid choice: {} (choose from {joined})", py_repr(raw)),
                        );
                    }
                    _ => {}
                }
            }
            i += if separate { 2 } else { 1 };
            continue;
        }

        // 短选项：-x、-xVALUE、-x VALUE
        if let Some(shorts) = t.strip_prefix('-') {
            if shorts.chars().all(|c| c.is_ascii_digit()) && !shorts.is_empty() {
                // 纯负数字 → 位置参数（argparse 负数匹配）
                positionals.push(t.clone());
                i += 1;
                continue;
            }
            let mut chars = shorts.chars();
            let c = chars.next().unwrap_or(' ');
            let attached: String = chars.collect();
            let Some(opt) = cmd.opts.iter().find(|o| o.short == Some(c)) else {
                extras.push(t.clone());
                i += 1;
                continue;
            };
            match opt.kind {
                Kind::Flag => {
                    if !attached.is_empty() {
                        extras.push(t.clone());
                    }
                    i += 1;
                    continue;
                }
                Kind::Str => {
                    if attached.is_empty() {
                        match toks.get(i + 1) {
                            Some(v) if !v.starts_with('-') || is_negative_number(v) => {
                                i += 2;
                                continue;
                            }
                            _ => {
                                return value_error(
                                    cmd,
                                    opt.err_name,
                                    "expected one argument".into(),
                                );
                            }
                        }
                    }
                    i += 1;
                    continue;
                }
                Kind::Int | Kind::Choice(_) => {
                    let had_attached = !attached.is_empty();
                    let raw = if had_attached {
                        attached
                    } else {
                        match toks.get(i + 1) {
                            Some(v) if !v.starts_with('-') || is_negative_number(v) => v.clone(),
                            _ => {
                                return value_error(
                                    cmd,
                                    opt.err_name,
                                    "expected one argument".into(),
                                );
                            }
                        }
                    };
                    match opt.kind {
                        Kind::Int => {
                            if raw.parse::<i64>().is_err() {
                                return value_error(
                                    cmd,
                                    opt.err_name,
                                    format!("invalid int value: {}", py_repr(&raw)),
                                );
                            }
                        }
                        Kind::Choice(list) => {
                            if !list.contains(&raw.as_str()) {
                                let joined = list
                                    .iter()
                                    .map(|c| format!("'{c}'"))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                return value_error(
                                    cmd,
                                    opt.err_name,
                                    format!(
                                        "invalid choice: {} (choose from {joined})",
                                        py_repr(&raw)
                                    ),
                                );
                            }
                        }
                        _ => unreachable!(),
                    }
                    i += if had_attached { 1 } else { 2 };
                    continue;
                }
            }
        }

        positionals.push(t.clone());
        i += 1;
    }

    // ② 缺必需位置参数
    if let Some(req) = cmd.required
        && positionals.is_empty()
    {
        return missing_error(cmd, req);
    }
    // ③ 未识别（多余位置参数并入）
    let take = usize::from(cmd.required.is_some());
    if cmd.name == "help" {
        if positionals.len() > 1 {
            extras.extend(positionals[1..].iter().cloned());
        }
    } else if positionals.len() > take {
        extras.extend(positionals[take..].iter().cloned());
    }
    if !extras.is_empty() {
        return extras_error(&extras);
    }
    Handled::Pass
}

/// 顶层分类。
pub fn classify(argv: &[OsString]) -> Handled {
    let toks: Vec<String> = argv
        .iter()
        .skip(1)
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    if toks.is_empty() {
        return Handled::Exit {
            code: 2,
            stdout: TOP_HELP,
            stderr: String::new(),
        };
    }

    // 顶层：-h/--help（含唯一前缀缩写）→ 顶层 help
    let first = &toks[0];
    if first == "-h" || first == "--help" {
        return Handled::Exit {
            code: 0,
            stdout: TOP_HELP,
            stderr: String::new(),
        };
    }
    if let Some(long) = first.strip_prefix("--") {
        let name = long.split('=').next().unwrap_or(long);
        if "help".starts_with(name) && !name.is_empty() {
            return Handled::Exit {
                code: 0,
                stdout: TOP_HELP,
                stderr: String::new(),
            };
        }
    }

    // 未知命令（非旗标首词）
    if !first.starts_with('-') {
        if !CMDS.iter().any(|c| c.name == first.as_str()) {
            let joined = CMDS
                .iter()
                .map(|c| format!("'{}'", c.name))
                .collect::<Vec<_>>()
                .join(", ");
            return Handled::Exit {
                code: 2,
                stdout: "",
                stderr: format!(
                    "{TOP_USAGE}bilibili_cli: error: argument cmd: invalid choice: {} (choose from {joined})\n",
                    py_repr(first)
                ),
            };
        }
        return parse_sub(&CMDS[first_index(first)], &toks[1..]);
    }

    // 顶层未知旗标：argparse 延后到解析完成才报（此处后无合法命令，直接报）
    extras_error(std::slice::from_ref(first))
}

fn first_index(name: &str) -> usize {
    CMDS.iter().position(|c| c.name == name).unwrap_or(0)
}

/// `cmd_help`（bilibili_cli.py:1042）。
pub fn cmd_help(topic: Option<&str>) -> u8 {
    // Python: `if not topic:` —— 空串同 None，打印总览
    if topic.is_none_or(|t| t.is_empty()) {
        print!("{HELP_OVERVIEW}");
        return 0;
    }
    let topic = topic.unwrap_or_default();
    let key = topic.trim().to_lowercase();
    let detail = match key.as_str() {
        "login" => Some(HELP_DETAIL_LOGIN),
        "list" => Some(HELP_DETAIL_LIST),
        "search" => Some(HELP_DETAIL_SEARCH),
        "info" => Some(HELP_DETAIL_INFO),
        "dl" => Some(HELP_DETAIL_DL),
        "doctor" => Some(HELP_DETAIL_DOCTOR),
        _ => None,
    };
    match detail {
        Some(text) => {
            print!("{text}");
            0
        }
        None => {
            eprint!("没有这个命令的帮助: {topic}\n\n可用：login, list, search, info, dl, doctor\n");
            2
        }
    }
}
