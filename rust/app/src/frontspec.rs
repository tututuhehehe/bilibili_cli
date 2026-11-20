//! argparse 前置层：逐字节复刻 Python argparse（3.9）对本工具命令面的
//! 接受/拒绝判定与报错排版（app-cases 的离线用例由此原生作答）。
//!
//! 语义（复刻器探针实测，含评审轮钉死的边角）：
//! ① 值错误（invalid choice / invalid int / expected one argument /
//!    ignored explicit argument / ambiguous option）——左到右即时报，
//!    prog = "bilibili_cli <cmd>"，usage = 子命令块
//! ② 缺必需位置参数——prog 带子命令，usage = 子命令块
//! ③ 未识别旗标/多余参数——最后报，prog 不带子命令，usage = 顶层块，
//!    **extras 按 argv 原序**（含顶层与子命令两侧合并）
//! 值语义：选项值可吞负数字（`^-\d+$|^-\d*\.\d+$`，Python re 的 Nd 域）
//! 与单独的 `-`，不吞其他旗标；长旗标支持唯一前缀缩写；
//! `--` 仅在命令拥有位置参数槽（含 help 的可选槽）时被吃掉，否则进 extras。
//! int() 域 = Python int()（借 parse 单元 pystr：Nd 数字、下划线、空白）。

use std::ffi::OsString;

use parse::pystr;

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
        stdout: std::borrow::Cow<'static, str>,
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
    /// 必需位置参数名（doctor 为 None；help 有可选槽，走 has_slot）
    required: Option<&'static str>,
    opts: &'static [Opt],
}

impl Cmd {
    /// 拥有位置参数槽（`--` 因此会被吃掉；login/doctor 没有 → `--` 进 extras）
    fn has_slot(&self) -> bool {
        self.required.is_some() || self.name == "help"
    }
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
                long: "cover",
                short: None,
                kind: Kind::Flag,
                err_name: "--cover",
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

fn value_error(cmd: &Cmd, err_name: &str, msg: String) -> Handled {
    Handled::Exit {
        code: 2,
        stdout: std::borrow::Cow::Borrowed(""),
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
        stdout: std::borrow::Cow::Borrowed(""),
        stderr: format!(
            "{usage}bilibili_cli {name}: error: the following arguments are required: {arg}\n",
            usage = cmd.usage,
            name = cmd.name
        ),
    }
}

fn extras_error(extras: &[(usize, String)]) -> Handled {
    let mut sorted: Vec<&(usize, String)> = extras.iter().collect();
    sorted.sort_by_key(|(idx, _)| *idx);
    let joined = sorted
        .iter()
        .map(|(_, s)| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Handled::Exit {
        code: 2,
        stdout: std::borrow::Cow::Borrowed(""),
        stderr: format!("{TOP_USAGE}bilibili_cli: error: unrecognized arguments: {joined}\n"),
    }
}

/// argparse `_negative_number_matcher`：`^-\d+$ | ^-\d*\.\d+$`（\d 为 Nd 域）。
fn is_negative_number(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('-') else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    if pystr::is_digit_str(rest) {
        return true;
    }
    if let Some((int_part, frac)) = rest.split_once('.')
        && pystr::is_digit_str(frac)
        && (int_part.is_empty() || pystr::is_digit_str(int_part))
    {
        return true;
    }
    false
}

/// 可作为选项值吞下的 token：非旗标 / 单独 `-` / 负数字。
fn consumable_value(v: &str) -> bool {
    !v.starts_with('-') || v == "-" || is_negative_number(v)
}

fn help_exit(cmd: &Cmd) -> Handled {
    // `dl -h` 的 -o 默认值按 argparse 语义随用户 HOME 展开（原版 per-user 行为）；
    // 其余命令的帮助文本无环境依赖，直接借用静态。
    if cmd.name == "dl" {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
        let rendered = cmd.subhelp.replace("__BCLI_HOME__", &home);
        return Handled::Exit {
            code: 0,
            stdout: std::borrow::Cow::Owned(rendered),
            stderr: String::new(),
        };
    }
    Handled::Exit {
        code: 0,
        stdout: std::borrow::Cow::Borrowed(cmd.subhelp),
        stderr: String::new(),
    }
}

/// 在命令的选项里找长旗标（含唯一前缀缩写）；空名 / 多义 → Err（歧义）。
fn find_long<'a>(cmd: &'a Cmd, name: &str) -> Result<Option<&'a Opt>, ()> {
    if name.is_empty() {
        return Err(());
    }
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
        _ => Err(()),
    }
}

