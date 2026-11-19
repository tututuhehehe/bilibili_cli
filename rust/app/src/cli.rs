use std::ffi::OsString;

use clap::{Arg, ArgAction, Command, value_parser};

/// The command `units::dispatch` gets when argv names no subcommand.
pub const ROOT: &str = "bilibili_cli";

/// clap 侧 int 校验 = Python `int()`（借 parse 单元 pystr：Nd 数字、数字间下划线）。
/// frontspec 接受面与 clap 接受面由此保持一致（否则 Pass 后 clap 拒绝 → 内部失配）。
fn py_int_cli(s: &str) -> Result<i64, String> {
    parse::pystr::py_int(s)
}

fn rest() -> Arg {
    Arg::new("args")
        .num_args(0..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .value_parser(value_parser!(OsString))
}

/// Routes argv to a command. Anything this parser rejects runs the original, so it never
/// has to match every way the original accepts its arguments.
pub fn command() -> Command {
    Command::new(ROOT)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .args_conflicts_with_subcommands(true)
        .infer_long_args(true)
        .arg(rest())
        .subcommand(Command::new("help").disable_help_flag(true).arg(rest()))
        .subcommand(
            Command::new("search")
                .disable_help_flag(true)
                .infer_long_args(true)
                .arg(
                    Arg::new("keyword")
                        .required(true)
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(
                    Arg::new("limit")
                        .short('n')
                        .long("limit")
                        .default_value("20")
                        .allow_hyphen_values(true)
                        .value_parser(py_int_cli),
                )
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .default_value("totalrank")
                        .value_parser(["totalrank", "play", "new", "dm", "stow"]),
                )
                .arg(Arg::new("users").long("users").action(ArgAction::SetTrue))
                .arg(Arg::new("desc").long("desc").action(ArgAction::SetTrue))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("list")
                .disable_help_flag(true)
                .infer_long_args(true)
                .arg(
                    Arg::new("target")
                        .required(true)
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(
                    Arg::new("limit")
                        .short('n')
                        .long("limit")
                        .default_value("20")
                        .allow_hyphen_values(true)
                        .value_parser(py_int_cli),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
                .arg(Arg::new("rights").long("rights").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("info")
                .disable_help_flag(true)
                .infer_long_args(true)
                .arg(
                    Arg::new("target")
                        .required(true)
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("doctor")
                .disable_help_flag(true)
                .infer_long_args(true),
        )
        .subcommand(
            Command::new("login")
                .disable_help_flag(true)
                .infer_long_args(true)
                .arg(Arg::new("tv").long("tv").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("dl")
                .disable_help_flag(true)
                .infer_long_args(true)
                .arg(
                    Arg::new("target")
                        .required(true)
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(Arg::new("audio").long("audio").action(ArgAction::SetTrue))
                .arg(
                    Arg::new("quality")
                        .long("quality")
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(
                    Arg::new("codec")
                        .long("codec")
                        .default_value("avc")
                        .value_parser(["avc", "hevc", "av1"]),
                )
                .arg(
                    Arg::new("parallel")
                        .long("parallel")
                        .default_value("4")
                        .allow_hyphen_values(true)
                        .value_parser(py_int_cli),
                )
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(Arg::new("subs").long("subs").action(ArgAction::SetTrue))
                .arg(
                    Arg::new("keep_parts")
                        .long("keep-parts")
                        .action(ArgAction::SetTrue),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
}