/// argparse `ambiguous option`：只列**匹配**的选项——隐式 `--help`（若前缀命中）最前，
/// 其后按声明序（探针：--co → "could match --cover, --codec"）。
fn ambiguous_error(cmd: &Cmd, tok: &str, name: &str) -> Handled {
    let mut names: Vec<String> = Vec::new();
    if "help".starts_with(name) {
        names.push("--help".to_string());
    }
    names.extend(
        cmd.opts
            .iter()
            .filter(|o| o.long.starts_with(name))
            .map(|o| format!("--{}", o.long)),
    );
    Handled::Exit {
        code: 2,
        stdout: std::borrow::Cow::Borrowed(""),
        stderr: format!(
            "{usage}bilibili_cli {name}: error: ambiguous option: {tok} could match {joined}\n",
            usage = cmd.usage,
            name = cmd.name,
            joined = names.join(", ")
        ),
    }
}

/// 位置参数：槽内收纳，槽满 → extras（带 argv 下标，保原序）。
fn push_positional(
    cmd: &Cmd,
    positionals: &mut Vec<String>,
    extras: &mut Vec<(usize, String)>,
    t: &str,
    idx: usize,
) {
    if cmd.has_slot() && positionals.is_empty() {
        positionals.push(t.to_string());
    } else {
        extras.push((idx, t.to_string()));
    }
}

/// 解析子命令 argv 段（toks 为子命令 token；base 为其在全局 argv 的起始下标，
/// extras 直接写入共享列表以保序）。
fn parse_sub(
    cmd: &Cmd,
    toks: &[String],
    base: usize,
    extras: &mut Vec<(usize, String)>,
) -> Handled {
    let mut positionals: Vec<String> = Vec::new();
    let mut i = 0;
    let mut only_pos = false;
    while i < toks.len() {
        let t = &toks[i];
        let gidx = base + i;
        if only_pos {
            push_positional(cmd, &mut positionals, extras, t, gidx);
            i += 1;
            continue;
        }
        if t == "--" {
            if cmd.has_slot() {
                // 被位置参数槽吃掉（argparse 2412-2419；help 的可选槽同样吃）
                only_pos = true;
            } else {
                extras.push((gidx, t.clone()));
                only_pos = true;
            }
            i += 1;
            continue;
        }
        if t == "-h" || t == "--help" {
            return help_exit(cmd);
        }
        if t == "-" {
            push_positional(cmd, &mut positionals, extras, t, gidx);
            i += 1;
            continue;
        }
        if let Some(long) = t.strip_prefix("--") {
            let (name, inline_val) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (long, None),
            };
            // help 前缀：无内联 → 帮助；有内联 → ignored explicit argument
            if !name.is_empty() && "help".starts_with(name) {
                return match inline_val {
                    None => help_exit(cmd),
                    Some(v) => value_error(
                        cmd,
                        "-h/--help",
                        format!("ignored explicit argument {}", pystr::py_repr(v)),
                    ),
                };
            }
            let opt = match find_long(cmd, name) {
                Ok(Some(o)) => o,
                Ok(None) => {
                    extras.push((gidx, t.clone()));
                    i += 1;
                    continue;
                }
                Err(()) => return ambiguous_error(cmd, t, name),
            };
            let mut separate = false;
            let value: Option<String> = match opt.kind {
                Kind::Flag => {
                    if let Some(v) = inline_val {
                        return value_error(
                            cmd,
                            opt.err_name,
                            format!("ignored explicit argument {}", pystr::py_repr(v)),
                        );
                    }
                    None
                }
                Kind::Str | Kind::Int | Kind::Choice(_) => match inline_val {
                    Some(v) => Some(v.to_string()),
                    None => match toks.get(i + 1) {
                        Some(v) if consumable_value(v) => {
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
                        if pystr::py_int(raw).is_err() {
                            return value_error(
                                cmd,
                                opt.err_name,
                                format!("invalid int value: {}", pystr::py_repr(raw)),
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
                            format!(
                                "invalid choice: {} (choose from {joined})",
                                pystr::py_repr(raw)
                            ),
                        );
                    }
                    _ => {}
                }
            }
            i += if separate { 2 } else { 1 };
            continue;
        }
        if let Some(shorts) = t.strip_prefix('-') {
            if is_negative_number(t) {
                push_positional(cmd, &mut positionals, extras, t, gidx);
                i += 1;
                continue;
            }
            let mut chars = shorts.chars();
            let c = chars.next().unwrap_or(' ');
            let attached: String = chars.collect();
            if c == 'h' {
                // -h / -h<v>：附着式即 ignored explicit argument（探针实测）
                return if attached.is_empty() {
                    help_exit(cmd)
                } else {
                    value_error(
                        cmd,
                        "-h/--help",
                        format!("ignored explicit argument {}", pystr::py_repr(&attached)),
                    )
                };
            }
            let Some(opt) = cmd.opts.iter().find(|o| o.short == Some(c)) else {
                extras.push((gidx, t.clone()));
                i += 1;
                continue;
            };
            match opt.kind {
                Kind::Flag => {
                    if !attached.is_empty() {
                        extras.push((gidx, t.clone()));
                    }
                    i += 1;
                    continue;
                }
                Kind::Str => {
                    if !attached.is_empty() {
                        i += 1;
                        continue;
                    }
                    match toks.get(i + 1) {
                        Some(v) if consumable_value(v) => {
                            i += 2;
                            continue;
                        }
                        _ => return value_error(cmd, opt.err_name, "expected one argument".into()),
                    }
                }
                Kind::Int | Kind::Choice(_) => {
                    let had_attached = !attached.is_empty();
                    let raw = if had_attached {
                        attached
                    } else {
                        match toks.get(i + 1) {
                            Some(v) if consumable_value(v) => v.clone(),
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
                            if pystr::py_int(&raw).is_err() {
                                return value_error(
                                    cmd,
                                    opt.err_name,
                                    format!("invalid int value: {}", pystr::py_repr(&raw)),
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
                                        pystr::py_repr(&raw)
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
        push_positional(cmd, &mut positionals, extras, t, gidx);
        i += 1;
    }

    // ② 缺必需位置参数（先于 extras 报错，探针实测）
    if let Some(req) = cmd.required
        && positionals.is_empty()
    {
        return missing_error(cmd, req);
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
            stdout: std::borrow::Cow::Borrowed(TOP_HELP),
            stderr: String::new(),
        };
    }

    let mut extras: Vec<(usize, String)> = Vec::new();
    let mut cmd_idx: Option<usize> = None;
    for (idx, t) in toks.iter().enumerate() {
        if cmd_idx.is_some() {
            break;
        }
        if t == "-h" || t == "--help" {
            return Handled::Exit {
                code: 0,
                stdout: std::borrow::Cow::Borrowed(TOP_HELP),
                stderr: String::new(),
            };
        }
        if let Some(long) = t.strip_prefix("--") {
            if long.is_empty() {
                // 顶层裸 `--`：探针实测 → unrecognized
                extras.push((idx, t.clone()));
                continue;
            }
            let (name, inline_val) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (long, None),
            };
            if !name.is_empty() && "help".starts_with(name) {
                return match inline_val {
                    None => Handled::Exit {
                        code: 0,
                        stdout: std::borrow::Cow::Borrowed(TOP_HELP),
                        stderr: String::new(),
                    },
                    Some(v) => Handled::Exit {
                        code: 2,
                        stdout: std::borrow::Cow::Borrowed(""),
                        stderr: format!(
                            "{TOP_USAGE}bilibili_cli: error: argument -h/--help: ignored explicit argument {}\n",
                            pystr::py_repr(v)
                        ),
                    },
                };
            }
            extras.push((idx, t.clone()));
            continue;
        }
        if t.starts_with('-') && t != "-" {
            extras.push((idx, t.clone()));
            continue;
        }
        cmd_idx = Some(idx);
    }

    let Some(ci) = cmd_idx else {
        // 全旗标 → 全部按序报未识别
        return extras_error(&extras);
    };
    let first = &toks[ci];
    if !CMDS.iter().any(|c| c.name == first.as_str()) {
        let joined = CMDS
            .iter()
            .map(|c| format!("'{}'", c.name))
            .collect::<Vec<_>>()
            .join(", ");
        return Handled::Exit {
            code: 2,
            stdout: std::borrow::Cow::Borrowed(""),
            stderr: format!(
                "{TOP_USAGE}bilibili_cli: error: argument cmd: invalid choice: {} (choose from {joined})\n",
                pystr::py_repr(first)
            ),
        };
    }

    let cmd = &CMDS[first_index(first)];
    match parse_sub(cmd, &toks[ci + 1..], ci + 1, &mut extras) {
        Handled::Pass => {
            if extras.is_empty() {
                Handled::Pass
            } else {
                extras_error(&extras)
            }
        }
        other => other,
    }
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
    let key = pystr::py_lower(pystr::strip_ws(topic));
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
